//! A tailer decides what to read from the length and the identity of the file. They come from the file it opened (so never from the
//! target of a link), or the swap of a link for the log would be taken for a rewrite of the log, and the log read again from its top.

use super::folder_tests::target_with_sentinel;
use super::test_support::made_session_folder;
use std::os::unix::fs::symlink;

#[test]
fn a_link_swapped_in_for_the_patch_log_does_not_reset_what_was_read() {
    let (session, _session) = made_session_folder("patch-link");
    let log = session.join("doc-patch.jsonl");
    std::fs::write(&log, b"{\"v\":1}\n").expect("a patch log");
    let mut patch_log = crate::live_patch_log::LivePatchLog::bind(log.clone());
    let read_so_far = patch_log.current_offset();
    assert!(read_so_far > 0, "the log held a line when it was bound");
    let (target, _target) = target_with_sentinel("patch-link-target");
    std::fs::write(target.join("victim.jsonl"), b"{\"v\":1}\n".repeat(50)).expect("a bigger file elsewhere");
    std::fs::remove_file(&log).expect("the agent removes the log");
    symlink(target.join("victim.jsonl"), &log).expect("and plants a link in its place");

    let (lines, errors) = patch_log.poll();

    assert_eq!((lines.len(), errors.len()), (0, 0), "the victim was read");
    assert_eq!(patch_log.current_offset(), read_so_far, "the victim's length and identity were taken for the log's, and the log was read again from its top");
}
