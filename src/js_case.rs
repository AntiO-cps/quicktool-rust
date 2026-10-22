//! ECMAScript default casing captured from the original application's Electron.
//! Mapping versions are independent of Rust's Unicode tables and the host OS.
#[path = "index_categories.rs"]
mod categories;
#[path = "js_case_data.rs"]
mod data;

/// The original V8 `\p{L}` property, independent of Rust's Unicode version.
pub fn is_letter(value: char) -> bool {
    categories::is_letter(value)
}
/// The original V8 `[\p{L}\p{N}]` property.
pub fn is_alphanumeric(value: char) -> bool {
    categories::is_alphanumeric(value)
}

fn member(value: char, ranges: &[(u32, u32)]) -> bool {
    let code = value as u32;
    let next = ranges.partition_point(|&(start, _)| start <= code);
    next > 0 && code <= ranges[next - 1].1
}
fn append(out: &mut String, value: char, table: &[(u32, &str)], upper: bool) {
    if value.is_ascii() {
        out.push(if upper {
            value.to_ascii_uppercase()
        } else {
            value.to_ascii_lowercase()
        });
    } else if let Ok(index) = table.binary_search_by_key(&(value as u32), |&(code, _)| code) {
        out.push_str(table[index].1);
    } else {
        out.push(value);
    }
}
/// Default `String.prototype.toLowerCase`; includes the Unicode Final_Sigma rule.
pub fn lower(value: &str) -> String {
    if value.is_ascii() {
        return value.to_ascii_lowercase();
    }
    let mut out = String::with_capacity(value.len());
    if !value.contains('\u{03a3}') {
        for c in value.chars() {
            append(&mut out, c, data::LOWER, false);
        }
        return out;
    }
    let mut preceded_by_cased = false;
    for (offset, c) in value.char_indices() {
        if c == '\u{03a3}' {
            // Sigma is not Case_Ignorable. Each lookahead stops before the next
            // Sigma, so these intervals never overlap: total cost stays linear.
            let followed_by_cased = value[offset + c.len_utf8()..]
                .chars()
                .find(|&next| !member(next, data::CASE_IGNORABLE))
                .is_some_and(|next| member(next, data::CASED));
            out.push(if preceded_by_cased && !followed_by_cased {
                '\u{03c2}'
            } else {
                '\u{03c3}'
            });
        } else {
            append(&mut out, c, data::LOWER, false);
        }
        if !member(c, data::CASE_IGNORABLE) {
            preceded_by_cased = member(c, data::CASED);
        }
    }
    out
}
/// Default `String.prototype.toUpperCase`; supports expanding mappings such as ß → SS.
pub fn upper(value: &str) -> String {
    if value.is_ascii() {
        return value.to_ascii_uppercase();
    }
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        append(&mut out, c, data::UPPER, true);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_scalars_match_original_electron_case_mappings() {
        let source: serde_json::Value =
            serde_json::from_str(include_str!("../tests/js-case-oracle.json")).unwrap();
        let lower_rows = source["lower"].as_array().unwrap();
        let upper_rows = source["upper"].as_array().unwrap();
        let (mut lo, mut up) = (0, 0);
        let (mut std_lower_diff, mut std_upper_diff) = (0, 0);
        for cp in 0..=0x10ffffu32 {
            let Some(c) = char::from_u32(cp) else {
                continue;
            };
            let text = c.to_string();
            let expected_lower =
                if lo < lower_rows.len() && lower_rows[lo][0].as_u64() == Some(cp as u64) {
                    let out = lower_rows[lo][1].as_str().unwrap();
                    lo += 1;
                    out
                } else {
                    &text
                };
            let expected_upper =
                if up < upper_rows.len() && upper_rows[up][0].as_u64() == Some(cp as u64) {
                    let out = upper_rows[up][1].as_str().unwrap();
                    up += 1;
                    out
                } else {
                    &text
                };
            assert_eq!(lower(&text), expected_lower, "lower U+{cp:04X}");
            assert_eq!(upper(&text), expected_upper, "upper U+{cp:04X}");
            std_lower_diff += usize::from(text.to_lowercase() != expected_lower);
            std_upper_diff += usize::from(text.to_uppercase() != expected_upper);
        }
        println!(
            "Rust std scalar case differences: lower={std_lower_diff}, upper={std_upper_diff}"
        );
    }
    #[test]
    fn contextual_sigma_and_expansions_match_original_electron() {
        let source: serde_json::Value =
            serde_json::from_str(include_str!("../tests/js-case-oracle.json")).unwrap();
        for case in source["contexts"].as_array().unwrap() {
            let text = case["text"].as_str().unwrap();
            assert_eq!(lower(text), case["lower"].as_str().unwrap(), "{text:?}");
            assert_eq!(upper(text), case["upper"].as_str().unwrap(), "{text:?}");
        }
    }
    #[test]
    fn all_codepoints_match_original_electron_letter_classes() {
        let source: serde_json::Value =
            serde_json::from_str(include_str!("../tests/index-categories-oracle.json")).unwrap();
        let ranges = source["letterRanges"].as_array().unwrap();
        let mut next = 0;
        let mut count = 0;
        for value in 0..=0x10ffffu32 {
            while next < ranges.len() && value as u64 > ranges[next][1].as_u64().unwrap() {
                next += 1;
            }
            let expected = next < ranges.len() && value as u64 >= ranges[next][0].as_u64().unwrap();
            let actual = char::from_u32(value).is_some_and(is_letter);
            assert_eq!(actual, expected, "U+{value:04X}");
            count += usize::from(actual);
        }
        assert_eq!(count as u64, source["letterCount"].as_u64().unwrap());
    }
}
