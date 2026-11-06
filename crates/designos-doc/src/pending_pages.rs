//! Pages whose content has not been loaded yet.
//!
//! A large multi-page `.fig` is imported with its inactive pages deferred: the instances on them have no
//! children yet, and the overrides that fill them wait in [`Document::page_hydrator`]. The native
//! serializer writes the document as it is and the loader is never serialized, so a write made before
//! those pages are loaded stores empty instances for good. This module is the one guard against that:
//! [`Document::has_pending_pages`] tells, [`Document::load_pending_pages`] loads them in place, and the
//! serializer ([`Document::to_json_counted`]) refuses a document that still has some.

use crate::Document;

/// Why a document cannot be written yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingPagesError {
    /// `pages` pages still hold instances whose content has not been loaded.
    NotLoaded { pages: usize },
    /// Loading the pages failed. The message is the loader's own.
    LoadFailed(String),
}

impl std::fmt::Display for PendingPagesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotLoaded { pages: 1 } => {
                f.write_str("1 page of this document is not loaded yet and its component instances are empty. Load every page before writing the file, or the file loses their content")
            }
            Self::NotLoaded { pages } => write!(
                f,
                "{pages} pages of this document are not loaded yet and their component instances are empty. Load every page before writing the file, or the file loses their content"
            ),
            Self::LoadFailed(cause) => write!(f, "Loading the pages that were not open yet failed ({cause}), so the file would lose their content"),
        }
    }
}

impl std::error::Error for PendingPagesError {}

impl Document {
    /// How many pages (the hidden library page included) still wait for their content: a loader is
    /// attached and it has not loaded them. Pages that are no longer in the document do not count, and a
    /// loader that has seen every page leaves the document complete even though it stays attached.
    ///
    /// A loader that cannot be asked (its lock is poisoned by a panic while it ran) leaves the state
    /// unknown, which counts as every page waiting: a write must not go ahead on a guess.
    pub fn pending_page_count(&self) -> usize {
        let Some(hydrator) = self.page_hydrator.as_ref() else { return 0 };
        let Ok(guard) = hydrator.lock() else { return (self.pages.len() + usize::from(self.library_page.is_some())).max(1) };
        self.pages.iter().chain(self.library_page.iter()).filter(|page| !guard.is_hydrated(**page)).count()
    }

    /// Whether some page still waits for its content (see [`Self::pending_page_count`]). Cheap: one lock
    /// and one lookup per page.
    pub fn has_pending_pages(&self) -> bool {
        self.pending_page_count() > 0
    }

    /// Loads every pending page, in place, so that a write of the whole document keeps their content.
    /// Returns how many instances the loader had to leave empty because the file went past the copy
    /// budget (`0` for a normal file): the caller says so, as opening a page does.
    ///
    /// Call this on the live document, never on a clone: a clone shares the loader with the document
    /// it came from, so pages loaded on the copy are used up for the original and stay empty there.
    ///
    /// It fails, naming the cause, when a page cannot be loaded; the document must not be written then.
    /// The loader's error is left in place for the window to report.
    pub fn load_pending_pages(&mut self) -> Result<usize, PendingPagesError> {
        if self.page_hydrator.is_none() {
            return Ok(0);
        }
        self.hydrate_all_pages();
        if let Some(cause) = self.hydration_error() {
            return Err(PendingPagesError::LoadFailed(cause));
        }
        match self.pending_page_count() {
            0 => Ok(self.take_hydration_unexpanded_instances()),
            pages => Err(PendingPagesError::NotLoaded { pages }),
        }
    }

    /// The serializer's guard: an error for a document that still has pending pages.
    pub(crate) fn refuse_pending_pages(&self) -> serde_json::Result<()> {
        match self.pending_page_count() {
            0 => Ok(()),
            pages => Err(<serde_json::Error as serde::ser::Error>::custom(PendingPagesError::NotLoaded { pages })),
        }
    }
}

#[cfg(test)]
mod tests;
