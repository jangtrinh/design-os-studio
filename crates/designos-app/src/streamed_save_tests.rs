//! A native save streams the document into the file instead of building one text of it: the bytes, the count of
//! replaced numbers and the safety of the write are the ones the text path gave.

use designos_doc::{Document, Node, Rect};
use std::path::PathBuf;

fn scratch(tag: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!("designos_streamed_save_{tag}_{}_{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn document_with_a_nan() -> Document {
    let mut d = Document::new("streamed");
    let page = d.add_page("P");
    let id = d.alloc_id();
    let rect = d.insert(page, Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
    d.get_mut(rect).unwrap().size.x = f32::NAN;
    d
}

#[test]
fn a_native_save_writes_the_bytes_of_the_text_and_counts_the_replaced_numbers() {
    let dir = scratch("bytes");
    let doc = document_with_a_nan();
    let (text, replaced_in_text) = doc.to_json_counted().unwrap();

    let (path, replaced) = crate::io::save_path_counted(&doc, &dir.join("out.designos")).expect("the save succeeds");

    assert_eq!(replaced, replaced_in_text);
    assert_eq!(std::fs::read(&path).unwrap(), text.as_bytes());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_native_save_replaces_the_old_file_whole_and_leaves_no_temporary_file() {
    let dir = scratch("replace");
    let out = dir.join("out.designos");
    std::fs::write(&out, "old content that is longer than nothing").unwrap();
    let doc = document_with_a_nan();

    crate::io::save_path_counted(&doc, &out).expect("the save succeeds");

    assert_eq!(std::fs::read(&out).unwrap(), doc.to_json().unwrap().as_bytes());
    let names: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names, vec!["out.designos".to_string()], "no temporary file stays behind");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_native_save_of_a_document_with_pages_to_load_is_refused_before_the_file_is_touched() {
    let dir = scratch("refused");
    let out = dir.join("out.designos");
    std::fs::write(&out, "old").unwrap();
    let fig = dir.join("kit.fig");
    std::fs::write(&fig, designos_fig::testkit::deferred_pages_fig()).unwrap();
    let mut doc = designos_library::load_external(&fig).unwrap();
    assert!(doc.has_pending_pages(), "precondition: a page waits for its content");

    let err = crate::io::save_path_counted(&doc, &out).expect_err("a partial document must not be written");

    assert!(err.to_string().contains("Cannot serialize"), "{err}");
    assert_eq!(std::fs::read(&out).unwrap(), b"old", "the old file is untouched");
    doc.load_pending_pages().unwrap();
    crate::io::save_path_counted(&doc, &out).expect("the loaded document saves");
    let _ = std::fs::remove_dir_all(&dir);
}
