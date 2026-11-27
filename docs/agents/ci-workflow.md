# CIとビルドの運用

- `rust-toolchain.toml` に固定されたRustを使用し、依存解決は `Cargo.lock` に従ってください。OS別のビルド要件は [開発手順](../DEVELOPMENT.md#開発) を参照してください。
- [CI](../../.github/workflows/ci.yml) はWindows・Linux・macOS（Apple Silicon の `macos-latest`）のGitHub提供ランナーを使い、フォークからのプルリクエストでは検証ジョブをスキップします。CIは main への push とプルリクエストで起動し（他のブランチは PR で検証、版タグの push では起動しません）、依存クレートのビルド結果をキャッシュし、同じプルリクエストへ続けて push したときは古い run を取り消します（main への push は run ごとに独立して完走します）。
- ワークフローの `uses:` はタグではなく commit SHA に固定し、更新時も SHA で指定してください（例外は自作の署名 Action `1llum1n4t1s/Sigilune@v1` の完全一致だけで、この移動タグは Sigilune が配信に成功した commit にだけ動かされます）。それ以外のタグやブランチのままの `uses:` は `tests/workflow_uses.rs` が失敗させます。
- `cargo build --release --locked`（thin LTO・`codegen-units = 1`）は時間がかかるため、CIでは実行しません（配信タグ `v<版>` の `release.yml` が配信前に release ビルドを行います）。手元では `Cargo.toml`・`Cargo.lock`・ビルド設定・機能（features）を変更した場合に実行してください。公開前の release ビルドは [配布物の作成と検証](../DEVELOPMENT.md#配布物の作成と検証) の手順で両OSとも行います。
- 文書だけの変更では、記載したコマンド・パス・挙動を実装と照合し、リンク先と差分を確認してください。
