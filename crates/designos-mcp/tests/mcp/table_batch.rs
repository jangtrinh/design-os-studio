//! Integration tests for the MCP `table` operation in `designos_apply_batch`.
#![allow(clippy::result_large_err)]

use designos_doc::{Color, DocumentKind, NodeId, NodeKind, Paint};
use designos_export::base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use designos_mcp::engine::Engine;
use designos_mcp::tools::errors::ErrorCode;
use serde_json::{json, Value};

fn sample_table_ops() -> Value {
    let mut ops = vec![json!({ "op": "table", "as": "t", "action": "create", "parent": 1, "x": 40, "y": 40, "rows": 3, "cols": 3 })];
    for r in 0..3 {
        for c in 0..3 {
            ops.push(json!({
                "op": "table",
                "node": "@t",
                "action": "set_cell_text",
                "row": r,
                "col": c,
                "text": format!("R{r}C{c}")
            }));
        }
    }
    ops.push(json!({ "op": "table", "node": "@t", "action": "merge", "row": 0, "col": 0, "row_span": 2, "col_span": 2 }));
    ops.push(json!({ "op": "table", "node": "@t", "action": "insert_rows", "at": 3, "count": 1 }));
    json!({ "ops": ops })
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
fn create_and_populate_in_one_batch() {
    let mut e = Engine::new();
    let req = sample_table_ops();
    let res = e.call("designos_apply_batch", req.clone()).expect("table batch should succeed");

    let table_u64 = res["name_to_id"]["t"].as_u64().expect("table id in name_to_id");
    let table_id = NodeId(table_u64);
    let table_node = e.doc.get(table_id).expect("table node in doc");
    assert_eq!((table_node.transform.tx, table_node.transform.ty), (40.0, 40.0));

    let NodeKind::Table(tp) = &table_node.kind else { panic!("expected NodeKind::Table") };
    assert_eq!(tp.cols.len(), 3, "3 cols");
    assert_eq!(tp.rows.len(), 4, "3 rows + 1 inserted row at 3 = 4 rows");
    assert_eq!(tp.merges.len(), 1, "2x2 merge at (0,0)");
    assert_eq!((tp.merges[0].row, tp.merges[0].col, tp.merges[0].row_span, tp.merges[0].col_span), (tp.rows[0].id, tp.cols[0].id, 2, 2));

    let c00 = cell_at(&e.doc, table_id, 0, 0);
    let c22 = cell_at(&e.doc, table_id, 2, 2);
    let NodeKind::TableCell(cp00) = &e.doc.get(c00).unwrap().kind else { unreachable!() };
    let NodeKind::TableCell(cp22) = &e.doc.get(c22).unwrap().kind else { unreachable!() };
    assert_eq!(cp00.text.content, "R0C0\nR0C1\nR1C0\nR1C1");
    assert_eq!(cp22.text.content, "R2C2");

    let undo_label = res["undo_label"].as_str().unwrap_or("");
    assert!(undo_label.starts_with("Agent: "), "expected undo_label, got {undo_label}");
    assert!(e.history.can_undo());

    if let Ok(dir) = std::env::var("TABLE_BATCH_EVIDENCE_DIR") {
        std::fs::create_dir_all(&dir).unwrap();
        let shot = e.call("designos_get_screenshot", json!({ "node_id": table_u64, "scale": 2.0 })).expect("screenshot of table");
        let b64 = shot["content"][0]["data"].as_str().expect("base64 image data");
        let png = BASE64_STANDARD.decode(b64).expect("decode base64 png");
        std::fs::write(format!("{dir}/table-screenshot.png"), png).unwrap();
        let text_log =
            format!("=== Request ===\n{}\n\n=== Reply ===\n{}\n", serde_json::to_string_pretty(&req).unwrap(), serde_json::to_string_pretty(&res).unwrap());
        std::fs::write(format!("{dir}/batch-request-reply.txt"), text_log).unwrap();
    }

    // Single designos_undo call removes the whole table.
    e.call("designos_undo", json!({})).expect("designos_undo table batch");
    assert!(!e.history.can_undo(), "can_undo() must be false after undoing the single batch");
    assert!(e.doc.get(table_id).is_none(), "table must be removed after undo");
    assert_eq!(e.doc.nodes.len(), 1, "only page node remains");
}

#[test]
fn all_thirteen_actions_and_noop_skip_undo() {
    let mut e = Engine::new();
    let res = e
        .call(
            "designos_apply_batch",
            json!({
                "ops": [
                    { "op": "table", "as": "t", "action": "create", "rows": 3, "cols": 3 },
                    { "op": "table", "node": "@t", "action": "insert_cols", "at": 1, "count": 1 },
                    { "op": "table", "node": "@t", "action": "delete_rows", "at": 2, "count": 1 },
                    { "op": "table", "node": "@t", "action": "move_col", "from_index": 2, "to_index": 0 },
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

    // NoChange op records no undo step.
    let mut e2 = Engine::new();
    e2.call("designos_apply_batch", json!({ "ops": [{ "op": "table", "as": "t", "action": "create", "rows": 2, "cols": 2 }] })).unwrap();
    let tid = e2.doc.pages[0];
    let table2 = e2.doc.get(tid).unwrap().children[0].0;
    e2.history = Default::default();
    assert!(!e2.history.can_undo());
    let noop_res = e2
        .call("designos_apply_batch", json!({ "ops": [{ "op": "table", "node": table2, "action": "move_row", "from_index": 0, "to_index": 0 }] }))
        .expect("NoChange move_row is a no-op");
    assert_eq!(noop_res["changed"]["updated"], 0);
    assert!(!e2.history.can_undo(), "NoChange must not record an undo step");
}

#[test]
fn handle_and_cell_path_resolution() {
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
        .expect("cell handle batch should succeed");

    let table_id = NodeId(res["name_to_id"]["t"].as_u64().expect("t in name_to_id"));
    let expected_cell = cell_at(&e.doc, table_id, 0, 1);
    assert_eq!(res["name_to_id"]["t/r0c1"].as_u64(), Some(expected_cell.0), "name_to_id[t/r0c1] must equal cell (0,1) NodeId");

    let cell_node = e.doc.get(expected_cell).expect("cell (0,1) exists");
    assert_eq!(cell_node.fills, vec![Paint::solid(Color::rgb(0x27, 0x27, 0x2A))]);

    if let Ok(dir) = std::env::var("TABLE_BATCH_EVIDENCE_DIR") {
        std::fs::create_dir_all(&dir).unwrap();
        let shot = e.call("designos_get_screenshot", json!({ "node_id": table_id.0, "scale": 2.0 })).expect("screenshot of styled table");
        let b64 = shot["content"][0]["data"].as_str().expect("base64 image data");
        let png = BASE64_STANDARD.decode(b64).expect("decode base64 png");
        std::fs::write(format!("{dir}/styled-table-screenshot.png"), png).unwrap();
    }
}

#[test]
fn scale_and_single_trial_clone() {
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

    // 50x50 table creates 2,501 nodes (1 table + 2,500 cells) and populates all 2,500 cell keys in name_to_id.
    let mut e50 = Engine::new();
    let res50 = e50
        .call("designos_apply_batch", json!({ "ops": [{ "op": "table", "as": "big", "action": "create", "rows": 50, "cols": 50 }] }))
        .expect("create 50x50 table");
    assert_eq!(res50["created_count"].as_u64(), Some(2501), "1 table + 2500 cells = 2501 nodes");
    let map50 = res50["name_to_id"].as_object().expect("name_to_id object");
    assert_eq!(map50.len(), 2501, "1 table alias + 2,500 cell aliases");
    for r in 0..50usize {
        for c in 0..50usize {
            let key = format!("big/r{r}c{c}");
            assert!(map50.get(&key).and_then(Value::as_u64).is_some(), "missing cell key {key}");
        }
    }
}

/// Run in `--release`: `cargo test -p designos-mcp --release --locked --test table_batch create_50x50_release_under_500ms -- --ignored --nocapture`
#[test]
#[ignore = "wall-clock < 500 ms budget is specified for a release build; run with --release -- --ignored"]
fn create_50x50_release_under_500ms() {
    let mut e50 = Engine::new();
    let t0 = std::time::Instant::now();
    let res50 = e50
        .call("designos_apply_batch", json!({ "ops": [{ "op": "table", "as": "big", "action": "create", "rows": 50, "cols": 50 }] }))
        .expect("create 50x50 table");
    let elapsed = t0.elapsed();
    assert_eq!(res50["created_count"].as_u64(), Some(2501));
    assert!(elapsed.as_millis() < 500, "50x50 table creation took {} ms (>= 500 ms)", elapsed.as_millis());
}

#[test]
fn dry_run_matches_commit_without_mutating_doc() {
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
fn refusals_and_rollback() {
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
        ("cols: 501", json!({ "op": "table", "action": "create", "rows": 3, "cols": 501 }), ErrorCode::ValueOutOfRange),
        (
            "set_cell_text at row: 99",
            json!({ "op": "table", "node": t_id, "action": "set_cell_text", "row": 99, "col": 0, "text": "x" }),
            ErrorCode::ValueOutOfRange,
        ),
        ("delete_rows removing all rows", json!({ "op": "table", "node": t_id, "action": "delete_rows", "at": 0, "count": 3 }), ErrorCode::InvalidArguments),
        (
            "move_row splitting a merge",
            json!({ "op": "table", "node": t_id, "action": "move_row", "from_index": 0, "to_index": 2 }),
            ErrorCode::InvalidArguments,
        ),
        (
            "merge overlapping an existing merge",
            json!({ "op": "table", "node": t_id, "action": "merge", "row": 1, "col": 1, "row_span": 2, "col_span": 2 }),
            ErrorCode::InvalidArguments,
        ),
        ("unmerge on an unmerged cell", json!({ "op": "table", "node": t_id, "action": "unmerge", "row": 2, "col": 2 }), ErrorCode::InvalidArguments),
    ];

    for (label, bad_op, expected_code) in cases {
        // Prepend a valid mutation op (index 0) so the failing op is at index 1.
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
        assert_eq!(err.failed_op, Some(1), "{label}: expected failed_op == Some(1), got {err:?}");
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
    assert_eq!(err.failed_op, Some(0));
    assert_eq!(err.cause.as_ref().map(|c| c.code), Some(ErrorCode::InvalidArguments));
    assert_eq!(designos_mcp::tools::batch::batch_state_hash(&slides_engine.doc, &[]), slides_hash);
}

#[test]
fn schema_and_description_list_table_and_example() {
    let tools = designos_mcp::engine::tools();
    let batch = tools.iter().find(|t| t["name"] == "designos_apply_batch").expect("designos_apply_batch tool");
    let ops_schema = &batch["inputSchema"]["properties"]["ops"];
    let op_enum = ops_schema["items"]["properties"]["op"]["enum"].as_array().expect("op enum array");
    assert!(op_enum.contains(&json!("table")), "op enum must contain 'table': {op_enum:?}");

    let ops_desc = ops_schema["description"].as_str().unwrap_or("");
    let top_desc = batch["description"].as_str().unwrap_or("");
    assert!(ops_desc.contains("table") && top_desc.contains("table"), "both top-level and ops descriptions must mention 'table'");

    let examples = batch["inputSchema"]["examples"].as_array().expect("examples array");
    let has_table_example = examples.iter().any(|ex| ex["ops"].as_array().is_some_and(|ops| ops.iter().any(|o| o["op"] == "table" && o["action"] == "create")));
    assert!(has_table_example && top_desc.contains("\"op\":\"table\""), "must include a table example");
}

#[test]
fn generic_node_tools_on_tables_and_cells() {
    let mut e = Engine::new();
    let res = e.call("designos_apply_batch", json!({ "ops": [{ "op": "table", "as": "t", "action": "create", "rows": 2, "cols": 2 }] })).unwrap();
    let table_id = res["name_to_id"]["t"].as_u64().unwrap();
    let cell_id = res["name_to_id"]["t/r0c0"].as_u64().unwrap();
    let snap_hash = designos_mcp::tools::batch::batch_state_hash(&e.doc, &[]);

    // 1. designos_update_node refuses x, y, width, height on a TableCell ("use the batch `table` op").
    for bad_update in [
        json!({ "node_id": cell_id, "x": 10.0 }),
        json!({ "node_id": cell_id, "y": 10.0 }),
        json!({ "node_id": cell_id, "width": 200.0 }),
        json!({ "node_id": cell_id, "height": 80.0 }),
    ] {
        let err = e.call("designos_update_node", bad_update.clone()).expect_err("cell geometry update must fail");
        assert!(err.message.to_lowercase().contains("use the batch `table` op"), "expected 'use the batch `table` op' in error for {bad_update}: {err:?}");
        assert_eq!(designos_mcp::tools::batch::batch_state_hash(&e.doc, &[]), snap_hash);
    }

    // 2. designos_create_node with parent_id = a Table returns the table container guard error.
    let create_err = e.call("designos_create_node", json!({ "parent_id": table_id, "kind": "rectangle" })).expect_err("create_node under Table must fail");
    assert!(create_err.message.contains("A table holds only cells"), "expected table guard message, got: {create_err:?}");
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

#[test]
fn update_node_and_batch_update_reject_rotation() {
    let tools = designos_mcp::engine::tools();
    let batch = tools.iter().find(|t| t["name"] == "designos_apply_batch").unwrap();
    let op_props = batch["inputSchema"]["properties"]["ops"]["items"]["properties"].as_object().unwrap();
    assert!(!op_props.contains_key("rotation"), "batch op_properties must not expose unrequested rotation parameter");

    let mut e = Engine::new();
    let created = e.call("designos_create_node", json!({ "parent_id": 1, "kind": "rectangle", "name": "Rect", "width": 100.0, "height": 50.0 })).unwrap();
    let rect_id = NodeId(created["node"]["id"].as_u64().unwrap());
    // Set horizontal flip on the rectangle (`transform.a = -1.0`).
    e.doc.get_mut(rect_id).unwrap().transform.a = -1.0;

    // `designos_update_node` (`#[serde(deny_unknown_fields)]`) must reject `rotation` and preserve the flip on `transform`.
    let err_single = e
        .call("designos_update_node", json!({ "node_id": rect_id.0, "name": "RectFlipped", "rotation": 45.0 }))
        .expect_err("designos_update_node with rotation must fail");
    assert!(err_single.message.contains("rotation"), "expected unknown field rotation error, got {err_single:?}");
    assert_eq!(e.doc.get(rect_id).unwrap().transform.a, -1.0, "designos_update_node must not overwrite transform or erase flips via rotation");

    // Batch `update` (`#[serde(deny_unknown_fields)]`) must also reject `rotation`.
    let err = e
        .call("designos_apply_batch", json!({ "ops": [{ "op": "update", "node": rect_id.0, "rotation": 45.0 }] }))
        .expect_err("batch update with rotation must fail");
    assert!(err.message.contains("rotation"), "expected unknown field rotation error, got {err:?}");
}

#[test]
fn metadata_failed_op_2500_keys_and_connect_schema() {
    // 1. Metadata XML after table batch shows <table> with rows="4" cols="3", <table-cell> with rowspan="2" colspan="2", and cell text.
    let mut e = Engine::new();
    e.call("designos_apply_batch", sample_table_ops()).expect("table batch");
    let meta = e.call("designos_get_metadata", json!({ "page_id": 1 })).expect("designos_get_metadata");
    let xml = meta["xml"].as_str().expect("metadata xml string");
    assert!(xml.contains("<table ") && xml.contains("rows=\"4\" cols=\"3\""), "metadata XML must show 4x3 table: {xml}");
    assert!(xml.contains("<table-cell ") && xml.contains("rowspan=\"2\" colspan=\"2\""), "metadata XML must show 2x2 merged table-cell: {xml}");
    assert!(xml.contains("R0C0") && xml.contains("R2C2"), "metadata XML must include cell texts: {xml}");

    // 2. Merge-overlap refusal reports `failed_op == Some(1)`.
    let table_id = e.doc.get(NodeId(1)).unwrap().children[0].0;
    let err = e
        .call(
            "designos_apply_batch",
            json!({
                "ops": [
                    { "op": "table", "node": table_id, "action": "set_cell_text", "row": 2, "col": 2, "text": "ok" },
                    { "op": "table", "node": table_id, "action": "merge", "row": 1, "col": 1, "row_span": 2, "col_span": 2 }
                ]
            }),
        )
        .expect_err("overlapping merge must fail");
    assert_eq!(err.failed_op, Some(1), "overlapping merge at op index 1 must set failed_op=Some(1)");

    // 3. Connect endpoint schema in `designos_apply_batch` is unchanged from base (`minimum: 1` and original description).
    let tools = designos_mcp::engine::tools();
    let batch = tools.iter().find(|t| t["name"] == "designos_apply_batch").unwrap();
    let from_schema = &batch["inputSchema"]["properties"]["ops"]["items"]["properties"]["from"];
    assert_eq!(from_schema["anyOf"][0]["minimum"], json!(1), "connect_end integer minimum must remain 1");
    assert_eq!(
        from_schema["description"],
        json!("Connector endpoint: numeric node ID, '$prev', '@<as>' / '@<as>/<name path>', or {\"point\": [x, y]} in page space."),
        "connect_end description must remain unchanged"
    );
}

#[test]
fn reconnect_skips_failed_table_patch_and_keeps_unrelated_edits() {
    use designos_doc::live_patch::{LiveOp, LivePatch, SiblingHint};
    use designos_doc::{Node, NodeKind, Rect};
    use designos_mcp::tools::ledger_reconnect::replay_unabsorbed_patches_counted;

    let mut e = Engine::new();
    let res_create = e.call("designos_apply_batch", json!({ "ops": [{ "op": "table", "as": "t", "action": "create", "rows": 2, "cols": 2 }] })).unwrap();
    let r0c0_id = NodeId(res_create["name_to_id"]["t/r0c0"].as_u64().unwrap());
    let r0c1_id = NodeId(res_create["name_to_id"]["t/r0c1"].as_u64().unwrap());
    let r0c0_kind = e.doc.get(r0c0_id).unwrap().kind.clone();
    let base_doc = e.doc.clone();
    let page = base_doc.pages[0];

    // Patch 1: valid unrelated rectangle created by an earlier tool call.
    let mut tmp_doc = base_doc.clone();
    let rect_id = tmp_doc.alloc_id();
    let rect_node = Node::rectangle(rect_id, Rect::new(10.0, 10.0, 100.0, 50.0));
    let p1 = LivePatch::new(vec![LiveOp::Insert { parent: page, at: SiblingHint::default(), nodes: vec![rect_node] }]);

    // Patch 2: broken table Update patch (moves r0c1 onto r0c0's (row, col), which fails `check_table_positions`).
    let mut set = serde_json::Map::new();
    set.insert("kind".to_string(), serde_json::to_value(&r0c0_kind).unwrap());
    let p2 = LivePatch::new(vec![LiveOp::Update { id: r0c1_id, set, unset: Vec::new() }]);

    let log = format!("{}\n{}\n", serde_json::to_string(&p1).unwrap(), serde_json::to_string(&p2).unwrap());
    let mut reloaded = base_doc.clone();
    let (applied, refused) = replay_unabsorbed_patches_counted(&mut reloaded, log.as_bytes(), None).expect("reconnect replay succeeds");
    assert_eq!(applied, 1);
    assert_eq!(refused.len(), 1);
    assert!(reloaded.get(rect_id).is_some(), "valid unrelated rectangle patch p1 must NOT be discarded when broken table patch p2 fails");
    let NodeKind::TableCell(cp1) = &reloaded.get(r0c1_id).unwrap().kind else { unreachable!() };
    let NodeKind::TableCell(cp0) = &r0c0_kind else { unreachable!() };
    assert_ne!(cp1.col, cp0.col, "broken table patch p2 must be rolled back");
}

#[test]
fn commit_point_checks_roll_back_broken_table_in_batch_and_reconnect() {
    use designos_doc::live_patch::{LiveOp, LivePatch};
    use designos_doc::NodeKind;
    use designos_mcp::tools::ledger_reconnect::replay_unabsorbed_patches;

    // Part A: `batch.rs` commit-point `check_table_positions` catches a broken table touched by a batch `update` op
    // (where `History::exec` does not run `check_table_positions` because `table::touched_tables` ignores cell fill updates).
    let mut e = Engine::new();
    let res = e.call("designos_apply_batch", json!({ "ops": [{ "op": "table", "as": "t", "action": "create", "rows": 2, "cols": 2 }] })).unwrap();
    let r0c0 = NodeId(res["name_to_id"]["t/r0c0"].as_u64().unwrap());
    let r0c1 = NodeId(res["name_to_id"]["t/r0c1"].as_u64().unwrap());
    let r0c0_col = match &e.doc.get(r0c0).unwrap().kind {
        NodeKind::TableCell(cp) => cp.col,
        _ => unreachable!(),
    };
    // Corrupt r0c1 so both cells sit at (row 0, col 0).
    if let NodeKind::TableCell(cp) = &mut e.doc.get_mut(r0c1).unwrap().kind {
        cp.col = r0c0_col;
    }
    let err = e
        .call("designos_apply_batch", json!({ "ops": [{ "op": "update", "node": r0c0.0, "fill": "#334455" }] }))
        .expect_err("batch touching a table with broken cell positions must be refused at the batch.rs commit point");
    assert!(
        err.message.contains("duplicate") || err.message.contains("cell") || err.message.contains("position"),
        "expected check_table_positions error, got: {err:?}"
    );
    assert!(e.doc.get(r0c0).unwrap().fills.is_empty(), "failed commit-point check must roll back the cell fill update");

    // Part B: `ledger_reconnect.rs` per-patch checked replay rejects a `LiveOp::Update` patch that moves a cell
    // onto another cell's (row, col).
    let mut e2 = Engine::new();
    let res2 = e2.call("designos_apply_batch", json!({ "ops": [{ "op": "table", "as": "t", "action": "create", "rows": 2, "cols": 2 }] })).unwrap();
    let r0c0_2 = NodeId(res2["name_to_id"]["t/r0c0"].as_u64().unwrap());
    let r0c1_2 = NodeId(res2["name_to_id"]["t/r0c1"].as_u64().unwrap());
    let r0c0_kind = e2.doc.get(r0c0_2).unwrap().kind.clone();
    let orig_r0c1_col = match &e2.doc.get(r0c1_2).unwrap().kind {
        NodeKind::TableCell(cp) => cp.col,
        _ => unreachable!(),
    };

    let mut set = serde_json::Map::new();
    set.insert("kind".to_string(), serde_json::to_value(&r0c0_kind).unwrap());
    let patch = LivePatch::new(vec![LiveOp::Update { id: r0c1_2, set, unset: Vec::new() }]);
    let log = format!("{}\n", serde_json::to_string(&patch).unwrap());

    let mut reloaded = e2.doc.clone();
    let _ = replay_unabsorbed_patches(&mut reloaded, log.as_bytes(), None);
    let NodeKind::TableCell(cp_after) = &reloaded.get(r0c1_2).unwrap().kind else { unreachable!() };
    assert_eq!(cp_after.col, orig_r0c1_col, "ledger_reconnect checked replay must reject and roll back duplicate cell position Update patch");
}

#[test]
fn doc_limits_and_resize_semantics_without_mcp_clamps() {
    // 1. Schema matches designos_doc constants (TABLE_MAX_TRACKS = 500, TABLE_MIN_TRACK = 24).
    let tools = designos_mcp::engine::tools();
    let batch = tools.iter().find(|t| t["name"] == "designos_apply_batch").unwrap();
    let props = &batch["inputSchema"]["properties"]["ops"]["items"]["properties"];
    assert_eq!(props["rows"]["maximum"], json!(500));
    assert_eq!(props["cols"]["maximum"], json!(500));
    assert_eq!(props["count"]["maximum"], json!(500));
    assert_eq!(props["col_width"]["minimum"], json!(24));
    assert_eq!(props["row_height"]["minimum"], json!(24));
    assert_eq!(props["size"]["minimum"], json!(24));

    // 2. Creating a 1x101 table with row_height=24 succeeds (no MCP-only 100-track cap or 28px row clamp).
    let mut e = Engine::new();
    let res = e
        .call(
            "designos_apply_batch",
            json!({
                "ops": [{ "op": "table", "as": "wide", "action": "create", "rows": 1, "cols": 101, "col_width": 24.0, "row_height": 24.0 }]
            }),
        )
        .expect("1x101 table with 24px row_height must succeed under designos_doc limits");
    let wide_id = NodeId(res["name_to_id"]["wide"].as_u64().unwrap());
    let NodeKind::Table(tp) = &e.doc.get(wide_id).unwrap().kind else { unreachable!() };
    assert_eq!(tp.cols.len(), 101);
    assert_eq!(tp.rows.len(), 1);
    assert_eq!(tp.rows[0].size, 24.0);

    // 3. `resize` changes only the requested dimension and uses TABLE_MIN_TRACK (24.0) for rows.
    let res2 = e
        .call(
            "designos_apply_batch",
            json!({
                "ops": [
                    { "op": "table", "as": "t", "action": "create", "rows": 2, "cols": 2, "col_width": 100.0, "row_height": 40.0 },
                    { "op": "table", "node": "@t", "action": "resize", "width": 300.0 }
                ]
            }),
        )
        .expect("width-only resize must succeed");
    let t_id = NodeId(res2["name_to_id"]["t"].as_u64().unwrap());
    let t_node = e.doc.get(t_id).unwrap();
    assert_eq!(t_node.size.x, 300.0, "width must scale to 300.0");
    assert_eq!(t_node.size.y, 80.0, "height must stay unchanged at 80.0 when omitted");

    e.call("designos_apply_batch", json!({ "ops": [{ "op": "table", "node": t_id.0, "action": "resize", "height": 48.0 }] }))
        .expect("height-only resize to 2 * 24px must succeed");
    let t_node = e.doc.get(t_id).unwrap();
    assert_eq!(t_node.size.x, 300.0, "width must stay 300.0 when height-only resize is given");
    assert_eq!(t_node.size.y, 48.0, "height must scale to 48.0 (24px per row, not clamped to 28px)");
}

#[test]
fn new_cells_keep_designos_doc_style_without_mcp_hardcoding() {
    let mut e = Engine::new();
    let res = e
        .call(
            "designos_apply_batch",
            json!({
                "ops": [
                    { "op": "table", "as": "t", "action": "create", "rows": 2, "cols": 2 },
                    { "op": "table", "node": "@t", "action": "insert_rows", "at": 2, "count": 1 },
                    { "op": "table", "node": "@t", "action": "insert_cols", "at": 2, "count": 1 }
                ]
            }),
        )
        .expect("create and insert_rows/insert_cols");
    let t_id = NodeId(res["name_to_id"]["t"].as_u64().unwrap());
    let expected_text = designos_doc::table::default_cell_text("");
    for (r, c) in [(0, 0), (2, 0), (0, 2), (2, 2)] {
        let cid = cell_at(&e.doc, t_id, r, c);
        let cell_node = e.doc.get(cid).unwrap();
        assert!(cell_node.fills.is_empty(), "MCP must not hardcode dark #1E1E22 cell fills at ({r},{c})");
        let NodeKind::TableCell(cp) = &cell_node.kind else { unreachable!() };
        assert_eq!(cp.text.style.font_size, expected_text.style.font_size, "cell ({r},{c}) font_size must match designos_doc default (16.0)");
        assert_eq!(cp.text.style.fills, expected_text.style.fills, "cell ({r},{c}) text fills must match designos_doc default, not MCP #B3B3B5");
    }
}

#[test]
fn tool_description_names_table_and_resize_action() {
    let tools = designos_mcp::engine::tools();
    let batch = tools.iter().find(|t| t["name"] == "designos_apply_batch").unwrap();
    let top_desc = batch["description"].as_str().unwrap();
    let ops_desc = batch["inputSchema"]["properties"]["ops"]["description"].as_str().unwrap();

    assert!(top_desc.contains("table)") && top_desc.contains("(5) table:"), "top-level description must name `table` and show (5) table: example");
    assert!(ops_desc.contains("|resize|"), "ops.description must list `resize` as the table resize action");
    assert!(!ops_desc.contains("resize_table"), "ops.description must not contain `resize_table`");
}

#[test]
fn batch_table_ops_replay_into_editor_document() {
    use designos_doc::live_patch::{replay_live_patch_checked, LivePatch, LivePatchState};

    let mut e = Engine::new();
    let mut editor = e.doc.clone();
    let mut state = LivePatchState::default();
    e.live_ops = Some(Vec::new());

    let res = e.call("designos_apply_batch", sample_table_ops()).expect("batch table ops succeed");
    let table_id = NodeId(res["name_to_id"]["t"].as_u64().unwrap());
    let live_ops = e.take_live_ops();
    assert!(!live_ops.is_empty(), "batch table ops must record live ops");

    let report = replay_live_patch_checked(&mut editor, &LivePatch::new(live_ops), &mut state).expect("recorded batch table ops replay cleanly");
    assert!(report.skipped.is_empty(), "no live ops skipped: {:?}", report.skipped);
    designos_doc::check_table_positions(&editor, table_id).expect("replayed editor table holds valid cell positions");

    // Second batch covering the remaining table actions (`insert_cols`, `delete_rows`, `delete_cols`,
    // `move_row`, `move_col`, `resize_row`, `resize_col`, `unmerge`, `resize`).
    e.call(
        "designos_apply_batch",
        json!({
            "ops": [
                { "op": "table", "node": table_id.0, "action": "unmerge", "row": 0, "col": 0 },
                { "op": "table", "node": table_id.0, "action": "insert_cols", "at": 1, "count": 1 },
                { "op": "table", "node": table_id.0, "action": "delete_rows", "at": 3, "count": 1 },
                { "op": "table", "node": table_id.0, "action": "delete_cols", "at": 1, "count": 1 },
                { "op": "table", "node": table_id.0, "action": "move_row", "from_index": 0, "to_index": 2 },
                { "op": "table", "node": table_id.0, "action": "move_col", "from_index": 2, "to_index": 0 },
                { "op": "table", "node": table_id.0, "action": "resize_row", "at": 0, "size": 56.0 },
                { "op": "table", "node": table_id.0, "action": "resize_col", "at": 1, "size": 180.0 },
                { "op": "table", "node": table_id.0, "action": "resize", "width": 540.0, "height": 180.0 }
            ]
        }),
    )
    .expect("second batch of table actions succeeds");
    let live_ops2 = e.take_live_ops();
    let report2 = replay_live_patch_checked(&mut editor, &LivePatch::new(live_ops2), &mut state).expect("second batch table ops replay cleanly");
    assert!(report2.skipped.is_empty(), "no live ops skipped: {:?}", report2.skipped);
    designos_doc::check_table_positions(&editor, table_id).expect("replayed editor table holds valid cell positions");

    let agent_table = e.doc.get(table_id).unwrap();
    let editor_table = editor.get(table_id).unwrap();
    assert_eq!(serde_json::to_value(editor_table).unwrap(), serde_json::to_value(agent_table).unwrap());
    for &cid in &agent_table.children {
        assert_eq!(serde_json::to_value(editor.get(cid).unwrap()).unwrap(), serde_json::to_value(e.doc.get(cid).unwrap()).unwrap());
    }
}

#[test]
fn move_row_and_move_col_use_from_index_to_index_and_validate_against_input_schema() {
    let tools = designos_mcp::engine::tools();
    let batch = tools.iter().find(|t| t["name"] == "designos_apply_batch").expect("designos_apply_batch tool");
    let op_props = &batch["inputSchema"]["properties"]["ops"]["items"]["properties"];
    assert_eq!(op_props["from_index"], json!({ "type": "integer", "minimum": 0 }));
    assert_eq!(op_props["to_index"], json!({ "type": "integer", "minimum": 0 }));
    let ops_desc = batch["inputSchema"]["properties"]["ops"]["description"].as_str().unwrap_or("");
    assert!(ops_desc.contains("from_index/to_index"), "ops description must mention from_index/to_index: {ops_desc}");

    let validator = jsonschema::validator_for(&batch["inputSchema"]).expect("compile published designos_apply_batch inputSchema");

    // 1. A move_col with from_index 2, to_index 0 is accepted by the published inputSchema and moves the 3rd column to the front.
    let move_col_args = json!({
        "ops": [
            { "op": "table", "as": "t", "action": "create", "parent": 1, "rows": 2, "cols": 3 },
            { "op": "table", "node": "@t", "action": "set_cell_text", "row": 0, "col": 0, "text": "C0" },
            { "op": "table", "node": "@t", "action": "set_cell_text", "row": 0, "col": 1, "text": "C1" },
            { "op": "table", "node": "@t", "action": "set_cell_text", "row": 0, "col": 2, "text": "C2" },
            { "op": "table", "node": "@t", "action": "move_col", "from_index": 2, "to_index": 0 }
        ]
    });
    if let Err(err) = validator.validate(&move_col_args) {
        panic!("move_col with from_index=2, to_index=0 failed inputSchema validation: {err}");
    }

    let mut e = Engine::new();
    let res = e.call("designos_apply_batch", move_col_args).expect("move_col with from_index=2, to_index=0 must succeed");
    let table_id = NodeId(res["name_to_id"]["t"].as_u64().unwrap());
    let col_texts: Vec<String> = (0..3)
        .map(|c| {
            let cid = cell_at(&e.doc, table_id, 0, c);
            let NodeKind::TableCell(cp) = &e.doc.get(cid).unwrap().kind else { unreachable!() };
            cp.text.content.clone()
        })
        .collect();
    assert_eq!(col_texts, vec!["C2", "C0", "C1"], "move_col from_index=2, to_index=0 must move the 3rd column to the front");

    // 2. A connector op with from: 0 is still rejected by the published inputSchema and at runtime.
    let bad_connect_args = json!({
        "ops": [
            { "op": "connect", "from": 0, "to": 1 }
        ]
    });
    assert!(validator.validate(&bad_connect_args).is_err(), "connector op with from: 0 must be rejected by the published inputSchema");
    assert!(e.call("designos_apply_batch", bad_connect_args).is_err(), "connector op with from: 0 must be rejected at runtime");

    // 3. move_row with from_index 0 is accepted by the published inputSchema and works at runtime.
    let move_row_args = json!({
        "ops": [
            { "op": "table", "node": table_id.0, "action": "set_cell_text", "row": 1, "col": 0, "text": "R1C0" },
            { "op": "table", "node": table_id.0, "action": "move_row", "from_index": 0, "to_index": 1 }
        ]
    });
    if let Err(err) = validator.validate(&move_row_args) {
        panic!("move_row with from_index=0, to_index=1 failed inputSchema validation: {err}");
    }
    e.call("designos_apply_batch", move_row_args).expect("move_row with from_index=0, to_index=1 must succeed");
    let r0c0 = cell_at(&e.doc, table_id, 0, 0);
    let r1c0 = cell_at(&e.doc, table_id, 1, 0);
    let NodeKind::TableCell(cp_r0) = &e.doc.get(r0c0).unwrap().kind else { unreachable!() };
    let NodeKind::TableCell(cp_r1) = &e.doc.get(r1c0).unwrap().kind else { unreachable!() };
    assert_eq!(cp_r0.text.content, "R1C0", "row 1 must move to row 0");
    assert_eq!(cp_r1.text.content, "C2", "row 0 must move to row 1");
}
