//! Which texts of a heavy `.fig` import keep the glyph outlines Figma stored with them.
//!
//! `derivedTextData` is where Figma keeps the outlines it drew. A small file keeps it for every text. A heavy import
//! (a message payload over 2 MB, or more than 5,000 entries) cannot afford that. The importer cannot ask the font
//! database which families a machine has, and a document must not change with the machine that opened it, so it
//! decides from what is the same everywhere: the bundled families. A text set in a bundled family always shapes
//! with the bundled face, so its stored outlines would never be drawn and are not kept.
//!
//! Of the others it keeps only what can be drawn: `designos_render::embedded_glyphs` draws a text's stored outlines,
//! and the importer's `fig_text_geometry` builds them only for a text with no style runs and one font in its derived
//! data. Anything else would be held for the life of the document and never used, so it is not kept.
//!
//! One allowance bounds the rest, charged in the kiwi decoder on wire bytes ([`ALLOWANCE_WIRE_BYTES`]), so it bounds
//! the memory of decoding as well as the memory of what stays. It covers every route that keeps this data: a heavy
//! file, a payload over 2 MB, and a paste.

use crate::kiwi::KiwiValue;
use std::sync::Arc;

/// How many wire bytes of `derivedTextData` one import keeps (4 MiB).
///
/// What is kept costs about 60 bytes of peak memory per wire byte: the decoded value tree, then a JSON string, then
/// the glyph paths (28 bytes a path command, by far the largest part) and the shared blobs. Measured on a design kit
/// whose every text was set in a font that is not bundled, 4 MiB took the import's peak from 9.63 to 9.90 GB and made
/// it about 0.7 seconds longer; all of that kit (about 11 MB, so 16 MiB) took the peak to 10.3 GB and about 1.5
/// seconds longer. A real kit that uses a few non-bundled fonts needs a few kilobytes. A text that does not fit is
/// left out and counted in [`crate::rest_import::ImportReport::dropped_stored_glyphs`]; texts are taken in file order.
pub(crate) const ALLOWANCE_WIRE_BYTES: usize = 4 * 1024 * 1024;

/// Whether `family` is one of the families the app embeds. Exact name, ignoring case: a family that only has a
/// substitute (Helvetica to Arimo) is not bundled, which is how the render path treats it too.
pub(crate) fn is_bundled_family(family: &str) -> bool {
    designos_fonts::bundled().iter().any(|asset| asset.family.eq_ignore_ascii_case(family))
}

fn field<'a>(fields: &'a [(Arc<str>, KiwiValue)], name: &str) -> Option<&'a KiwiValue> {
    fields.iter().find(|(k, _)| &**k == name).map(|(_, v)| v)
}

/// Whether a node change with these decoded `fields` is a text whose stored glyphs could be drawn: a text in a family
/// that is not bundled, with no style runs. `false` for any other node, and for a text whose `type` or `fontName`
/// is not among `fields` yet (the kiwi decoder asks while it is still reading the message). A text whose `textData`
/// has not been read yet is given the benefit of the doubt, and [`is_one_font`] and the importer decide the rest.
pub(crate) fn may_draw_stored_glyphs(fields: &[(Arc<str>, KiwiValue)]) -> bool {
    if !matches!(field(fields, "type"), Some(KiwiValue::Enum(t)) if &**t == "TEXT") {
        return false;
    }
    let Some(KiwiValue::String(family)) = field(fields, "fontName").and_then(|f| f.get("family")) else { return false };
    if is_bundled_family(family) {
        return false;
    }
    // a text has style runs when any character carries a style id other than 0
    let styled = match field(fields, "textData").and_then(|t| t.get("characterStyleIDs")) {
        Some(KiwiValue::Array(ids)) => ids.iter().any(|id| !matches!(id, KiwiValue::Uint(0) | KiwiValue::Int(0))),
        _ => false,
    };
    !styled
}

/// Whether `derived` (a decoded `derivedTextData`) names exactly one font, the only shape the importer builds a text's
/// stored glyphs from.
pub(crate) fn is_one_font(derived: &KiwiValue) -> bool {
    matches!(derived.get("fontMetaData"), Some(KiwiValue::Array(fonts)) if fonts.len() == 1)
}

/// The same decision as [`may_draw_stored_glyphs`] and [`is_one_font`] for a text that is already JSON (an entry
/// whose `derivedTextData` has been converted). Used where the kiwi decoder did not decide: a payload under 2 MB
/// with more than 5,000 entries, and JSON fixtures.
pub(crate) fn json_may_draw_stored_glyphs(node: &serde_json::Value, derived: &serde_json::Value) -> bool {
    use serde_json::Value;
    if node.get("type").and_then(Value::as_str) != Some("TEXT") {
        return false;
    }
    let Some(family) = node.get("fontName").and_then(|f| f.get("family")).and_then(Value::as_str) else { return false };
    if is_bundled_family(family) {
        return false;
    }
    let styled =
        node.get("textData").and_then(|t| t.get("characterStyleIDs")).and_then(Value::as_array).is_some_and(|ids| ids.iter().any(|id| id.as_u64() != Some(0)));
    !styled && derived.get("fontMetaData").and_then(Value::as_array).is_some_and(|fonts| fonts.len() == 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fig::take_big_file_glyphs;
    use crate::kiwi::KiwiValue as V;

    fn text_node(family: &str, style_ids: &[u32]) -> V {
        let font = V::Struct(vec![("family".into(), V::String(family.into()))]);
        let one_font = V::Array(vec![V::Message(vec![])]);
        let derived = V::Message(vec![("fontMetaData".into(), one_font)]);
        let ids = V::Array(style_ids.iter().map(|&i| V::Uint(i)).collect());
        let text_data = V::Message(vec![("characterStyleIDs".into(), ids)]);
        V::Message(vec![
            ("type".into(), V::Enum("TEXT".into())),
            ("fontName".into(), font),
            ("textData".into(), text_data),
            ("derivedTextData".into(), derived),
        ])
    }

    fn fields(v: &V) -> Vec<(Arc<str>, V)> {
        match v {
            V::Message(f) => f.clone(),
            _ => unreachable!(),
        }
    }

    #[test]
    fn only_a_non_bundled_family_can_draw_stored_glyphs() {
        assert!(!may_draw_stored_glyphs(&fields(&text_node("Inter", &[]))), "a bundled family is installed everywhere");
        assert!(!may_draw_stored_glyphs(&fields(&text_node("inter", &[]))), "the family name is matched ignoring case");
        assert!(!may_draw_stored_glyphs(&fields(&text_node("Roboto Mono", &[]))));
        assert!(may_draw_stored_glyphs(&fields(&text_node("Helvetica", &[]))), "a family that only has a substitute is not installed");
        assert!(may_draw_stored_glyphs(&fields(&text_node("Some Icon Font", &[]))));
        let mut frame = fields(&text_node("Some Icon Font", &[]));
        frame[0].1 = V::Enum("FRAME".into());
        assert!(!may_draw_stored_glyphs(&frame), "only a text node has glyphs worth keeping");
        assert!(!may_draw_stored_glyphs(&frame[..1]), "a message that has not shown its font yet keeps nothing");
    }

    #[test]
    fn a_text_with_style_runs_or_several_fonts_is_not_worth_keeping() {
        assert!(may_draw_stored_glyphs(&fields(&text_node("Some Icon Font", &[0, 0, 0]))), "style id 0 is the base style, not a run");
        assert!(
            !may_draw_stored_glyphs(&fields(&text_node("Some Icon Font", &[0, 3, 0]))),
            "a styled character makes a run, and a text with runs has no stored glyphs"
        );
        let two_fonts = V::Message(vec![("fontMetaData".into(), V::Array(vec![V::Message(vec![]), V::Message(vec![])]))]);
        assert!(is_one_font(&V::Message(vec![("fontMetaData".into(), V::Array(vec![V::Message(vec![])]))])));
        assert!(!is_one_font(&two_fonts));
        assert!(!is_one_font(&V::Message(vec![])), "no font list names no font");
    }

    #[test]
    fn the_json_twin_agrees_with_the_decoded_form() {
        let json =
            |ty: &str, family: &str, ids: &[u32]| serde_json::json!({"type": ty, "fontName": {"family": family}, "textData": {"characterStyleIDs": ids}});
        let one = serde_json::json!({"fontMetaData": [{}]});
        let two = serde_json::json!({"fontMetaData": [{}, {}]});
        assert!(json_may_draw_stored_glyphs(&json("TEXT", "Some Icon Font", &[0]), &one));
        assert!(!json_may_draw_stored_glyphs(&json("TEXT", "Inter", &[0]), &one));
        assert!(!json_may_draw_stored_glyphs(&json("FRAME", "Some Icon Font", &[0]), &one), "a node that is not text keeps nothing");
        assert!(!json_may_draw_stored_glyphs(&json("TEXT", "Some Icon Font", &[0, 2]), &one));
        assert!(!json_may_draw_stored_glyphs(&json("TEXT", "Some Icon Font", &[0]), &two));
    }

    #[test]
    fn taking_a_text_moves_its_derived_data_out_and_keeps_only_what_can_draw() {
        let mut kept = text_node("Some Icon Font", &[]);
        assert!(take_big_file_glyphs(&mut kept).is_some_and(|json| json.get("fontMetaData").is_some()));
        assert!(kept.get("derivedTextData").is_none(), "the node no longer holds it");

        let mut bundled = text_node("Inter", &[]);
        assert!(take_big_file_glyphs(&mut bundled).is_none());
        assert!(bundled.get("derivedTextData").is_none(), "a bundled family's is dropped");

        let mut styled = text_node("Some Icon Font", &[1]);
        assert!(take_big_file_glyphs(&mut styled).is_none());
    }
}
