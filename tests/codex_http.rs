use reqwest::{Client, StatusCode};
use skilvella::{AppState, library::Library};
use std::fs;

#[tokio::test]
async fn codex_forms_validate_requests_and_preserve_modes() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".work/test-http")
        .join(uuid::Uuid::new_v4().to_string());
    let home = root.join("home");
    let source = root.join("source");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(source.join("example")).unwrap();
    fs::write(
        source.join("example/SKILL.md"),
        "---\nname: example\ndescription: Example skill\n---\nOriginal body\n",
    )
    .unwrap();
    let library = Library::open_for_user(&root.join("data"), &home, None).unwrap();
    library.add_source("test", &source).unwrap();
    let id = library.search(Default::default(), false).unwrap().skills[0]
        .id
        .clone();
    let state = AppState::new(library);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let router = skilvella::server::router(state.clone(), port);
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    struct Stop(tokio::task::JoinHandle<()>);
    impl Drop for Stop {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let _stop = Stop(task);
    let base = format!("http://127.0.0.1:{port}");
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let policy_url = format!("{base}/skills/{id}/codex");
    let bulk_url = format!("{base}/bulk");
    let settings_url = format!("{base}/settings/codex");
    let token = state.csrf.to_string();
    let detail_url = format!("{base}/skills/{id}");
    let body = client
        .get(&detail_url)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(body.contains("value=\"search\" selected"));
    assert!(body.contains("Codexでの使い方"));
    assert!(!state.library.codex_settings().unwrap().enabled);
    assert!(body.contains("設定画面のCodex連携が有効で、同期に成功した場合に適用されます。"));
    assert!(body.contains("href=\"/settings#codex\""));
    for (url, field, value) in [
        (&policy_url, "mode", "command"),
        (&settings_url, "enabled", "true"),
        (&bulk_url, "codex_mode", "command"),
    ] {
        assert_eq!(
            client.get(url).send().await.unwrap().status(),
            StatusCode::METHOD_NOT_ALLOWED
        );
        for bad_token in ["", "wrong"] {
            assert_eq!(
                client
                    .post(url)
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(format!("csrf={bad_token}&{field}={value}"))
                    .send()
                    .await
                    .unwrap()
                    .status(),
                StatusCode::FORBIDDEN
            );
        }
        assert_eq!(
            client
                .post(url)
                .header("origin", "https://attacker.example")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(format!("csrf={token}&{field}={value}"))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        client
            .post(&policy_url)
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!("csrf={token}&mode=unknown"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    for (mode, automatic, command) in [
        ("command", false, true),
        ("automatic", true, true),
        ("search", false, false),
    ] {
        assert_eq!(
            client
                .post(&policy_url)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(format!("csrf={token}&mode={mode}"))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::SEE_OTHER
        );
        let body = client
            .get(&detail_url)
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(body.contains(&format!("value=\"{mode}\" selected")));
        let policy = state.library.codex_policy(&id).unwrap();
        assert_eq!((policy.automatic, policy.command), (automatic, command));
    }
    for (url, body) in [
        (
            format!("{base}/skills/{id}/metadata"),
            format!("csrf={token}&published=true"),
        ),
        (policy_url.clone(), format!("csrf={token}&mode=command")),
    ] {
        assert_eq!(
            client
                .post(url)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(body)
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::SEE_OTHER
        );
    }
    for name in ["second", "untouched"] {
        fs::create_dir_all(source.join(name)).unwrap();
        fs::write(
            source.join(name).join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Another skill\n---\nBody\n"),
        )
        .unwrap();
    }
    state.library.scan().unwrap();
    let results = state.library.search(Default::default(), false).unwrap();
    let second = &results
        .skills
        .iter()
        .find(|s| s.name == "second")
        .unwrap()
        .id;
    let untouched = &results
        .skills
        .iter()
        .find(|s| s.name == "untouched")
        .unwrap()
        .id;
    let listing = client
        .get(format!("{base}/?repository_source=test&repository="))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(listing.contains("name=\"codex_mode\""));
    assert!(listing.contains("自動選択＋コマンド候補"));
    for (mode, automatic, command) in [
        ("command", false, true),
        ("automatic", true, true),
        ("search", false, false),
    ] {
        let response = client
            .post(&bulk_url)
            .header("content-type", "application/x-www-form-urlencoded")
            .body(
                [
                    ("csrf", token.as_str()),
                    ("selected", &format!("{id},{second}")),
                    ("codex_mode", mode),
                    ("repository_source", "test"),
                    ("repository", ""),
                ]
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join("&"),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert!(
            response.headers()["location"]
                .to_str()
                .unwrap()
                .contains("repository_source=test")
        );
        for key in [&id, second] {
            let policy = state.library.codex_policy(key).unwrap();
            assert_eq!((policy.automatic, policy.command), (automatic, command));
        }
        assert_eq!(
            state.library.codex_policy(untouched).unwrap(),
            Default::default()
        );
    }
    for (selected, mode) in [
        (format!("{id},{second}"), "unknown"),
        (String::new(), "command"),
    ] {
        assert_eq!(
            client
                .post(&bulk_url)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(
                    [
                        ("csrf", token.as_str()),
                        ("selected", &selected),
                        ("codex_mode", mode)
                    ]
                    .iter()
                    .map(|(key, value)| format!("{key}={value}"))
                    .collect::<Vec<_>>()
                    .join("&")
                )
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::BAD_REQUEST
        );
    }
    let before = state.library.detail(&id, false).unwrap();
    assert_eq!(
        client
            .post(&bulk_url)
            .header("content-type", "application/x-www-form-urlencoded")
            .body(
                [
                    ("csrf", token.as_str()),
                    ("selected", &format!("{id},unknown")),
                    ("codex_mode", "automatic"),
                    ("published", "false"),
                    ("tags", "rollback")
                ]
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join("&")
            )
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(state.library.codex_policy(&id).unwrap(), Default::default());
    let after = state.library.detail(&id, false).unwrap();
    assert_eq!(before.summary.published, after.summary.published);
    assert_eq!(before.summary.tags, after.summary.tags);
    assert_eq!(
        client
            .post(&bulk_url)
            .header("content-type", "application/x-www-form-urlencoded")
            .body(
                [
                    ("csrf", token.as_str()),
                    ("selected", &format!("{id},{second}")),
                    ("codex_mode", "command"),
                    ("published", "true"),
                    ("tags", "batch")
                ]
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join("&")
            )
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::SEE_OTHER
    );
    for key in [&id, second] {
        let detail = state.library.detail(key, false).unwrap();
        assert!(detail.summary.published);
        assert!(detail.summary.tags.contains(&"batch".to_string()));
        assert!(detail.codex_policy.command);
    }
    // Codexモード未指定の従来の一括操作では設定を維持する。
    assert_eq!(
        client
            .post(&bulk_url)
            .header("content-type", "application/x-www-form-urlencoded")
            .body(
                [
                    ("csrf", token.as_str()),
                    ("selected", id.as_str()),
                    ("tags", "metadata-only")
                ]
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join("&")
            )
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::SEE_OTHER
    );
    assert!(state.library.codex_policy(&id).unwrap().command);
    for enabled in [true, false] {
        assert_eq!(
            client
                .post(&settings_url)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(format!("csrf={token}&enabled={enabled}"))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::SEE_OTHER
        );
        let body = client
            .get(format!("{base}/settings"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(body.contains(&format!("value=\"{enabled}\" selected")));
        assert!(body.contains("生成済み件数:"));
        assert!(body.contains("skilvella-search"));
        let saved = Library::open_for_user(&root.join("data"), &home, None).unwrap();
        assert_eq!(saved.codex_settings().unwrap().enabled, enabled);
        let unknown_file = home.join(".agents/skills/skilvella/unknown.txt");
        if enabled {
            assert_eq!(
                client
                    .post(&bulk_url)
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(format!(
                        "csrf={token}&selected={id},{second}&codex_mode=automatic"
                    ))
                    .send()
                    .await
                    .unwrap()
                    .status(),
                StatusCode::SEE_OTHER
            );
            for name in ["example", "second"] {
                let entry = home.join(".agents/skills/skilvella").join(name);
                assert!(
                    fs::read_to_string(entry.join("agents/openai.yaml"))
                        .unwrap()
                        .contains("allow_implicit_invocation: true")
                );
                assert!(
                    !fs::read_to_string(entry.join("SKILL.md"))
                        .unwrap()
                        .contains("Original body")
                );
            }
            assert!(!home.join(".agents/skills/skilvella/untouched").exists());
            fs::write(&unknown_file, "User-owned file").unwrap();
        } else {
            let settings = saved.codex_settings().unwrap();
            assert!(!settings.last_error.is_empty());
            assert!(settings.exported > 0);
            let warning = "同期が停止しています。保存した設定はCodexへまだ反映されていません。以前の候補や自動選択が残っている可能性があります。";
            assert!(body.contains(warning));
            let detail = client
                .get(&detail_url)
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            assert!(detail.contains(warning));
            assert!(detail.contains("unknown.txt"));
            assert_eq!(
                fs::read_to_string(&unknown_file).unwrap(),
                "User-owned file"
            );
        }
    }
    assert_eq!(
        fs::read_to_string(source.join("example/SKILL.md")).unwrap(),
        "---\nname: example\ndescription: Example skill\n---\nOriginal body\n"
    );
}
