//! The properties-panel export toast: it is painted while that panel is open, and Show in folder
//! reports a file manager that cannot start.

use super::*;
use crate::io::{DialogOutcome, ScriptedDialogs};
use crate::panels::export::{run_export_with, RowEdit};
use designos_doc::{ExportPreset, Node, Rect as DRect};
use egui::{Pos2, Rect, Vec2};

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("designos_export_toast_{}_{name}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn refuse_reveal(path: &std::path::Path) -> Result<(), String> {
    Err(format!("Cannot open the folder: no file manager. The file is at {}", path.display()))
}

/// `--panel export-toast` writes into one directory named with the process id.
fn preset_export_lock() -> std::sync::MutexGuard<'static, ()> {
    static PRESET_EXPORT: std::sync::Mutex<()> = std::sync::Mutex::new(());
    PRESET_EXPORT.lock().unwrap_or_else(|e| e.into_inner())
}

fn painted_text(s: &Sim, text: &str) -> bool {
    s.shapes.iter().any(|c| matches!(&c.shape, egui::Shape::Text(t) if t.galley.text() == text))
}

/// The next frame reads egui's clock (`app.now = i.time`), which a blocked UI thread has moved forward.
fn block_ui(s: &mut Sim, seconds: f64) {
    s.shell.app.now += seconds;
    s.t += seconds;
}

/// One frame whose egui clock lands on `at` (each frame adds the sim's 0.05 s step).
fn frame_at(s: &mut Sim, at: f64) {
    s.t = at - 0.05;
    s.frame(vec![]);
}

#[test]
fn export_toast_panel_toast_is_visible_beside_the_open_properties_panel() {
    let dir = scratch("visible");
    let mut s = Sim::new();
    let page = s.app().page;
    let id = s.shell.app.doc.alloc_id();
    let mut node = Node::rectangle(id, DRect::new(0.0, 0.0, 40.0, 30.0));
    node.name = "Card".into();
    s.shell.app.doc.insert(page, node).unwrap();
    s.shell.app.selection = vec![id];
    s.shell.app.edit_export_rows(&[id], &RowEdit::Add(ExportPreset::default()));
    s.shell.app.reveal_file = refuse_reveal;
    run_export_with(&mut s.shell.app, &[id], &ScriptedDialogs::answering(DialogOutcome::Unavailable).with_fallback(dir.clone()));
    let saved = dir.join("Card.png");
    crate::fig_export::tests::assert_offers_reveal(s.app(), &saved);
    s.frame(vec![]);
    s.frame(vec![]);

    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 1088.0));
    let panel = s.rect_of("right-tab-Design");
    assert!(screen.contains_rect(panel), "the properties panel is open: {panel:?}");
    let text = s.app().toast.as_ref().expect("toast").text.clone();
    let painted: Vec<Rect> = s
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Text(t) if t.galley.text() == text => Some(t.visual_bounding_rect()),
            _ => None,
        })
        .collect();
    assert_eq!(painted.len(), 1, "the full path is painted once");
    assert!(screen.contains_rect(painted[0]), "the path is on screen while the properties panel is open: {:?}", painted[0]);
    let panel_left = screen.right() - theme::RIGHT_PANEL_WIDTH;
    assert!(painted[0].left() < panel_left, "the toast is not tucked only under the properties panel: {:?}", painted[0]);
    let button = s.rect_of("toast-reveal");
    assert!(screen.contains_rect(button), "Show in folder is on screen: {button:?}");
    assert!(button.height() >= 24.0, "pointer target is at least 24 px, got {}", button.height());

    s.click(button.center());
    let toast = s.app().toast.as_ref().expect("a missing file manager is said");
    assert!(toast.error && toast.text.contains("no file manager"), "{}", toast.text);
    assert!(toast.text.contains(&saved.display().to_string()), "{}", toast.text);
    let _ = std::fs::remove_dir_all(&dir);
}

/// `show_toast` / `show_toast_with_reveal`, then 30 s with no frame: the next frame paints the toast,
/// it is still there `seconds - 0.1` after that paint, and gone `seconds + 0.1` after it.
fn toast_clock_runs_from_the_first_paint(raise: impl FnOnce(&mut Sim), seconds: f64) {
    let mut s = Sim::new();
    raise(&mut s);
    let text = s.app().toast.as_ref().expect("the toast is raised before the blocked frame").text.clone();
    block_ui(&mut s, 30.0);
    s.frame(vec![]);
    assert!(painted_text(&s, &text), "the frame after a 30 s block paints the toast, got {:?}", s.app().toast.as_ref().map(|t| &t.text));
    let started = s.app().now;
    frame_at(&mut s, started + seconds - 0.1);
    assert!(painted_text(&s, &text), "still painted {seconds} - 0.1 s after the first paint (now {})", s.app().now);
    frame_at(&mut s, started + seconds + 0.1);
    assert!(s.app().toast.is_none(), "gone {seconds} + 0.1 s after the first paint");
    assert!(!painted_text(&s, &text), "the expired toast is not painted");
}

#[test]
fn toast_clock_reveal_toast_survives_a_30s_block_then_lasts_reveal_seconds() {
    toast_clock_runs_from_the_first_paint(
        |s| {
            let path = std::path::PathBuf::from("/tmp/designos-toast-clock-reveal.png");
            s.shell.app.show_toast_with_reveal(format!("Exported {}", path.display()), path);
        },
        theme::TOAST_REVEAL_SECONDS,
    );
}

#[test]
fn toast_clock_plain_toast_survives_a_30s_block_then_lasts_toast_seconds() {
    toast_clock_runs_from_the_first_paint(|s| s.shell.app.show_toast("Saved the page", false), theme::TOAST_SECONDS);
}

#[test]
fn toast_clock_export_preset_is_painted_on_the_screenshot_frame() {
    let _preset = preset_export_lock();
    let shot = std::env::temp_dir().join(format!("designos_export_toast_shot_{}.png", std::process::id()));
    let opts = crate::cli::parse([
        "--panel".to_string(),
        "export-toast".to_string(),
        "--screenshot".to_string(),
        shot.display().to_string(),
        "--size".to_string(),
        "1600x1088".to_string(),
    ])
    .expect("export-toast screenshot parses");
    let mut s = Sim::built(opts);
    let text = s.app().toast.as_ref().expect("the preset raises the export toast before the first frame").text.clone();
    let reveal_dir = s.app().toast.as_ref().and_then(|t| t.reveal.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf())));
    block_ui(&mut s, 30.0);
    s.frame(vec![]);
    assert!(painted_text(&s, &text), "the first paint after a 30 s load shows the export toast");
    // Frames after that first paint run past the reveal lifetime before the screenshot is asked for.
    block_ui(&mut s, theme::TOAST_REVEAL_SECONDS + 1.0);
    let mut asked = false;
    for _ in 0..30 {
        s.frame(vec![]);
        if s.commands.iter().any(|c| matches!(c, egui::ViewportCommand::Screenshot(_))) {
            asked = true;
            break;
        }
    }
    assert!(asked, "a screenshot frame was reached");
    assert!(painted_text(&s, &text), "the capture frame paints the export toast");
    assert!(s.rect_of("toast-reveal").height() >= 24.0, "Show in folder is on the capture frame");
    if let Some(dir) = reveal_dir {
        let _ = std::fs::remove_dir_all(dir);
    }
    let _ = std::fs::remove_file(&shot);
}

#[test]
fn export_toast_panel_preset_parses_and_the_toast_has_a_reveal_path() {
    let _preset = preset_export_lock();
    let opts = crate::cli::parse(["--panel".to_string(), "export-toast".to_string()]).expect("export-toast parses");
    assert_eq!(opts.panel.as_deref(), Some("export-toast"));
    let s = Sim::with(opts);
    assert_eq!(s.app().ui.right_tab, Some(crate::app::RightTab::Design));
    let id = s.app().selection.first().copied().expect("the first frame is selected");
    assert!(matches!(s.app().doc.get(id).map(|n| &n.kind), Some(designos_doc::NodeKind::Frame(_))));
    let toast = s.app().toast.as_ref().expect("the export toast");
    let reveal = toast.reveal.clone().expect("Show in folder path");
    assert!(reveal.is_file(), "the export wrote the file the toast names: {}", reveal.display());
    assert!(toast.text.contains("No save dialog"), "{}", toast.text);
    assert!(toast.text.contains(&reveal.display().to_string()), "{}", toast.text);
    if let Some(dir) = reveal.parent() {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn export_toast_preset_survives_a_plain_toast_before_the_first_frame() {
    let _preset = preset_export_lock();
    let opts = crate::cli::parse(["--panel".to_string(), "export-toast".to_string()]).expect("export-toast parses");
    let mut s = Sim::built(opts);
    let expected = s.app().toast.as_ref().expect("export toast raised at build").text.clone();
    let reveal_dir = s.app().toast.as_ref().and_then(|t| t.reveal.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf())));
    // Simulate `poll_fonts` raising a plain toast before `draw_window` updates `app.now`.
    s.shell.app.show_toast("Font unavailable", false);
    s.frame(vec![]);
    assert!(painted_text(&s, &expected), "export reveal toast must not be replaced by a pre-frame plain toast");
    if let Some(dir) = reveal_dir {
        let _ = std::fs::remove_dir_all(dir);
    }
}
