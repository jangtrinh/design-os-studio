//! SVG writer.
//!
//! Layer structure follows Figma's export: one `<g>` per node carrying its
//! transform, opacity, blend mode and layer blur; fills and strokes as shape
//! elements inside it; children after (inside a `<clipPath>` group for frames
//! that clip). Gradients use the node's handle basis as `gradientTransform`,
//! strokes emulate inside/outside alignment with clip/mask, shadows are one
//! SVG filter per node.
//!
//! Approximations (see crate docs): angular and diamond gradients are built
//! from solid wedges/bands, background blur has no SVG equivalent and is
//! dropped, corner smoothing and polygon corner rounding are not applied, and
//! subtract/intersect boolean operations use masks instead of resolved geometry
//! unless the node carries imported `fill_geometry`.

use crate::bounds::{own_overflow, render_bounds};
use crate::geo::{node_geo, path_d, transform_cmds, Geo, GeoKind};
use crate::text;
use crate::util::{esc, hex, num, svg_transform};
use base64::Engine;
use designos_doc::*;
use std::collections::HashMap;
use std::f32::consts::TAU;

#[derive(Clone, Debug)]
pub struct SvgOptions {
    /// Convert text to glyph outlines (Figma's "Outline text", on by default).
    /// Off emits `<text>` with font attributes, one `<tspan>` per laid out line.
    pub outline_text: bool,
}

impl Default for SvgOptions {
    fn default() -> Self {
        Self { outline_text: true }
    }
}

/// SVG document for `node` (its subtree), sized to the layer plus effect overflow.
pub fn export_svg(doc: &Document, node: NodeId, opts: &SvgOptions) -> String {
    // board items are written as their lowered shapes and text
    if let Some(lowered) = designos_render::board::lowered_document(doc) {
        return export_svg_plain(&lowered, node, opts);
    }
    export_svg_plain(doc, node, opts)
}

fn export_svg_plain(doc: &Document, node: NodeId, opts: &SvgOptions) -> String {
    let Some(n) = doc.get(node) else {
        return "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"0\" height=\"0\"/>\n".to_string();
    };
    let b = render_bounds(doc, node);
    let mut s = Svg { doc, opts, defs: String::new(), body: String::new(), next: 0, depth: 1, images: HashMap::new() };
    if matches!(n.kind, NodeKind::Page { .. }) {
        let kids = n.children.clone();
        s.children(&kids, b);
    } else {
        s.node(node, true, false);
    }
    let mut out = String::new();
    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" width=\"{}\" height=\"{}\" viewBox=\"{} {} {} {}\" fill=\"none\">\n",
        num(b.w),
        num(b.h),
        num(b.x),
        num(b.y),
        num(b.w),
        num(b.h)
    ));
    if !s.defs.is_empty() {
        out.push_str("  <defs>\n");
        out.push_str(&s.defs);
        out.push_str("  </defs>\n");
    }
    out.push_str(&s.body);
    out.push_str("</svg>\n");
    out
}

struct Svg<'a> {
    doc: &'a Document,
    opts: &'a SvgOptions,
    defs: String,
    body: String,
    next: u32,
    depth: usize,
    /// image_ref -> (symbol id, width, height)
    images: HashMap<String, (String, u32, u32)>,
}

pub(crate) fn blend_css(b: BlendMode) -> Option<&'static str> {
    Some(match b {
        BlendMode::Normal | BlendMode::PassThrough => return None,
        BlendMode::Darken => "darken",
        BlendMode::Multiply => "multiply",
        BlendMode::ColorBurn => "color-burn",
        // CSS has no linear burn / linear dodge; closest equivalents.
        BlendMode::LinearBurn => "multiply",
        BlendMode::Lighten => "lighten",
        BlendMode::Screen => "screen",
        BlendMode::ColorDodge => "color-dodge",
        BlendMode::LinearDodge => "plus-lighter",
        BlendMode::Overlay => "overlay",
        BlendMode::SoftLight => "soft-light",
        BlendMode::HardLight => "hard-light",
        BlendMode::Difference => "difference",
        BlendMode::Exclusion => "exclusion",
        BlendMode::Hue => "hue",
        BlendMode::Saturation => "saturation",
        BlendMode::Color => "color",
        BlendMode::Luminosity => "luminosity",
    })
}

fn visible_paints(paints: &[Paint]) -> impl Iterator<Item = &Paint> {
    paints.iter().filter(|p| p.visible && p.opacity > 0.0)
}

fn sorted_stops(stops: &[GradientStop]) -> Vec<GradientStop> {
    let mut s = stops.to_vec();
    s.sort_by(|a, b| a.pos.partial_cmp(&b.pos).unwrap_or(std::cmp::Ordering::Equal));
    s
}

/// Color of a gradient at `t` (straight alpha).
fn eval_stops(stops: &[GradientStop], t: f32) -> Color {
    let Some(first) = stops.first() else { return Color::TRANSPARENT };
    let last = stops.last().unwrap();
    if t <= first.pos {
        return first.color;
    }
    if t >= last.pos {
        return last.color;
    }
    for w in stops.windows(2) {
        if t >= w[0].pos && t <= w[1].pos {
            let f = if w[1].pos > w[0].pos { (t - w[0].pos) / (w[1].pos - w[0].pos) } else { 0.0 };
            let l = |a: f32, b: f32| a + (b - a) * f;
            return Color {
                r: l(w[0].color.r, w[1].color.r),
                g: l(w[0].color.g, w[1].color.g),
                b: l(w[0].color.b, w[1].color.b),
                a: l(w[0].color.a, w[1].color.a),
            };
        }
    }
    last.color
}

/// Unit gradient space -> node-local space (the handle basis).
fn gradient_matrix(handles: &[Vec2; 3], size: Vec2) -> Affine {
    let [h0, h1, h2] = *handles;
    Affine::scale(size.x, size.y) * Affine { a: h1.x - h0.x, b: h1.y - h0.y, c: h2.x - h0.x, d: h2.y - h0.y, tx: h0.x, ty: h0.y }
}

fn matrix_attr(a: &Affine) -> String {
    format!("matrix({} {} {} {} {} {})", num(a.a), num(a.b), num(a.c), num(a.d), num(a.tx), num(a.ty))
}

/// Rectangle outline at (x, y) as a path.
fn rect_geo(x: f32, y: f32, w: f32, h: f32) -> Geo {
    let t = Affine::translate(x, y);
    Geo {
        kind: GeoKind::Path,
        paths: vec![(transform_cmds(&crate::geo::rounded_rect(w, h, [0.0; 4]), &t), FillRule::NonZero)],
        closed: true,
        fillable: true,
        is_rect: false,
        mask: None,
    }
}

impl Svg<'_> {
    // ---- plumbing ---------------------------------------------------------

    fn uid(&mut self, prefix: &str) -> String {
        let id = format!("{prefix}{}", self.next);
        self.next += 1;
        id
    }

    fn line(&mut self, s: &str) {
        for _ in 0..self.depth {
            self.body.push_str("  ");
        }
        self.body.push_str(s);
        self.body.push('\n');
    }

    fn def(&mut self, s: &str) {
        self.defs.push_str("    ");
        self.defs.push_str(s);
        self.defs.push('\n');
    }

    // ---- tree -------------------------------------------------------------

    fn children(&mut self, ids: &[NodeId], region: Rect) {
        let doc = self.doc;
        for (i, id) in ids.iter().enumerate() {
            let Some(c) = doc.get(*id) else { continue };
            if c.is_mask {
                if !c.visible {
                    continue;
                }
                let mid = self.mask_def(*id, region);
                self.line(&format!("<g mask=\"url(#{mid})\">"));
                self.depth += 1;
                self.children(&ids[i + 1..], region);
                self.depth -= 1;
                self.line("</g>");
                return;
            }
            self.node(*id, false, false);
        }
    }

    fn mask_def(&mut self, id: NodeId, region: Rect) -> String {
        let mid = self.uid("mask");
        let saved = std::mem::take(&mut self.body);
        let saved_depth = std::mem::replace(&mut self.depth, 3);
        self.node(id, false, true);
        let content = std::mem::replace(&mut self.body, saved);
        self.depth = saved_depth;
        let pad = 2.0;
        self.def(&format!(
            "<mask id=\"{mid}\" style=\"mask-type:alpha\" maskUnits=\"userSpaceOnUse\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\">",
            num(region.x - pad),
            num(region.y - pad),
            num(region.w + 2.0 * pad),
            num(region.h + 2.0 * pad)
        ));
        self.defs.push_str(&content);
        self.def("</mask>");
        mid
    }

    fn node(&mut self, id: NodeId, root: bool, as_mask: bool) {
        let doc = self.doc;
        let Some(n) = doc.get(id) else { return };
        if !n.visible || matches!(n.kind, NodeKind::Unknown { .. } | NodeKind::Page { .. }) {
            return;
        }
        let mut attrs = format!(" data-name=\"{}\"", esc(&n.name));
        if !root {
            if let Some(t) = svg_transform(&n.transform) {
                attrs.push_str(&format!(" transform=\"{t}\""));
            }
        }
        if !as_mask {
            if n.opacity < 1.0 {
                attrs.push_str(&format!(" opacity=\"{}\"", num(n.opacity.max(0.0))));
            }
            if let Some(b) = blend_css(n.blend) {
                attrs.push_str(&format!(" style=\"mix-blend-mode:{b}\""));
            }
            let blur = n
                .effects
                .iter()
                .filter_map(|e| match e {
                    Effect::LayerBlur { radius, visible: true } if *radius > 0.0 => Some(*radius),
                    _ => None,
                })
                .fold(0.0f32, f32::max);
            if blur > 0.0 {
                let fid = self.blur_filter(id, blur);
                attrs.push_str(&format!(" filter=\"url(#{fid})\""));
            }
        }
        self.line(&format!("<g{attrs}>"));
        self.depth += 1;
        self.body(n, as_mask);
        self.depth -= 1;
        self.line("</g>");
    }

    fn body(&mut self, n: &Node, as_mask: bool) {
        let doc = self.doc;
        // Drop/inner shadows, noise and texture are all SVG filter primitives on the node.
        // Glass has no SVG equivalent (it needs the backdrop), so it is skipped.
        let has_shadow = !as_mask
            && n.effects.iter().any(|e| {
                matches!(e, Effect::DropShadow { visible: true, .. } | Effect::InnerShadow { visible: true, .. })
                    || matches!(e, Effect::Noise(x) if x.visible)
                    || matches!(e, Effect::Texture(x) if x.visible && x.radius > 0.0)
            });
        let filter = if has_shadow { Some(self.shadow_filter(n)) } else { None };
        let geo = match &n.kind {
            NodeKind::BooleanOp(op) => self.bool_geo(n, *op),
            NodeKind::Text(_) => None,
            _ => node_geo(n),
        };
        let own_paint = matches!(n.kind, NodeKind::Text(_)) || geo.is_some();

        if own_paint {
            if let Some(f) = &filter {
                self.line(&format!("<g filter=\"url(#{f})\">"));
                self.depth += 1;
            }
            match (&n.kind, &geo) {
                (NodeKind::Text(t), _) => self.text_node(n, t),
                (_, Some(g)) => {
                    self.fills(n, g, as_mask);
                    if !as_mask && g.mask.is_none() {
                        self.strokes(n, g);
                    }
                }
                _ => {}
            }
            if filter.is_some() {
                self.depth -= 1;
                self.line("</g>");
            }
        }

        let draws_kids = !n.children.is_empty() && !matches!(n.kind, NodeKind::BooleanOp(_) | NodeKind::Text(_));
        if !draws_kids {
            return;
        }
        let clip = match (&n.kind.frame_props(), &geo) {
            (Some(f), Some(g)) if f.clips_content => Some(self.clip_def(g)),
            _ => None,
        };
        if let Some(c) = &clip {
            self.line(&format!("<g clip-path=\"url(#{c})\">"));
            self.depth += 1;
        }
        // A group has no paint of its own: its shadows apply to the children.
        let group_filter = if !own_paint { filter } else { None };
        if let Some(f) = &group_filter {
            self.line(&format!("<g filter=\"url(#{f})\">"));
            self.depth += 1;
        }
        let region = render_bounds(doc, n.id);
        let kids = n.paint_children().into_owned();
        self.children(&kids, region);
        if group_filter.is_some() {
            self.depth -= 1;
            self.line("</g>");
        }
        if clip.is_some() {
            self.depth -= 1;
            self.line("</g>");
        }
    }

    // ---- shapes -----------------------------------------------------------

    /// Shape element lines for `geo`; `attrs` is appended verbatim (starts with a space).
    fn shape_lines(&self, geo: &Geo, attrs: &str, clip_rule: bool) -> Vec<String> {
        let extra = geo.mask.as_ref().map(|m| format!(" {m}")).unwrap_or_default();
        let rule_name = if clip_rule { "clip-rule" } else { "fill-rule" };
        match &geo.kind {
            GeoKind::Rect { w, h, r } => {
                let rx = if *r > 0.0 { format!(" rx=\"{}\"", num(*r)) } else { String::new() };
                vec![format!("<rect width=\"{}\" height=\"{}\"{rx}{attrs}{extra}/>", num(*w), num(*h))]
            }
            GeoKind::Ellipse { w, h } => {
                vec![format!("<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\"{attrs}{extra}/>", num(w / 2.0), num(h / 2.0), num(w / 2.0), num(h / 2.0))]
            }
            GeoKind::Path => geo
                .paths
                .iter()
                .map(|(cmds, rule)| {
                    let r = if *rule == FillRule::EvenOdd { format!(" {rule_name}=\"evenodd\"") } else { String::new() };
                    format!("<path d=\"{}\"{r}{attrs}{extra}/>", path_d(cmds))
                })
                .collect(),
        }
    }

    fn emit_lines(&mut self, lines: Vec<String>) {
        for l in lines {
            self.line(&l);
        }
    }

    fn clip_def(&mut self, geo: &Geo) -> String {
        let id = self.uid("clip");
        self.def(&format!("<clipPath id=\"{id}\">"));
        for l in self.shape_lines(geo, "", true) {
            self.def(&format!("  {l}"));
        }
        self.def("</clipPath>");
        id
    }

    // ---- paints -----------------------------------------------------------

    fn image_symbol(&mut self, image_ref: &str) -> Option<(String, u32, u32)> {
        if let Some(hit) = self.images.get(image_ref) {
            return Some(hit.clone());
        }
        let data = self.doc.images.get(image_ref)?;
        let fmt = image::guess_format(data).ok()?;
        let mime = match fmt {
            image::ImageFormat::Png => "image/png",
            image::ImageFormat::Jpeg => "image/jpeg",
            image::ImageFormat::Gif => "image/gif",
            image::ImageFormat::WebP => "image/webp",
            _ => return None,
        };
        let (w, h) = image::ImageReader::new(std::io::Cursor::new(data)).with_guessed_format().ok()?.into_dimensions().ok()?;
        let id = self.uid("image");
        let b64 = base64::engine::general_purpose::STANDARD.encode(data);
        self.def(&format!("<image id=\"{id}\" width=\"{w}\" height=\"{h}\" xlink:href=\"data:{mime};base64,{b64}\"/>"));
        self.images.insert(image_ref.to_string(), (id.clone(), w, h));
        Some((id, w, h))
    }

    fn stops_lines(&self, stops: &[GradientStop], opacity: f32) -> Vec<String> {
        sorted_stops(stops)
            .iter()
            .map(|s| {
                let a = s.color.a * opacity;
                let so = if a < 0.999 { format!(" stop-opacity=\"{}\"", num(a)) } else { String::new() };
                format!("<stop offset=\"{}\" stop-color=\"{}\"{so}/>", num(s.pos.clamp(0.0, 1.0)), hex(&s.color))
            })
            .collect()
    }

    /// Value for a `fill`/`stroke` attribute plus its opacity, registering
    /// whatever definitions it needs. `None` when the paint draws nothing.
    fn paint_value(&mut self, p: &Paint, size: Vec2) -> Option<(String, f32)> {
        match &p.kind {
            PaintKind::Solid(c) => Some((hex(c), c.a * p.opacity)),
            PaintKind::LinearGradient { handles, stops } | PaintKind::RadialGradient { handles, stops } => {
                if stops.is_empty() {
                    return None;
                }
                if stops.len() == 1 {
                    let c = stops[0].color;
                    return Some((hex(&c), c.a * p.opacity));
                }
                let linear = matches!(p.kind, PaintKind::LinearGradient { .. });
                let id = self.uid(if linear { "linear" } else { "radial" });
                let m = matrix_attr(&gradient_matrix(handles, size));
                if linear {
                    self.def(&format!(
                        "<linearGradient id=\"{id}\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"0\" gradientUnits=\"userSpaceOnUse\" gradientTransform=\"{m}\">"
                    ));
                } else {
                    self.def(&format!("<radialGradient id=\"{id}\" cx=\"0\" cy=\"0\" r=\"1\" gradientUnits=\"userSpaceOnUse\" gradientTransform=\"{m}\">"));
                }
                for l in self.stops_lines(stops, p.opacity) {
                    self.def(&format!("  {l}"));
                }
                self.def(if linear { "</linearGradient>" } else { "</radialGradient>" });
                Some((format!("url(#{id})"), 1.0))
            }
            PaintKind::AngularGradient { stops, .. } | PaintKind::DiamondGradient { stops, .. } => {
                // Strokes can't use wedges; fall back to the first stop.
                let c = sorted_stops(stops).first()?.color;
                Some((hex(&c), c.a * p.opacity))
            }
            PaintKind::Image { image_ref, scale_mode } => {
                let (img, iw, ih) = self.image_symbol(image_ref)?;
                let (iw, ih) = (iw as f32, ih as f32);
                let id = self.uid("pattern");
                if *scale_mode == ImageScaleMode::Tile {
                    self.def(&format!("<pattern id=\"{id}\" patternUnits=\"userSpaceOnUse\" width=\"{}\" height=\"{}\">", num(iw), num(ih)));
                    self.def(&format!("  <use xlink:href=\"#{img}\"/>"));
                } else {
                    let (s, tx, ty) = match scale_mode {
                        ImageScaleMode::Fit => {
                            let s = (size.x / iw).min(size.y / ih);
                            (s, (size.x - iw * s) / 2.0, (size.y - ih * s) / 2.0)
                        }
                        _ => {
                            let s = (size.x / iw).max(size.y / ih);
                            (s, (size.x - iw * s) / 2.0, (size.y - ih * s) / 2.0)
                        }
                    };
                    self.def(&format!("<pattern id=\"{id}\" patternUnits=\"userSpaceOnUse\" width=\"{}\" height=\"{}\">", num(size.x), num(size.y)));
                    self.def(&format!("  <use xlink:href=\"#{img}\" transform=\"matrix({} 0 0 {} {} {})\"/>", num(s), num(s), num(tx), num(ty)));
                }
                self.def("</pattern>");
                Some((format!("url(#{id})"), p.opacity))
            }
        }
    }

    /// Lines painting `geo` with `p` as a fill.
    fn fill_lines(&mut self, geo: &Geo, p: &Paint, size: Vec2) -> Vec<String> {
        let blend = blend_css(p.blend).map(|b| format!(" style=\"mix-blend-mode:{b}\"")).unwrap_or_default();
        match &p.kind {
            PaintKind::AngularGradient { handles, stops } | PaintKind::DiamondGradient { handles, stops } if stops.len() > 1 => {
                let angular = matches!(p.kind, PaintKind::AngularGradient { .. });
                let clip = self.clip_def(geo);
                let sorted = sorted_stops(stops);
                let m = matrix_attr(&gradient_matrix(handles, size));
                let mut out = vec![format!("<g clip-path=\"url(#{clip})\"{blend}>"), format!("  <g transform=\"{m}\">")];
                let op = |c: &Color| {
                    let a = c.a * p.opacity;
                    if a < 0.999 {
                        format!(" fill-opacity=\"{}\"", num(a))
                    } else {
                        String::new()
                    }
                };
                if angular {
                    const N: usize = 90;
                    let r = 4.0f32;
                    for k in 0..N {
                        let (t0, t1) = (k as f32 / N as f32, (k as f32 + 1.0) / N as f32);
                        let c = eval_stops(&sorted, (t0 + t1) / 2.0);
                        // slight overlap hides hairline seams between wedges
                        let (a0, a1) = (t0 * TAU, t1 * TAU + 0.01);
                        out.push(format!(
                            "    <path d=\"M0 0 L{} {} L{} {} Z\" fill=\"{}\"{}/>",
                            num(r * a0.cos()),
                            num(r * a0.sin()),
                            num(r * a1.cos()),
                            num(r * a1.sin()),
                            hex(&c),
                            op(&c)
                        ));
                    }
                } else {
                    const N: usize = 48;
                    for k in 0..N {
                        let t = 1.0 - k as f32 / N as f32;
                        let c = eval_stops(&sorted, t - 0.5 / N as f32);
                        out.push(format!("    <path d=\"M{} 0 L0 {} L-{} 0 L0 -{} Z\" fill=\"{}\"{}/>", num(t), num(t), num(t), num(t), hex(&c), op(&c)));
                    }
                }
                out.push("  </g>".into());
                out.push("</g>".into());
                out
            }
            _ => {
                let Some((val, op)) = self.paint_value(p, size) else { return vec![] };
                let mut attrs = format!(" fill=\"{val}\"");
                if op < 0.999 {
                    attrs.push_str(&format!(" fill-opacity=\"{}\"", num(op)));
                }
                attrs.push_str(&blend);
                self.shape_lines(geo, &attrs, false)
            }
        }
    }

    fn fills(&mut self, n: &Node, geo: &Geo, as_mask: bool) {
        if !geo.fillable {
            return;
        }
        if as_mask {
            // alpha masks only care about coverage; one opaque fill is enough
            let lines = self.fill_lines(geo, &Paint::solid(Color::BLACK), n.size);
            self.emit_lines(lines);
            return;
        }
        for p in visible_paints(&n.fills) {
            let lines = self.fill_lines(geo, p, n.size);
            self.emit_lines(lines);
        }
    }

    fn strokes(&mut self, n: &Node, geo: &Geo) {
        let st = &n.stroke;
        let paints: Vec<&Paint> = visible_paints(&st.paints).collect();
        if paints.is_empty() || (st.weight <= 0.0 && st.side_weights.is_none()) {
            return;
        }
        // Imported outlines are filled with the stroke paints.
        if let Some(sg) = n.stroke_geometry.as_deref().filter(|g| !g.is_empty()) {
            let g = Geo {
                kind: GeoKind::Path,
                paths: sg.iter().map(|p| (p.cmds.clone(), p.fill_rule)).collect(),
                closed: true,
                fillable: true,
                is_rect: false,
                mask: None,
            };
            for p in paints {
                let lines = self.fill_lines(&g, p, n.size);
                self.emit_lines(lines);
            }
            return;
        }
        if let (true, Some([t, r, b, l])) = (geo.is_rect, st.side_weights) {
            let (w, h) = (n.size.x, n.size.y);
            let sides = [(0.0, 0.0, w, t), (w - r, 0.0, r, h), (0.0, h - b, w, b), (0.0, 0.0, l, h)];
            for p in &paints {
                for (x, y, sw, sh) in sides {
                    if sw <= 0.0 || sh <= 0.0 {
                        continue;
                    }
                    let g = rect_geo(x, y, sw, sh);
                    let lines = self.fill_lines(&g, p, n.size);
                    self.emit_lines(lines);
                }
            }
            return;
        }
        let w = st.weight;
        let aligned = geo.closed && geo.fillable;
        let (width, wrapper) = match (st.align, aligned) {
            (StrokeAlign::Inside, true) => {
                let c = self.clip_def(geo);
                (w * 2.0, Some(format!("<g clip-path=\"url(#{c})\">")))
            }
            (StrokeAlign::Outside, true) => {
                let id = self.uid("outside");
                let pad = w * 2.0 + 1.0;
                self.def(&format!(
                    "<mask id=\"{id}\" maskUnits=\"userSpaceOnUse\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\">",
                    num(-pad),
                    num(-pad),
                    num(n.size.x + 2.0 * pad),
                    num(n.size.y + 2.0 * pad)
                ));
                self.def(&format!(
                    "  <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"white\"/>",
                    num(-pad),
                    num(-pad),
                    num(n.size.x + 2.0 * pad),
                    num(n.size.y + 2.0 * pad)
                ));
                for l in self.shape_lines(&Geo { mask: None, ..geo.clone() }, " fill=\"black\"", false) {
                    self.def(&format!("  {l}"));
                }
                self.def("</mask>");
                (w * 2.0, Some(format!("<g mask=\"url(#{id})\">")))
            }
            _ => (w, None),
        };
        if let Some(wr) = &wrapper {
            self.line(wr);
            self.depth += 1;
        }
        for p in paints {
            let Some((val, op)) = self.paint_value(p, n.size) else { continue };
            let mut a = format!(" fill=\"none\" stroke=\"{val}\"");
            if op < 0.999 {
                a.push_str(&format!(" stroke-opacity=\"{}\"", num(op)));
            }
            a.push_str(&format!(" stroke-width=\"{}\"", num(width)));
            match st.cap {
                StrokeCap::None => {}
                StrokeCap::Round => a.push_str(" stroke-linecap=\"round\""),
                StrokeCap::Square => a.push_str(" stroke-linecap=\"square\""),
            }
            match st.join {
                StrokeJoin::Miter => {
                    if st.miter_limit > 0.0 && (st.miter_limit - 4.0).abs() > 1e-4 {
                        a.push_str(&format!(" stroke-miterlimit=\"{}\"", num(st.miter_limit)));
                    }
                }
                StrokeJoin::Bevel => a.push_str(" stroke-linejoin=\"bevel\""),
                StrokeJoin::Round => a.push_str(" stroke-linejoin=\"round\""),
            }
            if st.dashes.iter().any(|d| *d > 0.0) {
                let d: Vec<String> = st.dashes.iter().map(|d| num(*d)).collect();
                a.push_str(&format!(" stroke-dasharray=\"{}\"", d.join(" ")));
            }
            if let Some(b) = blend_css(p.blend) {
                a.push_str(&format!(" style=\"mix-blend-mode:{b}\""));
            }
            let lines = self.shape_lines(geo, &a, false);
            self.emit_lines(lines);
        }
        if wrapper.is_some() {
            self.depth -= 1;
            self.line("</g>");
        }
    }

    // ---- effects ----------------------------------------------------------

    fn blur_filter(&mut self, id: NodeId, radius: f32) -> String {
        let fid = self.uid("blur");
        let b = render_bounds(self.doc, id);
        self.def(&format!(
            "<filter id=\"{fid}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" filterUnits=\"userSpaceOnUse\" color-interpolation-filters=\"sRGB\">",
            num(b.x),
            num(b.y),
            num(b.w),
            num(b.h)
        ));
        self.def(&format!("  <feGaussianBlur stdDeviation=\"{}\"/>", num(radius / 2.0)));
        self.def("</filter>");
        fid
    }

    fn shadow_filter(&mut self, n: &Node) -> String {
        let fid = self.uid("shadow");
        let (l, t, r, b) = own_overflow(n);
        self.def(&format!(
            "<filter id=\"{fid}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" filterUnits=\"userSpaceOnUse\" color-interpolation-filters=\"sRGB\">",
            num(-l),
            num(-t),
            num(n.size.x + l + r),
            num(n.size.y + t + b)
        ));
        let colorize = |c: &Color| format!("0 0 0 0 {} 0 0 0 0 {} 0 0 0 0 {} 0 0 0 {} 0", num(c.r), num(c.g), num(c.b), num(c.a));
        self.def("  <feColorMatrix in=\"SourceAlpha\" type=\"matrix\" values=\"0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 127 0\" result=\"hardAlpha\"/>");
        let mut shadows = Vec::new();
        for (i, e) in n.effects.iter().enumerate() {
            if let Effect::DropShadow { color, offset, radius, spread, visible: true, show_behind, blend } = e {
                let mut input = "hardAlpha".to_string();
                if *spread != 0.0 {
                    let op = if *spread > 0.0 { "dilate" } else { "erode" };
                    self.def(&format!("  <feMorphology in=\"{input}\" operator=\"{op}\" radius=\"{}\" result=\"spread{i}\"/>", num(spread.abs())));
                    input = format!("spread{i}");
                }
                self.def(&format!("  <feOffset in=\"{input}\" dx=\"{}\" dy=\"{}\" result=\"offset{i}\"/>", num(offset.x), num(offset.y)));
                input = format!("offset{i}");
                if *radius > 0.0 {
                    self.def(&format!("  <feGaussianBlur in=\"{input}\" stdDeviation=\"{}\" result=\"blur{i}\"/>", num(radius / 2.0)));
                    input = format!("blur{i}");
                }
                if !show_behind {
                    self.def(&format!("  <feComposite in=\"{input}\" in2=\"hardAlpha\" operator=\"out\" result=\"knockout{i}\"/>"));
                    input = format!("knockout{i}");
                }
                self.def(&format!("  <feColorMatrix in=\"{input}\" type=\"matrix\" values=\"{}\" result=\"shadow{i}\"/>", colorize(color)));
                let shadow_result = if let Some(mode) = blend_css(*blend) {
                    self.def(&format!("  <feBlend mode=\"{mode}\" in=\"shadow{i}\" in2=\"SourceGraphic\" result=\"blendedShadow{i}\"/>"));
                    format!("blendedShadow{i}")
                } else {
                    format!("shadow{i}")
                };
                shadows.push(shadow_result);
            }
        }
        let mut current = "SourceGraphic".to_string();
        if !shadows.is_empty() {
            self.def("  <feMerge result=\"withShadows\">");
            for s in &shadows {
                self.def(&format!("    <feMergeNode in=\"{s}\"/>"));
            }
            self.def("    <feMergeNode in=\"SourceGraphic\"/>");
            self.def("  </feMerge>");
            current = "withShadows".into();
        }
        for (i, e) in n.effects.iter().enumerate() {
            if let Effect::InnerShadow { color, offset, radius, spread, visible: true, blend } = e {
                let mut input = "hardAlpha".to_string();
                if *spread > 0.0 {
                    self.def(&format!("  <feMorphology in=\"{input}\" operator=\"erode\" radius=\"{}\" result=\"inset{i}\"/>", num(*spread)));
                    input = format!("inset{i}");
                }
                self.def(&format!("  <feOffset in=\"{input}\" dx=\"{}\" dy=\"{}\" result=\"innerOffset{i}\"/>", num(offset.x), num(offset.y)));
                input = format!("innerOffset{i}");
                if *radius > 0.0 {
                    self.def(&format!("  <feGaussianBlur in=\"{input}\" stdDeviation=\"{}\" result=\"innerBlur{i}\"/>", num(radius / 2.0)));
                    input = format!("innerBlur{i}");
                }
                self.def(&format!("  <feComposite in=\"hardAlpha\" in2=\"{input}\" operator=\"arithmetic\" k2=\"1\" k3=\"-1\" result=\"innerCut{i}\"/>"));
                self.def(&format!("  <feColorMatrix in=\"innerCut{i}\" type=\"matrix\" values=\"{}\" result=\"innerShadow{i}\"/>", colorize(color)));
                let mode = blend_css(*blend).unwrap_or("normal");
                self.def(&format!("  <feBlend mode=\"{mode}\" in=\"innerShadow{i}\" in2=\"{current}\" result=\"withInner{i}\"/>"));
                current = format!("withInner{i}");
            }
        }
        // Texture: displace the result with fractal noise (feTurbulence + feDisplacementMap).
        for (i, e) in n.effects.iter().enumerate() {
            let Effect::Texture(tx) = e else { continue };
            if !tx.visible || tx.radius <= 0.0 {
                continue;
            }
            let freq = 1.0 / tx.size.max(0.5);
            self.def(&format!(
                "  <feTurbulence type=\"fractalNoise\" baseFrequency=\"{}\" numOctaves=\"2\" seed=\"{}\" result=\"texNoise{i}\"/>",
                num(freq),
                tx.seed
            ));
            self.def(&format!(
                "  <feDisplacementMap in=\"{current}\" in2=\"texNoise{i}\" scale=\"{}\" xChannelSelector=\"R\" yChannelSelector=\"G\" result=\"textured{i}\"/>",
                num(tx.radius * 2.0)
            ));
            current = format!("textured{i}");
            if tx.clip_to_shape {
                self.def(&format!("  <feComposite in=\"{current}\" in2=\"hardAlpha\" operator=\"in\" result=\"texClip{i}\"/>"));
                current = format!("texClip{i}");
            }
        }
        // Noise: thresholded turbulence tinted with the colour, kept inside the shape and drawn over.
        // Duotone is approximated with its first colour.
        for (i, e) in n.effects.iter().enumerate() {
            let Effect::Noise(nz) = e else { continue };
            if !nz.visible {
                continue;
            }
            let freq = 1.0 / nz.size.max(0.5);
            let k = 40.0;
            let off = -k * (1.0 - nz.density.clamp(0.0, 1.0));
            self.def(&format!(
                "  <feTurbulence type=\"fractalNoise\" baseFrequency=\"{}\" numOctaves=\"1\" seed=\"{}\" result=\"grain{i}\"/>",
                num(freq),
                nz.seed
            ));
            let matrix = match &nz.mode {
                NoiseMode::Mono { color } | NoiseMode::Duo { color, .. } => {
                    format!("0 0 0 0 {} 0 0 0 0 {} 0 0 0 0 {} {} 0 0 0 {}", num(color.r), num(color.g), num(color.b), num(k), num(off))
                }
                NoiseMode::Multi => format!("1 0 0 0 0 0 1 0 0 0 0 0 1 0 0 {} 0 0 0 {}", num(k), num(off)),
            };
            self.def(&format!("  <feColorMatrix in=\"grain{i}\" type=\"matrix\" values=\"{matrix}\" result=\"grainTint{i}\"/>"));
            self.def(&format!(
                "  <feComponentTransfer in=\"grainTint{i}\" result=\"grainFade{i}\"><feFuncA type=\"linear\" slope=\"{}\"/></feComponentTransfer>",
                num(nz.opacity.clamp(0.0, 1.0))
            ));
            self.def(&format!("  <feComposite in=\"grainFade{i}\" in2=\"hardAlpha\" operator=\"in\" result=\"grainClip{i}\"/>"));
            self.def(&format!(
                "  <feBlend mode=\"{}\" in=\"grainClip{i}\" in2=\"{current}\" result=\"withGrain{i}\"/>",
                blend_css(nz.blend).unwrap_or("normal")
            ));
            current = format!("withGrain{i}");
        }
        let _ = current;
        self.def("</filter>");
        fid
    }

    // ---- boolean operations -------------------------------------------------

    /// Flattens union/exclude operands into path commands in `acc` space.
    fn collect_operands(&self, n: &Node, acc: &Affine, out: &mut Vec<PathCmd>) {
        let doc = self.doc;
        for cid in &n.children {
            let Some(c) = doc.get(*cid) else { continue };
            if !c.visible {
                continue;
            }
            let t = *acc * c.transform;
            if let NodeKind::BooleanOp(_) = c.kind {
                self.collect_operands(c, &t, out);
            } else if let Some(g) = node_geo(c).filter(|g| g.fillable) {
                for (cmds, _) in &g.paths {
                    out.extend(transform_cmds(cmds, &t));
                }
            } else if matches!(c.kind, NodeKind::Group) {
                self.collect_operands(c, &t, out);
            }
        }
    }

    /// One boolean operand (with its transform) as a filled path element.
    fn operand_shape(&self, parent: &Node, c: &Node, fill: &str) -> String {
        let mut cmds = Vec::new();
        let mut tmp = parent.clone();
        tmp.children = vec![c.id];
        self.collect_operands(&tmp, &Affine::IDENTITY, &mut cmds);
        format!("<path d=\"{}\" fill=\"{fill}\"/>", path_d(&cmds))
    }

    fn bool_geo(&mut self, n: &Node, op: BooleanOp) -> Option<Geo> {
        if let Some(fg) = n.fill_geometry.as_deref().filter(|g| !g.is_empty()) {
            return node_geo(&Node { kind: NodeKind::Rectangle, fill_geometry: Some(fg.to_vec()), ..n.clone() });
        }
        let doc = self.doc;
        match op {
            BooleanOp::Union | BooleanOp::Exclude => {
                let mut cmds = Vec::new();
                self.collect_operands(n, &Affine::IDENTITY, &mut cmds);
                if cmds.is_empty() {
                    return None;
                }
                let rule = if op == BooleanOp::Exclude { FillRule::EvenOdd } else { FillRule::NonZero };
                Some(Geo { kind: GeoKind::Path, paths: vec![(cmds, rule)], closed: true, fillable: true, is_rect: false, mask: None })
            }
            BooleanOp::Subtract | BooleanOp::Intersect => {
                let operands: Vec<&Node> = n.children.iter().filter_map(|c| doc.get(*c)).filter(|c| c.visible).collect();
                if operands.is_empty() {
                    return None;
                }
                let region = render_bounds(doc, n.id);
                let open = |s: &mut Self, id: &str, inner: &str| {
                    let pad = 2.0;
                    s.def(&format!(
                        "<mask id=\"{id}\" maskUnits=\"userSpaceOnUse\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\">",
                        num(region.x - pad),
                        num(region.y - pad),
                        num(region.w + 2.0 * pad),
                        num(region.h + 2.0 * pad)
                    ));
                    s.defs.push_str(inner);
                    s.def("</mask>");
                };
                let mask_id;
                if op == BooleanOp::Subtract {
                    mask_id = self.uid("subtract");
                    let mut inner = String::new();
                    for (i, c) in operands.iter().enumerate() {
                        inner.push_str(&format!("      {}\n", self.operand_shape(n, c, if i == 0 { "white" } else { "black" })));
                    }
                    open(self, &mask_id, &inner);
                } else {
                    // chain: each operand is clipped by the mask of the previous ones
                    let mut prev: Option<String> = None;
                    for c in &operands {
                        let id = self.uid("intersect");
                        let shape = self.operand_shape(n, c, "white");
                        let inner = match &prev {
                            Some(p) => format!("      <g mask=\"url(#{p})\">\n        {shape}\n      </g>\n"),
                            None => format!("      {shape}\n"),
                        };
                        open(self, &id, &inner);
                        prev = Some(id);
                    }
                    mask_id = prev?;
                }
                let mut g = rect_geo(0.0, 0.0, n.size.x, n.size.y);
                g.mask = Some(format!("mask=\"url(#{mask_id})\""));
                Some(g)
            }
        }
    }

    // ---- text ---------------------------------------------------------------

    fn text_node(&mut self, n: &Node, t: &TextProps) {
        let out = text::layout(t, n.size);
        if self.opts.outline_text {
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
                for p in visible_paints(&style.fills) {
                    let lines = self.fill_lines(&geo, p, n.size);
                    self.emit_lines(lines);
                }
            }
            return;
        }
        let style_attrs = |s: &mut Self, st: &TextStyle, full: bool, base: &TextStyle| -> String {
            let mut a = String::new();
            if full || st.font_family != base.font_family {
                a.push_str(&format!(" font-family=\"{}\"", esc(&st.font_family)));
            }
            if full || st.font_size != base.font_size {
                a.push_str(&format!(" font-size=\"{}\"", num(st.font_size)));
            }
            if (full && st.font_weight != 400) || (!full && st.font_weight != base.font_weight) {
                a.push_str(&format!(" font-weight=\"{}\"", st.font_weight));
            }
            if (full && st.italic) || (!full && st.italic != base.italic) {
                a.push_str(&format!(" font-style=\"{}\"", if st.italic { "italic" } else { "normal" }));
            }
            if (full && st.letter_spacing != 0.0) || (!full && st.letter_spacing != base.letter_spacing) {
                a.push_str(&format!(" letter-spacing=\"{}\"", num(st.letter_spacing)));
            }
            if (full && st.decoration != TextDecoration::None) || (!full && st.decoration != base.decoration) {
                a.push_str(match st.decoration {
                    TextDecoration::None => " text-decoration=\"none\"",
                    TextDecoration::Underline => " text-decoration=\"underline\"",
                    TextDecoration::Strikethrough => " text-decoration=\"line-through\"",
                });
            }
            if let Some(p) = visible_paints(&st.fills).next() {
                if full || st.fills != base.fills {
                    if let Some((val, op)) = s.paint_value(p, n.size) {
                        a.push_str(&format!(" fill=\"{val}\""));
                        if op < 0.999 {
                            a.push_str(&format!(" fill-opacity=\"{}\"", num(op)));
                        }
                    }
                }
            }
            a
        };
        let base = out.styles[0].clone();
        let head = style_attrs(self, &base, true, &base);
        self.line(&format!("<text xml:space=\"preserve\"{head}>"));
        self.depth += 1;
        for line in &out.lines {
            for span in &line.spans {
                let extra = if span.style == 0 { String::new() } else { style_attrs(self, &out.styles[span.style], false, &base) };
                self.line(&format!("<tspan x=\"{}\" y=\"{}\"{extra}>{}</tspan>", num(span.x), num(line.baseline), esc(span.text.trim_end_matches('\n'))));
            }
        }
        self.depth -= 1;
        self.line("</text>");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc1(f: impl FnOnce(&mut Document, NodeId)) -> (Document, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        f(&mut d, page);
        let id = *d.get(page).unwrap().children.first().unwrap();
        (d, id)
    }

    fn svg(d: &Document, id: NodeId) -> String {
        export_svg(d, id, &SvgOptions { outline_text: false })
    }

    const HEAD: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\"";

    #[test]
    fn golden_rect_with_radius_and_opacity() {
        let (d, id) = doc1(|d, page| {
            let i = d.alloc_id();
            let mut n = Node::rectangle(i, Rect::new(10.0, 20.0, 100.0, 50.0));
            n.name = "Card".into();
            n.corner_radii = [8.0; 4];
            n.opacity = 0.5;
            d.insert(page, n).unwrap();
        });
        assert_eq!(
            svg(&d, id),
            format!(
                "{HEAD} width=\"100\" height=\"50\" viewBox=\"0 0 100 50\" fill=\"none\">\n  <g data-name=\"Card\" opacity=\"0.5\">\n    <rect width=\"100\" height=\"50\" rx=\"8\" fill=\"#D9D9D9\"/>\n  </g>\n</svg>\n"
            )
        );
    }

    #[test]
    fn golden_independent_radii_become_a_path() {
        let (d, id) = doc1(|d, page| {
            let i = d.alloc_id();
            let mut n = Node::rectangle(i, Rect::new(0.0, 0.0, 20.0, 20.0));
            n.corner_radii = [10.0, 0.0, 0.0, 0.0];
            d.insert(page, n).unwrap();
        });
        let s = svg(&d, id);
        assert!(s.contains("<path d=\"M10 0 L20 0 L20 20 L0 20 L0 10 C0 4.477 4.477 0 10 0 Z\" fill=\"#D9D9D9\"/>"), "{s}");
    }

    #[test]
    fn golden_ellipse_and_linear_gradient() {
        let (d, id) = doc1(|d, page| {
            let i = d.alloc_id();
            let mut n = Node::ellipse(i, Rect::new(0.0, 0.0, 40.0, 20.0));
            n.fills = vec![Paint {
                kind: PaintKind::LinearGradient {
                    handles: [Vec2::new(0.0, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.0, 1.0)],
                    stops: vec![GradientStop { pos: 0.0, color: Color::rgb(255, 0, 0) }, GradientStop { pos: 1.0, color: Color::rgba(0, 0, 255, 128) }],
                },
                opacity: 1.0,
                visible: true,
                blend: BlendMode::Normal,
            }];
            d.insert(page, n).unwrap();
        });
        let s = svg(&d, id);
        assert!(s.contains("<ellipse cx=\"20\" cy=\"10\" rx=\"20\" ry=\"10\" fill=\"url(#linear0)\"/>"), "{s}");
        // basis: start (0,10), end (40,10), width handle (0,20)
        assert!(s.contains("gradientTransform=\"matrix(40 0 0 10 0 10)\""), "{s}");
        assert!(s.contains("<stop offset=\"0\" stop-color=\"#FF0000\"/>"), "{s}");
        assert!(s.contains("<stop offset=\"1\" stop-color=\"#0000FF\" stop-opacity=\"0.502\"/>"), "{s}");
    }

    #[test]
    fn golden_inside_stroke_uses_clip_and_double_width() {
        let (d, id) = doc1(|d, page| {
            let i = d.alloc_id();
            let mut n = Node::rectangle(i, Rect::new(0.0, 0.0, 10.0, 10.0));
            n.fills.clear();
            n.stroke.paints = vec![Paint::solid(Color::BLACK)];
            n.stroke.weight = 2.0;
            n.stroke.align = StrokeAlign::Inside;
            n.stroke.dashes = vec![4.0, 2.0];
            d.insert(page, n).unwrap();
        });
        let s = svg(&d, id);
        assert!(s.contains("<clipPath id=\"clip0\">"), "{s}");
        assert!(s.contains("<g clip-path=\"url(#clip0)\">"), "{s}");
        assert!(s.contains("fill=\"none\" stroke=\"#000000\" stroke-width=\"4\" stroke-dasharray=\"4 2\""), "{s}");
    }

    #[test]
    fn outside_stroke_grows_viewbox_and_masks() {
        let (d, id) = doc1(|d, page| {
            let i = d.alloc_id();
            let mut n = Node::rectangle(i, Rect::new(0.0, 0.0, 10.0, 10.0));
            n.stroke.paints = vec![Paint::solid(Color::BLACK)];
            n.stroke.weight = 3.0;
            n.stroke.align = StrokeAlign::Outside;
            d.insert(page, n).unwrap();
        });
        let s = svg(&d, id);
        assert!(s.contains("viewBox=\"-3 -3 16 16\""), "{s}");
        assert!(s.contains("<mask id=\"outside0\""), "{s}");
        assert!(s.contains("stroke-width=\"6\""), "{s}");
    }

    #[test]
    fn golden_shadow_filter_and_blur() {
        let (d, id) = doc1(|d, page| {
            let i = d.alloc_id();
            let mut n = Node::rectangle(i, Rect::new(0.0, 0.0, 10.0, 10.0));
            n.effects = vec![
                Effect::DropShadow {
                    color: Color::rgba(0, 0, 0, 64),
                    offset: Vec2::new(0.0, 4.0),
                    radius: 4.0,
                    spread: 0.0,
                    visible: true,
                    show_behind: false,
                    blend: Default::default(),
                },
                Effect::InnerShadow { color: Color::WHITE, offset: Vec2::new(1.0, 1.0), radius: 2.0, spread: 0.0, visible: true, blend: Default::default() },
                Effect::LayerBlur { radius: 6.0, visible: true },
            ];
            d.insert(page, n).unwrap();
        });
        let s = svg(&d, id);
        assert!(s.contains("filter=\"url(#blur0)\""), "{s}");
        assert!(s.contains("<feGaussianBlur stdDeviation=\"3\"/>"), "{s}");
        assert!(s.contains("<feOffset in=\"hardAlpha\" dx=\"0\" dy=\"4\" result=\"offset0\"/>"), "{s}");
        assert!(s.contains("<feComposite in=\"blur0\" in2=\"hardAlpha\" operator=\"out\""), "{s}");
        assert!(s.contains("operator=\"arithmetic\" k2=\"1\" k3=\"-1\""), "{s}");
        assert!(s.contains("<g filter=\"url(#shadow1)\">"), "{s}");
    }

    #[test]
    fn noise_and_texture_become_filter_primitives_and_glass_is_skipped() {
        let (d, id) = doc1(|d, page| {
            let i = d.alloc_id();
            let mut n = Node::rectangle(i, Rect::new(0.0, 0.0, 40.0, 40.0));
            n.effects = vec![
                Effect::Noise(NoiseEffect {
                    mode: NoiseMode::Mono { color: Color::BLACK },
                    size: 2.0,
                    density: 0.5,
                    opacity: 0.25,
                    blend: BlendMode::Normal,
                    seed: 3,
                    visible: true,
                }),
                Effect::Texture(TextureEffect { size: 4.0, radius: 3.0, clip_to_shape: true, seed: 5, visible: true }),
                Effect::Glass(GlassEffect { light_angle: 0.0, light_intensity: 0.5, refraction: 0.5, depth: 8.0, dispersion: 0.0, frost: 4.0, visible: true }),
            ];
            d.insert(page, n).unwrap();
        });
        let s = svg(&d, id);
        assert!(s.contains("<feTurbulence type=\"fractalNoise\" baseFrequency=\"0.5\" numOctaves=\"1\" seed=\"3\" result=\"grain0\"/>"), "{s}");
        assert!(s.contains("<feDisplacementMap in=\"SourceGraphic\" in2=\"texNoise1\" scale=\"6\""), "{s}");
        assert!(s.contains("operator=\"in\" result=\"texClip1\""), "{s}");
        assert!(!s.to_lowercase().contains("glass"), "{s}");
    }

    #[test]
    fn frame_clips_children_and_groups_nest() {
        let (d, id) = doc1(|d, page| {
            let f = d.alloc_id();
            let mut fr = Node::frame(f, Rect::new(0.0, 0.0, 50.0, 50.0));
            if let NodeKind::Frame(fp) = &mut fr.kind {
                fp.clips_content = true;
            }
            let fr = d.insert(page, fr).unwrap();
            let r = d.alloc_id();
            let mut rect = Node::rectangle(r, Rect::new(5.0, 6.0, 20.0, 20.0));
            rect.blend = BlendMode::Multiply;
            d.insert(fr, rect).unwrap();
        });
        let s = svg(&d, id);
        assert!(s.contains("<g clip-path=\"url(#clip0)\">"), "{s}");
        assert!(s.contains("<g data-name=\"Rectangle\" transform=\"translate(5 6)\" style=\"mix-blend-mode:multiply\">"), "{s}");
    }

    #[test]
    fn text_as_text_element_and_as_outlines() {
        let (d, id) = doc1(|d, page| {
            let i = d.alloc_id();
            let style = TextStyle { font_size: 16.0, font_weight: 700, line_height: LineHeight::Pixels(20.0), ..Default::default() };
            let n = Node::text(i, Rect::new(0.0, 0.0, 200.0, 20.0), "Hi & <you>", style);
            d.insert(page, n).unwrap();
        });
        let s = svg(&d, id);
        assert!(s.contains("<text xml:space=\"preserve\" font-family=\"Inter\" font-size=\"16\" font-weight=\"700\" fill=\"#000000\">"), "{s}");
        assert!(s.contains(">Hi &amp; &lt;you&gt;</tspan>"), "{s}");
        let o = export_svg(&d, id, &SvgOptions::default());
        assert!(!o.contains("<text"), "{o}");
        assert!(o.contains("<path d=\"M"), "{o}");
    }

    #[test]
    fn star_vector_evenodd_and_image_fill() {
        let (d, id) = doc1(|d, page| {
            let i = d.alloc_id();
            let mut n = Node::new(
                i,
                NodeKind::Vector {
                    paths: vec![Path {
                        cmds: vec![
                            PathCmd::MoveTo(Vec2::new(0.0, 0.0)),
                            PathCmd::LineTo(Vec2::new(8.0, 0.0)),
                            PathCmd::LineTo(Vec2::new(8.0, 8.0)),
                            PathCmd::Close,
                        ],
                        fill_rule: FillRule::EvenOdd,
                    }],
                    network: None,
                },
            );
            n.size = Vec2::new(8.0, 8.0);
            n.fills = vec![Paint::solid(Color::rgb(1, 2, 3))];
            d.insert(page, n).unwrap();
            let j = d.alloc_id();
            let mut s = Node::new(j, NodeKind::Star { count: 5, inner_ratio: 0.5 });
            s.size = Vec2::new(10.0, 10.0);
            d.insert(page, s).unwrap();
        });
        let s = svg(&d, id);
        assert!(s.contains("<path d=\"M0 0 L8 0 L8 8 Z\" fill-rule=\"evenodd\" fill=\"#010203\"/>"), "{s}");
    }

    #[test]
    fn image_fill_is_embedded_as_base64_pattern() {
        let mut png = Vec::new();
        {
            let mut img = image::RgbaImage::new(4, 2);
            for p in img.pixels_mut() {
                *p = image::Rgba([255, 0, 0, 255]);
            }
            image::DynamicImage::ImageRgba8(img).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        }
        let (d, id) = doc1(|d, page| {
            d.images.insert("h1".into(), png.clone());
            let i = d.alloc_id();
            let mut n = Node::rectangle(i, Rect::new(0.0, 0.0, 8.0, 8.0));
            n.fills = vec![Paint {
                kind: PaintKind::Image { image_ref: "h1".into(), scale_mode: ImageScaleMode::Fill },
                opacity: 1.0,
                visible: true,
                blend: BlendMode::Normal,
            }];
            d.insert(page, n).unwrap();
        });
        let s = svg(&d, id);
        assert!(s.contains("xlink:href=\"data:image/png;base64,"), "{s}");
        assert!(s.contains("<pattern id=\"pattern1\""), "{s}");
        // cover scale: max(8/4, 8/2) = 4, centred horizontally: (8 - 16) / 2
        assert!(s.contains("matrix(4 0 0 4 -4 0)"), "{s}");
        assert!(s.contains("fill=\"url(#pattern1)\""), "{s}");
    }

    #[test]
    fn mask_node_masks_following_siblings() {
        let (d, id) = doc1(|d, page| {
            let f = d.alloc_id();
            let f = d.insert(page, Node::frame(f, Rect::new(0.0, 0.0, 50.0, 50.0))).unwrap();
            let m = d.alloc_id();
            let mut mask = Node::ellipse(m, Rect::new(0.0, 0.0, 30.0, 30.0));
            mask.is_mask = true;
            d.insert(f, mask).unwrap();
            let r = d.alloc_id();
            d.insert(f, Node::rectangle(r, Rect::new(0.0, 0.0, 50.0, 50.0))).unwrap();
        });
        let s = svg(&d, id);
        assert!(s.contains("<mask id=\"mask0\" style=\"mask-type:alpha\""), "{s}");
        assert!(s.contains("<g mask=\"url(#mask0)\">"), "{s}");
    }

    #[test]
    fn union_boolean_joins_operands() {
        let (d, id) = doc1(|d, page| {
            let b = d.alloc_id();
            let mut op = Node::new(b, NodeKind::BooleanOp(BooleanOp::Union));
            op.size = Vec2::new(30.0, 10.0);
            op.fills = vec![Paint::solid(Color::BLACK)];
            let op = d.insert(page, op).unwrap();
            let a = d.alloc_id();
            d.insert(op, Node::rectangle(a, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
            let c = d.alloc_id();
            d.insert(op, Node::rectangle(c, Rect::new(20.0, 0.0, 10.0, 10.0))).unwrap();
        });
        let s = svg(&d, id);
        assert!(s.contains("M0 0 L10 0 L10 10 L0 10 Z M20 0 L30 0 L30 10 L20 10 Z"), "{s}");
        let (d2, id2) = doc1(|d, page| {
            let b = d.alloc_id();
            let mut op = Node::new(b, NodeKind::BooleanOp(BooleanOp::Subtract));
            op.size = Vec2::new(30.0, 10.0);
            op.fills = vec![Paint::solid(Color::BLACK)];
            let op = d.insert(page, op).unwrap();
            let a = d.alloc_id();
            d.insert(op, Node::rectangle(a, Rect::new(0.0, 0.0, 30.0, 10.0))).unwrap();
            let c = d.alloc_id();
            d.insert(op, Node::rectangle(c, Rect::new(20.0, 0.0, 10.0, 10.0))).unwrap();
        });
        let s2 = svg(&d2, id2);
        assert!(s2.contains("<mask id=\"subtract0\""), "{s2}");
        assert!(s2.contains("fill=\"black\""), "{s2}");
        assert!(s2.contains("mask=\"url(#subtract0)\""), "{s2}");
    }
}
