//! Regression tests for #541:
//! 1. Migrate legacy `gridChildHorizontalAlign` / `gridChildVerticalAlign` from `Node::fig_metadata`
//!    onto `LayoutChild` on load (and during `.fig` export) and remove the migrated metadata keys.
//! 2. Preserve per-axis grid alignment inheritance during in-document `sync_instances` when an
//!    instance child has a margin-only or single-axis `OverrideField::Layout` override.
//! 3. Centre the full margin box when child margins exceed the cell size, matching the flow solver.

use designos_doc::*;
use designos_fig::{export_fig, import_fig, import_fig_node_changes, import_rest_json};
use serde_json::{json, Value};

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("missing node {name}"))
}

#[test]
fn legacy_designos_fig_metadata_grid_child_align_migrates_on_load_and_survives_fig_roundtrip() {
    let mut d = Document::new("legacy-align");
    let page = d.add_page("Page 1");
    let gid = d.alloc_id();
    let mut g = Node::frame(gid, Rect::new(0.0, 0.0, 100.0, 100.0));
    let mut al = AutoLayout {
        direction: LayoutDirection::Grid,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        grid_columns: 1,
        grid_rows: 1,
        ..Default::default()
    };
    al.grid_col_tracks[0] = GridTrack::Fixed(100.0);
    al.grid_row_tracks[0] = GridTrack::Fixed(100.0);
    g.kind = NodeKind::Frame(FrameProps { auto_layout: Some(al), ..Default::default() });
    let g = d.insert(page, g).unwrap();

    let cid = d.alloc_id();
    let mut c = Node::rectangle(cid, Rect::new(0.0, 0.0, 40.0, 20.0));
    c.name = "LegacyAlignedChild".into();
    // Simulate a .designos saved between #461 and #479: alignment stored only in fig_metadata.
    c.fig_metadata.insert("gridChildHorizontalAlign".into(), "CENTER".into());
    c.fig_metadata.insert("gridChildVerticalAlign".into(), "MAX".into());
    d.insert(g, c).unwrap();

    let saved_json = d.to_json().unwrap();
    let loaded = Document::from_json(&saved_json).unwrap();
    let loaded_child = find(&loaded, "LegacyAlignedChild");
    assert_eq!(
        (loaded_child.layout_child.grid_align_h, loaded_child.layout_child.grid_align_v),
        (GridChildAlign::Center, GridChildAlign::End),
        "legacy fig_metadata alignment must migrate onto LayoutChild on load"
    );
    assert!(
        !loaded_child.fig_metadata.contains_key("gridChildHorizontalAlign") && !loaded_child.fig_metadata.contains_key("gridChildVerticalAlign"),
        "migrated alignment keys must be removed from fig_metadata"
    );

    let bytes = export_fig(&loaded).expect("export_fig");
    let rt = import_fig(&bytes).expect("import_fig");
    let rt_child = find(&rt, "LegacyAlignedChild");
    assert_eq!(
        (rt_child.layout_child.grid_align_h, rt_child.layout_child.grid_align_v),
        (GridChildAlign::Center, GridChildAlign::End),
        "migrated alignment must survive export_fig -> import_fig"
    );
    assert!(!rt_child.fig_metadata.contains_key("gridChildHorizontalAlign") && !rt_child.fig_metadata.contains_key("gridChildVerticalAlign"));
}

#[test]
fn in_document_component_sync_copies_unoverridden_grid_align_axes_under_margin_or_single_axis_override() {
    let g = |s: u64, l: u64| -> Value { json!({"sessionID": s, "localID": l}) };
    let node = |id: (u64, u64), parent: (u64, u64), pos: &str, ty: &str, name: &str, extra: Value| -> Value {
        let mut v = json!({
            "guid": g(id.0, id.1),
            "parentIndex": {"guid": g(parent.0, parent.1), "position": pos},
            "type": ty,
            "name": name,
            "phase": "CREATED",
            "visible": true,
            "opacity": 1.0,
            "size": {"x": 40.0, "y": 20.0},
            "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
        });
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        v
    };

    let changes = json!([
        {"guid": g(0, 0), "type": "DOCUMENT", "name": "SyncAlignDoc"},
        {"guid": g(0, 1), "parentIndex": {"guid": g(0, 0), "position": "a"}, "type": "CANVAS", "name": "Page 1"},
        node(
            (1, 10),
            (0, 1),
            "a",
            "SYMBOL",
            "AlignComp",
            json!({
                "size": {"x": 100.0, "y": 100.0},
                "stackMode": "GRID",
                "gridColumns": 1,
                "gridRows": 1,
                "gridColumnsSizing": "100px",
                "gridRowsSizing": "100px"
            })
        ),
        node(
            (1, 11),
            (1, 10),
            "a",
            "RECTANGLE",
            "AlignedChild",
            json!({
                "overrideKey": g(9, 11),
                "gridChildHorizontalAlign": "CENTER",
                "gridChildVerticalAlign": "MAX"
            })
        ),
        node(
            (1, 12),
            (0, 1),
            "b",
            "INSTANCE",
            "MarginInst",
            json!({
                "size": {"x": 100.0, "y": 100.0},
                "symbolData": {
                    "symbolID": g(1, 10),
                    "symbolOverrides": [
                        {"guidPath": {"guids": [g(9, 11)]}, "stackChildMarginLeft": 8.0}
                    ]
                }
            })
        ),
        node(
            (1, 13),
            (0, 1),
            "c",
            "INSTANCE",
            "HorizOnlyInst",
            json!({
                "size": {"x": 100.0, "y": 100.0},
                "symbolData": {
                    "symbolID": g(1, 10),
                    "symbolOverrides": [
                        {"guidPath": {"guids": [g(9, 11)]}, "gridChildHorizontalAlign": "MIN"}
                    ]
                }
            })
        ),
        node(
            (1, 14),
            (0, 1),
            "d",
            "INSTANCE",
            "PlainInst",
            json!({
                "size": {"x": 100.0, "y": 100.0},
                "symbolData": {
                    "symbolID": g(1, 10),
                    "symbolOverrides": []
                }
            })
        )
    ]);

    let mut doc = import_fig_node_changes(&changes, "SyncAlignDoc").expect("import SyncAlignDoc");
    let margin_inst = find(&doc, "MarginInst").id;
    let horiz_inst = find(&doc, "HorizOnlyInst").id;
    let plain_inst = find(&doc, "PlainInst").id;
    let child_state = |doc: &Document, inst_id: NodeId| {
        let inst = doc.get(inst_id).unwrap();
        let ch = doc.get(*inst.children.first().expect("instance child")).unwrap();
        (ch.layout_child.grid_align_h, ch.layout_child.grid_align_v, ch.layout_child.margin[3])
    };

    assert_eq!(child_state(&doc, margin_inst), (GridChildAlign::Center, GridChildAlign::End, 8.0));
    assert_eq!(child_state(&doc, horiz_inst), (GridChildAlign::Start, GridChildAlign::End, 0.0));
    assert_eq!(child_state(&doc, plain_inst), (GridChildAlign::Center, GridChildAlign::End, 0.0));

    // Change the main component child to Start / Start in-memory and sync instances.
    let comp_id = find(&doc, "AlignComp").id;
    let main_child_id = doc.get(comp_id).unwrap().children[0];
    {
        let mc = doc.get_mut(main_child_id).unwrap();
        mc.layout_child.grid_align_h = GridChildAlign::Start;
        mc.layout_child.grid_align_v = GridChildAlign::Start;
    }
    let cmd = sync_instances(&mut doc, comp_id);
    cmd.apply(&mut doc).unwrap();

    assert_eq!(child_state(&doc, plain_inst), (GridChildAlign::Start, GridChildAlign::Start, 0.0));
    assert_eq!(
        child_state(&doc, margin_inst),
        (GridChildAlign::Start, GridChildAlign::Start, 8.0),
        "margin-only Layout override must still follow main component's grid_align_h/v on sync"
    );
    assert_eq!(
        child_state(&doc, horiz_inst),
        (GridChildAlign::Start, GridChildAlign::Start, 0.0),
        "vertical axis was not overridden on HorizOnlyInst and must follow main component on sync"
    );

    // Now change the main component child to End / Center and sync again: HorizOnlyInst keeps Start on H and follows Center on V.
    {
        let mc = doc.get_mut(main_child_id).unwrap();
        mc.layout_child.grid_align_h = GridChildAlign::End;
        mc.layout_child.grid_align_v = GridChildAlign::Center;
    }
    let cmd = sync_instances(&mut doc, comp_id);
    cmd.apply(&mut doc).unwrap();

    assert_eq!(
        child_state(&doc, horiz_inst),
        (GridChildAlign::Start, GridChildAlign::Center, 0.0),
        "overridden horizontal axis stays Start while unoverridden vertical axis follows Center"
    );
}

#[test]
fn grid_cell_offset_with_margins_larger_than_cell_centres_margin_box_like_flow_solver() {
    let mut d = Document::new("large-margins");
    let page = d.add_page("p");

    // 100x100 grid cell with a 40x40 child and margins [0, 40, 0, 80], aligned Center/Center.
    let gid = d.alloc_id();
    let mut g = Node::frame(gid, Rect::new(0.0, 0.0, 100.0, 100.0));
    let mut grid_al = AutoLayout {
        direction: LayoutDirection::Grid,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        grid_columns: 1,
        grid_rows: 1,
        ..Default::default()
    };
    grid_al.grid_col_tracks[0] = GridTrack::Fixed(100.0);
    grid_al.grid_row_tracks[0] = GridTrack::Fixed(100.0);
    g.kind = NodeKind::Frame(FrameProps { auto_layout: Some(grid_al), ..Default::default() });
    let g = d.insert(page, g).unwrap();

    let gcid = d.alloc_id();
    let mut gc = Node::rectangle(gcid, Rect::new(0.0, 0.0, 40.0, 40.0));
    gc.layout_child.margin = [0.0, 40.0, 0.0, 80.0];
    gc.layout_child.grid_align_h = GridChildAlign::Center;
    gc.layout_child.grid_align_v = GridChildAlign::Center;
    let gc = d.insert(g, gc).unwrap();

    // Equivalent 100x100 vertical flow frame with counter_align = Center and primary_align = Center.
    let fid = d.alloc_id();
    let mut f = Node::frame(fid, Rect::new(200.0, 0.0, 100.0, 100.0));
    let flow_al = AutoLayout {
        direction: LayoutDirection::Vertical,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        primary_align: LayoutAlign::Center,
        counter_align: LayoutAlign::Center,
        ..Default::default()
    };
    f.kind = NodeKind::Frame(FrameProps { auto_layout: Some(flow_al), ..Default::default() });
    let f = d.insert(page, f).unwrap();

    let fcid = d.alloc_id();
    let mut fc = Node::rectangle(fcid, Rect::new(0.0, 0.0, 40.0, 40.0));
    fc.layout_child.margin = [0.0, 40.0, 0.0, 80.0];
    let fc = d.insert(f, fc).unwrap();

    d.relayout_subtree(g);
    d.relayout_subtree(f);

    let (gx, gy) = (d.get(gc).unwrap().transform.tx, d.get(gc).unwrap().transform.ty);
    let (fx, fy) = (d.get(fc).unwrap().transform.tx, d.get(fc).unwrap().transform.ty);
    assert_eq!((fx, fy), (50.0, 30.0));
    assert_eq!((gx, gy), (fx, fy), "grid child with margins larger than the cell must centre its margin box at (50, 30) like the flow solver");
}

fn rest_grid_file() -> Value {
    let child = |id: &str, h: &str, v: &str| {
        json!({"id": id, "name": "AlignedChild", "type": "RECTANGLE", "size": {"x": 40, "y": 20},
               "gridChildHorizontalAlign": h, "gridChildVerticalAlign": v})
    };
    let grid = |id: &str, name: &str, ty: &str, kids: Value| {
        let mut v = json!({"id": id, "name": name, "type": ty, "layoutMode": "GRID", "size": {"x": 100, "y": 100},
                           "gridColumnCount": 1, "gridRowCount": 1, "children": kids});
        v["x"] = json!(0);
        v
    };
    let mut inst = grid("2:1", "RestOverriddenInst", "INSTANCE", json!([child("I2:1;1:3", "MAX", "CENTER")]));
    inst["componentId"] = json!("1:2");
    inst["overrides"] = json!([{"id": "I2:1;1:3", "overriddenFields": ["gridChildHorizontalAlign"]}]);
    json!({
        "name": "rest-align",
        "document": {"type": "DOCUMENT", "children": [{"id": "0:1", "type": "CANVAS", "name": "P", "children": [
            grid("1:2", "RestComp", "COMPONENT", json!([child("1:3", "CENTER", "CENTER")])),
            inst
        ]}]}
    })
}

#[test]
fn rest_overridden_fields_grid_align_keep_the_overridden_axis_through_component_sync() {
    let mut doc = import_rest_json(&rest_grid_file().to_string()).expect("import_rest_json");
    let comp = find(&doc, "RestComp").id;
    let inst = find(&doc, "RestOverriddenInst");
    let ch_id = inst.children[0];
    let ch = doc.get(ch_id).unwrap();
    assert!(ch.component.overrides.contains(&OverrideField::Layout));
    assert_eq!(ch.component.grid_align_overrides, [true, false], "gridChildHorizontalAlign marks only the horizontal axis");
    assert_eq!((ch.layout_child.grid_align_h, ch.layout_child.grid_align_v), (GridChildAlign::End, GridChildAlign::Center));

    let main_child = doc.get(comp).unwrap().children[0];
    {
        let mc = doc.get_mut(main_child).unwrap();
        mc.layout_child.grid_align_h = GridChildAlign::Start;
        mc.layout_child.grid_align_v = GridChildAlign::End;
    }
    sync_instances(&mut doc, comp).apply(&mut doc).unwrap();
    let ch = doc.get(ch_id).unwrap();
    assert_eq!(
        (ch.layout_child.grid_align_h, ch.layout_child.grid_align_v),
        (GridChildAlign::End, GridChildAlign::End),
        "REST-overridden horizontal axis stays End; vertical follows the main"
    );
}

/// Removes every `grid_align_overrides` key, the shape of a `.designos` saved before the key existed.
fn strip_grid_align_overrides(v: &mut Value) {
    match v {
        Value::Object(m) => {
            m.remove("grid_align_overrides");
            m.values_mut().for_each(strip_grid_align_overrides);
        }
        Value::Array(a) => a.iter_mut().for_each(strip_grid_align_overrides),
        _ => {}
    }
}

#[test]
fn designos_saved_without_axis_flags_infers_them_from_the_layout_override_on_load() {
    let g = |s: u64, l: u64| -> Value { json!({"sessionID": s, "localID": l}) };
    let node = |id: (u64, u64), parent: (u64, u64), ty: &str, name: &str, extra: Value| -> Value {
        let mut v = json!({
            "guid": g(id.0, id.1), "parentIndex": {"guid": g(parent.0, parent.1), "position": "a"},
            "type": ty, "name": name, "phase": "CREATED", "visible": true, "opacity": 1.0,
            "size": {"x": 40.0, "y": 20.0},
            "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
        });
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        v
    };
    let changes = json!([
        {"guid": g(0, 0), "type": "DOCUMENT", "name": "PrePatchDoc"},
        {"guid": g(0, 1), "parentIndex": {"guid": g(0, 0), "position": "a"}, "type": "CANVAS", "name": "Page 1"},
        node((1, 10), (0, 1), "SYMBOL", "AlignComp", json!({
            "size": {"x": 100.0, "y": 100.0}, "stackMode": "GRID", "gridColumns": 1, "gridRows": 1,
            "gridColumnsSizing": "100px", "gridRowsSizing": "100px"})),
        node((1, 11), (1, 10), "RECTANGLE", "AlignedChild", json!({
            "overrideKey": g(9, 11), "gridChildHorizontalAlign": "CENTER", "gridChildVerticalAlign": "CENTER"})),
        node((1, 12), (0, 1), "INSTANCE", "OlderInst", json!({
            "size": {"x": 100.0, "y": 100.0},
            "symbolData": {"symbolID": g(1, 10), "symbolOverrides": [
                {"guidPath": {"guids": [g(9, 11)]}, "gridChildHorizontalAlign": "MAX"}]}}))
    ]);
    let doc = import_fig_node_changes(&changes, "PrePatchDoc").expect("import");
    let inst_id = find(&doc, "OlderInst").id;
    let ch_id = doc.get(inst_id).unwrap().children[0];
    assert!(doc.get(ch_id).unwrap().component.overrides.contains(&OverrideField::Layout));

    let mut json_value: Value = serde_json::from_str(&doc.to_json().unwrap()).unwrap();
    strip_grid_align_overrides(&mut json_value);
    let mut doc = Document::from_json(&json_value.to_string()).expect("from_json");

    let ch = doc.get(ch_id).unwrap();
    assert_eq!(ch.component.grid_align_overrides, [true, false], "only the axis that differs from the main child is flagged");

    let comp = find(&doc, "AlignComp").id;
    let main_child = doc.get(comp).unwrap().children[0];
    {
        let mc = doc.get_mut(main_child).unwrap();
        mc.layout_child.grid_align_h = GridChildAlign::Start;
        mc.layout_child.grid_align_v = GridChildAlign::End;
    }
    sync_instances(&mut doc, comp).apply(&mut doc).unwrap();
    let ch = doc.get(ch_id).unwrap();
    assert_eq!(
        (ch.layout_child.grid_align_h, ch.layout_child.grid_align_v),
        (GridChildAlign::End, GridChildAlign::End),
        "inferred horizontal override stays End; vertical follows the main"
    );
}

#[test]
fn instance_expansion_clears_axis_flags_copied_from_the_component_subtree() {
    let g = |s: u64, l: u64| -> Value { json!({"sessionID": s, "localID": l}) };
    let node = |id: (u64, u64), parent: (u64, u64), ty: &str, name: &str, extra: Value| -> Value {
        let mut v = json!({
            "guid": g(id.0, id.1), "parentIndex": {"guid": g(parent.0, parent.1), "position": "a"},
            "type": ty, "name": name, "phase": "CREATED", "visible": true, "opacity": 1.0,
            "size": {"x": 40.0, "y": 20.0},
            "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
        });
        for (k, x) in extra.as_object().unwrap() {
            v[k] = x.clone();
        }
        v
    };
    let changes = json!([
        {"guid": g(0, 0), "type": "DOCUMENT", "name": "ExpandDoc"},
        {"guid": g(0, 1), "parentIndex": {"guid": g(0, 0), "position": "a"}, "type": "CANVAS", "name": "Page 1"},
        node((1, 10), (0, 1), "SYMBOL", "InnerComp", json!({
            "stackMode": "GRID", "gridColumns": 1, "gridRows": 1, "gridColumnsSizing": "100px", "gridRowsSizing": "100px"})),
        node((1, 11), (1, 10), "RECTANGLE", "AlignedChild", json!({
            "overrideKey": g(9, 11), "gridChildHorizontalAlign": "CENTER", "gridChildVerticalAlign": "CENTER"})),
        node((1, 20), (0, 1), "SYMBOL", "OuterComp", json!({"size": {"x": 100.0, "y": 100.0}})),
        node((1, 21), (1, 20), "INSTANCE", "NestedInst", json!({
            "symbolData": {"symbolID": g(1, 10), "symbolOverrides": [
                {"guidPath": {"guids": [g(9, 11)]}, "gridChildHorizontalAlign": "MAX"}]}})),
        node((1, 30), (0, 1), "INSTANCE", "OuterInst", json!({
            "size": {"x": 100.0, "y": 100.0}, "symbolData": {"symbolID": g(1, 20), "symbolOverrides": []}}))
    ]);
    let doc = import_fig_node_changes(&changes, "ExpandDoc").expect("import");
    let leaf = |root: &str| {
        let n = find(&doc, root);
        let nested = doc.get(n.children[0]).expect("nested instance");
        doc.get(nested.children[0]).expect("nested child")
    };
    let in_comp = leaf("OuterComp");
    assert_eq!(in_comp.component.grid_align_overrides, [true, false], "the nested override lives on the component's own nested child");
    assert_eq!(in_comp.layout_child.grid_align_h, GridChildAlign::End);
    let cloned = leaf("OuterInst");
    assert_eq!(cloned.component.source, Some(in_comp.id));
    assert!(cloned.component.overrides.is_empty());
    assert_eq!(cloned.component.grid_align_overrides, [false, false], "a fresh clone starts with no axis overrides, like copy_rec");
}
