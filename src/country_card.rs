use super::*;

pub(super) fn show(
    app: &mut QuickTool,
    ui: &mut Ui,
    ctx: &Context,
    country: &Value,
    index: usize,
) -> Response {
    let alpha = country["alpha2"].as_str().unwrap_or("");
    let key = format!("flag:{alpha}");
    if !app.icons.contains_key(&key) {
        let path = quicktool_rust::resources::asset("assets/flags/svg")
            .join(format!("{}.svg", alpha.to_lowercase()));
        let svg = std::fs::read(&path).unwrap_or_default();
        if let Some((w, h, rgba)) = render_svg(&svg, 200.) {
            app.icons.insert(
                key.clone(),
                ctx.load_texture(
                    &key,
                    ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba),
                    TextureOptions::LINEAR,
                ),
            );
        }
    }
    let response = Frame::new()
        .fill(if ui.visuals().dark_mode {
            Color32::from_rgb(24, 28, 39)
        } else {
            Color32::from_rgb(248, 250, 255)
        })
        .stroke(Stroke::new(
            1.,
            if index == app.selected {
                Color32::from_rgb(43, 48, 65)
            } else {
                Color32::from_white_alpha(23)
            },
        ))
        .corner_radius(15)
        .inner_margin(12)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                if let Some(texture) = app.icons.get(&key) {
                    ui.add(Image::new(texture).fit_to_exact_size(vec2(68., 46.)));
                }
                ui.add_space(6.);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(country["nameZh"].as_str().unwrap_or(""))
                            .font(FontId::new(16., FontFamily::Name("UiStrong".into())))
                            .strong(),
                    );
                    ui.label(
                        RichText::new(country["nameEn"].as_str().unwrap_or(""))
                            .size(11.)
                            .color(weak_color(ui)),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    Frame::new()
                        .fill(selected_surface(ui))
                        .stroke(Stroke::new(1., Color32::from_rgb(48, 57, 79)))
                        .corner_radius(8)
                        .inner_margin(Margin::symmetric(8, 5))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(alpha)
                                    .font(FontId::new(11., FontFamily::Name("MonoResult".into())))
                                    .color(ACCENT),
                            );
                        });
                });
            });
            ui.add_space(8.);
            ui.separator();
            let calling = country["callingCodes"]
                .as_array()
                .map(|xs| {
                    xs.iter()
                        .filter_map(Value::as_str)
                        .map(|x| format!("+{}", x.trim_start_matches('+')))
                        .collect::<Vec<_>>()
                        .join(" / ")
                })
                .unwrap_or_default();
            let tlds = country["tlds"]
                .as_array()
                .map(|xs| {
                    xs.iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(" / ")
                })
                .unwrap_or_default();
            let fields = [
                ("ISO 2", alpha),
                ("ISO 3", country["alpha3"].as_str().unwrap_or("")),
                ("Numeric", country["numeric"].as_str().unwrap_or("")),
                ("IOC", country["ioc"].as_str().unwrap_or("")),
                ("FIFA", country["fifa"].as_str().unwrap_or("")),
                ("ITU", country["itu"].as_str().unwrap_or("")),
                ("Calling", &calling),
                ("ccTLD", &tlds),
            ];
            let (allocated, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), 159.), Sense::hover());
            let region = allocated.expand2(vec2(12., 0.));
            let half = (region.width() - 1.) / 2.;
            for (index, (label, value)) in fields.iter().enumerate() {
                let min =
                    region.min + vec2((index % 2) as f32 * (half + 1.), (index / 2) as f32 * 40.);
                let rect = Rect::from_min_size(min, vec2(half, 39.));
                let response = ui
                    .interact(rect, ui.id().with((alpha, label)), Sense::click())
                    .native_tooltip("点击复制");
                ui.painter().rect_filled(
                    rect,
                    0.,
                    if response.hovered() {
                        surface(ui, 0.075)
                    } else {
                        if ui.visuals().dark_mode {
                            Color32::from_rgb(25, 29, 40)
                        } else {
                            Color32::from_rgb(246, 249, 255)
                        }
                    },
                );
                ui.painter().text(
                    rect.left_center() + vec2(10., 0.),
                    Align2::LEFT_CENTER,
                    *label,
                    FontId::proportional(9.5),
                    weak_color(ui),
                );
                let value_rect =
                    Rect::from_min_max(rect.min + vec2(85., 0.), rect.max - vec2(40., 0.));
                ui.painter().with_clip_rect(value_rect).text(
                    value_rect.left_center(),
                    Align2::LEFT_CENTER,
                    *value,
                    FontId::monospace(11.),
                    ui.visuals().text_color(),
                );
                if let Some(texture) = app.icons.get("mdi:content-copy") {
                    ui.painter().image(
                        texture.id(),
                        Rect::from_center_size(rect.right_center() - vec2(21., 0.), vec2(14., 14.)),
                        Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
                        weak_color(ui),
                    );
                }
                if response.clicked() {
                    ctx.copy_text(value.to_string());
                    app.message("已复制");
                }
            }
            ui.add_space(1.);
            ui.horizontal(|ui| {
                for (kind, label, icon) in [
                    ("svg", "复制 SVG", "mdi:content-copy"),
                    ("png", "复制 PNG", "mdi:content-copy"),
                    ("emoji", "复制 Emoji", "mdi:flag-outline"),
                ] {
                    let button = if let Some(texture) = app.icons.get(icon) {
                        Button::image_and_text(
                            Image::new(texture)
                                .fit_to_exact_size(vec2(14., 14.))
                                .tint(weak_color(ui)),
                            RichText::new(label).size(10.),
                        )
                    } else {
                        Button::new(RichText::new(label).size(10.))
                    };
                    if ui.add(button.min_size(vec2(80., 28.))).clicked() {
                        match system::copy_flag(alpha, kind) {
                            Ok(()) => app.message(format!("{label} 已复制")),
                            Err(error) => app.message(error),
                        }
                    }
                }
            });
        })
        .response;
    ui.add_space(10.);
    response
}
