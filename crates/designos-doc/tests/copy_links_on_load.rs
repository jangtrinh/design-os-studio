//! Opening a native file saved by an earlier release that read a Studio-written `.fig`: its nested copy layers link to
//! the layer of the innermost main. The load repairs those links and says how many; the file saved afterwards reads clean.
use designos_doc::*;
use serde_json::Value;

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
fn a_repaired_file_that_is_saved_and_loaded_again_reports_nothing_the_second_time() {
    for (depth, text, repaired) in [(1usize, DEPTH1, 1usize), (2, DEPTH2, 3)] {
        let (doc, report) = Document::from_json_with_report(text).unwrap();
        assert_eq!(report.copy_links_relinked, repaired, "depth {depth}: first load");
        let saved = doc.to_json().unwrap();
        let (again, report) = Document::from_json_with_report(&saved).unwrap();
        assert_eq!(report.copy_links_relinked, 0, "depth {depth}: second load");
        assert!(report.is_clean(), "depth {depth}: {report:?}");
        assert_eq!(again.to_json().unwrap(), saved, "depth {depth}: the second save is the first");
        let (_, streamed) = Document::from_reader_with_report(std::io::Cursor::new(saved.as_bytes())).unwrap();
        assert_eq!(streamed.copy_links_relinked, 0, "depth {depth}: a stream reads clean too");
    }
}

#[test]
fn the_saved_file_carries_no_field_of_this_repair() {
    let (doc, _) = Document::from_json_with_report(DEPTH2).unwrap();
    let saved: Value = serde_json::from_str(&doc.to_json().unwrap()).unwrap();
    let before: Value = serde_json::from_str(DEPTH2).unwrap();
    let keys = |v: &Value| v.as_object().unwrap().keys().cloned().collect::<std::collections::BTreeSet<_>>();
    let extra: Vec<_> = keys(&saved).difference(&keys(&before)).cloned().collect();
    assert!(extra.is_empty(), "the load and save added a document field: {extra:?}");
}

#[test]
fn the_repair_is_no_undo_step_and_leaves_the_document_as_a_plain_load_does() {
    let (mut repaired, report) = Document::from_json_with_report(DEPTH2).unwrap();
    assert_eq!(report.copy_links_relinked, 3);
    // the same file after its repair was saved: a load with nothing to repair
    let (plain, plain_report) = Document::from_json_with_report(&repaired.to_json().unwrap()).unwrap();
    assert_eq!(plain_report.copy_links_relinked, 0, "fixture: the saved copy of the file is a plain load");

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
