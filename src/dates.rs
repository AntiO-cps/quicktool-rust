use crate::core::ResultItem;
use regex_lite::Regex;
use std::sync::OnceLock;
use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset};
type Tz = tz::TimeZoneRef<'static>;
#[derive(Debug, Clone)]
pub struct ParsedDate {
    pub date: OffsetDateTime,
    pub format: String,
    pub precision: String,
    pub alternatives: Vec<ParsedDate>,
}
fn lookup_zone(name: &str) -> Result<Tz, String> {
    tzdb::tz_by_name(name).ok_or_else(|| format!("IANA时区无效：{name}"))
}
fn zone_from_source(source: impl FnOnce() -> Result<String, String>) -> Result<Tz, String> {
    lookup_zone(&source()?)
}
#[derive(Default)]
struct SystemZoneCache {
    entry: Option<(std::time::Duration, Tz)>,
}
impl SystemZoneCache {
    fn resolve(
        &mut self,
        now: std::time::Duration,
        source: impl FnOnce() -> Result<String, String>,
    ) -> Result<Tz, String> {
        if let Some((loaded, zone)) = self.entry {
            if now >= loaded && now - loaded < std::time::Duration::from_secs(1) {
                return Ok(zone);
            }
        }
        self.entry = None;
        let zone = zone_from_source(source)?;
        self.entry = Some((now, zone));
        Ok(zone)
    }
}
fn system_zone() -> Result<Tz, String> {
    // One-second TTL avoids per-keystroke WinRT work without permanently pinning the system zone.
    static CACHE: OnceLock<std::sync::Mutex<SystemZoneCache>> = OnceLock::new();
    static ORIGIN: OnceLock<std::time::Instant> = OnceLock::new();
    let now = ORIGIN.get_or_init(std::time::Instant::now).elapsed();
    CACHE
        .get_or_init(Default::default)
        .lock()
        .map_err(|_| "时区缓存不可用".to_string())?
        .resolve(now, || {
            iana_time_zone::get_timezone().map_err(|e| format!("无法读取系统IANA时区：{e}"))
        })
}
fn offset_at(d: OffsetDateTime, zone: Tz) -> Result<UtcOffset, String> {
    let seconds = zone
        .find_local_time_type(d.unix_timestamp())
        .map_err(|e| e.to_string())?
        .ut_offset();
    UtcOffset::from_whole_seconds(seconds).map_err(|e| e.to_string())
}
fn local(d: OffsetDateTime, zone: Tz) -> Result<OffsetDateTime, String> {
    d.checked_to_offset(offset_at(d, zone)?)
        .ok_or_else(|| "日期超出可表示范围".into())
}
fn date(y: i32, m: u8, d: u8, h: u8, mi: u8, s: u8, zone: Tz) -> Option<OffsetDateTime> {
    if !(1..=9999).contains(&y) {
        return None;
    }
    let wall = PrimitiveDateTime::new(
        Date::from_calendar_date(y, Month::try_from(m).ok()?, d).ok()?,
        Time::from_hms(h, mi, s).ok()?,
    )
    .assume_utc();
    let mut candidates = Vec::new();
    let mut offsets = zone
        .local_time_types()
        .iter()
        .map(|t| t.ut_offset())
        .collect::<Vec<_>>();
    for days in [-2, 0, 2] {
        offsets.push(
            offset_at(wall + time::Duration::days(days), zone)
                .ok()?
                .whole_seconds(),
        );
    }
    offsets.sort_unstable();
    offsets.dedup();
    for seconds in offsets {
        let candidate = wall - time::Duration::seconds(seconds as i64);
        if offset_at(candidate, zone).ok()?.whole_seconds() == seconds {
            candidates.push(candidate);
        }
    }
    let instant = if let Some(first) = candidates.into_iter().min() {
        first
    } else {
        // JS compatible disambiguation: preserve the offset before a forward gap.
        wall - time::Duration::seconds(
            offset_at(wall - time::Duration::days(2), zone)
                .ok()?
                .whole_seconds() as i64,
        )
    };
    let projected = local(instant, zone).ok()?;
    if projected.year() != y || projected.month() as u8 != m || projected.day() != d {
        return None;
    }
    Some(projected)
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
    if v.unsigned_abs() > 8_640_000_000_000_000 {
        return None;
    }
    OffsetDateTime::from_unix_timestamp_nanos(v.checked_mul(1_000_000)?).ok()
}
fn pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(||Regex::new(r"^(\d{1,4})([-/.])(\d{1,2})[-/.](\d{1,4})(?:[ T](\d{1,2})(?::(\d{2}))?(?::(\d{2}))?)?$").unwrap())
}
pub fn parse(raw: &str, order: &str) -> Option<ParsedDate> {
    parse_with_zone(raw, order, system_zone().ok()?)
}
/// Parse using an explicit IANA zone without changing process or system timezone.
pub fn parse_in_zone(raw: &str, order: &str, zone: &str) -> Option<ParsedDate> {
    parse_with_zone(raw, order, lookup_zone(zone).ok()?)
}
fn parse_with_zone(raw: &str, order: &str, zone: Tz) -> Option<ParsedDate> {
    let parsed = parse_raw_with_zone(raw, order, zone)?;
    for candidate in std::iter::once(&parsed).chain(parsed.alternatives.iter()) {
        candidate.date.checked_to_offset(UtcOffset::UTC)?;
        candidate
            .date
            .checked_to_offset(offset_at(candidate.date, zone).ok()?)?;
    }
    Some(parsed)
}
fn parse_raw_with_zone(raw: &str, order: &str, zone: Tz) -> Option<ParsedDate> {
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
    static ISO: OnceLock<Regex> = OnceLock::new();
    let iso=ISO.get_or_init(||Regex::new(r"(?i)^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})(?::(\d{2})(\.\d{1,3})?)?(Z|[+-]\d{2}:?\d{2})$").unwrap());
    if let Some(c) = iso.captures(&s) {
        date(
            c[1].parse().ok()?,
            c[2].parse().ok()?,
            c[3].parse().ok()?,
            c[4].parse().ok()?,
            c[5].parse().ok()?,
            c.get(6).map_or("0", |v| v.as_str()).parse().ok()?,
            zone,
        )?;
        let mut offset = c[8].to_uppercase();
        if offset.len() == 5 {
            offset.insert(3, ':');
        }
        let normalized = format!(
            "{}-{}-{}T{}:{}:{}{}{}",
            &c[1],
            &c[2],
            &c[3],
            &c[4],
            &c[5],
            c.get(6).map_or("00", |v| v.as_str()),
            c.get(7).map_or("", |v| v.as_str()),
            offset
        );
        let d = OffsetDateTime::parse(&normalized, &time::format_description::well_known::Rfc3339)
            .ok()?;
        return Some(parsed(d, "ISO 8601", "second"));
    }
    static SPECIAL: OnceLock<Regex> = OnceLock::new();
    let special = SPECIAL.get_or_init(|| {
        Regex::new(
            r"(?i)^(ft|filetime|oa|ole|exc|excel1900|exf|excel1904)\s*[:=]?\s*(-?\d+(?:\.\d+)?)$",
        )
        .unwrap()
    });
    if let Some(c) = special.captures(&s) {
        let prefix = c[1].to_lowercase();
        let n = &c[2];
        let ms = if matches!(prefix.as_str(), "ft" | "filetime") {
            (n.parse::<i128>().ok()? / 10000).checked_sub(11644473600000)?
        } else {
            let offset = if matches!(prefix.as_str(), "exf" | "excel1904") {
                24107.
            } else {
                25569.
            };
            let value = (n.parse::<f64>().ok()? - offset) * 86400000.;
            if !value.is_finite() {
                return None;
            }
            value.trunc() as i128
        };
        return millis(ms).map(|d| parsed(d, &prefix, "second"));
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
            return date(y, m, d, 0, 0, 0, zone).map(|d| parsed(d, f, p));
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
                if let Some(d) = date(y, m, d, h, mi, se, zone) {
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
    static ASIAN: OnceLock<Regex> = OnceLock::new();
    let asian=ASIAN.get_or_init(||Regex::new(r"^(\d{4})\s*[年년]\s*(?:(\d{1,2})\s*[月월]\s*(?:(\d{1,2})\s*[日일])?)?(?:\s+(\d{1,2}):(\d{2})(?::(\d{2}))?)?$").unwrap());
    if let Some(c) = asian.captures(&s) {
        let get = |i, default| c.get(i).map_or(default, |v| v.as_str()).parse::<u8>().ok();
        return date(
            year(&c[1])?,
            get(2, "1")?,
            get(3, "1")?,
            get(4, "0")?,
            get(5, "0")?,
            get(6, "0")?,
            zone,
        )
        .map(|d| {
            parsed(
                d,
                "年/月/日",
                if c.get(3).is_some() {
                    "day"
                } else if c.get(2).is_some() {
                    "month"
                } else {
                    "year"
                },
            )
        });
    }
    static MONTH_ONLY: OnceLock<Regex> = OnceLock::new();
    let month_only = MONTH_ONLY.get_or_init(|| Regex::new(r"^(\d{4})[-/.](\d{1,2})$").unwrap());
    if let Some(c) = month_only.captures(&s) {
        return date(year(&c[1])?, c[2].parse().ok()?, 1, 0, 0, 0, zone)
            .map(|d| parsed(d, "YYYY-MM", "month"));
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
                        local(OffsetDateTime::now_utc(), zone).ok()?.year(),
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
                    if let Some(d) = date(y, *m, d, 0, 0, 0, zone) {
                        return Some(parsed(d, f, p));
                    }
                }
            }
        }
    }
    None
}
fn in_js_range(d: OffsetDateTime) -> bool {
    d.unix_timestamp_nanos().unsigned_abs() <= 8_640_000_000_000_000_000_000
}
fn iso_week(d: OffsetDateTime, zone: Tz) -> Result<(String, String), String> {
    let mut d = local(d, zone)?.date();
    if (0..100).contains(&d.year()) {
        d = Date::from_calendar_date(d.year() + 1900, d.month(), 1).map_err(|e| e.to_string())?
            + time::Duration::days(d.day() as i64 - 1);
    }
    // Preserve JS Date.UTC/TimeClip behavior at the two extreme Date endpoints.
    let midnight = PrimitiveDateTime::new(d, Time::MIDNIGHT).assume_utc();
    if !in_js_range(midnight) {
        return Ok(("NaN".into(), "NaN".into()));
    }
    let thursday = midnight + time::Duration::days(4 - d.weekday().number_from_monday() as i64);
    if !in_js_range(thursday) {
        return Ok(("NaN".into(), "NaN".into()));
    }
    let year = thursday.year();
    let jan1 = Date::from_calendar_date(year, Month::January, 1)
        .map_err(|e| e.to_string())?
        .midnight()
        .assume_utc();
    let week = if in_js_range(jan1) {
        thursday.ordinal().div_ceil(7).to_string()
    } else {
        "NaN".into()
    };
    Ok((year.to_string(), week))
}
pub fn recognized(d: OffsetDateTime) -> String {
    system_zone()
        .and_then(|zone| recognized_with_zone(d, zone))
        .unwrap_or_else(|error| error)
}
pub fn recognized_in_zone(d: OffsetDateTime, zone: &str) -> Result<String, String> {
    recognized_with_zone(d, lookup_zone(zone)?)
}
fn recognized_with_zone(d: OffsetDateTime, zone: Tz) -> Result<String, String> {
    let d = local(d, zone)?;
    Ok(format!(
        "{}-{:02}-{:02} {:02}:{:02}:{:02}",
        d.year(),
        d.month() as u8,
        d.day(),
        d.hour(),
        d.minute(),
        d.second()
    ))
}
pub fn format_pattern(d: OffsetDateTime, pattern: &str, utc: bool) -> String {
    system_zone()
        .and_then(|zone| format_with_zone(d, pattern, utc, zone))
        .unwrap_or_else(|error| error)
}
pub fn format_pattern_in_zone(
    d: OffsetDateTime,
    pattern: &str,
    utc: bool,
    zone: &str,
) -> Result<String, String> {
    format_with_zone(d, pattern, utc, lookup_zone(zone)?)
}
fn format_with_zone(
    d: OffsetDateTime,
    pattern: &str,
    utc: bool,
    zone: Tz,
) -> Result<String, String> {
    let local_week = iso_week(d, zone)?.1;
    let d = if utc {
        d.checked_to_offset(UtcOffset::UTC)
            .ok_or("日期超出可表示范围")?
    } else {
        local(d, zone)?
    };
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
                    "YY" => {
                        let y = d.year().to_string();
                        y.chars()
                            .skip(y.chars().count().saturating_sub(2))
                            .collect()
                    }
                    "MM" => format!("{:02}", d.month() as u8),
                    "DD" => format!("{:02}", d.day()),
                    "HH" => format!("{:02}", d.hour()),
                    "mm" => format!("{:02}", d.minute()),
                    "ss" => format!("{:02}", d.second()),
                    "SSS" => format!("{:03}", d.millisecond()),
                    "X" => d.unix_timestamp().to_string(),
                    "x" => (d.unix_timestamp_nanos() / 1_000_000).to_string(),
                    "WW" => format!("{local_week:0>2}"),
                    _ => local_week.to_string(),
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
    Ok(out)
}
pub fn results(raw: &str, order: &str, custom: &str) -> Result<Vec<ResultItem>, String> {
    results_with_zone(raw, order, custom, system_zone()?)
}
pub fn results_in_zone(
    raw: &str,
    order: &str,
    custom: &str,
    zone: &str,
) -> Result<Vec<ResultItem>, String> {
    results_with_zone(raw, order, custom, lookup_zone(zone)?)
}
fn results_with_zone(
    raw: &str,
    order: &str,
    custom: &str,
    zone: Tz,
) -> Result<Vec<ResultItem>, String> {
    let p = parse_with_zone(raw, order, zone)
        .ok_or("无法识别日期/时间；特殊格式可用 ft: / oa: / exc: / exf: 前缀")?;
    let d = p.date;
    let ms = d.unix_timestamp_nanos() / 1_000_000;
    let universal = d.to_offset(UtcOffset::UTC);
    let iso_year = if (0..=9999).contains(&universal.year()) {
        format!("{:04}", universal.year())
    } else {
        format!(
            "{}{:06}",
            if universal.year() < 0 { "-" } else { "+" },
            universal.year().unsigned_abs()
        )
    };
    let iso = format!(
        "{}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        iso_year,
        universal.month() as u8,
        universal.day(),
        universal.hour(),
        universal.minute(),
        universal.second(),
        universal.millisecond()
    );
    let weekday = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
        [universal.weekday().number_days_from_monday() as usize];
    let month = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ][universal.month() as usize - 1];
    let utc_year = if universal.year() < 0 {
        format!("-{:04}", universal.year().unsigned_abs())
    } else {
        format!("{:04}", universal.year())
    };
    let utc = format!(
        "{weekday}, {:02} {month} {} {:02}:{:02}:{:02} GMT",
        universal.day(),
        utc_year,
        universal.hour(),
        universal.minute(),
        universal.second()
    );
    let mut out = vec![
        ResultItem::new("Unix 秒", d.unix_timestamp().to_string()),
        ResultItem::new("Unix 毫秒", ms.to_string()),
        ResultItem::new("本地日期时间", recognized_with_zone(d, zone)?),
        ResultItem::new("ISO 8601", iso),
        ResultItem::new("UTC", utc),
    ];
    for a in p.alternatives {
        out.push(ResultItem::new(
            format!("其他识别 [{}]", a.format),
            recognized_with_zone(a.date, zone)?,
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
            format!("{}-W{:0>2}", iso_week(d, zone)?.0, iso_week(d, zone)?.1),
        ),
    ]);
    for line in custom.lines() {
        if let Some((name, pattern)) = line.split_once('=') {
            let pattern = pattern.trim();
            let utc = pattern.to_uppercase().starts_with("UTC:");
            out.push(ResultItem::new(
                name.trim(),
                format_with_zone(d, if utc { &pattern[4..] } else { pattern }, utc, zone)?,
            ));
        }
    }
    Ok(out)
}
fn trim10(n: f64) -> String {
    let s = format!("{n:.10}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod timezone_tests {
    use super::*;
    #[test]
    fn changed_system_zone_source_is_rechecked_after_short_ttl() {
        let calls = std::cell::Cell::new(0);
        let source = || {
            let n = calls.get();
            calls.set(n + 1);
            Ok(if n == 0 { "UTC" } else { "Asia/Shanghai" }.to_string())
        };
        let mut cache = SystemZoneCache::default();
        let first = cache.resolve(std::time::Duration::ZERO, source).unwrap();
        let cached = cache
            .resolve(std::time::Duration::from_millis(999), source)
            .unwrap();
        assert_eq!(calls.get(), 1);
        assert_eq!(first, cached);
        let changed = cache
            .resolve(std::time::Duration::from_secs(1), source)
            .unwrap();
        let a = parse_with_zone("2026-01-01 12:00:00", "YMD", first).unwrap();
        let b = parse_with_zone("2026-01-01 12:00:00", "YMD", changed).unwrap();
        assert_eq!(calls.get(), 2);
        assert_eq!(a.date.unix_timestamp() - b.date.unix_timestamp(), 8 * 3600);
        assert!(
            cache
                .resolve(std::time::Duration::from_secs(2), || Err(
                    "read failure".into()
                ))
                .is_err()
        );
        assert!(
            cache
                .resolve(std::time::Duration::from_secs(2), || Ok("UTC".into()))
                .is_ok()
        );
    }
}
