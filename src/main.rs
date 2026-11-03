use anyhow::{Context, bail};
use skilvella::{AppState, library::Library};
use std::{env, net::Ipv4Addr, path::PathBuf};

mod desktop;
mod service;

struct Options {
    data_dir: PathBuf,
    port: u16,
    watch: bool,
    tray: bool,
}
fn options() -> anyhow::Result<Option<Options>> {
    let mut options = Options {
        data_dir: default_data_dir()?,
        port: 7465,
        watch: true,
        tray: cfg!(any(windows, target_os = "macos")),
    };
    let mut args = env::args_os().skip(1);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--data-dir") => {
                options.data_dir = args.next().context("--data-dir needs a path")?.into()
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
            Some("--help" | "-h") => {
                println!(
                    "Skilvella {}\n\nUsage: skilvella [--data-dir PATH] [--port PORT] [--no-watch] [--no-tray]\n\nBrowser: http://127.0.0.1:7465/\nMCP:     http://127.0.0.1:7465/mcp\n\nWindows/macOS: left-click the tray/menu bar icon to open the browser.\nRight-click for Open manager, Check for updates, Restart service, and Quit.\nUse --no-tray for headless operation.\nThe server always binds to IPv4 loopback. Default data directory: {}",
                    env!("CARGO_PKG_VERSION"),
                    options.data_dir.display()
                );
                return Ok(None);
            }
            _ => bail!("Unknown option: {} (see --help)", arg.to_string_lossy()),
        }
    }
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
        Ok(env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or(
                PathBuf::from(env::var_os("HOME").context("HOME missing; use --data-dir")?)
                    .join(".local/share"),
            )
            .join("skilvella"))
    }
}
#[tokio::main(worker_threads = 2)]
async fn main() -> anyhow::Result<()> {
    if skilvella::update::run_installer_if_requested()? {
        return Ok(());
    }
    let Some(options) = options()? else {
        return Ok(());
    };
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
        .run(move |lib| lib.initialize_directory(&home, codex_home.as_deref()))
        .await
        .context("Could not initialize the skill directory")?;
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
