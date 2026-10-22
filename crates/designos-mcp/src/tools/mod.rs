//! Shared tool validation helpers, batched transaction dispatcher (`tools::batch`),
//! frame lease write guard (`tools::lease`), multi-agent orchestration (`tools::agents`),
//! and modular MCP tool implementations (`v0.6 AI design`).

pub mod agents;
pub mod batch;
pub mod guidelines;
pub mod lease;
pub mod libraries;
pub mod library_source;
pub mod metadata;
pub mod screenshot;

use designos_doc::{resolve_easeui_color, Document, Node, NodeId, NodeKind, Paint, Rect, TextStyle};

pub(crate) fn bounded_text(field: &str, value: &str, max: usize) -> Result<(), String> {
    if value.len() > max {
        Err(format!("{field} exceeds {max} UTF-8 bytes"))
    } else {
        Ok(())
    }
}

pub(crate) fn geometry(x: f32, y: f32, width: f32, height: f32) -> Result<(), String> {
    if [x, y, width, height].iter().any(|v| !v.is_finite() || v.abs() > 1_000_000.0) {
        return Err("geometry must be finite and within +/-1000000".into());
    }
    if width < 0.0 || height < 0.0 {
        return Err("width and height must be non-negative".into());
    }
    Ok(())
}

pub(crate) fn validate_fill_color(fill: Option<&str>) -> Result<Option<Paint>, String> {
    let Some(f) = fill else { return Ok(None) };
    let c = resolve_easeui_color(f).ok_or_else(|| format!("invalid fill color '{f}'"))?;
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
) -> Result<fn(NodeId, Rect) -> Node, String> {
    geometry(x, y, width, height)?;
    if let Some(name) = name {
        bounded_text("name", name, 1024)?;
    }
    if let Some(text) = text {
        bounded_text("text", text, 100_000)?;
    }
    if kind != "text" && text.is_some() {
        return Err("text can only be set on text nodes".into());
    }
    match kind {
        "frame" => Ok(Node::frame),
        "rectangle" => Ok(Node::rectangle),
        "ellipse" => Ok(Node::ellipse),
        "text" => Ok(|id, r| Node::text(id, r, "", TextStyle::default())),
        _ => Err("kind must be frame, rectangle, ellipse, or text".into()),
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
) -> Result<(NodeId, usize, Node), String> {
    if parent_id.0 == 0 {
        return Err("node IDs must be positive integers".into());
    }
    let parent = doc.get(parent_id).ok_or_else(|| format!("node {} not found", parent_id.0))?;
    if !parent.kind.is_container() {
        return Err("parent_id must identify a container".into());
    }
    let children_len = parent.children.len();
    let target_index = index.unwrap_or(children_len).min(children_len);
    let constructor = validate_create_fields(kind, name.as_deref(), text.as_deref(), x, y, width, height)?;
    let rect = Rect::new(x, y, width, height);
    let id = doc.try_alloc_id().ok_or("Document has exhausted node IDs")?;
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

pub(crate) fn validate_update_fields(u: &UpdateFields) -> Result<(), String> {
    if let Some(name) = &u.name {
        bounded_text("name", name, 1024)?;
    }
    if let Some(x) = u.x {
        if !x.is_finite() || x.abs() > 1_000_000.0 {
            return Err("geometry must be finite and within +/-1000000".into());
        }
    }
    if let Some(y) = u.y {
        if !y.is_finite() || y.abs() > 1_000_000.0 {
            return Err("geometry must be finite and within +/-1000000".into());
        }
    }
    if let Some(w) = u.width {
        if !w.is_finite() || w.abs() > 1_000_000.0 {
            return Err("geometry must be finite and within +/-1000000".into());
        }
        if w < 0.0 {
            return Err("width and height must be non-negative".into());
        }
    }
    if let Some(h) = u.height {
        if !h.is_finite() || h.abs() > 1_000_000.0 {
            return Err("geometry must be finite and within +/-1000000".into());
        }
        if h < 0.0 {
            return Err("width and height must be non-negative".into());
        }
    }
    if let Some(opacity) = u.opacity {
        if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
            return Err("opacity must be between 0 and 1".into());
        }
    }
    if let Some(text) = &u.text {
        bounded_text("text", text, 100_000)?;
    }
    validate_fill_color(u.fill.as_deref())?;
    Ok(())
}

pub(crate) fn build_updated_node(doc: &Document, id: NodeId, u: UpdateFields) -> Result<(Node, bool), String> {
    if id.0 == 0 {
        return Err("node IDs must be positive integers".into());
    }
    let mut node = doc.get(id).ok_or_else(|| format!("node {} not found", id.0))?.clone();
    if node.parent.is_none() {
        return Err("page roots cannot be updated by this tool".into());
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
            return Err("text can only be set on text nodes".into());
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
