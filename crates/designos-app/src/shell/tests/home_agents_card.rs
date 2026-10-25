//! The Agents card of the Home resources view tells the truth. Its button says which panel it opens (the terminal dock
//! is gone, the button opens the Agents panel), and its description names the CLIs the panel supports and the MCP
//! server without a tool count that would drift.

use super::components::painted_strings;
use super::home::{home_sim, TmpDir};
use super::*;
use crate::home::Section;

const BUTTON: &str = "Open Agents";
const DESCRIPTION: &str = "Per-file Claude Code, Codex, Grok, Antigravity & Gemini CLI + the MCP server";

/// The Home screen on its Resources & Libraries section, where the card is drawn.
fn resources_sim() -> (Sim, TmpDir) {
    let (mut s, dir) = home_sim();
    s.shell.app.home.as_mut().expect("the home screen").section = Section::Resources;
    s.frame(vec![]);
    s.frame(vec![]);
    (s, dir)
}

#[test]
fn the_agents_card_pins_its_button_and_description() {
    let (s, _dir) = resources_sim();
    let texts = painted_strings(&s);
    assert!(texts.iter().any(|t| t == BUTTON), "the card button reads {BUTTON:?}: {texts:?}");
    assert!(texts.iter().any(|t| t == DESCRIPTION), "the card description reads {DESCRIPTION:?}: {texts:?}");
    assert!(!texts.iter().any(|t| t.contains("Agent Terminal")), "the removed terminal dock is no longer named: {texts:?}");
}

#[test]
fn the_agents_card_description_names_every_supported_cli_and_no_tool_count() {
    let (s, _dir) = resources_sim();
    let description = painted_strings(&s).into_iter().find(|t| t.contains("Claude Code")).expect("the Agents card description is painted");
    for name in ["Claude Code", "Codex", "Grok", "Antigravity", "Gemini CLI", "MCP server"] {
        assert!(description.contains(name), "the description names {name}: {description:?}");
    }
    assert!(!description.chars().any(|c| c.is_ascii_digit()), "a tool count drifts, so the description carries no number: {description:?}");
}

/// Bottom edge of the text run `text` painted last frame.
fn painted_bottom(s: &Sim, text: &str) -> Option<f32> {
    fn find(shape: &egui::Shape, text: &str) -> Option<f32> {
        match shape {
            egui::Shape::Text(t) if t.galley.job.text == text => Some(t.pos.y + t.galley.size().y),
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|x| find(x, text)),
            _ => None,
        }
    }
    s.shapes.iter().find_map(|c| find(&c.shape, text))
}

#[test]
fn the_longer_description_wraps_above_the_button() {
    let (s, _dir) = resources_sim();
    let bottom = painted_bottom(&s, DESCRIPTION).expect("the Agents card description is painted");
    let button = s.rect_of("home-resource-agents");
    assert!(bottom <= button.top(), "the description ends at y {bottom} and the button starts at y {}", button.top());
}

#[test]
fn the_agents_card_button_is_named_open_agents_for_a_screen_reader() {
    let (mut s, _dir) = home_sim();
    crate::agent_a11y::enable_accesskit(&s.ctx);
    s.shell.app.home.as_mut().expect("the home screen").section = Section::Resources;
    // AccessKit sends only the nodes that changed in a frame, so the name is read in the frame that first draws the button.
    let button = egui::Id::new(("home-res-b1", "home-resource-agents")).accesskit_id();
    let mut name = None;
    for _ in 0..4 {
        s.frame(vec![]);
        let update = s.accesskit_update.as_ref();
        name = update.and_then(|u| u.nodes.iter().find(|(id, _)| *id == button)).and_then(|(_, node)| node.label().map(str::to_string));
        if name.is_some() {
            break;
        }
    }
    assert_eq!(name.as_deref(), Some(BUTTON), "a screen reader hears the same words the button shows");
}
