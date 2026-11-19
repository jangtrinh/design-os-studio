//! Figma's cached glyph outlines for a text layer, kept so an imported text draws like Figma until it is edited.

use super::glyph_table::{GlyphTable, StoredGlyph};
use super::stored_text_extras::{StoredDecoration, StoredFallback};
use super::{Node, NodeKind, TextProps};
use crate::{Path, TextRun, TextStyle, Vec2};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Original Figma glyph geometry, retained while the imported text has not changed.
/// It lets the text display with the exact outlines embedded in a `.fig` file, whatever fonts are installed.
///
/// The glyphs are a table of 24 bytes each plus the command blobs Figma stored; the outlines are built from them when
/// the text is drawn. A node holds the geometry behind an [`Arc`], so cloning a node (undo snapshots, duplicates,
/// instance copies) shares one copy and costs one pointer.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct FigTextGeometry {
    /// The text as it was when the glyphs were taken. [`FigTextGeometry::matches`] compares against it.
    pub text: TextProps,
    pub size: Vec2,
    /// Every glyph outline in node-local space, fully built. Only a document file written before the glyph table
    /// existed fills it: its `glyphs` are empty and this path is what draws. A geometry with a table is not written
    /// with this field or `glyph_blobs`.
    #[serde(default)]
    pub path: Path,
    /// The distinct glyph command blobs of this text, in first-use order. Figma shares one blob
    /// between every occurrence of a glyph, so a repeated letter is stored once. They are written
    /// back into the blob table of an exported `.fig`.
    #[serde(with = "hex_blobs")]
    pub blobs: Vec<Arc<[u8]>>,
    /// For each entry of `derivedTextData.glyphs`, the index into [`blobs`](Self::blobs) of its command blob (`None` for
    /// a glyph without one). Only a document file written before the glyph table existed fills it; see [`path`](Self::path).
    #[serde(default)]
    pub glyph_blobs: Vec<Option<u32>>,
    /// The glyphs in Figma's order.
    #[serde(default, skip_serializing_if = "GlyphTable::is_empty")]
    pub glyphs: GlyphTable,
    /// The underline and strikethrough bars Figma stored, each tagged with the style it belongs to.
    #[serde(default)]
    pub decorations: Arc<[StoredDecoration]>,
    /// The style id Figma gave each of the text's style runs, in run order: what a later save maps the stored style ids
    /// through. Empty for a text without runs.
    #[serde(default)]
    pub run_sids: Arc<[u32]>,
    /// Why the importer decided this text is shaped although it holds data; `None` when the data alone does not say so.
    #[serde(default)]
    pub fallback: Option<StoredFallback>,
}

/// A geometry in table form is written without `path` and `glyph_blobs`; one from before the table is written with them,
/// in the shape 0.7.17 reads, whatever they hold. [`Node`] writes the first kind under a key of its own.
impl Serialize for FigTextGeometry {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        struct Blobs<'a>(&'a [Arc<[u8]>]);
        impl Serialize for Blobs<'_> {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                hex_blobs::serialize(self.0, s)
            }
        }
        let table = self.is_table_form();
        let mut out = s.serialize_struct("FigTextGeometry", if table { 7 } else { 5 })?;
        out.serialize_field("text", &self.text)?;
        out.serialize_field("size", &self.size)?;
        if !table {
            out.serialize_field("path", &self.path)?;
        }
        out.serialize_field("blobs", &Blobs(&self.blobs))?;
        if table {
            out.serialize_field("glyphs", &self.glyphs)?;
            if !self.decorations.is_empty() {
                out.serialize_field("decorations", &self.decorations)?;
            }
            if !self.run_sids.is_empty() {
                out.serialize_field("run_sids", &self.run_sids)?;
            }
            if let Some(fallback) = &self.fallback {
                out.serialize_field("fallback", fallback)?;
            }
        } else {
            out.serialize_field("glyph_blobs", &self.glyph_blobs)?;
        }
        out.end()
    }
}

impl FigTextGeometry {
    /// Whether this geometry is the glyph table (saved under its own key) rather than a path built before the table
    /// existed. A geometry with no glyphs that says why it is not drawn is in table form too: the older form would read
    /// back as an empty outline, which 0.7.17 exports as blank text.
    pub fn is_table_form(&self) -> bool {
        !self.glyphs.is_empty() || self.fallback.is_some()
    }

    /// Whether the saved outlines still describe `node`: same box, and the same text with the same
    /// settings that decide where a glyph lands. The text colour does not, so a recolour keeps them.
    pub fn matches(&self, node: &Node) -> bool {
        self.size == node.size && matches!(&node.kind, NodeKind::Text(text) if same_glyph_shapes(text, &self.text))
    }

    /// How many entries `derivedTextData.glyphs` has.
    pub fn glyph_count(&self) -> usize {
        if self.glyphs.is_empty() {
            self.glyph_blobs.len()
        } else {
            self.glyphs.len()
        }
    }

    /// For each glyph in Figma's order, the slot of its command blob in [`blobs`](Self::blobs), `None` for a glyph without one.
    pub fn glyph_blob_slots(&self) -> impl Iterator<Item = Option<u32>> + '_ {
        let table = self.glyphs.iter().map(StoredGlyph::blob_slot);
        let legacy = self.glyph_blobs.iter().copied();
        // one side is always empty: the table is the form of every document written since it exists
        table.chain(legacy)
    }

    /// The bytes this geometry keeps alive beside the node's own text properties: 24 bytes per glyph, the command blobs
    /// (shared between the texts of a file, counted in full here, so a sum over texts is an upper bound), and a legacy
    /// path when the document has one. The outlines are never in it: they are built when the text is drawn.
    ///
    /// The bound per text, then: at most 4,096 glyphs (24 B each) plus the blob bytes of the file's blobs it names, which
    /// the importer caps at 2 MB counted once per glyph, plus one clone of [`text`](Self::text), which is per text and not
    /// per wire byte. A file of many one-glyph texts therefore retains one `TextProps` clone per text on top of the table.
    pub fn retained_bytes(&self) -> usize {
        self.glyphs.len() * std::mem::size_of::<StoredGlyph>()
            + self.glyph_blobs.len() * std::mem::size_of::<Option<u32>>()
            + self.decorations.len() * std::mem::size_of::<StoredDecoration>()
            + self.run_sids.len() * std::mem::size_of::<u32>()
            + self.blobs.iter().map(|blob| blob.len()).sum::<usize>()
            + self.path.cmds.len() * std::mem::size_of::<crate::PathCmd>()
    }
}

/// Whether two nodes hold the same stored glyph geometry. The same [`Arc`] is the same geometry, which is what an
/// unedited node and its undo snapshot, a branch and its base, or a node before and after an agent's write hold, so the
/// glyph table and blobs of a text are walked only when two different geometries have to be told apart.
pub fn same_fig_text_geometry(a: &Option<Arc<FigTextGeometry>>, b: &Option<Arc<FigTextGeometry>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => Arc::ptr_eq(a, b) || **a == **b,
        _ => false,
    }
}

/// The same runs over the same ranges in the same glyph-shaping style: a run's paints do not move a glyph. Both sides
/// are destructured without a catch-all, so a field added to [`TextRun`] has to be classified here first.
fn same_runs(a: &[TextRun], b: &[TextRun]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            let TextRun { start, end, style } = a;
            let TextRun { start: b_start, end: b_end, style: b_style } = b;
            start == b_start && end == b_end && same_glyph_style(style, b_style)
        })
}

/// Equality of two texts in everything but the paints (of the text and of each run) and the layer's name. Both sides are
/// destructured without a catch-all, so a field added to either type has to be classified here before the crate builds.
fn same_glyph_shapes(a: &TextProps, b: &TextProps) -> bool {
    // The factor a layer applies to style values does not place a glyph by itself: the values it scaled do.
    let TextProps {
        content,
        style,
        runs,
        auto_resize,
        max_lines,
        paragraph,
        lists,
        direction,
        auto_rename: _,
        decoration_skip_ink,
        text_wrap,
        uniform_scale: _,
    } = a;
    let TextProps {
        content: b_content,
        style: b_style,
        runs: b_runs,
        auto_resize: b_auto_resize,
        max_lines: b_max_lines,
        paragraph: b_paragraph,
        lists: b_lists,
        direction: b_direction,
        auto_rename: _,
        decoration_skip_ink: b_decoration_skip_ink,
        text_wrap: b_text_wrap,
        uniform_scale: _,
    } = b;
    content == b_content
        && same_glyph_style(style, b_style)
        && same_runs(runs, b_runs)
        && auto_resize == b_auto_resize
        && max_lines == b_max_lines
        && paragraph == b_paragraph
        && lists == b_lists
        && direction == b_direction
        && decoration_skip_ink == b_decoration_skip_ink
        && text_wrap == b_text_wrap
}

fn same_glyph_style(a: &TextStyle, b: &TextStyle) -> bool {
    let TextStyle {
        font_family,
        font_weight,
        italic,
        font_size,
        line_height,
        letter_spacing,
        paragraph_spacing,
        paragraph_indent,
        list_spacing,
        leading_trim,
        align_h,
        align_v,
        case,
        decoration,
        fills: _,
        features,
        variation_axes,
    } = a;
    let TextStyle {
        font_family: b_font_family,
        font_weight: b_font_weight,
        italic: b_italic,
        font_size: b_font_size,
        line_height: b_line_height,
        letter_spacing: b_letter_spacing,
        paragraph_spacing: b_paragraph_spacing,
        paragraph_indent: b_paragraph_indent,
        list_spacing: b_list_spacing,
        leading_trim: b_leading_trim,
        align_h: b_align_h,
        align_v: b_align_v,
        case: b_case,
        decoration: b_decoration,
        fills: _,
        features: b_features,
        variation_axes: b_variation_axes,
    } = b;
    font_family == b_font_family
        && font_weight == b_font_weight
        && italic == b_italic
        && font_size == b_font_size
        && line_height == b_line_height
        && letter_spacing == b_letter_spacing
        && paragraph_spacing == b_paragraph_spacing
        && paragraph_indent == b_paragraph_indent
        && list_spacing == b_list_spacing
        && leading_trim == b_leading_trim
        && align_h == b_align_h
        && align_v == b_align_v
        && case == b_case
        && decoration == b_decoration
        && features == b_features
        && variation_axes == b_variation_axes
}

/// Glyph blobs in the document file: one `hex:`-prefixed string each, the form image bytes use,
/// instead of an integer array of about 3.5 characters per byte.
mod hex_blobs {
    use serde::de::Error;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::sync::Arc;

    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    /// Blobs up to half of this (less the prefix) are written from the stack. The document hash streams a
    /// node through its serializer and must not allocate, and a glyph outline is rarely larger than this.
    const STACK_TEXT: usize = 4096;

    struct HexBlob<'a>(&'a [u8]);

    impl Serialize for HexBlob<'_> {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            let len = 4 + self.0.len() * 2;
            let mut stack = [0u8; STACK_TEXT];
            let mut heap;
            let text: &mut [u8] = if len <= STACK_TEXT {
                &mut stack[..len]
            } else {
                heap = vec![0u8; len];
                &mut heap
            };
            text[..4].copy_from_slice(b"hex:");
            for (pair, &b) in text[4..].chunks_exact_mut(2).zip(self.0) {
                pair[0] = DIGITS[usize::from(b >> 4)];
                pair[1] = DIGITS[usize::from(b & 0x0f)];
            }
            s.serialize_str(std::str::from_utf8(text).map_err(serde::ser::Error::custom)?)
        }
    }

    pub fn serialize<S: Serializer>(blobs: &[Arc<[u8]>], s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(blobs.iter().map(|blob| HexBlob(blob)))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Arc<[u8]>>, D::Error> {
        let strings = Vec::<String>::deserialize(d)?;
        strings.iter().map(|s| decode(s).map(Arc::from).map_err(D::Error::custom)).collect()
    }

    fn decode(s: &str) -> Result<Vec<u8>, &'static str> {
        let hex = s.strip_prefix("hex:").ok_or("a glyph blob is a hex: string")?.as_bytes();
        if hex.len() % 2 != 0 {
            return Err("odd-length hex glyph blob");
        }
        let digit = |c: u8| match c {
            b'0'..=b'9' => Ok(c - b'0'),
            b'a'..=b'f' => Ok(c - b'a' + 10),
            b'A'..=b'F' => Ok(c - b'A' + 10),
            _ => Err("invalid hex digit in a glyph blob"),
        };
        hex.chunks_exact(2).map(|p| Ok((digit(p[0])? << 4) | digit(p[1])?)).collect()
    }
}
