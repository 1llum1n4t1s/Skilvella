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
        .route("/settings/codex", post(set_codex_enabled))
        .route("/settings/updates/check", post(check_update))
        .route("/settings/updates/install", post(install_update))
        .route("/sources/add", post(add_source))
        .route("/sources/name", post(set_source_name))
        .route("/sources/remove", post(remove_source))
        .route("/sources/directory", post(set_directory))
        .route("/scan", post(scan))
        .route("/bulk", post(bulk))
        .route("/skills/new", get(new_skill))
        .route("/skills/create", post(create))
        .route("/skills/{id}", get(detail))
        .route("/skills/{id}/content", post(save))
        .route("/skills/{id}/metadata", post(metadata))
        .route("/skills/{id}/codex", post(set_codex_policy))
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
    codex: CodexSyncSettings,
}
impl SettingsPage {
    fn display_codex_path(&self) -> String {
        display_path(&self.codex.path)
    }
}
async fn settings(State(state): State<AppState>) -> Response {
    match state.run(|lib| lib.codex_settings()).await {
        Ok(codex) => render(SettingsPage {
            csrf: state.csrf.to_string(),
            version: env!("CARGO_PKG_VERSION"),
            codex,
        }),
        Err(e) => failure(e),
    }
}
#[derive(Deserialize)]
struct CodexSettingsForm {
    csrf: String,
    enabled: bool,
}
async fn set_codex_enabled(
    State(state): State<AppState>,
    Form(form): Form<CodexSettingsForm>,
) -> Response {
    if let Some(response) = csrf_error(&state, &form.csrf) {
        return response;
    }
    match state
        .run_mut(move |lib| lib.set_codex_enabled(form.enabled))
        .await
    {
        Ok(_) => Redirect::to("/settings#codex").into_response(),
        Err(e) => failure(e),
    }
}
#[derive(Deserialize)]
struct CodexPolicyForm {
    csrf: String,
    mode: String,
}
fn codex_mode(mode: &str) -> Option<crate::model::CodexSkillPolicy> {
    let (automatic, command) = match mode {
        "search" => (false, false),
        "command" => (false, true),
        "automatic" => (true, true),
        _ => return None,
    };
    Some(crate::model::CodexSkillPolicy { automatic, command })
}
async fn set_codex_policy(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(form): Form<CodexPolicyForm>,
) -> Response {
    if let Some(response) = csrf_error(&state, &form.csrf) {
        return response;
    }
    let Some(policy) = codex_mode(&form.mode) else {
        return error(
            StatusCode::BAD_REQUEST,
            "Codexでの使い方を選択してください。",
        );
    };
    let key = id.clone();
    match state
        .run_mut(move |lib| lib.set_codex_policy(&key, policy.automatic, policy.command))
        .await
    {
        Ok(_) => redirect(
            &format!("/skills/{}", encoded(&id)),
            "Codex設定を保存しました。同期状態を確認してください。",
        ),
        Err(e) => failure(e),
    }
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
    selection: RepositorySelection,
    groups: Vec<RepositoryGroup>,
    selected: bool,
    reset: String,
    results: SearchResults,
    sources: Vec<Source>,
    codex: CodexSyncSettings,
    message: String,
    previous: String,
    next: String,
}
impl IndexPage {
    fn group_url(&self, group: &RepositoryGroup) -> String {
        if group.unknown {
            return "/sources".into();
        }
        format!(
            "/?repository_source={}&repository={}",
            encoded(&group.source),
            encoded(&group.repository)
        )
    }
    fn source_label(&self, alias: &str) -> String {
        self.sources
            .iter()
            .find(|s| s.alias == alias)
            .map(Source::label)
            .unwrap_or_else(|| alias.to_owned())
    }
}
async fn index(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
    Query(selection): Query<RepositorySelection>,
    Query(notice): Query<Notice>,
) -> Response {
    let mut query = query;
    query.limit = 50;
    query.source = selection.repository_source.clone();
    let q = query.clone();
    let selected = !selection.repository_source.is_empty();
    let scope = selection.clone();
    match state
        .run(move |lib| {
            let groups = lib.repository_groups()?;
            let results = if selected {
                lib.search_repository(q, scope)?
            } else {
                SearchResults {
                    skills: Vec::new(),
                    total: groups.iter().map(|g| g.count).sum(),
                    indexed_at: String::new(),
                }
            };
            Ok((results, groups, lib.sources()?, lib.codex_settings()?))
        })
        .await
    {
        Ok((results, groups, sources, codex)) => {
            let reset = format!(
                "/?repository_source={}&repository={}",
                encoded(&selection.repository_source),
                encoded(&selection.repository)
            );
            let page_url = |offset| {
                format!(
                    "{reset}&q={}&source={}&tag={}&category={}&status={}&offset={offset}",
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
                selection,
                groups,
                selected,
                reset,
                results,
                sources,
                codex,
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
}

impl SourcesPage {
    fn has_directory_source(&self) -> bool {
        self.sources
            .iter()
            .any(|source| source.alias == self.directory.source_alias)
    }

    fn display_path(&self, path: impl AsRef<str>) -> String {
        display_path(path.as_ref())
    }
}
fn display_path(path: &str) -> String {
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

async fn sources(State(state): State<AppState>, Query(n): Query<Notice>) -> Response {
    source_page(&state, n.message, String::new(), String::new()).await
}
#[derive(Deserialize)]
struct SourceNameForm {
    csrf: String,
    alias: String,
    display_name: String,
}
async fn set_source_name(
    State(state): State<AppState>,
    Form(form): Form<SourceNameForm>,
) -> Response {
    if let Some(response) = csrf_error(&state, &form.csrf) {
        return response;
    }
    match state
        .run_mut(move |lib| lib.set_source_display_name(&form.alias, &form.display_name))
        .await
    {
        Ok(()) => redirect("/sources", "表示名を保存しました。"),
        Err(e) => error(StatusCode::BAD_REQUEST, &format!("{e:#}")),
    }
}
async fn source_page(state: &AppState, message: String, alias: String, path: String) -> Response {
    source_page_with_directory(state, message, alias, path, None).await
}
async fn source_page_with_directory(
    state: &AppState,
    message: String,
    alias: String,
    path: String,
    draft: Option<(String, String)>,
) -> Response {
    match state
        .run(|lib| Ok((lib.sources()?, lib.directory_settings()?)))
        .await
    {
        Ok((sources, directory)) => {
            let directory_editing = draft.is_some();
            let directory_path = draft.map(|(_, path)| path).unwrap_or_else(|| {
                if directory.path.is_empty() {
                    directory.custom_path.clone()
                } else {
                    directory.path.clone()
                }
            });
            render(SourcesPage {
                csrf: state.csrf.to_string(),
                sources,
                message,
                alias,
                path,
                directory,
                directory_path,
                directory_editing,
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
        .run_mut(move |library| library.set_directory(&mode, &path))
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
    if !matches!(f.action.as_str(), "" | "register") {
        return error(StatusCode::BAD_REQUEST, "登録方法が不正です。");
    }
    match state
        .run_mut(move |lib| lib.add_source(&alias, std::path::Path::new(&path)))
        .await
    {
        Ok(report) => {
            state.rescan.notify_one();
            redirect("/sources", &scan_message(report))
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
async fn remove_source(State(state): State<AppState>, Form(f): Form<SourceForm>) -> Response {
    if let Some(r) = csrf_error(&state, &f.csrf) {
        return r;
    }
    match state.run_mut(move |lib| lib.remove_source(&f.alias)).await {
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
#[derive(Deserialize)]
struct RepositoryToken {
    csrf: String,
    #[serde(flatten)]
    selection: RepositorySelection,
}
fn repository_redirect(selection: &RepositorySelection, message: &str) -> Response {
    if selection.repository_source.is_empty() {
        redirect("/", message)
    } else {
        Redirect::to(&format!(
            "/?repository_source={}&repository={}&message={}",
            encoded(&selection.repository_source),
            encoded(&selection.repository),
            encoded(message)
        ))
        .into_response()
    }
}
async fn scan(State(state): State<AppState>, Form(f): Form<RepositoryToken>) -> Response {
    if let Some(r) = csrf_error(&state, &f.csrf) {
        return r;
    }
    match state.run_mut(|lib| lib.scan()).await {
        Ok(r) => {
            state.rescan.notify_one();
            repository_redirect(&f.selection, &scan_message(r))
        }
        Err(e) => failure(e),
    }
}
#[derive(Deserialize)]
struct BulkForm {
    csrf: String,
    #[serde(flatten)]
    selection: RepositorySelection,
    #[serde(default)]
    selected: String,
    #[serde(default)]
    published: String,
    #[serde(default)]
    tags: String,
    #[serde(default)]
    codex_mode: String,
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
    let codex = if f.codex_mode.is_empty() {
        None
    } else {
        let Some(policy) = codex_mode(&f.codex_mode) else {
            return error(
                StatusCode::BAD_REQUEST,
                "Codexでの使い方を選択してください。",
            );
        };
        Some(policy)
    };
    if published.is_none() && add_tags.is_empty() && codex.is_none() {
        return error(
            StatusCode::BAD_REQUEST,
            "変更する公開状態・追加タグ・Codexでの使い方のいずれかを指定してください。",
        );
    }
    match state
        .run_mut(move |lib| lib.bulk_settings(&ids, published, &add_tags, codex))
        .await
    {
        Ok(_) => repository_redirect(&f.selection, "選択したスキルを更新しました。"),
        Err(e) => failure(e),
    }
}
#[derive(Template)]
#[template(path = "detail.html")]
struct DetailPage {
    csrf: String,
    codex: CodexSyncSettings,
    source_label: String,
    library_url: String,
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
    match state
        .run(move |lib| {
            let detail = lib.detail(&key, false)?;
            let source_label = lib.source_label(&detail.summary.source)?;
            let library_url = match lib.repository_selection(&detail.summary) {
                Ok(selection) => format!(
                    "/?repository_source={}&repository={}",
                    encoded(&selection.repository_source),
                    encoded(&selection.repository)
                ),
                Err(_) => "/sources".into(),
            };
            let codex = lib.codex_settings()?;
            Ok((detail, source_label, codex, library_url))
        })
        .await
    {
        Ok((mut detail, source_label, codex, library_url)) => {
            let dirty = draft.is_some();
            if let Some((content, revision)) = draft {
                detail.content = content;
                detail.revision = revision;
            }
            let preview = markdown(&detail.content);
            let tag_text = detail.summary.tags.join(", ");
            render(DetailPage {
                csrf: state.csrf.to_string(),
                codex,
                source_label,
                library_url,
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
    match state.run_mut(move|lib|lib.save_content(&key,&revision,&content)).await {Ok(_)=>redirect(&format!("/skills/{}",encoded(&id)),"SKILL.md を保存しました。"),Err(e)=>detail_page(&state,id,format!("保存できませんでした: {e:#}。下書きは下の編集欄に保持しています。競合時は下書きをコピーしてから「最新版を開く」で最新内容へ統合してください。"),Some((f.content,f.revision))).await}
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
    match state
        .run_mut(move |lib| lib.set_metadata(&key, value))
        .await
    {
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
        .run_mut(move |lib| lib.restore(&key, f.history_id, &f.revision))
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
        .run_mut(move |lib| lib.create_skill(&source, &dir, &content))
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
