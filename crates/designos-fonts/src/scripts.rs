//! Complex scripts that need a font beyond the bundled Inter.
//!
//! Inter has no Arabic, Hebrew or Devanagari glyphs, and which fallback fonts the system
//! provides varies, so documents that use these scripts get the matching Noto Sans family
//! (OFL, from Fontsource) fetched on demand and registered with the renderer, where it
//! serves as the fallback for characters the text's own font lacks.

use crate::FontRequest;
use designos_doc::{Document, TextProps};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Script {
    Arabic,
    Hebrew,
    Devanagari,
}

impl Script {
    pub const ALL: [Script; 3] = [Script::Arabic, Script::Hebrew, Script::Devanagari];

    /// The open-source family used for the script.
    pub fn family(self) -> &'static str {
        match self {
            Script::Arabic => "Noto Sans Arabic",
            Script::Hebrew => "Noto Sans Hebrew",
            Script::Devanagari => "Noto Sans Devanagari",
        }
    }

    /// Fontsource subset that holds the script's glyphs.
    pub fn subset(self) -> &'static str {
        match self {
            Script::Arabic => "arabic",
            Script::Hebrew => "hebrew",
            Script::Devanagari => "devanagari",
        }
    }
}

/// The script a character belongs to, if it is one of the supported complex scripts.
pub fn script_of(c: char) -> Option<Script> {
    match c as u32 {
        // Arabic, Syriac-free: Arabic, Supplement, Extended-A, Presentation Forms A and B
        0x0600..=0x06FF | 0x0750..=0x077F | 0x08A0..=0x08FF | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF => Some(Script::Arabic),
        0x0590..=0x05FF | 0xFB1D..=0xFB4F => Some(Script::Hebrew),
        0x0900..=0x097F | 0xA8E0..=0xA8FF => Some(Script::Devanagari),
        _ => None,
    }
}

/// Every supported complex script used by `text`.
pub fn scripts_in(text: &str) -> BTreeSet<Script> {
    if text.is_ascii() {
        return BTreeSet::new();
    }
    text.chars().filter_map(script_of).collect()
}

fn text_scripts(t: &TextProps) -> BTreeSet<Script> {
    scripts_in(&t.content)
}

/// Font requests for the Noto family of every complex script the document's text uses, at the
/// weights of the text that uses it.
pub fn script_requests_for(doc: &Document) -> BTreeSet<FontRequest> {
    let mut out = BTreeSet::new();
    let limit = if doc.nodes.len() > 25_000 { 8_192 } else { usize::MAX };
    for node in doc.nodes.values().take(limit) {
        let Some(t) = node.kind.any_text() else { continue };
        if t.content.is_ascii() {
            continue;
        }
        for script in text_scripts(t) {
            let mut weights = BTreeSet::from([t.style.font_weight]);
            weights.extend(t.runs.iter().map(|r| r.style.font_weight));
            for w in weights {
                out.insert(FontRequest::new(script.family(), w, false));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Node, NodeKind, Rect, TextRun, TextStyle};

    #[test]
    fn classifies_scripts() {
        assert_eq!(script_of('\u{0645}'), Some(Script::Arabic));
        assert_eq!(script_of('\u{FEFB}'), Some(Script::Arabic));
        assert_eq!(script_of('\u{05D0}'), Some(Script::Hebrew));
        assert_eq!(script_of('\u{0928}'), Some(Script::Devanagari));
        assert_eq!(script_of('a'), None);
        assert_eq!(script_of('\u{4E2D}'), None);
        assert_eq!(scripts_in("abc \u{05D0} \u{0645}\u{0928}"), BTreeSet::from([Script::Arabic, Script::Hebrew, Script::Devanagari]));
        assert!(scripts_in("plain text 123").is_empty());
    }

    #[test]
    fn requests_follow_the_text_that_uses_the_script() {
        let mut doc = Document::new("t");
        let page = doc.add_page("p");
        let a = doc.alloc_id();
        doc.insert(page, Node::text(a, Rect::new(0.0, 0.0, 10.0, 10.0), "hello", TextStyle::default())).unwrap();
        assert!(script_requests_for(&doc).is_empty(), "Latin text needs nothing extra");
        let b = doc.alloc_id();
        let mut n = Node::text(b, Rect::new(0.0, 0.0, 10.0, 10.0), "\u{05E9}\u{05DC}\u{05D5}\u{05DD} \u{0645}", TextStyle::default());
        if let NodeKind::Text(t) = &mut n.kind {
            t.runs.push(TextRun { start: 0, end: 2, style: TextStyle { font_weight: 700, ..Default::default() } });
        }
        doc.insert(page, n).unwrap();
        let got: Vec<_> = script_requests_for(&doc).into_iter().collect();
        assert_eq!(
            got,
            vec![
                FontRequest::new("Noto Sans Arabic", 400, false),
                FontRequest::new("Noto Sans Arabic", 700, false),
                FontRequest::new("Noto Sans Hebrew", 400, false),
                FontRequest::new("Noto Sans Hebrew", 700, false),
            ]
        );
    }

    use crate::fontsource::metadata_url;
    use crate::{ensure_fonts_with, Fetch, FontCache, FontError};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    const NOTO_HEBREW: &str = r#"{"id":"noto-sans-hebrew","family":"Noto Sans Hebrew","subsets":["hebrew","latin"],"license":"OFL-1.1",
        "variants":{"400":{"normal":{"hebrew":{"url":{"ttf":"https://cdn.test/hebrew-400-normal.ttf"}},"latin":{"url":{"ttf":"https://cdn.test/latin-400-normal.ttf"}}}}}}"#;

    struct Mock {
        routes: HashMap<String, Vec<u8>>,
        hits: Arc<Mutex<Vec<String>>>,
    }

    impl Fetch for Mock {
        fn get(&self, url: &str) -> Result<Vec<u8>, FontError> {
            self.hits.lock().unwrap().push(url.to_string());
            self.routes.get(url).cloned().ok_or_else(|| FontError::NotFound(url.to_string()))
        }
    }

    fn mock_cache(dir: &std::path::Path, hits: Arc<Mutex<Vec<String>>>) -> FontCache {
        let mut routes = HashMap::new();
        routes.insert(metadata_url("Noto Sans Hebrew"), NOTO_HEBREW.as_bytes().to_vec());
        let mut ttf = vec![0, 1, 0, 0];
        ttf.extend_from_slice(&[9u8; 64]);
        routes.insert("https://cdn.test/hebrew-400-normal.ttf".into(), ttf.clone());
        routes.insert("https://cdn.test/latin-400-normal.ttf".into(), ttf);
        FontCache::with_dir(dir).with_fetcher(Box::new(Mock { routes, hits }))
    }

    #[test]
    fn script_fonts_come_from_their_own_subset_via_a_temp_cache() {
        let dir = std::env::temp_dir().join(format!("designos-script-fonts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let hits = Arc::new(Mutex::new(Vec::new()));
        let cache = mock_cache(&dir, hits.clone());
        let mut doc = Document::new("t");
        let page = doc.add_page("p");
        let id = doc.alloc_id();
        doc.insert(page, Node::text(id, Rect::new(0.0, 0.0, 10.0, 10.0), "\u{05E9}\u{05DC}\u{05D5}\u{05DD}", TextStyle::default())).unwrap();

        let report = ensure_fonts_with(&doc, &cache, |_| false);
        assert_eq!(report.bundled.len(), 1, "Inter itself is bundled");
        assert_eq!(report.loaded.len(), 1, "{:?}", report.failed);
        assert_eq!(report.loaded[0].family, "Noto Sans Hebrew");
        let urls = hits.lock().unwrap().clone();
        assert!(urls.iter().any(|u| u.ends_with("hebrew-400-normal.ttf")) && !urls.iter().any(|u| u.contains("latin-400")), "{urls:?}");
        let side = cache.sidecar(&FontRequest::new("Noto Sans Hebrew", 400, false)).unwrap();
        assert_eq!(side.license, "OFL-1.1");
        assert!(dir.join("noto-sans-hebrew-400-normal.ttf").exists());

        // a second pass is served from the disk cache, even offline
        let again = ensure_fonts_with(&doc, &FontCache::with_dir(&dir).offline(), |_| false);
        assert_eq!(again.loaded.len(), 1);
        // and an installed copy is skipped
        let skipped = ensure_fonts_with(&doc, &FontCache::with_dir(&dir).offline(), |r| r.family == "Noto Sans Hebrew");
        assert!(skipped.loaded.is_empty() && skipped.skipped.iter().any(|r| r.family == "Noto Sans Hebrew"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_script_font_offline_is_a_reported_failure() {
        let mut doc = Document::new("t");
        let page = doc.add_page("p");
        let id = doc.alloc_id();
        doc.insert(page, Node::text(id, Rect::new(0.0, 0.0, 10.0, 10.0), "\u{0928}\u{092E}\u{0938}\u{094D}\u{0924}\u{0947}", TextStyle::default())).unwrap();
        let dir = std::env::temp_dir().join(format!("designos-script-fonts-offline-{}", std::process::id()));
        let r = ensure_fonts_with(&doc, &FontCache::with_dir(&dir).offline(), |_| false);
        assert_eq!(r.failed.len(), 1);
        assert_eq!(r.failed[0].0.family, "Noto Sans Devanagari");
    }
}
