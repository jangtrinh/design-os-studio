//! Background loading of the open-source fonts a document uses.
//!
//! After a document is opened, every family/weight/style it references is
//! resolved through `designos-fonts` (bundled, disk cache, then Fontsource) on a
//! worker thread. Results are registered with the renderer on the UI thread.

use designos_doc::Document;
use designos_fonts::{ensure_font_requests_with, FontCache, FontReport, FontRequest, LoadedFont};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

pub struct FontLoader {
    pending: Option<Receiver<FontReport>>,
    /// Faces this loader has registered with the renderer.
    registered: HashSet<FontRequest>,
    /// Faces edits asked for that no finished load has resolved yet. Every load asks for them
    /// again, so a load started by a later edit never drops what an earlier one wanted.
    wanted: BTreeSet<FontRequest>,
    /// Faces whose failure has been reported this session. Edits leave them out of their loads, so
    /// a Ctrl+B on one text never fetches or toasts a face of another that is not on Fontsource.
    /// A document-level load still tries them (the network may be back) but does not toast again.
    failed: HashSet<FontRequest>,
    /// Opens the cache a load resolves through.
    open_cache: Arc<dyn Fn() -> FontCache + Send + Sync>,
    /// The texts waiting to be re-measured with the faces that arrived.
    pub(crate) refit: crate::font_refit::TextRefit,
    /// The faces the latest `load_for` asked for (tests read it instead of waiting for a fetch).
    #[cfg(test)]
    requested: BTreeSet<FontRequest>,
    /// How many loads have started.
    #[cfg(test)]
    started: usize,
}

impl Default for FontLoader {
    fn default() -> Self {
        Self {
            pending: None,
            registered: HashSet::new(),
            wanted: BTreeSet::new(),
            failed: HashSet::new(),
            open_cache: Arc::new(default_cache),
            refit: Default::default(),
            #[cfg(test)]
            requested: Default::default(),
            #[cfg(test)]
            started: 0,
        }
    }
}

/// Outcome of a finished load, for the toast.
pub struct FontsLoaded {
    pub loaded: usize,
    pub failed: Vec<String>,
    /// The families of the faces that arrived, under the name the document uses and the name the
    /// font holds (a document's "Gotham" is drawn with Montserrat): the texts to re-measure.
    pub families: Vec<String>,
}

impl FontLoader {
    /// Starts resolving the document's fonts (an open, a tab switch, a paste, the family picker);
    /// a previous load is abandoned. Faces that failed before are tried again.
    pub fn load_for(&mut self, doc: &Document) {
        self.load_for_with(doc, []);
    }

    /// [`Self::load_for`] plus `extra` faces the caller knows the document uses. A large document
    /// is only scanned in part (`requests_for` stops after 8,192 nodes), so a face further down
    /// would never be asked for. The extras stay wanted until a load finishes.
    pub fn load_for_with(&mut self, doc: &Document, extra: impl IntoIterator<Item = FontRequest>) {
        self.wanted.extend(extra);
        self.start(doc, false);
    }

    /// An edit introduced the faces `added`. Asks for the ones that still need a fetch and starts
    /// no load when there are none: a face that is bundled, installed, already registered, or that
    /// failed before needs nothing, and a Ctrl+B in Inter must not wake the network.
    pub fn request_new_faces(&mut self, doc: &Document, added: Vec<FontRequest>) {
        let fresh: BTreeSet<FontRequest> = added.into_iter().filter(|r| !self.failed.contains(r)).collect();
        let covered = self.available(&fresh);
        let needed: Vec<FontRequest> = fresh.into_iter().filter(|r| !covered.contains(r)).collect();
        if needed.is_empty() {
            return;
        }
        self.wanted.extend(needed);
        self.start(doc, true);
    }

    /// Forgets that `family` failed, so the next load reports it again: picking the family again
    /// is the explicit retry.
    pub fn retry_family(&mut self, family: &str) {
        self.failed.retain(|r| !r.family.eq_ignore_ascii_case(family));
    }

    /// Starts a load of the document's faces and everything wanted, leaving out the faces that
    /// failed before when `skip_failed`.
    fn start(&mut self, doc: &Document, skip_failed: bool) {
        // Fonts the text names, plus the Noto fallbacks of the complex scripts it uses (Arabic, Hebrew, Devanagari).
        let mut requests = designos_fonts::requests_for(doc);
        requests.extend(self.wanted.iter().cloned());
        requests.extend(designos_fonts::script_requests_for(doc));
        if skip_failed {
            requests.retain(|r| !self.failed.contains(r));
        }
        #[cfg(test)]
        {
            self.requested = requests.clone();
        }
        if requests.is_empty() {
            self.pending = None;
            return;
        }
        #[cfg(test)]
        {
            self.started += 1;
        }
        let available = self.available(&requests);
        let open_cache = self.open_cache.clone();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let report = ensure_font_requests_with(requests, &open_cache(), |request| available.contains(request));
            let _ = tx.send(report);
        });
        self.pending = Some(rx);
    }

    /// The faces that need no fetch. The renderer's font database is thread-local, so this is
    /// decided here on the UI thread, not in the worker.
    ///
    /// A face this loader registered, or one embedded in the binary, is available. A family installed on the system is available
    /// at every weight, because the system serves the nearest face. A family the loader fetched is
    /// not: having its regular face says nothing about its bold one, so each face is its own request.
    fn available(&self, requests: &BTreeSet<FontRequest>) -> HashSet<FontRequest> {
        let fetched: HashSet<String> = self.registered.iter().map(|r| r.family.to_lowercase()).collect();
        let mut system: HashMap<&str, bool> = HashMap::new();
        requests
            .iter()
            .filter(|r| {
                self.registered.contains(*r)
                    || is_bundled(r)
                    || *system.entry(r.family.as_str()).or_insert_with(|| !fetched.contains(&r.family.to_lowercase()) && is_installed(&r.family))
            })
            .cloned()
            .collect()
    }

    /// A face `t` uses is not one the renderer has (not embedded, not installed, not registered):
    /// the text is measured and drawn with a stand-in font until that face arrives.
    pub(crate) fn uses_stand_in(&self, t: &designos_doc::TextProps) -> bool {
        let faces = designos_fonts::text_faces(t);
        self.available(&faces).len() < faces.len()
    }

    /// Registers finished fonts with the renderer. Returns `Some` once per load.
    pub fn poll(&mut self) -> Option<FontsLoaded> {
        let report = self.pending.as_ref()?.try_recv().ok()?;
        self.pending = None;
        // The load that finished asked for everything wanted when it started (a later edit would
        // have replaced it), so what it could not fetch is reported once and not asked for again.
        self.wanted.clear();
        let loaded = report.loaded.len();
        let mut families = BTreeSet::new();
        for f in report.loaded {
            self.failed.remove(&f.request);
            families.insert(f.request.family.clone());
            families.insert(f.family.clone());
            self.register(f);
        }
        // A failure is reported the first time; a later load that fails again stays quiet.
        let failed =
            report.failed.iter().filter(|(req, _)| self.failed.insert(req.clone())).map(|(req, err)| format!("{} {}: {err}", req.family, req.weight)).collect();
        Some(FontsLoaded { loaded, failed, families: families.into_iter().collect() })
    }

    /// Registers one fetched font with the renderer and remembers its face.
    fn register(&mut self, f: LoadedFont) {
        // Under the name the document uses and under the one the font database holds ("Gotham" is
        // drawn with Montserrat). A family that was installed before this font arrived stays a
        // system family: it still covers every weight, so it is not recorded as fetched.
        let fetched = self.registered.iter().any(|r| r.family.eq_ignore_ascii_case(&f.family));
        if fetched || !is_installed(&f.family) {
            self.registered.insert(FontRequest::new(f.family.clone(), f.request.weight, f.request.italic));
        }
        self.registered.insert(f.request.clone());
        designos_export::register_font(f.data.clone());
        designos_render::register_font(f.data);
    }

    pub fn is_loading(&self) -> bool {
        self.pending.is_some()
    }

    #[cfg(test)]
    pub(crate) fn requested(&self) -> &BTreeSet<FontRequest> {
        &self.requested
    }

    #[cfg(test)]
    pub(crate) fn registered(&self) -> &HashSet<FontRequest> {
        &self.registered
    }

    #[cfg(test)]
    pub(crate) fn started(&self) -> usize {
        self.started
    }
}

/// The face is embedded in the binary, under its own name or a substitute's (SF Pro is drawn with Inter).
fn is_bundled(r: &FontRequest) -> bool {
    let family = designos_fonts::substitute(&r.family).unwrap_or(r.family.as_str());
    designos_fonts::bundled_for(&FontRequest::new(family, r.weight, r.italic)).is_some()
}

/// `family` is in the renderer's font database under its own name.
fn is_installed(family: &str) -> bool {
    designos_render::resolve_font(family, 400, false).is_some_and(|f| f.family.eq_ignore_ascii_case(family))
}

/// The cache a load resolves through. Unit tests never touch the network or the user's cache.
fn default_cache() -> FontCache {
    #[cfg(test)]
    {
        FontCache::with_dir(std::env::temp_dir().join("designos-app-test-fonts")).offline()
    }
    #[cfg(not(test))]
    {
        FontCache::new()
    }
}

/// Test support: a loader whose downloads are served from memory, and a wait for its load.
#[cfg(test)]
pub(crate) mod testkit {
    use super::*;
    use designos_fonts::fontsource::{font_id, metadata_url};
    use designos_fonts::{Fetch, FontError};
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Swaps `from` for `to` (same length) in every ASCII and UTF-16BE spelling inside `data`.
    fn swap_name(data: &mut [u8], from: &str, to: &str) {
        assert_eq!(from.len(), to.len(), "the name is swapped in place");
        let utf16 = |s: &str| s.encode_utf16().flat_map(u16::to_be_bytes).collect::<Vec<u8>>();
        for (a, b) in [(from.as_bytes().to_vec(), to.as_bytes().to_vec()), (utf16(from), utf16(to))] {
            let mut i = 0;
            while i + a.len() <= data.len() {
                if data[i..i + a.len()] == a[..] {
                    data[i..i + a.len()].copy_from_slice(&b);
                    i += a.len();
                } else {
                    i += 1;
                }
            }
        }
    }

    /// A real font (the bundled Roboto Mono) that names itself `family`, which must be 11 characters long.
    pub fn font_named(family: &str) -> Vec<u8> {
        let mut data = designos_fonts::bundled().iter().find(|a| a.family == "Roboto Mono").expect("Roboto Mono is bundled").data.to_vec();
        swap_name(&mut data, "Roboto Mono", family);
        data
    }

    /// A private cache directory, removed on drop.
    struct Scratch(std::path::PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// What a download returns, by URL.
    struct Served(Arc<HashMap<String, Vec<u8>>>);

    impl Fetch for Served {
        fn get(&self, url: &str) -> Result<Vec<u8>, FontError> {
            self.0.get(url).cloned().ok_or_else(|| FontError::NotFound(url.to_string()))
        }
    }

    /// Fontsource metadata and files for `family` at 400 and 700, upright and italic.
    fn serve(routes: &mut HashMap<String, Vec<u8>>, family: &str) {
        let id = font_id(family);
        let url = |w: u16, style: &str| format!("mock://{id}/latin-{w}-{style}.ttf");
        let variant = |w: u16| serde_json::json!({ "normal": { "latin": { "url": { "ttf": url(w, "normal") } } }, "italic": { "latin": { "url": { "ttf": url(w, "italic") } } } });
        let meta = serde_json::json!({ "id": id, "family": family, "subsets": ["latin"], "license": "OFL-1.1", "variable": false, "variants": { "400": variant(400), "700": variant(700) } });
        routes.insert(metadata_url(family), meta.to_string().into_bytes());
        for w in [400, 700] {
            for style in ["normal", "italic"] {
                // a name of another length cannot be swapped in place, so such a family serves the plain font
                let data = if family.len() == "Roboto Mono".len() { font_named(family) } else { font_named("Roboto Mono") };
                routes.insert(url(w, style), data);
            }
        }
    }

    /// A loader whose downloads are served for each of `families` from memory, into a private cache directory.
    pub fn serving(families: &[&str]) -> FontLoader {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("designos-app-fonts-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        let mut routes = HashMap::new();
        for family in families {
            serve(&mut routes, family);
        }
        let routes = Arc::new(routes);
        // The directory goes when the loader and its last worker are gone.
        let scratch = Scratch(dir);
        FontLoader { open_cache: Arc::new(move || FontCache::with_dir(&scratch.0).with_fetcher(Box::new(Served(routes.clone())))), ..FontLoader::default() }
    }

    /// Waits for the running load and registers what it fetched.
    pub fn settle(fonts: &mut FontLoader) -> FontsLoaded {
        for _ in 0..6000 {
            if let Some(done) = fonts.poll() {
                return done;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("the font load did not finish");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_face_fetched_under_a_substitute_family_is_registered_under_both_names() {
        // "Gotham" is drawn with Montserrat: the document names one, the font database holds the other.
        let mut doc = Document::new("substitute");
        let page = doc.add_page("p");
        let id = doc.alloc_id();
        let style = designos_doc::TextStyle { font_family: "Gotham".into(), ..Default::default() };
        doc.insert(page, designos_doc::Node::text(id, designos_doc::Rect::new(0.0, 0.0, 10.0, 10.0), "hello", style)).unwrap();
        let mut fonts = testkit::serving(&["Montserrat"]);
        fonts.load_for(&doc);
        let done = testkit::settle(&mut fonts);
        assert_eq!((done.loaded, done.failed.is_empty()), (1, true), "{:?}", done.failed);
        for family in ["Gotham", "Montserrat"] {
            assert!(fonts.registered().contains(&FontRequest::new(family, 400, false)), "{family}: {:?}", fonts.registered());
        }
    }

    #[test]
    fn a_substitute_installed_on_the_system_stays_a_system_family() {
        use designos_fonts::LoadedFont;
        let loaded = |doc_family: &str, font_family: &str| LoadedFont {
            request: FontRequest::new(doc_family, 400, false),
            family: font_family.into(),
            data: testkit::font_named(font_family),
        };
        // The substitute is on the system already: only the name the document uses is remembered as fetched.
        designos_render::register_font(testkit::font_named("TestSysSubs"));
        let mut fonts = FontLoader::default();
        fonts.register(loaded("TestDocOne", "TestSysSubs"));
        assert!(fonts.registered().contains(&FontRequest::new("TestDocOne", 400, false)));
        assert!(!fonts.registered().contains(&FontRequest::new("TestSysSubs", 400, false)), "{:?}", fonts.registered());
        let heavier: BTreeSet<_> = [FontRequest::new("TestSysSubs", 700, false)].into();
        assert_eq!(fonts.available(&heavier).len(), 1, "the system still covers every weight of its family");
        // A substitute that only this loader fetched is not a system family.
        fonts.register(loaded("TestDocTwo", "TestFetched"));
        assert!(fonts.registered().contains(&FontRequest::new("TestFetched", 400, false)), "{:?}", fonts.registered());
        let heavier: BTreeSet<_> = [FontRequest::new("TestFetched", 700, false)].into();
        assert!(fonts.available(&heavier).is_empty(), "its bold face still has to be fetched");
    }

    #[test]
    fn a_renamed_font_registers_under_its_new_family() {
        designos_render::register_font(testkit::font_named("TestSanity1"));
        let face = designos_render::resolve_font("TestSanity1", 400, false).expect("a face");
        assert_eq!(face.family, "TestSanity1");
    }
}
