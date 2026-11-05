//! `--select` against what the window records when a document comes on screen (#1319): the load watermark of a
//! page-0 expansion, of a web capture opened at startup, and of the documents a running window swaps in.

use super::live_patch_agent_switch::{agent_document, agent_switches};
use super::tab_disk_state::{other_document, rewrite_on_disk};
use super::unsaved_dot::{external_sim_with, two_rects, wait};
use super::*;
use crate::agent_cli_select_tests::{run, text, two_cards, Scratch};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Page 0 grows one node the first time the window expands it, the way a deferred page does on open.
#[derive(Debug)]
struct PageZeroHydrator {
    done: bool,
    created: Arc<AtomicU64>,
}

impl designos_doc::PageHydrator for PageZeroHydrator {
    fn hydrate_page(&mut self, doc: &mut designos_doc::Document, page: NodeId) -> bool {
        if self.done || doc.pages.first().copied() != Some(page) {
            return false;
        }
        self.done = true;
        let id = doc.alloc_id();
        self.created.store(id.0, Ordering::SeqCst);
        doc.insert(page, designos_doc::Node::rectangle(id, designos_doc::Rect::new(0.0, 0.0, 10.0, 10.0))).is_ok()
    }

    fn hydrate_all(&mut self, doc: &mut designos_doc::Document) -> bool {
        let Some(page) = doc.pages.first().copied() else { return false };
        self.hydrate_page(doc, page)
    }

    fn is_hydrated(&self, _page: NodeId) -> bool {
        self.done
    }
}

/// The next file this thread opens comes with a page 0 that expands one node when the window shows it.
fn page_zero_expands(created: &Arc<AtomicU64>) {
    let created = created.clone();
    crate::io::after_next_load(move |doc| {
        doc.page_hydrator = Some(Arc::new(Mutex::new(PageZeroHydrator { done: false, created })));
    });
}

#[test]
fn gui_rejects_a_page0_hydrated_id_like_the_cli() {
    let dir = Scratch::new("gui_page0_watermark");
    let file = dir.two_cards();
    let created = Arc::new(AtomicU64::new(0));
    let id = {
        page_zero_expands(&created);
        let opened = Sim::with(Options { file: Some(file.clone()), ..Default::default() });
        let id = created.load(Ordering::SeqCst);
        assert_ne!(id, 0, "opening the file expanded a node on page 0");
        assert_eq!(opened.app().load_next_id, NodeId(id), "the watermark is the next id from before that expansion");
        assert!(opened.app().doc.get(NodeId(id)).is_some(), "the expanded node is in the document");
        id
    };

    page_zero_expands(&created);
    let s = Sim::with(Options { file: Some(file.clone()), select: Some(format!("id:{id}")), ..Default::default() });
    let msg = s.shell.select_miss.as_deref().expect("the id page 0 created is refused");
    assert!(msg.contains(&format!("id {id} was created in this session and is not stable across runs; use fig:<id> or a name")), "{msg}");
    assert!(s.app().selection.is_empty());

    let err = run(&[text(&file), "--select", &format!("id:{id}"), "--inspect-json"]).expect_err("the agent CLI never created that node");
    assert!(err.contains(&format!("no node has id {id}")), "{err}");
    // Where both refuse the same id, both say why it does not last and what to use instead.
    assert!(err.contains("ids of expanded instance content are per session; use fig:<id> or a name"), "{err}");
}

#[test]
fn gui_overlap_with_a_page0_hydrated_id_names_only_the_name_and_the_cli_takes_the_layer() {
    let dir = Scratch::new("gui_overlap_watermark");
    let mut doc = two_cards();
    doc.get_mut(NodeId(3)).unwrap().name = "7".into();
    let file = dir.path("sevens.designos");
    crate::io::save_path(&doc, &file).unwrap();
    let created = Arc::new(AtomicU64::new(0));
    page_zero_expands(&created);
    let s = Sim::with(Options { file: Some(file.clone()), select: Some("7".into()), ..Default::default() });
    assert_eq!(created.load(Ordering::SeqCst), 7, "page 0 expanded node 7, the id the bare number also names");
    let msg = s.shell.select_miss.as_deref().expect("a layer named 7 and a node 7 are two readings");
    assert!(msg.starts_with("--select 7 is ambiguous: layer \"7\" (id 3) and node id 7"), "{msg}");
    assert!(msg.contains("name:7") && msg.contains("per session"), "{msg}");
    assert!(!msg.contains("id:7"), "id:7 would be refused: {msg}");
    assert!(s.app().selection.is_empty());

    // The agent CLI never created node 7: a bare 7 is the layer named 7.
    let out = dir.path("spec.json");
    run(&[text(&file), "--select", "7", "--export-spec", text(&out)]).expect("the agent CLI takes the layer");
    let spec: serde_json::Value = serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();
    assert_eq!(spec["name"], "7");
}

fn card_capture(dir: &Scratch) -> std::path::PathBuf {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../designos-webcapture/tests/playwright/fixtures/card.capture.json");
    let cap = dir.path("card.capture.json");
    std::fs::copy(&src, &cap).unwrap();
    cap
}

#[test]
fn a_web_capture_opened_at_startup_is_addressed_by_id_in_the_window_and_the_cli_alike() {
    let dir = Scratch::new("gui_capture_watermark");
    let cap = card_capture(&dir);
    let root = {
        let imported = Sim::with(Options { file: Some(cap.clone()), ..Default::default() });
        imported.app().selection.first().copied().expect("the capture import selects its root")
    };
    let s = Sim::with(Options { file: Some(cap.clone()), select: Some(format!("id:{}", root.0)), ..Default::default() });
    assert!(s.shell.select_miss.is_none(), "{:?}", s.shell.select_miss);
    assert_eq!(s.app().selection, vec![root]);
    run(&[text(&cap), "--select", &format!("id:{}", root.0), "--inspect-json"]).expect("the agent CLI accepts the same id on the same file");
}

#[test]
fn import_capture_leaves_the_window_the_watermark_the_agent_cli_records() {
    let dir = Scratch::new("gui_import_capture_watermark");
    let cap = card_capture(&dir);
    // The agent CLI records the next id of the document it starts from, before the import, and resolves --select there.
    let out = dir.path("out.designos");
    let err = run(&["--import-capture", text(&cap), "--out", text(&out), "--select", "id:2"]).expect_err("the captured node does not exist before the import");
    assert!(err.contains("id:2"), "{err}");
    assert!(!out.exists(), "a refused --select changes nothing");

    let s = Sim::with(Options { import_capture: Some(cap), select: Some("id:2".into()), ..Default::default() });
    assert_eq!(s.app().load_next_id, NodeId(2), "the window keeps the next id of the document the import went into");
    let msg = s.shell.select_miss.as_deref().expect("the window refuses the captured id too, as the agent CLI does");
    assert!(msg.contains("id 2 was created in this session and is not stable across runs"), "{msg}");
    assert!(!msg.contains("belongs to an expanded instance"), "an imported capture is not an expanded instance: {msg}");
}

#[test]
fn an_agent_document_that_replaces_the_shown_one_brings_its_own_watermark() {
    let (mut s, _dir, _path) = external_sim_with(&two_rects());
    let before = s.app().load_next_id;
    let agent = agent_document();
    let expected = agent.peek_next_id();
    assert_ne!(before, expected, "precondition: the two documents allocate past different ids");
    agent_switches(&mut s, agent);
    assert_eq!(s.app().doc.name, "Agent opened this");
    assert_eq!(s.app().load_next_id, expected);
}

#[test]
fn a_document_read_back_from_its_file_brings_that_files_watermark() {
    let (mut s, _dir, path) = external_sim_with(&two_rects());
    let other = other_document();
    let expected = other.peek_next_id();
    assert_ne!(s.app().load_next_id, expected, "precondition: the two documents allocate past different ids");
    rewrite_on_disk(&path, &other);
    wait(&mut s, 1.5);
    assert_eq!(s.app().doc.name, "Written by another program", "the window followed the file");
    assert_eq!(s.app().load_next_id, expected);
}
