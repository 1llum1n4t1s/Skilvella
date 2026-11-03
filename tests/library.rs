use skilvella::{
    library::Library,
    model::{SearchQuery, SkillMetadata},
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
    (base, library, source)
}
fn skill(name: &str, body: &str) -> String {
    format!(
        "---\nname: {name}\ndescription: Description for a skill\ncustom: preserved\n---\n{body}\n"
    )
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

#[test]
fn publication_metadata_history_and_restore_survive_rescan_and_reopen() {
    let (base, library, source) = fixture();
    assert_eq!(library.sources().unwrap()[0].display_name, "local");
    library
        .set_source_display_name("local", "仕事用 🌸")
        .unwrap();
    let id = id(&library);
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
    assert!(!library.detail(&second, false).unwrap().summary.published);
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
