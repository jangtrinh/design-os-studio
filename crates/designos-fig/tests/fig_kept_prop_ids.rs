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
/// The key Figma gave the text of the main in the overrides of its instances: not the layer's own guid.
const KEY: (u32, u32) = (7, 77);
const SIZE: (u32, u32) = (4, 10);
const SIZE_ON_SMALL: (u32, u32) = (4, 20);
const SIZE_ON_LARGE: (u32, u32) = (4, 21);
const LABEL: (u32, u32) = (4, 30);

/// A component set with a variant property and two variants, a main with a text property that a text of it shows, and an instance
/// of that main that sets the property.
/// A consumption entry that binds the opacity of a layer to the component property `id`.
fn bound_to(id: (u32, u32)) -> V {
    msg(vec![
        ("variableField", en("OPACITY")),
        ("variableData", msg(vec![("value", msg(vec![("propRefValue", msg(vec![("defId", guid(id.0, id.1))]))])), ("dataType", en("FLOAT"))])),
    ])
}

fn nodes() -> Vec<V> {
    nodes_with_label(LABEL)
}

fn nodes_with_label(label: (u32, u32)) -> Vec<V> {
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
                ("id", guid(label.0, label.1)),
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
        vec![
            ("componentPropRefs", V::Array(vec![msg(vec![("defID", guid(label.0, label.1)), ("componentPropNodeField", en("TEXT_DATA"))])])),
            ("overrideKey", guid(KEY.0, KEY.1)),
            ("parameterConsumptionMap", msg(vec![("entries", V::Array(vec![bound_to(label)]))])),
        ],
    ));
    nodes.push(node(
        INSTANCE,
        (0, 1),
        "c",
        "INSTANCE",
        "Use",
        plain(vec![
            ("symbolData", msg(vec![("symbolID", guid(MAIN.0, MAIN.1))])),
            ("componentPropAssignments", V::Array(vec![msg(vec![("defID", guid(label.0, label.1)), ("value", text("Bye"))])])),
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

/// Every reference to a definition the saved records hold no definition of.
fn dangling(all: &[V]) -> Vec<((u32, u32), (u32, u32))> {
    let defined: std::collections::BTreeSet<(u32, u32)> = all.iter().flat_map(|r| ids_of(r, "componentPropDefs", "id")).collect();
    let mut out = Vec::new();
    for r in all {
        let at = guid_of(r);
        for (list, key) in
            [("componentPropAssignments", "defID"), ("componentPropRefs", "defID"), ("variantPropSpecs", "propDefId"), ("componentPropDefs", "parentPropDefId")]
        {
            out.extend(ids_of(r, list, key).into_iter().filter(|g| !defined.contains(g)).map(|g| (at, g)));
        }
    }
    out
}

#[test]
fn a_set_a_main_and_an_instance_saved_with_no_edit_keep_figmas_property_records() {
    let original = file();
    let doc = import_fig(&original).expect("imports");
    let (saved, report) = export_fig_with_kept_report(&doc).expect("saves");
    let (was, now) = (records(&original), records(&saved));
    for (id, list) in [
        (SET, "componentPropDefs"),
        (SMALL, "componentPropDefs"),
        (LARGE, "componentPropDefs"),
        (SMALL, "variantPropSpecs"),
        (LARGE, "variantPropSpecs"),
        (MAIN, "componentPropDefs"),
        (MAIN_TEXT, "componentPropRefs"),
        (INSTANCE, "componentPropAssignments"),
    ] {
        assert_eq!(record_of(&now, id).get(list), record_of(&was, id).get(list), "{id:?} {list} is Figma's");
    }
    assert_eq!(dangling(&now), dangling(&was), "no reference names a definition the file does not hold");
    assert_eq!(report.nodes_edited, 0);
    assert_eq!(report.fields_dropped(), 0, "{report:?}");
}

#[test]
fn an_instance_studio_writes_names_the_ids_of_a_main_written_from_the_store() {
    let original = file();
    let mut doc = import_fig(&original).expect("imports");
    let instance = by_name(&doc, "Use");
    doc.get_mut(instance).unwrap().opacity = 0.5;
    let (saved, report) = export_fig_with_kept_report(&doc).expect("saves");
    let now = records(&saved);
    assert!(report.nodes_edited >= 1);
    assert_eq!(ids_of(record_of(&now, MAIN), "componentPropDefs", "id"), vec![LABEL], "the main is Figma's record");
    assert_eq!(ids_of(record_of(&now, INSTANCE), "componentPropAssignments", "defID"), vec![LABEL], "Studio's assignment names Figma's id");
    assert_eq!(dangling(&now), dangling(&records(&original)));
}

#[test]
fn a_main_studio_writes_leaves_the_kept_instance_and_variants_naming_its_ids() {
    let original = file();
    let mut doc = import_fig(&original).expect("imports");
    for name in ["Main", "Set"] {
        let id = by_name(&doc, name);
        doc.get_mut(id).unwrap().opacity = 0.5;
    }
    let (saved, report) = export_fig_with_kept_report(&doc).expect("saves");
    let now = records(&saved);
    let was = records(&original);
    assert_eq!(ids_of(record_of(&now, MAIN), "componentPropDefs", "id"), vec![LABEL]);
    assert_eq!(ids_of(record_of(&now, SET), "componentPropDefs", "id"), vec![SIZE]);
    assert_eq!(record_of(&now, INSTANCE).get("componentPropAssignments"), record_of(&was, INSTANCE).get("componentPropAssignments"), "the instance is Figma's");
    // the set is edited, so its whole group is Studio's: no alias is written, and the variants name the set's property by its id
    assert!(record_of(&now, SMALL).get("componentPropDefs").is_none(), "an alias is Figma's only inside a whole group");
    assert_eq!(ids_of(record_of(&now, SMALL), "variantPropSpecs", "propDefId"), vec![SIZE]);
    assert_eq!(dangling(&now), dangling(&was));
    assert!(report.fields_dropped_by_reason.get("group_not_verbatim").copied().unwrap_or(0) >= 1, "{report:?}");
}

#[test]
fn deleting_a_property_drops_the_kept_references_to_it_and_studio_writes_its_own() {
    let mut doc = import_fig(&file()).expect("imports");
    let main = by_name(&doc, "Main");
    doc.get_mut(main).unwrap().component.props.clear();
    let (saved, report) = export_fig_with_kept_report(&doc).expect("saves");
    let now = records(&saved);
    assert!(report.fields_dropped_by_reason.get("prop_def_not_written").copied().unwrap_or(0) >= 1, "the instance's assignment: {report:?}");
    assert!(report.fields_dropped_by_reason.get("group_not_verbatim").copied().unwrap_or(0) >= 1, "the main's group is Studio's: {report:?}");
    assert!(record_of(&now, MAIN).get("componentPropDefs").is_none(), "nothing defines it any more");
    let named_label = |all: &[V]| -> Vec<(u32, u32)> {
        all.iter()
            .flat_map(|r| ids_of(r, "componentPropAssignments", "defID").into_iter().chain(ids_of(r, "componentPropRefs", "defID")))
            .filter(|g| *g == LABEL)
            .collect()
    };
    assert!(named_label(&now).is_empty(), "no kept reference to the deleted property is written");
    let studio_made = ids_of(record_of(&now, INSTANCE), "componentPropAssignments", "defID");
    assert_eq!(studio_made.len(), 1, "Studio writes the instance's value as it models it");
    assert!(!session_of_file().contains(&studio_made[0].0), "{studio_made:?}");
    // the set was not touched
    assert_eq!(ids_of(record_of(&now, SET), "componentPropDefs", "id"), vec![SIZE]);
    assert_eq!(ids_of(record_of(&now, SMALL), "variantPropSpecs", "propDefId"), vec![SIZE]);
}

#[test]
fn deleting_a_set_property_drops_the_variants_alias_and_spec_that_name_it() {
    let mut doc = import_fig(&file()).expect("imports");
    let set = by_name(&doc, "Set");
    doc.get_mut(set).unwrap().component.props.clear();
    let (saved, report) = export_fig_with_kept_report(&doc).expect("saves");
    let now = records(&saved);
    assert!(report.fields_dropped_by_reason.get("group_not_verbatim").copied().unwrap_or(0) >= 2, "{report:?}");
    assert!(ids_of(record_of(&now, SMALL), "componentPropDefs", "parentPropDefId").is_empty(), "no alias names the deleted property");
    assert!(!ids_of(record_of(&now, SMALL), "variantPropSpecs", "propDefId").contains(&SIZE), "no spec names it");
    // the main is another component's: untouched
    assert_eq!(ids_of(record_of(&now, MAIN), "componentPropDefs", "id"), vec![LABEL]);
}

#[test]
fn an_assignment_of_a_property_the_file_never_defined_is_written_as_it_was() {
    // an instance of a main whose definitions live in a library file: the id means something there
    let library_def = (9, 99);
    let mut nodes = nodes();
    for n in &mut nodes {
        if guid_of(n) == INSTANCE {
            let V::Message(fields) = n else { unreachable!() };
            if let Some(slot) = fields.iter_mut().find(|(k, _)| &**k == "componentPropAssignments") {
                slot.1 = V::Array(vec![msg(vec![("defID", guid(library_def.0, library_def.1)), ("value", text("Bye"))])]);
            }
        }
    }
    let original = fig_file(nodes, vec![icon_blob()]);
    let doc = import_fig(&original).expect("imports");
    let (saved, report) = export_fig_with_kept_report(&doc).expect("saves");
    assert_eq!(ids_of(record_of(&records(&saved), INSTANCE), "componentPropAssignments", "defID"), vec![library_def], "{report:?}");
}

#[test]
fn an_id_the_file_gave_a_property_is_never_given_to_a_layer_studio_makes() {
    // the id Studio would hand the first layer it makes (the copy of the main's text inside the instance)
    let probe = import_fig(&fig_file(nodes_with_label((4, 2)), vec![icon_blob()])).expect("imports");
    let made: Vec<(u32, u32)> = records(&export_fig_with_kept_report(&probe).expect("saves").0).iter().map(guid_of).filter(|g| g.0 == 1).collect();
    assert!(!made.is_empty(), "the instance's copy of the text is a layer Studio makes");
    let clash = made.iter().copied().min().unwrap();
    let doc = import_fig(&fig_file(nodes_with_label((1, clash.1)), vec![icon_blob()])).expect("imports");
    let (saved, _) = export_fig_with_kept_report(&doc).expect("saves");
    let all = records(&saved);
    let defs: Vec<(u32, u32)> = all.iter().flat_map(|r| ids_of(r, "componentPropDefs", "id")).collect();
    assert_eq!(defs.iter().filter(|g| **g == (1, clash.1)).count(), 1, "Figma's id is written: {defs:?}");
    assert!(all.iter().all(|r| guid_of(r) != (1, clash.1)), "no layer is written under the id of a property definition");
}

fn override_paths(all: &[V], at: (u32, u32)) -> Vec<Vec<(u32, u32)>> {
    let entries = record_of(all, at).get("symbolData").and_then(|s| s.get("symbolOverrides")).and_then(V::as_array).map(|a| a.to_vec()).unwrap_or_default();
    entries
        .iter()
        .map(|e| e.get("guidPath").and_then(|p| p.get("guids")).and_then(V::as_array).map(|g| g.iter().map(pair).collect()).unwrap_or_default())
        .collect()
}

fn override_key(all: &[V], at: (u32, u32)) -> Option<(u32, u32)> {
    record_of(all, at).get("overrideKey").map(pair)
}

#[test]
fn a_layer_keeps_the_key_figma_gave_it_in_the_overrides_of_its_instances_edited_or_not() {
    let original = file();
    assert_eq!(override_key(&records(&original), MAIN_TEXT), Some(KEY));
    for edit in [false, true] {
        let mut doc = import_fig(&original).expect("imports");
        if edit {
            let id = by_name(&doc, "Caption");
            doc.get_mut(id).unwrap().opacity = 0.5;
        }
        let (saved, report) = export_fig_with_kept_report(&doc).expect("saves");
        assert_eq!(override_key(&records(&saved), MAIN_TEXT), Some(KEY), "edited: {edit}, {report:?}");
    }
}

#[test]
fn an_override_studio_writes_names_the_layer_by_the_key_figma_gave_it() {
    let original = file();
    let mut doc = import_fig(&original).expect("imports");
    let instance = by_name(&doc, "Use");
    let child = doc.get(instance).unwrap().children[0];
    let node = doc.get_mut(child).unwrap();
    node.opacity = 0.3;
    node.component.overrides.insert(designos_doc::OverrideField::Opacity);
    let (saved, _) = export_fig_with_kept_report(&doc).expect("saves");
    let all = records(&saved);
    let paths = override_paths(&all, INSTANCE);
    assert!(paths.iter().any(|p| p.last() == Some(&KEY)), "an entry names the text by its key: {paths:?}");
    assert!(paths.iter().all(|p| p.last() != Some(&MAIN_TEXT)), "none names it by its guid: {paths:?}");
    assert_eq!(override_key(&all, MAIN_TEXT), Some(KEY), "and the layer carries that key");
}

#[test]
fn a_layer_the_file_gave_no_key_is_addressed_by_its_own_guid_in_the_overrides_studio_writes() {
    let none = (u32::MAX, u32::MAX);
    let mut all = nodes();
    for n in &mut all {
        if guid_of(n) == MAIN_TEXT {
            let V::Message(fields) = n else { unreachable!() };
            fields.iter_mut().find(|(k, _)| &**k == "overrideKey").expect("the text has a key").1 = guid(none.0, none.1);
        }
    }
    let mut doc = import_fig(&fig_file(all, vec![icon_blob()])).expect("imports");
    let instance = by_name(&doc, "Use");
    let child = doc.get(instance).unwrap().children[0];
    let node = doc.get_mut(child).unwrap();
    node.opacity = 0.3;
    node.component.overrides.insert(designos_doc::OverrideField::Opacity);
    let (saved, _) = export_fig_with_kept_report(&doc).expect("saves");
    let out = records(&saved);
    let paths = override_paths(&out, INSTANCE);
    assert!(paths.iter().any(|p| p.last() == Some(&MAIN_TEXT)), "the entry names the text by its guid: {paths:?}");
    assert!(paths.iter().all(|p| p.last() != Some(&none)), "no entry names a layer by the null key: {paths:?}");
    assert_eq!(override_key(&out, MAIN_TEXT), Some(none), "the layer keeps the null key the file gave it");
}

#[test]
fn a_property_id_that_two_components_share_is_written_for_both() {
    // Figma keeps a property's id when a component is copied, so two mains define the same id
    let second = (2, 7);
    let mut all = nodes();
    all.push(node(
        second,
        (0, 1),
        "d",
        "SYMBOL",
        "Copy of Main",
        vec![
            ("size", vec2(100.0, 40.0)),
            (
                "componentPropDefs",
                V::Array(vec![msg(vec![
                    ("id", guid(LABEL.0, LABEL.1)),
                    ("name", V::String("Label".into())),
                    ("type", en("TEXT")),
                    ("initialValue", text("Hello")),
                ])]),
            ),
        ],
    ));
    for edit in [false, true] {
        let mut doc = import_fig(&fig_file(all.clone(), vec![icon_blob()])).expect("imports");
        if edit {
            edit_everything(&mut doc);
        }
        let (saved, _) = export_fig_with_kept_report(&doc).expect("saves");
        let out = records(&saved);
        for at in [MAIN, second] {
            assert_eq!(ids_of(record_of(&out, at), "componentPropDefs", "id"), vec![LABEL], "edit: {edit}, record {at:?}");
        }
    }
}

const SET_B: (u32, u32) = (2, 11);
const B_SMALL: (u32, u32) = (2, 12);
const B_LARGE: (u32, u32) = (2, 13);

/// `nodes()` and a second component set that is a Figma copy of the first: it defines the same property id and its variants carry
/// aliases of it.
fn two_sets_sharing_a_property() -> Vec<u8> {
    let mut all = nodes();
    let set = all.iter().find(|n| guid_of(n) == SET).expect("the set").clone();
    let mut b = set.clone();
    let V::Message(fields) = &mut b else { unreachable!() };
    for (k, v) in fields.iter_mut() {
        match &**k {
            "guid" => *v = guid(SET_B.0, SET_B.1),
            "name" => *v = V::String("Set B".into()),
            "parentIndex" => {
                if let V::Struct(f) | V::Message(f) = v {
                    for (pk, pv) in f.iter_mut() {
                        if &**pk == "position" {
                            *pv = V::String("e".into());
                        }
                    }
                }
            }
            _ => {}
        }
    }
    all.push(b);
    for (id, alias, pos, value) in [(B_SMALL, (4, 22), "a", "sm"), (B_LARGE, (4, 23), "b", "lg")] {
        let mut v = all.iter().find(|n| guid_of(n) == if value == "sm" { SMALL } else { LARGE }).expect("a variant").clone();
        let V::Message(fields) = &mut v else { unreachable!() };
        for (k, x) in fields.iter_mut() {
            match &**k {
                "guid" => *x = guid(id.0, id.1),
                "parentIndex" => {
                    if let V::Struct(f) | V::Message(f) = x {
                        for (pk, pv) in f.iter_mut() {
                            match &**pk {
                                "guid" => *pv = guid(SET_B.0, SET_B.1),
                                "position" => *pv = V::String(pos.into()),
                                _ => {}
                            }
                        }
                    }
                }
                "componentPropDefs" => *x = V::Array(vec![msg(vec![("id", guid(alias.0, alias.1)), ("parentPropDefId", guid(SIZE.0, SIZE.1))])]),
                _ => {}
            }
        }
        all.push(v);
    }
    fig_file(all, vec![icon_blob()])
}

/// Whether any property record of `record` holds the guid `id` anywhere.
fn names(record: &V, id: (u32, u32)) -> bool {
    fn has(v: &V, id: (u32, u32)) -> bool {
        match v {
            V::Message(f) | V::Struct(f) => {
                let own = matches!((v.get("sessionID"), v.get("localID")), (Some(V::Uint(s)), Some(V::Uint(l))) if (*s, *l) == id);
                own || f.iter().any(|(_, x)| has(x, id))
            }
            V::Array(a) => a.iter().any(|x| has(x, id)),
            _ => false,
        }
    }
    ["componentPropDefs", "componentPropAssignments", "componentPropRefs", "variantPropSpecs", "parameterConsumptionMap"]
        .iter()
        .any(|k| record.get(k).is_some_and(|v| has(v, id)))
}

fn naming(all: &[V], id: (u32, u32), records: &[(u32, u32)]) -> Vec<(u32, u32)> {
    records.iter().copied().filter(|at| names(record_of(all, *at), id)).collect()
}

fn the_other_set_is_as_it_was(was: &[V], now: &[V]) {
    for at in [SET_B, B_SMALL, B_LARGE] {
        for key in ["componentPropDefs", "variantPropSpecs"] {
            assert_eq!(record_of(now, at).get(key), record_of(was, at).get(key), "{at:?} {key} is Figma's");
        }
    }
}

#[test]
fn deleting_a_property_of_a_set_that_shares_its_id_leaves_no_reference_to_it_under_that_set() {
    let original = two_sets_sharing_a_property();
    let mut doc = import_fig(&original).expect("imports");
    let set = by_name(&doc, "Set");
    designos_doc::delete_prop(&mut doc, set, "Size").apply(&mut doc).expect("deletes");
    let (saved, _) = export_fig_with_kept_report(&doc).expect("saves");
    let (was, now) = (records(&original), records(&saved));
    assert_eq!(naming(&now, SIZE, &[SET, SMALL, LARGE]), Vec::<(u32, u32)>::new(), "the set no longer defines the property");
    for alias in [SIZE_ON_SMALL, SIZE_ON_LARGE] {
        assert_eq!(naming(&now, alias, &[SET, SMALL, LARGE]), Vec::<(u32, u32)>::new(), "nor does its variants' alias");
    }
    the_other_set_is_as_it_was(&was, &now);
}

#[test]
fn renaming_a_property_of_a_set_that_shares_its_id_leaves_no_reference_to_the_old_id_under_that_set() {
    let original = two_sets_sharing_a_property();
    let mut doc = import_fig(&original).expect("imports");
    let set = by_name(&doc, "Set");
    designos_doc::rename_prop(&mut doc, set, "Size", "Dimension").apply(&mut doc).expect("renames");
    let (saved, _) = export_fig_with_kept_report(&doc).expect("saves");
    let (was, now) = (records(&original), records(&saved));
    assert_eq!(naming(&now, SIZE, &[SET, SMALL, LARGE]), Vec::<(u32, u32)>::new(), "the renamed property is not the other set's");
    let defs = ids_of(record_of(&now, SET), "componentPropDefs", "id");
    assert_eq!(defs.len(), 1, "the set still defines its property");
    assert!(!session_of_file().contains(&defs[0].0) || defs[0] != SIZE);
    the_other_set_is_as_it_was(&was, &now);
}

#[test]
fn a_reference_in_a_main_to_a_deleted_property_is_not_written_from_metadata() {
    let original = file();
    let mut doc = import_fig(&original).expect("imports");
    let main = by_name(&doc, "Main");
    designos_doc::delete_prop(&mut doc, main, "Label").apply(&mut doc).expect("deletes");
    let (saved, _) = export_fig_with_kept_report(&doc).expect("saves");
    let now = records(&saved);
    assert_eq!(naming(&now, LABEL, &[MAIN, MAIN_TEXT]), Vec::<(u32, u32)>::new(), "no consumption entry names the deleted property");
}

#[test]
fn a_copy_of_a_main_binds_its_layers_to_its_own_property_and_not_the_originals() {
    let original = file();
    let mut doc = import_fig(&original).expect("imports");
    let main = by_name(&doc, "Main");
    let page = doc.get(main).unwrap().parent.unwrap();
    let nodes = designos_doc::duplicate_subtree(&mut doc, main);
    let copy = nodes[0].id;
    designos_doc::Command::Insert { parent: page, index: 99, nodes }.apply(&mut doc).expect("pastes");
    let (saved, report) = export_fig_with_kept_report(&doc).expect("saves");
    let now = records(&saved);
    assert!(doc.get(copy).and_then(|n| n.source_id.clone()).is_none(), "a copy has no record of its own");
    // the copy's own record is the SYMBOL that is neither the original nor the instance's; its children name it as their parent
    let copy_record = now
        .iter()
        .find(|r| matches!(r.get("type"), Some(V::Enum(t)) if &**t == "SYMBOL") && ![MAIN, SMALL, LARGE].contains(&guid_of(r)) && guid_of(r).0 == 1)
        .expect("the copy is written");
    let copy_guid = guid_of(copy_record);
    let under_copy: Vec<&V> = now.iter().filter(|r| r.get("parentIndex").and_then(|p| p.get("guid")).map(pair) == Some(copy_guid)).collect();
    assert!(report.prop_refs_dropped >= 1, "the copy's text lost its binding, and the loss is counted: {report:?}");
    assert!(under_copy.iter().all(|r| !names(r, LABEL)), "none of the copy's layers binds to the original's property");
    assert!(!names(copy_record, LABEL), "nor does the copy's definition");
}

#[test]
fn a_key_two_layers_of_one_main_would_share_is_kept_for_the_first_only() {
    // Figma gives a copy of a component the keys of the original; a layer moved from the copy into the original would share one
    let (copy_main, copy_text) = ((2, 8), (2, 9));
    let mut all = nodes();
    all.push(node(copy_main, (0, 1), "e", "SYMBOL", "Copy of Main", vec![("size", vec2(100.0, 40.0))]));
    all.push(glyph_text(copy_text, copy_main, "a", "Moved caption", FAMILY, "\u{e900}", &[(0, 0.0)], vec![("overrideKey", guid(KEY.0, KEY.1))]));
    let mut doc = import_fig(&fig_file(all, vec![icon_blob()])).expect("imports");
    let (main, moved) = (by_name(&doc, "Main"), by_name(&doc, "Moved caption"));
    doc.reparent(moved, main, 0).expect("moves");
    let (saved, _) = export_fig_with_kept_report(&doc).expect("saves");
    let out = records(&saved);
    let keys = [override_key(&out, MAIN_TEXT), override_key(&out, copy_text)];
    assert_eq!(keys.iter().filter(|k| **k == Some(KEY)).count(), 1, "one layer of the main keeps the file's key: {keys:?}");
    let other = if keys[0] == Some(KEY) { copy_text } else { MAIN_TEXT };
    let own = if keys[0] == Some(KEY) { keys[1] } else { keys[0] };
    assert_eq!(own, Some(other), "the other is addressed by its own guid");
}

const LOOSE_FRAME: (u32, u32) = (2, 30);
const LOOSE: (u32, u32) = (2, 31);

/// `nodes()` and a variant that sits in a plain frame, with a spec that names the set's property: the kit holds 798 of these.
fn with_a_loose_variant() -> Vec<u8> {
    let mut all = nodes();
    all.push(node(LOOSE_FRAME, (0, 1), "f", "FRAME", "Country", vec![("size", vec2(100.0, 40.0))]));
    all.push(node(
        LOOSE,
        LOOSE_FRAME,
        "a",
        "SYMBOL",
        "Loose",
        vec![
            ("size", vec2(10.0, 10.0)),
            ("variantPropSpecs", V::Array(vec![msg(vec![("propDefId", guid(SIZE.0, SIZE.1)), ("value", V::String("sm".into()))])])),
        ],
    ));
    fig_file(all, vec![icon_blob()])
}

#[test]
fn a_loose_variant_as_the_file_had_it_is_written_as_it_was_and_a_moved_or_edited_one_is_studios() {
    let original = with_a_loose_variant();
    let was = records(&original);
    let doc = import_fig(&original).expect("imports");
    let (saved, report) = export_fig_with_kept_report(&doc).expect("saves");
    assert_eq!(record_of(&records(&saved), LOOSE).get("variantPropSpecs"), record_of(&was, LOOSE).get("variantPropSpecs"), "{report:?}");
    assert_eq!(report.prop_name_unresolved, 0, "{report:?}");
    for edit in ["move", "edit"] {
        let mut doc = import_fig(&original).expect("imports");
        let id = by_name(&doc, "Loose");
        if edit == "move" {
            let page = doc.get(doc.get(id).unwrap().parent.unwrap()).unwrap().parent.unwrap();
            doc.reparent(id, page, 0).expect("moves");
        } else {
            doc.get_mut(id).unwrap().opacity = 0.5;
        }
        let (saved, report) = export_fig_with_kept_report(&doc).expect("saves");
        let ids = ids_of(record_of(&records(&saved), LOOSE), "variantPropSpecs", "propDefId");
        assert!(ids.iter().all(|g| *g != SIZE), "{edit}: Studio does not name another component's property by its name: {ids:?}");
        assert!(report.prop_name_unresolved >= 1, "{edit}: the unresolved name is counted: {report:?}");
    }
}
