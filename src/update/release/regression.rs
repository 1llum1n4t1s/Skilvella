//! 公開済みの起点と、テストに配信する候補版の実体を用意する。
use super::{
    CHECKSUMS, MAX_ARCHIVE, MAX_CHECKSUMS, VERSION, asset_name, checksum, digest, executable_name,
    read, verify_archive, write_new,
};
use crate::update::{REPO, executable, plain_path, request, validate_binary, version};
use anyhow::{Context, Result, ensure};
use std::{
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
};

pub struct Inputs {
    pub previous: String,
    pub candidate: &'static str,
    pub asset: &'static str,
    pub sums: Vec<u8>,
    pub archive: Vec<u8>,
    pub candidate_binary: Vec<u8>,
}

pub fn prepare(previous_tag: &str, distribution: &Path, target: &Path) -> Result<Inputs> {
    let previous = previous_tag
        .strip_prefix('v')
        .context("Expected a vX.Y.Z tag")?;
    ensure!(
        version(previous)? < version(VERSION)?,
        "The previous release must be older than {VERSION}"
    );
    let asset = asset_name().context("Run on Windows x64 or Apple Silicon Mac")?;
    plain_path(distribution)?;
    let sums = read(&distribution.join(CHECKSUMS), MAX_CHECKSUMS)?;
    let archive = read(&distribution.join(asset), MAX_ARCHIVE)?;
    let sum = checksum(std::str::from_utf8(&sums)?, asset)?;
    let candidate_binary = verify_archive(&archive, &sum, asset, VERSION)
        .with_context(|| format!("Verify candidate {asset}"))?;

    // フィクスチャを起動する前に、本物の公開版を GitHub から取得する。
    let base = format!("{REPO}/releases/download/{previous_tag}");
    let previous_sums = request(&format!("{base}/{CHECKSUMS}"), MAX_CHECKSUMS, false)
        .context("Download previous release checksums")?;
    let previous_archive = request(&format!("{base}/{asset}"), MAX_ARCHIVE, false)
        .context("Download previous release archive")?;
    let expected = checksum(std::str::from_utf8(&previous_sums)?, asset)?;
    ensure!(
        digest(&previous_archive) == expected,
        "Previous release checksum mismatch"
    );
    let binary = previous_binary(&previous_archive)?;
    validate_binary(&binary, cfg!(windows))?;
    write_new(target, &binary)?;
    executable(target)?;

    Ok(Inputs {
        previous: previous.to_owned(),
        candidate: VERSION,
        asset,
        sums,
        archive,
        candidate_binary,
    })
}

// 旧版には梱包形式の差がある。公開物のハッシュを照合し、実行ファイルだけを
// 新規ファイルに書く。アーカイブのパスや権限をファイルシステムへ適用しない。
fn previous_binary(archive: &[u8]) -> Result<Vec<u8>> {
    let name = executable_name();
    let mut binary = Vec::new();
    if cfg!(windows) {
        let mut zip = zip::ZipArchive::new(Cursor::new(archive))?;
        let entry = zip.by_name(name)?;
        ensure!(
            entry.is_file() && !entry.is_symlink(),
            "Expected a regular previous executable"
        );
        entry.take(MAX_ARCHIVE + 1).read_to_end(&mut binary)?;
    } else {
        let gzip = flate2::read::GzDecoder::new(Cursor::new(archive));
        let mut tar = tar::Archive::new(gzip.take(MAX_ARCHIVE * 2));
        for entry in tar.entries()? {
            let entry = entry?;
            let mut components = entry.path()?.into_owned();
            if let Ok(path) = components.strip_prefix(".") {
                components = path.to_owned();
            }
            if components == Path::new(name) {
                ensure!(
                    binary.is_empty() && entry.header().entry_type().is_file(),
                    "Expected one regular previous executable"
                );
                entry.take(MAX_ARCHIVE + 1).read_to_end(&mut binary)?;
            }
        }
    }
    ensure!(
        !binary.is_empty() && binary.len() as u64 <= MAX_ARCHIVE,
        "Invalid previous executable size"
    );
    Ok(binary)
}

pub fn workspace(parent: &Path) -> Result<PathBuf> {
    plain_path(parent)?;
    ensure!(parent.is_dir(), "The test workspace parent must exist");
    let path = parent.join(format!("skilvella-release-update-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&path)?;
    Ok(path)
}
