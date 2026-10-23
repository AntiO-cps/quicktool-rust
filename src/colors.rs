use crate::core::ResultItem;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Color {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}
impl Color {
    pub fn new(r: f64, g: f64, b: f64, a: f64) -> Self {
        fn clamp(v: f64) -> f64 {
            if v.is_nan() { 0. } else { v.clamp(0., 1.) }
        }
        Self {
            r: clamp(r),
            g: clamp(g),
            b: clamp(b),
            a: clamp(a),
        }
    }
    pub fn hex(self, alpha: bool) -> String {
        let mut s = format!(
            "#{:02X}{:02X}{:02X}",
            (self.r * 255.).round() as u8,
            (self.g * 255.).round() as u8,
            (self.b * 255.).round() as u8
        );
        if alpha {
            s.push_str(&format!("{:02X}", (self.a * 255.).round() as u8));
        }
        s
    }
}
fn linear(c: f64) -> f64 {
    let c = c.clamp(0., 1.);
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}
fn srgb(c: f64) -> f64 {
    let c = c.max(0.);
    if c <= 0.0031308 {
        (12.92 * c).clamp(0., 1.)
    } else {
        (1.055 * c.powf(1. / 2.4) - 0.055).clamp(0., 1.)
    }
}
pub fn hsv(c: Color) -> [f64; 3] {
    let max = c.r.max(c.g).max(c.b);
    let min = c.r.min(c.g).min(c.b);
    let d = max - min;
    let h = if d == 0. {
        0.
    } else if max == c.r {
        60. * ((c.g - c.b) / d % 6.)
    } else if max == c.g {
        60. * ((c.b - c.r) / d + 2.)
    } else {
        60. * ((c.r - c.g) / d + 4.)
    };
    [
        h.rem_euclid(360.),
        if max == 0. { 0. } else { d / max },
        max,
    ]
}
pub fn hsl(c: Color) -> [f64; 3] {
    let max = c.r.max(c.g).max(c.b);
    let min = c.r.min(c.g).min(c.b);
    let l = (max + min) / 2.;
    let d = max - min;
    let h = if d == 0. {
        0.
    } else if max == c.r {
        ((c.g - c.b) / d + if c.g < c.b { 6. } else { 0. }) * 60.
    } else if max == c.g {
        ((c.b - c.r) / d + 2.) * 60.
    } else {
        ((c.r - c.g) / d + 4.) * 60.
    };
    [
        h,
        if d == 0. {
            0.
        } else if l > 0.5 {
            d / (2. - max - min)
        } else {
            d / (max + min)
        },
        l,
    ]
}
fn from_hsv(h: f64, s: f64, v: f64, a: f64) -> Color {
    let h = h.rem_euclid(360.);
    let c = v.clamp(0., 1.) * s.clamp(0., 1.);
    let x = c * (1. - ((h / 60.) % 2. - 1.).abs());
    let m = v.clamp(0., 1.) - c;
    let (r, g, b) = match h {
        h if h < 60. => (c, x, 0.),
        h if h < 120. => (x, c, 0.),
        h if h < 180. => (0., c, x),
        h if h < 240. => (0., x, c),
        h if h < 300. => (x, 0., c),
        _ => (c, 0., x),
    };
    Color::new(r + m, g + m, b + m, a)
}
fn from_hsl(h: f64, s: f64, l: f64, a: f64) -> Color {
    let h = h.rem_euclid(360.) / 360.;
    let s = s.clamp(0., 1.);
    let l = l.clamp(0., 1.);
    if s == 0. {
        return Color::new(l, l, l, a);
    }
    let q = if l < 0.5 { l * (1. + s) } else { l + s - l * s };
    let p = 2. * l - q;
    let f = |mut t: f64| {
        if t < 0. {
            t += 1.
        }
        if t > 1. {
            t -= 1.
        }
        if t < 1. / 6. {
            p + (q - p) * 6. * t
        } else if t < 0.5 {
            q
        } else if t < 2. / 3. {
            p + (q - p) * (2. / 3. - t) * 6.
        } else {
            p
        }
    };
    Color::new(f(h + 1. / 3.), f(h), f(h - 1. / 3.), a)
}
fn xyz(c: Color) -> [f64; 3] {
    let (r, g, b) = (linear(c.r), linear(c.g), linear(c.b));
    [
        0.4124564 * r + 0.3575761 * g + 0.1804375 * b,
        0.2126729 * r + 0.7151522 * g + 0.072175 * b,
        0.0193339 * r + 0.119192 * g + 0.9503041 * b,
    ]
}
fn from_xyz([x, y, z]: [f64; 3], a: f64) -> Color {
    Color::new(
        srgb(3.2404542 * x - 1.5371385 * y - 0.4985314 * z),
        srgb(-0.969266 * x + 1.8760108 * y + 0.041556 * z),
        srgb(0.0556434 * x - 0.2040259 * y + 1.0572252 * z),
        a,
    )
}
fn lab([x, y, z]: [f64; 3]) -> [f64; 3] {
    let d = 6_f64 / 29.;
    let f = |t: f64| {
        if t > d.powi(3) {
            t.cbrt()
        } else {
            t / (3. * d * d) + 4. / 29.
        }
    };
    let (fx, fy, fz) = (f(x / 0.95047), f(y), f(z / 1.08883));
    [116. * fy - 16., 500. * (fx - fy), 200. * (fy - fz)]
}
fn from_lab([l, a, b]: [f64; 3]) -> [f64; 3] {
    let d = 6_f64 / 29.;
    let fy = (l + 16.) / 116.;
    let fx = fy + a / 500.;
    let fz = fy - b / 200.;
    let inv = |t: f64| {
        if t > d {
            t.powi(3)
        } else {
            3. * d * d * (t - 4. / 29.)
        }
    };
    [0.95047 * inv(fx), inv(fy), 1.08883 * inv(fz)]
}
fn lch([l, a, b]: [f64; 3]) -> [f64; 3] {
    [l, a.hypot(b), (b.atan2(a).to_degrees() + 360.) % 360.]
}
fn from_lch([l, c, h]: [f64; 3]) -> [f64; 3] {
    [l, c * h.to_radians().cos(), c * h.to_radians().sin()]
}
fn oklab(c: Color) -> [f64; 3] {
    let (r, g, b) = (linear(c.r), linear(c.g), linear(c.b));
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    [
        0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
    ]
}
fn from_oklab([l, a, b]: [f64; 3], alpha: f64) -> Color {
    let ll = (l + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let m = (l - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s = (l - 0.0894841775 * a - 1.291485548 * b).powi(3);
    Color::new(
        srgb(4.0767416621 * ll - 3.3077115913 * m + 0.2309699292 * s),
        srgb(-1.2684380046 * ll + 2.6097574011 * m - 0.3413193965 * s),
        srgb(-0.0041960863 * ll - 0.7034186147 * m + 1.707614701 * s),
        alpha,
    )
}
fn hsi(c: Color) -> [f64; 3] {
    let (r, g, b) = (c.r, c.g, c.b);
    let i = (r + g + b) / 3.;
    let s = if i == 0. {
        0.
    } else {
        1. - r.min(g).min(b) / i
    };
    let num = 0.5 * ((r - g) + (r - b));
    let den = ((r - g).powi(2) + (r - b) * (g - b)).sqrt();
    let theta = if den == 0. {
        0.
    } else {
        (num / den).clamp(-1., 1.).acos().to_degrees()
    };
    [if b <= g { theta } else { 360. - theta }, s, i]
}
fn from_hsi(h: f64, s: f64, i: f64, a: f64) -> Color {
    let h = h.rem_euclid(360.);
    let (s, i) = (s.clamp(0., 1.), i.clamp(0., 1.));
    let v =
        |hh: f64| i * (1. + s * hh.to_radians().cos() / (60. - hh).to_radians().cos().max(1e-9));
    if h < 120. {
        let b = i * (1. - s);
        let r = v(h);
        Color::new(r, 3. * i - r - b, b, a)
    } else if h < 240. {
        let r = i * (1. - s);
        let g = v(h - 120.);
        Color::new(r, g, 3. * i - r - g, a)
    } else {
        let g = i * (1. - s);
        let b = v(h - 240.);
        Color::new(3. * i - g - b, g, b, a)
    }
}
fn ycbcr(c: Color, kr: f64, kb: f64, limited: bool) -> [f64; 3] {
    let y = kr * c.r + (1. - kr - kb) * c.g + kb * c.b;
    let cb = (c.b - y) / (2. * (1. - kb));
    let cr = (c.r - y) / (2. * (1. - kr));
    if limited {
        [16. + 219. * y, 128. + 224. * cb, 128. + 224. * cr]
    } else {
        [255. * y, 128. + 255. * cb, 128. + 255. * cr]
    }
}
fn from_ycbcr([y, cb, cr]: [f64; 3], kr: f64, kb: f64, a: f64) -> Color {
    let (y, cb, cr) = (y / 255., (cb - 128.) / 255., (cr - 128.) / 255.);
    let r = y + 2. * (1. - kr) * cr;
    let b = y + 2. * (1. - kb) * cb;
    Color::new(r, (y - kr * r - kb * b) / (1. - kr - kb), b, a)
}
fn float(s: &str) -> f64 {
    let end = s
        .char_indices()
        .take_while(|(_, c)| c.is_ascii_digit() || "+-.eE".contains(*c))
        .last()
        .map_or(0, |(i, c)| i + c.len_utf8());
    s[..end].parse().unwrap_or(0.)
}
fn percent(s: &str) -> f64 {
    float(s) / 100.
}
fn alpha(s: &str) -> f64 {
    if s.ends_with('%') {
        percent(s)
    } else {
        float(s)
    }
    .clamp(0., 1.)
}
fn angle(s: &str) -> f64 {
    let n = float(s);
    let n = if s.ends_with("turn") {
        n * 360.
    } else if s.ends_with("rad") {
        n * 180. / std::f64::consts::PI
    } else if s.ends_with("grad") {
        n * 0.9
    } else {
        n
    };
    n.rem_euclid(360.)
}
fn from_hex(s: &str) -> Option<Color> {
    if s.len() != 6 && s.len() != 8 {
        return None;
    }
    let r = u8::from_str_radix(&s[..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    let a = if s.len() == 8 {
        u8::from_str_radix(&s[6..], 16).ok()?
    } else {
        255
    };
    Some(Color::new(
        r as f64 / 255.,
        g as f64 / 255.,
        b as f64 / 255.,
        a as f64 / 255.,
    ))
}
pub fn candidates(raw: &str) -> Vec<Color> {
    static NAMES: OnceLock<serde_json::Value> = OnceLock::new();
    let names = NAMES.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/data/css-colors.json")).unwrap()
    });
    let s = raw.trim().to_lowercase();
    let mut out = Vec::new();
    if s == "transparent" {
        out.push(Color::new(0., 0., 0., 0.));
    }
    if let Some(h) = names[&s].as_str() {
        if let Some(c) = from_hex(h.trim_start_matches('#')) {
            out.push(c);
        }
    }
    let h = s.trim_start_matches('#');
    if h.bytes().all(|b| b.is_ascii_hexdigit())
        && ((s.starts_with('#') && (1..=8).contains(&h.len())) || [6, 8].contains(&h.len()))
    {
        let expanded = match h.len() {
            1 => vec![h.repeat(6)],
            2 => vec![
                h.repeat(3),
                format!("{}{}", h[..1].repeat(6), h[1..].repeat(2)),
            ],
            3 => vec![h.chars().flat_map(|c| [c, c]).collect(), h.repeat(2)],
            4 => vec![h.chars().flat_map(|c| [c, c]).collect()],
            6 | 8 => vec![h.into()],
            _ => vec![],
        };
        for h in expanded {
            if let Some(c) = from_hex(&h) {
                out.push(c);
            }
        }
    }
    if let Some((name, body)) = s.split_once('(') {
        if let Some(body) = body.strip_suffix(')') {
            let (body, a) = if let Some((b, a)) = body.rsplit_once('/') {
                (b, alpha(a.trim()))
            } else {
                (body, 1.)
            };
            let body = body.replace(',', " ");
            let p: Vec<_> = body.split_whitespace().collect();
            let mut a = a;
            if ["rgba", "hsla"].contains(&name) && p.len() >= 4 {
                a = alpha(p[3]);
            }
            let vals = || [float(p[0]), float(p[1]), float(p[2])];
            let c = if p.len() < 3 {
                None
            } else {
                match name {
                    "rgb" | "rgba" => {
                        let ch = |s: &str| {
                            if s.ends_with('%') {
                                percent(s)
                            } else {
                                float(s) / 255.
                            }
                        };
                        Some(Color::new(ch(p[0]), ch(p[1]), ch(p[2]), a))
                    }
                    "hsl" | "hsla" => Some(from_hsl(angle(p[0]), percent(p[1]), percent(p[2]), a)),
                    "hsv" | "hsb" => Some(from_hsv(angle(p[0]), percent(p[1]), percent(p[2]), a)),
                    "hsi" => Some(from_hsi(angle(p[0]), percent(p[1]), percent(p[2]), a)),
                    "hwb" => {
                        let (w, b) = (percent(p[1]).clamp(0., 1.), percent(p[2]).clamp(0., 1.));
                        if w + b >= 1. {
                            let g = w / (w + b);
                            Some(Color::new(g, g, g, a))
                        } else {
                            let c = from_hsv(angle(p[0]), 1., 1., a);
                            let f = 1. - w - b;
                            Some(Color::new(c.r * f + w, c.g * f + w, c.b * f + w, a))
                        }
                    }
                    "cmyk" if p.len() >= 4 => {
                        let k = percent(p[3]);
                        Some(Color::new(
                            (1. - percent(p[0])) * (1. - k),
                            (1. - percent(p[1])) * (1. - k),
                            (1. - percent(p[2])) * (1. - k),
                            a,
                        ))
                    }
                    "lab" => Some(from_xyz(from_lab(vals()), a)),
                    "lch" => Some(from_xyz(
                        from_lab(from_lch([float(p[0]), float(p[1]), angle(p[2])])),
                        a,
                    )),
                    "oklab" => Some(from_oklab(vals(), a)),
                    "oklch" => Some(from_oklab(
                        from_lch([float(p[0]), float(p[1]), angle(p[2])]),
                        a,
                    )),
                    "xyz" => {
                        let mut v = vals();
                        if v.iter().any(|n| n.abs() > 2.) {
                            v = v.map(|v| v / 100.);
                        }
                        Some(from_xyz(v, a))
                    }
                    "yuv" => {
                        let [y, u, v] = vals();
                        Some(Color::new(
                            y + 1.13983 * v,
                            y - 0.39465 * u - 0.58060 * v,
                            y + 2.03211 * u,
                            a,
                        ))
                    }
                    "ycbcr" | "ycbcr601" => Some(from_ycbcr(vals(), 0.299, 0.114, a)),
                    "ycbcr709" => Some(from_ycbcr(vals(), 0.2126, 0.0722, a)),
                    "ycbcr2020" => Some(from_ycbcr(vals(), 0.2627, 0.0593, a)),
                    "color" if p.len() >= 4 && p[0] == "display-p3" => {
                        let (r, g, b) = (
                            linear(float(p[1])),
                            linear(float(p[2])),
                            linear(float(p[3])),
                        );
                        Some(from_xyz(
                            [
                                0.48657095 * r + 0.26566769 * g + 0.19821729 * b,
                                0.22897456 * r + 0.69173852 * g + 0.07928691 * b,
                                0.04511338 * g + 1.04394437 * b,
                            ],
                            a,
                        ))
                    }
                    _ => None,
                }
            };
            if let Some(c) = c {
                out.push(c);
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    out.retain(|c| seen.insert(c.hex(true)));
    out
}
fn round(n: f64, digits: usize) -> String {
    use num_bigint::BigUint;
    // Number.toFixed rounds the exact binary value away from zero on a tie.
    // Rust's formatting rounds ties to even (e.g. 40.625 -> 40.62).
    if !n.is_finite() {
        return n.to_string();
    }
    let bits = n.abs().to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1u64 << 52) - 1);
    let (mantissa, shift) = if exponent == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1u64 << 52), exponent - 1023 - 52)
    };
    let numerator = BigUint::from(mantissa) * BigUint::from(10u8).pow(digits as u32);
    let rounded = if shift >= 0 {
        numerator << shift as usize
    } else {
        let denominator = BigUint::from(1u8) << (-shift) as usize;
        (numerator + (&denominator >> 1usize)) / denominator
    };
    let mut s = rounded.to_string();
    if digits > 0 {
        if s.len() <= digits {
            s = format!("{}{}", "0".repeat(digits + 1 - s.len()), s);
        }
        s.insert(s.len() - digits, '.');
    }
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if n.is_sign_negative() && s != "0" {
        s.insert(0, '-');
    }
    s
}
pub fn formats(c: Color) -> Vec<ResultItem> {
    let [h, s, l] = hsl(c);
    let [hh, ss, v] = hsv(c);
    let [hi, si, ii] = hsi(c);
    let xyz = xyz(c);
    let lab = lab(xyz);
    let lch = lch(lab);
    let ok = oklab(c);
    let okc = self::lch(ok);
    let k = 1. - c.r.max(c.g).max(c.b);
    let cmyk = if k >= 0.999999 {
        [0., 0., 0., 1.]
    } else {
        [
            (1. - c.r - k) / (1. - k),
            (1. - c.g - k) / (1. - k),
            (1. - c.b - k) / (1. - k),
            k,
        ]
    };
    let a = round(c.a, 3);
    let suffix = if c.a < 0.999 {
        format!(" / {a}")
    } else {
        String::new()
    };
    let rgb = [c.r, c.g, c.b].map(|v| (v * 255.).round() as u8);
    let mut out = vec![
        ResultItem::new("HEX", c.hex(false)),
        ResultItem::new("HEXA", c.hex(true)),
        ResultItem::new("RGB", format!("rgb({}, {}, {})", rgb[0], rgb[1], rgb[2])),
        ResultItem::new(
            "RGBA",
            format!("rgba({}, {}, {}, {a})", rgb[0], rgb[1], rgb[2]),
        ),
        ResultItem::new(
            "HSL",
            format!(
                "hsl({} {}% {}%{suffix})",
                round(h, 2),
                round(s * 100., 2),
                round(l * 100., 2)
            ),
        ),
        ResultItem::new(
            "HSV / HSB",
            format!(
                "hsv({} {}% {}%{suffix})",
                round(hh, 2),
                round(ss * 100., 2),
                round(v * 100., 2)
            ),
        ),
        ResultItem::new(
            "HSI",
            format!(
                "hsi({} {}% {}%{suffix})",
                round(hi, 2),
                round(si * 100., 2),
                round(ii * 100., 2)
            ),
        ),
        ResultItem::new(
            "HWB",
            format!(
                "hwb({} {}% {}%{suffix})",
                round(hh, 2),
                round(c.r.min(c.g).min(c.b) * 100., 2),
                round((1. - v) * 100., 2)
            ),
        ),
        ResultItem::new(
            "CMYK",
            format!(
                "cmyk({}% {}% {}% {}%{suffix})",
                round(cmyk[0] * 100., 2),
                round(cmyk[1] * 100., 2),
                round(cmyk[2] * 100., 2),
                round(cmyk[3] * 100., 2)
            ),
        ),
        ResultItem::new(
            "CIE XYZ D65",
            format!(
                "xyz({} {} {}{suffix})",
                round(xyz[0] * 100., 4),
                round(xyz[1] * 100., 4),
                round(xyz[2] * 100., 4)
            ),
        ),
        ResultItem::new(
            "CIE Lab D65",
            format!(
                "lab({} {} {}{suffix})",
                round(lab[0], 3),
                round(lab[1], 3),
                round(lab[2], 3)
            ),
        ),
        ResultItem::new(
            "CIE LCH D65",
            format!(
                "lch({} {} {}{suffix})",
                round(lch[0], 3),
                round(lch[1], 3),
                round(lch[2], 3)
            ),
        ),
        ResultItem::new(
            "OKLab",
            format!(
                "oklab({} {} {}{suffix})",
                round(ok[0], 5),
                round(ok[1], 5),
                round(ok[2], 5)
            ),
        ),
        ResultItem::new(
            "OKLCH",
            format!(
                "oklch({} {} {}{suffix})",
                round(okc[0], 5),
                round(okc[1], 5),
                round(okc[2], 3)
            ),
        ),
    ];
    let [x, y, z] = xyz;
    out.push(ResultItem::new(
        "Display-P3",
        format!(
            "color(display-p3 {} {} {}{suffix})",
            round(srgb(2.4934969 * x - 0.9313836 * y - 0.4027108 * z), 5),
            round(srgb(-0.829489 * x + 1.762664 * y + 0.0236247 * z), 5),
            round(srgb(0.0358458 * x - 0.0761724 * y + 0.9568845 * z), 5)
        ),
    ));
    for (label, kr, kb, limited) in [
        ("YCbCr BT.601 Full", 0.299, 0.114, false),
        ("YCbCr BT.709 Full", 0.2126, 0.0722, false),
        ("YCbCr BT.709 Limited", 0.2126, 0.0722, true),
        ("YCbCr BT.2020 Full", 0.2627, 0.0593, false),
    ] {
        let [y, cb, cr] = ycbcr(c, kr, kb, limited);
        out.push(ResultItem::new(
            label,
            format!(
                "Y {} · Cb {} · Cr {}",
                round(y, 2),
                round(cb, 2),
                round(cr, 2)
            ),
        ));
    }
    out.push(ResultItem::new(
        "YUV BT.601",
        format!(
            "Y {} · U {} · V {}",
            round(0.299 * c.r + 0.587 * c.g + 0.114 * c.b, 5),
            round(-0.14713 * c.r - 0.28886 * c.g + 0.436 * c.b, 5),
            round(0.615 * c.r - 0.51499 * c.g - 0.10001 * c.b, 5)
        ),
    ));
    out
}
