//! Inspector sections for components and instances: instance header with the
//! swap picker, component properties, property definitions on main
//! components and binding a layer to a property.

use super::properties::{row_end, section_sep, sublabel, title, Pane};
use crate::app::App;
use crate::icons::Icon;
use crate::theme;
use crate::widgets;
use crate::widgets::field;
use crate::widgets::menu::{dropdown, dropdown_checked, popup_menu, MenuItem, MenuResult};
use designos_doc::*;
use egui::{Align2, Pos2, Rect, Sense, Ui, Vec2};

/// Draws the component-related sections for a single selected layer, right
/// below the name header. Each section ends with a separator.
pub(super) fn sections(app: &mut App, ui: &mut Ui, pane: &mut Pane, id: NodeId) {
    let Some(n) = app.doc.get(id).cloned() else { return };
    match &n.kind {
        NodeKind::Instance { .. } if is_top_instance(&n) => {
            instance_header(app, ui, pane, id);
            instance_props(app, ui, pane, id);
            overrides_row(app, ui, pane, id);
            if enclosing_component(&app.doc, id).is_some() {
                layer_bindings(app, ui, pane, id);
            }
        }
        NodeKind::Component(_) | NodeKind::ComponentSet(_) => component_props(app, ui, pane, id),
        _ => {
            if is_slot_container(&app.doc, id) {
                instance_slot_content(app, ui, pane, id);
            }
            if is_linked(&n) {
                overrides_row(app, ui, pane, id);
            } else {
                layer_bindings(app, ui, pane, id);
            }
        }
    }
}

fn comp_label(doc: &Document, comp: NodeId) -> String {
    let Some(c) = doc.get(comp) else { return "Missing component".into() };
    match c.parent.and_then(|p| doc.get(p)).filter(|p| matches!(p.kind, NodeKind::ComponentSet(_))) {
        Some(set) => format!("{} / {}", set.name, c.name),
        None => c.name.clone(),
    }
}

fn all_components_cached(app: &mut App) -> Vec<NodeId> {
    let key = (app.doc.nodes.len() as u64) ^ (app.canvas.tiles.rev() << 32);
    if let Some((k, cached)) = &app.props.all_components_cache {
        if *k == key {
            return cached.clone();
        }
    }
    let mut v: Vec<NodeId> = app.doc.nodes.values().filter(|n| matches!(n.kind, NodeKind::Component(_))).map(|n| n.id).collect();
    v.sort_by_key(|id| comp_label(&app.doc, *id));
    app.props.all_components_cache = Some((key, v.clone()));
    v
}

fn is_dropdown_open(ui: &Ui, key: impl crate::widgets::Key) -> bool {
    let id = ui.id().with(("dd", key));
    ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false)
}

/// 28x16 switch; returns the new state when clicked.
fn toggle(ui: &mut Ui, rect: Rect, key: &str, on: bool) -> Option<bool> {
    let r = ui.interact(rect, ui.id().with(("tog", key)), Sense::click());
    widgets::record(ui, &format!("toggle:{key}"), rect);
    let sw = Rect::from_min_size(Pos2::new(rect.left(), rect.center().y - theme::SPACE_INLINE), Vec2::new(28.0, 16.0));
    let p = ui.painter();
    p.rect_filled(
        sw,
        theme::cr(8),
        if on {
            theme::BG_BRAND
        } else if r.hovered() {
            theme::BG_TERTIARY
        } else {
            theme::BG_SECONDARY
        },
    );
    let kx = if on { sw.right() - theme::SPACE_INLINE } else { sw.left() + theme::SPACE_INLINE };
    p.circle_filled(Pos2::new(kx, sw.center().y), 6.0, egui::Color32::WHITE);
    r.clicked().then_some(!on)
}

// ---------------------------------------------------------------- instance
fn instance_header(app: &mut App, ui: &mut Ui, pane: &mut Pane, inst: NodeId) {
    let comp = app.main_of(inst);
    let label = comp.map(|c| comp_label(&app.doc, c)).unwrap_or_else(|| "Missing component".into());
    let row_y = 0.0;
    // Swap picker (the component name), "go to main" and the more menu.
    let dd = pane.rect(theme::SECTION_CONTENT_X, row_y, 156.0, theme::FIELD_HEIGHT);
    widgets::record(ui, "instance-swap", dd);
    if is_dropdown_open(ui, "instance-swap") {
        let comps = all_components_cached(app);
        let names: Vec<String> = comps.iter().map(|c| comp_label(&app.doc, *c)).collect();
        if let Some(i) = dropdown(ui, dd, "instance-swap", &label, &names) {
            app.swap_instance(inst, comps[i]);
        }
    } else {
        let _ = dropdown(ui, dd, "instance-swap", &label, &[]);
    }
    ui.painter().rect_stroke(
        dd,
        theme::cr(theme::RADIUS_CONTROL),
        egui::Stroke::new(1.0, theme::CANVAS_COMPONENT.gamma_multiply(0.55)),
        egui::StrokeKind::Inside,
    );
    let go = pane.rect(181.0, row_y, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
    widgets::record(ui, "go-to-main", go);
    if widgets::icon_button(ui, go, "go-main", Icon::Component, false, "Go to main component").clicked() {
        app.go_to_main(inst);
    }
    let more = pane.rect(theme::SECTION_TRAILING_X, row_y, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
    widgets::record(ui, "instance-more", more);
    if widgets::icon_button(ui, more, "inst-more", Icon::ChevronDown, app.ui.inst_menu.is_some(), "More").clicked() {
        app.ui.inst_menu = if app.ui.inst_menu.is_some() { None } else { Some(more) };
    }
    pane.y += theme::FIELD_HEIGHT + 8.0;
    if let Some(info) = library_instance_info(&app.doc, inst) {
        let row = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, 20.0);
        crate::libraries::instance_library_row(ui, row, &info);
        pane.y += 28.0;
    }
    section_sep(ui, pane, false);

    if let Some(anchor) = app.ui.inst_menu {
        let changed = has_changes(app, inst);
        let items = [
            MenuItem::new("Go to main component").disabled(comp.is_none()),
            MenuItem::new("Reset all changes").disabled(!changed),
            MenuItem::new("Detach instance").shortcut("Ctrl+Alt+B"),
        ];
        match popup_menu(ui.ctx(), egui::Id::new("inst-menu"), anchor.left_bottom() + Vec2::new(-150.0, 2.0), 180.0, &items, Some(anchor)) {
            MenuResult::Picked(i) => {
                app.ui.inst_menu = None;
                match i {
                    0 => app.go_to_main(inst),
                    1 => app.reset_selection(),
                    _ => app.detach_selection(),
                }
            }
            MenuResult::Closed => app.ui.inst_menu = None,
            MenuResult::Open => {}
        }
    }
}

fn has_changes(app: &App, inst: NodeId) -> bool {
    let mut stack = vec![inst];
    let mut visited = 0usize;
    while let Some(id) = stack.pop() {
        if visited >= 512 {
            break;
        }
        let Some(n) = app.doc.get(id) else { continue };
        visited += 1;
        if !n.component.overrides.is_empty() || (n.id == inst && !n.component.values.is_empty()) {
            return true;
        }
        stack.extend(n.children.iter().copied());
    }
    false
}

fn instance_props(app: &mut App, ui: &mut Ui, pane: &mut Pane, inst: NodeId) {
    let Some(comp) = app.main_of(inst) else { return };
    let defs = prop_defs(&app.doc, comp);
    if defs.is_empty() {
        return;
    }
    title(ui, pane, "Properties", None, false);
    for def in defs {
        let value = prop_value(&app.doc, inst, &def);
        let lab = pane.rect(theme::SECTION_CONTENT_X, 0.0, 92.0, theme::FIELD_HEIGHT);
        let is_set = app.doc.get(inst).is_some_and(|n| n.component.values.contains_key(&def.name));
        let shown = widgets::elide(ui.painter(), &def.name, &theme::font_body(), lab.width() - 4.0);
        ui.painter().text(
            Pos2::new(lab.left(), lab.center().y),
            Align2::LEFT_CENTER,
            shown,
            theme::font_body(),
            if is_set { theme::TEXT } else { theme::TEXT_SECONDARY },
        );
        let ctl = pane.col(1, 0.0);
        let key = format!("prop:{}", def.name);
        widgets::record(ui, &key, ctl);
        match (&def.kind, &value) {
            (PropKind::Boolean, PropValue::Bool(b)) => {
                if let Some(v) = toggle(ui, ctl, &key, *b) {
                    app.set_instance_prop(inst, &def.name, Some(PropValue::Bool(v)));
                }
            }
            (PropKind::Text, PropValue::Text(s)) => {
                if let Some(v) = field::text_input(ui, ctl, &key, s) {
                    app.set_instance_prop(inst, &def.name, Some(PropValue::Text(v)));
                }
            }
            (PropKind::Variant { options }, v) => {
                let cur = match v {
                    PropValue::Variant(s) => s.clone(),
                    _ => String::new(),
                };
                if let Some(i) = dropdown(ui, ctl, &key, &cur, options) {
                    app.set_instance_prop(inst, &def.name, Some(PropValue::Variant(options[i].clone())));
                }
            }
            (PropKind::InstanceSwap { .. }, v) => {
                let cur = match v {
                    PropValue::Instance(c) => comp_label(&app.doc, *c),
                    _ => String::new(),
                };
                if is_dropdown_open(ui, &key) {
                    let comps = all_components_cached(app);
                    let names: Vec<String> = comps.iter().map(|c| comp_label(&app.doc, *c)).collect();
                    if let Some(i) = dropdown(ui, ctl, &key, &cur, &names) {
                        app.set_instance_prop(inst, &def.name, Some(PropValue::Instance(comps[i])));
                    }
                } else {
                    let _ = dropdown(ui, ctl, &key, &cur, &[]);
                }
            }
            _ => {}
        }
        row_end(pane);
    }
    pane.y += 4.0;
    section_sep(ui, pane, false);
}

/// Inspector section for a selected slot container: shows current child count / max limit,
/// a "+" preferred-instance picker button and dropdown, and child rows with reorder / remove controls.
fn instance_slot_content(app: &mut App, ui: &mut Ui, pane: &mut Pane, slot: NodeId) {
    let Some(sn) = app.doc.get(slot).cloned() else { return };
    let cfg = slot_config_of(&app.doc, slot).unwrap_or_default();
    let count_str = match cfg.max_children {
        Some(max) => format!("Slot content ({}/{max})", sn.children.len()),
        None => format!("Slot content ({})", sn.children.len()),
    };
    title(ui, pane, &count_str, None, false);
    let plus = pane.rect(theme::SECTION_TRAILING_X, -theme::SECTION_TITLE_HEIGHT + 8.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
    widgets::record(ui, "slot-plus-picker", plus);
    if widgets::icon_button(ui, plus, "slot-plus-picker", Icon::Plus, app.ui.slot_picker.map(|(s, _)| s) == Some(slot), "Insert preferred instance").clicked() {
        app.ui.slot_picker = if app.ui.slot_picker.map(|(s, _)| s) == Some(slot) { None } else { Some((slot, plus)) };
    }

    // "+ Add instance" dropdown (lists preferred components when set, otherwise all components).
    let dd = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, theme::FIELD_HEIGHT);
    widgets::record(ui, "slot-add-child", dd);
    if is_dropdown_open(ui, "slot-add-child") {
        let comps = preferred_slot_components(&app.doc, slot);
        let names: Vec<String> = comps.iter().map(|c| comp_label(&app.doc, *c)).collect();
        if let Some(i) = dropdown(ui, dd, "slot-add-child", "Add instance...", &names) {
            app.insert_instance_into_slot(slot, comps[i]);
        }
    } else {
        let _ = dropdown(ui, dd, "slot-add-child", "Add instance...", &[]);
    }
    row_end(pane);

    for (idx, &cid) in sn.children.iter().enumerate() {
        let cname = app.doc.get(cid).map(|n| n.name.clone()).unwrap_or_else(|| "Item".into());
        let r = pane.rect(theme::SECTION_CONTENT_X, 0.0, 148.0, theme::FIELD_HEIGHT);
        let shown = widgets::elide(ui.painter(), &cname, &theme::font_body(), r.width());
        ui.painter().text(Pos2::new(r.left() + 4.0, r.center().y), Align2::LEFT_CENTER, shown, theme::font_body(), theme::TEXT);

        let down = pane.rect(168.0, 0.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
        let down_key = format!("slot-child-down:{}", cid.0);
        widgets::record(ui, &down_key, down);
        if idx + 1 < sn.children.len() && widgets::icon_button(ui, down, &down_key, Icon::ChevronDown, false, "Move later").clicked() {
            app.selection = vec![cid];
            app.reorder(true, false);
            app.selection = vec![slot];
        }

        let del = pane.rect(theme::SECTION_TRAILING_X, 0.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
        let del_key = format!("slot-child-del:{}", cid.0);
        widgets::record(ui, &del_key, del);
        if widgets::icon_button(ui, del, &del_key, Icon::Minus, false, "Remove from slot").clicked() {
            app.selection = vec![cid];
            app.delete_selection();
            app.selection = vec![slot];
        }
        row_end(pane);
    }
    pane.y += 4.0;
    section_sep(ui, pane, false);
}

/// Renders the active preferred-instance `+` picker popup (opened from either the canvas empty-slot
/// `+` button or the inspector `Slot content` `+` button).
pub fn show_slot_picker_popup(app: &mut App, ctx: &egui::Context) {
    let Some((slot, anchor)) = app.ui.slot_picker else { return };
    if !is_slot_container(&app.doc, slot) {
        app.ui.slot_picker = None;
        return;
    }
    let comps = preferred_slot_components(&app.doc, slot);
    let items: Vec<MenuItem> = if comps.is_empty() {
        vec![MenuItem::new("No components available").disabled(true)]
    } else {
        comps.iter().map(|&c| MenuItem::new(comp_label(&app.doc, c))).collect()
    };
    let pos = Pos2::new((anchor.center().x - 90.0).max(8.0), anchor.bottom() + 4.0);
    match popup_menu(ctx, egui::Id::new(("slot-picker-menu", slot)), pos, 180.0, &items, Some(anchor)) {
        MenuResult::Picked(i) => {
            app.ui.slot_picker = None;
            if let Some(&comp) = comps.get(i) {
                app.insert_instance_into_slot(slot, comp);
            }
        }
        MenuResult::Closed => app.ui.slot_picker = None,
        MenuResult::Open => {}
    }
}

/// "Changed in this instance" row with a reset button, for instances and the
/// layers inside them.
fn overrides_row(app: &mut App, ui: &mut Ui, pane: &mut Pane, id: NodeId) {
    let Some(n) = app.doc.get(id) else { return };
    let fields: Vec<OverrideField> = n.component.overrides.iter().copied().collect();
    if fields.is_empty() {
        return;
    }
    let names: Vec<&str> = fields.iter().map(|f| override_name(*f)).collect();
    let r = pane.rect(theme::SECTION_CONTENT_X, 0.0, 128.0, theme::FIELD_HEIGHT);
    let shown = widgets::elide(ui.painter(), &format!("Changed: {}", names.join(", ")), &theme::font_body(), r.width());
    ui.painter().text(Pos2::new(r.left(), r.center().y), Align2::LEFT_CENTER, shown, theme::font_body(), theme::TEXT_SECONDARY);
    let btn = pane.rect(theme::FIELD_COL_X[1] + 24.0, 0.0, 64.0, theme::FIELD_HEIGHT);
    widgets::record(ui, "reset-overrides", btn);
    let resp = ui.interact(btn, ui.id().with("reset-overrides"), Sense::click());
    ui.painter().rect_filled(btn, theme::cr(theme::RADIUS_CONTROL), if resp.hovered() { theme::BG_TERTIARY } else { theme::BG_SECONDARY });
    ui.painter().text(btn.center(), Align2::CENTER_CENTER, "Reset", theme::font_body(), theme::TEXT);
    if resp.clicked() {
        app.reset_node_overrides(id, &fields);
    }
    row_end(pane);
    pane.y += 4.0;
    section_sep(ui, pane, false);
}

fn override_name(f: OverrideField) -> &'static str {
    match f {
        OverrideField::Name => "name",
        OverrideField::Visible => "visibility",
        OverrideField::Locked => "lock",
        OverrideField::Opacity => "opacity",
        OverrideField::Blend => "blend",
        OverrideField::Size => "size",
        OverrideField::Transform => "position",
        OverrideField::Fills => "fill",
        OverrideField::Stroke => "stroke",
        OverrideField::Effects => "effects",
        OverrideField::CornerRadii => "radius",
        OverrideField::Characters => "text",
        OverrideField::TextStyle => "text style",
        OverrideField::Layout => "layout",
        OverrideField::MainComponent => "component",
        OverrideField::SlotContent => "slot content",
    }
}

// ---------------------------------------------------------------- main component
fn kind_label(k: &PropKind) -> &'static str {
    match k {
        PropKind::Boolean => "Boolean",
        PropKind::Text => "Text",
        PropKind::Variant { .. } => "Variant",
        PropKind::InstanceSwap { .. } => "Instance",
        PropKind::Slot => "Slot",
    }
}

fn component_props(app: &mut App, ui: &mut Ui, pane: &mut Pane, id: NodeId) {
    let owner = if matches!(app.doc.get(id).map(|n| &n.kind), Some(NodeKind::ComponentSet(_))) { id } else { app.prop_owner(id) };
    let defs = app.doc.get(owner).map(|n| n.component.props.clone()).unwrap_or_default();
    let plus = pane.rect(theme::SECTION_TRAILING_X, 8.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
    widgets::record(ui, "prop-add", plus);
    if title(ui, pane, "Properties", Some(Icon::Plus), defs.is_empty()) {
        app.ui.prop_menu = if app.ui.prop_menu.is_some() { None } else { Some(plus) };
    }
    for def in &defs {
        let name_r = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W - 28.0, theme::FIELD_HEIGHT);
        widgets::record(ui, &format!("propdef:{}", def.name), name_r);
        if let Some(new) = field::text_input(ui, name_r, ("propdef", &def.name), &def.name) {
            app.rename_prop_def(owner, &def.name, &new);
        }
        if !matches!(def.kind, PropKind::Variant { .. }) {
            let minus = pane.rect(theme::SECTION_TRAILING_X, 0.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
            widgets::record(ui, &format!("propdel:{}", def.name), minus);
            if widgets::icon_button(ui, minus, ("propdel", &def.name), Icon::Minus, false, "Delete property").clicked() {
                app.delete_prop_def(owner, &def.name);
            }
        }
        row_end(pane);
        let lab = pane.col(0, 0.0);
        ui.painter().text(Pos2::new(lab.left(), lab.center().y), Align2::LEFT_CENTER, kind_label(&def.kind), theme::font_body(), theme::TEXT_SECONDARY);
        let ctl = pane.col(1, 0.0);
        let key = format!("default:{}", def.name);
        match (&def.kind, &def.default) {
            (PropKind::Boolean, PropValue::Bool(b)) => {
                if let Some(v) = toggle(ui, ctl, &key, *b) {
                    app.set_prop_default(owner, &def.name, PropValue::Bool(v));
                }
            }
            (PropKind::Text, PropValue::Text(s)) => {
                if let Some(v) = field::text_input(ui, ctl, &key, s) {
                    app.set_prop_default(owner, &def.name, PropValue::Text(v));
                }
            }
            (PropKind::Variant { options }, PropValue::Variant(cur)) => {
                if let Some(i) = dropdown(ui, ctl, &key, cur, options) {
                    app.set_prop_default(owner, &def.name, PropValue::Variant(options[i].clone()));
                }
            }
            (PropKind::InstanceSwap { .. }, PropValue::Instance(c)) => {
                let cur_label = comp_label(&app.doc, *c);
                if is_dropdown_open(ui, &key) {
                    let comps: Vec<NodeId> = all_components_cached(app).into_iter().filter(|x| *x != owner).collect();
                    let names: Vec<String> = comps.iter().map(|x| comp_label(&app.doc, *x)).collect();
                    if let Some(i) = dropdown(ui, ctl, &key, &cur_label, &names) {
                        app.set_prop_default(owner, &def.name, PropValue::Instance(comps[i]));
                    }
                } else {
                    let _ = dropdown(ui, ctl, &key, &cur_label, &[]);
                }
            }
            (PropKind::Slot, _) => {
                let cfg = app.doc.get(owner).and_then(|n| n.component.slot_configs.get(&def.name)).cloned().unwrap_or_default();
                let summary = match (cfg.min_children, cfg.max_children) {
                    (Some(min), Some(max)) => format!("{min}–{max} items"),
                    (Some(min), None) => format!("≥ {min} items"),
                    (None, Some(max)) => format!("≤ {max} items"),
                    (None, None) => "Any content".into(),
                };
                ui.painter().text(Pos2::new(ctl.left(), ctl.center().y), Align2::LEFT_CENTER, summary, theme::font_body(), theme::TEXT_SECONDARY);
            }
            _ => {}
        }
        row_end(pane);
        if matches!(def.kind, PropKind::Slot) {
            slot_settings_rows(app, ui, pane, owner, &def.name);
        }
        pane.y += 4.0;
    }
    if let Some(anchor) = app.ui.prop_menu {
        let items = [MenuItem::new("Boolean"), MenuItem::new("Text"), MenuItem::new("Instance swap"), MenuItem::new("Slot")];
        match popup_menu(ui.ctx(), egui::Id::new("prop-add-menu"), anchor.left_bottom() + Vec2::new(-150.0, 2.0), 170.0, &items, Some(anchor)) {
            MenuResult::Picked(i) => {
                app.ui.prop_menu = None;
                let kind = match i {
                    0 => PropKind::Boolean,
                    1 => PropKind::Text,
                    2 => PropKind::InstanceSwap { preferred: vec![] },
                    _ => PropKind::Slot,
                };
                app.add_prop_def(owner, kind);
            }
            MenuResult::Closed => app.ui.prop_menu = None,
            MenuResult::Open => {}
        }
    }
    section_sep(ui, pane, false);
}

fn parse_optional_u32(raw: &str) -> Option<Option<u32>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") || trimmed == "-" {
        Some(None)
    } else {
        trimmed.parse::<u32>().ok().map(Some)
    }
}

fn slot_settings_rows(app: &mut App, ui: &mut Ui, pane: &mut Pane, owner: NodeId, prop_name: &str) {
    let cfg = app.doc.get(owner).and_then(|n| n.component.slot_configs.get(prop_name)).cloned().unwrap_or_default();

    // Min children
    let min_lab = pane.rect(theme::SECTION_CONTENT_X, 0.0, 128.0, theme::FIELD_HEIGHT);
    ui.painter().text(Pos2::new(min_lab.left(), min_lab.center().y), Align2::LEFT_CENTER, "Min children", theme::font_body(), theme::TEXT_SECONDARY);
    let min_ctl = pane.col(1, 0.0);
    widgets::record(ui, "slot-min-children", min_ctl);
    widgets::record(ui, &format!("slot-min:{prop_name}"), min_ctl);
    let min_str = cfg.min_children.map(|v| v.to_string()).unwrap_or_default();
    if let Some(committed) = field::text_input_ph(ui, min_ctl, ("slot-min", prop_name), &min_str, "None") {
        if let Some(parsed) = parse_optional_u32(&committed) {
            let mut next = cfg.clone();
            next.min_children = parsed;
            app.set_slot_config(owner, prop_name, next);
        }
    }
    row_end(pane);

    // Max children
    let max_lab = pane.rect(theme::SECTION_CONTENT_X, 0.0, 128.0, theme::FIELD_HEIGHT);
    ui.painter().text(Pos2::new(max_lab.left(), max_lab.center().y), Align2::LEFT_CENTER, "Max children", theme::font_body(), theme::TEXT_SECONDARY);
    let max_ctl = pane.col(1, 0.0);
    widgets::record(ui, "slot-max-children", max_ctl);
    widgets::record(ui, &format!("slot-max:{prop_name}"), max_ctl);
    let max_str = cfg.max_children.map(|v| v.to_string()).unwrap_or_default();
    if let Some(committed) = field::text_input_ph(ui, max_ctl, ("slot-max", prop_name), &max_str, "None") {
        if let Some(parsed) = parse_optional_u32(&committed) {
            let mut next = cfg.clone();
            next.max_children = parsed;
            app.set_slot_config(owner, prop_name, next);
        }
    }
    row_end(pane);

    // Preferred instances only
    let pref_lab = pane.rect(theme::SECTION_CONTENT_X, 0.0, 152.0, theme::FIELD_HEIGHT);
    ui.painter().text(
        Pos2::new(pref_lab.left(), pref_lab.center().y),
        Align2::LEFT_CENTER,
        "Preferred instances only",
        theme::font_body(),
        theme::TEXT_SECONDARY,
    );
    let pref_ctl = pane.rect(theme::SECTION_TRAILING_X - 4.0, 0.0, 28.0, theme::FIELD_HEIGHT);
    let pref_on = cfg.allow_preferred_values_only.unwrap_or(false);
    if let Some(v) = toggle(ui, pref_ctl, &format!("slot-preferred-only:{prop_name}"), pref_on) {
        let mut next = cfg.clone();
        next.allow_preferred_values_only = Some(v);
        app.set_slot_config(owner, prop_name, next);
    }
    row_end(pane);

    if pref_on || !cfg.preferred.is_empty() {
        let cur_pref = match cfg.preferred.split_first() {
            None => "Add preferred…".to_string(),
            Some((first, [])) => comp_label(&app.doc, *first),
            Some((first, rest)) => format!("{} +{}", comp_label(&app.doc, *first), rest.len()),
        };
        let dd_r = pane.full(0.0);
        let pref_key = format!("slot-preferred-picker:{prop_name}");
        widgets::record(ui, "slot-preferred-picker", dd_r);
        if is_dropdown_open(ui, &pref_key) {
            let comps: Vec<NodeId> = all_components_cached(app).into_iter().filter(|x| *x != owner).collect();
            let names: Vec<String> = comps.iter().map(|x| comp_label(&app.doc, *x)).collect();
            let selected: Vec<bool> = comps.iter().map(|x| cfg.preferred.contains(x)).collect();
            if let Some(i) = dropdown_checked(ui, dd_r, &pref_key, &cur_pref, &names, &selected) {
                let picked = comps[i];
                let mut next = cfg.clone();
                if let Some(pos) = next.preferred.iter().position(|x| *x == picked) {
                    next.preferred.remove(pos);
                } else {
                    next.preferred.push(picked);
                }
                app.set_slot_config(owner, prop_name, next);
            }
        } else {
            let _ = dropdown(ui, dd_r, &pref_key, &cur_pref, &[]);
        }
        row_end(pane);
    }

    // Stretch child on insert
    let stretch_lab = pane.rect(theme::SECTION_CONTENT_X, 0.0, 152.0, theme::FIELD_HEIGHT);
    ui.painter().text(
        Pos2::new(stretch_lab.left(), stretch_lab.center().y),
        Align2::LEFT_CENTER,
        "Stretch child on insert",
        theme::font_body(),
        theme::TEXT_SECONDARY,
    );
    let stretch_ctl = pane.rect(theme::SECTION_TRAILING_X - 4.0, 0.0, 28.0, theme::FIELD_HEIGHT);
    let stretch_on = cfg.stretch_child_on_insert.unwrap_or(false);
    if let Some(v) = toggle(ui, stretch_ctl, &format!("slot-stretch-on-insert:{prop_name}"), stretch_on) {
        let mut next = cfg.clone();
        next.stretch_child_on_insert = Some(v);
        app.set_slot_config(owner, prop_name, next);
    }
    row_end(pane);

    // Show empty by default
    let empty_lab = pane.rect(theme::SECTION_CONTENT_X, 0.0, 152.0, theme::FIELD_HEIGHT);
    ui.painter().text(
        Pos2::new(empty_lab.left(), empty_lab.center().y),
        Align2::LEFT_CENTER,
        "Show empty by default",
        theme::font_body(),
        theme::TEXT_SECONDARY,
    );
    let empty_ctl = pane.rect(theme::SECTION_TRAILING_X - 4.0, 0.0, 28.0, theme::FIELD_HEIGHT);
    let empty_on = cfg.display_by_default.unwrap_or(false);
    if let Some(v) = toggle(ui, empty_ctl, &format!("slot-show-empty:{prop_name}"), empty_on) {
        let mut next = cfg.clone();
        next.display_by_default = Some(v);
        app.set_slot_config(owner, prop_name, next);
    }
    row_end(pane);
}

/// Link a layer's visibility / text / instance / slot to a component property.
fn layer_bindings(app: &mut App, ui: &mut Ui, pane: &mut Pane, id: NodeId) {
    let Some(comp) = enclosing_component(&app.doc, id) else { return };
    if comp == id {
        return;
    }
    let Some(n) = app.doc.get(id).cloned() else { return };
    let owner = app.prop_owner(comp);
    let defs = prop_defs(&app.doc, comp);
    let is_frame = matches!(n.kind, NodeKind::Frame { .. });
    let bound_slot = n.component.bindings.get(&OverrideField::SlotContent).cloned();
    let is_slot = n.component.is_slot || bound_slot.is_some();

    #[allow(clippy::type_complexity)] // local row table of (label, field, predicate)
    let mut rows: Vec<(&'static str, OverrideField, fn(&PropKind) -> bool)> = vec![("Visibility", OverrideField::Visible, |k| matches!(k, PropKind::Boolean))];
    if matches!(n.kind, NodeKind::Text(_)) {
        rows.push(("Text", OverrideField::Characters, |k| matches!(k, PropKind::Text)));
    }
    if matches!(n.kind, NodeKind::Instance { .. }) {
        rows.push(("Instance", OverrideField::MainComponent, |k| matches!(k, PropKind::InstanceSwap { .. })));
    }
    if is_frame {
        rows.push(("Slot", OverrideField::SlotContent, |k| matches!(k, PropKind::Slot)));
    }
    let can_migrate = can_migrate_region_to_slot(&app.doc, id);
    let rows: Vec<_> = rows.into_iter().filter(|(_, f, accepts)| n.component.bindings.contains_key(f) || defs.iter().any(|d| accepts(&d.kind))).collect();
    if rows.is_empty() && !is_frame && !can_migrate {
        return;
    }
    title(ui, pane, "Component properties", None, false);
    for (label, field, accepts) in rows {
        sublabel(ui, pane, label);
        let cur = n.component.bindings.get(&field).cloned();
        let mut opts = vec!["None".to_string()];
        opts.extend(defs.iter().filter(|d| accepts(&d.kind)).map(|d| d.name.clone()));
        let shown = cur.clone().unwrap_or_else(|| "None".into());
        let r = pane.full(0.0);
        widgets::record(ui, &format!("bind:{label}"), r);
        if let Some(i) = dropdown(ui, r, ("bind", label), &shown, &opts) {
            let prop = (i > 0).then(|| opts[i].clone());
            app.bind_prop(id, field, prop.as_deref());
        }
        row_end(pane);
    }
    if is_frame && !is_slot {
        let btn = pane.full(0.0);
        widgets::record(ui, "convert-to-slot", btn);
        let resp = ui.interact(btn, ui.id().with(("convert-to-slot", id)), Sense::click());
        ui.painter().rect_filled(btn, theme::cr(theme::RADIUS_CONTROL), if resp.hovered() { theme::BG_TERTIARY } else { theme::BG_SECONDARY });
        ui.painter().rect_stroke(
            btn,
            theme::cr(theme::RADIUS_CONTROL),
            egui::Stroke::new(1.0, theme::CANVAS_COMPONENT.gamma_multiply(0.45)),
            egui::StrokeKind::Inside,
        );
        ui.painter().text(btn.center(), Align2::CENTER_CENTER, "Convert to slot", theme::font_body(), theme::TEXT);
        if resp.clicked() {
            app.convert_to_slot(id);
        }
        row_end(pane);
    }
    if can_migrate {
        let btn = pane.full(0.0);
        widgets::record(ui, "migrate-to-slot", btn);
        let resp = ui.interact(btn, ui.id().with("migrate-to-slot"), Sense::click());
        ui.painter().rect_filled(btn, theme::cr(theme::RADIUS_CONTROL), if resp.hovered() { theme::BG_TERTIARY } else { theme::BG_SECONDARY });
        ui.painter().text(btn.center(), Align2::CENTER_CENTER, "Migrate to slot", theme::font_body(), theme::TEXT);
        if resp.clicked() {
            let _ = app.migrate_node_to_slot(id);
        }
        row_end(pane);
    }
    pane.y += theme::SPACE_ROW;
    section_sep(ui, pane, false);

    if is_frame && is_slot {
        let prop_name = bound_slot.or_else(|| defs.iter().find(|d| matches!(d.kind, PropKind::Slot)).map(|d| d.name.clone())).unwrap_or_else(|| n.name.clone());
        title(ui, pane, "Slot settings", None, false);
        slot_settings_rows(app, ui, pane, owner, &prop_name);
        pane.y += theme::SPACE_ROW;
        section_sep(ui, pane, false);
    }
}
