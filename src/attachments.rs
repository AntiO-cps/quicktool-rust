//! Image attachments retain their encoded source for lossless Base64 / Data URL conversion.
use base64::{Engine, engine::general_purpose::STANDARD};
use image::{ImageEncoder, ImageFormat, RgbaImage};
use quicktool_rust::{annotations, core::ResultItem, platform};
use std::{path::Path, sync::Arc};

#[derive(Clone)]
pub struct Attachment {
    pub name: String,
    pub mime: String,
    pub bytes: Arc<[u8]>,
    pub rgba: Arc<RgbaImage>,
    pub thumbnail: Arc<RgbaImage>,
}
impl Attachment {
    pub fn from_base64(input: &str) -> Result<Self, String> {
        let input = input.trim();
        let (mut encoded, mut declared) = (input, None);
        if input
            .get(..5)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"))
        {
            let (header, data) = input.split_once(',').ok_or("Data URL 格式无效")?;
            let suffix = ";base64";
            if header.len() < 5 + suffix.len()
                || !header
                    .get(header.len() - suffix.len()..)
                    .is_some_and(|tail| tail.eq_ignore_ascii_case(suffix))
            {
                return Err("Data URL 必须使用 Base64 编码".into());
            }
            let mime = &header[5..header.len() - suffix.len()];
            if !mime.to_ascii_lowercase().starts_with("image/") || mime.contains(';') {
                return Err("Data URL 不是图片".into());
            }
            declared = Some(mime);
            encoded = data;
        }
        let raw = encoded
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '\u{feff}')
            .collect::<String>();
        if raw.len() < 16 || raw.len() > 256 * 1024 * 1024 {
            return Err("图片 Base64 长度无效".into());
        }
        let config = base64::engine::GeneralPurposeConfig::new()
            .with_decode_padding_mode(base64::engine::DecodePaddingMode::Indifferent)
            .with_decode_allow_trailing_bits(true);
        let engine = base64::engine::GeneralPurpose::new(&base64::alphabet::STANDARD, config);
        let bytes = engine.decode(raw).map_err(|_| "图片 Base64 编码无效")?;
        let mut attachment = Self::from_bytes("Base64 图片", &bytes)?;
        if let Some(mime) = declared {
            attachment.mime = mime.into();
        }
        Ok(attachment)
    }
    pub fn from_clipboard() -> Result<Self, String> {
        let (width, height, rgba) = platform::clipboard_image()?;
        let rgba = RgbaImage::from_raw(width, height, rgba).ok_or("剪贴板图片数据无效")?;
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(
                rgba.as_raw(),
                width,
                height,
                image::ExtendedColorType::Rgba8,
            )
            .map_err(|e| e.to_string())?;
        Ok(Self {
            name: "剪贴板图片".into(),
            mime: "image/png".into(),
            bytes: bytes.into(),
            thumbnail: Arc::new(image::imageops::thumbnail(&rgba, 128, 96)),
            rgba: Arc::new(rgba),
        })
    }
    pub fn from_path(path: &Path) -> Result<Self, String> {
        let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
        if metadata.len() > 192 * 1024 * 1024 {
            return Err("图片文件过大".into());
        }
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        Self::from_bytes(
            path.file_name().unwrap_or_default().to_string_lossy(),
            &bytes,
        )
    }
    pub fn from_bytes(name: impl Into<String>, bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() > 192 * 1024 * 1024 {
            return Err("图片文件为空或过大".into());
        }
        let format = image::guess_format(bytes).ok();
        let header = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]);
        let svg = format.is_none() && header.contains("<svg");
        let (mime, rgba) = if svg {
            let tree = resvg::usvg::Tree::from_data(bytes, &annotations::svg_options())
                .map_err(|e| e.to_string())?;
            let size = tree.size().to_int_size();
            if size.width() as u64 * size.height() as u64 > 64_000_000 {
                return Err("SVG 图片尺寸超过内存限制".into());
            }
            let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height())
                .ok_or("SVG 图片尺寸无效")?;
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::identity(),
                &mut pixmap.as_mut(),
            );
            let mut rgba = pixmap.take();
            for pixel in rgba.chunks_exact_mut(4) {
                if pixel[3] > 0 {
                    for channel in 0..3 {
                        pixel[channel] = ((pixel[channel] as u32 * 255 + pixel[3] as u32 / 2)
                            / pixel[3] as u32)
                            .min(255) as u8;
                    }
                }
            }
            let image =
                RgbaImage::from_raw(size.width(), size.height(), rgba).ok_or("SVG 像素数据无效")?;
            ("image/svg+xml", image)
        } else {
            let format = format.ok_or("无法识别图片格式")?;
            let mime = match format {
                ImageFormat::Png => "image/png",
                ImageFormat::Jpeg => "image/jpeg",
                ImageFormat::Gif => "image/gif",
                ImageFormat::WebP => "image/webp",
                ImageFormat::Bmp => "image/bmp",
                ImageFormat::Ico => "image/x-icon",
                ImageFormat::Tiff => "image/tiff",
                _ => return Err("不支持此图片格式".into()),
            };
            let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
            let mut limits = image::Limits::default();
            limits.max_alloc = Some(256 * 1024 * 1024);
            reader.limits(limits);
            let rgba = reader.decode().map_err(|e| e.to_string())?.to_rgba8();
            (mime, rgba)
        };
        Ok(Self {
            name: name.into(),
            mime: mime.into(),
            bytes: Arc::from(bytes),
            thumbnail: Arc::new(image::imageops::thumbnail(&rgba, 128, 96)),
            rgba: Arc::new(rgba),
        })
    }
    pub fn data_url(&self) -> String {
        format!("data:{};base64,{}", self.mime, STANDARD.encode(&self.bytes))
    }
    pub fn results(&self) -> Vec<ResultItem> {
        let encoded = STANDARD.encode(&self.bytes);
        let data_url = format!("data:{};base64,{encoded}", self.mime);
        vec![
            ResultItem::new("图片 → Base64", encoded),
            ResultItem::new("图片 → Data URL", data_url),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_source_and_result_order_are_preserved() {
        let rgba = RgbaImage::from_pixel(2, 1, image::Rgba([9, 19, 29, 128]));
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(rgba.as_raw(), 2, 1, image::ExtendedColorType::Rgba8)
            .unwrap();
        let attachment = Attachment::from_bytes("sample.png", &png).unwrap();
        assert_eq!(attachment.bytes.as_ref(), png.as_slice());
        let cloned = attachment.clone();
        assert!(Arc::ptr_eq(&attachment.bytes, &cloned.bytes));
        assert!(Arc::ptr_eq(&attachment.rgba, &cloned.rgba));
        assert!(Arc::ptr_eq(&attachment.thumbnail, &cloned.thumbnail));
        assert_eq!(attachment.rgba.as_ref(), &rgba);
        let results = attachment.results();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "图片 → Base64");
        assert_eq!(STANDARD.decode(&results[0].value).unwrap(), png);
        assert_eq!(results[1].title, "图片 → Data URL");
        assert_eq!(results[1].value, attachment.data_url());
    }
    #[test]
    fn svg_alpha_is_unpremultiplied() {
        let input=br##"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2" fill="#ff0000" opacity=".5"/></svg>"##;
        let attachment = Attachment::from_bytes("sample.svg", input).unwrap();
        assert_eq!(attachment.mime, "image/svg+xml");
        let p = attachment.rgba.get_pixel(0, 0);
        assert_eq!(p[0], 255);
        assert!(p[3].abs_diff(128) <= 1);
        assert_eq!(attachment.bytes.as_ref(), input.as_slice());
    }
    #[test]
    fn invalid_images_are_errors() {
        assert!(Attachment::from_bytes("text.txt", b"ordinary text").is_err());
        assert!(Attachment::from_bytes("empty.png", b"").is_err());
    }
    #[test]
    fn base64_image_formats_preserve_the_original_bytes() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"/>"#;
        let encoded = STANDARD.encode(svg);
        for text in [
            encoded.clone(),
            encoded.trim_end_matches('=').into(),
            format!("data:image/svg+xml;base64,{encoded}"),
            format!("data:image/svg+xml;base64,\n{encoded}\n"),
        ] {
            let a = Attachment::from_base64(&text).unwrap();
            assert_eq!(a.bytes.as_ref(), svg.as_slice());
            assert_eq!(a.rgba.dimensions(), (2, 2));
            assert_eq!(a.mime, "image/svg+xml");
        }
        for invalid in [
            "data:😃😃😃,abc",
            "data:text/plain;base64,aGVsbG8=",
            "not an image",
        ] {
            assert!(Attachment::from_base64(invalid).is_err());
        }
    }
}
