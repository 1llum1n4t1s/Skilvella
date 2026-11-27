# Installer (Setup) version: switching and autostart

Part of the [installation guide](installation.md).

- Switching from a standalone (ZIP) version: quit the old version from its task tray icon first. While it runs, the default port (7465) is unavailable, so the Setup version shows a message and does not start.
- The ZIP version's sign-in autostart registration is switched to the Setup version at install time, and the Start menu "Skilvella" shortcut then points to the Setup version. If you had disabled autostart, it stays disabled.
- Uninstalling removes the autostart registration that points to the Setup version.
- The old version's folder is not deleted automatically. Remove it by hand if you no longer need it.
- If you use an old Setup version without an auto-update mechanism (1.1.4 to 1.1.6), reinstall with the new Setup once.
- The standard data folder `%LOCALAPPDATA%\Skilvella` is shared and remains after uninstalling.
- The Setup version updates automatically ([App updates](app-updates.md)).

## Autostart of the standalone layout

- Launching the Windows standalone layout without arguments attempts to register the app to start at sign-in. If it is already registered, launching the executable from another location does not change the registered path.
- Turn autostart on or off in Windows "Settings" → "Apps" → "Startup".
- To skip registration for one launch, pass `--no-autostart`.
- For autostart under Toolport management, configure it on the Toolport side.
