use crate::core::{ResultItem, normalize_search};
use std::sync::OnceLock;
#[derive(Clone)]
struct Unit {
    cat: String,
    label: String,
    factor: f64,
    aliases: Vec<String>,
}
fn units() -> &'static Vec<Unit> {
    static UNITS: OnceLock<Vec<Unit>> = OnceLock::new();
    UNITS.get_or_init(|| {
        let data: serde_json::Value =
            serde_json::from_str(include_str!("../assets/units.json")).unwrap();
        let mut out = Vec::new();
        // Preserve the original insertion order for automatic conversion candidates.
        for cat in [
            "length",
            "mass",
            "duration",
            "area",
            "volume",
            "speed",
            "pressure",
            "energy",
            "power",
            "information",
            "angle",
            "acceleration",
        ] {
            for row in data[cat].as_array().unwrap() {
                out.push(Unit {
                    cat: cat.into(),
                    label: row[0].as_str().unwrap().into(),
                    factor: row[1].as_f64().unwrap(),
                    aliases: row[2]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|s| normalize_search(s.as_str().unwrap()))
                        .collect(),
                });
            }
        }
        for (label, aliases) in [
            ("°C", vec!["c", "°c", "celsius", "摄氏", "摄氏度"]),
            ("°F", vec!["f", "°f", "fahrenheit", "华氏", "华氏度"]),
            ("K", vec!["k", "kelvin", "开尔文"]),
        ] {
            out.push(Unit {
                cat: "temperature".into(),
                label: label.into(),
                factor: 1.,
                aliases: aliases.into_iter().map(normalize_search).collect(),
            });
        }
        out
    })
}
fn lookup(s: &str) -> Option<&'static Unit> {
    let n = normalize_search(s);
    units().iter().find(|u| u.aliases.contains(&n))
}
pub fn number(n: f64) -> String {
    if n == 0. {
        return if n.is_sign_negative() {
            "-0".into()
        } else {
            "0".into()
        };
    }
    let a = n.abs();
    if !(1e-6..1e12).contains(&a) {
        let text = format!("{n:.8e}");
        let (m, e) = text.split_once('e').unwrap();
        let e: i32 = e.parse().unwrap();
        let mut m = m.to_string();
        while m.ends_with('0') {
            m.pop();
        }
        return format!("{m}e{}{e}", if e >= 0 { "+" } else { "" });
    }
    crate::numbers::significant(n, 12)
}
fn convert(v: f64, from: &Unit, to: &Unit) -> f64 {
    if from.cat != "temperature" {
        return v * from.factor / to.factor;
    }
    let c = match from.label.as_str() {
        "°F" => (v - 32.) * 5. / 9.,
        "K" => v - 273.15,
        _ => v,
    };
    match to.label.as_str() {
        "°F" => c * 9. / 5. + 32.,
        "K" => c + 273.15,
        _ => c,
    }
}
pub fn results(raw: &str) -> Result<Vec<ResultItem>, String> {
    let raw = raw.trim();
    let end = raw
        .char_indices()
        .take_while(|&(i, c)| {
            c.is_ascii_digit()
                || c == '.'
                || ((c == '+' || c == '-')
                    && (i == 0
                        || raw
                            .as_bytes()
                            .get(i - 1)
                            .is_some_and(|c| *c == b'e' || *c == b'E')))
                || ((c == 'e' || c == 'E')
                    && i > 0
                    && raw
                        .as_bytes()
                        .get(i + 1)
                        .is_some_and(|c| c.is_ascii_digit() || *c == b'+' || *c == b'-'))
        })
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .ok_or("无法识别单位转换")?;
    let v: f64 = raw[..end].parse().map_err(|_| "无法识别数字")?;
    if !v.is_finite() {
        return Err("数字超出范围".into());
    }
    let rest_owned = raw[end..].trim().to_lowercase();
    let rest = rest_owned.as_str();
    let mut pair = None;
    for delimiter in [" to ", " in ", " = ", " → ", " 转 ", " 转换为 ", " 换算为 "] {
        if let Some((a, b)) = rest.split_once(delimiter) {
            pair = Some((a, b));
            break;
        }
    }
    let (from, to) = match pair {
        Some((a, b)) => (
            lookup(a).ok_or("未知单位")?,
            Some(lookup(b).ok_or("未知单位")?),
        ),
        None => (lookup(rest).ok_or("未知单位")?, None),
    };
    if to.is_some_and(|t| t.cat != from.cat) {
        return Err("单位类别不一致".into());
    }
    let targets: Vec<&Unit> = if let Some(to) = to {
        vec![to]
    } else {
        units()
            .iter()
            .filter(|u| u.cat == from.cat && u.label != from.label)
            .take(8)
            .collect()
    };
    Ok(targets
        .into_iter()
        .map(|to| {
            ResultItem::new(
                format!("{} → {}", from.label, to.label),
                format!("{} {}", number(convert(v, from, to)), to.label),
            )
        })
        .collect())
}
