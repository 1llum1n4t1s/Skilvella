# Usage modes and bulk changes

Part of [Automatic selection and command suggestions](invocation-entries.md).

Choose the mode on each skill's detail page or with "Usage" in the list, where you can check and switch it in place. MCP does not return the content of unpublished skills, and after a successful sync it also removes their suggestion entries. The mode is common to all sync targets.

| Mode | Name and description passed for automatic selection | Command suggestions |
| --- | --- | --- |
| Search only (default) | Not passed | Not shown |
| Command suggestions only | Not passed | Shown |
| Automatic selection + command suggestions | Passed | Shown |

Codex has no setting for "automatic selection but hidden from suggestions", so Skilvella provides these three modes. The original content is fetched from MCP when needed, not copied. Skills not shown in suggestions can be found with `$skilvella-search` (explicit invocation only) or MCP's `search_skills`.

## Invocation commands

For skills that show command suggestions, the command for each sync target (`$name` for Codex, `/name` for the others) is shown under the skill name in the list and under "Usage" on the detail page. If the name contains anything other than lowercase letters, digits, and `-`, it becomes `skill-<first 8 characters of the id>`; if a published skill with the same name exists, it becomes `<name>-<first 8 characters of the id>`. Use the command shown. When there is no command, the detail page shows the reason (search only, no sync target selected, unpublished, or read error). The command for a skill switched from unpublished to published appears after a reload.

## Bulk changes

- In the list, check skills, choose "Usage" under the bulk operations, and press "Apply to selected". "No change" keeps the current mode. Publication state and tags can be changed at the same time.
- "Bulk settings by source" under "All skills" lets you select multiple sources and change publication state and usage with "Apply to selected repositories". The scope is the skills on all pages registered at run time; later additions are not included. The pre-check count is based on the scan index and excludes missing and error entries, and the result shows the numbers updated and excluded.
- When publishing, files are checked again; if any cannot be re-validated (missing, unreadable, invalid), saving for all entries is canceled. Settings are saved in a single transaction, and the original skills are not changed.
- If you press Back or reload during a save in the list, the save result is checked before the latest settings are shown. A request cut off by a dropped connection is canceled.
