//! Custom shader effects and fills (`CUSTOM` in the kiwi schema) are carried whole through a `.fig`
//! import, a `.designos` save and a `.fig` export. The probes here read only through the public API,
//! so they run against a build that does not know shaders: there the shader is dropped and they fail.
use designos_doc::*;
use designos_fig::container::{self, Compression};
use designos_fig::kiwi::{self, KiwiValue as V, Schema};
use designos_fig::testkit::{en, figma_like_schema, fl, guid, matrix, msg, node, solid, vec2};
use designos_fig::{decode_fig_to_json, export_fig, import_fig};
use serde_json::Value;

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

fn code_component_id(local: u32, key: &str) -> V {
    msg(vec![("guid", guid(10, local)), ("assetRef", msg(vec![("key", V::String(key.into())), ("version", V::String("1".into()))]))])
}

fn prop_assignments() -> V {
    V::Array(vec![
        msg(vec![("defID", guid(1, 2)), ("value", msg(vec![("floatValue", fl(0.75))]))]),
        msg(vec![("defID", guid(1, 3)), ("value", msg(vec![("boolValue", V::Bool(true))]))]),
    ])
}

/// A shader effect: the reference, two property values, and fields of the schema that Studio never models.
fn shader_effect(local: u32, key: &str, visible: bool) -> V {
    msg(vec![
        ("type", en("CUSTOM")),
        ("visible", V::Bool(visible)),
        ("blendMode", en("MULTIPLY")),
        ("radius", fl(12.0)),
        ("count", V::Uint(3)),
        ("startOffset", vec2(2.0, 6.0)),
        ("customEffectId", code_component_id(local, key)),
        ("componentPropAssignments", prop_assignments()),
    ])
}

/// A shader fill: the same reference fields, plus pattern fields that Studio never models.
fn shader_fill(local: u32, key: &str) -> V {
    msg(vec![
        ("type", en("CUSTOM")),
        ("opacity", fl(0.5)),
        ("visible", V::Bool(true)),
        ("blendMode", en("SCREEN")),
        ("sourceNodeId", guid(3, 44)),
        ("patternSpacing", vec2(4.0, 8.0)),
        ("altText", V::String("pattern".into())),
        ("customEffectId", code_component_id(local, key)),
        ("componentPropAssignments", prop_assignments()),
    ])
}

fn fixture() -> (Vec<u8>, [V; 4]) {
    let effect = shader_effect(25, "effect_key", true);
    let hidden = shader_effect(26, "hidden_key", false);
    let fill = shader_fill(27, "fill_key");
    let style_effect = shader_effect(28, "style_key", true);
    let style = node(
        (2, 100),
        (0, 2),
        "s",
        "RECTANGLE",
        "Shader/Style",
        vec![
            ("styleType", en("EFFECT")),
            ("styleDescription", V::String("a shader style".into())),
            ("key", V::String("shader-style-key".into())),
            ("size", vec2(1.0, 1.0)),
            ("effects", V::Array(vec![style_effect.clone()])),
        ],
    );
    let frame = node(
        (1, 2),
        (0, 1),
        "!",
        "FRAME",
        "Shaded",
        vec![
            ("size", vec2(120.0, 80.0)),
            ("transform", matrix(10.0, 20.0)),
            ("fillPaints", V::Array(vec![solid(1.0, 0.0, 0.0), fill.clone(), solid(0.0, 0.0, 1.0)])),
            ("effects", V::Array(vec![effect.clone(), hidden.clone()])),
        ],
    );
    let nodes = vec![
        node((0, 0), (0, 0), "!", "DOCUMENT", "ShaderDoc", vec![]),
        node((0, 1), (0, 0), "a", "CANVAS", "Page 1", vec![]),
        node((0, 2), (0, 0), "b", "CANVAS", "Internal Only Canvas", vec![("internalOnly", V::Bool(true))]),
        style,
        frame,
    ];
    let message = msg(vec![("nodeChanges", V::Array(nodes)), ("blobs", V::Array(vec![]))]);
    let schema = figma_like_schema();
    let root = schema.find("Message").unwrap();
    let body = schema.encode_message(root, &message).unwrap();
    (container::write_container(&schema.encode(), &body, Compression::Zstd, 101), [effect, hidden, fill, style_effect])
}

/// The node change called `name` in the decoded message of a `.fig` file.
fn node_change(fig: &[u8], name: &str) -> Value {
    let decoded = decode_fig_to_json(fig).expect("decode the exported fig");
    let changes = decoded["message"]["nodeChanges"].as_array().expect("nodeChanges");
    changes.iter().find(|nc| nc["name"] == name).unwrap_or_else(|| panic!("{name} is exported")).clone()
}

fn exported(doc: &Document) -> Vec<u8> {
    export_fig(doc).expect("export_fig")
}

/// `got` with its fields in the order of `like`, and no field more or less. A message is a set of tagged fields and a
/// reader takes them in any order; the exporter writes the fields of a shader in key order, so the order is the one thing
/// that may differ from the file the shader came from. Every nested value must still encode to the same bytes.
fn in_order_of(got: &V, like: &V) -> V {
    let (V::Message(want), V::Message(have)) = (like, got) else { panic!("both are messages") };
    assert_eq!(want.len(), have.len(), "the same number of fields is written: {have:?}");
    V::Message(want.iter().map(|(name, _)| (name.clone(), got.get(name).unwrap_or_else(|| panic!("field {name} is written")).clone())).collect())
}

#[test]
fn shader_effect_and_fill_survive_fig_import_export_reimport() {
    let (fig, [effect, hidden, fill, _]) = fixture();
    let doc = import_fig(&fig).expect("import_fig");
    let out = exported(&doc);
    let shaded = node_change(&out, "Shaded");
    assert_eq!(shaded["effects"][0], kiwi::to_json(&effect), "the shader effect keeps every field");
    assert_eq!(shaded["effects"][1], kiwi::to_json(&hidden), "the hidden shader effect stays hidden and whole");
    assert_eq!(shaded["fillPaints"][1], kiwi::to_json(&fill), "the shader fill keeps every field");

    let again = import_fig(&out).expect("re-import the exported fig");
    assert_eq!(find(&again, "Shaded").effects, find(&doc, "Shaded").effects);
    assert_eq!(find(&again, "Shaded").fills, find(&doc, "Shaded").fills);
}

#[test]
fn shader_in_effect_style_round_trips() {
    let (fig, [_, _, _, style_effect]) = fixture();
    let doc = import_fig(&fig).expect("import_fig");
    let Some(Style::Effect(effects)) = doc.styles.get("2:100") else { panic!("the effect style is imported: {:?}", doc.styles.keys().collect::<Vec<_>>()) };
    assert_eq!(effects.len(), 1, "the style keeps its shader effect");
    let out = exported(&doc);
    let style = node_change(&out, "Shader/Style");
    assert_eq!(style["effects"][0], kiwi::to_json(&style_effect));
    let again = import_fig(&out).expect("re-import the exported fig");
    assert_eq!(again.styles.get("2:100"), doc.styles.get("2:100"));
}

#[test]
fn shader_fill_keeps_opacity_visible_blend_and_slot_order() {
    let (fig, _) = fixture();
    let doc = import_fig(&fig).expect("import_fig");
    let again = import_fig(&exported(&doc)).expect("re-import the exported fig");
    let fills = &find(&again, "Shaded").fills;
    assert_eq!(fills.len(), 3, "the shader fill keeps its slot among the solid fills");
    assert!(matches!(fills[0].kind, PaintKind::Solid(c) if c.r == 1.0));
    assert!(matches!(fills[2].kind, PaintKind::Solid(c) if c.b == 1.0));
    let shader = &fills[1];
    assert!((shader.opacity - 0.5).abs() < 1e-6);
    assert!(shader.visible);
    assert_eq!(shader.blend, BlendMode::Screen);
}

#[test]
fn unknown_field_on_a_shader_message_is_kept() {
    // `count`, `startOffset`, `sourceNodeId`, `patternSpacing` and `altText` are schema fields Studio does not model.
    let (fig, [effect, _, fill, _]) = fixture();
    let doc = import_fig(&fig).expect("import_fig");
    let shaded = node_change(&exported(&doc), "Shaded");
    for key in ["count", "startOffset", "radius", "blendMode"] {
        assert_eq!(shaded["effects"][0][key], kiwi::to_json(&effect)[key], "effect field {key} survives");
    }
    for key in ["sourceNodeId", "patternSpacing", "altText"] {
        assert_eq!(shaded["fillPaints"][1][key], kiwi::to_json(&fill)[key], "fill field {key} survives");
    }
}

#[test]
fn shader_message_bytes_are_equal_after_round_trip() {
    let (fig, [effect, _, fill, _]) = fixture();
    let doc = import_fig(&fig).expect("import_fig");
    let out = exported(&doc);

    let archive = container::open(&out).expect("open the exported fig");
    let (schema_bytes, message_bytes) = container::read_archive_chunks(&archive).expect("read the chunks");
    let schema = Schema::decode(&schema_bytes).expect("decode the schema");
    let root = schema.root_message().expect("root message");
    let message = schema.decode_message(root, &message_bytes).expect("decode the message");
    let shaded = message
        .get("nodeChanges")
        .and_then(V::as_array)
        .and_then(|all| all.iter().find(|nc| matches!(nc.get("name"), Some(V::String(n)) if n == "Shaded")))
        .expect("Shaded is exported");

    let encode = |def: &str, v: &V| schema.encode_message(schema.find(def).unwrap(), v).expect("encode");
    let got_effect = &shaded.get("effects").and_then(V::as_array).expect("effects")[0];
    let got_fill = &shaded.get("fillPaints").and_then(V::as_array).expect("fillPaints")[1];
    assert_eq!(encode("Effect", &in_order_of(got_effect, &effect)), encode("Effect", &effect), "the effect message encodes to the same bytes");
    assert_eq!(encode("Paint", &in_order_of(got_fill, &fill)), encode("Paint", &fill), "the fill message encodes to the same bytes");
}

#[test]
fn designos_json_round_trip_keeps_shader() {
    let (fig, [effect, _, fill, _]) = fixture();
    let doc = import_fig(&fig).expect("import_fig");
    let saved = doc.to_json().expect("to_json");
    let loaded = Document::from_json(&saved).expect("from_json");
    let shaded = node_change(&exported(&loaded), "Shaded");
    assert_eq!(shaded["effects"][0], kiwi::to_json(&effect));
    assert_eq!(shaded["fillPaints"][1], kiwi::to_json(&fill));
}
