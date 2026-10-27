//! The `PATH` an agent tab gets in a test build, so that no test reaches a real agent CLI.
//!
//! A test's agent tab (a stub spawned under an agent preset, or a tab on the stub CLIs of `path_prepend`) gets `path_prepend` first, then
//! the test process's own `PATH` with every folder that holds an executable of an agent CLI left out. The runner's own tools stay where
//! they are, so a stub's `python3` or `sh` is the one the runner has. A fixed list of system folders would put a slow `/usr/bin/python3`
//! shim in its place on a macOS runner, and nothing at all on Windows, where those folders do not exist.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The executables of the agent CLIs Studio starts.
const AGENT_CLIS: [&str; 6] = ["claude", "codex", "gemini", "grok", "agy", "opencode"];

/// The file names `cli` can have in a folder: its bare name, and on Windows also the extensions a launcher or script takes.
fn executable_names(cli: &str, windows: bool) -> Vec<String> {
    let extensions: &[&str] = if windows { &["", ".exe", ".cmd", ".bat", ".ps1"] } else { &[""] };
    extensions.iter().map(|extension| format!("{cli}{extension}")).collect()
}

/// True when `path` is a file a shell could start: on Unix, one with an execute bit (a link counts as what it points at).
fn is_executable_file(path: &Path) -> bool {
    let Ok(meta) = path.metadata() else { return false };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// True when `dir` holds an executable of an agent CLI.
pub fn holds_agent_cli(dir: &Path) -> bool {
    AGENT_CLIS.iter().flat_map(|cli| executable_names(cli, cfg!(windows))).any(|name| is_executable_file(&dir.join(name)))
}

/// `prepend` first, then the folders of `inherited` that hold no agent CLI, in their order.
pub fn test_agent_path(prepend: &[PathBuf], inherited: Option<OsString>) -> OsString {
    let kept = inherited.iter().flat_map(std::env::split_paths).filter(|dir| !dir.as_os_str().is_empty() && !holds_agent_cli(dir));
    std::env::join_paths(prepend.iter().cloned().chain(kept)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh folder under the temp folder holding the named files; `executable` ones get mode 0755, the rest 0644.
    fn folder(tag: &str, files: &[(&str, bool)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("designos-test-agent-path-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (name, executable) in files {
            std::fs::write(dir.join(name), "#!/bin/sh\nexit 0\n").unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(dir.join(name), std::fs::Permissions::from_mode(if *executable { 0o755 } else { 0o644 })).unwrap();
            }
            #[cfg(not(unix))]
            let _ = executable;
        }
        dir
    }

    fn joined(dirs: &[&Path]) -> OsString {
        std::env::join_paths(dirs).unwrap()
    }

    #[test]
    fn a_folder_with_a_stand_in_claude_is_dropped_and_one_with_only_python3_is_kept() {
        let with_claude = folder("claude", &[("claude", true), ("python3", true)]);
        let only_python = folder("python", &[("python3", true)]);
        let stubs = folder("stubs", &[("gemini", true)]);
        let empty = folder("empty", &[]);
        let missing = std::env::temp_dir().join("designos-test-agent-path-missing");
        let inherited = joined(&[&with_claude, &only_python, &empty, &missing]);
        let path = test_agent_path(std::slice::from_ref(&stubs), Some(inherited));
        let kept: Vec<PathBuf> = std::env::split_paths(&path).collect();
        // The stubs come first and stay although they hold a `gemini`; the folder with the stand-in `claude` is gone, with its python3.
        assert_eq!(kept, vec![stubs.clone(), only_python.clone(), empty.clone(), missing]);
        for dir in [with_claude, only_python, stubs, empty] {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn each_agent_cli_name_drops_its_folder_and_a_file_that_cannot_run_does_not() {
        let mut dirs = Vec::new();
        for cli in AGENT_CLIS {
            dirs.push((cli, folder(&format!("each-{cli}"), &[(cli, true)])));
        }
        let not_runnable = folder("not-runnable", &[("codex", false), ("python3", true)]);
        let mut all: Vec<&Path> = dirs.iter().map(|(_, dir)| dir.as_path()).collect();
        all.push(&not_runnable);
        let path = test_agent_path(&[], Some(joined(&all)));
        let kept: Vec<PathBuf> = std::env::split_paths(&path).collect();
        assert_eq!(kept, vec![not_runnable.clone()], "only the folder whose `codex` cannot run is left");
        for (cli, dir) in &dirs {
            assert!(holds_agent_cli(dir), "{cli}");
        }
        assert!(!holds_agent_cli(&not_runnable));
        for (_, dir) in dirs {
            let _ = std::fs::remove_dir_all(dir);
        }
        let _ = std::fs::remove_dir_all(not_runnable);
    }

    #[test]
    fn a_missing_or_empty_path_leaves_only_what_was_prepended() {
        let stubs = folder("only-stubs", &[("claude", true)]);
        assert_eq!(test_agent_path(std::slice::from_ref(&stubs), None), joined(&[&stubs]));
        assert_eq!(test_agent_path(std::slice::from_ref(&stubs), Some(OsString::new())), joined(&[&stubs]));
        assert!(test_agent_path(&[], None).is_empty());
        let _ = std::fs::remove_dir_all(stubs);
    }

    #[test]
    fn on_windows_a_launcher_or_script_counts_as_the_cli() {
        let names = executable_names("claude", true);
        for expected in ["claude", "claude.exe", "claude.cmd", "claude.bat", "claude.ps1"] {
            assert!(names.iter().any(|name| name == expected), "{expected}");
        }
        assert_eq!(executable_names("claude", false), vec!["claude".to_string()]);
    }
}
