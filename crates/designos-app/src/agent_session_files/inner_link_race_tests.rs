//! The race: an agent that can write its session folder swaps a real folder with a link to somewhere else, over and over,
//! while Studio rewrites the session files. Not one byte may land in the target of the link.

use super::hook_quoting_tests::job_in;
use super::refusal::{refusal_of, Refusal};
use super::test_support::made_session_folder;
use super::*;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::symlink;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Folders that stand for the ones Studio writes into, with the depth at which they sit.
const SWAPPED_FOLDERS: &[&str] = &[".gemini", ".grok/hooks", ".agents/plugins/designos"];

fn c_path(path: &Path) -> std::ffi::CString {
    std::ffi::CString::new(path.as_os_str().as_bytes()).expect("a path without NUL")
}

/// Makes `a` and `b` trade places in one step where the system can (`renamex_np` with `RENAME_SWAP` on macOS).
#[cfg(target_os = "macos")]
fn swap(a: &Path, b: &Path) -> bool {
    // SAFETY: both arguments are NUL-terminated paths that live through the call.
    unsafe { libc::renamex_np(c_path(a).as_ptr(), c_path(b).as_ptr(), libc::RENAME_SWAP) == 0 }
}

/// Makes `a` and `b` trade places in one step (`renameat2` with `RENAME_EXCHANGE`, which is 2 in the kernel's headers).
#[cfg(target_os = "linux")]
fn swap(a: &Path, b: &Path) -> bool {
    // SAFETY: both arguments are NUL-terminated paths that live through the call.
    unsafe { libc::syscall(libc::SYS_renameat2, libc::AT_FDCWD, c_path(a).as_ptr(), libc::AT_FDCWD, c_path(b).as_ptr(), 2 as libc::c_uint) == 0 }
}

/// Trades the places of `a` and `b` in three renames: there is a moment with neither at its name.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn swap(a: &Path, b: &Path) -> bool {
    let spare = a.with_extension("swap-spare");
    std::fs::rename(a, &spare).is_ok() && std::fs::rename(b, a).is_ok() && std::fs::rename(&spare, b).is_ok()
}

/// A thread that swaps `<session>/<rel>` (a real folder) with a link of the same parent that points at `target`.
fn start_swapper(session: &Path, rel: &str, target: &Path, stop: Arc<AtomicBool>, swaps: Arc<AtomicU64>) -> std::thread::JoinHandle<()> {
    let real = session.join(rel);
    std::fs::create_dir_all(&real).expect("the real folder");
    let alt = real.with_file_name(format!("{}-alt", real.file_name().expect("a name").to_string_lossy()));
    symlink(target, &alt).expect("the link the folder is swapped with");
    std::thread::spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            if swap(&real, &alt) {
                swaps.fetch_add(1, Ordering::Relaxed);
            }
            std::thread::sleep(Duration::from_micros(30));
        }
    })
}

#[test]
fn no_byte_lands_in_the_target_of_a_link_swapped_in_while_studio_rewrites_the_session_files() {
    let target = super::folder_tests::scratch_dir("race-target");
    let _target = super::folder_tests::Cleanup(vec![target.clone()]);
    let (session, _session) = made_session_folder("race");
    let (stop, swaps) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicU64::new(0)));
    let swappers: Vec<_> = SWAPPED_FOLDERS.iter().map(|rel| start_swapper(&session, rel, &target, stop.clone(), swaps.clone())).collect();

    let (mut rewrites, mut refused, mut clean, mut other, mut hits) = (0u32, 0u32, 0u32, Vec::new(), 0u32);
    let deadline = Instant::now() + Duration::from_secs(20);
    while rewrites < 200 && Instant::now() < deadline && hits == 0 {
        match crate::terminal::write_session_agent_files_for_job(&job_in(&session)) {
            Err(e) if refusal_of(&e) == Some(Refusal::Link) => refused += 1,
            Err(e) => other.push(e.to_string()),
            Ok(()) => clean += 1,
        }
        rewrites += 1;
        hits += u32::from(std::fs::read_dir(&target).expect("the target").next().is_some());
    }
    stop.store(true, Ordering::Relaxed);
    swappers.into_iter().for_each(|swapper| swapper.join().expect("the swapper ends"));

    eprintln!("race: swaps={} rewrites={rewrites} refused={refused} clean={clean} other={} HIT={hits}", swaps.load(Ordering::Relaxed), other.len());
    assert_eq!(hits, 0, "bytes landed in the target of a link swapped in during a rewrite");
    assert!(swaps.load(Ordering::Relaxed) > 20, "the swappers ran, so the folders really changed under the writer");
    assert!(other.is_empty(), "a rewrite failed for a reason other than a link: {:?}", other.first());
}
