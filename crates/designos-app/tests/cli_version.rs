//! The `designos` and `designos-studio` binaries answer `--version` / `-V` and refuse a file that does not exist,
//! all before any window opens.
use std::{
    path::Path,
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};

const BINARIES: [(&str, &str); 2] = [("designos", env!("CARGO_BIN_EXE_designos")), ("designos-studio", env!("CARGO_BIN_EXE_designos-studio"))];

/// Runs `exe` in `cwd`. A binary that wrongly opens a window would never exit, so it is killed after the
/// deadline and the test fails instead of hanging.
fn run(exe: &str, cwd: &Path, args: &[&str]) -> Output {
    let mut child = Command::new(exe)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("start {exe}: {e}"));
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if child.try_wait().expect("poll the child").is_some() {
            return child.wait_with_output().expect("collect the output");
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{exe} {args:?} did not exit within 15 s (it probably opened a window)");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn empty_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("designos_cli_version_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the scratch directory");
    dir
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("output is UTF-8")
}

#[test]
fn version_flags_print_the_cargo_version() {
    let dir = empty_dir("version");
    for (name, exe) in BINARIES {
        for flag in ["--version", "-V"] {
            let out = run(exe, &dir, &[flag]);
            assert!(out.status.success(), "{name} {flag} must exit 0, got {:?}\nstderr: {}", out.status, text(&out.stderr));
            assert_eq!(text(&out.stdout), format!("{name} {}\n", env!("CARGO_PKG_VERSION")), "{name} {flag}");
            assert!(out.stderr.is_empty(), "{name} {flag} writes nothing to stderr: {}", text(&out.stderr));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn version_wins_over_a_file_argument() {
    let dir = empty_dir("version_file");
    for (name, exe) in BINARIES {
        let out = run(exe, &dir, &["doctor", "--version"]);
        assert!(out.status.success(), "{name} doctor --version: {:?}", out.status);
        assert_eq!(text(&out.stdout), format!("{name} {}\n", env!("CARGO_PKG_VERSION")));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn help_lists_the_version_flag() {
    let dir = empty_dir("help");
    for (name, exe) in BINARIES {
        for flag in ["--help", "-h"] {
            let out = run(exe, &dir, &[flag]);
            assert!(out.status.success(), "{name} {flag}: {:?}", out.status);
            let help = text(&out.stdout);
            assert!(help.contains("--version"), "{name} {flag} must list --version:\n{help}");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_file_exits_non_zero_with_one_plain_line() {
    let dir = empty_dir("missing");
    for (name, exe) in BINARIES {
        let out = run(exe, &dir, &["doctor"]);
        assert!(!out.status.success(), "{name} doctor must fail, got {:?}", out.status);
        assert_eq!(text(&out.stderr), "No file named doctor. Run designos --help.\n", "{name}");
        assert!(out.stdout.is_empty(), "{name} prints nothing on stdout: {}", text(&out.stdout));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_file_is_refused_in_screenshot_and_agent_modes_too() {
    let dir = empty_dir("missing_modes");
    let (name, exe) = BINARIES[0];
    for args in [&["gone.designos", "--screenshot", "out.png"][..], &["gone.designos", "--inspect-json"][..]] {
        let out = run(exe, &dir, args);
        assert!(!out.status.success(), "{name} {args:?} must fail");
        assert_eq!(text(&out.stderr), "No file named gone.designos. Run designos --help.\n", "{name} {args:?}");
    }
    assert!(!dir.join("out.png").exists(), "no screenshot is written for a missing file");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_agent_command_that_creates_the_file_still_may() {
    let dir = empty_dir("creates");
    let (name, exe) = BINARIES[0];
    let spec = dir.join("spec.json");
    std::fs::write(&spec, r#"{"preset":"easeui-card","name":"Card"}"#).expect("write the spec");
    let out = run(exe, &dir, &["fresh.designos", "--apply-spec", "spec.json"]);
    assert!(out.status.success(), "{name} --apply-spec on a new file: {:?}\nstderr: {}", out.status, text(&out.stderr));
    assert!(dir.join("fresh.designos").exists(), "the file is created");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_existing_file_is_not_called_missing() {
    let dir = empty_dir("existing");
    let (name, exe) = BINARIES[0];
    let mut doc = designos_doc::Document::new("Existing");
    doc.add_page("Page 1");
    std::fs::write(dir.join("here.designos"), doc.to_json().expect("serialize the document")).expect("write the document");
    let out = run(exe, &dir, &["here.designos", "--inspect-json"]);
    assert!(out.status.success(), "{name} --inspect-json on an existing file: {:?}\nstderr: {}", out.status, text(&out.stderr));
    assert!(text(&out.stdout).contains("\"node_count\""), "the inspection ran: {}", text(&out.stdout));
    let _ = std::fs::remove_dir_all(&dir);
}
