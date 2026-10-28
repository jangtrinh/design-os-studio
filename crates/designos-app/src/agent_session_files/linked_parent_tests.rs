//! Every writer and deleter that works in a session folder, run against a folder that is itself a link: the agent moved its
//! folder away and planted a link at the name. The target of the link keeps every name and every byte it had.

use super::folder_tests::{scratch_dir, Cleanup};
use super::hook_quoting_tests::job_in;
use super::*;
use crate::agent_sessions::{load_or_init_registry, release_session_lock, save_registry, SessionLock, SessionRegistry};
use crate::skills::hook_delivery::{remove_all_replies, remove_replies, reply_file, write_replies};
use std::collections::BTreeMap;

/// Every file below `dir` by its path relative to `dir`, with its bytes.
pub(super) fn contents(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut found = BTreeMap::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(folder) = pending.pop() {
        for entry in std::fs::read_dir(&folder).expect("read the target").flatten() {
            let path = entry.path();
            match std::fs::symlink_metadata(&path).expect("metadata") {
                meta if meta.is_dir() => pending.push(path),
                _ => drop(found.insert(path.strip_prefix(dir).expect("below the target").to_path_buf(), std::fs::read(&path).unwrap_or_default())),
            }
        }
    }
    found
}

struct Case {
    name: &'static str,
    /// Files that are in the target before the call, by name.
    bait: Vec<(String, Vec<u8>)>,
    act: Box<dyn Fn(&Path)>,
}

fn case(name: &'static str, bait: Vec<(String, Vec<u8>)>, act: impl Fn(&Path) + 'static) -> Case {
    Case { name, bait, act: Box::new(act) }
}

fn cases() -> Vec<Case> {
    let reply_name = reply_file(Path::new("."), 1, 0).file_name().expect("a name").to_string_lossy().into_owned();
    let lock = SessionLock { pid: std::process::id(), window: "w".into(), started_at: 1 };
    let reading = crate::agent_usage::UsageReading { input: 1, output: 2, cost: None, source: crate::agent_usage::UsageSource::ALL[0], as_of_ms: 1 };
    let record = || crate::skills::run_record::RunSkillRecord { session_id: "s".into(), tab_id: 1, runtime: "codex".into(), skills: Vec::new() };
    vec![
        case("the usage cache", vec![], move |dir| crate::agent_usage::save_cached_usage(dir, "key", &reading)),
        case("the run record", vec![], move |dir| drop(crate::skills::run_record::append_run_record(dir, record()))),
        case("the sessions registry", vec![], |dir| drop(save_registry(dir, &SessionRegistry::default()))),
        case("a corrupt registry that is moved aside", vec![("sessions.json".into(), b"not json".to_vec())], |dir| drop(load_or_init_registry(dir))),
        case("the skills reply files", vec![], |dir| {
            let _ = write_replies(dir, 1, &["context".to_string()], 0);
        }),
        case("one skills reply file removed", vec![(reply_name.clone(), b"{}".to_vec())], |dir| remove_replies(dir, 1, 0..1)),
        case("every skills reply file removed", vec![(reply_name, b"{}".to_vec())], remove_all_replies),
        case(
            "a stale hooks file removed",
            vec![("hooks.json".into(), crate::agent_approval_channel::HOOK_SCRIPT_NAME.as_bytes().to_vec())],
            crate::agent_permissions::remove_stale_session_hooks,
        ),
        case("the lock of this window released", vec![("session.lock".into(), serde_json::to_vec(&lock).expect("lock json"))], |dir| {
            release_session_lock(dir, "w")
        }),
        case("a session file", vec![], |dir| drop(atomic_write_session_file(&dir.join("AGENTS.md"), b"studio"))),
        case("all the agent files of a sync", vec![], |dir| drop(crate::terminal::write_session_agent_files_for_job(&job_in(dir)))),
    ]
}

#[test]
fn every_writer_and_deleter_leaves_the_target_of_a_linked_session_folder_as_it_was() {
    let mut failures = Vec::new();
    for case in cases() {
        let lib_root = scratch_dir("linked-parent-library");
        let target = scratch_dir("linked-parent-target");
        let _cleanup = Cleanup(vec![lib_root.clone(), target.clone()]);
        for (name, bytes) in &case.bait {
            std::fs::write(target.join(name), bytes).expect("a file in the target");
        }
        let linked = crate::agent_sessions::stable_session_dir(&lib_root, "file-1");
        std::fs::create_dir_all(linked.parent().expect("agent-sessions")).expect("agent-sessions folder");
        std::os::unix::fs::symlink(&target, &linked).expect("plant the link");
        let before = contents(&target);

        (case.act)(&linked);

        let after = contents(&target);
        if after != before {
            let changed: Vec<_> = before.keys().chain(after.keys()).filter(|path| before.get(*path) != after.get(*path)).collect();
            failures.push(format!("{}: the target changed at {changed:?}", case.name));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
