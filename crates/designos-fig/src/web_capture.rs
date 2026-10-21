//! Web capture (`.webcapture.json` / `.capture.json`) importer bridge for `designos-fig`.
//!
//! Delegates to [`designos_webcapture`] while providing `FigError`-compatible helpers
//! and format detection for `designos-app` (`io::open_any`) and `designos-mcp`.

use crate::FigError;
use designos_doc::{Command, Document, NodeId};
pub use designos_webcapture::{
    import_capture, import_capture_into_document, import_capture_json, AbsoluteFallback, Capture, FontSubstitution, ImportError, ImportOptions, ImportReport,
    UncapturedImage, UnsupportedCssEntry, CAPTURE_SCHEMA_ID,
};

/// Returns `true` if `json` has a top-level `"schema"` field matching `designos-webcapture`.
pub fn looks_like_web_capture_json(json: &str) -> bool {
    #[derive(serde::Deserialize)]
    struct SchemaProbe {
        schema: Option<String>,
    }
    let trimmed = json.trim_start_matches('\u{feff}').trim_start();
    if !trimmed.starts_with('{') {
        return false;
    }
    serde_json::from_str::<SchemaProbe>(trimmed)
        .ok()
        .and_then(|p| p.schema)
        .is_some_and(|s| matches!(s.as_str(), "designos-webcapture" | "designos-web-capture/v1" | "designos-webcapture/v1"))
}

/// Imports a `.webcapture.json` / `.capture.json` payload into a standalone [`Document`]
/// and returns both the document and the [`ImportReport`].
pub fn import_web_capture_json_with_report(json: &str, opts: &ImportOptions) -> Result<(Document, ImportReport), FigError> {
    let capture = Capture::from_json(json).map_err(|e| FigError::Format(e.to_string()))?;
    import_capture_into_document(&capture, opts).map_err(|e| FigError::Format(e.to_string()))
}

/// Imports a `.webcapture.json` / `.capture.json` payload into a standalone [`Document`].
pub fn import_web_capture_json(json: &str) -> Result<Document, FigError> {
    let (doc, _) = import_web_capture_json_with_report(json, &ImportOptions::default())?;
    Ok(doc)
}

/// Builds a single undoable [`Command`] + [`ImportReport`] to import `json` onto `page` in `doc`.
pub fn import_web_capture_into_page(doc: &Document, page: NodeId, json: &str, opts: &ImportOptions) -> Result<(Command, ImportReport), FigError> {
    import_capture_json(doc, page, json, opts).map_err(|e| FigError::Format(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_and_imports_web_capture_bundle() {
        let json = r#"{
            "schema": "designos-webcapture",
            "version": 1,
            "url": "https://designos.dev/demo",
            "title": "Demo Page",
            "captured_at": "2026-04-20T00:00:00Z",
            "viewport": { "width": 800, "height": 600, "dpr": 2 },
            "scroll": { "width": 800, "height": 600 },
            "truncated": false,
            "root": {
                "tag": "html",
                "box": { "x": 0, "y": 0, "width": 800, "height": 600 },
                "styles": { "display": "block", "background-color": "rgb(255, 255, 255)" },
                "children": [
                    {
                        "tag": "h1",
                        "id": "hero",
                        "box": { "x": 32, "y": 32, "width": 400, "height": 36 },
                        "styles": { "display": "block", "font-family": "Inter", "font-size": "28px", "font-weight": "700", "color": "rgb(15, 23, 42)" },
                        "text_runs": [
                            {
                                "text": "Hello from Web Capture",
                                "styles": { "font-family": "Inter", "font-size": "28px", "font-weight": "700", "color": "rgb(15, 23, 42)" },
                                "lines": [{ "x": 32, "y": 32, "width": 320, "height": 34 }]
                            }
                        ],
                        "children": []
                    }
                ]
            }
        }"#;
        assert!(looks_like_web_capture_json(json));
        let (doc, report) = import_web_capture_json_with_report(json, &ImportOptions::default()).unwrap();
        assert_eq!(report.root_name, "Demo Page \u{2014} designos.dev");
        assert!(doc.get(report.root_node_id).is_some());
    }

    #[test]
    fn looks_like_web_capture_ignores_nested_substring() {
        let figma_rest_with_mention = r#"{"document":{"id":"0:0","name":"designos-webcapture"}}"#;
        assert!(!looks_like_web_capture_json(figma_rest_with_mention), "nested substring must not trigger web capture detection");
        let valid_top_level = r#"{"schema":"designos-webcapture","version":1}"#;
        assert!(looks_like_web_capture_json(valid_top_level));
    }
}
