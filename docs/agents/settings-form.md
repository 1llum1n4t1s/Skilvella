# 管理画面の設定フォーム

- 管理画面の設定フォームは `data-save-on-change` を付け、保存ボタンは `<noscript>` 内にだけ置きます。
- `tests/codex_http.rs`・`tests/http_mcp.rs`・`tests/localization.rs` がこの形（属性と `<noscript>` 内のボタン）を検証するため、設定フォームを足すときは同じ形にしてください。
- 管理画面はAskamaテンプレートと同梱アセットを使います。通常ビルドにフロントエンドのビルド工程を加えず、実行時のCDN依存も持ち込まないでください。
- HTTP境界・CSRF・プレビューの安全設定を維持してください。`Referrer-Policy: same-origin` は通常のフォームPOSTのOriginを保持するために必要です。
