use serde_json::Value;
#[test]
fn original_calculator_edge_and_seeded_cases() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("calculator-oracle.json")).unwrap();
    for case in cases {
        let input = case["input"].as_str().unwrap();
        let locale = case["locale"].as_str().unwrap();
        let actual = quicktool_rust::calculator::calculate(input, locale)
            .and_then(|n| quicktool_rust::numbers::calculation(n, locale));
        if case["error"] == true {
            assert!(
                actual.is_err(),
                "{input} ({locale}): expected error, got {actual:?}"
            );
        } else {
            assert_eq!(actual.unwrap(), case["result"], "{input} ({locale})");
        }
    }
}
