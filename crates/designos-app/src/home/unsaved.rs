//! The unsaved dot of a tab: whether the document differs from what the last save, or the open, wrote.
//!
//! The answer is a clean point on the undo history ([`designos_doc::History::mark_clean`]): the dot shows
//! when the document is not at that point. It costs nothing per frame, never hashes the document, clears
//! when an undo walks back to the saved state at any document size, and ignores changes that are not
//! edits (a page expanded on first visit, a layout re-solve, an agent's progress badge).
//!
//! A writer that changes the document without an undo step sets [`UnsavedDot::flag`] instead, which stays
//! until the next save. The writers known to do so: an agent session that replaces the whole document
//! (`live_patch_apply`). A reload from disk and a restored version end up saved, so they mark clean.

use crate::app::App;
use std::time::SystemTime;

/// What a save leaves in [`super::Autosave`]: the hash it wrote, the canvas revision and the file's mtime.
pub type SaveMark = (u64, u64, Option<SystemTime>);

#[derive(Default)]
pub struct UnsavedDot {
    /// The document was changed by a writer that leaves no undo step, since the last save.
    flag: bool,
    /// What the last tab bar paint showed for the open document; `None` when no tab bar was drawn.
    painted: Option<bool>,
    /// [`SaveMark`] as last seen, to notice a save made outside this module (`App::save_with`).
    save_seen: SaveMark,
}

impl UnsavedDot {
    pub fn flag(&self) -> bool {
        self.flag
    }

    /// Takes over the flag a parked tab kept, for the document that is about to be shown. `mark` is the
    /// current [`SaveMark`]: whatever another document left there is not a save of this one.
    pub fn adopt(&mut self, flag: bool, mark: SaveMark) {
        self.flag = flag;
        self.save_seen = mark;
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

    /// The document on screen is exactly what a save wrote (or what was loaded from disk).
    pub(crate) fn mark_saved(&mut self) {
        self.hist.mark_clean();
        self.autosave.dot.flag = false;
    }

    /// The document changed without an undo step, so the history cannot tell: the dot stays until a save.
    pub(crate) fn mark_edited_outside_history(&mut self) {
        self.autosave.dot.flag = true;
    }

    pub(crate) fn save_mark(&self) -> SaveMark {
        (self.autosave.saved_hash, self.autosave.seen_rev, self.autosave.disk_mtime)
    }

    /// Per frame, for a document outside the library: a save writes the file and records what it wrote in
    /// [`super::Autosave`], and another value there means the document on screen is the one that was written.
    /// (`save_with` is the one writer that does not call [`Self::mark_saved`] itself.)
    pub(super) fn notice_save_outside_library(&mut self) {
        if self.lib_file.is_some() {
            return;
        }
        let mark = self.save_mark();
        if mark != self.autosave.dot.save_seen {
            self.autosave.dot.save_seen = mark;
            self.mark_saved();
        }
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

    #[test]
    fn a_save_that_only_the_autosave_fields_show_is_noticed_once_and_another_document_never_counts() {
        let mut app = app_with_rect();
        app.notice_save_outside_library();
        edit(&mut app);
        assert!(app.has_unsaved_edits());
        app.notice_save_outside_library();
        assert!(app.has_unsaved_edits(), "nothing was saved");
        // what `save_with` leaves behind
        app.autosave.saved_hash = 77;
        app.autosave.seen_rev = 3;
        app.notice_save_outside_library();
        assert!(!app.has_unsaved_edits(), "the save was noticed");
        // another document's save state arrives with a tab switch: adopted, not taken for a save
        edit(&mut app);
        app.autosave.saved_hash = 99;
        let mark = app.save_mark();
        app.autosave.dot.adopt(false, mark);
        app.notice_save_outside_library();
        assert!(app.has_unsaved_edits(), "the edit is still unsaved");
    }
}
