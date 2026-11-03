use rusqlite::Connection;
use skilvella::{
    library::Library,
    model::{SearchQuery, SkillMetadata},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn skill(root: &Path, name: &str) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Migration fixture\n---\n{name} instructions\n"),
    )
    .unwrap();
}

fn fixture() -> (PathBuf, Library, PathBuf, String) {
    let base = std::env::var_os("SKILVELLA_MIGRATION_TEST_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let root = std::env::temp_dir();
            #[cfg(unix)]
            let root = root.canonicalize().unwrap();
            root.join("skilvella-migration-tests")
        })
        .join(uuid::Uuid::new_v4().to_string());
    let root = base.join("home/.agents/skills");
    skill(&root.join("parent"), "Parent");
    skill(&root.join("parent/child"), "Child");
    fs::write(root.join("parent/reference.md"), "reference text\n").unwrap();
    let library = Library::open(&base.join("data")).unwrap();
    library
        .initialize_directory(&base.join("home"), None)
        .unwrap();
    let alias = library.directory_settings().unwrap().source_alias;
    (base, library, root.canonicalize().unwrap(), alias)
}

fn db(base: &Path) -> Connection {
    Connection::open(base.join("data/library.sqlite3")).unwrap()
}

fn pending(base: &Path, alias: &str, from: &Path, to: &Path) {
    db(base)
        .execute(
            "INSERT INTO settings(key,value) VALUES('pending_migration',?1)",
            [serde_json::json!({"alias": alias, "from": from, "to": to, "identity": directory_identity(from)}).to_string()],
        )
        .unwrap();
}

fn directory_identity(path: &Path) -> serde_json::Value {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = fs::metadata(path).unwrap();
        serde_json::json!({ "volume": metadata.dev(), "file": metadata.ino() })
    }
    #[cfg(windows)]
    {
        use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
        use windows_sys::Win32::Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS, GetFileInformationByHandle,
        };
        let directory = fs::OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(path)
            .unwrap();
        let mut information = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
        assert_ne!(
            unsafe {
                GetFileInformationByHandle(directory.as_raw_handle() as _, information.as_mut_ptr())
            },
            0
        );
        let information = unsafe { information.assume_init() };
        serde_json::json!({ "volume": u64::from(information.dwVolumeSerialNumber), "file": (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow) })
    }
}

fn pending_count(base: &Path) -> i64 {
    db(base)
        .query_row(
            "SELECT count(*) FROM settings WHERE key='pending_migration'",
            [],
            |row| row.get(0),
        )
        .unwrap()
}

#[test]
fn whole_root_migration_keeps_identity_history_references_and_git_clone() {
    let (base, library, from, alias) = fixture();
    let parent = library
        .search(
            SearchQuery {
                q: "Parent".into(),
                ..SearchQuery::default()
            },
            false,
        )
        .unwrap()
        .skills
        .into_iter()
        .find(|skill| skill.name == "Parent")
        .unwrap();
    let before = library.detail(&parent.id, false).unwrap();
    library
        .save_content(
            &parent.id,
            &before.revision,
            &(before.content.clone() + "edited\n"),
        )
        .unwrap();
    library
        .set_metadata(
            &parent.id,
            SkillMetadata {
                tags: vec!["kept".into()],
                category: "migration".into(),
                published: false,
            },
        )
        .unwrap();
    fs::create_dir_all(from.join(".git/objects")).unwrap();
    fs::write(from.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
    let git_config = "[core]\nrepositoryformatversion = 0\nbare = false\n[remote \"origin\"]\nurl = https://example.invalid/skills.git\n";
    fs::write(from.join(".git/config"), git_config).unwrap();
    fs::create_dir_all(from.join("broken")).unwrap();
    fs::write(from.join("broken/SKILL.md"), "invalid frontmatter").unwrap();
    library.scan().unwrap();
    assert!(library.sources().unwrap()[0].native_discovery);
    assert_eq!(library.sources().unwrap()[0].mcp_candidate_count, 2);
    let dedicated = library.dedicated_directory().unwrap();
    assert_eq!(dedicated, base.canonicalize().unwrap().join("data-library"));
    assert!(!dedicated.exists());
    let to = dedicated.join(&alias);
    #[cfg(windows)]
    let input = PathBuf::from(
        to.to_string_lossy()
            .strip_prefix("\\\\?\\")
            .unwrap()
            .to_owned(),
    );
    #[cfg(not(windows))]
    let input = to.clone();
    let report = library.migrate_source(&alias, &input).unwrap();
    assert_eq!(report.indexed, 3);
    assert_eq!(report.errors, 1);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&dedicated).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
    assert!(!from.exists());
    assert!(to.join("parent/child/SKILL.md").is_file());
    assert_eq!(
        fs::read_to_string(to.join(".git/HEAD")).unwrap(),
        "ref: refs/heads/main\n"
    );
    assert_eq!(
        fs::read_to_string(to.join(".git/config")).unwrap(),
        git_config
    );
    assert_eq!(
        library.search(SearchQuery::default(), true).unwrap().total,
        2
    );
    let after = library.detail(&parent.id, true).unwrap();
    assert_eq!(after.summary.source, alias);
    assert_eq!(after.summary.tags, vec!["kept"]);
    assert_eq!(after.summary.category, "migration");
    assert_eq!(after.history.len(), 1);
    assert!(after.content.ends_with("edited\n"));
    assert_eq!(
        library
            .read_file(&parent.id, "reference.md", 1, 10)
            .unwrap()
            .text,
        "reference text"
    );
    assert!(
        library
            .read_file(&parent.id, "child/SKILL.md", 1, 10)
            .is_err()
    );
    let settings = library.directory_settings().unwrap();
    assert_eq!(settings.mode, "custom");
    assert_eq!(Path::new(&settings.path), to);
    assert_eq!(Path::new(&settings.custom_path), to);
    assert!(!library.sources().unwrap()[0].native_discovery);
    library
        .initialize_directory(&base.join("home"), None)
        .unwrap();
    assert_eq!(library.directory_settings().unwrap().mode, "custom");
    assert_eq!(pending_count(&base), 0);
}

#[test]
fn rejects_existing_overlapping_native_and_worktree_targets_without_moving() {
    let (base, library, from, alias) = fixture();
    let existing = base.join("existing");
    fs::create_dir_all(&existing).unwrap();
    fs::write(existing.join("keep.txt"), "keep").unwrap();
    let other = base.join("other");
    skill(&other, "Other");
    library.add_source("other", &other).unwrap();
    let standard = base.join("repo/.codex/skills");
    let mixed_case_standard = base.join("repo/.Agents/Skills");
    let custom = base.join("custom-codex/skills");
    fs::create_dir_all(&standard).unwrap();
    fs::create_dir_all(&mixed_case_standard).unwrap();
    fs::create_dir_all(&custom).unwrap();
    library
        .initialize_directory(&base.join("home"), Some(&base.join("custom-codex")))
        .unwrap();
    let targets = [
        existing.clone(),
        from.join("nested"),
        base.join("data/nested"),
        other.join("nested"),
        standard.join("nested"),
        mixed_case_standard.join("nested"),
        custom.join("nested"),
    ];
    for target in targets {
        assert!(
            library.migrate_source(&alias, &target).is_err(),
            "{}",
            target.display()
        );
        assert!(from.join("parent/SKILL.md").is_file());
        assert_eq!(pending_count(&base), 0);
    }
    assert_eq!(
        fs::read_to_string(existing.join("keep.txt")).unwrap(),
        "keep"
    );
    fs::write(from.join(".git"), "gitdir: ../external-worktree\n").unwrap();
    assert!(
        library
            .migrate_source(&alias, &base.join("dedicated"))
            .is_err()
    );
    assert!(from.exists());
    assert_eq!(pending_count(&base), 0);
    for (pointer, content) in [
        ("objects/info/alternates", "../../../../shared-objects\n"),
        ("worktrees/external/gitdir", "../../../../external/.git\n"),
        ("commondir", "../../external-common\n"),
        ("config.worktree", "[core]\nworktree = ../../external\n"),
        ("config", "[core]\n\tWorkTree = ../../external\n"),
    ] {
        let (base, library, from, alias) = fixture();
        let marker = from.join(".git").join(pointer);
        fs::create_dir_all(marker.parent().unwrap()).unwrap();
        fs::write(&marker, content).unwrap();
        assert!(
            library
                .migrate_source(&alias, &base.join("dedicated"))
                .is_err(),
            "{pointer}"
        );
        assert!(from.join("parent/SKILL.md").is_file());
        assert_eq!(fs::read_to_string(&marker).unwrap(), content);
        assert_eq!(pending_count(&base), 0);
    }
    let (base, library, from, alias) = fixture();
    fs::create_dir_all(from.join(".GIT")).unwrap();
    fs::write(from.join(".GIT/commondir"), "../../external-common\n").unwrap();
    assert!(
        library
            .migrate_source(&alias, &base.join("dedicated"))
            .is_err()
    );
    assert!(from.join(".GIT/commondir").is_file());
    assert_eq!(pending_count(&base), 0);
}

#[test]
fn database_failure_returns_original_root_and_leaves_metadata_unchanged() {
    let (base, library, from, alias) = fixture();
    let to = library.dedicated_directory().unwrap();
    db(&base).execute_batch("CREATE TRIGGER reject_migration BEFORE UPDATE OF path ON sources BEGIN SELECT RAISE(ABORT,'fixture failure'); END;").unwrap();
    assert!(library.migrate_source(&alias, &to).is_err());
    assert!(from.join("parent/SKILL.md").is_file());
    assert!(!to.exists());
    assert_eq!(Path::new(&library.sources().unwrap()[0].path), from);
    assert_eq!(
        library.search(SearchQuery::default(), true).unwrap().total,
        0
    );
    assert_eq!(library.directory_settings().unwrap().mode, "auto");
    assert_eq!(pending_count(&base), 0);
}

#[test]
fn startup_recovers_before_and_after_atomic_move() {
    for moved in [false, true] {
        let (base, library, from, alias) = fixture();
        let to = library.dedicated_directory().unwrap();
        pending(&base, &alias, &from, &to);
        if moved {
            fs::rename(&from, &to).unwrap();
        }
        drop(library);
        let recovered = Library::open(&base.join("data")).unwrap();
        assert_eq!(pending_count(&base), 0);
        let settings = recovered.directory_settings().unwrap();
        if moved {
            assert!(!from.exists());
            assert_eq!(Path::new(&settings.path), to);
            assert_eq!(settings.mode, "custom");
            assert_eq!(
                recovered
                    .search(SearchQuery::default(), true)
                    .unwrap()
                    .total,
                2
            );
        } else {
            assert!(from.exists());
            assert!(!to.exists());
            assert_eq!(Path::new(&settings.path), from);
            assert_eq!(
                recovered
                    .search(SearchQuery::default(), true)
                    .unwrap()
                    .total,
                0
            );
        }
    }
}

#[test]
fn startup_uses_current_native_roots_before_recovering_a_moved_directory() {
    let (base, library, from, alias) = fixture();
    let codex_home = base.join("new-codex-home");
    fs::create_dir_all(&codex_home).unwrap();
    let to = codex_home.join("skills");
    pending(&base, &alias, &from, &to);
    fs::rename(&from, &to).unwrap();
    drop(library);
    assert!(
        Library::open_for_user(&base.join("data"), &base.join("home"), Some(&codex_home)).is_err()
    );
    assert_eq!(pending_count(&base), 1);
    assert!(!from.exists());
    assert!(to.join("parent/SKILL.md").is_file());
    assert_eq!(
        db(&base)
            .query_row("SELECT count(*) FROM skills WHERE published=1", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        0
    );
    assert_eq!(
        db(&base)
            .query_row("SELECT path FROM sources WHERE alias=?1", [&alias], |row| {
                row.get::<_, String>(0)
            })
            .unwrap(),
        from.to_string_lossy()
    );
}

#[test]
fn ambiguous_recovery_keeps_journal_and_never_overwrites_either_root() {
    for both in [false, true] {
        let (base, library, from, alias) = fixture();
        let to = library.dedicated_directory().unwrap();
        pending(&base, &alias, &from, &to);
        if both {
            fs::create_dir_all(&to).unwrap();
            fs::write(to.join("keep.txt"), "keep").unwrap();
        } else {
            fs::rename(&from, from.with_file_name("parked-original")).unwrap();
        }
        assert!(library.scan().is_err());
        drop(library);
        assert!(Library::open(&base.join("data")).is_err());
        assert_eq!(pending_count(&base), 1);
        if both {
            assert!(from.join("parent/SKILL.md").is_file());
            assert_eq!(fs::read_to_string(to.join("keep.txt")).unwrap(), "keep");
        }
    }
}

#[test]
fn replaced_directories_are_not_recovered_or_published_as_the_original_source() {
    for replace_destination in [false, true] {
        let (base, library, from, alias) = fixture();
        let to = library.dedicated_directory().unwrap();
        pending(&base, &alias, &from, &to);
        let parked = base.join("parked-original");
        fs::rename(&from, &parked).unwrap();
        let replacement = if replace_destination { &to } else { &from };
        skill(&replacement.join("unrelated"), "Unrelated");
        drop(library);
        assert!(Library::open(&base.join("data")).is_err());
        assert_eq!(pending_count(&base), 1);
        assert!(parked.join("parent/SKILL.md").is_file());
        assert!(replacement.join("unrelated/SKILL.md").is_file());
        assert_eq!(
            db(&base)
                .query_row("SELECT count(*) FROM skills WHERE published=1", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap(),
            0
        );
        assert_eq!(
            db(&base)
                .query_row(
                    "SELECT count(*) FROM skills WHERE name='Unrelated'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        assert_eq!(
            db(&base)
                .query_row("SELECT path FROM sources WHERE alias=?1", [&alias], |row| {
                    row.get::<_, String>(0)
                })
                .unwrap(),
            from.to_string_lossy()
        );
    }
}

#[test]
fn linked_source_entries_are_refused_before_journaling() {
    let (base, library, from, alias) = fixture();
    let target = base.join("linked-target");
    fs::create_dir_all(&target).unwrap();
    let link = from.join("linked");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, &link).unwrap();
    #[cfg(windows)]
    {
        let normal = |path: &Path| {
            let text = path.to_string_lossy();
            text.strip_prefix("\\\\?\\").unwrap_or(&text).to_owned()
        };
        let output = std::process::Command::new("cmd.exe")
            .args(["/D", "/C", "mklink", "/J"])
            .arg(normal(&link))
            .arg(normal(&target))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "fixture junction creation failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert!(
        library
            .migrate_source(&alias, &library.dedicated_directory().unwrap())
            .is_err()
    );
    assert!(from.exists());
    assert_eq!(pending_count(&base), 0);
}

#[test]
fn unused_custom_native_root_and_inactive_source_paths_remain_reserved() {
    let (base, library, from, alias) = fixture();
    let custom_home = base.join("custom-native");
    fs::create_dir_all(&custom_home).unwrap();
    library
        .initialize_directory(&base.join("home"), Some(&custom_home))
        .unwrap();
    let native_root = custom_home.join("skills");
    assert!(!native_root.exists());
    assert!(library.migrate_source(&alias, &native_root).is_err());
    #[cfg(windows)]
    {
        let uppercase = PathBuf::from(native_root.to_string_lossy().to_uppercase());
        assert!(library.migrate_source(&alias, &uppercase).is_err());
    }
    assert!(from.exists());
    let reserved = base.join("reserved");
    skill(&reserved, "Reserved");
    library.add_source("reserved", &reserved).unwrap();
    library.remove_source("reserved").unwrap();
    fs::rename(&reserved, base.join("parked-reserved")).unwrap();
    assert!(!reserved.exists());
    assert!(library.migrate_source(&alias, &reserved).is_err());
    assert!(from.exists());
    assert_eq!(pending_count(&base), 0);
}

#[test]
fn repository_subdirectories_are_not_moved_out_of_their_worktree() {
    let (base, library, from, alias) = fixture();
    // source自体の.gitを持つ通常cloneは既存の成功テストで確認する。
    fs::create_dir_all(base.join("home/.git")).unwrap();
    fs::write(base.join("home/.git/HEAD"), "ref: refs/heads/main\n").unwrap();
    assert!(
        library
            .migrate_source(&alias, &base.join("dedicated"))
            .is_err()
    );
    assert!(from.join("parent/SKILL.md").is_file());
    assert_eq!(pending_count(&base), 0);
}

#[test]
fn independent_libraries_and_startup_wait_for_the_same_operation_lock() {
    use std::sync::{Arc, Barrier, mpsc};
    use std::time::Duration;
    let (base, library, from, alias) = fixture();
    let second = Library::open(&base.join("data")).unwrap();
    let operation = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(base.join("data/library.write.lock"))
        .unwrap();
    operation.lock().unwrap();
    let to = library.dedicated_directory().unwrap();
    pending(&base, &alias, &from, &to);
    let barrier = Arc::new(Barrier::new(3));
    let (opened_tx, opened_rx) = mpsc::channel();
    let (created_tx, created_rx) = mpsc::channel();
    let opening = {
        let data = base.join("data");
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            barrier.wait();
            let result = Library::open(&data);
            opened_tx.send(()).unwrap();
            result
        })
    };
    let creating = {
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            barrier.wait();
            let result = second.create_skill(
                &alias,
                "concurrent",
                "---\nname: Concurrent\ndescription: Concurrent create\n---\nbody\n",
            );
            created_tx.send(()).unwrap();
            result
        })
    };
    barrier.wait();
    assert!(matches!(
        opened_rx.recv_timeout(Duration::from_millis(100)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    assert!(matches!(
        created_rx.recv_timeout(Duration::from_millis(100)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    assert_eq!(pending_count(&base), 1);
    assert!(!from.join("concurrent").exists());
    fs::rename(&from, &to).unwrap();
    drop(operation);
    let reopened = opening.join().unwrap().unwrap();
    let created = creating.join().unwrap();
    assert!(!from.exists());
    assert_eq!(pending_count(&base), 0);
    assert_eq!(Path::new(&reopened.sources().unwrap()[0].path), to);
    if created.is_ok() {
        assert!(to.join("concurrent/SKILL.md").is_file());
    }
}

#[test]
fn initial_fallback_and_migration_pool_do_not_overlap() {
    let (base, _previous, from, _) = fixture();
    let home = base.join("empty-home");
    fs::create_dir_all(&home).unwrap();
    let library = Library::open(&base.join("fresh-data")).unwrap();
    library.initialize_directory(&home, None).unwrap();
    let fallback = library.directory_settings().unwrap();
    assert!(Path::new(&fallback.path).ends_with("fresh-data-skills"));
    library.add_source("import", &from).unwrap();
    let to = library.dedicated_directory().unwrap().join("import");
    library.migrate_source("import", &to).unwrap();
    assert!(!from.exists());
    assert_eq!(
        library.search(SearchQuery::default(), true).unwrap().total,
        2
    );
    assert_eq!(
        library.directory_settings().unwrap().source_alias,
        fallback.source_alias
    );
}

#[test]
fn internal_native_skill_directories_are_refused_without_changing_layout() {
    for (marker, skills) in [(".agents", "skills"), (".CODEX", "SKILLS")] {
        let (base, library, from, alias) = fixture();
        let internal = from.join("project").join(marker).join(skills);
        skill(&internal.join("nested"), "Nested native");
        assert!(
            library
                .migrate_source(&alias, &base.join("dedicated"))
                .is_err()
        );
        assert!(internal.join("nested/SKILL.md").is_file());
        assert!(from.join("parent/SKILL.md").is_file());
        assert_eq!(pending_count(&base), 0);
    }
}

#[test]
fn unavailable_source_volume_and_known_native_root_do_not_block_management() {
    let (base, library, from, alias) = fixture();
    #[cfg(windows)]
    let unavailable = {
        let drives = unsafe { windows_sys::Win32::Storage::FileSystem::GetLogicalDrives() };
        assert_ne!(drives, 0, "cannot inspect assigned drives");
        let letter = (b'A'..=b'Z')
            .rev()
            .find(|letter| drives & (1u32 << (letter - b'A')) == 0)
            .expect("test requires an unassigned drive letter");
        PathBuf::from(format!("{}:\\unavailable-codex", char::from(letter)))
    };
    #[cfg(unix)]
    // Unixには未割当ドライブ文字がないため、正規化が必ず失敗する読取境界を使う。
    let unavailable = PathBuf::from("/unavailable\0/codex");
    library
        .initialize_directory(&base.join("home"), Some(&unavailable))
        .unwrap();
    db(&base)
        .execute(
            "INSERT INTO sources(alias,path,active) VALUES('offline',?1,1)",
            [unavailable.join("source").to_str().unwrap()],
        )
        .unwrap();
    let sources = library.sources().unwrap();
    let offline = sources
        .iter()
        .find(|source| source.alias == "offline")
        .unwrap();
    assert!(offline.native_discovery);
    assert!(offline.native_discovery_unknown);
    assert!(library.scan().is_ok());
    assert!(
        library
            .initialize_directory(&base.join("home"), Some(&unavailable))
            .is_ok()
    );
    // 一覧表示のfallbackを、厳格な移動先検査へは流用しない。
    assert!(
        library
            .migrate_source(&alias, &base.join("dedicated"))
            .is_err()
    );
    assert!(from.exists());
    assert_eq!(pending_count(&base), 0);
    drop(library);
    let reopened = Library::open(&base.join("data")).unwrap();
    assert_eq!(reopened.sources().unwrap().len(), 2);
    assert!(reopened.scan().is_ok());

    let (fresh_base, current, fresh_from, fresh_alias) = fixture();
    current
        .initialize_directory(&fresh_base.join("home"), Some(&unavailable))
        .unwrap();
    assert!(
        current
            .migrate_source(&fresh_alias, &fresh_base.join("dedicated"))
            .is_err()
    );
    current
        .initialize_directory(&fresh_base.join("home"), None)
        .unwrap();
    current
        .migrate_source(&fresh_alias, &fresh_base.join("dedicated"))
        .unwrap();
    assert!(!fresh_from.exists());
}
