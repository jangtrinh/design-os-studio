//! Idle CPU of the watcher with 200 pages, measured the way the Project tab gate measures it: `ps -o time=` of the
//! process over 20 s (10 ms resolution). Ignored by default because it sleeps for 20 s:
//! `cargo test -p designos-docs --test watch_idle_cpu -- --ignored --nocapture`

use std::time::Duration;

fn cpu_seconds() -> f64 {
    let out = std::process::Command::new("ps").args(["-o", "time=", "-p", &std::process::id().to_string()]).output().expect("ps");
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    // [[dd-]hh:]mm:ss[.cc]
    text.rsplit(':').enumerate().map(|(i, part)| part.trim().parse::<f64>().unwrap_or(0.0) * 60f64.powi(i as i32)).sum()
}

#[test]
#[ignore = "sleeps 20 s"]
fn idle_watcher_with_200_pages_costs_no_cpu() {
    let tmp = tempfile::tempdir().unwrap();
    for g in 0..5 {
        std::fs::create_dir_all(tmp.path().join(format!("g{g}"))).unwrap();
        for i in 0..40 {
            std::fs::write(tmp.path().join(format!("g{g}/p{i}.md")), "# x\n").unwrap();
        }
    }
    let watcher = designos_docs::DocsWatcher::start(tmp.path(), || {});
    let mode = {
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while watcher.mode().is_none() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        watcher.mode()
    };
    std::thread::sleep(Duration::from_secs(2)); // the first look and the system watch settle
    let before = cpu_seconds();
    std::thread::sleep(Duration::from_secs(20));
    let after = cpu_seconds();
    drop(watcher);
    println!("IDLE_CPU mode={:?} 200 pages, 20 s idle: before={before:.2}s after={after:.2}s delta={:.2}s", mode, after - before);
    assert!(after - before < 0.01, "idle watcher used {:.2} s of CPU in 20 s", after - before);
}
