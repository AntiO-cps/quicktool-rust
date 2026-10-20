use quicktool_rust::{calculator, core::*, search, units};
use serde_json::{Value, json};
#[test]
fn original_implementation_differential() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("oracle.json")).unwrap();
    for case in &cases {
        let op = case["operation"].as_str().unwrap();
        let a = case["args"][0].as_str().unwrap();
        let result: Result<Value, String> = match op {
            "baseResults" => base_results(a).map(|v| json!(v)),
            "toFullWidth" => Ok(json!(to_full_width(a, true))),
            "toHalfWidth" => Ok(json!(to_half_width(a))),
            "percentEncode" => Ok(json!(percent_encode(a))),
            "percentDecode" => percent_decode(a).map(|v| json!(v)),
            "unicodeEscapeEncode" => Ok(json!(unicode_escape(a))),
            "htmlEncode" => Ok(json!(html_encode(a))),
            "fuzzySearch" => Ok(json!(search::fuzzy(a, case["args"][1].as_str().unwrap()))),
            "calculateExpression" => {
                calculator::calculate(a, case["args"][1].as_str().unwrap()).map(|v| json!(v))
            }
            "unitResults" => units::results(a).map(|v| json!(v)),
            _ => panic!("Unimplemented oracle operation: {op}"),
        };
        if case["error"] == true {
            assert!(result.is_err(), "{op}({a}): expected an error");
            continue;
        }
        let actual = result.unwrap_or_else(|e| panic!("{op}({a}): {e}"));
        compare(&actual, &case["result"], &format!("{op}({a})"));
    }
}
fn compare(actual: &Value, expected: &Value, context: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            assert!(
                (a - b).abs() <= 1e-12 * b.abs().max(1.),
                "{context}: {a} != {b}"
            );
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{context}");
            for (a, b) in a.iter().zip(b) {
                compare(a, b, context)
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len(), "{context}");
            for (k, b) in b {
                compare(&a[k], b, context)
            }
        }
        _ => assert_eq!(actual, expected, "{context}"),
    }
}
