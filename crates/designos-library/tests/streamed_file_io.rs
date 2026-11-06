//! A library file is written to disk and read back as a stream, so a document of several GB never exists as one
//! text in memory next to itself. The bytes and the result are the ones the text path gives.

use designos_doc::{Document, Node, Rect};
use designos_fig::testkit::{deferred_pages_fig, deferred_pages_shape, DeferredPagesShape};
use designos_library::{write_atomic_with, Library};
use std::path::PathBuf;

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("streamed-file-io-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A document with a NaN, so the count and the neutral value are part of what is compared.
fn document_with_a_nan() -> Document {
    let mut d = Document::new("streamed");
    let page = d.add_page("P");
    let id = d.alloc_id();
    let rect = d.insert(page, Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
    d.get_mut(rect).unwrap().size.x = f32::NAN;
    d
}

fn stored_path(lib: &Library, project: &str, file: &str) -> PathBuf {
    lib.path(format!("projects/{project}/files/{file}.designos"))
}

#[test]
fn a_created_file_holds_the_bytes_of_the_text_and_reports_the_same_count() {
    let dir = scratch("create");
    let lib = Library::open(dir.join("lib")).unwrap();
    let project = lib.create_project("P").unwrap();
    let doc = document_with_a_nan();
    let (text, replaced_in_text) = doc.to_json_counted().unwrap();

    let (entry, replaced) = lib.create_file_counted(&project.id, None, "streamed", &doc).unwrap();

    assert_eq!(replaced, replaced_in_text);
    assert_eq!(std::fs::read(stored_path(&lib, &project.id, &entry.id)).unwrap(), text.as_bytes());
}

#[test]
fn a_saved_file_holds_the_bytes_of_the_text_and_leaves_no_temporary_file() {
    let dir = scratch("save");
    let lib = Library::open(dir.join("lib")).unwrap();
    let project = lib.create_project("P").unwrap();
    let entry = lib.create_file(&project.id, None, "streamed", &Document::new("old")).unwrap();
    let doc = document_with_a_nan();
    let (text, replaced_in_text) = doc.to_json_counted().unwrap();

    assert_eq!(lib.save_file_counted(&entry.id, &doc).unwrap(), replaced_in_text);

    let path = stored_path(&lib, &project.id, &entry.id);
    assert_eq!(std::fs::read(&path).unwrap(), text.as_bytes());
    let temporary: Vec<_> =
        std::fs::read_dir(path.parent().unwrap()).unwrap().filter_map(|e| e.ok()).filter(|e| e.file_name().to_string_lossy().ends_with(".tmp")).collect();
    assert!(temporary.is_empty(), "no temporary file stays behind: {temporary:?}");
}

#[test]
fn a_stored_file_reopens_as_the_text_parse_gives() {
    let dir = scratch("reopen");
    let lib = Library::open(dir.join("lib")).unwrap();
    let project = lib.create_project("P").unwrap();
    let entry = lib.create_file(&project.id, None, "streamed", &document_with_a_nan()).unwrap();

    let streamed = lib.load_file(&entry.id).unwrap();

    let text = std::fs::read_to_string(stored_path(&lib, &project.id, &entry.id)).unwrap();
    assert_eq!(streamed.to_json().unwrap(), Document::from_json(&text).unwrap().to_json().unwrap());
}

#[test]
fn a_file_an_older_version_saved_with_a_null_number_still_reopens() {
    let dir = scratch("legacy");
    let lib = Library::open(dir.join("lib")).unwrap();
    let project = lib.create_project("P").unwrap();
    let entry = lib.create_file(&project.id, None, "legacy", &document_with_a_nan()).unwrap();
    // What versions before the fix wrote: serde_json's own `null` for the NaN.
    std::fs::write(stored_path(&lib, &project.id, &entry.id), serde_json::to_string(&document_with_a_nan()).unwrap()).unwrap();

    let reopened = lib.load_file(&entry.id).expect("the damaged number is repaired, not refused");

    assert_eq!(reopened.to_json().unwrap(), Document::from_json(&serde_json::to_string(&document_with_a_nan()).unwrap()).unwrap().to_json().unwrap());
}

#[test]
fn a_file_that_is_not_a_document_is_refused_with_a_parse_error() {
    let dir = scratch("broken");
    let lib = Library::open(dir.join("lib")).unwrap();
    let project = lib.create_project("P").unwrap();
    let entry = lib.create_file(&project.id, None, "broken", &document_with_a_nan()).unwrap();
    std::fs::write(stored_path(&lib, &project.id, &entry.id), b"{ not json").unwrap();

    assert!(lib.load_file(&entry.id).is_err());
}

#[test]
fn a_document_with_pages_to_load_is_refused_before_any_file_is_created() {
    let dir = scratch("refused");
    let fig = dir.join("kit.fig");
    std::fs::write(&fig, deferred_pages_fig()).unwrap();
    let lib = Library::open(dir.join("lib")).unwrap();
    let project = lib.create_project("P").unwrap();
    let mut doc = designos_library::load_external(&fig).unwrap();
    assert_eq!(deferred_pages_shape(&doc), DeferredPagesShape::deferred(), "precondition: the instances on the deferred page are empty");
    let files = dir.join("lib").join("projects").join(&project.id).join("files");
    let count = |d: &PathBuf| std::fs::read_dir(d).map(|r| r.count()).unwrap_or(0);
    let before = count(&files);

    assert!(lib.create_file_counted(&project.id, None, "kit", &doc).is_err());
    assert_eq!(count(&files), before, "nothing was written");

    doc.load_pending_pages().unwrap();
    let (entry, _) = lib.create_file_counted(&project.id, None, "kit", &doc).unwrap();
    assert_eq!(deferred_pages_shape(&lib.load_file(&entry.id).unwrap()), DeferredPagesShape::loaded());
}

#[test]
fn content_that_fails_while_it_is_written_leaves_the_old_file_and_no_temporary_file() {
    let dir = scratch("failing");
    let path = dir.join("file.designos");
    std::fs::write(&path, b"old").unwrap();

    let res = write_atomic_with(&path, |out| {
        out.write_all(b"half of the new")?;
        Err(std::io::Error::other("source failed"))
    });

    assert!(res.is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"old", "the old file is untouched");
    let names: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names, vec!["file.designos".to_string()], "no temporary file stays behind");
}

#[test]
fn content_written_in_pieces_replaces_the_file_whole() {
    let dir = scratch("pieces");
    let path = dir.join("file.designos");
    std::fs::write(&path, b"old").unwrap();

    write_atomic_with(&path, |out| {
        for piece in ["new ", "content ", "in ", "pieces"] {
            out.write_all(piece.as_bytes())?;
        }
        Ok(())
    })
    .unwrap();

    assert_eq!(std::fs::read(&path).unwrap(), b"new content in pieces");
}

#[test]
fn an_empty_write_leaves_an_empty_file() {
    let dir = scratch("empty");
    let path = dir.join("file.designos");
    std::fs::write(&path, b"old").unwrap();

    write_atomic_with(&path, |_| Ok(())).unwrap();

    assert_eq!(std::fs::read(&path).unwrap(), b"");
}
