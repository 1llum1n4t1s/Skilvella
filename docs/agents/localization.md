# 翻訳と人が読む文言

- 翻訳を変更するときは `src/web/i18n.rs` の `Locale` を使い、日本語の原文と `src/web/locales/` の辞書のキー・プレースホルダーを揃えてください。ブラウザーへ渡す翻訳文言は動的な操作に必要なものに限ります。
- 人が読む文言（画面・コマンドライン出力・エラー・トレイやメニュー・Setup版のメッセージ）を足したときは、9言語（ja・en・zh-CN・zh-TW・ko・es・fr・de・pt-BR）の辞書と整合テストを同時に更新してください。管理画面は `src/web/locales/`、管理画面の外（`skilvella::locale`）は `src/locales/` で、どちらも日本語の原文をキーにします。
- `locale::text("…")` / `locale::format("…")` のキーはリテラルで渡し、エスケープは使わないでください（テストが抽出します）。
- 起動時の `Browser:`・`MCP:`・`Data:` の行は機械が読むため訳さないでください。MCPのツール名・説明・入力スキーマ・AIへ返すエラーなどAIが読む文言は英語に統一し、訳し分けないでください。
- 検証は [test-localization.md](test-localization.md) を参照してください。
