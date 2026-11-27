# Options and troubleshooting

Part of the [Skilvella README](../../README.md).

```sh
skilvella --data-dir /absolute/path/to/data --port 7465 --no-watch
skilvella --help
```

- If the port is in use, quit the existing process or change it with `--port`. The manager and MCP both use the same port.
- If the manager does not come back after updating from an old Windows version, launch the updated `skilvella.exe` once in the normal layout, or the launcher once under Toolport management.
- If an interrupted folder move from an old version cannot be recovered at startup, startup is aborted. The original data and the migration record are kept.
- The data location and the skill registration locations cannot overlap. Specify separate folders on a local disk.
- Edits made in another tab are also detected as conflicts on save. Copy the retained draft, reload, and merge it into the latest content.
- Search uses whitespace-separated AND and substring matching. Searching English skills by meaning from a Japanese query, embedding models, and cloud sync are not included.
- Even if the source folder goes offline, the registration data and history are kept. Rescan after it reconnects.

Connections are limited to `127.0.0.1`. It is intended for use where you trust the users and processes on the same PC.
