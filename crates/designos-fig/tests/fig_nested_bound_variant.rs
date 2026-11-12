//! A nested instance whose variant property is bound to a variable survives a `.fig` save and reopen: the binding is
//! still there, the copy shows the same variant with the same overrides, and it still follows the mode.

use designos_doc::*;
use designos_fig::{export_fig, import_fig};

struct Fx {
    d: Document,
    host: NodeId,
}

fn string_variable(id: &str, light: &str, dark: &str) -> Variable {
    let s = |v: &str| VariableValue::String(v.into());
    Variable {
        id: id.into(),
        name: id.into(),
        collection: "theme".into(),
        kind: VariableKind::String,
        values: [("light".to_string(), s(light)), ("dark".to_string(), s(dark))].into_iter().collect(),
        description: String::new(),
        scopes: vec![],
        hidden_from_publishing: false,
        key: None,
        code_syntax: Default::default(),
        extensions: Default::default(),
        mode_extensions: Default::default(),
    }
}

fn child_named(d: &Document, parent: NodeId, name: &str) -> NodeId {
    *d.get(parent).unwrap().children.iter().find(|c| d.get(**c).unwrap().name == name).unwrap_or_else(|| panic!("no child {name}"))
}

fn node_named(d: &Document, name: &str) -> NodeId {
    d.nodes.values().find(|n| n.name == name).map(|n| n.id).unwrap_or_else(|| panic!("no node {name}"))
}

fn text_of(d: &Document, id: NodeId) -> String {
    match &d.get(id).unwrap().kind {
        NodeKind::Text(t) => t.content.clone(),
        _ => panic!("not text"),
    }
}

/// The nested copy inside `W` and the variant name it shows.
fn copy_and_variant(d: &Document) -> (NodeId, String) {
    let w = node_named(d, "W");
    let copy = *d.get(w).unwrap().children.first().expect("W holds the nested copy");
    let main = instance_component(d.get(copy).unwrap()).expect("the copy is an instance");
    (copy, d.get(main).unwrap().name.clone())
}

/// A `Size` set (Small, Large; each with a text `label`), a wrapper main holding an instance of it with `Size` bound to
/// the string variable `size`, and an instance `W` of the wrapper in `Host`, whose label text is overridden.
fn fixture() -> Fx {
    let mut d = Document::new("t");
    let page = d.add_page("P");
    d.variable_collections.insert(
        "theme".into(),
        VariableCollection {
            id: "theme".into(),
            name: "Theme".into(),
            modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
            default_mode: "light".into(),
            variables: vec!["size".into(), "size2".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    d.variables.insert("size".into(), string_variable("size", "Small", "Large"));
    d.variables.insert("size2".into(), string_variable("size2", "Large", "Small"));
    let mut mains = Vec::new();
    for (name, wd) in [("Size=Small", 100.0), ("Size=Large", 200.0)] {
        let id = d.alloc_id();
        let mut v = Node::frame(id, Rect::new(0.0, 0.0, wd, 40.0));
        v.name = name.into();
        let v = d.insert(page, v).unwrap();
        let id = d.alloc_id();
        let mut t = Node::text(id, Rect::new(10.0, 10.0, 60.0, 20.0), "Label", TextStyle::default());
        t.name = "label".into();
        d.insert(v, t).unwrap();
        create_component(&mut d, v).apply(&mut d).unwrap();
        mains.push(v);
    }
    combine_as_variants(&mut d, &mains).apply(&mut d).unwrap();
    let id = d.alloc_id();
    let mut wrapper = Node::frame(id, Rect::new(0.0, 300.0, 400.0, 100.0));
    wrapper.name = "Wrapper".into();
    let wrapper = d.insert(page, wrapper).unwrap();
    create_instance(&mut d, mains[0], wrapper, Vec2::new(10.0, 10.0)).apply(&mut d).unwrap();
    let nested = *d.get(wrapper).unwrap().children.last().unwrap();
    create_component(&mut d, wrapper).apply(&mut d).unwrap();
    bind_prop_variable(&mut d, nested, "Size", Some("size")).unwrap().apply(&mut d).unwrap();
    let id = d.alloc_id();
    let mut host = Node::frame(id, Rect::new(500.0, 0.0, 400.0, 200.0));
    host.name = "Host".into();
    let host = d.insert(page, host).unwrap();
    create_instance(&mut d, wrapper, host, Vec2::new(10.0, 10.0)).apply(&mut d).unwrap();
    let w = *d.get(host).unwrap().children.last().unwrap();
    let mut wn = d.get(w).unwrap().clone();
    wn.name = "W".into();
    Command::Update { node: Box::new(wn) }.apply(&mut d).unwrap();
    let copy = *d.get(w).unwrap().children.first().unwrap();
    let label = child_named(&d, copy, "label");
    let mut ln = d.get(label).unwrap().clone();
    if let NodeKind::Text(t) = &mut ln.kind {
        t.content = "Custom".into();
    }
    ln.component.overrides.insert(OverrideField::Characters);
    Command::Update { node: Box::new(ln) }.apply(&mut d).unwrap();
    Fx { d, host }
}

/// Switches the collection named Theme to the mode named `mode` ("light" or "dark", case-insensitive) on `host`; a reopened
/// file carries its own ids for both.
fn set_host_mode(d: &mut Document, host: NodeId, mode: &str) {
    let coll = d.variable_collections.values().find(|c| c.name == "Theme").expect("the Theme collection");
    let mode_id = coll.modes.iter().find(|m| m.name.eq_ignore_ascii_case(mode) || m.id == mode).expect("the mode").id.clone();
    let coll_id = coll.id.clone();
    let mut n = d.get(host).unwrap().clone();
    n.variables.modes.insert(coll_id, mode_id);
    History::default().exec(d, Command::Update { node: Box::new(n) }).unwrap();
}

#[test]
fn a_fig_save_and_reopen_keeps_the_nested_binding_the_variant_and_the_overrides() {
    let mut f = fixture();
    let host = f.host;
    set_host_mode(&mut f.d, host, "dark");
    assert_eq!(copy_and_variant(&f.d).1, "Size=Large", "before the save: the dark host gives Large");

    let saved = export_fig(&f.d).expect("export");
    let mut reopened = import_fig(&saved).expect("reopen");
    let (copy, variant) = copy_and_variant(&reopened);
    assert_eq!(variant, "Size=Large", "after the reopen the copy shows the same variant");
    assert!(reopened.get(copy).unwrap().variables.bindings.contains_key(&prop_binding_key("Size")), "the binding is still on the copy");
    let label = child_named(&reopened, copy, "label");
    assert_eq!(text_of(&reopened, label), "Custom", "the text override is kept");

    let host = node_named(&reopened, "Host");
    set_host_mode(&mut reopened, host, "light");
    let (copy, variant) = copy_and_variant(&reopened);
    assert_eq!(variant, "Size=Small", "and the reopened copy still follows the mode");
    let label = child_named(&reopened, copy, "label");
    assert_eq!(text_of(&reopened, label), "Custom", "with its override");
}

fn bound_variable_name(d: &Document, copy: NodeId) -> Option<String> {
    let id = d.get(copy)?.variables.bindings.get(&prop_binding_key("Size"))?;
    d.variables.get(id).map(|v| v.name.clone())
}

#[test]
fn a_fig_save_and_reopen_keeps_a_binding_set_on_the_copy_and_it_still_wins() {
    let mut f = fixture();
    let (copy, _) = copy_and_variant(&f.d);
    let c = bind_prop_variable(&mut f.d, copy, "Size", Some("size2")).expect("bind on the copy");
    History::default().exec(&mut f.d, c).unwrap();
    assert_eq!(copy_and_variant(&f.d).1, "Size=Large", "before the save: the copy's own variable gives Large in light, the main's gives Small");

    let saved = export_fig(&f.d).expect("export");
    let mut reopened = import_fig(&saved).expect("reopen");
    let (copy, variant) = copy_and_variant(&reopened);
    assert_eq!(bound_variable_name(&reopened, copy).as_deref(), Some("size2"), "the copy's own binding is kept");
    assert_eq!(variant, "Size=Large");
    let host = node_named(&reopened, "Host");
    set_host_mode(&mut reopened, host, "dark");
    assert_eq!(copy_and_variant(&reopened).1, "Size=Small", "and it still drives from the copy's variable");
    let wmain = node_named(&reopened, "Wrapper");
    sync_instances(&mut reopened, wmain).apply(&mut reopened).unwrap();
    let (copy, variant) = copy_and_variant(&reopened);
    assert_eq!(bound_variable_name(&reopened, copy).as_deref(), Some("size2"), "a sync of the wrapper main leaves it");
    assert_eq!(variant, "Size=Small");
}

#[test]
fn a_stored_swap_that_differs_from_the_variable_stays_a_pick_after_a_fig_reopen() {
    let mut f = fixture();
    let host = f.host;
    set_host_mode(&mut f.d, host, "dark");
    assert_eq!(copy_and_variant(&f.d).1, "Size=Large", "the dark host's variable gives Large");
    let (copy, _) = copy_and_variant(&f.d);
    let c = set_nested_instance_prop(&mut f.d, copy, "Size", Some(PropValue::Variant("Small".into())));
    History::default().exec(&mut f.d, c).unwrap();
    assert_eq!(copy_and_variant(&f.d).1, "Size=Small", "picked by hand against the variable");

    let saved = export_fig(&f.d).expect("export");
    let mut reopened = import_fig(&saved).expect("reopen");
    let (copy, variant) = copy_and_variant(&reopened);
    assert_eq!(variant, "Size=Small", "the reopened copy shows the pick");
    assert!(reopened.get(copy).unwrap().component.overrides.contains(&OverrideField::MainComponent), "and still holds it as a pick");
    let host = node_named(&reopened, "Host");
    set_host_mode(&mut reopened, host, "light");
    set_host_mode(&mut reopened, host, "dark");
    assert_eq!(copy_and_variant(&reopened).1, "Size=Small", "a mode change does not take the pick back");
}
