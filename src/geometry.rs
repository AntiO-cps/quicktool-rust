//! Normalized annotation geometry. Values stay independent of preview resolution.
use crate::annotations::{Annotation, Tool};
use eframe::egui::{Pos2, Vec2, pos2};

pub fn direction(delta: Vec2) -> Vec2 {
    let angle = (delta.y.atan2(delta.x) / std::f32::consts::FRAC_PI_4).round()
        * std::f32::consts::FRAC_PI_4;
    Vec2::angled(angle) * delta.length()
}
pub fn snap(anchor: Pos2, point: Pos2, size: Vec2) -> Pos2 {
    let d = direction((point - anchor) * size);
    let mut length = d.length();
    if length < 1e-8 {
        return anchor;
    }
    let u = d / length;
    if u.x.abs() > 1e-8 {
        length = length.min(if u.x > 0. {
            (1. - anchor.x) * size.x / u.x
        } else {
            -anchor.x * size.x / u.x
        });
    }
    if u.y.abs() > 1e-8 {
        length = length.min(if u.y > 0. {
            (1. - anchor.y) * size.y / u.y
        } else {
            -anchor.y * size.y / u.y
        });
    }
    anchor + u * length / size
}
pub fn square(anchor: Pos2, point: Pos2, size: Vec2) -> Pos2 {
    let d = (point - anchor) * size;
    let sx = if d.x < 0. { -1. } else { 1. };
    let sy = if d.y < 0. { -1. } else { 1. };
    let side =
        d.x.abs()
            .max(d.y.abs())
            .min(if sx > 0. {
                (1. - anchor.x) * size.x
            } else {
                anchor.x * size.x
            })
            .min(if sy > 0. {
                (1. - anchor.y) * size.y
            } else {
                anchor.y * size.y
            });
    anchor + Vec2::new(sx * side / size.x, sy * side / size.y)
}
pub fn handles(a: &Annotation) -> Vec<(&'static str, Pos2)> {
    if matches!(a.tool, Tool::Line | Tool::Arrow) {
        return vec![
            ("start", a.start()),
            ("middle", a.start().lerp(a.end(), 0.5)),
            ("end", a.end()),
        ];
    }
    let r = a.rect();
    let c = r.center();
    vec![
        ("nw", r.left_top()),
        ("n", pos2(c.x, r.top())),
        ("ne", r.right_top()),
        ("e", pos2(r.right(), c.y)),
        ("se", r.right_bottom()),
        ("s", pos2(c.x, r.bottom())),
        ("sw", r.left_bottom()),
        ("w", pos2(r.left(), c.y)),
    ]
}
pub fn translate(a: &Annotation, delta: Vec2) -> Annotation {
    let mut b = a.clone();
    let mut r = a.rect();
    if a.tool == Tool::Pen && !a.points.is_empty() {
        r = eframe::egui::Rect::from_points(
            &a.points
                .iter()
                .map(|p| pos2(p[0], p[1]))
                .collect::<Vec<_>>(),
        );
    }
    let d = Vec2::new(
        delta.x.clamp(-r.left(), 1. - r.right()),
        delta.y.clamp(-r.top(), 1. - r.bottom()),
    );
    b.x += d.x;
    b.x2 += d.x;
    b.y += d.y;
    b.y2 += d.y;
    for p in &mut b.points {
        p[0] += d.x;
        p[1] += d.y;
    }
    b
}
pub fn resize(
    a: &Annotation,
    handle: &str,
    p: Pos2,
    size: Vec2,
    shift: bool,
    start: Pos2,
) -> Annotation {
    if handle == "middle" {
        let d = p - start;
        return translate(a, if shift { direction(d * size) / size } else { d });
    }
    let mut b = a.clone();
    if matches!(a.tool, Tool::Line | Tool::Arrow) {
        let anchor = if handle == "start" {
            a.end()
        } else {
            a.start()
        };
        let p = if shift { snap(anchor, p, size) } else { p };
        if handle == "start" {
            b.x = p.x;
            b.y = p.y;
        } else {
            b.x2 = p.x;
            b.y2 = p.y;
        }
        return b;
    }
    let r = a.rect();
    let (west, east, north, south) = (
        handle.contains('w'),
        handle.contains('e'),
        handle.contains('n'),
        handle.contains('s'),
    );
    let (mut l, mut rr, mut t, mut bot) = (r.left(), r.right(), r.top(), r.bottom());
    if west {
        l = p.x;
    }
    if east {
        rr = p.x;
    }
    if north {
        t = p.y;
    }
    if south {
        bot = p.y;
    }
    if shift && r.width() > 1e-6 && r.height() > 1e-6 {
        let x = if west {
            r.right()
        } else if east {
            r.left()
        } else {
            r.center().x
        };
        let y = if north {
            r.bottom()
        } else if south {
            r.top()
        } else {
            r.center().y
        };
        let mut scale = if (west || east) && (north || south) {
            ((p.x - x).abs() / r.width()).max((p.y - y).abs() / r.height())
        } else if west || east {
            (p.x - x).abs() / r.width()
        } else {
            (p.y - y).abs() / r.height()
        };
        let ax = if west {
            x
        } else if east {
            1. - x
        } else {
            x.min(1. - x) * 2.
        };
        let ay = if north {
            y
        } else if south {
            1. - y
        } else {
            y.min(1. - y) * 2.
        };
        scale = scale.min(ax / r.width()).min(ay / r.height()).max(0.001);
        let w = r.width() * scale;
        let h = r.height() * scale;
        l = if west {
            x - w
        } else if east {
            x
        } else {
            x - w / 2.
        };
        rr = l + w;
        t = if north {
            y - h
        } else if south {
            y
        } else {
            y - h / 2.
        };
        bot = t + h;
    }
    b.x = l.min(rr);
    b.x2 = l.max(rr);
    b.y = t.min(bot);
    b.y2 = t.max(bot);
    b.radius = b
        .radius
        .min(((rr - l).abs() * size.x).min((bot - t).abs() * size.y) / (2. * size.x));
    b
}

/// Original pin corner resize rules, excluding the transparent 8 px margin.
pub fn resize_window(
    start: eframe::egui::Rect,
    corner: &str,
    start_pointer: Pos2,
    pointer: Pos2,
    natural: Option<Vec2>,
    editing: bool,
    shift: bool,
) -> eframe::egui::Rect {
    let west = corner.contains('w');
    let north = corner.contains('n');
    let dx = pointer.x - start_pointer.x;
    let dy = pointer.y - start_pointer.y;
    let mut width = start.width() + if west { -dx } else { dx };
    let mut height = start.height() + if north { -dy } else { dy };
    if let Some(natural) = natural {
        let ratio = natural.x / natural.y;
        let inset = if editing { 64. } else { 16. };
        if dx.abs() >= dy.abs() * ratio {
            height = (width - 16.) / ratio + inset;
        } else {
            width = (height - inset) * ratio + 16.;
        }
        if width < 316. {
            width = 316.;
            height = (width - 16.) / ratio + inset;
        }
        if height < 100. + inset {
            height = 100. + inset;
            width = (height - inset) * ratio + 16.;
        }
        let factor = 1_f32
            .min(2384. / (width - 16.))
            .min(1984. / (height - inset));
        width = (width - 16.) * factor + 16.;
        height = (height - inset) * factor + inset;
    } else if shift {
        let cw = start.width() - 16.;
        let ch = start.height() - 16.;
        let factor = ((width - 16.) / cw).max((height - 16.) / ch);
        let scale = (200. / cw)
            .max(120. / ch)
            .max(factor.min(2384. / cw).min(1984. / ch));
        width = cw * scale + 16.;
        height = ch * scale + 16.;
    } else {
        width = width.clamp(216., 2400.);
        height = height.clamp(136., 2000.);
    }
    let x = if west {
        start.right() - width
    } else {
        start.left()
    };
    let y = if north {
        start.bottom() - height
    } else {
        start.top()
    };
    eframe::egui::Rect::from_min_size(pos2(x, y), Vec2::new(width, height))
}
