//! MCP tool `designos_get_metadata` (`T7.2`): sparse XML node hierarchy for agent context.

use crate::engine::Engine;
use designos_doc::NodeId;
use designos_export::{emitted_metadata_nodes, metadata_xml, structural_lint_rules_for_nodes, MetadataOptions};
use serde::Deserialize;
use serde_json::{json, Value};

fn default_depth() -> i64 {
    3
}
fn default_max_nodes() -> i64 {
    1_000
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetadataArgs {
    #[serde(default)]
    pub node_id: Option<u64>,
    #[serde(default)]
    pub page_id: Option<u64>,
    #[serde(default = "default_depth")]
    pub depth: i64,
    #[serde(default = "default_max_nodes")]
    pub max_nodes: i64,
    #[serde(default = "default_true")]
    pub include_hidden: bool,
    #[serde(default)]
    pub lint: bool,
}

/// JSON property schema for `designos_get_metadata`.
pub fn properties_schema() -> Value {
    json!({
        "node_id": {"type": "integer", "minimum": 1},
        "page_id": {"type": "integer", "minimum": 1},
        "depth": {"type": "integer", "minimum": 1, "maximum": 10, "default": 3},
        "max_nodes": {"type": "integer", "minimum": 1, "maximum": 5000, "default": 1000},
        "include_hidden": {"type": "boolean", "default": true},
        "lint": {"type": "boolean", "default": false}
    })
}

/// Executes `designos_get_metadata` against `engine`.
pub fn call_get_metadata(engine: &mut Engine, args: Value) -> Result<Value, String> {
    let a: MetadataArgs = serde_json::from_value(args).map_err(|e| format!("Invalid arguments: {e}"))?;

    if a.node_id.is_some() && a.page_id.is_some() {
        return Err("EXACTLY_ONE_SCOPE: specify at most one of node_id or page_id".into());
    }
    if a.node_id == Some(0) || a.page_id == Some(0) {
        return Err("VALUE_OUT_OF_RANGE: node_id and page_id must be >= 1".into());
    }
    if !(1..=10).contains(&a.depth) {
        return Err(format!("VALUE_OUT_OF_RANGE: depth must be in 1..=10, got {}", a.depth));
    }
    if !(1..=5_000).contains(&a.max_nodes) {
        return Err(format!("VALUE_OUT_OF_RANGE: max_nodes must be in 1..=5000, got {}", a.max_nodes));
    }

    let root = if let Some(nid) = a.node_id {
        let id = NodeId(nid);
        if engine.doc.get(id).is_none() {
            return Err(format!("NODE_NOT_FOUND: node {nid} not found"));
        }
        id
    } else if let Some(pid) = a.page_id {
        let id = NodeId(pid);
        if engine.doc.get(id).is_none() {
            return Err(format!("NODE_NOT_FOUND: page {pid} not found"));
        }
        id
    } else {
        engine.resolve_default_page().map_err(|e| format!("NODE_NOT_FOUND: {e}"))?
    };

    engine.settle_layout();

    let opts = MetadataOptions {
        depth: a.depth as u8,
        max_nodes: a.max_nodes as usize,
        include_hidden: a.include_hidden,
        scoped: false,
        in_progress: Default::default(),
    };

    let rev = (engine.doc.runtime_id(), engine.doc.mutation_epoch());
    if a.lint {
        let emitted = emitted_metadata_nodes(&engine.doc, root, &opts);
        let (fresh_hits, checked) = structural_lint_rules_for_nodes(&engine.doc, &emitted, std::time::Duration::from_millis(50));
        engine.update_lint_cache(rev, &checked, &["zero-bounds", "clipped-text", "off-page", "overlapping-siblings"], fresh_hits);
    }

    let cached_map = engine.lint_cache.as_ref().and_then(|(cached_rev, map)| (*cached_rev == rev).then_some(map));
    let lint_ran = a.lint || cached_map.is_some();

    let meta = match cached_map {
        Some(map) => {
            let lookup = |id: NodeId| map.get(&id).copied();
            metadata_xml(&engine.doc, root, &opts, Some(&lookup))
        }
        None => metadata_xml(&engine.doc, root, &opts, None),
    };

    let mut out = json!({
        "root_id": meta.root_id,
        "xml": meta.xml,
        "node_count": meta.node_count,
        "truncated": meta.truncated,
        "remaining_estimate": meta.remaining_estimate,
        "hidden_omitted": meta.hidden_omitted,
        "lint_ran": lint_ran,
    });
    if meta.scoped {
        out["scoped"] = json!(true);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Node, NodeKind, Rect, TextAutoResize, TextStyle};
    use quick_xml::events::Event;
    use quick_xml::Reader;

    fn assert_valid_xml(xml: &str) -> usize {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().check_end_names = true;
        let mut count = 0usize;
        let mut open_depth = 0usize;
        loop {
            match reader.read_event().expect("XML must parse without error") {
                Event::Start(_) => {
                    count += 1;
                    open_depth += 1;
                }
                Event::Empty(_) => {
                    count += 1;
                }
                Event::End(_) => {
                    assert!(open_depth > 0, "unmatched closing tag");
                    open_depth -= 1;
                }
                Event::Eof => break,
                _ => {}
            }
        }
        assert_eq!(open_depth, 0, "all opening tags must be closed");
        count
    }

    #[test]
    fn xml_parses() {
        let mut e = Engine::default();
        let page = e.resolve_default_page().unwrap();
        let fid = e.doc.alloc_id();
        let mut f = Node::frame(fid, Rect::new(0.0, 0.0, 400.0, 300.0));
        f.name = "A & B <\"quote\">\nline2\u{0000}null".to_string();
        e.doc.insert(page, f).unwrap();

        let tid = e.doc.alloc_id();
        let mut t = Node::text(tid, Rect::new(10.0, 10.0, 200.0, 30.0), "Text with & < \" \n and \u{0000} control", TextStyle::default());
        t.name = "Label <1> & \"2\"".to_string();
        e.doc.insert(fid, t).unwrap();

        let res = e.call("designos_get_metadata", json!({})).unwrap();
        let xml = res["xml"].as_str().unwrap();
        assert!(!xml.contains('\u{0000}'), "NUL must be replaced");
        let parsed_elements = assert_valid_xml(xml);
        assert_eq!(parsed_elements, 3);
        assert_eq!(res["node_count"], 3);
    }

    #[test]
    fn count_equals_subtree_to_depth() {
        let mut e = Engine::default();
        let page = e.resolve_default_page().unwrap();
        let root_frame = e.doc.alloc_id();
        e.doc.insert(page, Node::frame(root_frame, Rect::new(0.0, 0.0, 1000.0, 1000.0))).unwrap();

        // Build 3 levels x 4 children under `root_frame`: 1 + 4 + 16 + 64 = 85 nodes
        let mut level1 = Vec::new();
        for i in 0..4 {
            let id1 = e.doc.alloc_id();
            let mut n1 = Node::frame(id1, Rect::new(i as f32 * 100.0, 0.0, 80.0, 80.0));
            n1.name = format!("L1-{i}");
            e.doc.insert(root_frame, n1).unwrap();
            level1.push(id1);
        }
        let mut level2 = Vec::new();
        for &p1 in &level1 {
            for j in 0..4 {
                let id2 = e.doc.alloc_id();
                let mut n2 = Node::frame(id2, Rect::new(j as f32 * 20.0, 0.0, 16.0, 16.0));
                n2.name = format!("L2-{j}");
                e.doc.insert(p1, n2).unwrap();
                level2.push(id2);
            }
        }
        for &p2 in &level2 {
            for k in 0..4 {
                let id3 = e.doc.alloc_id();
                let mut n3 = Node::rectangle(id3, Rect::new(k as f32 * 4.0, 0.0, 3.0, 3.0));
                n3.name = format!("L3-{k}");
                e.doc.insert(p2, n3).unwrap();
            }
        }

        let res_d2 = e.call("designos_get_metadata", json!({"node_id": root_frame.0, "depth": 2})).unwrap();
        assert_eq!(res_d2["node_count"], 21);
        let xml_d2 = res_d2["xml"].as_str().unwrap();
        assert_eq!(assert_valid_xml(xml_d2), 21);
        assert!(xml_d2.contains("children=\"4\" more=\"depth\""));

        let res_d3 = e.call("designos_get_metadata", json!({"node_id": root_frame.0, "depth": 3})).unwrap();
        assert_eq!(res_d3["node_count"], 85);
        let xml_d3 = res_d3["xml"].as_str().unwrap();
        assert_eq!(assert_valid_xml(xml_d3), 85);
        assert!(!xml_d3.contains("more=\"depth\""));
    }

    #[test]
    fn hidden_flagged() {
        let mut e = Engine::default();
        let page = e.resolve_default_page().unwrap();
        let frame = e.doc.alloc_id();
        e.doc.insert(page, Node::frame(frame, Rect::new(0.0, 0.0, 200.0, 200.0))).unwrap();

        let vis = e.doc.alloc_id();
        e.doc.insert(frame, Node::rectangle(vis, Rect::new(0.0, 0.0, 50.0, 50.0))).unwrap();

        let hid = e.doc.alloc_id();
        let mut hnode = Node::rectangle(hid, Rect::new(60.0, 0.0, 50.0, 50.0));
        hnode.name = "SecretLayer".to_string();
        hnode.visible = false;
        e.doc.insert(frame, hnode).unwrap();

        let with_hidden = e.call("designos_get_metadata", json!({"node_id": frame.0, "include_hidden": true})).unwrap();
        let xml_inc = with_hidden["xml"].as_str().unwrap();
        assert!(xml_inc.contains("hidden=\"true\""));
        assert_eq!(with_hidden["hidden_omitted"], 0);
        assert_eq!(with_hidden["node_count"], 3);

        let without_hidden = e.call("designos_get_metadata", json!({"node_id": frame.0, "include_hidden": false})).unwrap();
        let xml_omitted = without_hidden["xml"].as_str().unwrap();
        assert!(!xml_omitted.contains("SecretLayer"));
        assert!(!xml_omitted.contains("hidden=\"true\""));
        assert_eq!(without_hidden["hidden_omitted"], 1);
        assert_eq!(without_hidden["node_count"], 2);
    }

    #[test]
    fn budget_truncates() {
        let mut e = Engine::default();
        let page = e.resolve_default_page().unwrap();
        let root = e.doc.alloc_id();
        e.doc.insert(page, Node::frame(root, Rect::new(0.0, 0.0, 1000.0, 1000.0))).unwrap();

        // 1 root + 199 children = 200 nodes in subtree
        for i in 0..199 {
            let cid = e.doc.alloc_id();
            let mut r = Node::rectangle(cid, Rect::new((i % 10) as f32 * 20.0, (i / 10) as f32 * 20.0, 16.0, 16.0));
            r.name = format!("Item-{i}");
            e.doc.insert(root, r).unwrap();
        }

        let res = e.call("designos_get_metadata", json!({"node_id": root.0, "max_nodes": 50})).unwrap();
        assert_eq!(res["node_count"], 50);
        assert_eq!(res["truncated"], true);
        assert!(res["remaining_estimate"].as_u64().unwrap() > 0);
        let xml = res["xml"].as_str().unwrap();
        assert!(xml.contains("more=\"budget\""));
        assert_eq!(assert_valid_xml(xml), 50);
    }

    #[test]
    fn clipped_text_issue_count() {
        let mut e = Engine::default();
        let page = e.resolve_default_page().unwrap();
        let frame = e.doc.alloc_id();
        e.doc.insert(page, Node::frame(frame, Rect::new(0.0, 0.0, 400.0, 200.0))).unwrap();

        let tid = e.doc.alloc_id();
        let style = TextStyle { font_size: 18.0, ..Default::default() };
        let mut t = Node::text(tid, Rect::new(16.0, 16.0, 40.0, 20.0), "Complete your order right now before items sell out", style);
        if let NodeKind::Text(tp) = &mut t.kind {
            tp.auto_resize = TextAutoResize::None;
        }
        t.name = "Title".to_string();
        e.doc.insert(frame, t).unwrap();

        let res = e.call("designos_get_metadata", json!({"node_id": frame.0, "lint": true})).unwrap();
        assert_eq!(res["lint_ran"], true);
        let xml = res["xml"].as_str().unwrap();
        assert!(
            xml.contains(&format!("id=\"{}\" name=\"Title\"", tid.0)) && xml.contains("issues=\"1\""),
            "expected issues=\"1\" on clipped text node, got:\n{xml}"
        );
    }

    #[test]
    fn bounds_are_page_space() {
        let mut e = Engine::default();
        let page = e.resolve_default_page().unwrap();
        let parent = e.doc.alloc_id();
        let mut pf = Node::frame(parent, Rect::new(100.0, 200.0, 300.0, 300.0));
        pf.name = "Parent".to_string();
        e.doc.insert(page, pf).unwrap();

        let child = e.doc.alloc_id();
        let mut cr = Node::rectangle(child, Rect::new(10.0, 20.0, 50.0, 40.0));
        cr.name = "Child".to_string();
        e.doc.insert(parent, cr).unwrap();

        let res = e.call("designos_get_metadata", json!({"node_id": parent.0})).unwrap();
        let xml = res["xml"].as_str().unwrap();
        assert!(
            xml.contains("name=\"Child\" x=\"110\" y=\"220\" width=\"50\" height=\"40\""),
            "expected page-space bounds x=\"110\" y=\"220\" on child, got:\n{xml}"
        );
    }

    #[test]
    fn scope_and_range_errors() {
        let mut e = Engine::default();
        let err_both = e.call("designos_get_metadata", json!({"node_id": 1, "page_id": 1})).unwrap_err();
        assert!(err_both.contains("EXACTLY_ONE_SCOPE"), "{err_both}");

        let err_depth = e.call("designos_get_metadata", json!({"depth": 11})).unwrap_err();
        assert!(err_depth.contains("VALUE_OUT_OF_RANGE"), "{err_depth}");

        let err_max = e.call("designos_get_metadata", json!({"max_nodes": 5001})).unwrap_err();
        assert!(err_max.contains("VALUE_OUT_OF_RANGE"), "{err_max}");

        let err_missing = e.call("designos_get_metadata", json!({"node_id": 99999})).unwrap_err();
        assert!(err_missing.contains("NODE_NOT_FOUND"), "{err_missing}");
    }

    #[test]
    fn scoped_canvas_shells_emit_children_zero_and_dangling_comment_does_not_false_positive() {
        use designos_doc::{CommentAnchor, CommentThread, Vec2};

        // 1. A full (unscoped) document with a dangling comment anchor must NOT set scoped: true or children="0".
        let mut e = Engine::default();
        let page = e.resolve_default_page().unwrap();
        let empty_frame = e.doc.alloc_id();
        let mut f = Node::frame(empty_frame, Rect::new(0.0, 0.0, 400.0, 300.0));
        f.name = "EmptyFrame".to_string();
        e.doc.insert(page, f).unwrap();
        e.doc.comments.push(CommentThread {
            id: "c1".into(),
            anchor: CommentAnchor { page, node: Some(NodeId(99_999)), offset: Vec2::ZERO },
            resolved: false,
            messages: Vec::new(),
        });

        let full_res = e.call("designos_get_metadata", json!({})).unwrap();
        assert!(full_res.get("scoped").is_none(), "dangling comment on full file must not set scoped: true: {full_res}");
        let full_xml = full_res["xml"].as_str().unwrap();
        assert!(!full_xml.contains("children=\"0\""), "empty frame on full file must not get children=\"0\":\n{full_xml}");

        // 2. A scoped canvas built from a page with populated frames (no comments or comment on shell)
        // must set scoped: true and emit children="0" only on the shell frames.
        let mut src = designos_doc::Document::new("LargeDoc");
        let spage = src.add_page("Page 1");
        let screen_shell = src.alloc_id();
        let mut s1 = Node::frame(screen_shell, Rect::new(0.0, 0.0, 1440.0, 900.0));
        s1.name = "Screen 7".to_string();
        src.insert(spage, s1).unwrap();
        let s1_child = src.alloc_id();
        src.insert(screen_shell, Node::rectangle(s1_child, Rect::new(10.0, 10.0, 100.0, 40.0))).unwrap();

        let screen_selected = src.alloc_id();
        let mut s2 = Node::frame(screen_selected, Rect::new(1600.0, 0.0, 1440.0, 900.0));
        s2.name = "Screen 8".to_string();
        src.insert(spage, s2).unwrap();
        let s2_empty_inner = src.alloc_id();
        let mut inner = Node::frame(s2_empty_inner, Rect::new(20.0, 20.0, 200.0, 100.0));
        inner.name = "InnerEmpty".to_string();
        src.insert(screen_selected, inner).unwrap();

        // Simulate `scoped_canvas(&src, spage, &[screen_selected], "LargeDoc")`
        let mut scoped_doc = designos_doc::Document::new("LargeDoc");
        scoped_doc.pages.push(spage);
        let mut p = src.get(spage).unwrap().clone();
        p.fig_metadata.clear();
        p.fig_metadata.insert("designos:scoped".into(), "true".into());
        scoped_doc.nodes.insert(spage, p);

        // Screen 7 is unselected -> stripped shell
        let mut shell_copy = src.get(screen_shell).unwrap().clone();
        shell_copy.children.clear();
        shell_copy.fill_geometry = None;
        shell_copy.stroke_geometry = None;
        shell_copy.fig_metadata.clear();
        shell_copy.fig_metadata.insert("designos:scoped_shell".into(), "true".into());
        scoped_doc.nodes.insert(screen_shell, shell_copy);

        // Screen 8 is selected -> full subtree copied
        let mut sel_copy = src.get(screen_selected).unwrap().clone();
        sel_copy.fig_metadata.clear();
        scoped_doc.nodes.insert(screen_selected, sel_copy);
        let mut inner_copy = src.get(s2_empty_inner).unwrap().clone();
        inner_copy.fig_metadata.clear();
        scoped_doc.nodes.insert(s2_empty_inner, inner_copy);

        let mut scoped_engine = Engine { doc: scoped_doc, active_page_id: Some(spage), ..Default::default() };
        let scoped_res = scoped_engine.call("designos_get_metadata", json!({})).unwrap();
        assert_eq!(scoped_res["scoped"], json!(true), "scoped canvas must report scoped: true");
        let scoped_xml = scoped_res["xml"].as_str().unwrap();
        assert!(
            scoped_xml.contains(&format!("id=\"{}\" name=\"Screen 7\" x=\"0\" y=\"0\" width=\"1440\" height=\"900\" children=\"0\"/>", screen_shell.0)),
            "shell frame must carry children=\"0\":\n{scoped_xml}"
        );
        assert!(
            !scoped_xml.contains("name=\"InnerEmpty\" x=\"1620\" y=\"20\" width=\"200\" height=\"100\" children=\"0\""),
            "genuinely empty inner frame inside expanded subtree must not carry children=\"0\":\n{scoped_xml}"
        );
    }

    #[test]
    fn lint_only_checks_emitted_nodes_and_merges_across_writers_without_dropping_rules() {
        let mut e = Engine::default();
        let page = e.resolve_default_page().unwrap();
        let frame = e.doc.alloc_id();
        let mut f = Node::frame(frame, Rect::new(0.0, 0.0, 400.0, 200.0));
        f.name = "Card".to_string();
        e.doc.insert(page, f).unwrap();

        // Clipped text node named "Title" (direct child of `frame`, depth 1 from `frame`)
        let tid = e.doc.alloc_id();
        let style = TextStyle { font_size: 18.0, ..Default::default() };
        let mut t = Node::text(tid, Rect::new(16.0, 16.0, 40.0, 20.0), "Complete your order right now before items sell out", style);
        if let NodeKind::Text(tp) = &mut t.kind {
            tp.auto_resize = TextAutoResize::None;
        }
        t.name = "Title".to_string();
        e.doc.insert(frame, t).unwrap();

        // Zero-bounds + default-name node ("Rectangle") at depth 1 from `frame`
        let bad_rect = e.doc.alloc_id();
        let mut br = Node::rectangle(bad_rect, Rect::new(10.0, 50.0, 0.0, 20.0));
        br.name = "Rectangle".to_string();
        e.doc.insert(frame, br).unwrap();

        // Deep un-emitted node at depth 2 from `frame` (under a sub-container)
        let sub = e.doc.alloc_id();
        let mut sf = Node::frame(sub, Rect::new(10.0, 80.0, 100.0, 60.0));
        sf.name = "Sub".to_string();
        e.doc.insert(frame, sf).unwrap();
        let deep_bad = e.doc.alloc_id();
        let mut dbr = Node::rectangle(deep_bad, Rect::new(0.0, 0.0, 0.0, 0.0));
        dbr.name = "DeepZero".to_string();
        e.doc.insert(sub, dbr).unwrap();

        // 1. Run `designos_get_metadata` with `depth: 1, lint: true` on `frame`:
        // `deep_bad` is at depth 2 so it is NOT emitted and must NOT be linted into `lint_cache`.
        let res1 = e.call("designos_get_metadata", json!({"node_id": frame.0, "depth": 1, "lint": true})).unwrap();
        assert_eq!(res1["lint_ran"], true);
        let cache_map = &e.lint_cache.as_ref().unwrap().1;
        assert!(cache_map.contains_key(&tid), "emitted clipped text must be in lint_cache");
        assert!(!cache_map.contains_key(&deep_bad), "un-emitted depth-2 node must not be linted by lint: true");

        // 2. Run `designos_lint` at the same revision:
        // - It must not drop `clipped-text` on `tid` ("Title").
        // - `bad_rect` ("Rectangle") gets 2 findings (`zero-bounds` + `default-layer-name`).
        e.call("designos_lint", json!({"page_id": page.0})).unwrap();
        let after_lint = e.call("designos_get_metadata", json!({"node_id": frame.0, "depth": 1, "lint": false})).unwrap();
        let xml_after_lint = after_lint["xml"].as_str().unwrap();
        assert!(
            xml_after_lint.contains(&format!("id=\"{}\" name=\"Title\"", tid.0)) && xml_after_lint.contains("issues=\"1\""),
            "Title must still report issues=\"1\" after designos_lint:\n{xml_after_lint}"
        );
        assert!(
            xml_after_lint.contains(&format!("id=\"{}\" name=\"Rectangle\"", bad_rect.0)) && xml_after_lint.contains("issues=\"2\""),
            "Rectangle must report issues=\"2\" (zero-bounds + default-layer-name):\n{xml_after_lint}"
        );

        // 3. Re-running `designos_get_metadata` with `lint: true` at the same revision must NOT overwrite
        // `bad_rect`'s count of 2 down to 1!
        let after_re_lint = e.call("designos_get_metadata", json!({"node_id": frame.0, "depth": 1, "lint": true})).unwrap();
        let xml_re_lint = after_re_lint["xml"].as_str().unwrap();
        assert!(
            xml_re_lint.contains(&format!("id=\"{}\" name=\"Rectangle\"", bad_rect.0)) && xml_re_lint.contains("issues=\"2\""),
            "lint: true must not overwrite Rectangle's 2 findings down to 1:\n{xml_re_lint}"
        );
    }

    #[test]
    fn budget_truncation_inside_last_child_marks_all_unfinished_ancestors() {
        let mut e = Engine::default();
        let page = e.resolve_default_page().unwrap();
        let root = e.doc.alloc_id();
        let mut rf = Node::frame(root, Rect::new(0.0, 0.0, 800.0, 600.0));
        rf.name = "Root".to_string();
        e.doc.insert(page, rf).unwrap();

        let only_child = e.doc.alloc_id();
        let mut cf = Node::frame(only_child, Rect::new(0.0, 0.0, 400.0, 300.0));
        cf.name = "OnlyChild".to_string();
        e.doc.insert(root, cf).unwrap();

        for i in 0..5 {
            let gid = e.doc.alloc_id();
            let mut g = Node::rectangle(gid, Rect::new(i as f32 * 20.0, 0.0, 10.0, 10.0));
            g.name = format!("Grand-{i}");
            e.doc.insert(only_child, g).unwrap();
        }

        // max_nodes: 3 emits Root (1), OnlyChild (2), Grand-0 (3), then truncates inside OnlyChild (the last child of Root).
        let res = e.call("designos_get_metadata", json!({"node_id": root.0, "depth": 3, "max_nodes": 3})).unwrap();
        assert_eq!(res["truncated"], true);
        assert_eq!(res["node_count"], 3);
        let xml = res["xml"].as_str().unwrap();
        let first_line = xml.lines().next().unwrap();
        assert!(first_line.contains("name=\"Root\"") && first_line.contains("more=\"budget\""), "root element must carry more=\"budget\":\n{xml}");
        assert!(xml.contains("name=\"OnlyChild\"") && xml.contains("more=\"budget\""), "OnlyChild must carry more=\"budget\":\n{xml}");
    }

    #[test]
    fn partial_structural_lint_cutoff_preserves_cached_clipped_text_and_respects_first_node_budget() {
        let mut e = Engine::default();
        let page = e.resolve_default_page().unwrap();

        // 1. First node is a freeform frame with overlapping children; a zero budget must stop on idx == 0
        // without marking the first node as checked.
        let early_frame = e.doc.alloc_id();
        let mut ef = Node::frame(early_frame, Rect::new(0.0, 0.0, 600.0, 400.0));
        ef.name = "EarlyFrame".to_string();
        e.doc.insert(page, ef).unwrap();
        for i in 0..300 {
            let cid = e.doc.alloc_id();
            let mut c = Node::text(
                cid,
                Rect::new(0.0, 0.0, 50.0, 20.0),
                "Paragraph of text that requires font shaping across multiple words and lines to spend budget",
                TextStyle { font_size: 16.0, ..Default::default() },
            );
            if let NodeKind::Text(tp) = &mut c.kind {
                tp.auto_resize = TextAutoResize::None;
            }
            c.name = format!("BusyText-{i}");
            e.doc.insert(early_frame, c).unwrap();
        }

        let (zero_hits, zero_checked) = designos_export::structural_lint_rules_for_nodes(&e.doc, &[early_frame], std::time::Duration::ZERO);
        assert!(zero_hits.is_empty(), "zero budget must not run structural rules on idx == 0");
        assert!(zero_checked.is_empty(), "cutoff node must not be pushed into checked");

        // 2. Late frame with a clipped text node cached via `designos_get_metadata(lint: true)`
        let late_frame = e.doc.alloc_id();
        let mut lf = Node::frame(late_frame, Rect::new(0.0, 500.0, 400.0, 200.0));
        lf.name = "LateFrame".to_string();
        e.doc.insert(page, lf).unwrap();

        let late_text = e.doc.alloc_id();
        let mut lt = Node::text(
            late_text,
            Rect::new(10.0, 10.0, 30.0, 16.0),
            "This late text layer is definitely clipped and cached before the full-page lint cutoff",
            TextStyle { font_size: 18.0, ..Default::default() },
        );
        if let NodeKind::Text(tp) = &mut lt.kind {
            tp.auto_resize = TextAutoResize::None;
        }
        lt.name = "LateTitle".to_string();
        e.doc.insert(late_frame, lt).unwrap();

        // Populate cache for `late_frame` subtree via `lint: true`
        let seeded = e.call("designos_get_metadata", json!({"node_id": late_frame.0, "depth": 1, "lint": true})).unwrap();
        let seeded_xml = seeded["xml"].as_str().unwrap();
        assert!(
            seeded_xml.contains(&format!("id=\"{}\" name=\"LateTitle\"", late_text.0)) && seeded_xml.contains("issues=\"1\""),
            "LateTitle must initially cache issues=\"1\":\n{seeded_xml}"
        );

        // Now run `designos_lint` on the whole page: the 300 BusyText nodes consume the 50ms structural budget
        // before `late_text` is reached, but `late_text` is still in `checked_nodes` of the full walk.
        // `late_text` must NOT lose its cached `clipped-text` issue!
        e.call("designos_lint", json!({"page_id": page.0})).unwrap();

        let after = e.call("designos_get_metadata", json!({"node_id": late_frame.0, "depth": 1, "lint": false})).unwrap();
        let after_xml = after["xml"].as_str().unwrap();
        assert!(
            after_xml.contains(&format!("id=\"{}\" name=\"LateTitle\"", late_text.0)) && after_xml.contains("issues=\"1\""),
            "LateTitle must retain cached clipped-text issues=\"1\" when structural pass stops at 50ms cutoff:\n{after_xml}"
        );
    }
}
