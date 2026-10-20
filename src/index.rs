use crate::core::ResultItem;
use notify::Watcher;
use rusqlite::{Connection, params, params_from_iter};
use serde::{Deserialize, Serialize};
use std::{
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
                    excludes: excludes.into_iter().map(|s| s.to_lowercase()).collect(),
                    rx,
                    events,
                    wake: Box::new(wake),
                    generation: 0,
                    stop: false,
                    watcher: None,
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
    db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA cache_size=-8192; CREATE TABLE IF NOT EXISTS entries(path TEXT PRIMARY KEY,name TEXT,search TEXT,root TEXT,generation INTEGER,is_dir INTEGER,size INTEGER,modified INTEGER); CREATE INDEX IF NOT EXISTS entry_roots ON entries(root,generation); CREATE TABLE IF NOT EXISTS tokens(token TEXT,path TEXT,PRIMARY KEY(token,path)) WITHOUT ROWID; CREATE INDEX IF NOT EXISTS token_paths ON tokens(path); CREATE TRIGGER IF NOT EXISTS cleanup AFTER DELETE ON entries BEGIN DELETE FROM tokens WHERE path=old.path;END;")?;
    let has_path_search = db
        .prepare("PRAGMA table_info(entries)")?
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .iter()
        .any(|name| name == "search_path");
    if !has_path_search {
        db.execute_batch("BEGIN; ALTER TABLE entries ADD COLUMN search_path TEXT;")?;
        let paths = db
            .prepare("SELECT path FROM entries")?
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for path in paths {
            db.execute(
                "UPDATE entries SET search_path=? WHERE path=?",
                params![normalize(&path), path],
            )?;
        }
        db.execute_batch("COMMIT")?;
    }
    Ok(db)
}
fn normalize(s: &str) -> String {
    s.nfkc().flat_map(char::to_lowercase).collect()
}
fn grams(s: &str) -> Vec<String> {
    let chars: Vec<_> = s.chars().collect();
    let mut grams: Vec<String> = chars
        .windows(2)
        .filter(|p| p.iter().all(|c| c.is_alphanumeric()))
        .map(|p| p.iter().collect())
        .collect();
    grams.sort();
    grams.dedup();
    grams
}
fn escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}
pub fn query(db: &Connection, q: &str, limit: usize) -> rusqlite::Result<Vec<FileEntry>> {
    let q = normalize(q.trim());
    if q.is_empty() || q.len() > 2048 {
        return Ok(vec![]);
    }
    let mut conditions = Vec::new();
    let mut values: Vec<String> = Vec::new();
    let mut candidates = Vec::new();
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut quoted = false;
    for c in q.chars() {
        if c == '"' {
            quoted = !quoted;
        } else if c.is_whitespace() && !quoted {
            if !token.is_empty() {
                tokens.push(std::mem::take(&mut token));
            }
        } else {
            token.push(c);
        }
    }
    if !token.is_empty() {
        tokens.push(token);
    }
    for word in tokens {
        if word == "folder:" {
            conditions.push("is_dir=1".into());
            continue;
        }
        if word == "file:" {
            conditions.push("is_dir=0".into());
            continue;
        }
        if let Some(ext) = word.strip_prefix("ext:") {
            conditions.push("search LIKE ? ESCAPE '\\'".into());
            values.push(format!("%.{}", escape(ext)));
            continue;
        }
        let path_query = word.contains(['\\', '/']);
        let wildcard = word.contains(['*', '?']);
        let pattern = escape(&word).replace('*', "%").replace('?', "_");
        conditions.push(format!(
            "{} LIKE ? ESCAPE '\\'",
            if path_query { "search_path" } else { "search" }
        ));
        values.push(if wildcard {
            pattern
        } else {
            format!("%{pattern}%")
        });
        if !path_query && !wildcard {
            candidates.extend(grams(&word));
        }
    }
    let mut smallest = None;
    for gram in candidates {
        let count: i64 =
            db.query_row("SELECT count(*) FROM tokens WHERE token=?", [&gram], |r| {
                r.get(0)
            })?;
        if smallest.as_ref().is_none_or(|(_, n)| count < *n) {
            smallest = Some((gram, count));
        }
    }
    if let Some((gram, _)) = smallest {
        conditions.push("path IN (SELECT path FROM tokens WHERE token=?)".into());
        values.push(gram);
    }
    let sql = format!(
        "SELECT path,name,is_dir,size,modified FROM entries WHERE {} ORDER BY length(name),search LIMIT {}",
        conditions.join(" AND "),
        limit.clamp(1, 200)
    );
    let mut statement = db.prepare(&sql)?;
    statement
        .query_map(params_from_iter(values), |r| {
            Ok(FileEntry {
                path: r.get(0)?,
                name: r.get(1)?,
                is_dir: r.get::<_, i32>(2)? != 0,
                size: r.get(3)?,
                modified: r.get(4)?,
            })
        })?
        .collect()
}
fn write_entry(db: &Connection, path: &Path, root: &Path, generation: i64) -> rusqlite::Result<()> {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return Ok(());
    };
    if meta.file_type().is_symlink() {
        return Ok(());
    }
    let path = path.to_string_lossy();
    let name = Path::new(path.as_ref())
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let search = normalize(&name);
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as i64);
    db.execute("INSERT INTO entries(path,name,search,root,generation,is_dir,size,modified,search_path) VALUES(?,?,?,?,?,?,?,?,?) ON CONFLICT(path) DO UPDATE SET name=excluded.name,search=excluded.search,root=excluded.root,generation=excluded.generation,is_dir=excluded.is_dir,size=excluded.size,modified=excluded.modified,search_path=excluded.search_path",params![path,name,search,root.to_string_lossy(),generation,meta.is_dir(),meta.len()as i64,modified,normalize(&path)])?;
    db.execute("DELETE FROM tokens WHERE path=?", [path.as_ref()])?;
    let mut insert = db.prepare_cached("INSERT OR IGNORE INTO tokens VALUES(?,?)")?;
    for gram in grams(&search) {
        insert.execute(params![gram, path])?;
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
                            && !excludes.contains(&e.file_name().to_string_lossy().to_lowercase()))
                });
            let _ = self.db.execute_batch("BEGIN");
            let mut batch = 0;
            for entry in walker {
                if self.stop || self.status.paused || generation != self.generation {
                    break;
                }
                match entry {
                    Ok(entry) if entry.depth() > 0 => {
                        if let Err(e) = write_entry(&self.db, entry.path(), &root, generation) {
                            self.status.error = Some(e.to_string());
                        }
                        batch += 1;
                    }
                    Ok(_) => {}
                    Err(_) => self.status.denied += 1,
                }
                if batch >= 256 {
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
                let items = match query(&self.db, &q, limit) {
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
                self.excludes = excludes.into_iter().map(|s| s.to_lowercase()).collect();
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
                if let Some(root) = self.status.roots.iter().find(|r| path.starts_with(r)) {
                    if path.components().any(|c| {
                        self.excludes
                            .contains(&c.as_os_str().to_string_lossy().to_lowercase())
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
                                    && !excludes
                                        .contains(&e.file_name().to_string_lossy().to_lowercase())
                            })
                            .filter_map(Result::ok)
                        {
                            if let Err(e) =
                                write_entry(&self.db, entry.path(), root, self.generation)
                            {
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
