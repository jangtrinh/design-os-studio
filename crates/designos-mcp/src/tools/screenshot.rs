//! Inline node/region screenshot rendering for `designos_get_screenshot` (T7.1 / #238).
//!
//! Renders a single node (`node_id`), a union of nodes (`node_ids`, 1..=50 on the same page),
//! or a page-space rectangle (`region: {page_id, x, y, width, height}`) on the deterministic
//! CPU path with per-subtree variable resolution, downsizing to `max_long_edge` when needed.

use designos_doc::{resolve_node, Affine, Color, Document, Node, NodeId, NodeKind, PaintKind, Rect, Vec2};
use designos_export::{
    base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _},
    designos_render::{checked_pixmap_size, render_page, Viewport, MAX_PIXMAP_PIXELS, MAX_PIXMAP_SIDE},
    encode_jpg_pixmap, encode_png_pixmap, render_bounds,
    tiny_skia::Pixmap,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

/// Structured MCP tool error with stable code, valid options, and retry safety flag.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolError {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub path: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub valid_options: Vec<Value>,
    pub safe_to_retry: bool,
    pub next_step: String,
}

impl ToolError {
    pub fn new(code: &'static str, message: impl Into<String>, path: impl Into<String>, next_step: impl Into<String>) -> Self {
        Self { code, message: message.into(), path: path.into(), valid_options: Vec::new(), safe_to_retry: true, next_step: next_step.into() }
    }

    pub fn with_valid_options(mut self, valid_options: Vec<Value>) -> Self {
        self.valid_options = valid_options;
        self
    }

    pub fn to_json_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or_else(|_| {
            json!({
                "code": self.code,
                "message": self.message,
                "safe_to_retry": self.safe_to_retry,
                "next_step": self.next_step,
            })
        })
    }

    pub fn to_err_string(&self) -> String {
        serde_json::to_string(&self.to_json_value()).unwrap_or_else(|_| format!("{}: {}", self.code, self.message))
    }

    pub fn image_too_large(width: f32, height: f32, scale_used: f32) -> Self {
        let max_side = width.max(height).max(1.0) as f64;
        let area = (width.max(1.0) as f64) * (height.max(1.0) as f64);
        let side_scale = (MAX_PIXMAP_SIDE as f64) / max_side;
        let pixel_scale = ((MAX_PIXMAP_PIXELS as f64) / area).sqrt();
        let raw_max_scale = side_scale.min(pixel_scale).min(4.0);
        let largest_allowed_scale = ((raw_max_scale * 1000.0).floor() / 1000.0).max(0.01);
        Self::new(
            "IMAGE_TOO_LARGE",
            format!(
                "IMAGE_TOO_LARGE: requested render size ({:.0}x{:.0} at scale {:.3}) exceeds pixmap limits (max side {}, max pixels {}).",
                width, height, scale_used, MAX_PIXMAP_SIDE, MAX_PIXMAP_PIXELS
            ),
            "/scale",
            "Lower scale or set max_long_edge (e.g. 1024) and retry.",
        )
        .with_valid_options(vec![json!({"largest_allowed_scale": largest_allowed_scale}), json!({"max_long_edge": 1024})])
    }
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ToolError {}

/// Page-space rectangular crop target.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegionTarget {
    pub page_id: u64,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Target for `render_target` and `designos_get_screenshot`.
#[derive(Clone, Debug, PartialEq)]
pub enum ScreenshotTarget {
    Node(NodeId),
    Nodes(Vec<NodeId>),
    Region(RegionTarget),
}

impl ScreenshotTarget {
    pub fn to_json(&self) -> Value {
        match self {
            Self::Node(id) => json!({ "node_id": id.0 }),
            Self::Nodes(ids) => json!({ "node_ids": ids.iter().map(|id| id.0).collect::<Vec<_>>() }),
            Self::Region(r) => json!({
                "region": {
                    "page_id": r.page_id,
                    "x": r.x,
                    "y": r.y,
                    "width": r.width,
                    "height": r.height,
                }
            }),
        }
    }
}

/// Output image format (`png` or `jpeg`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ScreenshotFormat {
    #[default]
    Png,
    Jpeg,
}

impl ScreenshotFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
        }
    }

    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
        }
    }
}

/// Raw arguments for `designos_get_screenshot`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenshotArgs {
    #[serde(default)]
    pub node_id: Option<u64>,
    #[serde(default)]
    pub node_ids: Option<Vec<u64>>,
    #[serde(default)]
    pub region: Option<RegionTarget>,
    #[serde(default = "default_screenshot_scale")]
    pub scale: f32,
    #[serde(default = "default_max_long_edge")]
    pub max_long_edge: u32,
    #[serde(default)]
    pub format: ScreenshotFormat,
    #[serde(default = "default_jpeg_quality")]
    pub quality: u8,
}

pub fn default_screenshot_scale() -> f32 {
    0.5
}

pub fn default_max_long_edge() -> u32 {
    1024
}

pub fn default_jpeg_quality() -> u8 {
    80
}

impl ScreenshotArgs {
    /// Validates and extracts the single [`ScreenshotTarget`].
    pub fn resolve_target(&self) -> Result<ScreenshotTarget, ToolError> {
        let count = usize::from(self.node_id.is_some()) + usize::from(self.node_ids.is_some()) + usize::from(self.region.is_some());
        if count != 1 {
            return Err(ToolError::new(
                "EXACTLY_ONE_TARGET",
                "EXACTLY_ONE_TARGET: specify exactly one of node_id, node_ids, or region.",
                "",
                "Pass exactly one of node_id, node_ids, or region.",
            ));
        }
        if let Some(id) = self.node_id {
            if id == 0 {
                return Err(ToolError::new("NODE_NOT_FOUND", "NODE_NOT_FOUND: node_id 0 does not exist.", "/node_id", "Pass a valid node_id >= 1."));
            }
            return Ok(ScreenshotTarget::Node(NodeId(id)));
        }
        if let Some(ids) = &self.node_ids {
            if ids.is_empty() || ids.len() > 50 {
                return Err(ToolError::new(
                    "VALUE_OUT_OF_RANGE",
                    "VALUE_OUT_OF_RANGE: node_ids must contain between 1 and 50 node IDs.",
                    "/node_ids",
                    "Pass 1..=50 node IDs in node_ids.",
                ));
            }
            if ids.contains(&0) {
                return Err(ToolError::new("NODE_NOT_FOUND", "NODE_NOT_FOUND: node_id 0 does not exist.", "/node_ids", "Pass valid node IDs >= 1."));
            }
            return Ok(ScreenshotTarget::Nodes(ids.iter().copied().map(NodeId).collect()));
        }
        if let Some(r) = self.region {
            return Ok(ScreenshotTarget::Region(r));
        }
        unreachable!()
    }
}

/// Rendered image and metadata returned by [`render_target`].
#[derive(Clone, Debug)]
pub struct RenderedImage {
    pub target: ScreenshotTarget,
    pub pixmap: Pixmap,
    pub page_id: NodeId,
    pub bounds: Rect,
    pub scale_requested: f32,
    pub scale_used: f32,
    pub downscaled: bool,
    pub width: u32,
    pub height: u32,
    pub visible: bool,
    pub scoped: bool,
}

impl RenderedImage {
    /// Encodes the rendered pixmap into PNG or JPEG bytes.
    pub fn encode(&self, format: ScreenshotFormat, quality: u8) -> Vec<u8> {
        match format {
            ScreenshotFormat::Png => encode_png_pixmap(&self.pixmap),
            ScreenshotFormat::Jpeg => encode_jpg_pixmap(&self.pixmap, quality.clamp(1, 100)),
        }
    }
}

/// Finds the enclosing page for `id` (or `Some(id)` if `id` is itself a `Page`).
pub fn enclosing_page(doc: &Document, id: NodeId) -> Option<NodeId> {
    let mut cur = Some(id);
    let mut depth = 0usize;
    while let Some(cid) = cur {
        if depth > 256 {
            return None;
        }
        let n = doc.get(cid)?;
        if matches!(n.kind, NodeKind::Page { .. }) {
            return Some(cid);
        }
        cur = n.parent;
        depth += 1;
    }
    None
}

/// Whether `id` and all its ancestors up to the page root are visible.
fn is_visible_in_page(doc: &Document, id: NodeId) -> bool {
    let mut cur = Some(id);
    let mut depth = 0usize;
    while let Some(cid) = cur {
        if depth > 256 {
            return false;
        }
        let Some(n) = doc.get(cid) else { return false };
        let resolved = resolve_node(doc, n);
        if matches!(resolved.kind, NodeKind::Page { .. }) {
            return true;
        }
        if !resolved.visible {
            return false;
        }
        cur = resolved.parent;
        depth += 1;
    }
    false
}

/// World (page-space) affine transform of `id`.
fn world_transform(doc: &Document, id: NodeId) -> Affine {
    let mut world = Affine::IDENTITY;
    let mut cur = Some(id);
    let mut depth = 0usize;
    while let Some(cid) = cur {
        if depth > 256 {
            break;
        }
        let Some(n) = doc.get(cid) else { break };
        if matches!(n.kind, NodeKind::Page { .. }) {
            break;
        }
        let resolved = resolve_node(doc, n);
        world = resolved.transform * world;
        cur = resolved.parent;
        depth += 1;
    }
    world
}

fn transform_rect(t: &Affine, r: Rect) -> Rect {
    let pts = [t.apply(Vec2::new(r.x, r.y)), t.apply(Vec2::new(r.x + r.w, r.y)), t.apply(Vec2::new(r.x + r.w, r.y + r.h)), t.apply(Vec2::new(r.x, r.y + r.h))];
    let x0 = pts.iter().map(|p| p.x).fold(f32::MAX, f32::min);
    let y0 = pts.iter().map(|p| p.y).fold(f32::MAX, f32::min);
    let x1 = pts.iter().map(|p| p.x).fold(f32::MIN, f32::max);
    let y1 = pts.iter().map(|p| p.y).fold(f32::MIN, f32::max);
    Rect::new(x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0))
}

/// Page-space render bounds of `id` (including effect/stroke overflow).
pub fn page_render_bounds(doc: &Document, id: NodeId) -> Rect {
    let Some(n) = doc.get(id) else { return Rect::default() };
    if matches!(n.kind, NodeKind::Page { .. }) {
        return render_bounds(doc, id);
    }
    let local = render_bounds(doc, id);
    let world = world_transform(doc, id);
    transform_rect(&world, local)
}

/// Detects whether `page` is a scoped Studio page or holds top-level frame shells (from `live_canvas_scope.rs`).
fn page_has_scoped_shells(doc: &Document, page_id: NodeId) -> bool {
    let Some(page) = doc.get(page_id) else { return false };
    let is_scoped_flag = |val: &str| matches!(val, "true" | "1");
    if page.fig_metadata.get("designos:scoped").is_some_and(|v| is_scoped_flag(v))
        || page.fig_metadata.get("scoped").is_some_and(|v| is_scoped_flag(v))
        || page.plugin_data.get("scoped").is_some_and(|v| is_scoped_flag(v))
    {
        return true;
    }
    page.children.iter().filter_map(|&cid| doc.get(cid)).any(|c| {
        c.fig_metadata.get("designos:scoped_shell").is_some_and(|v| is_scoped_flag(v))
            || c.fig_metadata.get("scoped_shell").is_some_and(|v| is_scoped_flag(v))
            || c.fig_metadata.get("designos:scoped").is_some_and(|v| is_scoped_flag(v))
            || c.plugin_data.get("scoped").is_some_and(|v| is_scoped_flag(v))
    })
}

fn collect_node_images(n: &Node, out: &mut BTreeSet<String>) {
    let mut visit = |paints: &[designos_doc::Paint]| {
        for p in paints {
            if let PaintKind::Image { image_ref, .. } = &p.kind {
                out.insert(image_ref.clone());
            }
        }
    };
    visit(&n.fills);
    visit(&n.stroke.paints);
    if let NodeKind::Text(t) = &n.kind {
        visit(&t.style.fills);
        for r in &t.runs {
            visit(&r.style.fills);
        }
    }
}

/// Rasterizes `root`'s page-space `bounds` at `scale_used` into a `(w, h)` pixmap, keeping `root`'s
/// world matrix (`world_transform`) and resolving bound variables per node in `root`'s subtree.
fn render_node_in_page_space(doc: &Document, root: NodeId, bounds: Rect, scale_used: f32, w: u32, h: u32) -> Option<Pixmap> {
    let mut pm = Pixmap::new(w, h)?;
    let root_node = doc.get(root)?;
    let vp = Viewport { offset: Vec2::new(bounds.x, bounds.y), zoom: scale_used, dpr: 1.0 };
    if matches!(root_node.kind, NodeKind::Page { .. }) {
        render_page(doc, root, &vp, &mut pm);
        return Some(pm);
    }
    let mut scratch = Document::new(doc.name.clone());
    if let Some(max) = doc.nodes.keys().max() {
        scratch.reserve_ids(*max);
    }
    let page = scratch.add_page("export");
    if let Some(p) = scratch.get_mut(page) {
        p.kind = NodeKind::Page { background: Color::TRANSPARENT };
    }
    let mut refs = BTreeSet::new();
    let mut nodes: Vec<Node> = Vec::new();
    doc.walk(root, |n, _| {
        let mut resolved = resolve_node(doc, n).into_owned();
        resolved.variables.bindings.clear();
        collect_node_images(&resolved, &mut refs);
        nodes.push(resolved);
    });
    let root_world = world_transform(doc, root);
    for (i, mut n) in nodes.into_iter().enumerate() {
        let parent = if i == 0 {
            n.transform = root_world;
            page
        } else {
            n.parent.unwrap_or(page)
        };
        n.children.clear();
        let _ = scratch.insert(parent, n);
    }
    for r in refs {
        if let Some(data) = doc.images.get(&r) {
            scratch.images.insert(r, data.clone());
        }
    }
    render_page(&scratch, page, &vp, &mut pm);
    Some(pm)
}

/// Computes `(scale_used, downscaled)` for `bounds` given `scale` and `max_long_edge`.
pub fn compute_scale_used(bounds: Rect, scale: f32, max_long_edge: u32) -> (f32, bool) {
    let long_edge = bounds.w.max(bounds.h);
    if max_long_edge > 0 && long_edge > 0.0 && long_edge * scale > max_long_edge as f32 {
        let mut scale_used = (max_long_edge as f32) / long_edge;
        // Guard against IEEE-754 f32 rounding making `(long_edge * scale_used).ceil() > max_long_edge`.
        while (long_edge * scale_used).ceil() > max_long_edge as f32 && scale_used > 0.0 {
            scale_used = f32::from_bits(scale_used.to_bits() - 1);
        }
        (scale_used, true)
    } else {
        (scale, false)
    }
}

/// Renders `target` on `doc` at `scale` (0.25..=4.0), capped by `max_long_edge` (0 or 64..=4096).
pub fn render_target(doc: &Document, target: ScreenshotTarget, scale: f32, max_long_edge: u32) -> Result<RenderedImage, ToolError> {
    if !scale.is_finite() || !(0.25..=4.0).contains(&scale) {
        return Err(ToolError::new("VALUE_OUT_OF_RANGE", "VALUE_OUT_OF_RANGE: scale must be between 0.25 and 4.0.", "/scale", "Pass a scale in 0.25..=4.0."));
    }
    if max_long_edge != 0 && !(64..=4096).contains(&max_long_edge) {
        return Err(ToolError::new(
            "VALUE_OUT_OF_RANGE",
            "VALUE_OUT_OF_RANGE: max_long_edge must be 0 or between 64 and 4096.",
            "/max_long_edge",
            "Pass max_long_edge = 0 or in 64..=4096.",
        ));
    }

    match &target {
        ScreenshotTarget::Node(id) => {
            if doc.get(*id).is_none() {
                return Err(ToolError::new(
                    "NODE_NOT_FOUND",
                    format!("NODE_NOT_FOUND: node {} does not exist.", id.0),
                    "/node_id",
                    "Call designos_list_nodes or designos_get_document to find valid node IDs.",
                ));
            }
            let page_id = enclosing_page(doc, *id).ok_or_else(|| {
                ToolError::new(
                    "NODE_NOT_FOUND",
                    format!("NODE_NOT_FOUND: node {} does not belong to any page.", id.0),
                    "/node_id",
                    "Call designos_list_nodes to find a node on a page.",
                )
            })?;
            let visible = is_visible_in_page(doc, *id);
            let bounds = page_render_bounds(doc, *id);
            let (scale_used, downscaled) = compute_scale_used(bounds, scale, max_long_edge);
            if scale_used < 0.01 {
                return Err(ToolError::image_too_large(bounds.w, bounds.h, scale_used));
            }
            let (w, h) =
                checked_pixmap_size(bounds.w * scale_used, bounds.h * scale_used).ok_or_else(|| ToolError::image_too_large(bounds.w, bounds.h, scale_used))?;
            let pixmap = if visible {
                render_node_in_page_space(doc, *id, bounds, scale_used, w, h).ok_or_else(|| ToolError::image_too_large(bounds.w, bounds.h, scale_used))?
            } else {
                Pixmap::new(w, h).ok_or_else(|| ToolError::image_too_large(bounds.w, bounds.h, scale_used))?
            };
            let width = pixmap.width();
            let height = pixmap.height();
            Ok(RenderedImage { target, pixmap, page_id, bounds, scale_requested: scale, scale_used, downscaled, width, height, visible, scoped: false })
        }
        ScreenshotTarget::Nodes(ids) => {
            if ids.is_empty() || ids.len() > 50 {
                return Err(ToolError::new(
                    "VALUE_OUT_OF_RANGE",
                    "VALUE_OUT_OF_RANGE: node_ids must contain 1..=50 IDs.",
                    "/node_ids",
                    "Pass between 1 and 50 node IDs.",
                ));
            }
            let mut common_page: Option<NodeId> = None;
            let mut union_bounds: Option<Rect> = None;
            let mut any_visible = false;
            for (idx, &id) in ids.iter().enumerate() {
                if doc.get(id).is_none() {
                    return Err(ToolError::new(
                        "NODE_NOT_FOUND",
                        format!("NODE_NOT_FOUND: node {} does not exist.", id.0),
                        format!("/node_ids/{idx}"),
                        "Call designos_list_nodes to find valid node IDs.",
                    ));
                }
                let page_id = enclosing_page(doc, id).ok_or_else(|| {
                    ToolError::new(
                        "NODE_NOT_FOUND",
                        format!("NODE_NOT_FOUND: node {} is not on a page.", id.0),
                        format!("/node_ids/{idx}"),
                        "Call designos_list_nodes to find valid node IDs.",
                    )
                })?;
                if let Some(prev) = common_page {
                    if prev != page_id {
                        return Err(ToolError::new(
                            "NODES_ON_DIFFERENT_PAGES",
                            format!("NODES_ON_DIFFERENT_PAGES: node {} is on page {}, expected page {}.", id.0, page_id.0, prev.0),
                            format!("/node_ids/{idx}"),
                            "Request screenshots for nodes on one page at a time.",
                        ));
                    }
                } else {
                    common_page = Some(page_id);
                }
                any_visible |= is_visible_in_page(doc, id);
                let b = page_render_bounds(doc, id);
                union_bounds = Some(match union_bounds {
                    Some(acc) => acc.union(&b),
                    None => b,
                });
            }
            let page_id = common_page.unwrap();
            let bounds = union_bounds.unwrap_or_default();
            let (scale_used, downscaled) = compute_scale_used(bounds, scale, max_long_edge);
            if scale_used < 0.01 {
                return Err(ToolError::image_too_large(bounds.w, bounds.h, scale_used));
            }
            let (w, h) =
                checked_pixmap_size(bounds.w * scale_used, bounds.h * scale_used).ok_or_else(|| ToolError::image_too_large(bounds.w, bounds.h, scale_used))?;
            let mut pixmap = Pixmap::new(w, h).ok_or_else(|| ToolError::image_too_large(bounds.w, bounds.h, scale_used))?;
            if any_visible {
                let vp = Viewport { offset: Vec2::new(bounds.x, bounds.y), zoom: scale_used, dpr: 1.0 };
                render_page(doc, page_id, &vp, &mut pixmap);
            }
            Ok(RenderedImage {
                target,
                pixmap,
                page_id,
                bounds,
                scale_requested: scale,
                scale_used,
                downscaled,
                width: w,
                height: h,
                visible: any_visible,
                scoped: false,
            })
        }
        ScreenshotTarget::Region(r) => {
            if !r.x.is_finite() || !r.y.is_finite() || !r.width.is_finite() || !r.height.is_finite() || r.width <= 0.0 || r.height <= 0.0 {
                return Err(ToolError::new(
                    "INVALID_REGION",
                    "INVALID_REGION: region x, y, width, height must be finite and width, height > 0.",
                    "/region",
                    "Pass positive finite width and height in region.",
                ));
            }
            let page_id = NodeId(r.page_id);
            let Some(page_node) = doc.get(page_id) else {
                return Err(ToolError::new(
                    "NODE_NOT_FOUND",
                    format!("NODE_NOT_FOUND: page {} does not exist.", r.page_id),
                    "/region/page_id",
                    "Call designos_get_document to list valid page IDs.",
                ));
            };
            if !matches!(page_node.kind, NodeKind::Page { .. }) {
                return Err(ToolError::new(
                    "INVALID_REGION",
                    format!("INVALID_REGION: node {} is not a page.", r.page_id),
                    "/region/page_id",
                    "Pass a valid page_id from designos_get_document.",
                ));
            }
            let bounds = Rect::new(r.x, r.y, r.width, r.height);
            let (scale_used, downscaled) = compute_scale_used(bounds, scale, max_long_edge);
            if scale_used < 0.01 {
                return Err(ToolError::image_too_large(bounds.w, bounds.h, scale_used));
            }
            let (w, h) =
                checked_pixmap_size(bounds.w * scale_used, bounds.h * scale_used).ok_or_else(|| ToolError::image_too_large(bounds.w, bounds.h, scale_used))?;
            let mut pixmap = Pixmap::new(w, h).ok_or_else(|| ToolError::image_too_large(bounds.w, bounds.h, scale_used))?;
            let vp = Viewport { offset: Vec2::new(bounds.x, bounds.y), zoom: scale_used, dpr: 1.0 };
            render_page(doc, page_id, &vp, &mut pixmap);
            let scoped = page_has_scoped_shells(doc, page_id);
            Ok(RenderedImage { target, pixmap, page_id, bounds, scale_requested: scale, scale_used, downscaled, width: w, height: h, visible: true, scoped })
        }
    }
}

fn round6(v: f32) -> f64 {
    ((v as f64) * 1_000_000.0).round() / 1_000_000.0
}

/// Executes `designos_get_screenshot` and returns the structured JSON response (including `content[0]` image block).
pub fn execute_get_screenshot(doc: &Document, raw_args: Value) -> Result<Value, String> {
    let args: ScreenshotArgs = serde_json::from_value(raw_args).map_err(|e| {
        ToolError::new("INVALID_ARGUMENTS", format!("INVALID_ARGUMENTS: {e}"), "", "Check the arguments against the designos_get_screenshot schema.")
            .to_err_string()
    })?;
    if !(1..=100).contains(&args.quality) {
        return Err(ToolError::new("VALUE_OUT_OF_RANGE", "VALUE_OUT_OF_RANGE: quality must be between 1 and 100.", "/quality", "Pass quality in 1..=100.")
            .to_err_string());
    }
    let target = args.resolve_target().map_err(|e| e.to_err_string())?;
    let rendered = render_target(doc, target, args.scale, args.max_long_edge).map_err(|e| e.to_err_string())?;
    let encoded_bytes = rendered.encode(args.format, args.quality);
    let b64 = BASE64_STANDARD.encode(&encoded_bytes);
    let mime = args.format.mime_type();
    let mut out = json!({
        "target": rendered.target.to_json(),
        "page_id": rendered.page_id.0,
        "bounds": {
            "x": round6(rendered.bounds.x),
            "y": round6(rendered.bounds.y),
            "width": round6(rendered.bounds.w),
            "height": round6(rendered.bounds.h),
        },
        "scale_requested": round6(rendered.scale_requested),
        "scale_used": round6(rendered.scale_used),
        "downscaled": rendered.downscaled,
        "width": rendered.width,
        "height": rendered.height,
        "format": args.format.as_str(),
        "bytes": encoded_bytes.len(),
        "visible": rendered.visible,
        "content": [
            {
                "type": "image",
                "mimeType": mime,
                "data": b64,
            }
        ]
    });
    if rendered.scoped {
        if let Some(obj) = out.as_object_mut() {
            obj.insert("scoped".into(), json!(true));
        }
    }
    Ok(out)
}

/// JSON Schema descriptor for `designos_get_screenshot`.
pub fn tool_descriptor() -> Value {
    json!({
        "name": "designos_get_screenshot",
        "description": "Render a node, a set of nodes on the same page, or a page-space rectangular region and return it inline as MCP image content (PNG or JPEG), automatically downsized to max_long_edge (default 1024).",
        "inputSchema": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "node_id": {"type": "integer", "minimum": 1},
                "node_ids": {
                    "type": "array",
                    "items": {"type": "integer", "minimum": 1},
                    "minItems": 1,
                    "maxItems": 50
                },
                "region": {
                    "type": "object",
                    "required": ["page_id", "x", "y", "width", "height"],
                    "additionalProperties": false,
                    "properties": {
                        "page_id": {"type": "integer", "minimum": 1},
                        "x": {"type": "number"},
                        "y": {"type": "number"},
                        "width": {"type": "number", "exclusiveMinimum": 0},
                        "height": {"type": "number", "exclusiveMinimum": 0}
                    }
                },
                "scale": {"type": "number", "minimum": 0.25, "maximum": 4, "default": 0.5},
                "max_long_edge": {"type": "integer", "minimum": 0, "maximum": 4096, "default": 1024},
                "format": {"enum": ["png", "jpeg"], "default": "png"},
                "quality": {"type": "integer", "minimum": 1, "maximum": 100, "default": 80}
            }
        },
        "annotations": {
            "readOnlyHint": true,
            "destructiveHint": false,
            "idempotentHint": true,
            "openWorldHint": false
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Color, Node, Paint};

    fn decode_png_from_reply(reply: &Value) -> Pixmap {
        assert_eq!(reply["content"][0]["type"], "image");
        assert_eq!(reply["content"][0]["mimeType"], "image/png");
        let b64 = reply["content"][0]["data"].as_str().expect("base64 image data");
        let bytes = BASE64_STANDARD.decode(b64).expect("valid base64");
        Pixmap::decode_png(&bytes).expect("valid PNG")
    }

    #[test]
    fn png_image_content_at_scale() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let fid = doc.alloc_id();
        doc.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 200.0, 100.0))).unwrap();

        let reply = execute_get_screenshot(&doc, json!({"node_id": fid.0, "scale": 2.0, "max_long_edge": 0})).unwrap();
        let pm = decode_png_from_reply(&reply);
        assert!((pm.width() as i32 - 400).abs() <= 1, "width={}", pm.width());
        assert!((pm.height() as i32 - 200).abs() <= 1, "height={}", pm.height());
        assert_eq!(reply["downscaled"], false);
        assert_eq!(reply["scale_used"], 2.0);
        assert_eq!(reply["visible"], true);
    }

    #[test]
    fn default_downsizes_to_1024() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let fid = doc.alloc_id();
        doc.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 4000.0, 2400.0))).unwrap();

        let reply = execute_get_screenshot(&doc, json!({"node_id": fid.0})).unwrap();
        let pm = decode_png_from_reply(&reply);
        assert!((pm.width() as i32 - 1024).abs() <= 1, "width={}", pm.width());
        assert!((pm.height() as i32 - 615).abs() <= 1, "height={}", pm.height());
        assert_eq!(reply["width"], 1024);
        assert_eq!(reply["height"], 615);
        assert_eq!(reply["scale_requested"], 0.5);
        assert_eq!(reply["scale_used"], 0.256);
        assert_eq!(reply["downscaled"], true);
    }

    #[test]
    fn region_crop_matches_rect() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let fid = doc.alloc_id();
        let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 4000.0, 4000.0));
        frame.fills = vec![Paint::solid(Color::WHITE)];
        doc.insert(page, frame).unwrap();

        let sq_id = doc.alloc_id();
        let mut sq = Node::rectangle(sq_id, Rect::new(1000.0, 1000.0, 100.0, 100.0));
        sq.fills = vec![Paint::solid(Color::rgb(255, 0, 0))];
        doc.insert(fid, sq).unwrap();

        let reply = execute_get_screenshot(
            &doc,
            json!({
                "region": {
                    "page_id": page.0,
                    "x": 900.0,
                    "y": 900.0,
                    "width": 400.0,
                    "height": 400.0
                }
            }),
        )
        .unwrap();
        let pm = decode_png_from_reply(&reply);
        assert_eq!((pm.width(), pm.height()), (200, 200));

        let is_red = |x: u32, y: u32| {
            let p = pm.pixel(x, y).unwrap();
            p.red() > 240 && p.green() < 15 && p.blue() < 15
        };
        // Inside the red block [50..100, 50..100)
        assert!(is_red(51, 51));
        assert!(is_red(98, 98));
        assert!(is_red(75, 75));
        // Just outside the red block (±1 px tolerance)
        assert!(!is_red(48, 48));
        assert!(!is_red(101, 101));
        assert!(!is_red(20, 20));
        assert!(!is_red(150, 150));
    }

    #[test]
    fn amendment_region_long_edge() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let fid = doc.alloc_id();
        let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 4000.0, 4000.0));
        frame.fills = vec![Paint::solid(Color::rgb(240, 242, 245))];
        doc.insert(page, frame).unwrap();

        for i in 0..8 {
            let rid = doc.alloc_id();
            let mut r = Node::rectangle(rid, Rect::new(i as f32 * 500.0 + 40.0, 100.0, 320.0, 800.0));
            r.fills = vec![Paint::solid(Color::rgb((i * 30) as u8, 120, 200))];
            doc.insert(fid, r).unwrap();
        }

        let reply = execute_get_screenshot(
            &doc,
            json!({
                "region": {
                    "page_id": page.0,
                    "x": 0.0,
                    "y": 0.0,
                    "width": 4000.0,
                    "height": 1000.0
                },
                "max_long_edge": 1024
            }),
        )
        .unwrap();
        let pm = decode_png_from_reply(&reply);
        assert_eq!(pm.width().max(pm.height()), 1024);
        assert_eq!((pm.width(), pm.height()), (1024, 256));

        let scale_used = reply["scale_used"].as_f64().unwrap() as f32;
        let mut ref_pm = Pixmap::new(pm.width(), pm.height()).unwrap();
        let vp = Viewport { offset: Vec2::new(0.0, 0.0), zoom: scale_used, dpr: 1.0 };
        render_page(&doc, page, &vp, &mut ref_pm);

        let total_px = (pm.width() * pm.height()) as usize;
        let mismatched = pm.data().chunks_exact(4).zip(ref_pm.data().chunks_exact(4)).filter(|(a, b)| a != b).count();
        let diff_ratio = mismatched as f64 / total_px as f64;
        assert!(diff_ratio <= 0.005, "diff_ratio={diff_ratio}");
    }

    #[test]
    fn over_cap_is_named() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let fid = doc.alloc_id();
        doc.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 16000.0, 16000.0))).unwrap();

        let err = execute_get_screenshot(&doc, json!({"node_id": fid.0, "scale": 4.0, "max_long_edge": 0})).unwrap_err();
        assert!(err.contains("IMAGE_TOO_LARGE"), "{err}");
        let parsed: Value = serde_json::from_str(&err).unwrap();
        assert_eq!(parsed["code"], "IMAGE_TOO_LARGE");
        assert_eq!(parsed["safe_to_retry"], true);
        assert!(!parsed["valid_options"].as_array().unwrap().is_empty());
    }

    #[test]
    fn node_set_union() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");
        let n1 = doc.alloc_id();
        doc.insert(page, Node::rectangle(n1, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
        let n2 = doc.alloc_id();
        doc.insert(page, Node::rectangle(n2, Rect::new(300.0, 0.0, 100.0, 100.0))).unwrap();

        let reply = execute_get_screenshot(&doc, json!({"node_ids": [n1.0, n2.0]})).unwrap();
        assert_eq!(reply["bounds"]["width"], 400.0);
        assert_eq!(reply["bounds"]["height"], 100.0);
    }

    #[test]
    fn rotated_node_and_scaled_parent_keep_world_transform_and_page_bounds_size() {
        let mut doc = Document::new("Test");
        let page = doc.add_page("Page 1");

        // 4000x100 rectangle rotated 90 degrees -> page bounds ~100x4000, at scale 0.5 -> ~50x2000 px
        let rot_id = doc.alloc_id();
        let mut rot_rect = Node::rectangle(rot_id, Rect::new(0.0, 0.0, 4000.0, 100.0));
        rot_rect.transform = Affine::rotate(std::f32::consts::FRAC_PI_2);
        rot_rect.fills = vec![Paint::solid(Color::rgb(255, 0, 0))];
        doc.insert(page, rot_rect).unwrap();

        let reply = execute_get_screenshot(&doc, json!({"node_id": rot_id.0, "scale": 0.5, "max_long_edge": 0})).unwrap();
        let pm = decode_png_from_reply(&reply);
        assert!((reply["bounds"]["width"].as_f64().unwrap() - 100.0).abs() <= 1.0, "bounds={}", reply["bounds"]);
        assert!((reply["bounds"]["height"].as_f64().unwrap() - 4000.0).abs() <= 1.0, "bounds={}", reply["bounds"]);
        assert!((pm.width() as i32 - 50).abs() <= 1, "expected width ~50, got {}", pm.width());
        assert!((pm.height() as i32 - 2000).abs() <= 1, "expected height ~2000, got {}", pm.height());
        assert_eq!(reply["width"], pm.width());
        assert_eq!(reply["height"], pm.height());
        let center_px = pm.pixel(pm.width() / 2, pm.height() / 2).unwrap();
        assert!(center_px.red() > 240 && center_px.green() < 15 && center_px.blue() < 15, "center pixel should be red: {center_px:?}");

        // Button (100x40) inside a 2x scaled parent frame -> page bounds 200x80, at scale 0.5 -> 100x40 px
        let parent_id = doc.alloc_id();
        let mut parent = Node::frame(parent_id, Rect::new(0.0, 0.0, 200.0, 200.0));
        parent.transform = Affine::scale(2.0, 2.0);
        doc.insert(page, parent).unwrap();

        let btn_id = doc.alloc_id();
        let mut btn = Node::rectangle(btn_id, Rect::new(10.0, 20.0, 100.0, 40.0));
        btn.fills = vec![Paint::solid(Color::rgb(0, 200, 0))];
        doc.insert(parent_id, btn).unwrap();

        let btn_reply = execute_get_screenshot(&doc, json!({"node_id": btn_id.0, "scale": 0.5, "max_long_edge": 0})).unwrap();
        let btn_pm = decode_png_from_reply(&btn_reply);
        assert!((btn_reply["bounds"]["width"].as_f64().unwrap() - 200.0).abs() <= 1.0);
        assert!((btn_reply["bounds"]["height"].as_f64().unwrap() - 80.0).abs() <= 1.0);
        assert_eq!((btn_pm.width(), btn_pm.height()), (100, 40));
        assert_eq!(btn_reply["width"], 100);
        assert_eq!(btn_reply["height"], 40);
    }

    #[test]
    fn scoped_flag_tracks_scoped_shells_not_mixed_empty_frames() {
        // 1. Normal page with one empty frame and one populated frame must NOT set scoped: true.
        let mut normal_doc = Document::new("Normal");
        let normal_page = normal_doc.add_page("Page 1");
        let empty_f = normal_doc.alloc_id();
        normal_doc.insert(normal_page, Node::frame(empty_f, Rect::new(0.0, 0.0, 200.0, 200.0))).unwrap();
        let full_f = normal_doc.alloc_id();
        normal_doc.insert(normal_page, Node::frame(full_f, Rect::new(300.0, 0.0, 200.0, 200.0))).unwrap();
        let child = normal_doc.alloc_id();
        normal_doc.insert(full_f, Node::rectangle(child, Rect::new(10.0, 10.0, 50.0, 50.0))).unwrap();

        let normal_reply =
            execute_get_screenshot(&normal_doc, json!({"region": {"page_id": normal_page.0, "x": 0.0, "y": 0.0, "width": 500.0, "height": 200.0}})).unwrap();
        assert!(normal_reply.get("scoped").is_none(), "unscoped page must not report scoped: true: {normal_reply}");

        // 2. Scoped page where every top-level frame is an empty shell (no expanded frame) MUST set scoped: true.
        let mut scoped_doc = Document::new("Scoped");
        let scoped_page = scoped_doc.add_page("Page 1");
        let shell1 = scoped_doc.alloc_id();
        let mut f1 = Node::frame(shell1, Rect::new(0.0, 0.0, 200.0, 200.0));
        f1.fig_metadata.insert("designos:scoped_shell".into(), "true".into());
        scoped_doc.insert(scoped_page, f1).unwrap();
        let shell2 = scoped_doc.alloc_id();
        let mut f2 = Node::frame(shell2, Rect::new(300.0, 0.0, 200.0, 200.0));
        f2.fig_metadata.insert("designos:scoped_shell".into(), "true".into());
        scoped_doc.insert(scoped_page, f2).unwrap();
        scoped_doc.get_mut(scoped_page).unwrap().fig_metadata.insert("designos:scoped".into(), "true".into());

        let scoped_reply =
            execute_get_screenshot(&scoped_doc, json!({"region": {"page_id": scoped_page.0, "x": 0.0, "y": 0.0, "width": 500.0, "height": 200.0}})).unwrap();
        assert_eq!(scoped_reply["scoped"], json!(true));
    }
}
