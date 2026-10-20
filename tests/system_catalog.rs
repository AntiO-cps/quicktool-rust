use serde_json::{Value, json};
#[test]
fn windows_settings_search_matches_original() {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("windows-settings-oracle.json")).unwrap();
    for case in cases {
        let input = case["input"].as_str().unwrap();
        let actual = quicktool_rust::tools::windows_settings_search(input, 12)
            .into_iter()
            .map(|(item, score)| json!({"command":item.command,"score":score}))
            .collect::<Vec<_>>();
        let expected = case["results"].as_array().unwrap();
        assert_eq!(actual.len(), expected.len(), "{input}");
        for (a, b) in actual.iter().zip(expected) {
            assert_eq!(a["command"], b["command"], "{input}");
            assert_eq!(a["score"].as_f64(), b["score"].as_f64(), "{input}");
        }
    }
}
#[test]
fn command_catalog_includes_all_native_system_shortcuts() {
    let commands = quicktool_rust::tools::commands();
    assert_eq!(commands.len(), 47);
    for item in quicktool_rust::tools::system_commands() {
        let command = quicktool_rust::tools::command(&item.aliases[0]).unwrap();
        assert_eq!(command.id, format!("system:{}", item.id));
    }
    let empty = quicktool_rust::tools::suggestions("/");
    assert_eq!(empty.len(), 16);
    assert!(empty.iter().all(|(_, score)| *score == 0.7));
    let command = quicktool_rust::tools::command("calculator").unwrap();
    assert_eq!(
        quicktool_rust::tools::best_alias(command, "calc")
            .unwrap()
            .0,
        "calc"
    );
}
