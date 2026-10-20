use crate::{
    core::{ResultItem, code_points, unicode_escape},
    countries::locale_compare,
};
use serde_json::Value;
use std::sync::OnceLock;
fn metadata() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/data/unicode-meta.json")).unwrap()
    })
}
struct Row {
    hex: &'static str,
    name: &'static str,
    category: &'static str,
    cp: u32,
}
fn index() -> &'static Vec<Row> {
    static INDEX: OnceLock<Vec<Row>> = OnceLock::new();
    INDEX.get_or_init(|| {
        let mut rows: Vec<_> = metadata()["meta"]
            .as_object()
            .unwrap()
            .iter()
            .filter_map(|(hex, value)| {
                let name = value[0].as_str()?;
                if name.is_empty() {
                    return None;
                }
                Some(Row {
                    hex,
                    name,
                    category: value[1].as_str().unwrap_or("Cn"),
                    cp: u32::from_str_radix(hex, 16).ok()?,
                })
            })
            .collect();
        let key = |hex: &str| {
            hex.parse::<u32>()
                .ok()
                .filter(|n| n.to_string() == hex && *n < u32::MAX)
        };
        rows.sort_by(|a, b| match (key(a.hex), key(b.hex)) {
            (Some(a), Some(b)) => a.cmp(&b),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            _ => std::cmp::Ordering::Equal,
        });
        rows
    })
}
fn filter(row: &Row, kind: &str) -> bool {
    let cp = row.cp;
    match kind {
        "math" | "mathchar" => {
            row.category == "Sm"
                || [
                    "MATHEMATICAL",
                    "INTEGRAL",
                    "SUMMATION",
                    "PRODUCT",
                    "EQUAL",
                    "ANGLE",
                    "SUBSET",
                    "SUPERSET",
                    "ARROW",
                ]
                .iter()
                .any(|s| row.name.contains(s))
        }
        "greek" => {
            (0x370..=0x3ff).contains(&cp)
                || (0x1f00..=0x1fff).contains(&cp)
                || (0x1d6a8..=0x1d7cb).contains(&cp)
        }
        "bopomofo" => (0x3100..=0x312f).contains(&cp) || (0x31a0..=0x31bf).contains(&cp),
        "emoji" => {
            (0x1f000..=0x1faff).contains(&cp)
                || (0x2600..=0x27bf).contains(&cp)
                || (0x1f1e6..=0x1f1ff).contains(&cp)
        }
        _ => true,
    }
}
fn aliases() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/unicode-aliases.json")).unwrap()
    })
}
pub fn name_search(q: &str, kind: &str) -> Vec<ResultItem> {
    let raw = q.trim();
    if raw.is_empty() {
        return vec![];
    }
    let terms: Vec<String> = if let Some(values) = aliases()[raw].as_array() {
        values
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect()
    } else {
        raw.to_uppercase()
            .split_whitespace()
            .map(str::to_string)
            .collect()
    };
    let mut found = Vec::new();
    for row in index() {
        if !filter(row, kind) || !terms.iter().any(|t| row.name.contains(t)) {
            continue;
        }
        let Some(ch) = char::from_u32(row.cp) else {
            continue;
        };
        let score: f64 = if terms.iter().any(|t| row.name == t) {
            1.
        } else if terms.iter().any(|t| row.name.ends_with(&format!(" {t}"))) {
            0.94
        } else {
            0.78
        };
        let score = score
            + if kind != "char" && !kind.is_empty() {
                0.03
            } else {
                0.
            };
        found.push((
            score,
            ResultItem::new(format!("{ch} · U+{:04X}", row.cp), ch.to_string()),
        ));
        if found.len() >= 80 {
            break;
        }
    }
    found.sort_by(|a, b| {
        b.0.total_cmp(&a.0)
            .then_with(|| locale_compare(&a.1.title, &b.1.title))
    });
    found.into_iter().take(24).map(|(_, r)| r).collect()
}
pub fn results(q: &str, kind: &str) -> Result<Vec<ResultItem>, String> {
    let raw = q.trim();
    let cps = code_points(raw);
    let name_like = raw.len() >= 2
        && raw.len() <= 41
        && raw.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        && raw
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b" _-".contains(&b))
        && cps.is_none();
    if kind != "char" || aliases().get(raw).is_some() || name_like {
        let found = name_search(raw, kind);
        if !found.is_empty() {
            return Ok(found);
        }
    }
    let text: String = cps
        .as_ref()
        .map(|c| c.iter().collect())
        .unwrap_or_else(|| q.to_string());
    if text.is_empty() {
        return Err("请输入字符、Unicode 码位或 Unicode 名称".into());
    }
    let mut out = if cps.is_some() {
        vec![ResultItem::new("Unicode → 字符", &text)]
    } else {
        vec![]
    };
    for ch in text.chars().take(24) {
        let cp = ch as u32;
        let hex = format!("{cp:X}");
        let padded = format!("{cp:04X}");
        let default = if (0x4e00..=0x9fff).contains(&cp) {
            format!("CJK UNIFIED IDEOGRAPH-{hex}")
        } else {
            "<unnamed>".into()
        };
        let name = metadata()["meta"][&hex][0]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or(&default);
        let utf8 = ch
            .to_string()
            .bytes()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(" ");
        let utf16 = ch
            .to_string()
            .encode_utf16()
            .map(|b| format!("{b:04X}"))
            .collect::<Vec<_>>()
            .join(" ");
        out.extend([
            ResultItem::new(format!("{ch} · U+{padded}"), name),
            ResultItem::new(format!("U+{padded} UTF-8"), utf8),
            ResultItem::new(format!("U+{padded} UTF-16"), utf16),
            ResultItem::new(
                format!("U+{padded} JavaScript 转义"),
                unicode_escape(&ch.to_string()),
            ),
            ResultItem::new(format!("U+{padded} HTML 实体"), format!("&#x{hex};")),
        ]);
    }
    Ok(out)
}
