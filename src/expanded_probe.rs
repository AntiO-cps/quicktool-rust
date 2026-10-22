//! Normal-window expansion regression; native styles and clipping stay enabled.
use super::*;
#[cfg(windows)]
pub(super) struct ExpandedProbe {
    path: std::path::PathBuf,
    stage: usize,
    changed: Option<Instant>,
    started: Instant,
    checks: Vec<Value>,
}
#[cfg(windows)]
impl ExpandedProbe {
    pub(super) fn from_args() -> Option<Self> {
        Some(Self {
            path: std::env::args_os()
                .skip_while(|a| a != "--verify-expanded")
                .nth(1)?
                .into(),
            stage: 0,
            changed: None,
            started: Instant::now(),
            checks: vec![],
        })
    }
}
#[cfg(windows)]
pub(super) fn tick(app: &mut QuickTool, ctx: &Context) {
    use windows_sys::Win32::{Foundation::*, Graphics::Gdi::*, UI::WindowsAndMessaging::*};
    const CASES: &[&str] = &[
        "settings",
        "candidates",
        "history",
        "compact",
        "settings again",
        "candidates again",
    ];
    let Some(mut probe) = app.expanded_probe.take() else {
        return;
    };
    if app.frames < 5 || app.glow.owner == 0 {
        ctx.request_repaint_after(Duration::from_millis(20));
        app.expanded_probe = Some(probe);
        return;
    }
    if probe
        .changed
        .is_some_and(|t| t.elapsed() >= Duration::from_millis(500))
    {
        unsafe {
            let hwnd = app.glow.owner as HWND;
            let mut client: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut client);
            let region = CreateRectRgn(0, 0, 0, 0);
            let kind = GetWindowRgn(hwnd, region);
            let mut bounds: RECT = std::mem::zeroed();
            GetRgnBox(region, &mut bounds);
            let covers = kind == 0
                || (bounds.right >= client.right - 1 && bounds.bottom >= client.bottom - 1);
            DeleteObject(region);
            let expanded = probe.stage != 3;
            let rows = app.visible_indices().len();
            let mut point = POINT {
                x: (20. * ctx.pixels_per_point()) as i32,
                y: (140. * ctx.pixels_per_point()) as i32,
            };
            let mut rgb = None;
            let mut hit = true;
            if expanded {
                ClientToScreen(hwnd, &mut point);
                hit = GetAncestor(WindowFromPoint(point), GA_ROOT) == hwnd;
                if hit {
                    let dc = GetDC(std::ptr::null_mut());
                    let pixel = GetPixel(dc, point.x, point.y);
                    ReleaseDC(std::ptr::null_mut(), dc);
                    rgb = Some([pixel & 255, (pixel >> 8) & 255, (pixel >> 16) & 255]);
                }
            }
            let opaque_panel = matches!(probe.stage, 0 | 2 | 4);
            let content = !opaque_panel || rgb == Some([16, 20, 28]);
            let candidates = !matches!(probe.stage, 1 | 5) || rows > 1;
            let passed =
                covers && hit && content && candidates && (!expanded || client.bottom > 200);
            probe.checks.push(serde_json::json!({"case":CASES[probe.stage],"passed":passed,"client":[client.right,client.bottom],"region":[bounds.right,bounds.bottom],"regionCoversClient":covers,"bodyHitIsMainWindow":hit,"bodyRgb":rgb,"candidateRows":rows}));
        }
        probe.stage += 1;
        probe.changed = None;
    }
    if probe.stage == CASES.len() || probe.started.elapsed() > Duration::from_secs(20) {
        let passed = probe.stage == CASES.len() && probe.checks.iter().all(|c| c["passed"] == true);
        let report = serde_json::json!({"passed":passed,"checks":probe.checks,"method":"Actual production window, normal native styling and backdrop enabled; native clip bounds, body hit and own-window-only body pixel; no screenshot-mode bypass or OS input"});
        quicktool_rust::storage::atomic_write(
            &probe.path,
            &serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Close);
        return;
    }
    if probe.changed.is_none() {
        app.panel = match probe.stage {
            0 | 4 => Panel::Settings,
            2 => Panel::History,
            _ => Panel::None,
        };
        app.input = if matches!(probe.stage, 1 | 5) {
            "0xFF".into()
        } else {
            String::new()
        };
        app.mode = None;
        app.submode = None;
        app.draft = app.store.settings.clone();
        app.rebuild();
        probe.changed = Some(Instant::now());
    }
    ctx.request_repaint_after(Duration::from_millis(20));
    app.expanded_probe = Some(probe);
}
#[cfg(not(windows))]
pub(super) struct ExpandedProbe;
#[cfg(not(windows))]
impl ExpandedProbe {
    pub(super) fn from_args() -> Option<Self> {
        None
    }
}
#[cfg(not(windows))]
pub(super) fn tick(_: &mut QuickTool, _: &Context) {}
