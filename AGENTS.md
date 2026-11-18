# リポジトリ作業規約

SkilvellaはRust製のローカルスキル管理アプリです。構造・データフロー・設計上の制約は [DESIGN.md](DESIGN.md)、利用者向けの操作は [README.md](README.md)、開発環境とアイコン生成は [開発手順](docs/DEVELOPMENT.md) を参照してください。

## 開発と検証

- `rust-toolchain.toml` に固定されたRustを使用し、依存解決は `Cargo.lock` に従ってください。OS別のビルド要件は [開発手順](docs/DEVELOPMENT.md#開発) を参照してください。
- コード変更時は、CIと同じ次の確認をリポジトリルートで実行してください。[CI](.github/workflows/ci.yml) はWindows・Linux・macOS（Apple Silicon の `macos-latest`）のGitHub提供ランナーを使い、フォークからのプルリクエストでは検証ジョブをスキップします。CIは依存クレートのビルド結果をキャッシュし、同じブランチへ続けて push したときは古い run を取り消します（版タグの push では起動しません）。ワークフローの `uses:` はタグではなく commit SHA に固定し、更新時も SHA で指定してください（例外は自作の署名 Action `1llum1n4t1s/Sigilune@v1` の完全一致だけで、この移動タグは Sigilune が配信に成功した commit にだけ動かされます）。それ以外のタグやブランチのままの `uses:` は `tests/workflow_uses.rs` が失敗させます。

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets --features launcher -- -D warnings
cargo clippy --locked --features release-update-test --bin skilvella-release --test release_update -- -D warnings
cargo test --locked --features launcher
```

- 修正の途中は、変更に対応するテスト（`cargo test --locked --features launcher --test <名前>` や対象のテスト名）だけを実行し、上の一式は作業の最後に1回実行してください。
- `cargo build --release --locked`（thin LTO・`codegen-units = 1`）は時間がかかるため、CIでは実行しません（配信タグ `v<版>` の `release.yml` が配信前に release ビルドを行います）。手元では `Cargo.toml`・`Cargo.lock`・ビルド設定・機能（features）を変更した場合に実行してください。公開前の release ビルドは [配布物の作成と検証](docs/DEVELOPMENT.md#配布物の作成と検証) の手順で両OSとも行います。

- 文書だけの変更では、記載したコマンド・パス・挙動を実装と照合し、リンク先と差分を確認してください。
- `tests/localization.rs` は初回HTMLとリダイレクト後の通知・履歴の理由の翻訳、全辞書のキー・プレースホルダーとテンプレート参照キーの整合、言語Cookieのポート分離、言語設定と更新操作のHTTP/CSRF境界に加え、管理画面の外の辞書（`src/locales/`）のキー・プレースホルダー・コードの参照キーの整合と、`SKILVELLA_LANGUAGE` による `--help`・引数エラーの言語切替を検証します。出力の文言を確かめる統合テストは、実行環境の言語に左右されないよう `SKILVELLA_LANGUAGE=en` を指定してください。
- `tests/update.rs` は隔離した実行ファイルで更新ヘルパーの置換・再起動・復元・中止、再起動先のプロセス回収、Windowsのログハンドル継承防止を検証します。サービス再起動と監視・管理情報の保持は `src/service.rs` 内のテストで検証します。
- `tests/relocate.rs` は、フォルダーの移動後に同期が停止しても画面が開き「管理から外す」で復旧すること、場所の付け替え（識別子・公開状態・使い方・履歴の保持、同期の復旧、別の有効なフォルダーとの重複・親子・存在しない場所・データフォルダーとの重なりの拒否、既定フォルダーの設定の更新、CSRF）と、読み取れないフォルダーの警告・同期停止の警告からのリンク・場所の変更フォームの形（`data-save-on-change` を付けない明示送信）を検証します。`tests/library.rs` は保存・検索・パス境界、`tests/directory.rs` は管理フォルダーの選択、フォルダー追加時の識別子の自動採番・同じフォルダーの再登録・重複と親子関係の拒否、親子スキルの境界、`tests/http_mcp.rs` は管理フォームとMCPの連携・分割取得・継続時の競合検出、旧フォルダー画面の転送とフォルダー選択ダイアログのCSRF境界、一覧の公開切替（公開状態だけの保存と不正ファイルの公開拒否）、`tests/watch.rs` は外部変更の反映と付け替え後の監視先の張り替え、`tests/startup.rs` はヘッドレス起動とポート競合時の非変更、`tests/windows_icon.rs` はWindows版の本体に埋め込んだアイコンをシェルが取り出せること、`tests/migration.rs` は旧版の中断移行の起動時復旧・同一性検査・操作ロックを検証します。変更した契約に対応する検証を維持してください。
- `tests/repositories.rs` は取得元の分類・件数・ページ分割・切断時の保持、不在登録削除のHTTP/CSRF境界と全件検証、読み取りエラーの修正経路、`tests/codex_index.rs` はCodex入口の公開判定・所有権・競合・中断復旧と、Claude Code・Grok・Cursor・GitHub Copilot・Devinの入口の配置・自動選択の制御（`disable-model-invocation` と Devin の `triggers: [user]`）・リンク方式（Claude Code・GitHub Copilot・Devin）のリンクの所有と衝突・旧Codexリンクの引き継ぎがClaude Code限定であること・設定フォルダーの検出、`tests/codex_http.rs` は同期先フォーム（6クライアントの複数選択・折りたたみ内の送信と旧形式）のHTTP/CSRF境界、有効・生成済み・エラーありの同期先を折りたたまない表示の不変条件と一括変更を検証します。Devinの設定フォルダーは `Library::open_for_user` がテスト用ホームから導出し、実バイナリを起動するテストは `APPDATA` も隔離したホームへ向けてください。
- `src/windows_startup.rs` 内のテストは、空白・非ASCIIを含む起動コマンドと既存登録・既存のスタートメニューショートカットの保持、Setup版のインストール時のZIP版の自動起動登録の引き継ぎ（別プログラムの値は保持）と、アンインストール時にSetup版自身の登録だけを外すことを検証します。Setupのフックは `update-feed-test` ビルドでは別の値名を使い、隔離検証で実ユーザーの自動起動を置き換えません。ショートカットは `SKILVELLA_MIGRATION_TEST_ROOT` 配下（未指定時はOSの一時ディレクトリ配下）のテスト用フォルダーへ作成し、実ユーザーのスタートメニューを使いません。レジストリの書込みは `SKILVELLA_MIGRATION_TEST_ROOT` 配下の隔離ハイブで検証し、実ユーザーのスタートアップ設定を使わないでください。
- 管理画面の設定フォームは `data-save-on-change` を付け、保存ボタンは `<noscript>` 内にだけ置きます。`tests/codex_http.rs`・`tests/http_mcp.rs`・`tests/localization.rs` がこの形（属性と `<noscript>` 内のボタン）を検証するため、設定フォームを足すときは同じ形にしてください。
- `tests/line_exception.rs` は `correct-allow` の期限と承認者の判定（日付換算、期限切れ、人以外の承認）を検証します。実装は `tests/support/line_exception.rs` に置き、規約検査と `#[path]` で共有します。
- `tests/launcher.rs` は任意の起動補助の引数転送・作業ディレクトリ・起動失敗を検証します。`launcher` 機能を指定しない `cargo test --locked` では対象外のため、上記のとおり `--features launcher` を付けて実行してください。`launcher` 機能は依存を追加せず、`src`・`tests` に機能別の分岐もないため、機能付きの1回の実行で既定機能の検証も兼ねます。この前提が変わる場合はCIと本一覧に機能なしの実行を戻してください。
- `src/update/auto.rs` 内のテストは自動更新設定の既定値・壊れたJSON・原子的保存・24時間の間引きを検証し、`tests/localization.rs` は自動更新の保存経路のCSRFとSetup版限定の表示を検証します。Velopackを呼ぶ処理は `windows-installer` 機能付きのWindowsビルドだけが対象で、実機での取得・適用は [開発手順](docs/DEVELOPMENT.md#windowsインストーラーの追加配布) の隔離環境の手順（`update-feed-test` 機能）で確認します。
- `src/update/cleanup.rs` 内のテストは、起動時の一時物の清掃が、古い更新ステージと設定の一時ファイルだけを消すことを検証します。新しいもの・名前違い・未知のファイルを含むステージ・リンク・更新ロック保持中・実行ファイル本体とロックファイルは残ります。清掃の対象を広げるときは、DESIGN.md の条件と同時に見直してください。
- `tests/release_update.rs` は通常のテスト実行の対象外です。CIと上記一覧では `release-update-test` 機能のClippyだけを行い、実際の更新回帰は [配布物の作成と検証](docs/DEVELOPMENT.md#配布物の作成と検証) の手順で実行してください。
- `tests/workflow_uses.rs` は、ワークフローの `uses:` が40文字のコミット SHA であることを検証します。タグ・ブランチ参照は失敗します。
- `tests/release_scripts.rs` は、配布スクリプトが full nupkg のファイル名や `SHA256SUMS.txt` の行順を組み立て直さないことを検証します。vpk が書いた full nupkg を探し、チェックサムは行に書いてあるファイルと照合します。
- `tests/command_output.rs` は、コマンドの標準出力を `grep -q` へ直接パイプしないことと、`vpk --help` の色付き出力を除いてから照合することを検証します。
- ローカル検証では `SKILVELLA_MIGRATION_TEST_ROOT` と子プロセスの `TEMP` / `TMP` をGit作業ツリー外の作業専用フォルダーへ設定してください。ディレクトリ初期化・Codex入口生成にはテスト用ホームを明示し、利用者のフォルダーとクライアント設定を保護してください。対象テストと保存先は [開発手順の一覧](docs/DEVELOPMENT.md#開発) を参照してください。

## 変更時の制約

- スキル管理の同期ファイルI/OとSQLite操作は `src/library.rs` に置き、非同期ハンドラーからの読み取りは `AppState::run`、変更はCodex入口同期を待つ `AppState::run_mut` を通してください。Codex入口の生成・復旧はLibraryの操作ロックと所有権検証を通し、詳細は [Codex連携の設計](DESIGN.md#codexの呼び出し入口) を参照してください。
- ファイルを移動する新規操作は提供しません。許可するのは、ファイルを動かさず登録の場所だけを付け替える `Library::relocate_source`（`POST /sources/location`）だけで、登録と共通の `checked_source_root` と他の有効なフォルダーとの重複・親子の検査、操作ロック、`AppState::run_mut` を通してください。同期の停止条件（有効なソースを開けないと同期全体を止める `export_codex` の検査）は変えず、読み取れないフォルダーの印（`source_unavailable:<alias>`）は走査時にだけ書き、画面の描画で全フォルダーを `read_dir` しないでください。旧版の移行記録の起動時復旧はLibraryの操作ロック・ディレクトリ同一性検査を通してください。通常の走査で未完了の移行を修復する経路は追加しないでください。
- スキル数に比例してMCPツールを増やさず、既存の検索・本文取得・参照取得の3ツールを入口にしてください。MCPからの書込み・ファイル実行は提供しません。
- 公開判定、親子スキルの境界、パス検査、競合検出、履歴保存を迂回する別の読書き経路を追加しないでください。変更時は [設計の不変条件](DESIGN.md#重要な不変条件) を確認してください。
- 管理画面はAskamaテンプレートと同梱アセットを使います。通常ビルドにフロントエンドのビルド工程を加えず、実行時のCDN依存も持ち込まないでください。翻訳を変更するときは `src/web/i18n.rs` の `Locale` を使い、日本語の原文と `src/web/locales/` の辞書のキー・プレースホルダーを揃えてください。ブラウザーへ渡す翻訳文言は動的な操作に必要なものに限ります。
- 人が読む文言（画面・コマンドライン出力・エラー・トレイやメニュー・Setup版のメッセージ）を足したときは、9言語（ja・en・zh-CN・zh-TW・ko・es・fr・de・pt-BR）の辞書と整合テストを同時に更新してください。管理画面は `src/web/locales/`、管理画面の外（`skilvella::locale`）は `src/locales/` で、どちらも日本語の原文をキーにします。`locale::text("…")` / `locale::format("…")` のキーはリテラルで渡し、エスケープは使わないでください（テストが抽出します）。起動時の `Browser:`・`MCP:`・`Data:` の行は機械が読むため訳さないでください。MCPのツール名・説明・入力スキーマ・AIへ返すエラーなどAIが読む文言は英語に統一し、訳し分けないでください。
- READMEを変えたら、英語の `README.md`（正本）と `README.ja.md`・`README.zh-CN.md`・`README.zh-TW.md`・`README.ko.md`・`README.es.md`・`README.fr.md`・`README.de.md`・`README.pt-BR.md` の全言語版を同時に直してください。各ファイルの先頭に9言語の切替行を置き、コマンド・パス・設定キー・製品名・URLは訳さず、見出しへのリンクは言語で変わらない明示アンカー（`<a id="app-updates"></a>` など）を使ってください。
- HTTP境界・CSRF・プレビューの安全設定を維持してください。`Referrer-Policy: same-origin` は通常のフォームPOSTのOriginを保持するために必要です。
- アプリ更新は `src/update.rs` の検証・ロック・ヘルパー経路を通し、Libraryや利用者スキルを変更しないでください。信頼境界とToolport連携は [更新の設計](DESIGN.md#アプリの更新) を参照してください。
- ZIP版・macOS版の配布物は `release-tools` 機能の `skilvella-release` で梱包し、R2（`skilvella.kagayoi.com/portable/`）へ配信してください。配信元は `.github/workflows/release.yml` と `tools/publish-portable-r2.ps1` の1経路で、GitHub Release・タグは使いません。梱包とアプリ内更新では `src/update.rs` の `verify_archive` を共用してください。公開前に `verify` で両OSのアーカイブとチェックサムを検証し、Windows x64・Apple Silicon Mac の両方で `release-update-test` 機能の `release_update` を実行して、前の公開版から最終配布物への更新と再起動を確認してください。コマンドと手順は [配布物の作成と検証](docs/DEVELOPMENT.md#配布物の作成と検証) を参照してください。
- Windowsインストーラー（Setup）は `tools/pack-windows-installer.ps1` で梱包し、公開用の署名は `-Sign`（Sigilune）で行ってください。公開用は `.github/workflows/release.yml`（main のコミットに付けた軽量タグ `v<版>` の push で起動）で作り、署名用Secretsを梱包・署名のstep、R2用の `CLOUDFLARE_API_TOKEN` を配信のstep以外へ渡さないでください。Setup版の自動更新は `src/update/auto.rs`（更新元は `update_source` の1か所、設定は `app-update.json` でLibraryに入れない）で行い、配信は `tools/publish-windows-installer-r2.ps1` でR2へ行います。版付きの成果物は既存オブジェクトを上書きしないでください。手順は [Windowsインストーラーの追加配布](docs/DEVELOPMENT.md#windowsインストーラーの追加配布) を参照してください。
- アイコンの再生成は通常ビルドから独立した `tools/export-icons/Cargo.toml` の開発用パッケージで行い、生成済みの素材を追跡してください。再生成手順は [アイコン素材](docs/DEVELOPMENT.md#アイコン素材) を参照してください。
- 利用者向けの説明はREADME、設計はDESIGN、作業規約は本書に置き、開発環境・素材生成の詳細は `docs/DEVELOPMENT.md` へ集約してください。

## 規約と強制

次の検査が失敗する変更は、規約に合っていない。エラーがファイルと、代わりに使う形を示す。文書だけの注意は検査にならない。例外は違反している行に、理由・期限・人の承認を書いて付ける。期限を過ぎた例外は失敗する。

`correct-allow: 理由; expires: YYYY-MM-DD; approved: 人の名前`

| 規約 | 強制 |
| --- | --- |
| ワークフローの `uses:` はタグやブランチではなく、40文字のコミット SHA に固定する | `tests/workflow_uses.rs` |
| 配布スクリプトは full nupkg の名前と `SHA256SUMS.txt` の行順を組み立て直さない。書いたファイルを読む | `tests/release_scripts.rs` |
| コマンドの標準出力を `grep -q` へ直接パイプしない。変数に受けてから照合する | `tests/command_output.rs` |
| `vpk --help` は色付けのエスケープを同じ文で除いてから照合する | `tests/command_output.rs` |
