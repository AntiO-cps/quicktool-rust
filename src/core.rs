//! Native operations. JavaScript in reference/ is test input only.
use num_bigint::BigInt;
use num_traits::Signed;
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResultItem {
    pub title: String,
    pub value: String,
}
impl ResultItem {
    pub fn new(title: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            value: value.into(),
        }
    }
}
pub fn normalize_search(s: &str) -> String {
    s.nfkc()
        .flat_map(char::to_lowercase)
        .filter(|c| !c.is_whitespace() && !"_-–—()（）[]【】.,，。'\"·".contains(*c))
        .collect()
}
pub fn base_results(raw: &str) -> Result<Vec<ResultItem>, String> {
    let cleaned = raw.trim().replace('_', "");
    let raw = cleaned.as_str();
    let (negative, s) = if let Some(s) = raw.strip_prefix('-') {
        (true, s)
    } else {
        (false, raw.strip_prefix('+').unwrap_or(raw))
    };
    let (radix, digits) = if let Some((r, d)) = s.split_once(':') {
        let r: u32 = r.trim().parse().map_err(|_| "无效进制")?;
        if ![2, 8, 10, 16].contains(&r) {
            return Err("无效进制".into());
        }
        (r, d.trim())
    } else if s.starts_with("0x") || s.starts_with("0X") {
        (16, &s[2..])
    } else if s.starts_with("0b") || s.starts_with("0B") {
        (2, &s[2..])
    } else if s.starts_with("0o") || s.starts_with("0O") {
        (8, &s[2..])
    } else {
        (10, s)
    };
    if !digits.chars().all(|c| c.is_digit(radix)) {
        return Err("无法识别整数".into());
    }
    let mut n = BigInt::parse_bytes(digits.as_bytes(), radix).ok_or("无法识别整数")?;
    if negative {
        n = -n;
    }
    let sign = if n.is_negative() { "-" } else { "" };
    Ok([
        (2, "0b", "二进制 BIN"),
        (8, "0o", "八进制 OCT"),
        (10, "", "十进制 DEC"),
        (16, "0x", "十六进制 HEX"),
    ]
    .map(|(b, prefix, title)| {
        ResultItem::new(
            title,
            format!("{sign}{prefix}{}", n.abs().to_str_radix(b).to_uppercase()),
        )
    })
    .to_vec())
}
pub fn to_full_width(s: &str, space: bool) -> String {
    s.chars()
        .map(|c| match c {
            ' ' if space => '\u{3000}',
            '!'..='~' => char::from_u32(c as u32 + 0xfee0).unwrap(),
            _ => c,
        })
        .collect()
}
pub fn to_half_width(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{3000}' => ' ',
            '\u{ff01}'..='\u{ff5e}' => char::from_u32(c as u32 - 0xfee0).unwrap(),
            _ => c,
        })
        .collect()
}
pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            use std::fmt::Write;
            write!(out, "%{b:02X}").unwrap();
        }
    }
    out
}
pub fn percent_decode(s: &str) -> Result<String, String> {
    let mut out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let code = bytes.get(i + 1..i + 3).ok_or("无效的百分号编码")?;
            let text = std::str::from_utf8(code).map_err(|_| "无效的百分号编码")?;
            out.push(u8::from_str_radix(text, 16).map_err(|_| "无效的百分号编码")?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| "无效的百分号编码".into())
}
pub fn unicode_escape(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c as u32 <= 0xffff {
                format!("\\u{:04X}", c as u32)
            } else {
                format!("\\u{{{:X}}}", c as u32)
            }
        })
        .collect()
}
pub fn html_encode(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
pub fn unicode_unescape(s: &str) -> String {
    use std::sync::OnceLock;
    static RE: OnceLock<regex_lite::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex_lite::Regex::new(r"\\u\{([0-9a-fA-F]{1,6})\}|\\U([0-9a-fA-F]{8})|\\u([0-9a-fA-F]{4})")
            .unwrap()
    });
    let mut units = Vec::new();
    let mut at = 0;
    for capture in re.captures_iter(s) {
        let m = capture.get(0).unwrap();
        units.extend(s[at..m.start()].encode_utf16());
        let group = capture
            .get(1)
            .or_else(|| capture.get(2))
            .or_else(|| capture.get(3))
            .unwrap();
        let cp = u32::from_str_radix(group.as_str(), 16).unwrap();
        if capture.get(3).is_some() {
            units.push(cp as u16);
        } else if let Some(c) = char::from_u32(cp) {
            let mut buf = [0u16; 2];
            units.extend_from_slice(c.encode_utf16(&mut buf));
        } else {
            units.push(0xfffd);
        }
        at = m.end();
    }
    units.extend(s[at..].encode_utf16());
    String::from_utf16_lossy(&units)
}
pub fn html_decode(s: &str) -> String {
    use std::sync::OnceLock;
    static RE: OnceLock<regex_lite::Regex> = OnceLock::new();
    let re =
        RE.get_or_init(|| regex_lite::Regex::new(r"(?i)(&(#x[0-9a-f]+|#[0-9]+|[a-z]+);)").unwrap());
    re.replace_all(s, |caps: &regex_lite::Captures<'_>| {
        let key = caps[2].to_lowercase();
        match key.as_str() {
            "amp" => "&".into(),
            "lt" => "<".into(),
            "gt" => ">".into(),
            "quot" => "\"".into(),
            "apos" => "'".into(),
            "nbsp" => "\u{a0}".into(),
            _ => {
                let cp = if let Some(s) = key.strip_prefix("#x") {
                    u32::from_str_radix(s, 16).ok()
                } else if let Some(s) = key.strip_prefix('#') {
                    s.parse().ok()
                } else {
                    None
                };
                cp.filter(|c| *c <= 0x10ffff)
                    .map(|cp| char::from_u32(cp).unwrap_or('\u{fffd}').to_string())
                    .unwrap_or_else(|| caps[0].to_string())
            }
        }
    })
    .into_owned()
}
pub fn json_escape(s: &str) -> String {
    let text = serde_json::to_string(s).unwrap();
    text[1..text.len() - 1].to_string()
}
pub fn escape_results(s: &str) -> Vec<ResultItem> {
    let mut out = Vec::new();
    let mut push = |title: &str, value: String| {
        if !out.iter().any(|r: &ResultItem| r.value == value) {
            out.push(ResultItem::new(title, value));
        }
    };
    if s.as_bytes()
        .windows(3)
        .any(|s| s[0] == b'%' && s[1].is_ascii_hexdigit() && s[2].is_ascii_hexdigit())
    {
        if let Ok(decoded) = percent_decode(s) {
            push("百分号 / URL 解码", decoded);
        }
    }
    if s.contains("\\u{")
        || s.as_bytes()
            .windows(6)
            .any(|s| s.starts_with(b"\\u") && s[2..].iter().all(u8::is_ascii_hexdigit))
        || s.as_bytes()
            .windows(10)
            .any(|s| s.starts_with(b"\\U") && s[2..].iter().all(u8::is_ascii_hexdigit))
    {
        push("Unicode 转义解码", unicode_unescape(s));
    }
    if s.contains('&') && s.contains(';') {
        let decoded = html_decode(s);
        push("HTML 实体解码", decoded);
    }
    push("百分号 / URL 编码", percent_encode(s));
    push("Unicode 转义编码", unicode_escape(s));
    push("HTML 实体编码", html_encode(s));
    push("JSON 字符串转义", json_escape(s));
    out
}
pub fn code_points(raw: &str) -> Option<Vec<char>> {
    let tokens: Vec<_> = raw
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|s| !s.is_empty())
        .collect();
    let mut out = Vec::new();
    for token in &tokens {
        let lower = token.to_ascii_lowercase();
        let digits = if let Some(s) = lower
            .strip_prefix("u+")
            .or_else(|| lower.strip_prefix("0x"))
        {
            (1..=6).contains(&s.len()).then_some(s)
        } else if let Some(s) = lower.strip_prefix("\\u{").and_then(|s| s.strip_suffix('}')) {
            (1..=6).contains(&s.len()).then_some(s)
        } else if let Some(s) = lower.strip_prefix("\\u") {
            [4, 8].contains(&s.len()).then_some(s)
        } else {
            (tokens.len() == 1 && (4..=6).contains(&lower.len())).then_some(lower.as_str())
        }?;
        if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        out.push(char::from_u32(u32::from_str_radix(digits, 16).ok()?)?);
    }
    (!out.is_empty()).then_some(out)
}
