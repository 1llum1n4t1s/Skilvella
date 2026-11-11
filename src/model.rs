use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexSkillPolicy {
    pub automatic: bool,
    pub command: bool,
}

/// 呼び出し入口の同期状態。設定はクライアントごとに別キーへ保存する。
/// 名前は互換のためCodex時代のまま残し、Claude Code・Grokでも同じ形を使う。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CodexSyncSettings {
    pub enabled: bool,
    /// 生成物（SKILL.md群）を置く所有フォルダー。表示用で保存値は読み出し時に再計算する。
    pub path: String,
    /// 探索先へリンクを張るクライアント（Claude Code）のリンク置き場。表示用。
    #[serde(default)]
    pub link_path: String,
    pub last_error: String,
    pub exported: usize,
}

/// スキル呼び出し入口の同期先クライアント。
///
/// - Codex: `~/.agents/skills/skilvella/` へ実ファイル（入れ子を探索するため一括配置できる）
/// - Claude Code: `~/.claude/skills/` 直下1階層しか探索しないため、生成物を
///   `~/.claude/skilvella/skills/` に置き、`~/.claude/skills/<name>` へリンクを張る
/// - Grok: `~/.grok/skills/skilvella/` へ実ファイル（入れ子を探索する）
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillClient {
    Codex,
    Claude,
    Grok,
}

impl SkillClient {
    /// 同期・表示の順序。Codexを先頭に置き、既存の表示順を保つ。
    pub const ALL: [Self; 3] = [Self::Codex, Self::Claude, Self::Grok];

    /// フォーム・JSONで使う識別子。
    pub fn key(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Grok => "grok",
        }
    }

    /// 画面表示名（製品名のため翻訳しない）。
    pub fn label(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Claude => "Claude Code",
            Self::Grok => "Grok",
        }
    }
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
    pub available: usize,
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
    pub revision: String,
    pub path: String,
    pub text: String,
    pub start_line: usize,
    pub total_lines: usize,
}

/// フォルダー登録で利用者に原因と対処を示す必要がある失敗。
///
/// 管理画面はこの型を取り出して翻訳済みの文言に置き換える。英語の `Display` は
/// ログ・テスト・MCP以外の呼び出し元向けで、既存テストが参照する語（`overlap`）を保つ。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FolderError {
    /// 同じフォルダーが既に管理中。`name` は管理中の表示名。
    AlreadyRegistered { name: String },
    /// 管理中のフォルダーと親子関係にある。`name` / `path` は重なる側のフォルダー。
    Overlaps { name: String, path: String },
    /// Skilvellaのデータフォルダーと重なる。
    OverlapsData,
}

impl std::fmt::Display for FolderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyRegistered { name } => {
                write!(f, "This folder is already managed as \"{name}\"")
            }
            Self::Overlaps { name, path } => write!(
                f,
                "Source directories overlap with \"{name}\" ({path}); remove one of them first"
            ),
            Self::OverlapsData => {
                f.write_str("Source and library data directories must not overlap")
            }
        }
    }
}

impl std::error::Error for FolderError {}
