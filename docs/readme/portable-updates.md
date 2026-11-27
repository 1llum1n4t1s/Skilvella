# Updates for the standalone (ZIP), Mac, and Toolport-managed versions

Part of the [App updates](app-updates.md) guide.

You can check for the latest stable release at skilvella.kagayoi.com from the update section in "Settings" or from the tray menu's "Check for updates". Choosing "Install update" downloads the package for Windows x64 / Mac Apple Silicon and `SHA256SUMS.txt` over HTTPS, verifies the SHA-256, and then applies it. This requires the OS-bundled `curl` and write permission to the executable's location. In-app updating is not supported on Linux or other CPUs.

The update waits for in-flight HTTP operations to finish before exiting, then restarts with the same launch arguments and working folder. Reconnect your MCP clients. If package verification fails, the current version is kept. If the new process cannot be started, it tries to restore the old executable or the Toolport reference target. The settings page that started the update reloads automatically once the new version responds. If you operate an old settings page that was open before the restart, you are asked to reload it.

A `.skilvella-update-*` folder is created beside the pre-update executable and keeps `update.log`. In the normal layout it also stores `previous-binary` for recovery. Skilvella removes this folder automatically at startup once it is more than 24 hours old, only when no update is in progress and the folder contains nothing but files the updater created. You do not need to delete it; if you do, wait until the update helper has exited.

Under Toolport's `Skilvella-Latest.ps1` management, the existing version is kept, the new version's folder and verification information are added, and the launcher's reference target is switched. The update helper runs until the restarted app exits, so quit Skilvella before deleting the update folder.

When Skilvella is launched by an external process monitor, you may need to restart it from that tool. If the Toolport startup lock cannot be acquired, the update is not applied. If it does not start, rerun the launcher. A Toolport-managed location that contains non-ASCII characters, such as Japanese, does not support in-app updating. The normal standalone layout is not subject to this limitation.
