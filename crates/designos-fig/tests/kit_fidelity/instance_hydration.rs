//! Which instances hydration refused, which sat on deferred pages, and the no-change sync of every main.

use super::instances::{InstanceTruth, Truth};
use super::keys::NodeKeys;
use designos_doc::*;
use std::collections::HashSet;

/// Instances whose stored derived data hydration refused: they carry derived data in the file, and after every page
/// is loaded they hold no record of an imported layout.
pub fn refused(t: &Truth, doc: &Document, keys: &NodeKeys) -> HashSet<String> {
    t.instances
        .iter()
        .filter(|i| keys.by_source.get(&i.guid).and_then(|id| doc.get(*id)).is_some_and(|n| n.component.imported_layout.is_none()))
        .map(|i| i.guid.clone())
        .collect()
}

/// Of the refused instances: how many are top-level instances (no other instance holds them, the only ones hydration
/// records an imported layout for), and how many of those carry no override at all.
pub fn refused_split(t: &Truth, doc: &Document, keys: &NodeKeys, refused: &HashSet<String>) -> (usize, usize) {
    let holders: Vec<&InstanceTruth> = t
        .instances
        .iter()
        .filter(|i| refused.contains(&i.guid))
        .filter(|i| keys.by_source.get(&i.guid).is_some_and(|id| instance_root_of(doc, *id) == Some(*id)))
        .collect();
    (holders.len(), holders.iter().filter(|i| i.overrides.is_empty()).count())
}

/// Instances with derived data that sit on a page the import deferred (called before the pages are hydrated).
pub fn deferred(t: &Truth, doc: &Document) -> HashSet<String> {
    let wanted: HashSet<&str> = t.instances.iter().map(|i| i.guid.as_str()).collect();
    doc.nodes
        .values()
        .filter(|n| doc.is_pending_instance(n.id))
        .filter_map(|n| n.source_id.as_deref())
        .filter(|s| wanted.contains(s))
        .map(str::to_string)
        .collect()
}

/// A no-change sync of every main component that has instances.
pub fn sync_all(doc: &mut Document) -> usize {
    let mut mains: Vec<NodeId> = doc.nodes.values().filter(|n| matches!(n.kind, NodeKind::Component(_))).map(|n| n.id).collect();
    mains.sort_by_key(|i| i.0);
    let limit = std::env::var("SCOREBOARD_SYNC_LIMIT").ok().and_then(|s| s.parse().ok()).unwrap_or(usize::MAX);
    let (mut n, t0) = (0usize, std::time::Instant::now());
    for m in mains.into_iter().take(limit) {
        drop(sync_instances(doc, m));
        n += 1;
        if n.is_multiple_of(1000) {
            eprintln!("synced {n} mains in {:.0} s", t0.elapsed().as_secs_f64());
        }
    }
    n
}
