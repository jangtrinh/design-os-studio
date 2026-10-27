//! An agent can plant a link INSIDE its own session folder (a Codex `workspace-write` sandbox allows it). Studio must then
//! write nothing through it: the target of every link stays exactly as it was.

use super::folder_tests::{only_entry_is_the_sentinel, target_with_sentinel, Cleanup};
use super::hook_quoting_tests::job_in;
use super::refusal::{refusal_of, Refusal};
use super::test_support::{made_session_folder, plant_link};
use super::*;
use crate::live_sync_tests::unique_tab_id;
use std::os::unix::fs::{symlink, PermissionsExt};

/// Every folder below a session folder that Studio writes a file into.
const WRITTEN_FOLDERS: &[&str] = &[
    ".gemini",
    ".grok",
    ".grok/hooks",
    ".agents",
    ".agents/plugins",
    ".agents/plugins/designos",
    ".agents/skills",
    ".agents/skills/designos-studio",
    ".claude",
    ".claude/skills",
    ".claude/skills/designos-studio",
];

fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).expect("read the target").flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

#[test]
fn a_link_at_any_folder_below_the_session_folder_receives_no_session_file() {
    let mut failures = Vec::new();
    for rel in WRITTEN_FOLDERS {
        let (target, _target) = target_with_sentinel("inner-target");
        let (session, _session) = made_session_folder("inner");
        plant_link(&session, rel, &target);

        let result = crate::terminal::write_session_agent_files_for_job(&job_in(&session));

        if !only_entry_is_the_sentinel(&target) {
            failures.push(format!("{rel}: Studio wrote through the link, the target now holds {:?}", names_in(&target)));
        }
        match result {
            Err(e) if refusal_of(&e) == Some(Refusal::Link) && e.to_string().contains(&session.join(rel).display().to_string()) => {}
            other => failures.push(format!("{rel}: the refusal must be typed and name the link, got {other:?}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_link_at_a_folder_of_a_single_file_write_is_refused_and_its_target_is_untouched() {
    let mut failures = Vec::new();
    for rel in [".gemini", ".grok/hooks", ".agents/plugins"] {
        let (target, _target) = target_with_sentinel("inner-single");
        let (session, _session) = made_session_folder("single");
        plant_link(&session, rel, &target);

        let result = atomic_write_session_file(&session.join(rel).join("x.json"), b"studio");

        if !only_entry_is_the_sentinel(&target) {
            failures.push(format!("{rel}: the file went through the link, the target now holds {:?}", names_in(&target)));
        }
        if !result.as_ref().is_err_and(|e| refusal_of(e) == Some(Refusal::Link)) {
            failures.push(format!("{rel}: a link in the path must be refused as a link, got {result:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Plants a link at `<session>/<link_name>` that points at a file outside the session folder, runs `act`, and requires the
/// file to hold what it held before.
fn target_file_survives(link_name: &str, initial: &[u8], act: impl FnOnce(&Path), failures: &mut Vec<String>) {
    let (session, _session) = made_session_folder("leaf");
    let outside = session.parent().expect("sessions root").join(format!("outside-{}-{}.txt", std::process::id(), unique_tab_id()));
    let _outside = Cleanup(vec![outside.clone()]);
    std::fs::write(&outside, initial).expect("the file outside");
    symlink(&outside, session.join(link_name)).expect("plant the link");

    act(&session);

    if std::fs::read(&outside).expect("the file outside") != initial {
        failures.push(format!("a write through the link at {link_name} changed its target"));
    }
}

#[test]
fn a_link_at_a_file_name_studio_writes_never_changes_what_it_points_at() {
    let mut failures = Vec::new();
    target_file_survives(
        "AGENTS.md",
        b"mine",
        |session| {
            atomic_write_session_file(&session.join("AGENTS.md"), b"studio").expect("the link is replaced by Studio's file");
            assert_eq!(std::fs::read(session.join("AGENTS.md")).expect("studio's file"), b"studio");
        },
        &mut failures,
    );
    let record = || crate::skills::run_record::RunSkillRecord { session_id: "s".into(), tab_id: 1, runtime: "codex".into(), skills: Vec::new() };
    target_file_survives(
        "run.json",
        br#"{"runs":[]}"#,
        |session| {
            let _ = crate::skills::run_record::append_run_record(session, record());
        },
        &mut failures,
    );
    let reply = crate::skills::hook_delivery::reply_file(Path::new("."), 1, 0);
    let reply_name = reply.file_name().expect("a name").to_string_lossy().into_owned();
    target_file_survives(
        &reply_name,
        b"mine",
        |session| {
            crate::skills::hook_delivery::write_replies(session, 1, &["context".to_string()], 0);
        },
        &mut failures,
    );
    // The registry's temporary file has a name the agent can predict (`sessions.tmp.<pid>`).
    target_file_survives(
        &format!("sessions.tmp.{}", std::process::id()),
        b"mine",
        |session| {
            let _ = crate::agent_sessions::save_registry(session, &crate::agent_sessions::SessionRegistry::default());
        },
        &mut failures,
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_usage_cache_for_a_folder_that_does_not_exist_yet_makes_it_private() {
    let dir = ephemeral_session_dir(&format!("p{}-usage-{}", std::process::id(), unique_tab_id()));
    let _cleanup = Cleanup(vec![dir.clone()]);
    let reading = crate::agent_usage::UsageReading { input: 1, output: 2, cost: None, source: crate::agent_usage::UsageSource::ALL[0], as_of_ms: 1 };

    crate::agent_usage::save_cached_usage(&dir, "key", &reading);

    assert_eq!(std::fs::metadata(&dir).expect("the folder").permissions().mode() & 0o777, 0o700);
    assert!(crate::agent_usage::load_cached_usage(&dir, "key").is_some(), "the cache is written");
}
