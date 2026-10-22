//! Browser DOM & computed-style capture schema and self-contained ES2020 capture script (`T8.25`, Issue `#276`).

pub mod schema;

pub use schema::*;

/// Self-contained ES2020 browser capture script exposing `window.__designosCapture(options)`.
pub const CAPTURE_JS: &str = include_str!("../js/capture.js");
