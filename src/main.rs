#![cfg_attr(windows, windows_subsystem = "windows")]

use anyhow::{Context, bail};
use skilvella::{AppState, library::Library};
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
                data_dir = Some(PathBuf::from(
                    args.next().context("--data-dir needs a path")?,
                ))
            }
            Some("--port") => {
                options.port = args
                    .next()
                    .context("--port needs a number")?
                    .to_str()
                    .context("Invalid port")?
                    .parse()?
            }
            Some("--no-watch") => options.watch = false,
            Some("--no-tray") => options.tray = false,
            #[cfg(windows)]
            Some(windows_startup::NO_AUTOSTART) => {}
            Some("--help" | "-h") => {
                println!(
                    "Skilvella {}\n\nUsage: skilvella [--data-dir PATH] [--port PORT] [--no-watch] [--no-tray]{}\n\nBrowser: http://127.0.0.1:7465/\nMCP:     http://127.0.0.1:7465/mcp\n\nWindows/macOS: left-click the tray/menu bar icon to open the browser.\nRight-click for Open manager, Check for updates, Restart service, and Quit.\nUse --no-tray for headless operation.\nThe server always binds to IPv4 loopback. Default data directory: {}",
                    env!("CARGO_PKG_VERSION"),
                    if cfg!(windows) {
                        " [--no-autostart]"
                    } else {
                        ""
                    },
                    data_dir
                        .or_else(|| default_data_dir().ok())
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "unavailable; use --data-dir PATH".into())
                );
                return Ok(None);
            }
            _ => bail!("Unknown option: {} (see --help)", arg.to_string_lossy()),
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
        Ok(PathBuf::from(
            env::var_os("LOCALAPPDATA").context("LOCALAPPDATA missing; use --data-dir")?,
        )
        .join("Skilvella"))
    }
    #[cfg(target_os = "macos")]
    {
        Ok(
            PathBuf::from(env::var_os("HOME").context("HOME missing; use --data-dir")?)
                .join("Library/Application Support/Skilvella"),
        )
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let root = match env::var_os("XDG_DATA_HOME") {
            Some(path) => PathBuf::from(path),
            None => PathBuf::from(env::var_os("HOME").context("HOME missing; use --data-dir")?)
                .join(".local/share"),
        };
        Ok(root.join("skilvella"))
    }
}
fn main() -> anyhow::Result<()> {
    #[cfg(windows)]
    windows_startup::attach_parent_console();
    #[cfg(all(windows, feature = "windows-installer"))]
    velopack::VelopackApp::build().run();
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
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, options.port))
        .await
        .context("Could not bind localhost port; another instance may be running")?;
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
        .context("Could not initialize the skill directory")?;
    #[cfg(windows)]
    if !cfg!(feature = "windows-installer")
        && options.register_at_logon
        && let Err(error) = windows_startup::register_at_logon()
    {
        eprintln!("Could not register Skilvella for Windows sign-in: {error:#}");
    }
    #[cfg(windows)]
    if !cfg!(feature = "windows-installer")
        && options.register_start_menu
        && let Err(error) = windows_startup::register_start_menu()
    {
        eprintln!("Could not add Skilvella to the Start menu: {error:#}");
    }
    let port = listener.local_addr()?.port();
    println!(
        "Skilvella {}\nBrowser: http://127.0.0.1:{port}/\nMCP:     http://127.0.0.1:{port}/mcp\nData:    {}\nStop: Ctrl+C",
        env!("CARGO_PKG_VERSION"),
        options.data_dir.display()
    );
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
            eprintln!("Could not create the tray icon: {error:#}. The server is still running.");
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
    let home = PathBuf::from(home.context("User home directory is unavailable")?);
    if !home.is_absolute() {
        bail!("User home directory must be absolute");
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
