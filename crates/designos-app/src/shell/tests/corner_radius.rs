//! Unit tests for #1466: individual corner radius fields and corner smoothing slider.

use super::*;
use designos_doc::{VariableKind, VariableValue};

fn has_field(s: &Sim, key: &str) -> bool {
    s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("field-rect", format!("{key:?}"))))).is_some()
}

fn has_test_rect(s: &Sim, name: &str) -> bool {
    s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", name.to_string())))).is_some()
}

fn collect_texts_in_rect(shape: &egui::epaint::Shape, rect: Rect, out: &mut Vec<String>) {
    match shape {
        egui::epaint::Shape::Text(t) => {
            if rect.contains(t.pos) {
                out.push(t.galley.text().to_string());
            }
        }
        egui::epaint::Shape::Vec(shapes) => {
            for s in shapes {
                collect_texts_in_rect(s, rect, out);
            }
        }
        _ => {}
    }
}

fn texts_in_rect(s: &Sim, rect: Rect) -> Vec<String> {
    let mut out = Vec::new();
    for cs in &s.shapes {
        collect_texts_in_rect(&cs.shape, rect, &mut out);
    }
    out
}

fn edit_num_field(s: &mut Sim, key: &str, val: &str) {
    let r = s.field_rect(key);
    s.click(Pos2::new(r.left() + 40.0, r.center().y));
    s.type_text(val);
    s.key(Key::Enter);
}

#[test]
fn toggle_on_shows_four_fields_in_figma_order() {
    let mut s = Sim::with(Options { select: Some("Tile A".into()), ..Default::default() });
    let tile_a = s.by_name("Tile A");
    let tile_b = s.by_name("Tile B");
    assert_eq!(s.app().doc.get(tile_a).unwrap().corner_radii, [12.0; 4]);

    // Uniform corners start with the toggle off: one "radius" field, no per-corner fields.
    assert!(has_field(&s, "radius"));
    for key in ["radius-tl", "radius-tr", "radius-br", "radius-bl"] {
        assert!(!has_field(&s, key), "{key} should be hidden when toggle is off");
    }

    // Toggle on: four fields appear in Figma's 2x2 order (top-left, top-right, bottom-right, bottom-left).
    let toggle = s.rect_of("corner-individual");
    s.click(toggle.center());
    assert!(!has_field(&s, "radius"));
    let tl = s.field_rect("radius-tl");
    let tr = s.field_rect("radius-tr");
    let br = s.field_rect("radius-br");
    let bl = s.field_rect("radius-bl");
    assert!((tl.top() - tr.top()).abs() < 1.0, "top-left and top-right share top row: {tl:?} vs {tr:?}");
    assert!(tl.left() < tr.left(), "top-left is left of top-right: {tl:?} vs {tr:?}");
    assert!((br.top() - bl.top()).abs() < 1.0, "bottom-right and bottom-left share bottom row: {br:?} vs {bl:?}");
    assert!(br.top() > tl.bottom(), "bottom row is below top row: {tl:?} vs {br:?}");
    assert!(br.left() < bl.left(), "bottom-right is left of bottom-left in Figma order: {br:?} vs {bl:?}");

    // Toggle state follows selection (on when corners differ, off when uniform) and is view state, not doc data.
    s.shell.app.edit_nodes(&[tile_b], |n| n.corner_radii = [4.0, 8.0, 12.0, 16.0]);
    s.shell.app.selection = vec![tile_b];
    s.frame(vec![]);
    assert!(has_field(&s, "radius-tl"), "selecting non-uniform Tile B keeps toggle on");
    assert!(texts_in_rect(&s, s.field_rect("radius-tl")).contains(&"4".to_string()));
    assert!(texts_in_rect(&s, s.field_rect("radius-tr")).contains(&"8".to_string()));
    assert!(texts_in_rect(&s, s.field_rect("radius-br")).contains(&"12".to_string()));
    assert!(texts_in_rect(&s, s.field_rect("radius-bl")).contains(&"16".to_string()));

    // Turning the toggle off on a node whose corners differ shows "Mixed" in the single field without mutating doc.
    let toggle = s.rect_of("corner-individual");
    s.click(toggle.center());
    assert!(has_field(&s, "radius"));
    assert!(texts_in_rect(&s, s.field_rect("radius")).contains(&"Mixed".to_string()));
    assert_eq!(s.app().doc.get(tile_b).unwrap().corner_radii, [4.0, 8.0, 12.0, 16.0]);

    // Selecting Tile B again after switching to uniform Tile A automatically turns the toggle back on.
    s.shell.app.selection = vec![tile_a];
    s.frame(vec![]);
    assert!(has_field(&s, "radius"), "selecting uniform Tile A switches toggle off");
    s.shell.app.selection = vec![tile_b];
    s.frame(vec![]);
    assert!(has_field(&s, "radius-tl"), "selecting non-uniform Tile B switches toggle on");
}

#[test]
fn per_corner_edit_changes_only_that_corner() {
    let mut s = Sim::with(Options { select: Some("Tile A".into()), ..Default::default() });
    let tile_a = s.by_name("Tile A");
    assert_eq!(s.app().doc.get(tile_a).unwrap().corner_radii, [12.0, 12.0, 12.0, 12.0]);

    let toggle = s.rect_of("corner-individual");
    s.click(toggle.center());

    edit_num_field(&mut s, "radius-tl", "4");
    assert_eq!(s.app().doc.get(tile_a).unwrap().corner_radii, [4.0, 12.0, 12.0, 12.0], "editing top-left must change only corner_radii[0]");

    edit_num_field(&mut s, "radius-tr", "8");
    assert_eq!(s.app().doc.get(tile_a).unwrap().corner_radii, [4.0, 8.0, 12.0, 12.0], "editing top-right must change only corner_radii[1]");

    edit_num_field(&mut s, "radius-br", "16");
    assert_eq!(s.app().doc.get(tile_a).unwrap().corner_radii, [4.0, 8.0, 16.0, 12.0], "editing bottom-right must change only corner_radii[2]");

    edit_num_field(&mut s, "radius-bl", "20");
    assert_eq!(s.app().doc.get(tile_a).unwrap().corner_radii, [4.0, 8.0, 16.0, 20.0], "editing bottom-left must change only corner_radii[3]");

    // Each field commit is one undo step.
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(tile_a).unwrap().corner_radii, [4.0, 8.0, 16.0, 12.0]);
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(tile_a).unwrap().corner_radii, [4.0, 8.0, 12.0, 12.0]);
}

#[test]
fn mixed_selection_shows_mixed_and_edit_writes_every_node() {
    let mut s = Sim::new();
    let tile_a = s.by_name("Tile A");
    let tile_b = s.by_name("Tile B");
    let text_node = s.by_name("Welcome back");

    // Same top-left (16) but differing top-right (8 vs 12): uniform radius must still be "Mixed".
    s.shell.app.edit_nodes(&[tile_a], |n| n.corner_radii = [16.0, 8.0, 16.0, 16.0]);
    s.shell.app.edit_nodes(&[tile_b], |n| n.corner_radii = [16.0, 12.0, 16.0, 16.0]);
    s.shell.app.selection = vec![tile_a, tile_b, text_node];
    s.frame(vec![]);

    // Because corners differ on the selection, Individual corners starts on:
    // top-left is 16 across both corner-bearing nodes, while top-right differs ("Mixed").
    assert!(has_field(&s, "radius-tl"));
    assert!(texts_in_rect(&s, s.field_rect("radius-tl")).contains(&"16".to_string()));
    assert!(texts_in_rect(&s, s.field_rect("radius-tr")).contains(&"Mixed".to_string()));

    // Editing the mixed top-right field writes to every selected corner-bearing node in one undo step.
    edit_num_field(&mut s, "radius-tr", "24");
    assert_eq!(s.app().doc.get(tile_a).unwrap().corner_radii, [16.0, 24.0, 16.0, 16.0]);
    assert_eq!(s.app().doc.get(tile_b).unwrap().corner_radii, [16.0, 24.0, 16.0, 16.0]);
    assert_eq!(s.app().doc.get(text_node).unwrap().corner_radii, [0.0; 4]);
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(tile_a).unwrap().corner_radii, [16.0, 8.0, 16.0, 16.0]);
    assert_eq!(s.app().doc.get(tile_b).unwrap().corner_radii, [16.0, 12.0, 16.0, 16.0]);

    // Turn Individual corners off: the single field shows "Mixed", and editing it writes all four corners on every node.
    let toggle = s.rect_of("corner-individual");
    s.click(toggle.center());
    assert!(texts_in_rect(&s, s.field_rect("radius")).contains(&"Mixed".to_string()));
    edit_num_field(&mut s, "radius", "10");
    assert_eq!(s.app().doc.get(tile_a).unwrap().corner_radii, [10.0; 4]);
    assert_eq!(s.app().doc.get(tile_b).unwrap().corner_radii, [10.0; 4]);
    assert_eq!(s.app().doc.get(text_node).unwrap().corner_radii, [0.0; 4]);
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(tile_a).unwrap().corner_radii, [16.0, 8.0, 16.0, 16.0]);
    assert_eq!(s.app().doc.get(tile_b).unwrap().corner_radii, [16.0, 12.0, 16.0, 16.0]);
}

#[test]
fn smoothing_slider_writes_0_6_at_ios_preset_and_one_drag_is_one_undo_step() {
    let mut s = Sim::with(Options { select: Some("Tile A".into()), ..Default::default() });
    let tile_a = s.by_name("Tile A");
    assert_eq!(s.app().doc.get(tile_a).unwrap().corner_smoothing, 0.0);

    // Clicking the "iOS" preset writes 0.6 (60%) in one undo step.
    let ios_btn = s.rect_of("corner-smoothing-ios");
    s.click(ios_btn.center());
    assert!((s.app().doc.get(tile_a).unwrap().corner_smoothing - 0.6).abs() < 1e-6);
    assert!(texts_in_rect(&s, s.field_rect("corner-smoothing")).contains(&"60".to_string()));
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(tile_a).unwrap().corner_smoothing, 0.0);

    // Dragging the smoothing slider across multiple frames commits a single undo step from press to release.
    let slider = s.rect_of("corner-smoothing-slider");
    let from = Pos2::new(slider.left() + slider.width() * 0.1, slider.center().y);
    let to = Pos2::new(slider.left() + slider.width() * 0.8, slider.center().y);
    s.drag(from, to);
    let after_drag = s.app().doc.get(tile_a).unwrap().corner_smoothing;
    assert!((after_drag - 0.8).abs() < 0.08, "expected ~0.8 after slider drag, got {after_drag}");

    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(tile_a).unwrap().corner_smoothing, 0.0, "one slider drag from press to release must be a single undo step");
    s.cmd_key(Key::Y);
    assert!((s.app().doc.get(tile_a).unwrap().corner_smoothing - after_drag).abs() < 1e-6);

    // Text nodes do not have corners and do not show the smoothing controls.
    let text_node = s.by_name("Welcome back");
    s.shell.app.selection = vec![text_node];
    s.frame(vec![]);
    assert!(!has_test_rect(&s, "corner-smoothing-slider"));
    assert!(!has_field(&s, "corner-smoothing"));
}

#[test]
fn binding_corner_field_to_variable_resolves_and_survives_save_and_reopen() {
    let mut s = Sim::with(Options { select: Some("Tile A".into()), ..Default::default() });
    let tile_a = s.by_name("Tile A");
    assert_eq!(s.app().doc.get(tile_a).unwrap().corner_radii, [12.0; 4]);

    let coll_id = s.app().doc.variable_collections.values().find(|c| c.name == "Theme").unwrap().id.clone();
    let mode_id = s.app().doc.variable_collections[&coll_id].default_mode.clone();
    let mut store = designos_doc::VariableStore::from_doc(&s.app().doc);
    let md_id = store.add_variable(&coll_id, "Radius/md", VariableKind::Number).unwrap();
    store.set_value(&md_id, &mode_id, VariableValue::Number(20.0));
    s.shell.app.commit_store(store, &[], &[]);
    s.frame(vec![]);

    // Turn on Individual corners and bind only topRightRadius to Radius/md (20.0).
    let toggle = s.rect_of("corner-individual");
    s.click(toggle.center());
    let bind_tr = s.rect_of("bind:topRightRadius");
    s.click(bind_tr.center());
    s.frame(vec![]);
    let pick_row = s.rect_of("pick:Radius/md");
    s.click(pick_row.center());

    let n = s.app().doc.get(tile_a).unwrap();
    assert_eq!(n.variables.bindings.get("topRightRadius").map(String::as_str), Some(md_id.as_str()));
    assert!(!n.variables.bindings.contains_key("topLeftRadius"));
    assert_eq!(n.corner_radii, [12.0, 20.0, 12.0, 12.0]);

    // Save to disk and reopen: the variable binding and resolved per-corner radius survive round-trip.
    let tmp = std::env::temp_dir().join(format!("designos_corner_var_{}.designos", std::process::id()));
    let saved = crate::io::save_path(&s.app().doc, &tmp).expect("save doc");
    let mut s_reopened = Sim::with(Options { file: Some(saved.clone()), select: Some("Tile A".into()), ..Default::default() });
    std::fs::remove_file(&saved).ok();

    let tile_a_reopened = s_reopened.by_name("Tile A");
    let n_reopened = s_reopened.app().doc.get(tile_a_reopened).unwrap();
    assert_eq!(n_reopened.variables.bindings.get("topRightRadius").map(String::as_str), Some(md_id.as_str()));
    assert_eq!(n_reopened.corner_radii, [12.0, 20.0, 12.0, 12.0]);

    // Updating the bound variable updates the bound corner on the reopened document.
    let mut store = designos_doc::VariableStore::from_doc(&s_reopened.app().doc);
    assert!(store.set_value(&md_id, &mode_id, VariableValue::Number(28.0)));
    s_reopened.shell.app.commit_store(store, &[], &[]);
    assert_eq!(s_reopened.app().doc.get(tile_a_reopened).unwrap().corner_radii, [12.0, 28.0, 12.0, 12.0]);
}
