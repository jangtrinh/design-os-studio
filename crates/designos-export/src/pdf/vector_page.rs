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
}

impl<'a, 'f> PageBuilder<'a, 'f> {
    fn new(doc: &'a Document, fonts: &'f mut TextFonts, page: Vec2) -> Self {
        Self { doc, fonts, page, body: String::new(), resources: PageResources::default(), uses_wide_font: false, ctm: Affine::IDENTITY }
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
        if clips {
            // a clipping node without a shape draws none of its children
            let Some((cmds, rule)) = shape.and_then(|sh| path_cmds(&sh.paths).into_iter().next()) else { return };
            self.body.push_str("q\n");
            self.clip(&cmds, rule);
        }
        let doc = self.doc;
        let mut open_masks = 0;
        for cid in &n.children {
            let Some(raw) = doc.get(*cid) else { continue };
            let child = resolve_node(doc, raw);
            if child.is_mask {
                // a mask narrows the siblings after it; the page checks keep only opaque one-shape masks
                if !child.visible {
                    continue;
                }
                if let Some(sh) = outline_of(doc, &child, FLATTEN_TOLERANCE) {
                    if let Some((cmds, rule)) = path_cmds(sh.fill_paths()).into_iter().next() {
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
    /// covers the page plus the path). False when there is no such path or the space is singular.
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
            let cmds: Vec<PathCmd> = out.lines.iter().flat_map(|l| l.spans.iter().filter(|s| s.style == i)).flat_map(|s| s.outline.iter().copied()).collect();
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
        for cid in &n.children {
            let Some(raw) = doc.get(*cid) else { continue };
            let child = resolve_node(doc, raw);
            if child.is_mask || !is_drawn(&child) {
                continue;
            }
            let t = child.transform;
            let _ = writeln!(self.body, "q\n{} cm", [t.a, t.b, t.c, t.d, t.tx, t.ty].map(n5).join(" "));
            self.text_layer(&child);
            self.body.push_str("Q\n");
        }
    }
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
