//! Isolated application-driven frame and native-resize measurements (no OS input).
use super::*;
pub(super) struct MotionProbe {
    path: std::path::PathBuf,
    stage: usize,
    changed: Option<Instant>,
    previous: Option<Instant>,
    bounds: Option<[i32; 4]>,
    intervals: Vec<f64>,
    cpu: Vec<f64>,
    resizes: usize,
    checks: Vec<Value>,
}
impl MotionProbe {
    pub(super) fn from_args() -> Option<Self> {
        Some(Self {
            path: std::env::args_os()
                .skip_while(|a| a != "--verify-motion")
                .nth(1)?
                .into(),
            stage: 0,
            changed: None,
            previous: None,
            bounds: None,
            intervals: vec![],
            cpu: vec![],
            resizes: 0,
            checks: vec![],
        })
    }
}
fn percentile(values: &[f64], p: f64) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted
        .get(((sorted.len().saturating_sub(1)) as f64 * p).round() as usize)
        .copied()
        .unwrap_or(0.)
}
pub(super) fn tick(app: &mut QuickTool, ctx: &Context, cpu: Option<f32>) {
    let Some(mut probe) = app.motion_probe.take() else {
        return;
    };
    if app.frames < 12 || app.glow.owner == 0 {
        ctx.request_repaint_after(Duration::from_millis(16));
        app.motion_probe = Some(probe);
        return;
    }
    let now = Instant::now();
    if probe.changed.is_some() {
        if let Some(previous) = probe.previous {
            probe
                .intervals
                .push(now.duration_since(previous).as_secs_f64() * 1000.);
        }
        if let Some(cpu) = cpu {
            probe.cpu.push(cpu as f64 * 1000.);
        }
        #[cfg(windows)]
        unsafe {
            use windows_sys::Win32::{Foundation::RECT, UI::WindowsAndMessaging::GetWindowRect};
            let mut r: RECT = std::mem::zeroed();
            GetWindowRect(app.glow.owner as _, &mut r);
            let bounds = [r.left, r.top, r.right - r.left, r.bottom - r.top];
            if probe.bounds.is_some_and(|old| old[2..] != bounds[2..]) {
                probe.resizes += 1;
            }
            probe.bounds = Some(bounds);
        }
        probe.previous = Some(now);
    }
    if probe
        .changed
        .is_some_and(|at| at.elapsed() >= Duration::from_millis(650))
    {
        probe.checks.push(serde_json::json!({"stage":probe.stage,"frames":probe.intervals.len(),"nativeResizes":probe.resizes,
            "frameIntervalMs":{"p50":percentile(&probe.intervals,0.5),"p95":percentile(&probe.intervals,0.95),"max":percentile(&probe.intervals,1.)},
            "cpuUiMs":{"p50":percentile(&probe.cpu,0.5),"p95":percentile(&probe.cpu,0.95)},"bounds":probe.bounds}));
        probe.stage += 1;
        probe.changed = None;
    }
    if probe.stage == 8 {
        let passed = probe.checks.len() == 8
            && probe.checks.iter().all(|c| {
                c["nativeResizes"].as_u64().is_some_and(|n| n <= 2)
                    && c["frames"].as_u64().is_some_and(|n| n > 10)
            });
        let report = serde_json::json!({"passed":passed,"profile":if cfg!(debug_assertions){"debug"}else{"release"},"version":env!("CARGO_PKG_VERSION"),"checks":probe.checks,
            "method":"Eight alternating real app transitions with normal motion; own native bounds and eframe CPU UI duration, root frame intervals include rendering/event scheduling but not OS display presentation; no OS input or user data"});
        quicktool_rust::storage::atomic_write(
            &probe.path,
            &serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Close);
        return;
    }
    if probe.changed.is_none() {
        app.panel = if probe.stage % 4 == 2 {
            Panel::Settings
        } else {
            Panel::None
        };
        app.input = if probe.stage % 4 == 0 {
            "0xFF".into()
        } else {
            String::new()
        };
        app.mode = None;
        app.submode = None;
        app.draft = app.store.settings.clone();
        app.rebuild();
        probe.changed = Some(now);
        probe.previous = None;
        probe.intervals.clear();
        probe.cpu.clear();
        probe.resizes = 0;
    }
    ctx.request_repaint_after(Duration::from_millis(16));
    app.motion_probe = Some(probe);
}
