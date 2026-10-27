//! An agent may lock its own entry (macOS "Locked" in Finder, `chflags uchg`) or deny the delete of it with an ACL. The move into the
//! set-aside folder then fails with "permission denied" although every mode bit allows it, and the folder the refusal names has to be
//! the one that refuses, not a writable folder whose fix does not help.

use super::test_support::{is_there, plant, stamp_folders, tab_state_in};
use crate::terminal::{AgentCliPreset, PermissionMode};
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

const GROK: AgentCliPreset = AgentCliPreset::Grok;

/// Sets the flags of `path` (never through a link) and clears them again when it drops, so the test's folder can be removed.
struct Flags(PathBuf);

impl Flags {
    fn set(path: &Path, flags: libc::c_uint) -> Flags {
        chflags(path, flags);
        Flags(path.to_path_buf())
    }
}

impl Drop for Flags {
    fn drop(&mut self) {
        chflags(&self.0, 0);
    }
}

fn chflags(path: &Path, flags: libc::c_uint) {
    let c = CString::new(path.as_os_str().as_bytes()).expect("a path without NUL");
    // SAFETY: `c` is a NUL-terminated path that outlives the call.
    assert_eq!(unsafe { libc::chflags(c.as_ptr(), flags) }, 0, "chflags {}", path.display());
}

#[test]
fn a_locked_entry_is_named_as_locked_and_not_the_folder_it_is_in() {
    for (planted, entry) in [(".mcp.json", ".mcp.json"), (".grok/agents/explore.md", ".grok/agents")] {
        let mut st = tab_state_in(PermissionMode::Ask);
        plant(&st.session_dir, planted, "{}");
        let path = st.session_dir.join(entry);
        let _unlock = Flags::set(&path, libc::UF_IMMUTABLE);

        let message = st.spawn_tab(GROK, None).expect_err("a locked entry cannot be moved");

        assert_eq!(message, format!("Couldn't start Grok Build: {} is locked, so Studio can't move it. Unlock it, then try again.", path.display()), "{entry}");
        assert!(is_there(&path), "{entry}: nothing was lost");
    }
}

#[test]
fn a_locked_folder_the_entry_is_in_is_named_as_locked() {
    let mut st = tab_state_in(PermissionMode::Ask);
    plant(&st.session_dir, ".grok/agents/explore.md", "---\nname: explore\n---\n");
    let grok = st.session_dir.join(".grok");
    let _unlock = Flags::set(&grok, libc::UF_IMMUTABLE);

    let message = st.spawn_tab(GROK, None).expect_err("nothing can be moved out of a locked folder");

    assert_eq!(
        message,
        format!("Couldn't start Grok Build: {} is locked, so Studio can't move anything out of it. Unlock it, then try again.", grok.display())
    );
}

#[test]
fn an_entry_with_a_deny_delete_acl_gets_a_line_that_names_it_and_its_folder_and_does_not_say_make_it_writable() {
    let mut st = tab_state_in(PermissionMode::Ask);
    plant(&st.session_dir, ".mcp.json", "{}");
    let path = st.session_dir.join(".mcp.json");
    assert!(std::process::Command::new("/bin/chmod").arg("+a").arg("everyone deny delete").arg(&path).status().expect("chmod +a").success());

    let result = st.spawn_tab(GROK, None);

    let kept = if is_there(&path) { path.clone() } else { stamp_folders(&st.session_dir)[0].join(".mcp.json") };
    let _ = std::process::Command::new("/bin/chmod").arg("-N").arg(&kept).status();
    let message = result.expect_err("an entry that may not be deleted cannot be moved");
    assert_eq!(
        message,
        format!(
            "Couldn't start Grok Build: {} can't be moved out of {}: Permission denied (os error 13). Check its permissions and those of that folder, then try again.",
            path.display(),
            st.session_dir.display()
        )
    );
}
