//! The variant property rows of a nested instance, listed on the outer instance's panel, show the variable that drives them and
//! offer the bind control, like the rows of the outer instance. A bind or an unbind made there writes what the agent tools write
//! (`designos_update_node` with `prop_vars`), in one undo step.
//!
//! Scene: a set `Button` with the variant properties `Size` (Small / Large) and `State` (Default / Hover), four variants. A
//! wrapper main holds two nested instances of it, `Leading` and `Trailing`, both exposing their properties on the outer
//! instance `W`. Inside the wrapper main, `Leading` binds `Size` to the string variable `Size` (light Small, dark Large).
//! `State` is a string variable too (light Default, dark Hover). `Trailing` binds nothing.

use super::*;
use designos_doc::OverrideField;
use serde_json::json;

fn recorded_rect(s: &Sim, name: &str) -> Option<Rect> {
    s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", name.to_string()))))
}

fn clear_recorded_rect(s: &mut Sim, name: &str) {
    s.ctx.data_mut(|d| d.remove::<Rect>(egui::Id::new(("test-rect", name.to_string()))));
}

fn child_named(d: &designos_doc::Document, parent: NodeId, name: &str) -> NodeId {
    *d.get(parent).unwrap().children.iter().find(|c| d.get(**c).unwrap().name == name).unwrap_or_else(|| panic!("no child {name}"))
}

struct Scene {
    s: Sim,
    wrapper: NodeId,
    /// The outer instance of the wrapper.
    w: NodeId,
    /// The nested instances inside `w`.
    lead: NodeId,
    trail: NodeId,
    coll: String,
    dark: String,
}

impl Scene {
    fn doc(&self) -> &designos_doc::Document {
        &self.s.app().doc
    }

    /// The variant `inst` shows, as its name.
    fn shown(&self, inst: NodeId) -> String {
        let d = self.doc();
        d.get(designos_doc::instance_component(d.get(inst).unwrap()).unwrap()).unwrap().name.clone()
    }

    fn select_outer(&mut self) {
        self.s.shell.app.selection = vec![self.w];
        self.s.frame(vec![]);
        self.s.frame(vec![]);
    }

    fn set_mode(&mut self, mode: &str) {
        let (w, coll) = (self.w, self.coll.clone());
        self.s.shell.app.set_node_mode(&[w], &coll, Some(mode));
        self.s.frame(vec![]);
    }
}

fn scene() -> Scene {
    let mut s = Sim::new();
    let page = s.app().page;
    let d = &mut s.shell.app.doc;
    let mut mains = Vec::new();
    for (i, name) in ["Size=Small, State=Default", "Size=Large, State=Default", "Size=Small, State=Hover", "Size=Large, State=Hover"].into_iter().enumerate() {
        let id = d.alloc_id();
        let mut v = designos_doc::Node::frame(id, designos_doc::Rect::new(40.0 + i as f32 * 130.0, 700.0, 100.0 + (i % 2) as f32 * 20.0, 40.0));
        v.name = name.into();
        let v = d.insert(page, v).unwrap();
        create_component_of(d, v);
        mains.push(v);
    }
    designos_doc::combine_as_variants(d, &mains).apply(d).unwrap();
    let id = d.alloc_id();
    let mut wrapper = designos_doc::Node::frame(id, designos_doc::Rect::new(40.0, 800.0, 400.0, 100.0));
    wrapper.name = "Wrapper".into();
    let wrapper = d.insert(page, wrapper).unwrap();
    let mut nested = Vec::new();
    for (name, x) in [("Leading", 10.0), ("Trailing", 200.0)] {
        designos_doc::create_instance(d, mains[0], wrapper, DV::new(x, 10.0)).apply(d).unwrap();
        let n = *d.get(wrapper).unwrap().children.last().unwrap();
        d.get_mut(n).unwrap().name = name.into();
        nested.push(n);
    }
    create_component_of(d, wrapper);
    for &n in &nested {
        designos_doc::set_expose_nested_instance(d, n, true).apply(d).unwrap();
    }
    let mut store = designos_doc::VariableStore::from_doc(&s.app().doc);
    let theme = store.collections.values().find(|c| c.name == "Theme").expect("Theme collection").clone();
    let (light, dark) = (theme.modes[0].id.clone(), theme.modes[1].id.clone());
    for (name, l, dk) in [("Size", "Small", "Large"), ("State", "Default", "Hover")] {
        let var = store.add_variable(&theme.id, name, designos_doc::VariableKind::String).unwrap();
        store.set_value(&var, &light, designos_doc::VariableValue::String(l.into()));
        store.set_value(&var, &dark, designos_doc::VariableValue::String(dk.into()));
    }
    s.shell.app.commit_store(store, &[], &[]);
    let d = &mut s.shell.app.doc;
    let size_id = d.variables.values().find(|v| v.name == "Size").unwrap().id.clone();
    designos_doc::bind_prop_variable(d, nested[0], "Size", Some(&size_id)).unwrap().apply(d).unwrap();
    designos_doc::create_instance(d, wrapper, page, DV::new(900.0, 700.0)).apply(d).unwrap();
    let w = *d.get(page).unwrap().children.last().unwrap();
    let lead = child_named(d, w, "Leading");
    let trail = child_named(d, w, "Trailing");
    let mut sc = Scene { s, wrapper, w, lead, trail, coll: theme.id, dark };
    sc.select_outer();
    sc
}

fn create_component_of(d: &mut designos_doc::Document, node: NodeId) {
    designos_doc::create_component(d, node).apply(d).unwrap();
}

fn bound_key(row: &str) -> String {
    format!("bound:prop:{row}")
}

fn bind_key(row: &str) -> String {
    format!("bind:prop:{row}")
}

/// What `designos_update_node` with `prop_vars` does to a copy of `doc`, as the whole set of nodes.
fn agent_result(doc: &designos_doc::Document, node: NodeId, prop_vars: serde_json::Value) -> std::collections::BTreeMap<NodeId, designos_doc::Node> {
    let mut e = designos_mcp::engine::Engine { doc: doc.clone(), ..Default::default() };
    e.call("designos_update_node", json!({"node_id": node.0, "prop_vars": prop_vars})).expect("the agent tool binds");
    e.doc.nodes.clone()
}

#[test]
fn a_nested_variant_row_driven_by_a_variable_paints_the_pill_and_the_other_rows_offer_the_bind_control() {
    let sc = scene();
    let bound = recorded_rect(&sc.s, &bound_key("Leading/Size")).expect("Leading's Size is driven by the variable: the row paints the pill");
    assert!(bound.width() > 0.0);
    assert!(recorded_rect(&sc.s, &bind_key("Leading/Size")).is_none(), "a pill row carries no bind button");
    assert!(recorded_rect(&sc.s, &bound_key("Leading/State")).is_none(), "State is not bound, so no pill");
    assert!(recorded_rect(&sc.s, &bind_key("Leading/State")).is_some(), "State offers the bind control");
    assert!(recorded_rect(&sc.s, &bound_key("Trailing/Size")).is_none(), "Trailing binds nothing, so no pill");
    assert!(recorded_rect(&sc.s, &bind_key("Trailing/Size")).is_some(), "Trailing's Size offers the bind control");
    let texts = crate::lint::scan_shapes(&sc.s.shapes).0.into_iter().map(|t| t.text).collect::<Vec<_>>();
    assert!(texts.iter().any(|t| t == "Size"), "the pill names the variable: {texts:?}");
}

#[test]
fn binding_from_a_nested_row_writes_what_update_node_prop_vars_writes_and_leaves_the_other_row_alone() {
    let mut sc = scene();
    let before = sc.doc().clone();
    let expected = agent_result(&before, sc.trail, json!({"Size": "var:Theme/Size"}));

    let bind = recorded_rect(&sc.s, &bind_key("Trailing/Size")).expect("Trailing's Size offers the bind control");
    sc.s.click(bind.center());
    assert!(sc.s.app().vars.bind_picker.is_some(), "the bind control opens the variable picker");
    sc.s.frame(vec![]);
    sc.s.click(sc.s.rect_of("pick:Size").center());

    let key = designos_doc::prop_binding_key("Size");
    assert!(sc.doc().get(sc.trail).unwrap().variables.bindings.contains_key(&key), "the nested copy holds the binding");
    assert!(sc.doc().nodes == expected, "the bind is node for node what the agent tool writes");
    assert_eq!(sc.doc().get(sc.lead), before.get(sc.lead), "binding Trailing leaves Leading as it was");

    sc.s.frame(vec![]);
    assert!(recorded_rect(&sc.s, &bound_key("Trailing/Size")).is_some(), "the row now paints the pill");

    sc.s.cmd_key(Key::Z);
    assert!(sc.doc().nodes == before.nodes, "one undo takes the whole bind back, node for node");
}

#[test]
fn a_variable_bound_from_a_nested_row_stays_through_a_sync_of_the_wrapper_main_and_drives_the_variant() {
    let mut sc = scene();
    let bind = recorded_rect(&sc.s, &bind_key("Trailing/Size")).expect("Trailing's Size offers the bind control");
    sc.s.click(bind.center());
    sc.s.frame(vec![]);
    sc.s.click(sc.s.rect_of("pick:Size").center());
    let key = designos_doc::prop_binding_key("Size");
    assert_eq!(sc.shown(sc.trail), "Size=Small, State=Default", "light: the variable gives Small");

    let wrapper = sc.wrapper;
    sc.s.shell.app.edit_nodes(&[wrapper], |n| n.opacity = 0.9);
    assert!(sc.doc().get(sc.trail).unwrap().variables.bindings.contains_key(&key), "a sync of the wrapper main leaves the copy's own binding");
    let dark = sc.dark.clone();
    sc.set_mode(&dark);
    assert_eq!(sc.shown(sc.trail), "Size=Large, State=Default", "dark: the variable gives Large, and the copy follows it");
}

#[test]
fn unbinding_from_a_nested_pill_writes_what_update_node_null_writes_and_the_copy_stays_off_the_variable() {
    let mut sc = scene();
    let before = sc.doc().clone();
    let expected = agent_result(&before, sc.lead, json!({"Size": null}));
    let key = designos_doc::prop_binding_key("Size");
    assert!(before.get(sc.lead).unwrap().variables.bindings.contains_key(&key), "Leading inherits the main's binding");

    let pill = recorded_rect(&sc.s, &bound_key("Leading/Size")).expect("the pill");
    sc.s.mv(pill.center());
    sc.s.frame(vec![]);
    sc.s.click(egui::pos2(pill.right() - 13.0, pill.center().y));

    assert!(!sc.doc().get(sc.lead).unwrap().variables.bindings.contains_key(&key), "the copy holds no binding");
    assert!(sc.doc().nodes == expected, "the unbind is node for node what the agent tool writes");
    assert!(sc.doc().get(sc.lead).unwrap().component.overrides.contains(&OverrideField::MainComponent), "the copy is held on the variant it shows");

    // A sync of the wrapper main and a mode change leave the copy where it is.
    let wrapper = sc.wrapper;
    sc.s.shell.app.edit_nodes(&[wrapper], |n| n.opacity = 0.9);
    let shown = sc.shown(sc.lead);
    let dark = sc.dark.clone();
    sc.set_mode(&dark);
    assert_eq!(sc.shown(sc.lead), shown, "the unbound copy no longer follows the mode");
    assert!(!sc.doc().get(sc.lead).unwrap().variables.bindings.contains_key(&key), "a sync did not bring the main's binding back");
}

#[test]
fn a_hand_pick_on_a_nested_row_holds_that_property_only() {
    let mut sc = scene();
    assert_eq!(sc.shown(sc.lead), "Size=Small, State=Default");
    let state_row = recorded_rect(&sc.s, "prop:Leading/State").expect("Leading's State row");
    sc.s.click(state_row.center());
    sc.s.click(sc.s.rect_of("menu-row-0-1").center());
    assert_eq!(sc.shown(sc.lead), "Size=Small, State=Hover", "the pick switched State");
    let dark = sc.dark.clone();
    sc.set_mode(&dark);
    assert_eq!(sc.shown(sc.lead), "Size=Large, State=Hover", "Size still follows its variable; State keeps the pick");
    assert!(sc.doc().get(sc.lead).unwrap().variables.bindings.contains_key(&designos_doc::prop_binding_key("Size")), "the Size binding stayed");
}

#[test]
fn two_same_named_nested_rows_keep_their_own_pill_and_bind_control() {
    let mut sc = scene();
    // Both nested instances are called Leading, as in the outer instance's panel of a card with two badges.
    let (main_trail, trail) = (child_named(sc.doc(), sc.wrapper, "Trailing"), sc.trail);
    sc.s.shell.app.doc.get_mut(main_trail).unwrap().name = "Leading".into();
    sc.s.shell.app.doc.get_mut(trail).unwrap().name = "Leading".into();
    let other = format!("prop:{}:Leading/Size", trail.0);
    for name in [bound_key("Leading/Size"), format!("bound:{other}"), format!("bind:{other}"), bind_key("Leading/Size")] {
        clear_recorded_rect(&mut sc.s, &name);
    }
    sc.select_outer();
    assert!(recorded_rect(&sc.s, &bound_key("Leading/Size")).is_some(), "the first row paints its pill");
    assert!(recorded_rect(&sc.s, &format!("bound:{other}")).is_none(), "the second row, which binds nothing, paints none");
    assert!(recorded_rect(&sc.s, &format!("bind:{other}")).is_some(), "the second row offers the bind control under its own name");
}

/// Headless pictures of the exposed rows, written only when `DESIGNOS_1905_EVIDENCE_DIR` names a directory: Leading's `Size` as a
/// pill, the bind control on the other rows, then Trailing's `Size` after a bind through the picker.
#[test]
fn headless_capture_of_the_nested_rows_before_and_after_a_bind() {
    let Ok(dir) = std::env::var("DESIGNOS_1905_EVIDENCE_DIR") else { return };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).expect("evidence dir");
    let mut sc = scene();
    // A fresh font atlas, so every glyph the capture needs arrives in the texture deltas the capture collects.
    crate::theme::install_fonts(&sc.s.ctx);
    let mut atlas = std::collections::HashMap::new();
    let mut shot = |sc: &mut Scene, name: &str| {
        for _ in 0..2 {
            sc.s.t += 0.05;
            let mut raw = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 1088.0))),
                time: Some(sc.s.t),
                modifiers: sc.s.mods,
                events: vec![Event::PointerMoved(sc.s.pos)],
                ..Default::default()
            };
            let ctx = sc.s.ctx.clone();
            eframe::App::raw_input_hook(&mut sc.s.shell, &ctx, &mut raw);
            let shell = &mut sc.s.shell;
            let out = ctx.run_ui(raw, |ui| shell.draw(ui));
            super::typography::apply_textures_delta(&mut atlas, &out.textures_delta);
            sc.s.shapes = out.shapes;
        }
        let top = recorded_rect(&sc.s, "exposed-heading:Leading").expect("Leading heading");
        let bottom = recorded_rect(&sc.s, "prop:Trailing/State").expect("Trailing State row");
        let crop = Rect::from_min_max(egui::pos2(top.left() - 16.0, top.top() - 16.0), egui::pos2(top.right() + 16.0, bottom.bottom() + 16.0));
        let img = super::typography::rasterize_shapes(&sc.s.ctx, sc.s.shapes.clone(), &atlas, 1600, 1088);
        let tile = image::imageops::crop_imm(&img, crop.min.x as u32, crop.min.y as u32, crop.width() as u32, crop.height() as u32).to_image();
        let tile = image::imageops::resize(&tile, tile.width() * 3, tile.height() * 3, image::imageops::FilterType::Nearest);
        tile.save(dir.join(name)).expect("save capture");
    };
    shot(&mut sc, "1905-nested-rows-before.png");
    let bind = recorded_rect(&sc.s, &bind_key("Trailing/Size")).expect("Trailing's Size bind control");
    sc.s.click(bind.center());
    shot(&mut sc, "1905-nested-rows-picker-open.png");
    sc.s.click(sc.s.rect_of("pick:Size").center());
    shot(&mut sc, "1905-nested-rows-after-bind.png");
}
