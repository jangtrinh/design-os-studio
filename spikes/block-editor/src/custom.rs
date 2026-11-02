//! THROWAWAY backend B: own input handling over egui galleys. One editor surface (one egui `Id`)
//! owns keyboard focus for every block, so the IME session and focus never change when the caret
//! crosses a block edge. The IME composition lives in `App::preedit`, outside the document.

use crate::app::App;
use crate::model::char_len;
use crate::render;
use crate::theme;
use egui::text::CCursor;
use egui::text_selection::CCursorRange;
use egui::{output::IMEOutput, vec2, Context, Event, EventFilter, Id, ImeEvent, Key, Modifiers, Sense, Stroke, Ui};

pub fn editor_id() -> Id {
    Id::new("block-editor-surface")
}

/// Keyboard, clipboard and IME events for the focused block.
pub fn input(app: &mut App, ctx: &Context) {
    // egui drops focus while the window is inactive (a launch in the background, Cmd+Tab); take it
    // back whenever nothing else holds it so typing and the IME work as soon as the window is active.
    if ctx.memory(|m| m.focused().is_none()) {
        ctx.memory_mut(|m| m.request_focus(editor_id()));
    }
    if !ctx.memory(|m| m.has_focus(editor_id())) {
        return;
    }
    let all = EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true };
    ctx.memory_mut(|m| m.set_focus_lock_filter(editor_id(), all));
    for ev in ctx.input(|i| i.events.clone()) {
        match ev {
            Event::Text(s) if app.preedit.is_none() => app.insert_text(&s),
            Event::Paste(s) => app.insert_text(&s.replace("\r\n", "\n")),
            Event::Copy => copy(app, ctx, false),
            Event::Cut => copy(app, ctx, true),
            Event::Ime(ImeEvent::Preedit { text, .. }) => preedit(app, text),
            Event::Ime(ImeEvent::Commit(t)) => {
                app.preedit = None;
                app.insert_text(&t);
            }
            Event::Key { key, pressed: true, modifiers, .. } if app.preedit.is_none() => key_press(app, key, modifiers),
            _ => {}
        }
    }
}

fn preedit(app: &mut App, text: String) {
    if app.preedit.is_none() && !text.is_empty() && !app.sel().is_empty() {
        let r = app.sel();
        app.delete_range(r);
    }
    app.preedit = (!text.is_empty()).then_some(text);
}

fn copy(app: &mut App, ctx: &Context, cut: bool) {
    let r = app.sel();
    if r.is_empty() {
        return;
    }
    let t: String = app.doc.blocks[app.focus].text.chars().skip(r.start).take(r.len()).collect();
    ctx.copy_text(t);
    if cut {
        app.delete_range(r);
    }
}

fn key_press(app: &mut App, key: Key, m: Modifiers) {
    let len = char_len(&app.doc.blocks[app.focus].text);
    match key {
        Key::Enter if m.shift => app.insert_text("\n"),
        Key::Enter => app.op_split(),
        Key::Backspace if !app.sel().is_empty() => app.delete_range(app.sel()),
        Key::Backspace if app.head > 0 => app.delete_range(app.head - 1..app.head),
        Key::Backspace => app.op_backspace_at_start(),
        Key::Delete if !app.sel().is_empty() => app.delete_range(app.sel()),
        Key::Delete if app.head < len => app.delete_range(app.head..app.head + 1),
        Key::ArrowLeft => horizontal(app, -1, m.shift),
        Key::ArrowRight => horizontal(app, 1, m.shift),
        Key::ArrowUp => vertical(app, -1, m.shift),
        Key::ArrowDown => vertical(app, 1, m.shift),
        Key::Home | Key::End => row_edge(app, key == Key::End, m.shift),
        Key::A if m.command => {
            app.anchor = 0;
            app.head = len;
        }
        Key::B if m.command => app.op_bold(),
        _ => {}
    }
}

fn set_head(app: &mut App, head: usize, extend: bool) {
    app.prefer_next = false;
    app.head = head;
    if !extend {
        app.anchor = head;
    }
    app.hist.break_burst();
}

fn horizontal(app: &mut App, dir: i32, extend: bool) {
    app.goal_x = None;
    let len = char_len(&app.doc.blocks[app.focus].text);
    if !extend && !app.sel().is_empty() {
        let r = app.sel();
        return set_head(app, if dir < 0 { r.start } else { r.end }, false);
    }
    if dir < 0 && app.head == 0 && !extend && app.focus > 0 {
        let prev = app.focus - 1;
        return app.go(prev, char_len(&app.doc.blocks[prev].text));
    }
    if dir > 0 && app.head == len && !extend && app.focus + 1 < app.doc.blocks.len() {
        return app.go(app.focus + 1, 0);
    }
    let h = (app.head as i64 + dir as i64).clamp(0, len as i64) as usize;
    set_head(app, h, extend);
}

fn vertical(app: &mut App, dir: i32, extend: bool) {
    let id = app.doc.blocks[app.focus].id;
    let Some(g) = app.galleys.get(&id).cloned() else { return };
    let cur = app.cursor();
    let row = g.layout_from_cursor(cur).row;
    let x = app.goal_x.unwrap_or_else(|| g.pos_from_cursor(cur).center().x);
    let edge = if dir < 0 { row == 0 } else { row + 1 >= g.rows.len() };
    if edge {
        let target = app.focus as i64 + dir as i64;
        if extend || target < 0 || target as usize >= app.doc.blocks.len() {
            let len = char_len(&app.doc.blocks[app.focus].text);
            return set_head(app, if dir < 0 { 0 } else { len }, extend);
        }
        cross_block(app, dir, x);
        return;
    }
    let (c, _) = if dir < 0 { g.cursor_up_one_row(&cur, Some(x)) } else { g.cursor_down_one_row(&cur, Some(x)) };
    set_head(app, c.index.0, extend);
    app.prefer_next = c.prefer_next_row;
    app.goal_x = Some(x);
}

/// Moves the caret into the neighbouring block at horizontal position `x`: onto its last row when
/// going up, its first row when going down. Shared by both backends.
pub fn cross_block(app: &mut App, dir: i32, x: f32) {
    let t = (app.focus as i64 + dir as i64).clamp(0, app.doc.blocks.len() as i64 - 1) as usize;
    let tid = app.doc.blocks[t].id;
    let c = match app.galleys.get(&tid) {
        Some(tg) => tg.cursor_from_pos(vec2(x, if dir < 0 { tg.size().y - 2.0 } else { 2.0 })),
        None => CCursor::new(0usize),
    };
    app.go(t, c.index.0);
    app.prefer_next = c.prefer_next_row;
    app.goal_x = Some(x);
}

fn row_edge(app: &mut App, end: bool, extend: bool) {
    let id = app.doc.blocks[app.focus].id;
    let Some(g) = app.galleys.get(&id).cloned() else { return };
    let cur = app.cursor();
    let c = if end { g.cursor_end_of_row(&cur) } else { g.cursor_begin_of_row(&cur) };
    set_head(app, c.index.0, extend);
    app.prefer_next = c.prefer_next_row;
}

/// Paints and senses one block. Called from the row layout with the column width.
pub fn show(ui: &mut Ui, app: &mut App, idx: usize, width: f32) {
    let ctx = ui.ctx().clone();
    let (kind, id) = (app.doc.blocks[idx].kind, app.doc.blocks[idx].id);
    let focused = idx == app.focus && ctx.memory(|m| m.has_focus(editor_id()));
    let mut shown = app.doc.blocks[idx].clone();
    let mut underline = None;
    let mut pre_len = 0;
    if let (true, Some(pre)) = (idx == app.focus, app.preedit.clone()) {
        let at = app.sel().start;
        pre_len = char_len(&pre);
        shown.replace(at..at, &pre);
        underline = Some(at..at + pre_len);
    }
    let galley = ctx.fonts_mut(|f| f.layout_job(render::job(kind, &shown.text, &shown.bold, underline, width)));
    let slot = render::reserve_surface(ui);
    let (rect, resp) = ui.allocate_exact_size(vec2(width, galley.size().y.max(render::line_height(kind))), Sense::click_and_drag());
    render::fill_surface(ui, slot, kind, rect);

    if let Some(pos) = resp.interact_pointer_pos() {
        let hit = galley.cursor_from_pos(pos - rect.min);
        let at = hit.index.0;
        if resp.contains_pointer() && ctx.input(|i| i.pointer.primary_pressed()) {
            ctx.memory_mut(|m| m.request_focus(editor_id()));
            let shift = ctx.input(|i| i.modifiers.shift);
            let anchor = if shift && app.focus == idx { app.anchor } else { at };
            app.go(idx, at);
            app.anchor = anchor;
            app.prefer_next = hit.prefer_next_row;
        } else if resp.dragged() && app.focus == idx {
            app.head = at;
            app.prefer_next = hit.prefer_next_row;
        }
    }
    let color = theme::TEXT_SECONDARY;
    let mut painted = galley.clone();
    if focused && app.anchor != app.head && pre_len == 0 {
        let range = CCursorRange::two(CCursor::new(app.anchor), CCursor::new(app.head));
        egui::text_selection::visuals::paint_text_selection(&mut painted, ui.visuals(), &range, None);
    }
    ui.painter().galley(rect.min, painted, color);
    if kind == crate::model::Kind::Text && shown.text.is_empty() && idx == app.focus {
        ui.painter().text(rect.min, egui::Align2::LEFT_TOP, "Type / for blocks", theme::regular(14.0), theme::TEXT_PLACEHOLDER);
    }
    if focused {
        let caret_at = if pre_len > 0 { app.sel().start + pre_len } else { app.head };
        let cur = galley.pos_from_cursor(CCursor { index: caret_at.into(), prefer_next_row: app.prefer_next && pre_len == 0 }).translate(rect.min.to_vec2());
        ui.painter().line_segment([cur.center_top(), cur.center_bottom()], Stroke::new(1.5, theme::TEXT));
        app.caret_screen = Some(cur.left_bottom());
        ctx.output_mut(|o| o.ime = Some(IMEOutput { rect, cursor_rect: cur, should_interrupt_composition: std::mem::take(&mut app.interrupt_ime) }));
    }
    app.galleys.insert(id, galley);
}
