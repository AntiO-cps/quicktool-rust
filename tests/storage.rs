use quicktool_rust::storage::{Store, atomic_write};
fn root() -> std::path::PathBuf {
    let mut r = [0u8; 8];
    getrandom::fill(&mut r).unwrap();
    std::env::temp_dir().join(format!(
        "quicktool-rust-storage-{:x}",
        u64::from_le_bytes(r)
    ))
}
fn cleanup(root: &std::path::Path) {
    assert!(
        root.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("quicktool-rust-storage-")
            && root.parent() == Some(std::env::temp_dir().as_path())
    );
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn atomic_replace_and_pinned_retention() {
    let root = root();
    let mut store = Store::open(root.clone());
    store.settings["historyLimit"] = serde_json::json!(1.0);
    store.save_settings().unwrap();
    store.add("first", "text", "first", "1").unwrap();
    store.history[0].pinned = true;
    store.save_history().unwrap();
    store.add("second", "text", "second", "2").unwrap();
    store.add("third", "text", "third", "3").unwrap();
    assert_eq!(store.history.len(), 2);
    assert_eq!(store.history[0].value, "3");
    assert_eq!(store.history[1].value, "1");
    assert!(store.history[1].pinned);
    let restored = Store::open(root.clone());
    assert_eq!(restored.history.len(), 2);
    assert!(restored.load_errors.is_empty());
    assert!(
        !std::fs::read_dir(&root).unwrap().any(|e| e
            .unwrap()
            .path()
            .extension()
            .is_some_and(|e| e == "tmp"))
    );
    drop(restored);
    cleanup(&root);
}
#[test]
fn corrupt_sources_are_preserved() {
    let root = root();
    std::fs::create_dir_all(&root).unwrap();
    atomic_write(&root.join("history.json"), b"broken history").unwrap();
    atomic_write(&root.join("settings.json"), b"[]").unwrap();
    let mut store = Store::open(root.clone());
    assert_eq!(store.load_errors.len(), 2);
    assert!(store.save_settings().is_err());
    assert!(store.add("test", "text", "test", "test").is_err());
    assert_eq!(
        std::fs::read(root.join("history.json")).unwrap(),
        b"broken history"
    );
    assert_eq!(std::fs::read(root.join("settings.json")).unwrap(), b"[]");
    cleanup(&root);
}
#[test]
fn usage_is_persisted_and_corrupt_usage_is_preserved() {
    let root = root();
    let mut store = Store::open(root.clone());
    assert_eq!(store.usage_boost("base"), 0.);
    store.mark_used("base").unwrap();
    assert_eq!(store.usage["base"].count, 1);
    assert!((store.usage_boost("base") - 0.11).abs() < 0.0001);
    let restored = Store::open(root.clone());
    assert_eq!(restored.usage["base"].count, 1);
    atomic_write(&root.join("usage.json"), b"invalid usage").unwrap();
    let mut damaged = Store::open(root.clone());
    assert_eq!(damaged.load_errors.len(), 1);
    assert!(damaged.mark_used("unit").is_err());
    assert_eq!(
        std::fs::read(root.join("usage.json")).unwrap(),
        b"invalid usage"
    );
    cleanup(&root);
}
#[test]
fn history_restores_mode_and_subcommand() {
    use quicktool_rust::storage::HistoryMode;
    let root = root();
    let mut store = Store::open(root.clone());
    store
        .add_with_mode(
            "hello",
            "base64",
            "Base64 编码",
            "aGVsbG8=",
            Some(HistoryMode {
                tool_id: "base64".into(),
                args: vec![serde_json::json!({"id":"encode","label":"编码"})],
                submode: Some("encode".into()),
            }),
        )
        .unwrap();
    let restored = Store::open(root.clone());
    let mode = restored.history[0].mode.as_ref().unwrap();
    assert_eq!(mode.tool_id, "base64");
    assert_eq!(mode.submode.as_deref(), Some("encode"));
    assert_eq!(mode.args[0]["label"], "编码");
    cleanup(&root);
}

#[test]
fn legacy_history_without_image_asset_remains_readable() {
    let root = root();
    std::fs::create_dir_all(&root).unwrap();
    atomic_write(&root.join("history.json"), br#"[{"id":"legacy","input":"hello","tool":"text","title":"legacy","value":"hello","pinned":false,"created":1}]"#).unwrap();
    let store = Store::open(root.clone());
    assert!(store.load_errors.is_empty());
    assert!(store.history[0].image_asset.is_none());
    assert!(store.load_history_image("legacy").is_err());
    assert_eq!(store.history[0].value, "hello");
    cleanup(&root);
}
