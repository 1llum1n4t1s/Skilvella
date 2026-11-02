use std::{fs, net::TcpListener, path::PathBuf, process::Command};

#[test]
fn occupied_port_does_not_create_or_change_the_library() {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".work/test-startup")
        .join(uuid::Uuid::new_v4().to_string());
    let home = base.join("home");
    fs::create_dir_all(home.join(".agents/skills/demo")).unwrap();
    fs::write(
        home.join(".agents/skills/demo/SKILL.md"),
        "---\nname: startup-fixture\ndescription: port conflict\n---\nInstructions\n",
    )
    .unwrap();
    let occupied = TcpListener::bind("127.0.0.1:0").unwrap();
    let data = base.join("data");
    let output = Command::new(env!("CARGO_BIN_EXE_skilvella"))
        .args([
            "--data-dir",
            data.to_str().unwrap(),
            "--port",
            &occupied.local_addr().unwrap().port().to_string(),
        ])
        .env("USERPROFILE", &home)
        .env("HOME", &home)
        .env("CODEX_HOME", home.join(".codex"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Could not bind localhost port"));
    assert!(
        !data.exists(),
        "A failed second instance must not touch library settings"
    );
}
