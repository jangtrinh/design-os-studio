//! Tests for the `.fig` export: destination, naming, failures and the reveal command.

use super::{export_new, reveal_command, Exported};
use crate::app::App;
use crate::cmdk::CmdKAction;
use crate::home::Act;
use designos_doc::{Document, Node, Rect};
use std::path::{Path, PathBuf};

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("designos_fig_export_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn doc_named(name: &str) -> Document {
    let mut doc = Document::new(name);
    doc.add_page("Page 1");
    doc
}

/// The `.fig` files in `dir`, sorted by name.
fn figs_in(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> =
        std::fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "fig")).collect()).unwrap_or_default();
    found.sort();
    found
}

/// An app with a library in `root` holding one file named `name`, opened in the editor.
fn app_with_open_library_file(root: &Path, name: &str) -> (App, String) {
    let mut app = App::new(Document::new("Init"));
    app.attach_library(root.to_path_buf());
    let lib = app.lib.clone().expect("library attached");
    let proj = lib.create_project("Fig export tests").unwrap();
    let entry = lib.create_file(&proj.id, None, name, &doc_named(name)).unwrap();
    app.open_library_file(&entry.id);
    assert!(app.lib_file.is_some() && app.file_path.is_none(), "precondition: a library file is open");
    (app, entry.id)
}

/// A directory the process cannot create files in: its path is a regular file.
fn blocked_dir(parent: &Path) -> PathBuf {
    let blocked = parent.join("blocked");
    std::fs::write(&blocked, b"not a folder").unwrap();
    blocked
}

/// The success toast carries the exported file for its "Show in folder" action and stays long enough to act on.
fn assert_offers_reveal(app: &App, exported: &Path) {
    let toast = app.toast.as_ref().expect("success toast");
    assert!(!toast.error, "unexpected error toast: {}", toast.text);
    assert!(toast.text.contains(&exported.display().to_string()), "toast shows the full path: {}", toast.text);
    assert_eq!(toast.reveal.as_deref(), Some(exported), "toast carries the file to reveal");
    assert!(toast.until - app.now >= 8.0, "toast must stay at least 8 s, stays {}", toast.until - app.now);
}

fn assert_failed_without_claiming_success(app: &App) {
    let toast = app.toast.as_ref().expect("a failed export must say so");
    assert!(toast.error, "failure toast must use error styling: {}", toast.text);
    assert!(toast.text.starts_with("Export .fig failed"), "unexpected toast: {}", toast.text);
    assert!(!toast.text.contains("Exported"), "a failed export must not claim success: {}", toast.text);
}

// ---------------------------------------------------------------- palette (never overwrite the open file)

#[test]
fn palette_export_leaves_the_open_fig_file_untouched() {
    let dir = scratch("palette-open-fig");
    let source = dir.join("Source.fig");
    let original = designos_fig::export_fig_with_report(&doc_named("Source")).unwrap().0;
    std::fs::write(&source, &original).unwrap();

    let mut app = App::new(Document::new("Init"));
    app.open_path(source.clone());
    assert_eq!(app.file_path.as_deref(), Some(source.as_path()), "precondition: the .fig is the open file");
    // Edit in memory so a re-export differs from the bytes on disk.
    let page = app.page;
    let id = app.doc.alloc_id();
    app.doc.insert(page, Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 40.0))).unwrap();

    crate::shell::execute_cmdk_action(&mut app, CmdKAction::ExportFig);

    assert_eq!(std::fs::read(&source).unwrap(), original, "the open .fig must keep its bytes");
    let exports: Vec<PathBuf> = figs_in(&dir).into_iter().filter(|p| *p != source).collect();
    assert_eq!(exports.len(), 1, "the export is a new file next to the source: {exports:?}");
    assert_offers_reveal(&app, &exports[0]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_second_palette_export_does_not_replace_the_first() {
    let dir = scratch("palette-twice");
    let file = dir.join("Design.designos");
    std::fs::write(&file, doc_named("Design").to_json().unwrap()).unwrap();
    let mut app = App::new(Document::new("Init"));
    app.open_path(file.clone());

    crate::shell::execute_cmdk_action(&mut app, CmdKAction::ExportFig);
    let first = figs_in(&dir);
    assert_eq!(first.len(), 1, "first export lands next to the file: {first:?}");
    let first_bytes = std::fs::read(&first[0]).unwrap();
    crate::shell::execute_cmdk_action(&mut app, CmdKAction::ExportFig);

    let both = figs_in(&dir);
    assert_eq!(both.len(), 2, "second export picks a free name: {both:?}");
    assert_eq!(std::fs::read(&first[0]).unwrap(), first_bytes, "the first export keeps its bytes");
    assert!(std::fs::metadata(&file).is_ok_and(|m| m.len() > 0), "the open file is still there");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------- one place for every path

#[test]
fn export_copy_of_a_library_file_goes_to_the_library_exports_folder() {
    let root = scratch("copy-library");
    let (mut app, _) = app_with_open_library_file(&root, "Library Copy Case");

    app.export_fig_copy();

    let exports = figs_in(&root.join("exports"));
    assert_eq!(exports.len(), 1, "one export in the library exports folder: {exports:?}");
    assert_offers_reveal(&app, &exports[0]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn palette_export_of_a_library_file_goes_to_the_library_exports_folder() {
    let root = scratch("palette-library");
    let (mut app, _) = app_with_open_library_file(&root, "Library Palette Case");

    crate::shell::execute_cmdk_action(&mut app, CmdKAction::ExportFig);

    assert_eq!(figs_in(&root.join("exports")).len(), 1, "palette and menu share one destination");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn home_export_writes_to_the_library_exports_folder_and_shows_the_full_path() {
    let root = scratch("home-ok");
    let (mut app, id) = app_with_open_library_file(&root, "Home Case");
    app.go_home();

    crate::home::apply(&mut app, Act::ExportFig(id));

    let exports = figs_in(&root.join("exports"));
    assert_eq!(exports.len(), 1, "{exports:?}");
    assert_offers_reveal(&app, &exports[0]);
    let _ = std::fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------- failures are said out loud

#[test]
fn export_copy_reports_a_failed_write() {
    let dir = scratch("copy-fails");
    let mut app = App::new(doc_named("Blocked Copy"));
    app.file_path = Some(blocked_dir(&dir).join("Blocked Copy.designos"));

    app.export_fig_copy();

    assert_failed_without_claiming_success(&app);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn palette_export_reports_a_failed_write() {
    let dir = scratch("palette-fails");
    let mut app = App::new(doc_named("Blocked Palette"));
    app.file_path = Some(blocked_dir(&dir).join("Blocked Palette.designos"));

    crate::shell::execute_cmdk_action(&mut app, CmdKAction::ExportFig);

    assert_failed_without_claiming_success(&app);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn home_export_reports_a_failed_write_and_does_not_retry_in_the_temp_folder() {
    let root = scratch("home-fails");
    let (mut app, id) = app_with_open_library_file(&root, "Blocked Home Export");
    app.go_home();
    let _ = std::fs::remove_dir_all(root.join("exports"));
    std::fs::write(root.join("exports"), b"not a folder").unwrap();

    crate::home::apply(&mut app, Act::ExportFig(id));

    assert_failed_without_claiming_success(&app);
    for stem in ["Blocked Home Export", "Blocked_Home_Export"] {
        assert!(!std::env::temp_dir().join(format!("{stem}.fig")).exists(), "a failed write must not fall back to the temp folder ({stem})");
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn home_export_reports_a_file_that_cannot_be_loaded() {
    let root = scratch("home-unloadable");
    let (mut app, id) = app_with_open_library_file(&root, "Unloadable");
    app.go_home();
    let lib = app.lib.clone().unwrap();
    let entry = lib.file(&id).unwrap();
    std::fs::write(lib.path("projects").join(&entry.project).join("files").join(format!("{id}.designos")), b"{ not json").unwrap();

    crate::home::apply(&mut app, Act::ExportFig(id));

    assert_failed_without_claiming_success(&app);
    let _ = std::fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------- the writer and its helpers

#[test]
fn a_document_name_never_leaves_the_exports_folder() {
    let dir = scratch("names");
    let names = ["My Design", "a/b", "../../escape", "/etc/passwd", "a\\b", "C:\\x", "tab\there", "", "...", "///", "name. "];
    for name in names {
        let done = export_new(&doc_named("N"), &dir, name).unwrap_or_else(|e| panic!("{name:?}: {e}"));
        assert_eq!(done.path.parent(), Some(dir.as_path()), "{name:?} stays in the folder: {}", done.path.display());
        let file = done.path.file_name().unwrap().to_string_lossy().into_owned();
        assert!(file.ends_with(".fig") && !file.starts_with('.') && file.len() > ".fig".len(), "{name:?} gives a real file name: {file}");
    }
    assert_eq!(figs_in(&dir).len(), names.len(), "one file per export, nothing outside the folder");
    assert!(
        dir.join("My Design.fig").is_file() && dir.join("a_b.fig").is_file() && dir.join("Untitled.fig").is_file(),
        "plain names are kept, empty ones become Untitled"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unsafe_library_file_name_still_exports_into_the_exports_folder() {
    let root = scratch("unsafe-name");
    let (mut app, _) = app_with_open_library_file(&root, "../../escape");

    app.export_fig_copy();

    assert_eq!(figs_in(&root.join("exports")).len(), 1, "the export stays in the exports folder");
    assert!(
        figs_in(&root).is_empty() && figs_in(root.parent().unwrap_or(&root)).iter().all(|p| !p.to_string_lossy().contains("escape")),
        "nothing escapes the library"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn export_new_takes_the_next_free_name_and_keeps_what_is_there() {
    let dir = scratch("numbering");
    std::fs::write(dir.join("Design.fig"), b"first").unwrap();
    std::fs::write(dir.join("Design 2.fig"), b"second").unwrap();

    let done = export_new(&doc_named("Design"), &dir, "Design").unwrap();

    assert_eq!(done.path, dir.join("Design 3.fig"));
    assert_eq!(std::fs::read(dir.join("Design.fig")).unwrap(), b"first");
    assert_eq!(std::fs::read(dir.join("Design 2.fig")).unwrap(), b"second");
    assert!(std::fs::read(&done.path).unwrap().starts_with(b"PK"), "the export is a .fig archive");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn export_new_creates_a_missing_folder() {
    let dir = scratch("missing-folder").join("exports").join("deeper");
    let done = export_new(&doc_named("Design"), &dir, "Design").unwrap();
    assert_eq!(done.path, dir.join("Design.fig"));
    let _ = std::fs::remove_dir_all(dir.parent().unwrap().parent().unwrap());
}

#[test]
fn export_new_names_the_folder_it_cannot_create() {
    let dir = scratch("cannot-create");
    let blocked = blocked_dir(&dir);
    let err = export_new(&doc_named("Design"), &blocked, "Design").err().expect("a folder that is a file cannot be created");
    assert!(err.starts_with("cannot create ") && err.contains(&blocked.display().to_string()), "unexpected error: {err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn export_new_names_the_file_it_cannot_write() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("read-only");
    let locked = dir.join("locked");
    std::fs::create_dir(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
    // A process that ignores permissions (root) cannot show this failure: only assert where the folder is really read-only.
    let probe = locked.join(".probe");
    let enforced = std::fs::write(&probe, b"x").is_err();
    let _ = std::fs::remove_file(&probe);

    let result = export_new(&doc_named("Design"), &locked, "Design");

    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    if enforced {
        let err = result.err().expect("a read-only folder cannot be written");
        assert!(err.starts_with("cannot write to ") && err.contains(&locked.display().to_string()), "unexpected error: {err}");
        assert!(figs_in(&locked).is_empty(), "a failed write leaves no file behind");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_document_with_no_file_and_no_library_asks_to_save_first() {
    let mut app = App::new(doc_named("Floating"));
    assert!(app.lib.is_none() && app.file_path.is_none(), "precondition");

    app.export_fig_copy();

    let toast = app.toast.as_ref().expect("the user is told");
    assert!(toast.error && toast.text == "Export .fig failed: save the file first", "unexpected toast: {}", toast.text);
}

#[test]
fn the_toast_says_what_the_format_could_not_carry_without_a_parenthesised_count() {
    let mut app = App::new(doc_named("Notes"));
    for (omitted_keys, tail) in [(0, ""), (1, ". 1 extension key not carried"), (4, ". 4 extension keys not carried")] {
        app.show_fig_export_result(Ok(Exported { path: PathBuf::from("/exports/Notes.fig"), omitted_keys, omitted_comment_threads: 0 }));
        assert_eq!(app.toast.as_ref().unwrap().text, format!("Exported .fig to /exports/Notes.fig{tail}"));
    }
}

#[test]
fn reveal_command_selects_the_file_in_each_file_manager() {
    let file = Path::new("/lib/exports/Design.fig");
    let (program, args) = reveal_command("macos", file);
    assert_eq!((program, args), ("open", vec!["-R".into(), file.into()]));
    let (program, args) = reveal_command("windows", file);
    assert_eq!((program, args), ("explorer", vec!["/select,".into(), file.into()]));
    let (program, args) = reveal_command("linux", file);
    assert_eq!((program, args), ("xdg-open", vec![Path::new("/lib/exports").into()]));
}

#[test]
fn a_test_never_opens_a_real_file_manager() {
    let err = super::reveal_file(Path::new("/lib/exports/Design.fig")).expect_err("the production reveal refuses to run under test");
    assert!(err.contains("/lib/exports/Design.fig"), "{err}");
}

// ---------------------------------------------------------------- long names and relative paths

/// A document name of 100 CJK characters: 300 bytes, over the 255-byte file name limit of ext4 and most file systems.
fn long_cjk_name() -> String {
    "漢".repeat(100)
}

/// The longest file name the writer may produce: its stem cap plus ` 10000.fig`.
const LONGEST_EXPORT_NAME_BYTES: usize = 255;

#[test]
fn export_new_cuts_a_name_that_is_too_long_for_the_file_system() {
    let dir = scratch("long-name");
    let done = export_new(&doc_named("Long"), &dir, &long_cjk_name()).unwrap();
    let name = done.path.file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.len() <= LONGEST_EXPORT_NAME_BYTES, "a {}-byte file name does not fit ext4", name.len());
    assert!(name.ends_with(".fig") && done.path.starts_with(&dir), "still a .fig in the folder: {name}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_library_file_with_a_very_long_name_exports_with_a_cut_file_name() {
    let root = scratch("long-library-name");
    let (mut app, _) = app_with_open_library_file(&root, &long_cjk_name());

    app.export_fig_copy();

    let exports = figs_in(&root.join("exports"));
    assert_eq!(exports.len(), 1, "the export succeeds: {:?}", app.toast.as_ref().map(|t| &t.text));
    assert!(exports[0].file_name().unwrap().len() <= LONGEST_EXPORT_NAME_BYTES, "file name is cut to fit");
    assert_offers_reveal(&app, &exports[0]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_relative_file_path_still_exports_to_a_full_path() {
    let mut app = App::new(doc_named("Relative"));
    app.file_path = Some(PathBuf::from("some-relative-folder/Relative.designos"));

    let (dir, stem) = app.fig_export_target().unwrap();

    assert!(dir.is_absolute(), "the toast must show a full path, got {}", dir.display());
    assert!(dir.ends_with("some-relative-folder") && stem == "Relative", "{} / {stem}", dir.display());
}
