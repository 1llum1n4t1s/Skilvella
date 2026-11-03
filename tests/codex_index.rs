use skilvella::{
    library::Library,
    model::{SearchQuery, SkillMetadata},
};
use std::{fs, path::PathBuf};

fn setup() -> (PathBuf, Library, String) {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".work/test-codex-index")
        .join(uuid::Uuid::new_v4().to_string());
    fs::create_dir_all(base.join("source/one")).unwrap();
    fs::write(
        base.join("source/one/SKILL.md"),
        "---\nname: original\ndescription: A useful skill\n---\nPRIVATE ORIGINAL BODY\n",
    )
    .unwrap();
    let library = Library::open_for_user(&base.join("data"), &base.join("home"), None).unwrap();
    library.add_source("source", &base.join("source")).unwrap();
    library.scan().unwrap();
    let id = library
        .search(SearchQuery::default(), false)
        .unwrap()
        .skills[0]
        .id
        .clone();
    (base, library, id)
}
fn publish(library: &Library, id: &str, published: bool) {
    library
        .set_metadata(
            id,
            SkillMetadata {
                tags: vec![],
                category: String::new(),
                published,
            },
        )
        .unwrap();
    library.sync_codex_entries().unwrap();
}
fn entry(base: &std::path::Path, _id: &str) -> PathBuf {
    base.join("home/.agents/skills/skilvella").join("original")
}

#[test]
fn explicit_automatic_search_only_and_publication_preserve_the_original() {
    let (base, library, id) = setup();
    let original = fs::read(base.join("source/one/SKILL.md")).unwrap();
    let revision = library.detail(&id, false).unwrap().revision;
    assert!(!library.codex_settings().unwrap().enabled);
    assert!(!library.codex_policy(&id).unwrap().command);
    library.set_codex_enabled(true).unwrap();
    assert!(
        base.join("home/.agents/skills/skilvella/skilvella-search/SKILL.md")
            .is_file()
    );
    library.set_codex_policy(&id, false, true).unwrap();
    assert!(!entry(&base, &id).exists());
    publish(&library, &id, true);
    let wrapper = fs::read_to_string(entry(&base, &id).join("SKILL.md")).unwrap();
    assert!(wrapper.contains(&format!("load_skill with the fixed id \"{id}\"")));
    assert!(!wrapper.contains("PRIVATE ORIGINAL BODY"));
    assert!(
        fs::read_to_string(entry(&base, &id).join("agents/openai.yaml"))
            .unwrap()
            .contains("false")
    );
    library.set_codex_policy(&id, true, true).unwrap();
    assert!(
        fs::read_to_string(entry(&base, &id).join("agents/openai.yaml"))
            .unwrap()
            .contains("true")
    );
    assert!(library.set_codex_policy(&id, true, false).is_err());
    assert_eq!(library.codex_settings().unwrap().exported, 1);
    library.set_codex_policy(&id, false, false).unwrap();
    assert!(!entry(&base, &id).exists());
    library.set_codex_policy(&id, false, true).unwrap();
    publish(&library, &id, false);
    assert!(!entry(&base, &id).exists());
    publish(&library, &id, true);
    library.set_codex_enabled(false).unwrap();
    assert!(!base.join("home/.agents/skills/skilvella").exists());
    assert_eq!(library.codex_settings().unwrap().exported, 0);
    assert_eq!(
        fs::read(base.join("source/one/SKILL.md")).unwrap(),
        original
    );
    assert_eq!(library.detail(&id, false).unwrap().revision, revision);
    assert!(library.detail(&id, false).unwrap().history.is_empty());
    assert!(library.detail(&id, true).is_ok());
    let reopened = Library::open(&base.join("data")).unwrap();
    assert!(reopened.codex_policy(&id).unwrap().command);
    assert!(!reopened.codex_settings().unwrap().enabled);
}

#[test]
fn user_edits_unknown_files_and_other_libraries_are_preserved() {
    let (base, library, id) = setup();
    publish(&library, &id, true);
    library.set_codex_policy(&id, false, true).unwrap();
    library.set_codex_enabled(true).unwrap();
    let path = entry(&base, &id).join("SKILL.md");
    fs::write(&path, "user edit").unwrap();
    library.set_codex_enabled(false).unwrap();
    assert!(!library.codex_settings().unwrap().last_error.is_empty());
    assert_eq!(fs::read_to_string(&path).unwrap(), "user edit");

    let (base, library, id) = setup();
    library.set_codex_enabled(true).unwrap();
    let unknown = base.join("home/.agents/skills/skilvella/notes.txt");
    fs::write(&unknown, "personal notes").unwrap();
    assert!(library.sync_codex_entries().is_err());
    assert_eq!(fs::read_to_string(unknown).unwrap(), "personal notes");
    let other = Library::open_for_user(&base.join("other-data"), &base.join("home"), None).unwrap();
    other.set_codex_enabled(true).unwrap();
    assert!(
        other
            .codex_settings()
            .unwrap()
            .last_error
            .contains("not owned")
    );
    assert!(!library.codex_policy(&id).unwrap().command);
}

#[test]
fn invalid_missing_and_disconnected_sources_have_distinct_outcomes() {
    let (base, library, id) = setup();
    publish(&library, &id, true);
    library.set_codex_policy(&id, false, true).unwrap();
    library.set_codex_enabled(true).unwrap();
    fs::rename(base.join("source"), base.join("offline-source")).unwrap();
    assert!(library.sync_codex_entries().is_err());
    assert!(entry(&base, &id).is_dir());
    fs::rename(base.join("offline-source"), base.join("source")).unwrap();
    fs::write(base.join("source/one/SKILL.md"), "invalid frontmatter").unwrap();
    library.sync_codex_entries().unwrap();
    assert!(!entry(&base, &id).exists());
    assert!(library.detail(&id, true).is_err());
    assert!(library.codex_settings().unwrap().last_error.is_empty());
    library.scan().unwrap();
    fs::rename(
        base.join("source/one/SKILL.md"),
        base.join("source/one/removed.md"),
    )
    .unwrap();
    library.sync_codex_entries().unwrap();
    assert!(!entry(&base, &id).exists());
}

#[test]
fn absent_native_context_is_safe_until_enabled() {
    let (base, _, _) = setup();
    let library = Library::open(&base.join("without-context")).unwrap();
    library.sync_codex_entries().unwrap();
    library.set_codex_enabled(true).unwrap();
    assert!(
        library
            .codex_settings()
            .unwrap()
            .last_error
            .contains("not configured")
    );
}

#[test]
fn export_and_retirement_never_write_inside_managed_sources() {
    for relative in ["home/.agents/skills", "home/.agents"] {
        let (base, library, _) = setup();
        fs::create_dir_all(base.join(relative)).unwrap();
        library.add_source("native", &base.join(relative)).unwrap();
        library.set_codex_enabled(true).unwrap();
        assert!(
            library
                .codex_settings()
                .unwrap()
                .last_error
                .contains("overlaps")
        );
        assert!(!base.join("home/.agents/skills/skilvella").exists());
        assert!(!base.join("home/.agents/skilvella-staging").exists());
        assert!(!base.join("home/.agents/skilvella-retired").exists());
    }
}

#[test]
fn exports_every_selected_skill_beyond_the_search_page_size() {
    let (base, library, _) = setup();
    for n in 0..103 {
        let directory = base.join(format!("source/extra-{n}"));
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("SKILL.md"),
            format!("---\nname: extra-{n}\ndescription: Export pagination\n---\nBody\n"),
        )
        .unwrap();
    }
    library.scan().unwrap();
    let mut skills = library
        .search(
            SearchQuery {
                limit: 100,
                ..Default::default()
            },
            false,
        )
        .unwrap()
        .skills;
    skills.extend(
        library
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
    assert_eq!(skills.len(), 104);
    for skill in &skills {
        publish(&library, &skill.id, true);
        library.set_codex_policy(&skill.id, false, true).unwrap();
    }
    library.set_codex_enabled(true).unwrap();
    assert_eq!(library.codex_settings().unwrap().exported, 104);
    for skill in skills {
        assert!(
            base.join("home/.agents/skills/skilvella")
                .join(&skill.name)
                .join("SKILL.md")
                .is_file()
        );
    }
}

#[cfg(windows)]
#[test]
fn junction_export_destinations_are_rejected() {
    let (base, library, _) = setup();
    fs::create_dir_all(base.join("home/.agents/skills")).unwrap();
    fs::create_dir_all(base.join("external")).unwrap();
    let status = std::process::Command::new("cmd.exe")
        .args(["/C", "mklink", "/J"])
        .arg(
            base.join("home/.agents/skills/skilvella")
                .to_string_lossy()
                .replace('/', "\\"),
        )
        .arg(base.join("external").to_string_lossy().replace('/', "\\"))
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    library.set_codex_enabled(true).unwrap();
    assert!(
        library
            .codex_settings()
            .unwrap()
            .last_error
            .contains("forbidden")
    );
    assert!(
        fs::read_dir(base.join("external"))
            .unwrap()
            .next()
            .is_none()
    );
}

#[test]
fn staging_failure_keeps_the_previous_index_and_recovers_on_retry() {
    let (base, library, id) = setup();
    publish(&library, &id, true);
    library.set_codex_policy(&id, false, true).unwrap();
    library.set_codex_enabled(true).unwrap();
    let original = fs::read(entry(&base, &id).join("agents/openai.yaml")).unwrap();
    let staging = base.join("home/.agents/skilvella-staging");
    fs::rename(&staging, base.join("saved-staging")).unwrap();
    fs::write(&staging, "block staging creation").unwrap();
    library.set_codex_policy(&id, true, true).unwrap();
    assert!(!library.codex_settings().unwrap().last_error.is_empty());
    assert_eq!(
        fs::read(entry(&base, &id).join("agents/openai.yaml")).unwrap(),
        original
    );
    assert!(!base.join("data/codex-index/pending.json").exists());
    fs::rename(&staging, base.join("saved-blocker")).unwrap();
    fs::rename(base.join("saved-staging"), &staging).unwrap();
    library.sync_codex_entries().unwrap();
    assert!(library.codex_settings().unwrap().last_error.is_empty());
    assert!(
        fs::read_to_string(entry(&base, &id).join("agents/openai.yaml"))
            .unwrap()
            .contains("true")
    );
}

#[test]
fn completed_generations_are_bounded_and_edited_backups_stop_cleanup() {
    let (base, library, id) = setup();
    publish(&library, &id, true);
    library.set_codex_policy(&id, false, true).unwrap();
    library.set_codex_enabled(true).unwrap();
    for automatic in [true, false, true, false] {
        library.set_codex_policy(&id, automatic, true).unwrap();
        assert!(library.codex_settings().unwrap().last_error.is_empty());
        assert_eq!(
            fs::read_dir(base.join("home/.agents/skilvella-retired"))
                .unwrap()
                .count(),
            1
        );
    }
    assert!(base.join("data/codex-index/last-completed.json").is_file());
    assert!(
        !fs::read_dir(base.join("data/codex-index"))
            .unwrap()
            .any(|e| e
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("completed-"))
    );
    let backup = fs::read_dir(base.join("home/.agents/skilvella-retired"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    fs::write(backup.join("original/SKILL.md"), "edited backup").unwrap();
    let current = fs::read(entry(&base, &id).join("agents/openai.yaml")).unwrap();
    library.set_codex_policy(&id, true, true).unwrap();
    assert!(!library.codex_settings().unwrap().last_error.is_empty());
    assert_eq!(
        fs::read(entry(&base, &id).join("agents/openai.yaml")).unwrap(),
        current
    );
    assert_eq!(
        fs::read_to_string(backup.join("original/SKILL.md")).unwrap(),
        "edited backup"
    );
}

#[test]
fn disconnected_sources_do_not_prevent_disabling_the_index() {
    let (base, library, id) = setup();
    publish(&library, &id, true);
    library.set_codex_policy(&id, false, true).unwrap();
    library.set_codex_enabled(true).unwrap();
    fs::rename(base.join("source"), base.join("disconnected")).unwrap();
    library.set_codex_enabled(false).unwrap();
    assert!(library.codex_settings().unwrap().last_error.is_empty());
    assert!(!base.join("home/.agents/skills/skilvella").exists());
}

#[test]
fn partial_staging_files_stop_safely_with_their_absolute_path() {
    let (base, library, id) = setup();
    publish(&library, &id, true);
    library.set_codex_policy(&id, false, true).unwrap();
    library.set_codex_enabled(true).unwrap();
    library.sync_codex_entries().unwrap();
    let state = base.join("data/codex-index");
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(state.join("manifest.json")).unwrap()).unwrap();
    let root = PathBuf::from(manifest["root"].as_str().unwrap());
    let partial = root
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("skilvella-staging")
        .join(uuid::Uuid::new_v4().to_string());
    fs::create_dir_all(partial.join("original")).unwrap();
    fs::write(partial.join("original/SKILL.md"), "partial").unwrap();
    fs::write(
        state.join("staged.json"),
        serde_json::to_vec(&serde_json::json!({"manifest": manifest, "path": partial})).unwrap(),
    )
    .unwrap();
    for _ in 0..2 {
        assert!(library.sync_codex_entries().is_err());
        let error = library.codex_settings().unwrap().last_error;
        assert!(error.contains(&partial.to_string_lossy().to_string()));
        assert!(error.contains("未完の索引一時ファイル"));
        assert!(entry(&base, &id).join("SKILL.md").is_file());
        assert_eq!(fs::read_dir(partial.parent().unwrap()).unwrap().count(), 1);
    }
}

#[test]
fn interrupted_publication_restores_the_owned_previous_index() {
    let (base, library, id) = setup();
    publish(&library, &id, true);
    library.set_codex_policy(&id, false, true).unwrap();
    library.set_codex_enabled(true).unwrap();
    let root = base.join("home/.agents/skills/skilvella");
    let state = base.join("data/codex-index");
    let previous: serde_json::Value =
        serde_json::from_slice(&fs::read(state.join("manifest.json")).unwrap()).unwrap();
    let canonical_root = root.canonicalize().unwrap();
    let agents = canonical_root.parent().unwrap().parent().unwrap();
    let stage = agents
        .join("skilvella-staging")
        .join(uuid::Uuid::new_v4().to_string());
    let retired = agents
        .join("skilvella-retired")
        .join(uuid::Uuid::new_v4().to_string());
    fs::create_dir_all(&stage).unwrap();
    fs::create_dir_all(retired.parent().unwrap()).unwrap();
    for relative in previous["files"].as_object().unwrap().keys() {
        let destination = stage.join(relative);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::copy(root.join(relative), destination).unwrap();
    }
    let pending = serde_json::json!({"previous": previous, "next": previous, "stage": stage, "retired": retired});
    fs::write(
        state.join("pending.json"),
        serde_json::to_vec(&pending).unwrap(),
    )
    .unwrap();
    fs::rename(&root, &retired).unwrap();
    fs::rename(&stage, &root).unwrap();
    let reopened = Library::open(&base.join("data")).unwrap();
    reopened.sync_codex_entries().unwrap();
    assert!(entry(&base, &id).join("SKILL.md").is_file());
    assert!(!retired.exists());
    assert!(!stage.exists());
    assert!(!state.join("pending.json").exists());
    assert!(reopened.codex_settings().unwrap().last_error.is_empty());
}

#[test]
fn duplicate_reserved_and_invalid_names_receive_safe_distinct_commands() {
    let (base, library, _) = setup();
    for (directory, name) in [
        ("duplicate", "original"),
        ("reserved", "skilvella-search"),
        ("invalid", "日本語の名前"),
    ] {
        let directory = base.join("source").join(directory);
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Safe command name\n---\nBody\n"),
        )
        .unwrap();
    }
    library.scan().unwrap();
    let skills = library
        .search(SearchQuery::default(), false)
        .unwrap()
        .skills;
    for skill in &skills {
        publish(&library, &skill.id, true);
        library.set_codex_policy(&skill.id, false, true).unwrap();
    }
    library.set_codex_enabled(true).unwrap();
    assert_eq!(library.codex_settings().unwrap().exported, 4);
    for skill in skills {
        let prefix = if skill.name == "日本語の名前" {
            "skill"
        } else {
            &skill.name
        };
        let command = format!("{prefix}-{}", &skill.id[..8]);
        let exported = base.join("home/.agents/skills/skilvella").join(command);
        assert!(exported.join("SKILL.md").is_file());
        assert!(
            fs::read_to_string(exported.join("agents/openai.yaml"))
                .unwrap()
                .contains(&skill.name)
        );
    }
    assert!(
        base.join("home/.agents/skills/skilvella/skilvella-search/SKILL.md")
            .is_file()
    );
}

#[cfg(unix)]
#[test]
fn linked_export_destinations_are_rejected() {
    let (base, library, _) = setup();
    fs::create_dir_all(base.join("home/.agents/skills")).unwrap();
    fs::create_dir_all(base.join("external")).unwrap();
    std::os::unix::fs::symlink(
        base.join("external"),
        base.join("home/.agents/skills/skilvella"),
    )
    .unwrap();
    library.set_codex_enabled(true).unwrap();
    assert!(
        library
            .codex_settings()
            .unwrap()
            .last_error
            .contains("links")
    );
    assert!(
        fs::read_dir(base.join("external"))
            .unwrap()
            .next()
            .is_none()
    );
}
