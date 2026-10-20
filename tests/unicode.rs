use quicktool_rust::unicode;
use serde_json::{Value, json};
#[test]
fn generated_index_preserves_every_original_name_and_category() {
    let metadata: Value =
        serde_json::from_str(include_str!("../assets/data/unicode-meta.json")).unwrap();
    let mut count = 0;
    for (hex, value) in metadata["meta"].as_object().unwrap() {
        let name = value[0].as_str().unwrap_or("");
        if name.is_empty() {
            continue;
        }
        let cp = u32::from_str_radix(hex, 16).unwrap();
        assert_eq!(
            unicode::character_metadata(cp),
            Some((name, value[1].as_str().unwrap_or("Cn"))),
            "U+{hex}"
        );
        count += 1;
    }
    assert_eq!(count, 143668);
    assert_eq!(unicode::character_metadata(0x110000), None);
}
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
