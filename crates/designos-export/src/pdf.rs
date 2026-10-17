//! Bounded, lossless raster PDF export. Pages are clipped to frame bounds and
//! flatten transparency over white. Each design pixel maps to 0.75 PDF points
//! (96 CSS pixels per inch); the image is rendered at up to twice that resolution.
//! Text and vectors are rasterized, so this first PDF implementation does not
//! provide selectable text or vector paths.

use designos_doc::{Document, NodeId, NodeKind};
use flate2::{write::ZlibEncoder, Compression};
use std::io::Write;

const MAX_PAGES: usize = 500;
const MAX_SIDE: f32 = 4096.0;
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
        let mut body = format!("<< {dictionary} /Length {} >>\nstream\n", content.len()).into_bytes();
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

/// Export one PDF page for each supplied frame, preserving the supplied order.
/// Empty lists, non-frame nodes, invalid dimensions, and oversized exports return
/// an error. Transparency is composited over white; notes are not included.
pub fn export_pdf(doc: &Document, slides: &[NodeId]) -> Result<Vec<u8>, String> {
    if slides.is_empty() {
        return Err("There are no frames to export.".into());
    }
    if slides.len() > MAX_PAGES {
        return Err(format!("Export at most {MAX_PAGES} frames at a time."));
    }
    // Validate the complete selection before spending time rendering any page.
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
    let kids = (0..slides.len()).map(|i| format!("{} 0 R", 3 + i * 3)).collect::<Vec<_>>().join(" ");
    writer.object(2, format!("<< /Type /Pages /Count {} /Kids [{kids}] >>", slides.len()).as_bytes())?;
    for (index, id) in slides.iter().enumerate() {
        let node = doc.get(*id).expect("validated slide");
        let scale = 2.0_f32.min(MAX_SIDE / node.size.x).min(MAX_SIDE / node.size.y);
        let pixmap = designos_render::render_node(doc, *id, scale).ok_or("Cannot render this slide for PDF export.")?;
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
        let image = compressor.finish().map_err(|e| format!("Cannot finish PDF image: {e}"))?;
        let page = 3 + index * 3;
        let width = node.size.x * POINTS_PER_PIXEL;
        let height = node.size.y * POINTS_PER_PIXEL;
        writer.object(
            page,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] /Resources << /XObject << /Slide {} 0 R >> >> /Contents {} 0 R >>",
                page + 1,
                page + 2
            )
            .as_bytes(),
        )?;
        writer.stream(
            page + 1,
            &format!(
                "/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode",
                pixmap.width(),
                pixmap.height()
            ),
            &image,
        )?;
        writer.stream(page + 2, "", format!("q\n{width} 0 0 {height} 0 0 cm\n/Slide Do\nQ\n").as_bytes())?;
    }
    writer.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Color, Node, Paint, Rect};
    use std::io::Read;

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

    fn image_pixels(object: &[u8]) -> Vec<u8> {
        let marker = b"stream\n";
        let at = object.windows(marker.len()).position(|window| window == marker).unwrap() + marker.len();
        let header = std::str::from_utf8(&object[..at]).unwrap();
        let length: usize = header.split("/Length ").nth(1).unwrap().split_whitespace().next().unwrap().parse().unwrap();
        assert!(object[at + length..].starts_with(b"\nendstream"));
        let mut pixels = Vec::new();
        flate2::read::ZlibDecoder::new(&object[at..at + length]).read_to_end(&mut pixels).unwrap();
        pixels
    }

    #[test]
    fn pdf_has_valid_offsets_order_dimensions_and_lossless_page_pixels() {
        let (doc, red, blue) = document();
        let pdf = export_pdf(&doc, &[blue, red]).unwrap();
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.ends_with(b"%%EOF\n"));
        let objects = objects(&pdf);
        assert_eq!(objects.len(), 8);
        assert!(std::str::from_utf8(objects[1]).unwrap().contains("/Count 2 /Kids [3 0 R 6 0 R]"));
        assert!(std::str::from_utf8(objects[2]).unwrap().contains("/MediaBox [0 0 15 30]"));
        assert!(std::str::from_utf8(objects[5]).unwrap().contains("/MediaBox [0 0 30 15]"));
        let pixels = image_pixels(objects[3]);
        assert_eq!(pixels.len(), 40 * 80 * 3);
        assert!(pixels.chunks_exact(3).all(|pixel| pixel == [0, 0, 255]));
        assert!(image_pixels(objects[6]).chunks_exact(3).all(|pixel| pixel == [255, 0, 0]));
    }

    #[test]
    fn transparent_slide_is_white_and_single_slide_exports_alone() {
        let (mut doc, red, _) = document();
        doc.get_mut(red).unwrap().fills.clear();
        let pdf = export_pdf(&doc, &[red]).unwrap();
        let objects = objects(&pdf);
        assert_eq!(objects.len(), 5);
        assert!(image_pixels(objects[3]).iter().all(|channel| *channel == 255));
    }

    #[test]
    fn raster_resolution_is_bounded_without_changing_page_aspect() {
        let (mut doc, id, _) = document();
        doc.get_mut(id).unwrap().size = designos_doc::Vec2::new(8192.0, 4.0);
        let pdf = export_pdf(&doc, &[id]).unwrap();
        let objects = objects(&pdf);
        assert!(std::str::from_utf8(objects[2]).unwrap().contains("/MediaBox [0 0 6144 3]"));
        let header = String::from_utf8_lossy(&objects[3][..180.min(objects[3].len())]);
        assert!(header.contains("/Width 4096 /Height 2"));
        assert_eq!(image_pixels(objects[3]).len(), 4096 * 2 * 3);
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
}
