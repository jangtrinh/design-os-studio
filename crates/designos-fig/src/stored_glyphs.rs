//! Which texts of an import keep the data Figma stored with them, and how much of it.
//!
//! `derivedTextData` is where Figma keeps what it laid out for a text: the glyphs with their outlines, the decoration
//! bars, the lines. Every unedited imported text draws from it, whatever its font, and a save writes it back as Figma
//! stored it. A text in any family therefore keeps it; what a heavy import cannot afford is the sum, so the decoder
//! charges one allowance for it.
//!
//! The decoder keeps the `derivedTextData` of a node that is a text, and of no other node. Figma writes a node's fields
//! by ascending number, so `type` precedes `derivedTextData`; a message that does not show its `type` by then is judged
//! on less and keeps nothing, which is the safe direction. What it keeps it keeps as the wire bytes of Studio's own
//! schema (see `derived_text_wire`), so the value tree it decoded is gone before the next text is read. Nothing is
//! rejected after the decode: a kept text is a decoded text, and the allowance bounds both.
//!
//! The data Figma stored for a text inside an instance (`derivedTextData` in an entry of the instance's `derivedSymbolData`) is
//! kept the same way, on an allowance of its own in a file import so it cannot use up what the texts themselves keep; a paste
//! has one allowance for both. What is kept of it is only what draws (see `stored_text_import`).
//!
//! The allowance is charged in the kiwi decoder on wire bytes. Kept glyphs count toward the decode value cap and the
//! zero-size cap like every other field, and nothing is given back.

use crate::kiwi::KiwiValue;
use std::sync::Arc;

/// The most glyphs of one text the importer builds stored outlines for. A text with more keeps its data for a save and
/// is shaped. The renderer checks the same limit before it draws.
pub(crate) use designos_doc::MAX_DRAWN_GLYPHS;

/// How many wire bytes of `derivedTextData` a `.fig` file import keeps (64 MiB). The design kit the allowance was sized
/// on has 11.24 MB of it, so this is 5.7 times that. What is kept is retained at about two bytes per wire byte (the
/// bytes themselves, the glyph table and the snapshot of the text), and nothing is turned into a path when a file
/// opens. A text that does not fit is left out and counted in
/// [`crate::rest_import::ImportReport::dropped_stored_glyphs`]; texts are taken in file order.
pub(crate) const ALLOWANCE_FILE_WIRE_BYTES: usize = 64 * 1024 * 1024;

/// How many wire bytes of the data Figma stored for the texts inside instances (`derivedTextData` in the entries of
/// `derivedSymbolData`) a `.fig` file import keeps (128 MiB), on top of [`ALLOWANCE_FILE_WIRE_BYTES`], so that these can
/// never use up what the texts themselves need. The design kit holds 66.2 MiB of it, because a component used again and
/// again with an override has an entry for each use; the allowance is 1.9 times that. Equal spans are kept once
/// (see `stored_text_import`), and only a span that lands on a copy the component's data does not fit is kept for good
/// (the kit's 66.2 MiB of spans are 14.3 MiB of different ones), so what stays is far below the allowance. What
/// does not fit is counted in [`crate::rest_import::ImportReport::dropped_stored_glyphs`] with the rest. A paste has
/// no allowance of its own for them: they draw on its 4 MiB.
pub(crate) const ALLOWANCE_FILE_NESTED_WIRE_BYTES: usize = 128 * 1024 * 1024;

/// How many wire bytes a paste from Figma keeps (4 MiB). A paste is decoded on the UI thread and any page can plant
/// one, so it keeps far less than a file: a full keep would add about 14% decoded values to a large paste.
pub(crate) const ALLOWANCE_PASTE_WIRE_BYTES: usize = 4 * 1024 * 1024;

/// Whether the node change with these decoded `fields` is a text, which is the only node whose `derivedTextData` is
/// kept. `false` for a message whose `type` has not been read yet.
pub(crate) fn is_text_node(fields: &[(Arc<str>, KiwiValue)]) -> bool {
    matches!(fields.iter().find(|(k, _)| &**k == "type"), Some((_, KiwiValue::Enum(t))) if &**t == "TEXT")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(ty: &str) -> Vec<(Arc<str>, KiwiValue)> {
        vec![("type".into(), KiwiValue::Enum(ty.into()))]
    }

    #[test]
    fn only_a_text_node_keeps_its_derived_data() {
        assert!(is_text_node(&fields("TEXT")), "whatever its family, style or decoration");
        assert!(!is_text_node(&fields("FRAME")));
        assert!(!is_text_node(&[]), "a message that has not shown its type yet keeps nothing");
        assert!(!is_text_node(&[("fontName".into(), KiwiValue::String("Inter".into()))]));
    }

    #[test]
    fn a_file_keeps_sixteen_times_what_a_paste_does() {
        assert_eq!(ALLOWANCE_FILE_WIRE_BYTES / ALLOWANCE_PASTE_WIRE_BYTES, 16);
        assert_eq!(ALLOWANCE_PASTE_WIRE_BYTES, 4 << 20, "the paste allowance is the one of before");
    }
}
