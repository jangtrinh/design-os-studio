//! Selecting cells and blocks, typing into a cell, Tab, and the keys that act on selected cells.

use super::grips::{self, TableDrag};
use super::{selected_block, table_of, TableAction, TableGeo};
use crate::app::App;
use crate::canvas::interact::begin_text_edit;
use crate::canvas::{Interaction, TextSession};
use crate::edit::{Edit, Tx};
use designos_doc::table_ops;
use designos_doc::{Command, Document, NodeId, NodeKind, Vec2};
use egui::Pos2;

/// Whether `id` is a cell of a table.
pub fn is_cell(doc: &Document, id: NodeId) -> bool {
    doc.get(id).is_some_and(|n| matches!(n.kind, NodeKind::TableCell(_))) && table_of(doc, id).is_some()
}

/// The single selected cell, if the selection is exactly one.
pub fn single_cell(app: &App) -> Option<NodeId> {
    match app.selection.as_slice() {
        [one] if is_cell(&app.doc, *one) => Some(*one),
        _ => None,
    }
}

/// The cell of `table` under a page-space point.
fn cell_under(app: &App, table: NodeId, page: Vec2) -> Option<NodeId> {
    let geo = TableGeo::of(&app.doc, table)?;
    let local = geo.world.invert()?.apply(page);
    let (r, c) = geo.track_at(local)?;
    geo.cell_covering(r, c).map(|c| c.id)
}

/// Selects the block from `anchor` to `to`, whole merged blocks included.
fn select_block(app: &mut App, table: NodeId, anchor: NodeId, to: NodeId) {
    let Some(geo) = TableGeo::of(&app.doc, table) else { return };
    let (Some(a), Some(b)) = (geo.span_of(anchor), geo.span_of(to)) else { return };
    app.selection = geo.cells_in(geo.block(a, b));
}

/// Press on the canvas while a table or its cells are selected. Returns whether the press was taken.
/// `chrome` is false for the press that ended a text edit: it only ends the edit, it never presses a grip.
pub fn on_press(app: &mut App, pos: Pos2, page: Vec2, shift: bool, chrome: bool) -> bool {
    if !matches!(app.tool, crate::canvas::tools::Tool::Move) {
        return false;
    }
    if let Some((table, hover)) = grips::hover_at(app, pos).filter(|_| chrome) {
        if let Some(d) = grips::press(app, table, hover, page) {
            app.canvas.inter = Interaction::Table(d);
        }
        return true;
    }
    let Some((table, _)) = selected_block(app) else { return false };
    let Some(hit) = cell_under(app, table, page) else { return false };
    let anchor = app.selection.first().copied().unwrap_or(hit);
    if shift {
        select_block(app, table, anchor, hit);
    } else {
        app.select_only(hit);
    }
    app.canvas.inter = Interaction::Table(TableDrag::Block { table, anchor: if shift { anchor } else { hit } });
    true
}

/// A click (no drag) on a table that was already selected goes one level in: to the cell under it.
pub fn click_selected_table(app: &mut App, node: NodeId, page: Vec2) -> bool {
    if !matches!(app.doc.get(node).map(|n| &n.kind), Some(NodeKind::Table(_))) {
        return false;
    }
    match cell_under(app, node, page) {
        Some(cell) => {
            app.select_only(cell);
            true
        }
        None => false,
    }
}

/// Moves a block drag to the pointer.
pub fn drag_block(app: &mut App, table: NodeId, anchor: NodeId, page: Vec2) {
    if let Some(hit) = cell_under(app, table, page) {
        select_block(app, table, anchor, hit);
    }
}

/// Double-click on a cell of the selected table or cells: edit its text. Returns whether it did.
pub fn double_click(app: &mut App, page: Vec2) -> bool {
    let table = match app.selection.first().and_then(|id| table_of(&app.doc, *id)) {
        Some(t) => t,
        None => return false,
    };
    match cell_under(app, table, page) {
        Some(cell) => {
            begin_text_edit(app, cell);
            true
        }
        None => false,
    }
}

/// A printable key on one selected cell starts editing it; the typed text goes into the session.
pub fn start_edit_on_text(app: &mut App, ctx: &egui::Context) -> bool {
    let Some(cell) = single_cell(app) else { return false };
    let typed = ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Text(s) if s.chars().any(|c| !c.is_control() && !c.is_whitespace()))));
    if typed {
        begin_text_edit(app, cell);
    }
    typed
}

/// Keeps a cell's row and the table sized to its text while it is typed, in the session's edit.
pub fn preview_relayout(doc: &mut Document, ed: &mut Edit, cell: NodeId) {
    let Some(table) = table_of(doc, cell).filter(|t| *t != cell) else { return };
    for cmd in designos_doc::table::relayout(doc, table) {
        if let Command::Update { node } = cmd {
            if let Some(n) = ed.node(doc, node.id) {
                n.transform = node.transform;
                n.size = node.size;
            }
        }
    }
}

/// Commits the text of an edited cell with the layout it needs, as one undo step, and optionally adds a
/// row after the last one in that same step. Returns the new row's first cell when it did.
pub fn commit_cell(app: &mut App, mut sess: TextSession, add_row: bool) -> Option<NodeId> {
    let id = sess.id;
    let table = table_of(&app.doc, id)?;
    if let Some(range) = sess.preedit.take() {
        if let Some(n) = sess.tx.ed.node(&mut app.doc, id) {
            if let Some(t) = n.kind.any_text_mut() {
                crate::richtext::splice(t, range, "", None);
            }
        }
    }
    let edited = app.doc.get(id)?.clone();
    sess.tx.cancel(&mut app.doc);
    let changed = app.doc.get(id).is_none_or(|cur| cur.kind != edited.kind);
    let mut tx = Tx::new();
    let mut ok = true;
    if changed {
        ok = tx.cmd(&mut app.doc, Command::Update { node: Box::new(edited) }).is_ok();
        for fix in designos_doc::table::relayout(&app.doc, table) {
            ok &= tx.cmd(&mut app.doc, fix).is_ok();
        }
    }
    let mut first = None;
    if ok && add_row {
        let rows = TableGeo::of(&app.doc, table).map_or(0, |g| g.props.rows.len());
        match table_ops::insert_rows(&app.doc, table, rows, 1) {
            Ok(cmd) => {
                ok &= tx.cmd(&mut app.doc, cmd).is_ok();
                first = TableGeo::of(&app.doc, table).and_then(|g| g.cell_covering(rows, 0)).map(|c| c.id);
            }
            Err(e) => app.show_toast(e.to_string(), true),
        }
    }
    if !ok {
        tx.cancel(&mut app.doc);
        app.show_toast("This edit would break the table. Try again.", true);
        return None;
    }
    super::reflow_parents(app, &mut tx, table, None);
    let label = if add_row { "Add row" } else { "Edit cell" };
    app.run_tx_labeled(tx, Some(label.into()));
    app.canvas.dirty = true;
    first
}

/// Tab (or Shift+Tab) while a cell is edited: commit it and edit the next. Past the last cell a row is
/// added. Does nothing during an IME composition.
pub fn tab_in_cell(app: &mut App, back: bool) {
    let Some(sess) = app.canvas.text.as_ref() else { return };
    if sess.preedit.is_some() {
        return;
    }
    let id = sess.id;
    let Some(table) = table_of(&app.doc, id) else { return };
    let next = table_ops::next_cell(&app.doc, table, id, back);
    if next.is_none() && back {
        return;
    }
    let Some(sess) = app.canvas.text.take() else { return };
    let first = commit_cell(app, sess, next.is_none());
    match next.or(first) {
        Some(n) => begin_text_edit(app, n),
        None => app.select_only(id),
    }
}

/// Tab on a selected cell (no edit open): select the next one.
pub fn tab_select(app: &mut App, back: bool) -> bool {
    let Some(cell) = single_cell(app) else { return false };
    let Some(table) = table_of(&app.doc, cell) else { return false };
    if let Some(n) = table_ops::next_cell(&app.doc, table, cell, back) {
        app.select_only(n);
    }
    true
}

/// Delete / Backspace: a whole row or column deletes it, other cells clear their text, a table is
/// deleted by the ordinary layer delete. Returns whether it acted.
pub fn delete_key(app: &mut App) -> bool {
    let Some((table, (r0, c0, r1, c1))) = selected_block(app) else { return false };
    let Some(geo) = TableGeo::of(&app.doc, table) else { return false };
    let (rows, cols) = (geo.props.rows.len(), geo.props.cols.len());
    let whole_row = c0 == 0 && c1 + 1 == cols && !(r0 == 0 && r1 + 1 == rows);
    let whole_col = r0 == 0 && r1 + 1 == rows && !(c0 == 0 && c1 + 1 == cols);
    let action = if whole_row {
        TableAction::DeleteRow
    } else if whole_col {
        TableAction::DeleteCol
    } else {
        TableAction::ClearContent
    };
    super::run(app, action);
    true
}

/// Esc with cells selected: up to the table.
pub fn escape(app: &mut App) -> bool {
    let Some(cell) = app.selection.first().copied().filter(|c| is_cell(&app.doc, *c)) else { return false };
    if let Some(t) = table_of(&app.doc, cell) {
        app.select_only(t);
    }
    true
}

/// Right-click over a cell of the selected table or cells: that cell becomes the selection unless it is
/// already part of it, so the menu acts on what the pointer is on.
pub fn right_click(app: &mut App, page: Vec2) {
    let Some(table) = app.selection.first().and_then(|id| table_of(&app.doc, *id)) else { return };
    if let Some(cell) = cell_under(app, table, page) {
        if !app.selection.contains(&cell) {
            app.select_only(cell);
        }
    }
}

/// Scales the cells of a table being resized, so the drag shows what the release will make. Text keeps its size.
pub fn preview_scale(doc: &mut Document, ed: &mut Edit, table: NodeId, size0: Vec2, now: Vec2) {
    let Some(t) = doc.get(table).filter(|t| matches!(t.kind, NodeKind::Table(_))) else { return };
    let (sx, sy) = (now.x / size0.x.max(1e-3), now.y / size0.y.max(1e-3));
    for id in t.children.clone() {
        let Some(c) = doc.get(id).map(|c| (c.transform, c.size)) else { continue };
        if let Some(n) = ed.node(doc, id) {
            n.transform.tx = c.0.tx * sx;
            n.transform.ty = c.0.ty * sy;
            n.size = Vec2::new(c.1.x * sx, c.1.y * sy);
        }
    }
}

/// Ends a handle drag on a table: one step that moves the table and scales every track in proportion
/// (nothing was written to the document for the drag itself). Gives the drag back for any other node.
pub fn commit_table_resize(app: &mut App, r: crate::canvas::ResizeState) -> Option<crate::canvas::ResizeState> {
    if let crate::canvas::ResizeKind::Multi { nodes, .. } = &r.kind {
        let tables: Vec<_> = nodes
            .iter()
            .filter(|(id, ..)| matches!(app.doc.get(*id).map(|n| &n.kind), Some(NodeKind::Table(_))))
            .map(|(id, _, t0, size0)| (*id, *t0, *size0))
            .collect();
        if tables.is_empty() {
            return Some(r);
        }
        commit_multi_resize(app, r, tables);
        return None;
    }
    let crate::canvas::ResizeKind::Single { id, t0, size0, .. } = &r.kind else { return Some(r) };
    let (id, t0, size0) = (*id, *t0, *size0);
    let Some(n) = app.doc.get(id).filter(|n| matches!(n.kind, NodeKind::Table(_))) else { return Some(r) };
    let (transform, size) = (n.transform, n.size);
    r.tx.cancel(&mut app.doc);
    let mut tx = Tx::new();
    let mut ok = true;
    if transform != t0 {
        if let Some(mut moved) = app.doc.get(id).cloned() {
            moved.transform = transform;
            ok &= tx.cmd(&mut app.doc, Command::Update { node: Box::new(moved) }).is_ok();
        }
    }
    if ok && size != size0 {
        if let Ok(cmd) = table_ops::resize_table(&app.doc, id, size) {
            ok &= tx.cmd(&mut app.doc, cmd).is_ok();
        }
    }
    super::reflow_parents(app, &mut tx, id, None);
    if !ok {
        tx.cancel(&mut app.doc);
        app.show_toast("The table could not be resized. Try again.", true);
    } else {
        app.run_tx_labeled(tx, Some("Resize table".into()));
    }
    app.canvas.dirty = true;
    None
}

/// Whether the canvas owns the keyboard: the keys of a selected table cell (Tab, typing, Enter, Delete) are
/// its alone. Not while the shared overlay gate holds them (dialogs, modals, the palette, menus, marked popups),
/// nor for what only the canvas has to know: the Variables view, a context menu, the eyedropper, a version
/// preview, the agent terminal, a focused widget, or a last press that was not on the canvas itself.
/// The canvas's own egui focus does not close it: that is where the keys belong after Cmd+Alt+J.
pub fn canvas_owns_keys(app: &App, ctx: &egui::Context) -> bool {
    let agent_rail_open = !app.ui.left_hidden() && matches!(app.ui.rail, Some(crate::app::RailItem::Agents | crate::app::RailItem::Tools));
    let canvas_only = app.vars.open
        || app.ui.ctx_menu.is_some()
        || app.props.eyedropper.is_some()
        || app.props.picker.as_ref().is_some_and(|pk| pk.eyedrop)
        || app.history.viewing.is_some()
        // a presentation owns every key (Tab is a prototype trigger), and the editor's selection stays where it was
        || app.proto.present.is_some();
    let focus = crate::agent_a11y::non_canvas_has_focus(ctx) || crate::widgets::field_tab::press_lands_on_field(ctx);
    !(crate::terminal::overlay_owns_keyboard(app, ctx)
        || canvas_only
        || focus
        || app.terminal.is_capturing_keyboard(agent_rail_open)
        || app.canvas.keys_left_canvas)
}

/// Whether a press at `p` is a press on the canvas itself: inside its rectangle, on the background layer that
/// draws it, and not on a floating window (the colour picker, a toolbar, a dropdown) that sits over it.
pub fn press_on_canvas(app: &App, ctx: &egui::Context, p: egui::Pos2) -> bool {
    app.canvas_rect.contains(p) && ctx.layer_id_at(p).is_none_or(|l| l.order == egui::Order::Background)
}

/// Whether the selection is cells of a table.
pub fn cells_selected(app: &App) -> bool {
    app.selection.first().is_some_and(|id| is_cell(&app.doc, *id))
}

/// Tab (Shift+Tab) presses of one frame on a selected cell, taken out of the raw input in the frame they arrive
/// (egui would otherwise move its focus to a field) and acted on at once. Nothing is queued for a later frame.
pub fn take_tab_presses(app: &mut App, raw: &mut egui::RawInput) {
    let mut steps = Vec::new();
    raw.events.retain(|e| match e {
        egui::Event::Key { key: egui::Key::Tab, pressed, modifiers, .. } if !modifiers.command && !modifiers.alt => {
            if *pressed {
                steps.push(modifiers.shift);
            }
            false
        }
        _ => true,
    });
    for back in steps {
        tab_select(app, back);
    }
}

/// Handles a handle drag that ended on a selection holding tables and other layers: every table goes through the
/// table operation in the same step, so its tracks, cells and size stay one thing.
fn commit_multi_resize(app: &mut App, mut r: crate::canvas::ResizeState, tables: Vec<(NodeId, designos_doc::Affine, Vec2)>) {
    let mut finals = Vec::new();
    for (id, t0, size0) in &tables {
        let Some(n) = app.doc.get_mut(*id) else { continue };
        finals.push((*id, n.transform, n.size, *t0));
        // the drag wrote the table's box only: put it back and let the operation write all of it
        n.transform = *t0;
        n.size = *size0;
        r.tx.ed.forget(*id);
    }
    let mut ok = true;
    for (id, transform, size, t0) in &finals {
        if transform != t0 {
            if let Some(mut moved) = app.doc.get(*id).cloned() {
                moved.transform = *transform;
                ok &= r.tx.cmd(&mut app.doc, Command::Update { node: Box::new(moved) }).is_ok();
            }
        }
        if let Ok(cmd) = table_ops::resize_table(&app.doc, *id, *size) {
            ok &= r.tx.cmd(&mut app.doc, cmd).is_ok();
        }
        super::reflow_parents(app, &mut r.tx, *id, None);
    }
    if ok {
        app.run_tx_labeled(r.tx, Some("Resize layers".into()));
    } else {
        r.tx.cancel(&mut app.doc);
        app.show_toast("The layers could not be resized. Try again.", true);
    }
    app.canvas.dirty = true;
}
