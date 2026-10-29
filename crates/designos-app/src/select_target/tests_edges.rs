//! Edges of the `--select` resolver (#1319): a bare number whose node id is per session, a bare id that never
//! expands pages, the one expansion a miss runs, what a miss claims about it, and the 10,000-node line for `fig:`.

use super::tests::{lazy_cards, named, resolve_loaded, CountingHydrator};
use crate::select_target::{resolve, By};
use designos_doc::{Document, Node, NodeId, PageHydrator};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[test]
fn bare_id_resolves_without_hydrating() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut doc = lazy_cards(&calls);
    let hit = resolve_loaded(&mut doc, "5").unwrap();
    assert_eq!((hit.node, hit.matched_by, hit.id_stable), (Some(NodeId(5)), By::BareId, true));
    assert_eq!(calls.load(Ordering::SeqCst), 0, "a bare number that names a loaded node never expands a page");
}

#[test]
fn a_bare_number_that_misses_expands_once_and_retries_the_name_only() {
    // The expanded node is called "77" and has id 9.
    let calls = Arc::new(AtomicUsize::new(0));
    let mut doc = lazy_cards(&calls);
    doc.page_hydrator = Some(Arc::new(Mutex::new(CountingHydrator { calls: calls.clone(), late: Some((NodeId(2), named(9, "77", 0.0))) })));
    let found = resolve_loaded(&mut doc, "77").unwrap();
    assert_eq!((found.node, found.matched_by, found.id_stable), (Some(NodeId(9)), By::Name, false), "found by its name after the expansion");
    assert_eq!(calls.load(Ordering::SeqCst), 1, "one expansion");

    // The retry looks at names, not at the id the expansion just created: node 9 exists now, but `9` names no layer.
    let calls = Arc::new(AtomicUsize::new(0));
    let mut doc = lazy_cards(&calls);
    let err = resolve_loaded(&mut doc, "9").expect_err("the retry after the expansion is a name lookup only").to_string();
    assert!(doc.get(NodeId(9)).is_some(), "the expansion created node 9");
    assert_eq!(calls.load(Ordering::SeqCst), 1, "one expansion, no second look by id");
    assert!(err.contains("matched no node"), "{err}");
}

fn overlap_with_a_per_session_id() -> Document {
    // Layer "7" has id 2. Node 7 is above the watermark of 7, as if page 0 had expanded it.
    let mut doc = Document::new("numbers");
    let page = doc.add_page("Page 1");
    doc.insert(page, named(2, "7", 0.0)).unwrap();
    doc.insert(page, named(7, "Expanded", 20.0)).unwrap();
    doc
}

#[test]
fn an_overlap_with_a_per_session_id_stays_an_error_and_names_only_the_name() {
    let mut doc = overlap_with_a_per_session_id();
    let msg = resolve(&mut doc, "7", NodeId(7)).expect_err("a layer named 7 and a node 7 are still two readings").to_string();
    assert!(msg.starts_with("--select 7 is ambiguous: layer \"7\" (id 2) and node id 7"), "{msg}");
    assert!(msg.contains("per session"), "it says why the id is not offered: {msg}");
    assert!(msg.contains("name:7"), "{msg}");
    assert!(!msg.contains("id:7"), "id:7 would be refused, so the hint must not offer it: {msg}");
    let refused = resolve(&mut doc, "id:7", NodeId(7)).unwrap_err().to_string();
    assert!(refused.contains("not stable across runs"), "the id the hint left out is refused: {refused}");
    assert_eq!(resolve(&mut doc, "name:7", NodeId(7)).unwrap().node, Some(NodeId(2)), "name:7 reaches the layer");

    // The same watermark with a stable id keeps the hint of rule 4, which offers both ways.
    let msg = resolve(&mut doc, "7", NodeId(8)).unwrap_err().to_string();
    assert_eq!(msg, "--select 7 is ambiguous: layer \"7\" (id 2) and node id 7; use id:7 or name:7");

    // The agent CLI never created node 7: a bare 7 is the layer named 7.
    doc.remove(NodeId(7)).unwrap();
    let by_name = resolve(&mut doc, "7", NodeId(7)).unwrap();
    assert_eq!((by_name.node, by_name.matched_by), (Some(NodeId(2)), By::Name));
}

#[test]
fn a_missing_id_says_no_node_has_it_and_an_expanded_one_says_it_is_per_session() {
    let mut doc = crate::agent_cli_select_tests::two_cards();
    let msg = resolve(&mut doc, "id:99", NodeId(7)).unwrap_err().to_string();
    assert!(msg.contains("no node has id 99"), "{msg}");
    assert!(!msg.contains("belongs to an expanded instance"), "a typo is not said to belong to an instance: {msg}");

    doc.insert(NodeId(2), named(9, "Expanded", 0.0)).unwrap();
    let msg = resolve(&mut doc, "id:9", NodeId(7)).unwrap_err().to_string();
    assert!(msg.contains("id 9 was created in this session and is not stable across runs"), "{msg}");
}

/// Stays attached and reports no progress: an expansion that had nothing to do.
#[derive(Debug)]
struct IdleHydrator;

impl PageHydrator for IdleHydrator {
    fn hydrate_page(&mut self, _doc: &mut Document, _page: NodeId) -> bool {
        false
    }
    fn hydrate_all(&mut self, _doc: &mut Document) -> bool {
        false
    }
    fn is_hydrated(&self, _page: NodeId) -> bool {
        false
    }
    fn unexpanded_instances(&self) -> usize {
        1
    }
}

#[test]
fn a_miss_claims_the_pages_were_expanded_only_when_an_expansion_ran() {
    for selector in ["Nothing Here", "fig:0:0", "42"] {
        let mut idle = crate::agent_cli_select_tests::two_cards();
        idle.page_hydrator = Some(Arc::new(Mutex::new(IdleHydrator)));
        let msg = resolve_loaded(&mut idle, selector).unwrap_err().to_string();
        assert!(!msg.contains("all pages expanded"), "{selector}: the hydrator changed nothing: {msg}");

        let calls = Arc::new(AtomicUsize::new(0));
        let mut ran = lazy_cards(&calls);
        let msg = resolve_loaded(&mut ran, selector).unwrap_err().to_string();
        assert!(msg.contains("all pages expanded"), "{selector}: the expansion ran: {msg}");
    }
}

/// One page and `layers` layers, every one with a Figma id, and a hydrator that counts.
fn big_file(layers: u64, calls: &Arc<AtomicUsize>) -> Document {
    let mut doc = Document::new("kit");
    let page = doc.add_page("Page 1");
    for id in 2..2 + layers {
        let mut n: Node = named(id, "Layer", 0.0);
        n.source_id = Some(format!("1:{id}"));
        doc.insert(page, n).unwrap();
    }
    doc.page_hydrator = Some(Arc::new(Mutex::new(CountingHydrator { calls: calls.clone(), late: None })));
    doc
}

#[test]
fn fig_treats_exactly_10000_nodes_as_a_large_file() {
    const HINT: &str = "instance sublayers in files over 10,000 nodes keep no Figma id";
    let calls = Arc::new(AtomicUsize::new(0));
    let mut exactly = big_file(9_999, &calls);
    assert_eq!(exactly.nodes.len(), 10_000);
    let msg = resolve_loaded(&mut exactly, "fig:I1:2;3:4").unwrap_err().to_string();
    assert_eq!(calls.load(Ordering::SeqCst), 0, "10,000 nodes: no expansion");
    assert!(msg.contains(HINT), "10,000 nodes get the hint that says why: {msg}");

    let calls = Arc::new(AtomicUsize::new(0));
    let mut below = big_file(9_998, &calls);
    assert_eq!(below.nodes.len(), 9_999);
    let msg = resolve_loaded(&mut below, "fig:I1:2;3:4").unwrap_err().to_string();
    assert_eq!(calls.load(Ordering::SeqCst), 1, "9,999 nodes: one expansion");
    assert!(!msg.contains(HINT), "{msg}");
}

#[test]
fn every_miss_says_ids_of_expanded_content_are_per_session() {
    const SENTENCE: &str = "ids of expanded instance content are per session; use fig:<id> or a name";
    for selector in ["Card Z", "name:Card Z", "id:99", "fig:0:0", "42"] {
        let mut doc = crate::agent_cli_select_tests::two_cards();
        let msg = resolve_loaded(&mut doc, selector).unwrap_err().to_string();
        assert!(msg.contains(SENTENCE), "{selector}: {msg}");
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let mut expanded = lazy_cards(&calls);
    let msg = resolve_loaded(&mut expanded, "Card Z").unwrap_err().to_string();
    assert!(msg.contains("all pages expanded") && msg.contains(SENTENCE), "a miss after an expansion says both: {msg}");
}
