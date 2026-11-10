use anyhow::{Context, Result, bail, ensure};
use reqwest::{Client, Method, StatusCode, redirect::Policy};
use serde_json::Value;
use skilvella::update::release::regression::{self, Inputs};
#[cfg(windows)]
use std::path::PathBuf;
use std::{
    env, fs,
    io::{self, Read, Write},
    net::{Ipv4Addr, TcpListener},
    path::Path,
    process::{Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};

mod release_feed;
#[allow(dead_code)] // ヘルパー単体テストと共有する。こちらでは再起動したサーバーを検証する。
mod update_process;
use release_feed::ReleaseFeed;
use update_process::FixtureHelper;

const START_TIMEOUT: Duration = Duration::from_secs(30);
const STEP_TIMEOUT: Duration = Duration::from_secs(60);
const POLL: Duration = Duration::from_millis(100);
const PREVIOUS_PID: &str = "Release fixture previous PID: ";
const PREVIOUS_EXIT: &str = "Release fixture previous exit: ";

pub fn run() -> Result<()> {
    #[cfg(windows)]
    if env::current_exe()?
        .file_name()
        .is_some_and(|name| name == "curl.exe")
    {
        return curl_with_fixture_ca();
    }
    let args: Vec<_> = env::args_os().skip(1).collect();
    let (tag, distribution, parent) = match args.as_slice() {
        [mode, target, app_args @ ..] if mode == "--supervise" => {
            return supervise(Path::new(target), app_args);
        }
        [tag, distribution, parent] => (tag, Path::new(distribution), Path::new(parent)),
        _ => bail!(
            "Usage: release_update <previous-vX.Y.Z> <absolute-distribution-dir> <absolute-work-parent>"
        ),
    };
    let root = regression::workspace(parent)?;
    println!("Release update workspace: {}", root.display());
    let result = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(scenario(
            tag.to_str().context("Expected an ASCII release tag")?,
            distribution,
            &root,
        ));
    result.with_context(|| format!("Release update failed; see logs in {}", root.display()))
}

fn supervise(target: &Path, args: &[std::ffi::OsString]) -> Result<()> {
    // Windows では Job への参加を、Mac では専用プロセスグループの所有を確定してから
    // 旧版を起動する。旧版を wait で回収する間も、このプロセスがグループを保持する。
    let mut ready = [0];
    io::stdin().read_exact(&mut ready)?;
    ensure!(ready == [1], "Fixture supervisor was not started");
    #[cfg(unix)]
    let monitor = {
        let pid = std::process::id() as libc::pid_t;
        ensure!(
            pid > 0 && unsafe { libc::getpgrp() } == pid,
            "Fixture supervisor does not own its process group"
        );
        std::thread::spawn(move || {
            // テストの強制終了で制御パイプが閉じても、アプリだけを残さない。
            let _ = io::stdin().read_exact(&mut [0]);
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
        })
    };
    let mut child = Command::new(target)
        .args(args)
        .stdin(Stdio::null())
        .spawn()?;
    println!("{PREVIOUS_PID}{}", child.id());
    println!("{PREVIOUS_EXIT}{}", child.wait()?.success());
    #[cfg(unix)]
    monitor
        .join()
        .map_err(|_| anyhow::anyhow!("Fixture supervisor monitor failed"))?;
    Ok(())
}

async fn scenario(tag: &str, distribution: &Path, root: &Path) -> Result<()> {
    let home = root.join("home");
    let codex = home.join(".codex");
    let tmp = root.join("tmp");
    let bin = root.join("bin");
    for path in [&home, &codex, &tmp, &bin] {
        fs::create_dir(path)?;
    }
    let target = bin.join(if cfg!(windows) {
        "skilvella.exe"
    } else {
        "skilvella"
    });
    let files = Arc::new(regression::prepare(tag, distribution, &target)?);
    let cert = root.join("fixture.pem");
    let feed = ReleaseFeed::start(&cert, files.clone())?;

    // 更新ヘルパーは起動引数を再利用するため、再起動後も同じポートで開けることを確かめる。
    let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?
        .local_addr()?
        .port();
    let base = format!("http://127.0.0.1:{port}");
    let log_path = root.join("startup.log");
    let log = fs::File::create(&log_path)?;
    let mut command = Command::new(env::current_exe()?);
    command
        .current_dir(root)
        .arg("--supervise")
        .arg(&target)
        .arg("--data-dir")
        .arg(root.join("data"))
        .arg("--port")
        .arg(port.to_string())
        .args(["--no-tray", "--no-watch"])
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("LOCALAPPDATA", &home)
        .env("APPDATA", &home)
        .env("CODEX_HOME", &codex)
        .env("TMP", &tmp)
        .env("TEMP", &tmp)
        .env("TMPDIR", &tmp)
        .env("HTTPS_PROXY", feed.proxy_url())
        .env("https_proxy", feed.proxy_url())
        .env_remove("NO_PROXY")
        .env_remove("no_proxy")
        .env("CURL_CA_BUNDLE", &cert)
        .stdin(Stdio::piped())
        .stdout(log.try_clone()?)
        .stderr(log);
    configure_downloader(&mut command, root)?;
    let mut process = FixtureHelper::spawn(&mut command)?;
    drop(command);
    #[cfg(unix)]
    {
        process.may_relaunch = true;
    }
    process
        .child
        .stdin
        .as_mut()
        .context("Fixture supervisor stdin unavailable")?
        .write_all(&[1])?;

    let client = Client::builder()
        .no_proxy()
        .redirect(Policy::none())
        .timeout(STEP_TIMEOUT)
        .build()?;
    let result = verify(&client, &base, &log_path, &target, &files).await;
    let cleanup = process
        .stop()
        .context("Could not stop the isolated app processes");
    let (old_pid, new_pid) = match (result, cleanup) {
        (Err(test), Err(cleanup)) => return Err(test.context(cleanup)),
        (Ok(pids), Ok(())) => pids,
        (Err(error), Ok(())) | (Ok(_), Err(error)) => return Err(error),
    };
    println!(
        "PASS: v{} (PID {}) -> v{} (PID {}); manager and settings returned HTTP 200 at {base}, candidate version displayed",
        files.previous, old_pid, files.candidate, new_pid,
    );
    Ok(())
}

#[cfg(target_os = "macos")]
fn configure_downloader(command: &mut Command, _root: &Path) -> Result<()> {
    // macOS 同梱 curl の OpenSSL 互換バックエンドは子プロセス指定の CA を受け取れる。
    command.env("CURL_SSL_BACKEND", "openssl");
    Ok(())
}

#[cfg(windows)]
fn configure_downloader(command: &mut Command, root: &Path) -> Result<()> {
    use std::os::windows::process::CommandExt;
    let windows = PathBuf::from(env::var_os("SystemRoot").context("SystemRoot is unavailable")?);
    let real_curl = windows.join("System32/curl.exe");
    ensure!(
        real_curl.is_absolute() && real_curl.is_file(),
        "System curl is unavailable"
    );
    let fake_windows = root.join("windows");
    let system32 = fake_windows.join("System32");
    fs::create_dir_all(&system32)?;
    // Schannel は CURL_CA_BUNDLE を読まない。隔離した SystemRoot でテスト実行ファイルを
    // curl の入口にして、本物の curl にテスト用の --cacert だけを渡す。
    fs::copy(env::current_exe()?, system32.join("curl.exe"))?;
    // Winsock の標準プロバイダーも同じ変数で解決される。元の DLL を隔離先へ複製する。
    fs::copy(
        windows.join("System32/mswsock.dll"),
        system32.join("mswsock.dll"),
    )?;
    command
        .env("SystemRoot", fake_windows)
        .env("SKILVELLA_TEST_REAL_CURL", real_curl)
        .creation_flags(0x08000000);
    Ok(())
}

#[cfg(windows)]
fn curl_with_fixture_ca() -> Result<()> {
    use std::os::windows::process::CommandExt;
    let curl = PathBuf::from(
        env::var_os("SKILVELLA_TEST_REAL_CURL").context("No fixture downloader configured")?,
    );
    let ca = env::var_os("CURL_CA_BUNDLE").context("No fixture certificate configured")?;
    let windows = curl
        .parent()
        .and_then(Path::parent)
        .context("Invalid system curl path")?;
    let mut args = env::args_os().skip(1);
    ensure!(
        args.next().is_some_and(|arg| arg == "-q"),
        "Expected curl with config loading disabled"
    );
    let log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("curl.log")?;
    let status = Command::new(&curl)
        .env("SystemRoot", windows)
        .creation_flags(0x08000000)
        .stderr(log)
        .args(["-q", "--show-error", "--cacert"])
        .arg(ca)
        .args(args)
        .status()?;
    ensure!(status.success(), "System curl returned {status}");
    Ok(())
}

async fn verify(
    client: &Client,
    base: &str,
    log: &Path,
    target: &Path,
    files: &Inputs,
) -> Result<(u32, u32)> {
    let deadline = Instant::now() + START_TIMEOUT;
    let browser_line = format!("Browser: {base}/");
    let old_pid = loop {
        let output = fs::read_to_string(log)?;
        ensure!(
            log_value(&output, PREVIOUS_EXIT).is_none(),
            "Previous release exited during startup: {output}"
        );
        // 所有するプロセスが bind に成功してからだけ HTTP 操作を行う。
        if let Some(pid) = log_value(&output, PREVIOUS_PID)
            && output.lines().any(|line| line == browser_line)
        {
            break pid.parse::<u32>()?;
        }
        ensure!(
            Instant::now() < deadline,
            "Previous release did not open the manager"
        );
        tokio::time::sleep(POLL).await;
    };
    let settings = settings(client, base, files.previous.as_str()).await?;
    let csrf = settings
        .split_once("data-csrf=\"")
        .context("Update controls have no CSRF token")?
        .1
        .split_once('"')
        .context("Invalid CSRF attribute")?
        .0;
    ensure!(
        !csrf.is_empty() && csrf.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-'),
        "Invalid CSRF token"
    );
    let info = post(client, base, "check", csrf).await?;
    ensure!(
        info["current_version"] == files.previous
            && info["latest_version"] == files.candidate
            && info["supported"] == true
            && info["available"] == true,
        "Unexpected update check: {info}"
    );
    let update = post(client, base, "install", csrf).await?;
    ensure!(
        update["restarting"] == true,
        "Update did not request a restart: {update}"
    );

    let deadline = Instant::now() + START_TIMEOUT;
    let bin = target.parent().context("Executable has no parent")?;
    let new_pid = loop {
        if let Some(success) = log_value(&fs::read_to_string(log)?, PREVIOUS_EXIT) {
            ensure!(success == "true", "Previous release did not exit cleanly");
            if let Some(pid) = completed_restart(bin)? {
                break pid;
            }
        }
        ensure!(
            Instant::now() < deadline,
            "Update helper did not complete the restart"
        );
        tokio::time::sleep(POLL).await;
    };
    ensure!(new_pid != old_pid, "Update reused the previous process");
    ensure!(
        fs::read(target)? == files.candidate_binary,
        "Installed executable differs from the release candidate"
    );

    let deadline = Instant::now() + START_TIMEOUT;
    loop {
        match self::settings(client, base, files.candidate).await {
            Ok(_) => break,
            Err(error) if Instant::now() >= deadline => {
                return Err(error)
                    .context("Restarted manager did not display the candidate version");
            }
            Err(_) => tokio::time::sleep(POLL).await,
        }
    }
    let manager = page(client, base, "/").await?;
    ensure!(
        manager.contains("href=\"/settings\"") && manager.contains("<main id=\"main\">"),
        "Restarted library page did not open"
    );
    Ok((old_pid, new_pid))
}

fn log_value<'a>(log: &'a str, prefix: &str) -> Option<&'a str> {
    log.lines().find_map(|line| line.strip_prefix(prefix))
}

fn completed_restart(bin: &Path) -> Result<Option<u32>> {
    for entry in fs::read_dir(bin)? {
        let path = entry?.path();
        if !path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(".skilvella-update-"))
            || !path.join("complete").is_file()
        {
            continue;
        }
        let log = fs::read_to_string(path.join("update.log"))?;
        let pid = log
            .lines()
            .find_map(|line| line.strip_prefix("Restarted PID: "))
            .context("Missing restart PID")?;
        return Ok(Some(pid.parse()?));
    }
    Ok(None)
}

async fn page(client: &Client, base: &str, path: &str) -> Result<String> {
    let response = client
        .get(format!("{base}{path}"))
        .timeout(Duration::from_secs(2))
        .send()
        .await?;
    ensure!(
        response.status() == StatusCode::OK,
        "{path} returned {}",
        response.status()
    );
    Ok(response.text().await?)
}

async fn settings(client: &Client, base: &str, version: &str) -> Result<String> {
    let page = page(client, base, "/settings").await?;
    let section = page
        .split_once("id=\"updates\"")
        .context("Missing update settings")?
        .1
        .split_once("</section>")
        .context("Incomplete update settings")?
        .0;
    ensure!(
        section.contains(&format!("<strong>{version}</strong>")),
        "Update settings do not display {version}"
    );
    Ok(section.to_owned())
}

async fn post(client: &Client, base: &str, action: &str, csrf: &str) -> Result<Value> {
    let response = client
        .request(Method::POST, format!("{base}/settings/updates/{action}"))
        .header("Origin", base)
        .header("Accept", "application/json")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!("csrf={csrf}"))
        .send()
        .await?;
    let status = response.status();
    let body = response.text().await?;
    ensure!(
        status == StatusCode::OK,
        "Update {action} returned {status}: {body}"
    );
    Ok(serde_json::from_str(&body)?)
}
