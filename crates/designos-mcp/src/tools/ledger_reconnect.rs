//! Replays the edit log onto the canvas a reconnecting session loaded, before the idempotency
//! ledger checks that a stored write is still present (E15-6 §3.4, §3.7, §8.8).
//!
//! The editor rewrites `canvas.designos` after it applied the edits it read from
//! `doc-patch.jsonl`, but it can lag behind the log. A log line carries the modification time of
//! the canvas its writer loaded (`LivePatch::base`), so a line made on the canvas that is on disk
//! now cannot be inside that canvas yet. A line made on an older canvas was written before the
//! current snapshot and is taken as absorbed: replaying it could undo the user's later edits.

use designos_doc::live_patch::{replay_live_patch, LiveLogLine, LivePatch, LivePatchState};
use designos_doc::Document;

/// The edit-log lines made on the canvas whose modification time is `canvas_base` (`None` when
/// there is no canvas file), after the last document switch in the log. Unreadable lines are skipped
/// and counted on stderr.
pub fn unabsorbed_patches(log: &[u8], canvas_base: Option<u64>) -> Vec<LivePatch> {
    let mut patches = Vec::new();
    let mut skipped = 0usize;
    for line in log.split(|b| *b == b'\n').filter(|l| !l.iter().all(u8::is_ascii_whitespace)) {
        match LiveLogLine::parse(line) {
            Ok(LiveLogLine::Patch(patch)) => patches.push(patch),
            // Everything above a switch was made on the previous document.
            Ok(LiveLogLine::DocEpoch(_)) => patches.clear(),
            Err(_) => skipped += 1,
        }
    }
    if skipped > 0 {
        // A crashed writer can leave a torn line. Say so: keys whose write sat in it answer STALE.
        eprintln!("designos-mcp: skipped {skipped} unreadable line(s) in the edit log while replaying it onto the loaded canvas");
    }
    patches.retain(|p| p.base == canvas_base);
    patches
}

/// Replays [`unabsorbed_patches`] onto `doc` without recording history or live ops, so nothing is
/// published to the editor again. Returns how many patches were replayed. When a patch cannot be
/// replayed `doc` is restored to the loaded canvas and the reason is returned.
pub fn replay_unabsorbed_patches(doc: &mut Document, log: &[u8], canvas_base: Option<u64>) -> Result<usize, String> {
    let patches = unabsorbed_patches(log, canvas_base);
    if patches.is_empty() {
        return Ok(0);
    }
    let loaded = doc.clone();
    let mut state = LivePatchState::default();
    for patch in &patches {
        if let Err(e) = replay_live_patch(doc, patch, &mut state, &mut |d, c| c.apply(d).map(|_| ())) {
            *doc = loaded;
            return Err(e.to_string());
        }
    }
    Ok(patches.len())
}
