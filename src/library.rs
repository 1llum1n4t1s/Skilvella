use crate::model::*;
mod codex_index {
    use super::{CONTENT_LIMIT, Library, atomic_write, parse, read_text, reject_links, revision};
    use crate::model::{CodexSkillPolicy, CodexSyncSettings};
    use anyhow::{Context, Result, ensure};
    use rusqlite::{Connection, OptionalExtension, params};
    use serde::{Deserialize, Serialize};
    use std::{
        collections::BTreeMap,
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

    fn setting(c: &Connection, key: &str) -> Result<Option<String>> {
        Ok(
            c.query_row("SELECT value FROM settings WHERE key=?1", [key], |r| {
                r.get(0)
            })
            .optional()?,
        )
    }
    fn put(c: &Connection, key: &str, value: &str) -> Result<()> {
        c.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key,value])?;
        Ok(())
    }
    fn target(c: &Connection) -> Result<Option<PathBuf>> {
        let roots: Vec<PathBuf> = setting(c, "native_skill_roots")?
            .map(|s| serde_json::from_str(&s))
            .transpose()?
            .unwrap_or_default();
        match roots.first() {
            None => Ok(None),
            Some(root) => {
                ensure!(
                    root.is_absolute() && root.ends_with(".agents/skills"),
                    "Codex export requires the recorded .agents/skills root"
                );
                Ok(Some(root.join("skilvella")))
            }
        }
    }
    impl Library {
        pub(super) fn codex_policy_on(c: &Connection, id: &str) -> Result<CodexSkillPolicy> {
            Ok(setting(c, &format!("codex_policy:{id}"))?
                .map(|s| serde_json::from_str(&s))
                .transpose()?
                .unwrap_or_default())
        }
        pub fn codex_policy(&self, id: &str) -> Result<CodexSkillPolicy> {
            let c = self.connection()?;
            ensure!(
                c.query_row(
                    "SELECT EXISTS(SELECT 1 FROM skills WHERE id=?1)",
                    [id],
                    |r| r.get::<_, bool>(0)
                )?,
                "Skill not found"
            );
            Self::codex_policy_on(&c, id)
        }
        pub fn set_codex_policy(&self, id: &str, automatic: bool, command: bool) -> Result<()> {
            ensure!(
                !automatic || command,
                "Automatic invocation requires a visible Codex command"
            );
            let _guard = self.write_guard()?;
            self.ensure_no_pending_migration()?;
            self.codex_policy(id)?;
            let c = self.connection()?;
            put(
                &c,
                &format!("codex_policy:{id}"),
                &serde_json::to_string(&CodexSkillPolicy { automatic, command })?,
            )?;
            let _ = self.sync_codex_locked(&c);
            Ok(())
        }
        pub fn codex_settings(&self) -> Result<CodexSyncSettings> {
            let c = self.connection()?;
            let mut settings: CodexSyncSettings = setting(&c, "codex_sync")?
                .map(|s| serde_json::from_str(&s))
                .transpose()?
                .unwrap_or_default();
            settings.path = target(&c)?
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            Ok(settings)
        }
        pub fn set_codex_enabled(&self, enabled: bool) -> Result<()> {
            let _guard = self.write_guard()?;
            self.ensure_no_pending_migration()?;
            let mut settings = self.codex_settings()?;
            settings.enabled = enabled;
            let c = self.connection()?;
            put(&c, "codex_sync", &serde_json::to_string(&settings)?)?;
            let _ = self.sync_codex_locked(&c);
            Ok(())
        }
        pub fn sync_codex_entries(&self) -> Result<()> {
            let _guard = self.write_guard()?;
            self.ensure_no_pending_migration()?;
            self.sync_codex_locked(&self.connection()?)
        }
        fn sync_codex_locked(&self, c: &Connection) -> Result<()> {
            let mut settings: CodexSyncSettings = setting(c, "codex_sync")?
                .map(|s| serde_json::from_str(&s))
                .transpose()?
                .unwrap_or_default();
            let result = self.export_codex(c, settings.enabled);
            match &result {
                Ok(count) => {
                    settings.exported = *count;
                    settings.last_error.clear();
                }
                Err(error) => {
                    settings.last_error = format!("{error:#}");
                }
            }
            put(c, "codex_sync", &serde_json::to_string(&settings)?)?;
            result.map(|_| ())
        }
        fn export_codex(&self, c: &Connection, enabled: bool) -> Result<usize> {
            let root = target(c)?;
            ensure!(
                !enabled || root.is_some(),
                "Codex native skill root is not configured"
            );
            let state = self
                .db
                .parent()
                .context("Invalid database path")?
                .join("codex-index");
            reject_links(&state)?;
            let manifest_path = state.join("manifest.json");
            reject_links(&manifest_path)?;
            self.recover_codex(&state)?;
            self.cleanup_codex(&state)?;
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
                "Codex export belongs to another library"
            );
            if enabled {
                ensure_source_boundaries(c, root.as_deref().context("Missing Codex root")?)?;
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
                        format!("Codex source unavailable: {}", source.display())
                    })?;
                }
                let finder = "---\nname: skilvella-search\ndescription: Search the published Skilvella skill library on demand.\n---\nUse the Skilvella MCP search_skills tool to find relevant skills, then load_skill with the returned id. Fetch references only with read_skill_file after loading. If MCP is unavailable, report that it must be connected; do not substitute local files.\n";
                desired.insert("skilvella-search/SKILL.md".into(), finder.into());
                desired.insert(
                    "skilvella-search/agents/openai.yaml".into(),
                    "policy:\n  allow_implicit_invocation: false\n".into(),
                );
                let mut skills = c.prepare("SELECT k.id FROM skills k JOIN sources s ON s.alias=k.source WHERE s.active=1 AND k.published=1 AND k.missing=0 AND k.error IS NULL ORDER BY k.id")?;
                for id in skills.query_map([], |r| r.get::<_, String>(0))? {
                    let id = id?;
                    let policy = Self::codex_policy_on(c, &id)?;
                    if !policy.command {
                        continue;
                    }
                    let Some(entry) = self.codex_entry(c, &id)? else {
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
                    let duplicates: i64 = c.query_row("SELECT count(*) FROM skills k JOIN sources s ON s.alias=k.source WHERE s.active=1 AND k.published=1 AND k.missing=0 AND k.error IS NULL AND k.name=?1", [original], |r| r.get(0))?;
                    if name == "skilvella-search" || duplicates > 1 {
                        name.truncate(55);
                        name = format!("{}-{suffix}", name.trim_end_matches('-'));
                    }
                    ensure!(
                        !desired.contains_key(&format!("{name}/SKILL.md")),
                        "Codex command name collision: {name}"
                    );
                    let description = entry.description.replace('<', "(").replace('>', ")");
                    let description = if description.trim_start().starts_with("[TODO:") {
                        format!("Skilvella: {description}")
                    } else {
                        description
                    };
                    let description: String = description.chars().take(1024).collect();
                    let text = format!(
                        "---\nname: {}\ndescription: {}\n---\nUse Skilvella MCP load_skill with the fixed id {} before following this skill. Read references only through read_skill_file with this id. If MCP is unavailable, report that it must be connected; do not substitute local files.\n",
                        serde_json::to_string(&name)?,
                        serde_json::to_string(&description)?,
                        serde_json::to_string(&id)?
                    );
                    desired.insert(format!("{name}/SKILL.md"), text);
                    desired.insert(
                        format!("{name}/agents/openai.yaml"),
                        format!(
                            "interface:\n  display_name: {}\npolicy:\n  allow_implicit_invocation: {}\n",
                            serde_json::to_string(&if duplicates > 1 {
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
                    "Codex export belongs to another library"
                );
                ensure!(
                    old.root.is_absolute() && old.root.ends_with(".agents/skills/skilvella"),
                    "Invalid Codex ownership manifest root"
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
                    "Codex export destination already exists and is not owned by this library"
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
                    fs::create_dir_all(path.parent().context("Invalid Codex entry")?)?;
                    write_new(&path, &text)?;
                }
                validate_tree(&stage, &stage, &next.files)?;
                ensure!(
                    complete(&Manifest {
                        root: stage.clone(),
                        ..next.clone()
                    })?,
                    "Incomplete staged Codex index"
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
                self.recover_codex(&state)
                    .context("Cannot restore previous Codex index")?;
                return Err(error);
            }
            Ok(count)
        }
        fn codex_entry(&self, c: &Connection, id: &str) -> Result<Option<Entry>> {
            let row: Option<(String, String, String)> = c.query_row("SELECT k.source,k.relative_path,k.revision FROM skills k JOIN sources s ON s.alias=k.source WHERE k.id=?1 AND s.active=1 AND k.published=1 AND k.missing=0 AND k.error IS NULL", [id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
            let Some((source, relative_path, expected)) = row else {
                return Ok(None);
            };
            let (_, path) = self.locate(c, id)?;
            let content = match read_text(&path, CONTENT_LIMIT) {
                Ok(content) => content,
                Err(error) => {
                    if error
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|e| e.kind() != std::io::ErrorKind::NotFound)
                    {
                        return Err(error).context("Cannot read Codex source skill");
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
        fn cleanup_codex(&self, state: &Path) -> Result<()> {
            let staged = state.join("staged.json");
            reject_links(&staged)?;
            if staged.try_exists()? {
                let record: Staged = serde_json::from_str(&read_text(&staged, CONTENT_LIMIT)?)?;
                self.cleanup_codex_tree(&record.path, &record.manifest, "skilvella-staging")?;
                fs::remove_file(&staged)?;
            }
            let completed = state.join("last-completed.json");
            reject_links(&completed)?;
            if completed.try_exists()? {
                let record: Pending = serde_json::from_str(&read_text(&completed, CONTENT_LIMIT)?)?;
                self.cleanup_codex_tree(&record.stage, &record.next, "skilvella-staging")?;
                self.cleanup_codex_tree(&record.retired, &record.previous, "skilvella-retired")?;
            }
            Ok(())
        }
        fn cleanup_codex_tree(
            &self,
            path: &Path,
            manifest: &Manifest,
            category: &str,
        ) -> Result<()> {
            ensure!(
                manifest.owner == self.db,
                "Codex cleanup belongs to another library"
            );
            if path.as_os_str().is_empty() {
                return Ok(());
            }
            ensure!(
                manifest.root.is_absolute() && manifest.root.ends_with(".agents/skills/skilvella"),
                "Invalid Codex cleanup root"
            );
            let expected = manifest
                .root
                .parent()
                .and_then(Path::parent)
                .context("Invalid Codex cleanup root")?
                .join(category);
            ensure!(
                path.parent() == Some(expected.as_path())
                    && path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .is_some_and(|s| uuid::Uuid::parse_str(s).is_ok()),
                "Invalid Codex cleanup path"
            );
            ensure_source_boundaries(&self.connection()?, &manifest.root)?;
            validate_tree(path, path, &manifest.files).with_context(|| format!("Codex索引の一時領域 {} を安全に清掃できません。利用者の編集または未完の索引一時ファイルを確認して再試行してください", path.display()))?;
            if path.try_exists()? {
                remove_owned_tree(path, path, &manifest.files)?;
            }
            Ok(())
        }
        fn recover_codex(&self, state: &Path) -> Result<()> {
            let path = state.join("pending.json");
            reject_links(&path)?;
            if !path.try_exists()? {
                return Ok(());
            }
            let pending: Pending = serde_json::from_str(&read_text(&path, CONTENT_LIMIT)?)?;
            for manifest in [&pending.previous, &pending.next] {
                ensure!(
                    manifest.owner == self.db,
                    "Codex recovery belongs to another library"
                );
                ensure!(
                    manifest.root.as_os_str().is_empty()
                        || (manifest.root.is_absolute()
                            && manifest.root.ends_with(".agents/skills/skilvella")),
                    "Invalid Codex recovery root"
                );
                if !manifest.root.as_os_str().is_empty() {
                    ensure_source_boundaries(&self.connection()?, &manifest.root)?;
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
                    "Invalid Codex recovery path"
                );
                if !entry.as_os_str().is_empty() {
                    let parent = root
                        .parent()
                        .and_then(Path::parent)
                        .context("Invalid Codex root")?
                        .join(name);
                    ensure!(
                        entry.parent() == Some(parent.as_path())
                            && entry
                                .file_name()
                                .and_then(|s| s.to_str())
                                .is_some_and(|s| uuid::Uuid::parse_str(s).is_ok()),
                        "Invalid Codex recovery path"
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
    }
    fn ensure_source_boundaries(c: &Connection, root: &Path) -> Result<()> {
        let agents = root
            .parent()
            .and_then(Path::parent)
            .context("Invalid Codex root")?;
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
                    "Codex export overlaps a managed source: {}",
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
            .context("Invalid Codex root")?
            .join(category)
            .join(uuid::Uuid::new_v4().to_string()))
    }
    fn save_manifest(path: &Path, manifest: &Manifest) -> Result<()> {
        let text = serde_json::to_string(manifest)?;
        reject_links(path)?;
        if path.try_exists()? {
            atomic_write(path, &text, None)
        } else {
            write_new(path, &text)
        }
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
                    "Unknown directory in retired Codex index"
                );
                remove_owned_tree(root, &path, files)?;
            } else {
                let hash = files
                    .get(&relative)
                    .context("Unknown file in retired Codex index")?;
                ensure!(
                    revision(&read_text(&path, CONTENT_LIMIT)?) == *hash,
                    "Retired Codex index was edited by the user"
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
        ensure!(path.is_dir(), "Codex export directory was replaced");
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
                    "Unknown directory in Codex export: {relative}"
                );
                validate_tree(root, &path, files)?;
            } else {
                ensure!(entry.file_type()?.is_file(), "Special file in Codex export");
                let expected = files
                    .get(&relative)
                    .with_context(|| format!("Unknown file in Codex export: {}", path.display()))?;
                ensure!(
                    revision(&read_text(&path, CONTENT_LIMIT)?) == *expected,
                    "Codex export was edited by the user: {relative}"
                );
            }
        }
        Ok(())
    }
    fn write_new(path: &Path, text: &str) -> Result<()> {
        reject_links(path)?;
        let message = || {
            format!(
                "Codex索引ファイル {} を作成できません。未完の索引一時ファイルを確認して再試行してください",
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
mod migration {
    use super::{Library, rebuild_fts, refresh_valid, reject_links};
    use crate::model::ScanReport;
    use anyhow::{Context, Result, bail, ensure};
    use rusqlite::{Connection, OptionalExtension, params};
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
        pub fn dedicated_directory(&self) -> Result<PathBuf> {
            let data = self.db.parent().context("Invalid database location")?;
            let name = data.file_name().context("Invalid database location")?;
            Ok(data.with_file_name(format!("{}-library", name.to_string_lossy())))
        }

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
            c.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![NATIVE_ROOTS, serde_json::to_string(&roots)?])?;
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

        pub fn migrate_source(&self, alias: &str, destination: &Path) -> Result<ScanReport> {
            let _guard = self.write_guard()?;
            self.ensure_no_pending_migration()?;
            let c = self.connection()?;
            let from: String = c
                .query_row(
                    "SELECT path FROM sources WHERE alias=?1 AND active=1",
                    [alias],
                    |r| r.get(0),
                )
                .context("Source not found")?;
            let from = PathBuf::from(from);
            reject_links(&from)?;
            ensure!(from.is_dir(), "移行元フォルダーが見つかりません");
            for ancestor in from.ancestors().skip(1) {
                ensure!(
                    !exists(&ancestor.join(".git"))?,
                    "Gitリポジトリの一部分だけを移行することはできません"
                );
            }
            let dedicated = self.dedicated_directory()?;
            let default_destination = dedicated.join(alias);
            let normalized_destination = normalize_path(destination)?;
            if path_is_within(&normalized_destination, &default_destination)
                && path_is_within(&default_destination, &normalized_destination)
                && !exists(&dedicated)?
            {
                reject_links(&dedicated)?;
                ensure!(
                    !Self::is_native_root(&c, &dedicated)?,
                    "専用フォルダーがCodexの自動探索範囲内にあります"
                );
                let parent = dedicated.parent().context("専用フォルダーの親が必要です")?;
                ensure!(parent.is_dir(), "専用フォルダーの親が存在しません");
                let mut builder = fs::DirBuilder::new();
                builder.recursive(false);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::DirBuilderExt;
                    builder.mode(0o700);
                }
                builder.create(&dedicated)?;
                sync_directory(parent)?;
            }
            let to = checked_destination(destination)?;
            self.check_migration_bounds(&c, alias, &from, &to)?;
            check_tree(&from)?;
            let pending = PendingMigration {
                alias: alias.to_owned(),
                identity: directory_identity(&from)?,
                from,
                to,
            };
            c.execute(
                "INSERT INTO settings(key,value) VALUES(?1,?2)",
                params![PENDING, serde_json::to_string(&pending)?],
            )?;
            verify_identity(&pending.from, &pending.identity)?;
            if let Err(error) = rename_no_replace(&pending.from, &pending.to) {
                verify_identity(&pending.from, &pending.identity)?;
                clear_pending(&c)?;
                return Err(error).context(
                    "フォルダーを移動できません。同じボリューム上の未使用の移動先が必要です",
                );
            }
            match sync_rename_parents(&pending.from, &pending.to)
                .and_then(|_| self.finish_migration(&pending))
            {
                Ok(report) => Ok(report),
                Err(error) => match verify_identity(&pending.to, &pending.identity)
                    .and_then(|_| rename_no_replace(&pending.to, &pending.from))
                {
                    Ok(()) => {
                        verify_identity(&pending.from, &pending.identity)?;
                        sync_rename_parents(&pending.to, &pending.from).context("移行元へ戻しましたが、フォルダーの保存確認に失敗しました。再起動して復旧してください")?;
                        clear_pending(&c)?;
                        Err(error).context("索引を更新できなかったため移行元へ戻しました")
                    }
                    Err(rollback) => Err(error).context(format!(
                        "移行の復旧が必要です。移行元: {} / 移動先: {}。元への復帰失敗: {rollback}",
                        pending.from.display(),
                        pending.to.display()
                    )),
                },
            }
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
                "Codexが自動探索するフォルダー内へは移行できません"
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
        let value: Option<String> = c
            .query_row(
                "SELECT value FROM settings WHERE key=?1",
                [NATIVE_ROOTS],
                |r| r.get(0),
            )
            .optional()?;
        value
            .map(|value| serde_json::from_str(&value).map_err(Into::into))
            .unwrap_or_else(|| Ok(Vec::new()))
    }

    fn path_component_eq(left: &str, right: &str) -> bool {
        left.eq_ignore_ascii_case(right)
    }

    pub(super) fn overlaps(left: &Path, right: &Path) -> bool {
        path_is_within(left, right) || path_is_within(right, left)
    }

    fn path_is_within(path: &Path, root: &Path) -> bool {
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

    fn checked_destination(destination: &Path) -> Result<PathBuf> {
        ensure!(
            destination.is_absolute()
                && !destination
                    .components()
                    .any(|part| matches!(part, Component::CurDir | Component::ParentDir)),
            "移動先には正規の絶対パスが必要です"
        );
        reject_links(destination)?;
        ensure!(
            !exists(destination)?,
            "移動先は既に存在します。未使用のフォルダー名を指定してください"
        );
        let name = destination
            .file_name()
            .context("移動先のフォルダー名が必要です")?;
        let parent = destination
            .parent()
            .context("移動先の親フォルダーが必要です")?
            .canonicalize()
            .context("移動先の親フォルダーが存在しません")?;
        ensure!(
            parent.is_dir(),
            "移動先の親はフォルダーである必要があります"
        );
        let destination = parent.join(name);
        ensure!(
            destination.to_str().is_some(),
            "移動先はUTF-8である必要があります"
        );
        Ok(destination)
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
                bail!("移行元の内部にCodexが自動探索するスキルフォルダーがあるため移行できません");
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

    fn exists(path: &Path) -> Result<bool> {
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
        let value: Option<String> = c
            .query_row("SELECT value FROM settings WHERE key=?1", [PENDING], |r| {
                r.get(0)
            })
            .optional()?;
        value
            .map(|value| serde_json::from_str(&value).map_err(Into::into))
            .transpose()
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

#[cfg(windows)]
use anyhow::bail;
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension, params};
use saphyr::LoadableYamlNode;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

const CONTENT_LIMIT: u64 = 2 * 1024 * 1024;
const FILE_LIMIT: u64 = 1024 * 1024;

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
        let value: Option<String> = c
            .query_row(
                "SELECT value FROM settings WHERE key='directory'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        value
            .map(|s| serde_json::from_str(&s).map_err(Into::into))
            .unwrap_or_else(|| Ok(DirectorySettings::default()))
    }
    fn save_directory(c: &Connection, settings: &DirectorySettings) -> Result<()> {
        c.execute("INSERT INTO settings(key,value) VALUES('directory',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [serde_json::to_string(settings)?])?;
        Ok(())
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
        c.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![format!("directory_owned:{alias}"), if owned { "1" } else { "0" }])?;
        Ok(())
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
        let candidate = candidates
            .into_iter()
            .find(|p| p.is_dir() && reject_links(p).is_ok() && fs::read_dir(p).is_ok());
        let has_candidate = candidate.is_some();
        let detected = candidate.unwrap_or(fallback);
        let mut c = self.connection()?;
        let tx = c.transaction()?;
        let mut settings = Self::directory_on(&tx)?;
        let persisted_auto = settings.mode == "auto";
        settings.detected_path = detected.to_string_lossy().into_owned();
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
        let mut statement = c.prepare("SELECT s.alias,s.path,(SELECT count(*) FROM skills k WHERE k.source=s.alias AND k.missing=0),(SELECT count(*) FROM skills k WHERE k.source=s.alias AND (k.error IS NOT NULL OR k.missing=1)),(SELECT count(*) FROM skills k WHERE k.source=s.alias AND k.missing=0 AND k.error IS NULL) FROM sources s WHERE active=1 ORDER BY s.alias")?;
        let mut sources = statement
            .query_map([], |r| {
                Ok(Source {
                    alias: r.get(0)?,
                    display_name: String::new(),
                    path: r.get(1)?,
                    skill_count: r.get::<_, i64>(2)? as usize,
                    error_count: r.get::<_, i64>(3)? as usize,
                    mcp_candidate_count: r.get::<_, i64>(4)? as usize,
                    native_discovery: false,
                    native_discovery_unknown: false,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for source in &mut sources {
            source.display_name = Self::source_display_name_on(c, &source.alias)?;
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
            [format!("source_display_name:{alias}")],
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
        let key = format!("source_display_name:{alias}");
        if name.is_empty() || name == alias {
            tx.execute("DELETE FROM settings WHERE key=?1", [key])?;
        } else {
            tx.execute(
                "INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                params![key, name],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn add_source(&self, alias: &str, path: &Path) -> Result<ScanReport> {
        ensure!(
            !alias.is_empty()
                && alias.len() <= 100
                && alias
                    .chars()
                    .all(|c| c.is_alphanumeric() || "_-".contains(c)),
            "Invalid source alias"
        );
        let _guard = self.write_guard()?;
        self.ensure_no_pending_migration()?;
        reject_links(path)?;
        let root = path
            .canonicalize()
            .context("Source directory does not exist")?;
        ensure!(root.is_dir(), "Source must be a directory");
        ensure!(root.to_str().is_some(), "Source path must be valid UTF-8");
        let data = self.db.parent().context("Invalid database location")?;
        ensure!(
            !root.starts_with(data) && !data.starts_with(&root),
            "Source and library data directories must not overlap"
        );
        let mut c = self.connection()?;
        let tx = c.transaction()?;
        let mut stmt = tx.prepare("SELECT alias,path,active FROM sources")?;
        for row in stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, bool>(2)?,
            ))
        })? {
            let (old_alias, old_path, active) = row?;
            let old_root = PathBuf::from(old_path);
            if old_alias == alias {
                ensure!(
                    old_root == root,
                    "Alias already belongs to another directory"
                );
            } else if active {
                ensure!(
                    !root.starts_with(&old_root) && !old_root.starts_with(&root),
                    "Source directories overlap"
                );
            }
        }
        drop(stmt);
        tx.execute("INSERT INTO sources(alias,path,active) VALUES(?1,?2,1) ON CONFLICT(alias) DO UPDATE SET active=1", params![alias, root.to_string_lossy()])?;
        Self::set_source_ownership(&tx, alias, false)?;
        let report = self.scan_on(&tx)?;
        tx.commit()?;
        Ok(report)
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
        let mut report = ScanReport::default();
        for source in sources {
            let root = PathBuf::from(&source.path);
            let mut found = Vec::new();
            let mut blocked = Vec::new();
            let result =
                reject_links(&root).and_then(|_| discover(&root, &mut found, &mut blocked));
            // 部分的な読取失敗では未走査の行を消失扱いにせず、読めた兄弟を反映する。
            if result.is_ok() || !root.is_dir() {
                tx.execute(
                    "UPDATE skills SET missing=1 WHERE source=?1",
                    [&source.alias],
                )?;
            }
            if result.is_err() {
                if blocked.is_empty() {
                    blocked.push(root.clone());
                }
                report.errors += blocked.len();
                for path in blocked {
                    let key = relative_key(&root, &path).unwrap_or_default();
                    let prefix = format!("{key}/");
                    tx.execute("UPDATE skills SET error='Scan unavailable; check source permissions and rescan' WHERE source=?1 AND (?2='' OR relative_path=?2 OR substr(relative_path,1,length(?3))=?3)", params![source.alias,key,prefix])?;
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
                tx.execute("INSERT INTO skills(id,source,relative_path,name,description,content,revision,error,missing) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,0) ON CONFLICT(id) DO UPDATE SET name=excluded.name,description=excluded.description,content=excluded.content,revision=excluded.revision,error=excluded.error,missing=0,updated_at=CASE WHEN skills.revision<>excluded.revision THEN CURRENT_TIMESTAMP ELSE skills.updated_at END",params![id, source.alias,relative,name,description,content,revision(&content),error])?;
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
        let saved: Option<String> = c
            .query_row(
                "SELECT value FROM settings WHERE key=?1",
                [Self::repository_cache_key(source)],
                |row| row.get(0),
            )
            .optional()?;
        let saved: Option<Vec<String>> = saved
            .map(|value| serde_json::from_str(&value))
            .transpose()?;
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
                c.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                    params![Self::repository_cache_key(&source), serde_json::to_string(&roots)?])?;
            }
        }
        Ok(())
    }

    pub fn repository_groups(&self) -> Result<Vec<RepositoryGroup>> {
        let c = self.connection()?;
        let mut groups: std::collections::BTreeMap<(String, String), RepositoryGroup> =
            Default::default();
        let mut roots = std::collections::BTreeMap::new();
        for source in Self::sources_on(&c)? {
            match Self::repository_roots(&c, &source) {
                Ok(repositories) => {
                    for repository in &repositories {
                        groups.insert(
                            (source.alias.clone(), repository.clone()),
                            RepositoryGroup {
                                source: source.alias.clone(),
                                repository: repository.clone(),
                                count: 0,
                                missing: 0,
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
                            unknown: true,
                        },
                    );
                    roots.insert(source.alias, None);
                }
            }
        }
        let mut statement = c.prepare("SELECT k.source,k.relative_path,k.missing FROM skills k JOIN sources s ON s.alias=k.source WHERE s.active=1")?;
        for row in statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, bool>(2)?,
            ))
        })? {
            let (source, relative, missing) = row?;
            let repository = roots
                .get(&source)
                .and_then(|roots| roots.as_ref())
                .and_then(|roots| {
                    roots.iter().find(|root| {
                        root.as_str() == "." || relative.starts_with(&format!("{root}/"))
                    })
                })
                .cloned()
                .unwrap_or_default();
            let group = groups
                .entry((source.clone(), repository.clone()))
                .or_insert(RepositoryGroup {
                    source,
                    repository,
                    count: 0,
                    missing: 0,
                    unknown: false,
                });
            group.count += 1;
            group.missing += usize::from(missing);
        }
        Ok(groups.into_values().collect())
    }

    pub fn repository_selection(&self, skill: &SkillSummary) -> Result<RepositorySelection> {
        let c = self.connection()?;
        let source = Self::sources_on(&c)?
            .into_iter()
            .find(|source| source.alias == skill.source)
            .context("Source not found")?;
        let repository = Self::repository_roots(&c, &source)?
            .into_iter()
            .find(|repository| {
                repository == "." || skill.relative_path.starts_with(&format!("{repository}/"))
            })
            .unwrap_or_default();
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
        self.search_inner(query, false, Some(selection))
    }

    pub fn search(&self, query: SearchQuery, published_only: bool) -> Result<SearchResults> {
        self.search_inner(query, published_only, None)
    }

    fn search_inner(
        &self,
        query: SearchQuery,
        published_only: bool,
        selection: Option<RepositorySelection>,
    ) -> Result<SearchResults> {
        let groups = if selection.is_some() {
            self.repository_groups()?
        } else {
            Vec::new()
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
            AND CASE ?7 WHEN 'published' THEN k.published=1 WHEN 'unpublished' THEN k.published=0
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
                let repository = groups
                    .iter()
                    .filter(|group| {
                        group.source == s.source
                            && !group.repository.is_empty()
                            && (group.repository == "."
                                || s.relative_path
                                    .starts_with(&format!("{}/", group.repository)))
                    })
                    .max_by_key(|group| group.repository != ".")
                    .map(|group| group.repository.as_str())
                    .unwrap_or_default();
                if s.source != selection.repository_source || repository != selection.repository {
                    continue;
                }
            }
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
            let score = words
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
                .sum::<usize>();
            matches.push((score, s));
        }
        matches.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| a.1.name.cmp(&b.1.name))
                .then_with(|| a.1.id.cmp(&b.1.id))
        });
        let total = matches.len();
        let limit = if query.limit == 0 {
            50
        } else {
            query.limit.min(100)
        };
        let skills = matches
            .into_iter()
            .skip(query.offset)
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
        let (root,relative):(String,String)=c.query_row("SELECT s.path,k.relative_path FROM skills k JOIN sources s ON s.alias=k.source WHERE k.id=?1 AND s.active=1",[id],|r|Ok((r.get(0)?,r.get(1)?))).context("Skill not found")?;
        let root = PathBuf::from(root);
        let path = safe_join(&root, &relative)?;
        Ok((root, path))
    }
    pub fn detail(&self, id: &str, published_only: bool) -> Result<SkillDetail> {
        let c = self.connection()?;
        let mut st=c.prepare("SELECT id,source,relative_path,name,description,tags,category,published,error,missing,updated_at,(SELECT count(*)>1 FROM skills d JOIN sources ds ON ds.alias=d.source WHERE ds.active=1 AND d.missing=0 AND d.error IS NULL AND d.name=k.name AND k.name<>'') FROM skills k WHERE id=?1")?;
        let mut summary = st.query_row([id], summary_row).context("Skill not found")?;
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
        if path.parent().is_some_and(Path::is_dir) {
            let boundaries = Self::nested_skill_roots(&c, id, path.parent().unwrap())?;
            auxiliary(
                path.parent().context("Missing skill directory")?,
                path.parent().unwrap(),
                &mut files,
                &boundaries,
            )?;
        }
        files.sort();
        let mut hs = c.prepare(
            "SELECT id,created_at,reason FROM history WHERE skill_id=?1 ORDER BY id DESC",
        )?;
        let history = hs
            .query_map([id], |r| {
                Ok(HistoryEntry {
                    id: r.get(0)?,
                    created_at: r.get(1)?,
                    reason: r.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
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
        rebuild_fts(&tx)?;
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
        rebuild_fts(&tx)?;
        tx.commit()?;
        Ok(())
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
                "Automatic invocation requires a visible Codex command"
            );
        }
        let _guard = self.write_guard()?;
        self.ensure_no_pending_migration()?;
        let additions = normalize_tags(add_tags)?;
        let c = self.connection()?;
        let tx = c.unchecked_transaction()?;
        for id in ids {
            let (_, path) = self.locate(&tx, id)?;
            if published == Some(true) {
                refresh_valid(&tx, id, &path)?;
            }
            let (raw, old): (String, bool) =
                tx.query_row("SELECT tags,published FROM skills WHERE id=?1", [id], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })?;
            let mut tags: Vec<String> = serde_json::from_str(&raw)?;
            tags.extend(additions.clone());
            let tags = normalize_tags(&tags)?;
            tx.execute(
                "UPDATE skills SET tags=?2,published=?3 WHERE id=?1",
                params![id, serde_json::to_string(&tags)?, published.unwrap_or(old)],
            )?;
            if let Some(policy) = &codex {
                tx.execute(
                    "INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                    params![format!("codex_policy:{id}"), serde_json::to_string(policy)?],
                )?;
            }
        }
        rebuild_fts(&tx)?;
        tx.commit()?;
        Ok(())
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
    fn nested_skill_roots(c: &Connection, id: &str, root: &Path) -> Result<Vec<PathBuf>> {
        // 消失扱いの子も境界として残す。マーカーを移動しただけでは親の公開範囲を広げない。
        let mut statement = c.prepare("SELECT s.path,k.relative_path FROM skills k JOIN sources s ON s.alias=k.source WHERE k.source=(SELECT source FROM skills WHERE id=?1) AND k.id<>?1")?;
        let mut boundaries = Vec::new();
        for row in statement.query_map([id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })? {
            let (source, relative) = row?;
            let skill = Path::new(&source).join(relative);
            if let Some(directory) = skill.parent()
                && directory != root
                && directory.starts_with(root)
            {
                boundaries.push(directory.to_path_buf());
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
        let before = self.detail(id, true)?;
        let c = self.connection()?;
        let (_, skill) = self.locate(&c, id)?;
        let root = skill.parent().context("Invalid skill directory")?;
        let target = safe_join(root, path)?;
        Self::check_file_boundary(&c, id, root, &target)?;
        ensure!(target.is_file(), "Not a file");
        let text = read_text(&target, FILE_LIMIT)?;
        // Recheck publication and authoritative frontmatter immediately before returning reference text.
        let after = self.detail(id, true)?;
        ensure!(
            before.revision == after.revision,
            "Conflict: skill changed during file read"
        );
        Self::check_file_boundary(&c, id, root, &target)?;
        let lines: Vec<_> = text.lines().collect();
        let start = start_line.max(1);
        let total_lines = lines.len();
        let text = lines
            .iter()
            .skip(start - 1)
            .take(max_lines.clamp(1, 1000))
            .copied()
            .collect::<Vec<_>>()
            .join("\n");
        Ok(SkillFile {
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
    let mut rows = c.prepare("SELECT id,name,description,content,tags,category FROM skills")?;
    let mut insert = c.prepare("INSERT INTO skill_fts(id,name,description,content,tags,category) VALUES(?1,?2,?3,?4,?5,?6)")?;
    for row in rows.query_map([], |row| {
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
fn revision(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
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
fn discover(dir: &Path, found: &mut Vec<PathBuf>, blocked: &mut Vec<PathBuf>) -> Result<()> {
    let skill = dir.join("SKILL.md");
    match fs::symlink_metadata(&skill) {
        Ok(meta) if meta.is_file() && !is_link(&meta) => {
            found.push(skill);
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
                discover(&entry.path(), found, blocked)?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            first_error.get_or_insert(error);
        }
    }
    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
fn relative_key(root: &Path, path: &Path) -> Result<String> {
    path.strip_prefix(root)?
        .components()
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
fn auxiliary(
    root: &Path,
    dir: &Path,
    files: &mut Vec<String>,
    boundaries: &[PathBuf],
) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let meta = fs::symlink_metadata(entry.path())?;
        if is_link(&meta) || skipped(&entry.file_name().to_string_lossy()) {
            continue;
        }
        if meta.is_dir() {
            if boundaries.iter().any(|boundary| entry.path() == *boundary)
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
