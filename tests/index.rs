use quicktool_rust::index::{FileEntry, Index, Request, Response};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
fn search(index: &Index, q: &str, serial: u64) -> Vec<FileEntry> {
    index
        .requests
        .send(Request::Search {
            serial,
            query: q.into(),
            limit: 40,
        })
        .unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(Instant::now() < until, "query timed out");
        match index
            .responses
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
        {
            Response::Results { serial: s, items } if s == serial => return items,
            Response::Status(s) => assert!(s.error.is_none(), "Index error: {:?}", s.error),
            _ => {}
        }
    }
}
fn root() -> PathBuf {
    let mut r = [0u8; 8];
    getrandom::fill(&mut r).unwrap();
    std::env::temp_dir().join(format!("quicktool-rust-index-{:x}", u64::from_le_bytes(r)))
}
fn cleanup(root: &Path) {
    assert!(
        root.is_absolute()
            && root
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("quicktool-rust-index-")
            && root.parent() == Some(std::env::temp_dir().as_path())
    );
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn persistent_index_filters_watches_and_reconfigures() {
    let root = root();
    let files = root.join("files");
    let other = root.join("other");
    std::fs::create_dir_all(files.join("folder")).unwrap();
    std::fs::create_dir_all(files.join("excluded")).unwrap();
    std::fs::create_dir_all(&other).unwrap();
    for (name, text) in [
        ("hello world.txt", "a"),
        ("中文文件.txt", "中"),
        ("ПРИВЕТ.txt", "unicode path"),
        ("100%_literal.txt", "b"),
        ("folder/nested.md", "c"),
        ("excluded/hidden.txt", "d"),
    ] {
        std::fs::write(files.join(name), text).unwrap();
    }
    std::fs::write(other.join("other.txt"), "x").unwrap();
    let index = Index::start(
        root.join("index.db"),
        vec![files.clone()],
        vec!["excluded".into()],
        || {},
    );
    assert_eq!(search(&index, "\"hello world\" ext:txt file:", 1).len(), 1);
    assert_eq!(search(&index, "中文", 2).len(), 1);
    assert_eq!(search(&index, "100%_", 3).len(), 1);
    assert_eq!(search(&index, "folder:", 4).len(), 1);
    assert_eq!(search(&index, "*.md", 5).len(), 1);
    assert!(search(&index, "hidden", 6).is_empty());
    assert_eq!(
        search(&index, &files.join("привет.txt").to_string_lossy(), 60).len(),
        0
    );
    std::fs::write(files.join("created.txt"), "created").unwrap();
    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        if !search(&index, "created", 7).is_empty() {
            break;
        }
        assert!(Instant::now() < deadline, "create notification missing");
        std::thread::sleep(Duration::from_millis(30));
    }
    std::fs::rename(files.join("created.txt"), files.join("renamed.txt")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        if !search(&index, "renamed", 8).is_empty() && search(&index, "created", 9).is_empty() {
            break;
        }
        assert!(Instant::now() < deadline, "rename notification missing");
        std::thread::sleep(Duration::from_millis(30));
    }
    index
        .requests
        .send(Request::Configure {
            roots: vec![other.clone()],
            excludes: vec![],
        })
        .unwrap();
    assert_eq!(search(&index, "other", 10).len(), 1);
    assert!(search(&index, "hello", 11).is_empty());
    drop(index);
    let db = quicktool_rust::index::open(&root.join("index.db")).unwrap();
    assert_eq!(
        quicktool_rust::index::query(&db, "other", 40)
            .unwrap()
            .len(),
        1
    );
    drop(db);
    cleanup(&root);
}
#[test]
fn older_database_preserves_entries_and_original_ascii_path_matching() {
    let root = root();
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("index.db");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("CREATE TABLE entries(path TEXT PRIMARY KEY,name TEXT,search TEXT,root TEXT,generation INTEGER,is_dir INTEGER,size INTEGER,modified INTEGER)").unwrap();
    db.execute(
        "INSERT INTO entries VALUES(?,'ПРИВЕТ.txt','привет.txt','C:\\',1,0,12,0)",
        ["C:\\ПАПКА\\ПРИВЕТ.txt"],
    )
    .unwrap();
    drop(db);
    let upgraded = quicktool_rust::index::open(&path).unwrap();
    let result = quicktool_rust::index::query(&upgraded, "c:\\папка\\привет.txt", 10).unwrap();
    assert!(result.is_empty());
    let by_name = quicktool_rust::index::query(&upgraded, "\"\"", 10).unwrap();
    assert_eq!(by_name.len(), 1);
    assert_eq!(by_name[0].name, "ПРИВЕТ.txt");
    drop(upgraded);
    cleanup(&root);
}
#[test]
fn legacy_path_tokens_migrate_atomically_and_keep_search_and_cleanup() {
    let root = root();
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("legacy.db");
    let old = rusqlite::Connection::open(&path).unwrap();
    old.execute_batch("CREATE TABLE entries(path TEXT PRIMARY KEY,name TEXT,search TEXT,root TEXT,generation INTEGER,is_dir INTEGER,size INTEGER,modified INTEGER);
        CREATE TABLE tokens(token TEXT,path TEXT,PRIMARY KEY(token,path)) WITHOUT ROWID;
        CREATE INDEX token_paths ON tokens(path);
        CREATE TRIGGER cleanup AFTER DELETE ON entries BEGIN DELETE FROM tokens WHERE path=old.path;END;
        INSERT INTO entries(rowid,path,name,search,root,generation,is_dir,size,modified) VALUES(9,'C:\\synthetic\\alpha.txt','alpha.txt','alpha.txt','C:\\synthetic',1,0,1,0);
        INSERT INTO entries(rowid,path,name,search,root,generation,is_dir,size,modified) VALUES(3,'C:\\synthetic\\beta.txt','beta.txt','beta.txt','C:\\synthetic',1,0,1,0);
        INSERT INTO tokens VALUES('al','C:\\synthetic\\alpha.txt'),('lp','C:\\synthetic\\alpha.txt'),('ph','C:\\synthetic\\alpha.txt'),('ha','C:\\synthetic\\alpha.txt');").unwrap();
    drop(old);
    let migrated = quicktool_rust::index::open(&path).unwrap();
    let columns = migrated
        .prepare("PRAGMA table_info(tokens)")
        .unwrap()
        .query_map([], |r| r.get::<_, String>(1))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(columns, ["token", "entry_id"]);
    assert_eq!(
        quicktool_rust::index::query(&migrated, "alpha", 40).unwrap()[0].name,
        "alpha.txt"
    );
    assert_eq!(
        migrated
            .query_row("SELECT entry_id FROM tokens WHERE token='al'", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        9
    );
    migrated
        .execute("DELETE FROM entries WHERE name='alpha.txt'", [])
        .unwrap();
    assert_eq!(
        migrated
            .query_row("SELECT count(*) FROM tokens", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    drop(migrated);
    let reopened = quicktool_rust::index::open(&path).unwrap();
    assert_eq!(
        reopened
            .query_row("SELECT count(*) FROM entries", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    drop(reopened);
    cleanup(&root);
}
