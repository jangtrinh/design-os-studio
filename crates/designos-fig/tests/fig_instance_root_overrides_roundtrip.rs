//! An instance keeps its own overrides across a `.fig` save and reopen.
//!
//! Figma records what is overridden on the instance itself (its resize, a faded opacity, ...) as a
//! `symbolOverrides` entry whose `guidPath` is the instance's `symbolID`; the Untitled UI PRO v8.0 kit has
//! 36,403 such entries. The importer turns each key of such an entry into an override flag on the instance.
//! The writer has to emit the entry again, or the reopened instance follows its main on the next sync and
//! `--export-spec` omits the fields the flag gates.
use designos_doc::*;
use designos_fig::{decode_fig_to_json, export_fig, import_fig, import_fig_node_changes};
use serde_json::{json, Value};
use std::collections::BTreeSet;

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

fn g(s: u32, l: u32) -> Value {
    json!({"sessionID": s, "localID": l})
}

fn node(guid: (u32, u32), parent: (u32, u32), pos: &str, ty: &str, name: &str, extra: Value) -> Value {
    let mut m = json!({
        "guid": g(guid.0, guid.1),
        "phase": "CREATED",
        "parentIndex": {"guid": g(parent.0, parent.1), "position": pos},
        "type": ty,
        "name": name,
        "visible": true,
        "opacity": 1.0,
        "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
    });
    if let (Some(dst), Some(src)) = (m.as_object_mut(), extra.as_object()) {
        for (k, v) in src {
            dst.insert(k.clone(), v.clone());
        }
    }
    m
}

/// A placed `Badge` (main 44x22) whose own fields are `own` and whose root entry (`guidPath = [symbolID]`) is `entry`.
/// `entry` is merged over the path, so `Value::Null` places an instance with no override at all.
fn placed_badge(local: u32, pos: &str, name: &str, own: Value, entry: Value) -> Value {
    let mut sd = json!({"symbolID": g(1, 10)});
    if let Some(e) = entry.as_object() {
        let mut o = json!({"guidPath": {"guids": [g(1, 10)]}});
        for (k, v) in e {
            o[k] = v.clone();
        }
        sd["symbolOverrides"] = json!([o]);
    }
    let mut fields = json!({"size": {"x": 44.0, "y": 22.0}, "symbolData": sd});
    if let (Some(dst), Some(src)) = (fields.as_object_mut(), own.as_object()) {
        for (k, v) in src {
            dst.insert(k.clone(), v.clone());
        }
    }
    node((3, local), (0, 1), pos, "INSTANCE", name, fields)
}

fn solid(r: f64, gr: f64, b: f64) -> Value {
    json!({"type": "SOLID", "color": {"r": r, "g": gr, "b": b, "a": 1.0}, "opacity": 1.0, "visible": true, "blendMode": "NORMAL"})
}

/// One placed `Badge` per kind of own override: the node and its root entry both carry `fields`, as in a Figma file.
/// `check` asserts the value the override set.
struct Family {
    name: &'static str,
    fields: Value,
    flag: OverrideField,
    check: fn(&Node),
}

fn families() -> Vec<Family> {
    vec![
        Family { name: "Renamed badge", fields: json!({"name": "Renamed badge"}), flag: OverrideField::Name, check: |n| assert_eq!(n.name, "Renamed badge") },
        Family { name: "Hidden badge", fields: json!({"visible": false}), flag: OverrideField::Visible, check: |n| assert!(!n.visible) },
        Family { name: "Locked badge", fields: json!({"locked": true}), flag: OverrideField::Locked, check: |n| assert!(n.locked) },
        Family {
            name: "Blended badge",
            fields: json!({"blendMode": "MULTIPLY"}),
            flag: OverrideField::Blend,
            check: |n| assert_eq!(n.blend, BlendMode::Multiply),
        },
        Family {
            name: "Filled badge",
            fields: json!({"fillPaints": [solid(1.0, 0.0, 0.0)]}),
            flag: OverrideField::Fills,
            check: |n| match n.fills.as_slice() {
                [Paint { kind: PaintKind::Solid(c), .. }] => assert_eq!((c.r, c.g, c.b), (1.0, 0.0, 0.0)),
                other => panic!("one solid fill expected, got {other:?}"),
            },
        },
        Family {
            name: "Outlined badge",
            fields: json!({"strokePaints": [solid(0.0, 0.0, 1.0)], "strokeWeight": 2.0}),
            flag: OverrideField::Stroke,
            check: |n| assert_eq!((n.stroke.paints.len(), n.stroke.weight), (1, 2.0)),
        },
        Family {
            name: "Rounded badge",
            fields: json!({"cornerRadius": 6.0}),
            flag: OverrideField::CornerRadii,
            check: |n| assert_eq!(n.corner_radii, [6.0; 4]),
        },
        Family {
            name: "Shadowed badge",
            fields: json!({"effects": [{"type": "DROP_SHADOW", "color": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 0.25}, "offset": {"x": 0.0, "y": 2.0}, "radius": 4.0, "spread": 0.0, "visible": true}]}),
            flag: OverrideField::Effects,
            check: |n| assert!(matches!(n.effects.as_slice(), [Effect::DropShadow { radius, .. }] if *radius == 4.0), "{:?}", n.effects),
        },
        Family {
            name: "Unclipped badge",
            fields: json!({"frameMaskDisabled": true}),
            flag: OverrideField::Layout,
            check: |n| assert!(!n.kind.frame_props().expect("an instance is a frame").clips_content),
        },
    ]
}

fn changes() -> Value {
    let mut all = base_changes();
    for (i, f) in families().iter().enumerate() {
        all.push(placed_badge(10 + i as u32, &format!("g{i}"), f.name, f.fields.clone(), f.fields.clone()));
    }
    Value::Array(all)
}

fn base_changes() -> Vec<Value> {
    vec![
        node((0, 0), (0, 0), "!", "DOCUMENT", "Doc", json!({})),
        node((0, 1), (0, 0), "a", "CANVAS", "Page", json!({})),
        // Main `Badge` 44x22.
        node((1, 10), (0, 1), "a", "SYMBOL", "Badge", json!({"size": {"x": 44.0, "y": 22.0}})),
        node((1, 11), (1, 10), "a", "RECTANGLE", "Dot", json!({"size": {"x": 8.0, "y": 8.0}})),
        // Main `Card` 200x100 holding a nested `Badge` instance.
        node((2, 10), (0, 1), "b", "SYMBOL", "Card", json!({"size": {"x": 200.0, "y": 100.0}})),
        node((2, 11), (2, 10), "a", "INSTANCE", "Inner badge", json!({"size": {"x": 44.0, "y": 22.0}, "symbolData": {"symbolID": g(1, 10)}})),
        // Resized to 53x22 through its root entry.
        placed_badge(1, "c", "Resized badge", json!({"size": {"x": 53.0, "y": 22.0}}), json!({"size": {"x": 53.0, "y": 22.0}})),
        // A placed `Card` whose nested badge is resized to 60x22.
        node(
            (3, 2),
            (0, 1),
            "d",
            "INSTANCE",
            "Card copy",
            json!({
                "size": {"x": 200.0, "y": 100.0},
                "symbolData": {"symbolID": g(2, 10), "symbolOverrides": [{"guidPath": {"guids": [g(2, 11)]}, "size": {"x": 60.0, "y": 22.0}}]}
            }),
        ),
        // Faded to half opacity through its root entry.
        placed_badge(3, "e", "Faded badge", json!({"opacity": 0.5}), json!({"opacity": 0.5})),
        // Overrides nothing.
        placed_badge(4, "f", "Plain badge", json!({}), Value::Null),
    ]
}

fn nested_badge(doc: &Document) -> &Node {
    let card = find(doc, "Card copy");
    doc.get(card.children[0]).expect("nested badge inside the Card instance")
}

/// Imports the fixture, then saves and reopens it twice, calling `check` on the document of every stage.
fn through_two_saves(check: impl Fn(&str, &Document)) {
    let mut doc = import_fig_node_changes(&changes(), "Doc").expect("import");
    check("import", &doc);
    for round in 1..=2 {
        doc = import_fig(&export_fig(&doc).expect("export")).expect("reimport");
        check(&format!("round {round}"), &doc);
    }
}

#[test]
fn fig_save_and_reopen_keeps_instance_size_overrides() {
    through_two_saves(|stage, doc| {
        let top = find(doc, "Resized badge");
        assert_eq!(top.size, Vec2::new(53.0, 22.0), "{stage}: placed instance keeps its size");
        assert!(top.component.overrides.contains(&OverrideField::Size), "{stage}: placed instance keeps its Size override");
        let nested = nested_badge(doc);
        assert_eq!(nested.size, Vec2::new(60.0, 22.0), "{stage}: nested instance keeps its size");
        assert!(nested.component.overrides.contains(&OverrideField::Size), "{stage}: nested instance keeps its Size override");
        assert_eq!(find(doc, "Badge").size, Vec2::new(44.0, 22.0), "{stage}: the main is unchanged");
    });
}

#[test]
fn fig_save_and_reopen_keeps_instance_opacity_overrides() {
    through_two_saves(|stage, doc| {
        let faded = find(doc, "Faded badge");
        assert_eq!(faded.opacity, 0.5, "{stage}: placed instance keeps its opacity");
        assert!(faded.component.overrides.contains(&OverrideField::Opacity), "{stage}: placed instance keeps its Opacity override");
        assert!(!faded.component.overrides.contains(&OverrideField::Size), "{stage}: and gains no other override");
    });
}

#[test]
fn fig_save_and_reopen_keeps_every_kind_of_instance_override() {
    through_two_saves(|stage, doc| {
        for f in families() {
            let n = find(doc, f.name);
            assert_eq!(n.component.overrides, BTreeSet::from([f.flag]), "{stage}: {} keeps exactly its {:?} override", f.name, f.flag);
            (f.check)(n);
        }
    });
}

#[test]
fn the_root_entry_is_written_only_for_what_the_instance_overrides() {
    through_two_saves(|stage, doc| {
        let plain = find(doc, "Plain badge");
        assert!(plain.component.overrides.is_empty(), "{stage}: {:?}", plain.component.overrides);
    });

    let doc = import_fig_node_changes(&changes(), "Doc").expect("import");
    let decoded = decode_fig_to_json(&export_fig(&doc).expect("export")).expect("decode");
    let changes = decoded["message"]["nodeChanges"].as_array().expect("node changes");
    let sym_data = |name: &str| changes.iter().find(|n| n["name"] == name).unwrap_or_else(|| panic!("no node change named {name}"))["symbolData"].clone();
    let own_entries = |name: &str| {
        let sd = sym_data(name);
        let symbol = sd["symbolID"].clone();
        sd["symbolOverrides"].as_array().cloned().unwrap_or_default().into_iter().filter(|o| o["guidPath"]["guids"] == json!([symbol])).collect::<Vec<_>>()
    };
    assert!(own_entries("Plain badge").is_empty(), "{:?}", own_entries("Plain badge"));
    let resized = own_entries("Resized badge");
    assert_eq!(resized.len(), 1, "one entry for the instance itself: {resized:?}");
    assert_eq!(resized[0]["size"], json!({"x": 53.0, "y": 22.0}));
    assert!(resized[0].get("opacity").is_none(), "only what the instance overrides is written: {:?}", resized[0]);
}

const COLLECTION: &str = "VariableCollectionId:3:1";
const STOP_VAR: &str = "VariableID:3:37";
const FILL_VAR: &str = "VariableID:3:38";

fn color_variable(doc: &mut Document, id: &str, name: &str, value: Color) {
    doc.variable_collections.get_mut(COLLECTION).expect("collection").variables.push(id.into());
    doc.variables.insert(
        id.into(),
        Variable {
            id: id.into(),
            name: name.into(),
            collection: COLLECTION.into(),
            kind: VariableKind::Color,
            values: [("3:10".to_string(), VariableValue::Color(value))].into_iter().collect(),
            description: String::new(),
            scopes: Vec::new(),
            hidden_from_publishing: false,
            key: None,
            code_syntax: Default::default(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        },
    );
}

/// Two placed instances with variable bindings of their own: one paint bound without any override flag, and one
/// gradient stroke with a bound stop that the instance does override.
fn bound_instances() -> Document {
    let mut doc = Document::new("Bound");
    let page = doc.add_page("Page");
    doc.variable_collections.insert(
        COLLECTION.into(),
        VariableCollection {
            id: COLLECTION.into(),
            name: "Tokens".into(),
            modes: vec![VariableMode { id: "3:10".into(), name: "Light".into() }],
            default_mode: "3:10".into(),
            variables: vec![],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    color_variable(&mut doc, STOP_VAR, "stroke/start", Color::rgb(0xAA, 0x00, 0xFF));
    color_variable(&mut doc, FILL_VAR, "fill/brand", Color::rgb(0x10, 0x20, 0x30));

    let main = doc.alloc_id();
    let mut comp = Node::new(main, NodeKind::Component(FrameProps::default()));
    comp.name = "Badge".into();
    comp.size = Vec2::new(44.0, 22.0);
    doc.insert(page, comp).expect("component");

    let mut place = |name: &str, edit: &dyn Fn(&mut Node)| {
        let id = doc.alloc_id();
        let mut n = Node::new(id, NodeKind::Instance { frame: FrameProps::default(), component: Some(main), component_key: None });
        n.name = name.into();
        n.size = Vec2::new(44.0, 22.0);
        edit(&mut n);
        doc.insert(page, n).expect("instance");
    };
    // Resized, so it has a root entry, but its fill is only bound, not overridden.
    place("Bound fill badge", &|n| {
        n.size = Vec2::new(53.0, 22.0);
        n.fills = vec![Paint::solid(Color::rgb(0x10, 0x20, 0x30))];
        n.variables.bindings.insert("fills/0".into(), FILL_VAR.into());
        n.component.overrides.insert(OverrideField::Size);
    });
    place("Gradient outline badge", &|n| {
        n.stroke.weight = 2.0;
        n.stroke.paints = vec![Paint {
            kind: PaintKind::LinearGradient {
                handles: [Vec2::new(0.0, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.0, 1.0)],
                stops: vec![GradientStop { pos: 0.0, color: Color::rgb(0xAA, 0x00, 0xFF) }, GradientStop { pos: 1.0, color: Color::WHITE }],
            },
            opacity: 1.0,
            visible: true,
            blend: BlendMode::Normal,
        }];
        n.variables.bindings.insert("strokes/0/stops/0".into(), STOP_VAR.into());
        n.component.overrides.insert(OverrideField::Stroke);
    });
    doc
}

#[test]
fn what_the_instance_only_binds_is_not_turned_into_an_override_and_stop_bindings_stay() {
    let mut doc = bound_instances();
    for round in 1..=2 {
        doc = import_fig(&export_fig(&doc).expect("export")).expect("reopen");
        let bound = find(&doc, "Bound fill badge");
        assert_eq!(bound.component.overrides, BTreeSet::from([OverrideField::Size]), "round {round}: a bound paint is not an override");
        assert_eq!(bound.variables.bindings.get("fills/0").map(String::as_str), Some(FILL_VAR), "round {round}");

        let outlined = find(&doc, "Gradient outline badge");
        assert_eq!(outlined.component.overrides, BTreeSet::from([OverrideField::Stroke]), "round {round}");
        assert_eq!(
            outlined.variables.bindings.get("strokes/0/stops/0").map(String::as_str),
            Some(STOP_VAR),
            "round {round}: the bound stop survives the root entry"
        );
    }
}
