//! Figma file support.
//! CONTRACT: `import_rest_json`, `import_fig`, `FigError` are used by the app.
//!
//! - [`import_rest_json`]: body of Figma REST `GET /v1/files/:key` (ideally
//!   requested with `geometry=paths`) -> [`Document`].
//! - [`import_fig`]: `.fig` binary (fig-kiwi container, optionally zip-wrapped).
//! - [`export_rest_json`]: [`Document`] -> REST-shaped JSON.
//! - [`image_refs`]: image hashes a document needs fetched separately
//!   (REST `GET /v1/files/:key/images`); `.fig` files embed them already.
//! - `rest` (cargo feature `net`): blocking REST client; the access token is
//!   always supplied by the caller at runtime and never stored or logged.

mod board;
mod clipboard;
pub mod container;
mod dev;
mod export;
mod fig;
mod fig_export;
pub mod kiwi;
pub mod pen;
mod proto;
#[cfg(feature = "net")]
pub mod rest;
mod rest_import;
pub mod svgpath;
#[doc(hidden)]
pub mod testkit;
mod textfmt;
mod variables;
pub mod variables_dtcg;
mod vecnet;

pub use clipboard::{import_figma_clipboard, looks_like_figma_clipboard, parse_figma_clipboard_html, FigClipboard};
pub use designos_doc::Document;
pub use export::export_rest_json;
#[doc(hidden)]
pub use fig::import_fig_node_changes;
pub use fig::{commands_blob_to_path, decode_fig_to_json, import_fig};
pub use fig_export::{export_fig, export_fig_with_report, path_to_commands_blob};
pub use pen::{export_pen_json, import_pen_json, import_pen_json_with_warnings};
pub use proto::{gamepad_button_code, gamepad_button_name, key_code, key_name};
pub use rest_import::{image_refs, import_rest_json};
pub use variables::{export_variables_json, export_variables_meta, import_variables_json, import_variables_value};
pub use variables_dtcg::{
    collect_omitted_extensions, detect_variables_format, detect_variables_format_value, export_variables, import_variables, import_variables_bytes,
    mode_name_from_filename, validate_dtcg_json, validate_dtcg_value, DtcgValidationError, ExportBundle, ImportReport, ImportVariablesOptions, SkippedVariable,
    VariablesFormat,
};

#[derive(Debug, thiserror::Error)]
pub enum FigError {
    #[error("invalid json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("not a fig file: {0}")]
    Format(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
}
