//! Encoded image history must retain the original conversion bytes, not a PNG re-encode.
use base64::{Engine, engine::general_purpose::STANDARD};
use image::{DynamicImage, ImageFormat, RgbaImage};
use quicktool_rust::{
    pin_storage,
    storage::{EncodedImageSource, Store},
};
use std::{io::Cursor, path::PathBuf, sync::Arc};

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let mut random = [0u8; 8];
        getrandom::fill(&mut random).unwrap();
        let path = std::env::temp_dir().join(format!(
            "quicktool-image-source-test-{:x}",
            u64::from_le_bytes(random)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let path = self.0.canonicalize().unwrap();
        let base = std::env::temp_dir().canonicalize().unwrap();
        assert!(
            path.starts_with(base)
                && path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("quicktool-image-source-test-")
        );
        std::fs::remove_dir_all(path).unwrap();
    }
}
fn encoded(format: ImageFormat) -> (EncodedImageSource, Arc<RgbaImage>) {
    let image = RgbaImage::from_fn(17, 9, |x, y| {
        image::Rgba([(x * 13) as u8, (y * 23) as u8, 79, 255])
    });
    let mut output = Cursor::new(Vec::new());
    if format == ImageFormat::Gif {
        let mut encoder = image::codecs::gif::GifEncoder::new(&mut output);
        encoder
            .encode_frame(image::Frame::new(image.clone()))
            .unwrap();
        let second = RgbaImage::from_pixel(17, 9, image::Rgba([33, 18, 255, 255]));
        encoder.encode_frame(image::Frame::new(second)).unwrap();
    } else if format == ImageFormat::Jpeg {
        DynamicImage::ImageRgb8(DynamicImage::ImageRgba8(image).into_rgb8())
            .write_to(&mut output, format)
            .unwrap();
    } else {
        DynamicImage::ImageRgba8(image)
            .write_to(&mut output, format)
            .unwrap();
    }
    let bytes = output.into_inner();
    let pixels = image::load_from_memory(&bytes).unwrap().into_rgba8();
    (
        EncodedImageSource {
            bytes: bytes.into(),
            mime: format.to_mime_type().into(),
        },
        Arc::new(pixels),
    )
}
fn queue(store: &mut Store, pixels: Arc<RgbaImage>, source: Option<EncodedImageSource>) -> String {
    store
        .enqueue_image_with_source(
            "encoded input",
            "base64",
            "decode image",
            pixels,
            None,
            source,
            || {},
        )
        .unwrap()
}

#[test]
fn jpeg_webp_animated_gif_roundtrip_preserves_encoded_bytes_and_deduplicates() {
    for format in [ImageFormat::Jpeg, ImageFormat::WebP, ImageFormat::Gif] {
        let root = Root::new();
        let mut store = Store::open(root.0.clone());
        let (source, pixels) = encoded(format);
        let first = queue(&mut store, pixels.clone(), Some(source.clone()));
        let second = queue(&mut store, pixels.clone(), Some(source.clone()));
        assert!(store.flush_pin_updates().is_empty());
        assert_eq!(store.load_history_image(&first).unwrap(), *pixels);
        for id in [&first, &second] {
            let restored = store.load_history_image_source(id).unwrap().unwrap();
            assert_eq!(restored.mime, source.mime);
            assert_eq!(restored.bytes, source.bytes);
            assert_eq!(
                STANDARD.encode(&restored.bytes),
                STANDARD.encode(&source.bytes)
            );
        }
        assert_eq!(
            std::fs::read_dir(root.0.join("image-sources"))
                .unwrap()
                .count(),
            1
        );
        let saved = std::fs::read_to_string(root.0.join("history.json")).unwrap();
        assert!(!saved.contains(&STANDARD.encode(&source.bytes)));
        drop(store);
        let reopened = Store::open(root.0.clone());
        assert_eq!(
            reopened
                .load_history_image_source(&first)
                .unwrap()
                .unwrap()
                .bytes,
            source.bytes
        );
    }
}

#[test]
fn old_image_api_and_old_serialized_records_keep_png_fallback() {
    let root = Root::new();
    let mut store = Store::open(root.0.clone());
    let (_, pixels) = encoded(ImageFormat::Jpeg);
    let id = store
        .enqueue_image_with_mode("old", "base64", "old", pixels, None, || {})
        .unwrap();
    assert!(store.flush_pin_updates().is_empty());
    assert!(store.load_history_image_source(&id).unwrap().is_none());
    let value = serde_json::to_value(&store.history[0]).unwrap();
    assert!(value.get("image_source_asset").is_none());
    assert!(value.get("image_source_mime").is_none());
    drop(store);
    assert!(
        Store::open(root.0.clone())
            .load_history_image_source(&id)
            .unwrap()
            .is_none()
    );
}

#[test]
fn malformed_bytes_mime_and_asset_paths_never_publish_history() {
    let root = Root::new();
    let mut store = Store::open(root.0.clone());
    store.add("keep", "text", "keep", "unchanged").unwrap();
    let saved = std::fs::read(root.0.join("history.json")).unwrap();
    let (valid, pixels) = encoded(ImageFormat::Jpeg);
    for source in [
        EncodedImageSource {
            bytes: Arc::from(&b"not a JPEG"[..]),
            mime: "image/jpeg".into(),
        },
        EncodedImageSource {
            bytes: valid.bytes.clone(),
            mime: "text/html".into(),
        },
        EncodedImageSource {
            bytes: valid.bytes.clone(),
            mime: "image/jpeg\r\nX:evil".into(),
        },
    ] {
        queue(&mut store, pixels.clone(), Some(source));
        assert_eq!(store.flush_pin_updates().len(), 1);
        assert_eq!(store.history.len(), 1);
        assert_eq!(std::fs::read(root.0.join("history.json")).unwrap(), saved);
    }
    for asset in [
        "source:../outside",
        "asset:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "source:ABC",
        "source:",
    ] {
        assert!(pin_storage::encoded_source_path(&root.0, asset).is_err());
        assert!(pin_storage::load_encoded_source(&root.0, asset, "image/jpeg").is_err());
    }
    assert!(!root.0.join("image-sources").exists());
}

#[test]
fn corrupted_source_is_reported_and_retained_without_history_overwrite() {
    let root = Root::new();
    let mut store = Store::open(root.0.clone());
    let (source, pixels) = encoded(ImageFormat::Jpeg);
    let id = queue(&mut store, pixels.clone(), Some(source.clone()));
    assert!(store.flush_pin_updates().is_empty());
    let item = store.history.iter().find(|item| item.id == id).unwrap();
    let path =
        pin_storage::encoded_source_path(&root.0, item.image_source_asset.as_deref().unwrap())
            .unwrap();
    std::fs::write(&path, b"damaged original").unwrap();
    let saved = std::fs::read(root.0.join("history.json")).unwrap();
    assert!(store.load_history_image_source(&id).is_err());
    queue(&mut store, pixels, Some(source));
    assert_eq!(store.flush_pin_updates().len(), 1);
    assert_eq!(store.history.len(), 1);
    assert_eq!(std::fs::read(root.0.join("history.json")).unwrap(), saved);
    assert_eq!(std::fs::read(&path).unwrap(), b"damaged original");
    std::fs::remove_file(path).unwrap();
    assert!(store.load_history_image_source(&id).is_err());
}

#[test]
fn history_write_failure_rolls_back_memory_with_encoded_source() {
    let root = Root::new();
    let mut store = Store::open(root.0.clone());
    store.add("keep", "text", "keep", "original").unwrap();
    std::fs::remove_file(root.0.join("history.json")).unwrap();
    std::fs::create_dir(root.0.join("history.json")).unwrap();
    let (source, pixels) = encoded(ImageFormat::Jpeg);
    queue(&mut store, pixels, Some(source));
    assert_eq!(store.flush_pin_updates().len(), 1);
    assert_eq!(store.history.len(), 1);
    assert_eq!(store.history[0].value, "original");
    assert!(root.0.join("history.json").is_dir());
}

#[test]
fn legacy_jpeg_data_url_import_retains_original_source_bytes() {
    let root = Root::new();
    let source_root = root.0.join("legacy");
    let dest = root.0.join("new");
    std::fs::create_dir(&source_root).unwrap();
    let (source, _) = encoded(ImageFormat::Jpeg);
    let url = format!(
        "data:{};base64,{}",
        source.mime,
        STANDARD.encode(&source.bytes)
    );
    let rows = serde_json::json!([{"id":"jpeg-original","toolId":"base64","toolName":"旧版图片转换","outputType":"image","outputData":url,"createdAt":123}]);
    std::fs::write(
        source_root.join("history.json"),
        serde_json::to_vec(&rows).unwrap(),
    )
    .unwrap();
    let report = quicktool_rust::import_legacy::import_from(&source_root, &dest).unwrap();
    assert_eq!(report.imported, 1);
    assert_eq!(report.rejected, 0);
    let imported = Store::open(dest);
    assert_eq!(
        imported.history[0].tool_name.as_deref(),
        Some("旧版图片转换")
    );
    let restored = imported
        .load_history_image_source("jpeg-original")
        .unwrap()
        .unwrap();
    assert_eq!(restored.mime, "image/jpeg");
    assert_eq!(restored.bytes, source.bytes);
}
