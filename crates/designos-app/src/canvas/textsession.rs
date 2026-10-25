//! The text editing session: caret and selection over UTF-8 byte offsets,
//! keyboard / clipboard / IME handling, per-range styling and in-session undo.
//!
//! While a session is open the node is mutated live through the session's
//! transaction; closing it commits everything as one document undo step.
//! Inside the session Ctrl+Z / Ctrl+Shift+Z step through finer edits (typing
//! coalesced per word or burst, see [`crate::textedit::UndoLog`]).

use super::tools::Tool;
use super::*;
use crate::layout;
use crate::richtext::{self, ListKind};
use crate::textedit::{self as te, EditKind, Snap, UndoLog};
use crate::textmeasure as tm;
use crate::util::*;
use designos_doc::{NodeKind, TextDecoration, TextProps, TextStyle};
use std::ops::Range;

pub struct TextSession {
    pub id: NodeId,
    /// Caret as a byte offset into the content (always on a char boundary).
    pub caret: usize,
    /// The other end of the selection; equal to `caret` when collapsed.
    pub anchor: usize,
    pub tx: Tx,
    pub created: bool,
    /// Column (node-local x) kept while moving up/down.
    pub goal_x: Option<f32>,
    /// Style for the next typed text when the caret is collapsed (set by Ctrl+B etc.).
    pub typing: Option<TextStyle>,
    pub log: UndoLog,
    /// Active IME composition (byte range inside the content).
    pub preedit: Option<Range<usize>>,
    /// The pointer is dragging a selection.
    pub dragging: bool,
    /// Where the caret is drawn when its offset has two places on screen (the boundary of a
    /// right-to-left and a left-to-right run): set by clicks and arrow keys, cleared by edits.
    pub hint_x: Option<f32>,
    /// Tab presses (true = with Shift) taken out of the raw input before egui could use them for
    /// focus navigation; handled with the other keys of the frame.
    pub tabs: Vec<bool>,
}

impl TextSession {
    pub fn new(id: NodeId, caret: usize, anchor: usize, tx: Tx, created: bool) -> Self {
        Self {
            id,
            caret,
            anchor,
            tx,
            created,
            goal_x: None,
            typing: None,
            log: UndoLog::default(),
            preedit: None,
            dragging: false,
            hint_x: None,
            tabs: Vec::new(),
        }
    }

    /// Selected byte range (empty when only a caret).
    pub fn range(&self) -> Range<usize> {
        self.caret.min(self.anchor)..self.caret.max(self.anchor)
    }
}

/// Commands that edit the style of the selected range or text layers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextCmd {
    Bold,
    Italic,
    Underline,
    SizeUp,
    SizeDown,
    List(ListKind),
}

// ------------------------------------------------------------------ lifecycle
pub fn begin_text_edit(app: &mut App, id: NodeId) {
    finish_text(app, true);
    // text layers, and the text of stickies, shapes and connector labels
    let len = match app.doc.get(id).and_then(|n| n.kind.any_text()) {
        Some(t) => t.content.len(),
        _ => return,
    };
    app.selection = vec![id];
    // Like Figma, entering edit mode selects all the text; when the layer is the current Find result
    // the match is selected instead (so Enter / double-click after "next match" lands on it).
    let (mut caret, mut anchor) = (len, 0);
    if app.find.open {
        let hit = app.find.current.and_then(|i| app.find.results.get(i)).filter(|m| m.node == id && m.field == designos_doc::find::MatchField::Text);
        if let Some(m) = hit.filter(|m| m.range.end <= len) {
            (caret, anchor) = (m.range.end, m.range.start);
        }
    }
    app.canvas.text = Some(TextSession::new(id, caret, anchor, Tx::new(), false));
}

pub fn finish_text(app: &mut App, commit: bool) {
    let Some(mut sess) = app.canvas.text.take() else { return };
    if let Some(range) = sess.preedit.take() {
        // an unfinished composition is discarded
        if let Some(n) = sess.tx.ed.node(&mut app.doc, sess.id) {
            if let Some(t) = n.kind.any_text_mut() {
                richtext::splice(t, range, "", None);
            }
        }
    }
    let empty = app.doc.get(sess.id).and_then(|n| n.kind.any_text()).is_some_and(|t| t.content.is_empty());
    if empty && commit {
        // a connector label emptied out is no label at all
        if let Some(n) = sess.tx.ed.node(&mut app.doc, sess.id) {
            if let NodeKind::Connector(c) = &mut n.kind {
                c.label = None;
            }
        }
        if let Some(g) = designos_doc::connector_geometry(&app.doc, sess.id) {
            if let Some(n) = sess.tx.ed.node(&mut app.doc, sess.id) {
                g.apply(n);
            }
        }
    }
    if (sess.created && empty) || !commit {
        sess.tx.cancel(&mut app.doc);
        app.sanitize();
    } else {
        let id = sess.id;
        // text in Arabic, Hebrew or Devanagari needs its Noto fallback font (fetched once, then cached)
        let complex = app.doc.get(id).and_then(|n| n.kind.any_text()).is_some_and(|t| !designos_fonts::scripts_in(&t.content).is_empty());
        app.run_tx(sess.tx);
        app.selection = vec![id];
        if complex {
            app.fonts.load_for(&app.doc);
        }
    }
    app.changed();
}

// ------------------------------------------------------------------ queries
fn props_of(app: &App) -> Option<(TextProps, DV)> {
    let f = frame_of(app)?;
    Some((f.props, f.size))
}

/// The edited text as stored in the node (a sticky's props keep the largest size; the frame's
/// props carry the size fitted for drawing).
fn stored_of(app: &App) -> Option<TextProps> {
    let sess = app.canvas.text.as_ref()?;
    app.doc.get(sess.id)?.kind.any_text().cloned()
}

/// Where the edited text is laid out (see [`crate::board::create::text_frame`]).
fn frame_of(app: &App) -> Option<crate::board::create::TextFrame> {
    let sess = app.canvas.text.as_ref()?;
    crate::board::create::text_frame(&app.doc, app.doc.get(sess.id)?)
}

/// Node-local point of a page-space point for the edited node.
fn to_local(app: &App, id: NodeId, page: DV) -> Option<DV> {
    Some(app.doc.world_transform(id).invert()?.apply(page))
}

/// Whether `page` is on (or within 2px of) the edited node.
pub fn node_contains(app: &App, id: NodeId, page: DV) -> bool {
    let Some(n) = app.doc.get(id) else { return false };
    let Some(l) = to_local(app, id, page) else { return false };
    l.x >= -2.0 && l.y >= -2.0 && l.x <= n.size.x + 2.0 && l.y <= n.size.y + 2.0
}

/// Byte offset under the page-space point in the edited node.
fn hit_at(app: &App, page: DV) -> Option<usize> {
    hit_stop_at(app, page).map(|h| h.0)
}

/// Like [`hit_at`] with the x of the caret stop that was hit.
fn hit_stop_at(app: &App, page: DV) -> Option<(usize, f32)> {
    let id = app.canvas.text.as_ref()?.id;
    let l = to_local(app, id, page)?;
    let f = frame_of(app)?;
    Some(tm::hit_stop(&f.props, f.size, l.x - f.origin.x, l.y - f.origin.y))
}

fn set_hint(app: &mut App, x: f32) {
    if let Some(s) = app.canvas.text.as_mut() {
        s.hint_x = Some(x);
    }
}

// ------------------------------------------------------------------ selection
fn set_sel(app: &mut App, caret: usize, anchor: usize, keep_goal: bool) {
    let Some(s) = app.canvas.text.as_mut() else { return };
    s.caret = caret;
    s.anchor = anchor;
    s.hint_x = None;
    if !keep_goal {
        s.goal_x = None;
    }
    s.typing = None;
    s.log.break_burst();
    app.canvas.dirty = true;
}

/// Press at `page`: places the caret (Shift extends) and starts a drag selection.
pub fn press_at(app: &mut App, page: DV, shift: bool) {
    let Some((at, x)) = hit_stop_at(app, page) else { return };
    let anchor = if shift { app.canvas.text.as_ref().map(|s| s.anchor).unwrap_or(at) } else { at };
    set_sel(app, at, anchor, false);
    set_hint(app, x);
    if let Some(s) = app.canvas.text.as_mut() {
        s.dragging = true;
    }
}

/// Pointer moved while dragging a selection.
pub fn drag_to(app: &mut App, page: DV) {
    let Some((at, x)) = hit_stop_at(app, page) else { return };
    let anchor = app.canvas.text.as_ref().map(|s| s.anchor).unwrap_or(at);
    set_sel(app, at, anchor, false);
    set_hint(app, x);
}

/// Double-click: selects the word under the pointer.
pub fn select_word(app: &mut App, page: DV) {
    let Some(at) = hit_at(app, page) else { return };
    let Some((t, _)) = props_of(app) else { return };
    let r = te::word_at(&t.content, at);
    set_sel(app, r.end, r.start, false);
}

/// Triple-click: selects the paragraph under the pointer.
pub fn select_paragraph(app: &mut App, page: DV) {
    let Some(at) = hit_at(app, page) else { return };
    let Some((t, _)) = props_of(app) else { return };
    let r = te::paragraph_at(&t.content, at);
    set_sel(app, r.end, r.start, false);
}

fn select_all(app: &mut App) {
    let Some((t, _)) = props_of(app) else { return };
    set_sel(app, t.content.len(), 0, false);
}

/// Movement keys; `shift` extends the selection, `ctrl` moves by word / to text edges.
fn move_caret(app: &mut App, key: egui::Key, shift: bool, ctrl: bool) {
    let Some((t, size)) = props_of(app) else { return };
    let Some(sess) = app.canvas.text.as_ref() else { return };
    let (caret, anchor, goal, hint) = (sess.caret, sess.anchor, sess.goal_x, sess.hint_x);
    let range = sess.range();
    let c = &t.content;
    let mut keep_goal = false;
    let mut new_hint = None;
    // Left / Right follow the screen: inside a right-to-left run they go forward in the text.
    let rtl = matches!(key, egui::Key::ArrowLeft | egui::Key::ArrowRight) && tm::run_rtl(&t, size, caret);
    let target = match key {
        egui::Key::ArrowLeft if !shift && !range.is_empty() => {
            if rtl {
                range.end
            } else {
                range.start
            }
        }
        egui::Key::ArrowRight if !shift && !range.is_empty() => {
            if rtl {
                range.start
            } else {
                range.end
            }
        }
        egui::Key::ArrowLeft | egui::Key::ArrowRight => {
            let dir = if key == egui::Key::ArrowLeft { -1 } else { 1 };
            // logical direction of travel at the caret
            let forward = (dir > 0) != rtl;
            if ctrl {
                if forward {
                    te::next_word(c, caret)
                } else {
                    te::prev_word(c, caret)
                }
            } else if let Some((at, x)) = tm::move_visual(&t, size, caret, hint, dir) {
                new_hint = Some(x);
                at
            } else {
                // at the end of the line: carry on into the neighbouring line in text order
                let line_rtl = tm::line_rtl(&t, size, caret);
                if (dir > 0) != line_rtl {
                    te::next_grapheme(c, caret)
                } else {
                    te::prev_grapheme(c, caret)
                }
            }
        }
        egui::Key::ArrowUp | egui::Key::ArrowDown => {
            let gx = goal.unwrap_or_else(|| tm::caret_rect(&t, size, caret).0);
            keep_goal = true;
            if let Some(s) = app.canvas.text.as_mut() {
                s.goal_x = Some(gx);
            }
            tm::move_vertical(&t, size, caret, if key == egui::Key::ArrowUp { -1 } else { 1 }, gx)
        }
        egui::Key::Home if ctrl => 0,
        egui::Key::End if ctrl => c.len(),
        egui::Key::Home => tm::line_bounds(&t, size, caret).0,
        egui::Key::End => tm::line_bounds(&t, size, caret).1,
        _ => return,
    };
    set_sel(app, target, if shift { anchor } else { target }, keep_goal);
    if let Some(x) = new_hint {
        set_hint(app, x);
    }
}

// ------------------------------------------------------------------ editing
/// A connector label that grew or shrank changes the extent of its connector.
fn follow_label(doc: &mut designos_doc::Document, tx: &mut Tx, id: NodeId) {
    if !matches!(doc.get(id).map(|n| &n.kind), Some(NodeKind::Connector(_))) {
        return;
    }
    if let Some(g) = designos_doc::connector_geometry(doc, id) {
        if let Some(n) = tx.ed.node(doc, id) {
            g.apply(n);
        }
    }
}

/// Runs `f` on the edited text. `log` records an undo step (None = no step, for IME previews).
fn edit_props(app: &mut App, log: Option<(EditKind, bool)>, f: impl FnOnce(&mut TextProps, &mut usize, &mut usize)) {
    let now = app.now;
    let App { doc, canvas, .. } = app;
    let Some(sess) = canvas.text.as_mut() else { return };
    let id = sess.id;
    let (mut caret, mut anchor) = (sess.caret, sess.anchor);
    let mut before = None;
    let mut added_faces = Vec::new();
    if let Some(n) = sess.tx.ed.node(doc, id) {
        let is_text = matches!(n.kind, NodeKind::Text(_));
        if let Some(t) = n.kind.any_text_mut() {
            let snap = Snap { props: t.clone(), caret, anchor };
            f(t, &mut caret, &mut anchor);
            if *t != snap.props {
                added_faces = designos_fonts::new_faces(&snap.props, t);
                before = Some(snap);
            }
            if is_text {
                n.size = layout::fit_text(t, n.size);
            }
        }
    }
    follow_label(doc, &mut sess.tx, id);
    if let (Some(b), Some((kind, new_word))) = (before, log) {
        sess.log.record(b, kind, now, new_word);
    }
    sess.caret = caret;
    sess.anchor = anchor;
    sess.hint_x = None;
    layout::relayout_up(doc, &mut sess.tx.ed, id);
    canvas.dirty = true;
    // Every session edit ends here, so a weight, family or italic that is new to the text (Ctrl+B,
    // Edit > Bold, the inspector, a bold typing style) is asked for in the same frame. Typing and
    // edits that keep the faces start no load.
    if !added_faces.is_empty() {
        app.fonts.request_new_faces(&app.doc, added_faces);
    }
}

/// Runs `edit`, a style change to the text layers `ids` made outside a session, and asks for the
/// font faces it introduced (see `edit_props` for the session case).
pub fn requesting_new_faces(app: &mut App, ids: &[NodeId], edit: impl FnOnce(&mut App)) {
    let before: Vec<(NodeId, TextProps)> = ids.iter().filter_map(|id| Some((*id, app.doc.get(*id)?.kind.any_text()?.clone()))).collect();
    edit(app);
    let mut added: Vec<_> = before.iter().filter_map(|(id, b)| Some(designos_fonts::new_faces(b, app.doc.get(*id)?.kind.any_text()?))).flatten().collect();
    added.sort();
    added.dedup();
    if !added.is_empty() {
        app.fonts.request_new_faces(&app.doc, added);
    }
}

/// Replaces the selection with `s` (typing / paste).
pub fn insert_text(app: &mut App, s: &str, kind: EditKind) {
    let s = te::clean_input(s);
    if s.is_empty() {
        return;
    }
    let typing = app.canvas.text.as_mut().and_then(|x| x.typing.take());
    let Some(sess) = app.canvas.text.as_ref() else { return };
    let new_word = props_of(app).is_some_and(|(t, _)| te::starts_new_word(&t.content, sess.range().start, &s)) || s.contains('\n');
    edit_props(app, Some((kind, new_word)), |t, caret, anchor| {
        let r = (*caret).min(*anchor)..(*caret).max(*anchor);
        let end = richtext::splice(t, r, &s, typing.clone());
        *caret = end;
        *anchor = end;
    });
    if let Some(sess) = app.canvas.text.as_mut() {
        sess.goal_x = None;
    }
}

fn delete(app: &mut App, forward: bool, ctrl: bool) {
    // Backspace at the start of a list item turns it into a plain paragraph (outdents nested ones).
    if !forward {
        if let Some(caret) = app.canvas.text.as_ref().filter(|s| s.range().is_empty()).map(|s| s.caret) {
            if props_of(app).is_some_and(|(t, _)| t.has_lists()) {
                let mut done = false;
                edit_props(app, Some((EditKind::Deleting, true)), |t, _, _| done = richtext::backspace_at_item_start(t, caret));
                if done {
                    return;
                }
            }
        }
    }
    edit_props(app, Some((EditKind::Deleting, false)), |t, caret, anchor| {
        let mut r = (*caret).min(*anchor)..(*caret).max(*anchor);
        if r.is_empty() {
            r = if forward {
                *caret..if ctrl { te::next_word(&t.content, *caret) } else { te::next_grapheme(&t.content, *caret) }
            } else {
                (if ctrl { te::prev_word(&t.content, *caret) } else { te::prev_grapheme(&t.content, *caret) })..*caret
            };
        }
        if r.is_empty() {
            return;
        }
        richtext::splice(t, r.clone(), "", None);
        *caret = r.start;
        *anchor = r.start;
    });
    if let Some(sess) = app.canvas.text.as_mut() {
        sess.goal_x = None;
        sess.typing = None;
    }
}

fn enter(app: &mut App) {
    // Enter on an empty list item ends the list (or outdents a nested item); elsewhere the
    // newline keeps the paragraph's list membership, so Enter inside an item continues the list.
    let caret = app.canvas.text.as_ref().filter(|s| s.range().is_empty()).map(|s| s.caret);
    if let Some(caret) = caret {
        let mut done = false;
        edit_props(app, Some((EditKind::Other, true)), |t, _, _| done = richtext::exit_empty_item(t, caret));
        if done {
            return;
        }
    }
    insert_text(app, "\n", EditKind::Other);
}

/// Tab / Shift+Tab: nest or un-nest the list items touched by the selection.
fn change_list_level(app: &mut App, delta: i8) {
    let Some(range) = app.canvas.text.as_ref().map(|s| s.range()) else { return };
    edit_props(app, Some((EditKind::Other, true)), |t, _, _| {
        richtext::change_level(t, range.clone(), delta);
    });
}

/// In-session undo (`redo = false`) or redo.
fn undo_step(app: &mut App, redo: bool) {
    let now = (|| {
        let t = stored_of(app)?;
        let s = app.canvas.text.as_ref()?;
        Some(Snap { props: t, caret: s.caret, anchor: s.anchor })
    })();
    let Some(cur) = now else { return };
    let Some(sess) = app.canvas.text.as_mut() else { return };
    let snap = if redo { sess.log.redo(cur) } else { sess.log.undo(cur) };
    let Some(snap) = snap else { return };
    let App { doc, canvas, .. } = app;
    let sess = canvas.text.as_mut().unwrap();
    let id = sess.id;
    if let Some(n) = sess.tx.ed.node(doc, id) {
        let is_text = matches!(n.kind, NodeKind::Text(_));
        if let Some(t) = n.kind.any_text_mut() {
            *t = snap.props;
            if is_text {
                n.size = layout::fit_text(t, n.size);
            }
        }
    }
    follow_label(doc, &mut sess.tx, id);
    sess.caret = snap.caret;
    sess.anchor = snap.anchor;
    sess.hint_x = None;
    sess.typing = None;
    layout::relayout_up(doc, &mut sess.tx.ed, id);
    canvas.dirty = true;
}

// ------------------------------------------------------------------ styling
/// Applies `f` to the selected range of the open session (or to the typing style
/// of a collapsed caret; or to the whole text when it is empty). Returns false
/// when no session is open.
pub fn style_selection(app: &mut App, f: impl Fn(&mut TextStyle)) -> bool {
    style_selection_as(app, f, EditKind::Other)
}

/// [`style_selection`] with an explicit undo kind (inspector edits coalesce).
pub fn style_selection_as(app: &mut App, f: impl Fn(&mut TextStyle), kind: EditKind) -> bool {
    let Some(sess) = app.canvas.text.as_ref() else { return false };
    let range = sess.range();
    let empty_text = props_of(app).is_none_or(|(t, _)| t.content.is_empty());
    if range.is_empty() && !empty_text {
        let Some(t) = stored_of(app) else { return true };
        let caret = app.canvas.text.as_ref().map(|s| s.caret).unwrap_or(0);
        let mut st = app.canvas.text.as_ref().and_then(|s| s.typing.clone()).unwrap_or_else(|| richtext::style_for_typing(&t, caret));
        richtext::weight_in_step(&f)(&mut st);
        if let Some(s) = app.canvas.text.as_mut() {
            s.typing = Some(st);
        }
        return true;
    }
    edit_props(app, Some((kind, kind == EditKind::Other)), |t, _, _| {
        if empty_text {
            richtext::apply_all(t, &f);
        } else {
            richtext::apply_style(t, range.clone(), &f);
        }
    });
    true
}

/// Edits the base style of the edited text (paragraph level fields such as alignment).
pub fn base_style(app: &mut App, f: impl Fn(&mut TextStyle)) {
    edit_props(app, Some((EditKind::Other, true)), |t, _, _| f(&mut t.style));
}

/// Edits the text properties of the open session's text (paragraph settings, lists, direction).
pub fn edit_text_props(app: &mut App, f: impl Fn(&mut TextProps)) {
    edit_props(app, Some((EditKind::Other, true)), |t, _, _| f(t));
}

/// Style summary of the open session's selection (for the inspector).
pub fn session_summary(app: &App) -> Option<richtext::Summary> {
    let t = stored_of(app)?;
    let s = app.canvas.text.as_ref()?;
    let mut sum = richtext::summary(&t, s.range());
    if let (true, Some(ty)) = (s.range().is_empty(), &s.typing) {
        sum = richtext::summary(&TextProps { content: "x".into(), style: ty.clone(), runs: vec![], ..t }, 0..0);
    }
    Some(sum)
}

/// Runs a styling command on the open session, or on the selected text layers.
pub fn text_command(app: &mut App, cmd: TextCmd) {
    if app.canvas.text.is_some() {
        let sum = session_summary(app);
        match cmd {
            TextCmd::List(kind) => {
                let Some(range) = app.canvas.text.as_ref().map(|s| s.range()) else { return };
                edit_props(app, Some((EditKind::Other, true)), |t, _, _| richtext::toggle_list(t, range.clone(), kind));
            }
            c => {
                let f = style_fn(c, sum.as_ref());
                style_selection(app, f);
            }
        }
        return;
    }
    let ids = app.selection.clone();
    requesting_new_faces(app, &ids, |app| {
        for &id in &ids {
            let Some(t) = app.doc.get(id).and_then(|n| n.kind.any_text()).cloned() else { continue };
            let sum = richtext::summary(&t, 0..t.content.len());
            match cmd {
                TextCmd::List(kind) => {
                    app.edit_nodes(&[id], |n| {
                        let is_text = matches!(n.kind, NodeKind::Text(_));
                        if let Some(t) = n.kind.any_text_mut() {
                            let len = t.content.len();
                            richtext::toggle_list(t, 0..len, kind);
                            if is_text {
                                n.size = layout::fit_text(t, n.size);
                            }
                        }
                    });
                }
                c => {
                    let f = style_fn(c, Some(&sum));
                    app.edit_nodes(&[id], |n| {
                        let is_text = matches!(n.kind, NodeKind::Text(_));
                        if let Some(t) = n.kind.any_text_mut() {
                            richtext::apply_all(t, &f);
                            if is_text {
                                n.size = layout::fit_text(t, n.size);
                            }
                        }
                    });
                }
            }
        }
    });
}

fn style_fn(cmd: TextCmd, sum: Option<&richtext::Summary>) -> impl Fn(&mut TextStyle) {
    let bold = sum.and_then(|s| s.weight).is_some_and(|w| w >= 600);
    let italic = sum.and_then(|s| s.italic) == Some(true);
    let underline = sum.and_then(|s| s.decoration) == Some(TextDecoration::Underline);
    move |st: &mut TextStyle| match cmd {
        TextCmd::Bold => st.font_weight = if bold { 400 } else { 700 },
        TextCmd::Italic => st.italic = !italic,
        TextCmd::Underline => st.decoration = if underline { TextDecoration::None } else { TextDecoration::Underline },
        TextCmd::SizeUp => st.font_size = (st.font_size + 1.0).min(1000.0),
        TextCmd::SizeDown => st.font_size = (st.font_size - 1.0).max(1.0),
        TextCmd::List(_) => {}
    }
}

// ------------------------------------------------------------------ clipboard
fn selected_text(app: &App) -> Option<String> {
    let (t, _) = props_of(app)?;
    let r = app.canvas.text.as_ref()?.range();
    (!r.is_empty()).then(|| t.content[r].to_string())
}

fn cut(app: &mut App) {
    delete(app, false, false);
}

// ------------------------------------------------------------------ IME
fn preedit(app: &mut App, text: &str) {
    let old = app.canvas.text.as_mut().and_then(|s| s.preedit.take());
    let sel_start = app.canvas.text.as_ref().map(|s| s.range().start).unwrap_or(0);
    let mut new_range = None;
    edit_props(app, None, |t, caret, anchor| {
        let mut at = (*caret).min(*anchor);
        if let Some(r) = old.clone() {
            richtext::splice(t, r.clone(), "", None);
            at = r.start;
        } else {
            let r = (*caret).min(*anchor)..(*caret).max(*anchor);
            if !r.is_empty() {
                richtext::splice(t, r.clone(), "", None);
                at = r.start;
            }
        }
        let _ = sel_start;
        if !text.is_empty() {
            let mut st = richtext::style_for_typing(t, at);
            st.decoration = TextDecoration::Underline;
            let end = richtext::splice(t, at..at, text, Some(st));
            new_range = Some(at..end);
            *caret = end;
        } else {
            *caret = at;
        }
        *anchor = *caret;
    });
    if let Some(s) = app.canvas.text.as_mut() {
        s.preedit = new_range;
    }
}

fn commit_ime(app: &mut App, text: &str) {
    // Drop the composition text, then insert the final string as regular typing.
    preedit(app, "");
    insert_text(app, text, EditKind::Typing);
}

// ------------------------------------------------------------------ events
pub fn text_keys(app: &mut App, ctx: &Context) {
    // A focused inspector field owns the keyboard.
    if ctx.egui_wants_keyboard_input() {
        return;
    }
    let tabs = app.canvas.text.as_mut().map(|s| std::mem::take(&mut s.tabs)).unwrap_or_default();
    for shift in tabs {
        tab(app, shift);
    }
    let events = ctx.input(|i| i.events.clone());
    for ev in events {
        if app.canvas.text.is_none() {
            return;
        }
        match ev {
            egui::Event::Text(s) => insert_text(app, &s, EditKind::Typing),
            egui::Event::Paste(s) => insert_text(app, &s, EditKind::Other),
            egui::Event::Copy => {
                if let Some(s) = selected_text(app) {
                    ctx.copy_text(s);
                }
            }
            egui::Event::Cut => {
                if let Some(s) = selected_text(app) {
                    ctx.copy_text(s);
                    cut(app);
                }
            }
            egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => preedit(app, &text),
            egui::Event::Ime(egui::ImeEvent::Commit(text)) => commit_ime(app, &text),
            egui::Event::Key { key, pressed: true, modifiers, .. } => {
                if !key_event(app, key, modifiers) {
                    return;
                }
            }
            _ => {}
        }
    }
}

/// Handles one key press; returns false when the session ended.
fn key_event(app: &mut App, key: egui::Key, m: egui::Modifiers) -> bool {
    use egui::Key as K;
    let ctrl = m.command;
    // an unfinished IME composition owns navigation / editing keys
    if app.canvas.text.as_ref().is_some_and(|s| s.preedit.is_some()) && !matches!(key, K::Escape) {
        return true;
    }
    match key {
        K::Escape => {
            finish_text(app, true);
            app.tool = Tool::Move;
            return false;
        }
        K::Backspace => delete(app, false, ctrl),
        K::Delete => delete(app, true, ctrl),
        K::Enter => enter(app),
        K::ArrowLeft | K::ArrowRight | K::ArrowUp | K::ArrowDown | K::Home | K::End => move_caret(app, key, m.shift, ctrl),
        K::A if ctrl => select_all(app),
        K::B if ctrl => text_command(app, TextCmd::Bold),
        K::I if ctrl => text_command(app, TextCmd::Italic),
        K::U if ctrl => text_command(app, TextCmd::Underline),
        K::Period if ctrl && m.shift => text_command(app, TextCmd::SizeUp),
        K::Comma if ctrl && m.shift => text_command(app, TextCmd::SizeDown),
        K::Tab if !ctrl => tab(app, m.shift),
        K::Num7 if ctrl && m.shift => text_command(app, TextCmd::List(ListKind::Numbered)),
        K::Num8 if ctrl && m.shift => text_command(app, TextCmd::List(ListKind::Bullet)),
        K::Z if ctrl && !m.shift => undo_step(app, false),
        K::Z if ctrl && m.shift => undo_step(app, true),
        K::Y if ctrl => undo_step(app, true),
        _ => {}
    }
    true
}

/// Tab: on a sticky note it starts the next one to the right; elsewhere it changes the list level.
fn tab(app: &mut App, shift: bool) {
    let on_sticky = app.canvas.text.as_ref().and_then(|s| app.doc.get(s.id)).is_some_and(|n| matches!(n.kind, NodeKind::Sticky(_)));
    if on_sticky {
        if !shift {
            crate::board::create::tab_next(app);
        }
    } else {
        change_list_level(app, if shift { -1 } else { 1 });
    }
}

/// Screen rect of the caret (for the IME candidate window), if a session is open.
pub fn caret_screen_rect(app: &App, origin: egui::Pos2) -> Option<ERect> {
    let sess = app.canvas.text.as_ref()?;
    let f = frame_of(app)?;
    let (x, y, h) = tm::caret_rect_hint(&f.props, f.size, sess.caret, sess.hint_x);
    let (x, y) = (x + f.origin.x, y + f.origin.y);
    let w = app.doc.world_transform(sess.id);
    let a = app.view.to_screen(origin, w.apply(v2(x, y)));
    let b = app.view.to_screen(origin, w.apply(v2(x, y + h)));
    Some(ERect::from_two_pos(a, b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Document, Node};

    fn app_with_text(s: &str) -> (App, NodeId) {
        app_with_styled_text(s, TextStyle { font_size: 16.0, ..Default::default() })
    }

    fn app_with_styled_text(s: &str, style: TextStyle) -> (App, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let mut n = Node::text(id, Rect::new(0.0, 0.0, 10.0, 10.0), s, style);
        if let NodeKind::Text(t) = &n.kind {
            n.size = layout::estimate_text_size(t);
        }
        d.insert(page, n).unwrap();
        let mut app = App::new(d);
        begin_text_edit(&mut app, id);
        (app, id)
    }

    fn text_of(app: &App, id: NodeId) -> TextProps {
        match &app.doc.get(id).unwrap().kind {
            NodeKind::Text(t) => t.clone(),
            _ => panic!(),
        }
    }

    fn key(app: &mut App, k: egui::Key, shift: bool, ctrl: bool) {
        key_event(app, k, egui::Modifiers { shift, command: ctrl, ctrl, ..Default::default() });
    }

    #[test]
    fn entering_selects_all_and_typing_replaces() {
        let (mut app, id) = app_with_text("hello");
        assert_eq!(app.canvas.text.as_ref().unwrap().range(), 0..5);
        insert_text(&mut app, "bye", EditKind::Typing);
        assert_eq!(text_of(&app, id).content, "bye");
        assert_eq!(app.canvas.text.as_ref().unwrap().caret, 3);
    }

    #[test]
    fn arrows_shift_select_and_collapse() {
        let (mut app, _) = app_with_text("héllo wörld");
        key(&mut app, egui::Key::Home, false, false);
        for _ in 0..3 {
            key(&mut app, egui::Key::ArrowRight, true, false);
        }
        assert_eq!(app.canvas.text.as_ref().unwrap().range(), 0.."hél".len());
        // plain right collapses to the end of the selection
        key(&mut app, egui::Key::ArrowRight, false, false);
        let s = app.canvas.text.as_ref().unwrap();
        assert_eq!((s.caret, s.anchor), ("hél".len(), "hél".len()));
        // ctrl+right jumps a word, ctrl+shift extends
        key(&mut app, egui::Key::ArrowRight, true, true);
        assert_eq!(app.canvas.text.as_ref().unwrap().range(), "hél".len().."héllo".len());
        key(&mut app, egui::Key::End, false, true);
        assert_eq!(app.canvas.text.as_ref().unwrap().caret, "héllo wörld".len());
        key(&mut app, egui::Key::ArrowLeft, false, true);
        assert_eq!(app.canvas.text.as_ref().unwrap().caret, "héllo ".len());
    }

    #[test]
    fn backspace_deletes_graphemes_and_words() {
        let (mut app, id) = app_with_text("a👨‍👩‍👧 two words");
        key(&mut app, egui::Key::Home, false, true);
        key(&mut app, egui::Key::ArrowRight, false, false);
        key(&mut app, egui::Key::ArrowRight, false, false);
        key(&mut app, egui::Key::Backspace, false, false);
        assert_eq!(text_of(&app, id).content, "a two words", "the whole ZWJ emoji goes in one press");
        key(&mut app, egui::Key::End, false, true);
        key(&mut app, egui::Key::Backspace, false, true);
        assert_eq!(text_of(&app, id).content, "a two ");
        key(&mut app, egui::Key::Delete, false, false);
        assert_eq!(text_of(&app, id).content, "a two ");
    }

    #[test]
    fn ctrl_a_and_clipboard_text() {
        let (mut app, _) = app_with_text("abc def");
        key(&mut app, egui::Key::Home, false, true);
        key(&mut app, egui::Key::A, false, true);
        assert_eq!(selected_text(&app).as_deref(), Some("abc def"));
        key(&mut app, egui::Key::ArrowRight, false, false);
        assert_eq!(selected_text(&app), None, "collapsed after Right");
    }

    #[test]
    fn per_range_bold_and_mixed_summary() {
        let (mut app, id) = app_with_text("hello world");
        set_sel(&mut app, 5, 0, false);
        text_command(&mut app, TextCmd::Bold);
        let t = text_of(&app, id);
        assert_eq!(t.runs.len(), 1);
        assert_eq!((t.runs[0].start, t.runs[0].end, t.runs[0].style.font_weight), (0, 5, 700));
        assert_eq!(t.style.font_weight, 400);
        set_sel(&mut app, 11, 0, false);
        assert_eq!(session_summary(&app).unwrap().weight, None, "mixed");
        // toggling over a mixed range makes it bold first
        text_command(&mut app, TextCmd::Bold);
        let t = text_of(&app, id);
        assert!(t.runs.is_empty());
        assert_eq!(t.style.font_weight, 700);
        text_command(&mut app, TextCmd::Bold);
        assert_eq!(text_of(&app, id).style.font_weight, 400);
    }

    /// A family that is not bundled, so its faces have to be fetched.
    fn lato() -> TextStyle {
        TextStyle { font_family: "TestLato".into(), font_size: 16.0, ..Default::default() }
    }

    fn asked_for(app: &App, family: &str, weight: u16) -> bool {
        app.fonts.requested().contains(&designos_fonts::FontRequest::new(family, weight, false))
    }

    #[test]
    fn ctrl_b_requests_the_bold_face_of_a_downloaded_family() {
        let (mut app, id) = app_with_styled_text("hello world", lato());
        set_sel(&mut app, 5, 0, false);
        key(&mut app, egui::Key::B, false, true);
        assert_eq!(text_of(&app, id).runs[0].style.font_weight, 700);
        assert!(asked_for(&app, "TestLato", 700), "the 700 face must be requested in the frame Ctrl+B ran: {:?}", app.fonts.requested());
    }

    #[test]
    fn edit_menu_bold_requests_the_bold_face_in_a_session_and_on_a_selected_layer() {
        let (mut app, id) = app_with_styled_text("hello world", lato());
        set_sel(&mut app, 5, 0, false);
        text_command(&mut app, TextCmd::Bold);
        assert!(asked_for(&app, "TestLato", 700), "session: {:?}", app.fonts.requested());

        let (mut app, id2) = app_with_styled_text("hello world", lato());
        finish_text(&mut app, true);
        app.selection = vec![id2];
        text_command(&mut app, TextCmd::Bold);
        assert_eq!(text_of(&app, id2).style.font_weight, 700);
        assert!(asked_for(&app, "TestLato", 700), "selected layer: {:?}", app.fonts.requested());
        let _ = id;
    }

    #[test]
    fn bold_set_before_typing_requests_the_face_once_the_bold_text_exists() {
        let (mut app, id) = app_with_styled_text("ab", lato());
        set_sel(&mut app, 2, 2, false);
        key(&mut app, egui::Key::B, false, true);
        insert_text(&mut app, "C", EditKind::Typing);
        assert_eq!(text_of(&app, id).runs.len(), 1);
        assert!(asked_for(&app, "TestLato", 700), "{:?}", app.fonts.requested());
    }

    #[test]
    fn typing_in_a_face_the_text_already_uses_requests_nothing() {
        let (mut app, id) = app_with_styled_text("ab", lato());
        set_sel(&mut app, 2, 2, false);
        insert_text(&mut app, "c", EditKind::Typing);
        insert_text(&mut app, "d", EditKind::Typing);
        assert_eq!(text_of(&app, id).content, "abcd");
        assert!(app.fonts.requested().is_empty(), "plain typing must not start a font load: {:?}", app.fonts.requested());
    }

    #[test]
    fn inspector_style_edits_request_the_new_face_with_or_without_a_session() {
        use crate::panels::typography::set_style;
        let (mut app, id) = app_with_text("hello world");
        set_sel(&mut app, 5, 0, false);
        set_style(&mut app, &[id], |s| s.font_family = "TestLato".into());
        assert!(asked_for(&app, "TestLato", 400), "session: {:?}", app.fonts.requested());

        let (mut app, id) = app_with_text("hello world");
        finish_text(&mut app, true);
        set_style(&mut app, &[id], |s| s.font_family = "TestLato".into());
        assert!(asked_for(&app, "TestLato", 400), "layer: {:?}", app.fonts.requested());
        set_style(&mut app, &[id], |s| s.font_weight = 700);
        assert!(asked_for(&app, "TestLato", 700), "layer weight: {:?}", app.fonts.requested());
    }

    fn wght_inter(weight: u16, axis: f32) -> TextStyle {
        let mut st = TextStyle { font_size: 32.0, font_weight: weight, ..Default::default() };
        st.set_axis("wght", axis);
        st
    }

    /// Ink coverage of the first page, drawn on white.
    fn ink_of(app: &mut App) -> u64 {
        let page = app.page;
        if let Some(n) = app.doc.get_mut(page) {
            n.kind = NodeKind::Page { background: designos_doc::Color::WHITE };
        }
        let mut pm = designos_render::tiny_skia::Pixmap::new(400, 64).unwrap();
        designos_render::render_page(&app.doc, page, &designos_render::Viewport::default(), &mut pm);
        pm.pixels().iter().map(|p| 255 - p.red() as u64).sum()
    }

    #[test]
    fn bold_moves_the_wght_axis_so_the_canvas_follows_the_panel() {
        let (mut app, id) = app_with_styled_text("Hamburgefonstiv", wght_inter(400, 400.0));
        let regular = ink_of(&mut app);
        select_all(&mut app);
        key(&mut app, egui::Key::B, false, true);
        let t = text_of(&app, id);
        assert_eq!(t.style.font_weight, 700, "the panel reads font_weight");
        assert_eq!(t.style.get_axis("wght"), Some(700.0), "the renderer reads the axis first");
        let bold = ink_of(&mut app);
        assert!(bold > regular + regular / 10, "Bold must draw heavier: {regular} vs {bold}");
        key(&mut app, egui::Key::B, false, true);
        assert_eq!(text_of(&app, id).style.get_axis("wght"), Some(400.0), "toggling back follows too");
    }

    #[test]
    fn size_and_spacing_edits_start_no_font_load() {
        use crate::panels::typography::set_style;
        let (mut app, id) = app_with_styled_text("hello world", lato());
        set_sel(&mut app, 5, 0, false);
        text_command(&mut app, TextCmd::SizeUp);
        set_style(&mut app, &[id], |s| s.letter_spacing = 1.0);
        assert!(app.fonts.requested().is_empty(), "session: {:?}", app.fonts.requested());

        let (mut app, id) = app_with_styled_text("hello world", lato());
        finish_text(&mut app, true);
        set_style(&mut app, &[id], |s| s.font_size = 20.0);
        text_command_on(&mut app, id, TextCmd::SizeDown);
        assert!(app.fonts.requested().is_empty(), "layer: {:?}", app.fonts.requested());
    }

    fn text_command_on(app: &mut App, id: NodeId, cmd: TextCmd) {
        app.selection = vec![id];
        text_command(app, cmd);
    }

    #[test]
    fn a_face_added_in_a_document_too_large_to_scan_is_still_requested() {
        // `requests_for` reads only the first 8,192 nodes of a document over 25,000 nodes, so the
        // edited text (the last one) is out of its reach: the edit has to name the face itself.
        let mut d = Document::new("big");
        let page = d.add_page("p");
        let mut last = None;
        for i in 0..25_100 {
            let id = d.alloc_id();
            let st = if i == 25_099 { lato() } else { TextStyle::default() };
            d.insert(page, Node::text(id, Rect::new(0.0, 0.0, 10.0, 10.0), "hello world", st)).unwrap();
            last = Some(id);
        }
        let id = last.unwrap();
        let mut app = App::new(d);
        begin_text_edit(&mut app, id);
        set_sel(&mut app, 5, 0, false);
        key(&mut app, egui::Key::B, false, true);
        assert!(asked_for(&app, "TestLato", 700), "{} faces requested", app.fonts.requested().len());
    }

    #[test]
    fn a_bold_typing_style_moves_the_wght_axis() {
        let (mut app, id) = app_with_styled_text("ab", wght_inter(400, 400.0));
        set_sel(&mut app, 2, 2, false);
        key(&mut app, egui::Key::B, false, true);
        insert_text(&mut app, "C", EditKind::Typing);
        let t = text_of(&app, id);
        assert_eq!((t.runs[0].start, t.runs[0].style.font_weight, t.runs[0].style.get_axis("wght")), (2, 700, Some(700.0)), "caret + Ctrl+B + typing");
    }

    /// The weight dropdown writes only `font_weight`.
    #[test]
    fn the_inspector_weight_moves_the_wght_axis_in_a_session() {
        use crate::panels::typography::set_style;
        let (mut app, id) = app_with_styled_text("hello", wght_inter(400, 400.0));
        select_all(&mut app);
        set_style(&mut app, &[id], |s| s.font_weight = 700);
        assert_eq!(text_of(&app, id).style.get_axis("wght"), Some(700.0));
    }

    #[test]
    fn the_inspector_weight_moves_the_wght_axis_on_a_selected_layer() {
        use crate::panels::typography::set_style;
        let (mut app, id) = app_with_styled_text("hello", wght_inter(400, 400.0));
        finish_text(&mut app, true);
        set_style(&mut app, &[id], |s| s.font_weight = 700);
        assert_eq!(text_of(&app, id).style.get_axis("wght"), Some(700.0));
    }

    fn family_text(family: &str) -> TextStyle {
        TextStyle { font_family: family.into(), font_size: 16.0, ..Default::default() }
    }

    /// 25,100 Inter text layers: past the 25,000 where `requests_for` reads only the first 8,192.
    /// Returns the app and the ids of the last `n` layers, which that scan never reaches.
    fn large_app(n: usize) -> (App, Vec<NodeId>) {
        let mut d = Document::new("big");
        let page = d.add_page("p");
        let mut ids = Vec::new();
        for _ in 0..25_100 {
            let id = d.alloc_id();
            d.insert(page, Node::text(id, Rect::new(0.0, 0.0, 10.0, 10.0), "hello world", TextStyle::default())).unwrap();
            ids.push(id);
        }
        let last = ids.split_off(ids.len() - n);
        (App::new(d), last)
    }

    #[test]
    fn ctrl_b_fetches_the_bold_face_after_the_regular_face_of_its_family_is_registered() {
        use crate::fonts::testkit::{serving, settle};
        let (mut app, _) = app_with_styled_text("hello world", family_text("TestLatoOne"));
        app.fonts = serving(&["TestLatoOne"]);
        app.fonts.load_for(&app.doc);
        assert_eq!(settle(&mut app.fonts).loaded, 1, "the open load fetches the regular face");
        set_sel(&mut app, 5, 0, false);
        key(&mut app, egui::Key::B, false, true);
        let done = settle(&mut app.fonts);
        assert!(done.failed.is_empty(), "{:?}", done.failed);
        assert_eq!(done.loaded, 1, "the 700 face is fetched although the regular one is already registered");
        assert!(app.fonts.registered().contains(&designos_fonts::FontRequest::new("TestLatoOne", 700, false)));
    }

    #[test]
    fn a_family_installed_on_the_system_is_never_fetched_at_any_weight() {
        use crate::fonts::testkit::{font_named, serving, settle};
        designos_render::register_font(font_named("TestSysFamy")); // stands in for a system font
        let (mut app, _) = app_with_styled_text("hello world", family_text("TestSysFamy"));
        app.fonts = serving(&[]); // a download attempt would fail
        set_sel(&mut app, 5, 0, false);
        key(&mut app, egui::Key::B, false, true);
        assert_eq!(app.fonts.started(), 0, "Ctrl+B on a system family starts no load");
        app.fonts.load_for(&app.doc); // a document load names the bold face too
        let done = settle(&mut app.fonts);
        assert_eq!((done.loaded, done.failed.is_empty()), (0, true), "{:?}", done.failed);
    }

    #[test]
    fn the_family_picker_keeps_its_face_on_a_large_document() {
        use crate::fonts::testkit::{serving, settle};
        let (mut app, ids) = large_app(1);
        app.fonts = serving(&["TestLatoTwo"]);
        crate::panels::typography::pick_family(&mut app, &ids, "TestLatoTwo");
        assert!(asked_for(&app, "TestLatoTwo", 400), "{:?}", app.fonts.requested());
        settle(&mut app.fonts);
        assert!(app.fonts.registered().contains(&designos_fonts::FontRequest::new("TestLatoTwo", 400, false)));
    }

    #[test]
    fn back_to_back_edits_on_a_large_document_each_keep_their_face() {
        use crate::fonts::testkit::{serving, settle};
        use crate::panels::typography::set_style;
        let (mut app, ids) = large_app(2);
        app.fonts = serving(&["TestLatoTre", "TestLatoFor"]);
        set_style(&mut app, &ids[..1], |s| s.font_family = "TestLatoTre".into());
        set_style(&mut app, &ids[1..], |s| s.font_family = "TestLatoFor".into());
        settle(&mut app.fonts);
        for family in ["TestLatoTre", "TestLatoFor"] {
            assert!(app.fonts.registered().contains(&designos_fonts::FontRequest::new(family, 400, false)), "{family}: {:?}", app.fonts.registered());
        }
    }

    #[test]
    fn undoing_a_second_quick_style_edit_finds_the_face_of_the_first() {
        use crate::fonts::testkit::{serving, settle};
        let (mut app, id) = app_with_styled_text("hello world", family_text("TestLatoFiv"));
        app.fonts = serving(&["TestLatoFiv"]);
        set_sel(&mut app, 5, 0, false);
        key(&mut app, egui::Key::B, false, true);
        key(&mut app, egui::Key::I, false, true); // before the first fetch returns
        key(&mut app, egui::Key::Z, false, true);
        let run = text_of(&app, id).runs[0].style.clone();
        assert_eq!((run.font_weight, run.italic), (700, false), "undo brings the upright bold run back");
        settle(&mut app.fonts);
        assert!(app.fonts.registered().contains(&designos_fonts::FontRequest::new("TestLatoFiv", 700, false)), "{:?}", app.fonts.registered());
    }

    #[test]
    fn stepping_the_wght_field_costs_one_load_per_face() {
        use crate::panels::typography::set_style;
        let (mut app, id) = app_with_styled_text("hello", family_text("TestLatoSix"));
        finish_text(&mut app, true);
        // what the Variable tab writes for one wheel step
        let step = |app: &mut App, w: u16| {
            set_style(app, &[id], |s| {
                s.set_axis("wght", f32::from(w));
                s.font_weight = w;
            })
        };
        step(&mut app, 401);
        assert!(asked_for(&app, "TestLatoSix", 500) && !asked_for(&app, "TestLatoSix", 401), "401 is drawn with the 500 face: {:?}", app.fonts.requested());
        assert_eq!(app.fonts.started(), 1);
        step(&mut app, 402);
        step(&mut app, 450);
        assert_eq!(app.fonts.started(), 1, "every weight up to 500 is the same face");
        step(&mut app, 520);
        assert!(asked_for(&app, "TestLatoSix", 600), "{:?}", app.fonts.requested());
        assert_eq!(app.fonts.started(), 2);
    }

    /// Two texts: the first in `first`, the second in `other_family`.
    fn app_with_two_texts(first: TextStyle, other_family: &str) -> (App, NodeId, NodeId) {
        let (mut app, id) = app_with_styled_text("hello world", first);
        let other = app.doc.alloc_id();
        let page = app.page;
        app.doc.insert(page, Node::text(other, Rect::new(0.0, 100.0, 10.0, 10.0), "brand", family_text(other_family))).unwrap();
        (app, id, other)
    }

    #[test]
    fn bold_on_bundled_inter_starts_no_load_while_another_face_has_failed() {
        use crate::fonts::testkit::{serving, settle};
        let (mut app, _, _) = app_with_two_texts(TextStyle { font_size: 16.0, ..Default::default() }, "TestBrandOne");
        app.fonts = serving(&[]); // nothing is served: the brand family fails
        app.fonts.load_for(&app.doc);
        assert_eq!(settle(&mut app.fonts).failed.len(), 1, "reported once, as on open");
        let started = app.fonts.started();
        set_sel(&mut app, 5, 0, false);
        key(&mut app, egui::Key::B, false, true);
        assert_eq!(app.fonts.started(), started, "Inter's bold face is bundled: nothing to fetch");
        assert!(!app.fonts.is_loading());
    }

    #[test]
    fn an_edit_that_adds_a_face_does_not_refetch_or_report_a_failed_face_of_another_text() {
        use crate::fonts::testkit::{serving, settle};
        let (mut app, _, _) = app_with_two_texts(family_text("TestLatoEit"), "TestBrandTwo");
        app.fonts = serving(&["TestLatoEit"]);
        app.fonts.load_for(&app.doc);
        let open = settle(&mut app.fonts);
        assert_eq!((open.loaded, open.failed.len()), (1, 1));
        set_sel(&mut app, 5, 0, false);
        key(&mut app, egui::Key::B, false, true);
        let done = settle(&mut app.fonts);
        assert_eq!(done.loaded, 1, "the bold face of the text the user touched is fetched");
        assert!(done.failed.is_empty(), "the other text's failure is not toasted again: {:?}", done.failed);
    }

    #[test]
    fn toggling_bold_back_on_needs_no_second_load() {
        use crate::fonts::testkit::{serving, settle};
        let (mut app, _) = app_with_styled_text("hello world", family_text("TestLatoSev"));
        app.fonts = serving(&["TestLatoSev"]);
        set_sel(&mut app, 5, 0, false);
        key(&mut app, egui::Key::B, false, true);
        settle(&mut app.fonts);
        let started = app.fonts.started();
        key(&mut app, egui::Key::B, false, true); // off
        key(&mut app, egui::Key::B, false, true); // on again: the 700 face is registered
        assert_eq!(app.fonts.started(), started);
    }

    #[test]
    fn a_failed_face_is_reported_once_per_session_but_retried_when_the_document_reloads() {
        use crate::fonts::testkit::{serving, settle};
        let (mut app, _, _) = app_with_two_texts(TextStyle { font_size: 16.0, ..Default::default() }, "TestBrandThr");
        app.fonts = serving(&[]);
        app.fonts.load_for(&app.doc);
        assert_eq!(settle(&mut app.fonts).failed.len(), 1);
        let started = app.fonts.started();
        app.fonts.load_for(&app.doc); // a tab switch, an open
        assert_eq!(app.fonts.started(), started + 1, "the face is fetched again, in case the network is back");
        assert!(settle(&mut app.fonts).failed.is_empty(), "but its failure is not toasted again");
    }

    #[test]
    fn picking_the_family_again_retries_a_failed_face_and_reports_it_again() {
        use crate::fonts::testkit::{serving, settle};
        let (mut app, _, other) = app_with_two_texts(TextStyle { font_size: 16.0, ..Default::default() }, "TestBrandFor");
        app.fonts = serving(&[]);
        app.fonts.load_for(&app.doc);
        assert_eq!(settle(&mut app.fonts).failed.len(), 1);
        crate::panels::typography::pick_family(&mut app, &[other], "TestBrandFor");
        assert_eq!(settle(&mut app.fonts).failed.len(), 1, "an explicit retry says so when it fails again");
    }

    #[test]
    fn picking_the_family_again_reaches_a_failed_face_on_a_large_document() {
        use crate::fonts::testkit::{serving, settle};
        let (mut app, ids) = large_app(1);
        if let Some(NodeKind::Text(t)) = app.doc.get_mut(ids[0]).map(|n| &mut n.kind) {
            t.style.font_family = "TestBrandFiv".into();
        }
        app.fonts = serving(&[]);
        crate::panels::typography::pick_family(&mut app, &ids, "TestBrandFiv");
        assert_eq!(settle(&mut app.fonts).failed.len(), 1, "the retry names the face, the capped scan would not");
        crate::panels::typography::pick_family(&mut app, &ids, "TestBrandFiv");
        assert_eq!(settle(&mut app.fonts).failed.len(), 1, "and again on the next pick");
    }

    #[test]
    fn bold_leaves_text_without_a_wght_axis_free_of_axes() {
        let (mut app, id) = app_with_text("hello");
        select_all(&mut app);
        key(&mut app, egui::Key::B, false, true);
        assert!(text_of(&app, id).style.variation_axes.is_empty());
    }

    #[test]
    fn caret_style_applies_to_next_typed_text_only() {
        let (mut app, id) = app_with_text("ab");
        set_sel(&mut app, 2, 2, false);
        text_command(&mut app, TextCmd::Bold);
        insert_text(&mut app, "C", EditKind::Typing);
        insert_text(&mut app, "d", EditKind::Typing);
        let t = text_of(&app, id);
        assert_eq!(t.content, "abCd");
        assert_eq!(t.runs.len(), 1);
        assert_eq!((t.runs[0].start, t.runs[0].end), (2, 4), "bold continues from the typed char");
    }

    #[test]
    fn size_shortcuts_change_by_one() {
        let (mut app, id) = app_with_text("abc");
        text_command(&mut app, TextCmd::SizeUp);
        assert_eq!(text_of(&app, id).style.font_size, 17.0);
        set_sel(&mut app, 1, 0, false);
        text_command(&mut app, TextCmd::SizeDown);
        let t = text_of(&app, id);
        assert_eq!((t.runs[0].start, t.runs[0].end, t.runs[0].style.font_size), (0, 1, 16.0));
    }

    #[test]
    fn undo_steps_follow_words_and_redo() {
        let (mut app, id) = app_with_text("");
        app.now = 0.0;
        for (i, w) in ["he", "llo", " ", "wor", "ld"].iter().enumerate() {
            app.now = 0.1 * i as f64;
            insert_text(&mut app, w, EditKind::Typing);
        }
        assert_eq!(text_of(&app, id).content, "hello world");
        undo_step(&mut app, false);
        assert_eq!(text_of(&app, id).content, "hello ", "the last word goes first");
        undo_step(&mut app, false);
        assert_eq!(text_of(&app, id).content, "");
        undo_step(&mut app, true);
        assert_eq!(text_of(&app, id).content, "hello ");
        undo_step(&mut app, true);
        assert_eq!(text_of(&app, id).content, "hello world");
    }

    fn kinds(app: &App, id: NodeId) -> Vec<Option<(ListKind, u8)>> {
        text_of(app, id).paragraph_lists().iter().map(|i| i.map(|i| (i.kind, i.level))).collect()
    }

    #[test]
    fn enter_continues_lists_and_toggles_work() {
        let (mut app, id) = app_with_text("one");
        set_sel(&mut app, 3, 3, false);
        text_command(&mut app, TextCmd::List(ListKind::Bullet));
        assert_eq!(text_of(&app, id).content, "one", "the text itself is untouched");
        assert_eq!(kinds(&app, id), vec![Some((ListKind::Bullet, 0))]);
        key(&mut app, egui::Key::Enter, false, false);
        assert_eq!(text_of(&app, id).content, "one\n");
        assert_eq!(kinds(&app, id), vec![Some((ListKind::Bullet, 0)); 2], "Enter continues the list");
        insert_text(&mut app, "two", EditKind::Typing);
        key(&mut app, egui::Key::Enter, false, false);
        assert_eq!(kinds(&app, id).len(), 3);
        key(&mut app, egui::Key::Enter, false, false);
        let t = text_of(&app, id);
        assert_eq!(t.content, "one\ntwo\n", "Enter on an empty item adds no paragraph");
        assert_eq!(kinds(&app, id), vec![Some((ListKind::Bullet, 0)), Some((ListKind::Bullet, 0)), None], "and ends the list");
        // toggling off
        set_sel(&mut app, 0, 0, false);
        text_command(&mut app, TextCmd::List(ListKind::Bullet));
        assert_eq!(kinds(&app, id), vec![None, Some((ListKind::Bullet, 0)), None]);
    }

    #[test]
    fn shortcuts_toggle_and_tab_nests() {
        let (mut app, id) = app_with_text("a\nb\nc");
        key(&mut app, egui::Key::A, false, true);
        key(&mut app, egui::Key::Num8, true, true);
        assert_eq!(kinds(&app, id), vec![Some((ListKind::Bullet, 0)); 3]);
        key(&mut app, egui::Key::Num7, true, true);
        assert_eq!(kinds(&app, id), vec![Some((ListKind::Numbered, 0)); 3], "ctrl+shift+7 switches to numbers");
        set_sel(&mut app, 2, 2, false);
        key(&mut app, egui::Key::Tab, false, false);
        key(&mut app, egui::Key::Tab, false, false);
        assert_eq!(kinds(&app, id)[1], Some((ListKind::Numbered, 2)));
        key(&mut app, egui::Key::Tab, true, false);
        assert_eq!(kinds(&app, id)[1], Some((ListKind::Numbered, 1)));
        assert_eq!(kinds(&app, id)[0], Some((ListKind::Numbered, 0)), "only the caret's item moved");
        key(&mut app, egui::Key::Num7, true, true);
        assert_eq!(kinds(&app, id)[1], None, "toggling the caret's item off leaves the others");
        assert_eq!(kinds(&app, id)[2], Some((ListKind::Numbered, 0)));
    }

    #[test]
    fn backspace_at_item_start_unlists() {
        let (mut app, id) = app_with_text("ab");
        text_command(&mut app, TextCmd::List(ListKind::Numbered));
        set_sel(&mut app, 0, 0, false);
        key(&mut app, egui::Key::Backspace, false, false);
        assert_eq!(text_of(&app, id).content, "ab");
        assert!(text_of(&app, id).lists.is_empty());
        // mid text it deletes as usual
        set_sel(&mut app, 2, 2, false);
        key(&mut app, egui::Key::Backspace, false, false);
        assert_eq!(text_of(&app, id).content, "a");
    }

    #[test]
    fn arrows_follow_the_screen_in_right_to_left_text() {
        // three Hebrew letters: the start of the text is at the right edge of the line
        let (mut app, _) = app_with_text("\u{5D0}\u{5D1}\u{5D2}");
        set_sel(&mut app, 0, 0, false);
        let caret = |app: &App| app.canvas.text.as_ref().unwrap().caret;
        key(&mut app, egui::Key::ArrowLeft, false, false);
        assert_eq!(caret(&app), 2, "Left walks forward through the text in a right-to-left run");
        key(&mut app, egui::Key::ArrowLeft, false, false);
        key(&mut app, egui::Key::ArrowLeft, false, false);
        assert_eq!(caret(&app), 6, "at the left end of the line");
        key(&mut app, egui::Key::ArrowLeft, false, false);
        assert_eq!(caret(&app), 6, "nothing further left");
        key(&mut app, egui::Key::ArrowRight, false, false);
        assert_eq!(caret(&app), 4);
        // shift+Right extends the selection back through the text
        key(&mut app, egui::Key::ArrowRight, true, false);
        let s = app.canvas.text.as_ref().unwrap();
        assert_eq!((s.caret, s.anchor), (2, 4));
        // plain Left collapses to the visually left edge of the selection (the later offset)
        key(&mut app, egui::Key::ArrowLeft, false, false);
        assert_eq!(caret(&app), 4);
    }

    #[test]
    fn word_jumps_and_end_of_line_in_right_to_left_text() {
        let (mut app, _) = app_with_text("\u{5D0}\u{5D1} \u{5D2}\u{5D3}");
        set_sel(&mut app, 0, 0, false);
        key(&mut app, egui::Key::ArrowLeft, false, true);
        assert_eq!(caret_of(&app), 4, "ctrl+Left jumps to the next word in text order: it is to the left on screen");
        key(&mut app, egui::Key::ArrowRight, false, true);
        assert_eq!(caret_of(&app), 0);
        // Left at the left end of a right-to-left line continues into the next paragraph
        let (mut app, _) = app_with_text("\u{5D0}\n\u{5D1}");
        set_sel(&mut app, 2, 2, false);
        key(&mut app, egui::Key::ArrowLeft, false, false);
        assert_eq!(caret_of(&app), 3, "the end of the first line is its left end; Left goes on to the next line");
    }

    fn caret_of(app: &App) -> usize {
        app.canvas.text.as_ref().unwrap().caret
    }

    #[test]
    fn typing_right_to_left_text_keeps_the_caret_after_the_new_letters() {
        let (mut app, id) = app_with_text("");
        for ch in ["\u{5D0}", "\u{5D1}", "\u{5D2}"] {
            insert_text(&mut app, ch, EditKind::Typing);
        }
        assert_eq!(text_of(&app, id).content, "\u{5D0}\u{5D1}\u{5D2}");
        assert_eq!(caret_of(&app), 6);
        let t = text_of(&app, id);
        let size = app.doc.get(id).unwrap().size;
        let (x, _, _) = tm::caret_rect_hint(&t, size, 6, app.canvas.text.as_ref().unwrap().hint_x);
        let g = tm::geometry(&t, size);
        let leftmost = g.lines[0].glyphs.iter().map(|g| g.x).fold(f32::MAX, f32::min);
        assert!((x - leftmost).abs() < 0.01, "the caret is left of the last typed letter");
        // Backspace removes the letter before the caret in text order
        key(&mut app, egui::Key::Backspace, false, false);
        assert_eq!(text_of(&app, id).content, "\u{5D0}\u{5D1}");
    }

    #[test]
    fn ime_composition_is_previewed_then_committed() {
        let (mut app, id) = app_with_text("ab");
        set_sel(&mut app, 2, 2, false);
        preedit(&mut app, "に");
        assert_eq!(text_of(&app, id).content, "abに");
        preedit(&mut app, "にほ");
        assert_eq!(text_of(&app, id).content, "abにほ");
        commit_ime(&mut app, "日本");
        let t = text_of(&app, id);
        assert_eq!(t.content, "ab日本");
        assert!(t.runs.is_empty(), "no leftover underline style: {:?}", t.runs);
        assert!(app.canvas.text.as_ref().unwrap().preedit.is_none());
        assert_eq!(app.canvas.text.as_ref().unwrap().caret, "ab日本".len());
    }

    #[test]
    fn finish_commits_one_document_undo_step() {
        let (mut app, id) = app_with_text("x");
        insert_text(&mut app, "abc", EditKind::Typing);
        finish_text(&mut app, true);
        assert_eq!(text_of(&app, id).content, "abc");
        app.undo();
        assert_eq!(text_of(&app, id).content, "x");
    }
}
