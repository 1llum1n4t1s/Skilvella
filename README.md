# Skilvella

[日本語](README.ja.md) | **English** | [简体中文](README.zh-CN.md) | [繁體中文](README.zh-TW.md) | [한국어](README.ko.md) | [Español](README.es.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Português (Brasil)](README.pt-BR.md)

A local app that organizes large collections of `SKILL.md` files in your browser and loads only the skills you need through MCP. No matter how many skills you have, it exposes just three MCP tools.

- Keyword search across names, descriptions, content, and tags, including Japanese substring matching.
- Folder registration, skill creation, content editing, tags and categories, bulk operations, and per-skill MCP publication switching.
- History saved before every edit with restore, conflict detection against external edits, and checks for duplicate names, syntax errors, and missing files.
- Windows / macOS / Linux. Use the browser on the same PC.

## Getting started

On Windows, run [SkilvellaApp-win-Setup.exe](https://skilvella.kagayoi.com/SkilvellaApp-win-Setup.exe) and launch "Skilvella" from the Start menu. The installer (Setup) version registers itself at install time to start when the current user signs in. When switching from an existing standalone (ZIP) version, first quit the old version from its task tray icon (if the old version is still running, the default port (7465) is unavailable, and the installer (Setup) version shows a message to that effect and does not start). The ZIP version's sign-in autostart registration is switched over to the installer (Setup) version automatically at install time, and the Start menu "Skilvella" shortcut then points to the installer (Setup) version as well. If you had disabled autostart, it stays disabled. Uninstalling also removes the autostart registration that points to the installer (Setup) version. The old version's folder is not deleted automatically, so remove it by hand if you no longer need it. If you are using an old installer (Setup) version without an auto-update mechanism (1.1.4 to 1.1.6), reinstall with the new Setup once. The standard data folder `%LOCALAPPDATA%\Skilvella` is shared and remains after uninstalling. The installer version updates automatically ([App updates](#app-updates)).

To use the ZIP version or the macOS version, download the matching package from [Windows x64](https://skilvella.kagayoi.com/portable/Skilvella-windows-x86_64.zip) or [Mac Apple Silicon](https://skilvella.kagayoi.com/portable/Skilvella-macos-aarch64.tar.gz) (hosted at skilvella.kagayoi.com) and extract it. ZIP and macOS builds up to 1.1.8 checked GitHub for updates, which is no longer used, so if you are on one of them, download the new package manually once.

- Windows x64: Extract `Skilvella-windows-x86_64.zip` to the folder you will keep using, then launch `skilvella.exe`. It runs in the task tray without opening a terminal, and the first normal launch adds "Skilvella" to the current user's Start menu.
- Mac Apple Silicon: Extract `Skilvella-macos-aarch64.tar.gz` and run `./skilvella` in a terminal.
- Before extracting, you can verify the package's SHA-256 against [`SHA256SUMS.txt`](https://skilvella.kagayoi.com/portable/SHA256SUMS.txt).

For Linux or building from source, see the [development guide](docs/DEVELOPMENT.md).

Skilvella shows its icon in the task tray on Windows and in the menu bar on Mac. Left-click it to open the manager in your default browser. Right-click (or Control-click on Mac) to choose "Open manager", "Check for updates", "Restart service", or "Quit". On Windows, if you cannot see the icon, also check the tray's hidden icons overflow.

"Restart service" restarts the HTTP/MCP server and file watching in the same process and at the same URL. Saved skills and settings are kept. Duplicate requests during a restart are coalesced, and the restart waits for in-flight HTTP operations to finish. MCP connections are dropped, so reconnect from your client. Reload any open manager page before using it. "Quit" shuts down the server and the icon.

You can also open the manager directly at [http://127.0.0.1:7465/](http://127.0.0.1:7465/). The MCP endpoint is `http://127.0.0.1:7465/mcp`. In an environment without a display, pass `--no-tray`. On Linux it shows no icon and runs as an HTTP/MCP server.

For the Windows standalone layout, launching without arguments attempts to register the app to start when the current user signs in. If it is already registered, launching the executable from a different location does not change the registered path. You can turn autostart on or off in Windows "Settings" → "Apps" → "Startup". To skip registration for this launch, pass `--no-autostart`. For autostart under Toolport management, configure it on the Toolport side.

No Rust, Node.js, Python, or external database server is needed at runtime. When launched from a terminal, you can also quit with `Ctrl+C` in the attached console.

## Managing your library

On first launch, Skilvella detects the managed folder automatically. It checks the existing `~/.agents/skills`, `$CODEX_HOME/skills`, and `~/.codex/skills` in that order, and if none is found, creates `<data-folder-name>-skills` next to the data folder. A candidate that contains only Codex invocation entries generated by Skilvella is skipped, and the entries themselves are not counted as managed skills. This holds only when the data folder has an ownership record for those entries. If you recreate the data folder or point to a different one with `--data-dir`, previously generated entries that remain will be registered as ordinary skills. If sources were registered before, that configuration takes priority and is carried over. If a problem reading the entry ownership record or the candidates prevents automatic detection from being finalized, the saved automatic registration target is kept. Check the detection status in the folder list on the left side of the library.

The library is a single screen: the list of managed folders on the left and the list of skills on the right. Press "+ Add folder" to open the OS's standard folder picker; the chosen folder is registered as is and scanned. Files are not moved. You do not need to enter a name, and the folder name is used as the display name (you can change it later with "Change display name"). In environments where the dialog cannot be opened (such as SSH sessions), enter an absolute path under "Enter a path instead".

Selecting a folder on the left shows its location, count, and standard-discovery-path warning, along with "Change display name" and "Stop managing". For the default folder, "Change referenced folder" lets you switch the referenced folder, and "Browse…" lets you pick one from the dialog. The setting persists across restarts, and "Stop managing" does not delete files. If you choose a folder that is already managed, or one that has a parent/child relationship with a managed folder, the addition is stopped and the overlapping folder's name is shown. If a folder is moved or deleted outside Skilvella, a warning appears in the sidebar and on the folder's page, and sync stops until it is resolved. If you moved it, open "Change folder location" and enter the new location (or choose one with "Browse…"). Only the registration is updated; files are not moved, and the skill IDs, publication state, usage settings, and history are kept. If you no longer need the folder, choose "Stop managing". If the new location is already managed as another folder, stop managing that folder first.

New skills found through adding, automatic detection, or external edits are registered with **MCP publication on** if they can be read. Skills with read errors or invalid frontmatter stay unpublished, and they are not published automatically even after you fix them. The publication state of existing skills does not change on rescan or at startup (skills you unpublished are never published automatically). On Windows and macOS, a folder whose name differs only in ASCII letter case is treated as the same skill, and its publication state does not change. If you do not want a skill published, choose "Unpublished" in the list's "Status" column after adding the folder. Choosing "Published" or "Unpublished" in the list's "Status" column saves immediately. You can also switch it on the individual page or with the list's bulk operations. When switching to published, the files are checked again, and skills that are missing, have read errors, or have invalid frontmatter are not published; the selection is reverted and the reason is shown. Tags and categories do not change.

The library lists the skills of all folders from the start and lets you search them in place. A folder that has a `.git` directory or file directly inside it is shown nested under the folder as a source (repository), and everything else is grouped under "Personal skills". If a folder with no source repository directly inside it is itself a Git repository, the whole folder becomes one source. The normal list shows the skills that currently exist. Old registrations, such as those from before a move, are kept and can be reviewed under the status "Missing" or "All" and under "Clean up registrations for missing skills" on the left. In the "Missing" list, "Delete all missing registrations" deletes the registration data, settings, and history across all pages matching the search conditions. This cannot be undone. It does not change actual files, and it aborts the deletion of all entries if the source cannot be reached or if a path exists. Categories can be edited from each skill's metadata.

Skilvella searches the `SKILL.md` files in registered folders **recursively with no depth limit**. It keeps scanning deeper subdirectories even after finding a skill. Parent and child skills are managed with separate IDs and publication settings, and a child's files are not included in the parent's MCP publication scope. Symbolic links, Windows junctions, `.git`, `node_modules`, `target`, and `.work` are excluded. Unregistering a folder does not delete the original files. If you add back a folder you had removed from management, its earlier management data (publication settings, tags, history) is restored. Folders with the same name do not conflict with each other.

Saving content changes the actual file in the registered location. The preview disables HTML, image loading, and link actions, and does not execute scripts. Changes made in external editors are watched automatically. If they are not reflected, use "Rescan".

When there are read errors, open the cross-folder error list from the library's "Review and fix read errors" or from "Read errors" on the left. Open the skill name or path to check the cause, and for syntax errors, fix the content and save. If a file cannot be read, check its location, permissions, and character encoding, then rescan. The notice disappears once the error is resolved.

<a id="app-updates"></a>

## App updates

**Installer (Setup) version**: Automatic updates are on by default. It checks for a new version at startup and every 24 hours while it keeps running, and downloads it if one is available. The update is applied automatically at the next startup, relaunching with the same settings. It does not connect within 24 hours of the previous check. If a check or download fails, it keeps running the current version. You can turn this off with "Update automatically at startup" under "App updates" in "Settings" (the setting is saved in `app-update.json` in the data folder, such as `%LOCALAPPDATA%\Skilvella`, and does not change the skill library). "Check for updates" checks regardless of the 24-hour interval, and "Install update" downloads the update and restarts immediately. Update information is fetched from `skilvella.kagayoi.com`.

The following applies to the standalone (ZIP) version and to Mac.

You can check for the latest stable release at skilvella.kagayoi.com from the update section in "Settings" or from the tray menu's "Check for updates". Choosing "Install update" downloads the package for Windows x64 / Mac Apple Silicon and `SHA256SUMS.txt` over HTTPS, verifies the SHA-256, and then applies it. This requires the OS-bundled `curl` and write permission to the executable's location. In-app updating is not supported on Linux or other CPUs.

The update waits for in-flight HTTP operations to finish before exiting, then restarts with the same launch arguments and working folder. Reconnect your MCP clients. If package verification fails, the current version is kept. If the new process cannot be started after applying the update, it tries to restore the old executable or the Toolport reference target. The settings page that started the update reloads automatically once the new version responds. If you perform an update operation on an old settings page that was already open before the app restarted, you are asked to reload the page.

A `.skilvella-update-*` folder is created in the same location as the pre-update executable, and it keeps `update.log`. In the normal layout, it also stores `previous-binary` for recovery. Skilvella removes this folder automatically at startup once it is more than 24 hours old, and only when no update is in progress and the folder contains nothing but files the updater created. You do not need to delete it yourself; if you do, wait until the update helper has exited.

Under Toolport's `Skilvella-Latest.ps1` management, the existing version is kept, the new version's folder and verification information are added, and the launcher's reference target is switched. The update helper runs until the restarted app exits, so quit Skilvella before deleting the update folder.

When Skilvella is launched by an external process monitor, you may need to restart it from that tool. If the Toolport startup lock cannot be acquired, the update is not applied. If it does not start, rerun the launcher. A Toolport-managed location that contains non-ASCII characters, such as Japanese, does not support in-app updating. The normal standalone layout is not subject to this limitation.

<a id="mcp-connection"></a>

## MCP connection

Register the following URL with a client that supports Streamable HTTP.

```text
http://127.0.0.1:7465/mcp
```

In Toolport, register it as an HTTP server and specify the same URL. Keep Skilvella running.

| Tool | Purpose |
| --- | --- |
| `search_skills` | Search published skills and get summaries and IDs |
| `load_skill` | Get the `SKILL.md` content and the list of reference files by specifying an ID |
| `read_skill_file` | Get UTF-8 text under a skill, by line |

For `load_skill`, keep passing the continuation arguments until `next` is null, concatenate the content, and then follow the instructions. The list is normally 100 entries per page and up to 8 KiB of total paths, but if a single path exceeds 8 KiB, that path is returned alone, so the response becomes large. Fetch skills and the references they need one at a time. For references, use `read_skill_file` to fetch `next_text_offset` for the needed line range to the end, and use `next_start_line` only when more lines are needed. Byte pages of the same line range are concatenated as is, and line breaks are inserted between line ranges when concatenating. When continuing with `next_text_offset`, keep `start_line` and `max_lines`; for the next line range, set `start_line` to `next_start_line` and reset `text_offset` to 0. Line breaks are normalized as before, and the trailing line break is removed. For continuations, specify the returned `revision` as `expected_revision`. If the file changed in the meantime, fetch it again from the start. `id` is the skill id returned by `search_skills`; a unique exact published skill name is also accepted, and an ambiguous name returns candidate ids. The first `read_skill_file` call needs only `id` and `path`; when the result is truncated, call again with the `next` object, which already carries `expected_revision`.

MCP is read-only. It cannot return the content of unpublished skills, skills with invalid frontmatter, or missing skills. There is also no feature to execute reference files. Content is limited to 2 MiB and reference text to 1 MiB.

MCP publication alone does not change the fact that agents standardly discover and load the same skills. To avoid double loading, place skills outside the agent's standard discovery paths and register that location with the library's "+ Add folder". Skilvella never moves files. If existing files need to be moved, do it yourself and then give the new location with the folder's "Change folder location". Codex's configuration files are not edited. Restart Codex after changing the standard discovery paths. Content already loaded into a conversation is not removed. Check custom discovery settings and other clients separately.

## Automatic selection and command suggestions

First register and verify Skilvella with a client that uses [MCP connection](#mcp-connection). In "Settings", under "Invocation entry sync", check the sync targets (Codex, Claude Code, Grok, Cursor, GitHub Copilot, Devin; multiple can be selected) and save, and Skilvella itself automatically syncs the invocation entries for each client. Clients whose settings folder is not found on this PC (such as `~/.cursor`) are collapsed under "Other clients" in the same form; clients that are enabled or have a sync error are always shown. The status table lists only enabled clients and clients with a sync error. If you had enabled "Codex integration" in an older version, it is carried over with Codex selected. This works the same on Windows, macOS, and Linux and uses neither Task Scheduler nor any separate resident process. Sync runs at startup, after management operations, and during periodic checks, and the setting is kept after a service restart.

| Sync target | Entry location | How it is exposed to discovery | Automatic selection control |
| --- | --- | --- | --- |
| Codex | `~/.agents/skills/skilvella/<name>` | Real files (Codex searches nested folders) | `agents/openai.yaml` |
| Claude Code | `~/.claude/skilvella/skills/<name>` | A link to `~/.claude/skills/<name>` (a junction on Windows, a symbolic link on macOS and Linux) | `disable-model-invocation` |
| Grok | `~/.grok/skills/skilvella/<name>` | Real files (Grok searches nested folders) | `disable-model-invocation` |
| Cursor | `~/.cursor/skills/skilvella/<name>` | Real files (Cursor searches nested folders but may not follow links) | `disable-model-invocation` |
| GitHub Copilot | `~/.copilot/skilvella/skills/<name>` | A link to `~/.copilot/skills/<name>` | `disable-model-invocation` |
| Devin | `<Devin config>/skilvella/skills/<name>` | A link to `<Devin config>/skills/<name>` | `triggers: [user]` |

`<Devin config>` is `%APPDATA%\devin` on Windows and `~/.config/devin` on macOS and Linux (`XDG_CONFIG_HOME` is not used). GitHub Copilot always uses `~/.copilot`; `COPILOT_HOME` is not followed.

Claude Code searches only one level directly under `~/.claude/skills`, so entries are placed outside the discovery path and a link is created for each skill. If an existing skill or link with the same name is already in `~/.claude/skills`, it is not overwritten, and the name is shown as a sync error. A link that you created by hand from `~/.claude/skills/<name>` to `~/.agents/skills/skilvella/<name>` following an old version's guidance is re-pointed to the Claude Code entry and taken over for management, but only for names that are sync targets. Links for names that are not sync targets are left untouched (if you disable Codex, their reference target disappears). Disabling sync for Claude Code removes only the links that Skilvella created or took over. If you have registered `~/.claude/skills` itself as a managed folder, sync is not performed.

GitHub Copilot (the Copilot CLI finds only `~/.copilot/skills/<name>/SKILL.md`) and Devin use the same link method and the same rules: existing entries with the same name are not overwritten, only links that Skilvella created are removed, and the link folder must not be a managed folder. Links to Codex entries made by hand are taken over only for Claude Code. Whether the Copilot CLI honors `disable-model-invocation` and how deep Devin searches have not been confirmed.

Cursor reads `~/.agents/skills` and `~/.claude/skills`, GitHub Copilot reads `~/.agents/skills`, and Devin reads `~/.agents/skills` and `~/.copilot/skills` as well, so the same entry may appear more than once when several clients are enabled.

Grok also reads the discovery paths of Codex (`~/.agents/skills`) and Claude Code (`~/.claude/skills`), and for skills with the same name it gives priority in the order `~/.grok/skills`, `~/.agents/skills`, `~/.claude/skills`. Codex entries stop automatic selection with `agents/openai.yaml`, so in Grok, even "Command suggestions only" skills are selected automatically. If you use Grok, select Grok as a sync target.

Choose the mode on each skill's detail page or with "Usage" in the list. In the list you can check the current mode and switch it in place. MCP does not return the content of unpublished skills, and after a successful sync it also removes their suggestion entries.

If you press the browser's Back button or reload while a save is in progress in the list, Skilvella checks the save result before showing the latest settings. A request that was not accepted because the connection was interrupted is canceled, and the check result is shown on screen.

| Mode | Name and description passed for automatic selection | Command suggestions |
| --- | --- | --- |
| Search only (default) | Not passed | Not shown |
| Command suggestions only | Not passed | Shown |
| Automatic selection + command suggestions | Passed | Shown |

For skills that show command suggestions, the invocation command for each sync target (`$name` for Codex, `/name` for the other clients) is shown under the skill name in the list and under "Usage" on the detail page. If the name contains anything other than lowercase letters, digits, and `-`, it becomes `skill-<first 8 characters of the id>`, and if a published skill with the same name exists, it becomes `<name>-<first 8 characters of the id>`, so use the command that is shown. When there is no command, the detail page shows the reason (search only, no sync target selected, unpublished, or read error). The command for a skill you switch from unpublished to published in the list appears after a reload.

In the list, check skills, choose "Usage" under the bulk operations, and press "Apply to selected" to change the mode of multiple skills at once. "No change" keeps the current mode. You can change the publication state and tags at the same time.

Opening "Bulk settings by source" under "All skills" lets you select multiple sources and change the publication state and usage together with "Apply to selected repositories". The scope is the skills on all pages registered at run time and does not carry over to skills added later. The pre-check count is based on the scan index and excludes missing and error entries. The result shows the number updated and the number excluded. When publishing, the files are checked again, and if they cannot be re-validated because they are missing, unreadable, invalid, or similar, saving the settings for all entries is canceled. The settings are saved in a single transaction, and the original skills are not changed.

Codex has no separate setting for "enable automatic selection but hide it from suggestions", so Skilvella provides the three corresponding modes. Even with automatic selection, it does not keep a full copy of the original content at all times; the content is fetched from MCP when needed. Skills not shown in suggestions can be searched with `$skilvella-search`, which is for explicit invocation only. You can also use MCP's `search_skills` directly.

The usage mode is common to all sync targets. The entry locations are as in the table above. When you unpublish a skill or change its mode, artifacts that are no longer needed are moved outside the discovery paths (`skilvella-staging` / `skilvella-retired` directly under `~/.agents`, `~/.claude`, `~/.grok`, `~/.cursor`, `~/.copilot`, and `<Devin config>`). If unknown files or manual edits are detected, they are not overwritten, and a sync error is shown on the settings page. If incomplete temporary files remain because of insufficient disk space or a forced exit, it stops while protecting the existing healthy index. Check the contents of the displayed path and then retry. Disabling the integration removes the entries it owns from the discovery paths. While sync is stopped, earlier suggestions and automatic selection settings may remain. Original files, publication states, and edit history are not changed. If Codex's suggestion display does not update, reopen the suggestion menu or restart Codex. Claude Code reflects changes to `~/.claude/skills` even in a running session, but if you created `~/.claude/skills` itself after the session started, run `/reload-skills`. Content already loaded into an existing conversation cannot be removed.

<a id="display-settings"></a>

## Display settings

In the library, select a folder on the left and open "Change display name" to change the folder name shown on screen. The input must be within 100 characters including leading and trailing whitespace, and control characters such as line breaks are not allowed. Leading and trailing whitespace is trimmed on save, and an empty value reverts to the original identifier. The display name is saved in the app, and the actual folder name, skill IDs, publication settings, and edit history do not change. Duplicate display names are allowed, but a name you have changed is shown with its original identifier alongside it.

In "Settings" of the manager, you can choose the language as automatic or one of nine languages (Japanese, English, Simplified Chinese, Traditional Chinese, Korean, Spanish, French, German, Brazilian Portuguese). The default "Automatic" uses the browser's language setting. The language is saved when you select it, and if JavaScript is disabled, press "Save". The selection is saved per connection port in the same browser, and other tabs that are already open apply it from the next page they load. A language setting from an older version is carried over when you open the manager at the same address in the same browser. Carrying it over requires JavaScript. Skill names, descriptions, content, paths, and technical error details keep their original text. The tray and menu bar menus, command-line output such as `--help`, startup messages, and errors, and the installer (Setup) version's message boxes are also shown in the same nine languages. These languages are determined in the order of the environment variable `SKILVELLA_LANGUAGE` (for example `ja`, `pt-BR`) → `LC_ALL` → `LC_MESSAGES` → `LANG` → the OS display language, and if it cannot be determined, English is used. MCP tool names and descriptions are in English.

You can choose the theme from Automatic, Light, and Dark when JavaScript is enabled. The selection is saved in the browser, and Automatic follows the browser's color scheme setting.

## Data and backups

| OS | Default data location |
| --- | --- |
| Windows | `%LOCALAPPDATA%\Skilvella` |
| macOS | `~/Library/Application Support/Skilvella` |
| Linux | `$XDG_DATA_HOME/skilvella` or `~/.local/share/skilvella` |

The source of truth for `SKILL.md` and reference files is the registered folders. In addition to the search index, the data folder's `library.sqlite3` also stores tags, categories, publication settings, registered locations, folder display names, and edit history. **If you delete the entire DB, this management data is lost.**

To back up, quit Skilvella first and then copy both the data folder and the registered folders. The DB alone cannot restore the current skill files. On another PC the absolute paths of the registered sources differ, so sharing the DB as is for simultaneous use is not supported.

## Options and troubleshooting

```sh
skilvella --data-dir /absolute/path/to/data --port 7465 --no-watch
skilvella --help
```

- If the port is in use, quit the existing process or change it with `--port`. The manager and MCP both use the same port.
- If the manager does not come back after updating from an old Windows version, launch the updated `skilvella.exe` once in the normal layout, or the launcher once under Toolport management.
- If an interrupted folder move from an old version cannot be recovered at startup, startup is aborted. The original data and the migration record are kept.
- The data location and the skill registration locations cannot overlap. Specify separate folders on a local disk.
- Edits made in another tab are also detected as conflicts on save. Copy the retained draft, reload, and merge it into the latest content.
- Search uses whitespace-separated AND and substring matching. Searching English skills by meaning from a Japanese query, embedding models, and cloud sync are not included.
- Even if the source folder goes offline, the registration data and history are kept. Rescan after it reconnects.

Connections are limited to `127.0.0.1`. It is intended for use where you trust the users and processes on the same PC.

The license is [MIT](LICENSE).

If you want to contribute to development, see the [development guide](docs/DEVELOPMENT.md), the [design](DESIGN.md), and the [contributor guidelines](AGENTS.md).
