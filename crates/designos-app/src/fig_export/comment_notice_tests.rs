//! What a `.fig` export says about comments: the format has no place for them, so an export of a document that
//! has some says so in the same toast that names the file.

use super::{export_new, Exported};
use crate::app::App;
use crate::home::Act;
use designos_doc::{CommentThread, Document, Vec2};
use std::path::{Path, PathBuf};

const NOTICE: &str = "Comments left out: only .designos files keep them";

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("designos_fig_export_comments_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A one-page document with `threads` comment threads, the first of them resolved when `resolve_first` is set.
fn doc_with_threads(name: &str, threads: usize, resolve_first: bool) -> Document {
    let mut doc = Document::new(name);
    let page = doc.add_page("Page 1");
    for n in 0..threads {
        let anchor = crate::comments::model::make_anchor(&doc, page, Vec2::new(10.0 * n as f32, 10.0), None);
        let message = crate::comments::model::comment(format!("m{n}"), "Kir", "Make this bigger");
        doc.comments.push(CommentThread { id: format!("t{n}"), anchor, messages: vec![message], resolved: resolve_first && n == 0 });
    }
    doc
}

fn only_fig_in(dir: &Path) -> PathBuf {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "fig")).collect();
    assert_eq!(found.len(), 1, "one export: {found:?}");
    found.remove(0)
}

#[test]
fn an_export_of_a_document_with_comments_says_they_are_not_kept() {
    let dir = scratch("copy");
    let mut app = App::new(doc_with_threads("Review", 2, true));
    app.file_path = Some(dir.join("Review.designos"));

    app.export_fig_copy();

    let exported = only_fig_in(&dir);
    let toast = app.toast.as_ref().expect("the export says how it went");
    assert_eq!(toast.text, format!("Exported .fig to {}. {NOTICE}", exported.display()));
    assert!(toast.error, "a warning, not a plain success");
    assert_eq!(toast.reveal.as_deref(), Some(exported.as_path()), "the toast still offers the file");
    assert_eq!(app.doc.comments.len(), 2, "the open document keeps its comments");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_export_of_a_document_without_comments_stays_quiet_about_them() {
    let dir = scratch("quiet");
    let mut app = App::new(doc_with_threads("Plain", 0, false));
    app.file_path = Some(dir.join("Plain.designos"));

    app.export_fig_copy();

    let toast = app.toast.as_ref().expect("the export says how it went");
    assert_eq!(toast.text, format!("Exported .fig to {}", only_fig_in(&dir).display()));
    assert!(!toast.error);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The Home menu exports a library file, which is not the document open in the editor: the notice follows the file
/// that was exported.
#[test]
fn a_home_export_reads_the_comments_of_the_exported_file_not_of_the_open_document() {
    let root = scratch("home");
    let mut app = App::new(Document::new("Init"));
    app.attach_library(root.to_path_buf());
    let lib = app.lib.clone().expect("library attached");
    let project = lib.create_project("Comment notice tests").unwrap();
    let with_comments = lib.create_file(&project.id, None, "Has Comments", &doc_with_threads("Has Comments", 1, false)).unwrap();
    let without = lib.create_file(&project.id, None, "Has None", &doc_with_threads("Has None", 0, false)).unwrap();

    // The open document has no comments at all, so a notice read from it would be missing.
    app.open_library_file(&without.id);
    assert!(app.doc.comments.is_empty(), "precondition: the open document has no comments");
    app.go_home();
    crate::home::apply(&mut app, Act::ExportFig(with_comments.id.clone()));
    let toast = app.toast.as_ref().expect("the export says how it went");
    assert!(toast.text.starts_with("Exported .fig to ") && toast.text.ends_with(&format!(". {NOTICE}")), "{}", toast.text);

    // And the other way round: an open document with comments, an exported file without.
    app.open_library_file(&with_comments.id);
    assert_eq!(app.doc.comments.len(), 1, "precondition: the open document has a comment");
    app.go_home();
    crate::home::apply(&mut app, Act::ExportFig(without.id));
    let toast = app.toast.as_ref().expect("the export says how it went");
    assert!(toast.text.starts_with("Exported .fig to ") && !toast.text.contains(NOTICE) && !toast.error, "{}", toast.text);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn export_new_counts_the_comment_threads_the_file_leaves_out() {
    let dir = scratch("count");
    let Exported { omitted_comment_threads, .. } = export_new(&doc_with_threads("Count", 3, true), &dir, "Count").unwrap();
    assert_eq!(omitted_comment_threads, 3, "resolved threads count too");
    let Exported { omitted_comment_threads, .. } = export_new(&doc_with_threads("Count", 0, false), &dir, "Count").unwrap();
    assert_eq!(omitted_comment_threads, 0);
    let _ = std::fs::remove_dir_all(&dir);
}
