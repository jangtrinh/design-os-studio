use std::path::{Path, PathBuf};
use std::process::Command;

use image::{Rgba, RgbaImage};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_designos-diff"))
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("designos-diff-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn save(dir: &Path, name: &str, img: &RgbaImage) -> PathBuf {
    let p = dir.join(name);
    img.save(&p).unwrap();
    p
}

fn white() -> RgbaImage {
    RgbaImage::from_pixel(16, 16, Rgba([255, 255, 255, 255]))
}

fn off() -> RgbaImage {
    let mut b = white();
    b.put_pixel(5, 5, Rgba([0, 0, 0, 255]));
    b
}

#[test]
fn compare_pass_fail_json_and_ignore() {
    let d = tmp("compare");
    let a = save(&d, "a.png", &white());
    let b = save(&d, "b.png", &off());
    let ok = bin().arg("compare").arg(&a).arg(&a).output().unwrap();
    assert!(ok.status.success());
    assert!(String::from_utf8_lossy(&ok.stdout).starts_with("PASS"));

    let diff = d.join("diff.png");
    let bad = bin().arg("compare").arg(&a).arg(&b).arg("--out").arg(&diff).output().unwrap();
    assert_eq!(bad.status.code(), Some(1));
    assert!(diff.exists());

    let lenient = bin().arg("compare").arg(&a).arg(&b).args(["--threshold", "0.01"]).output().unwrap();
    assert!(lenient.status.success());

    let ign = bin().arg("compare").arg(&a).arg(&b).args(["--ignore", "4,4,3,3"]).output().unwrap();
    assert!(ign.status.success());

    let json = bin().arg("compare").arg(&a).arg(&b).arg("--json").output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(v["mismatched"], 1);
    assert_eq!(v["pass"], false);
}

#[test]
fn suite_writes_report_and_fails() {
    let d = tmp("suite");
    save(&d, "ref_ok.png", &white());
    save(&d, "act_ok.png", &white());
    save(&d, "ref_bad.png", &white());
    save(&d, "act_bad.png", &off());
    std::fs::write(
        d.join("manifest.json"),
        r#"{"cases":[
            {"name":"good","reference":"ref_ok.png","actual":"act_ok.png","threshold":0.0},
            {"name":"bad one","reference":"ref_bad.png","actual":"act_bad.png","threshold":0.0}
        ]}"#,
    )
    .unwrap();
    let out = bin().arg("suite").arg(d.join("manifest.json")).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("FAIL") && stdout.contains("PASS"));
    let html = std::fs::read_to_string(d.join("diff-report/index.html")).unwrap();
    assert!(html.find("bad one").unwrap() < html.find("good").unwrap(), "worst first");

    // Passing manifest with a region that excludes the differing pixel.
    std::fs::write(
        d.join("ok.json"),
        r#"[{"name":"cropped","reference":"ref_bad.png","actual":"act_bad.png","threshold":0.0,
             "region":{"x":0,"y":0,"w":4,"h":4}}]"#,
    )
    .unwrap();
    let out = bin().arg("suite").arg(d.join("ok.json")).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
}

/// Gray pixel at (3,3) whose only lighter neighbour (4,4) lies outside the
/// 4x4 crop: the anti-aliasing cross-check reaches past the smaller image.
fn big_and_small_with_edge_candidate() -> (RgbaImage, RgbaImage) {
    let mut big = RgbaImage::from_pixel(8, 8, Rgba([255, 255, 255, 255]));
    big.put_pixel(3, 3, Rgba([128, 128, 128, 255]));
    for (x, y) in [(2, 2), (3, 2), (4, 2), (2, 3), (4, 3), (2, 4), (3, 4)] {
        big.put_pixel(x, y, Rgba([0, 0, 0, 255]));
    }
    let mut small = image::imageops::crop_imm(&big, 0, 0, 4, 4).to_image();
    small.put_pixel(3, 3, Rgba([255, 255, 255, 255]));
    (big, small)
}

#[test]
fn compare_of_two_sizes_names_both_and_exits_non_zero() {
    let d = tmp("size-mismatch");
    let (big, small) = big_and_small_with_edge_candidate();
    let (a, b) = (save(&d, "big.png", &big), save(&d, "small.png", &small));

    for (x, y, message) in [(&a, &b, "Images differ in size: 8\u{d7}8 vs 4\u{d7}4"), (&b, &a, "Images differ in size: 4\u{d7}4 vs 8\u{d7}8")] {
        let out = bin().arg("compare").arg(x).arg(y).output().unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(1), "stderr: {stderr}");
        assert!(stderr.contains(message), "stderr: {stderr}");
        assert!(!stderr.contains("panicked"), "stderr: {stderr}");
        assert!(String::from_utf8_lossy(&out.stdout).starts_with("FAIL"));
    }

    // A lenient threshold never turns a size mismatch into a pass.
    let lenient = bin().arg("compare").arg(&a).arg(&b).args(["--threshold", "1"]).output().unwrap();
    assert_eq!(lenient.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&lenient.stderr).contains("Images differ in size"));
}

#[test]
fn compare_of_two_sizes_still_writes_json_and_the_diff_image() {
    let d = tmp("size-mismatch-outputs");
    let (big, small) = big_and_small_with_edge_candidate();
    let (a, b) = (save(&d, "big.png", &big), save(&d, "small.png", &small));
    let diff = d.join("diff.png");

    let out = bin().arg("compare").arg(&a).arg(&b).arg("--json").arg("--out").arg(&diff).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["size_mismatch"], true);
    assert_eq!(v["pass"], false);
    assert_eq!(v["size_a"], serde_json::json!([8, 8]));
    assert_eq!(v["size_b"], serde_json::json!([4, 4]));
    assert_eq!(image::open(&diff).unwrap().to_rgba8().dimensions(), (8, 8));
    assert!(String::from_utf8_lossy(&out.stderr).contains("Images differ in size: 8\u{d7}8 vs 4\u{d7}4"));
}

#[test]
fn suite_with_two_sizes_reports_the_case_instead_of_panicking() {
    let d = tmp("suite-size-mismatch");
    let (big, small) = big_and_small_with_edge_candidate();
    save(&d, "big.png", &big);
    save(&d, "small.png", &small);
    std::fs::write(
        d.join("manifest.json"),
        r#"{"cases":[
            {"name":"mixed sizes","reference":"big.png","actual":"small.png","threshold":0.0},
            {"name":"mixed sizes, lenient","reference":"big.png","actual":"small.png","threshold":1.0}
        ]}"#,
    )
    .unwrap();

    // Library level: each case carries a report with both sizes, and fails
    // even when its threshold would let the mismatch ratio through.
    let results = designos_diff::suite::run(&d.join("manifest.json"), &d.join("lib-report"), 0).unwrap();
    assert_eq!(results.len(), 2);
    for r in &results {
        let report = r.report.as_ref().expect("a report, not an error");
        assert!(report.size_mismatch && !r.passed, "{}", r.name);
        assert_eq!((report.size_a, report.size_b), ((8, 8), (4, 4)));
    }
    assert!(d.join("lib-report/index.html").exists());

    // CLI level: same verdict, same report on disk, no panic.
    let out = bin().arg("suite").arg(d.join("manifest.json")).output().unwrap();
    assert_eq!(out.status.code(), Some(1), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("size mismatch: 8x8 vs 4x4"));
    let html = std::fs::read_to_string(d.join("diff-report/index.html")).unwrap();
    assert!(html.contains("size mismatch (8x8 vs 4x4)"));
}

#[test]
fn version_flags_print_version_and_exit_zero() {
    let expected = format!("designos-diff {}\n", env!("CARGO_PKG_VERSION"));
    for flag in ["--version", "-V"] {
        let out = bin().arg(flag).output().unwrap();
        assert!(out.status.success(), "{flag} exited {:?}: {}", out.status.code(), String::from_utf8_lossy(&out.stderr));
        assert!(out.stderr.is_empty(), "{flag} stderr: {}", String::from_utf8_lossy(&out.stderr));
        assert_eq!(String::from_utf8_lossy(&out.stdout), expected, "{flag}");
    }
}
