# Skilvellaの設計

## 目的と実行形態

ローカルの `SKILL.md` 群をブラウザーで管理し、必要なスキルだけを固定3ツールのMCP経由で取得するアプリです。単一Rust実行ファイルが管理画面とStreamable HTTP MCPを同じIPv4 loopbackポートで提供し、テンプレート・静的アセット・SQLiteを同梱します。操作は [README.md](README.md)、開発・検証は [AGENTS.md](AGENTS.md) を参照してください。詳細は `docs/design/` に分割しています。

## 主要コンポーネントと境界

`src/main.rs` が起動、`src/service.rs` が待受・HTTP/MCP・監視・再起動、`src/server.rs` がAxumとHost/Origin検査、`src/web.rs` が管理画面、`src/mcp.rs` が読み取り専用の3ツール、`src/library.rs` がソース登録・走査・検索・公開判定・保存・パス検査（同期I/OとSQLiteはここだけ）、`src/watch.rs` が監視、`src/update.rs` が更新を担います。全ファイルの責務は [コンポーネント一覧](docs/design/components.md) です。

## Windowsの起動とサインイン登録

Run登録とスタートメニューのショートカット作成、Setup版での扱いは [windows-startup.md](docs/design/windows-startup.md) にあります。

## サービスの再起動と終了

トレイ・メニューバーからのプロセス内再起動と、同じポートでの待受再開は [service-restart.md](docs/design/service-restart.md) にあります。

## ライブラリの取得元選択

フォルダーと取得元を1画面に統合した一覧と、不在登録の扱いは [library-sources.md](docs/design/library-sources.md) にあります。場所変更と不在検出は [source-relocation.md](docs/design/source-relocation.md)、一括公開は [bulk-publish.md](docs/design/bulk-publish.md) です。

## MCPの分割取得

本文・参照を約8 KiBずつ返す継続取得とリビジョン検査は [mcp-chunking.md](docs/design/mcp-chunking.md) にあります。

## アプリの更新

R2の `latest.json` とチェックサムで検証し、更新ヘルパーが置換・再起動・復元します。詳細は [app-updates.md](docs/design/app-updates.md) です。

### Setup版（Velopack）の自動更新

Setup版は24時間間引きの自動取得と次回起動時の適用を行います。詳細は [velopack-updates.md](docs/design/velopack-updates.md)、配布物の梱包と検証は [release-packaging.md](docs/design/release-packaging.md) です。更新後の一時物の清掃は [update-cleanup.md](docs/design/update-cleanup.md) です。

## 表示設定

言語の決定、9言語の辞書、テーマ、設定フォームの自動保存は [display-settings.md](docs/design/display-settings.md) にあります。

## Codexの呼び出し入口

Codex連携は明示的に有効化し、MCPの本文取得へ誘導する短い `SKILL.md` を生成します（Claude Codeの入口のうち原本が `context: fork` を持つものだけは例外で、条件を満たせば原本の本文を埋め込みます。[claude-entry-frontmatter.md](docs/design/claude-entry-frontmatter.md) を参照）。所有記録の検査と同期の方針は [codex-entrypoints.md](docs/design/codex-entrypoints.md)、各クライアントの配置は [client-sync.md](docs/design/client-sync.md) です。探索先のリンクと検出表示は [client-links.md](docs/design/client-links.md) です。

## データフローと永続化

起動時にポートを確保し、Libraryを開いて走査し、`SKILL.md` のfrontmatterを解析してSQLiteの索引（FTS5）を更新します。検索は索引、本文・参照はファイルの再検査を経て返し、編集は履歴保存後に置換します。ファイルが正本で、DBは管理情報と索引です。詳細は [data-flow.md](docs/design/data-flow.md) です。

## 重要な不変条件

公開判定、親子スキルの境界、パス検査、CSRF・Host/Origin検査、サイズ上限、実行機能なしを維持します。条件の全文は [invariants.md](docs/design/invariants.md) です。

## 採用した構成の理由とトレードオフ

固定3ツール、ファイル正本、単一実行ファイル、監視の集約の理由は [tradeoffs.md](docs/design/tradeoffs.md) にあります。
