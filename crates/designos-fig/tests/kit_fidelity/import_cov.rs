//! Part 5, import coverage: for every `NodeChange` key the file uses, the share of its occurrences that the importer
//! maps to the model or keeps as metadata.
//!
//! - mapped: the key is in the importer's own list of the fields it reads (`MAPPED_NODE_CHANGE_FIELDS` in
//!   `src/fig.rs`, read from the source so the list cannot drift). This is the importer's declaration, not a check
//!   of each value; part 6 checks the values a save hands back.
//! - kept: the imported layer carries the key in its `fig_metadata`.
//! - dropped: neither. Records that did not become a layer (variables, the document) count as mapped only for listed
//!   keys.

use super::board::{broken_comparator, Part};
use super::feature::feature_of;
use super::keys::NodeKeys;
use super::raw::{keys_of, record_guid, Raw};
use designos_doc::Document;
use std::collections::BTreeSet;

const FIG_RS: &str = include_str!("../../src/fig.rs");

/// The importer's list of the `NodeChange` fields it reads.
pub fn mapped_fields() -> BTreeSet<String> {
    let start = FIG_RS.find("const MAPPED_NODE_CHANGE_FIELDS").expect("the importer's field list");
    let body = &FIG_RS[start..];
    let body = &body[body.find("= &[").expect("list start")..body.find("];").expect("list end")];
    body.split('"').skip(1).step_by(2).map(str::to_string).collect()
}

/// One live record: its guid and its top-level keys.
pub struct RecordKeys {
    pub guid: String,
    pub keys: Vec<String>,
}

pub fn truth(raw: &Raw) -> Vec<RecordKeys> {
    raw.live().filter_map(|r| Some(RecordKeys { guid: record_guid(r)?, keys: keys_of(r).into_iter().map(str::to_string).collect() })).collect()
}

pub fn part(records: &[RecordKeys], doc: &Document, keys: &NodeKeys) -> Part {
    let mut p = Part::new("import_coverage", "Import coverage", "share of a key's occurrences the importer maps (its own field list) or keeps as metadata");
    let mapped = mapped_fields();
    p.note(format!("importer field list: {} keys", mapped.len()));
    let broken = broken_comparator();
    for r in records {
        let node = keys.by_source.get(&r.guid).copied();
        let hidden = node.is_some_and(|n| keys.is_hidden(n));
        let meta = node.and_then(|n| doc.get(n)).map(|n| &n.fig_metadata);
        let mut all_ok = true;
        for k in &r.keys {
            let how = if meta.is_some_and(|m| m.contains_key(k)) {
                "kept"
            } else if mapped.contains(k) {
                "mapped"
            } else {
                "dropped"
            };
            let ok = broken || how != "dropped";
            all_ok &= ok;
            p.tally(&format!("key: {k}"), hidden, ok);
            p.tally(&format!("feature: {}", feature_of(k)), hidden, ok);
            p.tally("total occurrences", hidden, ok);
            p.detail(&format!("key: {k}"), how);
            if node.is_none() {
                p.detail(&format!("key: {k}"), "on a record with no layer");
            }
        }
        p.record(r.guid.clone(), hidden, all_ok, &["records".to_string()]);
    }
    p
}
