//! The unsaved dot of a tab: whether the document differs from what the last save, or the open, wrote.
//!
//! The undo stack cannot answer that: a save leaves it as it was. A library file knows its own state,
//! because [`super::Autosave`] holds an edit until it is written (or until a look at the document shows
//! it did not change), and feeds [`UnsavedDot::set_lit`]. A document outside the library (a `.designos`
//! or `.fig` file on disk, or one never saved) has no autosave, so [`UnsavedDot::observe`] compares it
//! with the document as last saved or opened, hashing it once per changed revision after the edit
//! settled, never per frame.

use super::session::{doc_hash, PROBE_DELAY};
use crate::app::App;

/// What [`UnsavedDot::observe`] wants from the caller this frame.
#[derive(Debug, PartialEq, Eq)]
pub enum Look {
    /// Nothing to compare.
    Idle,
    /// A change is waiting to settle: wake up again soon.
    Waiting,
    /// The change settled: hash the document and pass the hash to [`UnsavedDot::decide`].
    Due,
}

#[derive(Default)]
pub struct UnsavedDot {
    /// The document differs from the clean one.
    lit: bool,
    /// Hash of the document as last saved, or as opened.
    clean_hash: u64,
    /// `Autosave::saved_hash` as last seen. Another value means a save wrote the document as it is.
    saved_hash_seen: u64,
    /// Canvas revision last looked at.
    rev: u64,
    /// When the last revision change was seen, while it waits to settle.
    changed_at: Option<f64>,
}

impl UnsavedDot {
    pub fn lit(&self) -> bool {
        self.lit
    }

    /// For a library file, whose state autosave tracks.
    pub fn set_lit(&mut self, lit: bool) {
        self.lit = lit;
    }

    /// Hash of the document as last saved or opened; a tab keeps it while it is parked.
    pub fn clean_hash(&self) -> u64 {
        self.clean_hash
    }

    /// Takes over the state a parked tab kept, for the document that is about to be shown.
    pub fn adopt(&mut self, clean_hash: u64, lit: bool, saved_hash: u64) {
        self.clean_hash = clean_hash;
        self.lit = lit;
        self.saved_hash_seen = saved_hash;
        self.changed_at = None;
    }

    /// A save writes the document as it is and records its hash in `saved_hash`: that is the clean document now.
    pub fn note_saved(&mut self, saved_hash: u64) {
        if saved_hash != self.saved_hash_seen {
            self.saved_hash_seen = saved_hash;
            self.clean_hash = saved_hash;
            self.lit = false;
        }
    }

    /// One frame. `pristine` is true while the undo history is empty, so nothing was edited since the document
    /// was opened and it cannot differ from the clean one. `busy` is true while an edit is under way.
    pub fn observe(&mut self, rev: u64, saved_hash: u64, pristine: bool, now: f64, busy: bool) -> Look {
        self.note_saved(saved_hash);
        if rev != self.rev {
            self.rev = rev;
            self.changed_at = Some(now);
            if pristine {
                self.lit = false;
            }
        }
        match self.changed_at {
            None => Look::Idle,
            Some(_) if busy => {
                // An edit is under way (a drag, a text session): the dot keeps its state until it is over.
                self.changed_at = Some(now);
                Look::Waiting
            }
            // A document nothing was edited in has nothing to settle: look at once, so it has its baseline before any edit.
            Some(t) if pristine || now - t >= PROBE_DELAY => {
                self.changed_at = None;
                Look::Due
            }
            Some(_) => Look::Waiting,
        }
    }

    /// Compares the document's `hash` with the clean one. A document nothing was edited in is the clean one
    /// by definition: that is how a freshly opened or created document gets its baseline.
    pub fn decide(&mut self, hash: u64, pristine: bool) {
        if pristine {
            self.clean_hash = hash;
        }
        self.lit = hash != self.clean_hash;
    }
}

impl App {
    /// Whether the open document has edits that no save has written: the unsaved dot of its tab.
    pub(crate) fn has_unsaved_edits(&self) -> bool {
        self.autosave.dot.lit()
    }

    /// [`Self::has_unsaved_edits`] decided now, not after the edit settled: for a tab about to be parked.
    /// Call it after [`Self::flush_autosave`], with the document still in the app.
    pub(crate) fn unsaved_now(&mut self) -> bool {
        let hash = doc_hash(&self.doc);
        if self.lib_file.is_some() {
            // The flush wrote any edit: what differs now is an edit whose save failed.
            let differs = hash != self.autosave.saved_hash;
            self.autosave.dot.set_lit(differs);
            return differs;
        }
        let pristine = self.is_pristine();
        self.autosave.dot.note_saved(self.autosave.saved_hash);
        self.autosave.dot.decide(hash, pristine);
        self.autosave.dot.lit()
    }

    /// Nothing was edited since the document was opened or created: its undo and redo history is empty.
    fn is_pristine(&self) -> bool {
        !self.hist.can_undo() && !self.hist.can_redo()
    }

    /// Per frame: keeps the dot of the open document's tab current.
    pub(super) fn track_unsaved_dot(&mut self, ctx: &egui::Context, busy: bool) {
        if self.lib_file.is_some() {
            // An edit lights the dot once autosave has proven it differs from the saved file, and the dot
            // stays while the edit is outstanding (also while a drag holds the proof back). A write, or a
            // look that finds nothing changed, clears `changed_at`. A save that failed keeps it.
            let a = &self.autosave;
            let lit = a.pending() || a.save_failed_at.is_some() || (a.dot.lit() && a.changed_at.is_some());
            self.autosave.dot.set_lit(lit);
            return;
        }
        let pristine = self.is_pristine();
        let look = self.autosave.dot.observe(self.canvas.tiles.rev(), self.autosave.saved_hash, pristine, self.now, busy);
        match look {
            Look::Idle => {}
            // A bounded wake-up for the one pending look, not a timer that runs while idle.
            Look::Waiting => ctx.request_repaint_after(std::time::Duration::from_millis(250)),
            Look::Due => {
                let hash = doc_hash(&self.doc);
                self.autosave.dot.decide(hash, pristine);
                ctx.request_repaint();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_changed_revision_is_compared_only_after_it_settled() {
        let mut d = UnsavedDot::default();
        d.decide(7, true);
        assert_eq!(d.observe(1, 0, false, 1.0, false), Look::Waiting);
        assert_eq!(d.observe(1, 0, false, 1.2, false), Look::Waiting);
        assert_eq!(d.observe(1, 0, false, 1.4, false), Look::Due);
        assert_eq!(d.observe(1, 0, false, 1.5, false), Look::Idle, "one look per change");
        d.decide(8, false);
        assert!(d.lit());
    }

    #[test]
    fn an_edit_under_way_keeps_the_state_and_postpones_the_look() {
        let mut d = UnsavedDot::default();
        d.decide(7, true);
        d.decide(8, false);
        assert!(d.lit());
        assert_eq!(d.observe(2, 0, false, 1.0, true), Look::Waiting);
        assert_eq!(d.observe(2, 0, false, 5.0, true), Look::Waiting, "still busy: no look");
        assert!(d.lit(), "the dot does not flicker off under the pointer");
        assert_eq!(d.observe(2, 0, false, 5.1, false), Look::Waiting);
        assert_eq!(d.observe(2, 0, false, 5.4, false), Look::Due);
    }

    #[test]
    fn a_save_clears_the_dot_at_once_and_the_next_edit_lights_it_again() {
        let mut d = UnsavedDot::default();
        d.decide(7, true);
        d.decide(8, false);
        assert!(d.lit());
        assert_eq!(d.observe(1, 8, false, 1.0, false), Look::Waiting);
        assert!(!d.lit(), "saved_hash took the edited document's hash");
        d.decide(8, false);
        assert!(!d.lit());
        d.decide(9, false);
        assert!(d.lit());
        d.decide(8, false);
        assert!(!d.lit(), "undone back to what the save wrote");
    }

    #[test]
    fn a_pristine_document_has_no_dot_and_gets_its_baseline_at_once() {
        let mut d = UnsavedDot::default();
        d.decide(7, true);
        d.decide(8, false);
        assert!(d.lit());
        // another document replaced it: its undo history is empty
        assert_eq!(d.observe(3, 0, true, 2.0, false), Look::Due, "no settle delay without an edit");
        assert!(!d.lit());
        d.decide(55, true);
        assert!(!d.lit());
        assert_eq!(d.clean_hash(), 55);
    }

    #[test]
    fn a_pristine_document_under_an_edit_is_not_taken_as_the_baseline() {
        let mut d = UnsavedDot::default();
        assert_eq!(d.observe(1, 0, true, 1.0, true), Look::Waiting, "a drag is under way, not yet in the history");
    }
}
