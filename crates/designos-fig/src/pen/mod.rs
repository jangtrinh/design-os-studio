//! `.pen` JSON scene format importer and exporter at schema `2.21` with version migration (`1.0 -> 2.8 -> 2.13 -> 2.17 -> 2.21`).

pub mod export;
pub mod instances;
pub mod migrate;
pub mod node;
pub mod paint;
pub mod variables;

pub use export::export_pen_json;
pub use migrate::{Step, CURRENT_PEN_VERSION, MIGRATIONS};

use designos_doc::*;
use instances::{resolve_pen_connections, resolve_pen_instances, resolve_pen_library_imports};
use migrate::migrate_to_current;
use node::{import_pen_node, PenImportCtx};
use serde_json::Value;
use std::path::PathBuf;
use variables::import_pen_variables;

/// Upper bound on the total number of nodes cloned across all `ref` (component instance) expansions in a single
/// `.pen` import. A crafted file can chain components (`c0` has two `ref`s to `c1`, `c1` two to `c2`, ...) so the
/// expanded tree grows as $2^k$ in the input size; once this budget is spent, remaining `ref` nodes keep their link
/// to the target component without cloning its subtree, and a warning is returned.
pub const MAX_PEN_REF_CLONES: usize = 100_000;

/// Options controlling `.pen` scene import (such as `base_dir` for resolving relative `imports` and image paths).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PenImportOptions {
    pub base_dir: Option<PathBuf>,
}

/// Structured result of importing a `.pen` document, including structured warnings and the input's `source_version`.
#[derive(Clone, Debug)]
pub struct PenImport {
    pub doc: Document,
    pub warnings: Vec<PenWarning>,
    pub source_version: String,
}

/// Non-fatal diagnostic emitted during `.pen` import.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PenWarning {
    pub code: String,
    pub node_id: Option<String>,
    pub message: String,
}

/// Parse a `.pen` JSON string into a [`Document`].
///
/// Non-fatal import warnings (see [`import_pen_json_with_warnings`]) are written to stderr, the crate's only log sink.
pub fn import_pen_json(raw: &str) -> Result<Document, String> {
    let (doc, warning) = import_pen_json_with_warnings(raw)?;
    if let Some(warning) = warning {
        eprintln!("designos-fig: {warning}");
    }
    Ok(doc)
}

/// Like [`import_pen_json`], but also returns the non-fatal warning string (joined with `"; "`), if any.
pub fn import_pen_json_with_warnings(raw: &str) -> Result<(Document, Option<String>), String> {
    import_pen_json_budgeted(raw, MAX_PEN_REF_CLONES)
}

/// Full structured `.pen` import entry point returning [`PenImport`] (`doc`, `warnings`, `source_version`).
pub fn import_pen_json_report(raw: &str, opts: &PenImportOptions) -> Result<PenImport, String> {
    import_pen_json_report_budgeted(raw, opts, MAX_PEN_REF_CLONES)
}

pub(crate) fn import_pen_json_budgeted(raw: &str, clone_budget: usize) -> Result<(Document, Option<String>), String> {
    let report = import_pen_json_report_budgeted(raw, &PenImportOptions::default(), clone_budget)?;
    let joined = if report.warnings.is_empty() { None } else { Some(report.warnings.iter().map(|w| w.message.as_str()).collect::<Vec<_>>().join("; ")) };
    Ok((report.doc, joined))
}

fn import_pen_json_report_budgeted(raw: &str, opts: &PenImportOptions, clone_budget: usize) -> Result<PenImport, String> {
    let mut root: Value = serde_json::from_str(raw).map_err(|e| format!("Invalid .pen JSON: {e}"))?;
    if !root.is_object() {
        return Err("Invalid .pen file: root must be a JSON object".into());
    }

    let mut warnings = Vec::new();
    let source_version = migrate_to_current(&mut root, &mut warnings)?;

    let doc_name = root.get("name").and_then(Value::as_str).unwrap_or("Pencil Document");
    let mut doc = Document::new(doc_name);
    let page_id = doc.add_page("Page 1");

    // 1. Parse `themes` and `variables` into Document variable collections and tokens (preserving file key order).
    import_pen_variables(raw, &root, &mut doc);

    let mut ctx = PenImportCtx::new(opts.base_dir.clone(), warnings);

    // 2. Resolve external library `imports` if present.
    let mut visited_paths = Vec::new();
    resolve_pen_library_imports(&root, &mut doc, page_id, &mut ctx, &mut visited_paths, 0);

    // 3. Import top-level `children` onto `page_id`.
    if let Some(children) = root.get("children").and_then(Value::as_array) {
        for ch in children {
            import_pen_node(ch, &mut doc, page_id, &mut ctx);
        }
    }

    // 4. Resolve component instances (`type: "ref"`) and `descendants` overrides.
    resolve_pen_instances(&mut doc, &mut ctx, clone_budget)?;

    // 5. Resolve board connectors (`type: "connection"`).
    let pending_connections = std::mem::take(&mut ctx.pending_connections);
    resolve_pen_connections(&mut doc, &pending_connections);

    Ok(PenImport { doc, warnings: ctx.warnings, source_version })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pen::instances::subtree_len_up_to;

    #[test]
    fn pen_import_and_export_roundtrip() {
        let sample_pen = r##"{
            "version": "1.0",
            "name": "Design System Pen",
            "themes": {
                "mode": ["Light", "Dark"]
            },
            "variables": {
                "primary": {
                    "type": "color",
                    "value": [
                        { "theme": { "mode": "Light" }, "value": "#3B82F6" },
                        { "theme": { "mode": "Dark" }, "value": "#60A5FA" }
                    ]
                },
                "radius-md": {
                    "type": "number",
                    "value": 12
                }
            },
            "children": [
                {
                    "id": "btn-comp",
                    "type": "frame",
                    "reusable": true,
                    "name": "Primary Button",
                    "x": 20,
                    "y": 40,
                    "width": "fit_content(160)",
                    "height": 48,
                    "layout": "horizontal",
                    "gap": 8,
                    "padding": [12, 24],
                    "justifyContent": "center",
                    "alignItems": "center",
                    "fill": "$primary",
                    "cornerRadius": "$radius-md",
                    "children": [
                        {
                            "id": "btn-label",
                            "type": "text",
                            "name": "Label",
                            "content": "Click Me",
                            "fontFamily": "Inter",
                            "fontSize": 16,
                            "fontWeight": 600,
                            "fill": "#FFFFFF"
                        }
                    ]
                },
                {
                    "id": "btn-inst",
                    "type": "ref",
                    "ref": "btn-comp",
                    "name": "CTA Instance",
                    "x": 220,
                    "y": 40,
                    "width": "fill_container(200)",
                    "height": 48,
                    "descendants": {
                        "btn-label": {
                            "content": "Get Started"
                        }
                    }
                }
            ]
        }"##;

        let doc = import_pen_json(sample_pen).expect("import .pen should succeed");
        assert_eq!(doc.name, "Design System Pen");
        assert!(doc.variables.contains_key("primary"));
        assert!(doc.variables.contains_key("radius-md"));

        let page = doc.pages[0];
        let page_node = doc.get(page).unwrap();
        assert_eq!(page_node.children.len(), 2);

        let comp_id = page_node.children[0];
        let comp_node = doc.get(comp_id).unwrap();
        assert!(matches!(comp_node.kind, NodeKind::Component(_)));
        assert_eq!(comp_node.variables.bindings.get("fills/0").map(String::as_str), Some("primary"));
        assert_eq!(comp_node.corner_radii, [12.0; 4]);

        let inst_id = page_node.children[1];
        let inst_node = doc.get(inst_id).unwrap();
        assert!(matches!(inst_node.kind, NodeKind::Instance { component: Some(cid), .. } if cid == comp_id));
        assert_eq!(inst_node.layout_child.horizontal, Sizing::Fill);
        assert_eq!(inst_node.children.len(), 1);
        let inst_label = doc.get(inst_node.children[0]).unwrap();
        assert_eq!(inst_label.kind.any_text().unwrap().content, "Get Started");

        // Round-trip through export_pen_json -> import_pen_json
        let exported = export_pen_json(&doc).expect("export .pen should succeed");
        let reimported = import_pen_json(&exported).expect("re-import .pen should succeed");
        assert_eq!(reimported.name, "Design System Pen");
        assert_eq!(reimported.variables.len(), 2);
        let rpage = reimported.pages[0];
        let rchildren = &reimported.get(rpage).unwrap().children;
        assert_eq!(rchildren.len(), 2);
        let rinst = reimported.get(rchildren[1]).unwrap();
        let rlabel = reimported.get(rinst.children[0]).unwrap();
        assert_eq!(rlabel.kind.any_text().unwrap().content, "Get Started");
    }

    #[test]
    fn ref_to_own_component_or_itself_is_rejected() {
        let inside_component = r##"{"version":"1.0","children":[
            {"id":"c","type":"frame","reusable":true,"children":[{"id":"i","type":"ref","ref":"c"}]}]}"##;
        let err = import_pen_json(inside_component).unwrap_err();
        assert!(err.contains("referenced from inside itself"), "{err}");

        let deep = r##"{"version":"1.0","children":[
            {"id":"c","type":"frame","reusable":true,"children":[{"id":"g","type":"frame","children":[{"id":"i","type":"ref","ref":"c"}]}]}]}"##;
        assert!(import_pen_json(deep).is_err());

        let self_ref = r##"{"version":"1.0","children":[{"id":"i","type":"ref","ref":"i"}]}"##;
        assert!(import_pen_json(self_ref).is_err());
    }

    #[test]
    fn cyclic_variable_aliases_fall_back_instead_of_recursing() {
        let json = r##"{"version":"1.0",
            "variables":{"a":{"type":"number","value":"$b"},"b":{"type":"number","value":"$a"},
                         "c":{"type":"color","value":"$d"},"d":{"type":"color","value":"$c"},
                         "self":{"type":"number","value":"$self"}},
            "children":[
                {"id":"n","type":"frame","width":"$a","height":"$self","fill":"$c"}]}"##;
        let doc = import_pen_json(json).expect("a variable cycle must not fail the import");
        let n = doc.get(doc.get(doc.pages[0]).unwrap().children[0]).unwrap();
        let (w, h) = (n.size.x, n.size.y);
        assert!(w.is_finite() && h.is_finite());
        assert_eq!(n.variables.bindings.get("fills/0").map(String::as_str), Some("c"));
    }

    #[test]
    fn long_acyclic_alias_chain_still_resolves() {
        let mut vars = vec![r#""v0":{"type":"number","value":42}"#.to_string()];
        vars.extend((1..2000).map(|i| format!(r#""v{i}":{{"type":"number","value":"$v{}"}}"#, i - 1)));
        let json = format!(r##"{{"version":"1.0","variables":{{{}}},"children":[{{"id":"n","type":"frame","width":"$v1999","height":10}}]}}"##, vars.join(","));
        let doc = import_pen_json(&json).unwrap();
        let n = doc.get(doc.get(doc.pages[0]).unwrap().children[0]).unwrap();
        assert_eq!(n.size.x, 42.0);
    }

    #[test]
    fn chained_component_refs_hit_the_clone_budget() {
        let mut children = vec![r##"{"id":"c24","type":"frame","reusable":true,"children":[]}"##.to_string()];
        for i in (0..24).rev() {
            let next = i + 1;
            children.push(format!(
                r##"{{"id":"c{i}","type":"frame","reusable":true,"children":[{{"id":"a{i}","type":"ref","ref":"c{next}"}},{{"id":"b{i}","type":"ref","ref":"c{next}"}}]}}"##
            ));
        }
        let json = format!(r##"{{"version":"1.0","children":[{}]}}"##, children.join(","));
        let (doc, warning) = import_pen_json_budgeted(&json, 1_000).expect("an exhausted clone budget must not fail the import");
        let warning = warning.expect("budget exhaustion is surfaced as a warning");
        assert!(warning.contains("expand past 1000"), "{warning}");
        assert!(doc.get(doc.pages[0]).unwrap().children.len() >= 25);
        assert!(doc.nodes.len() < 1_000 + 200, "got {} nodes", doc.nodes.len());
    }

    #[test]
    fn exhausted_clone_budget_keeps_earlier_instances_and_skips_later_ones() {
        let mut kids = vec![r##"{"id":"comp","type":"frame","reusable":true,"children":[
            {"id":"l1","type":"frame"},{"id":"l2","type":"frame"},{"id":"l3","type":"frame"}]}"##
            .to_string()];
        kids.extend((0..5).map(|i| format!(r##"{{"id":"i{i}","type":"ref","ref":"comp"}}"##)));
        let json = format!(r##"{{"version":"1.0","children":[{}]}}"##, kids.join(","));

        let (doc, warning) = import_pen_json_budgeted(&json, 7).unwrap();
        let page = doc.get(doc.pages[0]).unwrap();
        let counts: Vec<usize> = page.children[1..].iter().map(|&i| doc.get(i).unwrap().children.len()).collect();
        assert_eq!(counts, [3, 3, 0, 0, 0]);
        let w = warning.unwrap();
        assert!(w.contains("3 instance(s)"), "{w}");

        let (doc, warning) = import_pen_json_budgeted(&json, 15).unwrap();
        assert!(warning.is_none());
        let page = doc.get(doc.pages[0]).unwrap();
        assert!(page.children[1..].iter().all(|&i| doc.get(i).unwrap().children.len() == 3));

        let json = format!(r##"{{"version":"1.0","children":[{},{{"id":"s","type":"ref","ref":"s"}}]}}"##, kids.join(","));
        assert!(import_pen_json_budgeted(&json, 7).is_err());
    }

    #[test]
    fn public_warning_entry_point_reports_none_for_a_small_document() {
        let json = r##"{"version":"1.0","children":[{"id":"c","type":"frame","reusable":true},{"id":"i","type":"ref","ref":"c"}]}"##;
        assert!(import_pen_json_with_warnings(json).unwrap().1.is_none());
        assert!(import_pen_json(json).is_ok());
    }

    #[test]
    fn subtree_count_stops_once_it_passes_the_limit() {
        let kids: Vec<String> = (0..50).map(|i| format!(r##"{{"id":"k{i}","type":"frame"}}"##)).collect();
        let json = format!(r##"{{"version":"1.0","children":[{{"id":"c","type":"frame","reusable":true,"children":[{}]}}]}}"##, kids.join(","));
        let (doc, _) = import_pen_json_budgeted(&json, 1_000).unwrap();
        let comp = doc.get(doc.pages[0]).unwrap().children[0];
        let roots = doc.get(comp).unwrap().children.clone();
        assert_eq!(subtree_len_up_to(&doc, &roots, 1_000), 50);
        assert_eq!(subtree_len_up_to(&doc, &roots, 50), 50);
        assert_eq!(subtree_len_up_to(&doc, &roots, 10), 11, "the walk gives up one node past the limit");
        assert_eq!(subtree_len_up_to(&doc, &roots, 0), 1);
    }

    #[test]
    fn skipped_refs_do_not_re_walk_a_huge_component() {
        let kids: Vec<String> = (0..20_000).map(|i| format!(r##"{{"id":"k{i}","type":"frame"}}"##)).collect();
        let refs: Vec<String> = (0..3_000).map(|i| format!(r##"{{"id":"r{i}","type":"ref","ref":"comp"}}"##)).collect();
        let json =
            format!(r##"{{"version":"1.0","children":[{{"id":"comp","type":"frame","reusable":true,"children":[{}]}},{}]}}"##, kids.join(","), refs.join(","));
        let started = std::time::Instant::now();
        let (doc, warning) = import_pen_json_budgeted(&json, 10).unwrap();
        let elapsed = started.elapsed();
        assert!(warning.expect("budget exhaustion is surfaced as a warning").contains("3000 instance(s)"));
        assert_eq!(doc.nodes.len(), 1 + 1 + 20_000 + 3_000, "nothing was cloned");
        assert!(elapsed < std::time::Duration::from_millis(1_500), "importing took {elapsed:?}");
    }
}
