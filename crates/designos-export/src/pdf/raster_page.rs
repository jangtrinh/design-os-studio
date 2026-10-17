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
