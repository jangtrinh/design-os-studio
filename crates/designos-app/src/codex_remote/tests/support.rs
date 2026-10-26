//! Helpers the codex_remote tests share: scratch folders, stub executables and bounded waiting.

use crate::codex_remote::shell_env::TabEnv;
use portable_pty::CommandBuilder;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// A scratch folder, removed with everything in it when the test drops it.
pub struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for Scratch {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<std::ffi::OsStr> for Scratch {
    fn as_ref(&self) -> &std::ffi::OsStr {
        self.0.as_os_str()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A fresh, empty folder with a short name. A unix socket path may not exceed 103 bytes and macOS puts the temp
/// folder at about 49 of them, so the tag stays short.
pub fn scratch_dir(tag: &str) -> Scratch {
    Scratch(fresh_dir(&std::env::temp_dir(), tag))
}

/// Like [`scratch_dir`], but under `/tmp` (12 bytes once macOS resolves it): for the tests that bind a socket whose
/// file name is Codex's 64-character hash.
pub fn short_scratch_dir(tag: &str) -> Scratch {
    Scratch(fresh_dir(Path::new("/tmp"), tag).canonicalize().expect("canonical /tmp folder"))
}

fn fresh_dir(parent: &Path, tag: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = parent.join(format!("cxr-{}-{tag}{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch folder");
    dir
}

/// An executable file `dir/name` with exactly `contents`.
pub fn write_executable(dir: &Path, name: &str, contents: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join(name);
    std::fs::write(&path, contents).expect("executable");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    // A process another test forks while the file is open for writing keeps that descriptor until it has exec'd, and
    // running the file in that window fails with "text file busy". Let any such child get past its exec first.
    std::thread::sleep(Duration::from_millis(40));
    path
}

/// An executable `#!/bin/sh` script `dir/name` with `body`. It has been run once (with `CXTEST_WARM` set, when it exits
/// at once) before this returns: the first run of a file that was just written is slow (macOS checks it), seconds on a
/// loaded machine, and no test should be timed across it.
pub fn write_script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = write_executable(dir, name, &format!("#!/bin/sh\n[ -z \"$CXTEST_WARM\" ] || exit 0\n{body}\n"));
    let quiet = || std::process::Stdio::null();
    let _ = std::process::Command::new(&path).env_clear().env("CXTEST_WARM", "1").stdin(quiet()).stdout(quiet()).stderr(quiet()).status();
    path
}

/// A tab environment: a copy of this process's, in memory only (never printed).
pub fn tab_env() -> TabEnv {
    TabEnv::from_builder(&CommandBuilder::new("true"))
}

/// [`tab_env`] with `name` set to `value`.
pub fn tab_env_with(name: &str, value: &str) -> TabEnv {
    let mut builder = CommandBuilder::new("true");
    builder.env(name, value);
    TabEnv::from_builder(&builder)
}

/// A tab builder with made-up values; no HOME, so a leak of Studio's own environment shows.
pub fn shell_builder() -> CommandBuilder {
    let mut builder = CommandBuilder::new("true");
    builder.env_remove("HOME");
    for (name, value) in
        [("PATH", "/usr/bin:/bin"), ("CXTEST_PLAIN", "plain-fake-1"), ("DESIGNOS_FILE", "/x/file.designos"), ("DESIGNOS_AGENT_RUNTIME", "codex")]
    {
        builder.env(name, value);
    }
    builder
}

/// A login shell that does `pre` (what an rc file might) and then runs the script it was given, like `$SHELL -l -i -c`.
pub fn stub_shell(dir: &Path, pre: &str, after: &str) -> PathBuf {
    write_script(dir, "shell", &format!("{pre}\n/bin/sh -c \"$4\"\nstatus=$?\n{after}\nexit $status"))
}

/// The value of `name` as text, for assertions that never print it.
pub fn env_value(env: &TabEnv, name: &str) -> Option<String> {
    env.get(name).map(|value| value.to_string_lossy().into_owned())
}

/// Ids the recorded fixtures use for their one thread and its two turns.
pub const THREAD: &str = "11111111-1111-4111-8111-111111111111";
pub const TURN1: &str = "22222222-2222-4222-8222-222222222222";
pub const TURN2: &str = "33333333-3333-4333-8333-333333333333";

/// A scrubbed frame recorded from Codex 0.164 (or built from its schema, when the name says `synthetic`).
pub fn fixture(name: &str) -> serde_json::Value {
    let path = format!("{}/src/codex_remote/fixtures/0.164/{name}.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).expect("a JSON fixture")
}

/// `frame` with the value at JSON pointer `at` replaced, or added when the parent object lacks the key.
pub fn with(mut frame: serde_json::Value, at: &str, new: serde_json::Value) -> serde_json::Value {
    if let Some(slot) = frame.pointer_mut(at) {
        *slot = new;
        return frame;
    }
    let (parent, key) = at.rsplit_once('/').expect("a pointer with a key");
    match frame.pointer_mut(parent).and_then(serde_json::Value::as_object_mut) {
        Some(map) => map.insert(key.to_string(), new),
        None => panic!("no {parent} object in the frame"),
    };
    frame
}

/// `frame` without the key at JSON pointer `at`.
pub fn without(mut frame: serde_json::Value, at: &str) -> serde_json::Value {
    let (parent, key) = at.rsplit_once('/').expect("a pointer with a key");
    frame.pointer_mut(parent).and_then(serde_json::Value::as_object_mut).and_then(|map| map.remove(key)).unwrap_or_else(|| panic!("no {at}"));
    frame
}

/// Polls `done` every 10 ms until it holds or `timeout` passes; the result says which.
pub fn wait_until(timeout: Duration, mut done: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    done()
}

#[test]
fn a_scratch_folder_is_removed_when_it_is_dropped() {
    let dir = scratch_dir("zz");
    let path = dir.to_path_buf();
    std::fs::write(path.join("file"), "x").unwrap();
    assert!(path.exists());
    drop(dir);
    assert!(!path.exists(), "the test's folder must not outlive the test");
}
