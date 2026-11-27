# 同期先クライアント入口のテスト

- `tests/codex_index.rs` はCodex入口の公開判定・所有権・競合・中断復旧と、Claude Code・Grok・Cursor・GitHub Copilot・Devinの入口の配置・自動選択の制御（`disable-model-invocation` と Devin の `triggers: [user]`）・リンク方式（Claude Code・GitHub Copilot・Devin）のリンクの所有と衝突・旧Codexリンクの引き継ぎがClaude Code限定であること・設定フォルダーの検出・Claude Code入口への原本のfrontmatter（許可リストのキーだけ。権限系キーと不正な値は除外し、他クライアントには引き継がない）と引数の受け皿の一文を検証します。`context: fork` の入口は、原本の本文の埋め込みと前置き・後置き、`arguments` の引き継ぎ（文字列・配列）、埋め込まない条件（`!` による動的コマンド・`CLAUDE_SKILL_DIR`・2 MiB超・空の本文。`panic!` のような英数字直後の `!` は対象外）、行頭以外・引用・箇条書きの ```! も従来方式に戻ること、fork 以外と他クライアントの入口が従来どおりであること、非公開化・コマンド候補オフで生成先から本文が消えること（`embedded_body_does_not_remain_after_unpublishing_or_disabling_commands`）を検証します。
- `tests/codex_http.rs` は同期先フォーム（6クライアントの複数選択・折りたたみ内の送信と旧形式）のHTTP/CSRF境界、有効・生成済み・エラーありの同期先を折りたたまない表示の不変条件と一括変更を検証します。
- Devinの設定フォルダーは `Library::open_for_user` がテスト用ホームから導出し、実バイナリを起動するテストは `APPDATA` も隔離したホームへ向けてください。
- Codex入口の設計は [Codex連携の設計](../../DESIGN.md#codexの呼び出し入口) を参照してください。
