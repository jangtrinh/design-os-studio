//! A Codex workspace-write agent can `mkfifo` any name in its session folder. A plain `open` of a FIFO with no writer never
//! returns, so a read of a session file that blocks on one hangs whoever called it: at a tab start that is the UI thread.

use super::hook_quoting_tests::job_in;
use super::test_support::{made_session_folder, make_fifo};
use super::*;
use std::sync::mpsc;

/// Runs `work` on a thread and waits `budget` for it. On a timeout it opens the FIFO for writing, which frees a reader that
/// is blocked in `open`, so the thread ends, and reports the hang.
fn finishes_within<T: Send + 'static>(fifo: &Path, budget: Duration, work: impl FnOnce() -> T + Send + 'static) -> Result<T, &'static str> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || drop(tx.send(work())));
    rx.recv_timeout(budget).map_err(|_| {
        drop(std::fs::OpenOptions::new().write(true).open(fifo));
        "did not return: blocked on the FIFO"
    })
}

#[test]
fn a_fifo_at_the_mcp_config_does_not_hang_the_provisioning_of_a_session() {
    let (session, _session) = made_session_folder("fifo-provision");
    let fifo = session.join("mcp-config.json");
    make_fifo(&fifo);
    let job = job_in(&session);

    let result = finishes_within(&fifo, Duration::from_secs(30), move || crate::terminal::write_session_agent_files_for_job(&job));

    result.expect("provisioning returned").expect("the FIFO is refused as a config, and replaced by Studio's file");
    assert!(std::fs::symlink_metadata(&fifo).expect("the config").is_file(), "Studio's config replaced the FIFO");
}

#[test]
fn a_fifo_at_a_session_file_is_refused_at_once_by_every_read() {
    let (session, _session) = made_session_folder("fifo-reads");
    let fifo = session.join("owner.json");
    std::fs::remove_file(&fifo).expect("the owner marker Studio wrote");
    make_fifo(&fifo);

    let read = finishes_within(&fifo, Duration::from_secs(30), {
        let path = fifo.clone();
        move || read_session_file(&path)
    });
    let e = read.expect("returned").expect_err("a FIFO is not a plain file");
    assert!(e.to_string().contains("not a plain file"), "{e}");

    let marker = finishes_within(&fifo, Duration::from_secs(30), {
        let session = session.clone();
        move || read_owner_marker(&session)
    });
    assert!(marker.expect("returned").is_none(), "a FIFO is no owner marker");
}
