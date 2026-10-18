//! A number that is not finite (NaN or infinity) must never leave a library file that cannot be
//! reopened. serde_json writes such a float as `null` and then refuses `null` for an `f32`, so one
//! bad value used to turn an imported file into `json: invalid type: null, expected f32`.

use designos_doc::{Document, Node, NodeKind, Rect};
use designos_library::Library;
use std::path::PathBuf;

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("non-finite-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A document with a NaN in one f32 field of three different kinds (size, opacity, corner radius)
/// and an infinite x position.
fn poisoned_document() -> Document {
    let mut d = Document::new("poisoned");
    let page = d.add_page("P");
    let id = d.alloc_id();
    let rect = d.insert(page, Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
    let id = d.alloc_id();
    let other = d.insert(page, Node::rectangle(id, Rect::new(5.0, 5.0, 10.0, 10.0))).unwrap();
    {
        let n = d.get_mut(rect).unwrap();
        n.size.x = f32::NAN;
        n.opacity = f32::NAN;
        n.corner_radii[2] = f32::NAN;
    }
    d.get_mut(other).unwrap().transform.tx = f32::INFINITY;
    d
}

fn rectangles(d: &Document) -> Vec<&Node> {
    d.nodes.values().filter(|n| !matches!(n.kind, NodeKind::Page { .. })).collect()
}

#[test]
fn a_nan_written_by_the_library_can_be_reopened() {
    let dir = scratch("reopen");
    let lib = Library::open(dir.join("lib")).unwrap();
    let project = lib.create_project("P").unwrap();
    let entry = lib.create_file(&project.id, None, "poisoned", &poisoned_document()).unwrap();
    let loaded = lib.load_file(&entry.id).unwrap_or_else(|e| panic!("reopen failed: {e}"));
    assert_eq!(loaded.nodes.len(), 3);
    let stored = std::fs::read_to_string(lib.path(format!("projects/{}/files/{}.designos", project.id, entry.id))).unwrap();
    assert!(Document::from_json_with_report(&stored).unwrap().1.is_clean(), "the stored file needs no repair");
}

#[test]
fn saving_an_edited_document_with_a_nan_keeps_the_file_openable() {
    let dir = scratch("save");
    let lib = Library::open(dir.join("lib")).unwrap();
    let project = lib.create_project("P").unwrap();
    let entry = lib.create_file(&project.id, None, "clean", &Document::new("clean")).unwrap();
    lib.save_file(&entry.id, &poisoned_document()).unwrap();
    assert_eq!(lib.load_file(&entry.id).unwrap().nodes.len(), 3);
}

/// A Figma REST file whose numbers do not fit an f32: they become infinity on import.
const OVERFLOWING_REST_FILE: &str = r#"{
  "name": "Overflow", "version": "1", "schemaVersion": 0,
  "document": {"id": "0:0", "name": "Document", "type": "DOCUMENT", "children": [
    {"id": "0:1", "name": "Page", "type": "CANVAS", "children": [
      {"id": "1:1", "name": "Huge", "type": "RECTANGLE", "size": {"x": 1e39, "y": 20},
       "relativeTransform": [[1, 0, -1e39], [0, 1, 4]], "strokeWeight": 1e39, "cornerRadius": 1e39}]}]}
}"#;

#[test]
fn a_rest_import_with_overflowing_numbers_opens_and_matches_the_stored_file() {
    let dir = scratch("rest-import");
    let imported = designos_fig::import_rest_json(OVERFLOWING_REST_FILE).unwrap();
    assert!(imported.to_json_counted().unwrap().1 >= 4, "the importer itself turns the overflowing numbers into infinity");
    let lib = Library::open(dir.join("lib")).unwrap();
    let project = lib.create_project("P").unwrap();
    let source = dir.join("overflow.json");
    std::fs::write(&source, OVERFLOWING_REST_FILE).unwrap();

    let (entry, returned) = lib.import_file_with_doc(&source, &project.id, None).unwrap();
    let reopened = lib.load_file(&entry.id).unwrap_or_else(|e| panic!("reopen failed: {e}"));

    assert_eq!(returned.to_json().unwrap(), reopened.to_json().unwrap(), "the returned document is what is stored");
    assert_eq!(returned.to_json_counted().unwrap().1, 0, "nothing non-finite is left in the returned document");
    let huge = rectangles(&reopened).into_iter().find(|n| n.name == "Huge").expect("the imported rectangle");
    assert_eq!((huge.size.x, huge.size.y, huge.transform.tx, huge.transform.ty), (0.0, 20.0, 0.0, 4.0));
}

#[test]
fn a_file_saved_by_an_older_version_with_null_numbers_opens() {
    let dir = scratch("legacy");
    let lib = Library::open(dir.join("lib")).unwrap();
    let project = lib.create_project("P").unwrap();
    let entry = lib.create_file(&project.id, None, "legacy", &Document::new("legacy")).unwrap();
    // The bytes version 0.2.0 wrote for a document holding a NaN: serde_json's own `null`.
    let legacy = serde_json::to_string(&poisoned_document()).unwrap();
    assert!(legacy.contains("null"));
    std::fs::write(lib.path(format!("projects/{}/files/{}.designos", project.id, entry.id)), &legacy).unwrap();

    let loaded = lib.load_file(&entry.id).unwrap_or_else(|e| panic!("legacy file did not open: {e}"));
    assert_eq!(loaded.nodes.len(), 3);
    let report = Document::from_json_with_report(&legacy).unwrap().1;
    assert_eq!(report.non_finite_numbers, 4, "size, opacity, corner radius and the x position");

    // The next save repairs the file on disk.
    lib.save_file(&entry.id, &loaded).unwrap();
    let saved = std::fs::read_to_string(lib.path(format!("projects/{}/files/{}.designos", project.id, entry.id))).unwrap();
    assert!(Document::from_json_with_report(&saved).unwrap().1.is_clean());
}

#[test]
fn a_counted_save_reports_the_replaced_numbers_from_the_one_serialization_it_performs() {
    let dir = scratch("save-counted");
    let lib = Library::open(dir.join("lib")).unwrap();
    let project = lib.create_project("P").unwrap();
    let entry = lib.create_file(&project.id, None, "clean", &Document::new("clean")).unwrap();
    let texts_built = designos_doc::finite_json::serialize_calls_on_this_thread;

    let before = texts_built();
    let replaced = lib.save_file_counted(&entry.id, &poisoned_document()).unwrap();
    assert_eq!(replaced, 4, "size, opacity, corner radius and the x position");
    assert_eq!(texts_built() - before, 1, "counting must not cost a second serialization");
    let stored = std::fs::read_to_string(lib.path(format!("projects/{}/files/{}.designos", project.id, entry.id))).unwrap();
    assert!(Document::from_json_with_report(&stored).unwrap().1.is_clean(), "the stored file needs no repair");

    let before = texts_built();
    assert_eq!(lib.save_file_counted(&entry.id, &Document::new("clean")).unwrap(), 0);
    assert_eq!(texts_built() - before, 1);
}
