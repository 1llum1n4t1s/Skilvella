mod i18n;

use self::i18n::Locale;
use crate::{AppState, model::*};
use askama::Template;
use axum::{
    Extension, Router,
    extract::{Form, Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    middleware,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

// サービス再起動でも、実行中の保存と完了記録を同じプロセスで共有する。
type PolicyOperations = Mutex<BTreeMap<uuid::Uuid, (&'static str, Instant)>>;
static POLICY_OPERATIONS: OnceLock<PolicyOperations> = OnceLock::new();
fn policy_operations() -> &'static PolicyOperations {
    POLICY_OPERATIONS.get_or_init(Mutex::default)
}
/// 完了済みの操作記録を保持する時間。画面側がこの間に結果を問い合わせる。
const POLICY_OPERATION_TTL: Duration = Duration::from_secs(600);
/// 操作記録の上限件数。実行中の記録は破棄しないため、上限到達時は呼び出し側が拒否する。
const POLICY_OPERATION_LIMIT: usize = 256;
/// 期限切れの完了記録を破棄する。実行中（pending）の記録は結果を書き戻すまで残す。
///
/// 上限到達時の応答（未登録の取消は503、保存開始は409）は呼び出し側ごとに異なるため、
/// ここでは破棄だけを行い、上限判定は `POLICY_OPERATION_LIMIT` を使って呼び出し側で行う。
fn prune_policy_operations(operations: &mut BTreeMap<uuid::Uuid, (&'static str, Instant)>) {
    operations
        .retain(|_, (state, time)| *state == "pending" || time.elapsed() < POLICY_OPERATION_TTL);
}
async fn policy_operation_status(Path(id): Path<String>) -> Response {
    let Ok(id) = uuid::Uuid::parse_str(&id) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    match policy_operations().lock() {
        Ok(operations) => axum::Json(
            serde_json::json!({"state": operations.get(&id).map_or("unknown", |entry| entry.0)}),
        )
        .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
async fn policy_operation(
    Extension(locale): Extension<Locale>,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(form): Form<Token>,
) -> Response {
    if let Some(response) = csrf_error(locale, &state, &form.csrf) {
        return response;
    }
    let Ok(id) = uuid::Uuid::parse_str(&id) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    match policy_operations().lock() {
        Ok(mut operations) => {
            prune_policy_operations(&mut operations);
            if !operations.contains_key(&id) {
                if operations.len() >= POLICY_OPERATION_LIMIT {
                    return StatusCode::SERVICE_UNAVAILABLE.into_response();
                }
                // 未受付なら取消を先に確定し、遅れて到着するPOSTを拒否する。
                operations.insert(id, ("cancelled", Instant::now()));
            }
            axum::Json(serde_json::json!({"state": operations[&id].0})).into_response()
        }
        Err(_) => error(
            locale,
            StatusCode::INTERNAL_SERVER_ERROR,
            "保存状態を確認できません。再読み込みしてください。",
        ),
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(index))
        .route("/sources", get(sources))
        .route("/settings", get(settings))
        .route("/settings/language", post(set_language))
        .route("/settings/codex", post(set_codex_enabled))
        .route(
            "/settings/policy-operations/{id}",
            get(policy_operation_status).post(policy_operation),
        )
        .route("/settings/updates/check", post(check_update))
        .route("/settings/updates/install", post(install_update))
        .route("/sources/add", post(add_source))
        .route("/sources/name", post(set_source_name))
        .route("/sources/remove", post(remove_source))
        .route("/sources/directory", post(set_directory))
        .route("/scan", post(scan))
        .route("/bulk", post(bulk))
        .route("/missing/forget", post(forget_missing))
        .route("/repositories/bulk", post(repository_bulk))
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
        .layer(middleware::from_fn(i18n::localize))
}
#[derive(Template)]
#[template(path = "settings.html")]
struct SettingsPage {
    locale: Locale,
    csrf: String,
    version: &'static str,
    /// 同期先クライアントごとの状態（Codex・Claude Code・Grokの順）。
    clients: Vec<ClientSyncView>,
}
/// 設定画面の同期先1件分。パスは表示用に整形済み。
struct ClientSyncView {
    key: &'static str,
    label: &'static str,
    settings: CodexSyncSettings,
    path: String,
    link_path: String,
}
async fn settings(Extension(locale): Extension<Locale>, State(state): State<AppState>) -> Response {
    let result = state
        .run(|lib| {
            SkillClient::ALL
                .into_iter()
                .map(|client| {
                    let settings = lib.client_settings(client)?;
                    Ok(ClientSyncView {
                        key: client.key(),
                        label: client.label(),
                        path: display_path(&settings.path),
                        link_path: display_path(&settings.link_path),
                        settings,
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()
        })
        .await;
    match result {
        Ok(clients) => render(
            locale,
            SettingsPage {
                locale,
                csrf: state.csrf.to_string(),
                version: env!("CARGO_PKG_VERSION"),
                clients,
            },
        ),
        Err(e) => failure(locale, e),
    }
}
#[derive(Deserialize)]
struct LanguageForm {
    csrf: String,
    language: String,
}
async fn set_language(
    Extension(locale): Extension<Locale>,
    State(state): State<AppState>,
    Form(form): Form<LanguageForm>,
) -> Response {
    if let Some(response) = csrf_error(locale, &state, &form.csrf) {
        return response;
    }
    let Some(language) = Locale::valid_preference(&form.language) else {
        return error(
            locale,
            StatusCode::BAD_REQUEST,
            "表示言語を選択してください。",
        );
    };
    (
        [(header::SET_COOKIE, locale.set_cookie(language))],
        Redirect::to("/settings"),
    )
        .into_response()
}
/// 同期先の設定フォーム。
///
/// - 新しい画面: `targets=1` と、選んだクライアントのチェックボックス（`codex` / `claude` /
///   `grok` = `true`）を送る。未選択のチェックボックスは送られないため、`targets` で
///   「全て未選択」と旧形式を区別する。
/// - 旧形式: `enabled` だけを送り、Codexだけを切り替える（他クライアントは変えない）。
#[derive(Deserialize)]
struct CodexSettingsForm {
    csrf: String,
    enabled: Option<bool>,
    #[serde(default)]
    targets: bool,
    #[serde(default)]
    codex: bool,
    #[serde(default)]
    claude: bool,
    #[serde(default)]
    grok: bool,
}
async fn set_codex_enabled(
    Extension(locale): Extension<Locale>,
    State(state): State<AppState>,
    Form(form): Form<CodexSettingsForm>,
) -> Response {
    if let Some(response) = csrf_error(locale, &state, &form.csrf) {
        return response;
    }
    let changes = match (form.enabled, form.targets) {
        (Some(enabled), false) => vec![(SkillClient::Codex, enabled)],
        (None, true) => vec![
            (SkillClient::Codex, form.codex),
            (SkillClient::Claude, form.claude),
            (SkillClient::Grok, form.grok),
        ],
        _ => {
            return error(
                locale,
                StatusCode::BAD_REQUEST,
                "同期先を選択してください。",
            );
        }
    };
    match state
        .run_mut(move |lib| lib.set_clients_enabled(&changes))
        .await
    {
        Ok(_) => Redirect::to("/settings#codex").into_response(),
        Err(e) => failure(locale, e),
    }
}
#[derive(Deserialize)]
struct CodexPolicyForm {
    csrf: String,
    mode: String,
    #[serde(default)]
    operation: String,
}
// フォームで受理する値と、一覧で選択済みとして表示する値の対応を共有する。
const CODEX_MODES: [(&str, CodexSkillPolicy); 3] = [
    (
        "search",
        CodexSkillPolicy {
            automatic: false,
            command: false,
        },
    ),
    (
        "command",
        CodexSkillPolicy {
            automatic: false,
            command: true,
        },
    ),
    (
        "automatic",
        CodexSkillPolicy {
            automatic: true,
            command: true,
        },
    ),
];
fn codex_mode(mode: &str) -> Option<CodexSkillPolicy> {
    CODEX_MODES
        .iter()
        .find_map(|(name, policy)| (*name == mode).then(|| policy.clone()))
}
async fn set_codex_policy(
    Extension(locale): Extension<Locale>,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<SearchQuery>,
    Query(selection): Query<RepositorySelection>,
    headers: HeaderMap,
    Form(form): Form<CodexPolicyForm>,
) -> Response {
    if let Some(response) = csrf_error(locale, &state, &form.csrf) {
        return response;
    }
    let Some(policy) = codex_mode(&form.mode) else {
        return error(
            locale,
            StatusCode::BAD_REQUEST,
            "使い方を選択してください。",
        );
    };
    let key = id.clone();
    let operation = if form.operation.is_empty() {
        None
    } else {
        let Ok(id) = uuid::Uuid::parse_str(&form.operation) else {
            return error(
                locale,
                StatusCode::BAD_REQUEST,
                "保存状態を確認できません。再読み込みしてください。",
            );
        };
        let Ok(mut operations) = policy_operations().lock() else {
            return error(
                locale,
                StatusCode::INTERNAL_SERVER_ERROR,
                "保存状態を確認できません。再読み込みしてください。",
            );
        };
        prune_policy_operations(&mut operations);
        if operations.contains_key(&id) || operations.len() >= POLICY_OPERATION_LIMIT {
            return error(
                locale,
                StatusCode::CONFLICT,
                "保存状態を確認できません。再読み込みしてください。",
            );
        }
        operations.insert(id, ("pending", Instant::now()));
        Some(id)
    };
    let owner = state.clone();
    // HTTPが切断されても保存・同期・完了記録まで実行する。
    // 保存結果と、完了判定に使った同期状態を一緒に返す。同期状態はJSON応答で再利用し、
    // 保存失敗時は読まないため `None` とする。
    let result = tokio::spawn(async move {
        let result = owner
            .run_mut(move |lib| lib.set_codex_policy(&key, policy.automatic, policy.command))
            .await;
        let sync = if result.is_err() {
            None
        } else {
            Some(owner.run(|lib| lib.sync_summary()).await)
        };
        let completion = match &sync {
            None => "failed",
            Some(Ok(sync)) if sync.last_error.is_empty() => "complete",
            Some(Ok(_)) => "sync_failed",
            Some(Err(_)) => "failed",
        };
        if let Some(id) = operation
            && let Ok(mut operations) = policy_operations().lock()
        {
            operations.insert(id, (completion, Instant::now()));
        }
        result.map(|saved| (saved, sync))
    })
    .await;
    match result.unwrap_or_else(|e| Err(e.into())) {
        Ok((_, sync))
            if headers
                .get(header::ACCEPT)
                .and_then(|h| h.to_str().ok())
                .is_some_and(|h| {
                    h.split(',')
                        .any(|v| v.trim().starts_with("application/json"))
                }) =>
        {
            // 完了判定で読めた同期状態をそのまま返す。読めなかった場合だけ従来どおり
            // 読み直し、2回目に成功すればJSONを返す挙動を保つ。
            let sync = match sync {
                Some(Ok(sync)) => Ok(sync),
                _ => state.run(|lib| lib.sync_summary()).await,
            };
            match sync {
                Ok(sync) => axum::Json(
                    serde_json::json!({"enabled": sync.enabled, "last_error": sync.last_error}),
                )
                .into_response(),
                Err(e) => failure(locale, e),
            }
        }
        Ok(_) if !selection.repository_source.is_empty() => Redirect::to(&format!(
            "{}&message=codex-saved#skill-{}",
            repository_page_url(&selection, &query),
            encoded(&id),
        ))
        .into_response(),
        Ok(_) => redirect(&format!("/skills/{}", encoded(&id)), "codex-saved"),
        Err(e) => failure(locale, e),
    }
}
async fn check_update(
    Extension(locale): Extension<Locale>,
    State(state): State<AppState>,
    Form(form): Form<Token>,
) -> Response {
    if let Some(response) = csrf_error(locale, &state, &form.csrf) {
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
async fn install_update(
    Extension(locale): Extension<Locale>,
    State(state): State<AppState>,
    Form(form): Form<Token>,
) -> Response {
    if let Some(response) = csrf_error(locale, &state, &form.csrf) {
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
fn render(locale: Locale, t: impl Template) -> Response {
    match t.render() {
        Ok(s) => Html(s).into_response(),
        Err(_) => error(
            locale,
            StatusCode::INTERNAL_SERVER_ERROR,
            "画面を表示できませんでした。再読み込みしてください。",
        ),
    }
}
#[derive(Template)]
#[template(path = "error.html")]
struct ErrorPage<'a> {
    locale: Locale,
    message: &'a str,
}
fn error(locale: Locale, status: StatusCode, key: &str) -> Response {
    error_page(locale, status, locale.text(key))
}
fn error_page(locale: Locale, status: StatusCode, message: &str) -> Response {
    (
        status,
        Html(
            ErrorPage { locale, message }
                .render()
                .unwrap_or_else(|_| locale.text("エラー").into()),
        ),
    )
        .into_response()
}
fn csrf_error(locale: Locale, state: &AppState, token: &str) -> Option<Response> {
    if token == state.csrf.as_ref() {
        None
    } else {
        Some(error(
            locale,
            StatusCode::FORBIDDEN,
            "操作用トークンが一致しません。画面を再読み込みしてからやり直してください。",
        ))
    }
}
fn failure(locale: Locale, e: anyhow::Error) -> Response {
    error_page(
        locale,
        StatusCode::BAD_REQUEST,
        &with_detail(locale, "操作できませんでした: {detail}", e),
    )
}
fn with_detail(locale: Locale, key: &str, error: anyhow::Error) -> String {
    locale.format(key, &[("detail", &format!("{error:#}"))])
}
fn encoded(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            result.push(b as char);
        } else {
            let hex = b"0123456789ABCDEF";
            result.push('%');
            result.push(hex[(b >> 4) as usize] as char);
            result.push(hex[(b & 15) as usize] as char);
        }
    }
    result
}
// message は内部で決まる通知IDと数値だけを含む。表示言語は遷移先のリクエストで決める。
fn redirect(path: &str, message: &str) -> Response {
    Redirect::to(&format!("{path}?message={message}")).into_response()
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
    indexed: usize,
    errors: usize,
    missing: usize,
    updated: usize,
    skipped: usize,
}
impl Notice {
    fn text(&self, locale: Locale) -> String {
        let key = match self.message.as_str() {
            "codex-saved" => "使い方を保存しました。同期状態を確認してください。",
            "source-renamed" => "表示名を保存しました。",
            "source-removed" => "登録を解除しました。元のフォルダーとファイルは保持されています。",
            "missing-forgotten" => {
                "不在スキルの登録情報と履歴を削除しました。実ファイルは変更していません。"
            }
            "skills-updated" => "選択したスキルを更新しました。",
            "content-saved" => "SKILL.md を保存しました。",
            "metadata-saved" => "管理情報を保存しました。",
            "history-restored" => "履歴から復元しました。",
            "skill-created" => {
                "スキルを作成しました。公開するには管理情報で公開を有効にしてください。"
            }
            "scan" | "directory-saved" => {
                let summary = locale.format(
                    "スキャン完了: {count}件を登録、エラー{errors}件、見つからないスキル{missing}件。",
                    &[("count", &self.indexed.to_string()), ("errors", &self.errors.to_string()), ("missing", &self.missing.to_string())],
                );
                return if self.message == "directory-saved" {
                    let separator = if locale.language() == "ja" { "" } else { " " };
                    format!(
                        "{}{separator}{summary}",
                        locale.text("管理対象フォルダーの設定を保存しました。")
                    )
                } else {
                    summary
                };
            }
            "repositories-updated" => {
                return locale.format(
                    "取得元の一括設定: {updated}件更新、{skipped}件対象外。",
                    &[
                        ("updated", &self.updated.to_string()),
                        ("skipped", &self.skipped.to_string()),
                    ],
                );
            }
            // 以前のバージョンが生成したURLの通知も、原文のまま表示する。
            _ => return self.message.clone(),
        };
        locale.text(key).to_owned()
    }
}
#[derive(Template)]
#[template(path = "index.html")]
struct IndexPage {
    locale: Locale,
    csrf: String,
    query: SearchQuery,
    selection: RepositorySelection,
    groups: Vec<RepositoryGroup>,
    selected: bool,
    reset: String,
    results: SearchResults,
    error_count: usize,
    policies: BTreeMap<String, Option<CodexSkillPolicy>>,
    current_query: String,
    sources: Vec<Source>,
    codex: CodexSyncSettings,
    message: String,
    previous: String,
    next: String,
}
impl IndexPage {
    fn policy_mode(&self, id: &str) -> &'static str {
        let Some(Some(policy)) = self.policies.get(id) else {
            return "";
        };
        CODEX_MODES
            .iter()
            .find_map(|(name, value)| (policy == value).then_some(*name))
            .unwrap_or_default()
    }
    fn group_value(&self, group: &RepositoryGroup) -> String {
        serde_json::json!({"repository_source": group.source, "repository": group.repository})
            .to_string()
    }
    fn group_url(&self, group: &RepositoryGroup) -> String {
        if group.unknown {
            return "/sources".into();
        }
        repository_url(&group.source, &group.repository)
    }
    fn source_label(&self, alias: &str) -> String {
        self.sources
            .iter()
            .find(|s| s.alias == alias)
            .map(Source::label)
            .unwrap_or_else(|| alias.to_owned())
    }
}
/// 取得元一覧の1リポジトリを開く基本URLを組み立てる。
///
/// 取得元とリポジトリ名はどちらも `encoded` で同じ規則に従ってエンコードする。
/// メッセージや状態などの追加パラメーターは、呼び出し側で `&...` として後ろへ足す。
/// 検索条件を引き継ぐ `repository_page_url` は `source` を重ねて持つ別形式のため、
/// この関数へは統合しない。
fn repository_url(source: &str, repository: &str) -> String {
    format!(
        "/?repository_source={}&repository={}",
        encoded(source),
        encoded(repository)
    )
}
fn repository_page_url(selection: &RepositorySelection, query: &SearchQuery) -> String {
    format!(
        "/?repository_source={}&repository={}&q={}&source={}&tag={}&category={}&status={}&offset={}",
        encoded(&selection.repository_source),
        encoded(&selection.repository),
        encoded(&query.q),
        encoded(&selection.repository_source),
        encoded(&query.tag),
        encoded(&query.category),
        encoded(&query.status),
        query.offset,
    )
}
async fn index(
    Extension(locale): Extension<Locale>,
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
    Query(selection): Query<RepositorySelection>,
    Query(notice): Query<Notice>,
) -> Response {
    let mut query = query;
    if query.status.is_empty() {
        query.status = "present".into();
    }
    query.limit = 50;
    query.source = selection.repository_source.clone();
    let q = query.clone();
    let selected = !selection.repository_source.is_empty()
        || matches!(query.status.as_str(), "error" | "missing" | "all");
    let scope = selection.clone();
    match state
        .run(move |lib| {
            let (groups, error_count) = lib.repository_groups_with_error_count()?;
            let results = if selected && scope.repository_source.is_empty() {
                lib.search(q, false)?
            } else if selected {
                lib.search_repository_with_groups(q, scope, &groups)?
            } else {
                SearchResults {
                    skills: Vec::new(),
                    total: groups.iter().map(|g| g.count - g.missing).sum(),
                    indexed_at: String::new(),
                }
            };
            let ids = results
                .skills
                .iter()
                .map(|skill| skill.id.clone())
                .collect::<Vec<_>>();
            let policies = lib.codex_policies(&ids)?;
            Ok((
                results,
                groups,
                policies,
                lib.sources()?,
                lib.sync_summary()?,
                error_count,
            ))
        })
        .await
    {
        Ok((results, groups, policies, sources, codex, error_count)) => {
            let reset = repository_url(&selection.repository_source, &selection.repository);
            let page_url = |offset| {
                let mut page = query.clone();
                page.offset = offset;
                repository_page_url(&selection, &page)
            };
            let current_query = page_url(query.offset).trim_start_matches("/?").to_owned();
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
            render(
                locale,
                IndexPage {
                    locale,
                    csrf: state.csrf.to_string(),
                    query,
                    selection,
                    groups,
                    selected,
                    reset,
                    results,
                    error_count,
                    policies,
                    current_query,
                    sources,
                    codex,
                    message: notice.text(locale),
                    previous,
                    next,
                },
            )
        }
        Err(e) => failure(locale, e),
    }
}
#[derive(Template)]
#[template(path = "sources.html")]
struct SourcesPage {
    locale: Locale,
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

async fn sources(
    Extension(locale): Extension<Locale>,
    State(state): State<AppState>,
    Query(n): Query<Notice>,
) -> Response {
    source_page(locale, &state, n.text(locale), String::new(), String::new()).await
}
#[derive(Deserialize)]
struct SourceNameForm {
    csrf: String,
    alias: String,
    display_name: String,
}
async fn set_source_name(
    Extension(locale): Extension<Locale>,
    State(state): State<AppState>,
    Form(form): Form<SourceNameForm>,
) -> Response {
    if let Some(response) = csrf_error(locale, &state, &form.csrf) {
        return response;
    }
    match state
        .run_mut(move |lib| lib.set_source_display_name(&form.alias, &form.display_name))
        .await
    {
        Ok(()) => redirect("/sources", "source-renamed"),
        Err(e) => error(locale, StatusCode::BAD_REQUEST, &format!("{e:#}")),
    }
}
async fn source_page(
    locale: Locale,
    state: &AppState,
    message: String,
    alias: String,
    path: String,
) -> Response {
    source_page_with_directory(locale, state, message, alias, path, None).await
}
async fn source_page_with_directory(
    locale: Locale,
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
            render(
                locale,
                SourcesPage {
                    locale,
                    csrf: state.csrf.to_string(),
                    sources,
                    message,
                    alias,
                    path,
                    directory,
                    directory_path,
                    directory_editing,
                },
            )
        }
        Err(e) => failure(locale, e),
    }
}
#[derive(Deserialize)]
struct DirectoryForm {
    csrf: String,
    mode: String,
    #[serde(default)]
    path: String,
}
async fn set_directory(
    Extension(locale): Extension<Locale>,
    State(state): State<AppState>,
    Form(form): Form<DirectoryForm>,
) -> Response {
    if let Some(response) = csrf_error(locale, &state, &form.csrf) {
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
            redirect("/sources", &scan_notice("directory-saved", report))
        }
        Err(error) => {
            source_page_with_directory(
                locale,
                &state,
                with_detail(
                    locale,
                    "設定を変更できませんでした: {detail}。以前の設定は保持されています。",
                    error,
                ),
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
async fn add_source(
    Extension(locale): Extension<Locale>,
    State(state): State<AppState>,
    Form(f): Form<SourceForm>,
) -> Response {
    if let Some(r) = csrf_error(locale, &state, &f.csrf) {
        return r;
    }
    let alias = f.alias.clone();
    let path = f.path.clone();
    if !matches!(f.action.as_str(), "" | "register") {
        return error(locale, StatusCode::BAD_REQUEST, "登録方法が不正です。");
    }
    match state
        .run_mut(move |lib| lib.add_source(&alias, std::path::Path::new(&path)))
        .await
    {
        Ok(report) => {
            state.rescan.notify_one();
            redirect("/sources", &scan_notice("scan", report))
        }
        Err(e) => {
            source_page(
                locale,
                &state,
                with_detail(locale, "登録できませんでした: {detail}", e),
                f.alias,
                f.path,
            )
            .await
        }
    }
}
async fn remove_source(
    Extension(locale): Extension<Locale>,
    State(state): State<AppState>,
    Form(f): Form<SourceForm>,
) -> Response {
    if let Some(r) = csrf_error(locale, &state, &f.csrf) {
        return r;
    }
    match state.run_mut(move |lib| lib.remove_source(&f.alias)).await {
        Ok(_) => {
            state.rescan.notify_one();
            redirect("/sources", "source-removed")
        }
        Err(e) => failure(locale, e),
    }
}
#[derive(Deserialize)]
struct Token {
    csrf: String,
}
fn scan_notice(id: &str, r: ScanReport) -> String {
    format!(
        "{id}&indexed={}&errors={}&missing={}",
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
        // メッセージは既存どおりエンコードせずに付け足す（呼び出し側が固定文字列を渡す）。
        Redirect::to(&format!(
            "{}&message={message}",
            repository_url(&selection.repository_source, &selection.repository)
        ))
        .into_response()
    }
}
async fn scan(
    Extension(locale): Extension<Locale>,
    State(state): State<AppState>,
    Form(f): Form<RepositoryToken>,
) -> Response {
    if let Some(r) = csrf_error(locale, &state, &f.csrf) {
        return r;
    }
    match state.run_mut(|lib| lib.scan()).await {
        Ok(r) => {
            state.rescan.notify_one();
            repository_redirect(&f.selection, &scan_notice("scan", r))
        }
        Err(e) => failure(locale, e),
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
#[derive(Deserialize)]
struct ForgetMissingForm {
    csrf: String,
    expected: usize,
    #[serde(flatten)]
    selection: RepositorySelection,
    #[serde(default)]
    q: String,
    #[serde(default)]
    tag: String,
    #[serde(default)]
    category: String,
}
async fn forget_missing(
    Extension(locale): Extension<Locale>,
    State(state): State<AppState>,
    Form(f): Form<ForgetMissingForm>,
) -> Response {
    if let Some(r) = csrf_error(locale, &state, &f.csrf) {
        return r;
    }
    let selection = f.selection.clone();
    let query = SearchQuery {
        q: f.q,
        tag: f.tag,
        category: f.category,
        ..Default::default()
    };
    match state
        .run_mut(move |lib| lib.forget_missing(query, selection, f.expected))
        .await
    {
        Ok(_) => {
            let target = format!(
                "{}&status=missing",
                repository_url(&f.selection.repository_source, &f.selection.repository)
            );
            Redirect::to(&format!("{target}&message=missing-forgotten")).into_response()
        }
        Err(e) => failure(locale, e),
    }
}
fn bulk_settings(
    published: &str,
    mode: &str,
) -> Result<(Option<bool>, Option<crate::model::CodexSkillPolicy>), &'static str> {
    let published = match published {
        "true" => Some(true),
        "false" => Some(false),
        "" => None,
        _ => return Err("公開状態が不正です。"),
    };
    let codex = if mode.is_empty() {
        None
    } else {
        Some(codex_mode(mode).ok_or("使い方を選択してください。")?)
    };
    Ok((published, codex))
}

async fn bulk(
    Extension(locale): Extension<Locale>,
    State(state): State<AppState>,
    Form(f): Form<BulkForm>,
) -> Response {
    if let Some(r) = csrf_error(locale, &state, &f.csrf) {
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
            locale,
            StatusCode::BAD_REQUEST,
            "スキルを1件以上選んでください。ブラウザーでJavaScriptを有効にしてください。",
        );
    }
    let (published, codex) = match bulk_settings(&f.published, &f.codex_mode) {
        Ok(settings) => settings,
        Err(message) => return error(locale, StatusCode::BAD_REQUEST, message),
    };
    let add_tags = tags(&f.tags);
    if published.is_none() && add_tags.is_empty() && codex.is_none() {
        return error(
            locale,
            StatusCode::BAD_REQUEST,
            "変更する公開状態・追加タグ・使い方のいずれかを指定してください。",
        );
    }
    match state
        .run_mut(move |lib| lib.bulk_settings(&ids, published, &add_tags, codex))
        .await
    {
        Ok(_) => repository_redirect(&f.selection, "skills-updated"),
        Err(e) => failure(locale, e),
    }
}
// 重複する selected は保持し、単一フィールドの重複や未知のキーは拒否する。
async fn repository_bulk(
    Extension(locale): Extension<Locale>,
    State(state): State<AppState>,
    Form(fields): Form<Vec<(String, String)>>,
) -> Response {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Scope {
        repository_source: String,
        repository: String,
    }
    let mut csrf = None;
    let mut published = None;
    let mut mode = None;
    let mut selections = Vec::new();
    for (key, value) in fields {
        let slot = match key.as_str() {
            "csrf" => &mut csrf,
            "published" => &mut published,
            "codex_mode" => &mut mode,
            "selected" => {
                let Ok(scope) = serde_json::from_str::<Scope>(&value) else {
                    return error(locale, StatusCode::BAD_REQUEST, "取得元の選択が不正です。");
                };
                if scope.repository_source.is_empty() {
                    return error(locale, StatusCode::BAD_REQUEST, "取得元の選択が不正です。");
                }
                selections.push(RepositorySelection {
                    repository_source: scope.repository_source,
                    repository: scope.repository,
                });
                continue;
            }
            _ => {
                return error(
                    locale,
                    StatusCode::BAD_REQUEST,
                    "取得元の一括設定が不正です。",
                );
            }
        };
        if slot.replace(value).is_some() {
            return error(
                locale,
                StatusCode::BAD_REQUEST,
                "取得元の一括設定が不正です。",
            );
        }
    }
    let (Some(csrf), Some(published), Some(mode)) = (csrf, published, mode) else {
        return error(
            locale,
            StatusCode::BAD_REQUEST,
            "取得元の一括設定が不正です。",
        );
    };
    if let Some(response) = csrf_error(locale, &state, &csrf) {
        return response;
    }
    if selections.is_empty() {
        return error(
            locale,
            StatusCode::BAD_REQUEST,
            "取得元を1件以上選んでください。",
        );
    }
    let (published, codex) = match bulk_settings(&published, &mode) {
        Ok(settings) => settings,
        Err(message) => return error(locale, StatusCode::BAD_REQUEST, message),
    };
    if published.is_none() && codex.is_none() {
        return error(
            locale,
            StatusCode::BAD_REQUEST,
            "変更する公開状態・使い方を指定してください。",
        );
    }
    match state
        .run_mut(move |lib| lib.bulk_repository_settings(&selections, published, codex))
        .await
    {
        Ok((updated, skipped)) => redirect(
            "/",
            &format!("repositories-updated&updated={updated}&skipped={skipped}"),
        ),
        Err(e) => failure(locale, e),
    }
}
#[derive(Template)]
#[template(path = "detail.html")]
struct DetailPage {
    locale: Locale,
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
    Extension(locale): Extension<Locale>,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(n): Query<Notice>,
) -> Response {
    detail_page(locale, &state, id, n.text(locale), None).await
}
#[derive(Template)]
#[template(path = "draft.html")]
struct DraftPage {
    locale: Locale,
    csrf: String,
    id: String,
    content: String,
    revision: String,
    message: String,
}
async fn detail_page(
    locale: Locale,
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
                Ok(selection) => {
                    repository_url(&selection.repository_source, &selection.repository)
                }
                Err(_) => "/sources".into(),
            };
            let codex = lib.sync_summary()?;
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
            render(
                locale,
                DetailPage {
                    locale,
                    csrf: state.csrf.to_string(),
                    codex,
                    source_label,
                    library_url,
                    detail,
                    preview,
                    tag_text,
                    message,
                    dirty,
                },
            )
        }
        Err(e) => match draft {
            Some((content, revision)) => render(
                locale,
                DraftPage {
                    locale,
                    csrf: state.csrf.to_string(),
                    id,
                    content,
                    revision,
                    message: format!(
                        "{message} {}",
                        with_detail(locale, "最新内容の取得にも失敗しました: {detail}", e)
                    ),
                },
            ),
            None => failure(locale, e),
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
    Extension(locale): Extension<Locale>,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(f): Form<ContentForm>,
) -> Response {
    if let Some(r) = csrf_error(locale, &state, &f.csrf) {
        return r;
    }
    let key = id.clone();
    let content = f.content.clone();
    let revision = f.revision.clone();
    match state.run_mut(move |lib| lib.save_content(&key, &revision, &content)).await {
        Ok(_) => redirect(&format!("/skills/{}", encoded(&id)), "content-saved"),
        Err(e) => detail_page(
            locale, &state, id,
            with_detail(locale, "保存できませんでした: {detail}。下書きは下の編集欄に保持しています。競合時は下書きをコピーしてから「最新版を開く」で最新内容へ統合してください。", e),
            Some((f.content, f.revision)),
        ).await,
    }
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
    Extension(locale): Extension<Locale>,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(f): Form<MetadataForm>,
) -> Response {
    if let Some(r) = csrf_error(locale, &state, &f.csrf) {
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
        Ok(_) => redirect(&format!("/skills/{}", encoded(&id)), "metadata-saved"),
        Err(e) => failure(locale, e),
    }
}
#[derive(Deserialize)]
struct RestoreForm {
    csrf: String,
    history_id: i64,
    revision: String,
}
async fn restore(
    Extension(locale): Extension<Locale>,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(f): Form<RestoreForm>,
) -> Response {
    if let Some(r) = csrf_error(locale, &state, &f.csrf) {
        return r;
    }
    let key = id.clone();
    match state
        .run_mut(move |lib| lib.restore(&key, f.history_id, &f.revision))
        .await
    {
        Ok(_) => redirect(&format!("/skills/{}", encoded(&id)), "history-restored"),
        Err(e) => {
            detail_page(
                locale,
                &state,
                id,
                with_detail(locale, "復元できませんでした: {detail}", e),
                None,
            )
            .await
        }
    }
}
#[derive(Template)]
#[template(path = "new.html")]
struct NewPage {
    locale: Locale,
    csrf: String,
    sources: Vec<Source>,
    source: String,
    relative_dir: String,
    content: String,
    message: String,
}
async fn new_skill(
    Extension(locale): Extension<Locale>,
    State(state): State<AppState>,
) -> Response {
    new_page(
        locale,
        &state,
        String::new(),
        String::new(),
        "---\nname: new-skill\ndescription: スキルの用途\n---\n\n# 手順\n\n".into(),
        String::new(),
    )
    .await
}
async fn new_page(
    locale: Locale,
    state: &AppState,
    source: String,
    relative_dir: String,
    content: String,
    message: String,
) -> Response {
    match state.run(|lib| lib.sources()).await {
        Ok(sources) => render(
            locale,
            NewPage {
                locale,
                csrf: state.csrf.to_string(),
                sources,
                source,
                relative_dir,
                content,
                message,
            },
        ),
        Err(e) => failure(locale, e),
    }
}
#[derive(Deserialize)]
struct CreateForm {
    csrf: String,
    source: String,
    relative_dir: String,
    content: String,
}
async fn create(
    Extension(locale): Extension<Locale>,
    State(state): State<AppState>,
    Form(f): Form<CreateForm>,
) -> Response {
    if let Some(r) = csrf_error(locale, &state, &f.csrf) {
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
            redirect(&format!("/skills/{}", encoded(&id)), "skill-created")
        }
        Err(e) => {
            new_page(
                locale,
                &state,
                f.source,
                f.relative_dir,
                f.content,
                with_detail(locale, "作成できませんでした: {detail}", e),
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
async fn preview(
    Extension(locale): Extension<Locale>,
    State(state): State<AppState>,
    Form(f): Form<PreviewForm>,
) -> Response {
    if let Some(r) = csrf_error(locale, &state, &f.csrf) {
        return r;
    }
    if f.content.len() > 2 * 1024 * 1024 {
        return error(
            locale,
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
        "policy-navigation.js" => (
            "text/javascript; charset=utf-8",
            include_bytes!("../static/policy-navigation.js"),
        ),
        "theme.js" => (
            "text/javascript; charset=utf-8",
            include_bytes!("../static/theme.js"),
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

#[cfg(test)]
mod tests {
    #[test]
    fn encodes_query_bytes() {
        assert_eq!(super::encoded(""), "");
        assert_eq!(super::encoded("aZ09-._~"), "aZ09-._~");
        assert_eq!(
            super::encoded(" /?&=+%日本語"),
            "%20%2F%3F%26%3D%2B%25%E6%97%A5%E6%9C%AC%E8%AA%9E"
        );
    }

    /// 集約前の `format!` と同じURLをバイト単位で生成することを確認する。
    #[test]
    fn repository_url_matches_previous_format() {
        let cases = [
            ("", ""),
            ("local", "repo"),
            ("git hub", "owner/名前&x=1"),
            ("a+b%", "?#~._-"),
        ];
        for (source, repository) in cases {
            let previous = format!(
                "/?repository_source={}&repository={}",
                super::encoded(source),
                super::encoded(repository)
            );
            assert_eq!(super::repository_url(source, repository), previous);
            // 呼び出し側で付け足す形式も旧実装と一致する。
            let message = "scan-done";
            assert_eq!(
                format!(
                    "{}&message={message}",
                    super::repository_url(source, repository)
                ),
                format!(
                    "/?repository_source={}&repository={}&message={message}",
                    super::encoded(source),
                    super::encoded(repository)
                )
            );
        }
        assert_eq!(
            super::repository_url("git hub", "a/b"),
            "/?repository_source=git%20hub&repository=a%2Fb"
        );
    }
}
