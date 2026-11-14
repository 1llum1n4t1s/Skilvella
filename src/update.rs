//! 安定版の明示的な確認・取得と、終了後の実行ファイル更新。
use anyhow::{Context, bail, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

const REPO: &str = "https://github.com/1llum1n4t1s/Skilvella";
const MAX_ARCHIVE: u64 = 150 * 1024 * 1024;
const MAX_CHECKSUMS: u64 = 1024 * 1024;
const WINDOWS_ASSET: &str = "Skilvella-windows-x86_64.zip";
const MAC_ASSET: &str = "Skilvella-macos-aarch64.tar.gz";
const BUILD_INFO: &str = "BUILD-INFO.txt";
static INSTALLING: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "release-tools")]
pub mod release;

#[derive(Debug, Serialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub available: bool,
    pub supported: bool,
    pub release_url: String,
}

#[derive(Serialize, Deserialize)]
struct Plan {
    parent_pid: u32,
    target: PathBuf,
    previous_hash: String,
    binary_hash: String,
    args: Vec<String>,
    cwd: PathBuf,
    managed: Option<Managed>,
}

#[derive(Serialize, Deserialize)]
struct Managed {
    root: PathBuf,
    previous: serde_json::Value,
    next: serde_json::Value,
}

/// この値は検証済みの prepare_install だけから作る。
pub struct PreparedUpdate {
    directory: PathBuf,
    plan: Plan,
    lock: Option<File>,
}

fn installation_lock(target: &Path) -> anyhow::Result<File> {
    let path = target
        .parent()
        .context("Executable directory missing")?
        .join(".skilvella-update.lock");
    plain_path(&path)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    file.try_lock()
        .context("Another process is updating this executable")?;
    Ok(file)
}

fn version(value: &str) -> anyhow::Result<[u64; 3]> {
    let parts: Vec<_> = value
        .strip_prefix('v')
        .unwrap_or(value)
        .split('.')
        .collect();
    ensure!(parts.len() == 3, "Invalid stable release version");
    let mut result = [0; 3];
    for (i, part) in parts.into_iter().enumerate() {
        ensure!(
            !part.is_empty() && part.bytes().all(|c| c.is_ascii_digit()),
            "Invalid stable release version"
        );
        ensure!(
            part.len() == 1 || !part.starts_with('0'),
            "Invalid stable release version"
        );
        result[i] = part.parse()?;
    }
    Ok(result)
}

fn asset_name() -> Option<&'static str> {
    if cfg!(all(windows, target_arch = "x86_64")) {
        Some(WINDOWS_ASSET)
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some(MAC_ASSET)
    } else {
        None
    }
}

fn in_app_asset() -> Option<&'static str> {
    // インストーラー版の配置はSetupが管理する。ZIP版のヘルパーで実体を置き換えない。
    if cfg!(all(windows, feature = "windows-installer")) {
        None
    } else {
        asset_name()
    }
}

fn executable_name() -> &'static str {
    if cfg!(windows) {
        "skilvella.exe"
    } else {
        "skilvella"
    }
}

fn hidden(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    #[cfg(not(windows))]
    let _ = command;
}

// シェルを介さずOS同梱curlを使う。引数・URLは内部生成し、HTTPへの降格を拒否する。
fn request(url: &str, limit: u64, head: bool) -> anyhow::Result<Vec<u8>> {
    let curl = if cfg!(windows) {
        let system = std::env::var_os("SystemRoot").context("SystemRoot is unavailable")?;
        PathBuf::from(system).join("System32/curl.exe")
    } else {
        PathBuf::from("/usr/bin/curl")
    };
    let mut command = Command::new(curl);
    command
        .arg("-q")
        .args([
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--location",
            "--max-redirs",
            "5",
            "--fail",
            "--silent",
            "--connect-timeout",
            "10",
            "--max-time",
            "120",
            "--max-filesize",
        ])
        .arg(limit.to_string());
    if head {
        command.args([
            "--head",
            "--output",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
            "--write-out",
            "%{url_effective}",
        ]);
    }
    command
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    hidden(&mut command);
    let mut child = command
        .spawn()
        .context("Could not start the system HTTPS downloader")?;
    let mut bytes = Vec::new();
    let read = child
        .stdout
        .take()
        .context("Downloader output unavailable")?
        .take(limit + 1)
        .read_to_end(&mut bytes);
    if read.is_err() || bytes.len() as u64 > limit {
        let _ = child.kill();
        let _ = child.wait();
        bail!("Release response exceeds the permitted size or could not be read");
    }
    // curl の標準エラーは捨てているため、失敗理由の手掛かりとして終了状態だけを含める。
    // （例: 接続失敗・HTTPエラー・サイズ超過は curl の終了コードで区別できる）
    let status = child.wait()?;
    ensure!(
        status.success(),
        "Could not retrieve the stable release over HTTPS ({status})"
    );
    Ok(bytes)
}

/// 同期I/Oを行うため、非同期ハンドラーからは spawn_blocking で呼ぶ。
pub fn check() -> anyhow::Result<UpdateInfo> {
    let resolved = String::from_utf8(request(&format!("{REPO}/releases/latest"), 4096, true)?)?;
    let prefix = format!("{REPO}/releases/tag/");
    let tag = resolved
        .strip_prefix(&prefix)
        .context("Unexpected stable release location")?;
    ensure!(tag.starts_with('v'), "Unexpected stable release tag");
    let latest = version(tag)?;
    Ok(UpdateInfo {
        current_version: env!("CARGO_PKG_VERSION").to_owned(),
        latest_version: tag.trim_start_matches('v').to_owned(),
        available: latest > version(env!("CARGO_PKG_VERSION"))?,
        supported: in_app_asset().is_some(),
        release_url: resolved,
    })
}

fn checksum(list: &str, asset: &str) -> anyhow::Result<String> {
    let mut found = None;
    let mut seen = HashSet::new();
    for line in list.lines() {
        let (hash, name) = line
            .split_once("  ")
            .or_else(|| line.split_once(" *"))
            .context("Malformed release checksum list")?;
        ensure!(
            hash.len() == 64 && hash.bytes().all(|c| c.is_ascii_hexdigit()),
            "Malformed release checksum"
        );
        ensure!(
            !name.is_empty()
                && !name.contains(['/', '\\'])
                && !name.chars().any(char::is_whitespace),
            "Invalid checksum filename"
        );
        ensure!(seen.insert(name), "Duplicate release checksum");
        if name == asset {
            found = Some(hash.to_ascii_lowercase());
        }
    }
    found.context("Release checksum is missing")
}

fn digest(bytes: &[u8]) -> String {
    hash_hex(&Sha256::digest(bytes))
}

fn hash_hex(hash: &[u8]) -> String {
    // チェックサム照合と同じ小文字16進表現を、ライブラリ共通の変換関数で作る。
    crate::sha256_hex(hash)
}

fn file_hash(path: &Path) -> anyhow::Result<String> {
    plain_path(path)?;
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let count = match file.read(&mut buffer) {
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(hash_hex(&hash.finalize()))
}

fn plain_path(path: &Path) -> anyhow::Result<()> {
    ensure!(path.is_absolute(), "Update paths must be absolute");
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(meta) => {
                ensure!(
                    !meta.file_type().is_symlink(),
                    "Update path contains a symbolic link"
                );
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    ensure!(
                        meta.file_attributes() & 0x400 == 0,
                        "Update path contains a reparse point"
                    );
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

fn write_new(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    plain_path(path)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn publish_marker(directory: &Path, name: &str) -> anyhow::Result<()> {
    let temporary = directory.join(format!("marker-{}", uuid::Uuid::new_v4()));
    write_new(&temporary, name.as_bytes())?;
    fs::rename(temporary, directory.join(name))?;
    Ok(())
}

fn extract(archive: &[u8], zip: bool, expected_version: &str) -> anyhow::Result<Vec<u8>> {
    let name = if zip { "skilvella.exe" } else { "skilvella" };
    let allowed = [name, "LICENSE", "CHANGELOG.md", "README.txt", BUILD_INFO];
    let mut seen = HashSet::new();
    let mut binary = None;
    let mut build_info = None;
    if zip {
        let mut entries = zip::ZipArchive::new(Cursor::new(archive))?;
        for i in 0..entries.len() {
            let entry = entries.by_index(i)?;
            let path = entry.name()?.into_owned();
            ensure!(
                allowed.contains(&path.as_str()) && seen.insert(path.clone()),
                "Unexpected or duplicate release archive entry"
            );
            ensure!(
                entry.is_file() && entry.unix_mode().is_none_or(|m| m & 0o170000 != 0o120000),
                "Release entry is not a regular file"
            );
            ensure!(entry.size() <= MAX_ARCHIVE, "Release entry is too large");
            if path == name {
                let mut bytes = Vec::new();
                entry.take(MAX_ARCHIVE + 1).read_to_end(&mut bytes)?;
                ensure!(
                    bytes.len() as u64 <= MAX_ARCHIVE,
                    "Release executable is too large"
                );
                binary = Some(bytes);
            } else if path == BUILD_INFO {
                let mut text = String::new();
                entry.take(16385).read_to_string(&mut text)?;
                ensure!(text.len() <= 16384, "Build metadata is too large");
                build_info = Some(text);
            }
        }
    } else {
        let reader = flate2::read::GzDecoder::new(Cursor::new(archive));
        let mut entries = tar::Archive::new(reader.take(MAX_ARCHIVE * 2));
        for entry in entries.entries()? {
            let mut entry = entry?;
            let path = entry.path()?.to_string_lossy().into_owned();
            ensure!(
                allowed.contains(&path.as_str()) && seen.insert(path.clone()),
                "Unexpected or duplicate release archive entry"
            );
            ensure!(
                entry.header().entry_type().is_file(),
                "Release entry is not a regular file"
            );
            ensure!(entry.size() <= MAX_ARCHIVE, "Release entry is too large");
            if path == name {
                let mut bytes = Vec::new();
                entry.read_to_end(&mut bytes)?;
                binary = Some(bytes);
            } else if path == BUILD_INFO {
                let mut text = String::new();
                entry.take(16385).read_to_string(&mut text)?;
                ensure!(text.len() <= 16384, "Build metadata is too large");
                build_info = Some(text);
            }
        }
    }
    ensure!(seen.len() == allowed.len(), "Release archive is incomplete");
    let build_info = build_info.context("Build metadata is missing")?;
    let versions: Vec<_> = build_info
        .lines()
        .filter(|line| line.starts_with("version="))
        .collect();
    ensure!(
        versions == [format!("version={expected_version}")],
        "Release tag and build version disagree"
    );
    let bytes = binary.context("Release executable is missing")?;
    ensure!(!bytes.is_empty(), "Release executable is empty");
    Ok(bytes)
}

fn toolport_launcher_layout(target: &Path) -> Option<(&Path, &Path)> {
    let versions = target.parent()?.parent()?;
    if versions.file_name()? != "versions" {
        return None;
    }
    let root = versions.parent()?;
    // versions という一般的な名前だけではランチャー所有と判定しない。
    root.join("Skilvella-Latest.ps1")
        .is_file()
        .then_some((root, versions))
}

#[cfg(windows)]
pub fn is_toolport_cache_executable(target: &Path) -> bool {
    toolport_launcher_layout(target).is_some()
}

fn managed(
    target: &Path,
    tag: &str,
    archive_hash: &str,
    binary_hash: &str,
) -> anyhow::Result<Option<Managed>> {
    let directory = target.parent().context("Executable directory missing")?;
    let Some((root, versions)) = toolport_launcher_layout(target) else {
        return Ok(None);
    };
    let pointer = root.join("cache.json");
    #[cfg(windows)]
    let _ = launcher_mutex_name(root)?;
    plain_path(&pointer)?;
    let previous: serde_json::Value = serde_json::from_slice(&fs::read(&pointer)?)?;
    plain_path(&directory.join("install.json"))?;
    let installed: serde_json::Value =
        serde_json::from_slice(&fs::read(directory.join("install.json"))?)?;
    ensure!(
        previous["schema"] == 1
            && previous["tag"].as_str() == directory.file_name().and_then(|n| n.to_str()),
        "Launcher cache does not own this executable"
    );
    for key in ["schema", "tag", "zip_sha256", "exe_sha256"] {
        ensure!(
            previous[key] == installed[key],
            "Launcher cache and installation disagree"
        );
    }
    version(previous["tag"].as_str().context("Launcher tag missing")?)?;
    ensure!(
        previous["zip_sha256"]
            .as_str()
            .is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())),
        "Invalid launcher archive checksum"
    );
    ensure!(
        previous["exe_sha256"].as_str() == Some(file_hash(target)?.as_str()),
        "Launcher executable hash mismatch"
    );
    let mut next = previous.clone();
    next["tag"] = tag.into();
    next["zip_sha256"] = archive_hash.into();
    next["exe_sha256"] = binary_hash.into();
    ensure!(
        !versions.join(tag).exists(),
        "The target version already exists; use the launcher to recover it"
    );
    Ok(Some(Managed {
        root: root.to_owned(),
        previous,
        next,
    }))
}

/// 新版の検証・書込み可否の確認を終えてから値を返す。まだ稼働中の版を変更しない。
pub fn prepare_install() -> anyhow::Result<PreparedUpdate> {
    ensure!(
        INSTALLING
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok(),
        "An update is already in progress"
    );
    let result = prepare();
    if result.is_err() {
        INSTALLING.store(false, Ordering::Release);
    }
    result
}

fn prepare() -> anyhow::Result<PreparedUpdate> {
    let asset = in_app_asset().context("In-app updates are unavailable on this platform")?;
    let target = std::env::current_exe()?;
    plain_path(&target)?;
    let lock = installation_lock(&target)?;
    let info = check()?;
    ensure!(info.available, "The installed version is already current");
    let tag = format!("v{}", info.latest_version);
    let base = format!("{REPO}/releases/download/{tag}");
    let sum = checksum(
        &String::from_utf8(request(
            &format!("{base}/SHA256SUMS.txt"),
            MAX_CHECKSUMS,
            false,
        )?)?,
        asset,
    )?;
    let archive = request(&format!("{base}/{asset}"), MAX_ARCHIVE, false)?;
    let binary = verify_archive(&archive, &sum, asset, &info.latest_version)?;
    let binary_hash = digest(&binary);
    let launcher = managed(&target, &tag, &sum, &binary_hash)?;
    let directory = target
        .parent()
        .context("Executable directory missing")?
        .join(format!(".skilvella-update-{}", uuid::Uuid::new_v4()));
    plain_path(&directory)?;
    fs::create_dir(&directory).context("The executable directory must be writable for updating")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
    }
    write_new(&directory.join("new-binary"), &binary)?;
    let plan = Plan {
        parent_pid: std::process::id(),
        target: target.clone(),
        previous_hash: file_hash(&target)?,
        binary_hash,
        args: std::env::args_os()
            .skip(1)
            .map(|s| {
                s.into_string()
                    .map_err(|_| anyhow::anyhow!("Restart arguments must be valid Unicode"))
            })
            .collect::<anyhow::Result<_>>()?,
        cwd: std::env::current_dir()?,
        managed: launcher,
    };
    Ok(PreparedUpdate {
        directory,
        plan,
        lock: Some(lock),
    })
}

// 配布前と更新時に、最終アーカイブへ同じチェックを適用する。
fn verify_archive(
    archive: &[u8],
    sum: &str,
    asset: &str,
    expected_version: &str,
) -> anyhow::Result<Vec<u8>> {
    ensure!(
        archive.len() as u64 <= MAX_ARCHIVE,
        "Release archive is too large"
    );
    ensure!(digest(archive) == sum, "Release archive checksum mismatch");
    let binary = extract(archive, asset == WINDOWS_ASSET, expected_version)?;
    validate_binary(&binary, asset == WINDOWS_ASSET)?;
    Ok(binary)
}

fn validate_binary(bytes: &[u8], windows: bool) -> anyhow::Result<()> {
    if windows {
        ensure!(
            bytes.starts_with(b"MZ") && bytes.len() >= 64,
            "Release is not a Windows executable"
        );
        let offset = u32::from_le_bytes(bytes[60..64].try_into()?) as usize;
        let header = bytes
            .get(offset..offset.saturating_add(6))
            .context("Invalid Windows executable header")?;
        ensure!(
            header == b"PE\0\0\x64\x86",
            "Release executable has the wrong architecture"
        );
    } else {
        ensure!(
            bytes.starts_with(b"\xcf\xfa\xed\xfe\x0c\0\0\x01"),
            "Release is not an Apple Silicon executable"
        );
    }
    Ok(())
}

impl Drop for PreparedUpdate {
    fn drop(&mut self) {
        INSTALLING.store(false, Ordering::Release);
    }
}

/// ヘルパーが検証・待機を開始した後だけ成功する。呼出元はHTTP応答後に正常終了する。
pub fn spawn_installer(mut prepared: PreparedUpdate) -> anyhow::Result<()> {
    let directory = &prepared.directory;
    let helper = directory.join(executable_name());
    write_new(&helper, &fs::read(std::env::current_exe()?)?)?;
    executable(&helper)?;
    let plan_file = directory.join("plan.json");
    write_new(&plan_file, &serde_json::to_vec(&prepared.plan)?)?;
    let log = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(directory.join("update.log"))?;
    let mut command = Command::new(helper);
    command
        .arg("--skilvella-install-update")
        .arg(&plan_file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(log));
    hidden(&mut command);
    // 子は同じOSファイルロックを取り直す。競合した場合は ready 前に失敗する。
    drop(prepared.lock.take());
    let mut child = command
        .spawn()
        .context("Could not start the update helper")?;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if directory.join("ready").is_file() {
            publish_marker(directory, "commit")?;
            // ヘルパーの生存中は次の適用を拒否する。終了時にプロセスごと破棄される。
            std::mem::forget(prepared);
            return Ok(());
        }
        if let Some(status) = child.try_wait()? {
            bail!(
                "Update helper failed ({status}); see {}",
                directory.join("update.log").display()
            );
        }
        if Instant::now() >= deadline {
            // 公開中のファイル操作を途中で止めず、握手を中止する。
            let _ = publish_marker(directory, "cancel");
            bail!("Update helper did not become ready; the running version was kept");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn executable(path: &Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// CLI解析前に呼ぶ。内部ヘルパーモードの場合だけ処理して true を返す。
pub fn run_installer_if_requested() -> anyhow::Result<bool> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args
        .first()
        .is_none_or(|arg| arg != "--skilvella-install-update")
    {
        return Ok(false);
    }
    ensure!(args.len() == 2, "Invalid updater invocation");
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Foundation::{HANDLE_FLAG_INHERIT, SetHandleInformation};

        // ヘルパーの標準エラーはログファイル。再起動先の標準エラーをnullにしても、
        // 継承可能なままでは未使用のハンドルとして子へ残る。
        let log = std::io::stderr();
        if unsafe { SetHandleInformation(log.as_raw_handle(), HANDLE_FLAG_INHERIT, 0) } == 0 {
            return Err(std::io::Error::last_os_error())
                .context("Could not prevent updater log inheritance");
        }
    }
    let path = PathBuf::from(&args[1]);
    plain_path(&path)?;
    let directory = path.parent().context("Missing update directory")?;
    ensure!(
        std::env::current_exe()?.parent() == Some(directory),
        "Update helper is outside its staging directory"
    );
    let plan: Plan = serde_json::from_slice(&fs::read(&path)?)?;
    let result = install(directory, &plan);
    if let Err(error) = &result {
        eprintln!("Update failed: {error:#}");
    }
    result?;
    Ok(true)
}

fn process_exists(pid: u32) -> bool {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::{
            Foundation::CloseHandle,
            System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
        };
        let handle = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if handle.is_null() {
            return false;
        }
        let alive = WaitForSingleObject(handle, 0) == 258;
        CloseHandle(handle);
        alive
    }
    #[cfg(unix)]
    unsafe {
        libc::kill(pid as libc::pid_t, 0) == 0
            || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
}

fn replace(source: &Path, target: &Path) -> anyhow::Result<()> {
    plain_path(source)?;
    plain_path(target)?;
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let src: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
        let dst: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
        let ok = unsafe {
            windows_sys::Win32::Storage::FileSystem::MoveFileExW(
                src.as_ptr(),
                dst.as_ptr(),
                windows_sys::Win32::Storage::FileSystem::MOVEFILE_REPLACE_EXISTING
                    | windows_sys::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH,
            )
        };
        ensure!(
            ok != 0,
            "Could not replace file: {}",
            std::io::Error::last_os_error()
        );
    }
    #[cfg(not(windows))]
    fs::rename(source, target)?;
    Ok(())
}

fn launcher_pointer(
    managed: &Managed,
    expected: &serde_json::Value,
    next: &serde_json::Value,
    directory: &Path,
) -> anyhow::Result<()> {
    let pointer = managed.root.join("cache.json");
    plain_path(&pointer)?;
    let current: serde_json::Value = serde_json::from_slice(&fs::read(&pointer)?)?;
    ensure!(
        &current == expected,
        "Launcher cache changed during updating"
    );
    let temporary = directory.join(format!("cache-{}.json", uuid::Uuid::new_v4()));
    write_new(&temporary, &serde_json::to_vec(next)?)?;
    replace(&temporary, &pointer)
}

fn rollback_managed(managed: &Managed, directory: &Path) -> anyhow::Result<()> {
    let pointer = managed.root.join("cache.json");
    plain_path(&pointer)?;
    let current: serde_json::Value = serde_json::from_slice(&fs::read(&pointer)?)?;
    if current == managed.next {
        launcher_pointer(managed, &managed.next, &managed.previous, directory)?;
    } else {
        ensure!(
            current == managed.previous,
            "Launcher cache changed; automatic recovery stopped"
        );
    }
    let tag = managed.next["tag"]
        .as_str()
        .context("Launcher tag missing")?;
    ensure!(tag.starts_with('v'), "Invalid launcher tag");
    version(tag)?;
    let published = managed.root.join("versions").join(tag);
    plain_path(&published)?;
    if !published.exists() {
        return Ok(());
    }
    // 旧ポインターと新版ディレクトリの組合せを残すと既存ランチャーが拒否する。
    // 今回公開した版だけを照合して非公開ステージへ戻し、既存の版には触れない。
    let manifest = published.join("install.json");
    plain_path(&manifest)?;
    ensure!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(manifest)?)? == managed.next,
        "Published version metadata changed; automatic recovery stopped"
    );
    ensure!(
        Some(file_hash(&published.join(executable_name()))?.as_str())
            == managed.next["exe_sha256"].as_str(),
        "Published executable changed; automatic recovery stopped"
    );
    let files: Vec<_> = fs::read_dir(&published)?.collect::<Result<_, _>>()?;
    ensure!(
        files.len() == 2
            && files
                .iter()
                .all(|e| e.file_name() == executable_name() || e.file_name() == "install.json"),
        "Published directory contains unrelated files; automatic recovery stopped"
    );
    fs::rename(published, directory.join("failed-version"))?;
    Ok(())
}

#[cfg(windows)]
struct LauncherMutex(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl Drop for LauncherMutex {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::System::Threading::ReleaseMutex(self.0);
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
#[cfg(not(windows))]
struct LauncherMutex;

#[cfg(windows)]
fn launcher_mutex_name(root: &Path) -> anyhow::Result<String> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, LocalFree},
        Security::{
            Authorization::ConvertSidToStringSidW, GetTokenInformation, TOKEN_QUERY, TOKEN_USER,
            TokenUser,
        },
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };
    let path = root.to_str().context("Launcher root must be Unicode")?;
    ensure!(
        path.is_ascii(),
        "For a non-ASCII launcher path, update using Skilvella-Latest.ps1"
    );
    let path = unsafe {
        use windows_sys::Win32::Storage::FileSystem::{GetFullPathNameW, GetLongPathNameW};
        let input: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
        // .NET GetFullPath は拡張パスをそのまま返し、通常パスの 8.3 名を展開する。
        // GitHub Actions の RUNNER~1 などでもランチャーと同じ名前を使う。
        if path.starts_with(r"\\?\") {
            path.to_owned()
        } else {
            let length = GetFullPathNameW(
                input.as_ptr(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            ensure!(length > 0, "Could not normalize the launcher path");
            let mut output = vec![0u16; length as usize];
            let written = GetFullPathNameW(
                input.as_ptr(),
                length,
                output.as_mut_ptr(),
                std::ptr::null_mut(),
            );
            ensure!(
                written > 0 && written < length,
                "Could not normalize the launcher path"
            );
            let normalized = String::from_utf16(&output[..written as usize])?;
            if !normalized.contains('~') {
                normalized
            } else {
                let length = GetLongPathNameW(output.as_ptr(), std::ptr::null_mut(), 0);
                ensure!(length > 0, "Could not expand the launcher path");
                let mut expanded = vec![0u16; length as usize];
                let written = GetLongPathNameW(output.as_ptr(), expanded.as_mut_ptr(), length);
                ensure!(
                    written > 0 && written < length,
                    "Could not expand the launcher path"
                );
                String::from_utf16(&expanded[..written as usize])?
            }
        }
    };
    ensure!(
        path.is_ascii(),
        "For a non-ASCII launcher path, update using Skilvella-Latest.ps1"
    );
    let sid = unsafe {
        let mut token = std::ptr::null_mut();
        ensure!(
            OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) != 0,
            "Could not inspect the current Windows identity"
        );
        let mut length = 0;
        GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut length);
        let mut buffer = vec![0u64; (length as usize).div_ceil(8)];
        let ok = GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            length,
            &mut length,
        );
        CloseHandle(token);
        ensure!(ok != 0, "Could not read the current Windows identity");
        let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
        let mut text = std::ptr::null_mut();
        ensure!(
            ConvertSidToStringSidW(user.User.Sid, &mut text) != 0,
            "Could not format the current Windows identity"
        );
        let mut len = 0;
        while *text.add(len) != 0 {
            len += 1;
        }
        let value = String::from_utf16(std::slice::from_raw_parts(text, len));
        LocalFree(text.cast());
        value?
    };
    Ok(format!(
        r"Local\SkilvellaLatest-{sid}-{}",
        digest(path.to_ascii_lowercase().as_bytes()).to_ascii_uppercase()
    ))
}

fn acquire_launcher_mutex(root: &Path) -> anyhow::Result<LauncherMutex> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::{
            Foundation::CloseHandle,
            System::Threading::{CreateMutexW, WaitForSingleObject},
        };
        let name: Vec<u16> = launcher_mutex_name(root)?
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        ensure!(
            !handle.is_null(),
            "Could not open the launcher's update lock"
        );
        let status = unsafe { WaitForSingleObject(handle, 15_000) };
        if status == 0 || status == 0x80 {
            return Ok(LauncherMutex(handle));
        }
        unsafe {
            CloseHandle(handle);
        }
        bail!(
            "Another launcher is active; the update was not applied. Restart through the launcher to update"
        );
    }
    #[cfg(not(windows))]
    {
        let _ = root;
        bail!("The Windows launcher layout is unsupported on this platform");
    }
}

#[derive(Default)]
struct InstallProgress {
    published: bool,
    parent_stopped: bool,
    launcher_lock: Option<LauncherMutex>,
    file_lock: Option<File>,
}

fn install(directory: &Path, plan: &Plan) -> anyhow::Result<()> {
    let mut progress = InstallProgress {
        file_lock: Some(installation_lock(&plan.target)?),
        ..InstallProgress::default()
    };
    let result = install_inner(directory, plan, &mut progress);
    if result.is_err() {
        if progress.published
            && let Some(managed) = &plan.managed
            && let Err(error) = rollback_managed(managed, directory)
        {
            // 参照だけ復元できた場合も、下の検査を通して旧版の再起動を試す。
            // 別の参照・実行ファイルへ変わっていれば recover_previous が拒否する。
            eprintln!("Could not fully restore the launcher after a failed update: {error:#}");
        }
        if progress.parent_stopped && (plan.managed.is_none() || progress.launcher_lock.is_some()) {
            // 復旧にも失敗すると戻り値は復旧側のエラーだけになるため、元の失敗原因を先にログへ残す。
            if let Err(error) = &result {
                eprintln!("Update failed before recovery: {error:#}");
            }
            let mut child = recover_previous(directory, plan)
                .context("Could not restart the previous version after a failed update")?;
            drop(progress.file_lock.take());
            if plan.managed.is_some()
                && let Err(error) = child.wait()
            {
                eprintln!("Could not wait for the restored process: {error}");
            }
        }
    }
    result
}

fn install_inner(
    directory: &Path,
    plan: &Plan,
    progress: &mut InstallProgress,
) -> anyhow::Result<()> {
    let binary = directory.join("new-binary");
    ensure!(
        plan.parent_pid != std::process::id() && process_exists(plan.parent_pid),
        "The updating process is unavailable"
    );
    ensure!(
        file_hash(&binary)? == plan.binary_hash,
        "Staged executable checksum mismatch"
    );
    ensure!(
        file_hash(&plan.target)? == plan.previous_hash,
        "Installed executable changed during updating"
    );
    let mut target = plan.target.clone();
    if let Some(managed) = &plan.managed {
        #[cfg(windows)]
        let _ = launcher_mutex_name(&managed.root)?;
        let tag = managed.next["tag"]
            .as_str()
            .context("Launcher tag missing")?;
        ensure!(tag.starts_with('v'), "Invalid launcher tag");
        version(tag)?;
        let next_dir = managed.root.join("versions").join(tag);
        plain_path(&next_dir)?;
        let version_stage = directory.join("managed-version");
        fs::create_dir(&version_stage)?;
        let staged_executable = version_stage.join(executable_name());
        write_new(&staged_executable, &fs::read(&binary)?)?;
        fs::set_permissions(
            &staged_executable,
            fs::metadata(&plan.target)?.permissions(),
        )?;
        write_new(
            &version_stage.join("install.json"),
            &serde_json::to_vec(&managed.next)?,
        )?;
        target = next_dir.join(executable_name());
    }
    publish_marker(directory, "ready")?;
    let confirmation_deadline = Instant::now() + Duration::from_secs(15);
    loop {
        ensure!(
            !directory.join("cancel").exists(),
            "Update was cancelled before confirmation"
        );
        if directory.join("commit").is_file() {
            break;
        }
        ensure!(
            process_exists(plan.parent_pid),
            "The app exited before confirming the update"
        );
        ensure!(
            Instant::now() < confirmation_deadline,
            "The app did not confirm the update; the running version was kept"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    // commit 後はアプリの正常終了を待ち切る。処理中のHTTP保存を時間で中断しない。
    while process_exists(plan.parent_pid) {
        std::thread::sleep(Duration::from_millis(200));
    }
    progress.parent_stopped = true;
    if let Some(managed) = &plan.managed {
        progress.launcher_lock = Some(acquire_launcher_mutex(&managed.root)?);
        let pointer = managed.root.join("cache.json");
        plain_path(&pointer)?;
        let current: serde_json::Value = serde_json::from_slice(&fs::read(pointer)?)?;
        if current == managed.next {
            plain_path(&target.with_file_name("install.json"))?;
            ensure!(
                serde_json::from_slice::<serde_json::Value>(&fs::read(
                    target.with_file_name("install.json")
                )?)? == managed.next,
                "Existing new version metadata does not match the verified update"
            );
            ensure!(
                file_hash(&target)? == plan.binary_hash,
                "Existing new version checksum does not match the verified update"
            );
        } else {
            ensure!(
                current == managed.previous,
                "Launcher cache changed while waiting for shutdown"
            );
            let next_dir = target.parent().context("New version directory missing")?;
            ensure!(
                !next_dir.exists(),
                "The target release directory already exists"
            );
            fs::rename(directory.join("managed-version"), next_dir)?;
            progress.published = true;
            launcher_pointer(managed, &managed.previous, &managed.next, directory)?;
        }
    } else {
        apply_binary(
            &binary,
            &target,
            &plan.previous_hash,
            &plan.binary_hash,
            directory,
        )?;
    }
    // ヘルパー境界でも実行形式を再検査し、不正な実体を起動せず旧版へ戻す。
    // 置換後の実体を1回だけ読み、同じバイト列でハッシュ照合と形式検査を行う。
    // 2回読むと、照合と形式検査の間に差し替えられた内容を見逃す余地も生じる。
    {
        // file_hash と同じく、リンク等を経由しない通常パスであることを先に確認する。
        plain_path(&target)?;
        let binary = fs::read(&target)?;
        ensure!(
            digest(&binary) == plan.binary_hash,
            "Installed update changed before relaunch"
        );
        if cfg!(any(windows, target_os = "macos")) {
            validate_binary(&binary, cfg!(windows))?;
        }
    }
    let mut child = relaunch(plan, &target)?;
    drop(progress.file_lock.take());
    // 再起動開始後のマーカー書込み失敗で、動作中の新版のキャッシュを戻さない。
    let _ = write_new(&directory.join("complete"), b"complete");
    // 管理ランチャーと同様、子の存命中は同じ名前付きMutexを保持する。
    if plan.managed.is_some()
        && let Err(error) = child.wait()
    {
        eprintln!("Could not wait for the updated process: {error}");
    }
    Ok(())
}

/// 更新前と同じ引数・作業ディレクトリでアプリを起動する。
///
/// 新版の再起動と失敗時の旧版の再起動で共用する。標準入出力はすべてnullにし、
/// ヘルパーのログ（標準エラー）を子へ渡さない。起動に成功した場合だけ
/// `Restarted PID: <pid>` をログへ1回出力する（tests/update.rs がこの形式を解析する）。
/// エラー文は呼び出し側の文脈で付ける。
fn start_app(target: &Path, plan: &Plan) -> std::io::Result<Child> {
    let mut command = Command::new(target);
    command
        .args(&plan.args)
        .current_dir(&plan.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hidden(&mut command);
    let child = command.spawn()?;
    eprintln!("Restarted PID: {}", child.id());
    Ok(child)
}

fn relaunch(plan: &Plan, target: &Path) -> anyhow::Result<Child> {
    start_app(target, plan).context("Could not relaunch the update")
}

fn recover_previous(directory: &Path, plan: &Plan) -> anyhow::Result<Child> {
    if let Some(managed) = &plan.managed {
        let pointer = managed.root.join("cache.json");
        plain_path(&pointer)?;
        let current: serde_json::Value = serde_json::from_slice(&fs::read(pointer)?)?;
        ensure!(
            current == managed.previous,
            "Another launcher changed the selected version; previous version was not restarted"
        );
    }
    let current = file_hash(&plan.target).ok();
    if current.as_deref() != Some(&plan.previous_hash) {
        ensure!(
            plan.managed.is_none()
                && (current.is_none() || current.as_deref() == Some(&plan.binary_hash)),
            "The installed executable changed; automatic recovery stopped"
        );
        let backup = directory.join("previous-binary");
        ensure!(
            file_hash(&backup)? == plan.previous_hash,
            "The recovery executable checksum does not match"
        );
        replace(&backup, &plan.target)?;
    }
    start_app(&plan.target, plan)
        .context("The old executable was preserved, but could not be restarted")
}

fn apply_binary(
    binary: &Path,
    target: &Path,
    old_hash: &str,
    new_hash: &str,
    directory: &Path,
) -> anyhow::Result<()> {
    ensure!(
        file_hash(target)? == old_hash,
        "Installed executable changed during updating"
    );
    ensure!(
        file_hash(binary)? == new_hash,
        "Staged executable checksum mismatch"
    );
    let permissions = fs::metadata(target)?.permissions();
    write_new(&directory.join("previous-binary"), &fs::read(target)?)?;
    fs::set_permissions(directory.join("previous-binary"), permissions.clone())?;
    fs::set_permissions(binary, permissions)?;
    replace(binary, target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_root() -> PathBuf {
        let root = std::env::var_os("SKILVELLA_MIGRATION_TEST_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let root = std::env::temp_dir();
                #[cfg(unix)]
                let root = root.canonicalize().unwrap();
                root
            });
        plain_path(&root).unwrap();
        root
    }

    #[test]
    fn failed_download_reports_the_downloader_status() {
        // ポート1のループバックは待受がなく、外部通信なしで curl の接続失敗を再現できる。
        let error = request("https://127.0.0.1:1/", 4096, false)
            .unwrap_err()
            .to_string();
        // 終了状態の表記はOSで異なる（Windows: exit code、unix: exit status）ため、
        // 括弧で状態が付くことだけを確認し、終了コードの値には依存しない。
        assert!(
            error.starts_with("Could not retrieve the stable release over HTTPS (")
                && error.ends_with(')'),
            "{error}"
        );
    }

    #[test]
    fn stable_versions_and_checksums_are_strict() {
        assert_eq!(
            digest(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(version("v1.10.0").unwrap() > version("1.9.9").unwrap());
        for invalid in ["v1.2.3-beta", "v1.2", "v01.2.3", "../1.2.3", "v1.2.3/"] {
            assert!(version(invalid).is_err());
        }
        let hash = "a".repeat(64);
        assert_eq!(
            checksum(&format!("{hash}  release.zip\n"), "release.zip").unwrap(),
            hash
        );
        assert!(
            checksum(
                &format!("{hash}  release.zip\n{hash}  release.zip"),
                "release.zip"
            )
            .is_err()
        );
        assert!(checksum(&format!("{hash}  ../release.zip"), "release.zip").is_err());
        assert!(checksum(&format!("{hash}  other.zip"), "release.zip").is_err());
    }

    #[test]
    fn archive_rejects_traversal_and_missing_executable() {
        fn zip_archive(names: &[&str]) -> Vec<u8> {
            let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
            for name in names {
                zip.start_file(*name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                zip.write_all(if *name == BUILD_INFO {
                    b"version=1.0.2\n"
                } else {
                    b"executable"
                })
                .unwrap();
            }
            zip.finish().unwrap().into_inner()
        }
        fn tar_archive(names: &[&str], executable_type: tar::EntryType) -> Vec<u8> {
            let gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            let mut tar = tar::Builder::new(gzip);
            for name in names {
                let kind = if *name == "skilvella" {
                    executable_type
                } else {
                    tar::EntryType::Regular
                };
                let bytes: &[u8] = if !kind.is_file() {
                    b""
                } else if *name == BUILD_INFO {
                    b"version=1.0.2\n"
                } else {
                    b"executable"
                };
                let mut header = tar::Header::new_gnu();
                // 不正パスも抽出側へ渡すため、Builderのパス検証を経由しない。
                header.as_mut_bytes()[..name.len()].copy_from_slice(name.as_bytes());
                header.set_entry_type(kind);
                header.set_size(bytes.len() as u64);
                if kind.is_symlink() || kind.is_hard_link() {
                    header.set_link_name("outside").unwrap();
                }
                header.set_cksum();
                tar.append(&header, bytes).unwrap();
            }
            tar.into_inner().unwrap().finish().unwrap()
        }
        for zip in [true, false] {
            let name = if zip { "skilvella.exe" } else { "skilvella" };
            let names = [name, "LICENSE", "CHANGELOG.md", "README.txt", BUILD_INFO];
            let archive = |names: &[&str]| {
                if zip {
                    zip_archive(names)
                } else {
                    tar_archive(names, tar::EntryType::Regular)
                }
            };
            let valid = archive(&names);
            assert_eq!(extract(&valid, zip, "1.0.2").unwrap(), b"executable");
            let mut mispackaged = names;
            mispackaged[4] = "BUILD-INFO";
            assert!(extract(&archive(&mispackaged), zip, "1.0.2").is_err());
            for path in ["../skilvella.exe", "/skilvella", "nested/skilvella"] {
                let mut unexpected = names.to_vec();
                unexpected.push(path);
                let error = extract(&archive(&unexpected), zip, "1.0.2").unwrap_err();
                assert!(
                    error
                        .to_string()
                        .contains("Unexpected or duplicate release archive entry")
                );
            }
            let error = extract(&archive(&names[1..]), zip, "1.0.2").unwrap_err();
            assert!(error.to_string().contains("Release archive is incomplete"));
            let error = extract(&valid, zip, "1.0.3").unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("Release tag and build version disagree")
            );
        }
        let names = [
            "skilvella",
            "LICENSE",
            "CHANGELOG.md",
            "README.txt",
            BUILD_INFO,
        ];
        for kind in [tar::EntryType::Symlink, tar::EntryType::Link] {
            let error = extract(&tar_archive(&names, kind), false, "1.0.2").unwrap_err();
            assert!(error.to_string().contains("not a regular file"));
        }
    }

    #[test]
    fn apply_requires_both_checksums_and_preserves_recovery_copy() {
        let root = fixture_root();
        let directory = root.join(format!("updater-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&directory).unwrap();
        let target = directory.join("target");
        let staged = directory.join("new-binary");
        write_new(&target, b"old").unwrap();
        write_new(&staged, b"new").unwrap();
        assert!(
            apply_binary(
                &staged,
                &target,
                &digest(b"wrong"),
                &digest(b"new"),
                &directory
            )
            .is_err()
        );
        assert!(
            apply_binary(
                &staged,
                &target,
                &digest(b"old"),
                &digest(b"wrong"),
                &directory
            )
            .is_err()
        );
        assert_eq!(fs::read(&target).unwrap(), b"old");
        apply_binary(
            &staged,
            &target,
            &digest(b"old"),
            &digest(b"new"),
            &directory,
        )
        .unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"new");
        assert_eq!(fs::read(directory.join("previous-binary")).unwrap(), b"old");
        replace(&directory.join("previous-binary"), &target).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"old");
    }

    #[test]
    fn launcher_cache_switch_preserves_fields_and_detects_conflicts() {
        let root = fixture_root();
        let root = root.join(format!("updater-launcher-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let installed = root.join("versions/v1.0.1");
        fs::create_dir_all(&installed).unwrap();
        let target = installed.join(executable_name());
        write_new(&target, b"old").unwrap();
        write_new(&root.join("Skilvella-Latest.ps1"), b"fixture").unwrap();
        let previous = serde_json::json!({"schema":1,"tag":"v1.0.1","exe_sha256":digest(b"old"),"zip_sha256":"a".repeat(64),"future_setting":true});
        write_new(
            &root.join("cache.json"),
            &serde_json::to_vec(&previous).unwrap(),
        )
        .unwrap();
        write_new(
            &installed.join("install.json"),
            &serde_json::to_vec(&previous).unwrap(),
        )
        .unwrap();
        let plan = managed(&target, "v1.0.2", &digest(b"archive"), &digest(b"new"))
            .unwrap()
            .unwrap();
        assert_eq!(plan.next["future_setting"], true);
        launcher_pointer(&plan, &plan.previous, &plan.next, &root).unwrap();
        assert!(launcher_pointer(&plan, &plan.previous, &plan.next, &root).is_err());
        let published = root.join("versions/v1.0.2");
        fs::create_dir(&published).unwrap();
        write_new(&published.join(executable_name()), b"new").unwrap();
        write_new(
            &published.join("install.json"),
            &serde_json::to_vec(&plan.next).unwrap(),
        )
        .unwrap();
        rollback_managed(&plan, &root).unwrap();
        assert!(!published.exists());
        assert_eq!(
            fs::read(root.join("failed-version").join(executable_name())).unwrap(),
            b"new"
        );
        assert_eq!(fs::read(&target).unwrap(), b"old");
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(
                &fs::read(root.join("cache.json")).unwrap()
            )
            .unwrap(),
            previous
        );
    }

    #[test]
    fn target_lock_prevents_concurrent_helpers() {
        let root = fixture_root();
        let directory = root.join(format!("updater-lock-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&directory).unwrap();
        let target = directory.join(executable_name());
        let first = installation_lock(&target).unwrap();
        assert!(installation_lock(&target).is_err());
        drop(first);
        assert!(installation_lock(&target).is_ok());
    }

    #[test]
    fn failed_restart_restores_previous_executable() {
        let root = fixture_root();
        let directory = root.join(format!("updater-rollback-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&directory).unwrap();
        let target = directory.join(executable_name());
        let staged = directory.join("new-binary");
        // OSが実行を拒否する内容で、置換後の起動失敗を再現する。
        write_new(&target, b"old-invalid-executable").unwrap();
        write_new(&staged, b"new-invalid-executable").unwrap();
        let plan = Plan {
            parent_pid: 0,
            target: target.clone(),
            previous_hash: digest(b"old-invalid-executable"),
            binary_hash: digest(b"new-invalid-executable"),
            args: vec![],
            cwd: directory.clone(),
            managed: None,
        };
        apply_binary(
            &staged,
            &target,
            &plan.previous_hash,
            &plan.binary_hash,
            &directory,
        )
        .unwrap();
        assert!(relaunch(&plan, &target).is_err());
        assert!(recover_previous(&directory, &plan).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"old-invalid-executable");
    }

    #[cfg(windows)]
    #[test]
    fn named_mutex_matches_the_powershell_launcher_and_excludes_other_processes() {
        let root = fixture_root();
        let directory = root.join(format!("updater-mutex-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&directory).unwrap();
        let script = "$identity=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value; $sha=[Security.Cryptography.SHA256]::Create(); try { $scope=[BitConverter]::ToString($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes([IO.Path]::GetFullPath($env:SKILVELLA_MUTEX_FIXTURE_ROOT).ToLowerInvariant()))).Replace('-', '') } finally { $sha.Dispose() }; $name='Local\\SkilvellaLatest-'+$identity+'-'+$scope; [Console]::WriteLine($name); $mutex=New-Object Threading.Mutex($false,$name); try { $owned=$mutex.WaitOne(0); [Console]::WriteLine($owned); if ($owned) {$mutex.ReleaseMutex()} } finally {$mutex.Dispose()}; [Console]::WriteLine([IO.Path]::GetFullPath($env:SKILVELLA_MUTEX_FIXTURE_ROOT))";
        let probe = |fixture_root: &Path| {
            let output = Command::new("pwsh")
                .args(["-NoProfile", "-NonInteractive", "-Command", script])
                .env("SKILVELLA_MUTEX_FIXTURE_ROOT", fixture_root)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout).unwrap()
        };
        let short_path = |path: &Path| unsafe {
            use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;
            let input: Vec<u16> = path
                .to_str()
                .unwrap()
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let length = GetShortPathNameW(input.as_ptr(), std::ptr::null_mut(), 0);
            assert!(length > 0);
            let mut output = vec![0u16; length as usize];
            let written = GetShortPathNameW(input.as_ptr(), output.as_mut_ptr(), length);
            assert!(written > 0 && written < length);
            PathBuf::from(String::from_utf16(&output[..written as usize]).unwrap())
        };
        // 作業ボリュームで 8.3 名の生成が無効でも、既存の別名で一致を確認する。
        let program_files = PathBuf::from(std::env::var_os("ProgramFiles").unwrap());
        let short_program_files = short_path(&program_files);
        let observed = probe(&short_program_files);
        assert_eq!(
            observed.lines().next().unwrap(),
            launcher_mutex_name(&short_program_files).unwrap(),
            "root: {short_program_files:?}; PowerShell output: {observed}"
        );
        for path in [
            directory.clone(),
            fs::canonicalize(&directory).unwrap(),
            short_path(&directory),
        ] {
            let guard = acquire_launcher_mutex(&path).unwrap();
            let blocked = probe(&path);
            let lines: Vec<_> = blocked.lines().collect();
            assert_eq!(
                lines[0],
                launcher_mutex_name(&path).unwrap(),
                "root: {path:?}; PowerShell output: {blocked}"
            );
            assert_eq!(lines[1], "False");
            drop(guard);
            assert_eq!(probe(&path).lines().nth(1), Some("True"));
        }
        // 8.3 別名がASCIIでも、実名が非ASCIIなら既存の拒否条件を維持する。
        let unicode = directory.join("non-ascii-Ä");
        fs::create_dir(&unicode).unwrap();
        assert!(launcher_mutex_name(&unicode).is_err());
        assert!(launcher_mutex_name(&short_path(&unicode)).is_err());
    }
}
