#![cfg_attr(windows, windows_subsystem = "windows")]

use anyhow::{Context, bail};
use skilvella::{AppState, library::Library, locale};
use std::{env, net::Ipv4Addr, path::PathBuf};

mod desktop;
mod service;
#[cfg(windows)]
mod windows_startup;

struct Options {
    data_dir: PathBuf,
    port: u16,
    watch: bool,
    tray: bool,
    #[cfg(windows)]
    register_at_logon: bool,
    #[cfg(windows)]
    register_start_menu: bool,
}
/// Windowsの自動登録を行うかを、プログラム名を除いた起動引数だけから決める。
///
/// 戻り値は `(サインイン登録, スタートメニュー登録)`。
/// - 引数なしの通常起動では両方を登録する。
/// - `--no-autostart` だけの起動はサインイン登録から起動された場合なので、
///   Run値は登録し直さず、スタートメニューのショートカットだけを補う。
/// - それ以外の引数を伴う起動（開発・ヘッドレス運用など）ではどちらも登録しない。
///
/// 引数の解析とは独立した純関数にして、登録条件だけを単体テストで固定する。
#[cfg(windows)]
fn registration(args: &[std::ffi::OsString]) -> (bool, bool) {
    let has_no_args = args.is_empty();
    let only_no_autostart = matches!(args, [arg] if arg == windows_startup::NO_AUTOSTART);
    (has_no_args, has_no_args || only_no_autostart)
}

fn options() -> anyhow::Result<Option<Options>> {
    // 登録判定に引数全体が必要なため、いったん集めてから解析する。
    let args: Vec<_> = env::args_os().skip(1).collect();
    #[cfg(windows)]
    let (register_at_logon, register_start_menu) = registration(&args);
    let mut args = args.into_iter();
    let mut options = Options {
        data_dir: PathBuf::new(),
        port: 7465,
        watch: true,
        tray: cfg!(any(windows, target_os = "macos")),
        #[cfg(windows)]
        register_at_logon,
        #[cfg(windows)]
        register_start_menu,
    };
    let mut data_dir = None;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--data-dir") => {
                data_dir =
                    Some(PathBuf::from(args.next().with_context(|| {
                        locale::text("--data-dir にはパスが必要です")
                    })?))
            }
            Some("--port") => {
                options.port = args
                    .next()
                    .with_context(|| locale::text("--port には番号が必要です"))?
                    .to_str()
                    .with_context(|| locale::text("ポート番号が不正です"))?
                    .parse()?
            }
            Some("--no-watch") => options.watch = false,
            Some("--no-tray") => options.tray = false,
            #[cfg(windows)]
            Some(windows_startup::NO_AUTOSTART) => {}
            Some("--help" | "-h") => {
                // オプション名・URL・パスは訳さず、説明文だけを現在の言語で出す。
                let data_dir = data_dir
                    .or_else(|| default_data_dir().ok())
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| {
                        locale::text("利用できません。--data-dir PATH を指定してください")
                            .to_owned()
                    });
                println!(
                    "Skilvella {}\n\n{} skilvella [--data-dir PATH] [--port PORT] [--no-watch] [--no-tray]{}\n\nBrowser: http://127.0.0.1:7465/\nMCP:     http://127.0.0.1:7465/mcp\n\n{}\n{}\n{}\n{}",
                    env!("CARGO_PKG_VERSION"),
                    locale::text("使い方:"),
                    if cfg!(windows) {
                        " [--no-autostart]"
                    } else {
                        ""
                    },
                    locale::text(
                        "Windows/macOS: トレイ／メニューバーのアイコンを左クリックすると、ブラウザーで管理画面が開きます。"
                    ),
                    locale::text(
                        "右クリックで「管理画面を開く」「更新を確認」「サービスを再起動」「終了」を選べます。"
                    ),
                    locale::text("画面のない環境では --no-tray を指定してください。"),
                    locale::format(
                        "サーバーは常にIPv4ループバックだけで待ち受けます。既定のデータフォルダー: {path}",
                        &[("path", &data_dir)]
                    )
                );
                return Ok(None);
            }
            _ => bail!(
                "{}",
                locale::format(
                    "不明なオプション: {option}（--help を参照）",
                    &[("option", &arg.to_string_lossy())]
                )
            ),
        }
    }
    options.data_dir = match data_dir {
        Some(path) => path,
        None => default_data_dir()?,
    };
    Ok(Some(options))
}
fn default_data_dir() -> anyhow::Result<PathBuf> {
    #[cfg(windows)]
    {
        Ok(PathBuf::from(env::var_os("LOCALAPPDATA").with_context(|| {
            locale::text("LOCALAPPDATA がありません。--data-dir を指定してください")
        })?)
        .join("Skilvella"))
    }
    #[cfg(target_os = "macos")]
    {
        Ok(PathBuf::from(
            env::var_os("HOME").with_context(|| {
                locale::text("HOME がありません。--data-dir を指定してください")
            })?,
        )
        .join("Library/Application Support/Skilvella"))
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let root = match env::var_os("XDG_DATA_HOME") {
            Some(path) => PathBuf::from(path),
            None => PathBuf::from(env::var_os("HOME").with_context(|| {
                locale::text("HOME がありません。--data-dir を指定してください")
            })?)
            .join(".local/share"),
        };
        Ok(root.join("skilvella"))
    }
}
fn main() -> anyhow::Result<()> {
    #[cfg(windows)]
    windows_startup::attach_parent_console();
    #[cfg(all(windows, feature = "windows-installer"))]
    velopack::VelopackApp::build()
        // インストール直後: サインイン時の自動起動をSetup版へ向ける（ZIP版の登録を引き継ぐ。
        // windows_startup::take_over_at_logon を参照）。高速フックはVelopackに時間制限があり、
        // 失敗してもインストールを止めないため、結果は無視する。
        .on_after_install_fast_callback(|_| {
            if let Ok(executable) = env::current_exe() {
                let _ = windows_startup::take_over_at_logon(&executable);
            }
        })
        // アンインストール直前: Setup版自身を指す自動起動だけを外す。
        .on_before_uninstall_fast_callback(|_| {
            if let Ok(executable) = env::current_exe() {
                let _ = windows_startup::remove_at_logon_if_ours(&executable);
            }
        })
        .run();
    // 更新ヘルパーは同期処理だけで置換・再起動を行うため、Tokioランタイムを作る前に分岐する。
    // ヘルパーとして起動された場合に不要なワーカースレッドを生成しない。
    // インストーラー版は配置をセットアップ側が管理するため、ヘルパー分岐を持たない。
    if !cfg!(all(windows, feature = "windows-installer"))
        && skilvella::update::run_installer_if_requested()?
    {
        return Ok(());
    }
    let Some(options) = options()? else {
        return Ok(());
    };
    run(options)
}

#[tokio::main(worker_threads = 2)]
async fn run(options: Options) -> anyhow::Result<()> {
    // 稼働中インスタンスと競合する場合は、保存済みの設定に触れる前に終了する。
    let listener = match tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, options.port)).await {
        Ok(listener) => listener,
        Err(error) => {
            // Setup版はコンソールを持たないため、ZIP版など別のSkilvellaがポートを使っていて起動できないことを
            // 画面で知らせる（何も表示されずに終わると、初心者には原因が分からない）。
            #[cfg(all(windows, feature = "windows-installer"))]
            windows_startup::show_port_in_use(options.port);
            return Err(error).with_context(|| {
                locale::text(
                    "ローカルホストのポートを待ち受けできませんでした。別のインスタンスが動作している可能性があります",
                )
            });
        }
    };
    let home = user_home()?;
    let codex_home = env::var_os("CODEX_HOME").map(PathBuf::from);
    let state = AppState::new(Library::open_for_user(
        &options.data_dir,
        &home,
        codex_home.as_deref(),
    )?);
    state
        .run_mut(move |lib| lib.initialize_directory(&home, codex_home.as_deref()))
        .await
        .with_context(|| locale::text("スキルフォルダーを初期化できませんでした"))?;
    #[cfg(windows)]
    if !cfg!(feature = "windows-installer")
        && options.register_at_logon
        && let Err(error) = windows_startup::register_at_logon()
    {
        eprintln!(
            "{}",
            locale::format(
                "Windowsのサインイン時の自動起動に Skilvella を登録できませんでした: {error}",
                &[("error", &format!("{error:#}"))]
            )
        );
    }
    #[cfg(windows)]
    if !cfg!(feature = "windows-installer")
        && options.register_start_menu
        && let Err(error) = windows_startup::register_start_menu()
    {
        eprintln!(
            "{}",
            locale::format(
                "スタートメニューに Skilvella を追加できませんでした: {error}",
                &[("error", &format!("{error:#}"))]
            )
        );
    }
    let port = listener.local_addr()?.port();
    // "Browser:" "MCP:" "Data:" の行は、テストや配布検証が読み取る固定形式のため訳さない。
    println!(
        "Skilvella {}\nBrowser: http://127.0.0.1:{port}/\nMCP:     http://127.0.0.1:{port}/mcp\nData:    {}\n{}",
        env!("CARGO_PKG_VERSION"),
        options.data_dir.display(),
        locale::text("停止: Ctrl+C")
    );
    // 更新処理が残した24時間より古い自分の一時物（`.skilvella-update-*` 等）を、専用スレッドで1回だけ片付ける。
    // 起動を待たせず、失敗しても本体は止めない（標準エラーに残して次回の起動で再試行する）。
    skilvella::update::cleanup::spawn_background(state.library.data_dir().to_path_buf());
    // Setup版だけ、起動時と常駐中に更新を確認・取得する（適用は次回起動時にVelopackが行う）。
    // サービス再起動のループの外で1回だけ開始する。専用スレッドなのでTokioのワーカーは塞がない。
    #[cfg(all(windows, feature = "windows-installer"))]
    skilvella::update::auto::spawn_background(state.library.data_dir().to_path_buf());
    let (control, commands) = service::Control::new();
    let server = tokio::spawn(service::run(
        listener.into_std()?,
        state.library,
        options.watch,
        control.clone(),
        commands,
        state.shutdown,
    ));
    if options.tray {
        // AppKitはメインスレッドで動かす。HTTPはTokioのワーカースレッドで継続する。
        if let Err(error) = desktop::run(&format!("http://127.0.0.1:{port}/"), control) {
            eprintln!(
                "{}",
                locale::format(
                    "トレイアイコンを作成できませんでした: {error}。サーバーは動作を続けます。",
                    &[("error", &format!("{error:#}"))]
                )
            );
        }
    }
    let result = server.await?;
    result?;
    Ok(())
}

fn user_home() -> anyhow::Result<PathBuf> {
    #[cfg(windows)]
    let home = env::var_os("USERPROFILE").or_else(|| env::var_os("HOME"));
    #[cfg(not(windows))]
    let home = env::var_os("HOME");
    let home = PathBuf::from(
        home.with_context(|| locale::text("ユーザーのホームフォルダーを取得できません"))?,
    );
    if !home.is_absolute() {
        bail!(
            "{}",
            locale::text("ユーザーのホームフォルダーは絶対パスである必要があります")
        );
    }
    Ok(home)
}

#[cfg(all(test, windows))]
mod tests {
    use super::registration;
    use std::ffi::OsString;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn registration_follows_the_startup_arguments() {
        // 通常起動ではサインイン登録とスタートメニュー登録の両方を行う。
        assert_eq!(registration(&args(&[])), (true, true));
        // サインイン登録からの起動ではRun値を書き直さず、ショートカットだけを補う。
        assert_eq!(registration(&args(&["--no-autostart"])), (false, true));
        // `--no-autostart` に他の引数が続く場合は、利用者の明示的な起動として登録しない。
        assert_eq!(
            registration(&args(&["--no-autostart", "--no-tray"])),
            (false, false)
        );
        // 開発・ヘッドレス運用の引数ではどちらも登録しない。
        assert_eq!(
            registration(&args(&["--no-tray", "--port", "0"])),
            (false, false)
        );
    }
}
