//! `CodexRemote::start` end to end with a stub `codex`: the gate rows in order, the server group, and the link's lifetime.

use super::fake_server::{FakeServer, WAIT};
use super::harness::FAST;
use super::support::{fixture, scratch_dir, short_scratch_dir, tab_env, wait_until, with, write_script};
use crate::codex_remote::gate::{lookup_version, warm, VersionLookup};
use crate::codex_remote::protocol::Decision;
use crate::codex_remote::remote::{CodexRemote, StartRequest};
use crate::codex_remote::shell_env::TabEnv;
use crate::codex_remote::{CodexEvent, FallbackReason, Originator, UnavailableReason};
use portable_pty::CommandBuilder;
use serde_json::json;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

/// A `codex` that reports `version` and otherwise acts as a server that never listens.
fn stub_codex(dir: &Path, version: &str) -> PathBuf {
    let bin = write_script(dir, "codex", &format!("[ \"$1\" = --version ] && {{ echo 'codex-cli {version}'; exit 0; }}; exec sleep 30"));
    let env = tab_env();
    warm(&bin, &env);
    let mut last = VersionLookup::Pending;
    wait_until(Duration::from_secs(10), || {
        last = lookup_version(&bin, &env);
        matches!(last, VersionLookup::Known(Some(_)))
    });
    match last {
        VersionLookup::Known(Some(_)) => bin,
        VersionLookup::Known(None) => panic!("the stub's version probe failed"),
        VersionLookup::Pending => panic!("the stub's version probe did not finish in 10 s"),
    }
}

fn tab(dir: &Path) -> CommandBuilder {
    let mut builder = CommandBuilder::new("true");
    builder.env("DESIGNOS_MCP_BIN", "/x/designos-mcp");
    builder.env("DESIGNOS_WORKSPACE", "/x/ws");
    builder.cwd(dir);
    builder
}

fn start(bin: &Path, channel: Option<&Path>, pty: &CommandBuilder) -> Result<CodexRemote, FallbackReason> {
    CodexRemote::start(StartRequest {
        codex_bin: bin,
        channel_dir: channel,
        tab_id: 7,
        pty,
        env: &TabEnv::from_builder(pty),
        wake: Arc::new(|| {}),
        timing: FAST,
    })
}

/// Processes whose command line names `marker` (the watchdog, which carries the stub's path).
fn running(marker: &Path) -> usize {
    let out = Command::new("ps").args(["-axo", "command="]).output().expect("ps");
    String::from_utf8_lossy(&out.stdout).lines().filter(|line| line.contains(marker.to_str().unwrap())).count()
}

#[test]
fn a_started_remote_serves_the_tab_and_dropping_it_ends_the_server_and_the_client() {
    let (channel, work) = (short_scratch_dir("rs"), scratch_dir("rw"));
    let bin = stub_codex(&work, "0.164.3");
    let remote = start(&bin, Some(&channel), &tab(&work)).expect("every gate row holds");
    assert_eq!(remote.rendezvous, channel.join("cx-7.sock"));
    assert_eq!(std::fs::metadata(channel.join("cx-7.log")).unwrap().permissions().mode() & 0o777, 0o600, "the server log is private");
    assert!(running(&work) >= 1, "the server runs");
    let server = FakeServer::bind_at(&remote.rendezvous);
    let mut conn = server.accept();
    let init = conn.expect("initialize");
    conn.reply(&init, with(fixture("initialize-response"), "/result/userAgent", json!("codex-tui/0.164.3 (OS; arch)"))["result"].clone());
    assert_eq!(wait_event(&remote), CodexEvent::Originator(Originator::Tui));
    remote.link.set_ask_mode(false);
    remote.link.set_ask_mode(true);
    remote.link.answer("cx-1-0", Decision::Approve);
    drop(remote);
    assert!(conn.hung_up(), "the client closes its connection");
    assert!(wait_until(Duration::from_secs(4), || running(&work) == 0), "the server group is gone, watchdog included");
}

fn wait_event(remote: &CodexRemote) -> CodexEvent {
    let mut event = None;
    assert!(wait_until(WAIT, || {
        event = remote.link.next_event();
        event.is_some()
    }));
    event.unwrap()
}

#[test]
fn a_server_that_never_listens_ends_the_client_with_a_timeout() {
    let (channel, work) = (short_scratch_dir("rt"), scratch_dir("rv"));
    let bin = stub_codex(&work, "0.164.0");
    let remote = start(&bin, Some(&channel), &tab(&work)).expect("the card path starts");
    assert_eq!(wait_event(&remote), CodexEvent::Unavailable(UnavailableReason::SocketTimeout));
    drop(remote);
    assert!(wait_until(Duration::from_secs(4), || running(&work) == 0));
}

#[test]
fn each_failing_gate_row_leaves_the_tab_on_todays_path_and_starts_nothing() {
    let (channel, work) = (short_scratch_dir("rf"), scratch_dir("rg"));
    let bin = stub_codex(&work, "0.164.0");
    let pty = tab(&work);
    let socket = channel.join("cx-7.sock");
    let _live = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    let long = channel.join("a".repeat(90));
    std::fs::create_dir_all(&long).unwrap();
    let no_folder = {
        let mut builder = CommandBuilder::new("true");
        builder.env("DESIGNOS_MCP_BIN", "/x");
        builder.env("DESIGNOS_WORKSPACE", "/y");
        builder
    };
    assert_eq!(start(&work.join("absent"), Some(&channel), &pty).err(), Some(FallbackReason::VersionUnknown));
    let old = scratch_dir("ro");
    assert_eq!(start(&stub_codex(&old, "0.163.2"), Some(&channel), &pty).err(), Some(FallbackReason::VersionOutsideRange("0.163.2".into())));
    assert_eq!(start(&bin, None, &pty).err(), Some(FallbackReason::NoPrivateFolder));
    assert_eq!(start(&bin, Some(&long), &pty).err(), Some(FallbackReason::PathTooLong));
    assert_eq!(start(&bin, Some(&channel), &pty).err(), Some(FallbackReason::SocketInUse), "a live server on the path is not replaced");
    std::fs::remove_file(&socket).unwrap();
    assert!(matches!(start(&bin, Some(&channel), &no_folder).err(), Some(FallbackReason::SpawnFailed(why)) if why.contains("working folder")));
    assert_eq!((running(&work), running(&old)), (0, 0), "a tab that falls back starts no server");
}

/// Starts a remote, plays the join and one command request (run in the tab's folder) through a fake server, and returns the
/// card's key.
fn first_card_key(work: &Path, bin: &Path) -> String {
    let channel = short_scratch_dir("rk");
    let remote = start(bin, Some(&channel), &tab(work)).expect("the card path starts");
    let mut conn = FakeServer::bind_at(&remote.rendezvous).accept();
    let init = conn.expect("initialize");
    conn.reply(&init, fixture("initialize-response")["result"].clone());
    conn.expect("initialized");
    let list = conn.expect("thread/loaded/list");
    conn.reply(&list, json!({"data": [], "nextCursor": null}));
    conn.send(&fixture("thread-started"));
    conn.send(&fixture("thread-status-active"));
    let resume = conn.expect("thread/resume");
    conn.reply(&resume, json!({"thread": fixture("thread-read-response")["result"]["thread"].clone()}));
    // The request names the folder the tab runs in, which is the one Studio started the server in.
    conn.send(&with(fixture("command-approval-request"), "/params/cwd", json!(work.to_str().unwrap())));
    loop {
        if let CodexEvent::Card(card) = wait_event(&remote) {
            return card.key;
        }
    }
}

#[test]
fn request_ids_restart_with_every_server_but_card_keys_never_repeat() {
    let work = scratch_dir("rn");
    let bin = stub_codex(&work, "0.164.0");
    let generation =
        |key: &str| key.strip_prefix("cx-").and_then(|rest| rest.split('-').next()).and_then(|n| n.parse::<u64>().ok()).expect("cx-<generation>-<id>");
    let (first, second) = (first_card_key(&work, &bin), first_card_key(&work, &bin));
    assert!(first.ends_with("-1") && second.ends_with("-1"), "both servers used request id 1: {first} {second}");
    assert!(generation(&first) < generation(&second), "the generation counts up per server start");
}
