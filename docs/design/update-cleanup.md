# 更新後の一時物の清掃

[app-updates.md](app-updates.md) から分割した詳細です。

更新後に残る自分の一時物は、起動ごとに1回、専用スレッド（`src/update/cleanup.rs`）で片付けます。対象は、実行ファイルと同じフォルダーの更新ステージ `.skilvella-update-<UUID>`（小文字・ハイフン区切りの標準形式のUUID）と、データフォルダーの自動更新設定の保存途中の一時ファイル `.app-update.json.<UUID>.tmp` だけです。次の条件をすべて満たすものだけを消します。

1. ステージは通常のディレクトリで、直下が更新処理の作る既知の名前（`new-binary`・ヘルパー複製・`plan.json`・`update.log`・`ready`/`commit`/`cancel`/`complete`・`previous-binary`・`managed-version`・`failed-version`・`cache-<UUID>.json`）だけです。未知の名前やリンク・reparse pointを1つでも含むものは触れません。
2. ステージ自身と直下の最終更新が24時間より古いこと。未来の時刻は新しいとみなします。更新の途中や復旧用の `previous-binary` を使う場面を避けるためです。
3. 更新と同じOSファイルロック（`.skilvella-update.lock`）を取得できること。取れなければ更新の進行中として何も消しません。候補がなければロックファイルも作りません。
4. 現在のプロセスの実行ファイルを含むステージ（ヘルパーとして動作中）は消しません。Windowsで稼働中のヘルパー複製を消せないステージは、半端に壊さず丸ごと残します。

現在の版の実体、固定名で再利用される `.skilvella-update.lock`、利用者のスキル・Library、スキルフォルダー内の `.skilvella-*.tmp` には触れません。削除はOSの通常の削除で行い、失敗は標準エラーに英語で残すだけで本体を止めず、次回の起動で再試行します。Setup版（Velopack）も同じ処理を通りますが、ステージを作らないため候補が見つからず何もしません。
