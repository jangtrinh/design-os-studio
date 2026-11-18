//! The saved glyph geometry of a text layer in a document file: blobs are hex strings, of any size, and a
//! malformed one is refused rather than guessed.

use designos_doc::{FigTextGeometry, Path, TextProps, Vec2};
use std::sync::Arc;

fn geometry(blobs: Vec<Vec<u8>>) -> FigTextGeometry {
    let glyph_blobs = (0..blobs.len() as u32).map(Some).collect();
    FigTextGeometry {
        text: TextProps::default(),
        size: Vec2::new(10.0, 10.0),
        path: Path::default(),
        blobs: blobs.into_iter().map(Arc::from).collect(),
        glyph_blobs,
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
