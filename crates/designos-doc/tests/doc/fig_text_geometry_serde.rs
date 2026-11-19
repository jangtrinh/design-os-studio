//! The saved glyph geometry of a text layer in a document file: blobs are hex strings, of any size, and a
//! malformed one is refused rather than guessed.

use designos_doc::{FigTextGeometry, Path, PathCmd, StoredGlyph, TextProps, Vec2};
use std::sync::Arc;

fn geometry(blobs: Vec<Vec<u8>>) -> FigTextGeometry {
    let glyphs: Vec<StoredGlyph> =
        (0..blobs.len() as u32).map(|i| StoredGlyph { x: i as f32, y: 8.0, font_size: 10.0, blob: i, first_char: i, style: 0, flags: 0 }).collect();
    FigTextGeometry {
        text: TextProps::default(),
        size: Vec2::new(10.0, 10.0),
        path: Path::default(),
        blobs: blobs.into_iter().map(Arc::from).collect(),
        glyph_blobs: Vec::new(),
        glyphs: glyphs.into(),
    }
}

#[test]
fn blobs_round_trip_as_hex_whatever_their_size() {
    // below and above what is hex-encoded from the stack
    let small: Vec<u8> = (0..=255).collect();
    let large: Vec<u8> = (0..5000u32).map(|i| (i * 7) as u8).collect();
    let saved = geometry(vec![small, large, Vec::new()]);
    let json = serde_json::to_value(&saved).unwrap();
    let blobs = json["blobs"].as_array().unwrap();
    assert!(blobs.iter().all(|b| b.as_str().is_some_and(|s| s.starts_with("hex:"))), "{blobs:?}");
    assert_eq!(blobs[0].as_str().unwrap().len(), 4 + 512);
    assert_eq!(blobs[2], "hex:");
    let restored: FigTextGeometry = serde_json::from_value(json).unwrap();
    assert_eq!(restored, saved);
}

#[test]
fn a_malformed_blob_is_refused() {
    let mut json = serde_json::to_value(geometry(vec![vec![1, 2, 3]])).unwrap();
    for bad in ["010203", "hex:0", "hex:zz"] {
        json["blobs"] = serde_json::json!([bad]);
        assert!(serde_json::from_value::<FigTextGeometry>(json.clone()).is_err(), "{bad} must not load");
    }
}

#[test]
fn the_glyph_table_is_written_as_hex_and_round_trips() {
    let saved = geometry(vec![vec![1, 2, 3], vec![4, 5]]);
    let json = serde_json::to_value(&saved).unwrap();
    assert!(json["glyphs"][0].as_str().is_some_and(|s| s.starts_with("hex:") && s.len() == 4 + 2 * 24 * 2), "two glyphs in one chunk: {}", json["glyphs"]);
    assert!(json.get("path").is_none() && json.get("glyph_blobs").is_none(), "the old fields are not written for a geometry that has a table: {json}");
    assert_eq!(serde_json::from_value::<FigTextGeometry>(json).unwrap(), saved);
}

#[test]
fn a_geometry_written_before_the_glyph_table_loads_and_is_written_back_the_same_way() {
    let legacy = serde_json::json!({
        "text": serde_json::to_value(TextProps::default()).unwrap(),
        "size": {"x": 10.0, "y": 10.0},
        "path": {"cmds": [{"MoveTo": {"x": 0.0, "y": 1.0}}, "Close"], "fill_rule": "NonZero"},
        "blobs": ["hex:010203"],
        "glyph_blobs": [0, null],
    });
    let loaded: FigTextGeometry = serde_json::from_value(legacy.clone()).unwrap();
    assert!(loaded.glyphs.is_empty());
    assert_eq!(loaded.path.cmds, vec![PathCmd::MoveTo(Vec2::new(0.0, 1.0)), PathCmd::Close]);
    assert_eq!(loaded.glyph_count(), 2);
    assert_eq!(loaded.glyph_blob_slots().collect::<Vec<_>>(), vec![Some(0), None]);
    let again = serde_json::to_value(&loaded).unwrap();
    assert_eq!(again["path"], legacy["path"]);
    assert_eq!(again["glyph_blobs"], legacy["glyph_blobs"]);
    assert_eq!(serde_json::from_value::<FigTextGeometry>(again).unwrap(), loaded);
}
