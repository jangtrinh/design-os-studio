//! Browser DOM & computed-style capture schema, self-contained ES2020 capture script (`T8.25`),
//! and editable auto-layout importer (`T8.27`).

pub mod import;
pub mod layout_infer;
pub mod paint_map;
pub mod schema;
pub mod text_map;

pub use import::{
    element_selector_label, import_capture, import_capture_into_document, import_capture_json, AbsoluteFallback, FontSubstitution, ImportError, ImportOptions,
    ImportReport, UncapturedImage, UnsupportedCssEntry,
};
pub use schema::*;

/// Self-contained ES2020 browser capture script exposing `window.__designosCapture(options)`.
pub const CAPTURE_JS: &str = include_str!("../js/capture.js");
