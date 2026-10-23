use quicktool_rust::tools;
use serde::Deserialize;
use std::collections::HashMap;
#[derive(Deserialize)]
struct Fixture {
    matching: Vec<Matching>,
    parsing: Vec<Parsing>,
    submodes: Vec<Submode>,
    usage: Usage,
}
#[derive(Deserialize)]
struct Matching {
    input: String,
    matches: Vec<(String, String, f64)>,
}
#[derive(Deserialize)]
struct Parsing {
    input: String,
    token: String,
    arg: String,
    exact: Option<String>,
}
#[derive(Deserialize)]
struct Submode {
    tool: String,
    input: String,
    submode: Option<String>,
    value: String,
}
#[derive(Deserialize)]
struct Usage {
    input: String,
    boosts: HashMap<String, f64>,
    matches: Vec<(String, String, f64)>,
}
fn fixture() -> Fixture {
    serde_json::from_str(include_str!("command-matching-oracle.json")).unwrap()
}
fn matches(input: &str, boosts: &HashMap<String, f64>) -> Vec<(String, String, f64)> {
    let mut rows = tools::suggestions_all(input)
        .into_iter()
        .map(|(tool, score)| {
            (
                tool.id.clone(),
                tools::best_alias(tool, input).unwrap().0.to_string(),
                score + boosts.get(&tool.id).copied().unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| b.2.total_cmp(&a.2));
    rows
}
#[test]
fn original_command_alias_matching_and_order() {
    for case in fixture().matching {
        assert_eq!(
            matches(&case.input, &HashMap::new()),
            case.matches,
            "query {:?}",
            case.input
        );
    }
}
#[test]
fn original_command_javascript_whitespace_and_submode_parsing() {
    let fixture = fixture();
    for case in fixture.parsing {
        let (token, arg) = tools::parse_command_input(&case.input);
        assert_eq!(
            (token, arg),
            (case.token.as_str(), case.arg.as_str()),
            "command {:?}",
            case.input
        );
        assert_eq!(
            tools::command(token).map(|c| c.id.clone()),
            case.exact,
            "exact {:?}",
            case.input
        );
    }
    for case in fixture.submodes {
        assert_eq!(
            tools::parse_submode(&case.tool, &case.input),
            (case.submode, case.value),
            "{} input {:?}",
            case.tool,
            case.input
        );
    }
}
#[test]
fn usage_is_applied_before_suggestion_truncation() {
    let usage = fixture().usage;
    let rows = matches(&usage.input, &usage.boosts)
        .into_iter()
        .take(16)
        .collect::<Vec<_>>();
    assert_eq!(rows, usage.matches);
    assert_eq!(rows[0].0, "system:control");
    assert!(
        !tools::suggestions("")
            .iter()
            .any(|(c, _)| c.id == "system:control")
    );
}
