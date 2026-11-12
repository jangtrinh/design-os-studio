//! The words for an edit that the component cycle guard refuses, or lands elsewhere.
//!
//! The guard lives in [`crate::components`] and answers with the main that would hold itself and the path of the
//! loop. Every surface that reports a refusal (the editor toast, the agent tool errors, a replayed live edit, a
//! branch merge) builds its sentence here, so the same edit reads the same everywhere. Machine clients keep their
//! error code next to the sentence, never inside it.

use crate::components::{container_labels_between, cycle_node_label};
use crate::{Document, NodeId};

const ARROW: &str = " \u{2192} ";

/// What the refused edit was trying to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CycleAction {
    /// Show another main in an instance.
    Swap,
    /// Pick another variant of the main an instance shows.
    SwitchVariant,
    /// Put a new instance in a container.
    Add,
    /// Move layers into another container.
    Move,
    /// Merge a branch.
    Merge,
    /// Turn a layer into a main.
    MakeComponent,
}

impl CycleAction {
    fn verb(self) -> &'static str {
        match self {
            Self::Swap => "swap",
            Self::SwitchVariant => "switch variant",
            Self::Add => "add",
            Self::Move => "move",
            Self::Merge => "merge",
            Self::MakeComponent => "turn into a component",
        }
    }
}

/// The loop as one line: a long one (layers nested deep inside the main) keeps its first two hops and its last.
fn loop_text(path: &[String]) -> String {
    const MAX_LABELS: usize = 5;
    if path.len() <= MAX_LABELS {
        return path.join(ARROW);
    }
    format!("{}{ARROW}{}{ARROW}\u{2026}{ARROW}{}", path[0], path[1], path[path.len() - 1])
}

/// Why `action` did nothing: `main` would hold itself, through `path` (the labels from `main` back to `main`).
pub fn cycle_refusal_message(action: CycleAction, main: &str, path: &[String]) -> String {
    format!("Can't {}: {main} would contain itself ({})", action.verb(), loop_text(path))
}

/// A move that went ahead for the other layers while `layer` stayed out of `main`.
pub fn cycle_kept_out_message(layer: &str, main: &str) -> String {
    format!("Kept {layer} out of {main}, which would contain itself")
}

/// A move that landed outside `main` instead of in it.
pub fn cycle_moved_outside_message(main: &str) -> String {
    format!("Moved outside {main}, which would contain itself")
}

/// A new instance that was placed outside `main` instead of in it.
pub fn cycle_added_outside_message(main: &str) -> String {
    format!("Added outside {main}, which would contain itself")
}

/// A paste that landed outside `main` instead of in it.
pub fn cycle_pasted_outside_message(main: &str) -> String {
    format!("Pasted outside {main}, which would contain itself")
}

/// The path of a loop that closes on `owner` itself: `owner`, the layers down to `inside`, and `owner` again.
pub(crate) fn nested_loop_path(doc: &Document, owner: NodeId, inside: Option<NodeId>) -> Vec<String> {
    let mut path = vec![cycle_node_label(doc, owner)];
    path.extend(container_labels_between(doc, owner, inside));
    path.push(cycle_node_label(doc, owner));
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(labels: &[&str]) -> Vec<String> {
        labels.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn every_action_reads_as_one_sentence_with_the_loop() {
        let p = path(&["Card", "Icon", "Row", "Card"]);
        assert_eq!(
            cycle_refusal_message(CycleAction::Swap, "Card", &p),
            "Can't swap: Card would contain itself (Card \u{2192} Icon \u{2192} Row \u{2192} Card)"
        );
        for (action, verb) in [
            (CycleAction::SwitchVariant, "switch variant"),
            (CycleAction::Add, "add"),
            (CycleAction::Move, "move"),
            (CycleAction::Merge, "merge"),
            (CycleAction::MakeComponent, "turn into a component"),
        ] {
            assert!(cycle_refusal_message(action, "Card", &p).starts_with(&format!("Can't {verb}: Card would contain itself (")), "{verb}");
        }
    }

    #[test]
    fn a_long_loop_keeps_its_first_two_hops_and_its_last() {
        let p = path(&["A", "B", "C", "D", "E", "F"]);
        assert_eq!(cycle_refusal_message(CycleAction::Move, "A", &p), "Can't move: A would contain itself (A \u{2192} B \u{2192} \u{2026} \u{2192} F)");
        let five = path(&["A", "B", "C", "D", "A"]);
        assert_eq!(cycle_refusal_message(CycleAction::Move, "A", &five), "Can't move: A would contain itself (A \u{2192} B \u{2192} C \u{2192} D \u{2192} A)");
    }

    #[test]
    fn the_outcomes_that_are_no_refusal_name_the_main() {
        assert_eq!(cycle_kept_out_message("A", "B"), "Kept A out of B, which would contain itself");
        assert_eq!(cycle_moved_outside_message("B"), "Moved outside B, which would contain itself");
        assert_eq!(cycle_added_outside_message("B"), "Added outside B, which would contain itself");
        assert_eq!(cycle_pasted_outside_message("B"), "Pasted outside B, which would contain itself");
    }
}
