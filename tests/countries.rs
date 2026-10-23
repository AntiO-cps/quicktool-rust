use quicktool_rust::{countries, tools};
use serde::Deserialize;

#[derive(Deserialize)]
struct Oracle {
    countries: usize,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    input: String,
    matches: Vec<(String, f64)>,
    detection: Option<f64>,
    cards: Vec<(String, f64)>,
    completions: Vec<(String, String)>,
}
fn rows(values: Vec<(serde_json::Value, f64)>) -> Vec<(String, f64)> {
    values
        .into_iter()
        .map(|(country, score)| (country["alpha2"].as_str().unwrap().into(), score))
        .collect()
}
#[test]
fn all_original_country_queries_detection_and_card_ranking() {
    let oracle: Oracle = serde_json::from_str(include_str!("countries-oracle.json")).unwrap();
    assert_eq!(oracle.countries, 249);
    assert!(oracle.cases.len() > 5000);
    for case in oracle.cases {
        assert_eq!(
            rows(countries::scored_matches(&case.input, 8)),
            case.matches,
            "matches {:?}",
            case.input
        );
        assert_eq!(
            countries::detection_score(&case.input),
            case.detection,
            "detection {:?}",
            case.input
        );
        assert_eq!(
            rows(countries::card_matches(&case.input)),
            case.cards,
            "cards {:?}",
            case.input
        );
        let completions = countries::scored_matches(&case.input, 4)
            .into_iter()
            .map(|(country, _)| {
                (
                    country["alpha2"].as_str().unwrap().to_string(),
                    countries::completion(&country, &case.input),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(completions, case.completions, "completion {:?}", case.input);
    }
}
#[test]
fn swiss_lowercase_card_overrides_detector_score() {
    assert_eq!(countries::detection_score("ch"), Some(0.86));
    assert_eq!(countries::detection_score("CH"), Some(0.99));
    let cards = rows(countries::card_matches("ch"));
    assert_eq!(cards[0], ("CH".into(), 1.15));
    assert_eq!(cards.len(), 4);
    assert!(cards[1..].iter().all(|(_, score)| *score == 0.72));
    assert_eq!(
        tools::execute("country", "不存在").unwrap_err(),
        "未找到国家/地区"
    );
}
