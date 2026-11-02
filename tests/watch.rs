use skilvella::{AppState, library::Library, model::SearchQuery, watch};
use std::{fs, path::PathBuf, time::Duration};

struct WatchTask(tokio::task::JoinHandle<()>);
impl Drop for WatchTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}

fn fixture() -> (AppState, PathBuf) {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".work/test-watch")
        .join(uuid::Uuid::new_v4().to_string());
    let source = base.join("source");
    fs::create_dir_all(source.join("demo")).unwrap();
    fs::write(source.join("demo/SKILL.md"), content("initialwatchtoken")).unwrap();
    let library = Library::open(&base.join("data")).unwrap();
    library.add_source("fixture", &source).unwrap();
    (AppState::new(library), source)
}
fn content(word: &str) -> String {
    format!(
        "---\nname: watcher-fixture\ndescription: External file event integration\n---\n{word}\n"
    )
}

async fn indexed(state: &AppState, term: &str, missing: bool) -> bool {
    let term = term.to_owned();
    state
        .run(move |library| {
            Ok(library
                .search(
                    SearchQuery {
                        q: term,
                        status: if missing {
                            "missing".into()
                        } else {
                            String::new()
                        },
                        ..Default::default()
                    },
                    false,
                )?
                .total
                == 1)
        })
        .await
        .unwrap()
}

async fn wait_for_index(state: &AppState, term: &str, missing: bool, task: &WatchTask) {
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut poll = tokio::time::interval(Duration::from_millis(100));
        loop {
            assert!(!task.0.is_finished(), "File watcher exited before indexing");
            if indexed(state, term, missing).await {
                break;
            }
            poll.tick().await;
        }
    })
    .await
    .expect("Explicit watcher rescan did not update the index");
}

async fn external_edit(state: &AppState, source: &std::path::Path, term: &str, task: &WatchTask) {
    let path = source.join("demo/SKILL.md");
    let text = content(term);
    // Registration has no public readiness hook. Repeat only the external write,
    // without explicit rescans, so success requires an actual filesystem event.
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut retry = tokio::time::interval(Duration::from_millis(200));
        loop {
            assert!(
                !task.0.is_finished(),
                "File watcher exited before the external edit"
            );
            retry.tick().await;
            fs::write(&path, &text).unwrap();
            if indexed(state, term, false).await {
                break;
            }
        }
    })
    .await
    .expect("Actual external file events did not update the index");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn external_skill_file_changes_are_automatically_indexed() {
    let (state, source) = fixture();
    let mut task = WatchTask(watch::start(state.clone()));
    fs::write(
        source.join("demo/SKILL.md"),
        content("registrationreadytoken"),
    )
    .unwrap();
    state.rescan.notify_one();
    wait_for_index(&state, "registrationreadytoken", false, &task).await;
    external_edit(&state, &source, "externalonlineeventtoken", &task).await;
    assert!(!indexed(&state, "registrationreadytoken", false).await);
    task.0.abort();
    let _ = (&mut task.0).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn startup_offline_source_is_rewatched_after_explicit_rescan() {
    let (state, source) = fixture();
    let parked = source.with_file_name("source-offline");
    fs::rename(&source, &parked).unwrap();
    let mut task = WatchTask(watch::start(state.clone()));
    state.rescan.notify_one();
    // A newly missing catalog row proves the watcher ran while the source was offline.
    wait_for_index(&state, "initialwatchtoken", true, &task).await;
    fs::rename(&parked, &source).unwrap();
    fs::write(source.join("demo/SKILL.md"), content("returnedrescantoken")).unwrap();
    state.rescan.notify_one();
    wait_for_index(&state, "returnedrescantoken", false, &task).await;
    external_edit(&state, &source, "externalreturnedeventtoken", &task).await;
    assert!(!indexed(&state, "returnedrescantoken", false).await);
    task.0.abort();
    let _ = (&mut task.0).await;
}
