//! A command card only appears for a command that runs locally in the tab's session folder itself: the model chooses
//! `workdir` and `environment_id`, and the card shows neither.

use super::mapping_tests::{card, open_tab, run_in, session_folder};
use super::support::{fixture, scratch_dir, with, without};
use crate::codex_remote::card_mapping::{Mapped, OpenTabReason};
use crate::codex_remote::session_folder::SessionFolder;
use serde_json::{json, Value};

fn command_in(command: &str, cwd: Value) -> Value {
    with(with(fixture("command-approval-request"), "/params/command", json!(command)), "/params/cwd", cwd)
}

/// The probe: a destructive command that reads like cleaning up the session, run in another repository.
#[test]
fn a_destructive_command_aimed_at_another_folder_gets_no_buttons() {
    let elsewhere = command_in("git clean -fdx", json!("/Users/someone/other-repo"));
    assert_eq!(open_tab(run_in(&elsewhere, &session_folder())), OpenTabReason::CommandOutsideSession);
    let here = command_in("git clean -fdx", json!("/session"));
    assert_eq!(card(run_in(&here, &session_folder())).summary, "git clean -fdx", "the same command in the session folder is a card");
}

#[test]
fn a_subfolder_a_sibling_or_a_parent_is_another_folder() {
    for cwd in ["/session/sub", "/session/sub/deeper", "/session/.hidden", "/sessions", "/session2", "/session/..", "/session/./", "/", "/Session", "session"] {
        let request = command_in("ls", json!(cwd));
        assert_eq!(open_tab(run_in(&request, &session_folder())), OpenTabReason::CommandOutsideSession, "{cwd}");
    }
}

#[test]
fn the_session_folder_with_a_trailing_slash_is_still_the_session_folder() {
    for cwd in ["/session/", "/session//"] {
        assert_eq!(card(run_in(&command_in("ls", json!(cwd)), &session_folder())).summary, "ls", "{cwd}");
    }
}

#[test]
fn the_canonical_form_counts_and_a_link_to_the_folder_does_not() {
    let dir = scratch_dir("q5");
    let (real, link, other_link) = (dir.join("real"), dir.join("link"), dir.join("other-link"));
    std::fs::create_dir(&real).unwrap();
    std::os::unix::fs::symlink(&real, &link).unwrap();
    std::os::unix::fs::symlink(&real, &other_link).unwrap();
    let session = SessionFolder::new(&link);
    let canonical = std::fs::canonicalize(&real).unwrap();
    assert_ne!(canonical, link, "the folder Studio holds is a link, so its canonical form is another name");
    for name in [link.to_str().unwrap().to_string(), format!("{}/", link.display()), canonical.to_str().unwrap().to_string()] {
        assert_eq!(card(run_in(&command_in("ls", json!(name)), &session)).summary, "ls", "{name}");
    }
    let swapped = command_in("ls", json!(other_link.to_str().unwrap()));
    assert_eq!(open_tab(run_in(&swapped, &session)), OpenTabReason::CommandOutsideSession, "the request's own folder is never resolved");
}

#[test]
fn a_command_without_a_folder_is_incomplete() {
    let base = fixture("command-approval-request");
    let missing = [
        without(base.clone(), "/params/cwd"),
        with(base.clone(), "/params/cwd", json!(null)),
        with(base.clone(), "/params/cwd", json!("")),
        with(base, "/params/cwd", json!(7)),
    ];
    for request in missing {
        assert_eq!(open_tab(run_in(&request, &session_folder())), OpenTabReason::Incomplete, "{request}");
    }
}

#[test]
fn only_a_local_environment_gets_buttons() {
    let base = fixture("command-approval-request");
    for other in [json!("remote-devbox"), json!(""), json!("Local"), json!(7), json!(["local"])] {
        let request = with(base.clone(), "/params/environmentId", other.clone());
        assert_eq!(open_tab(run_in(&request, &session_folder())), OpenTabReason::CommandOutsideSession, "{other}");
    }
    for same in [
        with(base.clone(), "/params/environmentId", json!("local")),
        with(base.clone(), "/params/environmentId", json!(null)),
        without(base, "/params/environmentId"),
    ] {
        assert!(matches!(run_in(&same, &session_folder()), Mapped::Card(_)), "{same}");
    }
}
