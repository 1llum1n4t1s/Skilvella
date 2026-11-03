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
}
