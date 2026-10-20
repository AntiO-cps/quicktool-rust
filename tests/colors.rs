use quicktool_rust::colors;
use serde_json::Value;
#[test]
fn original_color_pipeline_differential() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("colors-oracle.json")).unwrap();
    for case in cases {
        let input = case["input"].as_str().unwrap();
        let actual = colors::candidates(input);
        let expected = case["colors"].as_array().unwrap();
        assert_eq!(actual.len(), expected.len(), "{input}");
        for (c, e) in actual.into_iter().zip(expected) {
            assert_eq!(c.hex(true), e["hex"], "{input}");
            assert_eq!(
                serde_json::to_value(colors::formats(c)).unwrap(),
                e["formats"],
                "{input}"
            );
        }
    }
}
