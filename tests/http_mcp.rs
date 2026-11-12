use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use skilvella::{AppState, library::Library};
use std::{fs, path::PathBuf};

mod common;

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
    for index in 0..110 {
        fs::write(
            source.join(format!("example/references/{index:03}.txt")),
            "x",
        )
        .unwrap();
    }
    let state = AppState::new(Library::open(&base.join("data")).unwrap());
    let token = state.csrf.to_string();
    let server = common::HttpServer::start(state.clone()).await;
    let base_url = server.base.clone();
    let client = common::no_redirect_client();
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
        common::post_form(&client, &format!("{base_url}/scan"), &[("csrf", "wrong")])
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    let added = common::post_form(
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
            common::post_form(
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
        common::post_form(
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
    // 旧「フォルダー」画面のURLは、条件を保ったまま統合後のライブラリへ転送する。
    let moved = client
        .get(format!("{base_url}/sources?message=source-renamed"))
        .send()
        .await
        .unwrap();
    assert_eq!(moved.status(), StatusCode::SEE_OTHER);
    assert_eq!(moved.headers()["location"], "/?message=source-renamed");
    for path in [
        "/".to_owned(),
        "/?source=test".into(),
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
        common::post_form(
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
        common::post_form(
            &client,
            &name_url,
            &[("csrf", &token), ("alias", "test"), ("display_name", "")]
        )
        .await
        .status(),
        StatusCode::SEE_OTHER
    );
    // 表示名を空にすると識別子の表示に戻る。
    let body = client
        .get(format!("{base_url}/?source=test"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(body.contains("<span class=\"folder-label\">test</span>"));
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
    let published = common::post_form(
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
    let mut short_files: Vec<String> = loaded["structuredContent"]["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|path| path.as_str().unwrap().to_owned())
        .collect();
    let mut next = loaded["structuredContent"]["next"].clone();
    assert!(!next.is_null());
    while !next.is_null() {
        let response = rpc(
            &client,
            &base_url,
            71,
            "tools/call",
            json!({"name":"load_skill","arguments":next}),
        )
        .await;
        let data = &response["structuredContent"];
        assert_eq!(data["content"], "");
        short_files.extend(
            data["files"]
                .as_array()
                .unwrap()
                .iter()
                .map(|path| path.as_str().unwrap().to_owned()),
        );
        next = data["next"].clone();
    }
    assert_eq!(short_files, state.library.detail(&id, true).unwrap().files);

    let file = rpc(&client, &base_url, 6, "tools/call", json!({"name":"read_skill_file","arguments":{"id":id,"path":"references/help.txt","start_line":2,"max_lines":1}})).await;
    assert_eq!(file["structuredContent"]["text"], "second");
    let large_content = format!(
        "---\nname: rust-example\ndescription: Build useful Rust tools\n---\n{}",
        "日本語".repeat(4000)
    );
    let before = state.library.detail(&id, false).unwrap();
    state
        .library
        .save_content(&id, &before.revision, &large_content)
        .unwrap();
    let mut args = json!({"id":id});
    let mut collected = String::new();
    let mut files = Vec::new();
    let mut first = Value::Null;
    let mut saw_files_finish_before_content = false;
    loop {
        let response = rpc(
            &client,
            &base_url,
            60,
            "tools/call",
            json!({"name":"load_skill","arguments":args}),
        )
        .await;
        let data = &response["structuredContent"];
        collected.push_str(data["content"].as_str().unwrap());
        let file_page = data["files"].as_array().unwrap();
        assert!(file_page.len() <= 100);
        assert!(
            file_page
                .iter()
                .map(|path| path.as_str().unwrap().len())
                .sum::<usize>()
                <= 8192
        );
        files.extend(
            file_page
                .iter()
                .map(|path| path.as_str().unwrap().to_owned()),
        );
        if first.is_null() {
            first = data.clone();
        }
        if data["next"].is_null() {
            break;
        }
        args = data["next"].clone();
        if args["files_offset"].as_u64().unwrap() as usize
            == state.library.detail(&id, true).unwrap().files.len()
            && (args["content_offset"].as_u64().unwrap() as usize) < large_content.len()
        {
            saw_files_finish_before_content = true;
        }
    }
    assert_eq!(collected, large_content);
    assert!(saw_files_finish_before_content);
    assert_eq!(files, state.library.detail(&id, true).unwrap().files);
    let invalid_offset = rpc(&client, &base_url, 61, "tools/call", json!({"name":"load_skill","arguments":{"id":id,"content_offset":1,"expected_revision":first["revision"]}})).await;
    // Offset 1 is valid ASCII; use the middle of the first Japanese character instead.
    assert_ne!(invalid_offset["isError"], true);
    let bad = large_content.find('日').unwrap() + 1;
    let invalid_offset = rpc(&client, &base_url, 62, "tools/call", json!({"name":"load_skill","arguments":{"id":id,"content_offset":bad,"expected_revision":first["revision"]}})).await;
    assert_eq!(invalid_offset["isError"], true);
    fs::write(source.join("example/references/added.txt"), "added").unwrap();
    let stale_list = rpc(
        &client,
        &base_url,
        69,
        "tools/call",
        json!({"name":"load_skill","arguments":first["next"]}),
    )
    .await;
    assert_eq!(stale_list["isError"], true);
    assert!(
        stale_list["structuredContent"]["error"]
            .as_str()
            .unwrap()
            .contains("Conflict: content changed")
    );
    let fresh = rpc(
        &client,
        &base_url,
        70,
        "tools/call",
        json!({"name":"load_skill","arguments":{"id":id}}),
    )
    .await;
    let current = state.library.detail(&id, false).unwrap();
    let modified_content = large_content.replace("日本語", "語本日");
    state
        .library
        .save_content(&id, &current.revision, &modified_content)
        .unwrap();
    let stale_page = rpc(
        &client,
        &base_url,
        63,
        "tools/call",
        json!({"name":"load_skill","arguments":fresh["structuredContent"]["next"]}),
    )
    .await;
    assert_eq!(stale_page["isError"], true);
    assert!(
        stale_page["structuredContent"]["error"]
            .as_str()
            .unwrap()
            .contains("Conflict: content changed")
    );
    let current = state.library.detail(&id, false).unwrap();
    state
        .library
        .save_content(&id, &current.revision, original)
        .unwrap();

    let long_line = "日本語".repeat(4000);
    fs::write(
        source.join("example/references/help.txt"),
        format!("first\r\n{long_line}\r\nlast\r\n"),
    )
    .unwrap();
    let mut offset = 0;
    let mut reconstructed = String::new();
    let mut revision = Value::Null;
    loop {
        let response = rpc(&client, &base_url, 64, "tools/call", json!({"name":"read_skill_file","arguments":{"id":id,"path":"references/help.txt","start_line":2,"max_lines":1,"text_offset":offset,"expected_revision":revision}})).await;
        let data = &response["structuredContent"];
        reconstructed.push_str(data["text"].as_str().unwrap());
        revision = data["revision"].clone();
        if data["next_text_offset"].is_null() {
            assert_eq!(data["next_start_line"], 3);
            break;
        }
        offset = data["next_text_offset"].as_u64().unwrap();
    }
    assert_eq!(reconstructed, long_line);
    assert_eq!(revision, {
        use sha2::{Digest, Sha256};
        let body_revision = state.library.detail(&id, true).unwrap().revision;
        let full_text = format!("first\r\n{long_line}\r\nlast\r\n");
        Sha256::digest(format!("{body_revision}\0{full_text}").as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    });
    let last = rpc(&client, &base_url, 67, "tools/call", json!({"name":"read_skill_file","arguments":{"id":id,"path":"references/help.txt","start_line":3,"max_lines":1,"expected_revision":revision}})).await;
    assert_eq!(
        format!(
            "{}\n{}",
            reconstructed,
            last["structuredContent"]["text"].as_str().unwrap()
        ),
        format!("{long_line}\nlast")
    );
    let bad_file = rpc(&client, &base_url, 65, "tools/call", json!({"name":"read_skill_file","arguments":{"id":id,"path":"references/help.txt","start_line":2,"max_lines":1,"text_offset":1,"expected_revision":revision}})).await;
    assert_eq!(bad_file["isError"], true);
    fs::write(
        source.join("example/references/help.txt"),
        "x".repeat(1024 * 1024 + 1),
    )
    .unwrap();
    let oversized = rpc(
        &client,
        &base_url,
        68,
        "tools/call",
        json!({"name":"read_skill_file","arguments":{"id":id,"path":"references/help.txt"}}),
    )
    .await;
    assert_eq!(oversized["isError"], true);
    fs::write(
        source.join("example/references/help.txt"),
        format!("first\r\n{}\r\nlast\r\n", "語本日".repeat(4000)),
    )
    .unwrap();
    let changed_file = rpc(&client, &base_url, 66, "tools/call", json!({"name":"read_skill_file","arguments":{"id":id,"path":"references/help.txt","start_line":2,"max_lines":1,"text_offset":offset,"expected_revision":revision}})).await;
    assert_eq!(changed_file["isError"], true);
    assert!(
        changed_file["structuredContent"]["error"]
            .as_str()
            .unwrap()
            .contains("Conflict: content changed")
    );
    fs::write(
        source.join("example/references/help.txt"),
        "first\nsecond\nthird\n",
    )
    .unwrap();
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
    let saved = common::post_form(
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
    let stale = common::post_form(
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
    let restored = common::post_form(
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
        common::post_form(
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
    let server = common::HttpServer::start(state.clone()).await;
    let url = server.base.clone();
    let client = common::no_redirect_client();
    // 既定フォルダーの場所の変更は、ライブラリのフォルダー表示から行う。
    let default_alias = state.library.directory_settings().unwrap().source_alias;
    let page = client
        .get(format!("{url}/?source={default_alias}"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        page.contains("参照するフォルダー（絶対パス）")
            && page.contains("action=\"/sources/directory\"")
            && page.contains("data-folder-browse")
    );
    assert!(!page.contains("<select name=\"mode\""));
    assert!(!page.contains("/sources/migrate") && !page.contains("name=\"destination\""));
    assert_eq!(
        common::post_form(
            &client,
            &format!("{url}/sources/migrate"),
            &[("csrf", &token)]
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        common::post_form(
            &client,
            &format!("{url}/sources/add"),
            &[
                ("csrf", &token),
                ("alias", "obsolete-move"),
                ("path", &native.to_string_lossy()),
                ("action", "move")
            ]
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    assert!(native.join("example/SKILL.md").is_file());
    // フォルダー選択ダイアログもCSRF検査を通す（正しいトークンでは実際に開くため送らない）。
    for token in ["wrong", ""] {
        assert_eq!(
            common::post_form(&client, &format!("{url}/dialog/folder"), &[("csrf", token)])
                .await
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    // 識別子を入力しない追加で既に管理中のフォルダーを選ぶと、表示名を示して案内する。
    let duplicate = common::post_form(
        &client,
        &format!("{url}/sources/add"),
        &[("csrf", &token), ("path", &native.to_string_lossy())],
    )
    .await
    .text()
    .await
    .unwrap();
    assert!(duplicate.contains("として管理中です") && duplicate.contains("notice warning"));
    let empty = common::post_form(
        &client,
        &format!("{url}/sources/add"),
        &[("csrf", &token), ("path", "  ")],
    )
    .await
    .text()
    .await
    .unwrap();
    assert!(empty.contains("フォルダーを選んでください。"));

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
        common::post_form(
            &client,
            &format!("{url}/sources/directory"),
            &[("csrf", "wrong"), ("mode", "disabled")]
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let response = common::post_form(
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
    let failed = common::post_form(
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
        common::post_form(
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
        common::post_form(
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

/// 一覧の公開・非公開ラジオ: CSRF・入力値の検査、JSON応答、タグの保持、
/// 不正なファイルの公開拒否、JavaScriptなしの送信の戻り先を確認する。
#[tokio::test]
async fn list_publication_radio_saves_only_the_published_flag() {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".work/test-http")
        .join(uuid::Uuid::new_v4().to_string());
    let source = base.join("skills");
    fs::create_dir_all(source.join("good")).unwrap();
    fs::create_dir_all(source.join("broken")).unwrap();
    fs::write(
        source.join("good/SKILL.md"),
        "---\nname: good-skill\ndescription: Valid skill\n---\nbody\n",
    )
    .unwrap();
    fs::write(source.join("broken/SKILL.md"), "no frontmatter\n").unwrap();
    let library = Library::open(&base.join("data")).unwrap();
    library.add_source("test", &source).unwrap();
    let skills = library.search(Default::default(), false).unwrap().skills;
    let good = skills
        .iter()
        .find(|s| s.name == "good-skill")
        .unwrap()
        .id
        .clone();
    let broken = skills
        .iter()
        .find(|s| s.error.is_some())
        .unwrap()
        .id
        .clone();
    library
        .set_metadata(
            &good,
            skilvella::model::SkillMetadata {
                tags: vec!["keep-tag".into()],
                category: "kept".into(),
                published: false,
            },
        )
        .unwrap();
    let state = AppState::new(library);
    let token = state.csrf.to_string();
    let server = common::HttpServer::start(state.clone()).await;
    let client = common::no_redirect_client();
    let url = |id: &str| format!("{}/skills/{id}/published", server.base);
    let post_json = |id: String, fields: Vec<(&'static str, String)>| {
        let client = client.clone();
        let target = url(&id);
        async move {
            let pairs: Vec<(&str, &str)> = fields.iter().map(|(k, v)| (*k, v.as_str())).collect();
            common::form_post(&client, &target)
                .header("accept", "application/json")
                .body(common::encode_form(&pairs))
                .send()
                .await
                .unwrap()
        }
    };

    let page = client
        .get(&server.base)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(page.contains(&format!("action=\"/skills/{good}/published?")));
    assert!(page.contains("name=\"published\" value=\"false\" checked"));

    for csrf in ["wrong", ""] {
        let response = post_json(
            good.clone(),
            vec![("csrf", csrf.into()), ("published", "true".into())],
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    let invalid = post_json(
        good.clone(),
        vec![("csrf", token.clone()), ("published", "maybe".into())],
    )
    .await;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    assert!(invalid.json::<Value>().await.unwrap()["error"].is_string());

    // 公開に切り替えても、タグとカテゴリは変わらない。
    let published = post_json(
        good.clone(),
        vec![("csrf", token.clone()), ("published", "true".into())],
    )
    .await;
    assert_eq!(published.status(), StatusCode::OK);
    assert_eq!(published.json::<Value>().await.unwrap()["published"], true);
    let detail = state.library.detail(&good, false).unwrap();
    assert!(detail.summary.published);
    assert_eq!(detail.summary.tags, vec!["keep-tag"]);
    assert_eq!(detail.summary.category, "kept");

    // 読み取りエラーのスキルは公開せず、理由をJSONで返す。
    let rejected = post_json(
        broken.clone(),
        vec![("csrf", token.clone()), ("published", "true".into())],
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
    assert!(rejected.json::<Value>().await.unwrap()["error"].is_string());
    assert!(
        !state
            .library
            .detail(&broken, false)
            .unwrap()
            .summary
            .published
    );

    // JavaScriptなしの送信は、元の一覧の同じ行へ戻す。
    let fallback = common::post_form(
        &client,
        &format!(
            "{}?q=&source=&tag=&category=&status=present&offset=0",
            url(&good)
        ),
        &[("csrf", &token), ("published", "false")],
    )
    .await;
    assert_eq!(fallback.status(), StatusCode::SEE_OTHER);
    let location = fallback.headers()["location"].to_str().unwrap().to_owned();
    assert!(location.starts_with("/?q=") && location.ends_with(&format!("#skill-{good}")));
    assert!(
        !state
            .library
            .detail(&good, false)
            .unwrap()
            .summary
            .published
    );
}
