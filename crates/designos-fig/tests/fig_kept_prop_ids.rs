//! A `.fig` save names a component property by the id Figma gave it, on the layer that defines it and on every layer that
//! refers to it, whichever of the two Studio wrote.
#[path = "common/missing_font_fig.rs"]
mod missing_font_fig;

use designos_doc::{Document, NodeId};
use designos_fig::container;
use designos_fig::kiwi::{KiwiValue as V, Schema};
use designos_fig::testkit::{en, guid, msg, node, vec2};
use designos_fig::{export_fig_with_kept_report, import_fig};
use missing_font_fig::*;

fn records(bytes: &[u8]) -> Vec<V> {
    let archive = container::open(bytes).expect("open the container");
    let (schema_bin, msg_bin) = container::read_archive_chunks(&archive).expect("read the chunks");
    let schema = Schema::decode(&schema_bin).expect("decode the schema");
    let root = schema.root_message().expect("root message");
    let mut message = schema.decode_message(root, &msg_bin).expect("decode the message");
    match message.take_field("nodeChanges") {
        Some(V::Array(a)) => a,
        _ => Vec::new(),
    }
}

fn guid_of(record: &V) -> (u32, u32) {
    let g = record.get("guid").expect("a record has a guid");
    let n = |k: &str| match g.get(k) {
        Some(V::Uint(u)) => *u,
        other => panic!("guid.{k} is {other:?}"),
    };
    (n("sessionID"), n("localID"))
}

fn record_of(all: &[V], id: (u32, u32)) -> &V {
    all.iter().find(|r| guid_of(r) == id).unwrap_or_else(|| panic!("no record {id:?}"))
}

fn by_name(doc: &Document, name: &str) -> NodeId {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}")).id
}

fn pair(g: &V) -> (u32, u32) {
    let n = |k: &str| match g.get(k) {
        Some(V::Uint(u)) => *u,
        other => panic!("{k} is {other:?}"),
    };
    (n("sessionID"), n("localID"))
}

fn text(s: &str) -> V {
    msg(vec![("textValue", msg(vec![("characters", V::String(s.into()))]))])
}

const SET: (u32, u32) = (2, 1);
const SMALL: (u32, u32) = (2, 2);
const LARGE: (u32, u32) = (2, 3);
const MAIN: (u32, u32) = (2, 4);
const MAIN_TEXT: (u32, u32) = (2, 5);
const INSTANCE: (u32, u32) = (2, 6);

/// The ids Figma gave the properties: the set's variant property, the aliases of it on the two variants, and a text property
/// of `Main`.
const SIZE: (u32, u32) = (4, 10);
const SIZE_ON_SMALL: (u32, u32) = (4, 20);
const SIZE_ON_LARGE: (u32, u32) = (4, 21);
const LABEL: (u32, u32) = (4, 30);

/// A component set with a variant property and two variants, a main with a text property that a text of it shows, and an instance
/// of that main that sets the property.
fn nodes() -> Vec<V> {
    let mut nodes = document_and_page();
    let plain = |extra: Vec<(&'static str, V)>| {
        let mut fields = vec![("size", vec2(100.0, 40.0))];
        fields.extend(extra);
        fields
    };
    nodes.push(node(
        SET,
        (0, 1),
        "a",
        "FRAME",
        "Set",
        plain(vec![
            ("isStateGroup", V::Bool(true)),
            (
                "componentPropDefs",
                V::Array(vec![msg(vec![
                    ("id", guid(SIZE.0, SIZE.1)),
                    ("name", V::String("Size".into())),
                    ("type", en("VARIANT")),
                    ("initialValue", text("sm")),
                ])]),
            ),
            (
                "stateGroupPropertyValueOrders",
                V::Array(vec![msg(vec![("property", V::String("Size".into())), ("values", V::Array(vec![V::String("sm".into()), V::String("lg".into())]))])]),
            ),
        ]),
    ));
    let variant = |id: (u32, u32), pos: &str, name: &str, own: (u32, u32), value: &str| {
        node(
            id,
            SET,
            pos,
            "SYMBOL",
            name,
            plain(vec![
                ("variantPropSpecs", V::Array(vec![msg(vec![("propDefId", guid(SIZE.0, SIZE.1)), ("value", V::String(value.into()))])])),
                ("componentPropDefs", V::Array(vec![msg(vec![("id", guid(own.0, own.1)), ("parentPropDefId", guid(SIZE.0, SIZE.1))])])),
            ]),
        )
    };
    nodes.push(variant(SMALL, "a", "Size=sm", SIZE_ON_SMALL, "sm"));
    nodes.push(variant(LARGE, "b", "Size=lg", SIZE_ON_LARGE, "lg"));
    nodes.push(node(
        MAIN,
        (0, 1),
        "b",
        "SYMBOL",
        "Main",
        plain(vec![(
            "componentPropDefs",
            V::Array(vec![msg(vec![
                ("id", guid(LABEL.0, LABEL.1)),
                ("name", V::String("Label".into())),
                ("type", en("TEXT")),
                ("initialValue", text("Hello")),
            ])]),
        )]),
    ));
    nodes.push(glyph_text(
        MAIN_TEXT,
        MAIN,
        "a",
        "Caption",
        FAMILY,
        "\u{e900}",
        &[(0, 0.0)],
        vec![("componentPropRefs", V::Array(vec![msg(vec![("defID", guid(LABEL.0, LABEL.1)), ("componentPropNodeField", en("TEXT_DATA"))])]))],
    ));
    nodes.push(node(
        INSTANCE,
        (0, 1),
        "c",
        "INSTANCE",
        "Use",
        plain(vec![
            ("symbolData", msg(vec![("symbolID", guid(MAIN.0, MAIN.1))])),
            ("componentPropAssignments", V::Array(vec![msg(vec![("defID", guid(LABEL.0, LABEL.1)), ("value", text("Bye"))])])),
        ]),
    ));
    nodes
}

fn file() -> Vec<u8> {
    fig_file(nodes(), vec![icon_blob()])
}

fn ids_of(record: &V, list: &str, key: &str) -> Vec<(u32, u32)> {
    record.get(list).and_then(V::as_array).map(|a| a.iter().filter_map(|e| e.get(key)).map(pair).collect()).unwrap_or_default()
}

/// Every id a save writes for a property: what each layer defines, and what each layer refers to.
#[derive(Debug, Default)]
struct PropIdsWritten {
    defined: Vec<((u32, u32), (u32, u32))>,
    aliases: Vec<((u32, u32), (u32, u32))>,
    assigned: Vec<((u32, u32), (u32, u32))>,
    referred: Vec<((u32, u32), (u32, u32))>,
    specs: Vec<((u32, u32), (u32, u32))>,
}

fn written(all: &[V]) -> PropIdsWritten {
    let mut out = PropIdsWritten::default();
    for r in all {
        let at = guid_of(r);
        for (list, key, into) in [
            ("componentPropDefs", "id", &mut out.defined),
            ("componentPropAssignments", "defID", &mut out.assigned),
            ("componentPropRefs", "defID", &mut out.referred),
            ("variantPropSpecs", "propDefId", &mut out.specs),
        ] {
            let named: Vec<(u32, u32)> = if list == "componentPropDefs" {
                r.get(list)
                    .and_then(V::as_array)
                    .map(|a| a.iter().filter(|d| d.get("name").is_some()).filter_map(|d| d.get(key)).map(pair).collect())
                    .unwrap_or_default()
            } else {
                ids_of(r, list, key)
            };
            into.extend(named.into_iter().map(|g| (at, g)));
        }
        if let Some(a) = r.get("componentPropDefs").and_then(V::as_array) {
            out.aliases.extend(a.iter().filter(|d| d.get("name").is_none()).filter_map(|d| d.get("id")).map(|g| (at, pair(g))));
        }
    }
    out
}

/// Edits every layer a little, so the save writes each as Studio models it and none from the store.
fn edit_everything(doc: &mut Document) {
    let ids: Vec<NodeId> = doc.nodes.keys().copied().collect();
    for id in ids {
        if let Some(n) = doc.get_mut(id) {
            n.opacity = 0.9;
        }
    }
}

fn session_of_file() -> std::collections::BTreeSet<u32> {
    records(&file()).iter().map(|r| guid_of(r).0).chain([SIZE.0, SIZE_ON_SMALL.0, LABEL.0]).collect()
}

#[test]
fn studio_writes_figmas_ids_for_the_properties_of_layers_it_writes_itself() {
    let mut doc = import_fig(&file()).expect("imports");
    edit_everything(&mut doc);
    let (saved, report) = export_fig_with_kept_report(&doc).expect("saves");
    assert_eq!(report.nodes_written, 0, "every layer is Studio's: {report:?}");
    let w = written(&records(&saved));
    assert_eq!(w.defined, vec![(SET, SIZE), (MAIN, LABEL)], "the definitions on the set and on the main");
    assert_eq!(w.referred.iter().map(|r| r.1).collect::<Vec<_>>(), vec![LABEL; w.referred.len()], "every reference names the main's property");
    assert!(w.referred.iter().any(|r| r.0 == MAIN_TEXT), "the text of the main refers to it");
    assert_eq!(w.assigned.iter().find(|a| a.0 == INSTANCE).map(|a| a.1), Some(LABEL), "the instance sets the main's property");
    assert_eq!(w.specs, vec![(SMALL, SIZE), (LARGE, SIZE)], "the variants name the set's property");
}

#[test]
fn a_property_studio_made_takes_an_id_of_a_session_no_figma_id_uses() {
    let mut doc = import_fig(&file()).expect("imports");
    edit_everything(&mut doc);
    let main = by_name(&doc, "Main");
    let instance = by_name(&doc, "Use");
    doc.get_mut(main).unwrap().component.props.push(designos_doc::ComponentPropDef {
        name: "Extra".into(),
        kind: designos_doc::PropKind::Boolean,
        default: designos_doc::PropValue::Bool(true),
    });
    doc.get_mut(instance).unwrap().component.values.insert("Extra".into(), designos_doc::PropValue::Bool(false));
    let (saved, _) = export_fig_with_kept_report(&doc).expect("saves");
    let all = records(&saved);
    let made = record_of(&all, MAIN).get("componentPropDefs").and_then(V::as_array).expect("defs").iter().find_map(|d| match d.get("name") {
        Some(V::String(n)) if n == "Extra" => d.get("id").map(pair),
        _ => None,
    });
    let made = made.expect("the new property is defined");
    assert!(!session_of_file().contains(&made.0), "{made:?} is in a session the file's own ids use");
    let mut h = 2166136261u32;
    for b in "Extra".bytes() {
        h = (h ^ u32::from(b)).wrapping_mul(16777619);
    }
    assert_ne!(made, (4, h.max(1)), "never an id derived from the name in the session Figma's ids use");
    assert!(ids_of(record_of(&all, INSTANCE), "componentPropAssignments", "defID").contains(&made), "the instance's value names the new property");
    assert!(![SIZE, SIZE_ON_SMALL, SIZE_ON_LARGE, LABEL].contains(&made));
}

#[test]
fn a_file_with_no_store_names_each_property_by_one_id_on_both_sides() {
    let mut doc = import_fig(&file()).expect("imports");
    doc.fig_kept = Default::default();
    let (saved, _) = export_fig_with_kept_report(&doc).expect("saves");
    let w = written(&records(&saved));
    let label = w.defined.iter().find(|d| d.0 == MAIN).expect("the main's definition").1;
    let size = w.defined.iter().find(|d| d.0 == SET).expect("the set's definition").1;
    assert!(w.referred.iter().all(|r| r.1 == label));
    assert_eq!(w.assigned.iter().find(|a| a.0 == INSTANCE).map(|a| a.1), Some(label));
    assert!(w.specs.iter().all(|s| s.1 == size));
    assert!(!session_of_file().contains(&label.0) && !session_of_file().contains(&size.0), "Studio's ids, not Figma's, when nothing records them");
}
