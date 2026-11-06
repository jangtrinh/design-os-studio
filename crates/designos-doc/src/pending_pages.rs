//! Pages whose content has not been loaded yet.
//!
//! A large multi-page `.fig` is imported with its inactive pages deferred: the instances on them have no
//! children yet, and the overrides that fill them wait in [`Document::page_hydrator`]. The native
//! serializer writes the document as it is and the loader is never serialized, so a write made before
//! those pages are loaded stores empty instances for good. This module is the one guard against that:
//! [`Document::has_pending_pages`] tells, [`Document::load_pending_pages`] loads them in place, and the
//! serializer ([`Document::to_json_counted`]) refuses a document that still has some.

use crate::{Document, NodeId, PageHydrator};
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, MutexGuard};

/// Why a document cannot be written yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingPagesError {
    /// `pages` pages still hold instances whose content has not been loaded.
    NotLoaded { pages: usize },
    /// `pages` pages were loaded by another copy of this document and are still empty here.
    LoadedElsewhere { pages: usize },
    /// Loading the pages failed. The message is the loader's own.
    LoadFailed(String),
    /// The loader stopped working (it panicked), so the pages can never be loaded.
    LoaderBroken,
}

impl std::fmt::Display for PendingPagesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotLoaded { pages: 1 } => f.write_str("1 page is not loaded yet. Load it first"),
            Self::NotLoaded { pages } => write!(f, "{pages} pages are not loaded yet. Load them first"),
            Self::LoadedElsewhere { .. } => f.write_str("Another copy loaded these pages. Save from that copy"),
            Self::LoadFailed(cause) => write!(f, "Loading the pages failed ({cause}). Reopen the file"),
            Self::LoaderBroken => f.write_str("Loading the pages failed earlier. Reopen the file"),
        }
    }
}

impl std::error::Error for PendingPagesError {}

/// Which pages this document still has to load itself.
///
/// The loader is shared by a document and every clone of it, and it forgets a page once any of them has
/// loaded it. So "the loader has loaded this page" says nothing about the content of one document: a clone
/// that loaded the page filled its own instances and left the original's empty. Each document therefore
/// keeps the pages it has not loaded, and clones inherit the list, so a document that cannot load a page
/// itself any more still knows its instances are empty.
#[derive(Debug, Default)]
pub(crate) struct DeferredPages(Mutex<Option<Deferred>>);

#[derive(Debug, Clone)]
struct Deferred {
    /// Address of the loader the list was taken from; a different loader starts a new list.
    loader: usize,
    pages: BTreeSet<NodeId>,
}

impl Clone for DeferredPages {
    fn clone(&self) -> Self {
        Self(Mutex::new(self.lock().clone()))
    }
}

impl DeferredPages {
    fn lock(&self) -> MutexGuard<'_, Option<Deferred>> {
        self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// What a document knows about the pages it has not loaded.
enum PendingState {
    /// Nothing waits.
    Complete,
    /// The loader cannot be asked.
    LoaderBroken,
    /// `pages` pages wait, `elsewhere` of them already used up in the loader by another copy.
    Waiting { pages: usize, elsewhere: usize },
}

fn loader_address(loader: &Arc<Mutex<dyn PageHydrator>>) -> usize {
    Arc::as_ptr(loader) as *const () as usize
}

impl Document {
    /// Takes the list of pages this document has not loaded from `loader` the first time it is asked,
    /// and returns it for the caller to read or change.
    fn deferred_pages(&self, loader_at: usize, loader: &dyn PageHydrator) -> MutexGuard<'_, Option<Deferred>> {
        let mut state = self.deferred.lock();
        if state.as_ref().is_none_or(|d| d.loader != loader_at) {
            let pages = self.pages.iter().chain(self.library_page.iter()).copied().filter(|page| !loader.is_hydrated(*page)).collect();
            *state = Some(Deferred { loader: loader_at, pages });
        }
        state
    }

    /// Makes sure the list exists before a clone is taken, so that the clone carries the pages that were
    /// waiting at that moment and not what the loader says after the copy loaded some.
    pub(crate) fn remember_deferred_pages(&self) {
        let Some(hydrator) = self.page_hydrator.as_ref() else { return };
        // A clone never waits for the loader: if it is busy loading a page (or a loader calls back into a clone),
        // the list is taken the first time it is asked for instead.
        let Ok(loader) = hydrator.try_lock() else { return };
        drop(self.deferred_pages(loader_address(hydrator), &*loader));
    }

    /// Records that this document loaded `pages` (those the loader now reports as loaded) into itself.
    pub(crate) fn forget_loaded_pages(&self, loader_at: usize, loader: &dyn PageHydrator, pages: impl IntoIterator<Item = NodeId>) {
        let mut state = self.deferred_pages(loader_at, loader);
        if let Some(deferred) = state.as_mut() {
            for page in pages {
                if loader.is_hydrated(page) {
                    deferred.pages.remove(&page);
                }
            }
        }
    }

    /// The pages this document has not loaded and the loader can still load (or has already loaded for
    /// another copy), in document order.
    pub(crate) fn unloaded_pages(&self, loader_at: usize, loader: &dyn PageHydrator) -> Vec<NodeId> {
        let state = self.deferred_pages(loader_at, loader);
        let Some(deferred) = state.as_ref() else { return Vec::new() };
        self.pages.iter().chain(self.library_page.iter()).copied().filter(|page| deferred.pages.contains(page)).collect()
    }

    fn pending_state(&self) -> PendingState {
        let Some(hydrator) = self.page_hydrator.as_ref() else { return PendingState::Complete };
        let Ok(loader) = hydrator.lock() else { return PendingState::LoaderBroken };
        let waiting = self.unloaded_pages(loader_address(hydrator), &*loader);
        match waiting.len() {
            0 => PendingState::Complete,
            pages => PendingState::Waiting { pages, elsewhere: waiting.iter().filter(|page| loader.is_hydrated(**page)).count() },
        }
    }

    /// How many pages (the hidden library page included) still wait for their content in this document:
    /// a loader is attached and this document has not loaded them. Pages that are no longer in the
    /// document do not count, and a loader that has seen every page leaves the document complete even
    /// though it stays attached. A page that a copy of this document loaded still counts: its content
    /// went to the copy.
    ///
    /// A loader that cannot be asked (its lock is poisoned by a panic while it ran) leaves the state
    /// unknown, which counts as every page waiting: a write must not go ahead on a guess.
    pub fn pending_page_count(&self) -> usize {
        match self.pending_state() {
            PendingState::Complete => 0,
            PendingState::LoaderBroken => (self.pages.len() + usize::from(self.library_page.is_some())).max(1),
            PendingState::Waiting { pages, .. } => pages,
        }
    }

    /// Whether some page still waits for its content (see [`Self::pending_page_count`]). Cheap: one lock
    /// and one lookup per page.
    pub fn has_pending_pages(&self) -> bool {
        self.pending_page_count() > 0
    }

    /// Why this document cannot be written natively yet, or `None` when it can.
    pub fn pending_pages_error(&self) -> Option<PendingPagesError> {
        match self.pending_state() {
            PendingState::Complete => None,
            PendingState::LoaderBroken => Some(PendingPagesError::LoaderBroken),
            PendingState::Waiting { pages, elsewhere: 0 } => Some(PendingPagesError::NotLoaded { pages }),
            PendingState::Waiting { pages, .. } => Some(PendingPagesError::LoadedElsewhere { pages }),
        }
    }

    /// Loads every pending page, in place, so that a write of the whole document keeps their content.
    /// Returns how many instances the loader had to leave empty because the file went past the copy
    /// budget (`0` for a normal file): the caller says so, as opening a page does.
    ///
    /// Call this on the live document, never on a clone: a clone shares the loader with the document
    /// it came from, so pages loaded on the copy are used up for the original and stay empty there.
    /// Such a document refuses ([`PendingPagesError::LoadedElsewhere`]) instead of reporting success.
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
        match self.pending_pages_error() {
            None => Ok(self.take_hydration_unexpanded_instances()),
            Some(err) => Err(err),
        }
    }

    /// The serializer's guard: an error for a document that still has pending pages.
    pub(crate) fn refuse_pending_pages(&self) -> serde_json::Result<()> {
        match self.pending_pages_error() {
            None => Ok(()),
            Some(err) => Err(<serde_json::Error as serde::ser::Error>::custom(err)),
        }
    }
}

#[cfg(test)]
mod tests;
