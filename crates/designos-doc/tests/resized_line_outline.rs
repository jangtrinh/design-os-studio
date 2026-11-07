//! Resizing a Line keeps its stored stroke outline: the band stretches with the line and anything that
//! sticks out past an end (a cap) keeps its shape and moves with that end.

use designos_doc::*;

fn points(paths: &[Path]) -> Vec<(f32, f32)> {
    paths
        .iter()
        .flat_map(|p| p.cmds.iter())
        .filter_map(|c| match c {
            PathCmd::MoveTo(p) | PathCmd::LineTo(p) => Some((p.x, p.y)),
            _ => None,
        })
        .collect()
}

fn line_with_outline(w: f32, outline: Vec<PathCmd>) -> Node {
    let mut n = Node::new(NodeId(1), NodeKind::Line);
    n.size = Vec2::new(w, 0.0);
    n.stroke.weight = 10.0;
    n.stroke_geometry = Some(vec![Path { cmds: outline, fill_rule: FillRule::NonZero }]);
    n
}

#[test]
fn band_stretches_and_y_stays() {
    let band = vec![
        PathCmd::MoveTo(Vec2::new(0.0, -10.0)),
        PathCmd::LineTo(Vec2::new(100.0, -10.0)),
        PathCmd::LineTo(Vec2::new(100.0, 0.0)),
        PathCmd::LineTo(Vec2::new(0.0, 0.0)),
        PathCmd::Close,
    ];
    let mut n = line_with_outline(100.0, band);
    apply_constrained_geometry(&mut n, Affine::IDENTITY, Vec2::new(250.0, 0.0));
    assert_eq!(points(n.stroke_geometry.as_ref().unwrap()), vec![(0.0, -10.0), (250.0, -10.0), (250.0, 0.0), (0.0, 0.0)]);
}

#[test]
fn caps_past_the_ends_keep_their_shape() {
    // a band with a square cap of 5 on each end
    let outline = vec![
        PathCmd::MoveTo(Vec2::new(-5.0, -10.0)),
        PathCmd::LineTo(Vec2::new(105.0, -10.0)),
        PathCmd::LineTo(Vec2::new(105.0, 0.0)),
        PathCmd::LineTo(Vec2::new(-5.0, 0.0)),
        PathCmd::Close,
    ];
    let mut n = line_with_outline(100.0, outline);
    apply_constrained_geometry(&mut n, Affine::IDENTITY, Vec2::new(200.0, 0.0));
    let xs: Vec<f32> = points(n.stroke_geometry.as_ref().unwrap()).iter().map(|p| p.0).collect();
    assert_eq!(xs, vec![-5.0, 205.0, 205.0, -5.0], "5 past each end before, 5 past each end after");
}

#[test]
fn a_zero_length_line_keeps_its_outline() {
    let band = vec![PathCmd::MoveTo(Vec2::new(0.0, -10.0)), PathCmd::LineTo(Vec2::new(0.0, 0.0)), PathCmd::Close];
    let mut n = line_with_outline(0.0, band.clone());
    apply_constrained_geometry(&mut n, Affine::IDENTITY, Vec2::new(40.0, 0.0));
    assert!(n.stroke_geometry.is_some(), "never dropped, never NaN");
    assert!(points(n.stroke_geometry.as_ref().unwrap()).iter().all(|p| p.0.is_finite() && p.1.is_finite()));
}

#[test]
fn a_line_without_an_outline_stays_without() {
    let mut n = Node::new(NodeId(1), NodeKind::Line);
    n.size = Vec2::new(100.0, 0.0);
    apply_constrained_geometry(&mut n, Affine::IDENTITY, Vec2::new(200.0, 0.0));
    assert!(n.stroke_geometry.is_none() && n.fill_geometry.is_none());
}
