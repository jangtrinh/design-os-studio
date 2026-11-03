//! Watcher: external edits are reported within 1 s from a background thread; the caller never touches the disk.

use designos_docs::{ContentHash, DocChange, DocsStore, DocsWatcher, Expect, Page};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct Rig {
    _tmp: tempfile::TempDir,
    store: DocsStore,
    watcher: DocsWatcher,
    wake_threads: Arc<Mutex<Vec<std::thread::ThreadId>>>,
}

fn rig(seed: &[(&str, &str)]) -> Rig {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("docs");
    std::fs::create_dir_all(&dir).unwrap();
    for (name, text) in seed {
        let p = dir.join(name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    // Files written in the instant before `start` count as changes (see `DocsWatcher::start`), so let these settle.
    std::thread::sleep(Duration::from_millis(40));
    let wake_threads = Arc::new(Mutex::new(Vec::new()));
    let sink = wake_threads.clone();
    let watcher = DocsWatcher::start(dir.clone(), move || sink.lock().unwrap().push(std::thread::current().id()));
    Rig { _tmp: tmp, store: DocsStore::at(dir), watcher, wake_threads }
}

fn next(r: &Rig) -> Option<DocChange> {
    r.watcher.recv_timeout(Duration::from_millis(1000))
}

#[test]
fn an_external_edit_is_reported_within_one_second_with_the_new_hash() {
    let r = rig(&[("a.md", "# One\n")]);
    let t0 = Instant::now();
    std::fs::write(r.store.dir().join("a.md"), "# Two, longer\n").unwrap();
    let ev = next(&r).expect("no event within 1 s");
    assert!(t0.elapsed() < Duration::from_millis(1000), "took {:?}", t0.elapsed());
    assert_eq!(ev, DocChange { rel: "a.md".into(), hash: Some(ContentHash::of(b"# Two, longer\n")) });
}

#[test]
fn the_wake_callback_runs_off_the_callers_thread_and_the_caller_only_drains_a_queue() {
    let r = rig(&[("a.md", "# One\n")]);
    assert!(r.watcher.drain().is_empty(), "nothing changed yet, draining is a non-blocking queue read");
    std::fs::write(r.store.dir().join("a.md"), "# Changed\n").unwrap();
    let deadline = Instant::now() + Duration::from_millis(1000);
    while r.wake_threads.lock().unwrap().is_empty() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let threads = r.wake_threads.lock().unwrap().clone();
    assert!(!threads.is_empty(), "wake never called");
    assert!(threads.iter().all(|t| *t != std::thread::current().id()), "wake must run on the watcher thread");
    assert_eq!(r.watcher.drain().len(), 1);
}

#[test]
fn new_deleted_and_nested_pages_and_the_manifest_are_reported() {
    let r = rig(&[("gone.md", "# Bye\n")]);
    std::fs::create_dir_all(r.store.dir().join("us")).unwrap();
    std::fs::write(r.store.dir().join("us/us-1.md"), "# US\n").unwrap();
    assert_eq!(next(&r).unwrap(), DocChange { rel: "us/us-1.md".into(), hash: Some(ContentHash::of(b"# US\n")) });

    std::fs::remove_file(r.store.dir().join("gone.md")).unwrap();
    assert_eq!(next(&r).unwrap(), DocChange { rel: "gone.md".into(), hash: None });

    std::fs::write(r.store.dir().join("docs-manifest.json"), "{}").unwrap();
    assert_eq!(next(&r).unwrap().rel, "docs-manifest.json");
}

#[test]
fn temp_files_hidden_files_and_other_extensions_are_ignored() {
    let r = rig(&[]);
    std::fs::write(r.store.dir().join("a.md.123-0.tmp"), "x").unwrap();
    std::fs::write(r.store.dir().join(".docs.lock"), "").unwrap();
    std::fs::write(r.store.dir().join("image.png"), "x").unwrap();
    std::fs::write(r.store.dir().join("real.md"), "# Real\n").unwrap();
    let ev = next(&r).unwrap();
    assert_eq!(ev.rel, "real.md");
    assert!(r.watcher.recv_timeout(Duration::from_millis(700)).is_none(), "only the page is reported");
}

#[test]
fn studios_own_atomic_write_is_reported_once_as_one_page_change() {
    let r = rig(&[("a.md", "# One\n")]);
    let h = ContentHash::of(b"# One\n");
    let new_hash = r.store.write_page("a.md", &Page::parse("# Two\n"), Expect::Hash(h)).unwrap();
    // The consumer recognises its own write by the hash it got back from the store.
    assert_eq!(next(&r).unwrap(), DocChange { rel: "a.md".into(), hash: Some(new_hash) });
    assert!(r.watcher.recv_timeout(Duration::from_millis(700)).is_none(), "no second event for the same write");
}

#[test]
fn an_idle_directory_reports_nothing() {
    let r = rig(&[("a.md", "# One\n"), ("b.md", "# Two\n")]);
    assert!(r.watcher.recv_timeout(Duration::from_millis(1200)).is_none());
}

#[test]
fn dropping_the_watcher_stops_its_thread() {
    let r = rig(&[("a.md", "# One\n")]);
    let stopped = r.watcher.stopped_flag();
    drop(r.watcher);
    let deadline = Instant::now() + Duration::from_millis(1000);
    while !stopped.load(std::sync::atomic::Ordering::Acquire) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(stopped.load(std::sync::atomic::Ordering::Acquire), "watcher thread still running after drop");
}
