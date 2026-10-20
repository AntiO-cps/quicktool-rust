use quicktool_rust::file_providers::*;
#[test]
fn local_requests_sqlite_fallback() {
    assert_eq!(search("local", "report", 40).unwrap(), None);
}
#[test]
fn empty_query_never_invokes_provider() {
    for provider in ["local", "auto", "everything", "windows"] {
        assert_eq!(search(provider, " \t\n", 40).unwrap(), Some(vec![]));
    }
}
#[test]
fn invalid_queries_are_rejected() {
    assert!(search("windows", "name\0suffix", 40).is_err());
    assert!(search("unknown", "name", 40).is_err());
}
#[test]
fn windows_sql_escapes_literals_and_clamps_limit() {
    let sql = windows_query_sql(" O'Brien%_ ", 1000);
    assert!(sql.starts_with("SELECT TOP 100 "));
    assert!(sql.contains("LIKE '%O''Brien[%][_]%'"));
    assert!(windows_query_sql("x", 0).starts_with("SELECT TOP 1 "));
}
#[test]
fn everything_rows_preserve_paths_and_provider() {
    let out = parse_everything_output(
        "\u{feff}C:\\Some Folder\\alpha.txt\r\n\r\nD:\\目录\\文件.png\r\n".as_bytes(),
        40,
    );
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].name, "alpha.txt");
    assert_eq!(out[0].path, "C:\\Some Folder\\alpha.txt");
    assert_eq!(out[1].name, "文件.png");
    assert!(out.iter().all(|r| r.provider == "Everything"));
    assert_eq!(parse_everything_output(b"a\nb\nc\n", 1).len(), 1);
}
#[test]
fn everything_marks_real_directories() {
    let path = std::env::temp_dir();
    let output = format!("{}\r\n", path.display());
    let rows = parse_everything_output(output.as_bytes(), 10);
    assert_eq!(rows.len(), 1);
    assert!(rows[0].is_dir);
}
#[test]
fn absent_everything_results_request_local_fallback() {
    for provider in ["auto", "everything"] {
        assert_eq!(
            search(
                provider,
                "__QuickTool_nonexistent_file_f54b3297a9a244c3__",
                4
            )
            .unwrap(),
            None
        );
    }
}
#[cfg(windows)]
#[test]
#[ignore = "Requires Windows Search service and Search.CollatorDSO provider"]
fn windows_search_native_read_only() {
    let rows = search(
        "windows",
        "__QuickTool_nonexistent_file_f54b3297a9a244c3__",
        4,
    )
    .unwrap()
    .expect("explicit Windows Search must return an authoritative result");
    assert!(rows.is_empty());
}
