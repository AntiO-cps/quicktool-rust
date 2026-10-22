use eframe::egui::*;
use quicktool_rust::platform;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct EyeDropperOverlay {
    worker: Option<platform::EyeDropper>,
    preview: Option<platform::EyeDropperPreview>,
    texture: Option<TextureHandle>,
    error: Arc<Mutex<Option<String>>>,
}
fn viewport() -> ViewportId {
    ViewportId::from_hash_of("screen-eyedropper")
}
impl EyeDropperOverlay {
    pub fn active(&self) -> bool {
        self.worker.is_some()
    }
    pub fn start(&mut self, ctx: &Context) -> Result<(), String> {
        let wake = ctx.clone();
        self.worker = Some(platform::EyeDropper::new(move || wake.request_repaint())?);
        self.preview = None;
        self.texture = None;
        Ok(())
    }
    pub fn update(&mut self, ctx: &Context) -> Option<Result<Option<[u8; 3]>, String>> {
        let error = self.error.lock().unwrap_or_else(|e| e.into_inner()).take();
        let worker = self.worker.as_ref()?;
        let outcome = error.map(Err).or_else(|| worker.results.try_recv().ok());
        if let Some(outcome) = outcome {
            if let Some(preview) = &self.preview {
                let _ = platform::configure_screen_picker(
                    &format!("QuickTool Screen Color Picker {}", std::process::id()),
                    preview.desktop_bounds,
                    false,
                );
            }
            self.worker = None;
            self.preview = None;
            self.texture = None;
            ctx.send_viewport_cmd_to(viewport(), ViewportCommand::Close);
            return Some(outcome);
        }
        if let Some(preview) = worker.previews.try_iter().last() {
            let image = ColorImage::from_rgba_unmultiplied(
                [preview.width as usize, preview.height as usize],
                &preview.rgba,
            );
            if let Some(texture) = &mut self.texture {
                texture.set(image, TextureOptions::NEAREST);
            } else {
                self.texture =
                    Some(ctx.load_texture("eyedrop-preview", image, TextureOptions::NEAREST));
            }
            self.preview = Some(preview);
        }
        let preview = self.preview.as_ref()?;
        let texture = self.texture.as_ref()?.clone();
        let bounds = preview.desktop_bounds;
        let position = preview.position;
        let rgb = preview.color;
        let error = self.error.clone();
        let title = format!("QuickTool Screen Color Picker {}", std::process::id());
        let native_title = title.clone();
        ctx.show_viewport_deferred(
            viewport(),
            ViewportBuilder::default()
                .with_title(title)
                .with_decorations(false)
                .with_transparent(true)
                .with_resizable(false)
                .with_always_on_top()
                .with_taskbar(false)
                .with_active(false)
                .with_position(pos2(
                    bounds[0] as f32 / ctx.pixels_per_point(),
                    bounds[1] as f32 / ctx.pixels_per_point(),
                ))
                .with_inner_size(vec2(
                    bounds[2] as f32 / ctx.pixels_per_point(),
                    bounds[3] as f32 / ctx.pixels_per_point(),
                )),
            move |ctx, _| {
                if let Err(problem) = platform::configure_screen_picker(&native_title, bounds, true)
                {
                    *error.lock().unwrap_or_else(|e| e.into_inner()) = Some(problem);
                    ctx.request_repaint_of(ViewportId::ROOT);
                    return;
                }
                CentralPanel::default()
                    .frame(Frame::new().fill(Color32::from_black_alpha(1)))
                    .show(ctx, |ui| {
                        // The nearly transparent full-desktop window receives the confirm
                        // click. The background sample was frozen before this window exists.
                        ui.ctx().set_cursor_icon(CursorIcon::None);
                        let screen = ui.max_rect();
                        let at = screen.min
                            + vec2(
                                (position[0] - bounds[0]) as f32 / bounds[2] as f32
                                    * screen.width(),
                                (position[1] - bounds[1]) as f32 / bounds[3] as f32
                                    * screen.height(),
                            );
                        let top = pos2(
                            (at.x + 20.).min(screen.right() - 144.).max(screen.left()),
                            (at.y + 20.).min(screen.bottom() - 170.).max(screen.top()),
                        );
                        let magnifier = Rect::from_min_size(top, vec2(140., 164.));
                        ui.painter()
                            .rect_filled(magnifier, 8, Color32::from_rgb(42, 42, 42));
                        let pixels = Rect::from_min_size(top + vec2(4., 4.), vec2(132., 132.));
                        ui.painter().image(
                            texture.id(),
                            pixels,
                            Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
                            Color32::WHITE,
                        );
                        ui.painter().rect_stroke(
                            Rect::from_center_size(pixels.center(), vec2(8.8, 8.8)),
                            0,
                            Stroke::new(1., Color32::WHITE),
                            StrokeKind::Inside,
                        );
                        ui.painter().text(
                            top + vec2(70., 149.),
                            Align2::CENTER_CENTER,
                            format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]),
                            FontId::monospace(12.),
                            Color32::WHITE,
                        );
                        ui.painter()
                            .circle_stroke(at, 6., Stroke::new(1., Color32::WHITE));
                    });
            },
        );
        None
    }
}
