//! #1095: the Export section of the Properties panel follows the same dialog-or-Downloads
//! and toast rules as the other exports, never writes silently to `~`, never overwrites an
//! existing file, shows the full path and `Show in folder` on write, and shows the path and
//! reason on a failed write.

use super::*;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn scratch_home(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("designos_panel_export_{}_{name}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn painted_text(s: &Sim) -> Vec<(String, Pos2)> {
    fn collect(shape: &egui::Shape, out: &mut Vec<(String, Pos2)>) {
        match shape {
            egui::Shape::Text(t) => out.push((t.galley.job.text.clone(), t.pos + Vec2::new(0.0, t.galley.size().y / 2.0))),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|x| collect(x, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    s.shapes.iter().for_each(|c| collect(&c.shape, &mut out));
    out
}

fn export_section_origin(s: &Sim) -> (f32, f32) {
    let (_, at) =
        painted_text(s).into_iter().rfind(|(t, p)| t == "Export" && p.x > s.app().canvas_rect.right()).expect("the Export section is painted in the inspector");
    (at.x - theme::SECTION_CONTENT_X, at.y - theme::SECTION_TITLE_HEIGHT / 2.0)
}

fn select_hero(s: &mut Sim) -> NodeId {
    let hero = s.by_name("Hero");
    s.shell.app.selection = vec![hero];
    s.frame(vec![]);
    s.frame(vec![]);
    hero
}

fn click_export_add(s: &mut Sim) {
    s.scroll_inspector_down();
    let p = s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", "export-add".to_string())))).map(|r| r.center()).unwrap_or_else(|| {
        let (x0, top) = export_section_origin(s);
        Pos2::new(x0 + theme::SECTION_TRAILING_X + theme::PANEL_ACTION_BUTTON / 2.0, top + 8.0 + theme::PANEL_ACTION_BUTTON / 2.0)
    });
    s.click(p);
    s.scroll_inspector_down();
}

fn click_export_btn(s: &mut Sim) {
    s.scroll_inspector_down();
    let p = s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", "export-btn".to_string())))).map(|r| r.center()).unwrap_or_else(|| {
        painted_text(s)
            .into_iter()
            .rfind(|(t, p)| t.starts_with("Export ") && p.x > s.app().canvas_rect.right())
            .expect("the Export <layer> button is painted in the inspector")
            .1
    });
    s.click(p);
}

#[test]
fn panel_export_without_dialog_writes_to_fallback_folder_not_home_and_shows_success_toast_with_full_path() {
    static REVEALED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
    fn record_reveal(p: &std::path::Path) -> Result<(), String> {
        REVEALED.lock().unwrap().push(p.to_path_buf());
        Ok(())
    }

    let home = scratch_home("fallback_not_home");
    crate::io::SystemDialogs::with_test_home(&home, || {
        let mut s = Sim::new();
        s.shell.app.reveal_file = record_reveal;
        select_hero(&mut s);

        click_export_add(&mut s);
        click_export_btn(&mut s);

        let home_file = home.join("Hero.png");
        let fallback_file = home.join("Downloads").join("Hero.png");
        assert!(!home_file.exists(), "panel Export must never write silently to ~ ({})", home_file.display());
        assert!(fallback_file.is_file(), "panel Export without a dialog must write to the fallback Downloads folder ({})", fallback_file.display());
        let export_btn = s.rect_of("export-btn");
        assert!(export_btn.width() > 0.0, "export-btn must be recorded in the Export panel");
        let bytes = std::fs::read(&fallback_file).unwrap();
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "written file must be a valid PNG");

        let toast = s.app().toast.as_ref().expect("panel Export must show a toast after writing");
        assert_eq!(toast.text, format!("No save dialog. Saved to {}", fallback_file.display()));
        assert_eq!(toast.reveal.as_deref(), Some(fallback_file.as_path()), "toast must carry the full path for Show in folder");
        assert!(toast.until - s.app().now > 9.0, "export toast with Show in folder must stay for 10 seconds");

        // Toast lifetime and stacking (#937): a plain toast while the export toast is active must not silently replace it.
        s.shell.app.show_toast("Saved", false);
        s.frame(vec![]);
        let active = s.app().toast.as_ref().expect("export toast must remain active");
        assert_eq!(active.reveal.as_deref(), Some(fallback_file.as_path()), "plain toast must not replace an active export toast with Show in folder");

        REVEALED.lock().unwrap().clear();
        let reveal_btn = s.rect_of("toast-reveal");
        s.click(reveal_btn.center());
        assert_eq!(REVEALED.lock().unwrap().as_slice(), std::slice::from_ref(&fallback_file));
    });
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn panel_export_never_overwrites_an_existing_file() {
    let home = scratch_home("no_overwrite");
    let downloads = home.join("Downloads");
    std::fs::create_dir_all(&downloads).unwrap();
    let existing = downloads.join("Hero.png");
    std::fs::write(&existing, b"keep me").unwrap();

    crate::io::SystemDialogs::with_test_home(&home, || {
        let mut s = Sim::new();
        select_hero(&mut s);

        click_export_add(&mut s);
        click_export_btn(&mut s);

        assert_eq!(std::fs::read(&existing).unwrap(), b"keep me", "existing file must never be overwritten");
        let second = downloads.join("Hero 2.png");
        assert!(second.is_file(), "new export must write to numbered sibling {}", second.display());
        assert!(std::fs::read(&second).unwrap().starts_with(b"\x89PNG\r\n\x1a\n"));

        let toast = s.app().toast.as_ref().expect("panel Export must show a toast");
        assert_eq!(toast.text, format!("No save dialog. Saved to {}", second.display()));
        assert_eq!(toast.reveal.as_deref(), Some(second.as_path()));
        assert!(s.rect_of("toast-reveal").width() > 0.0, "Show in folder button must be drawn");
    });
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn panel_export_failed_write_shows_error_toast_with_path_and_reason() {
    let home = scratch_home("failed_write");
    // Block ~/Downloads by placing a regular file where the Downloads directory goes.
    let blocked_downloads = home.join("Downloads");
    std::fs::write(&blocked_downloads, b"not a directory").unwrap();

    crate::io::SystemDialogs::with_test_home(&home, || {
        let mut s = Sim::new();
        select_hero(&mut s);

        click_export_add(&mut s);
        click_export_btn(&mut s);

        assert!(!home.join("Hero.png").exists(), "failed write to Downloads must never fall back to ~");
        let toast = s.app().toast.as_ref().expect("a failed write must show an error toast");
        assert!(toast.error, "failed write toast must be marked as an error");
        assert!(toast.reveal.is_none(), "failed write toast must not show Show in folder");
        let expected_path = blocked_downloads.join("Hero.png");
        assert!(
            toast.text.starts_with("No save dialog. Export failed: ") && toast.text.contains(&expected_path.display().to_string()),
            "error toast must include the failed path and reason, got: {:?}",
            toast.text
        );
        let after_path = toast.text.split_once(&expected_path.display().to_string()).map(|(_, rest)| rest).unwrap_or("");
        assert!(after_path.starts_with(": ") && after_path.len() > 2, "error toast must include the OS reason after the path, got: {:?}", toast.text);
    });
    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn panel_export_multiple_settings_writes_all_files_to_fallback_and_shows_reveal_toast() {
    let home = scratch_home("multi_settings");
    crate::io::SystemDialogs::with_test_home(&home, || {
        let mut s = Sim::new();
        let hero = select_hero(&mut s);
        s.shell.app.edit_nodes(&[hero], |n| {
            n.export_settings = vec![
                designos_doc::ExportPreset {
                    format: designos_doc::ExportFileFormat::Png,
                    constraint: designos_doc::ExportConstraint::Scale(1.0),
                    suffix: String::new(),
                },
                designos_doc::ExportPreset {
                    format: designos_doc::ExportFileFormat::Svg,
                    constraint: designos_doc::ExportConstraint::Scale(1.0),
                    suffix: String::new(),
                },
            ];
        });
        s.frame(vec![]);
        s.frame(vec![]);

        let _ = Arc::new(0);
        click_export_btn(&mut s);

        let downloads = home.join("Downloads");
        let png = downloads.join("Hero.png");
        let svg = downloads.join("Hero.svg");
        assert!(png.is_file() && svg.is_file(), "both PNG and SVG settings must be written to ~/Downloads");
        assert!(!home.join("Hero.png").exists() && !home.join("Hero.svg").exists(), "neither file may be written to ~");

        let toast = s.app().toast.as_ref().expect("multi-file export must show a toast");
        assert_eq!(toast.text, format!("No save dialog. Saved 2 files to {}", downloads.display()));
        assert_eq!(toast.reveal.as_deref(), Some(png.as_path()), "multi-file export toast must support Show in folder");
        assert!(s.rect_of("toast-reveal").width() > 0.0);
    });
    std::fs::remove_dir_all(&home).ok();
}
