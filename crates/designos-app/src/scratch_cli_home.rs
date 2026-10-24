//! A stand-in for the user's home folder, for tests that start a real agent CLI.
//!
//! Every CLI keeps state under the user's home: Gemini writes its project registry on each start (even for `--version`) and
//! refreshes its login there, Codex and Claude have a home folder of their own, and the XDG folders hold the rest. A test that
//! runs one of them must not do that to the real home, so it gives the child process these folders instead.

use std::path::PathBuf;
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

/// Name of a scratch home folder, before the pid of the test process that made it. A run leaves its home behind (a few KB): nothing
/// here decides which of the others belongs to a finished run.
const HOME_PREFIX: &str = "designos-scratch-cli-home-";

/// The scratch home of this test process, made on first use. It holds the folders [`HOME_VARS`] name, and the one setting
/// `gemini mcp list` needs before it lists servers: the auth type. No key or login is stored.
pub fn scratch_cli_home() -> PathBuf {
    static HOME: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    HOME.get_or_init(|| {
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
