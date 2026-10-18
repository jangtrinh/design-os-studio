//! Bounded vector PDF 1.4 export. Pages are clipped to frame bounds over a white backdrop. Each
//! design pixel maps to 0.75 PDF points (96 CSS pixels per inch).
//!
//! A page is drawn the way the CPU renderer draws the frame: variables resolved per node, shapes and
//! boolean results from the renderer's own outlines, solid, linear and radial fills, strokes of every
//! alignment, clipping frames, and opaque shape masks become PDF paths, shadings and clips. Glyphs are
//! drawn as outlines; an invisible text run over each line keeps the text selectable and searchable
//! (`pdftotext` finds it), including characters outside WinAnsi.
//!
//! What the writer does not model (effects, images, blend modes, faded groups, translucent masks,
//! angular and diamond gradients, gradients that fade, arrow heads, outlined text) is never drawn
//! wrong: the whole frame is then written as the renderer's lossless image, with the same text layer.

use designos_doc::{resolve_node, Document, NodeId, NodeKind};
use std::io::Write;
use text_layer::{TextFonts, FONT_OBJECTS};
use vector_page::{build_page, PageOutput};

const MAX_PAGES: usize = 500;
const MAX_OUTPUT: usize = 128 * 1024 * 1024;
const POINTS_PER_PIXEL: f32 = 0.75;

/// Like [`crate::util::num`] with up to five fractional digits, for matrices.
fn n5(v: f32) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    let s = format!("{v:.5}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".into()
    } else {
        s.to_string()
    }
}

struct Writer {
    bytes: Vec<u8>,
    offsets: Vec<usize>,
}

impl Writer {
    fn new() -> Self {
        Self { bytes: b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec(), offsets: vec![0] }
    }
    fn object(&mut self, number: usize, body: &[u8]) -> Result<(), String> {
        if self.bytes.len().saturating_add(body.len()).saturating_add(64) > MAX_OUTPUT {
            return Err("PDF exceeds the 128 MiB export limit. Export fewer frames at a time.".into());
        }
        if number != self.offsets.len() {
            return Err("Invalid PDF object sequence.".into());
        }
        self.offsets.push(self.bytes.len());
        writeln!(self.bytes, "{number} 0 obj").map_err(|e| e.to_string())?;
        self.bytes.extend_from_slice(body);
        self.bytes.extend_from_slice(b"\nendobj\n");
        Ok(())
    }
    fn stream(&mut self, number: usize, dictionary: &str, content: &[u8]) -> Result<(), String> {
        let dict_prefix = if dictionary.is_empty() { String::new() } else { format!("{dictionary} ") };
        let mut body = format!("<< {dict_prefix}/Length {} >>\nstream\n", content.len()).into_bytes();
        body.extend_from_slice(content);
        body.extend_from_slice(b"\nendstream");
        self.object(number, &body)
    }
    fn finish(mut self) -> Result<Vec<u8>, String> {
        let xref = self.bytes.len();
        write!(self.bytes, "xref\n0 {}\n0000000000 65535 f \n", self.offsets.len()).map_err(|e| e.to_string())?;
        for offset in &self.offsets[1..] {
            writeln!(self.bytes, "{offset:010} 00000 n ").map_err(|e| e.to_string())?;
        }
        write!(self.bytes, "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", self.offsets.len()).map_err(|e| e.to_string())?;
        if self.bytes.len() > MAX_OUTPUT {
            return Err("PDF exceeds the 128 MiB export limit.".into());
        }
        Ok(self.bytes)
    }
}

/// Export one vector PDF page for each supplied frame, preserving the supplied order.
/// Empty lists, non-frame nodes, invalid dimensions, and oversized exports return
/// an error.
pub fn export_pdf(doc: &Document, slides: &[NodeId]) -> Result<Vec<u8>, String> {
    if slides.is_empty() {
        return Err("There are no frames to export.".into());
    }
    if slides.len() > MAX_PAGES {
        return Err(format!("Export at most {MAX_PAGES} frames at a time."));
    }
    let lowered = designos_render::board::lowered_document(doc);
    let doc = lowered.as_ref().unwrap_or(doc);
    // Validate the complete selection before spending time emitting any page. Sizes are the sizes the
    // canvas shows, with variable bindings applied.
    for id in slides {
        let node = doc.get(*id).ok_or_else(|| format!("Frame {} no longer exists.", id.0))?;
        if !matches!(node.kind, NodeKind::Frame(_)) {
            return Err(format!("{} is not a frame; only frames export as PDF pages.", node.name));
        }
        let size = resolve_node(doc, node).size;
        if !size.x.is_finite() || !size.y.is_finite() || size.x < 1.0 || size.y < 1.0 || size.x > 19_200.0 || size.y > 19_200.0 {
            return Err("Frame dimensions must be finite and between 1 and 19,200 pixels.".into());
        }
    }
    let mut fonts = TextFonts::default();
    let mut pages: Vec<PageOutput> = Vec::with_capacity(slides.len());
    let mut size = 0usize;
    for id in slides {
        let page = build_page(doc, *id, &mut fonts)?;
        size = size.saturating_add(page.body.len()).saturating_add(page.image.as_ref().map_or(0, |i| i.data.len()));
        if size > MAX_OUTPUT {
            return Err("PDF exceeds the 128 MiB export limit. Export fewer frames at a time.".into());
        }
        pages.push(page);
    }
    // Objects: catalog, page tree, a page and its content stream per frame, then the flattened page
    // images and the Unicode font for the pages that use them.
    let mut next = 3 + pages.len() * 2;
    let image_objects: Vec<Option<usize>> = pages
        .iter()
        .map(|p| {
            p.image.as_ref().map(|_| {
                next += 1;
                next - 1
            })
        })
        .collect();
    let font_object = fonts.is_used().then(|| {
        next += FONT_OBJECTS;
        next - FONT_OBJECTS
    });
    let mut writer = Writer::new();
    writer.object(1, b"<< /Type /Catalog /Pages 2 0 R >>")?;
    let kids = (0..pages.len()).map(|i| format!("{} 0 R", 3 + i * 2)).collect::<Vec<_>>().join(" ");
    writer.object(2, format!("<< /Type /Pages /Count {} /Kids [{kids}] >>", pages.len()).as_bytes())?;
    for (index, page) in pages.iter().enumerate() {
        let page_obj = 3 + index * 2;
        let contents_obj = page_obj + 1;
        let width = num_pt(page.width);
        let height = num_pt(page.height);
        let resources = page.resources.dict(font_object.filter(|_| page.uses_wide_font), image_objects[index]);
        writer.object(
            page_obj,
            format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] /Resources {resources} /Contents {contents_obj} 0 R >>").as_bytes(),
        )?;
        writer.stream(contents_obj, "", page.body.as_bytes())?;
    }
    for (page, object) in pages.iter().zip(&image_objects) {
        if let (Some(image), Some(object)) = (&page.image, object) {
            writer.stream(
                *object,
                &format!(
                    "/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode",
                    image.width, image.height
                ),
                &image.data,
            )?;
        }
    }
    if let Some(first) = font_object {
        fonts.write_objects(&mut writer, first)?;
    }
    writer.finish()
}

/// A frame dimension in design pixels as PDF points.
fn num_pt(pixels: f32) -> String {
    crate::util::num(pixels * POINTS_PER_PIXEL)
}

mod fidelity {
    //! Decides, before a page is written, whether the vector writer can draw a frame the way the canvas
    //! does. The conditions mirror the CPU renderer (`designos_render`): when a frame uses something the
    //! writer does not model, the page is flattened into an image instead of being drawn wrong.

    use designos_doc::{resolve_node, BlendMode, Document, Node, NodeId, NodeKind, Paint, PaintKind, StrokeAlign};
    use designos_render::{outline_of, vectorops::vertex_caps, NodeShape};
    use tiny_skia::{FillRule as SkRule, Path as SkPath, PathSegment, Point};

    /// Flattening tolerance, in local units, for shapes the renderer builds from other shapes (booleans).
    pub(super) const FLATTEN_TOLERANCE: f32 = 0.05;

    /// Why `root` has to be written as a flattened image, or `None` when it can be written as vector content.
    pub(super) fn raster_reason(doc: &Document, root: NodeId) -> Option<&'static str> {
        let node = resolve_node(doc, doc.get(root)?);
        if !is_drawn(&node) {
            return None;
        }
        subtree_reason(doc, &node)
    }

    /// The renderer draws nothing for hidden nodes, unknown kinds, and fully transparent nodes that do not blend.
    pub(super) fn is_drawn(n: &Node) -> bool {
        if !n.visible || matches!(n.kind, NodeKind::Unknown { .. } | NodeKind::Page { .. }) {
            return false;
        }
        n.opacity.clamp(0.0, 1.0) > 0.0 || layer_blend(n)
    }

    /// A blend mode other than normal makes the renderer composite the node as a layer.
    fn layer_blend(n: &Node) -> bool {
        !paintable_blend(n.blend)
    }

    fn paintable_blend(b: BlendMode) -> bool {
        matches!(b, BlendMode::Normal | BlendMode::PassThrough)
    }

    /// Whether the node paints its children (boolean operations only use them as operands).
    pub(super) fn draws_children(n: &Node) -> bool {
        !n.children.is_empty() && !matches!(n.kind, NodeKind::BooleanOp(_))
    }

    /// The fills the renderer paints: a text node's base style fills, the node's own fills otherwise.
    pub(super) fn visible_fills(n: &Node) -> Vec<&Paint> {
        match &n.kind {
            NodeKind::Text(t) => t.style.fills.iter().filter(|p| p.visible).collect(),
            _ => n.fills.iter().filter(|p| p.visible).collect(),
        }
    }

    /// Stroke paints, as the renderer decides them: a stroke exists with a weight or with per-side weights.
    pub(super) fn visible_strokes(n: &Node) -> Vec<&Paint> {
        if n.stroke.weight <= 0.0 && n.stroke.side_weights.is_none() {
            return Vec::new();
        }
        n.stroke.paints.iter().filter(|p| p.visible).collect()
    }

    fn subtree_reason(doc: &Document, n: &Node) -> Option<&'static str> {
        if let Some(reason) = node_reason(doc, n) {
            return Some(reason);
        }
        if !draws_children(n) {
            return None;
        }
        for cid in &n.children {
            let Some(raw) = doc.get(*cid) else { continue };
            let child = resolve_node(doc, raw);
            if child.is_mask {
                if child.visible {
                    if let Some(reason) = mask_reason(doc, &child) {
                        return Some(reason);
                    }
                }
                continue;
            }
            if is_drawn(&child) {
                if let Some(reason) = subtree_reason(doc, &child) {
                    return Some(reason);
                }
            }
        }
        None
    }

    fn node_reason(doc: &Document, n: &Node) -> Option<&'static str> {
        if layer_blend(n) {
            return Some("a layer blend mode");
        }
        if n.effects.iter().any(|e| e.visible()) {
            return Some("a visible effect");
        }
        let fills = visible_fills(n);
        let strokes = visible_strokes(n);
        if n.opacity.clamp(0.0, 1.0) < 1.0 && (draws_children(n) || fills.len() + strokes.len() > 1) {
            // a faded node with several parts is composited as one layer, then faded
            return Some("a faded group");
        }
        if let Some(reason) = fills.iter().chain(&strokes).find_map(|p| paint_reason(p)) {
            return Some(reason);
        }
        if let NodeKind::Text(t) = &n.kind {
            if let Some(reason) = t.runs.iter().flat_map(|r| &r.style.fills).filter(|p| p.visible).find_map(paint_reason) {
                return Some(reason);
            }
            if !strokes.is_empty() {
                return Some("an outlined text node");
            }
        }
        if vertex_caps(n).is_some() {
            return Some("arrow heads on a vector network");
        }
        clip_reason(doc, n, !strokes.is_empty())
    }

    /// Clips and inside/outside strokes are drawn from one outline; shapes made of several paths are not.
    fn clip_reason(doc: &Document, n: &Node, has_strokes: bool) -> Option<&'static str> {
        let clips = draws_children(n) && n.kind.frame_props().is_some_and(|f| f.clips_content);
        let aligned = has_strokes && n.stroke.align != StrokeAlign::Center;
        if !clips && !aligned {
            return None;
        }
        let shape = outline_of(doc, n, FLATTEN_TOLERANCE)?;
        if clips && shape.paths.len() != 1 {
            return Some("a clipping frame made of several paths");
        }
        if aligned && shape.stroke_paths.is_none() && shape.closed {
            if shape.fill_paths().len() != 1 {
                return Some("an aligned stroke around several paths");
            }
            if n.stroke.align == StrokeAlign::Outside && !outside_clip_is_exact(n, &shape) {
                return Some("an outside stroke around an outline that may cross itself");
            }
        }
        None
    }

    /// An outside stroke is kept where the shape is not. The page writes that as an even-odd clip of a
    /// page-sized rectangle plus the outline, which is the complement of the fill only where the even-odd
    /// and non-zero rules agree: even-odd outlines, the renderer's own primitives, and single contours
    /// that never cross themselves. A pentagram, for one, covers its centre twice and is filled by the
    /// non-zero rule but not by the even-odd rule.
    fn outside_clip_is_exact(n: &Node, shape: &NodeShape) -> bool {
        let [(path, rule)] = shape.fill_paths() else { return false };
        if *rule == SkRule::EvenOdd {
            return true;
        }
        let primitive = n.fill_geometry.is_none()
            && matches!(
                n.kind,
                NodeKind::Frame(_)
                    | NodeKind::Component(_)
                    | NodeKind::ComponentSet(_)
                    | NodeKind::Instance { .. }
                    | NodeKind::Section
                    | NodeKind::Rectangle
                    | NodeKind::Ellipse(_)
                    | NodeKind::Polygon { .. }
                    | NodeKind::Star { .. }
            );
        primitive || is_simple_contour(path)
    }

    /// One contour whose flattened outline has no two edges crossing. Anything longer than a few thousand
    /// edges, or made of several contours, is not examined and counts as not simple.
    fn is_simple_contour(path: &SkPath) -> bool {
        const PIECES: usize = 8;
        const MAX_EDGES: usize = 2000;
        let mut points: Vec<Point> = Vec::new();
        let mut contours = 0;
        let mut last = Point::zero();
        for segment in path.segments() {
            match segment {
                PathSegment::MoveTo(p) => {
                    contours += 1;
                    points.push(p);
                    last = p;
                }
                PathSegment::LineTo(p) => {
                    points.push(p);
                    last = p;
                }
                PathSegment::QuadTo(c, p) => {
                    for k in 1..=PIECES {
                        let (t, u) = (k as f32 / PIECES as f32, 1.0 - k as f32 / PIECES as f32);
                        points.push(Point::from_xy(u * u * last.x + 2.0 * u * t * c.x + t * t * p.x, u * u * last.y + 2.0 * u * t * c.y + t * t * p.y));
                    }
                    last = p;
                }
                PathSegment::CubicTo(c1, c2, p) => {
                    for k in 1..=PIECES {
                        let (t, u) = (k as f32 / PIECES as f32, 1.0 - k as f32 / PIECES as f32);
                        let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                        points.push(Point::from_xy(a * last.x + b * c1.x + c * c2.x + d * p.x, a * last.y + b * c1.y + c * c2.y + d * p.y));
                    }
                    last = p;
                }
                PathSegment::Close => {}
            }
        }
        if contours != 1 || points.len() > MAX_EDGES {
            return false;
        }
        points.dedup();
        if points.len() > 1 && points.first() == points.last() {
            points.pop();
        }
        let n = points.len();
        let side = |p: Point, q: Point, r: Point| (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x);
        for i in 0..n {
            let (a, b) = (points[i], points[(i + 1) % n]);
            for j in i + 2..n {
                if i == 0 && j == n - 1 {
                    continue;
                }
                let (c, d) = (points[j], points[(j + 1) % n]);
                if side(a, b, c) * side(a, b, d) < 0.0 && side(c, d, a) * side(c, d, b) < 0.0 {
                    return false;
                }
            }
        }
        true
    }

    fn paint_reason(p: &Paint) -> Option<&'static str> {
        if !paintable_blend(p.blend) {
            return Some("a paint blend mode");
        }
        match &p.kind {
            PaintKind::Solid(_) => None,
            PaintKind::Image { .. } => Some("an image paint"),
            PaintKind::AngularGradient { .. } | PaintKind::DiamondGradient { .. } => Some("an angular or diamond gradient"),
            PaintKind::LinearGradient { stops, .. } | PaintKind::RadialGradient { stops, .. } => {
                let first = stops.first().map_or(1.0, |s| s.color.a);
                stops.iter().any(|s| (s.color.a - first).abs() > 1e-4).then_some("a gradient whose stops differ in opacity")
            }
        }
    }

    /// A mask is a plain clip when it is one opaque shape; anything else lets part of the following
    /// siblings through at partial strength, which a clip cannot express.
    fn mask_reason(doc: &Document, m: &Node) -> Option<&'static str> {
        const NOT_A_CLIP: &str = "a mask that is not one opaque shape";
        if matches!(m.kind, NodeKind::Text(_) | NodeKind::Group) || draws_children(m) || m.opacity < 1.0 {
            return Some(NOT_A_CLIP);
        }
        let Some(shape) = outline_of(doc, m, FLATTEN_TOLERANCE) else { return Some(NOT_A_CLIP) };
        if !shape.fillable || shape.fill_paths().len() != 1 {
            return Some(NOT_A_CLIP);
        }
        let opaque = |p: &&Paint| matches!(&p.kind, PaintKind::Solid(c) if c.a * p.opacity >= 0.999);
        let fills = visible_fills(m);
        (!fills.iter().all(opaque)).then_some(NOT_A_CLIP)
    }
}

mod raster_page {
    //! A frame the vector writer cannot draw faithfully, flattened into a lossless image the way the
    //! first PDF exporter wrote every page. Each design pixel maps to 0.75 PDF points; the image is
    //! rendered at up to twice that resolution and transparency is composited over white.

    use designos_doc::{Document, NodeId};
    use flate2::{write::ZlibEncoder, Compression};
    use std::io::Write;

    const MAX_SIDE: f32 = 4096.0;

    /// Deflated 8-bit RGB pixels.
    pub(super) struct PageImage {
        pub width: u32,
        pub height: u32,
        pub data: Vec<u8>,
    }

    /// Renders frame `id` (variables resolved, board items lowered) at up to 2x and deflates it.
    pub(super) fn flatten(doc: &Document, id: NodeId, width: f32, height: f32) -> Result<PageImage, String> {
        let scale = 2.0_f32.min(MAX_SIDE / width).min(MAX_SIDE / height);
        let pixmap = designos_render::render_node(doc, id, scale).ok_or("Cannot render this slide for PDF export.")?;
        // tiny-skia stores premultiplied RGBA. Adding the missing white alpha
        // gives opaque RGB directly, without a lossy unpremultiply round-trip.
        let mut compressor = ZlibEncoder::new(Vec::new(), Compression::default());
        let mut row = Vec::with_capacity(pixmap.width() as usize * 3);
        for pixels in pixmap.data().chunks_exact(pixmap.width() as usize * 4) {
            row.clear();
            for pixel in pixels.chunks_exact(4) {
                let white = 255 - pixel[3];
                row.extend_from_slice(&[pixel[0].saturating_add(white), pixel[1].saturating_add(white), pixel[2].saturating_add(white)]);
            }
            compressor.write_all(&row).map_err(|e| format!("Cannot compress PDF image: {e}"))?;
        }
        let data = compressor.finish().map_err(|e| format!("Cannot finish PDF image: {e}"))?;
        Ok(PageImage { width: pixmap.width(), height: pixmap.height(), data })
    }
}

mod shading {
    //! Linear and radial gradients as PDF shadings, built the way the renderer builds them: colour stops
    //! live in a unit space (axis `(0,0)..(1,0)` for linear, circle of radius 1 at the origin for radial),
    //! and the three gradient handles map that space onto the node. The writer draws the shading under that
    //! matrix, so skewed and elliptical gradients come out as on the canvas.

    use super::n5;
    use crate::util::num;
    use designos_doc::{Color, GradientStop, Vec2};

    /// Stop positions are written with five decimals; two positions that print alike are one position.
    const POSITION_STEP: f32 = 1e-5;

    pub(super) enum Plan {
        /// One colour stop: the renderer paints it as a solid colour.
        Flat(Color),
        Shaded(Shaded),
    }

    pub(super) struct Shaded {
        /// Shading dictionary in the unit space.
        pub dict: String,
        /// `cm` operands that map the unit space onto the node.
        pub matrix: String,
        /// Opacity shared by all stops (stops of different opacity are flattened by the page checks).
        pub alpha: f32,
    }

    /// Shading for a gradient paint on a node of `size`, or `None` when the renderer paints nothing
    /// (no stops, or handles that make the gradient space singular).
    pub(super) fn plan(radial: bool, stops: &[GradientStop], handles: &[Vec2; 3], size: Vec2) -> Option<Plan> {
        let mut sorted: Vec<(f32, Color)> = stops.iter().map(|s| (if s.pos.is_nan() { 0.0 } else { s.pos.clamp(0.0, 1.0) }, s.color)).collect();
        sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
        match sorted.as_slice() {
            [] => return None,
            [(_, color)] => return Some(Plan::Flat(*color)),
            _ => {}
        }
        let [h0, h1, h2] = *handles;
        let m = [size.x * (h1.x - h0.x), size.y * (h1.y - h0.y), size.x * (h2.x - h0.x), size.y * (h2.y - h0.y), size.x * h0.x, size.y * h0.y];
        let det = m[0] * m[3] - m[1] * m[2];
        if !m.iter().all(|v| v.is_finite()) || det.abs() < 1e-9 {
            return None;
        }
        let function = function_dict(&sorted);
        let coords = if radial { "/ShadingType 3 /Coords [0 0 0 0 0 1]" } else { "/ShadingType 2 /Coords [0 0 1 0]" };
        let dict = format!("<< {coords} /ColorSpace /DeviceRGB /Extend [true true] /Function {function} >>");
        let matrix = m.map(n5).join(" ");
        Some(Plan::Shaded(Shaded { dict, matrix, alpha: sorted[0].1.a }))
    }

    fn linear_segment(a: Color, b: Color) -> String {
        format!("<< /FunctionType 2 /Domain [0 1] /C0 [{} {} {}] /C1 [{} {} {}] /N 1 >>", num(a.r), num(a.g), num(a.b), num(b.r), num(b.g), num(b.b))
    }

    /// Colour as a function of the position along the gradient, over `[0 1]`: flat before the first stop and
    /// after the last, linear between stops, a jump where two stops share a position.
    fn function_dict(stops: &[(f32, Color)]) -> String {
        // PDF wants `/Bounds` that increase strictly inside (0, 1) as printed, so work on the printed positions
        let stops: Vec<(f32, Color)> = stops.iter().map(|(pos, color)| ((pos / POSITION_STEP).round() * POSITION_STEP, *color)).collect();
        let (first, last) = (stops[0], stops[stops.len() - 1]);
        let wider = |from: f32, to: f32| to - from > POSITION_STEP / 2.0;
        // (start, end, colour at start, colour at end)
        let mut segments: Vec<(f32, f32, Color, Color)> = Vec::new();
        if wider(0.0, first.0) {
            segments.push((0.0, first.0, first.1, first.1));
        }
        for pair in stops.windows(2) {
            if wider(pair[0].0, pair[1].0) {
                segments.push((pair[0].0, pair[1].0, pair[0].1, pair[1].1));
            }
        }
        if wider(last.0, 1.0) {
            segments.push((last.0, 1.0, last.1, last.1));
        }
        if let [(_, _, a, b)] = segments.as_slice() {
            return linear_segment(*a, *b);
        }
        let functions = segments.iter().map(|(_, _, a, b)| linear_segment(*a, *b)).collect::<Vec<_>>().join(" ");
        let bounds = segments.iter().skip(1).map(|(start, ..)| n5(*start)).collect::<Vec<_>>().join(" ");
        let encode = vec!["0 1"; segments.len()].join(" ");
        format!("<< /FunctionType 3 /Domain [0 1] /Functions [{functions}] /Bounds [{bounds}] /Encode [{encode}] >>")
    }
}

mod text_layer {
    //! The invisible text layer that makes exported text selectable and searchable. The glyphs a viewer
    //! shows are the outlines drawn as paths, so the layer only has to carry the right characters:
    //! text that fits WinAnsi goes into a standard Helvetica run; anything else (Vietnamese, CJK, the euro
    //! sign, ...) goes into a two-byte font whose ToUnicode map names the real characters, so copying and
    //! searching give back what the canvas shows.

    use super::Writer;
    use std::collections::HashMap;
    use std::fmt::Write as _;

    /// How one run of text is written into a content stream.
    pub(super) enum Encoded {
        /// Escaped literal string for the WinAnsi font `/F1`.
        Latin(String),
        /// Hex string of two-byte character ids for the Unicode font `/F2`.
        Wide(String),
    }

    /// Characters of the document that need the Unicode font, numbered in order of first use.
    #[derive(Default)]
    pub(super) struct TextFonts {
        chars: Vec<char>,
        ids: HashMap<char, u16>,
    }

    /// Number of objects [`TextFonts::write_objects`] writes.
    pub(super) const FONT_OBJECTS: usize = 4;

    /// WinAnsi code of `c`, if the encoding has it.
    fn win_ansi_byte(c: char) -> Option<u8> {
        let u = c as u32;
        match u {
            0x20..=0x7E | 0xA0..=0xFF => Some(u as u8),
            _ => {
                const HIGH: [(char, u8); 27] = [
                    ('\u{20AC}', 0x80),
                    ('\u{201A}', 0x82),
                    ('\u{0192}', 0x83),
                    ('\u{201E}', 0x84),
                    ('\u{2026}', 0x85),
                    ('\u{2020}', 0x86),
                    ('\u{2021}', 0x87),
                    ('\u{02C6}', 0x88),
                    ('\u{2030}', 0x89),
                    ('\u{0160}', 0x8A),
                    ('\u{2039}', 0x8B),
                    ('\u{0152}', 0x8C),
                    ('\u{017D}', 0x8E),
                    ('\u{2018}', 0x91),
                    ('\u{2019}', 0x92),
                    ('\u{201C}', 0x93),
                    ('\u{201D}', 0x94),
                    ('\u{2022}', 0x95),
                    ('\u{2013}', 0x96),
                    ('\u{2014}', 0x97),
                    ('\u{02DC}', 0x98),
                    ('\u{2122}', 0x99),
                    ('\u{0161}', 0x9A),
                    ('\u{203A}', 0x9B),
                    ('\u{0153}', 0x9C),
                    ('\u{017E}', 0x9E),
                    ('\u{0178}', 0x9F),
                ];
                HIGH.iter().find(|(ch, _)| *ch == c).map(|(_, b)| *b)
            }
        }
    }

    impl TextFonts {
        pub(super) fn is_used(&self) -> bool {
            !self.chars.is_empty()
        }

        /// The characters of `text` as the page writes them, `None` when nothing remains to show.
        /// Whitespace controls become spaces; other control characters are dropped.
        pub(super) fn encode(&mut self, text: &str) -> Option<Encoded> {
            let chars: Vec<char> = text
                .chars()
                .filter_map(|c| match c {
                    '\t' | '\n' | '\r' => Some(' '),
                    c if c.is_control() => None,
                    c => Some(c),
                })
                .collect();
            if chars.is_empty() {
                return None;
            }
            if let Some(bytes) = chars.iter().map(|c| win_ansi_byte(*c)).collect::<Option<Vec<u8>>>() {
                let mut out = String::with_capacity(bytes.len());
                for b in bytes {
                    match b {
                        b'\\' | b'(' | b')' => {
                            out.push('\\');
                            out.push(char::from(b));
                        }
                        0x20..=0x7E => out.push(char::from(b)),
                        _ => {
                            let _ = write!(out, "\\{b:03o}");
                        }
                    }
                }
                return Some(Encoded::Latin(out));
            }
            let mut hex = String::with_capacity(chars.len() * 4);
            for c in chars {
                if let Some(id) = self.id_of(c) {
                    let _ = write!(hex, "{id:04X}");
                }
            }
            (!hex.is_empty()).then_some(Encoded::Wide(hex))
        }

        /// Character id of `c`; `None` once the 65,535 ids of a two-byte font are used up.
        fn id_of(&mut self, c: char) -> Option<u16> {
            if let Some(id) = self.ids.get(&c) {
                return Some(*id);
            }
            let id = u16::try_from(self.chars.len() + 1).ok()?;
            self.chars.push(c);
            self.ids.insert(c, id);
            Some(id)
        }

        /// Writes the Unicode font as [`FONT_OBJECTS`] objects starting at `first`: the composite font, its
        /// descendant, the descriptor, and the ToUnicode map. No glyph program is embedded; the layer is
        /// never drawn.
        pub(super) fn write_objects(&self, writer: &mut Writer, first: usize) -> Result<(), String> {
            writer.object(
                first,
                format!(
                    "<< /Type /Font /Subtype /Type0 /BaseFont /Helvetica /Encoding /Identity-H /DescendantFonts [{} 0 R] /ToUnicode {} 0 R >>",
                    first + 1,
                    first + 3
                )
                .as_bytes(),
            )?;
            writer.object(
            first + 1,
            format!(
                "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /Helvetica /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor {} 0 R /DW 600 /CIDToGIDMap /Identity >>",
                first + 2
            )
            .as_bytes(),
        )?;
            writer.object(
            first + 2,
            b"<< /Type /FontDescriptor /FontName /Helvetica /Flags 4 /FontBBox [-166 -225 1000 931] /ItalicAngle 0 /Ascent 718 /Descent -207 /CapHeight 718 /StemV 88 >>",
        )?;
            writer.stream(first + 3, "", self.to_unicode_map().as_bytes())
        }

        fn to_unicode_map(&self) -> String {
            let mut cmap = String::from(
            "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
        );
            for (block, chars) in self.chars.chunks(100).enumerate() {
                let _ = writeln!(cmap, "{} beginbfchar", chars.len());
                for (i, c) in chars.iter().enumerate() {
                    let id = block * 100 + i + 1;
                    let mut units = [0u16; 2];
                    let utf16: String = c.encode_utf16(&mut units).iter().map(|u| format!("{u:04X}")).collect();
                    let _ = writeln!(cmap, "<{id:04X}> <{utf16}>");
                }
                cmap.push_str("endbfchar\n");
            }
            cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
            cmap
        }
    }
}

mod vector_page {
    //! Draws one frame as PDF page content, in the order and with the geometry the CPU renderer uses:
    //! variables resolved per node, shapes and strokes from the renderer's own outlines, fills, strokes,
    //! then children (clipped for clipping frames, narrowed by masks). Frames the writer cannot draw
    //! faithfully (see `fidelity`) become a flattened image with the same invisible text layer on top.

    use super::fidelity::{draws_children, is_drawn, raster_reason, visible_fills, visible_strokes, FLATTEN_TOLERANCE};
    use super::raster_page::{self, PageImage};
    use super::shading::{plan, Plan};
    use super::text_layer::{Encoded, TextFonts};
    use super::{n5, POINTS_PER_PIXEL};
    use crate::geo::transform_cmds;
    use crate::text::{self, TextOut};
    use crate::util::num;
    use designos_doc::{
        resolve_node, Affine, Color, Document, FillRule, GradientStop, Node, NodeId, NodeKind, Paint, PaintKind, PathCmd, StrokeAlign, StrokeCap, StrokeJoin,
        TextProps, Vec2,
    };
    use designos_render::{outline_of, stroke_plan, NodeShape, StrokeClip};
    use std::fmt::Write as _;
    use tiny_skia::{FillRule as SkRule, Path as SkPath, PathSegment, Point};

    /// Device scale handed to the stroke expander for strokes that are written as filled outlines; PDF
    /// pages are zoomed far past 1:1, so curves are approximated finely.
    const OUTLINE_RES_SCALE: f32 = 8.0;

    /// Everything the page dictionary needs besides the content stream.
    #[derive(Default)]
    pub(super) struct PageResources {
        gstates: Vec<f32>,
        shadings: Vec<String>,
    }

    impl PageResources {
        /// Resource dictionary; `wide_font` and `image` are the object numbers of the Unicode font and of
        /// the flattened page image when the page uses them.
        pub(super) fn dict(&self, wide_font: Option<usize>, image: Option<usize>) -> String {
            let mut fonts = String::from("/F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>");
            if let Some(obj) = wide_font {
                let _ = write!(fonts, " /F2 {obj} 0 R");
            }
            let mut res = format!("<< /Font << {fonts} >>");
            if !self.gstates.is_empty() {
                res.push_str(" /ExtGState <<");
                for (i, a) in self.gstates.iter().enumerate() {
                    let _ = write!(res, " /GS{i} << /Type /ExtGState /ca {} /CA {} >>", num(*a), num(*a));
                }
                res.push_str(" >>");
            }
            if !self.shadings.is_empty() {
                res.push_str(" /Shading <<");
                for (i, sh) in self.shadings.iter().enumerate() {
                    let _ = write!(res, " /Sh{i} {sh}");
                }
                res.push_str(" >>");
            }
            if let Some(obj) = image {
                let _ = write!(res, " /XObject << /Slide {obj} 0 R >>");
            }
            res.push_str(" >>");
            res
        }
    }

    pub(super) struct PageOutput {
        /// Frame size in design pixels.
        pub width: f32,
        pub height: f32,
        pub body: String,
        pub resources: PageResources,
        /// The flattened page, when the frame could not be drawn as vector content.
        pub image: Option<PageImage>,
        pub uses_wide_font: bool,
    }

    /// Builds the content of the validated frame `id`.
    pub(super) fn build_page(doc: &Document, id: NodeId, fonts: &mut TextFonts) -> Result<PageOutput, String> {
        let raw = doc.get(id).ok_or_else(|| format!("Frame {} no longer exists.", id.0))?;
        let root = resolve_node(doc, raw);
        let (width, height) = (root.size.x, root.size.y);
        let mut builder = PageBuilder::new(doc, fonts, Vec2::new(width, height));
        let image = match raster_reason(doc, id) {
            Some(reason) => {
                builder.flattened_page(&root, reason);
                Some(raster_page::flatten(doc, id, width, height)?)
            }
            None => {
                builder.vector_page(&root);
                None
            }
        };
        Ok(PageOutput { width, height, body: builder.body, resources: builder.resources, image, uses_wide_font: builder.uses_wide_font })
    }

    struct PageBuilder<'a, 'f> {
        doc: &'a Document,
        fonts: &'f mut TextFonts,
        /// Frame size in design pixels.
        page: Vec2,
        body: String,
        resources: PageResources,
        uses_wide_font: bool,
        /// Transform from the local space of the node being drawn to the page (design pixels).
        ctm: Affine,
        /// The part of the page that the frame bounds, clipping frames and masks above the node being
        /// drawn leave visible; text outside it is not worth selecting.
        visible: Bounds,
    }

    impl<'a, 'f> PageBuilder<'a, 'f> {
        fn new(doc: &'a Document, fonts: &'f mut TextFonts, page: Vec2) -> Self {
            Self {
                doc,
                fonts,
                page,
                body: String::new(),
                resources: PageResources::default(),
                uses_wide_font: false,
                ctm: Affine::IDENTITY,
                visible: [0.0, 0.0, page.x, page.y],
            }
        }

        fn page_points(&self) -> (String, String) {
            (num(self.page.x * POINTS_PER_PIXEL), num(self.page.y * POINTS_PER_PIXEL))
        }

        /// Switches the content stream to design pixels, y down, at the frame's top-left.
        fn enter_design_space(&mut self) {
            let (_, height_pt) = self.page_points();
            let _ = writeln!(self.body, "q\n{} 0 0 -{} 0 {height_pt} cm", num(POINTS_PER_PIXEL), num(POINTS_PER_PIXEL));
        }

        /// White backdrop, then the frame's content clipped to its bounds. The frame's own transform is not
        /// applied, as in the renderer's single-node export.
        fn vector_page(&mut self, root: &Node) {
            let (width_pt, height_pt) = self.page_points();
            let _ = writeln!(self.body, "1 1 1 rg 0 0 {width_pt} {height_pt} re f");
            self.enter_design_space();
            let _ = writeln!(self.body, "0 0 {} {} re W n", num(self.page.x), num(self.page.y));
            if is_drawn(root) {
                self.contents(root);
            }
            self.body.push_str("Q\n");
        }

        /// The flattened image fills the page; only the invisible text layer is written on top. The comment
        /// says why the frame is not vector content.
        fn flattened_page(&mut self, root: &Node, reason: &str) {
            let (width_pt, height_pt) = self.page_points();
            let _ = writeln!(self.body, "% flattened to an image: {reason}\nq\n{width_pt} 0 0 {height_pt} 0 0 cm\n/Slide Do\nQ");
            self.enter_design_space();
            if is_drawn(root) {
                self.text_layer(root);
            }
            self.body.push_str("Q\n");
        }

        // ----- tree ----------------------------------------------------------------------------------

        fn node(&mut self, n: &Node) {
            if !is_drawn(n) {
                return;
            }
            let saved = self.ctm;
            self.ctm = self.ctm * n.transform;
            let t = n.transform;
            let _ = writeln!(self.body, "q\n{} cm", [t.a, t.b, t.c, t.d, t.tx, t.ty].map(n5).join(" "));
            self.contents(n);
            self.body.push_str("Q\n");
            self.ctm = saved;
        }

        /// Fills, strokes, then children, with the node's opacity applied to each paint. Nodes whose opacity
        /// must apply to the whole subtree were sent to the flattened path before getting here.
        fn contents(&mut self, n: &Node) {
            let alpha = n.opacity.clamp(0.0, 1.0);
            if let NodeKind::Text(tp) = &n.kind {
                self.text(n, tp, alpha);
                return;
            }
            let shape = outline_of(self.doc, n, FLATTEN_TOLERANCE);
            if let Some(sh) = &shape {
                if sh.fillable {
                    let paths = path_cmds(sh.fill_paths());
                    for p in visible_fills(n) {
                        self.fill(&paths, p, n.size, alpha);
                    }
                }
                self.strokes(n, sh, alpha);
            }
            if draws_children(n) {
                self.children(n, shape.as_ref());
            }
        }

        fn children(&mut self, n: &Node, shape: Option<&NodeShape>) {
            let clips = n.kind.frame_props().is_some_and(|f| f.clips_content);
            let visible = self.visible;
            if clips {
                // a clipping node without a shape draws none of its children
                let Some(first) = shape.and_then(|sh| sh.paths.first()) else { return };
                let (cmds, rule) = path_cmds(std::slice::from_ref(first)).remove(0);
                self.narrow_to(&Affine::IDENTITY, &first.0);
                self.body.push_str("q\n");
                self.clip(&cmds, rule);
            }
            let doc = self.doc;
            let mut open_masks = 0;
            for cid in n.paint_children().iter() {
                let Some(raw) = doc.get(*cid) else { continue };
                let child = resolve_node(doc, raw);
                if child.is_mask {
                    // a mask narrows the siblings after it; the page checks keep only opaque one-shape masks
                    if !child.visible {
                        continue;
                    }
                    if let Some(sh) = outline_of(doc, &child, FLATTEN_TOLERANCE) {
                        if let Some(first) = sh.fill_paths().first() {
                            let (cmds, rule) = path_cmds(std::slice::from_ref(first)).remove(0);
                            self.narrow_to(&child.transform, &first.0);
                            self.body.push_str("q\n");
                            self.clip(&transform_cmds(&cmds, &child.transform), rule);
                            open_masks += 1;
                        }
                    }
                    continue;
                }
                self.node(&child);
            }
            for _ in 0..open_masks {
                self.body.push_str("Q\n");
            }
            if clips {
                self.body.push_str("Q\n");
            }
            self.visible = visible;
        }

        /// Narrows the visible part of the page to the box of `path`, drawn at `local` inside the current node.
        fn narrow_to(&mut self, local: &Affine, path: &SkPath) {
            let b = path.bounds();
            self.visible = narrow(self.visible, in_page(&(self.ctm * *local), [b.left(), b.top(), b.right(), b.bottom()]));
        }

        // ----- clips ---------------------------------------------------------------------------------

        /// Intersects the clip with the path, in the current space.
        fn clip(&mut self, cmds: &[PathCmd], rule: FillRule) {
            emit_path_cmds(&mut self.body, cmds);
            self.body.push_str(if rule == FillRule::EvenOdd { "W* n\n" } else { "W n\n" });
        }

        /// Clips to the inside of the node's single fill path. False when there is none to clip to.
        fn clip_inside(&mut self, sh: &NodeShape) -> bool {
            let [path] = sh.fill_paths() else { return false };
            let (cmds, rule) = path_cmds(std::slice::from_ref(path)).remove(0);
            self.clip(&cmds, rule);
            true
        }

        /// Clips to everything outside the node's single fill path (an even-odd clip of a rectangle that
        /// covers the page plus the path). That is the complement of the fill only where the outline cannot
        /// overlap itself under the non-zero rule, which the page checks make sure of. False when there is
        /// no such path or the space is singular.
        fn clip_outside(&mut self, sh: &NodeShape) -> bool {
            let [path] = sh.fill_paths() else { return false };
            let Some(inv) = self.ctm.invert() else { return false };
            let corners =
                [Vec2::new(0.0, 0.0), Vec2::new(self.page.x, 0.0), Vec2::new(self.page.x, self.page.y), Vec2::new(0.0, self.page.y)].map(|c| inv.apply(c));
            let min = corners.iter().fold(Vec2::new(f32::MAX, f32::MAX), |m, c| Vec2::new(m.x.min(c.x), m.y.min(c.y)));
            let max = corners.iter().fold(Vec2::new(f32::MIN, f32::MIN), |m, c| Vec2::new(m.x.max(c.x), m.y.max(c.y)));
            let _ = writeln!(self.body, "{} {} {} {} re", num(min.x - 1.0), num(min.y - 1.0), num(max.x - min.x + 2.0), num(max.y - min.y + 2.0));
            let (cmds, _) = path_cmds(std::slice::from_ref(path)).remove(0);
            emit_path_cmds(&mut self.body, &cmds);
            self.body.push_str("W* n\n");
            true
        }

        // ----- paints --------------------------------------------------------------------------------

        fn set_alpha(&mut self, alpha: f32) {
            let a = (alpha.clamp(0.0, 1.0) * 1000.0).round() / 1000.0;
            let idx = match self.resources.gstates.iter().position(|&x| (x - a).abs() < 1e-4) {
                Some(idx) => idx,
                None => {
                    self.resources.gstates.push(a);
                    self.resources.gstates.len() - 1
                }
            };
            let _ = writeln!(self.body, "/GS{idx} gs");
        }

        fn add_shading(&mut self, dict: String) -> usize {
            if let Some(idx) = self.resources.shadings.iter().position(|s| *s == dict) {
                return idx;
            }
            self.resources.shadings.push(dict);
            self.resources.shadings.len() - 1
        }

        /// Paints every path with `paint`. Images, angular and diamond gradients, and gradients whose stops
        /// differ in opacity never get here: those frames are flattened.
        fn fill(&mut self, paths: &[(Vec<PathCmd>, FillRule)], paint: &Paint, size: Vec2, alpha: f32) {
            if !paint.visible || paint.opacity <= 0.0 {
                return;
            }
            let a = (alpha * paint.opacity).clamp(0.0, 1.0);
            match &paint.kind {
                PaintKind::Solid(c) => self.flat(paths, *c, a),
                PaintKind::LinearGradient { stops, handles } => self.gradient(false, paths, stops, handles, size, a),
                PaintKind::RadialGradient { stops, handles } => self.gradient(true, paths, stops, handles, size, a),
                PaintKind::AngularGradient { .. } | PaintKind::DiamondGradient { .. } | PaintKind::Image { .. } => {}
            }
        }

        fn flat(&mut self, paths: &[(Vec<PathCmd>, FillRule)], c: Color, a: f32) {
            if paths.is_empty() || a * c.a <= 0.0 {
                return;
            }
            self.set_alpha(a * c.a);
            let _ = writeln!(self.body, "{} {} {} rg", num(c.r), num(c.g), num(c.b));
            for (cmds, rule) in paths {
                emit_path_cmds(&mut self.body, cmds);
                self.body.push_str(if *rule == FillRule::EvenOdd { "f*\n" } else { "f\n" });
            }
        }

        fn gradient(&mut self, radial: bool, paths: &[(Vec<PathCmd>, FillRule)], stops: &[GradientStop], handles: &[Vec2; 3], size: Vec2, a: f32) {
            match plan(radial, stops, handles, size) {
                None => {}
                Some(Plan::Flat(c)) => self.flat(paths, c, a),
                Some(Plan::Shaded(g)) => {
                    let idx = self.add_shading(g.dict);
                    for (cmds, rule) in paths {
                        self.body.push_str("q\n");
                        self.set_alpha(a * g.alpha);
                        self.clip(cmds, *rule);
                        let _ = writeln!(self.body, "{} cm\n/Sh{idx} sh\nQ", g.matrix);
                    }
                }
            }
        }

        // ----- strokes -------------------------------------------------------------------------------

        fn strokes(&mut self, n: &Node, sh: &NodeShape, alpha: f32) {
            let paints = visible_strokes(n);
            if paints.is_empty() {
                return;
            }
            // precomputed outlines and per-side weights are areas to fill, whatever the paint
            let area = sh.stroke_paths.is_some() || (n.stroke.side_weights.is_some() && sh.is_rect);
            if !area && n.stroke.weight <= 0.0 {
                return;
            }
            for paint in paints {
                match &paint.kind {
                    PaintKind::Solid(c) if !area => self.stroke_line(n, sh, paint, *c, alpha),
                    _ => self.stroke_area(n, sh, paint, alpha),
                }
            }
        }

        /// Alignment as the renderer applies it: only closed shapes can be stroked inside or outside.
        fn alignment(n: &Node, sh: &NodeShape) -> StrokeAlign {
            if sh.closed {
                n.stroke.align
            } else {
                StrokeAlign::Center
            }
        }

        /// A solid stroke as a PDF stroke. Inside and outside strokes are twice as wide, clipped to one side.
        fn stroke_line(&mut self, n: &Node, sh: &NodeShape, paint: &Paint, c: Color, alpha: f32) {
            let s = &n.stroke;
            let align = Self::alignment(n, sh);
            self.body.push_str("q\n");
            let clipped = match align {
                StrokeAlign::Center => true,
                StrokeAlign::Inside => self.clip_inside(sh),
                StrokeAlign::Outside => self.clip_outside(sh),
            };
            if clipped {
                let width = if align == StrokeAlign::Center { s.weight } else { s.weight * 2.0 };
                self.set_alpha(alpha * paint.opacity * c.a);
                let cap = match s.cap {
                    StrokeCap::None => 0,
                    StrokeCap::Round => 1,
                    StrokeCap::Square => 2,
                };
                let join = match s.join {
                    StrokeJoin::Miter => 0,
                    StrokeJoin::Round => 1,
                    StrokeJoin::Bevel => 2,
                };
                let _ = writeln!(self.body, "{} {} {} RG {} w {cap} J {join} j {} M", num(c.r), num(c.g), num(c.b), num(width), num(s.miter_limit.max(1.0)));
                if let Some(dash) = dash_pattern(&s.dashes) {
                    let _ = writeln!(self.body, "{dash}");
                }
                for (cmds, _) in path_cmds(&sh.paths) {
                    emit_path_cmds(&mut self.body, &cmds);
                    self.body.push_str("S\n");
                }
            }
            self.body.push_str("Q\n");
        }

        /// A stroke written as the area it covers, filled with the stroke paint: precomputed outlines,
        /// per-side weights, and gradient strokes. The renderer's own stroke plan gives the area.
        fn stroke_area(&mut self, n: &Node, sh: &NodeShape, paint: &Paint, alpha: f32) {
            let Some(plan) = stroke_plan(n, sh, OUTLINE_RES_SCALE) else { return };
            self.body.push_str("q\n");
            let clipped = match plan.clip {
                StrokeClip::None => true,
                StrokeClip::Inside => self.clip_inside(sh),
                StrokeClip::Outside => self.clip_outside(sh),
            };
            if clipped {
                self.fill(&path_cmds(&plan.outline), paint, n.size, alpha);
            }
            self.body.push_str("Q\n");
        }

        // ----- text ----------------------------------------------------------------------------------

        /// Glyph outlines as filled paths, then the invisible text layer.
        fn text(&mut self, n: &Node, tp: &TextProps, alpha: f32) {
            let out = text::layout(tp, n.size);
            for (i, style) in out.styles.iter().enumerate() {
                let cmds: Vec<PathCmd> =
                    out.lines.iter().flat_map(|l| l.spans.iter().filter(|s| s.style == i)).flat_map(|s| s.outline.iter().copied()).collect();
                if cmds.is_empty() {
                    continue;
                }
                let paths = [(cmds, FillRule::NonZero)];
                for p in style.fills.iter().filter(|p| p.visible) {
                    self.fill(&paths, p, n.size, alpha);
                }
            }
            self.text_runs(&out, tp);
        }

        /// Every run as invisible text (render mode 3) at its baseline, so viewers can select, copy and
        /// search it while the visible glyphs stay the outlines.
        fn text_runs(&mut self, out: &TextOut, tp: &TextProps) {
            for line in &out.lines {
                for span in &line.spans {
                    let size = out.styles.get(span.style).unwrap_or(&tp.style).font_size.max(1.0);
                    // a run no part of which is on the visible page is not worth selecting; one that is only
                    // partly visible stays whole
                    let glyphs = cmd_bounds(&span.outline).unwrap_or([span.x, line.baseline - size, span.x, line.baseline]);
                    if !touches(in_page(&self.ctm, glyphs), self.visible) {
                        continue;
                    }
                    let (font, string) = match self.fonts.encode(span.text.trim_end_matches('\n')) {
                        None => continue,
                        Some(Encoded::Latin(s)) => ("F1", format!("({s})")),
                        Some(Encoded::Wide(hex)) => {
                            self.uses_wide_font = true;
                            ("F2", format!("<{hex}>"))
                        }
                    };
                    let _ = writeln!(self.body, "BT /{font} {} Tf 1 0 0 -1 {} {} Tm 3 Tr {string} Tj ET", num(size), num(span.x), num(line.baseline));
                }
            }
        }

        /// Only the invisible text of a subtree, for flattened pages.
        fn text_layer(&mut self, n: &Node) {
            if let NodeKind::Text(tp) = &n.kind {
                let out = text::layout(tp, n.size);
                self.text_runs(&out, tp);
                return;
            }
            if !draws_children(n) {
                return;
            }
            let doc = self.doc;
            let visible = self.visible;
            if n.kind.frame_props().is_some_and(|f| f.clips_content) {
                // like the canvas: a clipping node without a shape draws none of its children
                let Some(sh) = outline_of(doc, n, FLATTEN_TOLERANCE) else { return };
                let Some(first) = sh.paths.first() else { return };
                self.narrow_to(&Affine::IDENTITY, &first.0);
            }
            for cid in &n.children {
                let Some(raw) = doc.get(*cid) else { continue };
                let child = resolve_node(doc, raw);
                if child.is_mask {
                    // whatever the mask's strength, nothing shows outside its shape
                    if child.visible && !matches!(child.kind, NodeKind::Text(_) | NodeKind::Group) && !draws_children(&child) {
                        if let Some(first) = outline_of(doc, &child, FLATTEN_TOLERANCE).and_then(|sh| sh.fill_paths().first().cloned()) {
                            self.narrow_to(&child.transform, &first.0);
                        }
                    }
                    continue;
                }
                if !is_drawn(&child) {
                    continue;
                }
                let t = child.transform;
                let outer = self.ctm;
                self.ctm = self.ctm * t;
                let _ = writeln!(self.body, "q\n{} cm", [t.a, t.b, t.c, t.d, t.tx, t.ty].map(n5).join(" "));
                self.text_layer(&child);
                self.body.push_str("Q\n");
                self.ctm = outer;
            }
            self.visible = visible;
        }
    }

    /// A box in page pixels: left, top, right, bottom. A box with left > right holds nothing.
    type Bounds = [f32; 4];

    const NOTHING: Bounds = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];

    /// The box of every point of `cmds` (control points included), `None` for an empty outline.
    fn cmd_bounds(cmds: &[PathCmd]) -> Option<Bounds> {
        let mut points = cmds.iter().flat_map(|c| match *c {
            PathCmd::MoveTo(p) | PathCmd::LineTo(p) => vec![p],
            PathCmd::QuadTo(a, p) => vec![a, p],
            PathCmd::CubicTo(a, b, p) => vec![a, b, p],
            PathCmd::Close => vec![],
        });
        let first = points.next()?;
        Some(points.fold([first.x, first.y, first.x, first.y], |b, p| [b[0].min(p.x), b[1].min(p.y), b[2].max(p.x), b[3].max(p.y)]))
    }

    /// The box that `b`, in the space `m` maps from, covers after `m`.
    fn in_page(m: &Affine, b: Bounds) -> Bounds {
        let corners = [Vec2::new(b[0], b[1]), Vec2::new(b[2], b[1]), Vec2::new(b[2], b[3]), Vec2::new(b[0], b[3])].map(|c| m.apply(c));
        corners.iter().fold(NOTHING, |r, c| [r[0].min(c.x), r[1].min(c.y), r[2].max(c.x), r[3].max(c.y)])
    }

    fn narrow(a: Bounds, b: Bounds) -> Bounds {
        let r = [a[0].max(b[0]), a[1].max(b[1]), a[2].min(b[2]), a[3].min(b[3])];
        if r[0] > r[2] || r[1] > r[3] {
            NOTHING
        } else {
            r
        }
    }

    /// Whether the boxes share a point; a box that is not a number counts as visible.
    fn touches(a: Bounds, b: Bounds) -> bool {
        !(a[0] > b[2] || b[0] > a[2] || a[1] > b[3] || b[1] > a[3])
    }

    /// `[a b ...] 0 d` for a dash pattern the renderer would also apply: non-negative, not all zero.
    /// An odd-length pattern repeats, so it is written twice.
    fn dash_pattern(dashes: &[f32]) -> Option<String> {
        if dashes.is_empty() || dashes.iter().any(|d| d.is_nan() || *d < 0.0) || dashes.iter().sum::<f32>() <= 0.0 {
            return None;
        }
        let mut pattern = dashes.to_vec();
        if pattern.len() % 2 == 1 {
            pattern.extend_from_within(..);
        }
        Some(format!("[{}] 0 d", pattern.iter().map(|d| num(*d)).collect::<Vec<_>>().join(" ")))
    }

    fn path_cmds(paths: &[(SkPath, SkRule)]) -> Vec<(Vec<PathCmd>, FillRule)> {
        let v = |p: Point| Vec2::new(p.x, p.y);
        paths
            .iter()
            .map(|(path, rule)| {
                let cmds = path
                    .segments()
                    .map(|s| match s {
                        PathSegment::MoveTo(a) => PathCmd::MoveTo(v(a)),
                        PathSegment::LineTo(a) => PathCmd::LineTo(v(a)),
                        PathSegment::QuadTo(a, b) => PathCmd::QuadTo(v(a), v(b)),
                        PathSegment::CubicTo(a, b, c) => PathCmd::CubicTo(v(a), v(b), v(c)),
                        PathSegment::Close => PathCmd::Close,
                    })
                    .collect();
                (cmds, if *rule == SkRule::EvenOdd { FillRule::EvenOdd } else { FillRule::NonZero })
            })
            .collect()
    }

    fn emit_path_cmds(out: &mut String, cmds: &[PathCmd]) {
        let mut cur = Vec2::ZERO;
        let mut start = Vec2::ZERO;
        for cmd in cmds {
            match *cmd {
                PathCmd::MoveTo(p) => {
                    let _ = writeln!(out, "{} {} m", num(p.x), num(p.y));
                    cur = p;
                    start = p;
                }
                PathCmd::LineTo(p) => {
                    let _ = writeln!(out, "{} {} l", num(p.x), num(p.y));
                    cur = p;
                }
                PathCmd::QuadTo(ctrl, to) => {
                    let c1 = Vec2::new(cur.x + (ctrl.x - cur.x) * (2.0 / 3.0), cur.y + (ctrl.y - cur.y) * (2.0 / 3.0));
                    let c2 = Vec2::new(to.x + (ctrl.x - to.x) * (2.0 / 3.0), to.y + (ctrl.y - to.y) * (2.0 / 3.0));
                    let _ = writeln!(out, "{} {} {} {} {} {} c", num(c1.x), num(c1.y), num(c2.x), num(c2.y), num(to.x), num(to.y));
                    cur = to;
                }
                PathCmd::CubicTo(c1, c2, to) => {
                    let _ = writeln!(out, "{} {} {} {} {} {} c", num(c1.x), num(c1.y), num(c2.x), num(c2.y), num(to.x), num(to.y));
                    cur = to;
                }
                PathCmd::Close => {
                    out.push_str("h\n");
                    cur = start;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Color, Node, Paint, Rect};

    fn document() -> (Document, NodeId, NodeId) {
        let mut doc = Document::new("PDF test");
        let page = doc.add_page("Slides");
        let first = doc.alloc_id();
        let mut node = Node::frame(first, Rect::new(100.0, 200.0, 40.0, 20.0));
        node.fills = vec![Paint::solid(Color::rgb(255, 0, 0))];
        doc.insert(page, node).unwrap();
        let second = doc.alloc_id();
        let mut node = Node::frame(second, Rect::new(900.0, 200.0, 20.0, 40.0));
        node.fills = vec![Paint::solid(Color::rgb(0, 0, 255))];
        doc.insert(page, node).unwrap();
        (doc, first, second)
    }

    fn objects(pdf: &[u8]) -> Vec<&[u8]> {
        let marker = b"\nstartxref\n";
        let at = pdf.windows(marker.len()).rposition(|window| window == marker).unwrap() + marker.len();
        let end = pdf[at..].iter().position(|b| *b == b'\n').unwrap() + at;
        let offset: usize = std::str::from_utf8(&pdf[at..end]).unwrap().parse().unwrap();
        let xref = std::str::from_utf8(&pdf[offset..]).unwrap();
        let mut lines = xref.lines();
        assert_eq!(lines.next(), Some("xref"));
        let size: usize = lines.next().unwrap().split_whitespace().nth(1).unwrap().parse().unwrap();
        assert_eq!(lines.next(), Some("0000000000 65535 f "));
        let offsets: Vec<usize> = lines.take(size - 1).map(|line| line[..10].parse().unwrap()).collect();
        offsets
            .iter()
            .enumerate()
            .map(|(index, &start)| {
                let end = offsets.get(index + 1).copied().unwrap_or(offset);
                let object = &pdf[start..end];
                assert!(object.starts_with(format!("{} 0 obj\n", index + 1).as_bytes()));
                assert!(object.ends_with(b"\nendobj\n"));
                object
            })
            .collect()
    }

    fn stream_content(object: &[u8]) -> &str {
        let marker = b"stream\n";
        let at = object.windows(marker.len()).position(|window| window == marker).unwrap() + marker.len();
        let header = std::str::from_utf8(&object[..at]).unwrap();
        let length: usize = header.split("/Length ").nth(1).unwrap().split_whitespace().next().unwrap().parse().unwrap();
        assert!(object[at + length..].starts_with(b"\nendstream"));
        std::str::from_utf8(&object[at..at + length]).unwrap()
    }

    #[test]
    fn pdf_has_valid_offsets_order_dimensions_and_vector_page_streams() {
        let (doc, red, blue) = document();
        let pdf = export_pdf(&doc, &[blue, red]).unwrap();
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.ends_with(b"%%EOF\n"));
        let objects = objects(&pdf);
        assert_eq!(objects.len(), 6);
        assert!(std::str::from_utf8(objects[1]).unwrap().contains("/Count 2 /Kids [3 0 R 5 0 R]"));
        assert!(std::str::from_utf8(objects[2]).unwrap().contains("/MediaBox [0 0 15 30]"));
        assert!(std::str::from_utf8(objects[4]).unwrap().contains("/MediaBox [0 0 30 15]"));
        assert!(stream_content(objects[3]).contains("0 0 1 rg"), "first page fills with blue vector primitive");
        assert!(stream_content(objects[5]).contains("1 0 0 rg"), "second page fills with red vector primitive");
    }

    #[test]
    fn transparent_slide_is_white_and_single_slide_exports_alone() {
        let (mut doc, red, _) = document();
        doc.get_mut(red).unwrap().fills.clear();
        let pdf = export_pdf(&doc, &[red]).unwrap();
        let objects = objects(&pdf);
        assert_eq!(objects.len(), 4);
        assert!(stream_content(objects[3]).starts_with("1 1 1 rg 0 0 30 15 re f\n"));
    }

    #[test]
    fn large_frame_keeps_media_box_aspect_without_raster_image() {
        let (mut doc, id, _) = document();
        doc.get_mut(id).unwrap().size = designos_doc::Vec2::new(8192.0, 4.0);
        let pdf = export_pdf(&doc, &[id]).unwrap();
        let objects = objects(&pdf);
        assert!(std::str::from_utf8(objects[2]).unwrap().contains("/MediaBox [0 0 6144 3]"));
        assert!(!String::from_utf8_lossy(&pdf).contains("/Subtype /Image"));
    }

    #[test]
    fn invalid_selection_and_dimensions_return_errors() {
        let (mut doc, red, _) = document();
        assert!(export_pdf(&doc, &[]).is_err());
        assert!(export_pdf(&doc, &[NodeId(u64::MAX)]).is_err());
        assert!(export_pdf(&doc, &[doc.pages[0]]).is_err());
        assert!(export_pdf(&doc, &vec![red; MAX_PAGES + 1]).is_err());
        for width in [0.0, -1.0, f32::NAN, f32::INFINITY, 20_000.0] {
            doc.get_mut(red).unwrap().size.x = width;
            assert!(export_pdf(&doc, &[red]).is_err());
        }
    }

    #[test]
    fn two_frame_pdf_has_selectable_text_and_no_full_page_image() {
        use designos_doc::{GradientStop, PaintKind, Path, PathCmd, TextStyle, Vec2};

        let mut doc = Document::new("Vector PDF");
        let page = doc.add_page("Slides");

        let f1 = doc.alloc_id();
        let mut frame1 = Node::frame(f1, Rect::new(0.0, 0.0, 400.0, 200.0));
        frame1.fills = vec![Paint::solid(Color::rgb(240, 244, 248))];
        doc.insert(page, frame1).unwrap();

        let card_id = doc.alloc_id();
        let mut card = Node::rectangle(card_id, Rect::new(20.0, 20.0, 160.0, 80.0));
        card.corner_radii = [12.0; 4];
        card.fills = vec![Paint {
            visible: true,
            opacity: 1.0,
            blend: designos_doc::BlendMode::Normal,
            kind: PaintKind::LinearGradient {
                stops: vec![GradientStop { pos: 0.0, color: Color::rgb(255, 0, 0) }, GradientStop { pos: 1.0, color: Color::rgb(0, 0, 255) }],
                handles: [Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0)],
            },
        }];
        doc.insert(f1, card).unwrap();

        let t1_id = doc.alloc_id();
        let t1 = Node::text(t1_id, Rect::new(24.0, 120.0, 300.0, 40.0), "First slide headline", TextStyle::default());
        doc.insert(f1, t1).unwrap();

        let f2 = doc.alloc_id();
        let frame2 = Node::frame(f2, Rect::new(500.0, 0.0, 300.0, 150.0));
        doc.insert(page, frame2).unwrap();

        let vec_id = doc.alloc_id();
        let mut vec_node = Node::rectangle(vec_id, Rect::new(10.0, 10.0, 80.0, 60.0));
        vec_node.kind = NodeKind::Vector {
            paths: vec![Path {
                cmds: vec![PathCmd::MoveTo(Vec2::new(0.0, 0.0)), PathCmd::LineTo(Vec2::new(80.0, 0.0)), PathCmd::LineTo(Vec2::new(40.0, 60.0)), PathCmd::Close],
                fill_rule: designos_doc::FillRule::NonZero,
            }],
            network: None,
        };
        vec_node.fills = vec![Paint::solid(Color::rgb(0, 180, 90))];
        doc.insert(f2, vec_node).unwrap();

        let t2_id = doc.alloc_id();
        let t2 = Node::text(t2_id, Rect::new(16.0, 90.0, 240.0, 36.0), "Second slide takeaway", TextStyle::default());
        doc.insert(f2, t2).unwrap();

        let pdf = export_pdf(&doc, &[f1, f2]).unwrap();
        let raw = String::from_utf8_lossy(&pdf);
        assert!(!raw.contains("/Subtype /Image"), "vector PDF must not emit a full-page raster image XObject");
        assert!(raw.contains("/Shading"), "gradient fill should emit a PDF Shading dictionary");

        // Selectable text: every line is a `BT ... Tj ET` text run in the page streams.
        assert!(raw.contains("(First slide headline) Tj"), "page 1 text run missing from the PDF");
        assert!(raw.contains("(Second slide takeaway) Tj"), "page 2 text run missing from the PDF");

        // Cross-check with a real extractor when poppler is installed; hosted runners do not ship it.
        let tmp = std::env::temp_dir().join(format!("designos-vector-pdf-{}.pdf", std::process::id()));
        std::fs::write(&tmp, &pdf).unwrap();
        let extractor = std::process::Command::new("pdftotext").args(["-enc", "UTF-8"]).arg(&tmp).arg("-").output();
        let _ = std::fs::remove_file(&tmp);
        if let Ok(output) = extractor {
            assert!(output.status.success(), "pdftotext failed: {}", String::from_utf8_lossy(&output.stderr));
            let extracted = String::from_utf8_lossy(&output.stdout);
            assert!(extracted.contains("First slide headline"), "page 1 text missing from pdftotext output: {extracted:?}");
            assert!(extracted.contains("Second slide takeaway"), "page 2 text missing from pdftotext output: {extracted:?}");
        }
    }
}
