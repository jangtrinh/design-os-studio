//! Serde schema for `designos-webcapture` envelopes (`T8.25` §3.10, §4, §5).
//!
//! All structs leave `deny_unknown_fields` off for forward compatibility while
//! [`Capture::from_json`] enforces the schema identifier and major version (`1..=2`).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

/// Canonical schema identifier in `Capture::schema`.
pub const CAPTURE_SCHEMA_ID: &str = "designos-webcapture";

/// Current capture format version emitted by `js/capture.js`.
pub const CAPTURE_VERSION: u32 = 1;

fn default_schema() -> String {
    CAPTURE_SCHEMA_ID.to_string()
}

fn default_version() -> u32 {
    CAPTURE_VERSION
}

fn default_true() -> bool {
    true
}

fn is_true(v: &bool) -> bool {
    *v
}

fn is_false(v: &bool) -> bool {
    !*v
}

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("invalid capture JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unexpected capture schema '{0}' (expected 'designos-webcapture')")]
    UnsupportedSchema(String),
    #[error("unsupported capture version {0} (supported: 1..=2)")]
    UnsupportedCaptureVersion(u32),
}

/// Top-level web capture envelope (`T8.25` §3.10, §4).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Capture {
    #[serde(default = "default_schema")]
    pub schema: String,
    #[serde(default = "default_version", alias = "capture_version")]
    pub version: u32,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub viewport: Viewport,
    #[serde(default)]
    pub scroll: ScrollSize,
    #[serde(default)]
    pub captured_at: String,
    #[serde(default)]
    pub fonts: Vec<FontFace>,
    /// Stylesheet URLs whose rules could not be read due to cross-origin restrictions (`T8.25` §3.7).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub css_vars_unavailable: Vec<String>,
    pub root: Element,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default)]
    pub stats: CaptureStats,
    /// Optional theme names for multi-theme captures (`E15-43`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub themes: Vec<String>,
    /// Optional theme -> selector -> custom property map (`E15-43`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub custom_properties: BTreeMap<String, BTreeMap<String, BTreeMap<String, String>>>,
}

impl Capture {
    /// Parses a `.capture.json` payload and validates its schema and major version (`T8.25` §4, §5).
    pub fn from_json(json: &str) -> Result<Self, CaptureError> {
        let raw: serde_json::Value = serde_json::from_str(json)?;
        if let Some(v) = raw.get("version").or_else(|| raw.get("capture_version")).and_then(|v| v.as_u64()) {
            let ver = u32::try_from(v).unwrap_or(u32::MAX);
            if ver == 0 || ver > 2 {
                return Err(CaptureError::UnsupportedCaptureVersion(ver));
            }
        }
        let capture: Self = serde_json::from_value(raw)?;
        if !capture.schema.is_empty()
            && capture.schema != CAPTURE_SCHEMA_ID
            && capture.schema != "designos-web-capture/v1"
            && capture.schema != "designos-webcapture/v1"
        {
            return Err(CaptureError::UnsupportedSchema(capture.schema));
        }
        if capture.version == 0 || capture.version > 2 {
            return Err(CaptureError::UnsupportedCaptureVersion(capture.version));
        }
        Ok(capture)
    }

    /// Serialises the capture envelope to pretty JSON.
    pub fn to_json_pretty(&self) -> Result<String, CaptureError> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

/// Viewport dimensions and device pixel ratio at capture time (`T8.25` §3.10).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Viewport {
    #[serde(default)]
    pub width: f32,
    #[serde(default)]
    pub height: f32,
    #[serde(default = "default_dpr")]
    pub dpr: f32,
}

fn default_dpr() -> f32 {
    1.0
}

/// Full scrollable page dimensions (`T8.25` §3.10).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ScrollSize {
    #[serde(default)]
    pub width: f32,
    #[serde(default)]
    pub height: f32,
}

/// Axis-aligned rectangle in page coordinates (`T8.25` §3.3, §3.5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BoxRect {
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default)]
    pub width: f32,
    #[serde(default)]
    pub height: f32,
}

/// Loaded font face metadata recorded from `document.fonts` / `@font-face` (`T8.25` §3.10).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontFace {
    #[serde(default)]
    pub family: String,
    #[serde(default)]
    pub weight: String,
    #[serde(default)]
    pub style: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,
}

/// Summary statistics for a page capture (`T8.25` §3.9, §3.10).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CaptureStats {
    #[serde(default)]
    pub elements: usize,
    #[serde(default)]
    pub skipped_elements: usize,
    #[serde(default)]
    pub text_runs: usize,
    #[serde(default)]
    pub images: usize,
    #[serde(default)]
    pub image_bytes: usize,
    #[serde(default)]
    pub elapsed_ms: f64,
}

/// Captured DOM element (`T8.25` §3.3–§3.8).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Element {
    #[serde(default)]
    pub tag: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub classes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aria_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
    #[serde(rename = "box", default)]
    pub border_box: BoxRect,
    #[serde(default)]
    pub content_box: BoxRect,
    /// Dense paint index in stacking-context order (`T8.25` §3.2). The raw `z-index` stays in
    /// `styles`. Stored as `i64` so an older capture that folded a huge `z-index` into this field
    /// still loads.
    #[serde(default)]
    pub z: i64,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub visible: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub styles: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pseudo: Option<Pseudo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub text_runs: Vec<TextRun>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<ImageRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<ImageRef>,
    /// CSS custom properties (`--*`) declared or overridden on this element (`T8.25` §3.7a).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub css_vars_declared: BTreeMap<String, String>,
    /// For each captured property whose winning declaration uses `var(--x, ...)`, the chain of
    /// custom property names referenced (`T8.25` §3.7b).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub css_var_refs: BTreeMap<String, Vec<String>>,
    /// Raw winning declaration source string per property when using `var(...)` (`E15-43`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub declaration_sources: BTreeMap<String, String>,
    /// Nearest framework component name from React fiber, Vue, Svelte, Angular, or `data-component` (`T8.25` §3.8).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component_hint: Option<String>,
    /// Alias populated alongside `component_hint` for `E15-44` component detection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub framework_name: Option<String>,
    /// True when an open Shadow DOM root was walked under this host (`T8.25` §8.1).
    #[serde(default, skip_serializing_if = "is_false")]
    pub shadow: bool,
    /// True when a closed Shadow DOM root was detected on this host (`T8.25` §8.1).
    #[serde(default, skip_serializing_if = "is_false")]
    pub closed_shadow: bool,
    /// True when this element is a cross-origin `<iframe>` recorded as an opaque box (`T8.25` §8.2).
    #[serde(default, skip_serializing_if = "is_false")]
    pub cross_origin: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Element>,
}

/// Captured `::before` and `::after` pseudo-elements when they render content or a box (`T8.25` §3.3).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Pseudo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<PseudoBox>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<PseudoBox>,
}

/// A single rendered `::before` or `::after` pseudo-element (`T8.25` §3.3).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PseudoBox {
    #[serde(default)]
    pub content: String,
    #[serde(rename = "box", default)]
    pub border_box: BoxRect,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub styles: BTreeMap<String, String>,
}

/// Contiguous styled text run with its exact wrapped line boxes from `Range.getClientRects()` (`T8.25` §3.5).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TextRun {
    #[serde(default)]
    pub text: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub styles: BTreeMap<String, String>,
    #[serde(default)]
    pub lines: Vec<BoxRect>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub lines_truncated: bool,
}

/// Captured bitmap or vector image reference (`T8.25` §3.6, §8.4).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageRef {
    #[serde(default)]
    pub src: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub svg: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub blocked: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub pending: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}
