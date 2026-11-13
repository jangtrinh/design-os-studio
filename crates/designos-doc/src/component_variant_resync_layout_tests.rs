//! A variable-driven switch of a bound variant solves the auto-layout ancestors of the instances it resized in the same step.
//!
//! Fixture: a set `Button` with the property `Size` (Small 100 wide, Large 200 wide). The collection `theme` has the
//! string variable `size` (light Small, dark Large). A wrapper main (auto layout, hugging) holds one nested instance of
//! the set with `Size` bound to `size`. Inside `stage` (a plain frame that carries the mode) sit three hugging frames
//! with 4 px padding:
//! * `hug_top` holds `top`, an instance of the set with `Size` bound;
//! * `hug_nested` holds `w`, an instance of the wrapper, whose nested copy `copy` follows the binding;
//! * `hug_quiet` holds a fixed frame `quiet` and takes no part in any switch.

use crate::component_variant_picks_tests::string_variable;
use crate::*;

const SMALL: &str = "Size=Small";
const LARGE: &str = "Size=Large";

struct Fx {
    d: Document,
    h: History,
    stage: NodeId,
    hug_top: NodeId,
    top: NodeId,
    hug_nested: NodeId,
    w: NodeId,
    copy: NodeId,
    hug_quiet: NodeId,
    quiet: NodeId,
}

fn hugging_row(padding: f32) -> AutoLayout {
    AutoLayout {
        direction: LayoutDirection::Horizontal,
        primary_sizing: Sizing::Hug,
        counter_sizing: Sizing::Hug,
        padding: [padding; 4],
        layout_version: 1,
        ..AutoLayout::default()
    }
}

fn hugging_frame(d: &mut Document, parent: NodeId, x: f32, padding: f32) -> NodeId {
    let id = d.alloc_id();
    let mut f = Node::frame(id, Rect::new(x, 0.0, 10.0, 10.0));
    if let NodeKind::Frame(fp) = &mut f.kind {
        fp.auto_layout = Some(hugging_row(padding));
    }
    d.insert(parent, f).unwrap()
}

fn last_child(d: &Document, parent: NodeId) -> NodeId {
    *d.get(parent).unwrap().children.last().unwrap()
}

fn build() -> Fx {
    let mut d = Document::new("t");
    let page = d.add_page("P");
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
    d.variables.insert("size".into(), string_variable("size", "Small", "Large"));
    let mut mains = Vec::new();
    for (name, wd) in [(SMALL, 100.0), (LARGE, 200.0)] {
        let id = d.alloc_id();
        let mut v = Node::frame(id, Rect::new(0.0, 0.0, wd, 40.0));
        v.name = name.into();
        let v = d.insert(page, v).unwrap();
        create_component(&mut d, v).apply(&mut d).unwrap();
        mains.push(v);
    }
    combine_as_variants(&mut d, &mains).apply(&mut d).unwrap();

    let wmain = hugging_frame(&mut d, page, 0.0, 0.0);
    create_instance(&mut d, mains[0], wmain, Vec2::new(0.0, 0.0)).apply(&mut d).unwrap();
    let n_main = last_child(&d, wmain);
    create_component(&mut d, wmain).apply(&mut d).unwrap();
    let mut h = History::default();
    let c = bind_prop_variable(&mut d, n_main, "Size", Some("size")).expect("bind inside the wrapper main");
    h.exec(&mut d, c).unwrap();

    let id = d.alloc_id();
    let stage = d.insert(page, Node::frame(id, Rect::new(500.0, 0.0, 900.0, 900.0))).unwrap();
    let hug_top = hugging_frame(&mut d, stage, 0.0, 4.0);
    create_instance(&mut d, mains[0], hug_top, Vec2::new(0.0, 0.0)).apply(&mut d).unwrap();
    let top = last_child(&d, hug_top);
    let c = bind_prop_variable(&mut d, top, "Size", Some("size")).expect("bind on the top-level instance");
    h.exec(&mut d, c).unwrap();

    let hug_nested = hugging_frame(&mut d, stage, 300.0, 4.0);
    create_instance(&mut d, wmain, hug_nested, Vec2::new(0.0, 0.0)).apply(&mut d).unwrap();
    let w = last_child(&d, hug_nested);
    let copy = *d.get(w).unwrap().children.first().unwrap();

    let hug_quiet = hugging_frame(&mut d, stage, 600.0, 4.0);
    let id = d.alloc_id();
    let quiet = d.insert(hug_quiet, Node::frame(id, Rect::new(0.0, 0.0, 50.0, 30.0))).unwrap();

    d.relayout_subtree(stage);
    Fx { d, h, stage, hug_top, top, hug_nested, w, copy, hug_quiet, quiet }
}

fn width(f: &Fx, id: NodeId) -> f32 {
    f.d.get(id).unwrap().size.x
}

fn shown(f: &Fx, inst: NodeId) -> String {
    f.d.get(instance_component(f.d.get(inst).unwrap()).unwrap()).unwrap().name.clone()
}

fn set_mode(f: &mut Fx, mode: &str) {
    let mut n = f.d.get(f.stage).unwrap().clone();
    n.variables.modes.insert("theme".into(), mode.into());
    f.h.exec(&mut f.d, Command::Update { node: Box::new(n) }).unwrap();
}

#[test]
fn the_fixture_starts_solved() {
    let f = build();
    assert_eq!(shown(&f, f.top), SMALL);
    assert_eq!(shown(&f, f.copy), SMALL);
    assert_eq!(width(&f, f.hug_top), 108.0);
    assert_eq!(width(&f, f.w), 100.0);
    assert_eq!(width(&f, f.hug_nested), 108.0);
    assert_eq!(width(&f, f.hug_quiet), 58.0);
}

#[test]
fn a_hug_host_resizes_in_the_same_step_as_a_variable_driven_switch_of_a_top_instance() {
    let mut f = build();
    set_mode(&mut f, "dark");
    assert_eq!(shown(&f, f.top), LARGE);
    assert_eq!(width(&f, f.top), 200.0);
    assert_eq!(width(&f, f.hug_top), 208.0, "the host hugs the switched instance right after the step");
}

#[test]
fn undo_and_redo_of_a_variable_driven_switch_bring_the_hug_host_back_and_forth() {
    let mut f = build();
    set_mode(&mut f, "dark");
    assert_eq!(width(&f, f.hug_top), 208.0, "precondition: the step solved the host");

    assert!(f.h.undo(&mut f.d).unwrap());
    assert_eq!(shown(&f, f.top), SMALL);
    assert_eq!(width(&f, f.hug_top), 108.0, "undo");
    assert_eq!(width(&f, f.hug_nested), 108.0, "undo, nested chain");

    assert!(f.h.redo(&mut f.d).unwrap());
    assert_eq!(shown(&f, f.top), LARGE);
    assert_eq!(width(&f, f.hug_top), 208.0, "redo");
    assert_eq!(width(&f, f.hug_nested), 208.0, "redo, nested chain");
}

#[test]
fn a_hug_host_resizes_in_the_same_step_as_a_variable_driven_switch_of_a_nested_copy() {
    let mut f = build();
    set_mode(&mut f, "dark");
    assert_eq!(shown(&f, f.copy), LARGE);
    assert_eq!(width(&f, f.copy), 200.0);
    assert_eq!(width(&f, f.w), 200.0, "the wrapper instance hugs its nested copy");
    assert_eq!(width(&f, f.hug_nested), 208.0, "and so does the host around it");
}

#[test]
fn a_switch_that_resized_an_instance_is_not_marked_layout_recorded() {
    let mut f = build();
    let opened = f.d.layout_epoch();
    set_mode(&mut f, "dark");
    assert_ne!(f.d.layout_epoch(), opened, "the step solved layout outside its own command");
    assert!(
        !f.h.mark_last_undo_as_layout_recorded(&f.d, opened),
        "its stored command does not hold the ancestors' sizes, so undo and redo must solve them again"
    );
}

#[test]
fn a_hand_picked_bound_property_is_not_resynced_so_a_mode_change_solves_nothing() {
    let mut f = build();
    let c = set_instance_prop(&mut f.d, f.top, "Size", Some(PropValue::Variant("Large".into())));
    f.h.exec(&mut f.d, c).unwrap();
    let c = set_nested_instance_prop(&mut f.d, f.copy, "Size", Some(PropValue::Variant("Large".into())));
    f.h.exec(&mut f.d, c).unwrap();
    f.d.relayout_seeds_upward(&[f.hug_top, f.w, f.hug_nested]);
    assert_eq!((shown(&f, f.top), shown(&f, f.copy)), (LARGE.to_string(), LARGE.to_string()), "precondition: the picks hold Large in light mode");
    assert_eq!((width(&f, f.hug_top), width(&f, f.hug_nested)), (208.0, 208.0), "precondition: the hosts were solved after the picks");

    let before = f.d.layout_epoch();
    set_mode(&mut f, "dark");
    set_mode(&mut f, "light");
    assert_eq!(f.d.layout_epoch(), before, "light gives Small again, but a picked property is left alone, so no layout is solved");
    assert_eq!((shown(&f, f.top), shown(&f, f.copy)), (LARGE.to_string(), LARGE.to_string()));
    assert_eq!((width(&f, f.hug_top), width(&f, f.hug_nested)), (208.0, 208.0));
}

#[test]
fn a_step_without_a_variant_switch_solves_no_layout_and_keeps_its_recording() {
    let mut f = build();
    // A mode of another collection moves the scope of every bound instance below the stage but gives no new value.
    let opened = f.d.layout_epoch();
    let mut n = f.d.get(f.stage).unwrap().clone();
    n.variables.modes.insert("other".into(), "x".into());
    f.h.exec(&mut f.d, Command::Update { node: Box::new(n) }).unwrap();
    assert_eq!(f.d.layout_epoch(), opened, "no instance changed, so nothing is solved");
    assert!(f.h.mark_last_undo_as_layout_recorded(&f.d, opened), "the step keeps the recording its editor made");
    assert_eq!((width(&f, f.hug_top), width(&f, f.hug_nested)), (108.0, 108.0));
}

#[test]
fn hug_frames_off_the_resized_chain_keep_their_positions() {
    let mut f = build();
    // Park the quiet frame off the position a solve would give it: only a solve of its host could move it.
    let mut q = f.d.get(f.quiet).unwrap().clone();
    q.transform.tx = 17.0;
    f.h.exec(&mut f.d, Command::Update { node: Box::new(q) }).unwrap();
    let quiet_at = (f.d.get(f.quiet).unwrap().transform.tx, f.d.get(f.quiet).unwrap().transform.ty);
    let hug_at = |f: &Fx, id| (f.d.get(id).unwrap().transform.tx, f.d.get(id).unwrap().transform.ty, f.d.get(id).unwrap().size);
    let quiet_host = hug_at(&f, f.hug_quiet);

    set_mode(&mut f, "dark");
    assert_eq!(width(&f, f.hug_top), 208.0, "precondition: the chain beside it did resize");
    assert_eq!((f.d.get(f.quiet).unwrap().transform.tx, f.d.get(f.quiet).unwrap().transform.ty), quiet_at, "the child of the quiet host");
    assert_eq!(hug_at(&f, f.hug_quiet), quiet_host, "the quiet host");
}

fn set_variables(
    f: &mut Fx,
    edit: impl FnOnce(&mut std::collections::BTreeMap<String, Variable>, &mut std::collections::BTreeMap<String, VariableCollection>),
) {
    let mut vars = f.d.variables.clone();
    let mut collections = f.d.variable_collections.clone();
    edit(&mut vars, &mut collections);
    f.h.exec(&mut f.d, Command::SetVariables { variables: vars, collections }).unwrap();
}

fn assert_hosts_follow_step_undo_and_redo(f: &mut Fx, big: (f32, f32), small: (f32, f32)) {
    assert_eq!((shown(f, f.top), shown(f, f.copy)), (LARGE.to_string(), LARGE.to_string()));
    assert_eq!((width(f, f.hug_top), width(f, f.hug_nested)), big, "the step");
    assert!(f.h.undo(&mut f.d).unwrap());
    assert_eq!((shown(f, f.top), shown(f, f.copy)), (SMALL.to_string(), SMALL.to_string()));
    assert_eq!((width(f, f.hug_top), width(f, f.hug_nested)), small, "undo");
    assert!(f.h.redo(&mut f.d).unwrap());
    assert_eq!((width(f, f.hug_top), width(f, f.hug_nested)), big, "redo");
}

#[test]
fn a_variable_value_edit_resizes_the_hug_hosts_in_its_step_and_in_undo_and_redo() {
    let mut f = build();
    set_variables(&mut f, |vars, _| {
        vars.get_mut("size").unwrap().values.insert("light".into(), VariableValue::String("Large".into()));
    });
    assert_hosts_follow_step_undo_and_redo(&mut f, (208.0, 208.0), (108.0, 108.0));
}

#[test]
fn a_default_mode_change_resizes_the_hug_hosts_in_its_step_and_in_undo_and_redo() {
    let mut f = build();
    set_variables(&mut f, |_, collections| {
        collections.get_mut("theme").unwrap().default_mode = "dark".into();
    });
    assert_hosts_follow_step_undo_and_redo(&mut f, (208.0, 208.0), (108.0, 108.0));
}

#[test]
fn two_switched_instances_under_one_host_solve_it_once_for_the_step() {
    fn solves_for_the_step(instances_in_host: usize) -> (usize, f32) {
        let mut f = build();
        for _ in 1..instances_in_host {
            let small = f.d.get(f.hug_top).unwrap().children.first().map(|&c| instance_component(f.d.get(c).unwrap()).unwrap()).unwrap();
            create_instance(&mut f.d, small, f.hug_top, Vec2::new(0.0, 0.0)).apply(&mut f.d).unwrap();
            let extra = last_child(&f.d, f.hug_top);
            let bind = bind_prop_variable(&mut f.d, extra, "Size", Some("size")).unwrap();
            f.h.exec(&mut f.d, bind).unwrap();
        }
        f.d.relayout_subtree(f.stage);
        crate::layout_apply::reset_solve_calls();
        set_mode(&mut f, "dark");
        (crate::layout_apply::solve_calls(), width(&f, f.hug_top))
    }
    let (one, one_width) = solves_for_the_step(1);
    let (two, two_width) = solves_for_the_step(2);
    assert_eq!((one_width, two_width), (208.0, 408.0), "both instances are laid out in the host");
    assert!(one > 0, "the step solves the chain it resized");
    assert_eq!(two, one, "a second switched instance under the same host adds no solve");
}

#[test]
fn grouped_bound_variant_resync_hugs_through_nested_groups_and_undo_redo() {
    let mut f = build();
    let outer = f.d.alloc_id();
    let mut outer_node = Node::new(outer, NodeKind::Group);
    outer_node.size = Vec2::new(108.0, 48.0);
    let outer = f.d.insert(f.hug_top, outer_node).unwrap();
    let inner = f.d.alloc_id();
    let mut inner_node = Node::new(inner, NodeKind::Group);
    inner_node.size = Vec2::new(108.0, 48.0);
    let inner = f.d.insert(outer, inner_node).unwrap();
    f.d.reparent(f.top, inner, 0).unwrap();
    let mut offset = f.d.get(f.top).unwrap().clone();
    offset.transform.tx = 17.0;
    f.d.get_mut(f.top).unwrap().transform.tx = 17.0;
    f.d.relayout_subtree(f.stage);
    let original_child = (f.d.get(f.top).unwrap().transform, f.d.get(f.top).unwrap().size);
    let initial_host = width(&f, f.hug_top);
    assert!(initial_host >= 108.0);
    let nodes_before = f.d.nodes.clone();

    set_mode(&mut f, "dark");
    assert_eq!(shown(&f, f.top), LARGE);
    assert_eq!(width(&f, outer), 200.0);
    assert_eq!(width(&f, inner), 200.0);
    assert!(width(&f, f.hug_top) > initial_host);
    let nodes_after = f.d.nodes.clone();

    assert!(f.h.undo(&mut f.d).unwrap());
    assert_eq!(shown(&f, f.top), SMALL);
    assert_eq!((f.d.get(f.top).unwrap().transform, f.d.get(f.top).unwrap().size), original_child, "undo restores asymmetric child geometry");
    assert_eq!(width(&f, f.hug_top), initial_host);
    assert_eq!(f.d.nodes, nodes_before, "undo restores the whole document node map");
    assert!(f.h.redo(&mut f.d).unwrap());
    assert_eq!((width(&f, outer), width(&f, inner)), (200.0, 200.0));
    assert!(width(&f, f.hug_top) > initial_host);
    assert_eq!(f.d.nodes, nodes_after, "redo restores the whole document node map");
}

#[test]
fn direct_variant_swap_inside_a_root_group_terminates_and_refits_the_group() {
    let mut f = build();
    let page = f.d.get(f.stage).and_then(|n| n.parent).expect("stage page");
    let group = f.d.alloc_id();
    f.d.insert(page, Node::new(group, NodeKind::Group)).unwrap();
    f.d.reparent(f.top, group, 0).unwrap();

    crate::changelog_features::swap_instance_variant(&mut f.d, f.top, "Size", "Large").unwrap();

    assert_eq!(shown(&f, f.top), LARGE);
    assert_eq!(width(&f, group), 200.0, "the root group is refit after the swap");
}

#[test]
fn ordinary_group_edit_preserves_imported_stored_bounds() {
    let mut f = build();
    let group = f.d.alloc_id();
    let mut group_node = Node::new(group, NodeKind::Group);
    group_node.size = Vec2::new(300.0, 80.0);
    let group = f.d.insert(f.hug_quiet, group_node).unwrap();
    f.d.reparent(f.quiet, group, 0).unwrap();
    f.d.relayout_subtree(f.stage);
    assert_eq!(width(&f, group), 300.0);
    let group_before = f.d.get(group).unwrap().clone();
    let child_before = f.d.get(f.quiet).unwrap().clone();
    let host_before = f.d.get(f.hug_quiet).unwrap().clone();
    let nodes_before = f.d.nodes.clone();

    let mut child = f.d.get(f.quiet).unwrap().clone();
    child.size.x = 120.0;
    f.h.exec(&mut f.d, Command::Update { node: Box::new(child) }).unwrap();
    let nodes_after_resize = f.d.nodes.clone();
    assert_eq!(f.d.get(group).unwrap(), &group_before, "ordinary resize must not refit stored group bounds");
    assert!(f.h.undo(&mut f.d).unwrap());
    assert_eq!(f.d.get(group).unwrap(), &group_before, "undo must restore the whole group node");
    assert_eq!(f.d.get(f.quiet).unwrap(), &child_before, "undo must restore the whole child node");
    assert_eq!(f.d.get(f.hug_quiet).unwrap(), &host_before, "undo must restore the whole hug host node");
    assert_eq!(f.d.nodes, nodes_before, "undo must restore the whole document node map");
    assert!(f.h.redo(&mut f.d).unwrap());
    assert_eq!(f.d.nodes, nodes_after_resize, "redo must restore the whole document node map");
    assert!(f.h.undo(&mut f.d).unwrap());
    assert_eq!(f.d.nodes, nodes_before);

    let mut moved_child = f.d.get(f.quiet).unwrap().clone();
    moved_child.transform.tx = 24.0;
    moved_child.transform.ty = 12.0;
    f.h.exec(&mut f.d, Command::Update { node: Box::new(moved_child) }).unwrap();
    let nodes_after_move = f.d.nodes.clone();
    assert_eq!(f.d.get(group).unwrap(), &group_before, "transform-only move must not refit stored group bounds");
    assert!(f.h.undo(&mut f.d).unwrap());
    assert_eq!(f.d.nodes, nodes_before, "undo of transform-only move restores the whole document node map");
    assert!(f.h.redo(&mut f.d).unwrap());
    assert_eq!(f.d.nodes, nodes_after_move, "redo of transform-only move restores the whole document node map");
    assert!(f.h.undo(&mut f.d).unwrap());
    assert_eq!(f.d.nodes, nodes_before);

    // Also enter `apply_prop_resync_and_relayout` directly with a bound non-sizing property on an instance
    // inside `group`: `collect_update_ids` runs on the resync inverse and leaves the imported group bounds alone.
    let small = instance_component(f.d.get(f.top).unwrap()).unwrap();
    let set = f.d.get(small).and_then(|n| n.parent).unwrap();
    add_prop(&mut f.d, set, ComponentPropDef { name: "Label".into(), kind: PropKind::Text, default: PropValue::Text("A".into()) }).apply(&mut f.d).unwrap();
    let lbl = f.d.alloc_id();
    f.d.insert(small, Node::text(lbl, Rect::new(0.0, 0.0, 40.0, 16.0), "A", TextStyle::default())).unwrap();
    bind_prop(&mut f.d, lbl, OverrideField::Characters, Some("Label")).apply(&mut f.d).unwrap();
    f.d.variable_collections.get_mut("theme").unwrap().variables.push("label_var".into());
    f.d.variables.insert("label_var".into(), string_variable("label_var", "Hello", "World"));
    create_instance(&mut f.d, small, group, Vec2::new(10.0, 10.0)).apply(&mut f.d).unwrap();
    let quiet_inst = last_child(&f.d, group);
    let bind_lbl = bind_prop_variable(&mut f.d, quiet_inst, "Label", Some("label_var")).unwrap();
    f.h.exec(&mut f.d, bind_lbl).unwrap();
    let group_with_inst = f.d.get(group).unwrap().clone();
    assert_eq!(group_with_inst.size, Vec2::new(300.0, 80.0));
    let before_resync = f.d.nodes.clone();

    set_mode(&mut f, "dark");
    let after_resync = f.d.nodes.clone();
    assert_eq!(f.d.get(group).unwrap(), &group_with_inst, "non-sizing bound resync inside imported group preserves stored group bounds");
    assert!(f.h.undo(&mut f.d).unwrap());
    assert_eq!(f.d.nodes, before_resync, "undo after non-sizing resync restores the whole document node map");
    assert!(f.h.redo(&mut f.d).unwrap());
    assert_eq!(f.d.nodes, after_resync, "redo after non-sizing resync restores the whole document node map");
}

/// A hugging frame (4 px padding) at `x` on the stage that holds `levels` nested groups around one bound instance.
fn hug_around_groups(f: &mut Fx, x: f32, levels: usize) -> (NodeId, NodeId, Vec<NodeId>) {
    let host = hugging_frame(&mut f.d, f.stage, x, 4.0);
    let mut parent = host;
    let mut groups = Vec::new();
    for _ in 0..levels {
        let id = f.d.alloc_id();
        parent = f.d.insert(parent, Node::new(id, NodeKind::Group)).unwrap();
        groups.push(parent);
    }
    let small = instance_component(f.d.get(f.top).unwrap()).unwrap();
    create_instance(&mut f.d, small, parent, Vec2::ZERO).apply(&mut f.d).unwrap();
    let inst = last_child(&f.d, parent);
    let bind = bind_prop_variable(&mut f.d, inst, "Size", Some("size")).unwrap();
    f.h.exec(&mut f.d, bind).unwrap();
    // Fit the groups to their content the way the editor leaves them, innermost first.
    for g in groups.iter().rev() {
        let size = f.d.get(*f.d.get(*g).unwrap().children.first().unwrap()).unwrap().size;
        f.d.get_mut(*g).unwrap().size = size;
    }
    f.d.relayout_subtree(host);
    (host, inst, groups)
}

fn wrap_in_groups(d: &mut Document, host: NodeId, inner_child: NodeId, depth: usize) -> Vec<NodeId> {
    let sz = d.get(inner_child).unwrap().size;
    d.get_mut(inner_child).unwrap().transform = Affine::IDENTITY;
    let mut groups = Vec::with_capacity(depth);
    let mut parent = host;
    for _ in 0..depth {
        let g = d.alloc_id();
        let mut g_node = Node::new(g, NodeKind::Group);
        g_node.size = sz;
        let g = d.insert(parent, g_node).unwrap();
        groups.push(g);
        parent = g;
    }
    d.reparent(inner_child, parent, 0).unwrap();
    groups
}

#[test]
fn a_group_between_the_instance_and_the_hug_host_does_not_drop_the_chain() {
    for levels in [1, 2] {
        let mut f = build();
        let (host, inst, groups) = hug_around_groups(&mut f, 900.0 + levels as f32 * 100.0, levels);
        let nested_groups = wrap_in_groups(&mut f.d, f.hug_nested, f.w, levels);
        f.d.relayout_subtree(f.stage);
        assert_eq!((shown(&f, inst), width(&f, host), width(&f, f.hug_nested)), (SMALL.to_string(), 108.0, 108.0), "{levels} group(s): precondition");
        let before_nodes = f.d.nodes.clone();

        set_mode(&mut f, "dark");
        assert_eq!(shown(&f, inst), LARGE, "{levels} group(s)");
        assert_eq!((width(&f, inst), width(&f, f.copy), width(&f, f.w)), (200.0, 200.0, 200.0), "{levels} group(s): instances");
        for &g in groups.iter().chain(nested_groups.iter()) {
            assert_eq!(width(&f, g), 200.0, "{levels} group(s): the group around it fits the new size");
        }
        assert_eq!((width(&f, host), width(&f, f.hug_nested)), (208.0, 208.0), "{levels} group(s): the host hugs the switched instance in the step");
        let after_nodes = f.d.nodes.clone();

        assert!(f.h.undo(&mut f.d).unwrap());
        assert_eq!((width(&f, inst), width(&f, f.copy), width(&f, f.w)), (100.0, 100.0, 100.0), "{levels} group(s): undo, instances");
        for &g in groups.iter().chain(nested_groups.iter()) {
            assert_eq!(width(&f, g), 100.0, "{levels} group(s): undo, the group");
        }
        assert_eq!((width(&f, host), width(&f, f.hug_nested)), (108.0, 108.0), "{levels} group(s): undo");
        assert_eq!(f.d.nodes, before_nodes, "{levels} group(s): undo restores the whole document node map");

        assert!(f.h.redo(&mut f.d).unwrap());
        assert_eq!((width(&f, inst), width(&f, f.copy), width(&f, f.w)), (200.0, 200.0, 200.0), "{levels} group(s): redo, instances");
        for &g in groups.iter().chain(nested_groups.iter()) {
            assert_eq!(width(&f, g), 200.0, "{levels} group(s): redo, the group");
        }
        assert_eq!((width(&f, host), width(&f, f.hug_nested)), (208.0, 208.0), "{levels} group(s): redo");
        assert_eq!(f.d.nodes, after_nodes, "{levels} group(s): redo restores the whole document node map");
    }
}

#[test]
fn a_group_that_keeps_its_size_stops_the_climb_and_leaves_the_host_alone() {
    let mut f = build();
    let (host, inst, groups) = hug_around_groups(&mut f, 1100.0, 1);
    let g = groups[0];
    let rail = f.d.alloc_id();
    f.d.insert(g, Node::frame(rail, Rect::new(0.0, 0.0, 200.0, 40.0))).unwrap();
    f.d.get_mut(g).unwrap().size = Vec2::new(200.0, 40.0);
    f.d.remove(f.hug_top).unwrap();
    f.d.remove(f.hug_nested).unwrap();
    f.d.relayout_subtree(f.stage);
    let host_before = (width(&f, host), f.d.get(host).unwrap().transform.tx);
    assert_eq!(host_before.0, 208.0);

    let opened = f.d.layout_epoch();
    crate::layout_apply::reset_solve_calls();
    set_mode(&mut f, "dark");
    assert_eq!(shown(&f, inst), LARGE);
    assert_eq!(width(&f, g), 200.0, "the sibling rail holds the group at 200 wide");
    assert_eq!(crate::layout_apply::solve_calls(), 0, "a group whose bounds held solves no frame above it");
    assert_eq!(f.d.layout_epoch(), opened, "a fitted group whose bounds held does not advance layout_epoch");
    assert_eq!((width(&f, host), f.d.get(host).unwrap().transform.tx), host_before);
}

#[test]
fn hand_variant_pick_and_instance_swap_refit_hug_host_through_groups() {
    let mut f = build();
    let (host, inst, groups) = hug_around_groups(&mut f, 1100.0, 2);
    assert_eq!(width(&f, host), 108.0);

    let large_comp = crate::changelog_features::swap_instance_variant(&mut f.d, inst, "Size", "Large").unwrap();
    assert_eq!(shown(&f, inst), LARGE);
    for &g in &groups {
        assert_eq!(width(&f, g), 200.0);
    }
    assert_eq!(width(&f, host), 208.0, "hand variant pick refits nested groups and hug host");

    let small_comp = f.d.get(large_comp).and_then(|n| n.parent).map(|set| f.d.get(set).unwrap().children[0]).unwrap();
    crate::changelog_features::swap_instance_to_component(&mut f.d, inst, small_comp).unwrap();
    assert_eq!(shown(&f, inst), SMALL);
    for &g in &groups {
        assert_eq!(width(&f, g), 100.0);
    }
    assert_eq!(width(&f, host), 108.0, "instance swap refits nested groups and hug host");

    let alt_host = hugging_frame(&mut f.d, f.stage, 1400.0, 4.0);
    let (outer_group, mid_hug, inner_group, alt_inst) = alternating_group_hug_chain(&mut f, alt_host);
    f.d.relayout_subtree(alt_host);
    assert_eq!((width(&f, inner_group), width(&f, mid_hug), width(&f, outer_group), width(&f, alt_host)), (100.0, 108.0, 108.0, 116.0));
    crate::changelog_features::swap_instance_variant(&mut f.d, alt_inst, "Size", "Large").unwrap();
    assert_eq!((width(&f, inner_group), width(&f, mid_hug), width(&f, outer_group), width(&f, alt_host)), (200.0, 208.0, 208.0, 216.0));
    crate::changelog_features::swap_instance_to_component(&mut f.d, alt_inst, small_comp).unwrap();
    assert_eq!((width(&f, inner_group), width(&f, mid_hug), width(&f, outer_group), width(&f, alt_host)), (100.0, 108.0, 108.0, 116.0));
}

/// Builds a 4-level subtree `outer_group > mid_hug (4 px padding) > inner_group > bound instance` under `container`.
fn alternating_group_hug_chain(f: &mut Fx, container: NodeId) -> (NodeId, NodeId, NodeId, NodeId) {
    let outer_group = f.d.alloc_id();
    let mut og = Node::new(outer_group, NodeKind::Group);
    og.size = Vec2::new(108.0, 48.0);
    let outer_group = f.d.insert(container, og).unwrap();

    let mid_hug = hugging_frame(&mut f.d, outer_group, 0.0, 4.0);

    let inner_group = f.d.alloc_id();
    let mut ig = Node::new(inner_group, NodeKind::Group);
    ig.size = Vec2::new(100.0, 40.0);
    let inner_group = f.d.insert(mid_hug, ig).unwrap();

    let small = instance_component(f.d.get(f.top).unwrap()).unwrap();
    create_instance(&mut f.d, small, inner_group, Vec2::ZERO).apply(&mut f.d).unwrap();
    let inst = last_child(&f.d, inner_group);
    let bind = bind_prop_variable(&mut f.d, inst, "Size", Some("size")).unwrap();
    f.h.exec(&mut f.d, bind).unwrap();
    f.d.relayout_subtree(mid_hug);
    f.d.get_mut(outer_group).unwrap().size = f.d.get(mid_hug).unwrap().size;
    (outer_group, mid_hug, inner_group, inst)
}

#[test]
fn alternating_groups_and_auto_layout_frames_climb_inside_out_and_undo_redo() {
    let mut f = build();
    // 1. Five-level chain: hug_host > outer_group > mid_hug > inner_group > inst
    let hug_host = hugging_frame(&mut f.d, f.stage, 1100.0, 4.0);
    let (outer_group, mid_hug, inner_group, inst) = alternating_group_hug_chain(&mut f, hug_host);
    f.d.relayout_subtree(hug_host);

    // 2. Plain-frame chain: plain_frame > plain_outer_group > plain_mid_hug > plain_inner_group > plain_inst
    let plain_id = f.d.alloc_id();
    let plain_frame = f.d.insert(f.stage, Node::frame(plain_id, Rect::new(1400.0, 0.0, 400.0, 100.0))).unwrap();
    let (plain_outer_group, plain_mid_hug, plain_inner_group, plain_inst) = alternating_group_hug_chain(&mut f, plain_frame);

    assert_eq!(
        (width(&f, inner_group), width(&f, mid_hug), width(&f, outer_group), width(&f, hug_host)),
        (100.0, 108.0, 108.0, 116.0),
        "precondition: five-level chain starts fitted at Small"
    );
    assert_eq!(
        (width(&f, plain_inner_group), width(&f, plain_mid_hug), width(&f, plain_outer_group), width(&f, plain_frame)),
        (100.0, 108.0, 108.0, 400.0),
        "precondition: plain-frame chain starts fitted at Small"
    );
    let before_nodes = f.d.nodes.clone();

    set_mode(&mut f, "dark");
    assert_eq!((shown(&f, inst), shown(&f, plain_inst)), (LARGE.to_string(), LARGE.to_string()));
    assert_eq!(
        (width(&f, inner_group), width(&f, mid_hug), width(&f, outer_group), width(&f, hug_host)),
        (200.0, 208.0, 208.0, 216.0),
        "step: five-level chain refits inner_group -> mid_hug -> outer_group -> hug_host"
    );
    assert_eq!(
        (width(&f, plain_inner_group), width(&f, plain_mid_hug), width(&f, plain_outer_group), width(&f, plain_frame)),
        (200.0, 208.0, 208.0, 400.0),
        "step: plain-frame chain refits up to outer_group and leaves plain_frame at 400"
    );
    let after_nodes = f.d.nodes.clone();

    assert!(f.h.undo(&mut f.d).unwrap());
    assert_eq!(
        (width(&f, inner_group), width(&f, mid_hug), width(&f, outer_group), width(&f, hug_host)),
        (100.0, 108.0, 108.0, 116.0),
        "undo: five-level chain returns to old bounds"
    );
    assert_eq!(f.d.nodes, before_nodes, "undo restores the whole document node map");

    assert!(f.h.redo(&mut f.d).unwrap());
    assert_eq!(
        (width(&f, inner_group), width(&f, mid_hug), width(&f, outer_group), width(&f, hug_host)),
        (200.0, 208.0, 208.0, 216.0),
        "redo: five-level chain returns to new bounds"
    );
    assert_eq!(f.d.nodes, after_nodes, "redo restores the whole document node map");
}

#[test]
fn hand_pick_and_instance_swap_through_history_exec_refit_groups_and_hug_host_and_undo_redo() {
    let mut f = build();
    let (host, inst, groups) = hug_around_groups(&mut f, 1100.0, 2);
    assert_eq!(width(&f, host), 108.0);
    let before_pick = f.d.nodes.clone();

    // 1. Hand variant pick on a bound property through `History::exec` (the `App::set_instance_prop` entry point).
    let pick_cmd = set_instance_prop(&mut f.d, inst, "Size", Some(PropValue::Variant("Large".into())));
    f.h.exec(&mut f.d, pick_cmd).unwrap();
    assert_eq!(shown(&f, inst), LARGE);
    for &g in &groups {
        assert_eq!(width(&f, g), 200.0, "group refits in the same History::exec step as set_instance_prop");
    }
    assert_eq!(width(&f, host), 208.0, "hug host resizes in the same History::exec step as set_instance_prop");
    let after_pick = f.d.nodes.clone();

    assert!(f.h.undo(&mut f.d).unwrap());
    assert_eq!(f.d.nodes, before_pick, "undo of set_instance_prop restores the whole document node map");
    assert!(f.h.redo(&mut f.d).unwrap());
    assert_eq!(f.d.nodes, after_pick, "redo of set_instance_prop restores the whole document node map");

    // 2. Instance swap on a bound property through `History::exec` (the `App::swap_instance` entry point).
    let large_comp = instance_component(f.d.get(inst).unwrap()).unwrap();
    let small_comp = f.d.get(large_comp).and_then(|n| n.parent).map(|set| f.d.get(set).unwrap().children[0]).unwrap();
    let swap_cmd = swap_instance(&mut f.d, inst, small_comp);
    f.h.exec(&mut f.d, swap_cmd).unwrap();
    assert_eq!(shown(&f, inst), SMALL);
    for &g in &groups {
        assert_eq!(width(&f, g), 100.0, "group refits in the same History::exec step as swap_instance");
    }
    assert_eq!(width(&f, host), 108.0, "hug host resizes in the same History::exec step as swap_instance");
    let after_swap = f.d.nodes.clone();

    assert!(f.h.undo(&mut f.d).unwrap());
    assert_eq!(f.d.nodes, after_pick, "undo of swap_instance restores the whole document node map");
    assert!(f.h.redo(&mut f.d).unwrap());
    assert_eq!(f.d.nodes, after_swap, "redo of swap_instance restores the whole document node map");
}
