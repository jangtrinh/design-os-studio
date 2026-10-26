//! Studio opens the folder that holds a file and then works through it. A hook swaps that folder for a link to somewhere else
//! at the exact point between the two, so a call that opened the folder and then went back to its path would fail these every
//! run, not by chance: each call must land in the folder it opened, and the target of the link must stay as it was.

use super::folder_tests::target_with_sentinel;
use super::linked_parent_tests::contents;
use super::session_io::test_hooks::set_after_open;
use super::test_support::made_session_folder;
use super::*;
use std::os::unix::fs::symlink;

/// One call on `<session>/.gemini/<key>`. `setup` names files that exist in the real folder and in the target before it runs,
/// and `lands` says where its effect has to show in the folder that was opened, which is `.gemini-moved` by then.
struct Case {
    name: &'static str,
    key: &'static str,
    setup: &'static [&'static str],
    call: fn(&Path) -> std::io::Result<()>,
    lands: fn(&Path) -> bool,
}

fn holds_studio(moved: &Path) -> bool {
    std::fs::read(moved.join("x.json")).is_ok_and(|bytes| bytes == b"studio")
}

const CASES: &[Case] = &[
    Case { name: "write", key: "x.json", setup: &[], call: |dir| atomic_write_session_file(&dir.join("x.json"), b"studio"), lands: holds_studio },
    Case {
        name: "write and read back",
        key: "x.json",
        setup: &[],
        call: |dir| write_session_file_verified(&dir.join("x.json"), b"studio"),
        lands: holds_studio,
    },
    Case {
        name: "create exclusive",
        key: "lock",
        setup: &[],
        call: |dir| create_session_file_exclusive(&dir.join("lock"), b"studio"),
        lands: |moved| moved.join("lock").is_file(),
    },
    Case {
        name: "remove file",
        key: "x.json",
        setup: &["x.json"],
        call: |dir| remove_session_file(&dir.join("x.json")),
        lands: |moved| !moved.join("x.json").exists(),
    },
    Case {
        name: "remove folder",
        key: "jobs",
        setup: &["jobs/a"],
        call: |dir| remove_session_dir_all(&dir.join("jobs")),
        lands: |moved| !moved.join("jobs").exists(),
    },
    Case {
        name: "rename",
        key: "old.json",
        setup: &["old.json"],
        call: |dir| rename_session_file(&dir.join("old.json"), &dir.join("new.json")),
        lands: |moved| moved.join("new.json").is_file() && !moved.join("old.json").exists(),
    },
    Case {
        name: "create folder",
        key: "jobs/job-1",
        setup: &[],
        call: |dir| create_session_dir_all(&dir.join("jobs").join("job-1")),
        lands: |moved| moved.join("jobs").join("job-1").is_dir(),
    },
];

#[test]
fn every_call_works_in_the_folder_it_opened_when_its_path_becomes_a_link_meanwhile() {
    let mut failures = Vec::new();
    for case in CASES {
        let (target, _target) = target_with_sentinel("swap-target");
        let (session, _session) = made_session_folder("swap");
        let (folder, moved) = (session.join(".gemini"), session.join(".gemini-moved"));
        std::fs::create_dir_all(&folder).expect("the real folder");
        for name in case.setup {
            for base in [&folder, &target] {
                let path = base.join(name);
                std::fs::create_dir_all(path.parent().expect("a folder")).expect("setup folder");
                std::fs::write(&path, b"theirs-or-ours").expect("setup file");
            }
        }
        let before = contents(&target);
        let (swapped, moved_to, link_to) = (folder.clone(), moved.clone(), target.clone());
        set_after_open(
            &folder.join(case.key),
            Box::new(move || {
                std::fs::rename(&swapped, &moved_to).expect("the agent moves the folder");
                symlink(&link_to, &swapped).expect("and plants a link in its place");
            }),
        );

        let result = (case.call)(&folder);

        if let Err(e) = result {
            failures.push(format!("{}: the call failed instead of working in the folder it opened: {e}", case.name));
        }
        if !(case.lands)(&moved) {
            failures.push(format!("{}: the effect is not in the folder that was opened", case.name));
        }
        if contents(&target) != before {
            failures.push(format!("{}: the target of the link changed", case.name));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
