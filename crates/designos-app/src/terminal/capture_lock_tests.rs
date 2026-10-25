//! The transcript capture of Claude's hooks is a read-modify-write of one file that every Claude tab of a session shares, taken under a
//! lock folder. A capture that does not hold the lock must neither write the file nor remove the lock, or it loses another tab's
//! entry. The interleavings here are made exact with two stand-ins for `mkdir` and `sed` first on the `PATH` of the hook: the first
//! `mkdir` of the lock fails because the lock is held and is then released at once (the holder finished between the failure and the
//! next look), and every `sed` of the read-modify-write notes whether the lock is there and, when it is not, lets another tab take it.

use crate::agent_usage::claude_transcript_capture_command;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// What a capture of one tab runs in: the capture file and its lock, and the folders of the stand-ins and of what they note.
struct Rig {
    dir: PathBuf,
    capture: PathBuf,
    lock: PathBuf,
    shims: PathBuf,
    state: PathBuf,
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn rig(tag: &str) -> Rig {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("designos-capture-lock-{tag}-{}-{id}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (shims, state) = (dir.join("shims"), dir.join("state"));
    std::fs::create_dir_all(&shims).expect("the stand-ins' folder");
    std::fs::create_dir_all(&state).expect("the notes' folder");
    let capture = dir.join("claude-transcript.json");
    let lock = dir.join("claude-transcript.json.lock");
    Rig { dir, capture, lock, shims, state }
}

/// A stand-in for `mkdir`: the first one asked for the lock fails as if it were held, and the holder lets go before the next look.
const MKDIR_STAND_IN: &str = r#"#!/bin/sh
rest="${PATH#*:}"
if [ "$1" = "$FAKE_LOCK" ] && [ ! -e "$FAKE_STATE/released" ]; then
  : > "$FAKE_STATE/released"
  PATH="$rest"
  rmdir "$FAKE_LOCK"
  echo "mkdir: cannot create directory: File exists" >&2
  exit 1
fi
PATH="$rest"
exec mkdir "$@"
"#;

/// A stand-in for `sed`: after that release, it notes whether the lock is there, and while it is not, another tab takes it.
const SED_STAND_IN: &str = r#"#!/bin/sh
rest="${PATH#*:}"
if [ -e "$FAKE_STATE/released" ]; then
  if [ -d "$FAKE_LOCK" ]; then
    echo held >> "$FAKE_STATE/sed.log"
  else
    echo unheld >> "$FAKE_STATE/sed.log"
    if [ ! -e "$FAKE_STATE/other_tab_took_it" ]; then
      PATH="$rest"
      if mkdir "$FAKE_LOCK" 2>/dev/null; then : > "$FAKE_STATE/other_tab_took_it"; fi
    fi
  fi
fi
PATH="$rest"
exec sed "$@"
"#;

/// A stand-in for `mkdir` with a third tab: the holder (tab A) lets go between the first failed `mkdir` and the capture's next look; a third
/// tab (C) takes the lock before the capture's retry; and when the capture asks again, C has finished (its entry is in the file and its
/// lock is gone) and the capture's own `mkdir` goes through.
const MKDIR_THIRD_TAB_STAND_IN: &str = r#"#!/bin/sh
rest="${PATH#*:}"
if [ "$1" = "$FAKE_LOCK" ]; then
  n=$(cat "$FAKE_STATE/count" 2>/dev/null || echo 0)
  n=$((n+1))
  echo "$n" > "$FAKE_STATE/count"
  PATH="$rest"
  case "$n" in
    1)
      rmdir "$FAKE_LOCK"
      echo "mkdir: cannot create directory: File exists" >&2
      exit 1
      ;;
    2)
      mkdir "$FAKE_LOCK"
      : > "$FAKE_STATE/third_tab_took_it"
      echo "mkdir: cannot create directory: File exists" >&2
      exit 1
      ;;
    3)
      printf '%s\n' '{"session_id":"sid-c","transcript_path":"/t/c.jsonl","sessions":{"tab-a":"/t/a.jsonl","tab-c":"/t/c.jsonl"}}' > "$FAKE_CAPTURE"
      rmdir "$FAKE_LOCK"
      : > "$FAKE_STATE/third_tab_let_go"
      ;;
  esac
fi
PATH="$rest"
exec mkdir "$@"
"#;

fn install_stand_in(rig: &Rig, name: &str, body: &str) {
    let path = rig.shims.join(name);
    std::fs::write(&path, body).expect("a stand-in");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("executable");
}

fn install_stand_ins(rig: &Rig) {
    install_stand_in(rig, "mkdir", MKDIR_STAND_IN);
    install_stand_in(rig, "sed", SED_STAND_IN);
}

/// Runs the capture of the tab `key` (the tab session id Claude's hooks export) for the Claude session `sid`.
fn capture(rig: &Rig, key: &str, sid: &str, transcript: &str, stand_ins: bool) -> Output {
    use std::io::Write;
    let path =
        if stand_ins { format!("{}:{}", rig.shims.display(), std::env::var("PATH").unwrap_or_default()) } else { std::env::var("PATH").unwrap_or_default() };
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(claude_transcript_capture_command(&rig.capture))
        .current_dir(&rig.dir)
        .env("PATH", path)
        .env("DESIGNOS_TAB_SESSION_UUID", key)
        .env("FAKE_LOCK", &rig.lock)
        .env("FAKE_STATE", &rig.state)
        .env("FAKE_CAPTURE", &rig.capture)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("sh starts");
    let payload = format!(r#"{{"session_id":"{sid}","transcript_path":"{transcript}"}}"#);
    let _ = child.stdin.take().expect("stdin").write_all(payload.as_bytes());
    child.wait_with_output().expect("the capture ends")
}

/// What tab A had captured before: its entry in the shared file.
fn tab_a_captured(rig: &Rig) -> String {
    let text = "{\"session_id\":\"sid-a\",\"transcript_path\":\"/t/a.jsonl\",\"sessions\":{\"tab-a\":\"/t/a.jsonl\"}}\n".to_string();
    std::fs::write(&rig.capture, &text).expect("the capture file of tab A");
    text
}

fn sessions_of(path: &Path) -> std::collections::BTreeMap<String, String> {
    let text = std::fs::read_to_string(path).expect("the capture file");
    let value: serde_json::Value = serde_json::from_str(text.trim()).unwrap_or_else(|e| panic!("{e}: {text}"));
    value["sessions"].as_object().expect("sessions").iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string())).collect()
}

fn lines_of(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path).unwrap_or_default().lines().map(str::to_string).collect()
}

#[test]
fn a_lock_released_between_a_failed_mkdir_and_the_next_look_is_taken_not_skipped() {
    let rig = rig("released");
    install_stand_ins(&rig);
    tab_a_captured(&rig);
    std::fs::create_dir(&rig.lock).expect("tab A holds the lock");

    let output = capture(&rig, "tab-b", "sid-b", "/t/b.jsonl", true);

    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(rig.state.join("released").exists(), "the stand-in made the interleaving happen");
    let noted = lines_of(&rig.state.join("sed.log"));
    assert!(!noted.is_empty(), "the read-modify-write ran");
    assert!(noted.iter().all(|line| line == "held"), "the read-modify-write ran without the lock: {noted:?}");
    assert!(!rig.state.join("other_tab_took_it").exists() || rig.lock.exists(), "a capture removed a lock another tab had taken, and it was not the holder");
    assert_eq!(
        sessions_of(&rig.capture),
        [("tab-a".to_string(), "/t/a.jsonl".to_string()), ("tab-b".to_string(), "/t/b.jsonl".to_string())].into(),
        "both tabs keep their transcript"
    );
    assert!(!rig.lock.exists(), "the capture gave back the lock it took");
}

#[test]
fn a_lock_a_third_tab_takes_in_the_window_is_waited_for_and_the_entry_is_still_written_after_it() {
    let rig = rig("third");
    install_stand_in(&rig, "mkdir", MKDIR_THIRD_TAB_STAND_IN);
    tab_a_captured(&rig);
    std::fs::create_dir(&rig.lock).expect("tab A holds the lock");

    let output = capture(&rig, "tab-b", "sid-b", "/t/b.jsonl", true);

    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(rig.state.join("third_tab_took_it").exists(), "the stand-in made tab C take the lock before the retry of the capture");
    assert!(rig.state.join("third_tab_let_go").exists(), "the capture gave up before tab C finished: it never asked a third time");
    assert_eq!(
        sessions_of(&rig.capture),
        [("tab-a".to_string(), "/t/a.jsonl".to_string()), ("tab-b".to_string(), "/t/b.jsonl".to_string()), ("tab-c".to_string(), "/t/c.jsonl".to_string()),]
            .into(),
        "the capture lost the race to a third tab, waited, and still wrote its entry next to the entries of both"
    );
    assert!(!rig.lock.exists(), "the capture gave back the lock it took");
}

#[test]
fn a_lock_that_stays_held_is_never_removed_by_a_capture_that_gave_up_waiting() {
    let rig = rig("stale");
    let before = tab_a_captured(&rig);
    std::fs::create_dir(&rig.lock).expect("a lock nobody lets go of, left by a hook that was killed");

    let output = capture(&rig, "tab-b", "sid-b", "/t/b.jsonl", false);

    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(rig.lock.exists(), "a capture that did not take the lock removed it");
    assert_eq!(std::fs::read_to_string(&rig.capture).expect("the capture file"), before, "a capture that did not hold the lock wrote the file");
}

#[test]
fn without_a_lock_in_the_way_a_capture_adds_its_tab_and_gives_the_lock_back() {
    let rig = rig("free");
    tab_a_captured(&rig);

    let output = capture(&rig, "tab-b", "sid-b", "/t/b.jsonl", false);

    assert!(output.status.success(), "{output:?}");
    assert_eq!(sessions_of(&rig.capture).len(), 2, "{}", std::fs::read_to_string(&rig.capture).unwrap_or_default());
    assert!(!rig.lock.exists());
}
