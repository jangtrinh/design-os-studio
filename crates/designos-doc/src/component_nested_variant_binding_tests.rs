//! A nested instance whose variant property is bound to a variable follows the mode where the instance copy sits.
//!
//! Fixture: a variant set `Size` (Small / Large), each holding a text `label`, a rectangle `bg` and an instance
//! `glyph` of an icon component. A wrapper main holds one instance `N` of the set, its `Size` bound to the string
//! variable `size` (Light "Small", Dark "Large"). `W` is an instance of the wrapper inside `host`.

use crate::*;

struct Nx {
    d: Document,
    page: NodeId,
    wmain: NodeId,
    host: NodeId,
    w: NodeId,
    n_main: NodeId,
    small: NodeId,
    large: NodeId,
    icon_a: NodeId,
    icon_b: NodeId,
    h: History,
}

fn theme_variable(id: &str, kind: VariableKind, light: VariableValue, dark: VariableValue) -> Variable {
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
            variables: vec!["size".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    let s = |v: &str| VariableValue::String(v.into());
    d.variables.insert("size".into(), theme_variable("size", VariableKind::String, s("Small"), s("Large")));
}

fn child_named(d: &Document, parent: NodeId, name: &str) -> NodeId {
    *d.get(parent).unwrap().children.iter().find(|c| d.get(**c).unwrap().name == name).unwrap_or_else(|| panic!("no child {name}"))
}

fn first_child(d: &Document, parent: NodeId) -> NodeId {
    *d.get(parent).unwrap().children.first().unwrap()
}

fn main_of(d: &Document, inst: NodeId) -> NodeId {
    instance_component(d.get(inst).unwrap()).unwrap()
}

fn text_of(d: &Document, id: NodeId) -> String {
    match &d.get(id).unwrap().kind {
        NodeKind::Text(t) => t.content.clone(),
        _ => panic!("not text"),
    }
}

fn red() -> Vec<Paint> {
    vec![Paint::solid(Color::rgb(0xFF, 0x00, 0x00))]
}

fn new_icon(d: &mut Document, page: NodeId, name: &str) -> NodeId {
    let id = d.alloc_id();
    let mut n = Node::frame(id, Rect::new(0.0, 100.0, 16.0, 16.0));
    n.name = name.into();
    let n = d.insert(page, n).unwrap();
    create_component(d, n).apply(d).unwrap();
    n
}

fn make_fx() -> Nx {
    make_fx_layers(false)
}

/// With `twins`, each variant holds two instances of the same inner component, `lead` and `trail`, in place of `glyph`.
fn make_fx_layers(twins: bool) -> Nx {
    let mut d = Document::new("t");
    let page = d.add_page("P");
    add_theme(&mut d);
    let icon_a = new_icon(&mut d, page, "IconA");
    let icon_b = new_icon(&mut d, page, "IconB");
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
        let id = d.alloc_id();
        let mut bg = Node::rectangle(id, Rect::new(0.0, 0.0, 20.0, 20.0));
        bg.name = "bg".into();
        d.insert(v, bg).unwrap();
        create_instance(&mut d, icon_a, v, Vec2::new(70.0, 10.0)).apply(&mut d).unwrap();
        let g = *d.get(v).unwrap().children.last().unwrap();
        let mut g_node = d.get(g).unwrap().clone();
        g_node.name = if twins { "lead" } else { "glyph" }.into();
        Command::Update { node: Box::new(g_node) }.apply(&mut d).unwrap();
        if twins {
            create_instance(&mut d, icon_a, v, Vec2::new(100.0, 10.0)).apply(&mut d).unwrap();
            let t = *d.get(v).unwrap().children.last().unwrap();
            let mut t_node = d.get(t).unwrap().clone();
            t_node.name = "trail".into();
            Command::Update { node: Box::new(t_node) }.apply(&mut d).unwrap();
        }
        create_component(&mut d, v).apply(&mut d).unwrap();
        mains.push(v);
    }
    combine_as_variants(&mut d, &mains).apply(&mut d).unwrap();
    let id = d.alloc_id();
    let wmain = d.insert(page, Node::frame(id, Rect::new(0.0, 300.0, 400.0, 100.0))).unwrap();
    create_instance(&mut d, mains[0], wmain, Vec2::new(10.0, 10.0)).apply(&mut d).unwrap();
    let n_main = *d.get(wmain).unwrap().children.last().unwrap();
    create_component(&mut d, wmain).apply(&mut d).unwrap();
    let mut h = History::default();
    let c = bind_prop_variable(&mut d, n_main, "Size", Some("size")).expect("bind on the nested instance inside the wrapper main");
    h.exec(&mut d, c).unwrap();
    let id = d.alloc_id();
    let host = d.insert(page, Node::frame(id, Rect::new(500.0, 0.0, 400.0, 200.0))).unwrap();
    create_instance(&mut d, wmain, host, Vec2::new(10.0, 10.0)).apply(&mut d).unwrap();
    let w = *d.get(host).unwrap().children.last().unwrap();
    Nx { d, page, wmain, host, w, n_main, small: mains[0], large: mains[1], icon_a, icon_b, h }
}

fn n_copy(f: &Nx) -> NodeId {
    first_child(&f.d, f.w)
}

fn set_mode(f: &mut Nx, node: NodeId, mode: &str) {
    let mut n = f.d.get(node).unwrap().clone();
    n.variables.modes.insert("theme".into(), mode.into());
    f.h.exec(&mut f.d, Command::Update { node: Box::new(n) }).unwrap();
}

/// Text, fill and inner-swap overrides on the nested instance inside `W`, recorded as a person's edits are.
fn override_everything(f: &mut Nx) {
    let n = n_copy(f);
    override_everything_at(f, n);
}

fn override_everything_at(f: &mut Nx, n: NodeId) {
    let label = child_named(&f.d, n, "label");
    let mut ln = f.d.get(label).unwrap().clone();
    if let NodeKind::Text(t) = &mut ln.kind {
        t.content = "Custom".into();
    }
    ln.component.overrides.insert(OverrideField::Characters);
    f.h.exec(&mut f.d, Command::Update { node: Box::new(ln) }).unwrap();
    let bg = child_named(&f.d, n, "bg");
    let mut bn = f.d.get(bg).unwrap().clone();
    bn.fills = red();
    bn.component.overrides.insert(OverrideField::Fills);
    f.h.exec(&mut f.d, Command::Update { node: Box::new(bn) }).unwrap();
    let glyph = child_named(&f.d, n, "glyph");
    let c = swap_instance(&mut f.d, glyph, f.icon_b);
    f.h.exec(&mut f.d, c).unwrap();
}

fn assert_overrides_kept(f: &Nx) {
    assert_overrides_kept_at(f, n_copy(f));
}

fn assert_overrides_kept_at(f: &Nx, n: NodeId) {
    let label = child_named(&f.d, n, "label");
    assert_eq!(text_of(&f.d, label), "Custom", "the text override survives the switch");
    assert!(f.d.get(label).unwrap().component.overrides.contains(&OverrideField::Characters));
    let bg = child_named(&f.d, n, "bg");
    assert_eq!(f.d.get(bg).unwrap().fills, red(), "the fill override survives the switch");
    let glyph = child_named(&f.d, n, "glyph");
    assert_eq!(main_of(&f.d, glyph), f.icon_b, "the inner instance swap survives the switch");
}

#[test]
fn a_mode_switch_on_the_host_flips_the_nested_bound_variant() {
    let mut f = make_fx();
    assert_eq!(main_of(&f.d, n_copy(&f)), f.small);
    let host = f.host;
    set_mode(&mut f, host, "dark");
    assert_eq!(main_of(&f.d, n_copy(&f)), f.large, "dark host: the nested instance shows Large");
    assert_eq!(f.d.get(n_copy(&f)).unwrap().size.x, 200.0);
    assert!(f.d.get(n_copy(&f)).unwrap().variables.bindings.contains_key(&prop_binding_key("Size")), "the binding stays");
    set_mode(&mut f, host, "light");
    assert_eq!(main_of(&f.d, n_copy(&f)), f.small, "back to light: Small again");
}

#[test]
fn a_bound_variant_resizes_a_hug_parent_in_the_same_mode_switch() {
    let mut f = make_fx();
    let parent_id = f.d.alloc_id();
    let parent = f.d.insert(f.page, Node::frame(parent_id, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
    let mut parent_node = f.d.get(parent).unwrap().clone();
    if let NodeKind::Frame(fp) = &mut parent_node.kind {
        fp.auto_layout =
            Some(AutoLayout { direction: LayoutDirection::Horizontal, primary_sizing: Sizing::Hug, counter_sizing: Sizing::Fixed, ..AutoLayout::default() });
    }
    f.h.exec(&mut f.d, Command::Update { node: Box::new(parent_node) }).unwrap();
    let inst_cmd = create_instance(&mut f.d, f.small, parent, Vec2::ZERO);
    let inst = match &inst_cmd {
        Command::Insert { nodes, .. } => nodes[0].id,
        _ => panic!("instance creation refused"),
    };
    f.h.exec(&mut f.d, inst_cmd).unwrap();
    let bind = bind_prop_variable(&mut f.d, inst, "Size", Some("size")).unwrap();
    f.h.exec(&mut f.d, bind).unwrap();
    f.d.relayout_subtree(parent);
    let small_width = f.d.get(parent).unwrap().size.x;
    assert_eq!(small_width, 100.0);
    let mut p = f.d.get(parent).unwrap().clone();
    p.variables.modes.insert("theme".into(), "dark".into());
    f.h.exec(&mut f.d, Command::Update { node: Box::new(p) }).unwrap();
    let child = *f.d.get(parent).unwrap().children.first().unwrap();
    assert_eq!(main_of(&f.d, child), f.large);
    let large_width = f.d.get(parent).unwrap().size.x;
    assert_eq!(large_width, 200.0);
    assert!(large_width > small_width, "hug parent stayed at {small_width} after bound variant grew to {large_width}");
    f.h.undo(&mut f.d).unwrap();
    assert_eq!(f.d.get(parent).unwrap().size.x, small_width, "undo restores the hug parent size");
    assert_eq!(main_of(&f.d, child), f.small, "undo restores the bound variant");
    f.h.redo(&mut f.d).unwrap();
    assert_eq!(f.d.get(parent).unwrap().size.x, large_width, "redo restores the hug parent size");
    assert_eq!(main_of(&f.d, child), f.large, "redo restores the bound variant");
}

#[test]
fn text_fill_and_inner_swap_overrides_survive_the_variable_driven_switch() {
    let mut f = make_fx();
    override_everything(&mut f);
    assert_overrides_kept(&f);
    let host = f.host;
    set_mode(&mut f, host, "dark");
    assert_eq!(main_of(&f.d, n_copy(&f)), f.large);
    assert_overrides_kept(&f);
    set_mode(&mut f, host, "light");
    assert_eq!(main_of(&f.d, n_copy(&f)), f.small);
    assert_overrides_kept(&f);
}

#[test]
fn the_variable_driven_switch_carries_overrides_as_a_manual_switch_does() {
    let mut driven = make_fx();
    override_everything(&mut driven);
    let host = driven.host;
    set_mode(&mut driven, host, "dark");

    let mut manual = make_fx();
    override_everything(&mut manual);
    let n = n_copy(&manual);
    let c = set_nested_instance_prop(&mut manual.d, n, "Size", Some(PropValue::Variant("Large".into())));
    manual.h.exec(&mut manual.d, c).unwrap();

    let snap = |f: &Nx| {
        let n = n_copy(f);
        let label = child_named(&f.d, n, "label");
        let bg = child_named(&f.d, n, "bg");
        let glyph = child_named(&f.d, n, "glyph");
        (
            f.d.get(n).unwrap().size,
            text_of(&f.d, label),
            f.d.get(label).unwrap().component.overrides.clone(),
            f.d.get(bg).unwrap().fills.clone(),
            f.d.get(bg).unwrap().component.overrides.clone(),
            main_of(&f.d, glyph) == f.icon_b,
        )
    };
    assert_eq!(snap(&driven), snap(&manual));
}

#[test]
fn a_mode_switch_on_the_wrapper_instance_itself_flips_the_nested_variant() {
    let mut f = make_fx();
    override_everything(&mut f);
    let w = f.w;
    set_mode(&mut f, w, "dark");
    assert_eq!(main_of(&f.d, n_copy(&f)), f.large);
    assert_overrides_kept(&f);
}

#[test]
fn a_mode_switch_on_an_ancestor_above_the_host_flips_the_nested_variant() {
    let mut f = make_fx();
    let id = f.d.alloc_id();
    let outer = f.d.insert(f.page, Node::frame(id, Rect::new(500.0, 400.0, 600.0, 400.0))).unwrap();
    f.h.exec(&mut f.d, Command::Reparent { id: f.host, parent: outer, index: 0 }).unwrap();
    assert_eq!(main_of(&f.d, n_copy(&f)), f.small);
    set_mode(&mut f, outer, "dark");
    assert_eq!(main_of(&f.d, n_copy(&f)), f.large, "the mode of a frame two levels up reaches the nested instance");
}

#[test]
fn a_mode_switch_on_the_page_flips_the_nested_variant() {
    let mut f = make_fx();
    // The wrapper main keeps its own mode, so only the copy sees the page's.
    let wmain = f.wmain;
    set_mode(&mut f, wmain, "light");
    let page = f.page;
    set_mode(&mut f, page, "dark");
    assert_eq!(main_of(&f.d, f.n_main), f.small, "the wrapper main stays on its own mode");
    assert_eq!(main_of(&f.d, n_copy(&f)), f.large, "the copy resolves the page's mode");
}

#[test]
fn a_nested_instance_resolves_in_the_mode_of_where_it_sits_not_of_the_main() {
    let mut f = make_fx();
    let wmain = f.wmain;
    set_mode(&mut f, wmain, "dark");
    assert_eq!(main_of(&f.d, f.n_main), f.large, "the main-side instance follows the main's own mode");
    assert_eq!(main_of(&f.d, n_copy(&f)), f.small, "the copy in W is outside the main and stays on the host's light mode");
    let c = sync_instances(&mut f.d, f.wmain);
    c.apply(&mut f.d).unwrap();
    assert_eq!(main_of(&f.d, n_copy(&f)), f.small, "a sync of the wrapper main does not drag the copy into the main's mode");
    let host = f.host;
    set_mode(&mut f, host, "dark");
    assert_eq!(main_of(&f.d, n_copy(&f)), f.large);
}

#[test]
fn changing_the_value_of_the_variable_in_the_active_mode_flips_the_nested_variant() {
    let mut f = make_fx();
    override_everything(&mut f);
    let mut vars = f.d.variables.clone();
    vars.get_mut("size").unwrap().values.insert("light".into(), VariableValue::String("Large".into()));
    let cmd = Command::SetVariables { variables: vars, collections: f.d.variable_collections.clone() };
    f.h.exec(&mut f.d, cmd).unwrap();
    assert_eq!(main_of(&f.d, n_copy(&f)), f.large);
    assert_overrides_kept(&f);
    f.h.undo(&mut f.d).unwrap();
    assert_eq!(main_of(&f.d, n_copy(&f)), f.small, "one undo restores the variable and the variant");
    assert_overrides_kept(&f);
}

/// A second variable that gives the opposite of `size`: Light "Large", Dark "Small".
fn add_inverse_size_variable(f: &mut Nx) {
    let s = |v: &str| VariableValue::String(v.into());
    f.d.variables.insert("size2".into(), theme_variable("size2", VariableKind::String, s("Large"), s("Small")));
    f.d.variable_collections.get_mut("theme").unwrap().variables.push("size2".into());
}

fn bind_copy_to(f: &mut Nx, var: &str) {
    let copy = n_copy(f);
    let c = bind_prop_variable(&mut f.d, copy, "Size", Some(var)).expect("bind on the copy");
    f.h.exec(&mut f.d, c).unwrap();
}

fn bound_var(f: &Nx) -> Option<String> {
    f.d.get(n_copy(f)).unwrap().variables.bindings.get(&prop_binding_key("Size")).cloned()
}

#[test]
fn a_binding_set_on_the_copy_wins_over_the_mains_and_drives_in_the_copys_mode() {
    let mut f = make_fx();
    add_inverse_size_variable(&mut f);
    override_everything(&mut f);
    assert_eq!(main_of(&f.d, n_copy(&f)), f.small, "the main's binding gives Small in light");
    bind_copy_to(&mut f, "size2");
    assert_eq!(main_of(&f.d, n_copy(&f)), f.large, "the copy's own variable gives Large in light, against the main's Small");
    assert_overrides_kept(&f);
    let host = f.host;
    set_mode(&mut f, host, "dark");
    assert_eq!(main_of(&f.d, n_copy(&f)), f.small, "and it follows the copy's mode: dark gives Small, against the main's Large");
    assert_overrides_kept(&f);
}

#[test]
fn a_binding_set_on_the_copy_survives_a_sync_of_the_wrapper_main() {
    let mut f = make_fx();
    add_inverse_size_variable(&mut f);
    bind_copy_to(&mut f, "size2");
    sync_instances(&mut f.d, f.wmain).apply(&mut f.d).unwrap();
    assert_eq!(bound_var(&f).as_deref(), Some("size2"), "the sync leaves the copy's own binding");
    assert_eq!(main_of(&f.d, n_copy(&f)), f.large);
    let host = f.host;
    set_mode(&mut f, host, "dark");
    assert_eq!(main_of(&f.d, n_copy(&f)), f.small);
}

#[test]
fn a_binding_set_on_the_copy_where_the_main_has_none_survives_a_sync() {
    let mut f = make_fx();
    add_inverse_size_variable(&mut f);
    let c = bind_prop_variable(&mut f.d, f.n_main, "Size", None).unwrap();
    f.h.exec(&mut f.d, c).unwrap();
    sync_instances(&mut f.d, f.wmain).apply(&mut f.d).unwrap();
    assert_eq!(bound_var(&f), None);
    bind_copy_to(&mut f, "size2");
    sync_instances(&mut f.d, f.wmain).apply(&mut f.d).unwrap();
    assert_eq!(bound_var(&f).as_deref(), Some("size2"));
    assert_eq!(main_of(&f.d, n_copy(&f)), f.large);
}

#[test]
fn resetting_the_instance_gives_the_mains_binding_back() {
    let mut f = make_fx();
    add_inverse_size_variable(&mut f);
    bind_copy_to(&mut f, "size2");
    assert_eq!(main_of(&f.d, n_copy(&f)), f.large);
    let c = reset_instance(&mut f.d, f.w);
    f.h.exec(&mut f.d, c).unwrap();
    assert_eq!(bound_var(&f).as_deref(), Some("size"), "the main's variable is back");
    assert_eq!(main_of(&f.d, n_copy(&f)), f.small, "and drives: light gives Small");
    let host = f.host;
    set_mode(&mut f, host, "dark");
    assert_eq!(main_of(&f.d, n_copy(&f)), f.large);
    f.h.undo(&mut f.d).unwrap();
    f.h.undo(&mut f.d).unwrap();
    assert_eq!(bound_var(&f).as_deref(), Some("size2"), "undo of the reset gives the copy's own binding back");
}

#[test]
fn resetting_the_copys_variant_gives_the_mains_binding_back_and_with_none_removes_the_copys() {
    let mut f = make_fx();
    add_inverse_size_variable(&mut f);
    bind_copy_to(&mut f, "size2");
    let copy = n_copy(&f);
    let c = reset_overrides(&mut f.d, copy, &[OverrideField::MainComponent]);
    f.h.exec(&mut f.d, c).unwrap();
    assert_eq!(bound_var(&f).as_deref(), Some("size"));
    // the main has no binding: reset leaves the copy with none
    let c = bind_prop_variable(&mut f.d, f.n_main, "Size", None).unwrap();
    f.h.exec(&mut f.d, c).unwrap();
    sync_instances(&mut f.d, f.wmain).apply(&mut f.d).unwrap();
    bind_copy_to(&mut f, "size2");
    let copy = n_copy(&f);
    let c = reset_overrides(&mut f.d, copy, &[OverrideField::MainComponent]);
    f.h.exec(&mut f.d, c).unwrap();
    assert_eq!(bound_var(&f), None, "the main binds nothing, so the reset leaves nothing");
}

#[test]
fn a_binding_set_on_the_copy_survives_a_native_save_and_load() {
    let mut f = make_fx();
    add_inverse_size_variable(&mut f);
    bind_copy_to(&mut f, "size2");
    let json = f.d.to_json().unwrap();
    let mut d2 = Document::from_json(&json).unwrap();
    let copy = first_child(&d2, f.w);
    assert_eq!(d2.get(copy).unwrap().variables.bindings.get(&prop_binding_key("Size")).map(String::as_str), Some("size2"));
    sync_instances(&mut d2, f.wmain).apply(&mut d2).unwrap();
    assert_eq!(d2.get(copy).unwrap().variables.bindings.get(&prop_binding_key("Size")).map(String::as_str), Some("size2"), "still its own after a sync");
    let mut h = History::default();
    let mut hn = d2.get(f.host).unwrap().clone();
    hn.variables.modes.insert("theme".into(), "dark".into());
    h.exec(&mut d2, Command::Update { node: Box::new(hn) }).unwrap();
    assert_eq!(main_of(&d2, copy), f.small, "and drives from the copy's variable after the reload");
}

fn set_light_value(f: &mut Nx, value: &str) {
    let mut vars = f.d.variables.clone();
    vars.get_mut("size").unwrap().values.insert("light".into(), VariableValue::String(value.into()));
    let collections = f.d.variable_collections.clone();
    f.h.exec(&mut f.d, Command::SetVariables { variables: vars, collections }).unwrap();
}

#[test]
fn a_second_variable_edit_keeps_the_overrides_after_the_main_side_instance_flipped_with_the_copy() {
    let mut f = make_fx();
    override_everything(&mut f);
    set_light_value(&mut f, "Large");
    assert_eq!(main_of(&f.d, n_copy(&f)), f.large);
    assert_eq!(main_of(&f.d, f.n_main), f.large, "the wrapper main's own instance flipped in the same edit");
    set_light_value(&mut f, "Small");
    assert_eq!(main_of(&f.d, n_copy(&f)), f.small, "the next edit switches the copy back");
    assert_overrides_kept(&f);
}

#[test]
fn a_wrapper_sync_after_the_copy_and_the_main_side_instance_followed_the_same_mode_keeps_the_overrides() {
    let mut f = make_fx();
    override_everything(&mut f);
    let page = f.page;
    set_mode(&mut f, page, "dark");
    assert_eq!(main_of(&f.d, n_copy(&f)), f.large);
    assert_eq!(main_of(&f.d, f.n_main), f.large, "both follow the page");
    sync_instances(&mut f.d, f.wmain).apply(&mut f.d).unwrap();
    assert_eq!(main_of(&f.d, n_copy(&f)), f.large);
    assert_overrides_kept(&f);
}

#[test]
fn a_second_variable_edit_after_a_wrapper_sync_keeps_the_overrides_too() {
    let mut f = make_fx();
    override_everything(&mut f);
    set_light_value(&mut f, "Large");
    sync_instances(&mut f.d, f.wmain).apply(&mut f.d).unwrap();
    assert_overrides_kept(&f);
    set_light_value(&mut f, "Small");
    assert_eq!(main_of(&f.d, n_copy(&f)), f.small);
    assert_overrides_kept(&f);
}

#[test]
fn changing_the_default_mode_of_the_collection_flips_the_nested_variant() {
    let mut f = make_fx();
    // The wrapper main keeps its own mode, so only the copy sees the default change.
    let wmain = f.wmain;
    set_mode(&mut f, wmain, "light");
    let mut colls = f.d.variable_collections.clone();
    colls.get_mut("theme").unwrap().default_mode = "dark".into();
    let cmd = Command::SetVariables { variables: f.d.variables.clone(), collections: colls };
    f.h.exec(&mut f.d, cmd).unwrap();
    assert_eq!(main_of(&f.d, f.n_main), f.small);
    assert_eq!(main_of(&f.d, n_copy(&f)), f.large);
}

#[test]
fn a_binding_added_on_the_main_reaches_the_copy_after_a_sync() {
    let mut f = make_fx();
    // Take the binding off the main first, so the copies follow the main's variant only.
    let c = bind_prop_variable(&mut f.d, f.n_main, "Size", None).unwrap();
    f.h.exec(&mut f.d, c).unwrap();
    sync_instances(&mut f.d, f.wmain).apply(&mut f.d).unwrap();
    assert!(!f.d.get(n_copy(&f)).unwrap().variables.bindings.contains_key(&prop_binding_key("Size")), "the binding leaves the copy with the main");
    let host = f.host;
    set_mode(&mut f, host, "dark");
    assert_eq!(main_of(&f.d, n_copy(&f)), f.small, "no binding: the copy does not follow the mode");
    let c = bind_prop_variable(&mut f.d, f.n_main, "Size", Some("size")).unwrap();
    f.h.exec(&mut f.d, c).unwrap();
    sync_instances(&mut f.d, f.wmain).apply(&mut f.d).unwrap();
    assert!(f.d.get(n_copy(&f)).unwrap().variables.bindings.contains_key(&prop_binding_key("Size")), "the binding reaches the copy");
    assert_eq!(main_of(&f.d, n_copy(&f)), f.large, "the copy resolves the new binding where it sits (dark host)");
}

#[test]
fn a_binding_removed_on_the_main_sends_the_copy_back_to_the_main_variant() {
    let mut f = make_fx();
    let host = f.host;
    set_mode(&mut f, host, "dark");
    assert_eq!(main_of(&f.d, n_copy(&f)), f.large);
    let c = bind_prop_variable(&mut f.d, f.n_main, "Size", None).unwrap();
    f.h.exec(&mut f.d, c).unwrap();
    sync_instances(&mut f.d, f.wmain).apply(&mut f.d).unwrap();
    assert_eq!(main_of(&f.d, n_copy(&f)), main_of(&f.d, f.n_main), "the copy shows what the main shows again");
    assert!(!f.d.get(n_copy(&f)).unwrap().variables.bindings.contains_key(&prop_binding_key("Size")));
}

#[test]
fn moving_the_wrapper_instance_into_a_frame_with_another_mode_flips_the_variant() {
    let mut f = make_fx();
    override_everything(&mut f);
    let id = f.d.alloc_id();
    let dark = f.d.insert(f.page, Node::frame(id, Rect::new(1000.0, 0.0, 300.0, 200.0))).unwrap();
    set_mode(&mut f, dark, "dark");
    f.h.exec(&mut f.d, Command::Reparent { id: f.w, parent: dark, index: 0 }).unwrap();
    assert_eq!(main_of(&f.d, n_copy(&f)), f.large, "moved into a dark frame");
    assert_overrides_kept(&f);
    f.h.undo(&mut f.d).unwrap();
    assert_eq!(main_of(&f.d, n_copy(&f)), f.small, "one undo moves it back and restores the variant");
    assert_overrides_kept(&f);
}

#[test]
fn pasting_a_copy_of_the_wrapper_instance_into_a_frame_with_another_mode_flips_the_variant() {
    let mut f = make_fx();
    let id = f.d.alloc_id();
    let dark = f.d.insert(f.page, Node::frame(id, Rect::new(1000.0, 0.0, 300.0, 200.0))).unwrap();
    set_mode(&mut f, dark, "dark");
    let nodes = duplicate_subtree(&mut f.d, f.w);
    let pasted = nodes[0].id;
    f.h.exec(&mut f.d, Command::Insert { parent: dark, index: 0, nodes }).unwrap();
    assert_eq!(main_of(&f.d, first_child(&f.d, pasted)), f.large, "the pasted copy resolves in the dark frame");
    assert_eq!(main_of(&f.d, n_copy(&f)), f.small, "the original is untouched");
}

#[test]
fn a_manual_pick_on_the_nested_instance_wins_until_the_override_is_reset() {
    let mut f = make_fx();
    let n = n_copy(&f);
    let c = set_nested_instance_prop(&mut f.d, n, "Size", Some(PropValue::Variant("Large".into())));
    f.h.exec(&mut f.d, c).unwrap();
    assert_eq!(main_of(&f.d, n), f.large);
    assert!(f.d.get(n).unwrap().component.overrides.contains(&OverrideField::MainComponent));
    let host = f.host;
    set_mode(&mut f, host, "dark");
    set_mode(&mut f, host, "light");
    assert_eq!(main_of(&f.d, n), f.large, "the by-hand pick holds against the binding");
    let c = reset_overrides(&mut f.d, n, &[OverrideField::MainComponent]);
    f.h.exec(&mut f.d, c).unwrap();
    assert_eq!(main_of(&f.d, n), f.small, "reset: the binding drives again (light)");
    set_mode(&mut f, host, "dark");
    assert_eq!(main_of(&f.d, n), f.large, "and it follows the mode again");
}

#[test]
fn one_undo_restores_the_variant_and_every_override_and_redo_applies_them_again() {
    let mut f = make_fx();
    override_everything(&mut f);
    let host = f.host;
    set_mode(&mut f, host, "dark");
    assert_eq!(main_of(&f.d, n_copy(&f)), f.large);
    f.h.undo(&mut f.d).unwrap();
    assert_eq!(main_of(&f.d, n_copy(&f)), f.small);
    assert!(f.d.get(host).unwrap().variables.modes.is_empty(), "the mode is back too");
    assert_overrides_kept(&f);
    f.h.redo(&mut f.d).unwrap();
    assert_eq!(main_of(&f.d, n_copy(&f)), f.large);
    assert_overrides_kept(&f);
}

#[test]
fn a_nested_instance_without_a_binding_is_untouched_by_a_mode_switch() {
    let mut f = make_fx();
    let c = bind_prop_variable(&mut f.d, f.n_main, "Size", None).unwrap();
    f.h.exec(&mut f.d, c).unwrap();
    sync_instances(&mut f.d, f.wmain).apply(&mut f.d).unwrap();
    override_everything(&mut f);
    let before = f.d.get(n_copy(&f)).unwrap().clone();
    let host = f.host;
    set_mode(&mut f, host, "dark");
    let after = f.d.get(n_copy(&f)).unwrap();
    assert_eq!(main_of(&f.d, n_copy(&f)), f.small);
    assert_eq!(*after, before);
    assert_overrides_kept(&f);
}

#[test]
fn a_variant_value_that_is_no_option_keeps_the_nested_variant_it_has() {
    let mut f = make_fx();
    let mut vars = f.d.variables.clone();
    vars.get_mut("size").unwrap().values.insert("dark".into(), VariableValue::String("Huge".into()));
    let collections = f.d.variable_collections.clone();
    f.h.exec(&mut f.d, Command::SetVariables { variables: vars, collections }).unwrap();
    let host = f.host;
    set_mode(&mut f, host, "dark");
    assert_eq!(main_of(&f.d, n_copy(&f)), f.small, "no option called Huge: the variant stays");
    assert!(prop_binding_problem(&f.d, n_copy(&f), "Size").is_some(), "and the linter-facing check names the reason");
}

#[test]
fn the_binding_and_the_variant_survive_a_native_save_and_load() {
    let mut f = make_fx();
    override_everything(&mut f);
    let host = f.host;
    set_mode(&mut f, host, "dark");
    let json = f.d.to_json().unwrap();
    let mut d2 = Document::from_json(&json).unwrap();
    let n = first_child(&d2, f.w);
    assert_eq!(main_of(&d2, n), f.large, "the variant is stored");
    assert!(d2.get(n).unwrap().variables.bindings.contains_key(&prop_binding_key("Size")), "the binding is stored");
    let mut h = History::default();
    let mut hn = d2.get(f.host).unwrap().clone();
    hn.variables.modes.insert("theme".into(), "light".into());
    h.exec(&mut d2, Command::Update { node: Box::new(hn) }).unwrap();
    assert_eq!(main_of(&d2, n), f.small, "after the reload the nested instance still follows the mode");
    let label = child_named(&d2, n, "label");
    assert_eq!(text_of(&d2, label), "Custom");
    let _ = f.icon_a;
}

/// Timing of one mode switch over 2,000 wrapper instances that each hold a bound nested variant. Run by hand:
/// `cargo test -p designos-doc --lib --release mode_switch_cost -- --ignored --nocapture`.
#[test]
#[ignore = "timing probe, run by hand"]
fn mode_switch_cost_over_two_thousand_bound_nested_instances() {
    let mut f = make_fx();
    let (small, large, wmain) = (f.small, f.large, f.wmain);
    let host = f.host;
    let mut ids = vec![f.w];
    for i in 0..1999 {
        create_instance(&mut f.d, wmain, host, Vec2::new(10.0, 10.0 + i as f32)).apply(&mut f.d).unwrap();
        ids.push(*f.d.get(host).unwrap().children.last().unwrap());
    }
    let mut times = Vec::new();
    for mode in ["dark", "light", "dark", "light"] {
        let start = std::time::Instant::now();
        set_mode(&mut f, host, mode);
        times.push((mode, start.elapsed()));
    }
    let flipped = ids.iter().filter(|w| main_of(&f.d, first_child(&f.d, **w)) == large).count();
    let on_small = ids.iter().filter(|w| main_of(&f.d, first_child(&f.d, **w)) == small).count();
    eprintln!("MODE_SWITCH_COST 2000 instances: {times:?}; after the last switch (light) {on_small} show Small, {flipped} show Large");
}

/// `host2 > OI > W' > N''`: a wrapper main `O` holds an instance of the wrapper main, `OI` is an instance of `O` in `host2`.
/// Returns `host2` and the deepest copy, three levels below the main that holds the binding.
fn three_levels(f: &mut Nx) -> (NodeId, NodeId) {
    let id = f.d.alloc_id();
    let o = f.d.insert(f.page, Node::frame(id, Rect::new(0.0, 700.0, 500.0, 200.0))).unwrap();
    create_instance(&mut f.d, f.wmain, o, Vec2::new(5.0, 5.0)).apply(&mut f.d).unwrap();
    create_component(&mut f.d, o).apply(&mut f.d).unwrap();
    let id = f.d.alloc_id();
    let host2 = f.d.insert(f.page, Node::frame(id, Rect::new(1200.0, 0.0, 600.0, 300.0))).unwrap();
    create_instance(&mut f.d, o, host2, Vec2::new(5.0, 5.0)).apply(&mut f.d).unwrap();
    let oi = *f.d.get(host2).unwrap().children.last().unwrap();
    (host2, first_child(&f.d, first_child(&f.d, oi)))
}

#[test]
fn three_levels_deep_a_mode_switch_and_back_keeps_text_fill_and_inner_swap_overrides() {
    let mut f = make_fx();
    let (host2, deep) = three_levels(&mut f);
    override_everything_at(&mut f, deep);
    set_mode(&mut f, host2, "dark");
    assert_eq!(main_of(&f.d, deep), f.large, "the deepest copy follows the mode of its host");
    assert_overrides_kept_at(&f, deep);
    set_mode(&mut f, host2, "light");
    assert_eq!(main_of(&f.d, deep), f.small);
    assert_overrides_kept_at(&f, deep);
}

/// `host3 > PI > OI > W > N`: one more wrapper main `P` that holds an instance of `O`. Returns `host3` and the deepest copy.
fn four_levels(f: &mut Nx) -> (NodeId, NodeId) {
    let (_host2, deep3) = three_levels(f);
    let oi_main = f.d.get(f.d.get(deep3).unwrap().parent.unwrap()).unwrap().parent.unwrap();
    let o = instance_component(f.d.get(oi_main).unwrap()).unwrap();
    let id = f.d.alloc_id();
    let p = f.d.insert(f.page, Node::frame(id, Rect::new(0.0, 1300.0, 600.0, 250.0))).unwrap();
    create_instance(&mut f.d, o, p, Vec2::new(5.0, 5.0)).apply(&mut f.d).unwrap();
    create_component(&mut f.d, p).apply(&mut f.d).unwrap();
    let id = f.d.alloc_id();
    let host3 = f.d.insert(f.page, Node::frame(id, Rect::new(2000.0, 0.0, 700.0, 350.0))).unwrap();
    create_instance(&mut f.d, p, host3, Vec2::new(5.0, 5.0)).apply(&mut f.d).unwrap();
    let pi = *f.d.get(host3).unwrap().children.last().unwrap();
    let oi = first_child(&f.d, pi);
    let w = first_child(&f.d, oi);
    (host3, first_child(&f.d, w))
}

#[test]
fn four_levels_deep_a_mode_switch_and_back_keeps_text_fill_and_inner_swap_overrides() {
    let mut f = make_fx();
    let (host3, deep) = four_levels(&mut f);
    override_everything_at(&mut f, deep);
    set_mode(&mut f, host3, "dark");
    assert_eq!(main_of(&f.d, deep), f.large);
    assert_overrides_kept_at(&f, deep);
    set_mode(&mut f, host3, "light");
    assert_eq!(main_of(&f.d, deep), f.small);
    assert_overrides_kept_at(&f, deep);
}

fn names_and_mains(f: &Nx, copy: NodeId) -> Vec<String> {
    f.d.get(copy)
        .unwrap()
        .children
        .iter()
        .map(|k| {
            let n = f.d.get(*k).unwrap();
            let m = instance_component(n).and_then(|m| f.d.get(m)).map(|m| m.name.clone()).unwrap_or_default();
            format!("{}:{m}", n.name)
        })
        .collect()
}

fn swap_trail_to_icon_b(f: &mut Nx) {
    let copy = n_copy(f);
    let trail = child_named(&f.d, copy, "trail");
    let c = swap_instance(&mut f.d, trail, f.icon_b);
    f.h.exec(&mut f.d, c).unwrap();
}

#[test]
fn two_instances_of_the_same_inner_component_stay_apart_through_a_wrapper_sync() {
    let mut f = make_fx_layers(true);
    swap_trail_to_icon_b(&mut f);
    let before = names_and_mains(&f, n_copy(&f));
    assert_eq!(before, ["label:", "bg:", "lead:IconA", "trail:IconB"]);
    sync_instances(&mut f.d, f.wmain).apply(&mut f.d).unwrap();
    assert_eq!(names_and_mains(&f, n_copy(&f)), before, "the sync adds no layer and the swap stays on the same icon");
}

#[test]
fn two_instances_of_the_same_inner_component_keep_a_swap_on_the_same_icon_through_a_bound_mode_switch() {
    let mut f = make_fx_layers(true);
    swap_trail_to_icon_b(&mut f);
    let host = f.host;
    set_mode(&mut f, host, "dark");
    assert_eq!(main_of(&f.d, n_copy(&f)), f.large);
    assert_eq!(names_and_mains(&f, n_copy(&f)), ["label:", "bg:", "lead:IconA", "trail:IconB"], "dark: the swap is still on trail, no layer added");
    sync_instances(&mut f.d, f.wmain).apply(&mut f.d).unwrap();
    assert_eq!(names_and_mains(&f, n_copy(&f)), ["label:", "bg:", "lead:IconA", "trail:IconB"], "and after a wrapper sync");
    set_mode(&mut f, host, "light");
    assert_eq!(main_of(&f.d, n_copy(&f)), f.small);
    assert_eq!(names_and_mains(&f, n_copy(&f)), ["label:", "bg:", "lead:IconA", "trail:IconB"], "back to light too");
}

#[test]
fn three_levels_deep_an_inner_swap_the_new_variant_has_no_counterpart_for_is_counted() {
    let mut f = make_fx();
    let glyph = child_named(&f.d, f.large, "glyph");
    Command::Remove { id: glyph }.apply(&mut f.d).unwrap();
    let (_host2, deep) = three_levels(&mut f);
    override_everything_at(&mut f, deep);
    let (cmd, dropped) = crate::components::swap_instance_inner(&mut f.d, deep, f.large);
    assert_eq!(dropped, 1, "the swapped glyph has nowhere to go in Large and is counted");
    f.h.exec(&mut f.d, cmd).unwrap();
    let label = child_named(&f.d, deep, "label");
    assert_eq!(text_of(&f.d, label), "Custom", "what can carry still carries");
}

#[test]
fn a_copys_own_binding_does_not_reach_a_new_main_made_from_the_detached_wrapper_instance() {
    let mut f = make_fx();
    add_inverse_size_variable(&mut f);
    bind_copy_to(&mut f, "size");
    assert!(f.d.get(n_copy(&f)).unwrap().component.own_variant_bindings.contains("Size"));
    // detach the wrapper instance, make a component from the frame, place an instance of it
    let w = f.w;
    detach_instance(&mut f.d, w).apply(&mut f.d).unwrap();
    let inner = first_child(&f.d, w);
    assert!(f.d.get(inner).unwrap().component.own_variant_bindings.is_empty(), "the detached instance holds no copy-level state");
    create_component(&mut f.d, w).apply(&mut f.d).unwrap();
    let host = f.host;
    create_instance(&mut f.d, w, host, Vec2::new(10.0, 120.0)).apply(&mut f.d).unwrap();
    let placed = *f.d.get(host).unwrap().children.last().unwrap();
    let new_copy = first_child(&f.d, placed);
    assert!(f.d.get(new_copy).unwrap().component.own_variant_bindings.is_empty());
    // the new main's binding is followed: rebound to size2 it reaches the copy, unbound it leaves it
    let c = bind_prop_variable(&mut f.d, inner, "Size", Some("size2")).unwrap();
    f.h.exec(&mut f.d, c).unwrap();
    sync_instances(&mut f.d, w).apply(&mut f.d).unwrap();
    let bound = |f: &Nx| f.d.get(new_copy).unwrap().variables.bindings.get(&prop_binding_key("Size")).cloned();
    assert_eq!(bound(&f).as_deref(), Some("size2"), "the copy follows the new main's rebind");
    let c = bind_prop_variable(&mut f.d, inner, "Size", None).unwrap();
    f.h.exec(&mut f.d, c).unwrap();
    sync_instances(&mut f.d, w).apply(&mut f.d).unwrap();
    assert_eq!(bound(&f), None, "and its unbind");
}

#[test]
fn unbinding_the_binding_a_copy_inherits_holds_the_copy_on_its_variant_until_reset() {
    let mut f = make_fx();
    override_everything(&mut f);
    let host = f.host;
    set_mode(&mut f, host, "dark");
    assert_eq!(main_of(&f.d, n_copy(&f)), f.large);
    let copy = n_copy(&f);
    let c = bind_prop_variable(&mut f.d, copy, "Size", None).unwrap();
    assert!(!matches!(&c, Command::Batch(v) if v.is_empty()), "the unbind changes something: it is not reported and dropped");
    f.h.exec(&mut f.d, c).unwrap();
    assert_eq!(bound_var(&f), None, "the copy holds no binding now");
    assert!(f.d.get(copy).unwrap().component.overrides.contains(&OverrideField::MainComponent), "it is held like a pick");
    assert_eq!(main_of(&f.d, copy), f.large, "on the variant it showed");
    set_mode(&mut f, host, "light");
    assert_eq!(main_of(&f.d, copy), f.large, "a mode change no longer moves it");
    sync_instances(&mut f.d, f.wmain).apply(&mut f.d).unwrap();
    assert_eq!(bound_var(&f), None, "and a sync of the main does not put the binding back");
    assert_eq!(main_of(&f.d, copy), f.large);
    assert_overrides_kept(&f);
    let c = reset_instance(&mut f.d, f.w);
    f.h.exec(&mut f.d, c).unwrap();
    assert_eq!(bound_var(&f).as_deref(), Some("size"), "reset gives the main's binding back");
    assert_eq!(main_of(&f.d, copy), f.small, "and light gives Small again");
    set_mode(&mut f, host, "dark");
    assert_eq!(main_of(&f.d, copy), f.large, "it follows the mode again");
}

#[test]
fn unbinding_a_copys_own_binding_where_the_main_binds_nothing_just_removes_it() {
    let mut f = make_fx();
    add_inverse_size_variable(&mut f);
    let c = bind_prop_variable(&mut f.d, f.n_main, "Size", None).unwrap();
    f.h.exec(&mut f.d, c).unwrap();
    sync_instances(&mut f.d, f.wmain).apply(&mut f.d).unwrap();
    bind_copy_to(&mut f, "size2");
    let copy = n_copy(&f);
    let c = bind_prop_variable(&mut f.d, copy, "Size", None).unwrap();
    f.h.exec(&mut f.d, c).unwrap();
    assert_eq!(bound_var(&f), None);
    assert!(f.d.get(copy).unwrap().component.own_variant_bindings.is_empty());
    assert!(!f.d.get(copy).unwrap().component.overrides.contains(&OverrideField::MainComponent), "nothing was inherited, so nothing is held");
}

#[test]
fn a_new_instance_of_a_main_starts_its_copies_without_copy_level_state() {
    let mut f = make_fx();
    // state left on a main-side instance (for instance by a file written elsewhere) must not reach the copies of that main
    let mut n_main = f.d.get(f.n_main).unwrap().clone();
    n_main.component.own_variant_bindings.insert("Size".into());
    Command::Update { node: Box::new(n_main) }.apply(&mut f.d).unwrap();
    let host = f.host;
    create_instance(&mut f.d, f.wmain, host, Vec2::new(10.0, 120.0)).apply(&mut f.d).unwrap();
    let placed = *f.d.get(host).unwrap().children.last().unwrap();
    assert!(f.d.get(first_child(&f.d, placed)).unwrap().component.own_variant_bindings.is_empty());
}

#[test]
fn a_duplicate_of_the_wrapper_instance_keeps_the_copys_own_binding_as_its_own() {
    // `duplicate_subtree` copies without relinking (`copy_tree(.., false)`): a duplicate is the same instance with the
    // same overrides, so the copy's own binding stays its own there, as its other overrides do.
    let mut f = make_fx();
    add_inverse_size_variable(&mut f);
    bind_copy_to(&mut f, "size2");
    let nodes = duplicate_subtree(&mut f.d, f.w);
    let pasted = nodes[0].id;
    let host = f.host;
    f.h.exec(&mut f.d, Command::Insert { parent: host, index: 1, nodes }).unwrap();
    let copy = first_child(&f.d, pasted);
    assert!(f.d.get(copy).unwrap().component.own_variant_bindings.contains("Size"), "the duplicate's copy keeps its own binding");
    sync_instances(&mut f.d, f.wmain).apply(&mut f.d).unwrap();
    assert_eq!(f.d.get(copy).unwrap().variables.bindings.get(&prop_binding_key("Size")).map(String::as_str), Some("size2"), "through a sync too");
}

#[test]
fn a_copys_own_binding_is_a_typed_field_and_not_plugin_data() {
    let mut f = make_fx();
    add_inverse_size_variable(&mut f);
    bind_copy_to(&mut f, "size2");
    let copy = n_copy(&f);
    assert!(f.d.get(copy).unwrap().plugin_data.is_empty(), "nothing shows up as user plugin data");
    assert!(!f.d.to_json().unwrap().contains("designos:own"), "and the saved file names no such key");
}

#[test]
fn three_levels_deep_an_inner_swap_that_can_carry_is_not_counted() {
    let mut f = make_fx();
    let (_host2, deep) = three_levels(&mut f);
    override_everything_at(&mut f, deep);
    let (_cmd, dropped) = crate::components::swap_instance_inner(&mut f.d, deep, f.large);
    assert_eq!(dropped, 0);
}
