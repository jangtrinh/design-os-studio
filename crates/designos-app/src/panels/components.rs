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

/// 28x16 switch used by the component definition editor.
fn definition_toggle(ui: &mut Ui, rect: Rect, key: &str, on: bool) -> Option<bool> {
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
    if widgets::icon_button(ui, more, "inst-more", Icon::Ellipsis, app.ui.inst_menu.is_some(), "More").clicked() {
        app.ui.inst_menu = if app.ui.inst_menu.is_some() { None } else { Some(more) };
    }
    pane.y += theme::FIELD_HEIGHT + 8.0;
    if let Some(comp) = comp {
        source_line(&app.doc, ui, pane, inst);
        if let Some(description) = description_text(&app.doc, comp) {
            description_row(ui, pane, &description);
        }
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

fn source_line(doc: &Document, ui: &mut Ui, pane: &mut Pane, inst: NodeId) {
    let row = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, 20.0);
    widgets::record(ui, "instance-source", row);
    if let Some(info) = library_instance_info(doc, inst) {
        crate::libraries::instance_library_row(ui, row, &info);
    } else {
        let label = ui.painter().layout_no_wrap("From this file".to_string(), theme::font_body(), theme::TEXT_SECONDARY);
        let label_pos = Pos2::new(row.left(), row.center().y - label.size().y * 0.5);
        let label_right = label_pos.x + label.size().x;
        ui.painter().galley(label_pos, label, theme::TEXT_SECONDARY);
        let icon = Rect::from_min_size(
            Pos2::new(label_right + theme::INSTANCE_SOURCE_ICON_GAP, row.center().y - theme::SPACE_INLINE),
            Vec2::splat(theme::LAYER_ICON_SIZE),
        );
        widgets::paint_icon_in(ui.painter(), icon, Icon::Component, theme::LAYER_ICON_SIZE, theme::TEXT_SECONDARY);
    }
    pane.y += theme::INSTANCE_SOURCE_STEP;
}

fn description_text(doc: &Document, comp: NodeId) -> Option<String> {
    let node = doc.get(comp)?;
    let parent =
        node.parent.and_then(|parent| doc.get(parent)).filter(|p| matches!(p.kind, NodeKind::ComponentSet(_))).map(|p| p.component.description.as_str());
    resolve_description(&node.component.description, parent)
}

fn resolve_description(own: &str, parent: Option<&str>) -> Option<String> {
    let source = if own.trim().is_empty() { parent.unwrap_or("") } else { own };
    let text = source.split_whitespace().collect::<Vec<_>>().join(" ");
    (!text.is_empty()).then_some(text)
}

fn truncate_to_width(line: &str, max_w: f32, measure: impl Fn(&str) -> f32) -> String {
    let mut stem = line.trim_end().to_string();
    let mut result = format!("{stem}…");
    while measure(&result) > max_w && !stem.is_empty() {
        stem.pop();
        while stem.ends_with(' ') {
            stem.pop();
        }
        result = format!("{stem}…");
    }
    result
}

fn truncate_description_lines(lines: &[String], max_w: f32, measure: impl Fn(&str) -> f32) -> (Vec<String>, bool) {
    if lines.len() <= 2 {
        return (lines.to_vec(), false);
    }
    let mut result = lines[..2].to_vec();
    let last = result.last_mut().expect("two description lines");
    *last = truncate_to_width(last, max_w, measure);
    (result, true)
}

fn description_clipped(wrapped_lines: usize, line_h: f32, row_h: f32) -> bool {
    wrapped_lines > 2 || wrapped_lines as f32 * line_h > row_h + 0.5
}

fn description_row(ui: &mut Ui, pane: &mut Pane, text: &str) {
    let row = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, 40.0);
    widgets::record(ui, "instance-description", row);
    let galley = ui.painter().layout(text.to_owned(), theme::font_body(), theme::TEXT, row.width());
    let lines: Vec<String> = galley.rows.iter().map(|line| line.text().to_string()).collect();
    let (shown, truncated) =
        truncate_description_lines(&lines, row.width(), |line| ui.painter().layout_no_wrap(line.to_owned(), theme::font_body(), theme::TEXT).size().x);
    for (index, line) in shown.iter().enumerate() {
        ui.painter().text(Pos2::new(row.left(), row.top() + index as f32 * theme::LINE_BODY), Align2::LEFT_TOP, line, theme::font_body(), theme::TEXT);
    }
    if truncated || description_clipped(galley.rows.len(), theme::LINE_BODY, 40.0) {
        ui.interact(row, ui.id().with("instance-description"), Sense::hover()).on_hover_text(text);
    }
    pane.y += theme::INSTANCE_DESCRIPTION_STEP;
}

fn instance_boolean_toggle(ui: &mut Ui, rect: Rect, key: &str, on: bool) -> egui::Response {
    let track = Rect::from_min_size(Pos2::new(rect.left(), rect.center().y - theme::SPACE_INLINE), Vec2::new(theme::BADGE_LG, theme::LAYER_ICON_SIZE));
    widgets::record(ui, &format!("toggle:{key}"), rect);
    crate::widgets::kit::toggle(ui, track, key, on, true)
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

fn kind_rank(kind: &PropKind) -> u8 {
    match kind {
        PropKind::Variant { .. } => 0,
        PropKind::Boolean => 1,
        PropKind::Text => 2,
        PropKind::InstanceSwap { .. } => 3,
        PropKind::Slot => 4,
    }
}

#[cfg(test)]
mod props_panel_tests {
    use super::*;

    fn def(name: &str, kind: PropKind) -> ComponentPropDef {
        ComponentPropDef { name: name.into(), kind, default: PropValue::Text(String::new()) }
    }

    #[test]
    fn property_kind_rank_matches_panel_order() {
        assert_eq!(kind_rank(&PropKind::Variant { options: vec![] }), 0);
        assert_eq!(kind_rank(&PropKind::Boolean), 1);
        assert_eq!(kind_rank(&PropKind::Text), 2);
        assert_eq!(kind_rank(&PropKind::InstanceSwap { preferred: vec![] }), 3);
        assert_eq!(kind_rank(&PropKind::Slot), 4);
    }

    #[test]
    fn property_sort_is_stable_within_each_kind() {
        let mut defs = vec![
            def("text", PropKind::Text),
            def("bool-a", PropKind::Boolean),
            def("variant-a", PropKind::Variant { options: vec![] }),
            def("bool-b", PropKind::Boolean),
            def("variant-b", PropKind::Variant { options: vec![] }),
        ];
        sort_props_by_kind(&mut defs);
        assert_eq!(defs.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(), ["variant-a", "variant-b", "bool-a", "bool-b", "text"]);
    }

    #[test]
    fn description_resolution_prefers_own_then_set_and_normalizes_whitespace() {
        assert_eq!(resolve_description(" own text ", Some("set text")).as_deref(), Some("own text"));
        assert_eq!(resolve_description("", Some(" set\n fallback ")).as_deref(), Some("set fallback"));
        assert_eq!(resolve_description("", None), None);
        assert_eq!(resolve_description(" \n\t ", Some(" \n ")), None);
        assert_eq!(resolve_description("one\ntwo   three", None).as_deref(), Some("one two three"));
    }

    #[test]
    fn description_truncation_keeps_two_lines_and_marks_last_line() {
        let (lines, truncated) = truncate_description_lines(&["first".into(), "second".into(), "third".into()], 100.0, |line| line.chars().count() as f32);
        assert!(truncated);
        assert_eq!(lines, vec!["first".to_string(), "second…".to_string()]);
        assert_eq!(truncate_description_lines(&["one".into()], 100.0, |line| line.chars().count() as f32), (vec!["one".into()], false));
    }

    #[test]
    fn description_truncation_shrinks_to_fit_width() {
        let truncated = truncate_to_width("second   ", 60.0, |line| line.chars().count() as f32 * 10.0);
        assert_eq!(truncated, "secon…");
        assert!(truncated.chars().count() as f32 * 10.0 <= 60.0);
    }

    #[test]
    fn description_clipping_accounts_for_row_height() {
        assert!(!description_clipped(1, 18.0, 40.0));
        assert!(!description_clipped(2, 18.0, 40.0));
        assert!(description_clipped(3, 18.0, 40.0));
        assert!(description_clipped(2, 22.0, 40.0));
    }

    #[test]
    fn instance_boolean_toggle_shows_focus_ring_on_tab() {
        fn ring_strokes(shapes: &[egui::epaint::ClippedShape]) -> Vec<egui::epaint::RectShape> {
            shapes
                .iter()
                .filter_map(|clipped| match &clipped.shape {
                    egui::epaint::Shape::Rect(shape) if shape.stroke.color == theme::FOCUS_RING && shape.stroke.width == 2.0 => Some(shape.clone()),
                    _ => None,
                })
                .collect()
        }

        let rect = Rect::from_min_size(Pos2::new(20.0, 20.0), Vec2::new(92.0, theme::FIELD_HEIGHT));
        let track = Rect::from_min_size(Pos2::new(rect.left(), rect.center().y - theme::SPACE_INLINE), Vec2::new(theme::BADGE_LG, theme::LAYER_ICON_SIZE));

        let unfocused_ctx = egui::Context::default();
        theme::install_fonts(&unfocused_ctx);
        let mut unfocused_shapes = Vec::new();
        for _ in 0..3 {
            unfocused_shapes = unfocused_ctx
                .run_ui(egui::RawInput::default(), |ui| {
                    instance_boolean_toggle(ui, rect, "instance-bool", false);
                })
                .shapes;
        }
        assert!(ring_strokes(&unfocused_shapes).is_empty(), "unfocused toggle has no focus ring");

        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let first_id = std::cell::Cell::new(egui::Id::NULL);
        let toggle_id = std::cell::Cell::new(egui::Id::NULL);
        let frame = |input: egui::RawInput| {
            ctx.run_ui(input, |ui| {
                ui.push_id("tab-before-instance-bool", |ui| ui.add(egui::Button::new("Before toggle")));
                instance_boolean_toggle(ui, rect, "instance-bool", false);
            })
        };

        // Request focus on frame one, then let egui register and retain the widget for two frames.
        ctx.run_ui(egui::RawInput::default(), |ui| {
            let response = ui.push_id("tab-before-instance-bool", |ui| ui.add(egui::Button::new("Before toggle"))).inner;
            first_id.set(response.id);
            response.request_focus();
            toggle_id.set(instance_boolean_toggle(ui, rect, "instance-bool", false).id);
        });
        frame(egui::RawInput::default());
        frame(egui::RawInput::default());
        assert!(ctx.memory(|m| m.has_focus(first_id.get())), "the first widget holds focus before Tab");

        let before_tab = frame(egui::RawInput::default());
        assert!(ring_strokes(&before_tab.shapes).is_empty(), "toggle has no focus ring before Tab");
        let tab =
            egui::Event::Key { key: egui::Key::Tab, physical_key: Some(egui::Key::Tab), pressed: true, repeat: false, modifiers: egui::Modifiers::default() };
        let release =
            egui::Event::Key { key: egui::Key::Tab, physical_key: Some(egui::Key::Tab), pressed: false, repeat: false, modifiers: egui::Modifiers::default() };
        frame(egui::RawInput { events: vec![tab, release], ..Default::default() });
        let focused = frame(egui::RawInput::default());
        assert!(ctx.memory(|m| m.has_focus(toggle_id.get())), "Tab moves focus to the toggle");
        let rings = ring_strokes(&focused.shapes);
        assert_eq!(rings.len(), 1, "focused toggle has exactly one focus ring");
        assert_eq!(rings[0].rect, track.expand(theme::SPACE_FACT));
        assert_eq!(rings[0].stroke.color, theme::FOCUS_RING);
        assert_eq!(rings[0].stroke.width, 2.0);

        let shift_tab = egui::Event::Key {
            key: egui::Key::Tab,
            physical_key: Some(egui::Key::Tab),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers { shift: true, ..Default::default() },
        };
        let shift_release = egui::Event::Key {
            key: egui::Key::Tab,
            physical_key: Some(egui::Key::Tab),
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers { shift: true, ..Default::default() },
        };
        frame(egui::RawInput { events: vec![shift_tab, shift_release], ..Default::default() });
        let returned = frame(egui::RawInput::default());
        assert!(ctx.memory(|m| m.has_focus(first_id.get())), "Shift+Tab returns focus to the first widget");
        assert!(ring_strokes(&returned.shapes).is_empty(), "toggle ring disappears after Shift+Tab");
    }

    #[test]
    fn properties_panel_swap_preserves_slot_child_overrides_variable_bindings_visibility_name_and_undo() {
        let mut doc = Document::new("SwapOverrides");
        let page = doc.add_page("Page 1");
        let mut app = App::new(doc);
        app.page = page;

        let make_icon = |app: &mut App, name: &str, child_name: &str, x: f32, p0: (f32, f32), p1: (f32, f32)| -> NodeId {
            let id = app.doc.alloc_id();
            let mut f = Node::frame(id, designos_doc::Rect::new(x, 0.0, 16.0, 16.0));
            f.name = name.into();
            let comp = app.doc.insert(page, f).unwrap();
            let gid = app.doc.alloc_id();
            let mut g = Node::new(
                gid,
                NodeKind::Vector {
                    paths: vec![designos_doc::Path {
                        cmds: vec![
                            designos_doc::PathCmd::MoveTo(designos_doc::Vec2::new(p0.0, p0.1)),
                            designos_doc::PathCmd::LineTo(designos_doc::Vec2::new(p1.0, p1.1)),
                        ],
                        ..Default::default()
                    }],
                    network: None,
                },
            );
            g.size = designos_doc::Vec2::new(16.0, 16.0);
            g.name = child_name.into();
            g.fills = vec![Paint::solid(Color::rgb(255, 255, 255))];
            g.variables.bindings.insert("fills/0".into(), "s.color.icon".into());
            app.doc.insert(comp, g).unwrap();
            create_component(&mut app.doc, comp).apply(&mut app.doc).unwrap();
            comp
        };

        let icon_plus = make_icon(&mut app, "Icon/Plus", "Glyph", 0.0, (2.0, 8.0), (14.0, 8.0));
        let icon_check = make_icon(&mut app, "Icon/Check", "Glyph", 40.0, (4.0, 8.0), (12.0, 12.0));
        let icon_mismatch = make_icon(&mut app, "Icon/Mismatch", "OtherPath", 80.0, (1.0, 1.0), (15.0, 15.0));

        let btn = {
            let id = app.doc.alloc_id();
            let mut f = Node::frame(id, designos_doc::Rect::new(0.0, 100.0, 120.0, 40.0));
            f.name = "Button/Primary".into();
            let comp = app.doc.insert(page, f).unwrap();
            create_component(&mut app.doc, comp).apply(&mut app.doc).unwrap();
            create_instance(&mut app.doc, icon_plus, comp, designos_doc::Vec2::new(8.0, 10.0)).apply(&mut app.doc).unwrap();
            let slot = app.doc.get(comp).unwrap().children[0];
            {
                let sn = app.doc.get_mut(slot).unwrap();
                sn.name = "Leading icon".into();
                sn.visible = false;
                sn.size = designos_doc::Vec2::new(20.0, 20.0);
                sn.component.overrides.insert(OverrideField::Size);
            }
            let slot_glyph = app.doc.get(slot).unwrap().children[0];
            {
                let mut paint = Paint::solid(Color::rgb(10, 10, 10));
                paint.opacity = 0.5;
                let gn = app.doc.get_mut(slot_glyph).unwrap();
                let tf = gn.transform;
                designos_doc::layout_constraints::apply_constrained_geometry(gn, tf, designos_doc::Vec2::new(20.0, 20.0));
                gn.fills = vec![paint];
                gn.opacity = 0.5;
                gn.variables.bindings.insert("fills/0".into(), "s.color.text-on-brand".into());
                gn.component.overrides.insert(OverrideField::Size);
                gn.component.overrides.insert(OverrideField::Fills);
                gn.component.overrides.insert(OverrideField::Opacity);
            }
            add_prop(&mut app.doc, comp, ComponentPropDef { name: "Has Leading Icon".into(), kind: PropKind::Boolean, default: PropValue::Bool(false) })
                .apply(&mut app.doc)
                .unwrap();
            add_prop(
                &mut app.doc,
                comp,
                ComponentPropDef {
                    name: "Icon".into(),
                    kind: PropKind::InstanceSwap { preferred: vec![icon_plus, icon_check, icon_mismatch] },
                    default: PropValue::Instance(icon_plus),
                },
            )
            .apply(&mut app.doc)
            .unwrap();
            bind_prop(&mut app.doc, slot, OverrideField::Visible, Some("Has Leading Icon")).apply(&mut app.doc).unwrap();
            bind_prop(&mut app.doc, slot, OverrideField::MainComponent, Some("Icon")).apply(&mut app.doc).unwrap();
            comp
        };

        let expected_scaled_check_cmds =
            vec![designos_doc::PathCmd::MoveTo(designos_doc::Vec2::new(5.0, 10.0)), designos_doc::PathCmd::LineTo(designos_doc::Vec2::new(15.0, 15.0))];

        create_instance(&mut app.doc, btn, page, designos_doc::Vec2::new(200.0, 100.0)).apply(&mut app.doc).unwrap();
        let btn_inst = *app.doc.get(page).unwrap().children.last().unwrap();
        let slot_inst = app.doc.get(btn_inst).unwrap().children[0];
        let before_nodes = app.doc.nodes.clone();

        // 1. Properties panel `set_instance_prop` entry point (used by `instance_props` InstanceSwap dropdown).
        app.set_instance_prop(btn_inst, "Icon", Some(PropValue::Instance(icon_check)));
        let slot_after = app.doc.get(slot_inst).unwrap();
        assert_eq!(instance_component(slot_after), Some(icon_check));
        assert_eq!(slot_after.name, "Leading icon");
        assert_eq!(slot_after.size, designos_doc::Vec2::new(20.0, 20.0));
        assert!(!slot_after.visible);
        assert_eq!(slot_after.component.bindings.get(&OverrideField::Visible).map(String::as_str), Some("Has Leading Icon"));
        let glyph_after = app.doc.get(slot_after.children[0]).unwrap();
        assert_eq!(glyph_after.size, designos_doc::Vec2::new(20.0, 20.0));
        match &glyph_after.kind {
            NodeKind::Vector { paths, .. } => assert_eq!(paths[0].cmds, expected_scaled_check_cmds),
            other => panic!("expected Vector, got {other:?}"),
        }
        assert_eq!(glyph_after.fills[0].kind, PaintKind::Solid(Color::rgb(10, 10, 10)));
        assert_eq!(glyph_after.fills[0].opacity, 0.5);
        assert_eq!(glyph_after.opacity, 0.5);
        assert_eq!(glyph_after.variables.bindings.get("fills/0").map(String::as_str), Some("s.color.text-on-brand"));

        app.undo();
        assert_eq!(app.doc.nodes, before_nodes, "one undo step restores old main and overrides");

        // Mismatched child name drops override via Properties panel path.
        app.set_instance_prop(btn_inst, "Icon", Some(PropValue::Instance(icon_mismatch)));
        let mismatch_child = app.doc.get(app.doc.get(slot_inst).unwrap().children[0]).unwrap();
        assert_eq!(mismatch_child.name, "OtherPath");
        assert_eq!(mismatch_child.fills[0].kind, PaintKind::Solid(Color::rgb(255, 255, 255)));
        assert_eq!(mismatch_child.variables.bindings.get("fills/0").map(String::as_str), Some("s.color.icon"));
        app.undo();
        assert_eq!(app.doc.nodes, before_nodes);

        // 2. Properties panel `swap_instance` entry point (used by `instance_header` swap dropdown).
        app.swap_instance(slot_inst, icon_check);
        let slot_direct = app.doc.get(slot_inst).unwrap();
        assert_eq!(instance_component(slot_direct), Some(icon_check));
        assert_eq!(slot_direct.name, "Leading icon");
        assert_eq!(slot_direct.size, designos_doc::Vec2::new(20.0, 20.0));
        assert!(!slot_direct.visible);
        let glyph_direct = app.doc.get(slot_direct.children[0]).unwrap();
        assert_eq!(glyph_direct.size, designos_doc::Vec2::new(20.0, 20.0));
        match &glyph_direct.kind {
            NodeKind::Vector { paths, .. } => assert_eq!(paths[0].cmds, expected_scaled_check_cmds),
            other => panic!("expected Vector, got {other:?}"),
        }
        assert_eq!(glyph_direct.fills[0].kind, PaintKind::Solid(Color::rgb(10, 10, 10)));
        assert_eq!(glyph_direct.fills[0].opacity, 0.5);
        assert_eq!(glyph_direct.opacity, 0.5);
        assert_eq!(glyph_direct.variables.bindings.get("fills/0").map(String::as_str), Some("s.color.text-on-brand"));

        app.undo();
        assert_eq!(app.doc.nodes, before_nodes);

        // 3. Auto-named top-level instance takes new main's name on swap; custom-named keeps its own name.
        create_instance(&mut app.doc, icon_plus, page, designos_doc::Vec2::new(360.0, 100.0)).apply(&mut app.doc).unwrap();
        let auto_inst = *app.doc.get(page).unwrap().children.last().unwrap();
        assert_eq!(app.doc.get(auto_inst).unwrap().name, "Icon/Plus");
        app.swap_instance(auto_inst, icon_check);
        assert_eq!(app.doc.get(auto_inst).unwrap().name, "Icon/Check", "auto-named instance takes new main's name on panel swap");
        app.undo();
        assert_eq!(app.doc.get(auto_inst).unwrap().name, "Icon/Plus");
    }

    #[test]
    fn swapped_slot_follows_new_main_height_edit_in_app() {
        let mut doc = Document::new("FollowHeight");
        let page = doc.add_page("Page 1");
        let mut app = App::new(doc);
        app.page = page;
        let mkbtn = |app: &mut App, name: &str, x: f32, h: f32| -> NodeId {
            let id = app.doc.alloc_id();
            let mut f = Node::frame(id, designos_doc::Rect::new(x, 300.0, 80.0, h));
            f.name = name.into();
            let comp = app.doc.insert(page, f).unwrap();
            let rid = app.doc.alloc_id();
            let mut r = Node::rectangle(rid, designos_doc::Rect::new(0.0, 0.0, 80.0, h));
            r.name = "bg".into();
            app.doc.insert(comp, r).unwrap();
            create_component(&mut app.doc, comp).apply(&mut app.doc).unwrap();
            comp
        };
        let ba = mkbtn(&mut app, "Btn/A", 0.0, 32.0);
        let bb = mkbtn(&mut app, "Btn/B", 100.0, 40.0);
        let card = {
            let id = app.doc.alloc_id();
            let mut f = Node::frame(id, designos_doc::Rect::new(0.0, 0.0, 100.0, 100.0));
            f.name = "Card".into();
            let comp = app.doc.insert(page, f).unwrap();
            create_component(&mut app.doc, comp).apply(&mut app.doc).unwrap();
            create_instance(&mut app.doc, ba, comp, designos_doc::Vec2::new(10.0, 10.0)).apply(&mut app.doc).unwrap();
            let slot = app.doc.get(comp).unwrap().children[0];
            add_prop(
                &mut app.doc,
                comp,
                ComponentPropDef { name: "Btn".into(), kind: PropKind::InstanceSwap { preferred: vec![ba, bb] }, default: PropValue::Instance(ba) },
            )
            .apply(&mut app.doc)
            .unwrap();
            designos_doc::bind_prop(&mut app.doc, slot, OverrideField::MainComponent, Some("Btn")).apply(&mut app.doc).unwrap();
            comp
        };
        create_instance(&mut app.doc, card, page, designos_doc::Vec2::new(400.0, 0.0)).apply(&mut app.doc).unwrap();
        let ci = *app.doc.get(page).unwrap().children.last().unwrap();
        let si = app.doc.get(ci).unwrap().children[0];
        app.set_instance_prop(ci, "Btn", Some(PropValue::Instance(bb)));
        app.edit_nodes(&[bb], |n| n.size.y = 48.0);
        let s = app.doc.get(si).unwrap();
        assert_eq!(s.size.y, 48.0, "the swapped slot was never resized: it follows its main's height edit");
    }
}

fn sort_props_by_kind(defs: &mut Vec<ComponentPropDef>) {
    defs.sort_by_key(|def| kind_rank(&def.kind));
}

fn instance_props(app: &mut App, ui: &mut Ui, pane: &mut Pane, inst: NodeId) {
    let Some(comp) = app.main_of(inst) else { return };
    let mut defs = prop_defs(&app.doc, comp);
    sort_props_by_kind(&mut defs);
    if defs.is_empty() {
        return;
    }
    for def in defs {
        let value = prop_value(&app.doc, inst, &def);
        let lab = pane.rect(theme::SECTION_CONTENT_X, 0.0, 92.0, theme::FIELD_HEIGHT);
        let is_set = app.doc.get(inst).is_some_and(|n| n.component.values.contains_key(&def.name));
        let shown = widgets::elide(ui.painter(), &def.name, &theme::font_body(), lab.width() - 4.0);
        ui.painter().text(
            Pos2::new(lab.left(), lab.center().y),
            Align2::LEFT_CENTER,
            &shown,
            theme::font_body(),
            if is_set { theme::TEXT } else { theme::TEXT_SECONDARY },
        );
        if shown != def.name {
            ui.interact(lab, ui.id().with(("prop-label", &def.name)), Sense::hover()).on_hover_text(&def.name);
        }
        let ctl = pane.col(1, 0.0);
        let key = format!("prop:{}", def.name);
        widgets::record(ui, &key, ctl);
        match (&def.kind, &value) {
            (PropKind::Boolean, PropValue::Bool(b)) => {
                if instance_boolean_toggle(ui, ctl, &key, *b).clicked() {
                    let v = !*b;
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
    let mut defs: Vec<(NodeId, ComponentPropDef)> =
        app.doc.get(owner).map(|n| n.component.props.iter().cloned().map(|d| (owner, d)).collect()).unwrap_or_default();
    if owner != id {
        if let Some(vn) = app.doc.get(id) {
            for d in &vn.component.props {
                if let Some(pos) = defs.iter().position(|(_, existing)| existing.name == d.name) {
                    defs[pos] = (id, d.clone());
                } else {
                    defs.push((id, d.clone()));
                }
            }
        }
    }
    let plus = pane.rect(theme::SECTION_TRAILING_X, 8.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
    widgets::record(ui, "prop-add", plus);
    if title(ui, pane, "Properties", Some(Icon::Plus), defs.is_empty()) {
        app.ui.prop_menu = if app.ui.prop_menu.is_some() { None } else { Some(plus) };
    }
    for (def_owner, def) in &defs {
        let name_r = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W - 28.0, theme::FIELD_HEIGHT);
        widgets::record(ui, &format!("propdef:{}", def.name), name_r);
        if let Some(new) = field::text_input(ui, name_r, ("propdef", &def.name), &def.name) {
            app.rename_prop_def(*def_owner, &def.name, &new);
        }
        if !matches!(def.kind, PropKind::Variant { .. }) {
            let minus = pane.rect(theme::SECTION_TRAILING_X, 0.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
            widgets::record(ui, &format!("propdel:{}", def.name), minus);
            if widgets::icon_button(ui, minus, ("propdel", &def.name), Icon::Minus, false, "Delete property").clicked() {
                app.delete_prop_def(*def_owner, &def.name);
            }
        }
        row_end(pane);
        let lab = pane.col(0, 0.0);
        ui.painter().text(Pos2::new(lab.left(), lab.center().y), Align2::LEFT_CENTER, kind_label(&def.kind), theme::font_body(), theme::TEXT_SECONDARY);
        let ctl = pane.col(1, 0.0);
        let key = format!("default:{}", def.name);
        match (&def.kind, &def.default) {
            (PropKind::Boolean, PropValue::Bool(b)) => {
                if let Some(v) = definition_toggle(ui, ctl, &key, *b) {
                    app.set_prop_default(*def_owner, &def.name, PropValue::Bool(v));
                }
            }
            (PropKind::Text, PropValue::Text(s)) => {
                if let Some(v) = field::text_input(ui, ctl, &key, s) {
                    app.set_prop_default(*def_owner, &def.name, PropValue::Text(v));
                }
            }
            (PropKind::Variant { options }, PropValue::Variant(cur)) => {
                if let Some(i) = dropdown(ui, ctl, &key, cur, options) {
                    app.set_prop_default(*def_owner, &def.name, PropValue::Variant(options[i].clone()));
                }
            }
            (PropKind::InstanceSwap { .. }, PropValue::Instance(c)) => {
                let cur_label = comp_label(&app.doc, *c);
                if is_dropdown_open(ui, &key) {
                    let comps: Vec<NodeId> = all_components_cached(app).into_iter().filter(|x| *x != *def_owner && *x != owner).collect();
                    let names: Vec<String> = comps.iter().map(|x| comp_label(&app.doc, *x)).collect();
                    if let Some(i) = dropdown(ui, ctl, &key, &cur_label, &names) {
                        app.set_prop_default(*def_owner, &def.name, PropValue::Instance(comps[i]));
                    }
                } else {
                    let _ = dropdown(ui, ctl, &key, &cur_label, &[]);
                }
            }
            (PropKind::Slot, _) => {
                let cfg = app.doc.get(*def_owner).and_then(|n| n.component.slot_configs.get(&def.name)).cloned().unwrap_or_default();
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
            slot_settings_rows(app, ui, pane, *def_owner, &def.name);
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
    if let Some(v) = definition_toggle(ui, pref_ctl, &format!("slot-preferred-only:{prop_name}"), pref_on) {
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
    if let Some(v) = definition_toggle(ui, stretch_ctl, &format!("slot-stretch-on-insert:{prop_name}"), stretch_on) {
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
    if let Some(v) = definition_toggle(ui, empty_ctl, &format!("slot-show-empty:{prop_name}"), empty_on) {
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
        let slot_owner = app.prop_def_owner(comp, &prop_name);
        title(ui, pane, "Slot settings", None, false);
        slot_settings_rows(app, ui, pane, slot_owner, &prop_name);
        pane.y += theme::SPACE_ROW;
        section_sep(ui, pane, false);
    }
}
