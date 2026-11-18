//! Figma's cached glyph outlines for a text layer, kept so a font that is not installed still draws like Figma.

use super::{Node, NodeKind, TextProps};
use crate::{Path, TextStyle, Vec2};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Original Figma glyph geometry, retained while the imported text has not changed.
/// It lets a missing font display with the exact outlines embedded in a `.fig` file.
///
/// A node holds it behind an [`Arc`], so cloning a node (undo snapshots, duplicates, instance
/// copies) shares one copy and costs one pointer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FigTextGeometry {
    /// The text as it was when the glyphs were taken. [`FigTextGeometry::matches`] compares against it.
    pub text: TextProps,
    pub size: Vec2,
    /// Every glyph outline in node-local space.
    pub path: Path,
    /// The distinct glyph command blobs of this text, in first-use order. Figma shares one blob
    /// between every occurrence of a glyph, so a repeated letter is stored once. They are written
    /// back into the blob table of an exported `.fig`.
    #[serde(with = "hex_blobs")]
    pub blobs: Vec<Arc<[u8]>>,
    /// For each entry of `derivedTextData.glyphs`, the index into [`blobs`](Self::blobs) of its
    /// command blob (`None` for a glyph without one).
    pub glyph_blobs: Vec<Option<u32>>,
}

impl FigTextGeometry {
    /// Whether the saved outlines still describe `node`: same box, and the same text with the same
    /// settings that decide where a glyph lands. The text colour does not, so a recolour keeps them.
    pub fn matches(&self, node: &Node) -> bool {
        self.size == node.size && matches!(&node.kind, NodeKind::Text(text) if same_glyph_shapes(text, &self.text))
    }
}

/// Equality of two texts in everything but the paints. Both sides are destructured without a
/// catch-all, so a field added to either type has to be classified here before the crate builds.
fn same_glyph_shapes(a: &TextProps, b: &TextProps) -> bool {
    let TextProps { content, style, runs, auto_resize, max_lines, paragraph, lists, direction, auto_rename, decoration_skip_ink, text_wrap } = a;
    let TextProps {
        content: b_content,
        style: b_style,
        runs: b_runs,
        auto_resize: b_auto_resize,
        max_lines: b_max_lines,
        paragraph: b_paragraph,
        lists: b_lists,
        direction: b_direction,
        auto_rename: b_auto_rename,
        decoration_skip_ink: b_decoration_skip_ink,
        text_wrap: b_text_wrap,
    } = b;
    content == b_content
        && same_glyph_style(style, b_style)
        && runs == b_runs
        && auto_resize == b_auto_resize
        && max_lines == b_max_lines
        && paragraph == b_paragraph
        && lists == b_lists
        && direction == b_direction
        && auto_rename == b_auto_rename
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
