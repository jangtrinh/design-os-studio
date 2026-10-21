//! Capacity limits of the idempotency ledger (E15-6 §3.5).

use super::ledger::{CallLedgerDecision, Ledger, LedgerEntry};
use designos_doc::Document;
use serde_json::json;

fn entry(key: &str, pad: usize) -> LedgerEntry {
    LedgerEntry {
        key: key.into(),
        tool: "designos_create_node".into(),
        hash: 1,
        undo_depth: Some(0),
        auto_fix_undo_depth: None,
        dead: false,
        live: Vec::new(),
        gone: Vec::new(),
        reply: json!({"pad": "x".repeat(pad)}),
        pre_auto_fix_reply: None,
        created_ms: 0,
    }
}

fn is_absent(ledger: &Ledger, key: &str) -> bool {
    ledger.check_call(key, "designos_create_node", 1, 0, &Document::new("d")) == CallLedgerDecision::Absent
}

#[test]
fn the_oldest_entries_are_dropped_past_the_entry_cap() {
    let mut ledger = Ledger::new().with_limits(2, usize::MAX);
    for key in ["k1", "k2", "k3"] {
        ledger.store(entry(key, 1));
    }
    assert_eq!(ledger.len(), 2);
    assert_eq!(ledger.dropped_count(), 1);
    assert!(is_absent(&ledger, "k1"), "a dropped key behaves as unknown");
    assert!(ledger.get("k2").is_some() && ledger.get("k3").is_some());
}

#[test]
fn the_oldest_entries_are_dropped_past_the_reply_byte_cap() {
    let one = serde_json::to_vec(&entry("k", 100).reply).unwrap().len();
    let mut ledger = Ledger::new().with_limits(100, one * 2 + 1);
    for key in ["k1", "k2", "k3"] {
        ledger.store(entry(key, 100));
    }
    assert_eq!(ledger.len(), 2);
    assert_eq!(ledger.dropped_count(), 1);
    assert_eq!(ledger.total_reply_bytes(), one * 2);
    assert!(is_absent(&ledger, "k1"));
}

#[test]
fn the_newest_entry_is_kept_even_when_its_reply_alone_exceeds_the_byte_cap() {
    let mut ledger = Ledger::new().with_limits(10, 8);
    ledger.store(entry("k1", 50));
    ledger.store(entry("k2", 50));
    assert_eq!(ledger.len(), 1);
    assert!(ledger.get("k2").is_some());
    assert_eq!(ledger.dropped_count(), 1);
}

#[test]
fn re_storing_a_key_replaces_it_without_counting_a_drop() {
    let mut ledger = Ledger::new().with_limits(2, usize::MAX);
    ledger.store(entry("k1", 1));
    ledger.store(entry("k1", 5));
    assert_eq!(ledger.len(), 1);
    assert_eq!(ledger.dropped_count(), 0);
}
