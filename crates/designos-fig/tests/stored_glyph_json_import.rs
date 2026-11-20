//! A text whose `derivedTextData` arrives as JSON (a REST-shaped payload, a fixture) keeps it as wire bytes too: the metadata
//! does not hold the JSON once the import is done, and a save writes the data back.
#[path = "common/missing_font_fig.rs"]
mod missing_font_fig;

use designos_fig::{decode_fig_to_json, export_fig, import_fig_node_changes};
use missing_font_fig::*;
use serde_json::json;

#[test]
fn derived_text_data_given_as_json_becomes_wire_bytes_and_leaves_the_metadata() {
    let guid = |s: u32, l: u32| json!({"sessionID": s, "localID": l});
    let blob: Vec<u32> = icon_blob().into_iter().map(u32::from).collect();
    let changes = json!({
        "nodeChanges": [
            {"guid": guid(0, 0), "type": "DOCUMENT", "name": "Document"},
            {"guid": guid(0, 1), "parentIndex": {"guid": guid(0, 0), "position": "a"}, "type": "CANVAS", "name": "Page"},
            {
                "guid": guid(1, 2),
                "parentIndex": {"guid": guid(0, 1), "position": "a"},
                "type": "TEXT",
                "name": "Json text",
                "size": {"x": 20.0, "y": 20.0},
                "fontSize": 20.0,
                "fontName": {"family": FAMILY, "style": "Regular", "postscript": ""},
                "textData": {"characters": "\u{e900}"},
                "derivedTextData": {
                    "layoutSize": {"x": 20.0, "y": 20.0},
                    "fontMetaData": [{"key": {"family": FAMILY, "style": "Regular", "postscript": ""}}],
                    "glyphs": [{"commandsBlob": 0, "position": {"x": 0.0, "y": 20.0}, "fontSize": 20.0, "firstCharacter": 0}]
                }
            }
        ],
        "blobs": [{"bytes": blob}]
    });
    let doc = import_fig_node_changes(&changes, "json").unwrap();
    let text = doc.nodes.values().find(|n| n.name == "Json text").unwrap();
    assert!(!text.fig_metadata.contains_key("derivedTextData"), "no JSON is left in the metadata");
    let saved = text.fig_text_geometry.as_ref().expect("the text keeps its data");
    assert!(!saved.wire.is_empty() && saved.matches(text));
    assert_eq!(designos_render::stored_glyph_use(text), designos_render::StoredGlyphUse::Drawn);
    let written = export_fig(&doc).unwrap();
    let all = decode_fig_to_json(&written).unwrap();
    let derived = all["message"]["nodeChanges"].as_array().unwrap().iter().find(|n| n["name"] == "Json text").and_then(|n| n.get("derivedTextData")).cloned();
    assert_eq!(derived.as_ref().and_then(|d| d["glyphs"].as_array()).map(Vec::len), Some(1), "a save writes it back");
}
