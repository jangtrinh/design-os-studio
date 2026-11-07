use crate::{Document, FrameProps, Node, NodeId, NodeKind, PageHydrator, PendingPagesError, Rect, TextStyle};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// A loader like the `.fig` importer's, with the failure modes of the real one.
#[derive(Debug, Default)]
struct Loader {
    /// Unopened page -> the instance on it that has no children yet.
    waiting: BTreeMap<NodeId, NodeId>,
    /// Loading a page fails: the page counts as seen, stays empty and the loader keeps the error.
    fail: bool,
    error: Option<String>,
    /// Instances the loader had to leave empty (copy budget).
    skipped: usize,
}

impl PageHydrator for Loader {
    fn hydrate_page(&mut self, doc: &mut Document, page: NodeId) -> bool {
        let Some(instance) = self.waiting.remove(&page) else { return false };
        if self.fail {
            self.error = Some("node id space exhausted".into());
            return false;
        }
        let id = doc.alloc_id();
        doc.insert(instance, Node::text(id, Rect::new(0.0, 0.0, 80.0, 14.0), "Go back", TextStyle::default())).expect("insert the label");
        true
    }

    fn hydrate_all(&mut self, doc: &mut Document) -> bool {
        let pages: Vec<NodeId> = self.waiting.keys().copied().collect();
        pages.into_iter().fold(false, |any, page| self.hydrate_page(doc, page) | any)
    }

    fn is_hydrated(&self, page: NodeId) -> bool {
        !self.waiting.contains_key(&page)
    }

    fn unexpanded_instances(&self) -> usize {
        self.skipped
    }

    fn take_unexpanded_instances(&mut self) -> usize {
        std::mem::take(&mut self.skipped)
    }

    fn last_error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    fn take_last_error(&mut self) -> Option<String> {
        self.error.take()
    }
}

/// Two pages: the first is loaded, the second holds an empty instance and waits in the loader.
fn document_with(loader: impl FnOnce(&mut Loader)) -> (Document, NodeId, NodeId) {
    let mut doc = Document::new("deferred");
    doc.add_page("Cover");
    let page = doc.add_page("404 pages");
    let id = doc.alloc_id();
    let instance = doc.insert(page, Node::new(id, NodeKind::Instance { frame: FrameProps::default(), component: None, component_key: None })).unwrap();
    let mut state = Loader { waiting: BTreeMap::from([(page, instance)]), ..Loader::default() };
    loader(&mut state);
    doc.page_hydrator = Some(Arc::new(Mutex::new(state)));
    (doc, page, instance)
}

fn label_of(doc: &Document, instance: NodeId) -> Option<String> {
    doc.get(instance)?.children.iter().filter_map(|c| doc.get(*c)).find_map(|n| match &n.kind {
        NodeKind::Text(t) => Some(t.content.clone()),
        _ => None,
    })
}

#[test]
fn a_document_without_a_loader_has_no_pending_pages_and_loads_nothing() {
    let mut doc = Document::new("plain");
    doc.add_page("Page 1");
    assert_eq!((doc.pending_page_count(), doc.has_pending_pages()), (0, false));
    assert_eq!(doc.load_pending_pages(), Ok(0));
    doc.to_json().expect("a plain document serializes");
}

#[test]
fn the_page_that_waits_for_its_content_is_counted_until_it_is_loaded() {
    let (mut doc, page, _) = document_with(|_| {});
    assert_eq!((doc.pending_page_count(), doc.has_pending_pages()), (1, true));
    assert!(doc.ensure_page_hydrated(page));
    assert_eq!((doc.pending_page_count(), doc.has_pending_pages()), (0, false));
    assert!(doc.page_hydrator.is_some(), "opening a page does not detach the loader");
}

#[test]
fn loading_the_pending_pages_fills_the_instances_in_place_and_detaches_the_loader() {
    let (mut doc, _, instance) = document_with(|_| {});
    assert_eq!(label_of(&doc, instance), None, "precondition: the instance is empty");

    assert_eq!(doc.load_pending_pages(), Ok(0));

    assert_eq!(label_of(&doc, instance).as_deref(), Some("Go back"), "the live document holds the content");
    assert!(doc.page_hydrator.is_none(), "nothing is left to load");
    let reopened = Document::from_json(&doc.to_json().expect("the loaded document serializes")).unwrap();
    assert_eq!(label_of(&reopened, instance).as_deref(), Some("Go back"), "the file holds the content");
}

#[test]
fn loading_twice_is_harmless() {
    let (mut doc, _, instance) = document_with(|_| {});
    assert_eq!(doc.load_pending_pages(), Ok(0));
    assert_eq!(doc.load_pending_pages(), Ok(0));
    assert_eq!(doc.get(instance).unwrap().children.len(), 1);
}

#[test]
fn a_page_that_fails_to_load_blocks_the_write_with_the_loaders_error() {
    let (mut doc, _, instance) = document_with(|l| l.fail = true);

    let err = doc.load_pending_pages().expect_err("a page that cannot be loaded must not be written empty");

    assert_eq!(err, PendingPagesError::LoadFailed("node id space exhausted".into()));
    assert!(err.to_string().contains("node id space exhausted"), "the message names the cause: {err}");
    assert_eq!(doc.get(instance).unwrap().children.len(), 0, "the page is still empty");
    assert_eq!(doc.hydration_error().as_deref(), Some("node id space exhausted"), "the error stays for the window to report");
    assert!(doc.load_pending_pages().is_err(), "a second try is refused as well while the error stands");
}

#[test]
fn instances_the_loader_had_to_leave_empty_are_reported_to_the_caller() {
    let (mut doc, _, _) = document_with(|l| l.skipped = 3);
    assert_eq!(doc.load_pending_pages(), Ok(3), "the caller says how many instances stay empty");
    assert_eq!(doc.load_pending_pages(), Ok(0), "each instance is reported once");
}

#[test]
fn a_loader_that_cannot_be_asked_counts_every_page_as_pending_and_cannot_load() {
    let (mut doc, _, _) = document_with(|_| {});
    let loader = doc.page_hydrator.clone().unwrap();
    let crashed = {
        let loader = Arc::clone(&loader);
        std::thread::spawn(move || {
            let _held = loader.lock().unwrap();
            panic!("the loader crashed while it ran");
        })
        .join()
    };
    assert!(crashed.is_err() && loader.is_poisoned(), "precondition: the loader is poisoned");

    assert_eq!(doc.pending_page_count(), 2, "the state is unknown, so every page counts");
    assert_eq!(doc.load_pending_pages(), Err(PendingPagesError::LoaderBroken));
    assert!(doc.to_json().unwrap_err().to_string().contains("Reopen the file"), "the message says the load cannot work");
}

#[test]
fn the_error_names_the_page_count_and_the_next_step() {
    let (doc, _, _) = document_with(|_| {});
    let one = doc.to_json_counted().expect_err("refused").to_string();
    assert_eq!(one, "1 page is not loaded yet. Load it first");
    let many = PendingPagesError::NotLoaded { pages: 95 }.to_string();
    assert_eq!(many, "95 pages are not loaded yet. Load them first");
}

#[test]
fn a_copy_taken_before_the_load_that_loads_first_leaves_the_original_refusing_instead_of_empty() {
    let (mut doc, _, instance) = document_with(|_| {});
    let mut copy = doc.clone();

    assert_eq!(copy.load_pending_pages(), Ok(0));
    assert_eq!(label_of(&copy, instance).as_deref(), Some("Go back"), "the copy holds the content");
    copy.to_json().expect("the copy writes natively");

    assert_eq!(doc.pending_page_count(), 1, "the original's instance is still empty");
    assert_eq!(doc.load_pending_pages(), Err(PendingPagesError::LoadedElsewhere { pages: 1 }), "it cannot load a page the copy used up");
    assert!(doc.to_json().is_err(), "it must not write the empty instance as a success");
}

#[test]
fn a_copy_whose_pages_the_original_loaded_refuses_to_write() {
    let (mut doc, _, instance) = document_with(|_| {});
    let mut copy = doc.clone();

    assert_eq!(doc.load_pending_pages(), Ok(0));
    doc.to_json().expect("the original writes natively");

    assert_eq!(copy.load_pending_pages(), Err(PendingPagesError::LoadedElsewhere { pages: 1 }));
    assert!(copy.page_hydrator.is_some(), "the copy keeps the loader: its page is not loaded");
    assert_eq!(label_of(&copy, instance), None);
    assert!(copy.to_json().is_err());
}

#[test]
fn a_copy_that_visits_the_page_leaves_the_original_refusing() {
    let (doc, page, _) = document_with(|_| {});
    let mut copy = doc.clone();
    assert!(copy.ensure_page_hydrated(page));
    assert_eq!(doc.pending_pages_error(), Some(PendingPagesError::LoadedElsewhere { pages: 1 }));
    assert!(doc.to_json().is_err());
}

#[test]
fn a_clone_put_back_as_the_live_document_still_loads_and_writes() {
    let (mut doc, _, instance) = document_with(|_| {});
    // The rollback of an edit puts a clone taken before the edit back as the live document.
    let snapshot = doc.clone();
    doc = snapshot;
    assert_eq!(doc.pending_page_count(), 1);
    assert_eq!(doc.load_pending_pages(), Ok(0));
    let reopened = Document::from_json(&doc.to_json().expect("the restored document writes")).unwrap();
    assert_eq!(label_of(&reopened, instance).as_deref(), Some("Go back"));
}

#[test]
fn a_clone_taken_after_the_load_is_complete() {
    let (mut doc, _, instance) = document_with(|_| {});
    assert_eq!(doc.load_pending_pages(), Ok(0));
    let copy = doc.clone();
    assert!(!copy.has_pending_pages());
    assert_eq!(label_of(&Document::from_json(&copy.to_json().unwrap()).unwrap(), instance).as_deref(), Some("Go back"));
}

#[test]
fn a_clone_taken_after_the_page_was_opened_has_nothing_pending() {
    let (mut doc, page, _) = document_with(|_| {});
    assert!(doc.ensure_page_hydrated(page));
    let copy = doc.clone();
    assert_eq!((doc.has_pending_pages(), copy.has_pending_pages()), (false, false));
}
