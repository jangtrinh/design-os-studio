//! `write_atomic` must behave like a plain `fs::write` for links, modes and read-only files,
//! and must stay safe when many saves race for one path.

use designos_library::write_atomic;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("atomic-save-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn leftover_tmp_files(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".tmp")).collect()
}

#[cfg(unix)]
#[test]
fn saving_through_a_symlink_updates_the_real_file_and_keeps_the_link() {
    let dir = scratch("symlink");
    let real = dir.join("real.designos");
    let link = dir.join("link.designos");
    std::fs::write(&real, b"old").unwrap();
    std::os::unix::fs::symlink(&real, &link).unwrap();

    write_atomic(&link, b"new").unwrap();

    assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink(), "the link must stay a link");
    assert_eq!(std::fs::read(&real).unwrap(), b"new", "the real file receives the bytes");
    assert_eq!(std::fs::read(&link).unwrap(), b"new");
    assert!(leftover_tmp_files(&dir).is_empty());
}

#[cfg(unix)]
#[test]
fn saving_through_a_symlink_into_another_directory_writes_next_to_the_real_file() {
    let dir = scratch("symlink-dir");
    let elsewhere = dir.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    let real = elsewhere.join("real.designos");
    let link = dir.join("link.designos");
    std::fs::write(&real, b"old").unwrap();
    std::os::unix::fs::symlink(&real, &link).unwrap();

    write_atomic(&link, b"new").unwrap();

    assert_eq!(std::fs::read(&real).unwrap(), b"new");
    assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
    assert!(leftover_tmp_files(&dir).is_empty() && leftover_tmp_files(&elsewhere).is_empty());
}

#[cfg(unix)]
#[test]
fn saving_keeps_the_existing_permission_mode() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("mode");
    let file = dir.join("private.designos");
    std::fs::write(&file, b"old").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();

    write_atomic(&file, b"new").unwrap();

    assert_eq!(std::fs::read(&file).unwrap(), b"new");
    assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
}

#[cfg(unix)]
#[test]
fn read_only_target_is_refused_and_left_untouched() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("readonly");
    let file = dir.join("locked.designos");
    std::fs::write(&file, b"old").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o444)).unwrap();

    let err = write_atomic(&file, b"new").expect_err("a read-only file must not be replaced");

    assert!(err.to_string().contains("read-only"), "error names the cause: {err}");
    assert_eq!(std::fs::read(&file).unwrap(), b"old");
    assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o444);
    assert!(leftover_tmp_files(&dir).is_empty());
}

#[test]
fn directory_target_is_refused() {
    let dir = scratch("dir-target");
    let sub = dir.join("sub.designos");
    std::fs::create_dir(&sub).unwrap();
    assert!(write_atomic(&sub, b"x").is_err());
    assert!(sub.is_dir());
}

#[test]
fn missing_file_and_parent_directories_are_created() {
    let dir = scratch("create");
    let file = dir.join("a").join("b").join("new.designos");
    write_atomic(&file, b"fresh").unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), b"fresh");
    assert!(leftover_tmp_files(file.parent().unwrap()).is_empty());
}

#[test]
fn forty_concurrent_saves_of_one_file_all_succeed_and_none_is_torn() {
    const SAVERS: usize = 40;
    const LEN: usize = 64 * 1024;
    let dir = scratch("concurrent");
    let file = dir.join("doc.designos");
    std::fs::write(&file, b"seed").unwrap();
    let start = Arc::new(Barrier::new(SAVERS));

    let handles: Vec<_> = (0..SAVERS)
        .map(|i| {
            let (file, start) = (file.clone(), start.clone());
            std::thread::spawn(move || {
                let bytes = vec![b'a' + (i % 26) as u8; LEN];
                start.wait();
                write_atomic(&file, &bytes).map_err(|e| e.to_string())
            })
        })
        .collect();
    let failures: Vec<String> = handles.into_iter().filter_map(|h| h.join().unwrap().err()).collect();

    assert!(failures.is_empty(), "{} of {SAVERS} saves failed: {failures:?}", failures.len());
    let content = std::fs::read(&file).unwrap();
    assert_eq!(content.len(), LEN, "the file holds one complete save");
    assert!(content.iter().all(|b| *b == content[0]), "the file mixes bytes of two saves");
    assert!(leftover_tmp_files(&dir).is_empty(), "temporary files left behind");
}

#[cfg(unix)]
#[test]
fn saving_through_a_dangling_symlink_creates_the_target_and_keeps_the_link() {
    let dir = scratch("dangling-symlink");
    let real = dir.join("nested").join("real.designos");
    let link = dir.join("link.designos");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    assert!(!real.exists(), "precondition: symlink target does not exist yet");

    write_atomic(&link, b"created-through-dangling-link").unwrap();

    assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink(), "dangling symlink must stay a symlink, not be replaced by a regular file");
    assert_eq!(std::fs::read(&real).unwrap(), b"created-through-dangling-link", "target of the dangling symlink must be created");
    assert_eq!(std::fs::read(&link).unwrap(), b"created-through-dangling-link");
    assert!(leftover_tmp_files(&dir).is_empty());
}

#[test]
fn filename_of_249_or_more_bytes_succeeds_without_enametoolong() {
    let dir = scratch("long-vietnamese-name");
    // 1. 123 * 2 bytes ("ă" in UTF-8) + ".designos" (9 bytes) = 255 UTF-8 bytes (hits Linux ext4 255-byte NAME_MAX).
    // 2. 80 * 3 bytes ("ệ" in UTF-8) + ".designos" (9 bytes) = 249 UTF-8 bytes.
    // 3. 242 ASCII bytes + ".designos" (9 bytes) = 251 bytes/chars (hits both macOS APFS 255-char and Linux 255-byte NAME_MAX).
    for long_name in [format!("{}.designos", "ă".repeat(123)), format!("{}.designos", "ệ".repeat(80)), format!("{}.designos", "a".repeat(242))] {
        assert!(long_name.len() >= 249, "len={}", long_name.len());
        let file = dir.join(&long_name);

        // Plain fs::write works; write_atomic must also succeed without ENAMETOOLONG.
        std::fs::write(&file, b"initial").unwrap();
        write_atomic(&file, b"updated-long-name").unwrap();

        assert_eq!(std::fs::read(&file).unwrap(), b"updated-long-name");
        assert!(leftover_tmp_files(&dir).is_empty());
    }
}

#[cfg(unix)]
#[test]
fn temp_file_is_never_world_readable_while_saving_a_0600_document() {
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Barrier;
    use std::time::{Duration, Instant};

    let dir = scratch("temp-mode-0600");
    let file = dir.join("secret.designos");
    std::fs::write(&file, b"old-secret").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();

    let ready = Arc::new(Barrier::new(2));
    let done = Arc::new(AtomicBool::new(false));
    let seen_tmp = Arc::new(AtomicBool::new(false));
    let dir_obs = dir.clone();
    let ready_obs = ready.clone();
    let done_obs = done.clone();
    let seen_obs = seen_tmp.clone();

    let observer = std::thread::spawn(move || {
        ready_obs.wait();
        let mut bad_modes = Vec::new();
        while !done_obs.load(Ordering::Relaxed) {
            if let Ok(entries) = std::fs::read_dir(&dir_obs) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if name.ends_with(".tmp") {
                        if let Ok(meta) = entry.metadata() {
                            seen_obs.store(true, Ordering::Relaxed);
                            let mode = meta.permissions().mode() & 0o777;
                            if mode != 0o600 {
                                bad_modes.push((name, mode));
                            }
                        }
                    }
                }
            }
        }
        bad_modes
    });

    ready.wait();
    let payload = vec![b'S'; 2 * 1024 * 1024];
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut writes = 0usize;
    while (writes < 10 || !seen_tmp.load(Ordering::Relaxed)) && Instant::now() < deadline {
        write_atomic(&file, &payload).unwrap();
        writes += 1;
    }
    done.store(true, Ordering::Relaxed);
    let bad_modes = observer.join().unwrap();

    assert!(seen_tmp.load(Ordering::Relaxed), "observer should catch at least one in-flight .tmp file");
    assert!(bad_modes.is_empty(), "temp file for 0600 document was briefly created with wider mode: {bad_modes:?}");
    assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
}

#[cfg(unix)]
#[test]
fn file_with_0644_mode_owned_by_another_user_or_not_writable_is_refused() {
    use std::os::unix::fs::PermissionsExt;

    let dir = scratch("foreign-0644");
    let file = dir.join("foreign.designos");
    std::fs::write(&file, b"original").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();

    #[cfg(target_os = "macos")]
    {
        // Attach a write-deny ACL on `file` inside our writable `dir`:
        // - `meta.permissions().mode() & 0o777` is still `0o644` (`readonly()` is false).
        // - `dir` is writable, so an unguarded `rename(tmp, file)` replaces `file`!
        // - Plain `fs::write(&file, ...)` fails with PermissionDenied, and `write_atomic` must too.
        let status = std::process::Command::new("/bin/chmod").args(["+a", "everyone deny write", file.to_str().unwrap()]).status().unwrap();
        assert!(status.success());
        assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o644);
        assert!(!std::fs::metadata(&file).unwrap().permissions().readonly());
        assert!(std::fs::write(&file, b"plain").is_err(), "precondition: plain write is denied");

        let res = write_atomic(&file, b"replaced");
        let _ = std::process::Command::new("/bin/chmod").args(["-N", file.to_str().unwrap()]).status();
        assert!(res.is_err(), "0644 file that cannot be opened for writing in a writable folder must not be replaced");
        assert_eq!(std::fs::read(&file).unwrap(), b"original");
        assert!(leftover_tmp_files(&dir).is_empty());
    }
}

#[test]
fn crashed_save_leftover_tmp_files_are_swept_on_next_save() {
    let dir = scratch("sweep-stale-tmp");
    let file = dir.join("doc.designos");
    std::fs::write(&file, b"v1").unwrap();

    // Simulate leftover temp files from crashed processes (dead PIDs and legacy .tmp).
    let dead_tmp_1 = dir.join("doc.designos.999999-0.tmp");
    let dead_tmp_2 = dir.join("doc.designos.999998-17.tmp");
    let legacy_tmp = dir.join("doc.designos.tmp");
    std::fs::write(&dead_tmp_1, b"partial-1").unwrap();
    std::fs::write(&dead_tmp_2, b"partial-2").unwrap();
    std::fs::write(&legacy_tmp, b"partial-legacy").unwrap();

    // User files with `.tmp` suffix and live-process temp files MUST NOT be deleted!
    let user_notes_tmp = dir.join("doc.designos.my-notes.tmp");
    let live_self_tmp = dir.join(format!("doc.designos.{}-99999.tmp", std::process::id()));
    std::fs::write(&user_notes_tmp, b"keep-user-notes").unwrap();
    std::fs::write(&live_self_tmp, b"in-flight-save").unwrap();
    #[cfg(unix)]
    let pid1_tmp = {
        // PID 1 (launchd/init) is owned by root: kill(1, 0) fails with EPERM, not ESRCH.
        let p = dir.join("doc.designos.1-0.tmp");
        std::fs::write(&p, b"other-user-live-pid").unwrap();
        p
    };

    write_atomic(&file, b"v2").unwrap();

    assert_eq!(std::fs::read(&file).unwrap(), b"v2");
    assert!(!dead_tmp_1.exists(), "dead PID temp file 1 must be swept");
    assert!(!dead_tmp_2.exists(), "dead PID temp file 2 must be swept");
    assert!(!legacy_tmp.exists(), "legacy .tmp must be swept");
    assert_eq!(std::fs::read(&user_notes_tmp).unwrap(), b"keep-user-notes", "user file doc.designos.my-notes.tmp must never be deleted");
    assert_eq!(std::fs::read(&live_self_tmp).unwrap(), b"in-flight-save", "live process temp file must never be deleted");
    #[cfg(unix)]
    assert_eq!(std::fs::read(&pid1_tmp).unwrap(), b"other-user-live-pid", "EPERM process temp file must never be treated as dead");
}

#[cfg(unix)]
#[test]
fn publish_and_font_cache_use_the_shared_hardened_write_atomic() {
    let dir = scratch("shared-helper");
    let real = dir.join("sub").join("font.ttf");
    let link = dir.join("font-link.ttf");
    std::os::unix::fs::symlink(&real, &link).unwrap();

    // Calling designos_fonts::write_atomic on a dangling symlink creates the target and keeps the link.
    designos_fonts::write_atomic(&link, b"TTF-BYTES").unwrap();
    assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
    assert_eq!(std::fs::read(&real).unwrap(), b"TTF-BYTES");
}
