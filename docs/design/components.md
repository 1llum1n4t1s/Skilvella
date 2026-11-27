# コンポーネント一覧

[DESIGN.md](../../DESIGN.md) の「主要コンポーネントと境界」の詳細です。

| コンポーネント | 責務 |
| --- | --- |
| `src/main.rs` | 更新ヘルパー起動の分岐、CLI解析、データ保存先の決定、ポート確保、管理フォルダー初期化、デスクトップとサービスの起動 |
| `src/windows_startup.rs` | Windowsでの親コンソールへの接続と、現在のユーザーのサインイン起動・スタートメニューのショートカットの登録 |
| `src/bin/skilvella-launcher.rs` | 任意の起動補助。自身の配置先を作業ディレクトリにして、同じフォルダーの本体へ引数を転送 |
| `src/service.rs` | 待受ソケットとLibraryを保持し、HTTP/MCP・監視の起動、正常終了、プロセス内再起動を制御 |
| `src/update.rs` | 最新版の確認、配布アーカイブの共通検証、更新ヘルパーによる実行ファイルの置換・再起動・復元 |
| `src/update/auto.rs` | Setup版（Velopack）の自動更新。設定（`app-update.json`）・24時間の間引き・取得と適用。詳細は [velopack-updates.md](velopack-updates.md) |
| `src/update/cleanup.rs` | 起動時に1回、古い更新ステージと自動更新設定の一時ファイルだけを片付ける。詳細は [app-updates.md](app-updates.md) |
| `src/bin/skilvella-release.rs`、`src/update/release.rs`、`src/update/release/` | `release-tools` 機能でビルドする配布専用CLIと、公開前の更新回帰（`release_update`）の起点・候補の用意（`regression.rs`）。アーカイブの梱包、チェックサムの生成、公開前の検証を担当 |
| `src/desktop/` | WindowsのトレイとmacOSのメニューバーのアイコン、既定ブラウザーでの管理画面表示。OSイベント処理をメインスレッドで行う |
| `src/lib.rs` / `AppState` | Library・起動ごとのCSRFトークン・再スキャン通知の共有。`run` で同期I/Oを `spawn_blocking` へ送り、`run_mut` で変更後のCodex入口同期を待つ |
| `src/server.rs` | Axumルーターと `/mcp` の統合、Host / Origin検査、本文サイズ制限、応答セキュリティヘッダー |
| `src/dialog.rs` | 管理画面の「参照」から開くOS標準のフォルダー選択ダイアログ（Windowsは専用STAスレッドの `IFileOpenDialog`、macOSは `osascript`、Linuxは `zenity` / `kdialog`）。パスを返すだけで登録はしない |
| `src/web.rs`、`templates/`、`static/` | Askama SSR、フォーム操作、Comrakプレビュー、標準fetchによるプレビュー更新、未保存変更と二重送信への対処 |
| `src/web/i18n.rs`、`src/web/locales/` | 管理画面のリクエストごとの言語決定、同梱辞書からの翻訳とブラウザーで使う文言の選定 |
| `src/locale.rs`、`src/locales/` | コマンドライン・トレイ／メニューバー・Setup版メッセージボックスなど、管理画面の外の人向け文言の言語決定（`SKILVELLA_LANGUAGE` → `LC_ALL` → `LC_MESSAGES` → `LANG` → OSの表示言語 → 英語）と同梱辞書からの翻訳 |
| `src/mcp.rs` | rmcpによる `search_skills` / `load_skill` / `read_skill_file`。公開済みコンテンツの読み取り専用インターフェース |
| `src/library.rs` | ソース登録、再帰走査、YAML検証、検索、公開判定、本文・メタデータ・履歴の保存、パスと親子境界の検査 |
| `src/model.rs` | 検索条件、スキル概要・本文・参照ファイル、フォルダー設定などの受渡し型 |
| `src/watch.rs` | notifyによる再帰監視、通知の集約、ソース変更・明示再スキャンに応じた監視登録の更新 |

起動補助は `launcher` 機能でのみビルドし、標準ライブラリーだけを使用します。自身の配置先を作業ディレクトリにして本体を起動し、起動成功後は本体の終了を待たずに終了します。WindowsではGUI subsystemと `CREATE_NO_WINDOW` を使い、標準入出力は接続しません。子プロセスの起動失敗は終了コード1で返しますが、起動後の本体の成否は監視しません。通常のビルド・`cargo run` は本体を対象とします。配置・ビルド方法は [任意の起動補助](../DEVELOPMENT.md#任意の起動補助) を参照してください。
