//! History panel matching the original row layout. Copy/restore transactions remain owned by QuickTool.
use super::*;
use quicktool_rust::storage::HistoryItem;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RowAction {
    Restore,
    Copy,
    TogglePin,
    Delete,
}
// Chromium composites CSS colors in sRGB; egui's GPU blend is linear.
// Resolve panel-local CSS alpha in sRGB before sending opaque colors to egui.
fn css(dark: bool, r: u8, g: u8, b: u8, a: u8) -> Color32 {
    let base = if dark {
        [16u8, 21, 30]
    } else {
        [247u8, 249, 253]
    };
    blend(Color32::from_rgb(base[0], base[1], base[2]), r, g, b, a)
}
fn blend(background: Color32, r: u8, g: u8, b: u8, a: u8) -> Color32 {
    let channel = |value: u8, background: u8| {
        ((value as u32 * a as u32 + background as u32 * (255 - a as u32) + 127) / 255) as u8
    };
    Color32::from_rgb(
        channel(r, background.r()),
        channel(g, background.g()),
        channel(b, background.b()),
    )
}
fn opacity(background: Color32, color: Color32, amount: f32) -> Color32 {
    blend(
        background,
        color.r(),
        color.g(),
        color.b(),
        (amount * 255.).round() as u8,
    )
}
fn colors(dark: bool) -> (Color32, Color32, Color32) {
    if dark {
        (
            Color32::from_rgb(246, 248, 255),
            css(dark, 235, 240, 252, 189),
            css(dark, 226, 233, 248, 122),
        )
    } else {
        (
            Color32::from_rgb(23, 34, 52),
            css(dark, 31, 42, 60, 194),
            css(dark, 46, 59, 82, 128),
        )
    }
}
fn text(
    ui: &Ui,
    at: Pos2,
    value: &str,
    size: f32,
    color: Color32,
    max_width: f32,
    style: (bool, bool),
) -> f32 {
    let (strong, mono) = style;
    if max_width <= 0. {
        return 0.;
    }
    let family = if mono {
        FontFamily::Monospace
    } else if strong {
        FontFamily::Name("UiSetting".into())
    } else {
        FontFamily::Proportional
    };
    let mut job = eframe::egui::text::LayoutJob::simple(
        value.into(),
        FontId::new(size, family),
        color,
        max_width,
    );
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.break_on_newline = false;
    let galley = ui.painter().layout_job(job);
    let width = galley.size().x;
    ui.painter().galley(at, galley, color);
    width
}
fn glyph(ui: &Ui, icons: &HashMap<String, TextureHandle>, key: &str, rect: Rect, color: Color32) {
    if let Some(texture) = icons.get(key) {
        ui.painter().image(
            texture.id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
            color,
        );
    }
}
fn button(
    ui: &mut Ui,
    icons: &HashMap<String, TextureHandle>,
    rect: Rect,
    id: Id,
    key: &str,
    label: &str,
    appearance: (f32, Color32),
) -> Response {
    let (opacity, background) = appearance;
    let response = ui
        .interact(rect, id, Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand);
    let dark = ui.visuals().dark_mode;
    let (primary, secondary, _) = colors(dark);
    let opacity = if response.hovered() || response.has_focus() {
        1.
    } else {
        opacity
    };
    let fill = if response.hovered() {
        if dark {
            blend(background, 135, 155, 255, 24)
        } else {
            blend(background, 100, 119, 220, 19)
        }
    } else if dark {
        blend(background, 18, 22, 31, 41)
    } else {
        blend(background, 255, 255, 255, 92)
    };
    let stroke = if dark {
        blend(background, 235, 241, 255, 24)
    } else {
        blend(background, 57, 75, 107, 31)
    };
    ui.painter().rect(
        rect,
        10,
        self::opacity(background, fill, opacity),
        Stroke::new(1., self::opacity(background, stroke, opacity)),
        StrokeKind::Inside,
    );
    glyph(
        ui,
        icons,
        key,
        Rect::from_center_size(rect.center(), vec2(17., 17.)),
        self::opacity(
            background,
            if response.hovered() {
                primary
            } else {
                secondary
            },
            opacity,
        ),
    );
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, label));
    response.native_tooltip(label)
}
fn timestamp(created: u64) -> String {
    let Ok(date) = time::OffsetDateTime::from_unix_timestamp_nanos(created as i128 * 1_000_000)
    else {
        return "Invalid Date".into();
    };
    let recognized = quicktool_rust::dates::recognized(date);
    let Some((date, clock)) = recognized.split_once(' ') else {
        return recognized;
    };
    let mut parts = date.split('-');
    let year = parts.next().unwrap_or_default();
    let month = parts.next().and_then(|v| v.parse::<u8>().ok());
    let day = parts.next().and_then(|v| v.parse::<u8>().ok());
    match (month, day) {
        (Some(m), Some(d)) => format!("{year}/{m}/{d} {clock}"),
        _ => recognized,
    }
}
fn collapsed(value: &str, limit: usize) -> String {
    clip_text(value, limit)
        .split([' ', '\t', '\r', '\n', '\x0c'])
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
fn summary(item: &HistoryItem) -> std::borrow::Cow<'_, str> {
    let Some(pin) = &item.pin_state else {
        return std::borrow::Cow::Borrowed(&item.value);
    };
    let mut parts = vec![
        format!("{} × {} px", pin.width, pin.height),
        format!("圆角 {} px", pin.corner_radius),
    ];
    for (name, value) in [
        ("亮度", pin.filters.brightness),
        ("对比度", pin.filters.contrast),
        ("饱和度", pin.filters.saturation),
        ("曝光", pin.filters.exposure),
        ("色相", pin.filters.hue),
    ] {
        if value != 0. {
            parts.push(format!("{name} {value}"));
        }
    }
    if pin.filters.colorize {
        parts.push(format!(
            "着色 {}° / {}%",
            pin.filters.hue.rem_euclid(360.),
            pin.filters.colorize_saturation
        ));
    }
    std::borrow::Cow::Owned(parts.join(" · "))
}
fn section(ui: &mut Ui, icons: &HashMap<String, TextureHandle>, pinned: bool) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 34.), Sense::hover());
    let mut x = rect.left() + 9.;
    if pinned {
        glyph(
            ui,
            icons,
            "mdi:star",
            Rect::from_min_size(pos2(x, rect.top() + 13.75), vec2(10.5, 10.5)),
            Color32::from_rgb(243, 200, 92),
        );
        x += 16.5;
    }
    text(
        ui,
        pos2(x, rect.top() + 11.8),
        if pinned { "收藏" } else { "历史记录" },
        10.5,
        colors(ui.visuals().dark_mode).2,
        rect.width() - 20.,
        (true, false),
    );
}
fn paint_row(
    ui: &mut Ui,
    icons: &HashMap<String, TextureHandle>,
    item: &HistoryItem,
    index: usize,
) -> Option<RowAction> {
    let thumb = icons.get(&format!("history-thumb:{}", item.id));
    let image =
        item.image_asset.is_some() || item.pin_state.as_ref().is_some_and(|p| p.kind == "image");
    let thumb_size = thumb
        .map(|t| {
            let s = t.size_vec2();
            s * (96. / s.x).min(64. / s.y).min(1.)
        })
        .unwrap_or(vec2(96., 64.));
    let main_height = if image { thumb_size.y } else { 13.9 };
    let height = (15.2 + 4. + main_height + 2. + 13.65 + 17.6).max(66.85);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return None;
    }
    let response = ui
        .interact(
            rect,
            ui.make_persistent_id(("history-row", &item.id, index)),
            Sense::click(),
        )
        .on_hover_cursor(CursorIcon::PointingHand);
    let dark = ui.visuals().dark_mode;
    let (primary, secondary, weak) = colors(dark);
    let hovered = response.hovered() || ui.rect_contains_pointer(rect);
    let background = if item.pinned || hovered {
        css(
            dark,
            if dark { 135 } else { 100 },
            if dark { 155 } else { 119 },
            if dark { 255 } else { 220 },
            if dark { 24 } else { 19 },
        )
    } else {
        css(dark, 0, 0, 0, 0)
    };
    if item.pinned || hovered {
        ui.painter().rect_filled(rect, 12, background);
    }
    // Chromium snaps a 1 CSS px border down to whole device pixels.
    let border = ui.ctx().pixels_per_point().floor() / ui.ctx().pixels_per_point();
    let icon = Rect::from_center_size(
        pos2(rect.left() + 9. + border + 16., rect.center().y),
        vec2(32., 32.),
    );
    ui.painter().rect_filled(
        icon,
        9,
        if dark {
            blend(background, 255, 255, 255, 11)
        } else {
            blend(background, 65, 83, 120, 10)
        },
    );
    let command = tools::commands().iter().find(|c| c.id == item.tool);
    glyph(
        ui,
        icons,
        command.map_or("mdi:history", |c| c.icon.as_str()),
        Rect::from_center_size(icon.center(), vec2(16., 16.)),
        weak,
    );
    let action_start = rect.right() - 9. - border - 106.;
    let content = Rect::from_min_max(
        pos2(rect.left() + 53. + border, rect.top() + 8.8),
        pos2(action_start - 10., rect.bottom() - 8.8),
    );
    let name = item.tool_name.as_deref().unwrap_or_else(|| {
        command.map_or(
            if item.tool.starts_with("system:") {
                "系统命令"
            } else {
                "操作"
            },
            |c| c.name.as_str(),
        )
    });
    let title = format!("{name} · {}", collapsed(&item.title, 200));
    let time = timestamp(item.created);
    let time_width = ui
        .painter()
        .layout_no_wrap(time.clone(), FontId::proportional(10.), weak)
        .size()
        .x;
    let title_width = text(
        ui,
        content.min,
        &title,
        11.5,
        secondary,
        (content.width() - time_width - 7.).max(0.),
        (true, false),
    );
    text(
        ui,
        content.min + vec2(title_width + 7., 1.),
        &time,
        10.,
        weak,
        time_width,
        (false, false),
    );
    if image {
        if let Some(texture) = thumb {
            Image::new(texture)
                .fit_to_exact_size(thumb_size)
                .corner_radius(7)
                .paint_at(
                    ui,
                    Rect::from_min_size(content.min + vec2(0., 19.2), thumb_size),
                );
        }
    } else {
        text(
            ui,
            content.min + vec2(0., 19.2),
            &collapsed(&item.input, 90),
            10.5,
            primary,
            content.width(),
            (false, false),
        );
    }
    text(
        ui,
        content.min + vec2(0., 19.2 + main_height + 2.),
        &format!("→ {}", collapsed(&summary(item), 150)),
        10.5,
        weak,
        content.width(),
        (false, true),
    );
    let mut action = None;
    let mut action_hit = false;
    for (i, key, label, kind) in [
        (
            0,
            if item.pinned {
                "mdi:star"
            } else {
                "mdi:star-outline"
            },
            if item.pinned {
                "取消收藏"
            } else {
                "收藏"
            },
            RowAction::TogglePin,
        ),
        (1, "mdi:content-copy", "复制结果", RowAction::Copy),
        (2, "mdi:delete-outline", "删除", RowAction::Delete),
    ] {
        let r = Rect::from_min_size(
            pos2(action_start + i as f32 * 36., rect.center().y - 17.),
            vec2(34., 34.),
        );
        action_hit |= ui.input(|input| input.pointer.interact_pos().is_some_and(|p| r.contains(p)));
        let opacity = if hovered || (i == 0 && item.pinned) {
            1.
        } else {
            0.4
        };
        if button(
            ui,
            icons,
            r,
            ui.make_persistent_id((&item.id, key)),
            key,
            label,
            (opacity, background),
        )
        .clicked()
        {
            action = Some(kind);
        }
    }
    if action.is_none() && response.clicked() && !action_hit {
        action = Some(RowAction::Restore);
    }
    action
}
impl QuickTool {
    pub(super) fn history_panel(&mut self, ui: &mut Ui, ctx: &Context) {
        let mut changed = false;
        let mut previous_history = None;
        let mut change_message = None;
        let mut remove = None;
        let mut restore = None;
        let mut restore_pin = None;
        let mut restore_image = None;
        let mut copy_pin = None;
        let mut copy_image = None;
        let mut clear = false;
        let mut close = false;
        let mut chosen = None;
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            ui.set_min_height(620.);
            let (header, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), 66.), Sense::hover());
            let dark = ui.visuals().dark_mode;
            let (primary, secondary, weak) = colors(dark);
            let icon = Rect::from_min_size(header.min + vec2(14., 16.1), vec2(34., 34.));
            ui.painter().rect_filled(
                icon,
                10,
                if dark {
                    css(dark, 255, 255, 255, 11)
                } else {
                    css(dark, 65, 83, 120, 10)
                },
            );
            glyph(
                ui,
                &self.icons,
                "mdi:history",
                Rect::from_center_size(icon.center(), vec2(18., 18.)),
                secondary,
            );
            let title = header.min + vec2(58., 15.7);
            text(
                ui,
                title,
                "历史与收藏",
                13.5,
                primary,
                header.width() - 245.,
                (true, false),
            );
            text(
                ui,
                title + vec2(0., 20.4),
                "收藏项固定保留；点击历史项恢复输入与模式",
                10.5,
                weak,
                header.width() - 245.,
                (false, false),
            );
            let exit = Rect::from_min_size(
                pos2(header.right() - 46., header.top() + 16.),
                vec2(34., 34.),
            );
            close = button(
                ui,
                &self.icons,
                exit,
                Id::new("history-close"),
                "mdi:close",
                "关闭历史记录",
                (1., css(dark, 0, 0, 0, 0)),
            )
            .clicked();
            let clear_rect = Rect::from_min_max(
                pos2(exit.left() - 109., exit.top()),
                pos2(exit.left() - 5., exit.bottom()),
            );
            let clear_response = ui
                .interact(clear_rect, Id::new("history-clear"), Sense::click())
                .on_hover_cursor(CursorIcon::PointingHand)
                .native_tooltip("清空普通历史");
            ui.painter().rect(
                clear_rect,
                10,
                if dark {
                    css(dark, 18, 22, 31, 41)
                } else {
                    css(dark, 255, 255, 255, 92)
                },
                Stroke::new(
                    1.,
                    if dark {
                        css(dark, 235, 241, 255, 24)
                    } else {
                        css(dark, 57, 75, 107, 31)
                    },
                ),
                StrokeKind::Inside,
            );
            glyph(
                ui,
                &self.icons,
                "mdi:delete-outline",
                Rect::from_min_size(clear_rect.min + vec2(10., 11.), vec2(12., 12.)),
                secondary,
            );
            text(
                ui,
                clear_rect.min + vec2(28., 9.),
                "清空普通历史",
                11.,
                secondary,
                70.,
                (false, false),
            );
            clear = clear_response.clicked();
            ui.painter().line_segment(
                [header.left_bottom(), header.right_bottom()],
                Stroke::new(
                    1.,
                    if dark {
                        css(dark, 255, 255, 255, 19)
                    } else {
                        css(dark, 49, 64, 90, 25)
                    },
                ),
            );
            Frame::new().inner_margin(7).show(ui, |ui| {
                ScrollArea::vertical()
                    .id_salt("history-list")
                    .auto_shrink([false, false])
                    .max_height(ui.available_height())
                    .show(ui, |ui| {
                        if self.store.history.is_empty() {
                            let (r, _) = ui.allocate_exact_size(
                                vec2(ui.available_width(), 122.),
                                Sense::hover(),
                            );
                            let label = "暂无历史记录";
                            let galley = ui.painter().layout_no_wrap(
                                label.into(),
                                FontId::proportional(13.),
                                weak,
                            );
                            ui.painter().galley(
                                pos2(r.center().x - galley.size().x / 2., r.top() + 52.),
                                galley,
                                weak,
                            );
                        }
                        for pinned in [true, false] {
                            let indices = self
                                .store
                                .history
                                .iter()
                                .enumerate()
                                .filter(|(_, h)| h.pinned == pinned)
                                .map(|(i, _)| i)
                                .collect::<Vec<_>>();
                            if indices.is_empty() {
                                continue;
                            }
                            section(ui, &self.icons, pinned);
                            for index in indices {
                                // Decode only thumbnails for rows about to become visible.
                                let item = &self.store.history[index];
                                let key = format!("history-thumb:{}", item.id);
                                let next = Rect::from_min_size(
                                    ui.cursor().min,
                                    vec2(ui.available_width(), 118.),
                                );
                                if ui.is_rect_visible(next) && !self.icons.contains_key(&key) {
                                    if let Some(path) = self.store.thumbnail_path(&item.id) {
                                        if let Ok(image) = image::open(path) {
                                            let rgba = image.to_rgba8();
                                            self.icons.insert(
                                                key,
                                                ctx.load_texture(
                                                    format!("history:{}", item.id),
                                                    ColorImage::from_rgba_unmultiplied(
                                                        [
                                                            rgba.width() as usize,
                                                            rgba.height() as usize,
                                                        ],
                                                        rgba.as_raw(),
                                                    ),
                                                    TextureOptions::LINEAR,
                                                ),
                                            );
                                        }
                                    }
                                }
                                if let Some(action) =
                                    paint_row(ui, &self.icons, &self.store.history[index], index)
                                {
                                    chosen = Some((index, action));
                                }
                            }
                        }
                    });
            });
        });
        if close {
            self.panel = Panel::None;
            self.focus = true;
        }
        if clear {
            let previous = self.store.history.clone();
            self.store.history.retain(|h| h.pinned);
            if let Err(error) = self.store.save_history() {
                self.store.history = previous;
                self.message(error.to_string());
            } else {
                self.message("已清空普通历史；收藏保留");
            }
        }
        if let Some((index, action)) = chosen {
            if matches!(action, RowAction::TogglePin | RowAction::Delete) {
                previous_history = Some(self.store.history.clone());
            }
            if let Some(item) = self.store.history.get_mut(index) {
                match action {
                    RowAction::Restore => {
                        if item.pin_state.is_some() {
                            restore_pin = Some(item.id.clone());
                        } else if let Some(asset) = &item.image_asset {
                            restore_image = Some((
                                asset.clone(),
                                item.mode.clone(),
                                item.image_source_asset.clone(),
                                item.image_source_mime.clone(),
                            ));
                        } else {
                            restore = Some((
                                item.input.clone(),
                                item.tool.clone(),
                                item.value.clone(),
                                item.mode.clone(),
                            ));
                        }
                    }
                    RowAction::Copy => {
                        if item.pin_state.is_some() {
                            copy_pin = Some(item.id.clone());
                        } else if item.image_asset.is_some() {
                            copy_image = Some(item.id.clone());
                        } else {
                            ctx.copy_text(item.value.clone());
                            change_message = Some("历史结果已复制");
                        }
                    }
                    RowAction::TogglePin => {
                        item.pinned = !item.pinned;
                        change_message = Some(if item.pinned {
                            "已收藏"
                        } else {
                            "已取消收藏"
                        });
                        changed = true;
                    }
                    RowAction::Delete => remove = Some(index),
                }
            }
        }
        if let Some(index) = remove {
            self.store.history.remove(index);
            change_message = Some("已删除该条历史");
            changed = true;
        }
        if changed {
            if let Err(e) = self.store.save_history() {
                if let Some(previous) = previous_history {
                    self.store.history = previous;
                }
                change_message = None;
                self.message(e.to_string());
            }
        }
        if let Some(message) = change_message {
            self.message(message);
        }
        if let Some(id) = restore_pin {
            match self.store.load_pin(&id) {
                Ok((record, source)) => {
                    if let Err(error) = self.pins.restore(record, source) {
                        self.message(error);
                    } else {
                        self.panel = Panel::None;
                        self.focus = true;
                        self.message("已恢复 Pin 与调整");
                    }
                }
                Err(error) => self.message(format!("恢复 Pin 失败：{error}")),
            }
        }
        if let Some(id) = copy_pin {
            let record = self
                .store
                .history
                .iter()
                .find(|item| item.id == id)
                .and_then(|item| item.pin_state.clone());
            if let Some(record) = record {
                if record.kind != "image" {
                    ctx.copy_text(record.text);
                    self.message("历史结果已复制");
                } else if let Some(asset) = record.asset.clone() {
                    let root = self.store.root.clone();
                    let context = ctx.clone();
                    let (sender, receiver) = std::sync::mpsc::channel();
                    self.image_copy = Some(receiver);
                    std::thread::spawn(move || {
                        let outcome = quicktool_rust::pin_storage::load_image(&root, &asset)
                            .map_err(|e| e.to_string())
                            .and_then(|source| {
                                quicktool_rust::history_image::processed_pin_image(&record, &source)
                            })
                            .and_then(|image| {
                                platform::copy_image(image.width(), image.height(), image.as_raw())
                            });
                        let _ = sender.send(outcome);
                        context.request_repaint();
                    });
                } else {
                    self.message("复制 Pin 失败：原图资源不存在");
                }
            } else {
                self.message("复制 Pin 失败：Pin 历史不存在");
            }
        }
        if let Some(id) = copy_image {
            if let Some(asset) = self
                .store
                .history
                .iter()
                .find(|item| item.id == id)
                .and_then(|item| item.image_asset.clone())
            {
                let root = self.store.root.clone();
                let context = ctx.clone();
                let (sender, receiver) = std::sync::mpsc::channel();
                self.image_copy = Some(receiver);
                std::thread::spawn(move || {
                    let outcome = quicktool_rust::pin_storage::load_image(&root, &asset)
                        .map_err(|error| error.to_string())
                        .and_then(|image| {
                            platform::copy_image(image.width(), image.height(), image.as_raw())
                        });
                    let _ = sender.send(outcome);
                    context.request_repaint();
                });
            }
        }
        if let Some((asset, mode, encoded_asset, encoded_mime)) = restore_image {
            self.mode = mode.as_ref().map(|m| m.tool_id.clone());
            self.submode = mode.as_ref().and_then(|m| m.submode.clone());
            self.panel = Panel::None;
            self.input.clear();
            self.selected = 0;
            self.attachment = None;
            self.attachment_results = None;
            let root = self.store.root.clone();
            self.load_attachment(move || {
                match (encoded_asset, encoded_mime) {
                    (Some(encoded), Some(mime)) => {
                        let source = quicktool_rust::pin_storage::load_encoded_source(
                            &root, &encoded, &mime,
                        )
                        .map_err(|e| e.to_string())?;
                        let mut attachment =
                            attachments::Attachment::from_bytes("历史图片", &source.bytes)?;
                        attachment.mime = source.mime;
                        return Ok(attachment);
                    }
                    (None, None) => {}
                    _ => return Err("图片历史原始编码记录不完整".into()),
                }
                let rgba = quicktool_rust::pin_storage::load_image(&root, &asset)
                    .map_err(|e| e.to_string())?;
                let source = quicktool_rust::pin_storage::asset_path(&root, &asset, false)
                    .map_err(|e| e.to_string())?;
                let bytes = std::fs::read(source).map_err(|e| e.to_string())?;
                let thumbnail = image::imageops::thumbnail(&rgba, 128, 96);
                Ok(attachments::Attachment {
                    name: "历史图片".into(),
                    mime: "image/png".into(),
                    bytes: bytes.into(),
                    rgba: std::sync::Arc::new(rgba),
                    thumbnail: std::sync::Arc::new(thumbnail),
                })
            });
        }
        if let Some((input, tool, value, mode)) = restore {
            if matches!(tool.as_str(), "files" | "cd") && !std::path::Path::new(&value).exists() {
                self.message("该历史目标已失效或不存在");
                return;
            }
            if tool == "open"
                && !value.starts_with("http://")
                && !value.starts_with("https://")
                && !std::path::Path::new(&value).exists()
            {
                self.message("该历史目标已失效或不存在");
                return;
            }
            self.submode = mode.as_ref().and_then(|m| {
                m.submode.clone().or_else(|| {
                    m.args
                        .last()
                        .and_then(|arg| arg["id"].as_str().map(str::to_string))
                })
            });
            self.attachment = None;
            self.attachment_results = None;
            self.attachment_job = None;
            self.selected = 0;
            self.input = input;
            self.mode = mode.map(|mode| mode.tool_id);
            self.panel = Panel::None;
            self.rebuild();
            self.focus = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn row() -> HistoryItem {
        HistoryItem {
            id: "test-history-row".into(),
            input: "hello".into(),
            tool: "base64".into(),
            tool_name: None,
            title: "Base64 编码".into(),
            value: "aGVsbG8=".into(),
            pinned: false,
            created: 1,
            pin_state: None,
            thumbnail: None,
            mode: None,
            image_asset: None,
            image_source_asset: None,
            image_source_mime: None,
        }
    }
    fn click(at: Pos2) -> Option<RowAction> {
        let ctx = Context::default();
        let mut fonts = FontDefinitions::default();
        fonts.families.insert(
            FontFamily::Name("UiSetting".into()),
            fonts.families[&FontFamily::Proportional].clone(),
        );
        ctx.set_fonts(fonts);
        let icons = HashMap::new();
        let item = row();
        let mut action = None;
        for events in [
            vec![Event::PointerMoved(at)],
            vec![Event::PointerButton {
                pos: at,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            }],
            vec![Event::PointerButton {
                pos: at,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
        ] {
            let _ = ctx.run(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(640., 160.))),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    CentralPanel::default().show(ctx, |ui| {
                        action = paint_row(ui, &icons, &item, 0);
                    });
                },
            );
        }
        action
    }
    #[test]
    fn entire_row_restores_on_click() {
        assert_eq!(click(pos2(200., 40.)), Some(RowAction::Restore));
    }
    #[test]
    fn action_buttons_do_not_bubble_to_restore() {
        for (x, action) in [
            (533., RowAction::TogglePin),
            (569., RowAction::Copy),
            (605., RowAction::Delete),
        ] {
            assert_eq!(click(pos2(x, 40.)), Some(action));
        }
    }
}
