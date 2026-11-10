use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[path = "support/update_process.rs"]
mod update_process;
use update_process::FixtureHelper;

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn name() -> &'static str {
    if cfg!(windows) {
        "skilvella.exe"
    } else {
        "skilvella"
    }
}

fn wait_for(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !condition() {
        assert!(Instant::now() < deadline, "Updater fixture timed out");
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn serves_ui(port: u16) -> bool {
    let Ok(mut stream) =
        TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), Duration::from_millis(100))
    else {
        return false;
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    stream
        .write_all(format!("GET / HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\n\r\n").as_bytes())
        .unwrap();
    let mut bytes = [0; 128];
    stream
        .read(&mut bytes)
        .is_ok_and(|n| String::from_utf8_lossy(&bytes[..n]).contains("200 OK"))
}

fn copy_executable(source: &Path, target: &Path) {
    fs::copy(source, target).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(target, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

fn helper_lifecycle(bad_update: bool, cancel: bool, bad_args: bool, panic_after_relaunch: bool) {
    let root = std::env::var_os("SKILVELLA_MIGRATION_TEST_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let root = std::env::temp_dir();
            #[cfg(unix)]
            let root = root.canonicalize().unwrap();
            root
        });
    assert!(root.is_absolute());
    let root = root.join(format!("update-e2e-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let home = root.join("home");
    fs::create_dir(&home).unwrap();
    let stage = root.join(".skilvella-update-fixture");
    fs::create_dir(&stage).unwrap();
    let target = root.join(name());
    let helper = stage.join(name());
    let source = Path::new(env!("CARGO_BIN_EXE_skilvella"));
    copy_executable(source, &target);
    copy_executable(source, &helper);
    let bytes = fs::read(source).unwrap();
    fs::write(
        stage.join("new-binary"),
        if bad_update {
            b"invalid executable".as_slice()
        } else {
            &bytes
        },
    )
    .unwrap();
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let args = vec![
        "--no-tray".to_owned(),
        "--no-watch".to_owned(),
        "--port".to_owned(),
        port.to_string(),
        "--data-dir".to_owned(),
        root.join("data").to_str().unwrap().to_owned(),
    ];
    let environment = |command: &mut Command| {
        command
            .env("USERPROFILE", &home)
            .env("HOME", &home)
            .env("CODEX_HOME", home.join(".codex"))
            .env("TEMP", &root)
            .env("TMP", &root)
            .stdin(Stdio::null())
            .stdout(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
    };
    let mut command = Command::new(&target);
    command.args(&args).stderr(Stdio::null());
    environment(&mut command);
    let mut parent = Process(command.spawn().unwrap());
    drop(command);
    wait_for(|| serves_ui(port));
    let marker = root.join("data/retained-fixture");
    fs::write(&marker, "retained").unwrap();
    let mut relaunch_args = args.clone();
    if bad_args {
        relaunch_args.push("--invalid-fixture-relaunch-option".to_owned());
    }
    let plan = serde_json::json!({
        "parent_pid":parent.0.id(), "target":target, "previous_hash":hash(&bytes),
        "binary_hash":hash(if bad_update { b"invalid executable" } else { &bytes }),
        "args":relaunch_args, "cwd":root, "managed":null,
    });
    fs::write(stage.join("plan.json"), serde_json::to_vec(&plan).unwrap()).unwrap();
    let log = fs::File::create(stage.join("update.log")).unwrap();
    let mut command = Command::new(&helper);
    command
        .arg("--skilvella-install-update")
        .arg(stage.join("plan.json"))
        .stderr(Stdio::from(log));
    environment(&mut command);
    let updater = FixtureHelper::spawn(&mut command).unwrap();
    drop(command);
    let cleanup = Arc::clone(&updater.cleanup);
    #[cfg(windows)]
    let cleanup_log = stage.join("update.log");
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let mut updater = updater;
        wait_for(|| stage.join("ready").is_file() || updater.status().unwrap().is_some());
        assert!(
            stage.join("ready").is_file(),
            "{}",
            fs::read_to_string(stage.join("update.log")).unwrap()
        );
        // commit前に公開・置換しない。
        assert!(!stage.join("previous-binary").exists());
        assert_eq!(fs::read(&target).unwrap(), bytes);
        if cancel {
            fs::write(stage.join("cancel"), "cancel").unwrap();
        } else {
            #[cfg(unix)]
            {
                updater.may_relaunch = true;
            }
            fs::write(stage.join("commit"), "commit").unwrap();
            parent.0.kill().unwrap();
            parent.0.wait().unwrap();
        }
        wait_for(|| updater.status().unwrap().is_some());
        let status = updater.status().unwrap().unwrap();
        if panic_after_relaunch {
            wait_for(|| serves_ui(port));
            panic!("intentional fixture panic before reading restarted PID");
        }
        let log = fs::read_to_string(stage.join("update.log")).unwrap();
        let restarted = log.lines().find_map(|line| {
            line.strip_prefix("Restarted PID: ")
                .and_then(|s| s.parse::<u32>().ok())
        });
        assert_eq!(status, !bad_update && !cancel, "{log}");
        if cancel {
            assert!(restarted.is_none());
            assert!(parent.0.try_wait().unwrap().is_none());
        } else {
            assert!(restarted.is_some(), "{log}");
            if bad_args {
                updater.confirm_relaunch_exited(restarted.unwrap()).unwrap();
            } else {
                wait_for(|| serves_ui(port));
            }
            assert_eq!(stage.join("complete").exists(), !bad_update);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            // 再起動したアプリが動いていても、ヘルパーのログを排他的に開ける。
            fs::OpenOptions::new()
                .read(true)
                .share_mode(0)
                .open(stage.join("update.log"))
                .expect("The restarted app must not retain the updater log handle");
        }
        assert_eq!(fs::read(&target).unwrap(), bytes);
        assert_eq!(fs::read_to_string(marker).unwrap(), "retained");
        updater.stop().expect("Fixture helper cleanup must succeed");
    }));
    let cleanup_result = cleanup
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .take();
    match &cleanup_result {
        Some(Err(error)) => eprintln!("Fixture cleanup failed: {error}"),
        None => eprintln!("Fixture cleanup did not report its result"),
        Some(Ok(())) => {}
    }
    if panic_after_relaunch {
        let error = outcome.expect_err("The fixture must exercise unwinding");
        assert_eq!(
            error.downcast_ref::<&str>(),
            Some(&"intentional fixture panic before reading restarted PID")
        );
    } else if let Err(error) = outcome {
        std::panic::resume_unwind(error);
    }
    cleanup_result
        .expect("Fixture cleanup must report its result")
        .expect("Fixture cleanup must succeed, including during unwinding");
    if !cancel {
        wait_for(|| TcpListener::bind(("127.0.0.1", port)).is_ok());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(cleanup_log)
            .expect("Fixture cleanup must release the updater log handle, including after panic");
    }
}

#[test]
fn helper_replaces_and_relaunches_an_isolated_headless_server() {
    helper_lifecycle(false, false, false, false);
}

#[test]
fn helper_restores_and_restarts_after_an_invalid_staged_executable() {
    helper_lifecycle(true, false, false, false);
}

#[test]
fn cancelled_handshake_preserves_the_running_server() {
    helper_lifecycle(false, true, false, false);
}

#[test]
fn helper_owns_a_relaunched_process_that_exits_immediately() {
    helper_lifecycle(false, false, true, false);
}

#[test]
fn helper_unwind_releases_the_restarted_server_before_reading_its_pid() {
    helper_lifecycle(false, false, false, true);
}
