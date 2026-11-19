//! The glyphs of a stored text as a compact table of 24 bytes each. The outlines are not in it: a glyph names the
//! command blob it draws, and the renderer builds the paths when the text is drawn.

use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::sync::Arc;

/// One glyph of Figma's `derivedTextData.glyphs`, in the order Figma stored them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StoredGlyph {
    /// Where the glyph's origin stands in the node, y down.
    pub x: f32,
    pub y: f32,
    /// The em size the glyph's outline is scaled by.
    pub font_size: f32,
    /// Slot of the glyph's command blob in the geometry's `blobs`, or [`StoredGlyph::NO_BLOB`] for a glyph Figma stored
    /// without an outline (a space).
    pub blob: u32,
    /// The index of the glyph's first character, in UTF-16 units.
    pub first_char: u32,
    /// Style the glyph belongs to: 0 is the text's own style.
    pub style: u16,
    pub flags: u16,
}

const _: () = assert!(std::mem::size_of::<StoredGlyph>() == 24, "a stored glyph is 24 bytes");

impl StoredGlyph {
    pub const NO_BLOB: u32 = u32::MAX;
    /// The bytes one glyph takes in the document file.
    const WIRE_BYTES: usize = 24;

    /// The slot of the glyph's blob, `None` for a glyph without an outline.
    pub fn blob_slot(&self) -> Option<u32> {
        (self.blob != Self::NO_BLOB).then_some(self.blob)
    }

    fn to_le_bytes(self) -> [u8; Self::WIRE_BYTES] {
        let mut b = [0u8; Self::WIRE_BYTES];
        b[0..4].copy_from_slice(&self.x.to_le_bytes());
        b[4..8].copy_from_slice(&self.y.to_le_bytes());
        b[8..12].copy_from_slice(&self.font_size.to_le_bytes());
        b[12..16].copy_from_slice(&self.blob.to_le_bytes());
        b[16..20].copy_from_slice(&self.first_char.to_le_bytes());
        b[20..22].copy_from_slice(&self.style.to_le_bytes());
        b[22..24].copy_from_slice(&self.flags.to_le_bytes());
        b
    }

    fn from_le_bytes(b: &[u8]) -> Self {
        let word = |i: usize| [b[i], b[i + 1], b[i + 2], b[i + 3]];
        Self {
            x: f32::from_le_bytes(word(0)),
            y: f32::from_le_bytes(word(4)),
            font_size: f32::from_le_bytes(word(8)),
            blob: u32::from_le_bytes(word(12)),
            first_char: u32::from_le_bytes(word(16)),
            style: u16::from_le_bytes([b[20], b[21]]),
            flags: u16::from_le_bytes([b[22], b[23]]),
        }
    }
}

/// The glyphs of one text, shared by every clone of its geometry. Two tables are equal when they are the same table
/// or hold the same glyphs, so comparing the geometry of a node with its own snapshot does not walk the glyphs.
#[derive(Clone, Debug, Default)]
pub struct GlyphTable(Arc<[StoredGlyph]>);

impl GlyphTable {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn iter(&self) -> std::slice::Iter<'_, StoredGlyph> {
        self.0.iter()
    }
    pub fn as_slice(&self) -> &[StoredGlyph] {
        &self.0
    }
    /// The address of the shared table, for a cache that keys on identity.
    pub fn as_ptr(&self) -> *const StoredGlyph {
        self.0.as_ptr()
    }
}

impl From<Vec<StoredGlyph>> for GlyphTable {
    fn from(glyphs: Vec<StoredGlyph>) -> Self {
        Self(glyphs.into())
    }
}

impl PartialEq for GlyphTable {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.0 == other.0
    }
}

impl<'a> IntoIterator for &'a GlyphTable {
    type Item = &'a StoredGlyph;
    type IntoIter = std::slice::Iter<'a, StoredGlyph>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

const DIGITS: &[u8; 16] = b"0123456789abcdef";
/// Glyphs per hex string of the document file. The document hash streams a node through its serializer and must not
/// allocate, so a chunk's text (`hex:` and two digits a byte) has to fit the 4096-byte stack buffer of the writer.
const GLYPHS_PER_CHUNK: usize = (4096 - 4) / (2 * StoredGlyph::WIRE_BYTES);
const _: () = assert!(4 + 2 * GLYPHS_PER_CHUNK * StoredGlyph::WIRE_BYTES <= 4096);

impl Serialize for GlyphTable {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        struct Chunk<'a>(&'a [StoredGlyph]);
        impl Serialize for Chunk<'_> {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                let mut text = [0u8; 4096];
                text[..4].copy_from_slice(b"hex:");
                let mut at = 4;
                for glyph in self.0 {
                    for b in glyph.to_le_bytes() {
                        text[at] = DIGITS[usize::from(b >> 4)];
                        text[at + 1] = DIGITS[usize::from(b & 0x0f)];
                        at += 2;
                    }
                }
                s.serialize_str(std::str::from_utf8(&text[..at]).map_err(serde::ser::Error::custom)?)
            }
        }
        s.collect_seq(self.0.chunks(GLYPHS_PER_CHUNK).map(Chunk))
    }
}

impl<'de> Deserialize<'de> for GlyphTable {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let chunks = Vec::<String>::deserialize(d)?;
        let mut glyphs = Vec::new();
        for chunk in &chunks {
            let hex = chunk.strip_prefix("hex:").ok_or_else(|| D::Error::custom("a glyph table chunk is a hex: string"))?.as_bytes();
            if hex.len() % (2 * StoredGlyph::WIRE_BYTES) != 0 {
                return Err(D::Error::custom("a glyph table chunk holds whole 24-byte glyphs"));
            }
            let digit = |c: u8| match c {
                b'0'..=b'9' => Ok(c - b'0'),
                b'a'..=b'f' => Ok(c - b'a' + 10),
                b'A'..=b'F' => Ok(c - b'A' + 10),
                _ => Err(D::Error::custom("invalid hex digit in a glyph table chunk")),
            };
            for glyph in hex.chunks_exact(2 * StoredGlyph::WIRE_BYTES) {
                let mut bytes = [0u8; StoredGlyph::WIRE_BYTES];
                for (b, pair) in bytes.iter_mut().zip(glyph.chunks_exact(2)) {
                    *b = (digit(pair[0])? << 4) | digit(pair[1])?;
                }
                glyphs.push(StoredGlyph::from_le_bytes(&bytes));
            }
        }
        Ok(glyphs.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glyph(i: u32) -> StoredGlyph {
        StoredGlyph {
            x: i as f32 * 1.5,
            y: -2.25,
            font_size: 20.0,
            blob: if i % 5 == 0 { StoredGlyph::NO_BLOB } else { i % 7 },
            first_char: i,
            style: (i % 3) as u16,
            flags: 1,
        }
    }

    #[test]
    fn a_table_round_trips_through_hex_chunks_that_fit_the_stack_writer() {
        let table: GlyphTable = (0..1000).map(glyph).collect::<Vec<_>>().into();
        let json = serde_json::to_value(&table).unwrap();
        let chunks = json.as_array().unwrap();
        assert_eq!(chunks.len(), 1000usize.div_ceil(GLYPHS_PER_CHUNK));
        assert!(chunks.iter().all(|c| c.as_str().unwrap().len() <= 4096), "every chunk fits the writer's stack buffer");
        let back: GlyphTable = serde_json::from_value(json).unwrap();
        assert_eq!(back.as_slice(), table.as_slice());
        assert_eq!(back.iter().filter(|g| g.blob_slot().is_none()).count(), 200);
    }

    #[test]
    fn a_ragged_or_malformed_chunk_is_refused() {
        let good = serde_json::to_value(GlyphTable::from(vec![glyph(1)])).unwrap();
        let text = good[0].as_str().unwrap().to_owned();
        for bad in [text[..text.len() - 2].to_owned(), text.replacen("hex:", "bin:", 1), format!("{}zz", &text[..text.len() - 2])] {
            assert!(serde_json::from_value::<GlyphTable>(serde_json::json!([bad])).is_err(), "{bad} must not load");
        }
    }
}
