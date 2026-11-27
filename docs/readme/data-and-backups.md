# Data and backups

Part of the [Skilvella README](../../README.md).

| OS | Default data location |
| --- | --- |
| Windows | `%LOCALAPPDATA%\Skilvella` |
| macOS | `~/Library/Application Support/Skilvella` |
| Linux | `$XDG_DATA_HOME/skilvella` or `~/.local/share/skilvella` |

The source of truth for `SKILL.md` and reference files is the registered folders. In addition to the search index, the data folder's `library.sqlite3` also stores tags, categories, publication settings, registered locations, folder display names, and edit history. **If you delete the entire DB, this management data is lost.**

To back up, quit Skilvella first and then copy both the data folder and the registered folders. The DB alone cannot restore the current skill files. On another PC the absolute paths of the registered sources differ, so sharing the DB as is for simultaneous use is not supported.
