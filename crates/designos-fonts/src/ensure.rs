use crate::{bundled_for, requests_for, script_requests_for, substitute, FontCache, FontError, FontRequest};
use designos_doc::Document;

/// A font that was fetched or read from the cache.
#[derive(Debug)]
pub struct LoadedFont {
    /// What the document asked for.
    pub request: FontRequest,
    /// Family actually loaded (after substitution), as it will appear in the
    /// font file's name table.
    pub family: String,
    pub data: Vec<u8>,
}

/// Outcome of [`ensure_fonts`].
#[derive(Debug, Default)]
pub struct FontReport {
    /// Fonts to register with the renderer.
    pub loaded: Vec<LoadedFont>,
    /// Requests served by fonts already embedded in the binary (nothing to do).
    pub bundled: Vec<FontRequest>,
    /// Requests skipped because the caller said the face is already available.
    pub skipped: Vec<FontRequest>,
    /// Requests that could not be satisfied; rendering falls back to Inter.
    pub failed: Vec<(FontRequest, FontError)>,
}

/// Resolve every font `doc` uses (substituting proprietary families), fetching
/// anything missing into `cache`. Blocking; run it on a background thread.
pub fn ensure_fonts(doc: &Document, cache: &FontCache) -> FontReport {
    ensure_fonts_with(doc, cache, |_| false)
}

/// Like [`ensure_fonts`], but faces for which `is_available(request)` is true
/// (e.g. faces of installed system fonts) are skipped.
pub fn ensure_fonts_with(doc: &Document, cache: &FontCache, is_available: impl Fn(&FontRequest) -> bool) -> FontReport {
    // Noto fallbacks for the complex scripts the text uses come with the fonts it names.
    ensure_font_requests_with(requests_for(doc).into_iter().chain(script_requests_for(doc)), cache, is_available)
}

/// Resolves an explicit set of [`FontRequest`]s against bundled fonts,
/// `is_available`, and `cache` without cloning the [`Document`].
pub fn ensure_font_requests_with(
    requests: impl IntoIterator<Item = FontRequest>,
    cache: &FontCache,
    is_available: impl Fn(&FontRequest) -> bool,
) -> FontReport {
    let mut report = FontReport::default();
    for req in requests {
        let family = substitute(&req.family).map(str::to_string).unwrap_or_else(|| req.family.clone());
        let effective = FontRequest::new(family.clone(), req.weight, req.italic);
        if bundled_for(&effective).is_some() {
            report.bundled.push(req);
        } else if is_available(&req) {
            report.skipped.push(req);
        } else {
            match cache.resolve(&effective) {
                Ok(data) => report.loaded.push(LoadedFont { request: req, family, data }),
                Err(e) => report.failed.push((req, e)),
            }
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Node, NodeKind, Rect, TextRun, TextStyle};

    fn doc_with(styles: &[(&str, u16, bool)]) -> Document {
        let mut doc = Document::new("t");
        let page = doc.add_page("p");
        for (fam, w, it) in styles {
            let id = doc.alloc_id();
            let st = TextStyle { font_family: fam.to_string(), font_weight: *w, italic: *it, ..Default::default() };
            doc.insert(page, Node::text(id, Rect::new(0.0, 0.0, 10.0, 10.0), "hi", st)).unwrap();
        }
        doc
    }

    #[test]
    fn requests_collect_nodes_and_runs() {
        let mut doc = doc_with(&[("Inter", 400, false), ("Inter", 400, false), ("Roboto", 700, true)]);
        let id = doc.alloc_id();
        let page = doc.pages[0];
        let mut node = Node::text(id, Rect::new(0.0, 0.0, 10.0, 10.0), "hello", TextStyle::default());
        if let NodeKind::Text(t) = &mut node.kind {
            t.runs.push(TextRun { start: 0, end: 2, style: TextStyle { font_family: "Georgia".into(), font_weight: 600, ..Default::default() } });
        }
        doc.insert(page, node).unwrap();
        let got: Vec<_> = requests_for(&doc).into_iter().collect();
        assert_eq!(got, vec![FontRequest::new("Georgia", 600, false), FontRequest::new("Inter", 400, false), FontRequest::new("Roboto", 700, true),]);
    }

    #[test]
    fn requests_empty_without_text() {
        let doc = Document::new("empty");
        assert!(requests_for(&doc).is_empty());
    }

    #[test]
    fn ensure_substitutes_and_reports_offline_failures() {
        let doc = doc_with(&[("SF Pro Text", 500, false), ("Inter", 700, false), ("Roboto", 400, false), ("Installed", 400, false)]);
        let cache = FontCache::with_dir(std::env::temp_dir().join("designos-fonts-ensure-none")).offline();
        let r = ensure_fonts_with(&doc, &cache, |r| r.family == "Installed");
        assert_eq!(r.bundled.len(), 2); // Inter + SF Pro Text -> Inter
        assert_eq!(r.skipped, vec![FontRequest::new("Installed", 400, false)]);
        assert!(r.loaded.is_empty());
        assert_eq!(r.failed.len(), 1);
        assert_eq!(r.failed[0].0.family, "Roboto");
        assert!(matches!(r.failed[0].1, FontError::Offline(_)));
    }
}
