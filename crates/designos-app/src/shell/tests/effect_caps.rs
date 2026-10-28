//! The Effects "+" menu stops at 8 drop shadows, 8 inner shadows, 1 layer blur and 1 background blur per
//! layer, while layers that already hold more (imported files) keep every effect.

use super::*;
use crate::panels::effects::{at_cap, cap_of, default_effect};
use designos_doc::Effect;

const INNER: usize = 0;
const DROP: usize = 1;
const LAYER_BLUR: usize = 2;
const BG_BLUR: usize = 3;
const NOISE: usize = 4;

fn kind_count(s: &Sim, id: NodeId, kind: usize) -> usize {
    s.app().doc.get(id).unwrap().effects.iter().filter(|e| crate::panels::effects::kind_of(e) == kind).count()
}

fn sim_with(effects: Vec<Effect>) -> (Sim, NodeId) {
    let mut s = Sim::with(Options { select: Some("Welcome".into()), ..Default::default() });
    let id = s.by_name("Welcome");
    s.shell.app.edit_nodes(&[id], |n| n.effects = effects.clone());
    s.scroll_inspector_down();
    (s, id)
}

/// Opens the "+" menu and clicks its `row`-th row.
/// Returns true when the row was disabled: a click on it leaves the menu open, so the + closes it.
fn add(s: &mut Sim, row: usize) -> bool {
    let mut r = s.rect_of("fx-add");
    if r.top() < 200.0 {
        // A long effect list scrolls the "+" under the panel header: bring it back into view.
        s.shell.app.props.scroll -= 200.0 - r.top();
        s.frame(vec![]);
        s.frame(vec![]);
        r = s.rect_of("fx-add");
    }
    s.click(r.center());
    s.frame(vec![]);
    assert!(s.app().props.fx_menu, "the + opens its menu");
    // Near the bottom of the screen the menu is shifted up, so read where it landed.
    let menu = s.ctx.memory(|m| m.area_rect(egui::Id::new("fx-add-menu").with(("lvl", 0usize)))).expect("the menu is open");
    let top = menu.top() + theme::MENU_PADDING + row as f32 * theme::MENU_ITEM_HEIGHT;
    s.click(Pos2::new(menu.center().x, top + theme::MENU_ITEM_HEIGHT / 2.0));
    s.frame(vec![]);
    let blocked = s.app().props.fx_menu;
    if blocked {
        let plus = s.rect_of("fx-add");
        s.click(plus.center());
        assert!(!s.app().props.fx_menu, "a click on the + closes the menu");
    }
    s.scroll_inspector_down();
    blocked
}

fn full_of(kind: usize) -> Vec<Effect> {
    let (max, _) = cap_of(kind).unwrap();
    (0..max).map(|i| default_effect(kind, i as u32)).collect()
}

#[test]
fn each_capped_type_stops_at_its_limit_and_keeps_other_types_available() {
    for (kind, max) in [(INNER, 8), (DROP, 8), (LAYER_BLUR, 1), (BG_BLUR, 1)] {
        let (mut s, id) = sim_with(Vec::new());
        for _ in 0..max {
            assert!(!add(&mut s, kind), "type {kind} is open below its cap");
        }
        assert_eq!(kind_count(&s, id, kind), max, "type {kind} fills up to {max}");
        assert!(add(&mut s, kind), "type {kind} is disabled at {max}");
        assert_eq!(kind_count(&s, id, kind), max, "type {kind} stays at {max}");
        assert!(!add(&mut s, NOISE));
        assert_eq!(s.app().doc.get(id).unwrap().effects.len(), max + 1, "Noise is still available at the cap of type {kind}");
    }
}

#[test]
fn deleting_under_the_cap_enables_the_type_again_and_undo_works_at_the_cap() {
    let (mut s, id) = sim_with(full_of(DROP));
    assert!(add(&mut s, DROP), "disabled at the cap");
    assert_eq!(kind_count(&s, id, DROP), 8);
    let minus = s.rect_of("fx-minus-7");
    s.click(minus.center());
    assert_eq!(kind_count(&s, id, DROP), 7);
    s.scroll_inspector_down();
    assert!(!add(&mut s, DROP), "enabled again under the cap");
    assert_eq!(kind_count(&s, id, DROP), 8, "the 8th drop shadow can be added again");
    s.cmd_key(Key::Z);
    assert_eq!(kind_count(&s, id, DROP), 7, "undo removes the add that reached the cap");
    s.cmd_key(Key::Z);
    assert_eq!(kind_count(&s, id, DROP), 8, "undo restores the deleted one");
}

#[test]
fn a_layer_with_twelve_drop_shadows_keeps_all_of_them() {
    let twelve: Vec<Effect> = (0..12).map(|i| default_effect(DROP, i)).collect();
    let (mut s, id) = sim_with(twelve.clone());
    assert!(at_cap(&s.app().doc.get(id).unwrap().effects, DROP));
    assert!(add(&mut s, DROP), "disabled at twelve");
    assert_eq!(s.app().doc.get(id).unwrap().effects, twelve, "a blocked add leaves the 12 alone");
    let json = s.app().doc.to_json().unwrap();
    let back = designos_doc::Document::from_json(&json).unwrap();
    let id2 = back.nodes.values().find(|n| n.name == "Welcome").unwrap().id;
    assert_eq!(back.get(id2).unwrap().effects, twelve, "save and load keep all 12");
}
