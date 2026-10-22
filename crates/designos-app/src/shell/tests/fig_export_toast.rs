//! The "Show in folder" toast raised after an export: it fits the window, its action reveals the exported file,
//! and a reveal that fails says so.

use super::*;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Every path the toast action asked to reveal. Tests share it, so each asserts on its own unique path.
static REVEALED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

fn record_reveal(path: &Path) -> Result<(), String> {
    REVEALED.lock().unwrap().push(path.to_path_buf());
    Ok(())
}

fn refuse_reveal(path: &Path) -> Result<(), String> {
    Err(format!("Cannot open {}", path.display()))
}

fn revealed(path: &Path) -> bool {
    REVEALED.lock().unwrap().iter().any(|p| p == path)
}

/// A path that cannot fit one line of a 1600 px window.
fn long_export_path(tag: &str) -> PathBuf {
    let mut p = PathBuf::from("/Users/someone/Library/Application Support/designos/library/exports");
    for i in 0..12 {
        p.push(format!("a-deliberately-long-folder-name-{i}"));
    }
    p.join(format!("{tag}.fig"))
}

/// Raises the success toast of an export and lets the shell paint it.
fn raise(s: &mut Sim, path: &Path) -> String {
    let text = format!("Exported .fig to {}", path.display());
    s.shell.app.show_toast_with_reveal(text.clone(), path.to_path_buf());
    s.frame(vec![]);
    s.frame(vec![]);
    text
}

#[test]
fn a_long_path_toast_stays_inside_the_window() {
    let mut s = Sim::new();
    let path = long_export_path("fits");
    let text = raise(&mut s, &path);
    let button = s.rect_of("toast-reveal");
    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 1088.0));
    assert!(screen.contains_rect(button), "the action must be on screen, got {button:?}");
    let painted: Vec<Rect> = s
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == text => Some(t.visual_bounding_rect()),
            _ => None,
        })
        .collect();
    assert_eq!(painted.len(), 1, "the full text is painted once");
    assert!(screen.contains_rect(painted[0]), "the full path must be on screen, got {:?}", painted[0]);
}

#[test]
fn show_in_folder_reveals_the_exported_file() {
    let mut s = Sim::new();
    s.shell.app.reveal_file = record_reveal;
    let path = long_export_path("reveals");
    raise(&mut s, &path);
    assert!(!revealed(&path), "precondition: nothing revealed before the click");

    let button = s.rect_of("toast-reveal");
    assert!(button.height() >= 24.0, "pointer target is at least 24 px, got {}", button.height());
    s.click(button.center());

    assert!(revealed(&path), "clicking Show in folder reveals the exported file");
}

#[test]
fn a_failed_reveal_says_so() {
    let mut s = Sim::new();
    s.shell.app.reveal_file = refuse_reveal;
    let path = long_export_path("refused");
    raise(&mut s, &path);

    let button = s.rect_of("toast-reveal");
    s.click(button.center());

    let toast = s.app().toast.as_ref().expect("the failure is shown");
    assert!(toast.error && toast.text.starts_with("Cannot open"), "unexpected toast: {} (error: {})", toast.text, toast.error);
}
