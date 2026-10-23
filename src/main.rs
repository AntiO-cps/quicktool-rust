#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod cli;
mod ui;
pub static PROCESS_STARTED: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
fn main() -> eframe::Result {
    let _ = PROCESS_STARTED.set(std::time::Instant::now());
    if let Some(code) = cli::run_import(&std::env::args_os().collect::<Vec<_>>()) {
        std::process::exit(code);
    }
    if std::env::args().any(|a| a == "--version") {
        println!("QuickTool Rust {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let icon = image::load_from_memory(include_bytes!("../assets/assets/quicktool.ico"))
        .ok()
        .map(|icon| {
            let rgba = icon.into_rgba8();
            eframe::egui::IconData {
                width: rgba.width(),
                height: rgba.height(),
                rgba: rgba.into_raw(),
            }
        });
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("QuickTool Rust")
        .with_inner_size([812., 84.])
        .with_min_inner_size([640., 84.])
        .with_decorations(false)
        .with_transparent(true)
        .with_visible(false)
        .with_resizable(false);
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Glow,
        vsync: true,
        multisampling: 4,
        centered: false,
        ..Default::default()
    };
    eframe::run_native(
        "QuickTool Rust",
        options,
        Box::new(|cc| Ok(Box::new(ui::QuickTool::new(cc)))),
    )
}
