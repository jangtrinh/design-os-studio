//! Shared tool validation helpers, named error catalogue (`tools::errors`), batched transaction dispatcher (`tools::batch`),
//! per-call lint issues block (`tools::issues`), and session options (`tools::session_options`).

pub mod batch;
pub mod design_context;
pub mod errors;
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
pub mod screenshot;
pub mod search_design_system;
pub mod session_options;

use crate::tools::errors::{ErrorCode, ToolError};
use designos_doc::{resolve_easeui_color, Document, Node, NodeId, NodeKind, Paint, Rect, TextStyle};
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

pub(crate) fn validate_fill_color(fill: Option<&str>) -> Result<Option<Paint>, ToolError> {
    let Some(f) = fill else { return Ok(None) };
    let c = resolve_easeui_color(f).ok_or_else(|| {
        ToolError::new(ErrorCode::ValueOutOfRange, "/fill", format!("invalid fill color '{f}'"))
            .with_received(json!(f))
            .with_expected("valid #RRGGBB/#RRGGBBAA hex or $easeui.* color token")
    })?;
    Ok(Some(Paint::solid(c)))
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
    width: f32,
    height: f32,
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
    let constructor = validate_create_fields(kind, name.as_deref(), text.as_deref(), x, y, width, height)?;
    let rect = Rect::new(x, y, width, height);
    let id = doc.try_alloc_id().ok_or_else(|| ToolError::new(ErrorCode::IdExhausted, "", "Document has exhausted node IDs"))?;
    let mut node = constructor(id, rect);
    if let Some(name) = name {
        node.name = name;
    }
    if let (Some(text), NodeKind::Text(props)) = (text, &mut node.kind) {
        props.content = text;
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
    validate_fill_color(u.fill.as_deref())?;
    Ok(())
}

pub(crate) fn build_updated_node(doc: &Document, id: NodeId, u: UpdateFields) -> Result<(Node, bool), ToolError> {
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
    geometry(node.transform.tx, node.transform.ty, node.size.x, node.size.y)?;
    if let Some(visible) = u.visible {
        node.visible = visible;
    }
    if let Some(opacity) = u.opacity {
        node.opacity = opacity;
    }
    if let Some(paint) = validate_fill_color(u.fill.as_deref())? {
        node.fills = vec![paint];
    }
    if let Some(fills) = u.fills {
        node.fills = fills;
    }
    if let Some(text) = u.text {
        let NodeKind::Text(props) = &mut node.kind else {
            return Err(ToolError::new(ErrorCode::NotATextNode, "/text", "text can only be set on text nodes").with_received(json!(node.id.0)));
        };
        if props.content != text {
            props.content = text;
            props.runs.clear();
            props.lists.clear();
        }
    }
    let changed = doc.get(id) != Some(&node);
    Ok((node, changed))
}
