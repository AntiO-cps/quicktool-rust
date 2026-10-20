use quicktool_rust::unicode;
use serde_json::{Value, json};
#[test]
fn original_unicode_results_differential() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("unicode-oracle.json")).unwrap();
    for case in cases {
        let input = case["input"].as_str().unwrap();
        let kind = case["kind"].as_str().unwrap();
        let result = unicode::results(input, kind);
        if case["error"] == true {
            assert!(result.is_err(), "{kind}({input}) expected error");
        } else {
            assert_eq!(json!(result.unwrap()), case["result"], "{kind}({input})");
        }
    }
}
#[test]
fn original_number_format_differential() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("numbers-oracle.json")).unwrap();
    for case in cases {
        let raw = case["raw"].as_str().unwrap();
        let number: f64 = raw.parse().unwrap();
        let locale = case["locale"].as_str().unwrap();
        assert_eq!(
            quicktool_rust::numbers::calculation(number, locale).unwrap(),
            case["result"],
            "{raw} ({locale})"
        );
    }
}
