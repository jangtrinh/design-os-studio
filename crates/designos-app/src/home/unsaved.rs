//! The unsaved dot of a tab: whether the document differs from what the last save, or the open, wrote.
//!
//! The answer is a clean point on the undo history ([`designos_doc::History::mark_clean`]): the dot shows
//! when the document is not at that point. It costs nothing per frame, never hashes the document, clears
//! when an undo walks back to the saved state at any document size, and ignores changes that are not
//! edits (a page expanded on first visit, a layout re-solve, an agent's progress badge).
//!
//! A writer that changes the document without an undo step sets [`UnsavedDot::flag`] instead, which stays
//! until the next save. The writers known to do so: an agent session that replaces the whole document
//! (`live_patch_apply`) and a restored version whose write failed. Every save, and a reload that really
//! replaces the document with the file, mark clean at the moment they write: nothing infers a save.

use crate::app::App;

#[derive(Default)]
pub struct UnsavedDot {
    /// The document was changed by a writer that leaves no undo step, since the last save.
    flag: bool,
    /// What the last tab bar paint showed for the open document; `None` when no tab bar was drawn.
    painted: Option<bool>,
}

impl UnsavedDot {
    pub fn flag(&self) -> bool {
        self.flag
    }

    /// Takes over the flag a parked tab kept, for the document that is about to be shown.
    pub fn adopt(&mut self, flag: bool) {
        self.flag = flag;
        self.painted = None;
    }

    /// Records what the tab bar painted for the open document this frame.
    pub fn painted(&mut self, lit: bool) {
        self.painted = Some(lit);
    }
}

impl App {
    /// Whether the open document has edits that no save has written: the unsaved dot of its tab.
    pub(crate) fn has_unsaved_edits(&self) -> bool {
        !self.hist.is_clean() || self.autosave.dot.flag()
    }

    /// Ends an open text session with its text kept, so the typing is a step of the history. Until then it is in
    /// the document but in no history: the dot cannot show it, a tab switch would take it for nothing, and a
    /// write by another program would replace it unasked. Every place that parks or closes the document calls this first.
    pub(crate) fn commit_open_text_session(&mut self) {
        if self.canvas.text.is_some() {
            crate::canvas::textsession::finish_text(self, true);
        }
    }

    /// Ends what is under way, so that the document is settled and everything in it is in the history: the speaker
    /// notes being typed (a draft that lives only in the notes field until "Save notes" or a slide change, so no dot
    /// shows it), a drag or another canvas gesture (ended where it last drew, as the one undo step the release would
    /// have made), an open text session (its text kept), and a property scrub (its change as one undo step). For a place
    /// that asks about unsaved edits, or saves, while the pointer may still be down.
    pub(crate) fn commit_open_edits(&mut self) {
        crate::decks::ui::flush_notes(self);
        // The gesture first: drawing a sticky or a board shape ends in a text session, which is then ended too.
        crate::canvas::interact::commit_open_gesture(self);
        self.commit_open_text_session();
        if self.scrub.is_some() {
            self.scrub_end();
        }
    }

    /// The document on screen is exactly what a save wrote (or what was loaded from disk).
    pub(crate) fn mark_saved(&mut self) {
        self.hist.mark_clean();
        self.autosave.dot.flag = false;
    }

    /// The document changed without an undo step, so the history cannot tell: the dot stays until a save.
    pub(crate) fn mark_edited_outside_history(&mut self) {
        self.autosave.dot.flag = true;
    }

    /// The tab bar draws before the edit, the undo or the save of the frame that changes the dot, and egui
    /// sleeps until the next input. One repaint when the state moved after the tab bar painted it; none
    /// while it holds, so an idle window stays idle.
    pub fn repaint_if_dot_moved(&mut self, ctx: &egui::Context) {
        if let Some(painted) = self.autosave.dot.painted.take() {
            if painted != self.has_unsaved_edits() {
                ctx.request_repaint();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Command, Document, Node, Rect};

    fn app_with_rect() -> App {
        let mut d = Document::new("t");
        let page = d.add_page("P");
        let id = d.alloc_id();
        d.insert(page, Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        App::new(d)
    }

    fn edit(app: &mut App) {
        let id = app.doc.alloc_id();
        let parent = app.page;
        let cmd = Command::Insert { parent, index: 0, nodes: vec![Node::rectangle(id, Rect::new(0.0, 0.0, 5.0, 5.0))] };
        app.hist.exec(&mut app.doc, cmd).unwrap();
    }

    #[test]
    fn a_fresh_document_is_clean_an_edit_lights_the_dot_and_a_save_clears_it() {
        let mut app = app_with_rect();
        assert!(!app.has_unsaved_edits());
        edit(&mut app);
        assert!(app.has_unsaved_edits());
        app.mark_saved();
        assert!(!app.has_unsaved_edits());
    }

    #[test]
    fn undoing_back_to_the_saved_state_clears_the_dot_and_undoing_past_it_lights_it() {
        let mut app = app_with_rect();
        edit(&mut app);
        app.mark_saved();
        edit(&mut app);
        assert!(app.has_unsaved_edits());
        app.undo();
        assert!(!app.has_unsaved_edits(), "back at the saved state");
        app.undo();
        assert!(app.has_unsaved_edits(), "past it");
    }

    #[test]
    fn a_change_outside_the_history_lights_the_dot_until_the_next_save() {
        let mut app = app_with_rect();
        app.mark_edited_outside_history();
        assert!(app.has_unsaved_edits());
        app.mark_saved();
        assert!(!app.has_unsaved_edits());
    }
}
