//! Compose stored Pin image adjustments before copying from history.
use crate::{annotations, pins::PinRecord, pixels};
use image::RgbaImage;

pub fn processed_pin_image(record: &PinRecord, source: &RgbaImage) -> Result<RgbaImage, String> {
    if source.width() == 0 || source.height() == 0 {
        return Err("图片尺寸无效".into());
    }
    // Match ImageExportClient.process and the Pin copy/export pixel budget.
    let scale = (16_000_000. / (f64::from(source.width()) * f64::from(source.height())))
        .sqrt()
        .min(1.);
    let width = (f64::from(source.width()) * scale).round().max(1.) as u32;
    let height = (f64::from(source.height()) * scale).round().max(1.) as u32;
    let mut image = if (width, height) == source.dimensions() {
        source.clone()
    } else {
        image::imageops::resize(source, width, height, image::imageops::FilterType::Lanczos3)
    };
    pixels::process(image.as_mut(), record.filters)?;
    let display_width = if record.width.is_finite() && record.width > 0. {
        record.width
    } else {
        source.width() as f32
    };
    annotations::render(
        &image,
        &record.annotations,
        record.corner_radius * width as f32 / display_width,
    )
}
