//! Round-trip and lint regression tests for text truncation (`TextAutoResize::Truncate` and `max_lines`).
//!
//! `designos-mcp` has no setter for either field, so these tests set truncation directly on the document
//! and check that the tools preserve it and that the lint findings do not change.
#![allow(clippy::result_large_err)]

use designos_doc::{NodeId, NodeKind, TextAutoResize};
use designos_mcp::engine::Engine;
use serde_json::{json, Value};

fn default_page(e: &Engine) -> u64 {
    e.resolve_default_page().unwrap().0
}

#[test]
fn apply_spec_and_update_node_preserve_truncation_and_roundtrip() {
    let mut e = Engine::new();
    let page = default_page(&e);

    // 1. Build a component with a text layer via designos_apply_spec, then configure truncation + max_lines.
    let comp_res = e
        .call(
            "designos_apply_spec",
            json!({
                "parent_id": page,
                "spec": {
                    "kind": "component",
                    "name": "TruncCard",
                    "width": 200,
                    "height": 64,
                    "children": [
                        {
                            "kind": "text",
                            "name": "Excerpt",
                            "text": "Initial copy",
                            "x": 12,
                            "y": 12,
                            "width": 160,
                            "height": 38,
                            "font_family": "Inter",
                            "font_size": 14
                        }
                    ]
                }
            }),
        )
        .expect("create TruncCard component");
    let _comp_id = comp_res["root_node_id"].as_u64().unwrap();
    let excerpt_id = NodeId(comp_res["name_to_id"]["TruncCard/Excerpt"].as_u64().unwrap());

    {
        let node = e.doc.get_mut(excerpt_id).expect("Excerpt node");
        if let NodeKind::Text(tp) = &mut node.kind {
            tp.auto_resize = TextAutoResize::Truncate;
            tp.max_lines = Some(2);
        }
    }

    // 2. Instantiate the component via designos_apply_spec with a long text override;
    //    the instance's text child must keep TextAutoResize::Truncate, max_lines = Some(2), and its fixed 160x38 box.
    let inst_res = e
        .call(
            "designos_apply_spec",
            json!({
                "parent_id": page,
                "spec": {
                    "kind": "instance",
                    "name": "TruncCardInst",
                    "component": "TruncCard",
                    "x": 240,
                    "y": 0,
                    "overrides": [
                        {
                            "path": "Excerpt",
                            "text": "A much longer sentence that wraps across several lines inside the card without growing its fixed height."
                        }
                    ]
                }
            }),
        )
        .expect("instantiate TruncCard");
    let inst_id = NodeId(inst_res["root_node_id"].as_u64().unwrap());
    let inst_excerpt_id = e.doc.get(inst_id).unwrap().children[0];

    let got_inst = e.call("designos_get_node", json!({"node_id": inst_excerpt_id.0})).expect("get instance excerpt");
    assert_eq!(got_inst["node"]["kind"]["Text"]["auto_resize"], json!("Truncate"));
    assert_eq!(got_inst["node"]["kind"]["Text"]["max_lines"], json!(2));
    assert_eq!(got_inst["node"]["size"]["x"], json!(160.0));
    assert_eq!(got_inst["node"]["size"]["y"], json!(38.0));

    // 3. Update the truncating text layer via designos_update_node (text and width/height);
    //    truncation mode and max_lines must remain intact.
    e.call(
        "designos_update_node",
        json!({
            "node_id": excerpt_id.0,
            "text": "Updated body text that exceeds two lines when rendered inside the narrow box.",
            "width": 176.0,
            "height": 40.0
        }),
    )
    .expect("update_node on truncating text");

    let got = e.call("designos_get_node", json!({"node_id": excerpt_id.0})).expect("get updated excerpt");
    assert_eq!(got["node"]["kind"]["Text"]["auto_resize"], json!("Truncate"));
    assert_eq!(got["node"]["kind"]["Text"]["max_lines"], json!(2));
    assert_eq!(got["node"]["size"]["x"], json!(176.0));
    assert_eq!(got["node"]["size"]["y"], json!(40.0));

    // 4. Round-trip through .designos JSON and .fig Kiwi binary export/import via Session tools.
    let tmp = std::env::temp_dir().join(format!("designos-trunc-roundtrip-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("create tempdir");
    let designos_path = tmp.join("trunc-roundtrip.designos");

    let json_str = e.doc.to_json().expect("serialize .designos");
    std::fs::write(&designos_path, &json_str).expect("write .designos");

    let mut s = designos_mcp::server::Session::new(&tmp).expect("session");
    s.call("designos_open_file", json!({"path": "trunc-roundtrip.designos", "discard_changes": true})).expect("open .designos");
    let loaded_designos = s.call("designos_get_node", json!({"node_id": excerpt_id.0})).expect("get after .designos open");
    assert_eq!(loaded_designos["node"]["kind"]["Text"]["auto_resize"], json!("Truncate"));
    assert_eq!(loaded_designos["node"]["kind"]["Text"]["max_lines"], json!(2));

    s.call("designos_export_fig_file", json!({"output_path": "trunc-roundtrip.fig"})).expect("export .fig");
    s.call("designos_open_file", json!({"path": "trunc-roundtrip.fig", "discard_changes": true})).expect("open .fig");
    let query_fig = s.call("designos_query_xpath", json!({"selector": "//TEXT[@name='Excerpt']"})).expect("query Excerpt in .fig");
    let rows = query_fig["rows"].as_array().expect("rows");
    assert!(!rows.is_empty(), "expected Excerpt text nodes in .fig");
    let mut matched_updated = false;
    for r in rows {
        let nid = r[0].as_u64().unwrap();
        let got_fig = s.call("designos_get_node", json!({"node_id": nid})).expect("get_node in .fig");
        let tp = &got_fig["node"]["kind"]["Text"];
        if tp["content"].as_str().unwrap_or_default().starts_with("Updated body") {
            assert_eq!(tp["auto_resize"], json!("Truncate"));
            assert_eq!(tp["max_lines"], json!(2));
            matched_updated = true;
        }
    }
    assert!(matched_updated, "expected Updated body Excerpt in .fig round-trip");
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn truncation_lint_findings_and_xpath_query_are_unchanged() {
    let mut e = Engine::new();
    let page = default_page(&e);

    let res = e
        .call(
            "designos_apply_spec",
            json!({
                "parent_id": page,
                "spec": {
                    "kind": "frame",
                    "name": "LintFrame",
                    "width": 200,
                    "height": 200,
                    "children": [
                        {
                            "kind": "text",
                            "name": "Overflowing",
                            "text": "This long paragraph wraps onto three lines inside a narrow box",
                            "x": 10,
                            "y": 10,
                            "width": 120,
                            "height": 44,
                            "font_family": "Inter",
                            "font_size": 14
                        },
                        {
                            "kind": "text",
                            "name": "FitsOneLine",
                            "text": "Short",
                            "x": 10,
                            "y": 70,
                            "width": 120,
                            "height": 44,
                            "font_family": "Inter",
                            "font_size": 14
                        }
                    ]
                }
            }),
        )
        .expect("apply_spec LintFrame");
    let overflowing_id = NodeId(res["name_to_id"]["LintFrame/Overflowing"].as_u64().unwrap());
    let fits_id = NodeId(res["name_to_id"]["LintFrame/FitsOneLine"].as_u64().unwrap());

    for id in [overflowing_id, fits_id] {
        let n = e.doc.get_mut(id).unwrap();
        if let NodeKind::Text(tp) = &mut n.kind {
            tp.auto_resize = TextAutoResize::Truncate;
            tp.max_lines = Some(1);
        }
    }

    let lint_val = e.call("designos_lint", json!({"page_id": page, "rules": ["text-truncated"]})).expect("lint text-truncated");
    let findings = lint_val["findings"].as_array().expect("findings array");
    let lint_ids: Vec<u64> = findings.iter().filter_map(|f| f["node_id"].as_u64()).collect();
    assert_eq!(lint_ids, vec![overflowing_id.0], "only the overflowing max_lines=1 layer is flagged: {findings:?}");
    assert_eq!(findings[0]["rule"], json!("text-truncated"));
    assert_eq!(findings[0]["suggested_fix"]["property"], json!("textAutoResize"));
    assert_eq!(findings[0]["suggested_fix"]["from"], json!("TRUNCATE"));
    assert_eq!(findings[0]["suggested_fix"]["to"], json!("HEIGHT"));

    let query_val =
        e.call("designos_query_xpath", json!({"selector": "//TEXT[has-problem('text-truncated')]", "page_id": page})).expect("xpath text-truncated");
    let matched_ids: Vec<u64> = query_val["rows"].as_array().unwrap().iter().filter_map(|r: &Value| r[0].as_u64()).collect();
    assert_eq!(matched_ids, vec![overflowing_id.0]);

    // Clearing max_lines to None and giving the box enough height to fit all wrapped lines clears the finding.
    {
        let n = e.doc.get_mut(overflowing_id).unwrap();
        n.size.y = 120.0;
        if let NodeKind::Text(tp) = &mut n.kind {
            tp.max_lines = None;
        }
    }
    let lint_after = e.call("designos_lint", json!({"page_id": page, "rules": ["text-truncated"]})).expect("lint after clearing max_lines");
    assert_eq!(lint_after["findings"].as_array().unwrap().len(), 0, "no truncation finding once max_lines is None and box height fits all lines");
}
