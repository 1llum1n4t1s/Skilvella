use crate::{AppState, mcp::SkillServer, web};
use axum::{
    Router,
    extract::{DefaultBodyLimit, Request},
    http::{HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};

pub fn router(state: AppState, port: u16) -> Router {
    router_with_shutdown(state, port).0
}

/// MCPの長時間接続も停止できる、デスクトップ常駐用のルーター。
pub fn router_with_shutdown(state: AppState, port: u16) -> (Router, impl FnOnce()) {
    let hosts = vec![format!("127.0.0.1:{port}"), format!("localhost:{port}")];
    let origins: Vec<_> = hosts.iter().map(|host| format!("http://{host}")).collect();
    let mcp_state = state.clone();
    let mut config = StreamableHttpServerConfig::default();
    config.json_response = true;
    config.legacy_session_mode = false;
    config.allowed_hosts = hosts.clone();
    config.allowed_origins = origins.clone();
    let cancellation = config.cancellation_token.clone();
    let service = StreamableHttpService::new(
        move || Ok(SkillServer::new(mcp_state.clone())),
        LocalSessionManager::default().into(),
        config,
    );
    // URL エンコードされた UTF-8 本文は最大3倍になる。本文自体の上限は保存層で検査する。
    let router = web::router()
        .nest_service("/mcp", service)
        .layer(DefaultBodyLimit::max(6 * 1024 * 1024 + 64 * 1024))
        .layer(middleware::from_fn(move |request: Request, next: Next| {
            let hosts = hosts.clone();
            let origins = origins.clone();
            async move { local_boundary(request, next, &hosts, &origins).await }
        }))
        .with_state(state);
    (router, move || cancellation.cancel())
}
async fn local_boundary(
    request: Request,
    next: Next,
    hosts: &[String],
    origins: &[String],
) -> Response {
    let headers = request.headers();
    let host_ok = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|host| {
            hosts
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(host))
        });
    let origin_ok = headers
        .get(header::ORIGIN)
        .map(|origin| {
            origin
                .to_str()
                .ok()
                .is_some_and(|value| origins.iter().any(|allowed| allowed == value))
        })
        .unwrap_or(true);
    if !host_ok || !origin_ok {
        return (StatusCode::FORBIDDEN, "Local host/origin required").into_response();
    }
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'"));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    // no-referrer は通常のフォーム POST の Origin も null にするため同一サイト内だけ保持する。
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
