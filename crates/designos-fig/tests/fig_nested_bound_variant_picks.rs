//! A pick made by hand on a nested copy whose `Size` is bound to a variable survives a `.fig` save and reopen when it
//! differs from the variant the binding would choose: another property's pick, and a swap to another set that also has
//! a `Size`. Files here are written by Studio and read back by Studio.

use designos_doc::*;
use designos_fig::{export_fig, import_fig};

fn theme_collection() -> VariableCollection {
    VariableCollection {
        id: "theme".into(),
        name: "Theme".into(),
        modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
        default_mode: "light".into(),
        variables: vec!["size".into()],
        key: None,
        source_library_key: None,
        extensions: Default::default(),
    }
}

fn size_variable() -> Variable {
    let s = |v: &str| VariableValue::String(v.into());
    Variable {
        id: "size".into(),
        name: "size".into(),
        collection: "theme".into(),
        kind: VariableKind::String,
        values: [("light".to_string(), s("Small")), ("dark".to_string(), s("Large"))].into_iter().collect(),
        description: String::new(),
        scopes: vec![],
        hidden_from_publishing: false,
        key: None,
        code_syntax: Default::default(),
        extensions: Default::default(),
        mode_extensions: Default::default(),
    }
}

fn node_named(d: &Document, name: &str) -> NodeId {
    d.nodes.values().find(|n| n.name == name).map(|n| n.id).unwrap_or_else(|| panic!("no node {name}"))
}

/// "Set/Variant" the instance shows.
fn shown(d: &Document, id: NodeId) -> String {
    let m = instance_component(d.get(id).unwrap()).unwrap();
    let set = d.get(m).unwrap().parent.and_then(|p| d.get(p)).map(|p| p.name.clone()).unwrap_or_default();
    format!("{set}/{}", d.get(m).unwrap().name)
}

fn copy_in_w(d: &Document) -> NodeId {
    *d.get(node_named(d, "W")).unwrap().children.first().unwrap()
}

/// Switches the Theme collection to the mode named `mode` on the node named `name`; ids are looked up because a reopened
/// file carries its own.
fn set_theme_mode(d: &mut Document, name: &str, mode: &str) {
    let coll = d.variable_collections.values().find(|c| c.name == "Theme").expect("Theme").clone();
    let mode_id = coll.modes.iter().find(|m| m.name.eq_ignore_ascii_case(mode)).expect("mode").id.clone();
    let mut n = d.get(node_named(d, name)).unwrap().clone();
    n.variables.modes.insert(coll.id, mode_id);
    History::default().exec(d, Command::Update { node: Box::new(n) }).unwrap();
}

/// A set `set_name` whose variants are the given names, each a frame with a text `label`.
fn make_set(d: &mut Document, page: NodeId, set_name: &str, names: &[(&str, f32)], y: f32) -> Vec<NodeId> {
    let mut mains = Vec::new();
    for (name, wd) in names {
        let id = d.alloc_id();
        let mut v = Node::frame(id, Rect::new(0.0, y, *wd, 40.0));
        v.name = (*name).into();
        let v = d.insert(page, v).unwrap();
        let id = d.alloc_id();
        let mut t = Node::text(id, Rect::new(4.0, 4.0, 30.0, 20.0), "Label", TextStyle::default());
        t.name = "label".into();
        d.insert(v, t).unwrap();
        create_component(d, v).apply(d).unwrap();
        mains.push(v);
    }
    combine_as_variants(d, &mains).apply(d).unwrap();
    let set = d.get(mains[0]).unwrap().parent.unwrap();
    let mut sn = d.get(set).unwrap().clone();
    sn.name = set_name.into();
    Command::Update { node: Box::new(sn) }.apply(d).unwrap();
    mains
}

/// A wrapper main holding an instance of `base` with `Size` bound to `size`, and `W`, an instance of it in `Host`.
fn wrap(d: &mut Document, page: NodeId, base: NodeId) {
    wrap_bound(d, page, base, &["Size"]);
}

/// [`wrap`] with each property of `bound` bound to the variable of its lower-case name.
fn wrap_bound(d: &mut Document, page: NodeId, base: NodeId, bound: &[&str]) {
    let id = d.alloc_id();
    let mut wf = Node::frame(id, Rect::new(0.0, 600.0, 400.0, 100.0));
    wf.name = "Wrapper".into();
    let wmain = d.insert(page, wf).unwrap();
    create_instance(d, base, wmain, Vec2::new(10.0, 10.0)).apply(d).unwrap();
    let nested = *d.get(wmain).unwrap().children.last().unwrap();
    create_component(d, wmain).apply(d).unwrap();
    for prop in bound {
        bind_prop_variable(d, nested, prop, Some(&prop.to_lowercase())).unwrap().apply(d).unwrap();
    }
    let id = d.alloc_id();
    let mut hf = Node::frame(id, Rect::new(500.0, 0.0, 400.0, 200.0));
    hf.name = "Host".into();
    let host = d.insert(page, hf).unwrap();
    create_instance(d, wmain, host, Vec2::new(10.0, 10.0)).apply(d).unwrap();
    let w = *d.get(host).unwrap().children.last().unwrap();
    let mut wn = d.get(w).unwrap().clone();
    wn.name = "W".into();
    Command::Update { node: Box::new(wn) }.apply(d).unwrap();
}

fn base_document() -> (Document, NodeId) {
    let mut d = Document::new("t");
    let page = d.add_page("P");
    d.variable_collections.insert("theme".into(), theme_collection());
    d.variables.insert("size".into(), size_variable());
    (d, page)
}

#[test]
fn a_pick_on_another_property_than_the_bound_one_survives_a_fig_reopen_and_a_mode_switch() {
    let (mut d, page) = base_document();
    let mains = make_set(
        &mut d,
        page,
        "Button",
        &[("Size=Small, State=Default", 100.0), ("Size=Large, State=Default", 200.0), ("Size=Small, State=Hover", 110.0), ("Size=Large, State=Hover", 210.0)],
        0.0,
    );
    wrap(&mut d, page, mains[0]);
    let copy = copy_in_w(&d);
    let pick = set_nested_instance_prop(&mut d, copy, "State", Some(PropValue::Variant("Hover".into())));
    pick.apply(&mut d).unwrap();
    assert_eq!(shown(&d, copy), "Button/Size=Small, State=Hover");

    let mut reopened = import_fig(&export_fig(&d).expect("export")).expect("reopen");
    let copy = copy_in_w(&reopened);
    assert_eq!(shown(&reopened, copy), "Button/Size=Small, State=Hover", "the reopened copy shows the pick");
    assert!(reopened.get(copy).unwrap().component.overrides.contains(&OverrideField::MainComponent), "and still holds it as a pick");
    set_theme_mode(&mut reopened, "Host", "dark");
    assert_eq!(
        shown(&reopened, copy_in_w(&reopened)),
        "Button/Size=Large, State=Hover",
        "a mode switch keeps the State pick, and the bound Size follows its variable"
    );
}

#[test]
fn a_swap_to_another_set_with_the_same_property_survives_a_fig_reopen_and_a_mode_switch() {
    let (mut d, page) = base_document();
    let button = make_set(&mut d, page, "Button", &[("Size=Small", 100.0), ("Size=Large", 200.0)], 0.0);
    let badge = make_set(&mut d, page, "Badge", &[("Size=Small", 60.0), ("Size=Large", 90.0)], 300.0);
    wrap(&mut d, page, button[0]);
    let copy = copy_in_w(&d);
    swap_instance(&mut d, copy, badge[0]).apply(&mut d).unwrap();
    assert_eq!(shown(&d, copy), "Badge/Size=Small");

    let mut reopened = import_fig(&export_fig(&d).expect("export")).expect("reopen");
    let copy = copy_in_w(&reopened);
    assert_eq!(shown(&reopened, copy), "Badge/Size=Small", "the reopened copy shows the swap");
    assert!(reopened.get(copy).unwrap().component.overrides.contains(&OverrideField::MainComponent), "and still holds it as a pick");
    set_theme_mode(&mut reopened, "Host", "dark");
    assert_eq!(shown(&reopened, copy_in_w(&reopened)), "Badge/Size=Small", "a mode switch does not take the swap back");
}

fn state_variable() -> Variable {
    let s = |v: &str| VariableValue::String(v.into());
    Variable {
        id: "state".into(),
        name: "state".into(),
        values: [("light".to_string(), s("Default")), ("dark".to_string(), s("Hover"))].into_iter().collect(),
        ..size_variable()
    }
}

const BUTTON_VARIANTS: [(&str, f32); 4] =
    [("Size=Small, State=Default", 100.0), ("Size=Large, State=Default", 200.0), ("Size=Small, State=Hover", 110.0), ("Size=Large, State=Hover", 210.0)];

/// A document with a `Button` set, the wrapper whose nested instance binds `Size` and `State`, and `Top`, a top-level
/// instance in `Host` that binds `Size`.
fn picks_document() -> (Document, Vec<NodeId>) {
    let (mut d, page) = base_document();
    d.variables.insert("state".into(), state_variable());
    d.variable_collections.get_mut("theme").unwrap().variables.push("state".into());
    let mains = make_set(&mut d, page, "Button", &BUTTON_VARIANTS, 0.0);
    wrap_bound(&mut d, page, mains[0], &["Size", "State"]);
    let host = node_named(&d, "Host");
    create_instance(&mut d, mains[0], host, Vec2::new(10.0, 120.0)).apply(&mut d).unwrap();
    let top = *d.get(host).unwrap().children.last().unwrap();
    let mut tn = d.get(top).unwrap().clone();
    tn.name = "Top".into();
    Command::Update { node: Box::new(tn) }.apply(&mut d).unwrap();
    bind_prop_variable(&mut d, top, "Size", Some("size")).unwrap().apply(&mut d).unwrap();
    (d, mains)
}

fn picks_of(d: &Document, id: NodeId) -> Vec<String> {
    d.get(id).unwrap().component.variant_picks.iter().cloned().collect()
}

#[test]
fn variant_picks_survive_a_fig_reopen_on_a_top_instance_and_a_nested_copy() {
    let (mut d, _) = picks_document();
    let (copy, top) = (copy_in_w(&d), node_named(&d, "Top"));
    set_nested_instance_prop(&mut d, copy, "Size", Some(PropValue::Variant("Large".into()))).apply(&mut d).unwrap();
    set_instance_prop(&mut d, top, "Size", Some(PropValue::Variant("Large".into()))).apply(&mut d).unwrap();
    assert_eq!((picks_of(&d, copy), picks_of(&d, top)), (vec!["Size".to_string()], vec!["Size".to_string()]));

    let mut reopened = import_fig(&export_fig(&d).expect("export")).expect("reopen");
    let (copy, top) = (copy_in_w(&reopened), node_named(&reopened, "Top"));
    assert_eq!(picks_of(&reopened, copy), ["Size"], "the nested copy keeps its pick");
    assert_eq!(picks_of(&reopened, top), ["Size"], "and so does the top-level instance");
    assert_eq!(shown(&reopened, copy), "Button/Size=Large, State=Default");
    assert_eq!(shown(&reopened, top), "Button/Size=Large, State=Default");
    set_theme_mode(&mut reopened, "Host", "dark");
    assert_eq!(shown(&reopened, copy_in_w(&reopened)), "Button/Size=Large, State=Hover", "Size holds, State follows");
    assert_eq!(shown(&reopened, node_named(&reopened, "Top")), "Button/Size=Large, State=Default", "the top-level pick holds against the variable");
    set_theme_mode(&mut reopened, "Host", "light");
    assert_eq!(shown(&reopened, copy_in_w(&reopened)), "Button/Size=Large, State=Default");
}

#[test]
fn a_fig_written_before_picks_were_recorded_holds_only_the_bound_properties_that_differ_from_their_variable() {
    let (mut d, _) = picks_document();
    let copy = copy_in_w(&d);
    set_nested_instance_prop(&mut d, copy, "Size", Some(PropValue::Variant("Large".into()))).apply(&mut d).unwrap();
    // what a build without picks wrote: the swap is there, the pick record is not
    let mut node = d.get(copy).unwrap().clone();
    node.component.variant_picks.clear();
    Command::Update { node: Box::new(node) }.apply(&mut d).unwrap();

    let mut reopened = import_fig(&export_fig(&d).expect("export")).expect("reopen");
    let copy = copy_in_w(&reopened);
    assert_eq!(shown(&reopened, copy), "Button/Size=Large, State=Default", "the reopened copy shows what the file stored");
    assert_eq!(picks_of(&reopened, copy), ["Size"], "Size differs from the variable's value; State shows its variable's value and keeps following");
    set_theme_mode(&mut reopened, "Host", "dark");
    assert_eq!(shown(&reopened, copy_in_w(&reopened)), "Button/Size=Large, State=Hover");
}

#[test]
fn an_instance_that_binds_no_variant_property_writes_no_variant_picks() {
    let (mut d, page) = base_document();
    let mains = make_set(&mut d, page, "Button", &BUTTON_VARIANTS, 0.0);
    let spec: NodeSpec = serde_json::from_value(serde_json::json!({
        "kind": "instance",
        "component": "Button",
        "variant": { "Size": "Large", "State": "Hover" }
    }))
    .unwrap();
    let inst = d.apply_spec(page, &spec).expect("apply_spec");
    set_instance_prop(&mut d, inst, "Size", Some(PropValue::Variant("Small".into()))).apply(&mut d).unwrap();
    swap_instance(&mut d, inst, mains[2]).apply(&mut d).unwrap();
    assert!(d.get(inst).unwrap().component.variant_picks.is_empty());
    let decoded = designos_fig::decode_fig_to_json(&export_fig(&d).expect("export")).expect("decode");
    assert!(!decoded.to_string().contains("variantPicks"), "no carrier entry for an instance that holds no pick");
    let reopened = import_fig(&export_fig(&d).unwrap()).unwrap();
    assert!(reopened.nodes.values().all(|n| n.component.variant_picks.is_empty()));
}

#[test]
fn a_pick_that_equals_its_variable_at_save_time_is_still_a_pick_after_a_fig_reopen() {
    let (mut d, _) = picks_document();
    let (copy, top) = (copy_in_w(&d), node_named(&d, "Top"));
    set_nested_instance_prop(&mut d, copy, "Size", Some(PropValue::Variant("Large".into()))).apply(&mut d).unwrap();
    set_instance_prop(&mut d, top, "Size", Some(PropValue::Variant("Large".into()))).apply(&mut d).unwrap();
    set_theme_mode(&mut d, "Host", "dark");
    assert_eq!(shown(&d, copy_in_w(&d)), "Button/Size=Large, State=Hover", "the dark variable and the pick agree");

    let mut reopened = import_fig(&export_fig(&d).expect("export")).expect("reopen");
    assert_eq!(picks_of(&reopened, copy_in_w(&reopened)), ["Size"], "only the file's own record can tell this from a swap the binding made");
    assert_eq!(picks_of(&reopened, node_named(&reopened, "Top")), ["Size"]);
    set_theme_mode(&mut reopened, "Host", "light");
    assert_eq!(shown(&reopened, copy_in_w(&reopened)), "Button/Size=Large, State=Default", "the nested pick still holds against light");
    assert_eq!(shown(&reopened, node_named(&reopened, "Top")), "Button/Size=Large, State=Default", "and so does the top-level one");
}

/// How many property assignments and carrier entries the exported node named `name` has.
fn exported_assignments(d: &Document, name: &str) -> (usize, String) {
    let decoded = designos_fig::decode_fig_to_json(&export_fig(d).expect("export")).expect("decode");
    let node = decoded["message"]["nodeChanges"].as_array().unwrap().iter().find(|n| n["name"] == name).expect("node").clone();
    (node["componentPropAssignments"].as_array().map_or(0, Vec::len), node["pluginData"].to_string())
}

#[test]
fn swapping_an_instance_that_binds_only_a_text_property_changes_nothing_in_the_exported_file() {
    let (mut d, mains) = picks_document();
    let top = node_named(&d, "Top");
    bind_prop_variable(&mut d, top, "Size", None).unwrap().apply(&mut d).unwrap();
    let set = d.get(mains[0]).unwrap().parent.unwrap();
    add_prop(&mut d, set, ComponentPropDef { name: "Label".into(), kind: PropKind::Text, default: PropValue::Text("Buy".into()) }).apply(&mut d).unwrap();
    d.variables.insert("label".into(), Variable { id: "label".into(), name: "label".into(), ..size_variable() });
    d.variable_collections.get_mut("theme").unwrap().variables.push("label".into());
    bind_prop_variable(&mut d, top, "Label", Some("label")).unwrap().apply(&mut d).unwrap();
    let values_before = d.get(top).unwrap().component.values.clone();
    let exported_before = exported_assignments(&d, "Top");
    swap_instance(&mut d, top, mains[1]).apply(&mut d).unwrap();
    assert_eq!(shown(&d, top), "Button/Size=Large, State=Default");
    assert_eq!(d.get(top).unwrap().component.values, values_before, "no variant value is stored");
    assert!(picks_of(&d, top).is_empty());
    assert_eq!(exported_assignments(&d, "Top"), exported_before, "the exported property assignments and carrier entries are the ones before the swap");
}
