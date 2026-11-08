//! Integration tests for the MCP `table` operation in `designos_apply_batch` (T5b, #1231).
#![allow(clippy::result_large_err)]

use designos_doc::{Color, DocumentKind, NodeId, NodeKind, Paint};
use designos_export::base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use designos_mcp::engine::Engine;
use designos_mcp::tools::errors::ErrorCode;
use serde_json::{json, Value};

fn ac1_ops() -> Value {
    json!({
        "ops": [
            { "op": "table", "as": "t", "action": "create", "parent": 1, "x": 40, "y": 40, "rows": 3, "cols": 3 },
            { "op": "table", "node": "@t", "action": "set_cell_text", "row": 0, "col": 0, "text": "Mặt hàng" },
            { "op": "table", "node": "@t", "action": "set_cell_text", "row": 0, "col": 1, "text": "Giá" },
            { "op": "table", "node": "@t", "action": "insert_rows", "at": 1, "count": 2 },
            { "op": "table", "node": "@t", "action": "delete_cols", "at": 2, "count": 1 },
            { "op": "table", "node": "@t", "action": "move_row", "from": 3, "to": 0 },
            { "op": "table", "node": "@t", "action": "resize_col", "at": 1, "size": 240 },
            { "op": "table", "node": "@t", "action": "merge", "row": 0, "col": 0, "row_span": 1, "col_span": 2 }
        ]
    })
}

fn cell_at(doc: &designos_doc::Document, table_id: NodeId, row: usize, col: usize) -> NodeId {
    let t = doc.get(table_id).expect("table exists");
    let NodeKind::Table(tp) = &t.kind else { panic!("expected table") };
    let r_id = tp.rows[row].id;
    let c_id = tp.cols[col].id;
    t.children
        .iter()
        .copied()
        .find(|&cid| matches!(doc.get(cid).map(|n| &n.kind), Some(NodeKind::TableCell(cp)) if cp.row == r_id && cp.col == c_id))
        .expect("cell at (row, col)")
}

#[test]
fn ac1_create_and_populate_in_one_batch() {
    let mut e = Engine::new();
    let req = ac1_ops();
    let res = e.call("designos_apply_batch", req.clone()).expect("AC-1 batch should succeed");

    let table_u64 = res["name_to_id"]["t"].as_u64().expect("table id in name_to_id");
    let table_id = NodeId(table_u64);
    let table_node = e.doc.get(table_id).expect("table node in doc");
    assert_eq!((table_node.transform.tx, table_node.transform.ty), (40.0, 40.0));

    let NodeKind::Table(tp) = &table_node.kind else { panic!("expected NodeKind::Table") };
    assert_eq!(tp.cols.len(), 2, "3 cols - 1 deleted col = 2 cols");
    assert_eq!(tp.rows.len(), 5, "3 rows + 2 inserted rows = 5 rows");
    assert_eq!(tp.cols[1].size, 240.0, "col 1 resized to 240");
    assert_eq!(tp.merges.len(), 1, "1x2 header merge");
    assert_eq!((tp.merges[0].row, tp.merges[0].col, tp.merges[0].row_span, tp.merges[0].col_span), (tp.rows[0].id, tp.cols[0].id, 1, 2));

    // After moving row 3 to row 0, the original row 0 is now at row 1.
    let c10 = cell_at(&e.doc, table_id, 1, 0);
    let c11 = cell_at(&e.doc, table_id, 1, 1);
    let NodeKind::TableCell(cp10) = &e.doc.get(c10).unwrap().kind else { unreachable!() };
    let NodeKind::TableCell(cp11) = &e.doc.get(c11).unwrap().kind else { unreachable!() };
    assert_eq!(cp10.text.content, "Mặt hàng");
    assert_eq!(cp11.text.content, "Giá");

    let undo_label = res["undo_label"].as_str().unwrap_or("");
    assert!(undo_label.starts_with("Agent: "), "expected undo_label, got {undo_label}");
    assert!(e.history.can_undo());

    if let Ok(dir) = std::env::var("TABLE_BATCH_EVIDENCE_DIR") {
        std::fs::create_dir_all(&dir).unwrap();
        let shot = e.call("designos_get_screenshot", json!({ "node_id": table_u64, "scale": 2.0 })).expect("screenshot of AC-1 table");
        let b64 = shot["content"][0]["data"].as_str().expect("base64 image data");
        let png = BASE64_STANDARD.decode(b64).expect("decode base64 png");
        std::fs::write(format!("{dir}/ac1-table-screenshot.png"), png).unwrap();
        let text_log =
            format!("=== Request ===\n{}\n\n=== Reply ===\n{}\n", serde_json::to_string_pretty(&req).unwrap(), serde_json::to_string_pretty(&res).unwrap());
        std::fs::write(format!("{dir}/ac1-batch-request-reply.txt"), text_log).unwrap();
    }

    // Single undo removes the whole table.
    e.history.undo(&mut e.doc).expect("undo AC-1 batch");
    assert!(!e.history.can_undo(), "can_undo() must be false after undoing the single batch");
    assert!(e.doc.get(table_id).is_none(), "table must be removed after undo");
    assert_eq!(e.doc.nodes.len(), 1, "only page node remains");
}

#[test]
fn ac1_all_thirteen_actions_and_noop_skip_undo() {
    let mut e = Engine::new();
    let res = e
        .call(
            "designos_apply_batch",
            json!({
                "ops": [
                    { "op": "table", "as": "t", "action": "create", "rows": 3, "cols": 3 },
                    { "op": "table", "node": "@t", "action": "insert_cols", "at": 1, "count": 1 },
                    { "op": "table", "node": "@t", "action": "delete_rows", "at": 2, "count": 1 },
                    { "op": "table", "node": "@t", "action": "move_col", "from": 2, "to": 0 },
                    { "op": "table", "node": "@t", "action": "resize_row", "at": 0, "size": 60.0 },
                    { "op": "table", "node": "@t", "action": "merge", "row": 0, "col": 0, "row_span": 2, "col_span": 2 },
                    { "op": "table", "node": "@t", "action": "unmerge", "row": 1, "col": 1 },
                    { "op": "table", "node": "@t", "action": "resize", "width": 600.0, "height": 160.0 }
                ]
            }),
        )
        .expect("remaining actions succeed");
    let table_id = NodeId(res["name_to_id"]["t"].as_u64().unwrap());
    let t = e.doc.get(table_id).unwrap();
    assert!((t.size.x - 600.0).abs() < 1e-2 && (t.size.y - 160.0).abs() < 1e-2);
    let NodeKind::Table(tp) = &t.kind else { unreachable!() };
    assert!(tp.merges.is_empty(), "unmerge on covered cell (1,1) splits the 2x2 merge");
    assert_eq!(tp.rows.len(), 2);
    assert_eq!(tp.cols.len(), 4);

    // NoChange op records no undo step (R-T1-2 / R-T1-11).
    let mut e2 = Engine::new();
    e2.call("designos_apply_batch", json!({ "ops": [{ "op": "table", "as": "t", "action": "create", "rows": 2, "cols": 2 }] })).unwrap();
    let tid = e2.doc.pages[0];
    let table2 = e2.doc.get(tid).unwrap().children[0].0;
    e2.history = Default::default();
    assert!(!e2.history.can_undo());
    let noop_res = e2
        .call("designos_apply_batch", json!({ "ops": [{ "op": "table", "node": table2, "action": "move_row", "from": 0, "to": 0 }] }))
        .expect("NoChange move_row is a no-op");
    assert_eq!(noop_res["changed"]["updated"], 0);
    assert!(!e2.history.can_undo(), "NoChange must not record an undo step");
}

#[test]
fn ac2_handle_and_cell_path_resolution() {
    let mut e = Engine::new();
    let res = e
        .call(
            "designos_apply_batch",
            json!({
                "ops": [
                    { "op": "table", "as": "t", "action": "create", "rows": 2, "cols": 2 },
                    { "op": "update", "node": "@t/r0c1", "fill": "#27272A" }
                ]
            }),
        )
        .expect("AC-2 batch should succeed");

    let table_id = NodeId(res["name_to_id"]["t"].as_u64().expect("t in name_to_id"));
    let expected_cell = cell_at(&e.doc, table_id, 0, 1);
    assert_eq!(res["name_to_id"]["t/r0c1"].as_u64(), Some(expected_cell.0), "name_to_id[t/r0c1] must equal cell (0,1) NodeId");

    let cell_node = e.doc.get(expected_cell).expect("cell (0,1) exists");
    assert_eq!(cell_node.fills, vec![Paint::solid(Color::rgb(0x27, 0x27, 0x2A))]);

    if let Ok(dir) = std::env::var("TABLE_BATCH_EVIDENCE_DIR") {
        std::fs::create_dir_all(&dir).unwrap();
        let shot = e.call("designos_get_screenshot", json!({ "node_id": table_id.0, "scale": 2.0 })).expect("screenshot of AC-2 styled table");
        let b64 = shot["content"][0]["data"].as_str().expect("base64 image data");
        let png = BASE64_STANDARD.decode(b64).expect("decode base64 png");
        std::fs::write(format!("{dir}/ac2-styled-table-screenshot.png"), png).unwrap();
    }
}

#[test]
fn ac3_scale_and_single_trial_clone() {
    let mut e = Engine::new();
    let create_res = e
        .call("designos_apply_batch", json!({ "ops": [{ "op": "table", "as": "t", "action": "create", "rows": 10, "cols": 10 }] }))
        .expect("create 10x10 table");
    let table_u64 = create_res["name_to_id"]["t"].as_u64().unwrap();

    let mut ops = Vec::with_capacity(20);
    for i in 0..20usize {
        ops.push(json!({
            "op": "table",
            "node": table_u64,
            "action": "set_cell_text",
            "row": i / 10,
            "col": i % 10,
            "text": format!("Cell {i}")
        }));
    }
    let batch_res = e.call("designos_apply_batch", json!({ "ops": ops })).expect("20 set_cell_text ops");
    let updated = batch_res["changed"]["updated"].as_u64().unwrap_or(0);
    assert!(updated >= 20, "expected changed.updated >= 20, got {updated}");

    // Single undo step reverts all 20 cell text edits.
    e.history.undo(&mut e.doc).unwrap();
    let c00 = cell_at(&e.doc, NodeId(table_u64), 0, 0);
    let NodeKind::TableCell(cp) = &e.doc.get(c00).unwrap().kind else { unreachable!() };
    assert_eq!(cp.text.content, "");

    // 50x50 table creates 2,501 nodes (1 table + 2,500 cells) under 500 ms in release (and fast in debug).
    let mut e50 = Engine::new();
    let t0 = std::time::Instant::now();
    let res50 = e50
        .call("designos_apply_batch", json!({ "ops": [{ "op": "table", "as": "big", "action": "create", "rows": 50, "cols": 50 }] }))
        .expect("create 50x50 table");
    let elapsed = t0.elapsed();
    assert_eq!(res50["created_count"].as_u64(), Some(2501), "1 table + 2500 cells = 2501 nodes");
    assert!(elapsed.as_millis() < 500, "50x50 table creation took {} ms (>= 500 ms)", elapsed.as_millis());
}

#[test]
fn ac4_dry_run_matches_commit_without_mutating_doc() {
    let mut e = Engine::new();
    let node_count_before = e.doc.nodes.len();
    assert!(!e.history.can_undo());

    let ops = json!([
        { "op": "table", "as": "t", "action": "create", "parent": 1, "x": 40, "y": 40, "rows": 3, "cols": 3 },
        { "op": "table", "node": "@t", "action": "set_cell_text", "row": 0, "col": 0, "text": "Header" },
        { "op": "table", "node": "@t", "action": "merge", "row": 0, "col": 0, "row_span": 1, "col_span": 2 }
    ]);

    let dry = e.call("designos_apply_batch", json!({ "dry_run": true, "ops": ops })).expect("dry_run table batch should succeed");
    assert_eq!(dry["dry_run"], json!(true));
    assert_eq!(e.doc.nodes.len(), node_count_before, "dry_run must not mutate node_count");
    assert!(!e.history.can_undo(), "dry_run must not push undo history");

    let committed = e.call("designos_apply_batch", json!({ "dry_run": false, "ops": ops })).expect("commit table batch should succeed");
    assert_eq!(dry["created"], committed["created"], "created must match");
    assert_eq!(dry["name_to_id"], committed["name_to_id"], "name_to_id must match");
    assert_eq!(dry["touched_bounds"], committed["touched_bounds"], "touched_bounds must match");
    assert!(e.history.can_undo());
}

#[test]
fn ac5_refusals_and_rollback() {
    let mut e = Engine::new();
    let setup = e
        .call(
            "designos_apply_batch",
            json!({
                "ops": [
                    { "op": "table", "as": "t", "action": "create", "rows": 3, "cols": 3 },
                    { "op": "table", "node": "@t", "action": "merge", "row": 0, "col": 0, "row_span": 2, "col_span": 2 }
                ]
            }),
        )
        .expect("setup 3x3 table with 2x2 merge at (0,0)");
    let t_id = setup["name_to_id"]["t"].as_u64().unwrap();
    let snap_hash = designos_mcp::tools::batch::batch_state_hash(&e.doc, &[]);
    let undo_before = e.history.can_undo();

    let cases: Vec<(&str, Value, ErrorCode)> = vec![
        ("rows: 0", json!({ "op": "table", "action": "create", "rows": 0, "cols": 3 }), ErrorCode::ValueOutOfRange),
        ("cols: 101", json!({ "op": "table", "action": "create", "rows": 3, "cols": 101 }), ErrorCode::ValueOutOfRange),
        (
            "set_cell_text at row: 99",
            json!({ "op": "table", "node": t_id, "action": "set_cell_text", "row": 99, "col": 0, "text": "x" }),
            ErrorCode::ValueOutOfRange,
        ),
        ("delete_rows removing all rows", json!({ "op": "table", "node": t_id, "action": "delete_rows", "at": 0, "count": 3 }), ErrorCode::InvalidArguments),
        ("move_row splitting a merge", json!({ "op": "table", "node": t_id, "action": "move_row", "from": 0, "to": 2 }), ErrorCode::InvalidArguments),
        (
            "merge overlapping an existing merge",
            json!({ "op": "table", "node": t_id, "action": "merge", "row": 1, "col": 1, "row_span": 2, "col_span": 2 }),
            ErrorCode::InvalidArguments,
        ),
        ("unmerge on an unmerged cell", json!({ "op": "table", "node": t_id, "action": "unmerge", "row": 2, "col": 2 }), ErrorCode::InvalidArguments),
    ];

    for (label, bad_op, expected_code) in cases {
        // Prepend a valid mutation op to verify full transactional rollback on failure.
        let err = e
            .call(
                "designos_apply_batch",
                json!({
                    "ops": [
                        { "op": "table", "node": t_id, "action": "set_cell_text", "row": 2, "col": 2, "text": "should rollback" },
                        bad_op
                    ]
                }),
            )
            .expect_err(label);
        assert!(err.safe_to_retry, "{label}: expected safe_to_retry=true, got {err:?}");
        let cause_code = err.cause.as_ref().map(|c| c.code).unwrap_or(err.code);
        assert_eq!(cause_code, expected_code, "{label}: unexpected cause code in {err:?}");
        assert_eq!(designos_mcp::tools::batch::batch_state_hash(&e.doc, &[]), snap_hash, "{label}: document must remain unchanged after failed batch");
        assert_eq!(e.history.can_undo(), undo_before);
    }

    // Slides document refusal.
    let mut slides_engine = Engine::new();
    slides_engine.doc.kind = DocumentKind::Deck;
    let slides_hash = designos_mcp::tools::batch::batch_state_hash(&slides_engine.doc, &[]);
    let err = slides_engine
        .call("designos_apply_batch", json!({ "ops": [{ "op": "table", "action": "create", "rows": 2, "cols": 2 }] }))
        .expect_err("table in Slides document must be refused");
    assert!(err.safe_to_retry, "Slides refusal must have safe_to_retry=true");
    assert_eq!(err.cause.as_ref().map(|c| c.code), Some(ErrorCode::InvalidArguments));
    assert_eq!(designos_mcp::tools::batch::batch_state_hash(&slides_engine.doc, &[]), slides_hash);
}

#[test]
fn ac6_schema_and_description_list_table_and_example() {
    let tools = designos_mcp::engine::tools();
    let batch = tools.iter().find(|t| t["name"] == "designos_apply_batch").expect("designos_apply_batch tool");
    let ops_schema = &batch["inputSchema"]["properties"]["ops"];
    let op_enum = ops_schema["items"]["properties"]["op"]["enum"].as_array().expect("op enum array");
    assert!(op_enum.contains(&json!("table")), "op enum must contain 'table': {op_enum:?}");

    let ops_desc = ops_schema["description"].as_str().unwrap_or("");
    let top_desc = batch["description"].as_str().unwrap_or("");
    assert!(ops_desc.contains("table") || top_desc.contains("table"), "description must mention 'table'");

    let examples = batch["inputSchema"]["examples"].as_array().expect("examples array");
    let has_table_example = examples.iter().any(|ex| ex["ops"].as_array().is_some_and(|ops| ops.iter().any(|o| o["op"] == "table" && o["action"] == "create")));
    assert!(has_table_example || ops_desc.contains("\"op\":\"table\""), "must include a table example");
}

#[test]
fn ac7_generic_node_tools_on_tables_and_cells() {
    let mut e = Engine::new();
    let res = e.call("designos_apply_batch", json!({ "ops": [{ "op": "table", "as": "t", "action": "create", "rows": 2, "cols": 2 }] })).unwrap();
    let table_id = res["name_to_id"]["t"].as_u64().unwrap();
    let cell_id = res["name_to_id"]["t/r0c0"].as_u64().unwrap();
    let snap_hash = designos_mcp::tools::batch::batch_state_hash(&e.doc, &[]);

    // 1. designos_update_node refuses x, y, width, height, rotation on a TableCell ("use the batch `table` op").
    for bad_update in [
        json!({ "node_id": cell_id, "x": 10.0 }),
        json!({ "node_id": cell_id, "y": 10.0 }),
        json!({ "node_id": cell_id, "width": 200.0 }),
        json!({ "node_id": cell_id, "height": 80.0 }),
        json!({ "node_id": cell_id, "rotation": 45.0 }),
    ] {
        let err = e.call("designos_update_node", bad_update.clone()).expect_err("cell geometry update must fail");
        assert!(err.message.to_lowercase().contains("use the batch `table` op"), "expected 'use the batch `table` op' in error for {bad_update}: {err:?}");
        assert_eq!(designos_mcp::tools::batch::batch_state_hash(&e.doc, &[]), snap_hash);
    }

    // 2. designos_create_node with parent_id = a Table returns the T1 guard error.
    let create_err = e.call("designos_create_node", json!({ "parent_id": table_id, "kind": "rectangle" })).expect_err("create_node under Table must fail");
    assert!(create_err.message.contains("A table holds only cells"), "expected T1 guard message, got: {create_err:?}");
    assert_eq!(designos_mcp::tools::batch::batch_state_hash(&e.doc, &[]), snap_hash);

    // 3. designos_delete_node on a TableCell returns an error and leaves the table unchanged.
    let del_err = e.call("designos_delete_node", json!({ "node_id": cell_id })).expect_err("delete_node on TableCell must fail");
    assert!(del_err.message.to_lowercase().contains("batch `table` op"), "expected batch `table` op guidance in delete error, got: {del_err:?}");
    assert_eq!(designos_mcp::tools::batch::batch_state_hash(&e.doc, &[]), snap_hash);

    // 4. designos_apply_spec with "kind": "table" returns an error pointing to the batch `table` op.
    let spec_err =
        e.call("designos_apply_spec", json!({ "parent_id": 1, "spec": { "kind": "table", "name": "T" } })).expect_err("apply_spec with kind=table must fail");
    assert!(
        spec_err.message.to_lowercase().contains("batch `table` op") || spec_err.next_step.to_lowercase().contains("batch `table` op"),
        "expected apply_spec kind=table error to point to batch `table` op: {spec_err:?}"
    );
    assert_eq!(designos_mcp::tools::batch::batch_state_hash(&e.doc, &[]), snap_hash);

    // 5. Allowed cell update (fill) and deleting the whole Table node work in one undo step.
    e.call("designos_update_node", json!({ "node_id": cell_id, "fill": "#27272A" })).expect("cell fill update succeeds");
    e.call("designos_delete_node", json!({ "node_id": table_id })).expect("deleting whole Table succeeds");
    assert!(e.doc.get(NodeId(table_id)).is_none());
    e.history.undo(&mut e.doc).unwrap();
    assert!(e.doc.get(NodeId(table_id)).is_some(), "undo restores whole Table and its cells");
}
