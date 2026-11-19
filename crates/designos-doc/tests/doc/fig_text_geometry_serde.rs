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

/// The parts of `Node` that 0.7.17 reads for a text layer, with `fig_text_geometry` as 0.7.17 declares it: every field
/// required, no defaults, and `blobs` as the hex strings its own reader decodes. `Node` itself has no
/// `deny_unknown_fields` there (`node.rs:68-69` at `v0.7.17`), so a key it does not know is skipped, as it is here.
#[derive(serde::Deserialize)]
#[allow(dead_code)]
struct Node0717 {
    id: designos_doc::NodeId,
    name: String,
    kind: designos_doc::NodeKind,
    #[serde(default)]
    fig_text_geometry: Option<FigTextGeometry0717>,
}

#[derive(serde::Deserialize)]
#[allow(dead_code)]
struct FigTextGeometry0717 {
    text: TextProps,
    size: Vec2,
    path: Path,
    blobs: Vec<String>,
    glyph_blobs: Vec<Option<u32>>,
}

fn text_node_with(geometry: FigTextGeometry) -> designos_doc::Node {
    let mut node = designos_doc::Node::text(designos_doc::NodeId(1), designos_doc::Rect::new(0.0, 0.0, 10.0, 10.0), "x", designos_doc::TextStyle::default());
    node.fig_text_geometry = Some(Arc::new(geometry));
    node
}

#[test]
fn a_geometry_with_a_glyph_table_is_saved_under_a_key_0_7_17_does_not_know() {
    let saved = geometry(vec![vec![1, 2, 3], vec![4, 5]]);
    let json = serde_json::to_value(text_node_with(saved.clone())).unwrap();
    assert!(json.get("fig_text_geometry").is_none(), "the old key is not written for a table: {json}");
    assert!(json.get("stored_text_glyphs").is_some_and(serde_json::Value::is_object), "the table is under its own key");

    // 0.7.17 reads the node and finds no stored geometry, so it shapes the text everywhere
    let old: Node0717 = serde_json::from_value(json.clone()).expect("0.7.17 reads the node");
    assert!(old.fig_text_geometry.is_none(), "0.7.17 sees no stored geometry");

    // this build gets the table back from the new key
    let back: designos_doc::Node = serde_json::from_value(json).unwrap();
    assert_eq!(back.fig_text_geometry.as_deref(), Some(&saved));
}

#[test]
fn a_geometry_from_before_the_table_keeps_the_old_key_in_the_shape_0_7_17_reads() {
    let legacy = FigTextGeometry {
        text: TextProps::default(),
        size: Vec2::new(10.0, 10.0),
        path: Path { cmds: vec![PathCmd::MoveTo(Vec2::new(0.0, 1.0)), PathCmd::Close], ..Default::default() },
        blobs: vec![Arc::from(vec![1u8, 2, 3])],
        glyph_blobs: vec![Some(0), None],
        glyphs: Default::default(),
    };
    let json = serde_json::to_value(text_node_with(legacy.clone())).unwrap();
    assert!(json.get("stored_text_glyphs").is_none());
    let old: Node0717 = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(old.fig_text_geometry.map(|g| (g.path.cmds.len(), g.glyph_blobs)), Some((2, vec![Some(0), None])));
    let back: designos_doc::Node = serde_json::from_value(json).unwrap();
    assert_eq!(back.fig_text_geometry.as_deref(), Some(&legacy));
}

/// What a crafted document file or clipboard payload can hold: 4,096 glyphs, all naming one blob. The limits are not
/// checked on load; the renderer checks them before it assembles anything (see `stored_glyph_use`).
#[test]
fn a_crafted_table_loads_unchanged_so_the_renderer_has_to_look_at_it() {
    let glyphs: Vec<StoredGlyph> =
        (0..4096).map(|i| StoredGlyph { x: f32::NAN * i as f32, y: 0.0, font_size: 0.0, blob: 0, first_char: 0, style: 0, flags: 0 }).collect();
    let crafted = FigTextGeometry { glyphs: glyphs.into(), ..geometry(vec![vec![0u8; 1000]]) };
    let back: designos_doc::Node = serde_json::from_value(serde_json::to_value(text_node_with(crafted)).unwrap()).unwrap();
    let g = back.fig_text_geometry.unwrap();
    assert_eq!(g.glyphs.len(), 4096);
    assert!(g.glyphs.iter().all(|g| g.x.is_nan() && g.font_size == 0.0), "NaN and a zero size survive the hex form");
    assert_eq!(g.draw_refusal(), Some("stored glyphs not finite"));
}
