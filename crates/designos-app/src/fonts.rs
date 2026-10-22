//! Background loading of the open-source fonts a document uses.
//!
//! After a document is opened, every family/weight/style it references is
//! resolved through `designos-fonts` (bundled, disk cache, then Fontsource) on a
//! worker thread. Results are registered with the renderer on the UI thread.

use designos_doc::Document;
use designos_fonts::FontRequest;
use designos_fonts::{ensure_font_requests_with, FontCache, FontReport};
use std::collections::HashSet;
use std::sync::mpsc::{channel, Receiver};

#[derive(Default)]
pub struct FontLoader {
    pending: Option<Receiver<FontReport>>,
    /// The faces the latest `load_for` asked for (tests read it instead of waiting for a fetch).
    #[cfg(test)]
    requested: std::collections::BTreeSet<FontRequest>,
}

/// Outcome of a finished load, for the toast.
pub struct FontsLoaded {
    pub loaded: usize,
    pub failed: Vec<String>,
}

impl FontLoader {
    /// Starts resolving the document's fonts; a previous load is abandoned.
    pub fn load_for(&mut self, doc: &Document) {
        self.load_for_with(doc, []);
    }

    /// [`Self::load_for`] plus `extra` faces the caller knows the document now uses. A large
    /// document is only scanned in part (`requests_for` stops after 8,192 nodes), so a face an edit
    /// added further down would never be asked for.
    pub fn load_for_with(&mut self, doc: &Document, extra: impl IntoIterator<Item = FontRequest>) {
        // Fonts the text names, plus the Noto fallbacks of the complex scripts it uses (Arabic, Hebrew, Devanagari).
        let mut requests = designos_fonts::requests_for(doc);
        requests.extend(extra);
        requests.extend(designos_fonts::script_requests_for(doc));
        #[cfg(test)]
        {
            self.requested = requests.clone();
        }
        if requests.is_empty() {
            self.pending = None;
            return;
        }
        // The renderer's font database is thread-local, so check installed
        // families here on the UI thread, not in the worker.
        let installed: HashSet<String> = requests
            .iter()
            .map(|r| r.family.clone())
            .filter(|family| designos_render::resolve_font(family, 400, false).is_some_and(|f| f.family.eq_ignore_ascii_case(family)))
            .collect();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let report = ensure_font_requests_with(requests, &open_cache(), |family| installed.contains(family));
            let _ = tx.send(report);
        });
        self.pending = Some(rx);
    }

    /// Registers finished fonts with the renderer. Returns `Some` once per load.
    pub fn poll(&mut self) -> Option<FontsLoaded> {
        let report = self.pending.as_ref()?.try_recv().ok()?;
        self.pending = None;
        let loaded = report.loaded.len();
        for f in report.loaded {
            designos_export::register_font(f.data.clone());
            designos_render::register_font(f.data);
        }
        let failed = report.failed.iter().map(|(req, err)| format!("{} {}: {err}", req.family, req.weight)).collect();
        Some(FontsLoaded { loaded, failed })
    }

    pub fn is_loading(&self) -> bool {
        self.pending.is_some()
    }

    #[cfg(test)]
    pub(crate) fn requested(&self) -> &std::collections::BTreeSet<FontRequest> {
        &self.requested
    }
}

/// The cache a load resolves through. Unit tests never touch the network or the user's cache.
fn open_cache() -> FontCache {
    #[cfg(test)]
    {
        FontCache::with_dir(std::env::temp_dir().join("designos-app-test-fonts")).offline()
    }
    #[cfg(not(test))]
    {
        FontCache::new()
    }
}
