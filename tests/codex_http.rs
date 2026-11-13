use reqwest::{Client, StatusCode};
use skilvella::{AppState, library::Library};
use std::fs;

mod common;

#[tokio::test]
async fn disconnected_policy_save_remains_observable_until_sync_finishes() {
    let root = common::isolated_root("navigation");
    fs::create_dir_all(root.join("source/one")).unwrap();
    fs::write(
        root.join("source/one/SKILL.md"),
        "---\nname: one\ndescription: Test\n---\nBody\n",
    )
    .unwrap();
    let library = Library::open_for_user(&root.join("data"), &root.join("home"), None).unwrap();
    library.add_source("test", &root.join("source")).unwrap();
    let id = library.search(Default::default(), false).unwrap().skills[0]
        .id
        .clone();
    let state = AppState::new(library);
    let server = common::HttpServer::start(state.clone()).await;
    let operation = uuid::Uuid::new_v4();
    let status_url = format!("{}/settings/policy-operations/{operation}", server.base);
    let client = Client::new();
    let db = rusqlite::Connection::open(root.join("data/library.sqlite3")).unwrap();
    db.execute_batch("BEGIN IMMEDIATE").unwrap();
    let request = common::form_post(&client, format!("{}/skills/{id}/codex", server.base))
        .header("accept", "application/json")
        .body(format!(
            "csrf={}&mode=automatic&operation={operation}",
            state.csrf
        ));
    let pending = tokio::spawn(async move { request.send().await });
    for i in 0..100 {
        let status: serde_json::Value = client
            .get(&status_url)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if status["state"] == "pending" {
            break;
        }
        assert!(i < 99, "Save was not registered");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    pending.abort();
    db.execute_batch("ROLLBACK").unwrap();
    for i in 0..100 {
        let status: serde_json::Value = client
            .get(&status_url)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if status["state"] == "complete" {
            break;
        }
        assert!(i < 99, "Disconnected save did not finish");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(state.library.codex_policy(&id).unwrap().automatic);
    let response = common::form_post(&client, format!("{}/skills/{id}/codex", server.base))
        .body(format!(
            "csrf={}&mode=search&operation={operation}",
            state.csrf
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert!(state.library.codex_policy(&id).unwrap().automatic);
    let cancelled = uuid::Uuid::new_v4();
    let status: serde_json::Value = common::form_post(
        &client,
        format!("{}/settings/policy-operations/{cancelled}", server.base),
    )
    .body(format!("csrf={}", state.csrf))
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_eq!(status["state"], "cancelled");
    let late = common::form_post(&client, format!("{}/skills/{id}/codex", server.base))
        .body(format!(
            "csrf={}&mode=search&operation={cancelled}",
            state.csrf
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(late.status(), StatusCode::CONFLICT);
    assert!(state.library.codex_policy(&id).unwrap().automatic);
}

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
    let server = common::HttpServer::start(state.clone()).await;
    let base = server.base.clone();
    let client = common::no_redirect_client();
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
    assert!(body.contains("使い方"));
    assert!(!state.library.codex_settings().unwrap().enabled);
    assert!(body.contains("設定画面で選んだ同期先へ、同期に成功した場合に適用されます。"));
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
                common::form_post(&client, url)
                    .body(format!("csrf={bad_token}&{field}={value}"))
                    .send()
                    .await
                    .unwrap()
                    .status(),
                StatusCode::FORBIDDEN
            );
        }
        assert_eq!(
            common::form_post(&client, url)
                .header("origin", "https://attacker.example")
                .body(format!("csrf={token}&{field}={value}"))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        common::form_post(&client, &policy_url)
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
            common::form_post(&client, &policy_url)
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
            common::form_post(&client, url)
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
    let row = |body: &str, skill_id: &str| {
        let start = body.find(&format!("id=\"skill-{skill_id}\"")).unwrap();
        let end = body[start..].find("</tr>").unwrap() + start;
        body[start..end].to_owned()
    };
    assert!(row(&listing, &id).contains("value=\"command\" checked"));
    assert!(row(&listing, second).contains("value=\"search\" checked"));
    let inline_url = format!(
        "{base}/skills/{second}/codex?repository_source=test&repository=&q=two%20words&tag=a%2Bb&offset=50"
    );
    let response = common::form_post(&client, &inline_url)
        .header("accept", "application/json")
        .body(format!("csrf={token}&mode=automatic"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let sync: serde_json::Value = response.json().await.unwrap();
    assert_eq!(sync["enabled"], false);
    assert!(state.library.codex_policy(second).unwrap().automatic);
    assert!(state.library.codex_policy(&id).unwrap().command);
    assert_eq!(
        state.library.codex_policy(untouched).unwrap(),
        Default::default()
    );
    let listing = client
        .get(format!("{base}/?repository_source=test&repository="))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(row(&listing, second).contains("value=\"automatic\" checked"));
    assert_eq!(
        common::form_post(&client, &inline_url)
            .header("accept", "application/json")
            .body("csrf=bad&mode=search")
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert!(state.library.codex_policy(second).unwrap().automatic);
    let fallback = common::form_post(&client, &inline_url)
        .body(format!("csrf={token}&mode=search"))
        .send()
        .await
        .unwrap();
    assert_eq!(fallback.status(), StatusCode::SEE_OTHER);
    let location = fallback.headers()["location"].to_str().unwrap();
    assert!(location.contains("q=two%20words"));
    assert!(location.contains("tag=a%2Bb"));
    assert!(location.contains("offset=50"));
    assert!(location.ends_with(&format!("#skill-{second}")));
    let db = rusqlite::Connection::open(root.join("data/library.sqlite3")).unwrap();
    db.execute(
        "INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![format!("codex_policy:{untouched}"), "{broken"],
    ).unwrap();
    let listing = client
        .get(format!("{base}/?repository_source=test&repository="))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let unknown = row(&listing, untouched);
    assert!(unknown.contains("不明な設定です。使い方を選んでください。"));
    // 同じ行には公開状態のラジオもあるため、使い方のフォームだけで未選択を確認する。
    let mode_form = unknown
        .split("class=\"codex-policy-form\"")
        .nth(1)
        .and_then(|rest| rest.split("</form>").next())
        .unwrap();
    assert!(!mode_form.contains(" checked"));
    assert_eq!(
        common::form_post(&client, format!("{base}/skills/{untouched}/codex"))
            .header("accept", "application/json")
            .body(format!("csrf={token}&mode=search"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        state.library.codex_policy(untouched).unwrap(),
        Default::default()
    );
    drop(db);
    for (mode, automatic, command) in [
        ("command", false, true),
        ("automatic", true, true),
        ("search", false, false),
    ] {
        let response = common::form_post(&client, &bulk_url)
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
    for (selected, published, mode, message) in [
        (
            format!("{id},{second}"),
            "",
            "unknown",
            "使い方を選択してください。",
        ),
        (
            String::new(),
            "invalid",
            "unknown",
            "スキルを1件以上選んでください。",
        ),
        (
            format!("{id},{second}"),
            "invalid",
            "unknown",
            "公開状態が不正です。",
        ),
        (
            format!("{id},{second}"),
            "",
            "",
            "変更する公開状態・追加タグ・使い方のいずれかを指定してください。",
        ),
    ] {
        let response = common::form_post(&client, &bulk_url)
            .body(
                [
                    ("csrf", token.as_str()),
                    ("selected", &selected),
                    ("published", published),
                    ("codex_mode", mode),
                ]
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join("&"),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(response.text().await.unwrap().contains(message));
    }
    let before = state.library.detail(&id, false).unwrap();
    assert_eq!(
        common::form_post(&client, &bulk_url)
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
        common::form_post(&client, &bulk_url)
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
        common::form_post(&client, &bulk_url)
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
            common::form_post(&client, &settings_url)
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
        // 旧形式（enabledだけ）のPOSTはCodexのチェック状態だけを切り替える。
        assert_eq!(
            body.contains("name=\"codex\" value=\"true\" checked"),
            enabled
        );
        assert!(!body.contains("name=\"claude\" value=\"true\" checked"));
        assert!(body.contains("生成済み件数:"));
        assert!(body.contains("skilvella-search"));
        let saved = Library::open_for_user(&root.join("data"), &home, None).unwrap();
        assert_eq!(saved.codex_settings().unwrap().enabled, enabled);
        let unknown_file = home.join(".agents/skills/skilvella/unknown.txt");
        if enabled {
            assert_eq!(
                common::form_post(&client, &bulk_url)
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
            let response = common::form_post(&client, &policy_url)
                .header("accept", "application/json")
                .body(format!("csrf={token}&mode=command"))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let sync: serde_json::Value = response.json().await.unwrap();
            assert_eq!(sync["enabled"], true);
            assert_eq!(sync["last_error"], "");
            let config = home.join(".agents/skills/skilvella/example/agents/openai.yaml");
            assert!(
                fs::read_to_string(config)
                    .unwrap()
                    .contains("allow_implicit_invocation: false")
            );
            fs::write(&unknown_file, "User-owned file").unwrap();
            let response = common::form_post(&client, &policy_url)
                .header("accept", "application/json")
                .body(format!("csrf={token}&mode=automatic"))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let sync: serde_json::Value = response.json().await.unwrap();
            assert_eq!(sync["enabled"], true);
            // 同期先の失敗は、Codexも含めて全クライアント共通の「表示名: エラー」形式で返す。
            let last_error = sync["last_error"].as_str().unwrap();
            assert!(last_error.starts_with("Codex: "), "{last_error}");
        } else {
            let settings = saved.codex_settings().unwrap();
            assert!(!settings.last_error.is_empty());
            assert!(settings.exported > 0);
            let response = common::form_post(&client, &policy_url)
                .header("accept", "application/json")
                .body(format!("csrf={token}&mode=search"))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let sync: serde_json::Value = response.json().await.unwrap();
            assert_eq!(sync["enabled"], false);
            assert!(!sync["last_error"].as_str().unwrap().is_empty());
            let warning = "同期が停止しています。保存した設定は同期先へまだ反映されていません。以前の候補や自動選択が残っている可能性があります。";
            // 設定画面は同期先ごとの警告を表示し、一覧・詳細画面は従来の警告を表示する。
            assert!(body.contains(
                "同期が停止しています。保存した設定はこの同期先へまだ反映されていません。以前の候補や自動選択が残っている可能性があります。"
            ));
            assert!(body.contains("同期エラー:"));
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

/// 設定画面の同期先チェックボックス（複数選択）が、CSRF境界を保ったまま各クライアントへ保存されること。
#[tokio::test]
async fn sync_target_form_selects_multiple_clients() {
    use skilvella::model::SkillClient;
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".work/test-http")
        .join(uuid::Uuid::new_v4().to_string());
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    let library = Library::open_for_user(&root.join("data"), &home, None).unwrap();
    let state = AppState::new(library);
    let server = common::HttpServer::start(state.clone()).await;
    let base = server.base.clone();
    let client = common::no_redirect_client();
    let token = state.csrf.to_string();
    let url = format!("{base}/settings/codex");
    let post = |body: String| common::form_post(&client, &url).body(body).send();
    // CSRFなし・同期先の指定なしは拒否し、設定を変えない。
    assert_eq!(
        post("csrf=wrong&targets=true&claude=true".into())
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(format!("csrf={token}")).await.unwrap().status(),
        StatusCode::BAD_REQUEST
    );
    assert!(!state.library.sync_summary().unwrap().enabled);
    assert_eq!(
        post(format!("csrf={token}&targets=true&claude=true&grok=true"))
            .await
            .unwrap()
            .status(),
        StatusCode::SEE_OTHER
    );
    let enabled = |client| state.library.client_settings(client).unwrap().enabled;
    assert!(!enabled(SkillClient::Codex));
    assert!(enabled(SkillClient::Claude));
    assert!(enabled(SkillClient::Grok));
    assert!(
        home.join(".grok/skills/skilvella/skilvella-search/SKILL.md")
            .is_file()
    );
    assert!(
        home.join(".claude/skills/skilvella-search/SKILL.md")
            .is_file()
    );
    assert!(!home.join(".agents/skills/skilvella").exists());
    let body = client
        .get(format!("{base}/settings"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(body.contains("name=\"claude\" value=\"true\" checked"));
    assert!(body.contains("name=\"grok\" value=\"true\" checked"));
    assert!(!body.contains("name=\"codex\" value=\"true\" checked"));
    assert!(body.contains("リンク先:"));
    // 旧形式のPOSTはCodexだけを切り替え、Claude Code・Grokの選択を保つ。
    assert_eq!(
        post(format!("csrf={token}&enabled=true"))
            .await
            .unwrap()
            .status(),
        StatusCode::SEE_OTHER
    );
    assert!(enabled(SkillClient::Codex));
    assert!(enabled(SkillClient::Claude));
    // 全て外すと、各クライアントの入口とリンクを探索先から外す。
    assert_eq!(
        post(format!("csrf={token}&targets=true"))
            .await
            .unwrap()
            .status(),
        StatusCode::SEE_OTHER
    );
    assert!(!state.library.sync_summary().unwrap().enabled);
    assert!(state.library.sync_summary().unwrap().last_error.is_empty());
    assert!(!home.join(".claude/skills/skilvella-search").exists());
    assert!(
        fs::symlink_metadata(home.join(".claude/skills/skilvella-search")).is_err(),
        "link itself must be removed"
    );
    assert!(!home.join(".grok/skills/skilvella").exists());
}

/// 一覧・詳細に出す呼び出しコマンドが、実際に生成した入口のフォルダー名と一致することを確かめる。
/// 名前をそのまま使える場合・使えない名前（`skill-<id>`）・同名の重複（`-<id>`）を網羅し、
/// 「検索のみ」と非公開のスキルにはコマンドを出さない（理由の案内を出す）ことも検証する。
#[tokio::test]
async fn skill_pages_show_invocation_commands_matching_exported_entries() {
    use skilvella::model::SkillClient;
    let root = common::isolated_root("commands");
    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    let skills = [
        ("alpha", "alpha"),
        ("bad", "Bad Name"),
        ("dup1", "dup"),
        ("dup2", "dup"),
        ("quiet", "quiet"),
        ("hidden", "hidden"),
    ];
    for (dir, name) in skills {
        fs::create_dir_all(root.join("source").join(dir)).unwrap();
        fs::write(
            root.join("source").join(dir).join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Test {dir}\n---\nBody\n"),
        )
        .unwrap();
    }
    let library = Library::open_for_user(&root.join("data"), &home, None).unwrap();
    library.add_source("test", &root.join("source")).unwrap();
    let found = library.search(Default::default(), false).unwrap().skills;
    let id_of = |dir: &str| {
        found
            .iter()
            .find(|s| s.relative_path.starts_with(dir))
            .unwrap()
            .id
            .clone()
    };
    for (dir, _) in skills {
        let id = id_of(dir);
        library.set_published(&id, dir != "hidden").unwrap();
        // 「検索のみ」は既定値なので、quiet以外をコマンド候補にする。
        if dir != "quiet" {
            library.set_codex_policy(&id, false, true).unwrap();
        }
    }
    library
        .set_clients_enabled(&[(SkillClient::Codex, true), (SkillClient::Claude, true)])
        .unwrap();
    assert!(library.sync_summary().unwrap().last_error.is_empty());
    let exported = |name: &str| {
        home.join(".agents/skills/skilvella")
            .join(name)
            .join("SKILL.md")
            .is_file()
    };
    let suffix = |dir: &str| id_of(dir)[..8].to_owned();
    let expected = [
        ("alpha", "alpha".to_owned()),
        ("bad", format!("skill-{}", suffix("bad"))),
        ("dup1", format!("dup-{}", suffix("dup1"))),
        ("dup2", format!("dup-{}", suffix("dup2"))),
    ];
    for (_, name) in &expected {
        assert!(exported(name), "entry {name} was not exported");
    }
    assert!(!exported("quiet") && !exported("hidden"));

    let state = AppState::new(library);
    let server = common::HttpServer::start(state.clone()).await;
    let client = Client::new();
    let get = |path: String| {
        let client = client.clone();
        let url = format!("{}{path}", server.base);
        async move { client.get(url).send().await.unwrap().text().await.unwrap() }
    };
    let list = get("/".into()).await;
    for (dir, name) in &expected {
        // Codexは `$name`、Claude Codeは `/name`。一覧と詳細の両方に出す。
        assert!(list.contains(&format!(">${name}</code>")), "{dir}: {list}");
        assert!(list.contains(&format!(">/{name}</code>")), "{dir}");
        let detail = get(format!("/skills/{}", id_of(dir))).await;
        assert!(detail.contains(&format!(">${name}</code>")), "{dir}");
        assert!(detail.contains(&format!(">/{name}</code>")), "{dir}");
    }
    // 検索のみの行は名前を持つが隠し、非公開の行には名前自体を出さない。
    let quiet_row = list
        .split("id=\"skill-")
        .find(|row| row.starts_with(&id_of("quiet")))
        .unwrap();
    assert!(quiet_row.contains("data-skill-commands hidden"));
    assert!(!list.contains(">$hidden</code>") && !list.contains(">/hidden</code>"));
    let quiet = get(format!("/skills/{}", id_of("quiet"))).await;
    assert!(!quiet.contains("class=\"command\""));
    assert!(quiet.contains("「検索のみ」のため、コマンドはありません。"));
    let hidden = get(format!("/skills/{}", id_of("hidden"))).await;
    assert!(!hidden.contains("class=\"command\""));
    assert!(hidden.contains("公開中で読み取れるスキルだけがコマンドになります。"));

    // 同期先をすべて外すと、コマンドは出さず設定への案内を出す。
    state
        .library
        .set_clients_enabled(&[(SkillClient::Codex, false), (SkillClient::Claude, false)])
        .unwrap();
    let alpha = get(format!("/skills/{}", id_of("alpha"))).await;
    assert!(!alpha.contains("class=\"command\""));
    assert!(alpha.contains("同期先が選ばれていないため、コマンドはありません。"));
    assert!(!get("/".into()).await.contains("data-skill-commands"));
}
