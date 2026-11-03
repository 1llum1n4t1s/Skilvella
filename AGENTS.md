# リポジトリ作業規約

SkilvellaはRust製のローカルスキル管理アプリです。構造・データフロー・設計上の制約は [DESIGN.md](DESIGN.md)、利用者向けの操作は [README.md](README.md)、開発環境とアイコン生成は [開発手順](docs/DEVELOPMENT.md) を参照してください。

## 開発と検証

- `rust-toolchain.toml` のRust 1.97.1を使用し、依存解決は `Cargo.lock` に従ってください。manifestの最小Rust版は1.89、editionは2024です。
- WindowsではMSVC C++ビルドツールとWindows SDK、macOSではXcode Command Line Tools、LinuxではCコンパイラーが必要です。SQLiteはbundledビルド、Windows x64のCRT静的リンク設定は `.cargo/config.toml` にあります。
- コード変更時は、CIと同じ次の確認をリポジトリルートで実行してください。CIはWindows / macOS / Linuxを対象とします。

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
```

- 文書だけの変更では、記載したコマンド・パス・挙動を実装と照合し、リンク先と差分を確認してください。
- `tests/localization.rs` は設定画面・同梱言語アセット・更新操作のHTTP/CSRF境界、`tests/update.rs` は隔離した実行ファイルで更新ヘルパーの置換・再起動・復元・中止を検証します。後者も `SKILVELLA_MIGRATION_TEST_ROOT` を使います。サービス再起動と監視・管理情報の保持は `src/service.rs` 内のテストで検証します。
- `tests/library.rs` は保存・検索・パス境界、`tests/directory.rs` は管理フォルダーの選択と親子スキルの境界、`tests/http_mcp.rs` は管理フォームとMCPの連携、`tests/watch.rs` は外部変更の反映、`tests/startup.rs` はヘッドレス起動とポート競合時の非変更、`tests/migration.rs` は旧版の中断移行の起動時復旧・同一性検査・操作ロックを検証します。変更した契約に対応する検証を維持してください。
- `tests/repositories.rs` は取得元の分類・件数・ページ分割・切断時の保持、`tests/codex_index.rs` はCodex入口の公開判定・所有権・競合・中断復旧、`tests/codex_http.rs` は連携フォームのHTTP/CSRF境界と一括変更を検証します。
- テストは `.work/test-library`、`.work/test-directory`、`.work/test-http`、`.work/test-watch`、`.work/test-startup`、`.work/test-codex-index` 配下に独立したデータを作ります。実際の利用者フォルダーやクライアント設定で代用せず、ディレクトリ初期化・Codex入口生成にはテスト用ホームを明示してください。
- 旧版の移行復旧テストでは隔離したフォルダーを移動します。`SKILVELLA_MIGRATION_TEST_ROOT` と子プロセスの `TEMP` / `TMP` をGit作業ツリー外の作業専用フォルダーへ設定してください。未指定時はOSの一時ディレクトリ配下を使います。CIは `SKILVELLA_MIGRATION_TEST_ROOT` に `runner.temp` を指定します。

## 変更時の制約

- スキル管理の同期ファイルI/OとSQLite操作は `src/library.rs` に置き、非同期ハンドラーからの読み取りは `AppState::run`、変更はCodex入口同期を待つ `AppState::run_mut` を通してください。Codex入口の生成・復旧はLibraryの操作ロックと所有権検証を通し、詳細は [Codex連携の設計](DESIGN.md#codexの呼び出し入口) を参照してください。
- フォルダーを移動する新規操作は提供しません。旧版の移行記録の起動時復旧はLibraryの操作ロック・ディレクトリ同一性検査を通してください。通常の走査で未完了の移行を修復する経路は追加しないでください。
- スキル数に比例してMCPツールを増やさず、既存の検索・本文取得・参照取得の3ツールを入口にしてください。MCPからの書込み・ファイル実行は提供しません。
- 公開判定、親子スキルの境界、パス検査、競合検出、履歴保存を迂回する別の読書き経路を追加しないでください。変更時は [設計の不変条件](DESIGN.md#重要な不変条件) を確認してください。
- 管理画面はAskamaテンプレートと同梱アセットを使います。通常ビルドにフロントエンドのビルド工程を加えず、実行時のCDN依存も持ち込まないでください。
- HTTP境界・CSRF・プレビューの安全設定を維持してください。`Referrer-Policy: same-origin` は通常のフォームPOSTのOriginを保持するために必要です。
- アプリ更新は `src/update.rs` の検証・ロック・ヘルパー経路を通し、Libraryや利用者スキルを変更しないでください。信頼境界とToolport連携は [更新の設計](DESIGN.md#アプリの更新) を参照してください。
- 利用者向けの説明はREADME、設計はDESIGN、作業規約は本書に置き、開発環境・素材生成の詳細は `docs/DEVELOPMENT.md` へ集約してください。
