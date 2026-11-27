//! Windowsインストーラー版（Velopack）の自動更新。
//!
//! # 目的
//! AI初心者がコマンドを打たずに常に最新版を使えるよう、アプリの起動時と常駐中の24時間ごとに
//! 更新を確認し、新版があれば無言で取得だけ行う。適用は次回起動時に
//! `VelopackApp::run()`（既定でauto apply）が行うため、ここでは再起動しない。
//!
//! # 設計上の制約
//! - 設定（`auto_update`）と最終確認時刻は、Libraryとは別の小さなJSON（データフォルダー直下の
//!   `app-update.json`）に置く。AGENTS.md の「アプリ更新でLibraryや利用者スキルを変更しない」
//!   制約を守るため、SQLiteのsettingsテーブルは使わない。Velopackの導入先（`current\`）は
//!   更新のたびに入れ替わるので保存先にしない。
//! - 設定ファイルの読み書き・間引きの判定は、プラットフォームに依存しない純粋な処理として
//!   常にコンパイルし、単体テストで固定する。Velopackを呼ぶ部分だけを
//!   `windows-installer` 機能付きのWindowsビルドに限定する。ZIP版・macOS版・Toolport経路の
//!   挙動はこのモジュールの影響を受けない。
//! - 更新元（フィード）の生成は [`update_source`] の1か所に閉じる。公開先を差し替えるときは
//!   この関数だけを直す。
use anyhow::{Context, ensure};
use std::{
    fs,
    io::Write,
    path::Path,
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// 設定・最終確認時刻を保存するファイル名（データフォルダー直下）。
pub const SETTINGS_FILE: &str = "app-update.json";
/// 前回の確認からこの時間が過ぎるまでは、ネットワークへ出ない。
pub const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
/// 常駐中に「確認の時期か」を見直す間隔。24時間のsleepにせず壁時計で判定し直すことで、
/// PCのスリープ明けや設定変更にも1時間以内に追随する。
pub const POLL_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// このビルドがVelopackのSetup版（自動更新の対象）か。設定画面の表示切替に使う。
pub const INSTALLER_BUILD: bool = cfg!(all(windows, feature = "windows-installer"));

/// 自動更新の設定と前回の確認時刻。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoUpdateSettings {
    /// 起動時・常駐中に自動で確認・取得するか。未設定は `true`（既定ON）。
    pub auto_update: bool,
    /// 前回の確認に成功した時刻（UNIX秒）。未確認・読めない場合は `None`。
    pub last_checked_at: Option<u64>,
}

impl Default for AutoUpdateSettings {
    fn default() -> Self {
        Self {
            auto_update: true,
            last_checked_at: None,
        }
    }
}

/// JSONの内容から設定を作る。
///
/// 壊れたJSON・型の違う項目は項目ごとに既定値へ戻し、決して失敗しない
/// （更新機能の不調でアプリの起動や設定画面を止めないため）。
pub fn parse_settings(bytes: &[u8]) -> AutoUpdateSettings {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return AutoUpdateSettings::default();
    };
    AutoUpdateSettings {
        auto_update: value
            .get("auto_update")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true),
        last_checked_at: value
            .get("last_checked_at")
            .and_then(serde_json::Value::as_u64),
    }
}

/// データフォルダーから設定を読む。無い・読めない・リンクの場合は既定値を返す。
pub fn load_settings(data_dir: &Path) -> AutoUpdateSettings {
    let path = data_dir.join(SETTINGS_FILE);
    // シンボリックリンク等を辿らない。通常ファイル以外は存在しないものとして扱う。
    match fs::symlink_metadata(&path) {
        Ok(meta) if meta.is_file() => fs::read(&path)
            .map(|bytes| parse_settings(&bytes))
            .unwrap_or_default(),
        _ => AutoUpdateSettings::default(),
    }
}

/// 同じ一時ファイル経由の原子的置換で保存する。途中で止まっても旧ファイルか新ファイルの
/// どちらかが残り、半端なJSONは作らない。
fn save_settings(data_dir: &Path, settings: &AutoUpdateSettings) -> anyhow::Result<()> {
    let path = data_dir.join(SETTINGS_FILE);
    if let Ok(meta) = fs::symlink_metadata(&path) {
        ensure!(
            meta.is_file(),
            "The update settings path is not a regular file"
        );
    }
    let body = serde_json::to_vec_pretty(&serde_json::json!({
        "auto_update": settings.auto_update,
        "last_checked_at": settings.last_checked_at,
    }))?;
    let temporary = data_dir.join(format!(".{SETTINGS_FILE}.{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> anyhow::Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(&body)?;
        file.sync_all()?;
        // Windowsでも既存ファイルを置換できる（MoveFileEx REPLACE_EXISTING）。
        fs::rename(&temporary, &path)?;
        Ok(())
    })();
    if result.is_err() {
        // 後始末の失敗は元のエラーを優先して無視する（一時ファイルは次回以降に影響しない）。
        let _ = fs::remove_file(&temporary);
    }
    result.context("Could not save the app update settings")
}

/// 設定の読み書きを直列化する。背景スレッドの確認時刻の記録と設定画面の保存が
/// 同時に走っても、片方の更新を失わない（読み取り→変更→保存を1つのロックで囲む）。
static SETTINGS_LOCK: Mutex<()> = Mutex::new(());

fn update_settings(
    data_dir: &Path,
    change: impl FnOnce(&mut AutoUpdateSettings),
) -> anyhow::Result<()> {
    let _guard = SETTINGS_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut settings = load_settings(data_dir);
    change(&mut settings);
    save_settings(data_dir, &settings)
}

/// 自動更新のON/OFFを保存する。最終確認時刻は保持する。
pub fn set_auto_update(data_dir: &Path, enabled: bool) -> anyhow::Result<()> {
    update_settings(data_dir, |settings| settings.auto_update = enabled)
}

/// 確認に成功した時刻を記録する。ON/OFFは変更しない。
pub fn record_checked(data_dir: &Path, now: u64) -> anyhow::Result<()> {
    update_settings(data_dir, |settings| settings.last_checked_at = Some(now))
}

/// 現在時刻（UNIX秒）。時計が1970年より前の異常値でも0として扱う。
pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// 前回の確認から24時間以上たったか。
///
/// - 未確認は「時期」とみなす。
/// - 前回時刻が未来（時計の巻き戻し・手編集）の場合も、永久に確認しなくなるのを避けて
///   「時期」とみなす。
pub fn is_due(last_checked_at: Option<u64>, now: u64) -> bool {
    match last_checked_at {
        None => true,
        Some(last) if last > now => true,
        Some(last) => now - last >= CHECK_INTERVAL.as_secs(),
    }
}

/// 自動で確認・取得をするべきか（ON かつ 間引きの時期）。手動確認は間引きを無視するため使わない。
pub fn should_check(settings: &AutoUpdateSettings, now: u64) -> bool {
    settings.auto_update && is_due(settings.last_checked_at, now)
}

#[cfg(all(windows, feature = "windows-installer"))]
pub use installer::{manual_check, restart_to_apply, spawn_background};

/// Velopackを呼ぶ部分。Setup版のWindowsビルドだけがコンパイルする。
#[cfg(all(windows, feature = "windows-installer"))]
mod installer {
    use super::*;
    use crate::update::UpdateInfo;
    use std::{path::PathBuf, sync::TryLockError};
    use velopack::{UpdateCheck, UpdateManager, sources::UpdateSource};

    /// 本番の更新元。公開先（R2）の固定URLで、`releases.win.json` と
    /// `*-full.nupkg` をここから配信する（[`UpdateManager`] は `releases.<channel>.json` を読む）。
    const FEED_URL: &str = "https://skilvella.kagayoi.com";

    /// 更新元を作る。公開先を変えるときはこの関数だけを差し替える。
    ///
    /// `update-feed-test` 機能付きのビルドに限り、環境変数 `SKILVELLA_UPDATE_FEED_DIR` の
    /// ローカルフォルダーを更新元にできる（隔離したE2E検証用。本番ビルドにはこの分岐を残さない）。
    fn update_source() -> Box<dyn UpdateSource> {
        #[cfg(feature = "update-feed-test")]
        if let Some(directory) = std::env::var_os("SKILVELLA_UPDATE_FEED_DIR") {
            return Box::new(velopack::sources::FileSource::new(PathBuf::from(directory)));
        }
        Box::new(velopack::sources::HttpSource::new(FEED_URL))
    }

    /// 導入済みでない実行ファイル（開発ビルドなど）ではここで失敗する。
    fn manager() -> anyhow::Result<UpdateManager> {
        UpdateManager::new_boxed(update_source(), None, None)
            .map_err(|error| anyhow::anyhow!("Velopack update manager is unavailable: {error}"))
    }

    /// 確認・取得・適用の操作を直列化する。Velopackの取得も排他ロックを取るが、
    /// その失敗を利用者向けの「別の更新操作が進行中」に置き換えるため先に自前で止める。
    static OPERATION: Mutex<()> = Mutex::new(());

    fn try_operation() -> anyhow::Result<std::sync::MutexGuard<'static, ()>> {
        match OPERATION.try_lock() {
            Ok(guard) => Ok(guard),
            Err(TryLockError::Poisoned(poisoned)) => Ok(poisoned.into_inner()),
            Err(TryLockError::WouldBlock) => {
                anyhow::bail!("Another update operation is already in progress")
            }
        }
    }

    /// 背景の確認・取得を1回行う。新版があれば取得だけして、適用は次回起動に任せる。
    /// 成功（更新なしを含む）したときだけ確認時刻を記録し、失敗時は次の見直しで再試行する。
    fn background_once(data_dir: &Path) -> anyhow::Result<String> {
        let _operation = try_operation()?;
        let manager = manager()?;
        let message = match manager.check_for_updates()? {
            UpdateCheck::UpdateAvailable(info) => {
                // 取得済みなら即座に戻る。取得はVelopackが.partialから原子的に確定する。
                manager.download_updates(&info, None)?;
                format!(
                    "downloaded {}; it is applied at the next launch",
                    info.TargetFullRelease.Version
                )
            }
            UpdateCheck::NoUpdateAvailable => "up to date".to_owned(),
            UpdateCheck::RemoteIsEmpty => "the update feed is empty".to_owned(),
        };
        record_checked(data_dir, now_unix())?;
        Ok(message)
    }

    fn background_cycle(data_dir: &Path) {
        if !should_check(&load_settings(data_dir), now_unix()) {
            return;
        }
        match background_once(data_dir) {
            Ok(message) => eprintln!(
                "{}",
                crate::locale::format("Skilvellaの自動更新: {message}", &[("message", &message)])
            ),
            // 失敗しても現行版で動き続ける。ログだけ残す。
            Err(error) => eprintln!(
                "{}",
                crate::locale::format(
                    "Skilvellaの自動更新に失敗しました: {error}",
                    &[("error", &format!("{error:#}"))]
                )
            ),
        }
    }

    /// 起動後に1回確認し、以降は常駐中も定期的に見直す専用スレッドを開始する。
    ///
    /// Tokioのワーカー（2本）を塞がないよう、同期I/Oは専用のstd::threadで行う。
    /// プロセス終了で一緒に止まるためjoinしない。呼び出しは起動ごとに1回だけにする
    /// （サービス再起動のループ内では呼ばない）。
    pub fn spawn_background(data_dir: PathBuf) {
        let spawned = std::thread::Builder::new()
            .name("skilvella-auto-update".into())
            .spawn(move || {
                loop {
                    background_cycle(&data_dir);
                    std::thread::sleep(POLL_INTERVAL);
                }
            });
        if let Err(error) = spawned {
            eprintln!(
                "{}",
                crate::locale::format(
                    "自動更新のスレッドを開始できませんでした: {error}",
                    &[("error", &error.to_string())]
                )
            );
        }
    }

    /// 設定画面からの手動確認。間引きを無視し、Velopackで確認する。
    ///
    /// 画面のJavaScriptが `supported` を検査するため、常に妥当な値を返す。
    pub fn manual_check() -> anyhow::Result<UpdateInfo> {
        let _operation = try_operation()?;
        let manager = manager()?;
        let current = manager.get_current_version_as_string();
        let (available, latest) = match manager.check_for_updates()? {
            UpdateCheck::UpdateAvailable(info) => (true, info.TargetFullRelease.Version.clone()),
            UpdateCheck::NoUpdateAvailable | UpdateCheck::RemoteIsEmpty => (false, current.clone()),
        };
        // 手動確認は間引きを無視するだけで、確認時刻は記録しない。新版が見つかっても
        // 取得していない段階で記録すると、次の自動取得が24時間止まってしまうため。
        // 自動確認の起点は、取得まで終えた背景の確認だけが更新する。
        Ok(UpdateInfo {
            current_version: current,
            latest_version: latest,
            available,
            supported: true,
        })
    }

    /// 利用者の明示操作による適用。取得（済みなら省略）の後、アプリの終了を待って
    /// Velopackに適用させ、同じ起動引数で再起動させる。
    ///
    /// `apply_updates_and_restart` はこの関数内で `exit(0)` するためHTTP応答が返せない。
    /// そこで `wait_exit_then_apply_updates` を使い、呼び出し側が応答を返して通常の終了処理
    /// （`shutdown`）へ進む。Velopackの終了待ちは60秒なので、利用者の操作の直後という
    /// 終了経路の直前に限って使う（常駐中の自動取得では使わない）。
    pub fn restart_to_apply() -> anyhow::Result<()> {
        let _operation = try_operation()?;
        let manager = manager()?;
        let UpdateCheck::UpdateAvailable(info) = manager.check_for_updates()? else {
            anyhow::bail!("The installed version is already current");
        };
        manager.download_updates(&info, None)?;
        // 再起動後も同じ --data-dir / --port で動かすため、現在の引数をそのまま渡す。
        let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
        manager.wait_exit_then_apply_updates(&info.TargetFullRelease, false, true, args)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> std::path::PathBuf {
        let root = std::env::var_os("SKILVELLA_MIGRATION_TEST_ROOT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join(format!("auto-update-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn corrupt_file_on_disk_is_ignored_and_can_be_overwritten() {
        let dir = scratch();
        fs::write(dir.join(SETTINGS_FILE), b"{ broken").unwrap();
        assert_eq!(load_settings(&dir), AutoUpdateSettings::default());
        assert!(AutoUpdateSettings::default().auto_update);
        set_auto_update(&dir, false).unwrap();
        assert!(!load_settings(&dir).auto_update);
        // 型が違う項目だけが既定値へ戻り、読める項目は生きる。
        fs::write(
            dir.join(SETTINGS_FILE),
            br#"{"auto_update":false,"last_checked_at":"x"}"#,
        )
        .unwrap();
        let settings = load_settings(&dir);
        assert!(!settings.auto_update);
        assert_eq!(settings.last_checked_at, None);
        // 型が違う auto_update は既定（ON）へ戻り、読める last_checked_at は生きる。
        fs::write(
            dir.join(SETTINGS_FILE),
            br#"{"auto_update":"no","last_checked_at":7}"#,
        )
        .unwrap();
        let settings = load_settings(&dir);
        assert!(settings.auto_update);
        assert_eq!(settings.last_checked_at, Some(7));
    }

    #[test]
    fn writes_are_atomic_and_preserve_the_other_field() {
        let dir = scratch();
        record_checked(&dir, 1_000).unwrap();
        set_auto_update(&dir, false).unwrap();
        let settings = load_settings(&dir);
        assert!(!settings.auto_update);
        assert_eq!(settings.last_checked_at, Some(1_000));
        record_checked(&dir, 2_000).unwrap();
        let settings = load_settings(&dir);
        assert!(!settings.auto_update);
        assert_eq!(settings.last_checked_at, Some(2_000));
        // 一時ファイルを残さない。
        let names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, [SETTINGS_FILE]);
    }

    #[test]
    fn throttle_waits_24_hours_and_treats_future_timestamps_as_due() {
        let day = CHECK_INTERVAL.as_secs();
        assert!(is_due(None, 10));
        assert!(!is_due(Some(1_000), 1_000));
        assert!(!is_due(Some(1_000), 1_000 + day - 1));
        assert!(is_due(Some(1_000), 1_000 + day));
        // 時計の巻き戻しで前回時刻が未来になっても、永久に確認しない状態にならない。
        assert!(is_due(Some(5_000), 1_000));
        // 自動更新がOFFなら、期限を過ぎていても確認しない。
        let off = AutoUpdateSettings {
            auto_update: false,
            last_checked_at: None,
        };
        assert!(!should_check(&off, 10));
        let on = AutoUpdateSettings {
            auto_update: true,
            last_checked_at: None,
        };
        assert!(should_check(&on, 10));
    }
}
