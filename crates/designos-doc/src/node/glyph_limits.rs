//! What a stored text may ask of the renderer. The importer holds a `.fig` to these limits, and the renderer checks them
//! again before it assembles an outline, because a geometry also arrives from a document file or a clipboard payload,
//! where nothing else bounds the glyph count, the blobs a glyph names or the numbers in the table.

use super::fig_text_geometry::FigTextGeometry;
use super::glyph_table::StoredGlyph;
use super::stored_text_extras::StoredFallback;

/// The most glyphs of one text that draw stored outlines.
pub const MAX_DRAWN_GLYPHS: usize = 4096;
/// The most blob bytes one text may draw, counted once per glyph that names a blob (a shared blob costs its bytes for
/// every glyph that draws it).
pub const MAX_BLOB_BYTES_PER_TEXT: usize = 2_000_000;
/// The most path commands one assembled text may hold: glyphs times the commands of the blob each names. A command is
/// at least one byte of blob, so a text inside [`MAX_BLOB_BYTES_PER_TEXT`] cannot reach it; the renderer still counts
/// before it allocates.
pub const MAX_ASSEMBLED_COMMANDS: usize = 2_000_000;

impl FigTextGeometry {
    /// Why the renderer must not draw this geometry, `None` when it may. Allocates nothing and looks at each glyph once,
    /// after the glyph count is known to be within [`MAX_DRAWN_GLYPHS`]. A geometry from before the glyph table is its
    /// own built path and only has to have commands.
    ///
    /// The order is the scoreboard's ladder: what the table holds (count, numbers, blob bytes), then what the importer
    /// saw about a glyph (emoji, rotation), then the importer's own verdict on the text.
    pub fn draw_refusal(&self) -> Option<&'static str> {
        if self.glyphs.is_empty() {
            return match self.fallback {
                Some(fallback) => Some(fallback.reason()),
                None => self.path.cmds.is_empty().then_some("empty stored outline"),
            };
        }
        if self.glyphs.len() > MAX_DRAWN_GLYPHS {
            return Some(StoredFallback::GlyphCount.reason());
        }
        let mut blob_bytes = 0usize;
        let mut flags = 0u16;
        for glyph in &self.glyphs {
            if !(glyph.x.is_finite() && glyph.y.is_finite() && glyph.font_size.is_finite() && glyph.font_size > 0.0) {
                return Some("stored glyphs not finite");
            }
            flags |= glyph.flags;
            if let Some(blob) = glyph.blob_slot().and_then(|slot| self.blobs.get(slot as usize)) {
                blob_bytes = blob_bytes.saturating_add(blob.len());
            }
        }
        if blob_bytes > MAX_BLOB_BYTES_PER_TEXT {
            return Some(StoredFallback::BlobBudget.reason());
        }
        if flags & StoredGlyph::EMOJI != 0 {
            return Some("emoji glyph");
        }
        if flags & StoredGlyph::ROTATED != 0 {
            return Some(StoredFallback::RotatedGlyph.reason());
        }
        self.fallback.map(StoredFallback::reason)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Path, StoredGlyph, TextProps, Vec2};
    use std::sync::Arc;

    fn geometry(glyphs: Vec<StoredGlyph>, blobs: Vec<Vec<u8>>) -> FigTextGeometry {
        FigTextGeometry {
            text: TextProps::default(),
            size: Vec2::new(10.0, 10.0),
            path: Path::default(),
            blobs: blobs.into_iter().map(Arc::from).collect(),
            glyph_blobs: Vec::new(),
            glyphs: glyphs.into(),
            ..Default::default()
        }
    }

    fn glyph(x: f32, y: f32, font_size: f32, blob: u32) -> StoredGlyph {
        StoredGlyph { x, y, font_size, blob, first_char: 0, style: 0, flags: 0 }
    }

    #[test]
    fn the_glyph_count_and_the_blob_budget_are_inclusive_limits() {
        let at_count: Vec<_> = (0..MAX_DRAWN_GLYPHS).map(|_| glyph(0.0, 0.0, 10.0, StoredGlyph::NO_BLOB)).collect();
        assert_eq!(geometry(at_count.clone(), Vec::new()).draw_refusal(), None);
        let mut over = at_count;
        over.push(glyph(0.0, 0.0, 10.0, StoredGlyph::NO_BLOB));
        assert_eq!(geometry(over, Vec::new()).draw_refusal(), Some("too many stored glyphs"));

        let half = vec![0u8; MAX_BLOB_BYTES_PER_TEXT / 2];
        let two = vec![glyph(0.0, 0.0, 10.0, 0), glyph(1.0, 0.0, 10.0, 0)];
        assert_eq!(geometry(two.clone(), vec![half.clone()]).draw_refusal(), None, "a shared blob counts once per glyph: exactly the budget");
        let mut three = two;
        three.push(glyph(2.0, 0.0, 10.0, 0));
        assert_eq!(geometry(three, vec![half]).draw_refusal(), Some("stored blobs over budget"));
    }

    #[test]
    fn a_non_finite_or_non_positive_number_in_the_table_is_refused() {
        for (what, g) in [
            ("NaN x", glyph(f32::NAN, 0.0, 10.0, 0)),
            ("infinite y", glyph(0.0, f32::INFINITY, 10.0, 0)),
            ("NaN size", glyph(0.0, 0.0, f32::NAN, 0)),
            ("infinite size", glyph(0.0, 0.0, f32::INFINITY, 0)),
            ("zero size", glyph(0.0, 0.0, 0.0, 0)),
            ("negative size", glyph(0.0, 0.0, -1.0, 0)),
        ] {
            assert_eq!(geometry(vec![glyph(0.0, 0.0, 10.0, 0), g], vec![vec![0u8]]).draw_refusal(), Some("stored glyphs not finite"), "{what}");
        }
    }

    #[test]
    fn a_geometry_with_neither_table_nor_path_has_nothing_to_draw() {
        assert_eq!(geometry(Vec::new(), Vec::new()).draw_refusal(), Some("empty stored outline"));
    }
}
