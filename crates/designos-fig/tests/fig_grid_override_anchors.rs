//! Grid cell anchors that an instance overrides on a component child.
//!
//! v106 files store `gridColumnAnchor` / `gridRowAnchor` as the GUID of a track in the parent
//! grid's `gridColumns` / `gridRows` map. A direct child gets that GUID turned into a 0-based
//! index while the parent is converted; an override entry has to end up with the same index.

use designos_doc::*;
use designos_fig::{decode_fig_to_json, export_fig, import_fig, import_fig_node_changes};
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

#[test]
fn instance_track_reorder_remaps_component_child_anchor_without_repeating_it() {
    // Component columns are [9:3, 9:4, 9:5] and rows are [9:1, 9:2]; pin a second child `Pinned` to (9:4, 9:2)
    // (index 1, 1 in the component). The instance reorders columns to [9:4, 9:3, 9:5] and rows to [9:2, 9:1]
    // without repeating `Pinned`'s anchors in `symbolOverrides`: `Pinned` must stay on track (9:4, 9:2) -> (0, 0).
    let pinned = node(
        (1, 12),
        (1, 10),
        "b",
        "RECTANGLE",
        "Pinned",
        json!({
            "overrideKey": g(9, 12),
            "size": {"x": 40.0, "y": 20.0},
            "gridColumnAnchor": g(9, 4),
            "gridRowAnchor": g(9, 2)
        }),
    );
    let overrides = vec![json!({
        "guidPath": {"guids": []},
        "gridColumns": track_map(&[(9, 4), (9, 3), (9, 5)]),
        "gridRows": track_map(&[(9, 2), (9, 1)])
    })];
    let mut doc = grid_component_doc_with(overrides, json!({}), vec![pinned]);

    let master_pinned = find(&doc, "GridComp").children.iter().filter_map(|id| doc.get(*id)).find(|n| n.name == "Pinned").unwrap();
    assert_eq!((master_pinned.layout_child.grid_column, master_pinned.layout_child.grid_row), (Some(1), Some(1)));

    let moved_inst_id = find(&doc, "MovedInst").id;
    let plain_inst_id = find(&doc, "PlainInst").id;
    doc.relayout_subtree(moved_inst_id);
    doc.relayout_subtree(plain_inst_id);

    let moved_pinned = find(&doc, "MovedInst").children.iter().filter_map(|id| doc.get(*id)).find(|n| n.name == "Pinned").unwrap();
    assert_eq!(moved_pinned.layout_child.grid_column, Some(0), "9:4 moved to column 0 on MovedInst");
    assert_eq!(moved_pinned.layout_child.grid_row, Some(0), "9:2 moved to row 0 on MovedInst");
    assert_eq!((moved_pinned.transform.tx, moved_pinned.transform.ty), (0.0, 0.0));

    let moved_cell = cell_of(&doc, "MovedInst");
    assert_eq!(moved_cell.layout_child.grid_column, Some(1), "9:3 moved to column 1 on MovedInst");
    assert_eq!(moved_cell.layout_child.grid_row, Some(1), "9:1 moved to row 1 on MovedInst");
    assert_eq!((moved_cell.transform.tx, moved_cell.transform.ty), (40.0, 20.0));

    let plain_pinned = find(&doc, "PlainInst").children.iter().filter_map(|id| doc.get(*id)).find(|n| n.name == "Pinned").unwrap();
    assert_eq!((plain_pinned.layout_child.grid_column, plain_pinned.layout_child.grid_row), (Some(1), Some(1)));
    assert_eq!((plain_pinned.transform.tx, plain_pinned.transform.ty), (40.0, 20.0));
}

#[test]
fn restated_track_list_without_anchor_does_not_fall_through_to_file_wide_index() {
    // Component columns are [9:3, 9:4, 9:5] (9:4 is index 1, unique in the file); `Cell` is on 9:3 (index 0).
    // The instance restates columns to [9:6, 9:7, 9:5] and a child override still names 9:4: because the
    // restated column list is non-empty and lacks 9:4, `Cell` keeps its component cell (0) instead of landing on 9:7 (1).
    let overrides = vec![
        json!({"guidPath": {"guids": []}, "gridColumns": track_map(&[(9, 6), (9, 7), (9, 5)])}),
        json!({"guidPath": {"guids": [g(9, 11)]}, "gridColumnAnchor": g(9, 4)}),
    ];
    let doc = grid_component_doc(overrides, json!({}));
    assert_eq!(cell_of(&doc, "MovedInst").layout_child.grid_column, Some(0));
}

#[test]
fn restated_track_maps_at_same_path_merge_per_axis() {
    // The instance node sets its own column order [9:4, 9:3, 9:5], while an empty-guidPath override sets only
    // `gridRows` [9:2, 9:1]: the row override must not wipe out the instance's restated column order.
    let overrides = vec![
        json!({"guidPath": {"guids": []}, "gridRows": track_map(&[(9, 2), (9, 1)])}),
        json!({"guidPath": {"guids": [g(9, 11)]}, "gridColumnAnchor": g(9, 4), "gridRowAnchor": g(9, 2)}),
    ];
    let doc = grid_component_doc(overrides, json!({"gridColumns": track_map(&[(9, 4), (9, 3), (9, 5)])}));
    let moved = cell_of(&doc, "MovedInst");
    assert_eq!(moved.layout_child.grid_column, Some(0));
    assert_eq!(moved.layout_child.grid_row, Some(0));
}

#[test]
fn real_v106_binary_fig_with_instance_grid_overrides_remaps_and_relayouts() {
    use designos_fig::container::{write_container, Compression};
    use designos_fig::import_fig;
    use designos_fig::kiwi::{DefKind, FieldType, KiwiValue as V, Native, SchemaBuilder};

    let mut b = SchemaBuilder::new();
    let enum_def = |b: &mut SchemaBuilder, name: &str, variants: &[&str]| -> usize {
        let d = b.define(name, DefKind::Enum);
        for (i, v) in variants.iter().enumerate() {
            b.variant(d, v, i as u32);
        }
        d
    };
    enum_def(&mut b, "Phase", &["CREATED", "REMOVED"]);
    let nt_def = enum_def(&mut b, "NodeType", &["NONE", "DOCUMENT", "CANVAS", "SYMBOL", "INSTANCE", "ROUNDED_RECTANGLE"]);
    let sm_def = enum_def(&mut b, "StackMode", &["NONE", "HORIZONTAL", "VERTICAL", "GRID"]);
    let gs_type = enum_def(&mut b, "GridSizingType", &["FIXED", "FLEX", "HUG"]);

    let guid_def = b.define("GUID", DefKind::Struct);
    b.field(guid_def, "sessionID", FieldType::Native(Native::Uint), false, 1);
    b.field(guid_def, "localID", FieldType::Native(Native::Uint), false, 2);
    let guid_ty = FieldType::Def(guid_def);

    let vec_def = b.define("Vector", DefKind::Struct);
    b.field(vec_def, "x", FieldType::Native(Native::Float), false, 1);
    b.field(vec_def, "y", FieldType::Native(Native::Float), false, 2);
    let vec_ty = FieldType::Def(vec_def);

    let mat_def = b.define("Matrix", DefKind::Struct);
    for (i, n) in ["m00", "m01", "m02", "m10", "m11", "m12"].into_iter().enumerate() {
        b.field(mat_def, n, FieldType::Native(Native::Float), false, (i + 1) as u32);
    }
    let mat_ty = FieldType::Def(mat_def);

    let pi_def = b.define("ParentIndex", DefKind::Struct);
    b.field(pi_def, "guid", guid_ty, false, 1);
    b.field(pi_def, "position", FieldType::Native(Native::String), false, 2);
    let pi_ty = FieldType::Def(pi_def);

    let gp_entry = b.define("GUIDPositionEntry", DefKind::Message);
    b.field(gp_entry, "id", guid_ty, false, 1);
    b.field(gp_entry, "position", FieldType::Native(Native::String), false, 2);
    let gp_map = b.define("GUIDPositionMap", DefKind::Message);
    b.field(gp_map, "entries", FieldType::Def(gp_entry), true, 1);
    let gp_map_ty = FieldType::Def(gp_map);

    let gs_def = b.define("GridSizing", DefKind::Message);
    b.field(gs_def, "type", FieldType::Def(gs_type), false, 1);
    b.field(gs_def, "value", FieldType::Native(Native::Float), false, 2);
    let gts_def = b.define("GridTrackSize", DefKind::Message);
    b.field(gts_def, "minSizing", FieldType::Def(gs_def), false, 1);
    b.field(gts_def, "maxSizing", FieldType::Def(gs_def), false, 2);
    let gts_entry = b.define("GUIDGridTrackSizeEntry", DefKind::Message);
    b.field(gts_entry, "id", guid_ty, false, 1);
    b.field(gts_entry, "trackSize", FieldType::Def(gts_def), false, 2);
    let gts_map = b.define("GUIDGridTrackSizeMap", DefKind::Message);
    b.field(gts_map, "entries", FieldType::Def(gts_entry), true, 1);
    let gts_map_ty = FieldType::Def(gts_map);

    let gp_path = b.define("GUIDPath", DefKind::Message);
    b.field(gp_path, "guids", guid_ty, true, 1);

    let so_def = b.define("SymbolOverride", DefKind::Message);
    b.field(so_def, "guidPath", FieldType::Def(gp_path), false, 1);
    b.field(so_def, "gridColumns", gp_map_ty, false, 2);
    b.field(so_def, "gridRows", gp_map_ty, false, 3);
    b.field(so_def, "gridColumnsSizing", gts_map_ty, false, 4);
    b.field(so_def, "gridRowsSizing", gts_map_ty, false, 5);
    b.field(so_def, "gridColumnAnchor", guid_ty, false, 6);
    b.field(so_def, "gridRowAnchor", guid_ty, false, 7);

    let sd_def = b.define("SymbolData", DefKind::Message);
    b.field(sd_def, "symbolID", guid_ty, false, 1);
    b.field(sd_def, "symbolOverrides", FieldType::Def(so_def), true, 2);

    let nc_def = b.define("NodeChange", DefKind::Message);
    b.field(nc_def, "guid", guid_ty, false, 1);
    b.field(nc_def, "parentIndex", pi_ty, false, 2);
    b.field(nc_def, "type", FieldType::Def(nt_def), false, 3);
    b.field(nc_def, "name", FieldType::Native(Native::String), false, 4);
    b.field(nc_def, "size", vec_ty, false, 5);
    b.field(nc_def, "transform", mat_ty, false, 6);
    b.field(nc_def, "overrideKey", guid_ty, false, 7);
    b.field(nc_def, "stackMode", FieldType::Def(sm_def), false, 8);
    b.field(nc_def, "gridColumns", gp_map_ty, false, 9);
    b.field(nc_def, "gridRows", gp_map_ty, false, 10);
    b.field(nc_def, "gridColumnsSizing", gts_map_ty, false, 11);
    b.field(nc_def, "gridRowsSizing", gts_map_ty, false, 12);
    b.field(nc_def, "gridColumnAnchor", guid_ty, false, 13);
    b.field(nc_def, "gridRowAnchor", guid_ty, false, 14);
    b.field(nc_def, "symbolData", FieldType::Def(sd_def), false, 15);

    let msg_def = b.define("Message", DefKind::Message);
    b.field(msg_def, "nodeChanges", FieldType::Def(nc_def), true, 1);
    let schema = b.build();

    let kg = |s: u32, l: u32| V::Struct(vec![("sessionID".into(), V::Uint(s)), ("localID".into(), V::Uint(l))]);
    let kpi = |s: u32, l: u32, pos: &str| V::Struct(vec![("guid".into(), kg(s, l)), ("position".into(), V::String(pos.into()))]);
    let kvec = |x: f32, y: f32| V::Struct(vec![("x".into(), V::Float(x)), ("y".into(), V::Float(y))]);
    let kmat = || {
        V::Struct(vec![
            ("m00".into(), V::Float(1.0)),
            ("m01".into(), V::Float(0.0)),
            ("m02".into(), V::Float(0.0)),
            ("m10".into(), V::Float(0.0)),
            ("m11".into(), V::Float(1.0)),
            ("m12".into(), V::Float(0.0)),
        ])
    };
    let ktrack_map = |ids: &[(u32, u32)]| {
        let entries = ids
            .iter()
            .enumerate()
            .map(|(i, &(s, l))| V::Message(vec![("id".into(), kg(s, l)), ("position".into(), V::String(((b'!' + i as u8) as char).to_string()))]))
            .collect();
        V::Message(vec![("entries".into(), V::Array(entries))])
    };
    let ksizing_map = |items: &[((u32, u32), &str, f32)]| {
        let entries = items
            .iter()
            .map(|&((s, l), ty, val)| {
                let sz = V::Message(vec![("type".into(), V::Enum(ty.into())), ("value".into(), V::Float(val))]);
                let ts = V::Message(vec![("minSizing".into(), sz.clone()), ("maxSizing".into(), sz)]);
                V::Message(vec![("id".into(), kg(s, l)), ("trackSize".into(), ts)])
            })
            .collect();
        V::Message(vec![("entries".into(), V::Array(entries))])
    };

    let root = V::Message(vec![(
        "nodeChanges".into(),
        V::Array(vec![
            V::Message(vec![("guid".into(), kg(0, 0)), ("type".into(), V::Enum("DOCUMENT".into())), ("name".into(), V::String("Doc".into()))]),
            V::Message(vec![
                ("guid".into(), kg(0, 1)),
                ("parentIndex".into(), kpi(0, 0, "!")),
                ("type".into(), V::Enum("CANVAS".into())),
                ("name".into(), V::String("Page 1".into())),
            ]),
            V::Message(vec![
                ("guid".into(), kg(1, 10)),
                ("parentIndex".into(), kpi(0, 1, "!")),
                ("type".into(), V::Enum("SYMBOL".into())),
                ("name".into(), V::String("GridComp".into())),
                ("size".into(), kvec(300.0, 120.0)),
                ("transform".into(), kmat()),
                ("stackMode".into(), V::Enum("GRID".into())),
                ("gridColumns".into(), ktrack_map(&[(9, 3), (9, 4), (9, 5)])),
                ("gridRows".into(), ktrack_map(&[(9, 1), (9, 2)])),
                ("gridColumnsSizing".into(), ksizing_map(&[((9, 3), "FIXED", 80.0), ((9, 4), "FIXED", 120.0), ((9, 5), "FIXED", 100.0)])),
                ("gridRowsSizing".into(), ksizing_map(&[((9, 1), "FIXED", 50.0), ((9, 2), "FIXED", 70.0)])),
            ]),
            V::Message(vec![
                ("guid".into(), kg(1, 11)),
                ("parentIndex".into(), kpi(1, 10, "!")),
                ("type".into(), V::Enum("ROUNDED_RECTANGLE".into())),
                ("name".into(), V::String("Cell".into())),
                ("overrideKey".into(), kg(9, 11)),
                ("size".into(), kvec(40.0, 20.0)),
                ("transform".into(), kmat()),
                ("gridColumnAnchor".into(), kg(9, 4)),
                ("gridRowAnchor".into(), kg(9, 2)),
            ]),
            V::Message(vec![
                ("guid".into(), kg(1, 20)),
                ("parentIndex".into(), kpi(0, 1, "\"")),
                ("type".into(), V::Enum("INSTANCE".into())),
                ("name".into(), V::String("MovedInst".into())),
                ("size".into(), kvec(300.0, 120.0)),
                ("transform".into(), kmat()),
                (
                    "symbolData".into(),
                    V::Message(vec![
                        ("symbolID".into(), kg(1, 10)),
                        (
                            "symbolOverrides".into(),
                            V::Array(vec![V::Message(vec![
                                ("guidPath".into(), V::Message(vec![("guids".into(), V::Array(vec![]))])),
                                ("gridColumns".into(), ktrack_map(&[(9, 4), (9, 3), (9, 5)])),
                                ("gridColumnsSizing".into(), ksizing_map(&[((9, 4), "FIXED", 120.0), ((9, 3), "FIXED", 80.0), ((9, 5), "FIXED", 100.0)])),
                            ])]),
                        ),
                    ]),
                ),
            ]),
        ]),
    )]);

    let body = schema.encode_message(msg_def, &root).expect("encode v106 message");
    let bytes = write_container(&schema.encode(), &body, Compression::Deflate, 106);
    let mut doc = import_fig(&bytes).expect("import v106 binary fig");
    let inst_id = find(&doc, "MovedInst").id;
    doc.relayout_subtree(inst_id);

    let cell = cell_of(&doc, "MovedInst");
    assert_eq!(cell.layout_child.grid_column, Some(0), "9:4 remapped to column 0 in binary v106 .fig");
    assert_eq!(cell.layout_child.grid_row, Some(1), "9:2 stays at row 1");
    assert_eq!((cell.transform.tx, cell.transform.ty), (0.0, 50.0));
}

#[test]
fn reorder_only_override_permutes_unequal_track_sizes() {
    let sizing_map = |items: &[((u64, u64), &str, f32)]| -> Value {
        let entries: Vec<Value> = items
            .iter()
            .map(|&((s, l), ty, val)| {
                json!({
                    "id": g(s, l),
                    "trackSize": {
                        "minSizing": {"type": ty, "value": val},
                        "maxSizing": {"type": ty, "value": val}
                    }
                })
            })
            .collect();
        json!({"entries": entries})
    };
    let changes = json!([
        {"guid": g(0, 0), "type": "DOCUMENT", "name": "UnequalTrackReorderDoc"},
        {"guid": g(0, 1), "parentIndex": {"guid": g(0, 0), "position": "a"}, "type": "CANVAS", "name": "Page 1"},
        node(
            (1, 10),
            (0, 1),
            "a",
            "SYMBOL",
            "GridComp",
            json!({
                "size": {"x": 300.0, "y": 120.0},
                "stackMode": "GRID",
                "gridColumns": track_map(&[(9, 3), (9, 4), (9, 5)]),
                "gridRows": track_map(&[(9, 1), (9, 2)]),
                "gridColumnsSizing": sizing_map(&[((9, 3), "FIXED", 80.0), ((9, 4), "FIXED", 120.0), ((9, 5), "FIXED", 100.0)]),
                "gridRowsSizing": sizing_map(&[((9, 1), "FIXED", 50.0), ((9, 2), "FIXED", 70.0)])
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
                "size": {"x": 80.0, "y": 50.0},
                "layoutSizingHorizontal": "FILL",
                "layoutSizingVertical": "FILL",
                "gridColumnAnchor": g(9, 3),
                "gridRowAnchor": g(9, 1)
            })
        ),
        node(
            (1, 12),
            (1, 10),
            "b",
            "RECTANGLE",
            "Wide",
            json!({
                "overrideKey": g(9, 12),
                "size": {"x": 120.0, "y": 70.0},
                "layoutSizingHorizontal": "FILL",
                "layoutSizingVertical": "FILL",
                "gridColumnAnchor": g(9, 4),
                "gridRowAnchor": g(9, 2)
            })
        ),
        node(
            (1, 20),
            (0, 1),
            "b",
            "INSTANCE",
            "MovedInst",
            json!({
                "size": {"x": 300.0, "y": 120.0},
                "symbolData": {
                    "symbolID": g(1, 10),
                    "symbolOverrides": [{
                        "guidPath": {"guids": []},
                        "gridColumns": track_map(&[(9, 4), (9, 3), (9, 5)]),
                        "gridRows": track_map(&[(9, 2), (9, 1)])
                    }]
                }
            })
        ),
        node(
            (1, 30),
            (0, 1),
            "c",
            "INSTANCE",
            "NodeMovedInst",
            json!({
                "size": {"x": 300.0, "y": 120.0},
                "gridColumns": track_map(&[(9, 4), (9, 3), (9, 5)]),
                "gridRows": track_map(&[(9, 2), (9, 1)]),
                "symbolData": {"symbolID": g(1, 10), "symbolOverrides": []}
            })
        ),
        // The instance node itself carries the restated track lists next to its own grid stack mode,
        // so its track ids are known before the component grid is copied onto it.
        node(
            (1, 40),
            (0, 1),
            "d",
            "INSTANCE",
            "NodeGridMovedInst",
            json!({
                "size": {"x": 300.0, "y": 120.0},
                "stackMode": "GRID",
                "gridColumns": track_map(&[(9, 4), (9, 3), (9, 5)]),
                "gridRows": track_map(&[(9, 2), (9, 1)]),
                "symbolData": {"symbolID": g(1, 10), "symbolOverrides": []}
            })
        ),
        // Columns restated on the node, rows restated later by an override with an empty path.
        node(
            (1, 50),
            (0, 1),
            "e",
            "INSTANCE",
            "MixedAxisInst",
            json!({
                "size": {"x": 300.0, "y": 120.0},
                "stackMode": "GRID",
                "gridColumns": track_map(&[(9, 4), (9, 3), (9, 5)]),
                "symbolData": {
                    "symbolID": g(1, 10),
                    "symbolOverrides": [{"guidPath": {"guids": []}, "gridRows": track_map(&[(9, 2), (9, 1)])}]
                }
            })
        )
    ]);

    let mut doc = import_fig_node_changes(&changes, "unequal-track-reorder").expect("import_fig_node_changes");
    for inst_name in ["MovedInst", "NodeMovedInst", "NodeGridMovedInst", "MixedAxisInst"] {
        let inst_id = find(&doc, inst_name).id;
        let al = find(&doc, inst_name).kind.frame_props().unwrap().auto_layout.as_ref().expect("grid auto_layout").clone();
        assert_eq!(
            &al.grid_col_tracks[..3],
            &[GridTrack::Fixed(120.0), GridTrack::Fixed(80.0), GridTrack::Fixed(100.0)],
            "{inst_name} must permute column track sizes to match [9:4, 9:3, 9:5]"
        );
        assert_eq!(&al.grid_row_tracks[..2], &[GridTrack::Fixed(70.0), GridTrack::Fixed(50.0)], "{inst_name} must permute row track sizes to match [9:2, 9:1]");

        doc.relayout_subtree(inst_id);
        let wide = find(&doc, inst_name).children.iter().filter_map(|id| doc.get(*id)).find(|n| n.name == "Wide").unwrap();
        assert_eq!((wide.layout_child.grid_column, wide.layout_child.grid_row), (Some(0), Some(0)));
        assert_eq!((wide.transform.tx, wide.transform.ty), (0.0, 0.0), "{inst_name} Wide position");
        assert_eq!((wide.size.x, wide.size.y), (120.0, 70.0), "{inst_name} Wide stays in 120x70 track");

        let cell = cell_of(&doc, inst_name);
        assert_eq!((cell.layout_child.grid_column, cell.layout_child.grid_row), (Some(1), Some(1)));
        assert_eq!((cell.transform.tx, cell.transform.ty), (120.0, 70.0), "{inst_name} Cell shifts past 120x70 track");
        assert_eq!((cell.size.x, cell.size.y), (80.0, 50.0), "{inst_name} Cell stays in 80x50 track");
    }
}

#[test]
fn stack_mode_none_override_and_instance_node_remove_inherited_grid() {
    let mut cell_free = node(
        (1, 21),
        (1, 10),
        "a",
        "RECTANGLE",
        "FreeChild",
        json!({
            "overrideKey": g(9, 21),
            "size": {"x": 40.0, "y": 20.0},
            "gridColumnAnchor": g(9, 4),
            "gridRowAnchor": g(9, 2)
        }),
    );
    cell_free["transform"] = json!({"m00": 1.0, "m01": 0.0, "m02": 37.0, "m10": 0.0, "m11": 1.0, "m12": 53.0});

    let none_with_name = node(
        (1, 40),
        (0, 1),
        "d",
        "INSTANCE",
        "NoneWithNameRaw",
        json!({
            "size": {"x": 300.0, "y": 200.0},
            "symbolData": {
                "symbolID": g(1, 10),
                "symbolOverrides": [{
                    "guidPath": {"guids": []},
                    "stackMode": "NONE",
                    "name": "NoneWithNameInst"
                }]
            }
        }),
    );
    let none_on_node = node(
        (1, 50),
        (0, 1),
        "e",
        "INSTANCE",
        "NoneOnNodeRaw",
        json!({
            "size": {"x": 300.0, "y": 200.0},
            "stackMode": "NONE",
            "symbolData": {
                "symbolID": g(1, 10),
                "symbolOverrides": [{
                    "guidPath": {"guids": []},
                    "name": "NoneOnNodeInst"
                }]
            }
        }),
    );

    let none_only_overrides = vec![json!({"guidPath": {"guids": []}, "stackMode": "NONE"})];
    let mut doc = grid_component_doc_with(none_only_overrides, json!({}), vec![cell_free, none_with_name, none_on_node]);

    for inst_name in ["MovedInst", "NoneWithNameInst", "NoneOnNodeInst"] {
        let inst = find(&doc, inst_name);
        let inst_id = inst.id;
        assert!(inst.kind.frame_props().unwrap().auto_layout.is_none(), "{inst_name} must not inherit component grid when stackMode is NONE");
        assert!(inst.component.overrides.contains(&OverrideField::Layout), "{inst_name} must record OverrideField::Layout when stackMode is NONE");
        assert!(inst.component.layout_cleared, "{inst_name} must remember the clear when stackMode is NONE");
        doc.relayout_subtree(inst_id);
        let free_child = find(&doc, inst_name).children.iter().filter_map(|id| doc.get(*id)).find(|n| n.name == "FreeChild").unwrap();
        assert_eq!(
            (free_child.transform.tx, free_child.transform.ty),
            (37.0, 53.0),
            "{inst_name} FreeChild must stay at its free position after relayout_subtree"
        );
    }

    // PlainInst (which did not set stackMode: NONE) still inherits the component grid.
    let plain = find(&doc, "PlainInst");
    assert_eq!(plain.kind.frame_props().unwrap().auto_layout.as_ref().map(|a| a.direction), Some(LayoutDirection::Grid));
}

#[test]
fn stack_mode_none_instance_stays_without_a_grid_through_a_fig_round_trip() {
    let mut cell_free = node(
        (1, 21),
        (1, 10),
        "a",
        "RECTANGLE",
        "FreeChild",
        json!({
            "overrideKey": g(9, 21),
            "size": {"x": 40.0, "y": 20.0},
            "gridColumnAnchor": g(9, 4),
            "gridRowAnchor": g(9, 2)
        }),
    );
    cell_free["transform"] = json!({"m00": 1.0, "m01": 0.0, "m02": 37.0, "m10": 0.0, "m11": 1.0, "m12": 53.0});
    let none_on_node = node(
        (1, 50),
        (0, 1),
        "e",
        "INSTANCE",
        "NoneOnNodeInst",
        json!({
            "size": {"x": 300.0, "y": 200.0},
            "stackMode": "NONE",
            "symbolData": {"symbolID": g(1, 10), "symbolOverrides": []}
        }),
    );

    let none_only_overrides = vec![json!({"guidPath": {"guids": []}, "stackMode": "NONE"})];
    let original = grid_component_doc_with(none_only_overrides, json!({}), vec![cell_free, none_on_node]);
    let bytes = export_fig(&original).expect("export_fig");
    let mut doc = import_fig(&bytes).expect("import_fig");

    for inst_name in ["MovedInst", "NoneOnNodeInst"] {
        let inst = find(&doc, inst_name);
        let inst_id = inst.id;
        assert!(inst.kind.frame_props().unwrap().auto_layout.is_none(), "{inst_name} must not re-inherit the component grid after a .fig round trip");
        doc.relayout_subtree(inst_id);
        let free_child = find(&doc, inst_name).children.iter().filter_map(|id| doc.get(*id)).find(|n| n.name == "FreeChild").unwrap();
        assert_eq!((free_child.transform.tx, free_child.transform.ty), (37.0, 53.0), "{inst_name} FreeChild keeps its free position");
        assert!(
            find(&doc, inst_name).component.overrides.contains(&OverrideField::Layout),
            "{inst_name} must keep OverrideField::Layout after a .fig round trip"
        );
        assert!(find(&doc, inst_name).component.layout_cleared, "{inst_name} must keep the clear after a .fig round trip");
    }

    let plain = find(&doc, "PlainInst");
    assert_eq!(
        plain.kind.frame_props().unwrap().auto_layout.as_ref().map(|a| a.direction),
        Some(LayoutDirection::Grid),
        "an instance that never set stackMode NONE keeps the grid"
    );
}

#[test]
fn stack_mode_none_override_on_a_nested_frame_survives_a_fig_round_trip() {
    let changes = json!([
        {"guid": g(0, 0), "type": "DOCUMENT", "name": "NestedNoneDoc"},
        {"guid": g(0, 1), "parentIndex": {"guid": g(0, 0), "position": "a"}, "type": "CANVAS", "name": "Page 1"},
        node((1, 10), (0, 1), "a", "SYMBOL", "Comp", json!({"size": {"x": 200.0, "y": 100.0}})),
        node(
            (1, 11),
            (1, 10),
            "a",
            "FRAME",
            "Inner",
            json!({"overrideKey": g(9, 30), "size": {"x": 100.0, "y": 50.0}, "stackMode": "HORIZONTAL"})
        ),
        node(
            (1, 20),
            (0, 1),
            "b",
            "INSTANCE",
            "Inst",
            json!({
                "size": {"x": 200.0, "y": 100.0},
                "symbolData": {
                    "symbolID": g(1, 10),
                    "symbolOverrides": [{"guidPath": {"guids": [g(9, 30)]}, "stackMode": "NONE"}]
                }
            })
        ),
        node(
            (1, 30),
            (0, 1),
            "c",
            "INSTANCE",
            "PlainInst",
            json!({"size": {"x": 200.0, "y": 100.0}, "symbolData": {"symbolID": g(1, 10), "symbolOverrides": []}})
        )
    ]);
    let inner_of = |doc: &Document, inst: &str| -> Node {
        find(doc, inst).children.iter().filter_map(|id| doc.get(*id)).find(|n| n.name == "Inner").expect("Inner").clone()
    };

    let original = import_fig_node_changes(&changes, "nested-none").expect("import_fig_node_changes");
    assert!(inner_of(&original, "Inst").kind.frame_props().unwrap().auto_layout.is_none(), "the override clears the nested auto layout");

    let bytes = export_fig(&original).expect("export_fig");
    let written = decode_fig_to_json(&bytes).expect("decode_fig_to_json");
    let none_overrides = written["message"]["nodeChanges"]
        .as_array()
        .expect("message.nodeChanges")
        .iter()
        .flat_map(|nc| nc["symbolData"]["symbolOverrides"].as_array().into_iter().flatten())
        .filter(|ov| ov["stackMode"] == "NONE")
        .count();
    assert_eq!(none_overrides, 1, "the override of the nested frame must write stackMode NONE itself");

    let doc = import_fig(&bytes).expect("import_fig");
    let inner = inner_of(&doc, "Inst");
    assert!(inner.kind.frame_props().unwrap().auto_layout.is_none(), "stackMode NONE on a nested frame must survive a .fig round trip");
    assert!(inner.component.overrides.contains(&OverrideField::Layout));
    assert!(inner.component.layout_cleared);
    let plain = inner_of(&doc, "PlainInst");
    assert!(plain.kind.frame_props().unwrap().auto_layout.is_some(), "an instance without the override keeps the nested auto layout");
}

#[test]
fn instance_that_never_set_stack_mode_is_not_exported_as_none() {
    let changes = json!([
        {"guid": g(0, 0), "type": "DOCUMENT", "name": "PlainStackDoc"},
        {"guid": g(0, 1), "parentIndex": {"guid": g(0, 0), "position": "a"}, "type": "CANVAS", "name": "Page 1"},
        node((1, 10), (0, 1), "a", "SYMBOL", "RowComp", json!({"size": {"x": 200.0, "y": 100.0}, "stackMode": "HORIZONTAL"})),
        node((1, 11), (1, 10), "a", "RECTANGLE", "Item", json!({"size": {"x": 40.0, "y": 20.0}})),
        node(
            (1, 20),
            (0, 1),
            "b",
            "INSTANCE",
            "PlainInst",
            json!({"size": {"x": 200.0, "y": 100.0}, "symbolData": {"symbolID": g(1, 10), "symbolOverrides": []}})
        )
    ]);
    let original = import_fig_node_changes(&changes, "plain-stack").expect("import_fig_node_changes");
    let inst = find(&original, "PlainInst");
    eprintln!(
        "plain instance auto_layout = {:?}, overrides = {:?}",
        inst.kind.frame_props().unwrap().auto_layout.as_ref().map(|a| a.direction),
        inst.component.overrides
    );
    let written = decode_fig_to_json(&export_fig(&original).expect("export_fig")).expect("decode_fig_to_json");
    let none_nodes = written["message"]["nodeChanges"].as_array().unwrap().iter().filter(|nc| nc["name"] == "PlainInst" && nc["stackMode"] == "NONE").count();
    assert_eq!(none_nodes, 0, "an instance that never set stackMode NONE must not be written with it");
}

#[test]
fn explicit_flex_sizing_on_the_instance_node_is_not_replaced_by_component_sizes() {
    let sizing_map = |items: &[((u64, u64), &str, f32)]| -> Value {
        let entries: Vec<Value> = items
            .iter()
            .map(|&((s, l), ty, val)| json!({"id": g(s, l), "trackSize": {"minSizing": {"type": ty, "value": val}, "maxSizing": {"type": ty, "value": val}}}))
            .collect();
        json!({"entries": entries})
    };
    let changes = json!([
        {"guid": g(0, 0), "type": "DOCUMENT", "name": "ExplicitFlexDoc"},
        {"guid": g(0, 1), "parentIndex": {"guid": g(0, 0), "position": "a"}, "type": "CANVAS", "name": "Page 1"},
        node(
            (1, 10),
            (0, 1),
            "a",
            "SYMBOL",
            "GridComp",
            json!({
                "size": {"x": 300.0, "y": 120.0},
                "stackMode": "GRID",
                "gridColumns": track_map(&[(9, 3), (9, 4), (9, 5)]),
                "gridRows": track_map(&[(9, 1)]),
                "gridColumnsSizing": sizing_map(&[((9, 3), "FIXED", 80.0), ((9, 4), "FIXED", 120.0), ((9, 5), "FIXED", 100.0)])
            })
        ),
        node(
            (1, 20),
            (0, 1),
            "b",
            "INSTANCE",
            "FlexInst",
            json!({
                "size": {"x": 300.0, "y": 120.0},
                "stackMode": "GRID",
                "gridColumns": track_map(&[(9, 4), (9, 3), (9, 5)]),
                "gridRows": track_map(&[(9, 1)]),
                "gridColumnsSizing": sizing_map(&[((9, 4), "FLEX", 1.0), ((9, 3), "FLEX", 1.0), ((9, 5), "FLEX", 1.0)]),
                "symbolData": {"symbolID": g(1, 10), "symbolOverrides": []}
            })
        )
    ]);
    let doc = import_fig_node_changes(&changes, "explicit-flex").expect("import_fig_node_changes");
    let al = find(&doc, "FlexInst").kind.frame_props().unwrap().auto_layout.as_ref().expect("grid auto_layout").clone();
    assert_eq!(&al.grid_col_tracks[..3], &[GridTrack::Flex(1.0); 3], "the instance sized its own columns, so the component sizes must not replace them");
}

#[test]
fn partial_layout_override_on_a_stack_instance_keeps_layout_and_padding_through_a_fig_round_trip() {
    let changes = json!([
        {"guid": g(0, 0), "type": "DOCUMENT", "name": "PartialLayoutDoc"},
        {"guid": g(0, 1), "parentIndex": {"guid": g(0, 0), "position": "a"}, "type": "CANVAS", "name": "Page 1"},
        node((1, 10), (0, 1), "a", "SYMBOL", "RowComp", json!({"size": {"x": 200.0, "y": 100.0}, "stackMode": "HORIZONTAL", "stackSpacing": 4.0})),
        node((1, 11), (1, 10), "a", "RECTANGLE", "Item", json!({"size": {"x": 40.0, "y": 20.0}})),
        node(
            (1, 20),
            (0, 1),
            "b",
            "INSTANCE",
            "PaddedInst",
            json!({
                "size": {"x": 200.0, "y": 100.0},
                "symbolData": {"symbolID": g(1, 10), "symbolOverrides": [{"guidPath": {"guids": []}, "stackPaddingLeft": 12.0}]}
            })
        ),
        // Clipping is a layout field too, but it changes neither the layout nor its padding.
        node(
            (1, 30),
            (0, 1),
            "c",
            "INSTANCE",
            "ClipInst",
            json!({
                "size": {"x": 200.0, "y": 100.0},
                "symbolData": {"symbolID": g(1, 10), "symbolOverrides": [{"guidPath": {"guids": []}, "frameMaskDisabled": true}]}
            })
        )
    ]);
    let layout_of = |doc: &Document| find(doc, "PaddedInst").kind.frame_props().unwrap().auto_layout.clone();
    let clip_layout_of = |doc: &Document| find(doc, "ClipInst").kind.frame_props().unwrap().auto_layout.clone();

    let original = import_fig_node_changes(&changes, "partial-layout").expect("import_fig_node_changes");
    let al = layout_of(&original).expect("a padding override keeps the component's auto layout");
    assert_eq!((al.direction, al.padding[3], al.item_spacing), (LayoutDirection::Horizontal, 12.0, 4.0), "component layout plus the overridden padding");
    assert!(!find(&original, "PaddedInst").component.layout_cleared, "a padding override is not a layout clear");
    assert!(!find(&original, "ClipInst").component.layout_cleared, "a clipping override is not a layout clear");
    assert_eq!(clip_layout_of(&original).map(|a| (a.direction, a.padding[3])), Some((LayoutDirection::Horizontal, 0.0)), "clipping keeps the component layout");

    let bytes = export_fig(&original).expect("export_fig");
    let written = decode_fig_to_json(&bytes).expect("decode_fig_to_json");
    let none_nodes = written["message"]["nodeChanges"].as_array().unwrap().iter().filter(|nc| nc["stackMode"] == "NONE").count();
    assert_eq!(none_nodes, 0, "no stackMode NONE may be written for a padding override");

    let doc = import_fig(&bytes).expect("import_fig");
    let al = layout_of(&doc).expect("layout survives the round trip");
    assert_eq!((al.direction, al.padding[3], al.item_spacing), (LayoutDirection::Horizontal, 12.0, 4.0), "layout and padding survive the round trip");
    assert_eq!(
        clip_layout_of(&doc).map(|a| (a.direction, a.padding[3])),
        Some((LayoutDirection::Horizontal, 0.0)),
        "the clipped instance keeps its layout through the round trip"
    );
}
