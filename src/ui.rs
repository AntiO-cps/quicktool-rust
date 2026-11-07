#[path = "attachments.rs"]
mod attachments;
#[path = "candidates_probe.rs"]
mod candidates_probe;
#[path = "country_card.rs"]
mod country_card;
#[path = "expanded_probe.rs"]
mod expanded_probe;
#[path = "eyedrop_overlay.rs"]
mod eyedrop_overlay;
#[path = "theme.rs"]
mod glow;
#[path = "history_panel.rs"]
mod history_panel;
#[path = "host_candidates.rs"]
mod host_candidates;
#[path = "image_decode.rs"]
mod image_decode;
#[path = "interaction_probe.rs"]
mod interaction_probe;
#[path = "large_input.rs"]
mod large_input;
#[path = "motion_probe.rs"]
mod motion_probe;
#[path = "pickers.rs"]
mod pickers;
#[path = "search_layout_probe.rs"]
mod search_layout_probe;
#[path = "summon_probe.rs"]
mod summon_probe;
#[path = "widgets.rs"]
mod widgets;
use eframe::egui::{epaint::Shadow, *};
use quicktool_rust::{
    core::ResultItem, index, modules, pins::PinManager, platform, storage::Store, system, tools,
};
use serde_json::Value;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use widgets::NativeTooltip as _;
const ACCENT: Color32 = Color32::from_rgb(155, 188, 255);
type AttachmentLoad = Result<(attachments::Attachment, Option<Vec<ResultItem>>), String>;
fn clipboard_has_image() -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::DataExchange::{
            IsClipboardFormatAvailable, RegisterClipboardFormatW,
        };
        let png = "PNG\0".encode_utf16().collect::<Vec<_>>();
        unsafe {
            IsClipboardFormatAvailable(8) != 0
                || IsClipboardFormatAvailable(17) != 0
                || IsClipboardFormatAvailable(RegisterClipboardFormatW(png.as_ptr())) != 0
        }
    }
    #[cfg(not(windows))]
    {
        false
    }
}
#[derive(Clone)]
struct Row {
    score: f64,
    tool: String,
    item: ResultItem,
    command: bool,
    country: Option<(Value, bool)>,
    launch_app: Option<system::AppEntry>,
    passive: bool,
    mode_action: Option<(String, Option<String>)>,
    open_target: Option<String>,
}
#[derive(PartialEq, Clone, Copy)]
enum Panel {
    None,
    Settings,
    History,
}
fn parse_sub_arg<'a>(id: &str, input: &'a str) -> (Option<&'static str>, &'a str) {
    let (sub, tail) = tools::parse_submode(id, input);
    if let Some(sub) = sub {
        if let Some((key, _, _)) = subcommands(id).into_iter().find(|(key, _, _)| *key == sub) {
            return (Some(key), &input[input.len() - tail.len()..]);
        }
    }
    (None, input)
}
fn folder_label(name: &str) -> &str {
    match name {
        "Home" => "主目录 home",
        "Desktop" => "桌面 desktop zm",
        "Documents" => "文档 documents wd",
        "Downloads" => "下载 downloads xz",
        "Pictures" => "图片 pictures tp",
        "Music" => "音乐 music yy",
        "Videos" => "视频 videos sp",
        "Temp" => "临时目录 temp ls",
        _ => name,
    }
}
fn help_description(id: &str) -> &str {
    match id {
        "calculator" => "表达式、函数与常量",
        "time" => "日期、Unix 时间戳与其他时间格式",
        "color" => "颜色预览与多种色值转换",
        "base" => "二、八、十、十六进制互转",
        "unit" => "长度、质量、时间等单位换算",
        "json" => "格式化、压缩与检查",
        "case" => "大小写及命名格式转换",
        "base64" => "文本与图片编解码",
        "url" => "网址编码与解码",
        "escape" => "Unicode、HTML 等转义",
        "width" => "全角与半角互转",
        "text" => "文本处理与常用操作",
        "uuid" => "生成 UUID v4",
        "random" => "生成随机字符串",
        "char" => "字符、码位及 Unicode 名称",
        "country" => "国家名称、代码与国旗",
        "run" => "查找并运行本机应用",
        "files" => "本地文件名与目录索引",
        "cd" => "打开目录或在终端进入",
        "open" => "打开文件、目录或网址",
        "registry" => "查找注册表键与值",
        "pin" => "钉住文字和图片",
        "pinwindow" => "切换主窗口置顶",
        "clipboard" => "查看剪贴板原始格式",
        "history" => "查看、恢复和管理历史",
        "settings" => "外观、快捷键与索引设置",
        _ => "选择命令后进入工具模式",
    }
}
fn mode_entry(id: &str) -> bool {
    matches!(
        id,
        "time"
            | "json"
            | "color"
            | "case"
            | "base64"
            | "base"
            | "url"
            | "escape"
            | "char"
            | "mathchar"
            | "greek"
            | "bopomofo"
            | "emoji"
            | "width"
            | "country"
            | "pin"
            | "clipboard"
            | "run"
            | "cd"
            | "open"
            | "winsettings"
            | "cmd"
            | "powershell"
            | "terminal"
            | "registry"
            | "files"
            | "text"
            | "unit"
    ) && modules::is_enabled(id)
}
fn help_group_index(id: &str) -> usize {
    match id {
        "calculator" | "time" | "color" | "base" | "unit" => 0,
        "json" | "case" | "base64" | "url" | "escape" | "width" | "text" | "uuid" | "random" => 1,
        "char" | "mathchar" | "greek" | "bopomofo" | "emoji" | "country" => 2,
        "run" | "files" | "cd" | "open" | "registry" => 3,
        "pin" | "pinwindow" | "clipboard" | "history" | "settings" => 4,
        _ => 5,
    }
}
fn groups_contains(id: &str) -> bool {
    matches!(
        id,
        "calculator"
            | "time"
            | "color"
            | "base"
            | "unit"
            | "json"
            | "case"
            | "base64"
            | "url"
            | "escape"
            | "width"
            | "text"
            | "uuid"
            | "random"
            | "char"
            | "mathchar"
            | "greek"
            | "bopomofo"
            | "emoji"
            | "country"
            | "run"
            | "files"
            | "cd"
            | "open"
            | "registry"
            | "pin"
            | "pinwindow"
            | "clipboard"
            | "history"
            | "settings"
    )
}
fn disabled_module_row(id: &str) -> Row {
    let name = modules::module_of(id)
        .map(|info| info.name.as_str())
        .unwrap_or(id);
    Row {
        score: 1.,
        tool: id.to_string(),
        item: ResultItem::new(
            format!("{name} 模块未安装"),
            "在设置 → 模块管理中安装后可用",
        ),
        command: false,
        country: None,
        launch_app: None,
        passive: false,
        mode_action: None,
        open_target: None,
    }
}
fn render_svg(bytes: &[u8], width: f32) -> Option<(u32, u32, Vec<u8>)> {
    let tree = resvg::usvg::Tree::from_data(bytes, &Default::default()).ok()?;
    let scale = width / tree.size().width();
    let w = width.ceil() as u32;
    let h = (tree.size().height() * scale).ceil() as u32;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let rgba = pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let p = p.demultiply();
            [p.red(), p.green(), p.blue(), p.alpha()]
        })
        .collect();
    Some((w, h, rgba))
}
fn now_date() -> String {
    let d = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        d.year(),
        d.month() as u8,
        d.day(),
        d.hour(),
        d.minute(),
        d.second()
    )
}
fn setting_row(ui: &mut Ui, title: &str, description: &str, control: impl FnOnce(&mut Ui)) {
    let top = ui.cursor().top();
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width((ui.available_width() - 240.).max(120.));
            ui.label(setting_title(title, 12.5).strong().color(setting_text(ui)));
            ui.add_space(4.);
            ui.label(RichText::new(description).size(10.5).color(weak_color(ui)));
        });
        ui.with_layout(Layout::right_to_left(Align::Center), control);
    });
    let used = ui.cursor().top() - top;
    // .setting-row { min-height:62px; padding:10px 4px; border-bottom:1px }
    ui.add_space((62. - used).max(0.) + 10.);
    draw_bottom_line(ui, false, row_line(ui));
    ui.add_space(10.);
}
fn select_setting(ui: &mut Ui, draft: &mut Value, key: &str, options: &[(&str, &str)]) {
    widgets::select(ui, draft, key, options);
}
fn keycap(ui: &mut Ui, text: &str) {
    let font = if text.is_ascii() {
        FontId::new(9., FontFamily::Name("Jura".into()))
    } else {
        FontId::proportional(9.)
    };
    let galley =
        ui.painter()
            .layout_no_wrap(text.into(), font.clone(), ui.visuals().weak_text_color());
    let (rect, _) = ui.allocate_exact_size(vec2(galley.size().x + 12., 18.), Sense::hover());
    ui.painter().rect(
        rect,
        CornerRadius::same(5),
        surface(ui, 0.025),
        Stroke::new(
            1.,
            if ui.visuals().dark_mode {
                Color32::from_rgb(45, 49, 60)
            } else {
                Color32::from_gray(210)
            },
        ),
        StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        font,
        ui.visuals().weak_text_color(),
    );
}
fn weak_color(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(113, 120, 137)
    } else {
        Color32::from_rgb(108, 118, 138)
    }
}
fn surface(ui: &Ui, alpha: f32) -> Color32 {
    let base = if ui.visuals().dark_mode {
        [21., 24., 33.]
    } else {
        [249., 251., 255.]
    };
    let fg = if ui.visuals().dark_mode { 255. } else { 0. };
    Color32::from_rgb(
        (base[0] + (fg - base[0]) * alpha) as u8,
        (base[1] + (fg - base[1]) * alpha) as u8,
        (base[2] + (fg - base[2]) * alpha) as u8,
    )
}
fn selected_surface(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(33, 39, 58)
    } else {
        Color32::from_rgb(222, 232, 249)
    }
}
fn clip_text(text: &str, limit: usize) -> String {
    let mut prefix = text.encode_utf16().take(limit + 1).collect::<Vec<_>>();
    if prefix.len() <= limit {
        return text.into();
    }
    prefix.truncate(limit.saturating_sub(1));
    format!("{}…", String::from_utf16_lossy(&prefix))
}
// Settings design system; values mirror the original styles.css variables.
fn accent_color(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(155, 188, 255)
    } else {
        Color32::from_rgb(111, 120, 221)
    }
}
fn setting_text(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(245, 247, 251)
    } else {
        Color32::from_rgb(30, 37, 50)
    }
}
fn setting_text_2(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgba_unmultiplied(235, 239, 247, 178)
    } else {
        Color32::from_rgba_unmultiplied(35, 45, 62, 184)
    }
}
fn panel_line(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgba_unmultiplied(255, 255, 255, 15)
    } else {
        Color32::from_rgba_unmultiplied(57, 72, 105, 26)
    }
}
fn row_line(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgba_unmultiplied(255, 255, 255, 14)
    } else {
        Color32::from_rgba_unmultiplied(57, 72, 105, 22)
    }
}
fn border_color(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgba_unmultiplied(255, 255, 255, 24)
    } else {
        Color32::from_rgba_unmultiplied(57, 72, 105, 31)
    }
}
fn field_bg(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgba_unmultiplied(255, 255, 255, 11)
    } else {
        Color32::from_rgba_unmultiplied(246, 249, 255, 190)
    }
}
fn surface_soft_bg(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgba_unmultiplied(255, 255, 255, 14)
    } else {
        Color32::from_rgba_unmultiplied(55, 73, 111, 14)
    }
}
fn hover_bg(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgba_unmultiplied(255, 255, 255, 19)
    } else {
        Color32::from_rgba_unmultiplied(69, 91, 134, 19)
    }
}
fn setting_title(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).font(FontId::new(size, FontFamily::Name("UiSetting".into())))
}
fn draw_bottom_line(ui: &Ui, full_width: bool, color: Color32) {
    let y = ui.cursor().top() - 0.5;
    let left = if full_width {
        ui.max_rect().left()
    } else {
        ui.cursor().left()
    };
    ui.painter().line_segment(
        [pos2(left, y), pos2(ui.max_rect().right(), y)],
        Stroke::new(1., color),
    );
}
fn accent_checkbox(ui: &mut Ui, checked: &mut bool) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(vec2(16., 16.), Sense::click());
    if response.clicked() {
        *checked = !*checked;
        response.mark_changed();
    }
    let painter = ui.painter();
    if *checked {
        painter.rect_filled(rect, CornerRadius::same(4), accent_color(ui));
        let center = rect.center();
        let check = Stroke::new(1.8, Color32::WHITE);
        painter.line_segment([center + vec2(-3.4, 0.4), center + vec2(-1.1, 2.8)], check);
        painter.line_segment([center + vec2(-1.1, 2.8), center + vec2(3.8, -3.0)], check);
    } else {
        painter.rect_stroke(
            rect,
            CornerRadius::same(4),
            Stroke::new(1., border_color(ui)),
            StrokeKind::Inside,
        );
    }
    response
}
fn soft_button(ui: &mut Ui, label: &str) -> Response {
    let font = FontId::new(11., FontFamily::Proportional);
    let color = setting_text_2(ui);
    let galley = ui.painter().layout_no_wrap(label.into(), font, color);
    let size = vec2(galley.size().x + 20., 34.);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let fill = if response.hovered() {
        hover_bg(ui)
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect(
        rect,
        CornerRadius::same(10),
        fill,
        Stroke::NONE,
        StrokeKind::Inside,
    );
    ui.painter().galley(
        rect.left_top() + vec2(10., (rect.height() - galley.size().y) / 2.),
        galley,
        color,
    );
    response
}
fn primary_button(ui: &mut Ui, label: &str) -> Response {
    let dark = ui.visuals().dark_mode;
    let text_color = if dark {
        Color32::from_rgb(238, 244, 255)
    } else {
        Color32::from_rgb(29, 36, 49)
    };
    let font = FontId::new(11.5, FontFamily::Proportional);
    let galley = ui.painter().layout_no_wrap(label.into(), font, text_color);
    let size = vec2(galley.size().x + 28., 34.);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let (fill, stroke) = if response.hovered() {
        (
            if dark {
                Color32::from_rgba_unmultiplied(155, 188, 255, 56)
            } else {
                Color32::from_rgba_unmultiplied(120, 150, 255, 76)
            },
            Stroke::new(
                1.,
                if dark {
                    Color32::from_rgba_unmultiplied(155, 188, 255, 74)
                } else {
                    Color32::from_rgba_unmultiplied(70, 95, 180, 82)
                },
            ),
        )
    } else {
        (
            if dark {
                Color32::from_rgba_unmultiplied(155, 188, 255, 38)
            } else {
                Color32::from_rgba_unmultiplied(120, 150, 255, 51)
            },
            Stroke::new(
                1.,
                if dark {
                    Color32::from_rgba_unmultiplied(155, 188, 255, 56)
                } else {
                    Color32::from_rgba_unmultiplied(70, 95, 180, 82)
                },
            ),
        )
    };
    ui.painter().rect(
        rect,
        CornerRadius::same(10),
        fill,
        stroke,
        StrokeKind::Inside,
    );
    ui.painter().galley(
        rect.left_top() + vec2(14., (rect.height() - galley.size().y) / 2.),
        galley,
        text_color,
    );
    response
}
fn country_candidates(query: &str) -> Vec<Value> {
    quicktool_rust::countries::card_matches(query)
        .into_iter()
        .map(|(country, _)| country)
        .collect()
}
fn country_completion(country: &Value, input: &str) -> String {
    quicktool_rust::countries::completion(country, input)
}

fn subcommands(id: &str) -> Vec<(&'static str, &'static str, Vec<&'static str>)> {
    match id {
        "base64" => vec![
            ("encode", "编码", vec!["encode", "编码", "bianma", "bm"]),
            ("decode", "解码", vec!["decode", "解码", "jiema", "jm"]),
        ],
        "cmd" | "powershell" | "terminal" => vec![(
            "admin",
            "管理员",
            vec!["admin", "administrator", "管理员", "guanliyuan", "gly", "a"],
        )],
        _ => vec![],
    }
}
fn sub_label(id: &str) -> &str {
    match id {
        "encode" => "编码",
        "decode" => "解码",
        "admin" => "管理员",
        _ => id,
    }
}
fn is_launch(id: &str) -> bool {
    id.starts_with("system:")
        || matches!(
            id,
            "run"
                | "cd"
                | "open"
                | "cmd"
                | "powershell"
                | "terminal"
                | "winsettings"
                | "files"
                | "registry"
                | "system"
                | "service"
                | "power"
        )
}
struct IconWorker {
    requests: std::sync::mpsc::Sender<String>,
    responses: std::sync::mpsc::Receiver<(
        String,
        Result<quicktool_rust::shell_icons::ShellIcon, String>,
    )>,
    requested: std::collections::HashSet<String>,
    loaded: std::collections::VecDeque<String>,
}
fn shell_target(row: &Row) -> Option<&str> {
    if let Some(app) = &row.launch_app {
        return Some(&app.target);
    }
    (matches!(row.tool.as_str(), "run" | "files")
        || (row.tool == "cd" && row.item.title.starts_with("打开"))
        || (row.tool == "open"
            && !row.item.value.starts_with("http")
            && !row.item.title.starts_with("复制")))
    .then_some(row.item.value.as_str())
}
struct Probe {
    path: std::path::PathBuf,
    ready_at: Option<Instant>,
    startup_ms: f64,
    ready_frame: u32,
    idle_frames: Option<u32>,
    idle: Option<Value>,
    pending: Option<(String, Instant, f64)>,
    samples: Vec<Value>,
}
#[cfg(windows)]
fn process_memory() -> Value {
    #[repr(C)]
    struct Counters {
        cb: u32,
        faults: u32,
        peak_ws: usize,
        ws: usize,
        peak_paged: usize,
        paged: usize,
        peak_nonpaged: usize,
        nonpaged: usize,
        pagefile: usize,
        peak_pagefile: usize,
        private: usize,
    }
    #[link(name = "psapi")]
    unsafe extern "system" {
        fn GetProcessMemoryInfo(
            process: *mut std::ffi::c_void,
            counters: *mut Counters,
            size: u32,
        ) -> i32;
    }
    let mut c: Counters = unsafe { std::mem::zeroed() };
    c.cb = std::mem::size_of::<Counters>() as u32;
    let success = unsafe {
        GetProcessMemoryInfo(
            windows_sys::Win32::System::Threading::GetCurrentProcess(),
            &mut c,
            std::mem::size_of::<Counters>() as u32,
        )
    };
    if success == 0 {
        serde_json::json!({"error":std::io::Error::last_os_error().to_string()})
    } else {
        serde_json::json!({"working_set_bytes":c.ws,"private_bytes":c.private,"peak_working_set_bytes":c.peak_ws})
    }
}
#[cfg(not(windows))]
fn process_memory() -> Value {
    serde_json::json!({"error":"process memory metrics require Windows"})
}
pub struct QuickTool {
    store: Store,
    input: String,
    large_input: large_input::LargeInput,
    attachment: Option<attachments::Attachment>,
    attachment_job: Option<std::sync::mpsc::Receiver<AttachmentLoad>>,
    attachment_results: Option<Vec<ResultItem>>,
    decoded_image: Option<attachments::Attachment>,
    decoder: image_decode::LatestWorker<attachments::Attachment>,
    decoded_request: Option<(std::sync::Arc<str>, u64)>,
    decoded_pending: bool,
    image_copy: Option<std::sync::mpsc::Receiver<Result<(), String>>>,
    mode: Option<String>,
    rows: Vec<Row>,
    selected: usize,
    tab: usize,
    pinned: bool,
    panel: Panel,
    draft: Value,
    confirm_discard: bool,
    modules_page: bool,
    focus: bool,
    icons: HashMap<String, TextureHandle>,
    shell_icons: Option<IconWorker>,
    pins: PinManager,
    submode: Option<String>,
    completion_suppressed: Option<String>,
    recording_shortcut: bool,
    recording_rect: Option<Rect>,
    shortcut_error: Option<String>,
    color_picker: bool,
    native_color: pickers::ColorPicker,
    native_date: pickers::DatePicker,
    eyedrop: eyedrop_overlay::EyeDropperOverlay,
    eyedrop_pending: u8,
    picker_color: [f32; 4],
    date_picker: bool,
    date_value: String,
    toast: Option<(String, Instant)>,
    last_height: f32,
    result_height: f32,
    result_scroll: f32,
    screenshot: Option<std::path::PathBuf>,
    screenshot_sent: bool,
    screenshot_requested: Option<Instant>,
    probe: Option<Probe>,
    interaction_probe: Option<interaction_probe::InteractionProbe>,
    summon_probe: Option<summon_probe::SummonProbe>,
    expanded_probe: Option<expanded_probe::ExpandedProbe>,
    motion_probe: Option<motion_probe::MotionProbe>,
    search_layout_probe: Option<search_layout_probe::SearchLayoutProbe>,
    layout_snapshot: Value,
    candidates_probe: Option<candidates_probe::CandidatesProbe>,
    candidate_testing: bool,
    file_results: Option<(String, Vec<index::FileEntry>)>,
    scroll_reset: bool,
    render_context: Option<(String, Option<String>, Option<String>, usize)>,
    last_scroll_selection: Option<usize>,
    motion: quicktool_rust::launcher_motion::Motion,
    placement: quicktool_rust::launcher_native::LauncherPlacement,
    place_requested: bool,
    select_search: bool,
    frames: u32,
    hotkey: Option<platform::Hotkey>,
    hotkey_error: Option<String>,
    registered_shortcut: String,
    was_focused: bool,
    tray: Option<quicktool_rust::tray::Tray>,
    window_style: Option<(bool, u32, u32, [i32; 2])>,
    backdrop: Option<quicktool_rust::backdrop::Backdrop>,
    backdrop_initialized: bool,
    glow: glow::Glow,
    main_visible: bool,
    applied_topmost: Option<bool>,
    context: Context,
    file_index: Option<index::Index>,
    file_status: index::Status,
    file_serial: u64,
    files_pending: bool,
    file_query: Option<String>,
    system_backend: system::Backend,
    apps: Vec<system::AppEntry>,
    apps_requested: bool,
    clipboard_entries: Vec<platform::ClipboardEntry>,
    clipboard_loaded: bool,
    country_cache: Vec<Value>,
    system_dark: bool,
    theme_polled: Instant,
    clipboard_image: Option<(u32, u32, Vec<u8>)>,
    registry_serial: u64,
    registry_query: String,
    registry_items: Vec<system::RegistryEntry>,
    registry_due: Option<Instant>,
    registry_pending: bool,
    registry_running_query: String,
    file_debounce: Option<(String, Instant)>,
}
fn font_setup(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    for (name, bytes, family) in [
        (
            "misans",
            include_bytes!("../assets/assets/fonts/MiSans-VF.ttf").as_slice(),
            FontFamily::Proportional,
        ),
        (
            "jetbrains",
            include_bytes!("../assets/assets/fonts/JetBrainsMono-VF.ttf").as_slice(),
            FontFamily::Monospace,
        ),
        (
            "jura",
            include_bytes!("../assets/assets/fonts/Jura-VF.ttf").as_slice(),
            FontFamily::Name("Jura".into()),
        ),
    ] {
        fonts.font_data.insert(
            name.into(),
            std::sync::Arc::new(
                FontData::from_static(bytes)
                    .variation(*b"wght", if name == "misans" { 450. } else { 400. }),
            ),
        );
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, name.into());
    }
    for (name, path) in [
        ("segoe", "C:/Windows/Fonts/segoeui.ttf"),
        ("yahei", "C:/Windows/Fonts/msyh.ttc"),
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert(
                name.into(),
                std::sync::Arc::new(FontData::from_owned(bytes)),
            );
            for family in [FontFamily::Proportional, FontFamily::Monospace] {
                fonts.families.entry(family).or_default().push(name.into());
            }
        }
    }
    if fonts.font_data.contains_key("segoe") {
        fonts
            .families
            .get_mut(&FontFamily::Proportional)
            .unwrap()
            .push("segoe".into());
    }
    for (name, weight) in [
        ("UiMedium", 570.),
        ("UiSetting", 590.),
        ("UiStrong", 650.),
        ("UiHelp", 600.),
        ("UiTag", 560.),
    ] {
        let key = format!("misans-{weight}");
        fonts.font_data.insert(
            key.clone(),
            std::sync::Arc::new(
                FontData::from_static(include_bytes!("../assets/assets/fonts/MiSans-VF.ttf"))
                    .variation(*b"wght", weight),
            ),
        );
        let mut fallback = fonts.families[&FontFamily::Proportional].clone();
        fallback.insert(0, key);
        fonts
            .families
            .insert(FontFamily::Name(name.into()), fallback);
    }
    fonts.font_data.insert(
        "mono-result".into(),
        std::sync::Arc::new(
            FontData::from_static(include_bytes!(
                "../assets/assets/fonts/JetBrainsMono-VF.ttf"
            ))
            .variation(*b"wght", 610.),
        ),
    );
    fonts.font_data.insert(
        "mono-result-cjk".into(),
        std::sync::Arc::new(
            FontData::from_static(include_bytes!("../assets/assets/fonts/MiSans-VF.ttf"))
                .variation(*b"wght", 610.),
        ),
    );
    let mut mono_fallback = fonts.families[&FontFamily::Monospace].clone();
    mono_fallback.insert(0, "mono-result-cjk".into());
    mono_fallback.insert(0, "mono-result".into());
    fonts
        .families
        .insert(FontFamily::Name("MonoResult".into()), mono_fallback);
    fonts.font_data.insert(
        "pin-misans".into(),
        std::sync::Arc::new(
            FontData::from_static(include_bytes!("../assets/assets/fonts/MiSans-VF.ttf"))
                .variation(*b"wght", 400.),
        ),
    );
    fonts.families.insert(
        FontFamily::Name("PinText".into()),
        ["pin-misans", "yahei", "segoe", "misans"]
            .into_iter()
            .filter(|name| fonts.font_data.contains_key(*name))
            .map(str::to_string)
            .collect(),
    );
    fonts.families.insert(
        FontFamily::Name("NativePicker".into()),
        ["segoe", "yahei", "pin-misans"]
            .into_iter()
            .filter(|name| fonts.font_data.contains_key(*name))
            .map(str::to_owned)
            .collect(),
    );
    ctx.set_fonts(fonts);
}
fn icon_textures(ctx: &Context) -> HashMap<String, TextureHandle> {
    let mut map = HashMap::new();
    let data: Value = serde_json::from_str(include_str!("../assets/icons.json")).unwrap();
    let builtin: Value = serde_json::from_str(include_str!("ui_icons.json")).unwrap();
    for (key, value) in data["icons"]
        .as_object()
        .unwrap()
        .iter()
        .chain(builtin.as_object().unwrap().iter())
    {
        let Some(body) = value["body"].as_str() else {
            continue;
        };
        let svg = format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='24' height='24' viewBox='0 0 24 24'>{}</svg>",
            body.replace("currentColor", "#ffffff")
        );
        let Ok(tree) = resvg::usvg::Tree::from_str(&svg, &Default::default()) else {
            continue;
        };
        let mut pixmap = resvg::tiny_skia::Pixmap::new(48, 48).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::from_scale(2., 2.),
            &mut pixmap.as_mut(),
        );
        map.insert(
            format!("mdi:{key}"),
            ctx.load_texture(
                key,
                ColorImage::from_rgba_premultiplied([48, 48], pixmap.data()),
                TextureOptions::LINEAR,
            ),
        );
    }
    map
}
impl QuickTool {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        font_setup(&cc.egui_ctx);
        let icons = icon_textures(&cc.egui_ctx);
        let metrics_path = std::env::args()
            .skip_while(|a| a != "--metrics")
            .nth(1)
            .map(std::path::PathBuf::from);
        let interaction_probe = interaction_probe::InteractionProbe::from_args();
        let summon_probe = summon_probe::SummonProbe::from_args();
        let expanded_probe = expanded_probe::ExpandedProbe::from_args();
        let motion_probe = motion_probe::MotionProbe::from_args();
        let search_layout_probe = search_layout_probe::SearchLayoutProbe::from_args();
        let candidates_probe = candidates_probe::CandidatesProbe::from_args();
        let candidate_testing = candidates_probe.is_some();
        let isolated = candidate_testing
            || metrics_path.is_some()
            || interaction_probe.is_some()
            || summon_probe.is_some()
            || expanded_probe.is_some()
            || motion_probe.is_some()
            || search_layout_probe.is_some()
            || std::env::args().any(|a| a == "--picker-screenshot");
        let mut store = Store::open(
            if summon_probe.is_some()
                || expanded_probe.is_some()
                || motion_probe.is_some()
                || search_layout_probe.is_some()
            {
                std::env::temp_dir().join(format!("QuickTool-Rust-summon-{}", std::process::id()))
            } else if isolated {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(".qa")
                    .join(format!("metrics-data-{}", std::process::id()))
            } else {
                Store::default_root()
            },
        );
        if isolated {
            let empty = store.root.join("empty");
            if let Err(error) = std::fs::create_dir_all(&empty) {
                eprintln!("metrics empty index directory: {error}");
            }
            store.settings["theme"] = serde_json::json!("dark");
            store.settings["reduceMotion"] =
                serde_json::json!(motion_probe.is_none() && search_layout_probe.is_none());
            store.settings["globalShortcut"] = serde_json::json!("F24");
            store.settings["hideOnBlur"] = serde_json::json!(false);
            store.settings["fileIndexRoots"] = serde_json::json!(empty.to_string_lossy());
            if summon_probe.is_some() {
                store.settings["globalShortcut"] = serde_json::json!("Ctrl+Alt+Shift+F22");
                store.settings["hideOnBlur"] = serde_json::json!(true);
            }
        }
        let draft = store.settings.clone();
        let input = std::env::args()
            .skip_while(|a| a != "--query")
            .nth(1)
            .unwrap_or_default();
        let screenshot = std::env::args()
            .skip_while(|a| a != "--screenshot")
            .nth(1)
            .map(std::path::PathBuf::from);
        let unattended = candidate_testing
            || interaction_probe.is_some()
            || screenshot.is_some()
            || metrics_path.is_some()
            || std::env::args().any(|a| {
                a == "--picker-screenshot"
                    || a == "--pin-screenshot"
                    || a == "--pin-effects-screenshot"
                    || a == "--select-screenshot"
                    || a == "--glow-screenshot"
            });
        let (hotkey, hotkey_error) = if !unattended {
            let context = cc.egui_ctx.clone();
            match platform::hotkey(
                store.settings["globalShortcut"]
                    .as_str()
                    .unwrap_or("Alt+Space"),
                move || context.request_repaint_of(ViewportId::ROOT),
            ) {
                Ok(h) => (Some(h), None),
                Err(e) => (None, Some(e)),
            }
        } else {
            (None, None)
        };
        let app_cache_root = store.root.clone();
        let registered_shortcut = store.settings["globalShortcut"]
            .as_str()
            .unwrap_or("Alt+Space")
            .to_string();
        let mut app = Self {
            store,
            input,
            large_input: large_input::LargeInput::default(),
            attachment: None,
            attachment_job: None,
            attachment_results: None,
            decoded_image: None,
            decoder: {
                let context = cc.egui_ctx.clone();
                image_decode::LatestWorker::new(attachments::Attachment::from_base64, move || {
                    context.request_repaint_of(ViewportId::ROOT)
                })
            },
            decoded_request: None,
            decoded_pending: false,
            image_copy: None,
            mode: None,
            rows: vec![],
            selected: 0,
            tab: 0,
            pinned: false,
            panel: Panel::None,
            draft,
            confirm_discard: false,
            modules_page: false,
            focus: true,
            icons,
            shell_icons: None,
            pins: PinManager::new(),
            submode: None,
            completion_suppressed: None,
            recording_shortcut: false,
            recording_rect: None,
            shortcut_error: None,
            color_picker: false,
            native_color: pickers::ColorPicker::default(),
            native_date: pickers::DatePicker::default(),
            eyedrop: eyedrop_overlay::EyeDropperOverlay::default(),
            eyedrop_pending: 0,
            picker_color: [0.486, 0.227, 0.929, 1.],
            date_picker: false,
            date_value: now_date(),
            toast: None,
            last_height: 84.,
            result_height: 220.,
            result_scroll: 0.,
            screenshot: screenshot.clone(),
            screenshot_sent: false,
            screenshot_requested: None,
            interaction_probe,
            summon_probe,
            expanded_probe,
            motion_probe,
            search_layout_probe,
            layout_snapshot: Value::Null,
            candidates_probe,
            candidate_testing,
            file_results: None,
            scroll_reset: true,
            render_context: None,
            last_scroll_selection: None,
            motion: Default::default(),
            placement: Default::default(),
            place_requested: true,
            select_search: false,
            probe: metrics_path.map(|path| Probe {
                path,
                ready_at: None,
                startup_ms: 0.,
                ready_frame: 0,
                idle_frames: None,
                idle: None,
                pending: None,
                samples: vec![],
            }),
            frames: 0,
            hotkey,
            hotkey_error,
            registered_shortcut,
            was_focused: false,
            tray: if !unattended {
                let c = cc.egui_ctx.clone();
                quicktool_rust::tray::Tray::new(move || c.request_repaint_of(ViewportId::ROOT)).ok()
            } else {
                None
            },
            window_style: None,
            backdrop: None,
            backdrop_initialized: false,
            glow: Default::default(),
            main_visible: true,
            applied_topmost: None,
            context: cc.egui_ctx.clone(),
            file_index: None,
            file_status: Default::default(),
            file_serial: 0,
            files_pending: false,
            file_query: None,
            system_backend: {
                let c = cc.egui_ctx.clone();
                system::Backend::with_cache(app_cache_root, move || {
                    c.request_repaint_of(ViewportId::ROOT)
                })
            },
            apps: vec![],
            apps_requested: false,
            clipboard_entries: vec![],
            clipboard_loaded: false,
            country_cache: vec![],
            system_dark: system::dark_theme(),
            theme_polled: Instant::now(),
            clipboard_image: None,
            registry_serial: 0,
            registry_query: String::new(),
            registry_items: vec![],
            registry_due: None,
            registry_pending: false,
            registry_running_query: String::new(),
            file_debounce: None,
        };
        if let Some(path) = std::env::args().skip_while(|a| a != "--query-file").nth(1) {
            match std::fs::read_to_string(path) {
                Ok(input) => app.input = input,
                Err(error) => eprintln!("query-file: {error}"),
            }
        }
        if let Some(mode) = std::env::args().skip_while(|a| a != "--mode").nth(1) {
            app.mode = Some(mode);
        }
        if let Some(panel) = std::env::args().skip_while(|a| a != "--panel").nth(1) {
            app.panel = match panel.as_str() {
                "settings" => Panel::Settings,
                "modules" => {
                    app.modules_page = true;
                    Panel::Settings
                }
                "history" => Panel::History,
                "color-picker" | "color-native" => {
                    app.native_color.open = panel == "color-native";
                    app.color_picker = true;
                    if let Some(color) = quicktool_rust::colors::candidates(&app.input).first() {
                        app.picker_color = [
                            color.r as f32,
                            color.g as f32,
                            color.b as f32,
                            color.a as f32,
                        ];
                    }
                    Panel::None
                }
                "date-picker" => {
                    app.date_picker = true;
                    if !app.input.is_empty() {
                        app.date_value = app.input.clone();
                    }
                    Panel::None
                }
                _ => Panel::None,
            };
        }
        if let Some(path) = std::env::args().skip_while(|a| a != "--attachment").nth(1) {
            match attachments::Attachment::from_path(std::path::Path::new(&path)) {
                Ok(attachment) => app.set_attachment(attachment),
                Err(error) => app.message(error),
            }
        }
        if let Some(text) = std::env::args().skip_while(|a| a != "--pin-text").nth(1) {
            app.pins.create_text(text);
        }
        if let Some(path) = std::env::args().skip_while(|a| a != "--pin-image").nth(1) {
            if let Err(error) = app.pins.create_image_path(std::path::Path::new(&path)) {
                app.message(error);
            }
        }
        if std::env::args().any(|a| a == "--pin-effects") {
            app.pins.show_effects_for_all();
        }
        if let Some(path) = std::env::args()
            .skip_while(|a| a != "--pin-screenshot")
            .nth(1)
        {
            app.pins
                .request_screenshot(std::path::Path::new(&path), false);
        }
        if let Some(path) = std::env::args()
            .skip_while(|a| a != "--pin-effects-screenshot")
            .nth(1)
        {
            app.pins
                .request_screenshot(std::path::Path::new(&path), true);
        }
        modules::refresh();
        app.rebuild();
        if !app.store.load_errors.is_empty() {
            app.message(app.store.load_errors.join("；"));
        }
        app
    }
    fn message(&mut self, s: impl Into<String>) {
        self.toast = Some((s.into(), Instant::now()));
    }
    fn icon(&self, ui: &mut Ui, key: &str, size: f32) {
        if let Some(t) = self.icons.get(key).or_else(|| {
            if key == "mdi:wrench-outline" {
                self.icons.get("mdi:tools")
            } else {
                None
            }
        }) {
            ui.add(Image::new(t).fit_to_exact_size(vec2(size, size)).tint(
                if ui.visuals().dark_mode {
                    Color32::from_gray(168)
                } else {
                    Color32::from_gray(77)
                },
            ));
        } else {
            let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
            let p = ui.painter();
            let color = ui.visuals().weak_text_color();
            let stroke = Stroke::new(1.4, color);
            match key {
                "mdi:view-grid-outline" => {
                    for x in 0..2 {
                        for y in 0..2 {
                            let r = Rect::from_min_size(
                                rect.min
                                    + vec2(
                                        2. + x as f32 * size * 0.48,
                                        2. + y as f32 * size * 0.48,
                                    ),
                                vec2(size * 0.3, size * 0.3),
                            );
                            p.rect_stroke(r, CornerRadius::same(0), stroke, StrokeKind::Inside);
                        }
                    }
                }
                "mdi:play-circle-outline" => {
                    p.circle_stroke(rect.center(), size * 0.42, stroke);
                    p.add(Shape::convex_polygon(
                        vec![
                            rect.center() + vec2(-size * 0.1, -size * 0.2),
                            rect.center() + vec2(size * 0.2, 0.),
                            rect.center() + vec2(-size * 0.1, size * 0.2),
                        ],
                        color,
                        Stroke::NONE,
                    ));
                }
                "mdi:chevron-right" => {
                    p.line_segment(
                        [
                            rect.center() + vec2(-size * 0.16, -size * 0.28),
                            rect.center() + vec2(size * 0.16, 0.),
                        ],
                        stroke,
                    );
                    p.line_segment(
                        [
                            rect.center() + vec2(size * 0.16, 0.),
                            rect.center() + vec2(-size * 0.16, size * 0.28),
                        ],
                        stroke,
                    );
                }
                _ => {
                    p.circle_stroke(rect.center(), size * 0.35, stroke);
                }
            }
        }
    }
    fn icon_button(&self, ui: &mut Ui, key: &str, label: &str, active: bool) -> Response {
        let button = if let Some(t) = self.icons.get(key) {
            Button::image(Image::new(t).fit_to_exact_size(vec2(19., 19.)).tint(
                if ui.visuals().dark_mode {
                    Color32::from_rgb(165, 173, 189)
                } else {
                    Color32::from_rgb(91, 102, 122)
                },
            ))
        } else {
            Button::new(label)
        };
        ui.add(
            button
                .min_size(vec2(42., 42.))
                .corner_radius(12)
                .fill(if active {
                    Color32::from_rgba_unmultiplied(155, 188, 255, 35)
                } else {
                    Color32::TRANSPARENT
                }),
        )
        .native_tooltip(label)
    }
    fn icon_button_compact(&self, ui: &mut Ui, key: &str, label: &str) -> Response {
        let button = if let Some(t) = self.icons.get(key) {
            Button::image(
                Image::new(t)
                    .fit_to_exact_size(vec2(17., 17.))
                    .tint(setting_text_2(ui)),
            )
        } else {
            Button::new(label)
        };
        ui.add(
            button
                .min_size(vec2(34., 34.))
                .corner_radius(10)
                .fill(Color32::TRANSPARENT),
        )
        .native_tooltip(label)
    }
    // Original .panel-head: icon tile, strong title, weak subtitle, right actions.
    fn panel_head(
        &mut self,
        ui: &mut Ui,
        icon: &str,
        title: &str,
        subtitle: &str,
        actions: impl FnOnce(&mut Self, &mut Ui),
    ) {
        let top = ui.cursor().top();
        ui.horizontal(|ui| {
            ui.add_space(14.);
            let (rect, _) = ui.allocate_exact_size(vec2(34., 34.), Sense::hover());
            ui.painter().rect(
                rect,
                CornerRadius::same(10),
                surface_soft_bg(ui),
                Stroke::NONE,
                StrokeKind::Inside,
            );
            if let Some(texture) = self.icons.get(icon) {
                ui.painter().image(
                    texture.id(),
                    rect.shrink(7.5),
                    Rect::from_min_max(pos2(0., 0.), pos2(1., 1.)),
                    setting_text_2(ui),
                );
            }
            ui.add_space(10.);
            ui.vertical(|ui| {
                ui.label(setting_title(title, 13.5).strong().color(setting_text(ui)));
                ui.add_space(2.);
                ui.label(RichText::new(subtitle).size(10.5).color(weak_color(ui)));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(12.);
                actions(self, ui);
            });
        });
        let used = ui.cursor().top() - top;
        // .panel-head { min-height:66px; border-bottom:1px }
        ui.add_space((66. - used).max(0.) + 10.);
        draw_bottom_line(ui, true, panel_line(ui));
    }
    fn set_attachment(&mut self, attachment: attachments::Attachment) {
        let prepared = Some(attachment.results());
        self.set_attachment_ready(attachment, prepared);
    }
    fn set_attachment_ready(
        &mut self,
        attachment: attachments::Attachment,
        prepared: Option<Vec<ResultItem>>,
    ) {
        let image = &attachment.thumbnail;
        self.icons.insert(
            "input-attachment".into(),
            self.context.load_texture(
                "input-attachment",
                ColorImage::from_rgba_unmultiplied(
                    [image.width() as usize, image.height() as usize],
                    image.as_raw(),
                ),
                TextureOptions::LINEAR,
            ),
        );
        self.attachment = Some(attachment);
        self.attachment_results = prepared;
        self.rows.clear();
        self.input.clear();
        self.focus = true;
        self.rebuild();
    }
    fn load_attachment(
        &mut self,
        loader: impl FnOnce() -> Result<attachments::Attachment, String> + Send + 'static,
    ) {
        let prepare = self.mode.is_none() || self.mode.as_deref() == Some("base64");
        let context = self.context.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        self.attachment_job = Some(receiver);
        std::thread::spawn(move || {
            let result = loader().map(|attachment| {
                let results = prepare.then(|| attachment.results());
                (attachment, results)
            });
            let _ = sender.send(result);
            context.request_repaint_of(ViewportId::ROOT);
        });
    }
    fn rebuild(&mut self) {
        if self.attachment.is_some()
            && ((self.mode.is_none() && !self.input.starts_with('/'))
                || self.mode.as_deref() == Some("base64"))
            && self.rows.len() == 2
            && self.rows[0].item.title == "图片 → Base64"
            && self.rows[1].item.title == "图片 → Data URL"
        {
            return;
        }
        let previous_pending = std::mem::take(&mut self.decoded_pending);
        let previous_image = self.decoded_image.take();
        let previous_request = self.decoded_request.take();
        let context = (
            self.input.clone(),
            self.mode.clone(),
            self.submode.clone(),
            self.tab,
        );
        let changed = self.render_context.as_ref() != Some(&context);
        let query_changed = self
            .render_context
            .as_ref()
            .is_none_or(|old| old.0 != context.0 || old.1 != context.1 || old.2 != context.2);
        self.render_context = Some(context);
        let previous_selected = self.selected;
        if changed {
            self.scroll_reset = true;
            self.last_scroll_selection = Some(0);
        }
        if query_changed && !self.candidate_testing {
            self.file_serial += 1;
            self.files_pending = false;
            self.file_query = None;
            self.file_debounce = None;
        }
        self.rebuild_inner();
        self.finalize_candidates();
        if !changed {
            self.selected = if previous_selected < self.rows.len() {
                previous_selected
            } else {
                0
            };
        }
        if self
            .rows
            .iter()
            .any(|row| row.tool == "country" && !row.command)
        {
            let query = if self.mode.as_deref() == Some("country") {
                self.input.as_str()
            } else if let Some(command) = self.input.strip_prefix('/') {
                command
                    .split_once(quicktool_rust::core::js_space)
                    .map(|(_, args)| args)
                    .unwrap_or("")
            } else {
                self.input.as_str()
            };
            self.country_cache = country_candidates(query);
            for row in &mut self.rows {
                if row.tool == "country" && !row.command {
                    if let Some((index, country)) =
                        self.country_cache.iter().enumerate().find(|(_, country)| {
                            country["alpha2"].as_str() == Some(&row.item.value)
                        })
                    {
                        let primary = index == 0;
                        row.country = Some((country.clone(), primary));
                        row.score = if primary { 1.15 } else { 0.72 };
                        row.item.title = if primary {
                            format!(
                                "{} · {}",
                                country["nameZh"].as_str().unwrap_or(""),
                                country["nameEn"].as_str().unwrap_or("")
                            )
                        } else {
                            format!(
                                "{} {}",
                                country["flagEmoji"].as_str().unwrap_or(""),
                                country["nameZh"].as_str().unwrap_or("")
                            )
                        };
                        if !primary {
                            row.item.value = format!(
                                "{} · {}",
                                country["alpha2"].as_str().unwrap_or(""),
                                country["alpha3"].as_str().unwrap_or("")
                            );
                        }
                    }
                }
            }
        } else {
            self.country_cache.clear();
        }
        if self.attachment.is_some() || self.submode.as_deref() == Some("encode") {
            return;
        }
        if self
            .rows
            .iter()
            .any(|row| row.tool == "base64" && !row.command)
        {
            let (inline_sub, input) = if self.mode.as_deref() == Some("base64") {
                parse_sub_arg("base64", &self.input)
            } else if let Some((_, args)) = self
                .input
                .strip_prefix('/')
                .and_then(|s| s.split_once(quicktool_rust::core::js_space))
            {
                parse_sub_arg("base64", args)
            } else {
                (None, self.input.as_str())
            };
            if inline_sub == Some("encode") {
                return;
            }
            if let Some((key, serial)) = previous_request.filter(|(key, _)| key.as_ref() == input) {
                self.decoded_request = Some((key, serial));
                self.decoded_pending = previous_pending;
                if let Some(image) = previous_image {
                    self.install_decoded_image(image);
                }
            } else {
                let key: std::sync::Arc<str> = std::sync::Arc::from(input);
                self.decoded_pending = true;
                let serial = self.decoder.request(key.clone());
                self.decoded_request = Some((key, serial));
            }
        }
    }
    fn install_decoded_image(&mut self, image: attachments::Attachment) {
        let Some(index) = self
            .rows
            .iter()
            .position(|row| row.tool == "base64" && !row.command)
        else {
            return;
        };
        let rgba = &image.thumbnail;
        self.icons.insert(
            "decoded-image".into(),
            self.context.load_texture(
                "decoded-image",
                ColorImage::from_rgba_unmultiplied(
                    [rgba.width() as usize, rgba.height() as usize],
                    rgba.as_raw(),
                ),
                TextureOptions::LINEAR,
            ),
        );
        self.rows.insert(
            index + usize::from(self.rows[index].item.title.contains("编码")),
            Row {
                score: self.rows[index].score,
                tool: "base64".into(),
                item: ResultItem::new(
                    format!(
                        "Base64 → {} 图片",
                        image.mime.trim_start_matches("image/").to_uppercase()
                    ),
                    format!("{} 图片", image.mime),
                ),
                command: false,
                country: None,
                launch_app: None,
                passive: false,
                mode_action: None,
                open_target: None,
            },
        );
        self.decoded_image = Some(image);
    }

    fn rebuild_inner(&mut self) {
        self.rows.clear();
        self.selected = 0;

        if let Some(attachment) = &self.attachment {
            if (self.mode.is_none() && !self.input.starts_with('/'))
                || self.mode.as_deref() == Some("base64")
            {
                self.rows = self
                    .attachment_results
                    .take()
                    .unwrap_or_else(|| attachment.results())
                    .into_iter()
                    .map(|item| Row {
                        score: 1.2,
                        tool: "base64".into(),
                        item,
                        command: false,
                        country: None,
                        launch_app: None,
                        passive: false,
                        mode_action: None,
                        open_target: None,
                    })
                    .collect();
                return;
            }
            if self.mode.as_deref() == Some("pin") {
                self.rows = vec![Row {
                    score: 1.25,
                    tool: "pin".into(),
                    item: ResultItem::new("钉住图片", &attachment.name),
                    command: false,
                    country: None,
                    launch_app: None,
                    passive: false,
                    mode_action: None,
                    open_target: None,
                }];
                return;
            }
        }

        if self.mode.as_deref() == Some("files") {
            let q = self.input.clone();
            self.request_files(q);
            return;
        }
        if let Some(id) = self.mode.clone() {
            if self.input.is_empty() && !matches!(id.as_str(), "help" | "clipboard") {
                self.rows = subcommands(&id)
                    .into_iter()
                    .map(|(sub, label, aliases)| Row {
                        score: 0.98,
                        tool: format!("@{sub}"),
                        item: ResultItem::new(
                            format!("/{}", aliases[0]),
                            format!("固定为{label}模式"),
                        ),
                        command: true,
                        country: None,
                        launch_app: None,
                        passive: false,
                        mode_action: None,
                        open_target: None,
                    })
                    .collect();
                let name = tools::commands()
                    .iter()
                    .find(|command| command.id == id)
                    .map(|command| command.name.as_str())
                    .unwrap_or(&id);
                self.rows.push(Row {
                    score: 0.6,
                    tool: id.clone(),
                    item: ResultItem::new(format!("{name} 模式"), "输入内容后立即处理"),
                    command: false,
                    country: None,
                    launch_app: None,
                    passive: false,
                    mode_action: None,
                    open_target: None,
                });
                return;
            }
            let mut choices = Vec::new();
            if !self.input.starts_with("\\/") {
                for (key, label, aliases) in subcommands(&id) {
                    if self.input.is_empty()
                        || self
                            .input
                            .strip_prefix('/')
                            .is_some_and(|q| aliases.iter().any(|a| a.starts_with(q)))
                    {
                        choices.push(Row {
                            score: 0.99,
                            tool: format!("@{key}"),
                            item: ResultItem::new(
                                format!("/{}", aliases[0]),
                                format!("固定为{label}模式"),
                            ),
                            command: true,
                            country: None,
                            launch_app: None,
                            passive: false,
                            mode_action: None,
                            open_target: None,
                        });
                    }
                }
            }
            if self.input.is_empty() && !choices.is_empty() {
                self.rows = choices;
                return;
            }
            let input = self
                .input
                .strip_prefix("\\/")
                .map(|v| format!("/{v}"))
                .unwrap_or_else(|| self.input.clone());
            if self.system_rows(&id, &input) {
                return;
            }
            match self.execute_tool(&id, &input) {
                Ok(items) => {
                    self.rows = items
                        .into_iter()
                        .map(|item| Row {
                            score: 0.99,
                            tool: id.clone(),
                            item,
                            command: false,
                            country: None,
                            launch_app: None,
                            passive: false,
                            mode_action: None,
                            open_target: None,
                        })
                        .filter(|row| match self.submode.as_deref() {
                            Some("encode") => row.item.title.contains("编码"),
                            Some("decode") => row.item.title.contains("解码"),
                            _ => true,
                        })
                        .collect();
                    choices.append(&mut self.rows);
                    self.rows = choices;
                }
                Err(error) => {
                    self.rows = vec![Row {
                        score: 0.7,
                        tool: id.clone(),
                        item: ResultItem::new(
                            tools::commands()
                                .iter()
                                .find(|command| command.id == id)
                                .map(|command| command.name.as_str())
                                .unwrap_or(&id),
                            error,
                        ),
                        command: false,
                        country: None,
                        launch_app: None,
                        passive: false,
                        mode_action: None,
                        open_target: None,
                    }];
                }
            }
            return;
        }
        if self
            .input
            .trim_matches(quicktool_rust::core::js_space)
            .is_empty()
        {
            let mut recent = self
                .store
                .usage
                .iter()
                .filter(|(_, usage)| usage.last > 0)
                .collect::<Vec<_>>();
            recent.sort_by_key(|item| std::cmp::Reverse(item.1.last));
            self.rows = recent
                .into_iter()
                .filter_map(|(id, _)| tools::commands().iter().find(|c| c.id == *id))
                .filter(|c| modules::is_enabled(&c.id))
                .take(7)
                .map(|c| Row {
                    score: 0.99,
                    tool: c.id.clone(),
                    item: ResultItem::new(&c.name, format!("/{}", c.primary_alias)),
                    command: true,
                    country: None,
                    launch_app: None,
                    passive: false,
                    mode_action: None,
                    open_target: None,
                })
                .collect();
            if self.rows.is_empty() {
                self.rows = tools::commands()
                    .iter()
                    .filter(|command| modules::is_enabled(&command.id))
                    .take(7)
                    .map(|command| Row {
                        score: 0.5,
                        tool: command.id.clone(),
                        item: ResultItem::new(&command.name, format!("/{}", command.primary_alias)),
                        command: true,
                        country: None,
                        launch_app: None,
                        passive: false,
                        mode_action: None,
                        open_target: None,
                    })
                    .collect();
            }
            return;
        }
        if let Some(text) = self.input.strip_prefix('/') {
            if let Some((head, args)) = text.split_once(quicktool_rust::core::js_space) {
                if let Some(c) = tools::command(head) {
                    if args.trim_matches(quicktool_rust::core::js_space).is_empty()
                        && mode_entry(&c.id)
                    {
                        self.rows = vec![Row {
                            score: 0.99,
                            tool: c.id.clone(),
                            item: ResultItem::new(
                                format!("进入 {} 模式", c.name),
                                "输入内容后立即处理",
                            ),
                            command: true,
                            country: None,
                            launch_app: None,
                            passive: false,
                            mode_action: None,
                            open_target: None,
                        }];
                        return;
                    }
                    if c.id == "files" {
                        if modules::is_enabled("files") {
                            let query = args.to_string();
                            self.request_files(query);
                        } else {
                            self.rows = vec![disabled_module_row("files")];
                        }
                        return;
                    }
                    let id = c.id.clone();
                    let args = args.to_string();
                    if self.system_rows(&id, &args) {
                        return;
                    }
                    match self.execute_tool(&id, &args) {
                        Ok(items) => {
                            self.rows = items
                                .into_iter()
                                .map(|item| Row {
                                    score: 0.99,
                                    tool: c.id.clone(),
                                    item,
                                    command: false,
                                    country: None,
                                    launch_app: None,
                                    passive: false,
                                    mode_action: None,
                                    open_target: None,
                                })
                                .collect()
                        }
                        Err(error) => {
                            self.rows = vec![Row {
                                score: 1.,
                                tool: c.id.clone(),
                                item: ResultItem::new(&c.name, error),
                                command: false,
                                country: None,
                                launch_app: None,
                                passive: false,
                                mode_action: None,
                                open_target: None,
                            }];
                        }
                    }
                    return;
                }
            }
            if tools::command(text).is_some_and(|command| command.id == "help") {
                self.system_rows("help", "");
                return;
            }
            if let Some(command) = tools::command(text) {
                let id = command.id.clone();
                if id.starts_with("system:") {
                    // Missing module data leaves no system command to show; a
                    // blank result list must not be the answer.
                    if !self.system_rows(&id, "") {
                        self.rows = vec![disabled_module_row(&id)];
                    }
                    return;
                }
                if mode_entry(&id) {
                    self.rows = vec![Row {
                        score: 1.1,
                        tool: id,
                        item: ResultItem::new(
                            format!("进入 {} 模式", command.name),
                            "Enter 固定为 Tag；Esc 退出",
                        ),
                        command: true,
                        country: None,
                        launch_app: None,
                        passive: false,
                        mode_action: None,
                        open_target: None,
                    }];
                    return;
                }
                if matches!(id.as_str(), "settings" | "history" | "pinwindow") {
                    let (title, value) = match id.as_str() {
                        "settings" => ("打开 QuickTool 设置", "调整外观和快捷键"),
                        "history" => ("打开历史记录", "查看、恢复和复制历史"),
                        _ => (
                            if self.pinned {
                                "取消钉住主窗口"
                            } else {
                                "钉住主窗口"
                            },
                            "切换主窗口钉住状态",
                        ),
                    };
                    self.rows = vec![Row {
                        score: 1.,
                        tool: id,
                        item: ResultItem::new(title, value),
                        command: true,
                        country: None,
                        launch_app: None,
                        passive: false,
                        mode_action: None,
                        open_target: None,
                    }];
                    return;
                }
                self.rows = match self.execute_tool(&id, "") {
                    Ok(items) => items
                        .into_iter()
                        .map(|item| Row {
                            score: 1.,
                            tool: id.clone(),
                            item,
                            command: false,
                            country: None,
                            launch_app: None,
                            passive: false,
                            mode_action: None,
                            open_target: None,
                        })
                        .collect(),
                    Err(_) => vec![Row {
                        score: 1.,
                        tool: id,
                        item: ResultItem::new(&command.name, "输入内容后立即处理"),
                        command: false,
                        country: None,
                        launch_app: None,
                        passive: false,
                        mode_action: None,
                        open_target: None,
                    }],
                };
                return;
            }
            let mut suggestions = tools::suggestions_all(text)
                .into_iter()
                .map(|(c, score)| (c, score + self.store.usage_boost(&c.id)))
                .collect::<Vec<_>>();
            suggestions.sort_by(|a, b| b.1.total_cmp(&a.1));
            self.rows = suggestions
                .into_iter()
                .map(|(c, score)| Row {
                    score,
                    tool: c.id.clone(),
                    item: ResultItem::new(
                        format!(
                            "/{}",
                            tools::best_alias(c, text)
                                .map(|(alias, _)| alias)
                                .unwrap_or(&c.primary_alias)
                        ),
                        &c.name,
                    ),
                    command: true,
                    country: None,
                    launch_app: None,
                    passive: false,
                    mode_action: None,
                    open_target: None,
                })
                .collect();
        } else {
            let mut candidates = tools::detect_scored(&self.input);
            if let Some(_score) = candidates
                .iter()
                .find(|(id, _, _)| id == "country")
                .map(|(_, _, score)| *score)
            {
                candidates.retain(|(id, _, _)| id != "country");
                candidates.extend(country_candidates(&self.input).into_iter().enumerate().map(
                    |(index, country)| {
                        (
                            "country".to_string(),
                            ResultItem::new(
                                country["nameZh"].as_str().unwrap_or(""),
                                country["alpha2"].as_str().unwrap_or(""),
                            ),
                            if index == 0 { 1.15 } else { 0.72 },
                        )
                    },
                ));
            }
            if let Some(score) = candidates
                .iter()
                .find(|(id, _, _)| id == "time")
                .map(|(_, _, score)| *score)
            {
                if let Ok(items) = self.execute_tool("time", &self.input) {
                    let time_count = candidates.iter().filter(|(id, _, _)| id == "time").count();
                    candidates.retain(|(id, _, _)| id != "time");
                    candidates.extend(
                        items
                            .into_iter()
                            .take(time_count)
                            .enumerate()
                            .map(|(i, item)| ("time".into(), item, score - i as f64 * 0.001)),
                    );
                }
            }
            for (id, _, score) in &mut candidates {
                if id != "country" {
                    *score += self.store.usage_boost(id);
                }
            }
            candidates.sort_by(|a, b| b.2.total_cmp(&a.2));
            self.rows = candidates
                .into_iter()
                .map(|(tool, item, score)| Row {
                    score,
                    tool,
                    item,
                    command: false,
                    country: None,
                    launch_app: None,
                    passive: false,
                    mode_action: None,
                    open_target: None,
                })
                .collect();
            let command_rows = tools::suggestions_all(&self.input)
                .into_iter()
                .filter_map(|(command, score)| {
                    let score = score + self.store.usage_boost(&command.id);
                    if score < 0.60 {
                        return None;
                    }
                    let alias = tools::best_alias(command, &self.input)
                        .map(|(alias, _)| alias)
                        .unwrap_or(&command.primary_alias);
                    Some(Row {
                        score: score - 0.06,
                        tool: command.id.clone(),
                        item: ResultItem::new(&command.name, format!("命令 /{alias}")),
                        command: true,
                        country: None,
                        launch_app: None,
                        passive: false,
                        mode_action: None,
                        open_target: None,
                    })
                })
                .collect::<Vec<_>>();
            self.rows.extend(command_rows);
            // An exact command query whose module is uninstalled must still
            // outrank unrelated matches, mirroring the original ranking where
            // every command is installed.
            if let Some(command) = tools::disabled_exact(&self.input) {
                let mut hint = disabled_module_row(&command.id);
                hint.score = tools::best_alias(command, &self.input)
                    .map(|(_, score)| score - 0.06)
                    .unwrap_or(0.94);
                self.rows.push(hint);
            }
            self.rows.sort_by(|a, b| b.score.total_cmp(&a.score));
            if self.screenshot.is_none() {
                self.request_apps();
                self.rows
                    .extend(host_candidates::apps(self, &self.input, true));
                self.rows.sort_by(|a, b| b.score.total_cmp(&a.score));
            }
            let file_query = self.input.trim_matches(quicktool_rust::core::js_space);
            if self.store.settings["fileSearchGlobal"] == true
                && modules::is_enabled("files")
                && self.screenshot.is_none()
                && (2..=128).contains(&file_query.chars().count())
                && quicktool_rust::dates::parse(
                    file_query,
                    self.store.settings["dateOrder"].as_str().unwrap_or("YMD"),
                )
                .is_none()
                && !file_query
                    .chars()
                    .all(|c| c.is_ascii_digit() || "+- */().%".contains(c))
            {
                self.request_files(self.input.clone());
            }
        }
    }
    fn finalize_candidates(&mut self) {
        let explicit = self.input.strip_prefix('/').and_then(|raw| {
            let (head, args) = tools::parse_command_input(raw);
            tools::command(head).map(|command| (command.id.clone(), args.to_string()))
        });
        let active = self
            .mode
            .clone()
            .map(|id| (id, self.input.clone()))
            .or_else(|| explicit.clone());
        if let Some((id, argument)) = &active {
            let name = tools::commands()
                .iter()
                .find(|c| c.id == *id)
                .map(|c| c.name.as_str())
                .unwrap_or(id);
            if self.mode.is_some() && argument.is_empty() && id == "files" {
                self.rows = vec![Row {
                    score: 0.6,
                    tool: id.clone(),
                    item: ResultItem::new(format!("{name} 模式"), "输入内容后立即处理"),
                    command: false,
                    country: None,
                    launch_app: None,
                    passive: true,
                    mode_action: None,
                    open_target: None,
                }];
            } else if self.mode.is_none() && argument.is_empty() && mode_entry(id) {
                self.rows = vec![Row {
                    score: 1.1,
                    tool: id.clone(),
                    item: ResultItem::new(
                        format!("进入 {name} 模式"),
                        "Enter 固定为 Tag；Esc 退出",
                    ),
                    command: true,
                    country: None,
                    launch_app: None,
                    passive: false,
                    mode_action: Some((id.clone(), None)),
                    open_target: None,
                }];
            } else if self.mode.is_none() {
                let (sub, input) = tools::parse_submode(id, argument);
                if let Some(sub) = sub.filter(|_| input.is_empty()) {
                    let label = subcommands(id)
                        .into_iter()
                        .find(|(key, _, _)| *key == sub)
                        .map(|(_, label, _)| label)
                        .unwrap_or("");
                    self.rows = vec![Row {
                        score: 1.2,
                        tool: id.clone(),
                        item: ResultItem::new(format!("进入 {name} · {label}"), "固定模式"),
                        command: true,
                        country: None,
                        launch_app: None,
                        passive: false,
                        mode_action: Some((id.clone(), Some(sub))),
                        open_target: None,
                    }];
                } else if id == "files" {
                    if modules::is_enabled("files") {
                        self.rows = host_candidates::files(self, argument, false);
                    } else {
                        self.rows = vec![disabled_module_row("files")];
                    }
                }
            } else if id == "files" {
                if modules::is_enabled("files") {
                    self.rows = host_candidates::files(self, argument, false);
                } else {
                    self.rows = vec![disabled_module_row("files")];
                }
            }
            if self.mode.is_some()
                && !argument.is_empty()
                && argument.starts_with('/')
                && !argument.contains(' ')
            {
                let (sub, remaining) = tools::parse_submode(id, &argument[1..]);
                if let Some(sub) = sub.filter(|_| remaining.is_empty()) {
                    self.rows.retain(|row| !row.tool.starts_with('@'));
                    if let Some((_, label, aliases)) =
                        subcommands(id).into_iter().find(|(key, _, _)| *key == sub)
                    {
                        self.rows.insert(
                            0,
                            Row {
                                score: 1.2,
                                tool: format!("@{sub}"),
                                item: ResultItem::new(
                                    format!("切换到 {label}"),
                                    format!("/{}", aliases[0]),
                                ),
                                command: true,
                                country: None,
                                launch_app: None,
                                passive: false,
                                mode_action: Some((id.clone(), Some(sub))),
                                open_target: None,
                            },
                        );
                    }
                }
            }
            let mut rank = 0;
            for row in &mut self.rows {
                if let Some(sub) = row.tool.strip_prefix('@') {
                    if row.mode_action.is_none() {
                        row.mode_action = Some((id.clone(), Some(sub.into())));
                    }
                }
                if !row.command
                    && row.item.value != "输入内容后立即处理"
                    && !row.tool.starts_with("system:")
                    && !matches!(
                        row.tool.as_str(),
                        "country" | "winsettings" | "help" | "clipboard"
                    )
                {
                    row.score = (if self.mode.is_some() {
                        0.99
                    } else if argument.is_empty() {
                        1.0
                    } else {
                        1.05
                    }) - rank as f64 * 0.001;
                    rank += 1;
                }
                if id.starts_with("system:") && row.item.value == "输入目标后运行" {
                    row.score = 1.;
                    row.command = true;
                    row.passive = false;
                }
            }
        } else if !self
            .input
            .trim_matches(quicktool_rust::core::js_space)
            .is_empty()
            && !self.input.starts_with('/')
        {
            self.rows.retain(|row| row.tool != "files" || row.command);
            if modules::is_enabled("files") {
                self.rows
                    .extend(host_candidates::files(self, &self.input, true));
            }
        }
        if self.showing_help() {
            for (i, row) in self.rows.iter_mut().enumerate() {
                row.score = 1. - i as f64 * 0.001;
            }
            return;
        }
        if self.mode.is_none()
            && self
                .input
                .trim_matches(quicktool_rust::core::js_space)
                .is_empty()
        {
            for (i, row) in self.rows.iter_mut().enumerate() {
                row.score = 0.5 - i as f64 * 0.01;
            }
        }
        let mut seen = std::collections::HashSet::new();
        self.rows.retain(|row| {
            seen.insert((
                row.tool.clone(),
                row.item.title.clone(),
                row.item.value.clone(),
                row.command,
            ))
        });
        self.rows.sort_by(|a, b| b.score.total_cmp(&a.score));
        self.rows.truncate(16);
    }
    fn execute_tool(&self, id: &str, input: &str) -> Result<Vec<ResultItem>, String> {
        if id == "time" {
            if !modules::is_enabled("time") {
                return Err("时间日期模块未安装，可在设置 → 模块管理中安装".into());
            }
            return quicktool_rust::dates::results(
                input,
                self.store.settings["dateOrder"].as_str().unwrap_or("YMD"),
                self.store.settings["timeCustomFormats"]
                    .as_str()
                    .unwrap_or(""),
            );
        }
        let (sub, input) = if self.mode.is_none() {
            parse_sub_arg(id, input)
        } else {
            (self.submode.as_deref(), input)
        };
        let mut items = tools::execute(id, input)?;
        match sub {
            Some("encode") => items.retain(|r| r.title.contains("编码")),
            Some("decode") => items.retain(|r| r.title.contains("解码")),
            _ => {}
        }
        Ok(items)
    }
    fn request_apps(&mut self) {
        if self.candidate_testing {
            return;
        }
        if !self.apps_requested {
            self.apps_requested = true;
            self.system_backend
                .request(system::Request::Apps { refresh: false });
        }
    }
    fn system_rows(&mut self, id: &str, input: &str) -> bool {
        if let Some(command) = id
            .strip_prefix("system:")
            .and_then(|id| tools::system_commands().iter().find(|c| c.id == id))
        {
            self.rows = vec![Row {
                score: 1.1,
                tool: id.into(),
                item: ResultItem::new(
                    &command.name,
                    if command.needs_arg
                        && input
                            .trim_matches(quicktool_rust::core::js_space)
                            .is_empty()
                    {
                        "输入目标后运行".into()
                    } else if input
                        .trim_matches(quicktool_rust::core::js_space)
                        .is_empty()
                    {
                        command.command.clone()
                    } else {
                        format!("{} {}", command.command, input)
                    },
                ),
                command: false,
                country: None,
                launch_app: None,
                passive: false,
                mode_action: None,
                open_target: None,
            }];
            return true;
        }
        match id {
            "help" => {
                let mut commands = tools::commands()
                    .iter()
                    .filter(|command| command.id != "help")
                    .filter(|command| modules::is_enabled(&command.id))
                    .filter_map(|command| {
                        let score = if input.is_empty() {
                            0.5
                        } else {
                            tools::best_alias(command, input)
                                .map(|(_, score)| score)
                                .unwrap_or(0.)
                                .max(quicktool_rust::search::fuzzy(input, &command.name).score)
                        };
                        (input.is_empty() || score >= 0.58).then_some((command, score))
                    })
                    .collect::<Vec<_>>();
                commands.sort_by(|(a, ascore), (b, bscore)| {
                    help_group_index(&a.id)
                        .cmp(&help_group_index(&b.id))
                        .then_with(|| bscore.total_cmp(ascore))
                });
                self.rows = commands
                    .into_iter()
                    .map(|(command, score)| Row {
                        score,
                        tool: command.id.clone(),
                        item: ResultItem::new(
                            &command.name,
                            format!(
                                "/{}",
                                if command.id == "time" {
                                    "timestamp"
                                } else {
                                    &command.primary_alias
                                }
                            ),
                        ),
                        command: true,
                        country: None,
                        launch_app: None,
                        passive: false,
                        mode_action: None,
                        open_target: None,
                    })
                    .collect();
                true
            }
            "winsettings" => {
                self.rows = tools::windows_settings_search(input, 32)
                    .into_iter()
                    .map(|(item, score)| Row {
                        score,
                        tool: id.into(),
                        item: ResultItem::new(&item.name, &item.command),
                        command: false,
                        country: None,
                        launch_app: None,
                        passive: false,
                        mode_action: None,
                        open_target: None,
                    })
                    .collect();
                true
            }

            "country" => {
                self.country_cache = country_candidates(input);
                self.rows = self
                    .country_cache
                    .iter()
                    .map(|c| Row {
                        score: 0.99,
                        tool: "country".into(),
                        item: ResultItem::new(
                            c["nameZh"].as_str().unwrap_or(""),
                            c["alpha2"].as_str().unwrap_or(""),
                        ),
                        command: false,
                        country: None,
                        launch_app: None,
                        passive: false,
                        mode_action: None,
                        open_target: None,
                    })
                    .collect();
                true
            }
            "run" => {
                self.request_apps();
                self.rows = host_candidates::apps(self, input, false);
                true
            }
            "registry" => {
                if !modules::is_enabled("registry") {
                    self.rows = vec![disabled_module_row(id)];
                    return true;
                }
                if self.registry_query != input {
                    self.registry_query = input.into();
                    self.registry_items.clear();
                    self.registry_due = Some(Instant::now());
                    self.context
                        .request_repaint_after(Duration::from_millis(120));
                }
                self.rows = host_candidates::registry(self, input);
                true
            }
            "cmd" | "powershell" | "terminal" => {
                let (sub, command) = if self.mode.is_none() {
                    parse_sub_arg(id, input)
                } else {
                    (self.submode.as_deref(), input)
                };
                self.rows = vec![Row {
                    score: 0.99,
                    tool: id.into(),
                    item: ResultItem::new(
                        if sub == Some("admin") {
                            format!(
                                "以管理员身份在 {} 运行",
                                tools::commands()
                                    .iter()
                                    .find(|c| c.id == id)
                                    .map(|c| c.name.as_str())
                                    .unwrap_or(id)
                            )
                        } else {
                            format!(
                                "在 {} 运行",
                                tools::commands()
                                    .iter()
                                    .find(|c| c.id == id)
                                    .map(|c| c.name.as_str())
                                    .unwrap_or(id)
                            )
                        },
                        command,
                    ),
                    command: false,
                    country: None,
                    launch_app: None,
                    passive: false,
                    mode_action: None,
                    open_target: None,
                }];
                true
            }
            "open" => {
                if !input
                    .trim_matches(quicktool_rust::core::js_space)
                    .is_empty()
                {
                    self.rows = [
                        (if input.starts_with("http://") || input.starts_with("https://") {
                            "在浏览器打开"
                        } else {
                            "使用系统默认方式打开"
                        }),
                        (if input.starts_with("http://") || input.starts_with("https://") {
                            "复制 URL"
                        } else {
                            "复制路径"
                        }),
                    ]
                    .into_iter()
                    .map(|title| Row {
                        score: 0.99,
                        tool: id.into(),
                        item: ResultItem::new(title, input),
                        command: false,
                        country: None,
                        launch_app: None,
                        passive: false,
                        mode_action: None,
                        open_target: None,
                    })
                    .collect();
                }
                true
            }
            "cd" => {
                let direct = std::path::Path::new(input)
                    .is_dir()
                    .then(|| ("目录".to_string(), std::path::PathBuf::from(input), 2.));
                let mut folders = platform::known_folders()
                    .into_iter()
                    .filter(|(name, _)| {
                        !matches!(
                            name.as_str(),
                            "PublicDesktop" | "Programs" | "CommonPrograms"
                        )
                    })
                    .filter_map(|(name, path)| {
                        let score = quicktool_rust::search::fuzzy(input, folder_label(&name))
                            .score
                            .max(
                                quicktool_rust::search::fuzzy(input, &path.to_string_lossy()).score,
                            );
                        (score > 0.).then_some((name, path, score))
                    })
                    .collect::<Vec<_>>();
                if let Some(direct) = direct {
                    folders.push(direct);
                }
                folders.sort_by(|a, b| b.2.total_cmp(&a.2));
                if let Some((name, path, _)) = folders.first() {
                    let label = folder_label(name).split_whitespace().next().unwrap_or(name);
                    let path = path.to_string_lossy();
                    self.rows = [
                        format!("打开 {label}"),
                        "在 CMD 中打开".into(),
                        "在 PowerShell 中打开".into(),
                        "在 Windows Terminal 中打开".into(),
                        "复制路径".into(),
                    ]
                    .into_iter()
                    .map(|title| Row {
                        score: 0.99,
                        tool: id.into(),
                        item: ResultItem::new(title, path.as_ref()),
                        command: false,
                        country: None,
                        launch_app: None,
                        passive: false,
                        mode_action: None,
                        open_target: None,
                    })
                    .collect();
                }
                true
            }
            "pin" => {
                self.rows = vec![Row {
                    score: 0.99,
                    tool: id.into(),
                    item: ResultItem::new(
                        "钉住文字",
                        if input.is_empty() {
                            "输入文字或粘贴图片后按 Enter"
                        } else {
                            input
                        },
                    ),
                    command: false,
                    country: None,
                    launch_app: None,
                    passive: false,
                    mode_action: None,
                    open_target: None,
                }];
                true
            }
            "clipboard" => {
                if !self.clipboard_loaded && !self.candidate_testing {
                    match platform::clipboard_inspect() {
                        Ok(items) => {
                            self.clipboard_entries = items;
                            self.clipboard_image = platform::clipboard_image().ok();
                        }
                        Err(error) => self.message(error),
                    }
                    if let Some((w, h, bytes)) = &self.clipboard_image {
                        self.icons.insert(
                            "clipboard-image".into(),
                            self.context.load_texture(
                                "clipboard-image",
                                ColorImage::from_rgba_unmultiplied(
                                    [*w as usize, *h as usize],
                                    bytes,
                                ),
                                TextureOptions::LINEAR,
                            ),
                        );
                    }
                    self.clipboard_loaded = true;
                }
                self.rows = if self.clipboard_entries.is_empty() {
                    vec![Row {
                        score: 1.,
                        tool: "clipboard".into(),
                        item: ResultItem::new("读取剪贴板原始数据", "点击或按 Enter 刷新"),
                        command: false,
                        country: None,
                        launch_app: None,
                        passive: false,
                        mode_action: None,
                        open_target: None,
                    }]
                } else {
                    self.clipboard_entries
                        .iter()
                        .take(20)
                        .enumerate()
                        .map(|(i, entry)| {
                            use base64::Engine;
                            let value = if !entry.value.is_empty() {
                                entry.value.clone()
                            } else if !entry.preview.is_empty() {
                                entry.preview.clone()
                            } else if !entry.bytes.is_empty() {
                                base64::engine::general_purpose::STANDARD.encode(&entry.bytes)
                            } else {
                                format!("{} bytes", entry.byte_length)
                            };
                            Row {
                                score: 1. - i as f64 * 0.001,
                                tool: "clipboard".into(),
                                item: ResultItem::new(
                                    format!("{} · {}", entry.kind.to_uppercase(), entry.format),
                                    value,
                                ),
                                command: false,
                                country: None,
                                launch_app: None,
                                passive: false,
                                mode_action: None,
                                open_target: None,
                            }
                        })
                        .collect()
                };
                true
            }
            _ => false,
        }
    }
    fn poll_icons(&mut self) {
        if self.shell_icons.is_none() && self.rows.iter().any(|r| shell_target(r).is_some()) {
            let (tx, rx) = std::sync::mpsc::channel::<String>();
            let (out, responses) = std::sync::mpsc::channel();
            let ctx = self.context.clone();
            std::thread::spawn(move || {
                #[cfg(windows)]
                unsafe {
                    let _ = windows_sys::Win32::System::Com::CoInitializeEx(
                        std::ptr::null(),
                        windows_sys::Win32::System::Com::COINIT_APARTMENTTHREADED as u32,
                    );
                }
                let mut cache = quicktool_rust::shell_icons::IconCache::default();
                while let Ok(target) = rx.recv() {
                    let result = cache.get_target(&target);
                    if out.send((target, result)).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
                #[cfg(windows)]
                unsafe {
                    windows_sys::Win32::System::Com::CoUninitialize();
                }
            });
            self.shell_icons = Some(IconWorker {
                requests: tx,
                responses,
                requested: Default::default(),
                loaded: Default::default(),
            });
        }
        if let Some(worker) = &mut self.shell_icons {
            for row in &self.rows {
                if let Some(target) = shell_target(row) {
                    if worker.requested.insert(target.into()) {
                        let _ = worker.requests.send(target.into());
                    }
                }
            }
            for (target, result) in worker.responses.try_iter() {
                if let Ok(icon) = result {
                    let key = format!("shell:{target}");
                    self.icons.insert(
                        key.clone(),
                        self.context.load_texture(
                            &key,
                            ColorImage::from_rgba_unmultiplied(
                                [icon.width as usize, icon.height as usize],
                                &icon.rgba,
                            ),
                            TextureOptions::LINEAR,
                        ),
                    );
                    worker.loaded.push_back(key);
                }
            }
            while worker.loaded.len() > 256 {
                if let Some(key) = worker.loaded.pop_front() {
                    self.icons.remove(&key);
                    worker.requested.remove(key.trim_start_matches("shell:"));
                }
            }
        }
    }
    fn poll_system(&mut self) {
        if self
            .registry_due
            .is_some_and(|at| at.elapsed() >= Duration::from_millis(120))
        {
            self.registry_due = None;
            self.registry_running_query = self.registry_query.clone();
            let input = &self.registry_query;
            let (root, q) = input
                .split_once(quicktool_rust::core::js_space)
                .filter(|(r, _)| {
                    ["HKCU", "HKLM", "HKCR"].contains(&r.to_ascii_uppercase().as_str())
                })
                .map(|(r, q)| (r.to_ascii_uppercase(), q.to_string()))
                .unwrap_or_else(|| ("all".into(), input.clone()));
            if !q.trim_matches(quicktool_rust::core::js_space).is_empty() {
                self.registry_pending = true;
                self.registry_serial = self.system_backend.request(system::Request::Registry {
                    query: q,
                    limit: 60,
                    root,
                });
            }
        }
        if self.registry_due.is_some() || self.registry_pending {
            self.context
                .request_repaint_after(Duration::from_millis(50));
        }

        let events = self.system_backend.responses.try_iter().collect::<Vec<_>>();
        let mut changed = false;
        for (serial, event) in events {
            match event {
                system::Response::Apps(apps) => {
                    self.apps = apps;
                    changed = true;
                }
                system::Response::Registry(items) => {
                    if serial == self.registry_serial {
                        self.registry_pending = false;
                    }
                    if serial == self.registry_serial
                        && self.registry_running_query == self.registry_query
                    {
                        self.registry_items = items;
                        changed = true;
                    }
                }
            }
        }
        if changed {
            self.rebuild();
        }
    }
    fn index_config(&self) -> (Vec<std::path::PathBuf>, Vec<String>) {
        let roots = self.store.settings["fileIndexRoots"]
            .as_str()
            .unwrap_or("")
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(std::path::PathBuf::from)
            .filter(|p| p.is_absolute())
            .collect::<Vec<_>>();
        let roots = if roots.is_empty() {
            platform::known_folders()
                .into_iter()
                .filter(|(name, _)| matches!(name.as_str(), "Desktop" | "Documents" | "Downloads"))
                .map(|(_, p)| p)
                .filter(|p| p.exists())
                .collect()
        } else {
            roots
        };
        let excludes = self.store.settings["fileIndexExcludes"]
            .as_str()
            .unwrap_or("")
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        (roots, excludes)
    }
    fn request_files(&mut self, query: String) {
        if self.candidate_testing {
            return;
        }
        let query = query
            .trim_matches(quicktool_rust::core::js_space)
            .to_string();
        if self.files_pending && self.file_query.as_deref() == Some(&query) {
            return;
        }
        if self
            .file_results
            .as_ref()
            .is_some_and(|(cached, _)| cached == &query)
            && !self.files_pending
        {
            self.file_query = Some(query);
            return;
        }
        if query
            .trim_matches(quicktool_rust::core::js_space)
            .is_empty()
        {
            return;
        }
        self.files_pending = true;
        self.file_query = Some(query.clone());
        self.file_debounce = Some((query, Instant::now()));
        self.context
            .request_repaint_after(Duration::from_millis(120));
    }
    fn request_files_now(&mut self, query: String) {
        if query
            .trim_matches(quicktool_rust::core::js_space)
            .is_empty()
        {
            return;
        }
        if self.file_index.is_none() {
            let (roots, excludes) = self.index_config();
            let ctx = self.context.clone();
            self.file_index = Some(index::Index::start(
                self.store.root.join("file-index.sqlite"),
                roots,
                excludes,
                move || ctx.request_repaint_of(ViewportId::ROOT),
            ));
        }
        self.files_pending = true;
        self.file_query = Some(query.clone());
        self.file_serial += 1;
        if let Some(index) = &self.file_index {
            if index
                .requests
                .send(index::Request::SearchWithProvider {
                    serial: self.file_serial,
                    query,
                    limit: 40,
                    provider: self.store.settings["fileSearchProvider"]
                        .as_str()
                        .unwrap_or("auto")
                        .to_string(),
                })
                .is_err()
            {
                self.files_pending = false;
                self.message("文件索引服务已停止");
            }
        }
    }
    fn poll_files(&mut self) {
        if self
            .file_debounce
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed() >= Duration::from_millis(120))
        {
            if let Some((query, _)) = self.file_debounce.take() {
                self.request_files_now(query);
            }
        }
        let mut refresh = false;
        let events = self
            .file_index
            .as_ref()
            .map(|index| index.responses.try_iter().collect::<Vec<_>>())
            .unwrap_or_default();
        for event in events {
            match event {
                index::Response::Status(status) => {
                    refresh |= self.file_query.is_some()
                        && !status.building
                        && !status.paused
                        && status.error.is_none();
                    if let Some(error) = &status.error {
                        self.message(error.clone());
                    }
                    self.file_status = status;
                }
                index::Response::Results { serial, items } => {
                    if serial != self.file_serial {
                        continue;
                    }
                    self.files_pending = false;
                    let query = self.file_query.clone().unwrap_or_default();
                    self.file_results = Some((query, items));
                    let selected = self.selected;
                    self.rebuild();
                    self.selected = if selected < self.rows.len() {
                        selected
                    } else {
                        0
                    };
                }
            }
        }
        if refresh {
            if let Some(q) = self.file_query.clone() {
                self.file_results = None;
                self.request_files(q);
            }
        }
    }
    fn used(&mut self, id: &str) {
        if let Err(error) = self.store.mark_used(id) {
            self.message(format!("保存使用记录失败：{error}"));
        }
    }
    fn activate(&mut self, ctx: &Context, index: usize) {
        let Some(row) = self.rows.get(index).cloned() else {
            return;
        };
        if row.tool == "clipboard" && row.item.title == "读取剪贴板原始数据" {
            self.clipboard_loaded = false;
            self.rebuild();
            return;
        }
        if row.passive || (!row.command && row.item.value == "输入内容后立即处理") {
            return;
        }
        // Result rows of an uninstalled module cannot run; guide to module
        // management instead of executing half-alive actions. Inside a mode the
        // rows already carry the module error, so only gate panel-free results.
        if self.mode.is_none() {
            if let Some(info) = modules::module_of(&row.tool) {
                if !modules::is_enabled(&row.tool) {
                    self.draft = self.store.settings.clone();
                    self.modules_page = true;
                    self.panel = Panel::Settings;
                    self.message(format!("请先在模块管理中安装「{}」", info.name));
                    return;
                }
            }
        }
        let used_tool = row.tool.clone();

        if let Some((tool, submode)) = &row.mode_action {
            if tool == "clipboard" {
                self.clipboard_loaded = false;
            }
            self.mode = Some(tool.clone());
            self.submode = submode.clone();
            self.input.clear();
            self.used(tool);
            self.rebuild();
            self.focus = true;
            return;
        }
        if row.command {
            if row.tool.starts_with("system:") {
                if let Some(command) = tools::commands()
                    .iter()
                    .find(|command| command.id == row.tool)
                {
                    self.mode = None;
                    self.submode = None;
                    self.input = format!("/{} ", command.primary_alias);
                    self.rebuild();
                    self.focus = true;
                }
                return;
            }
            if let Some(sub) = row.tool.strip_prefix('@') {
                if let Some(id) = self.mode.clone() {
                    self.used(&id);
                }
                self.submode = Some(sub.into());
                self.input.clear();
                self.rebuild();
                self.focus = true;
                return;
            }
            match row.tool.as_str() {
                "settings" => self.open_settings(),
                "history" => self.panel = Panel::History,
                "pinwindow" => self.toggle_pin(ctx),
                _ => {
                    if row.tool == "clipboard" {
                        self.clipboard_loaded = false;
                    }
                    self.mode = Some(row.tool);
                    self.submode = None;
                    self.input.clear();
                    self.rebuild();
                    self.focus = true;
                }
            }
            self.used(&used_tool);
        } else {
            if row.tool == "base64" && row.item.title.starts_with("Base64 → ") {
                if let Some(image) = self.decoded_image.clone() {
                    let outcome = platform::copy_image(
                        image.rgba.width(),
                        image.rgba.height(),
                        image.rgba.as_raw(),
                    );
                    if outcome.is_err() {
                        ctx.copy_text(image.data_url());
                    }
                    let context = ctx.clone();
                    if let Err(error) = self.store.enqueue_image_with_source(
                        &self.input,
                        &row.tool,
                        &row.item.title,
                        image.rgba,
                        self.mode
                            .as_ref()
                            .map(|id| quicktool_rust::storage::HistoryMode {
                                tool_id: id.clone(),
                                args: vec![],
                                submode: self.submode.clone(),
                            }),
                        Some(quicktool_rust::storage::EncodedImageSource {
                            bytes: image.bytes.clone(),
                            mime: image.mime.clone(),
                        }),
                        move || context.request_repaint_of(ViewportId::ROOT),
                    ) {
                        self.message(format!("保存图片历史失败：{error}"));
                    }
                    self.used("base64");
                    self.message(if outcome.is_ok() {
                        "图片已复制"
                    } else {
                        "图片 Data URL 已复制"
                    });
                }
                return;
            }
            if let Some((country, false)) = &row.country {
                self.input = country_completion(country, &self.input);
                self.rebuild();
                self.focus = true;
                return;
            }
            if row.tool == "pin" {
                let outcome = if let Some(image) = self.attachment.take() {
                    self.pins.create_image(
                        image.rgba.width(),
                        image.rgba.height(),
                        image.rgba.as_raw().clone(),
                    )
                } else if row.item.value != "输入文字或粘贴图片后按 Enter"
                    && !row
                        .item
                        .value
                        .trim_matches(quicktool_rust::core::js_space)
                        .is_empty()
                {
                    self.pins.create_text(row.item.value.clone());
                    Ok(())
                } else {
                    return;
                };
                if let Err(error) = outcome {
                    self.message(error);
                    return;
                }
                self.used("pin");
                self.mode = Some("pin".into());
                self.input.clear();
                self.rebuild();
                self.focus = true;
                self.message("已钉住");
                return;
            }
            if row.tool.starts_with("system:") {
                if row.item.value.is_empty() || row.item.value == "输入目标后运行" {
                    if self.mode.is_none() {
                        self.input = format!(
                            "{} ",
                            self.input.trim_end_matches(quicktool_rust::core::js_space)
                        );
                        self.rebuild();
                        self.focus = true;
                    }
                    return;
                }
                if let Err(error) = system::run_shell("cmd", &row.item.value, None, false) {
                    self.message(error);
                    return;
                }
                self.used(&row.tool);
                if !self.pinned {
                    self.main_visible = false;
                    if let Some(glass) = &mut self.backdrop {
                        let _ = glass.update(false, self.pinned, true, 0., 0, 0.);
                    }
                    ctx.send_viewport_cmd(ViewportCommand::Visible(false));
                }
                let mode = self
                    .mode
                    .as_ref()
                    .map(|id| quicktool_rust::storage::HistoryMode {
                        tool_id: id.clone(),
                        args: vec![],
                        submode: self.submode.clone(),
                    });
                if let Err(error) = self.store.add_with_mode(
                    &self.input,
                    &row.tool,
                    &row.item.title,
                    &row.item.value,
                    mode,
                ) {
                    self.message(format!("保存历史失败：{error}"));
                    return;
                }
                self.message("已打开终端");
                return;
            }
            let copy_path =
                matches!(row.tool.as_str(), "cd" | "open") && row.item.title.starts_with("复制");
            let outcome = if copy_path {
                ctx.copy_text(row.item.value.clone());
                Ok(())
            } else if row.tool == "cd" && row.item.title.starts_with("在 ") {
                let kind = if row.item.title.contains("PowerShell") {
                    "powershell"
                } else if row.item.title.contains("Windows Terminal") {
                    "terminal"
                } else {
                    "cmd"
                };
                system::run_shell(kind, "", Some(&row.item.value), false)
            } else {
                match row.tool.as_str() {
                    "cmd" | "powershell" | "terminal" => system::run_shell(
                        &row.tool,
                        &row.item.value,
                        None,
                        self.submode.as_deref() == Some("admin")
                            || (self.mode.is_none()
                                && self
                                    .input
                                    .split_once(quicktool_rust::core::js_space)
                                    .is_some_and(|(_, args)| {
                                        parse_sub_arg(&row.tool, args).0 == Some("admin")
                                    })),
                    ),
                    "registry" => system::open_registry_at(
                        row.open_target.as_deref().unwrap_or(&row.item.value),
                    ),
                    "clipboard" => {
                        if self
                            .clipboard_entries
                            .get(index)
                            .is_some_and(|entry| entry.kind == "image")
                        {
                            if let Some((w, h, rgba)) = &self.clipboard_image {
                                platform::copy_image(*w, *h, rgba)
                            } else {
                                ctx.copy_text(row.item.value.clone());
                                Ok(())
                            }
                        } else {
                            ctx.copy_text(row.item.value.clone());
                            Ok(())
                        }
                    }
                    "run" => {
                        if let Some(app) = row
                            .launch_app
                            .as_ref()
                            .or_else(|| self.apps.iter().find(|a| a.target == row.item.value))
                        {
                            system::launch_app(app, false)
                        } else {
                            platform::open(&row.item.value, None, None, false)
                        }
                    }
                    "files" | "cd" | "open" | "winsettings" => {
                        platform::open(&row.item.value, None, None, false)
                    }
                    "pin" => {
                        if !self.input.is_empty() {
                            self.pins.create_text(self.input.clone());
                        }
                        Ok(())
                    }
                    _ => {
                        ctx.copy_text(row.item.value.clone());
                        Ok(())
                    }
                }
            };
            if let Err(error) = outcome {
                self.message(error);
                return;
            }
            self.used(&used_tool);
            if is_launch(&row.tool) && !copy_path && !self.pinned {
                self.main_visible = false;
                if let Some(glass) = &mut self.backdrop {
                    let _ = glass.update(false, self.pinned, true, 0., 0, 0.);
                }
                ctx.send_viewport_cmd(ViewportCommand::Visible(false));
            }
            let image_source = if row.tool == "clipboard"
                && self
                    .clipboard_entries
                    .get(index)
                    .is_some_and(|entry| entry.kind == "image")
            {
                self.clipboard_image
                    .as_ref()
                    .and_then(|(w, h, bytes)| image::RgbaImage::from_raw(*w, *h, bytes.clone()))
                    .map(std::sync::Arc::new)
            } else {
                None
            };
            if let Some(source) = image_source {
                let context = ctx.clone();
                if let Err(error) = self.store.enqueue_image_with_mode(
                    &self.input,
                    &row.tool,
                    &row.item.title,
                    source,
                    self.mode
                        .as_ref()
                        .map(|id| quicktool_rust::storage::HistoryMode {
                            tool_id: id.clone(),
                            args: vec![],
                            submode: self.submode.clone(),
                        }),
                    move || context.request_repaint_of(ViewportId::ROOT),
                ) {
                    self.message(format!("保存图片历史失败：{error}"));
                } else {
                    self.message("图片已复制");
                }
                return;
            }
            if let Err(e) = self.store.add_with_mode(
                &self.input,
                &row.tool,
                &row.item.title,
                &row.item.value,
                self.mode
                    .as_ref()
                    .map(|tool_id| quicktool_rust::storage::HistoryMode {
                        tool_id: tool_id.clone(),
                        args: self
                            .submode
                            .as_ref()
                            .map(|id| vec![serde_json::json!({"id":id,"label":sub_label(id)})])
                            .unwrap_or_default(),
                        submode: self.submode.clone(),
                    }),
            ) {
                self.message(format!("保存历史失败：{e}"));
            } else {
                self.message(if is_launch(&row.tool) {
                    "已执行"
                } else {
                    "已复制"
                });
            }
        }
    }
    fn toggle_pin(&mut self, ctx: &Context) {
        self.pinned = !self.pinned;
        ctx.send_viewport_cmd(ViewportCommand::WindowLevel(
            if self.pinned || self.store.settings["summonAlwaysOnTop"] == true {
                WindowLevel::AlwaysOnTop
            } else {
                WindowLevel::Normal
            },
        ));
    }
    fn show_main(&mut self, ctx: &Context) {
        self.main_visible = true;
        self.place_requested = true;
        if self.panel == Panel::Settings {
            self.close_settings();
        } else {
            self.panel = Panel::None;
        }
        self.select_search = true;
        // A tray click can arrive with the previous frame's focus-loss event.
        // The pending Focus command takes effect after this frame; do not let
        // hide-on-blur cancel the explicit show in the meantime.
        self.was_focused = false;
        for command in [
            ViewportCommand::Minimized(false),
            ViewportCommand::Visible(true),
            ViewportCommand::Focus,
        ] {
            ctx.send_viewport_cmd_to(ViewportId::ROOT, command);
        }
        if self.pinned || self.store.settings["summonAlwaysOnTop"] == true {
            ctx.send_viewport_cmd_to(
                ViewportId::ROOT,
                ViewportCommand::WindowLevel(WindowLevel::AlwaysOnTop),
            );
        }
        self.focus = true;
    }
    fn shortcut_preview(&mut self, shortcut: &str) -> Result<(), String> {
        if self.registered_shortcut == shortcut && self.hotkey.is_some() {
            return Ok(());
        }
        self.hotkey = None;
        self.registered_shortcut.clear();
        let c = self.context.clone();
        match platform::hotkey(shortcut, move || c.request_repaint_of(ViewportId::ROOT)) {
            Ok(h) => {
                self.hotkey = Some(h);
                self.registered_shortcut = shortcut.into();
                Ok(())
            }
            Err(error) => Err(error),
        }
    }
    fn restore_shortcut(&mut self) {
        self.recording_shortcut = false;
        let shortcut = self.store.settings["globalShortcut"]
            .as_str()
            .unwrap_or("Alt+Space")
            .to_string();
        if let Err(error) = self.shortcut_preview(&shortcut) {
            self.message(format!("恢复快捷键失败：{error}"));
        }
    }
    fn open_settings(&mut self) {
        self.draft = self.store.settings.clone();
        self.modules_page = false;
        self.panel = Panel::Settings;
    }
    fn close_settings(&mut self) {
        if self.recording_shortcut {
            self.restore_shortcut();
        }
        if self.draft != self.store.settings {
            self.confirm_discard = true
        } else {
            self.modules_page = false;
            self.panel = Panel::None;
            self.focus = true;
        }
    }
    fn switch_tab(&mut self, tab: usize) {
        self.tab = tab;
        self.scroll_reset = true;
        self.last_scroll_selection = None;
        self.selected = self.visible_indices().first().copied().unwrap_or(0);
        self.focus = true;
    }
    fn blur_hides(&self, ctx: &Context) -> bool {
        !self.pinned
            && self.panel != Panel::Settings
            && !self.recording_shortcut
            && !self.date_picker
            && !self.color_picker
            && !self.native_color.open
            && !widgets::is_open(ctx)
            && self.store.settings["hideOnBlur"] == true
    }
    fn keyboard(&mut self, ctx: &Context) {
        if self.eyedrop.active() || self.eyedrop_pending > 0 {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL | Modifiers::SHIFT, Key::Tab)) {
            self.switch_tab((self.tab + 2) % 3);
        } else if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::Tab)) {
            self.switch_tab((self.tab + 1) % 3);
        }
        let visible = self.visible_indices();
        if !visible.contains(&self.selected) {
            self.selected = visible.first().copied().unwrap_or(0);
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            if self.native_color.open {
                self.native_color.open = false;
            } else if self.color_picker {
                self.color_picker = false;
            } else if self.date_picker {
                self.date_picker = false;
            } else if self.confirm_discard {
                self.confirm_discard = false;
            } else if self.panel == Panel::Settings {
                // The module page is a drill-down layer; Esc walks back first.
                if self.modules_page {
                    self.modules_page = false;
                } else {
                    self.close_settings()
                }
            } else if self.panel == Panel::History {
                self.panel = Panel::None;
                self.focus = true;
            } else if self.mode.is_some() && !self.input.is_empty() {
                self.input.clear();
                self.rebuild();
            } else if self.submode.take().is_some() || self.mode.take().is_some() {
                self.rebuild();
            } else {
                self.main_visible = false;
                if let Some(glass) = &mut self.backdrop {
                    let _ = glass.update(false, self.pinned, true, 0., 0, 0.);
                }
                ctx.send_viewport_cmd(ViewportCommand::Visible(false));
            }
        }
        if self.date_picker || self.color_picker || self.native_color.open {
            return;
        }
        if !self.recording_shortcut
            && ctx.input_mut(|i| i.consume_key(Modifiers::CTRL | Modifiers::SHIFT, Key::P))
        {
            self.toggle_pin(ctx);
        }
        if self.panel == Panel::None {
            if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(Key::V)) && clipboard_has_image() {
                self.load_attachment(attachments::Attachment::from_clipboard);
                ctx.input_mut(|i| i.events.retain(|e| !matches!(e, Event::Paste(_))));
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown))
                && !self.rows.is_empty()
            {
                let visible = self.visible_indices();
                if !visible.is_empty() {
                    let at = visible
                        .iter()
                        .position(|i| *i == self.selected)
                        .unwrap_or(0);
                    self.selected = visible[(at + 1).min(visible.len() - 1)];
                }
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp))
                && !self.rows.is_empty()
            {
                let visible = self.visible_indices();
                if !visible.is_empty() {
                    let at = visible
                        .iter()
                        .position(|i| *i == self.selected)
                        .unwrap_or(0);
                    self.selected = visible[at.saturating_sub(1)];
                }
            }
            if self.completion().is_some()
                && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Backspace))
            {
                self.completion_suppressed = Some(self.input.clone());
            }
            if self.tab_completion().is_some()
                && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab))
            {
                if let Some(completion) = self.tab_completion() {
                    self.input = completion;
                    self.completion_suppressed = Some(self.input.clone());
                    self.rebuild();
                    self.focus = true;
                }
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
                self.activate(ctx, self.selected);
            }
        }
    }
    fn tab_completion(&self) -> Option<String> {
        if self.showing_help()
            || self
                .input
                .trim_matches(quicktool_rust::core::js_space)
                .is_empty()
            || self.completion_suppressed.as_deref() == Some(&self.input)
        {
            return None;
        }
        if let Some(ghost) = self.completion() {
            return Some(format!("{ghost} "));
        }
        let row = self.rows.get(self.selected)?;
        if !row.command || row.tool.starts_with('@') || self.mode.is_some() {
            return None;
        }
        let command = tools::commands()
            .iter()
            .find(|command| command.id == row.tool)?;
        let token = self.input.trim_start_matches('/');
        let alias = tools::best_alias(command, token)
            .map(|(alias, _)| alias)
            .unwrap_or(&command.primary_alias);
        let full = format!("/{alias} ");
        (full.trim_matches(quicktool_rust::core::js_space)
            != self.input.trim_matches(quicktool_rust::core::js_space))
        .then_some(full)
    }
    fn completion(&self) -> Option<String> {
        if self.completion_suppressed.as_deref() == Some(&self.input)
            || !self.input.starts_with('/')
            || self.input.contains(' ')
        {
            return None;
        }
        let token = &self.input[1..];
        if let Some(mode) = &self.mode {
            return subcommands(mode)
                .into_iter()
                .flat_map(|(_, _, a)| a)
                .find(|a| a.starts_with(token) && a.len() > token.len())
                .map(|a| format!("/{a}"));
        }
        tools::suggestions_all(token)
            .into_iter()
            .flat_map(|(c, _)| c.aliases.iter())
            .find(|a| a.starts_with(token) && a.len() > token.len())
            .map(|a| format!("/{a}"))
    }
    fn search_context(&mut self, ui: &Ui, response: &Response, ctx: &Context) {
        if self.large_input.active() {
            if let Some(action) = widgets::context(
                ui,
                response,
                "large-search-editor",
                &[
                    ("undo", "撤销\tCtrl+Z", true),
                    ("redo", "重做\tCtrl+Y", true),
                    ("sep1", "", false),
                    (
                        "cut",
                        "剪切\tCtrl+X",
                        !self.large_input.selection().is_empty(),
                    ),
                    (
                        "copy",
                        "复制\tCtrl+C",
                        !self.large_input.selection().is_empty(),
                    ),
                    ("paste", "粘贴\tCtrl+V", true),
                    ("sep2", "", false),
                    ("all", "全选\tCtrl+A", true),
                ],
            ) {
                if self.large_input.action(&mut self.input, ctx, &action) {
                    self.rebuild();
                }
                self.focus = true;
            }
        }
    }

    fn mode_tag(&self, ui: &mut Ui, label: &str, icon: Option<&str>) -> bool {
        let mut close = false;
        Frame::new()
            .fill(surface(ui, 0.02))
            .stroke(Stroke::new(
                1.,
                if ui.visuals().dark_mode {
                    Color32::from_rgb(42, 47, 60)
                } else {
                    Color32::from_rgb(215, 222, 235)
                },
            ))
            .corner_radius(9)
            .inner_margin(Margin {
                left: 9,
                right: 5,
                top: 3,
                bottom: 3,
            })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 5.;
                    ui.set_min_height(22.);
                    if let Some(texture) = icon.and_then(|icon| self.icons.get(icon)) {
                        ui.add(
                            Image::new(texture)
                                .fit_to_exact_size(vec2(14., 14.))
                                .tint(ACCENT),
                        );
                    }
                    ui.label(
                        RichText::new(label)
                            .font(FontId::new(11.5, FontFamily::Name("UiTag".into())))
                            .color(if ui.visuals().dark_mode {
                                Color32::from_rgb(184, 192, 208)
                            } else {
                                Color32::from_rgb(71, 81, 101)
                            }),
                    );
                    let button = if let Some(texture) = self.icons.get("mdi:close") {
                        Button::image(
                            Image::new(texture)
                                .fit_to_exact_size(vec2(13., 13.))
                                .tint(weak_color(ui)),
                        )
                    } else {
                        Button::new("×")
                    };
                    close = ui
                        .add_sized([20., 20.], button.frame(false))
                        .native_tooltip(&format!("退出 {label}"))
                        .clicked();
                });
            });
        close
    }
    fn date_button(&self, ui: &mut Ui) -> Response {
        let button = if let Some(texture) = self
            .icons
            .get("mdi:calendar-month-outline")
            .or_else(|| self.icons.get("mdi:calendar-clock"))
        {
            Button::image(
                Image::new(texture)
                    .fit_to_exact_size(vec2(18., 18.))
                    .tint(weak_color(ui)),
            )
        } else {
            Button::new("▦")
        };
        ui.add_sized([30., 30.], button.corner_radius(8).fill(surface(ui, 0.045)))
            .native_tooltip("选择日期和时间")
    }
    fn topbar(&mut self, ui: &mut Ui, ctx: &Context) {
        ui.spacing_mut().item_spacing.x = 8.;
        ui.horizontal(|ui| {
            let width = (ui.available_width() - 150.).max(100.);
            let search_frame = Frame::new()
                .fill(if ui.visuals().dark_mode {
                    Color32::from_rgb(23, 27, 37)
                } else {
                    Color32::from_rgb(249, 250, 254)
                })
                .stroke(Stroke::new(
                    1.,
                    if ui.visuals().dark_mode {
                        Color32::from_rgb(49, 59, 80)
                    } else {
                        Color32::from_rgb(188, 202, 231)
                    },
                ))
                .corner_radius(14)
                .inner_margin(Margin::symmetric(13, 8))
                .show(ui, |ui| {
                    ui.set_width(width - 28.);
                    ui.set_min_height(32.);
                    ui.horizontal_centered(|ui| {
                        self.icon(ui, "mdi:magnify", 20.);
                        if let Some(mode) = self.mode.clone() {
                            if let Some(command) =
                                tools::commands().iter().find(|command| command.id == mode)
                            {
                                if self.mode_tag(ui, &command.name, Some(&command.icon)) {
                                    self.mode = None;
                                    self.submode = None;
                                    self.rebuild();
                                    self.focus = true;
                                }
                            }
                        }
                        if let Some(attachment) = &self.attachment {
                            let name = attachment.name.chars().take(18).collect::<String>();
                            let mut remove = false;
                            Frame::new()
                                .fill(selected_surface(ui))
                                .corner_radius(7)
                                .inner_margin(4)
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        if let Some(texture) = self.icons.get("input-attachment") {
                                            ui.add(Image::new(texture).max_size(vec2(22., 22.)));
                                        }
                                        ui.add(
                                            Label::new(RichText::new(name).size(10.)).truncate(),
                                        );
                                        if ui.small_button("×").native_tooltip("移除图片").clicked()
                                        {
                                            remove = true;
                                        }
                                    });
                                });
                            if remove {
                                self.attachment = None;
                                self.rebuild();
                                self.focus = true;
                            }
                        }
                        if let Some(sub) = self.submode.clone() {
                            if self.mode_tag(ui, sub_label(&sub), None) {
                                self.submode = None;
                                self.rebuild();
                                self.focus = true;
                            }
                        }
                        let completion = self.completion();
                        let show_tab = self.tab_completion().is_some();
                        let date = self.mode.as_deref() == Some("time")
                            || (self.mode.is_none()
                                && (self.input.starts_with("/时间")
                                    || self.input.starts_with("/time")
                                    || quicktool_rust::dates::parse(
                                        &self.input,
                                        self.store.settings["dateOrder"].as_str().unwrap_or("YMD"),
                                    )
                                    .is_some()));
                        let reserve = if show_tab { 32. } else { 0. } + if date { 30. } else { 0. };
                        let w = (ui.available_width() - reserve).max(25.);
                        if self.select_search {
                            self.large_input.action(&mut self.input, ctx, "all");
                            self.select_search = false;
                        }
                        let r = self.large_input.show(ui, &mut self.input, vec2(w, 32.));
                        if let Some(full) = completion {
                            let prefix = ui
                                .painter()
                                .layout_no_wrap(
                                    self.input.clone(),
                                    FontId::proportional(16.),
                                    Color32::TRANSPARENT,
                                )
                                .size()
                                .x;
                            let suffix = full.strip_prefix(&self.input).unwrap_or("");
                            let at = pos2(r.rect.left() + prefix, r.rect.center().y - 8.);
                            ui.painter().text(
                                at,
                                Align2::LEFT_TOP,
                                suffix,
                                FontId::proportional(16.),
                                Color32::from_white_alpha(65),
                            );
                        }
                        if date && self.date_button(ui).clicked() {
                            self.date_picker = !self.date_picker;
                            self.native_date.reset();
                            let raw = if self.input.starts_with('/') {
                                self.input
                                    .split_once(quicktool_rust::core::js_space)
                                    .map(|(_, args)| args)
                                    .unwrap_or(&self.input)
                            } else {
                                &self.input
                            };
                            if let Some(parsed) = quicktool_rust::dates::parse(
                                raw,
                                self.store.settings["dateOrder"].as_str().unwrap_or("YMD"),
                            ) {
                                let d = parsed.date;
                                self.date_value = format!(
                                    "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                                    d.year(),
                                    d.month() as u8,
                                    d.day(),
                                    d.hour(),
                                    d.minute(),
                                    d.second()
                                );
                            }
                        }
                        if show_tab {
                            keycap(ui, "Tab");
                        }
                        if self.focus {
                            r.request_focus();
                            self.focus = false;
                        }
                        if r.changed() {
                            self.completion_suppressed = None;
                            self.rebuild();
                        }
                        self.search_context(ui, &r, ctx);
                    });
                });
            if ctx.memory(|memory| memory.has_focus(Id::new("quicktool-search"))) {
                ui.painter().rect_stroke(
                    search_frame.response.rect.expand(2.),
                    CornerRadius::same(16),
                    Stroke::new(
                        3.,
                        if ui.visuals().dark_mode {
                            Color32::from_rgb(28, 33, 46)
                        } else {
                            Color32::from_rgb(229, 235, 248)
                        },
                    ),
                    StrokeKind::Outside,
                );
            }
            if self
                .icon_button(
                    ui,
                    "mdi:pin-outline",
                    "钉起窗口 (Ctrl+Shift+P)",
                    self.pinned,
                )
                .clicked()
            {
                self.toggle_pin(ctx);
            }
            if self
                .icon_button(ui, "mdi:history", "历史记录", false)
                .clicked()
            {
                self.panel = Panel::History;
            }
            if self
                .icon_button(ui, "mdi:cog-outline", "设置", false)
                .clicked()
            {
                self.open_settings();
            }
        });
    }
    fn results(&mut self, ui: &mut Ui, ctx: &Context) {
        ui.visuals_mut().clip_rect_margin = 0.;
        ui.spacing_mut().item_spacing.y = 0.;
        let (tabs_rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), 38.), Sense::hover());
        for (i, (label, icon)) in [
            ("全部", "mdi:view-grid-outline"),
            ("运行", "mdi:play-circle-outline"),
            ("工具", "mdi:wrench-outline"),
        ]
        .into_iter()
        .enumerate()
        {
            let rect = Rect::from_min_size(
                tabs_rect.min + vec2(6. + i as f32 * 69., 1.),
                vec2(64., 30.),
            );
            let response = ui.interact(rect, ui.id().with(("tab", i)), Sense::click());
            if self.tab == i {
                ui.painter().rect_filled(rect, 9, selected_surface(ui));
                ui.painter().rect_stroke(
                    rect,
                    9,
                    Stroke::new(0.8, Color32::from_rgba_unmultiplied(155, 188, 255, 40)),
                    StrokeKind::Inside,
                );
            } else if response.hovered() {
                ui.painter().rect_filled(rect, 9, surface(ui, 0.05));
            }
            let tint = if self.tab == i {
                ui.visuals().text_color()
            } else {
                Color32::from_gray(130)
            };
            if let Some(texture) = self.icons.get(icon) {
                ui.painter().image(
                    texture.id(),
                    Rect::from_min_size(rect.min + vec2(10., 7.5), vec2(15., 15.)),
                    Rect::from_min_max(Pos2::ZERO, pos2(1., 1.)),
                    tint,
                );
            }
            ui.painter().text(
                rect.left_center() + vec2(31., 0.),
                Align2::LEFT_CENTER,
                label,
                FontId::new(11., FontFamily::Name("UiMedium".into())),
                tint,
            );
            if response.clicked() {
                self.switch_tab(i);
            }
        }
        let hint = pos2(tabs_rect.right() - 7., tabs_rect.top() + 16.);
        ui.painter().text(
            hint,
            Align2::RIGHT_CENTER,
            "切换分类",
            FontId::proportional(10.),
            weak_color(ui),
        );
        for (name, x, width) in [("Ctrl", -103., 26.5), ("Tab", -71.5, 27.)] {
            let rect = Rect::from_min_size(hint + vec2(x, -9.), vec2(width, 18.));
            ui.painter().rect_filled(rect, 5, surface(ui, 0.025));
            ui.painter().rect_stroke(
                rect,
                5,
                Stroke::new(0.8, Color32::from_rgb(45, 49, 60)),
                StrokeKind::Inside,
            );
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                name,
                FontId::new(9., FontFamily::Name("Jura".into())),
                weak_color(ui),
            );
        }
        ui.painter().hline(
            tabs_rect.x_range(),
            tabs_rect.bottom(),
            Stroke::new(0.8, Color32::from_white_alpha(12)),
        );
        let (context_rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), 36.), Sense::hover());
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(Rect::from_min_max(
                    context_rect.min,
                    context_rect.max - vec2(0., 5.),
                ))
                .layout(Layout::left_to_right(Align::Center)),
            |ui| {
                ui.horizontal(|ui| {
                    ui.set_max_width(ui.available_width() - 7.);
                    ui.add_space(7.);
                    ui.label(
                        RichText::new(if let Some(id) = &self.mode {
                            tools::commands()
                                .iter()
                                .find(|c| c.id == *id)
                                .map(|c| format!("{} 模式", c.name))
                                .unwrap_or_else(|| "工具模式".into())
                        } else if self
                            .input
                            .trim_matches(quicktool_rust::core::js_space)
                            .is_empty()
                        {
                            "最近使用".into()
                        } else if self.input.starts_with('/') {
                            "命令候选".into()
                        } else if self.visible_indices().len() > 1 {
                            format!("已筛选 {} 个高相关结果", self.visible_indices().len())
                        } else {
                            "最佳匹配".into()
                        })
                        .size(11.)
                        .color(weak_color(ui)),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new("逐层退出").size(10.).color(weak_color(ui)));
                        keycap(ui, "Esc");
                        ui.label(RichText::new("执行/复制").size(10.).color(weak_color(ui)));
                        keycap(ui, "Enter");
                        ui.label(RichText::new("选择").size(10.).color(weak_color(ui)));
                        keycap(ui, "↑↓");
                    });
                });
            },
        );
        ui.advance_cursor_after_rect(context_rect);
        ui.painter().hline(
            context_rect.x_range(),
            context_rect.bottom(),
            Stroke::new(0.8, Color32::from_white_alpha(14)),
        );
        ui.spacing_mut().item_spacing.y = 3.;

        self.layout_snapshot = serde_json::json!({"selectedTop":ui.cursor().top()});
        if self.showing_help() {
            self.help_cards(ui);
            return;
        }
        let raw_date = if self.input.starts_with('/') {
            self.input
                .split_once(quicktool_rust::core::js_space)
                .map(|(_, args)| args)
                .unwrap_or(&self.input)
        } else {
            &self.input
        };
        let date_parsed = if self
            .rows
            .iter()
            .any(|row| row.tool == "time" && !row.command)
        {
            quicktool_rust::dates::parse(
                raw_date,
                self.store.settings["dateOrder"].as_str().unwrap_or("YMD"),
            )
        } else {
            None
        };
        let visible = self.visible_indices();
        let body_top = ui.cursor().top();
        let mut selected_top = None;
        let mut row_rects = Vec::new();
        let reset_scroll = self.scroll_reset;
        if reset_scroll {
            eframe::egui::scroll_area::State::default()
                .store(ctx, ui.make_persistent_id("main-results"));
            self.result_scroll = 0.;
            self.scroll_reset = false;
            self.last_scroll_selection = Some(self.selected);
        }
        let reveal_selection = !reset_scroll
            && self.last_scroll_selection != Some(self.selected)
            && (self.motion.visual - self.motion.target).abs() < 0.7;
        if self.frames == 8 && std::env::args().any(|a| a == "--layout-debug") {
            eprintln!("layout tabs={tabs_rect:?} context={context_rect:?} body={body_top}");
        }
        ui.visuals_mut().widgets.inactive.bg_fill = Color32::from_rgb(54, 57, 66);
        ui.visuals_mut().widgets.hovered.bg_fill = Color32::from_rgb(69, 73, 84);
        ui.visuals_mut().widgets.active.bg_fill = Color32::from_rgb(85, 91, 105);
        // The original results scrollbar has a transparent track.
        ui.visuals_mut().extreme_bg_color = Color32::TRANSPARENT;
        let output = ScrollArea::vertical()
            .max_height(478.)
            .id_salt("main-results")
            .vertical_scroll_offset(self.result_scroll)
            .scroll_bar_visibility(eframe::egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                let mut action = None;
                let mut context_action = None;
                let mut pin_image = None;
                let mut color_action = None;
                if visible.is_empty() {
                    ui.add_space(30.);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(if self.files_pending {
                                "正在搜索文件…"
                            } else if self.mode.is_some() {
                                "输入内容后立即处理"
                            } else {
                                "没有匹配结果"
                            })
                            .size(13.)
                            .color(weak_color(ui)),
                        );
                    });
                }
                ui.spacing_mut().item_spacing.y = 0.;
                if self.tab == 0
                    && !self
                        .input
                        .trim_matches(quicktool_rust::core::js_space)
                        .is_empty()
                {
                    ui.add_space(9.);
                    ui.allocate_ui_with_layout(
                        vec2(ui.available_width(), 28.),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            ui.add_space(12.);
                            self.icon(ui, "mdi:star-outline", 14.);
                            ui.label(
                                RichText::new("最佳匹配")
                                    .font(FontId::new(10., FontFamily::Name("UiStrong".into())))
                                    .color(weak_color(ui)),
                            );
                        },
                    );
                }
                for (n, &index) in visible.iter().enumerate() {
                    if self.tab == 0
                        && !self
                            .input
                            .trim_matches(quicktool_rust::core::js_space)
                            .is_empty()
                        && n > 0
                        && (n == 1
                            || self.row_is_launch(&self.rows[visible[n - 1]])
                                != self.row_is_launch(&self.rows[index]))
                    {
                        ui.add_space(11.);
                        ui.horizontal(|ui| {
                            let launch = self.row_is_launch(&self.rows[index]);
                            self.icon(
                                ui,
                                if launch {
                                    "mdi:play-circle-outline"
                                } else {
                                    "mdi:wrench-outline"
                                },
                                12.,
                            );
                            ui.label(
                                RichText::new(if launch { "运行" } else { "工具" })
                                    .size(10.)
                                    .color(weak_color(ui)),
                            );
                        });
                        ui.add_space(8.);
                    }
                    if let Some((country, true)) = self.rows[index].country.clone() {
                        let response = country_card::show(self, ui, ctx, &country, index);
                        if index == self.selected {
                            selected_top = Some(response.rect.top());
                            if reveal_selection {
                                response.scroll_to_me(None);
                            }
                        }
                        if response.clicked() {
                            self.selected = index;
                        }
                        continue;
                    }
                    let mut swatch_rect = None;
                    let row = &self.rows[index];
                    let selected = index == self.selected;
                    let command = tools::commands().iter().find(|c| c.id == row.tool);
                    let r = Frame::new()
                        .fill(if selected {
                            selected_surface(ui)
                        } else {
                            Color32::TRANSPARENT
                        })
                        .stroke(Stroke::new(
                            1.,
                            if selected {
                                if ui.visuals().dark_mode {
                                    Color32::from_rgb(48, 53, 76)
                                } else {
                                    Color32::from_rgb(196, 211, 239)
                                }
                            } else {
                                Color32::TRANSPARENT
                            },
                        ))
                        .corner_radius(13)
                        .inner_margin(Margin::symmetric(11, 8))
                        .show(ui, |ui| {
                            ui.set_min_height(44.);
                            ui.add_space(4.);
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                Frame::new()
                                    .fill(surface(ui, if selected { 0.07 } else { 0.05 }))
                                    .corner_radius(10)
                                    .inner_margin(3)
                                    .show(ui, |ui| {
                                        // Match the original fixed 34px icon container. Native
                                        // icons arrive asynchronously and must not resize a row.
                                        ui.set_min_size(vec2(28., 28.));
                                        ui.set_max_size(vec2(28., 28.));
                                        let mut icon_slot = ui.new_child(
                                            UiBuilder::new()
                                                .max_rect(Rect::from_min_size(
                                                    ui.cursor().min,
                                                    vec2(28., 28.),
                                                ))
                                                .layout(Layout::centered_and_justified(
                                                    Direction::TopDown,
                                                )),
                                        );
                                        let ui = &mut icon_slot;
                                        if let Some(texture) =
                                            shell_target(row).and_then(|target| {
                                                self.icons.get(&format!("shell:{target}"))
                                            })
                                        {
                                            ui.add(Image::new(texture).max_size(vec2(28., 28.)));
                                        } else if row.tool == "base64"
                                            && row.item.title.starts_with("Base64 → ")
                                        {
                                            if let Some(texture) = self.icons.get("decoded-image") {
                                                ui.add(
                                                    Image::new(texture).max_size(vec2(28., 28.)),
                                                );
                                            }
                                        } else if row.tool == "clipboard"
                                            && row.item.title.starts_with("IMAGE")
                                        {
                                            if let Some(texture) = self.icons.get("clipboard-image")
                                            {
                                                ui.add(
                                                    Image::new(texture).max_size(vec2(34., 34.)),
                                                );
                                            }
                                        } else {
                                            let icon = command
                                                .map(|c| c.icon.as_str())
                                                .unwrap_or("mdi:information-outline");
                                            if selected {
                                                if let Some(texture) = self.icons.get(icon) {
                                                    ui.add(
                                                        Image::new(texture)
                                                            .fit_to_exact_size(vec2(18., 18.))
                                                            .tint(Color32::from_rgb(155, 164, 255)),
                                                    );
                                                } else {
                                                    self.icon(ui, icon, 18.);
                                                }
                                            } else {
                                                self.icon(ui, icon, 18.);
                                            }
                                        }
                                    });
                                ui.add_space(4.);
                                ui.vertical(|ui| {
                                    ui.set_max_width(ui.available_width() - 38.);
                                    let action = row.command
                                        || row.item.value == "输入内容后立即处理"
                                        || (is_launch(&row.tool)
                                            && !row.item.title.starts_with("复制"));
                                    let primary = if action {
                                        &row.item.title
                                    } else {
                                        &row.item.value
                                    };
                                    let primary = clip_text(primary, 96);
                                    ui.horizontal(|ui| {
                                        if row.tool == "color" {
                                            if let Some(color) =
                                                quicktool_rust::colors::candidates(&row.item.value)
                                                    .first()
                                                    .copied()
                                            {
                                                let (rect, _) = ui.allocate_exact_size(
                                                    vec2(15., 15.),
                                                    Sense::hover(),
                                                );
                                                ui.painter().rect_filled(
                                                    rect,
                                                    CornerRadius::same(4),
                                                    Color32::from_rgba_unmultiplied(
                                                        (color.r * 255.) as u8,
                                                        (color.g * 255.) as u8,
                                                        (color.b * 255.) as u8,
                                                        (color.a * 255.) as u8,
                                                    ),
                                                );
                                                swatch_rect = Some((rect, color));
                                            }
                                        }
                                        ui.add(
                                            Label::new(if action {
                                                RichText::new(&primary)
                                                    .font(FontId::new(
                                                        14.5,
                                                        FontFamily::Name("UiStrong".into()),
                                                    ))
                                                    .strong()
                                            } else {
                                                RichText::new(&primary)
                                                    .font(FontId::new(
                                                        14.,
                                                        FontFamily::Name("MonoResult".into()),
                                                    ))
                                                    .strong()
                                            })
                                            .truncate(),
                                        );
                                    });
                                    let secondary = if row.tool == "registry" {
                                        format!(
                                            "{} · {}",
                                            row.item.value,
                                            self.registry_items
                                                .get(index)
                                                .map(|entry| entry.value_type.as_str())
                                                .unwrap_or("Registry")
                                        )
                                    } else if row.tool == "time"
                                        && !row.command
                                        && date_parsed.is_some()
                                    {
                                        let parsed = date_parsed.as_ref().unwrap();
                                        let mapping = if row.item.title.starts_with("其他识别 [")
                                        {
                                            format!(
                                                "识别 {} [{}] → 本地日期时间",
                                                row.item.value,
                                                row.item
                                                    .title
                                                    .trim_start_matches("其他识别 [")
                                                    .trim_end_matches(']')
                                            )
                                        } else {
                                            format!(
                                                "识别 {} [{}]{} → {}",
                                                quicktool_rust::dates::recognized(parsed.date),
                                                parsed.format,
                                                match parsed.precision.as_str() {
                                                    "year" => " · 仅年份，按 1 月 1 日",
                                                    "month" => " · 仅月份，按 1 日",
                                                    _ => "",
                                                },
                                                row.item.value
                                            )
                                        };
                                        format!("{} · {mapping}", row.item.title)
                                    } else if action {
                                        row.item.value.clone()
                                    } else {
                                        row.item.title.clone()
                                    };
                                    ui.add(
                                        Label::new(
                                            RichText::new(clip_text(
                                                &format!(
                                                    "{}{}{}",
                                                    secondary,
                                                    if command.is_some() { " · " } else { "" },
                                                    command.map(|c| c.name.as_str()).unwrap_or("")
                                                ),
                                                110,
                                            ))
                                            .size(11.)
                                            .color(weak_color(ui)),
                                        )
                                        .truncate(),
                                    );
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    self.icon(
                                        ui,
                                        if row.command {
                                            "mdi:chevron-right"
                                        } else if is_launch(&row.tool)
                                            && !row.item.title.starts_with("复制")
                                        {
                                            "mdi:open-in-new"
                                        } else {
                                            "mdi:content-copy"
                                        },
                                        16.,
                                    );
                                });
                            });
                        })
                        .response;
                    row_rects.push((index, r.rect.height(), r.rect.width()));
                    let r = ui.interact(r.rect, ui.id().with(("row", index)), Sense::click());
                    if r.hovered() && ctx.input(|i| i.pointer.delta() != Vec2::ZERO) {
                        self.selected = index;
                    }
                    if r.clicked() {
                        if let Some((_, color)) = swatch_rect.filter(|(rect, _)| {
                            ctx.pointer_hover_pos().is_some_and(|p| rect.contains(p))
                        }) {
                            color_action = Some(color);
                        } else {
                            action = Some(index);
                        }
                    }
                    let context_key = format!("result:{index}");
                    let menu = widgets::context(
                        ui,
                        &r,
                        &context_key,
                        &[
                            ("copy", "复制结果", true),
                            ("pin", "钉住结果", true),
                            ("picker", "打开 Color Picker", row.tool == "color"),
                        ],
                    );
                    if let Some(menu) = menu {
                        match menu.as_str() {
                            "copy" => {
                                if row.tool == "base64" && row.item.title.starts_with("Base64 → ")
                                {
                                    action = Some(index);
                                } else {
                                    ctx.copy_text(row.item.value.clone());
                                }
                            }
                            "pin" => {
                                if row.tool == "base64" && row.item.title.starts_with("Base64 → ")
                                {
                                    pin_image =
                                        self.decoded_image.as_ref().map(|image| image.rgba.clone());
                                } else {
                                    context_action = Some(row.item.value.clone());
                                }
                            }
                            "picker" => self.color_picker = true,
                            _ => {}
                        }
                    }

                    if selected {
                        if self.frames == 8 && std::env::args().any(|a| a == "--layout-debug") {
                            eprintln!("selected rect={:?}", r.rect);
                        }
                        selected_top = Some(r.rect.top());
                        if reveal_selection {
                            r.scroll_to_me(None);
                        }
                    }
                }
                if let Some(color) = color_action {
                    self.picker_color = [
                        color.r as f32,
                        color.g as f32,
                        color.b as f32,
                        color.a as f32,
                    ];
                    self.color_picker = true;
                }
                if let Some(text) = context_action {
                    self.pins.create_text(text);
                }
                if let Some(image) = pin_image {
                    if let Err(error) = self.pins.create_image(
                        image.width(),
                        image.height(),
                        image.as_raw().clone(),
                    ) {
                        self.message(error);
                    }
                }
                if let Some(index) = action {
                    self.activate(ctx, index);
                }
            });
        if reveal_selection {
            self.last_scroll_selection = Some(self.selected);
        }
        self.result_scroll = output.state.offset.y;
        self.result_height = (body_top + output.content_size.y.min(478.) + 14.).clamp(220., 642.);
        self.layout_snapshot = serde_json::json!({"bodyTop":body_top,"contentHeight":output.content_size.y,"rowRects":row_rects,"selectedTop":selected_top.unwrap_or(body_top),"clipBottom":ui.clip_rect().bottom()});
    }
    fn help_cards(&mut self, ui: &mut Ui) {
        let groups = [
            (
                "计算与转换",
                "mdi:swap-horizontal",
                vec!["calculator", "time", "color", "base", "unit"],
            ),
            (
                "文本与编码",
                "mdi:format-text",
                vec![
                    "json", "case", "base64", "url", "escape", "width", "text", "uuid", "random",
                ],
            ),
            (
                "字符与地区",
                "mdi:earth",
                vec!["char", "mathchar", "greek", "bopomofo", "emoji", "country"],
            ),
            (
                "运行与文件",
                "mdi:folder-search-outline",
                vec!["run", "files", "cd", "open", "registry"],
            ),
            (
                "窗口与数据",
                "mdi:dock-window",
                vec!["pin", "pinwindow", "clipboard", "history", "settings"],
            ),
            ("系统与网络", "mdi:console", vec![]),
        ];
        let mut choose = None;
        let output = ScrollArea::vertical()
            .max_height(478.)
            .vertical_scroll_offset(self.result_scroll)
            .show(ui, |ui| {
                ui.add_space(7.);
                for (title, icon, ids) in groups {
                    let commands=self.rows.iter().filter(|row|self.tab==0||(self.tab==1&&is_launch(&row.tool))||(self.tab==2&&!is_launch(&row.tool))).filter_map(|row|tools::commands().iter().find(|command|command.id==row.tool)).filter(|command|if ids.is_empty(){!groups_contains(&command.id)}else{ids.contains(&command.id.as_str())}).collect::<Vec<_>>();
                    if commands.is_empty() {
                        continue;
                    }
                    Frame::new()
                        .fill(surface(ui, 0.025))
                        .stroke(Stroke::new(
                            1.,
                            if ui.visuals().dark_mode {
                                Color32::from_rgb(40, 45, 56)
                            } else {
                                Color32::from_gray(215)
                            },
                        ))
                        .corner_radius(16)
                        .inner_margin(16)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                self.icon(ui, icon, 20.);
                                ui.label(
                                    RichText::new(title)
                                        .font(FontId::new(13., FontFamily::Name("UiStrong".into())))
                                        .strong(),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(
                                        RichText::new(format!("{} 项", commands.len()))
                                            .size(11.)
                                            .color(weak_color(ui)),
                                    );
                                });
                            });
                            ui.add_space(9.);
                            let width = (ui.available_width() - 8.) / 2.;
                            for pair in commands.chunks(2) {
                                ui.horizontal_top(|ui| {
                                    for c in pair {
                                        let selected=self.rows.get(self.selected).is_some_and(|row|row.tool==c.id);
                                        let r = Frame::new()
                                            .fill(surface(ui, 0.025))
                                            .stroke(Stroke::new(
                                                1.,
                                                if selected {ACCENT} else if ui.visuals().dark_mode {
                                                    Color32::from_rgb(37, 42, 52)
                                                } else {
                                                    Color32::from_gray(225)
                                                },
                                            ))
                                            .corner_radius(10)
                                            .inner_margin(12)
                                            .show(ui, |ui| {
                                                ui.set_width(width - 26.);
                                                ui.horizontal(|ui| {
                                                    let (icon_rect,_)=ui.allocate_exact_size(vec2(28.,28.),Sense::hover());
                                                    if let Some(texture)=self.icons.get(&c.icon){ui.painter().image(texture.id(),Rect::from_center_size(icon_rect.center(),vec2(20.,20.)),Rect::from_min_max(Pos2::ZERO,pos2(1.,1.)),ACCENT);}
                                                    ui.add_space(2.);
                                                    ui.vertical(|ui| {
                                                        ui.set_max_width((width - 92.).max(100.));
                                                        ui.horizontal(|ui| {
                                                            ui.label(RichText::new(&c.name).font(
                                                                FontId::new(
                                                                    12.,
                                                                    FontFamily::Name(
                                                                        "UiHelp".into(),
                                                                    ),
                                                                ),
                                                            ));
                                                            ui.label(
                                                                RichText::new(format!(
                                                                    "/{}",
                                                                    if c.id == "time" {
                                                                        "timestamp"
                                                                    } else {
                                                                        &c.primary_alias
                                                                    }
                                                                ))
                                                                .monospace()
                                                                .size(10.5)
                                                                .color(ACCENT),
                                                            );
                                                        });
                                                        ui.add(
                                                            Label::new(
                                                                RichText::new(help_description(
                                                                    &c.id,
                                                                ))
                                                                .size(10.5)
                                                                .color(weak_color(ui)),
                                                            )
                                                            .truncate(),
                                                        );
                                                        ui.add(
                                                            Label::new(
                                                                RichText::new(
                                                                    c.aliases
                                                                        .iter()
                                                                        .filter(|alias|alias.as_str()!=(if c.id=="time"{"timestamp"}else{&c.primary_alias})&&alias.chars().any(|ch|('\u{3400}'..='\u{9fff}').contains(&ch))).take(3)
                                                                        .cloned()
                                                                        .collect::<Vec<_>>()
                                                                        .join(" · "),
                                                                )
                                                                .size(10.)
                                                                .color(weak_color(ui)),
                                                            )
                                                            .truncate(),
                                                        );
                                                    });
                                                });
                                            })
                                            .response;
                                        if let Some(texture)=self.icons.get("mdi:chevron-right") {ui.painter().image(texture.id(),Rect::from_center_size(r.rect.right_center()-vec2(20.,0.),vec2(14.,14.)),Rect::from_min_max(Pos2::ZERO,pos2(1.,1.)),weak_color(ui));}
                                        if selected && self.selected > 0 {r.scroll_to_me(None);}
                                        if ui
                                            .interact(
                                                r.rect,
                                                ui.id().with(("help", &c.id)),
                                                Sense::click(),
                                            )
                                            .clicked()
                                        {
                                            choose = Some(c.id.clone());
                                        }
                                    }
                                });
                                ui.add_space(8.);
                            }
                        });
                    ui.add_space(12.);
                }
            });
        self.result_scroll = output.state.offset.y;
        if let Some(id) = choose {
            if let Some(index) = self.rows.iter().position(|row| row.tool == id) {
                let context = ui.ctx().clone();
                self.activate(&context, index);
            }
        }
    }
    fn showing_help(&self) -> bool {
        self.mode.as_deref() == Some("help")
            || (self.mode.is_none()
                && self.input.strip_prefix('/').is_some_and(|query| {
                    tools::command(
                        query
                            .split_once(quicktool_rust::core::js_space)
                            .map(|(head, _)| head)
                            .unwrap_or(query),
                    )
                    .is_some_and(|command| command.id == "help")
                }))
    }
    fn row_is_launch(&self, row: &Row) -> bool {
        if let Some((tool, _)) = &row.mode_action {
            return is_launch(tool);
        }
        is_launch(if row.tool.starts_with('@') {
            self.mode.as_deref().unwrap_or(&row.tool)
        } else {
            &row.tool
        })
    }
    fn visible_indices(&self) -> Vec<usize> {
        let mut visible = self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                self.tab == 0
                    || (self.tab == 1 && self.row_is_launch(r))
                    || (self.tab == 2 && !self.row_is_launch(r))
            })
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        if self.mode.is_none()
            && !self
                .input
                .trim_matches(quicktool_rust::core::js_space)
                .is_empty()
            && !self.input.starts_with('/')
        {
            let top = visible
                .iter()
                .map(|i| self.rows[*i].score)
                .fold(0., f64::max);
            let threshold = (top - 0.34).max(0.54);
            visible = visible
                .into_iter()
                .enumerate()
                .filter(|(rank, index)| *rank < 2 || self.rows[*index].score >= threshold)
                .map(|(_, index)| index)
                .collect();
        }
        if self.showing_help() {
            return visible;
        }
        visible.truncate(if self.tab == 0 { 11 } else { 14 });
        if self.tab == 0 && visible.len() > 1 {
            visible[1..].sort_by_key(|i| !self.row_is_launch(&self.rows[*i]));
        }
        visible
    }
    fn settings(&mut self, ui: &mut Ui) {
        if self.modules_page {
            self.modules_settings(ui);
            return;
        }
        self.panel_head(
            ui,
            "mdi:cog-outline",
            "设置",
            "实时预览；保存后生效，取消恢复原设置",
            |app, ui| {
                if app
                    .icon_button_compact(ui, "mdi:close", "关闭设置")
                    .clicked()
                {
                    app.close_settings();
                }
            },
        );
        let available = (ui.available_height() - 78.).max(280.);
        ScrollArea::vertical()
            .max_height(available)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                Frame::new()
                    .inner_margin(Margin {
                        left: 14,
                        right: 14,
                        top: 10,
                        bottom: 8,
                    })
                    .show(ui, |ui| {
                        setting_row(
                            ui,
                            "模块管理",
                            "安装或卸载功能模块；卸载保留钉贴与历史等用户数据",
                            |ui| {
                                if soft_button(ui, "管理").clicked() {
                                    self.modules_page = true;
                                }
                            },
                        );
                        setting_row(
                            ui,
                            "全局快捷键",
                            "点击右侧，然后直接按下希望使用的快捷键组合",
                            |ui| {
                                let label = if self.recording_shortcut {
                                    "请按下快捷键…".to_string()
                                } else {
                                    self.draft["globalShortcut"]
                                        .as_str()
                                        .unwrap_or("Alt+Space")
                                        .replace('+', " + ")
                                };
                                let (rect, response) =
                                    ui.allocate_exact_size(vec2(220., 40.), Sense::click());
                                let dark = ui.visuals().dark_mode;
                                let (fill, stroke) = if self.recording_shortcut {
                                    (
                                        if dark {
                                            Color32::from_rgba_unmultiplied(120, 160, 240, 24)
                                        } else {
                                            Color32::from_rgba_unmultiplied(105, 125, 232, 24)
                                        },
                                        Stroke::new(1., accent_color(ui)),
                                    )
                                } else if response.hovered() {
                                    (hover_bg(ui), Stroke::new(1., border_color(ui)))
                                } else {
                                    (field_bg(ui), Stroke::new(1., border_color(ui)))
                                };
                                ui.painter().rect(
                                    rect,
                                    CornerRadius::same(10),
                                    fill,
                                    stroke,
                                    StrokeKind::Inside,
                                );
                                if let Some(texture) = self.icons.get("mdi:keyboard-outline") {
                                    ui.painter().image(
                                        texture.id(),
                                        Rect::from_min_size(
                                            rect.left_top() + vec2(10., 11.5),
                                            vec2(17., 17.),
                                        ),
                                        Rect::from_min_max(pos2(0., 0.), pos2(1., 1.)),
                                        accent_color(ui),
                                    );
                                }
                                let galley = ui.painter().layout_no_wrap(
                                    label,
                                    FontId::monospace(11.5),
                                    setting_text(ui),
                                );
                                ui.painter().galley(
                                    pos2(
                                        rect.left() + 37.,
                                        rect.center().y - galley.size().y / 2.,
                                    ),
                                    galley,
                                    setting_text(ui),
                                );
                                self.recording_rect = Some(rect);
                                if response.clicked() {
                                    self.hotkey = None;
                                    self.registered_shortcut.clear();
                                    self.recording_shortcut = true;
                                    self.shortcut_error = None;
                                }
                            },
                        );
                        if let Some(error) = &self.shortcut_error {
                            ui.colored_label(Color32::LIGHT_RED, error);
                        }
                setting_row(
                    ui,
                    "主题",
                    "浅色、深色或跟随 Windows 系统主题",
                    |ui| {
                        select_setting(
                            ui,
                            &mut self.draft,
                            "theme",
                            &[("system", "跟随系统"), ("light", "浅色"), ("dark", "深色")],
                        );
                    },
                );
                for (key, label, desc) in [
                    ("hideOnBlur", "失焦后隐藏", "钉起窗口时不会自动隐藏"),
                    (
                        "summonAlwaysOnTop",
                        "唤起时置顶",
                        "通过快捷键或任务栏唤起时固定到最上层并聚焦",
                    ),
                ] {
                    setting_row(ui, label, desc, |ui| {
                        let mut v = self.draft[key].as_bool().unwrap_or(true);
                        if accent_checkbox(ui, &mut v).changed() {
                            self.draft[key] = v.into();
                        }
                    });
                }
                setting_row(
                    ui,
                    "普通历史上限",
                    "收藏项目不计入该限制",
                    |ui| {
                        let mut v = self.draft["historyLimit"].as_u64().unwrap_or(500);
                        let changed = Frame::new()
                            .fill(field_bg(ui))
                            .stroke(Stroke::new(1., border_color(ui)))
                            .corner_radius(9)
                            .inner_margin(Margin::symmetric(8, 0))
                            .show(ui, |ui| {
                                ui.set_min_size(vec2(74., 32.));
                                ui.add_sized(
                                    [74., 32.],
                                    DragValue::new(&mut v).range(50..=5000).speed(50),
                                )
                            })
                            .inner
                            .changed();
                        if changed {
                            self.draft["historyLimit"] = v.into();
                        }
                    },
                );
                setting_row(
                    ui,
                    "全局文件搜索",
                    "普通输入同时查询文件名；无需安装 Everything",
                    |ui| {
                        let mut v = self.draft["fileSearchGlobal"].as_bool().unwrap_or(true);
                        if accent_checkbox(ui, &mut v).changed() {
                            self.draft["fileSearchGlobal"] = v.into();
                        }
                    },
                );
                setting_row(
                    ui,
                    "文件搜索来源",
                    "自动优先 Everything，未安装时使用内置索引",
                    |ui| {
                        select_setting(
                            ui,
                            &mut self.draft,
                            "fileSearchProvider",
                            &[
                                ("auto", "自动"),
                                ("local", "内置索引"),
                                ("windows", "Windows Search"),
                            ],
                        );
                    },
                );
                for (key, label, desc) in [
                    (
                        "fileIndexRoots",
                        "文件索引目录",
                        "每行一个绝对目录；留空为桌面、文档和下载。仅索引名称，不读取正文。",
                    ),
                    (
                        "fileIndexExcludes",
                        "排除目录名",
                        "每行一个目录名；跳过链接，避免循环扫描。",
                    ),
                ] {
                    setting_row(ui, label, desc, |ui| {
                        let mut value = self.draft[key].as_str().unwrap_or("").to_string();
                        let changed = Frame::new()
                            .fill(field_bg(ui))
                            .stroke(Stroke::new(1., border_color(ui)))
                            .corner_radius(10)
                            .inner_margin(Margin::symmetric(10, 9))
                            .show(ui, |ui| {
                                ui.add(
                                    TextEdit::multiline(&mut value)
                                        .desired_width(f32::INFINITY)
                                        .desired_rows(3)
                                        .frame(false)
                                        .font(FontId::monospace(11.)),
                                )
                                .changed()
                            })
                            .inner;
                        if changed {
                            self.draft[key] = value.into();
                        }
                    });
                }
                {
                    let top = ui.cursor().top();
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "索引 {} 项{}",
                                self.file_status.count,
                                if self.file_status.building {
                                    " · 建立中"
                                } else {
                                    ""
                                }
                            ))
                            .size(11.5)
                            .color(setting_text_2(ui)),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if soft_button(ui, "重建").clicked() {
                                if let Some(index) = &self.file_index {
                                    let (roots, excludes) = self.index_config();
                                    let _ = index
                                        .requests
                                        .send(index::Request::Configure { roots, excludes });
                                }
                            }
                            if soft_button(ui, if self.file_status.paused {
                                "继续"
                            } else {
                                "暂停"
                            })
                            .clicked()
                            {
                                if let Some(index) = &self.file_index {
                                    let _ = index
                                        .requests
                                        .send(index::Request::Pause(!self.file_status.paused));
                                }
                            }
                        });
                    });
                    let used = ui.cursor().top() - top;
                    ui.add_space((62. - used).max(0.) + 10.);
                    draw_bottom_line(ui, false, row_line(ui));
                    ui.add_space(10.);
                }
                setting_row(
                    ui,
                    "日期识别首选顺序",
                    "有歧义时优先此顺序，其他有效解释会在结果中列出",
                    |ui| {
                        select_setting(
                            ui,
                            &mut self.draft,
                            "dateOrder",
                            &[
                                ("YMD", "年/月/日 YMD"),
                                ("YDM", "年/日/月 YDM"),
                                ("DMY", "日/月/年 DMY"),
                                ("MDY", "月/日/年 MDY"),
                            ],
                        );
                    },
                );
                setting_row(
                    ui,
                    "自定义时间格式",
                    "每行“名称=格式”；支持 YYYY MM DD HH mm ss SSS X x W，UTC: 前缀使用 UTC",
                    |ui| {
                        let mut value = self.draft["timeCustomFormats"]
                            .as_str()
                            .unwrap_or("")
                            .to_string();
                        let changed = Frame::new()
                            .fill(field_bg(ui))
                            .stroke(Stroke::new(1., border_color(ui)))
                            .corner_radius(10)
                            .inner_margin(Margin::symmetric(10, 9))
                            .show(ui, |ui| {
                                ui.add(
                                    TextEdit::multiline(&mut value)
                                        .desired_width(f32::INFINITY)
                                        .desired_rows(4)
                                        .frame(false)
                                        .font(FontId::monospace(11.)),
                                )
                                .changed()
                            })
                            .inner;
                        if changed {
                            self.draft["timeCustomFormats"] = value.into();
                        }
                    },
                );
                setting_row(ui, "玻璃透明度", "向右更透明", |ui| {
                    let mut v = 1. - self.draft["glassOpacity"].as_f64().unwrap_or(0.78);
                    ui.visuals_mut().selection.bg_fill = accent_color(ui);
                    if ui
                        .add_sized([150., 20.], Slider::new(&mut v, 0.0..=1.).show_value(false))
                        .changed()
                    {
                        self.draft["glassOpacity"] = (1. - v).into();
                    }
                });
                setting_row(
                    ui,
                    "实时背景模糊",
                    "实时显示窗口后方内容；模糊半径由 Windows 控制",
                    |ui| {
                        let mut v = self.draft["blurStrength"].as_f64().unwrap_or(28.) > 0.;
                        if accent_checkbox(ui, &mut v).changed() {
                            self.draft["blurStrength"] = if v { 28.into() } else { 0.into() };
                        }
                    },
                );
                setting_row(ui, "边缘 Glow", "调整流动边缘光强度", |ui| {
                    let mut v = self.draft["glowStrength"].as_f64().unwrap_or(0.55);
                    ui.visuals_mut().selection.bg_fill = accent_color(ui);
                    if ui
                        .add_sized([150., 20.], Slider::new(&mut v, 0.0..=1.).show_value(false))
                        .changed()
                    {
                        self.draft["glowStrength"] = v.into();
                    }
                });
                setting_row(
                    ui,
                    "减少动画",
                    "关闭流动边缘光和大部分位移动画",
                    |ui| {
                        let mut v = self.draft["reduceMotion"].as_bool().unwrap_or(false);
                        if accent_checkbox(ui, &mut v).changed() {
                            self.draft["reduceMotion"] = v.into();
                        }
                    },
                );
                    });
            });
        ui.horizontal(|ui| {
            ui.add_space(14.);
            ui.label(
                RichText::new("界面使用 MiSans Global、Jura 与 JetBrains Mono 字体")
                    .size(10.)
                    .color(weak_color(ui)),
            );
        });
        let footer = ui.available_rect_before_wrap();
        ui.painter().line_segment(
            [
                pos2(footer.left(), footer.top() + 0.5),
                pos2(footer.right(), footer.top() + 0.5),
            ],
            Stroke::new(1., panel_line(ui)),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(14.);
            if primary_button(ui, "保存设置").clicked() && !self.recording_shortcut {
                self.save_settings();
            }
            ui.add_space(10.);
            if soft_button(ui, "取消").clicked() {
                self.close_settings();
            }
        });
    }
    fn modules_settings(&mut self, ui: &mut Ui) {
        self.panel_head(
            ui,
            "mdi:view-grid-outline",
            "模块管理",
            "安装或卸载功能模块，即时生效；卸载不删除钉贴与历史等用户数据",
            |app, ui| {
                if soft_button(ui, "返回设置").clicked() {
                    app.modules_page = false;
                }
                if app
                    .icon_button_compact(ui, "mdi:close", "关闭设置")
                    .clicked()
                {
                    app.close_settings();
                }
            },
        );
        let available = ui.available_height().max(280.);
        ScrollArea::vertical()
            .max_height(available)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                Frame::new()
                    .inner_margin(Margin {
                        left: 14,
                        right: 14,
                        top: 10,
                        bottom: 8,
                    })
                    .show(ui, |ui| {
                        for info in modules::catalog() {
                            let installed = modules::is_installed(&info.id);
                            let size_label = if info.size_bytes > 0 {
                                let mb = info.size_bytes as f64 / 1048576.;
                                if mb >= 1. {
                                    format!("约 {:.1} MB · {} 个命令", mb, info.commands.len())
                                } else {
                                    format!(
                                        "约 {:.0} KB · {} 个命令",
                                        info.size_bytes as f64 / 1024.,
                                        info.commands.len()
                                    )
                                }
                            } else {
                                format!("{} 个命令", info.commands.len())
                            };
                            let top = ui.cursor().top();
                            ui.horizontal(|ui| {
                                let (icon_rect, _) =
                                    ui.allocate_exact_size(vec2(22., 22.), Sense::hover());
                                if let Some(texture) = self.icons.get(&info.icon) {
                                    ui.painter().image(
                                        texture.id(),
                                        icon_rect,
                                        Rect::from_min_max(pos2(0., 0.), pos2(1., 1.)),
                                        setting_text_2(ui),
                                    );
                                }
                                ui.add_space(8.);
                                ui.vertical(|ui| {
                                    ui.set_width((ui.available_width() - 160.).max(120.));
                                    ui.label(
                                        setting_title(&info.name, 12.5)
                                            .strong()
                                            .color(setting_text(ui)),
                                    );
                                    ui.add_space(4.);
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 0.;
                                        ui.label(
                                            RichText::new(if installed {
                                                "已安装"
                                            } else {
                                                "未安装"
                                            })
                                            .size(10.5)
                                            .color(
                                                if installed {
                                                    if ui.visuals().dark_mode {
                                                        Color32::from_rgb(120, 200, 140)
                                                    } else {
                                                        Color32::from_rgb(61, 138, 85)
                                                    }
                                                } else {
                                                    weak_color(ui)
                                                },
                                            ),
                                        );
                                        ui.label(
                                            RichText::new(format!(
                                                " · {} · {}",
                                                info.description, size_label
                                            ))
                                            .size(10.5)
                                            .color(weak_color(ui)),
                                        );
                                    });
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let action = if installed { "卸载" } else { "安装" };
                                    if soft_button(ui, action).clicked() {
                                        self.toggle_module(&info.id, installed);
                                    }
                                });
                            });
                            let used = ui.cursor().top() - top;
                            ui.add_space((62. - used).max(0.) + 10.);
                            draw_bottom_line(ui, false, row_line(ui));
                            ui.add_space(10.);
                        }
                    });
            });
    }
    fn toggle_module(&mut self, id: &str, installed: bool) {
        let name = modules::info(id)
            .map(|info| info.name.clone())
            .unwrap_or_else(|| id.to_string());
        let outcome = if installed {
            if let Some(mode) = self.mode.as_ref() {
                if modules::module_of(mode).map(|info| info.id.as_str()) == Some(id) {
                    self.mode = None;
                    self.submode = None;
                }
            }
            modules::uninstall(id).map(|_| format!("已卸载 {name} 模块"))
        } else {
            modules::install(id).map(|_| format!("已安装 {name} 模块"))
        };
        match outcome {
            Ok(text) => self.message(text),
            Err(error) => self.message(error),
        }
        self.rebuild();
        self.focus = true;
    }
    fn save_settings(&mut self) {
        if let Some(path) = self.draft["fileIndexRoots"]
            .as_str()
            .unwrap_or("")
            .lines()
            .map(str::trim)
            .find(|p| !p.is_empty() && !std::path::Path::new(p).is_absolute())
        {
            self.message(format!("索引目录必须是绝对路径：{path}"));
            return;
        }
        let old = self.store.settings.clone();

        let new_shortcut = self.draft["globalShortcut"]
            .as_str()
            .unwrap_or("Alt+Space")
            .to_string();
        let new_hotkey = if self.registered_shortcut != new_shortcut {
            let context = self.context.clone();
            match platform::hotkey(&new_shortcut, move || {
                context.request_repaint_of(ViewportId::ROOT)
            }) {
                Ok(h) => Some(h),
                Err(error) => {
                    self.shortcut_error = Some(format!("快捷键不可用或已被占用：{error}"));
                    return;
                }
            }
        } else {
            None
        };
        self.store.settings = self.draft.clone();
        if let Err(error) = self.store.save_settings() {
            self.store.settings = old;
            self.message(format!("保存失败：{error}"));
            return;
        }
        if let Some(h) = new_hotkey {
            self.hotkey = Some(h);
            self.registered_shortcut = new_shortcut.to_string();
        }
        if let Some(index) = &self.file_index {
            let (roots, excludes) = self.index_config();
            let _ = index
                .requests
                .send(index::Request::Configure { roots, excludes });
        }
        self.panel = Panel::None;
        self.focus = true;
        self.message("设置已保存");
    }
    fn record_shortcut(&mut self, ctx: &Context) {
        if !self.recording_shortcut {
            return;
        }
        let lost_focus = ctx.input(|input| {
            input.viewport().focused == Some(false)
                || (input.pointer.any_pressed()
                    && input.pointer.interact_pos().is_some_and(|point| {
                        self.recording_rect
                            .is_some_and(|rect| !rect.contains(point))
                    }))
        });
        if lost_focus {
            self.recording_shortcut = false;
            let previous = self.draft["globalShortcut"]
                .as_str()
                .unwrap_or("Alt+Space")
                .to_string();
            if let Err(error) = self.shortcut_preview(&previous) {
                self.shortcut_error = Some(error);
            }
            return;
        }
        for event in ctx.input(|i| i.events.clone()) {
            if let Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = event
            {
                if key == Key::Escape {
                    self.recording_shortcut = false;
                    let old = self.draft["globalShortcut"]
                        .as_str()
                        .unwrap_or("Alt+Space")
                        .to_string();
                    if let Err(error) = self.shortcut_preview(&old) {
                        self.shortcut_error = Some(error);
                    }
                    ctx.input_mut(|i| i.events.retain(|e| !matches!(e, Event::Key { .. })));
                    return;
                }
                let name = match key {
                    Key::Space => "Space".into(),
                    Key::ArrowLeft => "Left".into(),
                    Key::ArrowRight => "Right".into(),
                    Key::ArrowUp => "Up".into(),
                    Key::ArrowDown => "Down".into(),
                    Key::Plus | Key::Equals => "Plus".into(),
                    Key::Minus => "Minus".into(),
                    _ => {
                        let name = format!("{key:?}");
                        name.strip_prefix("Num").unwrap_or(&name).to_string()
                    }
                };
                let win = {
                    #[cfg(windows)]
                    {
                        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
                            GetAsyncKeyState, VK_LWIN, VK_RWIN,
                        };
                        unsafe {
                            GetAsyncKeyState(VK_LWIN as i32) < 0
                                || GetAsyncKeyState(VK_RWIN as i32) < 0
                        }
                    }
                    #[cfg(not(windows))]
                    {
                        modifiers.mac_cmd
                    }
                };
                let bare_function = name
                    .strip_prefix('F')
                    .and_then(|n| n.parse::<u8>().ok())
                    .is_some_and(|n| (1..=24).contains(&n));
                if !(modifiers.ctrl || modifiers.alt || modifiers.shift || win || bare_function) {
                    self.shortcut_error =
                        Some("请使用修饰键（Ctrl / Alt / Shift / Win）或 F1–F24".into());
                    continue;
                }
                let mut parts = Vec::new();
                if modifiers.ctrl {
                    parts.push("Control".into());
                }
                if modifiers.alt {
                    parts.push("Alt".into());
                }
                if modifiers.shift {
                    parts.push("Shift".into());
                }
                if win {
                    parts.push("Super".into());
                }
                parts.push(name);
                let shortcut = parts.join("+");
                self.recording_shortcut = false;
                match self.shortcut_preview(&shortcut) {
                    Ok(()) => {
                        self.draft["globalShortcut"] = shortcut.into();
                        self.shortcut_error = None;
                    }
                    Err(error) => {
                        self.shortcut_error = Some(format!("快捷键不可用或已被占用：{error}"));
                        let old = self.draft["globalShortcut"]
                            .as_str()
                            .unwrap_or("Alt+Space")
                            .to_string();
                        let _ = self.shortcut_preview(&old);
                    }
                }
                ctx.input_mut(|i| {
                    i.events
                        .retain(|e| !matches!(e, Event::Key { .. } | Event::Text(_)))
                });
                return;
            }
        }
    }
    fn pickers(&mut self, ctx: &Context) {
        if self.eyedrop_pending > 0 {
            self.eyedrop_pending -= 1;
            if self.eyedrop_pending == 0 {
                if let Err(error) = self.eyedrop.start(ctx) {
                    self.message(error);
                    self.color_picker = true;
                }
            }
            ctx.request_repaint();
        }
        if let Some(result) = self.eyedrop.update(ctx) {
            self.color_picker = true;
            match result {
                Ok(Some(rgb)) => {
                    for (i, value) in rgb.into_iter().enumerate() {
                        self.picker_color[i] = value as f32 / 255.;
                    }
                    let c = self.picker_color;
                    self.input = quicktool_rust::colors::Color::new(
                        c[0] as f64,
                        c[1] as f64,
                        c[2] as f64,
                        c[3] as f64,
                    )
                    .hex(c[3] < 0.999);
                    self.rebuild();
                }
                Ok(None) => {}
                Err(error) => self.message(error),
            }
        }

        if self.color_picker {
            let action = if self.interaction_probe.is_some() || self.screenshot.is_some() {
                pickers::color_panel(
                    ctx,
                    &mut self.color_picker,
                    &mut self.picker_color,
                    &mut self.native_color,
                    self.icons.get("mdi:content-copy"),
                )
            } else {
                pickers::color_panel_detached(
                    ctx,
                    &mut self.color_picker,
                    &mut self.picker_color,
                    &mut self.native_color,
                    self.icons.get("mdi:content-copy"),
                    self.glow.owner,
                    self.applied_topmost.unwrap_or(false),
                )
            };
            if std::mem::take(&mut self.native_color.eyedrop_requested) {
                self.native_color.open = false;
                self.color_picker = false;
                self.eyedrop_pending = 2;
                ctx.request_repaint();
            }
            if let Some((title, value)) = action.copied {
                ctx.copy_text(value);
                self.message(format!("{title} 已复制"));
            }
            if action.edited {
                let c = self.picker_color;
                self.input = quicktool_rust::colors::Color::new(
                    c[0] as f64,
                    c[1] as f64,
                    c[2] as f64,
                    c[3] as f64,
                )
                .hex(c[3] < 0.999);
                self.rebuild();
            }
        }
        if self.date_picker {
            let changed = if self.interaction_probe.is_some() || self.screenshot.is_some() {
                self.native_date
                    .show(ctx, &mut self.date_picker, &mut self.date_value)
            } else {
                self.native_date.show_detached(
                    ctx,
                    &mut self.date_picker,
                    &mut self.date_value,
                    self.glow.owner,
                    self.applied_topmost.unwrap_or(false),
                )
            };
            if changed {
                self.input = self.date_value.clone();
                self.rebuild();
            }
        }
    }

    fn probe_tick(&mut self, ctx: &Context, target_height: f32) {
        let Some(mut probe) = self.probe.take() else {
            return;
        };
        if let Some(ready) = probe.ready_at {
            let elapsed = ready.elapsed();
            ctx.request_repaint_after(if elapsed < Duration::from_secs(5) {
                Duration::from_secs(5) - elapsed
            } else {
                Duration::from_millis(16)
            });
        } else {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        if self.frames >= 5 && probe.ready_at.is_none() {
            probe.ready_at = Some(Instant::now());
            probe.ready_frame = self.frames;
            probe.startup_ms = crate::PROCESS_STARTED
                .get()
                .map(|start| start.elapsed().as_secs_f64() * 1000.)
                .unwrap_or(0.);
        }
        if probe
            .ready_at
            .is_some_and(|at| at.elapsed() >= Duration::from_secs(5))
        {
            if probe.idle.is_none() {
                probe.idle = Some(process_memory());
                probe.idle_frames = Some(self.frames.saturating_sub(probe.ready_frame));
            }
            if probe.pending.as_ref().is_some_and(|(_, start, _)| {
                (self.last_height - target_height).abs() < 1.0
                    || start.elapsed() > Duration::from_secs(3)
            }) {
                if let Some((query, start, rebuild)) = probe.pending.take() {
                    probe.samples.push(serde_json::json!({"query":query,"rebuild_ms":rebuild,"query_to_stable_layout_ms":start.elapsed().as_secs_f64()*1000.,"rows":self.rows.len()}));
                }
            }
            if probe.pending.is_none() {
                let queries = [
                    "0xFF",
                    "100cm",
                    "#7c3aed",
                    "/时间 2026-10-20 12:34:56",
                    "/json {\"a\":1}",
                    "/base64 Rust 性能",
                    "/国家 CN",
                    "/大小写 QuickTool Rust",
                    "/unicode U+00FF",
                    "/帮助",
                ];
                if let Some(query) = queries.get(probe.samples.len()) {
                    self.input = query.to_string();
                    self.mode = None;
                    self.submode = None;
                    let start = Instant::now();
                    self.rebuild();
                    let rebuild = start.elapsed().as_secs_f64() * 1000.;
                    probe.pending = Some((query.to_string(), start, rebuild));
                } else {
                    let value = serde_json::json!({"version":env!("CARGO_PKG_VERSION"),"pid":std::process::id(),"profile":if cfg!(debug_assertions){"debug"}else{"release"},"dpi":ctx.pixels_per_point(),"first_stable_frame_ms":probe.startup_ms,"idle_seconds":5,"idle_memory":probe.idle,"idle_root_frames":probe.idle_frames,"query_samples":probe.samples,"measurement":"query_to_stable_layout includes resize animation and CPU UI layout; excludes OS display presentation; idle memory includes native glow/backdrop"});
                    match serde_json::to_vec_pretty(&value)
                        .map_err(std::io::Error::other)
                        .and_then(|bytes| {
                            quicktool_rust::storage::atomic_write(&probe.path, &bytes)
                        }) {
                        Ok(()) => eprintln!("UI metrics saved: {}", probe.path.display()),
                        Err(error) => {
                            eprintln!("UI metrics failed {}: {error}", probe.path.display())
                        }
                    }
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                    return;
                }
            }
        }
        self.probe = Some(probe);
    }
    fn history(&mut self, ui: &mut Ui, ctx: &Context) {
        self.history_panel(ui, ctx);
    }
}
impl eframe::App for QuickTool {
    fn on_exit(&mut self, _: Option<&eframe::glow::Context>) {
        self.pins.flush_history();
        for update in self.pins.drain_history_updates() {
            if let Err(error) = self.store.enqueue_pin(update, || {}) {
                eprintln!("保存 Pin 历史失败：{error}");
            }
        }
        for error in self.store.flush_pin_updates() {
            eprintln!("保存 Pin 历史失败：{error}");
        }
    }
    fn clear_color(&self, _: &Visuals) -> [f32; 4] {
        [0., 0., 0., 0.]
    }
    fn raw_input_hook(&mut self, _ctx: &Context, raw: &mut RawInput) {
        if raw.viewport_id == ViewportId::ROOT {
            interaction_probe::raw_input(self, raw);
            search_layout_probe::raw_input(self, raw);
        }
    }
    fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        if self.screenshot.is_some() {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        if let Some(outcome) = self
            .image_copy
            .as_ref()
            .and_then(|receiver| receiver.try_recv().ok())
        {
            self.image_copy = None;
            match outcome {
                Ok(()) => self.message("图片已复制"),
                Err(error) => self.message(format!("复制图片失败：{error}")),
            }
        }
        self.frames += 1;
        summon_probe::tick(self, ctx);
        let tray_events = self
            .tray
            .as_ref()
            .map(|tray| tray.events.try_iter().collect::<Vec<_>>())
            .unwrap_or_default();
        for event in tray_events {
            match event {
                quicktool_rust::tray::TrayEvent::Show => {
                    self.show_main(ctx);
                }
                quicktool_rust::tray::TrayEvent::Quit => {
                    ctx.send_viewport_cmd(ViewportCommand::Close)
                }
            }
        }
        if let Some(result) = self
            .attachment_job
            .as_ref()
            .and_then(|receiver| receiver.try_recv().ok())
        {
            self.attachment_job = None;
            match result {
                Ok((attachment, results)) => self.set_attachment_ready(attachment, results),
                Err(error) => self.message(format!("打开图片失败：{error}")),
            }
        }
        let drops = ctx.input(|input| input.raw.dropped_files.clone());
        if self.panel == Panel::None && !drops.is_empty() {
            self.load_attachment(move || {
                for file in drops {
                    let result = if let Some(path) = file.path {
                        attachments::Attachment::from_path(&path)
                    } else if let Some(bytes) = file.bytes {
                        attachments::Attachment::from_bytes(file.name, &bytes)
                    } else {
                        continue;
                    };
                    if result.is_ok() {
                        return result;
                    }
                }
                Err("未找到可识别图片".into())
            });
        }
        let decoded = self.decoder.results.try_iter().collect::<Vec<_>>();
        for (serial, result) in decoded {
            if self
                .decoded_request
                .as_ref()
                .is_some_and(|(_, current)| *current == serial)
            {
                self.decoded_pending = false;
                if let Ok(image) = result {
                    self.install_decoded_image(image);
                }
            }
        }
        if !self.candidate_testing {
            self.poll_files();
        }
        if !self.candidate_testing {
            self.poll_system();
        }
        self.poll_icons();
        if self.files_pending {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        if self
            .hotkey
            .as_ref()
            .is_some_and(|h| h.events.try_recv().is_ok())
            && !self.recording_shortcut
        {
            if self.main_visible {
                self.main_visible = false;
                if let Some(glass) = &mut self.backdrop {
                    let _ = glass.update(false, self.pinned, true, 0., 0, 0.);
                }
                ctx.send_viewport_cmd(ViewportCommand::Visible(false));
            } else {
                self.show_main(ctx);
            }
        }
        if let Some(error) = self.hotkey_error.take() {
            self.message(error);
        }
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(false));
        if self.was_focused
            && !focused
            && self.blur_hides(ctx)
            && self.screenshot.is_none()
            && self.probe.is_none()
            && self.interaction_probe.is_none()
        {
            self.main_visible = false;
            if let Some(glass) = &mut self.backdrop {
                let _ = glass.update(false, self.pinned, true, 0., 0, 0.);
            }
            ctx.send_viewport_cmd(ViewportCommand::Visible(false));
        }
        self.was_focused = focused;
        let visual = if self.panel == Panel::Settings {
            self.draft.clone()
        } else {
            self.store.settings.clone()
        };
        ctx.request_repaint_after(Duration::from_secs(1));
        if self.theme_polled.elapsed() >= Duration::from_secs(1) {
            self.system_dark = system::dark_theme();
            self.theme_polled = Instant::now();
        }
        let light =
            visual["theme"] == "light" || (visual["theme"] == "system" && !self.system_dark);
        let effective_topmost = self.pinned || visual["summonAlwaysOnTop"] == true;
        if self.applied_topmost != Some(effective_topmost) {
            ctx.send_viewport_cmd(ViewportCommand::WindowLevel(if effective_topmost {
                WindowLevel::AlwaysOnTop
            } else {
                WindowLevel::Normal
            }));
            self.applied_topmost = Some(effective_topmost);
        }
        let opacity = visual["glassOpacity"].as_f64().unwrap_or(0.78) as f32;
        let blur = visual["blurStrength"].as_u64().unwrap_or(28) as u32;
        #[cfg(windows)]
        if self.screenshot.is_none() {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(handle) = frame.window_handle() {
                if let RawWindowHandle::Win32(handle) = handle.as_raw() {
                    self.glow.owner = handle.hwnd.get();
                    unsafe {
                        use windows_sys::Win32::{
                            Foundation::RECT, UI::WindowsAndMessaging::GetClientRect,
                        };
                        let mut client: RECT = std::mem::zeroed();
                        if GetClientRect(handle.hwnd.get() as _, &mut client) != 0 {
                            // The native clip region must follow every actual resize, including
                            // compact -> settings/results and delayed viewport resize delivery.
                            let style = (
                                light,
                                blur,
                                (ctx.pixels_per_point() * 100.).round() as u32,
                                [client.right - client.left, client.bottom - client.top],
                            );
                            if self.window_style != Some(style)
                                && platform::apply_window_style(
                                    handle.hwnd.get(),
                                    !light,
                                    0,
                                    (8. * ctx.pixels_per_point()).round() as i32,
                                )
                                .is_ok()
                            {
                                self.window_style = Some(style);
                            }
                        }
                    }
                }
            }
        }
        #[cfg(windows)]
        if !self.backdrop_initialized && self.screenshot.is_none() {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(handle) = frame.window_handle() {
                if let RawWindowHandle::Win32(handle) = handle.as_raw() {
                    match quicktool_rust::backdrop::Backdrop::new(handle.hwnd.get()) {
                        Ok(glass) => self.backdrop = Some(glass),
                        Err(error) => self.message(format!("背景模糊不可用：{error}")),
                    }
                    self.backdrop_initialized = true;
                }
            }
        }
        #[cfg(not(windows))]
        let _ = frame;
        let mut visuals = if light {
            Visuals::light()
        } else {
            Visuals::dark()
        };
        visuals.override_text_color = Some(if light {
            Color32::from_rgb(29, 36, 49)
        } else {
            Color32::from_rgb(245, 247, 251)
        });
        visuals.selection.bg_fill = Color32::from_rgb(45, 59, 87);
        visuals.widgets.inactive.corner_radius = CornerRadius::same(4);
        visuals.window_shadow = Shadow::NONE;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(
            1.,
            if light {
                Color32::from_gray(220)
            } else {
                Color32::from_rgb(36, 40, 50)
            },
        );
        visuals.widgets.inactive.bg_fill = if light {
            Color32::from_rgb(239, 242, 249)
        } else {
            Color32::from_rgb(22, 26, 35)
        };
        visuals.widgets.inactive.bg_stroke = Stroke::new(
            1.,
            if light {
                Color32::from_gray(215)
            } else {
                Color32::from_rgb(41, 46, 57)
            },
        );
        visuals.widgets.inactive.weak_bg_fill = visuals.widgets.inactive.bg_fill;
        visuals.widgets.hovered.weak_bg_fill = if light {
            Color32::from_rgb(228, 234, 246)
        } else {
            Color32::from_rgb(36, 41, 53)
        };
        visuals.widgets.active.weak_bg_fill = if light {
            Color32::from_rgb(216, 226, 247)
        } else {
            Color32::from_rgb(42, 51, 73)
        };
        ctx.set_visuals(visuals);
        self.record_shortcut(ctx);

        if !self.recording_shortcut && !widgets::is_open(ctx) {
            self.keyboard(ctx);
            if (self.date_picker || self.color_picker)
                && self.interaction_probe.is_none()
                && self.screenshot.is_none()
            {
                pickers::hold_input(ctx);
            }
        }
        let expanded = !self
            .input
            .trim_matches(quicktool_rust::core::js_space)
            .is_empty()
            || self.mode.is_some()
            || self.panel != Panel::None;
        let bg = if light {
            Color32::from_rgba_unmultiplied(249, 251, 255, (opacity * 255.) as u8)
        } else {
            Color32::from_rgba_unmultiplied(20, 24, 34, (opacity * 255.) as u8)
        };
        if !expanded {
            self.layout_snapshot = serde_json::json!({"selectedTop":0.});
        }
        let requested_height = if !expanded {
            84.
        } else {
            match self.panel {
                Panel::None if self.showing_help() => 642.,
                Panel::None => self.result_height,
                _ => 642.,
            }
        };
        let mut max_height = 642.;
        if self.glow.owner != 0 {
            match self.placement.update(
                self.glow.owner,
                self.place_requested,
                self.motion.allocated,
            ) {
                Ok(bounds) => {
                    max_height = bounds[2];
                    self.place_requested = false;
                }
                Err(error) => {
                    self.message(format!("窗口定位失败：{error}"));
                }
            }
        }
        let height = requested_height.min(max_height).max(84.);
        if self.motion.step(
            ctx.input(|i| i.time),
            height,
            visual["reduceMotion"] == true || self.screenshot.is_some(),
        ) {
            ctx.request_repaint_after(Duration::from_secs_f64(1. / 60.));
        }
        if self.glow.owner != 0 {
            let _ = self
                .placement
                .update(self.glow.owner, false, self.motion.allocated);
        } else if (self.motion.allocated - ctx.screen_rect().height()).abs() > 0.7 {
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(vec2(
                812.,
                self.motion.allocated,
            )));
        }
        if self.frames == 1 && self.main_visible {
            ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        }
        self.last_height = self.motion.visual;
        let backdrop_error = self.backdrop.as_mut().and_then(|glass| {
            glass
                .update_clipped(
                    self.main_visible,
                    effective_topmost,
                    !light,
                    blur as f32,
                    0,
                    8. * ctx.pixels_per_point(),
                    Some((self.motion.visual * ctx.pixels_per_point()).round() as i32),
                )
                .err()
        });
        if let Some(error) = backdrop_error {
            self.backdrop = None;
            self.message(format!("背景模糊已停止：{error}"));
        }
        ctx.style_mut(|style| {
            style.spacing.item_spacing = vec2(8., 5.);
            style.spacing.scroll.floating = false;
            style.spacing.scroll.foreground_color = false;
            style.spacing.scroll.bar_width = 6.;
            style.spacing.interact_size.y = 18.;
            style.spacing.button_padding = vec2(8., 5.);
            style.animation_time = if visual["reduceMotion"] == true {
                0.
            } else {
                0.14
            };
        });
        let shell_frame = Frame::new()
            .fill(bg)
            .corner_radius(8)
            .stroke(Stroke::new(
                1.,
                if light {
                    Color32::from_rgb(201, 210, 225)
                } else {
                    Color32::from_rgb(45, 51, 65)
                },
            ))
            .inner_margin(if self.panel == Panel::None {
                Margin {
                    left: 14,
                    right: 14,
                    top: 16,
                    bottom: 10,
                }
            } else if self.panel == Panel::History {
                // CSS inset:10px excludes the outer shell's stroke.
                Margin::same(9)
            } else {
                Margin::same(10)
            });
        CentralPanel::default().frame(Frame::NONE).show(ctx, |ui| {
            let shell = Rect::from_min_size(
                ui.max_rect().min,
                vec2(ui.max_rect().width(), self.motion.visual),
            );
            ui.painter()
                .add(shell_frame.paint(shell - shell_frame.total_margin()));
            let layout = Rect::from_min_size(
                shell.min,
                vec2(
                    shell.width(),
                    if expanded && self.panel == Panel::None {
                        max_height.min(642.)
                    } else {
                        height
                    },
                ),
            ) - shell_frame.total_margin();
            // The launcher shell has fixed viewport bounds, like the
            // original overflow:hidden element. A long row may grow its
            // own layout, but must not expand the outer frame off-screen.
            let mut content = ui.new_child(UiBuilder::new().max_rect(layout));
            content.set_clip_rect(content.clip_rect().intersect(shell));
            let ui = &mut content;
            if self.panel == Panel::None {
                self.topbar(ui, ctx);
            }
            if expanded {
                if self.panel == Panel::None {
                    ui.add_space(12.);
                }
                match self.panel {
                    Panel::None => {
                        let rect = ui.available_rect_before_wrap().expand2(vec2(7., 0.));
                        ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                            self.results(ui, ctx)
                        });
                    }
                    Panel::Settings => {
                        Frame::new()
                            .fill(if light {
                                Color32::from_rgb(246, 248, 253)
                            } else {
                                Color32::from_rgb(16, 20, 28)
                            })
                            .stroke(Stroke::new(
                                1.,
                                if light {
                                    Color32::from_gray(215)
                                } else {
                                    Color32::from_rgb(40, 45, 56)
                                },
                            ))
                            .corner_radius(16)
                            .inner_margin(0)
                            .show(ui, |ui| self.settings(ui));
                    }
                    Panel::History => {
                        Frame::new()
                            .fill(if light {
                                Color32::from_rgb(246, 248, 253)
                            } else {
                                Color32::from_rgb(16, 20, 28)
                            })
                            .stroke(Stroke::new(
                                ctx.pixels_per_point().floor() / ctx.pixels_per_point(),
                                if light {
                                    Color32::from_gray(215)
                                } else {
                                    Color32::from_rgb(40, 45, 56)
                                },
                            ))
                            .corner_radius(15)
                            .inner_margin(0)
                            .show(ui, |ui| self.history(ui, ctx));
                    }
                }
                if self.panel == Panel::None
                    && !self.showing_help()
                    && (self.result_height.min(max_height) - height).abs() > 0.7
                {
                    ctx.request_repaint();
                }
            }
        });
        if self.main_visible {
            widgets::show(ctx, self.glow.owner, effective_topmost);
        } else {
            widgets::close(ctx);
        }
        widgets::show_tooltip(ctx, self.glow.owner, effective_topmost, self.main_visible);
        let glow = visual["glowStrength"].as_f64().unwrap_or(0.55) as f32;
        if self.confirm_discard {
            Window::new("放弃设置修改？")
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label("尚有未保存的设置修改。");
                    ui.horizontal(|ui| {
                        if ui.button("继续编辑").clicked() {
                            self.confirm_discard = false;
                        }
                        if ui.button("放弃修改").clicked() {
                            self.restore_shortcut();
                            self.draft = self.store.settings.clone();
                            self.confirm_discard = false;
                            self.modules_page = false;
                            self.panel = Panel::None;
                            self.focus = true;
                        }
                    });
                });
        }
        if let Some((text, start)) = &self.toast {
            if start.elapsed() < Duration::from_secs(2) {
                Area::new(Id::new("toast"))
                    .anchor(Align2::CENTER_BOTTOM, [0., -8.])
                    .show(ctx, |ui| {
                        Frame::popup(ui.style()).show(ui, |ui| {
                            ui.label(text);
                        });
                    });
                ctx.request_repaint_after(Duration::from_millis(100));
            }
        }
        self.pins.set_shortcut_recording(self.recording_shortcut);
        self.pins.configure(light, blur as f32, opacity);
        self.pins.show(ctx);
        for update in self.pins.drain_history_updates() {
            let c = ctx.clone();
            if let Err(error) = self
                .store
                .enqueue_pin(update, move || c.request_repaint_of(ViewportId::ROOT))
            {
                self.message(format!("保存 Pin 历史失败：{error}"));
            }
        }
        for error in self.store.poll_pin_updates() {
            self.message(format!("保存 Pin 历史失败：{error}"));
        }
        self.glow.show(
            ctx,
            self.main_visible,
            self.pinned || visual["summonAlwaysOnTop"] == true,
            light,
            glow,
            visual["reduceMotion"] == true,
            vec2(ctx.screen_rect().width(), self.motion.visual),
        );
        if let Some(error) = self.glow.error.take() {
            self.message(format!("边缘 Glow 不可用：{error}"));
        }
        if self.main_visible {
            self.pickers(ctx);
        } else {
            self.date_picker = false;
            self.native_color.open = false;
            pickers::close_detached(ctx);
        }
        self.probe_tick(ctx, height);
        if let Some(path) = self.screenshot.clone() {
            ctx.request_repaint_after(Duration::from_millis(100));
            if self.frames >= 5 && !self.decoded_pending && !self.screenshot_sent {
                ctx.send_viewport_cmd(ViewportCommand::Screenshot(Default::default()));
                self.screenshot_sent = true;
                self.screenshot_requested = Some(Instant::now());
            }
            let screenshot = ctx.input(|i| {
                i.events.iter().find_map(|e| {
                    if let Event::Screenshot { image, .. } = e {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
            });
            if self
                .screenshot_requested
                .is_some_and(|at| at.elapsed() > Duration::from_secs(10))
            {
                eprintln!("Screenshot capture timed out: {}", path.display());
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            if let Some(image) = screenshot {
                let bytes: Vec<u8> = image
                    .pixels
                    .iter()
                    .flat_map(|c| c.to_srgba_unmultiplied())
                    .collect();
                if let Some(parent) = path.parent() {
                    if let Err(error) = std::fs::create_dir_all(parent) {
                        eprintln!("Screenshot directory failed {}: {error}", parent.display());
                    }
                }
                if let Err(e) = image::save_buffer(
                    &path,
                    &bytes,
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgba8,
                ) {
                    eprintln!("Screenshot failed: {e}");
                }
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }
        interaction_probe::tick(self, ctx);
        expanded_probe::tick(self, ctx);
        motion_probe::tick(self, ctx, frame.info().cpu_usage);
        search_layout_probe::tick(self, ctx);
        candidates_probe::tick(self, ctx);
    }
}
