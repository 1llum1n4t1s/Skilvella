use skilvella::{
    library::{Library, create_directory_link},
    model::{SearchQuery, SkillClient, SkillMetadata},
};
use std::{fs, path::PathBuf};

// 出力先リンクの拒否テスト用。directory.rs と共有するリンク作成フィクスチャ（本体の実装とは独立）。
#[path = "support/link.rs"]
mod link;

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
    let finder =
        fs::read_to_string(base.join("home/.agents/skills/skilvella/skilvella-search/SKILL.md"))
            .unwrap();
    for instructions in [&wrapper, &finder] {
        assert!(instructions.contains("toolport_search_tools"));
        assert!(instructions.contains("toolport_call_tool"));
        assert!(instructions.contains("parameters inside arguments"));
        assert!(instructions.contains("available tool inventory"));
        assert!(instructions.contains("Never replace MCP retrieval with local skill files"));
    }
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

/// 出力先（Codex入口の生成先）がリンクなら同期を拒否し、リンク先（外部）へ何も書かないこと。
///
/// リンクは本体の `create_directory_link` ではなく独立したフィクスチャで作る（Windowsはジャンクション、
/// unixはシンボリックリンク）。拒否理由の文言はOSごとに検出経路が異なるため、それぞれ従来の期待値を維持する。
#[test]
fn linked_export_destinations_are_rejected() {
    let (base, library, _) = setup();
    fs::create_dir_all(base.join("home/.agents/skills")).unwrap();
    fs::create_dir_all(base.join("external")).unwrap();
    link::link_directory(
        &base.join("external"),
        &base.join("home/.agents/skills/skilvella"),
    );
    library.set_codex_enabled(true).unwrap();
    // Windowsはジャンクションを "forbidden"、unixはシンボリックリンクを "links" として拒否する。
    #[cfg(windows)]
    let expected = "forbidden";
    #[cfg(unix)]
    let expected = "links";
    let last_error = library.codex_settings().unwrap().last_error;
    assert!(last_error.contains(expected), "{last_error}");
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
    let duplicates = library
        .search(SearchQuery::default(), false)
        .unwrap()
        .skills
        .into_iter()
        .filter(|skill| skill.name == "original")
        .collect::<Vec<_>>();
    library
        .set_codex_policy(&duplicates[0].id, false, false)
        .unwrap();
    library.sync_codex_entries().unwrap();
    assert_eq!(library.codex_settings().unwrap().exported, 3);
    let command = format!("original-{}", &duplicates[1].id[..8]);
    let exported = base.join("home/.agents/skills/skilvella").join(command);
    assert!(exported.join("SKILL.md").is_file());
    assert!(
        fs::read_to_string(exported.join("agents/openai.yaml"))
            .unwrap()
            .contains(&duplicates[1].relative_path)
    );
}

/// 探索先のリンク（Windowsはジャンクション）かどうか。
fn is_link(path: &std::path::Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
}

#[test]
fn claude_and_grok_targets_use_their_own_layout_and_invocation_policy() {
    let (base, library, id) = setup();
    publish(&library, &id, true);
    library.set_codex_policy(&id, false, true).unwrap();
    // 既定ではClaude Code・Grokは無効で、旧版の設定（Codexだけ）と同じ状態になる。
    assert!(
        !library
            .client_settings(SkillClient::Claude)
            .unwrap()
            .enabled
    );
    assert!(!library.client_settings(SkillClient::Grok).unwrap().enabled);
    library
        .set_clients_enabled(&[(SkillClient::Claude, true), (SkillClient::Grok, true)])
        .unwrap();
    assert!(library.sync_summary().unwrap().last_error.is_empty());
    // Codexは選んでいないため生成しない。
    assert!(!base.join("home/.agents/skills/skilvella").exists());

    // Grokは入れ子を探索するため、専用フォルダーへ実ファイルで置く。
    let grok = base.join("home/.grok/skills/skilvella/original");
    let grok_text = fs::read_to_string(grok.join("SKILL.md")).unwrap();
    assert!(grok_text.contains("disable-model-invocation: true\n---\n"));
    assert!(grok_text.contains(&format!("load_skill with the fixed id \"{id}\"")));
    assert!(!grok_text.contains("PRIVATE ORIGINAL BODY"));
    assert!(!grok.join("agents/openai.yaml").exists());
    assert!(
        fs::read_to_string(base.join("home/.grok/skills/skilvella/skilvella-search/SKILL.md"))
            .unwrap()
            .contains("disable-model-invocation: true")
    );

    // Claude Codeは1階層しか探索しないため、生成物を探索外に置いてリンクを張る。
    let store = base.join("home/.claude/skilvella/skills/original");
    let link = base.join("home/.claude/skills/original");
    assert!(store.join("SKILL.md").is_file());
    assert!(is_link(&link));
    assert!(is_link(&base.join("home/.claude/skills/skilvella-search")));
    assert_eq!(
        fs::read_to_string(link.join("SKILL.md")).unwrap(),
        fs::read_to_string(store.join("SKILL.md")).unwrap()
    );
    let claude = library.client_settings(SkillClient::Claude).unwrap();
    assert_eq!(claude.exported, 1);
    assert!(PathBuf::from(&claude.link_path).ends_with(".claude/skills"));

    // 自動選択を許可するとfrontmatterの制限を外す。リンクは張り直さずに追従する。
    library.set_codex_policy(&id, true, true).unwrap();
    assert!(
        !fs::read_to_string(grok.join("SKILL.md"))
            .unwrap()
            .contains("disable-model-invocation")
    );
    assert!(
        !fs::read_to_string(link.join("SKILL.md"))
            .unwrap()
            .contains("disable-model-invocation")
    );

    // 検索のみへ戻すと、個別の入口とリンクを外す。
    library.set_codex_policy(&id, false, false).unwrap();
    assert!(!grok.exists());
    assert!(fs::symlink_metadata(&link).is_err());
    assert!(is_link(&base.join("home/.claude/skills/skilvella-search")));

    // 無効にすると生成物・リンク・リンクの所有記録を外し、原本は変えない。
    library
        .set_clients_enabled(&[(SkillClient::Claude, false), (SkillClient::Grok, false)])
        .unwrap();
    assert!(library.sync_summary().unwrap().last_error.is_empty());
    assert!(!base.join("home/.grok/skills/skilvella").exists());
    assert!(!base.join("home/.claude/skilvella/skills").exists());
    assert!(fs::symlink_metadata(base.join("home/.claude/skills/skilvella-search")).is_err());
    assert!(!base.join("data/claude-index/links.json").exists());
    assert!(
        fs::read_to_string(base.join("source/one/SKILL.md"))
            .unwrap()
            .contains("PRIVATE ORIGINAL BODY")
    );
}

#[test]
fn claude_links_preserve_user_entries_and_adopt_legacy_codex_links() {
    let (base, library, id) = setup();
    publish(&library, &id, true);
    library.set_codex_policy(&id, false, true).unwrap();
    library.set_codex_enabled(true).unwrap();
    let skills = base.join("home/.claude/skills");
    fs::create_dir_all(&skills).unwrap();
    // 旧版で手動移行した、Codex入口への同名リンク。
    let codex = base.join("home/.agents/skills/skilvella");
    let normalized_codex = PathBuf::from(library.codex_settings().unwrap().path);
    create_directory_link(&normalized_codex.join("original"), &skills.join("original")).unwrap();
    // 利用者自身のスキルと同名の検索入口は上書きしない。
    fs::create_dir_all(skills.join("skilvella-search")).unwrap();
    fs::write(skills.join("skilvella-search/SKILL.md"), "user skill").unwrap();
    // 無関係なリンク・スキルにも触れない。
    fs::create_dir_all(skills.join("mine")).unwrap();
    fs::write(skills.join("mine/SKILL.md"), "mine").unwrap();

    library
        .set_clients_enabled(&[(SkillClient::Claude, true)])
        .unwrap();
    let claude = library.client_settings(SkillClient::Claude).unwrap();
    assert!(
        claude.last_error.contains("skilvella-search"),
        "{}",
        claude.last_error
    );
    // 衝突以外は同期し、Codexは影響を受けない。
    assert_eq!(claude.exported, 1);
    assert!(library.codex_settings().unwrap().last_error.is_empty());
    assert!(codex.join("original/SKILL.md").is_file());
    assert_eq!(
        fs::read_to_string(skills.join("skilvella-search/SKILL.md")).unwrap(),
        "user skill"
    );
    assert_eq!(
        fs::read_to_string(skills.join("mine/SKILL.md")).unwrap(),
        "mine"
    );
    // 旧リンクはClaude Code用の生成物（disable-model-invocation付き）へ張り替える。
    assert!(is_link(&skills.join("original")));
    assert!(
        fs::read_to_string(skills.join("original/SKILL.md"))
            .unwrap()
            .contains("disable-model-invocation: true")
    );

    // 利用者がリンクを自分のフォルダーへ置き換えた場合は、無効化しても残す。
    library
        .set_clients_enabled(&[(SkillClient::Claude, false)])
        .unwrap();
    assert!(fs::symlink_metadata(skills.join("original")).is_err());
    assert_eq!(
        fs::read_to_string(skills.join("skilvella-search/SKILL.md")).unwrap(),
        "user skill"
    );
    fs::remove_dir_all(skills.join("skilvella-search")).unwrap();
    library
        .set_clients_enabled(&[(SkillClient::Claude, true)])
        .unwrap();
    assert!(library.sync_summary().unwrap().last_error.is_empty());
    let replaced = skills.join("original");
    fs::remove_dir(&replaced)
        .or_else(|_| fs::remove_file(&replaced))
        .unwrap();
    fs::create_dir_all(&replaced).unwrap();
    fs::write(replaced.join("SKILL.md"), "replaced by user").unwrap();
    library
        .set_clients_enabled(&[(SkillClient::Claude, false)])
        .unwrap();
    assert_eq!(
        fs::read_to_string(replaced.join("SKILL.md")).unwrap(),
        "replaced by user"
    );
    assert!(fs::symlink_metadata(skills.join("skilvella-search")).is_err());
    // Codexの入口は残る。
    assert!(codex.join("original/SKILL.md").is_file());
}

#[test]
fn claude_links_refuse_a_managed_source_link_directory() {
    let (base, library, _) = setup();
    fs::create_dir_all(base.join("home/.claude/skills/own/x")).unwrap();
    fs::write(
        base.join("home/.claude/skills/own/x/SKILL.md"),
        "---\nname: x\ndescription: d\n---\nbody\n",
    )
    .unwrap();
    library
        .add_source("claude", &base.join("home/.claude/skills"))
        .unwrap();
    library
        .set_clients_enabled(&[(SkillClient::Claude, true)])
        .unwrap();
    let claude = library.client_settings(SkillClient::Claude).unwrap();
    assert!(
        claude.last_error.contains("overlap"),
        "{}",
        claude.last_error
    );
    assert!(fs::symlink_metadata(base.join("home/.claude/skills/skilvella-search")).is_err());
    assert!(!base.join("home/.claude/skilvella").exists());
}

/// 変化のない再同期では同期状態（settings）を書き直さないこと。
///
/// 観測方法の注意: SQLiteは同じ値のUPSERTではページを変更しないため、別接続の
/// `PRAGMA data_version` は旧実装（毎回UPSERT）でも変わらず、退行を検出できない（実測済み）。
/// そこで隔離したテストDBに、同期状態キーへのINSERT/UPDATEを数えるトリガーを置き、
/// 同値の上書きも含めて書込み文の実行そのものを数える。
#[test]
fn unchanged_sync_state_is_not_rewritten() {
    let (base, library, id) = setup();
    library.set_codex_enabled(true).unwrap();
    library.set_codex_policy(&id, false, true).unwrap();
    publish(&library, &id, true);
    assert!(entry(&base, &id).join("SKILL.md").is_file());
    let db = rusqlite::Connection::open(base.join("data/library.sqlite3")).unwrap();
    db.execute_batch(
        "CREATE TABLE sync_writes(key TEXT NOT NULL);
         CREATE TRIGGER sync_writes_update AFTER UPDATE ON settings
           WHEN new.key IN ('codex_sync','claude_sync','grok_sync')
           BEGIN INSERT INTO sync_writes VALUES(new.key); END;
         CREATE TRIGGER sync_writes_insert AFTER INSERT ON settings
           WHEN new.key IN ('codex_sync','claude_sync','grok_sync')
           BEGIN INSERT INTO sync_writes VALUES(new.key); END;",
    )
    .unwrap();
    let writes = |db: &rusqlite::Connection| -> i64 {
        db.query_row("SELECT count(*) FROM sync_writes", [], |r| r.get(0))
            .unwrap()
    };
    library.sync_codex_entries().unwrap();
    assert_eq!(writes(&db), 0, "unchanged sync rewrote the sync state");
    // 同期結果（件数・エラー）は従来どおり保存済みの値で読める。
    let settings = library.codex_settings().unwrap();
    assert!(settings.enabled);
    assert_eq!(settings.exported, 1);
    assert!(settings.last_error.is_empty(), "{}", settings.last_error);
    // 実際に同期状態が変わる操作では書込みが数えられること（観測の有効性の確認）。
    library
        .set_clients_enabled(&[(SkillClient::Grok, true)])
        .unwrap();
    assert!(writes(&db) > 0);
}

/// 管理画面の詳細は同名スキルの重複を従来どおり示し、単独名では示さないこと。
#[test]
fn detail_reports_duplicate_names() {
    let (base, library, id) = setup();
    assert!(!library.detail(&id, false).unwrap().summary.duplicate);
    fs::create_dir_all(base.join("source/two")).unwrap();
    fs::write(
        base.join("source/two/SKILL.md"),
        "---\nname: original\ndescription: Another skill\n---\nbody\n",
    )
    .unwrap();
    library.scan().unwrap();
    let detail = library.detail(&id, false).unwrap();
    assert!(detail.summary.error.is_none() && !detail.summary.missing);
    assert!(detail.summary.duplicate);
}

/// 公開スキルのCodexポリシーが不正なJSONなら、入口の生成は従来どおり失敗として記録されること。
/// （ポリシーを主クエリでまとめて読むようにしても、不正値を既定値として扱わない）
#[test]
fn broken_policy_of_a_published_skill_fails_the_export() {
    let (base, library, id) = setup();
    library.set_codex_enabled(true).unwrap();
    publish(&library, &id, true);
    let db = rusqlite::Connection::open(base.join("data/library.sqlite3")).unwrap();
    db.execute(
        "INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![format!("codex_policy:{id}"), "{broken"],
    )
    .unwrap();
    assert!(library.sync_codex_entries().is_err());
    assert!(!library.codex_settings().unwrap().last_error.is_empty());
}
