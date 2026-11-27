# Invocation entries per client

Part of [Automatic selection and command suggestions](invocation-entries.md).

| Sync target | Entry location | How it is exposed to discovery | Automatic selection control |
| --- | --- | --- | --- |
| Codex | `~/.agents/skills/skilvella/<name>` | Real files (Codex searches nested folders) | `agents/openai.yaml` |
| Claude Code | `~/.claude/skilvella/skills/<name>` | A link to `~/.claude/skills/<name>` (junction on Windows, symlink elsewhere) | `disable-model-invocation` |
| Grok | `~/.grok/skills/skilvella/<name>` | Real files (Grok searches nested folders) | `disable-model-invocation` |
| Cursor | `~/.cursor/skills/skilvella/<name>` | Real files (Cursor searches nested folders but may not follow links) | `disable-model-invocation` |
| GitHub Copilot | `~/.copilot/skilvella/skills/<name>` | A link to `~/.copilot/skills/<name>` | `disable-model-invocation` |
| Devin | `<Devin config>/skilvella/skills/<name>` | A link to `<Devin config>/skills/<name>` | `triggers: [user]` |

`<Devin config>` is `%APPDATA%\devin` on Windows and `~/.config/devin` on macOS and Linux (`XDG_CONFIG_HOME` is not used). GitHub Copilot always uses `~/.copilot` (`COPILOT_HOME` is ignored).

## Claude Code

Claude Code searches only one level directly under `~/.claude/skills`, so entries sit elsewhere and a link is made per skill. An existing skill or link with the same name there is not overwritten and is shown as a sync error. A hand-made link to the Codex entry (old guidance) is re-pointed and taken over, only for names that are sync targets. Disabling sync removes only links Skilvella created or took over. If `~/.claude/skills` is itself a managed folder, no sync is done. For carried-over keys, see [frontmatter carry-over](claude-frontmatter-carryover.md).

## GitHub Copilot and Devin

Same rules as Claude Code links: same-name entries are not overwritten, only links Skilvella created are removed, and the link folder must not be a managed folder. The Copilot CLI finds only `~/.copilot/skills/<name>/SKILL.md`. Whether it honors `disable-model-invocation`, and how deep Devin searches, are unconfirmed.

## Overlapping discovery paths

- Cursor reads `~/.agents/skills` and `~/.claude/skills`; GitHub Copilot reads `~/.agents/skills`; Devin reads `~/.agents/skills` and `~/.copilot/skills`. The same entry may appear more than once.
- Grok also reads `~/.agents/skills` and `~/.claude/skills`, preferring `~/.grok/skills`, then `~/.agents/skills`, then `~/.claude/skills`. Codex entries stop automatic selection with `agents/openai.yaml`, so in Grok even "Command suggestions only" skills are selected automatically. Select Grok as a target if you use it.
