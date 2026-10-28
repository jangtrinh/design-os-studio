//! The scan of shipped code on source made for the test, part two: the ways to write a file the scan has to see whatever the code around
//! it looks like. A `use` alias, a program started by name, a redirect in any form, a session file name, a name joined to a session path.

use super::scanner_tests::{flagged, scan};

#[test]
fn a_use_that_renames_or_globs_std_fs_is_reported() {
    for statement in [
        "use std::fs as f;",
        "use std::{fs as f2, io};",
        "use std::{\n    io,\n    fs as f3,\n};",
        "use std::fs::*;",
        "pub use std::fs::File as F;",
        "use std as s;",
    ] {
        let found = scan(&[("lib.rs", &format!("{statement}\npub fn nothing() {{}}\n"))]);
        assert!(!found.is_empty(), "not reported: {statement}");
    }
}

#[test]
fn a_use_that_does_not_name_std_fs_is_not_reported() {
    for statement in
        ["use std::os::unix::fs::PermissionsExt;", "use std::{io, path::Path};", "use std::sync::Arc;", "use std::fsx::Thing;", "use self::fs::Thing;"]
    {
        let found = scan(&[("lib.rs", &format!("{statement}\npub fn nothing() {{}}\n"))]);
        assert!(found.is_empty(), "reported: {statement}: {found:?}");
    }
}

#[test]
fn a_command_that_writes_files_is_reported_with_or_without_a_path_and_a_space() {
    for program in ["cp", "mv", "ln", "rm", "tee", "touch", "truncate", "dd", "install", "mkdir", "sh", "bash", "/bin/cp", "/usr/bin/touch"] {
        let source = format!("pub fn run() {{\n    let _ = std::process::Command::new(\"{program}\").status();\n}}\n");
        let found = scan(&[("lib.rs", &source)]);
        assert!(!found.is_empty(), "not reported: Command::new(\"{program}\")");
    }
}

#[test]
fn a_command_that_does_not_write_files_is_not_reported() {
    for program in ["ps", "osascript", "notify-send", "/bin/kill", "date"] {
        let source = format!("pub fn run() {{\n    let _ = std::process::Command::new(\"{program}\").output();\n}}\n");
        let found = scan(&[("lib.rs", &source)]);
        assert!(found.is_empty(), "reported: Command::new(\"{program}\"): {found:?}");
    }
}

#[test]
fn every_shell_redirect_form_in_a_string_is_reported() {
    for command in [
        "echo hi>out",
        "cmd 3> out",
        "cmd &> out",
        "> out",
        "cmd >> out",
        "cmd 2>>out",
        "cmd | tee out",
        "truncate -s0 out",
        "echo hi > out",
        "cmd 2>/dev/null > out",
    ] {
        let found = scan(&[("lib.rs", &format!("pub const C: &str = \"{command}\";\n"))]);
        assert!(!found.is_empty(), "not reported: {command}");
    }
}

#[test]
fn text_with_an_arrow_an_angle_pair_a_character_or_a_write_to_dev_null_is_not_taken_for_a_redirect() {
    for text in
        ["a -> b", "x => y", "<b>bold</b>", "Vec<u8>", "Option<Box<dyn Fn()>>", "cmd 2>/dev/null", "cat > /dev/null", "cmd 2>&1", "pop-rm-action", "grid-mv-1"]
    {
        let found = scan(&[("lib.rs", &format!("pub const C: &str = \"{text}\";\n"))]);
        assert!(found.is_empty(), "reported: {text}: {found:?}");
    }
    let found = scan(&[("lib.rs", "pub fn closes(c: char) -> bool {\n    c == '>'\n}\n")]);
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn the_names_of_the_image_job_and_plugin_files_are_session_file_names() {
    for name in ["input.png", "output.png", "request.json", "mask.png", "plugin.json", "mcp_config.json", ".grok/config.toml"] {
        let found = scan(&[("lib.rs", &format!("pub fn p(base: &std::path::Path) -> std::path::PathBuf {{\n    base.join(\"{name}\")\n}}\n"))]);
        assert!(!found.is_empty(), "not reported: {name}");
    }
}

#[test]
fn a_name_joined_with_a_format_to_a_session_path_must_be_a_known_session_file_name() {
    let source = "pub fn p(session_dir: &std::path::Path, n: u8) -> std::path::PathBuf {\n    session_dir.join(&format!(\"novel-{n}.dat\"))\n}\n";
    let found = scan(&[("lib.rs", source)]);
    assert!(flagged(&found, "SESSION_NAMES has no name for it"), "{found:?}");
}

#[test]
fn a_name_joined_to_a_job_or_plugin_folder_or_after_another_join_must_be_a_known_session_file_name() {
    for source in [
        "pub fn p(job_dir: &std::path::Path) -> std::path::PathBuf {\n    job_dir.join(\"novel.dat\")\n}\n",
        "pub fn p(plugin_dir: &std::path::Path) -> std::path::PathBuf {\n    plugin_dir.join(\"novel.dat\")\n}\n",
        "pub fn p(session_dir: &std::path::Path) -> std::path::PathBuf {\n    session_dir.join(\".gemini\").join(\"novel.dat\")\n}\n",
        "pub fn p(session_log: &std::path::Path) -> std::path::PathBuf {\n    session_log.with_file_name(\"novel.dat\")\n}\n",
    ] {
        let found = scan(&[("lib.rs", source)]);
        assert!(flagged(&found, "SESSION_NAMES has no name for it"), "not reported: {source}: {found:?}");
    }
}

#[test]
fn a_name_joined_to_a_path_that_is_no_session_folder_is_not_reported() {
    let found = scan(&[("lib.rs", "pub fn p(base: &std::path::Path) -> std::path::PathBuf {\n    base.join(\"novel.dat\")\n}\n")]);
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_name_joined_in_the_private_channel_folder_or_below_a_known_subfolder_is_not_reported() {
    for source in [
        "pub fn p(session_dir: &std::path::Path) -> Option<std::path::PathBuf> {\n    channel_dir(session_dir).map(|dir| dir.join(\"novel.dat\"))\n}\n",
        "pub fn p(session_dir: &std::path::Path) -> std::path::PathBuf {\n    session_dir.join(\".agents\").join(\"plugins\").join(\"designos\")\n}\n",
    ] {
        let found = scan(&[("lib.rs", source)]);
        assert!(!flagged(&found, "SESSION_NAMES has no name for it"), "reported: {source}: {found:?}");
    }
}
