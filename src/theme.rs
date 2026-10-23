//! The launcher glow is a separate, non-activating native viewport.
//! Geometry stays outside the rounded panel, so the panel interior is a true cutout.
use eframe::egui::*;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    time::Duration,
};

pub struct Glow {
    contours: Arc<Vec<Vec<Pos2>>>,
    alpha: Arc<Vec<[f32; 3]>>,
    angles: Arc<Vec<f32>>,
    indices: Arc<Vec<u32>>,
    panel_size: Vec2,
    created: bool,
    pub owner: isize,
    pub error: Option<String>,
    native_status: Arc<Mutex<(Option<String>, bool)>>,
    native_placement: Arc<Mutex<Option<NativePlacement>>>,
    qa_path: Option<std::path::PathBuf>,
    qa_frames: Arc<AtomicU32>,
    qa_sent: Arc<AtomicBool>,
    qa_done: Arc<AtomicBool>,
}
impl Default for Glow {
    fn default() -> Self {
        Self {
            contours: Arc::new(vec![]),
            alpha: Arc::new(vec![]),
            angles: Arc::new(vec![]),
            indices: Arc::new(vec![]),
            panel_size: Vec2::ZERO,
            created: false,
            owner: 0,
            error: None,
            native_status: Arc::new(Mutex::new((None, false))),
            native_placement: Arc::new(Mutex::new(None)),
            qa_path: std::env::args()
                .skip_while(|arg| arg != "--glow-screenshot")
                .nth(1)
                .map(std::path::PathBuf::from),
            qa_frames: Arc::new(AtomicU32::new(0)),
            qa_sent: Arc::new(AtomicBool::new(false)),
            qa_done: Arc::new(AtomicBool::new(false)),
        }
    }
}
const MARGIN: f32 = 160.;
const TITLE: &str = "QuickTool Rust · Edge Glow";
const FRAME_INTERVAL: Duration = Duration::from_nanos(1_000_000_000 / 60);

#[derive(Clone, Copy, PartialEq)]
struct NativePlacement {
    owner: isize,
    panel: Rect,
    size: Vec2,
    pixels_per_point: f32,
    topmost: bool,
    focused: bool,
}

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
fn orbit_color(angle: f32, phase: f32) -> [f32; 4] {
    let t = (angle - phase).rem_euclid(1.);
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
        // The previous deferred callback can still own the old geometry. Build
        // new vectors directly rather than clone that geometry only to clear it.
        let mut contours = Vec::with_capacity(20);
        let mut coverage = Vec::with_capacity(20);
        // Extra half-pixel contours preserve the crisp 2px rim; the wider bands
        // use smooth Gaussian coverage, without filtering a full desktop bitmap.
        for d in [
            0., 0.5, 1., 1.5, 2., 3., 4., 6., 8., 10., 14., 18., 24., 32., 44., 60., 80., 110.,
            140., 160.,
        ] {
            contours.push(contour(size, d));
            coverage.push([
                ring_alpha(d, 8., 18.),
                ring_alpha(d, 8., 38.),
                ring_alpha(d, 2., 1.) + 0.9 * ring_alpha(d, 2., 8.),
            ]);
        }
        self.contours = Arc::new(contours);
        self.alpha = Arc::new(coverage);
        let center = pos2(MARGIN + size.x * 0.5, MARGIN + size.y * 0.5);
        self.angles = Arc::new(
            self.contours
                .iter()
                .flatten()
                .map(|point| (point.x - center.x).atan2(center.y - point.y) / std::f32::consts::TAU)
                .collect(),
        );
        let mut indices = Vec::new();
        let count = self.contours[0].len() as u32;
        indices.reserve((self.contours.len() - 1) * count as usize * 6);
        for band in 0..self.contours.len() - 1 {
            for point in 0..count {
                let next = (point + 1) % count;
                let a = band as u32 * count + point;
                let b = band as u32 * count + next;
                indices.extend_from_slice(&[a, b, a + count, b, b + count, a + count]);
            }
        }
        self.indices = Arc::new(indices);
    }
    #[allow(clippy::too_many_arguments)] // Visual height intentionally differs from HWND allocation.
    pub fn show(
        &mut self,
        ctx: &Context,
        visible: bool,
        topmost: bool,
        light: bool,
        strength: f32,
        reduce_motion: bool,
        visual_size: Vec2,
    ) {
        if let Ok(mut status) = self.native_status.lock() {
            if let Some(error) = status.0.take() {
                self.error = Some(error);
            }
        }
        let visible = visible && strength > 0.;
        if !visible {
            if let Ok(mut placement) = self.native_placement.lock() {
                *placement = None;
            }
        }
        if !visible && !self.created {
            return;
        }
        let panel = ctx
            .input(|i| i.viewport().outer_rect)
            .unwrap_or_else(|| Rect::from_min_size(Pos2::ZERO, ctx.screen_rect().size()));
        let allocated_size = ctx.screen_rect().size();
        let size = visual_size;
        if size != self.panel_size {
            self.rebuild(size);
        }
        let id = ViewportId::from_hash_of("quicktool-edge-glow");
        let builder = ViewportBuilder::default()
            .with_title(TITLE)
            .with_inner_size(allocated_size + vec2(MARGIN * 2., MARGIN * 2.))
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
        let contours = self.contours.clone();
        let alpha = self.alpha.clone();
        let angles = self.angles.clone();
        let indices = self.indices.clone();
        let native_status = self.native_status.clone();
        let native_placement = self.native_placement.clone();
        let owner = self.owner;
        let focused = ctx.input(|input| input.viewport().focused.unwrap_or(false));
        let qa_path = self.qa_path.clone();
        let qa_frames = self.qa_frames.clone();
        let qa_sent = self.qa_sent.clone();
        let qa_done = self.qa_done.clone();
        ctx.show_viewport_deferred(id, builder, move |glow_ctx, _| {
            glow_ctx.send_viewport_cmd(ViewportCommand::Visible(visible));
            if !visible {
                if let Ok(mut placement) = native_placement.lock() {
                    *placement = None;
                }
                return;
            }
            let time = if reduce_motion {
                0.
            } else {
                glow_ctx.input(|i| i.time) as f32
            };
            let painter = glow_ctx.layer_painter(LayerId::background());
            for (layer, seconds, opacity) in [
                (0, 30., if light { 0.85 } else { 0.95 }),
                (1, -36., 1.),
                (2, 24., 0.8),
            ] {
                let mut mesh = Mesh {
                    indices: indices.as_ref().clone(),
                    vertices: Vec::with_capacity(angles.len()),
                    ..Default::default()
                };
                let mut vertex = 0;
                for (band, points) in contours.iter().enumerate() {
                    for point in points {
                        let color = orbit_color(angles[vertex], time / seconds);
                        vertex += 1;
                        let alpha =
                            (alpha[band][layer] * opacity * strength * color[3]).clamp(0., 1.);
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
                painter.add(mesh);
            }
            if !reduce_motion {
                glow_ctx.request_repaint_after(FRAME_INTERVAL);
            }
            if let Some(path) = &qa_path {
                glow_ctx.request_repaint_after(Duration::from_millis(16));
                let frame = qa_frames.fetch_add(1, Ordering::Relaxed);
                if frame >= 10 && !qa_sent.swap(true, Ordering::Relaxed) {
                    glow_ctx.send_viewport_cmd(ViewportCommand::Screenshot(Default::default()));
                }
                let screenshot = glow_ctx.input(|input| {
                    input.events.iter().find_map(|event| {
                        if let Event::Screenshot { image, .. } = event {
                            Some(image.clone())
                        } else {
                            None
                        }
                    })
                });
                if let Some(image) = screenshot {
                    let bytes = image
                        .pixels
                        .iter()
                        .flat_map(|color| color.to_srgba_unmultiplied())
                        .collect::<Vec<_>>();
                    if let Some(parent) = path.parent() {
                        if let Err(error) = std::fs::create_dir_all(parent) {
                            eprintln!("glow screenshot directory: {error}");
                        }
                    }
                    if let Err(error) = image::save_buffer(
                        path,
                        &bytes,
                        image.width() as u32,
                        image.height() as u32,
                        image::ColorType::Rgba8,
                    ) {
                        eprintln!("glow screenshot: {error}");
                    }
                    qa_done.store(true, Ordering::Relaxed);
                    glow_ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Close);
                }
                if frame > 900 && !qa_done.swap(true, Ordering::Relaxed) {
                    eprintln!("glow screenshot timed out: {}", path.display());
                    glow_ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Close);
                }
            }
            #[cfg(windows)]
            if owner != 0 {
                let placement = NativePlacement {
                    owner,
                    panel,
                    size: allocated_size,
                    pixels_per_point: glow_ctx.pixels_per_point(),
                    topmost,
                    focused,
                };
                let needs_placement = native_placement
                    .lock()
                    .map(|last| *last != Some(placement))
                    .unwrap_or(true);
                if !needs_placement {
                    return;
                }
                let result =
                    quicktool_rust::platform::configure_glow(TITLE, owner, visible, topmost)
                        .and_then(|_| {
                            if let Some(window) = quicktool_rust::platform::find_window(TITLE) {
                                quicktool_rust::platform::place_companion_window(
                                    owner,
                                    window,
                                    (MARGIN * glow_ctx.pixels_per_point()).round() as i32,
                                    visible,
                                    topmost,
                                    true,
                                )
                            } else {
                                Ok(())
                            }
                        });
                if result.is_ok() {
                    if let Ok(mut last) = native_placement.lock() {
                        *last = Some(placement);
                    }
                }
                if let Ok(mut status) = native_status.lock() {
                    match result {
                        Err(error) => {
                            if !status.1 {
                                status.0 = Some(error);
                                glow_ctx.request_repaint_of(ViewportId::ROOT);
                            }
                            status.1 = true;
                        }
                        Ok(()) => status.1 = false,
                    }
                }
            }
            #[cfg(not(windows))]
            let _ = (&native_status, owner);
        });
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
        for points in glow.contours.iter() {
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
    #[test]
    fn cached_glow_geometry_and_phase_match_every_layer() {
        let mut glow = Glow::default();
        for height in [84., 642., 220., 84.] {
            let size = vec2(812., height);
            glow.rebuild(size);
            let count = glow.contours[0].len();
            assert_eq!(glow.angles.len(), glow.contours.len() * count);
            assert_eq!(glow.indices.len(), (glow.contours.len() - 1) * count * 6);
            assert!(
                glow.indices
                    .iter()
                    .all(|index| (*index as usize) < glow.angles.len())
            );
            let center = pos2(MARGIN + size.x * 0.5, MARGIN + size.y * 0.5);
            for (vertex, point) in glow.contours.iter().flatten().enumerate() {
                let angle = (point.x - center.x).atan2(center.y - point.y) / std::f32::consts::TAU;
                for phase in [0., 0.125, -0.5, 1.75] {
                    assert_eq!(
                        orbit_color(glow.angles[vertex], phase),
                        orbit_color(angle, phase)
                    );
                }
            }
            for (band, triangles) in glow.indices.chunks_exact(count * 6).enumerate() {
                for (point, pair) in triangles.chunks_exact(6).enumerate() {
                    let a = (band * count + point) as u32;
                    let b = (band * count + (point + 1) % count) as u32;
                    assert_eq!(
                        pair,
                        [
                            a,
                            b,
                            a + count as u32,
                            b,
                            b + count as u32,
                            a + count as u32
                        ]
                    );
                }
            }
        }
        assert!(FRAME_INTERVAL.as_secs_f64() <= 1. / 60.);
    }
}
