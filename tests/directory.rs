use skilvella::{
    library::Library,
    model::{SearchQuery, SkillMetadata},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

// リンク作成は codex_index.rs と共有するフィクスチャを使う（本体の実装とは独立）。
#[path = "support/link.rs"]
mod link;
use link::link_directory;

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
    // 新規に見つかったスキルは、読み取りエラーがなければ公開状態で登録される。
    assert!(all(&l).iter().all(|s| s.published));
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
fn automatic_selection_ignores_owned_entries_and_still_finds_original_skills() {
    let b = base();
    let home = b.join("home");
    let l = Library::open(&b.join("data")).unwrap();
    l.initialize_directory(&home, None).unwrap();
    let fallback = l.directory_settings().unwrap();
    let original = Path::new(&fallback.path).join("one");
    skill(&original, "Original");
    l.scan().unwrap();
    let id = all(&l)[0].id.clone();
    publish(&l, &id);
    l.set_codex_policy(&id, false, true).unwrap();
    l.set_codex_enabled(true).unwrap();
    l.sync_codex_entries().unwrap();
    let export = PathBuf::from(l.codex_settings().unwrap().path);
    assert!(export.join("skilvella-search/SKILL.md").is_file());
    let original_bytes = fs::read(original.join("SKILL.md")).unwrap();
    drop(l);

    for _ in 0..2 {
        let reopened = Library::open(&b.join("data")).unwrap();
        reopened.initialize_directory(&home, None).unwrap();
        assert_eq!(reopened.directory_settings().unwrap().path, fallback.path);
        assert_eq!(
            reopened.directory_settings().unwrap().source_alias,
            fallback.source_alias
        );
        assert_eq!(all(&reopened).len(), 1);
        assert_eq!(all(&reopened)[0].id, id);
        assert!(all(&reopened)[0].published);
        reopened.sync_codex_entries().unwrap();
        assert!(reopened.codex_settings().unwrap().last_error.is_empty());
    }
    assert_eq!(fs::read(original.join("SKILL.md")).unwrap(), original_bytes);

    // 所有出力の子を直接ソースとして登録しても、入口を原本に取り込まない。
    let check = Library::open(&b.join("data")).unwrap();
    check
        .add_source("entry", &export.join("skilvella-search"))
        .unwrap();
    assert_eq!(all(&check).len(), 1);
    assert_eq!(all(&check)[0].id, id);
    check.remove_source("entry").unwrap();

    // 手編集した既知の入口を原本として再登録せず、同期の衝突として保護する。
    let finder = export.join("skilvella-search/SKILL.md");
    let saved = fs::read(&finder).unwrap();
    fs::write(
        &finder,
        "---\nname: edited\ndescription: Edited entry\n---\nkeep\n",
    )
    .unwrap();
    check.initialize_directory(&home, None).unwrap();
    assert_eq!(check.directory_settings().unwrap().path, fallback.path);
    assert!(check.sync_codex_entries().is_err());
    assert!(fs::read_to_string(&finder).unwrap().contains("keep"));
    fs::write(&finder, &saved).unwrap();
    check.sync_codex_entries().unwrap();

    // 一つの入口をリンクへ差し替えても、他の正常な入口まで原本扱いしない。
    let command = fs::read_dir(&export)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.is_dir() && p.file_name().unwrap() != "skilvella-search")
        .unwrap();
    fs::rename(&command, b.join("saved-command")).unwrap();
    let external = b.join("external");
    skill(&external, "External original");
    link_directory(&external, &command);
    check.initialize_directory(&home, None).unwrap();
    assert_eq!(check.directory_settings().unwrap().path, fallback.path);
    assert_eq!(all(&check).len(), 1);
    assert!(check.sync_codex_entries().is_err());
    fs::rename(&command, b.join("saved-index-link")).unwrap();
    fs::rename(b.join("saved-command"), &command).unwrap();
    check.sync_codex_entries().unwrap();

    // 生成先全体が原本へのリンクになっても、そのリンク先を保留領域にしない。
    fs::rename(&export, b.join("saved-whole-index")).unwrap();
    link_directory(Path::new(&fallback.path), &export);
    check.initialize_directory(&home, None).unwrap();
    assert_eq!(check.directory_settings().unwrap().path, fallback.path);
    assert!(check.detail(&id, true).is_ok());
    assert!(check.sync_codex_entries().is_err());
    fs::rename(&export, b.join("saved-whole-link")).unwrap();
    fs::rename(b.join("saved-whole-index"), &export).unwrap();
    check.sync_codex_entries().unwrap();

    // 未知の原本を出力先に追加しても隠さず、元のファイルを保護する。
    skill(&export.join("user"), "Unknown original");
    check.initialize_directory(&home, None).unwrap();
    assert_eq!(all(&check).len(), 1);
    assert_eq!(all(&check)[0].name, "Unknown original");
    assert!(check.sync_codex_entries().is_err());
    fs::rename(export.join("user"), b.join("saved-user-skill")).unwrap();
    check.initialize_directory(&home, None).unwrap();
    assert_eq!(check.directory_settings().unwrap().path, fallback.path);
    assert_eq!(all(&check)[0].id, id);

    // 出力が消えた後の空フォルダーは、従来どおり既存候補として扱う。
    fs::rename(&export, b.join("saved-export")).unwrap();
    check.initialize_directory(&home, None).unwrap();
    assert_eq!(
        Path::new(&check.directory_settings().unwrap().path),
        home.join(".agents/skills").canonicalize().unwrap()
    );
    fs::rename(b.join("saved-export"), &export).unwrap();
    check.initialize_directory(&home, None).unwrap();
    assert_eq!(check.directory_settings().unwrap().path, fallback.path);

    // 生成物だけの優先候補を飛ばし、次の標準探索先にある原本を選べる。
    let legacy = home.join(".codex/skills");
    skill(&legacy.join("real"), "Legacy original");
    let reopened = Library::open(&b.join("data")).unwrap();
    reopened.initialize_directory(&home, None).unwrap();
    assert_eq!(
        Path::new(&reopened.directory_settings().unwrap().path),
        legacy.canonicalize().unwrap()
    );
    assert_eq!(all(&reopened).len(), 1);
    assert_eq!(all(&reopened)[0].name, "Legacy original");
    reopened.sync_codex_entries().unwrap();

    // 同じ候補内に原本が増えた場合は優先順位を維持し、生成物を索引に混ぜない。
    skill(&home.join(".agents/skills/real"), "Preferred original");
    reopened.initialize_directory(&home, None).unwrap();
    assert_eq!(
        Path::new(&reopened.directory_settings().unwrap().path),
        home.join(".agents/skills").canonicalize().unwrap()
    );
    let indexed = all(&reopened);
    assert_eq!(
        indexed
            .iter()
            .filter(|s| !s.missing)
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Preferred original"]
    );
    assert!(
        indexed
            .iter()
            .any(|s| s.name == "Unknown original" && s.missing)
    );
    let generated = fs::read(export.join("skilvella-search/SKILL.md")).unwrap();
    assert!(
        reopened
            .sync_codex_entries()
            .unwrap_err()
            .to_string()
            .contains("overlaps")
    );
    assert_eq!(
        fs::read(export.join("skilvella-search/SKILL.md")).unwrap(),
        generated
    );
    assert_eq!(fs::read(original.join("SKILL.md")).unwrap(), original_bytes);
}

#[test]
fn an_unowned_folder_named_skilvella_remains_a_normal_source() {
    let b = base();
    let home = b.join("home");
    skill(&home.join(".agents/skills/skilvella/real"), "User original");
    let l = Library::open(&b.join("data")).unwrap();
    l.initialize_directory(&home, None).unwrap();
    assert_eq!(all(&l).len(), 1);
    assert_eq!(all(&l)[0].name, "User original");
}

#[test]
fn uncertain_records_hold_the_index_of_an_already_selected_native_source() {
    let b = base();
    let home = b.join("home");
    let l = Library::open(&b.join("data")).unwrap();
    l.initialize_directory(&home, None).unwrap();
    l.set_codex_enabled(true).unwrap();
    skill(&home.join(".agents/skills/real"), "Real original");
    l.initialize_directory(&home, None).unwrap();
    let current = l.directory_settings().unwrap();
    let id = all(&l)[0].id.clone();
    publish(&l, &id);
    assert!(l.detail(&id, true).is_ok());
    #[cfg(any(windows, target_os = "macos"))]
    {
        let export = home.join(".agents/skills/skilvella");
        let renamed = home.join(".agents/skills/Skilvella");
        fs::rename(&export, &renamed).unwrap();
        if !export.exists() {
            fs::rename(renamed, export).unwrap();
        } // 大小文字を区別するFSでは通常ケースを確認。
    }
    let path = b.join("data/codex-index/manifest.json");
    let saved = fs::read(&path).unwrap();
    let mut manifest: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    let files = manifest["files"].as_object_mut().unwrap();
    let hash = files.remove("skilvella-search/SKILL.md").unwrap();
    files.insert("../outside/SKILL.md".into(), hash);
    fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    fs::write(
        home.join(".agents/skills/real/SKILL.md"),
        "---\nname: Real original\ndescription: Refreshed sibling\n---\nafter\n",
    )
    .unwrap();
    l.initialize_directory(&home, None).unwrap();
    assert_eq!(l.directory_settings().unwrap().path, current.path);
    assert_eq!(all(&l).len(), 1);
    assert_eq!(all(&l)[0].id, id);
    assert!(all(&l)[0].published);
    assert!(all(&l)[0].error.is_none());
    assert_eq!(all(&l)[0].description, "Refreshed sibling");
    assert!(l.detail(&id, true).unwrap().content.ends_with("after\n"));
    // 記録全体が壊れ、固定名の予定先しか分からなくても、大文字小文字の別名を辿らない。
    fs::write(&path, "{").unwrap();
    l.initialize_directory(&home, None).unwrap();
    assert_eq!(l.scan().unwrap().errors, 1);
    assert_eq!(all(&l).len(), 1);
    assert!(l.detail(&id, true).is_ok());
    fs::write(&path, saved).unwrap();
    l.scan().unwrap();
    assert!(l.detail(&id, true).is_ok());
    assert_eq!(all(&l).len(), 1);
}

#[test]
fn automatic_selection_distinguishes_staged_and_published_first_entries() {
    for published in [false, true] {
        let b = base();
        let home = b.join("home");
        let l = Library::open(&b.join("data")).unwrap();
        l.initialize_directory(&home, None).unwrap();
        let fallback = l.directory_settings().unwrap();
        skill(&Path::new(&fallback.path).join("one"), "original");
        l.scan().unwrap();
        publish(&l, &all(&l)[0].id);
        l.set_codex_enabled(true).unwrap();
        let export = PathBuf::from(l.codex_settings().unwrap().path);
        let state = b.join("data/codex-index");
        let next: serde_json::Value =
            serde_json::from_slice(&fs::read(state.join("manifest.json")).unwrap()).unwrap();
        let agents = export.parent().unwrap().parent().unwrap();
        let stage = agents
            .join("skilvella-staging")
            .join(uuid::Uuid::new_v4().to_string());
        let retired = PathBuf::new();
        let previous = serde_json::json!({"owner": next["owner"], "root": "", "files": {}});
        fs::rename(state.join("manifest.json"), b.join("saved-manifest.json")).unwrap();
        fs::write(state.join("pending.json"), serde_json::to_vec(&serde_json::json!({"previous": previous,"next": next,"stage": stage,"retired": retired})).unwrap()).unwrap();
        if !published {
            fs::create_dir_all(stage.parent().unwrap()).unwrap();
            fs::rename(&export, &stage).unwrap();
            skill(&export.join("skilvella-search"), "User replacement");
        }
        let reopened = Library::open(&b.join("data")).unwrap();
        reopened.initialize_directory(&home, None).unwrap();
        if published {
            assert_eq!(reopened.directory_settings().unwrap().path, fallback.path);
            reopened.sync_codex_entries().unwrap();
            assert_eq!(all(&reopened)[0].name, "original");
            assert!(!state.join("pending.json").exists());
        } else {
            assert_eq!(all(&reopened).len(), 1);
            assert_eq!(all(&reopened)[0].name, "User replacement");
            assert!(reopened.sync_codex_entries().is_err());
            assert!(
                fs::read_to_string(export.join("skilvella-search/SKILL.md"))
                    .unwrap()
                    .contains("User replacement")
            );
        }
    }
}

#[test]
fn an_original_at_the_retired_output_location_is_not_excluded() {
    let b = base();
    let home = b.join("home");
    let l = Library::open(&b.join("data")).unwrap();
    l.initialize_directory(&home, None).unwrap();
    l.set_codex_enabled(true).unwrap();
    let export = PathBuf::from(l.codex_settings().unwrap().path);
    let state = b.join("data/codex-index");
    let previous: serde_json::Value =
        serde_json::from_slice(&fs::read(state.join("manifest.json")).unwrap()).unwrap();
    let agents = export.parent().unwrap().parent().unwrap();
    let stage = agents
        .join("skilvella-staging")
        .join(uuid::Uuid::new_v4().to_string());
    let retired = agents
        .join("skilvella-retired")
        .join(uuid::Uuid::new_v4().to_string());
    for relative in previous["files"].as_object().unwrap().keys() {
        let file = stage.join(relative);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::copy(export.join(relative), file).unwrap();
    }
    fs::create_dir_all(retired.parent().unwrap()).unwrap();
    fs::rename(&export, &retired).unwrap();
    skill(&export.join("skilvella-search"), "User replacement");
    fs::write(state.join("pending.json"), serde_json::to_vec(&serde_json::json!({"previous":previous,"next":previous,"stage":stage,"retired":retired})).unwrap()).unwrap();
    l.initialize_directory(&home, None).unwrap();
    assert_eq!(all(&l).len(), 1);
    assert_eq!(all(&l)[0].name, "User replacement");
    assert!(l.sync_codex_entries().is_err());
    assert!(stage.join("skilvella-search/SKILL.md").is_file());
    assert!(retired.join("skilvella-search/SKILL.md").is_file());
    assert!(
        fs::read_to_string(export.join("skilvella-search/SKILL.md"))
            .unwrap()
            .contains("User replacement")
    );
}

#[test]
fn unverified_ownership_never_becomes_a_confirmed_automatic_directory() {
    for mode in ["auto", "custom", "disabled"] {
        for problem in ["invalid-marker", "foreign-owner", "corrupt-json"] {
            let b = base();
            let home = b.join("home");
            let l = Library::open(&b.join("data")).unwrap();
            l.initialize_directory(&home, None).unwrap();
            let fallback = l.directory_settings().unwrap();
            skill(&Path::new(&fallback.path).join("one"), "original");
            l.scan().unwrap();
            let id = all(&l)[0].id.clone();
            publish(&l, &id);
            l.set_codex_policy(&id, false, true).unwrap();
            l.set_codex_enabled(true).unwrap();
            l.set_directory(mode, &fallback.path).unwrap();
            let before = l.directory_settings().unwrap();
            let path = b.join("data/codex-index/manifest.json");
            let mut manifest: serde_json::Value =
                serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            match problem {
                "invalid-marker" => {
                    let files = manifest["files"].as_object_mut().unwrap();
                    let hash = files.remove("original/SKILL.md").unwrap();
                    files.insert("../outside/SKILL.md".into(), hash);
                    fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
                }
                "foreign-owner" => {
                    manifest["owner"] = serde_json::json!(b.join("other/library.sqlite3"));
                    fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
                }
                _ => fs::write(&path, "{").unwrap(),
            }
            l.initialize_directory(&home, None).unwrap();
            let after = l.directory_settings().unwrap();
            assert_eq!(after.path, before.path);
            assert_eq!(after.mode, before.mode);
            assert_eq!(after.source_alias, before.source_alias);
            assert!(after.detected_path.is_empty());
            assert!(l.set_directory("auto", "").is_err());
            assert_eq!(l.directory_settings().unwrap().path, before.path);
        }
    }
}

#[cfg(unix)]
#[test]
fn unreadable_candidates_keep_the_current_source_and_retire_old_entry_rows() {
    use std::os::unix::fs::PermissionsExt;
    struct Restore(PathBuf);
    impl Drop for Restore {
        fn drop(&mut self) {
            let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o700));
        }
    }
    let b = base();
    let home = b.join("home");
    let l = Library::open(&b.join("data")).unwrap();
    l.initialize_directory(&home, None).unwrap();
    let fallback = l.directory_settings().unwrap();
    skill(&Path::new(&fallback.path).join("one"), "original");
    l.scan().unwrap();
    let original = all(&l)[0].id.clone();
    publish(&l, &original);
    l.set_codex_policy(&original, false, true).unwrap();
    l.set_codex_enabled(true).unwrap();
    let native = home.join(".agents/skills");
    let denied = native.join("denied");
    skill(&denied, "Unreadable original");
    fs::set_permissions(&denied, fs::Permissions::from_mode(0o0)).unwrap();
    let guard = Restore(denied.clone());
    if fs::read_dir(&denied).is_ok() {
        return;
    } // 特権実行では読取拒否条件を作れない。
    l.initialize_directory(&home, None).unwrap();
    assert_eq!(l.directory_settings().unwrap().path, fallback.path);
    assert_eq!(all(&l)[0].id, original);
    drop(guard);

    let owned_entry = native.join("skilvella/original");
    fs::set_permissions(&owned_entry, fs::Permissions::from_mode(0o0)).unwrap();
    let guard = Restore(owned_entry);
    l.initialize_directory(&home, None).unwrap();
    assert_eq!(l.directory_settings().unwrap().path, fallback.path);
    drop(guard);

    // 旧版で入口を索引済みの状態を作り、部分走査失敗でもその行だけは無効化する。
    let state = b.join("data/codex-index/manifest.json");
    let saved = b.join("saved-manifest.json");
    fs::rename(&state, &saved).unwrap();
    l.set_directory("custom", native.to_str().unwrap()).unwrap();
    let old_entries = all(&l)
        .into_iter()
        .filter(|s| s.relative_path.starts_with("skilvella/"))
        .map(|s| s.id)
        .collect::<Vec<_>>();
    assert_eq!(old_entries.len(), 2);
    for id in &old_entries {
        publish(&l, id);
    }
    fs::rename(&saved, &state).unwrap();
    fs::set_permissions(&denied, fs::Permissions::from_mode(0o0)).unwrap();
    let _guard = Restore(denied);
    assert!(l.scan().unwrap().errors > 0);
    for id in old_entries {
        assert!(all(&l).iter().find(|s| s.id == id).unwrap().missing);
        assert!(l.detail(&id, true).is_err());
    }
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
    // 別の保存先の新規行は元の行とは別IDで、走査の既定どおり公開状態になる（元の公開状態を引き継ぐわけではない）。
    assert!(custom.published);
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
    // 新しい保存先の行は別IDの新規行で、走査の既定どおり公開状態になる。
    assert!(all(&reopened)[0].published);
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

/// Claude Code・Grokの生成物も、Codexと同じく管理フォルダー内で原本として取り込まない。
#[test]
fn client_entries_are_never_ingested_as_original_skills() {
    use skilvella::model::SkillClient;
    let b = base();
    let home = b.join("home");
    let source = b.join("source");
    skill(&source.join("one"), "original");
    let l = Library::open_for_user(&b.join("data"), &home, None).unwrap();
    l.add_source("source", &source).unwrap();
    let id = all(&l)[0].id.clone();
    publish(&l, &id);
    l.set_codex_policy(&id, false, true).unwrap();
    l.set_clients_enabled(&[(SkillClient::Claude, true), (SkillClient::Grok, true)])
        .unwrap();
    assert!(l.sync_summary().unwrap().last_error.is_empty());
    for (alias, folder) in [
        ("grok", home.join(".grok/skills")),
        ("claude-store", home.join(".claude/skilvella")),
        ("claude-links", home.join(".claude/skills")),
    ] {
        l.add_source(alias, &folder).unwrap();
        l.scan().unwrap();
        let names: Vec<_> = all(&l)
            .into_iter()
            .filter(|skill| !skill.missing)
            .map(|skill| skill.name)
            .collect();
        assert_eq!(names, ["original"], "{alias}");
        l.remove_source(alias).unwrap();
    }
}

/// 利用者が選んだフォルダーの登録は識別子を入力させず、過去の登録や既定フォルダーと
/// 名前が衝突しても登録できる（別PCで「Alias already belongs to another directory」に
/// なった不具合の回帰テスト）。
#[test]
fn added_folders_get_unique_identifiers_and_reuse_only_the_same_folder() {
    use skilvella::model::FolderError;
    let b = base();
    let home = b.join("home");
    let auto = home.join(".agents/skills");
    skill(&auto.join("auto"), "auto");
    let first = b.join("one/skills");
    let second = b.join("two/skills");
    let japanese = b.join("スキル集");
    skill(&first.join("a"), "first");
    skill(&second.join("b"), "second");
    skill(&japanese.join("c"), "japanese");
    let l = Library::open_for_user(&b.join("data"), &home, None).unwrap();
    l.initialize_directory(&home, None).unwrap();
    assert_eq!(l.directory_settings().unwrap().source_alias, "default");

    // 同名フォルダーは連番の識別子になり、表示名はフォルダー名のまま。
    let (one, _) = l.add_folder(&first).unwrap();
    let (two, _) = l.add_folder(&second).unwrap();
    assert_eq!((one.as_str(), two.as_str()), ("skills", "skills-2"));
    let (third, _) = l.add_folder(&japanese).unwrap();
    assert_eq!(third, "folder");
    let sources = l.sources().unwrap();
    let label = |alias: &str| {
        sources
            .iter()
            .find(|s| s.alias == alias)
            .unwrap()
            .display_name
            .clone()
    };
    assert_eq!(label("skills-2"), "skills");
    assert_eq!(label("folder"), "スキル集");

    // 管理から外した後、同じ識別子の別フォルダーを登録しても衝突しない。
    let first_id = all(&l).into_iter().find(|s| s.name == "first").unwrap().id;
    publish(&l, &first_id);
    l.remove_source(&one).unwrap();
    let elsewhere = b.join("three/skills");
    skill(&elsewhere.join("d"), "elsewhere");
    let (fresh, _) = l.add_folder(&elsewhere).unwrap();
    assert_eq!(fresh, "skills-3");

    // 同じフォルダーを再登録すると、以前の識別子・スキルID・公開設定を引き継ぐ。
    let (again, _) = l.add_folder(&first).unwrap();
    assert_eq!(again, one);
    let restored = all(&l).into_iter().find(|s| s.name == "first").unwrap();
    assert_eq!(restored.id, first_id);
    assert!(restored.published);

    // 管理中のフォルダーは表記揺れがあっても重複登録せず、表示名を示す。
    let spelled = PathBuf::from(
        second
            .to_string_lossy()
            .replace('/', std::path::MAIN_SEPARATOR_STR),
    );
    let spelled = if cfg!(windows) {
        PathBuf::from(spelled.to_string_lossy().to_uppercase())
    } else {
        spelled
    };
    let error = l.add_folder(&spelled).unwrap_err();
    assert_eq!(
        error.downcast_ref::<FolderError>(),
        Some(&FolderError::AlreadyRegistered {
            name: "skills".into()
        })
    );

    // 親子関係にあるフォルダーは、重なる側の表示名と場所を示して拒否する。
    let error = l.add_folder(&b.join("two")).unwrap_err();
    assert!(matches!(
        error.downcast_ref::<FolderError>(),
        Some(FolderError::Overlaps { name, .. }) if name == "skills"
    ));
    assert!(format!("{error:#}").contains("overlap"));

    // 既定フォルダーの識別子 `default` と同じ名前のフォルダーも登録できる。
    let named_default = b.join("default");
    skill(&named_default.join("e"), "named default");
    let (alias, _) = l.add_folder(&named_default).unwrap();
    assert_eq!(alias, "default-2");
}

/// 旧来の明示識別子でも、同じフォルダーの再登録は表記揺れを区別しない。
#[test]
fn explicit_alias_reregistration_compares_canonical_paths() {
    let b = base();
    let root = b.join("source");
    skill(&root.join("one"), "one");
    let l = Library::open(&b.join("data")).unwrap();
    l.add_source("work", &root).unwrap();
    l.remove_source("work").unwrap();
    // Windowsでは大文字小文字の違う表記で再登録しても同じフォルダーとして扱う。
    let spelled = if cfg!(windows) {
        PathBuf::from(root.to_string_lossy().to_uppercase())
    } else {
        root.clone()
    };
    l.add_source("work", &spelled).unwrap();
    assert_eq!(l.sources().unwrap().len(), 1);
    let other = b.join("other");
    skill(&other.join("two"), "two");
    assert!(
        format!("{:#}", l.add_source("work", &other).unwrap_err())
            .contains("Alias already belongs")
    );
}
