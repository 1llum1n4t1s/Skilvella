# Linked client entries

Part of [Invocation entries per client](client-entry-sync.md).

Claude Code, GitHub Copilot, and Devin search only one level directly under their skills folder, so Skilvella keeps the entries elsewhere and makes one link per skill (junction on Windows, symlink elsewhere).

- An existing skill or link with the same name is not overwritten and is shown as a sync error.
- For Claude Code, a hand-made link to the Codex entry (old guidance) is re-pointed and taken over, only for names that are sync targets.
- Disabling sync removes only links Skilvella created or took over.
- If the link folder (such as `~/.claude/skills`) is itself a managed folder, no sync is done.

The Copilot CLI finds only `~/.copilot/skills/<name>/SKILL.md`. Whether it honors `disable-model-invocation` or `user-invocable: false` is not documented (VS Code honors the latter), so with "Automatic selection only" `/name` may remain in the CLI. How deep Devin searches is unconfirmed.

For keys carried over to the Claude Code entry, see [frontmatter carry-over](claude-frontmatter-carryover.md).
