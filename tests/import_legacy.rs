use base64::{Engine, engine::general_purpose::STANDARD};
use image::ImageEncoder;
use quicktool_rust::{import_legacy::import_from, storage::Store};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let mut id = [0u8; 16];
        getrandom::fill(&mut id).unwrap();
        let root = std::env::temp_dir().join(format!(
            "quicktool-import-test-{:x}",
            u128::from_le_bytes(id)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("old")).unwrap();
        Self(root)
    }
    fn source(&self) -> PathBuf {
        self.0.join("old")
    }
    fn dest(&self) -> PathBuf {
        self.0.join("new")
    }
    fn rows(&self, rows: Value) {
        fs::write(
            self.source().join("history.json"),
            serde_json::to_vec(&rows).unwrap(),
        )
        .unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let p = self.0.canonicalize().unwrap();
        let root = std::env::temp_dir().canonicalize().unwrap();
        assert!(
            p.starts_with(root)
                && p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("quicktool-import-test-")
        );
        fs::remove_dir_all(p).unwrap();
    }
}
fn data() -> (String, image::RgbaImage) {
    let image = image::RgbaImage::from_fn(13, 7, |x, y| {
        image::Rgba([x as u8 * 10, y as u8 * 25, 99, 127])
    });
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(image.as_raw(), 13, 7, image::ExtendedColorType::Rgba8)
        .unwrap();
    (
        format!("data:image/png;base64,{}", STANDARD.encode(png)),
        image,
    )
}
#[test]
fn legacy_settings_text_image_and_pin_roundtrip_with_asset_dedup() {
    let f = Fixture::new();
    let (url, expected) = data();
    let hash = format!("{:x}", Sha256::digest(url.as_bytes()));
    fs::create_dir(f.source().join("image-assets")).unwrap();
    fs::write(
        f.source().join("image-assets").join(format!("{hash}.txt")),
        &url,
    )
    .unwrap();
    fs::write(
        f.source().join("settings.json"),
        br##"{"theme":"light","blurStrength":12,"glassOpacity":0.4,"historyLimit":1}"##,
    )
    .unwrap();
    f.rows(json!([
 {"id":"text","toolId":"base64","toolName":"Base64","action":"decode","input":"aGk=","output":"hi","createdAt":12,"pinned":true,"mode":{"toolId":"base64","args":[{"id":"decode","label":"decode"}]}},
 {"id":"image","toolId":"base64","outputType":"image","outputData":url,"createdAt":13},
 {"id":"pin","toolId":"pin","outputType":"image","pinState":{"type":"image","dataUrl":format!("asset:{hash}"),"width":430,"height":260,"cornerRadius":17,"filters":{"brightness":21},"annotations":[{"id":"uuid-1","type":"arrow","x":0.2,"y":0.3,"x2":0.7,"y2":0.8,"color":"#12ab34","stroke":0.02,"fontSize":0.1,"points":[]}],"background":{"color":"#123456","textColor":"#abcdef","blur":8,"opacity":0.8}},"createdAt":14},
 {"id":"textpin","toolId":"pin","pinState":{"type":"text","text":"edited","initialText":"original","fontSize":23,"background":{}}}
 ]));
    let original = fs::read(f.source().join("history.json")).unwrap();
    let report = import_from(f.source(), f.dest()).unwrap();
    assert_eq!(
        (report.imported, report.images, report.pins, report.rejected),
        (4, 1, 2, 0)
    );
    let store = Store::open(f.dest());
    assert!(store.load_errors.is_empty());
    assert_eq!(
        store.history.len(),
        4,
        "import must retain all rows regardless of new history limit"
    );
    assert_eq!(store.settings["blurStrength"], 12);
    assert_eq!(
        store.history[0].mode.as_ref().unwrap().submode.as_deref(),
        Some("decode")
    );
    assert!(store.history[0].pinned);
    assert_eq!(store.load_history_image("image").unwrap(), expected);
    let (pin, image) = store.load_pin("pin").unwrap();
    assert_eq!(image.unwrap(), expected);
    assert_eq!(pin.annotations[0].color, [18, 171, 52]);
    assert_eq!(pin.annotations[0].id, 1);
    assert_eq!(pin.background, [18, 52, 86]);
    assert_eq!(pin.foreground, Some([171, 205, 239]));
    assert_eq!(pin.filters.brightness, 21.);
    assert_eq!(pin.corner_radius, 17.);
    let (text, _) = store.load_pin("textpin").unwrap();
    assert_eq!(text.original_text, "original");
    assert_eq!(text.text, "edited");
    assert_eq!(text.background, [248, 250, 255]);
    assert_eq!(text.blur, 12.);
    assert_eq!(text.opacity, 0.4);
    assert_eq!(text.style_automatic, Some([true; 3]));
    assert_eq!(
        fs::read_dir(f.dest().join("image-assets")).unwrap().count(),
        2
    );
    assert_eq!(fs::read(f.source().join("history.json")).unwrap(), original);
    assert_eq!(
        fs::read(f.dest().join("import-source/history.json")).unwrap(),
        original
    );
}
#[test]
fn bad_rows_are_reported_with_original_values_without_poisoning_valid_rows() {
    let f = Fixture::new();
    let bad = json!({"id":"broken","toolId":"base64","outputType":"image","outputData":"asset:../../outside","unknown":{"preserve":"exact"}});
    f.rows(json!([{"id":"ok","toolId":"text","output":"kept"},bad]));
    let report = import_from(f.source(), f.dest()).unwrap();
    assert_eq!((report.imported, report.rejected), (1, 1));
    assert_eq!(report.errors[0].raw, bad);
    assert_eq!(report.errors[0].index, Some(1));
    let store = Store::open(f.dest());
    assert_eq!(store.history[0].value, "kept");
    let saved: Value =
        serde_json::from_slice(&fs::read(f.dest().join("import-report.json")).unwrap()).unwrap();
    assert_eq!(saved["errors"][0]["raw"], bad);
}
#[test]
fn existing_destination_is_never_changed_and_nested_source_destination_is_refused() {
    let f = Fixture::new();
    f.rows(json!([]));
    fs::create_dir(f.dest()).unwrap();
    fs::write(f.dest().join("history.json"), b"untouched").unwrap();
    assert!(import_from(f.source(), f.dest()).is_err());
    assert_eq!(
        fs::read(f.dest().join("history.json")).unwrap(),
        b"untouched"
    );
    assert!(import_from(f.source(), f.source().join("nested")).is_err());
    assert!(!f.source().join("nested").exists());
}
#[test]
fn corrupt_asset_hash_and_duplicate_ids_are_retained_in_report() {
    let f = Fixture::new();
    fs::create_dir(f.source().join("image-assets")).unwrap();
    let hash = "a".repeat(64);
    fs::write(
        f.source().join("image-assets").join(format!("{hash}.txt")),
        data().0,
    )
    .unwrap();
    f.rows(json!([{"id":"same","toolId":"text","output":"first"},{"id":"same","toolId":"text","output":"second"},{"id":"badasset","toolId":"base64","outputType":"image","outputData":format!("asset:{hash}")} ]));
    let report = import_from(f.source(), f.dest()).unwrap();
    assert_eq!((report.imported, report.rejected), (1, 2));
    assert!(report.errors.iter().any(|e| e.error.contains("校验")));
    assert_eq!(Store::open(f.dest()).history[0].value, "first");
}
#[test]
fn malformed_json_is_archived_verbatim() {
    let f = Fixture::new();
    fs::write(f.source().join("history.json"), b"[{broken").unwrap();
    fs::write(f.source().join("settings.json"), b"[]").unwrap();
    let report = import_from(f.source(), f.dest()).unwrap();
    assert_eq!(report.imported, 0);
    assert_eq!(report.errors.len(), 2);
    assert_eq!(
        fs::read(f.dest().join("import-source/history.json")).unwrap(),
        b"[{broken"
    );
    assert!(Store::open(f.dest()).history.is_empty());
}
#[test]
fn absent_legacy_data_does_not_publish_destination_or_leave_staging() {
    let f = Fixture::new();
    assert!(import_from(f.source(), f.dest()).is_err());
    assert!(!f.dest().exists());
    assert_eq!(fs::read_dir(&f.0).unwrap().count(), 1);
}
#[test]
fn svg_image_output_is_rasterized_and_restorable() {
    let f = Fixture::new();
    let svg=br##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="3"><rect width="4" height="3" fill="#ff0000"/></svg>"##;
    f.rows(json!([{"id":"svg","toolId":"base64","outputType":"image","outputData":format!("data:image/svg+xml;base64,{}",STANDARD.encode(svg))}]));
    let report = import_from(f.source(), f.dest()).unwrap();
    assert_eq!(report.imported, 1);
    let im = Store::open(f.dest()).load_history_image("svg").unwrap();
    assert_eq!(im.dimensions(), (4, 3));
    assert_eq!(im.get_pixel(0, 0).0, [255, 0, 0, 255]);
}

#[test]
fn svg_external_file_is_not_resolved_and_record_is_preserved() {
    let f = Fixture::new();
    let external = f.0.join("outside.png");
    let (url, _) = data();
    fs::write(
        &external,
        STANDARD.decode(url.split_once(',').unwrap().1).unwrap(),
    )
    .unwrap();
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"3\"><image href=\"{}\" width=\"4\" height=\"3\"/></svg>",
        external.to_string_lossy().replace('\\', "/")
    );
    let row = json!({"id":"external","toolId":"base64","outputType":"image","outputData":format!("data:image/svg+xml;base64,{}", STANDARD.encode(svg))});
    f.rows(json!([row]));
    let report = import_from(f.source(), f.dest()).unwrap();
    assert_eq!((report.imported, report.rejected), (0, 1));
    assert!(report.errors[0].error.contains("外部"));
    assert_eq!(report.errors[0].raw, row);
}
