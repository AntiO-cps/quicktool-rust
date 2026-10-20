use crate::core::ResultItem;
use serde_json::Value;
pub fn stringify(value: &Value, pretty: bool) -> String {
    let mut out = String::new();
    write(value, pretty, 0, &mut out);
    out
}
fn property_index(s: &str) -> Option<u32> {
    if s.is_empty() || (s.len() > 1 && s.starts_with('0')) {
        return None;
    }
    let n: u32 = s.parse().ok()?;
    (n < u32::MAX && n.to_string() == s).then_some(n)
}
fn write(value: &Value, pretty: bool, depth: usize, out: &mut String) {
    let newline = |out: &mut String, level: usize| {
        if pretty {
            out.push('\n');
            out.push_str(&"  ".repeat(level));
        }
    };
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(v) => out.push_str(if *v { "true" } else { "false" }),
        Value::Number(n) => {
            let number = n.to_string().parse::<f64>().unwrap_or(f64::NAN);
            if number.is_finite() {
                out.push_str(ryu_js::Buffer::new().format_finite(number));
            } else {
                out.push_str("null");
            }
        }
        Value::String(s) => out.push_str(&serde_json::to_string(s).unwrap()),
        Value::Array(values) => {
            out.push('[');
            for (i, v) in values.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                newline(out, depth + 1);
                write(v, pretty, depth + 1, out);
            }
            if !values.is_empty() {
                newline(out, depth);
            }
            out.push(']');
        }
        Value::Object(map) => {
            out.push('{');
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by(|a, b| match (property_index(a.0), property_index(b.0)) {
                (Some(a), Some(b)) => a.cmp(&b),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            });
            for (i, (k, v)) in entries.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                newline(out, depth + 1);
                out.push_str(&serde_json::to_string(k).unwrap());
                out.push(':');
                if pretty {
                    out.push(' ');
                }
                write(v, pretty, depth + 1, out);
            }
            if !entries.is_empty() {
                newline(out, depth);
            }
            out.push('}');
        }
    }
}
pub fn results(s: &str) -> Result<Vec<ResultItem>, String> {
    let value: Value = serde_json::from_str(s).map_err(|e| e.to_string())?;
    Ok(vec![
        ResultItem::new("格式化 JSON", stringify(&value, true)),
        ResultItem::new("压缩 JSON", stringify(&value, false)),
    ])
}
