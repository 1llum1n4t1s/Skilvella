# Invocation entries per client

Part of [Automatic selection and command suggestions](invocation-entries.md).

| Sync target | Entry location | How it is exposed to discovery | Automatic selection control |
| --- | --- | --- | --- |
| Codex | `~/.agents/skills/skilvella/<name>` | Real files (Codex searches nested folders) | `agents/openai.yaml` |
| Claude Code | `~/.claude/skilvella/skills/<name>` | A link to `~/.claude/skills/<name>` (junction on Windows, symlink elsewhere) | `disable-model-invocation`, `user-invocable` |
| Grok | `~/.grok/skills/skilvella/<name>` | Real files (Grok searches nested folders) | `disable-model-invocation`, `user-invocable` |
| Cursor | `~/.cursor/skills/skilvella/<name>` | Real files (Cursor searches nested folders but may not follow links) | `disable-model-invocation` |
| GitHub Copilot | `~/.copilot/skilvella/skills/<name>` | A link to `~/.copilot/skills/<name>` | `disable-model-invocation`, `user-invocable` |
| Devin | `<Devin config>/skilvella/skills/<name>` | A link to `<Devin config>/skills/<name>` | `triggers: [user]` |

`user-invocable: false` is written only for "Automatic selection only" (the skill leaves the `/` menu but can still be selected automatically). Codex, Cursor, and Devin cannot hide the command, so they get the same entry as "Automatic selection + command suggestions" (Codex: `allow_implicit_invocation: true` in `agents/openai.yaml`).

`<Devin config>` is `%APPDATA%\devin` on Windows and `~/.config/devin` on macOS and Linux (`XDG_CONFIG_HOME` is not used). GitHub Copilot always uses `~/.copilot` (`COPILOT_HOME` is ignored).

## Linked entries

Claude Code, GitHub Copilot, and Devin get one link per skill; same-name entries are not overwritten and only links Skilvella created are removed. Details: [Linked client entries](client-link-entries.md).

## Overlapping discovery paths

- Cursor reads `~/.agents/skills` and `~/.claude/skills`; GitHub Copilot reads `~/.agents/skills`; Devin reads `~/.agents/skills` and `~/.copilot/skills`. The same entry may appear more than once.
- Grok also reads `~/.agents/skills` and `~/.claude/skills` (after `~/.grok/skills`). Codex entries stop automatic selection only via `agents/openai.yaml`, so Grok selects even "Command suggestions only" skills automatically; select Grok as a target if you use it.
- With Codex selected, Grok and GitHub Copilot in VS Code may show the command of an "Automatic selection only" skill from `~/.agents/skills`.
