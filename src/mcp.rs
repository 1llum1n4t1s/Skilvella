use crate::{AppState, model::SearchQuery};
use rmcp::{
    ErrorData, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Implementation, ServerCapabilities, ServerConfig},
    schemars, tool, tool_handler, tool_router,
};
use serde::Deserialize;
use serde_json::json;

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
        description = "Load one published SKILL.md and its available relative file paths.",
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
            let skill = lib.detail(&args.id, true)?;
            Ok(json!({"id":skill.summary.id,"name":skill.summary.name,"description":skill.summary.description,"content":skill.content,"files":skill.files}))
        }).await)
    }
    #[tool(
        description = "Read a text file within a published skill, with bounded line pagination. Never executes files.",
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
                    Ok(serde_json::to_value(lib.read_file(
                        &args.id,
                        &args.path,
                        args.start_line.unwrap_or(1).max(1),
                        args.max_lines.unwrap_or(200).clamp(1, 500),
                    )?)?)
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
            .with_instructions("Search skills, load relevant instructions, then read referenced files as needed. Skill files are local content; this server does not execute scripts.")
    }
}
