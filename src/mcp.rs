use crate::{AppState, model::SearchQuery};
use rmcp::{
    ErrorData, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Implementation, ServerCapabilities, ServerConfig},
    schemars, tool, tool_handler, tool_router,
};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

const PAGE_BYTES: usize = 8192;
const FILES_PER_PAGE: usize = 100;

fn hash<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
    let mut digest = Sha256::new();
    for part in parts {
        digest.update((part.len() as u64).to_be_bytes());
        digest.update(part.as_bytes());
    }
    // 16進化は共通関数へ集約する（固定ベクトルのテストで出力の同一性を確認している）。
    crate::sha256_hex(&digest.finalize())
}

fn page(text: &str, offset: usize) -> anyhow::Result<(&str, usize)> {
    anyhow::ensure!(
        offset <= text.len() && text.is_char_boundary(offset),
        "Invalid UTF-8 byte offset"
    );
    let mut end = offset.saturating_add(PAGE_BYTES).min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    Ok((&text[offset..end], end))
}

fn files_page_end(files: &[String], offset: usize) -> usize {
    let mut end = offset;
    let mut bytes = 0usize;
    while end < files.len() && end - offset < FILES_PER_PAGE {
        let len = files[end].len();
        if len > PAGE_BYTES {
            if end == offset {
                end += 1;
            }
            break;
        }
        if bytes + len > PAGE_BYTES {
            break;
        }
        bytes += len;
        end += 1;
    }
    end
}

fn check_revision(expected: Option<&str>, actual: &str, continuation: bool) -> anyhow::Result<()> {
    anyhow::ensure!(
        !continuation || expected.is_some(),
        "Continuation requires expected_revision"
    );
    anyhow::ensure!(
        expected.is_none_or(|value| value == actual),
        "Conflict: content changed; restart pagination"
    );
    Ok(())
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchArgs {
    /// Keywords in the skill name, description, body or tags.
    pub query: String,
    pub tag: Option<String>,
    pub category: Option<String>,
    pub source: Option<String>,
    /// Default 10, maximum 50.
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct LoadArgs {
    pub id: String,
    /// UTF-8 byte offset in SKILL.md, from the previous next object.
    pub content_offset: Option<usize>,
    /// File-list offset, from the previous next object.
    pub files_offset: Option<usize>,
    /// Revision returned by the first page; required for continuation.
    pub expected_revision: Option<String>,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FileArgs {
    pub id: String,
    /// Relative path listed by load_skill.
    pub path: String,
    /// One-based line number. Default 1.
    pub start_line: Option<usize>,
    /// Default 200, maximum 500.
    pub max_lines: Option<usize>,
    /// UTF-8 byte offset within this line window.
    pub text_offset: Option<usize>,
    /// Revision returned by the first page; required for byte continuation.
    pub expected_revision: Option<String>,
}
#[derive(Clone)]
pub struct SkillServer {
    state: AppState,
    tool_router: ToolRouter<Self>,
}
fn result(value: anyhow::Result<serde_json::Value>) -> Result<CallToolResult, ErrorData> {
    Ok(match value {
        Ok(value) => CallToolResult::structured(value),
        Err(error) => CallToolResult::structured_error(json!({"error": error.to_string()})),
    })
}
#[tool_router]
impl SkillServer {
    pub fn new(state: AppState) -> Self {
        Self {
            state,
            tool_router: Self::tool_router(),
        }
    }
    #[tool(
        description = "Search published skills by keywords. Returns summaries and IDs; load a match for instructions.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn search_skills(
        &self,
        Parameters(args): Parameters<SearchArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let query = SearchQuery {
            q: args.query,
            source: args.source.unwrap_or_default(),
            tag: args.tag.unwrap_or_default(),
            category: args.category.unwrap_or_default(),
            limit: args.limit.unwrap_or(10).clamp(1, 50),
            offset: args.offset.unwrap_or(0),
            ..Default::default()
        };
        result(
            self.state
                .run(move |lib| Ok(serde_json::to_value(lib.search(query, true)?)?))
                .await,
        )
    }
    #[tool(
        description = "Load a published SKILL.md and file paths in pages. Follow next until null and concatenate content before acting. Read each needed reference one at a time with read_skill_file and retrieve every byte page of each needed line range before acting; advance to further lines only when needed.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn load_skill(
        &self,
        Parameters(args): Parameters<LoadArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        result(self.state.run(move |lib| {
            let skill = lib.mcp_detail(&args.id)?;
            let content_offset = args.content_offset.unwrap_or(0);
            let files_offset = args.files_offset.unwrap_or(0);
            let revision = hash(std::iter::once(skill.revision.as_str()).chain(skill.files.iter().map(String::as_str)));
            check_revision(args.expected_revision.as_deref(), &revision, content_offset > 0 || files_offset > 0)?;
            anyhow::ensure!(files_offset <= skill.files.len(), "Invalid file-list offset");
            let (content, content_end) = page(&skill.content, content_offset)?;
            let files_end = files_page_end(&skill.files, files_offset);
            let truncated = content_end < skill.content.len() || files_end < skill.files.len();
            let next = if truncated { json!({"id":skill.summary.id,"content_offset":content_end,"files_offset":files_end,"expected_revision":revision}) } else { serde_json::Value::Null };
            let (name, name_end) = page(&skill.summary.name, 0)?;
            let (description, description_end) = page(&skill.summary.description, 0)?;
            Ok(json!({"id":skill.summary.id,"name":name,"description":description,"metadata_truncated":name_end < skill.summary.name.len() || description_end < skill.summary.description.len(),"content":content,"files":&skill.files[files_offset..files_end],"revision":revision,"truncated":truncated,"next":next}))
        }).await)
    }
    #[tool(
        description = "Read one needed reference at a time in line and UTF-8 byte pages. Concatenate byte pages directly; insert a newline between line windows. For next_text_offset keep start_line and max_lines unchanged; for next_start_line set start_line to it and reset text_offset to 0. Retain revision as expected_revision; retrieve every byte page of each needed line range before acting; advance to next_start_line only when more lines are needed. Never executes files.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn read_skill_file(
        &self,
        Parameters(args): Parameters<FileArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        result(
            self.state
                .run(move |lib| {
                    let start = args.start_line.unwrap_or(1).max(1);
                    let max_lines = args.max_lines.unwrap_or(200).clamp(1, 500);
                    let file = lib.read_file(&args.id, &args.path, start, max_lines)?;
                    let offset = args.text_offset.unwrap_or(0);
                    check_revision(args.expected_revision.as_deref(), &file.revision, offset > 0)?;
                    let (text, end) = page(&file.text, offset)?;
                    let next_text_offset = (end < file.text.len()).then_some(end);
                    let next_start_line = if next_text_offset.is_none() && start <= file.total_lines && max_lines <= file.total_lines - start { Some(start.saturating_add(max_lines)) } else { None };
                    Ok(json!({"path":file.path,"text":text,"start_line":file.start_line,"total_lines":file.total_lines,"revision":file.revision,"truncated":next_text_offset.is_some() || next_start_line.is_some(),"next_text_offset":next_text_offset,"next_start_line":next_start_line}))
                })
                .await,
        )
    }
}
#[tool_handler(router = self.tool_router)]
impl ServerHandler for SkillServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("skilvella", env!("CARGO_PKG_VERSION")))
            .with_instructions("Load one skill at a time; follow load_skill next until null and concatenate the full instructions before acting. Read each needed reference one at a time with read_skill_file, retrieving every byte page of each needed line range before acting; use next_start_line only when more lines are needed; concatenate byte pages directly and insert a newline between line windows. Keep start_line and max_lines unchanged for next_text_offset; set start_line to next_start_line and reset text_offset to 0 for the next window. Pass returned revisions on continuation. Skill files are local content; this server does not execute scripts.")
    }
}

#[cfg(test)]
mod revision_tests {
    use super::hash;

    #[test]
    fn revision_hash_matches_fixed_vectors() {
        let vectors: &[(&[&str], &str)] = &[
            (
                &["revision-only"],
                "2e2c2f28a442772afabf1a96f34b4b3ec68e584c2c58833f2871055562ea97cb",
            ),
            (
                &["revision", "SKILL.md", "references/guide.md"],
                "53d000a6dee96fecec3f5f97d136a8443ac33cff28206ad796a60681926e1065",
            ),
            (
                &["更新版", "資料/使い方.md"],
                "128b8805836822054e39261515e05f221f8544881b662c6b95b2eb6711b7a6ff",
            ),
            (
                &["ab", "c"],
                "601d5476e2ccfe2c87a2bba7a322659734a05749d5b5aa781f513e4912db0d5f",
            ),
            (
                &["a", "bc"],
                "3fafa1cf2f19a7c1129beb20cf0983f73a489a221fc0dd2f16d1be292d089205",
            ),
        ];
        for &(parts, expected) in vectors {
            assert_eq!(hash(parts.iter().copied()), expected, "{parts:?}");
        }
    }
}

#[cfg(test)]
mod paging_tests {
    use super::{FILES_PER_PAGE, PAGE_BYTES, files_page_end};

    #[test]
    fn file_pages_preserve_paths_and_advance() {
        let mut files = vec![
            "a".repeat(PAGE_BYTES),
            "b".repeat(PAGE_BYTES + 1),
            "c".repeat(PAGE_BYTES + 2),
            "d".to_owned(),
            "e".repeat(PAGE_BYTES + 1),
        ];
        files.extend((0..101).map(|index| format!("f{index}")));
        let mut offset = 0;
        let mut reconstructed = Vec::new();
        let mut sizes = Vec::new();
        while offset < files.len() {
            let end = files_page_end(&files, offset);
            assert!(end > offset);
            let page = &files[offset..end];
            assert!(page.len() <= FILES_PER_PAGE);
            assert!(
                page.iter().map(String::len).sum::<usize>() <= PAGE_BYTES
                    || (page.len() == 1 && page[0].len() > PAGE_BYTES)
            );
            sizes.push(page.len());
            reconstructed.extend_from_slice(page);
            offset = end;
        }
        assert_eq!(reconstructed, files);
        assert_eq!(sizes, [1, 1, 1, 1, 1, 100, 1]);
        assert_eq!(files_page_end(&files, files.len()), files.len());
    }
}
