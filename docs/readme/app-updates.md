# App updates

Part of the [Skilvella README](../../README.md).

**Installer (Setup) version**: Automatic updates are on by default. It checks for a new version at startup and every 24 hours while it keeps running, and downloads it if one is available. The update is applied automatically at the next startup, relaunching with the same settings. It does not connect within 24 hours of the previous check. If a check or download fails, it keeps running the current version. You can turn this off with "Update automatically at startup" under "App updates" in "Settings" (the setting is saved in `app-update.json` in the data folder, such as `%LOCALAPPDATA%\Skilvella`, and does not change the skill library). "Check for updates" checks regardless of the 24-hour interval, and "Install update" downloads the update and restarts immediately. Update information is fetched from `skilvella.kagayoi.com`.

For the standalone (ZIP) version, Mac, and Toolport-managed launches, including verification, recovery, and the `.skilvella-update-*` folder, see [Updates for the standalone, Mac, and Toolport-managed versions](portable-updates.md).
