use quicktool_rust::tools;
use serde_json::{Value, json};
#[test]
fn original_renderer_color_result_variants_and_formats() {
    let fixture: Value = serde_json::from_str(include_str!("detector-rows-oracle.json")).unwrap();
    for case in fixture["colors"].as_array().unwrap() {
        let input = case["input"].as_str().unwrap();
        let result = tools::execute("color", input);
        if case["error"] == true {
            assert!(result.is_err(), "query {input:?}");
        } else {
            assert_eq!(json!(result.unwrap()), case["rows"], "query {input:?}");
        }
    }
}
#[test]
fn original_detector_row_limits_and_per_item_score_decay() {
    let fixture: Value = serde_json::from_str(include_str!("detector-rows-oracle.json")).unwrap();
    for case in fixture["detectors"].as_array().unwrap() {
        let input = case["input"].as_str().unwrap();
        let actual = tools::detect_scored(input)
            .into_iter()
            .filter(|(id, _, _)| matches!(id.as_str(), "time" | "color"))
            .take(16)
            .map(|(tool, row, score)| json!({"tool":tool,"title":row.title,"score":score}))
            .collect::<Vec<_>>();
        assert_eq!(json!(actual), case["rows"], "query {input:?}");
    }
}
