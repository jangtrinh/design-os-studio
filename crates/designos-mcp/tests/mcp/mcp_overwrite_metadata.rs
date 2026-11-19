#![cfg(unix)]

//! An overwrite through `designos_save_file` or `designos_export_fig_file` behaves like Studio's own save: the file keeps its
//! mode, and on macOS its ACL and extended attributes, and a file that only its mode (not an ACL) makes read-only is still
//! refused in the same words as before.

use designos_mcp::{server::Session, tools::errors::ErrorCode};
use serde_json::json;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static SEQ: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("designos-mcp-overwrite-{tag}-{}-{nonce}-{seq}", std::process::id()));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn page_id(s: &mut Session) -> u64 {
    s.call("designos_get_document", json!({})).unwrap()["pages"][0]["node_id"].as_u64().unwrap()
}

fn temp_files(dir: &Path) -> Vec<String> {
    fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".tmp")).collect()
}

#[cfg(target_os = "macos")]
fn current_user() -> String {
    let out = std::process::Command::new("/usr/bin/id").arg("-un").output().unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

#[cfg(target_os = "macos")]
fn chmod_acl(file: &Path, rule: &str) {
    let status = std::process::Command::new("/bin/chmod").args(["+a", rule, file.to_str().unwrap()]).status().unwrap();
    assert!(status.success(), "chmod +a failed");
}

#[cfg(target_os = "macos")]
fn clear_acl(file: &Path) {
    let _ = std::process::Command::new("/bin/chmod").args(["-N", file.to_str().unwrap()]).status();
}

#[cfg(target_os = "macos")]
#[test]
fn save_file_overwrite_preserves_macos_metadata() {
    let ws = TempDir::new("ws-save-xattr");
    let mut s = Session::new(&ws.0).unwrap();
    let page_id = page_id(&mut s);
    s.call("designos_create_node", json!({"parent_id": page_id, "kind": "rectangle"})).unwrap();
    s.call("designos_save_file", json!({"path": "document.designos"})).unwrap();

    let file = ws.0.join("document.designos");
    let attr_name = "com.designos.mcp-save-test";
    let status = std::process::Command::new("/usr/bin/xattr").args(["-w", attr_name, "preserve-this", file.to_str().unwrap()]).status().unwrap();
    assert!(status.success(), "xattr -w failed");
    let acl_rule = format!("user:{} allow write", current_user());
    chmod_acl(&file, &acl_rule);
    fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();

    s.call("designos_create_node", json!({"parent_id": page_id, "kind": "ellipse"})).unwrap();
    s.call("designos_save_file", json!({"path": "document.designos", "overwrite": true})).unwrap();

    let out = std::process::Command::new("/usr/bin/xattr").args(["-p", attr_name, file.to_str().unwrap()]).output().unwrap();
    assert!(out.status.success(), "xattr must survive MCP overwrite: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "preserve-this");
    let ls = std::process::Command::new("/bin/ls").args(["-le", file.to_str().unwrap()]).output().unwrap();
    assert!(ls.status.success(), "ls -le failed");
    let ls_text = String::from_utf8_lossy(&ls.stdout);
    assert!(ls_text.contains(&acl_rule), "write ACL must survive MCP overwrite: {ls_text}");
    assert_eq!(fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o644, "mode must survive MCP overwrite");
    clear_acl(&file);
}

#[cfg(target_os = "macos")]
#[test]
fn save_file_overwrite_allows_acl_write_on_readonly_mode() {
    let ws = TempDir::new("ws-save-acl-readonly");
    let mut s = Session::new(&ws.0).unwrap();
    let page_id = page_id(&mut s);
    s.call("designos_save_file", json!({"path": "document.designos"})).unwrap();

    let file = ws.0.join("document.designos");
    chmod_acl(&file, &format!("user:{} allow write", current_user()));
    fs::set_permissions(&file, fs::Permissions::from_mode(0o444)).unwrap();

    s.call("designos_create_node", json!({"parent_id": page_id, "kind": "ellipse"})).unwrap();
    let res = s.call("designos_save_file", json!({"path": "document.designos", "overwrite": true}));

    let mode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
    clear_acl(&file);
    res.expect("a 0444 file whose ACL grants write is saved like Studio saves it");
    assert_eq!(mode, 0o444, "mode must survive ACL-authorized overwrite");
    assert!(temp_files(&ws.0).is_empty(), "no temporary file stays behind");
}

#[cfg(target_os = "macos")]
#[test]
fn export_fig_overwrite_preserves_extended_attributes() {
    let ws = TempDir::new("ws-fig-xattr");
    let mut s = Session::new(&ws.0).unwrap();
    s.call("designos_export_fig_file", json!({"output_path": "out.fig"})).unwrap();

    let file = ws.0.join("out.fig");
    let attr_name = "com.designos.mcp-fig-test";
    let status = std::process::Command::new("/usr/bin/xattr").args(["-w", attr_name, "keep-me", file.to_str().unwrap()]).status().unwrap();
    assert!(status.success(), "xattr -w failed");

    s.call("designos_export_fig_file", json!({"output_path": "out.fig", "overwrite": true})).unwrap();

    let out = std::process::Command::new("/usr/bin/xattr").args(["-p", attr_name, file.to_str().unwrap()]).output().unwrap();
    assert!(out.status.success(), "xattr must survive the .fig overwrite: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "keep-me");
}

#[test]
fn a_file_that_only_its_mode_makes_read_only_is_refused_in_the_same_words_and_left_alone() {
    let ws = TempDir::new("ws-plain-readonly");
    let mut s = Session::new(&ws.0).unwrap();
    s.call("designos_save_file", json!({"path": "locked.designos"})).unwrap();
    s.call("designos_export_fig_file", json!({"output_path": "locked.fig"})).unwrap();
    let designos = ws.0.join("locked.designos");
    let fig = ws.0.join("locked.fig");
    let (designos_before, fig_before) = (fs::read(&designos).unwrap(), fs::read(&fig).unwrap());
    for file in [&designos, &fig] {
        fs::set_permissions(file, fs::Permissions::from_mode(0o444)).unwrap();
    }
    if fs::OpenOptions::new().write(true).open(&designos).is_ok() {
        // Running as a user the mode does not bind (root): the refusal cannot be produced here.
        return;
    }

    let page = page_id(&mut s);
    s.call("designos_create_node", json!({"parent_id": page, "kind": "ellipse"})).unwrap();
    let save = s.call("designos_save_file", json!({"path": "locked.designos", "overwrite": true})).expect_err("a read-only file must be refused");
    let export = s.call("designos_export_fig_file", json!({"output_path": "locked.fig", "overwrite": true})).expect_err("a read-only file must be refused");

    for (err, name) in [(save, "locked.designos"), (export, "locked.fig")] {
        assert_eq!(err.code, ErrorCode::InvalidArguments, "{err}");
        assert_eq!(err.message, format!("{name} is read-only. Make it writable first, or choose a new path."));
    }
    assert_eq!(fs::read(&designos).unwrap(), designos_before);
    assert_eq!(fs::read(&fig).unwrap(), fig_before);
    assert!(temp_files(&ws.0).is_empty(), "no temporary file stays behind");
}

#[test]
fn an_overwrite_into_a_folder_that_does_not_exist_is_refused_and_makes_no_folder() {
    let ws = TempDir::new("ws-missing-folder");
    let mut s = Session::new(&ws.0).unwrap();

    let err = s.call("designos_save_file", json!({"path": "no/such/doc.designos", "overwrite": true})).expect_err("the folder must already exist");

    assert_eq!(err.code, ErrorCode::InvalidArguments, "{err}");
    assert!(err.message.starts_with("Save directory must already exist"), "{err}");
    assert!(!ws.0.join("no").exists(), "the overwrite made a folder");
}

#[test]
fn an_overwrite_leaves_the_workspace_with_the_one_file_and_no_temporary_file() {
    let ws = TempDir::new("ws-no-tmp");
    let mut s = Session::new(&ws.0).unwrap();
    s.call("designos_save_file", json!({"path": "doc.designos"})).unwrap();
    s.call("designos_save_file", json!({"path": "doc.designos", "overwrite": true})).unwrap();

    let mut names: Vec<String> = fs::read_dir(&ws.0).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    assert_eq!(names, vec!["doc.designos".to_string()]);
}
