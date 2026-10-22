//! What Open, Save As and Export selection do for each way a file dialog can end.

use super::*;
use crate::app::App;
use designos_doc::{Document, Node, Rect};

/// The one message Open and Save As show when no dialog can open.
const NO_FILE_DIALOG: &str = "No file dialog. Install zenity or xdg-desktop-portal.";

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("designos_dialog_{}_{name}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// An app with one selected rectangle called `layer_name`.
fn app_with_selected_layer(layer_name: &str) -> App {
    let mut doc = Document::new("Doc");
    let page = doc.add_page("Page");
    let id = doc.alloc_id();
    let mut node = Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 30.0));
    node.name = layer_name.to_string();
    doc.insert(page, node).unwrap();
    let mut app = App::new(doc);
    app.selection = vec![id];
    app
}

fn toast_of(app: &App) -> Option<(String, bool)> {
    app.toast.as_ref().map(|t| (t.text.clone(), t.error))
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

#[test]
fn export_selection_without_a_dialog_says_so_and_saves_to_the_fallback_folder() {
    let dir = scratch_dir("export_unavailable");
    let mut app = app_with_selected_layer("Frame 1");
    let dialogs = ScriptedDialogs::answering(DialogOutcome::Unavailable).with_fallback(dir.clone());
    app.export_selection_with(&dialogs);
    let saved = dir.join("Frame 1.png");
    assert!(saved.is_file(), "the export still lands in the fallback folder: {:?}", files_in(&dir));
    assert!(std::fs::metadata(&saved).unwrap().len() > 0, "the file holds the rendered PNG");
    let (text, error) = toast_of(&app).expect("a toast says what happened");
    assert_eq!(text, format!("No save dialog. Saved to {}", saved.display()));
    assert!(error, "a missing dialog is a warning, not a plain success");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn export_selection_fallback_never_overwrites_an_existing_file() {
    let dir = scratch_dir("export_no_overwrite");
    std::fs::write(dir.join("Frame 1.png"), b"keep me").unwrap();
    std::fs::write(dir.join("Frame 1 2.png"), b"keep me too").unwrap();
    let mut app = app_with_selected_layer("Frame 1");
    app.export_selection_with(&ScriptedDialogs::answering(DialogOutcome::Unavailable).with_fallback(dir.clone()));
    assert_eq!(std::fs::read(dir.join("Frame 1.png")).unwrap(), b"keep me");
    assert_eq!(std::fs::read(dir.join("Frame 1 2.png")).unwrap(), b"keep me too");
    let third = dir.join("Frame 1 3.png");
    assert!(third.is_file(), "the next free name is used: {:?}", files_in(&dir));
    assert_eq!(toast_of(&app).unwrap().0, format!("No save dialog. Saved to {}", third.display()));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn export_selection_fallback_keeps_a_layer_name_inside_the_folder() {
    let dir = scratch_dir("export_sanitized");
    let mut app = app_with_selected_layer("../escape/Frame");
    app.export_selection_with(&ScriptedDialogs::answering(DialogOutcome::Unavailable).with_fallback(dir.clone()));
    let names = files_in(&dir);
    assert_eq!(names.len(), 1, "one file, directly in the folder: {names:?}");
    assert!(!names[0].contains('/') && names[0].ends_with(".png"), "{names:?}");
    assert!(!dir.parent().unwrap().join("escape").exists());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn export_selection_fallback_saves_a_layer_with_a_very_long_name() {
    let dir = scratch_dir("export_long_name");
    for (label, name) in [("ascii", "a".repeat(300)), ("wide", "界".repeat(300))] {
        let mut app = app_with_selected_layer(&name);
        app.export_selection_with(&ScriptedDialogs::answering(DialogOutcome::Unavailable).with_fallback(dir.clone()));
        let (text, error) = toast_of(&app).unwrap();
        assert!(text.starts_with("No save dialog. Saved to "), "{label}: {text}");
        assert!(error);
    }
    let names = files_in(&dir);
    assert_eq!(names.len(), 2, "{names:?}");
    assert!(names.iter().all(|n| n.len() <= 255 && n.ends_with(".png")), "{names:?}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn export_selection_without_a_dialog_or_a_fallback_folder_still_says_so() {
    let mut app = app_with_selected_layer("Frame 1");
    app.export_selection_with(&ScriptedDialogs::answering(DialogOutcome::Unavailable));
    let (text, error) = toast_of(&app).expect("not silent");
    assert_eq!(text, "No save dialog. No Downloads or home folder found.");
    assert!(error);
}

#[test]
fn export_selection_cancel_is_silent_and_writes_nothing() {
    let dir = scratch_dir("export_cancel");
    let mut app = app_with_selected_layer("Frame 1");
    app.export_selection_with(&ScriptedDialogs::answering(DialogOutcome::Cancelled).with_fallback(dir.clone()));
    assert_eq!(toast_of(&app), None, "a cancel in a working dialog says nothing");
    assert!(files_in(&dir).is_empty(), "and writes nothing");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn export_selection_writes_to_the_path_the_dialog_picked() {
    let dir = scratch_dir("export_picked");
    let target = dir.join("chosen.png");
    let mut app = app_with_selected_layer("Frame 1");
    app.export_selection_with(&ScriptedDialogs::answering(DialogOutcome::Picked(target.clone())).with_fallback(dir.join("unused")));
    assert!(target.is_file());
    assert_eq!(toast_of(&app), Some((format!("Exported {}", target.display()), false)));
    assert_eq!(files_in(&dir), vec!["chosen.png".to_string()], "nothing lands in the fallback folder");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn export_selection_asks_for_a_png_named_after_the_layer() {
    let mut app = app_with_selected_layer("Frame 1");
    let dialogs = ScriptedDialogs::answering(DialogOutcome::Cancelled);
    app.export_selection_with(&dialogs);
    assert_eq!(*dialogs.requests.borrow(), vec![export_request("Frame 1")]);
}

#[test]
fn save_as_without_a_dialog_says_so_and_saves_nothing() {
    let mut app = app_with_selected_layer("Frame 1");
    let dialogs = ScriptedDialogs::answering(DialogOutcome::Unavailable);
    app.save_with(true, &dialogs);
    assert_eq!(toast_of(&app), Some((NO_FILE_DIALOG.to_string(), true)));
    assert_eq!(app.file_path, None, "the document is not bound to any file");
}

#[test]
fn save_as_cancel_is_silent() {
    let mut app = app_with_selected_layer("Frame 1");
    app.save_with(true, &ScriptedDialogs::answering(DialogOutcome::Cancelled));
    assert_eq!(toast_of(&app), None);
    assert_eq!(app.file_path, None);
}

#[test]
fn open_without_a_dialog_says_so() {
    let mut app = app_with_selected_layer("Frame 1");
    app.open_dialog_with(&ScriptedDialogs::answering(DialogOutcome::Unavailable));
    assert_eq!(toast_of(&app), Some((NO_FILE_DIALOG.to_string(), true)));
    assert_eq!(app.doc.name, "Doc", "the open document is untouched");
}

#[test]
fn open_cancel_is_silent() {
    let mut app = app_with_selected_layer("Frame 1");
    app.open_dialog_with(&ScriptedDialogs::answering(DialogOutcome::Cancelled));
    assert_eq!(toast_of(&app), None);
}

#[test]
fn web_capture_import_without_a_dialog_says_so() {
    let mut app = app_with_selected_layer("Frame 1");
    app.import_web_capture_dialog_with(&ScriptedDialogs::answering(DialogOutcome::Unavailable));
    assert_eq!(toast_of(&app), Some((NO_FILE_DIALOG.to_string(), true)));
}

#[test]
fn web_capture_import_cancel_is_silent() {
    let mut app = app_with_selected_layer("Frame 1");
    app.import_web_capture_dialog_with(&ScriptedDialogs::answering(DialogOutcome::Cancelled));
    assert_eq!(toast_of(&app), None);
}

/// Every other place a dialog opens: unavailable shows the message and changes nothing, a cancel is silent.
mod other_dialog_sites {
    use super::*;

    fn unchanged_doc_bytes(app: &App) -> (usize, usize) {
        (app.doc.images.len(), app.doc.page_node_count(app.page))
    }

    #[test]
    fn the_image_fill_picker_says_so_and_leaves_the_paint_alone() {
        let mut app = app_with_selected_layer("Frame 1");
        let mut paint = designos_doc::Paint::solid(designos_doc::Color::rgb(1, 2, 3));
        let before = (paint.clone(), unchanged_doc_bytes(&app));
        let changed = crate::panels::properties::choose_image_fill(&mut app, &mut paint, &ScriptedDialogs::answering(DialogOutcome::Unavailable));
        assert!(!changed);
        assert_eq!(toast_of(&app), Some((NO_FILE_DIALOG.to_string(), true)));
        assert_eq!((paint, unchanged_doc_bytes(&app)), before);
    }

    #[test]
    fn the_image_fill_picker_is_silent_on_cancel() {
        let mut app = app_with_selected_layer("Frame 1");
        let mut paint = designos_doc::Paint::solid(designos_doc::Color::rgb(1, 2, 3));
        let changed = crate::panels::properties::choose_image_fill(&mut app, &mut paint, &ScriptedDialogs::answering(DialogOutcome::Cancelled));
        assert!(!changed);
        assert_eq!(toast_of(&app), None);
    }

    #[test]
    fn the_image_tool_says_so_and_adds_nothing() {
        let mut app = app_with_selected_layer("Frame 1");
        let before = unchanged_doc_bytes(&app);
        crate::canvas::interact::place_image_with(&mut app, designos_doc::Vec2::new(10.0, 10.0), &ScriptedDialogs::answering(DialogOutcome::Unavailable));
        assert_eq!(toast_of(&app), Some((NO_FILE_DIALOG.to_string(), true)));
        assert_eq!(unchanged_doc_bytes(&app), before);
    }

    #[test]
    fn the_image_tool_is_silent_on_cancel() {
        let mut app = app_with_selected_layer("Frame 1");
        crate::canvas::interact::place_image_with(&mut app, designos_doc::Vec2::new(10.0, 10.0), &ScriptedDialogs::answering(DialogOutcome::Cancelled));
        assert_eq!(toast_of(&app), None);
    }

    #[test]
    fn home_import_says_so_and_imports_nothing() {
        let mut app = app_with_selected_layer("Frame 1");
        crate::home::import_files_dialog(&mut app, &ScriptedDialogs::answering(DialogOutcome::Unavailable));
        assert_eq!(toast_of(&app), Some((NO_FILE_DIALOG.to_string(), true)));
    }

    #[test]
    fn home_import_is_silent_on_cancel() {
        let mut app = app_with_selected_layer("Frame 1");
        crate::home::import_files_dialog(&mut app, &ScriptedDialogs::answering(DialogOutcome::Cancelled));
        assert_eq!(toast_of(&app), None);
    }

    #[test]
    fn variables_import_says_so_and_starts_no_import() {
        let mut app = app_with_selected_layer("Frame 1");
        crate::variables::view::import_variables_dialog(&mut app, designos_fig::VariablesFormat::Dtcg, &ScriptedDialogs::answering(DialogOutcome::Unavailable));
        assert_eq!(toast_of(&app), Some((NO_FILE_DIALOG.to_string(), true)));
        assert!(app.vars.pending_io.is_none());
    }

    #[test]
    fn variables_export_says_so_and_starts_no_export() {
        let mut app = app_with_selected_layer("Frame 1");
        let dialogs = ScriptedDialogs::answering(DialogOutcome::Unavailable);
        crate::variables::view::export_variables_dialog(&mut app, designos_fig::VariablesFormat::Dtcg, "tokens.zip", &["zip", "json"], &dialogs);
        assert_eq!(toast_of(&app), Some((NO_FILE_DIALOG.to_string(), true)));
        assert!(app.vars.pending_io.is_none());
    }

    #[test]
    fn variables_import_and_export_are_silent_on_cancel() {
        let mut app = app_with_selected_layer("Frame 1");
        let cancelled = ScriptedDialogs::answering(DialogOutcome::Cancelled);
        crate::variables::view::import_variables_dialog(&mut app, designos_fig::VariablesFormat::Dtcg, &cancelled);
        crate::variables::view::export_variables_dialog(&mut app, designos_fig::VariablesFormat::Dtcg, "tokens.zip", &["zip"], &cancelled);
        assert_eq!(toast_of(&app), None);
        assert!(app.vars.pending_io.is_none());
    }

    #[test]
    fn asset_download_without_a_dialog_saves_to_the_fallback_folder_and_says_so() {
        let dir = scratch_dir("asset_unavailable");
        let mut app = app_with_selected_layer("Frame 1");
        let id = app.selection[0];
        let dialogs = ScriptedDialogs::answering(DialogOutcome::Unavailable).with_fallback(dir.clone());
        crate::panels::inspect::download_with(&mut app, id, &crate::panels::inspect::Asset::Svg, &dialogs);
        let saved = dir.join("Frame 1.svg");
        assert!(std::fs::read_to_string(&saved).unwrap().contains("<svg"), "{:?}", files_in(&dir));
        assert_eq!(toast_of(&app), Some((format!("No save dialog. Saved to {}", saved.display()), true)));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn asset_download_without_a_dialog_or_a_fallback_folder_says_so() {
        let mut app = app_with_selected_layer("Frame 1");
        let id = app.selection[0];
        crate::panels::inspect::download_with(&mut app, id, &crate::panels::inspect::Asset::Svg, &ScriptedDialogs::answering(DialogOutcome::Unavailable));
        assert_eq!(toast_of(&app), Some(("No save dialog. No Downloads or home folder found.".to_string(), true)));
    }

    #[test]
    fn asset_download_is_silent_on_cancel() {
        let mut app = app_with_selected_layer("Frame 1");
        let id = app.selection[0];
        crate::panels::inspect::download_with(&mut app, id, &crate::panels::inspect::Asset::Svg, &ScriptedDialogs::answering(DialogOutcome::Cancelled));
        assert_eq!(toast_of(&app), None);
    }

    #[test]
    fn the_deck_pdf_export_without_a_dialog_saves_to_the_fallback_folder_and_says_so() {
        let dir = scratch_dir("deck_unavailable");
        let mut app = App::new(crate::decks::new_document("Talk"));
        app.export_deck_pdf_with(true, &ScriptedDialogs::answering(DialogOutcome::Unavailable).with_fallback(dir.clone()));
        let saved = dir.join("Talk.pdf");
        assert!(std::fs::read(&saved).unwrap().starts_with(b"%PDF-"), "{:?}", files_in(&dir));
        assert_eq!(toast_of(&app), Some((format!("No save dialog. Saved to {}", saved.display()), true)));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_deck_pdf_export_without_a_dialog_or_a_fallback_folder_says_so() {
        let mut app = App::new(crate::decks::new_document("Talk"));
        app.export_deck_pdf_with(true, &ScriptedDialogs::answering(DialogOutcome::Unavailable));
        assert_eq!(toast_of(&app), Some(("No save dialog. No Downloads or home folder found.".to_string(), true)));
    }

    #[test]
    fn the_deck_pdf_export_is_silent_on_cancel() {
        let mut app = App::new(crate::decks::new_document("Talk"));
        app.export_deck_pdf_with(true, &ScriptedDialogs::answering(DialogOutcome::Cancelled));
        assert_eq!(toast_of(&app), None);
    }
}

fn repo_file(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn the_deb_recommends_a_dialog_backend() {
    let package = repo_file("scripts/package.sh");
    let control = package.lines().filter(|l| l.starts_with("Depends:") || l.starts_with("Recommends:")).collect::<Vec<_>>();
    assert!(control.contains(&"Recommends: zenity | xdg-desktop-portal"), "{control:?}");
}

#[test]
fn the_linux_install_docs_say_what_the_file_dialog_needs() {
    let install = repo_file("docs/install.md");
    let linux = install.split("## 3. Linux").nth(1).and_then(|rest| rest.split("\n## 4.").next()).expect("a Linux section");
    assert!(linux.contains("xdg-desktop-portal") && linux.contains("zenity"), "{linux}");
    assert!(linux.contains("`zenity | xdg-desktop-portal`"), "the doc states the same order as the package: {linux}");
}
