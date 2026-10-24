//! The two steps of Studio's side of the Codex sandbox probe (`plans/reports/v062-fix-1061-evidence/scripts/codex-sandbox-probe.sh`).
//! The script runs the agent's side itself, in a real Codex sandbox whose working folder is the session folder these steps make.
//! Both are ignored in a normal run: they only mean something with `DESIGNOS_PROBE_SESSION` set by the script.

use super::hook_quoting_tests::job_in;
use super::*;

fn probe_session() -> PathBuf {
    PathBuf::from(std::env::var_os("DESIGNOS_PROBE_SESSION").expect("DESIGNOS_PROBE_SESSION names the session folder"))
}

#[test]
#[ignore = "run by the Codex sandbox probe script"]
fn probe_make_session_folder() {
    let session = probe_session();
    match ensure_session_dir(&session) {
        Ok(()) => println!("PROBE make: ok {}", session.display()),
        Err(e) => println!("PROBE make: {}", files_not_updated_message(&e)),
    }
}

#[test]
#[ignore = "run by the Codex sandbox probe script"]
fn probe_write_session_files() {
    let session = probe_session();
    match crate::terminal::write_session_agent_files_for_job(&job_in(&session)) {
        Ok(()) => println!("PROBE write: ok"),
        Err(e) => println!("PROBE write: refused: {}", files_not_updated_message(&e)),
    }
}
