//! The wrapper that starts the remote TUI, and falls back to today's launch when the socket never shows up.

use super::support::scratch_dir;
use crate::codex_remote::launch::{tui_launch_command, tui_launch_command_with_ticks, ENV_CODEX_BIN, ENV_CODEX_SOCKET};
use crate::terminal::{AgentCliPreset, PermissionMode};
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::process::Command;

const GOLDEN: &str = r#"exec /bin/sh -c 'i=0; while [ ! -S "$DESIGNOS_CODEX_SOCKET" ] && [ "$i" -lt 100 ]; do sleep 0.05; i=$((i+1)); done; [ -S "$DESIGNOS_CODEX_SOCKET" ] && exec "$DESIGNOS_CODEX_BIN" --remote "unix://$DESIGNOS_CODEX_SOCKET"; exec "$@"' designos-codex-tui codex -c 'x=1'"#;

fn ask_launch() -> &'static str {
    AgentCliPreset::Codex.cli_launch_command_for_mode(PermissionMode::Ask)
}

fn run(command: &str, env: &[(&str, &str)]) -> std::process::Output {
    Command::new("/bin/sh").arg("-c").arg(command).env_clear().env("PATH", "/usr/bin:/bin").envs(env.iter().copied()).output().expect("sh")
}

#[test]
fn the_wrapper_matches_its_golden_and_waits_five_seconds_in_production() {
    assert_eq!(tui_launch_command("codex -c 'x=1'"), GOLDEN);
    assert_eq!(tui_launch_command_with_ticks("codex", 4), GOLDEN.replace("-lt 100", "-lt 4").replace("codex -c 'x=1'", "codex"));
    assert_eq!(ENV_CODEX_BIN, "DESIGNOS_CODEX_BIN");
    assert_eq!(ENV_CODEX_SOCKET, "DESIGNOS_CODEX_SOCKET");
}

#[test]
fn the_remote_branch_names_no_directory_mode_or_override_and_the_fallback_is_todays_command() {
    let command = tui_launch_command(ask_launch());
    assert!(command.starts_with("exec "), "the shell must not add a second exec in front of the wrapper");
    let (script, fallback) = command.split_once(" designos-codex-tui ").expect("the wrapper names its fallback");
    assert_eq!(fallback, ask_launch(), "the fallback words are today's Ask command, untouched");
    let inner = script.split_once("-c '").expect("the wrapper is `sh -c '...'`").1.trim_end_matches('\'');
    for token in inner.split_whitespace() {
        assert!(!["--cd", "-a", "-s", "-c", "--sandbox", "--ask-for-approval", "--add-dir"].contains(&token), "{token} must not be in the remote branch");
    }
    assert!(inner.contains("--remote") && inner.contains("exec \"$@\""));
    // fish parses the wrapper too: nothing in the quoted script may need an escape, and none of it can end the quote.
    assert!(!inner.contains('\\') && !inner.contains('\''), "the quoted script has no backslash and no single quote: {inner}");
}

#[test]
fn a_socket_that_is_there_starts_the_remote_tui_on_it() {
    let dir = scratch_dir("lr");
    let socket = dir.join("s.sock");
    let _listener = UnixListener::bind(&socket).expect("socket");
    let out = dir.join("remote.txt");
    let marker = dir.join("fallback.txt");
    let tui = super::support::write_script(&dir, "tui", &format!("printf '%s\\n' \"$@\" > {}", out.display()));
    let fallback = super::support::write_script(&dir, "fallback", &format!("echo ran > {}", marker.display()));
    let command = tui_launch_command_with_ticks(&fallback.display().to_string(), 4);
    let result = run(&command, &[(ENV_CODEX_BIN, tui.to_str().unwrap()), (ENV_CODEX_SOCKET, socket.to_str().unwrap())]);
    assert!(result.status.success(), "{result:?}");
    assert_eq!(std::fs::read_to_string(&out).unwrap(), format!("--remote\nunix://{}\n", socket.display()));
    assert!(!marker.exists(), "the fallback must not run when the socket is up");
}

#[test]
fn a_socket_that_never_appears_runs_todays_command_with_its_words_intact() {
    let dir = scratch_dir("lf");
    let words = dir.join("words.txt");
    let tui = super::support::write_script(&dir, "tui", &format!("echo remote > {}", dir.join("remote.txt").display()));
    let fallback = super::support::write_script(&dir, "fallback", &format!("printf '%s\\n' \"$@\" > {}", words.display()));
    let command = tui_launch_command_with_ticks(&format!("{} first 'second word' \"$DESIGNOS_WORKSPACE\"", fallback.display()), 4);
    let never = dir.join("never.sock");
    let env = [(ENV_CODEX_BIN, tui.to_str().unwrap()), (ENV_CODEX_SOCKET, never.to_str().unwrap()), ("DESIGNOS_WORKSPACE", "/some folder")];
    assert!(run(&command, &env).status.success());
    assert_eq!(std::fs::read_to_string(&words).unwrap(), "first\nsecond word\n/some folder\n");
    assert!(!dir.join("remote.txt").exists());
}

/// The words the fallback hands to `codex` are what the plain Ask launch hands it: the same argv, byte for byte.
#[test]
fn the_fallback_runs_the_ask_golden_byte_for_byte() {
    let dir = scratch_dir("lg");
    let argv = |name: &str| dir.join(name);
    let stub = |out: &Path| super::support::write_script(&dir, "codex", &format!("printf '%s\\n' \"$@\" > {}", out.display()));
    let env = |path: &str| {
        [
            ("PATH", path.to_string()),
            ("DESIGNOS_MCP_BIN", "/x/designos-mcp".into()),
            ("DESIGNOS_WORKSPACE", "/x/work space".into()),
            (ENV_CODEX_SOCKET, dir.join("none.sock").display().to_string()),
        ]
    };
    let path = format!("{}:/usr/bin:/bin", dir.display());
    stub(&argv("plain.txt"));
    let direct = Command::new("/bin/sh").arg("-c").arg(format!("exec {}", ask_launch())).env_clear().envs(env(&path)).output().unwrap();
    assert!(direct.status.success(), "{direct:?}");
    let plain = std::fs::read_to_string(argv("plain.txt")).unwrap();
    stub(&argv("wrapped.txt"));
    let wrapped = Command::new("/bin/sh").arg("-c").arg(tui_launch_command_with_ticks(ask_launch(), 4)).env_clear().envs(env(&path)).output().unwrap();
    assert!(wrapped.status.success(), "{wrapped:?}");
    assert_eq!(std::fs::read_to_string(argv("wrapped.txt")).unwrap(), plain);
    assert!(plain.contains("mcp_servers.designos.command=\"/x/designos-mcp\"") && plain.contains("/x/work space"), "{plain}");
}
