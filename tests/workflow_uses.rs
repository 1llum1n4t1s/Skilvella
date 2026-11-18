//! GitHub Actions の `uses:` はコミット SHA に固定する。
//!
//! タグ参照は署名 Action と `actions/checkout@v7.0.1`・`Swatinem/rust-cache@v2.9.3`（830769d）で
//! 繰り返された。文書へ「SHA で指定する」と書いたあとも、検査がなければ次の `uses:` はタグのまま通る。
//!
//! 例外は自作の署名 Action `1llum1n4t1s/Sigilune@v1` の完全一致だけ。この移動タグは、Sigilune が
//! その版の配信に成功した commit へだけ動かす運用で、第三者 Action は SHA 固定のまま。

use std::{fs, path::Path};

#[path = "support/line_exception.rs"]
mod line_exception;

#[test]
fn repository_workflows_pin_actions_to_commit_shas() {
    let mut seen = 0;
    for path in workflow_files() {
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let relative = relative_path(&path);
        let errors = workflow_use_errors(&relative, &text);
        assert!(errors.is_empty(), "{}", errors.join("\n"));
        seen += text
            .lines()
            .filter(|line| uses_reference(line).is_some())
            .count();
    }
    assert!(
        seen >= 10,
        "workflow scanner did not see the pinned actions"
    );
}

#[test]
fn rejects_the_tag_refs_corrected_in_830769d() {
    let past = "\
      - uses: actions/checkout@v7.0.1
      - uses: Swatinem/rust-cache@v2.9.3
        # actions/checkout@v7.0.1
        uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
";
    let errors = workflow_use_errors(".github/workflows/ci.yml", past);
    assert_eq!(errors.len(), 2, "{errors:?}");
    assert!(errors[0].contains(".github/workflows/ci.yml:1:"));
    assert!(errors[0].contains("actions/checkout@v7.0.1"));
    assert!(errors[0].contains("40-character commit SHA"));
    assert!(errors[1].contains("Swatinem/rust-cache@v2.9.3"));
}

#[test]
fn allows_only_the_exact_own_moving_tag_of_sigilune() {
    let allowed = "\
      - uses: 1llum1n4t1s/Sigilune@v1
        uses: \"1llum1n4t1s/Sigilune@v1\" # 配信成功済みの移動タグ
";
    let errors = workflow_use_errors(".github/workflows/release.yml", allowed);
    assert!(errors.is_empty(), "{errors:?}");

    let rejected = [
        "1llum1n4t1s/Sigilune@main",
        "1llum1n4t1s/Sigilune@v1.1",
        "1llum1n4t1s/Sigilune@v1.1.4",
        "1llum1n4t1s/Sigilune@v2",
        "1llum1n4t1s/Sigilune@V1",
        "1llum1n4t1s/Sigilune@v1-rc",
        "1llum1n4t1s/Sigilune/sub@v1",
        "1llum1n4t1s/Sigilune",
        "someone-else/Sigilune@v1",
        "1llum1n4t1s/Other@v1",
        "actions/checkout@v1",
        "Swatinem/rust-cache@v1",
    ];
    for reference in rejected {
        let text = format!("      - uses: {reference}\n");
        let errors = workflow_use_errors(".github/workflows/release.yml", &text);
        assert_eq!(errors.len(), 1, "{reference} must be rejected: {errors:?}");
        assert!(errors[0].contains(reference), "{errors:?}");
        assert!(errors[0].contains("40-character commit SHA"), "{errors:?}");
    }
}

fn workflow_files() -> Vec<std::path::PathBuf> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows");
    let mut paths: Vec<_> = fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
        .map(|entry| {
            entry
                .unwrap_or_else(|error| panic!("read workflow entry: {error}"))
                .path()
        })
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "yml" || extension == "yaml")
        })
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no workflow files");
    paths
}

fn relative_path(path: &Path) -> String {
    path.strip_prefix(env!("CARGO_MANIFEST_DIR"))
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn workflow_use_errors(file: &str, text: &str) -> Vec<String> {
    let mut errors = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let Some(reference) = uses_reference(line) else {
            continue;
        };
        let line_number = index + 1;
        match line_exception::exemption(line) {
            Ok(true) => continue,
            Ok(false) => {}
            Err(message) => {
                errors.push(format!("{file}:{line_number}: {message}"));
                continue;
            }
        }
        if is_commit_pinned(reference) {
            continue;
        }
        errors.push(format!(
            "{file}:{line_number}: `uses: {reference}` is a branch or tag. Pin a 40-character commit SHA, for example `uses: actions/checkout@<40-hex-sha> # vX.Y.Z`. Moving tags already shipped as actions/checkout@v7.0.1 and Swatinem/rust-cache@v2.9.3. Only the exact `1llum1n4t1s/Sigilune@v1` is allowed"
        ));
    }
    errors
}

fn uses_reference(line: &str) -> Option<&str> {
    let code = line_exception::code_before_comment(line).trim();
    let rest = code.strip_prefix("- ").map(str::trim).unwrap_or(code);
    let rest = rest.strip_prefix("uses:")?.trim();
    let reference = rest.split_whitespace().next()?;
    let reference = reference.trim_matches('"').trim_matches('\'');
    (!reference.is_empty()).then_some(reference)
}

/// 自作の署名 Action だけは、配信が成功した commit にだけ動かす移動タグ `v1` で参照する。
/// 完全一致だけを許し、`Sigilune@main`・`Sigilune@v1.1`・他者の `@v1` は許さない。
const OWN_MOVING_TAG_ACTION: &str = "1llum1n4t1s/Sigilune@v1";

fn is_commit_pinned(reference: &str) -> bool {
    if reference.starts_with("./") || reference == OWN_MOVING_TAG_ACTION {
        return true;
    }
    let Some((repository, revision)) = reference.rsplit_once('@') else {
        return false;
    };
    !repository.is_empty()
        && repository.contains('/')
        && !repository.ends_with('/')
        && revision.len() == 40
        && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
}
