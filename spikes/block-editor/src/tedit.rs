//! THROWAWAY backend A: one `egui::TextEdit` per block. egui owns caret, selection, IME and text
//! editing; the app has to intercept keys BEFORE the widget runs (Enter, Backspace at start, arrows
//! at a row edge, Cmd+B) and push cursor changes back through `TextEditState`.

use crate::app::App;
use crate::custom::cross_block;
use crate::model::char_len;
use crate::render;
use crate::theme;
use egui::text::CCursor;
use egui::text_selection::CCursorRange;
use egui::text_edit::TextEditState;
use egui::{Context, Event, Id, Key, Margin, TextEdit, Ui};

pub fn block_id(app: &App, idx: usize) -> Id {
    Id::new(("tb", app.doc.blocks[idx].id))
}

/// Removes and reports pressed `key` events whose Shift state equals `shift` and that carry no
/// Cmd/Alt. `Modifiers::matches_logically` ignores an extra Shift, so `consume_key(NONE, ArrowUp)`
/// would also eat Shift+ArrowUp; this does not.
pub fn take_key(ctx: &Context, key: Key, shift: bool) -> bool {
    ctx.input_mut(|i| {
        let before = i.events.len();
        i.events.retain(|e| !matches!(e, Event::Key { key: k, pressed: true, modifiers, .. } if *k == key && modifiers.shift == shift && !modifiers.command && !modifiers.alt));
        i.events.len() != before
    })
}

/// Reads the widget's cursor into `app.anchor/head`, then lets the app take structural keys first.
pub fn pre_input(app: &mut App, ctx: &Context) {
    let idx = app.focus;
    let id = block_id(app, idx);
    if !ctx.memory(|m| m.has_focus(id)) {
        return;
    }
    let Some(r) = TextEditState::load(ctx, id).and_then(|s| s.cursor.char_range()) else { return };
    app.head = r.primary.index.0;
    app.anchor = r.secondary.index.0;
    app.prefer_next = r.primary.prefer_next_row;
    if app.preedit_active {
        return; // the IME owns Enter / Backspace / arrows while composing
    }
    let (head, collapsed) = (app.head, app.head == app.anchor);
    let len = char_len(&app.doc.blocks[idx].text);
    let bid = app.doc.blocks[idx].id;
    if take_key(ctx, Key::Enter, false) {
        return app.op_split();
    }
    if collapsed && head == 0 && take_key(ctx, Key::Backspace, false) {
        return app.op_backspace_at_start();
    }
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, Key::B)) {
        return app.op_bold();
    }
    if collapsed && head == 0 && idx > 0 && take_key(ctx, Key::ArrowLeft, false) {
        let prev = idx - 1;
        return app.go(prev, char_len(&app.doc.blocks[prev].text));
    }
    if collapsed && head == len && idx + 1 < app.doc.blocks.len() && take_key(ctx, Key::ArrowRight, false) {
        return app.go(idx + 1, 0);
    }
    let Some(g) = app.galleys.get(&bid).cloned() else { return };
    let cur = app.cursor();
    let row = g.layout_from_cursor(cur).row;
    let x = g.pos_from_cursor(cur).center().x;
    if row == 0 && idx > 0 && take_key(ctx, Key::ArrowUp, false) {
        cross_block(app, -1, x);
    } else if row + 1 >= g.rows.len() && idx + 1 < app.doc.blocks.len() && take_key(ctx, Key::ArrowDown, false) {
        cross_block(app, 1, x);
    }
}

pub fn show(ui: &mut Ui, app: &mut App, idx: usize, width: f32) {
    let ctx = ui.ctx().clone();
    let id = block_id(app, idx);
    let (kind, bid) = (app.doc.blocks[idx].kind, app.doc.blocks[idx].id);
    if let Some((b, anchor, head)) = app.pending {
        if b == idx {
            let mut st = TextEditState::load(&ctx, id).unwrap_or_default();
            st.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(anchor), CCursor { index: head.into(), prefer_next_row: app.prefer_next })));
            st.store(&ctx, id);
            ctx.memory_mut(|m| m.request_focus(id));
            app.pending = None;
        }
    }
    let bold = app.doc.blocks[idx].bold.clone();
    let before = app.doc.blocks[idx].text.clone();
    let mut text = before.clone();
    let mut layouter = |ui: &Ui, buf: &dyn egui::TextBuffer, wrap: f32| ui.ctx().fonts_mut(|f| f.layout_job(render::job(kind, buf.as_str(), &bold, None, wrap)));
    let slot = render::reserve_surface(ui);
    let mut edit = TextEdit::multiline(&mut text)
        .id(id)
        .frame(egui::Frame::NONE)
        .margin(Margin::ZERO)
        .desired_width(width)
        .desired_rows(1)
        .layouter(&mut layouter);
    if kind == crate::model::Kind::Text {
        edit = edit.hint_text(egui::RichText::new("Type / for blocks").color(theme::TEXT_PLACEHOLDER));
    }
    let out = edit.show(ui);
    render::fill_surface(ui, slot, kind, out.response.rect);
    if text != before {
        apply_diff(app, idx, &before, &text);
    }
    if out.response.has_focus() {
        app.focus = idx;
        if let Some(r) = out.cursor_range {
            app.head = r.primary.index.0;
            app.anchor = r.secondary.index.0;
            app.prefer_next = r.primary.prefer_next_row;
            let c = out.galley.pos_from_cursor(r.primary).translate(out.galley_pos.to_vec2());
            app.caret_screen = Some(c.left_bottom());
        }
    }
    app.galleys.insert(bid, out.galley);
}

/// The widget edits a plain `String`; recover the replaced range (common prefix / suffix) so bold
/// runs can follow the edit. This is the reconciliation cost of backend A.
fn apply_diff(app: &mut App, idx: usize, old: &str, new: &str) {
    let (o, n): (Vec<char>, Vec<char>) = (old.chars().collect(), new.chars().collect());
    let pre = o.iter().zip(&n).take_while(|(a, b)| a == b).count();
    let suf = o[pre..].iter().rev().zip(n[pre..].iter().rev()).take_while(|(a, b)| a == b).count();
    let inserted: String = n[pre..n.len() - suf].iter().collect();
    app.doc.blocks[idx].replace(pre..o.len() - suf, &inserted);
}
