use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

// ヘルパーが今回起動した子のPIDだけを終了する。
struct Restarted(u32);
impl Drop for Restarted {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::{
                Foundation::CloseHandle,
                System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess},
            };
            let handle = OpenProcess(PROCESS_TERMINATE, 0, self.0);
            if !handle.is_null() {
                TerminateProcess(handle, 0);
                CloseHandle(handle);
            }
        }
        #[cfg(unix)]
        unsafe {
            libc::kill(self.0 as libc::pid_t, libc::SIGTERM);
        }
    }
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
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

fn helper_lifecycle(bad_update: bool, cancel: bool) {
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
    wait_for(|| serves_ui(port));
    let marker = root.join("data/retained-fixture");
    fs::write(&marker, "retained").unwrap();
    let plan = serde_json::json!({
        "parent_pid":parent.0.id(), "target":target, "previous_hash":hash(&bytes),
        "binary_hash":hash(if bad_update { b"invalid executable" } else { &bytes }),
        "args":args, "cwd":root, "managed":null,
    });
    fs::write(stage.join("plan.json"), serde_json::to_vec(&plan).unwrap()).unwrap();
    let log = fs::File::create(stage.join("update.log")).unwrap();
    let mut command = Command::new(&helper);
    command
        .arg("--skilvella-install-update")
        .arg(stage.join("plan.json"))
        .stderr(Stdio::from(log));
    environment(&mut command);
    let mut updater = Process(command.spawn().unwrap());
    wait_for(|| stage.join("ready").is_file() || updater.0.try_wait().unwrap().is_some());
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
        fs::write(stage.join("commit"), "commit").unwrap();
        parent.0.kill().unwrap();
        parent.0.wait().unwrap();
    }
    wait_for(|| updater.0.try_wait().unwrap().is_some());
    let status = updater.0.wait().unwrap();
    let log = fs::read_to_string(stage.join("update.log")).unwrap();
    let restarted = log
        .lines()
        .find_map(|line| {
            line.strip_prefix("Restarted PID: ")
                .and_then(|s| s.parse().ok())
        })
        .map(Restarted);
    assert_eq!(status.success(), !bad_update && !cancel, "{log}");
    if cancel {
        assert!(restarted.is_none());
        assert!(parent.0.try_wait().unwrap().is_none());
    } else {
        assert!(restarted.is_some(), "{log}");
        wait_for(|| serves_ui(port));
        assert_eq!(stage.join("complete").exists(), !bad_update);
    }
    assert_eq!(fs::read(&target).unwrap(), bytes);
    assert_eq!(fs::read_to_string(marker).unwrap(), "retained");
}

#[test]
fn helper_replaces_and_relaunches_an_isolated_headless_server() {
    helper_lifecycle(false, false);
}

#[test]
fn helper_restores_and_restarts_after_an_invalid_staged_executable() {
    helper_lifecycle(true, false);
}

#[test]
fn cancelled_handshake_preserves_the_running_server() {
    helper_lifecycle(false, true);
}
