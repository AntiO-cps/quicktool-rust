//! A single-line text editor with bounded glyph layout for large encoded images.
use eframe::egui::{
    text::{CCursor, CCursorRange},
    *,
};
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
#[derive(Clone)]
struct Edit {
    start: usize,
    removed: String,
    inserted: String,
    before: (usize, usize),
    after: (usize, usize),
}
struct Drag {
    anchor: usize,
    start: usize,
    end: usize,
    shift: f32,
    last: Pos2,
    follow: bool,
}
#[derive(Default)]
pub struct LargeInput {
    active: bool,
    cursor: usize,
    anchor: usize,
    undo: VecDeque<Edit>,
    redo: Vec<Edit>,
    preedit: String,
    signature: Option<u64>,
    drag: Option<Drag>,
}
fn signature(text: &str) -> u64 {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hash);
    hash.finish()
}
thread_local! {
 static WORDS: icu_segmenter::WordSegmenter = icu_segmenter::WordSegmenter::new_auto();
 static GRAPHEMES: icu_segmenter::GraphemeClusterSegmenter = icu_segmenter::GraphemeClusterSegmenter::new();
}
fn white(ch: char) -> bool {
    ch.is_whitespace() && ch != '\u{85}'
}
fn punct(ch: char) -> bool {
    ch.len_utf16() == 2
        || quicktool_rust::unicode::character_metadata(ch as u32)
            .is_some_and(|(_, c)| c.starts_with(['P', 'S']))
}
fn word_break(ch: char) -> bool {
    ch.len_utf16() == 2
        || matches!(ch, '\r' | '\n' | '_')
        || (!white(ch)
            && quicktool_rust::unicode::character_metadata(ch as u32)
                .is_some_and(|(_, c)| c.starts_with(['L', 'M', 'N', 'P', 'S']) || c == "Zs"))
}
fn skip_white(text: &str, mut at: usize) -> usize {
    for ch in text[at..].chars() {
        if !white(ch) {
            break;
        }
        at += ch.len_utf8();
    }
    at
}
fn previous_word(text: &str, cursor: usize) -> usize {
    WORDS.with(|s| {
        let bounds: Vec<_> = s.segment_str(text).take_while(|&i| i < cursor).collect();
        let mut punctuation = None;
        for at in bounds.into_iter().rev() {
            let Some(ch) = text[at..].chars().next() else {
                continue;
            };
            if punct(ch) {
                if text[..at].chars().next_back().is_some_and(|c| {
                    c.len_utf16() == 1 && quicktool_rust::js_case::is_alphanumeric(c)
                }) {
                    return at;
                }
                punctuation = Some(at);
                continue;
            }
            if let Some(at) = punctuation {
                return at;
            }
            if word_break(ch) {
                return at;
            }
        }
        0
    })
}
fn next_word(text: &str, cursor: usize) -> usize {
    WORDS.with(|s| {
        for at in s.segment_str(text).filter(|&i| i > cursor) {
            let prev = text[..at].chars().next_back();
            let next = text[at..].chars().next();
            if next.is_some_and(punct) {
                if prev.is_some_and(|c| {
                    c.len_utf16() == 1 && quicktool_rust::js_case::is_alphanumeric(c)
                }) {
                    return skip_white(text, at);
                }
                continue;
            }
            if prev.is_some_and(word_break)
                || (prev.is_some_and(white) && next.is_some_and(word_break))
            {
                return skip_white(text, at);
            }
        }
        text.len()
    })
}
fn previous_grapheme(text: &str, cursor: usize) -> usize {
    GRAPHEMES.with(|s| {
        s.segment_str(text)
            .take_while(|&i| i < cursor)
            .last()
            .unwrap_or(0)
    })
}
fn previous_backspace(text: &str, cursor: usize) -> usize {
    let before = boundary(text, cursor.saturating_sub(1));
    if text[before..cursor]
        .chars()
        .next()
        .and_then(|c| quicktool_rust::unicode::character_metadata(c as u32))
        .is_some_and(|(_, c)| c.starts_with('M'))
    {
        before
    } else {
        previous_grapheme(text, cursor)
    }
}
fn next_grapheme(text: &str, cursor: usize) -> usize {
    GRAPHEMES.with(|s| {
        s.segment_str(text)
            .find(|&i| i > cursor)
            .unwrap_or(text.len())
    })
}
fn ceil_grapheme(text: &str, cursor: usize) -> usize {
    GRAPHEMES.with(|s| {
        s.segment_str(text)
            .find(|&i| i >= cursor)
            .unwrap_or(text.len())
    })
}
fn select_word(text: &str, at: usize) -> (usize, usize) {
    WORDS.with(|s| {
        let mut start = 0;
        for end in s.segment_str(text).skip(1) {
            if end > at {
                return (start, end);
            }
            start = end;
        }
        (start, text.len())
    })
}
fn boundary(text: &str, index: usize) -> usize {
    let mut i = index.min(text.len());
    while !text.is_char_boundary(i) {
        i -= 1;
    }
    i
}
pub fn byte_index(text: &str, character: usize) -> usize {
    if text.is_ascii() {
        character.min(text.len())
    } else {
        text.char_indices()
            .nth(character)
            .map(|(i, _)| i)
            .unwrap_or(text.len())
    }
}
fn char_index(text: &str, byte: usize) -> usize {
    if text.is_ascii() {
        byte.min(text.len())
    } else {
        text[..boundary(text, byte)].chars().count()
    }
}
fn window(text: &str, cursor: usize) -> (usize, usize) {
    (
        boundary(text, cursor.saturating_sub(2048)),
        boundary(text, cursor.saturating_add(2048).min(text.len())),
    )
}
impl LargeInput {
    fn sync_document(&mut self, text: &str) -> bool {
        let hash = signature(text);
        if self.signature != Some(hash) {
            self.active = true;
            self.cursor = text.len();
            self.anchor = self.cursor;
            self.undo.clear();
            self.redo.clear();
            self.preedit.clear();
            self.drag = None;
            self.signature = Some(hash);
            return true;
        }
        false
    }
    fn record(&mut self, text: &str) {
        self.signature = Some(signature(text));
    }
    pub fn active(&self) -> bool {
        self.active
    }
    pub fn intercept_paste(&mut self, text: &mut String, incoming: String, range: (usize, usize)) {
        let previously_active = self.active;
        self.sync_document(text);
        if !previously_active {
            self.cursor = boundary(text, range.1);
            self.anchor = boundary(text, range.0);
            self.undo.clear();
            self.redo.clear();
        }
        self.active = true;
        self.replace(text, incoming);
    }
    pub(super) fn selection(&self) -> std::ops::Range<usize> {
        self.cursor.min(self.anchor)..self.cursor.max(self.anchor)
    }
    fn replace(&mut self, text: &mut String, inserted: String) {
        let inserted = inserted.replace(['\r', '\n'], "");
        self.drag = None;
        let range = self.selection();
        let removed = text[range.clone()].to_string();
        let before = (self.cursor, self.anchor);
        if removed == inserted {
            self.cursor = range.start + inserted.len();
            self.anchor = self.cursor;
            return;
        }
        text.replace_range(range.clone(), &inserted);
        self.cursor = range.start + inserted.len();
        self.anchor = self.cursor;
        self.record(text);
        self.undo.push_back(Edit {
            start: range.start,
            removed,
            inserted,
            before,
            after: (self.cursor, self.anchor),
        });
        self.redo.clear();
        let mut bytes = self
            .undo
            .iter()
            .map(|e| e.removed.len() + e.inserted.len())
            .sum::<usize>();
        while (bytes > 64 * 1024 * 1024 || self.undo.len() > 1000) && self.undo.len() > 1 {
            let edit = self.undo.pop_front().unwrap();
            bytes -= edit.removed.len() + edit.inserted.len();
        }
    }
    pub fn action(&mut self, text: &mut String, ctx: &Context, action: &str) -> bool {
        self.sync_document(text);
        self.drag = None;
        let mut changed = false;
        match action {
            "all" => {
                self.anchor = 0;
                self.cursor = text.len();
            }
            "copy" if !self.selection().is_empty() => {
                ctx.copy_text(text[self.selection()].to_string())
            }
            "cut" if !self.selection().is_empty() => {
                ctx.copy_text(text[self.selection()].to_string());
                self.replace(text, String::new());
                changed = true;
            }
            "paste" => match quicktool_rust::platform::clipboard_text() {
                Ok(incoming) => {
                    self.replace(text, incoming);
                    changed = true;
                }
                Err(error) => eprintln!("Clipboard paste failed: {error}"),
            },
            "undo" => {
                if let Some(edit) = self.undo.pop_back() {
                    text.replace_range(edit.start..edit.start + edit.inserted.len(), &edit.removed);
                    (self.cursor, self.anchor) = edit.before;
                    self.redo.push(edit);
                    changed = true;
                }
            }
            "redo" => {
                if let Some(edit) = self.redo.pop() {
                    text.replace_range(edit.start..edit.start + edit.removed.len(), &edit.inserted);
                    (self.cursor, self.anchor) = edit.after;
                    self.undo.push_back(edit);
                    changed = true;
                }
            }
            _ => {}
        }
        self.record(text);
        self.store(ctx, text);
        changed
    }
    fn store(&self, ctx: &Context, text: &str) {
        let id = Id::new("quicktool-search");
        let mut state = TextEdit::load_state(ctx, id).unwrap_or_default();
        state.cursor.set_char_range(Some(CCursorRange::two(
            CCursor::new(char_index(text, self.cursor)),
            CCursor::new(char_index(text, self.anchor)),
        )));
        state.store(ctx, id);
    }
    pub fn show(&mut self, ui: &mut Ui, text: &mut String, size: Vec2) -> Response {
        let id = Id::new("quicktool-search");
        if self.sync_document(text) {
            eframe::egui::text_edit::TextEditState::default().store(ui.ctx(), id);
        }
        self.cursor = boundary(text, self.cursor);
        self.anchor = boundary(text, self.anchor);
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let mut response = ui.interact(rect, id, Sense::click_and_drag());
        let pointer = response.clone();
        if pointer.clicked() || pointer.drag_started() {
            pointer.request_focus();
        }
        let mut changed = false;
        if ui.memory(|m| m.has_focus(id)) {
            let events = ui.input_mut(|i| std::mem::take(&mut i.events));
            let mut keep = vec![];
            for event in events {
                match event {
                    Event::Ime(ImeEvent::Preedit(preedit)) => self.preedit = preedit,
                    Event::Ime(ImeEvent::Commit(incoming)) => {
                        self.preedit.clear();
                        self.replace(text, incoming);
                        changed = true;
                    }
                    Event::Ime(ImeEvent::Disabled) => self.preedit.clear(),
                    Event::Paste(incoming) | Event::Text(incoming) => {
                        self.intercept_paste(text, incoming, (self.anchor, self.cursor));
                        changed = true;
                    }
                    Event::Copy if !self.selection().is_empty() => {
                        ui.ctx().copy_text(text[self.selection()].to_string());
                    }
                    Event::Cut => {
                        self.action(text, ui.ctx(), "cut");
                        changed = true;
                    }
                    Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } => {
                        if !self.preedit.is_empty()
                            && matches!(
                                key,
                                Key::Backspace
                                    | Key::Delete
                                    | Key::ArrowLeft
                                    | Key::ArrowRight
                                    | Key::Home
                                    | Key::End
                            )
                        {
                            continue;
                        }
                        let command = modifiers.command || modifiers.ctrl;
                        let action = match (key, command, modifiers.shift) {
                            (Key::A, true, _) => Some("all"),
                            (Key::Z, true, true) | (Key::Y, true, _) => Some("redo"),
                            (Key::Z, true, false) => Some("undo"),
                            _ => None,
                        };
                        if let Some(action) = action {
                            changed |= self.action(text, ui.ctx(), action);
                            continue;
                        }
                        match key {
                            Key::Backspace | Key::Delete => {
                                if command && modifiers.shift {
                                    continue;
                                }
                                if self.cursor == self.anchor {
                                    self.cursor = ceil_grapheme(text, self.cursor);
                                    self.anchor = self.cursor;
                                    if key == Key::Backspace {
                                        if self.cursor > 0 {
                                            self.anchor = if command {
                                                previous_word(text, self.cursor)
                                            } else {
                                                previous_backspace(text, self.cursor)
                                            };
                                        }
                                    } else if self.cursor < text.len() {
                                        let end = next_grapheme(text, self.cursor);
                                        self.anchor = if command {
                                            next_word(text, self.cursor)
                                        } else {
                                            end
                                        };
                                    }
                                }
                                if self.cursor != self.anchor {
                                    self.replace(text, String::new());
                                    changed = true;
                                }
                            }
                            Key::Home | Key::End | Key::ArrowLeft | Key::ArrowRight => {
                                self.drag = None;
                                self.cursor = ceil_grapheme(text, self.cursor);
                                self.anchor = ceil_grapheme(text, self.anchor);
                                let next = match key {
                                    Key::Home => 0,
                                    Key::End => text.len(),
                                    Key::ArrowLeft
                                        if !modifiers.shift && self.cursor != self.anchor =>
                                    {
                                        self.cursor.min(self.anchor)
                                    }
                                    Key::ArrowRight
                                        if !modifiers.shift && self.cursor != self.anchor =>
                                    {
                                        self.cursor.max(self.anchor)
                                    }
                                    Key::ArrowLeft if command => previous_word(text, self.cursor),
                                    Key::ArrowRight if command => next_word(text, self.cursor),
                                    Key::ArrowLeft => previous_grapheme(text, self.cursor),
                                    _ => next_grapheme(text, self.cursor),
                                };
                                self.cursor = next;
                                if !modifiers.shift {
                                    self.anchor = next;
                                }
                            }
                            _ => keep.push(event),
                        }
                    }
                    other => keep.push(other),
                }
            }
            ui.input_mut(|i| i.events = keep);
        }
        let (start, end) = self
            .drag
            .as_ref()
            .filter(|drag| !drag.follow)
            .map(|drag| (drag.start, drag.end))
            .unwrap_or_else(|| window(text, self.cursor));
        let visible = &text[start..end];
        let galley = ui.painter().layout_no_wrap(
            visible.to_string(),
            FontId::proportional(16.),
            ui.visuals().text_color(),
        );
        let local_cursor = char_index(visible, self.cursor - start);
        let cursor_rect = galley.pos_from_ccursor(CCursor::new(local_cursor));
        let shift = self
            .drag
            .as_ref()
            .filter(|drag| !drag.follow)
            .map(|drag| drag.shift)
            .unwrap_or_else(|| (cursor_rect.min.x - rect.width() + 8.).max(0.));
        let origin = rect.left_center() - vec2(shift, galley.size().y / 2.);
        if let Some(at) = pointer.interact_pointer_pos() {
            let local = galley.cursor_from_pos(at - origin).ccursor.index;
            let byte = start + byte_index(visible, local);
            if ui.input(|i| i.pointer.primary_pressed()) && rect.contains(at) {
                self.cursor = byte;
                if !ui.input(|i| i.modifiers.shift) {
                    self.anchor = byte;
                }
                self.drag = Some(Drag {
                    anchor: self.anchor,
                    start,
                    end,
                    shift,
                    last: at,
                    follow: false,
                });
            }
            if pointer.dragged() {
                if let Some(drag) = &mut self.drag {
                    self.anchor = drag.anchor;
                    let outside = if at.x < rect.left() {
                        at.x - rect.left()
                    } else if at.x > rect.right() {
                        at.x - rect.right()
                    } else {
                        0.
                    };
                    if outside != 0. {
                        let step = (outside.abs() / 8.).ceil().clamp(1., 64.) as usize;
                        self.cursor = if outside < 0. {
                            boundary(text, self.cursor.saturating_sub(step))
                        } else {
                            boundary(text, self.cursor.saturating_add(step).min(text.len()))
                        };
                        drag.follow = true;
                        ui.ctx()
                            .request_repaint_after(std::time::Duration::from_millis(16));
                    } else if at != drag.last {
                        self.cursor = byte;
                        drag.start = start;
                        drag.end = end;
                        drag.shift = shift;
                        drag.follow = false;
                    }
                    drag.last = at;
                }
            }
            if pointer.triple_clicked() {
                self.anchor = 0;
                self.cursor = text.len();
            } else if pointer.double_clicked() {
                let (a, b) = select_word(text, byte);
                self.anchor = a;
                self.cursor = b;
            } else if pointer.clicked() {
                self.cursor = byte;
                if !ui.input(|i| i.modifiers.shift) {
                    self.anchor = byte;
                }
            }
        }
        if ui.input(|i| i.pointer.primary_released() || i.viewport().focused == Some(false)) {
            self.drag = None;
        }
        let painter = ui.painter().with_clip_rect(rect);
        if self.cursor != self.anchor {
            let selected = self.selection();
            let a = char_index(
                visible,
                selected.start.saturating_sub(start).min(visible.len()),
            );
            let b = char_index(
                visible,
                selected.end.saturating_sub(start).min(visible.len()),
            );
            let left = galley.pos_from_ccursor(CCursor::new(a)).min.x;
            let right = galley.pos_from_ccursor(CCursor::new(b)).min.x;
            painter.rect_filled(
                Rect::from_min_max(
                    origin + vec2(left, 0.),
                    origin + vec2(right, galley.size().y),
                ),
                0,
                if pointer.has_focus() {
                    ui.visuals().selection.bg_fill
                } else {
                    Color32::from_gray(70)
                },
            );
        }
        if text.is_empty() && self.preedit.is_empty() {
            painter.text(
                rect.left_center(),
                Align2::LEFT_CENTER,
                "输入命令、文本或表达式…",
                FontId::proportional(16.),
                super::weak_color(ui),
            );
        }
        painter.galley(origin, galley, ui.visuals().text_color());
        if pointer.has_focus() {
            let x = origin.x + cursor_rect.min.x;
            painter.line_segment(
                [
                    pos2(x, rect.center().y - 10.),
                    pos2(x, rect.center().y + 10.),
                ],
                Stroke::new(1., ui.visuals().text_color()),
            );
            if !self.preedit.is_empty() {
                painter.text(
                    pos2(x, rect.center().y),
                    Align2::LEFT_CENTER,
                    &self.preedit,
                    FontId::proportional(16.),
                    ui.visuals().text_color(),
                );
            }
            ui.ctx().output_mut(|output| {
                output.ime = Some(eframe::egui::output::IMEOutput {
                    rect,
                    cursor_rect: Rect::from_min_size(pos2(x, rect.top()), vec2(1., rect.height())),
                })
            });
        }
        self.store(ui.ctx(), text);
        if changed {
            response.mark_changed();
        }
        response.on_hover_cursor(CursorIcon::Text)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_payload_layout_window_is_bounded() {
        let text = "x".repeat(16 * 1024 * 1024);
        for cursor in [0, 1000, text.len() / 2, text.len()] {
            let (a, b) = window(&text, cursor);
            assert!(b - a <= 4096);
            assert!(a <= cursor && cursor <= b);
        }
    }
    #[test]
    fn unicode_window_preserves_boundaries() {
        let text = "图片🙂".repeat(3000);
        let (a, b) = window(&text, 7000);
        assert!(text.is_char_boundary(a) && text.is_char_boundary(b));
    }
    #[test]
    fn full_selection_edit_and_undo_retain_large_payload() {
        let ctx = Context::default();
        let mut input = "x".repeat(16 * 1024 * 1024);
        let original = input.clone();
        let mut editor = LargeInput::default();
        editor.intercept_paste(&mut input, "png".into(), (0, original.len()));
        assert_eq!(input, "png");
        assert!(editor.action(&mut input, &ctx, "undo"));
        assert_eq!(input, original);
        assert!(editor.action(&mut input, &ctx, "redo"));
        assert_eq!(input, "png");
    }
    #[test]
    fn same_length_external_unicode_document_invalidates_undo() {
        let ctx = Context::default();
        let mut input = "x".repeat(65537);
        let mut editor = LargeInput::default();
        let end = input.len();
        editor.intercept_paste(&mut input, "🙂".into(), (end, end));
        input = "🙂".repeat(16385) + "x";
        let expected = input.clone();
        assert!(!editor.action(&mut input, &ctx, "undo"));
        assert_eq!(input, expected);
    }
    #[test]
    fn paste_removes_line_breaks_and_empty_cut_does_not_add_history() {
        let ctx = Context::default();
        let mut input = String::new();
        let mut editor = LargeInput::default();
        editor.intercept_paste(&mut input, "a\r\nb\nc".into(), (0, 0));
        assert_eq!(input, "abc");
        let count = editor.undo.len();
        for _ in 0..1500 {
            assert!(!editor.action(&mut input, &ctx, "cut"));
        }
        assert_eq!(editor.undo.len(), count);
        editor.action(&mut input, &ctx, "copy");
        assert!(ctx.output(|output| output.commands.is_empty()));
    }
    #[test]
    fn word_navigation_and_double_click_select_words() {
        let text = "alpha beta 世界";
        assert_eq!(previous_word(text, 10), 6);
        assert_eq!(next_word(text, 0), 6);
        assert_eq!(select_word(text, 8), (6, 10));
        assert_eq!(select_word(text, 12), (11, 17));
    }
    fn frame(ctx: &Context, editor: &mut LargeInput, input: &mut String, events: Vec<Event>) {
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400., 80.))),
                events,
                ..Default::default()
            },
            |ctx| {
                CentralPanel::default().show(ctx, |ui| {
                    ui.memory_mut(|memory| memory.request_focus(Id::new("quicktool-search")));
                    editor.show(ui, input, vec2(300., 32.));
                });
            },
        );
    }
    #[test]
    fn ime_preedit_backspace_does_not_delete_committed_text() {
        let ctx = Context::default();
        let mut editor = LargeInput::default();
        let mut input = "x".repeat(70000);
        let original = input.clone();
        frame(
            &ctx,
            &mut editor,
            &mut input,
            vec![
                Event::Ime(ImeEvent::Enabled),
                Event::Ime(ImeEvent::Preedit("你".into())),
                Event::Key {
                    key: Key::Backspace,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        assert_eq!(input, original);
        assert_eq!(editor.preedit, "你");
    }
    #[test]
    fn pointer_drag_anchors_on_press_and_does_not_drift_when_stationary() {
        let ctx = Context::default();
        let mut editor = LargeInput::default();
        let mut input = "abcdefghijklmnop ".repeat(5000);
        frame(&ctx, &mut editor, &mut input, vec![]);
        frame(
            &ctx,
            &mut editor,
            &mut input,
            vec![
                Event::PointerMoved(pos2(200., 20.)),
                Event::PointerButton {
                    pos: pos2(200., 20.),
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        let anchor = editor.anchor;
        frame(
            &ctx,
            &mut editor,
            &mut input,
            vec![Event::PointerMoved(pos2(250., 20.))],
        );
        assert_eq!(editor.anchor, anchor);
        assert!(editor.cursor > anchor);
        let selection = (editor.anchor, editor.cursor);
        frame(&ctx, &mut editor, &mut input, vec![]);
        assert_eq!((editor.anchor, editor.cursor), selection);
    }
    #[test]
    fn chromium_editing_oracle() {
        run_oracle(include_str!("../tests/editing-oracle.json"), true);
        run_oracle(include_str!("../tests/editing-plain-oracle.json"), false);
    }
    fn run_oracle(source: &str, ctrl: bool) {
        let oracle: serde_json::Value = serde_json::from_str(source).unwrap();
        let mut errors = vec![];
        for (index, case) in oracle["cases"].as_array().unwrap().iter().enumerate() {
            let ctx = Context::default();
            let mut text = case["value"].as_str().unwrap().to_string();
            let mut editor = LargeInput::default();
            editor.sync_document(&text);
            let utf16 = case["caret"].as_u64().unwrap() as usize;
            let mut used = 0;
            let cursor = text
                .char_indices()
                .find_map(|(byte, ch)| {
                    if used == utf16 {
                        Some(byte)
                    } else {
                        used += ch.len_utf16();
                        None
                    }
                })
                .unwrap_or(text.len());
            editor.cursor = cursor;
            editor.anchor = cursor;
            let key = match case["key"].as_str().unwrap() {
                "Left" => Key::ArrowLeft,
                "Right" => Key::ArrowRight,
                "Backspace" => Key::Backspace,
                _ => Key::Delete,
            };
            let modifiers = Modifiers {
                ctrl,
                shift: case["shift"] == true,
                ..Modifiers::NONE
            };
            frame(
                &ctx,
                &mut editor,
                &mut text,
                vec![Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers,
                }],
            );
            let selected = editor.selection();
            let actual = serde_json::json!({"value":text,"start":text[..selected.start].encode_utf16().count(),"end":text[..selected.end].encode_utf16().count(),"direction":if editor.cursor<editor.anchor{"backward"}else{"forward"}});
            if actual != case["actual"] {
                errors.push(serde_json::json!({"case":index,"input":case,"actual":actual}));
            }
        }
        assert!(
            errors.is_empty(),
            "{} Chromium editing differences: {}",
            errors.len(),
            serde_json::to_string_pretty(&errors).unwrap()
        );
    }
}
