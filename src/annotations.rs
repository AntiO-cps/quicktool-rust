//! Pure Rust annotation model, rendering and layered PSD export.
use eframe::egui::{Color32, Pos2, Rect, Vec2, pos2};
#[cfg(test)]
use image::Rgba;
use image::RgbaImage;
use serde::{Deserialize, Serialize};
use std::fmt::Write;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tool {
    #[default]
    Pan,
    Rect,
    Ellipse,
    Line,
    Arrow,
    Pen,
    Text,
    Number,
    Mosaic,
    Highlight,
}
impl Tool {
    pub fn key(self) -> &'static str {
        match self {
            Self::Pan => "pan",
            Self::Rect => "rect",
            Self::Ellipse => "ellipse",
            Self::Line => "line",
            Self::Arrow => "arrow",
            Self::Pen => "pen",
            Self::Text => "text",
            Self::Number => "number",
            Self::Mosaic => "mosaic",
            Self::Highlight => "highlight",
        }
    }
    pub const ALL: [Self; 10] = [
        Self::Pan,
        Self::Rect,
        Self::Ellipse,
        Self::Line,
        Self::Arrow,
        Self::Pen,
        Self::Text,
        Self::Number,
        Self::Mosaic,
        Self::Highlight,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Pan => "移动",
            Self::Rect => "矩形",
            Self::Ellipse => "椭圆",
            Self::Line => "直线",
            Self::Arrow => "箭头",
            Self::Pen => "画笔",
            Self::Text => "文字",
            Self::Number => "序号",
            Self::Mosaic => "马赛克",
            Self::Highlight => "高亮",
        }
    }
    pub fn rank(self) -> u8 {
        match self {
            Self::Mosaic => 0,
            Self::Highlight => 1,
            _ => 2,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    pub id: u64,
    #[serde(rename = "type")]
    pub tool: Tool,
    pub x: f32,
    pub y: f32,
    pub x2: f32,
    pub y2: f32,
    pub color: [u8; 3],
    pub text_color: Option<[u8; 3]>,
    pub radius: f32,
    pub opacity: f32,
    pub stroke: f32,
    pub font_size: f32,
    pub text: String,
    pub number: u32,
    pub points: Vec<[f32; 2]>,
}
impl Annotation {
    pub fn new(id: u64, tool: Tool, p: Pos2, width: f32) -> Self {
        Self {
            id,
            tool,
            x: p.x,
            y: p.y,
            x2: p.x,
            y2: p.y,
            color: if tool == Tool::Highlight {
                [0, 0, 0]
            } else {
                [255, 79, 99]
            },
            text_color: None,
            radius: 0.,
            opacity: 0.55,
            stroke: 3. / width,
            font_size: 20. / width,
            text: String::new(),
            number: 1,
            points: Vec::new(),
        }
    }
    pub fn start(&self) -> Pos2 {
        pos2(self.x, self.y)
    }
    pub fn end(&self) -> Pos2 {
        pos2(self.x2, self.y2)
    }
    pub fn rect(&self) -> Rect {
        Rect::from_two_pos(self.start(), self.end())
    }
    pub fn normalize(&mut self) {
        for v in [&mut self.x, &mut self.x2, &mut self.y, &mut self.y2] {
            *v = if v.is_finite() { v.clamp(0., 1.) } else { 0. };
        }
        self.radius = self.radius.clamp(0., 0.5);
        self.opacity = self.opacity.clamp(0., 1.);
        self.stroke = self.stroke.clamp(0.00001, 0.5);
        self.font_size = self.font_size.clamp(0.01, 0.3);
        self.number = self.number.clamp(1, 999);
        self.text = self.text.chars().take(2000).collect();
        self.points.truncate(2048);
        for p in &mut self.points {
            p[0] = p[0].clamp(0., 1.);
            p[1] = p[1].clamp(0., 1.);
        }
    }
    pub fn arrow(&self, size: Vec2) -> Vec<Pos2> {
        let p = pos2(self.x * size.x, self.y * size.y);
        let d = (self.end() - self.start()) * size;
        let len = d.length();
        if len < 0.01 {
            return vec![p];
        }
        let u = d / len;
        let n = Vec2::new(-u.y, u.x);
        let shaft = (self.stroke * size.x).min(len * 0.25);
        let head = (shaft * 4.).max(12.).min(len * 0.6);
        let base = p + d - u * head;
        vec![
            p + n * shaft / 2.,
            base + n * shaft / 2.,
            base + n * head / 2.,
            p + d,
            base - n * head / 2.,
            base - n * shaft / 2.,
            p - n * shaft / 2.,
        ]
    }
    pub fn bounds(&self, size: Vec2) -> Rect {
        if self.tool == Tool::Highlight {
            return Rect::from_min_size(Pos2::ZERO, size);
        }
        let mut r = Rect::from_two_pos(
            pos2(self.x * size.x, self.y * size.y),
            pos2(self.x2 * size.x, self.y2 * size.y),
        );
        if self.tool == Tool::Arrow {
            r = Rect::from_points(&self.arrow(size));
        }
        if self.tool == Tool::Pen && !self.points.is_empty() {
            r = Rect::from_points(
                &self
                    .points
                    .iter()
                    .map(|p| pos2(p[0] * size.x, p[1] * size.y))
                    .collect::<Vec<_>>(),
            );
        }
        if self.tool == Tool::Text {
            let rows = self.text.split('\n').collect::<Vec<_>>();
            r = Rect::from_min_size(
                pos2(self.x * size.x, self.y * size.y),
                Vec2::new(
                    rows.iter()
                        .map(|s| s.chars().count())
                        .max()
                        .unwrap_or(1)
                        .max(1) as f32
                        * self.font_size
                        * size.x,
                    rows.len() as f32 * self.font_size * size.x * 1.35,
                ),
            );
        }
        let pad = 4_f32
            .max(self.stroke * size.x * 3.)
            .max(if self.tool == Tool::Number {
                self.font_size * size.x
            } else {
                0.
            });
        r.expand(pad)
            .intersect(Rect::from_min_size(Pos2::ZERO, size))
    }
    pub fn hit(&self, p: Pos2, size: Vec2) -> bool {
        let p = pos2(p.x * size.x, p.y * size.y);
        let a = pos2(self.x * size.x, self.y * size.y);
        let b = pos2(self.x2 * size.x, self.y2 * size.y);
        let t = 6_f32.max(self.stroke * size.x / 2.);
        match self.tool {
            Tool::Number => p.distance(a) <= 10_f32.max(self.font_size * size.x * 0.7) + t,
            Tool::Text => self.bounds(size).contains(p),
            Tool::Rect | Tool::Mosaic | Tool::Highlight => {
                Rect::from_two_pos(a, b).expand(t).contains(p)
            }
            Tool::Ellipse => {
                let c = a.lerp(b, 0.5);
                let r = (b - a).abs() / 2. + Vec2::splat(t);
                ((p - c) / r).length_sq() <= 1.
            }
            _ => {
                let points = match self.tool {
                    Tool::Arrow => {
                        let mut pts = self.arrow(size);
                        if let Some(first) = pts.first().copied() {
                            pts.push(first);
                        }
                        pts
                    }
                    Tool::Pen => self
                        .points
                        .iter()
                        .map(|p| pos2(p[0] * size.x, p[1] * size.y))
                        .collect(),
                    _ => vec![a, b],
                };
                if self.tool == Tool::Arrow && polygon_contains(&points, p) {
                    return true;
                }
                points
                    .windows(2)
                    .any(|s| segment_distance(p, s[0], s[1]) <= t)
            }
        }
    }
}
fn segment_distance(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let d = b - a;
    let n = ((p - a).dot(d) / d.length_sq().max(1e-12)).clamp(0., 1.);
    p.distance(a + d * n)
}
fn polygon_contains(pts: &[Pos2], p: Pos2) -> bool {
    let mut inside = false;
    for s in pts.windows(2) {
        let (a, b) = (s[0], s[1]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    inside
}
pub fn number_color(color: [u8; 3]) -> [u8; 3] {
    let linear = |v: u8| {
        let v = v as f64 / 255.;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let l = 0.2126 * linear(color[0]) + 0.7152 * linear(color[1]) + 0.0722 * linear(color[2]);
    if (l + 0.05) / 0.05 >= 1.05 / (l + 0.05) {
        [0, 0, 0]
    } else {
        [255, 255, 255]
    }
}
pub fn egui_color(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}
fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
const MISANS: &[u8] = include_bytes!("../assets/assets/fonts/MiSans-VF.ttf");
fn canvas_font_metrics(size: f32) -> (f32, f32) {
    static ASCENT: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
    let ratio = *ASCENT.get_or_init(|| {
        let face = ttf_parser::Face::parse(MISANS, 0).expect("bundled MiSans must be a valid font");
        face.typographic_ascender().unwrap_or(face.ascender()) as f64 / face.units_per_em() as f64
    });
    let size = (size as f64 * 64.).round() / 64.;
    (size as f32, ((size * ratio * 64.).round() / 64.) as f32)
}
struct GlyphPath {
    data: String,
    x: f32,
    y: f32,
}
impl ttf_parser::OutlineBuilder for GlyphPath {
    fn move_to(&mut self, x: f32, y: f32) {
        let _ = write!(self.data, "M{} {} ", x + self.x, y + self.y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let _ = write!(self.data, "L{} {} ", x + self.x, y + self.y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let _ = write!(
            self.data,
            "Q{} {} {} {} ",
            x1 + self.x,
            y1 + self.y,
            x + self.x,
            y + self.y
        );
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let _ = write!(
            self.data,
            "C{} {} {} {} {} {} ",
            x1 + self.x,
            y1 + self.y,
            x2 + self.x,
            y2 + self.y,
            x + self.x,
            y + self.y
        );
    }
    fn close(&mut self) {
        self.data.push('Z');
    }
}
fn number_glyphs(
    number: u32,
    x: f32,
    y: f32,
    size: f32,
    radius: f32,
    color: &str,
) -> Option<String> {
    let mut font = ttf_parser::Face::parse(MISANS, 0).ok()?;
    font.set_variation(ttf_parser::Tag::from_bytes(b"wght"), 600.)?;
    let units = font.units_per_em() as f32;
    let glyphs = number
        .to_string()
        .chars()
        .map(|c| font.glyph_index(c))
        .collect::<Option<Vec<_>>>()?;
    let width = glyphs
        .iter()
        .map(|id| font.glyph_hor_advance(*id).unwrap_or(0) as f32)
        .sum::<f32>();
    let (size, _) = canvas_font_metrics(size.min(radius * 1.6 * units / width.max(1.)));
    let scale = size / units;
    let (_, top) = canvas_font_metrics(size);
    let baseline = y + top - size / 2.;
    let mut path = GlyphPath {
        data: String::new(),
        x: 0.,
        y: 0.,
    };
    for glyph in glyphs {
        font.outline_glyph(glyph, &mut path);
        path.x += font.glyph_hor_advance(glyph).unwrap_or(0) as f32;
    }
    Some(format!(
        "<path d='{}' fill='{color}' transform='translate({} {baseline}) scale({scale} {})'/>",
        path.data,
        x - width * scale / 2.,
        -scale
    ))
}
fn text_glyphs(text: &str, x: f32, y: f32, size: f32, color: &str) -> Option<String> {
    use unicode_script::{Script, UnicodeScript};
    const FONTS: [&[u8]; 4] = [
        MISANS,
        include_bytes!("../assets/assets/fonts/MiSansArabicVF.ttf"),
        include_bytes!("../assets/assets/fonts/MiSans-Devanagari-VF.ttf"),
        include_bytes!("../assets/assets/fonts/MiSansThaiVF.ttf"),
    ];
    let mut fonts = FONTS
        .iter()
        .map(|bytes| ttf_parser::Face::parse(bytes, 0).ok())
        .collect::<Option<Vec<_>>>()?;
    for font in &mut fonts {
        font.set_variation(ttf_parser::Tag::from_bytes(b"wght"), 400.)?;
    }
    // The canvas top baseline comes from the main face's em metrics, including fallback runs.
    let (size, top) = canvas_font_metrics(size);
    let baseline = y + top;
    let bidi = unicode_bidi::BidiInfo::new(text, Some(unicode_bidi::Level::ltr()));
    let mut out = String::new();
    let mut cursor = 0.;
    for paragraph in &bidi.paragraphs {
        let (levels, runs) = bidi.visual_runs(paragraph, paragraph.range.clone());
        for run in runs {
            let rtl = levels[run.start].is_rtl();
            let mut segments: Vec<(usize, usize, usize, Script)> = Vec::new();
            let mut inherited = Script::Latin;
            let mut previous_font = 0;
            for (offset, c) in text[run.clone()].char_indices() {
                let index = run.start + offset;
                let font = if matches!(c as u32, 0x200c | 0x200d | 0xfe0e | 0xfe0f) {
                    previous_font
                } else {
                    fonts
                        .iter()
                        .position(|font| font.glyph_index(c).is_some())?
                };
                let raw_script = c.script();
                let script = if matches!(
                    raw_script,
                    Script::Common | Script::Inherited | Script::Unknown
                ) {
                    inherited
                } else {
                    raw_script
                };
                inherited = script;
                previous_font = font;
                if let Some(segment) = segments.last_mut() {
                    if segment.2 == font && segment.3 == script {
                        segment.1 = index + c.len_utf8();
                        continue;
                    }
                }
                segments.push((index, index + c.len_utf8(), font, script));
            }
            if rtl {
                segments.reverse();
            }
            for (start, end, font_index, _) in segments {
                let font = &fonts[font_index];
                let mut face = rustybuzz::Face::from_slice(FONTS[font_index], 0)?;
                face.set_variations(&[rustybuzz::Variation {
                    tag: ttf_parser::Tag::from_bytes(b"wght"),
                    value: 400.,
                }]);
                let mut buffer = rustybuzz::UnicodeBuffer::new();
                buffer.push_str(&text[start..end]);
                buffer.set_direction(if rtl {
                    rustybuzz::Direction::RightToLeft
                } else {
                    rustybuzz::Direction::LeftToRight
                });
                buffer.guess_segment_properties();
                let shaped = rustybuzz::shape(&face, &[], buffer);
                let scale = size / font.units_per_em() as f32;
                let mut path = GlyphPath {
                    data: String::new(),
                    x: 0.,
                    y: 0.,
                };
                let (mut pen_x, mut pen_y) = (0., 0.);
                for (glyph, position) in shaped.glyph_infos().iter().zip(shaped.glyph_positions()) {
                    path.x = pen_x + position.x_offset as f32;
                    path.y = pen_y + position.y_offset as f32;
                    font.outline_glyph(ttf_parser::GlyphId(glyph.glyph_id as u16), &mut path);
                    pen_x += position.x_advance as f32;
                    pen_y += position.y_advance as f32;
                }
                let _ = write!(
                    out,
                    "<path d='{}' fill='{color}' transform='translate({} {baseline}) scale({scale} {})'/>",
                    path.data,
                    x + cursor,
                    -scale
                );
                cursor += pen_x * scale;
            }
        }
    }
    Some(out)
}
fn svg_annotation(a: &Annotation, w: f32, h: f32) -> String {
    let (x, y, x2, y2) = (a.x * w, a.y * h, a.x2 * w, a.y2 * h);
    let (l, t, bw, bh) = (x.min(x2), y.min(y2), (x2 - x).abs(), (y2 - y).abs());
    let s = a.stroke * w;
    let f = a.font_size * w;
    let c = hex(a.color);
    let r = (a.radius * w).min(bw / 2.).min(bh / 2.);
    match a.tool {
        Tool::Rect => format!(
            "<rect x='{l}' y='{t}' width='{bw}' height='{bh}' rx='{r}' fill='none' stroke='{c}' stroke-width='{s}'/>"
        ),
        Tool::Ellipse => format!(
            "<ellipse cx='{}' cy='{}' rx='{}' ry='{}' fill='none' stroke='{c}' stroke-width='{s}'/>",
            l + bw / 2.,
            t + bh / 2.,
            (bw / 2.).max(0.1),
            (bh / 2.).max(0.1)
        ),
        Tool::Line => format!(
            "<path d='M{x} {y} L{x2} {y2}' fill='none' stroke='{c}' stroke-width='{s}' stroke-linecap='round'/>"
        ),
        Tool::Arrow => {
            let mut path = String::new();
            for (i, p) in a.arrow(Vec2::new(w, h)).iter().enumerate() {
                let _ = write!(path, "{}{} {} ", if i == 0 { 'M' } else { 'L' }, p.x, p.y);
            }
            format!("<path d='{path}Z' fill='{c}'/>")
        }
        Tool::Pen => {
            let mut path = String::new();
            for (i, p) in a.points.iter().enumerate() {
                let _ = write!(
                    path,
                    "{}{} {} ",
                    if i == 0 { 'M' } else { 'L' },
                    p[0] * w,
                    p[1] * h
                );
            }
            if a.points.len() == 1 {
                format!("<circle cx='{x}' cy='{y}' r='{}' fill='{c}'/>", s / 2.)
            } else {
                format!(
                    "<path d='{path}' fill='none' stroke='{c}' stroke-width='{s}' stroke-linecap='round' stroke-linejoin='round'/>"
                )
            }
        }
        Tool::Text => {
            let mut out = String::new();
            for (i, line) in a.text.split('\n').enumerate() {
                if let Some(path) = text_glyphs(line, x, y + i as f32 * f * 1.35, f, &c) {
                    out.push_str(&path);
                    continue;
                }
                let (font_size, top) = canvas_font_metrics(f);
                let _ = write!(
                    out,
                    "<text x='{x}' y='{}' font-size='{font_size}' font-family='MiSans,MiSans Arabic,MiSans Devanagari,MiSans Thai,Microsoft YaHei,sans-serif' fill='{c}'>{}</text>",
                    y + top + i as f32 * f * 1.35,
                    escape(line)
                );
            }
            out
        }
        Tool::Number => {
            let r = 10_f32.max(f * 0.7);
            let tc = hex(a.text_color.unwrap_or_else(|| number_color(a.color)));
            if let Some(text) = number_glyphs(a.number, x, y, f, r, &tc) {
                return format!("<circle cx='{x}' cy='{y}' r='{r}' fill='{c}'/>{text}");
            }
            let fs = f.min(r * 1.6 / (a.number.to_string().len() as f32 * 0.6));
            format!(
                "<circle cx='{x}' cy='{y}' r='{r}' fill='{c}'/><text x='{x}' y='{}' text-anchor='middle' font-family='MiSans,sans-serif' font-weight='600' font-size='{fs}' fill='{tc}'>{}</text>",
                y + fs * 0.35,
                a.number
            )
        }
        _ => String::new(),
    }
}
pub fn svg_options() -> resvg::usvg::Options<'static> {
    static FONTDB: std::sync::OnceLock<std::sync::Arc<resvg::usvg::fontdb::Database>> =
        std::sync::OnceLock::new();
    let db = FONTDB.get_or_init(|| {
        let mut db = resvg::usvg::fontdb::Database::new();
        db.load_system_fonts();
        for font in [
            include_bytes!("../assets/assets/fonts/MiSans-VF.ttf").as_slice(),
            include_bytes!("../assets/assets/fonts/MiSansArabicVF.ttf").as_slice(),
            include_bytes!("../assets/assets/fonts/MiSans-Devanagari-VF.ttf").as_slice(),
            include_bytes!("../assets/assets/fonts/MiSansThaiVF.ttf").as_slice(),
        ] {
            db.load_font_data(font.to_vec());
        }
        std::sync::Arc::new(db)
    });
    resvg::usvg::Options {
        fontdb: db.clone(),
        ..Default::default()
    }
}
fn draw_svg(image: &mut RgbaImage, body: &str) -> Result<(), String> {
    if body.is_empty() {
        return Ok(());
    }
    let (w, h) = image.dimensions();
    let svg =
        format!("<svg xmlns='http://www.w3.org/2000/svg' width='{w}' height='{h}'>{body}</svg>");
    let options = if body.contains("<text") {
        svg_options()
    } else {
        resvg::usvg::Options::default()
    };
    let tree = resvg::usvg::Tree::from_str(&svg, &options).map_err(|e| e.to_string())?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h).ok_or("图片尺寸无效")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    for (dst, src) in image.pixels_mut().zip(pixmap.data().chunks_exact(4)) {
        let alpha = src[3] as f32 / 255.;
        if alpha == 0. {
            continue;
        }
        let da = dst[3] as f32 / 255.;
        let outa = alpha + da * (1. - alpha);
        for c in 0..3 {
            dst[c] = ((src[c] as f32 + dst[c] as f32 * da * (1. - alpha)) / outa)
                .round()
                .clamp(0., 255.) as u8;
        }
        dst[3] = (outa * 255.).round() as u8;
    }
    Ok(())
}
fn rounded_contains(px: f32, py: f32, l: f32, t: f32, w: f32, h: f32, r: f32) -> bool {
    if px < l || py < t || px >= l + w || py >= t + h {
        return false;
    }
    let r = r.min(w / 2.).min(h / 2.).max(0.);
    let dx = (px - (l + r).max(px.min(l + w - r))).abs();
    let dy = (py - (t + r).max(py.min(t + h - r))).abs();
    dx * dx + dy * dy <= r * r
}
pub fn mask(image: &mut RgbaImage, radius: f32) {
    if radius <= 0. {
        return;
    }
    let (w, h) = image.dimensions();
    for (x, y, p) in image.enumerate_pixels_mut() {
        let mut n = 0;
        for oy in [0.25, 0.75] {
            for ox in [0.25, 0.75] {
                if rounded_contains(
                    x as f32 + ox,
                    y as f32 + oy,
                    0.,
                    0.,
                    w as f32,
                    h as f32,
                    radius,
                ) {
                    n += 1;
                }
            }
        }
        p[3] = (p[3] as u16 * n / 4) as u8;
    }
}
fn draw_mosaic(dst: &mut RgbaImage, background: &RgbaImage, a: &Annotation) {
    let (w, h) = dst.dimensions();
    let r = a.rect();
    let (l, t, bw, bh) = (
        r.left() * w as f32,
        r.top() * h as f32,
        r.width() * w as f32,
        r.height() * h as f32,
    );
    if bw < 1. || bh < 1. {
        return;
    }
    let tile = (a.stroke * w as f32 * 4.).max(6.);
    let tw = (bw / tile).ceil().max(1.) as u32;
    let th = (bh / tile).ceil().max(1.) as u32;
    // Canvas drawImage uses bilinear source sampling at fractional pixel centers.
    // Cropping to integer coordinates first loses the fractional origin and shifts every block.
    let small = RgbaImage::from_fn(tw, th, |x, y| {
        let px = (l + (x as f32 + 0.5) * bw / tw as f32 - 0.5).clamp(0., w as f32 - 1.);
        let py = (t + (y as f32 + 0.5) * bh / th as f32 - 0.5).clamp(0., h as f32 - 1.);
        let x0 = px.floor() as u32;
        let y0 = py.floor() as u32;
        let fx = px - x0 as f32;
        let fy = py - y0 as f32;
        let samples = [
            (background.get_pixel(x0, y0), (1. - fx) * (1. - fy)),
            (
                background.get_pixel((x0 + 1).min(w - 1), y0),
                fx * (1. - fy),
            ),
            (
                background.get_pixel(x0, (y0 + 1).min(h - 1)),
                (1. - fx) * fy,
            ),
            (
                background.get_pixel((x0 + 1).min(w - 1), (y0 + 1).min(h - 1)),
                fx * fy,
            ),
        ];
        let alpha = samples
            .iter()
            .map(|(p, weight)| p[3] as f32 * weight)
            .sum::<f32>();
        let mut pixel = [0u8; 4];
        pixel[3] = alpha.round() as u8;
        for c in 0..3 {
            pixel[c] = (samples
                .iter()
                .map(|(p, weight)| p[c] as f32 * p[3] as f32 * weight)
                .sum::<f32>()
                / alpha.max(1e-8))
            .round()
            .clamp(0., 255.) as u8;
        }
        image::Rgba(pixel)
    });
    for y in t as u32..((t + bh).ceil() as u32).min(h) {
        for x in l as u32..((l + bw).ceil() as u32).min(w) {
            if rounded_contains(
                x as f32 + 0.5,
                y as f32 + 0.5,
                l,
                t,
                bw,
                bh,
                a.radius * w as f32,
            ) {
                let sx = ((((x as f32 + 0.5) - l) / bw * tw as f32) as u32).min(tw - 1);
                let sy = ((((y as f32 + 0.5) - t) / bh * th as f32) as u32).min(th - 1);
                dst.put_pixel(x, y, *small.get_pixel(sx, sy));
            }
        }
    }
}
fn draw_highlights(dst: &mut RgbaImage, items: &[&Annotation]) {
    let Some(style) = items.last() else {
        return;
    };
    let (w, h) = dst.dimensions();
    let alpha = style.opacity;
    for (x, y, p) in dst.enumerate_pixels_mut() {
        let inside = items.iter().any(|a| {
            let r = a.rect();
            rounded_contains(
                x as f32 + 0.5,
                y as f32 + 0.5,
                r.left() * w as f32,
                r.top() * h as f32,
                r.width() * w as f32,
                r.height() * h as f32,
                a.radius * w as f32,
            )
        });
        if !inside {
            let da = p[3] as f32 / 255.;
            let oa = alpha + da * (1. - alpha);
            for c in 0..3 {
                p[c] = ((style.color[c] as f32 * alpha + p[c] as f32 * da * (1. - alpha))
                    / oa.max(1e-8))
                .round() as u8;
            }
            p[3] = (oa * 255.).round() as u8;
        }
    }
}
pub fn render(base: &RgbaImage, items: &[Annotation], radius: f32) -> Result<RgbaImage, String> {
    let mut out = base.clone();
    mask(&mut out, radius);
    if items.is_empty() {
        return Ok(out);
    }
    for a in items.iter().filter(|a| a.tool == Tool::Mosaic) {
        let source = out.clone();
        draw_mosaic(&mut out, &source, a);
    }
    draw_highlights(
        &mut out,
        &items
            .iter()
            .filter(|a| a.tool == Tool::Highlight)
            .collect::<Vec<_>>(),
    );
    let body = items
        .iter()
        .filter(|a| a.tool.rank() == 2)
        .map(|a| svg_annotation(a, out.width() as f32, out.height() as f32))
        .collect::<String>();
    draw_svg(&mut out, &body)?;
    mask(&mut out, radius);
    Ok(out)
}

pub struct Layer {
    pub name: String,
    pub left: u32,
    pub top: u32,
    pub image: RgbaImage,
    pub hidden: bool,
}
pub fn export_psd(
    original: &RgbaImage,
    filtered: &RgbaImage,
    items: &[Annotation],
    radius: f32,
) -> Result<Vec<u8>, String> {
    let (w, h) = original.dimensions();
    let mut composite = filtered.clone();
    mask(&mut composite, radius);
    let mut adjusted = filtered.clone();
    mask(&mut adjusted, radius);
    let mut layers = vec![
        Layer {
            name: "原图".into(),
            left: 0,
            top: 0,
            image: original.clone(),
            hidden: true,
        },
        Layer {
            name: "调整后的图片".into(),
            left: 0,
            top: 0,
            image: adjusted,
            hidden: false,
        },
    ];
    for a in items.iter().filter(|a| a.tool == Tool::Mosaic) {
        let mut layer = RgbaImage::new(w, h);
        draw_mosaic(&mut layer, &composite, a);
        mask(&mut layer, radius);
        image::imageops::overlay(&mut composite, &layer, 0, 0);
        let name = format!("{} mosaic", layers.len() - 1);
        layers.push(crop_layer(
            layer,
            a.bounds(Vec2::new(w as f32, h as f32)),
            &name,
        ));
    }
    let highlights = items
        .iter()
        .filter(|a| a.tool == Tool::Highlight)
        .collect::<Vec<_>>();
    if !highlights.is_empty() {
        let mut layer = RgbaImage::new(w, h);
        draw_highlights(&mut layer, &highlights);
        mask(&mut layer, radius);
        image::imageops::overlay(&mut composite, &layer, 0, 0);
        layers.push(Layer {
            name: format!("{} highlight-group", layers.len() - 1),
            left: 0,
            top: 0,
            image: layer,
            hidden: false,
        });
    }
    for a in items.iter().filter(|a| a.tool.rank() == 2) {
        let mut layer = RgbaImage::new(w, h);
        draw_svg(&mut layer, &svg_annotation(a, w as f32, h as f32))?;
        mask(&mut layer, radius);
        image::imageops::overlay(&mut composite, &layer, 0, 0);
        let name = format!(
            "{} {}",
            layers.len() - 1,
            if a.tool == Tool::Text {
                a.text.chars().take(30).collect::<String>()
            } else {
                a.tool.key().into()
            }
        );
        layers.push(crop_layer(
            layer,
            a.bounds(Vec2::new(w as f32, h as f32)),
            &name,
        ));
    }
    mask(&mut composite, radius);
    encode_psd(&layers, &composite)
}
fn crop_layer(image: RgbaImage, r: Rect, name: &str) -> Layer {
    let l = r.left().floor().max(0.) as u32;
    let t = r.top().floor().max(0.) as u32;
    let w = (r.right().ceil() as u32)
        .saturating_sub(l)
        .max(1)
        .min(image.width() - l.min(image.width() - 1));
    let h = (r.bottom().ceil() as u32)
        .saturating_sub(t)
        .max(1)
        .min(image.height() - t.min(image.height() - 1));
    let cropped = image::imageops::crop_imm(
        &image,
        l.min(image.width() - 1),
        t.min(image.height() - 1),
        w,
        h,
    )
    .to_image();
    Layer {
        name: name.into(),
        left: l,
        top: t,
        image: cropped,
        hidden: false,
    }
}
pub fn encode_psd(layers: &[Layer], composite: &RgbaImage) -> Result<Vec<u8>, String> {
    fn u16b(b: &mut Vec<u8>, v: u16) {
        b.extend(v.to_be_bytes());
    }
    fn u32b(b: &mut Vec<u8>, v: u32) {
        b.extend(v.to_be_bytes());
    }
    let (w, h) = composite.dimensions();
    if w == 0 || h == 0 || w > 30000 || h > 30000 || layers.len() > 32767 {
        return Err("PSD 尺寸或图层数量超限".into());
    }
    let estimate = layers
        .iter()
        .map(|l| l.image.as_raw().len() + 256)
        .sum::<usize>()
        + composite.as_raw().len()
        + 128;
    if estimate > 192 * 1024 * 1024 {
        return Err("PSD 图层数据过大，请减少图片尺寸或大范围标注".into());
    }
    let mut info = Vec::with_capacity(estimate);
    u16b(&mut info, (-(layers.len() as i16)) as u16);
    for l in layers.iter().rev() {
        let (lw, lh) = l.image.dimensions();
        u32b(&mut info, l.top);
        u32b(&mut info, l.left);
        u32b(&mut info, l.top + lh);
        u32b(&mut info, l.left + lw);
        u16b(&mut info, 4);
        for id in [0i16, 1, 2, -1] {
            u16b(&mut info, id as u16);
            u32b(&mut info, lw * lh + 2);
        }
        info.extend(b"8BIMnorm");
        info.extend([255, 0, if l.hidden { 2 } else { 0 }, 0]);
        let mut extra = Vec::new();
        u32b(&mut extra, 0);
        u32b(&mut extra, 0);
        extra.extend([5, b'L', b'a', b'y', b'e', b'r', 0, 0]);
        let unicode = l.name.encode_utf16().collect::<Vec<_>>();
        extra.extend(b"8BIMluni");
        u32b(&mut extra, (4 + unicode.len() * 2) as u32);
        u32b(&mut extra, unicode.len() as u32);
        for c in unicode {
            u16b(&mut extra, c);
        }
        u32b(&mut info, extra.len() as u32);
        info.extend(extra);
    }
    for l in layers.iter().rev() {
        for c in 0..4 {
            u16b(&mut info, 0);
            info.extend(l.image.pixels().map(|p| p[c]));
        }
    }
    if info.len() % 2 == 1 {
        info.push(0);
    }
    let mut out = Vec::with_capacity(estimate);
    out.extend(b"8BPS");
    u16b(&mut out, 1);
    out.extend([0; 6]);
    u16b(&mut out, 4);
    u32b(&mut out, h);
    u32b(&mut out, w);
    u16b(&mut out, 8);
    u16b(&mut out, 3);
    u32b(&mut out, 0);
    u32b(&mut out, 0);
    u32b(&mut out, (info.len() + 8) as u32);
    u32b(&mut out, info.len() as u32);
    out.extend(info);
    u32b(&mut out, 0);
    u16b(&mut out, 0);
    for c in 0..4 {
        out.extend(composite.pixels().map(|p| p[c]));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn highlighter_union_leaves_both_regions_clear() {
        let base = RgbaImage::from_pixel(20, 10, Rgba([255, 255, 255, 255]));
        let mut a = Annotation::new(1, Tool::Highlight, pos2(0.1, 0.1), 20.);
        a.x2 = 0.4;
        a.y2 = 0.8;
        let mut b = a.clone();
        b.x = 0.6;
        b.x2 = 0.9;
        let out = render(&base, &[a, b], 0.).unwrap();
        assert_eq!(out.get_pixel(4, 4)[0], 255);
        assert_eq!(out.get_pixel(14, 4)[0], 255);
        assert!(out.get_pixel(10, 4)[0] < 130);
    }
    #[test]
    fn psd_header_and_unicode_layer() {
        let im = RgbaImage::from_pixel(2, 1, Rgba([1, 2, 3, 255]));
        let out = encode_psd(
            &[Layer {
                name: "原图".into(),
                left: 0,
                top: 0,
                image: im.clone(),
                hidden: true,
            }],
            &im,
        )
        .unwrap();
        assert_eq!(&out[..4], b"8BPS");
        assert_eq!(&out[14..18], &1u32.to_be_bytes());
        assert!(out.windows(4).any(|s| s == b"luni"));
        assert_eq!(&out[out.len() - 8..], &[1, 1, 2, 2, 3, 3, 255, 255]);
    }
    #[test]
    fn translate_stays_inside() {
        let mut a = Annotation::new(1, Tool::Rect, pos2(0.2, 0.3), 100.);
        a.x2 = 0.8;
        a.y2 = 0.9;
        let b = crate::geometry::translate(&a, Vec2::new(2., -2.));
        assert!((b.x2 - 1.).abs() < 1e-6);
        assert_eq!(b.y, 0.);
    }
    #[test]
    fn square_uses_pixel_aspect() {
        let p = crate::geometry::square(pos2(0.1, 0.1), pos2(0.3, 0.5), Vec2::new(200., 100.));
        assert!(((p.x - 0.1) * 200. - (p.y - 0.1) * 100.).abs() < 1e-5);
    }
    #[test]
    fn round_mask_has_transparent_corner() {
        let mut im = RgbaImage::from_pixel(20, 20, Rgba([200, 100, 50, 255]));
        mask(&mut im, 8.);
        assert_eq!(im.get_pixel(0, 0)[3], 0);
        assert_eq!(im.get_pixel(10, 10)[3], 255);
    }
    #[test]
    fn canvas_font_em_metrics_match_all_captured_sizes() {
        let oracle: serde_json::Value =
            serde_json::from_str(include_str!("../tests/annotation-pixels.json")).unwrap();
        for sample in oracle["fontMetrics"].as_array().unwrap() {
            let size = sample["size"].as_f64().unwrap() as f32;
            let (quantized, top) = canvas_font_metrics(size);
            let metrics = sample["metrics"].as_array().unwrap();
            let alpha = metrics[0]["ascent"].as_f64().unwrap() as f32;
            let expected_top = alpha - metrics[1]["ascent"].as_f64().unwrap() as f32;
            let expected_middle = alpha - metrics[2]["ascent"].as_f64().unwrap() as f32;
            assert!(
                (top - expected_top).abs() < 0.001,
                "font size {size}: top baseline {top} != {expected_top}"
            );
            assert!(
                (top - quantized / 2. - expected_middle).abs() < 0.001,
                "font size {size}: middle baseline"
            );
        }
    }
    #[test]
    fn canvas_text_calibration() {
        let oracle: serde_json::Value =
            serde_json::from_str(include_str!("../tests/annotation-pixels.json")).unwrap();
        let expected = include_bytes!("../tests/annotation-pixels.bin");
        let (w, h) = (128u32, 96u32);
        let stride = (w * h * 4) as usize;
        let base = RgbaImage::from_fn(w, h, |x, y| Rgba([(x * 2) as u8, (y * 2) as u8, 120, 255]));
        for index in [5usize, 6usize] {
            let v = &oracle["items"][index];
            let mut a = Annotation::new(
                1,
                if index == 5 { Tool::Text } else { Tool::Number },
                pos2(
                    v["x"].as_f64().unwrap() as f32,
                    v["y"].as_f64().unwrap() as f32,
                ),
                w as f32,
            );
            a.font_size = v["fontSize"].as_f64().unwrap() as f32;
            a.text = v["text"].as_str().unwrap().into();
            a.number = v["number"].as_u64().unwrap() as u32;
            let body = svg_annotation(&a, w as f32, h as f32);
            let expected = &expected[index * stride..(index + 1) * stride];
            let mut best = (f64::MAX, 0.);
            for step in -20..=20 {
                let dy = step as f32 / 20.;
                let body = if let Some(start) = body.find("<path ") {
                    format!(
                        "{}<g transform='translate(0 {dy})'>{}</g>",
                        &body[..start],
                        &body[start..]
                    )
                } else {
                    body.replace("<text ", &format!("<text dy='{dy}' "))
                };
                let mut out = base.clone();
                draw_svg(&mut out, &body).unwrap();
                let sum: u64 = out
                    .as_raw()
                    .iter()
                    .zip(expected)
                    .map(|(a, b)| a.abs_diff(*b) as u64)
                    .sum();
                let mae = sum as f64 / stride as f64;
                if mae < best.0 {
                    best = (mae, dy);
                }
            }
            println!(
                "Canvas baseline {:?}: best dy={} px, MAE={:.5}/255",
                a.tool, best.1, best.0
            );
            assert!(best.0 < 0.6);
        }
    }
}
