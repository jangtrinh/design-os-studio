//! Inspector side of variables: the "bind variable" hover button, the bound
//! variable pill that replaces a field, the picker, and the per-collection
//! mode selector of the Appearance section.

use super::properties::{row_end, Pane};
use crate::app::App;
use crate::icons::{self, Icon};
use crate::theme;
use crate::variables::widgets::{hexagon, pick_item, picker, pill, to_c32, PickResult, PillResponse};
use crate::variables::BindPicker;
use crate::widgets::menu::dropdown;
use crate::widgets::{self};
use designos_doc::*;
use egui::{Align2, Context, Id, Pos2, Rect, Sense, Ui, Vec2};

/// Picker ids of library variables: `LIB_PREFIX <library id> LIB_SEP <asset key>`.
const LIB_PREFIX: &str = "lib\u{1f}";
const LIB_SEP: char = '\u{1e}';

#[derive(Clone, Default)]
struct PickerStopBindState {
    prefix: Option<(&'static str, usize)>,
    has_vars: bool,
    open_key: Option<String>,
    bound_stops: std::collections::BTreeMap<String, (String, Option<egui::Color32>)>,
}

#[derive(Clone, Default)]
enum PickerStopBindAction {
    #[default]
    None,
    Open {
        key: String,
        anchor: Rect,
    },
    Detach {
        key: String,
    },
}

fn picker_state_id() -> Id {
    Id::new("picker-stop-bind-state")
}

fn picker_action_id() -> Id {
    Id::new("picker-stop-bind-action")
}

fn sync_picker_paint_from_doc(app: &mut App) {
    let Some(pk) = app.props.picker.as_mut() else { return };
    if !pk.tx.ed.touched().is_empty() {
        return;
    }
    let Some(&first) = pk.ids.first() else { return };
    let Some(n) = app.doc.get(first) else { return };
    let doc_paint = match pk.target {
        super::properties::PickTarget::Fill(pi) => n.fills.get(pi).cloned(),
        super::properties::PickTarget::Stroke(pi) => n.stroke.paints.get(pi).cloned(),
        _ => None,
    };
    if let Some(p) = doc_paint {
        if crate::widgets::picker::mode_of(&p.kind) == crate::widgets::picker::mode_of(&pk.paint.kind) {
            pk.paint = p;
            let n_stops = crate::widgets::picker::stops_of(&pk.paint.kind).map(|s| s.len()).unwrap_or(1);
            pk.state.sel = pk.state.sel.min(n_stops.saturating_sub(1));
            pk.state.reset_stop_origins(&pk.paint);
            let rgba = crate::widgets::picker::active_rgba(&pk.paint, pk.state.sel);
            pk.state.sync(rgba);
        }
    }
}

pub(super) fn sync_picker_context(app: &mut App, ctx: &Context) {
    sync_picker_paint_from_doc(app);
    let mut state = PickerStopBindState::default();
    if let Some(pk) = app.props.picker.as_ref() {
        let prefix = match pk.target {
            super::properties::PickTarget::Fill(pi) => Some(("fills", pi)),
            super::properties::PickTarget::Stroke(pi) => Some(("strokes", pi)),
            _ => None,
        };
        if let (Some((pfx, pi)), false) = (prefix, pk.ids.is_empty()) {
            state.prefix = Some((pfx, pi));
            state.has_vars = !app.doc.variables.is_empty() || !app.doc.library.enabled.is_empty();
            state.open_key = app.vars.bind_picker.as_ref().and_then(|b| b.keys.first().cloned());
            let stop_prefix = format!("{pfx}/{pi}/stops/");
            let nodes: Vec<NodeId> = pk.ids.iter().copied().filter(|&id| app.doc.get(id).is_some()).collect();
            let mut keys = std::collections::BTreeSet::new();
            for &id in &nodes {
                if let Some(node) = app.doc.get(id) {
                    for k in node.variables.bindings.keys() {
                        if k.starts_with(&stop_prefix) && bound_var(app, id, k).is_some() {
                            keys.insert(k.clone());
                        }
                    }
                }
            }
            for k in keys {
                let mut uniform: Option<(&str, &str, Option<egui::Color32>)> = None;
                let mut mixed = false;
                for &id in &nodes {
                    match bound_var(app, id, &k) {
                        Some(var) => {
                            let swatch = match app.doc.bound_value(id, &k) {
                                Some(VariableValue::Color(c)) => Some(to_c32(c)),
                                _ => None,
                            };
                            match uniform {
                                None => uniform = Some((var.id.as_str(), var.name.as_str(), swatch)),
                                Some((fid, fname, fsw)) => {
                                    if fid != var.id.as_str() {
                                        mixed = true;
                                        break;
                                    }
                                    if fsw != swatch {
                                        uniform = Some((fid, fname, None));
                                    }
                                }
                            }
                        }
                        None => {
                            mixed = true;
                            break;
                        }
                    }
                }
                if mixed {
                    state.bound_stops.insert(k, ("Mixed".to_string(), None));
                } else if let Some((_, name, swatch)) = uniform {
                    state.bound_stops.insert(k, (name.to_string(), swatch));
                }
            }
        }
    }
    ctx.data_mut(|d| d.insert_temp(picker_state_id(), state));
}

/// Remaps index-keyed variable bindings under `prefix` (e.g. `"fills/0/stops/"` or `"effects/"`)
/// using `origins[new_idx] = Some(old_idx)`. Keys whose `old_idx` does not appear in `origins`
/// are removed; keys whose `old_idx` moves to `new_idx` keep their trailing suffix (empty for
/// stops, `"/{field}"` for effects) at `"{prefix}{new_idx}{suffix}"`.
pub(super) fn remap_indexed_bindings(bindings: &mut std::collections::BTreeMap<String, String>, prefix: &str, origins: &[Option<usize>]) {
    let old: Vec<(usize, String, String)> = bindings
        .keys()
        .filter_map(|k| {
            let rest = k.strip_prefix(prefix)?;
            let (idx_str, suffix) = match rest.find('/') {
                Some(pos) => (&rest[..pos], &rest[pos..]),
                None => (rest, ""),
            };
            let idx = idx_str.parse::<usize>().ok()?;
            Some((idx, suffix.to_string(), k.clone()))
        })
        .collect();
    if old.is_empty() {
        return;
    }
    let mut removed = Vec::with_capacity(old.len());
    for (idx, suffix, k) in old {
        if let Some(vid) = bindings.remove(&k) {
            removed.push((idx, suffix, vid));
        }
    }
    for (new_idx, orig) in origins.iter().enumerate() {
        if let Some(orig_idx) = orig {
            for (old_idx, suffix, vid) in &removed {
                if old_idx == orig_idx {
                    bindings.insert(format!("{prefix}{new_idx}{suffix}"), vid.clone());
                }
            }
        }
    }
}

fn commit_picker_tx_before_bind(app: &mut App) {
    if let Some(pk) = app.props.picker.as_mut() {
        if !pk.tx.ed.touched().is_empty() {
            let tx = std::mem::replace(&mut pk.tx, crate::edit::Tx::new());
            pk.state.reset_stop_origins(&pk.paint);
            app.run_tx(tx);
        }
    }
}

fn draw_pill(ui: &mut Ui, rect: Rect, record_key: &str, id: impl crate::widgets::Key, label: &str, swatch: Option<egui::Color32>) -> PillResponse {
    widgets::record(ui, record_key, rect);
    pill(ui, rect, id, label, swatch, true)
}

fn draw_hover_button(ui: &mut Ui, rect: Rect, record_key: &str, id: impl crate::widgets::Key, has_vars: bool, is_open: bool) -> bool {
    if !has_vars || !ui.rect_contains_pointer(rect) && !is_open {
        return false;
    }
    let b = Rect::from_center_size(Pos2::new(rect.right() - (theme::SPACE_GROUP + theme::INSET_SEGMENT), rect.center().y), Vec2::splat(20.0));
    let r = ui.interact(b, ui.id().with(id), Sense::click());
    let p = ui.painter();
    p.rect_filled(b, theme::cr(theme::RADIUS_CONTROL), if r.hovered() { theme::BG_TERTIARY } else { theme::BG_SECONDARY });
    hexagon(p, b.center(), 5.0, if r.hovered() { theme::ICON } else { theme::ICON_SECONDARY }, false);
    widgets::record(ui, record_key, b);
    let clicked = r.clicked();
    r.on_hover_text("Apply variable");
    clicked
}

/// Keeps the colour picker open while the variable bind picker is active on a gradient stop.
pub(crate) fn picker_keep_open(ctx: &Context, st: &mut crate::widgets::picker::State) {
    let is_open = ctx.data(|d| d.get_temp::<PickerStopBindState>(picker_state_id())).is_some_and(|s| s.open_key.is_some());
    if is_open {
        st.menu_open = true;
    }
}

/// Whether gradient stop `stop_idx` on the currently edited fill/stroke paint is bound to a variable.
pub(crate) fn picker_stop_is_bound(ctx: &Context, stop_idx: usize) -> bool {
    let Some(st_ctx) = ctx.data(|d| d.get_temp::<PickerStopBindState>(picker_state_id())) else { return false };
    let Some((pfx, pi)) = st_ctx.prefix else { return false };
    let key = format!("{pfx}/{pi}/stops/{stop_idx}");
    st_ctx.bound_stops.contains_key(&key)
}

/// Draws the bound-variable pill in place of the colour picker's value row when gradient stop `stop_idx` is bound.
pub(crate) fn picker_stop_pill_field(ui: &mut Ui, st: &mut crate::widgets::picker::State, rect: Rect, stop_idx: usize) -> bool {
    let Some(st_ctx) = ui.ctx().data(|d| d.get_temp::<PickerStopBindState>(picker_state_id())) else { return false };
    let Some((pfx, pi)) = st_ctx.prefix else { return false };
    let key = format!("{pfx}/{pi}/stops/{stop_idx}");
    let Some((label, swatch)) = st_ctx.bound_stops.get(&key).cloned() else { return false };
    let r = draw_pill(ui, rect, &format!("bound:{key}"), ("bound", key.as_str()), &label, swatch);
    if r.detach {
        st.menu_open = true;
        ui.ctx().data_mut(|d| d.insert_temp(picker_action_id(), PickerStopBindAction::Detach { key }));
    } else if r.body.clicked() {
        st.menu_open = true;
        ui.ctx().data_mut(|d| d.insert_temp(picker_action_id(), PickerStopBindAction::Open { key, anchor: rect }));
    }
    true
}

/// Small hexagon button shown on hover in the colour picker's value row that opens the variable picker for gradient stop `stop_idx`.
pub(crate) fn picker_stop_hover_button(ui: &mut Ui, st: &mut crate::widgets::picker::State, rect: Rect, stop_idx: usize) {
    let Some(st_ctx) = ui.ctx().data(|d| d.get_temp::<PickerStopBindState>(picker_state_id())) else { return };
    let Some((pfx, pi)) = st_ctx.prefix else { return };
    let key = format!("{pfx}/{pi}/stops/{stop_idx}");
    let is_open = st_ctx.open_key.as_deref() == Some(key.as_str());
    if draw_hover_button(ui, rect, &format!("bind:{key}"), ("bind-btn", key.as_str()), st_ctx.has_vars, is_open) {
        st.menu_open = true;
        ui.ctx().data_mut(|d| d.insert_temp(picker_action_id(), PickerStopBindAction::Open { key, anchor: rect }));
    }
}

/// The variable bound to `key` on `node` in `doc`, if any.
pub(super) fn doc_bound_var<'a>(doc: &'a Document, node: NodeId, key: &str) -> Option<&'a Variable> {
    let id = doc.get(node)?.variables.bindings.get(key)?;
    doc.variables.get(id)
}

/// The variable bound to `key` on `node`, if any.
pub fn bound_var<'a>(app: &'a App, node: NodeId, key: &str) -> Option<&'a Variable> {
    doc_bound_var(&app.doc, node, key)
}

fn open_picker(app: &mut App, sel: &[NodeId], keys: &[&str], kind: VariableKind, anchor: Rect) {
    app.vars.bind_picker =
        Some(BindPicker { keys: keys.iter().map(|k| k.to_string()).collect(), kind, nodes: sel.to_vec(), style_key: None, anchor, query: String::new() });
}

fn open_style_picker(app: &mut App, style_key: &str, prop: &str, kind: VariableKind, anchor: Rect) {
    app.vars.bind_picker =
        Some(BindPicker { keys: vec![prop.to_string()], kind, nodes: Vec::new(), style_key: Some(style_key.to_string()), anchor, query: String::new() });
}

/// Draws the bound-variable pill in place of a field when `keys[0]` is bound
/// on the first selected node. Returns whether it did.
pub fn pill_field(app: &mut App, ui: &mut Ui, rect: Rect, sel: &[NodeId], keys: &[&str]) -> bool {
    let Some(first) = sel.first() else { return false };
    let Some(var) = bound_var(app, *first, keys[0]).cloned() else { return false };
    let swatch = match app.doc.bound_value(*first, keys[0]) {
        Some(VariableValue::Color(c)) => Some(to_c32(c)),
        _ => None,
    };
    let r = draw_pill(ui, rect, &format!("bound:{}", keys[0]), ("bound", keys[0]), &var.name, swatch);
    if r.detach {
        app.detach_variable(sel, keys);
    } else if r.body.clicked() {
        open_picker(app, sel, keys, var.kind, rect);
    }
    true
}

/// Small hexagon button shown on hover that opens the variable picker.
pub fn hover_button(app: &mut App, ui: &mut Ui, rect: Rect, sel: &[NodeId], keys: &[&str], kind: VariableKind) {
    let has_vars = !app.doc.variables.is_empty() || !app.doc.library.enabled.is_empty();
    let is_open = app.vars.bind_picker.as_ref().is_some_and(|b| b.keys.first().map(String::as_str) == Some(keys[0]));
    if draw_hover_button(ui, rect, &format!("bind:{}", keys[0]), ("bind-btn", keys[0]), has_vars, is_open) {
        open_picker(app, sel, keys, kind, rect);
    }
}

/// A numeric field that turns into a variable pill when bound, with a bind
/// button on hover when not.
pub fn numeric(app: &mut App, ui: &mut Ui, rect: Rect, sel: &[NodeId], keys: &[&str], field: impl FnOnce(&mut Ui) -> Option<f32>) -> Option<f32> {
    if pill_field(app, ui, rect, sel, keys) {
        return None;
    }
    let v = field(ui);
    hover_button(app, ui, rect, sel, keys, VariableKind::Number);
    v
}

/// Draws the bound-variable pill in place of a style editor field when `prop` is bound on `style_key`.
pub fn style_pill_field(app: &mut App, ui: &mut Ui, rect: Rect, style_key: &str, prop: &str) -> bool {
    let Some(vid) = app.doc.style_meta.get(style_key).and_then(|m| m.bindings.get(prop)).cloned() else {
        return false;
    };
    let (label, kind) = match app.doc.variables.get(&vid) {
        Some(var) => (var.name.clone(), var.kind),
        None => (format!("{vid} (missing)"), property_kind(prop).unwrap_or(VariableKind::Color)),
    };
    let swatch = match app.doc.resolve_variable_default(&vid) {
        Some(VariableValue::Color(c)) => Some(to_c32(c)),
        _ => None,
    };
    let r = draw_pill(ui, rect, &format!("style-bound:{style_key}:{prop}"), ("style-bound", style_key, prop), &label, swatch);
    if r.detach {
        app.unbind_style_variable(style_key, prop);
    } else if r.body.clicked() {
        open_style_picker(app, style_key, prop, kind, rect);
    }
    true
}

/// Small hexagon button shown on hover in the style editor that opens the variable picker for `prop`.
pub fn style_hover_button(app: &mut App, ui: &mut Ui, rect: Rect, style_key: &str, prop: &str, kind: VariableKind) {
    let has_vars = !app.doc.variables.is_empty() || !app.doc.library.enabled.is_empty();
    let is_open = app.vars.bind_picker.as_ref().is_some_and(|b| b.style_key.as_deref() == Some(style_key) && b.keys.first().map(String::as_str) == Some(prop));
    if draw_hover_button(ui, rect, &format!("style-bind:{style_key}:{prop}"), ("style-bind-btn", style_key, prop), has_vars, is_open) {
        open_style_picker(app, style_key, prop, kind, rect);
    }
}

/// A numeric field in the style editor that turns into a variable pill when bound.
pub fn style_numeric(app: &mut App, ui: &mut Ui, rect: Rect, style_key: &str, prop: &str, field: impl FnOnce(&mut Ui) -> Option<f32>) -> Option<f32> {
    if style_pill_field(app, ui, rect, style_key, prop) {
        return None;
    }
    let v = field(ui);
    style_hover_button(app, ui, rect, style_key, prop, VariableKind::Number);
    v
}

/// Renders the picker opened by [`hover_button`] / [`style_hover_button`] / a pill.
pub fn bind_picker(app: &mut App, ctx: &Context) {
    if let Some(action) = ctx.data_mut(|d| d.remove_temp::<PickerStopBindAction>(picker_action_id())) {
        let ids = app.props.picker.as_ref().map(|pk| pk.ids.clone()).unwrap_or_default();
        if !ids.is_empty() {
            match action {
                PickerStopBindAction::None => {}
                PickerStopBindAction::Open { key, anchor } => {
                    open_picker(app, &ids, &[key.as_str()], VariableKind::Color, anchor);
                }
                PickerStopBindAction::Detach { key } => {
                    commit_picker_tx_before_bind(app);
                    app.detach_variable(&ids, &[key.as_str()]);
                }
            }
        }
    }
    let Some(mut bp) = app.vars.bind_picker.take() else {
        return;
    };
    let key0 = bp.keys.first().cloned().unwrap_or_default();
    let (current, kind_node) = if let Some(skey) = &bp.style_key {
        let cur = app.doc.style_meta.get(skey).and_then(|m| m.bindings.get(&key0)).cloned();
        let kn = match app.doc.styles.get(skey) {
            Some(Style::Text(_)) => NodeKind::Text(TextProps::default()),
            _ => NodeKind::Rectangle,
        };
        (cur, kn)
    } else {
        let Some(first) = bp.nodes.first().copied() else {
            return;
        };
        let Some(node) = app.doc.get(first) else {
            return;
        };
        (node.variables.bindings.get(&key0).cloned(), node.kind.clone())
    };
    let mut items = Vec::new();
    for c in app.doc.variable_collections.values() {
        for vid in &c.variables {
            let Some(v) = app.doc.variables.get(vid) else { continue };
            if v.kind == bp.kind && scope_allows(&v.scopes, &key0, &kind_node) {
                items.push(pick_item(&app.doc, v));
            }
        }
    }
    // Variables of enabled libraries, one section per library; copied into the file when picked.
    for lv in app.library_variables().into_iter().filter(|v| v.kind == bp.kind && v.local_id.is_none() && scope_allows(&v.scopes, &key0, &kind_node)) {
        let Some(render) = app.library_render_doc(&lv.library_id) else { continue };
        let Some(v) = render.variables.get(lv.key.strip_prefix("var:").unwrap_or(&lv.key)) else { continue };
        let mut item = pick_item(&render, v);
        item.id = format!("{LIB_PREFIX}{}{LIB_SEP}{}", lv.library_id, lv.key);
        item.group = lv.library_name.clone();
        item.name = lv.name.clone();
        items.push(item);
    }
    let title = match bp.kind {
        VariableKind::Color => "Colors",
        VariableKind::Number => "Numbers",
        VariableKind::String => "Strings",
        VariableKind::Boolean => "Booleans",
    };
    let screen = ctx.content_rect();
    let pos = Pos2::new((bp.anchor.left() - 256.0).max(screen.left() + 8.0), bp.anchor.top() - 8.0);
    let keys: Vec<String> = bp.keys.clone();
    let key_refs: Vec<&str> = keys.iter().map(String::as_str).collect();
    match picker(ctx, Id::new("bind-picker"), pos, Some(bp.anchor), title, &mut bp.query, &items, current.as_deref(), current.is_some()) {
        PickResult::Open => app.vars.bind_picker = Some(bp),
        PickResult::Closed => {}
        PickResult::Picked(id) => {
            if let Some(skey) = &bp.style_key {
                match id.strip_prefix(LIB_PREFIX).and_then(|r| r.split_once(LIB_SEP)) {
                    Some((lib, key)) => {
                        app.bind_style_library_variable(skey, &key0, lib, key);
                    }
                    None => {
                        app.bind_style_variable(skey, &key0, &id);
                    }
                }
            } else {
                commit_picker_tx_before_bind(app);
                match id.strip_prefix(LIB_PREFIX).and_then(|r| r.split_once(LIB_SEP)) {
                    Some((lib, key)) => app.bind_library_variable(&bp.nodes, &key_refs, lib, key),
                    None => app.bind_variable(&bp.nodes, &key_refs, &id),
                }
            }
        }
        PickResult::Detach => {
            if let Some(skey) = &bp.style_key {
                app.unbind_style_variable(skey, &key0);
            } else {
                commit_picker_tx_before_bind(app);
                app.detach_variable(&bp.nodes, &key_refs);
            }
        }
    }
}

/// "Variable mode" rows for a container: one dropdown per collection, where
/// "Auto" inherits the mode from the parent.
pub(super) fn mode_rows(app: &mut App, ui: &mut Ui, pane: &mut Pane, sel: &[NodeId]) {
    if app.doc.variable_collections.is_empty() {
        return;
    }
    let Some(first) = sel.first().copied() else { return };
    let colls: Vec<VariableCollection> = app.doc.variable_collections.values().cloned().collect();
    for c in colls {
        let explicit = app.doc.get(first).and_then(|n| n.variables.modes.get(&c.id)).cloned();
        let effective = app.doc.variable_mode(first, &c.id).unwrap_or_default();
        let mode_name = |id: &str| c.modes.iter().find(|m| m.id == id).map(|m| m.name.clone()).unwrap_or_else(|| id.to_string());
        let mut options: Vec<String> = vec!["Auto".into()];
        options.extend(c.modes.iter().map(|m| m.name.clone()));
        let label = match &explicit {
            Some(m) => mode_name(m),
            None => format!("Auto ({})", mode_name(&effective)),
        };
        let lr = pane.col(0, 0.0);
        icons::paint(ui.painter(), Icon::Variables, Pos2::new(lr.left() + 8.0, lr.center().y), 14.0, theme::ICON_SECONDARY);
        let nm = widgets::elide(ui.painter(), &c.name, &theme::font_body(), lr.width() - 28.0);
        ui.painter().text(Pos2::new(lr.left() + 24.0, lr.center().y), Align2::LEFT_CENTER, nm, theme::font_body(), theme::TEXT_SECONDARY);
        let dd = pane.col(1, 0.0);
        widgets::record(ui, &format!("mode:{}", c.name), dd);
        if let Some(i) = dropdown(ui, dd, ("var-mode", &c.id), &label, &options) {
            let pick = if i == 0 { None } else { Some(c.modes[i - 1].id.clone()) };
            app.set_node_mode(sel, &c.id, pick.as_deref());
        }
        row_end(pane);
    }
}
