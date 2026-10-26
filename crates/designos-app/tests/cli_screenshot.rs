//! `--screenshot` refuses broken files with one plain line on stderr without writing a PNG,
//! and captures a valid file (`#1003` RT-04).

use std::{
    path::Path,
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};

const BINARIES: [(&str, &str); 2] = [("designos", env!("CARGO_BIN_EXE_designos")), ("designos-studio", env!("CARGO_BIN_EXE_designos-studio"))];

fn run_cli_bin(exe: &str, cwd: &Path, args: &[&str]) -> Output {
    let mut child = Command::new(exe)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("start {exe}: {e}"));
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if child.try_wait().expect("poll child").is_some() {
            return child.wait_with_output().expect("collect output");
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{exe} {args:?} did not exit within 30 s");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn cli_screenshot_refuses_broken_files_and_captures_good_file() {
    let dir = std::env::temp_dir().join(format!("designos_rt04_cli_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let mut good_doc = designos_doc::Document::new("Good");
    good_doc.add_page("Page 1");
    std::fs::write(dir.join("good.designos"), good_doc.to_json().expect("serialize good.designos")).unwrap();
    let fig_bytes = designos_fig::export_fig(&good_doc).expect("export valid .fig");

    std::fs::write(dir.join("trunc.fig"), &fig_bytes[..24.min(fig_bytes.len())]).unwrap();
    std::fs::write(dir.join("corrupt.designos"), b"{corrupt designos json").unwrap();
    std::fs::write(dir.join("garbage.fig"), b"not a fig file at all").unwrap();

    for (bin_name, bin_path) in BINARIES {
        for broken in ["trunc.fig", "corrupt.designos", "garbage.fig"] {
            let png_name = format!("{bin_name}_{broken}.png");
            let png_path = dir.join(&png_name);
            let _ = std::fs::remove_file(&png_path);
            let out = run_cli_bin(bin_path, &dir, &[broken, "--screenshot", &png_name]);
            let stderr = String::from_utf8_lossy(&out.stderr);
            assert!(!out.status.success(), "{bin_name} {broken} --screenshot must exit non-zero, got {:?}\nstderr: {stderr}", out.status);
            assert!(!png_path.exists(), "{bin_name} {broken} --screenshot must not write {png_name}");
            let lines: Vec<&str> = stderr.lines().collect();
            assert_eq!(lines.len(), 1, "{bin_name} {broken} must print exactly one line to stderr, got: {stderr:?}");
            assert!(
                lines[0].starts_with(&format!("{broken} could not be opened: ")),
                "{bin_name} {broken} stderr must start with '{broken} could not be opened: ', got: {:?}",
                lines[0]
            );
            assert!(
                lines[0].ends_with(". Check the file, then try again."),
                "{bin_name} {broken} stderr must end with '. Check the file, then try again.', got: {:?}",
                lines[0]
            );
        }
    }

    let good_png = dir.join("good.png");
    let (_, designos_bin) = BINARIES[0];
    let mut out = run_cli_bin(designos_bin, &dir, &["good.designos", "--screenshot", "good.png"]);
    for _ in 0..2 {
        if out.status.success() || !String::from_utf8_lossy(&out.stderr).contains("screenshot was not captured") {
            break;
        }
        out = run_cli_bin(designos_bin, &dir, &["good.designos", "--screenshot", "good.png"]);
    }
    assert!(out.status.success(), "control run on good.designos must exit 0, got {:?}\nstderr: {}", out.status, String::from_utf8_lossy(&out.stderr));
    assert!(good_png.metadata().is_ok_and(|m| m.len() > 0), "control run on good.designos must write non-empty good.png");

    let _ = std::fs::remove_dir_all(&dir);
}
