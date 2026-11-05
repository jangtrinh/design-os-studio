//! A board file keeps writing its connectors as `CONNECTOR` nodes with exactly the fields it always wrote.
//!
//! `fixtures/board_connector_export.json` holds the `.fig` node-change and the REST node of one board
//! connector, captured from the exporter as it was before design files started lowering connectors to
//! vectors. A field added to, dropped from or changed on a board connector fails this test.

use designos_doc::*;
use designos_fig::{decode_fig_to_json, export_fig, export_rest_json};
use serde_json::{json, Value};

const BASELINE: &str = include_str!("fixtures/board_connector_export.json");

/// Two frames and one labelled, elbowed, capped connector on a board. No `fig_metadata` is set anywhere.
fn board_with_connector() -> Document {
    let mut doc = Document::new("board-connector-baseline");
    doc.kind = DocumentKind::Board;
    let page = doc.add_page("Board Page");

    let a = doc.alloc_id();
    let mut fa = Node::frame(a, Rect::new(40.0, 60.0, 120.0, 80.0));
    fa.name = "Frame A".into();
    doc.insert(page, fa).unwrap();

    let b = doc.alloc_id();
    let mut fb = Node::frame(b, Rect::new(320.0, 180.0, 140.0, 90.0));
    fb.name = "Frame B".into();
    doc.insert(page, fb).unwrap();

    let c = doc.alloc_id();
    let mut cn = Node::connector(
        c,
        ConnectorProps {
            start: ConnectorEnd::attached(a, Magnet::Right),
            end: ConnectorEnd::attached(b, Magnet::Left),
            line: ConnectorLine::Elbowed,
            start_cap: ConnectorCap::Circle,
            end_cap: ConnectorCap::Triangle,
            label: Some(TextProps { content: "approves".into(), ..Default::default() }),
        },
    );
    cn.name = "Board Flow".into();
    doc.insert(page, cn).unwrap();
    sync_connector(&mut doc, c);
    assert!(doc.get(c).unwrap().fig_metadata.is_empty(), "the baseline connector carries no kiwi metadata");
    doc
}

/// The `.fig` node-change and the REST node of the board connector, as decoded JSON.
fn board_connector_export() -> Value {
    let doc = board_with_connector();

    let bytes = export_fig(&doc).expect("export_fig");
    let decoded = decode_fig_to_json(&bytes).expect("decode_fig_to_json");
    let changes = decoded["message"]["nodeChanges"].as_array().expect("nodeChanges");
    let fig_connector = changes.iter().find(|n| n["type"] == "CONNECTOR").expect("a CONNECTOR node-change");

    let rest = export_rest_json(&doc);
    let page_children = rest["document"]["children"][0]["children"].as_array().expect("REST page children");
    let rest_connector = page_children.iter().find(|n| n["type"] == "CONNECTOR").expect("a CONNECTOR REST node");

    json!({ "fig": fig_connector, "rest": rest_connector })
}

#[test]
fn board_connector_export_matches_the_stored_baseline() {
    let expected: Value = serde_json::from_str(BASELINE).expect("baseline fixture is valid JSON");
    let actual = board_connector_export();
    assert!(
        actual == expected,
        "board connector export drifted from the stored baseline; actual:\n{}",
        serde_json::to_string_pretty(&actual).unwrap_or_default()
    );
}

#[test]
fn board_connector_export_is_deterministic() {
    assert_eq!(board_connector_export(), board_connector_export());
}
