//! 管理フォルダーが移動・消失したときの表示と、場所の付け替え・管理から外す操作を検証する。

use reqwest::StatusCode;
use skilvella::{AppState, library::Library, model::SkillClient};
use std::{fs, path::PathBuf};

mod common;

struct Fixture {
    root: PathBuf,
    state: AppState,
    alias: String,
}

fn skill_text(name: &str) -> String {
    format!("---\nname: {name}\ndescription: Test\n---\nBody\n")
}

/// フォルダーを登録し、Claude Codeへの同期を有効にした状態を作る。
fn fixture(label: &str) -> Fixture {
    let root = common::isolated_root(label);
    fs::create_dir_all(root.join("moved-from/one")).unwrap();
    fs::write(root.join("moved-from/one/SKILL.md"), skill_text("one")).unwrap();
    fs::create_dir_all(root.join("home")).unwrap();
    // Claude Codeの設定フォルダーがあれば同期先として扱われる。
    fs::create_dir_all(root.join("home/.claude")).unwrap();
    let library = Library::open_for_user(&root.join("data"), &root.join("home"), None).unwrap();
    let (alias, _) = library.add_folder(&root.join("moved-from")).unwrap();
    library
        .set_clients_enabled(&[(SkillClient::Claude, true)])
        .unwrap();
    assert!(library.sync_summary().unwrap().last_error.is_empty());
    Fixture {
        root,
        state: AppState::new(library),
        alias,
    }
}

/// 利用者がフォルダーをエクスプローラーで移動した状態にし、監視と同じ再スキャンと同期を走らせる。
async fn move_folder(f: &Fixture) -> PathBuf {
    let to = f.root.join("moved-to");
    fs::rename(f.root.join("moved-from"), &to).unwrap();
    // 監視の再スキャンと同じ経路（`run_mut` が入口同期まで待つ）。
    f.state.run_mut(|lib| lib.scan()).await.ok();
    to
}

#[tokio::test]
async fn remove_recovers_even_when_new_location_is_already_added() {
    let f = fixture("relocate-added");
    let to = move_folder(&f).await;
    let (new_alias, _) = f.state.library.add_folder(&to).unwrap();
    assert_ne!(new_alias, f.alias);
    f.state.run_mut(|lib| lib.scan()).await.ok();
    assert!(
        f.state
            .library
            .sync_summary()
            .unwrap()
            .last_error
            .contains("Skill source unavailable")
    );

    let server = common::HttpServer::start(f.state.clone()).await;
    let client = common::no_redirect_client();
    // 同期が止まっていても主要な画面は開ける。
    for url in [
        format!("{}/", server.base),
        format!("{}/?source={}", server.base, f.alias),
        format!("{}/settings", server.base),
    ] {
        let response = client.get(&url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{url}");
    }
    let response = common::post_form(
        &client,
        &format!("{}/sources/remove", server.base),
        &[("csrf", &*f.state.csrf), ("alias", &f.alias)],
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let sources = f.state.library.sources().unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].alias, new_alias);
    let recovered = f.state.library.sync_summary().unwrap();
    assert!(recovered.last_error.is_empty(), "{}", recovered.last_error);
}

fn first_id(library: &Library) -> String {
    library.search(Default::default(), false).unwrap().skills[0]
        .id
        .clone()
}

/// 画面を取得して本文を返す（JavaScriptなしで見える初期HTML）。
async fn page(client: &reqwest::Client, url: &str) -> String {
    let response = client.get(url).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK, "{url}");
    response.text().await.unwrap()
}

#[tokio::test]
async fn unavailable_folder_is_shown_with_guidance_and_links_from_every_sync_warning() {
    let f = fixture("relocate-pages");
    let id = first_id(&f.state.library);
    move_folder(&f).await;
    assert!(f.state.library.sources().unwrap()[0].unavailable);

    let server = common::HttpServer::start(f.state.clone()).await;
    let client = common::no_redirect_client();
    let folder_url = format!("/?source={}", f.alias);

    let library = page(&client, &format!("{}/", server.base)).await;
    // サイドバーのバッジは読み上げられる説明を持ち、同期停止の警告から対象フォルダーへ案内する。
    assert!(
        library.contains("sr-only\">フォルダーを読み取れません"),
        "{library}"
    );
    assert!(library.contains("class=\"sync-unavailable\""));
    assert!(library.contains(&format!("<a href=\"{folder_url}\">")));
    // 同じ案内を設定画面と詳細画面の警告にも出す。
    for url in [
        format!("{}/settings", server.base),
        format!("{}/skills/{id}", server.base),
    ] {
        let body = page(&client, &url).await;
        assert!(body.contains("class=\"sync-unavailable\""), "{url}");
        assert!(
            body.contains(&format!("<a href=\"{folder_url}\">")),
            "{url}"
        );
    }

    // フォルダー画面は警告文を静的に置き（読み込み時に割り込み読み上げさせない）、
    // 場所の変更フォームを開いた状態で出す。
    let folder = page(&client, &format!("{}{folder_url}", server.base)).await;
    let start = folder.find("id=\"folder-unavailable\"").unwrap();
    let notice = &folder[folder[..start].rfind('<').unwrap()..start + 300];
    assert!(notice.contains("notice warning") && !notice.contains("role="));
    let form = folder.find("action=\"/sources/location\"").unwrap();
    let details = folder[..form].rfind("<details").unwrap();
    assert!(folder[details..form].contains("folder-secondary\" open"));
    let form_end = form + folder[form..].find("</form>").unwrap();
    let form_html = &folder[form..form_end];
    assert!(form_html.contains("data-folder-form") && form_html.contains("data-folder-browse"));
    assert!(form_html.contains("aria-describedby=\"relocate-help\""));
    assert!(folder.contains("<label for=\"relocate-path\">"));
    assert!(!form_html.contains("data-save-on-change"));
    // 追加した文言は他の言語でも表示される。
    let english = client
        .get(format!("{}{folder_url}", server.base))
        .header("accept-language", "en")
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(english.contains("Change folder location"));
    assert!(english.contains("This folder cannot be read."));
    assert!(english.contains("Some folders cannot be read."));
}

#[tokio::test]
async fn relocation_keeps_ids_and_management_data_and_recovers_sync() {
    let f = fixture("relocate-keep");
    let library = &f.state.library;
    let id = first_id(library);
    library
        .set_metadata(
            &id,
            skilvella::model::SkillMetadata {
                tags: vec!["keep".into()],
                category: "cat".into(),
                published: false,
            },
        )
        .unwrap();
    library.set_codex_policy(&id, false, true).unwrap();
    let revision = library.detail(&id, false).unwrap().revision;
    library
        .save_content(&id, &revision, &skill_text("one-edited"))
        .unwrap();
    let history = library.detail(&id, false).unwrap().history.len();
    assert!(history > 0);
    let to = move_folder(&f).await;
    assert!(
        library
            .sync_summary()
            .unwrap()
            .last_error
            .contains("Skill source unavailable")
    );

    let server = common::HttpServer::start(f.state.clone()).await;
    let client = common::no_redirect_client();
    // CSRFのない要求は拒否し、何も変えない。
    let rejected = common::post_form(
        &client,
        &format!("{}/sources/location", server.base),
        &[
            ("csrf", "wrong"),
            ("alias", &f.alias),
            ("path", &to.to_string_lossy()),
        ],
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::FORBIDDEN);
    assert!(library.sources().unwrap()[0].unavailable);

    let response = common::post_form(
        &client,
        &format!("{}/sources/location", server.base),
        &[
            ("csrf", &*f.state.csrf),
            ("alias", &f.alias),
            ("path", &to.to_string_lossy()),
        ],
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response.headers()["location"].to_str().unwrap().to_owned();
    assert!(location.contains("folder-relocated"), "{location}");

    let sources = library.sources().unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].alias, f.alias);
    assert!(!sources[0].unavailable);
    assert_eq!(PathBuf::from(&sources[0].path), to.canonicalize().unwrap());
    // 識別子・管理情報・使い方・履歴を引き継ぎ、同期も復旧する。
    assert_eq!(first_id(library), id);
    let detail = library.detail(&id, false).unwrap();
    assert!(!detail.summary.published && !detail.summary.missing);
    assert_eq!(detail.summary.tags, vec!["keep"]);
    assert_eq!(detail.history.len(), history);
    assert!(library.codex_policy(&id).unwrap().command);
    let sync = library.sync_summary().unwrap();
    assert!(sync.last_error.is_empty(), "{}", sync.last_error);
    // 実ファイルは動かしていない。
    assert!(to.join("one/SKILL.md").is_file());
}

#[tokio::test]
async fn relocation_rejects_conflicts_and_keeps_the_previous_registration() {
    let f = fixture("relocate-reject");
    let library = &f.state.library;
    fs::create_dir_all(f.root.join("other/two")).unwrap();
    fs::write(f.root.join("other/two/SKILL.md"), skill_text("two")).unwrap();
    let (other, _) = library.add_folder(&f.root.join("other")).unwrap();
    fs::create_dir_all(f.root.join("other/nested")).unwrap();

    let server = common::HttpServer::start(f.state.clone()).await;
    let client = common::no_redirect_client();
    let post = |path: String| {
        let client = client.clone();
        let base = server.base.clone();
        let csrf = f.state.csrf.to_string();
        let alias = f.alias.clone();
        async move {
            common::post_form(
                &client,
                &format!("{base}/sources/location"),
                &[("csrf", &csrf), ("alias", &alias), ("path", &path)],
            )
            .await
        }
    };
    // 別の有効なフォルダーと同じ場所: 先に管理から外すよう案内し、入力値を保持する。
    let same = post(f.root.join("other").to_string_lossy().into_owned()).await;
    assert_eq!(same.status(), StatusCode::OK);
    let body = same.text().await.unwrap();
    assert!(
        body.contains("先に「other」を管理から外してください"),
        "{body}"
    );
    assert!(body.contains("notice warning"));
    assert!(body.contains("other"));
    // 別の有効なフォルダーとの親子関係。
    let nested = post(f.root.join("other/nested").to_string_lossy().into_owned()).await;
    assert!(nested.text().await.unwrap().contains("notice warning"));
    // 存在しない場所・相対パス・空。
    for path in [
        f.root.join("nowhere").to_string_lossy().into_owned(),
        "relative/path".to_owned(),
        String::new(),
    ] {
        let response = post(path.clone()).await;
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        assert!(response.text().await.unwrap().contains("notice warning"));
    }
    // データフォルダーと重なる場所。
    let data = post(f.root.join("data").to_string_lossy().into_owned()).await;
    assert!(data.text().await.unwrap().contains("notice warning"));
    // どれも登録を変えない。
    let sources = library.sources().unwrap();
    assert_eq!(sources.len(), 2);
    let own = sources.iter().find(|s| s.alias == f.alias).unwrap();
    assert_eq!(
        PathBuf::from(&own.path),
        f.root.join("moved-from").canonicalize().unwrap()
    );
    assert!(sources.iter().any(|s| s.alias == other));
    // 管理から外した別フォルダーと同じ場所へは、その行を触らずに付け替えられる。
    library.remove_source(&other).unwrap();
    let report = library
        .relocate_source(&f.alias, &f.root.join("other"))
        .unwrap();
    assert!(report.indexed >= 1);
}

#[tokio::test]
async fn relocating_the_default_folder_updates_the_directory_settings() {
    let root = common::isolated_root("relocate-default");
    let home = root.join("home");
    let native = home.join(".agents/skills");
    fs::create_dir_all(native.join("example")).unwrap();
    fs::write(native.join("example/SKILL.md"), skill_text("example")).unwrap();
    let state = AppState::new(Library::open(&root.join("data")).unwrap());
    state.library.initialize_directory(&home, None).unwrap();
    let alias = state.library.directory_settings().unwrap().source_alias;
    let id = first_id(&state.library);
    let moved = root.join("moved-default");
    fs::rename(&native, &moved).unwrap();
    state.run_mut(|lib| lib.scan()).await.ok();

    let server = common::HttpServer::start(state.clone()).await;
    let client = common::no_redirect_client();
    let before = page(&client, &format!("{}/?source={alias}", server.base)).await;
    assert!(before.contains("id=\"folder-unavailable\""));
    // 既定フォルダーでも、移動の追従は識別子を保つ場所の変更で行う（切替フォームは別の用途）。
    assert!(before.contains("action=\"/sources/location\""));
    assert!(before.contains("action=\"/sources/directory\""));

    let response = common::post_form(
        &client,
        &format!("{}/sources/location", server.base),
        &[
            ("csrf", &*state.csrf),
            ("alias", &alias),
            ("path", &moved.to_string_lossy()),
        ],
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let settings = state.library.directory_settings().unwrap();
    assert_eq!(settings.source_alias, alias);
    assert_eq!(settings.mode, "custom");
    let canonical = moved.canonicalize().unwrap();
    assert_eq!(PathBuf::from(&settings.path), canonical);
    assert_eq!(PathBuf::from(&settings.custom_path), canonical);
    assert_eq!(first_id(&state.library), id);
    assert!(!state.library.sources().unwrap()[0].unavailable);
}

/// 再スキャンを呼ばず、サービスの定期同期（`sync_codex_entries`）だけで印が付き、
/// 戻って同期が成功すれば次の再スキャンを待たずに消えること。
#[tokio::test]
async fn periodic_sync_alone_marks_and_clears_the_unavailable_folder() {
    let f = fixture("relocate-sync-only");
    let library = &f.state.library;
    let from = f.root.join("moved-from");
    let parked = f.root.join("parked");
    fs::rename(&from, &parked).unwrap();
    assert!(!library.sources().unwrap()[0].unavailable);

    assert!(library.sync_codex_entries().is_err());
    let error = library.sync_summary().unwrap().last_error;
    assert!(error.contains("Skill source unavailable"), "{error}");
    assert!(!error.contains(r"\\?\"), "{error}");
    assert!(library.sources().unwrap()[0].unavailable);

    let server = common::HttpServer::start(f.state.clone()).await;
    let client = common::no_redirect_client();
    let folder_url = format!("/?source={}", f.alias);
    let body = page(&client, &format!("{}/", server.base)).await;
    assert!(body.contains("class=\"sync-unavailable\""));
    assert!(body.contains(&format!("<a href=\"{folder_url}\">")));
    assert!(body.contains("sr-only\">フォルダーを読み取れません"));
    // 読み取りエラー一覧にも、フォルダー自体が読めない場合の案内を出す。
    let errors = page(&client, &format!("{}/?status=error", server.base)).await;
    assert!(errors.contains(&format!("<a href=\"{folder_url}\">")));

    // 戻して同期が成功すれば、再スキャンなしで印とリンクが消える。
    fs::rename(&parked, &from).unwrap();
    library.sync_codex_entries().unwrap();
    assert!(library.sync_summary().unwrap().last_error.is_empty());
    assert!(!library.sources().unwrap()[0].unavailable);
    let body = page(&client, &format!("{}/", server.base)).await;
    assert!(!body.contains("class=\"sync-unavailable\""));
}

/// 登録と場所の変更は同じ判定を使い、存在しない・解除済みの識別子は拒否する。
#[tokio::test]
async fn relocation_and_registration_share_conflict_rules() {
    let f = fixture("relocate-shared");
    let library = &f.state.library;
    fs::create_dir_all(f.root.join("other/two")).unwrap();
    fs::write(f.root.join("other/two/SKILL.md"), skill_text("two")).unwrap();
    fs::create_dir_all(f.root.join("other/inner")).unwrap();
    let (other, _) = library.add_folder(&f.root.join("other")).unwrap();

    let kind = |error: anyhow::Error| match error.downcast_ref::<skilvella::model::FolderError>() {
        Some(skilvella::model::FolderError::AlreadyRegistered { .. }) => "same-register",
        Some(skilvella::model::FolderError::RelocationTargetManaged { .. }) => "same-relocate",
        Some(skilvella::model::FolderError::Overlaps { .. }) => "overlap",
        _ => "other",
    };
    // 同じ場所: どちらも管理中として拒否する（文言だけが操作ごとに違う）。
    let same = f.root.join("other");
    assert_eq!(
        kind(library.add_folder(&same).unwrap_err()),
        "same-register"
    );
    assert_eq!(
        kind(library.relocate_source(&f.alias, &same).unwrap_err()),
        "same-relocate"
    );
    // 親子: 同じ種類のエラー。
    let inner = f.root.join("other/inner");
    assert_eq!(kind(library.add_folder(&inner).unwrap_err()), "overlap");
    assert_eq!(
        kind(library.relocate_source(&f.alias, &inner).unwrap_err()),
        "overlap"
    );
    // 自分自身の旧行は除外する（自分の場所への付け替えは再走査として成功する）。
    library
        .relocate_source(&f.alias, &f.root.join("moved-from"))
        .unwrap();
    // 存在しない・解除済みの識別子は拒否する。
    assert!(library.relocate_source("nope", &same).is_err());
    library.remove_source(&other).unwrap();
    assert!(
        library
            .relocate_source(&other, &f.root.join("moved-from"))
            .is_err()
    );
    // 読めない保存パス（移動済み）は保存値のまま比べ、同じ場所は同じ判定になる。
    fs::rename(f.root.join("moved-from"), f.root.join("parked")).unwrap();
    let missing = f.root.join("moved-from");
    fs::create_dir_all(&missing).unwrap();
    assert!(library.relocate_source(&f.alias, &missing).is_ok());
}
