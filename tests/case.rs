use serde_json::Value;
#[test]
fn case_tools_match_original_electron_unicode_and_utf16_rules() {
    let oracle: Value = serde_json::from_str(include_str!("case-oracle.json")).unwrap();
    for row in oracle["cases"].as_array().unwrap() {
        let input = row["input"].as_str().unwrap();
        assert_eq!(
            serde_json::to_value(quicktool_rust::text::case_results(input)).unwrap(),
            row["result"],
            "input={input:?}"
        );
    }
}
