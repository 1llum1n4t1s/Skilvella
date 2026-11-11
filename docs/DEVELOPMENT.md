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

Windowsの本体はGUI subsystemで起動します。CLIの確認では `cargo run -- --help` など、呼び出し元が終了を待機する方法を使ってください。起動補助を使った運用では、標準ライブラリーだけを使う補助を次のコマンドでビルドできます。通常のビルドと `cargo run` は本体を選びます。

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

## 配布物の作成と検証

Windows x64とApple Silicon Macで、同じリリースコミットから本体と配布ツールをビルドします。

```sh
cargo build --locked --release --features release-tools --bin skilvella --bin skilvella-release
```

各環境で本体の `--help` に対象の版が表示されることと、従来の署名状態を確認します。配布ツールの `pack <コミットSHA> <本体のパス> <作成済みの出力先>` で、その環境のアーカイブを作成します。ビルド情報のファイル名は `BUILD-INFO.txt` です。

両方のアーカイブを同じ配布ディレクトリに集め、配布ツールの `checksums <配布ディレクトリ>` で `SHA256SUMS.txt` を作成します。`verify <配布ディレクトリ>` は自動更新と同じ実装で、両アーカイブのSHA-256、ファイル構成、版、実行形式とアーキテクチャを確認します。

**公開前に `verify` と、次の更新回帰テストを Windows x64・Apple Silicon Mac の両方で通します。** リリース候補と同じソースツリーで、3つの配布ファイルを置いた絶対パスと、Git作業ツリー外に作成した書き込み可能な作業用ディレクトリの絶対パスを指定します。起点のタグには、これから置き換える公開済み安定版を指定します（例: v1.0.12 の候補なら `v1.0.11`）。

```sh
cargo test --locked --features release-update-test --test release_update -- <前の公開タグ> <配布ディレクトリの絶対パス> <作業ディレクトリの絶対パス>
```

テストは GitHub から前の公開版を取得してチェックサムを照合し、専用のホーム、データ、実行ファイルで起動します。候補版の配布応答は子プロセスだけが使うループバックの HTTPS フィクスチャが返すため、候補の事前公開や OS の証明書ストアの変更は不要です。設定画面が使う確認・適用の HTTP 操作を実行し、旧プロセスの正常終了、候補実行ファイルへの置き換え、別プロセスへの再起動、同じ URL の管理画面と設定画面が開き、新しい版が表示されることを確認します。

両方の環境で `PASS:` と終了コード 0 を確認し、検証した3ファイルをそのままアップロードします。失敗時は、出力された作業場所にある `startup.log` と `bin/.skilvella-update-*/update.log` を確認します。テストは隔離プロセスを停止します。ログや試験用の実行ファイルは作業場所に残るため、確認後に今回分を清掃してください。再梱包した場合はチェックサムを作り直し、`verify` と両 OS の更新回帰テストを再実行します。

## アイコン素材

`static/icons/skilvella.png` がimagegenで生成した透過マスターです。PNG各サイズとICOを再生成する場合は、リポジトリルートで開発用Rustツールを実行します。

```sh
cargo run --locked --release --manifest-path tools/export-icons/Cargo.toml
```

既定の出力先は `static/icons/` です。別のマスターや出力先を使う場合は、コマンド末尾に `-- --source <正方形のPNG> --output-dir <作成済みのフォルダー>` を追加します。指定したフォルダーの同名PNG・ICOは置き換わるため、同じ出力先には一度に1プロセスで実行してください。

ツールは独立したCargoパッケージです。通常のアプリビルドでは、リポジトリに保存したアイコン素材を同梱します。ICNSは同じPNGをMac標準の `iconutil` で包装したものです。生成方式とプロンプトは [アイコンの説明](../static/icons/README.md) にあります。
