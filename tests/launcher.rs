use std::{
    env, fs,
    path::PathBuf,
    process::Command,
    thread,
    time::{Duration, Instant},
};

#[test]
#[ignore = "隔離した子プロセスからのみ実行する起動確認用のテスト"]
fn launcher_probe() {
    let Some(output) = env::var_os("SKILVELLA_LAUNCHER_PROBE") else {
        return;
    };
    let data = serde_json::json!({
        "cwd": env::current_dir().unwrap(),
        "args": env::args_os().skip(1).map(|arg| arg.into_string().unwrap()).collect::<Vec<_>>()
    });
    fs::write(output, serde_json::to_vec(&data).unwrap()).unwrap();
}

#[test]
fn launcher_starts_sibling_with_arguments_and_working_directory_and_reports_failure() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".work/test-launcher")
        .join(format!("launcher with spaces {}", uuid::Uuid::new_v4()));
    let installed = root.join("installed with spaces");
    let absent = root.join("missing target");
    fs::create_dir_all(&installed).unwrap();
    fs::create_dir_all(&absent).unwrap();
    let launcher_name = if cfg!(windows) {
        "skilvella-launcher.exe"
    } else {
        "skilvella-launcher"
    };
    let target_name = if cfg!(windows) {
        "skilvella.exe"
    } else {
        "skilvella"
    };
    let launcher = installed.join(launcher_name);
    fs::copy(env!("CARGO_BIN_EXE_skilvella-launcher"), &launcher).unwrap();
    // 実アプリを起動せず、テスト実行ファイルを同名の確認用ターゲットにする。
    fs::copy(env::current_exe().unwrap(), installed.join(target_name)).unwrap();
    let output = root.join("probe.json");
    let args = [
        "--exact",
        "launcher_probe",
        "--ignored",
        "--nocapture",
        "argument with spaces",
        "日本語",
    ];
    let status = Command::new(&launcher)
        .current_dir(&root)
        .env("SKILVELLA_LAUNCHER_PROBE", &output)
        .args(args)
        .status()
        .unwrap();
    assert!(status.success());
    let deadline = Instant::now() + Duration::from_secs(10);
    let data = loop {
        if let Ok(bytes) = fs::read(&output)
            && let Ok(data) = serde_json::from_slice::<serde_json::Value>(&bytes)
        {
            break data;
        }
        assert!(
            Instant::now() < deadline,
            "The launched target did not report its arguments"
        );
        thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(
        PathBuf::from(data["cwd"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        installed.canonicalize().unwrap()
    );
    assert_eq!(data["args"], serde_json::json!(args));
    let missing_launcher = absent.join(launcher_name);
    fs::copy(env!("CARGO_BIN_EXE_skilvella-launcher"), &missing_launcher).unwrap();
    assert!(
        !Command::new(missing_launcher)
            .current_dir(root)
            .status()
            .unwrap()
            .success()
    );
}
