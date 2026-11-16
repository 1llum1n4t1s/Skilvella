//! コマンドの標準出力を、装飾や途中で閉じるパイプのまま照合しない。
//!
//! `./skilvella --help | grep -q` は grep が先にパイプを閉じ、macOS の版確認が
//! Broken pipe で失敗した（596f801）。`vpk --help` を色付きのまま正規表現にかけると、
//! GitHub Actions のエスケープで版の照合が外れた（aff94b7）。

use std::{fs, path::Path};

#[path = "support/line_exception.rs"]
mod line_exception;

#[test]
fn command_checks_capture_plain_output() {
    let mut seen = 0;
    for path in tracked_files() {
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let relative = relative_path(&path);
        let errors = command_output_errors(&relative, &text);
        assert!(errors.is_empty(), "{}", errors.join("\n"));
        seen += 1;
    }
    assert!(
        seen >= 3,
        "command-output scanner did not see the workflow and scripts"
    );
}

#[test]
fn rejects_the_help_pipe_from_596f801() {
    let past = r#"          ./target/release/skilvella --help | grep -qx "Skilvella $EXPECTED_VERSION" || { echo "unexpected version" >&2; exit 1; }"#;
    let errors = command_output_errors(".github/workflows/release.yml", past);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].contains(".github/workflows/release.yml:1:"));
    assert!(errors[0].contains("grep -q"));
    assert!(errors[0].contains("help_text="));
}

#[test]
fn rejects_the_colored_vpk_help_from_aff94b7() {
    let past = "$vpkHelp = vpk --help | Out-String";
    let errors = command_output_errors("tools/pack-windows-installer.ps1", past);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].contains("tools/pack-windows-installer.ps1:1:"));
    assert!(errors[0].contains("ANSI"));
    assert!(errors[0].contains("-replace"));
}

#[test]
fn allows_matching_text_that_was_already_captured() {
    let current = r#"printf '%s\n' "$help_text" | grep -qx "Skilvella $EXPECTED_VERSION""#;
    let errors = command_output_errors(".github/workflows/release.yml", current);
    assert!(errors.is_empty(), "{errors:?}");
    let stripped = r#"$help = (vpk --help 2>&1 | Out-String) -replace "`e\[[0-9;]*m", ''"#;
    assert!(command_output_errors("tools/pack-windows-installer.ps1", stripped).is_empty());
}

fn tracked_files() -> Vec<std::path::PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut paths = Vec::new();
    collect(
        &root.join(".github/workflows"),
        &["yml", "yaml"],
        &mut paths,
    );
    collect(&root.join("tools"), &["ps1", "sh"], &mut paths);
    paths.sort();
    assert!(!paths.is_empty(), "no command files");
    paths
}

fn collect(directory: &Path, extensions: &[&str], paths: &mut Vec<std::path::PathBuf>) {
    let entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()));
    for entry in entries {
        let path = entry
            .unwrap_or_else(|error| panic!("read command file: {error}"))
            .path();
        if path.is_dir() {
            collect(&path, extensions, paths);
        } else if path
            .extension()
            .is_some_and(|extension| extensions.iter().any(|expected| *expected == extension))
        {
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

fn command_output_errors(file: &str, text: &str) -> Vec<String> {
    let mut errors = Vec::new();
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
        if pipes_live_command_into_grep_q(code) {
            errors.push(format!(
                "{file}:{line_number}: do not pipe a command into grep -q. Capture the output, then match the saved text (`help_text=\"$(./target/release/skilvella --help)\"` and `printf '%s\\n' \"$help_text\" | grep -q`). A direct pipe failed the macOS version check when grep closed the pipe"
            ));
        }
        if code.contains("vpk --help") && !code.contains("-replace") {
            errors.push(format!(
                "{file}:{line_number}: strip ANSI color from `vpk --help` on the same statement before matching it: `(vpk --help | Out-String) -replace \"`e\\[[0-9;]*m\", ''`. Colored output on GitHub Actions broke the Velopack version check"
            ));
        }
    }
    errors
}

fn pipes_live_command_into_grep_q(code: &str) -> bool {
    let mut segments = code.split('|');
    let command = segments
        .next()
        .unwrap_or("")
        .split_whitespace()
        .next()
        .unwrap_or("");
    if matches!(command, "printf" | "echo") {
        return false;
    }
    segments.any(is_quiet_grep)
}

fn is_quiet_grep(segment: &str) -> bool {
    let mut tokens = segment.split_whitespace();
    if tokens.next() != Some("grep") {
        return false;
    }
    tokens.any(|token| {
        token == "--quiet"
            || token == "--silent"
            || (token.starts_with('-') && !token.starts_with("--") && token.contains('q'))
    })
}
