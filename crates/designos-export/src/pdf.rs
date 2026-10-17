//! Bounded vector PDF 1.4 export. Pages are clipped to frame bounds over a white
//! backdrop. Each design pixel maps to 0.75 PDF points (96 CSS pixels per inch).
//! Shapes, boolean operations, strokes, solid and gradient fills, clipping frames,
//! and glyph outlines are emitted as PDF vector primitives alongside selectable
//! text runs (`BT ... ET`) so text extractors (`pdftotext`) and PDF viewers can
//! search and copy text.

use crate::geo::{node_geo, rounded_rect, transform_cmds, Geo, GeoKind};
use crate::text;
use crate::util::num;
use designos_doc::{
    Affine, BooleanOp, Color, Document, FillRule, GradientStop, Node, NodeId, NodeKind, Paint, PaintKind, PathCmd, StrokeAlign, StrokeCap, StrokeJoin,
    TextProps, Vec2,
};
use std::fmt::Write as _;
use std::io::Write;

const MAX_PAGES: usize = 500;
const MAX_OUTPUT: usize = 128 * 1024 * 1024;
const POINTS_PER_PIXEL: f32 = 0.75;

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

struct PageBuilder<'a> {
    doc: &'a Document,
    body: String,
    shadings: Vec<String>,
    ext_gstates: Vec<f32>,
}

impl<'a> PageBuilder<'a> {
    fn new(doc: &'a Document) -> Self {
        Self { doc, body: String::new(), shadings: Vec::new(), ext_gstates: Vec::new() }
    }

    fn gs_for_alpha(&mut self, alpha: f32) -> usize {
        let a = (alpha.clamp(0.0, 1.0) * 1000.0).round() / 1000.0;
        if let Some(idx) = self.ext_gstates.iter().position(|&x| (x - a).abs() < 1e-4) {
            return idx;
        }
        let idx = self.ext_gstates.len();
        self.ext_gstates.push(a);
        idx
    }

    fn set_alpha(&mut self, alpha: f32) {
        let idx = self.gs_for_alpha(alpha);
        let _ = writeln!(self.body, "/GS{idx} gs");
    }

    fn resources_dict(&self) -> String {
        let mut res = String::from("<< /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >> >>");
        if !self.ext_gstates.is_empty() {
            res.push_str(" /ExtGState <<");
            for (i, a) in self.ext_gstates.iter().enumerate() {
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
        res.push_str(" >>");
        res
    }

    fn add_shading(&mut self, dict: String) -> usize {
        let idx = self.shadings.len();
        self.shadings.push(dict);
        idx
    }

    fn emit_slide(&mut self, root_id: NodeId) {
        let Some(root) = self.doc.get(root_id) else { return };
        let w = root.size.x;
        let h = root.size.y;
        let width_pt = w * POINTS_PER_PIXEL;
        let height_pt = h * POINTS_PER_PIXEL;

        // White page background in PDF points, then flip Y into top-left design pixels.
        let _ = writeln!(self.body, "1 1 1 rg 0 0 {} {} re f", num(width_pt), num(height_pt));
        let _ = writeln!(self.body, "q\n{} 0 0 -{} 0 {} cm", num(POINTS_PER_PIXEL), num(POINTS_PER_PIXEL), num(height_pt));
        // Clip to the frame boundary in design pixels.
        let frame_geo = node_geo(root);
        if let Some(g) = &frame_geo {
            self.emit_clip_geo(g);
        } else {
            let _ = writeln!(self.body, "0 0 {} {} re W n", num(w), num(h));
        }
        self.emit_node_contents(root, 1.0);
        self.body.push_str("Q\n");
    }

    fn emit_node(&mut self, id: NodeId, parent_opacity: f32) {
        let Some(n) = self.doc.get(id) else { return };
        if !n.visible || n.opacity <= 0.0 {
            return;
        }
        let op = (parent_opacity * n.opacity).clamp(0.0, 1.0);
        let t = n.transform;
        let _ = writeln!(self.body, "q\n{} {} {} {} {} {} cm", num(t.a), num(t.b), num(t.c), num(t.d), num(t.tx), num(t.ty));
        self.emit_node_contents(n, op);
        self.body.push_str("Q\n");
    }

    fn emit_node_contents(&mut self, n: &Node, opacity: f32) {
        if let NodeKind::Text(tp) = &n.kind {
            self.emit_text(n, tp, opacity);
            return;
        }
        let geo = match &n.kind {
            NodeKind::BooleanOp(op) => self.boolean_geo(n, *op),
            _ => node_geo(n),
        };
        if let Some(g) = &geo {
            if g.fillable {
                for paint in visible_paints(&n.fills) {
                    self.emit_fill(g, paint, n.size, opacity);
                }
            }
            if n.stroke.weight > 0.0 {
                for paint in visible_paints(&n.stroke.paints) {
                    self.emit_stroke(g, &n.stroke, paint, n.size, opacity);
                }
            }
        }
        if !matches!(n.kind, NodeKind::BooleanOp(_)) && !n.children.is_empty() {
            let clips = n.kind.frame_props().is_some_and(|fp| fp.clips_content);
            if clips {
                self.body.push_str("q\n");
                if let Some(g) = &geo {
                    self.emit_clip_geo(g);
                } else {
                    let _ = writeln!(self.body, "0 0 {} {} re W n", num(n.size.x), num(n.size.y));
                }
            }
            self.emit_children(&n.children, opacity);
            if clips {
                self.body.push_str("Q\n");
            }
        }
    }

    fn emit_children(&mut self, children: &[NodeId], opacity: f32) {
        let mut mask_open = false;
        for &cid in children {
            let Some(child) = self.doc.get(cid) else { continue };
            if !child.visible {
                continue;
            }
            if child.is_mask {
                if mask_open {
                    self.body.push_str("Q\n");
                }
                self.body.push_str("q\n");
                mask_open = true;
                if let Some(mut g) = node_geo(child) {
                    for (cmds, _) in &mut g.paths {
                        *cmds = transform_cmds(cmds, &child.transform);
                    }
                    self.emit_clip_geo(&g);
                }
                continue;
            }
            self.emit_node(cid, opacity);
        }
        if mask_open {
            self.body.push_str("Q\n");
        }
    }

    fn boolean_geo(&self, n: &Node, op: BooleanOp) -> Option<Geo> {
        if let Some(fg) = n.fill_geometry.as_deref().filter(|g| !g.is_empty()) {
            return node_geo(&Node { kind: NodeKind::Rectangle, fill_geometry: Some(fg.to_vec()), ..n.clone() });
        }
        let mut cmds = Vec::new();
        self.collect_operands(n, &Affine::IDENTITY, &mut cmds);
        if cmds.is_empty() {
            return None;
        }
        let rule = if matches!(op, BooleanOp::Exclude | BooleanOp::Subtract) { FillRule::EvenOdd } else { FillRule::NonZero };
        Some(Geo { kind: GeoKind::Path, paths: vec![(cmds, rule)], closed: true, fillable: true, is_rect: false, mask: None })
    }

    fn collect_operands(&self, parent: &Node, xf: &Affine, out: &mut Vec<PathCmd>) {
        for &cid in &parent.children {
            let Some(c) = self.doc.get(cid) else { continue };
            if !c.visible {
                continue;
            }
            let next_xf = *xf * c.transform;
            if let Some(g) = node_geo(c) {
                for (cmds, _) in g.paths {
                    out.extend(transform_cmds(&cmds, &next_xf));
                }
            } else {
                self.collect_operands(c, &next_xf, out);
            }
        }
    }

    fn emit_clip_geo(&mut self, geo: &Geo) {
        let mut even_odd = false;
        for (cmds, rule) in &geo.paths {
            emit_path_cmds(&mut self.body, cmds);
            if *rule == FillRule::EvenOdd {
                even_odd = true;
            }
        }
        let _ = writeln!(self.body, "{} n", if even_odd { "W*" } else { "W" });
    }

    fn emit_fill(&mut self, geo: &Geo, paint: &Paint, size: Vec2, node_opacity: f32) {
        let base_alpha = (paint.opacity * node_opacity).clamp(0.0, 1.0);
        match &paint.kind {
            PaintKind::Solid(c) => {
                self.set_alpha(base_alpha * c.a);
                let _ = writeln!(self.body, "{} {} {} rg", num(c.r), num(c.g), num(c.b));
                let mut even_odd = false;
                for (cmds, rule) in &geo.paths {
                    emit_path_cmds(&mut self.body, cmds);
                    if *rule == FillRule::EvenOdd {
                        even_odd = true;
                    }
                }
                let _ = writeln!(self.body, "{}", if even_odd { "f*" } else { "f" });
            }
            PaintKind::LinearGradient { stops, handles } if !stops.is_empty() => {
                let stop_alpha = stops.iter().map(|s| s.color.a).sum::<f32>() / stops.len() as f32;
                self.set_alpha(base_alpha * stop_alpha);
                let p0 = Vec2::new(handles[0].x * size.x, handles[0].y * size.y);
                let p1 = Vec2::new(handles[1].x * size.x, handles[1].y * size.y);
                let func = gradient_function_dict(stops);
                let sh_dict = format!(
                    "<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [{} {} {} {}] /Extend [true true] /Function {func} >>",
                    num(p0.x),
                    num(p0.y),
                    num(p1.x),
                    num(p1.y)
                );
                let idx = self.add_shading(sh_dict);
                self.body.push_str("q\n");
                self.emit_clip_geo(geo);
                let _ = writeln!(self.body, "/Sh{idx} sh\nQ");
            }
            PaintKind::RadialGradient { stops, handles } | PaintKind::AngularGradient { stops, handles } | PaintKind::DiamondGradient { stops, handles }
                if !stops.is_empty() =>
            {
                let stop_alpha = stops.iter().map(|s| s.color.a).sum::<f32>() / stops.len() as f32;
                self.set_alpha(base_alpha * stop_alpha);
                let p0 = Vec2::new(handles[0].x * size.x, handles[0].y * size.y);
                let dx = (handles[1].x - handles[0].x) * size.x;
                let dy = (handles[1].y - handles[0].y) * size.y;
                let r = dx.hypot(dy).max(0.01);
                let func = gradient_function_dict(stops);
                let sh_dict = format!(
                    "<< /ShadingType 3 /ColorSpace /DeviceRGB /Coords [{} {} 0 {} {} {}] /Extend [true true] /Function {func} >>",
                    num(p0.x),
                    num(p0.y),
                    num(p0.x),
                    num(p0.y),
                    num(r)
                );
                let idx = self.add_shading(sh_dict);
                self.body.push_str("q\n");
                self.emit_clip_geo(geo);
                let _ = writeln!(self.body, "/Sh{idx} sh\nQ");
            }
            _ => {}
        }
    }

    fn emit_stroke(&mut self, geo: &Geo, stroke: &designos_doc::Stroke, paint: &Paint, _size: Vec2, node_opacity: f32) {
        let color = match &paint.kind {
            PaintKind::Solid(c) => *c,
            PaintKind::LinearGradient { stops, .. }
            | PaintKind::RadialGradient { stops, .. }
            | PaintKind::AngularGradient { stops, .. }
            | PaintKind::DiamondGradient { stops, .. } => stops.first().map(|s| s.color).unwrap_or(Color::BLACK),
            _ => return,
        };
        let alpha = (paint.opacity * node_opacity * color.a).clamp(0.0, 1.0);
        self.body.push_str("q\n");
        let mut weight = stroke.weight;
        if geo.closed && stroke.align == StrokeAlign::Inside {
            self.emit_clip_geo(geo);
            weight *= 2.0;
        }
        self.set_alpha(alpha);
        let cap = match stroke.cap {
            StrokeCap::Round => 1,
            StrokeCap::Square => 2,
            _ => 0,
        };
        let join = match stroke.join {
            StrokeJoin::Miter => 0,
            StrokeJoin::Round => 1,
            StrokeJoin::Bevel => 2,
        };
        let _ = writeln!(self.body, "{} {} {} RG {} w {cap} J {join} j", num(color.r), num(color.g), num(color.b), num(weight));
        if !stroke.dashes.is_empty() {
            let dashes = stroke.dashes.iter().map(|d| num(*d)).collect::<Vec<_>>().join(" ");
            let _ = writeln!(self.body, "[{dashes}] 0 d");
        }
        for (cmds, _) in &geo.paths {
            emit_path_cmds(&mut self.body, cmds);
        }
        self.body.push_str("S\nQ\n");
    }

    fn emit_text(&mut self, n: &Node, tp: &TextProps, opacity: f32) {
        let out = text::layout(tp, n.size);
        // 1. Emit selectable PDF text runs (`BT ... ET`) so `pdftotext` and PDF viewers find every line.
        for line in &out.lines {
            for span in &line.spans {
                let clean = span.text.trim_end_matches('\n');
                if clean.is_empty() {
                    continue;
                }
                let st = out.styles.get(span.style).unwrap_or(&tp.style);
                let sz = st.font_size.max(1.0);
                let escaped = pdf_escape_text(clean);
                if span.outline.is_empty() {
                    let c = first_solid_color(&st.fills).or_else(|| first_solid_color(&n.fills)).unwrap_or(Color::BLACK);
                    self.set_alpha(opacity * c.a);
                    let _ = writeln!(
                        self.body,
                        "{} {} {} rg BT /F1 {} Tf 1 0 0 -1 {} {} Tm 0 Tr ({escaped}) Tj ET",
                        num(c.r),
                        num(c.g),
                        num(c.b),
                        num(sz),
                        num(span.x),
                        num(line.baseline)
                    );
                } else {
                    let _ = writeln!(self.body, "BT /F1 {} Tf 1 0 0 -1 {} {} Tm 3 Tr ({escaped}) Tj ET", num(sz), num(span.x), num(line.baseline));
                }
            }
        }
        // 2. Emit exact vector glyph outlines for each style run.
        for (i, style) in out.styles.iter().enumerate() {
            let mut cmds = Vec::new();
            for line in &out.lines {
                for span in line.spans.iter().filter(|s| s.style == i) {
                    cmds.extend(span.outline.iter().copied());
                }
            }
            if cmds.is_empty() {
                continue;
            }
            let geo = Geo { kind: GeoKind::Path, paths: vec![(cmds, FillRule::NonZero)], closed: true, fillable: true, is_rect: false, mask: None };
            let paints = if style.fills.is_empty() { &n.fills } else { &style.fills };
            for p in visible_paints(paints) {
                self.emit_fill(&geo, p, n.size, opacity);
            }
        }
        let _ = rounded_rect;
    }
}

fn visible_paints(paints: &[Paint]) -> impl Iterator<Item = &Paint> {
    paints.iter().filter(|p| p.visible && p.opacity > 0.0)
}

fn first_solid_color(paints: &[Paint]) -> Option<Color> {
    visible_paints(paints).find_map(|p| match &p.kind {
        PaintKind::Solid(c) => Some(*c),
        PaintKind::LinearGradient { stops, .. }
        | PaintKind::RadialGradient { stops, .. }
        | PaintKind::AngularGradient { stops, .. }
        | PaintKind::DiamondGradient { stops, .. } => stops.first().map(|s| s.color),
        _ => None,
    })
}

fn gradient_function_dict(stops: &[GradientStop]) -> String {
    let mut sorted = stops.to_vec();
    sorted.sort_by(|a, b| a.pos.partial_cmp(&b.pos).unwrap_or(std::cmp::Ordering::Equal));
    if sorted.len() == 1 {
        let c = sorted[0].color;
        return format!("<< /FunctionType 2 /Domain [0 1] /C0 [{} {} {}] /C1 [{} {} {}] /N 1 >>", num(c.r), num(c.g), num(c.b), num(c.r), num(c.g), num(c.b));
    }
    if sorted.len() == 2 {
        let (a, b) = (sorted[0].color, sorted[1].color);
        return format!("<< /FunctionType 2 /Domain [0 1] /C0 [{} {} {}] /C1 [{} {} {}] /N 1 >>", num(a.r), num(a.g), num(a.b), num(b.r), num(b.g), num(b.b));
    }
    let mut funcs = Vec::new();
    let mut bounds = Vec::new();
    let mut encode = Vec::new();
    for (idx, pair) in sorted.windows(2).enumerate() {
        let (a, b) = (pair[0].color, pair[1].color);
        funcs.push(format!(
            "<< /FunctionType 2 /Domain [0 1] /C0 [{} {} {}] /C1 [{} {} {}] /N 1 >>",
            num(a.r),
            num(a.g),
            num(a.b),
            num(b.r),
            num(b.g),
            num(b.b)
        ));
        if idx + 1 < sorted.len() - 1 {
            bounds.push(num(pair[1].pos.clamp(0.0, 1.0)));
        }
        encode.push("0 1");
    }
    format!("<< /FunctionType 3 /Domain [0 1] /Functions [{}] /Bounds [{}] /Encode [{}] >>", funcs.join(" "), bounds.join(" "), encode.join(" "))
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

fn pdf_escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\r' | '\n' => out.push(' '),
            '\u{2022}' => out.push_str("\\225"),
            '\u{2013}' => out.push_str("\\226"),
            '\u{2014}' => out.push_str("\\227"),
            '\u{2018}' => out.push_str("\\221"),
            '\u{2019}' => out.push_str("\\222"),
            '\u{201C}' => out.push_str("\\223"),
            '\u{201D}' => out.push_str("\\224"),
            '\u{2026}' => out.push_str("\\205"),
            c if (' '..='~').contains(&c) => out.push(c),
            c if (c as u32) <= 0xFF => {
                let _ = write!(out, "\\{:03o}", c as u8);
            }
            _ => out.push('?'),
        }
    }
    out
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
    // Validate the complete selection before spending time emitting any page.
    for id in slides {
        let node = doc.get(*id).ok_or_else(|| format!("Frame {} no longer exists.", id.0))?;
        if !matches!(node.kind, NodeKind::Frame(_)) {
            return Err(format!("{} is not a frame; only frames export as PDF pages.", node.name));
        }
        let size = node.size;
        if !size.x.is_finite() || !size.y.is_finite() || size.x < 1.0 || size.y < 1.0 || size.x > 19_200.0 || size.y > 19_200.0 {
            return Err("Frame dimensions must be finite and between 1 and 19,200 pixels.".into());
        }
    }
    let mut writer = Writer::new();
    writer.object(1, b"<< /Type /Catalog /Pages 2 0 R >>")?;
    let kids = (0..slides.len()).map(|i| format!("{} 0 R", 3 + i * 2)).collect::<Vec<_>>().join(" ");
    writer.object(2, format!("<< /Type /Pages /Count {} /Kids [{kids}] >>", slides.len()).as_bytes())?;
    for (index, id) in slides.iter().enumerate() {
        let node = doc.get(*id).expect("validated slide");
        let mut page_builder = PageBuilder::new(doc);
        page_builder.emit_slide(*id);
        let page_obj = 3 + index * 2;
        let contents_obj = page_obj + 1;
        let width = num(node.size.x * POINTS_PER_PIXEL);
        let height = num(node.size.y * POINTS_PER_PIXEL);
        let resources = page_builder.resources_dict();
        writer.object(
            page_obj,
            format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] /Resources {resources} /Contents {contents_obj} 0 R >>").as_bytes(),
        )?;
        writer.stream(contents_obj, "", page_builder.body.as_bytes())?;
    }
    writer.finish()
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

        // Verify pdftotext extracts selectable text from both pages.
        let tmp = std::env::temp_dir().join(format!("designos-vector-pdf-{}.pdf", std::process::id()));
        std::fs::write(&tmp, &pdf).unwrap();
        let output = std::process::Command::new("pdftotext").arg(&tmp).arg("-").output().expect("pdftotext runs");
        let _ = std::fs::remove_file(&tmp);
        assert!(output.status.success(), "pdftotext failed: {}", String::from_utf8_lossy(&output.stderr));
        let extracted = String::from_utf8_lossy(&output.stdout);
        assert!(extracted.contains("First slide headline"), "page 1 text missing from pdftotext output: {extracted:?}");
        assert!(extracted.contains("Second slide takeaway"), "page 2 text missing from pdftotext output: {extracted:?}");
    }
}
