# 配布物の梱包と配信

## ZIP版・macOS版

- ZIP版・macOS版の配布物は `release-tools` 機能の `skilvella-release` で梱包し、R2（`skilvella.kagayoi.com/portable/`）へ配信してください。配信元は `.github/workflows/release.yml` と `tools/publish-portable-r2.ps1` の1経路で、GitHub Release は使いません（起動は main のコミットに付けた軽量タグ `v<版>` の push です）。
- 梱包とアプリ内更新では `src/update.rs` の `verify_archive` を共用してください。
- 公開前に `verify` で両OSのアーカイブとチェックサムを検証し、Windows x64・Apple Silicon Mac の両方で `release-update-test` 機能の `release_update` を実行して、前の公開版から最終配布物への更新と再起動を確認してください。コマンドと手順は [配布物の作成と検証](../DEVELOPMENT.md#配布物の作成と検証) を参照してください。

## Windowsインストーラー（Setup）

- `tools/pack-windows-installer.ps1` で梱包し、公開用の署名は `-Sign`（Sigilune）で行ってください。
- Setup に同梱する README.md は、README.md をコピーせず `skilvella-release readme <ソースのSHA> <出力ファイル>` で書き出してください（ZIP版の README.txt と同じリンクの書き換え）。CIでは署名用Secretsのない前の step で、本体と同じ `--target`・`--target-dir` に `skilvella-release`（`release-tools` 機能）もビルドしておいてください。
- 公開用は `.github/workflows/release.yml`（main のコミットに付けた軽量タグ `v<版>` の push で起動）で作り、署名用Secretsを梱包・署名のstep、R2用の `CLOUDFLARE_API_TOKEN` を配信のstep以外へ渡さないでください。
- Setup版の自動更新は [app-update.md](app-update.md) のとおりで、配信は `tools/publish-windows-installer-r2.ps1` でR2へ行います。版付きの成果物は既存オブジェクトを上書きしないでください。手順は [Windowsインストーラーの追加配布](../DEVELOPMENT.md#windowsインストーラーの追加配布) を参照してください。
