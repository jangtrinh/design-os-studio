//! Edits that remove layers, switch variable modes or bring in a capture re-solve the frames that
//! depend on them.

use super::{add_frame, add_rect, auto_layout, solve_everything};
use crate::engine::Engine;
use designos_doc::{LayoutAlign, LayoutDirection, NodeId, Rect, Sizing};
use serde_json::{json, Value};

fn ty(e: &Engine, id: NodeId) -> f32 {
    e.doc.get(id).unwrap().transform.ty
}

#[test]
fn lint_fix_removing_an_empty_frame_closes_the_gap_it_left_in_its_column() {
    let mut e = Engine::new();
    let page = e.resolve_default_page().unwrap();
    let screen = add_frame(
        &mut e,
        page,
        Rect::new(0.0, 0.0, 200.0, 10.0),
        auto_layout(LayoutDirection::Vertical, Sizing::Hug, Sizing::Fixed, LayoutAlign::Start, 0.0),
        "screen",
    );
    let column = add_frame(
        &mut e,
        screen,
        Rect::new(0.0, 0.0, 200.0, 10.0),
        auto_layout(LayoutDirection::Vertical, Sizing::Hug, Sizing::Fixed, LayoutAlign::Start, 10.0),
        "column",
    );
    add_rect(&mut e, column, Rect::new(0.0, 0.0, 100.0, 20.0), "top");
    let spacer = add_frame(&mut e, column, Rect::new(0.0, 0.0, 100.0, 40.0), designos_doc::NodeKind::Frame(Default::default()), "spacer");
    e.doc.get_mut(spacer).unwrap().fills.clear();
    let bottom = add_rect(&mut e, column, Rect::new(0.0, 0.0, 100.0, 20.0), "bottom");
    add_rect(&mut e, screen, Rect::new(0.0, 0.0, 100.0, 20.0), "after");
    solve_everything(&mut e);
    assert_eq!(ty(&e, bottom), 80.0, "precondition: top, gap, the 40 px spacer, gap");

    let reply = e.call("designos_lint_fix", json!({ "rules": ["empty-container"] })).unwrap();

    assert_eq!(reply["fixed_count"], 1);
    assert!(e.doc.get(spacer).is_none(), "the empty frame is removed");
    assert_eq!(ty(&e, bottom), 30.0, "the layer below it moves up to follow the top layer");
    assert_eq!(e.doc.get(column).unwrap().size.y, 50.0, "the column shrinks with its content");
    assert_eq!(e.doc.get(screen).unwrap().size.y, 70.0, "and so does the screen that hugs it");
}

/// Two screens, each a hug column whose item spacing is bound to the `gap` variable of the `space`
/// collection (compact 8, roomy 40), holding two 20 px layers. With `second_page` the screens sit on
/// different pages. Returns the second layer of each column.
fn bound_spacing_screens(second_page: bool) -> (Engine, Vec<NodeId>) {
    let mut e = Engine::new();
    let first_page = e.resolve_default_page().unwrap();
    e.call(
        "designos_apply_batch",
        json!({ "ops": [{ "op": "set_variables",
            "upsert_collections": [{ "id": "space", "name": "Space", "modes": [{ "id": "m1", "name": "Compact" }, { "id": "m2", "name": "Roomy" }], "default_mode": "m1" }],
            "upsert_variables": [{ "id": "gap", "name": "gap", "collection": "space", "kind": "number", "values": { "m1": 8, "m2": 40 } }] }] }),
    )
    .unwrap();
    let second_page = if second_page { e.doc.add_page("second") } else { first_page };
    let mut seconds = Vec::new();
    for (page, x) in [(first_page, 0.0), (second_page, 400.0)] {
        let screen = add_frame(
            &mut e,
            page,
            Rect::new(x, 0.0, 200.0, 10.0),
            auto_layout(LayoutDirection::Vertical, Sizing::Hug, Sizing::Fixed, LayoutAlign::Start, 0.0),
            "screen",
        );
        let column = add_frame(
            &mut e,
            screen,
            Rect::new(0.0, 0.0, 200.0, 10.0),
            auto_layout(LayoutDirection::Vertical, Sizing::Hug, Sizing::Fixed, LayoutAlign::Start, 8.0),
            "column",
        );
        e.doc.get_mut(column).unwrap().variables.bindings.insert("itemSpacing".into(), "gap".into());
        add_rect(&mut e, column, Rect::new(0.0, 0.0, 100.0, 20.0), "first");
        seconds.push(add_rect(&mut e, column, Rect::new(0.0, 0.0, 100.0, 20.0), "second"));
        add_rect(&mut e, screen, Rect::new(0.0, 0.0, 100.0, 20.0), "after");
    }
    solve_everything(&mut e);
    for id in &seconds {
        assert_eq!(ty(&e, *id), 28.0, "precondition: a 20 px layer and the compact 8 px gap");
    }
    (e, seconds)
}

#[test]
fn a_shared_spacing_value_change_re_spaces_the_columns_bound_to_it_on_every_page() {
    let (mut e, seconds) = bound_spacing_screens(true);

    e.call("designos_apply_batch", json!({ "ops": [{ "op": "set_variables", "upsert_variables": [{ "id": "gap", "values": { "m1": 24 } }] }] })).unwrap();

    assert_eq!(seconds.iter().map(|id| ty(&e, *id)).collect::<Vec<_>>(), vec![44.0, 44.0]);
}

#[test]
fn switching_a_collection_default_mode_re_spaces_the_columns_bound_to_its_variables() {
    let (mut e, seconds) = bound_spacing_screens(false);

    e.call("designos_apply_batch", json!({ "ops": [{ "op": "set_variables", "upsert_collections": [{ "id": "space", "default_mode": "m2" }] }] })).unwrap();

    assert_eq!(seconds.iter().map(|id| ty(&e, *id)).collect::<Vec<_>>(), vec![60.0, 60.0], "the roomy 40 px gap now applies");
}

#[test]
fn replacing_the_modes_of_a_collection_re_spaces_the_columns_bound_to_its_variables() {
    let (mut e, seconds) = bound_spacing_screens(false);

    e.call(
        "designos_apply_batch",
        json!({ "ops": [{ "op": "set_variables",
            "upsert_collections": [{ "id": "space", "modes": [{ "id": "m2", "name": "Roomy" }, { "id": "m1", "name": "Compact" }], "default_mode": "m2" }] }] }),
    )
    .unwrap();

    assert_eq!(seconds.iter().map(|id| ty(&e, *id)).collect::<Vec<_>>(), vec![60.0, 60.0]);
}

#[test]
fn a_collection_change_that_leaves_its_modes_alone_re_spaces_nothing() {
    let (mut e, seconds) = bound_spacing_screens(false);
    for id in &seconds {
        e.doc.get_mut(*id).unwrap().transform.ty = 5.0;
    }

    e.call("designos_apply_batch", json!({ "ops": [{ "op": "set_variables", "upsert_collections": [{ "id": "space", "name": "Spacing" }] }] })).unwrap();

    assert_eq!(seconds.iter().map(|id| ty(&e, *id)).collect::<Vec<_>>(), vec![5.0, 5.0], "a rename solves no frame");
}

#[test]
fn switching_a_collection_default_mode_re_measures_hug_text_bound_to_its_variables() {
    let mut e = Engine::new();
    e.call(
        "designos_apply_batch",
        json!({ "ops": [{ "op": "set_variables",
            "upsert_collections": [{ "id": "type", "name": "Type", "modes": [{ "id": "s", "name": "Small" }, { "id": "l", "name": "Large" }], "default_mode": "s" }],
            "upsert_variables": [{ "id": "size", "name": "size", "collection": "type", "kind": "number", "values": { "s": 12, "l": 48 } }] }] }),
    )
    .unwrap();
    let created = e.call("designos_create_node", json!({ "kind": "text", "text": "Headline" })).unwrap();
    let text = NodeId(created["node"]["id"].as_u64().unwrap());
    e.doc.get_mut(text).unwrap().variables.bindings.insert("fontSize".into(), "size".into());
    let before = e.doc.get(text).unwrap().size;

    e.call("designos_apply_batch", json!({ "ops": [{ "op": "set_variables", "upsert_collections": [{ "id": "type", "default_mode": "l" }] }] })).unwrap();

    let after = e.doc.get(text).unwrap().size;
    assert!(after.x > before.x * 2.0 && after.y > before.y * 2.0, "the 48 px size is measured: {before:?} -> {after:?}");
}

fn column_capture(second_child_y: f32) -> Value {
    let child = |id: &str, y: f32| json!({ "tag": "div", "id": id, "box": { "x": 0.0, "y": y, "width": 100.0, "height": 40.0 }, "styles": {}, "children": [] });
    json!({
        "schema": "designos-webcapture",
        "version": 1,
        "url": "https://designos.dev/capture",
        "title": "Capture",
        "viewport": { "width": 320.0, "height": 240.0, "dpr": 1.0 },
        "scroll": { "width": 320.0, "height": 240.0 },
        "root": {
            "tag": "div",
            "id": "card",
            "box": { "x": 0.0, "y": 0.0, "width": 100.0, "height": 200.0 },
            "styles": { "display": "flex", "flex-direction": "column", "row-gap": "12px" },
            "children": [child("first", 0.0), child("second", second_child_y)]
        }
    })
}

#[test]
fn an_imported_flex_column_is_solved_so_its_children_sit_a_gap_apart() {
    let mut e = Engine::new();

    let reply = e.call("designos_import_web_capture", json!({ "capture": column_capture(100.0) })).unwrap();

    let card = NodeId(reply["root_node_id"].as_u64().unwrap());
    let second = *e.doc.get(card).unwrap().children.last().unwrap();
    assert_eq!(ty(&e, second), 52.0, "the second child's captured y of 100 gives way to the 40 px first child and the 12 px gap");
}
