//! A vector or a boolean group draws the geometry it stores in its own space, so resizing one has to
//! scale that geometry with the box: from the handles, from the W and H fields, with Cmd held, with the
//! Scale tool, and when an auto layout hands a Fill slot a new size. Undo puts the old geometry back.

use super::*;
use crate::canvas::tools::Tool;
use designos_doc::{ArcData, AutoLayout, BooleanOp, Node, Path, PathCmd, Rect as DRect, Sizing};
use designos_render::vectorops::node_paths;

const STROKE: f32 = 4.0;
/// How far the drawn shape may miss its box, in page units.
const SLACK: f32 = 0.6;

fn add(s: &mut Sim, mut n: Node, name: &str) -> NodeId {
    n.name = name.into();
    n.fills = vec![Paint::solid(Color::rgb(0x40, 0x90, 0xF0))];
    n.stroke.paints = vec![Paint::solid(Color::rgb(0x20, 0x20, 0x20))];
    n.stroke.weight = STROKE;
    let page = s.app().page;
    let id = s.shell.app.doc.insert(page, n).unwrap();
    s.shell.app.changed();
    id
}

/// A star flattened into a vector, selected: the node the issue is about.
fn flattened_star(s: &mut Sim) -> NodeId {
    let id = s.shell.app.doc.alloc_id();
    let mut star = Node::new(id, NodeKind::Star { count: 5, inner_ratio: 0.4 });
    star.transform = designos_doc::Affine::translate(300.0, 300.0);
    star.size = DV::new(100.0, 100.0);
    let star = add(s, star, "Star");
    s.shell.app.selection = vec![star];
    s.shell.app.flatten_selection();
    let [flat] = s.app().selection[..] else { panic!("flatten leaves one vector selected") };
    assert!(matches!(s.app().doc.get(flat).unwrap().kind, NodeKind::Vector { .. }));
    s.frame(vec![]);
    flat
}

/// Two overlapping rectangles united into a boolean group, selected.
fn union_of_two_rects(s: &mut Sim) -> NodeId {
    let a = s.shell.app.doc.alloc_id();
    let a = add(s, Node::rectangle(a, DRect::new(300.0, 300.0, 100.0, 80.0)), "A");
    let b = s.shell.app.doc.alloc_id();
    let b = add(s, Node::rectangle(b, DRect::new(360.0, 340.0, 120.0, 90.0)), "B");
    s.shell.app.selection = vec![a, b];
    s.shell.app.boolean_selection(BooleanOp::Union);
    let [group] = s.app().selection[..] else { panic!("the boolean leaves one group selected") };
    s.frame(vec![]);
    group
}

/// Min and max corner of every point in `paths`.
fn ink_of(paths: &[Path]) -> (DV, DV) {
    let (mut lo, mut hi) = (DV::new(f32::MAX, f32::MAX), DV::new(f32::MIN, f32::MIN));
    for cmd in paths.iter().flat_map(|p| &p.cmds) {
        let pts: &[DV] = match cmd {
            PathCmd::MoveTo(p) | PathCmd::LineTo(p) => std::slice::from_ref(p),
            PathCmd::QuadTo(c, p) => &[*c, *p],
            PathCmd::CubicTo(c1, c2, p) => &[*c1, *c2, *p],
            PathCmd::Close => &[],
        };
        for p in pts {
            lo = DV::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = DV::new(hi.x.max(p.x), hi.y.max(p.y));
        }
    }
    (lo, hi)
}

/// What the renderers draw for `id`, as the corners of its ink in the node's own space.
fn ink(doc: &designos_doc::Document, id: NodeId) -> (DV, DV) {
    let n = doc.get(id).unwrap();
    match &n.kind {
        NodeKind::Vector { paths, .. } => ink_of(paths),
        _ => ink_of(&node_paths(doc, n).expect("a boolean group has geometry")),
    }
}

/// The drawn shape starts at the node origin and fills the node box.
#[track_caller]
fn assert_fills_its_box(s: &Sim, id: NodeId, what: &str) {
    let size = s.app().doc.get(id).unwrap().size;
    let (lo, hi) = ink(&s.app().doc, id);
    assert!(
        lo.x.abs() < SLACK && lo.y.abs() < SLACK && (hi.x - size.x).abs() < SLACK && (hi.y - size.y).abs() < SLACK,
        "{what}: the drawn shape ({lo:?} to {hi:?}) must fill the box {size:?}"
    );
}

fn corner_on_screen(s: &Sim, corner: usize) -> Pos2 {
    let (corners, _) = crate::canvas::interact::selection_box(s.app()).unwrap();
    s.app().view.to_screen(s.app().canvas_rect.min, corners[corner])
}

/// Drags the bottom-right handle of the selection 60 px right and 30 px down, on the usual multi-frame gesture.
fn drag_bottom_right(s: &mut Sim) {
    let br = corner_on_screen(s, 2);
    s.drag(br, br + Vec2::new(60.0, 30.0));
}

fn type_into(s: &mut Sim, field: &str, text: &str) {
    let r = s.field_rect(field);
    s.click(Pos2::new(r.left() + 40.0, r.center().y));
    s.type_text(text);
    s.key(Key::Enter);
}

#[test]
fn dragging_a_handle_scales_a_flattened_star_to_the_new_box() {
    let mut s = Sim::new();
    let star = flattened_star(&mut s);
    assert_fills_its_box(&s, star, "before the drag");
    let (before, depth) = (s.app().doc.get(star).unwrap().clone(), s.app().hist.undo_depth());

    drag_bottom_right(&mut s);

    let n = s.app().doc.get(star).unwrap();
    assert!(n.size.x > before.size.x + 20.0 && n.size.y > before.size.y + 10.0, "the handle grew the box: {:?}", n.size);
    assert_fills_its_box(&s, star, "after the drag");
    assert_eq!(n.stroke.weight, STROKE, "a plain resize leaves the stroke weight alone");
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "one gesture is one undo step");

    s.cmd_key(Key::Z);
    assert_eq!(*s.app().doc.get(star).unwrap(), before, "undo brings back the box and the geometry in one step");
}

#[test]
fn typing_a_width_and_a_height_scales_a_flattened_star() {
    let mut s = Sim::new();
    let star = flattened_star(&mut s);
    let (before, depth) = (s.app().doc.get(star).unwrap().clone(), s.app().hist.undo_depth());

    type_into(&mut s, "w", "250");
    assert_eq!(s.app().doc.get(star).unwrap().size.x, 250.0);
    assert_fills_its_box(&s, star, "after the W field");

    type_into(&mut s, "h", "40");
    assert_eq!(s.app().doc.get(star).unwrap().size.y, 40.0);
    assert_fills_its_box(&s, star, "after the H field");
    assert_eq!(s.app().doc.get(star).unwrap().stroke.weight, STROKE);

    s.cmd_key(Key::Z);
    s.cmd_key(Key::Z);
    assert_eq!(s.app().hist.undo_depth(), depth);
    assert_eq!(*s.app().doc.get(star).unwrap(), before, "two undos restore the original geometry");
}

#[test]
fn the_scale_tool_scales_the_geometry_once_and_the_stroke_with_it() {
    let mut s = Sim::new();
    let star = flattened_star(&mut s);
    let w0 = s.app().doc.get(star).unwrap().size.x;
    s.shell.app.tool = Tool::Scale;
    s.frame(vec![]);

    drag_bottom_right(&mut s);

    let n = s.app().doc.get(star).unwrap();
    let factor = n.size.x / w0;
    assert!(factor > 1.2, "the Scale tool grew the star: {factor}");
    assert_fills_its_box(&s, star, "after the Scale tool");
    assert!((n.stroke.weight - STROKE * factor).abs() < 0.05, "scale mode scales the stroke: {} for factor {factor}", n.stroke.weight);
}

#[test]
fn dragging_a_handle_scales_the_operands_of_a_union() {
    let mut s = Sim::new();
    let group = union_of_two_rects(&mut s);
    assert_fills_its_box(&s, group, "before the drag");
    let operands: Vec<NodeId> = s.app().doc.get(group).unwrap().children.clone();
    let (before, depth) = (operands.iter().map(|o| s.app().doc.get(*o).unwrap().clone()).collect::<Vec<_>>(), s.app().hist.undo_depth());

    drag_bottom_right(&mut s);

    let g = s.app().doc.get(group).unwrap();
    assert!(g.size.x > 180.0 + 20.0, "the handle grew the union: {:?}", g.size);
    assert_fills_its_box(&s, group, "after the drag");
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "one gesture is one undo step");
    for (o, was) in operands.iter().zip(&before) {
        assert_ne!(s.app().doc.get(*o).unwrap().size, was.size, "operand {} followed the union", was.name);
    }

    s.cmd_key(Key::Z);
    for (o, was) in operands.iter().zip(&before) {
        assert_eq!(s.app().doc.get(*o).unwrap(), was, "undo restores operand {}", was.name);
    }
}

#[test]
fn cmd_resizing_a_union_scales_its_operands() {
    let mut s = Sim::new();
    let group = union_of_two_rects(&mut s);

    s.mods = Modifiers::COMMAND;
    drag_bottom_right(&mut s);
    s.mods = Modifiers::NONE;

    assert!(s.app().doc.get(group).unwrap().size.x > 180.0 + 20.0, "the handle grew the union");
    assert_fills_its_box(&s, group, "after the Cmd drag");
}

#[test]
fn typing_a_width_scales_the_operands_of_a_union() {
    let mut s = Sim::new();
    let group = union_of_two_rects(&mut s);

    type_into(&mut s, "w", "360");
    type_into(&mut s, "h", "70");

    assert_eq!(s.app().doc.get(group).unwrap().size, DV::new(360.0, 70.0));
    assert_fills_its_box(&s, group, "after the W and H fields");
}

/// A 300 wide row: a fixed 40 wide post, then `shape` filling whatever the padding and the gap leave.
fn row_with_a_fill_child(s: &mut Sim, mut shape: Node) -> (NodeId, NodeId) {
    let page = s.app().page;
    let fid = s.shell.app.doc.alloc_id();
    let mut row = Node::frame(fid, DRect::new(300.0, 300.0, 300.0, 120.0));
    if let Some(fp) = row.kind.frame_props_mut() {
        fp.auto_layout = Some(AutoLayout { item_spacing: 10.0, padding: [10.0; 4], ..Default::default() });
    }
    let row = s.shell.app.doc.insert(page, row).unwrap();
    let post = s.shell.app.doc.alloc_id();
    s.shell.app.doc.insert(row, Node::rectangle(post, DRect::new(0.0, 0.0, 40.0, 100.0))).unwrap();
    shape.size = DV::new(100.0, 100.0);
    shape.layout_child.horizontal = Sizing::Fill;
    let shape = s.shell.app.doc.insert(row, shape).unwrap();
    s.shell.app.changed();
    (row, shape)
}

fn arrow_vector(s: &mut Sim) -> Node {
    let corners = [(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (50.0, 60.0), (0.0, 100.0)];
    let mut cmds = vec![PathCmd::MoveTo(DV::new(corners[0].0, corners[0].1))];
    cmds.extend(corners[1..].iter().map(|(x, y)| PathCmd::LineTo(DV::new(*x, *y))));
    cmds.push(PathCmd::Close);
    let paths = vec![Path { cmds, fill_rule: designos_doc::FillRule::NonZero }];
    Node::new(s.shell.app.doc.alloc_id(), NodeKind::Vector { paths, network: None })
}

/// The row is resized to 500 wide, so the Fill slot grows from 100 to 430.
fn widen_the_row_and_expect_the_slot_to_grow(s: &mut Sim, row: NodeId, shape: NodeId) {
    assert_fills_its_box(s, shape, "before the row is resized");
    let w0 = s.app().doc.get(shape).unwrap().size.x;
    s.shell.app.selection = vec![row];
    s.frame(vec![]);

    type_into(s, "w", "500");

    let n = s.app().doc.get(shape).unwrap();
    assert!((n.size.x - 430.0).abs() < 1.0 && n.size.x > w0, "the slot grew with the row: {:?}", n.size);
    assert_fills_its_box(s, shape, "after the row is resized");
}

#[test]
fn a_fill_vector_in_an_auto_layout_scales_with_its_slot() {
    let mut s = Sim::new();
    let arrow = arrow_vector(&mut s);
    let (row, vector) = row_with_a_fill_child(&mut s, arrow);
    widen_the_row_and_expect_the_slot_to_grow(&mut s, row, vector);
}

#[test]
fn a_fill_union_in_an_auto_layout_scales_its_operands_with_its_slot() {
    let mut s = Sim::new();
    let union = Node::new(s.shell.app.doc.alloc_id(), NodeKind::BooleanOp(BooleanOp::Union));
    let (row, union) = row_with_a_fill_child(&mut s, union);
    for r in [DRect::new(0.0, 0.0, 60.0, 60.0), DRect::new(40.0, 40.0, 60.0, 60.0)] {
        let id = s.shell.app.doc.alloc_id();
        s.shell.app.doc.insert(union, Node::rectangle(id, r)).unwrap();
    }
    widen_the_row_and_expect_the_slot_to_grow(&mut s, row, union);
}

#[test]
fn a_resized_vector_and_union_keep_their_geometry_through_a_fig_file() {
    let mut s = Sim::new();
    let star = flattened_star(&mut s);
    type_into(&mut s, "w", "220");
    type_into(&mut s, "h", "60");
    let union = union_of_two_rects(&mut s);
    type_into(&mut s, "w", "300");
    type_into(&mut s, "h", "50");
    // the starter document has layers of its own, so the two shapes get names nothing else carries
    s.shell.app.doc.get_mut(star).unwrap().name = "Resized star".into();
    s.shell.app.doc.get_mut(union).unwrap().name = "Resized union".into();

    let bytes = designos_fig::export_fig(&s.app().doc).expect("export .fig");
    let back = designos_fig::import_fig(&bytes).expect("import .fig");
    let find = |name: &str| back.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("{name} is in the file")).id;
    let (star_back, union_back) = (find("Resized star"), find("Resized union"));

    for (id, back_id, what) in [(star, star_back, "the star"), (union, union_back, "the union")] {
        let (was, now) = (s.app().doc.get(id).unwrap().size, back.get(back_id).unwrap().size);
        assert!((was.x - now.x).abs() < SLACK && (was.y - now.y).abs() < SLACK, "{what} keeps its size: {was:?} -> {now:?}");
        let (lo, hi) = ink(&back, back_id);
        assert!(
            lo.x.abs() < SLACK && lo.y.abs() < SLACK && (hi.x - now.x).abs() < SLACK && (hi.y - now.y).abs() < SLACK,
            "{what} still fills its box after the round trip: {lo:?} to {hi:?} in {now:?}"
        );
    }
}

/// A shape the way a `.fig` import leaves it: the primitive, plus Figma's outline of it and of its stroke at
/// the size it had. The renderers prefer those outlines to the primitive, so they must not outlive a resize.
fn imported(s: &mut Sim, kind: NodeKind, name: &str) -> NodeId {
    let id = s.shell.app.doc.alloc_id();
    let mut n = Node::new(id, kind);
    n.transform = designos_doc::Affine::translate(300.0, 300.0);
    n.size = DV::new(100.0, 100.0);
    let id = add(s, n, name);
    let outline = node_paths(&s.app().doc, s.app().doc.get(id).unwrap()).expect("a shape has an outline");
    let n = s.shell.app.doc.get_mut(id).unwrap();
    (n.fill_geometry, n.stroke_geometry) = (Some(outline.clone()), Some(outline));
    s.shell.app.selection = vec![id];
    s.shell.app.changed();
    s.frame(vec![]);
    id
}

fn imported_kinds() -> [(NodeKind, &'static str); 4] {
    [
        (NodeKind::Ellipse(ArcData::default()), "Imported oval"),
        (NodeKind::Star { count: 5, inner_ratio: 0.4 }, "Imported star"),
        (NodeKind::Polygon { count: 6 }, "Imported hexagon"),
        (NodeKind::Rectangle, "Imported box"),
    ]
}

/// The shape is drawn the way the same primitive is drawn at its size when nothing is imported.
#[track_caller]
fn assert_drawn_at_its_size(doc: &designos_doc::Document, id: NodeId, what: &str) {
    let mut native = doc.get(id).unwrap().clone();
    (native.fill_geometry, native.stroke_geometry) = (None, None);
    let want = ink_of(&node_paths(doc, &native).unwrap());
    let got = ink(doc, id);
    assert!(
        (got.0.x - want.0.x).abs() < SLACK && (got.0.y - want.0.y).abs() < SLACK && (got.1.x - want.1.x).abs() < SLACK && (got.1.y - want.1.y).abs() < SLACK,
        "{what}: drawn {got:?} instead of {want:?} for the box {:?}",
        native.size
    );
}

#[test]
fn dragging_a_handle_draws_an_imported_shape_at_its_new_size() {
    for (kind, name) in imported_kinds() {
        let mut s = Sim::new();
        let id = imported(&mut s, kind, name);
        assert_drawn_at_its_size(&s.app().doc, id, &format!("{name} before the drag"));
        let (before, depth) = (s.app().doc.get(id).unwrap().clone(), s.app().hist.undo_depth());

        drag_bottom_right(&mut s);

        let n = s.app().doc.get(id).unwrap();
        assert!(n.size.x > before.size.x + 20.0 && n.size.y > before.size.y + 10.0, "{name}: the handle grew the box: {:?}", n.size);
        assert_drawn_at_its_size(&s.app().doc, id, &format!("{name} after the drag"));
        assert!(n.stroke_geometry.is_none(), "{name}: an outlined stroke of the old size is not kept");
        assert_eq!(n.stroke.weight, STROKE, "{name}: a plain resize leaves the stroke weight alone");
        assert_eq!(s.app().hist.undo_depth(), depth + 1, "{name}: one gesture is one undo step");

        s.cmd_key(Key::Z);
        assert_eq!(*s.app().doc.get(id).unwrap(), before, "{name}: undo brings back the box and both outlines");
    }
}

#[test]
fn typing_a_width_and_a_height_draws_an_imported_shape_at_its_new_size() {
    for (kind, name) in imported_kinds() {
        let mut s = Sim::new();
        let id = imported(&mut s, kind, name);
        let (before, depth) = (s.app().doc.get(id).unwrap().clone(), s.app().hist.undo_depth());

        type_into(&mut s, "w", "250");
        type_into(&mut s, "h", "40");

        assert_eq!(s.app().doc.get(id).unwrap().size, DV::new(250.0, 40.0), "{name}");
        assert_drawn_at_its_size(&s.app().doc, id, &format!("{name} after the W and H fields"));
        assert!(s.app().doc.get(id).unwrap().stroke_geometry.is_none(), "{name}: an outlined stroke of the old size is not kept");

        s.cmd_key(Key::Z);
        s.cmd_key(Key::Z);
        assert_eq!(s.app().hist.undo_depth(), depth, "{name}");
        assert_eq!(*s.app().doc.get(id).unwrap(), before, "{name}: two undos restore the box and both outlines");
    }
}

#[test]
fn typing_a_width_drops_the_outlined_stroke_of_an_imported_line() {
    let mut s = Sim::new();
    let id = s.shell.app.doc.alloc_id();
    let mut line = Node::new(id, NodeKind::Line);
    line.size = DV::new(100.0, 0.0);
    let id = add(&mut s, line, "Rule");
    let outline = vec![Path {
        cmds: vec![
            PathCmd::MoveTo(DV::new(0.0, -2.0)),
            PathCmd::LineTo(DV::new(100.0, -2.0)),
            PathCmd::LineTo(DV::new(100.0, 2.0)),
            PathCmd::LineTo(DV::new(0.0, 2.0)),
            PathCmd::Close,
        ],
        fill_rule: designos_doc::FillRule::NonZero,
    }];
    s.shell.app.doc.get_mut(id).unwrap().stroke_geometry = Some(outline);
    s.shell.app.selection = vec![id];
    s.shell.app.changed();
    s.frame(vec![]);

    type_into(&mut s, "w", "250");

    let n = s.app().doc.get(id).unwrap();
    assert_eq!(n.size.x, 250.0);
    assert!(n.stroke_geometry.is_none(), "a stroke outline of the old length is not kept");
}

#[test]
fn a_resized_imported_shape_keeps_its_drawing_through_a_fig_file() {
    let mut s = Sim::new();
    let mut ids = Vec::new();
    for (kind, name) in imported_kinds() {
        let id = imported(&mut s, kind, name);
        type_into(&mut s, "w", "220");
        type_into(&mut s, "h", "60");
        ids.push((id, name));
    }
    let bytes = designos_fig::export_fig(&s.app().doc).expect("export .fig");
    let back = designos_fig::import_fig(&bytes).expect("import .fig");
    for (id, name) in ids {
        let back_id = back.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("{name} is in the file")).id;
        let (want, got) = (s.app().doc.get(id).unwrap(), back.get(back_id).unwrap());
        assert_eq!(got.size, want.size, "{name} keeps its size");
        assert_drawn_at_its_size(&s.app().doc, id, &format!("{name} before the export"));
        assert_drawn_at_its_size(&back, back_id, &format!("{name} after the round trip"));
        let (a, b) = (ink(&s.app().doc, id), ink(&back, back_id));
        assert!(
            (a.0.x - b.0.x).abs() < SLACK && (a.0.y - b.0.y).abs() < SLACK && (a.1.x - b.1.x).abs() < SLACK && (a.1.y - b.1.y).abs() < SLACK,
            "{name} is drawn the same after the round trip: {a:?} vs {b:?}"
        );
    }
}
