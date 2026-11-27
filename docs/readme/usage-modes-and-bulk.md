# Usage modes and bulk changes

Part of [Automatic selection and command suggestions](invocation-entries.md).

Choose the mode with the "Usage" drop-down on each skill's detail page or in the list, where you can check and switch it in place (the bulk operations use the same drop-down). MCP does not return the content of unpublished skills, and after a successful sync it also removes their suggestion entries. The mode is common to all sync targets.

| Mode | Name and description passed for automatic selection | Command suggestions |
| --- | --- | --- |
| Search only (default) | Not passed | Not shown |
| Command suggestions only | Not passed | Shown |
| Automatic selection only | Passed | Hidden where the client allows it |
| Automatic selection + command suggestions | Passed | Shown |

"Automatic selection only" hides the command with `user-invocable: false` in the Claude Code, Grok, and GitHub Copilot entries (the skill leaves the `/` menu but the model can still select it). Codex, Cursor, and Devin have no way to hide the command, so their entries are the same as "Automatic selection + command suggestions" (for Codex, `allow_implicit_invocation: true` in `agents/openai.yaml`). The saved value is the existing `{"automatic":true,"command":false}`; there is no new storage format or migration.

The original content is fetched from MCP when needed, not copied. Skills not shown in suggestions can be found with `$skilvella-search` (explicit invocation only) or MCP's `search_skills`.

If you go back to an older version, the entries are removed at that version's next sync, the same result as "Search only". The older version's list shows "Unknown setting. Choose a usage." for such a skill, and its detail page shows "Automatic selection + command suggestions" in the selector but does not save it unless you change it.

## Invocation commands

For skills that show command suggestions, the command for each sync target (`$name` for Codex, `/name` for the others) is shown under the skill name in the list and under "Usage" on the detail page. With "Automatic selection only", only the commands of clients that cannot hide them are shown (for example Codex's `$name` appears but Claude Code's `/name` does not). If the name contains anything other than lowercase letters, digits, and `-`, it becomes `skill-<first 8 characters of the id>`; if a published skill with the same name exists, it becomes `<name>-<first 8 characters of the id>`. Use the command shown. When there is no command, the detail page shows the reason (search only, automatic selection only so there is no command, no sync target selected, unpublished, or read error). The command for a skill switched from unpublished to published appears after a reload.

## Bulk changes

- In the list, check skills, choose "Usage" under the bulk operations, and press "Apply to selected". "No change" keeps the current mode. Publication state and tags can be changed at the same time.
- "Bulk settings by source" under "All skills" lets you select multiple sources and change publication state and usage with "Apply to selected repositories". The scope is the skills on all pages registered at run time; later additions are not included. The pre-check count is based on the scan index and excludes missing and error entries, and the result shows the numbers updated and excluded.
- When publishing, files are checked again; if any cannot be re-validated (missing, unreadable, invalid), saving for all entries is canceled. Settings are saved in a single transaction, and the original skills are not changed.
- If you press Back or reload during a save in the list, the save result is checked before the latest settings are shown. A request cut off by a dropped connection is canceled.
