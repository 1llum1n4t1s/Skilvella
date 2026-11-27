# 同期先クライアント入口のテスト

- `tests/codex_index.rs` はCodex入口の公開判定・所有権・競合・中断復旧と、Claude Code・Grok・Cursor・GitHub Copilot・Devinの入口の配置・自動選択の制御（`disable-model-invocation` と Devin の `triggers: [user]`）・リンク方式（Claude Code・GitHub Copilot・Devin）のリンクの所有と衝突・旧Codexリンクの引き継ぎがClaude Code限定であること・設定フォルダーの検出・Claude Code入口への原本のfrontmatter（許可リストのキーだけ。権限系キーと不正な値は除外し、他クライアントには引き継がない）と引数の受け皿の一文を検証します。
- `tests/codex_http.rs` は同期先フォーム（6クライアントの複数選択・折りたたみ内の送信と旧形式）のHTTP/CSRF境界、有効・生成済み・エラーありの同期先を折りたたまない表示の不変条件と一括変更を検証します。
- Devinの設定フォルダーは `Library::open_for_user` がテスト用ホームから導出し、実バイナリを起動するテストは `APPDATA` も隔離したホームへ向けてください。
- Codex入口の設計は [Codex連携の設計](../../DESIGN.md#codexの呼び出し入口) を参照してください。
