//! Selectors of the previous query tool against this query: the bare-step rule of release's
//! `matches_single_step` (`crates/designos-mcp/src/engine.rs` on `release/v0.6.0`, f03354be) is ported
//! below verbatim and run over one fixture next to [`run`]. The two must agree for every selector the
//! previous tool accepted, except the exact upper-case node-type tokens (`TEXT`, `PAGE`, `*`, ...),
//! which are kind tests only and are asserted on their own.

use super::*;
use crate::paint::TextStyle;
use crate::{BooleanOp, Rect, Vec2};
use std::collections::BTreeSet;

/// Release's `matches_single_step`, copied unchanged (only the crate paths differ).
fn release_matches_single_step(doc: &Document, n: &Node, step: &str) -> bool {
    let kind_upper = n.kind.default_name().to_ascii_uppercase();
    if step.is_empty() || step == "*" {
        return true;
    }
    if let Some((tag_part, rest)) = step.split_once('[') {
        let tag_ok = tag_part.is_empty() || tag_part == "*" || kind_upper == tag_part.to_ascii_uppercase();
        if !tag_ok {
            return false;
        }
        let pred_clean = rest.trim_end_matches(']').trim();
        if let Some(val) = pred_clean.strip_prefix("@name=") {
            let unquoted = val.trim().trim_matches('\'').trim_matches('"');
            n.name == unquoted
        } else if pred_clean.contains("contains(@name") {
            let parts: Vec<&str> = pred_clean.split(['\'', '"']).collect();
            parts.get(1).is_some_and(|needle| n.name.to_lowercase().contains(&needle.to_lowercase()))
        } else if let Some(val) = pred_clean.strip_prefix("@component=") {
            let unquoted = val.trim().trim_matches('\'').trim_matches('"');
            crate::instance_component(n).and_then(|cid| doc.get(cid)).is_some_and(|cn| {
                cn.name == unquoted || cn.parent.and_then(|pid| doc.get(pid)).is_some_and(|p| matches!(p.kind, NodeKind::ComponentSet(_)) && p.name == unquoted)
            })
        } else if pred_clean == "@fill" {
            !n.fills.is_empty()
        } else {
            true
        }
    } else {
        kind_upper == step.to_ascii_uppercase() || n.name.to_lowercase().contains(&step.to_lowercase())
    }
}

/// Release's `query_xpath_nodes`, copied unchanged (only the crate paths differ).
fn release_query_xpath_nodes<'a>(doc: &'a Document, root: Option<NodeId>, selector: &str) -> Vec<&'a Node> {
    let mut matches = Vec::new();
    let steps: Vec<&str> = if let Some(rest) = selector.strip_prefix("//") { rest.split("//").filter(|s| !s.is_empty()).collect() } else { Vec::new() };

    let candidate_ids: Vec<NodeId> = if let Some(r) = root {
        let mut ids = Vec::new();
        doc.walk(r, |n, _| ids.push(n.id));
        ids
    } else {
        doc.nodes.keys().copied().collect()
    };

    for nid in candidate_ids {
        let Some(n) = doc.get(nid) else {
            continue;
        };
        let matched = if selector.is_empty() || selector == "//*" || selector == "*" {
            true
        } else if steps.len() >= 2 {
            let last = steps[steps.len() - 1];
            if !release_matches_single_step(doc, n, last) {
                false
            } else {
                let mut cur_parent = n.parent;
                let mut idx = steps.len() - 1;
                while idx > 0 {
                    let target_step = steps[idx - 1];
                    let mut found = None;
                    while let Some(pid) = cur_parent {
                        if let Some(pnode) = doc.get(pid) {
                            if release_matches_single_step(doc, pnode, target_step) {
                                found = pnode.parent;
                                break;
                            }
                            cur_parent = pnode.parent;
                        } else {
                            break;
                        }
                    }
                    if found.is_some() || (idx == 1 && cur_parent.is_some()) {
                        cur_parent = found;
                        idx -= 1;
                    } else {
                        break;
                    }
                }
                idx == 0
            }
        } else if let Some(&only) = steps.first() {
            release_matches_single_step(doc, n, only)
        } else {
            release_matches_single_step(doc, n, selector)
        };
        if matched {
            matches.push(n);
        }
    }
    matches
}

/// Two pages. Page "Home" holds, in pre-order: the frame "Button group" (with the text "Label"), the
/// frames "Home page", "Page header", "Line chart", "Direct link" and "Union members", the boolean layer
/// "Combined", the rectangle "Text field", the group "Cluster" and the group "Icon group" (with the text
/// "Icon label"). Page "Components" holds the frame "Card" (texts "Card title" and "Go home", rectangle
/// "Card bg"), the frames "Card_2" and "Node list", the boolean layer "Merged" and the group "Group 1".
///
/// Release ignored every predicate it did not parse (`@width>90` was always true), so every node a
/// predicate row can reach by kind is wider than the number the rows use: GROUP and BOOLEAN layers are
/// 100 or 120 wide, TEXT is 80 wide.
struct ParityDoc {
    doc: Document,
    home: NodeId,
    components: NodeId,
}

impl ParityDoc {
    fn new() -> Self {
        let mut doc = Document::new("Release Parity");
        let home = doc.add_page("Home");
        let components = doc.add_page("Components");
        let mut taken: BTreeSet<String> = ["Home".to_string(), "Components".to_string()].into();

        let mut add = |doc: &mut Document, parent: NodeId, mut node: Node, name: &str| {
            node.name = name.to_string();
            let id = node.id;
            doc.insert(parent, node).unwrap();
            assert!(taken.insert(name.to_string()), "fixture names are unique: {name}");
            id
        };
        let frame = |doc: &mut Document, w: f32, h: f32| Node::frame(doc.alloc_id(), Rect::new(0.0, 0.0, w, h));
        let text = |doc: &mut Document, content: &str| Node::text(doc.alloc_id(), Rect::new(0.0, 0.0, 80.0, 20.0), content, TextStyle::default());
        let layer = |doc: &mut Document, kind: NodeKind| {
            let mut n = Node::new(doc.alloc_id(), kind);
            n.size = Vec2::new(120.0, 120.0);
            n
        };
        let rectangle = |doc: &mut Document| Node::rectangle(doc.alloc_id(), Rect::new(0.0, 0.0, 80.0, 20.0));

        let n = frame(&mut doc, 100.0, 40.0);
        let button_group = add(&mut doc, home, n, "Button group");
        let n = text(&mut doc, "Pay");
        add(&mut doc, button_group, n, "Label");
        for name in ["Home page", "Page header", "Line chart", "Direct link", "Union members"] {
            let n = frame(&mut doc, 100.0, 100.0);
            add(&mut doc, home, n, name);
        }
        let n = layer(&mut doc, NodeKind::BooleanOp(BooleanOp::Union));
        add(&mut doc, home, n, "Combined");
        let n = rectangle(&mut doc);
        add(&mut doc, home, n, "Text field");
        let n = layer(&mut doc, NodeKind::Group);
        add(&mut doc, home, n, "Cluster");
        let n = layer(&mut doc, NodeKind::Group);
        let icon_group = add(&mut doc, home, n, "Icon group");
        let n = text(&mut doc, "Go");
        add(&mut doc, icon_group, n, "Icon label");

        let n = frame(&mut doc, 100.0, 100.0);
        let card = add(&mut doc, components, n, "Card");
        let n = text(&mut doc, "Title");
        add(&mut doc, card, n, "Card title");
        let n = text(&mut doc, "Back");
        add(&mut doc, card, n, "Go home");
        let n = rectangle(&mut doc);
        add(&mut doc, card, n, "Card bg");
        for name in ["Card_2", "Node list"] {
            let n = frame(&mut doc, 100.0, 100.0);
            add(&mut doc, components, n, name);
        }
        let n = layer(&mut doc, NodeKind::BooleanOp(BooleanOp::Union));
        add(&mut doc, components, n, "Merged");
        let n = layer(&mut doc, NodeKind::Group);
        add(&mut doc, components, n, "Group 1");

        Self { doc, home, components }
    }

    fn name_of(&self, id: u64) -> String {
        self.doc.get(NodeId(id)).map_or_else(|| format!("#{id}"), |n| n.name.clone())
    }

    fn names_of(&self, ids: &[u64]) -> Vec<String> {
        ids.iter().map(|&id| self.name_of(id)).collect()
    }

    /// What this query answers, as ids in the order it returned them.
    fn query(&self, selector: &str, page: Option<NodeId>) -> Vec<u64> {
        let res = run(&self.doc, &QueryOptions { selector: selector.to_string(), page, limit: Some(MAX_LIMIT), ..Default::default() })
            .unwrap_or_else(|e| panic!("{selector}: {e}"));
        assert!(res.done, "{selector}: the fixture fits one page");
        assert_eq!(res.scanned_total, res.scope_size, "{selector}: every node the query visited is part of the scope");
        res.rows.iter().map(|r| r[0].as_u64().unwrap()).collect()
    }

    /// The same answer one row per call, through the cursor.
    fn query_paged(&self, selector: &str, page: Option<NodeId>) -> Vec<u64> {
        let mut ids = Vec::new();
        let mut cursor = None;
        for _ in 0..200 {
            let res = run(&self.doc, &QueryOptions { selector: selector.to_string(), page, limit: Some(1), cursor: cursor.clone(), ..Default::default() })
                .unwrap_or_else(|e| panic!("{selector}: {e}"));
            ids.extend(res.rows.iter().map(|r| r[0].as_u64().unwrap()));
            if res.done {
                return ids;
            }
            cursor = res.cursor;
        }
        panic!("{selector}: paging did not finish");
    }

    /// What release answered: pre-order below the scope root, or every node in id order without a scope.
    fn release(&self, selector: &str, root: Option<NodeId>) -> Vec<u64> {
        release_query_xpath_nodes(&self.doc, root, selector).iter().map(|n| n.id.0).collect()
    }
}

/// Selectors release accepted, every casing of the bare words, each with the reason it is here.
const SAME_AS_RELEASE: &[&str] = &[
    "//Group",
    "//group",
    "Group",
    "//Text",
    "//text",
    "//Frame",
    "//Page",
    "//page",
    // A page named like the word: the page root is a candidate for any bare-name step.
    "//Home",
    "Home",
    "//Union",
    "//UNION",
    "//Line",
    // Alias kind words are names in release, and `//node` is not the wildcard.
    "//rect",
    "//node",
    "//Home//Text",
    "//Group//Text",
    "//Page//Text",
    // A type word with predicates is a kind test only: "Button group" and "Union members" are frames.
    "//Group[contains(@name,'group')]",
    "//Group[@width>90]",
    "//Text[@width>20]",
    "//Union[@width>90]",
    "//group[@name='Icon group']",
    "//Frame[contains(@name,'Card')]",
    // Upper-case kind token with a predicate: the kind test is the same on both sides.
    "//FRAME[contains(@name,'GROUP')]",
    // `contains(@name, ..)` in every casing; the needles avoid the page names.
    "//*[contains(@name,'card')]",
    "//*[contains(@name,'Card')]",
    "//*[contains(@name,'CARD')]",
];

#[test]
fn bare_steps_match_like_release_for_every_selector_release_accepted() {
    let d = ParityDoc::new();
    assert!(SAME_AS_RELEASE.len() >= 15);
    let mut differences = Vec::new();
    for selector in SAME_AS_RELEASE {
        // Scoped to a page: release walked the page root and everything below it, in pre-order.
        for (label, page) in [("page Home", d.home), ("page Components", d.components)] {
            let release = d.release(selector, Some(page));
            let new = d.query(selector, Some(page));
            if new != release {
                differences.push(format!("{selector} on {label}: query {:?}, release {:?}", d.names_of(&new), d.names_of(&release)));
            }
        }
        // Without a scope release took every node of the document; this query visits the pages in order.
        let mut release = d.release(selector, None);
        let mut new = d.query(selector, None);
        release.sort_unstable();
        new.sort_unstable();
        if new != release {
            differences.push(format!("{selector} without a scope: query {:?}, release {:?}", d.names_of(&new), d.names_of(&release)));
        }
        // A cursor returns each row once, the page root included.
        for page in [Some(d.home), Some(d.components), None] {
            if d.query_paged(selector, page) != d.query(selector, page) {
                differences.push(format!("{selector}: paging with one row per call differs from a single call (scope {page:?})"));
            }
        }
        assert!(!d.release(selector, None).is_empty(), "{selector}: release finds something, so equality is not vacuous");
    }
    assert!(differences.is_empty(), "{} answers differ from release:\n{}", differences.len(), differences.join("\n"));
}

#[test]
fn the_port_of_release_answers_what_release_answered() {
    let d = ParityDoc::new();
    let names = |selector: &str, root| d.names_of(&d.release(selector, root));
    assert_eq!(names("//Home", None), ["Home", "Home page", "Go home"], "the page named Home is a candidate");
    assert_eq!(names("//Group[@width>90]", None), ["Cluster", "Icon group", "Group 1"], "an unparsed predicate is true, the kind decides");
    assert_eq!(names("//Group[contains(@name,'group')]", None), ["Icon group", "Group 1"]);
    assert_eq!(names("//rect", None), ["Direct link"], "RECT is no kind word of release");
    assert_eq!(names("//Home//Text", Some(d.home)), ["Label", "Text field", "Icon label"], "the page is an ancestor of every text on it");
    // And this query answers the two rows the controller named.
    assert_eq!(d.names_of(&d.query("//Home", Some(d.home))), ["Home", "Home page"]);
    assert_eq!(d.names_of(&d.query("//Group[@width>90]", Some(d.home))), ["Cluster", "Icon group"]);
}

#[test]
fn the_exact_upper_case_token_is_the_only_departure_from_release() {
    let d = ParityDoc::new();
    let all_but_pages = [
        "Button group",
        "Label",
        "Home page",
        "Page header",
        "Line chart",
        "Direct link",
        "Union members",
        "Combined",
        "Text field",
        "Cluster",
        "Icon group",
        "Icon label",
        "Card",
        "Card title",
        "Go home",
        "Card bg",
        "Card_2",
        "Node list",
        "Merged",
        "Group 1",
    ];
    // (selector, kind-only answer of this query, what release answered instead)
    let rows: [(&str, &[&str], &[&str]); 8] = [
        ("//TEXT", &["Label", "Icon label", "Card title", "Go home"], &["Label", "Text field", "Icon label", "Card title", "Go home"]),
        ("//GROUP", &["Cluster", "Icon group", "Group 1"], &["Button group", "Cluster", "Icon group", "Group 1"]),
        (
            "//FRAME",
            &["Button group", "Home page", "Page header", "Line chart", "Direct link", "Union members", "Card", "Card_2", "Node list"],
            &["Button group", "Home page", "Page header", "Line chart", "Direct link", "Union members", "Card", "Card_2", "Node list"],
        ),
        ("//PAGE", &["Home", "Components"], &["Home", "Home page", "Page header", "Components"]),
        ("//RECT", &["Text field", "Card bg"], &["Direct link"]),
        ("//LINE", &[], &["Line chart"]),
        ("//BOOLEAN_OPERATION", &["Combined", "Merged"], &[]),
        ("//*", &all_but_pages, &[]),
    ];
    for (selector, expected, release_answer) in rows {
        assert_eq!(d.names_of(&d.query(selector, None)), expected, "{selector} is a kind test only");
        if selector == "//*" {
            // Release's wildcard also took the two page roots.
            let mut release = d.names_of(&d.release(selector, None));
            release.sort();
            let mut with_pages: Vec<String> = all_but_pages.iter().map(|s| s.to_string()).chain(["Home".to_string(), "Components".to_string()]).collect();
            with_pages.sort();
            assert_eq!(release, with_pages, "release answered `//*` with the pages as well");
        } else {
            let mut release = d.names_of(&d.release(selector, None));
            release.sort();
            let mut want: Vec<&str> = release_answer.to_vec();
            want.sort_unstable();
            assert_eq!(release, want, "release answered {selector} by kind or name");
        }
    }
    // The same token with a predicate stays a kind test.
    assert_eq!(d.names_of(&d.query("//TEXT[contains(@name,'label')]", None)), ["Label", "Icon label"]);
    assert_eq!(d.names_of(&d.query("//PAGE[@name='Home']", None)), ["Home"]);
}
