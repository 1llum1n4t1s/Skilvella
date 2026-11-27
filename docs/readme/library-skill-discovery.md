# Skill listing and discovery

Part of [Managing your library](library-management.md).

## The skill list

The library lists the skills of all folders from the start and lets you search them in place. A folder with a `.git` directory or file directly inside it is shown nested under the folder as a source (repository); everything else is grouped under "Personal skills". If a folder with no source repository directly inside it is itself a Git repository, the whole folder becomes one source. Categories can be edited from each skill's metadata.

## Missing skills

The normal list shows only skills that currently exist. Old registrations, such as those from before a move, are kept. Review them under the status "Missing" or "All", or under "Clean up registrations for missing skills" on the left.

In the "Missing" list, "Delete all missing registrations" deletes the registration data, settings, and history across all pages matching the search conditions. This cannot be undone and does not change actual files. It aborts the whole deletion if the source cannot be reached or if a path exists.

## How skills are found

Skilvella searches for `SKILL.md` in registered folders **recursively with no depth limit**, and keeps scanning deeper subdirectories even after finding a skill. Excluded: symbolic links, Windows junctions, `.git`, `node_modules`, `target`, and `.work`.

Parent and child skills have separate IDs and publication settings, and a child's files are not included in the parent's MCP publication scope. Folders with the same name do not conflict.

Unregistering a folder does not delete the original files. If you add back a folder you had removed, its earlier management data (publication settings, tags, history) is restored.
