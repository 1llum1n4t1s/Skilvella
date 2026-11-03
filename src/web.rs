use crate::{AppState, model::*};
use askama::Template;
use axum::{
    Router,
    extract::{Form, Path, Query, State},
    http::{StatusCode, header},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use serde::Deserialize;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(index))
        .route("/sources", get(sources))
        .route("/settings", get(settings))
        .route("/settings/updates/check", post(check_update))
        .route("/settings/updates/install", post(install_update))
        .route("/sources/add", post(add_source))
        .route("/sources/migrate", post(migrate_source))
        .route("/sources/remove", post(remove_source))
        .route("/sources/directory", post(set_directory))
        .route("/scan", post(scan))
        .route("/bulk", post(bulk))
        .route("/skills/new", get(new_skill))
        .route("/skills/create", post(create))
        .route("/skills/{id}", get(detail))
        .route("/skills/{id}/content", post(save))
        .route("/skills/{id}/metadata", post(metadata))
        .route("/skills/{id}/restore", post(restore))
        .route("/preview", post(preview))
        .route("/assets/{name}", get(asset))
        .route("/favicon.ico", get(favicon))
}
#[derive(Template)]
#[template(path = "settings.html")]
struct SettingsPage {
    csrf: String,
    version: &'static str,
}
async fn settings(State(state): State<AppState>) -> Response {
    render(SettingsPage {
        csrf: state.csrf.to_string(),
        version: env!("CARGO_PKG_VERSION"),
    })
}
async fn check_update(State(state): State<AppState>, Form(form): Form<Token>) -> Response {
    if let Some(response) = csrf_error(&state, &form.csrf) {
        return response;
    }
    match tokio::task::spawn_blocking(crate::update::check).await {
        Ok(Ok(info)) => axum::Json(info).into_response(),
        Ok(Err(error)) => update_failure(error.to_string()),
        Err(error) => update_failure(error.to_string()),
    }
}
fn update_failure(message: String) -> Response {
    (
        StatusCode::BAD_GATEWAY,
        axum::Json(serde_json::json!({"error": message})),
    )
        .into_response()
}
async fn install_update(State(state): State<AppState>, Form(form): Form<Token>) -> Response {
    if let Some(response) = csrf_error(&state, &form.csrf) {
        return response;
    }
    let result = tokio::task::spawn_blocking(|| {
        let prepared = crate::update::prepare_install()?;
        crate::update::spawn_installer(prepared)
    })
    .await;
    match result {
        Ok(Ok(())) => {
            state.shutdown.notify_one();
            axum::Json(serde_json::json!({"restarting": true})).into_response()
        }
        Ok(Err(error)) => update_failure(error.to_string()),
        Err(error) => update_failure(error.to_string()),
    }
}
fn render(t: impl Template) -> Response {
    match t.render() {
        Ok(s) => Html(s).into_response(),
        Err(_) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "画面を表示できませんでした。再読み込みしてください。",
        ),
    }
}
#[derive(Template)]
#[template(path = "error.html")]
struct ErrorPage<'a> {
    message: &'a str,
}
fn error(status: StatusCode, message: &str) -> Response {
    (
        status,
        Html(
            ErrorPage { message }
                .render()
                .unwrap_or_else(|_| "エラー".into()),
        ),
    )
        .into_response()
}
fn csrf_error(state: &AppState, token: &str) -> Option<Response> {
    if token == state.csrf.as_ref() {
        None
    } else {
        Some(error(
            StatusCode::FORBIDDEN,
            "操作用トークンが一致しません。画面を再読み込みしてからやり直してください。",
        ))
    }
}
fn failure(e: anyhow::Error) -> Response {
    error(
        StatusCode::BAD_REQUEST,
        &format!("操作できませんでした: {e:#}"),
    )
}
fn encoded(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
fn redirect(path: &str, message: &str) -> Response {
    Redirect::to(&format!("{path}?message={}", encoded(message))).into_response()
}
fn tags(s: &str) -> Vec<String> {
    s.split([',', '、', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct Notice {
    message: String,
}
#[derive(Template)]
#[template(path = "index.html")]
struct IndexPage {
    csrf: String,
    query: SearchQuery,
    results: SearchResults,
    sources: Vec<Source>,
    message: String,
    previous: String,
    next: String,
}
async fn index(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
    Query(notice): Query<Notice>,
) -> Response {
    let mut query = query;
    query.limit = 50;
    let q = query.clone();
    match state
        .run(move |lib| Ok((lib.search(q, false)?, lib.sources()?)))
        .await
    {
        Ok((results, sources)) => {
            let page_url = |offset| {
                format!(
                    "/?q={}&source={}&tag={}&category={}&status={}&offset={offset}",
                    encoded(&query.q),
                    encoded(&query.source),
                    encoded(&query.tag),
                    encoded(&query.category),
                    encoded(&query.status)
                )
            };
            let previous = if query.offset > 0 {
                page_url(query.offset.saturating_sub(50))
            } else {
                String::new()
            };
            let next = if query.offset.saturating_add(50) < results.total {
                page_url(query.offset.saturating_add(50))
            } else {
                String::new()
            };
            render(IndexPage {
                csrf: state.csrf.to_string(),
                query,
                results,
                sources,
                message: notice.message,
                previous,
                next,
            })
        }
        Err(e) => failure(e),
    }
}
#[derive(Template)]
#[template(path = "sources.html")]
struct SourcesPage {
    csrf: String,
    sources: Vec<Source>,
    message: String,
    alias: String,
    path: String,
    directory: DirectorySettings,
    directory_path: String,
    directory_editing: bool,
    dedicated_root: String,
    migration_alias: String,
    migration_destination: String,
}

impl SourcesPage {
    fn has_directory_source(&self) -> bool {
        self.sources
            .iter()
            .any(|source| source.alias == self.directory.source_alias)
    }

    fn display_path(&self, path: impl AsRef<str>) -> String {
        let path = path.as_ref();
        #[cfg(windows)]
        {
            if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
                return format!(r"\\{unc}");
            }
            if let Some(drive) = path.strip_prefix(r"\\?\") {
                let bytes = drive.as_bytes();
                if bytes.len() >= 3
                    && bytes[0].is_ascii_alphabetic()
                    && bytes[1] == b':'
                    && bytes[2] == b'\\'
                {
                    return drive.to_owned();
                }
            }
        }
        path.to_owned()
    }

    fn destination_for(&self, source: &Source) -> String {
        if self.migration_alias == source.alias {
            self.migration_destination.clone()
        } else {
            std::path::PathBuf::from(&self.dedicated_root)
                .join(&source.alias)
                .to_string_lossy()
                .into_owned()
        }
    }
}

async fn sources(State(state): State<AppState>, Query(n): Query<Notice>) -> Response {
    source_page(&state, n.message, String::new(), String::new()).await
}
async fn source_page(state: &AppState, message: String, alias: String, path: String) -> Response {
    source_page_with_directory(state, message, alias, path, None, None).await
}
async fn source_page_with_directory(
    state: &AppState,
    message: String,
    alias: String,
    path: String,
    draft: Option<(String, String)>,
    migration: Option<(String, String)>,
) -> Response {
    match state
        .run(|lib| {
            Ok((
                lib.sources()?,
                lib.directory_settings()?,
                lib.dedicated_directory()?,
            ))
        })
        .await
    {
        Ok((sources, directory, dedicated_root)) => {
            let directory_editing = draft.is_some();
            let directory_path = draft.map(|(_, path)| path).unwrap_or_else(|| {
                if directory.path.is_empty() {
                    directory.custom_path.clone()
                } else {
                    directory.path.clone()
                }
            });
            let (migration_alias, migration_destination) = migration.unwrap_or_default();
            render(SourcesPage {
                csrf: state.csrf.to_string(),
                sources,
                message,
                alias,
                path,
                directory,
                directory_path,
                directory_editing,
                dedicated_root: dedicated_root.to_string_lossy().into_owned(),
                migration_alias,
                migration_destination,
            })
        }
        Err(e) => failure(e),
    }
}
#[derive(Deserialize)]
struct DirectoryForm {
    csrf: String,
    mode: String,
    #[serde(default)]
    path: String,
}
async fn set_directory(State(state): State<AppState>, Form(form): Form<DirectoryForm>) -> Response {
    if let Some(response) = csrf_error(&state, &form.csrf) {
        return response;
    }
    let mode = form.mode.clone();
    let path = form.path.clone();
    match state
        .run(move |library| library.set_directory(&mode, &path))
        .await
    {
        Ok(report) => {
            state.rescan.notify_one();
            redirect(
                "/sources",
                &format!(
                    "管理対象フォルダーの設定を保存しました。{}",
                    scan_message(report)
                ),
            )
        }
        Err(error) => {
            source_page_with_directory(
                &state,
                format!("設定を変更できませんでした: {error:#}。以前の設定は保持されています。"),
                String::new(),
                String::new(),
                Some((form.mode, form.path)),
                None,
            )
            .await
        }
    }
}
#[derive(Deserialize)]
struct SourceForm {
    csrf: String,
    alias: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    action: String,
}
async fn add_source(State(state): State<AppState>, Form(f): Form<SourceForm>) -> Response {
    if let Some(r) = csrf_error(&state, &f.csrf) {
        return r;
    }
    let alias = f.alias.clone();
    let path = f.path.clone();
    if !matches!(f.action.as_str(), "" | "register" | "move") {
        return error(StatusCode::BAD_REQUEST, "登録方法が不正です。");
    }
    let move_source = f.action == "move";
    match state
        .run(move |lib| {
            let report = lib.add_source(&alias, std::path::Path::new(&path))?;
            let migration = move_source.then(|| {
                let destination = lib.dedicated_directory()?.join(&alias);
                lib.migrate_source(&alias, &destination)
            });
            Ok((report, migration))
        })
        .await
    {
        Ok((r, migration)) => {
            state.rescan.notify_one();
            match migration {
                Some(Ok(report)) => redirect("/sources", &format!("ソースフォルダーを専用フォルダーへ移動し、有効なスキルをMCP公開しました。Codexを再起動してください。{}", scan_message(report))),
                Some(Err(e)) => source_page(&state, format!("登録後の移動を完了できませんでした: {e:#}。現在の登録先・元フォルダー・移動先・公開状態を確認してください。"), f.alias, f.path).await,
                None => redirect("/sources", &scan_message(r)),
            }
        }
        Err(e) => {
            source_page(
                &state,
                format!("登録できませんでした: {e:#}"),
                f.alias,
                f.path,
            )
            .await
        }
    }
}
#[derive(Deserialize)]
struct MigrationForm {
    csrf: String,
    alias: String,
    destination: String,
}
async fn migrate_source(State(state): State<AppState>, Form(f): Form<MigrationForm>) -> Response {
    if let Some(r) = csrf_error(&state, &f.csrf) {
        return r;
    }
    let alias = f.alias.clone();
    let destination = f.destination.clone();
    let result = state
        .run(move |lib| lib.migrate_source(&alias, std::path::Path::new(&destination)))
        .await;
    state.rescan.notify_one();
    match result {
        Ok(report) => redirect("/sources", &format!("ソースフォルダーを移動し、有効なスキルをMCP公開しました。Codexを再起動してください。{}", scan_message(report))),
        Err(e) => source_page_with_directory(
            &state,
            format!("移動を完了できませんでした: {e:#}。現在の登録先・元フォルダー・移動先・公開状態を確認してください。"),
            String::new(), String::new(), None, Some((f.alias, f.destination)),
        ).await,
    }
}
async fn remove_source(State(state): State<AppState>, Form(f): Form<SourceForm>) -> Response {
    if let Some(r) = csrf_error(&state, &f.csrf) {
        return r;
    }
    match state.run(move |lib| lib.remove_source(&f.alias)).await {
        Ok(_) => {
            state.rescan.notify_one();
            redirect(
                "/sources",
                "登録を解除しました。元のフォルダーとファイルは保持されています。",
            )
        }
        Err(e) => failure(e),
    }
}
#[derive(Deserialize)]
struct Token {
    csrf: String,
}
fn scan_message(r: ScanReport) -> String {
    format!(
        "スキャン完了: {}件を登録、エラー{}件、見つからないスキル{}件。",
        r.indexed, r.errors, r.missing
    )
}
async fn scan(State(state): State<AppState>, Form(f): Form<Token>) -> Response {
    if let Some(r) = csrf_error(&state, &f.csrf) {
        return r;
    }
    match state.run(|lib| lib.scan()).await {
        Ok(r) => {
            state.rescan.notify_one();
            redirect("/", &scan_message(r))
        }
        Err(e) => failure(e),
    }
}
#[derive(Deserialize)]
struct BulkForm {
    csrf: String,
    #[serde(default)]
    selected: String,
    #[serde(default)]
    published: String,
    #[serde(default)]
    tags: String,
}
async fn bulk(State(state): State<AppState>, Form(f): Form<BulkForm>) -> Response {
    if let Some(r) = csrf_error(&state, &f.csrf) {
        return r;
    }
    let ids: Vec<String> = f
        .selected
        .split(',')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    if ids.is_empty() {
        return error(
            StatusCode::BAD_REQUEST,
            "スキルを1件以上選んでください。ブラウザーでJavaScriptを有効にしてください。",
        );
    }
    let published = match f.published.as_str() {
        "true" => Some(true),
        "false" => Some(false),
        "" => None,
        _ => return error(StatusCode::BAD_REQUEST, "公開状態が不正です。"),
    };
    let add_tags = tags(&f.tags);
    if published.is_none() && add_tags.is_empty() {
        return error(
            StatusCode::BAD_REQUEST,
            "変更する公開状態または追加タグを指定してください。",
        );
    }
    match state
        .run(move |lib| lib.bulk_metadata(&ids, published, &add_tags))
        .await
    {
        Ok(_) => redirect("/", "選択したスキルを更新しました。"),
        Err(e) => failure(e),
    }
}
#[derive(Template)]
#[template(path = "detail.html")]
struct DetailPage {
    csrf: String,
    detail: SkillDetail,
    preview: String,
    tag_text: String,
    message: String,
    dirty: bool,
}
async fn detail(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(n): Query<Notice>,
) -> Response {
    detail_page(&state, id, n.message, None).await
}
#[derive(Template)]
#[template(path = "draft.html")]
struct DraftPage {
    csrf: String,
    id: String,
    content: String,
    revision: String,
    message: String,
}
async fn detail_page(
    state: &AppState,
    id: String,
    message: String,
    draft: Option<(String, String)>,
) -> Response {
    let key = id.clone();
    match state.run(move |lib| lib.detail(&key, false)).await {
        Ok(mut detail) => {
            let dirty = draft.is_some();
            if let Some((content, revision)) = draft {
                detail.content = content;
                detail.revision = revision;
            }
            let preview = markdown(&detail.content);
            let tag_text = detail.summary.tags.join(", ");
            render(DetailPage {
                csrf: state.csrf.to_string(),
                detail,
                preview,
                tag_text,
                message,
                dirty,
            })
        }
        Err(e) => match draft {
            Some((content, revision)) => render(DraftPage {
                csrf: state.csrf.to_string(),
                id,
                content,
                revision,
                message: format!("{message} 最新内容の取得にも失敗しました: {e:#}"),
            }),
            None => failure(e),
        },
    }
}
#[derive(Deserialize)]
struct ContentForm {
    csrf: String,
    content: String,
    revision: String,
}
async fn save(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(f): Form<ContentForm>,
) -> Response {
    if let Some(r) = csrf_error(&state, &f.csrf) {
        return r;
    }
    let key = id.clone();
    let content = f.content.clone();
    let revision = f.revision.clone();
    match state.run(move|lib|lib.save_content(&key,&revision,&content)).await {Ok(_)=>redirect(&format!("/skills/{}",encoded(&id)),"SKILL.md を保存しました。"),Err(e)=>detail_page(&state,id,format!("保存できませんでした: {e:#}。下書きは下の編集欄に保持しています。競合時は下書きをコピーしてから「最新版を開く」で最新内容へ統合してください。"),Some((f.content,f.revision))).await}
}
#[derive(Deserialize)]
struct MetadataForm {
    csrf: String,
    #[serde(default)]
    tags: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    published: Option<String>,
}
async fn metadata(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(f): Form<MetadataForm>,
) -> Response {
    if let Some(r) = csrf_error(&state, &f.csrf) {
        return r;
    }
    let key = id.clone();
    let value = SkillMetadata {
        tags: tags(&f.tags),
        category: f.category,
        published: f.published.as_deref() == Some("true"),
    };
    match state.run(move |lib| lib.set_metadata(&key, value)).await {
        Ok(_) => redirect(
            &format!("/skills/{}", encoded(&id)),
            "管理情報を保存しました。",
        ),
        Err(e) => failure(e),
    }
}
#[derive(Deserialize)]
struct RestoreForm {
    csrf: String,
    history_id: i64,
    revision: String,
}
async fn restore(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(f): Form<RestoreForm>,
) -> Response {
    if let Some(r) = csrf_error(&state, &f.csrf) {
        return r;
    }
    let key = id.clone();
    match state
        .run(move |lib| lib.restore(&key, f.history_id, &f.revision))
        .await
    {
        Ok(_) => redirect(
            &format!("/skills/{}", encoded(&id)),
            "履歴から復元しました。",
        ),
        Err(e) => detail_page(&state, id, format!("復元できませんでした: {e:#}"), None).await,
    }
}
#[derive(Template)]
#[template(path = "new.html")]
struct NewPage {
    csrf: String,
    sources: Vec<Source>,
    source: String,
    relative_dir: String,
    content: String,
    message: String,
}
async fn new_skill(State(state): State<AppState>) -> Response {
    new_page(
        &state,
        String::new(),
        String::new(),
        "---\nname: new-skill\ndescription: スキルの用途\n---\n\n# 手順\n\n".into(),
        String::new(),
    )
    .await
}
async fn new_page(
    state: &AppState,
    source: String,
    relative_dir: String,
    content: String,
    message: String,
) -> Response {
    match state.run(|lib| lib.sources()).await {
        Ok(sources) => render(NewPage {
            csrf: state.csrf.to_string(),
            sources,
            source,
            relative_dir,
            content,
            message,
        }),
        Err(e) => failure(e),
    }
}
#[derive(Deserialize)]
struct CreateForm {
    csrf: String,
    source: String,
    relative_dir: String,
    content: String,
}
async fn create(State(state): State<AppState>, Form(f): Form<CreateForm>) -> Response {
    if let Some(r) = csrf_error(&state, &f.csrf) {
        return r;
    }
    let source = f.source.clone();
    let dir = f.relative_dir.clone();
    let content = f.content.clone();
    match state
        .run(move |lib| lib.create_skill(&source, &dir, &content))
        .await
    {
        Ok(id) => {
            state.rescan.notify_one();
            redirect(
                &format!("/skills/{}", encoded(&id)),
                "スキルを作成しました。公開するには管理情報で公開を有効にしてください。",
            )
        }
        Err(e) => {
            new_page(
                &state,
                f.source,
                f.relative_dir,
                f.content,
                format!("作成できませんでした: {e:#}"),
            )
            .await
        }
    }
}
// 生HTMLを無効にし、スキル本文の画像読込とリンク遷移も防ぐ。
fn markdown(content: &str) -> String {
    use comrak::{Arena, Options, format_html, nodes::NodeValue, parse_document};
    let arena = Arena::new();
    let options = Options::default();
    let root = parse_document(&arena, content, &options);
    for node in root.descendants() {
        let mut data = node.data.borrow_mut();
        match &data.value {
            NodeValue::Image(_) | NodeValue::Link(_) => data.value = NodeValue::Emph,
            _ => {}
        }
    }
    let mut html = String::new();
    if format_html(root, &options, &mut html).is_err() {
        return String::new();
    }
    html
}
#[derive(Deserialize)]
struct PreviewForm {
    csrf: String,
    content: String,
}
async fn preview(State(state): State<AppState>, Form(f): Form<PreviewForm>) -> Response {
    if let Some(r) = csrf_error(&state, &f.csrf) {
        return r;
    }
    if f.content.len() > 2 * 1024 * 1024 {
        return error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "本文は2 MiB以内にしてください。",
        );
    }
    Html(format!(
        "<div class=\"markdown\">{}</div>",
        markdown(&f.content)
    ))
    .into_response()
}
async fn asset(Path(name): Path<String>) -> Response {
    let (mime, body): (&str, &[u8]) = match name.as_str() {
        "app.css" => (
            "text/css; charset=utf-8",
            include_bytes!("../static/app.css"),
        ),
        "app.js" => (
            "text/javascript; charset=utf-8",
            include_bytes!("../static/app.js"),
        ),
        "i18n.js" => (
            "text/javascript; charset=utf-8",
            include_bytes!("../static/i18n.js"),
        ),
        "theme.js" => (
            "text/javascript; charset=utf-8",
            include_bytes!("../static/theme.js"),
        ),
        "htmx.min.js" => (
            "text/javascript; charset=utf-8",
            include_bytes!("../static/htmx.min.js"),
        ),
        "icon-32.png" => ("image/png", include_bytes!("../static/icons/icon-32.png")),
        "icon-180.png" => ("image/png", include_bytes!("../static/icons/icon-180.png")),
        "icon-512.png" => ("image/png", include_bytes!("../static/icons/icon-512.png")),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    (
        [
            (header::CONTENT_TYPE, mime),
            (header::CACHE_CONTROL, "public, max-age=3600"),
        ],
        body,
    )
        .into_response()
}
async fn favicon() -> Response {
    (
        [(header::CONTENT_TYPE, "image/x-icon")],
        include_bytes!("../static/icons/skilvella.ico").as_slice(),
    )
        .into_response()
}
