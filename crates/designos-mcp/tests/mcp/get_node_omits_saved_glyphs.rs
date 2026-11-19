//! `designos_get_node` on an imported text layer does not hand the agent Figma's cached glyph outlines and blobs:
//! kilobytes of path data that say nothing an agent can act on.
#![allow(clippy::result_large_err)]

#[path = "../../../designos-fig/tests/common/missing_font_fig.rs"]
mod missing_font_fig;

use designos_mcp::engine::Engine;
use missing_font_fig::*;
use serde_json::json;

#[test]
fn imported_icon_text_reply_has_no_saved_glyph_data() {
    let mut e = Engine::new();
    e.doc = designos_fig::import_fig(&icon_fixture(FAMILY)).unwrap();
    let icon = e.doc.nodes.values().find(|n| n.name == "Icon text").unwrap();
    assert!(icon.fig_text_geometry.is_some(), "the fixture keeps its glyphs in the document");
    let id = icon.id.0;

    let got = e.call("designos_get_node", json!({"node_id": id})).unwrap();
    let node = got["node"].as_object().expect("a node object");
    assert!(!node.contains_key("fig_text_geometry"), "the reply carries no saved glyph data: {}", got["node"]);
    assert_eq!(node["name"], "Icon text");
}
