//! Every native write of the editor keeps the content of the pages that were not loaded when the file was
//! imported: a large `.fig` defers its inactive pages, the instances on them are empty until the page is
//! loaded, and a write made before that would store them empty for good.

use crate::app::App;
use crate::home::LibFile;
use designos_doc::{Document, History};
use designos_fig::testkit::{deferred_pages_fig, deferred_pages_shape, DeferredPagesShape};
use designos_library::Library;
use std::path::{Path, PathBuf};

/// A scratch folder of its own, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let dir =
            std::env::temp_dir().join(format!("designos_native_write_{tag}_{}_{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    /// The deferred-page `.fig` on disk.
    fn fig(&self) -> PathBuf {
        let path = self.0.join("kit.fig");
        std::fs::write(&path, deferred_pages_fig()).unwrap();
        path
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// An editor with a library holding one project.
fn editor_with_library(scratch: &Scratch) -> (App, Library, String) {
    let mut app = App::new(Document::new("Init"));
    app.attach_library(scratch.path("library"));
    let lib = app.lib.clone().expect("library attached");
    let project = lib.list_projects().into_iter().next().unwrap_or_else(|| lib.create_project("Projects").unwrap());
    (app, lib, project.id)
}

/// The `.fig` opened in the editor: its first page loaded, the third waiting.
fn open_fig(app: &mut App, fig: &Path) {
    app.open_path(fig.to_path_buf());
    assert!(app.doc.page_hydrator.is_some(), "precondition: the importer deferred a page");
    assert_eq!(deferred_pages_shape(&app.doc), DeferredPagesShape::deferred(), "precondition: the instances on the deferred page are empty");
}

fn read_native(path: &Path) -> Document {
    crate::io::load_path(path).unwrap_or_else(|e| panic!("reopen {}: {e}", path.display()))
}

#[test]
fn a_large_fig_imported_through_home_keeps_the_instance_content_of_pages_that_were_not_loaded() {
    let scratch = Scratch::new("import");
    let (mut app, lib, project) = editor_with_library(&scratch);
    let before: Vec<String> = lib.all_files().into_iter().map(|f| f.id).collect();

    assert_eq!(app.import_into_library(&[scratch.fig()], &project, None), 1, "precondition: the import succeeds");

    let imported = lib.all_files().into_iter().find(|f| !before.contains(&f.id)).expect("the imported file is listed");
    app.open_library_file(&imported.id);
    assert!(app.lib_file.as_ref().is_some_and(|f| f.id == imported.id), "precondition: the library file opened: {:?}", app.toast.as_ref().map(|t| &t.text));
    assert_eq!(deferred_pages_shape(&app.doc), DeferredPagesShape::loaded(), "the reopened library file keeps the children and the overrides");
}

#[test]
fn a_large_fig_opened_in_the_editor_and_saved_to_the_library_keeps_the_instance_content_of_pages_that_were_not_loaded() {
    let scratch = Scratch::new("save_to_library");
    let (mut app, lib, project) = editor_with_library(&scratch);
    open_fig(&mut app, &scratch.fig());

    app.save_to_library_as(&project, None, "Kit");

    let saved = app.lib_file.clone().expect("the document is now a library file");
    assert_eq!(deferred_pages_shape(&lib.load_file(&saved.id).unwrap()), DeferredPagesShape::loaded(), "the stored file keeps the children and the overrides");
    assert_eq!(deferred_pages_shape(&app.doc), DeferredPagesShape::loaded(), "the document on screen holds what the file holds");
}

#[test]
fn a_large_fig_opened_in_the_editor_and_saved_as_designos_keeps_the_instance_content_of_pages_that_were_not_loaded() {
    let scratch = Scratch::new("save_as");
    let (mut app, _lib, _project) = editor_with_library(&scratch);
    open_fig(&mut app, &scratch.fig());
    let target = scratch.path("kit.designos");

    app.persist_document_to(&target).expect("the save succeeds");

    assert_eq!(deferred_pages_shape(&read_native(&target)), DeferredPagesShape::loaded(), "the saved file keeps the children and the overrides");
}

#[test]
fn the_save_of_an_open_library_file_keeps_the_instance_content_of_pages_that_were_not_loaded() {
    let scratch = Scratch::new("autosave");
    let (_app, lib, project) = editor_with_library(&scratch);
    let entry = lib.create_file(&project, None, "Kit", &Document::new("Kit")).unwrap();
    let file = LibFile { id: entry.id.clone(), project_id: project, project: "Projects".into(), name: "Kit".into() };
    let mut doc = designos_library::load_external(&scratch.fig()).unwrap();
    assert_eq!(deferred_pages_shape(&doc), DeferredPagesShape::deferred(), "precondition: the instances on the deferred page are empty");

    App::save_doc_to_library(&lib, &file, &mut doc, &mut History::default()).expect("the save succeeds");

    assert_eq!(deferred_pages_shape(&lib.load_file(&entry.id).unwrap()), DeferredPagesShape::loaded(), "the stored file keeps the children and the overrides");
}

#[test]
fn a_version_of_the_open_file_is_not_saved_while_pages_are_not_loaded() {
    let scratch = Scratch::new("version");
    let (mut app, lib, project) = editor_with_library(&scratch);
    let entry = lib.create_file(&project, None, "Kit", &Document::new("Kit")).unwrap();
    app.lib_file = Some(LibFile { id: entry.id.clone(), project_id: project, project: "Projects".into(), name: "Kit".into() });
    app.doc = designos_library::load_external(&scratch.fig()).unwrap();
    assert_eq!(deferred_pages_shape(&app.doc), DeferredPagesShape::deferred(), "precondition: the instances on the deferred page are empty");

    assert!(app.save_auto_version().is_none(), "no version is written from a partial document");

    assert!(lib.list_versions(&entry.id).is_empty());
    assert!(
        app.toast.as_ref().is_some_and(|t| t.error && t.text.contains("not loaded yet")),
        "the toast names the cause: {:?}",
        app.toast.as_ref().map(|t| &t.text)
    );
}

/// Runs `designos <args>` in-process and reports a refusal as the error text.
fn run_cli(args: &[&str]) -> Result<(), String> {
    let opts = crate::cli::parse(args.iter().map(|a| a.to_string()))?;
    crate::run_agent_cli(&opts)
}

#[test]
fn writing_a_large_fig_out_as_designos_from_the_cli_keeps_the_instance_content_of_pages_that_were_not_loaded() {
    let scratch = Scratch::new("cli");
    let fig = scratch.fig();
    let out = scratch.path("kit.designos");

    run_cli(&[fig.to_str().unwrap(), "--out", out.to_str().unwrap(), "--add-comment", "note"]).expect("the CLI run succeeds");

    assert_eq!(deferred_pages_shape(&read_native(&out)), DeferredPagesShape::loaded(), "the written file keeps the children and the overrides");
}
