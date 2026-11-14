# リポジトリ作業規約

SkilvellaはRust製のローカルスキル管理アプリです。構造・データフロー・設計上の制約は [DESIGN.md](DESIGN.md)、利用者向けの操作は [README.md](README.md)、開発環境とアイコン生成は [開発手順](docs/DEVELOPMENT.md) を参照してください。

## 開発と検証

- `rust-toolchain.toml` に固定されたRustを使用し、依存解決は `Cargo.lock` に従ってください。OS別のビルド要件は [開発手順](docs/DEVELOPMENT.md#開発) を参照してください。
- コード変更時は、CIと同じ次の確認をリポジトリルートで実行してください。[CI](.github/workflows/ci.yml) はWindows・LinuxのGitHub提供ランナーとmacOSのARM64セルフホストランナーを使い、フォークからのプルリクエストでは検証ジョブをスキップします。CIは依存クレートのビルド結果をキャッシュします。

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets --features launcher -- -D warnings
cargo clippy --locked --features release-update-test --bin skilvella-release --test release_update -- -D warnings
cargo test --locked --features launcher
```

- 修正の途中は、変更に対応するテスト（`cargo test --locked --features launcher --test <名前>` や対象のテスト名）だけを実行し、上の一式は作業の最後に1回実行してください。
- `cargo build --release --locked`（thin LTO・`codegen-units = 1`）は時間がかかるため、CIでは `release/` ブランチとタグでだけ実行します。手元では `Cargo.toml`・`Cargo.lock`・ビルド設定・機能（features）を変更した場合に実行してください。公開前の release ビルドは [配布物の作成と検証](docs/DEVELOPMENT.md#配布物の作成と検証) の手順で両OSとも行います。

- 文書だけの変更では、記載したコマンド・パス・挙動を実装と照合し、リンク先と差分を確認してください。
- `tests/localization.rs` は初回HTMLとリダイレクト後の通知・履歴の理由の翻訳、全辞書のキー・プレースホルダーとテンプレート参照キーの整合、言語Cookieのポート分離、言語設定と更新操作のHTTP/CSRF境界を検証します。
- `tests/update.rs` は隔離した実行ファイルで更新ヘルパーの置換・再起動・復元・中止、再起動先のプロセス回収、Windowsのログハンドル継承防止を検証します。サービス再起動と監視・管理情報の保持は `src/service.rs` 内のテストで検証します。
- `tests/library.rs` は保存・検索・パス境界、`tests/directory.rs` は管理フォルダーの選択、フォルダー追加時の識別子の自動採番・同じフォルダーの再登録・重複と親子関係の拒否、親子スキルの境界、`tests/http_mcp.rs` は管理フォームとMCPの連携・分割取得・継続時の競合検出、旧フォルダー画面の転送とフォルダー選択ダイアログのCSRF境界、一覧の公開切替（公開状態だけの保存と不正ファイルの公開拒否）、`tests/watch.rs` は外部変更の反映、`tests/startup.rs` はヘッドレス起動とポート競合時の非変更、`tests/windows_icon.rs` はWindows版の本体に埋め込んだアイコンをシェルが取り出せること、`tests/migration.rs` は旧版の中断移行の起動時復旧・同一性検査・操作ロックを検証します。変更した契約に対応する検証を維持してください。
- `tests/repositories.rs` は取得元の分類・件数・ページ分割・切断時の保持、不在登録削除のHTTP/CSRF境界と全件検証、読み取りエラーの修正経路、`tests/codex_index.rs` はCodex入口の公開判定・所有権・競合・中断復旧と、Claude Code・Grokの入口の配置・自動選択の制御・Claude Code用リンクの所有と衝突、`tests/codex_http.rs` は同期先フォーム（複数選択と旧形式）のHTTP/CSRF境界と一括変更を検証します。
- `src/windows_startup.rs` 内のテストは、空白・非ASCIIを含む起動コマンドと既存登録・既存のスタートメニューショートカットの保持を検証します。ショートカットは `SKILVELLA_MIGRATION_TEST_ROOT` 配下（未指定時はOSの一時ディレクトリ配下）のテスト用フォルダーへ作成し、実ユーザーのスタートメニューを使いません。レジストリの書込みは `SKILVELLA_MIGRATION_TEST_ROOT` 配下の隔離ハイブで検証し、実ユーザーのスタートアップ設定を使わないでください。
- `tests/launcher.rs` は任意の起動補助の引数転送・作業ディレクトリ・起動失敗を検証します。`launcher` 機能を指定しない `cargo test --locked` では対象外のため、上記のとおり `--features launcher` を付けて実行してください。`launcher` 機能は依存を追加せず、`src`・`tests` に機能別の分岐もないため、機能付きの1回の実行で既定機能の検証も兼ねます。この前提が変わる場合はCIと本一覧に機能なしの実行を戻してください。
- `tests/release_update.rs` は通常のテスト実行の対象外です。CIと上記一覧では `release-update-test` 機能のClippyだけを行い、実際の更新回帰は [配布物の作成と検証](docs/DEVELOPMENT.md#配布物の作成と検証) の手順で実行してください。
- ローカル検証では `SKILVELLA_MIGRATION_TEST_ROOT` と子プロセスの `TEMP` / `TMP` をGit作業ツリー外の作業専用フォルダーへ設定してください。ディレクトリ初期化・Codex入口生成にはテスト用ホームを明示し、利用者のフォルダーとクライアント設定を保護してください。対象テストと保存先は [開発手順の一覧](docs/DEVELOPMENT.md#開発) を参照してください。

## 変更時の制約

- スキル管理の同期ファイルI/OとSQLite操作は `src/library.rs` に置き、非同期ハンドラーからの読み取りは `AppState::run`、変更はCodex入口同期を待つ `AppState::run_mut` を通してください。Codex入口の生成・復旧はLibraryの操作ロックと所有権検証を通し、詳細は [Codex連携の設計](DESIGN.md#codexの呼び出し入口) を参照してください。
- フォルダーを移動する新規操作は提供しません。旧版の移行記録の起動時復旧はLibraryの操作ロック・ディレクトリ同一性検査を通してください。通常の走査で未完了の移行を修復する経路は追加しないでください。
- スキル数に比例してMCPツールを増やさず、既存の検索・本文取得・参照取得の3ツールを入口にしてください。MCPからの書込み・ファイル実行は提供しません。
- 公開判定、親子スキルの境界、パス検査、競合検出、履歴保存を迂回する別の読書き経路を追加しないでください。変更時は [設計の不変条件](DESIGN.md#重要な不変条件) を確認してください。
- 管理画面はAskamaテンプレートと同梱アセットを使います。通常ビルドにフロントエンドのビルド工程を加えず、実行時のCDN依存も持ち込まないでください。翻訳を変更するときは `src/web/i18n.rs` の `Locale` を使い、日本語の原文と `src/web/locales/` の辞書のキー・プレースホルダーを揃えてください。ブラウザーへ渡す翻訳文言は動的な操作に必要なものに限ります。
- HTTP境界・CSRF・プレビューの安全設定を維持してください。`Referrer-Policy: same-origin` は通常のフォームPOSTのOriginを保持するために必要です。
- アプリ更新は `src/update.rs` の検証・ロック・ヘルパー経路を通し、Libraryや利用者スキルを変更しないでください。信頼境界とToolport連携は [更新の設計](DESIGN.md#アプリの更新) を参照してください。
- GitHub配布物は `release-tools` 機能の `skilvella-release` で梱包してください。梱包とアプリ内更新では `src/update.rs` の `verify_archive` を共用してください。公開前に `verify` で両OSのアーカイブとチェックサムを検証し、Windows x64・Apple Silicon Mac の両方で `release-update-test` 機能の `release_update` を実行して、前の公開版から最終配布物への更新と再起動を確認してください。コマンドと手順は [配布物の作成と検証](docs/DEVELOPMENT.md#配布物の作成と検証) を参照してください。
- Windowsインストーラー（Setup）は `tools/pack-windows-installer.ps1` で梱包し、公開用の署名は `-Sign`（Sigilune）で行ってください。公開用は手動起動の `.github/workflows/release-windows-installer.yml` で作り、署名用Secretsを梱包・署名のstep以外へ渡さず、既存のRelease assetを上書きしないでください。手順は [Windowsインストーラーの追加配布](docs/DEVELOPMENT.md#windowsインストーラーの追加配布) を参照してください。
- アイコンの再生成は通常ビルドから独立した `tools/export-icons/Cargo.toml` の開発用パッケージで行い、生成済みの素材を追跡してください。再生成手順は [アイコン素材](docs/DEVELOPMENT.md#アイコン素材) を参照してください。
- 利用者向けの説明はREADME、設計はDESIGN、作業規約は本書に置き、開発環境・素材生成の詳細は `docs/DEVELOPMENT.md` へ集約してください。
