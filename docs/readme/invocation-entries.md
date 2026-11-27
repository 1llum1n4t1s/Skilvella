# Automatic selection and command suggestions

Part of the [Skilvella README](../../README.md).

First register and verify Skilvella with a client that uses [MCP connection](mcp-connection.md). In "Settings", under "Invocation entry sync", check the sync targets (Codex, Claude Code, Grok, Cursor, GitHub Copilot, Devin; multiple can be selected) and save, and Skilvella itself syncs the invocation entries for each client. Clients whose settings folder is not found on this PC (such as `~/.cursor`) are collapsed under "Other clients"; clients that are enabled or have a sync error are always shown, and the status table lists only those. If you had enabled "Codex integration" in an older version, it is carried over with Codex selected. This works the same on Windows, macOS, and Linux and uses neither Task Scheduler nor any separate resident process. Sync runs at startup, after management operations, and during periodic checks, and the setting is kept after a service restart.

- Entry locations, link methods, and overlapping discovery paths for each client: [Invocation entries per client](client-entry-sync.md).
- Frontmatter keys carried over to the Claude Code entry: [Claude Code frontmatter carry-over](claude-frontmatter-carryover.md).
- Usage modes (search only, command suggestions only, automatic selection only, automatic selection + command suggestions), invocation commands, and bulk changes: [Usage modes and bulk changes](usage-modes-and-bulk.md).
- Cleanup, sync errors, and reflecting changes in clients: [Entry sync cleanup and errors](entry-sync-maintenance.md).
