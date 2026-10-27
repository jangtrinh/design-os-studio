//! Shared tool validation helpers, named error catalogue (`tools::errors`), batched transaction dispatcher (`tools::batch`),
//! per-call lint issues block (`tools::issues`), and session options (`tools::session_options`).

pub mod batch;
pub mod bootstrap;
pub mod design_context;
pub mod errors;
pub mod fill_write;
pub mod guidelines;
pub mod issues;
pub mod ledger;
pub use ledger as idempotency;
pub mod ledger_reconnect;
#[cfg(test)]
mod ledger_tests;
pub mod libraries;
pub mod library_source;
pub mod metadata;
pub mod query;
pub mod screenshot;
pub mod search_design_system;
pub mod session_options;

use crate::tools::errors::{ErrorCode, ToolError};
use designos_doc::{Document, Node, NodeId, NodeKind, Paint, Rect, SpecWarning, TextAutoResize, TextStyle, Vec2};
use serde_json::json;

pub(crate) fn bounded_text(field: &str, value: &str, max: usize) -> Result<(), ToolError> {
    if value.len() > max {
        Err(ToolError::new(ErrorCode::TextTooLong, format!("/{field}"), format!("{field} exceeds {max} UTF-8 bytes"))
            .with_received(json!(value.len()))
            .with_expected(format!("<= {max} UTF-8 bytes")))
    } else {
        Ok(())
    }
}

pub(crate) fn geometry(x: f32, y: f32, width: f32, height: f32) -> Result<(), ToolError> {
    for (field, v) in [("x", x), ("y", y), ("width", width), ("height", height)] {
        if !v.is_finite() || v.abs() > 1_000_000.0 {
            return Err(ToolError::new(ErrorCode::InvalidGeometry, format!("/{field}"), "geometry must be finite and within +/-1000000")
                .with_expected("finite number within -1000000..=1000000"));
        }
    }
    if width < 0.0 || height < 0.0 {
        let field = if width < 0.0 { "/width" } else { "/height" };
        let bad = if width < 0.0 { width } else { height };
        return Err(ToolError::new(ErrorCode::InvalidGeometry, field, "width and height must be non-negative").with_received(json!(bad)).with_expected(">= 0"));
    }
    Ok(())
}

/// Sets the paints `node` is drawn with, and cuts the variable bindings and the fill style link of the
/// paints it replaces, as `apply_spec` does for a literal colour. Left in place they would be applied
/// over the new paints, so the canvas would keep drawing the old colour.
///
/// Every renderer reads a text node's colour from its text style (`style.fills`, and each run's), never from
/// `Node::fills`, so a text node takes the paints there and keeps none of its own. This is the rule
/// `designos_doc::apply_style_values` follows when a colour style is applied to text.
pub(crate) fn set_node_fills(node: &mut Node, paints: Vec<Paint>) {
    designos_doc::detach_fill_paints(node);
    match &mut node.kind {
        NodeKind::Text(t) => {
            for run in &mut t.runs {
                run.style.fills = paints.clone();
            }
            t.style.fills = paints;
            node.fills.clear();
        }
        _ => node.fills = paints,
    }
}

pub(crate) fn validate_create_fields(
    kind: &str,
    name: Option<&str>,
    text: Option<&str>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> Result<fn(NodeId, Rect) -> Node, ToolError> {
    geometry(x, y, width, height)?;
    if let Some(name) = name {
        bounded_text("name", name, 1024)?;
    }
    if let Some(text) = text {
        bounded_text("text", text, 100_000)?;
    }
    if kind != "text" && text.is_some() {
        return Err(ToolError::new(ErrorCode::NotATextNode, "/text", "text can only be set on text nodes")
            .with_received(json!(kind))
            .with_expected("kind == \"text\""));
    }
    match kind {
        "frame" => Ok(Node::frame),
        "rectangle" => Ok(Node::rectangle),
        "ellipse" => Ok(Node::ellipse),
        "text" => Ok(|id, r| Node::text(id, r, "", TextStyle::default())),
        other => Err(ToolError::new(ErrorCode::UnsupportedNodeKind, "/kind", "kind must be frame, rectangle, ellipse, or text")
            .with_received(json!(other))
            .with_valid_options(vec!["frame", "rectangle", "ellipse", "text"])
            .with_corrected_example(json!({"kind": "frame"}))),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_created_node(
    doc: &mut Document,
    parent_id: NodeId,
    index: Option<usize>,
    kind: &str,
    name: Option<String>,
    text: Option<String>,
    x: f32,
    y: f32,
    width: Option<f32>,
    height: Option<f32>,
) -> Result<(NodeId, usize, Node), ToolError> {
    if parent_id.0 == 0 {
        return Err(ToolError::new(ErrorCode::ValueOutOfRange, "/parent_id", "node IDs must be positive integers")
            .with_received(json!(0))
            .with_expected(">= 1"));
    }
    let parent = doc
        .get(parent_id)
        .ok_or_else(|| ToolError::new(ErrorCode::NodeNotFound, "/parent_id", format!("node {} not found", parent_id.0)).with_received(json!(parent_id.0)))?;
    if !parent.kind.is_container() {
        return Err(ToolError::new(ErrorCode::NotAContainer, "/parent_id", "parent_id must identify a container")
            .with_received(json!(parent_id.0))
            .with_expected("page, frame, component, section, or boolean group"));
    }
    let children_len = parent.children.len();
    let target_index = index.unwrap_or(children_len).min(children_len);
    let init_w = width.unwrap_or(100.0);
    let init_h = height.unwrap_or(100.0);
    let constructor = validate_create_fields(kind, name.as_deref(), text.as_deref(), x, y, init_w, init_h)?;
    let rect = Rect::new(x, y, init_w, init_h);
    let id = doc.try_alloc_id().ok_or_else(|| ToolError::new(ErrorCode::IdExhausted, "", "Document has exhausted node IDs"))?;
    let mut node = constructor(id, rect);
    if let Some(name) = name {
        node.name = name;
    }
    if let NodeKind::Text(props) = &mut node.kind {
        if let Some(text) = text {
            props.content = text;
        }
        match (width, height) {
            (None, None) => {
                props.auto_resize = TextAutoResize::WidthAndHeight;
                node.size = designos_doc::text_measure::intrinsic_text_size(props, None);
            }
            (Some(w), None) => {
                props.auto_resize = TextAutoResize::Height;
                let h = designos_doc::text_measure::intrinsic_text_size(props, Some(w.max(1.0))).y;
                node.size = Vec2::new(w, h);
            }
            (None, Some(h)) => {
                props.auto_resize = TextAutoResize::WidthAndHeight;
                let w = designos_doc::text_measure::intrinsic_text_size(props, None).x;
                node.size = Vec2::new(w, h);
            }
            (Some(w), Some(h)) => {
                props.auto_resize = TextAutoResize::None;
                node.size = Vec2::new(w, h);
            }
        }
    }
    Ok((id, target_index, node))
}

#[derive(Clone, Debug, Default)]
pub(crate) struct UpdateFields {
    pub name: Option<String>,
    pub text: Option<String>,
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub visible: Option<bool>,
    pub opacity: Option<f32>,
    pub fill: Option<String>,
    pub fills: Option<Vec<Paint>>,
}

pub(crate) fn validate_update_fields(u: &UpdateFields) -> Result<(), ToolError> {
    if let Some(name) = &u.name {
        bounded_text("name", name, 1024)?;
    }
    if let Some(x) = u.x {
        if !x.is_finite() || x.abs() > 1_000_000.0 {
            return Err(ToolError::new(ErrorCode::InvalidGeometry, "/x", "geometry must be finite and within +/-1000000")
                .with_expected("finite number within -1000000..=1000000"));
        }
    }
    if let Some(y) = u.y {
        if !y.is_finite() || y.abs() > 1_000_000.0 {
            return Err(ToolError::new(ErrorCode::InvalidGeometry, "/y", "geometry must be finite and within +/-1000000")
                .with_expected("finite number within -1000000..=1000000"));
        }
    }
    if let Some(w) = u.width {
        if !w.is_finite() || w.abs() > 1_000_000.0 {
            return Err(ToolError::new(ErrorCode::InvalidGeometry, "/width", "geometry must be finite and within +/-1000000")
                .with_expected("finite number within 0..=1000000"));
        }
        if w < 0.0 {
            return Err(ToolError::new(ErrorCode::InvalidGeometry, "/width", "width and height must be non-negative")
                .with_received(json!(w))
                .with_expected(">= 0"));
        }
    }
    if let Some(h) = u.height {
        if !h.is_finite() || h.abs() > 1_000_000.0 {
            return Err(ToolError::new(ErrorCode::InvalidGeometry, "/height", "geometry must be finite and within +/-1000000")
                .with_expected("finite number within 0..=1000000"));
        }
        if h < 0.0 {
            return Err(ToolError::new(ErrorCode::InvalidGeometry, "/height", "width and height must be non-negative")
                .with_received(json!(h))
                .with_expected(">= 0"));
        }
    }
    if let Some(opacity) = u.opacity {
        if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
            return Err(ToolError::new(ErrorCode::ValueOutOfRange, "/opacity", "opacity must be between 0 and 1")
                .with_received(json!(opacity))
                .with_expected("0.0..=1.0"));
        }
    }
    if let Some(text) = &u.text {
        bounded_text("text", text, 100_000)?;
    }
    fill_write::validate_fill_color(u.fill.as_deref())
}

pub(crate) fn build_updated_node(doc: &mut Document, id: NodeId, u: UpdateFields) -> Result<(Node, bool, Vec<SpecWarning>), ToolError> {
    if id.0 == 0 {
        return Err(ToolError::new(ErrorCode::ValueOutOfRange, "/node_id", "node IDs must be positive integers").with_received(json!(0)).with_expected(">= 1"));
    }
    let mut node =
        doc.get(id).ok_or_else(|| ToolError::new(ErrorCode::NodeNotFound, "/node_id", format!("node {} not found", id.0)).with_received(json!(id.0)))?.clone();
    if node.parent.is_none() {
        return Err(ToolError::new(ErrorCode::PageRootProtected, "/node_id", "page roots cannot be updated by this tool").with_received(json!(id.0)));
    }
    validate_update_fields(&u)?;
    if let Some(name) = u.name {
        node.name = name;
    }
    if let Some(x) = u.x {
        node.transform.tx = x;
    }
    if let Some(y) = u.y {
        node.transform.ty = y;
    }
    if let Some(width) = u.width {
        node.size.x = width;
    }
    if let Some(height) = u.height {
        node.size.y = height;
    }
    if let NodeKind::Text(props) = &mut node.kind {
        if u.height.is_some() {
            if matches!(props.auto_resize, designos_doc::TextAutoResize::WidthAndHeight | designos_doc::TextAutoResize::Height) {
                props.auto_resize = designos_doc::TextAutoResize::None;
            }
        } else if u.width.is_some() && props.auto_resize == designos_doc::TextAutoResize::WidthAndHeight {
            props.auto_resize = designos_doc::TextAutoResize::Height;
        }
    }
    geometry(node.transform.tx, node.transform.ty, node.size.x, node.size.y)?;
    if let Some(visible) = u.visible {
        node.visible = visible;
    }
    if let Some(opacity) = u.opacity {
        node.opacity = opacity;
    }
    let size_omitted = u.width.is_none() && u.height.is_none();
    let mut warnings = Vec::new();
    fill_write::write_fill(doc, &mut node, id, fill_write::FillTarget::ExistingLayer, u.fill.as_deref(), u.fills, &mut warnings)?;
    if let Some(text) = u.text {
        let NodeKind::Text(props) = &mut node.kind else {
            return Err(ToolError::new(ErrorCode::NotATextNode, "/text", "text can only be set on text nodes").with_received(json!(node.id.0)));
        };
        if props.content != text {
            props.content = text;
            props.runs.clear();
            props.lists.clear();
            if size_omitted {
                match props.auto_resize {
                    TextAutoResize::WidthAndHeight => {
                        node.size = designos_doc::text_measure::intrinsic_text_size(props, None);
                    }
                    TextAutoResize::Height => {
                        node.size.y = designos_doc::text_measure::intrinsic_text_size(props, Some(node.size.x.max(20.0))).y;
                    }
                    TextAutoResize::None | TextAutoResize::Truncate => {}
                }
            }
        }
    }
    let changed = doc.get(id) != Some(&node);
    Ok((node, changed, warnings))
}
