use crate::core::ResultItem;
use regex_lite::Regex;
use std::sync::OnceLock;
use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset};
#[derive(Debug, Clone)]
pub struct ParsedDate {
    pub date: OffsetDateTime,
    pub format: String,
    pub precision: String,
    pub alternatives: Vec<ParsedDate>,
}
fn local_offset() -> UtcOffset {
    UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC)
}
fn date(y: i32, m: u8, d: u8, h: u8, mi: u8, s: u8) -> Option<OffsetDateTime> {
    if !(1..=9999).contains(&y) {
        return None;
    }
    let month = Month::try_from(m).ok()?;
    Some(
        PrimitiveDateTime::new(
            Date::from_calendar_date(y, month, d).ok()?,
            Time::from_hms(h, mi, s).ok()?,
        )
        .assume_offset(local_offset()),
    )
}
fn year(s: &str) -> Option<i32> {
    let n: i32 = s.parse().ok()?;
    Some(if s.len() <= 2 {
        if n < 69 { 2000 + n } else { 1900 + n }
    } else {
        n
    })
}
fn parsed(date: OffsetDateTime, format: &str, precision: &str) -> ParsedDate {
    ParsedDate {
        date,
        format: format.into(),
        precision: precision.into(),
        alternatives: vec![],
    }
}
fn millis(v: i128) -> Option<OffsetDateTime> {
    OffsetDateTime::from_unix_timestamp_nanos(v * 1_000_000).ok()
}
fn pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(||Regex::new(r"^(\d{1,4})([-/.])(\d{1,2})[-/.](\d{1,4})(?:[ T](\d{1,2})(?::(\d{2}))?(?::(\d{2}))?)?$").unwrap())
}
pub fn parse(raw: &str, order: &str) -> Option<ParsedDate> {
    let s: String = raw
        .trim()
        .chars()
        .map(|c| {
            let cp = c as u32;
            for base in [0x660, 0x6f0, 0x966, 0x9e6, 0xe50, 0xff10] {
                if (base..base + 10).contains(&cp) {
                    return char::from_u32(48 + cp - base).unwrap();
                }
            }
            c
        })
        .collect();
    if s.is_empty() || s.len() > 640 {
        return None;
    }
    if let Ok(d) = OffsetDateTime::parse(
        &s.to_uppercase(),
        &time::format_description::well_known::Rfc3339,
    ) {
        return Some(parsed(d, "ISO 8601", "second"));
    }
    for prefix in [
        "filetime",
        "excel1900",
        "excel1904",
        "ft",
        "oa",
        "ole",
        "exc",
        "exf",
    ] {
        if let Some(tail) = s.to_lowercase().strip_prefix(prefix) {
            let n = tail.trim_start_matches([' ', ':', '=']).trim();
            let ms = if matches!(prefix, "ft" | "filetime") {
                n.parse::<i128>().ok()?.checked_sub(116444736000000000)? / 10000
            } else {
                let offset = if matches!(prefix, "exf" | "excel1904") {
                    24107.
                } else {
                    25569.
                };
                ((n.parse::<f64>().ok()? - offset) * 86400000.).trunc() as i128
            };
            return millis(ms).map(|d| parsed(d, prefix, "second"));
        }
    }
    if s.bytes().all(|c| c.is_ascii_digit()) {
        if [10, 13].contains(&s.len()) {
            let n: i128 = s.parse().ok()?;
            return millis(if s.len() == 10 { n * 1000 } else { n })
                .map(|d| parsed(d, "timestamp", "second"));
        }
        let ymd = match s.len() {
            4 => {
                let y: i32 = s.parse().ok()?;
                if !(1000..=2999).contains(&y) {
                    return None;
                }
                Some((y, 1, 1, "YYYY", "year"))
            }
            6 => Some((
                year(&s[..2])?,
                s[2..4].parse().ok()?,
                s[4..6].parse().ok()?,
                "YYMMDD",
                "day",
            )),
            8 => Some((
                s[..4].parse().ok()?,
                s[4..6].parse().ok()?,
                s[6..8].parse().ok()?,
                "YYYYMMDD",
                "day",
            )),
            _ => None,
        };
        if let Some((y, m, d, f, p)) = ymd {
            return date(y, m, d, 0, 0, 0).map(|d| parsed(d, f, p));
        }
    }
    if let Some(c) = pattern().captures(&s) {
        if c.get(2)?.as_str().chars().next()?
            != s.chars().filter(|c| matches!(c, '-' | '/' | '.')).nth(1)?
        {
            return None;
        }
        let tokens = [c.get(1)?.as_str(), c.get(3)?.as_str(), c.get(4)?.as_str()];
        let h = c.get(5).map_or(Some(0), |v| v.as_str().parse().ok())?;
        let mi = c.get(6).map_or(Some(0), |v| v.as_str().parse().ok())?;
        let se = c.get(7).map_or(Some(0), |v| v.as_str().parse().ok())?;
        let orders: Vec<&str> = if tokens[0].len() == 4 {
            if order == "YDM" {
                vec!["YDM", "YMD"]
            } else {
                vec!["YMD", "YDM"]
            }
        } else if tokens[2].len() == 4 {
            if order == "MDY" {
                vec!["MDY", "DMY"]
            } else {
                vec!["DMY", "MDY"]
            }
        } else {
            let mut os = vec![order];
            os.extend(
                ["YMD", "YDM", "DMY", "MDY"]
                    .into_iter()
                    .filter(|o| *o != order),
            );
            os
        };
        let mut out = Vec::new();
        for o in orders {
            let Some(yi) = o.find('Y') else { continue };
            let Some(mi_index) = o.find('M') else {
                continue;
            };
            let Some(di) = o.find('D') else { continue };
            if let (Some(y), Ok(m), Ok(d)) = (
                year(tokens[yi]),
                tokens[mi_index].parse(),
                tokens[di].parse(),
            ) {
                if let Some(d) = date(y, m, d, h, mi, se) {
                    if !out.iter().any(|p: &ParsedDate| p.date == d) {
                        let f = o
                            .chars()
                            .enumerate()
                            .map(|(i, c)| {
                                if c == 'Y' {
                                    if tokens[i].len() == 2 {
                                        "YY".into()
                                    } else {
                                        "YYYY".into()
                                    }
                                } else {
                                    format!("{c}{c}")
                                }
                            })
                            .collect::<Vec<String>>()
                            .join(c.get(2)?.as_str());
                        out.push(parsed(d, &f, "day"));
                    }
                }
            }
        }
        if !out.is_empty() {
            let mut p = out.remove(0);
            p.alternatives = out;
            return Some(p);
        }
    }
    let asian = s
        .replace(['年', '년', '月', '월'], "-")
        .replace(['日', '일'], "");
    let parts: Vec<_> = asian
        .trim_end_matches('-')
        .split('-')
        .map(str::trim)
        .collect();
    if s.contains(['年', '년']) && (1..=3).contains(&parts.len()) {
        let y = year(parts[0])?;
        let m = parts.get(1).map_or(Some(1), |s| s.parse().ok())?;
        let d = parts.get(2).map_or(Some(1), |s| s.parse().ok())?;
        return date(y, m, d, 0, 0, 0).map(|d| {
            parsed(
                d,
                "年/月/日",
                match parts.len() {
                    1 => "year",
                    2 => "month",
                    _ => "day",
                },
            )
        });
    }
    if parts.len() == 2 && parts[0].len() == 4 {
        if let (Some(y), Ok(m)) = (year(parts[0]), parts[1].parse()) {
            return date(y, m, 1, 0, 0, 0).map(|d| parsed(d, "YYYY-MM", "month"));
        }
    }
    if s.chars().any(|c| c.is_ascii_digit())
        && !s.contains(['@', '?', '#', '\\', ':'])
        && s.chars().any(char::is_alphabetic)
    {
        use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};
        static MONTHS: OnceLock<Vec<(String, u8)>> = OnceLock::new();
        let months = MONTHS
            .get_or_init(|| serde_json::from_str(include_str!("../assets/months.json")).unwrap());
        let clean = s
            .nfkd()
            .filter(|c| !is_combining_mark(*c))
            .flat_map(char::to_lowercase)
            .filter(|c| !['.', ',', '\u{200e}', '\u{200f}'].contains(c))
            .collect::<String>()
            .replace('/', " ");
        static WORDS: OnceLock<Regex> = OnceLock::new();
        let words = WORDS.get_or_init(|| {
            Regex::new(r"\b(?:de|del|of|г|года|roku)\b|(?:st|nd|rd|th)\b").unwrap()
        });
        let clean = words.replace_all(&clean, " ");
        for (name, m) in months {
            if let Some(at) = clean.find(name) {
                let before = clean[..at].trim();
                let after = clean[at + name.len()..].trim();
                let rest = format!("{before} {after}");
                if !rest
                    .chars()
                    .all(|c| c.is_ascii_digit() || c.is_whitespace() || c == '-')
                {
                    continue;
                }
                let nums: Vec<_> = rest
                    .split(|c: char| !c.is_ascii_digit())
                    .filter(|s| !s.is_empty())
                    .collect();
                let ymd = if nums.len() == 1 && nums[0].len() <= 2 {
                    Some((
                        OffsetDateTime::now_utc().year(),
                        nums[0].parse().ok()?,
                        "月份名称 DD",
                        "day",
                    ))
                } else if nums.len() == 1 && nums[0].len() == 4 {
                    Some((year(nums[0])?, 1, "月份名称 YYYY", "month"))
                } else if nums.len() == 2 {
                    let yi = nums.iter().position(|s| s.len() == 4).unwrap_or(1);
                    Some((
                        year(nums[yi])?,
                        nums[1 - yi].parse().ok()?,
                        "本地化月份日期",
                        "day",
                    ))
                } else {
                    None
                };
                if let Some((y, d, f, p)) = ymd {
                    if let Some(d) = date(y, *m, d, 0, 0, 0) {
                        return Some(parsed(d, f, p));
                    }
                }
            }
        }
    }
    None
}
pub fn recognized(d: OffsetDateTime) -> String {
    let d = d.to_offset(local_offset());
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        d.year(),
        d.month() as u8,
        d.day(),
        d.hour(),
        d.minute(),
        d.second()
    )
}
pub fn format_pattern(d: OffsetDateTime, pattern: &str, utc: bool) -> String {
    let d = d.to_offset(if utc { UtcOffset::UTC } else { local_offset() });
    let mut out = String::new();
    let mut rest = pattern;
    while !rest.is_empty() {
        let mut found = false;
        for token in [
            "YYYY", "SSS", "YY", "MM", "DD", "HH", "mm", "ss", "WW", "X", "x", "W",
        ] {
            if let Some(tail) = rest.strip_prefix(token) {
                let value = match token {
                    "YYYY" => format!("{}", d.year()),
                    "YY" => format!("{:02}", d.year() % 100),
                    "MM" => format!("{:02}", d.month() as u8),
                    "DD" => format!("{:02}", d.day()),
                    "HH" => format!("{:02}", d.hour()),
                    "mm" => format!("{:02}", d.minute()),
                    "ss" => format!("{:02}", d.second()),
                    "SSS" => format!("{:03}", d.millisecond()),
                    "X" => d.unix_timestamp().to_string(),
                    "x" => (d.unix_timestamp_nanos() / 1_000_000).to_string(),
                    "WW" => format!("{:02}", d.iso_week()),
                    _ => d.iso_week().to_string(),
                };
                out.push_str(&value);
                rest = tail;
                found = true;
                break;
            }
        }
        if !found {
            let c = rest.chars().next().unwrap();
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    out
}
pub fn results(raw: &str, order: &str, custom: &str) -> Result<Vec<ResultItem>, String> {
    let p =
        parse(raw, order).ok_or("无法识别日期/时间；特殊格式可用 ft: / oa: / exc: / exf: 前缀")?;
    let d = p.date;
    let ms = d.unix_timestamp_nanos() / 1_000_000;
    let iso = d
        .to_offset(UtcOffset::UTC)
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|e| e.to_string())?;
    let iso = if !iso.contains('.') {
        iso.replace('Z', ".000Z")
    } else {
        iso
    };
    let utc = d
        .to_offset(UtcOffset::UTC)
        .format(&time::format_description::well_known::Rfc2822)
        .map_err(|e| e.to_string())?
        .replace("+0000", "GMT");
    let mut out = vec![
        ResultItem::new("Unix 秒", d.unix_timestamp().to_string()),
        ResultItem::new("Unix 毫秒", ms.to_string()),
        ResultItem::new("本地日期时间", recognized(d)),
        ResultItem::new("ISO 8601", iso),
        ResultItem::new("UTC", utc),
    ];
    for a in p.alternatives {
        out.push(ResultItem::new(
            format!("其他识别 [{}]", a.format),
            recognized(a.date),
        ));
    }
    let days = ms as f64 / 86400000.;
    out.extend([
        ResultItem::new(
            "Windows File Time",
            (ms * 10000 + 116444736000000000_i128).to_string(),
        ),
        ResultItem::new("OLE Automation Date", trim10(days + 25569.)),
        ResultItem::new("Excel 1900", trim10(days + 25569.)),
        ResultItem::new("Excel 1904", trim10(days + 24107.)),
        ResultItem::new(
            "ISO 周数",
            format!("{}-W{:02}", d.to_iso_week_date().0, d.iso_week()),
        ),
    ]);
    for line in custom.lines() {
        if let Some((name, pattern)) = line.split_once('=') {
            let pattern = pattern.trim();
            let utc = pattern.to_uppercase().starts_with("UTC:");
            out.push(ResultItem::new(
                name.trim(),
                format_pattern(d, if utc { &pattern[4..] } else { pattern }, utc),
            ));
        }
    }
    Ok(out)
}
fn trim10(n: f64) -> String {
    let s = format!("{n:.10}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}
