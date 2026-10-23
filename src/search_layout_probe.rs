//! Isolated real-widget input and frame geometry regression; never injects OS input.
use super::*;

const QUERIES: &[&str] = &[
    "a",
    "ab",
    "abc",
    "ch",
    "CH",
    "1",
    "12",
    "1+1",
    "hello",
    "#fff",
    "0xFF",
    "/base64 hello",
    "/国家 ch",
    "",
];
const STAGE_MS: u64 = 800;
const TAIL_MS: f64 = 450.;
const RAPID_QUERIES: &[&str] = &["h", "he", "hel", "hell", "hello", "hello", "c", "ch", "ch"];

pub(super) struct SearchLayoutProbe {
    path: std::path::PathBuf,
    stage: usize,
    started: Option<Instant>,
    trace: Vec<Value>,
    checks: Vec<Value>,
    rapid: bool,
}

impl SearchLayoutProbe {
    pub(super) fn from_args() -> Option<Self> {
        Some(Self {
            path: std::env::args_os()
                .skip_while(|arg| arg != "--verify-search-layout")
                .nth(1)?
                .into(),
            stage: 0,
            started: None,
            trace: vec![],
            checks: vec![],
            rapid: std::env::args().any(|arg| arg == "--rapid-search-layout"),
        })
    }
    fn queries(&self) -> &'static [&'static str] {
        if self.rapid { RAPID_QUERIES } else { QUERIES }
    }
    fn stage_ms(&self) -> u64 {
        if self.rapid && matches!(self.stage, 0..=3 | 6) {
            60
        } else {
            STAGE_MS
        }
    }
}

pub(super) fn raw_input(app: &mut QuickTool, raw: &mut RawInput) {
    if raw.viewport_id != ViewportId::ROOT {
        return;
    }
    let Some(mut probe) = app.search_layout_probe.take() else {
        return;
    };
    // Keep unrelated real pointer activity from changing selection during a
    // deterministic keyboard/layout trace. This only filters this app's input.
    raw.events.retain(|event| {
        !matches!(
            event,
            Event::PointerMoved(_) | Event::PointerButton { .. } | Event::MouseWheel { .. }
        )
    });
    raw.events.push(Event::PointerGone);
    if app.frames >= 12 && probe.started.is_none() && probe.stage < probe.queries().len() {
        // Select through the same editor action used by Summon/Ctrl+A. Insert
        // through RawInput so rebuild, completion and geometry use normal code.
        app.panel = Panel::None;
        app.mode = None;
        app.submode = None;
        app.tab = 0;
        app.focus = true;
        app.date_picker = false;
        app.color_picker = false;
        app.native_color.open = false;
        let ctx = app.context.clone();
        ctx.memory_mut(|memory| memory.request_focus(Id::new("quicktool-search")));
        app.large_input.action(&mut app.input, &ctx, "all");
        raw.modifiers = Modifiers::NONE;
        let query = probe.queries()[probe.stage];
        raw.events.push(if query.is_empty() {
            Event::Key {
                key: Key::Backspace,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }
        } else {
            Event::Text(query.to_owned())
        });
        probe.started = Some(Instant::now());
    }
    app.search_layout_probe = Some(probe);
}

fn native_bounds(owner: isize) -> Option<[i32; 4]> {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::{Foundation::RECT, UI::WindowsAndMessaging::GetWindowRect};
        let mut rect: RECT = std::mem::zeroed();
        if owner != 0 && GetWindowRect(owner as _, &mut rect) != 0 {
            return Some([
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
            ]);
        }
    }
    #[cfg(not(windows))]
    let _ = owner;
    None
}

fn range(values: impl Iterator<Item = f64>) -> Option<f64> {
    let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
    for value in values.filter(|v| v.is_finite()) {
        min = min.min(value);
        max = max.max(value);
    }
    (min.is_finite() && max.is_finite()).then_some(max - min)
}

fn selected_top(snapshot: &Value) -> Option<f64> {
    // Root's snapshot can use explicit selectedTop or a serialized rectangle.
    snapshot["selectedTop"]
        .as_f64()
        .or_else(|| snapshot["selected_top"].as_f64())
        .or_else(|| snapshot["selectedRect"]["min"][1].as_f64())
        .or_else(|| snapshot["selectedRect"][1].as_f64())
}

fn summarize(query: &str, trace: &[Value]) -> Value {
    let tail: Vec<_> = trace
        .iter()
        .filter(|frame| frame["elapsedMs"].as_f64().unwrap_or(0.) >= TAIL_MS)
        .collect();
    let target = range(tail.iter().filter_map(|v| v["target"].as_f64()));
    let scroll = range(tail.iter().filter_map(|v| v["resultScroll"].as_f64()));
    let top = range(tail.iter().filter_map(|v| selected_top(&v["layout"])));
    let bounds: Vec<_> = trace
        .iter()
        .filter_map(|v| v["nativeBounds"].as_array())
        .collect();
    let resizes = bounds
        .windows(2)
        .filter(|pair| pair[0][2..] != pair[1][2..])
        .count();
    let xy_fixed = !bounds.is_empty() && bounds.iter().all(|b| b[..2] == bounds[0][..2]);
    let mut row_heights = std::collections::HashMap::<String, (f64, f64)>::new();
    for frame in trace {
        for rect in frame["layout"]["rowRects"].as_array().into_iter().flatten() {
            if let (Some(index), Some(height)) = (rect[0].as_u64(), rect[1].as_f64()) {
                if let Some(key) = frame["rowKeys"][index as usize].as_str() {
                    let range = row_heights
                        .entry(key.to_string())
                        .or_insert((height, height));
                    range.0 = range.0.min(height);
                    range.1 = range.1.max(height);
                }
            }
        }
    }
    let row_height_stable = row_heights.values().all(|(min, max)| max - min < 0.1);
    // Missing selected geometry is reported as unmeasured, never silently passed.
    let passed = !tail.is_empty()
        && [target, scroll, top]
            .into_iter()
            .all(|v| v.is_some_and(|v| v < 1.))
        && xy_fixed
        && resizes <= 2
        && row_height_stable
        && trace.iter().all(|v| v["input"].as_str() == Some(query));
    serde_json::json!({
        "query":query,"passed":passed,"frames":trace.len(),"tailFrames":tail.len(),
        "tailTargetRange":target,"tailScrollRange":scroll,"tailSelectedTopRange":top,
        "nativeResizeCount":resizes,"nativeXYConstant":xy_fixed,"rowHeightStable":row_height_stable,"trace":trace
    })
}

pub(super) fn tick(app: &mut QuickTool, ctx: &Context) {
    let Some(mut probe) = app.search_layout_probe.take() else {
        return;
    };
    if let Some(started) = probe.started {
        let elapsed = started.elapsed();
        probe.trace.push(serde_json::json!({
            "frame":app.frames,"input":app.input,"elapsedMs":elapsed.as_secs_f64()*1000.,
            "resultHeight":app.result_height,"target":app.motion.target,
            "visual":app.motion.visual,"allocated":app.motion.allocated,
            "resultScroll":app.result_scroll,"layout":app.layout_snapshot,
            "selected":app.selected,"rowKeys":app.rows.iter().map(|r|format!("{}|{}",r.tool,r.item.title)).collect::<Vec<_>>(),
            "filePending":app.files_pending,"fileQuery":app.file_query,"appCount":app.apps.len(),
            "nativeBounds":native_bounds(app.glow.owner)
        }));
        if elapsed >= Duration::from_millis(probe.stage_ms()) {
            let query = probe.queries()[probe.stage];
            let mut check = summarize(query, &probe.trace);
            let typing = probe.stage_ms() < STAGE_MS;
            if typing {
                check["passed"] = serde_json::json!(
                    check["nativeXYConstant"] == true
                        && probe
                            .trace
                            .iter()
                            .all(|v| v["input"].as_str() == Some(query))
                );
            }
            check["phase"] = serde_json::json!(if typing {
                "typing (input/anchor only)"
            } else {
                "settled (all geometry checks)"
            });
            probe.checks.push(check);
            probe.trace.clear();
            probe.stage += 1;
            probe.started = None;
        }
    }
    if probe.stage == probe.queries().len() {
        let passed = probe.checks.iter().all(|stage| stage["passed"] == true);
        let report = serde_json::json!({
            "passed":passed,"version":env!("CARGO_PKG_VERSION"),"checks":probe.checks,
            "stageDurationMs":STAGE_MS,"tailDurationMs":350,"rapid":probe.rapid,
            "method":"Fourteen queries injected as egui RawInput Text/Backspace after real editor Select All; no OS input, clipboard or direct input assignment. Records all root frames and own GetWindowRect. Tail checks use the last 350 ms; native XY normalized to each stage's first frame. Missing geometry fails measurement."
        });
        let result = serde_json::to_vec_pretty(&report)
            .map_err(std::io::Error::other)
            .and_then(|data| quicktool_rust::storage::atomic_write(&probe.path, &data));
        match result {
            Ok(()) => eprintln!(
                "Search layout probe saved: {} (passed={passed})",
                probe.path.display()
            ),
            Err(error) => eprintln!(
                "Search layout probe failed {}: {error}",
                probe.path.display()
            ),
        }
        ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Close);
        return;
    }
    ctx.request_repaint_after(Duration::from_millis(16));
    app.search_layout_probe = Some(probe);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unstable_tail_and_missing_geometry_cannot_pass() {
        let frame = |target: f64, scroll: f64, top: Option<f64>| serde_json::json!({"input":"a","elapsedMs":500.,"target":target,"resultScroll":scroll,"layout":{"selectedTop":top},"nativeBounds":[1,2,812,642]});
        assert!(
            summarize(
                "a",
                &[frame(642., 37., Some(158.)), frame(642., 37., Some(158.))]
            )["passed"]
                == true
        );
        assert!(
            summarize(
                "a",
                &[frame(642., 37., Some(158.)), frame(630., 37., Some(158.))]
            )["passed"]
                == false
        );
        assert!(summarize("a", &[frame(642., 37., None)])["passed"] == false);
    }
}
