//! An edit that only repaints a layer, or changes a guide the solver never reads, re-solves nothing.
//! A stroke width re-solves only the frame whose solve reads it. The fixtures are not a fixed point
//! of the solver, so a solve that runs shows up as a layer that moved.

use super::{add_frame, add_rect, auto_layout, fully_solved, geometry, moved};
use crate::engine::Engine;
use designos_doc::{Color, LayoutAlign, LayoutDirection, LayoutGrid, Node, NodeId, Paint, Rect, Sizing, StrokeAlign, TextStyle};
use serde_json::json;

/// screen (column, fixed, gap 0) > [title text, subtitle text stored at y = 100].
struct TextColumn {
    e: Engine,
    title: NodeId,
    subtitle: NodeId,
}

fn text_column() -> TextColumn {
    let mut e = Engine::new();
    let page = e.resolve_default_page().unwrap();
    let screen = add_frame(
        &mut e,
        page,
        Rect::new(0.0, 0.0, 300.0, 300.0),
        auto_layout(LayoutDirection::Vertical, Sizing::Fixed, Sizing::Fixed, LayoutAlign::Start, 0.0),
        "screen",
    );
    let add_text = |e: &mut Engine, y: f32, content: &str| {
        let id = e.doc.alloc_id();
        let mut node = Node::text(id, Rect::new(0.0, y, 100.0, 20.0), content, TextStyle::default());
        node.name = content.into();
        e.doc.insert(screen, node).unwrap()
    };
    let title = add_text(&mut e, 0.0, "Title");
    let subtitle = add_text(&mut e, 100.0, "Subtitle");
    assert_ne!(fully_solved(&e.doc), geometry(&e.doc), "the fixture must not be a fixed point of the solver");
    TextColumn { e, title, subtitle }
}

fn ty(e: &Engine, id: NodeId) -> f32 {
    e.doc.get(id).unwrap().transform.ty
}

#[test]
fn a_fill_on_a_text_layer_moves_no_layer() {
    let TextColumn { mut e, title, subtitle } = text_column();
    let before = geometry(&e.doc);

    e.call("designos_update_node", json!({ "node_id": title.0, "fill": "#00A06026" })).unwrap();

    assert_eq!(moved(&before, &geometry(&e.doc)), Vec::<u64>::new());
    assert_eq!(ty(&e, subtitle), 100.0);
}

#[test]
fn a_batch_fill_on_a_text_layer_moves_no_layer() {
    let TextColumn { mut e, title, .. } = text_column();
    let before = geometry(&e.doc);

    e.call("designos_apply_batch", json!({ "ops": [{ "op": "update", "node": title.0, "fill": "#00A06026" }] })).unwrap();

    assert_eq!(moved(&before, &geometry(&e.doc)), Vec::<u64>::new());
}

#[test]
fn a_layout_grid_style_on_a_frame_moves_no_layer() {
    let mut e = Engine::new();
    let page = e.resolve_default_page().unwrap();
    let screen = add_frame(
        &mut e,
        page,
        Rect::new(0.0, 0.0, 300.0, 300.0),
        auto_layout(LayoutDirection::Vertical, Sizing::Fixed, Sizing::Fixed, LayoutAlign::Start, 0.0),
        "screen",
    );
    let card = add_frame(
        &mut e,
        screen,
        Rect::new(0.0, 0.0, 100.0, 40.0),
        auto_layout(LayoutDirection::Horizontal, Sizing::Fixed, Sizing::Fixed, LayoutAlign::Start, 0.0),
        "card",
    );
    add_rect(&mut e, card, Rect::new(10.0, 10.0, 20.0, 20.0), "inside");
    add_rect(&mut e, screen, Rect::new(0.0, 200.0, 50.0, 30.0), "tail");
    assert_ne!(fully_solved(&e.doc), geometry(&e.doc), "the fixture must not be a fixed point of the solver");
    let before = geometry(&e.doc);
    let grids = serde_json::to_value(vec![LayoutGrid::Grid { size: 8.0, color: Color::BLACK, visible: true }]).unwrap();

    e.call(
        "designos_apply_batch",
        json!({ "ops": [{ "op": "set_style",
            "upsert": [{ "key": "guides", "name": "Guides", "property": "grid", "grids": grids }],
            "apply": [{ "node": card.0, "style": "guides" }] }] }),
    )
    .unwrap();

    assert!(!e.doc.get(card).unwrap().style_refs.is_empty(), "the style must have reached the frame");
    assert_eq!(moved(&before, &geometry(&e.doc)), Vec::<u64>::new());
}

#[test]
fn a_font_size_change_on_a_text_layer_still_re_solves_its_column() {
    let TextColumn { mut e, title, subtitle } = text_column();
    let big = TextStyle { font_size: 48.0, ..TextStyle::default() };

    e.call(
        "designos_apply_batch",
        json!({ "ops": [{ "op": "set_style",
            "upsert": [{ "key": "big", "name": "Big", "property": "text", "text_style": big }],
            "apply": [{ "node": title.0, "style": "big" }] }] }),
    )
    .unwrap();

    assert_eq!(ty(&e, subtitle), e.doc.get(title).unwrap().size.y, "the subtitle follows the taller title");
    assert!(e.doc.get(title).unwrap().size.y > 20.0);
}

/// screen (column, fixed) > [panel (row, fixed 100x60, stroke 2 inside, width bound to the `w` variable) > [inner], tail].
/// `inner` is stored at (0, 0); a solve of the panel with a 10 px inset would put it at (10, 10).
struct Panel {
    e: Engine,
    inner: NodeId,
    tail: NodeId,
}

fn panel(layout_version: u8) -> Panel {
    let mut e = Engine::new();
    let page = e.resolve_default_page().unwrap();
    let screen = add_frame(
        &mut e,
        page,
        Rect::new(0.0, 0.0, 300.0, 300.0),
        auto_layout(LayoutDirection::Vertical, Sizing::Fixed, Sizing::Fixed, LayoutAlign::Start, 0.0),
        "screen",
    );
    let mut kind = auto_layout(LayoutDirection::Horizontal, Sizing::Fixed, Sizing::Fixed, LayoutAlign::Start, 0.0);
    if let Some(al) = kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()) {
        al.layout_version = layout_version;
        al.stroke_included = true;
    }
    let panel = add_frame(&mut e, screen, Rect::new(0.0, 0.0, 100.0, 60.0), kind, "panel");
    let inner = add_rect(&mut e, panel, Rect::new(0.0, 0.0, 20.0, 20.0), "inner");
    let tail = add_rect(&mut e, screen, Rect::new(0.0, 200.0, 50.0, 30.0), "tail");
    {
        let node = e.doc.get_mut(panel).unwrap();
        node.stroke.paints = vec![Paint::solid(Color::BLACK)];
        node.stroke.weight = 2.0;
        node.stroke.align = StrokeAlign::Inside;
        node.variables.bindings.insert("strokeWeight".into(), "w".into());
    }
    let stored = geometry(&e.doc);
    e.call(
        "designos_apply_batch",
        json!({ "ops": [{ "op": "set_variables",
            "upsert_collections": [{ "id": "c", "name": "C", "modes": [{ "id": "m1", "name": "One" }], "default_mode": "m1" }],
            "upsert_variables": [{ "id": "w", "name": "w", "collection": "c", "kind": "number", "values": { "m1": 2 } }] }] }),
    )
    .unwrap();
    super::restore_geometry(&mut e, &stored);
    Panel { e, inner, tail }
}

fn widen_stroke(e: &mut Engine) {
    e.call("designos_apply_batch", json!({ "ops": [{ "op": "set_variables", "upsert_variables": [{ "id": "w", "name": "w", "collection": "c", "kind": "number", "values": { "m1": 10 } }] }] }))
        .unwrap();
}

fn at(e: &Engine, id: NodeId) -> (f32, f32) {
    let t = e.doc.get(id).unwrap().transform;
    (t.tx, t.ty)
}

#[test]
fn a_stroke_width_variable_on_a_frame_whose_borders_take_space_re_solves_it() {
    let Panel { mut e, inner, .. } = panel(0);
    assert_eq!(at(&e, inner), (0.0, 0.0));

    widen_stroke(&mut e);

    assert_eq!(at(&e, inner), (10.0, 10.0), "the 10 px border pushes the content in");
}

#[test]
fn a_stroke_width_variable_on_a_frame_whose_borders_sit_inside_the_box_re_solves_nothing() {
    let Panel { mut e, inner, tail } = panel(1);
    let before = geometry(&e.doc);

    widen_stroke(&mut e);

    assert_eq!(moved(&before, &geometry(&e.doc)), Vec::<u64>::new());
    assert_eq!(at(&e, inner), (0.0, 0.0));
    assert_eq!(at(&e, tail), (0.0, 200.0));
}
