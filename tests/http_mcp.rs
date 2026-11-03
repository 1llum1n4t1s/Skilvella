use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use skilvella::{AppState, library::Library};
use std::{fs, path::PathBuf};

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
fn form(fields: &[(&str, &str)]) -> String {
    fields
        .iter()
        .map(|(k, v)| format!("{}={}", encode(k), encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}
async fn post(client: &Client, url: &str, fields: &[(&str, &str)]) -> reqwest::Response {
    client
        .post(url)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(form(fields))
        .send()
        .await
        .unwrap()
}
async fn rpc(client: &Client, base: &str, id: u64, method: &str, params: Value) -> Value {
    let response = client
        .post(format!("{base}/mcp"))
        .header("accept", "application/json, text/event-stream")
        .header("mcp-protocol-version", "2025-11-25")
        .json(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
        .send()
        .await
        .unwrap();
    let status = response.status();
    let text = response.text().await.unwrap();
    assert!(status.is_success(), "{method}: {status} {text}");
    let value: Value = serde_json::from_str(&text).unwrap_or_else(|_| panic!("{method}: {text}"));
    assert!(value.get("error").is_none(), "{method}: {value}");
    value["result"].clone()
}

#[tokio::test]
async fn browser_management_and_mcp_share_publication_and_live_files() {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".work/test-http")
        .join(uuid::Uuid::new_v4().to_string());
    let source = base.join("skills");
    fs::create_dir_all(source.join("example/references")).unwrap();
    let original = "---\nname: rust-example\ndescription: Build useful Rust tools\n---\n\n# Example\n\nRead references/help.txt.\n";
    fs::write(source.join("example/SKILL.md"), original).unwrap();
    fs::write(
        source.join("example/references/help.txt"),
        "first\nsecond\nthird\n",
    )
    .unwrap();
    let state = AppState::new(Library::open(&base.join("data")).unwrap());
    let token = state.csrf.to_string();
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
    let base_url = format!("http://127.0.0.1:{port}");
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let page = client.get(&base_url).send().await.unwrap();
    assert_eq!(page.status(), StatusCode::OK);
    assert!(page.headers().contains_key("content-security-policy"));
    assert_eq!(
        client
            .get(&base_url)
            .header("host", "attacker.example")
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        client
            .get(&base_url)
            .header("origin", "https://attacker.example")
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(&client, &format!("{base_url}/scan"), &[("csrf", "wrong")])
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    let added = post(
        &client,
        &format!("{base_url}/sources/add"),
        &[
            ("csrf", &token),
            ("alias", "test"),
            ("path", &source.to_string_lossy()),
        ],
    )
    .await;
    assert_eq!(added.status(), StatusCode::SEE_OTHER);
    let name_url = format!("{base_url}/sources/name");
    assert_eq!(
        client.get(&name_url).send().await.unwrap().status(),
        StatusCode::METHOD_NOT_ALLOWED
    );
    for token in ["wrong", ""] {
        assert_eq!(
            post(
                &client,
                &name_url,
                &[
                    ("csrf", token),
                    ("alias", "test"),
                    ("display_name", "blocked")
                ]
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
    }
    let display_name = "仕事 <script>alert(1)</script> & \"🌸\"";
    assert_eq!(
        post(
            &client,
            &name_url,
            &[
                ("csrf", &token),
                ("alias", "test"),
                ("display_name", display_name)
            ]
        )
        .await
        .status(),
        StatusCode::SEE_OTHER
    );
    let skill_id = state
        .library
        .search(Default::default(), false)
        .unwrap()
        .skills[0]
        .id
        .clone();
    for path in [
        "/sources".to_owned(),
        "/?repository_source=test&repository=".into(),
        "/skills/new".into(),
        format!("/skills/{skill_id}"),
    ] {
        let body = client
            .get(format!("{base_url}{path}"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(
            body.contains("仕事 &#60;script&#62;alert(1)&#60;/script&#62; &#38; &#34;🌸&#34;"),
            "{path}: missing escaped display name"
        );
        assert!(!body.contains(display_name));
        if path.starts_with("/?repository_source=") || path == "/skills/new" {
            assert!(body.contains("value=\"test\""));
        }
    }
    assert_eq!(
        post(
            &client,
            &name_url,
            &[
                ("csrf", &token),
                ("alias", "test"),
                ("display_name", "a\nb")
            ]
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        post(
            &client,
            &name_url,
            &[("csrf", &token), ("alias", "test"), ("display_name", "")]
        )
        .await
        .status(),
        StatusCode::SEE_OTHER
    );
    let body = client
        .get(format!("{base_url}/sources"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(body.contains("<h3>test</h3>"));
    let initial = rpc(&client, &base_url, 1, "initialize", json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"integration-test","version":"1"}})).await;
    assert_eq!(initial["serverInfo"]["name"], "skilvella");
    let tools = rpc(&client, &base_url, 2, "tools/list", json!({})).await;
    let names: std::collections::BTreeSet<_> = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        ["load_skill", "read_skill_file", "search_skills"]
            .into_iter()
            .collect()
    );
    for tool in tools["tools"].as_array().unwrap() {
        assert_eq!(tool["annotations"]["readOnlyHint"], true);
    }
    let search = rpc(
        &client,
        &base_url,
        3,
        "tools/call",
        json!({"name":"search_skills","arguments":{"query":"Rust"}}),
    )
    .await;
    assert_eq!(search["structuredContent"]["total"], 0);
    let id = state
        .library
        .search(Default::default(), false)
        .unwrap()
        .skills[0]
        .id
        .clone();
    let published = post(
        &client,
        &format!("{base_url}/skills/{id}/metadata"),
        &[
            ("csrf", &token),
            ("tags", "rust,仕事"),
            ("category", "development"),
            ("published", "true"),
        ],
    )
    .await;
    assert_eq!(published.status(), StatusCode::SEE_OTHER);
    let search = rpc(
        &client,
        &base_url,
        4,
        "tools/call",
        json!({"name":"search_skills","arguments":{"query":"Rust","tag":"仕事"}}),
    )
    .await;
    assert_eq!(search["structuredContent"]["total"], 1);
    let loaded = rpc(
        &client,
        &base_url,
        5,
        "tools/call",
        json!({"name":"load_skill","arguments":{"id":id}}),
    )
    .await;
    assert_eq!(loaded["structuredContent"]["content"], original);
    let file = rpc(&client, &base_url, 6, "tools/call", json!({"name":"read_skill_file","arguments":{"id":id,"path":"references/help.txt","start_line":2,"max_lines":1}})).await;
    assert_eq!(file["structuredContent"]["text"], "second");
    let escaped = rpc(
        &client,
        &base_url,
        7,
        "tools/call",
        json!({"name":"read_skill_file","arguments":{"id":id,"path":"../../data/library.sqlite3"}}),
    )
    .await;
    assert_eq!(escaped["isError"], true);
    let detail = state.library.detail(&id, false).unwrap();
    let edited = original.replace("# Example", "# Edited");
    let saved = post(
        &client,
        &format!("{base_url}/skills/{id}/content"),
        &[
            ("csrf", &token),
            ("revision", &detail.revision),
            ("content", &edited),
        ],
    )
    .await;
    assert_eq!(saved.status(), StatusCode::SEE_OTHER);
    let stale = post(
        &client,
        &format!("{base_url}/skills/{id}/content"),
        &[
            ("csrf", &token),
            ("revision", &detail.revision),
            ("content", "my unsaved draft"),
        ],
    )
    .await;
    assert!(stale.text().await.unwrap().contains("my unsaved draft"));
    assert_eq!(
        fs::read_to_string(source.join("example/SKILL.md")).unwrap(),
        edited
    );
    let current = state.library.detail(&id, false).unwrap();
    let restored = post(
        &client,
        &format!("{base_url}/skills/{id}/restore"),
        &[
            ("csrf", &token),
            ("revision", &current.revision),
            ("history_id", &current.history[0].id.to_string()),
        ],
    )
    .await;
    assert_eq!(restored.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        fs::read_to_string(source.join("example/SKILL.md")).unwrap(),
        original
    );
    let large = "日本語".repeat(140_000);
    assert_eq!(
        post(
            &client,
            &format!("{base_url}/preview"),
            &[("csrf", &token), ("content", &large)]
        )
        .await
        .status(),
        StatusCode::OK
    );
    fs::write(
        source.join("example/SKILL.md"),
        "invalid external replacement",
    )
    .unwrap();
    let invalid = rpc(
        &client,
        &base_url,
        8,
        "tools/call",
        json!({"name":"load_skill","arguments":{"id":id}}),
    )
    .await;
    assert_eq!(invalid["isError"], true);
    let invalid = rpc(
        &client,
        &base_url,
        9,
        "tools/call",
        json!({"name":"read_skill_file","arguments":{"id":id,"path":"references/help.txt"}}),
    )
    .await;
    assert_eq!(invalid["isError"], true);
}

#[tokio::test]
async fn directory_settings_forms_and_icons_work_over_http() {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".work/test-http")
        .join(uuid::Uuid::new_v4().to_string());
    let home = base.join("home");
    let native = home.join(".agents/skills");
    let custom = base.join("custom");
    for (root, name) in [(&native, "autodetected"), (&custom, "custom-directory")] {
        fs::create_dir_all(root.join("example")).unwrap();
        fs::write(
            root.join("example/SKILL.md"),
            format!("---\nname: {name}\ndescription: directory fixture\n---\nInstructions\n"),
        )
        .unwrap();
    }
    let state = AppState::new(Library::open(&base.join("data")).unwrap());
    state.library.initialize_directory(&home, None).unwrap();
    let original = state
        .library
        .search(Default::default(), false)
        .unwrap()
        .skills[0]
        .id
        .clone();
    state
        .library
        .set_metadata(
            &original,
            skilvella::model::SkillMetadata {
                tags: vec!["keep-tag".into()],
                category: String::new(),
                published: true,
            },
        )
        .unwrap();
    let token = state.csrf.to_string();
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
    let url = format!("http://127.0.0.1:{port}");
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let page = client
        .get(format!("{url}/sources"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(page.contains("参照するフォルダー（絶対パス）") && page.contains("現在の場所"));
    assert!(!page.contains("<select name=\"mode\""));
    let expected_path: PathBuf = native.components().collect();
    assert!(page.contains(&format!("value=\"{}\"", expected_path.to_string_lossy())));
    assert!(page.contains("/favicon.ico") && page.contains("/assets/icon-32.png"));
    let icon = client
        .get(format!("{url}/favicon.ico"))
        .send()
        .await
        .unwrap();
    assert_eq!(icon.headers()["content-type"], "image/x-icon");
    assert_eq!(&icon.bytes().await.unwrap()[..4], &[0, 0, 1, 0]);
    let png = client
        .get(format!("{url}/assets/icon-32.png"))
        .send()
        .await
        .unwrap();
    assert_eq!(png.headers()["content-type"], "image/png");
    let bytes = png.bytes().await.unwrap();
    assert_eq!(&bytes[16..24], &[0, 0, 0, 32, 0, 0, 0, 32]);
    assert_eq!(
        post(
            &client,
            &format!("{url}/sources/directory"),
            &[("csrf", "wrong"), ("mode", "disabled")]
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let response = post(
        &client,
        &format!("{url}/sources/directory"),
        &[
            ("csrf", &token),
            ("mode", "custom"),
            ("path", &custom.to_string_lossy()),
        ],
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(state.library.directory_settings().unwrap().mode, "custom");
    let switched = state.library.search(Default::default(), false).unwrap();
    assert_eq!(switched.total, 1);
    assert_ne!(switched.skills[0].id, original);
    assert!(!switched.skills[0].published && switched.skills[0].tags.is_empty());
    let failed = post(
        &client,
        &format!("{url}/sources/directory"),
        &[
            ("csrf", &token),
            ("mode", "custom"),
            ("path", &base.join("missing-folder").to_string_lossy()),
        ],
    )
    .await;
    let failed = failed.text().await.unwrap();
    assert!(failed.contains("以前の設定は保持") && failed.contains("missing-folder"));
    assert_eq!(state.library.directory_settings().unwrap().mode, "custom");
    assert_eq!(
        state
            .library
            .search(Default::default(), false)
            .unwrap()
            .total,
        1
    );
    assert_eq!(
        post(
            &client,
            &format!("{url}/sources/directory"),
            &[("csrf", &token), ("mode", "auto")]
        )
        .await
        .status(),
        StatusCode::SEE_OTHER
    );
    let restored = state.library.detail(&original, true).unwrap();
    assert_eq!(restored.summary.tags, vec!["keep-tag"]);
    assert!(native.join("example/SKILL.md").exists() && custom.join("example/SKILL.md").exists());
    assert_eq!(
        post(
            &client,
            &format!("{url}/sources/directory"),
            &[("csrf", &token), ("mode", "disabled")]
        )
        .await
        .status(),
        StatusCode::SEE_OTHER
    );
    let reopened = Library::open(&base.join("data")).unwrap();
    reopened.initialize_directory(&home, None).unwrap();
    assert_eq!(reopened.directory_settings().unwrap().mode, "disabled");
    assert_eq!(reopened.search(Default::default(), false).unwrap().total, 0);
}
