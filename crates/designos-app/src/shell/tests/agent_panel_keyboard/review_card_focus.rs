//! #922: Revert, Keep and Dismiss hand the keyboard back to the composer when it had the caret.

use super::*;
use designos_doc::live_patch::{LiveOp, LivePatch};

/// The agent renames Hero: one applied edit, so the panel shows the turn's review card with Revert and Keep.
fn agent_renames_hero(s: &mut Sim) {
    let hero = s.by_name("Hero");
    let mut set = serde_json::Map::new();
    set.insert("name".into(), "Hero (agent)".into());
    crate::live_patch_apply::apply_live_patches(&mut s.shell.app, &[LivePatch::new(vec![LiveOp::Update { id: hero, set, unset: Vec::new() }])]);
    s.frame(vec![]);
    s.frame(vec![]);
}

/// A window with the review card shown and the typing caret in the composer.
fn sim_with_review_card_and_focused_composer() -> Sim {
    let mut s = agent_sim();
    agent_renames_hero(&mut s);
    assert!(s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", "agent-turn-revert".to_string())))).is_some(), "the review card is on screen");
    let input = s.rect_of("agents-composer-input");
    s.click(input.center());
    assert_eq!(s.ctx.memory(|m| m.focused()), Some(crate::agent_a11y::composer_id()), "precondition: the composer has the caret");
    s
}

/// A capital `S`, as the QA report typed it: egui delivers the key and its text together.
fn type_capital_s(s: &mut Sim) {
    s.mods = Modifiers::SHIFT;
    s.one_frame().key(Key::S).text("S").send();
    s.mods = Modifiers::NONE;
    s.frame(vec![]);
}

fn assert_typing_went_to_the_composer(s: &Sim, draft: &str, after: &str) {
    let (typed, tool, caret) = (s.app().terminal.composer_draft().to_string(), s.app().tool, s.ctx.memory(|m| m.focused()));
    assert!(
        typed == draft && tool == Tool::Move && caret == Some(crate::agent_a11y::composer_id()),
        "{after}: typed letters belong in the composer and no canvas tool may change; composer text {typed:?} (want {draft:?}), tool {tool:?} (want Move), caret on composer: {}",
        caret == Some(crate::agent_a11y::composer_id())
    );
}

#[test]
fn revert_hands_the_keyboard_back_to_the_composer() {
    let mut s = sim_with_review_card_and_focused_composer();
    let revert = s.rect_of("agent-turn-revert");
    s.click(revert.center());
    assert_eq!(s.app().doc.get(s.by_name("Hero")).map(|n| n.name.clone()).as_deref(), Some("Hero"), "precondition: Revert took the agent's edit back");
    type_capital_s(&mut s);
    assert_typing_went_to_the_composer(&s, "S", "after Revert");

    // The card now reads "Reverted" with a Dismiss button; it hands the keyboard back as well.
    let dismiss = s.rect_of("agent-turn-dismiss");
    s.click(dismiss.center());
    type_capital_s(&mut s);
    assert_typing_went_to_the_composer(&s, "SS", "after Dismiss");
    finish(s);
}

#[test]
fn keep_hands_the_keyboard_back_to_the_composer() {
    let mut s = sim_with_review_card_and_focused_composer();
    let keep = s.rect_of("agent-turn-keep");
    s.click(keep.center());
    assert_eq!(
        s.app().doc.get(s.by_name("Hero (agent)")).map(|n| n.name.clone()).as_deref(),
        Some("Hero (agent)"),
        "precondition: Keep leaves the edit in place"
    );
    type_capital_s(&mut s);
    assert_typing_went_to_the_composer(&s, "S", "after Keep");
    finish(s);
}

#[test]
fn revert_does_not_steal_the_keyboard_from_the_canvas() {
    let mut s = agent_sim();
    agent_renames_hero(&mut s);
    // The user last clicked the canvas, so the composer never had the caret.
    s.click(s.screen(900.0, 700.0));
    assert_ne!(s.ctx.memory(|m| m.focused()), Some(crate::agent_a11y::composer_id()), "precondition: the composer has no caret");
    let revert = s.rect_of("agent-turn-revert");
    s.click(revert.center());
    assert_ne!(s.ctx.memory(|m| m.focused()), Some(crate::agent_a11y::composer_id()), "Revert must not move the caret into a composer that did not have it");
    finish(s);
}
