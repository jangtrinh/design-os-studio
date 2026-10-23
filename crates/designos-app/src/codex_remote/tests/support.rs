//! Helpers the codex_remote tests share: scratch folders, stub executables and bounded waiting.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// A fresh, empty folder with a short name. A unix socket path may not exceed 103 bytes and macOS puts the temp
/// folder at about 49 of them, so the tag stays short.
pub fn scratch_dir(tag: &str) -> PathBuf {
    fresh_dir(&std::env::temp_dir(), tag)
}

/// Like [`scratch_dir`], but under `/tmp` (12 bytes once macOS resolves it): for the tests that bind a socket whose
/// file name is Codex's 64-character hash.
pub fn short_scratch_dir(tag: &str) -> PathBuf {
    fresh_dir(Path::new("/tmp"), tag).canonicalize().expect("canonical /tmp folder")
}

fn fresh_dir(parent: &Path, tag: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = parent.join(format!("cxr-{}-{tag}{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch folder");
    dir
}

/// An executable `#!/bin/sh` script `dir/name` with `body`.
pub fn write_script(dir: &Path, name: &str, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("script");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    path
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
