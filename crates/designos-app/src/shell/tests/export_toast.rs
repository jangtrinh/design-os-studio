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
