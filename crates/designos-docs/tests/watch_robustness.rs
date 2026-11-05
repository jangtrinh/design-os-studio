//! Watcher edge cases found in review: size cap, a panicking wake callback, and no disk work on the caller's thread.

use designos_docs::{DocChange, DocsWatcher, WatchMode};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

#[test]
fn an_oversized_file_is_not_read_into_memory_and_not_reported() {
    let tmp = tempfile::tempdir().unwrap();
    let watcher = DocsWatcher::start(tmp.path(), || {});
    std::fs::write(tmp.path().join("huge.md"), vec![b'a'; 9 * 1024 * 1024]).unwrap();
    std::fs::write(tmp.path().join("small.md"), "# ok\n").unwrap();
    let mut seen = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline && !seen.iter().any(|c: &DocChange| c.rel == "small.md") {
        seen.extend(watcher.recv_timeout(Duration::from_millis(200)));
    }
    assert!(seen.iter().any(|c| c.rel == "small.md"), "the small page is reported: {seen:?}");
    // Let anything still queued arrive before judging.
    std::thread::sleep(Duration::from_millis(600));
    seen.extend(watcher.drain());
    assert!(!seen.iter().any(|c| c.rel == "huge.md"), "a page over the 8 MiB read limit is skipped, like the store: {seen:?}");
}

#[test]
fn a_panicking_wake_callback_still_marks_the_watcher_stopped() {
    let tmp = tempfile::tempdir().unwrap();
    let watcher = DocsWatcher::start(tmp.path(), || panic!("wake callback failed"));
    let stopped = watcher.stopped_flag();
    std::fs::write(tmp.path().join("a.md"), "# a\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !stopped.load(Ordering::Acquire) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(stopped.load(Ordering::Acquire), "the flag must say the watcher is dead");
}

#[test]
fn start_does_no_disk_work_on_the_callers_thread() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    for g in 0..10 {
        std::fs::create_dir_all(dir.join(format!("g{g}"))).unwrap();
        for i in 0..400 {
            std::fs::write(dir.join(format!("g{g}/p{i}.md")), "# x\n").unwrap();
        }
    }
    // What a full look at the folder costs here.
    let scan = Instant::now();
    let mut count = 0;
    for g in 0..10 {
        for entry in std::fs::read_dir(dir.join(format!("g{g}"))).unwrap().flatten() {
            let _ = entry.metadata();
            count += 1;
        }
    }
    let scan = scan.elapsed();
    assert_eq!(count, 4000);
    let started = Instant::now();
    let watcher = DocsWatcher::start(dir, || {});
    let took = started.elapsed();
    assert!(took * 4 < scan, "start() took {took:?}, a scan of the folder takes {scan:?}: it must not scan on the caller's thread");
    drop(watcher);
}

fn wait_for_mode(watcher: &DocsWatcher) -> WatchMode {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(mode) = watcher.mode() {
            return mode;
        }
        assert!(Instant::now() < deadline, "the watcher never settled on a mode");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn the_watcher_is_driven_by_the_system_not_by_a_timer() {
    let tmp = tempfile::tempdir().unwrap();
    let watcher = DocsWatcher::start(tmp.path(), || {});
    assert_eq!(wait_for_mode(&watcher), WatchMode::Native);
    std::fs::write(tmp.path().join("a.md"), "# a\n").unwrap();
    assert_eq!(watcher.recv_timeout(Duration::from_millis(1000)).map(|c| c.rel).as_deref(), Some("a.md"));
}

#[test]
fn the_timer_fallback_reports_changes_too() {
    let tmp = tempfile::tempdir().unwrap();
    let watcher = DocsWatcher::start_polling(tmp.path(), || {});
    assert_eq!(wait_for_mode(&watcher), WatchMode::Polling);
    std::fs::write(tmp.path().join("a.md"), "# a\n").unwrap();
    assert_eq!(watcher.recv_timeout(Duration::from_millis(1500)).map(|c| c.rel).as_deref(), Some("a.md"));
    std::fs::remove_file(tmp.path().join("a.md")).unwrap();
    let gone = watcher.recv_timeout(Duration::from_millis(1500)).unwrap();
    assert_eq!((gone.rel.as_str(), gone.hash), ("a.md", None));
}

#[test]
fn a_page_written_right_after_start_is_never_missed() {
    for _ in 0..20 {
        let tmp = tempfile::tempdir().unwrap();
        let watcher = DocsWatcher::start(tmp.path(), || {});
        std::fs::write(tmp.path().join("fast.md"), "# fast\n").unwrap();
        let change = watcher.recv_timeout(Duration::from_millis(1500));
        assert_eq!(change.map(|c| c.rel).as_deref(), Some("fast.md"));
    }
}

#[test]
fn a_docs_folder_that_does_not_exist_yet_is_picked_up_when_it_appears() {
    let tmp = tempfile::tempdir().unwrap();
    let docs = tmp.path().join("projects").join("p1").join("docs");
    let watcher = DocsWatcher::start(&docs, || {});
    assert_eq!(wait_for_mode(&watcher), WatchMode::Native);
    std::fs::create_dir_all(docs.join("spec")).unwrap();
    std::fs::write(docs.join("spec").join("first.md"), "# first\n").unwrap();
    assert_eq!(watcher.recv_timeout(Duration::from_millis(1500)).map(|c| c.rel).as_deref(), Some("spec/first.md"));
    // From here the folder itself is watched.
    std::fs::write(docs.join("second.md"), "# second\n").unwrap();
    assert_eq!(watcher.recv_timeout(Duration::from_millis(1500)).map(|c| c.rel).as_deref(), Some("second.md"));
}

#[test]
fn folders_created_one_level_at_a_time_with_pauses_are_followed() {
    let root = tempfile::tempdir().unwrap();
    let docs = root.path().join("proj").join("docs");
    let watcher = DocsWatcher::start(&docs, || {});
    wait_for_mode(&watcher);
    std::thread::sleep(Duration::from_millis(300));
    std::fs::create_dir(root.path().join("proj")).unwrap();
    std::thread::sleep(Duration::from_millis(600));
    std::fs::create_dir(&docs).unwrap();
    std::thread::sleep(Duration::from_millis(600));
    std::fs::write(docs.join("a.md"), "# a\n").unwrap();
    assert_eq!(watcher.recv_timeout(Duration::from_millis(2000)).map(|c| c.rel).as_deref(), Some("a.md"));
}

/// inotify drops the watch of a deleted folder without any error, so the folder must be armed again.
#[test]
fn a_docs_folder_that_is_deleted_and_created_again_is_watched_again() {
    let root = tempfile::tempdir().unwrap();
    let docs = root.path().join("docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("old.md"), "# old\n").unwrap();
    std::thread::sleep(Duration::from_millis(40));
    let watcher = DocsWatcher::start(&docs, || {});
    wait_for_mode(&watcher);
    std::thread::sleep(Duration::from_millis(200));

    std::fs::remove_dir_all(&docs).unwrap();
    let gone = watcher.recv_timeout(Duration::from_millis(2000)).expect("the deletion is reported");
    assert_eq!((gone.rel.as_str(), gone.hash), ("old.md", None));

    std::thread::sleep(Duration::from_millis(600));
    std::fs::create_dir_all(&docs).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    std::fs::write(docs.join("new.md"), "# new\n").unwrap();
    assert_eq!(watcher.recv_timeout(Duration::from_millis(2000)).map(|c| c.rel).as_deref(), Some("new.md"));

    std::thread::sleep(Duration::from_millis(300));
    std::fs::write(docs.join("new.md"), "# newer\n").unwrap();
    assert_eq!(watcher.recv_timeout(Duration::from_millis(2000)).map(|c| c.rel).as_deref(), Some("new.md"), "edits after the re-creation are seen");
}
