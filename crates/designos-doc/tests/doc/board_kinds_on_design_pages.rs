//! Whiteboard items (sticky, connector, shape with text, stamp, section) placed on a design page,
//! at the top level, inside a frame and inside a main component, survive load, repair, lint and
//! save exactly; damaged board nodes are still repaired the way every other node is.

use designos_doc::lint::{lint, ApproxLintEnv, LintConfig};
use designos_doc::*;
use std::collections::BTreeMap;

struct Fixture {
    doc: Document,
    page: NodeId,
    frame: NodeId,
    component: NodeId,
    sticky: NodeId,
    nested_sticky: NodeId,
    connector: NodeId,
    stamp: NodeId,
    section: NodeId,
    shape: NodeId,
    component_sticky: NodeId,
}

/// A design file (no board kind) with one of each board item: a sticky and a connector between
/// two frames, a stamp in a frame, a section, a shape with text, and a sticky inside a component.
fn design_page_with_board_items() -> Fixture {
    let mut doc = Document::new("Design with board items");
    assert_eq!(doc.kind, DocumentKind::Design);
    let page = doc.add_page("Page 1");

    let frame = doc.alloc_id();
    doc.insert(page, Node::frame(frame, Rect::new(0.0, 0.0, 400.0, 300.0))).unwrap();
    let other = doc.alloc_id();
    doc.insert(page, Node::frame(other, Rect::new(600.0, 0.0, 400.0, 300.0))).unwrap();

    let sticky = doc.alloc_id();
    doc.insert(page, Node::sticky(sticky, Rect::new(0.0, 400.0, 160.0, 160.0), "Top level")).unwrap();

    let nested_sticky = doc.alloc_id();
    doc.insert(frame, Node::sticky(nested_sticky, Rect::new(20.0, 20.0, 140.0, 140.0), "In a frame")).unwrap();
    let stamp = doc.alloc_id();
    doc.insert(frame, Node::stamp(stamp, Vec2::new(200.0, 40.0), "heart", 64.0)).unwrap();

    let section = doc.alloc_id();
    let mut section_node = Node::new(section, NodeKind::Section);
    section_node.name = "Ideas".into();
    section_node.transform = Affine::translate(0.0, 700.0);
    section_node.size = Vec2::new(500.0, 400.0);
    doc.insert(page, section_node).unwrap();
    let shape = doc.alloc_id();
    doc.insert(section, Node::shape_with_text(shape, Rect::new(20.0, 20.0, 120.0, 80.0), ShapeKind::Star, "In a section")).unwrap();

    let connector = doc.alloc_id();
    let props = ConnectorProps { start: ConnectorEnd::attached(frame, Magnet::Right), end: ConnectorEnd::attached(other, Magnet::Left), ..Default::default() };
    doc.insert(page, Node::connector(connector, props)).unwrap();

    let component = doc.alloc_id();
    doc.insert(page, Node::frame(component, Rect::new(0.0, 1200.0, 300.0, 300.0))).unwrap();
    let component_sticky = doc.alloc_id();
    doc.insert(component, Node::sticky(component_sticky, Rect::new(10.0, 10.0, 120.0, 120.0), "In a component")).unwrap();
    let make = create_component(&mut doc, component);
    make.apply(&mut doc).unwrap();
    assert!(matches!(doc.get(component).unwrap().kind, NodeKind::Component(_)));

    Fixture { doc, page, frame, component, sticky, nested_sticky, connector, stamp, section, shape, component_sticky }
}

fn board_ids(f: &Fixture) -> [NodeId; 7] {
    [f.sticky, f.nested_sticky, f.connector, f.stamp, f.section, f.shape, f.component_sticky]
}

#[test]
fn board_items_on_a_design_page_load_save_and_reload_with_identical_nodes() {
    let f = design_page_with_board_items();
    let first = f.doc.to_json().unwrap();

    let (loaded, report) = Document::from_json_with_report(&first).unwrap();
    assert!(report.is_clean(), "a well-formed file needs no repair: {report:?}");
    assert_eq!(loaded.kind, DocumentKind::Design);
    for id in board_ids(&f) {
        assert_eq!(loaded.get(id), f.doc.get(id), "node {id:?} changed through load");
    }
    assert_eq!(loaded.get(f.nested_sticky).unwrap().parent, Some(f.frame));
    assert_eq!(loaded.get(f.component_sticky).unwrap().parent, Some(f.component));
    assert_eq!(loaded.get(f.shape).unwrap().parent, Some(f.section));
    assert_eq!(loaded.get(f.stamp).unwrap().parent, Some(f.frame));
    assert_eq!(loaded.get(f.section).unwrap().parent, Some(f.page));

    let second = loaded.to_json().unwrap();
    assert_eq!(first, second, "saving the loaded file must reproduce the same bytes");
    let reloaded = Document::from_json(&second).unwrap();
    for id in board_ids(&f) {
        assert_eq!(reloaded.get(id), f.doc.get(id));
    }
}

#[test]
fn a_design_file_with_board_items_saved_without_a_kind_key_still_reads_as_design() {
    let f = design_page_with_board_items();
    let mut v: serde_json::Value = serde_json::from_str(&f.doc.to_json().unwrap()).unwrap();
    // A file saved before documents had a kind carries no `kind` key.
    v.as_object_mut().unwrap().remove("kind");
    let old: Document = serde_json::from_value(v).unwrap();
    assert_eq!(old.kind, DocumentKind::Design);
    for id in board_ids(&f) {
        assert_eq!(old.get(id), f.doc.get(id), "node {id:?} lost by the kind-less shape");
    }
}

#[test]
fn a_design_file_with_board_items_adds_no_top_level_key_a_plain_design_file_lacks() {
    let f = design_page_with_board_items();
    let keys = |d: &Document| -> Vec<String> {
        let v: serde_json::Value = serde_json::from_str(&d.to_json().unwrap()).unwrap();
        v.as_object().unwrap().keys().cloned().collect()
    };
    let plain = {
        let mut d = Document::new("Plain");
        let p = d.add_page("Page 1");
        let fr = d.alloc_id();
        d.insert(p, Node::frame(fr, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        d
    };
    let plain_keys = keys(&plain);
    for k in keys(&f.doc) {
        assert!(plain_keys.contains(&k), "board items add the top-level key `{k}` to a design file");
    }
}

/// Reads the data object of a board layer (`Sticky`, `Connector`, `ShapeWithText`) out of saved JSON.
fn kind_data<'a>(saved: &'a mut serde_json::Value, id: NodeId, variant: &str) -> &'a mut serde_json::Map<String, serde_json::Value> {
    saved["nodes"][id.0.to_string()]["kind"][variant].as_object_mut().unwrap_or_else(|| panic!("{variant} data of {id:?}"))
}

#[test]
fn optional_fields_a_newer_release_stores_on_board_items_survive_load_and_save() {
    let f = design_page_with_board_items();
    let mut saved: serde_json::Value = serde_json::from_str(&f.doc.to_json().unwrap()).unwrap();
    kind_data(&mut saved, f.sticky, "Sticky").insert("vote_count".into(), serde_json::json!(5));
    kind_data(&mut saved, f.shape, "ShapeWithText").insert("vote_count".into(), serde_json::json!(2));

    let (loaded, report) = Document::from_json_with_report(&saved.to_string()).unwrap();
    assert!(report.is_clean());
    let mut again: serde_json::Value = serde_json::from_str(&loaded.to_json().unwrap()).unwrap();
    assert_eq!(kind_data(&mut again, f.sticky, "Sticky").get("vote_count"), Some(&serde_json::json!(5)));
    assert_eq!(kind_data(&mut again, f.shape, "ShapeWithText").get("vote_count"), Some(&serde_json::json!(2)));
    assert_eq!(again, saved, "a load and save changes nothing else");
}

#[test]
fn board_items_without_extra_fields_save_without_any() {
    let f = design_page_with_board_items();
    let mut saved: serde_json::Value = serde_json::from_str(&f.doc.to_json().unwrap()).unwrap();
    let sticky = kind_data(&mut saved, f.sticky, "Sticky");
    let keys: Vec<&str> = sticky.keys().map(String::as_str).collect();
    assert_eq!(keys, ["author", "color", "show_author", "text"], "no new key appears in an ordinary sticky");
}

#[test]
fn metadata_stored_on_a_board_node_survives_a_save_and_a_load() {
    let mut f = design_page_with_board_items();
    f.doc.get_mut(f.sticky).unwrap().plugin_data.insert("vote_count".into(), "3".into());
    let loaded = Document::from_json(&f.doc.to_json().unwrap()).unwrap();
    assert_eq!(loaded.get(f.sticky).unwrap().plugin_data.get("vote_count").map(String::as_str), Some("3"));
}

#[test]
fn a_board_item_saved_with_a_null_number_still_opens_with_the_neutral_value() {
    let f = design_page_with_board_items();
    let mut saved: serde_json::Value = serde_json::from_str(&f.doc.to_json().unwrap()).unwrap();
    kind_data(&mut saved, f.sticky, "Sticky")["text"]["style"]["font_size"] = serde_json::Value::Null;
    kind_data(&mut saved, f.sticky, "Sticky").insert("vote_count".into(), serde_json::json!(1));
    let (loaded, report) = Document::from_json_with_report(&saved.to_string()).unwrap();
    assert_eq!(report.non_finite_numbers, 1);
    let NodeKind::Sticky(s) = &loaded.get(f.sticky).unwrap().kind else { panic!("sticky kept") };
    assert_eq!(s.text.style.font_size, 0.0);
    assert_eq!(s.extra.get("vote_count"), Some(&serde_json::json!(1)));
}

#[test]
fn a_damaged_board_node_is_repaired_like_any_other_node() {
    let f = design_page_with_board_items();
    let mut v: serde_json::Value = serde_json::from_str(&f.doc.to_json().unwrap()).unwrap();
    // The sticky inside the frame names a wrong parent; the frame lists a child that does not exist;
    // the stamp is listed twice; the connector's id key disagrees with its stored id.
    v["nodes"][f.nested_sticky.0.to_string()]["parent"] = serde_json::json!(f.page.0);
    v["nodes"][f.frame.0.to_string()]["children"].as_array_mut().unwrap().push(serde_json::json!(9_999));
    let stamp_entry = serde_json::json!(f.stamp.0);
    v["nodes"][f.frame.0.to_string()]["children"].as_array_mut().unwrap().push(stamp_entry);
    v["nodes"][f.connector.0.to_string()]["id"] = serde_json::json!(8_888);

    let (fixed, report) = Document::from_json_with_report(&v.to_string()).unwrap();
    assert_eq!(report.dangling_ids, 1, "{report:?}");
    assert_eq!(report.duplicate_claims, 1, "{report:?}");
    assert_eq!(report.parents_rebuilt, 1, "{report:?}");
    assert_eq!(report.ids_realigned, 1, "{report:?}");
    for id in board_ids(&f) {
        assert_eq!(fixed.get(id), f.doc.get(id), "repair changed node {id:?}");
    }
    assert!(fixed.get(f.frame).unwrap().children.contains(&f.nested_sticky));
}

#[test]
fn a_connector_flowing_in_an_auto_layout_frame_on_a_design_page_is_still_made_absolute_and_kept() {
    let mut f = design_page_with_board_items();
    {
        let fp = f.doc.get_mut(f.frame).unwrap().kind.frame_props_mut().unwrap();
        fp.auto_layout = Some(AutoLayout::default());
    }
    let inner = f.doc.alloc_id();
    let c = Node::connector(inner, ConnectorProps::default());
    f.doc.insert(f.frame, c).unwrap();
    f.doc.get_mut(inner).unwrap().layout_child.absolute = false;
    let (loaded, report) = Document::from_json_with_report(&f.doc.to_json().unwrap()).unwrap();
    assert_eq!(report.connectors_made_absolute, 1);
    assert!(matches!(loaded.get(inner).unwrap().kind, NodeKind::Connector(_)));
    assert!(loaded.get(inner).unwrap().layout_child.absolute);
    assert!(loaded.get(f.sticky).is_some());
}

#[test]
fn lint_and_its_fixes_keep_every_board_items_kind_and_content_on_a_design_page() {
    let f = design_page_with_board_items();
    // Linting a page with board items completes over every layer.
    let report = lint(&f.doc, &LintConfig::default(), &ApproxLintEnv);
    assert!(report.complete);
    assert!(report.scanned_nodes >= board_ids(&f).len());

    // Positive control: the same fix pass does remove an empty, invisible container.
    let mut fixed = f.doc.clone();
    let empty = fixed.alloc_id();
    let mut hollow = Node::frame(empty, Rect::new(900.0, 900.0, 50.0, 50.0));
    hollow.fills.clear();
    fixed.insert(f.page, hollow).unwrap();
    for scope in [None, Some(f.page)] {
        designos_doc::lint::lint_fix_document(&mut fixed, scope, None);
    }
    assert!(fixed.get(empty).is_none(), "the fix pass must remove an empty container, or this test proves nothing");
    // Geometry is not compared: the grid fix may still snap a board item's size, as it does for any layer.
    for id in board_ids(&f) {
        let before = f.doc.get(id).expect("fixture node");
        let after = fixed.get(id).unwrap_or_else(|| panic!("lint fix removed {id:?}"));
        assert_eq!(before.kind, after.kind, "lint fix changed the kind or content of {id:?}");
        assert_eq!(before.parent, after.parent, "lint fix moved {id:?}");
    }
    let count_kinds = |d: &Document| -> BTreeMap<&'static str, usize> {
        let mut m = BTreeMap::new();
        for n in d.nodes.values() {
            *m.entry(n.kind.default_name()).or_insert(0) += 1;
        }
        m
    };
    let (b, a) = (count_kinds(&f.doc), count_kinds(&fixed));
    for k in ["Sticky note", "Connector", "Stamp", "Section", "Shape"] {
        assert!(a.get(k).copied().unwrap_or(0) >= b.get(k).copied().unwrap_or(0), "lint fix lowered the number of `{k}` layers: {b:?} -> {a:?}");
    }
}

/// Native files saved by earlier releases and kept in the repository.
const SAVED_FILES: [&str; 8] = [
    "tests/fixtures/release-0.4-landing.designos",
    "tests/fixtures/release-0.6-landing.designos",
    "tests/fixtures/release-0.4-variables-code-syntax.designos",
    "tests/fixtures/design-page-board-items.designos",
    "../designos-app/tests/fixtures/designos-studio-landing.designos",
    "../designos-app/tests/fixtures/frame-labels-1197.designos",
    "../designos-app/tests/fixtures/mask-types.designos",
    "../designos-app/assets/starter/designos-studio-starter.designos",
];

#[test]
fn saved_files_from_earlier_releases_open_clean_and_save_stable() {
    for rel in SAVED_FILES {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
        println!("checking {rel}");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{rel}: {e}"));
        let (doc, report) = Document::from_json_with_report(&text).unwrap_or_else(|e| panic!("{rel}: {e}"));
        assert!(report.is_clean(), "{rel}: {report:?}");
        let once = doc.to_json().unwrap();
        let twice = Document::from_json(&once).unwrap().to_json().unwrap();
        assert_eq!(once, twice, "{rel}: a second save differs from the first");
        let a: serde_json::Value = serde_json::from_str(&text).unwrap();
        let b: serde_json::Value = serde_json::from_str(&once).unwrap();
        let mut diffs = Vec::new();
        json_diff(&a, &b, String::new(), &mut diffs);
        // Style metadata is the one thing a load adds to older files: a record per text style
        // (paragraph fields, and the name and property of a style saved without any).
        let before = diffs.len();
        diffs.retain(|d| !d.starts_with("/style_meta/"));
        println!("  {rel}: {} style metadata entries added, {} other differences", before - diffs.len(), diffs.len());
        assert!(diffs.is_empty(), "{rel}: the saved file differs from the file that was opened at {:?}", &diffs[..diffs.len().min(10)]);
    }
}

/// Collects the JSON paths at which `a` and `b` differ (long values are not printed).
fn json_diff(a: &serde_json::Value, b: &serde_json::Value, path: String, out: &mut Vec<String>) {
    use serde_json::Value::{Array, Object};
    match (a, b) {
        (Object(x), Object(y)) => {
            for k in x.keys().chain(y.keys().filter(|k| !x.contains_key(*k))) {
                let sub = format!("{path}/{k}");
                match (x.get(k), y.get(k)) {
                    (Some(l), Some(r)) => json_diff(l, r, sub, out),
                    _ => out.push(sub),
                }
            }
        }
        (Array(x), Array(y)) if x.len() == y.len() => {
            for (i, (l, r)) in x.iter().zip(y).enumerate() {
                json_diff(l, r, format!("{path}[{i}]"), out);
            }
        }
        _ if a == b => {}
        _ => out.push(path),
    }
}

fn fixture_text(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn the_design_file_fixture_with_board_items_opens_clean_and_saves_the_same_data() {
    let text = fixture_text("design-page-board-items.designos");
    let (doc, report) = Document::from_json_with_report(&text).unwrap();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(doc.kind, DocumentKind::Design);
    let names: Vec<&str> = doc.nodes.values().map(|n| n.kind.default_name()).collect();
    for want in ["Sticky note", "Connector", "Stamp", "Section", "Shape", "Component"] {
        assert!(names.contains(&want), "fixture lacks a {want}: {names:?}");
    }
    let before: serde_json::Value = serde_json::from_str(&text).unwrap();
    let after: serde_json::Value = serde_json::from_str(&doc.to_json().unwrap()).unwrap();
    assert_eq!(before, after);
}

#[test]
fn the_fixture_with_fields_from_a_newer_release_keeps_them_through_load_and_save() {
    let text = fixture_text("design-page-board-items-newer-fields.designos");
    let (doc, report) = Document::from_json_with_report(&text).unwrap();
    assert!(report.is_clean(), "{report:?}");
    let before: serde_json::Value = serde_json::from_str(&text).unwrap();
    let after: serde_json::Value = serde_json::from_str(&doc.to_json().unwrap()).unwrap();
    assert_eq!(before, after, "a field this version does not know was dropped");
    let carried = doc.nodes.values().filter(|n| matches!(&n.kind, NodeKind::Sticky(s) if s.extra.contains_key("vote_count"))).count();
    assert_eq!(carried, 1, "the top-level sticky carries its vote count");
}
