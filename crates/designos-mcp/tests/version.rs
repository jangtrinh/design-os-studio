//! `designos-mcp --version` / `-V` prints the release version, and `--help` lists the flag.
use std::process::Command;

/// Runs the binary in an empty scratch directory, so an argument read as a path can never touch the checkout.
/// Each call gets its own directory: the tests run in parallel, and a shared one was removed under a running test.
fn run(args: &[&str]) -> (bool, String, String) {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("designos_mcp_version_{}_{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create the scratch directory");
    let out = Command::new(env!("CARGO_BIN_EXE_designos-mcp")).args(args).current_dir(&dir).output().expect("run designos-mcp");
    let _ = std::fs::remove_dir_all(&dir);
    (out.status.success(), String::from_utf8(out.stdout).expect("stdout is UTF-8"), String::from_utf8(out.stderr).expect("stderr is UTF-8"))
}

#[test]
fn version_flags_print_the_cargo_version() {
    for flag in ["--version", "-V"] {
        let (ok, stdout, stderr) = run(&[flag]);
        assert!(ok, "designos-mcp {flag} must exit 0\nstderr: {stderr}");
        assert_eq!(stdout, format!("designos-mcp {}\n", env!("CARGO_PKG_VERSION")), "designos-mcp {flag}");
        assert!(stderr.is_empty(), "designos-mcp {flag} writes nothing to stderr: {stderr}");
    }
}

#[test]
fn help_lists_the_version_flag() {
    let (ok, stdout, stderr) = run(&["--help"]);
    assert!(ok, "designos-mcp --help must exit 0");
    assert!(format!("{stdout}{stderr}").contains("--version"), "--help must list --version:\nstdout: {stdout}\nstderr: {stderr}");
}

#[test]
fn version_is_not_mistaken_for_a_workspace_argument() {
    let (ok, stdout, _) = run(&["--workspace", "--version"]);
    assert!(!ok, "`--workspace --version` names a directory called --version and must not print a version");
    assert!(stdout.is_empty(), "nothing is printed on stdout: {stdout}");
}
