use crate::{calculator, core::*, modules, search, units};

use serde::Deserialize;
use serde_json::Value;
use std::sync::OnceLock;
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Command {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub primary_alias: String,
    pub aliases: Vec<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemCommand {
    pub id: String,
    pub name: String,
    pub command: String,
    pub aliases: Vec<String>,
    #[serde(default)]
    pub needs_arg: bool,
}
fn system_command_data() -> &'static [SystemCommand] {
    static DATA: modules::LeakedCache<[SystemCommand]> = modules::LeakedCache::new();
    DATA.get_or_load(|| {
        modules::read_module_file("system", "system-commands.json")
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .map(|data: Vec<SystemCommand>| &*Box::leak(data.into_boxed_slice()))
    })
    .unwrap_or(&[])
}
pub fn system_commands() -> &'static [SystemCommand] {
    system_command_data()
}
#[derive(Clone, Debug, Deserialize)]
pub struct WindowsSetting {
    pub name: String,
    pub command: String,
    pub aliases: Vec<String>,
}
fn windows_setting_data() -> &'static [WindowsSetting] {
    static DATA: modules::LeakedCache<[WindowsSetting]> = modules::LeakedCache::new();
    DATA.get_or_load(|| {
        modules::read_module_file("system", "windows-settings.json")
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .map(|data: Vec<WindowsSetting>| &*Box::leak(data.into_boxed_slice()))
    })
    .unwrap_or(&[])
}
pub fn windows_settings_search(q: &str, limit: usize) -> Vec<(&'static WindowsSetting, f64)> {
    let data = windows_setting_data();
    let q = q.trim();
    let mut matches = data
        .iter()
        .filter_map(|item| {
            let mut best = if q.is_empty() {
                0.55
            } else {
                search::fuzzy(q, &item.name).score
            };
            for alias in &item.aliases {
                best = best.max(search::fuzzy(q, alias).score);
            }
            (q.is_empty() || best >= 0.58).then_some((item, best))
        })
        .collect::<Vec<_>>();
    matches.sort_by(|a, b| {
        b.1.total_cmp(&a.1)
            .then_with(|| crate::countries::locale_compare(&a.0.name, &b.0.name))
    });
    matches.truncate(limit);
    matches
}
pub fn commands() -> &'static Vec<Command> {
    static DATA: OnceLock<Vec<Command>> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(include_str!("../assets/commands.json")).unwrap())
}
pub fn command(input: &str) -> Option<&'static Command> {
    let n = normalize_search(input);
    commands()
        .iter()
        .find(|c| c.id == n || c.aliases.iter().any(|a| normalize_search(a) == n))
}
/// Command whose id or an alias exactly matches the query while its module is
/// not installed. The caller surfaces an install hint so an exact command
/// query never falls through to unrelated app matches.
pub fn disabled_exact(input: &str) -> Option<&'static Command> {
    let command = command(input)?;
    (!modules::is_enabled(&command.id)).then_some(command)
}
pub fn suggestions(input: &str) -> Vec<(&'static Command, f64)> {
    suggestions_all(input).into_iter().take(16).collect()
}
/// Apply usage before the caller truncates; ties retain the original tool order.
pub fn suggestions_all(input: &str) -> Vec<(&'static Command, f64)> {
    let input = input.strip_prefix('/').unwrap_or(input);
    let mut results: Vec<_> = commands()
        .iter()
        .filter(|c| modules::is_enabled(&c.id))
        .filter_map(|c| {
            let score = best_alias(c, input).map_or(0., |(_, score)| score);
            (score > 0.).then_some((c, score))
        })
        .collect();
    results.sort_by(|a, b| b.1.total_cmp(&a.1));
    results
}
/// Split commandParse's first JavaScript whitespace, retaining the argument text.
pub fn parse_command_input(raw: &str) -> (&str, &str) {
    parse_command_input_without_slash(raw.strip_prefix('/').unwrap_or(raw))
}
/// Match the original parseToolArg/subcommandMatch threshold and alias order.
pub fn parse_submode(id: &str, raw: &str) -> (Option<String>, String) {
    let subcommands: &[(&str, &[&str])] = match id {
        "base64" => &[
            ("encode", &["encode", "编码", "bianma", "bm"]),
            ("decode", &["decode", "解码", "jiema", "jm"]),
        ],
        "cmd" | "powershell" | "terminal" => &[(
            "admin",
            &["admin", "administrator", "管理员", "guanliyuan", "gly", "a"],
        )],
        _ => return (None, raw.into()),
    };
    let trimmed = raw.trim_start_matches(js_space);
    let (token, input) = parse_command_input_without_slash(trimmed);
    let mut best = None;
    for &(submode, aliases) in subcommands {
        for alias in aliases {
            let score = if normalize_search(token).is_empty() {
                0.70
            } else {
                search::fuzzy(token, alias).score
            };
            if score > 0. && best.is_none_or(|(_, old_score)| score > old_score) {
                best = Some((submode, score));
            }
        }
    }
    match best {
        Some((submode, score)) if !token.is_empty() && score >= 0.79 => (
            Some(submode.into()),
            input.trim_start_matches(js_space).into(),
        ),
        _ => (None, raw.into()),
    }
}
fn parse_command_input_without_slash(raw: &str) -> (&str, &str) {
    raw.char_indices()
        .find(|(_, ch)| js_space(*ch))
        .map(|(offset, ch)| (&raw[..offset], &raw[offset + ch.len_utf8()..]))
        .unwrap_or((raw, ""))
}
pub fn best_alias<'a>(command: &'a Command, input: &str) -> Option<(&'a str, f64)> {
    let mut best: Option<(&str, f64)> = None;
    for alias in &command.aliases {
        let score = if normalize_search(input).is_empty() {
            0.70
        } else {
            search::fuzzy(input, alias).score
        };
        if score > 0.
            && best.is_none_or(|(old, old_score)| {
                score > old_score
                    || (score == old_score
                        && alias.encode_utf16().count() < old.encode_utf16().count())
            })
        {
            best = Some((alias, score));
        }
    }
    best
}
pub fn execute(id: &str, q: &str) -> Result<Vec<ResultItem>, String> {
    if let Some(info) = modules::module_of(id) {
        if !modules::is_enabled(id) {
            return Err(format!(
                "{}模块未安装，可在设置 → 模块管理中安装",
                info.name
            ));
        }
    }
    Ok(match id {
        "time" => crate::dates::results(
            q,
            "YMD",
            "Local compact=YYYY-MM-DD HH:mm:ss\nUTC compact=UTC:YYYY-MM-DD HH:mm:ss\nISO week=YYYY-WW",
        )?,
        "calculator" => vec![ResultItem::new(
            "计算结果",
            crate::numbers::calculation(calculator::calculate(q, "en-US")?, "en-US")?,
        )],
        "base" => base_results(q)?,
        "unit" => units::results(q)?,
        "width" => vec![
            ResultItem::new("转换为半角", to_half_width(q)),
            ResultItem::new("转换为全角", to_full_width(q, true)),
        ],
        "url" => vec![
            ResultItem::new("URL 编码", percent_encode(q)),
            ResultItem::new("URL 解码", percent_decode(q).unwrap_or_else(|_| q.into())),
        ],
        "escape" => escape_results(q),
        "json" => crate::json::results(q)?,
        "base64" => crate::text::base64_results(q),
        "case" => crate::text::case_results(q),
        "text" => crate::text::text_results(q),
        "uuid" => {
            let mut b = [0u8; 16];
            getrandom::fill(&mut b).map_err(|e| e.to_string())?;
            b[6] = (b[6] & 0x0f) | 0x40;
            b[8] = (b[8] & 0x3f) | 0x80;
            let hex = b.map(|v| format!("{v:02x}")).join("");
            vec![ResultItem::new(
                "UUID v4",
                format!(
                    "{}-{}-{}-{}-{}",
                    &hex[..8],
                    &hex[8..12],
                    &hex[12..16],
                    &hex[16..20],
                    &hex[20..]
                ),
            )]
        }
        "random" => {
            let mut bytes = [0u8; 24];
            getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
            vec![ResultItem::new(
                "32位随机字符串",
                bytes
                    .map(|v| char::from_digit((v % 36) as u32, 36).unwrap())
                    .iter()
                    .collect::<String>(),
            )]
        }
        "country" => countries(q)?,
        "color" => color_results(q)?,
        "char" | "mathchar" | "greek" | "bopomofo" | "emoji" => crate::unicode::results(q, id)?,
        "help" => commands()
            .iter()
            .filter(|c| c.id == "help" || modules::is_enabled(&c.id))
            .filter(|c| {
                q.is_empty()
                    || search::fuzzy(q, &c.name).success
                    || c.aliases.iter().any(|a| search::fuzzy(q, a).success)
            })
            .map(|c| {
                ResultItem::new(
                    &c.name,
                    format!("/{} · {}", c.primary_alias, c.aliases.join(" / ")),
                )
            })
            .collect(),
        _ => return Err(format!("{id} 尚在迁移中")),
    })
}
pub fn detect(q: &str) -> Vec<(String, ResultItem)> {
    detect_scored(q)
        .into_iter()
        .take(16)
        .map(|(id, item, _)| (id, item))
        .collect()
}
pub fn detect_scored(q: &str) -> Vec<(String, ResultItem, f64)> {
    static PATTERNS: OnceLock<Vec<regex_lite::Regex>> = OnceLock::new();
    let patterns=PATTERNS.get_or_init(||[
        r"^\d{4}[-/.]\d{1,2}(?:[-/.]\d{1,4})?(?:[ T]\d{1,2}:\d{2}(?::\d{2})?)?$",
        r"(?i)(?:\d\s*[+\-*/%^()!]|\)\s*\(|(?:\d|\))\s*(?:pi|tau|e)\b|\b(?:sin|cos|tan|asin|acos|atan|sqrt|log|log10|ln|abs|floor|ceil|round|exp|min|max|pow|hypot)\s*\()",
        r"^[+-]?\d+(?:\.\d+)?$",r"^\d{10,13}$",r"^\d{6}(?:\d{2})?$",
        r"^[A-Za-z0-9+/=\s]+$",r"(?i)^https?://",
        r"(?:%[0-9A-Fa-f]{2}|\\u(?:\{|[0-9A-Fa-f]{4})|&(?:#\d+|#x[0-9a-f]+|\w+);)",
        r"(?i)^[+-]?0(?:x[0-9a-f]+|b[01]+|o[0-7]+)$",
        r"(?i)^(?:U\+|\\u(?:\{|[0-9A-Fa-f]{4})|\\U[0-9A-Fa-f]{8}|0x[0-9A-Fa-f]{1,6})",
    ].iter().map(|p|regex_lite::Regex::new(p).unwrap()).collect());
    let raw = q.trim();
    let has = |i: usize| patterns[i].is_match(raw);
    let compact = has(4) && execute("time", q).is_ok();
    let mut out = Vec::new();
    let mut add = |id: &str, score: f64, limit: usize| {
        if let Ok(items) = execute(id, q) {
            out.extend(
                items
                    .into_iter()
                    .take(limit)
                    .enumerate()
                    .map(|(i, item)| (id.to_string(), item, score - i as f64 * 0.001)),
            );
        }
    };
    if !has(0) && has(1) {
        add("calculator", 0.96, usize::MAX)
    }
    add(
        "time",
        if has(3) { 0.9 } else { 0.94 },
        if compact { 4 } else { usize::MAX },
    );
    if !has(2) {
        add("json", 0.97, usize::MAX)
    }
    add(
        "color",
        if compact { 0.93 } else { 0.98 },
        if compact { 4 } else { usize::MAX },
    );
    if q.encode_utf16().count() < 500 && crate::js_case::upper(q) != crate::js_case::lower(q) {
        add("case", 0.28, usize::MAX)
    }
    if modules::is_enabled("base64")
        && raw.len() >= 8
        && has(5)
        && crate::text::base64_decode(q).is_some_and(|s| !s.is_empty())
    {
        let item = execute("base64", q).unwrap().remove(1);
        out.push(("base64".into(), item, 0.72));
    }
    // Finish all additions through the closure before appending the special decoded row.
    if has(6) {
        if let Ok(items) = execute("url", q) {
            out.extend(
                items
                    .into_iter()
                    .enumerate()
                    .map(|(i, v)| ("url".into(), v, 0.66 - i as f64 * 0.001)),
            );
        }
    }
    let mut add = |id: &str, score: f64| {
        if let Ok(items) = execute(id, q) {
            out.extend(
                items
                    .into_iter()
                    .enumerate()
                    .map(|(i, item)| (id.to_string(), item, score - i as f64 * 0.001)),
            );
        }
    };
    if has(7) {
        add("escape", 0.9)
    }
    if has(8) {
        add("base", 0.93)
    }
    add(
        "unit",
        if [" to ", " in ", " = ", " → ", " 转 ", " 转换为 ", " 换算为 "]
            .iter()
            .any(|s| raw.to_lowercase().contains(s))
        {
            0.94
        } else {
            0.80
        },
    );
    if has(9) && code_points(raw).is_some() {
        add("char", 0.96)
    }
    if q.chars()
        .any(|c| ('\u{ff01}'..='\u{ff5e}').contains(&c) || c == '\u{3000}')
    {
        add("width", 0.92)
    }
    if modules::is_enabled("country") {
        if let Some(score) = crate::countries::detection_score(q) {
            add("country", score)
        }
    }
    if q.contains('\n') {
        add("text", 0.36)
    }
    out.sort_by(|a, b| b.2.total_cmp(&a.2));
    out
}
fn countries(q: &str) -> Result<Vec<ResultItem>, String> {
    let matches = crate::countries::matches(q);
    if matches.is_empty() {
        return Err("未找到国家/地区".into());
    }
    Ok(matches.iter().flat_map(crate::countries::items).collect())
}
fn color_results(q: &str) -> Result<Vec<ResultItem>, String> {
    let variants = crate::colors::candidates(q);
    let Some(&primary) = variants.first() else {
        return Err("无法识别颜色".into());
    };
    let mut rows = Vec::new();
    if variants.len() > 1 {
        let labels = if q.trim_matches(js_space).len() == 3 {
            ["QuickTool 灰度短 HEX", "灰度 + Alpha"]
        } else {
            ["CSS #RGB", "QuickTool 重复模式"]
        };
        rows.extend(
            variants
                .iter()
                .zip(labels)
                .map(|(color, label)| ResultItem::new(label, color.hex(true))),
        );
    }
    rows.extend(crate::colors::formats(primary));
    Ok(rows)
}
pub fn country_matches(q: &str) -> Vec<Value> {
    crate::countries::matches(q)
}
