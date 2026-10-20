use serde::Deserialize;
#[derive(Deserialize, Debug, PartialEq)]
struct Group {
    id: String,
    score: f64,
}
#[derive(Deserialize)]
struct Case {
    input: String,
    groups: Vec<Group>,
}
#[test]
fn original_detection_groups_and_scores() {
    let cases: Vec<Case> = serde_json::from_str(include_str!("detection-oracle.json")).unwrap();
    for case in cases {
        let mut seen = std::collections::HashSet::new();
        let actual = quicktool_rust::tools::detect_scored(&case.input)
            .into_iter()
            .filter_map(|(id, _, score)| seen.insert(id.clone()).then_some(Group { id, score }))
            .collect::<Vec<_>>();
        assert_eq!(actual, case.groups, "query {:?}", case.input);
    }
}
