//! Inspector side of variables: the "bind variable" hover button, the bound
//! variable pill that replaces a field, the picker, and the per-collection
//! mode selector of the Appearance section.

use super::properties::{row_end, Pane};
use crate::app::App;
use crate::icons::{self, Icon};
use crate::theme;
use crate::variables::widgets::{hexagon, pick_item, picker, pill, to_c32, PickResult};
use crate::variables::BindPicker;
use crate::widgets::menu::dropdown;
use crate::widgets::{self};
use designos_doc::*;
use egui::{Align2, Context, Id, Pos2, Rect, Sense, Ui, Vec2};

/// Picker ids of library variables: `LIB_PREFIX <library id> LIB_SEP <asset key>`.
const LIB_PREFIX: &str = "lib\u{1f}";
const LIB_SEP: char = '\u{1e}';

/// The variable bound to `key` on `node`, if any.
pub fn bound_var<'a>(app: &'a App, node: NodeId, key: &str) -> Option<&'a Variable> {
    let id = app.doc.get(node)?.variables.bindings.get(key)?;
    app.doc.variables.get(id)
}

fn open_picker(app: &mut App, sel: &[NodeId], keys: &[&str], kind: VariableKind, anchor: Rect) {
    app.vars.bind_picker = Some(BindPicker { keys: keys.iter().map(|k| k.to_string()).collect(), kind, nodes: sel.to_vec(), anchor, query: String::new() });
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
    widgets::record(ui, &format!("bound:{}", keys[0]), rect);
    let r = pill(ui, rect, ("bound", keys[0]), &var.name, swatch, true);
    if r.detach {
        app.detach_variable(sel, keys);
    } else if r.body.clicked() {
        open_picker(app, sel, keys, var.kind, rect);
    }
    true
}

/// Small hexagon button shown on hover that opens the variable picker.
pub fn hover_button(app: &mut App, ui: &mut Ui, rect: Rect, sel: &[NodeId], keys: &[&str], kind: VariableKind) {
    if app.doc.variables.is_empty() && app.doc.library.enabled.is_empty()
        || !ui.rect_contains_pointer(rect) && app.vars.bind_picker.as_ref().is_none_or(|b| b.keys.first().map(String::as_str) != Some(keys[0]))
    {
        return;
    }
    let b = Rect::from_center_size(Pos2::new(rect.right() - 14.0, rect.center().y), Vec2::splat(20.0));
    let r = ui.interact(b, ui.id().with(("bind-btn", keys[0])), Sense::click());
    let p = ui.painter();
    p.rect_filled(b, theme::cr(theme::RADIUS_CONTROL), if r.hovered() { theme::BG_TERTIARY } else { theme::BG_SECONDARY });
    hexagon(p, b.center(), 5.0, if r.hovered() { theme::ICON } else { theme::ICON_SECONDARY }, false);
    widgets::record(ui, &format!("bind:{}", keys[0]), b);
    if r.clicked() {
        open_picker(app, sel, keys, kind, rect);
    }
    r.on_hover_text("Apply variable");
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

/// Renders the picker opened by [`hover_button`] / a pill.
pub fn bind_picker(app: &mut App, ctx: &Context) {
    let Some(mut bp) = app.vars.bind_picker.take() else { return };
    let Some(first) = bp.nodes.first().copied() else { return };
    let Some(node) = app.doc.get(first) else { return };
    let key0 = bp.keys.first().cloned().unwrap_or_default();
    let current = node.variables.bindings.get(&key0).cloned();
    let kind_node = node.kind.clone();
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
        PickResult::Picked(id) => match id.strip_prefix(LIB_PREFIX).and_then(|r| r.split_once(LIB_SEP)) {
            Some((lib, key)) => app.bind_library_variable(&bp.nodes, &key_refs, lib, key),
            None => app.bind_variable(&bp.nodes, &key_refs, &id),
        },
        PickResult::Detach => app.detach_variable(&bp.nodes, &key_refs),
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
