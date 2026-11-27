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
- Keys that change permissions or the whole session (`allowed-tools`, `disallowed-tools`, `hooks`) are not carried over. Neither are `shell`, `arguments`, `when_to_use`, `license`, and similar keys.
- `disable-model-invocation` and `user-invocable` follow the "Usage" setting in Skilvella.
- Other clients' entries are unchanged.
- If a call has arguments, the entry tells the agent to use them as input for the loaded skill.
