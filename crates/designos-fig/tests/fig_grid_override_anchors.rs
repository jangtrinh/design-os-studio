//! Grid cell anchors that an instance overrides on a component child.
//!
//! v106 files store `gridColumnAnchor` / `gridRowAnchor` as the GUID of a track in the parent
//! grid's `gridColumns` / `gridRows` map. A direct child gets that GUID turned into a 0-based
//! index while the parent is converted; an override entry has to end up with the same index.

use designos_doc::*;
use designos_fig::import_fig_node_changes;
use serde_json::{json, Value};

fn g(s: u64, l: u64) -> Value {
    json!({"sessionID": s, "localID": l})
}

fn node(id: (u64, u64), parent: (u64, u64), pos: &str, ty: &str, name: &str, extra: Value) -> Value {
    let mut v = json!({
        "guid": g(id.0, id.1),
        "parentIndex": {"guid": g(parent.0, parent.1), "position": pos},
        "type": ty,
        "name": name,
        "phase": "CREATED",
        "visible": true,
        "opacity": 1.0,
        "size": {"x": 120.0, "y": 60.0},
        "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
    });
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    v
}

fn track_map(ids: &[(u64, u64)]) -> Value {
    let entries: Vec<Value> = ids.iter().enumerate().map(|(i, id)| json!({"id": g(id.0, id.1), "position": ((b'!' + i as u8) as char).to_string()})).collect();
    json!({"entries": entries})
}

/// A grid component `GridComp` (columns 9:3, 9:4, 9:5; rows 9:1, 9:2) whose child `Cell` sits in the
/// first column and row, and two instances of it: `MovedInst` carries `overrides` for `Cell`,
/// `PlainInst` carries none.
fn grid_component_doc(overrides: Vec<Value>, extra_inst: Value) -> Document {
    grid_component_doc_with(overrides, extra_inst, vec![])
}

/// The same file plus a second grid `OtherGrid` that lists the column ids in another order.
fn grid_component_doc_with_other_grid(overrides: Vec<Value>) -> Document {
    let other = node(
        (1, 40),
        (0, 1),
        "d",
        "FRAME",
        "OtherGrid",
        json!({"size": {"x": 300.0, "y": 200.0}, "stackMode": "GRID", "gridColumns": track_map(&[(9, 4), (9, 3), (9, 5)])}),
    );
    grid_component_doc_with(overrides, json!({}), vec![other])
}

fn grid_component_doc_with(overrides: Vec<Value>, extra_inst: Value, extra_nodes: Vec<Value>) -> Document {
    let mut changes = json!([
        {"guid": g(0, 0), "type": "DOCUMENT", "name": "GridOverrideDoc"},
        {"guid": g(0, 1), "parentIndex": {"guid": g(0, 0), "position": "a"}, "type": "CANVAS", "name": "Page 1"},
        node(
            (1, 10),
            (0, 1),
            "a",
            "SYMBOL",
            "GridComp",
            json!({
                "size": {"x": 300.0, "y": 200.0},
                "stackMode": "GRID",
                "gridColumns": track_map(&[(9, 3), (9, 4), (9, 5)]),
                "gridRows": track_map(&[(9, 1), (9, 2)])
            })
        ),
        node(
            (1, 11),
            (1, 10),
            "a",
            "RECTANGLE",
            "Cell",
            json!({
                "overrideKey": g(9, 11),
                "size": {"x": 40.0, "y": 20.0},
                "gridColumnAnchor": g(9, 3),
                "gridRowAnchor": g(9, 1)
            })
        ),
        node(
            (1, 20),
            (0, 1),
            "b",
            "INSTANCE",
            "MovedInst",
            {
                let mut inst = json!({
                    "size": {"x": 300.0, "y": 200.0},
                    "symbolData": {"symbolID": g(1, 10), "symbolOverrides": overrides}
                });
                for (k, x) in extra_inst.as_object().unwrap() {
                    inst[k] = x.clone();
                }
                inst
            }
        ),
        node(
            (1, 30),
            (0, 1),
            "c",
            "INSTANCE",
            "PlainInst",
            json!({"size": {"x": 300.0, "y": 200.0}, "symbolData": {"symbolID": g(1, 10), "symbolOverrides": []}})
        )
    ]);
    changes.as_array_mut().unwrap().extend(extra_nodes);
    import_fig_node_changes(&changes, "grid-override-anchors").expect("import_fig_node_changes")
}

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

fn cell_of<'a>(doc: &'a Document, instance: &str) -> &'a Node {
    find(doc, instance).children.iter().filter_map(|id| doc.get(*id)).find(|n| n.name == "Cell").unwrap_or_else(|| panic!("{instance} has no Cell child"))
}

#[test]
fn master_child_anchors_resolve_to_indexes() {
    let doc = grid_component_doc(vec![], json!({}));
    let master_cell = find(&doc, "GridComp").children.iter().filter_map(|id| doc.get(*id)).find(|n| n.name == "Cell").expect("master Cell").clone();
    assert_eq!(master_cell.layout_child.grid_column, Some(0));
    assert_eq!(master_cell.layout_child.grid_row, Some(0));
    assert_eq!(cell_of(&doc, "PlainInst").layout_child.grid_column, Some(0));
    assert_eq!(cell_of(&doc, "PlainInst").layout_child.grid_row, Some(0));
}

#[test]
fn instance_override_moves_a_component_child_to_another_column_and_row() {
    let overrides = vec![json!({"guidPath": {"guids": [g(9, 11)]}, "gridColumnAnchor": g(9, 4), "gridRowAnchor": g(9, 2)})];
    let doc = grid_component_doc(overrides, json!({}));
    let moved = cell_of(&doc, "MovedInst");
    assert_eq!(moved.layout_child.grid_column, Some(1), "column override 9:4 is the second column");
    assert_eq!(moved.layout_child.grid_row, Some(1), "row override 9:2 is the second row");
    // The other instance keeps the main component's cell.
    assert_eq!(cell_of(&doc, "PlainInst").layout_child.grid_column, Some(0));
    assert_eq!(cell_of(&doc, "PlainInst").layout_child.grid_row, Some(0));
}

#[test]
fn instance_override_reads_columns_alone() {
    let overrides = vec![json!({"guidPath": {"guids": [g(9, 11)]}, "gridColumnAnchor": g(9, 5)})];
    let doc = grid_component_doc(overrides, json!({}));
    let moved = cell_of(&doc, "MovedInst");
    assert_eq!(moved.layout_child.grid_column, Some(2), "column override 9:5 is the third column");
    assert_eq!(moved.layout_child.grid_row, Some(0), "no row override keeps the main component's row");
}

#[test]
fn instance_override_reads_rows_alone() {
    let overrides = vec![json!({"guidPath": {"guids": [g(9, 11)]}, "gridRowAnchor": g(9, 2)})];
    let doc = grid_component_doc(overrides, json!({}));
    let moved = cell_of(&doc, "MovedInst");
    assert_eq!(moved.layout_child.grid_row, Some(1), "row override 9:2 is the second row");
    assert_eq!(moved.layout_child.grid_column, Some(0), "no column override keeps the main component's column");
}

#[test]
fn tracks_restated_by_the_instance_decide_the_index() {
    // The instance restates the grid with a new first column (9:6), so 9:4 is its third column
    // even though the component lists it second.
    let overrides = vec![
        json!({"guidPath": {"guids": []}, "gridColumns": track_map(&[(9, 6), (9, 3), (9, 4), (9, 5)])}),
        json!({"guidPath": {"guids": [g(9, 11)]}, "gridColumnAnchor": g(9, 4)}),
    ];
    let doc = grid_component_doc(overrides, json!({}));
    assert_eq!(cell_of(&doc, "MovedInst").layout_child.grid_column, Some(2));
}

#[test]
fn tracks_on_the_instance_node_decide_the_index_of_a_one_step_path() {
    // The instance carries its own column map that lists 9:4 first: for a child one step below the
    // instance, that map is the grid the anchor names, not the component's order (9:4 second).
    let overrides = vec![json!({"guidPath": {"guids": [g(9, 11)]}, "gridColumnAnchor": g(9, 4), "gridRowAnchor": g(9, 2)})];
    let doc = grid_component_doc(overrides, json!({"gridColumns": track_map(&[(9, 4), (9, 3), (9, 5)])}));
    let moved = cell_of(&doc, "MovedInst");
    assert_eq!(moved.layout_child.grid_column, Some(0));
    assert_eq!(moved.layout_child.grid_row, Some(1), "the instance restates no rows: the component's rows decide");
}

#[test]
fn a_track_id_two_grids_place_differently_is_not_guessed() {
    // 9:4 is the second column of the component's grid and the first of another grid in the file:
    // without the grid that owns the anchor, the component's cell is kept rather than a guessed one.
    let overrides = vec![json!({"guidPath": {"guids": [g(9, 11)]}, "gridColumnAnchor": g(9, 4)})];
    let doc = grid_component_doc_with_other_grid(overrides);
    assert_eq!(cell_of(&doc, "MovedInst").layout_child.grid_column, Some(0));
}

#[test]
fn an_instance_restating_the_tracks_does_not_disturb_the_others() {
    let restating = node(
        (1, 50),
        (0, 1),
        "e",
        "INSTANCE",
        "RestatingInst",
        json!({
            "size": {"x": 300.0, "y": 200.0},
            "gridColumns": track_map(&[(9, 5), (9, 3), (9, 4)]),
            "symbolData": {"symbolID": g(1, 10), "symbolOverrides": []}
        }),
    );
    let overrides = vec![json!({"guidPath": {"guids": [g(9, 11)]}, "gridColumnAnchor": g(9, 4)})];
    let doc = grid_component_doc_with(overrides, json!({}), vec![restating]);
    assert_eq!(cell_of(&doc, "MovedInst").layout_child.grid_column, Some(1), "the component's order decides for an instance that restates nothing");
}

#[test]
fn unknown_override_anchor_leaves_the_component_cell() {
    let overrides = vec![json!({"guidPath": {"guids": [g(9, 11)]}, "gridColumnAnchor": g(9, 99), "gridRowAnchor": g(9, 98)})];
    let doc = grid_component_doc(overrides, json!({}));
    let moved = cell_of(&doc, "MovedInst");
    assert_eq!(moved.layout_child.grid_column, Some(0));
    assert_eq!(moved.layout_child.grid_row, Some(0));
}

#[test]
fn index_valued_override_anchor_is_kept() {
    let overrides = vec![json!({"guidPath": {"guids": [g(9, 11)]}, "gridColumnAnchorIndex": 2, "gridRowAnchorIndex": 1})];
    let doc = grid_component_doc(overrides, json!({}));
    let moved = cell_of(&doc, "MovedInst");
    assert_eq!(moved.layout_child.grid_column, Some(2));
    assert_eq!(moved.layout_child.grid_row, Some(1));
}
