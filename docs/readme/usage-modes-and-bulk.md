# Usage modes and bulk changes

Part of [Automatic selection and command suggestions](invocation-entries.md).

Choose the mode with the "Usage" drop-down on each skill's detail page or in the list. MCP does not return the content of unpublished skills, and after a successful sync it also removes their suggestion entries. The mode is common to all sync targets.

| Mode | Name and description passed for automatic selection | Command suggestions |
| --- | --- | --- |
| Search only (default) | Not passed | Not shown |
| Command suggestions only | Not passed | Shown |
| Automatic selection only | Passed | Hidden where the client allows it |
| Automatic selection + command suggestions | Passed | Shown |

"Automatic selection only" hides the command only in clients that allow it ([per client](client-entry-sync.md)). It is saved as the existing `{"automatic":true,"command":false}`, with no migration.

The original content is fetched from MCP when needed, not copied. Skills not shown in suggestions can be found with `$skilvella-search` (explicit invocation only) or MCP's `search_skills`.

An older version treats "Automatic selection only" as "Search only" at its next sync and shows "Unknown setting. Choose a usage."; nothing is saved unless you change it.

## Invocation commands

For skills with command suggestions, the command per target (`$name` for Codex, `/name` for others) is shown under the skill name in the list and under "Usage" on the detail page; with "Automatic selection only", only commands that cannot be hidden appear. Names with characters other than lowercase letters, digits, and `-` become `skill-<id prefix>`, and a name taken by another published skill becomes `<name>-<id prefix>` (8 characters). Use the command shown; otherwise the detail page shows why there is none. A newly published skill's command appears after a reload.

## Bulk changes

- In the list, check skills, choose "Usage" under the bulk operations, and press "Apply to selected". "No change" keeps the current mode. Publication state and tags can be changed together.
- "Bulk settings by source" under "All skills" lets you select multiple sources and change publication state and usage with "Apply to selected repositories". It covers skills registered at run time (all pages); the result shows the numbers updated and excluded (missing and error entries).
- When publishing, files are checked again; if any cannot be re-validated (missing, unreadable, invalid), saving for all entries is canceled. Settings are saved in a single transaction, and the original skills are not changed.
- If you press Back or reload during a save in the list, the save result is checked before the latest settings are shown. A request cut off by a dropped connection is canceled.
