//! Decimal rounding compatible with Intl's default half-expand mode.
pub fn significant(value: f64, precision: usize) -> String {
    if value == 0. {
        return if value.is_sign_negative() {
            "-0".into()
        } else {
            "0".into()
        };
    }
    if !value.is_finite() {
        return value.to_string();
    }
    let mut buffer = ryu_js::Buffer::new();
    let raw = buffer.format_finite(value.abs());
    let (body, exponent) = raw
        .split_once('e')
        .map(|(b, e)| (b, e.parse::<i32>().unwrap()))
        .unwrap_or((raw, 0));
    let dot = body.find('.').unwrap_or(body.len()) as i32;
    let mut digits: Vec<u8> = body
        .bytes()
        .filter(|c| *c != b'.')
        .map(|c| c - b'0')
        .collect();
    let mut point = dot + exponent;
    while digits.first() == Some(&0) {
        digits.remove(0);
        point -= 1;
    }
    if digits.len() > precision {
        let up = digits[precision] >= 5;
        digits.truncate(precision);
        if up {
            let mut pos = digits.len();
            loop {
                if pos == 0 {
                    digits.insert(0, 1);
                    point += 1;
                    break;
                }
                pos -= 1;
                if digits[pos] < 9 {
                    digits[pos] += 1;
                    break;
                }
                digits[pos] = 0;
            }
        }
    }
    let mut out = String::new();
    if value.is_sign_negative() {
        out.push('-');
    }
    if point <= 0 {
        out.push_str("0.");
        out.push_str(&"0".repeat((-point) as usize));
        for d in &digits {
            out.push((b'0' + *d) as char);
        }
    } else {
        for i in 0..point as usize {
            out.push((b'0' + digits.get(i).copied().unwrap_or(0)) as char);
        }
        if (point as usize) < digits.len() {
            out.push('.');
            for d in &digits[point as usize..] {
                out.push((b'0' + *d) as char);
            }
        }
    }
    if out.contains('.') {
        while out.ends_with('0') {
            out.pop();
        }
        if out.ends_with('.') {
            out.pop();
        }
    }
    out
}
pub fn calculation(value: f64, locale: &str) -> Result<String, String> {
    if !value.is_finite() {
        return Err("计算结果无效".into());
    }
    let precision = if value.fract() == 0. && value.abs() < 1e21 {
        32
    } else {
        14
    };
    let out = significant(value, precision);
    Ok(
        if ["de", "fr", "es", "it", "ru"]
            .iter()
            .any(|p| locale.starts_with(p))
        {
            out.replace('.', ",")
        } else {
            out
        },
    )
}
