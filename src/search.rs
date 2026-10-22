use crate::core::{js_space, normalize_search};
use serde::Serialize;
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

#[derive(Debug, Clone, Serialize)]
pub struct Match {
    pub success: bool,
    pub score: f64,
    pub indexes: Vec<Option<usize>>,
}
fn fold(c: char) -> Vec<char> {
    crate::js_case::lower(&c.to_string().nfkd().collect::<String>())
        .chars()
        .filter(|c| !is_combining_mark(*c))
        .collect()
}

fn separator(c: char) -> bool {
    js_space(c) || "_-–—()（）[]【】.,，。'\"·/\\:;".contains(c)
}
pub fn fuzzy(query: &str, target: &str) -> Match {
    let fail = || Match {
        success: false,
        score: 0.,
        indexes: vec![],
    };
    if target.is_empty() || query.trim_matches(js_space).is_empty() {
        return fail();
    }
    let chars: Vec<char> = target.chars().collect();
    let mut hay = Vec::new();
    let mut map = Vec::new();
    let mut boundaries = vec![0];
    for (i, &c) in chars.iter().enumerate() {
        if i == 0
            || chars
                .get(i - 1)
                .is_some_and(|c| js_space(*c) || "_-–—()（）[]【】.,，。/\\:;".contains(*c))
        {
            boundaries.push(hay.len());
        }
        for c in fold(c) {
            if !separator(c) {
                hay.push(c);
                map.push(i);
            }
        }
    }
    let hay: Vec<u16> = hay.iter().collect::<String>().encode_utf16().collect();
    // Ordered query tokens cannot consume more UTF-16 units than the target.
    // Stop early before normalizing megabytes of Base64 for each short alias.
    let mut tokens = Vec::new();
    let mut units = 0;
    for word in query.split(js_space) {
        for c in word.chars().flat_map(fold).filter(|c| !separator(*c)) {
            units += c.len_utf16();
            if units > hay.len() {
                return fail();
            }
        }
        let token: Vec<u16> = crate::js_case::lower(&word.nfkd().collect::<String>())
            .chars()
            .filter(|c| !is_combining_mark(*c) && !separator(*c))
            .collect::<String>()
            .encode_utf16()
            .collect();
        if !token.is_empty() {
            tokens.push(token);
        }
    }
    if hay.is_empty() || tokens.is_empty() {
        return fail();
    }
    let mut indexes = Vec::new();
    let mut total = 0.;
    let mut from = 0;
    for token in &tokens {
        let mut best: Option<(f64, Vec<usize>)> = None;
        if let Some(pos) = hay[from..].windows(token.len()).position(|w| w == token) {
            let p = pos + from;
            let extra = hay.len().saturating_sub(token.len());
            let score: f64 = 0.82
                + if p == 0 { 0.10 } else { 0. }
                + if boundaries.contains(&p) { 0.055 } else { 0. }
                + if extra < 5 {
                    0.035
                } else if extra < 10 {
                    0.018
                } else {
                    0.
                };
            best = Some((score.min(1.), (p..p + token.len()).collect()));
        }
        for start in from..hay.len() {
            if hay[start] != token[0] {
                continue;
            }
            let mut positions = vec![start];
            let mut qi = 1;
            for (i, &ch) in hay.iter().enumerate().skip(start + 1) {
                if qi < token.len() && ch == token[qi] {
                    positions.push(i);
                    qi += 1;
                }
            }
            if qi != token.len() {
                continue;
            }
            let span = positions.last().unwrap() - start + 1;
            let density = token.len() as f64 / span as f64;
            let contiguous = positions.windows(2).filter(|v| v[1] == v[0] + 1).count() as f64
                / token.len().saturating_sub(1).max(1) as f64;
            let score = (0.48
                + density * 0.22
                + contiguous * 0.10
                + (0.08 - start as f64 * 0.006).max(0.)
                + if boundaries.contains(&start) {
                    0.055
                } else {
                    0.
                }
                + if hay == *token { 0.10 } else { 0. })
            .min(0.995);
            if best.as_ref().is_none_or(|b| score > b.0) {
                best = Some((score, positions));
            }
            if best.as_ref().is_some_and(|b| b.0 > 0.97) {
                break;
            }
        }
        let Some((score, positions)) = best else {
            return fail();
        };
        total += score;
        from = (positions.last().unwrap() + 1).min(hay.len());
        for p in positions {
            let index = map.get(p).copied();
            if !indexes.contains(&index) {
                indexes.push(index);
            }
        }
    }
    let score = (total / tokens.len() as f64
        + if normalize_search(target) == normalize_search(query.trim_matches(js_space)) {
            0.08
        } else {
            0.
        })
    .min(1.);
    Match {
        success: true,
        score,
        indexes,
    }
}
