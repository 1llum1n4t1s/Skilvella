use skilvella::{
    library::Library,
    model::{CodexSkillPolicy, RepositorySelection, SearchQuery, SkillMetadata},
};
use std::{fs, path::PathBuf};

fn fixture() -> (PathBuf, Library, PathBuf) {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".work/test-library")
        .join(format!("library-test-{}", uuid::Uuid::new_v4()));
    let source = base.join("source");
    fs::create_dir_all(source.join("alpha")).unwrap();
    fs::write(source.join("alpha/SKILL.md"), skill("日本語検索", "初版")).unwrap();
    fs::write(source.join("alpha/reference.txt"), "one\ntwo\nthree\n").unwrap();
    let library = Library::open(&base.join("data")).unwrap();
    library.add_source("local", &source).unwrap();
    unpublish_all(&library);
    (base, library, source)
}
fn skill(name: &str, body: &str) -> String {
    format!(
        "---\nname: {name}\ndescription: Description for a skill\ncustom: preserved\n---\n{body}\n"
    )
}
/// 新規スキルは走査で公開状態になる。未公開スキルの扱いを検証するテストは、準備段階で全件を明示的に未公開へ戻す。
fn unpublish_all(l: &Library) {
    // 検索は1回あたり最大100件のため、全件を取り切るまでoffsetを進める。
    let mut ids = Vec::new();
    loop {
        let page = l
            .search(
                SearchQuery {
                    limit: 100,
                    offset: ids.len(),
                    ..Default::default()
                },
                false,
            )
            .unwrap();
        if page.skills.is_empty() {
            break;
        }
        ids.extend(page.skills.into_iter().map(|skill| skill.id));
    }
    for id in ids {
        l.set_published(&id, false).unwrap();
    }
}
fn id(l: &Library) -> String {
    l.search(SearchQuery::default(), false).unwrap().skills[0]
        .id
        .clone()
}
fn publish(l: &Library, id: &str) {
    l.set_metadata(
        id,
        SkillMetadata {
            tags: vec!["仕事".into(), "rust".into()],
            category: "development".into(),
            published: true,
        },
    )
    .unwrap();
}

fn repository_bulk_fixture() -> (PathBuf, Library, PathBuf) {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".work/test-library")
        .join(format!("repository-bulk-{}", uuid::Uuid::new_v4()));
    let source = base.join("source");
    for repo in ["repo", "repo-extra", "peer", "empty"] {
        fs::create_dir_all(source.join(repo).join(".git")).unwrap();
    }
    for folder in (0..103).map(|i| format!("repo/skill-{i:03}")).chain([
        "repo-extra/one".into(),
        "peer/one".into(),
        "personal".into(),
    ]) {
        fs::create_dir_all(source.join(&folder)).unwrap();
        fs::write(
            source.join(&folder).join("SKILL.md"),
            skill(&folder.replace('/', "-"), "original"),
        )
        .unwrap();
    }
    let library = Library::open_for_user(&base.join("data"), &base.join("home"), None).unwrap();
    library.add_source("local", &source).unwrap();
    unpublish_all(&library);
    (base, library, source)
}

fn repository_scope(source: &str, repository: &str) -> RepositorySelection {
    RepositorySelection {
        repository_source: source.into(),
        repository: repository.into(),
    }
}

#[test]
fn repository_bulk_updates_all_selected_skills_and_preserves_files_and_history() {
    let (base, library, source) = repository_bulk_fixture();
    fs::rename(
        source.join("repo/skill-101/SKILL.md"),
        source.join("repo/skill-101/retired.txt"),
    )
    .unwrap();
    fs::write(
        source.join("repo/skill-102/SKILL.md"),
        "---\nname: [broken\n---\ninvalid",
    )
    .unwrap();
    library.scan().unwrap();
    let groups = library.repository_groups().unwrap();
    let repo = groups
        .iter()
        .find(|group| group.repository == "repo")
        .unwrap();
    assert_eq!((repo.count, repo.missing, repo.available), (103, 1, 101));
    // カード取得後に増えたスキルもPOST時点の対象へ含める。
    let added = library
        .create_skill("local", "repo/new", &skill("new", "new file"))
        .unwrap();
    let before = library.detail(&added, false).unwrap();
    library
        .save_content(&added, &before.revision, &skill("new", "edited file"))
        .unwrap();
    let before = library.detail(&added, false).unwrap();
    let snapshots: Vec<_> = (0..101)
        .map(|i| {
            let path = source.join(format!("repo/skill-{i:03}/SKILL.md"));
            let bytes = fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect();
    let policy = CodexSkillPolicy {
        automatic: true,
        command: true,
    };
    assert_eq!(
        library
            .bulk_repository_settings(
                &[
                    repository_scope("local", "repo"),
                    repository_scope("local", "peer"),
                    repository_scope("local", "repo"),
                ],
                Some(true),
                Some(policy.clone())
            )
            .unwrap(),
        (103, 2)
    );
    let db = rusqlite::Connection::open(base.join("data/library.sqlite3")).unwrap();
    let count = |sql: &str| db.query_row(sql, [], |row| row.get::<_, i64>(0)).unwrap();
    assert_eq!(count("SELECT count(*) FROM skills WHERE published=1"), 103);
    assert_eq!(
        count("SELECT count(*) FROM skills WHERE published=1 AND (missing=1 OR error IS NOT NULL)"),
        0
    );
    assert_eq!(
        count("SELECT count(*) FROM settings WHERE key LIKE 'codex_policy:%'"),
        103
    );
    for repository in ["repo-extra", ""] {
        let unselected = library
            .search_repository(
                SearchQuery::default(),
                repository_scope("local", repository),
            )
            .unwrap();
        assert_eq!(unselected.total, 1);
        assert!(!unselected.skills[0].published);
        assert_eq!(
            library.codex_policy(&unselected.skills[0].id).unwrap(),
            CodexSkillPolicy::default()
        );
    }
    assert_eq!(library.codex_policy(&added).unwrap(), policy);
    let after = library.detail(&added, false).unwrap();
    assert_eq!(after.content, before.content);
    assert_eq!(
        after.history.iter().map(|h| h.id).collect::<Vec<_>>(),
        before.history.iter().map(|h| h.id).collect::<Vec<_>>()
    );
    for (path, bytes) in snapshots {
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
    assert_eq!(
        fs::read(source.join("repo/new/SKILL.md")).unwrap(),
        before.content.as_bytes()
    );
    assert_eq!(
        library
            .bulk_repository_settings(
                &[repository_scope("local", "")],
                None,
                Some(CodexSkillPolicy {
                    automatic: false,
                    command: true
                })
            )
            .unwrap(),
        (1, 0)
    );
    let standalone = base.join("standalone");
    fs::create_dir_all(standalone.join(".git")).unwrap();
    fs::create_dir_all(standalone.join("one")).unwrap();
    fs::write(standalone.join("one/SKILL.md"), skill("standalone", "body")).unwrap();
    library.add_source("standalone", &standalone).unwrap();
    assert_eq!(
        library
            .bulk_repository_settings(&[repository_scope("standalone", ".")], Some(true), None)
            .unwrap(),
        (1, 0)
    );
    // 一括設定の結果は新規スキルへ継承しない。公開状態は一括設定ではなく走査の既定（公開）で決まり、
    // Codex同期方針は一括設定の値を引き継がず既定値のままになる。
    let later = library
        .create_skill("local", "repo/later", &skill("later", "body"))
        .unwrap();
    assert!(library.detail(&later, false).unwrap().summary.published);
    assert_eq!(
        library.codex_policy(&later).unwrap(),
        CodexSkillPolicy::default()
    );
}

#[test]
fn repository_bulk_rejects_invalid_scopes_and_rolls_back_stale_publication() {
    let (base, library, source) = repository_bulk_fixture();
    let selected = repository_scope("local", "repo");
    let policy = CodexSkillPolicy {
        automatic: true,
        command: true,
    };
    let db = rusqlite::Connection::open(base.join("data/library.sqlite3")).unwrap();
    let unchanged = || {
        assert_eq!(
            db.query_row("SELECT count(*) FROM skills WHERE published=1", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
            0
        );
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM settings WHERE key LIKE 'codex_policy:%'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
    };
    assert!(
        library
            .bulk_repository_settings(&[], Some(true), None)
            .is_err()
    );
    assert!(
        library
            .bulk_repository_settings(std::slice::from_ref(&selected), None, None)
            .is_err()
    );
    assert!(
        library
            .bulk_repository_settings(
                std::slice::from_ref(&selected),
                None,
                Some(CodexSkillPolicy {
                    automatic: true,
                    command: false
                })
            )
            .is_err()
    );
    for invalid in [
        repository_scope("unknown", "repo"),
        repository_scope("local", "unknown"),
        repository_scope("local", "empty"),
    ] {
        assert!(
            library
                .bulk_repository_settings(
                    &[selected.clone(), invalid],
                    Some(true),
                    Some(policy.clone())
                )
                .is_err()
        );
        unchanged();
    }
    // 索引が正常でも、公開直前のファイル再検査で失敗すれば他の対象も戻る。
    let stale = source.join("repo/skill-102/SKILL.md");
    let original = fs::read(&stale).unwrap();
    fs::write(&stale, "---\nname: [broken\n---\nstale").unwrap();
    assert!(
        library
            .bulk_repository_settings(
                std::slice::from_ref(&selected),
                Some(true),
                Some(policy.clone())
            )
            .is_err()
    );
    unchanged();
    fs::write(&stale, original).unwrap();
    fs::write(
        source.join("peer/one/SKILL.md"),
        "---\nname: [broken\n---\ninvalid",
    )
    .unwrap();
    library.scan().unwrap();
    assert!(
        library
            .bulk_repository_settings(
                &[repository_scope("local", "peer")],
                Some(false),
                Some(policy.clone())
            )
            .is_err()
    );
    unchanged();
    // 切断中にキャッシュからカードを表示できても設定は変更しない。
    let disconnected = base.join("disconnected-source");
    fs::rename(&source, &disconnected).unwrap();
    assert!(
        library
            .bulk_repository_settings(
                std::slice::from_ref(&selected),
                Some(false),
                Some(policy.clone())
            )
            .is_err()
    );
    unchanged();
    db.execute(
        "UPDATE settings SET key='test-unknown:' || key WHERE key LIKE 'repositories:%'",
        [],
    )
    .unwrap();
    assert!(
        library
            .repository_groups()
            .unwrap()
            .iter()
            .any(|group| group.unknown)
    );
    assert!(
        library
            .bulk_repository_settings(&[repository_scope("local", "")], Some(true), None)
            .is_err()
    );
    unchanged();
    fs::rename(&disconnected, &source).unwrap();
    library.remove_source("local").unwrap();
    assert!(
        library
            .bulk_repository_settings(&[selected], Some(true), Some(policy))
            .is_err()
    );
    unchanged();
}

#[test]
fn publication_metadata_history_and_restore_survive_rescan_and_reopen() {
    let (base, library, source) = fixture();
    assert_eq!(library.sources().unwrap()[0].display_name, "local");
    library
        .set_source_display_name("local", "仕事用 🌸")
        .unwrap();
    let id = id(&library);
    // Python hashlibで算出した既知値。依存更新でも既存IDと本文revisionを維持する。
    assert_eq!(
        id,
        "8ca3b665c809f41fd8a3d10fe0d70b0bca0389822cfd888922784f87a4c99556"
    );
    assert_eq!(
        library.detail(&id, false).unwrap().revision,
        "7adc13176e6bbaafac0f0bfc1441aa92bda64f489958c4c9fb42bf946226de6a"
    );
    assert_eq!(
        library.search(SearchQuery::default(), true).unwrap().total,
        0
    );
    assert!(library.detail(&id, true).is_err());
    assert!(library.read_file(&id, "reference.txt", 1, 10).is_err());
    publish(&library, &id);
    let before = library.detail(&id, true).unwrap();
    let updated = skill("日本語検索", "second revision");
    library
        .save_content(&id, &before.revision, &updated)
        .unwrap();
    let edited = library.detail(&id, false).unwrap();
    assert_eq!(edited.history.len(), 1);
    assert_eq!(edited.content, updated);
    library.scan().unwrap();
    library.remove_source("local").unwrap();
    assert_eq!(
        library.search(SearchQuery::default(), false).unwrap().total,
        0
    );
    assert!(source.join("alpha/SKILL.md").is_file());
    library.add_source("local", &source).unwrap();
    drop(library);
    let library = Library::open(&base.join("data")).unwrap();
    assert_eq!(library.sources().unwrap()[0].display_name, "仕事用 🌸");
    let detail = library.detail(&id, true).unwrap();
    assert_eq!(detail.summary.tags, vec!["仕事", "rust"]);
    assert_eq!(detail.summary.category, "development");
    library
        .restore(&id, detail.history[0].id, &detail.revision)
        .unwrap();
    let restored = library.detail(&id, true).unwrap();
    assert_eq!(restored.content, before.content);
    assert_eq!(restored.history.len(), 2);
    let file = library.read_file(&id, "reference.txt", 2, 1).unwrap();
    assert_eq!(file.text, "two");
    assert_eq!(file.total_lines, 3);
    for (content, start, limit, expected, total) in [
        ("", 0, 0, "", 0),
        ("one\r\n\r\nthree\r\n", 0, 0, "one", 3),
        ("one\r\n\r\nthree", 2, 10, "\nthree", 3),
        ("one\n", 2, 1, "", 1),
        ("one\ntwo", usize::MAX, 1, "", 2),
    ] {
        fs::write(source.join("alpha/reference.txt"), content).unwrap();
        let file = library
            .read_file(&id, "reference.txt", start, limit)
            .unwrap();
        assert_eq!(file.text, expected);
        assert_eq!(file.total_lines, total);
        assert_eq!(file.start_line, start.max(1));
    }
    fs::write(source.join("alpha/reference.txt"), "line\n".repeat(1001)).unwrap();
    let file = library
        .read_file(&id, "reference.txt", 1, usize::MAX)
        .unwrap();
    assert_eq!(file.total_lines, 1001);
    assert_eq!(file.text.lines().count(), 1000);
}

#[test]
fn source_counts_preserve_invalid_missing_empty_and_inactive_sources() {
    let (base, library, source) = fixture();
    fs::create_dir_all(source.join("invalid")).unwrap();
    fs::write(source.join("invalid/SKILL.md"), "invalid frontmatter").unwrap();
    library.scan().unwrap();
    let sources = library.sources().unwrap();
    assert_eq!((sources[0].skill_count, sources[0].error_count), (2, 1));
    fs::rename(source.join("invalid"), base.join("retired-invalid")).unwrap();
    library.scan().unwrap();
    let empty = base.join("empty");
    fs::create_dir_all(&empty).unwrap();
    library.add_source("empty", &empty).unwrap();
    let inactive = base.join("inactive");
    fs::create_dir_all(&inactive).unwrap();
    fs::write(inactive.join("SKILL.md"), skill("inactive", "body")).unwrap();
    library.add_source("inactive", &inactive).unwrap();
    library.remove_source("inactive").unwrap();
    let sources = library.sources().unwrap();
    assert_eq!(
        sources
            .iter()
            .map(|s| (s.alias.as_str(), s.skill_count, s.error_count))
            .collect::<Vec<_>>(),
        vec![("empty", 0, 0), ("local", 1, 1)]
    );
}

#[test]
fn forget_missing_preserves_search_filters_and_ignores_page_bounds() {
    let (_base, library, source) = fixture();
    let alpha = id(&library);
    let before = library.detail(&alpha, false).unwrap();
    library
        .save_content(&alpha, &before.revision, &skill("日本語検索", "İSTANBUL"))
        .unwrap();
    let mut ids = vec![alpha.clone()];
    for (folder, body) in [
        ("category", "İSTANBUL"),
        ("tag", "İSTANBUL"),
        ("query", "別の本文"),
    ] {
        ids.push(
            library
                .create_skill("local", folder, &skill("日本語検索", body))
                .unwrap(),
        );
    }
    for (index, id) in ids.iter().enumerate() {
        library
            .set_metadata(
                id,
                SkillMetadata {
                    tags: vec![if index == 2 { "別タグ" } else { "絞込" }.into()],
                    category: if index == 1 { "別分類" } else { "分類" }.into(),
                    published: false,
                },
            )
            .unwrap();
    }
    for folder in ["alpha", "category", "tag", "query"] {
        fs::rename(
            source.join(folder).join("SKILL.md"),
            source.join(folder).join("retired.txt"),
        )
        .unwrap();
    }
    library.scan().unwrap();
    let query = SearchQuery {
        q: "日本語 İSTANBUL".into(),
        tag: "絞込".into(),
        category: "分類".into(),
        status: "missing".into(),
        ..Default::default()
    };
    let matching = library.search(query.clone(), false).unwrap();
    assert_eq!(matching.total, 1);
    assert_eq!(matching.skills[0].id, alpha);
    assert_eq!(
        library
            .forget_missing(
                SearchQuery {
                    offset: usize::MAX,
                    limit: 1,
                    ..query
                },
                repository_scope("local", ""),
                1,
            )
            .unwrap(),
        1
    );
    assert!(library.detail(&alpha, false).is_err());
    for id in &ids[1..] {
        assert!(library.detail(id, false).unwrap().summary.missing);
    }
}

#[test]
fn single_updates_keep_unicode_search_and_other_skills_and_scan_repairs_index() {
    let (base, library, _source) = fixture();
    let alpha = id(&library);
    let peer = library
        .create_skill("local", "peer", &skill("別スキル", "保持する検索本文"))
        .unwrap();
    let before = library.detail(&alpha, false).unwrap();
    library
        .save_content(
            &alpha,
            &before.revision,
            &skill("İSTANBUL", "更新した日本語本文"),
        )
        .unwrap();
    library
        .set_metadata(
            &alpha,
            SkillMetadata {
                tags: vec!["İSTANBULタグ".into(), "日本語タグ".into()],
                category: "日本語分類".into(),
                published: true,
            },
        )
        .unwrap();
    for term in [
        "İSTANBUL",
        "更新した日本語本文",
        "İSTANBULタグ",
        "日本語タグ",
        "日本語分類",
    ] {
        let results = library
            .search(
                SearchQuery {
                    q: term.into(),
                    ..Default::default()
                },
                true,
            )
            .unwrap();
        assert_eq!(results.total, 1, "{term}");
        assert_eq!(results.skills[0].id, alpha, "{term}");
    }
    let peer_search = || {
        library
            .search(
                SearchQuery {
                    q: "保持する検索本文".into(),
                    ..Default::default()
                },
                false,
            )
            .unwrap()
    };
    assert_eq!(peer_search().skills[0].id, peer);
    let db = rusqlite::Connection::open(base.join("data/library.sqlite3")).unwrap();
    db.execute("DELETE FROM skill_fts WHERE id=?1", [&peer])
        .unwrap();
    assert_eq!(peer_search().total, 0);
    library.scan().unwrap();
    assert_eq!(peer_search().skills[0].id, peer);
}

#[test]
fn metadata_and_refreshed_content_roll_back_if_index_update_fails() {
    let (base, library, source) = fixture();
    let alpha = id(&library);
    let before = library.detail(&alpha, false).unwrap();
    fs::write(
        source.join("alpha/SKILL.md"),
        skill("外部更新", "公開前の再読込"),
    )
    .unwrap();
    let db = rusqlite::Connection::open(base.join("data/library.sqlite3")).unwrap();
    db.execute("ALTER TABLE skill_fts RENAME TO saved_skill_fts", [])
        .unwrap();
    assert!(
        library
            .set_metadata(
                &alpha,
                SkillMetadata {
                    tags: vec!["保存失敗".into()],
                    category: "保存失敗".into(),
                    published: true,
                }
            )
            .is_err()
    );
    let stored: (String, String, String, bool) = db
        .query_row(
            "SELECT content,tags,category,published FROM skills WHERE id=?1",
            [&alpha],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(stored, (before.content, "[]".into(), String::new(), false));
    db.execute("ALTER TABLE saved_skill_fts RENAME TO skill_fts", [])
        .unwrap();
    library.scan().unwrap();
    let refreshed = library
        .search(
            SearchQuery {
                q: "外部更新".into(),
                ..Default::default()
            },
            false,
        )
        .unwrap();
    assert_eq!(refreshed.total, 1);
    assert_eq!(refreshed.skills[0].id, alpha);
}

#[test]
fn search_pages_preserve_score_name_id_order_and_extreme_offsets() {
    let (_, library, source) = fixture();
    for (dir, name, body) in [
        ("b", "same", "needle"),
        ("c", "same", "needle"),
        ("d", "needle", "body"),
        ("e", "last", "needle"),
    ] {
        fs::create_dir_all(source.join(dir)).unwrap();
        fs::write(source.join(dir).join("SKILL.md"), skill(name, body)).unwrap();
    }
    library.scan().unwrap();
    for q in ["", "needle"] {
        let all = library
            .search(
                SearchQuery {
                    q: q.into(),
                    ..Default::default()
                },
                false,
            )
            .unwrap();
        let mut expected = all.skills.clone();
        expected.sort_by(|a, b| {
            let score = |s: &skilvella::model::SkillSummary| {
                usize::from(!q.is_empty() && s.name.contains(q))
            };
            score(b)
                .cmp(&score(a))
                .then_with(|| a.name.cmp(&b.name))
                .then_with(|| a.id.cmp(&b.id))
        });
        assert_eq!(
            all.skills.iter().map(|s| &s.id).collect::<Vec<_>>(),
            expected.iter().map(|s| &s.id).collect::<Vec<_>>()
        );
        for offset in 0..=all.total {
            let page = library
                .search(
                    SearchQuery {
                        q: q.into(),
                        offset,
                        limit: 1,
                        ..Default::default()
                    },
                    false,
                )
                .unwrap();
            assert_eq!(page.total, all.total);
            assert_eq!(
                page.skills.iter().map(|s| &s.id).collect::<Vec<_>>(),
                expected
                    .iter()
                    .skip(offset)
                    .take(1)
                    .map(|s| &s.id)
                    .collect::<Vec<_>>()
            );
        }
        let page = library
            .search(
                SearchQuery {
                    q: q.into(),
                    offset: usize::MAX,
                    limit: 100,
                    ..Default::default()
                },
                false,
            )
            .unwrap();
        assert_eq!(page.total, all.total);
        assert!(page.skills.is_empty());
    }
}

#[test]
fn source_display_names_validate_without_changing_identity_or_files() {
    let (base, library, source) = fixture();
    let original_id = id(&library);
    let path = library.sources().unwrap()[0].path.clone();
    for invalid in [
        "a\nb".to_owned(),
        "a\0b".into(),
        "a".repeat(101),
        "🌸".repeat(101),
    ] {
        assert!(library.set_source_display_name("local", &invalid).is_err());
    }
    assert!(library.set_source_display_name("unknown", "name").is_err());
    library
        .set_source_display_name("local", &"🌸".repeat(100))
        .unwrap();
    assert_eq!(library.sources().unwrap()[0].display_name, "🌸".repeat(100));
    library
        .set_source_display_name("local", "  <仕事> & \"🌸\"  ")
        .unwrap();
    assert_eq!(
        library.sources().unwrap()[0].display_name,
        "<仕事> & \"🌸\""
    );
    library.scan().unwrap();
    assert_eq!(id(&library), original_id);
    assert_eq!(library.sources().unwrap()[0].path, path);
    assert!(source.join("alpha/SKILL.md").is_file());
    let second_source = base.join("second-source");
    fs::create_dir_all(&second_source).unwrap();
    library.add_source("second", &second_source).unwrap();
    library
        .set_source_display_name("second", "<仕事> & \"🌸\"")
        .unwrap();
    let sources = library.sources().unwrap();
    assert_eq!(sources[0].display_name, sources[1].display_name);
    assert_ne!(sources[0].label(), sources[1].label());
    library.set_source_display_name("local", "   ").unwrap();
    assert_eq!(library.sources().unwrap()[0].display_name, "local");
    library.remove_source("local").unwrap();
    assert!(library.set_source_display_name("local", "hidden").is_err());
}
#[test]
fn yaml_errors_show_file_location_and_cause_without_exposing_invalid_skills() {
    let (_base, library, source) = fixture();
    let id = id(&library);
    publish(&library, &id);
    let duplicate = "---\r\nname: broken\r\ndescription: duplicate\r\nmetadata:\r\n  role: specialist\r\n  role: orchestrator\r\n---\r\n";
    fs::write(source.join("alpha/SKILL.md"), duplicate).unwrap();
    assert_eq!(library.scan().unwrap().errors, 1);
    let indexed = library.search(SearchQuery::default(), false).unwrap();
    let error = indexed.skills[0].error.as_ref().unwrap();
    assert!(error.contains("SKILL.md の6行目・3列目"), "{error}");
    assert!(error.contains("項目名が重複") && error.contains("role: orchestrator"));
    assert_eq!(
        library.detail(&id, false).unwrap().summary.error.as_ref(),
        Some(error)
    );
    assert_eq!(
        library.search(SearchQuery::default(), true).unwrap().total,
        0
    );
    assert!(library.detail(&id, true).is_err());
    assert!(library.read_file(&id, "reference.txt", 1, 3).is_err());

    let malformed = "---\nname: broken\ndescription: [unclosed\n---\n";
    let current = library.detail(&id, false).unwrap();
    let error = library
        .save_content(&id, &current.revision, malformed)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("行目・") && error.contains("列目"),
        "{error}"
    );
    assert!(error.contains("YAMLヘッダーのエラー"));
    assert!(!error.ends_with("列目）: "));
    assert_eq!(
        fs::read_to_string(source.join("alpha/SKILL.md")).unwrap(),
        duplicate
    );
}

#[test]
fn invalid_missing_and_external_edit_conflicts_do_not_expose_content() {
    let (_base, library, source) = fixture();
    let id = id(&library);
    publish(&library, &id);
    let detail = library.detail(&id, false).unwrap();
    fs::write(
        source.join("alpha/SKILL.md"),
        skill("External change", "new externally"),
    )
    .unwrap();
    assert!(
        library
            .save_content(&id, &detail.revision, &skill("Overwrite", "bad"))
            .unwrap_err()
            .to_string()
            .contains("Conflict")
    );
    assert!(
        fs::read_to_string(source.join("alpha/SKILL.md"))
            .unwrap()
            .contains("External change")
    );
    assert!(
        library
            .save_content(&id, &detail.revision, "---\nname: [invalid\n---\n")
            .is_err()
    );
    fs::write(
        source.join("alpha/SKILL.md"),
        "---\nname: 42\ndescription: invalid\n---\n",
    )
    .unwrap();
    assert!(library.detail(&id, true).is_err());
    assert!(library.read_file(&id, "reference.txt", 1, 3).is_err());
    assert_eq!(library.scan().unwrap().errors, 1);
    assert_eq!(
        library.search(SearchQuery::default(), true).unwrap().total,
        0
    );
    let invalid = library.detail(&id, false).unwrap();
    assert!(invalid.summary.error.is_some());
    fs::rename(source.join("alpha/SKILL.md"), source.join("alpha/OFF.md")).unwrap();
    assert_eq!(library.scan().unwrap().missing, 1);
    let missing = library
        .search(
            SearchQuery {
                status: "missing".into(),
                ..Default::default()
            },
            false,
        )
        .unwrap();
    assert_eq!(missing.total, 1);
    assert!(missing.skills[0].published);
    fs::write(source.join("alpha/SKILL.md"), skill("Restored", "body")).unwrap();
    let returned = library.detail(&id, false).unwrap();
    assert!(!returned.summary.missing && returned.needs_rescan);
    assert_ne!(returned.revision, "missing");
    assert!(library.detail(&id, true).is_err());
    library.scan().unwrap();
    assert_eq!(
        library.detail(&id, true).unwrap().summary.tags,
        vec!["仕事", "rust"]
    );
}
#[test]
fn literal_unicode_search_filters_and_path_boundaries() {
    let (_base, library, source) = fixture();
    let id = id(&library);
    publish(&library, &id);
    for term in ["日本語", "日本", "仕事", "日本語検索 Description"] {
        assert_eq!(
            library
                .search(
                    SearchQuery {
                        q: term.into(),
                        ..Default::default()
                    },
                    true
                )
                .unwrap()
                .total,
            1,
            "{term}"
        );
    }
    assert_eq!(
        library
            .search(
                SearchQuery {
                    q: "NOT OR \" *".into(),
                    ..Default::default()
                },
                false
            )
            .unwrap()
            .total,
        0
    );
    assert_eq!(
        library
            .search(
                SearchQuery {
                    tag: "rust".into(),
                    category: "development".into(),
                    source: "local".into(),
                    ..Default::default()
                },
                true
            )
            .unwrap()
            .total,
        1
    );
    for path in ["../outside", "/absolute", "reference.txt/../../outside"]
        .into_iter()
        .chain(cfg!(windows).then_some("C:/windows"))
        .chain(cfg!(windows).then_some("..\\outside"))
    {
        assert!(library.read_file(&id, path, 1, 10).is_err(), "{path}");
        assert!(
            library
                .create_skill("local", path, &skill("New", "body"))
                .is_err(),
            "{path}"
        );
    }
    assert!(library.add_source("nested", &source.join("alpha")).is_err());
    let second = library
        .create_skill("local", "beta", &skill("日本語検索", "second"))
        .unwrap();
    // 新規スキルは走査で公開状態になる（以前は未公開で始まっていた）。
    assert!(library.detail(&second, false).unwrap().summary.published);
    let duplicate = library
        .search(
            SearchQuery {
                status: "duplicates".into(),
                ..Default::default()
            },
            false,
        )
        .unwrap();
    assert_eq!(duplicate.total, 2);
    library
        .create_skill("local", "alpha/child", &skill("nested", "yes"))
        .unwrap();
    let child = library
        .create_skill("local", "group/child", &skill("Child", "preserve"))
        .unwrap();
    library
        .create_skill("local", "group", &skill("Parent", "preserve child"))
        .unwrap();
    assert!(
        library
            .create_skill("local", "group", &skill("Duplicate", "no"))
            .is_err()
    );
    assert!(library.detail(&child, false).is_ok());
}
#[test]
fn cloned_writers_serialize_creates_and_bulk_updates_are_atomic() {
    let (_base, library, _source) = fixture();
    let first = id(&library);
    let threads: Vec<_> = (0..4)
        .map(|n| {
            let l = library.clone();
            std::thread::spawn(move || {
                l.create_skill(
                    "local",
                    &format!("parallel-{n}"),
                    &skill(&format!("Parallel {n}"), "body"),
                )
                .unwrap()
            })
        })
        .collect();
    let ids: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(
        library.search(SearchQuery::default(), false).unwrap().total,
        5
    );
    assert!(
        library
            .bulk_metadata(
                &[first.clone(), "not-found".into()],
                Some(true),
                &["atomic".into()]
            )
            .is_err()
    );
    assert!(!library.detail(&first, false).unwrap().summary.published);
    library
        .bulk_metadata(&ids, Some(true), &["parallel".into()])
        .unwrap();
    assert_eq!(
        library
            .search(
                SearchQuery {
                    tag: "parallel".into(),
                    ..Default::default()
                },
                true
            )
            .unwrap()
            .total,
        4
    );
}

#[test]
fn unicode_casefold_search_and_missing_file_recovery() {
    let (_base, library, source) = fixture();
    let unicode = library
        .create_skill(
            "local",
            "unicode",
            &skill("İSTANBUL", "casefold fixture abİ"),
        )
        .unwrap();
    publish(&library, &unicode);
    library
        .set_metadata(
            &unicode,
            SkillMetadata {
                tags: vec!["foo\"bar".into()],
                category: String::new(),
                published: true,
            },
        )
        .unwrap();
    for term in ["İSTANBUL", "İ", "abi", "i", "foo\"bar", "o\""] {
        let results = library
            .search(
                SearchQuery {
                    q: term.into(),
                    ..Default::default()
                },
                true,
            )
            .unwrap();
        assert_eq!(
            results
                .skills
                .iter()
                .map(|s| s.id.as_str())
                .collect::<Vec<_>>(),
            vec![unicode.as_str()],
            "{term}"
        );
    }

    let id = library
        .search(
            SearchQuery {
                source: "local".into(),
                q: "日本語検索".into(),
                ..Default::default()
            },
            false,
        )
        .unwrap()
        .skills[0]
        .id
        .clone();
    publish(&library, &id);
    let original = library.detail(&id, false).unwrap();
    let edited = skill("日本語検索", "cached edited instructions");
    library
        .save_content(&id, &original.revision, &edited)
        .unwrap();
    let history_id = library.detail(&id, false).unwrap().history[0].id;
    fs::rename(
        source.join("alpha/SKILL.md"),
        source.join("alpha/first.off"),
    )
    .unwrap();
    library.scan().unwrap();
    let missing = library.detail(&id, false).unwrap();
    assert!(missing.summary.missing);
    assert_eq!(missing.revision, "missing");
    assert_eq!(missing.content, edited);
    assert!(library.detail(&id, true).is_err());
    assert!(library.read_file(&id, "reference.txt", 1, 5).is_err());

    library.restore(&id, history_id, &missing.revision).unwrap();
    assert_eq!(
        fs::read_to_string(source.join("alpha/SKILL.md")).unwrap(),
        original.content
    );
    fs::rename(
        source.join("alpha/SKILL.md"),
        source.join("alpha/restored.off"),
    )
    .unwrap();
    let missing = library.detail(&id, false).unwrap();
    assert_eq!(missing.revision, "missing");
    let external = skill("External owner", "must survive missing-revision conflict");
    fs::write(source.join("alpha/SKILL.md"), &external).unwrap();
    assert!(library.save_content(&id, "missing", &edited).is_err());
    assert_eq!(
        fs::read_to_string(source.join("alpha/SKILL.md")).unwrap(),
        external
    );
    fs::rename(
        source.join("alpha/SKILL.md"),
        source.join("alpha/external.off"),
    )
    .unwrap();
    library.save_content(&id, "missing", &edited).unwrap();
    assert_eq!(
        fs::read_to_string(source.join("alpha/SKILL.md")).unwrap(),
        edited
    );

    fs::rename(source.join("alpha"), source.join("alpha-away")).unwrap();
    assert_eq!(library.detail(&id, false).unwrap().revision, "missing");
    assert!(
        library
            .save_content(&id, "missing", &original.content)
            .is_err()
    );
    assert!(
        !source.join("alpha").exists(),
        "Recovery must not recreate a missing parent directory"
    );
}

#[cfg(unix)]
#[test]
fn unreadable_subtree_keeps_readable_siblings_and_blocks_its_published_rows() {
    use std::os::unix::fs::PermissionsExt;
    let (_base, library, source) = fixture();
    let alpha = id(&library);
    publish(&library, &alpha);
    let restricted = library
        .create_skill(
            "local",
            "restricted/group/child",
            &skill("Restricted", "private"),
        )
        .unwrap();
    publish(&library, &restricted);
    let path = source.join("restricted");
    struct Restore(PathBuf);
    impl Drop for Restore {
        fn drop(&mut self) {
            let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o700));
        }
    }
    let _restore = Restore(path.clone());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read_dir(&path).is_ok() {
        return;
    } // root 実行では EACCES を再現できない。
    let report = library.scan().unwrap();
    assert!(report.errors > 0);
    assert!(library.detail(&alpha, true).is_ok());
    let all = library.search(Default::default(), false).unwrap();
    let blocked = all
        .skills
        .iter()
        .find(|skill| skill.id == restricted)
        .unwrap();
    assert!(!blocked.missing && blocked.error.is_some());
    let published = library.search(Default::default(), true).unwrap();
    assert_eq!(
        published
            .skills
            .iter()
            .map(|skill| skill.id.as_str())
            .collect::<Vec<_>>(),
        vec![alpha.as_str()]
    );
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    library.scan().unwrap();
    assert!(library.detail(&restricted, true).is_ok());
}

#[test]
fn new_skills_are_published_by_default_except_unreadable_and_user_unpublished_ones() {
    let (base, library, source) = fixture();
    // fixtureは既存のalphaを明示的に未公開へ戻してあり、再スキャンで公開へ戻らないことも併せて確認する。
    let alpha = id(&library);
    assert!(!library.detail(&alpha, false).unwrap().summary.published);
    // 新規の正常なスキルは走査で公開状態になる。
    fs::create_dir_all(source.join("fresh")).unwrap();
    fs::write(source.join("fresh/SKILL.md"), skill("fresh", "body")).unwrap();
    // 不正なfrontmatterのスキルは読み取りエラーのため、新規でも未公開のまま。
    fs::create_dir_all(source.join("broken")).unwrap();
    fs::write(
        source.join("broken/SKILL.md"),
        "---
name: [broken
---
invalid",
    )
    .unwrap();
    library.scan().unwrap();
    let find = |name: &str| {
        library
            .search(
                SearchQuery {
                    q: name.into(),
                    ..Default::default()
                },
                false,
            )
            .unwrap()
            .skills
    };
    let fresh = find("fresh");
    assert_eq!(fresh.len(), 1);
    assert!(fresh[0].published && fresh[0].error.is_none());
    let broken: Vec<_> = library
        .search(SearchQuery::default(), false)
        .unwrap()
        .skills
        .into_iter()
        .filter(|skill| skill.relative_path == "broken/SKILL.md")
        .collect();
    assert_eq!(broken.len(), 1);
    assert!(broken[0].error.is_some() && !broken[0].published);
    // 公開検索にも読み取りエラーのスキルは現れない。
    let visible = library.search(SearchQuery::default(), true).unwrap();
    assert_eq!(visible.total, 1);
    assert_eq!(visible.skills[0].id, fresh[0].id);
    // 利用者が未公開にした既存スキルは、再スキャンしても公開へ戻らない。
    library.set_published(&fresh[0].id, false).unwrap();
    library.scan().unwrap();
    library.scan().unwrap();
    assert!(
        !library
            .detail(&fresh[0].id, false)
            .unwrap()
            .summary
            .published
    );
    assert!(!library.detail(&alpha, false).unwrap().summary.published);
    // 内容更新を伴う再スキャンでも公開状態は変わらない。
    fs::write(source.join("fresh/SKILL.md"), skill("fresh", "edited")).unwrap();
    library.scan().unwrap();
    assert!(
        !library
            .detail(&fresh[0].id, false)
            .unwrap()
            .summary
            .published
    );
    // 閉じて開き直しても保たれる。
    drop(library);
    let reopened = Library::open(&base.join("data")).unwrap();
    assert_eq!(
        reopened.search(SearchQuery::default(), true).unwrap().total,
        0
    );
}

#[cfg(unix)]
#[test]
fn unix_permissions_and_distinct_backslash_paths() {
    use std::os::unix::fs::PermissionsExt;
    let (base, library, source) = fixture();
    assert_eq!(
        fs::metadata(base.join("data/library.sqlite3"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let id = id(&library);
    let path = source.join("alpha/SKILL.md");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let before = library.detail(&id, false).unwrap();
    library
        .save_content(&id, &before.revision, &skill("日本語検索", "private edit"))
        .unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );

    let backslash = library
        .create_skill(
            "local",
            "a\\b",
            &skill("Backslash folder", "literal backslash"),
        )
        .unwrap();
    let nested = library
        .create_skill("local", "a/b", &skill("Nested folders", "slash separators"))
        .unwrap();
    assert_ne!(backslash, nested);
    library.scan().unwrap();
    let literal = library.detail(&backslash, false).unwrap();
    let hierarchical = library.detail(&nested, false).unwrap();
    assert_eq!(literal.summary.relative_path, "a\\b/SKILL.md");
    assert_eq!(hierarchical.summary.relative_path, "a/b/SKILL.md");
    assert!(literal.content.contains("literal backslash"));
    assert!(hierarchical.content.contains("slash separators"));
    assert!(source.join("a\\b/SKILL.md").is_file());
    assert!(source.join("a/b/SKILL.md").is_file());
}
