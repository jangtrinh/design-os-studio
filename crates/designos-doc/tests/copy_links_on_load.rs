//! Opening a native file saved by an earlier release that read a Studio-written `.fig`: its nested copy layers link to
//! the layer of the innermost main. The load repairs those links once, says how many, and writes a marker so a file
//! saved afterwards is not examined again.
use designos_doc::*;
use serde_json::{json, Value};

const DEPTH1: &str = include_str!("fixtures/older-native-after-fig-import-depth1.designos.json");
const DEPTH2: &str = include_str!("fixtures/older-native-after-fig-import-depth2.designos.json");

fn fixture(depth: usize) -> &'static str {
    if depth == 1 {
        DEPTH1
    } else {
        DEPTH2
    }
}

fn named(doc: &Document, name: &str, component: bool) -> NodeId {
    doc.nodes.values().find(|n| n.name == name && matches!(n.kind, NodeKind::Component(_)) == component).map(|n| n.id).unwrap_or_else(|| panic!("no {name}"))
}

/// The `Shape` copy under the page instance `Target`.
fn target_shape(doc: &Document) -> NodeId {
    let mut cur = named(doc, "Target", false);
    loop {
        let kid = *doc.get(cur).unwrap().children.first().expect("copy layer");
        if doc.get(kid).unwrap().name == "Shape" {
            return kid;
        }
        cur = kid;
    }
}

fn green() -> Vec<Paint> {
    vec![Paint::solid(Color::rgb(0, 200, 0))]
}

fn edit_main_and_sync_everything(doc: &mut Document, same_field: bool) {
    let a = named(doc, "A", true);
    let shape = doc.get(a).unwrap().children[0];
    let mut n = doc.get(shape).unwrap().clone();
    if same_field {
        n.fills = vec![Paint::solid(Color::rgb(0, 0, 255))];
    } else {
        n.corner_radii = [5.0; 4];
    }
    Command::Update { node: Box::new(n) }.apply(doc).unwrap();
    for name in ["A", "I", "O"] {
        if let Some(c) = doc.nodes.values().find(|n| n.name == name && matches!(n.kind, NodeKind::Component(_))).map(|n| n.id) {
            sync_instances(doc, c).apply(doc).unwrap();
        }
    }
}

/// The smallest epoch from which the dirty log still answers "nothing changed since". After a plain load that is the
/// current epoch: the last thing the load did was mark the whole document dirty.
fn first_epoch_with_a_precise_answer(doc: &Document) -> u64 {
    let (mut dirty, mut removed) = (Vec::new(), Vec::new());
    (0..=doc.mutation_epoch()).find(|since| doc.dirty_changes_since(*since, &mut dirty, &mut removed)).expect("the current epoch answers")
}

#[test]
fn an_older_native_file_keeps_a_nested_override_after_a_main_edit_and_the_load_says_how_many_links_it_repaired() {
    for (depth, repaired) in [(1usize, 1usize), (2, 3)] {
        for same_field in [false, true] {
            let (mut doc, report) = Document::from_json_with_report(fixture(depth)).unwrap();
            assert_eq!(report.copy_links_relinked, repaired, "depth {depth}: the load reports the links it repaired");
            assert_eq!(report.structural_summary(), None, "depth {depth}: relinking is not a structural repair");
            let shape = target_shape(&doc);
            edit_main_and_sync_everything(&mut doc, same_field);
            let now = target_shape(&doc);
            assert_eq!(now, shape, "depth {depth} same_field {same_field}: the layer was rebuilt");
            assert_eq!(doc.get(now).unwrap().fills, green(), "depth {depth} same_field {same_field}: the override is gone");
            assert!(doc.get(now).unwrap().component.overrides.contains(&OverrideField::Fills), "depth {depth}: the flag is gone");
        }
    }
}

#[test]
fn a_file_saved_after_the_repair_carries_the_marker_and_is_not_examined_again() {
    let (doc, report) = Document::from_json_with_report(DEPTH2).unwrap();
    assert_eq!(report.copy_links_relinked, 3);
    let saved = doc.to_json().unwrap();
    let mut value: Value = serde_json::from_str(&saved).unwrap();
    assert_eq!(value["copy_links_checked"], json!(true), "the saved file carries the marker");

    // Break one link by hand in the saved file. Only a load that examines the copies again could repair it.
    let shape = target_shape(&doc);
    let main_shape = doc.get(named(&doc, "A", true)).unwrap().children[0];
    let right = doc.get(shape).unwrap().component.source.expect("linked");
    assert_ne!(right, main_shape);
    value["nodes"][shape.0.to_string()]["component"]["source"] = json!(main_shape.0);
    let broken = value.to_string();

    let (again, report) = Document::from_json_with_report(&broken).unwrap();
    assert_eq!(report.copy_links_relinked, 0, "the marker keeps the pass off");
    assert_eq!(again.get(shape).unwrap().component.source, Some(main_shape), "the planted link stays as it is: no pass ran");
    let (streamed, report) = Document::from_reader_with_report(std::io::Cursor::new(broken.as_bytes())).unwrap();
    assert_eq!(report.copy_links_relinked, 0, "a file read as a stream skips the pass too");
    assert_eq!(streamed.get(shape).unwrap().component.source, Some(main_shape));

    // the same file without the marker is examined and repaired
    value["copy_links_checked"] = json!(false);
    let (repaired, report) = Document::from_json_with_report(&value.to_string()).unwrap();
    assert_eq!(report.copy_links_relinked, 1);
    assert_eq!(repaired.get(shape).unwrap().component.source, Some(right));
}

#[test]
fn a_new_document_is_saved_with_the_marker_also_when_a_stale_work_flag_is_dropped_on_save() {
    let mut doc = Document::new("fresh");
    let value: Value = serde_json::from_str(&doc.to_json().unwrap()).unwrap();
    assert_eq!(value["copy_links_checked"], json!(true), "a document made by this release needs no examination");

    // an `in_progress` flag of a layer that no longer exists is dropped through a separate writer path
    doc.in_progress.insert(NodeId(99_999), InProgress::new(None, None, 1));
    let value: Value = serde_json::from_str(&doc.to_json().unwrap()).unwrap();
    assert_eq!(value["in_progress"], Value::Null, "fixture: the stale flag was dropped by the filtered writer");
    assert_eq!(value["copy_links_checked"], json!(true), "the filtered writer carries the marker too");
}

#[test]
fn the_previous_release_opens_a_file_that_carries_the_marker() {
    // The previous release has no such field and no `deny_unknown_fields` on the document, so it reads a file with an
    // unknown field next to the other fields. This is the same shape of file, with the marker and one more unknown field.
    let mut value: Value = serde_json::from_str(&Document::new("compat").to_json().unwrap()).unwrap();
    value["copy_links_checked"] = json!(true);
    value["a_field_of_a_later_release"] = json!({"x": [1, 2, 3]});
    let text = value.to_string();
    let (doc, report) = Document::from_json_with_report(&text).unwrap();
    assert_eq!(doc.name, "compat");
    assert!(report.is_clean(), "{report:?}");
    Document::from_reader_with_report(std::io::Cursor::new(text.as_bytes())).expect("read as a stream");
}

#[test]
fn the_repair_is_no_undo_step_and_leaves_the_document_as_a_plain_load_does() {
    let (mut repaired, report) = Document::from_json_with_report(DEPTH2).unwrap();
    assert_eq!(report.copy_links_relinked, 3);
    let marked = DEPTH2.replacen("\"next_id\"", "\"copy_links_checked\":true,\"next_id\"", 1);
    let (plain, plain_report) = Document::from_json_with_report(&marked).unwrap();
    assert_eq!(plain_report.copy_links_relinked, 0, "fixture: the marked copy of the file is a plain load");

    for (label, doc) in [("repaired", &repaired), ("plain", &plain)] {
        assert_eq!(first_epoch_with_a_precise_answer(doc), doc.mutation_epoch(), "{label}: the load ended by marking the whole document dirty");
    }

    let shape = target_shape(&repaired);
    let relinked = repaired.get(shape).unwrap().component.source;
    let mut history = History::default();
    assert!(!history.can_undo() && !history.can_redo(), "no undo or redo step was made by the load");
    assert!(history.is_clean(), "the history is clean as after a plain load");
    assert!(!history.undo(&mut repaired).unwrap(), "there is nothing to undo");
    assert_eq!(repaired.get(shape).unwrap().component.source, relinked, "undo does not take the repair back");

    // an edit and its undo leave the repair where it is
    let mut n = repaired.get(shape).unwrap().clone();
    n.opacity = 0.5;
    history.exec(&mut repaired, Command::Update { node: Box::new(n) }).unwrap();
    assert!(history.undo(&mut repaired).unwrap());
    assert!(!history.can_undo(), "the edit was the only step");
    assert_eq!(repaired.get(shape).unwrap().component.source, relinked);
}
