//! The store of Figma's records, its snapshots and its arena.

use crate::branch_diff::{node_group_eq, Group};
use crate::{
    BlendMode, BlobSlot, Color, Document, FigKeptStore, KeptArena, KeptCounts, KeptData, KeptField, KeptRecord, KeptRecordKind, Node, NodeId, NodeKind, Paint,
    Rect,
};
use std::sync::Arc;

const ALL_GROUPS: [Group; 27] = [
    Group::Name,
    Group::Visible,
    Group::Locked,
    Group::Opacity,
    Group::Blend,
    Group::Transform,
    Group::Size,
    Group::Fills,
    Group::Stroke,
    Group::Effects,
    Group::CornerRadii,
    Group::Constraints,
    Group::LayoutChild,
    Group::Mask,
    Group::StyleRefs,
    Group::Variables,
    Group::Component,
    Group::Prototype,
    Group::Dev,
    Group::ExportSettings,
    Group::Kind,
    Group::TextContent,
    Group::TextStyle,
    Group::Frame,
    Group::Vector,
    Group::PluginData,
    Group::Other,
];

/// A store of `n` records with guids `(1, 1..=n)`, each holding one field of two bytes, and a document with a node per record.
fn document_with_store(n: u32) -> (Document, Vec<NodeId>) {
    let arena: Vec<u8> = (0..(n * 2) as u8).collect();
    let records: Vec<KeptRecord> = (0..n).map(|i| KeptRecord::new((1, i + 1), KeptRecordKind::Layer, i, 1)).collect();
    let fields: Vec<KeptField> = (0..n).map(|i| KeptField { no: 7, offset: i * 2, len: 2 }).collect();
    let counts = KeptCounts { records_seen: n as usize, records_kept: n as usize, fields_kept: n as usize, ..Default::default() };
    let data = KeptData::new(KeptArena::in_memory(arena), records, fields, Vec::new(), counts);
    let mut doc = Document::new("kept");
    let page = doc.add_page("P");
    let mut ids = Vec::new();
    for i in 0..n {
        let id = doc.alloc_id();
        let mut node = Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0));
        node.source_id = Some(format!("1:{}", i + 1));
        doc.insert(page, node).unwrap();
        ids.push(id);
    }
    doc.fig_kept = Arc::new(FigKeptStore::new(data));
    doc.take_fig_kept_snapshot();
    (doc, ids)
}

#[test]
fn a_record_is_attached_to_the_node_whose_source_id_names_its_guid() {
    let (doc, ids) = document_with_store(3);
    assert_eq!(doc.fig_kept.snapshot_count(), 3);
    for (i, id) in ids.iter().enumerate() {
        let rec = doc.fig_kept.record_of_node(*id).expect("record");
        assert_eq!(rec.guid(), (1, i as u32 + 1));
        assert_eq!(rec.field(7), Some(&[(i * 2) as u8, (i * 2 + 1) as u8][..]));
        assert_eq!(doc.fig_kept_node_matches_snapshot(*id), Some(true));
    }
}

#[test]
fn a_node_that_is_not_the_files_own_has_no_record() {
    let (mut doc, ids) = document_with_store(1);
    let page = doc.pages[0];
    let id = doc.alloc_id();
    let mut copy = Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0));
    copy.source_id = Some("I1:1;1:1".into());
    doc.insert(page, copy).unwrap();
    let dup = doc.alloc_id();
    doc.insert(page, Node::rectangle(dup, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
    doc.take_fig_kept_snapshot();
    assert_eq!(doc.fig_kept.snapshot_count(), 1);
    assert!(doc.fig_kept.record_of_node(id).is_none() && doc.fig_kept.record_of_node(dup).is_none());
    assert!(doc.fig_kept.record_of_node(ids[0]).is_some());
}

#[test]
fn a_clone_of_the_document_shares_the_store_as_one_arc() {
    let (doc, _) = document_with_store(2);
    let copy = doc.clone();
    assert!(Arc::ptr_eq(&doc.fig_kept, &copy.fig_kept));
    let lighter = doc.clone_without_images();
    assert!(Arc::ptr_eq(&doc.fig_kept, &lighter.fig_kept));
}

#[test]
fn an_edit_changes_the_snapshot_match_and_undoing_it_brings_the_match_back() {
    let (mut doc, ids) = document_with_store(2);
    let original = doc.get(ids[0]).unwrap().name.clone();
    doc.get_mut(ids[0]).unwrap().name = "renamed".into();
    assert_eq!(doc.fig_kept_node_matches_snapshot(ids[0]), Some(false));
    assert_eq!(doc.fig_kept_node_matches_snapshot(ids[1]), Some(true), "the other node is untouched");
    doc.get_mut(ids[0]).unwrap().name = original;
    assert_eq!(doc.fig_kept_node_matches_snapshot(ids[0]), Some(true));
}

#[test]
fn a_layout_float_within_the_tolerance_is_not_an_edit_and_one_beyond_it_is() {
    let (mut doc, ids) = document_with_store(1);
    doc.get_mut(ids[0]).unwrap().size.x += 1e-4;
    doc.get_mut(ids[0]).unwrap().transform.tx += 1e-4;
    doc.get_mut(ids[0]).unwrap().opacity -= 1e-4;
    assert_eq!(doc.fig_kept_node_matches_snapshot(ids[0]), Some(true));
    doc.get_mut(ids[0]).unwrap().size.x += 0.5;
    assert_eq!(doc.fig_kept_node_matches_snapshot(ids[0]), Some(false));
}

#[test]
fn what_a_loader_writes_of_its_own_is_not_an_edit() {
    let (mut doc, ids) = document_with_store(1);
    doc.get_mut(ids[0]).unwrap().component.fig_own_entry_flags.insert(crate::OverrideField::Layout);
    assert_eq!(doc.fig_kept_node_matches_snapshot(ids[0]), Some(true), "the runtime flags of a deferred entry are bookkeeping");
}

/// A snapshot that still matches after a change means no group `node_group_eq` compares differs, whatever the hash is made of.
/// The converse is not claimed: the snapshot leaves out bookkeeping that `node_group_eq` compares.
#[test]
fn a_match_after_a_change_means_every_group_is_equal() {
    type Change = (&'static str, fn(&mut Node));
    let changes: Vec<Change> = vec![
        ("name", |n| n.name.push('x')),
        ("visible", |n| n.visible = !n.visible),
        ("locked", |n| n.locked = !n.locked),
        ("opacity", |n| n.opacity = 0.25),
        ("blend", |n| n.blend = BlendMode::Multiply),
        ("transform", |n| n.transform.tx += 5.0),
        ("size", |n| n.size.y += 5.0),
        ("fills", |n| n.fills.push(Paint::solid(Color::WHITE))),
        ("stroke", |n| n.stroke.weight += 2.0),
        ("corner radii", |n| n.corner_radii[1] = 3.0),
        ("corner smoothing", |n| n.corner_smoothing = 0.6),
        ("mask", |n| n.is_mask = true),
        ("lock aspect", |n| n.lock_aspect = !n.lock_aspect),
        ("hyperlink", |n| n.hyperlink = Some("https://example.com".into())),
        ("source id", |n| n.source_id = Some("1:1 ".into())),
        ("kind", |n| n.kind = NodeKind::Ellipse(Default::default())),
        ("kind to group", |n| n.kind = NodeKind::Group),
        ("component link", |n| n.component.description = "d".into()),
        ("component overrides", |n| {
            n.component.overrides.insert(crate::OverrideField::Layout);
        }),
        ("plugin data", |n| {
            n.plugin_data.insert("p".into(), "v".to_string());
        }),
    ];
    for (what, change) in changes {
        let (mut doc, ids) = document_with_store(1);
        let before = doc.get(ids[0]).unwrap().clone();
        change(doc.get_mut(ids[0]).unwrap());
        let after = doc.get(ids[0]).unwrap();
        let matches = doc.fig_kept_node_matches_snapshot(ids[0]).unwrap();
        let equal_everywhere = ALL_GROUPS.iter().all(|g| node_group_eq(&before, after, *g));
        assert!(!matches || equal_everywhere, "{what}: the snapshot still matches though a group differs");
        assert!(!matches, "{what}: this change is an edit");
    }
}

#[test]
fn a_loader_write_moves_the_snapshot_forward_only_from_the_value_the_snapshot_took() {
    let (mut doc, ids) = document_with_store(2);
    // the user edited the second node before the loader wrote into both
    doc.get_mut(ids[1]).unwrap().name = "edited".into();
    let (_, written) = doc.record_fig_kept_writes(|d| {
        d.get_mut(ids[0]).unwrap().size.x += 40.0;
        d.get_mut(ids[1]).unwrap().size.x += 40.0;
    });
    assert_eq!(doc.fig_kept_node_matches_snapshot(ids[0]), Some(false), "the loader's write differs from the snapshot");
    // `written` holds each node as it was before the loader's first write, which the record function only keeps while recording
    doc.advance_fig_kept_snapshots(&written);
    assert_eq!(doc.fig_kept_node_matches_snapshot(ids[0]), Some(true), "an untouched node moves forward over the loader's write");
    assert_eq!(doc.fig_kept_node_matches_snapshot(ids[1]), Some(false), "an edited node stays different from its snapshot");
}

#[test]
fn the_open_time_layout_settle_ends_once_and_the_store_is_shared_with_the_earlier_snapshot() {
    let (mut doc, _) = document_with_store(2);
    let before = doc.fig_kept.clone();
    doc.take_fig_kept_snapshot_before_settle();
    assert!(doc.fig_kept.snapshot_due_after_settle());
    assert!(doc.fig_kept.shares_data_with(&before), "a snapshot shares the arena and the records");
    doc.finish_fig_kept_settle();
    assert!(!doc.fig_kept.snapshot_due_after_settle());
}

#[test]
fn renumbering_the_ids_keeps_every_kept_entry_on_the_same_node() {
    let mut doc = Document::new("high ids");
    let page = doc.add_page("P");
    let arena: Vec<u8> = (0..20).collect();
    let records: Vec<KeptRecord> = (0..4).map(|i| KeptRecord::new((9, i + 1), KeptRecordKind::Layer, i, 1)).collect();
    let fields: Vec<KeptField> = (0..4).map(|i| KeptField { no: 3, offset: i * 2, len: 2 }).collect();
    let data = KeptData::new(KeptArena::in_memory(arena), records, fields, Vec::new(), KeptCounts::default());
    for i in 0..4u32 {
        let id = NodeId(if i < 2 { u64::MAX - u64::from(i) } else { 10 + u64::from(i) });
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 1.0, 1.0));
        n.source_id = Some(format!("9:{}", i + 1));
        doc.insert(page, n).unwrap();
    }
    doc.fig_kept = Arc::new(FigKeptStore::new(data));
    doc.take_fig_kept_snapshot();
    assert_eq!(doc.fig_kept.snapshot_count(), 4);
    assert!(doc.renumber_ids_for_headroom() > 0);
    assert_eq!(doc.fig_kept.snapshot_count(), 4);
    for node in doc.nodes.values().filter(|n| n.source_id.is_some()) {
        let rec = doc.fig_kept.record_of_node(node.id).expect("the entry followed its node");
        assert_eq!(format!("9:{}", rec.guid().1), node.source_id.clone().unwrap());
        assert_eq!(doc.fig_kept_node_matches_snapshot(node.id), Some(true));
    }
}

#[test]
fn a_record_or_blob_that_names_bytes_outside_the_arena_is_dropped_and_counted() {
    let arena = vec![0u8; 10];
    let records = vec![
        KeptRecord::new((1, 1), KeptRecordKind::Layer, 0, 1),
        KeptRecord::new((1, 2), KeptRecordKind::Layer, 1, 1),
        KeptRecord::new((1, 3), KeptRecordKind::Layer, 5, 2),
    ];
    let fields = vec![KeptField { no: 1, offset: 0, len: 10 }, KeptField { no: 1, offset: 4, len: 7 }];
    let counts = KeptCounts { records_kept: 3, ..Default::default() };
    let data = KeptData::new(KeptArena::in_memory(arena), records, fields, vec![BlobSlot { offset: 2, len: 8 }, BlobSlot { offset: 9, len: 5 }], counts);
    let store = FigKeptStore::new(data);
    assert!(store.record_by_guid((1, 1)).is_some());
    assert!(store.record_by_guid((1, 2)).is_none(), "its range ends past the arena");
    assert!(store.record_by_guid((1, 3)).is_none(), "its run is past the field table");
    assert_eq!(store.counts().dropped_kept_records, 2);
    assert_eq!(store.counts().records_kept, 1);
    assert_eq!(store.blob(0).map(<[u8]>::len), Some(8));
    assert_eq!(store.blob(1).map(<[u8]>::len), Some(0), "an out-of-range blob slot reads as empty");
}

#[test]
fn a_spilled_arena_is_a_mapped_file_with_the_same_bytes_and_no_file_left_behind() {
    let dir = std::env::temp_dir().join(format!("designos-kept-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes: Vec<u8> = (0..100_000u32).map(|i| (i % 251) as u8).collect();
    let arena = KeptArena::on_disk(&bytes, &dir).expect("spill");
    assert!(arena.is_file_backed());
    assert_eq!(arena.memory_bytes(), 0, "a mapped arena holds no memory of its own");
    assert_eq!(arena.bytes(), &bytes[..]);
    drop(arena);
    let left = std::fs::read_dir(&dir).map(|d| d.count()).unwrap_or(0);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(left, 0, "the spill file is removed with the arena");
}

/// An auto-layout column whose two children sit where the file stored them (both at the top), so the solver moves the second.
fn column_with_store() -> (Document, NodeId, [NodeId; 2]) {
    let arena: Vec<u8> = vec![0; 8];
    let records: Vec<KeptRecord> = (1..=3).map(|i| KeptRecord::new((1, i), KeptRecordKind::Layer, i - 1, 1)).collect();
    let fields: Vec<KeptField> = (0..3).map(|i| KeptField { no: 1, offset: i, len: 1 }).collect();
    let data = KeptData::new(KeptArena::in_memory(arena), records, fields, Vec::new(), KeptCounts::default());
    let mut doc = Document::new("column");
    let page = doc.add_page("P");
    let parent = doc.alloc_id();
    let mut column = Node::frame(parent, Rect::new(0.0, 0.0, 100.0, 100.0));
    if let NodeKind::Frame(fp) = &mut column.kind {
        fp.auto_layout = Some(crate::AutoLayout {
            direction: crate::LayoutDirection::Vertical,
            primary_sizing: crate::Sizing::Fixed,
            counter_sizing: crate::Sizing::Fixed,
            ..Default::default()
        });
    }
    column.source_id = Some("1:1".into());
    doc.insert(page, column).unwrap();
    let mut kids = [NodeId(0); 2];
    for (i, kid) in kids.iter_mut().enumerate() {
        let id = doc.alloc_id();
        let mut r = Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0));
        r.source_id = Some(format!("1:{}", i + 2));
        doc.insert(parent, r).unwrap();
        *kid = id;
    }
    doc.fig_kept = Arc::new(FigKeptStore::new(data));
    doc.take_fig_kept_snapshot_before_settle();
    (doc, parent, kids)
}

fn settle_on_open(doc: &mut Document, root: NodeId) {
    doc.pending_relayout.push(root);
    while doc.flush_pending_relayout_step_on_open() > 0 {}
}

#[test]
fn the_open_time_layout_is_not_an_edit_and_a_second_settle_changes_nothing() {
    let (mut doc, parent, kids) = column_with_store();
    let y0 = doc.get(kids[1]).unwrap().transform.ty;
    settle_on_open(&mut doc, parent);
    assert_ne!(doc.get(kids[1]).unwrap().transform.ty, y0, "the solver moved the second child");
    for id in [parent, kids[0], kids[1]] {
        assert_eq!(doc.fig_kept_node_matches_snapshot(id), Some(true), "the solver's own write is not an edit");
    }
    doc.finish_fig_kept_settle();
    settle_on_open(&mut doc, parent);
    for id in [parent, kids[0], kids[1]] {
        assert_eq!(doc.fig_kept_node_matches_snapshot(id), Some(true), "a second settle is the same");
    }
    doc.get_mut(kids[0]).unwrap().name = "edited".into();
    assert_eq!(doc.fig_kept_node_matches_snapshot(kids[0]), Some(false), "an edit of one property changes it");
    assert_eq!(doc.fig_kept_node_matches_snapshot(kids[1]), Some(true));
}

#[test]
fn a_layout_after_an_edit_is_not_blessed_and_a_node_edited_before_the_settle_stays_edited() {
    let (mut doc, parent, kids) = column_with_store();
    doc.get_mut(kids[1]).unwrap().name = "edited before the layout".into();
    settle_on_open(&mut doc, parent);
    assert_eq!(doc.fig_kept_node_matches_snapshot(kids[1]), Some(false), "the edited node did not move forward");
    assert_eq!(doc.fig_kept_node_matches_snapshot(kids[0]), Some(true));
    // after the settle, the layout that an edit causes moves nothing
    doc.finish_fig_kept_settle();
    doc.get_mut(kids[0]).unwrap().size.y = 40.0;
    doc.pending_relayout.push(parent);
    while doc.flush_pending_relayout_step() > 0 {}
    assert_eq!(doc.fig_kept_node_matches_snapshot(kids[0]), Some(false));
}
