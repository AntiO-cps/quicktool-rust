use quicktool_rust::dates;
use serde_json::{Value, json};
#[test]
fn original_date_parser_differential() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("dates-oracle.json")).unwrap();
    for case in cases {
        let input = case["input"].as_str().unwrap();
        let order = case["order"].as_str().unwrap();
        let actual=dates::parse(input,order).map(|p|json!({"millis":p.date.unix_timestamp_nanos()/1_000_000,"format":p.format,"precision":p.precision,"recognized":dates::recognized(p.date),"alternatives":p.alternatives.iter().map(|a|dates::recognized(a.date)).collect::<Vec<_>>()})).unwrap_or(Value::Null);
        assert_eq!(actual, case["result"], "{input} ({order})");
    }
}
