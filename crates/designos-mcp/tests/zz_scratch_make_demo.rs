//! Scratch (not committed): writes the demo document used for the property-binding captures.
use designos_doc::*;
use designos_mcp::engine::Engine;

fn variable(id: &str, name: &str, kind: VariableKind, light: VariableValue, dark: VariableValue) -> Variable {
    Variable {
        id: id.into(),
        name: name.into(),
        collection: "theme".into(),
        kind,
        values: [("light".to_string(), light), ("dark".to_string(), dark)].into_iter().collect(),
        description: String::new(),
        scopes: vec![],
        hidden_from_publishing: false,
        key: None,
        code_syntax: Default::default(),
        extensions: Default::default(),
        mode_extensions: Default::default(),
    }
}

#[test]
fn make_demo_doc() {
    let Ok(out) = std::env::var("DEMO_OUT") else { return };
    let bound = std::env::var("DEMO_BOUND").is_ok();
    let mut e = Engine::default();
    let page = e.resolve_default_page().unwrap();
    let d = &mut e.doc;
    d.variable_collections.insert(
        "theme".into(),
        VariableCollection {
            id: "theme".into(),
            name: "Theme".into(),
            modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
            default_mode: "light".into(),
            variables: vec!["show".into(), "title".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    d.variables.insert("show".into(), variable("show", "Flags/Show icon", VariableKind::Boolean, VariableValue::Boolean(true), VariableValue::Boolean(false)));
    let s = |v: &str| VariableValue::String(v.into());
    d.variables.insert("title".into(), variable("title", "Copy/Button label", VariableKind::String, s("Pay now"), s("Pay later")));

    let id = d.alloc_id();
    let mut f = Node::frame(id, Rect::new(0.0, 0.0, 160.0, 48.0));
    f.name = "Button".into();
    f.corner_radii = [10.0; 4];
    f.fills = vec![Paint::solid(Color::rgb(30, 64, 175))];
    let comp = d.insert(page, f).unwrap();
    let id = d.alloc_id();
    let mut t = Node::text(id, Rect::new(16.0, 14.0, 90.0, 20.0), "Label", TextStyle { font_size: 16.0, ..Default::default() });
    t.name = "label".into();
    if let NodeKind::Text(tp) = &mut t.kind {
        tp.style.fills = vec![Paint::solid(Color::WHITE)];
    }
    t.fills = vec![Paint::solid(Color::WHITE)];
    let label = d.insert(comp, t).unwrap();
    let id = d.alloc_id();
    let mut ic = Node::ellipse(id, Rect::new(124.0, 14.0, 20.0, 20.0));
    ic.name = "icon".into();
    ic.fills = vec![Paint::solid(Color::rgb(250, 204, 21))];
    let icon = d.insert(comp, ic).unwrap();
    create_component(d, comp).apply(d).unwrap();
    for (name, kind, default) in [("Show icon", PropKind::Boolean, PropValue::Bool(true)), ("Label", PropKind::Text, PropValue::Text("Label".into()))] {
        add_prop(d, comp, ComponentPropDef { name: name.into(), kind, default }).apply(d).unwrap();
    }
    bind_prop(d, icon, OverrideField::Visible, Some("Show icon")).apply(d).unwrap();
    bind_prop(d, label, OverrideField::Characters, Some("Label")).apply(d).unwrap();
    let id = d.alloc_id();
    let mut host = Node::frame(id, Rect::new(260.0, 0.0, 400.0, 160.0));
    host.name = "Checkout".into();
    host.fills = vec![Paint::solid(Color::rgb(243, 244, 246))];
    let host = d.insert(page, host).unwrap();
    create_instance(d, comp, host, Vec2::new(24.0, 24.0)).apply(d).unwrap();
    let inst = *d.get(host).unwrap().children.last().unwrap();
    d.get_mut(inst).unwrap().name = "Pay button".into();
    if bound {
        bind_prop_variables(d, inst, &[("Show icon", Some("show")), ("Label", Some("title"))]).unwrap().apply(d).unwrap();
    }
    std::fs::write(out, d.to_json().unwrap()).unwrap();
}
