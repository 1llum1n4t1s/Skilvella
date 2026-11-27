# Installation and launching

Part of the [Skilvella README](../../README.md).

On Windows, run [SkilvellaApp-win-Setup.exe](https://skilvella.kagayoi.com/SkilvellaApp-win-Setup.exe) and launch "Skilvella" from the Start menu. The installer (Setup) version registers itself at install time to start when the current user signs in, and it updates automatically ([App updates](app-updates.md)). For switching from the ZIP version, autostart, and uninstalling, see [Installer version: switching and autostart](setup-migration.md).

To use the ZIP version or the macOS version, download the matching package from [Windows x64](https://skilvella.kagayoi.com/portable/Skilvella-windows-x86_64.zip) or [Mac Apple Silicon](https://skilvella.kagayoi.com/portable/Skilvella-macos-aarch64.tar.gz) (hosted at skilvella.kagayoi.com) and extract it. ZIP and macOS builds up to 1.1.8 checked GitHub for updates, which is no longer used, so if you are on one of them, download the new package manually once.

- Windows x64: Extract `Skilvella-windows-x86_64.zip` to the folder you will keep using, then launch `skilvella.exe`. It runs in the task tray without opening a terminal, and the first normal launch adds "Skilvella" to the current user's Start menu.
- Mac Apple Silicon: Extract `Skilvella-macos-aarch64.tar.gz` and run `./skilvella` in a terminal.
- Before extracting, you can verify the package's SHA-256 against [`SHA256SUMS.txt`](https://skilvella.kagayoi.com/portable/SHA256SUMS.txt).

For Linux or building from source, see the [development guide](../DEVELOPMENT.md).

For the tray menu, "Restart service", the direct URL (`http://127.0.0.1:7465/`), and `--no-tray`, see [Tray icon, service restart, and direct access](tray-and-service.md).
