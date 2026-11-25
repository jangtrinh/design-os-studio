//! A `.fig` open keeps Figma's records of the file as ranges of its inflated payload; nothing else changes.
#[path = "common/missing_font_fig.rs"]
mod missing_font_fig;

use designos_doc::KeptRecordKind;
use designos_fig::container::{self, Compression};
use designos_fig::kiwi::{KiwiValue as V, Schema};
use designos_fig::testkit::{figma_like_schema, guid, msg, node, vec2};
use designos_fig::{
    import_fig_with_kept_options, import_fig_with_report, import_figma_clipboard, FigClipboard, KeptOptions, MAX_KEPT_RECORD_BYTES, SPILL_THRESHOLD_BYTES,
};
use missing_font_fig::*;
use std::sync::Arc;

fn nc_field(schema: &Schema, name: &str) -> u32 {
    let nc = schema.find("NodeChange").unwrap();
    schema.defs[nc].field_named(name).unwrap_or_else(|| panic!("{name} is a NodeChange field")).value
}

/// The bytes the file holds for the value of one field: the field alone is written as a message, and its field number and
/// the message's terminator are taken off.
fn value_bytes(schema: &Schema, name: &str, value: V) -> Vec<u8> {
    let nc = schema.find("NodeChange").unwrap();
    let wire = schema.encode_message(nc, &msg(vec![(name, value)])).unwrap();
    let mut tag_len = 1;
    let mut n = nc_field(schema, name);
    while n >= 0x80 {
        n >>= 7;
        tag_len += 1;
    }
    wire[tag_len..wire.len() - 1].to_vec()
}

fn edit_info() -> V {
    msg(vec![("userId", V::String("user-1".into())), ("createdAt", V::Uint(1_700_000_000))])
}

fn framed(extra: Vec<(&str, V)>) -> Vec<V> {
    let mut nodes = document_and_page();
    nodes.push(node((2, 1), (0, 1), "a", "FRAME", "Keeper", [vec![("size", vec2(40.0, 30.0))], extra].concat()));
    nodes.push(node((2, 2), (0, 1), "b", "FRAME", "Plain", vec![("size", vec2(8.0, 8.0))]));
    nodes
}

/// A message payload over 2 MB (a padding blob), so the decoder is heavy and skips `editInfo`.
fn heavy(nodes: Vec<V>) -> Vec<u8> {
    fig_file(nodes, vec![icon_blob(), vec![0u8; 2_100_000]])
}

fn light(nodes: Vec<V>) -> Vec<u8> {
    fig_file(nodes, vec![icon_blob()])
}

#[test]
fn a_heavy_open_keeps_edit_info_and_font_version_as_the_files_own_bytes() {
    let schema = figma_like_schema();
    let extra = || vec![("editInfo", edit_info()), ("fontVersion", V::String("1.2.3".into()))];
    for (label, bytes) in [("heavy", heavy(framed(extra()))), ("light", light(framed(extra())))] {
        let (doc, report) = import_fig_with_report(&bytes).expect("imports");
        let rec = doc.fig_kept.record_by_guid((2, 1)).unwrap_or_else(|| panic!("{label}: the record is kept"));
        assert_eq!(rec.kind(), KeptRecordKind::Layer);
        assert_eq!(rec.field(nc_field(&schema, "editInfo")), Some(&value_bytes(&schema, "editInfo", edit_info())[..]), "{label}: editInfo");
        assert_eq!(
            rec.field(nc_field(&schema, "fontVersion")),
            Some(&value_bytes(&schema, "fontVersion", V::String("1.2.3".into()))[..]),
            "{label}: fontVersion"
        );
        // the tree writer's own fields are not kept for a layer
        for owned in ["guid", "parentIndex", "phase"] {
            assert_eq!(rec.field(nc_field(&schema, owned)), None, "{label}: {owned} is the tree writer's");
        }
        assert!(rec.field(nc_field(&schema, "size")).is_some(), "{label}: a modelled field is kept too");
        assert_eq!(report.kept_records, 4, "{label}: document, page and the two frames");
        assert_eq!((report.dropped_kept_records, report.kept_fields_not_conformed), (0, 0));
        let plain = doc.fig_kept.record_by_guid((2, 2)).expect("the other frame");
        assert_eq!(plain.field(nc_field(&schema, "editInfo")), None, "a record keeps only what the file holds for it");
    }
}

#[test]
fn a_page_and_the_document_are_kept_whole_and_the_records_attach_to_nodes() {
    let schema = figma_like_schema();
    let (doc, _) = import_fig_with_report(&light(framed(vec![]))).expect("imports");
    let page = doc.fig_kept.record_by_guid((0, 1)).expect("the page record");
    assert_eq!(page.kind(), KeptRecordKind::Other);
    assert!(page.field(nc_field(&schema, "parentIndex")).is_some() && page.field(nc_field(&schema, "guid")).is_some(), "whole");
    let keeper = doc.nodes.values().find(|n| n.name == "Keeper").expect("the frame");
    assert_eq!(doc.fig_kept.record_of_node(keeper.id).map(|r| r.guid()), Some((2, 1)));
    assert_eq!(doc.fig_kept_node_matches_snapshot(keeper.id), Some(true));
    assert!(doc.fig_kept.snapshot_due_after_settle(), "the app takes the snapshot once more when the open-time layout is done");
}

#[test]
fn the_blob_table_is_kept_as_slots_of_the_arena() {
    let (doc, _) = import_fig_with_report(&heavy(framed(vec![]))).expect("imports");
    assert_eq!(doc.fig_kept.blob_count(), 2);
    assert_eq!(doc.fig_kept.blob(0), Some(&icon_blob()[..]));
    assert_eq!(doc.fig_kept.blob(1).map(<[u8]>::len), Some(2_100_000));
    assert!(doc.fig_kept.blob(2).is_none());
}

#[test]
fn a_clone_of_the_imported_document_shares_the_store() {
    let (doc, _) = import_fig_with_report(&light(framed(vec![]))).expect("imports");
    assert!(Arc::ptr_eq(&doc.fig_kept, &doc.clone().fig_kept));
}

#[test]
fn a_record_past_the_per_record_cap_is_dropped_and_counted_and_the_rest_is_kept() {
    let huge = V::String("x".repeat(MAX_KEPT_RECORD_BYTES + 100));
    let (doc, report) = import_fig_with_report(&light(framed(vec![("fontVersion", huge)]))).expect("imports");
    assert_eq!(report.dropped_kept_records, 1);
    assert!(doc.fig_kept.record_by_guid((2, 1)).is_none());
    assert!(doc.fig_kept.record_by_guid((2, 2)).is_some() && doc.fig_kept.record_by_guid((0, 1)).is_some());
    assert_eq!(doc.fig_kept.counts().records_seen, 4);
}

#[test]
fn a_field_whose_definition_is_not_studios_is_left_out_and_counted() {
    let mut schema = figma_like_schema();
    let nc = schema.find("NodeChange").unwrap();
    // the file numbers `fontVersion` differently from the schema Studio writes with
    let f = schema.defs[nc].fields.iter_mut().find(|f| &*f.name == "fontVersion").unwrap();
    f.value = 3999;
    schema.defs[nc].reindex();
    let nodes = framed(vec![("fontVersion", V::String("9".into()))]);
    let body = schema.encode_message(schema.find("Message").unwrap(), &msg(vec![("nodeChanges", V::Array(nodes)), ("blobs", V::Array(vec![]))])).unwrap();
    let bytes = container::write_container(&schema.encode(), &body, Compression::Zstd, 161);
    let (doc, report) = import_fig_with_report(&bytes).expect("imports");
    assert_eq!(report.kept_fields_not_conformed, 1);
    let rec = doc.fig_kept.record_by_guid((2, 1)).expect("the record is kept without that field");
    assert_eq!(rec.field(3999), None);
    assert!(rec.field(nc_field(&figma_like_schema(), "size")).is_some());
}

#[test]
fn a_nested_node_change_is_not_a_record() {
    let mut nodes = framed(vec![]);
    let entry = msg(vec![("guidPath", msg(vec![("guids", V::Array(vec![guid(2, 2)]))])), ("visible", V::Bool(false))]);
    let symbol_data = msg(vec![("symbolID", guid(2, 1)), ("symbolOverrides", V::Array(vec![entry]))]);
    nodes.push(node((2, 3), (0, 1), "c", "INSTANCE", "Inst", vec![("size", vec2(8.0, 8.0)), ("symbolData", symbol_data)]));
    let (doc, report) = import_fig_with_report(&light(nodes)).expect("imports");
    assert_eq!(doc.fig_kept.counts().records_seen, 5, "the top-level records only, not the override entry inside one");
    assert_eq!(report.kept_records, 5);
}

#[test]
fn a_paste_keeps_nothing() {
    let clip = FigClipboard { meta: None, buffer: light(framed(vec![("fontVersion", V::String("1".into()))])) };
    let doc = import_figma_clipboard(&clip).expect("the paste imports");
    assert!(doc.fig_kept.is_empty());
    assert_eq!(doc.fig_kept.snapshot_count(), 0);
}

#[test]
fn an_open_without_options_keeps_nothing() {
    let (doc, report) = import_fig_with_kept_options(&light(framed(vec![])), None).expect("imports");
    assert!(doc.fig_kept.is_empty());
    assert_eq!(report.kept_records, 0);
}

#[test]
fn a_spilled_arena_reads_the_same_bytes_as_one_in_memory() {
    let dir = std::env::temp_dir().join(format!("designos-kept-open-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = heavy(framed(vec![("editInfo", edit_info()), ("fontVersion", V::String("1.2.3".into()))]));
    let (in_memory, _) =
        import_fig_with_kept_options(&bytes, Some(&KeptOptions { spill_threshold_bytes: SPILL_THRESHOLD_BYTES, spill_dir: dir.clone() })).unwrap();
    let (spilled, _) = import_fig_with_kept_options(&bytes, Some(&KeptOptions { spill_threshold_bytes: 1, spill_dir: dir.clone() })).unwrap();
    assert!(!in_memory.fig_kept.arena().is_file_backed());
    assert!(spilled.fig_kept.arena().is_file_backed(), "with the threshold forced low the store is file-backed");
    assert_eq!(spilled.fig_kept.retained_bytes().arena_memory, 0);
    assert_eq!(in_memory.fig_kept.arena().bytes(), spilled.fig_kept.arena().bytes());
    for guid in [(0, 0), (0, 1), (2, 1), (2, 2)] {
        let a: Vec<_> = in_memory.fig_kept.record_by_guid(guid).unwrap().fields().collect();
        let b: Vec<_> = spilled.fig_kept.record_by_guid(guid).unwrap().fields().collect();
        assert_eq!(a, b, "{guid:?}");
    }
    assert_eq!(spilled.fig_kept.blob(1).map(<[u8]>::len), Some(2_100_000));
    drop(spilled);
    let left = std::fs::read_dir(&dir).map(|d| d.count()).unwrap_or(0);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(left, 0, "no spill file outlives its store");
}

#[test]
fn a_spill_that_cannot_be_written_keeps_nothing_and_counts_the_records() {
    // the spill directory is a file, so it cannot be created
    let file = std::env::temp_dir().join(format!("designos-kept-notadir-{}", std::process::id()));
    std::fs::write(&file, b"x").unwrap();
    let bytes = light(framed(vec![]));
    let (doc, report) =
        import_fig_with_kept_options(&bytes, Some(&KeptOptions { spill_threshold_bytes: 1, spill_dir: file.join("inner") })).expect("the open still works");
    let _ = std::fs::remove_file(&file);
    assert!(doc.fig_kept.is_empty());
    assert_eq!((report.kept_records, report.dropped_kept_records), (0, 4));
}

#[test]
fn a_no_edit_save_is_the_same_with_and_without_the_store() {
    let bytes = heavy(framed(vec![("editInfo", edit_info()), ("fontVersion", V::String("1.2.3".into()))]));
    let (with, _) = import_fig_with_kept_options(&bytes, Some(&KeptOptions::default())).unwrap();
    let (without, _) = import_fig_with_kept_options(&bytes, None).unwrap();
    assert!(!with.fig_kept.is_empty());
    assert_eq!(designos_fig::export_fig(&with).unwrap(), designos_fig::export_fig(&without).unwrap());
}
