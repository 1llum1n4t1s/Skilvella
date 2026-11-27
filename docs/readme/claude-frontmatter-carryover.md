# Claude Code frontmatter carry-over

Part of [Invocation entries per client](client-entry-sync.md).

Only the Claude Code entry carries over some frontmatter keys of the original `SKILL.md`. A skill that sets `context: fork` (and `agent`, such as `agent: Explore`) can therefore still run as a forked subagent when called through the entry.

| Carried over | Condition |
| --- | --- |
| `context` | Only `fork` |
| `agent`, `background` | Only together with `context: fork` |
| `model` | None |
| `effort` | `low`, `medium`, `high`, `xhigh`, `max` |
| `argument-hint`, `paths` | None |

- Values are type-checked, and only an invalid key is dropped.
- Keys that change permissions or the whole session (`allowed-tools`, `disallowed-tools`, `hooks`) are not carried over. Neither are `shell`, `when_to_use`, `license`, and similar keys. `arguments` (a space-separated string or an array of strings) is carried over only for an entry that embeds the body (see below).
- `disable-model-invocation` and `user-invocable` follow the "Usage" setting in Skilvella.
- Other clients' entries are unchanged.
- If a call has arguments, the entry tells the agent to use them as input for the loaded skill.

## Body embedding for `context: fork`

- For an entry whose original has `context: fork`, the entry embeds the original `SKILL.md` body as is, instead of the "read the body with `load_skill`" pointer. The body of a forked skill reaches only the forked agent, so saving the parent's context is no reason to use a pointer, and forked agents have ignored the pointer. A short note before the body says not to re-read it with `load_skill` and to use `read_skill_file` for referenced files (relative links, `references`, `scripts`), which are not in the entry folder.
- It is not embedded, and the entry stays the pointer, when the body contains a dynamic command with `!` (inline `` !`...` `` after a line start, space or symbol, or ```! anywhere in the body, not only at a line start, including inside quotes and lists; `~~~!` and a space between the fence characters and `!` also count, because Claude Code matches its block syntax anywhere in the body), because Skilvella does not run files or scripts and Claude Code would run it without a permission prompt. A `!` right after a letter or digit, as in `panic!`, does not count. It is also not embedded when the body contains `CLAUDE_SKILL_DIR` (it would point at the entry folder and give a wrong value), when the whole entry exceeds 2 MiB, or when the body is empty.
- Claude Code replaces `$ARGUMENTS`, `$0`, `$name`, and similar, as it does for native skills. An embedding entry also carries over `arguments`.
- `~/.claude/skills` is also read by Cursor and Grok, so the embedded body is visible to them too.
- Making a skill private or turning off command candidates removes the entry as before. The retired entry (`~/.claude/skilvella-retired/<uuid>`) keeps a copy with the body until the next sync; it is not a discovery location.
