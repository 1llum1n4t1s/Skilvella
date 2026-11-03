use skilvella::{
    library::Library,
    model::{SearchQuery, SkillMetadata},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn base() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".work/test-directory")
        .join(uuid::Uuid::new_v4().to_string());
    fs::create_dir_all(&root).unwrap();
    root
}
fn skill(root: &Path, name: &str) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Directory test\n---\n{name}\n"),
    )
    .unwrap();
}
fn all(l: &Library) -> Vec<skilvella::model::SkillSummary> {
    l.search(SearchQuery::default(), false).unwrap().skills
}
fn publish(l: &Library, id: &str) {
    l.set_metadata(
        id,
        SkillMetadata {
            tags: vec![],
            category: String::new(),
            published: true,
        },
    )
    .unwrap();
}

#[test]
fn detects_preferred_home_and_recurses_below_skill_markers() {
    let b = base();
    let home = b.join("home");
    let root = home.join(".agents/skills");
    skill(&root.join("parent"), "Parent");
    skill(&root.join("parent/a/b/c/d/child"), "Child");
    skill(&home.join(".codex/skills/ignored"), "Ignored");
    skill(&b.join("codex/skills/ignored"), "Ignored");
    let l = Library::open(&b.join("data")).unwrap();
    assert!(l.sources().unwrap().is_empty());
    l.initialize_directory(&home, Some(&b.join("codex")))
        .unwrap();
    let settings = l.directory_settings().unwrap();
    assert_eq!(settings.mode, "auto");
    assert_eq!(Path::new(&settings.path), root.canonicalize().unwrap());
    assert_eq!(all(&l).len(), 2);
    assert!(all(&l).iter().all(|s| !s.published));
}

#[test]
fn detects_codex_home_then_legacy_home_and_creates_separate_fallback() {
    let b = base();
    let home = b.join("home");
    skill(&home.join(".codex/skills/old"), "Legacy");
    skill(&b.join("codex/skills/custom"), "Environment");
    let l = Library::open(&b.join("data")).unwrap();
    l.initialize_directory(&home, Some(&b.join("codex")))
        .unwrap();
    assert_eq!(all(&l)[0].name, "Environment");
    let legacy = Library::open(&b.join("legacy-data")).unwrap();
    legacy.initialize_directory(&home, None).unwrap();
    assert_eq!(all(&legacy)[0].name, "Legacy");
    let fallback = Library::open(&b.join("empty-data")).unwrap();
    fallback
        .initialize_directory(&b.join("empty-home"), None)
        .unwrap();
    assert_eq!(
        Path::new(&fallback.directory_settings().unwrap().path),
        b.join("empty-data-skills").canonicalize().unwrap()
    );
    assert!(all(&fallback).is_empty());
}

#[test]
fn switching_is_persistent_atomic_and_keeps_root_history_separate() {
    let b = base();
    let home = b.join("home");
    let original = home.join(".agents/skills");
    skill(&original.join("same"), "Original");
    skill(&b.join("custom/same"), "Custom");
    skill(&b.join("manual/extra"), "Manual");
    let l = Library::open(&b.join("data")).unwrap();
    l.initialize_directory(&home, None).unwrap();
    l.add_source("manual", &b.join("manual")).unwrap();
    let original_alias = l.directory_settings().unwrap().source_alias;
    l.set_source_display_name(&original_alias, "標準スキル")
        .unwrap();
    let original_id = all(&l)
        .into_iter()
        .find(|s| s.name == "Original")
        .unwrap()
        .id;
    publish(&l, &original_id);
    let detail = l.detail(&original_id, false).unwrap();
    l.save_content(
        &original_id,
        &detail.revision,
        &detail.content.replace("Original", "Updated"),
    )
    .unwrap();
    l.set_directory("custom", b.join("custom").to_str().unwrap())
        .unwrap();
    let custom = all(&l).into_iter().find(|s| s.name == "Custom").unwrap();
    assert_ne!(custom.id, original_id);
    assert!(!custom.published);
    assert!(l.detail(&custom.id, false).unwrap().history.is_empty());
    assert_eq!(l.sources().unwrap().len(), 2);
    let before = l.directory_settings().unwrap();
    for invalid in [b.join("absent"), b.join("data")] {
        assert!(
            l.set_directory("custom", invalid.to_str().unwrap())
                .is_err()
        );
        assert_eq!(
            l.directory_settings().unwrap().source_alias,
            before.source_alias
        );
        assert_eq!(l.sources().unwrap().len(), 2);
    }
    assert!(l.set_directory("custom", "relative").is_err());
    let reopened = Library::open(&b.join("data")).unwrap();
    reopened.initialize_directory(&home, None).unwrap();
    assert_eq!(reopened.directory_settings().unwrap().mode, "custom");
    reopened.set_directory("auto", "").unwrap();
    let restored = reopened.detail(&original_id, false).unwrap();
    assert_eq!(
        reopened
            .sources()
            .unwrap()
            .iter()
            .find(|s| s.alias == original_alias)
            .unwrap()
            .display_name,
        "標準スキル"
    );
    assert!(restored.summary.published);
    assert!(!restored.history.is_empty());
    assert_eq!(all(&reopened).len(), 2);
}

#[test]
fn legacy_registration_is_respected_and_removing_managed_source_stays_disabled() {
    let b = base();
    skill(&b.join("manual/one"), "Manual");
    skill(&b.join("home/.agents/skills/two"), "Detected");
    let l = Library::open(&b.join("data")).unwrap();
    l.add_source("mine", &b.join("manual")).unwrap();
    l.initialize_directory(&b.join("home"), None).unwrap();
    assert_eq!(l.directory_settings().unwrap().mode, "custom");
    assert_eq!(l.sources().unwrap().len(), 1);
    l.remove_source("mine").unwrap();
    let reopened = Library::open(&b.join("data")).unwrap();
    reopened
        .initialize_directory(&b.join("home"), None)
        .unwrap();
    assert_eq!(reopened.directory_settings().unwrap().mode, "disabled");
    assert!(reopened.sources().unwrap().is_empty());
}

#[test]
fn parent_publication_does_not_expose_nested_skill_files_even_when_invalid() {
    let b = base();
    let source = b.join("source");
    skill(&source.join("parent"), "Parent");
    skill(&source.join("parent/deep/child"), "Child");
    fs::write(
        source.join("parent/deep/child/reference.txt"),
        "secret child reference",
    )
    .unwrap();
    let l = Library::open(&b.join("data")).unwrap();
    l.add_source("local", &source).unwrap();
    let parent = all(&l).into_iter().find(|s| s.name == "Parent").unwrap();
    publish(&l, &parent.id);
    for malformed in [false, true] {
        if malformed {
            fs::write(
                source.join("parent/deep/child/SKILL.md"),
                "invalid frontmatter",
            )
            .unwrap();
        }
        let detail = l.detail(&parent.id, true).unwrap();
        assert!(!detail.files.iter().any(|f| f.contains("child/")));
        assert!(
            l.read_file(&parent.id, "deep/child/SKILL.md", 1, 100)
                .is_err()
        );
        assert!(
            l.read_file(&parent.id, "deep/child/reference.txt", 1, 100)
                .is_err()
        );
    }
}

#[test]
fn transaction_failure_preserves_previous_settings_and_active_source() {
    let b = base();
    skill(&b.join("home/.agents/skills/one"), "Original");
    skill(&b.join("custom/two"), "Custom");
    let l = Library::open(&b.join("data")).unwrap();
    l.initialize_directory(&b.join("home"), None).unwrap();
    let before = l.directory_settings().unwrap();
    let db = rusqlite::Connection::open(b.join("data/library.sqlite3")).unwrap();
    db.execute_batch("CREATE TRIGGER reject_directory_update BEFORE UPDATE ON settings WHEN NEW.key='directory' BEGIN SELECT RAISE(ABORT,'test rejection'); END;").unwrap();
    assert!(
        l.set_directory("custom", b.join("custom").to_str().unwrap())
            .is_err()
    );
    assert_eq!(
        l.directory_settings().unwrap().source_alias,
        before.source_alias
    );
    assert_eq!(l.directory_settings().unwrap().mode, before.mode);
    assert_eq!(l.sources().unwrap().len(), 1);
    assert_eq!(l.sources().unwrap()[0].alias, before.source_alias);
    assert_eq!(all(&l)[0].name, "Original");
}

#[test]
fn disabled_initialization_still_refreshes_other_manual_sources() {
    let b = base();
    let l = Library::open(&b.join("data")).unwrap();
    l.initialize_directory(&b.join("home"), None).unwrap();
    skill(&b.join("manual/one"), "Manual");
    l.add_source("manual", &b.join("manual")).unwrap();
    l.set_directory("disabled", "").unwrap();
    skill(&b.join("manual/two"), "New after shutdown");
    let reopened = Library::open(&b.join("data")).unwrap();
    reopened
        .initialize_directory(&b.join("home"), None)
        .unwrap();
    assert_eq!(reopened.directory_settings().unwrap().mode, "disabled");
    assert_eq!(all(&reopened).len(), 2);
    assert_eq!(reopened.sources().unwrap()[0].alias, "manual");
}

#[test]
fn selecting_a_manual_source_does_not_transfer_ownership() {
    let b = base();
    let l = Library::open(&b.join("data")).unwrap();
    l.initialize_directory(&b.join("home"), None).unwrap();
    skill(&b.join("manual/one"), "Manual");
    skill(&b.join("custom/two"), "Custom");
    l.add_source("manual", &b.join("manual")).unwrap();
    l.set_directory("custom", b.join("manual").to_str().unwrap())
        .unwrap();
    l.set_directory("custom", b.join("custom").to_str().unwrap())
        .unwrap();
    assert!(l.sources().unwrap().iter().any(|s| s.alias == "manual"));
    l.set_directory("custom", b.join("manual").to_str().unwrap())
        .unwrap();
    l.set_directory("disabled", "").unwrap();
    assert_eq!(l.sources().unwrap().len(), 1);
    assert_eq!(l.sources().unwrap()[0].alias, "manual");
    l.set_directory("custom", b.join("manual").to_str().unwrap())
        .unwrap();
    l.remove_source("manual").unwrap();
    assert_eq!(l.directory_settings().unwrap().mode, "disabled");
    assert!(l.sources().unwrap().is_empty());
}

#[test]
fn legacy_manual_and_old_settings_without_ownership_are_preserved() {
    let b = base();
    skill(&b.join("manual/one"), "Manual");
    let l = Library::open(&b.join("data")).unwrap();
    l.add_source("mine", &b.join("manual")).unwrap();
    l.initialize_directory(&b.join("home"), None).unwrap();
    // 以前のJSON形式は追加フィールドなしで読み込める。
    let db = rusqlite::Connection::open(b.join("data/library.sqlite3")).unwrap();
    db.execute(
        "DELETE FROM settings WHERE key LIKE 'directory_owned:%'",
        [],
    )
    .unwrap();
    l.set_directory("auto", "").unwrap();
    assert!(l.sources().unwrap().iter().any(|s| s.alias == "mine"));
}

#[test]
fn persisted_and_legacy_offline_directories_do_not_prevent_startup() {
    for legacy in [false, true] {
        let b = base();
        let root = b.join("custom");
        skill(&root.join("one"), "Offline");
        let l = Library::open(&b.join("data")).unwrap();
        if legacy {
            l.add_source("legacy", &root).unwrap();
        } else {
            l.initialize_directory(&b.join("home"), None).unwrap();
            l.set_directory("custom", root.to_str().unwrap()).unwrap();
        }
        let id = all(&l)[0].id.clone();
        publish(&l, &id);
        let detail = l.detail(&id, false).unwrap();
        l.save_content(
            &id,
            &detail.revision,
            &detail.content.replace("Offline", "Edited offline"),
        )
        .unwrap();
        fs::rename(&root, b.join("offline-retained")).unwrap();
        let reopened = Library::open(&b.join("data")).unwrap();
        reopened
            .initialize_directory(&b.join("home"), None)
            .unwrap();
        assert_eq!(reopened.directory_settings().unwrap().mode, "custom");
        assert!(all(&reopened)[0].missing);
        assert!(
            reopened
                .search(SearchQuery::default(), true)
                .unwrap()
                .skills
                .is_empty()
        );
        let detail = reopened.detail(&id, false).unwrap();
        assert!(!detail.history.is_empty());
        assert!(detail.summary.published);
        reopened.set_directory("auto", "").unwrap();
    }
}

#[test]
fn active_manual_alias_is_preferred_over_inactive_same_root_history() {
    let b = base();
    let home = b.join("home");
    let root = home.join(".agents/skills");
    skill(&root.join("one"), "Original");
    skill(&b.join("custom/two"), "Custom");
    let l = Library::open(&b.join("data")).unwrap();
    l.initialize_directory(&home, None).unwrap();
    let old = l.directory_settings().unwrap().source_alias;
    l.set_directory("custom", b.join("custom").to_str().unwrap())
        .unwrap();
    l.add_source("aaa", &root).unwrap();
    l.set_directory("auto", "").unwrap();
    assert_eq!(l.directory_settings().unwrap().source_alias, "aaa");
    let sources = l.sources().unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].alias, "aaa");
    assert!(!sources.iter().any(|s| s.alias == old));
    l.set_directory("disabled", "").unwrap();
    assert_eq!(l.sources().unwrap()[0].alias, "aaa");
}

#[test]
fn known_nested_skill_remains_a_boundary_when_its_marker_is_missing() {
    let b = base();
    let root = b.join("source");
    skill(&root.join("parent"), "Parent");
    skill(&root.join("parent/child"), "Private child");
    fs::write(root.join("parent/child/reference.txt"), "private reference").unwrap();
    let l = Library::open(&b.join("data")).unwrap();
    l.add_source("local", &root).unwrap();
    let parent = all(&l).into_iter().find(|s| s.name == "Parent").unwrap();
    publish(&l, &parent.id);
    fs::rename(
        root.join("parent/child/SKILL.md"),
        root.join("parent/child/SKILL.md.offline"),
    )
    .unwrap();
    for scanned in [false, true] {
        if scanned {
            l.scan().unwrap();
        }
        let detail = l.detail(&parent.id, true).unwrap();
        assert!(!detail.files.iter().any(|f| f.starts_with("child/")));
        assert!(
            l.read_file(&parent.id, "child/reference.txt", 1, 100)
                .is_err()
        );
        assert!(
            l.read_file(&parent.id, "child/SKILL.md.offline", 1, 100)
                .is_err()
        );
    }
}

#[test]
fn legacy_inactive_sources_are_not_reactivated_on_initialization() {
    let b = base();
    let home = b.join("home");
    let root = home.join(".agents/skills");
    skill(&root.join("one"), "Previously public");
    let l = Library::open(&b.join("data")).unwrap();
    l.add_source("old", &root).unwrap();
    publish(&l, &all(&l)[0].id);
    l.remove_source("old").unwrap();
    let reopened = Library::open(&b.join("data")).unwrap();
    reopened.initialize_directory(&home, None).unwrap();
    assert_eq!(reopened.directory_settings().unwrap().mode, "disabled");
    assert!(reopened.sources().unwrap().is_empty());
    assert!(
        reopened
            .search(SearchQuery::default(), true)
            .unwrap()
            .skills
            .is_empty()
    );
}

#[test]
fn failed_manual_registration_rolls_back_managed_source_ownership() {
    let b = base();
    let home = b.join("home");
    let root = home.join(".agents/skills");
    skill(&root.join("one"), "Original");
    let l = Library::open(&b.join("data")).unwrap();
    l.initialize_directory(&home, None).unwrap();
    let alias = l.directory_settings().unwrap().source_alias;
    let db = rusqlite::Connection::open(b.join("data/library.sqlite3")).unwrap();
    db.execute_batch("CREATE TRIGGER reject_scan BEFORE UPDATE ON settings WHEN NEW.key='indexed_at' BEGIN SELECT RAISE(ABORT,'test rejection'); END;").unwrap();
    assert!(l.add_source(&alias, &root).is_err());
    db.execute_batch("DROP TRIGGER reject_scan").unwrap();
    l.set_directory("disabled", "").unwrap();
    assert!(
        l.sources().unwrap().is_empty(),
        "A failed manual registration must not transfer ownership"
    );
}

#[test]
fn auto_detection_switches_from_offline_old_root_to_available_preferred_root() {
    let b = base();
    let home = b.join("home");
    let old_root = home.join(".codex/skills");
    skill(&old_root.join("one"), "Old root");
    let l = Library::open(&b.join("data")).unwrap();
    l.initialize_directory(&home, None).unwrap();
    let old_alias = l.directory_settings().unwrap().source_alias;
    let old_id = all(&l)[0].id.clone();
    publish(&l, &old_id);
    fs::rename(&old_root, home.join(".codex/skills-retained")).unwrap();
    let reopened = Library::open(&b.join("data")).unwrap();
    reopened.initialize_directory(&home, None).unwrap();
    assert_eq!(
        reopened.directory_settings().unwrap().source_alias,
        old_alias
    );
    assert!(all(&reopened)[0].missing);
    let preferred = home.join(".agents/skills");
    skill(&preferred.join("one"), "Preferred root");
    reopened.initialize_directory(&home, None).unwrap();
    let current = reopened.directory_settings().unwrap();
    assert_eq!(Path::new(&current.path), preferred.canonicalize().unwrap());
    assert_ne!(current.source_alias, old_alias);
    assert_eq!(all(&reopened)[0].name, "Preferred root");
    assert!(!all(&reopened)[0].published);
    fs::rename(home.join(".codex/skills-retained"), &old_root).unwrap();
    reopened
        .set_directory("custom", old_root.to_str().unwrap())
        .unwrap();
    assert_eq!(
        reopened.directory_settings().unwrap().source_alias,
        old_alias
    );
    assert!(reopened.detail(&old_id, false).unwrap().summary.published);
}

#[test]
fn startup_auto_selection_conflict_keeps_previous_settings_and_scans_manual_source() {
    let b = base();
    let home = b.join("home");
    let old_root = home.join(".codex/skills");
    skill(&old_root.join("one"), "Old automatic root");
    fs::create_dir_all(home.join(".agents")).unwrap();
    let l = Library::open(&b.join("data")).unwrap();
    l.initialize_directory(&home, None).unwrap();
    let before = l.directory_settings().unwrap();
    l.add_source("manual", &home.join(".agents")).unwrap();
    fs::rename(&old_root, home.join(".codex/skills-retained")).unwrap();
    let preferred = home.join(".agents/skills");
    skill(&preferred.join("new"), "New manual child");
    let reopened = Library::open(&b.join("data")).unwrap();
    reopened.initialize_directory(&home, None).unwrap();
    let after = reopened.directory_settings().unwrap();
    assert_eq!(after.mode, before.mode);
    assert_eq!(after.path, before.path);
    assert_eq!(after.source_alias, before.source_alias);
    assert_eq!(
        Path::new(&after.detected_path).canonicalize().unwrap(),
        preferred.canonicalize().unwrap()
    );
    assert_eq!(reopened.sources().unwrap().len(), 2);
    assert!(
        reopened
            .sources()
            .unwrap()
            .iter()
            .any(|s| s.alias == "manual")
    );
    assert!(
        all(&reopened)
            .iter()
            .any(|s| s.name == "New manual child" && s.source == "manual")
    );
    assert!(
        reopened.set_directory("auto", "").is_err(),
        "Explicit changes still reject overlapping sources"
    );
    reopened.set_directory("disabled", "").unwrap();
    assert_eq!(
        reopened.sources().unwrap().len(),
        1,
        "The retained automatic source must retain its ownership"
    );
    assert_eq!(reopened.sources().unwrap()[0].alias, "manual");
}
