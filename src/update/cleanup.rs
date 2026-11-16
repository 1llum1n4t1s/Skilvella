//! 更新処理がSkilvella自身の置き場に残した一時物の、起動時の自動清掃。
//!
//! GitHub Releaseを廃止してR2配信へ移したため、手動ダウンロードで更新する利用者が増えた。
//! 更新ヘルパーのステージは「更新後の復旧に使えるよう保持する」設計で、アプリ自身は消さない。
//! 放置すると、更新のたびにヘルパー用の実行ファイル複製（数十MB）が実行ファイルの隣へ溜まるため、
//! 起動時に1回だけ、確実に自分が作ったと判別できる古い物に限って片付ける。
//!
//! 安全条件（すべて満たしたものだけを消す）:
//! 1. 名前・中身が完全一致する。ステージは `.skilvella-update-<UUID>` で、直下が更新処理の
//!    作る既知のファイル名だけで構成されるものに限る。1つでも未知の名前があれば利用者の物とみなして触れない。
//! 2. リンク・reparse pointは辿らず、対象にもしない。
//! 3. 最終更新が24時間より新しいものは、更新の途中・直後（復旧用の `previous-binary` を使う場面）
//!    の可能性があるため残す。時刻が未来のものも残す。
//! 4. 更新用の排他ロック（`.skilvella-update.lock`）を取れないとき（更新が進行中）は一切消さない。
//!    現在実行中のプロセスがステージ内の実行ファイルなら、そのステージも消さない。
//! 5. 失敗しても本体は止めない。標準エラーへ1行残すだけで、次回の起動で再試行する。
//!
//! 触れないもの: 現在の版の実体、`.skilvella-update.lock`（更新の排他に使い続ける空ファイル）、
//! 利用者のスキル・Library、Velopackが管理する `current` / `packages`。
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

/// この時間より新しいものは更新の途中かもしれないため消さない。
const MIN_AGE: Duration = Duration::from_secs(24 * 60 * 60);
/// 更新ステージのフォルダー名の接頭辞（`prepare` が `.skilvella-update-<UUID>` を作る）。
const STAGE_PREFIX: &str = ".skilvella-update-";
/// ステージ直下に更新処理が作る固定名。ヘルパーの実行ファイル複製の名前はOSで変わる。
const STAGE_FILES: &[&str] = &[
    "new-binary",
    "plan.json",
    "update.log",
    "ready",
    "commit",
    "cancel",
    "complete",
    "previous-binary",
    "managed-version",
    "failed-version",
];
/// 設定ファイルの原子的保存（`auto::save_settings`）が経由する一時ファイルの名前。
/// `.<設定ファイル名>.<UUID>.tmp`。保存の途中で止まった場合に残りうる。
const SETTINGS_TMP_PREFIX: &str = ".app-update.json.";
const TMP_SUFFIX: &str = ".tmp";

/// 清掃の結果。テストと、起動時の診断出力に使う。
#[derive(Debug, Default)]
pub struct Report {
    pub removed: Vec<PathBuf>,
    pub failed: Vec<(PathBuf, String)>,
}

/// 起動時に1回だけ背景スレッドで清掃する。起動を遅らせず、失敗しても本体は止めない。
pub fn spawn_background(data_dir: PathBuf) {
    let Ok(target) = std::env::current_exe() else {
        return;
    };
    let _ = std::thread::Builder::new()
        .name("skilvella-update-cleanup".into())
        .spawn(move || {
            let report = sweep(&target, &data_dir, SystemTime::now());
            for path in &report.removed {
                eprintln!("Removed a leftover update file: {}", path.display());
            }
            for (path, error) in &report.failed {
                // 次回の起動で再試行する。利用者への操作は求めない。
                eprintln!(
                    "Could not remove a leftover update file {}: {error}",
                    path.display()
                );
            }
        });
}

/// 実行ファイル `target` の隣のステージと、データフォルダー内の一時ファイルを清掃する。
/// `now` を引数にするのは、テストが時間の経過を再現できるようにするため。
pub fn sweep(target: &Path, data_dir: &Path, now: SystemTime) -> Report {
    let mut report = Report::default();
    if let Some(dir) = target.parent() {
        sweep_stages(dir, target, now, &mut report);
    }
    sweep_settings_temporaries(data_dir, now, &mut report);
    report
}

fn sweep_stages(dir: &Path, target: &Path, now: SystemTime, report: &mut Report) {
    // 候補がなければロックファイルも作らず、何も触れずに戻る（更新したことのない配置を汚さない）。
    if stage_candidates(dir, now).is_empty() {
        return;
    }
    // 更新の排他ロックを取る。取れなければ更新が進行中なので何も消さない。
    // ロックを取ってから候補を数え直し、判定後に始まった更新のステージを巻き込まない。
    let _lock = match super::installation_lock(target) {
        Ok(lock) => lock,
        Err(error) => {
            report.failed.push((dir.to_owned(), format!("{error:#}")));
            return;
        }
    };
    for stage in stage_candidates(dir, now) {
        // Windowsでは稼働中のヘルパー（管理ランチャー配下では長時間生存しうる）は削除できない。
        // 先にヘルパーの複製だけを消し、消せなければ半端に壊さずステージごと残す。
        let helper = stage.join(super::executable_name());
        if helper.exists()
            && let Err(error) = fs::remove_file(&helper)
        {
            report.failed.push((stage, error.to_string()));
            continue;
        }
        match fs::remove_dir_all(&stage) {
            Ok(()) => report.removed.push(stage),
            Err(error) => report.failed.push((stage, error.to_string())),
        }
    }
}

/// 消してよいステージの一覧。上の安全条件1〜3を判定する（4はロック側）。
fn stage_candidates(dir: &Path, now: SystemTime) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    // 実行中のプロセス自身がステージ内にいる場合（更新ヘルパー）は、そのステージを守る。
    let running_in = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_owned));
    entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let id = name.strip_prefix(STAGE_PREFIX)?;
            is_hyphenated_uuid(id).then_some(())?;
            let path = entry.path();
            if running_in.as_deref() == Some(path.as_path()) {
                return None;
            }
            is_owned_stage(&path, now).then_some(path)
        })
        .collect()
}

/// ステージとして完全一致するか（実体がフォルダー・直下が既知の名前だけ・リンクなし・十分古い）。
fn is_owned_stage(path: &Path, now: SystemTime) -> bool {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return false;
    };
    if !meta.is_dir() || is_link(&meta) {
        return false;
    }
    let Ok(children) = fs::read_dir(path) else {
        return false;
    };
    // フォルダー自身と直下の最新の更新時刻を採る。更新中のヘルパーが書き続けるログなどで
    // フォルダー自身の時刻が古いままでも、最近の書込みを見逃さない。
    // 時刻を取れないものは新しい側（消さない側）へ倒す。
    let Some(mut newest) = modified(&meta) else {
        return false;
    };
    for child in children {
        let Ok(child) = child else { return false };
        let Ok(name) = child.file_name().into_string() else {
            return false;
        };
        if !is_stage_file_name(&name) {
            return false;
        }
        let Ok(child_meta) = fs::symlink_metadata(child.path()) else {
            return false;
        };
        if is_link(&child_meta) {
            return false;
        }
        let Some(time) = modified(&child_meta) else {
            return false;
        };
        newest = newest.max(time);
    }
    old_enough(newest, now)
}

fn is_stage_file_name(name: &str) -> bool {
    STAGE_FILES.contains(&name)
        // ヘルパーとして複製した現行バイナリ（`prepare`/`spawn_installer` の `executable_name`）。
        || name == super::executable_name()
        // 管理ランチャーの参照ファイルを置換する前の一時ファイル（`launcher_pointer`）。
        || name
            .strip_prefix("cache-")
            .and_then(|rest| rest.strip_suffix(".json"))
            .is_some_and(is_hyphenated_uuid)
}

fn sweep_settings_temporaries(data_dir: &Path, now: SystemTime, report: &mut Report) {
    let Ok(entries) = fs::read_dir(data_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let Some(id) = name
            .strip_prefix(SETTINGS_TMP_PREFIX)
            .and_then(|rest| rest.strip_suffix(TMP_SUFFIX))
        else {
            continue;
        };
        if !is_hyphenated_uuid(id) {
            continue;
        }
        let path = entry.path();
        // 通常ファイルで、十分古いものだけ。
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if !meta.is_file()
            || is_link(&meta)
            || !modified(&meta).is_some_and(|time| old_enough(time, now))
        {
            continue;
        }
        match fs::remove_file(&path) {
            Ok(()) => report.removed.push(path),
            Err(error) => report.failed.push((path, error.to_string())),
        }
    }
}

/// `uuid::Uuid::new_v4().to_string()` と同じ、小文字・ハイフン区切りの形式に完全一致するか。
fn is_hyphenated_uuid(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok_and(|id| id.hyphenated().to_string() == value)
}

fn modified(meta: &fs::Metadata) -> Option<SystemTime> {
    meta.modified().ok()
}

fn old_enough(newest: SystemTime, now: SystemTime) -> bool {
    // 未来の時刻（`duration_since` が Err）は消さない。
    now.duration_since(newest).is_ok_and(|age| age >= MIN_AGE)
}

fn is_link(meta: &fs::Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // junction などの reparse point。
        if meta.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// テスト用の作業フォルダー。`SKILVELLA_MIGRATION_TEST_ROOT`（未指定時はOSの一時領域）配下に作る。
    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new(label: &str) -> Self {
            let base = std::env::var_os("SKILVELLA_MIGRATION_TEST_ROOT")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir);
            #[cfg(unix)]
            let base = base.canonicalize().unwrap();
            let root = base.join(format!(
                "skilvella-cleanup-{label}-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir_all(root.join("bin")).unwrap();
            fs::create_dir_all(root.join("data")).unwrap();
            Self { root }
        }
        fn target(&self) -> PathBuf {
            self.root.join("bin").join(super::super::executable_name())
        }
        fn data(&self) -> PathBuf {
            self.root.join("data")
        }
        /// 更新処理が作るのと同じ構成のステージを作る。
        fn stage(&self, files: &[&str]) -> PathBuf {
            let path = self
                .root
                .join("bin")
                .join(format!("{STAGE_PREFIX}{}", uuid::Uuid::new_v4()));
            fs::create_dir(&path).unwrap();
            for name in files {
                fs::write(path.join(name), b"x").unwrap();
            }
            path
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn later(hours: u64) -> SystemTime {
        SystemTime::now() + Duration::from_secs(hours * 3600)
    }

    #[test]
    fn removes_only_old_owned_stage() {
        let f = Fixture::new("stage");
        let exe = super::super::executable_name();
        let stage_files = [
            "new-binary",
            "plan.json",
            "update.log",
            "previous-binary",
            exe,
        ];
        let old = f.stage(&stage_files);
        // 現行版の実体とロックは対象外。
        fs::write(f.target(), b"current").unwrap();
        fs::write(f.root.join("bin").join(".skilvella-update.lock"), b"").unwrap();
        // 名前が違うもの（接頭辞だけ・UUIDでない・大文字UUID・別の接頭辞）。
        let decoys = [
            format!("{STAGE_PREFIX}notauuid"),
            format!(
                "{STAGE_PREFIX}{}",
                uuid::Uuid::new_v4().to_string().to_uppercase()
            ),
            format!(".skilvella-other-{}", uuid::Uuid::new_v4()),
            format!("skilvella-update-{}", uuid::Uuid::new_v4()),
        ];
        for name in &decoys {
            let path = f.root.join("bin").join(name);
            fs::create_dir(&path).unwrap();
            fs::write(path.join("plan.json"), b"x").unwrap();
        }
        // 未知のファイルを含むステージ（利用者の物とみなす）。
        let foreign = f.stage(&["plan.json", "my-notes.txt"]);
        // 通常ファイルの `.skilvella-update-<UUID>`（フォルダーでない）。
        let file_like = f
            .root
            .join("bin")
            .join(format!("{STAGE_PREFIX}{}", uuid::Uuid::new_v4()));
        fs::write(&file_like, b"x").unwrap();

        let report = sweep(&f.target(), &f.data(), later(25));

        assert_eq!(report.removed, vec![old.clone()], "{report:?}");
        assert!(report.failed.is_empty(), "{report:?}");
        assert!(!old.exists());
        assert!(f.target().is_file());
        assert!(f.root.join("bin").join(".skilvella-update.lock").is_file());
        assert!(foreign.is_dir() && file_like.is_file());
        for name in &decoys {
            assert!(f.root.join("bin").join(name).is_dir(), "{name}");
        }
    }

    #[test]
    fn keeps_recent_stage() {
        let f = Fixture::new("recent");
        let stage = f.stage(&["new-binary", "plan.json", "update.log"]);
        // 23時間後はまだ新しい（更新の途中・復旧の可能性）。
        let report = sweep(&f.target(), &f.data(), later(23));
        assert!(report.removed.is_empty() && report.failed.is_empty());
        assert!(stage.is_dir());
        // 今すぐ（作成直後）も残る。
        sweep(&f.target(), &f.data(), SystemTime::now());
        assert!(stage.is_dir());
        // 25時間後には消える。
        let report = sweep(&f.target(), &f.data(), later(25));
        assert_eq!(report.removed, vec![stage.clone()]);
        assert!(!stage.exists());
    }

    #[test]
    fn keeps_stage_while_update_lock_is_held() {
        let f = Fixture::new("locked");
        let stage = f.stage(&["new-binary"]);
        // 別のハンドルで更新ロックを保持している間は、更新が進行中とみなして何も消さない。
        let lock = super::super::installation_lock(&f.target()).unwrap();
        let report = sweep(&f.target(), &f.data(), later(48));
        assert!(report.removed.is_empty());
        assert_eq!(report.failed.len(), 1);
        assert!(stage.is_dir());
        drop(lock);
        let report = sweep(&f.target(), &f.data(), later(48));
        assert_eq!(report.removed, vec![stage]);
    }

    #[test]
    fn does_not_create_lock_when_nothing_to_clean() {
        let f = Fixture::new("nolock");
        sweep(&f.target(), &f.data(), later(48));
        assert!(!f.root.join("bin").join(".skilvella-update.lock").exists());
    }

    #[test]
    fn removes_only_old_settings_temporaries() {
        let f = Fixture::new("settings");
        let id = uuid::Uuid::new_v4();
        let old = f.data().join(format!(".app-update.json.{id}.tmp"));
        fs::write(&old, b"{").unwrap();
        // 本物の設定・Library・名前違いは残す。
        let keep = [
            "app-update.json".to_owned(),
            "library.sqlite3".to_owned(),
            ".app-update.json.notauuid.tmp".to_owned(),
            format!(".app-update.json.{}.bak", uuid::Uuid::new_v4()),
            format!("app-update.json.{}.tmp", uuid::Uuid::new_v4()),
        ];
        for name in &keep {
            fs::write(f.data().join(name), b"x").unwrap();
        }
        // 新しいうちは残る。
        sweep(&f.target(), &f.data(), later(1));
        assert!(old.is_file());
        let report = sweep(&f.target(), &f.data(), later(25));
        assert_eq!(report.removed, vec![old.clone()]);
        assert!(!old.exists());
        for name in &keep {
            assert!(f.data().join(name).is_file(), "{name}");
        }
    }

    #[test]
    fn missing_directories_are_ignored() {
        let f = Fixture::new("missing");
        let target = f.target();
        let data = f.root.join("no-such-data");
        fs::remove_dir_all(f.root.join("bin")).unwrap();
        let report = sweep(&target, &data, later(48));
        assert!(report.removed.is_empty() && report.failed.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_stage_is_not_followed() {
        let f = Fixture::new("linkstage");
        let outside = f.root.join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("plan.json"), b"keep").unwrap();
        let link = f
            .root
            .join("bin")
            .join(format!("{STAGE_PREFIX}{}", uuid::Uuid::new_v4()));
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        let report = sweep(&f.target(), &f.data(), later(48));
        assert!(report.removed.is_empty(), "{report:?}");
        assert!(outside.join("plan.json").is_file());
    }

    #[test]
    fn stage_with_a_link_is_left_alone() {
        let f = Fixture::new("link");
        let outside = f.root.join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("keep.txt"), b"x").unwrap();
        let stage = f.stage(&["plan.json"]);
        // 直下にリンクがあるステージは、リンク先に触れないよう丸ごと対象外にする。
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, stage.join("managed-version")).unwrap();
        #[cfg(windows)]
        if std::os::windows::fs::symlink_dir(&outside, stage.join("managed-version")).is_err() {
            // 権限がなければ作れない環境。シンボリックリンクの検査は他環境に任せる。
            return;
        }
        let report = sweep(&f.target(), &f.data(), later(48));
        assert!(report.removed.is_empty(), "{report:?}");
        assert!(stage.is_dir());
        assert!(outside.join("keep.txt").is_file());
    }
}
