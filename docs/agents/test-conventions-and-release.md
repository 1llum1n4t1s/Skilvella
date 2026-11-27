# 規約検査・起動補助・配布検査のテスト

- `tests/line_exception.rs` は `correct-allow` の期限と承認者の判定（日付換算、期限切れ、人以外の承認）を検証します。実装は `tests/support/line_exception.rs` に置き、規約検査と `#[path]` で共有します。
- `tests/launcher.rs` は任意の起動補助の引数転送・作業ディレクトリ・起動失敗を検証します。`launcher` 機能を指定しない `cargo test --locked` では対象外のため、`--features launcher` を付けて実行してください。`launcher` 機能は依存を追加せず、`src`・`tests` に機能別の分岐もないため、機能付きの1回の実行で既定機能の検証も兼ねます。この前提が変わる場合はCIと AGENTS.md のコマンド一覧に機能なしの実行を戻してください。
- `tests/release_update.rs` は通常のテスト実行の対象外です。CIと AGENTS.md のコマンド一覧では `release-update-test` 機能のClippyだけを行い、実際の更新回帰は [配布物の作成と検証](../DEVELOPMENT.md#配布物の作成と検証) の手順で実行してください。
- `tests/workflow_uses.rs` は、ワークフローの `uses:` が40文字のコミット SHA であることを検証します。タグ・ブランチ参照は失敗します。
- `tests/release_scripts.rs` は、配布スクリプトが full nupkg のファイル名や `SHA256SUMS.txt` の行順を組み立て直さないことを検証します。vpk が書いた full nupkg を探し、チェックサムは行に書いてあるファイルと照合します。
- `tests/command_output.rs` は、コマンドの標準出力を `grep -q` へ直接パイプしないことと、`vpk --help` の色付き出力を除いてから照合することを検証します。
