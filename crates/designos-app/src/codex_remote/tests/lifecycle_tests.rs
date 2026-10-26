//! The app-server process: its argv, its environment, and the watchdog that ends its whole process group.

use super::support::{scratch_dir, wait_until, write_script};
use crate::codex_remote::server_process::{start, ServerLaunch, MCP_ENV_VARS};
use crate::codex_remote::shell_capture::capture;
use crate::codex_remote::shell_env::TabEnv;
use crate::terminal::AgentCliPreset;
use portable_pty::CommandBuilder;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

/// (pid, state) of every process in group `pgid`, from `ps`.
fn group(pgid: u32) -> Vec<(u32, String)> {
    let out = Command::new("ps").args(["-axo", "pid=,pgid=,stat="]).output().expect("ps");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let mut cols = line.split_whitespace();
            let (pid, group, state) = (cols.next()?.parse::<u32>().ok()?, cols.next()?.parse::<u32>().ok()?, cols.next()?.to_string());
            (group == pgid).then_some((pid, state))
        })
        .collect()
}

fn alive(pgid: u32) -> Vec<u32> {
    group(pgid).into_iter().filter(|(_, state)| !state.starts_with('Z')).map(|(pid, _)| pid).collect()
}

fn launch_for(dir: &Path, stub: &Path, workspace: &str) -> ServerLaunch {
    let mut builder = CommandBuilder::new("true");
    builder.env("DESIGNOS_MCP_BIN", "/x/designos-mcp");
    builder.env("DESIGNOS_WORKSPACE", workspace);
    builder.env("DESIGNOS_FILE", "/x/file.designos");
    builder.cwd(dir);
    ServerLaunch::from_command_builder(&builder, &TabEnv::from_builder(&builder), stub, &dir.join("cx-1.sock"), &dir.join("cx-1.log")).expect("a launch")
}

#[test]
fn the_server_argv_is_the_listen_flag_and_the_designos_overrides_and_nothing_else() {
    let dir = scratch_dir("sa");
    let launch = launch_for(&dir, Path::new("/opt/bin/codex"), "/x/with \"quote\" and \\ slash");
    let argv = launch.server_args().expect("utf-8 paths");
    let names = MCP_ENV_VARS.iter().map(|n| format!("\"{n}\"")).collect::<Vec<_>>().join(",");
    assert_eq!(
        argv,
        vec![
            "/opt/bin/codex".to_string(),
            "app-server".into(),
            "--listen".into(),
            format!("unix://{}", dir.join("cx-1.sock").display()),
            "-c".into(),
            "mcp_servers.designos.command=\"/x/designos-mcp\"".into(),
            "-c".into(),
            "mcp_servers.designos.args=[\"--workspace\",\"/x/with \\\"quote\\\" and \\\\ slash\"]".into(),
            "-c".into(),
            format!("mcp_servers.designos.env_vars=[{names}]"),
        ]
    );
    let joined = argv.join(" ");
    for forbidden in ["approval", "sandbox", "projects", "analytics", "trust", "--cd", "effort"] {
        assert!(!joined.contains(forbidden), "{forbidden} must not be passed: {joined}");
    }
}

#[test]
fn the_forwarded_env_names_are_the_ones_todays_codex_launch_forwards() {
    let launch = AgentCliPreset::Codex.cli_launch_command();
    let start = launch.find("mcp_servers.designos.env_vars=[").expect("today's launch forwards env vars") + "mcp_servers.designos.env_vars=[".len();
    let list = &launch[start..start + launch[start..].find(']').unwrap()];
    let today: Vec<&str> = list.split(',').map(|name| name.trim().trim_matches(|c| c == '"' || c == '\'')).collect();
    assert_eq!(today, MCP_ENV_VARS.to_vec(), "the two launches must forward the same variables");
    assert!(launch.contains("mcp_servers.designos.command=\"") && launch.contains("mcp_servers.designos.args=[\"--workspace\""), "{launch}");
}

#[test]
fn a_launch_needs_the_mcp_binary_the_workspace_and_a_working_folder() {
    let dir = scratch_dir("sb");
    let try_launch = |builder: &CommandBuilder| {
        ServerLaunch::from_command_builder(builder, &TabEnv::from_builder(builder), Path::new("/b/codex"), &dir.join("r.sock"), &dir.join("r.log")).is_err()
    };
    let mut full = CommandBuilder::new("true");
    full.env("DESIGNOS_MCP_BIN", "/m");
    full.env("DESIGNOS_WORKSPACE", "/w");
    assert!(try_launch(&full), "no working folder");
    full.cwd(&dir);
    assert!(!try_launch(&full));
    full.env_remove("DESIGNOS_MCP_BIN");
    assert!(try_launch(&full), "no MCP binary");
    full.env("DESIGNOS_MCP_BIN", "/m");
    full.env_remove("DESIGNOS_WORKSPACE");
    assert!(try_launch(&full), "no workspace");
}

#[test]
fn the_server_gets_the_tabs_login_shell_environment_and_folder_and_logs_to_a_private_file() {
    let dir = scratch_dir("sc");
    let seen = dir.join("seen.txt");
    let body = format!(
        "{{ pwd; echo \"args: $*\"; echo \"FILE=$DESIGNOS_FILE\"; echo \"WORKSPACE=$DESIGNOS_WORKSPACE\"; echo \"RC=$CXTEST_FROM_RC\"; \
         if [ -z \"${{HOME+x}}\" ]; then echo HOME_UNSET; fi; echo from-stderr >&2; }} > {} 2>&1; exec sleep 30",
        seen.display()
    );
    let stub = write_script(&dir, "codex", &body);
    let mut builder = CommandBuilder::new("true");
    for (name, value) in
        [("DESIGNOS_MCP_BIN", "/x/designos-mcp"), ("DESIGNOS_WORKSPACE", "/x/ws"), ("DESIGNOS_FILE", "/x/file.designos"), ("PATH", "/usr/bin:/bin")]
    {
        builder.env(name, value);
    }
    builder.env_remove("HOME");
    builder.cwd(&dir);
    let login_shell = write_script(&dir, "shell", "export CXTEST_FROM_RC=fake-rc-value\n/bin/sh -c \"$4\"");
    let env = capture(&builder, &login_shell, Duration::from_secs(10)).expect("a captured environment");
    let launch = ServerLaunch::from_command_builder(&builder, &env, &stub, &dir.join("cx-1.sock"), &dir.join("cx-1.log")).unwrap();
    let handle = start(&launch).expect("start");
    assert!(wait_until(Duration::from_secs(5), || std::fs::read_to_string(&seen).is_ok_and(|text| text.contains("from-stderr"))));
    drop(handle);
    let text = std::fs::read_to_string(&seen).unwrap();
    assert!(text.starts_with(&format!("{}\n", dir.canonicalize().unwrap().display())), "the server runs in the tab's working folder");
    assert!(text.contains("RC=fake-rc-value\n"), "the server sees what the login shell exported");
    assert!(text.contains("FILE=/x/file.designos\n") && text.contains("WORKSPACE=/x/ws\n"), "and Studio's own values");
    assert!(text.contains("HOME_UNSET\n"), "and nothing of Studio's own process environment");
    assert!(text.contains("args: app-server --listen unix://"));
    let mode = std::fs::metadata(dir.join("cx-1.log")).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "the server log is owner-only");
}

#[test]
fn closing_the_pipe_ends_the_whole_group_within_three_seconds_and_reaps_the_watchdog() {
    let dir = scratch_dir("sd");
    let stub = write_script(&dir, "codex", "sleep 30 & exec sleep 30");
    let handle = start(&launch_for(&dir, &stub, "/x/ws")).expect("start");
    let pgid = handle.pid();
    assert!(wait_until(Duration::from_secs(5), || alive(pgid).len() >= 3), "watchdog, server and its child are up: {:?}", group(pgid));
    let closed = Instant::now();
    drop(handle);
    assert!(wait_until(Duration::from_secs(3), || alive(pgid).is_empty()), "members left after 3 s: {:?}", group(pgid));
    assert!(closed.elapsed() < Duration::from_secs(3));
    assert!(wait_until(Duration::from_secs(2), || group(pgid).is_empty()), "a defunct watchdog was left: {:?}", group(pgid));
}

#[test]
fn a_server_that_ignores_term_is_killed_after_the_grace_period() {
    let dir = scratch_dir("se");
    let stub = write_script(&dir, "codex", "trap '' TERM; exec sleep 30");
    let handle = start(&launch_for(&dir, &stub, "/x/ws")).expect("start");
    let pgid = handle.pid();
    assert!(wait_until(Duration::from_secs(5), || alive(pgid).len() >= 2));
    drop(handle);
    std::thread::sleep(Duration::from_millis(1000));
    assert!(alive(pgid).len() >= 2, "TERM alone must not have ended it: {:?}", group(pgid));
    assert!(wait_until(Duration::from_secs(4), || alive(pgid).is_empty()), "KILL must follow: {:?}", group(pgid));
}

#[test]
fn the_server_log_is_never_opened_through_a_link() {
    let dir = scratch_dir("sl");
    let stub = write_script(&dir, "codex", "exit 0");
    let target = dir.join("elsewhere.txt");
    std::fs::write(&target, "keep").unwrap();
    std::os::unix::fs::symlink(&target, dir.join("cx-1.log")).unwrap();
    assert!(start(&launch_for(&dir, &stub, "/x/ws")).is_err(), "a link in place of the log is refused");
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "keep", "and what it pointed to is untouched");
}
