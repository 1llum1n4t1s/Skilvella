# リポジトリ作業規約

SkilvellaはRust製のローカルスキル管理アプリです。構造・設計上の制約は [DESIGN.md](DESIGN.md)、利用者向けの操作は [README.md](README.md)、開発環境は [開発手順](docs/DEVELOPMENT.md) を参照してください。

## 開発と検証

- コード変更時は、CIと同じ次の確認をリポジトリルートで実行してください。`uses:` は commit SHA に固定します。

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets --features launcher -- -D warnings
cargo clippy --locked --features release-update-test --bin skilvella-release --test release_update -- -D warnings
cargo test --locked --features launcher
```

- 修正の途中は、変更に対応するテスト（`cargo test --locked --features launcher --test <名前>`）だけを実行し、上の一式は作業の最後に1回実行してください。
- Cargo.toml・Cargo.lock・機能を変えたら `cargo build --release --locked` も実行してください。
- 変更した契約に対応する検証を維持してください。

## 変更時の制約

- Library・SQLite・MCPは `AppState::run` / `run_mut` を通し、公開判定・パス検査・競合検出・履歴保存を迂回する経路、ファイル移動の新規操作、MCPの書込みを追加しない。
- 人が読む文言は9言語の辞書と整合テストを同時に更新する。READMEは全言語版を同時に直す。
- HTTP境界・CSRF・プレビューの安全設定、設定フォームの `data-save-on-change` を維持する。
- アプリ更新は `src/update.rs` の経路を通し、Libraryや利用者スキルを変更しない。

## 詳細（変更前に読む）

- CI・`uses:` の固定・release ビルドは、変更前に [ci-workflow.md](docs/agents/ci-workflow.md) を読む。
- Library・登録フォルダー・MCPは、変更前に [library-and-sources.md](docs/agents/library-and-sources.md) を読む。
- 翻訳・文言は、変更前に [localization.md](docs/agents/localization.md) を読む。
- READMEは、変更前に [readme-translations.md](docs/agents/readme-translations.md) を読む。
- 設定フォーム・HTTP境界は、変更前に [settings-form.md](docs/agents/settings-form.md) を読む。
- アプリ更新は、変更前に [app-update.md](docs/agents/app-update.md) を読む。
- 配布物・Setup・配信は、変更前に [release-distribution.md](docs/agents/release-distribution.md) を読む。
- アイコンは、変更前に [icon-assets.md](docs/agents/icon-assets.md) を読む。
- テストを足す前に、[test-localization.md](docs/agents/test-localization.md)・[test-sources-and-library.md](docs/agents/test-sources-and-library.md)・[test-client-entries.md](docs/agents/test-client-entries.md)・[test-update-and-startup.md](docs/agents/test-update-and-startup.md)・[test-conventions-and-release.md](docs/agents/test-conventions-and-release.md) の該当を読む。
- ローカル検証は、実行前に [test-isolation.md](docs/agents/test-isolation.md) を読み、`TEMP` / `TMP` を作業ツリー外へ向ける。

## 規約と強制

次の検査が失敗する変更は、規約に合っていない。例外は違反している行に、理由・期限・人の承認を書いて付ける。期限を過ぎた例外は失敗する。

`correct-allow: 理由; expires: YYYY-MM-DD; approved: 人の名前`

| 規約 | 強制 |
| --- | --- |
| ワークフローの `uses:` はタグやブランチではなく、40文字のコミット SHA に固定する | `tests/workflow_uses.rs` |
| 配布スクリプトは full nupkg の名前と `SHA256SUMS.txt` の行順を組み立て直さない。書いたファイルを読む | `tests/release_scripts.rs` |
| コマンドの標準出力を `grep -q` へ直接パイプしない。変数に受けてから照合する | `tests/command_output.rs` |
| `vpk --help` は色付けのエスケープを同じ文で除いてから照合する | `tests/command_output.rs` |
