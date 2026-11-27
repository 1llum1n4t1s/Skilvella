# Skilvella

[日本語](README.ja.md) | **English** | [简体中文](README.zh-CN.md) | [繁體中文](README.zh-TW.md) | [한국어](README.ko.md) | [Español](README.es.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Português (Brasil)](README.pt-BR.md)

A local app that organizes large collections of `SKILL.md` files in your browser and loads only the skills you need through MCP. No matter how many skills you have, it exposes just three MCP tools.

- Keyword search across names, descriptions, content, and tags, including Japanese substring matching.
- Folder registration, skill creation, content editing, tags and categories, bulk operations, and per-skill MCP publication switching.
- History saved before every edit with restore, conflict detection against external edits, and checks for duplicate names, syntax errors, and missing files.
- Windows / macOS / Linux. Use the browser on the same PC.

## Getting started

On Windows, run [SkilvellaApp-win-Setup.exe](https://skilvella.kagayoi.com/SkilvellaApp-win-Setup.exe). For the ZIP or macOS version, get [Windows x64](https://skilvella.kagayoi.com/portable/Skilvella-windows-x86_64.zip) or [Mac Apple Silicon](https://skilvella.kagayoi.com/portable/Skilvella-macos-aarch64.tar.gz). Open the manager at [http://127.0.0.1:7465/](http://127.0.0.1:7465/). See [Installation](docs/readme/installation.md); for Linux or source builds, the [development guide](docs/DEVELOPMENT.md).

## Managing your library

Add folders with "+ Add folder"; files are never moved. See [Managing your library](docs/readme/library-management.md). Details: [folder registration](docs/readme/library-folder-registration.md), [skill discovery](docs/readme/library-skill-discovery.md), [publishing to MCP](docs/readme/library-skill-publication.md), [usage modes and bulk changes](docs/readme/usage-modes-and-bulk.md).

<a id="app-updates"></a>

## App updates

The Setup version updates automatically. See [App updates](docs/readme/app-updates.md). For ZIP, Mac, and Toolport-managed versions, see [Portable updates](docs/readme/portable-updates.md); for switching to Setup and autostart, [Setup migration](docs/readme/setup-migration.md).

<a id="mcp-connection"></a>

## MCP connection

Register `http://127.0.0.1:7465/mcp` with a Streamable HTTP client (read-only). See [MCP connection](docs/readme/mcp-connection.md).

## Automatic selection and command suggestions

Sync invocation entries to six clients from "Settings". See [Automatic selection and command suggestions](docs/readme/invocation-entries.md). Details: [per-client entries](docs/readme/client-entry-sync.md), [cleanup and errors](docs/readme/entry-sync-maintenance.md), [Claude Code frontmatter carry-over](docs/readme/claude-frontmatter-carryover.md).

<a id="display-settings"></a>

## Display settings

Display names, nine languages, and theme. See [Display settings](docs/readme/display-settings.md).

## Data and backups

Locations and backup steps: [Data and backups](docs/readme/data-and-backups.md).

## Options and troubleshooting

Run `skilvella --help`. See [Options and troubleshooting](docs/readme/troubleshooting.md). For the tray icon and service restart, see [Tray and service](docs/readme/tray-and-service.md).

The license is [MIT](LICENSE). To contribute, see the [development guide](docs/DEVELOPMENT.md), the [design](DESIGN.md), and the [contributor guidelines](AGENTS.md).
