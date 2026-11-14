//! Component properties bound to variables: boolean, text and variant.

use crate::*;

struct Fx {
    d: Document,
    page: NodeId,
    comp: NodeId,
    icon: NodeId,
    label: NodeId,
    host: NodeId,
    inst: NodeId,
    h: History,
}

fn var(id: &str, kind: VariableKind, light: VariableValue, dark: VariableValue) -> Variable {
    Variable {
        id: id.into(),
        name: id.into(),
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

fn add_theme(d: &mut Document) {
    d.variable_collections.insert(
        "theme".into(),
        VariableCollection {
            id: "theme".into(),
            name: "Theme".into(),
            modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
            default_mode: "light".into(),
            variables: vec!["show".into(), "title".into(), "size".into(), "count".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    let s = |v: &str| VariableValue::String(v.into());
    d.variables.insert("show".into(), var("show", VariableKind::Boolean, VariableValue::Boolean(true), VariableValue::Boolean(false)));
    d.variables.insert("title".into(), var("title", VariableKind::String, s("Pay"), s("Pay later")));
    d.variables.insert("size".into(), var("size", VariableKind::String, s("Small"), s("Large")));
    d.variables.insert("count".into(), var("count", VariableKind::Number, VariableValue::Number(1.0), VariableValue::Number(2.0)));
}

fn text_of(d: &Document, id: NodeId) -> String {
    match &d.get(id).unwrap().kind {
        NodeKind::Text(t) => t.content.clone(),
        _ => panic!("not text"),
    }
}

fn child_named(d: &Document, parent: NodeId, name: &str) -> NodeId {
    *d.get(parent).unwrap().children.iter().find(|c| d.get(**c).unwrap().name == name).unwrap_or_else(|| panic!("no child {name}"))
}

/// A main component with an `icon` and a `label` layer, one instance of it inside `host`, and a theme collection.
fn fx() -> Fx {
    let mut d = Document::new("t");
    let page = d.add_page("P");
    add_theme(&mut d);
    let id = d.alloc_id();
    let comp = d.insert(page, Node::frame(id, Rect::new(0.0, 0.0, 100.0, 40.0))).unwrap();
    let id = d.alloc_id();
    let mut t = Node::text(id, Rect::new(10.0, 10.0, 60.0, 20.0), "Label", TextStyle::default());
    t.name = "label".into();
    let label = d.insert(comp, t).unwrap();
    let id = d.alloc_id();
    let mut ic = Node::ellipse(id, Rect::new(75.0, 10.0, 20.0, 20.0));
    ic.name = "icon".into();
    let icon = d.insert(comp, ic).unwrap();
    create_component(&mut d, comp).apply(&mut d).unwrap();
    for (name, kind, default) in [("Show icon", PropKind::Boolean, PropValue::Bool(true)), ("Label", PropKind::Text, PropValue::Text("Label".into()))] {
        add_prop(&mut d, comp, ComponentPropDef { name: name.into(), kind, default }).apply(&mut d).unwrap();
    }
    bind_prop(&mut d, icon, OverrideField::Visible, Some("Show icon")).apply(&mut d).unwrap();
    bind_prop(&mut d, label, OverrideField::Characters, Some("Label")).apply(&mut d).unwrap();
    let id = d.alloc_id();
    let host = d.insert(page, Node::frame(id, Rect::new(300.0, 0.0, 400.0, 200.0))).unwrap();
    create_instance(&mut d, comp, host, Vec2::new(10.0, 10.0)).apply(&mut d).unwrap();
    let inst = *d.get(host).unwrap().children.last().unwrap();
    Fx { d, page, comp, icon, label, host, inst, h: History::default() }
}

fn set_mode(f: &mut Fx, mode: &str) {
    let mut n = f.d.get(f.host).unwrap().clone();
    n.variables.modes.insert("theme".into(), mode.into());
    f.h.exec(&mut f.d, Command::Update { node: Box::new(n) }).unwrap();
}

fn bind(f: &mut Fx, prop: &str, var: &str) {
    let c = bind_prop_variable(&mut f.d, f.inst, prop, Some(var)).unwrap();
    f.h.exec(&mut f.d, c).unwrap();
}

fn icon_visible(f: &Fx) -> bool {
    f.d.get(child_named(&f.d, f.inst, "icon")).unwrap().visible
}

fn label_text(f: &Fx) -> String {
    text_of(&f.d, child_named(&f.d, f.inst, "label"))
}

#[test]
fn a_boolean_variable_toggles_the_property_under_a_mode_switch() {
    let mut f = fx();
    bind(&mut f, "Show icon", "show");
    assert!(icon_visible(&f), "light mode: the variable is true");
    set_mode(&mut f, "dark");
    assert!(!icon_visible(&f), "dark mode: the variable is false, the layer the property drives hides");
    assert_eq!(f.d.get(f.inst).unwrap().component.values.get("Show icon"), Some(&PropValue::Bool(false)));
    set_mode(&mut f, "light");
    assert!(icon_visible(&f));
}

#[test]
fn a_mode_switch_with_a_bound_property_is_one_undo_step() {
    let mut f = fx();
    bind(&mut f, "Show icon", "show");
    set_mode(&mut f, "dark");
    assert!(!icon_visible(&f));
    f.h.undo(&mut f.d).unwrap();
    assert!(icon_visible(&f), "one undo puts the layer and the mode back together");
    assert_eq!(f.d.get(f.host).unwrap().variables.modes.get("theme"), None);
    assert!(f.d.get(f.inst).unwrap().variables.bindings.contains_key(&prop_binding_key("Show icon")), "the bind step is still there");
    f.h.undo(&mut f.d).unwrap();
    assert!(!f.d.get(f.inst).unwrap().variables.bindings.contains_key(&prop_binding_key("Show icon")), "the second undo takes the bind back");
    f.h.redo(&mut f.d).unwrap();
    f.h.redo(&mut f.d).unwrap();
    assert!(!icon_visible(&f), "redo replays both");
}

#[test]
fn a_text_variable_drives_the_text_property() {
    let mut f = fx();
    bind(&mut f, "Label", "title");
    assert_eq!(label_text(&f), "Pay");
    set_mode(&mut f, "dark");
    assert_eq!(label_text(&f), "Pay later");
    // editing the variable value re-resolves too
    let mut vars = f.d.variables.clone();
    vars.get_mut("title").unwrap().values.insert("dark".into(), VariableValue::String("Later".into()));
    let cols = f.d.variable_collections.clone();
    f.h.exec(&mut f.d, Command::SetVariables { variables: vars, collections: cols }).unwrap();
    assert_eq!(label_text(&f), "Later");
    f.h.undo(&mut f.d).unwrap();
    assert_eq!(label_text(&f), "Pay later");
}

#[test]
fn unbinding_keeps_the_last_applied_value_and_stops_following_modes() {
    let mut f = fx();
    bind(&mut f, "Show icon", "show");
    set_mode(&mut f, "dark");
    let c = bind_prop_variable(&mut f.d, f.inst, "Show icon", None).unwrap();
    f.h.exec(&mut f.d, c).unwrap();
    assert!(f.d.get(f.inst).unwrap().variables.bindings.keys().all(|k| prop_binding_name(k).is_none()));
    assert!(!icon_visible(&f), "the last applied value stays");
    set_mode(&mut f, "light");
    assert!(!icon_visible(&f), "an unbound property no longer follows the mode");
}

#[test]
fn a_binding_must_match_the_property_type() {
    let mut f = fx();
    let e = bind_prop_variable(&mut f.d, f.inst, "Show icon", Some("title")).unwrap_err();
    assert!(matches!(e, PropBindError::KindMismatch { .. }), "{e:?}");
    let e = bind_prop_variable(&mut f.d, f.inst, "Label", Some("count")).unwrap_err();
    assert!(matches!(e, PropBindError::KindMismatch { .. }), "{e:?}");
    let e = bind_prop_variable(&mut f.d, f.inst, "Nope", Some("show")).unwrap_err();
    assert_eq!(e, PropBindError::NoSuchProperty("Nope".into()));
    let e = bind_prop_variable(&mut f.d, f.inst, "Show icon", Some("ghost")).unwrap_err();
    assert_eq!(e, PropBindError::NoSuchVariable("ghost".into()));
    let e = bind_prop_variable(&mut f.d, f.comp, "Show icon", Some("show")).unwrap_err();
    assert_eq!(e, PropBindError::NotAnInstance(f.comp));
    assert!(f.d.get(f.inst).unwrap().variables.bindings.is_empty(), "a refused bind changes nothing");
}

#[test]
fn main_component_edits_keep_the_binding_and_its_value() {
    let mut f = fx();
    bind(&mut f, "Label", "title");
    set_mode(&mut f, "dark");
    let mut n = f.d.get(f.label).unwrap().clone();
    if let NodeKind::Text(t) = &mut n.kind {
        t.content = "Other".into();
    }
    Command::Update { node: Box::new(n) }.apply(&mut f.d).unwrap();
    sync_instances(&mut f.d, f.comp).apply(&mut f.d).unwrap();
    assert_eq!(label_text(&f), "Pay later");
    assert!(f.d.get(f.inst).unwrap().variables.bindings.contains_key(&prop_binding_key("Label")));
    let _ = (f.page, f.icon);
}

#[test]
fn renaming_and_deleting_a_property_moves_and_drops_its_binding() {
    let mut f = fx();
    bind(&mut f, "Show icon", "show");
    rename_prop(&mut f.d, f.comp, "Show icon", "Icon on").apply(&mut f.d).unwrap();
    let b = &f.d.get(f.inst).unwrap().variables.bindings;
    assert_eq!(b.get(&prop_binding_key("Icon on")).map(String::as_str), Some("show"));
    assert!(!b.contains_key(&prop_binding_key("Show icon")));
    set_mode(&mut f, "dark");
    assert!(!icon_visible(&f), "the renamed property keeps following the mode");
    delete_prop(&mut f.d, f.comp, "Icon on").apply(&mut f.d).unwrap();
    assert!(f.d.get(f.inst).unwrap().variables.bindings.is_empty());
}

#[test]
fn rendering_does_not_clone_a_bound_instance() {
    let mut f = fx();
    bind(&mut f, "Show icon", "show");
    let n = f.d.get(f.inst).unwrap();
    assert!(matches!(resolve_node(&f.d, n), std::borrow::Cow::Borrowed(_)), "a property binding needs no per-render substitution");
}

/// A set with `Size=Small` and `Size=Large` variants, and one instance of the small one inside a themed host.
struct VarFx {
    d: Document,
    host: NodeId,
    inst: NodeId,
    small: NodeId,
    large: NodeId,
    h: History,
}

fn variant_fx() -> VarFx {
    let mut d = Document::new("t");
    let page = d.add_page("P");
    add_theme(&mut d);
    let mut mains = Vec::new();
    for (name, w) in [("Size=Small", 100.0), ("Size=Large", 200.0)] {
        let id = d.alloc_id();
        let mut v = Node::frame(id, Rect::new(0.0, 0.0, w, 40.0));
        v.name = name.into();
        let v = d.insert(page, v).unwrap();
        create_component(&mut d, v).apply(&mut d).unwrap();
        mains.push(v);
    }
    combine_as_variants(&mut d, &mains).apply(&mut d).unwrap();
    let id = d.alloc_id();
    let host = d.insert(page, Node::frame(id, Rect::new(300.0, 0.0, 400.0, 200.0))).unwrap();
    create_instance(&mut d, mains[0], host, Vec2::new(10.0, 10.0)).apply(&mut d).unwrap();
    let inst = *d.get(host).unwrap().children.last().unwrap();
    VarFx { d, host, inst, small: mains[0], large: mains[1], h: History::default() }
}

fn instance_main(d: &Document, inst: NodeId) -> NodeId {
    instance_component(d.get(inst).unwrap()).unwrap()
}

#[test]
fn a_string_variable_picks_the_variant_under_a_mode_switch() {
    let mut f = variant_fx();
    let c = bind_prop_variable(&mut f.d, f.inst, "Size", Some("size")).unwrap();
    f.h.exec(&mut f.d, c).unwrap();
    assert_eq!(instance_main(&f.d, f.inst), f.small, "light mode: Small");
    let mut n = f.d.get(f.host).unwrap().clone();
    n.variables.modes.insert("theme".into(), "dark".into());
    f.h.exec(&mut f.d, Command::Update { node: Box::new(n) }).unwrap();
    assert_eq!(instance_main(&f.d, f.inst), f.large, "dark mode: the instance switched to the Large variant");
    assert!(f.d.get(f.inst).unwrap().variables.bindings.contains_key(&prop_binding_key("Size")), "the swap keeps the binding");
    assert_eq!(f.d.get(f.inst).unwrap().size.x, 200.0);
    f.h.undo(&mut f.d).unwrap();
    assert_eq!(instance_main(&f.d, f.inst), f.small, "one undo restores variant and mode together");
}

#[test]
fn a_variant_value_that_is_no_option_keeps_the_last_variant_and_is_reported() {
    let mut f = variant_fx();
    let c = bind_prop_variable(&mut f.d, f.inst, "Size", Some("size")).unwrap();
    f.h.exec(&mut f.d, c).unwrap();
    // the variable's dark value is no option of the Size property
    let mut vars = f.d.variables.clone();
    vars.get_mut("size").unwrap().values.insert("dark".into(), VariableValue::String("Huge".into()));
    let cols = f.d.variable_collections.clone();
    f.h.exec(&mut f.d, Command::SetVariables { variables: vars, collections: cols }).unwrap();
    assert!(prop_binding_problem(&f.d, f.inst, "Size").is_none(), "light still gives Small");
    let mut n = f.d.get(f.host).unwrap().clone();
    n.variables.modes.insert("theme".into(), "dark".into());
    f.h.exec(&mut f.d, Command::Update { node: Box::new(n) }).unwrap();
    assert_eq!(instance_main(&f.d, f.inst), f.small, "no silent change: the last valid variant stays");
    assert!(f.d.get(f.inst).unwrap().variables.bindings.contains_key(&prop_binding_key("Size")), "the binding is kept");
    match prop_binding_problem(&f.d, f.inst, "Size") {
        Some(PropBindingProblem::NotAnOption { value, options }) => {
            assert_eq!(value, "Huge");
            assert_eq!(options, ["Small", "Large"]);
        }
        other => panic!("expected NotAnOption, got {other:?}"),
    }
    assert_eq!(prop_binding_problems(&f.d).len(), 1);
}

#[test]
fn a_property_follows_a_mode_of_an_extended_collection() {
    let mut f = fx();
    let mut s = VariableStore::from_doc(&f.d);
    s.collections.get_mut("theme").unwrap().set_extendable(true);
    let brand = s.extend_collection("theme", "Brand").unwrap();
    let b_dark = s.collections[&brand].modes[1].id.clone();
    assert!(s.set_override(&brand, "title", &b_dark, VariableValue::String("Brand pay".into())));
    f.h.exec(&mut f.d, s.command()).unwrap();
    bind(&mut f, "Label", "title");
    assert_eq!(label_text(&f), "Pay");
    let mut n = f.d.get(f.host).unwrap().clone();
    n.variables.modes.insert(brand.clone(), b_dark);
    f.h.exec(&mut f.d, Command::Update { node: Box::new(n) }).unwrap();
    assert_eq!(label_text(&f), "Brand pay", "the extension's override reaches the property");
    f.h.undo(&mut f.d).unwrap();
    assert_eq!(label_text(&f), "Pay");
}

#[test]
fn the_linter_flags_a_property_that_cannot_follow_its_variable() {
    let mut f = variant_fx();
    let c = bind_prop_variable(&mut f.d, f.inst, "Size", Some("size")).unwrap();
    f.h.exec(&mut f.d, c).unwrap();
    let cfg = lint::LintConfig::from_rule_list(&["component-property-variable".to_string()]).expect("the rule is registered");
    assert!(lint::lint(&f.d, &cfg, &lint::NoLintEnv).findings.is_empty(), "a bound property that follows its variable is clean");
    let mut vars = f.d.variables.clone();
    vars.get_mut("size").unwrap().values.insert("light".into(), VariableValue::String("Huge".into()));
    let cols = f.d.variable_collections.clone();
    f.h.exec(&mut f.d, Command::SetVariables { variables: vars, collections: cols }).unwrap();
    assert!(prop_binding_problem(&f.d, f.inst, "Size").is_some());
    let findings = lint::lint(&f.d, &cfg, &lint::NoLintEnv).findings;
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0].node, f.inst);
    assert!(findings[0].message.contains("Size") && findings[0].message.contains("Huge"), "{}", findings[0].message);
}

fn spec_of(v: serde_json::Value) -> NodeSpec {
    serde_json::from_value(v).expect("a valid spec")
}

#[test]
fn apply_spec_binds_properties_and_export_spec_writes_them_back() {
    let mut f = fx();
    let spec = spec_of(serde_json::json!({
        "kind": "instance", "component": format!("node:{}", f.comp.0),
        "props": {"Show icon": "var:Theme/show", "Label": "var:Theme/title"}
    }));
    let report = f.d.apply_spec_with_libraries(f.page, &spec, &[]).expect("the spec applies");
    let inst = report.root_node_id;
    let n = f.d.get(inst).unwrap();
    assert_eq!(n.variables.bindings.get(&prop_binding_key("Show icon")).map(String::as_str), Some("show"));
    assert_eq!(n.variables.bindings.get(&prop_binding_key("Label")).map(String::as_str), Some("title"));
    assert_eq!(text_of(&f.d, child_named(&f.d, inst, "label")), "Pay", "the bound value is applied");
    assert_eq!(report.bindings_created, 2);

    let exported = f.d.export_spec(inst).expect("export");
    assert_eq!(exported.props.get("Show icon"), Some(&serde_json::json!("var:Theme/show")));
    assert_eq!(exported.props.get("Label"), Some(&serde_json::json!("var:Theme/title")));
    let again = f.d.apply_spec_with_libraries(f.page, &exported, &[]).expect("the exported spec applies");
    assert_eq!(f.d.get(again.root_node_id).unwrap().variables.bindings, f.d.get(inst).unwrap().variables.bindings);
}

#[test]
fn apply_spec_refuses_a_variable_of_the_wrong_type_and_binds_a_variant() {
    let mut f = fx();
    let bad = spec_of(serde_json::json!({"kind": "instance", "component": format!("node:{}", f.comp.0), "props": {"Show icon": "var:Theme/title"}}));
    let before = f.d.nodes.len();
    assert!(f.d.apply_spec_with_libraries(f.page, &bad, &[]).is_err(), "a text variable does not bind a boolean property");
    assert_eq!(f.d.nodes.len(), before, "nothing is left behind");

    let mut v = variant_fx();
    let page = v.d.pages[0];
    let spec = spec_of(serde_json::json!({"kind": "instance", "component": format!("node:{}", v.small.0), "props": {"Size": "var:Theme/size"}}));
    let report = v.d.apply_spec_with_libraries(page, &spec, &[]).expect("a variant property binds");
    assert_eq!(v.d.get(report.root_node_id).unwrap().variables.bindings.get(&prop_binding_key("Size")).map(String::as_str), Some("size"));
    let _ = (v.host, v.inst, v.large, &mut v.h);
}

#[test]
fn a_binding_set_through_a_plain_node_update_is_applied_too() {
    let mut f = fx();
    let mut n = f.d.get(f.inst).unwrap().clone();
    n.variables.bindings.insert(prop_binding_key("Label"), "title".into());
    f.h.exec(&mut f.d, Command::Update { node: Box::new(n) }).unwrap();
    assert_eq!(label_text(&f), "Pay", "the bound value reaches the layer in the same step");
    f.h.undo(&mut f.d).unwrap();
    assert_eq!(label_text(&f), "Label");
}

#[test]
fn rename_prop_moves_bound_property_default_and_undoes() {
    let mut f = fx();
    f.d.get_mut(f.comp).unwrap().component.prop_default_variables.insert("Show icon".into(), "show".into());

    let rename = rename_prop(&mut f.d, f.comp, "Show icon", "Icon");
    f.h.exec(&mut f.d, rename).unwrap();
    assert_eq!(f.d.get(f.comp).unwrap().component.prop_default_variables.get("Icon").map(String::as_str), Some("show"));
    assert!(!f.d.get(f.comp).unwrap().component.prop_default_variables.contains_key("Show icon"));

    f.h.undo(&mut f.d).unwrap();
    assert_eq!(f.d.get(f.comp).unwrap().component.prop_default_variables.get("Show icon").map(String::as_str), Some("show"));
    assert!(!f.d.get(f.comp).unwrap().component.prop_default_variables.contains_key("Icon"));
}

#[test]
fn delete_prop_removes_bound_property_default_and_undoes() {
    let mut f = fx();
    f.d.get_mut(f.comp).unwrap().component.prop_default_variables.insert("Label".into(), "title".into());

    let del = delete_prop(&mut f.d, f.comp, "Label");
    f.h.exec(&mut f.d, del).unwrap();
    assert!(!f.d.get(f.comp).unwrap().component.prop_default_variables.contains_key("Label"));

    f.h.undo(&mut f.d).unwrap();
    assert_eq!(f.d.get(f.comp).unwrap().component.prop_default_variables.get("Label").map(String::as_str), Some("title"));
}

#[test]
fn instances_and_detaches_and_resets_do_not_carry_property_default_variables() {
    let mut f = fx();
    f.d.get_mut(f.comp).unwrap().component.prop_default_variables.insert("Show icon".into(), "show".into());
    f.d.get_mut(f.icon).unwrap().component.prop_default_variables.insert("Leak".into(), "show".into());

    let create = create_instance(&mut f.d, f.comp, f.page, Vec2::new(300.0, 0.0));
    f.h.exec(&mut f.d, create).unwrap();
    let placed = *f.d.get(f.page).unwrap().children.last().unwrap();
    assert!(f.d.get(placed).unwrap().component.prop_default_variables.is_empty(), "instance root must not inherit prop_default_variables");
    for &kid in &f.d.get(placed).unwrap().children {
        assert!(f.d.get(kid).unwrap().component.prop_default_variables.is_empty(), "cloned instance child must not inherit prop_default_variables");
    }

    // Resetting an instance clears any stray prop_default_variables on it.
    f.d.get_mut(placed).unwrap().component.prop_default_variables.insert("Show icon".into(), "show".into());
    let reset = reset_instance(&mut f.d, placed);
    f.h.exec(&mut f.d, reset).unwrap();
    assert!(f.d.get(placed).unwrap().component.prop_default_variables.is_empty(), "reset_instance clears prop_default_variables");

    // Detaching an instance (including a nested instance inside another component) clears prop_default_variables.
    let outer_comp = f.d.alloc_id();
    f.d.insert(f.page, Node::new(outer_comp, NodeKind::Component(FrameProps::default()))).unwrap();
    let nested_create = create_instance(&mut f.d, f.comp, outer_comp, Vec2::ZERO);
    f.h.exec(&mut f.d, nested_create).unwrap();
    let nested_inst = *f.d.get(outer_comp).unwrap().children.last().unwrap();
    f.d.get_mut(nested_inst).unwrap().component.prop_default_variables.insert("Show icon".into(), "show".into());
    let detach_nested = detach_instance(&mut f.d, nested_inst);
    f.h.exec(&mut f.d, detach_nested).unwrap();
    assert!(f.d.get(nested_inst).unwrap().component.prop_default_variables.is_empty(), "nested detach_instance clears prop_default_variables");
}

#[test]
fn slot_migration_removes_bound_default_for_migrated_and_dropped_visibility_props_and_undoes() {
    let mut f = fx();
    let badge_comp = f.d.alloc_id();
    f.d.insert(f.page, Node::new(badge_comp, NodeKind::Component(FrameProps::default()))).unwrap();
    let inner_inst = f.d.alloc_id();
    let mut inner = Node::new(inner_inst, NodeKind::Instance { frame: FrameProps::default(), component: Some(badge_comp), component_key: None });
    inner.name = "Badge".into();
    inner.component.bindings.insert(OverrideField::MainComponent, "Swap".into());
    inner.component.bindings.insert(OverrideField::Visible, "Show icon".into());
    f.d.insert(f.comp, inner).unwrap();
    // Unbind `f.icon` from "Show icon" so "Show icon" is dropped when `inner_inst` migrates to a slot.
    f.d.get_mut(f.icon).unwrap().component.bindings.clear();
    let comp_mut = f.d.get_mut(f.comp).unwrap();
    comp_mut.component.props.push(ComponentPropDef {
        name: "Swap".into(),
        kind: PropKind::InstanceSwap { preferred: vec![badge_comp] },
        default: PropValue::Instance(badge_comp),
    });
    comp_mut.component.prop_default_variables.insert("Swap".into(), "title".into());
    comp_mut.component.prop_default_variables.insert("Show icon".into(), "show".into());

    let migrate = migrate_region_to_slot(&mut f.d, inner_inst).expect("migrate instance layer with dropped visibility prop");
    f.h.exec(&mut f.d, migrate).unwrap();
    assert!(!f.d.get(f.comp).unwrap().component.prop_default_variables.contains_key("Swap"), "migrated slot prop default binding removed");
    assert!(!f.d.get(f.comp).unwrap().component.prop_default_variables.contains_key("Show icon"), "dropped visibility prop default binding removed");

    f.h.undo(&mut f.d).unwrap();
    assert_eq!(f.d.get(f.comp).unwrap().component.prop_default_variables.get("Swap").map(String::as_str), Some("title"));
    assert_eq!(f.d.get(f.comp).unwrap().component.prop_default_variables.get("Show icon").map(String::as_str), Some("show"));
}
