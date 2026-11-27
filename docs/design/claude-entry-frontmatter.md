# Claude Codeの入口のfrontmatterと本文の埋め込み

[client-sync.md](client-sync.md) から分割した詳細です。

## frontmatterの引き継ぎ

Claude Codeの入口だけが原本のfrontmatterの一部を引き継ぎます。目的は、`context: fork` と `agent: Explore` を持つスキルを入口経由で呼んでも、Claude Codeがサブエージェントとして fork 実行できるようにすることです。許可リストは `context`（値 `fork` のみ）、`agent` と `background`（`context: fork` のときだけ）、`model`、`effort`（low・medium・high・xhigh・max）、`argument-hint`、`paths` です。値は型を検査し、不正なキーだけを捨てます。

`allowed-tools`・`disallowed-tools`・`hooks` は権限やセッション全体を変え、入口のフォルダーを指してしまうため引き継ぎません。`shell`・`when_to_use`・`license` なども引き継がず、`disable-model-invocation`・`user-invocable` はSkilvellaの「使い方」の設定を正本とします。他のクライアントはこれらのキーの扱いを確認できていないため、name・description と自動選択の表し方だけです。

## `context: fork` の本文の埋め込み

`context: fork` の入口は、案内文の代わりに原本の `SKILL.md` の本文をそのまま埋め込みます。案内文方式は親の文脈を節約するためですが、fork のスキル本文は fork 先にしか届かず、fork 先が案内文を無視した事例もあったためです。前には「`load_skill` で読み直さず、相対リンク・`references`・`scripts` は固定 id の `read_skill_file` で読む」旨を、後ろには `ARGUMENTS:` 行の扱いと `load_skill` 手順を除くMCP案内を付けます。埋め込む入口に限り `arguments`（空白区切りの文字列か文字列の配列）も引き継ぎ、`$ARGUMENTS`・`$0`・`$name` の置換はClaude Codeが行います。

次のときは埋め込まず案内文に戻ります。

- 本文が `!` の動的コマンドを含む：直前が行頭・空白・記号のインライン `` !`…` ``、または本文のどこか（引用・箇条書きの中も含む）の ```!（`~~~!`、フェンス文字と `!` の間の空白も含む）。Claude Codeのブロック構文は本文中のどこでも一致し、許可確認なしに実行するため、実行機能を持たない原則に反します。`panic!` のように英数字の直後の `!` は対象外です。
- `CLAUDE_SKILL_DIR` を含む（入口のフォルダーを指して誤った値になるため）。
- 入口全体が2 MiBを超える、または本文が空。

`~/.claude/skills` はCursor・Grokも読むため、埋め込んだ本文はそれらにも見えます。
