use quicktool_rust::{history_image::processed_pin_image, pins::PinRecord, pixels::Filters};
use serde::Deserialize;
#[derive(Deserialize)]
struct Case {
    filters: Filters,
    input: Vec<u8>,
    output: Vec<u8>,
}
#[test]
fn copied_pin_history_uses_original_processed_pixels() {
    let cases: Vec<Case> = serde_json::from_str(include_str!("pixels-oracle.json")).unwrap();
    for (i, case) in cases.into_iter().enumerate() {
        let record:PinRecord=serde_json::from_value(serde_json::json!({
            "historyId":"synthetic-copy", "kind":"image", "text":"", "originalText":"", "asset":null,
            "filters":case.filters,"annotations":[],"cornerRadius":0.,"fontSize":15.,"background":[0,0,0],
            "foreground":null,"opacity":1.,"blur":0.,"width":case.input.len()/4,"height":1.,"position":null,"created":0
        })).unwrap();
        let source =
            image::RgbaImage::from_raw((case.input.len() / 4) as u32, 1, case.input).unwrap();
        let result = processed_pin_image(&record, &source).unwrap();
        assert_eq!(result.as_raw(), &case.output, "Original filter case {i}");
    }
}
