//! Explicit, read-only legacy import. A new destination is published only after staging succeeds.
use crate::{
    annotations::{Annotation, Tool},
    pins::PinRecord,
    pixels::Filters,
    storage::{HistoryItem, HistoryMode, Store, atomic_write},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use image::RgbaImage;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ImportReport {
    pub imported: usize,
    pub images: usize,
    pub pins: usize,
    pub rejected: usize,
    pub errors: Vec<ImportError>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ImportError {
    pub index: Option<usize>,
    pub error: String,
    pub raw: Value,
}
fn error(message: &str) -> io::Error {
    io::Error::other(message)
}
fn read_source(root: &Path, relative: &Path) -> io::Result<Vec<u8>> {
    let path = root.join(relative).canonicalize()?;
    if !path.starts_with(root) {
        return Err(error("旧数据引用超出源目录"));
    }
    let file = fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(192 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 192 * 1024 * 1024 {
        return Err(error("旧文件超过192MB上限"));
    }
    Ok(bytes)
}
/// The destination must not exist, including an empty directory. Neither source nor existing data is modified.
pub fn import_from(
    source_dir: impl AsRef<Path>,
    dest_dir: impl AsRef<Path>,
) -> io::Result<ImportReport> {
    let source = source_dir.as_ref().canonicalize()?;
    if !source.is_dir() {
        return Err(error("源路径不是目录"));
    }
    let destination = dest_dir.as_ref();
    if destination.exists() {
        return Err(error("导入目标已存在，禁止覆盖"));
    }
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()?;
    let name = destination
        .file_name()
        .ok_or_else(|| error("目标目录名无效"))?;
    let destination = parent.join(name);
    if destination.starts_with(&source) {
        return Err(error("导入目标不能位于旧数据目录内"));
    }
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(|e| io::Error::other(e.to_string()))?;
    let staging = parent.join(format!(
        ".quicktool-import-{:032x}",
        u128::from_le_bytes(random)
    ));
    fs::create_dir(&staging)?;
    let result = prepare(&source, &staging).and_then(|report| {
        if destination.exists() {
            return Err(error("导入期间目标已创建，停止发布"));
        }
        // On Windows rename refuses an existing directory; files never mix with another Store.
        fs::rename(&staging, &destination)?;
        Ok(report)
    });
    if result.is_err()
        && staging.parent() == Some(parent.as_path())
        && staging
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with(".quicktool-import-"))
    {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}
fn prepare(source: &Path, dest: &Path) -> io::Result<ImportReport> {
    let mut report = ImportReport::default();
    let mut store = Store::open(dest.into());
    let mut any = false;
    for name in ["settings.json", "history.json"] {
        let bytes = match read_source(source, Path::new(name)) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e),
        };
        any = true;
        atomic_write(&dest.join("import-source").join(name), &bytes)?;
        if name == "settings.json" {
            match serde_json::from_slice::<Value>(&bytes) {
                Ok(Value::Object(values)) => store.settings.as_object_mut().unwrap().extend(values),
                _ => report.errors.push(ImportError {
                    index: None,
                    error: "settings.json 无效，保留原文并使用默认设置".into(),
                    raw: Value::Null,
                }),
            }
        } else {
            match serde_json::from_slice::<Value>(&bytes) {
                Ok(Value::Array(rows)) => {
                    let mut ids = HashSet::new();
                    for (i, row) in rows.into_iter().enumerate() {
                        let converted =
                            convert(source, dest, &row, i, &store.settings).and_then(|h| {
                                if !ids.insert(h.id.clone()) {
                                    return Err(error("重复历史ID"));
                                }
                                Ok(h)
                            });
                        match converted {
                            Ok(h) => {
                                report.imported += 1;
                                report.images += usize::from(h.image_asset.is_some());
                                report.pins += usize::from(h.pin_state.is_some());
                                store.history.push(h);
                            }
                            Err(e) => {
                                report.rejected += 1;
                                report.errors.push(ImportError {
                                    index: Some(i),
                                    error: e.to_string(),
                                    raw: row,
                                });
                            }
                        }
                    }
                }
                _ => {
                    report.rejected += 1;
                    report.errors.push(ImportError {
                        index: None,
                        error: "history.json 不是有效数组，保留原文".into(),
                        raw: Value::Null,
                    });
                }
            }
        }
    }
    if !any {
        return Err(error("源目录没有settings.json或history.json"));
    }
    store.save_settings()?;
    store.save_history()?;
    atomic_write(
        &dest.join("import-report.json"),
        &serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(report)
}
fn text(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().into()
}
fn num(v: &Value, key: &str, default: f32) -> f32 {
    v[key]
        .as_f64()
        .map(|n| n as f32)
        .filter(|n| n.is_finite())
        .unwrap_or(default)
}
fn color(v: &Value) -> Option<[u8; 3]> {
    let s = v.as_str()?.strip_prefix('#')?;
    if s.len() != 6 || !s.is_ascii() {
        return None;
    }
    Some([
        u8::from_str_radix(&s[0..2], 16).ok()?,
        u8::from_str_radix(&s[2..4], 16).ok()?,
        u8::from_str_radix(&s[4..6], 16).ok()?,
    ])
}
fn convert(
    source: &Path,
    dest: &Path,
    row: &Value,
    index: usize,
    settings: &Value,
) -> io::Result<HistoryItem> {
    if !row.is_object() {
        return Err(error("历史记录不是对象"));
    }
    let mut id = text(row, "id");
    if id.is_empty() {
        id = format!("legacy-{index}");
    }
    let tool = text(row, "toolId");
    if tool.is_empty() {
        return Err(error("历史记录缺少toolId"));
    }
    let mode = row
        .get("mode")
        .filter(|v| !v.is_null())
        .map(|m| HistoryMode {
            tool_id: text(m, "toolId"),
            args: m["args"].as_array().cloned().unwrap_or_default(),
            submode: m["submode"].as_str().map(str::to_string).or_else(|| {
                m["args"].as_array()?.last()?["id"]
                    .as_str()
                    .map(str::to_string)
            }),
        });
    let created = row["createdAt"].as_u64().unwrap_or(0);
    let mut h = HistoryItem {
        id: id.clone(),
        input: text(row, "input"),
        tool,
        title: if row["action"].is_string() {
            text(row, "action")
        } else {
            text(row, "toolName")
        },
        value: text(row, "output"),
        pinned: row["pinned"].as_bool().unwrap_or(false),
        created,
        pin_state: None,
        thumbnail: None,
        mode,
        image_asset: None,
    };
    if let Some(ps) = row.get("pinState").filter(|v| !v.is_null()) {
        let kind = text(ps, "type");
        if kind != "text" && kind != "image" {
            return Err(error("Pin类型无效"));
        }
        let bg = &ps["background"];
        let asset = if kind == "image" {
            let raw = ps
                .get("dataUrl")
                .filter(|v| v.is_string())
                .unwrap_or(&row["outputData"]);
            Some(import_image(source, dest, raw)?)
        } else {
            None
        };
        let width = num(ps, "width", 430.).clamp(1., 2384.);
        let height = num(ps, "height", 260.).clamp(1., 1984.);
        let marks = ps["annotations"]
            .as_array()
            .map(|rows| {
                rows.iter()
                    .enumerate()
                    .map(|(i, a)| annotation(a, i, width))
                    .collect::<io::Result<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();
        if marks.len() > 200 || marks.iter().map(|a| a.points.len()).sum::<usize>() > 10000 {
            return Err(error("Pin标注超限"));
        }
        let default_bg = if settings["theme"] == "light" {
            [248, 250, 255]
        } else {
            [25, 28, 37]
        };
        h.pin_state = Some(PinRecord {
            history_id: id,
            kind: kind.clone(),
            text: text(ps, "text"),
            original_text: ps["initialText"]
                .as_str()
                .unwrap_or(ps["text"].as_str().unwrap_or_default())
                .into(),
            asset: asset.clone(),
            filters: serde_json::from_value::<Filters>(
                ps["filters"]
                    .as_object()
                    .map(|_| ps["filters"].clone())
                    .unwrap_or(serde_json::json!({})),
            )?
            .normalized(),
            annotations: marks,
            corner_radius: num(ps, "cornerRadius", if kind == "text" { 16. } else { 0. })
                .clamp(0., width.min(height) / 2.),
            font_size: num(ps, "fontSize", 15.).clamp(8., 96.),
            background: color(&bg["color"]).unwrap_or(default_bg),
            foreground: color(&bg["textColor"]),
            opacity: num(bg, "opacity", num(settings, "glassOpacity", 0.78)).clamp(0., 1.),
            blur: num(bg, "blur", num(settings, "blurStrength", 28.)).clamp(0., 48.),
            width,
            height,
            position: None,
            created,
            style_automatic: Some([
                color(&bg["color"]).is_none(),
                !bg["blur"].is_number(),
                !bg["opacity"].is_number(),
            ]),
        });
        h.thumbnail = asset;
    } else if row["outputType"] == "image" {
        let asset = import_image(source, dest, &row["outputData"])?;
        h.thumbnail = Some(asset.clone());
        h.image_asset = Some(asset);
    }
    Ok(h)
}
fn annotation(v: &Value, i: usize, width: f32) -> io::Result<Annotation> {
    let tool = match v["type"].as_str().unwrap_or_default() {
        "rect" => Tool::Rect,
        "ellipse" => Tool::Ellipse,
        "line" => Tool::Line,
        "arrow" => Tool::Arrow,
        "pen" => Tool::Pen,
        "text" => Tool::Text,
        "number" => Tool::Number,
        "mosaic" => Tool::Mosaic,
        "highlight" => Tool::Highlight,
        _ => return Err(error("未知标注类型")),
    };
    let mut a = Annotation::new(
        i as u64 + 1,
        tool,
        eframe::egui::pos2(num(v, "x", 0.), num(v, "y", 0.)),
        width,
    );
    a.x2 = num(v, "x2", a.x);
    a.y2 = num(v, "y2", a.y);
    a.color = color(&v["color"]).unwrap_or(a.color);
    a.text_color = color(&v["textColor"]);
    a.radius = num(v, "radius", 0.);
    a.opacity = num(v, "opacity", 0.55);
    a.stroke = num(v, "stroke", 0.008);
    a.font_size = num(v, "fontSize", 0.06);
    a.text = text(v, "text");
    a.number = v["number"].as_u64().unwrap_or(1).min(999) as u32;
    if let Some(points) = v["points"].as_array() {
        if points.len() > 2048 {
            return Err(error("标注点超限"));
        }
        a.points = points
            .iter()
            .map(|p| [num(p, "x", 0.), num(p, "y", 0.)])
            .collect();
    }
    a.normalize();
    Ok(a)
}
fn import_image(source: &Path, dest: &Path, value: &Value) -> io::Result<String> {
    let value = value.as_str().ok_or_else(|| error("图片资源为空"))?;
    let data = if let Some(hash) = value.strip_prefix("asset:") {
        crate::pin_storage::asset_path(source, value, false)?;
        let bytes = read_source(
            source,
            &PathBuf::from("image-assets").join(format!("{hash}.txt")),
        )?;
        if format!("{:x}", Sha256::digest(&bytes)) != hash {
            return Err(error("旧图片资产校验失败"));
        }
        String::from_utf8(bytes).map_err(io::Error::other)?
    } else {
        value.into()
    };
    if data.len() > 32 * 1024 * 1024 || !data.starts_with("data:image/") {
        return Err(error("旧图片DataURL无效或超过32MB"));
    }
    let (header, payload) = data
        .split_once(',')
        .ok_or_else(|| error("图片DataURL缺少内容"))?;
    if !header.ends_with(";base64") {
        return Err(error("旧图片DataURL必须使用Base64"));
    }
    let bytes = STANDARD.decode(payload).map_err(io::Error::other)?;
    if header.split(';').next() == Some("data:image/svg+xml") {
        let mut options = crate::annotations::svg_options();
        let external = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let detected = external.clone();
        options.image_href_resolver.resolve_string = Box::new(move |_, _| {
            detected.store(true, std::sync::atomic::Ordering::Relaxed);
            None
        });
        let tree = resvg::usvg::Tree::from_data(&bytes, &options).map_err(io::Error::other)?;
        if external.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(error("SVG含外部资源，未读取外部文件；记录已保留"));
        }

        let size = tree.size().to_int_size();
        if size.width() as u64 * size.height() as u64 > 16_000_000 {
            return Err(error("SVG超过1600万像素"));
        }
        let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height())
            .ok_or_else(|| error("SVG尺寸无效"))?;
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut pixmap.as_mut(),
        );
        let mut rgba = pixmap.data().to_vec();
        for pixel in rgba.chunks_exact_mut(4) {
            if pixel[3] > 0 {
                for c in 0..3 {
                    pixel[c] = ((pixel[c] as u32 * 255 + pixel[3] as u32 / 2) / pixel[3] as u32)
                        .min(255) as u8;
                }
            }
        }
        let source = RgbaImage::from_raw(size.width(), size.height(), rgba)
            .ok_or_else(|| error("SVG像素无效"))?;
        return crate::pin_storage::prepare_image(dest, &source);
    }
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(32768);
    limits.max_image_height = Some(32768);
    limits.max_alloc = Some(192 * 1024 * 1024);
    reader.limits(limits);
    let source: RgbaImage = reader.decode().map_err(io::Error::other)?.to_rgba8();
    crate::pin_storage::prepare_image(dest, &source)
}
