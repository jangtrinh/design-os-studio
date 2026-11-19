//! Which texts of a heavy `.fig` import keep the glyph outlines Figma stored with them.
//!
//! `derivedTextData` is where Figma keeps the outlines it drew. A small file keeps it for every text. A heavy import
//! (a message payload over 2 MB, or more than 5,000 entries) cannot afford that. The importer cannot ask the font
//! database which families a machine has, and a document must not change with the machine that opened it, so it
//! decides from what is the same everywhere: the bundled families. A text set in a bundled family always shapes
//! with the bundled face, so its stored outlines would never be drawn and are not kept.
//!
//! Of the others it keeps only what the drawing code can draw. `designos_render::embedded_glyphs` draws a text's
//! stored outlines when they still describe the layer, its font is missing and it has no text decoration; the
//! importer's `fig_text_geometry` builds them only for a text with content, no style runs, one font whose family is the
//! layer's, at most [`MAX_DRAWN_GLYPHS`] glyphs, and a layout size equal to the layer's. Anything else would be held
//! for the life of the document and never used, so it is not kept. A character style id other than 0 counts as a run
//! here even when no style table entry makes it one, so such a text is left out although it could draw.
//!
//! The decoder can only judge what it has already read. Figma writes a node's fields by ascending number, so `type`,
//! `size`, `textDecoration`, `fontName` and `textData` all precede `derivedTextData`; a file written in another order
//! is judged on less, and what the decoder could not check is checked again after the decode or by the importer.
//!
//! One allowance bounds what is read, charged in the kiwi decoder on wire bytes ([`ALLOWANCE_WIRE_BYTES`]). It covers
//! every route that keeps this data: a heavy file, a payload over 2 MB, and a paste.

use crate::kiwi::KiwiValue;
use std::sync::Arc;

/// The most glyphs of one text the importer builds stored outlines for (`fig_text_geometry`). A text with more is kept
/// by nobody: it would use the allowance and never draw.
pub(crate) const MAX_DRAWN_GLYPHS: usize = 4096;

/// How many wire bytes of `derivedTextData` one import keeps (4 MiB). The allowance bounds the bytes of the file that
/// are read and kept; what they cost in memory depends on the glyph outlines they point at.
///
/// Measured on a Latin design kit whose every text was set in a font that is not bundled, what is kept costs about 60
/// bytes of peak memory per wire byte: the decoded value tree, then a JSON string, then the glyph paths (28 bytes a
/// path command, by far the largest part) and the shared blobs. There 4 MiB took the import's peak from 9.63 to 9.90
/// GB and made it about 0.7 seconds longer; all of that kit (about 11 MB, so 16 MiB) took the peak to 10.3 GB and
/// about 1.5 seconds longer. A real kit that uses a few non-bundled fonts needs a few kilobytes. The cost per wire byte
/// is NOT measured for CJK or other scripts, whose outlines are larger and so cost more memory per wire byte. A text
/// that does not fit is left out and counted in [`crate::rest_import::ImportReport::dropped_stored_glyphs`]; texts are
/// taken in file order.
pub(crate) const ALLOWANCE_WIRE_BYTES: usize = 4 * 1024 * 1024;

/// Whether `family` is one of the families the app embeds. Exact name, ignoring case: a family that only has a
/// substitute (Helvetica to Arimo) is not bundled, which is how the render path treats it too.
pub(crate) fn is_bundled_family(family: &str) -> bool {
    designos_fonts::bundled().iter().any(|asset| asset.family.eq_ignore_ascii_case(family))
}

fn field<'a>(fields: &'a [(Arc<str>, KiwiValue)], name: &str) -> Option<&'a KiwiValue> {
    fields.iter().find(|(k, _)| &**k == name).map(|(_, v)| v)
}

fn float(v: &KiwiValue, name: &str) -> Option<f32> {
    match v.get(name)? {
        KiwiValue::Float(f) => Some(*f),
        _ => None,
    }
}

/// Whether a node change with these decoded `fields` is a text whose stored glyphs could be drawn: a text in a family
/// that is not bundled, with content, no style runs and no text decoration. `false` for any other node, and for a text
/// whose `type` or `fontName` is not among `fields` yet (the kiwi decoder asks while it is still reading the message).
/// A field that has not been read yet is given the benefit of the doubt, and [`derived_can_draw`] and the importer
/// decide the rest.
pub(crate) fn may_draw_stored_glyphs(fields: &[(Arc<str>, KiwiValue)]) -> bool {
    if !matches!(field(fields, "type"), Some(KiwiValue::Enum(t)) if &**t == "TEXT") {
        return false;
    }
    let Some(KiwiValue::String(family)) = field(fields, "fontName").and_then(|f| f.get("family")) else { return false };
    if is_bundled_family(family) {
        return false;
    }
    let text_data = field(fields, "textData");
    // a text has style runs when any character carries a style id other than 0
    let styled = match text_data.and_then(|t| t.get("characterStyleIDs")) {
        Some(KiwiValue::Array(ids)) => ids.iter().any(|id| !matches!(id, KiwiValue::Uint(0) | KiwiValue::Int(0))),
        _ => false,
    };
    let empty = matches!(text_data.and_then(|t| t.get("characters")), Some(KiwiValue::String(chars)) if chars.is_empty());
    let decorated = matches!(field(fields, "textDecoration"), Some(KiwiValue::Enum(d)) if &**d != "NONE");
    !styled && !empty && !decorated
}

/// Whether `derived` (a decoded `derivedTextData`) of the node change with these `fields` is what `fig_text_geometry`
/// builds stored glyphs from: exactly one font, that font's family the layer's, between 1 and [`MAX_DRAWN_GLYPHS`]
/// glyphs, and a layout size equal to the layer's (when the layer's size has been read).
pub(crate) fn derived_can_draw(derived: &KiwiValue, fields: &[(Arc<str>, KiwiValue)]) -> bool {
    let Some(KiwiValue::Array(fonts)) = derived.get("fontMetaData") else { return false };
    let [font] = fonts.as_slice() else { return false };
    let own_family = field(fields, "fontName").and_then(|f| f.get("family"));
    match (font.get("key").and_then(|k| k.get("family")), own_family) {
        (Some(KiwiValue::String(stored)), Some(KiwiValue::String(own))) if stored.eq_ignore_ascii_case(own) => {}
        _ => return false,
    }
    let glyphs = match derived.get("glyphs") {
        Some(KiwiValue::Array(glyphs)) => glyphs.len(),
        _ => 0,
    };
    if glyphs == 0 || glyphs > MAX_DRAWN_GLYPHS {
        return false;
    }
    match (derived.get("layoutSize"), field(fields, "size")) {
        (Some(layout), Some(size)) => match (float(layout, "x"), float(layout, "y"), float(size, "x"), float(size, "y")) {
            (Some(lx), Some(ly), Some(sx), Some(sy)) => (lx - sx).abs() <= 0.1 && (ly - sy).abs() <= 0.1,
            _ => false,
        },
        _ => true,
    }
}

/// The same decision as [`may_draw_stored_glyphs`] and [`derived_can_draw`] for a text that is already JSON (an entry
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
    let text_data = node.get("textData");
    let styled = text_data.and_then(|t| t.get("characterStyleIDs")).and_then(Value::as_array).is_some_and(|ids| ids.iter().any(|id| id.as_u64() != Some(0)));
    let empty = text_data.and_then(|t| t.get("characters")).and_then(Value::as_str).is_some_and(str::is_empty);
    let decorated = node.get("textDecoration").and_then(Value::as_str).is_some_and(|d| d != "NONE");
    if styled || empty || decorated {
        return false;
    }
    let Some([font]) = derived.get("fontMetaData").and_then(Value::as_array).map(Vec::as_slice) else { return false };
    if !font.get("key").and_then(|k| k.get("family")).and_then(Value::as_str).is_some_and(|stored| stored.eq_ignore_ascii_case(family)) {
        return false;
    }
    let glyphs = derived.get("glyphs").and_then(Value::as_array).map_or(0, Vec::len);
    if glyphs == 0 || glyphs > MAX_DRAWN_GLYPHS {
        return false;
    }
    let axis = |v: &Value, key: &str| v.get(key).and_then(Value::as_f64);
    match (derived.get("layoutSize"), node.get("size")) {
        (Some(layout), Some(size)) => match (axis(layout, "x"), axis(layout, "y"), axis(size, "x"), axis(size, "y")) {
            (Some(lx), Some(ly), Some(sx), Some(sy)) => (lx - sx).abs() <= 0.1 && (ly - sy).abs() <= 0.1,
            _ => false,
        },
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fig::take_big_file_glyphs;
    use crate::kiwi::KiwiValue as V;

    fn font_key(family: &str) -> V {
        V::Struct(vec![("family".into(), V::String(family.into()))])
    }

    fn vec2(x: f32, y: f32) -> V {
        V::Struct(vec![("x".into(), V::Float(x)), ("y".into(), V::Float(y))])
    }

    fn derived(family: &str, glyphs: usize) -> V {
        V::Message(vec![
            ("layoutSize".into(), vec2(20.0, 20.0)),
            ("fontMetaData".into(), V::Array(vec![V::Message(vec![("key".into(), font_key(family))])])),
            ("glyphs".into(), V::Array(vec![V::Message(vec![]); glyphs])),
        ])
    }

    fn text_node(family: &str, style_ids: &[u32]) -> V {
        let ids = V::Array(style_ids.iter().map(|&i| V::Uint(i)).collect());
        let text_data = V::Message(vec![("characters".into(), V::String("a".into())), ("characterStyleIDs".into(), ids)]);
        V::Message(vec![
            ("type".into(), V::Enum("TEXT".into())),
            ("fontName".into(), font_key(family)),
            ("size".into(), vec2(20.0, 20.0)),
            ("textData".into(), text_data),
            ("derivedTextData".into(), derived(family, 1)),
        ])
    }

    fn fields(v: &V) -> Vec<(Arc<str>, V)> {
        match v {
            V::Message(f) => f.clone(),
            _ => unreachable!(),
        }
    }

    fn set(node: &mut V, name: &str, value: V) {
        let V::Message(fields) = node else { unreachable!() };
        match fields.iter_mut().find(|(k, _)| &**k == name) {
            Some(slot) => slot.1 = value,
            None => fields.push((name.into(), value)),
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
    fn a_text_with_style_runs_is_not_worth_keeping() {
        assert!(may_draw_stored_glyphs(&fields(&text_node("Some Icon Font", &[0, 0, 0]))), "style id 0 is the base style, not a run");
        assert!(!may_draw_stored_glyphs(&fields(&text_node("Some Icon Font", &[0, 3, 0]))), "a styled character makes a run");
    }

    #[test]
    fn a_text_the_drawing_code_would_refuse_is_not_kept() {
        let node = text_node("Some Icon Font", &[]);
        let ok = |n: &V| {
            let f = fields(n);
            may_draw_stored_glyphs(&f) && derived_can_draw(f.iter().find(|(k, _)| &**k == "derivedTextData").map(|(_, v)| v).unwrap(), &f)
        };
        assert!(ok(&node));

        let mut underlined = node.clone();
        set(&mut underlined, "textDecoration", V::Enum("UNDERLINE".into()));
        assert!(!ok(&underlined), "embedded_glyphs does not draw a decorated text");
        let mut plain = node.clone();
        set(&mut plain, "textDecoration", V::Enum("NONE".into()));
        assert!(ok(&plain));

        let mut empty = node.clone();
        set(&mut empty, "textData", V::Message(vec![("characters".into(), V::String(String::new()))]));
        assert!(!ok(&empty), "an empty text has no glyphs to draw");

        let mut other_size = node.clone();
        set(&mut other_size, "size", vec2(25.0, 20.0));
        assert!(!ok(&other_size), "a layout size that is not the layer's never matches");
        let mut near_size = node.clone();
        set(&mut near_size, "size", vec2(20.05, 20.0));
        assert!(ok(&near_size), "the importer allows a tenth of a unit");

        for (what, derived) in [
            ("another family than the layer's", derived("Another Face", 1)),
            ("no glyphs", derived("Some Icon Font", 0)),
            ("more glyphs than are built", derived("Some Icon Font", MAX_DRAWN_GLYPHS + 1)),
        ] {
            let mut n = node.clone();
            set(&mut n, "derivedTextData", derived);
            assert!(!ok(&n), "{what}");
        }
        let mut at_limit = node.clone();
        set(&mut at_limit, "derivedTextData", derived("some icon font", MAX_DRAWN_GLYPHS));
        assert!(ok(&at_limit), "the limit itself is built, and the family is matched ignoring case");

        let mut two_fonts = node;
        let V::Message(f) = &mut two_fonts else { unreachable!() };
        let mut d = derived("Some Icon Font", 1);
        set(&mut d, "fontMetaData", V::Array(vec![V::Message(vec![("key".into(), font_key("Some Icon Font"))]); 2]));
        f.iter_mut().find(|(k, _)| &**k == "derivedTextData").unwrap().1 = d;
        assert!(!ok(&two_fonts), "two fonts");
    }

    #[test]
    fn the_json_twin_agrees_with_the_decoded_form() {
        let node = |ty: &str, family: &str, ids: &[u32]| serde_json::json!({"type": ty, "fontName": {"family": family}, "size": {"x": 20.0, "y": 20.0}, "textData": {"characters": "a", "characterStyleIDs": ids}});
        let derived = |family: &str, glyphs: usize| serde_json::json!({"layoutSize": {"x": 20.0, "y": 20.0}, "fontMetaData": [{"key": {"family": family}}], "glyphs": vec![serde_json::json!({}); glyphs]});
        let two = serde_json::json!({"layoutSize": {"x": 20.0, "y": 20.0}, "fontMetaData": [{}, {}], "glyphs": [{}]});
        let face = "Some Icon Font";
        assert!(json_may_draw_stored_glyphs(&node("TEXT", face, &[0]), &derived(face, 1)));
        assert!(!json_may_draw_stored_glyphs(&node("TEXT", "Inter", &[0]), &derived("Inter", 1)));
        assert!(!json_may_draw_stored_glyphs(&node("FRAME", face, &[0]), &derived(face, 1)), "a node that is not text keeps nothing");
        assert!(!json_may_draw_stored_glyphs(&node("TEXT", face, &[0, 2]), &derived(face, 1)));
        assert!(!json_may_draw_stored_glyphs(&node("TEXT", face, &[0]), &two));
        assert!(!json_may_draw_stored_glyphs(&node("TEXT", face, &[0]), &derived("Another Face", 1)));
        assert!(!json_may_draw_stored_glyphs(&node("TEXT", face, &[0]), &derived(face, MAX_DRAWN_GLYPHS + 1)));
        assert!(!json_may_draw_stored_glyphs(&node("TEXT", face, &[0]), &derived(face, 0)));
        let mut underlined = node("TEXT", face, &[0]);
        underlined["textDecoration"] = "UNDERLINE".into();
        assert!(!json_may_draw_stored_glyphs(&underlined, &derived(face, 1)));
        let mut resized = node("TEXT", face, &[0]);
        resized["size"] = serde_json::json!({"x": 30.0, "y": 20.0});
        assert!(!json_may_draw_stored_glyphs(&resized, &derived(face, 1)));
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
