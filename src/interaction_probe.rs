//! Opt-in, isolated whole-app event regression. No OS input or clipboard operations.
use super::*;
use std::path::PathBuf;

pub(super) struct InteractionProbe {
    path: PathBuf,
    step: usize,
    armed: bool,
    checks: Vec<Value>,
    started: Instant,
    expected_selection: usize,
    expected_input: String,
}
impl InteractionProbe {
    pub(super) fn from_args() -> Option<Self> {
        let path = std::env::args_os()
            .skip_while(|a| a != "--verify-ui")
            .nth(1)?;
        Some(Self {
            path: path.into(),
            step: 0,
            armed: false,
            checks: vec![],
            started: Instant::now(),
            expected_selection: 0,
            expected_input: String::new(),
        })
    }
}
const LABELS: &[&str] = &[
    "Tab completes command",
    "Enter enters Base64 mode",
    "Escape clears input before modes",
    "Escape removes submode",
    "Escape removes mode",
    "ArrowDown selects next visible row",
    "ArrowUp selects first visible row",
    "Ctrl+Tab advances tab",
    "Ctrl+Shift+Tab returns tab",
    "Escape prompts for changed settings",
    "Escape cancels discard prompt",
    "Escape closes unchanged settings",
    "Escape closes date picker",
    "Escape closes native color popup first",
    "Escape closes color panel",
    "Calendar ArrowRight crosses leap day",
    "Calendar Tab focuses hour",
    "Calendar hour wraps 23 to 00",
    "Calendar Tab focuses minute",
    "Calendar minute wraps 59 to 00",
    "Escape closes edited calendar",
    "Backspace suppresses ghost without deleting input",
    "Hiding owner dismisses native popups and preserves color panel state",
    "Summon closes history and selects all search text",
    "Keyboard category change resets selection and focuses search",
    "Summon protects unsaved settings",
    "Ctrl+Shift+P works in settings",
    "Escape closes history and focuses search",
    "History hides on blur while settings, recording, pin and popups protect focus",
    "Lowercase ch ranks Swiss country card first alongside mixed tools",
    "Run category excludes country cards",
    "Country alternative activates its own completion metadata",
];
pub(super) fn raw_input(app: &mut QuickTool, input: &mut RawInput) {
    let Some(mut probe) = app.interaction_probe.take() else {
        return;
    };
    if app.frames >= 5 && probe.step < LABELS.len() && !probe.armed {
        let mut modifiers = Modifiers::NONE;
        let key = match probe.step {
            0 => {
                app.panel = Panel::None;
                app.mode = None;
                app.submode = None;
                app.input = "/bas".into();
                app.tab = 0;
                app.rebuild();
                Key::Tab
            }
            1 => Key::Enter,
            2 => {
                app.mode = Some("base64".into());
                app.submode = Some("encode".into());
                app.input = "hello".into();
                app.rebuild();
                Key::Escape
            }
            3 | 4 => Key::Escape,
            5 => {
                app.mode = None;
                app.submode = None;
                app.input = "0xFF".into();
                app.tab = 0;
                app.rebuild();
                let visible = app.visible_indices();
                app.selected = visible.first().copied().unwrap_or(0);
                probe.expected_selection = visible.get(1).copied().unwrap_or(app.selected);
                Key::ArrowDown
            }
            6 => {
                probe.expected_selection = app.visible_indices().first().copied().unwrap_or(0);
                Key::ArrowUp
            }
            7 => {
                modifiers = Modifiers::CTRL;
                Key::Tab
            }
            8 => {
                modifiers = Modifiers::CTRL | Modifiers::SHIFT;
                Key::Tab
            }
            9 => {
                app.panel = Panel::Settings;
                app.draft = app.store.settings.clone();
                app.draft["theme"] = serde_json::json!(if app.store.settings["theme"] == "light" {
                    "dark"
                } else {
                    "light"
                });
                Key::Escape
            }
            10 => Key::Escape,
            11 => {
                app.draft = app.store.settings.clone();
                Key::Escape
            }
            12 => {
                app.date_picker = true;
                Key::Escape
            }
            13 => {
                app.color_picker = true;
                app.native_color.open = true;
                Key::Escape
            }
            15 => {
                app.date_picker = true;
                app.native_date.reset();
                app.date_value = "2024-02-29 23:59".into();
                Key::ArrowRight
            }
            16 | 18 => Key::Tab,
            17 | 19 => Key::ArrowDown,
            21 => {
                app.mode = None;
                app.submode = None;
                app.input = "/bas".into();
                app.completion_suppressed = None;
                app.rebuild();
                Key::Backspace
            }
            22 => {
                app.date_picker = true;
                app.color_picker = true;
                app.native_color.open = true;
                app.main_visible = false;
                Key::Escape
            }
            23 => {
                app.color_picker = false;
                app.panel = Panel::History;
                app.input = "保留🙂abc".into();
                app.rebuild();
                let ctx = app.context.clone();
                app.show_main(&ctx);
                Key::F20
            }
            24 => {
                app.input = "0xFF".into();
                app.rebuild();
                app.tab = 0;
                app.selected = *app.visible_indices().last().unwrap();
                modifiers = Modifiers::CTRL | Modifiers::SHIFT;
                Key::Tab
            }
            25 => {
                app.panel = Panel::Settings;
                app.draft = app.store.settings.clone();
                app.draft["glowStrength"] = serde_json::json!(0.123);
                let ctx = app.context.clone();
                app.show_main(&ctx);
                Key::F20
            }
            26 => {
                app.confirm_discard = false;
                app.pinned = false;
                modifiers = Modifiers::CTRL | Modifiers::SHIFT;
                Key::P
            }
            27 => {
                app.panel = Panel::History;
                Key::Escape
            }
            28 => Key::F20,
            29 => {
                app.mode = None;
                app.panel = Panel::None;
                app.tab = 0;
                app.input = "ch".into();
                app.rebuild();
                Key::F20
            }
            30 => {
                modifiers = Modifiers::CTRL;
                Key::Tab
            }
            31 => {
                app.mode = Some("country".into());
                app.tab = 0;
                app.input = "congo".into();
                app.rebuild();
                if let Some((index, country)) = app.rows.iter().enumerate().find_map(|(i, r)| {
                    r.country
                        .as_ref()
                        .filter(|(_, primary)| !*primary)
                        .map(|(c, _)| (i, c.clone()))
                }) {
                    probe.expected_input = country["nameEn"].as_str().unwrap_or("").into();
                    let ctx = app.context.clone();
                    app.activate(&ctx, index);
                }
                Key::F20
            }
            _ => Key::Escape,
        };
        input.modifiers = modifiers;
        input.events.push(Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers,
        });
        input.events.push(Event::Key {
            key,
            physical_key: Some(key),
            pressed: false,
            repeat: false,
            modifiers,
        });
        probe.armed = true;
    }
    app.interaction_probe = Some(probe);
}
pub(super) fn tick(app: &mut QuickTool, ctx: &Context) {
    let Some(mut probe) = app.interaction_probe.take() else {
        return;
    };
    if probe.armed {
        let pass = match probe.step {
            0 => app.input == "/base64 ",
            1 => app.mode.as_deref() == Some("base64") && app.input.is_empty(),
            2 => {
                app.input.is_empty()
                    && app.mode.as_deref() == Some("base64")
                    && app.submode.as_deref() == Some("encode")
            }
            3 => app.submode.is_none() && app.mode.as_deref() == Some("base64"),
            4 => app.mode.is_none(),
            5 | 6 => app.visible_indices().len() > 1 && app.selected == probe.expected_selection,
            7 => app.tab == 1,
            8 => app.tab == 0,
            9 => app.confirm_discard && app.panel == Panel::Settings,
            10 => !app.confirm_discard && app.panel == Panel::Settings,
            11 => app.panel == Panel::None,
            12 => !app.date_picker,
            13 => !app.native_color.open && app.color_picker,
            14 => !app.color_picker,
            15 | 16 => app.date_picker && app.date_value == "2024-03-01 23:59",
            17 | 18 => app.date_picker && app.date_value == "2024-03-01 00:59",
            19 => app.date_picker && app.date_value == "2024-03-01 00:00",
            20 => !app.date_picker,
            21 => app.input == "/bas" && app.completion().is_none(),
            22 => {
                !app.main_visible && !app.date_picker && !app.native_color.open && app.color_picker
            }
            23 => {
                let range = TextEdit::load_state(ctx, Id::new("quicktool-search"))
                    .and_then(|s| s.cursor.char_range());
                app.panel == Panel::None
                    && app.main_visible
                    && range.is_some_and(|r| {
                        r.sorted()[0].index == 0 && r.sorted()[1].index == app.input.chars().count()
                    })
            }
            24 => {
                app.tab == 2
                    && app.selected == app.visible_indices().first().copied().unwrap_or(0)
                    && ctx.memory(|m| m.has_focus(Id::new("quicktool-search")))
            }
            25 => {
                app.panel == Panel::Settings
                    && app.confirm_discard
                    && app.draft["glowStrength"] == 0.123
            }
            26 => app.pinned && app.panel == Panel::Settings,
            27 => {
                app.panel == Panel::None && ctx.memory(|m| m.has_focus(Id::new("quicktool-search")))
            }
            28 => {
                app.pinned = false;
                app.panel = Panel::History;
                app.store.settings["hideOnBlur"] = true.into();
                let history = app.blur_hides(ctx);
                app.panel = Panel::Settings;
                let settings = !app.blur_hides(ctx);
                app.panel = Panel::None;
                app.recording_shortcut = true;
                let recording = !app.blur_hides(ctx);
                app.recording_shortcut = false;
                app.pinned = true;
                let pin = !app.blur_hides(ctx);
                app.pinned = false;
                app.date_picker = true;
                let popup = !app.blur_hides(ctx);
                app.date_picker = false;
                app.store.settings["hideOnBlur"] = false.into();
                history && settings && recording && pin && popup
            }
            29 => {
                app.rows.first().is_some_and(|r| {
                    r.country
                        .as_ref()
                        .is_some_and(|(c, primary)| *primary && c["alpha2"] == "CH")
                        && r.score == 1.15
                }) && app.visible_indices().iter().any(|&i| app.rows[i].command)
            }
            30 => {
                app.tab == 1
                    && app
                        .visible_indices()
                        .iter()
                        .all(|&i| app.rows[i].country.is_none() && app.row_is_launch(&app.rows[i]))
            }
            31 => !probe.expected_input.is_empty() && app.input == probe.expected_input,
            _ => false,
        };
        probe.checks.push(serde_json::json!({"check":LABELS[probe.step],"passed":pass,"frame":app.frames,"mode":app.mode,"submode":app.submode,"input":app.input,"selected":app.selected,"tab":app.tab,"settingsOpen":app.panel==Panel::Settings,"discardPrompt":app.confirm_discard,"datePicker":app.date_picker,"colorPanel":app.color_picker,"nativeColorPopup":app.native_color.open,"dateValue":app.date_value}));
        probe.step += 1;
        probe.armed = false;
    }
    if probe.step == LABELS.len() || probe.started.elapsed() > Duration::from_secs(30) {
        let passed = probe.step == LABELS.len() && probe.checks.iter().all(|c| c["passed"] == true);
        let report = serde_json::json!({"passed":passed,"checks":probe.checks,"elapsedMs":probe.started.elapsed().as_secs_f64()*1000.,"method":"Real eframe frames with RawInput key events injected only into this isolated app; no OS input, clipboard, or external launches. Does not replace manual mouse/keyboard acceptance."});
        match serde_json::to_vec_pretty(&report)
            .map_err(|e| e.to_string())
            .and_then(|bytes| {
                quicktool_rust::storage::atomic_write(&probe.path, &bytes)
                    .map_err(|e| e.to_string())
            }) {
            Ok(()) => eprintln!(
                "Whole-app UI event probe passed={passed}: {}",
                probe.path.display()
            ),
            Err(error) => eprintln!("Whole-app UI probe report failed: {error}"),
        }
        ctx.send_viewport_cmd(ViewportCommand::Close);
    } else {
        app.interaction_probe = Some(probe);
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}
