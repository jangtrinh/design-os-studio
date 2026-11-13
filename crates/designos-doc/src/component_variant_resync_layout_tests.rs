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
    AutoLayout { direction: LayoutDirection::Horizontal, primary_sizing: Sizing::Hug, counter_sizing: Sizing::Hug, padding: [padding; 4], layout_version: 1, ..AutoLayout::default() }
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
