//! Importer that converts a [`Capture`] (`.capture.json` / `.webcapture.json`)
//! into editable [`designos_doc`] frames, auto-layout containers, text nodes, vectors, and images.

use crate::layout_infer::{
    infer_auto_layout, infer_layout_child, is_out_of_flow, map_overflow_clips, parse_border_widths, parse_padding, sort_flow_children_visually,
};
use crate::paint_map::{
    decode_data_url, has_own_box_paint, map_background_paints, map_corner_radii, map_effects_and_blend, map_rotation_rad, map_stroke, node_transform_at,
    parse_css_color, parse_css_px, store_image_dedup,
};
use crate::schema::{Capture, CaptureError, Element, PseudoBox};
use crate::text_map::{build_text_node_in_rect, has_element_pseudo, is_pure_text_element, map_text_style};
use designos_doc::{
    Affine, AutoLayout, BlendMode, Color, Command, Document, FrameProps, ImageScaleMode, LayoutAlign, LayoutDirection, Node, NodeId, NodeKind, Paint,
    PaintKind, Rect, Sizing, Vec2,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Options controlling how a [`Capture`] is imported into a [`Document`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImportOptions {
    /// Optional target `(x, y)` placement on the destination page.
    /// When `None`, the imported root frame is placed to the right of existing page content.
    pub origin: Option<(f32, f32)>,
    /// Whether to infer vertical [`AutoLayout`] for stacked `display: block` containers (§3.2).
    pub infer_block_auto_layout: bool,
    /// Whether to flatten pure inline wrappers (`<span>`, `<strong>`, `<em>`, `<a>`, ...) into [`designos_doc::TextRun`]s (§3.3).
    pub flatten_inline_wrappers: bool,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self { origin: None, infer_block_auto_layout: true, flatten_inline_wrappers: true }
    }
}

/// Record of a container that fell back to absolute positioning instead of auto-layout.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbsoluteFallback {
    pub element_id: String,
    pub tag: String,
    pub reason: String,
}

/// Record of a requested CSS font family that was substituted with a bundled/open font.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontSubstitution {
    pub requested: String,
    pub substituted: String,
}

/// Record of an `<img>` that could not be inlined during capture and was replaced by a placeholder rectangle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UncapturedImage {
    pub element_id: String,
    pub src: String,
    pub reason: String,
}

/// Record of an unsupported CSS property/value encountered during import.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsupportedCssEntry {
    pub element_id: String,
    pub property: String,
    pub value: String,
    pub note: String,
}

/// Structured report returned by [`import_capture`] for the post-import toast and `Details` sheet (§7).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportReport {
    pub root_node_id: NodeId,
    pub root_name: String,
    pub elements: usize,
    pub nodes_created: usize,
    pub flattened_wrappers: usize,
    pub truncated: bool,
    pub absolute_fallbacks: Vec<AbsoluteFallback>,
    pub font_substitutions: Vec<FontSubstitution>,
    pub images_not_captured: Vec<UncapturedImage>,
    pub unsupported_css: Vec<UnsupportedCssEntry>,
    pub notes: Vec<String>,
}

impl Default for ImportReport {
    fn default() -> Self {
        Self {
            root_node_id: NodeId(0),
            root_name: String::new(),
            elements: 0,
            nodes_created: 0,
            flattened_wrappers: 0,
            truncated: false,
            absolute_fallbacks: Vec::new(),
            font_substitutions: Vec::new(),
            images_not_captured: Vec::new(),
            unsupported_css: Vec::new(),
            notes: Vec::new(),
        }
    }
}

impl ImportReport {
    /// Formats the one-line summary toast required by §7:
    /// `Imported page: <N> layers · <M> fonts substituted · Details`
    pub fn summary_toast(&self) -> String {
        let plural = |n: usize, one: &str, many: &str| if n == 1 { one.to_string() } else { many.to_string() };
        let fonts = self.font_substitutions.len();
        format!(
            "Imported page: {} {} \u{00b7} {} {} substituted \u{00b7} Details",
            self.nodes_created,
            plural(self.nodes_created, "layer", "layers"),
            fonts,
            plural(fonts, "font", "fonts")
        )
    }
}

/// Errors returned by [`import_capture`].
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("target page {0:?} not found in document")]
    PageNotFound(NodeId),
    #[error("target node {0:?} is not a page")]
    NotAPage(NodeId),
    #[error(transparent)]
    Schema(#[from] CaptureError),
    #[error(transparent)]
    Doc(#[from] designos_doc::DocError),
}

struct IdGen {
    next: u64,
}

impl IdGen {
    fn new(doc: &Document) -> Self {
        let largest = doc.nodes.keys().next_back().map_or(0, |id| id.0);
        let start = doc.peek_next_id().0.max(largest + 1).max(1);
        Self { next: start }
    }

    fn alloc(&mut self) -> NodeId {
        let id = NodeId(self.next);
        self.next += 1;
        id
    }
}

/// Returns a concise selector-like label for `el` (`tag#id`, `tag.class`, `tag (aria)`, or `tag`).
pub fn element_selector_label(el: &Element) -> String {
    let tag = el.tag.to_ascii_lowercase();
    if let Some(id_attr) = el.id.as_deref().filter(|s| !s.trim().is_empty()) {
        return format!("{tag}#{}", id_attr.trim());
    }
    if let Some(cls) = el.classes.first().filter(|s| !s.trim().is_empty()) {
        return format!("{tag}.{}", cls.trim());
    }
    if let Some(aria) = el.aria_label.as_deref().filter(|s| !s.trim().is_empty()) {
        return format!("{tag} ({})", aria.trim());
    }
    tag
}

/// Imports a parsed [`Capture`] onto `page` in `doc`, returning a single undoable [`Command`]
/// and an [`ImportReport`] (§6).
pub fn import_capture(doc: &Document, page: NodeId, capture: &Capture, opts: &ImportOptions) -> Result<(Command, ImportReport), ImportError> {
    let page_node = doc.get(page).ok_or(ImportError::PageNotFound(page))?;
    if !matches!(page_node.kind, NodeKind::Page { .. }) {
        return Err(ImportError::NotAPage(page));
    }

    let mut ids = IdGen::new(doc);
    let mut working_images = doc.images.clone();
    let mut report = ImportReport {
        root_node_id: NodeId(0),
        root_name: make_root_frame_name(capture),
        elements: count_elements(&capture.root),
        nodes_created: 0,
        flattened_wrappers: 0,
        truncated: capture.truncated,
        ..Default::default()
    };

    let (place_x, place_y) = opts.origin.unwrap_or_else(|| next_free_page_spot(doc, page));
    let root_w = capture.scroll.width.max(capture.viewport.width).max(capture.root.border_box.width).max(1.0);
    let root_h = capture.scroll.height.max(capture.viewport.height).max(capture.root.border_box.height).max(1.0);

    let mut nodes: Vec<Node> = Vec::new();
    let root_id = build_root_frame(capture, Rect::new(place_x, place_y, root_w, root_h), opts, &mut ids, &mut working_images, &mut report, &mut nodes);
    report.root_node_id = root_id;
    report.nodes_created = nodes.len();

    let mut added_images = BTreeMap::new();
    for (k, v) in working_images {
        if !doc.images.contains_key(&k) {
            added_images.insert(k, v);
        }
    }

    let insert_cmd = Command::Insert { parent: page, index: page_node.children.len(), nodes };

    let cmd = if added_images.is_empty() { insert_cmd } else { Command::Batch(vec![Command::SetImages { add: added_images, remove: Vec::new() }, insert_cmd]) };

    Ok((cmd, report))
}

/// Parses `json` as a [`Capture`] and builds an undoable [`Command`] for `page` in `doc`.
pub fn import_capture_json(doc: &Document, page: NodeId, json: &str, opts: &ImportOptions) -> Result<(Command, ImportReport), ImportError> {
    let capture = Capture::from_json(json)?;
    import_capture(doc, page, &capture, opts)
}

/// Convenience helper that creates a new [`Document`] with a single page and applies [`import_capture`] to it.
pub fn import_capture_into_document(capture: &Capture, opts: &ImportOptions) -> Result<(Document, ImportReport), ImportError> {
    let doc_name = if !capture.title.trim().is_empty() { capture.title.trim().to_string() } else { "Web Capture".to_string() };
    let mut doc = Document::new(doc_name);
    let page = doc.add_page("Page 1");
    let (cmd, report) = import_capture(&doc, page, capture, opts)?;
    cmd.apply(&mut doc)?;
    Ok((doc, report))
}

fn count_elements(el: &Element) -> usize {
    1 + el.children.iter().map(count_elements).sum::<usize>()
}

fn next_free_page_spot(doc: &Document, page: NodeId) -> (f32, f32) {
    let Some(p) = doc.get(page) else {
        return (0.0, 0.0);
    };
    if p.children.is_empty() {
        return (0.0, 0.0);
    }
    let mut max_right = f32::NEG_INFINITY;
    let mut min_top = f32::INFINITY;
    for &cid in &p.children {
        if let Some(n) = doc.get(cid) {
            let right = n.transform.tx + n.size.x;
            if right > max_right {
                max_right = right;
            }
            if n.transform.ty < min_top {
                min_top = n.transform.ty;
            }
        }
    }
    if max_right.is_finite() && min_top.is_finite() {
        (max_right + 100.0, min_top)
    } else {
        (0.0, 0.0)
    }
}

fn make_root_frame_name(capture: &Capture) -> String {
    let title = capture.title.trim();
    let host = extract_url_host(&capture.url);
    match (!title.is_empty(), host) {
        (true, Some(h)) => format!("{title} \u{2014} {h}"),
        (true, None) => title.to_string(),
        (false, Some(h)) => h,
        (false, None) => {
            if !capture.url.trim().is_empty() {
                capture.url.trim().to_string()
            } else {
                "Web Capture".to_string()
            }
        }
    }
}

fn extract_url_host(url: &str) -> Option<String> {
    let s = url.trim();
    let after_scheme = s.split_once("://").map(|(_, r)| r).unwrap_or(s);
    let host_port = after_scheme.split('/').next()?.trim();
    if host_port.is_empty() {
        None
    } else {
        Some(host_port.to_string())
    }
}

#[allow(clippy::too_many_arguments)]
fn build_root_frame(
    capture: &Capture,
    root_rect: Rect,
    opts: &ImportOptions,
    ids: &mut IdGen,
    images: &mut BTreeMap<String, Vec<u8>>,
    report: &mut ImportReport,
    out_nodes: &mut Vec<Node>,
) -> NodeId {
    let root_el = &capture.root;
    let root_sel = element_selector_label(root_el);
    let root_id = ids.alloc();
    let mut root_node = Node::frame(root_id, root_rect);
    root_node.name = make_root_frame_name(capture);

    let mut fills = map_background_paints(&root_sel, &root_el.styles, images, report);
    // In CSS, when `<html>` has a transparent background, `<body>`'s background propagates to the canvas.
    if fills.is_empty() {
        if let Some(body) = root_el.children.iter().find(|c| c.tag.eq_ignore_ascii_case("body")) {
            let body_sel = element_selector_label(body);
            fills = map_background_paints(&body_sel, &body.styles, images, report);
        }
    }
    if fills.is_empty() {
        fills.push(Paint::solid(Color::WHITE));
    }
    root_node.fills = fills;
    root_node.stroke = map_stroke(&root_el.styles, 16.0);
    root_node.corner_radii = map_corner_radii(&root_el.styles, root_rect.w, root_rect.h, 16.0);
    let (effects, blend, opacity) = map_effects_and_blend(&root_sel, &root_el.styles, 16.0, report);
    root_node.effects = effects;
    root_node.blend = blend;
    root_node.opacity = opacity;

    let flow_children: Vec<&Element> = root_el.children.iter().filter(|c| !is_out_of_flow(c) && is_renderable_element(c)).collect();
    let flow_children = sort_flow_children_visually(root_el, flow_children);
    let abs_children: Vec<&Element> = root_el.children.iter().filter(|c| is_out_of_flow(c) && is_renderable_element(c)).collect();

    let auto_layout = infer_auto_layout(root_el, &flow_children, opts.infer_block_auto_layout, report);
    root_node.kind = NodeKind::Frame(FrameProps { clips_content: true, auto_layout: auto_layout.clone(), grids: Vec::new() });

    let root_slot = out_nodes.len();
    out_nodes.push(root_node);

    let page_origin = (0.0f32, 0.0f32);
    let mut child_ids = Vec::new();

    for child in &flow_children {
        if let Some(cid) = convert_element(child, root_id, page_origin, Some((root_el, auto_layout.as_ref())), opts, ids, images, report, out_nodes) {
            child_ids.push(cid);
        }
    }

    let mut sorted_abs = abs_children;
    sorted_abs.sort_by_key(|c| c.z);
    for child in &sorted_abs {
        if let Some(cid) = convert_element(child, root_id, page_origin, Some((root_el, auto_layout.as_ref())), opts, ids, images, report, out_nodes) {
            child_ids.push(cid);
        }
    }

    out_nodes[root_slot].children = child_ids;
    root_id
}

#[allow(clippy::too_many_arguments)]
fn convert_element(
    el: &Element,
    parent_id: NodeId,
    parent_page_origin: (f32, f32),
    parent_ctx: Option<(&Element, Option<&AutoLayout>)>,
    opts: &ImportOptions,
    ids: &mut IdGen,
    images: &mut BTreeMap<String, Vec<u8>>,
    report: &mut ImportReport,
    out_nodes: &mut Vec<Node>,
) -> Option<NodeId> {
    if !is_renderable_element(el) {
        return None;
    }

    let sel = element_selector_label(el);
    let rel_x = el.border_box.x - parent_page_origin.0;
    let rel_y = el.border_box.y - parent_page_origin.1;
    let w = el.border_box.width.max(1.0);
    let h = el.border_box.height.max(1.0);
    let rect = Rect::new(rel_x, rel_y, w, h);

    let layout_child = parent_ctx.map(|(p, al)| infer_layout_child(el, p, al)).unwrap_or_default();

    // 1. Inline <svg> element (`el.image.svg`)
    if let Some(svg_markup) = el.image.as_ref().and_then(|img| img.svg.as_deref()) {
        if let Some(svg_id) = convert_inline_svg(el, svg_markup, parent_id, rect, layout_child, ids, images, report, out_nodes) {
            return Some(svg_id);
        }
    }

    // 2. <img> / <canvas> element with `el.image`
    let tag_lower = el.tag.to_ascii_lowercase();
    let is_media_img =
        tag_lower == "img" || tag_lower == "canvas" || el.image.as_ref().is_some_and(|img| matches!(img.kind.as_deref(), Some("img") | Some("canvas")));
    if is_media_img {
        return Some(convert_image_or_placeholder(el, parent_id, rect, layout_child, ids, images, report, out_nodes));
    }

    // 3. Pure text element (either bare Text node or Frame wrapping a Text node when painted/padded)
    if is_pure_text_element(el, opts.flatten_inline_wrappers) {
        let has_box = has_own_box_paint(&el.styles) || has_element_pseudo(el);
        if !has_box {
            let (tx, ty, tw, th) = resolve_content_rect_in_parent(el, rel_x, rel_y, w, h);
            let text_id = ids.alloc();
            if let Some(mut text_node) = build_text_node_in_rect(text_id, el, Rect::new(tx, ty, tw, th), opts.flatten_inline_wrappers, true, report) {
                text_node.parent = Some(parent_id);
                text_node.layout_child = layout_child;
                out_nodes.push(text_node);
                return Some(text_id);
            }
        } else {
            return Some(convert_painted_text_box(el, parent_id, rect, layout_child, opts, ids, images, report, out_nodes));
        }
    }

    // 4. Form inputs (<input>, <textarea>, <select>) without text_runs
    if matches!(tag_lower.as_str(), "input" | "textarea" | "select") && el.children.is_empty() {
        return Some(convert_form_input_box(el, parent_id, rect, layout_child, ids, images, report, out_nodes));
    }

    // 5. General container / Frame
    let frame_id = ids.alloc();
    let font_size = el.styles.get("font-size").and_then(|s| parse_css_px(s, 16.0)).unwrap_or(16.0);

    let mut fills = map_background_paints(&sel, &el.styles, images, report);
    // Also check if `el.image` holds an inlined background image (`kind: "background"`) not already added.
    if let Some(img_ref) = el.image.as_ref().filter(|i| i.kind.as_deref() == Some("background")) {
        if !fills.iter().any(|p| matches!(p.kind, PaintKind::Image { .. })) {
            if let Some(ref data_url) = img_ref.data_url {
                if let Some((_, bytes)) = decode_data_url(data_url) {
                    let image_ref = store_image_dedup(images, bytes);
                    fills.push(Paint {
                        kind: PaintKind::Image { image_ref, scale_mode: ImageScaleMode::Fill },
                        opacity: 1.0,
                        visible: true,
                        blend: BlendMode::Normal,
                    });
                }
            }
        }
    }

    // Avoid duplicating the body fill when it was already propagated onto the root frame.
    if tag_lower == "body" && (el.border_box.width - parent_ctx.map(|(p, _)| p.border_box.width).unwrap_or(0.0)).abs() <= 1.0 {
        if let Some((root_el, _)) = parent_ctx {
            if root_el.tag.eq_ignore_ascii_case("html") && !has_own_box_paint(&root_el.styles) {
                fills.clear();
            }
        }
    }

    let stroke = map_stroke(&el.styles, font_size);
    let corner_radii = map_corner_radii(&el.styles, w, h, font_size);
    let (effects, blend, opacity) = map_effects_and_blend(&sel, &el.styles, font_size, report);
    let rad = map_rotation_rad(&sel, &el.styles, report);

    let flow_children: Vec<&Element> = el.children.iter().filter(|c| !is_out_of_flow(c) && is_renderable_element(c)).collect();
    let flow_children = sort_flow_children_visually(el, flow_children);
    let abs_children: Vec<&Element> = el.children.iter().filter(|c| is_out_of_flow(c) && is_renderable_element(c)).collect();

    let auto_layout = infer_auto_layout(el, &flow_children, opts.infer_block_auto_layout, report);
    let clips_content = map_overflow_clips(&el.styles);

    let mut frame_node = Node::frame(frame_id, rect);
    frame_node.name = make_element_layer_name(el);
    frame_node.parent = Some(parent_id);
    frame_node.fills = fills;
    frame_node.stroke = stroke;
    frame_node.corner_radii = corner_radii;
    frame_node.effects = effects;
    frame_node.blend = blend;
    frame_node.opacity = opacity;
    frame_node.transform = node_transform_at(rect.x, rect.y, rect.w, rect.h, rad);
    frame_node.layout_child = layout_child;
    frame_node.kind = NodeKind::Frame(FrameProps { clips_content, auto_layout: auto_layout.clone(), grids: Vec::new() });

    let frame_slot = out_nodes.len();
    out_nodes.push(frame_node);

    let my_page_origin = (el.border_box.x, el.border_box.y);
    let mut child_ids = Vec::new();

    if let Some(pb) = el.pseudo.as_ref().and_then(|p| p.before.as_ref()) {
        if let Some(pid) = convert_pseudo_box(pb, "::before", frame_id, my_page_origin, ids, images, report, out_nodes) {
            child_ids.push(pid);
        }
    }

    if el.text_runs.iter().any(|r| !r.text.trim().is_empty()) {
        let (tx, ty, tw, th) = resolve_content_rect_local(el, w, h, font_size);
        let tid = ids.alloc();
        if let Some(mut tnode) = build_text_node_in_rect(tid, el, Rect::new(tx, ty, tw, th), false, false, report) {
            tnode.parent = Some(frame_id);
            out_nodes.push(tnode);
            child_ids.push(tid);
        }
    }

    for child in &flow_children {
        if let Some(cid) = convert_element(child, frame_id, my_page_origin, Some((el, auto_layout.as_ref())), opts, ids, images, report, out_nodes) {
            child_ids.push(cid);
        }
    }

    let mut sorted_abs = abs_children;
    sorted_abs.sort_by_key(|c| c.z);
    for child in &sorted_abs {
        if let Some(cid) = convert_element(child, frame_id, my_page_origin, Some((el, auto_layout.as_ref())), opts, ids, images, report, out_nodes) {
            child_ids.push(cid);
        }
    }

    if let Some(pa) = el.pseudo.as_ref().and_then(|p| p.after.as_ref()) {
        if let Some(pid) = convert_pseudo_box(pa, "::after", frame_id, my_page_origin, ids, images, report, out_nodes) {
            child_ids.push(pid);
        }
    }

    out_nodes[frame_slot].children = child_ids;
    Some(frame_id)
}

fn resolve_content_rect_in_parent(el: &Element, rel_x: f32, rel_y: f32, w: f32, h: f32) -> (f32, f32, f32, f32) {
    let font_size = el.styles.get("font-size").and_then(|s| parse_css_px(s, 16.0)).unwrap_or(16.0);
    let (lx, ly, lw, lh) = resolve_content_rect_local(el, w, h, font_size);
    (rel_x + lx, rel_y + ly, lw, lh)
}

fn resolve_content_rect_local(el: &Element, w: f32, h: f32, font_size: f32) -> (f32, f32, f32, f32) {
    if el.content_box.width > 0.0 && el.content_box.height > 0.0 {
        let lx = (el.content_box.x - el.border_box.x).max(0.0);
        let ly = (el.content_box.y - el.border_box.y).max(0.0);
        return (lx, ly, el.content_box.width.max(1.0), el.content_box.height.max(1.0));
    }
    let pad = parse_padding(&el.styles, font_size);
    let bdr = parse_border_widths(&el.styles, font_size);
    let lx = pad[3] + bdr[3];
    let ly = pad[0] + bdr[0];
    let lw = (w - pad[1] - pad[3] - bdr[1] - bdr[3]).max(1.0);
    let lh = (h - pad[0] - pad[2] - bdr[0] - bdr[2]).max(1.0);
    (lx, ly, lw, lh)
}

#[allow(clippy::too_many_arguments)]
fn convert_painted_text_box(
    el: &Element,
    parent_id: NodeId,
    rect: Rect,
    layout_child: designos_doc::LayoutChild,
    opts: &ImportOptions,
    ids: &mut IdGen,
    images: &mut BTreeMap<String, Vec<u8>>,
    report: &mut ImportReport,
    out_nodes: &mut Vec<Node>,
) -> NodeId {
    let frame_id = ids.alloc();
    let sel = element_selector_label(el);
    let font_size = el.styles.get("font-size").and_then(|s| parse_css_px(s, 16.0)).unwrap_or(16.0);
    let pad = parse_padding(&el.styles, font_size);

    let fills = map_background_paints(&sel, &el.styles, images, report);
    let stroke = map_stroke(&el.styles, font_size);
    let corner_radii = map_corner_radii(&el.styles, rect.w, rect.h, font_size);
    let (effects, blend, opacity) = map_effects_and_blend(&sel, &el.styles, font_size, report);
    let rad = map_rotation_rad(&sel, &el.styles, report);
    let clips_content = map_overflow_clips(&el.styles);

    let text_align = el.styles.get("text-align").map(|s| s.trim().to_ascii_lowercase()).unwrap_or_default();
    let primary_align = if text_align == "center" {
        LayoutAlign::Center
    } else if text_align == "right" || text_align == "end" {
        LayoutAlign::End
    } else {
        LayoutAlign::Start
    };

    let auto_layout = Some(AutoLayout {
        direction: LayoutDirection::Horizontal,
        wrap: false,
        item_spacing: 4.0,
        counter_spacing: 0.0,
        padding: pad,
        primary_align,
        counter_align: LayoutAlign::Center,
        layout_version: 1,
        primary_sizing: Sizing::Hug,
        counter_sizing: Sizing::Hug,
        stroke_included: false,
        reverse_z: false,
        ..Default::default()
    });

    let mut frame_node = Node::frame(frame_id, rect);
    frame_node.name = make_element_layer_name(el);
    frame_node.parent = Some(parent_id);
    frame_node.fills = fills;
    frame_node.stroke = stroke;
    frame_node.corner_radii = corner_radii;
    frame_node.effects = effects;
    frame_node.blend = blend;
    frame_node.opacity = opacity;
    frame_node.transform = node_transform_at(rect.x, rect.y, rect.w, rect.h, rad);
    frame_node.layout_child = layout_child;
    frame_node.kind = NodeKind::Frame(FrameProps { clips_content, auto_layout, grids: Vec::new() });

    let frame_slot = out_nodes.len();
    out_nodes.push(frame_node);

    let my_page_origin = (el.border_box.x, el.border_box.y);
    let mut child_ids = Vec::new();

    if let Some(pb) = el.pseudo.as_ref().and_then(|p| p.before.as_ref()) {
        if let Some(pid) = convert_pseudo_box(pb, "::before", frame_id, my_page_origin, ids, images, report, out_nodes) {
            child_ids.push(pid);
        }
    }

    let (tx, ty, tw, th) = resolve_content_rect_local(el, rect.w, rect.h, font_size);
    let text_id = ids.alloc();
    if let Some(mut text_node) = build_text_node_in_rect(text_id, el, Rect::new(tx, ty, tw, th), opts.flatten_inline_wrappers, false, report) {
        text_node.parent = Some(frame_id);
        out_nodes.push(text_node);
        child_ids.push(text_id);
    }

    if let Some(pa) = el.pseudo.as_ref().and_then(|p| p.after.as_ref()) {
        if let Some(pid) = convert_pseudo_box(pa, "::after", frame_id, my_page_origin, ids, images, report, out_nodes) {
            child_ids.push(pid);
        }
    }

    out_nodes[frame_slot].children = child_ids;
    frame_id
}

#[allow(clippy::too_many_arguments)]
fn convert_form_input_box(
    el: &Element,
    parent_id: NodeId,
    rect: Rect,
    layout_child: designos_doc::LayoutChild,
    ids: &mut IdGen,
    images: &mut BTreeMap<String, Vec<u8>>,
    report: &mut ImportReport,
    out_nodes: &mut Vec<Node>,
) -> NodeId {
    let frame_id = ids.alloc();
    let sel = element_selector_label(el);
    let font_size = el.styles.get("font-size").and_then(|s| parse_css_px(s, 16.0)).unwrap_or(14.0);
    let pad = parse_padding(&el.styles, font_size);

    let fills = map_background_paints(&sel, &el.styles, images, report);
    let stroke = map_stroke(&el.styles, font_size);
    let corner_radii = map_corner_radii(&el.styles, rect.w, rect.h, font_size);
    let (effects, blend, opacity) = map_effects_and_blend(&sel, &el.styles, font_size, report);

    let mut frame_node = Node::frame(frame_id, rect);
    frame_node.name = make_element_layer_name(el);
    frame_node.parent = Some(parent_id);
    frame_node.fills = fills;
    frame_node.stroke = stroke;
    frame_node.corner_radii = corner_radii;
    frame_node.effects = effects;
    frame_node.blend = blend;
    frame_node.opacity = opacity;
    frame_node.layout_child = layout_child;
    frame_node.kind = NodeKind::Frame(FrameProps {
        clips_content: true,
        auto_layout: Some(AutoLayout {
            direction: LayoutDirection::Horizontal,
            padding: pad,
            primary_align: LayoutAlign::Start,
            counter_align: LayoutAlign::Center,
            layout_version: 1,
            primary_sizing: Sizing::Fixed,
            counter_sizing: Sizing::Fixed,
            ..Default::default()
        }),
        grids: Vec::new(),
    });

    let frame_slot = out_nodes.len();
    out_nodes.push(frame_node);

    let label = el.aria_label.as_deref().filter(|s| !s.trim().is_empty()).or_else(|| el.alt.as_deref().filter(|s| !s.trim().is_empty())).map(str::to_string);

    if let Some(text) = label {
        let mut style = map_text_style(&el.styles, report);
        style.fills = vec![Paint::solid(Color::rgb(148, 163, 184))];
        let (tx, ty, tw, th) = resolve_content_rect_local(el, rect.w, rect.h, font_size);
        let tid = ids.alloc();
        let mut tnode = Node::text(tid, Rect::new(tx, ty, tw, th), text, style);
        tnode.parent = Some(frame_id);
        out_nodes.push(tnode);
        out_nodes[frame_slot].children.push(tid);
    }

    frame_id
}

#[allow(clippy::too_many_arguments)]
fn convert_inline_svg(
    el: &Element,
    svg_markup: &str,
    parent_id: NodeId,
    rect: Rect,
    layout_child: designos_doc::LayoutChild,
    ids: &mut IdGen,
    images: &mut BTreeMap<String, Vec<u8>>,
    report: &mut ImportReport,
    out_nodes: &mut Vec<Node>,
) -> Option<NodeId> {
    let name = make_element_layer_name(el);
    match designos_svg::svg_to_nodes(svg_markup, &name, &mut || ids.alloc()) {
        Ok(mut svg_out) => {
            for (k, v) in svg_out.images {
                images.entry(k).or_insert(v);
            }
            let root = svg_out.nodes.first_mut()?;
            let root_id = root.id;
            root.parent = Some(parent_id);
            root.transform = Affine::translate(rect.x, rect.y);
            root.size = Vec2::new(rect.w, rect.h);
            root.layout_child = layout_child;
            out_nodes.extend(svg_out.nodes);
            Some(root_id)
        }
        Err(err) => {
            report.notes.push(format!("svg {} fallback to frame: {err}", element_selector_label(el)));
            None
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn convert_image_or_placeholder(
    el: &Element,
    parent_id: NodeId,
    rect: Rect,
    layout_child: designos_doc::LayoutChild,
    ids: &mut IdGen,
    images: &mut BTreeMap<String, Vec<u8>>,
    report: &mut ImportReport,
    out_nodes: &mut Vec<Node>,
) -> NodeId {
    let sel = element_selector_label(el);
    let font_size = el.styles.get("font-size").and_then(|s| parse_css_px(s, 16.0)).unwrap_or(16.0);

    if let Some(data_url) = el.image.as_ref().and_then(|i| i.data_url.as_deref()) {
        if let Some((mime, bytes)) = decode_data_url(data_url) {
            if mime.contains("svg") {
                if let Ok(svg_str) = std::str::from_utf8(&bytes) {
                    if let Some(svg_id) = convert_inline_svg(el, svg_str, parent_id, rect, layout_child, ids, images, report, out_nodes) {
                        return svg_id;
                    }
                }
            }
            let image_ref = store_image_dedup(images, bytes);
            let scale_mode = match el.styles.get("object-fit").map(|s| s.trim().to_ascii_lowercase()).as_deref() {
                Some("contain") | Some("scale-down") => ImageScaleMode::Fit,
                Some("none") => ImageScaleMode::Crop,
                _ => ImageScaleMode::Fill,
            };
            let node_id = ids.alloc();
            let mut node = Node::rectangle(node_id, rect);
            node.name = el.alt.as_deref().filter(|s| !s.trim().is_empty()).map(str::to_string).unwrap_or_else(|| make_element_layer_name(el));
            node.parent = Some(parent_id);
            let mut fills = map_background_paints(&sel, &el.styles, images, report);
            fills.push(Paint { kind: PaintKind::Image { image_ref, scale_mode }, opacity: 1.0, visible: true, blend: BlendMode::Normal });
            node.fills = fills;
            node.stroke = map_stroke(&el.styles, font_size);
            node.corner_radii = map_corner_radii(&el.styles, rect.w, rect.h, font_size);
            let (effects, blend, opacity) = map_effects_and_blend(&sel, &el.styles, font_size, report);
            node.effects = effects;
            node.blend = blend;
            node.opacity = opacity;
            node.layout_child = layout_child;
            out_nodes.push(node);
            return node_id;
        }
    }

    // Blocked or uncaptured image -> neutral placeholder rectangle + report entry (§3.4)
    let (src, reason) = match el.image.as_ref() {
        Some(img) => {
            let r = if let Some(ref s) = img.skipped {
                s.clone()
            } else if img.blocked {
                "blocked".to_string()
            } else if img.pending {
                "pending".to_string()
            } else {
                "not inlined".to_string()
            };
            (img.src.clone(), r)
        }
        None => (String::new(), "not inlined".to_string()),
    };

    report.images_not_captured.push(UncapturedImage { element_id: sel, src, reason: reason.clone() });

    let node_id = ids.alloc();
    let mut node = Node::rectangle(node_id, rect);
    node.name = format!("Image (not captured: {reason})");
    node.parent = Some(parent_id);
    node.fills = vec![Paint::solid(Color::rgb(226, 232, 240))];
    node.stroke = map_stroke(&el.styles, font_size);
    node.corner_radii = map_corner_radii(&el.styles, rect.w, rect.h, font_size);
    node.layout_child = layout_child;
    out_nodes.push(node);
    node_id
}

#[allow(clippy::too_many_arguments)]
fn convert_pseudo_box(
    pseudo: &PseudoBox,
    pseudo_name: &str,
    parent_id: NodeId,
    parent_page_origin: (f32, f32),
    ids: &mut IdGen,
    images: &mut BTreeMap<String, Vec<u8>>,
    report: &mut ImportReport,
    out_nodes: &mut Vec<Node>,
) -> Option<NodeId> {
    let text = pseudo.content.trim().trim_matches(|c| c == '"' || c == '\'').trim();
    let has_text = !text.is_empty() && !text.eq_ignore_ascii_case("none") && !text.eq_ignore_ascii_case("normal");

    if pseudo.border_box.width <= 0.0 && pseudo.border_box.height <= 0.0 && !has_text {
        return None;
    }
    let rel_x = pseudo.border_box.x - parent_page_origin.0;
    let rel_y = pseudo.border_box.y - parent_page_origin.1;
    let w = pseudo.border_box.width.max(1.0);
    let h = pseudo.border_box.height.max(1.0);
    let rect = Rect::new(rel_x, rel_y, w, h);

    if has_text && !has_own_box_paint(&pseudo.styles) {
        let id = ids.alloc();
        let style = map_text_style(&pseudo.styles, report);
        let mut node = Node::text(id, rect, text.to_string(), style);
        node.name = pseudo_name.into();
        node.parent = Some(parent_id);
        out_nodes.push(node);
        return Some(id);
    }

    if has_text && has_own_box_paint(&pseudo.styles) {
        let frame_id = ids.alloc();
        let font_size = pseudo.styles.get("font-size").and_then(|s| parse_css_px(s, 16.0)).unwrap_or(12.0);
        let pad = parse_padding(&pseudo.styles, font_size);
        let bdr = parse_border_widths(&pseudo.styles, font_size);
        let mut frame = Node::frame(frame_id, rect);
        frame.name = pseudo_name.into();
        frame.parent = Some(parent_id);
        frame.fills = map_background_paints(pseudo_name, &pseudo.styles, images, report);
        frame.stroke = map_stroke(&pseudo.styles, font_size);
        frame.corner_radii = map_corner_radii(&pseudo.styles, w, h, font_size);
        let (effects, blend, opacity) = map_effects_and_blend(pseudo_name, &pseudo.styles, font_size, report);
        frame.effects = effects;
        frame.blend = blend;
        frame.opacity = opacity;
        frame.kind = NodeKind::Frame(FrameProps {
            clips_content: false,
            auto_layout: Some(AutoLayout {
                direction: LayoutDirection::Horizontal,
                padding: pad,
                primary_align: LayoutAlign::Center,
                counter_align: LayoutAlign::Center,
                layout_version: 1,
                primary_sizing: Sizing::Hug,
                counter_sizing: Sizing::Hug,
                ..Default::default()
            }),
            grids: Vec::new(),
        });
        let slot = out_nodes.len();
        out_nodes.push(frame);

        let tid = ids.alloc();
        let style = map_text_style(&pseudo.styles, report);
        let tx = pad[3] + bdr[3];
        let ty = pad[0] + bdr[0];
        let tw = (w - pad[1] - pad[3] - bdr[1] - bdr[3]).max(1.0);
        let th = (h - pad[0] - pad[2] - bdr[0] - bdr[2]).max(1.0);
        let mut tnode = Node::text(tid, Rect::new(tx, ty, tw, th), text.to_string(), style);
        tnode.name = text.to_string();
        tnode.parent = Some(frame_id);
        out_nodes.push(tnode);
        out_nodes[slot].children.push(tid);
        return Some(frame_id);
    }

    let id = ids.alloc();
    let mut node = Node::rectangle(id, rect);
    node.name = pseudo_name.into();
    node.parent = Some(parent_id);
    node.fills = map_background_paints(pseudo_name, &pseudo.styles, images, report);
    node.stroke = map_stroke(&pseudo.styles, 16.0);
    node.corner_radii = map_corner_radii(&pseudo.styles, w, h, 16.0);
    let (effects, blend, opacity) = map_effects_and_blend(pseudo_name, &pseudo.styles, 16.0, report);
    node.effects = effects;
    node.blend = blend;
    node.opacity = opacity;
    if node.fills.is_empty() && node.stroke.paints.is_empty() && node.effects.is_empty() {
        if let Some(c) = pseudo.styles.get("color").and_then(|s| parse_css_color(s)) {
            if c.a > 0.0 {
                node.fills.push(Paint::solid(c));
            }
        }
    }
    out_nodes.push(node);
    Some(id)
}

fn is_renderable_element(el: &Element) -> bool {
    if !el.visible {
        return false;
    }
    if matches!(el.styles.get("display").map(|s| s.trim().to_ascii_lowercase()).as_deref(), Some("none")) {
        return false;
    }
    if matches!(el.styles.get("visibility").map(|s| s.trim().to_ascii_lowercase()).as_deref(), Some("hidden") | Some("collapse")) {
        return false;
    }
    let tag = el.tag.to_ascii_lowercase();
    if matches!(tag.as_str(), "script" | "style" | "meta" | "link" | "head" | "title" | "noscript" | "template") {
        return false;
    }
    if el.border_box.width <= 0.0 && el.border_box.height <= 0.0 && el.children.is_empty() && el.image.is_none() && el.text_runs.is_empty() {
        return false;
    }
    true
}

fn make_element_layer_name(el: &Element) -> String {
    if let Some(comp) = el.component_hint.as_deref().or(el.framework_name.as_deref()).filter(|s| !s.trim().is_empty()) {
        return format!("{} ({})", element_selector_label(el), comp.trim());
    }
    element_selector_label(el)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{BoxRect, ImageRef, ScrollSize, TextRun as SchemaTextRun, Viewport};
    use designos_doc::History;

    const CARD_FIXTURE_JSON: &str = include_str!("../tests/playwright/fixtures/card.capture.json");

    #[test]
    fn summary_toast_uses_singular_nouns_for_one() {
        let mut report = ImportReport { nodes_created: 1, ..ImportReport::default() };
        report.font_substitutions.push(FontSubstitution { requested: "Arial".into(), substituted: "Inter".into() });
        assert_eq!(report.summary_toast(), "Imported page: 1 layer \u{00b7} 1 font substituted \u{00b7} Details");

        report.nodes_created = 16;
        report.font_substitutions.push(FontSubstitution { requested: "Georgia".into(), substituted: "Inter".into() });
        assert_eq!(report.summary_toast(), "Imported page: 16 layers \u{00b7} 2 fonts substituted \u{00b7} Details");

        report.font_substitutions.clear();
        assert_eq!(report.summary_toast(), "Imported page: 16 layers \u{00b7} 0 fonts substituted \u{00b7} Details");
    }

    fn elem(id: Option<&str>, tag: &str, bbox: BoxRect, styles: &[(&str, &str)], text: &str, children: Vec<Element>) -> Element {
        let style_map: BTreeMap<String, String> = styles.iter().map(|(k, v)| ((*k).to_string(), (*v).to_string())).collect();
        let text_runs = if text.is_empty() {
            Vec::new()
        } else {
            vec![SchemaTextRun { text: text.to_string(), styles: style_map.clone(), lines: vec![bbox], lines_truncated: false }]
        };
        Element {
            tag: tag.into(),
            id: id.map(str::to_string),
            classes: Vec::new(),
            border_box: bbox,
            content_box: bbox,
            visible: true,
            styles: style_map,
            text_runs,
            children,
            ..Default::default()
        }
    }

    #[test]
    fn flex_maps_to_auto_layout() {
        let capture = Capture::from_json(CARD_FIXTURE_JSON).expect("parse card fixture");
        let (doc, report) = import_capture_into_document(&capture, &ImportOptions::default()).expect("import card");
        assert!(report.nodes_created >= 8);

        let card = doc.nodes.values().find(|n| n.name.starts_with("article#pricing-card")).expect("article#pricing-card node exists");
        let fp = card.kind.frame_props().expect("card is a Frame");
        let al = fp.auto_layout.as_ref().expect("card has inferred AutoLayout");
        assert_eq!(al.direction, LayoutDirection::Vertical);
        assert_eq!(al.item_spacing, 12.0);
        assert_eq!(al.padding, [20.0, 20.0, 20.0, 20.0]);

        // Also test a horizontal flex row with wrap, gap, justify-content: space-between, align-items: center, and flex-grow child
        let child1 = elem(Some("c1"), "div", BoxRect { x: 16.0, y: 12.0, width: 120.0, height: 32.0 }, &[("display", "block"), ("flex-grow", "1")], "", vec![]);
        let child2 = elem(Some("c2"), "div", BoxRect { x: 152.0, y: 12.0, width: 80.0, height: 32.0 }, &[("display", "block")], "", vec![]);
        let row = elem(
            Some("top-nav"),
            "nav",
            BoxRect { x: 0.0, y: 0.0, width: 248.0, height: 56.0 },
            &[
                ("display", "flex"),
                ("flex-direction", "row"),
                ("flex-wrap", "wrap"),
                ("column-gap", "16px"),
                ("row-gap", "8px"),
                ("padding-top", "12px"),
                ("padding-right", "16px"),
                ("padding-bottom", "12px"),
                ("padding-left", "16px"),
                ("justify-content", "space-between"),
                ("align-items", "center"),
            ],
            "",
            vec![child1, child2],
        );
        let root = elem(None, "html", BoxRect { x: 0.0, y: 0.0, width: 248.0, height: 56.0 }, &[("display", "block")], "", vec![row]);
        let cap = Capture {
            url: "https://example.com".into(),
            title: "Flex Test".into(),
            viewport: Viewport { width: 248.0, height: 56.0, dpr: 1.0 },
            scroll: ScrollSize { width: 248.0, height: 56.0 },
            root,
            ..Default::default()
        };
        let (doc2, _) = import_capture_into_document(&cap, &ImportOptions::default()).unwrap();
        let nav = doc2.nodes.values().find(|n| n.name == "nav#top-nav").unwrap();
        let nav_al = nav.kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
        assert_eq!(nav_al.direction, LayoutDirection::Horizontal);
        assert!(nav_al.wrap);
        assert_eq!(nav_al.item_spacing, 16.0);
        assert_eq!(nav_al.counter_spacing, 8.0);
        assert_eq!(nav_al.padding, [12.0, 16.0, 12.0, 16.0]);
        assert_eq!(nav_al.primary_align, LayoutAlign::SpaceBetween);
        assert_eq!(nav_al.counter_align, LayoutAlign::Center);
        let first_child = doc2.get(nav.children[0]).unwrap();
        assert_eq!(first_child.layout_child.horizontal, Sizing::Fill);
        assert_eq!(first_child.layout_child.grow, 1.0);
    }

    #[test]
    fn text_stays_editable() {
        let capture = Capture::from_json(CARD_FIXTURE_JSON).expect("parse card fixture");
        let (doc, report) = import_capture_into_document(&capture, &ImportOptions::default()).expect("import card");

        let text_nodes: Vec<&Node> = doc.nodes.values().filter(|n| matches!(n.kind, NodeKind::Text(_))).collect();
        assert!(text_nodes.len() >= 4, "expected at least 4 editable text nodes, got {}", text_nodes.len());

        let contents: Vec<&str> = text_nodes.iter().filter_map(|n| n.kind.any_text().map(|t| t.content.as_str())).collect();
        assert!(contents.contains(&"PRO"));
        assert!(contents.contains(&"DesignOS Pro Studio"));
        assert!(contents.iter().any(|c| c.contains("Build pixel-accurate vector interfaces")));
        assert!(contents.contains(&"Upgrade Now"));
        assert!(report.font_substitutions.iter().any(|s| s.requested == "Arial" && s.substituted == "Inter"));

        // Verify inline wrapper flattening (`<strong>`, `<a>`) into `TextRun`s
        let link_el = elem(
            Some("docs-link"),
            "a",
            BoxRect { x: 90.0, y: 0.0, width: 40.0, height: 20.0 },
            &[("display", "inline"), ("color", "rgb(37, 99, 235)"), ("text-decoration-line", "underline")],
            "Studio",
            vec![],
        );
        let strong_el =
            elem(None, "strong", BoxRect { x: 40.0, y: 0.0, width: 45.0, height: 20.0 }, &[("display", "inline"), ("font-weight", "700")], "Design", vec![]);
        let para = elem(
            Some("intro"),
            "p",
            BoxRect { x: 0.0, y: 0.0, width: 200.0, height: 20.0 },
            &[("display", "block"), ("font-family", "Inter"), ("font-size", "14px"), ("color", "rgb(15, 23, 42)")],
            "Open",
            vec![strong_el, link_el],
        );
        let root = elem(None, "html", BoxRect { x: 0.0, y: 0.0, width: 200.0, height: 20.0 }, &[("display", "block")], "", vec![para]);
        let cap = Capture {
            url: "https://example.com".into(),
            title: "Inline Text".into(),
            viewport: Viewport { width: 200.0, height: 20.0, dpr: 1.0 },
            scroll: ScrollSize { width: 200.0, height: 20.0 },
            root,
            ..Default::default()
        };
        let (doc2, report2) = import_capture_into_document(&cap, &ImportOptions::default()).unwrap();
        assert_eq!(report2.flattened_wrappers, 2);
        assert!(report2.notes.iter().any(|n| n.contains("a#docs-link")));
        let p_node = doc2.nodes.values().find(|n| matches!(n.kind, NodeKind::Text(_))).expect("flattened paragraph text node");
        let tp = p_node.kind.any_text().unwrap();
        assert_eq!(tp.content, "Open Design Studio");
        assert_eq!(tp.runs.len(), 2);
        assert_eq!(tp.runs[0].style.font_weight, 700);
        assert_eq!(tp.runs[1].style.decoration, designos_doc::TextDecoration::Underline);
    }

    #[test]
    fn block_flow_inference() {
        let b1 = elem(None, "div", BoxRect { x: 20.0, y: 20.0, width: 200.0, height: 30.0 }, &[("display", "block")], "", vec![]);
        let b2 = elem(None, "div", BoxRect { x: 20.5, y: 62.0, width: 200.0, height: 30.0 }, &[("display", "block")], "", vec![]);
        let b3 = elem(None, "div", BoxRect { x: 20.0, y: 104.5, width: 200.0, height: 30.0 }, &[("display", "block")], "", vec![]);
        let stacked = elem(
            Some("stack"),
            "section",
            BoxRect { x: 0.0, y: 0.0, width: 240.0, height: 154.0 },
            &[("display", "block"), ("padding-top", "20px"), ("padding-left", "20px"), ("padding-right", "20px"), ("padding-bottom", "20px")],
            "",
            vec![b1, b2, b3],
        );
        let root = elem(None, "html", BoxRect { x: 0.0, y: 0.0, width: 240.0, height: 154.0 }, &[("display", "block")], "", vec![stacked]);
        let cap = Capture {
            url: "https://example.com".into(),
            title: "Block Flow".into(),
            viewport: Viewport { width: 240.0, height: 154.0, dpr: 1.0 },
            scroll: ScrollSize { width: 240.0, height: 154.0 },
            root,
            ..Default::default()
        };
        let (doc, _) = import_capture_into_document(&cap, &ImportOptions::default()).unwrap();
        let sec = doc.nodes.values().find(|n| n.name == "section#stack").unwrap();
        let al = sec.kind.frame_props().unwrap().auto_layout.as_ref().expect("inferred vertical block auto-layout");
        assert_eq!(al.direction, LayoutDirection::Vertical);
        assert_eq!(al.item_spacing, 12.0);
        assert_eq!(al.padding, [20.0, 20.0, 20.0, 20.0]);
    }

    #[test]
    fn blocked_image_placeholder_reported() {
        let mut img = elem(
            Some("hero-photo"),
            "img",
            BoxRect { x: 10.0, y: 10.0, width: 120.0, height: 80.0 },
            &[("display", "block"), ("border-top-left-radius", "8px")],
            "",
            vec![],
        );
        img.image = Some(ImageRef {
            src: "https://cdn.example.com/photo.jpg".into(),
            blocked: true,
            skipped: Some("tainted".into()),
            kind: Some("img".into()),
            ..Default::default()
        });
        let root = elem(None, "html", BoxRect { x: 0.0, y: 0.0, width: 200.0, height: 120.0 }, &[("display", "block")], "", vec![img]);
        let cap = Capture {
            url: "https://example.com".into(),
            title: "Blocked Image".into(),
            viewport: Viewport { width: 200.0, height: 120.0, dpr: 1.0 },
            scroll: ScrollSize { width: 200.0, height: 120.0 },
            root,
            ..Default::default()
        };

        let (doc, report) = import_capture_into_document(&cap, &ImportOptions::default()).unwrap();
        assert_eq!(report.images_not_captured.len(), 1);
        assert_eq!(report.images_not_captured[0].reason, "tainted");
        assert_eq!(report.images_not_captured[0].src, "https://cdn.example.com/photo.jpg");

        let placeholder = doc.nodes.values().find(|n| n.name == "Image (not captured: tainted)").expect("placeholder rectangle created");
        assert!(matches!(placeholder.kind, NodeKind::Rectangle));
        assert!(!placeholder.fills.is_empty());
    }

    #[test]
    fn one_undo_step() {
        let capture = Capture::from_json(CARD_FIXTURE_JSON).expect("parse card fixture");
        let mut doc = Document::new("Undo Test");
        let page = doc.add_page("Page 1");
        let mut history = History::default();

        let initial_nodes = doc.nodes.len();
        let initial_images = doc.images.len();

        let (cmd, report) = import_capture(&doc, page, &capture, &ImportOptions::default()).expect("import_capture");
        history.exec(&mut doc, cmd).expect("exec import command");

        assert_eq!(history.undo_depth(), 1, "entire import must be a single undo step");
        assert!(doc.get(report.root_node_id).is_some());
        assert!(doc.nodes.len() > initial_nodes);

        let undone = history.undo(&mut doc).expect("undo import");
        assert!(undone);
        assert_eq!(doc.nodes.len(), initial_nodes);
        assert_eq!(doc.images.len(), initial_images);
        assert!(doc.get(report.root_node_id).is_none());
    }

    #[test]
    fn screenshot_diff_within_threshold() {
        use designos_diff::{compare, DiffOptions};

        let cases: [(&str, Capture, &[u8]); 5] = [
            ("card", build_card_fixture(), include_bytes!("../tests/playwright/fixtures/ref_card.png")),
            ("nav", build_nav_fixture(), include_bytes!("../tests/playwright/fixtures/ref_nav.png")),
            ("pricing_grid", build_pricing_grid_fixture(), include_bytes!("../tests/playwright/fixtures/ref_pricing_grid.png")),
            ("article", build_article_fixture(), include_bytes!("../tests/playwright/fixtures/ref_article.png")),
            ("form", build_form_fixture(), include_bytes!("../tests/playwright/fixtures/ref_form.png")),
        ];

        for (name, cap, ref_png_bytes) in cases {
            let (doc, _report) = import_capture_into_document(&cap, &ImportOptions::default()).unwrap_or_else(|e| panic!("import {name}: {e}"));
            let page = doc.pages[0];
            let w = cap.viewport.width.max(1.0) as u32;
            let h = cap.viewport.height.max(1.0) as u32;
            let rendered = render_document_rgba(&doc, page, w, h);
            let reference = image::load_from_memory(ref_png_bytes).unwrap_or_else(|e| panic!("decode ref_{name}.png: {e}")).to_rgba8();
            let d = compare(&rendered, &reference, &DiffOptions { tolerance: 16, detect_aa: true, ignore: Vec::new() });
            assert!(d.ratio <= 0.03, "fixture {name} diff ratio {:.4} exceeded 0.03 threshold", d.ratio);
        }
    }

    fn render_document_rgba(doc: &Document, page: NodeId, w: u32, h: u32) -> image::RgbaImage {
        let mut pm = designos_render::tiny_skia::Pixmap::new(w, h).expect("pixmap");
        designos_render::render_page(doc, page, &designos_render::Viewport::default(), &mut pm);
        image::RgbaImage::from_raw(w, h, pm.take()).expect("rgba image")
    }

    fn build_card_fixture() -> Capture {
        let title = elem(
            Some("title"),
            "h2",
            BoxRect { x: 40.0, y: 40.0, width: 280.0, height: 24.0 },
            &[
                ("display", "block"),
                ("font-family", "Inter"),
                ("font-size", "20px"),
                ("font-weight", "700"),
                ("line-height", "24px"),
                ("color", "rgb(15, 23, 42)"),
            ],
            "Pro Workspace",
            vec![],
        );
        let desc = elem(
            Some("desc"),
            "p",
            BoxRect { x: 40.0, y: 76.0, width: 280.0, height: 20.0 },
            &[
                ("display", "block"),
                ("font-family", "Inter"),
                ("font-size", "14px"),
                ("font-weight", "400"),
                ("line-height", "20px"),
                ("color", "rgb(71, 85, 105)"),
            ],
            "Unlimited projects and shared libraries.",
            vec![],
        );
        let btn = elem(
            Some("btn"),
            "div",
            BoxRect { x: 40.0, y: 108.0, width: 120.0, height: 36.0 },
            &[
                ("display", "flex"),
                ("background-color", "rgb(37, 99, 235)"),
                ("color", "rgb(255, 255, 255)"),
                ("font-family", "Inter"),
                ("font-size", "14px"),
                ("font-weight", "600"),
                ("line-height", "20px"),
                ("padding-top", "8px"),
                ("padding-bottom", "8px"),
                ("padding-left", "16px"),
                ("padding-right", "16px"),
                ("border-top-left-radius", "8px"),
                ("border-top-right-radius", "8px"),
                ("border-bottom-right-radius", "8px"),
                ("border-bottom-left-radius", "8px"),
            ],
            "Upgrade",
            vec![],
        );
        let card = elem(
            Some("card"),
            "article",
            BoxRect { x: 20.0, y: 20.0, width: 320.0, height: 160.0 },
            &[
                ("display", "flex"),
                ("flex-direction", "column"),
                ("row-gap", "12px"),
                ("padding-top", "20px"),
                ("padding-right", "20px"),
                ("padding-bottom", "20px"),
                ("padding-left", "20px"),
                ("background-color", "rgb(255, 255, 255)"),
                ("border-top-width", "1px"),
                ("border-right-width", "1px"),
                ("border-bottom-width", "1px"),
                ("border-left-width", "1px"),
                ("border-top-style", "solid"),
                ("border-right-style", "solid"),
                ("border-bottom-style", "solid"),
                ("border-left-style", "solid"),
                ("border-top-color", "rgb(226, 232, 240)"),
                ("border-right-color", "rgb(226, 232, 240)"),
                ("border-bottom-color", "rgb(226, 232, 240)"),
                ("border-left-color", "rgb(226, 232, 240)"),
                ("border-top-left-radius", "12px"),
                ("border-top-right-radius", "12px"),
                ("border-bottom-right-radius", "12px"),
                ("border-bottom-left-radius", "12px"),
            ],
            "",
            vec![title, desc, btn],
        );
        let root = elem(
            None,
            "html",
            BoxRect { x: 0.0, y: 0.0, width: 360.0, height: 200.0 },
            &[("display", "block"), ("background-color", "rgb(241, 245, 249)")],
            "",
            vec![card],
        );
        Capture {
            url: "https://designos.dev/card".into(),
            title: "Card Fixture".into(),
            viewport: Viewport { width: 360.0, height: 200.0, dpr: 1.0 },
            scroll: ScrollSize { width: 360.0, height: 200.0 },
            root,
            ..Default::default()
        }
    }

    fn build_nav_fixture() -> Capture {
        let brand = elem(
            Some("brand"),
            "span",
            BoxRect { x: 24.0, y: 20.0, width: 110.0, height: 24.0 },
            &[("display", "inline-block"), ("font-family", "Inter"), ("font-size", "18px"), ("font-weight", "700"), ("color", "rgb(15, 23, 42)")],
            "Design OS",
            vec![],
        );
        let cta = elem(
            Some("cta"),
            "button",
            BoxRect { x: 660.0, y: 14.0, width: 116.0, height: 36.0 },
            &[
                ("display", "inline-flex"),
                ("background-color", "rgb(37, 99, 235)"),
                ("color", "rgb(255, 255, 255)"),
                ("font-family", "Inter"),
                ("font-size", "14px"),
                ("font-weight", "600"),
                ("padding-top", "8px"),
                ("padding-bottom", "8px"),
                ("padding-left", "16px"),
                ("padding-right", "16px"),
                ("border-top-left-radius", "8px"),
                ("border-top-right-radius", "8px"),
                ("border-bottom-right-radius", "8px"),
                ("border-bottom-left-radius", "8px"),
            ],
            "Get Started",
            vec![],
        );
        let nav = elem(
            Some("header"),
            "header",
            BoxRect { x: 0.0, y: 0.0, width: 800.0, height: 64.0 },
            &[
                ("display", "flex"),
                ("flex-direction", "row"),
                ("justify-content", "space-between"),
                ("align-items", "center"),
                ("padding-left", "24px"),
                ("padding-right", "24px"),
                ("background-color", "rgb(255, 255, 255)"),
                ("border-bottom-width", "1px"),
                ("border-bottom-style", "solid"),
                ("border-bottom-color", "rgb(226, 232, 240)"),
            ],
            "",
            vec![brand, cta],
        );
        let root = elem(
            None,
            "html",
            BoxRect { x: 0.0, y: 0.0, width: 800.0, height: 64.0 },
            &[("display", "block"), ("background-color", "rgb(248, 250, 252)")],
            "",
            vec![nav],
        );
        Capture {
            url: "https://designos.dev/nav".into(),
            title: "Nav Fixture".into(),
            viewport: Viewport { width: 800.0, height: 64.0, dpr: 1.0 },
            scroll: ScrollSize { width: 800.0, height: 64.0 },
            root,
            ..Default::default()
        }
    }

    fn build_pricing_grid_fixture() -> Capture {
        let tier = |id: &str, x: f32, title: &str, price: &str| {
            let h3 = elem(
                None,
                "h3",
                BoxRect { x: x + 20.0, y: 40.0, width: 160.0, height: 24.0 },
                &[("display", "block"), ("font-family", "Inter"), ("font-size", "16px"), ("font-weight", "600"), ("color", "rgb(15, 23, 42)")],
                title,
                vec![],
            );
            let p = elem(
                None,
                "p",
                BoxRect { x: x + 20.0, y: 76.0, width: 160.0, height: 36.0 },
                &[("display", "block"), ("font-family", "Inter"), ("font-size", "28px"), ("font-weight", "700"), ("color", "rgb(37, 99, 235)")],
                price,
                vec![],
            );
            elem(
                Some(id),
                "div",
                BoxRect { x, y: 20.0, width: 200.0, height: 140.0 },
                &[
                    ("display", "flex"),
                    ("flex-direction", "column"),
                    ("row-gap", "12px"),
                    ("padding-top", "20px"),
                    ("padding-left", "20px"),
                    ("padding-right", "20px"),
                    ("padding-bottom", "20px"),
                    ("background-color", "rgb(255, 255, 255)"),
                    ("border-top-width", "1px"),
                    ("border-right-width", "1px"),
                    ("border-bottom-width", "1px"),
                    ("border-left-width", "1px"),
                    ("border-top-style", "solid"),
                    ("border-right-style", "solid"),
                    ("border-bottom-style", "solid"),
                    ("border-left-style", "solid"),
                    ("border-top-color", "rgb(226, 232, 240)"),
                    ("border-right-color", "rgb(226, 232, 240)"),
                    ("border-bottom-color", "rgb(226, 232, 240)"),
                    ("border-left-color", "rgb(226, 232, 240)"),
                    ("border-top-left-radius", "12px"),
                    ("border-top-right-radius", "12px"),
                    ("border-bottom-right-radius", "12px"),
                    ("border-bottom-left-radius", "12px"),
                ],
                "",
                vec![h3, p],
            )
        };
        let grid = elem(
            Some("tiers"),
            "section",
            BoxRect { x: 0.0, y: 0.0, width: 680.0, height: 180.0 },
            &[
                ("display", "grid"),
                ("grid-template-columns", "200px 200px 200px"),
                ("column-gap", "20px"),
                ("row-gap", "20px"),
                ("padding-top", "20px"),
                ("padding-left", "20px"),
                ("padding-right", "20px"),
                ("padding-bottom", "20px"),
            ],
            "",
            vec![tier("starter", 20.0, "Starter", "$0"), tier("pro", 240.0, "Pro", "$19"), tier("team", 460.0, "Team", "$49")],
        );
        let root = elem(
            None,
            "html",
            BoxRect { x: 0.0, y: 0.0, width: 680.0, height: 180.0 },
            &[("display", "block"), ("background-color", "rgb(241, 245, 249)")],
            "",
            vec![grid],
        );
        Capture {
            url: "https://designos.dev/pricing".into(),
            title: "Pricing Grid".into(),
            viewport: Viewport { width: 680.0, height: 180.0, dpr: 1.0 },
            scroll: ScrollSize { width: 680.0, height: 180.0 },
            root,
            ..Default::default()
        }
    }

    fn build_article_fixture() -> Capture {
        let h1 = elem(
            None,
            "h1",
            BoxRect { x: 40.0, y: 32.0, width: 560.0, height: 36.0 },
            &[
                ("display", "block"),
                ("font-family", "Inter"),
                ("font-size", "28px"),
                ("line-height", "36px"),
                ("font-weight", "700"),
                ("color", "rgb(15, 23, 42)"),
            ],
            "Editable Web Captures",
            vec![],
        );
        let p1 = elem(
            None,
            "p",
            BoxRect { x: 40.0, y: 84.0, width: 560.0, height: 24.0 },
            &[("display", "block"), ("font-family", "Inter"), ("font-size", "16px"), ("line-height", "24px"), ("color", "rgb(51, 65, 85)")],
            "Every heading, paragraph, and inline link imports as a native text layer.",
            vec![],
        );
        let article = elem(
            Some("post"),
            "article",
            BoxRect { x: 0.0, y: 0.0, width: 640.0, height: 200.0 },
            &[("display", "block"), ("padding-top", "32px"), ("padding-left", "40px"), ("padding-right", "40px"), ("padding-bottom", "32px")],
            "",
            vec![h1, p1],
        );
        let root = elem(
            None,
            "html",
            BoxRect { x: 0.0, y: 0.0, width: 640.0, height: 200.0 },
            &[("display", "block"), ("background-color", "rgb(255, 255, 255)")],
            "",
            vec![article],
        );
        Capture {
            url: "https://designos.dev/blog".into(),
            title: "Article".into(),
            viewport: Viewport { width: 640.0, height: 200.0, dpr: 1.0 },
            scroll: ScrollSize { width: 640.0, height: 200.0 },
            root,
            ..Default::default()
        }
    }

    fn build_form_fixture() -> Capture {
        let label = elem(
            None,
            "label",
            BoxRect { x: 24.0, y: 24.0, width: 312.0, height: 20.0 },
            &[("display", "block"), ("font-family", "Inter"), ("font-size", "14px"), ("font-weight", "600"), ("color", "rgb(15, 23, 42)")],
            "Workspace URL",
            vec![],
        );
        let mut input = elem(
            Some("url-input"),
            "input",
            BoxRect { x: 24.0, y: 56.0, width: 312.0, height: 40.0 },
            &[
                ("display", "block"),
                ("font-family", "Inter"),
                ("font-size", "13px"),
                ("background-color", "rgb(255, 255, 255)"),
                ("border-top-width", "1px"),
                ("border-right-width", "1px"),
                ("border-bottom-width", "1px"),
                ("border-left-width", "1px"),
                ("border-top-style", "solid"),
                ("border-right-style", "solid"),
                ("border-bottom-style", "solid"),
                ("border-left-style", "solid"),
                ("border-top-color", "rgb(203, 213, 225)"),
                ("border-right-color", "rgb(203, 213, 225)"),
                ("border-bottom-color", "rgb(203, 213, 225)"),
                ("border-left-color", "rgb(203, 213, 225)"),
                ("border-top-left-radius", "6px"),
                ("border-top-right-radius", "6px"),
                ("border-bottom-right-radius", "6px"),
                ("border-bottom-left-radius", "6px"),
                ("padding-left", "12px"),
                ("padding-right", "12px"),
                ("padding-top", "10px"),
                ("padding-bottom", "10px"),
            ],
            "",
            vec![],
        );
        input.aria_label = Some("https://designos.dev".into());
        let submit = elem(
            Some("submit"),
            "button",
            BoxRect { x: 24.0, y: 108.0, width: 312.0, height: 40.0 },
            &[
                ("display", "flex"),
                ("background-color", "rgb(15, 23, 42)"),
                ("color", "rgb(255, 255, 255)"),
                ("font-family", "Inter"),
                ("font-size", "14px"),
                ("font-weight", "600"),
                ("padding-top", "10px"),
                ("padding-bottom", "10px"),
                ("padding-left", "16px"),
                ("padding-right", "16px"),
                ("border-top-left-radius", "6px"),
                ("border-top-right-radius", "6px"),
                ("border-bottom-right-radius", "6px"),
                ("border-bottom-left-radius", "6px"),
            ],
            "Import Capture",
            vec![],
        );
        let form = elem(
            Some("import-form"),
            "form",
            BoxRect { x: 0.0, y: 0.0, width: 360.0, height: 172.0 },
            &[
                ("display", "flex"),
                ("flex-direction", "column"),
                ("row-gap", "12px"),
                ("padding-top", "24px"),
                ("padding-left", "24px"),
                ("padding-right", "24px"),
                ("padding-bottom", "24px"),
                ("background-color", "rgb(248, 250, 252)"),
            ],
            "",
            vec![label, input, submit],
        );
        let root = elem(
            None,
            "html",
            BoxRect { x: 0.0, y: 0.0, width: 360.0, height: 172.0 },
            &[("display", "block"), ("background-color", "rgb(248, 250, 252)")],
            "",
            vec![form],
        );
        Capture {
            url: "https://designos.dev/form".into(),
            title: "Form".into(),
            viewport: Viewport { width: 360.0, height: 172.0, dpr: 1.0 },
            scroll: ScrollSize { width: 360.0, height: 172.0 },
            root,
            ..Default::default()
        }
    }

    #[test]
    fn css_order_and_flex_margins_and_scale_transform_reported() {
        // 1. CAPTURE_JS must capture "order" in CAPTURED_STYLE_PROPS.
        assert!(crate::CAPTURE_JS.contains("\"order\""), "CAPTURE_JS must include \"order\" in CAPTURED_STYLE_PROPS");

        // 2. Flex row children reordered visually by border_box.x (even without styles["order"]) and with non-uniform margins + transform: scale()
        let dom_first_visually_second = elem(
            Some("second-visual"),
            "div",
            BoxRect { x: 140.0, y: 0.0, width: 100.0, height: 40.0 },
            &[("display", "block"), ("margin-left", "20px"), ("transform", "scale(1.1)")],
            "",
            vec![],
        );
        let dom_second_visually_first = elem(
            Some("first-visual"),
            "div",
            BoxRect { x: 0.0, y: 0.0, width: 100.0, height: 40.0 },
            &[("display", "block"), ("margin-right", "8px")],
            "",
            vec![],
        );
        let row = elem(
            Some("reordered-row"),
            "div",
            BoxRect { x: 0.0, y: 0.0, width: 260.0, height: 40.0 },
            &[("display", "flex"), ("flex-direction", "row")],
            "",
            vec![dom_first_visually_second, dom_second_visually_first],
        );

        // 3. Flex row with uniform inter-item margins (margin-right: 12px on child 0, 12px on child 1, 0px on child 2) and column-gap: 8px -> item_spacing = 20px, child margins zeroed.
        let u1 = elem(Some("u1"), "div", BoxRect { x: 0.0, y: 50.0, width: 50.0, height: 30.0 }, &[("display", "block"), ("margin-right", "12px")], "", vec![]);
        let u2 =
            elem(Some("u2"), "div", BoxRect { x: 70.0, y: 50.0, width: 50.0, height: 30.0 }, &[("display", "block"), ("margin-right", "12px")], "", vec![]);
        let u3 = elem(Some("u3"), "div", BoxRect { x: 140.0, y: 50.0, width: 50.0, height: 30.0 }, &[("display", "block")], "", vec![]);
        let uniform_row = elem(
            Some("uniform-row"),
            "div",
            BoxRect { x: 0.0, y: 50.0, width: 190.0, height: 30.0 },
            &[("display", "flex"), ("flex-direction", "row"), ("column-gap", "8px")],
            "",
            vec![u1, u2, u3],
        );

        let root = elem(None, "html", BoxRect { x: 0.0, y: 0.0, width: 300.0, height: 100.0 }, &[("display", "block")], "", vec![row, uniform_row]);
        let cap = Capture {
            url: "https://example.com".into(),
            title: "Order & Margin Test".into(),
            viewport: Viewport { width: 300.0, height: 100.0, dpr: 1.0 },
            scroll: ScrollSize { width: 300.0, height: 100.0 },
            root,
            ..Default::default()
        };

        let (doc, report) = import_capture_into_document(&cap, &ImportOptions::default()).unwrap();

        let reordered = doc.nodes.values().find(|n| n.name == "div#reordered-row").unwrap();
        let c0 = doc.get(reordered.children[0]).unwrap();
        let c1 = doc.get(reordered.children[1]).unwrap();
        assert_eq!(c0.name, "div#first-visual", "visually leftmost child must be first");
        assert_eq!(c1.name, "div#second-visual", "visually rightmost child must be second");
        assert_eq!(c0.layout_child.margin, [0.0, 8.0, 0.0, 0.0], "non-uniform margin-right preserved on layout_child");
        assert_eq!(c1.layout_child.margin, [0.0, 0.0, 0.0, 20.0], "non-uniform margin-left preserved on layout_child");

        let u_row = doc.nodes.values().find(|n| n.name == "div#uniform-row").unwrap();
        let u_al = u_row.kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
        assert_eq!(u_al.item_spacing, 20.0, "uniform 12px margin between flex children added to 8px column-gap");
        let uc0 = doc.get(u_row.children[0]).unwrap();
        assert_eq!(uc0.layout_child.margin, [0.0, 0.0, 0.0, 0.0], "folded uniform margin must not double-apply on layout_child");

        assert!(
            report.unsupported_css.iter().any(|u| u.property == "transform" && u.value.contains("scale(1.1)")),
            "transform: scale(1.1) must be recorded in report.unsupported_css, got: {:?}",
            report.unsupported_css
        );
    }
}
