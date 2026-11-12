use crate::model::*;
mod codex_index {
    use super::{
        CONTENT_LIMIT, Library, atomic_write, parse, put, read_text, reject_links, revision,
        safe_join, setting, setting_json,
    };
    use crate::model::{CodexSkillPolicy, CodexSyncSettings, SkillClient};
    use anyhow::{Context, Result, bail, ensure};
    use rusqlite::{Connection, OptionalExtension};
    use serde::{Deserialize, Serialize};
    use std::{
        collections::{BTreeMap, BTreeSet, HashSet},
        fs,
        io::Write,
        path::{Path, PathBuf},
    };

    #[derive(Clone, Default, Serialize, Deserialize)]
    struct Manifest {
        owner: PathBuf,
        root: PathBuf,
        files: BTreeMap<String, String>,
    }
    #[derive(Serialize, Deserialize)]
    struct Pending {
        previous: Manifest,
        next: Manifest,
        stage: PathBuf,
        retired: PathBuf,
    }
    #[derive(Serialize, Deserialize)]
    struct Staged {
        manifest: Manifest,
        path: PathBuf,
    }
    struct Entry {
        name: String,
        description: String,
        source: String,
        relative_path: String,
    }
    /// 入口生成に必要な索引の列（`export_codex` の主クエリ1本で読む）。
    struct EntryRow {
        source: String,
        relative_path: String,
        revision: String,
        root: String,
    }

    // 個別の入口と検索の入口で、直接接続・集約接続の案内を共通化する。
    const MCP_ACCESS: &str = "MCP tool access: use the connected Skilvella tools directly when available, including their advertised namespace. If Toolport is available, use toolport_search_tools to discover each required capability (query: load_skill, read_skill_file, or search_skills), then invoke the exact returned tool name with toolport_call_tool, passing the tool parameters inside arguments. Do not guess a server prefix or treat a tool missing from the initial tool list as a disconnected MCP. If the Toolport helpers are deferred, discover them in the available tool inventory first. Only report MCP as unavailable after tool discovery or an actual MCP request fails; include that failure. Load one skill at a time; follow load_skill next until null and concatenate all content before acting. Read each needed reference one at a time with read_skill_file, retrieving every byte page of each needed line range before acting and using next_start_line only when more lines are needed; concatenate byte pages directly and insert a newline between line windows; keep start_line and max_lines unchanged for next_text_offset, then set start_line to next_start_line and reset text_offset to 0; pass returned revisions on continuation. Never replace MCP retrieval with local skill files.\n";

    // settingsの読書き（setting / setting_json / put）は親モジュールで共用する。
    /// クライアントごとの同期設定キー。Codexは旧版と同じキーを使い、既存設定をそのまま引き継ぐ。
    fn settings_key(client: SkillClient) -> &'static str {
        match client {
            SkillClient::Codex => "codex_sync",
            SkillClient::Claude => "claude_sync",
            SkillClient::Grok => "grok_sync",
        }
    }
    /// 所有記録（manifest・中断記録）を置くデータフォルダー内の名前。Codexは旧版と共通。
    fn state_name(client: SkillClient) -> &'static str {
        match client {
            SkillClient::Codex => "codex-index",
            SkillClient::Claude => "claude-index",
            SkillClient::Grok => "grok-index",
        }
    }
    /// 生成先として受け入れるパスの末尾。記録の改ざん・破損で無関係な場所を
    /// 退避・削除しないよう、復旧・清掃の前にも必ず照合する。
    fn root_suffix(client: SkillClient) -> &'static str {
        match client {
            SkillClient::Codex => ".agents/skills/skilvella",
            SkillClient::Claude => ".claude/skilvella/skills",
            SkillClient::Grok => ".grok/skills/skilvella",
        }
    }
    fn valid_root(client: SkillClient, root: &Path) -> bool {
        root.is_absolute() && root.ends_with(root_suffix(client))
    }
    fn load_settings(c: &Connection, client: SkillClient) -> Result<CodexSyncSettings> {
        Ok(setting_json(c, settings_key(client))?.unwrap_or_default())
    }
    /// 生成物の所有フォルダー。ホームは起動時に記録した `~/.agents/skills` から求め、
    /// Codexと同じ利用者ホームを全クライアントで共有する。
    fn target(c: &Connection, client: SkillClient) -> Result<Option<PathBuf>> {
        let roots: Vec<PathBuf> = setting_json(c, "native_skill_roots")?.unwrap_or_default();
        match roots.first() {
            None => Ok(None),
            Some(root) => {
                ensure!(
                    root.is_absolute() && root.ends_with(".agents/skills"),
                    "Skill entry export requires the recorded .agents/skills root"
                );
                let home = root
                    .parent()
                    .and_then(Path::parent)
                    .context("Invalid native skill root")?;
                Ok(Some(match client {
                    SkillClient::Codex => root.join("skilvella"),
                    // Claude Codeは `~/.claude/skills` 直下しか探索しないため、生成物は
                    // 探索外に置き、リンクだけを探索先へ出す。
                    SkillClient::Claude => home.join(".claude").join("skilvella").join("skills"),
                    SkillClient::Grok => home.join(".grok").join("skills").join("skilvella"),
                }))
            }
        }
    }
    /// 探索先へ1スキルずつリンクを張るクライアントのリンク置き場。
    fn link_dir(root: &Path, client: SkillClient) -> Option<PathBuf> {
        match client {
            // `~/.claude/skilvella/skills` → `~/.claude/skills`
            SkillClient::Claude => root
                .parent()
                .and_then(Path::parent)
                .map(|claude| claude.join("skills")),
            SkillClient::Codex | SkillClient::Grok => None,
        }
    }
    impl Library {
        /// 全クライアントの生成入口（SKILL.md）を、原本の走査から除外するための目印として集める。
        /// 戻り値は (既知の入口, 所有記録が確認できない, 未確認の出力領域)。
        /// Codexだけでなく、Claude Code・Grokの生成物が管理フォルダー内に入っても
        /// 原本として登録しないよう、同じ検証を各クライアントの所有記録に適用する。
        pub(super) fn owned_codex_markers(&self) -> (HashSet<PathBuf>, bool, HashSet<PathBuf>) {
            let mut markers = HashSet::new();
            let mut uncertain = false;
            let mut regions = HashSet::new();
            for client in SkillClient::ALL {
                let (owned, unsure, region) = self.owned_client_markers(client);
                markers.extend(owned);
                uncertain |= unsure;
                regions.extend(region);
            }
            (markers, uncertain, regions)
        }
        fn owned_client_markers(
            &self,
            client: SkillClient,
        ) -> (HashSet<PathBuf>, bool, HashSet<PathBuf>) {
            let Some(parent) = self.db.parent() else {
                return (HashSet::new(), true, HashSet::new());
            };
            let state = parent.join(state_name(client));
            let read_record = |name: &str| -> Result<String> {
                let path = state.join(name);
                reject_links(&path)?;
                read_text(&path, CONTENT_LIMIT)
            };
            let mut regions = HashSet::new();
            let mut checked_markers = |manifest: &Manifest| -> Result<(HashSet<PathBuf>, bool)> {
                ensure!(
                    manifest.owner == self.db,
                    "Foreign skill entry ownership manifest"
                );
                if manifest.root.as_os_str().is_empty() {
                    return Ok((HashSet::new(), false));
                }
                ensure!(
                    valid_root(client, &manifest.root),
                    "Invalid skill entry ownership manifest root"
                );
                reject_links(&manifest.root)?;
                let root = super::migration::normalize_path(&manifest.root)?;
                regions.insert(root.clone());
                let mut markers = HashSet::new();
                let mut invalid = false;
                for relative in manifest
                    .files
                    .keys()
                    .filter(|p| p.ends_with("/SKILL.md") || *p == "SKILL.md")
                {
                    if !relative.split('/').all(|part| {
                        !part.is_empty()
                            && part != "."
                            && part != ".."
                            && !part.ends_with(['.', ' '])
                            && !part.contains(['\\', ':', '\0'])
                    }) {
                        invalid = true;
                        continue;
                    }
                    let path = root.join(relative);
                    // リンクを辿らず既知のパスを保持し、他の正常な入口も失効させない。
                    markers.insert(path.clone());
                    if reject_links(&path).is_ok()
                        && let Ok(canonical) = super::migration::normalize_path(&path)
                    {
                        markers.insert(canonical);
                    }
                }
                Ok((markers, invalid))
            };
            let valid_location = |path: &Path, manifest: &Manifest, category: &str| {
                if manifest.root.as_os_str().is_empty() {
                    return path.as_os_str().is_empty();
                }
                // 祖父フォルダーが無い（不正なroot）場合は従来どおり不一致として扱う。
                manifest
                    .root
                    .parent()
                    .and_then(Path::parent)
                    .is_some_and(|p| is_area_entry(&p.join(category), path))
                    && reject_links(path).is_ok()
            };
            let exists = |path: &Path| -> Result<bool> {
                if path.as_os_str().is_empty() {
                    Ok(false)
                } else {
                    Ok(path.try_exists()?)
                }
            };
            let mut markers = HashSet::new();
            let mut uncertain = false;
            let mut previous_retired = false;
            match read_record("pending.json")
                .and_then(|text| Ok(serde_json::from_str::<Pending>(&text)?))
            {
                Ok(pending) => {
                    let mut inspect = || -> Result<(HashSet<PathBuf>, bool, bool)> {
                        let (previous, previous_invalid) = checked_markers(&pending.previous)?;
                        let (next, next_invalid) = checked_markers(&pending.next)?;
                        ensure!(
                            valid_location(&pending.stage, &pending.next, "skilvella-staging")
                                && valid_location(
                                    &pending.retired,
                                    &pending.previous,
                                    "skilvella-retired"
                                ),
                            "Invalid skill entry publication paths"
                        );
                        let retired = exists(&pending.retired)?;
                        let mut owned = if retired { HashSet::new() } else { previous };
                        if !exists(&pending.stage)? && pending.next.root.is_dir() {
                            owned.extend(next);
                        }
                        Ok((owned, retired, previous_invalid || next_invalid))
                    };
                    match inspect() {
                        Ok((owned, retired, invalid)) => {
                            markers.extend(owned);
                            previous_retired = retired;
                            uncertain |= invalid;
                        }
                        Err(_) => uncertain = true,
                    }
                }
                Err(error) => {
                    uncertain |= !error
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound)
                }
            }
            // 旧出力が退避済みなら、旧manifestの場所にできた別ファイルを隠さない。
            if !previous_retired {
                match read_record("manifest.json")
                    .and_then(|text| Ok(serde_json::from_str::<Manifest>(&text)?))
                    .and_then(|manifest| checked_markers(&manifest))
                {
                    Ok((owned, invalid)) => {
                        markers.extend(owned);
                        uncertain |= invalid;
                    }
                    Err(error) => {
                        uncertain |= !error
                            .downcast_ref::<std::io::Error>()
                            .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound)
                    }
                }
            }
            if uncertain
                && let Ok(c) = self.connection()
                && let Ok(Some(root)) = target(&c, client)
            {
                // native親は記録時に正規化済み。子を再解決してリンク先を巻き込まない。
                regions.insert(root);
            }
            (markers, uncertain, regions)
        }

        pub(super) fn codex_policy_on(c: &Connection, id: &str) -> Result<CodexSkillPolicy> {
            Ok(setting_json(c, &format!("codex_policy:{id}"))?.unwrap_or_default())
        }
        fn ensure_skill_on(c: &Connection, id: &str) -> Result<()> {
            ensure!(
                c.query_row(
                    "SELECT EXISTS(SELECT 1 FROM skills WHERE id=?1)",
                    [id],
                    |r| r.get::<_, bool>(0)
                )?,
                "Skill not found"
            );
            Ok(())
        }
        pub fn codex_policy(&self, id: &str) -> Result<CodexSkillPolicy> {
            let c = self.connection()?;
            Self::ensure_skill_on(&c, id)?;
            Self::codex_policy_on(&c, id)
        }
        pub(crate) fn codex_policies(
            &self,
            ids: &[String],
        ) -> Result<BTreeMap<String, Option<CodexSkillPolicy>>> {
            if ids.is_empty() {
                return Ok(BTreeMap::new());
            }
            let c = self.connection()?;
            let mut statement = c.prepare("SELECT value FROM settings WHERE key=?1")?;
            ids.iter()
                .map(|id| {
                    let value: Option<String> = statement
                        .query_row([format!("codex_policy:{id}")], |row| row.get(0))
                        .optional()?;
                    let policy = match value {
                        None => Some(CodexSkillPolicy::default()),
                        Some(value) => serde_json::from_str(&value).ok(),
                    };
                    Ok((id.clone(), policy))
                })
                .collect()
        }
        pub fn set_codex_policy(&self, id: &str, automatic: bool, command: bool) -> Result<()> {
            ensure!(
                !automatic || command,
                "Automatic invocation requires a visible command"
            );
            let _guard = self.write_guard()?;
            self.ensure_no_pending_migration()?;
            let c = self.connection()?;
            Self::ensure_skill_on(&c, id)?;
            put(
                &c,
                &format!("codex_policy:{id}"),
                &serde_json::to_string(&CodexSkillPolicy { automatic, command })?,
            )?;
            let _ = self.sync_codex_locked(&c);
            Ok(())
        }
        /// Codexの同期状態（旧版互換のAPI）。
        pub fn codex_settings(&self) -> Result<CodexSyncSettings> {
            self.client_settings(SkillClient::Codex)
        }
        /// クライアントごとの同期状態。生成先・リンク置き場は保存値でなく現在のホームから求める。
        pub fn client_settings(&self, client: SkillClient) -> Result<CodexSyncSettings> {
            Self::client_settings_on(&self.connection()?, client)
        }
        /// `client_settings` の接続共有版。全クライアント分を読む `sync_summary` が
        /// クライアントごとに接続を開き直さないよう、呼び出し元の接続で読む（読み取りのみ）。
        fn client_settings_on(c: &Connection, client: SkillClient) -> Result<CodexSyncSettings> {
            let mut settings = load_settings(c, client)?;
            let root = target(c, client)?;
            settings.path = root
                .as_deref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            settings.link_path = root
                .as_deref()
                .and_then(|root| link_dir(root, client))
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            Ok(settings)
        }
        /// 全クライアントの同期状態を、一覧・詳細画面の警告やJSON応答用に1件へまとめる。
        /// `enabled` はいずれかが有効、`last_error` は失敗したクライアント名付きで連結する。
        pub fn sync_summary(&self) -> Result<CodexSyncSettings> {
            let mut summary = CodexSyncSettings::default();
            let mut errors = Vec::new();
            // 公開シグネチャは保ったまま、全クライアントを1本の接続で読む。
            let c = self.connection()?;
            for client in SkillClient::ALL {
                let settings = Self::client_settings_on(&c, client)?;
                summary.enabled |= settings.enabled;
                summary.exported += settings.exported;
                if !settings.last_error.is_empty() {
                    // どの同期先で失敗したか分かるよう、全クライアントを同じ形式
                    // （`{表示名}: {エラー}`）で並べる。特定のクライアントを特別扱いしない。
                    errors.push(format!("{}: {}", client.label(), settings.last_error));
                }
            }
            summary.last_error = errors.join(
                "
",
            );
            Ok(summary)
        }
        /// Codexだけを切り替える旧版互換の操作。他クライアントの設定は変えない。
        pub fn set_codex_enabled(&self, enabled: bool) -> Result<()> {
            self.set_clients_enabled(&[(SkillClient::Codex, enabled)])
        }
        /// 指定したクライアントの有効・無効を保存し、全クライアントを1回同期する。
        /// 同期失敗は各クライアントの `last_error` に残し、設定の保存自体は取り消さない。
        pub fn set_clients_enabled(&self, changes: &[(SkillClient, bool)]) -> Result<()> {
            let _guard = self.write_guard()?;
            self.ensure_no_pending_migration()?;
            let c = self.connection()?;
            for (client, enabled) in changes {
                let mut settings = load_settings(&c, *client)?;
                settings.enabled = *enabled;
                put(
                    &c,
                    settings_key(*client),
                    &serde_json::to_string(&settings)?,
                )?;
            }
            let _ = self.sync_codex_locked(&c);
            Ok(())
        }
        pub fn sync_codex_entries(&self) -> Result<()> {
            let _guard = self.write_guard()?;
            self.ensure_no_pending_migration()?;
            self.sync_codex_locked(&self.connection()?)
        }
        /// 全クライアントを順に同期する。1件の失敗で他のクライアントを止めず、最初の失敗を返す。
        fn sync_codex_locked(&self, c: &Connection) -> Result<()> {
            let mut first_error = None;
            for client in SkillClient::ALL {
                if let Err(error) = self.sync_client_locked(c, client) {
                    first_error.get_or_insert(error);
                }
            }
            first_error.map_or(Ok(()), Err)
        }
        fn sync_client_locked(&self, c: &Connection, client: SkillClient) -> Result<()> {
            // 保存済みの生文字列を1回だけ読み、ここで解析する（JSON不正は従来どおりErr）。
            // 最後に同じ文字列なら書き込まず、変化のない同期でDBを更新しない。
            let saved = setting(c, settings_key(client))?;
            let mut settings: CodexSyncSettings = saved
                .as_deref()
                .map(serde_json::from_str)
                .transpose()?
                .unwrap_or_default();
            let exported = self.export_codex(c, client, settings.enabled);
            // リンク方式のクライアントは、生成物の公開結果（成功・復旧後のどちらでも）に
            // 合わせてリンクを揃える。生成に失敗した場合も、現在の所有記録との整合を保つ。
            let linked = self.sync_client_links(c, client);
            match (&exported, &linked) {
                (Ok(count), Ok(())) => {
                    settings.exported = *count;
                    settings.last_error.clear();
                }
                (Ok(count), Err(error)) => {
                    settings.exported = *count;
                    settings.last_error = format!("{error:#}");
                }
                (Err(error), _) => {
                    settings.last_error = format!("{error:#}");
                }
            }
            let next = serde_json::to_string(&settings)?;
            // キー未作成（None）または内容が変わった場合だけ書く。値・last_error・exported・
            // 戻り値は従来と同じで、省くのは同一内容の上書き（不要なDB更新）だけ。
            if saved.as_deref() != Some(next.as_str()) {
                put(c, settings_key(client), &next)?;
            }
            exported.and(linked)
        }
        fn export_codex(
            &self,
            c: &Connection,
            client: SkillClient,
            enabled: bool,
        ) -> Result<usize> {
            let root = target(c, client)?;
            ensure!(
                !enabled || root.is_some(),
                "Native skill root is not configured"
            );
            let state = self.client_state(client)?;
            reject_links(&state)?;
            let manifest_path = state.join("manifest.json");
            reject_links(&manifest_path)?;
            self.recover_codex(c, &state, client)?;
            self.cleanup_codex(c, &state, client)?;
            let old: Manifest = if manifest_path.exists() {
                serde_json::from_str(&read_text(&manifest_path, CONTENT_LIMIT)?)?
            } else {
                Manifest {
                    owner: self.db.clone(),
                    ..Default::default()
                }
            };
            ensure!(
                old.owner == self.db,
                "Skill entry export belongs to another library"
            );
            if enabled {
                let root = root.as_deref().context("Missing skill entry root")?;
                ensure_source_boundaries(c, root)?;
                if let Some(links) = link_dir(root, client) {
                    // リンク置き場が管理フォルダーと重なると、入口を原本として再登録してしまう。
                    ensure_outside_sources(c, &links)?;
                }
            }
            if !old.root.as_os_str().is_empty() {
                ensure_source_boundaries(c, &old.root)?;
            }
            let mut desired = BTreeMap::new();
            let mut count = 0;
            if enabled {
                // 接続できないソースを削除済みと判断しない。
                let mut sources = c.prepare("SELECT path FROM sources WHERE active=1")?;
                for source in sources.query_map([], |r| r.get::<_, String>(0))? {
                    let source = PathBuf::from(source?);
                    reject_links(&source)?;
                    fs::read_dir(&source).with_context(|| {
                        format!("Skill source unavailable: {}", source.display())
                    })?;
                }
                let finder = format!(
                    "---\nname: skilvella-search\ndescription: Search the published Skilvella skill library on demand.\n---\nUse the Skilvella MCP search_skills tool to find relevant skills, then load_skill with the returned id. Fetch references only with read_skill_file after loading.\n\n{MCP_ACCESS}"
                );
                desired.insert(
                    "skilvella-search/SKILL.md".into(),
                    frontmatter_policy(client, finder, false),
                );
                if client == SkillClient::Codex {
                    desired.insert(
                        "skilvella-search/agents/openai.yaml".into(),
                        "policy:\n  allow_implicit_invocation: false\n".into(),
                    );
                }
                let mut duplicate_names = None;
                // 公開スキルごとにポリシーと入口情報を個別に問い合わせていた（N+1）ため、
                // 1本のクエリでポリシーの生JSON（未設定はNULL）と入口生成に必要な列を読む。
                // 絞込み条件・ORDER BY k.id は従来の主クエリと入口取得クエリと同一に保つ。
                // ポリシーの解析は行ごとにループ内で行い、不正JSONのErrが出る位置と順序を保つ。
                let mut skills = c.prepare("SELECT k.id,p.value,k.source,k.relative_path,k.revision,s.path FROM skills k JOIN sources s ON s.alias=k.source LEFT JOIN settings p ON p.key='codex_policy:'||k.id WHERE s.active=1 AND k.published=1 AND k.missing=0 AND k.error IS NULL ORDER BY k.id")?;
                let rows = skills.query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<String>>(1)?,
                        EntryRow {
                            source: r.get(2)?,
                            relative_path: r.get(3)?,
                            revision: r.get(4)?,
                            root: r.get(5)?,
                        },
                    ))
                })?;
                for row in rows {
                    let (id, policy, row) = row?;
                    // `codex_policy_on` と同じ解釈（未設定は既定値、不正JSONはErr）。
                    let policy: CodexSkillPolicy = policy
                        .as_deref()
                        .map(serde_json::from_str)
                        .transpose()?
                        .unwrap_or_default();
                    if !policy.command {
                        continue;
                    }
                    // コマンドを出すスキルだけ原本を読む（従来どおりポリシー判定の後）。
                    let Some(entry) = Self::codex_entry(row)? else {
                        continue;
                    };
                    let original = &entry.name;
                    let valid_name = !original.is_empty()
                        && original.len() <= 64
                        && !original.contains("--")
                        && !original.starts_with('-')
                        && !original.ends_with('-')
                        && original
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
                    let suffix = &id[..id.len().min(8)];
                    let mut name = if valid_name {
                        original.clone()
                    } else {
                        format!("skill-{suffix}")
                    };
                    let duplicates = if let Some(names) = &duplicate_names {
                        names
                    } else {
                        let mut statement = c.prepare("SELECT k.name FROM skills k JOIN sources s ON s.alias=k.source WHERE s.active=1 AND k.published=1 AND k.missing=0 AND k.error IS NULL GROUP BY k.name HAVING count(*)>1")?;
                        duplicate_names.insert(
                            statement
                                .query_map([], |r| r.get::<_, String>(0))?
                                .collect::<rusqlite::Result<HashSet<_>>>()?,
                        )
                    };
                    let duplicates = duplicates.contains(original);
                    if name == "skilvella-search" || duplicates {
                        name.truncate(55);
                        name = format!("{}-{suffix}", name.trim_end_matches('-'));
                    }
                    ensure!(
                        !desired.contains_key(&format!("{name}/SKILL.md")),
                        "Skill entry command name collision: {name}"
                    );
                    let description = entry.description.replace('<', "(").replace('>', ")");
                    let description = if description.trim_start().starts_with("[TODO:") {
                        format!("Skilvella: {description}")
                    } else {
                        description
                    };
                    let description: String = description.chars().take(1024).collect();
                    let text = format!(
                        "---\nname: {}\ndescription: {}\n---\nUse Skilvella MCP load_skill with the fixed id {} before following this skill. Read references only through read_skill_file with this id.\n\n{MCP_ACCESS}",
                        serde_json::to_string(&name)?,
                        serde_json::to_string(&description)?,
                        serde_json::to_string(&id)?
                    );
                    desired.insert(
                        format!("{name}/SKILL.md"),
                        frontmatter_policy(client, text, policy.automatic),
                    );
                    if client != SkillClient::Codex {
                        // Claude Code・GrokはSKILL.mdのfrontmatterで自動選択を制御する。
                        count += 1;
                        continue;
                    }
                    desired.insert(
                        format!("{name}/agents/openai.yaml"),
                        format!(
                            "interface:\n  display_name: {}\npolicy:\n  allow_implicit_invocation: {}\n",
                            serde_json::to_string(&if duplicates {
                                format!("{} ({}/{}, {})", original, entry.source, entry.relative_path, suffix)
                            } else { original.clone() })?,
                            policy.automatic
                        ),
                    );
                    count += 1;
                }
            }
            let hashes = desired
                .iter()
                .map(|(p, text): (&String, &String)| (p.clone(), revision(text)))
                .collect();
            let next = Manifest {
                owner: self.db.clone(),
                root: if enabled {
                    root.unwrap_or_default()
                } else {
                    PathBuf::new()
                },
                files: hashes,
            };
            if !old.root.as_os_str().is_empty() {
                ensure!(
                    old.owner == self.db,
                    "Skill entry export belongs to another library"
                );
                ensure!(
                    valid_root(client, &old.root),
                    "Invalid skill entry ownership manifest root"
                );
                validate_tree(&old.root, &old.root, &old.files)?;
            }
            if old.root == next.root && old.files == next.files && complete(&old)? {
                return Ok(count);
            }
            if !next.files.is_empty() && next.root != old.root {
                reject_links(&next.root)?;
                ensure!(
                    !next.root.try_exists()?,
                    "Skill entry export destination already exists and is not owned by this library"
                );
            }
            // 探索外で全ファイルを検証するまで、現在の索引には触れない。
            let stage = if next.root.as_os_str().is_empty() {
                PathBuf::new()
            } else {
                outside(&next.root, "skilvella-staging")?
            };
            fs::create_dir_all(&state)?;
            if !stage.as_os_str().is_empty() {
                publish_new(
                    &state.join("staged.json"),
                    &serde_json::to_string(&Staged {
                        manifest: next.clone(),
                        path: stage.clone(),
                    })?,
                )?;
                reject_links(&stage)?;
                fs::create_dir_all(stage.parent().context("Invalid stage")?)?;
                fs::create_dir(&stage)?;
                for (relative, text) in desired {
                    let path = stage.join(relative);
                    reject_links(&path)?;
                    fs::create_dir_all(path.parent().context("Invalid skill entry")?)?;
                    write_new(&path, &text)?;
                }
                validate_tree(&stage, &stage, &next.files)?;
                ensure!(
                    complete(&Manifest {
                        root: stage.clone(),
                        ..next.clone()
                    })?,
                    "Incomplete staged skill entry index"
                );
            }
            let retired = if old.root.as_os_str().is_empty() {
                PathBuf::new()
            } else {
                outside(&old.root, "skilvella-retired")?
            };
            if !retired.as_os_str().is_empty() {
                reject_links(&retired)?;
                fs::create_dir_all(retired.parent().context("Invalid retirement path")?)?;
            }
            fs::create_dir_all(&state)?;
            let pending = Pending {
                previous: old,
                next,
                stage,
                retired,
            };
            publish_new(
                &state.join("pending.json"),
                &serde_json::to_string(&pending)?,
            )?;
            let publish = (|| -> Result<()> {
                if !pending.previous.root.as_os_str().is_empty()
                    && pending.previous.root.try_exists()?
                {
                    validate_tree(
                        &pending.previous.root,
                        &pending.previous.root,
                        &pending.previous.files,
                    )?;
                    super::migration::rename_no_replace(&pending.previous.root, &pending.retired)?;
                }
                if !pending.stage.as_os_str().is_empty() {
                    reject_links(&pending.next.root)?;
                    fs::create_dir_all(pending.next.root.parent().context("Invalid export path")?)?;
                    super::migration::rename_no_replace(&pending.stage, &pending.next.root)?;
                }
                save_manifest(&manifest_path, &pending.next)?;
                finish_pending(&state)?;
                Ok(())
            })();
            if let Err(error) = publish {
                self.recover_codex(c, &state, client)
                    .context("Cannot restore previous skill entry index")?;
                return Err(error);
            }
            Ok(count)
        }
        /// 原本の `SKILL.md` を読み、索引のリビジョンと一致する場合だけ入口情報を返す。
        /// 不在・frontmatter不正・リビジョン不一致は `None`、それ以外の読取失敗はErr（従来どおり）。
        /// 行は `export_codex` の主クエリで公開条件を満たしたものだけが渡される。
        fn codex_entry(row: EntryRow) -> Result<Option<Entry>> {
            let EntryRow {
                source,
                relative_path,
                revision: expected,
                root,
            } = row;
            let path = safe_join(Path::new(&root), &relative_path)?;
            let content = match read_text(&path, CONTENT_LIMIT) {
                Ok(content) => content,
                Err(error) => {
                    if error
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|e| e.kind() != std::io::ErrorKind::NotFound)
                    {
                        return Err(error).context("Cannot read source skill");
                    }
                    return Ok(None);
                }
            };
            let Ok((name, description)) = parse(&content) else {
                return Ok(None);
            };
            if revision(&content) != expected {
                return Ok(None);
            }
            Ok(Some(Entry {
                name,
                description,
                source,
                relative_path,
            }))
        }
        /// クライアントごとの所有記録フォルダー（データフォルダー内）。
        fn client_state(&self, client: SkillClient) -> Result<PathBuf> {
            Ok(self
                .db
                .parent()
                .context("Invalid database path")?
                .join(state_name(client)))
        }
        fn cleanup_codex(&self, c: &Connection, state: &Path, client: SkillClient) -> Result<()> {
            let staged = state.join("staged.json");
            reject_links(&staged)?;
            if staged.try_exists()? {
                let record: Staged = serde_json::from_str(&read_text(&staged, CONTENT_LIMIT)?)?;
                self.cleanup_codex_tree(
                    c,
                    &record.path,
                    &record.manifest,
                    "skilvella-staging",
                    client,
                )?;
                fs::remove_file(&staged)?;
            }
            let completed = state.join("last-completed.json");
            reject_links(&completed)?;
            if completed.try_exists()? {
                let record: Pending = serde_json::from_str(&read_text(&completed, CONTENT_LIMIT)?)?;
                self.cleanup_codex_tree(
                    c,
                    &record.stage,
                    &record.next,
                    "skilvella-staging",
                    client,
                )?;
                self.cleanup_codex_tree(
                    c,
                    &record.retired,
                    &record.previous,
                    "skilvella-retired",
                    client,
                )?;
            }
            Ok(())
        }
        fn cleanup_codex_tree(
            &self,
            c: &Connection,
            path: &Path,
            manifest: &Manifest,
            category: &str,
            client: SkillClient,
        ) -> Result<()> {
            ensure!(
                manifest.owner == self.db,
                "Skill entry cleanup belongs to another library"
            );
            if path.as_os_str().is_empty() {
                return Ok(());
            }
            ensure!(
                valid_root(client, &manifest.root),
                "Invalid skill entry cleanup root"
            );
            let expected = manifest
                .root
                .parent()
                .and_then(Path::parent)
                .context("Invalid skill entry cleanup root")?
                .join(category);
            ensure!(
                is_area_entry(&expected, path),
                "Invalid skill entry cleanup path"
            );
            ensure_source_boundaries(c, &manifest.root)?;
            validate_tree(path, path, &manifest.files).with_context(|| format!("呼び出し入口の索引の一時領域 {} を安全に清掃できません。利用者の編集または未完の索引一時ファイルを確認して再試行してください", path.display()))?;
            if path.try_exists()? {
                remove_owned_tree(path, path, &manifest.files)?;
            }
            Ok(())
        }
        fn recover_codex(&self, c: &Connection, state: &Path, client: SkillClient) -> Result<()> {
            let path = state.join("pending.json");
            reject_links(&path)?;
            if !path.try_exists()? {
                return Ok(());
            }
            let pending: Pending = serde_json::from_str(&read_text(&path, CONTENT_LIMIT)?)?;
            for manifest in [&pending.previous, &pending.next] {
                ensure!(
                    manifest.owner == self.db,
                    "Skill entry recovery belongs to another library"
                );
                ensure!(
                    manifest.root.as_os_str().is_empty() || valid_root(client, &manifest.root),
                    "Invalid skill entry recovery root"
                );
                if !manifest.root.as_os_str().is_empty() {
                    ensure_source_boundaries(c, &manifest.root)?;
                }
            }
            for (entry, root, name) in [
                (&pending.stage, &pending.next.root, "skilvella-staging"),
                (
                    &pending.retired,
                    &pending.previous.root,
                    "skilvella-retired",
                ),
            ] {
                ensure!(
                    entry.as_os_str().is_empty() == root.as_os_str().is_empty(),
                    "Invalid skill entry recovery path"
                );
                if !entry.as_os_str().is_empty() {
                    let parent = root
                        .parent()
                        .and_then(Path::parent)
                        .context("Invalid skill entry root")?
                        .join(name);
                    ensure!(
                        is_area_entry(&parent, entry),
                        "Invalid skill entry recovery path"
                    );
                    reject_links(entry)?;
                }
            }
            if !pending.next.root.as_os_str().is_empty()
                && pending.next.root.try_exists()?
                && !pending.stage.try_exists()?
            {
                validate_tree(&pending.next.root, &pending.next.root, &pending.next.files)?;
                super::migration::rename_no_replace(&pending.next.root, &pending.stage)?;
            }
            if !pending.retired.as_os_str().is_empty() && pending.retired.try_exists()? {
                validate_tree(&pending.retired, &pending.retired, &pending.previous.files)?;
                reject_links(&pending.previous.root)?;
                super::migration::rename_no_replace(&pending.retired, &pending.previous.root)?;
            }
            save_manifest(&state.join("manifest.json"), &pending.previous)?;
            finish_pending(state)?;
            Ok(())
        }

        /// Claude Code向けに、生成物の各スキルフォルダーへのリンクを `~/.claude/skills` に揃える。
        ///
        /// - Claude Codeは `~/.claude/skills/<name>/SKILL.md` の1階層しか探索しないため、
        ///   生成物は探索外の所有フォルダーへ一括公開し、探索先にはリンクだけを置く。
        /// - Windowsはジャンクション（管理者権限・開発者モード不要）、macOS・Linuxは
        ///   シンボリックリンクを使う。リンクの削除は中身に触れない。
        /// - 作成したリンクは `links.json` に記録し、記録と一致するリンクだけを外す。
        ///   利用者のフォルダー・別の場所へのリンクは上書きせず、エラーとして表示する。
        /// - 旧版で手動移行した、Codex入口（`~/.agents/skills/skilvella/<name>`）への
        ///   同名リンクはSkilvellaの生成物しか指さないため、引き継いで張り替える。
        fn sync_client_links(&self, c: &Connection, client: SkillClient) -> Result<()> {
            if client != SkillClient::Claude {
                return Ok(());
            }
            let state = self.client_state(client)?;
            let record_path = state.join("links.json");
            reject_links(&record_path)?;
            let record: LinkRecord = if record_path.try_exists()? {
                serde_json::from_str(&read_text(&record_path, CONTENT_LIMIT)?)?
            } else {
                LinkRecord {
                    owner: self.db.clone(),
                    ..Default::default()
                }
            };
            ensure!(
                record.owner == self.db,
                "Claude Code link record belongs to another library"
            );
            // 公開済みの生成物（復旧後の状態を含む）から、張るべきリンク名を決める。
            let manifest_path = state.join("manifest.json");
            reject_links(&manifest_path)?;
            let mut store = PathBuf::new();
            let mut desired = BTreeSet::new();
            if manifest_path.try_exists()? {
                let manifest: Manifest =
                    serde_json::from_str(&read_text(&manifest_path, CONTENT_LIMIT)?)?;
                ensure!(
                    manifest.owner == self.db,
                    "Skill entry export belongs to another library"
                );
                if !manifest.root.as_os_str().is_empty() {
                    ensure!(
                        valid_root(client, &manifest.root),
                        "Invalid skill entry ownership manifest root"
                    );
                    desired = manifest
                        .files
                        .keys()
                        .filter_map(|path| path.split_once('/').map(|(name, _)| name))
                        .filter(|name| valid_link_name(name))
                        .map(str::to_owned)
                        .collect();
                    store = manifest.root;
                }
            }
            if desired.is_empty() && record.names.is_empty() {
                // 未使用のクライアントでは利用者のフォルダーに触れない。
                return Ok(());
            }
            let dir = if store.as_os_str().is_empty() {
                None
            } else {
                link_dir(&store, client)
            };
            let mut kept = BTreeSet::new();
            let mut failures = Vec::new();
            // 1) 記録済みのリンクのうち、不要になったものと置き場が変わったものを外す。
            let record_valid = record.dir.is_absolute()
                && record.dir.ends_with(".claude/skills")
                && valid_root(client, &record.target);
            for name in record.names.iter().filter(|name| valid_link_name(name)) {
                if !record_valid {
                    break;
                }
                let path = record.dir.join(name);
                let still_wanted = record.target == store
                    && dir.as_deref() == Some(record.dir.as_path())
                    && desired.contains(name);
                match link_target(&path) {
                    Ok(Some(actual)) if same_path(&actual, &record.target.join(name)) => {
                        if still_wanted {
                            kept.insert(name.clone());
                        } else if let Err(error) = remove_link(&path) {
                            // 外せなかったリンクは次回も再試行できるよう記録に残す。
                            kept.insert(name.clone());
                            failures.push(format!("{}: {error:#}", path.display()));
                        }
                    }
                    // 利用者が置き換えた・消したものは触らず、所有記録から外す。
                    Ok(_) => {}
                    Err(error) => {
                        kept.insert(name.clone());
                        failures.push(format!("{}: {error:#}", path.display()));
                    }
                }
            }
            let relocating = record.target != store || dir.as_deref() != Some(record.dir.as_path());
            if relocating && !kept.is_empty() {
                // 旧置き場のリンクを外せなかった場合は、旧記録を保って次回に再試行する。
                // 1つの記録に新旧2つの置き場を混在させない。
                save_text(
                    &record_path,
                    &serde_json::to_string(&LinkRecord {
                        names: kept,
                        ..record
                    })?,
                )?;
                bail!(
                    "Claude Codeの入口リンクを更新できません: {}",
                    failures.join("; ")
                );
            }
            // 2) 必要なリンクを作る。既存の同名項目は所有を確認できたものだけ扱う。
            let mut conflicts = Vec::new();
            if let Some(dir) = &dir {
                reject_links(dir)?;
                fs::create_dir_all(dir)?;
                let legacy = target(c, SkillClient::Codex)?;
                let pending: Vec<String> = desired.difference(&kept).cloned().collect();
                for name in &pending {
                    let path = dir.join(name);
                    let expected = store.join(name);
                    let result = (|| -> Result<bool> {
                        match link_target(&path)? {
                            None if !super::migration::exists(&path)? => {
                                create_link(&expected, &path)?;
                                Ok(true)
                            }
                            Some(actual) if same_path(&actual, &expected) => Ok(true),
                            Some(actual)
                                if legacy
                                    .as_ref()
                                    .is_some_and(|codex| same_path(&actual, &codex.join(name))) =>
                            {
                                remove_link(&path)?;
                                create_link(&expected, &path)?;
                                Ok(true)
                            }
                            _ => Ok(false),
                        }
                    })();
                    match result {
                        Ok(true) => {
                            kept.insert(name.clone());
                        }
                        Ok(false) => conflicts.push(name.clone()),
                        Err(error) => failures.push(format!("{}: {error:#}", path.display())),
                    }
                }
            }
            // 3) 衝突・失敗の有無にかかわらず、作成済みのリンクを所有記録へ残す。
            let next = LinkRecord {
                owner: self.db.clone(),
                dir: dir.clone().unwrap_or_default(),
                target: store,
                names: kept,
            };
            if next.names.is_empty() {
                if record_path.try_exists()? {
                    fs::remove_file(&record_path)?;
                }
            } else {
                fs::create_dir_all(&state)?;
                save_text(&record_path, &serde_json::to_string(&next)?)?;
            }
            if !conflicts.is_empty() {
                bail!(
                    "Claude Codeの探索先 {} に同名のスキルがあるため、入口のリンクを作成しませんでした（上書きしません）: {}",
                    dir.as_deref().unwrap_or(Path::new("")).display(),
                    conflicts.join(", ")
                );
            }
            ensure!(
                failures.is_empty(),
                "Claude Codeの入口リンクを更新できません: {}",
                failures.join("; ")
            );
            Ok(())
        }
    }

    /// Claude Code向けリンクの所有記録。`target` は作成時の生成物フォルダー。
    #[derive(Default, Serialize, Deserialize)]
    struct LinkRecord {
        owner: PathBuf,
        dir: PathBuf,
        target: PathBuf,
        names: BTreeSet<String>,
    }

    /// Codexの `agents/openai.yaml` の代わりに、Claude Code・Grok共通の
    /// `disable-model-invocation` で自動選択を止める（手動の `/name` 呼び出しは残る）。
    fn frontmatter_policy(client: SkillClient, text: String, automatic: bool) -> String {
        if client == SkillClient::Codex || automatic {
            return text;
        }
        // 生成文は必ず `---\n...\n---\n` で始まるため、閉じ区切りの直前へ1行加える。
        match text[3..].find("\n---\n") {
            Some(index) => {
                let at = 3 + index + 1;
                format!(
                    "{}disable-model-invocation: true\n{}",
                    &text[..at],
                    &text[at..]
                )
            }
            None => text,
        }
    }

    /// リンク名として安全な1階層の名前だけを受け入れる（記録の改ざん対策）。
    fn valid_link_name(name: &str) -> bool {
        !name.is_empty()
            && name != "."
            && name != ".."
            && !name.ends_with(['.', ' '])
            && !name.contains(['/', '\\', ':', '\0'])
    }

    /// 生成物の一時領域（`skilvella-staging` / `skilvella-retired`）の項目かを判定する純粋関数。
    ///
    /// `area` は「生成先rootの祖父フォルダー/カテゴリ名」。直下にありUUID名であることだけを見る。
    /// 祖父フォルダーが取れない場合のエラー文面・空パスの扱い・リンク検査は呼び出し元ごとに
    /// 異なるため、ここには含めず各呼び出し元に残す（所有記録の検査を弱めないため）。
    fn is_area_entry(area: &Path, path: &Path) -> bool {
        path.parent() == Some(area)
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| uuid::Uuid::parse_str(name).is_ok())
    }

    /// リンク（Windowsのジャンクションを含む）なら絶対パスのリンク先を返す。
    /// 実フォルダー・ファイル・不在は `None`。
    fn link_target(path: &Path) -> Result<Option<PathBuf>> {
        let meta = match fs::symlink_metadata(path) {
            Ok(meta) => meta,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        // Rust標準はジャンクションも名前代理のリパースポイントとしてリンク扱いにする。
        if !meta.file_type().is_symlink() {
            return Ok(None);
        }
        let target = fs::read_link(path)?;
        Ok(Some(if target.is_absolute() {
            target
        } else {
            path.parent().unwrap_or(Path::new("")).join(target)
        }))
    }

    /// `\\?\`・`\??\` の有無と区切り文字、Windows・macOSの大文字小文字差を無視して比べる。
    fn same_path(left: &Path, right: &Path) -> bool {
        fn plain(path: &Path) -> String {
            let text = path.to_string_lossy();
            let text = if cfg!(windows) {
                text.replace('/', "\\")
            } else {
                text.into_owned()
            };
            let text = text
                .strip_prefix(r"\\?\UNC\")
                .map(|rest| format!(r"\\{rest}"))
                .or_else(|| text.strip_prefix(r"\\?\").map(str::to_owned))
                .or_else(|| text.strip_prefix(r"\??\").map(str::to_owned))
                .unwrap_or(text.clone());
            text.trim_end_matches(['/', '\\']).to_owned()
        }
        let (left, right) = (plain(left), plain(right));
        if cfg!(any(windows, target_os = "macos")) {
            left.eq_ignore_ascii_case(&right)
        } else {
            left == right
        }
    }

    /// フォルダーへのリンクを作る。既存項目があれば失敗し、上書きしない。
    pub fn create_link(target: &Path, link: &Path) -> Result<()> {
        ensure!(target.is_absolute(), "Link target must be absolute");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, link)
                .with_context(|| format!("Cannot create link {}", link.display()))?;
        }
        #[cfg(windows)]
        {
            create_junction(target, link)
                .with_context(|| format!("Cannot create junction {}", link.display()))?;
        }
        Ok(())
    }

    /// リンク自体だけを外す。リンク先の生成物には触れない。
    fn remove_link(path: &Path) -> Result<()> {
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            // ジャンクション・ディレクトリリンクはremove_dir、ファイルリンクはremove_fileで外す。
            const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
            if fs::symlink_metadata(path)?.file_attributes() & FILE_ATTRIBUTE_DIRECTORY != 0 {
                fs::remove_dir(path)?;
            } else {
                fs::remove_file(path)?;
            }
        }
        #[cfg(not(windows))]
        fs::remove_file(path)?;
        Ok(())
    }

    /// 空フォルダーを作り、マウントポイントのリパースデータを設定してジャンクションにする。
    /// シンボリックリンクと違い、管理者権限・開発者モードを必要としない。
    #[cfg(windows)]
    fn create_junction(target: &Path, link: &Path) -> Result<()> {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::{
            Foundation::{CloseHandle, GENERIC_WRITE, INVALID_HANDLE_VALUE},
            Storage::FileSystem::{
                CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
                OPEN_EXISTING,
            },
            System::IO::DeviceIoControl,
        };
        const FSCTL_SET_REPARSE_POINT: u32 = 0x0009_00A4;
        const IO_REPARSE_TAG_MOUNT_POINT: u32 = 0xA000_0003;
        let text = target.to_string_lossy();
        // 置換名はNT形式（\??\C:\...）、表示名は通常のWin32形式にする。
        let (substitute, print) = if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
            (format!(r"\??\UNC\{rest}"), format!(r"\\{rest}"))
        } else if let Some(rest) = text.strip_prefix(r"\\?\") {
            (format!(r"\??\{rest}"), rest.to_owned())
        } else {
            (format!(r"\??\{text}"), text.clone().into_owned())
        };
        let substitute: Vec<u16> = substitute.encode_utf16().collect();
        let print: Vec<u16> = print.encode_utf16().collect();
        let path_bytes = (substitute.len() + 1 + print.len() + 1) * 2;
        // MountPointReparseBuffer: 4つのu16（名前の位置と長さ）＋名前本体。
        let data_length = u16::try_from(8 + path_bytes).context("Junction target is too long")?;
        let mut buffer = Vec::with_capacity(8 + usize::from(data_length));
        buffer.extend_from_slice(&IO_REPARSE_TAG_MOUNT_POINT.to_le_bytes());
        buffer.extend_from_slice(&data_length.to_le_bytes());
        buffer.extend_from_slice(&0u16.to_le_bytes());
        buffer.extend_from_slice(&0u16.to_le_bytes());
        buffer.extend_from_slice(&((substitute.len() * 2) as u16).to_le_bytes());
        buffer.extend_from_slice(&(((substitute.len() + 1) * 2) as u16).to_le_bytes());
        buffer.extend_from_slice(&((print.len() * 2) as u16).to_le_bytes());
        for unit in substitute
            .iter()
            .chain([&0])
            .chain(print.iter())
            .chain([&0])
        {
            buffer.extend_from_slice(&unit.to_le_bytes());
        }
        fs::create_dir(link)?;
        let wide: Vec<u16> = link.as_os_str().encode_wide().chain(Some(0)).collect();
        let result = (|| -> Result<()> {
            // SAFETY: 終端付きのパスと、長さを明示したバッファだけを渡す。
            let handle = unsafe {
                CreateFileW(
                    wide.as_ptr(),
                    GENERIC_WRITE,
                    0,
                    std::ptr::null(),
                    OPEN_EXISTING,
                    FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                    std::ptr::null_mut(),
                )
            };
            ensure!(
                handle != INVALID_HANDLE_VALUE,
                "{}",
                std::io::Error::last_os_error()
            );
            let mut returned = 0u32;
            let ok = unsafe {
                DeviceIoControl(
                    handle,
                    FSCTL_SET_REPARSE_POINT,
                    buffer.as_ptr().cast(),
                    buffer.len() as u32,
                    std::ptr::null_mut(),
                    0,
                    &mut returned,
                    std::ptr::null_mut(),
                )
            };
            let error = std::io::Error::last_os_error();
            unsafe { CloseHandle(handle) };
            ensure!(ok != 0, "{error}");
            Ok(())
        })();
        if result.is_err() {
            // 途中で失敗した空フォルダーを残さない。
            let _ = fs::remove_dir(link);
        }
        result
    }

    /// リンク置き場など、生成物以外に書き込む場所が管理フォルダーと重ならないことを確かめる。
    fn ensure_outside_sources(c: &Connection, path: &Path) -> Result<()> {
        let path = super::migration::normalize_path(path)?;
        let mut sources = c.prepare("SELECT path FROM sources WHERE active=1")?;
        for source in sources.query_map([], |r| r.get::<_, String>(0))? {
            let source = PathBuf::from(source?);
            ensure!(source.is_absolute(), "Invalid managed source path");
            ensure!(
                !super::migration::overlaps(&source, &path),
                "Client skill links overlap a managed source: {}",
                source.display()
            );
        }
        Ok(())
    }
    fn save_text(path: &Path, text: &str) -> Result<()> {
        reject_links(path)?;
        if path.try_exists()? {
            atomic_write(path, text, None)
        } else {
            write_new(path, text)
        }
    }
    fn ensure_source_boundaries(c: &Connection, root: &Path) -> Result<()> {
        let agents = root
            .parent()
            .and_then(Path::parent)
            .context("Invalid skill entry root")?;
        let targets = [
            root.to_owned(),
            agents.join("skilvella-staging"),
            agents.join("skilvella-retired"),
        ];
        let mut sources = c.prepare("SELECT path FROM sources WHERE active=1")?;
        for source in sources.query_map([], |r| r.get::<_, String>(0))? {
            // 登録時に正規化済みのパスを使い、無関係な切断volumeを再解決しない。
            let source = PathBuf::from(source?);
            ensure!(source.is_absolute(), "Invalid managed source path");
            for target in &targets {
                let target = super::migration::normalize_path(target)?;
                ensure!(
                    !super::migration::overlaps(&source, &target),
                    "Skill entry export overlaps a managed source: {}",
                    source.display()
                );
            }
        }
        Ok(())
    }
    fn outside(root: &Path, category: &str) -> Result<PathBuf> {
        Ok(root
            .parent()
            .and_then(Path::parent)
            .context("Invalid skill entry root")?
            .join(category)
            .join(uuid::Uuid::new_v4().to_string()))
    }
    fn save_manifest(path: &Path, manifest: &Manifest) -> Result<()> {
        // 直列化の失敗をリンク検査より先に返す従来の順序を保つため、先に文字列化する。
        save_text(path, &serde_json::to_string(manifest)?)
    }
    fn finish_pending(state: &Path) -> Result<()> {
        let path = state.join("pending.json");
        let text = read_text(&path, CONTENT_LIMIT)?;
        let destination = state.join("last-completed.json");
        reject_links(&destination)?;
        if destination.try_exists()? {
            atomic_write(&destination, &text, None)?;
        } else {
            publish_new(&destination, &text)?;
        }
        fs::remove_file(path)?;
        Ok(())
    }
    fn publish_new(path: &Path, text: &str) -> Result<()> {
        // 未完了の書込みがある場合は増殖させず停止する。
        let temporary = path.with_extension("next");
        write_new(&temporary, text)?;
        super::migration::rename_no_replace(&temporary, path)
    }
    fn remove_owned_tree(root: &Path, path: &Path, files: &BTreeMap<String, String>) -> Result<()> {
        reject_links(path)?;
        for entry in fs::read_dir(path)? {
            let path = entry?.path();
            reject_links(&path)?;
            let relative = path
                .strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/");
            if path.is_dir() {
                ensure!(
                    files.keys().any(|p| p.starts_with(&format!("{relative}/"))),
                    "Unknown directory in retired skill entry index"
                );
                remove_owned_tree(root, &path, files)?;
            } else {
                let hash = files
                    .get(&relative)
                    .context("Unknown file in retired skill entry index")?;
                ensure!(
                    revision(&read_text(&path, CONTENT_LIMIT)?) == *hash,
                    "Retired skill entry index was edited by the user"
                );
                fs::remove_file(&path)?;
            }
        }
        fs::remove_dir(path)?;
        Ok(())
    }
    fn complete(manifest: &Manifest) -> Result<bool> {
        for relative in manifest.files.keys() {
            if !manifest.root.join(relative).try_exists()? {
                return Ok(false);
            }
        }
        Ok(true)
    }
    fn validate_tree(root: &Path, path: &Path, files: &BTreeMap<String, String>) -> Result<()> {
        reject_links(path)?;
        if !path.try_exists()? {
            return Ok(());
        }
        ensure!(path.is_dir(), "Skill entry export directory was replaced");
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            reject_links(&path)?;
            let relative = path
                .strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/");
            if entry.file_type()?.is_dir() {
                ensure!(
                    files.keys().any(|p| p.starts_with(&format!("{relative}/"))),
                    "Unknown directory in skill entry export: {relative}"
                );
                validate_tree(root, &path, files)?;
            } else {
                ensure!(
                    entry.file_type()?.is_file(),
                    "Special file in skill entry export"
                );
                let expected = files.get(&relative).with_context(|| {
                    format!("Unknown file in skill entry export: {}", path.display())
                })?;
                ensure!(
                    revision(&read_text(&path, CONTENT_LIMIT)?) == *expected,
                    "Skill entry export was edited by the user: {relative}"
                );
            }
        }
        Ok(())
    }
    fn write_new(path: &Path, text: &str) -> Result<()> {
        reject_links(path)?;
        let message = || {
            format!(
                "呼び出し入口の索引ファイル {} を作成できません。未完の索引一時ファイルを確認して再試行してください",
                path.display()
            )
        };
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .with_context(message)?;
        file.write_all(text.as_bytes()).with_context(message)?;
        file.sync_all().with_context(message)?;
        Ok(())
    }
}
/// フォルダーへのリンク（Windowsはジャンクション）を作る。テストで旧版の手動リンクを再現するために公開する。
#[doc(hidden)]
pub use codex_index::create_link as create_directory_link;
mod migration {
    use super::{Library, put, rebuild_fts, refresh_valid, reject_links, setting_json};
    use crate::model::ScanReport;
    use anyhow::{Context, Result, bail, ensure};
    use rusqlite::{Connection, params};
    use serde::{Deserialize, Serialize};
    use std::{
        fs,
        path::{Component, Path, PathBuf},
    };

    const PENDING: &str = "pending_migration";
    const NATIVE_ROOTS: &str = "native_skill_roots";

    #[derive(Serialize, Deserialize)]
    struct PendingMigration {
        alias: String,
        from: PathBuf,
        to: PathBuf,
        identity: DirectoryIdentity,
    }

    #[derive(Serialize, Deserialize, PartialEq, Eq)]
    struct DirectoryIdentity {
        volume: u64,
        file: u64,
    }

    impl Library {
        pub(super) fn record_native_roots(
            &self,
            home: &Path,
            codex_home: Option<&Path>,
        ) -> Result<()> {
            let c = self.connection()?;
            // 探索先は今回のhome/CODEX_HOMEだけを保持し、過去のvolumeを予約し続けない。
            let mut roots = Vec::new();
            let mut known = vec![home.join(".agents/skills"), home.join(".codex/skills")];
            if let Some(codex) = codex_home {
                known.push(codex.join("skills"));
            }
            for root in known {
                ensure!(root.is_absolute(), "Native skill root must be absolute");
                // 一時的なvolume不在でも既知の探索パスを保持し、管理画面を開く。
                let root = normalize_path(&root).unwrap_or(root);
                if !roots.contains(&root) {
                    roots.push(root);
                }
            }
            put(&c, NATIVE_ROOTS, &serde_json::to_string(&roots)?)?;
            Ok(())
        }

        pub(super) fn is_native_root(c: &Connection, path: &Path) -> Result<bool> {
            let path = normalize_path(path)?;
            let components: Vec<_> = path
                .components()
                .filter_map(|part| match part {
                    Component::Normal(value) => value.to_str(),
                    _ => None,
                })
                .collect();
            let standard = components.windows(2).any(|parts| {
                (path_component_eq(parts[0], ".agents") || path_component_eq(parts[0], ".codex"))
                    && path_component_eq(parts[1], "skills")
            });
            if standard {
                return Ok(true);
            }
            for root in native_roots(c)? {
                if path_is_within(&path, &normalize_path(&root)?) {
                    return Ok(true);
                }
            }
            Ok(false)
        }

        pub(super) fn ensure_no_pending_migration(&self) -> Result<()> {
            ensure!(
                read_pending(&self.connection()?)?.is_none(),
                "未完了のフォルダー移行があります。アプリを再起動して復旧してください"
            );
            Ok(())
        }

        fn check_migration_bounds(
            &self,
            c: &Connection,
            alias: &str,
            from: &Path,
            to: &Path,
        ) -> Result<()> {
            let data = self.db.parent().context("Invalid database location")?;
            ensure!(
                !overlaps(to, data),
                "移動先とアプリのデータフォルダーが重なっています"
            );
            ensure!(!overlaps(to, from), "移動元と移動先が重なっています");
            ensure!(
                !Self::is_native_root(c, to)?,
                "エージェントが自動探索するフォルダー内へは移行できません"
            );
            let mut statement = c.prepare("SELECT path FROM sources WHERE alias<>?1")?;
            for row in statement.query_map([alias], |r| r.get::<_, String>(0))? {
                ensure!(
                    !overlaps(to, &normalize_path(Path::new(&row?))?),
                    "移動先と別の登録フォルダーが重なっています"
                );
            }
            Ok(())
        }

        fn finish_migration(&self, pending: &PendingMigration) -> Result<ScanReport> {
            verify_identity(&pending.to, &pending.identity)?;
            let mut c = self.connection()?;
            let tx = c.transaction()?;
            ensure!(
                tx.execute(
                    "UPDATE sources SET path=?2 WHERE alias=?1 AND active=1 AND path=?3",
                    params![
                        pending.alias,
                        pending
                            .to
                            .to_str()
                            .context("移動先はUTF-8である必要があります")?,
                        pending
                            .from
                            .to_str()
                            .context("移動元はUTF-8である必要があります")?
                    ]
                )? == 1,
                "移行元の登録状態が変更されています"
            );
            let mut settings = Self::directory_on(&tx)?;
            if settings.source_alias == pending.alias {
                settings.mode = "custom".into();
                settings.path = pending.to.to_string_lossy().into_owned();
                settings.custom_path = settings.path.clone();
                Self::save_directory(&tx, &settings)?;
            }
            let report = self.scan_on(&tx)?;
            let ids: Vec<String> = {
                let mut statement = tx.prepare(
                    "SELECT id FROM skills WHERE source=?1 AND missing=0 AND error IS NULL",
                )?;
                statement
                    .query_map([&pending.alias], |r| r.get(0))?
                    .collect::<rusqlite::Result<_>>()?
            };
            for id in ids {
                refresh_valid(&tx, &id, &self.locate(&tx, &id)?.1)?;
                tx.execute("UPDATE skills SET published=1 WHERE id=?1", [&id])?;
            }
            rebuild_fts(&tx)?;
            verify_identity(&pending.to, &pending.identity)?;
            clear_pending(&tx)?;
            tx.commit()?;
            Ok(report)
        }

        pub(super) fn recover_pending_migration(&self) -> Result<()> {
            let c = self.connection()?;
            let Some(pending) = read_pending(&c)? else {
                return Ok(());
            };
            reject_links(&pending.from)?;
            reject_links(&pending.to)?;
            ensure!(
                pending.from.is_absolute() && pending.to.is_absolute(),
                "移行復旧情報のパスが不正です"
            );
            self.check_migration_bounds(&c, &pending.alias, &pending.from, &pending.to)?;
            match (exists(&pending.from)?, exists(&pending.to)?) {
                (true, false) => {
                    verify_identity(&pending.from, &pending.identity)?;
                    ensure!(pending.from.is_dir(), "移行元がフォルダーではありません");
                    sync_rename_parents(&pending.from, &pending.to)?;
                    clear_pending(&c)?;
                }
                (false, true) => {
                    verify_identity(&pending.to, &pending.identity)?;
                    ensure!(pending.to.is_dir(), "移動先がフォルダーではありません");
                    check_tree(&pending.to)?;
                    sync_rename_parents(&pending.from, &pending.to)?;
                    self.finish_migration(&pending).with_context(|| {
                        format!(
                            "フォルダー移行を復旧できません。移行元: {} / 移動先: {}",
                            pending.from.display(),
                            pending.to.display()
                        )
                    })?;
                }
                _ => bail!(
                    "移行元と移動先の状態を自動判定できません。両方存在するか両方不在です。移行元: {} / 移動先: {}",
                    pending.from.display(),
                    pending.to.display()
                ),
            }
            Ok(())
        }
    }

    fn native_roots(c: &Connection) -> Result<Vec<PathBuf>> {
        // 未記録なら空、JSON不正ならErr（従来どおり）。
        Ok(setting_json(c, NATIVE_ROOTS)?.unwrap_or_default())
    }

    fn path_component_eq(left: &str, right: &str) -> bool {
        left.eq_ignore_ascii_case(right)
    }

    pub(super) fn overlaps(left: &Path, right: &Path) -> bool {
        path_is_within(left, right) || path_is_within(right, left)
    }

    pub(super) fn path_is_within(path: &Path, root: &Path) -> bool {
        let mut path = path.components();
        root.components().all(|part| {
            path.next().is_some_and(|candidate| {
                if cfg!(any(windows, target_os = "macos")) {
                    candidate
                        .as_os_str()
                        .to_string_lossy()
                        .eq_ignore_ascii_case(&part.as_os_str().to_string_lossy())
                } else {
                    candidate == part
                }
            })
        })
    }

    pub(super) fn normalize_path(path: &Path) -> Result<PathBuf> {
        ensure!(path.is_absolute(), "フォルダーパスには絶対パスが必要です");
        let mut ancestor = path;
        let mut suffix = Vec::new();
        loop {
            match ancestor.canonicalize() {
                Ok(mut canonical) => {
                    for part in suffix.into_iter().rev() {
                        canonical.push(part);
                    }
                    return Ok(canonical);
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    suffix.push(
                        ancestor
                            .file_name()
                            .context("フォルダーの親を解決できません")?
                            .to_os_string(),
                    );
                    ancestor = ancestor
                        .parent()
                        .context("フォルダーの親を解決できません")?;
                }
                Err(error) => return Err(error).context("フォルダーの正規パスを解決できません"),
            }
        }
    }

    fn sync_directory(path: &Path) -> Result<()> {
        #[cfg(unix)]
        fs::File::open(path)?.sync_all()?;
        #[cfg(not(unix))]
        let _ = path;
        Ok(())
    }

    fn sync_rename_parents(from: &Path, to: &Path) -> Result<()> {
        let old_parent = from.parent().context("移動元の親が必要です")?;
        let new_parent = to.parent().context("移動先の親が必要です")?;
        sync_directory(old_parent)?;
        if old_parent != new_parent {
            sync_directory(new_parent)?;
        }
        Ok(())
    }

    fn check_tree(root: &Path) -> Result<()> {
        reject_links(root)?;
        for entry in fs::read_dir(root)? {
            let path = entry?.path();
            reject_links(&path)?;
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.is_dir()
                && path
                    .file_name()
                    .is_some_and(|name| path_component_eq(&name.to_string_lossy(), "skills"))
                && root.file_name().is_some_and(|name| {
                    path_component_eq(&name.to_string_lossy(), ".agents")
                        || path_component_eq(&name.to_string_lossy(), ".codex")
                })
            {
                bail!(
                    "移行元の内部にエージェントが自動探索するスキルフォルダーがあるため移行できません"
                );
            }
            if path
                .file_name()
                .is_some_and(|name| path_component_eq(&name.to_string_lossy(), ".git"))
            {
                ensure!(
                    metadata.is_dir(),
                    "git worktreeなど.git参照ファイルのあるフォルダーは移行できません"
                );
                check_git_directory(&path)?;
            }
            ensure!(
                metadata.is_dir() || metadata.is_file(),
                "特殊ファイルのあるフォルダーは移行できません"
            );
            if metadata.is_dir() {
                check_tree(&path)?;
            }
        }
        Ok(())
    }

    fn check_git_directory(git: &Path) -> Result<()> {
        // 外部のオブジェクトやworktreeへの参照はルート移動だけでは保全できない。
        for pointer in [
            "objects/info/alternates",
            "worktrees",
            "commondir",
            "config.worktree",
        ] {
            ensure!(
                !exists(&git.join(pointer))?,
                "Gitの外部共有・worktree参照があるフォルダーは移行できません"
            );
        }
        let config = git.join("config");
        reject_links(&config)?;
        if exists(&config)? {
            let text = super::read_text(&config, super::FILE_LIMIT)?;
            ensure!(
                !text.lines().any(|line| {
                    line.trim_start()
                        .split(['=', ' ', '\t'])
                        .next()
                        .is_some_and(|key| key.eq_ignore_ascii_case("worktree"))
                }),
                "Gitのworktree設定があるフォルダーは移行できません"
            );
        }
        Ok(())
    }

    /// リンクを辿らずに項目の有無を返す（壊れたリンクも「存在する」）。不在以外のI/O失敗はErr。
    /// 移行復旧とClaude Code用リンクの作成判定で共用する（`try_exists` はリンクを辿るため別物）。
    pub(super) fn exists(path: &Path) -> Result<bool> {
        match fs::symlink_metadata(path) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    fn directory_identity(path: &Path) -> Result<DirectoryIdentity> {
        reject_links(path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = fs::metadata(path)?;
            ensure!(metadata.is_dir(), "移行対象がフォルダーではありません");
            Ok(DirectoryIdentity {
                volume: metadata.dev(),
                file: metadata.ino(),
            })
        }
        #[cfg(windows)]
        {
            use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
            use windows_sys::Win32::Storage::FileSystem::{
                BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS, GetFileInformationByHandle,
            };
            let directory = fs::OpenOptions::new()
                .read(true)
                .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
                .open(path)?;
            ensure!(
                directory.metadata()?.is_dir(),
                "移行対象がフォルダーではありません"
            );
            let mut information = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
            if unsafe {
                GetFileInformationByHandle(directory.as_raw_handle() as _, information.as_mut_ptr())
            } == 0
            {
                return Err(std::io::Error::last_os_error().into());
            }
            let information = unsafe { information.assume_init() };
            Ok(DirectoryIdentity {
                volume: u64::from(information.dwVolumeSerialNumber),
                file: (u64::from(information.nFileIndexHigh) << 32)
                    | u64::from(information.nFileIndexLow),
            })
        }
    }

    fn verify_identity(path: &Path, expected: &DirectoryIdentity) -> Result<()> {
        ensure!(
            &directory_identity(path)? == expected,
            "移行対象のフォルダーが置き換わっています。自動復旧を停止し、復旧情報を保持しました"
        );
        Ok(())
    }

    fn read_pending(c: &Connection) -> Result<Option<PendingMigration>> {
        setting_json(c, PENDING)
    }

    fn clear_pending(c: &Connection) -> Result<()> {
        c.execute("DELETE FROM settings WHERE key=?1", [PENDING])?;
        Ok(())
    }

    pub(super) fn rename_no_replace(from: &Path, to: &Path) -> Result<()> {
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            let from: Vec<_> = from.as_os_str().encode_wide().chain(Some(0)).collect();
            let to: Vec<_> = to.as_os_str().encode_wide().chain(Some(0)).collect();
            // 既存先の置換と別ボリュームへのコピーを許可せず、保存完了を待つ。
            if unsafe {
                windows_sys::Win32::Storage::FileSystem::MoveFileExW(
                    from.as_ptr(),
                    to.as_ptr(),
                    windows_sys::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH,
                )
            } == 0
            {
                return Err(std::io::Error::last_os_error().into());
            }
        }
        #[cfg(unix)]
        {
            use std::{ffi::CString, os::unix::ffi::OsStrExt};
            let from = CString::new(from.as_os_str().as_bytes())?;
            let to = CString::new(to.as_os_str().as_bytes())?;
            #[cfg(target_os = "linux")]
            let result = unsafe {
                libc::renameat2(
                    libc::AT_FDCWD,
                    from.as_ptr(),
                    libc::AT_FDCWD,
                    to.as_ptr(),
                    libc::RENAME_NOREPLACE,
                )
            };
            #[cfg(target_os = "macos")]
            let result = unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) };
            #[cfg(not(any(target_os = "linux", target_os = "macos")))]
            compile_error!(
                "Atomic migration requires a no-replace rename implementation for this OS"
            );
            if result != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
        }
        Ok(())
    }
}

use anyhow::{Context, Result, bail, ensure};
use rusqlite::{Connection, OptionalExtension, params};
use saphyr::LoadableYamlNode;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

const CONTENT_LIMIT: u64 = 2 * 1024 * 1024;
const FILE_LIMIT: u64 = 1024 * 1024;
const SOURCE_DISPLAY_NAME_PREFIX: &str = "source_display_name:";

struct WriteGuard<'a> {
    _file: fs::File,
    _mutex: MutexGuard<'a, ()>,
}

fn lock_writer_file(path: &Path) -> Result<fs::File> {
    reject_links(path)?;
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path)?;
    ensure!(
        file.metadata()?.is_file(),
        "Writer lock must be a regular file"
    );
    file.lock()
        .context("Cannot acquire library operation lock")?;
    Ok(file)
}

#[derive(Clone)]
pub struct Library {
    db: PathBuf,
    writer: Arc<Mutex<()>>,
}

impl Library {
    pub fn open(data_dir: &Path) -> Result<Self> {
        Self::open_inner(data_dir, None)
    }
    pub fn open_for_user(data_dir: &Path, home: &Path, codex_home: Option<&Path>) -> Result<Self> {
        Self::open_inner(data_dir, Some((home, codex_home)))
    }
    fn open_inner(data_dir: &Path, native_context: Option<(&Path, Option<&Path>)>) -> Result<Self> {
        reject_links(data_dir)?;
        let mut directory = fs::DirBuilder::new();
        directory.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            directory.mode(0o700);
        }
        directory.create(data_dir)?;
        let db = data_dir.canonicalize()?.join("library.sqlite3");
        // 別Library・別プロセスの移行が終わるまで起動時復旧も待機する。
        let _operation = lock_writer_file(&db.with_file_name("library.write.lock"))?;
        reject_links(&db)?;
        // SQLite が初めて開く前から履歴と索引のファイルを非公開で作成する。
        if !db.exists() {
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&db) {
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
        let library = Self {
            db,
            writer: Arc::new(Mutex::new(())),
        };
        library.connection()?.execute_batch("PRAGMA journal_mode=WAL;
            CREATE TABLE IF NOT EXISTS sources(alias TEXT PRIMARY KEY,path TEXT NOT NULL,active INTEGER NOT NULL DEFAULT 1);
            CREATE TABLE IF NOT EXISTS skills(id TEXT PRIMARY KEY,source TEXT NOT NULL,relative_path TEXT NOT NULL,name TEXT NOT NULL DEFAULT '',description TEXT NOT NULL DEFAULT '',content TEXT NOT NULL DEFAULT '',revision TEXT NOT NULL DEFAULT '',tags TEXT NOT NULL DEFAULT '[]',category TEXT NOT NULL DEFAULT '',published INTEGER NOT NULL DEFAULT 0,error TEXT,missing INTEGER NOT NULL DEFAULT 0,updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
            CREATE TABLE IF NOT EXISTS history(id INTEGER PRIMARY KEY,skill_id TEXT NOT NULL,content TEXT NOT NULL,created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,reason TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY,value TEXT NOT NULL);
            CREATE VIRTUAL TABLE IF NOT EXISTS skill_fts USING fts5(id UNINDEXED,name,description,content,tags,category,tokenize='trigram');")?;
        if let Some((home, codex_home)) = native_context {
            library.record_native_roots(home, codex_home)?;
        }
        library.recover_pending_migration()?;
        library.refresh_repository_cache(&library.connection()?)?;
        Ok(library)
    }
    fn write_guard(&self) -> Result<WriteGuard<'_>> {
        let mutex = self
            .writer
            .lock()
            .map_err(|_| anyhow::anyhow!("Writer lock poisoned"))?;
        let file = lock_writer_file(&self.db.with_file_name("library.write.lock"))?;
        Ok(WriteGuard {
            _file: file,
            _mutex: mutex,
        })
    }
    fn connection(&self) -> Result<Connection> {
        reject_links(&self.db)?;
        let c = Connection::open(&self.db)?;
        c.busy_timeout(Duration::from_secs(10))?;
        Ok(c)
    }
    fn directory_on(c: &Connection) -> Result<DirectorySettings> {
        // 未設定なら既定値、JSON不正ならErr（従来どおり）。
        Ok(setting_json(c, "directory")?.unwrap_or_default())
    }
    fn save_directory(c: &Connection, settings: &DirectorySettings) -> Result<()> {
        put(c, "directory", &serde_json::to_string(settings)?)
    }
    fn directory_owns_source(c: &Connection, alias: &str) -> Result<bool> {
        Ok(c.query_row(
            "SELECT value='1' FROM settings WHERE key=?1",
            [format!("directory_owned:{alias}")],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(false))
    }
    fn set_source_ownership(c: &Connection, alias: &str, owned: bool) -> Result<()> {
        put(
            c,
            &format!("directory_owned:{alias}"),
            if owned { "1" } else { "0" },
        )
    }
    fn deactivate_directory_source(c: &Connection, alias: &str) -> Result<()> {
        if Self::directory_owns_source(c, alias)? {
            c.execute("UPDATE sources SET active=0 WHERE alias=?1", [alias])?;
        }
        Ok(())
    }
    pub fn directory_settings(&self) -> Result<DirectorySettings> {
        Self::directory_on(&self.connection()?)
    }
    pub fn initialize_directory(&self, home: &Path, codex_home: Option<&Path>) -> Result<()> {
        let _guard = self.write_guard()?;
        self.ensure_no_pending_migration()?;
        self.record_native_roots(home, codex_home)?;
        let mut candidates = vec![home.join(".agents/skills")];
        if let Some(path) = codex_home {
            candidates.push(path.join("skills"));
        }
        candidates.push(home.join(".codex/skills"));
        let data = self.db.parent().context("Invalid database location")?;
        let fallback = data.with_file_name(format!(
            "{}-skills",
            data.file_name()
                .context("Invalid database location")?
                .to_string_lossy()
        ));
        let (owned_codex, mut inconclusive, _) = self.owned_codex_markers();
        let excluded = owned_codex.iter().map(|p| path_key(p)).collect();
        let mut candidate = None;
        for p in candidates {
            if inconclusive {
                break;
            }
            if !p.is_dir() || reject_links(&p).is_err() || fs::read_dir(&p).is_err() {
                continue;
            }
            let Ok(root) = p.canonicalize() else {
                continue;
            };
            let mut present = false;
            for marker in owned_codex
                .iter()
                .filter(|owned| migration::path_is_within(owned, &root))
            {
                if reject_links(marker).is_err() {
                    inconclusive = true;
                    break;
                }
                match fs::symlink_metadata(marker) {
                    Ok(meta) => present |= meta.is_file(),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(_) => {
                        inconclusive = true;
                        break;
                    }
                }
            }
            if inconclusive {
                break;
            }
            if !present {
                candidate = Some(p);
                break;
            }
            // 生成で作られた探索先は、入口以外に原本がある場合だけ候補にする。
            let mut found = Vec::new();
            let mut blocked = Vec::new();
            let result = discover(
                &root,
                &mut found,
                &mut blocked,
                &excluded,
                &Default::default(),
                true,
            );
            if !found.is_empty() {
                candidate = Some(p);
                break;
            }
            if result.is_err() {
                inconclusive = true;
                break;
            }
        }
        let has_candidate = candidate.is_some();
        let detected = candidate.unwrap_or(fallback);
        let mut c = self.connection()?;
        let tx = c.transaction()?;
        let mut settings = Self::directory_on(&tx)?;
        let persisted_auto = settings.mode == "auto";
        settings.detected_path = if inconclusive {
            String::new()
        } else {
            detected.to_string_lossy().into_owned()
        };
        if settings.mode.is_empty() {
            // 既存登録があるライブラリへ、推測したルートを追加しない。
            if let Some(source) = Self::sources_on(&tx)?.into_iter().next() {
                settings.mode = "custom".into();
                settings.custom_path = source.path.clone();
                settings.path = source.path;
                settings.source_alias = source.alias;
            } else if tx.query_row("SELECT EXISTS(SELECT 1 FROM sources)", [], |r| {
                r.get::<_, bool>(0)
            })? {
                // 旧版で解除済みの登録を、新規ライブラリとして復活させない。
                settings.mode = "disabled".into();
            } else {
                settings.mode = "auto".into();
            }
        }
        if inconclusive && settings.mode == "auto" {
            Self::save_directory(&tx, &settings)?;
            self.scan_on(&tx)?;
            tx.commit()?;
            return Ok(());
        }
        // 保存済みルートが一時的に不在でも、管理画面から設定を変更できるよう起動する。
        // 索引と公開判定には通常の走査失敗・消失処理を使い、履歴は保持する。
        if !settings.source_alias.is_empty()
            && (settings.mode != "auto" || !has_candidate)
            && (reject_links(Path::new(&settings.path)).is_err()
                || fs::read_dir(&settings.path).is_err())
        {
            Self::save_directory(&tx, &settings)?;
            self.scan_on(&tx)?;
            tx.commit()?;
            return Ok(());
        }
        if settings.mode == "auto" {
            reject_links(&detected)?;
            fs::create_dir_all(&detected)?;
        }
        let mode = settings.mode.clone();
        let path = settings.custom_path.clone();
        let previous_settings = settings.clone();
        tx.execute_batch("SAVEPOINT directory_selection")?;
        match self.set_directory_on(&tx, settings, &mode, &path) {
            Ok(_) => tx.execute_batch("RELEASE directory_selection")?,
            Err(error) if persisted_auto => {
                // 起動時の再選択が手動ソースと重なる場合も、既存設定で管理画面を開く。
                tx.execute_batch("ROLLBACK TO directory_selection; RELEASE directory_selection")?;
                Self::save_directory(&tx, &previous_settings)?;
                self.scan_on(&tx)?;
                eprintln!(
                    "Automatic directory selection unavailable; retained previous settings: {error}"
                );
            }
            Err(error) => return Err(error),
        }
        tx.commit()?;
        Ok(())
    }
    pub fn set_directory(&self, mode: &str, path: &str) -> Result<ScanReport> {
        let _guard = self.write_guard()?;
        self.ensure_no_pending_migration()?;
        let mut c = self.connection()?;
        let tx = c.transaction()?;
        let settings = Self::directory_on(&tx)?;
        let report = self.set_directory_on(&tx, settings, mode, path)?;
        tx.commit()?;
        Ok(report)
    }
    fn set_directory_on(
        &self,
        c: &Connection,
        mut settings: DirectorySettings,
        mode: &str,
        path: &str,
    ) -> Result<ScanReport> {
        ensure!(
            matches!(mode, "auto" | "custom" | "disabled"),
            "Invalid directory mode"
        );
        if mode == "disabled" {
            Self::deactivate_directory_source(c, &settings.source_alias)?;
            settings.mode = mode.into();
            settings.path.clear();
            settings.source_alias.clear();
            Self::save_directory(c, &settings)?;
            return self.scan_on(c);
        }
        let selected = if mode == "auto" {
            settings.detected_path.as_str()
        } else {
            path
        };
        let selected = Path::new(selected);
        ensure!(selected.is_absolute(), "Directory must be absolute");
        reject_links(selected)?;
        let data = self.db.parent().context("Invalid database location")?;
        let fallback = data.with_file_name(format!(
            "{}-skills",
            data.file_name()
                .context("Invalid database location")?
                .to_string_lossy()
        ));
        if mode == "auto" && selected == fallback && !selected.exists() {
            fs::create_dir_all(selected)?;
        }
        let root = selected
            .canonicalize()
            .context("Source directory does not exist")?;
        ensure!(root.is_dir(), "Source must be a directory");
        fs::read_dir(&root).context("Cannot read source directory")?;
        let root_text = root.to_str().context("Source path must be valid UTF-8")?;
        let data = self.db.parent().context("Invalid database location")?;
        ensure!(
            !root.starts_with(data) && !data.starts_with(&root),
            "Source and library data directories must not overlap"
        );
        let mut statement = c.prepare("SELECT alias,path,active FROM sources ORDER BY alias")?;
        let rows = statement
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, bool>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let owns_current = Self::directory_owns_source(c, &settings.source_alias)?;
        let mut alias = None;
        let mut matching_active = false;
        for (old_alias, old_path, active) in &rows {
            let old_root = Path::new(old_path)
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from(old_path));
            if old_root == root {
                // 別名で再登録した手動ソースを優先し、同じルートを二重に有効化しない。
                if !matching_active
                    && (*active || alias.is_none() || *old_alias == settings.source_alias)
                {
                    alias = Some(old_alias.clone());
                    matching_active = *active;
                }
            } else if *active && (*old_alias != settings.source_alias || !owns_current) {
                ensure!(
                    !root.starts_with(&old_root) && !old_root.starts_with(&root),
                    "Source directories overlap"
                );
            }
        }
        let new_alias = alias.is_none();
        let alias = alias.unwrap_or_else(|| {
            let mut alias = "default".to_owned();
            let mut n = 2;
            while rows.iter().any(|r| r.0 == alias) {
                alias = format!("default-{n}");
                n += 1;
            }
            alias
        });
        Self::deactivate_directory_source(c, &settings.source_alias)?;
        c.execute("INSERT INTO sources(alias,path,active) VALUES(?1,?2,1) ON CONFLICT(alias) DO UPDATE SET active=1", params![alias,root_text])?;
        if new_alias {
            Self::set_source_ownership(c, &alias, true)?;
        }
        settings.mode = mode.into();
        settings.path = root_text.into();
        settings.source_alias = alias;
        if mode == "custom" {
            settings.custom_path = root_text.into();
        }
        Self::save_directory(c, &settings)?;
        self.scan_on(c)
    }
    pub fn sources(&self) -> Result<Vec<Source>> {
        let c = self.connection()?;
        Self::sources_on(&c)
    }
    fn sources_on(c: &Connection) -> Result<Vec<Source>> {
        // 表示名も同じ一覧クエリで取り、取得元ごとにsettingsを読み直さない。
        let mut statement = c.prepare(
            "SELECT s.alias,s.path,coalesce(k.skill_count,0),coalesce(k.error_count,0),
                    coalesce(d.value,s.alias)
             FROM sources s
             LEFT JOIN (
                 SELECT source,sum(CASE WHEN missing=0 THEN 1 ELSE 0 END) AS skill_count,
                        sum(CASE WHEN error IS NOT NULL OR missing=1 THEN 1 ELSE 0 END) AS error_count
                 FROM skills GROUP BY source
             ) k ON k.source=s.alias
             LEFT JOIN settings d ON d.key=(?1 || s.alias)
             WHERE s.active=1 ORDER BY s.alias",
        )?;
        let mut sources = statement
            .query_map([SOURCE_DISPLAY_NAME_PREFIX], |r| {
                Ok(Source {
                    alias: r.get(0)?,
                    display_name: r.get(4)?,
                    path: r.get(1)?,
                    skill_count: r.get::<_, i64>(2)? as usize,
                    error_count: r.get::<_, i64>(3)? as usize,
                    native_discovery: false,
                    native_discovery_unknown: false,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for source in &mut sources {
            match Self::is_native_root(c, Path::new(&source.path)) {
                Ok(native) => source.native_discovery = native,
                Err(_) => {
                    // オフラインvolumeなどの判定不能を、専用フォルダーの保証にしない。
                    source.native_discovery = true;
                    source.native_discovery_unknown = true;
                }
            }
        }
        Ok(sources)
    }
    fn source_display_name_on(c: &Connection, alias: &str) -> Result<String> {
        Ok(c.query_row(
            "SELECT value FROM settings WHERE key=?1",
            [format!("{SOURCE_DISPLAY_NAME_PREFIX}{alias}")],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or_else(|| alias.to_owned()))
    }

    pub fn source_label(&self, alias: &str) -> Result<String> {
        let c = self.connection()?;
        let display_name = Self::source_display_name_on(&c, alias)?;
        Ok(Source::format_label(alias, &display_name))
    }

    pub fn set_source_display_name(&self, alias: &str, name: &str) -> Result<()> {
        ensure!(
            !name.chars().any(char::is_control) && name.chars().count() <= 100,
            "表示名は制御文字を含まない100文字以内で入力してください。"
        );
        let name = name.trim();
        let _guard = self.write_guard()?;
        self.ensure_no_pending_migration()?;
        let mut c = self.connection()?;
        let tx = c.transaction()?;
        ensure!(
            tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM sources WHERE alias=?1 AND active=1)",
                [alias],
                |r| r.get::<_, bool>(0),
            )?,
            "Source not found"
        );
        let key = format!("{SOURCE_DISPLAY_NAME_PREFIX}{alias}");
        if name.is_empty() || name == alias {
            tx.execute("DELETE FROM settings WHERE key=?1", [key])?;
        } else {
            put(&tx, &key, name)?;
        }
        tx.commit()?;
        Ok(())
    }
    /// 識別子を明示してフォルダーを登録する（旧来のAPI。テスト・互換用）。
    ///
    /// 識別子は既存の行（解除済みを含む）と同じフォルダーを指す場合だけ再利用できる。
    /// 解除済みの行も履歴とスキルIDを保持するため、別フォルダーへ付け替えない。
    pub fn add_source(&self, alias: &str, path: &Path) -> Result<ScanReport> {
        ensure!(valid_source_alias(alias), "Invalid source alias");
        self.register_source(Some(alias), path)
            .map(|(_, report)| report)
    }

    /// 利用者が選んだフォルダーを登録し、割り当てた識別子と走査結果を返す。
    ///
    /// 識別子は利用者に入力させず、フォルダー名から自動で作る。以前に管理から外した
    /// 同じフォルダーがあれば、その識別子を再利用して履歴・設定・スキルIDを引き継ぐ。
    /// 別PCや再インストール後に、解除済みの登録や既定フォルダー（`default`）と
    /// 名前が衝突して登録できない問題を避けるための入口。
    pub fn add_folder(&self, path: &Path) -> Result<(String, ScanReport)> {
        self.register_source(None, path)
    }

    fn register_source(
        &self,
        requested: Option<&str>,
        path: &Path,
    ) -> Result<(String, ScanReport)> {
        let _guard = self.write_guard()?;
        self.ensure_no_pending_migration()?;
        ensure!(path.is_absolute(), "Directory must be absolute");
        reject_links(path)?;
        let root = path
            .canonicalize()
            .context("Source directory does not exist")?;
        ensure!(root.is_dir(), "Source must be a directory");
        let root_text = root
            .to_str()
            .context("Source path must be valid UTF-8")?
            .to_owned();
        let data = self.db.parent().context("Invalid database location")?;
        if root.starts_with(data) || data.starts_with(&root) {
            return Err(FolderError::OverlapsData.into());
        }
        let mut c = self.connection()?;
        let tx = c.transaction()?;
        let rows = {
            let mut stmt = tx.prepare("SELECT alias,path,active FROM sources ORDER BY alias")?;
            stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, bool>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
        };
        let root_key = path_key(&root);
        // 保存済みのパスは表記揺れ（大文字小文字・\\?\ 接頭辞など）を正規化して比べる。
        // 一時的に読めないフォルダーは保存値のまま比べる。
        let canonical = |stored: &str| {
            Path::new(stored)
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from(stored))
        };
        let mut reuse = None;
        for (old_alias, old_path, active) in &rows {
            let old_root = canonical(old_path);
            let same = path_key(&old_root) == root_key;
            let explicit = requested.is_some_and(|alias| alias == old_alias);
            if explicit {
                ensure!(same, "Alias already belongs to another directory");
            }
            if same {
                if *active && !explicit {
                    return Err(FolderError::AlreadyRegistered {
                        name: Self::source_display_name_on(&tx, old_alias)?,
                    }
                    .into());
                }
                // 解除済みの同じフォルダーは、明示指定がなければ識別子を引き継ぐ。
                // 明示指定と同じ登録の再追加は、従来どおり再走査だけを行う。
                if requested.is_none() && reuse.is_none() {
                    reuse = Some(old_alias.clone());
                }
            } else if *active && (root.starts_with(&old_root) || old_root.starts_with(&root)) {
                return Err(FolderError::Overlaps {
                    name: Self::source_display_name_on(&tx, old_alias)?,
                    path: old_root.to_string_lossy().into_owned(),
                }
                .into());
            }
        }
        let created = reuse.is_none() && requested.is_none();
        let alias = match (requested, reuse) {
            (Some(alias), _) => alias.to_owned(),
            (None, Some(alias)) => alias,
            (None, None) => {
                let base = folder_alias_base(&root);
                let taken = |candidate: &str| rows.iter().any(|row| row.0 == candidate);
                let mut alias = base.clone();
                let mut n = 2;
                while taken(&alias) {
                    alias = format!("{base}-{n}");
                    n += 1;
                }
                alias
            }
        };
        tx.execute(
            "INSERT INTO sources(alias,path,active) VALUES(?1,?2,1)
             ON CONFLICT(alias) DO UPDATE SET active=1,path=excluded.path",
            params![alias, root_text],
        )?;
        Self::set_source_ownership(&tx, &alias, false)?;
        if created {
            // 自動採番した識別子の代わりに、フォルダー名をそのまま表示名にする。
            let name = root
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| root_text.clone());
            let name: String = name.chars().filter(|c| !c.is_control()).take(100).collect();
            if !name.is_empty() && name != alias {
                put(&tx, &format!("{SOURCE_DISPLAY_NAME_PREFIX}{alias}"), &name)?;
            }
        }
        let report = self.scan_on(&tx)?;
        tx.commit()?;
        Ok((alias, report))
    }
    pub fn remove_source(&self, alias: &str) -> Result<()> {
        let _guard = self.write_guard()?;
        self.ensure_no_pending_migration()?;
        let mut c = self.connection()?;
        let tx = c.transaction()?;
        tx.execute("UPDATE sources SET active=0 WHERE alias=?1", [alias])?;
        let mut settings = Self::directory_on(&tx)?;
        if settings.source_alias == alias {
            settings.mode = "disabled".into();
            settings.path.clear();
            settings.source_alias.clear();
            Self::save_directory(&tx, &settings)?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn scan(&self) -> Result<ScanReport> {
        let _guard = self.write_guard()?;
        self.ensure_no_pending_migration()?;
        self.scan_locked(&self.connection()?)
    }
    fn scan_locked(&self, c: &Connection) -> Result<ScanReport> {
        let tx = c.unchecked_transaction()?;
        let report = self.scan_on(&tx)?;
        tx.commit()?;
        Ok(report)
    }
    fn scan_on(&self, tx: &Connection) -> Result<ScanReport> {
        self.refresh_repository_cache(tx)?;
        let sources = Self::sources_on(tx)?;
        let (owned_codex, uncertain, regions) = self.owned_codex_markers();
        let excluded = owned_codex.iter().map(|p| path_key(p)).collect();
        let regions = if uncertain {
            regions
        } else {
            Default::default()
        };
        let mut report = ScanReport::default();
        for source in sources {
            let root = PathBuf::from(&source.path);
            let mut found = Vec::new();
            let mut blocked = Vec::new();
            let result = reject_links(&root).and_then(|_| {
                discover(&root, &mut found, &mut blocked, &excluded, &regions, false)
            });
            for region in &regions {
                if migration::overlaps(&root, region) {
                    let path = if migration::path_is_within(region, &root) {
                        region
                    } else {
                        &root
                    };
                    if !blocked.iter().any(|p| path_key(p) == path_key(path)) {
                        blocked.push(path.clone());
                    }
                }
            }
            // 部分読取失敗でも、既知の生成入口の旧索引を原本として残さない。
            // ループ内の固定SQLは prepare_cached で接続ごとに1回だけ準備する（実行順・トランザクションは不変）。
            for marker in &owned_codex {
                if let Ok(relative) = relative_key(&root, marker) {
                    tx.prepare_cached(&format!(
                        "UPDATE skills SET missing=1 WHERE source=?1 AND relative_path=?2 COLLATE {}",
                        path_collation()
                    ))?
                    .execute(params![source.alias, relative])?;
                }
            }
            // 部分的な読取失敗では未走査の行を消失扱いにせず、読めた兄弟を反映する。
            if (result.is_ok() && blocked.is_empty()) || !root.is_dir() {
                tx.execute(
                    "UPDATE skills SET missing=1 WHERE source=?1",
                    [&source.alias],
                )?;
            }
            if result.is_err() || !blocked.is_empty() {
                if blocked.is_empty() {
                    blocked.push(root.clone());
                }
                report.errors += blocked.len();
                for path in blocked {
                    let ownership_blocked = regions
                        .iter()
                        .any(|region| migration::overlaps(&path, region));
                    let key = relative_key(&root, &path).unwrap_or_default();
                    let prefix = format!("{key}/");
                    let message = if ownership_blocked {
                        "Skill entry index ownership is unverified; check index records and rescan"
                    } else {
                        "Scan unavailable; check source permissions and rescan"
                    };
                    let collation = path_collation();
                    tx.prepare_cached(&format!("UPDATE skills SET error=?4 WHERE source=?1 AND (?2='' OR relative_path=?2 COLLATE {collation} OR substr(relative_path,1,length(?3))=?3 COLLATE {collation})"))?
                        .execute(params![source.alias, key, prefix, message])?;
                }
            }
            for path in found {
                let relative = match relative_key(&root, &path) {
                    Ok(relative) => relative,
                    Err(_) => {
                        report.errors += 1;
                        continue;
                    }
                };
                let id = skill_id(&source.alias, &relative);
                let read = read_text(&path, CONTENT_LIMIT);
                let (content, name, description, error) = match read {
                    Ok(content) => match parse(&content) {
                        Ok((name, description)) => (content, name, description, None),
                        Err(e) => (content, String::new(), String::new(), Some(e.to_string())),
                    },
                    Err(e) => (
                        String::new(),
                        String::new(),
                        String::new(),
                        Some(e.to_string()),
                    ),
                };
                if error.is_some() {
                    report.errors += 1;
                }
                report.indexed += 1;
                // 発見したスキルごとに実行されるため、固定SQLは prepare_cached で再利用する。
                tx.prepare_cached("INSERT INTO skills(id,source,relative_path,name,description,content,revision,error,missing) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,0) ON CONFLICT(id) DO UPDATE SET name=excluded.name,description=excluded.description,content=excluded.content,revision=excluded.revision,error=excluded.error,missing=0,updated_at=CASE WHEN skills.revision<>excluded.revision THEN CURRENT_TIMESTAMP ELSE skills.updated_at END")?
                    .execute(params![id, source.alias, relative, name, description, content, revision(&content), error])?;
            }
        }
        report.missing = tx.query_row("SELECT count(*) FROM skills k JOIN sources s ON s.alias=k.source WHERE s.active=1 AND k.missing=1",[],|r|r.get::<_,i64>(0))? as usize;
        rebuild_fts(tx)?;
        tx.execute("INSERT INTO settings(key,value) VALUES('indexed_at',CURRENT_TIMESTAMP) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[])?;
        Ok(report)
    }
    fn repository_cache_key(source: &Source) -> String {
        format!("repositories:{}:{}", source.alias, revision(&source.path))
    }

    // .gitの内容やリンク先は読まず、直下のマーカーをスキル件数と独立に検出する。
    fn detect_repositories(root: &Path) -> Result<Vec<String>> {
        reject_links(root)?;
        let marker = |path: &Path| -> Result<bool> {
            match fs::symlink_metadata(path.join(".git")) {
                Ok(metadata) => {
                    ensure!(
                        !is_link(&metadata) && (metadata.is_dir() || metadata.is_file()),
                        "Cannot determine repository: unsafe .git marker in {}",
                        path.display()
                    );
                    Ok(true)
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
                Err(error) => Err(error)
                    .with_context(|| format!("Cannot inspect repository {}", path.display())),
            }
        };
        let mut repositories = Vec::new();
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            let metadata = fs::symlink_metadata(entry.path())?;
            if !is_link(&metadata) && metadata.is_dir() && marker(&entry.path())? {
                repositories.push(
                    entry
                        .file_name()
                        .into_string()
                        .map_err(|_| anyhow::anyhow!("Repository name is not UTF-8"))?,
                );
            }
        }
        if repositories.is_empty() && marker(root)? {
            repositories.push(".".into());
        }
        Ok(repositories)
    }

    fn repository_roots(c: &Connection, source: &Source) -> Result<Vec<String>> {
        let saved: Option<Vec<String>> = setting_json(c, &Self::repository_cache_key(source))?;
        let detected = Self::detect_repositories(Path::new(&source.path));
        let detected_children = detected
            .as_ref()
            .is_ok_and(|roots| roots.iter().any(|root| root != "."));
        let mut roots = match (detected, saved) {
            (Ok(mut roots), Some(saved)) => {
                for root in saved {
                    let has_skills: bool = c.query_row("SELECT EXISTS(SELECT 1 FROM skills WHERE source=?1 AND (?2='.' OR substr(relative_path,1,length(?3))=?3))",
                            params![source.alias, root, format!("{root}/")], |row| row.get(0))?;
                    if has_skills {
                        roots.push(root);
                    }
                }
                roots
            }
            (Ok(roots), None) => roots,
            (Err(_), Some(saved)) => saved,
            (Err(error), None) => {
                return Err(error).context(
                    "取得元を確認できません。ソースの接続・権限を確認して再スキャンしてください。",
                );
            }
        };
        if detected_children {
            roots.retain(|root| root != ".");
        }
        roots.sort_by(|a, b| (a == ".").cmp(&(b == ".")).then(a.cmp(b)));
        roots.dedup();
        Ok(roots)
    }

    // 起動・走査の既存操作ロック内だけで保存し、GETは書き込まない。
    fn refresh_repository_cache(&self, c: &Connection) -> Result<()> {
        for source in Self::sources_on(c)? {
            if let Ok(roots) = Self::repository_roots(c, &source) {
                put(
                    c,
                    &Self::repository_cache_key(&source),
                    &serde_json::to_string(&roots)?,
                )?;
            }
        }
        Ok(())
    }

    pub fn repository_groups(&self) -> Result<Vec<RepositoryGroup>> {
        self.repository_groups_with_error_count()
            .map(|(groups, _)| groups)
    }

    pub(crate) fn repository_groups_with_error_count(
        &self,
    ) -> Result<(Vec<RepositoryGroup>, usize)> {
        let c = self.connection()?;
        Self::repository_groups_on(&c)
    }

    fn repository_for<'a>(relative: &str, repositories: impl Iterator<Item = &'a str>) -> &'a str {
        repositories
            .filter(|root| {
                !root.is_empty()
                    && (*root == "."
                        || relative
                            .strip_prefix(*root)
                            .is_some_and(|rest| rest.starts_with('/')))
            })
            .max_by_key(|root| *root != ".")
            .unwrap_or_default()
    }

    fn repository_groups_on(c: &Connection) -> Result<(Vec<RepositoryGroup>, usize)> {
        let mut groups: std::collections::BTreeMap<(String, String), RepositoryGroup> =
            Default::default();
        let mut roots = std::collections::BTreeMap::new();
        for source in Self::sources_on(c)? {
            match Self::repository_roots(c, &source) {
                Ok(repositories) => {
                    for repository in &repositories {
                        groups.insert(
                            (source.alias.clone(), repository.clone()),
                            RepositoryGroup {
                                source: source.alias.clone(),
                                repository: repository.clone(),
                                count: 0,
                                missing: 0,
                                available: 0,
                                unknown: false,
                            },
                        );
                    }
                    roots.insert(source.alias, Some(repositories));
                }
                Err(_) => {
                    groups.insert(
                        (source.alias.clone(), String::new()),
                        RepositoryGroup {
                            source: source.alias.clone(),
                            repository: String::new(),
                            count: 0,
                            missing: 0,
                            available: 0,
                            unknown: true,
                        },
                    );
                    roots.insert(source.alias, None);
                }
            }
        }
        let mut error_count = 0;
        let mut statement = c.prepare("SELECT k.source,k.relative_path,k.missing,k.error IS NULL FROM skills k JOIN sources s ON s.alias=k.source WHERE s.active=1")?;
        for row in statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, bool>(2)?,
                row.get::<_, bool>(3)?,
            ))
        })? {
            let (source, relative, missing, valid) = row?;
            error_count += usize::from(!valid);
            let repository = Self::repository_for(
                &relative,
                roots
                    .get(&source)
                    .and_then(|roots| roots.as_ref())
                    .into_iter()
                    .flatten()
                    .map(String::as_str),
            )
            .to_owned();
            let group = groups
                .entry((source.clone(), repository.clone()))
                .or_insert(RepositoryGroup {
                    source,
                    repository,
                    count: 0,
                    missing: 0,
                    available: 0,
                    unknown: false,
                });
            group.count += 1;
            group.missing += usize::from(missing);
            group.available += usize::from(!missing && valid);
        }
        Ok((groups.into_values().collect(), error_count))
    }

    pub fn repository_selection(&self, skill: &SkillSummary) -> Result<RepositorySelection> {
        let c = self.connection()?;
        let source = Self::sources_on(&c)?
            .into_iter()
            .find(|source| source.alias == skill.source)
            .context("Source not found")?;
        let roots = Self::repository_roots(&c, &source)?;
        let repository =
            Self::repository_for(&skill.relative_path, roots.iter().map(String::as_str)).to_owned();
        Ok(RepositorySelection {
            repository_source: skill.source.clone(),
            repository,
        })
    }

    pub fn search_repository(
        &self,
        query: SearchQuery,
        selection: RepositorySelection,
    ) -> Result<SearchResults> {
        self.search_inner(query, false, Some(selection), None, false)
    }

    pub(crate) fn search_repository_with_groups(
        &self,
        query: SearchQuery,
        selection: RepositorySelection,
        groups: &[RepositoryGroup],
    ) -> Result<SearchResults> {
        self.search_inner(query, false, Some(selection), Some(groups), false)
    }

    pub fn search(&self, query: SearchQuery, published_only: bool) -> Result<SearchResults> {
        self.search_inner(query, published_only, None, None, false)
    }

    fn search_inner(
        &self,
        query: SearchQuery,
        published_only: bool,
        selection: Option<RepositorySelection>,
        shared_groups: Option<&[RepositoryGroup]>,
        collect_all: bool,
    ) -> Result<SearchResults> {
        let owned_groups;
        let groups: &[RepositoryGroup] = if let Some(groups) = shared_groups {
            groups
        } else if selection.is_some() {
            owned_groups = self.repository_groups()?;
            &owned_groups
        } else {
            &[]
        };
        if let Some(selection) = &selection {
            ensure!(
                !groups
                    .iter()
                    .any(|group| group.source == selection.repository_source && group.unknown),
                "取得元を確認できません。ソースの接続・権限を確認して再スキャンしてください。"
            );
        }
        let c = self.connection()?;
        let words: Vec<String> = query.q.split_whitespace().map(str::to_lowercase).collect();
        // 索引側も同じ Rust lowercase で正規化し、Unicode 展開の差による候補漏れを防ぐ。
        let long: Vec<_> = words.iter().filter(|w| w.chars().count() >= 3).collect();
        let expression = long
            .iter()
            .map(|w| format!("\"{}\"", w.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(" AND ");
        let sql = "WITH duplicates AS (
            SELECT d.name,count(*) AS n FROM skills d JOIN sources ds ON ds.alias=d.source
            WHERE ds.active=1 AND d.missing=0 AND d.error IS NULL AND d.name<>'' GROUP BY d.name
        ) SELECT k.id,k.source,k.relative_path,k.name,k.description,k.tags,k.category,k.published,k.error,k.missing,k.updated_at,coalesce(d.n,0)>1,
            CASE WHEN ?1=0 THEN '' ELSE k.content END
        FROM skills k JOIN sources s ON s.alias=k.source LEFT JOIN duplicates d ON d.name=k.name
        WHERE s.active=1
            AND (?2=0 OR (k.published=1 AND k.missing=0 AND k.error IS NULL))
            AND (?3='' OR k.source=?3)
            AND (?4='' OR EXISTS(SELECT 1 FROM json_each(k.tags) WHERE value=?4 COLLATE NOCASE))
            AND (?5='' OR k.category=?5)
            AND (?6='' OR k.id IN(SELECT id FROM skill_fts WHERE skill_fts MATCH ?6))
            AND CASE ?7 WHEN 'present' THEN k.missing=0 WHEN 'published' THEN k.published=1 WHEN 'unpublished' THEN k.published=0
                WHEN 'error' THEN k.error IS NOT NULL WHEN 'errors' THEN k.error IS NOT NULL
                WHEN 'missing' THEN k.missing=1 WHEN 'duplicate' THEN coalesce(d.n,0)>1 WHEN 'duplicates' THEN coalesce(d.n,0)>1 ELSE 1 END";
        let sql = if long.is_empty() {
            sql.to_owned()
        } else {
            sql.replace(
                "(?6='' OR k.id IN(SELECT id FROM skill_fts WHERE skill_fts MATCH ?6))",
                "k.id IN(SELECT id FROM skill_fts WHERE skill_fts MATCH ?6)",
            )
        };
        let mut st = c.prepare(&sql)?;
        let mut matches = Vec::new();
        for row in st.query_map(
            params![
                !words.is_empty(),
                published_only,
                query.source,
                query.tag,
                query.category,
                expression,
                query.status
            ],
            |r| Ok((summary_row(r)?, r.get::<_, String>(12)?)),
        )? {
            let (s, content) = row?;
            if let Some(selection) = &selection {
                let repository = Self::repository_for(
                    &s.relative_path,
                    groups
                        .iter()
                        .filter(|group| group.source == s.source)
                        .map(|group| group.repository.as_str()),
                );
                if s.source != selection.repository_source || repository != selection.repository {
                    continue;
                }
            }
            let score = if words.is_empty() {
                0
            } else {
                let haystack = format!(
                    "{} {} {} {} {}",
                    s.name,
                    s.description,
                    s.tags.join(" "),
                    s.category,
                    content
                )
                .to_lowercase();
                // Literal contains is the Unicode-safe fallback for short terms and verifies FTS candidates.
                if !words.iter().all(|w| haystack.contains(w)) {
                    continue;
                }
                let name = s.name.to_lowercase();
                let tags = s.tags.join(" ").to_lowercase();
                words
                    .iter()
                    .map(|w| {
                        if name.contains(w) {
                            10
                        } else if tags.contains(w) {
                            5
                        } else {
                            1
                        }
                    })
                    .sum::<usize>()
            };
            matches.push((score, s));
        }
        let total = matches.len();
        let (offset, limit) = if collect_all {
            (0, total)
        } else if query.limit == 0 {
            (query.offset, 50)
        } else {
            (query.offset, query.limit.min(100))
        };
        let compare = |a: &(usize, SkillSummary), b: &(usize, SkillSummary)| {
            b.0.cmp(&a.0)
                .then_with(|| a.1.name.cmp(&b.1.name))
                .then_with(|| a.1.id.cmp(&b.1.id))
        };
        if offset >= total {
            matches.clear();
        } else {
            let keep = offset.saturating_add(limit).min(total);
            if keep < total {
                matches.select_nth_unstable_by(keep, compare);
                matches.truncate(keep);
            }
            matches.sort_by(compare);
        }
        let skills = matches
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|(_, s)| s)
            .collect();
        let indexed_at = c
            .query_row(
                "SELECT value FROM settings WHERE key='indexed_at'",
                [],
                |r| r.get(0),
            )
            .unwrap_or_default();
        Ok(SearchResults {
            skills,
            total,
            indexed_at,
        })
    }
    fn locate(&self, c: &Connection, id: &str) -> Result<(PathBuf, PathBuf)> {
        // 一括操作・不在削除のループから呼ばれるため prepare_cached で再利用する。
        // 準備の失敗も従来の query_row と同じく "Skill not found" の文脈で返す。
        let (root, relative): (String, String) = c
            .prepare_cached("SELECT s.path,k.relative_path FROM skills k JOIN sources s ON s.alias=k.source WHERE k.id=?1 AND s.active=1")
            .and_then(|mut st| st.query_row([id], |r| Ok((r.get(0)?, r.get(1)?))))
            .context("Skill not found")?;
        let root = PathBuf::from(root);
        let path = safe_join(&root, &relative)?;
        Ok((root, path))
    }
    pub fn detail(&self, id: &str, published_only: bool) -> Result<SkillDetail> {
        self.detail_inner(id, published_only, true, true, true)
    }

    pub(crate) fn mcp_detail(&self, id: &str) -> Result<SkillDetail> {
        self.detail_inner(id, true, true, false, false)
    }

    // 公開・実ファイルの検査を共通化し、取得先が使わない一覧や履歴は読まない。
    fn detail_inner(
        &self,
        id: &str,
        published_only: bool,
        include_files: bool,
        include_history: bool,
        include_duplicate: bool,
    ) -> Result<SkillDetail> {
        let c = self.connection()?;
        // 同名スキルの件数（相関サブクエリ）は管理画面の詳細（`detail`）だけが使う。
        // MCPの `load_skill`・`read_file` の前後検査は `summary.duplicate` を返さないため、
        // ?2 が偽なら評価せず 0（false）にして、毎回の全表走査を省く。列位置は summary_row と共通。
        let mut st=c.prepare("SELECT id,source,relative_path,name,description,tags,category,published,error,missing,updated_at,CASE WHEN ?2 THEN (SELECT count(*)>1 FROM skills d JOIN sources ds ON ds.alias=d.source WHERE ds.active=1 AND d.missing=0 AND d.error IS NULL AND d.name=k.name AND k.name<>'') ELSE 0 END FROM skills k WHERE id=?1")?;
        let mut summary = st
            .query_row(params![id, include_duplicate], summary_row)
            .context("Skill not found")?;
        let index_unavailable = summary.missing || summary.error.is_some();
        ensure!(
            !published_only || (summary.published && !summary.missing && summary.error.is_none()),
            "Skill is not published or available"
        );
        let (_, path) = self.locate(&c, id)?;
        let (content, missing) = match read_text(&path, CONTENT_LIMIT) {
            Ok(content) => (content, false),
            Err(error)
                if !published_only
                    && error
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
            {
                summary.missing = true;
                (
                    c.query_row("SELECT content FROM skills WHERE id=?1", [id], |row| {
                        row.get(0)
                    })?,
                    true,
                )
            }
            Err(error) => return Err(error),
        };
        let parsed = parse(&content);
        let needs_rescan = !missing && parsed.is_ok() && index_unavailable;
        if !missing {
            summary.missing = false;
        }
        if published_only {
            let (name, description) = parsed?;
            summary.name = name;
            summary.description = description;
        } else {
            match parsed {
                Ok((n, d)) => {
                    summary.name = n;
                    summary.description = d;
                    summary.error = None
                }
                Err(e) => summary.error = Some(e.to_string()),
            }
        }
        let mut files = Vec::new();
        if include_files && path.parent().is_some_and(Path::is_dir) {
            let boundaries = Self::nested_skill_roots(&c, id, path.parent().unwrap())?;
            auxiliary(
                path.parent().context("Missing skill directory")?,
                path.parent().unwrap(),
                &mut files,
                &boundaries,
            )?;
        }
        files.sort();
        let history = if include_history {
            let mut hs = c.prepare(
                "SELECT id,created_at,reason FROM history WHERE skill_id=?1 ORDER BY id DESC",
            )?;
            hs.query_map([id], |r| {
                Ok(HistoryEntry {
                    id: r.get(0)?,
                    created_at: r.get(1)?,
                    reason: r.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?
        } else {
            Vec::new()
        };
        Ok(SkillDetail {
            codex_policy: Self::codex_policy_on(&c, id)?,
            summary,
            needs_rescan,
            revision: if missing {
                "missing".into()
            } else {
                revision(&content)
            },
            content,
            files,
            history,
        })
    }
    pub fn save_content(&self, id: &str, expected_revision: &str, content: &str) -> Result<()> {
        let _guard = self.write_guard()?;
        self.ensure_no_pending_migration()?;
        self.save_locked(id, expected_revision, content, "edit")
    }
    fn save_locked(&self, id: &str, expected: &str, content: &str, reason: &str) -> Result<()> {
        ensure!(
            content.len() as u64 <= CONTENT_LIMIT,
            "Content exceeds 2 MiB"
        );
        let (name, description) = parse(content)?;
        let c = self.connection()?;
        let (root, path) = self.locate(&c, id)?;
        let missing = expected == "missing";
        let old = if missing {
            ensure!(
                !path.exists(),
                "Conflict: file appeared externally; reload before saving"
            );
            ensure!(
                root.is_dir() && path.parent().is_some_and(Path::is_dir),
                "Source or skill directory is unavailable; restore the directory before recreating SKILL.md"
            );
            c.query_row("SELECT content FROM skills WHERE id=?1", [id], |row| {
                row.get::<_, String>(0)
            })?
        } else {
            let old = read_text(&path, CONTENT_LIMIT)?;
            ensure!(
                revision(&old) == expected,
                "Conflict: file changed externally; reload before saving"
            );
            old
        };
        if !missing && old == content {
            return Ok(());
        }
        // Commit a recoverable copy before changing the authoritative file.
        c.execute(
            "INSERT INTO history(skill_id,content,reason) VALUES(?1,?2,?3)",
            params![id, old, reason],
        )?;
        if missing {
            reject_links(&path)?;
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options
                .open(&path)
                .context("Conflict: cannot recreate the missing file")?;
            file.write_all(content.as_bytes())?;
            file.sync_all()?;
        } else {
            atomic_write(&path, content, Some(expected))?;
        }
        let tx = c.unchecked_transaction()?;
        tx.execute("UPDATE skills SET name=?2,description=?3,content=?4,revision=?5,error=NULL,missing=0,updated_at=CURRENT_TIMESTAMP WHERE id=?1",params![id,name,description,content,revision(content)])?;
        refresh_fts(&tx, id)?;
        tx.commit()?;
        Ok(())
    }
    pub fn set_metadata(&self, id: &str, metadata: SkillMetadata) -> Result<()> {
        let _guard = self.write_guard()?;
        self.ensure_no_pending_migration()?;
        let c = self.connection()?;
        self.locate(&c, id)?;
        let tags = normalize_tags(&metadata.tags)?;
        ensure!(metadata.category.len() <= 200, "Category is too long");
        let tx = c.unchecked_transaction()?;
        if metadata.published {
            refresh_valid(&tx, id, &self.locate(&tx, id)?.1)?;
        }
        tx.execute(
            "UPDATE skills SET tags=?2,category=?3,published=?4 WHERE id=?1",
            params![
                id,
                serde_json::to_string(&tags)?,
                metadata.category,
                metadata.published
            ],
        )?;
        refresh_fts(&tx, id)?;
        tx.commit()?;
        Ok(())
    }
    /// 一覧のラジオボタンから、1件の公開状態だけを切り替える。
    ///
    /// タグ・カテゴリは変更しない（管理情報フォームと違い、他の項目を上書きしない）。
    /// 公開時は `set_metadata` と同じくファイルを再検証し、不在・読み取り不能・不正な
    /// frontmatterなら公開しない。再検証で本文が更新されるため、その場合だけ対象行の
    /// 検索索引を置き換える。
    pub fn set_published(&self, id: &str, published: bool) -> Result<()> {
        let _guard = self.write_guard()?;
        self.ensure_no_pending_migration()?;
        let c = self.connection()?;
        let tx = c.unchecked_transaction()?;
        let (_, path) = self.locate(&tx, id)?;
        if published {
            refresh_valid(&tx, id, &path)?;
        }
        tx.execute(
            "UPDATE skills SET published=?2 WHERE id=?1",
            params![id, published],
        )?;
        if published {
            refresh_fts(&tx, id)?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn forget_missing(
        &self,
        mut query: SearchQuery,
        selection: RepositorySelection,
        expected: usize,
    ) -> Result<usize> {
        ensure!(
            expected > 0,
            "削除対象がありません。再読み込みしてください。"
        );
        let _guard = self.write_guard()?;
        self.ensure_no_pending_migration()?;
        query.status = "missing".into();
        query.source = selection.repository_source.clone();
        let selection = (!selection.repository_source.is_empty()).then_some(selection);
        let matches = self.search_inner(query, false, selection, None, true)?;
        ensure!(
            matches.total == expected,
            "対象件数が変わりました。再読み込みしてください。"
        );
        let ids: Vec<_> = matches.skills.into_iter().map(|skill| skill.id).collect();
        let c = self.connection()?;
        let tx = c.unchecked_transaction()?;
        // 接続障害・権限不足・復活したファイルを、不在として削除しない。
        for id in &ids {
            let (root, path) = self.locate(&tx, id)?;
            fs::read_dir(&root)
                .context("ソースを確認できません。接続・権限を確認してください。")?;
            match fs::symlink_metadata(&path) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                _ => bail!(
                    "パスが存在するか不在を確認できないため、削除を中止しました。再スキャンしてください。"
                ),
            }
        }
        // 3文の固定SQLをループの外で1回だけ準備し、件数分の再コンパイルを省く。
        // 同じトランザクション内で、1件ごとに履歴→ポリシー→索引の順で削除する順序は従来どおり。
        let mut delete_history = tx.prepare("DELETE FROM history WHERE skill_id=?1")?;
        let mut delete_policy = tx.prepare("DELETE FROM settings WHERE key=?1")?;
        let mut delete_skill = tx.prepare("DELETE FROM skills WHERE id=?1 AND missing=1")?;
        for id in &ids {
            delete_history.execute([id])?;
            delete_policy.execute([format!("codex_policy:{id}")])?;
            delete_skill.execute([id])?;
        }
        // 準備済み文はトランザクションを借用するため、FTS再構築・コミットの前に解放する。
        drop((delete_history, delete_policy, delete_skill));
        rebuild_fts(&tx)?;
        tx.commit()?;
        Ok(ids.len())
    }

    pub fn bulk_metadata(
        &self,
        ids: &[String],
        published: Option<bool>,
        add_tags: &[String],
    ) -> Result<()> {
        self.bulk_settings(ids, published, add_tags, None)
    }
    pub fn bulk_settings(
        &self,
        ids: &[String],
        published: Option<bool>,
        add_tags: &[String],
        codex: Option<CodexSkillPolicy>,
    ) -> Result<()> {
        if let Some(policy) = &codex {
            ensure!(
                !policy.automatic || policy.command,
                "Automatic invocation requires a visible command"
            );
        }
        let _guard = self.write_guard()?;
        self.ensure_no_pending_migration()?;
        let additions = normalize_tags(add_tags)?;
        let c = self.connection()?;
        let tx = c.unchecked_transaction()?;
        self.bulk_settings_on(&tx, ids, published, &additions, codex.as_ref())?;
        rebuild_fts(&tx)?;
        tx.commit()?;
        Ok(())
    }

    fn bulk_settings_on(
        &self,
        c: &Connection,
        ids: &[String],
        published: Option<bool>,
        additions: &[String],
        codex: Option<&CodexSkillPolicy>,
    ) -> Result<()> {
        for id in ids {
            let (_, path) = self.locate(c, id)?;
            if published == Some(true) {
                refresh_valid(c, id, &path)?;
            }
            // ループ内の固定SQLは prepare_cached で再利用する（実行順・トランザクションは呼び出し元のまま）。
            let (raw, old): (String, bool) = c
                .prepare_cached("SELECT tags,published FROM skills WHERE id=?1")?
                .query_row([id], |r| Ok((r.get(0)?, r.get(1)?)))?;
            let mut tags: Vec<String> = serde_json::from_str(&raw)?;
            tags.extend(additions.iter().cloned());
            let tags = normalize_tags(&tags)?;
            c.prepare_cached("UPDATE skills SET tags=?2,published=?3 WHERE id=?1")?
                .execute(params![
                    id,
                    serde_json::to_string(&tags)?,
                    published.unwrap_or(old)
                ])?;
            if let Some(policy) = codex {
                // 固定SQLは put 内の prepare_cached で接続ごとに1回だけ準備される。
                put(
                    c,
                    &format!("codex_policy:{id}"),
                    &serde_json::to_string(policy)?,
                )?;
            }
        }
        Ok(())
    }

    pub fn bulk_repository_settings(
        &self,
        selections: &[RepositorySelection],
        published: Option<bool>,
        codex: Option<CodexSkillPolicy>,
    ) -> Result<(usize, usize)> {
        ensure!(!selections.is_empty(), "取得元を選択してください。");
        ensure!(
            published.is_some() || codex.is_some(),
            "変更する設定を選択してください。"
        );
        if let Some(policy) = &codex {
            ensure!(
                !policy.automatic || policy.command,
                "Automatic invocation requires a visible command"
            );
        }
        let _guard = self.write_guard()?;
        self.ensure_no_pending_migration()?;
        let c = self.connection()?;
        let tx = c.unchecked_transaction()?;
        let (groups, _) = Self::repository_groups_on(&tx)?;
        let scopes: HashSet<_> = selections
            .iter()
            .map(|selection| {
                (
                    selection.repository_source.as_str(),
                    selection.repository.as_str(),
                )
            })
            .collect();
        for (source, repository) in &scopes {
            ensure!(
                groups.iter().any(|group| group.source == *source
                    && group.repository == *repository
                    && !group.unknown
                    && group.count > 0),
                "取得元が存在しないか、対象のスキルがありません。再スキャンしてください。"
            );
        }
        // キャッシュされた取得元でも、切断中のソースへ設定を書き込まない。
        for source in Self::sources_on(&tx)? {
            if scopes.iter().any(|(alias, _)| *alias == source.alias) {
                Self::detect_repositories(Path::new(&source.path))?;
            }
        }
        let mut ids = Vec::new();
        let mut skipped = 0;
        {
            let mut statement = tx.prepare("SELECT k.id,k.source,k.relative_path,k.missing,k.error IS NULL FROM skills k JOIN sources s ON s.alias=k.source WHERE s.active=1")?;
            for row in statement.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, bool>(3)?,
                    row.get::<_, bool>(4)?,
                ))
            })? {
                let (id, source, relative, missing, valid) = row?;
                let repository = Self::repository_for(
                    &relative,
                    groups
                        .iter()
                        .filter(|group| group.source == source)
                        .map(|group| group.repository.as_str()),
                );
                if scopes.contains(&(source.as_str(), repository)) {
                    if missing || !valid {
                        skipped += 1;
                    } else {
                        ids.push(id);
                    }
                }
            }
        }
        ensure!(
            !ids.is_empty(),
            "設定を変更できるスキルがありません。再スキャンしてください。"
        );
        self.bulk_settings_on(&tx, &ids, published, &[], codex.as_ref())?;
        rebuild_fts(&tx)?;
        tx.commit()?;
        Ok((ids.len(), skipped))
    }
    pub fn restore(&self, id: &str, history_id: i64, expected_revision: &str) -> Result<()> {
        let _guard = self.write_guard()?;
        self.ensure_no_pending_migration()?;
        let content: String = self
            .connection()?
            .query_row(
                "SELECT content FROM history WHERE id=?1 AND skill_id=?2",
                params![history_id, id],
                |r| r.get(0),
            )
            .context("History entry not found")?;
        self.save_locked(id, expected_revision, &content, "restore")
    }
    pub fn create_skill(&self, source: &str, relative_dir: &str, content: &str) -> Result<String> {
        let _guard = self.write_guard()?;
        self.ensure_no_pending_migration()?;
        ensure!(
            content.len() as u64 <= CONTENT_LIMIT,
            "Content exceeds 2 MiB"
        );
        parse(content)?;
        let c = self.connection()?;
        let root: String = c
            .query_row(
                "SELECT path FROM sources WHERE alias=?1 AND active=1",
                [source],
                |r| r.get(0),
            )
            .context("Source not found")?;
        let root = PathBuf::from(root);
        let directory = safe_join(&root, relative_dir)?;
        ensure!(
            !directory.join("SKILL.md").exists(),
            "A skill already exists in this directory"
        );
        fs::create_dir_all(&directory)?;
        reject_links(&directory)?;
        let path = directory.join("SKILL.md");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        self.scan_locked(&c)?;
        let relative = relative_key(&root, &path)?;
        Ok(skill_id(source, &relative))
    }
    fn nested_skill_roots(c: &Connection, id: &str, root: &Path) -> Result<HashSet<PathBuf>> {
        // 消失扱いの子も境界として残す。マーカーを移動しただけでは親の公開範囲を広げない。
        let mut statement = c.prepare("SELECT s.path,k.relative_path FROM skills k JOIN sources s ON s.alias=k.source WHERE k.source=(SELECT source FROM skills WHERE id=?1) AND k.id<>?1")?;
        let mut boundaries = HashSet::new();
        for row in statement.query_map([id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })? {
            let (source, relative) = row?;
            let skill = Path::new(&source).join(relative);
            if let Some(directory) = skill.parent()
                && directory != root
                && directory.starts_with(root)
            {
                boundaries.insert(directory.to_path_buf());
            }
        }
        Ok(boundaries)
    }
    fn check_file_boundary(c: &Connection, id: &str, root: &Path, target: &Path) -> Result<()> {
        let boundaries = Self::nested_skill_roots(c, id, root)?;
        ensure!(
            !boundaries
                .iter()
                .any(|boundary| target.starts_with(boundary)),
            "File belongs to a nested skill; use its skill id"
        );
        let mut ancestor = target.parent().context("Invalid file path")?;
        while ancestor != root {
            ensure!(
                fs::symlink_metadata(ancestor.join("SKILL.md")).is_err(),
                "File belongs to a nested skill; use its skill id"
            );
            ancestor = ancestor.parent().context("Invalid file path")?;
        }
        Ok(())
    }
    pub fn read_file(
        &self,
        id: &str,
        path: &str,
        start_line: usize,
        max_lines: usize,
    ) -> Result<SkillFile> {
        let before = self.detail_inner(id, true, false, false, false)?;
        let c = self.connection()?;
        let (_, skill) = self.locate(&c, id)?;
        let root = skill.parent().context("Invalid skill directory")?;
        let target = safe_join(root, path)?;
        Self::check_file_boundary(&c, id, root, &target)?;
        ensure!(target.is_file(), "Not a file");
        let text = read_text(&target, FILE_LIMIT)?;
        // Recheck publication and authoritative frontmatter immediately before returning reference text.
        let after = self.detail_inner(id, true, false, false, false)?;
        ensure!(
            before.revision == after.revision,
            "Conflict: skill changed during file read"
        );
        Self::check_file_boundary(&c, id, root, &target)?;
        let file_revision = revision_parts(&[before.revision.as_bytes(), b"\0", text.as_bytes()]);
        let total_lines = text.lines().count();
        let start = start_line.max(1);
        let text = text
            .lines()
            .skip(start - 1)
            .take(max_lines.clamp(1, 1000))
            .collect::<Vec<_>>()
            .join("\n");
        Ok(SkillFile {
            revision: file_revision,
            path: path.to_owned(),
            text,
            start_line: start,
            total_lines,
        })
    }
}

fn summary_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<SkillSummary> {
    let raw: String = r.get(5)?;
    Ok(SkillSummary {
        id: r.get(0)?,
        source: r.get(1)?,
        relative_path: r.get(2)?,
        name: r.get(3)?,
        description: r.get(4)?,
        tags: serde_json::from_str(&raw).unwrap_or_default(),
        category: r.get(6)?,
        published: r.get(7)?,
        error: r.get(8)?,
        missing: r.get(9)?,
        updated_at: r.get(10)?,
        duplicate: r.get(11)?,
    })
}
fn rebuild_fts(c: &Connection) -> Result<()> {
    c.execute("DELETE FROM skill_fts", [])?;
    insert_fts(c, None)
}
fn refresh_fts(c: &Connection, id: &str) -> Result<()> {
    c.execute("DELETE FROM skill_fts WHERE id=?1", [id])?;
    insert_fts(c, Some(id))
}
fn insert_fts(c: &Connection, id: Option<&str>) -> Result<()> {
    let sql = format!(
        "SELECT id,name,description,content,tags,category FROM skills{}",
        if id.is_some() { " WHERE id=?1" } else { "" }
    );
    let mut rows = c.prepare(&sql)?;
    let mut insert = c.prepare("INSERT INTO skill_fts(id,name,description,content,tags,category) VALUES(?1,?2,?3,?4,?5,?6)")?;
    for row in rows.query_map(rusqlite::params_from_iter(id), |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
        ))
    })? {
        let (id, name, description, content, tags, category) = row?;
        let tags: Vec<String> = serde_json::from_str(&tags)?;
        insert.execute(params![
            id,
            name.to_lowercase(),
            description.to_lowercase(),
            content.to_lowercase(),
            tags.join(" ").to_lowercase(),
            category.to_lowercase()
        ])?;
    }
    Ok(())
}
fn refresh_valid(c: &Connection, id: &str, path: &Path) -> Result<()> {
    let content = read_text(path, CONTENT_LIMIT)?;
    let (name, description) = parse(&content)?;
    c.execute("UPDATE skills SET name=?2,description=?3,content=?4,revision=?5,error=NULL,missing=0,updated_at=CASE WHEN revision<>?5 THEN CURRENT_TIMESTAMP ELSE updated_at END WHERE id=?1", params![id,name,description,content,revision(&content)])?;
    Ok(())
}
/// `settings` テーブルから生の文字列値を読む。キーが無ければ `None`。
///
/// codex_index・migration・Library本体で同じSELECTを重複して書いていたため、
/// ここへ集約する。ループ内（ポリシーの一括読込みなど）からも呼ばれるので、
/// 固定SQLは `prepare_cached` で接続ごとに1回だけ準備する。
fn setting(c: &Connection, key: &str) -> Result<Option<String>> {
    Ok(c.prepare_cached("SELECT value FROM settings WHERE key=?1")?
        .query_row([key], |r| r.get(0))
        .optional()?)
}
/// `settings` のJSON値を読む。キーが無ければ `None`、JSONが不正なら従来どおり `Err`。
/// 既定値の扱い（`unwrap_or_default` など）は呼び出し元ごとに異なるため、ここでは決めない。
fn setting_json<T: serde::de::DeserializeOwned>(c: &Connection, key: &str) -> Result<Option<T>> {
    Ok(setting(c, key)?
        .map(|s| serde_json::from_str(&s))
        .transpose()?)
}
/// `settings` へ値をUPSERTする。一括設定のループからも呼ばれるため `prepare_cached` を使う。
/// トランザクション内では `&tx`（`Connection` へderefされる）を渡し、境界は呼び出し元が持つ。
fn put(c: &Connection, key: &str, value: &str) -> Result<()> {
    c.prepare_cached(
        "INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
    )?
    .execute(params![key, value])?;
    Ok(())
}
fn revision(text: &str) -> String {
    revision_parts(&[text.as_bytes()])
}
fn revision_parts(parts: &[&[u8]]) -> String {
    let mut hash = Sha256::new();
    for part in parts {
        hash.update(part);
    }
    // スキルIDもこの値から作られSQLiteに保存済みのため、16進表現は共通関数で固定する。
    crate::sha256_hex(&hash.finalize())
}
/// 旧来の明示識別子の書式。URL・設定キー・スキルIDの材料になるため制御文字や区切りを含めない。
fn valid_source_alias(alias: &str) -> bool {
    !alias.is_empty()
        && alias.len() <= 100
        && alias
            .chars()
            .all(|c| c.is_alphanumeric() || "_-".contains(c))
}
/// フォルダー名から自動採番の基になる識別子を作る。
///
/// 識別子はURLやスキルIDに入るため、ASCIIの英数字と `-` に限る。日本語などの
/// フォルダー名は表示名で保持し、識別子は `folder` に置き換える。
fn folder_alias_base(root: &Path) -> String {
    let name = root
        .file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let mut alias = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            alias.push(c);
        } else if !alias.is_empty() && !alias.ends_with('-') {
            alias.push('-');
        }
    }
    let alias: String = alias.trim_end_matches('-').chars().take(60).collect();
    if alias.is_empty() {
        "folder".into()
    } else {
        alias
    }
}
fn skill_id(alias: &str, path: &str) -> String {
    revision(&format!("{alias}\0{path}"))
}
fn normalize_tags(tags: &[String]) -> Result<Vec<String>> {
    ensure!(tags.len() <= 100, "Too many tags");
    let mut result = Vec::new();
    for tag in tags {
        let tag = tag.trim();
        ensure!(tag.len() <= 100, "Tag is too long");
        if !tag.is_empty() && !result.iter().any(|t: &String| t.eq_ignore_ascii_case(tag)) {
            result.push(tag.to_owned());
        }
    }
    Ok(result)
}
fn parse(content: &str) -> Result<(String, String)> {
    let mut lines = content.lines();
    ensure!(
        lines.next().map(|s| s.trim_start_matches('\u{feff}')) == Some("---"),
        "SKILL.md must start with YAML frontmatter (---)"
    );
    let mut yaml = String::new();
    let mut closed = false;
    for line in lines {
        if line == "---" {
            closed = true;
            break;
        }
        yaml.push_str(line);
        yaml.push('\n');
    }
    ensure!(closed, "Unclosed YAML frontmatter");
    let docs = saphyr::Yaml::load_from_str(&yaml).map_err(|error| {
        // YAMLから除いた先頭の「---」を補い、SKILL.md上の位置を表示する。
        let line = error.marker().line() + 1;
        let column = error.marker().col() + 1;
        let reason = if error.info() == "duplicated key in mapping" {
            "項目名が重複しています。同じ階層では項目名を1つにしてください"
        } else {
            error.info()
        };
        let excerpt: String = content
            .lines()
            .nth(line - 1)
            .unwrap_or_default()
            .trim()
            .chars()
            .take(160)
            .collect();
        anyhow::anyhow!(
            "YAMLヘッダーのエラー（SKILL.md の{line}行目・{column}列目）: {reason}{}",
            if excerpt.is_empty() {
                String::new()
            } else {
                format!("。該当行: {excerpt}")
            }
        )
    })?;
    ensure!(docs.len() == 1, "Expected one YAML document");
    let doc = &docs[0];
    let name = doc["name"]
        .as_str()
        .filter(|v| !v.trim().is_empty())
        .context("Frontmatter name must be a nonempty string")?
        .to_owned();
    let description = doc["description"]
        .as_str()
        .filter(|v| !v.trim().is_empty())
        .context("Frontmatter description must be a nonempty string")?
        .to_owned();
    Ok((name, description))
}
fn read_text(path: &Path, limit: u64) -> Result<String> {
    reject_links(path)?;
    let file = fs::File::open(path).with_context(|| format!("Cannot read {}", path.display()))?;
    ensure!(file.metadata()?.is_file(), "Not a regular file");
    ensure!(file.metadata()?.len() <= limit, "File exceeds size limit");
    let mut text = String::new();
    file.take(limit + 1)
        .read_to_string(&mut text)
        .context("File is not UTF-8 text")?;
    ensure!(text.len() as u64 <= limit, "File exceeds size limit");
    ensure!(!text.contains('\0'), "Binary files are not supported");
    Ok(text)
}
fn reject_links(path: &Path) -> Result<()> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut current = PathBuf::new();
    for component in absolute.components() {
        ensure!(
            !matches!(component, Component::ParentDir),
            "Parent traversal is forbidden"
        );
        current.push(component);
        if matches!(component, Component::Prefix(_)) {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(meta) => {
                ensure!(
                    !meta.file_type().is_symlink(),
                    "Symbolic links are forbidden"
                );
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    ensure!(
                        meta.file_attributes() & 0x400 == 0,
                        "Reparse points are forbidden"
                    );
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(e)
                    .with_context(|| format!("Cannot inspect path {}", current.display()));
            }
        }
    }
    Ok(())
}
fn safe_join(root: &Path, relative: &str) -> Result<PathBuf> {
    ensure!(
        !relative.is_empty() && !relative.contains('\0'),
        "Invalid relative path"
    );
    #[cfg(windows)]
    ensure!(
        !relative.contains('\\') && !relative.contains(':'),
        "Invalid Windows relative path"
    );
    let p = Path::new(relative);
    ensure!(
        p.components().all(|c| matches!(c, Component::Normal(_))),
        "Path must stay within its source"
    );
    for part in relative.split('/') {
        ensure!(!skipped(part), "Path is excluded from the library");
        #[cfg(windows)]
        ensure!(
            !part.ends_with(['.', ' ']),
            "Windows path aliases are forbidden"
        );
    }
    let mut target = root.to_owned();
    for part in p.components() {
        target.push(part);
    }
    reject_links(root)?;
    reject_links(&target)?;
    ensure!(target.starts_with(root), "Path escapes source");
    Ok(target)
}
fn skipped(name: &str) -> bool {
    name.starts_with(".skilvella-")
        || matches!(
            name.to_ascii_lowercase().as_str(),
            ".git" | "node_modules" | "target" | ".work"
        )
}
fn is_link(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    meta.file_type().is_symlink()
}
fn discover(
    dir: &Path,
    found: &mut Vec<PathBuf>,
    blocked: &mut Vec<PathBuf>,
    excluded: &std::collections::HashSet<String>,
    unverified: &std::collections::HashSet<PathBuf>,
    first_only: bool,
) -> Result<()> {
    if unverified
        .iter()
        .any(|region| migration::path_is_within(dir, region))
    {
        blocked.push(dir.to_path_buf());
        anyhow::bail!("Skill entry index ownership is unverified");
    }
    let skill = dir.join("SKILL.md");
    match fs::symlink_metadata(&skill) {
        Ok(meta) if meta.is_file() && !is_link(&meta) => {
            if !excluded.contains(&path_key(&skill)) {
                found.push(skill);
                if first_only {
                    return Ok(());
                }
            }
        }
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            blocked.push(dir.to_path_buf());
            return Err(error.into());
        }
        _ => {}
    }
    let mut first_error = None;
    let entries = fs::read_dir(dir).inspect_err(|_| {
        blocked.push(dir.to_path_buf());
    })?;
    for entry in entries {
        let result = (|| -> Result<()> {
            let entry = entry.inspect_err(|_| {
                blocked.push(dir.to_path_buf());
            })?;
            let meta = fs::symlink_metadata(entry.path()).inspect_err(|_| {
                blocked.push(entry.path());
            })?;
            if !is_link(&meta) && meta.is_dir() && !skipped(&entry.file_name().to_string_lossy()) {
                discover(
                    &entry.path(),
                    found,
                    blocked,
                    excluded,
                    unverified,
                    first_only,
                )?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            if first_only {
                return Err(error);
            }
            first_error.get_or_insert(error);
        }
        if first_only && !found.is_empty() {
            return Ok(());
        }
    }
    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
fn relative_key(root: &Path, path: &Path) -> Result<String> {
    ensure!(
        migration::path_is_within(path, root),
        "Path is outside its source"
    );
    path.components()
        .skip(root.components().count())
        .map(|component| {
            component
                .as_os_str()
                .to_str()
                .map(str::to_owned)
                .context("File paths must be valid UTF-8")
        })
        .collect::<Result<Vec<_>>>()
        .map(|parts| parts.join("/"))
}
fn path_collation() -> &'static str {
    if cfg!(any(windows, target_os = "macos")) {
        "NOCASE"
    } else {
        "BINARY"
    }
}
fn path_key(path: &Path) -> String {
    let path = path.to_string_lossy();
    if cfg!(windows) {
        path.replace('\\', "/").to_ascii_lowercase()
    } else if cfg!(target_os = "macos") {
        path.to_ascii_lowercase()
    } else {
        path.into_owned()
    }
}
fn auxiliary(
    root: &Path,
    dir: &Path,
    files: &mut Vec<String>,
    boundaries: &HashSet<PathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let meta = fs::symlink_metadata(entry.path())?;
        if is_link(&meta) || skipped(&entry.file_name().to_string_lossy()) {
            continue;
        }
        if meta.is_dir() {
            if boundaries.contains(&entry.path())
                || fs::symlink_metadata(entry.path().join("SKILL.md")).is_ok()
            {
                continue;
            }
            auxiliary(root, &entry.path(), files, boundaries)?;
        } else if meta.is_file() {
            files.push(relative_key(root, &entry.path())?);
        }
    }
    Ok(())
}
fn atomic_write(path: &Path, content: &str, expected: Option<&str>) -> Result<()> {
    reject_links(path)?;
    let permissions = fs::metadata(path)?.permissions();
    let temporary = path.with_file_name(format!(".skilvella-{}.tmp", uuid::Uuid::new_v4()));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary)?;
    file.write_all(content.as_bytes())?;
    #[cfg(target_os = "macos")]
    {
        use std::os::fd::AsRawFd;
        unsafe extern "C" {
            fn fcopyfile(
                from: std::ffi::c_int,
                to: std::ffi::c_int,
                state: *mut std::ffi::c_void,
                flags: u32,
            ) -> std::ffi::c_int;
        }
        let original = fs::File::open(path)?;
        // Apple COPYFILE_ACL(1) + XATTR(4)。新しい本文の更新時刻はそのまま保持する。
        let copied = unsafe {
            fcopyfile(
                original.as_raw_fd(),
                file.as_raw_fd(),
                std::ptr::null_mut(),
                5,
            )
        };
        ensure!(
            copied == 0,
            "Cannot preserve file metadata: {}",
            std::io::Error::last_os_error()
        );
    }
    file.set_permissions(permissions)?;
    file.sync_all()?;
    drop(file);
    if let Some(expected) = expected {
        ensure!(
            revision(&read_text(path, CONTENT_LIMIT)?) == expected,
            "Conflict: file changed externally; recoverable temporary file retained"
        );
    }
    reject_links(path)?;
    #[cfg(not(windows))]
    fs::rename(&temporary, path)?;
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn ReplaceFileW(
                replaced: *const u16,
                replacement: *const u16,
                backup: *const u16,
                flags: u32,
                exclude: *mut std::ffi::c_void,
                reserved: *mut std::ffi::c_void,
            ) -> i32;
        }
        let old: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let new: Vec<u16> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
        let result = unsafe {
            ReplaceFileW(
                old.as_ptr(),
                new.as_ptr(),
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if result == 0 {
            bail!(
                "Atomic replacement failed; original and recovery file retained: {}",
                std::io::Error::last_os_error()
            );
        }
    }
    Ok(())
}
