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
    // 新規スキルは走査で公開状態になる。公開判定とCodex入口の関係を検証するため、
    // 準備段階で明示的に未公開へ戻し、各テストが公開操作から始められるようにする。
    library.set_published(&id, false).unwrap();
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
    // 自動選択のみは許可する。Codexはyamlで暗黙呼び出しを許可し、入口は残る。
    library.set_codex_policy(&id, true, false).unwrap();
    assert!(
        fs::read_to_string(entry(&base, &id).join("agents/openai.yaml"))
            .unwrap()
            .contains("allow_implicit_invocation: true")
    );
    library.set_codex_policy(&id, true, true).unwrap();
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

/// Cursorは実体配置、GitHub Copilot・Devinはリンク方式で、Devinだけ `triggers: [user]` で自動選択を止める。
/// リンク方式の利用者エントリは上書きせず、旧Codex入口へのリンクの引き継ぎはClaude Code以外では行わない。
#[test]
fn cursor_copilot_and_devin_targets_use_their_own_layout_and_invocation_policy() {
    let (base, library, id) = setup();
    let home = base.join("home");
    let devin = skilvella::library::default_devin_config_dir(&home);
    // 検出はホーム配下の設定フォルダーだけを見る（隔離したホームで差し替えられる）。
    assert!(library.detected_clients().unwrap().is_empty());
    fs::create_dir_all(home.join(".grok")).unwrap();
    fs::create_dir_all(&devin).unwrap();
    assert_eq!(
        library.detected_clients().unwrap(),
        [SkillClient::Grok, SkillClient::Devin]
    );

    publish(&library, &id, true);
    library.set_codex_policy(&id, false, true).unwrap();
    for client in [
        SkillClient::Cursor,
        SkillClient::Copilot,
        SkillClient::Devin,
    ] {
        // 設定キーがない旧データでは無効。
        assert!(!library.client_settings(client).unwrap().enabled);
    }
    // 旧版の手動移行を模した、Codex入口への同名リンク（Devinの探索先）。Claude Code以外では引き継がない。
    library.set_codex_enabled(true).unwrap();
    let codex = PathBuf::from(library.codex_settings().unwrap().path);
    let devin_links = devin.join("skills");
    fs::create_dir_all(&devin_links).unwrap();
    create_directory_link(&codex.join("original"), &devin_links.join("original")).unwrap();
    // GitHub Copilotの探索先にある利用者自身のスキル（検索入口と同名・無関係）は上書きしない。
    let copilot_links = home.join(".copilot/skills");
    fs::create_dir_all(copilot_links.join("skilvella-search")).unwrap();
    fs::write(
        copilot_links.join("skilvella-search/SKILL.md"),
        "user skill",
    )
    .unwrap();
    fs::create_dir_all(copilot_links.join("mine")).unwrap();
    fs::write(copilot_links.join("mine/SKILL.md"), "mine").unwrap();

    library
        .set_clients_enabled(&[
            (SkillClient::Cursor, true),
            (SkillClient::Copilot, true),
            (SkillClient::Devin, true),
        ])
        .unwrap();

    // Cursorは入れ子を探索するため、専用フォルダーへ実ファイルで置く（リンクを使わない）。
    let cursor = home.join(".cursor/skills/skilvella");
    let cursor_text = fs::read_to_string(cursor.join("original/SKILL.md")).unwrap();
    assert!(cursor_text.contains("disable-model-invocation: true\n---\n"));
    assert!(!cursor_text.contains("triggers:"));
    assert!(!cursor_text.contains("PRIVATE ORIGINAL BODY"));
    assert!(!cursor.join("original/agents/openai.yaml").exists());
    assert!(!is_link(&cursor.join("original")));
    assert!(
        fs::read_to_string(cursor.join("skilvella-search/SKILL.md"))
            .unwrap()
            .contains("disable-model-invocation: true")
    );
    let cursor_settings = library.client_settings(SkillClient::Cursor).unwrap();
    assert!(cursor_settings.last_error.is_empty());
    assert!(cursor_settings.link_path.is_empty());
    assert_eq!(cursor_settings.exported, 1);

    // GitHub Copilotは生成物を探索外に置き、`~/.copilot/skills/<name>` へリンクを張る。
    let copilot_store = home.join(".copilot/skilvella/skills");
    assert!(is_link(&copilot_links.join("original")));
    assert_eq!(
        fs::read_to_string(copilot_links.join("original/SKILL.md")).unwrap(),
        fs::read_to_string(copilot_store.join("original/SKILL.md")).unwrap()
    );
    assert!(
        fs::read_to_string(copilot_store.join("original/SKILL.md"))
            .unwrap()
            .contains("disable-model-invocation: true\n---\n")
    );
    let copilot = library.client_settings(SkillClient::Copilot).unwrap();
    assert!(PathBuf::from(&copilot.link_path).ends_with(".copilot/skills"));
    assert_eq!(copilot.exported, 1);
    assert!(
        copilot.last_error.contains("GitHub Copilot")
            && copilot.last_error.contains("skilvella-search"),
        "{}",
        copilot.last_error
    );
    assert_eq!(
        fs::read_to_string(copilot_links.join("skilvella-search/SKILL.md")).unwrap(),
        "user skill"
    );
    assert!(base.join("data/copilot-index/links.json").is_file());

    // Devinは設定フォルダーの `skills/<name>` へリンクを張り、`triggers: [user]` で自動選択を止める。
    let devin_store = devin.join("skilvella/skills");
    let devin_text = fs::read_to_string(devin_store.join("original/SKILL.md")).unwrap();
    assert!(
        devin_text.contains("triggers: [user]\n---\n"),
        "{devin_text}"
    );
    assert!(!devin_text.contains("disable-model-invocation"));
    assert!(
        fs::read_to_string(devin_store.join("skilvella-search/SKILL.md"))
            .unwrap()
            .contains("triggers: [user]")
    );
    assert!(is_link(&devin_links.join("skilvella-search")));
    let devin_settings = library.client_settings(SkillClient::Devin).unwrap();
    assert!(PathBuf::from(&devin_settings.link_path).ends_with("devin/skills"));
    // Codex入口へのリンクは引き継がず、同名の衝突として報告する（リンクはCodex入口を指したまま）。
    assert!(
        devin_settings.last_error.contains("Devin")
            && devin_settings.last_error.contains("original"),
        "{}",
        devin_settings.last_error
    );
    assert!(devin_links.join("original/agents/openai.yaml").is_file());
    assert!(
        !fs::read_to_string(devin_links.join("original/SKILL.md"))
            .unwrap()
            .contains("triggers:")
    );
    assert!(base.join("data/devin-index/links.json").is_file());
    // 他のクライアントの失敗はCodex・Cursorを止めない。
    assert!(library.codex_settings().unwrap().last_error.is_empty());

    // 自動選択を許可するとfrontmatterの制限を外す。
    library.set_codex_policy(&id, true, true).unwrap();
    assert!(
        !fs::read_to_string(cursor.join("original/SKILL.md"))
            .unwrap()
            .contains("disable-model-invocation")
    );
    assert!(
        !fs::read_to_string(devin_store.join("original/SKILL.md"))
            .unwrap()
            .contains("triggers:")
    );
    assert!(
        !fs::read_to_string(copilot_links.join("original/SKILL.md"))
            .unwrap()
            .contains("disable-model-invocation")
    );

    // 無効にすると生成物・自分のリンク・所有記録を外し、利用者のエントリと旧リンクは残す。
    library
        .set_clients_enabled(&[
            (SkillClient::Cursor, false),
            (SkillClient::Copilot, false),
            (SkillClient::Devin, false),
        ])
        .unwrap();
    assert!(!cursor.exists());
    assert!(!copilot_store.exists());
    assert!(!devin_store.exists());
    assert!(fs::symlink_metadata(copilot_links.join("original")).is_err());
    assert!(fs::symlink_metadata(devin_links.join("skilvella-search")).is_err());
    assert!(!base.join("data/copilot-index/links.json").exists());
    assert!(!base.join("data/devin-index/links.json").exists());
    assert_eq!(
        fs::read_to_string(copilot_links.join("skilvella-search/SKILL.md")).unwrap(),
        "user skill"
    );
    assert_eq!(
        fs::read_to_string(copilot_links.join("mine/SKILL.md")).unwrap(),
        "mine"
    );
    assert!(is_link(&devin_links.join("original")));
    for client in [
        SkillClient::Cursor,
        SkillClient::Copilot,
        SkillClient::Devin,
    ] {
        assert!(
            library
                .client_settings(client)
                .unwrap()
                .last_error
                .is_empty(),
            "{client:?}"
        );
    }
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
           WHEN new.key IN ('codex_sync','claude_sync','grok_sync','cursor_sync','copilot_sync','devin_sync')
           BEGIN INSERT INTO sync_writes VALUES(new.key); END;
         CREATE TRIGGER sync_writes_insert AFTER INSERT ON settings
           WHEN new.key IN ('codex_sync','claude_sync','grok_sync','cursor_sync','copilot_sync','devin_sync')
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

#[test]
fn automatic_only_hides_commands_only_where_supported() {
    let (base, library, id) = setup();
    let home = base.join("home");
    let devin = skilvella::library::default_devin_config_dir(&home);
    publish(&library, &id, true);
    library
        .set_clients_enabled(&[
            (SkillClient::Claude, true),
            (SkillClient::Grok, true),
            (SkillClient::Cursor, true),
            (SkillClient::Copilot, true),
            (SkillClient::Devin, true),
        ])
        .unwrap();
    library.set_codex_enabled(true).unwrap();
    library.set_codex_policy(&id, true, false).unwrap();
    assert!(library.sync_summary().unwrap().last_error.is_empty());

    let read = |path: PathBuf| fs::read_to_string(path.join("original/SKILL.md")).unwrap();
    let claude_store = home.join(".claude/skilvella/skills");
    let copilot_store = home.join(".copilot/skilvella/skills");
    for (label, store) in [
        ("claude", claude_store.clone()),
        ("grok", home.join(".grok/skills/skilvella")),
        ("copilot", copilot_store.clone()),
    ] {
        let text = read(store);
        assert!(
            text.contains(
                "user-invocable: false
---
"
            ),
            "{label}: {text}"
        );
        assert!(
            !text.contains("disable-model-invocation"),
            "{label}: {text}"
        );
        assert!(!text.contains("triggers:"), "{label}: {text}");
    }
    for (label, store) in [
        ("cursor", home.join(".cursor/skills/skilvella")),
        ("devin", devin.join("skilvella/skills")),
    ] {
        let text = read(store);
        for appended in ["user-invocable", "disable-model-invocation", "triggers:"] {
            assert!(!text.contains(appended), "{label}: {text}");
        }
    }
    assert!(
        fs::read_to_string(entry(&base, &id).join("agents/openai.yaml"))
            .unwrap()
            .contains("allow_implicit_invocation: true")
    );
    assert!(is_link(&home.join(".claude/skills/original")));
    assert!(is_link(&home.join(".copilot/skills/original")));
    assert!(is_link(&devin.join("skills/original")));
    let db = rusqlite::Connection::open(base.join("data/library.sqlite3")).unwrap();
    let stored: String = db
        .query_row(
            "SELECT value FROM settings WHERE key=?1",
            [format!("codex_policy:{id}")],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored, r#"{"automatic":true,"command":false}"#);

    // 検索のみへ戻すと、すべての入口が外れる。
    library.set_codex_policy(&id, false, false).unwrap();
    assert!(!entry(&base, &id).exists());
    assert!(!claude_store.join("original").exists());
    assert!(!copilot_store.join("original").exists());
    assert!(!home.join(".claude/skills/original").exists());
}

/// 入口のfrontmatter（先頭の `---` と閉じ `---` の間）をYAMLとして再解析する。
fn entry_frontmatter(text: &str) -> saphyr::YamlOwned {
    use saphyr::LoadableYamlNode;
    let body = text.strip_prefix("---\n").expect("starts with frontmatter");
    let end = body.find("\n---\n").expect("closed frontmatter");
    let mut docs = saphyr::YamlOwned::load_from_str(&body[..end]).expect("valid YAML");
    assert_eq!(docs.len(), 1);
    docs.remove(0)
}

/// 原本のfrontmatterを差し替えて再走査し、公開・全クライアントへ再生成する。
fn rewrite_source(base: &std::path::Path, library: &Library, frontmatter: &str) {
    fs::write(
        base.join("source/one/SKILL.md"),
        format!("---\nname: original\ndescription: A useful skill\n{frontmatter}---\nPRIVATE ORIGINAL BODY\n"),
    )
    .unwrap();
    library.scan().unwrap();
    library.sync_codex_entries().unwrap();
}

fn claude_entry_text(base: &std::path::Path) -> String {
    fs::read_to_string(base.join("home/.claude/skilvella/skills/original/SKILL.md")).unwrap()
}

#[test]
fn claude_entry_inherits_only_allowed_frontmatter_and_other_clients_do_not() {
    let (base, library, id) = setup();
    let home = base.join("home");
    let devin = skilvella::library::default_devin_config_dir(&home);
    fs::create_dir_all(&devin).unwrap();
    publish(&library, &id, true);
    library.set_codex_policy(&id, false, true).unwrap();
    library
        .set_clients_enabled(&[
            (SkillClient::Claude, true),
            (SkillClient::Grok, true),
            (SkillClient::Cursor, true),
            (SkillClient::Copilot, true),
            (SkillClient::Devin, true),
        ])
        .unwrap();
    library.set_codex_enabled(true).unwrap();
    rewrite_source(
        &base,
        &library,
        "context: fork\nagent: Explore\nbackground: true\nmodel: opus\neffort: high\nargument-hint: \"[topic]\"\npaths:\n  - src/**\n  - docs/**\nallowed-tools: Read\nhooks:\n  PreToolUse: []\ndisable-model-invocation: false\nuser-invocable: true\n",
    );

    let claude = claude_entry_text(&base);
    assert!(
        claude.contains("context: \"fork\"\nagent: \"Explore\"\n"),
        "{claude}"
    );
    // fork の Claude Code 入口だけは原本本文を埋め込む。
    assert!(claude.contains("PRIVATE ORIGINAL BODY"), "{claude}");
    assert!(claude.contains("`ARGUMENTS:`"));
    // 自動選択の停止行は最後に残り、原本の `disable-model-invocation: false` は通さない。
    assert!(
        claude.contains("disable-model-invocation: true\n---\n"),
        "{claude}"
    );
    for dropped in ["allowed-tools", "hooks", "user-invocable", "PreToolUse"] {
        assert!(!claude.contains(dropped), "{dropped}: {claude}");
    }
    let doc = entry_frontmatter(&claude);
    let get = |key: &str| doc.as_mapping_get(key);
    assert_eq!(get("context").and_then(|v| v.as_str()), Some("fork"));
    assert_eq!(get("agent").and_then(|v| v.as_str()), Some("Explore"));
    assert_eq!(get("background").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(get("model").and_then(|v| v.as_str()), Some("opus"));
    assert_eq!(get("effort").and_then(|v| v.as_str()), Some("high"));
    assert_eq!(
        get("argument-hint").and_then(|v| v.as_str()),
        Some("[topic]")
    );
    let paths: Vec<_> = get("paths")
        .and_then(|v| v.as_sequence())
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert_eq!(paths, ["src/**", "docs/**"]);
    assert_eq!(
        get("disable-model-invocation").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(doc.as_mapping().unwrap().len(), 10);

    // 他のクライアントの入口は従来どおり（引き継ぎなし）。
    let others = [
        home.join(".agents/skills/skilvella/original/SKILL.md"),
        home.join(".grok/skills/skilvella/original/SKILL.md"),
        home.join(".cursor/skills/skilvella/original/SKILL.md"),
        home.join(".copilot/skilvella/skills/original/SKILL.md"),
        devin.join("skilvella/skills/original/SKILL.md"),
    ];
    for path in others {
        let text = fs::read_to_string(&path).unwrap();
        for key in [
            "context:",
            "agent:",
            "model:",
            "effort:",
            "paths:",
            "argument-hint:",
            "background:",
        ] {
            assert!(!text.contains(key), "{key} in {}: {text}", path.display());
        }
        assert!(!text.contains("PRIVATE ORIGINAL BODY"));
        assert!(text.contains("`ARGUMENTS:`"));
    }

    // 原本から context を消すと、再生成で context と、それに依存する agent・background も消える。
    rewrite_source(
        &base,
        &library,
        "agent: Explore\nbackground: true\nmodel: opus\n",
    );
    let claude = claude_entry_text(&base);
    assert!(
        !claude.contains("context:")
            && !claude.contains("agent:")
            && !claude.contains("background:"),
        "{claude}"
    );
    assert!(
        claude.contains("model: \"opus\"\ndisable-model-invocation: true\n---\n"),
        "{claude}"
    );
}

#[test]
fn claude_entry_drops_invalid_inherited_values_without_stopping_generation() {
    let (base, library, id) = setup();
    publish(&library, &id, true);
    library.set_codex_policy(&id, false, true).unwrap();
    library
        .set_clients_enabled(&[(SkillClient::Claude, true)])
        .unwrap();

    // 型不正・入れ子・制御文字・許可外の値は、そのキーだけ捨てる。
    rewrite_source(
        &base,
        &library,
        "context: fork\nagent: \"a\\nb\"\nbackground: \"yes\"\nmodel:\n  x: 1\neffort: ultra\nargument-hint: \"[issue]\"\npaths:\n  - a\n  - x: 1\n",
    );
    let claude = claude_entry_text(&base);
    let doc = entry_frontmatter(&claude);
    let keys: Vec<_> = doc
        .as_mapping()
        .unwrap()
        .keys()
        .filter_map(|k| k.as_str())
        .collect();
    assert_eq!(
        keys,
        [
            "name",
            "description",
            "context",
            "argument-hint",
            "disable-model-invocation"
        ],
        "{claude}"
    );

    // context が fork 以外なら agent・background も出さない。
    rewrite_source(
        &base,
        &library,
        "context: inline\nagent: Explore\nbackground: true\npaths: src/**\neffort: max\n",
    );
    let claude = claude_entry_text(&base);
    assert!(
        !claude.contains("context:")
            && !claude.contains("agent:")
            && !claude.contains("background:"),
        "{claude}"
    );
    assert!(
        claude
            .contains("effort: \"max\"\npaths: \"src/**\"\ndisable-model-invocation: true\n---\n"),
        "{claude}"
    );
    assert!(library.sync_summary().unwrap().last_error.is_empty());
}

#[test]
fn claude_entry_inherits_with_automatic_selection_and_quotes_special_values() {
    let (base, library, id) = setup();
    publish(&library, &id, true);
    // 自動選択＋コマンド候補（停止行なし）。
    library.set_codex_policy(&id, true, true).unwrap();
    library
        .set_clients_enabled(&[(SkillClient::Claude, true)])
        .unwrap();
    rewrite_source(
        &base,
        &library,
        "context: fork\nagent: Explore\nargument-hint: 'a: \"b\" # c'\n",
    );
    let claude = claude_entry_text(&base);
    assert!(!claude.contains("disable-model-invocation"), "{claude}");
    assert!(claude.contains("`ARGUMENTS:`") && claude.contains("PRIVATE ORIGINAL BODY"));
    assert!(!claude.contains("it has none."), "{claude}");
    let doc = entry_frontmatter(&claude);
    assert_eq!(
        doc.as_mapping_get("context").and_then(|v| v.as_str()),
        Some("fork")
    );
    assert_eq!(
        doc.as_mapping_get("agent").and_then(|v| v.as_str()),
        Some("Explore")
    );
    assert_eq!(
        doc.as_mapping_get("argument-hint").and_then(|v| v.as_str()),
        Some("a: \"b\" # c")
    );

    // 非文字（U+FFFF）を含む値は捨てる。
    rewrite_source(
        &base,
        &library,
        "context: fork\nagent: \"\u{ffff}\"\nmodel: opus\n",
    );
    let claude = claude_entry_text(&base);
    assert!(!claude.contains("agent:"), "{claude}");
    assert!(claude.contains("model: \"opus\"\n---\n"), "{claude}");
}

// ---- context: fork の Claude Code 入口への本文埋め込み ----

/// Claude Code と Grok だけを有効にし、自動選択＋コマンド候補で公開した状態にする。
fn embed_setup() -> (PathBuf, Library, String) {
    let (base, library, id) = setup();
    publish(&library, &id, true);
    library.set_codex_policy(&id, true, true).unwrap();
    library
        .set_clients_enabled(&[(SkillClient::Claude, true), (SkillClient::Grok, true)])
        .unwrap();
    (base, library, id)
}

/// frontmatter の後ろの本文を丸ごと指定して原本を差し替え、再生成する。
fn rewrite_body(base: &std::path::Path, library: &Library, frontmatter: &str, body: &str) {
    fs::write(
        base.join("source/one/SKILL.md"),
        format!("---\nname: original\ndescription: A useful skill\n{frontmatter}---\n{body}"),
    )
    .unwrap();
    library.scan().unwrap();
    library.sync_codex_entries().unwrap();
}

fn grok_entry_text(base: &std::path::Path) -> String {
    fs::read_to_string(base.join("home/.grok/skills/skilvella/original/SKILL.md")).unwrap()
}

const LEGACY_TAIL: &str = "Never replace MCP retrieval with local skill files.\n";

fn assert_legacy(claude: &str) {
    assert!(claude.contains("load_skill with the fixed id"), "{claude}");
    assert!(!claude.contains("Skilvella embedded"), "{claude}");
    assert!(claude.ends_with(LEGACY_TAIL), "{claude}");
}

#[test]
fn fork_claude_entry_automatic_only_adds_one_user_invocable_false() {
    let (base, library, id) = embed_setup();
    library.set_codex_policy(&id, true, false).unwrap();
    rewrite_body(&base, &library, "context: fork\n", "EMBED ME\n");
    let claude = claude_entry_text(&base);
    assert!(claude.contains("EMBED ME"), "{claude}");
    assert!(!claude.contains("load_skill with the fixed id"), "{claude}");
    assert_eq!(
        claude.matches("user-invocable: false").count(),
        1,
        "{claude}"
    );
    assert!(!claude.contains("disable-model-invocation"), "{claude}");
}

#[test]
fn fork_claude_entry_embeds_the_original_body() {
    let (base, library, id) = embed_setup();
    let body = "# Title\n\nUse $ARGUMENTS and read [ref](references/a.md).\n";
    rewrite_body(
        &base,
        &library,
        "context: fork\nagent: Explore\narguments: topic scope\nargument-hint: \"[topic]\"\n",
        body,
    );
    let claude = claude_entry_text(&base);
    assert!(claude.contains(body), "{claude}");
    assert!(claude.contains("read_skill_file"), "{claude}");
    assert!(claude.contains(&format!("\"{id}\"")), "{claude}");
    assert!(!claude.contains("load_skill with the fixed id"), "{claude}");
    assert!(claude.contains("`ARGUMENTS:`"), "{claude}");
    assert!(claude.ends_with(LEGACY_TAIL), "{claude}");
    let doc = entry_frontmatter(&claude);
    let get = |key: &str| doc.as_mapping_get(key);
    assert_eq!(get("context").and_then(|v| v.as_str()), Some("fork"));
    assert_eq!(get("agent").and_then(|v| v.as_str()), Some("Explore"));
    assert_eq!(
        get("arguments").and_then(|v| v.as_str()),
        Some("topic scope")
    );
    assert_eq!(
        get("argument-hint").and_then(|v| v.as_str()),
        Some("[topic]")
    );
    let keys: Vec<_> = doc
        .as_mapping()
        .unwrap()
        .keys()
        .filter_map(|k| k.as_str())
        .collect();
    assert_eq!(
        keys,
        [
            "name",
            "description",
            "context",
            "agent",
            "argument-hint",
            "arguments"
        ]
    );
    // 本文は frontmatter の後ろ、前置き文の次に原文のまま置かれる。
    assert!(claude.contains(&format!("\n\n{body}\n")), "{claude}");

    // 配列の arguments。制御文字・入れ子を含む値は捨てる。
    rewrite_body(
        &base,
        &library,
        "context: fork\narguments:\n  - topic\n  - scope\n",
        body,
    );
    let doc = entry_frontmatter(&claude_entry_text(&base));
    let items: Vec<_> = doc
        .as_mapping_get("arguments")
        .and_then(|v| v.as_sequence())
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert_eq!(items, ["topic", "scope"]);
    rewrite_body(
        &base,
        &library,
        "context: fork\narguments: \"a\\nb\"\n",
        body,
    );
    assert!(!claude_entry_text(&base).contains("arguments:"));
    rewrite_body(
        &base,
        &library,
        "context: fork\narguments:\n  - a\n  - x: 1\n",
        body,
    );
    assert!(!claude_entry_text(&base).contains("arguments:"));
}

#[test]
fn fork_claude_entry_keeps_the_legacy_text_when_body_is_not_embeddable() {
    let (base, library, _id) = embed_setup();
    let cases = [
        "!`date`\nbody\n",
        "Run !`x` now\n",
        "Run\t!`x`\n",
        "```!\ndate\n```\n",
        "  ~~~~ !\ndate\n~~~~\n",
        "> ```!\n> date\n> ```\n",
        "- ```!\n  date\n  ```\n",
        "Run ```!date``` now\n",
        "see ${CLAUDE_SKILL_DIR}/a\n",
        "   \n\n",
        "",
    ];
    for body in cases {
        rewrite_body(&base, &library, "context: fork\narguments: a b\n", body);
        let claude = claude_entry_text(&base);
        assert_legacy(&claude);
        // 従来方式では arguments を引き継がない。
        assert!(!claude.contains("\narguments: "), "{body:?}: {claude}");
        assert!(library.sync_summary().unwrap().last_error.is_empty());
    }
    // 英数字・`_` の直後の `!`（例: panic!）や、`!` で始まらないフェンスは除外しない。
    for body in [
        "Use `panic!` and my_!`x` here\n",
        "```rust\nlet a = 1;\n```\n",
    ] {
        rewrite_body(&base, &library, "context: fork\n", body);
        let claude = claude_entry_text(&base);
        assert!(claude.contains(body), "{claude}");
        assert!(!claude.contains("load_skill with the fixed id"), "{claude}");
    }
    // agent が捨てられても fork なら埋め込む。
    rewrite_body(&base, &library, "context: fork\nagent: \"a\\nb\"\n", "B\n");
    let claude = claude_entry_text(&base);
    assert!(
        !claude.contains("agent:") && claude.contains("\nB\n"),
        "{claude}"
    );
}

#[test]
fn fork_claude_entry_embeds_crlf_and_bom_sources_unchanged() {
    let (base, library, _id) = embed_setup();
    let source = "\u{feff}---\r\nname: original\r\ndescription: A useful skill\r\ncontext: fork\r\n---\r\nline one\r\nline two";
    fs::write(base.join("source/one/SKILL.md"), source).unwrap();
    library.scan().unwrap();
    library.sync_codex_entries().unwrap();
    let claude = claude_entry_text(&base);
    // 改行で終わらない本文には \n だけを補う。CRLFはそのまま。
    assert!(claude.contains("\n\nline one\r\nline two\n\n"), "{claude}");
    assert!(!claude.contains("load_skill with the fixed id"), "{claude}");
    let doc = entry_frontmatter(&claude);
    assert_eq!(
        doc.as_mapping_get("context").and_then(|v| v.as_str()),
        Some("fork")
    );
}

#[test]
fn fork_claude_entry_over_the_size_limit_falls_back_without_a_sync_error() {
    let (base, library, _id) = embed_setup();
    let header = "---\nname: original\ndescription: A useful skill\ncontext: fork\n---\n";
    let limit = 2 * 1024 * 1024;
    // 原本は上限ちょうど。埋め込むと入口が上限を超える。
    let body = "x".repeat(limit - header.len() - 1) + "\n";
    fs::write(base.join("source/one/SKILL.md"), format!("{header}{body}")).unwrap();
    assert_eq!(header.len() + body.len(), limit);
    library.scan().unwrap();
    library.sync_codex_entries().unwrap();
    let claude = claude_entry_text(&base);
    assert_legacy(&claude);
    assert!(!claude.contains("xxxx"));
    assert!(library.sync_summary().unwrap().last_error.is_empty());
}

#[test]
fn embedded_body_does_not_remain_after_unpublishing_or_disabling_commands() {
    let (base, library, id) = embed_setup();
    rewrite_body(&base, &library, "context: fork\n", "SECRET EMBEDDED BODY\n");
    let leaks = |base: &std::path::Path| {
        let mut found = false;
        // 退避フォルダー（skilvella-retired）には次の同期まで写しが残る仕様のため、入口の生成先だけ見る。
        let mut stack = vec![base.join("home/.claude/skilvella/skills")];
        while let Some(dir) = stack.pop() {
            let Ok(read) = fs::read_dir(&dir) else {
                continue;
            };
            for item in read.flatten() {
                let path = item.path();
                if path.is_dir() {
                    stack.push(path);
                } else if fs::read_to_string(&path)
                    .is_ok_and(|t| t.contains("SECRET EMBEDDED BODY"))
                {
                    found = true;
                }
            }
        }
        found
    };
    assert!(leaks(&base));
    publish(&library, &id, false);
    assert!(!leaks(&base));
    publish(&library, &id, true);
    assert!(leaks(&base));
    library.set_codex_policy(&id, false, false).unwrap();
    library.sync_codex_entries().unwrap();
    assert!(!leaks(&base));
}

#[test]
fn embedded_entry_text_written_by_skilvella_has_no_dollar_sign() {
    let (base, library, _id) = embed_setup();
    rewrite_body(&base, &library, "context: fork\n", "BODY\n");
    let claude = claude_entry_text(&base);
    assert!(!claude.contains('$'), "{claude}");
    // 本文に $ があっても、それ以外の生成文には現れない。
    rewrite_body(&base, &library, "context: fork\n", "cost $ARGUMENTS ${x}\n");
    let claude = claude_entry_text(&base);
    assert!(claude.contains("cost $ARGUMENTS ${x}\n"), "{claude}");
    let without = claude.replacen("cost $ARGUMENTS ${x}\n", "", 1);
    assert!(!without.contains('$'), "{claude}");
}

#[test]
fn non_fork_claude_entries_and_other_clients_keep_the_legacy_bytes() {
    let (base, library, _id) = embed_setup();
    rewrite_body(
        &base,
        &library,
        "model: opus\narguments: a b\n",
        "PRIVATE ORIGINAL BODY\n",
    );
    let claude = claude_entry_text(&base);
    assert_legacy(&claude);
    assert!(!claude.contains("PRIVATE ORIGINAL BODY") && !claude.contains("\narguments: "));
    let grok_plain = grok_entry_text(&base);
    // 継承行を除けば Claude Code の入口は他クライアントと同じ文面。
    assert_eq!(claude.replace("model: \"opus\"\n", ""), grok_plain);
    assert!(grok_plain.ends_with(LEGACY_TAIL));

    // fork の原本でも、他クライアントの入口は fork でない原本のときとバイト列が同じ。
    rewrite_body(
        &base,
        &library,
        "context: fork\narguments: a b\n",
        "PRIVATE ORIGINAL BODY\n",
    );
    assert_eq!(grok_entry_text(&base), grok_plain);
    assert!(!grok_entry_text(&base).contains("PRIVATE ORIGINAL BODY"));
}
