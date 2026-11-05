//! The one `--select` resolver (#1127): grammar, name order, bare numbers, duplicate Figma ids, lazy pages and the
//! wording of a miss.

use crate::agent_cli_select_tests::two_cards;
use crate::select_target::{resolve, By};
use designos_doc::{Document, Node, NodeId, PageHydrator, Rect};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub(super) fn named(id: u64, name: &str, x: f32) -> Node {
    let mut n = Node::rectangle(NodeId(id), Rect::new(x, 0.0, 10.0, 10.0));
    n.name = name.into();
    n
}

pub(super) fn node_of(doc: &mut Document, selector: &str) -> Option<NodeId> {
    resolve_loaded(doc, selector).unwrap_or_else(|e| panic!("{selector}: {e}")).node
}

/// The watermark a caller records at load, before this resolve expands anything.
pub(super) fn resolve_loaded(doc: &mut Document, raw: &str) -> Result<crate::select_target::Target, crate::select_target::SelectError> {
    let load_next_id = doc.peek_next_id();
    resolve(doc, raw, load_next_id)
}

#[test]
fn resolve_by_id_fig_and_name() {
    let mut doc = two_cards();
    // A layer whose name looks like a selector prefix is reached with `name:`.
    doc.insert(NodeId(2), named(7, "id:3", 0.0)).unwrap();

    let by_id = resolve_loaded(&mut doc, "id:5").unwrap();
    assert_eq!((by_id.node, by_id.matched_by), (Some(NodeId(5)), By::Id));
    let by_fig = resolve_loaded(&mut doc, "fig:12:34").unwrap();
    assert_eq!((by_fig.node, by_fig.matched_by), (Some(NodeId(5)), By::Figma));
    assert_eq!(node_of(&mut doc, "fig:I12:34;56:78"), Some(NodeId(6)), "a Figma id may hold colons and semicolons");
    let by_name = resolve_loaded(&mut doc, "Card B").unwrap();
    assert_eq!((by_name.node, by_name.matched_by), (Some(NodeId(5)), By::Name));
    assert_eq!(node_of(&mut doc, "name:Card B"), Some(NodeId(5)));
    let loose = resolve_loaded(&mut doc, "card b").unwrap();
    assert_eq!((loose.node, loose.matched_by), (Some(NodeId(5)), By::NameCaseInsensitive));

    assert_eq!(node_of(&mut doc, "id:3"), Some(NodeId(3)), "`id:` is the node id");
    assert_eq!(node_of(&mut doc, "name:id:3"), Some(NodeId(7)), "`name:` takes the rest literally");
}

#[test]
fn name_order_prefers_page_then_screen_then_walk() {
    // Page 1 > [ CARD (2), Group (3) > [ Card (4) ], Hero (5) ]  and a layer deeper down also called Hero (6).
    let mut doc = Document::new("order");
    let page = doc.add_page("Page 1");
    doc.insert(page, named(2, "CARD", 0.0)).unwrap();
    let mut group = Node::frame(NodeId(3), Rect::new(0.0, 0.0, 100.0, 100.0));
    group.name = "Group".into();
    doc.insert(page, group).unwrap();
    doc.insert(NodeId(3), named(4, "Card", 0.0)).unwrap();
    doc.insert(NodeId(3), named(6, "Hero", 0.0)).unwrap();
    doc.insert(page, named(5, "Hero", 0.0)).unwrap();

    assert_eq!(node_of(&mut doc, "Hero"), Some(NodeId(5)), "a screen beats a deeper layer that comes first in the walk");
    assert_eq!(node_of(&mut doc, "Card"), Some(NodeId(4)), "an exact name beats a case-insensitive one that comes first");
    let loose = resolve_loaded(&mut doc, "card").unwrap();
    assert_eq!((loose.node, loose.matched_by), (Some(NodeId(2)), By::NameCaseInsensitive), "without an exact name the first case-insensitive layer wins");
}

#[test]
fn bare_number_matching_name_and_id_is_ambiguous() {
    let mut doc = Document::new("numbers");
    let page = doc.add_page("Page 1");
    doc.insert(page, named(2, "4", 0.0)).unwrap();
    doc.insert(page, named(4, "Four", 20.0)).unwrap();

    let msg = resolve_loaded(&mut doc, "4").expect_err("a bare number that is both a layer name and a node id").to_string();
    assert_eq!(msg, "--select 4 is ambiguous: layer \"4\" (id 2) and node id 4; use id:4 or name:4");
    assert_eq!(node_of(&mut doc, "id:4"), Some(NodeId(4)));
    assert_eq!(node_of(&mut doc, "name:4"), Some(NodeId(2)));

    doc.remove(NodeId(2)).unwrap();
    let by_id = resolve_loaded(&mut doc, "4").unwrap();
    assert_eq!((by_id.node, by_id.matched_by), (Some(NodeId(4)), By::BareId), "with no layer named 4, the bare number is the node id");

    doc.insert(page, named(8, "4000", 40.0)).unwrap();
    let by_name = resolve_loaded(&mut doc, "4000").unwrap();
    assert_eq!((by_name.node, by_name.matched_by), (Some(NodeId(8)), By::Name), "a number that names a layer and no node is the name");
}

#[test]
fn fig_id_duplicates_are_an_error() {
    let mut doc = Document::new("duplicates");
    let page = doc.add_page("Page 1");
    for (i, id) in (100..112).enumerate() {
        let mut n = named(id, &format!("Copy {i}"), i as f32 * 20.0);
        n.source_id = Some("12:34".into());
        doc.insert(page, n).unwrap();
    }
    let msg = resolve_loaded(&mut doc, "fig:12:34").expect_err("a Figma id is meant to be unique").to_string();
    assert!(msg.contains("12:34"), "{msg}");
    assert!(msg.contains("100") && msg.contains("109"), "the first ten ids are listed: {msg}");
    assert!(!msg.contains("110") && !msg.contains("111"), "no more than ten ids are listed: {msg}");

    let mut two = Document::new("two copies");
    let page = two.add_page("Page 1");
    for id in [7, 9] {
        let mut n = named(id, "Copy", 0.0);
        n.source_id = Some("5:5".into());
        two.insert(page, n).unwrap();
    }
    let msg = resolve_loaded(&mut two, "fig:5:5").unwrap_err().to_string();
    assert!(msg.contains('7') && msg.contains('9'), "{msg}");
}

/// A page hydrator that stays attached (instances are still unexpanded) and keeps claiming progress for its first
/// calls: a resolver that retries until nothing changes would call it again and again.
#[derive(Debug)]
pub(super) struct CountingHydrator {
    pub(super) calls: Arc<AtomicUsize>,
    pub(super) late: Option<(NodeId, Node)>,
}

impl PageHydrator for CountingHydrator {
    fn hydrate_page(&mut self, _doc: &mut Document, _page: NodeId) -> bool {
        false
    }
    fn hydrate_all(&mut self, doc: &mut Document) -> bool {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some((parent, node)) = self.late.take() {
            doc.insert(parent, node).unwrap();
        }
        n < 3
    }
    fn is_hydrated(&self, _page: NodeId) -> bool {
        false
    }
    fn unexpanded_instances(&self) -> usize {
        1
    }
}

/// `two_cards` plus a deferred layer, `Lazy Card` (id 9, Figma id 99:1), that exists only after hydration.
pub(super) fn lazy_cards(calls: &Arc<AtomicUsize>) -> Document {
    let mut doc = two_cards();
    let mut late = named(9, "Lazy Card", 0.0);
    late.source_id = Some("99:1".into());
    doc.page_hydrator = Some(Arc::new(Mutex::new(CountingHydrator { calls: calls.clone(), late: Some((NodeId(2), late)) })));
    doc
}

#[test]
fn miss_hydrates_lazy_pages_once() {
    // A name, and a Figma id in a file under 10,000 nodes, expand pages once. An id never does (see below).
    for selector in ["Lazy Card", "fig:99:1"] {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut doc = lazy_cards(&calls);
        assert_eq!(node_of(&mut doc, selector), Some(NodeId(9)), "{selector} is found once the pages are expanded");
        assert_eq!(calls.load(Ordering::SeqCst), 1, "{selector}: one hydration");
    }

    let calls = Arc::new(AtomicUsize::new(0));
    let mut doc = lazy_cards(&calls);
    let msg = resolve_loaded(&mut doc, "Nothing Here").unwrap_err().to_string();
    assert_eq!(calls.load(Ordering::SeqCst), 1, "a miss expands the pages once, not until nothing changes");
    assert!(msg.contains("all pages expanded; 1 instances left unexpanded by the clone budget"), "the message says the retry happened: {msg}");

    let calls = Arc::new(AtomicUsize::new(0));
    let mut doc = lazy_cards(&calls);
    for selector in ["id:5", "fig:12:34", "Card B", "name:Card A"] {
        node_of(&mut doc, selector);
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0, "a hit never expands a page");
}

#[test]
fn a_miss_says_what_was_searched() {
    let mut doc = two_cards();
    let msg = resolve_loaded(&mut doc, "Card Z").unwrap_err().to_string();
    assert_eq!(
        msg,
        "--select \"Card Z\" matched no node (searched 6 layers on 1 page by name; use --inspect-json to list screens, or id:<n> / fig:<id>) \
         (ids of expanded instance content are per session; use fig:<id> or a name)"
    );
    let msg = resolve_loaded(&mut doc, "id:99").unwrap_err().to_string();
    assert!(msg.starts_with("--select \"id:99\" matched no node (no node has id 99;"), "an id nothing has is a miss: {msg}");
    assert!(!msg.contains("belongs to an expanded instance"), "a typo is not said to belong to an instance: {msg}");
    doc.remove(NodeId(6)).unwrap();
    let msg = resolve_loaded(&mut doc, "id:6").unwrap_err().to_string();
    assert!(msg.starts_with("--select \"id:6\" matched no node") && msg.contains("by id"), "an id below the watermark that is not there is a miss: {msg}");
    let msg = resolve_loaded(&mut doc, "fig:0:0").unwrap_err().to_string();
    assert!(msg.contains("by Figma id"), "{msg}");
    assert!(!msg.contains("no Figma ids") && !msg.contains("10,000"), "a document with Figma ids gets no extra hint: {msg}");

    doc.add_page("Page 2");
    let msg = resolve_loaded(&mut doc, "Card Z").unwrap_err().to_string();
    assert!(msg.contains("on 2 pages"), "{msg}");
}

#[test]
fn a_figma_id_miss_explains_a_document_without_figma_ids() {
    let mut doc = Document::new("native");
    let page = doc.add_page("Page 1");
    doc.insert(page, named(2, "Box", 0.0)).unwrap();
    let msg = resolve_loaded(&mut doc, "fig:12:34").unwrap_err().to_string();
    assert!(msg.contains("this document has no Figma ids"), "{msg}");
}

#[test]
fn a_figma_id_miss_in_a_big_file_explains_instance_sublayers() {
    let mut doc = Document::new("kit");
    let page = doc.add_page("Page 1");
    for id in 2..10_050 {
        let mut n = named(id, "Layer", 0.0);
        n.source_id = Some(format!("1:{id}"));
        doc.insert(page, n).unwrap();
    }
    let msg = resolve_loaded(&mut doc, "fig:I1:2;3:4").unwrap_err().to_string();
    assert!(msg.contains("instance sublayers in files over 10,000 nodes keep no Figma id; select the instance and use id:<n>"), "{msg}");
}

#[test]
fn id_selector_never_hydrates() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut doc = lazy_cards(&calls);
    let err = resolve_loaded(&mut doc, "id:9").unwrap_err().to_string();
    assert!(err.contains("no node has id 9"), "node 9 exists only after hydration, which id: never runs: {err}");
    assert_eq!(calls.load(Ordering::SeqCst), 0, "id: does not expand pages");

    let calls = Arc::new(AtomicUsize::new(0));
    let mut doc = lazy_cards(&calls);
    let hit = resolve_loaded(&mut doc, "id:5").unwrap();
    assert_eq!(hit.node, Some(NodeId(5)));
    assert_eq!(calls.load(Ordering::SeqCst), 0, "a loaded id does not expand pages");
}

#[test]
fn fig_on_a_large_file_does_not_hydrate() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut doc = Document::new("kit");
    let page = doc.add_page("Page 1");
    for id in 2..10_002 {
        let mut n = named(id, "Layer", 0.0);
        n.source_id = Some(format!("1:{id}"));
        doc.insert(page, n).unwrap();
    }
    assert!(doc.nodes.len() >= 10_000, "the fixture is a large file ({})", doc.nodes.len());
    doc.page_hydrator = Some(Arc::new(Mutex::new(CountingHydrator { calls: calls.clone(), late: None })));
    let err = resolve_loaded(&mut doc, "fig:I1:2;3:4").unwrap_err().to_string();
    assert_eq!(calls.load(Ordering::SeqCst), 0, "fig: does not expand a file of 10,000 nodes or more");
    assert!(err.contains("instance sublayers in files over 10,000 nodes keep no Figma id"), "{err}");
    assert!(!err.contains("all pages expanded"), "a fig: miss that skipped hydration does not claim the pages were expanded: {err}");
}

#[test]
fn hydrated_ids_are_reported_unstable() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut doc = lazy_cards(&calls);
    let by_name = resolve_loaded(&mut doc, "Lazy Card").unwrap();
    assert_eq!(by_name.node, Some(NodeId(9)));
    assert!(!by_name.id_stable, "a name found only after hydration is above the load watermark");
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let calls = Arc::new(AtomicUsize::new(0));
    let mut doc = lazy_cards(&calls);
    let by_fig = resolve_loaded(&mut doc, "fig:99:1").unwrap();
    assert_eq!(by_fig.node, Some(NodeId(9)));
    assert!(!by_fig.id_stable, "a Figma id found only after hydration is above the load watermark");

    let calls = Arc::new(AtomicUsize::new(0));
    let mut doc = lazy_cards(&calls);
    let load_next_id = doc.peek_next_id();
    resolve(&mut doc, "Lazy Card", load_next_id).unwrap();
    let err = resolve(&mut doc, "id:9", load_next_id).unwrap_err().to_string();
    assert!(err.contains("id 9 was created in this session and is not stable across runs"), "the node exists now, above the watermark: {err}");
    assert_eq!(calls.load(Ordering::SeqCst), 1, "only the name lookup expanded pages; refusing the id did not");

    let mut doc = two_cards();
    let stable = resolve_loaded(&mut doc, "id:5").unwrap();
    assert_eq!((stable.node, stable.id_stable), (Some(NodeId(5)), true));
    let stable_name = resolve_loaded(&mut doc, "Card B").unwrap();
    assert!(stable_name.id_stable);
}

#[test]
fn a_malformed_id_is_an_error_not_a_name() {
    let mut doc = two_cards();
    for selector in ["id:abc", "id:-1", "id:1.5", "id:", "fig:", "name:", ""] {
        assert!(resolve_loaded(&mut doc, selector).is_err(), "{selector:?}");
    }
}

#[test]
fn several_layers_with_one_name_select_the_first_and_say_how_many() {
    let mut doc = two_cards();
    let t = resolve_loaded(&mut doc, "Title").unwrap();
    assert_eq!(t.node, Some(NodeId(4)), "the first in page order");
    assert_eq!(t.ambiguous, 2);
}

// The two cases below are #1145's own resolver tests, kept as they were written.

#[test]
fn duplicate_figma_ids_are_rejected() {
    let mut doc = Document::new("duplicate figma ids");
    let page = doc.add_page("Page");
    for id in [2, 3] {
        let mut node = Node::new(NodeId(id), designos_doc::NodeKind::Rectangle);
        node.source_id = Some("12:34".into());
        doc.insert(page, node).unwrap();
    }
    let err = resolve_loaded(&mut doc, "fig:12:34").unwrap_err().to_string();
    assert!(err.contains("ambiguous"), "{err}");
    assert!(err.contains('2') && err.contains('3'), "{err}");
}

#[test]
fn duplicate_names_report_count_while_selecting_first() {
    let mut doc = Document::new("duplicate names");
    let page = doc.add_page("Page");
    for id in [2, 3] {
        let mut node = Node::new(NodeId(id), designos_doc::NodeKind::Rectangle);
        node.name = "Card".into();
        doc.insert(page, node).unwrap();
    }
    let target = resolve_loaded(&mut doc, "name:Card").unwrap();
    assert_eq!(target.node, Some(NodeId(2)));
    assert_eq!(target.ambiguous, 2);
}
