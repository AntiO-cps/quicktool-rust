//! Native pin windows. No browser, JavaScript or HTML is used by this module.
use crate::{
    annotations::{self, Annotation, Tool},
    geometry,
    pixels::{self, Filters},
    undo::Undo,
};
use eframe::egui::{
    self, Align2, Color32, FontId, Id, Key, Pos2, Rect, Sense, Stroke, Vec2, pos2, vec2,
};
use image::RgbaImage;
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    sync::{Arc, mpsc},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Serialize, Deserialize)]
struct PinState {
    text: String,
    filters: Filters,
    annotations: Vec<Annotation>,
    radius: f32,
    font_size: f32,
    background: [u8; 3],
    foreground: Option<[u8; 3]>,
    opacity: f32,
    blur: f32,
    auto_background: bool,
    auto_blur: bool,
    auto_opacity: bool,
    width: f32,
    height: f32,
}
impl Default for PinState {
    fn default() -> Self {
        Self {
            text: String::new(),
            filters: Filters::default(),
            annotations: Vec::new(),
            radius: 0.,
            font_size: 15.,
            background: [25, 28, 37],
            foreground: None,
            opacity: 0.78,
            blur: 28.,
            auto_background: true,
            auto_blur: true,
            auto_opacity: true,
            width: 430.,
            height: 260.,
        }
    }
}
/// Serializable pin payload stored in the application's shared history file.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinRecord {
    pub history_id: String,
    pub kind: String,
    pub text: String,
    pub original_text: String,
    pub asset: Option<String>,
    pub filters: Filters,
    pub annotations: Vec<Annotation>,
    pub corner_radius: f32,
    pub font_size: f32,
    pub background: [u8; 3],
    pub foreground: Option<[u8; 3]>,
    pub opacity: f32,
    pub blur: f32,
    pub width: f32,
    pub height: f32,
    pub position: Option<[f32; 2]>,
    pub created: u64,
    #[serde(default)]
    pub style_automatic: Option<[bool; 3]>,
}
pub struct PinHistoryUpdate {
    pub record: PinRecord,
    pub source: Option<Arc<RgbaImage>>,
}
struct RenderRequest {
    revision: u64,
    source: Arc<RgbaImage>,
    state: PinState,
    size: [u32; 2],
    original: bool,
}
#[derive(Clone)]
struct AnnotationDefaults {
    color: [u8; 3],
    shade: [u8; 3],
    stroke: f32,
    pixel_size: f32,
    font_size: f32,
    radius: f32,
    opacity: f32,
    text_color: Option<[u8; 3]>,
}
impl Default for AnnotationDefaults {
    fn default() -> Self {
        Self {
            color: [255, 79, 99],
            shade: [0, 0, 0],
            stroke: 3.,
            pixel_size: 12.,
            font_size: 20.,
            radius: 0.,
            opacity: 0.55,
            text_color: None,
        }
    }
}
impl AnnotationDefaults {
    fn make(&self, id: u64, tool: Tool, p: Pos2, width: f32) -> Annotation {
        let mut a = Annotation::new(id, tool, p, width);
        a.color = if tool == Tool::Highlight {
            self.shade
        } else {
            self.color
        };
        a.stroke = if tool == Tool::Mosaic {
            self.pixel_size / 4.
        } else {
            self.stroke
        } / width;
        a.font_size = self.font_size / width;
        a.radius = self.radius / width;
        a.opacity = self.opacity;
        a.text_color = self.text_color;
        a
    }
    fn update(&mut self, a: &Annotation, old: &Annotation, width: f32) {
        if a.color != old.color {
            if a.tool == Tool::Highlight {
                self.shade = a.color;
            } else {
                self.color = a.color;
            }
        }
        if a.stroke != old.stroke {
            if a.tool == Tool::Mosaic {
                self.pixel_size = a.stroke * width * 4.;
            } else {
                self.stroke = a.stroke * width;
            }
        }
        if a.font_size != old.font_size {
            self.font_size = a.font_size * width;
        }
        if a.radius != old.radius {
            self.radius = a.radius * width;
        }
        if a.opacity != old.opacity {
            self.opacity = a.opacity;
        }
        if a.text_color != old.text_color {
            self.text_color = a.text_color;
        }
    }
}
struct RenderResult {
    revision: u64,
    result: Result<RgbaImage, String>,
}
struct Renderer {
    jobs: mpsc::Sender<RenderRequest>,
    results: mpsc::Receiver<RenderResult>,
}
impl Renderer {
    fn new(ctx: &egui::Context) -> Self {
        let (tx, rx) = mpsc::channel::<RenderRequest>();
        let (result_tx, result_rx) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            while let Ok(mut job) = rx.recv() {
                while let Ok(newer) = rx.try_recv() {
                    job = newer;
                }
                let result = (|| {
                    let mut base = image::imageops::resize(
                        &*job.source,
                        job.size[0],
                        job.size[1],
                        image::imageops::FilterType::Triangle,
                    );
                    if !job.original {
                        pixels::process(base.as_mut(), job.state.filters)?;
                        base =
                            annotations::render(&base, &job.state.annotations, job.state.radius)?;
                    }
                    Ok(base)
                })();
                if result_tx
                    .send(RenderResult {
                        revision: job.revision,
                        result,
                    })
                    .is_err()
                {
                    break;
                }
                ctx.request_repaint();
            }
        });
        Self {
            jobs: tx,
            results: result_rx,
        }
    }
}
#[derive(Clone)]
enum Drag {
    New(Annotation),
    Move {
        original: Annotation,
        start: Pos2,
    },
    Resize {
        original: Annotation,
        handle: String,
        start: Pos2,
    },
}
#[cfg(windows)]
struct WindowMove {
    hwnd: isize,
    cursor: windows_sys::Win32::Foundation::POINT,
    rect: windows_sys::Win32::Foundation::RECT,
    corner: Option<&'static str>,
    dpi: f32,
}
#[derive(Default)]
struct PinCapture {
    pin: Option<std::path::PathBuf>,
    effects: Option<std::path::PathBuf>,
    pin_sent: bool,
    effects_sent: bool,
    pin_done: bool,
    effects_done: bool,
    frames: u32,
    started: Option<std::time::Instant>,
}
struct Pin {
    id: u64,
    state: PinState,
    initial_text: String,
    source: Option<Arc<RgbaImage>>,
    size: Vec2,
    annotation_width: f32,
    resize_finished: bool,
    resize_target: Option<Vec2>,
    default_blur: f32,
    default_opacity: f32,
    open: bool,
    effects: bool,
    editing: bool,
    original: bool,
    history: Undo<PinState>,
    tool: Tool,
    selected: Option<u64>,
    next_annotation: u64,
    defaults: AnnotationDefaults,
    drag: Option<Drag>,
    draft: Option<Annotation>,
    text_editor: Option<Annotation>,
    text_editor_focus: bool,
    radius_editor: Option<u64>,
    renderer: Option<Renderer>,
    texture: Option<egui::TextureHandle>,
    revision: u64,
    render_key: String,
    last_size: Vec2,
    status: String,
    exporting: bool,
    export_rx: Option<mpsc::Receiver<Result<String, String>>>,
    compare_started: Option<(f64, bool)>,
    native_style_key: String,
    history_id: String,
    asset: Option<String>,
    created: u64,
    history_key: String,
    last_history_sent: u64,
    position: Option<[f32; 2]>,
    position_initialized: bool,
    light: bool,
    restore_size: bool,
    capture: Option<PinCapture>,
    popup_open: bool,
    #[cfg(windows)]
    backdrop: Option<crate::backdrop::Backdrop>,
    #[cfg(windows)]
    window_move: Option<WindowMove>,
}
impl Pin {
    fn new(id: u64, text: String, source: Option<RgbaImage>) -> Self {
        let mut state = PinState {
            text: text.clone(),
            ..Default::default()
        };
        if source.is_none() {
            state.radius = 16.;
        }
        let size = source
            .as_ref()
            .map(|im| {
                let ratio = im.width() as f32 / im.height() as f32;
                let mut width = (im.width() as f32).clamp(300., 720.);
                let mut height = width / ratio;
                if height > 640. {
                    height = 640.;
                    width = height * ratio;
                }
                vec2(width.round(), height.round())
            })
            .unwrap_or(vec2(430., 260.));
        state.width = size.x;
        state.height = size.y;
        let history = Undo::new(state.clone(), 100, 8 * 1024 * 1024);
        Self {
            id,
            state,
            initial_text: text,
            source: source.map(Arc::new),
            size,
            annotation_width: size.x,
            resize_finished: false,
            resize_target: None,
            default_blur: 28.,
            default_opacity: 0.78,
            open: true,
            effects: false,
            editing: false,
            original: false,
            history,
            tool: Tool::Pan,
            selected: None,
            next_annotation: 1,
            defaults: AnnotationDefaults::default(),
            drag: None,
            draft: None,
            text_editor: None,
            text_editor_focus: false,
            radius_editor: None,
            renderer: None,
            texture: None,
            revision: 0,
            render_key: String::new(),
            last_size: Vec2::ZERO,
            status: String::new(),
            exporting: false,
            export_rx: None,
            compare_started: None,
            native_style_key: String::new(),
            history_id: new_history_id(id),
            asset: None,
            created: now(),
            history_key: String::new(),
            last_history_sent: 0,
            position: None,
            position_initialized: false,
            light: false,
            restore_size: false,
            capture: None,
            popup_open: false,
            #[cfg(windows)]
            backdrop: None,
            #[cfg(windows)]
            window_move: None,
        }
    }
    fn record(&mut self, key: Option<&str>) {
        self.state.width = self.size.x;
        self.state.height = self.size.y;
        self.history.record(self.state.clone(), key, now());
    }
    fn undo(&mut self, redo: bool) {
        let state = if redo {
            self.history.redo()
        } else {
            self.history.undo()
        };
        if let Some(state) = state {
            self.size = vec2(state.width, state.height);
            self.annotation_width = state.width;
            self.resize_finished = false;
            self.resize_target = None;
            self.restore_size = true;
            self.state = state;
            self.draft = None;
            self.drag = None;
            self.text_editor = None;
            self.text_editor_focus = false;
            self.radius_editor = None;
            self.selected = None;
            self.original = false;
        }
    }
    fn clear_annotations(&mut self) {
        self.state.annotations.clear();
        self.selected = None;
        self.drag = None;
        self.draft = None;
        self.text_editor = None;
        self.text_editor_focus = false;
        self.radius_editor = None;
        self.record(None);
    }
    fn set_content_size(&mut self, size: Vec2) {
        if self.image()
            && size.x > 0.
            && self.annotation_width > 0.
            && (size.x - self.annotation_width).abs() > 0.001
        {
            let ratio = self.annotation_width / size.x;
            for annotation in &mut self.state.annotations {
                annotation.stroke = (annotation.stroke * ratio).clamp(0.00001, 0.5);
            }
            if let Some(annotation) = &mut self.draft {
                annotation.stroke = (annotation.stroke * ratio).clamp(0.00001, 0.5);
            }
            if let Some(annotation) = &mut self.text_editor {
                annotation.stroke = (annotation.stroke * ratio).clamp(0.00001, 0.5);
            }
        }
        self.annotation_width = size.x;
        self.size = size;
        self.state.radius = self
            .state
            .radius
            .min((size.min_elem() / 2.).floor().max(0.));
    }
    fn reset_text_effects(&mut self) {
        self.state.font_size = 15.;
        self.state.background = if self.light {
            [248, 250, 255]
        } else {
            [25, 28, 37]
        };
        self.state.foreground = None;
        self.state.blur = self.default_blur;
        self.state.opacity = self.default_opacity;
        self.state.auto_background = true;
        self.state.auto_blur = true;
        self.state.auto_opacity = true;
    }
    fn image(&self) -> bool {
        self.source.is_some()
    }
    fn history_record(&self) -> PinRecord {
        PinRecord {
            history_id: self.history_id.clone(),
            kind: if self.image() { "image" } else { "text" }.into(),
            text: self.state.text.clone(),
            original_text: self.initial_text.clone(),
            asset: self.asset.clone(),
            filters: self.state.filters,
            annotations: self.state.annotations.clone(),
            corner_radius: self.state.radius,
            font_size: self.state.font_size,
            background: self.state.background,
            foreground: self.state.foreground,
            opacity: self.state.opacity,
            blur: self.state.blur,
            width: self.size.x,
            height: self.size.y,
            position: self.position,
            created: self.created,
            style_automatic: Some([
                self.state.auto_background,
                self.state.auto_blur,
                self.state.auto_opacity,
            ]),
        }
    }
    fn commit_text(&mut self) {
        if let Some(mut a) = self.text_editor.take() {
            a.text = a.text.chars().take(2000).collect();
            if a.text.trim().is_empty() {
                return;
            }
            if let Some(existing) = self.state.annotations.iter_mut().find(|x| x.id == a.id) {
                *existing = a.clone();
            } else if self.state.annotations.len() < 200 {
                self.state.annotations.push(a.clone());
            }
            self.selected = Some(a.id);
            self.record(None);
        }
    }
    fn fresh(&mut self, p: Pos2) -> Annotation {
        let mut a = self
            .defaults
            .make(self.next_annotation, self.tool, p, self.size.x);
        a.id = self.next_annotation;
        self.next_annotation += 1;
        a.tool = self.tool;
        a.x = p.x;
        a.x2 = p.x;
        a.y = p.y;
        a.y2 = p.y;
        a.points.clear();
        a.text.clear();
        if self.tool == Tool::Highlight {
            if let Some(prev) = self
                .state
                .annotations
                .iter()
                .rev()
                .find(|a| a.tool == Tool::Highlight)
            {
                a.color = prev.color;
                a.opacity = prev.opacity;
            }
        }
        a.number = self
            .state
            .annotations
            .iter()
            .filter(|a| a.tool == Tool::Number)
            .count() as u32
            + 1;
        a
    }
    fn remove_selected(&mut self) {
        if let Some(id) = self.selected.take() {
            self.state.annotations.retain(|a| a.id != id);
            let mut n = 0;
            for a in &mut self.state.annotations {
                if a.tool == Tool::Number {
                    n += 1;
                    a.number = n;
                }
            }
            self.record(None);
        }
    }
    fn queue_render(&mut self, ctx: &egui::Context, size: Vec2) {
        let Some(source) = self.source.clone() else {
            return;
        };
        if self.renderer.is_none() {
            self.renderer = Some(Renderer::new(ctx));
        }
        if let Some(renderer) = &self.renderer {
            while let Ok(result) = renderer.results.try_recv() {
                if result.revision == self.revision {
                    match result.result {
                        Ok(image) => {
                            let color = egui::ColorImage::from_rgba_unmultiplied(
                                [image.width() as usize, image.height() as usize],
                                image.as_raw(),
                            );
                            if let Some(texture) = &mut self.texture {
                                texture.set(color, egui::TextureOptions::LINEAR);
                            } else {
                                self.texture = Some(ctx.load_texture(
                                    format!("pin-{}", self.id),
                                    color,
                                    egui::TextureOptions::LINEAR,
                                ));
                            }
                        }
                        Err(e) => self.status = e,
                    }
                }
            }
        }
        let scale = (960. / size.max_elem())
            .min(ctx.pixels_per_point())
            .min(1.5);
        let preview = vec2(
            (size.x * scale).round().max(1.),
            (size.y * scale).round().max(1.),
        );
        let mut state = self.state.clone();
        state.radius *= preview.x / size.x;
        if let Some(draft) = &self.draft {
            if let Some(a) = state.annotations.iter_mut().find(|a| a.id == draft.id) {
                *a = draft.clone();
            } else {
                state.annotations.push(draft.clone());
            }
        }
        let key = format!(
            "{}:{}:{}:{}",
            serde_json::to_string(&state).unwrap_or_default(),
            preview.x,
            preview.y,
            self.original
        );
        if key != self.render_key {
            self.render_key = key;
            self.revision += 1;
            let _ = self.renderer.as_ref().unwrap().jobs.send(RenderRequest {
                revision: self.revision,
                source,
                state,
                size: [preview.x as u32, preview.y as u32],
                original: self.original,
            });
        }
    }
    fn export(&mut self, ctx: &egui::Context, format: &str, copy: bool) {
        self.commit_text();
        if self.exporting {
            return;
        }
        let Some(source) = self.source.clone() else {
            let _ = crate::platform::copy_text(&self.state.text);
            return;
        };
        let path = if copy {
            None
        } else {
            match crate::platform::save_file_dialog(format) {
                Ok(Some(p)) => Some(p),
                Ok(None) => return,
                Err(e) => {
                    self.status = e;
                    return;
                }
            }
        };
        let mut state = self.state.clone();
        let current_width = self.size.x.max(1.);
        let format = format.to_owned();
        let (tx, rx) = mpsc::channel();
        self.export_rx = Some(rx);
        self.exporting = true;
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = (|| {
                let limit = if format == "psd" {
                    8_000_000.
                } else {
                    16_000_000.
                };
                let scale = (limit / (source.width() as f64 * source.height() as f64))
                    .sqrt()
                    .min(1.);
                let (w, h) = (
                    (source.width() as f64 * scale).round().max(1.) as u32,
                    (source.height() as f64 * scale).round().max(1.) as u32,
                );
                let original =
                    image::imageops::resize(&*source, w, h, image::imageops::FilterType::Lanczos3);
                let mut filtered = original.clone();
                pixels::process(filtered.as_mut(), state.filters)?;
                state.radius *= w as f32 / current_width;
                if copy {
                    let image = annotations::render(&filtered, &state.annotations, state.radius)?;
                    crate::platform::copy_image(w, h, image.as_raw())?;
                    return Ok("已复制处理后的图片".into());
                }
                let path = path.ok_or("未选择导出路径")?;
                if format == "psd" {
                    let data = annotations::export_psd(
                        &original,
                        &filtered,
                        &state.annotations,
                        state.radius,
                    )?;
                    crate::storage::atomic_write(&path, &data).map_err(|e| e.to_string())?;
                } else {
                    let image = annotations::render(&filtered, &state.annotations, state.radius)?;
                    let mut bytes = std::io::Cursor::new(Vec::new());
                    image
                        .write_to(&mut bytes, image::ImageFormat::Png)
                        .map_err(|e| e.to_string())?;
                    crate::storage::atomic_write(&path, bytes.get_ref())
                        .map_err(|e| e.to_string())?;
                }
                Ok(format!("已导出 {} · {w} × {h} px", format.to_uppercase()))
            })();
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }
    fn show(&mut self, ctx: &egui::Context) {
        self.popup_open = false;
        if self.image()
            && self.state.radius == 0.
            && !self.filters_changed()
            && self.state.annotations.is_empty()
        {
            self.original = false;
            self.compare_started = None;
        }
        if let Some(rect) = ctx.input(|i| i.viewport().outer_rect) {
            self.position = Some([rect.left(), rect.top()]);
        }
        if !ctx.input(|i| i.viewport().focused.unwrap_or(true)) {
            if let Some((_, prior)) = self.compare_started.take() {
                self.original = prior;
            }
        }
        self.update_move(ctx);
        self.apply_native_style(ctx);
        if let Some(rx) = &self.export_rx {
            if let Ok(result) = rx.try_recv() {
                self.status = result.unwrap_or_else(|e| e);
                self.exporting = false;
                self.export_rx = None;
            }
        }
        if ctx.input(|i| i.viewport().close_requested()) {
            self.open = false;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, Key::Z)) {
            self.undo(false);
        }
        if ctx.input_mut(|i| {
            i.consume_key(egui::Modifiers::COMMAND, Key::Y)
                || i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, Key::Z)
        }) {
            self.undo(true);
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.commit_text();
            self.open = false;
        }
        if !self.image() {
            let zoom_keys = ctx.input_mut(|input| {
                let reset = input.consume_key(egui::Modifiers::COMMAND, Key::Num0);
                let smaller = input.consume_key(egui::Modifiers::COMMAND, Key::Minus);
                let larger = input.consume_key(egui::Modifiers::COMMAND, Key::Plus)
                    || input.consume_key(egui::Modifiers::COMMAND, Key::Equals)
                    || input
                        .consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, Key::Plus);
                (reset, smaller, larger)
            });
            if zoom_keys.0 {
                self.state.font_size = 15.;
                self.record(Some("font-size"));
            } else if zoom_keys.1 || zoom_keys.2 {
                self.zoom_text(if zoom_keys.1 { 1. / 1.1 } else { 1.1 });
            }
            let scroll = ctx.input(|input| {
                input
                    .events
                    .iter()
                    .filter_map(|event| match event {
                        egui::Event::MouseWheel {
                            delta, modifiers, ..
                        } if modifiers.ctrl && delta.y != 0. => Some(delta.y),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            });
            for delta in scroll {
                self.zoom_text(if delta > 0. { 1.1 } else { 1. / 1.1 });
            }
        }
        if self.editing
            && !ctx.wants_keyboard_input()
            && ctx.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace))
        {
            self.remove_selected();
        }
        if self.image() && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, Key::C)) {
            self.export(ctx, "png", true);
        }
        if self.restore_size {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(
                self.size + vec2(16., if self.editing { 64. } else { 16. }),
            ));
            self.restore_size = false;
        }
        let bg = if self.image() {
            Color32::TRANSPARENT
        } else {
            Color32::from_rgba_unmultiplied(
                self.state.background[0],
                self.state.background[1],
                self.state.background[2],
                (self.state.opacity * 255.) as u8,
            )
        };
        let bounds = ctx.screen_rect();
        let bottom = if self.editing { 56. } else { 8. };
        let card = Rect::from_min_max(bounds.min + vec2(8., 8.), bounds.max - vec2(8., bottom));
        self.set_content_size(card.size());
        if self.resize_finished {
            if self
                .resize_target
                .is_none_or(|target| (self.size - target).abs().max_elem() < 2.)
            {
                self.record(Some("resize"));
                self.resize_finished = false;
                self.resize_target = None;
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(8));
            }
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::TRANSPARENT))
            .show(ctx, |ui| {
                ui.painter()
                    .rect_filled(card, self.state.radius.min(255.) as u8, bg);
                if self.image() {
                    self.queue_render(ctx, card.size());
                    if let Some(texture) = &self.texture {
                        ui.painter().image(
                            texture.id(),
                            card,
                            Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
                            Color32::WHITE,
                        );
                    } else {
                        ui.painter().text(
                            card.center(),
                            Align2::CENTER_CENTER,
                            "正在加载图片…",
                            FontId::proportional(14.),
                            Color32::WHITE,
                        );
                    }
                    let response =
                        ui.interact(card, Id::new((self.id, "image")), Sense::click_and_drag());
                    if self.editing && !self.original {
                        self.annotation_input(ui, &response, card);
                    } else if response.drag_started() {
                        self.begin_move(ctx);
                    }
                    response.context_menu(|ui| self.context_menu(ui, ctx));
                } else {
                    let text_rect =
                        Rect::from_min_max(card.min + vec2(20., 44.), card.max - vec2(20., 20.));
                    ui.scope_builder(egui::UiBuilder::new().max_rect(text_rect), |ui| {
                        let fg =
                            annotations::egui_color(self.state.foreground.unwrap_or_else(|| {
                                annotations::number_color(self.state.background)
                            }));
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            let font_size = self.state.font_size;
                            let font =
                                FontId::new(font_size, egui::FontFamily::Name("PinText".into()));
                            let mut layouter = |ui: &egui::Ui, text: &str, width: f32| {
                                let mut job = egui::text::LayoutJob::simple(
                                    text.into(),
                                    font.clone(),
                                    fg,
                                    width,
                                );
                                for section in &mut job.sections {
                                    section.format.line_height = Some(font_size * 1.6);
                                }
                                ui.fonts(|fonts| fonts.layout_job(job))
                            };
                            let response = ui.add(
                                egui::TextEdit::multiline(&mut self.state.text)
                                    .id(Id::new((self.id, "text")))
                                    .margin(Vec2::ZERO)
                                    .font(font.clone())
                                    .layouter(&mut layouter)
                                    .text_color(fg)
                                    .frame(false)
                                    .desired_width(text_rect.width())
                                    .desired_rows(1),
                            );
                            if response.changed() {
                                self.record(Some("text"));
                            }
                            response.context_menu(|ui| self.context_menu(ui, ctx));
                        });
                    });
                    let drag = ui.interact(
                        Rect::from_min_size(card.min, vec2((card.width() - 110.).max(1.), 40.)),
                        Id::new((self.id, "drag")),
                        Sense::drag(),
                    );
                    if drag.drag_started() {
                        self.begin_move(ctx);
                    }
                    if ui.rect_contains_pointer(card) || ctx.wants_keyboard_input() {
                        ui.painter().rect_stroke(
                            card,
                            16,
                            Stroke::new(1., Color32::from_rgb(50, 155, 255)),
                            egui::StrokeKind::Inside,
                        );
                    }
                }
            });
        let hovered = ctx.input(|i| i.pointer.hover_pos().is_some()) || ctx.wants_keyboard_input();
        if hovered {
            egui::Area::new(Id::new((self.id, "buttons")))
                .fixed_pos(pos2(card.right() - 110., card.top() + 8.))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    ui.spacing_mut().item_spacing.x = 6.;
                    ui.horizontal(|ui| {
                        if self.image()
                            && (self.state.radius > 0.
                                || self.filters_changed()
                                || !self.state.annotations.is_empty())
                        {
                            let r = ui
                                .button(if self.original { "效果" } else { "原图" })
                                .on_hover_text("按住查看原图；单击切换");
                            if r.is_pointer_button_down_on() {
                                let time = ctx.input(|i| i.time);
                                if self.compare_started.is_none() {
                                    self.compare_started = Some((time, self.original));
                                }
                                if let Some((start, _)) = self.compare_started {
                                    if time - start >= 0.18 {
                                        self.original = true;
                                        ctx.request_repaint();
                                    } else {
                                        ctx.request_repaint_after(
                                            std::time::Duration::from_millis(180),
                                        );
                                    }
                                }
                            } else if let Some((start, prior)) = self.compare_started.take() {
                                if ctx.input(|i| i.time) - start >= 0.18 {
                                    self.original = prior;
                                } else if r.clicked() {
                                    self.original = !prior;
                                }
                            } else if r.clicked() {
                                self.original = !self.original;
                            }
                        } else if !self.image()
                            && pin_icon_button(ui, "restore", "重置为初始文字", false, true)
                                .clicked()
                        {
                            self.state.text = self.initial_text.clone();
                            self.record(None);
                        }
                        if pin_icon_button(
                            ui,
                            "tune",
                            if self.image() {
                                "图片调整"
                            } else {
                                "文字调整"
                            },
                            false,
                            true,
                        )
                        .clicked()
                        {
                            self.effects = !self.effects;
                            if self.image() {
                                self.editing = self.effects;
                            }
                        }
                        if pin_icon_button(ui, "close", "关闭钉住窗口", false, true).clicked()
                        {
                            self.open = false;
                        }
                    });
                });
        }
        if self.editing {
            self.toolbar(ctx, card);
        }
        if self.effects {
            self.effects_panel(ctx);
        }
        if let Some(id) = self.radius_editor {
            let mut done = false;
            egui::Window::new("标注圆角")
                .id(Id::new((self.id, "radius-editor")))
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    if let Some(a) = self.state.annotations.iter_mut().find(|a| a.id == id) {
                        let mut radius = a.radius * self.size.x;
                        let max = (a.rect().width() * self.size.x)
                            .min(a.rect().height() * self.size.y)
                            / 2.;
                        ui.horizontal(|ui| {
                            ui.label("圆角");
                            ui.add(
                                egui::DragValue::new(&mut radius)
                                    .range(0.0..=max)
                                    .suffix(" px"),
                            );
                            done = ui.button("确定").clicked();
                        });
                        a.radius = radius / self.size.x;
                    } else {
                        done = true;
                    }
                });
            if done {
                self.radius_editor = None;
                self.record(None);
            }
        }
        if self.text_editor.is_some() {
            let mut commit = false;
            let mut cancel = false;
            egui::Window::new("标注文字")
                .id(Id::new((self.id, "text-editor")))
                .collapsible(false)
                .resizable(false)
                .default_width(240.)
                .show(ctx, |ui| {
                    if let Some(a) = &mut self.text_editor {
                        let response = ui.add(
                            egui::TextEdit::multiline(&mut a.text)
                                .desired_rows(3)
                                .return_key(egui::KeyboardShortcut::new(
                                    egui::Modifiers::SHIFT,
                                    Key::Enter,
                                ))
                                .hint_text("输入文字；Shift+Enter 换行"),
                        );
                        if self.text_editor_focus {
                            response.request_focus();
                            self.text_editor_focus = false;
                        }
                        if response.has_focus()
                            && ui.input(|i| i.key_pressed(Key::Enter) && !i.modifiers.shift)
                        {
                            commit = true;
                        }
                        ui.horizontal(|ui| {
                            commit |= ui.button("确定").clicked();
                            cancel |= ui.button("取消").clicked();
                        });
                    }
                });
            if commit {
                self.commit_text();
            }
            if cancel {
                self.text_editor = None;
            }
        }
        if !self.status.is_empty() || self.exporting {
            egui::Area::new(Id::new((self.id, "status")))
                .anchor(
                    Align2::CENTER_BOTTOM,
                    vec2(0., -if self.editing { 64. } else { 24. }),
                )
                .order(egui::Order::Tooltip)
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        if self.exporting {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label("正在导出…");
                            });
                        } else if ui.button(&self.status).clicked() {
                            self.status.clear();
                        }
                    });
                });
        }
        self.resize_handles(ctx, card);
        self.popup_open |= ctx.memory(|memory| memory.any_popup_open());
        self.capture_viewport(ctx, false);
    }
    fn text_selection(&self, ctx: &egui::Context) -> std::ops::Range<usize> {
        let count = self.state.text.chars().count();
        egui::text_edit::TextEditState::load(ctx, Id::new((self.id, "text")))
            .and_then(|state| state.cursor.char_range())
            .map(|range| {
                let [start, end] = range.sorted();
                start.index.min(count)..end.index.min(count)
            })
            .unwrap_or(count..count)
    }
    fn select_text(&self, ctx: &egui::Context, range: std::ops::Range<usize>) {
        let id = Id::new((self.id, "text"));
        let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(range.start),
                egui::text::CCursor::new(range.end),
            )));
        state.store(ctx, id);
        ctx.memory_mut(|memory| memory.request_focus(id));
    }
    fn replace_text_selection(&mut self, ctx: &egui::Context, replacement: &str) {
        use egui::TextBuffer;
        let range = self.text_selection(ctx);
        let start = range.start;
        self.state.text.delete_char_range(range);
        let count = self.state.text.insert_text(replacement, start);
        self.select_text(ctx, start + count..start + count);
        self.record(Some("text"));
    }
    fn filters_changed(&self) -> bool {
        let f = self.state.filters;
        f.brightness != 0.
            || f.contrast != 0.
            || f.saturation != 0.
            || f.exposure != 0.
            || f.hue != 0.
            || f.colorize
    }
    fn context_menu(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if ui
            .add_enabled(self.history.can_undo(), egui::Button::new("撤回 · Ctrl+Z"))
            .clicked()
        {
            self.undo(false);
            ui.close_menu();
        }
        if ui
            .add_enabled(
                self.history.can_redo(),
                egui::Button::new("重做 · Ctrl+Shift+Z"),
            )
            .clicked()
        {
            self.undo(true);
            ui.close_menu();
        }
        ui.separator();
        if self.image() {
            if ui.button("复制处理前图片").clicked() {
                if let Some(im) = &self.source {
                    self.status = crate::platform::copy_image(im.width(), im.height(), im.as_raw())
                        .map(|_| "已复制原图".into())
                        .unwrap_or_else(|e| e);
                }
                ui.close_menu();
            }
            if ui.button("复制处理后图片").clicked() {
                self.export(ctx, "png", true);
                ui.close_menu();
            }
            if ui.button("图片调整").clicked() {
                self.effects = true;
                self.editing = true;
                ui.close_menu();
            }
            for (name, extension) in [("导出 PNG…", "png"), ("导出 PSD…", "psd")] {
                if ui.button(name).clicked() {
                    self.export(ctx, extension, false);
                    ui.close_menu();
                }
            }
        } else {
            use egui::TextBuffer;
            let selection = self.text_selection(ctx);
            if ui.button("复制").clicked() {
                let text = if selection.is_empty() {
                    &self.state.text
                } else {
                    self.state.text.char_range(selection.clone())
                };
                self.status = crate::platform::copy_text(text)
                    .map(|_| "已复制文字".into())
                    .unwrap_or_else(|e| e);
                ui.close_menu();
            }
            if ui.button("编辑文字").clicked() {
                self.select_text(ctx, selection.clone());
                ui.close_menu();
            }
            if ui.button("全选").clicked() {
                self.select_text(ctx, 0..self.state.text.chars().count());
                ui.close_menu();
            }
            if ui.button("剪切").clicked() {
                if !selection.is_empty() {
                    match crate::platform::copy_text(self.state.text.char_range(selection)) {
                        Ok(()) => self.replace_text_selection(ctx, ""),
                        Err(error) => self.status = error,
                    }
                }
                ui.close_menu();
            }
            if ui.button("粘贴").clicked() {
                match crate::platform::clipboard_text() {
                    Ok(text) => self.replace_text_selection(
                        ctx,
                        &text.replace("\r\n", "\n").replace('\r', "\n"),
                    ),
                    Err(error) => self.status = error,
                }
                ui.close_menu();
            }
        }
    }
    fn zoom_text(&mut self, factor: f32) {
        self.state.font_size = ((self.state.font_size * factor).clamp(8., 96.) * 10.).round() / 10.;
        self.record(Some("font-size"));
    }
    fn toolbar(&mut self, ctx: &egui::Context, card: Rect) {
        egui::Area::new(Id::new((self.id, "annotation-tools")))
            .fixed_pos(pos2(8., card.bottom() + 8.))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let bar =
                    Rect::from_min_size(pos2(8., card.bottom() + 8.), vec2(card.width(), 40.));
                let bg = if self.light {
                    Color32::from_rgb(248, 250, 255)
                } else {
                    Color32::from_rgb(36, 41, 54)
                };
                ui.painter().rect_filled(bar, 10., bg);
                ui.painter().rect_stroke(
                    bar,
                    10.,
                    Stroke::new(1., pin_border(self.light)),
                    egui::StrokeKind::Inside,
                );
                ui.set_min_size(vec2(card.width(), 40.));
                ui.add_space(6.5);
                ui.spacing_mut().item_spacing.x = 3.;
                ui.horizontal(|ui| {
                    ui.add_space(((card.width() - 258.) / 2.).max(6.));
                    for tool in [
                        Tool::Pan,
                        Tool::Rect,
                        Tool::Arrow,
                        Tool::Pen,
                        Tool::Text,
                        Tool::Number,
                        Tool::Mosaic,
                        Tool::Highlight,
                    ] {
                        let active = self.tool == tool
                            || (tool == Tool::Rect
                                && matches!(self.tool, Tool::Ellipse | Tool::Line));
                        let symbol = match tool {
                            Tool::Pan => "cursor-move",
                            Tool::Rect => match self.tool {
                                Tool::Ellipse => "ellipse-outline",
                                Tool::Line => "vector-line",
                                _ => "rectangle-outline",
                            },
                            Tool::Arrow => "arrow-top-right",
                            Tool::Pen => "draw",
                            Tool::Text => "format-text",
                            Tool::Number => "numeric-1-circle-outline",
                            Tool::Mosaic => "checkerboard",
                            Tool::Highlight => "marker",
                            _ => "rectangle-outline",
                        };
                        if pin_icon_button(ui, symbol, tool.label(), active, false).clicked() {
                            self.commit_text();
                            self.tool = tool;
                            self.selected = None;
                        }
                    }
                    if pin_icon_button(ui, "delete-outline", "清空全部标注", false, false).clicked()
                    {
                        self.clear_annotations();
                    }
                });
            });
        let selected = self
            .selected
            .and_then(|id| self.state.annotations.iter().find(|a| a.id == id).cloned());
        let mut a = selected
            .clone()
            .unwrap_or_else(|| self.defaults.make(0, self.tool, Pos2::ZERO, card.width()));
        let previous = a.clone();
        if a.tool == Tool::Pan {
            return;
        }
        let before = serde_json::to_string(&a).unwrap_or_default();
        egui::Area::new(Id::new((self.id, "annotation-options")))
            .anchor(Align2::CENTER_BOTTOM, vec2(0., -56.))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(if self.light {
                        Color32::from_rgb(248, 250, 255)
                    } else {
                        Color32::from_rgb(36, 41, 54)
                    })
                    .stroke(Stroke::new(1., pin_border(self.light)))
                    .corner_radius(7.)
                    .inner_margin(egui::Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        ui.set_max_width((ctx.screen_rect().width() - 40.).max(1.));
                        ui.style_mut().override_font_id = Some(FontId::proportional(10.));
                        ui.spacing_mut().interact_size.y = 25.;
                        ui.horizontal_wrapped(|ui| {
                            if matches!(a.tool, Tool::Rect | Tool::Ellipse | Tool::Line) {
                                egui::ComboBox::from_id_salt("shape")
                                    .selected_text(a.tool.label())
                                    .width(62.)
                                    .show_ui(ui, |ui| {
                                        for tool in [Tool::Rect, Tool::Ellipse, Tool::Line] {
                                            ui.selectable_value(&mut a.tool, tool, tool.label());
                                        }
                                    });
                            }
                            if a.tool != Tool::Mosaic {
                                ui.color_edit_button_srgb(&mut a.color);
                            }
                            if matches!(
                                a.tool,
                                Tool::Rect
                                    | Tool::Ellipse
                                    | Tool::Line
                                    | Tool::Arrow
                                    | Tool::Pen
                                    | Tool::Mosaic
                            ) {
                                let mosaic = a.tool == Tool::Mosaic;
                                ui.label(if mosaic { "像素块" } else { "线宽" });
                                let divisor = if mosaic { 4. } else { 1. };
                                let mut width = a.stroke * card.width() * divisor;
                                ui.add(
                                    egui::DragValue::new(&mut width)
                                        .range(if mosaic { 6.0..=128.0 } else { 1.0..=32.0 })
                                        .speed(1.),
                                );
                                a.stroke = width / card.width() / divisor;
                            }
                            if matches!(a.tool, Tool::Text | Tool::Number) {
                                ui.label("字号");
                                let mut size = a.font_size * card.width();
                                ui.add(
                                    egui::DragValue::new(&mut size).range(10.0..=96.0).speed(1.),
                                );
                                a.font_size = size / card.width();
                            }
                            if a.tool == Tool::Number {
                                let mut manual = a.text_color.is_some();
                                if ui.checkbox(&mut manual, "自选文字色").changed() {
                                    a.text_color =
                                        if manual { Some([255, 255, 255]) } else { None };
                                }
                                if let Some(c) = &mut a.text_color {
                                    ui.color_edit_button_srgb(c);
                                }
                            }
                            if matches!(a.tool, Tool::Rect | Tool::Mosaic | Tool::Highlight) {
                                ui.label("圆角");
                                let max = if selected.is_some() {
                                    (a.rect().width() * card.width())
                                        .min(a.rect().height() * card.height())
                                        / 2.
                                } else {
                                    card.size().min_elem() / 2.
                                };
                                let mut radius = a.radius * card.width();
                                ui.add(
                                    egui::DragValue::new(&mut radius).range(0.0..=max).speed(1.),
                                );
                                a.radius = radius / card.width();
                            }
                            if a.tool == Tool::Highlight {
                                ui.label("压暗");
                                ui.add(
                                    egui::Slider::new(&mut a.opacity, 0.0..=1.0).custom_formatter(
                                        |v, _| format!("{}%", (v * 100.).round()),
                                    ),
                                );
                            }
                        });
                    });
            });
        if before != serde_json::to_string(&a).unwrap_or_default() {
            if selected.as_ref().is_none_or(|old| old.tool != a.tool) {
                self.tool = a.tool;
            }
            self.defaults.update(&a, &previous, card.width());
            if let Some(selected) = selected {
                for item in &mut self.state.annotations {
                    if item.id == selected.id {
                        *item = a.clone();
                    } else if a.tool == Tool::Highlight && item.tool == Tool::Highlight {
                        item.color = a.color;
                        item.opacity = a.opacity;
                    }
                }
                self.record(Some("annotation-property"));
            } else if a.tool == Tool::Highlight
                && (a.color != previous.color || a.opacity != previous.opacity)
            {
                for item in &mut self.state.annotations {
                    if item.tool == Tool::Highlight {
                        item.color = a.color;
                        item.opacity = a.opacity;
                    }
                }
                self.record(Some("annotation-property"));
            }
        }
    }
    fn annotation_input(&mut self, ui: &mut egui::Ui, response: &egui::Response, rect: Rect) {
        let point = |p: Pos2| {
            let p = (p - rect.min) / rect.size();
            pos2(p.x.clamp(0., 1.), p.y.clamp(0., 1.))
        };
        let pointer = response
            .interact_pointer_pos()
            .or_else(|| ui.ctx().pointer_interact_pos());
        let shift = ui.input(|i| i.modifiers.shift);
        let mut handle_hit = None;
        if let Some(a) = self
            .selected
            .and_then(|id| self.state.annotations.iter().find(|a| a.id == id).cloned())
        {
            let display = self.draft.as_ref().filter(|b| b.id == a.id).unwrap_or(&a);
            let bounds = display.bounds(rect.size()).translate(rect.min.to_vec2());
            ui.painter().rect_stroke(
                bounds,
                0,
                Stroke::new(1., Color32::from_rgb(50, 155, 255)),
                egui::StrokeKind::Outside,
            );
            if matches!(
                a.tool,
                Tool::Rect
                    | Tool::Ellipse
                    | Tool::Line
                    | Tool::Arrow
                    | Tool::Mosaic
                    | Tool::Highlight
            ) {
                for (handle, p) in geometry::handles(display) {
                    let center = rect.min + p.to_vec2() * rect.size();
                    let hrect = Rect::from_center_size(center, vec2(9., 9.));
                    ui.painter().rect_filled(hrect, 2, Color32::WHITE);
                    ui.painter().rect_stroke(
                        hrect,
                        2,
                        Stroke::new(1., Color32::from_rgb(50, 155, 255)),
                        egui::StrokeKind::Inside,
                    );
                    if pointer.is_some_and(|p| hrect.expand(4.).contains(p)) {
                        handle_hit = Some((a.clone(), handle));
                    }
                }
            }
            if matches!(a.tool, Tool::Rect | Tool::Mosaic | Tool::Highlight) {
                let r = display.rect();
                let max = (r.width() * rect.width()).min(r.height() * rect.height()) / 2.;
                let offset = max.min(14_f32.max((display.radius * rect.width()).min(max) * 0.293));
                for (name, corner, sx, sy) in [
                    ("radius-nw", r.left_top(), 1., 1.),
                    ("radius-ne", r.right_top(), -1., 1.),
                    ("radius-sw", r.left_bottom(), 1., -1.),
                    ("radius-se", r.right_bottom(), -1., -1.),
                ] {
                    let center =
                        rect.min + corner.to_vec2() * rect.size() + vec2(sx * offset, sy * offset);
                    ui.painter().circle_filled(center, 5.5, Color32::WHITE);
                    ui.painter().circle_stroke(
                        center,
                        5.5,
                        Stroke::new(2., Color32::from_rgb(50, 155, 255)),
                    );
                    if pointer.is_some_and(|p| p.distance(center) < 8.) {
                        handle_hit = Some((a.clone(), name));
                        if response.double_clicked() {
                            self.radius_editor = Some(a.id);
                        }
                    }
                }
            }
        }
        if response.double_clicked() {
            if let Some(p) = pointer {
                if let Some(a) = self
                    .state
                    .annotations
                    .iter()
                    .rev()
                    .find(|a| a.tool == Tool::Text && a.hit(point(p), rect.size()))
                    .cloned()
                {
                    self.text_editor = Some(a);
                    self.text_editor_focus = true;
                    return;
                }
            }
        }
        let pressed = ui.input(|i| i.pointer.primary_pressed());
        if pressed && response.contains_pointer() {
            if let Some(p) = pointer {
                let p = point(p);
                if let Some((a, handle)) = handle_hit {
                    self.drag = Some(Drag::Resize {
                        original: a,
                        handle: handle.into(),
                        start: p,
                    });
                } else {
                    let mut order = self.state.annotations.iter().collect::<Vec<_>>();
                    order.sort_by_key(|a| a.tool.rank());
                    let hit = order
                        .into_iter()
                        .rev()
                        .find(|a| a.hit(p, rect.size()))
                        .cloned();
                    if let Some(a) = hit {
                        self.selected = Some(a.id);
                        self.drag = Some(Drag::Move {
                            original: a,
                            start: p,
                        });
                    } else {
                        self.selected = None;
                        if self.tool != Tool::Pan {
                            let mut a = self.fresh(p);
                            match self.tool {
                                Tool::Text => {
                                    self.text_editor = Some(a);
                                    self.text_editor_focus = true;
                                }
                                Tool::Number => {
                                    if self.state.annotations.len() < 200 {
                                        self.selected = Some(a.id);
                                        self.state.annotations.push(a);
                                        self.record(None);
                                    }
                                }
                                _ => {
                                    if self.tool == Tool::Pen {
                                        a.points.push([p.x, p.y]);
                                    }
                                    self.drag = Some(Drag::New(a));
                                }
                            }
                        } else {
                            self.begin_move(ui.ctx());
                        }
                    }
                }
            }
        }
        if let (Some(drag), Some(pointer)) = (self.drag.clone(), pointer) {
            let p = point(pointer);
            let draft = match drag {
                Drag::New(mut a) => {
                    let end = if shift {
                        if matches!(a.tool, Tool::Line | Tool::Arrow) {
                            geometry::snap(a.start(), p, rect.size())
                        } else if matches!(
                            a.tool,
                            Tool::Rect | Tool::Ellipse | Tool::Mosaic | Tool::Highlight
                        ) {
                            geometry::square(a.start(), p, rect.size())
                        } else {
                            p
                        }
                    } else {
                        p
                    };
                    a.x2 = end.x;
                    a.y2 = end.y;
                    if a.tool == Tool::Pen
                        && a.points.len() < 2048
                        && a.points
                            .last()
                            .is_none_or(|last| pos2(last[0], last[1]).distance(p) > 0.002)
                    {
                        a.points.push([p.x, p.y]);
                        self.drag = Some(Drag::New(a.clone()));
                    }
                    a
                }
                Drag::Move { original, start } => geometry::translate(&original, p - start),
                Drag::Resize {
                    original,
                    handle,
                    start,
                } => {
                    if handle.starts_with("radius-") {
                        let mut a = original;
                        let sx = if handle.ends_with("ne") || handle.ends_with("se") {
                            -1.
                        } else {
                            1.
                        };
                        let sy = if handle.ends_with("sw") || handle.ends_with("se") {
                            -1.
                        } else {
                            1.
                        };
                        let max = (a.rect().width() * rect.width())
                            .min(a.rect().height() * rect.height())
                            / 2.;
                        a.radius = (a.radius * rect.width()
                            + ((p.x - start.x) * rect.width() * sx
                                + (p.y - start.y) * rect.height() * sy))
                            .clamp(0., max)
                            / rect.width();
                        a
                    } else {
                        geometry::resize(&original, &handle, p, rect.size(), shift, start)
                    }
                }
            };
            self.draft = Some(draft);
            ui.ctx().request_repaint();
        }
        if ui.input(|i| i.pointer.primary_released()) && self.drag.is_some() {
            let drag = self.drag.take();
            if let Some(mut a) = self.draft.take() {
                a.normalize();
                if let Some(existing) = self.state.annotations.iter_mut().find(|x| x.id == a.id) {
                    *existing = a.clone();
                } else if matches!(drag, Some(Drag::New(_)))
                    && (a.tool == Tool::Pen || a.start().distance(a.end()) > 0.004)
                {
                    let total: usize = self.state.annotations.iter().map(|a| a.points.len()).sum();
                    if self.state.annotations.len() < 200 && total + a.points.len() <= 10000 {
                        self.state.annotations.push(a.clone());
                    } else {
                        self.status = "标注已达上限，请撤回部分标注".into();
                    }
                }
                self.selected = Some(a.id);
                self.record(None);
            }
        }
    }
    fn effects_panel(&mut self, ctx: &egui::Context) {
        let before = serde_json::to_string(&self.state).unwrap_or_default();
        let previous = (self.state.background, self.state.blur, self.state.opacity);
        let mut reset = false;
        let image = self.image();
        let title = if image {
            "图片调整"
        } else {
            "文字调整"
        };
        let panel_size = vec2(260., if image { 430. } else { 400. });
        let parent = ctx
            .input(|i| i.viewport().outer_rect)
            .unwrap_or(ctx.screen_rect());
        let monitor = ctx
            .input(|i| i.viewport().monitor_size)
            .unwrap_or(vec2(1920., 1080.));
        let work = pin_work_area(self.id, ctx.pixels_per_point())
            .unwrap_or(Rect::from_min_size(Pos2::ZERO, monitor));
        let mut panel_pos = pos2(parent.right() + 12., parent.top());
        if panel_pos.x + panel_size.x > work.right() {
            panel_pos.x = parent.left() - panel_size.x - 12.;
        }
        if panel_pos.x < work.left() {
            panel_pos.x = parent
                .left()
                .clamp(work.left(), (work.right() - panel_size.x).max(work.left()));
            panel_pos.y = parent.bottom() + 12.;
            if panel_pos.y + panel_size.y > work.bottom() {
                panel_pos.y = parent.top() - panel_size.y - 12.;
            }
        }
        panel_pos.y = panel_pos
            .y
            .clamp(work.top(), (work.bottom() - panel_size.y).max(work.top()));
        let id = egui::ViewportId::from_hash_of(("pin-effects", self.id));
        let builder = egui::ViewportBuilder::default()
            .with_title(format!("QuickTool {title} · {}", self.id))
            .with_inner_size(panel_size)
            .with_position(panel_pos)
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
            .with_taskbar(false)
            .with_active(false)
            .with_always_on_top();
        ctx.show_viewport_immediate(id, builder, |ctx, _| {
            if ctx.input(|i| i.viewport().close_requested())
                || (!ctx.wants_keyboard_input() && ctx.input(|i| i.key_pressed(Key::Escape)))
            {
                self.effects = false;
                self.editing = false;
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, Key::Z)) {
                self.undo(false);
            }
            if ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::COMMAND, Key::Y)
                    || i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, Key::Z)
            }) {
                self.undo(true);
            }
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.inner_margin(8.))
                .show(ctx, |ui| {
                    let light = !ui.visuals().dark_mode;
                    let bg = if light {
                        Color32::from_rgb(248, 250, 255)
                    } else {
                        Color32::from_rgb(36, 41, 54)
                    };
                    egui::Frame::new()
                        .fill(bg)
                        .stroke(Stroke::new(
                            1.,
                            if light {
                                pin_border(true)
                            } else {
                                pin_border(self.light)
                            },
                        ))
                        .corner_radius(14.)
                        .inner_margin(16.)
                        .show(ui, |ui| {
                            ui.style_mut().override_font_id = Some(FontId::proportional(12.));
                            ui.visuals_mut().override_text_color = Some(if light {
                                Color32::from_rgb(36, 42, 56)
                            } else {
                                Color32::from_rgb(245, 247, 255)
                            });
                            ui.spacing_mut().interact_size = vec2(24., 24.);
                            let widgets = &mut ui.visuals_mut().widgets;
                            for visuals in [
                                &mut widgets.inactive,
                                &mut widgets.hovered,
                                &mut widgets.active,
                            ] {
                                visuals.weak_bg_fill = Color32::TRANSPARENT;
                                visuals.bg_fill = Color32::TRANSPARENT;
                                visuals.bg_stroke = Stroke::new(
                                    1.,
                                    if light {
                                        pin_border(true)
                                    } else {
                                        pin_border(self.light)
                                    },
                                );
                            }
                            ui.set_min_size(panel_size - vec2(48., 48.));
                            ui.set_max_width(212.);
                            ui.spacing_mut().item_spacing = vec2(6., 4.);
                            ui.spacing_mut().slider_width = 212.;
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(title).size(12.).strong());
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("×").on_hover_text("关闭调整").clicked()
                                        {
                                            self.effects = false;
                                            self.editing = false;
                                        }
                                        if ui.small_button("↺").on_hover_text("重置全部").clicked()
                                        {
                                            reset = true;
                                            if image {
                                                self.state.filters = Filters::default();
                                            } else {
                                                self.reset_text_effects();
                                            }
                                        }
                                    },
                                );
                            });
                            ui.separator();
                            ui.add_space(6.);
                            ui.data_mut(|data| {
                                data.insert_temp(
                                    Id::new("pin-effect-hue"),
                                    self.state.filters.hue as f32,
                                )
                            });
                            if image {
                                slider(
                                    ui,
                                    "亮度",
                                    &mut self.state.filters.brightness,
                                    -150.0..=150.0,
                                    0.,
                                );
                                let mut contrast = if self.state.filters.contrast < 0. {
                                    self.state.filters.contrast * 2.
                                } else {
                                    self.state.filters.contrast
                                };
                                slider(ui, "对比度", &mut contrast, -100.0..=100.0, 0.);
                                self.state.filters.contrast = if contrast < 0. {
                                    contrast / 2.
                                } else {
                                    contrast
                                };
                                if self.state.filters.colorize {
                                    slider(
                                        ui,
                                        "饱和度",
                                        &mut self.state.filters.colorize_saturation,
                                        0.0..=100.0,
                                        25.,
                                    );
                                } else {
                                    slider(
                                        ui,
                                        "饱和度",
                                        &mut self.state.filters.saturation,
                                        -100.0..=100.0,
                                        0.,
                                    );
                                }
                                slider(
                                    ui,
                                    "曝光",
                                    &mut self.state.filters.exposure,
                                    -3.0..=3.0,
                                    0.,
                                );
                                let mut hue = if self.state.filters.colorize {
                                    self.state.filters.hue.rem_euclid(360.)
                                } else {
                                    self.state.filters.hue
                                };
                                let range = if self.state.filters.colorize {
                                    0.0..=359.0
                                } else {
                                    -180.0..=180.0
                                };
                                effect_control_inner(
                                    ui,
                                    "色相",
                                    &mut hue,
                                    range,
                                    0.,
                                    "°",
                                    Some(&mut self.state.filters.colorize),
                                );
                                self.state.filters.hue = if hue > 180. { hue - 360. } else { hue };
                                let mut radius = self.state.radius as f64;
                                effect_control(
                                    ui,
                                    "圆角",
                                    &mut radius,
                                    0.0..=self.size.min_elem() as f64 / 2.,
                                    0.,
                                    " px",
                                );
                                self.state.radius = radius as f32;
                            } else {
                                ui.add_space(3.);
                                ui.label(egui::RichText::new("字号").size(12.));
                                ui.add_space(3.);
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 8.;
                                    if ui.add_sized([32., 30.], egui::Button::new("−")).clicked()
                                    {
                                        self.state.font_size =
                                            font_step(self.state.font_size, false);
                                    }
                                    egui::Frame::new()
                                        .stroke(ui.visuals().widgets.inactive.bg_stroke)
                                        .corner_radius(6.)
                                        .show(ui, |ui| {
                                            ui.spacing_mut().item_spacing.x = 0.;
                                            ui.horizontal(|ui| {
                                                let font_response =
                                                    font_size_input(ui, &mut self.state.font_size);
                                                let popup_id = ui
                                                    .make_persistent_id(Id::new("font-presets"))
                                                    .with("popup");
                                                if font_response.has_focus()
                                                    && ui.input(|i| {
                                                        i.modifiers.alt
                                                            && i.key_pressed(Key::ArrowDown)
                                                    })
                                                {
                                                    ui.memory_mut(|memory| {
                                                        memory.open_popup(popup_id)
                                                    });
                                                }
                                                egui::ComboBox::from_id_salt("font-presets")
                                                    .selected_text("")
                                                    .width(20.)
                                                    .icon(|ui, rect, visuals, _, _| {
                                                        let center = rect.center();
                                                        let stroke =
                                                            Stroke::new(1., visuals.text_color());
                                                        ui.painter().line_segment(
                                                            [
                                                                center + vec2(-3., -1.),
                                                                center + vec2(0., 2.),
                                                            ],
                                                            stroke,
                                                        );
                                                        ui.painter().line_segment(
                                                            [
                                                                center + vec2(0., 2.),
                                                                center + vec2(3., -1.),
                                                            ],
                                                            stroke,
                                                        );
                                                    })
                                                    .show_ui(ui, |ui| {
                                                        for n in [
                                                            8., 9., 10., 11., 12., 14., 16., 18.,
                                                            20., 24., 28., 32., 36., 40., 44., 48.,
                                                            54., 60., 66., 72., 80., 88., 96.,
                                                        ] {
                                                            ui.selectable_value(
                                                                &mut self.state.font_size,
                                                                n,
                                                                format!("{n} px"),
                                                            );
                                                        }
                                                    });
                                            });
                                        });
                                    if ui.add_sized([32., 30.], egui::Button::new("+")).clicked() {
                                        self.state.font_size =
                                            font_step(self.state.font_size, true);
                                    }
                                });
                                ui.add_space(2.);
                                ui.label(egui::RichText::new("背景颜色").size(12.));
                                ui.add_space(-4.);
                                egui::Frame::new()
                                    .stroke(ui.visuals().widgets.inactive.bg_stroke)
                                    .corner_radius(6.)
                                    .inner_margin(4.)
                                    .show(ui, |ui| {
                                        ui.spacing_mut().interact_size =
                                            vec2(ui.available_width(), 22.);
                                        ui.color_edit_button_srgb(&mut self.state.background);
                                    });
                                ui.add_space(4.);
                                ui.label(egui::RichText::new("文字颜色").size(12.));
                                ui.add_space(4.);
                                ui.horizontal(|ui| {
                                    let mut manual = self.state.foreground.is_some();
                                    let before = manual;
                                    let width =
                                        ui.available_width() - if manual { 48. } else { 0. };
                                    egui::ComboBox::from_id_salt("text-color-mode")
                                        .selected_text(if manual {
                                            "自选颜色"
                                        } else {
                                            "自动黑白"
                                        })
                                        .width(width)
                                        .height(26.)
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(&mut manual, false, "自动黑白");
                                            ui.selectable_value(&mut manual, true, "自选颜色");
                                        });
                                    if before != manual {
                                        self.state.foreground =
                                            if manual { Some([255, 255, 255]) } else { None };
                                    }
                                    if let Some(c) = &mut self.state.foreground {
                                        ui.color_edit_button_srgb(c);
                                    }
                                });
                                ui.add_space(4.);
                                let mut blur = self.state.blur as f64;
                                effect_control(ui, "背景模糊", &mut blur, 0.0..=48., 28., "");
                                self.state.blur = blur as f32;
                                let mut transparency = (100. - self.state.opacity * 100.) as f64;
                                effect_control(
                                    ui,
                                    "透明度",
                                    &mut transparency,
                                    0.0..=100.,
                                    22.,
                                    "%",
                                );
                                self.state.opacity = 1. - transparency as f32 / 100.;
                            }
                        });
                });
            self.capture_viewport(ctx, true);
            self.popup_open |=
                ctx.memory(|memory| memory.any_popup_open()) || ctx.wants_keyboard_input();
        });
        if !reset {
            if previous.0 != self.state.background {
                self.state.auto_background = false;
            }
            if previous.1 != self.state.blur {
                self.state.auto_blur = false;
            }
            if previous.2 != self.state.opacity {
                self.state.auto_opacity = false;
            }
        }
        if before != serde_json::to_string(&self.state).unwrap_or_default() {
            self.record(Some("effects"));
        }
    }
    fn capture_viewport(&mut self, ctx: &egui::Context, effects: bool) {
        let Some(capture) = &mut self.capture else {
            return;
        };
        let started = capture.started.get_or_insert_with(std::time::Instant::now);
        if started.elapsed() > std::time::Duration::from_secs(30) {
            eprintln!("Pin screenshot timed out before all requested viewports completed");
            ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
            return;
        }
        if !effects {
            capture.frames += 1;
        }
        let path = if effects {
            capture.effects.clone()
        } else {
            capture.pin.clone()
        };
        let Some(path) = path else {
            return;
        };
        let sent = if effects {
            capture.effects_sent
        } else {
            capture.pin_sent
        };
        if capture.frames >= 10 && (self.source.is_none() || self.texture.is_some()) && !sent {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            if effects {
                capture.effects_sent = true;
            } else {
                capture.pin_sent = true;
            }
        }
        let screenshot = ctx.input(|input| {
            input.events.iter().find_map(|event| {
                if let egui::Event::Screenshot { image, .. } = event {
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
                .flat_map(|pixel| pixel.to_srgba_unmultiplied())
                .collect::<Vec<_>>();
            if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    eprintln!("Pin screenshot directory creation failed: {e}");
                    ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
                    return;
                }
            }
            match image::save_buffer(
                &path,
                &bytes,
                image.width() as u32,
                image.height() as u32,
                image::ColorType::Rgba8,
            ) {
                Ok(()) => {
                    if effects {
                        capture.effects_done = true;
                    } else {
                        capture.pin_done = true;
                    }
                }
                Err(e) => {
                    self.status = e.to_string();
                    eprintln!("Pin screenshot failed: {e}");
                    ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
                }
            }
            if (capture.pin.is_none() || capture.pin_done)
                && (capture.effects.is_none() || capture.effects_done)
            {
                ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(60));
    }
    fn begin_move(&mut self, ctx: &egui::Context) {
        #[cfg(windows)]
        {
            use windows_sys::Win32::{
                Foundation::{POINT, RECT},
                UI::WindowsAndMessaging::*,
            };
            let title = pin_title(self.id)
                .encode_utf16()
                .chain(Some(0))
                .collect::<Vec<_>>();
            let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
            let mut cursor = POINT { x: 0, y: 0 };
            let mut rect: RECT = unsafe { std::mem::zeroed() };
            if !hwnd.is_null()
                && unsafe { GetCursorPos(&mut cursor) } != 0
                && unsafe { GetWindowRect(hwnd, &mut rect) } != 0
            {
                self.window_move = Some(WindowMove {
                    hwnd: hwnd as isize,
                    cursor,
                    rect,
                    corner: None,
                    dpi: ctx.pixels_per_point(),
                });
                ctx.request_repaint();
                return;
            }
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    fn update_move(&mut self, ctx: &egui::Context) {
        #[cfg(windows)]
        if let Some(moving) = &self.window_move {
            use windows_sys::Win32::{
                Foundation::POINT,
                UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
            };
            let mut cursor = POINT { x: 0, y: 0 };
            if unsafe { GetCursorPos(&mut cursor) } == 0 {
                self.window_move = None;
                return;
            }
            let delta = vec2(
                (cursor.x - moving.cursor.x) as f32,
                (cursor.y - moving.cursor.y) as f32,
            );
            let shift = unsafe { GetAsyncKeyState(VK_SHIFT as i32) } < 0;
            let resizing = moving.corner.is_some();
            if let Some(corner) = moving.corner {
                let dpi = moving.dpi;
                let r = moving.rect;
                let start = Rect::from_min_max(
                    pos2(r.left as f32 / dpi, r.top as f32 / dpi),
                    pos2(r.right as f32 / dpi, r.bottom as f32 / dpi),
                );
                let natural = self
                    .source
                    .as_ref()
                    .map(|im| vec2(im.width() as f32, im.height() as f32));
                let next = geometry::resize_window(
                    start,
                    corner,
                    pos2(moving.cursor.x as f32 / dpi, moving.cursor.y as f32 / dpi),
                    pos2(cursor.x as f32 / dpi, cursor.y as f32 / dpi),
                    natural,
                    self.editing,
                    shift,
                );
                unsafe {
                    SetWindowPos(
                        moving.hwnd as _,
                        std::ptr::null_mut(),
                        (next.left() * dpi).round() as i32,
                        (next.top() * dpi).round() as i32,
                        (next.width() * dpi).round() as i32,
                        (next.height() * dpi).round() as i32,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
                self.resize_target = Some(vec2(
                    next.width() - 16.,
                    next.height() - if self.editing { 64. } else { 16. },
                ));
            } else {
                let delta = if shift {
                    geometry::direction(delta)
                } else {
                    delta
                };
                unsafe {
                    SetWindowPos(
                        moving.hwnd as _,
                        std::ptr::null_mut(),
                        moving.rect.left + delta.x.round() as i32,
                        moving.rect.top + delta.y.round() as i32,
                        0,
                        0,
                        SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
            }
            if unsafe { GetAsyncKeyState(VK_LBUTTON as i32) } >= 0 {
                self.window_move = None;
                if resizing {
                    self.resize_finished = true;
                }
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(8));
            }
        }
    }
    fn begin_resize(
        &mut self,
        ctx: &egui::Context,
        corner: &'static str,
        direction: egui::ResizeDirection,
    ) {
        #[cfg(windows)]
        {
            self.begin_move(ctx);
            if let Some(moving) = &mut self.window_move {
                moving.corner = Some(corner);
                return;
            }
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
    }
    fn apply_native_style(&mut self, ctx: &egui::Context) {
        #[cfg(windows)]
        if !self.image() {
            let key = format!("{}:{}:{}", self.state.blur, self.state.radius, self.light);
            if key != self.native_style_key || self.backdrop.is_some() {
                let title = pin_title(self.id)
                    .encode_utf16()
                    .chain(Some(0))
                    .collect::<Vec<_>>();
                // The title includes this process ID and pin ID, so another app's window is never styled.
                let hwnd = unsafe {
                    windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW(
                        std::ptr::null(),
                        title.as_ptr(),
                    )
                };
                if !hwnd.is_null() {
                    if key != self.native_style_key {
                        let _ = unsafe {
                            crate::platform::apply_window_style(hwnd as isize, !self.light, 0, 0)
                        };
                    }
                    if self.backdrop.is_none() {
                        match crate::backdrop::Backdrop::new(hwnd as isize) {
                            Ok(backdrop) => self.backdrop = Some(backdrop),
                            Err(e) => self.status = e,
                        }
                    }
                    if let Some(backdrop) = &mut self.backdrop {
                        let dpi = ctx.pixels_per_point();
                        if let Err(e) = backdrop.update(
                            true,
                            true,
                            !self.light,
                            self.state.blur,
                            (12. * dpi).round() as i32,
                            12. * dpi,
                        ) {
                            self.status = e;
                        }
                    }
                    self.native_style_key = key;
                }
            }
        }
    }
    fn resize_handles(&mut self, ctx: &egui::Context, card: Rect) {
        for (corner, p, direction) in [
            (
                "nw",
                card.left_top() - vec2(8., 8.),
                egui::ResizeDirection::NorthWest,
            ),
            (
                "ne",
                card.right_top() + vec2(-20., -8.),
                egui::ResizeDirection::NorthEast,
            ),
            (
                "sw",
                card.left_bottom() + vec2(-8., -20.),
                egui::ResizeDirection::SouthWest,
            ),
            (
                "se",
                card.right_bottom() - vec2(20., 20.),
                egui::ResizeDirection::SouthEast,
            ),
        ] {
            egui::Area::new(Id::new((self.id, "resize", corner)))
                .fixed_pos(p)
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    let (_, response) = ui.allocate_exact_size(vec2(28., 28.), Sense::drag());
                    let response = response.on_hover_cursor(match direction {
                        egui::ResizeDirection::NorthWest | egui::ResizeDirection::SouthEast => {
                            egui::CursorIcon::ResizeNwSe
                        }
                        _ => egui::CursorIcon::ResizeNeSw,
                    });
                    if response.drag_started() {
                        self.begin_resize(ctx, corner, direction);
                    }
                });
        }
        // Keep the physical image aspect ratio while resizing the native window.
        #[cfg(not(windows))]
        if self.image() && self.last_size != card.size() {
            if let Some(source) = &self.source {
                let ratio = source.width() as f32 / source.height() as f32;
                let expected_h = card.width() / ratio;
                if (card.height() - expected_h).abs() > 2. {
                    let bottom = if self.editing { 64. } else { 16. };
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(vec2(
                        card.width() + 16.,
                        expected_h + bottom,
                    )));
                }
            }
        }
        self.last_size = card.size();
    }
}
fn pin_work_area(id: u64, dpi: f32) -> Option<Rect> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::{Graphics::Gdi::*, UI::WindowsAndMessaging::FindWindowW};
        let title = pin_title(id)
            .encode_utf16()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
        if hwnd.is_null() {
            return None;
        }
        let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
        let mut info: MONITORINFO = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if unsafe { GetMonitorInfoW(monitor, &mut info) } != 0 {
            let r = info.rcWork;
            return Some(Rect::from_min_max(
                pos2(r.left as f32 / dpi, r.top as f32 / dpi),
                pos2(r.right as f32 / dpi, r.bottom as f32 / dpi),
            ));
        }
    }
    None
}
fn font_size_input(ui: &mut egui::Ui, value: &mut f32) -> egui::Response {
    let id = ui.make_persistent_id("pin-font-size-input");
    let editing = ui.memory(|memory| memory.has_focus(id));
    let mut text = if editing {
        ui.data(|data| data.get_temp::<String>(id))
            .unwrap_or_else(|| value.to_string())
    } else {
        value.to_string()
    };
    let response = ui.add_sized(
        [75., 28.],
        egui::TextEdit::singleline(&mut text)
            .id(id)
            .frame(false)
            .font(FontId::new(12., egui::FontFamily::Name("Jura".into())))
            .vertical_align(egui::Align::Center)
            .margin(vec2(3., 3.)),
    );
    let enter = response.has_focus() && ui.input(|input| input.key_pressed(Key::Enter));
    if response.lost_focus() || enter {
        if let Ok(parsed) = text.parse::<f32>() {
            if parsed.is_finite() {
                *value = parsed.clamp(8., 96.);
            }
        }
        if enter {
            ui.memory_mut(|memory| memory.surrender_focus(id));
        }
        ui.data_mut(|data| data.remove::<String>(id));
    } else if response.has_focus() {
        ui.data_mut(|data| data.insert_temp(id, text));
    }
    response
}
fn pin_border(light: bool) -> Color32 {
    // CSS composites these borders on the opaque control background in sRGB.
    // Precompositing preserves that color through the native renderer and window compositor.
    if light {
        Color32::from_rgb(201, 212, 233)
    } else {
        Color32::from_rgb(71, 81, 106)
    }
}
fn pin_icon_button(
    ui: &mut egui::Ui,
    name: &str,
    label: &str,
    selected: bool,
    framed: bool,
) -> egui::Response {
    type Icons = Arc<std::collections::HashMap<String, egui::TextureHandle>>;
    let cache_id = Id::new("quicktool-pin-icons");
    let icons=ui.ctx().data_mut(|data|data.get_temp::<Icons>(cache_id)).unwrap_or_else(||{
        let source:serde_json::Value=serde_json::from_str(include_str!("../assets/pin-icons.json")).unwrap_or_default();let mut icons=std::collections::HashMap::new();
        if let Some(map)=source.as_object(){for(name,icon)in map{let Some(body)=icon["body"].as_str()else{continue;};let svg=format!("<svg xmlns='http://www.w3.org/2000/svg' width='24' height='24' viewBox='0 0 24 24'>{}</svg>",body.replace("currentColor","#ffffff"));if let Ok(tree)=resvg::usvg::Tree::from_str(&svg,&Default::default()){if let Some(mut pm)=resvg::tiny_skia::Pixmap::new(48,48){resvg::render(&tree,resvg::tiny_skia::Transform::from_scale(2.,2.),&mut pm.as_mut());let image=egui::ColorImage::from_rgba_premultiplied([48,48],pm.data());icons.insert(name.clone(),ui.ctx().load_texture(format!("pin-icon-{name}"),image,egui::TextureOptions::LINEAR));}}}}
        let icons=Arc::new(icons);ui.ctx().data_mut(|data|data.insert_temp(cache_id,icons.clone()));icons
    });
    let (rect, response) = ui.allocate_exact_size(
        if framed {
            vec2(28., 28.)
        } else {
            vec2(26., 27.)
        },
        Sense::click(),
    );
    let text = if ui.visuals().dark_mode {
        Color32::from_rgb(245, 247, 255)
    } else {
        Color32::from_rgb(36, 42, 56)
    };
    let bg = if selected || response.hovered() {
        if ui.visuals().dark_mode {
            Color32::from_rgb(38, 62, 92)
        } else {
            Color32::from_rgb(211, 232, 255)
        }
    } else if framed {
        if ui.visuals().dark_mode {
            Color32::from_rgb(36, 41, 54)
        } else {
            Color32::from_rgb(248, 250, 255)
        }
    } else {
        Color32::TRANSPARENT
    };
    ui.painter()
        .rect_filled(rect, if framed { 8 } else { 5 }, bg);
    if framed {
        ui.painter().rect_stroke(
            rect,
            8,
            Stroke::new(1., pin_border(!ui.visuals().dark_mode)),
            egui::StrokeKind::Inside,
        );
    }
    if let Some(icon) = icons.get(name) {
        ui.painter().image(
            icon.id(),
            Rect::from_center_size(rect.center(), vec2(18., 18.)),
            Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
            text,
        );
    }
    response.on_hover_text(label)
}
fn pin_title(id: u64) -> String {
    format!("QuickTool Pin · {}/{id}", std::process::id())
}
fn new_history_id(id: u64) -> String {
    let mut bytes = [0u8; 16];
    if getrandom::fill(&mut bytes).is_ok() {
        format!("pin-{:032x}", u128::from_le_bytes(bytes))
    } else {
        format!("pin-{}-{}-{id}", now(), std::process::id())
    }
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
fn slider(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    reset: f64,
) {
    effect_control(
        ui,
        label,
        value,
        range,
        reset,
        if label == "曝光" {
            " EV"
        } else if label == "饱和度" {
            "%"
        } else {
            ""
        },
    );
}
fn effect_control(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    reset: f64,
    suffix: &str,
) {
    effect_control_inner(ui, label, value, range, reset, suffix, None);
}
fn effect_control_inner(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    reset: f64,
    suffix: &str,
    mut tint: Option<&mut bool>,
) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 0.;
        ui.horizontal(|ui| {
            let heading = ui
                .add(egui::Label::new(egui::RichText::new(label).size(12.)).sense(Sense::click()));
            if heading.double_clicked() && label != "背景模糊" && label != "透明度" {
                *value = reset;
                if let Some(tint) = tint.as_deref_mut() {
                    *tint = false;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().interact_size = vec2(0., 18.);
                ui.spacing_mut().button_padding = Vec2::ZERO;
                if let Some(tint) = tint {
                    ui.checkbox(tint, "着色");
                }
                ui.style_mut().override_font_id =
                    Some(FontId::new(11., egui::FontFamily::Name("Jura".into())));
                let widgets = &mut ui.visuals_mut().widgets;
                widgets.inactive.bg_stroke = Stroke::NONE;
                widgets.hovered.bg_stroke = Stroke::NONE;
                widgets.active.bg_stroke = Stroke::NONE;
                if label == "背景模糊" || label == "透明度" {
                    if label == "背景模糊" && *value == 0. {
                        ui.label(egui::RichText::new("关闭").font(FontId::proportional(11.)));
                    } else {
                        ui.label(format!("{value:.0}{suffix}"));
                    }
                } else {
                    let mut shown = if label == "对比度" && *value < 0. {
                        *value / 2.
                    } else {
                        *value
                    };
                    let display_range = if label == "对比度" {
                        -50.0..=100.0
                    } else {
                        range.clone()
                    };
                    let r = ui.add(
                        egui::DragValue::new(&mut shown)
                            .update_while_editing(false)
                            .range(display_range)
                            .speed(if label == "曝光" {
                                0.1
                            } else if label == "对比度" {
                                0.5
                            } else {
                                1.
                            })
                            .suffix(suffix)
                            .custom_formatter(|value, _| {
                                if value.fract().abs() < 0.001 {
                                    format!("{value:.0}")
                                } else {
                                    format!("{value:.1}")
                                }
                            }),
                    );
                    if r.changed() {
                        *value = if label == "对比度" && shown < 0. {
                            shown * 2.
                        } else {
                            shown
                        };
                    }
                }
            });
        });
        ui.spacing_mut().slider_width = ui.available_width();
        let min = *range.start();
        let max = *range.end();
        let response = ui.add_sized(
            [ui.available_width(), 24.],
            egui::Slider::new(value, range)
                .show_value(false)
                .step_by(if label == "曝光" { 0.1 } else { 1. }),
        );
        // Keep egui's keyboard, pointer and accessibility handling; draw the original gradient rail and circular thumb.
        let rail = Rect::from_center_size(
            response.rect.center(),
            vec2((response.rect.width() - 6.).max(1.), 8.),
        );
        let stops: Vec<Color32> = match label {
            "色相" => vec![
                Color32::CYAN,
                Color32::BLUE,
                Color32::MAGENTA,
                Color32::RED,
                Color32::YELLOW,
                Color32::GREEN,
                Color32::CYAN,
            ],
            "饱和度" => vec![
                Color32::from_gray(146),
                Color32::from(egui::ecolor::Hsva::new(
                    ui.data(|data| {
                        data.get_temp::<f32>(Id::new("pin-effect-hue"))
                            .unwrap_or(0.)
                    })
                    .rem_euclid(360.)
                        / 360.,
                    1.,
                    1.,
                    1.,
                )),
            ],
            "圆角" => vec![
                Color32::from_rgb(83, 103, 127),
                Color32::from_rgb(170, 187, 216),
            ],
            _ => vec![Color32::from_rgb(23, 25, 30), Color32::WHITE],
        };
        let painter = ui.painter();
        for i in 0..128 {
            let position = i as f32 / 127. * (stops.len() - 1) as f32;
            let low = (position.floor() as usize).min(stops.len() - 2);
            let t = (position - low as f32).min(1.);
            let left = stops[low].to_array();
            let right = stops[low + 1].to_array();
            let rgb = std::array::from_fn::<_, 3, _>(|c| {
                (left[c] as f32 * (1. - t) + right[c] as f32 * t).round() as u8
            });
            let r = Rect::from_min_max(
                pos2(rail.left() + rail.width() * i as f32 / 128., rail.top()),
                pos2(
                    rail.left() + rail.width() * (i + 1) as f32 / 128.,
                    rail.bottom(),
                ),
            );
            let edge = (r.center().x - rail.left()).min(rail.right() - r.center().x);
            let half = if edge < 4. {
                (16. - (4. - edge).powi(2)).max(0.).sqrt()
            } else {
                4.
            };
            let rounded = Rect::from_min_max(
                pos2(r.left(), rail.center().y - half),
                pos2(r.right(), rail.center().y + half),
            );
            if label == "对比度" {
                let t = i as f32 / 127.;
                let upper = (119. * (1. - t) + 36. * t).round() as u8;
                let lower = (146. * (1. - t) + 255. * t).round() as u8;
                painter.rect_filled(
                    Rect::from_min_max(rounded.min, pos2(rounded.right(), rail.center().y)),
                    0.,
                    Color32::from_gray(upper),
                );
                painter.rect_filled(
                    Rect::from_min_max(pos2(rounded.left(), rail.center().y), rounded.max),
                    0.,
                    Color32::from_gray(lower),
                );
            } else {
                painter.rect_filled(rounded, 0., Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
            }
        }
        painter.rect_stroke(
            rail,
            4.,
            Stroke::new(1., Color32::from_rgba_unmultiplied(128, 128, 128, 53)),
            egui::StrokeKind::Inside,
        );
        let fraction = if max > min {
            ((*value - min) / (max - min)).clamp(0., 1.) as f32
        } else {
            0.
        };
        let center = pos2(
            response.rect.left() + 7. + fraction * (response.rect.width() - 14.),
            rail.center().y,
        );
        painter.circle_filled(center, 8., Color32::from_rgb(113, 132, 184));
        painter.circle_filled(center, 7., ui.visuals().panel_fill);
        painter.circle_filled(center, 5., Color32::from_rgb(244, 246, 252));
        if label == "对比度" {
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 14.), Sense::hover());
            for (position, align, text) in [
                (rect.left_center(), Align2::LEFT_CENTER, "柔和"),
                (rect.center(), Align2::CENTER_CENTER, "0"),
                (rect.right_center(), Align2::RIGHT_CENTER, "鲜明"),
            ] {
                ui.painter().text(
                    position,
                    align,
                    text,
                    FontId::proportional(9.),
                    ui.visuals().weak_text_color(),
                );
            }
        }
    });
}

fn font_step(current: f32, increase: bool) -> f32 {
    const STEPS: [f32; 23] = [
        8., 9., 10., 11., 12., 14., 16., 18., 20., 24., 28., 32., 36., 40., 44., 48., 54., 60.,
        66., 72., 80., 88., 96.,
    ];
    if increase {
        STEPS
            .into_iter()
            .find(|v| *v > current + 0.01)
            .unwrap_or(96.)
    } else {
        STEPS
            .into_iter()
            .rev()
            .find(|v| *v < current - 0.01)
            .unwrap_or(8.)
    }
}

#[derive(Default)]
pub struct PinManager {
    pins: Vec<Pin>,
    next: u64,
    history_updates: Vec<PinHistoryUpdate>,
    shortcut_recording: bool,
    #[cfg(windows)]
    escape: Option<crate::platform::PinEscape>,
    #[cfg(windows)]
    escape_failed: bool,
}
impl PinManager {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn show_effects_for_all(&mut self) {
        for pin in &mut self.pins {
            pin.effects = true;
            if pin.image() {
                pin.editing = true;
            }
        }
    }
    pub fn request_screenshot(&mut self, path: &Path, effects: bool) {
        if let Some(pin) = self.pins.last_mut() {
            let capture = pin.capture.get_or_insert_with(PinCapture::default);
            if effects {
                capture.effects = Some(path.into());
                pin.effects = true;
                pin.editing = pin.source.is_some();
            } else {
                capture.pin = Some(path.into());
            }
        }
    }
    pub fn configure(&mut self, light: bool, blur: f32, opacity: f32) {
        for pin in &mut self.pins {
            pin.light = light;
            pin.default_blur = blur.clamp(0., 48.);
            pin.default_opacity = opacity.clamp(0., 1.);
            if pin.state.auto_background {
                pin.state.background = if light { [248, 250, 255] } else { [25, 28, 37] };
            }
            if pin.state.auto_blur {
                pin.state.blur = blur.clamp(0., 48.);
            }
            if pin.state.auto_opacity {
                pin.state.opacity = opacity.clamp(0., 1.);
            }
        }
    }
    pub fn set_shortcut_recording(&mut self, recording: bool) {
        self.shortcut_recording = recording;
    }
    pub fn native_windows(&self) -> Vec<(u64, isize)> {
        #[cfg(windows)]
        {
            self.pins
                .iter()
                .filter(|pin| pin.open)
                .filter_map(|pin| {
                    let title = pin_title(pin.id)
                        .encode_utf16()
                        .chain(Some(0))
                        .collect::<Vec<_>>();
                    let hwnd = unsafe {
                        windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW(
                            std::ptr::null(),
                            title.as_ptr(),
                        )
                    };
                    if hwnd.is_null() {
                        None
                    } else {
                        Some((pin.id, hwnd as isize))
                    }
                })
                .collect()
        }
        #[cfg(not(windows))]
        Vec::new()
    }
    pub fn create_text(&mut self, text: String) {
        self.next += 1;
        self.pins.push(Pin::new(self.next, text, None));
    }
    pub fn restore(&mut self, record: PinRecord, source: Option<RgbaImage>) -> Result<(), String> {
        if let Some(pin) = self
            .pins
            .iter_mut()
            .find(|p| p.history_id == record.history_id)
        {
            pin.open = true;
            return Ok(());
        }
        if record.kind == "image" && source.is_none() {
            return Err("历史图片资源缺失".into());
        }
        if record.kind != "image" && record.kind != "text" {
            return Err("历史Pin类型无效".into());
        }
        self.next += 1;
        let mut pin = Pin::new(self.next, record.original_text.clone(), source);
        pin.state = PinState {
            text: record.text,
            filters: record.filters.normalized(),
            annotations: record
                .annotations
                .into_iter()
                .take(200)
                .map(|mut a| {
                    a.normalize();
                    a
                })
                .collect(),
            radius: record.corner_radius.max(0.),
            font_size: record.font_size.clamp(8., 96.),
            background: record.background,
            foreground: record.foreground,
            opacity: record.opacity.clamp(0., 1.),
            blur: record.blur.clamp(0., 48.),
            auto_background: record.style_automatic.unwrap_or([false; 3])[0],
            auto_blur: record.style_automatic.unwrap_or([false; 3])[1],
            auto_opacity: record.style_automatic.unwrap_or([false; 3])[2],
            width: record.width.round().clamp(300., 1600.),
            height: record.height.round().clamp(100., 1400.),
        };
        pin.next_annotation = pin
            .state
            .annotations
            .iter()
            .map(|a| a.id)
            .max()
            .unwrap_or(0)
            + 1;
        pin.history = Undo::new(pin.state.clone(), 100, 8 * 1024 * 1024);
        pin.history_id = record.history_id;
        pin.asset = record.asset;
        pin.created = record.created;
        pin.size = vec2(
            record.width.round().clamp(300., 1600.),
            record.height.round().clamp(100., 1400.),
        );
        pin.annotation_width = pin.size.x;
        pin.position = record.position;
        self.pins.push(pin);
        Ok(())
    }
    pub fn drain_history_updates(&mut self) -> Vec<PinHistoryUpdate> {
        std::mem::take(&mut self.history_updates)
    }
    pub fn flush_history(&mut self) {
        for pin in &mut self.pins {
            pin.commit_text();
            let record = pin.history_record();
            let key = serde_json::to_string(&record).unwrap_or_default();
            if key != pin.history_key {
                self.history_updates.push(PinHistoryUpdate {
                    record,
                    source: pin.source.clone(),
                });
                pin.history_key = key;
                pin.last_history_sent = now();
            }
        }
    }
    pub fn create_image(&mut self, width: u32, height: u32, rgba: Vec<u8>) -> Result<(), String> {
        if width == 0 || height == 0 || width as u64 * height as u64 > 16_000_000 {
            return Err("图片超过 1600 万像素，请先缩小尺寸".into());
        }
        let image = RgbaImage::from_raw(width, height, rgba).ok_or("RGBA 图片数据长度无效")?;
        self.next += 1;
        self.pins
            .push(Pin::new(self.next, String::new(), Some(image)));
        Ok(())
    }
    pub fn create_image_bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() > 192 * 1024 * 1024 {
            return Err("图片文件过大".into());
        }
        let image = if bytes.starts_with(b"<svg")
            || bytes.starts_with(b"<?xml")
            || bytes.windows(4).take(1024).any(|s| s == b"<svg")
        {
            let options = annotations::svg_options();
            let tree = resvg::usvg::Tree::from_data(bytes, &options).map_err(|e| e.to_string())?;
            let size = tree.size().to_int_size();
            if size.width() as u64 * size.height() as u64 > 16_000_000 {
                return Err("SVG 尺寸过大".into());
            }
            let mut pm =
                resvg::tiny_skia::Pixmap::new(size.width(), size.height()).ok_or("SVG 尺寸无效")?;
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::identity(),
                &mut pm.as_mut(),
            );
            let mut rgba = pm.data().to_vec();
            for p in rgba.chunks_exact_mut(4) {
                if p[3] > 0 {
                    for c in 0..3 {
                        p[c] = ((p[c] as u32 * 255 + p[3] as u32 / 2) / p[3] as u32).min(255) as u8;
                    }
                }
            }
            RgbaImage::from_raw(size.width(), size.height(), rgba).ok_or("SVG 图片数据无效")?
        } else {
            let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
                .with_guessed_format()
                .map_err(|e| e.to_string())?;
            let mut limits = image::Limits::default();
            limits.max_alloc = Some(256 * 1024 * 1024);
            reader.limits(limits);
            reader.decode().map_err(|e| e.to_string())?.to_rgba8()
        };
        let (w, h) = image.dimensions();
        self.create_image(w, h, image.into_raw())
    }
    pub fn create_image_path(&mut self, path: &Path) -> Result<(), String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        self.create_image_bytes(&bytes)
    }
    pub fn paste_clipboard(&mut self) -> Result<(), String> {
        let (w, h, rgba) = crate::platform::clipboard_image()?;
        self.create_image(w, h, rgba)
    }
    pub fn len(&self) -> usize {
        self.pins.len()
    }
    pub fn is_empty(&self) -> bool {
        self.pins.is_empty()
    }
    pub fn show(&mut self, ctx: &egui::Context) {
        #[cfg(windows)]
        {
            if !self.pins.is_empty() && self.escape.is_none() && !self.escape_failed {
                let wake = ctx.clone();
                match crate::platform::PinEscape::new(move || wake.request_repaint()) {
                    Ok(escape) => self.escape = Some(escape),
                    Err(e) => {
                        eprintln!("Pin Escape initialization failed: {e}");
                        self.escape_failed = true;
                    }
                }
            }
            if let Some(escape) = &self.escape {
                for id in escape.events.try_iter() {
                    if let Some(pin) = self.pins.iter_mut().find(|pin| pin.id == id) {
                        pin.open = false;
                    }
                }
            }
        }
        for pin in &mut self.pins {
            if !pin.open {
                continue;
            }
            let id = egui::ViewportId::from_hash_of(("quicktool-pin", pin.id));
            let size = pin.size + vec2(16., if pin.editing { 64. } else { 16. });
            let mut builder = egui::ViewportBuilder::default()
                .with_title(pin_title(pin.id))
                .with_decorations(false)
                .with_transparent(true)
                .with_resizable(true)
                .with_taskbar(false)
                .with_active(false)
                .with_inner_size(size)
                .with_min_inner_size(vec2(if pin.image() { 316. } else { 216. }, 136.))
                .with_max_inner_size(vec2(2400., 2000.))
                .with_always_on_top();
            if !pin.position_initialized {
                if let Some(p) = pin.position {
                    builder = builder.with_position(pos2(p[0], p[1]));
                }
                pin.position_initialized = true;
            }
            ctx.show_viewport_immediate(id, builder, |ctx, class| {
                if class == egui::ViewportClass::Embedded {
                    let mut open = pin.open;
                    egui::Window::new("QuickTool Pin")
                        .id(Id::new(pin.id))
                        .open(&mut open)
                        .show(ctx, |ui| {
                            ui.label("当前渲染后端不支持独立窗口");
                        });
                    pin.open = open;
                } else {
                    pin.show(ctx);
                }
            });
        }
        for pin in &mut self.pins {
            if !pin.open {
                pin.commit_text();
            }
            let record = pin.history_record();
            let key = serde_json::to_string(&record).unwrap_or_default();
            let time = now();
            if key != pin.history_key
                && (pin.history_key.is_empty()
                    || !pin.open
                    || time.saturating_sub(pin.last_history_sent) >= 650)
            {
                self.history_updates.push(PinHistoryUpdate {
                    record,
                    source: pin.source.clone(),
                });
                pin.history_key = key;
                pin.last_history_sent = time;
            } else if key != pin.history_key {
                ctx.request_repaint_after(std::time::Duration::from_millis(
                    650 - time.saturating_sub(pin.last_history_sent),
                ));
            }
        }
        self.pins.retain(|pin| pin.open);
        #[cfg(windows)]
        {
            let windows = self.native_windows();
            let suspended = self.shortcut_recording || self.pins.iter().any(|pin| pin.popup_open);
            if let Some(escape) = &mut self.escape {
                if let Err(e) = escape.update(windows, suspended) {
                    eprintln!("Pin Escape update failed: {e}");
                }
            }
            if self.pins.is_empty() {
                if self
                    .escape
                    .as_ref()
                    .is_none_or(|escape| !escape.status().registered)
                {
                    self.escape = None;
                    self.escape_failed = false;
                } else {
                    ctx.request_repaint_after(std::time::Duration::from_millis(30));
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_image_rejected() {
        let mut p = PinManager::new();
        assert!(p.create_image(0, 1, vec![]).is_err());
        assert!(p.create_image(2, 2, vec![0; 4]).is_err());
    }
    #[test]
    fn decode_png_and_svg() {
        let mut p = PinManager::new();
        p.create_image_bytes(br##"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="3"><rect width="2" height="3" fill="#ff0000"/></svg>"##).unwrap();
        let image = p.pins[0].source.as_ref().unwrap();
        assert_eq!(image.dimensions(), (2, 3));
        assert_eq!(image.get_pixel(0, 0).0, [255, 0, 0, 255]);
    }
    #[test]
    fn font_steps() {
        assert_eq!(font_step(15., true), 16.);
        assert_eq!(font_step(15., false), 14.);
    }
    #[test]
    fn history_restores_edits_and_initial_text() {
        let mut p = PinManager::new();
        p.create_text("original".into());
        p.pins[0].state.text = "edited".into();
        p.pins[0].state.blur = 9.;
        p.pins[0].state.font_size = 32.;
        p.flush_history();
        let record = p.drain_history_updates().pop().unwrap().record;
        assert_eq!(record.text, "edited");
        assert_eq!(record.original_text, "original");
        let history_id = record.history_id.clone();
        let mut q = PinManager::new();
        q.restore(record, None).unwrap();
        assert_eq!(q.pins[0].history_id, history_id);
        assert_eq!(q.pins[0].state.font_size, 32.);
        assert_eq!(q.pins[0].state.blur, 9.);
        assert_eq!(q.pins[0].initial_text, "original");
        q.flush_history();
        assert_eq!(q.drain_history_updates().len(), 1);
        q.flush_history();
        assert!(q.drain_history_updates().is_empty());
    }
    #[test]
    fn clear_cancels_pending_annotation_edits() {
        let mut p = Pin::new(1, String::new(), Some(RgbaImage::new(200, 100)));
        let a = p.defaults.make(1, Tool::Text, pos2(0.2, 0.3), 300.);
        p.state.annotations.push(a.clone());
        p.selected = Some(1);
        p.draft = Some(a.clone());
        p.drag = Some(Drag::New(a.clone()));
        p.text_editor = Some(a);
        p.text_editor_focus = true;
        p.radius_editor = Some(1);
        p.clear_annotations();
        p.commit_text();
        assert!(p.state.annotations.is_empty());
        assert!(p.selected.is_none() && p.draft.is_none() && p.drag.is_none());
        assert!(p.text_editor.is_none() && p.radius_editor.is_none());
        assert!(!p.text_editor_focus);
    }
    #[test]
    fn image_resize_preserves_screen_stroke_and_mosaic_block_size() {
        let mut p = Pin::new(1, String::new(), Some(RgbaImage::new(300, 200)));
        for (id, tool, width) in [(1, Tool::Line, 3.), (2, Tool::Mosaic, 12.)] {
            let mut a = p.defaults.make(id, tool, pos2(0.2, 0.3), 300.);
            a.stroke = width / 300.;
            p.state.annotations.push(a);
        }
        p.record(None);
        let original = p.state.annotations.clone();
        p.set_content_size(vec2(800., 400.));
        p.record(Some("resize"));
        for (a, width) in p.state.annotations.iter().zip([3., 12.]) {
            assert!((a.stroke * 800. - width).abs() < 0.0001);
        }
        assert_eq!(p.state.annotations[0].font_size, original[0].font_size);
        assert_eq!(p.state.annotations[0].radius, original[0].radius);
        p.undo(false);
        assert_eq!(p.size.x, 300.);
        assert_eq!(p.state.annotations[0].stroke, original[0].stroke);
        p.set_content_size(p.size);
        p.undo(true);
        assert_eq!(p.size.x, 800.);
        assert!((p.state.annotations[0].stroke * 800. - 3.).abs() < 0.0001);
    }
    #[test]
    fn text_selection_replace_handles_unicode_and_cursor() {
        use egui::TextBuffer;
        let ctx = egui::Context::default();
        let mut p = Pin::new(1, "A中😀B".into(), None);
        p.select_text(&ctx, 1..3);
        assert_eq!(p.state.text.char_range(p.text_selection(&ctx)), "中😀");
        p.replace_text_selection(&ctx, "好");
        assert_eq!(p.state.text, "A好B");
        assert_eq!(p.text_selection(&ctx), 2..2);
        p.replace_text_selection(&ctx, "插入");
        assert_eq!(p.state.text, "A好插入B");
        assert_eq!(p.text_selection(&ctx), 4..4);
        p.select_text(&ctx, 0..p.state.text.chars().count());
        p.replace_text_selection(&ctx, "");
        assert_eq!(p.state.text, "");
    }
    #[test]
    fn restore_uses_original_saved_size_limits_and_rounding() {
        let mut manager = PinManager::new();
        manager.create_text("x".into());
        let mut record = manager.pins[0].history_record();
        record.width = 12.;
        record.height = 3000.;
        let mut restored = PinManager::new();
        restored.restore(record.clone(), None).unwrap();
        assert_eq!(restored.pins[0].size, vec2(300., 1400.));
        record.history_id.push('2');
        record.width = 500.6;
        record.height = 100.2;
        restored.restore(record, None).unwrap();
        assert_eq!(restored.pins[1].size, vec2(501., 100.));
    }
    #[test]
    fn text_effect_reset_immediately_uses_current_global_defaults() {
        let mut manager = PinManager::new();
        manager.create_text("x".into());
        manager.configure(true, 9., 0.46);
        let p = &mut manager.pins[0];
        p.state.blur = 48.;
        p.state.opacity = 0.99;
        p.state.background = [0, 0, 0];
        p.reset_text_effects();
        assert_eq!(p.state.background, [248, 250, 255]);
        assert_eq!(p.state.blur, 9.);
        assert_eq!(p.state.opacity, 0.46);
        assert!(p.state.auto_background && p.state.auto_blur && p.state.auto_opacity);
    }
}
