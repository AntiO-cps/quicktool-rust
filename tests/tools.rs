use quicktool_rust::{core, countries, tools};
use serde_json::{Value, json};
fn equivalent(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| equivalent(a, b))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(k, v)| b.get(k).is_some_and(|other| equivalent(v, other)))
        }
        _ => left == right,
    }
}
#[test]
fn original_tool_outputs_differential() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("tools-oracle.json")).unwrap();
    for case in cases {
        let op = case["operation"].as_str().unwrap();
        let input = case["input"].as_str().unwrap();
        let result = if op == "country" {
            Ok(json!(
                countries::scored_matches(input, 8)
                    .into_iter()
                    .map(|(c, s)| json!({"alpha2":c["alpha2"],"score":s}))
                    .collect::<Vec<_>>()
            ))
        } else if op == "base" {
            core::base_results(input).map(|v| json!(v))
        } else {
            tools::execute(op, input).map(|v| json!(v))
        };
        if case["error"] == true {
            assert!(result.is_err(), "{op}({input}) should reject");
        } else {
            let actual = result.unwrap_or_else(|e| panic!("{op}({input}): {e}"));
            assert!(
                equivalent(&actual, &case["result"]),
                "{op}({input}): {actual:?} != {:?}",
                case["result"]
            );
        }
    }
}
