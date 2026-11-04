use reqwest::{Client, StatusCode};
use skilvella::{AppState, library::Library};

mod common;

#[tokio::test]
async fn settings_are_read_only_and_update_actions_require_csrf() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".work/test-http")
        .join(uuid::Uuid::new_v4().to_string());
    let state = AppState::new(Library::open(&root).unwrap());
    let server = common::HttpServer::start(state.clone()).await;
    let client = Client::new();
    let base = server.base.clone();
    let response = client
        .get(format!("{base}/settings?check_updates=1"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.text().await.unwrap();
    assert!(body.contains("data-language-selector"));
    assert!(body.contains("data-theme-selector"));
    assert!(body.contains(state.csrf.as_ref()));
    assert!(body.contains("/assets/i18n.js"));
    for action in ["check", "install"] {
        let url = format!("{base}/settings/updates/{action}");
        assert_eq!(
            client.get(&url).send().await.unwrap().status(),
            StatusCode::METHOD_NOT_ALLOWED
        );
        let response = client
            .post(&url)
            .header("content-type", "application/x-www-form-urlencoded")
            .body("csrf=wrong")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    let response = client
        .get(format!("{base}/assets/i18n.js"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/javascript")
    );
}
