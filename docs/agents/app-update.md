# アプリ更新

- アプリ更新は `src/update.rs` の検証・ロック・ヘルパー経路を通し、Libraryや利用者スキルを変更しないでください。信頼境界とToolport連携は [更新の設計](../../DESIGN.md#アプリの更新) を参照してください。
- Setup版の自動更新は `src/update/auto.rs`（更新元は `update_source` の1か所、設定は `app-update.json` でLibraryに入れない）で行います。
- 検証は [test-update-and-startup.md](test-update-and-startup.md) を参照してください。
