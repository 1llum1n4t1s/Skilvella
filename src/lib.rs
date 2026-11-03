pub mod library;
pub mod mcp;
pub mod model;
pub mod server;
pub mod update;
pub mod watch;
pub mod web;

use library::Library;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub library: Library,
    pub csrf: Arc<str>,
    pub rescan: Arc<tokio::sync::Notify>,
    pub shutdown: Arc<tokio::sync::Notify>,
}

impl AppState {
    pub fn new(library: Library) -> Self {
        Self {
            library,
            csrf: uuid::Uuid::new_v4().to_string().into(),
            rescan: Arc::new(tokio::sync::Notify::new()),
            shutdown: Arc::new(tokio::sync::Notify::new()),
        }
    }

    pub async fn run<T, F>(&self, action: F) -> anyhow::Result<T>
    where
        T: Send + 'static,
        F: FnOnce(Library) -> anyhow::Result<T> + Send + 'static,
    {
        let library = self.library.clone();
        tokio::task::spawn_blocking(move || action(library)).await?
    }

    pub async fn run_mut<T, F>(&self, action: F) -> anyhow::Result<T>
    where
        T: Send + 'static,
        F: FnOnce(Library) -> anyhow::Result<T> + Send + 'static,
    {
        self.run(move |library| {
            let result = action(library.clone());
            // 保存結果と同期状態を同じレスポンスで表示できるよう、同期の終了を待つ。
            // 同期失敗はlast_errorへ保存し、原本への保存自体は取り消さない。
            if let Err(error) = library.sync_codex_entries()
                && result.is_ok()
                && library
                    .codex_settings()
                    .map_or(true, |settings| settings.last_error.is_empty())
            {
                return Err(error
                    .context("変更を保存しましたが、Codex同期とエラー状態の保存に失敗しました"));
            }
            result
        })
        .await
    }
}
