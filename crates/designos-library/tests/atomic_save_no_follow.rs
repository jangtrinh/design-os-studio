//! `write_atomic_with_no_follow` is the streaming twin of `write_atomic_with` for a path an agent named. It keeps what a save
//! keeps (mode, macOS ACLs and extended attributes, streaming, a failed fill leaves the old file) and refuses what a plain
//! save would follow or carry onto another name: a link at the final component, a hard link, a pipe, a folder.

use designos_library::{no_follow_refusal, write_atomic_no_follow, write_atomic_with_no_follow, NoFollowRefusal};
use std::path::{Path, PathBuf};

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("atomic-no-follow-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn leftover_tmp_files(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".tmp")).collect()
}

fn into_io(res: designos_library::Result<()>) -> std::io::Result<()> {
    res.map_err(|e| match e {
        designos_library::LibraryError::Io(io) => io,
        other => std::io::Error::other(other.to_string()),
    })
}

fn save(path: &Path, bytes: &'static [u8]) -> std::io::Result<()> {
    into_io(write_atomic_with_no_follow(path, |out| out.write_all(bytes)))
}

fn refusal(res: std::io::Result<()>) -> Option<NoFollowRefusal> {
    no_follow_refusal(&res.expect_err("the target must be refused"))
}

#[cfg(unix)]
#[test]
fn a_link_at_the_final_component_is_refused_and_what_it_points_at_is_untouched() {
    let dir = scratch("link");
    let real = dir.join("real.designos");
    let link = dir.join("link.designos");
    std::fs::write(&real, b"old").unwrap();
    std::os::unix::fs::symlink(&real, &link).unwrap();

    assert_eq!(refusal(save(&link, b"new")), Some(NoFollowRefusal::Link));

    assert_eq!(std::fs::read(&real).unwrap(), b"old");
    assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink(), "the link must stay a link");
    assert!(leftover_tmp_files(&dir).is_empty());
}

#[cfg(unix)]
#[test]
fn a_dangling_link_is_refused_and_nothing_is_created_behind_it() {
    let dir = scratch("dangling");
    let victim = dir.join("victim.designos");
    let link = dir.join("link.designos");
    std::os::unix::fs::symlink(&victim, &link).unwrap();

    assert_eq!(refusal(save(&link, b"new")), Some(NoFollowRefusal::Link));

    assert!(!victim.exists(), "a dangling link must not create its target");
    assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
}

#[cfg(unix)]
#[test]
fn a_file_with_a_second_name_is_refused_and_both_names_keep_their_bytes() {
    let dir = scratch("hard-link");
    let file = dir.join("doc.designos");
    let other = dir.join("other-name.designos");
    std::fs::write(&file, b"old").unwrap();
    std::fs::hard_link(&file, &other).unwrap();

    assert_eq!(refusal(save(&file, b"new")), Some(NoFollowRefusal::SecondName));

    assert_eq!(std::fs::read(&file).unwrap(), b"old");
    assert_eq!(std::fs::read(&other).unwrap(), b"old");
    assert!(leftover_tmp_files(&dir).is_empty());
}

#[cfg(unix)]
#[test]
fn a_named_pipe_is_refused_without_waiting_for_a_reader() {
    let dir = scratch("fifo");
    let fifo = dir.join("doc.designos");
    assert!(std::process::Command::new("mkfifo").arg(&fifo).status().unwrap().success());
    let (tx, rx) = std::sync::mpsc::channel();
    let target = fifo.clone();
    std::thread::spawn(move || {
        let _ = tx.send(save(&target, b"new"));
    });
    let res = rx.recv_timeout(std::time::Duration::from_secs(2)).expect("a named pipe must be refused, not waited on");
    assert_eq!(refusal(res), Some(NoFollowRefusal::NotRegularFile));
    assert!(leftover_tmp_files(&dir).is_empty());
}

#[test]
fn a_folder_is_refused_and_stays_a_folder() {
    let dir = scratch("folder");
    let sub = dir.join("sub.designos");
    std::fs::create_dir(&sub).unwrap();

    assert_eq!(refusal(save(&sub, b"new")), Some(NoFollowRefusal::Directory));

    assert!(sub.is_dir());
}

#[cfg(unix)]
#[test]
fn a_read_only_file_is_refused_and_left_untouched() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("readonly");
    let file = dir.join("locked.designos");
    std::fs::write(&file, b"old").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o444)).unwrap();

    assert_eq!(refusal(save(&file, b"new")), Some(NoFollowRefusal::NotWritable));

    assert_eq!(std::fs::read(&file).unwrap(), b"old");
    assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o444);
    assert!(leftover_tmp_files(&dir).is_empty());
}

#[test]
fn a_new_file_is_created_but_a_missing_folder_is_not() {
    let dir = scratch("create");
    let file = dir.join("new.designos");
    save(&file, b"fresh").unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), b"fresh");

    let missing = dir.join("no-such-folder").join("new.designos");
    let err = save(&missing, b"fresh").expect_err("the writer does not make folders");
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound, "{err}");
    assert!(!dir.join("no-such-folder").exists(), "no folder was made");
    assert!(leftover_tmp_files(&dir).is_empty());
}

#[cfg(unix)]
#[test]
fn a_replace_keeps_the_existing_permission_mode() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("mode");
    let file = dir.join("private.designos");
    std::fs::write(&file, b"old").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();

    save(&file, b"new").unwrap();

    assert_eq!(std::fs::read(&file).unwrap(), b"new");
    assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
}

#[test]
fn a_fill_that_fails_leaves_the_old_bytes_and_no_temporary_file() {
    let dir = scratch("failed-fill");
    let file = dir.join("doc.designos");
    std::fs::write(&file, b"old").unwrap();

    let res = into_io(write_atomic_with_no_follow(&file, |out| {
        out.write_all(b"half of the new content")?;
        Err(std::io::Error::other("the document could not be serialized"))
    }));

    assert!(res.is_err());
    assert_eq!(std::fs::read(&file).unwrap(), b"old");
    assert!(leftover_tmp_files(&dir).is_empty());
}

#[test]
fn the_content_is_streamed_to_the_temporary_file_while_it_is_produced() {
    let dir = scratch("streams");
    let file = dir.join("doc.designos");
    const CHUNK: usize = 64 * 1024;
    const CHUNKS: usize = 64;
    let mut on_disk_midway = 0;

    into_io(write_atomic_with_no_follow(&file, |out| {
        for i in 0..CHUNKS {
            out.write_all(&[b'x'; CHUNK])?;
            if i + 1 == CHUNKS * 3 / 4 {
                // Three quarters of 4 MiB are written: a whole-content buffer would hold all of it, a stream has most on disk.
                on_disk_midway = std::fs::read_dir(&dir)
                    .unwrap()
                    .filter_map(Result::ok)
                    .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
                    .map(|e| e.metadata().unwrap().len())
                    .max()
                    .unwrap_or(0);
            }
        }
        Ok(())
    }))
    .unwrap();

    assert!(on_disk_midway >= 1024 * 1024, "only {on_disk_midway} bytes reached the temporary file while the content was produced");
    assert_eq!(std::fs::metadata(&file).unwrap().len(), (CHUNK * CHUNKS) as u64);
}

#[cfg(unix)]
#[test]
fn a_link_swapped_in_while_the_content_is_produced_is_replaced_and_never_written_through() {
    let dir = scratch("swap");
    let file = dir.join("doc.designos");
    let victim = dir.join("victim.designos");
    std::fs::write(&file, b"old").unwrap();
    std::fs::write(&victim, b"victim").unwrap();

    into_io(write_atomic_with_no_follow(&file, |out| {
        std::fs::remove_file(&file)?;
        std::os::unix::fs::symlink(&victim, &file)?;
        out.write_all(b"new")
    }))
    .unwrap();

    assert_eq!(std::fs::read(&victim).unwrap(), b"victim", "the swapped-in link was written through");
    assert!(!std::fs::symlink_metadata(&file).unwrap().file_type().is_symlink(), "the link must be replaced by the new file");
    assert_eq!(std::fs::read(&file).unwrap(), b"new");
}

#[test]
fn the_bytes_twin_writes_the_same_file() {
    let dir = scratch("bytes");
    let file = dir.join("doc.designos");
    into_io(write_atomic_no_follow(&file, b"bytes")).unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), b"bytes");
}

#[cfg(target_os = "macos")]
fn chmod_acl(file: &Path, rule: &str) {
    let status = std::process::Command::new("/bin/chmod").args(["+a", rule, file.to_str().unwrap()]).status().unwrap();
    assert!(status.success(), "chmod +a failed");
}

#[cfg(target_os = "macos")]
fn current_user() -> String {
    let out = std::process::Command::new("/usr/bin/id").arg("-un").output().unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

#[cfg(target_os = "macos")]
#[test]
fn a_replace_keeps_the_extended_attributes_and_the_acl() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("metadata");
    let file = dir.join("doc.designos");
    std::fs::write(&file, b"v1").unwrap();
    let status =
        std::process::Command::new("/usr/bin/xattr").args(["-w", "com.designos.test-xattr", "xattr-payload", file.to_str().unwrap()]).status().unwrap();
    assert!(status.success(), "xattr -w failed");
    let rule = format!("user:{} allow write", current_user());
    chmod_acl(&file, &rule);
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();

    save(&file, b"v2").unwrap();

    let out = std::process::Command::new("/usr/bin/xattr").args(["-p", "com.designos.test-xattr", file.to_str().unwrap()]).output().unwrap();
    assert!(out.status.success(), "the xattr must survive: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "xattr-payload");
    let ls = std::process::Command::new("/bin/ls").args(["-le", file.to_str().unwrap()]).output().unwrap();
    let listing = String::from_utf8_lossy(&ls.stdout).into_owned();
    let _ = std::process::Command::new("/bin/chmod").args(["-N", file.to_str().unwrap()]).status();
    assert!(listing.contains(&rule), "the ACL must survive, got:\n{listing}");
    assert_eq!(std::fs::read(&file).unwrap(), b"v2");
    assert!(leftover_tmp_files(&dir).is_empty());
}

#[cfg(target_os = "macos")]
#[test]
fn an_acl_that_grants_write_on_a_0444_file_lets_the_replace_through() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("acl-0444");
    let file = dir.join("doc.designos");
    std::fs::write(&file, b"v1").unwrap();
    let rule = format!("user:{} allow write", current_user());
    chmod_acl(&file, &rule);
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o444)).unwrap();

    let res = save(&file, b"v2");

    let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
    let bytes = std::fs::read(&file).unwrap();
    let _ = std::process::Command::new("/bin/chmod").args(["-N", file.to_str().unwrap()]).status();
    res.expect("a 0444 file whose ACL grants write must be replaced");
    assert_eq!(bytes, b"v2");
    assert_eq!(mode, 0o444, "the mode must survive");
    assert!(leftover_tmp_files(&dir).is_empty());
}
