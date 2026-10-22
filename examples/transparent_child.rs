//! Test native composition against our own green host, without desktop capture.
#[cfg(windows)]
fn main() -> eframe::Result {
    use eframe::egui::*;
    use quicktool_rust::platform;
    use windows_sys::Win32::{Foundation::*, Graphics::Gdi::*, UI::WindowsAndMessaging::*};
    struct App {
        frames: usize,
        report: std::path::PathBuf,
    }
    impl eframe::App for App {
        fn clear_color(&self, _: &Visuals) -> [f32; 4] {
            [0.; 4]
        }
        fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
            self.frames += 1;
            CentralPanel::default()
                .frame(Frame::new().fill(Color32::from_rgb(40, 180, 90)))
                .show(ctx, |_| {});
            let origin = ctx.input(|i| i.viewport().outer_rect.unwrap().min);
            ctx.show_viewport_deferred(
                ViewportId::from_hash_of("transparent-test"),
                ViewportBuilder::default()
                    .with_title("QuickTool transparent child test")
                    .with_position(origin + vec2(40., 40.))
                    .with_inner_size(vec2(160., 100.))
                    .with_decorations(false)
                    .with_transparent(true)
                    .with_mouse_passthrough(true)
                    .with_active(false)
                    .with_taskbar(false)
                    .with_window_level(WindowLevel::AlwaysOnTop),
                |child, _| {
                    child.layer_painter(LayerId::background()).rect_filled(
                        Rect::from_min_size(pos2(60., 30.), vec2(40., 40.)),
                        0,
                        Color32::RED,
                    );
                },
            );
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            if self.frames == 10 {
                let child = platform::find_window("QuickTool transparent child test").unwrap();
                unsafe { platform::apply_window_style(child, true, 0, 8).unwrap() };
            }
            if self.frames == 15 {
                let window =
                    platform::find_window("QuickTool transparent child test").unwrap() as HWND;
                unsafe {
                    let mut point = POINT { x: 10, y: 10 };
                    ClientToScreen(window, &mut point);
                    let at = GetAncestor(WindowFromPoint(point), GA_ROOT);
                    let mut pid = 0;
                    GetWindowThreadProcessId(at, &mut pid);
                    assert_eq!(
                        pid,
                        std::process::id(),
                        "Sample must be covered by our own window"
                    );
                    let mut bounds: RECT = std::mem::zeroed();
                    GetClientRect(window, &mut bounds);
                    let mut center = POINT {
                        x: bounds.right / 2,
                        y: bounds.bottom / 2,
                    };
                    ClientToScreen(window, &mut center);
                    let dc = GetDC(std::ptr::null_mut());
                    let pixel = GetPixel(dc, point.x, point.y);
                    let opaque = GetPixel(dc, center.x, center.y);
                    ReleaseDC(std::ptr::null_mut(), dc);
                    let rgb = [pixel & 255, (pixel >> 8) & 255, (pixel >> 16) & 255];
                    let opaque_rgb = [opaque & 255, (opaque >> 8) & 255, (opaque >> 16) & 255];
                    let passed = rgb == [40, 180, 90] && opaque_rgb == [255, 0, 0];
                    std::fs::write(&self.report,serde_json::to_vec_pretty(&serde_json::json!({"passed":passed,"expected":[40,180,90],"actual":rgb,"opaqueExpected":[255,0,0],"opaqueActual":opaque_rgb,"method":"Native screen pixels strictly inside own transparent child over own opaque green host; no other desktop contents recorded"})).unwrap()).unwrap();
                }
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }
    }
    let report = std::env::args_os().nth(1).expect("Report path").into();
    eframe::run_native(
        "QuickTool green composition host",
        eframe::NativeOptions {
            renderer: eframe::Renderer::Glow,
            multisampling: 4,
            viewport: ViewportBuilder::default()
                .with_inner_size([320., 200.])
                .with_position([100., 100.])
                .with_transparent(true)
                .with_decorations(false)
                .with_window_level(WindowLevel::AlwaysOnTop),
            ..Default::default()
        },
        Box::new(move |_| Ok(Box::new(App { frames: 0, report }))),
    )
}
#[cfg(not(windows))]
fn main() {}
