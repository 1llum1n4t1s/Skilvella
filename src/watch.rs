use crate::AppState;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::{collections::HashSet, path::PathBuf, time::Duration};

/// ファイル通知をまとめ、SQLite/ファイル走査はブロッキング専用スレッドへ送る。
pub fn start(state: AppState) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let watcher = RecommendedWatcher::new(
            move |event: notify::Result<notify::Event>| match event {
                Ok(event) if !matches!(event.kind, notify::EventKind::Access(_)) => {
                    let _ = tx.try_send(());
                }
                Err(error) => {
                    eprintln!("File watcher: {error}. Use the rescan button if needed.");
                    let _ = tx.try_send(());
                }
                _ => {}
            },
            notify::Config::default(),
        );
        let mut watcher = match watcher {
            Ok(watcher) => watcher,
            Err(error) => {
                eprintln!("File watcher unavailable: {error}. Use the rescan button.");
                return;
            }
        };
        let mut watched = HashSet::<PathBuf>::new();
        let mut scan_pending = false;
        loop {
            match state.run(|lib| lib.sources()).await {
                Ok(sources) => {
                    let desired: HashSet<_> =
                        sources.into_iter().map(|s| PathBuf::from(s.path)).collect();
                    for path in watched.difference(&desired) {
                        let _ = watcher.unwatch(path);
                    }
                    let mut next = watched
                        .intersection(&desired)
                        .cloned()
                        .collect::<HashSet<_>>();
                    for path in desired.difference(&watched) {
                        match watcher.watch(path, RecursiveMode::Recursive) {
                            Ok(()) => {
                                next.insert(path.clone());
                            }
                            Err(error) => eprintln!(
                                "Cannot watch {}: {error}. Use the rescan button.",
                                path.display()
                            ),
                        }
                    }
                    watched = next;
                }
                Err(error) => eprintln!("Cannot refresh watched sources: {error}"),
            }
            if scan_pending {
                // 再登録後に走査する。走査中の変更通知は次の走査へ残す。
                tokio::time::sleep(Duration::from_millis(450)).await;
                while rx.try_recv().is_ok() {}
                if let Err(error) = state.run(|lib| lib.scan()).await {
                    eprintln!("Automatic scan failed: {error}");
                }
            }
            tokio::select! {
                _ = state.rescan.notified() => {
                    // オフラインから復帰した同じパスも明示再スキャンで再登録する。
                    for path in watched.drain() { let _ = watcher.unwatch(&path); }
                },
                event = rx.recv() => { if event.is_none() { return; } },
            }
            scan_pending = true;
        }
    })
}
