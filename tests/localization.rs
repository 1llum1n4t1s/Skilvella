use reqwest::{Client, StatusCode, header, redirect::Policy};
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
    assert!(body.contains("data-client-messages"));
    assert!(!body.contains("/assets/i18n.js"));
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
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn initial_html_is_localized_without_javascript() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".work/test-http")
        .join(uuid::Uuid::new_v4().to_string());
    let source = root.join("skills");
    std::fs::create_dir_all(&source).unwrap();
    let library = Library::open(&root.join("data")).unwrap();
    library.add_source("local", &source).unwrap();
    let id = library
        .create_skill(
            "local",
            "unique-path",
            "---\nname: 設定\ndescription: 日本語の説明\n---\n\n# 保存\n",
        )
        .unwrap();
    let state = AppState::new(library);
    let server = common::HttpServer::start(state.clone()).await;
    let client = Client::new();
    for (header_value, lang, title) in [
        ("ja", "ja", "設定"),
        ("en-GB", "en", "Settings"),
        ("zh-Hans", "zh-CN", "设置"),
        ("zh-HK", "zh-TW", "設定"),
        ("ko", "ko", "설정"),
        ("es-MX", "es", "Configuración"),
        ("de;q=0.4, fr-CA;q=0.9, en;q=0", "fr", "Paramètres"),
        ("de", "de", "Einstellungen"),
        ("pt-PT", "pt-BR", "Configurações"),
    ] {
        let body = client
            .get(format!("{}/settings", server.base))
            .header(header::ACCEPT_LANGUAGE, header_value)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(
            body.contains(&format!("<html lang=\"{lang}\">")),
            "{header_value}"
        );
        assert!(
            body.contains(&format!("<title>{title} · Skilvella</title>")),
            "{header_value}"
        );
    }
    let body = client
        .get(format!("{}/skills/{id}", server.base))
        .header(header::ACCEPT_LANGUAGE, "en")
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(body.contains("<title>設定 · Skilvella</title>"));
    assert!(body.contains("日本語の説明"));
    assert!(body.contains("<h1>保存</h1>"));
    assert!(body.contains("unique-path/SKILL.md"));

    // 件数は通知IDと値で引き渡し、遷移先のリクエストで翻訳する。
    let response = Client::builder()
        .redirect(Policy::none())
        .build()
        .unwrap()
        .post(format!("{}/scan", server.base))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(format!("csrf={}", state.csrf))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response.headers()[header::LOCATION].to_str().unwrap();
    assert!(location.starts_with("/?message=scan&indexed=1&"));
    let body = client
        .get(format!("{}{location}", server.base))
        .header(header::ACCEPT_LANGUAGE, "en")
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(body.contains("Scan complete: 1 registered, 0 errors, 0 missing skills."));
    assert!(body.contains("<title>Library · Skilvella</title>"));
}

#[tokio::test]
async fn language_cookie_requires_csrf_and_is_scoped_to_the_port() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".work/test-http")
        .join(uuid::Uuid::new_v4().to_string());
    let state = AppState::new(Library::open(&root).unwrap());
    let server = common::HttpServer::start(state.clone()).await;
    let other = common::HttpServer::start(state.clone()).await;
    let client = Client::builder().redirect(Policy::none()).build().unwrap();
    let url = format!("{}/settings/language", server.base);
    for (origin, csrf, language, status) in [
        (server.base.as_str(), "wrong", "de", StatusCode::FORBIDDEN),
        (
            other.base.as_str(),
            state.csrf.as_ref(),
            "de",
            StatusCode::FORBIDDEN,
        ),
        (
            server.base.as_str(),
            state.csrf.as_ref(),
            "de%3B+Path%3D%2F",
            StatusCode::BAD_REQUEST,
        ),
    ] {
        let response = client
            .post(&url)
            .header(header::ORIGIN, origin)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(format!("csrf={csrf}&language={language}"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        assert!(!response.headers().contains_key(header::SET_COOKIE));
    }
    let port = server.base.rsplit_once(':').unwrap().1;
    let fresh = client
        .get(format!("{}/settings", server.base))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(fresh.contains(&format!(
        "data-language-cookie=\"skilvella-language-{port}\""
    )));
    for (choice, lang) in [("de", "de"), ("auto", "fr")] {
        let response = client
            .post(&url)
            .header(header::ORIGIN, &server.base)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(format!("csrf={}&language={choice}", state.csrf))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(response.headers()[header::LOCATION], "/settings");
        let cookie = response.headers()[header::SET_COOKIE].to_str().unwrap();
        assert!(cookie.starts_with(&format!("skilvella-language-{port}={choice};")));
        assert!(cookie.contains("SameSite=Lax"));
        let cookie = cookie.split(';').next().unwrap();
        for (base, expected) in [(&server.base, lang), (&other.base, "fr")] {
            let body = client
                .get(format!("{base}/settings"))
                .header(header::ACCEPT_LANGUAGE, "fr")
                .header(header::COOKIE, cookie)
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            assert!(body.contains(&format!("<html lang=\"{expected}\">")));
            if base == &server.base {
                assert!(!body.contains("data-language-cookie"));
            }
        }
    }
}
