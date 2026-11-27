//! 通常ビルドから分離した、ZIP版・macOS版の配布物（R2 の portable/ に置くもの）の梱包と公開前検証。
//! Setup に同梱する README.md の書き出し（`readme`）も扱う。
use super::{
    BUILD_INFO, MAC_ASSET, MAX_ARCHIVE, MAX_CHECKSUMS, WINDOWS_ASSET, asset_name, checksum, digest,
    executable_name, verify_archive, write_new,
};
use anyhow::{Context, Result, bail, ensure};
use std::{
    ffi::OsString,
    fs::File,
    io::{Cursor, Read, Write},
    path::Path,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const ASSETS: [&str; 2] = [MAC_ASSET, WINDOWS_ASSET];
const CHECKSUMS: &str = "SHA256SUMS.txt";

#[cfg(feature = "release-update-test")]
pub mod regression;

pub fn run(args: impl Iterator<Item = OsString>) -> Result<()> {
    let args: Vec<_> = args.collect();
    match args.as_slice() {
        [command, sha, binary, directory] if command == "pack" => pack(
            sha.to_str().context("Source SHA must be ASCII")?,
            Path::new(binary),
            Path::new(directory),
        ),
        [command, sha, output] if command == "readme" => write_readme(
            sha.to_str().context("Source SHA must be ASCII")?,
            Path::new(output),
        ),
        [command, directory] if command == "checksums" => {
            let directory = Path::new(directory);
            let mut list = String::new();
            for asset in ASSETS {
                let archive = read(&directory.join(asset), MAX_ARCHIVE)?;
                list.push_str(&format!("{}  {asset}\n", digest(&archive)));
            }
            write_new(&directory.join(CHECKSUMS), list.as_bytes())
        }
        [command, directory] if command == "verify" => {
            let directory = Path::new(directory);
            let list = String::from_utf8(read(&directory.join(CHECKSUMS), MAX_CHECKSUMS)?)?;
            for asset in ASSETS {
                let sum = checksum(&list, asset)?;
                let archive = read(&directory.join(asset), MAX_ARCHIVE)?;
                verify_archive(&archive, &sum, asset, VERSION)
                    .with_context(|| format!("Verify {asset}"))?;
                println!("{sum}  {asset}");
            }
            Ok(())
        }
        _ => bail!(
            "Usage: skilvella-release pack <source-sha> <binary> <output-dir> | readme <source-sha> <output-file> | checksums <output-dir> | verify <output-dir>"
        ),
    }
}

fn read(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)
        .with_context(|| format!("Open {}", path.display()))?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= limit,
        "File is too large: {}",
        path.display()
    );
    Ok(bytes)
}

/// README.md の相対リンク `](path)` を、その版のソースを指す絶対URLへ書き換える。
/// 配布物にはリンク先の文書が入らないため、README.txt では読める場所を指す。
/// `http(s)://`・`//`・`#`・`mailto:` で始まるリンクと、空・空白を含むリンク先は変えず、`path#anchor` はアンカーを残す。
fn absolutize_links(readme: &str, repository: &str, sha: &str) -> String {
    let base = repository.trim_end_matches('/');
    let base = base.strip_suffix(".git").unwrap_or(base);
    let mut out = String::with_capacity(readme.len());
    let mut rest = readme;
    while let Some(start) = rest.find("](") {
        let (head, tail) = rest.split_at(start + 2);
        out.push_str(head);
        let Some(end) = tail.find(')') else {
            rest = tail;
            break;
        };
        let target = &tail[..end];
        let external = ["http://", "https://", "//", "#", "mailto:"]
            .iter()
            .any(|prefix| target.starts_with(prefix));
        if external || target.is_empty() || target.contains(char::is_whitespace) {
            out.push_str(target);
        } else {
            let (path, anchor) = target.split_at(target.find('#').unwrap_or(target.len()));
            let path = path.trim_start_matches("./");
            out.push_str(&format!("{base}/blob/{sha}/{path}{anchor}"));
        }
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

fn check_sha(sha: &str) -> Result<()> {
    ensure!(
        sha.len() == 40 && sha.bytes().all(|c| c.is_ascii_hexdigit()),
        "Expected a full Git commit SHA"
    );
    Ok(())
}

/// 配布物に同梱する README。ZIP版・macOS版の README.txt と Setup の README.md で同じ内容にする。
fn bundled_readme(sha: &str) -> String {
    absolutize_links(
        include_str!("../../README.md"),
        env!("CARGO_PKG_REPOSITORY"),
        sha,
    )
}

/// `readme` サブコマンド。梱包の前段で書き出すため、既存ファイルは上書きしない（`pack` の出力と同じ）。
fn write_readme(sha: &str, output: &Path) -> Result<()> {
    check_sha(sha)?;
    write_new(output, bundled_readme(sha).as_bytes())
}

fn pack(sha: &str, binary: &Path, directory: &Path) -> Result<()> {
    check_sha(sha)?;
    let asset =
        asset_name().context("Build the release package on Windows x64 or Apple Silicon")?;
    let zip = asset == WINDOWS_ASSET;
    let target = if zip {
        "x86_64-pc-windows-msvc"
    } else {
        "aarch64-apple-darwin"
    };
    let signing = if zip { "unsigned" } else { "adhoc" };
    let binary = read(binary, MAX_ARCHIVE)?;
    let info = format!(
        "version={VERSION}\nsource={sha}\ntarget={target}\nconfiguration=release\nsigning={signing}\n"
    );
    let readme = bundled_readme(sha);
    let entries: [(&str, &[u8]); 5] = [
        (executable_name(), &binary),
        ("LICENSE", include_bytes!("../../LICENSE")),
        ("CHANGELOG.md", include_bytes!("../../CHANGELOG.md")),
        ("README.txt", readme.as_bytes()),
        (BUILD_INFO, info.as_bytes()),
    ];
    let archive = if zip {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, bytes) in entries {
            writer.start_file(name, options)?;
            writer.write_all(bytes)?;
        }
        writer.finish()?.into_inner()
    } else {
        let gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut writer = tar::Builder::new(gzip);
        for (name, bytes) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_mode(if name == executable_name() {
                0o755
            } else {
                0o644
            });
            header.set_size(bytes.len() as u64);
            writer.append_data(&mut header, name, bytes)?;
        }
        writer.into_inner()?.finish()?
    };
    // ZIPとtar.gzのどちらも、更新機構が読み取るバイナリが梱包元と一致してから保存する。
    let sum = digest(&archive);
    ensure!(
        verify_archive(&archive, &sum, asset, VERSION)? == binary,
        "Packaged executable differs from the input"
    );
    write_new(&directory.join(asset), &archive)?;
    println!("{sum}  {asset}");
    Ok(())
}
