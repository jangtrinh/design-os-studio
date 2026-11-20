//! A text's `derivedTextData` as bytes in Studio's export schema.
//!
//! Figma stores, for every text, the glyphs it laid out (their outlines, positions and styles), the decoration bars and the
//! line data. Studio keeps that value as the wire bytes of the export schema's `DerivedTextData` definition, which is
//! what a save needs to write it back, and builds the glyph table the renderer draws from by decoding those bytes one
//! text at a time. Nothing else holds the value: no JSON, and no decoded tree outlives its text.

use crate::fig_export::export_schema;
use crate::kiwi::{KiwiValue, Schema};
use std::sync::OnceLock;

/// Studio's export schema, the definition of `DerivedTextData` in it, and the zero-size table its decoder reads.
struct Target {
    schema: &'static Schema,
    derived: usize,
    node_change: usize,
    zero_wire: Vec<bool>,
}

fn target() -> Option<&'static Target> {
    static TARGET: OnceLock<Option<Target>> = OnceLock::new();
    TARGET
        .get_or_init(|| {
            let schema = export_schema();
            Some(Target { schema, derived: schema.find("DerivedTextData")?, node_change: schema.find("NodeChange")?, zero_wire: schema.zero_wire_table() })
        })
        .as_ref()
}

/// The wire bytes of a decoded `derivedTextData`, or `None` when it cannot be written whole in the export schema (an
/// enum name it does not have, a value that does not fit its field).
pub(crate) fn wire_from_kiwi(derived: &KiwiValue) -> Option<Vec<u8>> {
    let t = target()?;
    let value = t.schema.conform_def(t.derived, derived)?;
    t.schema.encode_message(t.derived, &value).ok()
}

/// The same for a `derivedTextData` that is JSON (a REST-shaped payload, a test fixture, a document file of an older build).
pub(crate) fn wire_from_json(derived: &serde_json::Value) -> Option<Vec<u8>> {
    let t = target()?;
    // strictly: a value that does not convert whole is not kept shortened
    let field = t.schema.defs.get(t.node_change)?.fields.iter().find(|f| &*f.name == "derivedTextData")?;
    let value = t.schema.field_from_json_strict(field, derived, 0)?;
    t.schema.encode_message(t.derived, &value).ok()
}

/// The value of `wire`, as [`wire_from_kiwi`] or [`wire_from_json`] wrote it.
pub(crate) fn decode_wire(wire: &[u8]) -> Option<KiwiValue> {
    let t = target()?;
    t.schema.decode_message_prepared(t.derived, wire, &t.zero_wire).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kiwi::KiwiValue as V;

    fn derived_json() -> serde_json::Value {
        serde_json::json!({
            "layoutSize": {"x": 20.0, "y": 10.5},
            "glyphs": [
                {"commandsBlob": 3, "position": {"x": 0.0, "y": 8.0}, "fontSize": 12.0, "firstCharacter": 0, "advance": 0.5},
                {"position": {"x": 6.0, "y": 8.0}, "fontSize": 12.0, "firstCharacter": 1, "styleID": 7}
            ],
            "decorations": [{"rects": [{"x": 0.0, "y": 10.0, "w": 12.0, "h": 1.0}], "styleID": 7}],
            "fontMetaData": [{"key": {"family": "Some Face", "style": "Regular", "postscript": ""}}],
            "truncationStartIndex": -1
        })
    }

    #[test]
    fn bytes_decode_to_the_value_that_was_written() {
        let wire = wire_from_json(&derived_json()).expect("a derivedTextData in the export schema's shape");
        let value = decode_wire(&wire).expect("the bytes decode");
        let glyphs = value.get("glyphs").and_then(V::as_array).unwrap();
        assert_eq!(glyphs.len(), 2);
        assert_eq!(glyphs[0].get("commandsBlob"), Some(&V::Uint(3)));
        assert!(glyphs[1].get("commandsBlob").is_none(), "a glyph without an outline has no blob number");
        assert_eq!(glyphs[1].get("styleID"), Some(&V::Uint(7)));
        assert_eq!(value.get("truncationStartIndex"), Some(&V::Int(-1)));
        assert_eq!(wire_from_kiwi(&value).as_deref(), Some(&wire[..]), "encoding what was decoded gives the same bytes");
    }

    #[test]
    fn a_decoded_value_of_another_schema_gives_the_same_bytes_as_its_json() {
        let from_json = wire_from_json(&derived_json()).unwrap();
        let value = decode_wire(&from_json).unwrap();
        // a value with a field Studio's schema does not have, as a newer file's schema would give
        let V::Message(mut fields) = value else { unreachable!() };
        fields.insert(0, ("fieldOfAFutureFigma".into(), V::Uint(1)));
        assert_eq!(wire_from_kiwi(&V::Message(fields)).as_deref(), Some(&from_json[..]), "what the schema does not name is left out");
    }

    #[test]
    fn a_value_that_cannot_be_written_whole_has_no_bytes() {
        let mut bad = derived_json();
        bad["glyphs"][0]["position"] = serde_json::json!("not a vector");
        assert!(wire_from_json(&bad).is_none());
        let value = V::Message(vec![("derivedLines".into(), V::Array(vec![V::Message(vec![("directionality".into(), V::Enum("NOT_A_DIRECTION".into()))])]))]);
        assert!(wire_from_kiwi(&value).is_none(), "an enum name Studio's schema lacks");
    }

    #[test]
    fn hostile_bytes_are_refused_not_panicked_on() {
        assert!(decode_wire(&[0xff, 0xff, 0xff, 0xff, 0x0f]).is_none());
        let wire = wire_from_json(&derived_json()).unwrap();
        assert!(decode_wire(&wire[..wire.len() / 2]).is_none());
    }
}
