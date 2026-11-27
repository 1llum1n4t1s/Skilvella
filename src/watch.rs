use crate::{AppState, locale};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    collections::HashSet,
    path::{Component, Path, PathBuf},
    sync::{Arc, RwLock},
    time::Duration,
};

/// 監視ルートの下で、探索が辿らないフォルダー（`.git` など）の内側にある通知か。
/// `.git` 自体のような最後の成分は内側とみなさない。
fn inside_skipped(roots: &[PathBuf], path: &Path) -> bool {
    for root in roots {
        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        if relative.parent().is_some_and(|parent| {
            parent.components().any(|part| {
                matches!(part, Component::Normal(name) if crate::library::skipped(&name.to_string_lossy()))
            })
        }) {
            return true;
        }
    }
    false
}

/// ファイル通知をまとめ、SQLite/ファイル走査はブロッキング専用スレッドへ送る。
pub fn start(state: AppState) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let roots: Arc<RwLock<Vec<PathBuf>>> = Arc::new(RwLock::new(Vec::new()));
        let callback_roots = roots.clone();
        let watcher = RecommendedWatcher::new(
            move |event: notify::Result<notify::Event>| match event {
                Ok(event) if !matches!(event.kind, notify::EventKind::Access(_)) => {
                    let ignorable = !event.need_rescan()
                        && !event.paths.is_empty()
                        && callback_roots.read().is_ok_and(|roots| {
                            event.paths.iter().all(|path| inside_skipped(&roots, path))
                        });
                    if !ignorable {
                        let _ = tx.try_send(());
                    }
                }
                Err(error) => {
                    eprintln!(
                        "{}",
                        locale::format(
                            "ファイル監視でエラーが発生しました: {error}。必要なら再スキャンを使ってください。",
                            &[("error", &error.to_string())]
                        )
                    );
                    let _ = tx.try_send(());
                }
                _ => {}
            },
            notify::Config::default(),
        );
        let mut watcher = match watcher {
            Ok(watcher) => watcher,
            Err(error) => {
                eprintln!(
                    "{}",
                    locale::format(
                        "ファイル監視を利用できません: {error}。再スキャンを使ってください。",
                        &[("error", &error.to_string())]
                    )
                );
                return;
            }
        };
        let mut watched = HashSet::<PathBuf>::new();
        let mut scan_pending = false;
        loop {
            match state.run(|lib| lib.source_rows()).await {
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
                                "{}",
                                locale::format(
                                    "{path} を監視できません: {error}。再スキャンを使ってください。",
                                    &[
                                        ("path", &path.display().to_string()),
                                        ("error", &error.to_string())
                                    ]
                                )
                            ),
                        }
                    }
                    watched = next;
                    if let Ok(mut roots) = roots.write() {
                        *roots = watched.iter().cloned().collect();
                    }
                }
                Err(error) => eprintln!(
                    "{}",
                    locale::format(
                        "監視対象のソースを更新できません: {error}",
                        &[("error", &error.to_string())]
                    )
                ),
            }
            if scan_pending {
                // 再登録後に走査する。走査中の変更通知は次の走査へ残す。
                tokio::time::sleep(Duration::from_millis(450)).await;
                while rx.try_recv().is_ok() {}
                if let Err(error) = state.run_mut(|lib| lib.scan()).await {
                    eprintln!(
                        "{}",
                        locale::format(
                            "自動スキャンに失敗しました: {error}",
                            &[("error", &error.to_string())]
                        )
                    );
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
