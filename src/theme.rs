//! The launcher glow is a separate, non-activating native viewport.
//! Geometry stays outside the rounded panel, so the panel interior is a true cutout.
use eframe::egui::*;
use std::time::Duration;

pub struct Glow {
    contours: Vec<Vec<Pos2>>,
    alpha: Vec<[f32; 3]>,
    panel_size: Vec2,
    created: bool,
    pub owner: isize,
    pub error: Option<String>,
    warned: bool,
}
impl Default for Glow {
    fn default() -> Self {
        Self {
            contours: vec![],
            alpha: vec![],
            panel_size: Vec2::ZERO,
            created: false,
            owner: 0,
            error: None,
            warned: false,
        }
    }
}
const MARGIN: f32 = 160.;
const TITLE: &str = "QuickTool Rust · Edge Glow";

fn cdf(x: f32) -> f32 {
    // Abramowitz–Stegun normal CDF; max absolute approximation error < 8e-8.
    let z = x.abs();
    let t = 1. / (1. + 0.2316419 * z);
    let polynomial =
        t * (0.31938153 + t * (-0.35656378 + t * (1.7814779 + t * (-1.8212559 + t * 1.3302745))));
    let p = 1. - 0.3989423 * (-z * z * 0.5).exp() * polynomial;
    if x < 0. { 1. - p } else { p }
}
fn ring_alpha(distance: f32, width: f32, sigma: f32) -> f32 {
    (cdf((width - distance) / sigma) - cdf(-distance / sigma)).clamp(0., 1.)
}
fn contour(size: Vec2, distance: f32) -> Vec<Pos2> {
    let radius = 8. + distance;
    let mut points = Vec::new();
    let centers = [
        pos2(size.x - 8., 8.),
        pos2(size.x - 8., size.y - 8.),
        pos2(8., size.y - 8.),
        pos2(8., 8.),
    ];
    for (corner, center) in centers.iter().enumerate() {
        let start = -std::f32::consts::FRAC_PI_2 + corner as f32 * std::f32::consts::FRAC_PI_2;
        for sample in 0..=24 {
            let angle = start + sample as f32 / 24. * std::f32::consts::FRAC_PI_2;
            points.push(*center + radius * vec2(angle.cos(), angle.sin()) + vec2(MARGIN, MARGIN));
        }
        let end = *points.last().unwrap();
        let next_center = centers[(corner + 1) % 4];
        let angle = start + std::f32::consts::FRAC_PI_2;
        let next = next_center + radius * vec2(angle.cos(), angle.sin()) + vec2(MARGIN, MARGIN);
        let count = (end.distance(next) / 24.).ceil().max(1.) as usize;
        for step in 1..count {
            points.push(end.lerp(next, step as f32 / count as f32));
        }
    }
    points
}
fn orbit_color(point: Pos2, center: Pos2, phase: f32) -> [f32; 4] {
    let t = ((point.x - center.x).atan2(center.y - point.y) / std::f32::consts::TAU - phase)
        .rem_euclid(1.);
    let stops = [
        (0., [119., 181., 255., 0.95]),
        (0.25, [220., 138., 255., 0.94]),
        (0.48, [250., 148., 211., 0.92]),
        (0.75, [112., 227., 245., 0.94]),
        (1., [119., 181., 255., 0.95]),
    ];
    for pair in stops.windows(2) {
        if t <= pair[1].0 {
            let f = (t - pair[0].0) / (pair[1].0 - pair[0].0);
            return std::array::from_fn(|i| pair[0].1[i] + (pair[1].1[i] - pair[0].1[i]) * f);
        }
    }
    stops[0].1
}
impl Glow {
    fn rebuild(&mut self, size: Vec2) {
        self.panel_size = size;
        self.contours.clear();
        self.alpha.clear();
        // Extra half-pixel contours preserve the crisp 2px rim; the wider bands
        // use smooth Gaussian coverage, without filtering a full desktop bitmap.
        for d in [
            0., 0.5, 1., 1.5, 2., 3., 4., 6., 8., 10., 14., 18., 24., 32., 44., 60., 80., 110.,
            140., 160.,
        ] {
            self.contours.push(contour(size, d));
            self.alpha.push([
                ring_alpha(d, 8., 18.),
                ring_alpha(d, 8., 38.),
                ring_alpha(d, 2., 1.) + 0.9 * ring_alpha(d, 2., 8.),
            ]);
        }
    }
    pub fn show(
        &mut self,
        ctx: &Context,
        visible: bool,
        topmost: bool,
        light: bool,
        strength: f32,
        reduce_motion: bool,
    ) {
        let visible = visible && strength > 0.;
        if !visible && !self.created {
            return;
        }
        let panel = ctx
            .input(|i| i.viewport().outer_rect)
            .unwrap_or_else(|| Rect::from_min_size(Pos2::ZERO, ctx.screen_rect().size()));
        let size = ctx.screen_rect().size();
        if size != self.panel_size {
            self.rebuild(size);
        }
        let id = ViewportId::from_hash_of("quicktool-edge-glow");
        let builder = ViewportBuilder::default()
            .with_title(TITLE)
            .with_inner_size(size + vec2(MARGIN * 2., MARGIN * 2.))
            .with_position(panel.min - vec2(MARGIN, MARGIN))
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
            .with_active(false)
            .with_taskbar(false)
            .with_mouse_passthrough(true)
            .with_visible(visible)
            .with_window_level(if topmost {
                WindowLevel::AlwaysOnTop
            } else {
                WindowLevel::Normal
            });
        self.created = true;
        ctx.show_viewport_immediate(id, builder, |glow_ctx, _| {
            glow_ctx.send_viewport_cmd(ViewportCommand::Visible(visible));
            if !visible {
                return;
            }
            let time = if reduce_motion {
                0.
            } else {
                glow_ctx.input(|i| i.time) as f32
            };
            let center = pos2(MARGIN + size.x * 0.5, MARGIN + size.y * 0.5);
            let painter = glow_ctx.layer_painter(LayerId::background());
            for (layer, seconds, opacity) in [
                (0, 30., if light { 0.85 } else { 0.95 }),
                (1, -36., 1.),
                (2, 24., 0.8),
            ] {
                let mut mesh = Mesh::default();
                for (band, points) in self.contours.iter().enumerate() {
                    for point in points {
                        let color = orbit_color(*point, center, time / seconds);
                        let alpha =
                            (self.alpha[band][layer] * opacity * strength * color[3]).clamp(0., 1.);
                        mesh.colored_vertex(
                            *point,
                            Color32::from_rgba_unmultiplied(
                                color[0] as u8,
                                color[1] as u8,
                                color[2] as u8,
                                (alpha * 255.).round() as u8,
                            ),
                        );
                    }
                }
                let count = self.contours[0].len() as u32;
                for band in 0..self.contours.len() - 1 {
                    for point in 0..count {
                        let next = (point + 1) % count;
                        let a = band as u32 * count + point;
                        let b = band as u32 * count + next;
                        let c = a + count;
                        let d = b + count;
                        mesh.add_triangle(a, b, c);
                        mesh.add_triangle(b, d, c);
                    }
                }
                painter.add(mesh);
            }
            if !reduce_motion {
                glow_ctx.request_repaint_after(Duration::from_millis(33));
            }
        });
        #[cfg(windows)]
        if self.owner != 0 {
            let result =
                quicktool_rust::platform::configure_glow(TITLE, self.owner, visible, topmost);
            if let Err(error) = result {
                if !self.warned {
                    self.error = Some(error);
                    self.warned = true;
                }
            } else {
                self.warned = false;
                if let Some(window) = quicktool_rust::platform::find_window(TITLE) {
                    let _ = quicktool_rust::platform::place_companion_window(
                        self.owner,
                        window,
                        (MARGIN * ctx.pixels_per_point()).round() as i32,
                        visible,
                        topmost,
                        true,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn glow_geometry_never_covers_panel_interior() {
        let size = vec2(812., 84.);
        let mut glow = Glow::default();
        glow.rebuild(size);
        let inner = Rect::from_min_max(
            pos2(MARGIN + 8., MARGIN),
            pos2(MARGIN + size.x - 8., MARGIN + size.y),
        );
        let inner = inner.shrink(0.001);
        for points in &glow.contours {
            for point in points {
                assert!(
                    !(point.x > inner.left()
                        && point.x < inner.right()
                        && point.y > inner.top()
                        && point.y < inner.bottom())
                );
            }
        }
        assert!(glow.contours.windows(2).all(|p| p[0].len() == p[1].len()));
    }
    #[test]
    fn gaussian_bloom_fades_at_margin() {
        assert!(ring_alpha(160., 8., 38.) < 0.0001);
        assert!(ring_alpha(4., 8., 18.) > ring_alpha(80., 8., 18.));
    }
}
