//! Every definition, field and enum value of an exported `.fig` is Figma's own: same kind, same field id, same type,
//! same enum value.
//!
//! `tests/data/figma-real-schema.txt` is Figma's schema as decoded from real `.fig` files (every definition of it). It is
//! the reference here: the embedded export schema is checked against it field by field, and the message of each export
//! is decoded twice, once with the schema embedded in the file and once with the reference alone, which must agree.

mod common;
use common::{grid_document, rich_document};
use designos_doc::*;
use designos_fig::kiwi::{DefKind, Field, FieldType, KiwiValue as V, Schema};
use designos_fig::testkit::{figma_like_schema, parse_schema_text, sample_fig, upgrade_fig};
use designos_fig::{container, export_fig, import_fig, import_rest_json};

const REAL_SCHEMA: &str = include_str!("data/figma-real-schema.txt");

fn real() -> Schema {
    parse_schema_text(REAL_SCHEMA)
}

fn type_name(schema: &Schema, f: &Field) -> String {
    let base = match f.ty {
        FieldType::Native(n) => format!("{n:?}"),
        FieldType::Def(i) => schema.defs[i].name.clone(),
    };
    if f.is_array {
        format!("{base}[]")
    } else {
        base
    }
}

fn kind_name(k: DefKind) -> &'static str {
    match k {
        DefKind::Enum => "enum",
        DefKind::Struct => "struct",
        DefKind::Message => "message",
    }
}

/// Panics, naming `Def.field`, at the first definition, field or enum value of `ours` that Figma's schema does not have
/// with the same kind, id and type.
fn assert_matches_figma(ours: &Schema, figma: &Schema) {
    for def in &ours.defs {
        let Some(fi) = figma.find(&def.name) else { panic!("{}: not a definition of Figma's schema", def.name) };
        let theirs = &figma.defs[fi];
        assert_eq!(kind_name(def.kind), kind_name(theirs.kind), "{}: kind", def.name);
        if def.kind == DefKind::Struct {
            // A struct is positional on the wire: same fields, same order, same types.
            let list = |s: &Schema, d: &designos_fig::kiwi::Def| d.fields.iter().map(|f| format!("{}:{}", f.name, type_name(s, f))).collect::<Vec<_>>();
            assert_eq!(list(ours, def), list(figma, theirs), "{}: struct fields", def.name);
            continue;
        }
        let mut ids = std::collections::BTreeSet::new();
        for f in &def.fields {
            assert!(ids.insert(f.value), "{}.{}: id {} is used twice", def.name, f.name, f.value);
            let Some(t) = theirs.fields.iter().find(|t| t.name == f.name) else { panic!("{}.{}: not in Figma's {}", def.name, f.name, kind_name(def.kind)) };
            assert_eq!(f.value, t.value, "{}.{}: {} id", def.name, f.name, if def.kind == DefKind::Enum { "enum value" } else { "field" });
            if def.kind == DefKind::Message {
                assert_eq!(type_name(ours, f), type_name(figma, t), "{}.{}: type", def.name, f.name);
            }
        }
    }
}

#[test]
fn the_embedded_schema_is_a_subset_of_figmas_real_schema() {
    assert_matches_figma(&figma_like_schema(), &real());
}

/// Walks a decoded value of definition `def` of `ours`, checking each field it holds against `figma` (the same rule as
/// [`assert_matches_figma`], but only for what the value uses), and the enum variants it holds.
fn walk(ours: &Schema, figma: &Schema, def: usize, v: &V, path: &str) {
    let d = &ours.defs[def];
    match (d.kind, v) {
        (DefKind::Enum, V::Enum(name)) => {
            let ours_v = d.fields.iter().find(|f| f.name == *name).unwrap_or_else(|| panic!("{path}: {name} is not a variant of {}", d.name));
            let fi = figma.find(&d.name).unwrap_or_else(|| panic!("{path}: {} is not in Figma's schema", d.name));
            let theirs =
                figma.defs[fi].fields.iter().find(|f| f.name == *name).unwrap_or_else(|| panic!("{path}: {}.{name} is not a Figma enum value", d.name));
            assert_eq!(ours_v.value, theirs.value, "{path}: {}.{name} value", d.name);
        }
        (DefKind::Enum, V::EnumRaw(x)) => panic!("{path}: {} holds the value {x}, which has no name", d.name),
        (DefKind::Struct | DefKind::Message, V::Struct(fields) | V::Message(fields)) => {
            let fi = figma.find(&d.name).unwrap_or_else(|| panic!("{path}: {} is not in Figma's schema", d.name));
            for (name, value) in fields {
                let here = format!("{path}.{name}");
                let f = d.fields.iter().find(|f| f.name == *name).unwrap_or_else(|| panic!("{here}: not a field of {}", d.name));
                let t = figma.defs[fi].fields.iter().find(|t| t.name == *name).unwrap_or_else(|| panic!("{here}: {}.{name} is not in Figma's schema", d.name));
                if d.kind == DefKind::Message {
                    assert_eq!(f.value, t.value, "{here}: {}.{name} id", d.name);
                }
                assert_eq!(type_name(ours, f), type_name(figma, t), "{here}: {}.{name} type", d.name);
                if let FieldType::Def(child) = f.ty {
                    match value {
                        V::Array(items) => items.iter().enumerate().for_each(|(i, x)| walk(ours, figma, child, x, &format!("{here}[{i}]"))),
                        other => walk(ours, figma, child, other, &here),
                    }
                }
            }
        }
        (_, V::Bytes(_)) => {}
        (kind, other) => panic!("{path}: a {} holds {other:?}", kind_name(kind)),
    }
}

/// The schema chunk and the message chunk of an exported archive.
fn chunks(bytes: &[u8]) -> (Schema, Vec<u8>) {
    let archive = container::open(bytes).expect("open archive");
    let (schema, message) = container::read_archive_chunks(&archive).expect("chunks");
    (Schema::decode(&schema).expect("file schema"), message)
}

/// Decodes `bytes` with the schema in the file and with Figma's alone, checks that both read the same message, and
/// walks the message against Figma's schema.
fn assert_export_is_figmas(bytes: &[u8], label: &str) {
    let (in_file, message) = chunks(bytes);
    let figma = real();
    let with_file = in_file.decode_message(in_file.root_message().expect("root"), &message).unwrap_or_else(|e| panic!("{label}: {e}"));
    let with_figma = figma
        .decode_message(figma.root_message().expect("root"), &message)
        .unwrap_or_else(|e| panic!("{label}: Figma's own schema cannot read the export: {e}"));
    assert_eq!(with_file, with_figma, "{label}: the embedded schema and Figma's read different messages");
    walk(&in_file, &figma, in_file.root_message().unwrap(), &with_file, &format!("{label}: Message"));
}

fn rest_fixtures() -> Vec<(&'static str, Document)> {
    [
        ("rest_file", include_str!("fixtures/rest_file.json")),
        ("figjam_file", include_str!("fixtures/figjam_file.json")),
        ("proto_file", include_str!("fixtures/proto_file.json")),
        ("proto_vars", include_str!("fixtures/proto_vars.json")),
        ("variables_file", include_str!("fixtures/variables_file.json")),
    ]
    .into_iter()
    .map(|(name, json)| (name, import_rest_json(json).unwrap_or_else(|e| panic!("{name}: {e}"))))
    .collect()
}

#[test]
fn every_definition_and_field_an_export_writes_is_figmas() {
    let mut docs: Vec<(&str, Document)> = vec![("rich", rich_document()), ("grids", grid_document())];
    docs.extend(rest_fixtures());
    docs.push(("sample_fig", import_fig(&sample_fig(container::Compression::Zstd, true)).expect("sample")));
    docs.push(("upgrade_fig", import_fig(&upgrade_fig(container::Compression::Deflate)).expect("upgrade")));
    for (label, doc) in &docs {
        assert_export_is_figmas(&export_fig(doc).unwrap_or_else(|e| panic!("{label}: {e}")), label);
    }
}

#[test]
#[should_panic(expected = "NodeChange.fillPaints: field id")]
fn the_walk_stops_at_the_first_field_with_another_id() {
    let mut tampered = figma_like_schema();
    let nc = tampered.find("NodeChange").unwrap();
    let f = tampered.defs[nc].fields.iter_mut().find(|f| &*f.name == "fillPaints").unwrap();
    f.value += 1000;
    assert_matches_figma(&tampered, &real());
}

#[test]
#[should_panic(expected = "Message.nodeChanges")]
fn the_message_walk_stops_at_the_first_field_with_another_id() {
    let mut ours = figma_like_schema();
    let msg = ours.find("Message").unwrap();
    ours.defs[msg].fields.iter_mut().find(|f| &*f.name == "nodeChanges").unwrap().value = 1;
    // Same values as a real export, read against the tampered schema.
    let bytes = export_fig(&rich_document()).unwrap();
    let (in_file, message) = chunks(&bytes);
    let value = in_file.decode_message(in_file.root_message().unwrap(), &message).unwrap();
    walk(&ours, &real(), ours.find("Message").unwrap(), &value, "Message");
}
