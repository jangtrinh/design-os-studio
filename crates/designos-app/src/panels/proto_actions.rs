//! The action list of the interaction editor: every action with its parameters, including the
//! variable actions (Set variable, Set variable mode), Conditional branches ("If / Else if /
//! Else", each holding a nested action list) and video controls.

use super::properties::Pane;
use super::prototype::{action_params, dest_dropdown, label_ctl, next_row, variants_of};
use crate::app::App;
use crate::icons::{self, Icon};
use crate::proto;
use crate::theme;
use crate::widgets::field::{self, Prefix};
use crate::widgets::menu::dropdown;
use crate::widgets::{self, record};
use designos_doc::*;
use egui::{Align2, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

/// Indent of a nested action list, in px.
const INDENT: f32 = 12.0;
/// Conditionals may nest this deep in the editor (the player allows more).
const MAX_UI_DEPTH: usize = 2;

/// What the editor needs to know about the layer being edited.
#[derive(Clone, Copy)]
pub(super) struct EditCtx<'a> {
    pub node: NodeId,
    pub frames: &'a [NodeId],
    pub own_top: Option<NodeId>,
    /// Nesting level of the list being edited (0 = the interaction's own actions).
    pub depth: usize,
    /// Every action of the list can be removed (nested lists), otherwise the last one stays.
    pub allow_empty: bool,
}

impl EditCtx<'_> {
    fn fallback_dest(&self) -> NodeId {
        self.frames.iter().copied().find(|f| Some(*f) != self.own_top).or(self.own_top).unwrap_or(self.node)
    }

    /// The context for a list nested inside a conditional branch.
    fn nested(self) -> Self {
        EditCtx { depth: self.depth + 1, allow_empty: true, ..self }
    }
}

/// Edits `actions` in place. `key` is the path of this list (unique per list for widget ids and
/// recorded rects: top-level actions are "0", "1"...; nested ones "1.0.2").
#[allow(clippy::needless_range_loop)]
pub(super) fn actions_editor(app: &mut App, ui: &mut Ui, pane: &mut Pane, cx: EditCtx<'_>, actions: &mut Vec<Action>, key: &str) {
    let (depth, allow_empty) = (cx.depth, cx.allow_empty);
    let n = actions.len();
    let mut remove = None;
    for ai in 0..n {
        let k = if key.is_empty() { format!("{ai}") } else { format!("{key}.{ai}") };
        // A short label (the names are long); nested lists drop it and use the full width.
        let row = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, theme::FIELD_HEIGHT);
        let ac = if depth == 0 {
            ui.painter().text(Pos2::new(row.left(), row.center().y), Align2::LEFT_CENTER, "Action", theme::font_body(), theme::TEXT_SECONDARY);
            Rect::from_min_max(Pos2::new(row.left() + 52.0, row.top()), row.max)
        } else {
            row
        };
        record(ui, &format!("proto-pop-action-{k}"), ac);
        // The menu hides "Conditional" (index 10) at the deepest level, so later entries shift by one there.
        let hide_cond = depth >= MAX_UI_DEPTH;
        let anames: Vec<String> = proto::ACTIONS.iter().enumerate().filter(|(i, _)| *i != 10 || !hide_cond).map(|(_, s)| s.to_string()).collect();
        let cur_i = proto::action_index(&actions[ai]);
        let removable = n > 1 || allow_empty;
        let dd = if removable { Rect::from_min_max(ac.min, Pos2::new(ac.right() - 28.0, ac.bottom())) } else { ac };
        if let Some(picked) = dropdown(ui, dd, ("pop-action", k.as_str()), proto::ACTIONS[cur_i], &anames) {
            let i = if hide_cond && picked >= 10 { picked + 1 } else { picked };
            if i != cur_i {
                let fb = if i == 7 { variants_of(&app.doc, cx.node).first().copied().unwrap_or(cx.fallback_dest()) } else { cx.fallback_dest() };
                actions[ai] = proto::action_of_index(&app.doc, i, &actions[ai], fb);
            }
        }
        if removable {
            let b = Rect::from_min_size(Pos2::new(ac.right() - 24.0, ac.top()), Vec2::splat(24.0));
            record(ui, &format!("proto-pop-rm-action-{k}"), b);
            if widgets::icon_button(ui, b, ("pop-rm-action", k.as_str()), Icon::Minus, false, "Remove action").clicked() {
                remove = Some(ai);
            }
        }
        next_row(pane);
        match &mut actions[ai] {
            Action::SetVariable { variable, value } => set_variable_params(app, ui, pane, &k, variable, value),
            Action::SetVariableMode { collection, mode } => set_mode_params(app, ui, pane, &k, collection, mode),
            Action::MediaControl { dest, op } => media_params(app, ui, pane, &k, dest, op),
            Action::Conditional { branches, otherwise } => conditional_params(app, ui, pane, cx, &k, branches, otherwise),
            a => action_params(app, ui, pane, cx.node, cx.frames, cx.own_top, &k, a),
        }
    }
    if let Some(ai) = remove {
        actions.remove(ai);
    }
}

/// "+ Add ..." row; true when clicked.
fn add_row(ui: &mut Ui, pane: &mut Pane, rec: &str, label: &str, indent_icon: f32) -> bool {
    let r = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, theme::FIELD_HEIGHT);
    record(ui, rec, r);
    let resp = ui.interact(r, ui.id().with(("add-row", rec)), Sense::click());
    let col = if resp.hovered() { theme::ICON } else { theme::ICON_SECONDARY };
    icons::paint(ui.painter(), Icon::Plus, Pos2::new(r.left() + 8.0 + indent_icon, r.center().y), 12.0, col);
    ui.painter().text(Pos2::new(r.left() + 22.0 + indent_icon, r.center().y), Align2::LEFT_CENTER, label, theme::font_body(), theme::TEXT_SECONDARY);
    pane.y += theme::FIELD_HEIGHT + 4.0;
    resp.clicked()
}

fn sub_pane(pane: &Pane) -> Pane {
    Pane { x0: pane.x0 + INDENT, oy: pane.oy, y: pane.y, title_end: pane.title_end }
}

fn conditional_params(
    app: &mut App,
    ui: &mut Ui,
    pane: &mut Pane,
    cx: EditCtx<'_>,
    k: &str,
    branches: &mut Vec<(Expression, Vec<Action>)>,
    otherwise: &mut Vec<Action>,
) {
    let mut remove_branch = None;
    for bi in 0..branches.len() {
        let bk = format!("{k}.if{bi}");
        // "If" / "Else if" header with the remove button.
        let head = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, theme::FIELD_HEIGHT);
        let title = if bi == 0 { "If" } else { "Else if" };
        ui.painter().text(Pos2::new(head.left(), head.center().y), Align2::LEFT_CENTER, title, theme::font_body_strong(), theme::TEXT);
        if branches.len() > 1 || !otherwise.is_empty() || bi > 0 {
            let b = Rect::from_min_size(Pos2::new(head.right() - 24.0, head.top()), Vec2::splat(24.0));
            record(ui, &format!("proto-pop-rm-branch-{bk}"), b);
            if widgets::icon_button(ui, b, ("pop-rm-branch", bk.as_str()), Icon::Minus, false, "Remove condition").clicked() {
                remove_branch = Some(bi);
            }
        }
        next_row(pane);
        let (cond, acts) = &mut branches[bi];
        let text = proto::expression_text(&app.doc, cond);
        let res = expression_field(app, ui, pane, &format!("{bk}.cond"), &text, Expect::Condition);
        if let Some(e) = res {
            *cond = e;
        }
        let mut sub = sub_pane(pane);
        actions_editor(app, ui, &mut sub, cx.nested(), acts, &bk);
        pane.y = sub.y;
        if add_row_indented(ui, pane, &format!("proto-pop-add-then-{bk}"), "Add action") {
            acts.push(proto::action_of_index(&app.doc, 0, &Action::Back, cx.fallback_dest()));
        }
    }
    if let Some(bi) = remove_branch {
        branches.remove(bi);
    }
    if add_row(ui, pane, &format!("proto-pop-add-condition-{k}"), "Add condition", 0.0) {
        let cond = default_condition(&app.doc);
        branches.push((cond, vec![proto::action_of_index(&app.doc, 0, &Action::Back, cx.fallback_dest())]));
    }
    // Else.
    if otherwise.is_empty() {
        if add_row(ui, pane, &format!("proto-pop-add-else-{k}"), "Else", 0.0) {
            otherwise.push(proto::action_of_index(&app.doc, 0, &Action::Back, cx.fallback_dest()));
        }
    } else {
        let head = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, theme::FIELD_HEIGHT);
        record(ui, &format!("proto-pop-else-{k}"), head);
        ui.painter().text(Pos2::new(head.left(), head.center().y), Align2::LEFT_CENTER, "Else", theme::font_body_strong(), theme::TEXT);
        next_row(pane);
        let mut sub = sub_pane(pane);
        actions_editor(app, ui, &mut sub, cx.nested(), otherwise, &format!("{k}.else"));
        pane.y = sub.y;
        if !otherwise.is_empty() && add_row_indented(ui, pane, &format!("proto-pop-add-else-action-{k}"), "Add action") {
            otherwise.push(proto::action_of_index(&app.doc, 0, &Action::Back, cx.fallback_dest()));
        }
    }
}

fn add_row_indented(ui: &mut Ui, pane: &mut Pane, rec: &str, label: &str) -> bool {
    add_row(ui, pane, rec, label, INDENT)
}

/// A starting condition: "<first boolean variable>" else "<first number> > 0", else "true".
fn default_condition(doc: &Document) -> Expression {
    let all = || doc.variable_collections.values().flat_map(|c| c.variables.iter()).filter_map(|id| doc.variables.get(id));
    if let Some(v) = all().find(|v| v.kind == VariableKind::Boolean) {
        return Expression::var(v.id.clone());
    }
    if let Some(v) = all().find(|v| v.kind == VariableKind::Number) {
        return Expression::binary(BinaryOp::Gt, Expression::var(v.id.clone()), Expression::number(0.0));
    }
    Expression::boolean(true)
}

// ------------------------------------------------------------------ variables

/// "Collection / name" labels of every variable, in collection order, with their ids.
fn variable_options(doc: &Document) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for c in doc.variable_collections.values() {
        for id in &c.variables {
            if let Some(v) = doc.variables.get(id) {
                out.push((id.clone(), format!("{} / {}", c.name, v.name)));
            }
        }
    }
    out
}

fn set_variable_params(app: &mut App, ui: &mut Ui, pane: &mut Pane, k: &str, variable: &mut String, value: &mut Expression) {
    let opts = variable_options(&app.doc);
    let c = label_ctl(ui, pane, "Variable");
    record(ui, &format!("proto-pop-var-{k}"), c);
    if opts.is_empty() {
        ui.painter().text(Pos2::new(c.left() + 8.0, c.center().y), Align2::LEFT_CENTER, "No variables yet", theme::font_body(), theme::TEXT_TERTIARY);
    } else {
        let names: Vec<String> = opts.iter().map(|(_, n)| n.clone()).collect();
        let label = opts.iter().find(|(id, _)| id == variable).map(|(_, n)| n.clone()).unwrap_or_else(|| "Choose variable".into());
        if let Some(i) = dropdown(ui, c, ("pop-var", k), &label, &names) {
            let id = opts[i].0.clone();
            if &id != variable {
                *variable = id.clone();
                // a value of the old type rarely fits: start from a sensible one for the new variable
                if let Some(v) = app.doc.variables.get(&id) {
                    *value = proto::default_value_for(v.kind, v, &app.doc);
                }
            }
        }
    }
    next_row(pane);
    let kind = app.doc.variables.get(variable.as_str()).map(|v| v.kind);
    let text = proto::expression_text(&app.doc, value);
    let label_r = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, 16.0);
    ui.painter().text(Pos2::new(label_r.left(), label_r.center().y), Align2::LEFT_CENTER, "Value", theme::font_body(), theme::TEXT_SECONDARY);
    pane.y += 18.0;
    if let Some(e) = expression_field(app, ui, pane, &format!("{k}.value"), &text, Expect::Value(kind)) {
        *value = e;
    }
    insert_variable_picker(app, ui, pane, k, value, kind);
}

/// "Insert variable" dropdown under an expression field: appends a reference to the text.
fn insert_variable_picker(app: &mut App, ui: &mut Ui, pane: &mut Pane, k: &str, value: &mut Expression, kind: Option<VariableKind>) {
    let opts = variable_options(&app.doc);
    if opts.is_empty() {
        return;
    }
    let c = pane.full(0.0);
    record(ui, &format!("proto-pop-insert-{k}"), c);
    let names: Vec<String> = opts.iter().map(|(_, n)| n.clone()).collect();
    if let Some(i) = dropdown(ui, c, ("pop-insert", k), "Insert variable", &names) {
        let name = app.doc.variables.get(&opts[i].0).map(|v| v.name.clone()).unwrap_or_default();
        let reference = Expression::var(opts[i].0.clone()).to_source(&|_| Some(name.clone()));
        let cur = proto::expression_text(&app.doc, value);
        let joined = if cur.trim().is_empty() { reference } else { format!("{cur} {reference}") };
        let (e, _) = check(&app.doc, &joined, kind.map(Expect::value_of).unwrap_or(Expect::Any));
        match e {
            Some(e) => *value = e,
            None => {
                // not valid yet (needs an operator): keep it as a draft the user can finish
                ui.data_mut(|d| d.insert_temp(draft_id(ui, &format!("{k}.value")), joined));
            }
        }
    }
    next_row(pane);
}

fn set_mode_params(app: &mut App, ui: &mut Ui, pane: &mut Pane, k: &str, collection: &mut String, mode: &mut String) {
    let colls: Vec<(String, String)> = app.doc.variable_collections.values().map(|c| (c.id.clone(), c.name.clone())).collect();
    let c = label_ctl(ui, pane, "Collection");
    record(ui, &format!("proto-pop-coll-{k}"), c);
    if colls.is_empty() {
        ui.painter().text(Pos2::new(c.left() + 8.0, c.center().y), Align2::LEFT_CENTER, "No collections yet", theme::font_body(), theme::TEXT_TERTIARY);
        next_row(pane);
        return;
    }
    let names: Vec<String> = colls.iter().map(|(_, n)| n.clone()).collect();
    let label = colls.iter().find(|(id, _)| id == collection).map(|(_, n)| n.clone()).unwrap_or_else(|| "Choose collection".into());
    if let Some(i) = dropdown(ui, c, ("pop-coll", k), &label, &names) {
        if &colls[i].0 != collection {
            *collection = colls[i].0.clone();
            *mode = app.doc.variable_collections.get(collection.as_str()).map(|c| c.default_mode.clone()).unwrap_or_default();
        }
    }
    next_row(pane);
    let modes: Vec<(String, String)> =
        app.doc.variable_collections.get(collection.as_str()).map(|c| c.modes.iter().map(|m| (m.id.clone(), m.name.clone())).collect()).unwrap_or_default();
    let c = label_ctl(ui, pane, "Mode");
    record(ui, &format!("proto-pop-mode-{k}"), c);
    let mnames: Vec<String> = modes.iter().map(|(_, n)| n.clone()).collect();
    let mlabel = modes.iter().find(|(id, _)| id == mode).map(|(_, n)| n.clone()).unwrap_or_else(|| "Choose mode".into());
    if let Some(i) = dropdown(ui, c, ("pop-mode", k), &mlabel, &mnames) {
        *mode = modes[i].0.clone();
    }
    next_row(pane);
}

fn media_params(app: &mut App, ui: &mut Ui, pane: &mut Pane, k: &str, dest: &mut NodeId, op: &mut MediaOp) {
    let videos = proto::video_layers(&app.doc);
    if videos.is_empty() {
        let r = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, 28.0);
        ui.painter().text(Pos2::new(r.left(), r.top() + 6.0), Align2::LEFT_CENTER, "No video layers in this file", theme::font_caption(), theme::TEXT_TERTIARY);
        pane.y += 24.0;
    } else if let Some(d) = dest_dropdown(ui, pane, &app.doc, &format!("media-{k}"), "Video", *dest, &videos) {
        *dest = d;
    }
    let c = label_ctl(ui, pane, "Control");
    record(ui, &format!("proto-pop-media-op-{k}"), c);
    let names: Vec<String> = proto::MEDIA_OPS.iter().map(|s| s.to_string()).collect();
    let cur = proto::media_op_index(op);
    if let Some(i) = dropdown(ui, c, ("pop-media-op", k), proto::MEDIA_OPS[cur], &names) {
        if i != cur {
            *op = proto::media_op_of_index(i, op);
        }
    }
    next_row(pane);
    let label = if matches!(op, MediaOp::SeekTo(_)) { "Time" } else { "Seconds" };
    if let MediaOp::SkipBy(s) | MediaOp::SeekTo(s) = op {
        let c = label_ctl(ui, pane, label);
        record(ui, &format!("proto-pop-media-secs-{k}"), c);
        if let Some(v) = field::num_field(ui, c, ("pop-media-secs", k), Prefix::None, Some(*s), " s") {
            *s = v;
        }
        next_row(pane);
    }
}

// ----------------------------------------------------------- expression field

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Expect {
    /// Any expression.
    Any,
    /// The value for a variable of this kind (`None`: unknown variable).
    Value(Option<VariableKind>),
    /// A boolean condition.
    Condition,
}

impl Expect {
    fn value_of(k: VariableKind) -> Expect {
        Expect::Value(Some(k))
    }
}

/// Parses `text` for the field: the expression when valid, plus a message (error, or a preview of
/// the result against the document's current values).
pub(super) fn check(doc: &Document, text: &str, expect: Expect) -> (Option<Expression>, Result<String, String>) {
    let parsed = match expect {
        Expect::Condition => proto::parse_condition_text(doc, text),
        Expect::Value(k) => proto::parse_expression_text(doc, text, k),
        Expect::Any => proto::parse_expression_text(doc, text, None),
    };
    match parsed {
        Err(e) => (None, Err(e)),
        Ok(e) => {
            let env = |id: &str| {
                let node = doc.pages.first().copied().unwrap_or(NodeId(0));
                doc.resolve_variable(node, id).as_ref().map(ExprValue::from)
            };
            let preview = match e.eval(&env) {
                Ok(v) => format!("= {}", v.display()),
                Err(err) => format!("Evaluates to an error now: {err}"),
            };
            (Some(e), Ok(preview))
        }
    }
}

fn draft_id(ui: &Ui, key: &str) -> egui::Id {
    ui.id().with(("expr-draft", key))
}

/// A text field for an expression with live validation. While the field has focus the text being
/// typed is checked on every keystroke; the message line below shows the problem, or what the
/// expression is worth right now. A committed text that does not parse is kept as a draft (shown
/// in red) until it is fixed; a valid one is returned.
pub(super) fn expression_field(app: &mut App, ui: &mut Ui, pane: &mut Pane, key: &str, current: &str, expect: Expect) -> Option<Expression> {
    let rect = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, theme::FIELD_HEIGHT);
    record(ui, &format!("proto-pop-expr-{key}"), rect);
    let did = draft_id(ui, key);
    let draft = ui.data(|d| d.get_temp::<String>(did));
    let shown = draft.clone().unwrap_or_else(|| current.to_string());
    // the same id `text_input` uses for its edit buffer
    let field_id = ui.id().with(("txt", ("pop-expr", key)));
    let live = if ui.memory(|m| m.has_focus(field_id)) { ui.data(|d| d.get_temp::<String>(field_id)) } else { None };
    let checked_text = live.clone().unwrap_or_else(|| shown.clone());
    let (_, verdict) = check(&app.doc, &checked_text, expect);
    let invalid = verdict.is_err();
    let committed = field::text_input_ph(ui, rect, ("pop-expr", key), &shown, "e.g. $count + 1");
    if invalid {
        ui.painter().rect_stroke(rect, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, theme::DANGER), StrokeKind::Inside);
    }
    pane.y += theme::FIELD_HEIGHT + 2.0;
    // message line
    let (msg, col) = match &verdict {
        Ok(m) => (m.clone(), theme::TEXT_TERTIARY),
        Err(e) => (e.clone(), theme::DANGER),
    };
    let w = theme::SECTION_CONTENT_W;
    let galley = ui.painter().layout(msg, theme::font_caption(), col, w);
    let h = galley.size().y.max(14.0);
    let mr = pane.rect(theme::SECTION_CONTENT_X, 0.0, w, h);
    record(ui, &format!("proto-pop-expr-msg-{key}"), mr);
    ui.painter().galley(mr.min, galley, col);
    pane.y += h + 6.0;
    let text = committed?;
    let (e, verdict) = check(&app.doc, &text, expect);
    match (e, verdict) {
        (Some(e), Ok(_)) => {
            ui.data_mut(|d| d.remove_temp::<String>(did));
            Some(e)
        }
        _ => {
            ui.data_mut(|d| d.insert_temp(did, text));
            None
        }
    }
}
