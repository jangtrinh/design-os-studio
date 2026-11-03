//! Edit menu parity tests for:
//! - `Paste and match style` (no shortcut)
//! - `Swap fill and stroke` (`Shift+X` / `⇧X`)
//! - `Select all with same text properties`, `Select all with same properties`, `Select all with same variant`
//! - Edit menu, context menu, and command palette labels, shortcuts, and enabled states.

use super::*;
use crate::panels::menus::Action;
use designos_doc::find::MatchCriteria;
use designos_doc::{
    create_instance, BlendMode, Color, Effect, FrameProps, GradientStop, LineHeight, Node, NodeKind, OverrideField, Paint, PaintKind, Rect, Stroke, Style,
    StyleMeta, StyleProperty, TextRun, TextStyle, VariableKind, VariableStore, VariableValue, Vec2 as DV,
};

fn run_palette_command(s: &mut Sim, query: &str) {
    s.cmd_key(Key::K);
    s.frame(vec![]);
    s.type_text(query);
    s.frame(vec![]);
    s.key(Key::Enter);
    s.frame(vec![]);
}

fn make_component_set(id: NodeId, name: &str, r: Rect) -> Node {
    let mut n = Node::new(id, NodeKind::ComponentSet(FrameProps::default()));
    n.name = name.into();
    n.transform.tx = r.x;
    n.transform.ty = r.y;
    n.size = DV::new(r.w, r.h);
    n
}

fn make_component(id: NodeId, name: &str, r: Rect) -> Node {
    let mut n = Node::new(id, NodeKind::Component(FrameProps::default()));
    n.name = name.into();
    n.transform.tx = r.x;
    n.transform.ty = r.y;
    n.size = DV::new(r.w, r.h);
    n
}

#[test]
fn paste_and_match_style_layer_and_text_carries_target_style_and_undoes_in_one_step() {
    let mut s = Sim::new();
    let page = s.app().page;

    // Source text layer with Style A: 32px bold Roboto, red fill, blue stroke, drop shadow A.
    let src_id = s.shell.app.doc.alloc_id();
    let style_a = TextStyle {
        font_family: "Roboto".into(),
        font_weight: 700,
        italic: true,
        font_size: 32.0,
        line_height: LineHeight::Pixels(40.0),
        letter_spacing: 1.5,
        fills: vec![Paint::solid(Color::rgb(220, 40, 40))],
        ..Default::default()
    };
    let mut src_node = Node::text(src_id, Rect::new(40.0, 40.0, 200.0, 50.0), "Copied Headline", style_a);
    src_node.stroke = Stroke { paints: vec![Paint::solid(Color::rgb(30, 80, 220))], weight: 3.0, ..Default::default() };
    src_node.effects = vec![Effect::DropShadow {
        color: Color::rgba(0, 0, 0, 128),
        offset: DV::new(4.0, 8.0),
        radius: 12.0,
        spread: 0.0,
        visible: true,
        show_behind: false,
        blend: BlendMode::Normal,
    }];
    src_node.opacity = 0.65;
    s.shell.app.doc.insert(page, src_node).unwrap();

    // Target text layer with Style B: 14px regular Inter, green fill, amber stroke, inner shadow B.
    let dst_id = s.shell.app.doc.alloc_id();
    let style_b = TextStyle {
        font_family: "Inter".into(),
        font_weight: 400,
        italic: false,
        font_size: 14.0,
        line_height: LineHeight::Pixels(20.0),
        letter_spacing: 0.25,
        fills: vec![Paint::solid(Color::rgb(20, 180, 90))],
        ..Default::default()
    };
    let mut dst_node = Node::text(dst_id, Rect::new(300.0, 120.0, 160.0, 30.0), "Target Body", style_b.clone());
    let stroke_b = Stroke { paints: vec![Paint::solid(Color::rgb(240, 170, 20))], weight: 1.5, ..Default::default() };
    let effects_b = vec![Effect::InnerShadow {
        color: Color::rgba(10, 20, 30, 200),
        offset: DV::new(1.0, 2.0),
        radius: 4.0,
        spread: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    }];
    dst_node.stroke = stroke_b.clone();
    dst_node.effects = effects_b.clone();
    dst_node.opacity = 0.9;
    s.shell.app.doc.insert(page, dst_node).unwrap();

    // Copy source with style A, select target with style B, and invoke "Paste and match style".
    s.shell.app.select_only(src_id);
    s.shell.app.copy_selection();
    s.shell.app.select_only(dst_id);
    let undo_before = s.app().hist.undo_depth();

    run_palette_command(&mut s, "Paste and match style");

    assert_eq!(s.app().hist.undo_depth(), undo_before + 1, "Paste and match style must record exactly one undo step");
    assert_eq!(s.app().selection.len(), 1);
    let pasted_id = s.app().selection[0];
    assert_ne!(pasted_id, src_id);
    assert_ne!(pasted_id, dst_id);
    let pasted = s.app().doc.get(pasted_id).expect("pasted node exists");
    let tp = pasted.kind.any_text().expect("pasted node is text");
    assert_eq!(tp.content, "Copied Headline", "pasted node carries the copied text content");
    assert_eq!(tp.style.font_family, "Inter", "pasted node takes target font family");
    assert_eq!(tp.style.font_weight, 400, "pasted node takes target font weight");
    assert!(!tp.style.italic, "pasted node takes target italic flag");
    assert_eq!(tp.style.font_size, 14.0, "pasted node takes target font size");
    assert_eq!(tp.style.line_height, LineHeight::Pixels(20.0), "pasted node takes target line height");
    assert_eq!(tp.style.letter_spacing, 0.25, "pasted node takes target letter spacing");
    assert_eq!(tp.style.fills, style_b.fills, "pasted node takes target text fill");
    assert_eq!(pasted.stroke, stroke_b, "pasted node takes target stroke");
    assert_eq!(pasted.effects, effects_b, "pasted node takes target effects");
    assert_eq!(pasted.opacity, 0.9, "pasted node takes target opacity");

    // Undo restores the prior state in one step.
    s.shell.app.undo();
    assert_eq!(s.app().hist.undo_depth(), undo_before);
    assert!(s.app().doc.get(pasted_id).is_none(), "single undo removes the pasted node");

    // Inside an open text editing session the command pastes plain text with the target run's style.
    crate::canvas::textsession::begin_text_edit(&mut s.shell.app, dst_id);
    assert!(s.app().canvas.text.is_some());
    let ctx = s.ctx.clone();
    crate::panels::menus::run(&mut s.shell.app, &ctx, Action::PasteAndMatchStyle);
    crate::canvas::textsession::finish_text(&mut s.shell.app, true);

    let dst_after = s.app().doc.get(dst_id).unwrap().kind.any_text().unwrap();
    assert_eq!(dst_after.content, "Copied Headline");
    assert_eq!(dst_after.style.font_family, "Inter");
    assert_eq!(dst_after.style.font_size, 14.0);
    assert_eq!(dst_after.style.fills, style_b.fills);
    assert!(dst_after.runs.is_empty(), "spliced text matches the target base style with no foreign style runs");

    s.shell.app.undo();
    let dst_restored = s.app().doc.get(dst_id).unwrap().kind.any_text().unwrap();
    assert_eq!(dst_restored.content, "Target Body", "one undo restores target text before session edit");
}

#[test]
fn swap_fill_and_stroke_single_and_multi_selection_preserves_gradients_and_bindings_and_round_trips() {
    let mut s = Sim::new();
    let page = s.app().page;

    let c_fill = Color::rgb(24, 144, 255);
    let c_stroke = Color::rgb(255, 85, 0);
    let c_stop = Color::rgb(120, 220, 40);
    s.shell.app.doc.styles.insert("style_fill_a".into(), Style::Fill(vec![Paint::solid(c_fill)]));
    s.shell.app.doc.style_meta.insert("style_fill_a".into(), StyleMeta { name: "Brand/Fill".into(), property: StyleProperty::Fill, ..Default::default() });
    s.shell.app.doc.styles.insert("style_stroke_b".into(), Style::Fill(vec![Paint::solid(c_stroke)]));
    s.shell
        .app
        .doc
        .style_meta
        .insert("style_stroke_b".into(), StyleMeta { name: "Brand/Stroke".into(), property: StyleProperty::Stroke, ..Default::default() });

    // Node 1: Solid variable-bound fill + LinearGradient variable-bound stroke + linked fill/stroke styles.
    let n1 = s.shell.app.doc.alloc_id();
    let solid_fill = vec![Paint::solid(c_fill)];
    let grad_stroke = vec![Paint {
        kind: PaintKind::LinearGradient {
            handles: [DV::new(0.0, 0.0), DV::new(1.0, 1.0), DV::new(0.0, 1.0)],
            stops: vec![GradientStop { pos: 0.0, color: c_stop }, GradientStop { pos: 1.0, color: Color::WHITE }],
        },
        opacity: 0.75,
        visible: true,
        blend: BlendMode::Normal,
    }];
    let mut node1 = Node::rectangle(n1, Rect::new(50.0, 50.0, 120.0, 80.0));
    node1.fills = solid_fill.clone();
    node1.stroke = Stroke { paints: grad_stroke.clone(), weight: 2.0, ..Default::default() };
    node1.variables.bindings.insert("fills/0".into(), "var_fill".into());
    node1.variables.bindings.insert("strokes/0/stops/0".into(), "var_stop0".into());
    node1.variables.bindings.insert("strokes/0/opacity".into(), "var_opacity".into());
    node1.variables.bindings.insert("strokeWeight".into(), "var_weight".into());
    node1.style_refs.insert("fill".into(), "style_fill_a".into());
    node1.style_refs.insert("stroke".into(), "style_stroke_b".into());
    s.shell.app.doc.insert(page, node1).unwrap();

    // Node 2: Solid fill with empty stroke and weight 0.0 -> swapping gives stroke weight 1.0.
    let n2 = s.shell.app.doc.alloc_id();
    let fill_2 = vec![Paint::solid(c_stroke)];
    let mut node2 = Node::rectangle(n2, Rect::new(220.0, 50.0, 100.0, 100.0));
    node2.fills = fill_2.clone();
    node2.stroke = Stroke { paints: vec![], weight: 0.0, ..Default::default() };
    node2.variables.bindings.insert("fills/0".into(), "var_stroke".into());
    s.shell.app.doc.insert(page, node2).unwrap();

    // 1. Single-node swap via Shift+X shortcut.
    s.shell.app.select_only(n1);
    let u0 = s.app().hist.undo_depth();
    shift_key(&mut s, Key::X);
    assert_eq!(s.app().hist.undo_depth(), u0 + 1, "single-node swap records 1 undo step");

    let after1 = s.app().doc.get(n1).unwrap();
    assert_eq!(after1.fills, grad_stroke, "n1 fills received the gradient stroke");
    assert_eq!(after1.stroke.paints, solid_fill, "n1 stroke received the solid fill");
    assert_eq!(after1.variables.bindings.get("strokes/0").map(String::as_str), Some("var_fill"), "fills/0 binding moved to strokes/0");
    assert_eq!(after1.variables.bindings.get("fills/0/stops/0").map(String::as_str), Some("var_stop0"), "strokes/0/stops/0 binding moved to fills/0/stops/0");
    assert_eq!(after1.variables.bindings.get("fills/0/opacity").map(String::as_str), Some("var_opacity"), "strokes/0/opacity binding moved to fills/0/opacity");
    assert_eq!(after1.variables.bindings.get("strokeWeight").map(String::as_str), Some("var_weight"), "non-paint strokeWeight binding stays on strokeWeight");
    assert!(!after1.variables.bindings.contains_key("fills/0"));
    assert!(!after1.variables.bindings.contains_key("strokes/0/stops/0"));
    assert_eq!(after1.style_refs.get("fill").map(String::as_str), Some("style_stroke_b"));
    assert_eq!(after1.style_refs.get("stroke").map(String::as_str), Some("style_fill_a"));

    // Run again to round-trip n1.
    shift_key(&mut s, Key::X);
    assert_eq!(s.app().hist.undo_depth(), u0 + 2, "second swap records 1 undo step");
    let rt1 = s.app().doc.get(n1).unwrap();
    assert_eq!(rt1.fills, solid_fill);
    assert_eq!(rt1.stroke.paints, grad_stroke);
    assert_eq!(rt1.variables.bindings.get("fills/0").map(String::as_str), Some("var_fill"));
    assert_eq!(rt1.variables.bindings.get("strokes/0/stops/0").map(String::as_str), Some("var_stop0"));
    assert_eq!(rt1.variables.bindings.get("strokes/0/opacity").map(String::as_str), Some("var_opacity"));
    assert_eq!(rt1.style_refs.get("fill").map(String::as_str), Some("style_fill_a"));
    assert_eq!(rt1.style_refs.get("stroke").map(String::as_str), Some("style_stroke_b"));

    // 2. Multi-selection swap across n1 and n2 in one undo step.
    s.shell.app.selection = vec![n1, n2];
    let u2 = s.app().hist.undo_depth();
    run_palette_command(&mut s, "Swap fill and stroke");
    assert_eq!(s.app().hist.undo_depth(), u2 + 1, "multi-selection swap records 1 undo step");

    let m1 = s.app().doc.get(n1).unwrap();
    let m2 = s.app().doc.get(n2).unwrap();
    assert_eq!(m1.fills, grad_stroke);
    assert_eq!(m1.stroke.paints, solid_fill);
    assert!(m2.fills.is_empty());
    assert_eq!(m2.stroke.paints, fill_2);
    assert_eq!(m2.stroke.weight, 1.0, "swapping fill into an empty 0-weight stroke sets weight to 1.0 like Figma");
    assert_eq!(m2.variables.bindings.get("strokes/0").map(String::as_str), Some("var_stroke"));

    // Single undo restores both n1 and n2.
    s.shell.app.undo();
    assert_eq!(s.app().hist.undo_depth(), u2);
    assert_eq!(s.app().doc.get(n1).unwrap().fills, solid_fill);
    assert_eq!(s.app().doc.get(n2).unwrap().fills, fill_2);
    assert!(s.app().doc.get(n2).unwrap().stroke.paints.is_empty());
}

#[test]
fn select_same_text_style_and_variant_match_exact_unlocked_visible_siblings() {
    let mut s = Sim::new();
    let page = s.app().page;
    // Remove default demo children so the page contains only our controlled test siblings.
    let existing: Vec<NodeId> = s.app().doc.get(page).unwrap().children.clone();
    for id in existing {
        s.shell.app.doc.remove(id).unwrap();
    }

    // 1. Select all with same text properties:
    let ts_match = TextStyle { font_family: "Inter".into(), font_size: 18.0, font_weight: 600, ..Default::default() };
    let ts_diff_size = TextStyle { font_size: 24.0, ..ts_match.clone() };
    let t1 = s.shell.app.doc.alloc_id();
    let t2 = s.shell.app.doc.alloc_id();
    let t_diff = s.shell.app.doc.alloc_id();
    let t_hidden = s.shell.app.doc.alloc_id();
    let t_locked = s.shell.app.doc.alloc_id();
    s.shell.app.doc.insert(page, Node::text(t1, Rect::new(0.0, 0.0, 80.0, 24.0), "A", ts_match.clone())).unwrap();
    s.shell.app.doc.insert(page, Node::text(t2, Rect::new(0.0, 30.0, 80.0, 24.0), "B", ts_match.clone())).unwrap();
    s.shell.app.doc.insert(page, Node::text(t_diff, Rect::new(0.0, 60.0, 80.0, 24.0), "C", ts_diff_size)).unwrap();
    let mut nh = Node::text(t_hidden, Rect::new(0.0, 90.0, 80.0, 24.0), "H", ts_match.clone());
    nh.visible = false;
    s.shell.app.doc.insert(page, nh).unwrap();
    let mut nl = Node::text(t_locked, Rect::new(0.0, 120.0, 80.0, 24.0), "L", ts_match);
    nl.locked = true;
    s.shell.app.doc.insert(page, nl).unwrap();

    let ctx = s.ctx.clone();
    s.shell.app.select_only(t1);
    crate::panels::menus::run(&mut s.shell.app, &ctx, Action::SelectSame(MatchCriteria::TextProperties));
    assert_eq!(s.app().selection, vec![t2, t1], "SelectSame(TextProperties) selects only visible unlocked matching text nodes");

    // 2. Select all with same properties (select-same-style):
    let r1 = s.shell.app.doc.alloc_id();
    let r2 = s.shell.app.doc.alloc_id();
    let r_diff_opacity = s.shell.app.doc.alloc_id();
    let r_hidden = s.shell.app.doc.alloc_id();
    let make_rect = |id| {
        let mut n = Node::rectangle(id, Rect::new(200.0, 0.0, 50.0, 50.0));
        n.fills = vec![Paint::solid(Color::rgb(10, 200, 120))];
        n.opacity = 0.8;
        n
    };
    s.shell.app.doc.insert(page, make_rect(r1)).unwrap();
    s.shell.app.doc.insert(page, make_rect(r2)).unwrap();
    let mut rd = make_rect(r_diff_opacity);
    rd.opacity = 0.5;
    s.shell.app.doc.insert(page, rd).unwrap();
    let mut rh = make_rect(r_hidden);
    rh.visible = false;
    s.shell.app.doc.insert(page, rh).unwrap();

    s.shell.app.select_only(r1);
    crate::panels::menus::run(&mut s.shell.app, &ctx, Action::SelectSame(MatchCriteria::Properties));
    assert_eq!(s.app().selection, vec![r2, r1], "SelectSame(Properties) selects only visible unlocked nodes with identical properties");

    // 3. Select all with same variant:
    let set_a = s.shell.app.doc.alloc_id();
    let comp_a1 = s.shell.app.doc.alloc_id();
    let comp_a2 = s.shell.app.doc.alloc_id();
    let comp_other = s.shell.app.doc.alloc_id();
    s.shell.app.doc.insert(page, make_component_set(set_a, "Chip", Rect::new(400.0, 0.0, 200.0, 100.0))).unwrap();
    s.shell.app.doc.insert(set_a, make_component(comp_a1, "Size=Sm", Rect::new(0.0, 0.0, 80.0, 32.0))).unwrap();
    s.shell.app.doc.insert(set_a, make_component(comp_a2, "Size=Lg", Rect::new(90.0, 0.0, 80.0, 32.0))).unwrap();
    s.shell.app.doc.insert(page, make_component(comp_other, "Standalone", Rect::new(400.0, 120.0, 80.0, 32.0))).unwrap();

    let mut inst_ids = Vec::new();
    for (i, cid) in [comp_a1, comp_a2, comp_other, comp_a2].into_iter().enumerate() {
        let cmd = create_instance(&mut s.shell.app.doc, cid, page, DV::new(650.0, i as f32 * 40.0));
        let designos_doc::Command::Insert { nodes, .. } = &cmd else { panic!("expected Insert") };
        let iid = nodes[0].id;
        cmd.apply(&mut s.shell.app.doc).unwrap();
        inst_ids.push(iid);
    }
    // Lock the last variant instance to verify locked instances are excluded.
    s.shell.app.doc.get_mut(inst_ids[3]).unwrap().locked = true;

    s.shell.app.select_only(inst_ids[0]);
    crate::panels::menus::run(&mut s.shell.app, &ctx, Action::SelectSame(MatchCriteria::Variant));
    assert_eq!(
        s.app().selection,
        vec![inst_ids[1], inst_ids[0]],
        "SelectSame(Variant) selects all visible unlocked instances from the same ComponentSet and excludes standalone/locked instances"
    );
}

#[test]
fn edit_menu_and_palette_expose_all_five_commands_with_labels_shortcuts_and_enabled_states() {
    let mut s = Sim::new();
    let page = s.app().page;

    let rect_id = s.shell.app.doc.alloc_id();
    s.shell.app.doc.insert(page, Node::rectangle(rect_id, Rect::new(20.0, 20.0, 60.0, 60.0))).unwrap();
    let text_id = s.shell.app.doc.alloc_id();
    s.shell.app.doc.insert(page, Node::text(text_id, Rect::new(100.0, 20.0, 80.0, 24.0), "Hi", TextStyle::default())).unwrap();

    let set_id = s.shell.app.doc.alloc_id();
    let comp_id = s.shell.app.doc.alloc_id();
    s.shell.app.doc.insert(page, make_component_set(set_id, "Tag", Rect::new(200.0, 20.0, 100.0, 50.0))).unwrap();
    s.shell.app.doc.insert(set_id, make_component(comp_id, "State=On", Rect::new(0.0, 0.0, 60.0, 24.0))).unwrap();
    let cmd = create_instance(&mut s.shell.app.doc, comp_id, page, DV::new(320.0, 20.0));
    let designos_doc::Command::Insert { nodes, .. } = &cmd else { panic!("expected Insert") };
    let var_inst_id = nodes[0].id;
    cmd.apply(&mut s.shell.app.doc).unwrap();

    // Empty selection: all five commands disabled in palette / Edit menu.
    s.shell.app.selection.clear();
    s.shell.app.clip = None;
    let check_cmd = |app: &App, label: &str| -> (String, bool) {
        crate::panels::menus::palette_command_info(app, label).unwrap_or_else(|| panic!("missing command in Edit menu / palette: {label}"))
    };

    assert_eq!(check_cmd(s.app(), "Paste and match style"), ("".into(), true));
    assert_eq!(check_cmd(s.app(), "Swap fill and stroke"), ("Shift+X".into(), true));
    assert_eq!(check_cmd(s.app(), "Select all with same text properties"), ("".into(), true));
    assert_eq!(check_cmd(s.app(), "Select all with same properties"), ("".into(), true));
    assert_eq!(check_cmd(s.app(), "Select all with same variant"), ("".into(), true));

    // Rectangle selected (not copied yet): Swap fill and stroke & Select same properties enabled;
    // Paste and match style, Select same text properties, and Select same variant disabled.
    s.shell.app.select_only(rect_id);
    assert_eq!(check_cmd(s.app(), "Swap fill and stroke"), ("Shift+X".into(), false));
    assert_eq!(check_cmd(s.app(), "Select all with same properties"), ("".into(), false));
    assert_eq!(check_cmd(s.app(), "Paste and match style"), ("".into(), true));
    assert_eq!(check_cmd(s.app(), "Select all with same text properties"), ("".into(), true));
    assert_eq!(check_cmd(s.app(), "Select all with same variant"), ("".into(), true));

    // Copy rectangle, then select text node: Paste and match style and Select same text properties enabled.
    s.shell.app.copy_selection();
    s.shell.app.select_only(text_id);
    assert_eq!(check_cmd(s.app(), "Paste and match style"), ("".into(), false));
    assert_eq!(check_cmd(s.app(), "Select all with same text properties"), ("".into(), false));

    // Select variant instance: Select all with same variant enabled.
    s.shell.app.select_only(var_inst_id);
    assert_eq!(check_cmd(s.app(), "Select all with same variant"), ("".into(), false));
}

fn solid(r: u8, g: u8, b: u8) -> Vec<Paint> {
    vec![Paint::solid(Color::rgb(r, g, b))]
}

fn child_named(d: &designos_doc::Document, parent: NodeId, name: &str) -> NodeId {
    *d.get(parent).unwrap().children.iter().find(|c| d.get(**c).unwrap().name == name).unwrap_or_else(|| panic!("no child {name}"))
}

/// A five-character text layer whose base style paints `base` and whose two runs differ in weight and paint `run_fills`.
fn insert_text_with_runs(s: &mut Sim, base: Vec<Paint>, run_fills: &[Vec<Paint>]) -> NodeId {
    let page = s.app().page;
    let id = s.shell.app.doc.alloc_id();
    let style = TextStyle { fills: base, ..Default::default() };
    let mut node = Node::text(id, Rect::new(10.0, 10.0, 120.0, 24.0), "Hello", style.clone());
    if let NodeKind::Text(tp) = &mut node.kind {
        tp.runs = run_fills
            .iter()
            .enumerate()
            .map(|(i, f)| TextRun { start: i * 2, end: i * 2 + 2, style: TextStyle { fills: f.clone(), font_weight: 400 + 300 * i as u16, ..style.clone() } })
            .collect();
    }
    s.shell.app.doc.insert(page, node).unwrap();
    id
}

#[test]
fn swap_fill_and_stroke_on_uniform_text_exchanges_the_painted_fills_with_the_stroke_and_round_trips() {
    let mut s = Sim::new();
    let (red, green, stale) = (solid(220, 40, 40), solid(20, 180, 90), solid(128, 128, 128));
    let id = insert_text_with_runs(&mut s, red.clone(), &[red.clone(), red.clone()]);
    {
        let n = s.shell.app.doc.get_mut(id).unwrap();
        n.stroke = Stroke { paints: green.clone(), weight: 2.0, ..Default::default() };
        // Paste properties leaves a list here; a text layer never paints it.
        n.fills = stale.clone();
    }
    s.shell.app.select_only(id);
    let undo = s.app().hist.undo_depth();

    shift_key(&mut s, Key::X);
    assert_eq!(s.app().hist.undo_depth(), undo + 1, "one undo step");
    let n = s.app().doc.get(id).unwrap();
    let tp = n.kind.any_text().unwrap();
    assert_eq!(tp.style.fills, green, "the base style paints the old stroke");
    assert!(tp.runs.iter().all(|r| r.style.fills == green), "every run paints the old stroke");
    assert_eq!(tp.runs[1].style.font_weight, 700, "a run keeps the rest of its own style");
    assert_eq!(n.stroke.paints, red, "the stroke takes the painted text fills");
    assert_eq!(n.fills, stale, "Node::fills paints nothing on text and is left alone");

    shift_key(&mut s, Key::X);
    let n = s.app().doc.get(id).unwrap();
    let tp = n.kind.any_text().unwrap();
    assert_eq!(tp.style.fills, red, "the second swap restores the base style");
    assert!(tp.runs.iter().all(|r| r.style.fills == red), "the second swap restores every run");
    assert_eq!(n.stroke.paints, green, "the second swap restores the stroke");
}

#[test]
fn swap_fill_and_stroke_leaves_text_with_different_run_fills_unchanged_and_swaps_the_rest() {
    let mut s = Sim::new();
    let page = s.app().page;
    let (red, blue, green) = (solid(220, 40, 40), solid(30, 80, 220), solid(20, 180, 90));
    let mixed = insert_text_with_runs(&mut s, red.clone(), &[red.clone(), blue.clone()]);
    {
        let n = s.shell.app.doc.get_mut(mixed).unwrap();
        n.stroke = Stroke { paints: green.clone(), weight: 2.0, ..Default::default() };
        n.variables.bindings.insert("fills/0".into(), "var_text_fill".into());
        n.variables.bindings.insert("strokes/0".into(), "var_text_stroke".into());
        n.style_refs.insert("fill".into(), "style_text_fill".into());
    }
    let rect = s.shell.app.doc.alloc_id();
    let mut r = Node::rectangle(rect, Rect::new(200.0, 10.0, 60.0, 60.0));
    r.fills = red.clone();
    r.stroke = Stroke { paints: blue.clone(), weight: 1.0, ..Default::default() };
    s.shell.app.doc.insert(page, r).unwrap();
    let before = s.app().doc.get(mixed).unwrap().clone();

    s.shell.app.selection = vec![mixed, rect];
    let undo = s.app().hist.undo_depth();
    shift_key(&mut s, Key::X);

    assert_eq!(s.app().hist.undo_depth(), undo + 1, "the rest of the selection still swaps in one undo step");
    assert_eq!(s.app().doc.get(mixed).unwrap(), &before, "a text layer whose runs paint different fills is left exactly as it was");
    let r = s.app().doc.get(rect).unwrap();
    assert_eq!((&r.fills, &r.stroke.paints), (&blue, &red), "the rectangle in the same selection swapped");
}

#[test]
fn swap_fill_and_stroke_on_instance_text_survives_a_sync_of_its_main() {
    let mut s = Sim::new();
    let (inst, other, main) = (s.by_name("Get started"), s.by_name("Learn more"), s.by_name("Primary button"));
    let label = child_named(&s.app().doc, inst, "Label");
    let other_label = child_named(&s.app().doc, other, "Label");
    let main_label = child_named(&s.app().doc, main, "Label");
    let white = s.app().doc.get(label).unwrap().kind.any_text().unwrap().style.fills.clone();
    assert!(!white.is_empty() && s.app().doc.get(label).unwrap().stroke.paints.is_empty());

    s.shell.app.select_only(label);
    shift_key(&mut s, Key::X);
    let n = s.app().doc.get(label).unwrap();
    assert_eq!(n.stroke.paints, white, "the instance label now has the old text fill as its stroke");
    assert!(n.kind.any_text().unwrap().style.fills.is_empty());
    assert!(n.component.overrides.contains(&OverrideField::TextStyle), "the text fills are recorded as an override");
    assert!(n.component.overrides.contains(&OverrideField::Stroke), "the stroke is recorded as an override");

    let (red, green) = (solid(255, 0, 0), solid(0, 160, 0));
    s.shell.app.edit_nodes(&[main_label], |n| {
        if let Some(tp) = n.kind.any_text_mut() {
            tp.style.fills = red.clone();
        }
        n.stroke = Stroke { paints: green.clone(), weight: 1.0, ..Default::default() };
    });
    let follower = s.app().doc.get(other_label).unwrap();
    assert_eq!(follower.kind.any_text().unwrap().style.fills, red, "an instance without the override follows the main");
    assert_eq!(follower.stroke.paints, green);
    let n = s.app().doc.get(label).unwrap();
    assert!(n.kind.any_text().unwrap().style.fills.is_empty(), "the swapped fills survive the sync");
    assert_eq!(n.stroke.paints, white, "the swapped stroke survives the sync");
}

#[test]
fn swap_fill_and_stroke_gives_a_stroke_with_every_width_zero_a_visible_width() {
    let mut s = Sim::new();
    let page = s.app().page;
    let red = solid(220, 40, 40);
    let rect_with = |s: &mut Sim, weight: f32, sides: Option<[f32; 4]>| {
        let id = s.shell.app.doc.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(10.0, 10.0, 80.0, 60.0));
        n.fills = red.clone();
        n.stroke = Stroke { paints: vec![], weight, side_weights: sides, ..Default::default() };
        s.shell.app.doc.insert(page, n).unwrap();
        id
    };
    let all_sides_zero = rect_with(&mut s, 1.0, Some([0.0; 4]));
    let one_side = rect_with(&mut s, 0.0, Some([2.0, 0.0, 0.0, 0.0]));
    let no_sides = rect_with(&mut s, 0.0, None);
    let visible = |s: &Sim, id: NodeId| {
        let n = s.app().doc.get(id).unwrap();
        let shape = designos_render::outline_of(&s.app().doc, n, 0.25).expect("a rectangle has a shape");
        designos_render::stroke_plan(n, &shape, 1.0).is_some()
    };

    s.shell.app.selection = vec![all_sides_zero, one_side, no_sides];
    shift_key(&mut s, Key::X);

    let n = s.app().doc.get(all_sides_zero).unwrap();
    assert_eq!(n.stroke.paints, red);
    assert_eq!((n.stroke.weight, n.stroke.side_weights), (1.0, Some([1.0; 4])), "all four sides were 0, so each becomes 1");
    assert!(visible(&s, all_sides_zero), "the swapped stroke is drawn");
    let n = s.app().doc.get(one_side).unwrap();
    assert_eq!((n.stroke.weight, n.stroke.side_weights), (0.0, Some([2.0, 0.0, 0.0, 0.0])), "a side that already has a width keeps every side as it was");
    assert!(visible(&s, one_side));
    let n = s.app().doc.get(no_sides).unwrap();
    assert_eq!((n.stroke.weight, n.stroke.side_weights), (1.0, None), "a zero uniform weight becomes 1");
    assert!(visible(&s, no_sides));
}

/// A rectangle copied and pasted over a bound, linked text layer that also binds layout and content.
#[test]
fn paste_and_match_style_copies_only_the_visual_bindings_and_style_refs() {
    let mut s = Sim::new();
    let page = s.app().page;
    let src = s.shell.app.doc.alloc_id();
    let mut source = Node::rectangle(src, Rect::new(20.0, 20.0, 60.0, 60.0));
    source.variables.bindings.insert("width".into(), "var_src_width".into());
    source.variables.bindings.insert("fills/0".into(), "var_src_fill".into());
    source.variables.bindings.insert("itemSpacing".into(), "var_src_gap".into());
    source.style_refs.insert("grid".into(), "grid_src".into());
    source.style_refs.insert("fill".into(), "fill_src".into());
    s.shell.app.doc.insert(page, source).unwrap();

    let dst = s.shell.app.doc.alloc_id();
    let style = TextStyle { font_family: "Inter".into(), font_size: 14.0, fills: solid(20, 180, 90), ..Default::default() };
    let mut target = Node::text(dst, Rect::new(300.0, 20.0, 160.0, 30.0), "Target", style);
    for (k, v) in [
        ("width", "var_width"),
        ("characters", "var_characters"),
        ("itemSpacing", "var_gap"),
        ("fontSize", "var_font_size"),
        ("fills/0", "var_fill"),
        ("strokes/0", "var_stroke"),
        ("strokeWeight", "var_weight"),
        ("strokeTopWeight", "var_top"),
        ("opacity", "var_opacity"),
        ("topLeftRadius", "var_radius"),
    ] {
        target.variables.bindings.insert(k.into(), v.into());
    }
    for (k, v) in [("fill", "fill_tgt"), ("stroke", "stroke_tgt"), ("effect", "effect_tgt"), ("text", "text_tgt"), ("grid", "grid_tgt")] {
        target.style_refs.insert(k.into(), v.into());
    }
    s.shell.app.doc.insert(page, target).unwrap();

    s.shell.app.select_only(src);
    s.shell.app.copy_selection();
    s.shell.app.select_only(dst);
    let undo = s.app().hist.undo_depth();
    let ctx = s.ctx.clone();
    crate::panels::menus::run(&mut s.shell.app, &ctx, Action::PasteAndMatchStyle);

    assert_eq!(s.app().hist.undo_depth(), undo + 1, "one undo step");
    let pasted = s.app().doc.get(s.app().selection[0]).unwrap();
    let bound = |k: &str| pasted.variables.bindings.get(k).map(String::as_str);
    for (k, v) in [
        ("fills/0", "var_fill"),
        ("strokes/0", "var_stroke"),
        ("strokeWeight", "var_weight"),
        ("strokeTopWeight", "var_top"),
        ("opacity", "var_opacity"),
        ("topLeftRadius", "var_radius"),
    ] {
        assert_eq!(bound(k), Some(v), "the target's {k} binding is copied");
    }
    assert_eq!(bound("width"), Some("var_src_width"), "the pasted layer keeps its own size binding, not the target's");
    assert_eq!(bound("itemSpacing"), Some("var_src_gap"), "the pasted layer keeps its own layout binding, not the target's");
    assert_eq!(bound("characters"), None, "a rectangle has no characters to bind");
    assert_eq!(bound("fontSize"), None, "a rectangle takes no typography binding");
    let linked = |k: &str| pasted.style_refs.get(k).map(String::as_str);
    assert_eq!(linked("fill"), Some("fill_tgt"));
    assert_eq!(linked("stroke"), Some("stroke_tgt"));
    assert_eq!(linked("effect"), Some("effect_tgt"));
    assert_eq!(linked("text"), None, "a rectangle takes no text style");
    assert_eq!(linked("grid"), Some("grid_src"), "a grid style is never copied");
}

#[test]
fn paste_and_match_style_restyles_the_text_inside_a_pasted_frame() {
    let mut s = Sim::new();
    let page = s.app().page;
    let frame = s.shell.app.doc.alloc_id();
    s.shell.app.doc.insert(page, Node::frame(frame, Rect::new(20.0, 20.0, 200.0, 100.0))).unwrap();
    let inner = s.shell.app.doc.alloc_id();
    let src_style = TextStyle { font_family: "Roboto".into(), font_size: 30.0, fills: solid(220, 40, 40), ..Default::default() };
    let mut child = Node::text(inner, Rect::new(10.0, 10.0, 120.0, 40.0), "Inner", src_style);
    child.variables.bindings.insert("fontSize".into(), "var_src_font".into());
    child.variables.bindings.insert("width".into(), "var_src_width".into());
    child.style_refs.insert("text".into(), "text_src".into());
    s.shell.app.doc.insert(frame, child).unwrap();

    let dst = s.shell.app.doc.alloc_id();
    let style = TextStyle { font_family: "Inter".into(), font_size: 14.0, font_weight: 400, fills: solid(20, 180, 90), ..Default::default() };
    let mut target = Node::text(dst, Rect::new(300.0, 20.0, 160.0, 30.0), "Target", style.clone());
    target.variables.bindings.insert("fontSize".into(), "var_font_size".into());
    target.style_refs.insert("text".into(), "text_tgt".into());
    s.shell.app.doc.insert(page, target).unwrap();

    s.shell.app.select_only(frame);
    s.shell.app.copy_selection();
    s.shell.app.select_only(dst);
    let undo = s.app().hist.undo_depth();
    let ctx = s.ctx.clone();
    crate::panels::menus::run(&mut s.shell.app, &ctx, Action::PasteAndMatchStyle);

    assert_eq!(s.app().hist.undo_depth(), undo + 1, "one undo step");
    let root = s.app().doc.get(s.app().selection[0]).unwrap();
    assert!(matches!(root.kind, NodeKind::Frame(_)), "the frame is pasted, not its text");
    let text = s.app().doc.get(root.children[0]).unwrap();
    let tp = text.kind.any_text().unwrap();
    assert_eq!(tp.content, "Inner", "the text keeps its own characters");
    assert_eq!(
        (tp.style.font_family.as_str(), tp.style.font_size, &tp.style.fills),
        ("Inter", 14.0, &style.fills),
        "the text child takes the target's text style"
    );
    assert_eq!(text.style_refs.get("text").map(String::as_str), Some("text_tgt"), "and its text style link");
    assert_eq!(text.variables.bindings.get("fontSize").map(String::as_str), Some("var_font_size"), "and its typography binding");
    assert_eq!(text.variables.bindings.get("width").map(String::as_str), Some("var_src_width"), "but not a size binding");
}

#[test]
fn paste_and_match_style_rebinds_the_effect_colour_to_the_targets_variable() {
    let mut s = Sim::new();
    let page = s.app().page;
    let (red, blue) = (Color::rgb(220, 40, 40), Color::rgb(30, 80, 220));
    let mut store = VariableStore::from_doc(&s.app().doc);
    let theme = s.app().doc.variable_collections.values().find(|c| c.name == "Theme").expect("the demo has a Theme collection");
    let (collection, modes) = (theme.id.clone(), theme.modes.iter().map(|m| m.id.clone()).collect::<Vec<_>>());
    let var_a = store.add_variable(&collection, "Effect/ShadowA", VariableKind::Color).unwrap();
    let var_b = store.add_variable(&collection, "Effect/ShadowB", VariableKind::Color).unwrap();
    for mode in &modes {
        store.set_value(&var_a, mode, VariableValue::Color(red));
        store.set_value(&var_b, mode, VariableValue::Color(blue));
    }
    s.shell.app.commit_store(store, &[], &[]);

    let shadow =
        |color| Effect::DropShadow { color, offset: DV::new(0.0, 4.0), radius: 8.0, spread: 0.0, visible: true, show_behind: false, blend: BlendMode::Normal };
    let src = s.shell.app.doc.alloc_id();
    let mut source = Node::rectangle(src, Rect::new(20.0, 20.0, 60.0, 60.0));
    source.effects = vec![shadow(red)];
    source.variables.bindings.insert("effects/0/color".into(), var_a.clone());
    s.shell.app.doc.insert(page, source).unwrap();
    let dst = s.shell.app.doc.alloc_id();
    let mut target = Node::rectangle(dst, Rect::new(300.0, 20.0, 60.0, 60.0));
    target.effects = vec![shadow(blue)];
    target.variables.bindings.insert("effects/0/color".into(), var_b.clone());
    s.shell.app.doc.insert(page, target).unwrap();

    s.shell.app.select_only(src);
    s.shell.app.copy_selection();
    s.shell.app.select_only(dst);
    let ctx = s.ctx.clone();
    crate::panels::menus::run(&mut s.shell.app, &ctx, Action::PasteAndMatchStyle);

    let pasted = s.app().doc.get(s.app().selection[0]).unwrap();
    assert_eq!(pasted.variables.bindings.get("effects/0/color"), Some(&var_b), "the pasted layer drops its own effect binding for the target's");
    let resolved = designos_doc::resolve_node(&s.app().doc, pasted);
    let Effect::DropShadow { color, .. } = &resolved.effects[0] else { panic!("expected a drop shadow") };
    assert_eq!(*color, blue, "after a variable pass the effect colour is the target's variable, not the source's");
}

#[test]
fn paste_and_match_style_keeps_the_target_text_style_on_text_inside_a_pasted_instance_after_a_sync_of_its_main() {
    let mut s = Sim::new();
    let page = s.app().page;
    let (main, other) = (s.by_name("Primary button"), s.by_name("Learn more"));
    let main_label = child_named(&s.app().doc, main, "Label");
    let other_label = child_named(&s.app().doc, other, "Label");
    let frame = s.shell.app.doc.alloc_id();
    s.shell.app.doc.insert(page, Node::frame(frame, Rect::new(20.0, 400.0, 260.0, 120.0))).unwrap();
    let cmd = create_instance(&mut s.shell.app.doc, main, frame, DV::new(10.0, 10.0));
    cmd.apply(&mut s.shell.app.doc).unwrap();

    let dst = s.shell.app.doc.alloc_id();
    let style = TextStyle { font_family: "Inter".into(), font_size: 14.0, font_weight: 400, fills: solid(20, 180, 90), ..Default::default() };
    s.shell.app.doc.insert(page, Node::text(dst, Rect::new(300.0, 400.0, 160.0, 30.0), "Target", style.clone())).unwrap();

    s.shell.app.select_only(frame);
    s.shell.app.copy_selection();
    s.shell.app.select_only(dst);
    let ctx = s.ctx.clone();
    crate::panels::menus::run(&mut s.shell.app, &ctx, Action::PasteAndMatchStyle);

    let root = s.app().doc.get(s.app().selection[0]).unwrap();
    let inst = root.children[0];
    assert!(matches!(s.app().doc.get(inst).unwrap().kind, NodeKind::Instance { .. }), "the pasted frame still holds an instance");
    let label = child_named(&s.app().doc, inst, "Label");
    let pasted = s.app().doc.get(label).unwrap();
    assert!(pasted.component.source.is_some(), "the label still mirrors the main's");
    assert_eq!(pasted.kind.any_text().unwrap().style, style, "the label takes the target's text style");
    assert!(pasted.component.overrides.contains(&OverrideField::TextStyle), "and the change is recorded as an override");

    s.shell.app.edit_nodes(&[main_label], |n| {
        if let Some(tp) = n.kind.any_text_mut() {
            tp.style.font_size = 40.0;
            tp.style.font_family = "Roboto".into();
        }
    });
    assert_eq!(s.app().doc.get(other_label).unwrap().kind.any_text().unwrap().style.font_size, 40.0, "an instance without the override follows the main");
    assert_eq!(s.app().doc.get(label).unwrap().kind.any_text().unwrap().style, style, "the pasted label keeps the target style after the sync");
}

#[test]
fn the_old_paste_and_match_style_chord_does_nothing() {
    let mut s = Sim::new();
    let page = s.app().page;
    let src = s.shell.app.doc.alloc_id();
    s.shell.app.doc.insert(page, Node::rectangle(src, Rect::new(20.0, 20.0, 60.0, 60.0))).unwrap();
    let dst = s.shell.app.doc.alloc_id();
    s.shell.app.doc.insert(page, Node::text(dst, Rect::new(300.0, 20.0, 160.0, 30.0), "Target", TextStyle::default())).unwrap();
    s.shell.app.select_only(src);
    s.shell.app.copy_selection();
    s.shell.app.select_only(dst);
    let (undo, count) = (s.app().hist.undo_depth(), s.count());

    let is_mac = s.shell.app.terminal.platform_is_mac_override.unwrap_or(cfg!(target_os = "macos"));
    s.mods = Modifiers { alt: true, ctrl: !is_mac, shift: true, mac_cmd: is_mac, command: true };
    s.key(Key::V);
    s.mods = Modifiers::NONE;

    assert_eq!((s.app().hist.undo_depth(), s.count()), (undo, count), "no key claims Paste and match style");
}
