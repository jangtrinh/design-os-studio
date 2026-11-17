//! An override entry of an instance that only links a grid style (`styleIdForGrid`) says nothing about the nested frame's
//! own layout grids or clipping. Opening the page and saving must not turn that link into inline grids or write
//! `frameMaskDisabled`; an entry that also states explicit grids is still read and saved.

mod common;

use common::deferred_fig::*;
use common::fig_fixture::*;
use designos_fig::{export_fig, import_fig};
use serde_json::{json, Value};

const INNER: (u64, u64) = (1, 30);
const GRID_STYLE: (u64, u64) = (1, 40);

fn columns(count: u32) -> Value {
    json!([{"axis": "X", "pattern": "STRIPES", "numSections": count, "gutterSize": 32.0, "offset": 0.0, "sectionSize": 8.0, "type": "STRETCH", "visible": true}])
}

/// A fixture whose button holds a nested frame, a grid style, and an instance override for that frame built from `entry`.
fn fixture_with_nested_override(entry: Value) -> Vec<u8> {
    let mut override_entry = json!({"guidPath": {"guids": [guid(INNER.0, INNER.1)]}, "size": {"x": 100.0, "y": 40.0}});
    for (k, v) in entry.as_object().expect("entry object") {
        override_entry[k] = v.clone();
    }
    let extra = vec![
        fig_node(INNER, BUTTON, "FRAME", "Inner", json!({"size": {"x": 100.0, "y": 40.0}})),
        fig_node(GRID_STYLE, (0, 3), "RECTANGLE", "2 column", json!({"styleType": "GRID", "key": "grid-two-columns", "layoutGrids": columns(2)})),
    ];
    fixture(json!([override_entry]), extra, Vec::new())
}

fn saved_entry_of_inner(fig: &[u8]) -> Option<Value> {
    let mut doc = import_fig(fig).expect("import");
    doc.hydrate_all_pages();
    let saved = export_fig(&doc).expect("export after opening every page");
    let want = guid(INNER.0, INNER.1);
    saved_entries(&saved, INSTANCE).into_iter().find(|e| e["guidPath"]["guids"].as_array().and_then(|g| g.last()) == Some(&want))
}

#[test]
fn an_override_that_only_links_a_grid_style_saves_no_grids_and_no_clip_flag() {
    for key in ["styleIdForGrid", "inheritGridStyleID"] {
        let link = if key == "styleIdForGrid" { json!({"guid": guid(GRID_STYLE.0, GRID_STYLE.1)}) } else { guid(GRID_STYLE.0, GRID_STYLE.1) };
        let fig = fixture_with_nested_override(json!({ key: link }));
        let entry = saved_entry_of_inner(&fig);
        let keys: Vec<String> = entry.as_ref().and_then(Value::as_object).map(|o| o.keys().cloned().collect()).unwrap_or_default();
        assert!(
            !keys.iter().any(|k| k == "layoutGrids" || k == "frameMaskDisabled"),
            "{key}: the link alone must not save inline grids or a clip flag: {entry:?}"
        );
    }
}

#[test]
fn an_override_that_states_grids_next_to_the_style_link_still_saves_them() {
    let fig = fixture_with_nested_override(json!({"styleIdForGrid": {"guid": guid(GRID_STYLE.0, GRID_STYLE.1)}, "layoutGrids": columns(2)}));
    let entry = saved_entry_of_inner(&fig).expect("the entry that states grids is saved");
    let grids = entry["layoutGrids"].as_array().expect("explicit grids are saved");
    assert_eq!(grids.len(), 1, "{entry}");
}
