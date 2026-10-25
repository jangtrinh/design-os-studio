//! Manifest-driven suites of reference/actual comparisons plus an HTML report.

use std::fs;
use std::path::{Path, PathBuf};

use image::RgbaImage;
use serde::{Deserialize, Serialize};

use crate::{crop, diff_image, downscale, DiffOptions, DiffReport, Rect};

/// One comparison. Paths are relative to the manifest file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Case {
    pub name: String,
    pub reference: PathBuf,
    pub actual: PathBuf,
    /// Max allowed mismatch ratio.
    pub threshold: f64,
    /// Crop both images to this region before comparing.
    #[serde(default)]
    pub region: Option<Rect>,
    /// Ignore rects, in the coordinate space of the (cropped) images.
    #[serde(default)]
    pub ignore: Vec<Rect>,
    /// Per-channel tolerance override.
    #[serde(default)]
    pub tolerance: Option<u8>,
    /// Integer device pixel ratio of the reference screenshot (e.g. 2).
    #[serde(default)]
    pub dpr: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ManifestFile {
    Object { cases: Vec<Case> },
    List(Vec<Case>),
}

pub fn load_manifest(path: &Path) -> Result<Vec<Case>, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let m: ManifestFile = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(match m {
        ManifestFile::Object { cases } => cases,
        ManifestFile::List(cases) => cases,
    })
}

#[derive(Debug, Clone)]
pub struct CaseResult {
    pub name: String,
    pub threshold: f64,
    pub passed: bool,
    pub report: Option<DiffReport>,
    pub error: Option<String>,
    /// Directory name (inside the report dir) holding reference/actual/diff PNGs.
    pub asset_dir: Option<String>,
}

fn load(path: &Path) -> Result<RgbaImage, String> {
    image::open(path).map(|i| i.to_rgba8()).map_err(|e| format!("{}: {e}", path.display()))
}

fn safe_name(name: &str, index: usize) -> String {
    let s: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    format!("{index:03}-{s}")
}

fn run_case(case: &Case, base: &Path, out_dir: &Path, index: usize, default_tolerance: u8) -> Result<(DiffReport, String), String> {
    let mut reference = load(&base.join(&case.reference))?;
    let mut actual = load(&base.join(&case.actual))?;
    if let Some(dpr) = case.dpr {
        reference = downscale(&reference, dpr);
    }
    if let Some(r) = case.region {
        reference = crop(&reference, r);
        actual = crop(&actual, r);
    }
    let opts = DiffOptions { tolerance: case.tolerance.unwrap_or(default_tolerance), detect_aa: true, ignore: case.ignore.clone() };
    let (report, diff) = diff_image(&reference, &actual, &opts);
    let dir_name = safe_name(&case.name, index);
    let dir = out_dir.join(&dir_name);
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for (file, img) in [("reference.png", &reference), ("actual.png", &actual), ("diff.png", &diff)] {
        img.save(dir.join(file)).map_err(|e| format!("{}: {e}", dir.join(file).display()))?;
    }
    Ok((report, dir_name))
}

/// Run every case, write PNG assets and `index.html` into `out_dir`, and return
/// the results sorted worst first (errors, then descending ratio).
pub fn run(manifest: &Path, out_dir: &Path, default_tolerance: u8) -> Result<Vec<CaseResult>, String> {
    let cases = load_manifest(manifest)?;
    let base = manifest.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let mut results: Vec<CaseResult> = cases
        .iter()
        .enumerate()
        .map(|(i, case)| match run_case(case, base, out_dir, i, default_tolerance) {
            Ok((report, dir)) => CaseResult {
                name: case.name.clone(),
                threshold: case.threshold,
                passed: report.passes(case.threshold),
                report: Some(report),
                error: None,
                asset_dir: Some(dir),
            },
            Err(e) => CaseResult { name: case.name.clone(), threshold: case.threshold, passed: false, report: None, error: Some(e), asset_dir: None },
        })
        .collect();
    results.sort_by(|a, b| {
        let key = |r: &CaseResult| r.report.as_ref().map_or(f64::INFINITY, |d| d.ratio);
        key(b).partial_cmp(&key(a)).unwrap_or(std::cmp::Ordering::Equal)
    });
    fs::write(out_dir.join("index.html"), html(&results)).map_err(|e| format!("{}: {e}", out_dir.join("index.html").display()))?;
    Ok(results)
}

/// Plain-text table of results.
pub fn table(results: &[CaseResult]) -> String {
    let width = results.iter().map(|r| r.name.len()).max().unwrap_or(4).max(4);
    let mut out = format!("{:<width$}  {:>9}  {:>9}  {:>6}  {}\n", "case", "ratio", "threshold", "ssim", "status");
    for r in results {
        match (&r.report, &r.error) {
            (Some(d), _) => out.push_str(&format!(
                "{:<width$}  {:>9.6}  {:>9.6}  {:>6.4}  {}{}\n",
                r.name,
                d.ratio,
                r.threshold,
                d.ssim,
                if r.passed { "PASS" } else { "FAIL" },
                d.size_note().map(|n| format!(" ({n})")).unwrap_or_default(),
            )),
            (None, e) => out.push_str(&format!("{:<width$}  {:>9}  {:>9.6}  {:>6}  ERROR {}\n", r.name, "-", r.threshold, "-", e.as_deref().unwrap_or(""))),
        }
    }
    let failed = results.iter().filter(|r| !r.passed).count();
    out.push_str(&format!("{} cases, {} failed\n", results.len(), failed));
    out
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn html(results: &[CaseResult]) -> String {
    let failed = results.iter().filter(|r| !r.passed).count();
    let mut h = String::from(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>designos diff report</title>\
<style>body{font:14px system-ui,sans-serif;margin:24px;background:#fafafa;color:#111}\
.case{background:#fff;border:1px solid #ddd;border-radius:6px;margin:0 0 20px;padding:12px}\
.case.fail{border-color:#e33}.pass-tag{color:#1a7f37}.fail-tag{color:#d11;font-weight:600}\
.row{display:flex;gap:12px;overflow-x:auto}.row figure{margin:0}\
.row img{max-width:480px;border:1px solid #ccc;background:\
repeating-conic-gradient(#eee 0 25%,#fff 0 50%) 0 0/16px 16px}\
figcaption{font-size:12px;color:#555}</style></head><body>",
    );
    h.push_str(&format!("<h1>designos diff report</h1><p>{} cases, {} failed</p>", results.len(), failed));
    for r in results {
        h.push_str(&format!("<div class=\"case{}\"><h2>{} ", if r.passed { "" } else { " fail" }, esc(&r.name)));
        h.push_str(if r.passed { "<span class=\"pass-tag\">PASS</span>" } else { "<span class=\"fail-tag\">FAIL</span>" });
        h.push_str("</h2>");
        if let Some(d) = &r.report {
            h.push_str(&format!(
                "<p>ratio {:.6} (threshold {:.6}) &middot; mismatched {} &middot; AA {} &middot; ssim {:.4} &middot; {}x{}{}</p>",
                d.ratio,
                r.threshold,
                d.mismatched,
                d.aa_pixels,
                d.ssim,
                d.width,
                d.height,
                if d.size_mismatch {
                    format!(" &middot; size mismatch ({}x{} vs {}x{})", d.size_a.0, d.size_a.1, d.size_b.0, d.size_b.1)
                } else {
                    String::new()
                }
            ));
        }
        if let Some(e) = &r.error {
            h.push_str(&format!("<p class=\"fail-tag\">{}</p>", esc(e)));
        }
        if let Some(dir) = &r.asset_dir {
            h.push_str("<div class=\"row\">");
            for (file, label) in [("reference.png", "reference"), ("actual.png", "actual"), ("diff.png", "diff")] {
                h.push_str(&format!("<figure><img src=\"{0}/{1}\" alt=\"{2}\"><figcaption>{2}</figcaption></figure>", esc(dir), file, label));
            }
            h.push_str("</div>");
        }
        h.push_str("</div>");
    }
    h.push_str("</body></html>");
    h
}
