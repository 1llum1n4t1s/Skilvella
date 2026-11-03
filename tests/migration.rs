use rusqlite::Connection;
use skilvella::{library::Library, model::SearchQuery};
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
fn startup_recovers_before_and_after_atomic_move() {
    for moved in [false, true] {
        let (base, library, from, alias) = fixture();
        let to = base.join("data-library");
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
        let to = base.join("data-library");
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
        let to = base.join("data-library");
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
fn independent_libraries_and_startup_wait_for_the_same_operation_lock() {
    use std::sync::{Arc, Barrier, mpsc};
    use std::time::Duration;
    let (base, _library, from, alias) = fixture();
    let second = Library::open(&base.join("data")).unwrap();
    let operation = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(base.join("data/library.write.lock"))
        .unwrap();
    operation.lock().unwrap();
    let to = base.join("data-library");
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
fn unavailable_source_volume_and_known_native_root_do_not_block_management() {
    let (base, library, from, _alias) = fixture();
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
    assert!(from.exists());
    assert_eq!(pending_count(&base), 0);
    drop(library);
    let reopened = Library::open(&base.join("data")).unwrap();
    assert_eq!(reopened.sources().unwrap().len(), 2);
    assert!(reopened.scan().is_ok());
}
