use quicktool_rust::search;
use serde_json::Value;
#[test]
fn fuzzy_search_matches_original_electron_unicode_and_boundaries() {
    let fixture: Value = serde_json::from_str(include_str!("search-oracle.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let query = case["query"].as_str().unwrap();
        let target = case["target"].as_str().unwrap();
        let actual = serde_json::to_value(search::fuzzy(query, target)).unwrap();
        assert_eq!(
            actual["success"], case["result"]["success"],
            "query={query:?} target={target:?}"
        );
        assert_eq!(
            actual["indexes"], case["result"]["indexes"],
            "query={query:?} target={target:?}"
        );
        let score = actual["score"].as_f64().unwrap();
        let expected = case["result"]["score"].as_f64().unwrap();
        assert!(
            (score - expected).abs() < 1e-12,
            "query={query:?} target={target:?}: {score} != {expected}"
        );
    }
}
