//! Native Windows regression: background work must wake a hidden viewport.
//! No OS keyboard/mouse input, clipboard, settings, or external application use.
#[cfg(windows)]
fn main() -> eframe::Result {
    use eframe::egui::{self, ViewportCommand, ViewportId};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::{sync::mpsc, time::Duration};
    use windows_sys::Win32::UI::WindowsAndMessaging::IsWindowVisible;
    enum Event {
        Show,
        Finish,
    }
    struct App {
        frames: usize,
        rx: Option<mpsc::Receiver<Event>>,
        report: std::path::PathBuf,
    }
    impl eframe::App for App {
        fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
            self.frames += 1;
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.label("Hidden viewport wake regression");
            });
            if self.frames < 3 {
                ctx.request_repaint();
            } else if self.rx.is_none() {
                let RawWindowHandle::Win32(handle) = frame.window_handle().unwrap().as_raw() else {
                    panic!("Expected Windows viewport");
                };
                let hwnd = handle.hwnd.get();
                ctx.send_viewport_cmd(ViewportCommand::Visible(false));
                let (tx, rx) = mpsc::channel();
                self.rx = Some(rx);
                let ctx = ctx.clone();
                let report = self.report.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(400));
                    let hidden = unsafe { IsWindowVisible(hwnd as _) == 0 };
                    tx.send(Event::Show).unwrap();
                    ctx.request_repaint_of(ViewportId::ROOT);
                    std::thread::sleep(Duration::from_millis(800));
                    let visible = unsafe { IsWindowVisible(hwnd as _) != 0 };
                    let passed = hidden && visible;
                    let result = serde_json::json!({"passed":passed,"hiddenBeforeWake":hidden,"visibleAfterWake":visible,"method":"Native viewport, delayed worker channel and ROOT repaint; no OS input"});
                    std::fs::write(report, serde_json::to_vec_pretty(&result).unwrap()).unwrap();
                    if !passed {
                        std::process::exit(1);
                    }
                    tx.send(Event::Finish).unwrap();
                    ctx.request_repaint_of(ViewportId::ROOT);
                });
            }
            if let Some(rx) = &self.rx {
                for event in rx.try_iter() {
                    match event {
                        Event::Show => ctx.send_viewport_cmd(ViewportCommand::Visible(true)),
                        Event::Finish => ctx.send_viewport_cmd(ViewportCommand::Close),
                    }
                }
            }
        }
    }
    let report = std::env::args_os()
        .nth(1)
        .expect("Supply report JSON path")
        .into();
    eframe::run_native(
        "QuickTool hidden wake regression",
        eframe::NativeOptions {
            renderer: eframe::Renderer::Glow,
            viewport: egui::ViewportBuilder::default().with_inner_size([320., 100.]),
            ..Default::default()
        },
        Box::new(move |_| {
            Ok(Box::new(App {
                frames: 0,
                rx: None,
                report,
            }))
        }),
    )
}

#[cfg(not(windows))]
fn main() {
    eprintln!("This native viewport regression requires Windows");
}
