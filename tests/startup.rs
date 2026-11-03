use std::{
    fs,
    io::{BufRead, BufReader},
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};

struct RunningServer(Child);

impl Drop for RunningServer {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test]
async fn headless_process_serves_ui_and_mcp_on_the_bound_port() {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".work/test-startup")
        .join(uuid::Uuid::new_v4().to_string());
    let home = base.join("home");
    fs::create_dir_all(&home).unwrap();
    let mut server = RunningServer(
        Command::new(env!("CARGO_BIN_EXE_skilvella"))
            .args(["--no-tray", "--no-watch", "--port", "0", "--data-dir"])
            .arg(base.join("data"))
            .env("USERPROFILE", &home)
            .env("HOME", &home)
            .env("CODEX_HOME", home.join(".codex"))
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let stdout = server.0.stdout.take().unwrap();
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(url) = line.strip_prefix("Browser: ") {
                let _ = sender.send(url.to_owned());
                break;
            }
        }
    });
    let url = receiver.recv_timeout(Duration::from_secs(15)).unwrap();
    assert!(!url.ends_with(":0/"));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    assert_eq!(client.get(&url).send().await.unwrap().status(), 200);
    let response: serde_json::Value = client
        .post(format!("{url}mcp"))
        .header("Accept", "application/json, text/event-stream")
        .json(&serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26", "capabilities": {},
                "clientInfo": {"name": "headless-startup-test", "version": "1"}
            }
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(response["result"]["serverInfo"]["name"], "skilvella");
}

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
