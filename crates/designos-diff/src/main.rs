use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use designos_diff::{compare, diff_image, suite, DiffOptions, Rect};

#[derive(Parser)]
#[command(name = "designos-diff", version, about = "Pixel diff harness")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Compare two images; exits 1 when the mismatch ratio exceeds the threshold.
    Compare {
        a: PathBuf,
        b: PathBuf,
        /// Max per-channel delta (0..=255) treated as equal.
        #[arg(long, default_value_t = 0)]
        tolerance: u8,
        /// Max allowed mismatched-pixel ratio (0..=1).
        #[arg(long, default_value_t = 0.0)]
        threshold: f64,
        /// Write a diff visualisation PNG here.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Ignore region `x,y,w,h` (repeatable).
        #[arg(long, value_parser = Rect::parse)]
        ignore: Vec<Rect>,
        /// Do not forgive anti-aliased pixels.
        #[arg(long)]
        no_aa: bool,
        /// Print the report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Run all cases of a manifest and write an HTML report.
    Suite {
        manifest: PathBuf,
        /// Report directory (default: `diff-report` next to the manifest).
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Default per-channel tolerance for cases that do not set one.
        #[arg(long, default_value_t = 0)]
        tolerance: u8,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}

fn run(cli: Cli) -> Result<bool, String> {
    match cli.command {
        Command::Compare { a, b, tolerance, threshold, out, ignore, no_aa, json } => {
            let load = |p: &PathBuf| image::open(p).map(|i| i.to_rgba8()).map_err(|e| format!("{}: {e}", p.display()));
            let (ia, ib) = (load(&a)?, load(&b)?);
            let opts = DiffOptions { tolerance, detect_aa: !no_aa, ignore };
            let report = if let Some(out) = out {
                let (report, img) = diff_image(&ia, &ib, &opts);
                img.save(&out).map_err(|e| format!("{}: {e}", out.display()))?;
                report
            } else {
                compare(&ia, &ib, &opts)
            };
            let pass = report.passes(threshold);
            if json {
                let mut v = serde_json::to_value(&report).map_err(|e| e.to_string())?;
                v["threshold"] = threshold.into();
                v["pass"] = pass.into();
                println!("{v}");
            } else {
                println!(
                    "{} ratio={:.6} threshold={:.6} mismatched={} aa={} ignored={} ssim={:.4} size={}x{}{}",
                    if pass { "PASS" } else { "FAIL" },
                    report.ratio,
                    threshold,
                    report.mismatched,
                    report.aa_pixels,
                    report.ignored,
                    report.ssim,
                    report.width,
                    report.height,
                    report.size_note().map(|n| format!(" ({n})")).unwrap_or_default()
                );
            }
            if let Some(message) = report.size_mismatch_message() {
                eprintln!("{message}. Render both at the same size.");
            }
            Ok(pass)
        }
        Command::Suite { manifest, out_dir, tolerance } => {
            let out_dir = out_dir.unwrap_or_else(|| manifest.parent().unwrap_or(std::path::Path::new("")).join("diff-report"));
            let results = suite::run(&manifest, &out_dir, tolerance)?;
            print!("{}", suite::table(&results));
            println!("report: {}", out_dir.join("index.html").display());
            Ok(results.iter().all(|r| r.passed))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::error::ErrorKind;

    #[test]
    fn parses_version_flags_and_formats_like_other_binaries() {
        let expected = format!("designos-diff {}\n", env!("CARGO_PKG_VERSION"));
        for flag in ["--version", "-V"] {
            let err = match Cli::try_parse_from(["designos-diff", flag]) {
                Err(e) => e,
                Ok(_) => panic!("{flag} should exit with DisplayVersion"),
            };
            assert_eq!(err.kind(), ErrorKind::DisplayVersion, "{flag}: {err}");
            assert_eq!(err.to_string(), expected, "{flag}");
        }
    }
}
