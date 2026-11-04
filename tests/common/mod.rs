use skilvella::AppState;

pub struct HttpServer {
    pub base: String,
    task: tokio::task::JoinHandle<()>,
}

impl HttpServer {
    pub async fn start(state: AppState) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let router = skilvella::server::router(state, port);
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self {
            base: format!("http://127.0.0.1:{port}"),
            task,
        }
    }
}

impl Drop for HttpServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
