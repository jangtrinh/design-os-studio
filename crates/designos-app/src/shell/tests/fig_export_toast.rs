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

/// The Home screen over a fresh library, the place the tester ran Export .fig.
fn home_sim(tag: &str) -> (Sim, PathBuf) {
    let root = std::env::temp_dir().join(format!("designos_fig_export_toast_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let s = Sim::with(Options { home: true, library: Some(root.clone()), ..Default::default() });
    (s, root)
}

#[test]
fn show_in_folder_works_on_the_home_screen_after_an_export() {
    let (mut s, root) = home_sim("home");
    s.shell.app.reveal_file = record_reveal;
    let lib = s.app().lib.clone().expect("a library");
    let project = lib.list_projects()[0].id.clone();
    let entry = lib.create_file(&project, None, "Home Reveal Case", &designos_doc::Document::new("Home Reveal Case")).unwrap();
    assert!(s.app().home.is_some(), "precondition: the Home screen is showing");

    crate::home::apply(&mut s.shell.app, crate::home::Act::ExportFig(entry.id));
    s.frame(vec![]);
    s.frame(vec![]);

    let exported = root.join("exports").join("Home Reveal Case.fig");
    assert!(exported.is_file(), "the export is in the library exports folder");
    assert!(!revealed(&exported), "precondition: nothing revealed before the click");
    let button = s.rect_of("toast-reveal");
    s.click(button.center());
    assert!(revealed(&exported), "clicking Show in folder on Home reveals the exported file");
    let _ = std::fs::remove_dir_all(&root);
}

/// The text and the action of an export toast are painted; its body is the rest of the toast, left of the button.
fn toast_body_point(s: &Sim, text: &str) -> Pos2 {
    let bounds = s
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == text => Some(t.visual_bounding_rect()),
            _ => None,
        })
        .expect("the toast text is painted");
    Pos2::new(bounds.left() + 12.0, bounds.center().y)
}

#[test]
fn a_click_on_the_toast_body_reaches_the_canvas_below() {
    let mut s = Sim::new();
    // One shape that covers everything the toast can sit on.
    let page = s.app().page;
    let id = s.shell.app.doc.alloc_id();
    let mut shape = designos_doc::Node::rectangle(id, designos_doc::Rect::new(-5000.0, -5000.0, 10000.0, 10000.0));
    shape.fills = vec![designos_doc::Paint::solid(designos_doc::Color::rgb(0x40, 0x90, 0xF0))];
    s.shell.app.doc.insert(page, shape).unwrap();
    s.shell.app.changed();
    s.frame(vec![]);
    let path = long_export_path("click-through");
    let text = raise(&mut s, &path);
    let body = toast_body_point(&s, &text);

    // Control: with no toast, the same click selects the shape.
    s.shell.app.toast = None;
    s.frame(vec![]);
    s.frame(vec![]);
    s.click(body);
    assert_eq!(s.app().selection, vec![id], "precondition: the point is on the shape");
    s.shell.app.clear_selection();
    s.frame(vec![]);
    assert!(s.app().selection.is_empty(), "precondition: nothing is selected");

    raise(&mut s, &path);
    s.click(body);

    assert_eq!(s.app().selection, vec![id], "a click on the toast body selects the layer under it");
}

#[test]
fn a_click_on_a_plain_toast_reaches_the_canvas_below() {
    let mut s = Sim::new();
    let page = s.app().page;
    let id = s.shell.app.doc.alloc_id();
    let mut shape = designos_doc::Node::rectangle(id, designos_doc::Rect::new(-5000.0, -5000.0, 10000.0, 10000.0));
    shape.fills = vec![designos_doc::Paint::solid(designos_doc::Color::rgb(0x40, 0x90, 0xF0))];
    s.shell.app.doc.insert(page, shape).unwrap();
    s.shell.app.changed();
    s.frame(vec![]);
    s.shell.app.show_toast("Copied properties", false);
    s.frame(vec![]);
    s.frame(vec![]);
    let body = toast_body_point(&s, "Copied properties");
    s.click(body);
    assert_eq!(s.app().selection, vec![id], "a click on a plain toast selects the layer under it");
}
