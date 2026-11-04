# 開発

Rust 1.99.1を `rust-toolchain.toml` に固定しています。`Cargo.lock` を使ってビルドしてください。WindowsではMSVC C++ビルドツールとWindows SDK、macOSではXcode Command Line Tools、LinuxではCコンパイラーが必要です。SQLiteのCソースは `rusqlite/bundled` で同梱します。Windows x64はCRTも静的リンクします。

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
```

テスト用のファイル・DBは次の隔離先に作成します。実際のスキルフォルダーやクライアント設定は使わず、ディレクトリ初期化・Codex入口生成にはテスト用のホームを明示します。

| 保存先 | 使用するテスト |
| --- | --- |
| `.work/test-library` | `tests/library.rs` |
| `.work/test-directory` | `tests/directory.rs` |
| `.work/test-http` | `tests/http_mcp.rs`、`tests/codex_http.rs`、`tests/localization.rs` |
| `.work/test-watch` | `tests/watch.rs` |
| `.work/test-startup` | `tests/startup.rs`、`src/service.rs` 内のテスト |
| `.work/test-codex-index` | `tests/codex_index.rs` |
| `.work/test-launcher` | `tests/launcher.rs`（`launcher` 機能を指定した場合） |
| `SKILVELLA_MIGRATION_TEST_ROOT` 配下（未指定時はOSの一時ディレクトリ配下） | `tests/migration.rs`、`tests/repositories.rs`、`tests/update.rs`、`src/update.rs` 内の一部テスト |

ローカル検証では、`SKILVELLA_MIGRATION_TEST_ROOT` と子プロセスの `TEMP` / `TMP` をGit作業ツリー外の作業専用フォルダーへ設定してください。CIは `SKILVELLA_MIGRATION_TEST_ROOT` に `runner.temp` を指定します。調査用に残るデータは、不要になった時点で今回のテストが作成したフォルダーだけを清掃してください。

## 構成

### 任意の起動補助

WindowsでコンソールやPowerShellを介さず起動する場合は、標準ライブラリーだけを使う任意の起動補助をビルドできます。通常のビルドは従来どおり `skilvella` だけを生成し、`cargo run` も本体を選びます。

```sh
cargo build --release --locked --features launcher --bin skilvella-launcher
cargo test --locked --features launcher --test launcher
```

`skilvella-launcher.exe` と `skilvella.exe` を同じフォルダーへ配置します。補助はそのフォルダーを作業ディレクトリにして本体へ引数を渡し、起動後に終了します。WindowsではGUI subsystemと `CREATE_NO_WINDOW` を使い、本体の起動に失敗した場合は終了コード1を返します。起動後の本体の成否は待機しません。追加依存や起動用スクリプトは不要です。起動補助自体はRust 1.97.1でもビルドでき、Windows以外では同じフォルダーの `skilvella` を起動します。

統合テストは空白を含む隔離フォルダーへ補助と確認用テスト実行ファイルを配置し、実アプリを起動せずに引数・作業ディレクトリ・対象不在時の失敗を検証します。

### 本体

単一のRust実行ファイルに管理画面・静的アセット・MCPサーバーを含めます。フロントエンドのビルド工程や、実行時のCDN参照はありません。

| 役割 | 採用ライブラリー |
| --- | --- |
| HTTP / 非同期実行 | Axum / Tokio |
| SSR / プレビュー更新 | Askama / 標準fetch |
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
