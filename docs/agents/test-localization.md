# 翻訳・言語切替のテスト

- `tests/localization.rs` は初回HTMLとリダイレクト後の通知・履歴の理由の翻訳、全辞書のキー・プレースホルダーとテンプレート参照キーの整合、言語Cookieのポート分離、言語設定と更新操作のHTTP/CSRF境界に加え、管理画面の外の辞書（`src/locales/`）のキー・プレースホルダー・コードの参照キーの整合と、`SKILVELLA_LANGUAGE` による `--help`・引数エラーの言語切替を検証します。
- 出力の文言を確かめる統合テストは、実行環境の言語に左右されないよう `SKILVELLA_LANGUAGE=en` を指定してください。
- 自動更新の保存経路のCSRFとSetup版限定の表示も `tests/localization.rs` が検証します（[app-update.md](app-update.md) を参照）。
