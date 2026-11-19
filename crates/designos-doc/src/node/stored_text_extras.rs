//! What a stored text keeps beside its glyphs: the decoration bars Figma drew, and the reason a text that has stored data
//! is still shaped by Studio.

use serde::{Deserialize, Serialize};

/// One underline or strikethrough bar Figma stored in `derivedTextData.decorations`, in the same node-local space as the
/// glyph positions (y down). Skip-ink gaps are separate bars.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoredDecoration {
    /// The style the bar belongs to: 0 is the text's own style, `i + 1` is `runs[i]`.
    pub style: u16,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Why the importer decided that a text's stored data is not drawn, when the data alone says so. What the node says (an
/// edit, a list, a decoration the data lacks) is decided when the text is drawn and is not stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoredFallback {
    /// `layoutSize` is missing or more than 0.1 px off the layer's size.
    LayoutSize,
    /// The fonts Figma stored do not include any font the text uses.
    Fonts,
    /// A glyph names a character at or past the end of the text, or none.
    FirstCharacter,
    /// A glyph is rotated: the builder places glyphs upright only.
    RotatedGlyph,
    /// More glyphs than one text may draw.
    GlyphCount,
    /// More blob bytes than one text may draw.
    BlobBudget,
    /// A blob the file does not have, or a number that is not finite.
    Malformed,
    /// The text is truncated: Figma's glyphs may stop short of the text.
    Truncated,
    /// The text has blockquote bars or checklist markers Figma draws outside its glyphs.
    Markers,
    /// The style runs the importer found are not the ones the stored style ids were counted against.
    RunIds,
    /// A name this build does not know, from a file written by a later one.
    #[serde(other)]
    Unrecognised,
}

impl StoredFallback {
    /// The name the scoreboard and the tests use.
    pub fn reason(self) -> &'static str {
        match self {
            Self::LayoutSize => "layout size missing or not the layer's",
            Self::Fonts => "stored fonts do not meet the text's fonts",
            Self::FirstCharacter => "glyph first character outside the text",
            Self::RotatedGlyph => "rotated glyph",
            Self::GlyphCount => "too many stored glyphs",
            Self::BlobBudget => "stored blobs over budget",
            Self::Malformed => "malformed stored glyph data",
            Self::Truncated => "truncated text",
            Self::Markers => "blockquote or checklist markers",
            Self::RunIds => "stored style ids not counted against these runs",
            Self::Unrecognised => "stored fallback this build does not know",
        }
    }
}
