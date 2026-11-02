use crate::model::*;
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
    sync::{Arc, Mutex},
    time::Duration,
};

const CONTENT_LIMIT: u64 = 2 * 1024 * 1024;
const FILE_LIMIT: u64 = 1024 * 1024;

#[derive(Clone)]
pub struct Library {
    db: PathBuf,
    writer: Arc<Mutex<()>>,
}

impl Library {
    pub fn open(data_dir: &Path) -> Result<Self> {
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
        Ok(library)
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
        let _guard = self
            .writer
            .lock()
            .map_err(|_| anyhow::anyhow!("Writer lock poisoned"))?;
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
        let _guard = self
            .writer
            .lock()
            .map_err(|_| anyhow::anyhow!("Writer lock poisoned"))?;
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
        let mut statement = c.prepare("SELECT s.alias,s.path,(SELECT count(*) FROM skills k WHERE k.source=s.alias AND k.missing=0),(SELECT count(*) FROM skills k WHERE k.source=s.alias AND (k.error IS NOT NULL OR k.missing=1)) FROM sources s WHERE active=1 ORDER BY s.alias")?;
        Ok(statement
            .query_map([], |r| {
                Ok(Source {
                    alias: r.get(0)?,
                    path: r.get(1)?,
                    skill_count: r.get::<_, i64>(2)? as usize,
                    error_count: r.get::<_, i64>(3)? as usize,
                })
            })?
            .collect::<rusqlite::Result<_>>()?)
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
        let _guard = self
            .writer
            .lock()
            .map_err(|_| anyhow::anyhow!("Writer lock poisoned"))?;
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
        let _guard = self
            .writer
            .lock()
            .map_err(|_| anyhow::anyhow!("Writer lock poisoned"))?;
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
        let _guard = self
            .writer
            .lock()
            .map_err(|_| anyhow::anyhow!("Writer lock poisoned"))?;
        self.scan_locked(&self.connection()?)
    }
    fn scan_locked(&self, c: &Connection) -> Result<ScanReport> {
        let tx = c.unchecked_transaction()?;
        let report = self.scan_on(&tx)?;
        tx.commit()?;
        Ok(report)
    }
    fn scan_on(&self, tx: &Connection) -> Result<ScanReport> {
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
    pub fn search(&self, query: SearchQuery, published_only: bool) -> Result<SearchResults> {
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
        let _guard = self
            .writer
            .lock()
            .map_err(|_| anyhow::anyhow!("Writer lock poisoned"))?;
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
        let _guard = self
            .writer
            .lock()
            .map_err(|_| anyhow::anyhow!("Writer lock poisoned"))?;
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
        let _guard = self
            .writer
            .lock()
            .map_err(|_| anyhow::anyhow!("Writer lock poisoned"))?;
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
        }
        rebuild_fts(&tx)?;
        tx.commit()?;
        Ok(())
    }
    pub fn restore(&self, id: &str, history_id: i64, expected_revision: &str) -> Result<()> {
        let _guard = self
            .writer
            .lock()
            .map_err(|_| anyhow::anyhow!("Writer lock poisoned"))?;
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
        let _guard = self
            .writer
            .lock()
            .map_err(|_| anyhow::anyhow!("Writer lock poisoned"))?;
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
    let docs = saphyr::Yaml::load_from_str(&yaml).context("Invalid YAML frontmatter")?;
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
