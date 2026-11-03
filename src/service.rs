use anyhow::{Context, Result};
use skilvella::{AppState, library::Library};
use std::{
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::{mpsc, oneshot};

#[derive(Clone, Copy)]
pub enum Command {
    #[cfg_attr(not(any(windows, target_os = "macos", test)), allow(dead_code))]
    Restart,
    Quit,
}

#[derive(Clone)]
pub struct Control {
    #[cfg_attr(not(any(windows, target_os = "macos", test)), allow(dead_code))]
    sender: mpsc::UnboundedSender<Command>,
    stopped: Arc<AtomicBool>,
    restarting: Arc<AtomicBool>,
}

impl Control {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<Command>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        (
            Self {
                sender,
                stopped: Arc::new(AtomicBool::new(false)),
                restarting: Arc::new(AtomicBool::new(false)),
            },
            receiver,
        )
    }

    #[cfg(any(windows, target_os = "macos", test))]
    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
    }

    #[cfg(any(windows, target_os = "macos", test))]
    pub fn is_restarting(&self) -> bool {
        self.restarting.load(Ordering::Acquire)
    }

    #[cfg(any(windows, target_os = "macos", test))]
    pub fn restart(&self) {
        // 再起動中の連打をまとめ、終了要求は別途受け付ける。
        if !self.is_stopped()
            && !self.restarting.swap(true, Ordering::AcqRel)
            && self.sender.send(Command::Restart).is_err()
        {
            self.restarting.store(false, Ordering::Release);
        }
    }

    #[cfg(any(windows, target_os = "macos", test))]
    pub fn quit(&self) {
        let _ = self.sender.send(Command::Quit);
    }
}

struct StopOnDrop(Control);

impl Drop for StopOnDrop {
    fn drop(&mut self) {
        // エラー・パニック時も、OSのイベントループを終了させる。
        self.0.stopped.store(true, Ordering::Release);
    }
}

#[derive(Clone)]
struct ServiceListener {
    inner: Arc<tokio::sync::Mutex<tokio::net::TcpListener>>,
    address: std::net::SocketAddr,
}

impl ServiceListener {
    fn new(listener: TcpListener) -> std::io::Result<Self> {
        let address = listener.local_addr()?;
        listener.set_nonblocking(true)?;
        Ok(Self {
            inner: Arc::new(tokio::sync::Mutex::new(tokio::net::TcpListener::from_std(
                listener,
            )?)),
            address,
        })
    }
}

impl axum::serve::Listener for ServiceListener {
    type Io = tokio::net::TcpStream;
    type Addr = std::net::SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        // Axumのacceptエラー処理を維持し、再起動時は待受の所有権だけを共有する。
        axum::serve::Listener::accept(&mut *self.inner.lock().await).await
    }

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        Ok(self.address)
    }
}

pub async fn run(
    listener: TcpListener,
    library: Library,
    watch: bool,
    control: Control,
    mut commands: mpsc::UnboundedReceiver<Command>,
    shutdown: Arc<tokio::sync::Notify>,
) -> Result<()> {
    let _stopped = StopOnDrop(control.clone());
    // Windowsのtry_cloneは継承可能なソケットを作り得るため、OSハンドルを複製しない。
    // https://github.com/rust-lang/rust/issues/70719
    let listener = ServiceListener::new(listener)?;
    let port = listener.address.port();
    let signal = tokio::signal::ctrl_c();
    tokio::pin!(signal);
    loop {
        // 待受ソケットは保持し、port=0でも再起動前と同じURLを維持する。
        // Libraryも共有し、処理中の保存・移行の操作ロックを維持する。
        let incoming = listener.clone();
        let mut state = AppState::new(library.clone());
        state.shutdown = shutdown.clone();
        let watcher = watch.then(|| skilvella::watch::start(state.clone()));
        let codex_index = start_codex_index(state.library.clone());
        let (router, cancel_mcp) = skilvella::server::router_with_shutdown(state, port);
        let (stop_http, requested) = oneshot::channel();
        let mut server = tokio::spawn(async move {
            axum::serve(incoming, router)
                .with_graceful_shutdown(async {
                    let _ = requested.await;
                })
                .await
        });
        control.restarting.store(false, Ordering::Release);
        let command = tokio::select! {
            result = &mut server => {
                cancel_mcp();
                stop_watcher(watcher).await;
                stop_codex_index(codex_index).await;
                result??;
                return Ok(());
            }
            _ = &mut signal => Command::Quit,
            _ = shutdown.notified() => Command::Quit,
            command = commands.recv() => command.unwrap_or(Command::Quit),
        };
        // MCPの長時間接続を閉じてから、実行中のHTTP処理の完了を待つ。
        cancel_mcp();
        let _ = stop_http.send(());
        let result = server.await;
        stop_watcher(watcher).await;
        stop_codex_index(codex_index).await;
        result.context("The HTTP service task failed")??;
        match command {
            Command::Restart => println!("Skilvella service restarting on port {port}"),
            Command::Quit => return Ok(()),
        }
    }
}

fn start_codex_index(library: Library) -> (oneshot::Sender<()>, tokio::task::JoinHandle<()>) {
    let (stop, mut stopped) = oneshot::channel();
    let task = tokio::spawn(async move {
        let mut previous_error = String::new();
        loop {
            let library = library.clone();
            // 更新操作と同じLibrary経路を使い、同期を再帰的に呼び出さない。
            let result = tokio::task::spawn_blocking(move || library.sync_codex_entries()).await;
            let error = match result {
                Ok(Ok(())) => String::new(),
                Ok(Err(error)) => format!("{error:#}"),
                Err(error) => error.to_string(),
            };
            if !error.is_empty() && error != previous_error {
                eprintln!("Codex index synchronization failed: {error}");
            }
            previous_error = error;
            tokio::select! {
                _ = &mut stopped => break,
                _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {},
            }
        }
    });
    (stop, task)
}

async fn stop_codex_index(index: (oneshot::Sender<()>, tokio::task::JoinHandle<()>)) {
    let _ = index.0.send(());
    // ファイル更新中のspawn_blockingを途中で放置せず、完了を待つ。
    let _ = index.1.await;
}

async fn stop_watcher(watcher: Option<tokio::task::JoinHandle<()>>) {
    if let Some(watcher) = watcher {
        watcher.abort();
        let _ = watcher.await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skilvella::model::{SearchQuery, SkillMetadata};
    use std::{fs, path::PathBuf, time::Duration};

    #[cfg(windows)]
    #[tokio::test]
    async fn service_sockets_are_not_inherited_by_the_update_helper() {
        use std::os::windows::io::AsRawSocket;
        use windows_sys::Win32::Foundation::{GetHandleInformation, HANDLE_FLAG_INHERIT};

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let listener = ServiceListener::new(listener).unwrap();
        let mut incoming = listener.clone();
        let socket = incoming.inner.lock().await.as_raw_socket();
        let client = std::net::TcpStream::connect(incoming.address).unwrap();
        let (accepted, _) = axum::serve::Listener::accept(&mut incoming).await;
        for socket in [socket, accepted.as_raw_socket()] {
            let mut flags = 0;
            assert_ne!(unsafe { GetHandleInformation(socket as _, &mut flags) }, 0);
            assert_eq!(flags & HANDLE_FLAG_INHERIT, 0);
        }
        drop(client);
    }

    fn csrf(html: &str) -> &str {
        html.split("name=\"csrf\" value=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
    }

    #[tokio::test]
    async fn restart_keeps_url_and_skills_resumes_watch_and_quit_releases_port() {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".work/test-startup")
            .join(uuid::Uuid::new_v4().to_string());
        let source = base.join("skills");
        fs::create_dir_all(source.join("demo")).unwrap();
        let file = source.join("demo/SKILL.md");
        fs::write(
            &file,
            "---\nname: restart-fixture\ndescription: before restart\n---\nBody\n",
        )
        .unwrap();
        let home = base.join("home");
        fs::create_dir_all(&home).unwrap();
        let library = Library::open_for_user(&base.join("data"), &home, None).unwrap();
        library.add_source("local", &source).unwrap();
        let skill = library
            .search(SearchQuery::default(), false)
            .unwrap()
            .skills
            .remove(0);
        library
            .set_metadata(
                &skill.id,
                SkillMetadata {
                    tags: vec!["retained".into()],
                    category: "test".into(),
                    published: true,
                },
            )
            .unwrap();
        library.set_codex_policy(&skill.id, false, true).unwrap();
        library.set_codex_enabled(true).unwrap();
        let codex_entry =
            PathBuf::from(library.codex_settings().unwrap().path).join("restart-fixture/SKILL.md");
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let url = format!("http://{address}");
        let (control, commands) = Control::new();
        let service = tokio::spawn(run(
            listener,
            library.clone(),
            true,
            control.clone(),
            commands,
            Arc::new(tokio::sync::Notify::new()),
        ));
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap();
        let before = client.get(&url).send().await.unwrap().text().await.unwrap();
        let token = csrf(&before).to_owned();
        control.restart();
        control.restart(); // メニューの連打でも、一度の再起動にまとめる。
        assert!(control.is_restarting());
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                // 再起動で閉じたkeep-alive接続は次のGETで張り直す。
                if let Ok(response) = client.get(&url).send().await
                    && let Ok(page) = response.text().await
                    && csrf(&page) != token
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(30)).await;
            }
        })
        .await
        .unwrap();
        assert!(!control.is_restarting());
        let stale = client
            .post(format!("{url}/scan"))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!("csrf={token}"))
            .send()
            .await
            .unwrap();
        assert_eq!(stale.status(), reqwest::StatusCode::FORBIDDEN);
        let metadata = library.detail(&skill.id, true).unwrap().summary;
        assert_eq!(metadata.tags, ["retained"]);
        assert_eq!(metadata.category, "test");
        let mcp: serde_json::Value = client
            .post(format!("{url}/mcp"))
            .header("accept", "application/json, text/event-stream")
            .header("mcp-protocol-version", "2025-11-25")
            .json(
                &serde_json::json!({"jsonrpc":"2.0", "id":1, "method":"tools/call", "params":{
                    "name":"search_skills", "arguments":{"query":"restart-fixture"}
                }}),
            )
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(mcp["result"]["structuredContent"]["total"], 1);
        // 新しいwatcherが登録を終えるのを待ち、再起動後の外部編集を検出させる。
        // イベントに依存せず確認できるよう、待機中は同じファイルへの編集を続ける。
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                fs::write(
                    &file,
                    "---\nname: restart-fixture\ndescription: after restart\n---\nUpdated\n",
                )
                .unwrap();
                tokio::time::sleep(Duration::from_millis(650)).await;
                if library.search(SearchQuery::default(), true).unwrap().skills[0].description
                    == "after restart"
                {
                    break;
                }
            }
        })
        .await
        .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if fs::read_to_string(&codex_entry).is_ok_and(|text| text.contains("after restart"))
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .expect("The built-in index did not follow the watched source after restart");
        control.quit();
        tokio::time::timeout(Duration::from_secs(5), service)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(control.is_stopped());
        assert!(TcpListener::bind(address).is_ok());
        assert!(client.get(&url).send().await.is_err());
    }
}
