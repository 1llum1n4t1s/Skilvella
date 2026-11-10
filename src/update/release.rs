//! 通常ビルドから分離した、GitHub配布物の梱包と公開前検証。
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
            "Usage: skilvella-release pack <source-sha> <binary> <output-dir> | checksums <output-dir> | verify <output-dir>"
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

fn pack(sha: &str, binary: &Path, directory: &Path) -> Result<()> {
    ensure!(
        sha.len() == 40 && sha.bytes().all(|c| c.is_ascii_hexdigit()),
        "Expected a full Git commit SHA"
    );
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
    let entries: [(&str, &[u8]); 5] = [
        (executable_name(), &binary),
        ("LICENSE", include_bytes!("../../LICENSE")),
        ("CHANGELOG.md", include_bytes!("../../CHANGELOG.md")),
        ("README.txt", include_bytes!("../../README.md")),
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
