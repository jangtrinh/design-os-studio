//! Grid tracks, cell anchors and auto-tracks of an exported `.fig` use Figma's own shapes (GUID-keyed maps, a GUID
//! anchor, an enum auto-track), and what that shape cannot hold survives in a carrier entry.

mod common;
use common::{assert_grids_survived, auto_layout, find, grid_document};
use designos_fig::{decode_fig_to_json, export_fig, import_fig};
use serde_json::Value;

fn node<'a>(decoded: &'a Value, name: &str) -> &'a Value {
    decoded["message"]["nodeChanges"].as_array().unwrap().iter().find(|n| n["name"] == name).unwrap_or_else(|| panic!("no node change named {name}"))
}

fn guid_of(v: &Value) -> (u64, u64) {
    (v["sessionID"].as_u64().expect("sessionID"), v["localID"].as_u64().expect("localID"))
}

/// Track ids of a position map, in position order.
fn ordered_ids(map: &Value) -> Vec<(u64, u64)> {
    let mut entries: Vec<(&str, (u64, u64))> =
        map["entries"].as_array().expect("a track map").iter().map(|e| (e["position"].as_str().expect("position"), guid_of(&e["id"]))).collect();
    entries.sort();
    entries.into_iter().map(|(_, id)| id).collect()
}

fn carrier<'a>(n: &'a Value, field: &str) -> Option<&'a str> {
    let key = format!("field:{field}");
    n["pluginData"].as_array()?.iter().find(|e| e["pluginID"] == "designos-studio" && e["key"] == key.as_str()).and_then(|e| e["value"].as_str())
}

#[test]
fn tracks_are_guid_keyed_maps_and_cells_name_their_track() {
    let decoded = decode_fig_to_json(&export_fig(&grid_document()).unwrap()).unwrap();
    let main = node(&decoded, "Main");
    let cols = ordered_ids(&main["gridColumns"]);
    let rows = ordered_ids(&main["gridRows"]);
    assert_eq!((cols.len(), rows.len()), (3, 2));
    let positions: Vec<&str> = main["gridColumns"]["entries"].as_array().unwrap().iter().map(|e| e["position"].as_str().unwrap()).collect();
    assert_eq!(positions, ["!", "\"", "#"], "positions are fractional indexes in track order");
    let mut all: Vec<_> = cols.iter().chain(&rows).collect();
    all.sort();
    all.dedup();
    assert_eq!(all.len(), 5, "every track has an id of its own");

    // Sizing is keyed by the same ids; a track carries the same sizing as minimum and maximum.
    let sized: Vec<((u64, u64), String, f64)> = main["gridColumnsSizing"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            assert_eq!(e["trackSize"]["minSizing"], e["trackSize"]["maxSizing"]);
            (guid_of(&e["id"]), e["trackSize"]["minSizing"]["type"].as_str().unwrap().to_string(), e["trackSize"]["minSizing"]["value"].as_f64().unwrap())
        })
        .collect();
    assert_eq!(sized, vec![(cols[0], "FIXED".into(), 120.0), (cols[1], "FLEX".into(), 2.0), (cols[2], "HUG".into(), 1.0)]);
    assert_eq!(main["gridRowsSizing"]["entries"][1]["trackSize"]["maxSizing"]["type"], "FIXED");

    // Figma's auto-track is an enum; Studio's 50 px auto-track rides in a carrier beside it.
    assert_eq!(main["gridAutoTracks"], "ROWS");
    assert_eq!(carrier(main, "gridAutoTracks"), Some("\"50px\""));
    assert_eq!(node(&decoded, "Wide")["gridAutoTracks"], "ROWS");
    assert!(carrier(node(&decoded, "Wide"), "gridAutoTracks").is_none(), "plain ROWS needs no carrier");

    // A cell inside the tracks names them by id; no carrier is written for it.
    let cell = node(&decoded, "InTracks");
    assert_eq!(guid_of(&cell["gridColumnAnchor"]), cols[2]);
    assert_eq!(guid_of(&cell["gridRowAnchor"]), rows[1]);
    assert!(carrier(cell, "gridColumnAnchorIndex").is_none());
    assert!(node(&decoded, "Unpinned").get("gridColumnAnchor").is_none());
}

#[test]
fn a_cell_outside_the_tracks_and_a_grid_past_the_entry_cap_use_carriers() {
    let decoded = decode_fig_to_json(&export_fig(&grid_document()).unwrap()).unwrap();
    let past = node(&decoded, "PastTracks");
    assert!(past.get("gridColumnAnchor").is_none() && past.get("gridRowAnchor").is_none(), "no track to name");
    assert_eq!((carrier(past, "gridColumnAnchorIndex"), carrier(past, "gridRowAnchorIndex")), (Some("5"), Some("4")));

    let wide = node(&decoded, "Wide");
    assert_eq!(wide["gridColumns"]["entries"].as_array().unwrap().len(), 256, "entries are capped");
    assert_eq!(carrier(wide, "gridColumns"), Some("300"), "the real count is carried");
    let far = node(&decoded, "FarCell");
    assert!(far.get("gridColumnAnchor").is_none(), "a grid past the cap names no track");
    assert_eq!(carrier(far, "gridColumnAnchorIndex"), Some("299"));
}

#[test]
fn an_override_names_the_track_of_the_grid_that_restates_its_tracks() {
    let decoded = decode_fig_to_json(&export_fig(&grid_document()).unwrap()).unwrap();
    // The instance restates its grid, so the cell it moves names the instance's own tracks.
    let inst = node(&decoded, "GridInstance");
    let cols = ordered_ids(&inst["gridColumns"]);
    let rows = ordered_ids(&inst["gridRows"]);
    let comp = node(&decoded, "GridComp");
    assert!(ordered_ids(&comp["gridColumns"]).iter().all(|id| !cols.contains(id)), "the component's tracks are other ids");
    let overrides = inst["symbolData"]["symbolOverrides"].as_array().expect("symbol overrides");
    let moved = overrides.iter().find(|o| o.get("gridColumnAnchor").is_some()).expect("an override that moves the cell");
    assert_eq!(guid_of(&moved["gridColumnAnchor"]), cols[2]);
    assert_eq!(guid_of(&moved["gridRowAnchor"]), rows[1]);
}

#[test]
fn an_override_under_a_grid_that_does_not_restate_names_the_components_track() {
    let decoded = decode_fig_to_json(&export_fig(&grid_document()).unwrap()).unwrap();
    let master = node(&decoded, "InnerGrid");
    let (cols, rows) = (ordered_ids(&master["gridColumns"]), ordered_ids(&master["gridRows"]));
    let overrides = node(&decoded, "OuterInstance")["symbolData"]["symbolOverrides"].as_array().expect("symbol overrides").clone();
    let moved = overrides.iter().find(|o| o.get("gridColumnAnchor").is_some()).expect("an override that moves the cell");
    assert_eq!(guid_of(&moved["gridColumnAnchor"]), cols[1]);
    // The row is past the component's two rows: no track to name, so the index rides in a carrier on the override.
    assert!(moved.get("gridRowAnchor").is_none(), "{moved}");
    assert_eq!(carrier(moved, "gridRowAnchorIndex"), Some("7"));
    assert!(rows.len() == 2);
}

#[test]
fn every_grid_shape_round_trips_without_loss() {
    let rt = import_fig(&export_fig(&grid_document()).unwrap()).unwrap();
    assert_grids_survived(&rt);
}

#[test]
fn a_second_grid_round_trip_changes_nothing() {
    let once = import_fig(&export_fig(&grid_document()).unwrap()).unwrap();
    let twice = import_fig(&export_fig(&once).unwrap()).unwrap();
    for name in ["Main", "Wide", "GridComp"] {
        assert_eq!(auto_layout(&once, name), auto_layout(&twice, name), "{name}");
    }
    for name in ["InTracks", "PastTracks", "Unpinned", "FarCell"] {
        assert_eq!(find(&once, name).layout_child, find(&twice, name).layout_child, "{name}");
    }
}
