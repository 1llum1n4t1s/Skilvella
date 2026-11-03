#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::run;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::run;

#[cfg(not(any(windows, target_os = "macos")))]
pub fn run(_: &str, _: crate::service::Control) -> anyhow::Result<()> {
    Ok(())
}
