//! THROWAWAY document-level undo. One entry per user action; typing/deleting bursts coalesce like
//! `crates/designos-app/src/textedit.rs::UndoLog` (same 1 s rule). That log is concrete over
//! `TextProps`, so the real build needs it generic over the snapshot type or a second log.

use crate::model::Doc;

#[derive(Clone, Debug, PartialEq)]
pub struct Snap {
    pub doc: Doc,
    pub block: usize,
    pub at: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Edit {
    Typing,
    Deleting,
    Other,
}

pub const BURST_SECONDS: f64 = 1.0;

#[derive(Default)]
pub struct History {
    undo: Vec<Snap>,
    redo: Vec<Snap>,
    last: Option<(Edit, f64)>,
}

impl History {
    /// Records `before` unless this edit continues the previous typing/deleting burst.
    pub fn record(&mut self, before: Snap, kind: Edit, now: f64) {
        let continues = matches!((self.last, kind), (Some((k, t)), Edit::Typing | Edit::Deleting) if k == kind && now - t < BURST_SECONDS);
        self.last = Some((kind, now));
        self.redo.clear();
        if !continues {
            self.undo.push(before);
        }
    }

    pub fn break_burst(&mut self) {
        self.last = None;
    }

    pub fn undo(&mut self, current: Snap) -> Option<Snap> {
        let s = self.undo.pop()?;
        self.redo.push(current);
        self.last = None;
        Some(s)
    }

    pub fn redo(&mut self, current: Snap) -> Option<Snap> {
        let s = self.redo.pop()?;
        self.undo.push(current);
        self.last = None;
        Some(s)
    }

    pub fn depth(&self) -> usize {
        self.undo.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Kind;

    fn snap(t: &str) -> Snap {
        Snap { doc: Doc::new(vec![(Kind::Text, t)]), block: 0, at: 0 }
    }

    #[test]
    fn typing_burst_is_one_step_but_split_is_its_own() {
        let mut h = History::default();
        h.record(snap(""), Edit::Typing, 0.0);
        h.record(snap("a"), Edit::Typing, 0.2);
        h.record(snap("ab"), Edit::Other, 0.3); // a split
        h.record(snap("ab|"), Edit::Typing, 0.4);
        assert_eq!(h.depth(), 3);
        assert_eq!(h.undo(snap("now")).unwrap().doc.blocks[0].text, "ab|");
        assert_eq!(h.undo(snap("now")).unwrap().doc.blocks[0].text, "ab");
        assert_eq!(h.undo(snap("now")).unwrap().doc.blocks[0].text, "");
        assert!(h.undo(snap("now")).is_none());
    }

    #[test]
    fn a_pause_starts_a_new_step_and_redo_replays() {
        let mut h = History::default();
        h.record(snap(""), Edit::Typing, 0.0);
        h.record(snap("a"), Edit::Typing, 2.0);
        assert_eq!(h.depth(), 2);
        let back = h.undo(snap("ab")).unwrap();
        assert_eq!(back.doc.blocks[0].text, "a");
        assert_eq!(h.redo(back).unwrap().doc.blocks[0].text, "ab");
    }
}
