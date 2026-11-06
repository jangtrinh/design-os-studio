//! Loading the pages a large `.fig` import left unloaded before the document is written.
//!
//! The importer defers the inactive pages of a large multi-page `.fig`: the instances on them are empty until the
//! page is opened. The native serializer refuses such a document, so every write of the whole document loads the
//! pages first, in place, on the document that is open. Never on a copy: a copy shares the loader.

use crate::app::App;
use crate::io;

impl App {
    /// Loads every page of the open document that is still waiting for its content, so that the write that follows
    /// keeps it. Returns the notice to show when the file has more nested copies than can be expanded.
    ///
    /// A large file takes a while to load, and the window waits for it.
    pub(crate) fn load_pending_pages_for_write(&mut self) -> Result<Option<String>, String> {
        if self.doc.page_hydrator.is_none() {
            return Ok(None);
        }
        let pending = self.doc.pending_page_count();
        let started = std::time::Instant::now();
        let left_empty = self.doc.load_pending_pages().map_err(|e| e.to_string())?;
        if pending > 0 {
            // The pages are in memory now: the window shows what the file will hold.
            self.changed();
            eprintln!("designos: loaded {pending} pages that were not open yet before writing ({:.1} s)", started.elapsed().as_secs_f32());
        }
        Ok((left_empty > 0).then(|| io::clone_budget_notice(left_empty)))
    }
}
