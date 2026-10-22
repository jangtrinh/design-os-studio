//! The edit-log format version: lines written by this build carry the current one, older logs still read,
//! and a newer log is refused with a message that tells the user what to update.
use super::*;
use crate::{Command, Document, Node, NodeId, Rect};

const UPDATE_HINT: &str = "Update Studio and designos-mcp to the same version.";

fn doc_with_frame() -> (Document, NodeId) {
    let mut doc = Document::new("versions");
    let page = doc.add_page("Page");
    let frame = doc.alloc_id();
    doc.insert(page, Node::frame(frame, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
    (doc, frame)
}

fn replay(doc: &mut Document, patch: &LivePatch) -> LivePatchReport {
    let mut state = LivePatchState::default();
    replay_live_patch(doc, patch, &mut state, &mut |d, c: Command| c.apply(d).map(|_| ())).unwrap()
}

fn remove_patch(id: NodeId, v: u32) -> LivePatch {
    let mut patch = LivePatch::new(vec![LiveOp::Remove { id }]);
    patch.v = v;
    patch
}

#[test]
fn the_format_version_is_two_and_new_lines_carry_it() {
    assert_eq!(LIVE_PATCH_VERSION, 2, "the v0.6 operations (variables, styles, library, images, in-progress, session fields) are format 2");
    assert_eq!(MIN_LIVE_PATCH_VERSION, 1, "version 1 logs still read");
    assert_eq!(LivePatch::new(Vec::new()).v, LIVE_PATCH_VERSION);
    assert_eq!(DocEpoch::of(1, b"doc").v, LIVE_PATCH_VERSION);
}

#[test]
fn a_version_one_patch_still_replays() {
    let (mut doc, frame) = doc_with_frame();
    let report = replay(&mut doc, &remove_patch(frame, 1));
    assert_eq!(report.applied, 1, "{:?}", report.skipped);
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    assert!(doc.get(frame).is_none(), "the op was applied");
}

#[test]
fn a_current_patch_replays() {
    let (mut doc, frame) = doc_with_frame();
    let report = replay(&mut doc, &remove_patch(frame, LIVE_PATCH_VERSION));
    assert_eq!(report.applied, 1, "{:?}", report.skipped);
    assert!(doc.get(frame).is_none());
}

#[test]
fn a_newer_patch_is_refused_with_an_update_message_and_leaves_the_document_alone() {
    let (mut doc, frame) = doc_with_frame();
    let before = doc.to_json().unwrap();
    let report = replay(&mut doc, &remove_patch(frame, LIVE_PATCH_VERSION + 1));
    assert_eq!(report.applied, 0);
    assert_eq!(report.skipped.len(), 1, "{:?}", report.skipped);
    assert!(report.skipped[0].contains(UPDATE_HINT), "the message names the fix: {}", report.skipped[0]);
    assert!(report.skipped[0].contains(&(LIVE_PATCH_VERSION + 1).to_string()), "the message names the version it met: {}", report.skipped[0]);
    assert_eq!(doc.to_json().unwrap(), before, "the document is untouched");
}

#[test]
fn version_zero_is_refused() {
    let (mut doc, frame) = doc_with_frame();
    let report = replay(&mut doc, &remove_patch(frame, 0));
    assert_eq!(report.applied, 0);
    assert_eq!(report.skipped.len(), 1, "{:?}", report.skipped);
    assert!(doc.get(frame).is_some());
}

#[test]
fn the_refusal_message_names_the_version_and_the_fix() {
    assert_eq!(log_version_refusal(MIN_LIVE_PATCH_VERSION), None);
    assert_eq!(log_version_refusal(LIVE_PATCH_VERSION), None);
    let newer = log_version_refusal(LIVE_PATCH_VERSION + 1).expect("a newer version is refused");
    assert_eq!(newer, format!("Log version {} is newer than this build reads. {UPDATE_HINT}", LIVE_PATCH_VERSION + 1));
    assert!(!newer.contains('('), "no parenthetical: {newer}");
    assert_eq!(UPDATE_BOTH_HINT, UPDATE_HINT);
    assert_eq!(log_version_refusal(0).as_deref(), Some("Log version 0 is not supported."));
}

#[test]
fn a_version_one_log_line_and_switch_still_parse() {
    let patch = serde_json::to_string(&remove_patch(NodeId(7), 1)).unwrap();
    assert!(matches!(LiveLogLine::parse(patch.as_bytes()), Ok(LiveLogLine::Patch(p)) if p.v == 1 && p.ops.len() == 1));
    let switch = br#"{"v":1,"doc_epoch":4,"bytes":0,"hash":0}"#;
    assert!(matches!(LiveLogLine::parse(switch), Ok(LiveLogLine::DocEpoch(e)) if e.v == 1 && e.doc_epoch == 4));
}

#[test]
fn a_newer_log_line_is_named_before_its_ops_are_read() {
    // A newer writer may add ops this build has never heard of; the reader must still say "newer", not "unreadable".
    let future = format!(r#"{{"v":{},"ops":[{{"from_the_future":{{"id":1}}}}]}}"#, LIVE_PATCH_VERSION + 1);
    assert_eq!(LiveLogLine::parse(future.as_bytes()).unwrap(), LiveLogLine::Newer { v: LIVE_PATCH_VERSION + 1 });
    // The same ops under a version this build reads are a real parse error.
    let broken = format!(r#"{{"v":{LIVE_PATCH_VERSION},"ops":[{{"from_the_future":{{"id":1}}}}]}}"#);
    assert!(LiveLogLine::parse(broken.as_bytes()).is_err());
}

#[test]
fn a_newer_document_switch_is_still_a_switch() {
    // Studio stops following the agent's old document at a switch, so a newer switch must not be read as an edit.
    let switch = format!(r#"{{"v":{},"doc_epoch":9,"bytes":3,"hash":4}}"#, LIVE_PATCH_VERSION + 1);
    assert!(matches!(LiveLogLine::parse(switch.as_bytes()), Ok(LiveLogLine::DocEpoch(e)) if e.v == LIVE_PATCH_VERSION + 1 && e.doc_epoch == 9));
}
