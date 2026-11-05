use reqwest::Client;
use skilvella::{
    AppState,
    library::Library,
    model::{SearchQuery, SkillMetadata},
};
use std::{fs, path::PathBuf};

mod common;

#[tokio::test]
async fn scan_errors_link_to_global_list_editor_and_disappear_after_fix() {
    let root = std::env::var_os("SKILVELLA_MIGRATION_TEST_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(format!("error-flow-{}", uuid::Uuid::new_v4()));
    let source = root.join("source");
    fs::create_dir_all(source.join("broken")).unwrap();
    fs::write(source.join("broken/SKILL.md"), "---\nname: [\n---\nBody").unwrap();
    let library = Library::open_for_user(&root.join("data"), &root.join("home"), None).unwrap();
    library.add_source("test", &source).unwrap();
    let broken = library
        .search(
            SearchQuery {
                status: "error".into(),
                ..Default::default()
            },
            false,
        )
        .unwrap()
        .skills
        .remove(0);
    let revision = library.detail(&broken.id, false).unwrap().revision;
    let state = AppState::new(library);
    let csrf = state.csrf.to_string();
    let server = common::HttpServer::start(state).await;
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let scan = client
        .post(format!("{}/scan", server.base))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(format!("csrf={csrf}"))
        .send()
        .await
        .unwrap();
    let landing = client
        .get(format!(
            "{}{}",
            server.base,
            scan.headers()["location"].to_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(landing.contains("エラー1件"));
    assert!(landing.contains("href=\"/?status=error\""));
    let errors = client
        .get(format!("{}/?status=error", server.base))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(errors.contains(&format!("href=\"/skills/{}\"", broken.id)));
    assert!(errors.contains("broken/SKILL.md"));
    assert!(errors.contains("構文の誤りは本文を修正して保存してください。"));
    assert!(!errors.contains("id=\"repository-bulk-form\""));
    let detail = client
        .get(format!("{}/skills/{}", server.base, broken.id))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(detail.contains("読み取りエラー:"));
    assert!(detail.contains("id=\"content\""));
    let saved = client
        .post(format!("{}/skills/{}/content", server.base, broken.id))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(format!("csrf={csrf}&revision={revision}&content=---%0Aname%3A%20repaired%0Adescription%3A%20repaired%20skill%0A---%0ABody"))
        .send()
        .await
        .unwrap();
    assert_eq!(saved.status(), reqwest::StatusCode::SEE_OTHER);
    let home = client
        .get(&server.base)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(!home.contains("読み取りエラーを確認・修正"));
    let errors = client
        .get(format!("{}/?status=error", server.base))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(errors.contains("該当するスキルはありません"));
}

#[tokio::test]
async fn forget_missing_checks_paths_and_deletes_filtered_records_across_pages() {
    let root = std::env::var_os("SKILVELLA_MIGRATION_TEST_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(format!("forget-missing-{}", uuid::Uuid::new_v4()));
    let source = root.join("source");
    for i in 0..101 {
        let dir = source.join(format!("repo/old-{i:03}"));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: old-{i:03}\ndescription: old\n---\nBody"),
        )
        .unwrap();
    }
    fs::create_dir_all(source.join("repo/.git")).unwrap();
    fs::create_dir_all(source.join("keep")).unwrap();
    fs::write(
        source.join("keep/SKILL.md"),
        "---\nname: keep\ndescription: keep\n---\nBody",
    )
    .unwrap();
    let library = Library::open_for_user(&root.join("data"), &root.join("home"), None).unwrap();
    library.add_source("test", &source).unwrap();
    let old_id = library
        .search(
            SearchQuery {
                q: "old-000".into(),
                ..Default::default()
            },
            false,
        )
        .unwrap()
        .skills[0]
        .id
        .clone();
    library.set_codex_policy(&old_id, false, true).unwrap();
    let db = rusqlite::Connection::open(root.join("data/library.sqlite3")).unwrap();
    db.execute(
        "INSERT INTO history(skill_id,content,reason) VALUES(?1,'before','test')",
        [&old_id],
    )
    .unwrap();
    fs::rename(source.join("repo"), root.join("retired")).unwrap();
    fs::create_dir_all(source.join("repo/.git")).unwrap();
    library.scan().unwrap();
    let state = AppState::new(library.clone());
    let token = state.csrf.to_string();
    let server = common::HttpServer::start(state).await;
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let post = |csrf: &str, count: usize| {
        client.post(format!("{}/missing/forget", server.base))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(format!("csrf={csrf}&expected={count}&repository_source=test&repository=repo&q=old&tag=&category="))
    };
    let page = client
        .get(format!("{}/?status=missing", server.base))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(page.contains("action=\"/missing/forget\""));
    assert!(page.contains("name=\"expected\" value=\"101\""));
    assert_eq!(post("invalid", 101).send().await.unwrap().status(), 403);
    assert!(
        !post(&token, 100)
            .send()
            .await
            .unwrap()
            .status()
            .is_success()
    );
    // 索引が不在でもファイルが復活していたら、全件を保持する。
    fs::create_dir_all(source.join("repo/old-000")).unwrap();
    fs::rename(
        root.join("retired/old-000/SKILL.md"),
        source.join("repo/old-000/SKILL.md"),
    )
    .unwrap();
    assert!(
        !post(&token, 101)
            .send()
            .await
            .unwrap()
            .status()
            .is_redirection()
    );
    fs::rename(
        source.join("repo/old-000/SKILL.md"),
        root.join("restored.md"),
    )
    .unwrap();
    fs::rename(&source, root.join("offline")).unwrap();
    assert!(
        !post(&token, 101)
            .send()
            .await
            .unwrap()
            .status()
            .is_redirection()
    );
    fs::rename(root.join("offline"), &source).unwrap();
    assert_eq!(
        library
            .search(
                SearchQuery {
                    status: "missing".into(),
                    ..Default::default()
                },
                false
            )
            .unwrap()
            .total,
        101
    );
    let deleted = post(&token, 101).send().await.unwrap();
    assert!(deleted.status().is_redirection());
    let location = deleted.headers()["location"].to_str().unwrap();
    assert!(location.contains("status=missing&message="));
    let completed = client
        .get(format!("{}{location}", server.base))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(completed.contains("不在スキルの登録情報と履歴を削除しました。"));
    assert!(!completed.contains("action=\"/missing/forget\""));
    assert_eq!(
        library.search(SearchQuery::default(), false).unwrap().total,
        1
    );
    assert!(source.join("keep/SKILL.md").is_file());
    assert!(root.join("retired/old-001/SKILL.md").is_file());
    let histories: i64 = db
        .query_row(
            "SELECT count(*) FROM history WHERE skill_id=?1",
            [&old_id],
            |r| r.get(0),
        )
        .unwrap();
    let policies: i64 = db
        .query_row(
            "SELECT count(*) FROM settings WHERE key=?1",
            [format!("codex_policy:{old_id}")],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!((histories, policies), (0, 0));
    assert_eq!(
        library
            .search(
                SearchQuery {
                    q: "old".into(),
                    ..Default::default()
                },
                false
            )
            .unwrap()
            .total,
        0
    );
    assert!(
        !post(&token, 101)
            .send()
            .await
            .unwrap()
            .status()
            .is_redirection()
    );
}

#[tokio::test]
async fn repository_bulk_form_validates_input_and_updates_all_selected_pages() {
    let root = std::env::var_os("SKILVELLA_MIGRATION_TEST_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(format!("repository-bulk-http-{}", uuid::Uuid::new_v4()));
    let source = root.join("source");
    let special = "repo & 日本語 + %";
    for repository in ["repo", special, "unselected"] {
        fs::create_dir_all(source.join(repository).join(".git")).unwrap();
    }
    for folder in (0..105)
        .map(|i| format!("repo/skill-{i:02}"))
        .chain([format!("{special}/one"), "unselected/one".into()])
    {
        fs::create_dir_all(source.join(&folder)).unwrap();
        fs::write(
            source.join(&folder).join("SKILL.md"),
            format!(
                "---\nname: {}\ndescription: Bulk form fixture\n---\nOriginal body\n",
                folder.replace('/', "-")
            ),
        )
        .unwrap();
    }
    let library = Library::open_for_user(&root.join("data"), &root.join("home"), None).unwrap();
    library.add_source("local", &source).unwrap();
    fs::rename(
        source.join("repo/skill-103/SKILL.md"),
        source.join("repo/skill-103/retired.txt"),
    )
    .unwrap();
    fs::write(
        source.join("repo/skill-104/SKILL.md"),
        "---\nname: [broken\n---\ninvalid",
    )
    .unwrap();
    library.scan().unwrap();
    library.set_codex_enabled(true).unwrap();
    let control = library.clone();
    let state = AppState::new(library);
    let token = state.csrf.to_string();
    let server = common::HttpServer::start(state).await;
    let client = Client::new();
    let post = |fields: &[(String, String)]| {
        let encode = |value: &str| {
            value
                .bytes()
                .map(|byte| format!("%{byte:02X}"))
                .collect::<String>()
        };
        let body = fields
            .iter()
            .map(|(key, value)| format!("{}={}", encode(key), encode(value)))
            .collect::<Vec<_>>()
            .join("&");
        client
            .post(format!("{}/repositories/bulk", server.base))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(body)
    };
    let scope = |repository: &str| {
        serde_json::json!({
            "repository_source": "local", "repository": repository
        })
        .to_string()
    };
    let valid = vec![
        ("csrf".to_owned(), token),
        ("published".into(), "true".into()),
        ("codex_mode".into(), "automatic".into()),
        ("selected".into(), scope("repo")),
        ("selected".into(), scope(special)),
        ("selected".into(), scope("repo")),
    ];
    let top = client
        .get(&server.base)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(top.contains("action=\"/repositories/bulk\""));
    assert!(top.contains("data-count=\"103\""));
    assert!(top.contains("対象件数はスキャン索引に基づきます。"));
    for (key, replacement) in [
        ("csrf", "invalid"),
        ("published", "yes"),
        ("codex_mode", "invalid"),
        ("selected", "not-json"),
    ] {
        let mut fields = valid.clone();
        fields.iter_mut().find(|(name, _)| name == key).unwrap().1 = replacement.into();
        let response = post(&fields).send().await.unwrap();
        assert_eq!(
            response.status().as_u16(),
            if key == "csrf" { 403 } else { 400 }
        );
    }
    for key in ["csrf", "published", "codex_mode"] {
        let mut fields = valid.clone();
        fields.push(fields.iter().find(|(name, _)| name == key).unwrap().clone());
        assert_eq!(post(&fields).send().await.unwrap().status().as_u16(), 400);
        fields.retain(|(name, _)| name != key);
        assert_eq!(post(&fields).send().await.unwrap().status().as_u16(), 400);
    }
    for (has_selection, published, mode, message) in [
        (
            false,
            "invalid",
            "invalid",
            "取得元を1件以上選んでください。",
        ),
        (true, "invalid", "invalid", "公開状態が不正です。"),
        (true, "", "invalid", "Codexでの使い方を選択してください。"),
        (
            true,
            "",
            "",
            "変更する公開状態・Codexでの使い方を指定してください。",
        ),
    ] {
        let mut fields = valid.clone();
        fields
            .iter_mut()
            .find(|(name, _)| name == "published")
            .unwrap()
            .1 = published.into();
        fields
            .iter_mut()
            .find(|(name, _)| name == "codex_mode")
            .unwrap()
            .1 = mode.into();
        if !has_selection {
            fields.retain(|(name, _)| name != "selected");
        }
        let response = post(&fields).send().await.unwrap();
        assert_eq!(response.status().as_u16(), 400);
        assert!(response.text().await.unwrap().contains(message));
    }
    let mut unknown = valid.clone();
    unknown.push(("selected".into(), scope("unknown")));
    assert_eq!(post(&unknown).send().await.unwrap().status().as_u16(), 400);
    assert!(
        control
            .search(SearchQuery::default(), true)
            .unwrap()
            .skills
            .is_empty()
    );
    let result = post(&valid).send().await.unwrap();
    assert!(result.status().is_success());
    assert_eq!(result.url().path(), "/");
    let body = result.text().await.unwrap();
    assert!(body.contains("取得元の一括設定: 104件更新、2件対象外。"));
    let mut all = control
        .search(
            SearchQuery {
                limit: 100,
                ..Default::default()
            },
            false,
        )
        .unwrap()
        .skills;
    all.extend(
        control
            .search(
                SearchQuery {
                    limit: 100,
                    offset: 100,
                    ..Default::default()
                },
                false,
            )
            .unwrap()
            .skills,
    );
    assert_eq!(all.len(), 107);
    assert_eq!(all.iter().filter(|skill| skill.published).count(), 104);
    for skill in all {
        let selected = !skill.relative_path.starts_with("unselected/")
            && !skill.missing
            && skill.error.is_none();
        assert_eq!(skill.published, selected);
        let policy = control.codex_policy(&skill.id).unwrap();
        assert_eq!((policy.automatic, policy.command), (selected, selected));
    }
    let codex = control.codex_settings().unwrap();
    assert!(codex.last_error.is_empty(), "{}", codex.last_error);
    assert_eq!(codex.exported, 104);
    assert_eq!(
        fs::read_to_string(source.join("repo/skill-00/SKILL.md")).unwrap(),
        "---\nname: repo-skill-00\ndescription: Bulk form fixture\n---\nOriginal body\n"
    );

    // 取得元が未確認なら、古い索引に有効な行が残っていても対象件数を表示しない。
    fs::rename(&source, root.join("offline-source")).unwrap();
    rusqlite::Connection::open(root.join("data/library.sqlite3"))
        .unwrap()
        .execute("DELETE FROM settings WHERE key LIKE 'repositories:%'", [])
        .unwrap();
    let group = control.repository_groups().unwrap().remove(0);
    assert!(group.unknown && group.available > 0);
    let unknown = client
        .get(&server.base)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let card = unknown
        .split("<div class=\"repository-card\">")
        .nth(1)
        .unwrap();
    assert!(card.contains("取得元を確認できません"));
    assert!(card.contains("disabled"));
    assert!(!card.contains("<!--i18n-->対象:"));
}

#[tokio::test]
async fn repository_selection_scopes_search_counts_and_pagination() {
    let root = std::env::var_os("SKILVELLA_MIGRATION_TEST_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(format!("repositories-{}", uuid::Uuid::new_v4()));
    let source = root.join("source");
    fs::create_dir_all(source.join("repo/.git")).unwrap();
    fs::create_dir_all(source.join("empty/.git")).unwrap();
    fs::create_dir_all(source.join(".git")).unwrap();
    fs::create_dir_all(source.join("repo-extra")).unwrap();
    fs::write(
        source.join("repo-extra/.git"),
        "gitdir: ../../unread-target",
    )
    .unwrap();
    for (folder, name) in (0..52)
        .map(|i| (format!("repo/skill-{i:02}"), format!("skill-{i:02}")))
        .chain([
            ("repo-extra/other".into(), "other-repository".into()),
            ("personal".into(), "personal-only".into()),
        ])
    {
        fs::create_dir_all(source.join(&folder)).unwrap();
        fs::write(
            source.join(folder).join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Searchable example\n---\nBody\n"),
        )
        .unwrap();
    }
    let library = Library::open_for_user(&root.join("data"), &root.join("home"), None).unwrap();
    library.add_source("test", &source).unwrap();
    for skill in library
        .search(
            SearchQuery {
                limit: 100,
                ..Default::default()
            },
            false,
        )
        .unwrap()
        .skills
    {
        library
            .set_metadata(
                &skill.id,
                SkillMetadata {
                    tags: vec!["chosen".into()],
                    category: "tools".into(),
                    published: false,
                },
            )
            .unwrap();
    }
    // 移動前の索引を残し、カードと状態フィルターに不在を含める。
    fs::rename(
        source.join("repo/skill-00/SKILL.md"),
        source.join("repo/skill-00/retired.txt"),
    )
    .unwrap();
    library.scan().unwrap();
    let standalone = root.join("standalone");
    fs::create_dir_all(standalone.join(".git")).unwrap();
    fs::create_dir_all(standalone.join("one")).unwrap();
    fs::write(
        standalone.join("one/SKILL.md"),
        "---\nname: standalone-skill\ndescription: Standalone repository\n---\nBody\n",
    )
    .unwrap();
    library.add_source("standalone", &standalone).unwrap();
    // 旧DBのアップグレードを再現する。手動走査なしで起動時に種別を保存する。
    rusqlite::Connection::open(root.join("data/library.sqlite3"))
        .unwrap()
        .execute("DELETE FROM settings WHERE key LIKE 'repositories:%'", [])
        .unwrap();
    drop(library);
    let library = Library::open_for_user(&root.join("data"), &root.join("home"), None).unwrap();
    assert!(
        rusqlite::Connection::open(root.join("data/library.sqlite3"))
            .unwrap()
            .query_row(
                "SELECT count(*) FROM settings WHERE key LIKE 'repositories:%'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap()
            >= 2
    );
    let control = library.clone();
    let state = AppState::new(library);
    let token = state.csrf.to_string();
    let selected_id = state
        .library
        .search(
            SearchQuery {
                q: "skill-01".into(),
                ..Default::default()
            },
            false,
        )
        .unwrap()
        .skills[0]
        .id
        .clone();
    let server = common::HttpServer::start(state).await;
    let client = Client::new();
    let base = server.base.clone();
    let get = |query: &str| client.get(format!("{base}/{query}"));
    let top = get("").send().await.unwrap().text().await.unwrap();
    assert!(!top.contains("<table>"));
    fs::create_dir_all(root.join("standalone").join("empty/.git")).unwrap();
    control.scan().unwrap();
    assert!(
        control
            .repository_groups()
            .unwrap()
            .iter()
            .any(|group| group.source == "standalone"
                && group.repository == "empty"
                && group.count == 0)
    );
    assert!(
        !control
            .repository_groups()
            .unwrap()
            .iter()
            .any(|group| group.source == "standalone" && group.repository == ".")
    );
    fs::rename(
        root.join("standalone").join("empty"),
        root.join("retired-empty"),
    )
    .unwrap();
    control.scan().unwrap();
    assert!(
        control
            .repository_groups()
            .unwrap()
            .iter()
            .any(|group| group.source == "standalone" && group.repository == ".")
    );
    assert!(
        !control
            .repository_groups()
            .unwrap()
            .iter()
            .any(|group| group.source == "standalone" && group.repository == "empty")
    );
    assert!(!top.contains("skill-name"));
    assert!(top.contains("repository=repo-extra"));
    assert!(top.contains("repository=empty"));
    assert!(!top.contains("repository_source=test&#38;repository=."));
    assert!(top.contains("repository=repo\""));
    assert!(top.contains("repository=\""));
    assert!(top.contains("現在のファイル: <!--/i18n-->51 ·"));
    assert!(top.contains("<!--/i18n-->1</span>"));
    let page = get("?repository_source=test&repository=repo")
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(page.contains("<table>"));
    assert!(!page.contains("skill-00"));
    assert!(page.contains("skill-01"));
    let all = get("?repository_source=test&repository=repo&status=all")
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(all.contains("skill-00"));
    assert!(all.contains("skill-01"));
    assert!(!page.contains("other-repository"));
    assert!(!page.contains("personal-only"));
    assert!(page.contains("repository_source=test&#38;repository=repo&#38;q="));
    assert!(page.contains("offset=50"));
    let filtered = get("?repository_source=test&repository=repo&q=skill&tag=chosen&category=tools&status=unpublished").send().await.unwrap().text().await.unwrap();
    assert!(filtered.contains("q=skill&#38;source=test&#38;tag=chosen&#38;category=tools&#38;status=unpublished&#38;offset=50"));
    let next = get("?repository_source=test&repository=repo&offset=50")
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(next.contains("skill-51"));
    assert!(!next.contains("skill-00"));
    let personal = get("?repository_source=test&repository=")
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(personal.contains("personal-only"));
    assert!(!personal.contains("skill-00"));
    assert!(!personal.contains("other-repository"));
    let missing = get("?repository_source=test&repository=repo&status=missing&q=skill&tag=chosen&category=tools&source=wrong").send().await.unwrap().text().await.unwrap();
    assert!(missing.contains("skill-00"));
    assert!(!missing.contains("skill-01"));
    let empty = get("?repository_source=test&repository=unknown")
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(empty.contains("該当するスキルはありません"));
    assert!(!empty.contains("<table>"));
    let unselected = get("?q=skill&category=tools")
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(!unselected.contains("<table>"));
    let standalone = get("?repository_source=standalone&repository=.")
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(standalone.contains("standalone-skill"));
    assert!(!standalone.contains("personal-only"));
    let detail = get(&format!("skills/{selected_id}"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        detail.contains("class=\"back\" href=\"/?repository_source=test&#38;repository=repo\"")
    );
    for action in ["scan", "bulk"] {
        let response = client.post(format!("{base}/{action}"))
            .header("content-type", "application/x-www-form-urlencoded")
            .body(format!("csrf={token}&repository_source=test&repository=repo&selected={selected_id}&published=true&tags="))
            .send().await.unwrap();
        assert!(response.status().is_success());
        assert!(
            response
                .url()
                .query()
                .unwrap()
                .contains("repository_source=test&repository=repo&")
        );
        let body = response.text().await.unwrap();
        assert!(body.contains("<table>"));
        assert!(body.contains("skill-01"));
        assert!(!body.contains("personal-only"));
    }
    let top = client
        .post(format!("{base}/scan"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body(format!("csrf={token}"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(!top.contains("<table>"));
    fs::rename(source.join("repo"), root.join("offline-repo")).unwrap();
    control.scan().unwrap();
    let groups = control.repository_groups().unwrap();
    let repository = groups
        .iter()
        .find(|group| group.source == "test" && group.repository == "repo")
        .unwrap();
    assert_eq!((repository.count, repository.missing), (52, 52));
    assert_eq!(
        groups
            .iter()
            .find(|group| group.source == "test" && group.repository.is_empty())
            .unwrap()
            .count,
        1
    );
    fs::rename(&source, root.join("offline-source")).unwrap();
    control.scan().unwrap();
    let reopened = Library::open_for_user(&root.join("data"), &root.join("home"), None).unwrap();
    assert!(
        reopened
            .repository_groups()
            .unwrap()
            .iter()
            .any(|group| group.source == "test" && group.repository == "empty" && group.count == 0)
    );
    assert!(
        reopened
            .repository_groups()
            .unwrap()
            .iter()
            .any(|group| group.source == "test" && group.repository == "repo" && group.count == 52)
    );
    let offline = get("?repository_source=test&repository=repo&status=missing")
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(offline.contains("skill-00"));
    assert!(!offline.contains("personal-only"));
    fs::create_dir_all(root.join("standalone/old-child/.git")).unwrap();
    fs::write(
        root.join("standalone/old-child/SKILL.md"),
        "---\nname: retired-child\ndescription: Previous child repository\n---\nBody\n",
    )
    .unwrap();
    control.scan().unwrap();
    fs::rename(
        root.join("standalone/old-child"),
        root.join("offline-old-child"),
    )
    .unwrap();
    control.scan().unwrap();
    let groups = control.repository_groups().unwrap();
    assert!(
        groups.iter().any(|group| group.source == "standalone"
            && group.repository == "."
            && group.count == 1)
    );
    assert!(groups.iter().any(|group| group.source == "standalone"
        && group.repository == "old-child"
        && group.count == 1
        && group.missing == 1));
    let current = get("?repository_source=standalone&repository=.")
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(current.contains("standalone-skill"));
    assert!(!current.contains("retired-child"));
    let previous = get("?repository_source=standalone&repository=old-child&status=missing")
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(previous.contains("retired-child"));
    assert!(!previous.contains("standalone-skill"));
    // 初回から取得元を確認できない場合、個人スキルと誤分類せず明示する。
    rusqlite::Connection::open(root.join("data/library.sqlite3"))
        .unwrap()
        .execute("DELETE FROM settings WHERE key LIKE 'repositories:%'", [])
        .unwrap();
    let unknown = get("").send().await.unwrap();
    assert_eq!(unknown.status(), reqwest::StatusCode::OK);
    assert!(
        unknown
            .text()
            .await
            .unwrap()
            .contains("取得元を確認できません")
    );
    let unknown = get("?repository_source=test&repository=repo")
        .send()
        .await
        .unwrap();
    assert_eq!(unknown.status(), reqwest::StatusCode::BAD_REQUEST);
    let unavailable_detail = get(&format!("skills/{selected_id}")).send().await.unwrap();
    assert_eq!(unavailable_detail.status(), reqwest::StatusCode::OK);
    assert!(
        unavailable_detail
            .text()
            .await
            .unwrap()
            .contains("class=\"back\" href=\"/sources\"")
    );
    let normal = get("?repository_source=standalone&repository=.")
        .send()
        .await
        .unwrap();
    assert_eq!(normal.status(), reqwest::StatusCode::OK);
    assert!(normal.text().await.unwrap().contains("standalone-skill"));
    let normal_id = control
        .search(
            SearchQuery {
                q: "standalone-skill".into(),
                ..Default::default()
            },
            false,
        )
        .unwrap()
        .skills[0]
        .id
        .clone();
    let normal = get(&format!("skills/{normal_id}")).send().await.unwrap();
    assert_eq!(normal.status(), reqwest::StatusCode::OK);
}
