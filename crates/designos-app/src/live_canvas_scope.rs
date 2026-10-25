//! The copy of a large document the editor shares with an agent session (`canvas.designos`).
//!
//! Serializing a whole large file on every edit would stall the UI thread, so the agent gets the
//! current page with its top-level frames as empty shells, plus the frames that hold the current
//! selection in full, every variable collection, variable, shared style and style metadata entry,
//! and every comment thread. The editor selects what an agent edit created, so the agent can keep
//! working on it in its next tool call.

use designos_doc::{Document, NodeId};
use std::collections::BTreeSet;

/// Top-level frames shared beyond the selected ones.
const MAX_TOP_LEVEL: usize = 128;
/// Layers the selected frames may bring along in full; larger frames stay empty shells.
const FULL_SUBTREE_BUDGET: usize = 5_000;

/// The page `page` of `doc` as the agent sees it (see the module docs).
pub fn scoped_canvas(doc: &Document, page: NodeId, selection: &[NodeId], name: &str) -> Document {
    let mut scoped = Document::new(name);
    scoped.pages.push(page);
    scoped.reserve_ids(NodeId(doc.nodes.keys().next_back().map_or(page.0, |id| id.0)));
    let Some(page_node) = doc.get(page) else { return scoped };
    let selected = selected_top_level(doc, page, selection);
    let mut p = page_node.clone();
    p.fig_metadata.clear();
    p.fig_metadata.insert("designos:scoped".into(), "true".into());
    p.children = page_node.children.iter().enumerate().filter(|(i, c)| *i < MAX_TOP_LEVEL || selected.contains(c)).map(|(_, c)| *c).collect();
    let mut budget = FULL_SUBTREE_BUDGET;
    for &cid in &p.children {
        if selected.contains(&cid) {
            if let Some(subtree) = full_subtree(doc, cid, budget) {
                budget -= subtree.len();
                for n in subtree {
                    scoped.nodes.insert(n.id, n);
                }
                continue;
            }
        }
        if let Some(cn) = doc.get(cid) {
            let mut c = cn.clone();
            c.children.clear();
            c.fill_geometry = None;
            c.stroke_geometry = None;
            c.fig_metadata.clear();
            c.fig_metadata.insert("designos:scoped_shell".into(), "true".into());
            scoped.nodes.insert(cid, c);
        }
    }
    scoped.nodes.insert(page, p);
    // Token maps and comment threads are carried whole with unchanged ids so `var:` and
    // `text_style` references resolve on large files and comment replies keep stable ids.
    scoped.variable_collections = doc.variable_collections.clone();
    scoped.variables = doc.variables.clone();
    scoped.styles = doc.styles.clone();
    scoped.style_meta = doc.style_meta.clone();
    scoped.comments = doc.comments.clone();
    let now = doc.now_ms();
    for (&id, ip) in &doc.in_progress {
        if scoped.nodes.contains_key(&id) && !ip.is_stale(now, designos_doc::IN_PROGRESS_DEFAULT_STALE_MS) {
            scoped.in_progress.insert(id, ip.clone());
        }
    }
    scoped
}

/// Children of `page` that are, or contain, a selected node.
fn selected_top_level(doc: &Document, page: NodeId, selection: &[NodeId]) -> BTreeSet<NodeId> {
    let mut out = BTreeSet::new();
    for &id in selection {
        let mut cur = id;
        while let Some(parent) = doc.get(cur).and_then(|n| n.parent) {
            if parent == page {
                out.insert(cur);
                break;
            }
            cur = parent;
        }
    }
    out
}

/// Every node under `root` (complete, so child links stay valid), or `None` past `budget`.
fn full_subtree(doc: &Document, root: NodeId, budget: usize) -> Option<Vec<designos_doc::Node>> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let n = doc.get(id)?;
        if out.len() == budget {
            return None;
        }
        stack.extend(n.children.iter().copied());
        let mut copy = n.clone();
        copy.fig_metadata.clear();
        out.push(copy);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{
        Color, Node, Paint, Rect, Style, StyleMeta, StyleProperty, TextStyle, Variable, VariableCollection, VariableKind, VariableMode, VariableValue,
    };

    fn frame_with(doc: &mut Document, parent: NodeId, kids: usize) -> NodeId {
        let f = doc.alloc_id();
        doc.insert(parent, Node::frame(f, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
        for _ in 0..kids {
            let k = doc.alloc_id();
            doc.insert(f, Node::rectangle(k, Rect::new(0.0, 0.0, 4.0, 4.0))).unwrap();
        }
        f
    }

    #[test]
    fn selected_frames_go_whole_within_budget_others_stay_shells() {
        let mut doc = Document::new("d");
        let page = doc.add_page("p");
        let plain = frame_with(&mut doc, page, 3);
        let picked = frame_with(&mut doc, page, 3);
        let huge = frame_with(&mut doc, page, FULL_SUBTREE_BUDGET + 1);
        let deep_child = doc.get(picked).unwrap().children[0];

        let s = scoped_canvas(&doc, page, &[deep_child, huge], "d");
        assert!(s.get(plain).unwrap().children.is_empty());
        assert_eq!(s.get(picked).unwrap().children.len(), 3);
        assert!(s.get(deep_child).is_some());
        assert!(s.get(huge).unwrap().children.is_empty(), "frames past the budget stay shells");
        for n in s.nodes.values() {
            assert!(n.children.iter().all(|c| s.get(*c).is_some()), "every child link must resolve");
        }
    }

    #[test]
    fn a_selected_frame_past_the_cap_is_still_shared() {
        let mut doc = Document::new("d");
        let page = doc.add_page("p");
        let frames: Vec<_> = (0..MAX_TOP_LEVEL + 5).map(|_| frame_with(&mut doc, page, 0)).collect();
        let last = *frames.last().unwrap();
        let s = scoped_canvas(&doc, page, &[last], "d");
        let shared = &s.get(page).unwrap().children;
        assert_eq!(shared.len(), MAX_TOP_LEVEL + 1);
        assert_eq!(shared.last(), Some(&last));
    }

    #[test]
    fn scoped_canvas_metadata_xml_emits_children_zero_and_scoped_true() {
        let mut doc = Document::new("d");
        let page = doc.add_page("p");
        let shell_frame = frame_with(&mut doc, page, 3);
        let selected_frame = frame_with(&mut doc, page, 2);

        let s = scoped_canvas(&doc, page, &[selected_frame], "d");
        let meta = designos_export::metadata_xml(&s, page, &designos_export::MetadataOptions::default(), None);
        assert!(meta.scoped, "metadata_xml on scoped_canvas must report scoped: true");
        assert!(
            meta.xml.contains(&format!("id=\"{}\" name=\"Frame\" x=\"0\" y=\"0\" width=\"100\" height=\"100\" children=\"0\"/>", shell_frame.0)),
            "shell frame must carry children=\"0\":\n{}",
            meta.xml
        );
    }

    #[test]
    fn scoped_canvas_carries_all_collections_variables_styles_and_meta() {
        let mut doc = Document::new("d");
        let page = doc.add_page("p");
        let plain = frame_with(&mut doc, page, 4);
        let picked = frame_with(&mut doc, page, 3);
        doc.variable_collections.insert(
            "col:brand".into(),
            VariableCollection {
                id: "col:brand".into(),
                name: "Brand".into(),
                modes: vec![VariableMode { id: "m:light".into(), name: "Light".into() }],
                default_mode: "m:light".into(),
                variables: vec!["var:primary".into()],
                key: None,
                source_library_key: None,
                extensions: Default::default(),
            },
        );
        doc.variables.insert(
            "var:primary".into(),
            Variable {
                id: "var:primary".into(),
                name: "brand/primary".into(),
                collection: "col:brand".into(),
                kind: VariableKind::Color,
                values: [("m:light".to_string(), VariableValue::Color(Color::rgb(0x25, 0x63, 0xEB)))].into(),
                description: "Primary brand color".into(),
                scopes: vec!["ALL_FILLS".into()],
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );
        doc.styles.insert("S:body".into(), Style::Text(TextStyle { font_size: 15.0, ..Default::default() }));
        doc.style_meta.insert(
            "S:body".into(),
            StyleMeta {
                name: "Body/Regular".into(),
                key: Some("k-body".into()),
                description: "Body copy".into(),
                property: StyleProperty::Text,
                bindings: Default::default(),
                ..Default::default()
            },
        );
        doc.styles.insert("S:surface".into(), Style::Fill(vec![Paint::solid(Color::WHITE)]));
        doc.style_meta.insert("S:surface".into(), StyleMeta { name: "Surface/Card".into(), property: StyleProperty::Fill, ..Default::default() });

        let s = scoped_canvas(&doc, page, &[picked], "d");
        assert_eq!(s.variable_collections, doc.variable_collections);
        assert_eq!(s.variables, doc.variables);
        assert_eq!(s.styles, doc.styles);
        assert_eq!(s.style_meta, doc.style_meta);
        // Node count is unchanged: page (1) + plain shell (1) + picked with 3 children (4) = 6.
        assert_eq!(s.nodes.len(), 6);
        assert!(s.get(plain).unwrap().children.is_empty());
        assert_eq!(s.get(picked).unwrap().children.len(), 3);
    }

    #[test]
    fn scope_threshold_and_node_budgets_are_unchanged() {
        assert_eq!(MAX_TOP_LEVEL, 128);
        assert_eq!(FULL_SUBTREE_BUDGET, 5_000);
    }
}
