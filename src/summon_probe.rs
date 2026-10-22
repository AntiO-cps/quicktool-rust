//! Opt-in regression using this application's actual native tray callback.
use super::*;

#[cfg(windows)]
mod native {
    use super::*;
    use std::{path::PathBuf, sync::mpsc};
    use windows_sys::Win32::{
        Foundation::{BOOL, HWND, LPARAM},
        System::Threading::GetCurrentProcessId,
        UI::WindowsAndMessaging::*,
    };

    enum Command {
        Hide,
        PrepareShow,
        Finish,
    }
    pub(crate) struct SummonProbe {
        path: PathBuf,
        started: Instant,
        tx: Option<mpsc::Sender<Command>>,
        rx: mpsc::Receiver<Command>,
    }
    impl SummonProbe {
        pub(crate) fn from_args() -> Option<Self> {
            let path = std::env::args_os()
                .skip_while(|arg| arg != "--verify-summon")
                .nth(1)?
                .into();
            let (tx, rx) = mpsc::channel();
            Some(Self {
                path,
                started: Instant::now(),
                tx: Some(tx),
                rx,
            })
        }
    }
    fn report(path: &std::path::Path, checks: &[Value], started: Instant) {
        let result = serde_json::json!({
            "passed": !checks.is_empty() && checks.iter().all(|c| c["passed"] == true),
            "checks": checks,
            "elapsedMs": started.elapsed().as_secs_f64() * 1000.,
            "method": "Actual QuickTool, native hidden/visible state and own-process tray callback; hideOnBlur enabled; no OS keyboard/mouse input or user data"
        });
        let bytes = serde_json::to_vec_pretty(&result).unwrap();
        quicktool_rust::storage::atomic_write(path, &bytes).unwrap();
    }
    unsafe extern "system" fn find_tray(hwnd: HWND, parameter: LPARAM) -> BOOL {
        unsafe {
            let mut pid = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid == GetCurrentProcessId() {
                let mut class = [0u16; 128];
                let len = GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32);
                if len > 0
                    && String::from_utf16_lossy(&class[..len as usize])
                        .starts_with("QuickToolRustTray")
                {
                    *(parameter as *mut isize) = hwnd as isize;
                    return 0;
                }
            }
            1
        }
    }
    fn wait_visible(hwnd: isize, visible: bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if unsafe { IsWindowVisible(hwnd as HWND) != 0 } == visible {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }
    pub(crate) fn tick(app: &mut QuickTool, ctx: &Context) {
        let Some(mut probe) = app.summon_probe.take() else {
            return;
        };
        if probe.started.elapsed() > Duration::from_secs(30) {
            report(
                &probe.path,
                &[serde_json::json!({"check":"summon watchdog","passed":false})],
                probe.started,
            );
            std::process::exit(1);
        }
        for command in probe.rx.try_iter() {
            match command {
                Command::Hide => {
                    app.main_visible = false;
                    ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Visible(false));
                }
                // Exercise a stale previous-frame focus state while the actual
                // HWND is hidden; Show must override hide-on-blur in this frame.
                Command::PrepareShow => app.was_focused = true,
                Command::Finish => {
                    ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Close);
                    return;
                }
            }
        }
        if app.frames >= 5 && app.glow.owner != 0 && probe.tx.is_some() {
            let mut tray = 0isize;
            unsafe {
                EnumWindows(Some(find_tray), &mut tray as *mut isize as LPARAM);
            }
            if tray != 0 {
                let tx = probe.tx.take().unwrap();
                app.main_visible = false;
                ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Visible(false));
                let root = app.glow.owner;
                let context = ctx.clone();
                let path = probe.path.clone();
                let started = probe.started;
                std::thread::spawn(move || {
                    let mut checks = Vec::new();
                    for round in 1..=3 {
                        if round > 1 {
                            tx.send(Command::Hide).unwrap();
                            context.request_repaint_of(ViewportId::ROOT);
                        }
                        let hidden = wait_visible(root, false);
                        std::thread::sleep(Duration::from_millis(150));
                        tx.send(Command::PrepareShow).unwrap();
                        let posted = unsafe {
                            PostMessageW(tray as HWND, WM_APP + 1, 1, WM_LBUTTONUP as LPARAM) != 0
                        };
                        let visible = wait_visible(root, true);
                        std::thread::sleep(Duration::from_millis(150));
                        let remains_visible = unsafe { IsWindowVisible(root as HWND) != 0 };
                        let passed = hidden && posted && visible && remains_visible;
                        checks.push(serde_json::json!({"check":"native tray restores hidden panel without blur cancellation","round":round,"passed":passed,"hiddenBefore":hidden,"callbackPosted":posted,"visibleAfter":visible,"remainsVisible":remains_visible}));
                        if !passed {
                            report(&path, &checks, started);
                            std::process::exit(1);
                        }
                    }
                    report(&path, &checks, started);
                    tx.send(Command::Finish).unwrap();
                    context.request_repaint_of(ViewportId::ROOT);
                });
            }
        }
        app.summon_probe = Some(probe);
    }
}

#[cfg(windows)]
pub(super) use native::{SummonProbe, tick};

#[cfg(not(windows))]
pub(super) struct SummonProbe;
#[cfg(not(windows))]
impl SummonProbe {
    pub(super) fn from_args() -> Option<Self> {
        None
    }
}
#[cfg(not(windows))]
pub(super) fn tick(_: &mut QuickTool, _: &Context) {}
