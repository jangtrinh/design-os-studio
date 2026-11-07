//! Agents read tables over MCP: `designos_get_metadata`, `designos_query_xpath` and `designos_get_node`.
#![allow(clippy::result_large_err)]

use designos_doc::{build_table, Command, NodeId, NodeKind, Vec2};
use designos_mcp::engine::Engine;
use serde_json::{json, Value};

/// An engine whose default page holds a 3x3 table with text in two cells. Returns it with the table id.
fn engine_with_table() -> (Engine, u64) {
    let mut e = Engine::new();
    let page = e.resolve_default_page().unwrap();
    let nodes = build_table(&mut e.doc, 3, 3, Vec2::new(40.0, 60.0));
    let table = nodes[0].id;
    Command::Insert { parent: page, index: 0, nodes }.apply(&mut e.doc).unwrap();
    for (i, text) in [(0usize, "Bảng giá tháng 11"), (4, "Qty")] {
        let cell = e.doc.get(table).unwrap().children[i];
        if let NodeKind::TableCell(c) = &mut e.doc.get_mut(cell).unwrap().kind {
            c.text.content = text.to_string();
        }
    }
    (e, table.0)
}

#[test]
fn get_metadata_lists_the_table_with_its_cells_by_zero_based_row_and_column() {
    let (mut e, table) = engine_with_table();
    let res = e.call("designos_get_metadata", json!({})).unwrap();
    let xml = res["xml"].as_str().unwrap();
    assert!(xml.contains(&format!("<table id=\"{table}\"")), "{xml}");
    assert!(xml.contains("rows=\"3\" cols=\"3\""), "{xml}");
    assert_eq!(xml.matches("<table-cell ").count(), 9, "{xml}");
    assert!(xml.contains("row=\"0\" col=\"0\""), "{xml}");
    assert!(xml.contains("row=\"2\" col=\"2\""), "{xml}");
    assert!(xml.contains("text=\"Bảng giá tháng 11\""), "{xml}");
    assert_eq!(res["node_count"], 11, "page + table + 9 cells");
    // Set TABLE_READ_EVIDENCE_DIR to keep the XML an agent receives (the PR evidence capture).
    if let Ok(dir) = std::env::var("TABLE_READ_EVIDENCE_DIR") {
        std::fs::write(format!("{dir}/get-metadata-3x3-table.xml"), xml).unwrap();
    }
}

#[test]
fn query_xpath_returns_one_row_per_cell() {
    let (mut e, table) = engine_with_table();
    let res = e.call("designos_query_xpath", json!({"selector": "//TABLE_CELL"})).unwrap();
    assert_eq!(res["count"], 9, "{res}");
    assert_eq!(res["rows"].as_array().unwrap().len(), 9);
    let tables = e.call("designos_query_xpath", json!({"selector": "//TABLE"})).unwrap();
    assert_eq!(tables["rows"][0][0].as_u64(), Some(table));
}

#[test]
fn get_node_returns_the_cell_json_with_its_text() {
    let (mut e, table) = engine_with_table();
    let cell: NodeId = e.doc.get(NodeId(table)).unwrap().children[0];
    let res = e.call("designos_get_node", json!({"node_id": cell.0})).unwrap();
    let kind: &Value = &res["node"]["kind"]["TableCell"];
    assert_eq!(kind["text"]["content"], "Bảng giá tháng 11", "{res}");
    assert!(kind["row"]["local"].is_u64() && kind["col"]["local"].is_u64(), "{res}");
    assert_eq!(res["node"]["parent"].as_u64(), Some(table));
}

#[test]
fn get_metadata_shows_the_span_of_a_merged_cell() {
    let (mut e, table) = engine_with_table();
    let table_id = NodeId(table);
    let (first, covered) = {
        let t = e.doc.get(table_id).unwrap();
        (t.children[0], [t.children[1], t.children[3], t.children[4]])
    };
    for id in covered {
        e.doc.remove(id).unwrap();
    }
    let NodeKind::Table(p) = &mut e.doc.get_mut(table_id).unwrap().kind else { unreachable!() };
    let (row, col) = (p.rows[0].id, p.cols[0].id);
    p.merges.push(designos_doc::TableMerge { row, col, row_span: 2, col_span: 2 });
    let res = e.call("designos_get_metadata", json!({})).unwrap();
    let xml = res["xml"].as_str().unwrap();
    assert_eq!(xml.matches("rowspan=\"2\" colspan=\"2\"").count(), 1, "{xml}");
    assert!(xml.contains(&format!("<table-cell id=\"{}\"", first.0)));
    assert_eq!(xml.matches("<table-cell ").count(), 6, "{xml}");
    assert_eq!(xml.matches("rowspan").count(), 1, "unmerged cells carry no span: {xml}");
}
