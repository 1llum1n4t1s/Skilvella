//! 配布スクリプトは、別のツールが書いた成果物の名前や並びを組み立て直さない。
//!
//! `SkilvellaApp-$version-win-full.nupkg` は vpk の実際の名前と違い、v1.1.7 の
//! Setup 配信が梱包直後に止まった（cadee57）。`SHA256SUMS.txt` を `$assets` の順で
//! 連結すると、`skilvella-release checksums` の行順とずれて 1.1.9 の配信が失敗した（835d70d）。

use std::{fs, path::Path};

#[path = "support/line_exception.rs"]
mod line_exception;

#[test]
fn release_scripts_read_the_artifact_the_tool_wrote() {
    let mut seen_pack = false;
    let mut seen_publish = false;
    for path in script_files() {
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let relative = relative_path(&path);
        if relative.ends_with("pack-windows-installer.ps1") {
            seen_pack = true;
        }
        if relative.ends_with("publish-portable-r2.ps1") {
            seen_publish = true;
        }
        let errors = release_script_errors(&relative, &text);
        assert!(errors.is_empty(), "{}", errors.join("\n"));
    }
    assert!(
        seen_pack && seen_publish,
        "release scripts were not scanned"
    );
}

#[test]
fn rejects_the_channel_nupkg_name_from_cadee57() {
    let past = r#"foreach ($required in @('releases.win.json', "SkilvellaApp-$version-win-full.nupkg")) {
    if (!(Test-Path -LiteralPath (Join-Path $packages $required) -PathType Leaf)) { throw "Update feed file was not generated: $required" }
}"#;
    let errors = release_script_errors("tools/pack-windows-installer.ps1", past);
    assert!(
        errors.iter().any(
            |error| error.contains("tools/pack-windows-installer.ps1:1:")
                && error.contains("SkilvellaApp-*-full.nupkg")
        ),
        "{errors:?}"
    );
}

#[test]
fn rejects_the_rebuilt_checksum_list_from_835d70d() {
    let past = r#"$expectedSums = ($assets | ForEach-Object { "$($hashes[$_])  $_" }) -join "`n"
if ($sumsText.Replace("`r`n", "`n").Trim() -cne $expectedSums) { throw "$sumsName does not match the archives" }"#;
    let errors = release_script_errors("tools/publish-portable-r2.ps1", past);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].contains("tools/publish-portable-r2.ps1:1:"));
    assert!(errors[0].contains("SHA256SUMS.txt"));
    assert!(errors[0].contains("src/update.rs"));
}

fn script_files() -> Vec<std::path::PathBuf> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tools");
    let mut paths = Vec::new();
    collect_scripts(&directory, &mut paths);
    paths.sort();
    assert!(!paths.is_empty(), "no PowerShell scripts");
    paths
}

fn collect_scripts(directory: &Path, paths: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
    {
        let path = entry
            .unwrap_or_else(|error| panic!("read script entry: {error}"))
            .path();
        if path.is_dir() {
            collect_scripts(&path, paths);
        } else if path.extension().is_some_and(|extension| extension == "ps1") {
            paths.push(path);
        }
    }
}

fn relative_path(path: &Path) -> String {
    path.strip_prefix(env!("CARGO_MANIFEST_DIR"))
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn release_script_errors(file: &str, text: &str) -> Vec<String> {
    let mut errors = Vec::new();
    let mut discovers_nupkg = false;
    let mut rebuilds_checksums = false;
    let mut rebuild_line = 0;
    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        match line_exception::exemption(line) {
            Ok(true) => continue,
            Ok(false) => {}
            Err(message) => {
                errors.push(format!("{file}:{line_number}: {message}"));
                continue;
            }
        }
        let code = line_exception::code_before_comment(line);
        if code.contains("-win-full.nupkg") {
            errors.push(format!(
                "{file}:{line_number}: do not require a full nupkg whose name inserts the channel (`-win-full.nupkg`). vpk writes `SkilvellaApp-<version>-full.nupkg`. Discover the single `SkilvellaApp-*-full.nupkg` in tools/pack-windows-installer.ps1"
            ));
        }
        if code.contains("SkilvellaApp-*-full.nupkg") {
            discovers_nupkg = true;
        }
        if !rebuilds_checksums && (code.contains("$expectedSums") || code.contains("$($hashes[")) {
            rebuilds_checksums = true;
            rebuild_line = line_number;
        }
    }
    if file.ends_with("pack-windows-installer.ps1") && !discovers_nupkg {
        errors.push(format!(
            "{file}: find the one `SkilvellaApp-*-full.nupkg` that vpk wrote. Do not build `SkilvellaApp-$version-win-full.nupkg`"
        ));
    }
    if file.ends_with("publish-portable-r2.ps1") && rebuilds_checksums {
        errors.push(format!(
            "{file}:{rebuild_line}: do not rebuild SHA256SUMS.txt from the `$assets` order. Parse each line and compare that hash to the named file, the same way `checksum` in src/update.rs does. Joining hashes in script order failed the 1.1.9 publish"
        ));
    }
    errors
}
