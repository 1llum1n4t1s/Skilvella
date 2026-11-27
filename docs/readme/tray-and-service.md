# Tray icon, service restart, and direct access

Part of the [installation guide](installation.md).

Skilvella shows its icon in the task tray on Windows and in the menu bar on Mac. Left-click it to open the manager in your default browser. Right-click (or Control-click on Mac) to choose "Open manager", "Check for updates", "Restart service", or "Quit". On Windows, if you cannot see the icon, also check the tray's hidden icons overflow.

"Restart service" restarts the HTTP/MCP server and file watching in the same process and at the same URL. Saved skills and settings are kept. Duplicate requests during a restart are coalesced, and the restart waits for in-flight HTTP operations to finish. MCP connections are dropped, so reconnect from your client, and reload any open manager page. "Quit" shuts down the server and the icon.

You can also open the manager directly at [http://127.0.0.1:7465/](http://127.0.0.1:7465/). The MCP endpoint is `http://127.0.0.1:7465/mcp`. In an environment without a display, pass `--no-tray`. On Linux it shows no icon and runs as an HTTP/MCP server.

No Rust, Node.js, Python, or external database server is needed at runtime. When launched from a terminal, you can also quit with `Ctrl+C` in the attached console.
