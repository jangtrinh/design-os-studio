//! A stand-in for the user's home folder, for tests that start a real agent CLI.
//!
//! Every CLI keeps state under the user's home: Gemini writes its project registry on each start (even for `--version`) and
//! refreshes its login there, Codex and Claude have a home folder of their own, and the XDG folders hold the rest. A test that
//! runs one of them must not do that to the real home, so it gives the child process these folders instead.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Every environment variable that moves where an agent CLI keeps its state, paired with the folder under the scratch home it is
/// pointed at (`""` is the scratch home itself). `HOME` covers the CLIs that name no variable of their own.
pub const HOME_VARS: &[(&str, &str)] = &[
    ("HOME", ""),
    ("GEMINI_CLI_HOME", ""),
    ("CODEX_HOME", ".codex"),
    ("CLAUDE_CONFIG_DIR", ".claude"),
    ("XDG_CONFIG_HOME", ".config"),
    ("XDG_DATA_HOME", ".local/share"),
    ("XDG_CACHE_HOME", ".cache"),
    ("XDG_STATE_HOME", ".local/state"),
];

/// Name of a scratch home folder, before the pid of the test process that made it. The older prefix is the Gemini-only home of an
/// earlier build.
const HOME_PREFIX: &str = "designos-scratch-cli-home-";
const OLD_HOME_PREFIX: &str = "designos-scratch-gemini-home-";

/// Removes the scratch homes in `dir` whose test process is gone: a test run leaves its home behind, so each run clears the
/// ones before it. Never the home of a live process, never this process's own, and never anything that is not a plain folder
/// (a link is left alone, and so is what it points at).
pub fn remove_dead_homes(dir: &Path) {
    // Off Unix `is_pid_alive` cannot tell, and it would call a live run's home dead.
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    if !cfg!(unix) {
        return;
    }
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(pid) = [HOME_PREFIX, OLD_HOME_PREFIX].iter().find_map(|prefix| name.strip_prefix(prefix)).and_then(|rest| rest.parse::<u32>().ok()) else {
            continue;
        };
        let plain_folder = entry.path().symlink_metadata().is_ok_and(|meta| meta.file_type().is_dir());
        if plain_folder && pid != std::process::id() && !crate::agent_sessions::is_pid_alive(pid) {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// The scratch home of this test process, made on first use. It holds the folders [`HOME_VARS`] name, and the one setting
/// `gemini mcp list` needs before it lists servers: the auth type. No key or login is stored.
pub fn scratch_cli_home() -> PathBuf {
    static HOME: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    HOME.get_or_init(|| {
        remove_dead_homes(&std::env::temp_dir());
        let home = std::env::temp_dir().join(format!("{HOME_PREFIX}{}", std::process::id()));
        for (_, folder) in HOME_VARS {
            let _ = std::fs::create_dir_all(home.join(folder));
        }
        let _ = std::fs::create_dir_all(home.join(".gemini"));
        let _ = std::fs::write(home.join(".gemini").join("settings.json"), r#"{"security":{"auth":{"selectedType":"gemini-api-key"}}}"#);
        home
    })
    .clone()
}

/// Points `cmd` (a real agent CLI run by a test) at [`scratch_cli_home`] through every variable in [`HOME_VARS`].
pub fn isolate_cli_homes(cmd: &mut Command) {
    let home = scratch_cli_home();
    for (name, folder) in HOME_VARS {
        cmd.env(name, home.join(folder));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::AgentCliPreset;
    use std::path::Path;

    /// Every preset; the match fails to compile when a preset is added, so a new CLI cannot slip past the check below.
    fn every_preset() -> Vec<AgentCliPreset> {
        use AgentCliPreset::*;
        let all = [Shell, Claude, Codex, Antigravity, Grok, Gemini, OpenCode, DesignOsHelp];
        for preset in all {
            match preset {
                Shell | Claude | Codex | Antigravity | Grok | Gemini | OpenCode | DesignOsHelp => {}
            }
        }
        all.to_vec()
    }

    /// A child process that has already ended: its pid belongs to nobody.
    fn dead_pid() -> u32 {
        let mut child = Command::new("true").spawn().expect("spawn true");
        let pid = child.id();
        child.wait().expect("wait");
        pid
    }

    #[cfg(unix)]
    #[test]
    fn a_dead_process_scratch_home_is_removed_and_nothing_else_is() {
        let base = std::env::temp_dir().join(format!("designos-scratch-sweep-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let mut live = Command::new("sleep").arg("30").spawn().expect("spawn sleep");
        let (dead, dead_old, dead_link) = (dead_pid(), dead_pid(), dead_pid());
        let own = std::process::id();
        let mk = |name: String| {
            let dir = base.join(name);
            std::fs::create_dir_all(dir.join(".gemini")).unwrap();
            dir
        };
        let gone = [mk(format!("{HOME_PREFIX}{dead}")), mk(format!("{OLD_HOME_PREFIX}{dead_old}"))];
        let kept = [mk(format!("{HOME_PREFIX}{}", live.id())), mk(format!("{HOME_PREFIX}{own}")), mk("designos-other-1".to_string())];
        let target = mk("link-target".to_string());
        std::os::unix::fs::symlink(&target, base.join(format!("{HOME_PREFIX}{dead_link}"))).unwrap();
        std::fs::write(base.join(format!("{HOME_PREFIX}notapid")), b"x").unwrap();

        remove_dead_homes(&base);

        for dir in &gone {
            assert!(!dir.exists(), "the home of a dead process is still there: {}", dir.display());
        }
        for dir in kept.iter().chain([&target]) {
            assert!(dir.exists(), "a folder that is not a dead process's home was removed: {}", dir.display());
        }
        assert!(base.join(format!("{HOME_PREFIX}{dead_link}")).symlink_metadata().is_ok(), "a link was removed");
        assert!(target.join(".gemini").exists(), "what a link points at was removed");
        assert!(base.join(format!("{HOME_PREFIX}notapid")).is_file());
        let _ = live.kill();
        let _ = live.wait();
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_test_never_runs_a_cli_version_probe_against_the_users_home() {
        let scratch = scratch_cli_home();
        let mut wrong = Vec::new();
        for preset in every_preset() {
            let cmd = crate::agent_session_failure::version_probe_command(Path::new(preset.binary_name()), preset);
            for &(name, _) in HOME_VARS {
                let value = cmd.get_envs().find(|(key, _)| key.to_str() == Some(name)).and_then(|(_, value)| value).map(PathBuf::from);
                if !value.as_ref().is_some_and(|folder| folder.starts_with(&scratch)) {
                    wrong.push(format!("{preset:?}: {name} = {value:?}"));
                }
            }
            assert!(cmd.get_args().eq(["--version"]), "{preset:?} probe is not a plain --version");
        }
        assert!(wrong.is_empty(), "these variables do not point inside the scratch home {}:\n{}", scratch.display(), wrong.join("\n"));
        assert!(std::env::var_os("HOME").is_none_or(|real| !scratch.starts_with(real)), "the scratch home sits inside the real one");
        assert!(scratch.join(".gemini").join("settings.json").is_file());
    }
}
