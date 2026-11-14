use reqwest::{Client, StatusCode, header};
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
        let response = common::form_post(&client, &url)
            .body("csrf=wrong")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        // 画面のJavaScriptが「HTTP 403」ではなく再読み込みの案内を出せるよう、JSONで返す。
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["reload"], true);
        assert!(body["error"].as_str().unwrap().contains("再読み込み"));
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
    let response = common::form_post(
        &common::no_redirect_client(),
        format!("{}/scan", server.base),
    )
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
    let client = common::no_redirect_client();
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
        let response = common::form_post(&client, &url)
            .header(header::ORIGIN, origin)
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
        let response = common::form_post(&client, &url)
            .header(header::ORIGIN, &server.base)
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

const LOCALES: [&str; 8] = ["en", "de", "es", "fr", "ko", "pt-BR", "zh-CN", "zh-TW"];

fn catalog(language: &str) -> std::collections::BTreeMap<String, String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/web/locales")
        .join(format!("{language}.json"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

/// `{name}` 形式のプレースホルダー名の集合を取り出す。
fn placeholders(text: &str) -> std::collections::BTreeSet<String> {
    let mut names = std::collections::BTreeSet::new();
    let mut rest = text;
    while let Some((_, token)) = rest.split_once('{') {
        let Some((name, suffix)) = token.split_once('}') else {
            break;
        };
        names.insert(name.to_owned());
        rest = suffix;
    }
    names
}

/// テンプレートが `locale.text("…")` / `locale.format("…")` へ直接渡す文字列リテラルを集める。
fn template_keys() -> std::collections::BTreeSet<String> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("templates");
    let mut keys = std::collections::BTreeSet::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|extension| extension != "html") {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap();
        for call in ["locale.text(\"", "locale.format(\""] {
            let mut rest = source.as_str();
            while let Some((_, after)) = rest.split_once(call) {
                // Rust の文字列リテラルと同じく、バックスラッシュのエスケープを解いてキーにする。
                let mut key = String::new();
                let mut chars = after.char_indices();
                let end = loop {
                    let (offset, c) = chars.next().expect("文字列リテラルが閉じている");
                    match c {
                        '"' => break offset,
                        '\\' => key.push(chars.next().expect("エスケープが完結している").1),
                        c => key.push(c),
                    }
                };
                keys.insert(key);
                rest = &after[end..];
            }
        }
    }
    keys
}

/// 辞書は全言語で同じキー・同じプレースホルダーを持ち、空の訳を含まない。
#[test]
fn catalogs_share_keys_and_placeholders() {
    let reference = catalog(LOCALES[0]);
    assert!(!reference.is_empty());
    for language in LOCALES {
        let entries = catalog(language);
        assert!(
            entries.keys().eq(reference.keys()),
            "{language}: キー集合が en と一致しない"
        );
        for (key, value) in &entries {
            assert!(!value.trim().is_empty(), "{language}: 空の訳 {key}");
            assert_eq!(
                placeholders(key),
                placeholders(value),
                "{language}: プレースホルダーが原文と一致しない {key}"
            );
        }
    }
}

/// テンプレートが直接参照する日本語の原文は、すべての辞書に訳がある。
#[test]
fn template_keys_exist_in_every_catalog() {
    let keys = template_keys();
    assert!(keys.len() > 50, "テンプレートのキーを抽出できていない");
    for language in LOCALES {
        let entries = catalog(language);
        for key in &keys {
            assert!(
                entries.contains_key(key),
                "{language}: テンプレートのキーが辞書にない {key}"
            );
        }
    }
}

/// 履歴の理由IDと作成後の通知は、表示言語に合わせた文言で出る。
#[tokio::test]
async fn history_reasons_and_created_notice_are_localized() {
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
            "history-skill",
            "---\nname: history\ndescription: d\n---\n\nfirst\n",
        )
        .unwrap();
    let revision = library.detail(&id, false).unwrap().revision;
    library
        .save_content(
            &id,
            &revision,
            "---\nname: history\ndescription: d\n---\n\nsecond\n",
        )
        .unwrap();
    let state = AppState::new(library);
    let server = common::HttpServer::start(state.clone()).await;
    let client = Client::new();
    let fetch = |language: &'static str| {
        let url = format!("{}/skills/{id}?message=skill-created", server.base);
        let client = client.clone();
        async move {
            client
                .get(url)
                .header(header::ACCEPT_LANGUAGE, language)
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap()
                .text()
                .await
                .unwrap()
        }
    };
    let ja = fetch("ja").await;
    assert!(ja.contains("<p>編集</p>"));
    assert!(ja.contains("公開状態は一覧または管理情報で確認・変更できます。"));
    assert!(!ja.contains("<p>edit</p>"));
    let en = fetch("en").await;
    assert!(en.contains("<p>Edit</p>"));
    assert!(en.contains("check or change its publication status"));
    let zh = fetch("zh-TW").await;
    assert!(zh.contains("<p>編輯</p>"));
}
