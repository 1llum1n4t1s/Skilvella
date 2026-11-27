# Entry sync cleanup and errors

Part of [Automatic selection and command suggestions](invocation-entries.md).

- When you unpublish a skill or change its mode, artifacts no longer needed are moved outside the discovery paths (`skilvella-staging` / `skilvella-retired` directly under `~/.agents`, `~/.claude`, `~/.grok`, `~/.cursor`, `~/.copilot`, and `<Devin config>`).
- If unknown files or manual edits are detected, they are not overwritten, and a sync error is shown on the settings page.
- If incomplete temporary files remain because of insufficient disk space or a forced exit, sync stops while protecting the existing healthy index. Check the contents of the displayed path, then retry.
- Disabling the integration removes the entries it owns from the discovery paths. While sync is stopped, earlier suggestions and automatic selection settings may remain. Original files, publication states, and edit history are not changed.
- If Codex's suggestion display does not update, reopen the suggestion menu or restart Codex.
- Claude Code reflects changes to `~/.claude/skills` even in a running session. If you created `~/.claude/skills` itself after the session started, run `/reload-skills`.
- Content already loaded into an existing conversation cannot be removed.
