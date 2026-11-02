use anyhow::{Context, bail};
use skilvella::{AppState, library::Library};
use std::{env, net::Ipv4Addr, path::PathBuf};

struct Options {
    data_dir: PathBuf,
    port: u16,
    watch: bool,
}
fn options() -> anyhow::Result<Option<Options>> {
    let mut options = Options {
        data_dir: default_data_dir()?,
        port: 7465,
        watch: true,
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
            Some("--help" | "-h") => {
                println!(
                    "Skilvella {}\n\nUsage: skilvella [--data-dir PATH] [--port PORT] [--no-watch]\n\nBrowser: http://127.0.0.1:7465/\nMCP:     http://127.0.0.1:7465/mcp\n\nThe server always binds to IPv4 loopback. Default data directory: {}",
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
    let Some(options) = options()? else {
        return Ok(());
    };
    // 稼働中インスタンスと競合する場合は、保存済みの設定に触れる前に終了する。
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, options.port))
        .await
        .context("Could not bind localhost port; another instance may be running")?;
    let state = AppState::new(Library::open(&options.data_dir)?);
    let home = user_home()?;
    let codex_home = env::var_os("CODEX_HOME").map(PathBuf::from);
    state
        .run(move |lib| lib.initialize_directory(&home, codex_home.as_deref()))
        .await
        .context("Could not initialize the skill directory")?;
    let port = listener.local_addr()?.port();
    let watcher = if options.watch {
        Some(skilvella::watch::start(state.clone()))
    } else {
        None
    };
    println!(
        "Skilvella {}\nBrowser: http://127.0.0.1:{port}/\nMCP:     http://127.0.0.1:{port}/mcp\nData:    {}\nStop: Ctrl+C",
        env!("CARGO_PKG_VERSION"),
        options.data_dir.display()
    );
    axum::serve(listener, skilvella::server::router(state, port))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    if let Some(watcher) = watcher {
        watcher.abort();
    }
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
