//! テスト用のディレクトリリンク作成フィクスチャ。
//!
//! 被検査コード（`skilvella::library::create_directory_link`）とは独立した手段でリンクを作る。
//! 本体の実装をフィクスチャに流用すると、本体の不具合でフィクスチャも同じように壊れ、
//! 「リンクを検出して拒否する」契約の検証が成り立たなくなるため、ここでは OS の標準手段を直接使う。

use std::path::Path;

/// `link` に `target` を指すディレクトリリンクを作る。
///
/// - unix: シンボリックリンク。
/// - Windows: `mklink /J` によるジャンクション（管理者権限なしで作れるため）。
///   `cmd.exe` は `\\?\` 付きの拡張パスを受け付けないため除去し、区切りを `\` に揃える。
///   `CREATE_NO_WINDOW`（0x08000000）でテスト中にコンソールウィンドウを開かない。
pub fn link_directory(target: &Path, link: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let status = std::process::Command::new("cmd.exe")
            .args(["/C", "mklink", "/J"])
            .arg(
                link.to_string_lossy()
                    .trim_start_matches(r"\\?\")
                    .replace('/', "\\"),
            )
            .arg(
                target
                    .to_string_lossy()
                    .trim_start_matches(r"\\?\")
                    .replace('/', "\\"),
            )
            .creation_flags(0x08000000)
            .output()
            .unwrap();
        assert!(
            status.status.success(),
            "{}",
            String::from_utf8_lossy(&status.stderr)
        );
    }
}
