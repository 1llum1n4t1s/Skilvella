# 開発

`rust-toolchain.toml` に固定されたRustを使用し、`Cargo.lock` を使ってビルドしてください。WindowsではMSVC C++ビルドツールとWindows SDK、macOSではXcode Command Line Tools、LinuxではCコンパイラーが必要です。SQLiteのCソースは `rusqlite/bundled` で同梱します。Windows x64はCRTも静的リンクします。

```sh
cargo build --release --locked
```

コード変更時の全チェックは [開発と検証](../AGENTS.md#開発と検証) に従ってください。

テスト用のファイル・DBは次の隔離先に作成します。実際のスキルフォルダーやクライアント設定は使わず、ディレクトリ初期化・Codex入口生成にはテスト用のホームを明示します。

| 保存先 | 使用するテスト |
| --- | --- |
| `.work/test-library` | `tests/library.rs` |
| `.work/test-directory` | `tests/directory.rs` |
| `.work/test-http` | `tests/http_mcp.rs`、`tests/codex_http.rs` の通常フォーム検証、`tests/localization.rs` |
| `.work/test-watch` | `tests/watch.rs` |
| `.work/test-startup` | `tests/startup.rs`、`src/service.rs` 内のテスト |
| `.work/test-codex-index` | `tests/codex_index.rs` |
| `.work/test-launcher` | `tests/launcher.rs`（`launcher` 機能を指定した場合） |
| `SKILVELLA_MIGRATION_TEST_ROOT` 配下（未指定時はOSの一時ディレクトリ配下） | `tests/migration.rs`、`tests/repositories.rs`、`tests/update.rs`、`tests/codex_http.rs` の切断中保存検証、`src/update.rs` 内の一部テスト |

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

同期I/O・検索・保存・索引復旧は [データフローと永続化](../DESIGN.md#データフローと永続化)、スキルID・親子境界・パス検査は [重要な不変条件](../DESIGN.md#重要な不変条件) を参照してください。

既定ディレクトリの自動判定は `Library::initialize_directory` にホームディレクトリを明示して行います。`Library::open` だけではユーザーのスキルを自動探索しません。テストは偽のホームを渡し、実際の利用者フォルダーを走査しないようにしています。

MCPの固定3ツールと検索拡張の制約は [採用した構成の理由とトレードオフ](../DESIGN.md#採用した構成の理由とトレードオフ) を参照してください。

## HTTP境界

Host / Origin・CSRF・プレビュー・`Referrer-Policy` の制約は [重要な不変条件](../DESIGN.md#重要な不変条件)、監視と手動再スキャンの役割は [採用した構成の理由とトレードオフ](../DESIGN.md#採用した構成の理由とトレードオフ) を参照してください。

GitHub ActionsはWindows / macOS / Linuxでformat・Clippy・テスト・リリースビルドを確認します。配布・署名・公証・自動更新のワークフローはまだ含めていません。

## アイコン素材

`static/icons/skilvella.png` がimagegenで生成した透過マスターです。PNG各サイズとICOはWindowsの `pwsh -File scripts/Export-Icons.ps1` で書き出せます。アセットはコミット対象なので、通常ビルドで画像ツールやPowerShellは不要です。ICNSは同じPNGをMac標準の `iconutil` で包装したものです。生成方式とプロンプトは [アイコンの説明](../static/icons/README.md) にあります。
