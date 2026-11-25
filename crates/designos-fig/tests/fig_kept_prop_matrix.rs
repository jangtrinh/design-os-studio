//! Nine shapes of component property owners, each saved after eight edits, read back by an oracle that looks only at the saved
//! file: no layer may name a property id its owner does not define, none may name a property under a name the file never gave it,
//! and every reference that dangles is counted in the save's report.
#[path = "common/missing_font_fig.rs"]
mod missing_font_fig;
#[path = "common/prop_oracle.rs"]
mod prop_oracle;

use designos_doc::{Command, ComponentPropDef, Document, NodeId, Paint, PropKind, PropValue};
use designos_fig::container;
use designos_fig::kiwi::{KiwiValue as V, Schema};
use designos_fig::testkit::{en, guid, msg, node, vec2};
use designos_fig::{export_fig_with_kept_report, import_fig};
use missing_font_fig::*;
use prop_oracle::{counts, new_violations, Violation};

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

fn gid(r: &V) -> (u32, u32) {
    let g = r.get("guid").expect("guid");
    match (g.get("sessionID"), g.get("localID")) {
        (Some(V::Uint(s)), Some(V::Uint(l))) => (*s, *l),
        _ => panic!("guid"),
    }
}

fn record_of(all: &[V], id: (u32, u32)) -> &V {
    all.iter().find(|r| gid(r) == id).unwrap_or_else(|| panic!("no record {id:?}"))
}

fn text(s: &str) -> V {
    msg(vec![("textValue", msg(vec![("characters", V::String(s.into()))]))])
}

fn by_name(doc: &Document, name: &str) -> NodeId {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}")).id
}

const X: (u32, u32) = (4, 10);
const LABEL: (u32, u32) = (4, 30);
const SIZE_C: (u32, u32) = (4, 50);

fn plain(extra: Vec<(&'static str, V)>) -> Vec<(&'static str, V)> {
    let mut f = vec![("size", vec2(100.0, 40.0))];
    f.extend(extra);
    f
}

fn defs(list: Vec<V>) -> (&'static str, V) {
    ("componentPropDefs", V::Array(list))
}

fn def(id: (u32, u32), name: &str, ty: &str) -> V {
    msg(vec![("id", guid(id.0, id.1)), ("name", V::String(name.into())), ("type", en(ty)), ("initialValue", text("x"))])
}

fn alias(id: (u32, u32), parent: (u32, u32)) -> V {
    msg(vec![("id", guid(id.0, id.1)), ("parentPropDefId", guid(parent.0, parent.1))])
}

fn spec(id: (u32, u32), value: &str) -> (&'static str, V) {
    ("variantPropSpecs", V::Array(vec![msg(vec![("propDefId", guid(id.0, id.1)), ("value", V::String(value.into()))])]))
}

fn assign(id: (u32, u32), value: &str) -> (&'static str, V) {
    ("componentPropAssignments", V::Array(vec![msg(vec![("defID", guid(id.0, id.1)), ("value", text(value))])]))
}

fn instance(id: (u32, u32), parent: (u32, u32), pos: &str, name: &str, main: (u32, u32), extra: Vec<(&'static str, V)>) -> V {
    let mut f = vec![("symbolData", msg(vec![("symbolID", guid(main.0, main.1))]))];
    f.extend(extra);
    node(id, parent, pos, "INSTANCE", name, plain(f))
}

const SET: (u32, u32) = (2, 1);
const V_SM: (u32, u32) = (2, 2);
const V_LG: (u32, u32) = (2, 3);
const MAIN: (u32, u32) = (2, 4);
const CAPTION: (u32, u32) = (2, 5);
const USE: (u32, u32) = (2, 6);
const SET_B: (u32, u32) = (2, 11);
const OUTER: (u32, u32) = (2, 60);

/// Every shape in one file: a set with two variants (one holds an instance of `Main`), a Figma copy of it that shares its property
/// id, a set with a variant inside a frame, a variant loose in a plain frame, a lone main with a text bound to its property, an
/// instance of it, an instance of a variant, and a main that holds an instance of `Main` with an instance of its own.
fn matrix_file() -> Vec<u8> {
    let mut all = document_and_page();
    let set = |id: (u32, u32), pos: &str, name: &str, prop: (u32, u32), pname: &str| {
        node(
            id,
            (0, 1),
            pos,
            "FRAME",
            name,
            plain(vec![
                ("isStateGroup", V::Bool(true)),
                defs(vec![def(prop, pname, "VARIANT")]),
                (
                    "stateGroupPropertyValueOrders",
                    V::Array(vec![msg(vec![("property", V::String(pname.into())), ("values", V::Array(vec![V::String("sm".into())]))])]),
                ),
            ]),
        )
    };
    let variant = |id: (u32, u32), parent: (u32, u32), pos: &str, name: &str, own: (u32, u32), prop: (u32, u32), value: &str| {
        node(id, parent, pos, "SYMBOL", name, plain(vec![spec(prop, value), defs(vec![alias(own, prop)])]))
    };
    all.push(set(SET, "a", "Set", X, "Size"));
    all.push(variant(V_SM, SET, "a", "V sm", (4, 20), X, "sm"));
    all.push(variant(V_LG, SET, "b", "V lg", (4, 21), X, "lg"));
    all.push(set(SET_B, "e", "Set B", X, "Size"));
    all.push(variant((2, 12), SET_B, "a", "B sm", (4, 22), X, "sm"));
    all.push(variant((2, 13), SET_B, "b", "B lg", (4, 23), X, "lg"));
    all.push(set((2, 40), "f", "Set C", SIZE_C, "Size C"));
    all.push(node((2, 41), (2, 40), "a", "FRAME", "Wrap", plain(vec![])));
    all.push(variant((2, 42), (2, 41), "a", "C variant", (4, 51), SIZE_C, "x"));
    all.push(node((2, 30), (0, 1), "g", "FRAME", "Country", plain(vec![])));
    all.push(variant((2, 31), (2, 30), "a", "Loose", (4, 60), X, "sm"));
    all.push(node(MAIN, (0, 1), "b", "SYMBOL", "Main", plain(vec![defs(vec![def(LABEL, "Label", "TEXT")])])));
    all.push(glyph_text(
        CAPTION,
        MAIN,
        "a",
        "Caption",
        FAMILY,
        "\u{e900}",
        &[(0, 0.0)],
        vec![
            ("componentPropRefs", V::Array(vec![msg(vec![("defID", guid(LABEL.0, LABEL.1)), ("componentPropNodeField", en("TEXT_DATA"))])])),
            (
                "parameterConsumptionMap",
                msg(vec![(
                    "entries",
                    V::Array(vec![msg(vec![
                        ("variableField", en("OPACITY")),
                        (
                            "variableData",
                            msg(vec![("value", msg(vec![("propRefValue", msg(vec![("defId", guid(LABEL.0, LABEL.1))]))])), ("dataType", en("FLOAT"))]),
                        ),
                    ])]),
                )]),
            ),
        ],
    ));
    all.push(instance(USE, (0, 1), "c", "Use", MAIN, vec![assign(LABEL, "Bye")]));
    all.push(instance((2, 50), (0, 1), "d", "Use variant", V_SM, vec![assign(X, "sm")]));
    all.push(instance((2, 70), V_SM, "z", "Nested use", MAIN, vec![assign(LABEL, "In")]));
    all.push(node(OUTER, (0, 1), "h", "SYMBOL", "Outer", plain(vec![])));
    all.push(instance((2, 61), OUTER, "a", "Inner use", MAIN, vec![assign(LABEL, "Deep")]));
    all.push(instance((2, 62), (0, 1), "i", "Outer use", OUTER, vec![]));
    fig_file(all, vec![icon_blob()])
}

struct Shape {
    name: &'static str,
    /// The set or main whose property is edited.
    owner: &'static str,
    prop: &'static str,
    /// A layer inside or under the owner that is moved and painted.
    layer: &'static str,
    /// An instance whose value is edited, and that must stay as the file had it when only the owner's group changes.
    instance: Option<&'static str>,
    /// Make a Studio copy of the owner first and edit that.
    copy_first: bool,
}

const SHAPES: [Shape; 9] = [
    Shape { name: "variant_under_set", owner: "Set", prop: "Size", layer: "V lg", instance: Some("Use variant"), copy_first: false },
    Shape { name: "variant_in_frame_in_set", owner: "Set C", prop: "Size C", layer: "C variant", instance: None, copy_first: false },
    Shape { name: "loose_variant_in_frame", owner: "Set", prop: "Size", layer: "Loose", instance: None, copy_first: false },
    Shape { name: "lone_main", owner: "Main", prop: "Label", layer: "Caption", instance: Some("Use"), copy_first: false },
    Shape { name: "instance_of_a_set_variant", owner: "Set", prop: "Size", layer: "V lg", instance: Some("Use variant"), copy_first: false },
    Shape { name: "instance_inside_a_variant", owner: "Set", prop: "Size", layer: "V sm", instance: Some("Nested use"), copy_first: false },
    Shape { name: "instance_in_instance", owner: "Main", prop: "Label", layer: "Caption", instance: Some("Outer use"), copy_first: false },
    Shape { name: "studio_copy_of_an_imported_main", owner: "Main", prop: "Label", layer: "Caption", instance: Some("Use"), copy_first: true },
    Shape { name: "two_sets_sharing_ids", owner: "Set B", prop: "Size", layer: "B lg", instance: None, copy_first: false },
];

const EDITS: [&str; 8] = [
    "none",
    "delete_prop",
    "rename_prop",
    "add_prop",
    "reparent_variant_to_page",
    "reparent_sublayer_from_copy",
    "fill_edit_on_a_variant",
    "value_edit_on_an_instance",
];

fn copy_of(doc: &mut Document, name: &str) -> NodeId {
    let src = by_name(doc, name);
    let page = doc.get(src).unwrap().parent.unwrap();
    let nodes = designos_doc::duplicate_subtree(doc, src);
    let copy = nodes[0].id;
    let kids: Vec<NodeId> = nodes.iter().skip(1).map(|n| n.id).collect();
    Command::Insert { parent: page, index: 99, nodes }.apply(doc).expect("pastes the copy");
    doc.get_mut(copy).unwrap().name = format!("{name} copy");
    for k in kids {
        let n = doc.get_mut(k).unwrap();
        n.name = format!("{} copy", n.name);
    }
    copy
}

fn run(shape: &Shape, edit: &str) -> (Vec<Violation>, Vec<u8>, designos_fig::KeptWriteReport, Vec<V>) {
    let original = matrix_file();
    let mut doc = import_fig(&original).expect("imports");
    let (owner_name, layer_name) = if shape.copy_first {
        copy_of(&mut doc, shape.owner);
        (format!("{} copy", shape.owner), format!("{} copy", shape.layer))
    } else {
        (shape.owner.to_string(), shape.layer.to_string())
    };
    let owner = by_name(&doc, &owner_name);
    let layer = by_name(&doc, &layer_name);
    let page = doc.get(owner).unwrap().parent.unwrap();
    let page = if doc.pages.contains(&page) { page } else { doc.pages[0] };
    match edit {
        "none" => {}
        "delete_prop" => {
            designos_doc::delete_prop(&mut doc, owner, shape.prop).apply(&mut doc).expect("deletes");
        }
        "rename_prop" => {
            designos_doc::rename_prop(&mut doc, owner, shape.prop, &format!("{} renamed", shape.prop)).apply(&mut doc).expect("renames");
        }
        "add_prop" => {
            let def = ComponentPropDef { name: "Extra".into(), kind: PropKind::Boolean, default: PropValue::Bool(true) };
            designos_doc::add_prop(&mut doc, owner, def).apply(&mut doc).expect("adds");
        }
        "reparent_variant_to_page" => doc.reparent(layer, page, 0).expect("moves"),
        "reparent_sublayer_from_copy" => {
            let copy = copy_of(&mut doc, &owner_name);
            let inside = doc.get(copy).unwrap().children.first().copied();
            if let Some(inside) = inside {
                doc.reparent(inside, owner, 0).expect("moves the copy's layer into the original");
            }
        }
        "fill_edit_on_a_variant" => doc.get_mut(layer).unwrap().fills = vec![Paint::solid(designos_doc::Color::rgb(0, 0, 255))],
        "value_edit_on_an_instance" => {
            if let Some(i) = shape.instance {
                let id = by_name(&doc, i);
                doc.get_mut(id).unwrap().component.values.insert("Label".into(), PropValue::Text("edited".into()));
            }
        }
        other => panic!("{other}"),
    }
    let (saved, report) = export_fig_with_kept_report(&doc).expect("saves");
    let (was, now) = (records(&original), records(&saved));
    (new_violations(&now, &was), original, report, now)
}

fn run_shape(index: usize) {
    let shape = &SHAPES[index];
    let mut failures = Vec::new();
    for edit in EDITS {
        let (violations, original, report, now) = run(shape, edit);
        let c = counts(&violations);
        let bad = c.get("CrossOwner").copied().unwrap_or(0) + c.get("WrongName").copied().unwrap_or(0);
        let dangling = c.get("Dangling").copied().unwrap_or(0);
        let counted = report.prop_name_unresolved + report.prop_refs_dropped;
        if bad > 0 || dangling > counted {
            failures.push(format!("{edit}: {violations:?} report {report:?}"));
        }
        let was = records(&original);
        if edit == "none" && !shape.copy_first && (report.fields_dropped() != 0 || report.prop_name_unresolved != 0 || report.prop_refs_dropped != 0) {
            failures.push(format!("{edit}: a save with no edit drops or counts something: {report:?}"));
        }
        // a loose variant is a group of one: editing it leaves nothing else in its group to hand over
        let alone = shape.name == "loose_variant_in_frame" && edit == "fill_edit_on_a_variant";
        if matches!(edit, "delete_prop" | "rename_prop" | "add_prop" | "fill_edit_on_a_variant") && !shape.copy_first && !alone {
            // an edit in a group hands every key of the group's definition side to Studio
            let g = report.fields_dropped_by_reason.get("group_not_verbatim").copied().unwrap_or(0);
            if g == 0 {
                failures.push(format!("{edit}: no key of the edited group was handed to Studio: {report:?}"));
            }
        }
        if edit == "fill_edit_on_a_variant" {
            if let Some(i) = shape.instance {
                // one edit in the main's group must not rewrite the instances of it: their assignments are the file's
                let g = match i {
                    "Use variant" => (2, 50),
                    "Use" => USE,
                    "Nested use" => (2, 70),
                    "Outer use" => (2, 62),
                    _ => unreachable!(),
                };
                if record_of(&now, g).get("componentPropAssignments") != record_of(&was, g).get("componentPropAssignments") {
                    failures.push(format!("{edit}: the assignments of {i} are not the file's"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "shape {}:\n{}", shape.name, failures.join("\n"));
}

#[test]
fn shape_variant_under_set() {
    run_shape(0);
}
#[test]
fn shape_variant_in_frame_in_set() {
    run_shape(1);
}
#[test]
fn shape_loose_variant_in_frame() {
    run_shape(2);
}
#[test]
fn shape_lone_main() {
    run_shape(3);
}
#[test]
fn shape_instance_of_a_set_variant() {
    run_shape(4);
}
#[test]
fn shape_instance_inside_a_variant() {
    run_shape(5);
}
#[test]
fn shape_instance_in_instance() {
    run_shape(6);
}
#[test]
fn shape_studio_copy_of_an_imported_main() {
    run_shape(7);
}
#[test]
fn shape_two_sets_sharing_ids() {
    run_shape(8);
}
