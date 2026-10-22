//! Pure Rust counterparts of Chromium's datetime-local and color input popovers.
use eframe::egui::{self, *};
use time::{Date, Duration, Month, OffsetDateTime};

const BG: Color32 = Color32::from_rgb(59, 59, 59);
const FG: Color32 = Color32::WHITE;
const BLUE: Color32 = Color32::from_rgb(153, 200, 255);
fn font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("NativePicker".into()))
}
fn text(ui: &Ui, rect: Rect, label: impl ToString, size: f32, color: Color32) {
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label.to_string(),
        font(size),
        color,
    );
}
fn hit(
    ui: &mut Ui,
    rect: Rect,
    key: impl std::hash::Hash,
    label: impl ToString,
    selected: bool,
) -> Response {
    let r = ui.interact(rect, ui.id().with(key), Sense::click());
    if selected {
        ui.painter().rect_filled(rect, 0, BLUE);
        ui.painter()
            .rect_stroke(rect.shrink(0.5), 0, Stroke::new(1., FG), StrokeKind::Inside);
    } else if r.hovered() {
        ui.painter()
            .rect_stroke(rect.shrink(0.5), 0, Stroke::new(1., FG), StrokeKind::Inside);
    }
    text(
        ui,
        rect,
        label,
        12.,
        if selected {
            Color32::from_rgb(35, 35, 35)
        } else {
            FG
        },
    );
    r
}
fn calendar_action(ui: &mut Ui, rect: Rect, key: &str, label: &str) -> Response {
    let response = hit(ui, rect, key, "", false);
    text(ui, rect, label, 12., BLUE);
    response
}
fn editor_event(event: &Event) -> bool {
    matches!(
        event,
        Event::Key { .. }
            | Event::Text(_)
            | Event::Paste(_)
            | Event::Copy
            | Event::Cut
            | Event::Ime(_)
    )
}
pub fn hold_input(ctx: &Context) {
    let events = ctx.input_mut(|input| {
        let mut held = Vec::new();
        input.events.retain(|event| {
            if editor_event(event) {
                held.push(event.clone());
                false
            } else {
                true
            }
        });
        held
    });
    ctx.data_mut(|data| data.insert_temp(Id::new("picker-held-input"), events));
}
/// Native child surfaces keep picker bounds independent of the result-window height.
fn popup(
    ctx: &Context,
    title: &str,
    anchor: Pos2,
    size: Vec2,
    owner: isize,
    topmost: bool,
    mut paint: impl FnMut(&Context),
) {
    let origin = ctx
        .input(|i| i.viewport().inner_rect.or(i.viewport().outer_rect))
        .map(|rect| rect.min)
        .unwrap_or_default();
    let events = ctx
        .data_mut(|data| data.remove_temp::<Vec<Event>>(Id::new("picker-held-input")))
        .unwrap_or_else(|| {
            ctx.input(|input| {
                input
                    .events
                    .iter()
                    .filter(|event| editor_event(event))
                    .cloned()
                    .collect()
            })
        });
    ctx.show_viewport_immediate(
        ViewportId::from_hash_of(title),
        ViewportBuilder::default()
            .with_title(title)
            .with_position(origin + anchor.to_vec2())
            .with_inner_size(size)
            .with_decorations(false)
            .with_resizable(false)
            .with_taskbar(false)
            .with_active(false)
            .with_transparent(true)
            .with_window_level(if topmost {
                WindowLevel::AlwaysOnTop
            } else {
                WindowLevel::Normal
            }),
        |child, _| {
            child.input_mut(|i| i.events.extend(events.clone()));
            paint(child);
            if let Some(path) = std::env::args_os()
                .skip_while(|a| a != "--picker-screenshot")
                .nth(1)
            {
                let frame = child.data_mut(|data| {
                    let count = data.get_temp_mut_or_default::<u32>(Id::new("picker-qa-frame"));
                    *count += 1;
                    *count
                });
                if frame == 10 {
                    child.send_viewport_cmd(ViewportCommand::Screenshot(UserData::default()));
                }
                let screenshot = child.input(|input| {
                    input.events.iter().find_map(|event| {
                        if let Event::Screenshot { image, .. } = event {
                            Some(image.clone())
                        } else {
                            None
                        }
                    })
                });
                if let Some(image) = screenshot {
                    let bytes: Vec<u8> = image
                        .pixels
                        .iter()
                        .flat_map(|pixel| pixel.to_array())
                        .collect();
                    if let Err(error) = image::save_buffer(
                        &path,
                        &bytes,
                        image.width() as u32,
                        image.height() as u32,
                        image::ColorType::Rgba8,
                    ) {
                        eprintln!("picker screenshot failed: {error}");
                    }
                    child.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Close);
                }
                if frame > 600 {
                    child.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Close);
                }
                child.request_repaint_after(std::time::Duration::from_millis(16));
            }
        },
    );
    #[cfg(windows)]
    if owner != 0 {
        if let Err(error) = quicktool_rust::platform::configure_popup(title, owner, true, topmost) {
            eprintln!("picker native style: {error}");
        }
    }
}
pub fn close_detached(ctx: &Context) {
    for title in [
        "QuickTool Rust · Date Picker",
        "QuickTool Rust · Color Picker",
    ] {
        let id = ViewportId::from_hash_of(title);
        if ctx.input(|input| input.raw.viewports.contains_key(&id)) {
            ctx.send_viewport_cmd_to(id, ViewportCommand::Close);
        }
    }
}
fn outside(ctx: &Context, rect: Rect) -> bool {
    ctx.input(|i| {
        i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !rect.contains(p))
    })
}
pub fn month_cells(year: i32, month: Month) -> [Date; 42] {
    let first = Date::from_calendar_date(year, month, 1).unwrap();
    // Match the Monday-first zh-CN calendar in Electron 38.
    let start = first - Duration::days(first.weekday().number_days_from_monday() as i64);
    std::array::from_fn(|i| {
        start
            .checked_add(Duration::days(i as i64))
            .unwrap_or(Date::MAX)
    })
}
#[derive(Default)]
pub struct DatePicker {
    month: Option<(i32, Month)>,
    month_menu: bool,
    time_base: [Option<u8>; 2],
    focus_column: usize,
}
impl DatePicker {
    pub fn reset(&mut self) {
        self.month = None;
        self.month_menu = false;
        self.time_base = [None, None];
        self.focus_column = 0;
    }
    pub fn show(&mut self, ctx: &Context, open: &mut bool, value: &mut String) -> bool {
        self.show_at(ctx, open, value, pos2(30.4, 42.4))
    }
    pub fn show_detached(
        &mut self,
        ctx: &Context,
        open: &mut bool,
        value: &mut String,
        owner: isize,
        topmost: bool,
    ) -> bool {
        let anchor = pos2(30.4, 42.4);
        let size = vec2(336., 283.2);
        if outside(ctx, Rect::from_min_size(anchor, size)) {
            *open = false;
            return false;
        }
        let mut changed = false;
        popup(
            ctx,
            "QuickTool Rust · Date Picker",
            anchor,
            size,
            owner,
            topmost,
            |child| {
                changed = self.show_at(child, open, value, Pos2::ZERO);
            },
        );
        changed
    }
    fn show_at(
        &mut self,
        ctx: &Context,
        open: &mut bool,
        value: &mut String,
        anchor: Pos2,
    ) -> bool {
        let mut date = quicktool_rust::dates::parse(value, "YMD")
            .map(|d| d.date)
            .unwrap_or_else(OffsetDateTime::now_utc);
        let mut changed = false;
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab)) {
            self.focus_column = (self.focus_column + 1) % 3;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::Tab)) {
            self.focus_column = (self.focus_column + 2) % 3;
        }
        let step = if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown)) {
            1
        } else if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp)) {
            -1
        } else {
            0
        };
        let horizontal = if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowRight)) {
            1
        } else if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowLeft)) {
            -1
        } else {
            0
        };
        if step != 0 || horizontal != 0 {
            if self.focus_column == 0 {
                if let Some(day) = date
                    .date()
                    .checked_add(Duration::days((step * 7 + horizontal) as i64))
                {
                    date = date.replace_date(day);
                    self.month = Some((day.year(), day.month()));
                    changed = true;
                }
            } else if let Ok(t) = time::Time::from_hms(
                if self.focus_column == 1 {
                    (date.hour() as i32 + step).rem_euclid(24) as u8
                } else {
                    date.hour()
                },
                if self.focus_column == 2 {
                    (date.minute() as i32 + step).rem_euclid(60) as u8
                } else {
                    date.minute()
                },
                0,
            ) {
                date = date.replace_time(t);
                changed = true;
                self.time_base = [None, None];
            }
        }
        if changed {
            *value = format!("{} {:02}:{:02}", date.date(), date.hour(), date.minute());
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
            *open = false;
        }
        let (mut year, mut month) = self.month.unwrap_or((date.year(), date.month()));
        let result = Area::new(Id::new("chromium-date-picker"))
            .order(Order::Tooltip)
            .fixed_pos(anchor)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(336., 283.2), Sense::hover());
                ui.set_clip_rect(rect);
                ui.painter().rect_filled(rect, 0, BG);
                let p = rect.min;
                let calendar = Rect::from_min_size(p, vec2(217., 283.2));
                ui.painter().line_segment(
                    [p + vec2(216.5, 1.), p + vec2(216.5, 282.)],
                    Stroke::new(1., Color32::from_gray(180)),
                );
                let title = Rect::from_min_size(p + vec2(10., 8.), vec2(119., 30.));
                ui.painter().text(
                    title.left_center(),
                    Align2::LEFT_CENTER,
                    format!("{year}年{:02}月", month as u8),
                    font(12.),
                    FG,
                );
                let arrow = p + vec2(82., 22.);
                ui.painter().add(Shape::convex_polygon(
                    vec![
                        arrow + vec2(-3., -2.),
                        arrow + vec2(3., -2.),
                        arrow + vec2(0., 2.),
                    ],
                    FG,
                    Stroke::NONE,
                ));
                if ui
                    .interact(title, ui.id().with("month"), Sense::click())
                    .clicked()
                {
                    self.month_menu = !self.month_menu;
                }
                for (back, x) in [(true, 145.), (false, 177.)] {
                    let r = Rect::from_min_size(p + vec2(x, 8.), vec2(24., 30.));
                    let center = r.center();
                    let direction = if back { -1. } else { 1. };
                    ui.painter().line_segment(
                        [center - vec2(0., 7.), center + vec2(0., 7.)],
                        Stroke::new(1., FG),
                    );
                    ui.painter().line_segment(
                        [center + vec2(0., direction * 7.), center + vec2(-6., 0.)],
                        Stroke::new(1., FG),
                    );
                    ui.painter().line_segment(
                        [center + vec2(0., direction * 7.), center + vec2(6., 0.)],
                        Stroke::new(1., FG),
                    );
                    if ui.interact(r, ui.id().with(back), Sense::click()).clicked() {
                        let number = (year * 12 + month as i32 - 1 + if back { -1 } else { 1 })
                            .clamp(12, 119999);
                        year = number.div_euclid(12);
                        month = Month::try_from((number.rem_euclid(12) + 1) as u8).unwrap();
                    }
                }
                if self.month_menu {
                    for i in 0..12 {
                        let r = Rect::from_min_size(
                            p + vec2(10. + (i % 3) as f32 * 67., 48. + (i / 3) as f32 * 45.),
                            vec2(63., 40.),
                        );
                        if hit(
                            ui,
                            r,
                            ("month-choice", i),
                            format!("{}月", i + 1),
                            month as usize == i + 1,
                        )
                        .clicked()
                        {
                            month = Month::try_from((i + 1) as u8).unwrap();
                            self.month_menu = false;
                        }
                    }
                } else {
                    for (i, label) in ["一", "二", "三", "四", "五", "六", "日"]
                        .iter()
                        .enumerate()
                    {
                        text(
                            ui,
                            Rect::from_min_size(
                                p + vec2(10. + i as f32 * 28., 48.),
                                vec2(28., 24.),
                            ),
                            label,
                            12.,
                            FG,
                        );
                    }
                    for (i, day) in month_cells(year, month).iter().enumerate() {
                        let r = Rect::from_min_size(
                            p + vec2(10. + (i % 7) as f32 * 28., 75. + (i / 7) as f32 * 28.),
                            vec2(28., 28.),
                        );
                        let selected = *day == date.date();
                        let response = hit(ui, r, ("day", i), day.day(), selected);
                        if response.clicked() {
                            self.focus_column = 0;
                            *value = format!("{} {:02}:{:02}", day, date.hour(), date.minute());
                            changed = true;
                        }
                    }
                }
                if calendar_action(
                    ui,
                    Rect::from_min_size(calendar.min + vec2(10., 245.), vec2(45., 30.)),
                    "clear",
                    "清除",
                )
                .clicked()
                {
                    value.clear();
                    changed = true;
                    *open = false;
                }
                if calendar_action(
                    ui,
                    Rect::from_min_size(calendar.min + vec2(165., 245.), vec2(42., 30.)),
                    "today",
                    "今天",
                )
                .clicked()
                {
                    let now =
                        OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
                    *value = format!("{} {:02}:{:02}", now.date(), date.hour(), date.minute());
                    year = now.year();
                    month = now.month();
                    changed = true;
                }
                for (hour, x, count, selected) in [
                    (true, 222., 24, date.hour()),
                    (false, 278., 60, date.minute()),
                ] {
                    let column = usize::from(!hour);
                    let initial = self.time_base[column].is_none();
                    let base = *self.time_base[column].get_or_insert(selected);
                    let r = Rect::from_min_size(p + vec2(x, 15.), vec2(54., 268.));
                    ui.scope_builder(UiBuilder::new().max_rect(r), |ui| {
                        let mut scroll = ScrollArea::vertical().id_salt(("date-time", hour));
                        if initial {
                            scroll = scroll.vertical_scroll_offset(count as f32 * 35.2);
                        }
                        scroll
                            .scroll_bar_visibility(
                                egui::scroll_area::ScrollBarVisibility::AlwaysHidden,
                            )
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing.y = 0.;
                                for index in 0..count * 3 {
                                    let number = (base + index) % count;
                                    let (r, _) =
                                        ui.allocate_exact_size(vec2(51., 35.2), Sense::hover());
                                    if hit(
                                        ui,
                                        r,
                                        (hour, index),
                                        format!("{number:02}"),
                                        number == selected,
                                    )
                                    .clicked()
                                    {
                                        self.focus_column = if hour { 1 } else { 2 };
                                        *value = format!(
                                            "{} {:02}:{:02}",
                                            date.date(),
                                            if hour { number } else { date.hour() },
                                            if hour { date.minute() } else { number }
                                        );
                                        changed = true;
                                    }
                                }
                            });
                    });
                }
            });
        if !self.month_menu {
            self.month = Some((year, month));
        }
        if ctx.input(|i| {
            i.pointer.any_pressed()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|p| !result.response.rect.contains(p))
                || i.key_pressed(Key::Escape)
        }) {
            *open = false;
        }
        changed
    }
}

fn rgb_hsv(rgb: [f32; 3]) -> [f32; 3] {
    let max = rgb.into_iter().fold(0., f32::max);
    let min = rgb.into_iter().fold(1., f32::min);
    let d = max - min;
    let h = if d == 0. {
        0.
    } else if max == rgb[0] {
        ((rgb[1] - rgb[2]) / d).rem_euclid(6.) / 6.
    } else if max == rgb[1] {
        ((rgb[2] - rgb[0]) / d + 2.) / 6.
    } else {
        ((rgb[0] - rgb[1]) / d + 4.) / 6.
    };
    [h, if max == 0. { 0. } else { d / max }, max]
}
fn hsv_rgb([h, s, v]: [f32; 3]) -> [f32; 3] {
    let h = h.rem_euclid(1.) * 6.;
    let c = v * s;
    let x = c * (1. - (h.rem_euclid(2.) - 1.).abs());
    let m = v - c;
    let rgb = match h as u8 {
        0 => [c, x, 0.],
        1 => [x, c, 0.],
        2 => [0., c, x],
        3 => [0., x, c],
        4 => [x, 0., c],
        _ => [c, 0., x],
    };
    rgb.map(|n| n + m)
}
fn color(rgb: [f32; 3]) -> Color32 {
    Color32::from_rgb(
        (rgb[0] * 255.).round() as u8,
        (rgb[1] * 255.).round() as u8,
        (rgb[2] * 255.).round() as u8,
    )
}
#[derive(Default)]
pub struct ColorPanelAction {
    pub edited: bool,
    pub copied: Option<(String, String)>,
}
pub fn color_panel_detached(
    ctx: &Context,
    open: &mut bool,
    rgba: &mut [f32; 4],
    native: &mut ColorPicker,
    copy_icon: Option<&TextureHandle>,
    owner: isize,
    topmost: bool,
) -> ColorPanelAction {
    let anchor = pos2(438., 68.);
    let size = vec2(360., 444.4);
    if outside(ctx, Rect::from_min_size(anchor, size)) {
        *open = false;
        native.open = false;
        return ColorPanelAction::default();
    }
    let mut action = ColorPanelAction::default();
    popup(
        ctx,
        "QuickTool Rust · Color Picker",
        anchor,
        size,
        owner,
        topmost,
        |child| {
            action = color_panel_at(child, open, rgba, native, copy_icon, Pos2::ZERO);
        },
    );
    action
}
pub fn color_panel(
    ctx: &Context,
    open: &mut bool,
    rgba: &mut [f32; 4],
    native: &mut ColorPicker,
    copy_icon: Option<&TextureHandle>,
) -> ColorPanelAction {
    color_panel_at(ctx, open, rgba, native, copy_icon, pos2(438., 68.))
}
fn color_panel_at(
    ctx: &Context,
    open: &mut bool,
    rgba: &mut [f32; 4],
    native: &mut ColorPicker,
    copy_icon: Option<&TextureHandle>,
    anchor: Pos2,
) -> ColorPanelAction {
    let mut action = ColorPanelAction::default();
    let weak = if ctx.style().visuals.dark_mode {
        Color32::from_rgb(123, 131, 149)
    } else {
        Color32::from_rgb(110, 120, 139)
    };
    let strong = if ctx.style().visuals.dark_mode {
        Color32::from_rgb(228, 233, 245)
    } else {
        Color32::from_rgb(37, 45, 62)
    };
    let formats = quicktool_rust::colors::formats(quicktool_rust::colors::Color::new(
        rgba[0] as f64,
        rgba[1] as f64,
        rgba[2] as f64,
        rgba[3] as f64,
    ));
    Area::new(Id::new("color-format-panel"))
        .order(Order::Foreground)
        .fixed_pos(anchor)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(360., 444.4), Sense::hover());
            let p = rect.min;
            ui.painter().rect_filled(
                rect,
                16,
                if ui.visuals().dark_mode {
                    Color32::from_rgb(17, 21, 30)
                } else {
                    Color32::from_rgb(248, 250, 255)
                },
            );
            ui.painter().rect_stroke(
                rect,
                16,
                Stroke::new(0.8, Color32::from_white_alpha(30)),
                StrokeKind::Inside,
            );
            let swatch = color([rgba[0], rgba[1], rgba[2]]);
            let preview = Rect::from_min_size(p + vec2(12.8, 11.8), vec2(25., 25.));
            ui.painter().rect_filled(preview, 8, swatch);
            ui.painter().rect_stroke(
                preview,
                8,
                Stroke::new(0.8, Color32::from_white_alpha(45)),
                StrokeKind::Inside,
            );
            ui.painter().text(
                p + vec2(46.8, 24.8),
                Align2::LEFT_CENTER,
                "Color Picker",
                FontId::new(12.5, FontFamily::Name("UiStrong".into())),
                strong,
            );
            let close = Rect::from_min_size(p + vec2(317.2, 7.8), vec2(34., 34.));
            let response = ui.interact(close, ui.id().with("close"), Sense::click());
            ui.painter().rect_stroke(
                close,
                10,
                Stroke::new(0.8, Color32::from_white_alpha(30)),
                StrokeKind::Inside,
            );
            for direction in [-1., 1.] {
                ui.painter().line_segment(
                    [
                        close.center() + vec2(-4., -4. * direction),
                        close.center() + vec2(4., 4. * direction),
                    ],
                    Stroke::new(1.2, strong),
                );
            }
            if response.clicked() {
                *open = false;
                native.open = false;
            }
            for y in [48.8, 113.6] {
                ui.painter().line_segment(
                    [p + vec2(0.8, y), p + vec2(359.2, y)],
                    Stroke::new(0.8, Color32::from_white_alpha(15)),
                );
            }
            let color_button = Rect::from_min_size(p + vec2(11.8, 63.8), vec2(38., 34.));
            ui.painter()
                .rect_filled(color_button, 9, Color32::from_white_alpha(10));
            ui.painter().rect_stroke(
                color_button,
                9,
                Stroke::new(0.8, Color32::from_white_alpha(30)),
                StrokeKind::Inside,
            );
            ui.painter()
                .rect_filled(color_button.shrink2(vec2(5., 7.)), 0, swatch);
            if ui
                .interact(color_button, ui.id().with("native-color"), Sense::click())
                .clicked()
            {
                native.open = !native.open;
            }
            ui.painter().text(
                p + vec2(56.8, 80.8),
                Align2::LEFT_CENTER,
                "颜色",
                FontId::proportional(10.5),
                weak,
            );
            ui.painter().text(
                p + vec2(101.8, 80.8),
                Align2::LEFT_CENTER,
                "Alpha",
                FontId::proportional(10.5),
                weak,
            );
            let track = Rect::from_min_size(p + vec2(140.8, 77.8), vec2(158.4, 6.));
            let response = ui.interact(
                track.expand2(vec2(0., 10.)),
                ui.id().with("alpha"),
                Sense::click_and_drag(),
            );
            ui.painter()
                .rect_filled(track, 3, Color32::from_rgb(153, 188, 255));
            ui.painter().circle_filled(
                pos2(track.left() + rgba[3] * track.width(), track.center().y),
                7.5,
                Color32::from_rgb(153, 188, 255),
            );
            if (response.clicked() || response.dragged())
                && response.interact_pointer_pos().is_some()
            {
                rgba[3] = ((response.interact_pointer_pos().unwrap().x - track.left())
                    / track.width())
                .clamp(0., 1.);
                action.edited = true;
            }
            ui.painter().text(
                p + vec2(348.2, 80.8),
                Align2::RIGHT_CENTER,
                format!("{}%", (rgba[3] * 100.).round() as u32),
                FontId::monospace(10.5),
                strong,
            );
            ui.scope_builder(
                UiBuilder::new()
                    .max_rect(Rect::from_min_size(p + vec2(6.8, 119.6), vec2(346.4, 318.))),
                |ui| {
                    ScrollArea::vertical()
                        .id_salt("color-formats")
                        .max_height(318.)
                        .scroll_bar_visibility(
                            egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                        )
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 0.;
                            for format in formats {
                                let (r, response) = ui.allocate_exact_size(
                                    vec2(ui.available_width(), 34.),
                                    Sense::click(),
                                );
                                if response.hovered() {
                                    ui.painter()
                                        .rect_filled(r, 9, Color32::from_white_alpha(12));
                                }
                                ui.painter().text(
                                    r.left_center() + vec2(8., 0.),
                                    Align2::LEFT_CENTER,
                                    &format.title,
                                    FontId::proportional(10.5),
                                    weak,
                                );
                                let value = Rect::from_min_max(
                                    r.min + vec2(121., 0.),
                                    r.max - vec2(32., 0.),
                                );
                                ui.painter().with_clip_rect(value).text(
                                    value.left_center(),
                                    Align2::LEFT_CENTER,
                                    &format.value,
                                    FontId::monospace(10.5),
                                    strong,
                                );
                                if let Some(icon) = copy_icon {
                                    ui.painter().image(
                                        icon.id(),
                                        Rect::from_center_size(
                                            r.right_center() - vec2(16., 0.),
                                            vec2(14., 14.),
                                        ),
                                        Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
                                        weak,
                                    );
                                }
                                if response.clicked() {
                                    action.copied = Some((format.title, format.value));
                                }
                            }
                        });
                },
            );
        });
    if native.show(ctx, anchor + vec2(11., 96.8), rgba) {
        action.edited = true;
    }
    action
}
#[derive(Default)]
pub struct ColorPicker {
    pub open: bool,
    pub eyedrop_requested: bool,
    mode: u8,
    hue: Option<f32>,
    hex_buffer: Option<String>,
}
impl ColorPicker {
    pub fn show(&mut self, ctx: &Context, anchor: Pos2, rgba: &mut [f32; 4]) -> bool {
        if !self.open {
            return false;
        }
        let mut hsv = rgb_hsv([rgba[0], rgba[1], rgba[2]]);
        if hsv[1] == 0. {
            hsv[0] = self.hue.unwrap_or(0.);
        }
        let mut changed = false;
        let result = Area::new(Id::new("chromium-color-picker"))
            .order(Order::Tooltip)
            .fixed_pos(anchor)
            .show(ctx, |ui| {
                ui.style_mut().override_font_id = Some(font(14.));
                ui.visuals_mut().override_text_color = Some(FG);
                let widgets = &mut ui.visuals_mut().widgets;
                for widget in [
                    &mut widgets.inactive,
                    &mut widgets.hovered,
                    &mut widgets.active,
                ] {
                    widget.bg_fill = BG;
                    widget.weak_bg_fill = BG;
                    widget.bg_stroke = Stroke::new(1., Color32::from_gray(128));
                    widget.corner_radius = CornerRadius::same(1);
                }
                let (rect, _) = ui.allocate_exact_size(vec2(234., 252.), Sense::hover());
                let p = rect.min;
                ui.painter().rect_filled(rect, 0, BG);
                ui.painter().rect_stroke(
                    rect,
                    0,
                    Stroke::new(1., Color32::from_gray(130)),
                    StrokeKind::Inside,
                );
                let sv = Rect::from_min_size(p + vec2(1., 1.), vec2(232., 137.));
                let mut mesh = egui::Mesh::default();
                for y in 0..=24 {
                    for x in 0..=24 {
                        let s = x as f32 / 24.;
                        let v = 1. - y as f32 / 24.;
                        mesh.colored_vertex(
                            pos2(
                                sv.left() + s * sv.width(),
                                sv.top() + (1. - v) * sv.height(),
                            ),
                            color(hsv_rgb([hsv[0], s, v])),
                        );
                    }
                }
                for y in 0..24 {
                    for x in 0..24 {
                        let i = (y * 25 + x) as u32;
                        mesh.add_triangle(i, i + 1, i + 25);
                        mesh.add_triangle(i + 1, i + 26, i + 25);
                    }
                }
                ui.painter().add(mesh);
                let r = ui.interact(sv, ui.id().with("sv"), Sense::click_and_drag());
                if (r.dragged() || r.clicked()) && r.interact_pointer_pos().is_some() {
                    let at = r.interact_pointer_pos().unwrap();
                    hsv[1] = ((at.x - sv.left()) / sv.width()).clamp(0., 1.);
                    hsv[2] = (1. - (at.y - sv.top()) / sv.height()).clamp(0., 1.);
                    changed = true;
                }
                ui.painter().circle_stroke(
                    pos2(
                        sv.left() + hsv[1] * sv.width(),
                        sv.bottom() - hsv[2] * sv.height(),
                    ),
                    6.5,
                    Stroke::new(3., FG),
                );
                let hue_rect = Rect::from_min_size(p + vec2(88., 155.), vec2(130., 12.));
                let mut hue_mesh = egui::Mesh::default();
                for i in 0..=6 {
                    let x = hue_rect.left() + i as f32 / 6. * hue_rect.width();
                    let tint = color(hsv_rgb([1. - i as f32 / 6., 1., 1.]));
                    hue_mesh.colored_vertex(pos2(x, hue_rect.top()), tint);
                    hue_mesh.colored_vertex(pos2(x, hue_rect.bottom()), tint);
                }
                for i in 0..6 {
                    let n = i * 2;
                    hue_mesh.add_triangle(n, n + 2, n + 1);
                    hue_mesh.add_triangle(n + 2, n + 3, n + 1);
                }
                ui.painter().add(hue_mesh);
                let r = ui.interact(
                    hue_rect.expand(5.),
                    ui.id().with("hue"),
                    Sense::click_and_drag(),
                );
                if (r.dragged() || r.clicked()) && r.interact_pointer_pos().is_some() {
                    hsv[0] = (1.
                        - (r.interact_pointer_pos().unwrap().x - hue_rect.left())
                            / hue_rect.width())
                    .clamp(0., 0.999999);
                    changed = true;
                }
                ui.painter().circle_stroke(
                    pos2(
                        hue_rect.left() + (1. - hsv[0]) * hue_rect.width(),
                        hue_rect.center().y,
                    ),
                    7.,
                    Stroke::new(1.5, FG),
                );
                ui.painter()
                    .circle_filled(p + vec2(59., 162.), 16., color(hsv_rgb(hsv)));
                let eyedrop = Rect::from_min_size(p + vec2(8., 148.), vec2(30., 28.));
                let center = eyedrop.center();
                ui.painter().line_segment(
                    [center + vec2(-5., 5.), center + vec2(5., -5.)],
                    Stroke::new(3., FG),
                );
                ui.painter().line_segment(
                    [center + vec2(-2., -5.), center + vec2(5., 2.)],
                    Stroke::new(3., FG),
                );
                ui.painter().line_segment(
                    [center + vec2(-7., 7.), center + vec2(-5., 5.)],
                    Stroke::new(1., FG),
                );
                if ui
                    .interact(eyedrop, ui.id().with("eyedropper"), Sense::click())
                    .clicked()
                {
                    self.eyedrop_requested = true;
                }
                let rgb = hsv_rgb(hsv);
                let values = rgb.map(|v| (v * 255.).round() as i32);
                if self.mode == 2 {
                    let canonical = format!("#{:02x}{:02x}{:02x}", values[0], values[1], values[2]);
                    let mut hex = self.hex_buffer.take().unwrap_or_else(|| canonical.clone());
                    ui.scope_builder(
                        UiBuilder::new()
                            .max_rect(Rect::from_min_size(p + vec2(24., 184.), vec2(171., 32.))),
                        |ui| {
                            let response = ui.add_sized(
                                [171., 32.],
                                TextEdit::singleline(&mut hex)
                                    .font(font(14.))
                                    .horizontal_align(Align::Center),
                            );
                            if response.changed() {
                                if let Ok(n) = u32::from_str_radix(hex.trim_start_matches('#'), 16)
                                {
                                    if hex.trim_start_matches('#').len() == 6 {
                                        hsv = rgb_hsv([
                                            ((n >> 16) & 255) as f32 / 255.,
                                            ((n >> 8) & 255) as f32 / 255.,
                                            (n & 255) as f32 / 255.,
                                        ]);
                                        changed = true;
                                    }
                                }
                            }
                            if !response.has_focus() {
                                hex = canonical;
                            }
                        },
                    );
                    self.hex_buffer = Some(hex);
                    text(
                        ui,
                        Rect::from_min_size(p + vec2(24., 220.), vec2(171., 25.)),
                        "HEX",
                        11.,
                        FG,
                    );
                } else {
                    let hsl_l = hsv[2] * (1. - hsv[1] / 2.);
                    let hsl_s = if hsl_l == 0. || hsl_l == 1. {
                        0.
                    } else {
                        (hsv[2] - hsl_l) / hsl_l.min(1. - hsl_l)
                    };
                    let mut fields = if self.mode == 0 {
                        values
                    } else {
                        [
                            (hsv[0] * 360.).round() as i32,
                            (hsl_s * 100.).round() as i32,
                            (hsl_l * 100.).round() as i32,
                        ]
                    };
                    for i in 0..3 {
                        ui.scope_builder(
                            UiBuilder::new().max_rect(Rect::from_min_size(
                                p + vec2(24. + i as f32 * 59., 184.),
                                vec2(52., 32.),
                            )),
                            |ui| {
                                if ui
                                    .add_sized(
                                        [52., 32.],
                                        DragValue::new(&mut fields[i])
                                            .range(
                                                0..=if self.mode == 0 {
                                                    255
                                                } else if i == 0 {
                                                    360
                                                } else {
                                                    100
                                                },
                                            )
                                            .speed(1.),
                                    )
                                    .changed()
                                {
                                    if self.mode == 0 {
                                        hsv = rgb_hsv(fields.map(|n| n as f32 / 255.));
                                    } else {
                                        let h = fields[0] as f32 / 360.;
                                        let s = fields[1] as f32 / 100.;
                                        let l = fields[2] as f32 / 100.;
                                        let v = l + s * l.min(1. - l);
                                        hsv = [h, if v == 0. { 0. } else { 2. * (1. - l / v) }, v];
                                    }
                                    changed = true;
                                }
                            },
                        );
                        text(
                            ui,
                            Rect::from_min_size(
                                p + vec2(24. + i as f32 * 59., 220.),
                                vec2(52., 25.),
                            ),
                            if self.mode == 0 {
                                ["R", "G", "B"][i]
                            } else {
                                ["H", "S", "L"][i]
                            },
                            11.,
                            FG,
                        );
                    }
                }
                let format_rect = Rect::from_min_size(p + vec2(192., 220.), vec2(24., 25.));
                for direction in [-1., 1.] {
                    let c = format_rect.center() + vec2(0., direction * 3.);
                    ui.painter().line_segment(
                        [c + vec2(-2., direction), c - vec2(0., direction)],
                        Stroke::new(0.8, FG),
                    );
                    ui.painter().line_segment(
                        [c - vec2(0., direction), c + vec2(2., direction)],
                        Stroke::new(0.8, FG),
                    );
                }
                if hit(ui, format_rect, "format", "", false).clicked() {
                    self.mode = (self.mode + 1) % 3;
                }
            });
        self.hue = Some(hsv[0]);
        if changed {
            rgba[..3].copy_from_slice(&hsv_rgb(hsv));
        }
        if ctx.input(|i| {
            i.pointer.any_pressed()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|p| !result.response.rect.contains(p))
                || i.key_pressed(Key::Escape)
        }) {
            self.open = false;
        }
        changed
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detached_picker_receives_text_without_editing_the_parent_search() {
        let ctx = Context::default();
        let _ = ctx.run(
            RawInput {
                events: vec![
                    Event::Text("#ff0000".into()),
                    Event::PointerMoved(pos2(2., 3.)),
                    Event::Key {
                        key: Key::ArrowRight,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ctx| {
                hold_input(ctx);
                assert!(ctx.input(|input| input.events.iter().all(|event| !editor_event(event))));
                assert!(ctx.input(|input| {
                    input
                        .events
                        .iter()
                        .any(|event| matches!(event, Event::PointerMoved(_)))
                }));
                let held = ctx
                    .data_mut(|data| data.remove_temp::<Vec<Event>>(Id::new("picker-held-input")))
                    .unwrap();
                assert_eq!(held.len(), 2);
                assert!(matches!(&held[0], Event::Text(value) if value == "#ff0000"));
            },
        );
    }
    #[test]
    fn calendar_includes_adjacent_months_and_leap_day() {
        let days = month_cells(2024, Month::February);
        assert_eq!(days[0].to_string(), "2024-01-29");
        assert!(days.iter().any(|d| d.to_string() == "2024-02-29"));
        assert_eq!(days[41].to_string(), "2024-03-10");
    }
    #[test]
    fn rgb_hsv_round_trip_preserves_channel_values() {
        for rgb in [
            [0., 0., 0.],
            [1., 1., 1.],
            [124. / 255., 58. / 255., 237. / 255.],
            [1., 0., 0.3],
        ] {
            for (a, b) in rgb.into_iter().zip(hsv_rgb(rgb_hsv(rgb))) {
                assert!((a - b).abs() < 0.00001);
            }
        }
    }
    #[test]
    fn calendar_keyboard_crosses_leap_day_without_changing_time() {
        let context = Context::default();
        let mut fonts = FontDefinitions::default();
        fonts.families.insert(
            FontFamily::Name("NativePicker".into()),
            fonts.families[&FontFamily::Proportional].clone(),
        );
        context.set_fonts(fonts);
        let mut picker = DatePicker::default();
        let mut open = true;
        let mut value = "2024-02-29 23:59".to_string();
        let input = RawInput {
            events: vec![Event::Key {
                key: Key::ArrowRight,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
            ..Default::default()
        };
        let mut changed = false;
        let _ = context.run(input, |ctx| {
            changed |= picker.show(ctx, &mut open, &mut value);
        });
        assert!(changed);
        assert_eq!(value, "2024-03-01 23:59");
        assert!(open);
    }
}
