//! 公開済みの起点と、テストに配信する候補版の実体を用意する。
use super::{
    BUILD_INFO, CHECKSUMS, MAX_ARCHIVE, MAX_CHECKSUMS, VERSION, asset_name, checksum, digest,
    executable_name, read, verify_archive, write_new,
};
use crate::update::{FEED, executable, plain_path, request, validate_binary, version};
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

/// 起点（更新前の版）の取得元。
enum Origin<'a> {
    /// 公開中の版 `X.Y.Z`。R2 の `portable/X.Y.Z/` から取得する。
    Published(&'a str),
    /// アーカイブと SHA256SUMS.txt を置いたローカルの絶対パス。版は同梱の BUILD-INFO.txt から読む。
    /// 公開版を起点にできない初回の移行版や、複製ツリーの版を下げて作った起点の検証に使う。
    Folder(&'a Path),
}

/// `origin` は `X.Y.Z`（v なし）か、起点のアーカイブとチェックサムを置いた絶対パスのフォルダー。
pub fn prepare(origin: &str, distribution: &Path, target: &Path) -> Result<Inputs> {
    let path = Path::new(origin);
    let origin = if path.is_absolute() {
        Origin::Folder(path)
    } else {
        // 公開物の版は数字始まり。v 付きは受け付けない（配信先のパスと一致させるため）。
        ensure!(
            origin.starts_with(|c: char| c.is_ascii_digit()),
            "Expected X.Y.Z or an absolute folder as the previous release"
        );
        Origin::Published(origin)
    };
    let asset = asset_name().context("Run on Windows x64 or Apple Silicon Mac")?;
    plain_path(distribution)?;
    let sums = read(&distribution.join(CHECKSUMS), MAX_CHECKSUMS)?;
    let archive = read(&distribution.join(asset), MAX_ARCHIVE)?;
    let sum = checksum(std::str::from_utf8(&sums)?, asset)?;
    let candidate_binary = verify_archive(&archive, &sum, asset, VERSION)
        .with_context(|| format!("Verify candidate {asset}"))?;

    // フィクスチャを起動する前に、起点の実体を用意する。公開版は R2 から、ローカル起点はフォルダーから読む。
    let (previous_sums, previous_archive) = match origin {
        Origin::Published(previous) => {
            version(previous)?;
            let base = format!("{FEED}/{previous}");
            (
                request(&format!("{base}/{CHECKSUMS}"), MAX_CHECKSUMS)
                    .context("Download previous release checksums")?,
                request(&format!("{base}/{asset}"), MAX_ARCHIVE)
                    .context("Download previous release archive")?,
            )
        }
        Origin::Folder(folder) => {
            plain_path(folder)?;
            (
                read(&folder.join(CHECKSUMS), MAX_CHECKSUMS)
                    .context("Read previous release checksums")?,
                read(&folder.join(asset), MAX_ARCHIVE).context("Read previous release archive")?,
            )
        }
    };
    let expected = checksum(std::str::from_utf8(&previous_sums)?, asset)?;
    ensure!(
        digest(&previous_archive) == expected,
        "Previous release checksum mismatch"
    );
    let (binary, build_version) = previous_files(&previous_archive)?;
    // 版は同梱の BUILD-INFO.txt を正とし、公開版の指定とは一致を確かめる。
    let previous = match origin {
        Origin::Published(previous) => {
            ensure!(
                build_version.as_deref() == Some(previous),
                "Previous release build version differs from {previous}"
            );
            previous.to_owned()
        }
        Origin::Folder(_) => build_version.context("Previous archive has no build version")?,
    };
    ensure!(
        version(&previous)? < version(VERSION)?,
        "The previous release must be older than {VERSION}"
    );
    validate_binary(&binary, cfg!(windows))?;
    write_new(target, &binary)?;
    executable(target)?;

    Ok(Inputs {
        previous,
        candidate: VERSION,
        asset,
        sums,
        archive,
        candidate_binary,
    })
}

// 旧版には梱包形式の差がある。公開物のハッシュを照合し、実行ファイルだけを
// 新規ファイルに書く。アーカイブのパスや権限をファイルシステムへ適用しない。
fn previous_files(archive: &[u8]) -> Result<(Vec<u8>, Option<String>)> {
    let name = executable_name();
    let mut binary = Vec::new();
    let mut info = Vec::new();
    if cfg!(windows) {
        let mut zip = zip::ZipArchive::new(Cursor::new(archive))?;
        let entry = zip.by_name(name)?;
        ensure!(
            entry.is_file() && !entry.is_symlink(),
            "Expected a regular previous executable"
        );
        entry.take(MAX_ARCHIVE + 1).read_to_end(&mut binary)?;
        if let Ok(entry) = zip.by_name(BUILD_INFO) {
            entry.take(16385).read_to_end(&mut info)?;
        }
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
            } else if components == Path::new(BUILD_INFO) {
                entry.take(16385).read_to_end(&mut info)?;
            }
        }
    }
    ensure!(
        !binary.is_empty() && binary.len() as u64 <= MAX_ARCHIVE,
        "Invalid previous executable size"
    );
    // BUILD-INFO.txt は `version=X.Y.Z` の行を1つだけ持つ（verify_archive と同じ形式）。無ければ None。
    let build_version = std::str::from_utf8(&info).ok().and_then(|text| {
        let mut versions = text
            .lines()
            .filter_map(|line| line.strip_prefix("version="));
        let first = versions.next()?;
        versions.next().is_none().then(|| first.to_owned())
    });
    Ok((binary, build_version))
}

pub fn workspace(parent: &Path) -> Result<PathBuf> {
    plain_path(parent)?;
    ensure!(parent.is_dir(), "The test workspace parent must exist");
    let path = parent.join(format!("skilvella-release-update-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&path)?;
    Ok(path)
}
