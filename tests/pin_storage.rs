//! Integration checks for the shared pin history and content-addressed originals.
use quicktool_rust::{
    pin_storage,
    pins::{PinHistoryUpdate, PinManager},
    storage::Store,
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

struct TestRoot(PathBuf);
impl TestRoot {
    fn new() -> Self {
        let mut id = [0u8; 16];
        getrandom::fill(&mut id).unwrap();
        let path = std::env::temp_dir().join(format!(
            "quicktool-pin-storage-test-{:032x}",
            u128::from_le_bytes(id)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for TestRoot {
    fn drop(&mut self) {
        if let (Ok(target), Ok(temp)) = (self.0.canonicalize(), std::env::temp_dir().canonicalize())
        {
            if target.starts_with(&temp)
                && target.file_name().is_some_and(|name| {
                    name.to_string_lossy()
                        .starts_with("quicktool-pin-storage-test-")
                })
            {
                let _ = std::fs::remove_dir_all(target);
            }
        }
    }
}
fn image_update() -> PinHistoryUpdate {
    let mut pins = PinManager::new();
    let pixels = image::RgbaImage::from_fn(17, 11, |x, y| {
        image::Rgba([
            (x * 13) as u8,
            (y * 21) as u8,
            99,
            if x % 3 == 0 { 120 } else { 255 },
        ])
    });
    pins.create_image(17, 11, pixels.into_raw()).unwrap();
    pins.flush_history();
    pins.drain_history_updates().pop().unwrap()
}
fn text_update(text: &str) -> PinHistoryUpdate {
    let mut pins = PinManager::new();
    pins.create_text(text.into());
    pins.flush_history();
    pins.drain_history_updates().pop().unwrap()
}

#[test]
fn shared_history_roundtrip_restores_original_and_pin_style() {
    let root = TestRoot::new();
    let mut store = Store::open(root.0.clone());
    let mut update = image_update();
    let expected = update.source.as_ref().unwrap().as_ref().clone();
    update.record.filters.brightness = 23.;
    update.record.corner_radius = 12.;
    update.record.blur = 9.;
    update.record.opacity = 0.42;
    update.record.position = Some([-120., 80.]);
    let mut mark = quicktool_rust::annotations::Annotation::new(
        8,
        quicktool_rust::annotations::Tool::Arrow,
        eframe::egui::pos2(0.2, 0.3),
        300.,
    );
    mark.x2 = 0.8;
    mark.y2 = 0.7;
    update.record.annotations.push(mark);
    let id = update.record.history_id.clone();
    store.upsert_pin(&update).unwrap();
    assert_eq!(store.history.len(), 1);
    assert_eq!(store.history[0].tool, "pin");
    let asset = store.history[0]
        .pin_state
        .as_ref()
        .unwrap()
        .asset
        .clone()
        .unwrap();
    assert!(asset.starts_with("asset:"));
    assert!(
        pin_storage::asset_path(&root.0, &asset, false)
            .unwrap()
            .is_file()
    );
    assert!(store.thumbnail_path(&id).unwrap().is_file());
    drop(store);
    let store = Store::open(root.0.clone());
    assert!(store.load_errors.is_empty());
    let (record, source) = store.load_pin(&id).unwrap();
    assert_eq!(source.unwrap(), expected);
    assert_eq!(record.filters.brightness, 23.);
    assert_eq!(record.corner_radius, 12.);
    assert_eq!(record.annotations[0].id, 8);
    assert_eq!(record.position, Some([-120., 80.]));
    assert_eq!(record.style_automatic, update.record.style_automatic);
    let history = std::fs::read_to_string(root.0.join("history.json")).unwrap();
    assert!(!history.contains("data:image"));
    assert!(history.contains(&asset));
}

#[test]
fn identical_originals_are_deduplicated_across_pins_and_edits() {
    let root = TestRoot::new();
    let mut store = Store::open(root.0.clone());
    let first = image_update();
    let first_id = first.record.history_id.clone();
    store.upsert_pin(&first).unwrap();
    let first_asset = store.load_pin(&first_id).unwrap().0.asset.unwrap();
    let second = image_update();
    assert_ne!(first.record.history_id, second.record.history_id);
    store.upsert_pin(&second).unwrap();
    let second_asset = store
        .load_pin(&second.record.history_id)
        .unwrap()
        .0
        .asset
        .unwrap();
    assert_eq!(first_asset, second_asset);
    let files = std::fs::read_dir(root.0.join("image-assets"))
        .unwrap()
        .count();
    assert_eq!(files, 2, "one original PNG and one thumbnail PNG");
    let mut edited = PinHistoryUpdate {
        record: first.record.clone(),
        source: None,
    };
    edited.record.filters.hue = 120.;
    edited
        .record
        .annotations
        .push(quicktool_rust::annotations::Annotation::new(
            1,
            quicktool_rust::annotations::Tool::Number,
            eframe::egui::pos2(0.5, 0.5),
            300.,
        ));
    store.upsert_pin(&edited).unwrap();
    assert_eq!(
        store.load_pin(&first_id).unwrap().0.asset.unwrap(),
        first_asset
    );
    assert_eq!(
        std::fs::read_dir(root.0.join("image-assets"))
            .unwrap()
            .count(),
        2
    );
    assert_eq!(store.history.len(), 2);
}

#[test]
fn async_save_flushes_in_order_and_restores_plaintext() {
    let root = TestRoot::new();
    let mut store = Store::open(root.0.clone());
    let wake = Arc::new(AtomicUsize::new(0));
    let mut first = text_update("初始文字\nRust");
    let id = first.record.history_id.clone();
    first.record.font_size = 28.;
    let wake_copy = wake.clone();
    store
        .enqueue_pin(first, move || {
            wake_copy.fetch_add(1, Ordering::SeqCst);
        })
        .unwrap();
    let mut second = text_update("unused");
    second.record.history_id = id.clone();
    second.record.text = "最后一次编辑".into();
    second.record.original_text = "初始文字\nRust".into();
    let wake_copy = wake.clone();
    store
        .enqueue_pin(second, move || {
            wake_copy.fetch_add(1, Ordering::SeqCst);
        })
        .unwrap();
    assert!(store.flush_pin_updates().is_empty());
    assert_eq!(wake.load(Ordering::SeqCst), 2);
    assert_eq!(store.history.len(), 1);
    let (record, source) = store.load_pin(&id).unwrap();
    assert!(source.is_none());
    assert_eq!(record.text, "最后一次编辑");
    assert_eq!(record.original_text, "初始文字\nRust");
    drop(store);
    let store = Store::open(root.0.clone());
    assert_eq!(store.load_pin(&id).unwrap().0.text, "最后一次编辑");
}

#[test]
fn async_image_edits_reuse_the_prepared_original() {
    let root = TestRoot::new();
    let mut store = Store::open(root.0.clone());
    let first = image_update();
    let id = first.record.history_id.clone();
    let mut edited = PinHistoryUpdate {
        record: first.record.clone(),
        source: None,
    };
    edited.record.filters.contrast = 18.;
    store.enqueue_pin(first, || {}).unwrap();
    store.enqueue_pin(edited, || {}).unwrap();
    assert!(store.flush_pin_updates().is_empty());
    let (record, image) = store.load_pin(&id).unwrap();
    assert_eq!(record.filters.contrast, 18.);
    assert_eq!(image.unwrap().dimensions(), (17, 11));
    assert_eq!(
        std::fs::read_dir(root.0.join("image-assets"))
            .unwrap()
            .count(),
        2
    );
}

#[test]
fn invalid_asset_references_cannot_escape_asset_directory() {
    let root = TestRoot::new();
    for asset in [
        "../../secret.png",
        "asset:../secret",
        "asset:C:\\secret",
        "asset:",
        "asset:abc",
        "asset:fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffG",
        "asset:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff/../x",
    ] {
        assert!(
            pin_storage::asset_path(&root.0, asset, false).is_err(),
            "{asset}"
        );
        assert!(pin_storage::load_image(&root.0, asset).is_err());
    }
    let good = format!("asset:{}", "a".repeat(64));
    let path = pin_storage::asset_path(&root.0, &good, false).unwrap();
    assert_eq!(path.parent().unwrap(), root.0.join("image-assets"));
    assert!(pin_storage::load_image(&root.0, &good).is_err());
}

#[test]
fn corrupted_or_missing_original_is_reported_without_overwrite() {
    let root = TestRoot::new();
    let mut store = Store::open(root.0.clone());
    let update = image_update();
    let id = update.record.history_id.clone();
    store.upsert_pin(&update).unwrap();
    let asset = store.load_pin(&id).unwrap().0.asset.unwrap();
    let path = pin_storage::asset_path(&root.0, &asset, false).unwrap();
    let broken = b"not a PNG";
    std::fs::write(&path, broken).unwrap();
    assert!(store.load_pin(&id).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), broken);
    assert!(Store::open(root.0.clone()).load_pin(&id).is_err());
    std::fs::remove_file(&path).unwrap();
    assert!(store.load_pin(&id).is_err());
    let edit = PinHistoryUpdate {
        record: update.record,
        source: None,
    };
    assert!(store.upsert_pin(&edit).is_err());
    assert!(!path.exists());
}

#[test]
fn an_async_asset_error_does_not_replace_existing_history() {
    let root = TestRoot::new();
    let mut store = Store::open(root.0.clone());
    let update = text_update("保留这条记录");
    let id = update.record.history_id.clone();
    store.upsert_pin(&update).unwrap();
    let mut invalid = PinHistoryUpdate {
        record: update.record,
        source: None,
    };
    invalid.record.kind = "image".into();
    invalid.record.asset = Some("asset:../invalid".into());
    store.enqueue_pin(invalid, || {}).unwrap();
    let errors = store.flush_pin_updates();
    assert_eq!(errors.len(), 1);
    assert_eq!(store.load_pin(&id).unwrap().0.text, "保留这条记录");
    assert_eq!(store.history.len(), 1);
}
