# 開発

Rust 1.97.1を `rust-toolchain.toml` に固定しています。`Cargo.lock` を使ってビルドしてください。WindowsではMSVC C++ビルドツールとWindows SDK、macOSではXcode Command Line Tools、LinuxではCコンパイラーが必要です。SQLiteのCソースは `rusqlite/bundled` で同梱します。Windows x64はCRTも静的リンクします。

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
```

テスト用のファイル・DBはリポジトリの `.work/test-library` と `.work/test-http` に作成します。実際のスキルフォルダーやクライアント設定は使いません。テスト終了後も調査用に残すため、不要になった時点でそのフォルダーだけを清掃してください。

## 構成

単一のRust実行ファイルに管理画面・静的アセット・MCPサーバーを含めます。フロントエンドのビルド工程や、実行時のCDN参照はありません。

| 役割 | 採用ライブラリー |
| --- | --- |
| HTTP / 非同期実行 | Axum / Tokio |
| SSR / 部分更新 | Askama / 同梱htmx |
| MCP | 公式Rust SDK `rmcp` のサーバー・HTTP機能 |
| 保存 / 検索 | `rusqlite` + bundled SQLite / FTS5 trigram |
| YAML / Markdown | Saphyr / Comrak（CLI・シンタックスハイライト機能なし） |
| ファイル監視 | notify |

`library.rs` は同期I/Oを担当し、HTTPハンドラーは `AppState::run` からブロッキングスレッドへ送ります。SQLiteはWAL、接続ごとのbusy timeout、複製間で共有する書込ロックを使います。ファイルとSQLiteは単一のトランザクションにはならないため、本文を置き換える前に元本文を履歴へ保存します。索引更新が失敗した場合、再スキャンでファイルから再構築します。別プロセスが同じデータを同時に編集する運用は対象外です。

スキルIDは登録エイリアスと正規化した相対パスから作ります。絶対パスそのものをIDには含めません。走査はSKILL.mdの発見後も継続し、入れ子になったスキルを別IDとして扱います。参照ファイルの列挙・取得では子スキルの境界を越えません。登録先の重複とリンク経由の逸脱は拒否します。外部編集は保存直前にもハッシュで検出します。OS外部のプロセスが検査直後にファイル構造を差し替える競合を完全に排除するものではありません。

既定ディレクトリの自動判定は `Library::initialize_directory` にホームディレクトリを明示して行います。`Library::open` だけではユーザーのスキルを自動探索しません。テストは偽のホームを渡し、実際の利用者フォルダーを走査しないようにしています。

長いキーワードはFTS5で候補を絞り、短い語はリテラル部分一致で補います。候補を名前・タグ優先で順位付けします。意味検索を追加する場合も、MCPのツール数をスキル数に比例させず、検索サービス内へ追加してください。

## HTTP境界

- IPv4 loopbackへbindし、Host / Originを検査します。MCPは同じ制限を使います。
- 管理画面の変更はPOST + 起動ごとのCSRFトークンです。
- AskamaのHTMLエスケープ、CSP、Comrakの安全設定を維持します。
- `Referrer-Policy: same-origin` は同一サイト内フォームのOriginを保持するために必要です。`no-referrer` へ変更すると通常のフォームPOSTでOriginがnullになり、境界検査に拒否されます。
- ファイル監視は通知をまとめ、手動再スキャンを復旧経路として残します。

GitHub ActionsはWindows / macOS / Linuxでformat・Clippy・テスト・リリースビルドを確認します。配布・署名・公証・自動更新のワークフローはまだ含めていません。

## アイコン素材

`static/icons/skilvella.png` がimagegenで生成した透過マスターです。PNG各サイズとICOはWindowsの `pwsh -File scripts/Export-Icons.ps1` で書き出せます。アセットはコミット対象なので、通常ビルドで画像ツールやPowerShellは不要です。ICNSは同じPNGをMac標準の `iconutil` で包装したものです。生成方式とプロンプトは [アイコンの説明](../static/icons/README.md) にあります。
