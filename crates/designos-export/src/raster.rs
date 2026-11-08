//! PNG / JPG export through `designos_render`.
//!
//! Figma grows the exported image to include effects that spill outside the
//! layer (shadows, blur, outside strokes). `render_node` crops to the layer
//! box, so when the expanded bounds differ the subtree is copied into a
//! scratch document positioned at the expanded origin and drawn with
//! `render_page` instead.

use crate::bounds::try_render_bounds;
use designos_doc::{Affine, Color, Document, Node, NodeId, NodeKind, PaintKind, Rect, Vec2};
use designos_render::{checked_pixmap_size, render_node, render_node_native, render_page, render_page_native, Viewport};
use std::collections::BTreeSet;
use tiny_skia::Pixmap;

/// How an export row sizes its output (Figma's "1x", "2x", "512w", "512h").
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ExportScale {
    Factor(f32),
    /// Scale so the layer is this many px wide.
    Width(u32),
    /// Scale so the layer is this many px tall.
    Height(u32),
}

impl ExportScale {
    pub const PRESETS: [ExportScale; 7] = [
        ExportScale::Factor(0.5),
        ExportScale::Factor(0.75),
        ExportScale::Factor(1.0),
        ExportScale::Factor(1.5),
        ExportScale::Factor(2.0),
        ExportScale::Factor(3.0),
        ExportScale::Factor(4.0),
    ];

    /// Parses `2x`, `0.5x`, `512w`, `512h` (case-insensitive, whitespace ignored).
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().to_ascii_lowercase();
        let (num, unit) = s.split_at(s.len().checked_sub(1)?);
        match unit {
            "x" => {
                let v: f32 = num.trim().parse().ok()?;
                (v > 0.0 && v <= 32.0).then_some(Self::Factor(v))
            }
            "w" | "h" => {
                let v: u32 = num.trim().parse().ok()?;
                (v > 0 && v <= 32_000).then_some(if unit == "w" { Self::Width(v) } else { Self::Height(v) })
            }
            _ => None,
        }
    }

    /// Multiplier for a layer of `size`.
    pub fn factor(&self, size: Vec2) -> f32 {
        match *self {
            Self::Factor(f) => f,
            Self::Width(w) => w as f32 / size.x.max(1.0),
            Self::Height(h) => h as f32 / size.y.max(1.0),
        }
    }

    /// File name suffix: empty for 1x, else `@2x`, `@512w`.
    pub fn suffix(&self) -> String {
        match *self {
            Self::Factor(f) if (f - 1.0).abs() < 1e-6 => String::new(),
            _ => format!("@{self}"),
        }
    }
}

impl std::fmt::Display for ExportScale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::Factor(v) => write!(f, "{}x", crate::util::num(v)),
            Self::Width(w) => write!(f, "{w}w"),
            Self::Height(h) => write!(f, "{h}h"),
        }
    }
}

/// Renders `node` at `scale`, including effect overflow. Transparent background. A Display P3
/// document is converted to sRGB, so the pixels suit any sRGB consumer.
pub fn render_export(doc: &Document, node: NodeId, scale: f32) -> Option<Pixmap> {
    render_export_in(doc, node, scale, false)
}

/// [`render_export`] whose pixels are in the document's own colour profile (no conversion): the
/// file written from them must carry that profile's tag, as [`export_png`] does.
fn render_export_native(doc: &Document, node: NodeId, scale: f32) -> Option<Pixmap> {
    render_export_in(doc, node, scale, true)
}

fn render_export_in(doc: &Document, node: NodeId, scale: f32, native: bool) -> Option<Pixmap> {
    let draw_node = |d: &Document, n: NodeId, sc: f32| if native { render_node_native(d, n, sc) } else { render_node(d, n, sc) };
    let n = designos_doc::resolve_node(doc, doc.get(node)?);
    let scale = scale.clamp(0.01, 32.0);
    let is_page = matches!(n.kind, NodeKind::Page { .. });
    let is_group = matches!(n.kind, NodeKind::Group);
    let b = try_render_bounds(doc, node)?;
    let nominal = Rect::new(0.0, 0.0, n.size.x, n.size.y);
    if !is_page && !is_group && b == nominal {
        return draw_node(doc, node, scale);
    }
    render_bounded(doc, node, scale, b, native)
}

/// Draws `page` of `d` into `pm`, in the document's own colour profile when `native`, else in sRGB.
fn draw_page(native: bool, d: &Document, page: NodeId, vp: &Viewport, pm: &mut Pixmap) {
    if native {
        render_page_native(d, page, vp, pm)
    } else {
        render_page(d, page, vp, pm)
    }
}

/// Renders `node` at `scale` into the local-space rectangle `b`, which may reach outside its box.
/// `native` keeps the document's own colour profile; otherwise the pixels are converted to sRGB.
pub(crate) fn render_bounded(doc: &Document, node: NodeId, scale: f32, b: Rect, native: bool) -> Option<Pixmap> {
    let n = designos_doc::resolve_node(doc, doc.get(node)?);
    let is_page = matches!(n.kind, NodeKind::Page { .. });
    let (w, h) = checked_pixmap_size(b.w * scale, b.h * scale)?;
    let mut pm = Pixmap::new(w, h)?;
    let vp = Viewport { offset: Vec2::new(0.0, 0.0), zoom: scale, dpr: 1.0 };
    if is_page {
        let vp = Viewport { offset: Vec2::new(b.x, b.y), ..vp };
        draw_page(native, doc, node, &vp, &mut pm);
        return Some(pm);
    }
    let (scratch, page) = scratch_doc(doc, node, Vec2::new(-b.x, -b.y));
    draw_page(native, &scratch, page, &vp, &mut pm);
    Some(pm)
}

/// Copies the subtree of `root` under a transparent page, placing the root at `at`,
/// resolving bound variables per node in the subtree without cloning the whole document.
fn scratch_doc(doc: &Document, root: NodeId, at: Vec2) -> (Document, NodeId) {
    let mut out = Document::new(doc.name.clone());
    out.color_profile = doc.color_profile;
    if let Some(max) = doc.nodes.keys().max() {
        out.reserve_ids(*max);
    }
    let page = out.add_page("export");
    if let Some(p) = out.get_mut(page) {
        p.kind = NodeKind::Page { background: Color::TRANSPARENT };
    }
    let mut refs = BTreeSet::new();
    let mut nodes: Vec<Node> = Vec::new();
    doc.walk(root, |n, _| {
        let mut resolved = designos_doc::resolve_node(doc, n).into_owned();
        resolved.variables.bindings.clear();
        collect_images(&resolved, &mut refs);
        nodes.push(resolved);
    });
    for (i, mut n) in nodes.into_iter().enumerate() {
        let parent = if i == 0 {
            n.transform = Affine::translate(at.x, at.y);
            page
        } else {
            n.parent.unwrap_or(page)
        };
        n.children.clear();
        let _ = out.insert(parent, n);
    }
    for r in refs {
        if let Some(data) = doc.images.get(&r) {
            out.images.insert(r, data.clone());
        }
    }
    (out, page)
}

/// Adds the `image_ref` of every image paint `n` draws (fills, strokes, text and run fills) to `out`.
pub(crate) fn collect_images(n: &Node, out: &mut BTreeSet<String>) {
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

/// Un-premultiplied RGBA bytes of a pixmap.
pub fn rgba_bytes(pm: &Pixmap) -> Vec<u8> {
    let mut out = Vec::with_capacity(pm.data().len());
    for px in pm.data().chunks_exact(4) {
        let a = px[3] as u32;
        if a == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            let un = |c: u8| ((c as u32 * 255 + a / 2) / a).min(255) as u8;
            out.extend_from_slice(&[un(px[0]), un(px[1]), un(px[2]), a as u8]);
        }
    }
    out
}

/// Encodes `pm` as PNG bytes.
pub fn encode_png_pixmap(pm: &Pixmap) -> Vec<u8> {
    pm.encode_png().unwrap_or_default()
}

/// Encodes `pm` as JPEG bytes at `quality` (1..=100), flattening transparency onto white.
pub fn encode_jpg_pixmap(pm: &Pixmap, quality: u8) -> Vec<u8> {
    let mut rgb = Vec::with_capacity((pm.width() * pm.height() * 3) as usize);
    for px in pm.data().chunks_exact(4) {
        // premultiplied over white: c + (255 - a)
        let inv = 255 - px[3] as u32;
        for c in &px[..3] {
            rgb.push((*c as u32 + inv).min(255) as u8);
        }
    }
    let mut out = Vec::new();
    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality.clamp(1, 100));
    match image::ImageEncoder::write_image(enc, &rgb, pm.width(), pm.height(), image::ExtendedColorType::Rgb8) {
        Ok(()) => out,
        Err(_) => Vec::new(),
    }
}

/// PNG bytes of `node` at `scale` (1.0 = 100%). A Display P3 document is written in Display P3 and
/// tagged as such; an sRGB document is written as it always was.
pub fn export_png(doc: &Document, node: NodeId, scale: f32) -> Vec<u8> {
    let Some(pm) = render_export_native(doc, node, scale) else { return Vec::new() };
    crate::png_profile::tag_png(encode_png_pixmap(&pm), doc.color_profile)
}

/// PNG bytes of `node` at `scale` with the colours as the canvas shows them: a Display P3 document is
/// converted to sRGB and the file carries no profile tag. For previews inside the app and for images
/// handed to readers that ignore tags (decoders such as `image`, vision models).
pub fn export_png_srgb(doc: &Document, node: NodeId, scale: f32) -> Vec<u8> {
    match render_export(doc, node, scale) {
        Some(pm) => encode_png_pixmap(&pm),
        None => Vec::new(),
    }
}

/// JPEG bytes of `node` at `scale`; transparency is flattened onto white.
/// `quality` is 1..=100.
pub fn export_jpg(doc: &Document, node: NodeId, scale: f32, quality: u8) -> Vec<u8> {
    let Some(pm) = render_export(doc, node, scale) else { return Vec::new() };
    encode_jpg_pixmap(&pm, quality)
}

/// Lossless WebP bytes of `node` at `scale` (1.0 = 100%), preserving RGBA pixels identically to PNG.
pub fn export_webp(doc: &Document, node: NodeId, scale: f32) -> Vec<u8> {
    let Some(pm) = render_export(doc, node, scale) else { return Vec::new() };
    let rgba = rgba_bytes(&pm);
    let mut out = Vec::new();
    let enc = image::codecs::webp::WebPEncoder::new_lossless(&mut out);
    match image::ImageEncoder::write_image(enc, &rgba, pm.width(), pm.height(), image::ExtendedColorType::Rgba8) {
        Ok(()) => out,
        Err(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_parse_and_display() {
        assert_eq!(ExportScale::parse("2x"), Some(ExportScale::Factor(2.0)));
        assert_eq!(ExportScale::parse(" 0.75X "), Some(ExportScale::Factor(0.75)));
        assert_eq!(ExportScale::parse("512w"), Some(ExportScale::Width(512)));
        assert_eq!(ExportScale::parse("64H"), Some(ExportScale::Height(64)));
        assert_eq!(ExportScale::parse("abc"), None);
        assert_eq!(ExportScale::parse("0x"), None);
        assert_eq!(ExportScale::Factor(1.5).to_string(), "1.5x");
        assert_eq!(ExportScale::Width(512).suffix(), "@512w");
        assert_eq!(ExportScale::Factor(1.0).suffix(), "");
        assert_eq!(ExportScale::Width(200).factor(Vec2::new(100.0, 50.0)), 2.0);
    }
}
