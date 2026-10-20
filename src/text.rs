use crate::core::ResultItem;
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD},
};
fn words(s: &str) -> Vec<&str> {
    s.trim()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect()
}
fn capital(s: &str) -> String {
    let mut chars = s.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase())
        .unwrap_or_default()
}
pub fn case_results(q: &str) -> Vec<ResultItem> {
    let lower = q.to_lowercase();
    let mut previous = None;
    let mut title = String::new();
    for c in lower.chars() {
        let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
        let boundary = previous.is_some_and(word) != word(c);
        if boundary && c.is_alphabetic() {
            title.extend(c.to_uppercase());
        } else {
            title.push(c);
        }
        previous = Some(c);
    }
    let mut chars = lower.chars();
    let sentence = chars
        .next()
        .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
        .unwrap_or_default();
    let words = words(q);
    vec![
        ResultItem::new("全大写", q.to_uppercase()),
        ResultItem::new("全小写", &lower),
        ResultItem::new("首字母大写", title),
        ResultItem::new("Sentence case", sentence),
        ResultItem::new(
            "大小写反转",
            q.chars()
                .map(|c| {
                    let s = c.to_string();
                    if s == s.to_uppercase() {
                        s.to_lowercase()
                    } else {
                        s.to_uppercase()
                    }
                })
                .collect::<String>(),
        ),
        ResultItem::new(
            "camelCase",
            words
                .iter()
                .enumerate()
                .map(|(i, w)| if i == 0 { w.to_lowercase() } else { capital(w) })
                .collect::<String>(),
        ),
        ResultItem::new(
            "PascalCase",
            words.iter().map(|w| capital(w)).collect::<String>(),
        ),
        ResultItem::new(
            "snake_case",
            words
                .iter()
                .map(|w| w.to_lowercase())
                .collect::<Vec<_>>()
                .join("_"),
        ),
        ResultItem::new(
            "kebab-case",
            words
                .iter()
                .map(|w| w.to_lowercase())
                .collect::<Vec<_>>()
                .join("-"),
        ),
    ]
}
pub fn text_results(q: &str) -> Vec<ResultItem> {
    let lines: Vec<_> = q
        .split('\n')
        .map(|s| s.strip_suffix('\r').unwrap_or(s))
        .collect();
    let mut sorted = lines.clone();
    sorted.sort_by(|a, b| crate::countries::locale_compare(a, b));
    let mut seen = std::collections::HashSet::new();
    let unique = lines
        .iter()
        .copied()
        .filter(|s| seen.insert(*s))
        .collect::<Vec<_>>()
        .join("\n");
    let mut merged = String::new();
    let mut space = false;
    for c in q.chars() {
        if c == ' ' || c == '\t' {
            if !space {
                merged.push(' ');
            }
            space = true;
        } else {
            merged.push(c);
            space = false;
        }
    }
    vec![
        ResultItem::new("Trim", q.trim()),
        ResultItem::new("合并多余空格", merged.trim()),
        ResultItem::new(
            "删除空行",
            lines
                .iter()
                .filter(|s| !s.trim().is_empty())
                .copied()
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        ResultItem::new("行去重", unique),
        ResultItem::new("行排序", sorted.join("\n")),
        ResultItem::new(
            "统计",
            format!(
                "字符 {} · 行 {} · 单词 {}",
                q.encode_utf16().count(),
                lines.len(),
                q.split_whitespace().count()
            ),
        ),
    ]
}
pub fn base64_decode(q: &str) -> Option<String> {
    let raw: String = q
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\u{c}' | '\r' | ' '))
        .collect();
    let bytes = STANDARD
        .decode(&raw)
        .or_else(|_| STANDARD_NO_PAD.decode(&raw))
        .ok()?;
    String::from_utf8(bytes).ok()
}
pub fn base64_results(q: &str) -> Vec<ResultItem> {
    let decoded = base64_decode(q)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "（无法作为 UTF-8 文本解码）".into());
    vec![
        ResultItem::new("Base64 编码", STANDARD.encode(q)),
        ResultItem::new("Base64 解码", decoded),
    ]
}
