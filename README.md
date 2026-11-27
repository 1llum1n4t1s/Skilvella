# Skilvella

[日本語](README.ja.md) | **English** | [简体中文](README.zh-CN.md) | [繁體中文](README.zh-TW.md) | [한국어](README.ko.md) | [Español](README.es.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Português (Brasil)](README.pt-BR.md)

A local app that organizes `SKILL.md` files in your browser and loads only the skills you need through MCP, with just three MCP tools.

Search, edit, tag, publish, and restore skills, with conflict and error checks. Windows, macOS, and Linux.

## Getting started

Run [SkilvellaApp-win-Setup.exe](https://skilvella.kagayoi.com/SkilvellaApp-win-Setup.exe). Or get [Windows x64](https://skilvella.kagayoi.com/portable/Skilvella-windows-x86_64.zip) or [Mac Apple Silicon](https://skilvella.kagayoi.com/portable/Skilvella-macos-aarch64.tar.gz). Open the manager at [http://127.0.0.1:7465/](http://127.0.0.1:7465/). See [Installation](docs/readme/installation.md); for Linux or source builds, the [development guide](docs/DEVELOPMENT.md).

## Managing your library

Add folders with "+ Add folder"; files are never moved. See [Managing your library](docs/readme/library-management.md), [folder registration](docs/readme/library-folder-registration.md), [skill discovery](docs/readme/library-skill-discovery.md), [publishing to MCP](docs/readme/library-skill-publication.md), [usage modes and bulk changes](docs/readme/usage-modes-and-bulk.md).

<a id="app-updates"></a>

## App updates

Setup updates automatically: [App updates](docs/readme/app-updates.md), [Portable updates](docs/readme/portable-updates.md), [Setup migration](docs/readme/setup-migration.md).

<a id="mcp-connection"></a>

## MCP connection

Register `http://127.0.0.1:7465/mcp` with a Streamable HTTP client (read-only). See [MCP connection](docs/readme/mcp-connection.md).

## Automatic selection and command suggestions

Sync invocation entries to six clients from "Settings": [overview](docs/readme/invocation-entries.md), [per-client entries](docs/readme/client-entry-sync.md), [cleanup and errors](docs/readme/entry-sync-maintenance.md), [Claude Code frontmatter carry-over](docs/readme/claude-frontmatter-carryover.md), [body embedding for `context: fork`](docs/readme/fork-body-embedding.md).

<a id="display-settings"></a>

## Display settings

Display names, nine languages, and theme. See [Display settings](docs/readme/display-settings.md).

## Data and backups

Locations and backup steps: [Data and backups](docs/readme/data-and-backups.md).

## Options and troubleshooting

Run `skilvella --help`. See [Options and troubleshooting](docs/readme/troubleshooting.md) and [Tray and service](docs/readme/tray-and-service.md).

The license is [MIT](LICENSE). To contribute, see the [development guide](docs/DEVELOPMENT.md), the [design](DESIGN.md), and the [contributor guidelines](AGENTS.md).
