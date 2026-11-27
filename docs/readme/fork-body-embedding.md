# Body embedding for `context: fork`

Part of [Claude Code frontmatter carry-over](claude-frontmatter-carryover.md).

- For an entry whose original has `context: fork`, the entry embeds the original `SKILL.md` body as is, instead of the "read the body with `load_skill`" pointer. The body of a forked skill reaches only the forked agent, so saving the parent's context is no reason to use a pointer, and forked agents have ignored the pointer. A short note before the body says not to re-read it with `load_skill` and to use `read_skill_file` for referenced files (relative links, `references`, `scripts`), which are not in the entry folder.
- The body is not embedded, and the entry stays the pointer, when:
  - the body contains a dynamic command with `!`: inline `` !`...` `` after a line start, space or symbol, or ```! anywhere in the body (including quotes and lists; `~~~!` and a space before `!` also count). Skilvella does not run files or scripts, and Claude Code would run it without a permission prompt. A `!` right after a letter or digit, as in `panic!`, does not count.
  - the body contains `CLAUDE_SKILL_DIR` (it would point at the entry folder).
  - the whole entry exceeds 2 MiB, or the body is empty.
- Claude Code replaces `$ARGUMENTS`, `$0`, `$name`, and similar, as it does for native skills. An embedding entry also carries over `arguments`.
- `~/.claude/skills` is also read by Cursor and Grok, so the embedded body is visible to them too.
- Making a skill private or turning off command candidates removes the entry as before. The retired entry (`~/.claude/skilvella-retired/<uuid>`) keeps a copy with the body until the next sync; it is not a discovery location.
