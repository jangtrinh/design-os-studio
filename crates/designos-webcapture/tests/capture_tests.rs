use designos_webcapture::{Capture, CaptureError, Element, CAPTURE_JS, CAPTURE_SCHEMA_ID, CAPTURE_VERSION};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// Checked-in samples, refreshed with `node run_capture_tests.mjs --write-fixtures`. They back the
/// schema-compatibility test and the explicit opt-out below; behaviour is asserted against live
/// captures written by the Playwright harness.
const FIXTURES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/playwright/fixtures");

/// Set to `1` to run without Node and Playwright. Prints a skip notice and falls back to the
/// checked-in samples. CI never sets it.
const SKIP_ENV: &str = "DESIGNOS_WEBCAPTURE_SKIP_HARNESS";

struct LiveRun {
    dir: PathBuf,
    stdout: String,
}

/// Runs the Playwright harness once per test binary and keeps every live capture it wrote.
/// `Ok(None)` means the explicit opt-out is active. Any other problem (no Node, no Playwright, no
/// browser, a failing case, a script `TypeError`) is an `Err` and fails every dependent test.
fn live_run() -> Result<Option<&'static LiveRun>, String> {
    static LIVE: OnceLock<Result<Option<LiveRun>, String>> = OnceLock::new();
    LIVE.get_or_init(run_harness).as_ref().map(Option::as_ref).map_err(Clone::clone)
}

fn run_harness() -> Result<Option<LiveRun>, String> {
    if std::env::var(SKIP_ENV).as_deref() == Ok("1") {
        eprintln!("SKIPPED: {SKIP_ENV}=1, web capture tests use the checked-in samples, not the live script");
        return Ok(None);
    }
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/playwright/run_capture_tests.mjs");
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("webcapture-live");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;

    let mut node_path: Vec<PathBuf> = std::env::var_os("NODE_PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        let user_modules = PathBuf::from(home).join("node_modules");
        if user_modules.is_dir() {
            node_path.push(user_modules);
        }
    }
    let node_path: OsString = std::env::join_paths(node_path).map_err(|e| format!("NODE_PATH: {e}"))?;

    let output = Command::new("node")
        .arg(&script)
        .arg("--out-dir")
        .arg(&dir)
        .env("NODE_PATH", node_path)
        .output()
        .map_err(|e| format!("cannot run `node` ({e}); install Node and Playwright, or set {SKIP_ENV}=1 to skip the live harness"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Playwright harness exited with {}:\n{stdout}\n{stderr}", output.status));
    }
    Ok(Some(LiveRun { dir, stdout }))
}

fn live_or_panic() -> Option<&'static LiveRun> {
    match live_run() {
        Ok(run) => run,
        Err(message) => panic!("{message}"),
    }
}

/// The live capture `name` when the harness ran, else the checked-in sample of the same name.
fn capture_json(name: &str) -> String {
    let path = match live_or_panic() {
        Some(run) => run.dir.join(format!("{name}.capture.json")),
        None => Path::new(FIXTURES_DIR).join(format!("{name}.capture.json")),
    };
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn card_capture_json() -> String {
    capture_json("card")
}

fn index_by_id<'a>(el: &'a Element, out: &mut BTreeMap<String, &'a Element>) {
    if let Some(id) = &el.id {
        out.insert(id.clone(), el);
    }
    for child in &el.children {
        index_by_id(child, out);
    }
}

fn capture_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()))
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with(".capture.json")))
        .collect();
    files.sort();
    files
}

#[test]
fn playwright_harness_passes() {
    let Some(run) = live_or_panic() else { return };
    assert!(run.stdout.contains("\"ok\":true"), "unexpected playwright output: {}", run.stdout);
    assert!(!run.stdout.contains("FAIL "), "harness reported a failing case: {}", run.stdout);
}

#[test]
fn boxes_styles_runs_images() {
    let capture = Capture::from_json(&card_capture_json()).expect("parse card capture");
    assert_eq!(capture.schema, CAPTURE_SCHEMA_ID);
    assert_eq!(capture.version, CAPTURE_VERSION);
    assert_eq!(capture.viewport.width, 1280.0);
    assert_eq!(capture.viewport.height, 800.0);
    assert!(!capture.truncated);

    let mut by_id = BTreeMap::new();
    index_by_id(&capture.root, &mut by_id);

    let card = by_id.get("pricing-card").expect("pricing-card element");
    assert!(card.border_box.width >= 320.0);
    assert!(card.border_box.height > 50.0);
    assert_eq!(card.styles.get("display").map(String::as_str), Some("flex"));
    assert_eq!(card.styles.get("flex-direction").map(String::as_str), Some("column"));
    assert_eq!(card.styles.get("border-top-left-radius").map(String::as_str), Some("12px"));
    let before = card.pseudo.as_ref().and_then(|p| p.before.as_ref()).expect("card ::before pseudo box");
    assert_eq!(before.content, "PRO");
    assert!(before.border_box.width > 0.0);

    let desc = by_id.get("desc").expect("desc paragraph");
    assert!(!desc.text_runs.is_empty());
    assert!(desc.text_runs[0].lines.len() >= 2, "expected wrapped multi-line text rects, got {}", desc.text_runs[0].lines.len());
    for rect in &desc.text_runs[0].lines {
        assert!(rect.width > 0.0 && rect.height > 0.0);
    }

    let avatar = by_id.get("avatar-img").expect("avatar-img");
    let avatar_img = avatar.image.as_ref().expect("avatar ImageRef");
    assert!(!avatar_img.blocked);
    assert!(avatar_img.data_url.as_deref().unwrap_or_default().starts_with("data:image/"));

    let badge = by_id.get("badge-svg").expect("badge-svg");
    let badge_img = badge.image.as_ref().expect("badge ImageRef");
    assert!(badge_img.svg.as_deref().unwrap_or_default().contains("<svg") || badge_img.data_url.as_deref().unwrap_or_default().starts_with("data:image/svg"));

    let spark = by_id.get("spark-canvas").expect("spark-canvas");
    let spark_img = spark.image.as_ref().expect("spark ImageRef");
    assert!(spark_img.data_url.as_deref().unwrap_or_default().starts_with("data:image/png"));

    let bg = by_id.get("bg-banner").expect("bg-banner");
    assert!(bg.image.is_some() || !bg.images.is_empty());

    assert!(!by_id.contains_key("secret-hidden"), "display:none element must be omitted by default");
}

#[test]
fn css_var_refs() {
    let capture = Capture::from_json(&card_capture_json()).expect("parse card capture");
    assert_eq!(capture.root.css_vars_declared.get("--brand").map(String::as_str), Some("#0d99ff"));

    let mut by_id = BTreeMap::new();
    index_by_id(&capture.root, &mut by_id);
    let btn = by_id.get("cta-btn").expect("cta-btn");
    assert_eq!(btn.css_var_refs.get("background-color"), Some(&vec!["--brand".to_string()]));
}

#[test]
fn component_hint() {
    let capture = Capture::from_json(&card_capture_json()).expect("parse card capture");
    let mut by_id = BTreeMap::new();
    index_by_id(&capture.root, &mut by_id);
    let card = by_id.get("pricing-card").expect("pricing-card");
    assert_eq!(card.component_hint.as_deref(), Some("PricingCard"));
}

#[test]
fn cross_origin_image_blocked_recorded() {
    let capture = Capture::from_json(&card_capture_json()).expect("parse card capture");
    let mut by_id = BTreeMap::new();
    index_by_id(&capture.root, &mut by_id);
    let blocked = by_id.get("blocked-img").expect("blocked-img");
    let img = blocked.image.as_ref().expect("blocked ImageRef");
    assert!(img.blocked);
    assert!(img.src.contains("unreachable-cross-origin.png"));
}

#[test]
fn rust_parses_capture() {
    assert!(CAPTURE_JS.contains("__designosCapture"), "embedded CAPTURE_JS must define window.__designosCapture");

    let capture = Capture::from_json(&card_capture_json()).expect("parse card capture");
    let serialized = capture.to_json_pretty().expect("serialize capture");
    let roundtrip = Capture::from_json(&serialized).expect("re-parse serialized capture");
    assert_eq!(capture, roundtrip);

    // Every checked-in sample and every live capture the harness wrote must parse.
    let mut dirs = vec![PathBuf::from(FIXTURES_DIR)];
    if let Some(run) = live_or_panic() {
        dirs.push(run.dir.clone());
    }
    for dir in dirs {
        let files = capture_files(&dir);
        assert!(!files.is_empty(), "no *.capture.json in {}", dir.display());
        for file in files {
            let json = std::fs::read_to_string(&file).expect("read capture");
            Capture::from_json(&json).unwrap_or_else(|e| panic!("{} must parse: {e}", file.display()));
        }
    }

    // Version rejection (`UnsupportedCaptureVersion`)
    let future_json = r#"{"schema":"designos-webcapture","version":99,"root":{"tag":"html"}}"#;
    match Capture::from_json(future_json) {
        Err(CaptureError::UnsupportedCaptureVersion(99)) => {}
        other => panic!("expected UnsupportedCaptureVersion(99), got {other:?}"),
    }

    // Schema rejection (`UnsupportedSchema`)
    let bad_schema = r#"{"schema":"other-tool","version":1,"root":{"tag":"html"}}"#;
    match Capture::from_json(bad_schema) {
        Err(CaptureError::UnsupportedSchema(s)) if s == "other-tool" => {}
        other => panic!("expected UnsupportedSchema, got {other:?}"),
    }
}

#[test]
fn paint_index_beyond_i32_parses() {
    // Captures from the previous script folded `z-index * 1000` into `z`; a page with
    // `z-index: 2147483647` must still load rather than fail the whole capture.
    let json = r#"{"schema":"designos-webcapture","version":1,"root":{"tag":"html","z":2147483647006,"children":[{"tag":"div","z":-7}]}}"#;
    let capture = Capture::from_json(json).expect("a z beyond i32 must parse");
    assert_eq!(capture.root.z, 2_147_483_647_006);
    assert_eq!(capture.root.children[0].z, -7);
}

#[test]
fn truncation() {
    let truncated = Capture::from_json(&capture_json("card.truncated")).expect("parse truncated capture");
    let full = Capture::from_json(&card_capture_json()).expect("parse card capture");
    assert!(truncated.truncated);
    assert_eq!(truncated.stats.elements, 4);
    assert_eq!(truncated.stats.skipped_elements, full.stats.elements - truncated.stats.elements);
}
