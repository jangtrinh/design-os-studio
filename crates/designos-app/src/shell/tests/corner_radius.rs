//! Unit tests for individual corner radius fields and corner smoothing slider.

use super::*;
use designos_doc::{FillRule, FrameProps, Node, NodeKind, Path, PathCmd, VariableKind, VariableValue, Vec2 as DocVec2};

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

fn collect_text_spans_in_rect(shape: &egui::epaint::Shape, rect: Rect, out: &mut Vec<(String, f32, f32)>) {
    match shape {
        egui::epaint::Shape::Text(t) => {
            if rect.contains(t.pos) {
                out.push((t.galley.text().to_string(), t.pos.x, t.pos.x + t.galley.size().x));
            }
        }
        egui::epaint::Shape::Vec(shapes) => {
            for s in shapes {
                collect_text_spans_in_rect(s, rect, out);
            }
        }
        _ => {}
    }
}

fn text_spans_in_rect(s: &Sim, rect: Rect) -> Vec<(String, f32, f32)> {
    let mut out = Vec::new();
    for cs in &s.shapes {
        collect_text_spans_in_rect(&cs.shape, rect, &mut out);
    }
    out
}

fn shape_has_circle_in_rect(shape: &egui::epaint::Shape, rect: Rect) -> bool {
    match shape {
        egui::epaint::Shape::Circle(c) => rect.expand(2.0).contains(c.center),
        egui::epaint::Shape::Vec(shapes) => shapes.iter().any(|s| shape_has_circle_in_rect(s, rect)),
        _ => false,
    }
}

fn has_circle_in_rect(s: &Sim, rect: Rect) -> bool {
    s.shapes.iter().any(|cs| shape_has_circle_in_rect(&cs.shape, rect))
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

    // Toggle on: each field sits on the corner it edits (top row TL then TR, bottom row BL then BR).
    let toggle = s.rect_of("corner-individual");
    s.click(toggle.center());
    assert!(!has_field(&s, "radius"));
    let tl = s.field_rect("radius-tl");
    let tr = s.field_rect("radius-tr");
    let bl = s.field_rect("radius-bl");
    let br = s.field_rect("radius-br");
    assert!((tl.top() - tr.top()).abs() < 1.0, "top-left and top-right share top row: {tl:?} vs {tr:?}");
    assert!(tl.left() < tr.left(), "top-left is left of top-right: {tl:?} vs {tr:?}");
    assert!((bl.top() - br.top()).abs() < 1.0, "bottom-left and bottom-right share bottom row: {bl:?} vs {br:?}");
    assert!(bl.left() < br.left(), "bottom-left is left of bottom-right: {bl:?} vs {br:?}");
    assert!(tl.bottom() < bl.top(), "top-left is above bottom-left: {tl:?} vs {bl:?}");
    assert!(tr.bottom() < br.top(), "top-right is above bottom-right: {tr:?} vs {br:?}");
    assert!((tl.left() - bl.left()).abs() < 1.0, "top-left and bottom-left share left column: {tl:?} vs {bl:?}");
    assert!((tr.left() - br.left()).abs() < 1.0, "top-right and bottom-right share right column: {tr:?} vs {br:?}");

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

    // Each of the four field commits is one undo step.
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(tile_a).unwrap().corner_radii, [4.0, 8.0, 16.0, 12.0]);
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(tile_a).unwrap().corner_radii, [4.0, 8.0, 12.0, 12.0]);
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(tile_a).unwrap().corner_radii, [4.0, 12.0, 12.0, 12.0]);
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(tile_a).unwrap().corner_radii, [12.0, 12.0, 12.0, 12.0]);
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

#[test]
fn selection_of_129_rectangles_is_edited_by_one_commit_and_restored_by_one_undo() {
    let mut s = Sim::new();
    let page = s.app().page;
    let mut rect_ids = Vec::with_capacity(129);
    for i in 0..129 {
        let id = s.shell.app.doc.alloc_id();
        let mut n = Node::new(id, NodeKind::Rectangle);
        n.name = format!("BulkRect {i}");
        n.size = DocVec2::new(40.0, 40.0);
        n.corner_radii = [6.0; 4];
        s.shell.app.doc.insert(page, n).unwrap();
        rect_ids.push(id);
    }
    s.shell.app.selection = rect_ids.clone();
    s.frame(vec![]);

    assert!(has_field(&s, "radius"));
    edit_num_field(&mut s, "radius", "18");

    for (idx, &id) in rect_ids.iter().enumerate() {
        assert_eq!(s.app().doc.get(id).unwrap().corner_radii, [18.0; 4], "rectangle at index {idx} must be updated by the uniform radius edit");
    }

    s.cmd_key(Key::Z);
    for (idx, &id) in rect_ids.iter().enumerate() {
        assert_eq!(s.app().doc.get(id).unwrap().corner_radii, [6.0; 4], "rectangle at index {idx} must be restored by a single undo step");
    }
}

#[test]
fn selection_beyond_128_reports_mixed_values_for_opacity_and_radius() {
    let mut s = Sim::new();
    let page = s.app().page;
    let mut rect_ids = Vec::with_capacity(129);
    for i in 0..129 {
        let id = s.shell.app.doc.alloc_id();
        let mut n = Node::new(id, NodeKind::Rectangle);
        n.name = format!("MixedRect {i}");
        n.size = DocVec2::new(40.0, 40.0);
        n.opacity = if i < 128 { 0.5 } else { 0.75 };
        n.corner_radii = if i < 128 { [6.0; 4] } else { [8.0; 4] };
        s.shell.app.doc.insert(page, n).unwrap();
        rect_ids.push(id);
    }
    s.shell.app.selection = rect_ids.clone();
    s.frame(vec![]);

    assert!(has_field(&s, "opacity"));
    assert!(texts_in_rect(&s, s.field_rect("opacity")).contains(&"Mixed".to_string()), "opacity must reflect all selected layers");
    assert!(has_field(&s, "radius"));
    assert!(texts_in_rect(&s, s.field_rect("radius")).contains(&"Mixed".to_string()), "radius must reflect all selected layers");

    edit_num_field(&mut s, "opacity", "25");
    edit_num_field(&mut s, "radius", "18");
    for &id in &rect_ids {
        let n = s.app().doc.get(id).unwrap();
        assert!((n.opacity - 0.25).abs() < 1e-6);
        assert_eq!(n.corner_radii, [18.0; 4]);
    }

    s.cmd_key(Key::Z);
    s.cmd_key(Key::Z);
    for (idx, &id) in rect_ids.iter().enumerate() {
        let n = s.app().doc.get(id).unwrap();
        let expected_opacity = if idx < 128 { 0.5 } else { 0.75 };
        let expected_radius = if idx < 128 { [6.0; 4] } else { [8.0; 4] };
        assert!((n.opacity - expected_opacity).abs() < 1e-6);
        assert_eq!(n.corner_radii, expected_radius);
    }
}

/// Every frame of an inspector capture installs a fresh copy of one base document and inserts that frame's
/// layers, so the frames share layer ids and edit counts while their values differ. The inspector must show the
/// values of the frame it is drawing, not the ones of an earlier frame.
#[test]
fn inspector_capture_frames_show_their_own_width_and_radius() {
    let mut s = Sim::new();
    let base = s.app().doc.clone();
    let total = 10;
    let mut seen = Vec::new();
    for cur in [0usize, total - 1] {
        let _ = crate::shell::apply_micro_interaction_continuous_frame(&mut s.shell.app, &base, "micro-inspector-math", cur, total);
        s.frame(vec![]);
        s.frame(vec![]);
        let id = s.app().selection[0];
        let n = s.app().doc.get(id).unwrap();
        let (width, radius) = (n.size.x, n.corner_radii[0]);
        let shown_width = texts_in_rect(&s, s.field_rect("w"));
        let shown_radius = texts_in_rect(&s, s.field_rect("radius"));
        assert!(shown_width.contains(&format!("{width}")), "frame {cur}: the W field {shown_width:?} must show the document width {width}");
        assert!(shown_radius.contains(&format!("{radius}")), "frame {cur}: the radius field {shown_radius:?} must show the document radius {radius}");
        seen.push((width, radius));
    }
    assert_ne!(seen[0], seen[1], "the first and the last frame must differ, or the test proves nothing: {seen:?}");
}

#[test]
fn selection_of_128_text_layers_plus_one_frame_shows_radius_row_and_edits_frame() {
    let mut s = Sim::new();
    let page = s.app().page;
    let mut sel_ids = Vec::with_capacity(129);
    for i in 0..128 {
        let id = s.shell.app.doc.alloc_id();
        let mut n = Node::new(id, NodeKind::Text(Default::default()));
        n.name = format!("BulkText {i}");
        n.size = DocVec2::new(60.0, 20.0);
        s.shell.app.doc.insert(page, n).unwrap();
        sel_ids.push(id);
    }
    let frame_id = s.shell.app.doc.alloc_id();
    let mut frame_node = Node::new(frame_id, NodeKind::Frame(FrameProps::default()));
    frame_node.name = "TrailingFrame".into();
    frame_node.size = DocVec2::new(120.0, 80.0);
    frame_node.corner_radii = [8.0; 4];
    s.shell.app.doc.insert(page, frame_node).unwrap();
    sel_ids.push(frame_id);

    s.shell.app.selection = sel_ids.clone();
    s.frame(vec![]);

    assert!(has_field(&s, "radius"), "selection of 128 text layers plus one frame must show the radius row");
    assert!(texts_in_rect(&s, s.field_rect("radius")).contains(&"8".to_string()));

    edit_num_field(&mut s, "radius", "22");
    assert_eq!(s.app().doc.get(frame_id).unwrap().corner_radii, [22.0; 4]);
    for &tid in &sel_ids[..128] {
        assert_eq!(s.app().doc.get(tid).unwrap().corner_radii, [0.0; 4]);
    }
}

#[test]
fn star_shows_single_field_only_and_ellipse_and_section_show_no_row() {
    let mut s = Sim::new();
    let page = s.app().page;

    let star_id = s.shell.app.doc.alloc_id();
    let mut star = Node::new(star_id, NodeKind::Star { count: 5, inner_ratio: 0.5 });
    star.name = "TestStar".into();
    star.size = DocVec2::new(80.0, 80.0);
    star.corner_radii = [6.0, 10.0, 14.0, 18.0];
    s.shell.app.doc.insert(page, star).unwrap();

    let poly_id = s.shell.app.doc.alloc_id();
    let mut poly = Node::new(poly_id, NodeKind::Polygon { count: 6 });
    poly.name = "TestPoly".into();
    poly.size = DocVec2::new(80.0, 80.0);
    poly.corner_radii = [4.0; 4];
    s.shell.app.doc.insert(page, poly).unwrap();

    let ellipse_id = s.shell.app.doc.alloc_id();
    let mut ellipse = Node::new(ellipse_id, NodeKind::Ellipse(Default::default()));
    ellipse.name = "TestEllipse".into();
    ellipse.size = DocVec2::new(80.0, 80.0);
    s.shell.app.doc.insert(page, ellipse).unwrap();

    let section_id = s.shell.app.doc.alloc_id();
    let mut section = Node::new(section_id, NodeKind::Section);
    section.name = "TestSection".into();
    section.size = DocVec2::new(200.0, 200.0);
    s.shell.app.doc.insert(page, section).unwrap();

    // Star shows the single radius field only: no Individual corners toggle, no four corner fields, no smoothing row.
    s.shell.app.selection = vec![star_id];
    s.shell.app.props.individual_corners = true;
    s.frame(vec![]);
    assert!(has_field(&s, "radius"), "star must show the single radius field");
    assert!(!has_test_rect(&s, "corner-individual"), "star must not show the Individual corners toggle");
    for key in ["radius-tl", "radius-tr", "radius-bl", "radius-br"] {
        assert!(!has_field(&s, key), "star must not show per-corner field {key}");
    }
    assert!(!has_test_rect(&s, "corner-smoothing-slider"), "star must not show smoothing slider");
    assert!(!has_field(&s, "corner-smoothing"), "star must not show smoothing field");
    assert!(!has_test_rect(&s, "corner-smoothing-ios"), "star must not show smoothing iOS button");
    edit_num_field(&mut s, "radius", "10");
    assert_eq!(s.app().doc.get(star_id).unwrap().corner_radii[0], 10.0);

    // Polygon also shows the single radius field only.
    s.shell.app.selection = vec![poly_id];
    s.frame(vec![]);
    assert!(has_field(&s, "radius"), "polygon must show the single radius field");
    assert!(!has_test_rect(&s, "corner-individual"), "polygon must not show the Individual corners toggle");
    for key in ["radius-tl", "radius-tr", "radius-bl", "radius-br"] {
        assert!(!has_field(&s, key), "polygon must not show per-corner field {key}");
    }
    assert!(!has_test_rect(&s, "corner-smoothing-slider"), "polygon must not show smoothing slider");
    assert!(!has_field(&s, "corner-smoothing"), "polygon must not show smoothing field");

    // Ellipse shows no radius row at all.
    s.shell.app.selection = vec![ellipse_id];
    s.frame(vec![]);
    assert!(!has_field(&s, "radius"), "ellipse must not show radius field");
    assert!(!has_test_rect(&s, "corner-individual"), "ellipse must not show Individual corners toggle");
    assert!(!has_field(&s, "radius-tl"), "ellipse must not show per-corner fields");
    assert!(!has_field(&s, "corner-smoothing"), "ellipse must not show smoothing field");

    // Section shows no radius row at all.
    s.shell.app.selection = vec![section_id];
    s.frame(vec![]);
    assert!(!has_field(&s, "radius"), "section must not show radius field");
    assert!(!has_test_rect(&s, "corner-individual"), "section must not show Individual corners toggle");
    assert!(!has_field(&s, "radius-tl"), "section must not show per-corner fields");
    assert!(!has_field(&s, "corner-smoothing"), "section must not show smoothing field");
}

#[test]
fn bind_only_top_right_turn_toggle_off_and_type_value_detaches_all_four_keys() {
    let mut s = Sim::with(Options { select: Some("Tile A".into()), ..Default::default() });
    let tile_a = s.by_name("Tile A");

    let coll_id = s.app().doc.variable_collections.values().find(|c| c.name == "Theme").unwrap().id.clone();
    let mode_id = s.app().doc.variable_collections[&coll_id].default_mode.clone();
    let mut store = designos_doc::VariableStore::from_doc(&s.app().doc);
    let md_id = store.add_variable(&coll_id, "Radius/md", VariableKind::Number).unwrap();
    store.set_value(&md_id, &mode_id, VariableValue::Number(20.0));
    s.shell.app.commit_store(store, &[], &[]);
    s.frame(vec![]);

    // Bind only top-right while Individual corners is on.
    let toggle = s.rect_of("corner-individual");
    s.click(toggle.center());
    let bind_tr = s.rect_of("bind:topRightRadius");
    s.click(bind_tr.center());
    s.frame(vec![]);
    let pick_row = s.rect_of("pick:Radius/md");
    s.click(pick_row.center());
    assert_eq!(s.app().doc.get(tile_a).unwrap().variables.bindings.get("topRightRadius").map(String::as_str), Some(md_id.as_str()));

    // Turn the toggle off: the uniform field shows the bound pill when any of the four corner keys is bound,
    // and typing a value detaches all four corner keys.
    let toggle = s.rect_of("corner-individual");
    s.click(toggle.center());
    let pill_shown = has_test_rect(&s, "bound:topLeftRadius");
    edit_num_field(&mut s, "radius", "10");

    let n = s.app().doc.get(tile_a).unwrap();
    for key in ["topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"] {
        assert!(!n.variables.bindings.contains_key(key), "{key} must be detached after typing a uniform radius value");
    }
    assert_eq!(n.corner_radii, [10.0; 4]);
    assert!(pill_shown, "uniform field must show the bound pill when topRightRadius is bound");

    // Also verify when all four corner keys are bound: typing a uniform value detaches all four keys.
    s.shell.app.bind_variable(&[tile_a], &["topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"], &md_id);
    s.frame(vec![]);
    assert!(has_test_rect(&s, "bound:topLeftRadius"));
    edit_num_field(&mut s, "radius", "14");
    let n = s.app().doc.get(tile_a).unwrap();
    for key in ["topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"] {
        assert!(!n.variables.bindings.contains_key(key), "{key} must be detached after typing a uniform radius value");
    }
    assert_eq!(n.corner_radii, [14.0; 4]);
}

#[test]
fn mixed_smoothing_draws_no_thumb_and_drag_writes_every_corner_node_in_one_undo_step() {
    let mut s = Sim::new();
    let tile_a = s.by_name("Tile A");
    let tile_b = s.by_name("Tile B");
    s.shell.app.edit_nodes(&[tile_a], |n| n.corner_smoothing = 0.0);
    s.shell.app.edit_nodes(&[tile_b], |n| n.corner_smoothing = 0.6);
    s.shell.app.selection = vec![tile_a, tile_b];
    s.frame(vec![]);

    assert!(texts_in_rect(&s, s.field_rect("corner-smoothing")).contains(&"Mixed".to_string()));
    let slider = s.rect_of("corner-smoothing-slider");
    assert!(!has_circle_in_rect(&s, slider), "mixed smoothing must not draw a slider thumb");

    let from = Pos2::new(slider.left() + slider.width() * 0.2, slider.center().y);
    let to = Pos2::new(slider.left() + slider.width() * 0.7, slider.center().y);
    s.drag(from, to);

    let sa = s.app().doc.get(tile_a).unwrap().corner_smoothing;
    let sb = s.app().doc.get(tile_b).unwrap().corner_smoothing;
    assert!((sa - sb).abs() < 1e-6, "drag must write the same smoothing to every corner node: {sa} vs {sb}");
    assert!((sa - 0.7).abs() < 0.08, "expected smoothing near 0.7 after drag, got {sa}");
    assert!(has_circle_in_rect(&s, s.rect_of("corner-smoothing-slider")), "uniform smoothing after drag draws the thumb");

    s.cmd_key(Key::Z);
    assert!((s.app().doc.get(tile_a).unwrap().corner_smoothing - 0.0).abs() < 1e-6);
    assert!((s.app().doc.get(tile_b).unwrap().corner_smoothing - 0.6).abs() < 1e-6);
}

#[test]
fn component_set_with_imported_geometry_clears_it_on_radius_edit() {
    let mut s = Sim::new();
    let page = s.app().page;
    let cs_id = s.shell.app.doc.alloc_id();
    let mut cs = Node::new(cs_id, NodeKind::ComponentSet(FrameProps::default()));
    cs.name = "ButtonSet".into();
    cs.size = DocVec2::new(200.0, 100.0);
    cs.corner_radii = [8.0; 4];
    let dummy_path = Path {
        cmds: vec![
            PathCmd::MoveTo(DocVec2::new(0.0, 0.0)),
            PathCmd::LineTo(DocVec2::new(200.0, 0.0)),
            PathCmd::LineTo(DocVec2::new(200.0, 100.0)),
            PathCmd::Close,
        ],
        fill_rule: FillRule::NonZero,
    };
    cs.fill_geometry = Some(vec![dummy_path.clone()]);
    cs.stroke_geometry = Some(vec![dummy_path]);
    s.shell.app.doc.insert(page, cs).unwrap();

    s.shell.app.selection = vec![cs_id];
    s.frame(vec![]);

    assert!(has_field(&s, "radius"));
    assert!(has_test_rect(&s, "corner-individual"));
    assert!(has_field(&s, "corner-smoothing"));

    edit_num_field(&mut s, "radius", "16");
    let updated = s.app().doc.get(cs_id).unwrap();
    assert_eq!(updated.corner_radii, [16.0; 4]);
    assert!(updated.fill_geometry.is_none(), "editing radius on a ComponentSet must clear imported fill_geometry");
    assert!(updated.stroke_geometry.is_none(), "editing radius on a ComponentSet must clear imported stroke_geometry");
}

#[test]
fn all_four_corner_fields_have_equal_label_to_value_gaps() {
    let mut s = Sim::with(Options { select: Some("Tile A".into()), ..Default::default() });
    let tile_a = s.by_name("Tile A");
    s.shell.app.edit_nodes(&[tile_a], |n| n.corner_radii = [24.0, 4.0, 16.0, 8.0]);
    s.frame(vec![]);

    let mut gaps = Vec::new();
    for (field_key, label, val_str) in [("radius-tl", "TL", "24"), ("radius-tr", "TR", "4"), ("radius-bl", "BL", "8"), ("radius-br", "BR", "16")] {
        let r = s.field_rect(field_key);
        let spans = text_spans_in_rect(&s, r);
        let (_, _, label_right) = spans.iter().find(|(t, _, _)| t == label).unwrap_or_else(|| panic!("missing label {label} in {field_key}: {spans:?}"));
        let (_, val_left, _) = spans.iter().find(|(t, _, _)| t == val_str).unwrap_or_else(|| panic!("missing value {val_str} in {field_key}: {spans:?}"));
        let gap = *val_left - *label_right;
        assert!(
            (gap - crate::theme::SPACE_FACT).abs() < 1e-2,
            "{field_key} ({label} -> {val_str}) gap was {gap} px, expected SPACE_FACT ({} px)",
            crate::theme::SPACE_FACT
        );
        gaps.push((field_key, gap));
    }
    for &(field_key, gap) in &gaps[1..] {
        assert!((gap - gaps[0].1).abs() < 1e-2, "gap for {field_key} ({gap}) differs from {} ({})", gaps[0].0, gaps[0].1);
    }
}

/// A 100 x 100 shape of `kind` that carries the baked fill and stroke outline a `.fig` import leaves.
fn add_baked_shape(s: &mut Sim, kind: NodeKind, name: &str) -> NodeId {
    let page = s.app().page;
    let id = s.shell.app.doc.alloc_id();
    let mut n = Node::new(id, kind);
    n.name = name.into();
    n.size = DocVec2::new(100.0, 100.0);
    n.fills = vec![designos_doc::Paint::solid(designos_doc::Color::rgb(255, 0, 0))];
    n.stroke.paints = vec![designos_doc::Paint::solid(designos_doc::Color::rgb(0, 0, 255))];
    n.stroke.weight = 4.0;
    let square = Path {
        cmds: vec![
            PathCmd::MoveTo(DocVec2::new(0.0, 0.0)),
            PathCmd::LineTo(DocVec2::new(100.0, 0.0)),
            PathCmd::LineTo(DocVec2::new(100.0, 100.0)),
            PathCmd::LineTo(DocVec2::new(0.0, 100.0)),
            PathCmd::Close,
        ],
        fill_rule: FillRule::NonZero,
    };
    n.fill_geometry = Some(vec![square.clone()]);
    n.stroke_geometry = Some(vec![square]);
    s.shell.app.doc.insert(page, n).unwrap()
}

fn pixels_of(s: &Sim, id: NodeId) -> Vec<u8> {
    designos_render::render_node(&s.app().doc, id, 1.0).expect("render").data().to_vec()
}

#[test]
fn a_radius_edit_redraws_a_shape_that_carries_a_baked_outline() {
    let shapes = [
        ("rectangle", NodeKind::Rectangle),
        ("frame", NodeKind::Frame(FrameProps::default())),
        ("polygon", NodeKind::Polygon { count: 5 }),
        ("star", NodeKind::Star { count: 5, inner_ratio: 0.5 }),
    ];
    for (what, kind) in shapes {
        let mut s = Sim::new();
        let id = add_baked_shape(&mut s, kind, "Baked");
        s.shell.app.selection = vec![id];
        s.frame(vec![]);
        let before = pixels_of(&s, id);
        edit_num_field(&mut s, "radius", "24");
        let n = s.app().doc.get(id).unwrap();
        assert_eq!(n.corner_radii, [24.0; 4], "the radius was set ({what})");
        assert!(pixels_of(&s, id) != before, "the edit shows at once ({what})");
        assert!(n.fill_geometry.is_none() && n.stroke_geometry.is_none(), "the old outline is gone ({what})");
    }
}

#[test]
fn an_individual_corner_edit_redraws_a_rectangle_or_frame_that_carries_a_baked_outline() {
    for (what, kind) in [("rectangle", NodeKind::Rectangle), ("frame", NodeKind::Frame(FrameProps::default()))] {
        let mut s = Sim::new();
        let id = add_baked_shape(&mut s, kind, "Baked");
        s.shell.app.selection = vec![id];
        s.frame(vec![]);
        let toggle = s.rect_of("corner-individual");
        s.click(toggle.center());
        let before = pixels_of(&s, id);
        edit_num_field(&mut s, "radius-br", "30");
        let n = s.app().doc.get(id).unwrap();
        assert_eq!(n.corner_radii, [0.0, 0.0, 30.0, 0.0], "only the bottom-right corner was set ({what})");
        assert!(pixels_of(&s, id) != before, "the edit shows at once ({what})");
        assert!(n.fill_geometry.is_none() && n.stroke_geometry.is_none(), "the old outline is gone ({what})");
    }
}
