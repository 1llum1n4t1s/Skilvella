# Publishing skills to MCP

Part of [Managing your library](library-management.md).

## Initial state of new skills

New skills found through adding, automatic detection, or external edits are registered with **MCP publication on** if they can be read. Skills with read errors or invalid frontmatter stay unpublished, and they are not published automatically even after you fix them.

The publication state of existing skills never changes on rescan or at startup, so skills you unpublished are never published automatically. On Windows and macOS, a folder whose name differs only in ASCII letter case is treated as the same skill, and its publication state does not change.

If you do not want a skill published, choose "Unpublished" in the list's "Status" column after adding the folder.

## Changing the state

Choosing "Published" or "Unpublished" in the "Status" column saves immediately. You can also switch it on the individual skill page or with the list's bulk operations. Tags and categories do not change.

When switching to published, the files are checked again. Skills that are missing, have read errors, or have invalid frontmatter are not published: the selection is reverted and the reason is shown.
