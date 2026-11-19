//! What counts as an edit of an imported text: what moves a glyph drops the stored glyphs, what only paints one or names
//! the layer does not.

use designos_doc::{Color, FigTextGeometry, Node, NodeId, NodeKind, Paint, Rect, StoredGlyph, TextProps, TextRun, TextStyle};

fn two_run_text() -> (Node, FigTextGeometry) {
    let base = TextStyle { font_family: "Base Face".into(), font_size: 20.0, ..Default::default() };
    let second = TextStyle { font_family: "Second Face".into(), font_size: 20.0, ..Default::default() };
    let mut node = Node::text(NodeId(1), Rect::new(0.0, 0.0, 40.0, 20.0), "abcd", base);
    let NodeKind::Text(props) = &mut node.kind else { unreachable!() };
    props.runs = vec![TextRun { start: 2, end: 4, style: second }];
    let geometry = FigTextGeometry {
        text: props.clone(),
        size: node.size,
        glyphs: vec![StoredGlyph { x: 0.0, y: 15.0, font_size: 20.0, blob: StoredGlyph::NO_BLOB, first_char: 0, style: 0, flags: 0 }].into(),
        ..Default::default()
    };
    (node, geometry)
}

fn edited(node: &Node, f: impl FnOnce(&mut TextProps)) -> Node {
    let mut n = node.clone();
    let NodeKind::Text(props) = &mut n.kind else { unreachable!() };
    f(props);
    n
}

#[test]
fn a_run_paint_does_not_move_a_glyph() {
    let (node, geometry) = two_run_text();
    assert!(geometry.matches(&node));
    let recoloured = edited(&node, |p| p.runs[0].style.fills = vec![Paint::solid(Color::rgb(255, 0, 0))]);
    assert!(geometry.matches(&recoloured), "recolouring one run keeps the stored glyphs");
    let base = edited(&node, |p| p.style.fills = vec![Paint::solid(Color::rgb(0, 255, 0))]);
    assert!(geometry.matches(&base), "so does recolouring the text's own style");
}

#[test]
fn what_moves_a_glyph_in_a_run_drops_the_stored_glyphs() {
    let (node, geometry) = two_run_text();
    for (what, n) in [
        ("the run's font", edited(&node, |p| p.runs[0].style.font_family = "Third Face".into())),
        ("the run's size", edited(&node, |p| p.runs[0].style.font_size = 22.0)),
        ("where the run starts", edited(&node, |p| p.runs[0].start = 1)),
        ("where the run ends", edited(&node, |p| p.runs[0].end = 3)),
        ("a run added", edited(&node, |p| p.runs.push(TextRun { start: 0, end: 1, style: p.style.clone() }))),
        ("the run removed", edited(&node, |p| p.runs.clear())),
    ] {
        assert!(!geometry.matches(&n), "{what} changes the glyphs");
    }
}

#[test]
fn renaming_the_layer_by_hand_does_not_move_a_glyph() {
    let (node, geometry) = two_run_text();
    let renamed = edited(&node, |p| p.auto_rename = !p.auto_rename);
    assert!(geometry.matches(&renamed), "whether the layer's name tracks its text never moves a glyph");
}
