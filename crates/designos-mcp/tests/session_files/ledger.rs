//! The idempotency ledger file, with a link or a FIFO planted at its name or at the name its rewrite used to start from.
use super::scratch::{assert_untouched, old_temp_name, within_one_second, Scratch, VICTIM};
use designos_doc::Document;
use designos_mcp::tools::ledger::{Ledger, LedgerEntry, LedgerNode};
use serde_json::json;
use std::fs;

const LEDGER: &str = "idempotency-ledger.jsonl";

/// A stored write whose result is a node the open document does not have.
fn entry(key: &str) -> LedgerEntry {
    LedgerEntry {
        key: key.into(),
        tool: "designos_create_node".into(),
        hash: 1,
        undo_depth: Some(0),
        auto_fix_undo_depth: None,
        dead: false,
        live: vec![LedgerNode { id: 9999, kind: "frame".into() }],
        gone: Vec::new(),
        reply: json!({}),
        pre_auto_fix_reply: None,
        created_ms: 0,
    }
}

#[test]
fn clear_never_truncates_through_a_link() {
    let scratch = Scratch::new();
    let victim = scratch.victim(VICTIM);
    let link = scratch.link(LEDGER, &victim);
    let mut ledger = Ledger::load_from_path(&link);
    ledger.clear();
    assert_untouched(&victim, VICTIM, "clear");
}

#[test]
fn store_never_appends_through_a_link() {
    let scratch = Scratch::new();
    let victim = scratch.victim(VICTIM);
    let link = scratch.link(LEDGER, &victim);
    let mut ledger = Ledger::load_from_path(&link);
    ledger.store(entry("k1"));
    assert_untouched(&victim, VICTIM, "store");
    assert_eq!(ledger.len(), 1, "the entry is still kept in memory");
}

#[test]
fn the_rewrite_never_writes_through_a_link_at_the_old_temp_name() {
    let scratch = Scratch::new();
    let victim = scratch.victim(VICTIM);
    scratch.link(&old_temp_name("idempotency-ledger"), &victim);
    let path = scratch.session.join(LEDGER);
    let mut ledger = Ledger::load_from_path(&path);
    ledger.store(entry("k1"));
    // The editor shows another document: the entry is forgotten, and the file is rewritten without it.
    ledger.on_canvas_reload(0, &Document::new("Other"), false, 10);
    assert_eq!(ledger.len(), 0);
    assert_untouched(&victim, VICTIM, "rewrite");
    assert_eq!(fs::read(&path).unwrap(), b"", "the rewritten ledger holds no entries");
}

#[test]
fn a_fifo_at_the_ledger_never_blocks_load_clear_or_store() {
    let scratch = Scratch::new();
    let fifo = scratch.fifo(LEDGER);
    let path = fifo.clone();
    within_one_second(&fifo, move || {
        let mut ledger = Ledger::load_from_path(&path);
        ledger.clear();
        ledger.store(entry("k1"));
        ledger.len()
    });
}
