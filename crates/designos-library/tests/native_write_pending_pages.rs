//! A library write never stores a document whose pages are not loaded yet: their instances would be
//! empty in the file for good. An import loads the pages itself; a write of a document that is only
//! borrowed refuses with an error and leaves the library as it was.

use designos_doc::Document;
use designos_fig::testkit::{deferred_pages_fig, deferred_pages_shape, DeferredPagesShape};
use designos_library::Library;
use std::path::PathBuf;

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("native-write-pending-pages-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A library with one project, and the deferred-page `.fig` on disk next to it.
fn library_and_fig(name: &str) -> (Library, String, PathBuf) {
    let dir = scratch(name);
    let fig = dir.join("kit.fig");
    std::fs::write(&fig, deferred_pages_fig()).unwrap();
    let lib = Library::open(dir.join("library")).unwrap();
    let project = lib.create_project("Projects").unwrap();
    (lib, project.id, fig)
}

/// The `.fig` opened the way an editor open does, with its third page not loaded.
fn document_with_unloaded_page(fig: &std::path::Path) -> Document {
    let doc = designos_library::load_external(fig).unwrap();
    assert!(doc.page_hydrator.is_some(), "precondition: the importer deferred a page");
    assert_eq!(deferred_pages_shape(&doc), DeferredPagesShape::deferred(), "precondition: the instances on the deferred page are empty");
    doc
}

#[test]
fn a_large_fig_imported_into_the_library_keeps_the_instance_content_of_pages_that_were_not_loaded() {
    let (lib, project, fig) = library_and_fig("import");
    document_with_unloaded_page(&fig);

    let (entry, returned) = lib.import_file_with_doc(&fig, &project, None).unwrap();

    let reopened = lib.load_file(&entry.id).unwrap();
    assert_eq!(deferred_pages_shape(&reopened), DeferredPagesShape::loaded(), "the stored file keeps the children and the overrides");
    assert_eq!(deferred_pages_shape(&returned), DeferredPagesShape::loaded(), "the document handed back for the thumbnail matches the stored file");
}

#[test]
fn a_library_file_is_not_created_from_a_document_with_pages_that_were_not_loaded() {
    let (lib, project, fig) = library_and_fig("create");
    let doc = document_with_unloaded_page(&fig);

    let err = lib.create_file(&project, None, "Kit", &doc).expect_err("a partial document must not become a file");

    assert!(err.to_string().contains("not loaded yet"), "the error names the cause: {err}");
    assert!(lib.files_in(&project, None).unwrap().is_empty(), "nothing is listed");
}

#[test]
fn a_library_file_is_not_overwritten_by_a_document_with_pages_that_were_not_loaded() {
    let (lib, project, fig) = library_and_fig("save");
    let doc = document_with_unloaded_page(&fig);
    let entry = lib.create_file(&project, None, "Kept", &Document::new("Kept")).unwrap();
    let before = lib.load_file(&entry.id).unwrap().to_json().unwrap();

    assert!(lib.save_file(&entry.id, &doc).is_err(), "a partial document must not replace the stored one");

    assert_eq!(lib.load_file(&entry.id).unwrap().to_json().unwrap(), before, "the stored file is untouched");
}

#[test]
fn a_version_is_not_saved_from_a_document_with_pages_that_were_not_loaded() {
    let (lib, project, fig) = library_and_fig("version");
    let doc = document_with_unloaded_page(&fig);
    let entry = lib.create_file(&project, None, "Kept", &Document::new("Kept")).unwrap();

    assert!(lib.save_version(&entry.id, &doc, None, true).is_err());

    assert!(lib.list_versions(&entry.id).is_empty(), "no version was written");
}

#[test]
fn a_branch_is_not_created_or_saved_from_a_document_with_pages_that_were_not_loaded() {
    let (lib, project, fig) = library_and_fig("branch");
    let doc = document_with_unloaded_page(&fig);
    let main = Document::new("Main");
    let entry = lib.create_file(&project, None, "Kept", &main).unwrap();

    assert!(lib.create_branch(&entry.id, &doc, "Try", "me").is_err(), "no branch is made from a partial document");
    assert!(lib.list_branches(&entry.id).unwrap().is_empty());

    let branch = lib.create_branch(&entry.id, &main, "Real", "me").unwrap();
    let before = lib.load_branch(&entry.id, &branch.id).unwrap().to_json().unwrap();
    assert!(lib.save_branch(&entry.id, &branch.id, &doc).is_err(), "a partial document must not replace the branch");
    assert_eq!(lib.load_branch(&entry.id, &branch.id).unwrap().to_json().unwrap(), before, "the branch is untouched");
}

#[test]
fn a_library_is_not_published_from_a_document_with_pages_that_were_not_loaded() {
    let (lib, _project, fig) = library_and_fig("publish");
    let doc = document_with_unloaded_page(&fig);

    assert!(lib.publish_library("kit", &doc, "first").is_err(), "a snapshot of a partial document must not be published");

    assert!(lib.list_libraries().unwrap().is_empty(), "nothing was published");
}
