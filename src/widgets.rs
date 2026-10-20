//! Detached native setting menus. Selection is applied by the owning settings transaction.
use eframe::egui::*;
use serde_json::Value;
use std::sync::{Arc, Mutex};

struct Tooltip {
    id: Id,
    text: String,
    anchor: Rect,
    since: f64,
    seen: u64,
}
impl Default for Tooltip {
    fn default() -> Self {
        Self {
            id: Id::NULL,
            text: String::new(),
            anchor: Rect::NOTHING,
            since: 0.,
            seen: 0,
        }
    }
}
fn tooltip_state(ctx: &Context) -> Arc<Mutex<Tooltip>> {
    ctx.data_mut(|data| {
        data.get_temp_mut_or_default::<Arc<Mutex<Tooltip>>>(Id::new("native-tooltip-state"))
            .clone()
    })
}
pub trait NativeTooltip {
    fn native_tooltip(self, text: &str) -> Self;
}
impl NativeTooltip for Response {
    fn native_tooltip(self, text: &str) -> Self {
        if self.hovered() || self.has_focus() {
            if let Ok(mut tip) = tooltip_state(&self.ctx).lock() {
                if tip.id != self.id {
                    tip.since = self.ctx.input(|input| input.time);
                    tip.id = self.id;
                }
                tip.text = text.into();
                tip.anchor = self.rect;
                tip.seen = self.ctx.cumulative_pass_nr();
            }
        }
        self
    }
}
pub fn show_tooltip(ctx: &Context, owner: isize, topmost: bool, visible: bool) {
    let shared = tooltip_state(ctx);
    let (text, anchor, since, seen) = {
        let Ok(tip) = shared.lock() else { return };
        (tip.text.clone(), tip.anchor, tip.since, tip.seen)
    };
    let now = ctx.input(|input| input.time);
    let interrupted = ctx.input(|input| {
        input.pointer.any_pressed()
            || input
                .events
                .iter()
                .any(|event| matches!(event, Event::Key { pressed: true, .. }))
            || input.raw_scroll_delta != Vec2::ZERO
    });
    if interrupted {
        if let Ok(mut tip) = shared.lock() {
            tip.since = now;
        }
    }
    if seen != ctx.cumulative_pass_nr() {
        if let Ok(mut tip) = shared.lock() {
            tip.id = Id::NULL;
        }
    }
    if !visible
        || is_open(ctx)
        || seen != ctx.cumulative_pass_nr()
        || interrupted
        || text.is_empty()
    {
        return;
    }
    if now - since < 0.35 {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(
            (0.35 - (now - since)).max(0.001),
        ));
        return;
    }
    let dark = ctx.style().visuals.dark_mode;
    let color = if dark {
        Color32::from_rgb(236, 241, 252)
    } else {
        Color32::from_rgb(34, 44, 61)
    };
    let width = (text
        .chars()
        .map(|ch| if ch as u32 > 255 { 12. } else { 7. })
        .sum::<f32>()
        + 24.)
        .clamp(70., 300.);
    let height =
        ((text.encode_utf16().count() as f32 * 12. / (width - 24.)).ceil() * 18. + 20.).min(150.);
    let size = vec2(width, height);
    let mut job = eframe::egui::text::LayoutJob::simple(
        text.clone(),
        FontId::new(12., FontFamily::Name("PinText".into())),
        color,
        width - 28.,
    );
    for section in &mut job.sections {
        section.format.line_height = Some(18.);
    }
    let galley = ctx.fonts(|fonts| fonts.layout_job(job));
    let origin = ctx
        .input(|input| input.viewport().inner_rect.or(input.viewport().outer_rect))
        .map(|rect| rect.min)
        .unwrap_or_default();
    let position = origin + pos2(anchor.center().x - size.x / 2., anchor.bottom() + 7.).to_vec2();
    ctx.show_viewport_immediate(
        ViewportId::from_hash_of("native-hover-tooltip"),
        ViewportBuilder::default()
            .with_title("QuickTool Rust · Tooltip")
            .with_position(position)
            .with_inner_size(size)
            .with_decorations(false)
            .with_resizable(false)
            .with_transparent(true)
            .with_active(false)
            .with_mouse_passthrough(true)
            .with_taskbar(false)
            .with_window_level(if topmost {
                WindowLevel::AlwaysOnTop
            } else {
                WindowLevel::Normal
            }),
        |tooltip_ctx, _| {
            let painter = tooltip_ctx.layer_painter(LayerId::background());
            painter.rect(
                Rect::from_min_max(pos2(4., 4.), pos2(size.x - 4., size.y - 4.)),
                CornerRadius::same(7),
                if dark {
                    Color32::from_rgb(37, 42, 53)
                } else {
                    Color32::from_rgb(248, 250, 255)
                },
                Stroke::new(
                    1.,
                    if dark {
                        Color32::from_rgb(75, 85, 107)
                    } else {
                        Color32::from_rgb(194, 201, 217)
                    },
                ),
                StrokeKind::Inside,
            );
            painter.galley(pos2(14., 11.), galley.clone(), color);
        },
    );
    #[cfg(windows)]
    if owner != 0 {
        if let Err(error) = quicktool_rust::platform::configure_tooltip(
            "QuickTool Rust · Tooltip",
            owner,
            true,
            topmost,
        ) {
            eprintln!("tooltip native style: {error}");
        }
    }
}

struct Menu {
    key: String,
    options: Vec<(String, String)>,
    value: String,
    chosen: Option<(String, String)>,
    anchor: Rect,
    open: bool,
    highlighted: usize,
    opened_frame: u64,
    qa_key: Option<String>,
    qa_path: Option<std::path::PathBuf>,
    qa_started: bool,
    qa_frames: u32,
    qa_sent: bool,
}
impl Default for Menu {
    fn default() -> Self {
        Self {
            key: String::new(),
            options: vec![],
            value: String::new(),
            chosen: None,
            anchor: Rect::NOTHING,
            open: false,
            highlighted: 0,
            opened_frame: 0,
            qa_key: std::env::args()
                .skip_while(|arg| arg != "--select-menu")
                .nth(1),
            qa_path: std::env::args()
                .skip_while(|arg| arg != "--select-screenshot")
                .nth(1)
                .map(std::path::PathBuf::from),
            qa_started: false,
            qa_frames: 0,
            qa_sent: false,
        }
    }
}
fn state(ctx: &Context) -> Arc<Mutex<Menu>> {
    ctx.data_mut(|data| {
        data.get_temp_mut_or_default::<Arc<Mutex<Menu>>>(Id::new("native-setting-menu"))
            .clone()
    })
}
pub fn is_open(ctx: &Context) -> bool {
    state(ctx).lock().is_ok_and(|menu| menu.open)
}
pub fn close(ctx: &Context) {
    if let Ok(mut menu) = state(ctx).lock() {
        menu.open = false;
    }
}
pub fn select(ui: &mut Ui, draft: &mut Value, key: &str, options: &[(&str, &str)]) {
    let shared = state(ui.ctx());
    if let Ok(mut menu) = shared.lock() {
        if menu
            .chosen
            .as_ref()
            .is_some_and(|(chosen_key, _)| chosen_key == key)
        {
            if let Some((_, value)) = menu.chosen.take() {
                draft[key] = value.into();
            }
        }
    }
    let value = draft[key].as_str().unwrap_or(options[0].0);
    let label = options
        .iter()
        .find(|(option, _)| *option == value)
        .map(|(_, label)| *label)
        .unwrap_or(value);
    let (rect, response) = ui.allocate_exact_size(vec2(170., 36.), Sense::click());
    let dark = ui.visuals().dark_mode;
    let color = if dark {
        Color32::from_rgb(235, 239, 247)
    } else {
        Color32::from_rgb(39, 47, 62)
    };
    ui.painter().rect(
        rect,
        CornerRadius::same(10),
        if response.hovered() {
            if dark {
                Color32::from_rgb(32, 37, 48)
            } else {
                Color32::from_rgb(237, 242, 252)
            }
        } else {
            if dark {
                Color32::from_rgb(15, 19, 27)
            } else {
                Color32::from_rgb(249, 251, 255)
            }
        },
        Stroke::new(
            1.,
            if dark {
                Color32::from_rgb(42, 47, 59)
            } else {
                Color32::from_rgb(213, 220, 232)
            },
        ),
        StrokeKind::Inside,
    );
    ui.painter().text(
        rect.left_center() + vec2(12., 0.),
        Align2::LEFT_CENTER,
        label,
        FontId::new(12., FontFamily::Name("UiMedium".into())),
        color,
    );
    let center = rect.right_center() + vec2(-18., 0.);
    ui.painter().add(Shape::line(
        vec![
            center + vec2(-3., -1.5),
            center + vec2(0., 1.5),
            center + vec2(3., -1.5),
        ],
        Stroke::new(1.2, color.gamma_multiply(0.6)),
    ));
    let qa_requested = shared
        .lock()
        .is_ok_and(|menu| !menu.qa_started && menu.qa_key.as_deref() == Some(key));
    if response.clicked() || qa_requested {
        if let Ok(mut menu) = shared.lock() {
            let was_open = menu.open && menu.key == key;
            menu.key = key.into();
            menu.options = options
                .iter()
                .map(|(key, label)| (key.to_string(), label.to_string()))
                .collect();
            menu.value = value.into();
            menu.highlighted = options
                .iter()
                .position(|(option, _)| *option == value)
                .unwrap_or(0);
            menu.anchor = rect;
            menu.open = !was_open;
            menu.opened_frame = ui.ctx().cumulative_pass_nr();
            if qa_requested {
                menu.qa_started = true;
            }
        }
    }
}
pub fn context(
    ui: &Ui,
    response: &Response,
    key: &str,
    options: &[(&str, &str, bool)],
) -> Option<String> {
    let shared = state(ui.ctx());
    let mut chosen = None;
    if let Ok(mut menu) = shared.lock() {
        if menu
            .chosen
            .as_ref()
            .is_some_and(|(target, _)| target == key)
        {
            chosen = menu.chosen.take().map(|(_, action)| action);
        }
        if response.secondary_clicked() {
            menu.key = key.into();
            menu.options = options
                .iter()
                .map(|(id, label, enabled)| {
                    (
                        if *enabled {
                            id.to_string()
                        } else {
                            format!("!{id}")
                        },
                        label.to_string(),
                    )
                })
                .collect();
            menu.value.clear();
            let position = response
                .interact_pointer_pos()
                .unwrap_or(response.rect.left_bottom());
            menu.anchor = Rect::from_min_size(position, vec2(200., 0.));
            menu.open = true;
            menu.highlighted = 0;
            menu.opened_frame = ui.ctx().cumulative_pass_nr();
        }
    }
    chosen
}
pub fn show(ctx: &Context, owner: isize, topmost: bool) {
    let shared = state(ctx);
    let (
        key,
        options,
        value,
        anchor,
        mut open,
        mut highlighted,
        opened_frame,
        qa_path,
        qa_frames,
        qa_sent,
    ) = {
        let Ok(mut menu) = shared.lock() else { return };
        if menu.qa_started {
            menu.qa_frames += 1;
        }
        (
            menu.key.clone(),
            menu.options.clone(),
            menu.value.clone(),
            menu.anchor,
            menu.open,
            menu.highlighted,
            menu.opened_frame,
            menu.qa_path.clone(),
            menu.qa_frames,
            menu.qa_sent,
        )
    };
    if !open {
        return;
    }
    let current_pass = ctx.cumulative_pass_nr();
    if ctx.input(|input| {
        input.key_pressed(Key::Escape)
            || (current_pass > opened_frame
                && input.pointer.any_pressed()
                && input
                    .pointer
                    .interact_pos()
                    .is_some_and(|point| !anchor.contains(point)))
    }) {
        if let Ok(mut menu) = shared.lock() {
            menu.open = false;
        }
        open = false;
    }
    let origin = ctx
        .input(|input| input.viewport().inner_rect.or(input.viewport().outer_rect))
        .map(|rect| rect.min)
        .unwrap_or_default();
    let height = options
        .iter()
        .map(|(key, _)| if key.starts_with("!sep") { 8. } else { 32. })
        .sum::<f32>()
        + 12.;
    let size = vec2(anchor.width(), height);
    let position = origin + anchor.left_bottom().to_vec2() + vec2(0., 5.);
    let dark = ctx.style().visuals.dark_mode;
    let mut selected = None;
    if ctx.input(|input| input.key_pressed(Key::ArrowDown)) {
        highlighted = (highlighted + 1).min(options.len().saturating_sub(1));
        while highlighted + 1 < options.len() && options[highlighted].0.starts_with('!') {
            highlighted += 1;
        }
    }
    if ctx.input(|input| input.key_pressed(Key::ArrowUp)) {
        highlighted = highlighted.saturating_sub(1);
        while highlighted > 0 && options[highlighted].0.starts_with('!') {
            highlighted -= 1;
        }
    }
    if ctx.input(|input| input.key_pressed(Key::Enter)) && open {
        selected = options
            .get(highlighted)
            .filter(|(option, _)| !option.starts_with('!'))
            .map(|(option, _)| option.clone());
    }
    ctx.show_viewport_immediate(
        ViewportId::from_hash_of("settings-native-select"),
        ViewportBuilder::default()
            .with_title("QuickTool Rust · Select")
            .with_position(position)
            .with_inner_size(size)
            .with_decorations(false)
            .with_resizable(false)
            .with_transparent(true)
            .with_active(false)
            .with_taskbar(false)
            .with_visible(open)
            .with_window_level(if topmost {
                WindowLevel::AlwaysOnTop
            } else {
                WindowLevel::Normal
            }),
        |menu_ctx, _| {
            menu_ctx.set_style((*ctx.style()).clone());
            CentralPanel::default()
                .frame(
                    Frame::new()
                        .fill(if dark {
                            Color32::from_rgb(22, 27, 37)
                        } else {
                            Color32::from_rgb(249, 251, 255)
                        })
                        .corner_radius(10)
                        .stroke(Stroke::new(
                            1.,
                            if dark {
                                Color32::from_rgb(54, 62, 81)
                            } else {
                                Color32::from_rgb(210, 220, 237)
                            },
                        ))
                        .inner_margin(5),
                )
                .show(menu_ctx, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.;
                    for (index, (option, label)) in options.iter().enumerate() {
                        if option.starts_with("!sep") {
                            ui.add_space(3.);
                            ui.separator();
                            ui.add_space(3.);
                            continue;
                        }
                        let enabled = !option.starts_with('!');
                        let (rect, response) = ui.allocate_exact_size(
                            vec2(size.x - 10., 32.),
                            if enabled {
                                Sense::click()
                            } else {
                                Sense::hover()
                            },
                        );
                        if response.hovered() || highlighted == index {
                            ui.painter().rect_filled(
                                rect,
                                CornerRadius::same(6),
                                if dark {
                                    Color32::from_rgb(37, 45, 63)
                                } else {
                                    Color32::from_rgb(227, 235, 252)
                                },
                            );
                        }
                        let (label, hint) = label.split_once('\t').unwrap_or((label, ""));
                        let color = if enabled {
                            ui.visuals().text_color()
                        } else {
                            ui.visuals().text_color().gamma_multiply(0.4)
                        };
                        ui.painter().text(
                            rect.left_center() + vec2(10., 0.),
                            Align2::LEFT_CENTER,
                            label,
                            FontId::new(12., FontFamily::Name("PinText".into())),
                            color,
                        );
                        if !hint.is_empty() {
                            ui.painter().text(
                                rect.right_center() - vec2(8., 0.),
                                Align2::RIGHT_CENTER,
                                hint,
                                FontId::proportional(10.5),
                                color.gamma_multiply(0.55),
                            );
                        }
                        if *option == value {
                            ui.painter().text(
                                rect.right_center() - vec2(10., 0.),
                                Align2::RIGHT_CENTER,
                                "✓",
                                FontId::proportional(13.),
                                Color32::from_rgb(155, 188, 255),
                            );
                        }
                        if response.clicked() {
                            selected = Some(option.clone());
                        }
                        if response.hovered()
                            && menu_ctx.input(|input| input.pointer.delta() != Vec2::ZERO)
                        {
                            highlighted = index;
                        }
                    }
                });
            if menu_ctx.input(|input| input.key_pressed(Key::Escape)) {
                open = false;
            }
            if let Some(path) = &qa_path {
                ctx.request_repaint_after(std::time::Duration::from_millis(16));
                menu_ctx.request_repaint_after(std::time::Duration::from_millis(16));
                if qa_frames >= 10 && !qa_sent {
                    menu_ctx.send_viewport_cmd(ViewportCommand::Screenshot(Default::default()));
                }
                let screenshot = menu_ctx.input(|input| {
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
                        .flat_map(|pixel| pixel.to_srgba_unmultiplied())
                        .collect::<Vec<_>>();
                    if let Some(parent) = path.parent() {
                        if let Err(error) = std::fs::create_dir_all(parent) {
                            eprintln!("select screenshot directory: {error}");
                        }
                    }
                    if let Err(error) = image::save_buffer(
                        path,
                        &bytes,
                        image.width() as u32,
                        image.height() as u32,
                        image::ColorType::Rgba8,
                    ) {
                        eprintln!("select screenshot: {error}");
                    }
                    ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Close);
                }
                if qa_frames > 600 {
                    eprintln!("select screenshot timed out: {}", path.display());
                    ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Close);
                }
            }
        },
    );
    if let Ok(mut menu) = shared.lock() {
        menu.highlighted = highlighted;
        if qa_path.is_some() && qa_frames >= 10 {
            menu.qa_sent = true;
        }
        if let Some(value) = selected {
            menu.chosen = Some((key, value));
            menu.open = false;
            ctx.request_repaint();
        } else {
            menu.open = open;
        }
    }
    #[cfg(windows)]
    if owner != 0 {
        if let Err(error) = quicktool_rust::platform::configure_popup(
            "QuickTool Rust · Select",
            owner,
            open,
            topmost,
        ) {
            eprintln!("setting menu native style: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context() -> Context {
        let ctx = Context::default();
        let mut fonts = FontDefinitions::default();
        fonts.families.insert(
            FontFamily::Name("UiMedium".into()),
            fonts.families[&FontFamily::Proportional].clone(),
        );
        fonts.families.insert(
            FontFamily::Name("PinText".into()),
            fonts.families[&FontFamily::Proportional].clone(),
        );
        ctx.set_fonts(fonts);
        ctx
    }
    fn frame(ctx: &Context, draft: &mut Value, key: Option<Key>) {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(812., 642.))),
            events: key
                .map(|key| {
                    vec![Event::Key {
                        key,
                        physical_key: Some(key),
                        pressed: true,
                        repeat: false,
                        modifiers: Modifiers::NONE,
                    }]
                })
                .unwrap_or_default(),
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            CentralPanel::default().show(ctx, |ui| {
                select(
                    ui,
                    draft,
                    "theme",
                    &[("system", "跟随系统"), ("light", "浅色"), ("dark", "深色")],
                )
            });
            show(ctx, 0, false);
        });
    }
    fn open(ctx: &Context) {
        let shared = state(ctx);
        let mut menu = shared.lock().unwrap();
        menu.key = "theme".into();
        menu.options = vec![
            ("system".into(), "跟随系统".into()),
            ("light".into(), "浅色".into()),
            ("dark".into(), "深色".into()),
        ];
        menu.value = "system".into();
        menu.anchor = Rect::from_min_size(pos2(100., 100.), vec2(170., 36.));
        menu.open = true;
    }
    #[test]
    fn keyboard_selection_updates_only_its_draft_field_and_clamps_at_end() {
        let ctx = context();
        let mut draft = serde_json::json!({"theme":"system","fileSearchProvider":"auto"});
        open(&ctx);
        for _ in 0..4 {
            frame(&ctx, &mut draft, Some(Key::ArrowDown));
        }
        frame(&ctx, &mut draft, Some(Key::Enter));
        frame(&ctx, &mut draft, None);
        assert_eq!(draft["theme"], "dark");
        assert_eq!(draft["fileSearchProvider"], "auto");
        assert!(!is_open(&ctx));
    }
    #[test]
    fn escape_closes_menu_without_committing_highlighted_preview() {
        let ctx = context();
        let mut draft = serde_json::json!({"theme":"system"});
        open(&ctx);
        frame(&ctx, &mut draft, Some(Key::ArrowDown));
        frame(&ctx, &mut draft, Some(Key::Escape));
        frame(&ctx, &mut draft, None);
        assert_eq!(draft["theme"], "system");
        assert!(!is_open(&ctx));
    }
}
