use reqwest::Client;
use skilvella::{
    AppState,
    library::Library,
    model::{SearchQuery, SkillMetadata},
};
use std::{fs, path::PathBuf};

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
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let task = tokio::spawn(async move {
        axum::serve(listener, skilvella::server::router(state, port))
            .await
            .unwrap();
    });
    struct Stop(tokio::task::JoinHandle<()>);
    impl Drop for Stop {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let _stop = Stop(task);
    let client = Client::new();
    let base = format!("http://127.0.0.1:{port}");
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
    assert!(top.contains("<!--/i18n-->52 ·"));
    assert!(top.contains("<!--/i18n-->1</span>"));
    let page = get("?repository_source=test&repository=repo")
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(page.contains("<table>"));
    assert!(page.contains("skill-00"));
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
