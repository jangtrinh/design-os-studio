//! The fields whose id, type or enum value an export used to get wrong, each pinned to Figma's own id and type, and each
//! round-tripped: export, then import, with nothing lost.

mod common;
use common::{assert_rich_survived, find, rich_document};
use designos_doc::*;
use designos_fig::kiwi::{DefKind, FieldType, Schema};
use designos_fig::testkit::{figma_like_schema, parse_schema_text};
use designos_fig::{decode_fig_to_json, export_fig, import_fig, import_rest_json};
use serde_json::{json, Value};

const REAL_SCHEMA: &str = include_str!("data/figma-real-schema.txt");

fn field_of(schema: &Schema, def: &str, field: &str) -> (u32, String) {
    let d = &schema.defs[schema.find(def).unwrap_or_else(|| panic!("no definition {def}"))];
    let f = d.fields.iter().find(|f| &*f.name == field).unwrap_or_else(|| panic!("no field {def}.{field}"));
    let ty = match f.ty {
        FieldType::Native(n) => format!("{n:?}"),
        FieldType::Def(i) if d.kind == DefKind::Enum => schema.defs[i].name.clone(),
        FieldType::Def(i) => schema.defs[i].name.clone(),
    };
    (f.value, if f.is_array { format!("{ty}[]") } else { ty })
}

/// (definition, field, Figma's id, Figma's type), for every field this round changed.
const PINNED: &[(&str, &str, u32, &str)] = &[
    // wire types of the grid fields
    ("NodeChange", "gridColumns", 436, "GUIDPositionMap"),
    ("NodeChange", "gridRows", 435, "GUIDPositionMap"),
    ("NodeChange", "gridColumnAnchor", 440, "GUID"),
    ("NodeChange", "gridRowAnchor", 439, "GUID"),
    ("NodeChange", "gridColumnsSizing", 474, "GUIDGridTrackSizeMap"),
    ("NodeChange", "gridRowsSizing", 475, "GUIDGridTrackSizeMap"),
    ("NodeChange", "gridAutoTracks", 555, "GridAutoTracks"),
    ("NodeChange", "minSize", 325, "OptionalVector"),
    ("NodeChange", "maxSize", 326, "OptionalVector"),
    ("NodeChange", "maxLines", 351, "Int"),
    ("NodeChange", "overrideLevel", 321, "Int"),
    ("GUIDPositionMapEntry", "position", 2, "String"),
    ("GridTrackSize", "minSizing", 1, "GridTrackSizingFunction"),
    ("GridTrackSizingFunction", "value", 2, "Float"),
    // component properties
    ("ComponentPropDef", "initialValue", 3, "ComponentPropValue"),
    ("ComponentPropDef", "type", 6, "ComponentPropType"),
    ("ComponentPropDef", "preferredValues", 8, "ComponentPropPreferredValues"),
    ("ComponentPropDef", "slotPropConfig", 12, "SlotPropConfig"),
    ("ComponentPropRef", "defID", 2, "GUID"),
    ("ComponentPropRef", "componentPropNodeField", 4, "ComponentPropNodeField"),
    ("ComponentPropValue", "textValue", 2, "TextData"),
    ("VariantPropSpec", "propDefId", 1, "GUID"),
    // the message every export is wrapped in
    ("Message", "type", 1, "MessageType"),
    ("Message", "sessionID", 2, "Uint"),
    ("Message", "ackID", 3, "Uint"),
    ("Message", "nodeChanges", 4, "NodeChange[]"),
    ("Message", "blobs", 6, "Blob[]"),
    // other definitions the exporter wrote with its own ids
    ("Paint", "imageScaleMode", 10, "ImageScaleMode"),
    ("Paint", "colorVar", 21, "VariableData"),
    ("LayoutGrid", "numSections", 4, "Int"),
    ("LayoutGrid", "pattern", 9, "LayoutGridPattern"),
    ("LayoutGrid", "axis", 2, "Axis"),
    ("VariableData", "value", 1, "VariableAnyValue"),
    ("VariableData", "dataType", 2, "VariableDataType"),
    ("TextData", "lines", 12, "TextLineData[]"),
    ("TextLineData", "indentationLevel", 2, "Int"),
    ("ConnectorEndpoint", "endpointNodeID", 1, "GUID"),
    ("ConnectorEndpoint", "magnet", 3, "ConnectorMagnet"),
    ("VectorData", "styleOverrideTable", 3, "NodeChange[]"),
    // enum values
    ("NodeType", "VARIABLE_SET", 31, "MessageType"),
    ("NodeType", "TABLE", 29, "MessageType"),
    ("StrokeCap", "DIAMOND_FILLED", 5, "MessageType"),
    ("StrokeCap", "TRIANGLE_FILLED", 6, "MessageType"),
    ("StrokeCap", "CIRCLE_FILLED", 14, "MessageType"),
    ("VariableDataType", "COLOR", 4, "MessageType"),
    ("VariableDataType", "ALIAS", 3, "MessageType"),
    ("ComponentPropType", "SLOT", 7, "MessageType"),
    ("StackJustify", "SPACE_BETWEEN", 4, "MessageType"),
    ("WindingRule", "ODD", 1, "MessageType"),
    ("BooleanOperation", "XOR", 3, "MessageType"),
    ("TextAutoResize", "HEIGHT", 2, "MessageType"),
    ("SectionStatus", "BUILD", 1, "MessageType"),
    ("LayoutGridPattern", "STRIPES", 0, "MessageType"),
];

#[test]
fn every_changed_field_has_figmas_id_and_type_in_the_embedded_schema_and_in_the_reference() {
    let (ours, figma) = (figma_like_schema(), parse_schema_text(REAL_SCHEMA));
    for &(def, field, id, ty) in PINNED {
        let want = (id, ty.to_string());
        // An enum variant has no type of its own; the dump above names the (meaningless) enum index type.
        let is_enum = ours.defs[ours.find(def).expect(def)].kind == DefKind::Enum;
        let norm = |(i, t): (u32, String)| if is_enum { (i, String::new()) } else { (i, t) };
        assert_eq!(norm(field_of(&ours, def, field)), norm(want.clone()), "embedded {def}.{field}");
        assert_eq!(norm(field_of(&figma, def, field)), norm(want), "reference {def}.{field}");
    }
}

fn decoded(doc: &Document) -> Value {
    decode_fig_to_json(&export_fig(doc).expect("export_fig")).expect("decode")
}

fn changes(v: &Value) -> &Vec<Value> {
    v["message"]["nodeChanges"].as_array().expect("node changes")
}

fn node<'a>(v: &'a Value, name: &str) -> &'a Value {
    changes(v).iter().find(|n| n["name"] == name).unwrap_or_else(|| panic!("no node change named {name}"))
}

/// Button set with every kind of property, built the way the importer builds them from a REST file.
fn props_document() -> Document {
    let rect = |id: &str, name: &str| json!({"id": id, "name": name, "type": "RECTANGLE", "size": {"x": 10, "y": 10}});
    let kids = |p: &str| {
        json!([rect(&format!("{p}1:3"), "Bg"),
            {"id": format!("{p}1:4"), "name": "Label", "type": "TEXT", "characters": "Go", "componentPropertyReferences": {"characters": "Label#1:8"}},
            {"id": format!("{p}1:5"), "name": "Icon", "type": "ELLIPSE", "componentPropertyReferences": {"visible": "Show icon#1:9"}}])
    };
    let file = json!({"name": "props", "document": {"type": "DOCUMENT", "children": [{"id": "0:1", "type": "CANVAS", "name": "P", "children": [
        {"id": "1:1", "name": "Btn", "type": "COMPONENT_SET",
         "componentPropertyDefinitions": {
            "Size": {"type": "VARIANT", "defaultValue": "Small", "variantOptions": ["Small", "Large"]},
            "Show icon#1:9": {"type": "BOOLEAN", "defaultValue": true},
            "Label#1:8": {"type": "TEXT", "defaultValue": "Go"},
            "Swap#1:7": {"type": "INSTANCE_SWAP", "defaultValue": "1:2"}
         },
         "children": [{"id": "1:2", "name": "Size=Small", "type": "COMPONENT", "size": {"x": 100, "y": 40}, "children": kids("")}]},
        {"id": "2:1", "name": "Btn instance", "type": "INSTANCE", "componentId": "1:2", "size": {"x": 100, "y": 40},
         "componentProperties": {"Show icon#1:9": {"type": "BOOLEAN", "value": false}, "Size": {"type": "VARIANT", "value": "Large"}},
         "children": kids("I2:1;")}
    ]}]}});
    let mut doc = import_rest_json(&file.to_string()).expect("import");
    let comp = find(&doc, "Size=Small").id;
    let set = find(&doc, "Btn").id;
    let props = &mut doc.get_mut(set).unwrap().component.props;
    for p in props.iter_mut() {
        if p.name == "Swap" {
            p.kind = PropKind::InstanceSwap { preferred: vec![comp] };
        }
    }
    let inst = find(&doc, "Btn instance").id;
    doc.get_mut(inst).unwrap().component.variant_props.insert("Size".into(), "Large".into());
    let props = &mut doc.get_mut(set).unwrap().component.props;
    props.push(ComponentPropDef { name: "Body#1:20".into(), kind: PropKind::Slot, default: PropValue::Slot(vec![]) });
    doc.get_mut(set).unwrap().component.slot_configs.insert(
        "Body#1:20".into(),
        SlotPropConfig { stretch_child_on_insert: Some(true), min_children: Some(1), max_children: Some(3), preferred: vec![comp], ..Default::default() },
    );
    doc
}

#[test]
fn component_properties_are_written_with_figmas_field_ids_and_survive() {
    let doc = props_document();
    let v = decoded(&doc);
    let set = node(&v, "Btn");
    let defs = set["componentPropDefs"].as_array().expect("defs");
    let by_type = |t: &str| defs.iter().find(|d| d["type"] == t).unwrap_or_else(|| panic!("no {t} definition"));

    let swap = by_type("INSTANCE_SWAP");
    assert!(swap["initialValue"]["guidValue"].is_object(), "the default component is a guid");
    let pref = &swap["preferredValues"]["instanceSwapValues"][0];
    assert_eq!(pref["type"], "COMPONENT");
    let comp = node(&v, "Size=Small")["guid"].clone();
    assert_eq!(pref["key"], format!("{}:{}", comp["sessionID"], comp["localID"]).as_str(), "a component of this file is keyed by its guid");

    let slot = by_type("SLOT");
    assert_eq!(slot["slotPropConfig"]["maxChildren"], 3);
    assert_eq!(slot["preferredValues"]["instanceSwapValues"].as_array().map(Vec::len), Some(1));
    assert_eq!(by_type("VARIANT")["initialValue"]["textValue"]["characters"], "Small");

    let label = node(&v, "Label");
    let r = &label["componentPropRefs"][0];
    assert!(r["defID"].is_object() && r["componentPropNodeField"] == "TEXT_DATA", "{r}");

    // The instance's variant spec holds the property's id and the value; the name is found through the id.
    let inst = node(&v, "Btn instance");
    let spec = &inst["variantPropSpecs"][0];
    assert!(spec["propDefId"].is_object() && spec["value"] == "Large" && spec.get("propertyName").is_none(), "{spec}");
    assert!(inst["pluginData"].as_array().is_none_or(|a| a.iter().all(|e| e["key"] != "field:variantPropSpecs")), "no carrier needed");

    // Round trip.
    let rt = import_fig(&export_fig(&doc).unwrap()).unwrap();
    let (before, after) = (find(&doc, "Btn"), find(&rt, "Btn"));
    let comp_rt = find(&rt, "Size=Small").id;
    let names = |n: &Node| {
        let mut v = n.component.props.iter().map(|p| (p.name.split('#').next().unwrap().to_string(), p.kind.clone())).collect::<Vec<_>>();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    };
    let expect: Vec<_> = names(before)
        .into_iter()
        .map(|(n, k)| (n, if let PropKind::InstanceSwap { .. } = k { PropKind::InstanceSwap { preferred: vec![comp_rt] } } else { k }))
        .collect();
    assert_eq!(names(after), expect);
    assert_eq!(
        after.component.slot_configs.values().next(),
        Some(&SlotPropConfig {
            stretch_child_on_insert: Some(true),
            min_children: Some(1),
            max_children: Some(3),
            preferred: vec![comp_rt],
            ..Default::default()
        })
    );
    let inst_rt = find(&rt, "Btn instance");
    assert_eq!(inst_rt.component.variant_props.get("Size").map(String::as_str), Some("Large"));
    assert_eq!(inst_rt.component.values.get("Show icon"), Some(&PropValue::Bool(false)));
}

#[test]
fn a_variant_spec_with_no_definition_keeps_its_name_in_a_carrier() {
    let mut doc = props_document();
    let inst = find(&doc, "Btn instance").id;
    doc.get_mut(inst).unwrap().component.variant_props.insert("Orphan".into(), "Yes".into());
    let rt = import_fig(&export_fig(&doc).unwrap()).unwrap();
    assert_eq!(find(&rt, "Btn instance").component.variant_props.get("Orphan").map(String::as_str), Some("Yes"));
    assert_eq!(find(&rt, "Btn instance").component.variant_props.get("Size").map(String::as_str), Some("Large"));
}

#[test]
fn layout_guides_are_stripes_on_an_axis_and_round_trip() {
    let mut doc = Document::new("guides");
    let page = doc.add_page("P");
    let id = doc.alloc_id();
    let mut frame = Node::frame(id, Rect::new(0.0, 0.0, 400.0, 300.0));
    frame.name = "Guided".into();
    let red = Color::rgba(255, 0, 0, 25);
    let mut cols = LayoutGrid::columns(4, 8.0, 2.0, red);
    let mut rows = LayoutGrid::rows(0, 4.0, 1.0, red);
    if let LayoutGrid::Columns { alignment, section_size, .. } = &mut cols {
        *alignment = LayoutGridAlign::Center;
        *section_size = 60.0;
    }
    if let LayoutGrid::Rows { visible, .. } = &mut rows {
        *visible = false;
    }
    let grids = vec![LayoutGrid::grid(12.0, red), cols, rows];
    frame.kind.frame_props_mut().unwrap().grids = grids.clone();
    doc.insert(page, frame).unwrap();

    let v = decoded(&doc);
    let g = node(&v, "Guided")["layoutGrids"].as_array().expect("guides").clone();
    assert_eq!((g[0]["pattern"].as_str(), g[0]["axis"].as_str(), g[0]["sectionSize"].as_f64()), (Some("GRID"), Some("X"), Some(12.0)));
    assert_eq!(
        (g[1]["pattern"].as_str(), g[1]["axis"].as_str(), g[1]["type"].as_str(), g[1]["numSections"].as_i64()),
        (Some("STRIPES"), Some("X"), Some("CENTER"), Some(4))
    );
    assert_eq!(
        (g[2]["pattern"].as_str(), g[2]["axis"].as_str(), g[2]["numSections"].as_i64(), g[2]["visible"].as_bool()),
        (Some("STRIPES"), Some("Y"), Some(-1), Some(false))
    );
    assert!(g.iter().all(|x| x.get("count").is_none()), "no invented count field");

    let rt = import_fig(&export_fig(&doc).unwrap()).unwrap();
    assert_eq!(find(&rt, "Guided").kind.frame_props().unwrap().grids, grids);
}

#[test]
fn every_connector_cap_round_trips_on_figmas_stroke_cap_values() {
    let caps = [ConnectorCap::None, ConnectorCap::Arrow, ConnectorCap::Triangle, ConnectorCap::Circle, ConnectorCap::Diamond];
    let mut doc = Document::new("caps");
    let page = doc.add_page("P");
    let mut expect = Vec::new();
    for (i, start) in caps.iter().enumerate() {
        for (j, end) in caps.iter().enumerate() {
            let id = doc.alloc_id();
            let props = ConnectorProps {
                start: ConnectorEnd::free(Vec2::new(i as f32 * 40.0, j as f32 * 40.0)),
                end: ConnectorEnd::free(Vec2::new(i as f32 * 40.0 + 20.0, j as f32 * 40.0 + 20.0)),
                line: ConnectorLine::Straight,
                start_cap: *start,
                end_cap: *end,
                label: None,
            };
            let mut n = Node::connector(id, props);
            n.name = format!("C{i}{j}");
            doc.insert(page, n).unwrap();
            expect.push((n_name(i, j), *start, *end));
        }
    }
    let v = decoded(&doc);
    let wire = |c: ConnectorCap| match c {
        ConnectorCap::None => "NONE",
        ConnectorCap::Arrow => "ARROW_LINES",
        ConnectorCap::Triangle => "ARROW_EQUILATERAL",
        ConnectorCap::Circle => "CIRCLE_FILLED",
        ConnectorCap::Diamond => "DIAMOND_FILLED",
    };
    let rt = import_fig(&export_fig(&doc).unwrap()).unwrap();
    for (name, start, end) in expect {
        let n = node(&v, &name);
        assert_eq!((n["connectorStartCap"].as_str(), n["connectorEndCap"].as_str()), (Some(wire(start)), Some(wire(end))), "{name} on the wire");
        let NodeKind::Connector(p) = &find(&rt, &name).kind else { panic!("{name} is a connector") };
        assert_eq!((p.start_cap, p.end_cap), (start, end), "{name} after the round trip");
    }
}

fn n_name(i: usize, j: usize) -> String {
    format!("C{i}{j}")
}

#[test]
fn size_limits_are_vectors_with_zero_and_infinity_for_the_side_that_is_not_set() {
    let mut doc = rich_document();
    let page = doc.pages[0];
    let id = doc.alloc_id();
    let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0));
    n.name = "Limited".into();
    n.layout_child.min_w = Some(180.0);
    n.layout_child.max_h = Some(90.0);
    doc.insert(page, n).unwrap();
    let v = decoded(&doc);
    let n = node(&v, "Limited");
    assert_eq!((n["minSize"]["value"]["x"].as_f64(), n["minSize"]["value"]["y"].as_f64()), (Some(180.0), Some(0.0)));
    assert_eq!((n["maxSize"]["value"]["x"].as_f64(), n["maxSize"]["value"]["y"].as_f64()), (None, Some(90.0)), "an unset maximum is infinite (null here)");
    let rt = import_fig(&export_fig(&doc).unwrap()).unwrap();
    let lc = find(&rt, "Limited").layout_child;
    assert_eq!((lc.min_w, lc.min_h, lc.max_w, lc.max_h), (Some(180.0), None, None, Some(90.0)));
}

#[test]
fn text_lists_use_integer_indentation_and_auto_truncation_uses_the_truncation_field() {
    let mut doc = Document::new("text");
    let page = doc.add_page("P");
    let id = doc.alloc_id();
    let mut n = Node::text(id, Rect::new(0.0, 0.0, 100.0, 40.0), "one\ntwo", TextStyle::default());
    n.name = "Listed".into();
    let NodeKind::Text(tp) = &mut n.kind else { panic!("text") };
    tp.auto_resize = TextAutoResize::Truncate;
    tp.max_lines = Some(2);
    doc.insert(page, n).unwrap();
    let v = decoded(&doc);
    let n = node(&v, "Listed");
    assert_eq!((n["textAutoResize"].as_str(), n["textTruncation"].as_str(), n["maxLines"].as_i64()), (Some("NONE"), Some("ENDING"), Some(2)));
    let rt = import_fig(&export_fig(&doc).unwrap()).unwrap();
    let NodeKind::Text(tp) = &find(&rt, "Listed").kind else { panic!("text") };
    assert_eq!((tp.auto_resize, tp.max_lines), (TextAutoResize::Truncate, Some(2)));
}

#[test]
fn the_rich_document_round_trips_through_figmas_shapes() {
    assert_rich_survived(&import_fig(&export_fig(&rich_document()).unwrap()).unwrap());
}
