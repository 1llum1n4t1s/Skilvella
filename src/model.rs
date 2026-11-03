use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexSkillPolicy {
    pub automatic: bool,
    pub command: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CodexSyncSettings {
    pub enabled: bool,
    pub path: String,
    pub last_error: String,
    pub exported: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DirectorySettings {
    pub mode: String,
    pub path: String,
    pub custom_path: String,
    pub detected_path: String,
    pub source_alias: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Source {
    pub alias: String,
    pub display_name: String,
    pub path: String,
    pub skill_count: usize,
    pub error_count: usize,
    pub native_discovery: bool,
    pub native_discovery_unknown: bool,
}

impl Source {
    pub fn label(&self) -> String {
        Self::format_label(&self.alias, &self.display_name)
    }

    pub(crate) fn format_label(alias: &str, display_name: &str) -> String {
        if display_name == alias {
            alias.to_owned()
        } else {
            format!("{display_name} ({alias})")
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SkillSummary {
    pub id: String,
    pub source: String,
    pub relative_path: String,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub category: String,
    pub published: bool,
    pub error: Option<String>,
    pub duplicate: bool,
    pub missing: bool,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub created_at: String,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SkillDetail {
    pub codex_policy: CodexSkillPolicy,
    pub summary: SkillSummary,
    pub needs_rescan: bool,
    pub content: String,
    pub revision: String,
    pub files: Vec<String>,
    pub history: Vec<HistoryEntry>,
}

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(default)]
pub struct SearchQuery {
    pub q: String,
    pub source: String,
    pub tag: String,
    pub category: String,
    pub status: String,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(default)]
pub struct RepositorySelection {
    pub repository_source: String,
    pub repository: String,
}

#[derive(Clone, Debug)]
pub struct RepositoryGroup {
    pub source: String,
    pub repository: String,
    pub count: usize,
    pub missing: usize,
    pub unknown: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct SearchResults {
    pub skills: Vec<SkillSummary>,
    pub total: usize,
    pub indexed_at: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ScanReport {
    pub indexed: usize,
    pub errors: usize,
    pub missing: usize,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SkillMetadata {
    pub tags: Vec<String>,
    pub category: String,
    pub published: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct SkillFile {
    pub path: String,
    pub text: String,
    pub start_line: usize,
    pub total_lines: usize,
}
