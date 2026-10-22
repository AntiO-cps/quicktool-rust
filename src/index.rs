use crate::core::ResultItem;
use notify::Watcher;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::mpsc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use unicode_normalization::UnicodeNormalization;
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Status {
    pub building: bool,
    pub paused: bool,
    pub count: u64,
    pub denied: u64,
    pub roots: Vec<PathBuf>,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: u64,
}
pub enum Request {
    SearchWithProvider {
        serial: u64,
        query: String,
        limit: usize,
        provider: String,
    },
    Search {
        serial: u64,
        query: String,
        limit: usize,
    },
    Configure {
        roots: Vec<PathBuf>,
        excludes: Vec<String>,
    },
    Pause(bool),
    Changed(PathBuf),
    Shutdown,
}
pub enum Response {
    Results { serial: u64, items: Vec<FileEntry> },
    Status(Status),
}
pub struct Index {
    pub requests: mpsc::Sender<Request>,
    pub responses: mpsc::Receiver<Response>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Index {
    pub fn start(
        database: PathBuf,
        roots: Vec<PathBuf>,
        excludes: Vec<String>,
        wake: impl Fn() + Send + 'static,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let (events, responses) = mpsc::channel();
        let internal = tx.clone();
        let failure_events = events.clone();
        let worker = std::thread::spawn(move || {
            let run = || -> Result<(), String> {
                if let Some(parent) = database.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?
                }
                let db = open(&database).map_err(|e| e.to_string())?;
                let mut worker = Worker {
                    db,
                    status: Status {
                        roots,
                        ..Default::default()
                    },
                    excludes: excludes
                        .into_iter()
                        .map(|s| crate::js_case::lower(&s))
                        .collect(),
                    rx,
                    events,
                    wake: Box::new(wake),
                    generation: 0,
                    stop: false,
                    watcher: None,
                    frequencies: HashMap::new(),
                };
                let watch_tx = internal;
                let mut watcher = notify::recommended_watcher(
                    move |event: Result<notify::Event, notify::Error>| {
                        if let Ok(event) = event {
                            for path in event.paths {
                                let _ = watch_tx.send(Request::Changed(path));
                            }
                        }
                    },
                )
                .map_err(|e| e.to_string())?;
                for root in &worker.status.roots {
                    if let Err(e) = watcher.watch(root, notify::RecursiveMode::Recursive) {
                        worker.status.error = Some(e.to_string());
                    }
                }
                worker.watcher = Some(watcher);
                worker.scan();
                while !worker.stop {
                    match worker.rx.recv_timeout(Duration::from_millis(500)) {
                        Ok(request) => worker.handle(request),
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(_) => break,
                    }
                }
                Ok(())
            };
            if let Err(e) = run() {
                let _ = failure_events.send(Response::Status(Status {
                    error: Some(e.clone()),
                    ..Default::default()
                }));
                eprintln!("File index: {e}");
            }
        });
        Self {
            requests: tx,
            responses,
            worker: Some(worker),
        }
    }
}
impl Drop for Index {
    fn drop(&mut self) {
        let _ = self.requests.send(Request::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let db = Connection::open(path)?;
    db.busy_timeout(Duration::from_secs(5))?;
    db.set_prepared_statement_cache_capacity(64);
    db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA cache_size=-8192;
        CREATE TABLE IF NOT EXISTS entries(id INTEGER PRIMARY KEY,path TEXT UNIQUE,name TEXT,search TEXT,root TEXT,generation INTEGER,is_dir INTEGER,size INTEGER,modified INTEGER,search_path TEXT);
        CREATE TABLE IF NOT EXISTS tokens(token TEXT,entry_id INTEGER,PRIMARY KEY(token,entry_id)) WITHOUT ROWID;")?;
    let columns = |table: &str| -> rusqlite::Result<Vec<String>> {
        db.prepare(&format!("PRAGMA table_info({table})"))?
            .query_map([], |row| row.get(1))?
            .collect()
    };
    let entry_columns = columns("entries")?;
    let legacy_entries = !entry_columns.iter().any(|name| name == "id");
    let has_search_path = entry_columns.iter().any(|name| name == "search_path");
    let legacy_tokens = columns("tokens")?.iter().any(|name| name == "path");
    let migrated = legacy_entries || legacy_tokens || !has_search_path;
    if migrated {
        db.execute_batch("BEGIN IMMEDIATE; DROP TRIGGER IF EXISTS cleanup;")?;
    }
    if legacy_entries {
        // An explicit INTEGER PRIMARY KEY preserves IDs across VACUUM. Hidden
        // rowids in the old TEXT-primary-key table may otherwise be renumbered.
        db.execute_batch("CREATE TABLE entries_integer(id INTEGER PRIMARY KEY,path TEXT UNIQUE,name TEXT,search TEXT,root TEXT,generation INTEGER,is_dir INTEGER,size INTEGER,modified INTEGER,search_path TEXT);")?;
        db.execute_batch(&format!("INSERT INTO entries_integer SELECT rowid,path,name,search,root,generation,is_dir,size,modified,{} FROM entries;
            DROP TABLE entries; ALTER TABLE entries_integer RENAME TO entries;", if has_search_path { "search_path" } else { "NULL" }))?;
    } else if !has_search_path {
        db.execute_batch("ALTER TABLE entries ADD COLUMN search_path TEXT;")?;
    }
    if legacy_tokens {
        db.execute_batch("CREATE TABLE tokens_integer(token TEXT,entry_id INTEGER,PRIMARY KEY(token,entry_id)) WITHOUT ROWID;
            INSERT INTO tokens_integer SELECT tokens.token,entries.id FROM tokens JOIN entries ON entries.path=tokens.path;
            DROP TABLE tokens; ALTER TABLE tokens_integer RENAME TO tokens;")?;
    }
    db.execute_batch("CREATE INDEX IF NOT EXISTS entry_roots ON entries(root,generation);
        CREATE INDEX IF NOT EXISTS token_entries ON tokens(entry_id);
        CREATE TRIGGER IF NOT EXISTS cleanup AFTER DELETE ON entries BEGIN DELETE FROM tokens WHERE entry_id=old.id;END;")?;
    if migrated {
        db.execute_batch("COMMIT;")?;
        // Reclaim the old repeated-path table after the atomic migration. A
        // compaction failure leaves the migrated, usable database intact.
        if let Err(error) = db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); VACUUM;") {
            eprintln!("File index compaction: {error}");
        }
    }
    Ok(db)
}

fn normalize(s: &str) -> String {
    if s.is_ascii() {
        s.to_ascii_lowercase()
    } else {
        crate::js_case::lower(&s.nfkc().collect::<String>())
    }
}
fn letter_or_number(c: char) -> bool {
    crate::js_case::is_alphanumeric(c)
}

fn grams(s: &str) -> Vec<String> {
    let chars: Vec<_> = s.chars().collect();
    let grams: Vec<String> = chars
        .windows(2)
        .filter(|pair| pair.iter().all(|&c| letter_or_number(c)))
        .map(|p| p.iter().collect())
        .collect();
    let mut unique = Vec::new();
    for gram in grams {
        if !unique.contains(&gram) {
            unique.push(gram);
        }
    }
    unique
}
fn escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}
// ECMAScript WhiteSpace + LineTerminator, shared by trim and /\S+/.
fn js_space(c: char) -> bool {
    matches!(c, '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}
fn query_words(q: &str) -> Vec<&str> {
    let mut rest = q;
    let mut out = Vec::new();
    while !rest.is_empty() {
        rest = rest.trim_start_matches(js_space);
        if rest.is_empty() {
            break;
        }
        // Original /"[^"]+"|\S+/g: a quoted match may end before adjacent text.
        let quoted_end = rest
            .strip_prefix('"')
            .and_then(|s| s.find('"'))
            .filter(|&n| n > 0)
            .map(|n| n + 2);
        let end = quoted_end.unwrap_or_else(|| rest.find(js_space).unwrap_or(rest.len()));
        let mut word = &rest[..end];
        if let Some(stripped) = word.strip_prefix('"') {
            word = stripped;
        }
        if let Some(stripped) = word.strip_suffix('"') {
            word = stripped;
        }
        out.push(word);
        rest = &rest[end..];
    }
    out
}
pub fn query(db: &Connection, q: &str, limit: usize) -> rusqlite::Result<Vec<FileEntry>> {
    query_cached(db, q, limit, &mut HashMap::new())
}
fn query_cached(
    db: &Connection,
    q: &str,
    limit: usize,
    cache: &mut HashMap<String, i64>,
) -> rusqlite::Result<Vec<FileEntry>> {
    let q = normalize(q.trim_matches(js_space));
    if q.is_empty() || q.encode_utf16().count() > 512 {
        return Ok(vec![]);
    }
    let max = limit.clamp(1, 100);
    let mut conditions = Vec::new();
    let mut values: Vec<String> = Vec::new();
    let mut candidates = Vec::new();
    for word in query_words(&q) {
        if word == "folder:" {
            conditions.push("is_dir=1".into());
            continue;
        }
        if word == "file:" {
            conditions.push("is_dir=0".into());
            continue;
        }
        if let Some(ext) = word.strip_prefix("ext:") {
            conditions.push("name LIKE ?".into());
            values.push(format!("%.{ext}"));
            continue;
        }
        let path_query = word.contains(['\\', '/']);
        let wildcard = word.contains(['*', '?']);
        let pattern = escape(word).replace('*', "%").replace('?', "_");
        conditions.push(format!(
            "{} LIKE ? ESCAPE '\\'",
            if path_query { "lower(path)" } else { "search" }
        ));
        values.push(if wildcard {
            pattern
        } else {
            format!("%{pattern}%")
        });
        if !path_query {
            for gram in grams(word) {
                if candidates.len() < 128 && !candidates.contains(&gram) {
                    candidates.push(gram);
                }
            }
        }
    }
    let mut frequencies = Vec::new();
    for gram in candidates {
        let count = if let Some(&count) = cache.get(&gram) {
            count
        } else {
            let count = db
                .prepare_cached("SELECT count(*) FROM tokens WHERE token=?")?
                .query_row([&gram], |row| row.get::<_, i64>(0))?;
            if cache.len() >= 256 {
                cache.clear();
            }
            cache.insert(gram.clone(), count);
            count
        };
        if count == 0 {
            return Ok(Vec::new());
        }
        frequencies.push((gram, count));
    }
    frequencies.sort_by_key(|(_, n)| *n);
    for (i, (gram, count)) in frequencies.into_iter().enumerate() {
        conditions.push(
            if i == 0 && count <= (max * 8) as i64 {
                "entries.rowid IN (SELECT entry_id FROM tokens WHERE token=?)"
            } else {
                // Frequent grams should scan insertion order and stop at max*4,
                // rather than materializing/sorting every matching path first.
                "EXISTS(SELECT 1 FROM tokens WHERE tokens.entry_id=entries.rowid AND token=?)"
            }
            .into(),
        );
        values.push(gram);
    }
    // Original first takes max*4 insertion-ordered candidates, then stable JS sort.
    let sql = format!(
        "SELECT path,name,is_dir,size,modified FROM entries WHERE {} ORDER BY entries.rowid LIMIT {}",
        if conditions.is_empty() {
            "1".into()
        } else {
            conditions.join(" AND ")
        },
        max * 4
    );
    let mut statement = db.prepare_cached(&sql)?;
    let mut rows = statement
        .query_map(params_from_iter(values), |r| {
            Ok(FileEntry {
                path: r.get(0)?,
                name: r.get(1)?,
                is_dir: r.get::<_, i32>(2)? != 0,
                size: r.get(3)?,
                modified: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.sort_by(|a, b| {
        a.name
            .encode_utf16()
            .count()
            .cmp(&b.name.encode_utf16().count())
            .then_with(|| crate::countries::locale_compare(&a.name, &b.name))
    });
    rows.truncate(max);
    Ok(rows)
}

fn write_entry(
    db: &Connection,
    entry: &walkdir::DirEntry,
    root: &Path,
    generation: i64,
) -> rusqlite::Result<()> {
    // WalkDir retains FindFirstFile metadata on Windows: no second per-file stat/open.
    let Ok(meta) = entry.metadata() else {
        return Ok(());
    };
    if meta.file_type().is_symlink() {
        return Ok(());
    }
    let path = entry.path().to_string_lossy();
    let name = entry.file_name().to_string_lossy();
    let search = normalize(&name);
    let modified = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_millis() as i64);
    let normalized_path = normalize(&path);
    let inserted=db.prepare_cached("INSERT INTO entries(path,name,search,root,generation,is_dir,size,modified,search_path) VALUES(?,?,?,?,?,?,?,?,?) ON CONFLICT(path) DO NOTHING")?
        .execute(params![path,name,search,root.to_string_lossy(),generation,meta.is_dir(),meta.len() as i64,modified,normalized_path])?;
    let entry_id;
    if inserted == 0 {
        let previous = db
            .prepare_cached("SELECT rowid,name,search FROM entries WHERE path=?")?
            .query_row([path.as_ref()], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .optional()?;
        db.prepare_cached("UPDATE entries SET name=?,search=?,root=?,generation=?,is_dir=?,size=?,modified=?,search_path=? WHERE path=?")?
            .execute(params![name,search,root.to_string_lossy(),generation,meta.is_dir(),meta.len() as i64,modified,normalized_path,path])?;
        // File contents/mtime changes do not change its filename's token set.
        if previous
            .as_ref()
            .is_some_and(|(_, old_name, old_search)| old_name == &*name && old_search == &search)
        {
            return Ok(());
        }
        entry_id = previous.ok_or(rusqlite::Error::QueryReturnedNoRows)?.0;
        db.prepare_cached("DELETE FROM tokens WHERE entry_id=?")?
            .execute([entry_id])?;
    } else {
        entry_id = db.last_insert_rowid();
    }
    let mut insert = db.prepare_cached("INSERT OR IGNORE INTO tokens VALUES(?,?)")?;
    for gram in grams(&search) {
        insert.execute(params![gram, entry_id])?;
    }
    Ok(())
}

struct Worker {
    db: Connection,
    status: Status,
    excludes: Vec<String>,
    rx: mpsc::Receiver<Request>,
    events: mpsc::Sender<Response>,
    wake: Box<dyn Fn() + Send>,
    generation: i64,
    stop: bool,
    watcher: Option<notify::RecommendedWatcher>,
    frequencies: HashMap<String, i64>,
}
impl Worker {
    fn notify(&mut self) {
        self.status.count = self
            .db
            .query_row("SELECT count(*) FROM entries", [], |r| r.get(0))
            .unwrap_or(0);
        let _ = self.events.send(Response::Status(self.status.clone()));
        (self.wake)();
    }
    fn scan(&mut self) {
        self.frequencies.clear();
        self.generation = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        self.status.building = true;
        self.status.denied = 0;
        self.notify();
        let generation = self.generation;
        for root in self.status.roots.clone() {
            let excludes = self.excludes.clone();
            let walker = walkdir::WalkDir::new(&root)
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| {
                    e.depth() == 0
                        || (!e.file_type().is_symlink()
                            && !excludes
                                .contains(&crate::js_case::lower(&e.file_name().to_string_lossy())))
                });
            let _ = self.db.execute_batch("BEGIN");
            let mut batch = 0;
            for entry in walker {
                if self.stop || self.status.paused || generation != self.generation {
                    break;
                }
                match entry {
                    Ok(entry) if entry.depth() > 0 => {
                        if let Err(e) = write_entry(&self.db, &entry, &root, generation) {
                            self.status.error = Some(e.to_string());
                        }
                        batch += 1;
                    }
                    Ok(_) => {}
                    Err(_) => self.status.denied += 1,
                }
                if batch >= 256 {
                    self.frequencies.clear();
                    let _ = self.db.execute_batch("COMMIT");
                    self.notify();
                    while let Ok(request) = self.rx.try_recv() {
                        self.handle(request);
                    }
                    let _ = self.db.execute_batch("BEGIN");
                    batch = 0;
                }
            }
            let _ = self.db.execute_batch("COMMIT");
            if !self.stop && !self.status.paused && generation == self.generation {
                let _ = self.db.execute(
                    "DELETE FROM entries WHERE root=? AND generation<>?",
                    params![root.to_string_lossy(), generation],
                );
            }
        }
        self.frequencies.clear();
        self.status.building = false;
        self.notify();
        if generation != self.generation && !self.stop && !self.status.paused {
            self.scan();
        }
    }
    fn handle(&mut self, request: Request) {
        match request {
            Request::SearchWithProvider {
                serial,
                query: q,
                limit,
                provider,
            } => match crate::file_providers::search(&provider, &q, limit) {
                Ok(None) => self.handle(Request::Search {
                    serial,
                    query: q,
                    limit,
                }),
                result => {
                    let items = match result {
                        Ok(Some(items)) => items
                            .into_iter()
                            .map(|item| FileEntry {
                                path: item.path,
                                name: item.name,
                                is_dir: item.is_dir,
                                size: item.size.unwrap_or(0),
                                modified: 0,
                            })
                            .collect(),
                        Err(error) => {
                            self.status.error = Some(error);
                            self.notify();
                            vec![]
                        }
                        _ => unreachable!(),
                    };
                    let _ = self.events.send(Response::Results { serial, items });
                    (self.wake)();
                }
            },
            Request::Search {
                serial,
                query: q,
                limit,
            } => {
                let items = match query_cached(&self.db, &q, limit, &mut self.frequencies) {
                    Ok(items) => items,
                    Err(e) => {
                        self.status.error = Some(e.to_string());
                        vec![]
                    }
                };
                let _ = self.events.send(Response::Results { serial, items });
                (self.wake)();
            }
            Request::Pause(value) => {
                self.status.paused = value;
                self.notify();
                if !value && !self.status.building {
                    self.scan();
                }
            }
            Request::Configure { roots, excludes } => {
                self.frequencies.clear();
                if let Some(watcher) = &mut self.watcher {
                    for root in &self.status.roots {
                        let _ = watcher.unwatch(root);
                    }
                    for root in &roots {
                        if let Err(e) = watcher.watch(root, notify::RecursiveMode::Recursive) {
                            self.status.error = Some(e.to_string());
                        }
                    }
                }
                for old in self.status.roots.iter().filter(|r| !roots.contains(r)) {
                    if let Err(e) = self
                        .db
                        .execute("DELETE FROM entries WHERE root=?", [old.to_string_lossy()])
                    {
                        self.status.error = Some(e.to_string());
                    }
                }
                self.status.roots = roots;
                self.excludes = excludes
                    .into_iter()
                    .map(|s| crate::js_case::lower(&s))
                    .collect();
                if !self.status.building {
                    self.scan();
                } else {
                    self.generation += 1;
                }
            }
            Request::Changed(path) => {
                if self.status.paused {
                    return;
                }
                self.frequencies.clear();
                if let Some(root) = self.status.roots.iter().find(|r| path.starts_with(r)) {
                    if path.components().any(|c| {
                        self.excludes
                            .contains(&crate::js_case::lower(&c.as_os_str().to_string_lossy()))
                    }) {
                        return;
                    }
                    if path.exists() {
                        let excludes = self.excludes.clone();
                        let _ = self.db.execute_batch("BEGIN");
                        for entry in walkdir::WalkDir::new(&path)
                            .follow_links(false)
                            .into_iter()
                            .filter_entry(|e| {
                                !e.file_type().is_symlink()
                                    && !excludes.contains(&crate::js_case::lower(
                                        &e.file_name().to_string_lossy(),
                                    ))
                            })
                            .filter_map(Result::ok)
                        {
                            if let Err(e) = write_entry(&self.db, &entry, root, self.generation) {
                                self.status.error = Some(e.to_string());
                            }
                        }
                        let _ = self.db.execute_batch("COMMIT");
                    } else {
                        let _ = self.db.execute(
                            "DELETE FROM entries WHERE path=? OR path LIKE ? ESCAPE '\\'",
                            params![
                                path.to_string_lossy(),
                                format!(
                                    "{}%",
                                    escape(&format!(
                                        "{}{}",
                                        path.to_string_lossy(),
                                        std::path::MAIN_SEPARATOR
                                    ))
                                )
                            ],
                        );
                    }
                }
                self.notify();
            }
            Request::Shutdown => self.stop = true,
        }
    }
}
pub fn result_item(entry: &FileEntry) -> ResultItem {
    ResultItem::new(&entry.name, &entry.path)
}

#[cfg(test)]
mod category_tests {
    #[test]
    fn all_codepoints_match_original_electron_letter_number_classes() {
        let source: serde_json::Value =
            serde_json::from_str(include_str!("../tests/index-categories-oracle.json")).unwrap();
        let ranges = source["ranges"].as_array().unwrap();
        let mut index = 0;
        let mut count = 0;
        for value in 0..=0x10ffffu32 {
            while index < ranges.len() && value as u64 > ranges[index][1].as_u64().unwrap() {
                index += 1;
            }
            let expected =
                index < ranges.len() && value as u64 >= ranges[index][0].as_u64().unwrap();
            let actual = char::from_u32(value).is_some_and(super::letter_or_number);
            assert_eq!(actual, expected, "U+{value:04X}");
            count += usize::from(actual);
        }
        assert_eq!(count as u64, source["count"].as_u64().unwrap());
    }
}
