//! AC-04 integration test: importing and round-tripping a `.fig` fixture whose
//! nodes carry 64-bit `strokeSeed` values with bit 63 set (`0xa3c4_d2ae_ffed_a986`,
//! `1 << 63`, and `u64::MAX`).

use designos_doc::*;
use designos_fig::{decode_fig_to_json, export_fig, import_fig};

const FIXTURE_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stroke-seed-bit63.fig");

fn build_fixture_document() -> Document {
    let mut doc = Document::new("stroke-seed-bit63");
    let page = doc.add_page("Page 1");

    let card_id = doc.alloc_id();
    let mut card = Node::new(card_id, NodeKind::Frame(FrameProps::default()));
    card.name = "Bit63Card".into();
    card.size = Vec2::new(320.0, 200.0);
    card.fills.push(Paint::solid(Color::rgba(30, 30, 34, 255)));
    card.fig_metadata.insert("strokeSeed".into(), 0xa3c4_d2ae_ffed_a986u64.to_string());
    doc.insert(page, card).unwrap();

    let bar1_id = doc.alloc_id();
    let mut bar1 = Node::new(bar1_id, NodeKind::Rectangle);
    bar1.name = "AccentBar".into();
    bar1.transform = Affine::translate(24.0, 24.0);
    bar1.size = Vec2::new(272.0, 64.0);
    bar1.fills.push(Paint::solid(Color::rgba(227, 99, 35, 255)));
    bar1.fig_metadata.insert("strokeSeed".into(), (1u64 << 63).to_string());
    doc.insert(card_id, bar1).unwrap();

    let bar2_id = doc.alloc_id();
    let mut bar2 = Node::new(bar2_id, NodeKind::Rectangle);
    bar2.name = "SecondaryBar".into();
    bar2.transform = Affine::translate(24.0, 112.0);
    bar2.size = Vec2::new(272.0, 64.0);
    bar2.fills.push(Paint::solid(Color::rgba(53, 130, 255, 255)));
    bar2.fig_metadata.insert("strokeSeed".into(), u64::MAX.to_string());
    doc.insert(card_id, bar2).unwrap();

    doc
}

#[test]
fn ac04_fixture_with_bit63_stroke_seed_imports_and_roundtrips() {
    let bytes = std::fs::read(FIXTURE_PATH).expect("read stroke-seed-bit63.fig fixture");
    let raw_json = decode_fig_to_json(&bytes).expect("decode_fig_to_json on stroke-seed-bit63.fig");
    assert!(raw_json.get("message").and_then(|m| m.get("nodeChanges")).and_then(|v| v.as_array()).is_some());

    let doc = import_fig(&bytes).expect("import stroke-seed-bit63.fig");
    assert_eq!(doc.nodes.len(), 4, "expected 4 nodes (Page 1, Bit63Card, AccentBar, SecondaryBar)");

    let find = |name: &str| doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("missing node {name}"));
    assert_eq!(find("Bit63Card").fig_metadata.get("strokeSeed").map(String::as_str), Some("11800788572677450118"));
    assert_eq!(find("AccentBar").fig_metadata.get("strokeSeed").map(String::as_str), Some("9223372036854775808"));
    assert_eq!(find("SecondaryBar").fig_metadata.get("strokeSeed").map(String::as_str), Some("18446744073709551615"));

    // Export the imported file and re-import it -> round-trip OK (PM spec test step 4).
    let re_exported = export_fig(&doc).expect("export_fig round-trip");
    let roundtrip = import_fig(&re_exported).expect("re-import exported .fig with bit-63 strokeSeed");
    assert_eq!(roundtrip.nodes.len(), 4);
    let find_rt = |name: &str| roundtrip.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("missing node {name}"));
    assert_eq!(find_rt("Bit63Card").fig_metadata.get("strokeSeed").map(String::as_str), Some("11800788572677450118"));
    assert_eq!(find_rt("AccentBar").fig_metadata.get("strokeSeed").map(String::as_str), Some("9223372036854775808"));
    assert_eq!(find_rt("SecondaryBar").fig_metadata.get("strokeSeed").map(String::as_str), Some("18446744073709551615"));

    // Verify the helper builds a valid fixture that also round-trips.
    let fresh = export_fig(&build_fixture_document()).expect("export_fig fresh");
    assert_eq!(import_fig(&fresh).unwrap().nodes.len(), 4);
}
