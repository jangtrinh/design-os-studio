//! UI tests of the vector tools: shape builder, pen and pencil, vector edit
//! mode (vertices, marquee and lasso, bend, paint bucket, the vertex inspector)
//! and the flatten / outline stroke / boolean commands.

use super::*;
use crate::canvas::tools::Tool;
use crate::panels::menus::{self, Action};
use designos_doc::{BooleanOp, Color, Node, PathCmd, Rect as DRect, StrokeAlign, VectorNetwork};
use designos_render::vectorops::{arrange, node_paths, paths_area, paths_to_shapes, shapes_area, transform_shapes};
use std::collections::BTreeSet;

fn add_shape(s: &mut Sim, mut n: Node, name: &str, c: Color) -> NodeId {
    n.name = name.into();
    n.fills = vec![Paint::solid(c)];
    let page = s.app().page;
    let id = s.shell.app.doc.insert(page, n).unwrap();
    s.shell.app.changed();
    id
}

fn add_rect(s: &mut Sim, name: &str, x: f32, y: f32, w: f32, h: f32) -> NodeId {
    let id = s.shell.app.doc.alloc_id();
    add_shape(s, Node::rectangle(id, DRect::new(x, y, w, h)), name, Color::rgb(0x40, 0x90, 0xF0))
}

fn shape_area(s: &Sim, id: NodeId) -> f64 {
    let n = s.app().doc.get(id).unwrap();
    paths_area(&node_paths(&s.app().doc, n).unwrap(), 0.02)
}

fn vectors(s: &Sim) -> Vec<NodeId> {
    let page = s.app().page;
    s.app().doc.get(page).unwrap().children.iter().copied().filter(|id| matches!(s.app().doc.get(*id).unwrap().kind, NodeKind::Vector { .. })).collect()
}

fn network(s: &Sim, id: NodeId) -> VectorNetwork {
    match &s.app().doc.get(id).unwrap().kind {
        NodeKind::Vector { network: Some(n), .. } => n.clone(),
        k => panic!("no network on {k:?}"),
    }
}

fn mods(s: &mut Sim, m: Modifiers) {
    s.mods = m;
}

fn press_key_with(s: &mut Sim, m: Modifiers, key: Key) {
    s.mods = m;
    s.key(key);
    s.mods = Modifiers::NONE;
}

/// Page-space point to screen.
fn at(s: &Sim, x: f32, y: f32) -> Pos2 {
    s.screen(x, y)
}

// ---------------------------------------------------------------- shape builder
/// Two 200x150 rectangles overlapping in a 100x100 lens, selected, with the tool active.
fn builder_sim() -> (Sim, NodeId, NodeId) {
    let mut s = Sim::new();
    let a = add_rect(&mut s, "A", 300.0, 800.0, 200.0, 150.0);
    let b = add_rect(&mut s, "B", 400.0, 850.0, 200.0, 150.0);
    s.shell.app.selection = vec![a, b];
    s.frame(vec![]);
    press_key_with(&mut s, Modifiers::SHIFT, Key::M);
    assert_eq!(s.app().tool, Tool::ShapeBuilder);
    (s, a, b)
}

#[test]
fn the_shape_builder_needs_overlapping_shapes_selected() {
    let mut s = Sim::new();
    let a = add_rect(&mut s, "A", 300.0, 800.0, 200.0, 150.0);
    s.shell.app.selection = vec![a];
    press_key_with(&mut s, Modifiers::SHIFT, Key::M);
    assert_eq!(s.app().tool, Tool::Move, "one shape is not enough");
    assert!(s.app().canvas.builder.is_none());
    assert!(s.app().toast.is_some());
}

#[test]
fn hovering_a_region_highlights_it() {
    let (mut s, _, _) = builder_sim();
    let b = s.app().canvas.builder.as_ref().unwrap();
    assert_eq!(b.cells.len(), 3, "A only, the lens, B only");
    let p = at(&s, 450.0, 900.0);
    s.mv(p);
    let b = s.app().canvas.builder.as_ref().unwrap();
    let hot = b.hover.expect("a region is hovered");
    assert_eq!(b.cells[hot].members.len(), 2, "the lens");
    s.mv(at(&s, 320.0, 820.0));
    let b = s.app().canvas.builder.as_ref().unwrap();
    assert_eq!(b.cells[b.hover.unwrap()].members, vec![0]);
    s.mv(at(&s, 10.0, 10.0));
    assert!(s.app().canvas.builder.as_ref().unwrap().hover.is_none(), "nothing under the pointer");
}

#[test]
fn clicking_a_region_extracts_it_and_cuts_the_shapes_around_it() {
    let (mut s, a, b) = builder_sim();
    s.click(at(&s, 450.0, 900.0));
    assert!(s.app().canvas.builder.as_ref().unwrap().edited());
    s.key(Key::Enter);
    assert!(s.app().canvas.builder.is_none());
    assert_eq!(s.app().tool, Tool::Move);
    assert!(s.app().doc.get(a).is_none() && s.app().doc.get(b).is_none(), "the originals were replaced");
    let vs = vectors(&s);
    assert_eq!(vs.len(), 3, "the lens and the two L shapes");
    let mut areas: Vec<f64> = vs.iter().map(|v| shape_area(&s, *v)).collect();
    areas.sort_by(f64::total_cmp);
    assert!((areas[0] - 10000.0).abs() < 1.0 && (areas[1] - 20000.0).abs() < 1.0 && (areas[2] - 20000.0).abs() < 1.0, "{areas:?}");
    // the result is a vector network
    for v in &vs {
        assert!(!network(&s, *v).regions.is_empty());
    }
    // the whole operation is one undo step
    s.cmd_key(Key::Z);
    assert!(s.app().doc.get(a).is_some() && s.app().doc.get(b).is_some());
    assert!(vectors(&s).is_empty());
}

#[test]
fn dragging_across_regions_merges_them_into_one_shape() {
    let (mut s, _, _) = builder_sim();
    // A only -> the lens -> B only
    let (p0, p1, p2) = (at(&s, 320.0, 820.0), at(&s, 450.0, 900.0), at(&s, 580.0, 980.0));
    s.mv(p0);
    s.button(true);
    s.mv(p1);
    s.mv(p2);
    s.button(false);
    let b = s.app().canvas.builder.as_ref().unwrap();
    let groups: BTreeSet<usize> = b.group.iter().flatten().copied().collect();
    assert_eq!(groups.len(), 1, "all three regions share one merge group");
    s.key(Key::Enter);
    let vs = vectors(&s);
    assert_eq!(vs.len(), 1, "the union is the only shape left");
    assert!((shape_area(&s, vs[0]) - 50000.0).abs() < 1.0);
}

#[test]
fn alt_click_deletes_a_region() {
    let (mut s, _, _) = builder_sim();
    mods(&mut s, Modifiers::ALT);
    s.click(at(&s, 450.0, 900.0));
    mods(&mut s, Modifiers::NONE);
    let b = s.app().canvas.builder.as_ref().unwrap();
    assert_eq!(b.deleted.iter().filter(|d| **d).count(), 1);
    // a deleted region is no longer hoverable
    s.mv(at(&s, 450.0, 900.0));
    assert!(s.app().canvas.builder.as_ref().unwrap().hover.is_none());
    s.key(Key::Enter);
    let vs = vectors(&s);
    assert_eq!(vs.len(), 2, "two shapes with a hole notched out of each");
    for v in vs {
        assert!((shape_area(&s, v) - 20000.0).abs() < 1.0);
    }
}

#[test]
fn leaving_without_edits_keeps_the_shapes_and_switching_tools_commits() {
    let (mut s, a, b) = builder_sim();
    s.key(Key::Escape);
    assert_eq!(s.app().tool, Tool::Move);
    assert!(s.app().doc.get(a).is_some() && s.app().doc.get(b).is_some(), "nothing changed");
    assert!(!s.app().hist.can_undo());
    // Escape while brushing only drops the brush; the second one applies
    s.shell.app.selection = vec![a, b];
    press_key_with(&mut s, Modifiers::SHIFT, Key::M);
    s.click(at(&s, 450.0, 900.0));
    s.key(Key::V);
    assert_eq!(s.app().tool, Tool::Move, "V switches tool and applies the result");
    assert_eq!(vectors(&s).len(), 3);
}

// ---------------------------------------------------------------- pen
#[test]
fn the_pen_closes_the_path_on_its_first_point() {
    let mut s = Sim::new();
    s.key(Key::P);
    for (x, y) in [(300.0, 800.0), (420.0, 800.0), (360.0, 900.0)] {
        let p = at(&s, x, y);
        s.click(p);
    }
    assert!(s.app().canvas.pen.is_some(), "still drawing");
    let first = at(&s, 300.0, 800.0);
    s.click(first);
    assert!(s.app().canvas.pen.is_none(), "clicking the first point finished the path");
    assert_eq!(s.app().tool, Tool::Move);
    let id = s.app().selection[0];
    let net = network(&s, id);
    assert_eq!((net.vertices.len(), net.segments.len(), net.regions.len()), (3, 3, 1), "a closed, filled loop");
    let NodeKind::Vector { paths, .. } = &s.app().doc.get(id).unwrap().kind else { panic!() };
    assert!(matches!(paths[0].cmds.last(), Some(PathCmd::Close)));
    s.cmd_key(Key::Z);
    assert!(s.app().doc.get(id).is_none(), "one undo step");
}

#[test]
fn an_open_pen_path_stays_open() {
    let mut s = Sim::new();
    s.key(Key::P);
    for (x, y) in [(300.0, 800.0), (420.0, 800.0), (360.0, 900.0)] {
        let p = at(&s, x, y);
        s.click(p);
    }
    s.key(Key::Enter);
    let id = s.app().selection[0];
    let net = network(&s, id);
    assert!(net.regions.is_empty());
    assert_eq!(net.segments.len(), 2);
}

#[test]
fn shift_keeps_pen_segments_on_45_degree_steps() {
    let mut s = Sim::new();
    s.key(Key::P);
    let p0 = at(&s, 300.0, 800.0);
    s.click(p0);
    mods(&mut s, Modifiers::SHIFT);
    let p1 = at(&s, 400.0, 812.0);
    s.click(p1);
    let p2 = at(&s, 470.0, 880.0);
    s.click(p2);
    mods(&mut s, Modifiers::NONE);
    s.key(Key::Enter);
    let id = s.app().selection[0];
    let net = network(&s, id);
    let (a, b, c) = (net.pos(0), net.pos(1), net.pos(2));
    assert!((a.y - b.y).abs() < 1e-3, "the first segment is horizontal: {a:?} {b:?}");
    let (dx, dy) = (c.x - b.x, c.y - b.y);
    assert!((dx.abs() - dy.abs()).abs() < 1e-2 || dx.abs() < 1e-3 || dy.abs() < 1e-3, "the second is on a 45 degree step: {dx} {dy}");
}

// ---------------------------------------------------------------- pencil
fn scribble(s: &mut Sim, pts: &[Pos2]) {
    s.mv(pts[0]);
    s.button(true);
    for p in &pts[1..] {
        s.mv(*p);
    }
    s.button(false);
    s.frame(vec![]);
}

#[test]
fn the_pencil_smooths_a_wobbly_circle_into_a_closed_curve() {
    let mut s = Sim::new();
    press_key_with(&mut s, Modifiers::SHIFT, Key::P);
    assert_eq!(s.app().tool, Tool::Pencil);
    let c = at(&s, 450.0, 900.0);
    let pts: Vec<Pos2> = (0..=120)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / 120.0;
            let r = 70.0 + if i % 2 == 0 { 1.0 } else { -1.0 };
            Pos2::new(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect();
    scribble(&mut s, &pts);
    let vs = vectors(&s);
    assert_eq!(vs.len(), 1, "one stroke, one vector");
    let net = network(&s, vs[0]);
    assert!(net.vertices.len() <= 10, "{} anchors", net.vertices.len());
    assert!(net.vertices.len() >= 3, "{} anchors", net.vertices.len());
    assert_eq!(net.regions.len(), 1, "ending on its start closes the shape");
    // it is still a circle: the area is that of the drawn circle within a few percent
    let want = std::f64::consts::PI * 70.0 * 70.0 / (s.app().view.zoom as f64).powi(2);
    let got = shape_area(&s, vs[0]);
    assert!((got - want).abs() / want < 0.05, "{got} vs {want}");
    assert!(net.segments.iter().any(|g| !(g.ta == DV::ZERO && g.tb == DV::ZERO)), "curved");
    s.cmd_key(Key::Z);
    assert!(vectors(&s).is_empty());
}

#[test]
fn the_pencil_keeps_an_open_stroke_open() {
    let mut s = Sim::new();
    press_key_with(&mut s, Modifiers::SHIFT, Key::P);
    let o = at(&s, 320.0, 820.0);
    let pts: Vec<Pos2> = (0..=40).map(|i| Pos2::new(o.x + i as f32 * 3.0, o.y + (i as f32 * 0.25).sin() * 30.0)).collect();
    scribble(&mut s, &pts);
    let vs = vectors(&s);
    let net = network(&s, vs[0]);
    assert!(net.regions.is_empty());
    assert!(net.vertices.len() < 12, "simplified from 41 raw points: {}", net.vertices.len());
}

// ---------------------------------------------------------------- vector edit mode
fn editing() -> (Sim, NodeId) {
    let (mut s, id) = sim_with_vector();
    s.key(Key::Enter);
    assert!(s.app().canvas.vedit.is_some());
    (s, id)
}

fn sel_of(s: &Sim) -> BTreeSet<usize> {
    s.app().canvas.vedit.as_ref().unwrap().sel.clone()
}

#[test]
fn marquee_selects_vertices_and_a_click_on_empty_space_clears() {
    let (mut s, _) = editing();
    let (a, b) = (at(&s, 250.0, 780.0), at(&s, 400.0, 850.0));
    s.drag(a, b);
    assert_eq!(sel_of(&s), BTreeSet::from([0, 1]), "the first two vertices are inside");
    // shift adds the third
    mods(&mut s, Modifiers::SHIFT);
    s.drag(at(&s, 250.0, 860.0), at(&s, 340.0, 900.0));
    mods(&mut s, Modifiers::NONE);
    assert_eq!(sel_of(&s), BTreeSet::from([0, 1, 2]));
    s.click(at(&s, 650.0, 900.0));
    assert!(sel_of(&s).is_empty());
    assert!(s.app().canvas.vedit.is_some(), "still editing");
}

/// Screen position of the scale handle `h` of the selected points' box, found by probing the
/// canvas hit test, so the tests do not depend on where the handle is drawn.
fn scale_handle_at(s: &Sim, h: crate::canvas::handles::Handle) -> Pos2 {
    use crate::canvas::vector::{hit_test, Hit};
    let app = s.app();
    let v = app.canvas.vedit.as_ref().expect("editing");
    let world = app.doc.world_transform(v.id);
    let to_s = |p: DV| {
        let q = app.view.to_screen(app.canvas_rect.min, world.apply(p));
        DV::new(q.x, q.y)
    };
    let pts: Vec<DV> = v.sel.iter().map(|i| to_s(v.net.vertices[*i].pos)).collect();
    let (x0, x1) = (pts.iter().map(|p| p.x).fold(f32::MAX, f32::min) - 60.0, pts.iter().map(|p| p.x).fold(f32::MIN, f32::max) + 60.0);
    let (y0, y1) = (pts.iter().map(|p| p.y).fold(f32::MAX, f32::min) - 60.0, pts.iter().map(|p| p.y).fold(f32::MIN, f32::max) + 60.0);
    let (mut sum, mut n) = (DV::new(0.0, 0.0), 0u32);
    let mut y = y0;
    while y <= y1 {
        let mut x = x0;
        while x <= x1 {
            let hit = hit_test(&v.net, &v.sel, v.seg.map(|g| g.0), &to_s, DV::new(x, y), crate::theme::VEDIT_HIT, crate::theme::VEDIT_SEGMENT_HIT);
            if hit == Some(Hit::BoxHandle(h)) {
                sum = DV::new(sum.x + x, sum.y + y);
                n += 1;
            }
            x += 1.0;
        }
        y += 1.0;
    }
    assert!(n > 0, "the {h:?} scale handle cannot be hit");
    Pos2::new(sum.x / n as f32, sum.y / n as f32)
}

#[test]
fn scaling_selected_points_bounding_box_scales_only_selected_points() {
    let (mut s, id) = editing();
    // Split segment 0 ((300,800) -> (370,840)) at t=0.5 so vertex 3 is at (335, 820)
    crate::canvas::vector::insert_point(&mut s.shell.app, 0, 0.5);
    // Select vertices 0 ((300,800)), 3 ((335,820)), and 1 ((370,840)); leave vertex 2 ((300,880)) unselected
    s.shell.app.canvas.vedit.as_mut().unwrap().sel = BTreeSet::from([0, 1, 3]);
    s.frame(vec![]);

    // Drag the BR corner handle of the selected points' box by the pointer travel from (370, 840)
    // to (440, 880): the handle ends 2x as far from the TL anchor in X and Y
    let from = scale_handle_at(&s, crate::canvas::handles::Handle::BR);
    let travel = at(&s, 440.0, 880.0) - at(&s, 370.0, 840.0);
    s.drag(from, from + travel);

    let w = s.app().doc.world_transform(id);
    let net = network(&s, id);
    let p0 = w.apply(net.vertices[0].pos);
    let p3 = w.apply(net.vertices[3].pos);
    let p1 = w.apply(net.vertices[1].pos);
    let p2 = w.apply(net.vertices[2].pos);

    assert!((p0.x - 300.0).abs() < 1.0 && (p0.y - 800.0).abs() < 1.0, "anchor TL stayed at (300,800), got {p0:?}");
    assert!((p3.x - 370.0).abs() < 1.0 && (p3.y - 840.0).abs() < 1.0, "midpoint scaled 2x to (370,840), got {p3:?}");
    assert!((p1.x - 440.0).abs() < 1.0 && (p1.y - 880.0).abs() < 1.0, "BR corner scaled 2x to (440,880), got {p1:?}");
    assert!((p2.x - 300.0).abs() < 1.0 && (p2.y - 880.0).abs() < 1.0, "unselected vertex stayed at (300,880), got {p2:?}");

    // Undo restores all vertices in one step
    s.cmd_key(Key::Z);
    let w_undo = s.app().doc.world_transform(id);
    let net_undo = network(&s, id);
    let p1_undo = w_undo.apply(net_undo.vertices[1].pos);
    let p3_undo = w_undo.apply(net_undo.vertices[3].pos);
    assert!((p1_undo.x - 370.0).abs() < 1.0 && (p1_undo.y - 840.0).abs() < 1.0, "{p1_undo:?}");
    assert!((p3_undo.x - 335.0).abs() < 1.0 && (p3_undo.y - 820.0).abs() < 1.0, "{p3_undo:?}");
}

#[test]
fn dragging_a_selected_vertex_of_a_multi_point_selection_moves_the_whole_selection() {
    let (mut s, id) = editing();
    // marquee the first two vertices: (300, 800) and (370, 840) are the corners of their box
    s.drag(at(&s, 250.0, 780.0), at(&s, 400.0, 850.0));
    assert_eq!(sel_of(&s), BTreeSet::from([0, 1]));

    // dragging a selected vertex moves every selected vertex, even one that is a corner of the box
    let from = at(&s, 370.0, 840.0);
    s.drag(from, from + (at(&s, 390.0, 840.0) - at(&s, 370.0, 840.0)));

    let w = s.app().doc.world_transform(id);
    let net = network(&s, id);
    let (p0, p1, p2) = (w.apply(net.vertices[0].pos), w.apply(net.vertices[1].pos), w.apply(net.vertices[2].pos));
    assert!((p0.x - 320.0).abs() < 1.0 && (p0.y - 800.0).abs() < 1.0, "the other selected vertex followed, got {p0:?}");
    assert!((p1.x - 390.0).abs() < 1.0 && (p1.y - 840.0).abs() < 1.0, "the dragged vertex moved by 20, got {p1:?}");
    assert!((p2.x - 300.0).abs() < 1.0 && (p2.y - 880.0).abs() < 1.0, "the unselected vertex stayed, got {p2:?}");
}

#[test]
fn alt_drag_lassos_the_vertices_it_encloses() {
    let (mut s, _) = editing();
    mods(&mut s, Modifiers::ALT);
    let pts = [at(&s, 345.0, 815.0), at(&s, 400.0, 815.0), at(&s, 400.0, 865.0), at(&s, 345.0, 865.0)];
    s.mv(pts[0]);
    s.button(true);
    for p in &pts[1..] {
        s.mv(*p);
    }
    s.mv(pts[0]);
    s.button(false);
    mods(&mut s, Modifiers::NONE);
    assert_eq!(sel_of(&s), BTreeSet::from([1]), "only the middle vertex is inside the loop");
}

#[test]
fn ctrl_drag_bends_a_segment_in_one_undo_step() {
    let (mut s, id) = editing();
    let before = network(&s, id);
    assert!(before.is_straight(0));
    mods(&mut s, Modifiers::COMMAND);
    let a = at(&s, 335.0, 820.0);
    s.drag(a, a + Vec2::new(25.0, -20.0));
    mods(&mut s, Modifiers::NONE);
    let after = network(&s, id);
    assert!(!after.is_straight(0), "the segment is curved now");
    assert_eq!(after.vertices.len(), before.vertices.len(), "bending adds no vertices");
    s.cmd_key(Key::Z);
    assert!(s.app().canvas.vedit.is_some(), "still editing");
    assert!(s.app().canvas.vedit.as_ref().unwrap().net.is_straight(0), "undo restored the straight segment");
}

#[test]
fn the_bend_tool_pulls_handles_out_of_a_vertex() {
    let (mut s, id) = editing();
    s.key(Key::B);
    assert_eq!(s.app().canvas.vedit.as_ref().unwrap().tool, crate::canvas::vector::VTool::Bend);
    let v = at(&s, 370.0, 840.0);
    s.drag(v, v + Vec2::new(0.0, 40.0));
    let net = network(&s, id);
    assert!(net.vertices[1].mirror == designos_doc::HandleMirror::AngleAndLength);
    assert!(net.segments.iter().any(|g| g.ta != DV::ZERO || g.tb != DV::ZERO));
    // a click on the vertex straightens it again
    s.click(v);
    let net = network(&s, id);
    assert!(net.segments.iter().all(|g| g.ta == DV::ZERO && g.tb == DV::ZERO));
}

#[test]
fn the_paint_bucket_fills_and_unfills_an_enclosed_area() {
    let mut s = Sim::new();
    s.key(Key::P);
    for (x, y) in [(300.0, 800.0), (440.0, 800.0), (370.0, 920.0)] {
        let p = at(&s, x, y);
        s.click(p);
    }
    let first = at(&s, 300.0, 800.0);
    s.click(first);
    let id = s.app().selection[0];
    s.key(Key::Enter);
    assert!(s.app().canvas.vedit.is_some());
    s.key(Key::K);
    assert_eq!(s.app().canvas.vedit.as_ref().unwrap().tool, crate::canvas::vector::VTool::Bucket);
    // hovering shows the area
    s.mv(at(&s, 370.0, 830.0));
    assert!(s.app().canvas.vedit.as_ref().unwrap().hover_face.is_some());
    s.click(at(&s, 370.0, 830.0));
    assert!(network(&s, id).regions.is_empty(), "the loop was filled, now it is not");
    assert!(matches!(&s.app().doc.get(id).unwrap().kind, NodeKind::Vector { network: Some(n), .. } if n.has_stroke_only_segments()));
    s.click(at(&s, 370.0, 830.0));
    assert_eq!(network(&s, id).regions.len(), 1, "filled again");
    s.cmd_key(Key::Z);
    assert!(network(&s, id).regions.is_empty());
    // clicking outside the shape does nothing
    let n0 = s.app().hist.can_undo();
    s.click(at(&s, 700.0, 960.0));
    assert_eq!(s.app().hist.can_undo(), n0);
}

#[test]
fn the_vertex_inspector_edits_the_corner_radius_as_one_undo_step() {
    let (mut s, id) = editing();
    s.click(at(&s, 370.0, 840.0));
    assert_eq!(sel_of(&s), BTreeSet::from([1]));
    let r = s.field_rect("vtx-radius");
    s.click(Pos2::new(r.left() + 40.0, r.center().y));
    s.type_text("30");
    s.key(Key::Enter);
    let net = network(&s, id);
    assert_eq!(net.vertices[1].corner_radius, 30.0);
    let NodeKind::Vector { paths, .. } = &s.app().doc.get(id).unwrap().kind else { panic!() };
    assert!(paths[0].cmds.iter().any(|c| matches!(c, PathCmd::CubicTo(..))), "the corner is rounded in the derived path");
    assert!(s.app().canvas.vedit.is_some(), "Enter in the field does not leave edit mode");
    s.cmd_key(Key::Z);
    assert_eq!(s.app().canvas.vedit.as_ref().unwrap().net.vertices[1].corner_radius, 0.0);
}

#[test]
fn the_vertex_inspector_offers_cap_and_handle_modes_and_commands() {
    let (mut s, _) = editing();
    s.click(at(&s, 370.0, 840.0));
    for key in ["vtx-cap", "vtx-mirror"] {
        s.rect_of(key);
    }
    for key in ["vtx-join", "vtx-split", "vtx-simplify"] {
        s.rect_of(key);
    }
    // Split cuts the path at the selected vertex through the button
    let r = s.rect_of("vtx-split");
    s.click(r.center());
    assert_eq!(s.app().canvas.vedit.as_ref().unwrap().net.vertices.len(), 4);
    // the on-canvas sub-tool switcher
    let r = s.rect_of("vtool:Paint bucket");
    s.click(r.center());
    assert_eq!(s.app().canvas.vedit.as_ref().unwrap().tool, crate::canvas::vector::VTool::Bucket);
    let r = s.rect_of("vtool:Move");
    s.click(r.center());
    assert_eq!(s.app().canvas.vedit.as_ref().unwrap().tool, crate::canvas::vector::VTool::Move);
}

#[test]
fn join_closes_two_selected_endpoints_with_ctrl_j() {
    let (mut s, id) = editing();
    // select both ends of the open path with a marquee over the first and last vertex
    let mut sel = BTreeSet::new();
    sel.extend([0usize, 2]);
    s.shell.app.canvas.vedit.as_mut().unwrap().sel = sel;
    press_key_with(&mut s, Modifiers::COMMAND, Key::J);
    let net = network(&s, id);
    assert_eq!(net.vertices.len(), 2, "the two ends merged into one");
    assert_eq!(net.segments.len(), 2);
}

#[test]
fn vertices_move_with_the_arrow_keys_and_delete_removes_them() {
    let (mut s, id) = editing();
    s.click(at(&s, 370.0, 840.0));
    let w0 = s.app().doc.world_transform(id).apply(s.app().canvas.vedit.as_ref().unwrap().net.pos(1));
    s.key(Key::ArrowRight);
    let w1 = s.app().doc.world_transform(id).apply(s.app().canvas.vedit.as_ref().unwrap().net.pos(1));
    assert!((w1.x - w0.x - theme::NUDGE).abs() < 1e-3 && (w1.y - w0.y).abs() < 1e-3, "{w0:?} -> {w1:?}");
    s.key(Key::Delete);
    assert_eq!(network(&s, id).vertices.len(), 2, "the path is joined over the deleted vertex");
}

#[test]
fn double_clicking_a_rectangle_enters_vertex_editing() {
    let mut s = Sim::new();
    let r = add_rect(&mut s, "Plain", 300.0, 800.0, 200.0, 150.0);
    let p = at(&s, 400.0, 880.0);
    s.click(p);
    s.click(p);
    assert_eq!(s.app().canvas.vedit.as_ref().map(|v| v.id), Some(r), "double-click edits the shape");
    assert_eq!(s.app().canvas.vedit.as_ref().unwrap().net.vertices.len(), 4);
    assert!(matches!(s.app().doc.get(r).unwrap().kind, NodeKind::Rectangle), "entering alone converts nothing");
}

#[test]
fn double_clicking_inside_a_group_selects_the_child_before_editing_it() {
    let mut s = Sim::new();
    let a = add_rect(&mut s, "In group", 300.0, 800.0, 200.0, 150.0);
    let b = add_rect(&mut s, "Other", 560.0, 800.0, 100.0, 150.0);
    s.shell.app.selection = vec![a, b];
    s.shell.app.group_selection(false);
    let p = at(&s, 400.0, 880.0);
    s.click(p);
    assert_ne!(s.app().selection, vec![a], "the first click selects the group");
    s.click(p);
    assert_eq!(s.app().selection, vec![a], "the double-click steps into the group");
    assert!(s.app().canvas.vedit.is_none(), "stepping into the group does not start editing");
    s.key(Key::Enter);
    assert_eq!(s.app().canvas.vedit.as_ref().map(|v| v.id), Some(a), "Enter edits the selected shape");
}

// ---------------------------------------------------------------- flatten, outline, boolean
#[test]
fn flatten_merges_a_boolean_group_into_one_vector() {
    let mut s = Sim::new();
    let a = add_rect(&mut s, "A", 300.0, 800.0, 100.0, 100.0);
    let b = add_rect(&mut s, "B", 350.0, 850.0, 100.0, 100.0);
    s.shell.app.selection = vec![a, b];
    let ctx = s.ctx.clone();
    menus::run(&mut s.shell.app, &ctx, Action::Boolean(BooleanOp::Subtract));
    let g = s.app().selection[0];
    assert!(matches!(s.app().doc.get(g).unwrap().kind, NodeKind::BooleanOp(BooleanOp::Subtract)));
    assert!(s.app().doc.get(a).unwrap().parent == Some(g), "the layers moved into the group");
    press_key_with(&mut s, Modifiers::COMMAND, Key::E);
    assert!(s.app().doc.get(g).is_none() && s.app().doc.get(a).is_none(), "the group and its layers are gone");
    let vs = vectors(&s);
    assert_eq!(vs.len(), 1);
    assert!((shape_area(&s, vs[0]) - 7500.0).abs() < 1.0, "the computed subtraction");
    assert_eq!(s.app().selection, vs);
    assert!(!network(&s, vs[0]).vertices.is_empty());
    s.cmd_key(Key::Z);
    assert!(s.app().doc.get(g).is_some() && s.app().doc.get(a).is_some(), "one undo step brings the group back");
}

#[test]
fn editing_a_child_of_a_boolean_group_in_place_then_flattening() {
    let mut s = Sim::new();
    let a = add_rect(&mut s, "A", 300.0, 800.0, 100.0, 100.0);
    let b = add_rect(&mut s, "B", 350.0, 850.0, 100.0, 100.0);
    s.shell.app.selection = vec![a, b];
    let ctx = s.ctx.clone();
    menus::run(&mut s.shell.app, &ctx, Action::Boolean(BooleanOp::Subtract));
    let g = s.app().selection[0];
    // step into the group and edit the first child's corner
    s.shell.app.select_only(a);
    s.key(Key::Enter);
    assert_eq!(s.app().canvas.vedit.as_ref().map(|v| v.id), Some(a), "the child is edited in place");
    s.shell.app.canvas.vedit.as_mut().unwrap().sel = BTreeSet::from([2]);
    crate::canvas::vector::nudge(&mut s.shell.app, DV::new(40.0, 40.0));
    s.key(Key::Escape);
    assert!(matches!(s.app().doc.get(a).unwrap().kind, NodeKind::Vector { .. }), "the rectangle became a vector");
    assert_eq!(s.app().doc.get(a).unwrap().parent, Some(g), "still inside the group");
    // the group's geometry follows the edit
    let doc = &s.app().doc;
    let mut a_shape = paths_to_shapes(&node_paths(doc, doc.get(a).unwrap()).unwrap(), 0.02);
    transform_shapes(&mut a_shape, &doc.world_transform(a));
    let mut b_shape = paths_to_shapes(&node_paths(doc, doc.get(b).unwrap()).unwrap(), 0.02);
    transform_shapes(&mut b_shape, &doc.world_transform(b));
    let expect: f64 = arrange(&[a_shape, b_shape]).iter().filter(|c| c.members == vec![0]).map(|c| c.area).sum();
    s.shell.app.selection = vec![g];
    press_key_with(&mut s, Modifiers::COMMAND, Key::E);
    let vs = vectors(&s);
    assert_eq!(vs.len(), 1);
    let got = shape_area(&s, vs[0]);
    assert!((got - expect).abs() < 2.0, "{got} vs {expect}");
    assert!(got > 7500.0, "the grown rectangle leaves more behind: {got}");
    let _ = shapes_area;
}

#[test]
fn outline_stroke_turns_the_stroke_into_a_filled_vector() {
    let mut s = Sim::new();
    let id = s.shell.app.doc.alloc_id();
    let mut n = Node::rectangle(id, DRect::new(300.0, 800.0, 100.0, 50.0));
    n.fills.clear();
    n.stroke.paints = vec![Paint::solid(Color::rgb(0xFF, 0x00, 0x00))];
    n.stroke.weight = 10.0;
    n.stroke.align = StrokeAlign::Center;
    let page = s.app().page;
    let id = s.shell.app.doc.insert(page, n).unwrap();
    s.shell.app.selection = vec![id];
    press_key_with(&mut s, Modifiers::COMMAND | Modifiers::ALT, Key::O);
    let vs = vectors(&s);
    assert_eq!(vs.len(), 1);
    assert!(s.app().doc.get(id).is_none(), "a stroke-only layer is replaced");
    let v = s.app().doc.get(vs[0]).unwrap();
    assert!(v.stroke.paints.is_empty(), "no stroke left");
    assert!(matches!(&v.fills[0].kind, designos_doc::PaintKind::Solid(c) if c.r > 0.9), "the stroke colour is the fill now");
    assert!((shape_area(&s, vs[0]) - (110.0 * 60.0 - 90.0 * 40.0)).abs() < 1.5);
    s.cmd_key(Key::Z);
    assert!(s.app().doc.get(id).is_some() && vectors(&s).is_empty());
}

#[test]
fn outline_stroke_keeps_a_filled_shape_and_adds_the_outline_above() {
    let mut s = Sim::new();
    let id = s.shell.app.doc.alloc_id();
    let mut n = Node::rectangle(id, DRect::new(300.0, 800.0, 100.0, 50.0));
    n.stroke.paints = vec![Paint::solid(Color::BLACK)];
    n.stroke.weight = 4.0;
    n.stroke.align = StrokeAlign::Inside;
    let page = s.app().page;
    let id = s.shell.app.doc.insert(page, n).unwrap();
    s.shell.app.selection = vec![id];
    press_key_with(&mut s, Modifiers::COMMAND | Modifiers::ALT, Key::O);
    let kept = s.app().doc.get(id).unwrap();
    assert!(kept.stroke.paints.is_empty() && !kept.fills.is_empty(), "the shape keeps its fill");
    let vs = vectors(&s);
    assert_eq!(vs.len(), 1);
    assert!((shape_area(&s, vs[0]) - (5000.0 - 92.0 * 42.0)).abs() < 1.5, "an inside stroke of weight 4");
    let page_children = &s.app().doc.get(page).unwrap().children;
    let (i, j) = (page_children.iter().position(|c| *c == id).unwrap(), page_children.iter().position(|c| *c == vs[0]).unwrap());
    assert_eq!(j, i + 1, "the outline sits right above the shape");
}

#[test]
fn flatten_a_single_vector_bakes_its_corner_radii() {
    let (mut s, id) = sim_with_vector();
    s.key(Key::Enter);
    s.shell.app.canvas.vedit.as_mut().unwrap().sel = BTreeSet::from([1]);
    crate::canvas::vector::set_vertices(&mut s.shell.app, |v| v.corner_radius = 12.0);
    s.key(Key::Escape);
    press_key_with(&mut s, Modifiers::COMMAND, Key::E);
    let vs = vectors(&s);
    assert_eq!(vs.len(), 1);
    let net = network(&s, vs[0]);
    assert!(net.vertices.iter().all(|v| v.corner_radius == 0.0), "radii are baked into the geometry");
    assert!(net.segments.iter().any(|g| g.ta != DV::ZERO || g.tb != DV::ZERO), "the rounded corner became real curves");
    assert_ne!(vs[0], id);
}

#[test]
fn simplify_path_from_the_menu_removes_nodes() {
    let mut s = Sim::new();
    press_key_with(&mut s, Modifiers::SHIFT, Key::P);
    let o = at(&s, 320.0, 820.0);
    let pts: Vec<Pos2> = (0..=60).map(|i| Pos2::new(o.x + i as f32 * 2.0, o.y + (i as f32 * 0.2).sin() * 20.0)).collect();
    scribble(&mut s, &pts);
    let id = vectors(&s)[0];
    // add many points back by editing a polyline: use a dense path through the document
    let dense: Vec<designos_doc::Path> = vec![designos_doc::Path {
        cmds: std::iter::once(PathCmd::MoveTo(DV::new(0.0, 0.0)))
            .chain((1..=50).map(|i| PathCmd::LineTo(DV::new(i as f32 * 2.0, (i as f32 * 0.15).sin() * 20.0))))
            .collect(),
        fill_rule: designos_doc::FillRule::NonZero,
    }];
    if let Some(n) = s.shell.app.doc.get_mut(id) {
        n.kind = NodeKind::Vector { paths: dense, network: None };
    }
    s.shell.app.selection = vec![id];
    let ctx = s.ctx.clone();
    menus::run(&mut s.shell.app, &ctx, Action::VectorSimplify);
    let net = network(&s, id);
    assert!(net.vertices.len() < 20, "{}", net.vertices.len());
}

// ---------------------------------------------------------------- fixtures for screenshots
#[test]
fn the_shape_builder_fixture_hovers_the_lens() {
    let s = Sim::with(Options { fixture: Some("shape-builder".into()), ..Default::default() });
    assert_eq!(s.app().tool, Tool::ShapeBuilder);
    let b = s.app().canvas.builder.as_ref().expect("the session is running");
    assert_eq!(b.cells.len(), 7, "three overlapping shapes");
    assert_eq!(b.cells[b.hover.unwrap()].members, vec![0, 1]);
}

#[test]
fn the_vector_edit_fixture_shows_the_corner_radius_field() {
    let s = Sim::with(Options { fixture: Some("vector-edit".into()), ..Default::default() });
    let v = s.app().canvas.vedit.as_ref().expect("editing");
    assert_eq!(v.sel, BTreeSet::from([1]));
    s.field_rect("vtx-radius");
    assert_eq!(v.net.vertices[1].corner_radius, 22.0);
}
