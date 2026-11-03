//! The folder look behind the watcher: what files are watched, and what changed since the last look.

use crate::hash::ContentHash;
use crate::manifest::MANIFEST_FILE;
use crate::paths::page_path_problem;
use crate::store::read_capped;
use crate::watch::DocChange;
use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc::Sender;
use std::time::{Duration, SystemTime};

const MAX_DEPTH: usize = 8;
/// A file this recent when the watcher starts may have been written after the caller asked for it (file times
/// are rounded down by the system clock), so it is reported instead of being taken as already known.
const START_SLACK: Duration = Duration::from_millis(10);

pub(crate) type Stat = (Option<SystemTime>, u64);

pub(crate) struct Known {
    stat: Stat,
    /// Hash last reported (or seen) for this file; `None` until the first change.
    hash: Option<ContentHash>,
}

/// What the folder held when the watcher started, minus files written at or after `started_at`: those may have
/// changed between the caller's `start()` and this look, so the first `check` reports them.
pub(crate) fn baseline(dir: &Path, started_at: SystemTime) -> HashMap<String, Known> {
    let threshold = started_at.checked_sub(START_SLACK).unwrap_or(started_at);
    scan(dir).into_iter().filter(|(_, (modified, _))| modified.is_none_or(|m| m < threshold)).map(|(rel, stat)| (rel, Known { stat, hash: None })).collect()
}

/// Compares the folder with `known`, queues one `DocChange` per page that changed, appeared or vanished. Returns
/// whether anything was queued.
pub(crate) fn check(dir: &Path, known: &mut HashMap<String, Known>, tx: &Sender<DocChange>) -> bool {
    let now = scan(dir);
    let mut sent = false;
    let mut send = |change: DocChange| sent |= tx.send(change).is_ok();
    for (rel, stat) in &now {
        if known.get(rel).is_some_and(|k| k.stat == *stat) {
            continue;
        }
        // The size seen by `scan` may be stale (a file still being copied in), so the cap is applied to the read
        // itself. Read failure means it vanished again or is mid-rename: the next event or pass sees the settled state.
        let bytes = match read_capped(&dir.join(rel)) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => {
                // Over the store's read limit: remembered, so it is not read again until it changes, never reported.
                known.insert(rel.clone(), Known { stat: *stat, hash: None });
                continue;
            }
            Err(_) => continue,
        };
        // A file still being written changes while it is read: what was read is a piece of it. Leave it unrecorded;
        // the write that follows raises another event (or the next timer look) and the settled file is read then.
        let settled = std::fs::metadata(dir.join(rel)).is_ok_and(|m| (m.modified().ok(), m.len()) == *stat && m.len() == bytes.len() as u64);
        if !settled {
            continue;
        }
        let hash = ContentHash::of(&bytes);
        let unchanged = known.get(rel).is_some_and(|k| k.hash == Some(hash));
        known.insert(rel.clone(), Known { stat: *stat, hash: Some(hash) });
        if !unchanged {
            send(DocChange { rel: rel.clone(), hash: Some(hash) });
        }
    }
    let gone: Vec<String> = known.keys().filter(|rel| !now.contains_key(*rel)).cloned().collect();
    for rel in gone {
        known.remove(&rel);
        send(DocChange { rel, hash: None });
    }
    sent
}

/// Stat of every watched file: pages and the manifest. Hidden names, temp files and other extensions are not
/// watched. A missing folder is an empty one.
fn scan(dir: &Path) -> HashMap<String, Stat> {
    let mut out = HashMap::new();
    scan_into(dir, "", 0, &mut out);
    out
}

fn scan_into(dir: &Path, prefix: &str, depth: usize, out: &mut HashMap<String, Stat>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let rel = format!("{prefix}{name}");
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            if depth < MAX_DEPTH {
                scan_into(&entry.path(), &format!("{rel}/"), depth + 1, out);
            }
        } else if kind.is_file() && (rel == MANIFEST_FILE || page_path_problem(&rel).is_none()) {
            if let Ok(meta) = entry.metadata() {
                out.insert(rel, (meta.modified().ok(), meta.len()));
            }
        }
    }
}
