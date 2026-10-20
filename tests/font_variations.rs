//! Exercises the actual epaint atlas path used by the native UI.
use eframe::egui::{
    Color32, FontData, FontDefinitions, FontFamily, FontId,
    epaint::{Fonts, ImageData},
};
use std::sync::Arc;

const MISANS: &[u8] = include_bytes!("../assets/assets/fonts/MiSans-VF.ttf");
const MONO: &[u8] = include_bytes!("../assets/assets/fonts/JetBrainsMono-VF.ttf");

fn atlas(font: FontData, text: &str) -> (Vec<f32>, f32) {
    let mut definitions = FontDefinitions::empty();
    definitions.font_data.insert("test".into(), Arc::new(font));
    definitions
        .families
        .insert(FontFamily::Proportional, vec!["test".into()]);
    let fonts = Fonts::new(1.25, 4096, definitions);
    let galley = fonts.layout_no_wrap(text.into(), FontId::proportional(24.), Color32::WHITE);
    let delta = fonts.font_image_delta().unwrap();
    let ImageData::Font(image) = delta.image else {
        panic!("expected coverage atlas")
    };
    (image.pixels, galley.size().x)
}

#[test]
fn ui_weight_changes_real_atlas_coverage_and_shares_embedded_bytes() {
    let regular = FontData::from_static(MISANS).variation(*b"wght", 450.);
    let bold = FontData::from_static(MISANS).variation(*b"wght", 650.);
    assert!(std::ptr::eq(regular.font.as_ptr(), bold.font.as_ptr()));
    let (regular, _) = atlas(regular, "设置结果 QuickTool");
    let (bold, _) = atlas(bold, "设置结果 QuickTool");
    assert!(bold.iter().sum::<f32>() > regular.iter().sum::<f32>() * 1.1);
}

#[test]
fn unknown_axis_keeps_default_output_unchanged() {
    let original = atlas(FontData::from_static(MISANS), "Aa设置");
    let unknown = atlas(
        FontData::from_static(MISANS).variation(*b"xxxx", 650.),
        "Aa设置",
    );
    assert_eq!(original, unknown);
}

#[test]
fn monospaced_results_keep_the_same_advance_at_original_weight() {
    let regular = atlas(
        FontData::from_static(MONO).variation(*b"wght", 400.),
        "0xFF 0123456789",
    );
    let result = atlas(
        FontData::from_static(MONO).variation(*b"wght", 610.),
        "0xFF 0123456789",
    );
    assert_eq!(regular.1, result.1);
    assert!(result.0.iter().sum::<f32>() > regular.0.iter().sum::<f32>());
}
