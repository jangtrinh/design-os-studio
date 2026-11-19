//! A document with pages that are not loaded yet is never written natively: the file would hold their
//! instances empty, and nothing could repair it later. The pages must be loaded first, in place.

use designos_doc::{Command, Document, FrameProps, Node, NodeId, NodeKind, PageHydrator, Rect, TextStyle};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// A loader like the `.fig` importer's: the instance of an unopened page gets its child when the page loads.
#[derive(Debug, Default)]
struct Loader {
    /// Unopened page -> the instance on it that has no children yet.
    waiting: BTreeMap<NodeId, NodeId>,
}

impl PageHydrator for Loader {
    fn hydrate_page(&mut self, doc: &mut Document, page: NodeId) -> bool {
        let Some(instance) = self.waiting.remove(&page) else { return false };
        let id = doc.alloc_id();
        let label = Node::text(id, Rect::new(0.0, 0.0, 80.0, 14.0), "Go back", TextStyle::default());
        doc.insert(instance, label).expect("insert the label under the instance");
        true
    }

    fn hydrate_all(&mut self, doc: &mut Document) -> bool {
        let pages: Vec<NodeId> = self.waiting.keys().copied().collect();
        pages.into_iter().fold(false, |any, page| self.hydrate_page(doc, page) | any)
    }

    fn is_hydrated(&self, page: NodeId) -> bool {
        !self.waiting.contains_key(&page)
    }
}

struct Fixture {
    doc: Document,
    unopened_page: NodeId,
    instance: NodeId,
}

/// Two pages: the first is loaded, the second holds an instance that is still empty and waits in the loader.
fn document_with_an_unopened_page() -> Fixture {
    let mut doc = Document::new("deferred");
    doc.add_page("Cover");
    let unopened_page = doc.add_page("404 pages");
    let id = doc.alloc_id();
    let instance = Node::new(id, NodeKind::Instance { frame: FrameProps::default(), component: None, component_key: None });
    let instance = doc.insert(unopened_page, instance).expect("insert the instance");
    let loader = Loader { waiting: BTreeMap::from([(unopened_page, instance)]) };
    doc.page_hydrator = Some(Arc::new(Mutex::new(loader)));
    Fixture { doc, unopened_page, instance }
}

fn children_of(doc: &Document, id: NodeId) -> usize {
    doc.get(id).expect("the node exists").children.len()
}

#[test]
fn a_document_with_an_unloaded_page_is_not_written_natively_and_the_error_says_why() {
    let Fixture { doc, instance, .. } = document_with_an_unopened_page();
    assert_eq!(children_of(&doc, instance), 0, "precondition: the instance on the unopened page is empty");

    let err = doc.to_json().expect_err("a document with an unloaded page must not become a file");
    assert!(err.to_string().contains("not loaded yet"), "the error names the cause: {err}");
    assert!(doc.to_json_counted().is_err(), "the counted serializer refuses too");
}

#[test]
fn a_native_write_after_the_page_was_loaded_keeps_the_instance_content() {
    let Fixture { mut doc, unopened_page, instance } = document_with_an_unopened_page();
    assert!(doc.ensure_page_hydrated(unopened_page), "precondition: loading the page fills the instance");

    let reopened = Document::from_json(&doc.to_json().expect("every page is loaded")).expect("reopen the file");
    assert_eq!(children_of(&reopened, instance), 1, "the instance keeps its child");
}

#[test]
fn a_document_whose_loader_has_nothing_left_to_load_serializes_while_the_loader_stays_attached() {
    let Fixture { mut doc, unopened_page, .. } = document_with_an_unopened_page();
    doc.ensure_page_hydrated(unopened_page);
    assert!(doc.page_hydrator.is_some(), "precondition: only loading every page at once detaches the loader");
    doc.to_json().expect("nothing is pending, so a save is fine");
}

#[test]
fn deleting_the_unloaded_page_leaves_a_document_that_can_be_written() {
    let Fixture { mut doc, unopened_page, .. } = document_with_an_unopened_page();
    assert!(doc.to_json().is_err(), "precondition: the page blocks the write while it is part of the document");
    Command::RemovePage { id: unopened_page }.apply(&mut doc).expect("remove the page");
    assert!(doc.page_hydrator.is_some(), "precondition: the loader still lists the removed page");
    doc.to_json().expect("a page that is no longer in the document does not block the write");
}

#[test]
fn a_loader_that_cannot_be_asked_blocks_the_write() {
    let Fixture { doc, .. } = document_with_an_unopened_page();
    let loader = doc.page_hydrator.clone().expect("loader");
    let poisoned = Arc::clone(&loader);
    let crashed = std::thread::spawn(move || {
        let _held = poisoned.lock().expect("lock");
        panic!("the loader crashed while it ran");
    })
    .join();
    assert!(crashed.is_err() && loader.is_poisoned(), "precondition: the loader is poisoned");
    assert!(doc.to_json().is_err(), "an unknown state is not a licence to write");
}
