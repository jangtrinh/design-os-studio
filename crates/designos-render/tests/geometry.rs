mod common;
use common::*;
use designos_doc::*;

fn rect_path(x: f32, y: f32, w: f32, h: f32) -> Path {
    let v = Vec2::new;
    Path {
        cmds: vec![PathCmd::MoveTo(v(x, y)), PathCmd::LineTo(v(x + w, y)), PathCmd::LineTo(v(x + w, y + h)), PathCmd::LineTo(v(x, y + h)), PathCmd::Close],
        fill_rule: FillRule::NonZero,
    }
}

#[test]
fn fill_geometry_replaces_derived_shape() {
    let mut t = T::new();
    let r = t.rect(None, Rect::new(10.0, 10.0, 40.0, 40.0), RED);
    // only the left half is covered by the precomputed outline
    t.node(r).fill_geometry = Some(vec![rect_path(0.0, 0.0, 20.0, 40.0)]);
    let pm = t.render(60, 60);
    assert_eq!(px(&pm, 15, 30), [255, 0, 0, 255], "inside geometry");
    assert_eq!(px(&pm, 40, 30), WHITE, "inside the node box but outside the geometry");
}

#[test]
fn boolean_op_uses_precomputed_geometry() {
    let mut t = T::new();
    let b = t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::BooleanOp(BooleanOp::Subtract));
        n.transform = Affine::translate(10.0, 10.0);
        n.size = Vec2::new(40.0, 40.0);
        n.fills = vec![Paint::solid(RED)];
        n
    });
    // children whose boolean result would be empty-ish: same ellipse subtracted from itself
    t.add(Some(b), |id| Node::ellipse(id, Rect::new(0.0, 0.0, 40.0, 40.0)));
    t.add(Some(b), |id| Node::ellipse(id, Rect::new(0.0, 0.0, 40.0, 40.0)));
    let pm = t.render(60, 60);
    assert_eq!(px(&pm, 30, 30), WHITE, "derived subtract of identical shapes is empty");
    // Figma already computed the result: a plain square
    t.node(b).fill_geometry = Some(vec![rect_path(5.0, 5.0, 30.0, 30.0)]);
    let pm = t.render(60, 60);
    assert_eq!(px(&pm, 30, 30), [255, 0, 0, 255]);
    assert_eq!(px(&pm, 12, 12), WHITE);
}

#[test]
fn stroke_geometry_is_filled_with_stroke_paints() {
    let mut t = T::new();
    let r = t.rect(None, Rect::new(10.0, 10.0, 40.0, 40.0), RED);
    let n = t.node(r);
    n.fills.clear();
    n.stroke.paints = vec![Paint::solid(BLUE)];
    n.stroke.weight = 4.0;
    // a thick bar through the middle, nowhere near the node outline
    n.stroke_geometry = Some(vec![rect_path(0.0, 18.0, 40.0, 6.0)]);
    let pm = t.render(60, 60);
    assert_eq!(px(&pm, 30, 30), [0, 0, 255, 255], "bar filled with stroke paint");
    assert_eq!(px(&pm, 30, 11), WHITE, "outline not stroked");
    assert_eq!(px(&pm, 30, 48), WHITE, "outline not stroked");
}

#[test]
fn stroke_geometry_honours_even_odd_fill_rule() {
    let mut t = T::new();
    let r = t.rect(None, Rect::new(0.0, 0.0, 40.0, 40.0), RED);
    let n = t.node(r);
    n.fills.clear();
    n.stroke.paints = vec![Paint::solid(BLUE)];
    // ring: outer minus inner via even-odd
    let mut ring = rect_path(0.0, 0.0, 40.0, 40.0);
    ring.cmds.extend(rect_path(10.0, 10.0, 20.0, 20.0).cmds);
    ring.fill_rule = FillRule::EvenOdd;
    n.stroke_geometry = Some(vec![ring]);
    let pm = t.render(40, 40);
    assert_eq!(px(&pm, 5, 20), [0, 0, 255, 255]);
    assert_eq!(px(&pm, 20, 20), WHITE, "hole");
}

#[test]
fn outside_stroke_with_imported_outer_disc_keeps_fill_in_centre() {
    let mut t = T::new();
    // 8 px ellipse at (6, 6)..(14, 14), centre at (10, 10)
    let e = t.add(None, |id| Node::ellipse(id, Rect::new(6.0, 6.0, 8.0, 8.0)));
    let n = t.node(e);
    n.fills = vec![Paint::solid(RED)];
    n.stroke.paints = vec![Paint::solid(Color::WHITE)];
    n.stroke.weight = 1.5;
    n.stroke.align = StrokeAlign::Outside;
    // Imported stroke_geometry covering the full outer disc (r = 5.5, local -1.5..9.5)
    n.stroke_geometry = Some(vec![rect_path(-1.5, -1.5, 11.0, 11.0)]);
    let pm = t.render(20, 20);
    // Centre at (10, 10) must keep its red fill
    assert_eq!(px(&pm, 10, 10), [255, 0, 0, 255], "interior fill must not be covered by outside stroke_geometry");
    // Outside ring at (5, 10) (1 px left of the ellipse's x=6 edge) must be painted with the white stroke
    assert_eq!(px(&pm, 5, 10), [255, 255, 255, 255], "outside ring must be stroked");
}
