//! Files written by Studio 0.4.0 still import.
//!
//! Those exports number their schema their own way (sequential NodeChange ids, `Message.nodeChanges` = 1, grid counts
//! as plain numbers and track sizes as text, a circle cap written as `ROUND`). The importer reads a file through the
//! schema embedded in it, so none of that may matter.
//!
//! `tests/fixtures/legacy-0.4.0-rich.fig` and `legacy-0.4.0-grids.fig` were written by the `v0.4.0` tag (c02ca56) with
//! its `export_fig` on `common::rich_document()` and `common::grid_document()` as they are at this commit. They hold
//! synthetic nodes only.

mod common;
use common::{assert_grids_survived, assert_rich_survived};
use designos_fig::kiwi::Schema;
use designos_fig::{container, import_fig};

const RICH: &[u8] = include_bytes!("fixtures/legacy-0.4.0-rich.fig");
const GRIDS: &[u8] = include_bytes!("fixtures/legacy-0.4.0-grids.fig");

fn schema_of(bytes: &[u8]) -> Schema {
    let archive = container::open(bytes).unwrap();
    Schema::decode(&container::read_archive_chunks(&archive).unwrap().0).unwrap()
}

#[test]
fn the_fixtures_carry_the_old_numbering() {
    for bytes in [RICH, GRIDS] {
        let schema = schema_of(bytes);
        let root = &schema.defs[schema.root_message().unwrap()];
        assert_eq!(root.fields.iter().find(|f| &*f.name == "nodeChanges").map(|f| f.value), Some(1), "Figma's id is 4");
        let nc = &schema.defs[schema.find("NodeChange").unwrap()];
        assert!(nc.fields.iter().any(|f| &*f.name == "gridColumns" && f.value != 436), "not Figma's id for gridColumns");
        assert!(!schema.has_enum_variant("StrokeCap", "CIRCLE_FILLED"));
    }
}

#[test]
fn a_0_4_0_export_of_the_rich_document_imports_with_everything_it_held() {
    let rt = import_fig(RICH).expect("import a 0.4.0 export");
    assert_rich_survived(&rt);
}

#[test]
fn a_0_4_0_export_of_the_grids_imports_with_every_track_and_cell() {
    let rt = import_fig(GRIDS).expect("import a 0.4.0 export");
    assert_grids_survived(&rt);
}

#[test]
fn a_circle_and_a_diamond_cap_written_as_round_and_square_come_back_as_caps() {
    use designos_doc::{ConnectorCap, NodeKind};
    let rt = import_fig(RICH).unwrap();
    let n = rt.nodes.values().find(|n| n.name == "RoundCaps").expect("RoundCaps");
    let NodeKind::Connector(p) = &n.kind else { panic!("connector") };
    assert_eq!((p.start_cap, p.end_cap), (ConnectorCap::Circle, ConnectorCap::Diamond));
}
