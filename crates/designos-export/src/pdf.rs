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

mod fidelity;
mod raster_page;
mod shading;
mod text_layer;
mod vector_page;

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
        let extractor = std::process::Command::new("pdftotext").arg(&tmp).arg("-").output();
        let _ = std::fs::remove_file(&tmp);
        if let Ok(output) = extractor {
            assert!(output.status.success(), "pdftotext failed: {}", String::from_utf8_lossy(&output.stderr));
            let extracted = String::from_utf8_lossy(&output.stdout);
            assert!(extracted.contains("First slide headline"), "page 1 text missing from pdftotext output: {extracted:?}");
            assert!(extracted.contains("Second slide takeaway"), "page 2 text missing from pdftotext output: {extracted:?}");
        }
    }
}
