use crate::{
    pins::{PinHistoryUpdate, PinRecord},
    storage::atomic_write,
};
use image::{ImageEncoder, RgbaImage};
use sha2::{Digest, Sha256};
use std::{
    io,
    path::{Path, PathBuf},
};
pub struct PreparedPin {
    pub record: PinRecord,
    pub thumbnail: Option<String>,
}
pub enum Prepared {
    Pin(Box<PreparedPin>),
    Image(Box<crate::storage::HistoryItem>),
}
enum Job {
    Image(
        Box<crate::storage::HistoryItem>,
        std::sync::Arc<RgbaImage>,
        Option<crate::storage::EncodedImageSource>,
    ),
    Prepare(Box<PinHistoryUpdate>),
    Barrier(std::sync::mpsc::Sender<()>),
    Stop,
}
pub struct Worker {
    requests: std::sync::mpsc::Sender<Job>,
    pub responses: std::sync::mpsc::Receiver<io::Result<Prepared>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Worker {
    pub fn start(
        root: PathBuf,
        existing: Vec<(String, String)>,
        wake: impl Fn() + Send + 'static,
    ) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let (done, responses) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            let mut assets: std::collections::HashMap<String, String> =
                existing.into_iter().collect();
            while let Ok(job) = rx.recv() {
                match job {
                    Job::Prepare(update) => {
                        let result = prepare(
                            &root,
                            &update,
                            assets.get(&update.record.history_id).cloned(),
                        );
                        if let Ok(prepared) = &result {
                            if let Some(asset) = &prepared.record.asset {
                                assets.insert(prepared.record.history_id.clone(), asset.clone());
                            }
                        }
                        if done
                            .send(result.map(|pin| Prepared::Pin(Box::new(pin))))
                            .is_err()
                        {
                            break;
                        }
                        wake();
                    }
                    Job::Image(mut item, source, encoded) => {
                        let result = (|| -> io::Result<Prepared> {
                            // Validate before creating either asset, so invalid source bytes
                            // cannot publish an image history entry.
                            if let Some(encoded) = &encoded {
                                validate_encoded_source(encoded)?;
                            }
                            let asset = prepare_image(&root, &source)?;
                            if let Some(encoded) = &encoded {
                                item.image_source_asset =
                                    Some(write_encoded_source(&root, encoded)?);
                                item.image_source_mime = Some(encoded.mime.clone());
                            }
                            item.image_asset = Some(asset.clone());
                            item.thumbnail = Some(asset);
                            Ok(Prepared::Image(item))
                        })();
                        if done.send(result).is_err() {
                            break;
                        }
                        wake();
                    }
                    Job::Barrier(sender) => {
                        let _ = sender.send(());
                    }
                    Job::Stop => break,
                }
            }
        });
        Self {
            requests: tx,
            responses,
            thread: Some(thread),
        }
    }
    pub fn queue(&self, update: PinHistoryUpdate) -> io::Result<()> {
        self.requests
            .send(Job::Prepare(Box::new(update)))
            .map_err(|_| io::Error::other("Pin 保存服务已停止"))
    }
    pub fn queue_image(
        &self,
        item: crate::storage::HistoryItem,
        source: std::sync::Arc<RgbaImage>,
    ) -> io::Result<()> {
        self.queue_image_with_source(item, source, None)
    }
    pub fn queue_image_with_source(
        &self,
        item: crate::storage::HistoryItem,
        source: std::sync::Arc<RgbaImage>,
        encoded: Option<crate::storage::EncodedImageSource>,
    ) -> io::Result<()> {
        self.requests
            .send(Job::Image(Box::new(item), source, encoded))
            .map_err(|_| io::Error::other("图片历史保存服务已停止"))
    }
    pub fn flush(&self) -> io::Result<()> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.requests
            .send(Job::Barrier(tx))
            .map_err(|_| io::Error::other("Pin 保存服务已停止"))?;
        rx.recv()
            .map_err(|_| io::Error::other("Pin 保存服务已停止"))
    }
}

/// Encoded sources have their own namespace: their hash is over original bytes,
/// while image-assets always contains the decoded PNG used by pins/thumbnails.
pub fn encoded_source_path(root: &Path, asset: &str) -> io::Result<PathBuf> {
    let hash = asset
        .strip_prefix("source:")
        .ok_or_else(|| io::Error::other("原始图片资源引用无效"))?;
    if hash.len() != 64
        || !hash
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(io::Error::other("原始图片资源引用无效"));
    }
    Ok(root.join("image-sources").join(format!("{hash}.bin")))
}
fn validate_encoded_source(source: &crate::storage::EncodedImageSource) -> io::Result<()> {
    if source.bytes.is_empty() || source.bytes.len() > 192 * 1024 * 1024 {
        return Err(io::Error::other("原始图片为空或超过192MB"));
    }
    if source.mime.len() <= 6
        || source.mime.len() > 128
        || !source.mime.to_ascii_lowercase().starts_with("image/")
        || source
            .mime
            .bytes()
            .any(|c| c <= b' ' || c >= 127 || c == b';' || c == b',')
    {
        return Err(io::Error::other("原始图片MIME无效"));
    }
    let header = String::from_utf8_lossy(&source.bytes[..source.bytes.len().min(4096)]);
    if image::guess_format(&source.bytes).is_err() && header.contains("<svg") {
        let mut options = crate::annotations::svg_options();
        let external = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let detected = external.clone();
        options.image_href_resolver.resolve_string = Box::new(move |_, _| {
            detected.store(true, std::sync::atomic::Ordering::Relaxed);
            None
        });
        let tree =
            resvg::usvg::Tree::from_data(&source.bytes, &options).map_err(io::Error::other)?;
        if external.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(io::Error::other("原始SVG含外部资源，已保留历史"));
        }
        let size = tree.size().to_int_size();
        if u64::from(size.width()) * u64::from(size.height()) > 16_000_000 {
            return Err(io::Error::other("原始图片超过1600万像素"));
        }
    } else {
        let mut reader = image::ImageReader::new(std::io::Cursor::new(source.bytes.as_ref()))
            .with_guessed_format()?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(32768);
        limits.max_image_height = Some(32768);
        limits.max_alloc = Some(192 * 1024 * 1024);
        reader.limits(limits);
        let image = reader.decode().map_err(io::Error::other)?;
        if u64::from(image.width()) * u64::from(image.height()) > 16_000_000 {
            return Err(io::Error::other("原始图片超过1600万像素"));
        }
    }
    Ok(())
}
pub fn prepare_encoded_source(
    root: &Path,
    source: &crate::storage::EncodedImageSource,
) -> io::Result<String> {
    validate_encoded_source(source)?;
    write_encoded_source(root, source)
}
fn write_encoded_source(
    root: &Path,
    source: &crate::storage::EncodedImageSource,
) -> io::Result<String> {
    let asset = format!("source:{:x}", Sha256::digest(&source.bytes));
    let path = encoded_source_path(root, &asset)?;
    if path.exists() {
        if std::fs::read(&path)?.as_slice() != source.bytes.as_ref() {
            return Err(io::Error::other("原始图片编码校验失败，已保留文件"));
        }
    } else {
        atomic_write(&path, &source.bytes)?;
    }
    Ok(asset)
}
pub fn load_encoded_source(
    root: &Path,
    asset: &str,
    mime: &str,
) -> io::Result<crate::storage::EncodedImageSource> {
    let path = encoded_source_path(root, asset)?;
    if std::fs::metadata(&path)?.len() > 192 * 1024 * 1024 {
        return Err(io::Error::other("原始图片超过192MB"));
    }
    let bytes = std::fs::read(&path)?;
    if format!("source:{:x}", Sha256::digest(&bytes)) != asset {
        return Err(io::Error::other("原始图片编码校验失败，已保留文件"));
    }
    let source = crate::storage::EncodedImageSource {
        bytes: bytes.into(),
        mime: mime.into(),
    };
    validate_encoded_source(&source)?;
    Ok(source)
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.requests.send(Job::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
pub fn asset_path(root: &Path, asset: &str, thumbnail: bool) -> io::Result<PathBuf> {
    let hash = asset
        .strip_prefix("asset:")
        .ok_or_else(|| io::Error::other("图像资源引用无效"))?;
    if hash.len() != 64
        || !hash
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(io::Error::other("图像资源引用无效"));
    }
    Ok(root.join("image-assets").join(format!(
        "{hash}{}.png",
        if thumbnail { ".thumb" } else { "" }
    )))
}
fn png(image: &RgbaImage) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgba8,
        )
        .map_err(io::Error::other)?;
    Ok(bytes)
}
pub fn prepare(
    root: &Path,
    update: &PinHistoryUpdate,
    existing: Option<String>,
) -> io::Result<PreparedPin> {
    let mut record = update.record.clone();
    let mut thumbnail = None;
    if record.kind == "image" {
        let asset = existing.or_else(|| record.asset.clone());
        let asset = if let Some(asset) = asset {
            let path = asset_path(root, &asset, false)?;
            if !path.is_file() {
                return Err(io::Error::other("Pin 原图资源已丢失"));
            }
            asset
        } else {
            let source = update
                .source
                .as_ref()
                .ok_or_else(|| io::Error::other("缺少 Pin 原图"))?;
            prepare_image(root, source)?
        };
        record.asset = Some(asset.clone());
        thumbnail = Some(asset);
    }
    Ok(PreparedPin { record, thumbnail })
}
/// Shared content-addressed storage for Pin and ordinary image history.
pub fn prepare_image(root: &Path, source: &RgbaImage) -> io::Result<String> {
    if source.width() == 0
        || source.height() == 0
        || source.width() as u64 * source.height() as u64 > 16_000_000
    {
        return Err(io::Error::other("图片尺寸无效或超过1600万像素"));
    }
    let bytes = png(source)?;
    let asset = format!("asset:{:x}", Sha256::digest(&bytes));
    let path = asset_path(root, &asset, false)?;
    if path.exists() {
        // Preserve damaged originals instead of silently replacing them.
        if std::fs::read(&path)? != bytes {
            return Err(io::Error::other("原图校验失败，已保留文件"));
        }
    } else {
        atomic_write(&path, &bytes)?;
    }
    let thumbnail = asset_path(root, &asset, true)?;
    if !thumbnail.exists() {
        let small = image::imageops::thumbnail(source, 96, 64);
        atomic_write(&thumbnail, &png(&small)?)?;
    }
    Ok(asset)
}
pub fn load_image(root: &Path, asset: &str) -> io::Result<RgbaImage> {
    let path = asset_path(root, asset, false)?;
    let bytes = std::fs::read(path)?;
    if bytes.len() > 192 * 1024 * 1024 {
        return Err(io::Error::other("Pin 原图过大"));
    }
    let expected = asset.strip_prefix("asset:").unwrap();
    if format!("{:x}", Sha256::digest(&bytes)) != expected {
        return Err(io::Error::other("Pin 原图校验失败，保留文件"));
    }
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(io::Error::other)?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(32768);
    limits.max_image_height = Some(32768);
    limits.max_alloc = Some(192 * 1024 * 1024);
    reader.limits(limits);
    reader
        .decode()
        .map(|i| i.into_rgba8())
        .map_err(io::Error::other)
}
