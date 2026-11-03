//! Issue #1477: Edit menu parity tests for:
//! - `Paste and match style` (`Ctrl+Shift+Alt+V` / `⇧⌥⌘V`)
//! - `Swap fill and stroke` (`Shift+X` / `⇧X`)
//! - `Select all with same text properties`, `Select all with same properties`, `Select all with same variant`
//! - Edit menu, context menu, and command palette labels, shortcuts, and enabled states.

use super::*;
use crate::panels::menus::Action;
use designos_doc::find::MatchCriteria;
use designos_doc::{
    create_instance, BlendMode, Color, Effect, FrameProps, GradientStop, LineHeight, Node, NodeKind, Paint, PaintKind, Rect, Stroke, Style, StyleMeta,
    StyleProperty, TextStyle, Vec2 as DV,
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

    // Also verify shortcut Cmd+Shift+Alt+V inside an open text editing session pastes plain text with the target run's style.
    crate::canvas::textsession::begin_text_edit(&mut s.shell.app, dst_id);
    assert!(s.app().canvas.text.is_some());
    let is_mac = s.shell.app.terminal.platform_is_mac_override.unwrap_or(cfg!(target_os = "macos"));
    s.mods = Modifiers { alt: true, ctrl: !is_mac, shift: true, mac_cmd: is_mac, command: true };
    s.key(Key::V);
    s.mods = Modifiers::NONE;
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

    assert_eq!(check_cmd(s.app(), "Paste and match style"), ("Ctrl+Shift+Alt+V".into(), true));
    assert_eq!(check_cmd(s.app(), "Swap fill and stroke"), ("Shift+X".into(), true));
    assert_eq!(check_cmd(s.app(), "Select all with same text properties"), ("".into(), true));
    assert_eq!(check_cmd(s.app(), "Select all with same properties"), ("".into(), true));
    assert_eq!(check_cmd(s.app(), "Select all with same variant"), ("".into(), true));

    // Rectangle selected (not copied yet): Swap fill and stroke & Select same properties enabled;
    // Paste and match style, Select same text properties, and Select same variant disabled.
    s.shell.app.select_only(rect_id);
    assert_eq!(check_cmd(s.app(), "Swap fill and stroke"), ("Shift+X".into(), false));
    assert_eq!(check_cmd(s.app(), "Select all with same properties"), ("".into(), false));
    assert_eq!(check_cmd(s.app(), "Paste and match style"), ("Ctrl+Shift+Alt+V".into(), true));
    assert_eq!(check_cmd(s.app(), "Select all with same text properties"), ("".into(), true));
    assert_eq!(check_cmd(s.app(), "Select all with same variant"), ("".into(), true));

    // Copy rectangle, then select text node: Paste and match style and Select same text properties enabled.
    s.shell.app.copy_selection();
    s.shell.app.select_only(text_id);
    assert_eq!(check_cmd(s.app(), "Paste and match style"), ("Ctrl+Shift+Alt+V".into(), false));
    assert_eq!(check_cmd(s.app(), "Select all with same text properties"), ("".into(), false));

    // Select variant instance: Select all with same variant enabled.
    s.shell.app.select_only(var_inst_id);
    assert_eq!(check_cmd(s.app(), "Select all with same variant"), ("".into(), false));
}
