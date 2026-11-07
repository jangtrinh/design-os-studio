//! The grid row-gap rule and the Auto cell-alignment rule each have one copy, in the layout
//! solver (`grid_row_gap` and `resolve_cell_align`). These checks read this crate's `src/*.rs`
//! sources, so a private copy fails even when it still produces the same output.

use std::fs;
use std::path::Path;

fn lines_containing(needle: &str) -> Vec<String> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let entries = fs::read_dir(&dir).unwrap_or_else(|err| panic!("read {}: {err}", dir.display()));
    let mut hits = Vec::new();
    let mut scanned = 0;
    for entry in entries {
        let path = entry.unwrap_or_else(|err| panic!("dir entry: {err}")).path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        scanned += 1;
        let text = fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
        for (idx, line) in text.lines().enumerate() {
            if line.contains(needle) {
                hits.push(format!("{}:{}: {}", path.display(), idx + 1, line.trim()));
            }
        }
    }
    assert!(scanned > 10, "expected to scan the export sources, scanned {scanned}");
    hits
}

#[test]
fn row_gap_rule_is_not_copied_into_export_sources() {
    let hits = lines_containing("counter_spacing > 0.0 { al");
    assert!(hits.is_empty(), "copied row-gap rule:\n{}", hits.join("\n"));
}

#[test]
fn auto_cell_alignment_mapping_is_not_copied_into_export_sources() {
    let hits = lines_containing("GridChildAlign::Auto => match");
    assert!(hits.is_empty(), "copied Auto alignment mapping:\n{}", hits.join("\n"));
}
