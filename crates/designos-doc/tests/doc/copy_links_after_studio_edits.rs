//! The link pass runs on every native load, so it meets documents that Studio itself edited. It has to leave a correct
//! one alone: no link moves, and the loaded document saves back to the same bytes.
//!
//! Fixture: a variant set `Wrapper` (Mode A / B) whose variants each hold a `Card`, a `Card` that holds two instances of
//! the variant set `Button` (Size Small / Large; the second one has its `Size` bound to a variable), and two page copies
//! of the wrapper, `Outer` and `Outer2`. After every edit the document is saved natively and loaded again.
use designos_doc::*;

struct Fx {
    doc: Document,
    history: History,
    page: NodeId,
    icon_b: NodeId,
    card2: NodeId,
}

fn named_frame(doc: &mut Document, page: NodeId, name: &str, w: f32) -> NodeId {
    let id = doc.alloc_id();
    let f = doc.insert(page, Node::frame(id, Rect::new(0.0, 0.0, w, 40.0))).unwrap();
    doc.get_mut(f).unwrap().name = name.into();
    f
}

fn add_instance(doc: &mut Document, comp: NodeId, parent: NodeId, name: &str) -> NodeId {
    create_instance(doc, comp, parent, Vec2::new(0.0, 0.0)).apply(doc).unwrap();
    let inst = *doc.get(parent).unwrap().children.last().unwrap();
    doc.get_mut(inst).unwrap().name = name.into();
    inst
}

fn variable(id: &str, small: &str, large: &str) -> Variable {
    let s = |v: &str| VariableValue::String(v.into());
    Variable {
        id: id.into(),
        name: id.into(),
        collection: "theme".into(),
        kind: VariableKind::String,
        values: [("light".to_string(), s(small)), ("dark".to_string(), s(large))].into_iter().collect(),
        description: String::new(),
        scopes: vec![],
        hidden_from_publishing: false,
        key: None,
        code_syntax: Default::default(),
        extensions: Default::default(),
        mode_extensions: Default::default(),
    }
}

fn fixture() -> Fx {
    let mut doc = Document::new("edited");
    let page = doc.add_page("P");
    doc.variable_collections.insert(
        "theme".into(),
        VariableCollection {
            id: "theme".into(),
            name: "Theme".into(),
            modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
            default_mode: "light".into(),
            variables: vec!["size".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    doc.variables.insert("size".into(), variable("size", "Small", "Large"));
    let icon_a = named_frame(&mut doc, page, "IconA", 16.0);
    create_component(&mut doc, icon_a).apply(&mut doc).unwrap();
    let icon_b = named_frame(&mut doc, page, "IconB", 16.0);
    create_component(&mut doc, icon_b).apply(&mut doc).unwrap();

    let mut buttons = Vec::new();
    for (name, w) in [("Size=Small", 100.0), ("Size=Large", 200.0)] {
        let v = named_frame(&mut doc, page, name, w);
        let id = doc.alloc_id();
        let mut label = Node::text(id, Rect::new(10.0, 10.0, 60.0, 20.0), "Label", TextStyle::default());
        label.name = "label".into();
        doc.insert(v, label).unwrap();
        let id = doc.alloc_id();
        let mut bg = Node::rectangle(id, Rect::new(0.0, 0.0, 20.0, 20.0));
        bg.name = "bg".into();
        doc.insert(v, bg).unwrap();
        add_instance(&mut doc, icon_a, v, "glyph");
        create_component(&mut doc, v).apply(&mut doc).unwrap();
        buttons.push(v);
    }
    combine_as_variants(&mut doc, &buttons).apply(&mut doc).unwrap();

    let mut cards = Vec::new();
    for name in ["Card", "Card2"] {
        let c = named_frame(&mut doc, page, name, 300.0);
        add_instance(&mut doc, buttons[0], c, "button");
        let bound = add_instance(&mut doc, buttons[0], c, "button2");
        create_component(&mut doc, c).apply(&mut doc).unwrap();
        let bind = bind_prop_variable(&mut doc, bound, "Size", Some("size")).expect("bind the second button's size");
        bind.apply(&mut doc).unwrap();
        cards.push(c);
    }

    let mut wrappers = Vec::new();
    for name in ["Mode=A", "Mode=B"] {
        let w = named_frame(&mut doc, page, name, 400.0);
        add_instance(&mut doc, cards[0], w, "card");
        create_component(&mut doc, w).apply(&mut doc).unwrap();
        wrappers.push(w);
    }
    combine_as_variants(&mut doc, &wrappers).apply(&mut doc).unwrap();
    for name in ["Outer", "Outer2"] {
        add_instance(&mut doc, wrappers[0], page, name);
    }
    Fx { doc, history: History::default(), page, icon_b, card2: cards[1] }
}

fn by_name(doc: &Document, root: NodeId, name: &str) -> NodeId {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let n = doc.get(id).unwrap();
        if id != root && n.name == name {
            return id;
        }
        stack.extend(n.children.iter().copied());
    }
    panic!("no layer {name} under {root:?}");
}

fn page_copy(doc: &Document, name: &str) -> NodeId {
    doc.nodes.values().find(|n| n.name == name && matches!(n.kind, NodeKind::Instance { .. })).map(|n| n.id).unwrap_or_else(|| panic!("no instance {name}"))
}

/// Saves natively, loads again, and asserts that the pass moved nothing, in the live document and in the loaded one.
fn assert_correct_document_is_left_alone(doc: &mut Document, step: &str) {
    assert_eq!(relink_copy_layers(doc), 0, "{step}: the pass relinked something in the live document");
    let saved = doc.to_json().unwrap();
    let (loaded, report) = Document::from_json_with_report(&saved).unwrap();
    assert_eq!(report.copy_links_relinked, 0, "{step}: the load relinked something");
    assert_eq!(loaded.to_json().unwrap(), saved, "{step}: the loaded document saves to other bytes");
}

fn shows(doc: &Document, inst: NodeId) -> String {
    let main = instance_component(doc.get(inst).unwrap()).expect("an instance with a main");
    doc.get(main).unwrap().name.clone()
}

fn exec(f: &mut Fx, cmd: Command) {
    f.history.exec(&mut f.doc, cmd).unwrap();
}

#[test]
fn a_document_edited_through_the_commands_loads_with_no_link_moved() {
    let mut f = fixture();
    assert_correct_document_is_left_alone(&mut f.doc, "fixture");

    // switch the variant of a nested instance
    let outer = page_copy(&f.doc, "Outer");
    let button = by_name(&f.doc, outer, "button");
    let c = set_nested_instance_prop(&mut f.doc, button, "Size", Some(PropValue::Variant("Large".into())));
    exec(&mut f, c);
    assert_eq!(shows(&f.doc, button), "Size=Large", "the nested switch took effect");
    assert_correct_document_is_left_alone(&mut f.doc, "nested variant switch");

    // switch the variant of the outer instance
    let c = set_instance_prop(&mut f.doc, outer, "Mode", Some(PropValue::Variant("B".into())));
    exec(&mut f, c);
    assert_eq!(shows(&f.doc, outer), "Mode=B", "the outer switch took effect");
    assert_correct_document_is_left_alone(&mut f.doc, "outer variant switch");

    // text and fill overrides on a nested layer
    let button = by_name(&f.doc, outer, "button");
    let label = by_name(&f.doc, button, "label");
    let mut ln = f.doc.get(label).unwrap().clone();
    if let NodeKind::Text(t) = &mut ln.kind {
        t.content = "Custom".into();
    }
    ln.component.overrides.insert(OverrideField::Characters);
    exec(&mut f, Command::Update { node: Box::new(ln) });
    let bg = by_name(&f.doc, button, "bg");
    let mut bn = f.doc.get(bg).unwrap().clone();
    bn.fills = vec![Paint::solid(Color::rgb(255, 0, 0))];
    bn.component.overrides.insert(OverrideField::Fills);
    exec(&mut f, Command::Update { node: Box::new(bn) });
    let (label, bg) = (by_name(&f.doc, button, "label"), by_name(&f.doc, button, "bg"));
    assert!(matches!(&f.doc.get(label).unwrap().kind, NodeKind::Text(t) if t.content == "Custom"), "the text override is there");
    assert_eq!(f.doc.get(bg).unwrap().fills, vec![Paint::solid(Color::rgb(255, 0, 0))], "the fill override is there");
    assert_correct_document_is_left_alone(&mut f.doc, "text and fill overrides");

    // instance-swap a nested copy to another component
    let card = by_name(&f.doc, outer, "card");
    let c = swap_instance(&mut f.doc, card, f.card2);
    exec(&mut f, c);
    assert_eq!(shows(&f.doc, card), "Card2", "the swap took effect");
    assert_correct_document_is_left_alone(&mut f.doc, "instance swap of a nested copy");
    let glyph = by_name(&f.doc, outer, "glyph");
    let c = swap_instance(&mut f.doc, glyph, f.icon_b);
    exec(&mut f, c);
    assert_eq!(shows(&f.doc, glyph), "IconB", "the inner swap took effect");
    assert_correct_document_is_left_alone(&mut f.doc, "instance swap of an inner copy");

    // pick a bound variant property by hand
    let bound = by_name(&f.doc, outer, "button2");
    let c = set_nested_instance_prop(&mut f.doc, bound, "Size", Some(PropValue::Variant("Large".into())));
    exec(&mut f, c);
    assert_eq!(shows(&f.doc, bound), "Size=Large", "the hand pick took effect");
    assert_correct_document_is_left_alone(&mut f.doc, "hand pick on a bound variant property");

    // detach one nested level of the second page copy
    let outer2 = page_copy(&f.doc, "Outer2");
    let nested = by_name(&f.doc, outer2, "button");
    let c = detach_instance(&mut f.doc, nested);
    exec(&mut f, c);
    assert!(matches!(f.doc.get(nested).unwrap().kind, NodeKind::Frame(_)), "the nested copy is a frame now");
    assert_correct_document_is_left_alone(&mut f.doc, "detach of a nested level");

    // duplicate the outer instance, then undo and redo that step
    let nodes = duplicate_subtree(&mut f.doc, outer);
    let at = f.doc.get(f.page).unwrap().children.len();
    let page = f.page;
    exec(&mut f, Command::Insert { parent: page, index: at, nodes });
    assert_eq!(f.doc.get(page).unwrap().children.len(), at + 1, "the duplicate was inserted");
    assert_correct_document_is_left_alone(&mut f.doc, "duplicate of the outer instance");
    assert!(f.history.undo(&mut f.doc).unwrap());
    assert_eq!(f.doc.get(page).unwrap().children.len(), at, "the undo removed it");
    assert_correct_document_is_left_alone(&mut f.doc, "undo of the duplicate");
    assert!(f.history.redo(&mut f.doc).unwrap());
    assert_eq!(f.doc.get(page).unwrap().children.len(), at + 1, "the redo put it back");
    assert_correct_document_is_left_alone(&mut f.doc, "redo of the duplicate");
}

#[test]
fn one_link_corrupted_by_hand_before_the_save_is_the_only_one_the_load_repairs() {
    let mut f = fixture();
    let outer = page_copy(&f.doc, "Outer");
    let glyph = by_name(&f.doc, outer, "glyph");
    let right = f.doc.get(glyph).unwrap().component.source.expect("linked");
    let innermost = authored_layer(&f.doc, glyph);
    assert_ne!(right, innermost, "fixture: the layer sits two levels below the main it comes from");
    f.doc.get_mut(glyph).unwrap().component.source = Some(innermost);
    let saved = f.doc.to_json().unwrap();

    let (loaded, report) = Document::from_json_with_report(&saved).unwrap();
    assert_eq!(report.copy_links_relinked, 1, "exactly the corrupted link is repaired");
    let glyph = by_name(&loaded, page_copy(&loaded, "Outer"), "glyph");
    assert_eq!(loaded.get(glyph).unwrap().component.source, Some(right), "and it is repaired to the layer one level up");
}
