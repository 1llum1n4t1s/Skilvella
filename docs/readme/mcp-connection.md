# MCP connection

Part of the [Skilvella README](../../README.md).

Register the following URL with a client that supports Streamable HTTP.

```text
http://127.0.0.1:7465/mcp
```

In Toolport, register it as an HTTP server and specify the same URL. Keep Skilvella running.

| Tool | Purpose |
| --- | --- |
| `search_skills` | Search published skills and get summaries and IDs |
| `load_skill` | Get the `SKILL.md` content and the list of reference files by specifying an ID |
| `read_skill_file` | Get UTF-8 text under a skill, by line |

For `load_skill`, keep passing the continuation arguments until `next` is null, concatenate the content, and then follow the instructions. The list is normally 100 entries per page and up to 8 KiB of total paths, but if a single path exceeds 8 KiB, that path is returned alone, so the response becomes large. Fetch skills and the references they need one at a time. For references, use `read_skill_file` to fetch `next_text_offset` for the needed line range to the end, and use `next_start_line` only when more lines are needed. Byte pages of the same line range are concatenated as is, and line breaks are inserted between line ranges when concatenating. When continuing with `next_text_offset`, keep `start_line` and `max_lines`; for the next line range, set `start_line` to `next_start_line` and reset `text_offset` to 0. Line breaks are normalized as before, and the trailing line break is removed. For continuations, specify the returned `revision` as `expected_revision`. If the file changed in the meantime, fetch it again from the start. `id` is the skill id returned by `search_skills`; a unique exact published skill name is also accepted, and an ambiguous name returns candidate ids. The first `read_skill_file` call needs only `id` and `path`; when the result is truncated, call again with the `next` object, which already carries `expected_revision`.

MCP is read-only. It cannot return the content of unpublished skills, skills with invalid frontmatter, or missing skills. There is also no feature to execute reference files. Content is limited to 2 MiB and reference text to 1 MiB.

MCP publication alone does not change the fact that agents standardly discover and load the same skills. To avoid double loading, place skills outside the agent's standard discovery paths and register that location with the library's "+ Add folder". Skilvella never moves files. If existing files need to be moved, do it yourself and then give the new location with the folder's "Change folder location". Codex's configuration files are not edited. Restart Codex after changing the standard discovery paths. Content already loaded into a conversation is not removed. Check custom discovery settings and other clients separately.
