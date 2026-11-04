//! The paints of a shared text style are written to the style node of a `.fig` save as `fillPaints`, so they come
//! back on import. A text style that has no paints writes no such field.
use designos_doc::*;
use designos_fig::container::{self, Compression};
use designos_fig::kiwi::{self, KiwiValue as V, Schema};
use designos_fig::testkit::{en, figma_like_schema, fl, guid, msg, node, solid, st, vec2};
use designos_fig::{decode_fig_to_json, export_fig, import_fig};
use serde_json::Value;

fn style_with(name: &str, fills: Vec<Paint>) -> Document {
    let mut doc = Document::new("TextStyleFills");
    doc.add_page("Page 1");
    doc.styles.insert(name.into(), Style::Text(TextStyle { font_size: 20.0, font_weight: 400, fills, ..Default::default() }));
    doc.style_meta.insert(name.into(), StyleMeta { name: name.into(), property: StyleProperty::Text, ..Default::default() });
    doc
}

/// The exported style node called `name`, as JSON.
fn style_node_json(fig: &[u8], name: &str) -> Value {
    let decoded = decode_fig_to_json(fig).expect("decode the exported fig");
    let changes = decoded["message"]["nodeChanges"].as_array().expect("nodeChanges");
    changes.iter().find(|nc| nc["name"] == name).unwrap_or_else(|| panic!("the style node {name} is exported")).clone()
}

/// The fills of the text style called `name` after an import.
fn text_style_fills(doc: &Document, name: &str) -> Vec<Paint> {
    let (key, _) = doc.style_meta.iter().find(|(_, m)| m.name == name).unwrap_or_else(|| panic!("style {name} is imported"));
    match &doc.styles[key] {
        Style::Text(ts) => ts.fills.clone(),
        other => panic!("{name} is not a text style: {other:?}"),
    }
}

#[test]
fn a_text_style_with_a_solid_fill_round_trips_its_fill_through_fig() {
    let blue = Color::from_hex("#2563EB").unwrap();
    let doc = style_with("Body", vec![Paint::solid(blue)]);
    let out = export_fig(&doc).expect("export_fig");

    let style = style_node_json(&out, "Body");
    let paints = style["fillPaints"].as_array().expect("the text style node carries fillPaints");
    assert_eq!(paints.len(), 1);
    assert_eq!(paints[0]["type"], "SOLID");

    let again = import_fig(&out).expect("re-import the exported fig");
    let fills = text_style_fills(&again, "Body");
    assert_eq!(fills.len(), 1, "the fill comes back with the style");
    assert_eq!(fills[0].kind, PaintKind::Solid(blue), "the colour comes back unchanged");
}

#[test]
fn a_text_style_without_fills_writes_no_fill_paints_field() {
    let out = export_fig(&style_with("Plain", vec![])).expect("export_fig");
    let style = style_node_json(&out, "Plain");
    assert!(style.get("fillPaints").is_none(), "no paints, no field: {style}");
    assert_eq!(style["styleType"], "TEXT");
    assert_eq!(style["fontSize"], 20.0, "the rest of the style is written as before");
}

fn shader_fill() -> V {
    msg(vec![
        ("type", en("CUSTOM")),
        ("opacity", fl(0.5)),
        ("visible", V::Bool(true)),
        ("blendMode", en("SCREEN")),
        ("patternSpacing", vec2(4.0, 8.0)),
        ("customEffectId", msg(vec![("guid", guid(10, 25)), ("assetRef", msg(vec![("key", V::String("k".into())), ("version", V::String("1".into()))]))])),
    ])
}

/// A `.fig` with one TEXT style node whose only paint is `fill`.
fn text_style_fig(fill: V) -> Vec<u8> {
    let style = node(
        (2, 200),
        (0, 2),
        "t",
        "TEXT",
        "Shader/Text",
        vec![
            ("styleType", en("TEXT")),
            ("styleDescription", V::String("a text style".into())),
            ("key", V::String("text-style-key".into())),
            ("size", vec2(1.0, 1.0)),
            ("fontSize", fl(20.0)),
            ("fontName", st(vec![("family", V::String("Inter".into())), ("style", V::String("Regular".into())), ("postscript", V::String(String::new()))])),
            ("fillPaints", V::Array(vec![fill])),
        ],
    );
    let nodes = vec![
        node((0, 0), (0, 0), "!", "DOCUMENT", "Doc", vec![]),
        node((0, 1), (0, 0), "a", "CANVAS", "Page 1", vec![]),
        node((0, 2), (0, 0), "b", "CANVAS", "Internal Only Canvas", vec![("internalOnly", V::Bool(true))]),
        style,
    ];
    let message = msg(vec![("nodeChanges", V::Array(nodes)), ("blobs", V::Array(vec![]))]);
    let schema = figma_like_schema();
    let root = schema.find("Message").unwrap();
    let body = schema.encode_message(root, &message).unwrap();
    container::write_container(&schema.encode(), &body, Compression::Zstd, 101)
}

#[test]
fn a_text_style_whose_fill_is_a_shader_round_trips_it_byte_equal() {
    let fill = shader_fill();
    let doc = import_fig(&text_style_fig(fill.clone())).expect("import_fig");
    let imported = text_style_fills(&doc, "Shader/Text");
    assert!(matches!(imported.as_slice(), [Paint { kind: PaintKind::Shader(_), .. }]), "the shader is imported with the style: {imported:?}");

    let out = export_fig(&doc).expect("export_fig");
    let style = style_node_json(&out, "Shader/Text");
    assert_eq!(style["fillPaints"][0], kiwi::to_json(&fill), "every field of the shader fill is written");

    // the paint message encodes to the same bytes as the one the file came from (a message is a set of tagged fields;
    // the exporter writes a shader's fields in key order, so the order is put back before the bytes are compared)
    let archive = container::open(&out).expect("open the exported fig");
    let (schema_bytes, message_bytes) = container::read_archive_chunks(&archive).expect("read the chunks");
    let schema = Schema::decode(&schema_bytes).expect("decode the schema");
    let message = schema.decode_message(schema.root_message().expect("root message"), &message_bytes).expect("decode the message");
    let written = message
        .get("nodeChanges")
        .and_then(V::as_array)
        .and_then(|all| all.iter().find(|nc| matches!(nc.get("name"), Some(V::String(n)) if n == "Shader/Text")))
        .and_then(|nc| nc.get("fillPaints"))
        .and_then(V::as_array)
        .and_then(|paints| paints.first())
        .expect("the text style node carries the shader fill");
    let (V::Message(want), V::Message(have)) = (&fill, written) else { panic!("both are messages") };
    assert_eq!(want.len(), have.len(), "the same number of fields is written: {have:?}");
    let ordered = V::Message(want.iter().map(|(n, _)| (n.clone(), written.get(n).unwrap_or_else(|| panic!("field {n} is written")).clone())).collect());
    let encode = |v: &V| schema.encode_message(schema.find("Paint").unwrap(), v).expect("encode");
    assert_eq!(encode(&ordered), encode(&fill), "the paint message encodes to the same bytes");

    let again = import_fig(&out).expect("re-import the exported fig");
    assert_eq!(text_style_fills(&again, "Shader/Text"), imported, "the shader comes back with the style");
}

#[test]
fn a_solid_fill_on_a_text_style_of_a_file_comes_back_after_a_round_trip() {
    let doc = import_fig(&text_style_fig(solid(1.0, 0.0, 0.0))).expect("import_fig");
    let before = text_style_fills(&doc, "Shader/Text");
    assert_eq!(before.len(), 1);
    let again = import_fig(&export_fig(&doc).expect("export_fig")).expect("re-import the exported fig");
    assert_eq!(text_style_fills(&again, "Shader/Text"), before);
}

/// `(style name, fills)` of every text style, in name order.
fn text_style_fills_by_name(doc: &Document) -> Vec<(String, Vec<Paint>)> {
    let mut all: Vec<(String, Vec<Paint>)> = doc
        .styles
        .iter()
        .filter_map(|(key, style)| match style {
            Style::Text(ts) => Some((doc.style_name(key).to_string(), ts.fills.clone())),
            _ => None,
        })
        .collect();
    all.sort_by(|a, b| a.0.cmp(&b.0));
    all
}

/// Set `DESIGNOS_FIG_ROUND_TRIP_PATH` to a copy of a large `.fig` to check that an import, an export and an import keep the
/// node count and the fills of every text style. Skipped without the variable.
#[test]
fn a_file_keeps_its_node_count_and_text_style_fill_counts_through_a_round_trip_when_requested() {
    let Ok(path) = std::env::var("DESIGNOS_FIG_ROUND_TRIP_PATH") else { return };
    let bytes = std::fs::read(&path).expect("read DESIGNOS_FIG_ROUND_TRIP_PATH");
    let first = import_fig(&bytes).expect("import the file");
    let before = text_style_fills_by_name(&first);
    let not_default = |all: &[(String, Vec<Paint>)]| all.iter().filter(|(_, f)| *f != vec![Paint::solid(Color::BLACK)]).count();
    println!(
        "round_trip before: nodes={} text_styles={} text_style_fills={} styles_with_other_than_the_default_black_fill={}",
        first.nodes.len(),
        before.len(),
        before.iter().map(|(_, f)| f.len()).sum::<usize>(),
        not_default(&before)
    );

    let out = export_fig(&first).expect("export the document");
    let second = import_fig(&out).expect("import the exported file");
    let after = text_style_fills_by_name(&second);
    println!(
        "round_trip after:  nodes={} text_styles={} text_style_fills={} styles_with_other_than_the_default_black_fill={}",
        second.nodes.len(),
        after.len(),
        after.iter().map(|(_, f)| f.len()).sum::<usize>(),
        not_default(&after)
    );

    assert_eq!(second.nodes.len(), first.nodes.len(), "the node count is kept");
    let counts = |all: &[(String, Vec<Paint>)]| all.iter().map(|(n, f)| (n.clone(), f.len())).collect::<Vec<_>>();
    assert_eq!(counts(&after), counts(&before), "every text style keeps its number of fills");
    assert_eq!(after, before, "every text style keeps its fills");
}
