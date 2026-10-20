use crate::core::normalize_search;
use serde_json::Value;
use std::{cmp::Ordering, sync::OnceLock};

struct Entry {
    country: Value,
    fields: Vec<String>,
}
fn data() -> &'static Vec<Entry> {
    static DATA: OnceLock<Vec<Entry>> = OnceLock::new();
    DATA.get_or_init(|| {
        let data: Value =
            serde_json::from_str(include_str!("../assets/data/countries.json")).unwrap();
        data["countries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|country| {
                let mut fields: Vec<_> = [
                    "nameZh",
                    "officialNameZh",
                    "nameEn",
                    "officialNameEn",
                    "alpha2",
                    "alpha3",
                    "numeric",
                    "ioc",
                    "fifa",
                    "itu",
                ]
                .iter()
                .filter_map(|k| country[k].as_str().map(normalize_search))
                .collect();
                for k in ["callingCodes", "tlds", "aliases"] {
                    for v in country[k].as_array().into_iter().flatten() {
                        if let Some(s) = v.as_str() {
                            fields.push(normalize_search(s));
                            if k == "callingCodes" {
                                fields.push(normalize_search(&format!(
                                    "+{}",
                                    s.trim_start_matches('+')
                                )));
                            }
                        }
                    }
                }
                Entry {
                    country: country.clone(),
                    fields,
                }
            })
            .collect()
    })
}
pub fn scored_matches(q: &str, limit: usize) -> Vec<(Value, f64)> {
    let q = normalize_search(q.trim());
    if q.is_empty() {
        return vec![];
    }
    let mut out: Vec<_> = data()
        .iter()
        .filter_map(|e| {
            let score = e
                .fields
                .iter()
                .map(|f| {
                    if f == &q {
                        1.
                    } else if f.starts_with(&q) {
                        0.9 - ((f.encode_utf16().count() - q.encode_utf16().count()) as f64 * 0.002)
                            .min(0.08)
                    } else if f.contains(&q) {
                        0.72
                    } else {
                        0.
                    }
                })
                .fold(0., f64::max);
            (score > 0.).then_some((e, score))
        })
        .collect();
    out.sort_by(|a, b| {
        b.1.total_cmp(&a.1).then_with(|| {
            locale_compare(
                a.0.country["nameEn"].as_str().unwrap_or(""),
                b.0.country["nameEn"].as_str().unwrap_or(""),
            )
        })
    });
    out.into_iter()
        .take(limit)
        .map(|(e, s)| (e.country.clone(), s))
        .collect()
}
pub fn matches(q: &str) -> Vec<Value> {
    scored_matches(q, 5).into_iter().map(|(c, _)| c).collect()
}
pub fn items(c: &Value) -> Vec<crate::core::ResultItem> {
    use crate::core::ResultItem;
    let mut out = vec![ResultItem::new(
        format!(
            "{} {}",
            c["flagEmoji"].as_str().unwrap_or(""),
            c["nameZh"].as_str().unwrap_or("")
        ),
        c["nameEn"].as_str().unwrap_or(""),
    )];
    for (k, label) in [
        ("alpha2", "ISO 3166-1 Alpha-2"),
        ("alpha3", "ISO 3166-1 Alpha-3"),
        ("numeric", "ISO 3166-1 Numeric"),
        ("ioc", "IOC"),
        ("fifa", "FIFA"),
        ("itu", "ITU"),
    ] {
        if let Some(s) = c[k].as_str().filter(|s| !s.is_empty()) {
            out.push(ResultItem::new(label, s));
        }
    }
    for (k, label, prefix) in [
        ("callingCodes", "国际电话区号", "+"),
        ("tlds", "国家/地区顶级域名 ccTLD", ""),
    ] {
        let values = c[k]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|v| format!("{prefix}{}", v.trim_start_matches('+')))
            .collect::<Vec<_>>();
        if !values.is_empty() {
            out.push(ResultItem::new(label, values.join(", ")));
        }
    }
    for (k, label) in [
        ("flagEmoji", "国旗 Emoji"),
        ("flagSvg", "国旗 SVG"),
        ("flagPng250", "国旗 PNG 250px"),
    ] {
        if let Some(s) = c[k].as_str().filter(|s| !s.is_empty()) {
            out.push(ResultItem::new(label, s));
        }
    }
    out
}
pub fn locale_compare(a: &str, b: &str) -> Ordering {
    static COLLATOR: OnceLock<icu_collator::CollatorBorrowed<'static>> = OnceLock::new();
    let collator = COLLATOR.get_or_init(|| {
        let mut name = "en-US".to_string();
        #[cfg(windows)]
        {
            unsafe extern "system" {
                fn GetUserDefaultLocaleName(buffer: *mut u16, length: i32) -> i32;
            }
            let mut buffer = [0u16; 85];
            let length =
                unsafe { GetUserDefaultLocaleName(buffer.as_mut_ptr(), buffer.len() as i32) };
            if length > 1 {
                name = String::from_utf16_lossy(&buffer[..length as usize - 1]);
            }
        }
        let locale = name
            .parse::<icu_locale::Locale>()
            .unwrap_or(icu_locale::locale!("en-US"));
        icu_collator::Collator::try_new(locale.into(), Default::default()).unwrap_or_else(|_| {
            icu_collator::Collator::try_new(Default::default(), Default::default()).unwrap()
        })
    });
    collator.compare(a, b)
}
