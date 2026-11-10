//! リリース候補の公開前に、公開済みの実行ファイルから実際に更新する。
#[cfg(not(any(
    all(windows, target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64")
)))]
fn main() -> anyhow::Result<()> {
    anyhow::bail!("Run on Windows x64 or Apple Silicon Mac");
}

#[cfg(any(
    all(windows, target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64")
))]
#[path = "support/release_update.rs"]
mod scenario;

#[cfg(any(
    all(windows, target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64")
))]
fn main() -> anyhow::Result<()> {
    scenario::run()
}
