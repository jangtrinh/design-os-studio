//! Raw Kiwi records of a real FigJam table (owner-authored board, 2 rows x 4 columns,
//! no merged cells). Reads only the decoded message, no importer mapping, so it holds
//! on any base. Seed for the table `.fig` import work: it pins what Figma actually writes.

use serde_json::Value;

const FIXTURE: &[u8] = include_bytes!("fixtures/figjam-table-2x4.jam");
/// Session id of the reserved template guids that start every `guidPath`.
const TEMPLATE_SESSION: u64 = 40_000_000;

fn message() -> Value {
    designos_fig::decode_fig_to_json(FIXTURE).expect("fixture decodes")["message"].clone()
}

fn guid(v: &Value) -> (u64, u64) {
    (v["sessionID"].as_u64().unwrap(), v["localID"].as_u64().unwrap())
}

fn path(o: &Value) -> Vec<(u64, u64)> {
    o["guidPath"]["guids"].as_array().unwrap().iter().map(guid).collect()
}

fn table(msg: &Value) -> Value {
    let all: Vec<&Value> = msg["nodeChanges"].as_array().unwrap().iter().filter(|n| n["type"] == "TABLE").collect();
    assert_eq!(all.len(), 1, "exactly one TABLE record");
    all[0].clone()
}

#[test]
fn table_is_one_record_and_cells_are_not_node_changes() {
    let msg = message();
    let types: Vec<&str> = msg["nodeChanges"].as_array().unwrap().iter().map(|n| n["type"].as_str().unwrap()).collect();
    assert_eq!(types, ["DOCUMENT", "CANVAS", "CANVAS", "TABLE"]);
    assert!(!types.contains(&"TABLE_CELL"), "no TABLE_CELL NodeChange exists in the file");
    let t = table(&msg);
    assert_eq!(t["size"]["x"], 768.0);
    assert_eq!(t["size"]["y"], 128.0);
    assert!(t.get("tableMergedCells").is_none(), "this board has no merge");
}

#[test]
fn row_and_column_tracks_carry_guid_position_and_size() {
    let t = table(&message());
    let ids = |key: &str| -> Vec<((u64, u64), Value)> { t[key]["entries"].as_array().unwrap().iter().map(|e| (guid(&e["id"]), e.clone())).collect() };
    let rows = ids("tableRowPositions");
    let cols = ids("tableColumnPositions");
    assert_eq!(rows.iter().map(|r| r.0).collect::<Vec<_>>(), [(1, 4), (1, 5)]);
    assert_eq!(cols.iter().map(|c| c.0).collect::<Vec<_>>(), [(1, 6), (1, 7), (1, 8), (1, 9)]);
    // position strings are the parentIndex fractional-index strings, consecutive here
    let pos = |v: &[((u64, u64), Value)]| v.iter().map(|e| e.1["position"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(pos(&rows), ["!", "\""]);
    assert_eq!(pos(&cols), ["!", "\"", "#", "$"]);
    // widths are explicit, heights are absent when the row has its default
    let widths = t["tableColumnWidths"]["entries"].as_array().unwrap();
    assert!(widths.iter().all(|e| e["size"] == 192.0));
    assert!(t["tableRowHeights"]["entries"].as_array().unwrap().is_empty());
    // track ids live in the file's node id space (session 1), disjoint from the template session
    assert!(rows.iter().chain(cols.iter()).all(|(g, _)| g.0 == 1));
}

#[test]
fn cell_text_lives_in_node_generation_data_keyed_by_template_row_column() {
    let t = table(&message());
    let ov = t["nodeGenerationData"]["overrides"].as_array().unwrap();
    let cells: Vec<(Vec<(u64, u64)>, &str)> = ov
        .iter()
        .filter(|o| o.get("textData").is_some() && o["guidPath"]["guids"].as_array().unwrap().len() == 3)
        .map(|o| (path(o), o["textData"]["characters"].as_str().unwrap()))
        .collect();
    let want = [("A", 4, 6), ("B", 4, 7), ("C", 4, 8), ("D", 4, 9), ("E", 5, 6)];
    assert_eq!(cells.len(), want.len(), "empty cells carry no override");
    for (c, (text, row, col)) in cells.iter().zip(want) {
        assert_eq!(c.0, [(TEMPLATE_SESSION, 1), (1, row), (1, col)]);
        assert_eq!(c.1, text);
    }
    // shared cell styling sits on single-guid template overrides (fill/stroke on 2, text style on 3)
    let tpl: Vec<u64> = ov.iter().filter(|o| path(o).len() == 1).map(|o| path(o)[0].1).collect();
    assert_eq!(tpl, [2, 3]);
}

#[test]
fn derived_immutable_frame_data_holds_one_frame_and_one_text_entry_per_cell() {
    let t = table(&message());
    let ov = t["derivedImmutableFrameData"]["overrides"].as_array().unwrap();
    assert_eq!(ov.len(), 16, "8 cell frames + 8 cell texts, empty cells included");
    let frames: Vec<_> = ov.iter().filter(|o| path(o)[0] == (TEMPLATE_SESSION, 0)).collect();
    let texts: Vec<_> = ov.iter().filter(|o| path(o)[0] == (TEMPLATE_SESSION, 1)).collect();
    assert_eq!((frames.len(), texts.len()), (8, 8));
    // every frame is 192 x 64 at (column offset, row offset), indexed by track order
    let order = |key: &str| -> Vec<(u64, u64)> { t[key]["entries"].as_array().unwrap().iter().map(|e| guid(&e["id"])).collect() };
    let (rows, cols) = (order("tableRowPositions"), order("tableColumnPositions"));
    for f in &frames {
        let p = path(f);
        let row = rows.iter().position(|r| *r == p[1]).expect("row id is a track");
        let col = cols.iter().position(|c| *c == p[2]).expect("column id is a track");
        assert_eq!((f["size"]["x"].as_f64(), f["size"]["y"].as_f64()), (Some(192.0), Some(64.0)));
        assert_eq!(f["transform"]["m02"].as_f64(), Some(col as f64 * 192.0));
        assert_eq!(f["transform"]["m12"].as_f64(), Some(row as f64 * 64.0));
    }
}
