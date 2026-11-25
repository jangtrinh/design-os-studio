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

/// A document with one node, `base`, that has a record.
fn document_with_node(base: impl FnOnce(NodeId) -> Node) -> (Document, NodeId) {
    let arena: Vec<u8> = vec![0, 0];
    let records = vec![KeptRecord::new((1, 1), KeptRecordKind::Layer, 0, 1)];
    let fields = vec![KeptField { no: 7, offset: 0, len: 2 }];
    let data = KeptData::new(KeptArena::in_memory(arena), records, fields, Vec::new(), KeptCounts::default());
    let mut doc = Document::new("kept");
    let page = doc.add_page("P");
    let id = doc.alloc_id();
    let mut node = base(id);
    node.source_id = Some("1:1".into());
    doc.insert(page, node).unwrap();
    doc.fig_kept = Arc::new(FigKeptStore::new(data));
    doc.take_fig_kept_snapshot();
    (doc, id)
}

fn frame_base(id: NodeId) -> Node {
    Node::frame(id, Rect::new(0.0, 0.0, 10.0, 10.0))
}

fn text_base(id: NodeId) -> Node {
    Node::text(id, Rect::new(0.0, 0.0, 10.0, 10.0), "hi", Default::default())
}

fn vector_base(id: NodeId) -> Node {
    let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0));
    n.kind = NodeKind::Vector { paths: Vec::new(), network: None };
    n
}

fn instance_base(id: NodeId) -> Node {
    let mut n = Node::frame(id, Rect::new(0.0, 0.0, 10.0, 10.0));
    n.kind = NodeKind::Instance { frame: Default::default(), component: Some(NodeId(98)), component_key: None };
    n
}

fn connector_base(id: NodeId) -> Node {
    let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0));
    n.kind = NodeKind::Connector(Default::default());
    n
}

fn with_text(n: &mut Node, f: impl FnOnce(&mut crate::TextProps)) {
    let NodeKind::Text(t) = &mut n.kind else { panic!("a text node") };
    f(t);
}

/// A snapshot that still matches after a change means no group `node_group_eq` compares differs, whatever the hash is made of.
/// The converse is not claimed: the snapshot leaves out bookkeeping that `node_group_eq` compares. Every change here is one
/// edit of one property of the kinds of node whose fields a save writes back.
#[test]
fn a_match_after_a_change_means_every_group_is_equal() {
    type Change = (&'static str, fn(NodeId) -> Node, fn(&mut Node));
    let rect: fn(NodeId) -> Node = |id| Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0));
    let changes: Vec<Change> = vec![
        ("name", rect, |n| n.name.push('x')),
        ("visible", rect, |n| n.visible = !n.visible),
        ("locked", rect, |n| n.locked = !n.locked),
        ("opacity", rect, |n| n.opacity = 0.25),
        ("blend", rect, |n| n.blend = BlendMode::Multiply),
        ("transform", rect, |n| n.transform.tx += 5.0),
        ("size", rect, |n| n.size.y += 5.0),
        ("fills", rect, |n| n.fills.push(Paint::solid(Color::WHITE))),
        ("stroke", rect, |n| n.stroke.weight += 2.0),
        ("corner radii", rect, |n| n.corner_radii[1] = 3.0),
        ("corner smoothing", rect, |n| n.corner_smoothing = 0.6),
        ("mask", rect, |n| n.is_mask = true),
        ("lock aspect", rect, |n| n.lock_aspect = !n.lock_aspect),
        ("hyperlink", rect, |n| n.hyperlink = Some("https://example.com".into())),
        ("source id", rect, |n| n.source_id = Some("1:1 ".into())),
        ("kind", rect, |n| n.kind = NodeKind::Ellipse(Default::default())),
        ("kind to group", rect, |n| n.kind = NodeKind::Group),
        ("component link", rect, |n| n.component.description = "d".into()),
        ("component overrides", rect, |n| {
            n.component.overrides.insert(crate::OverrideField::Layout);
        }),
        ("plugin data", rect, |n| {
            n.plugin_data.insert("p".into(), "v".to_string());
        }),
        ("effects", frame_base, |n| n.effects.push(crate::Effect::LayerBlur { radius: 4.0, visible: true })),
        ("constraints", frame_base, |n| n.constraints.horizontal = crate::Constraint::Max),
        ("layout child", frame_base, |n| n.layout_child.grow = 1.0),
        ("style refs", frame_base, |n| {
            n.style_refs.insert("fill".into(), "1:2".into());
        }),
        ("variable binding", frame_base, |n| {
            n.variables.bindings.insert("fills".into(), "VariableID:1:2".into());
        }),
        ("variable mode", frame_base, |n| {
            n.variables.modes.insert("VariableCollectionId:1:3".into(), "1:4".into());
        }),
        ("auto layout", frame_base, |n| n.kind.frame_props_mut().unwrap().auto_layout = Some(crate::AutoLayout::default())),
        ("clips content", frame_base, |n| {
            let fp = n.kind.frame_props_mut().unwrap();
            fp.clips_content = !fp.clips_content;
        }),
        ("prototype reactions", frame_base, |n| n.prototype.fixed_children.push(NodeId(77))),
        ("dev status", frame_base, |n| n.dev.status_updated = Some("2026-01-01T00:00:00Z".into())),
        ("export settings", frame_base, |n| n.export_settings.push(Default::default())),
        ("slide", frame_base, |n| n.slide = Some(crate::SlideProps { notes: "n".into() })),
        ("detached symbol", frame_base, |n| n.detached_symbol_id = Some("1:9".into())),
        ("metadata", frame_base, |n| {
            n.fig_metadata.insert("editInfo".into(), "{}".into());
        }),
        ("extra", frame_base, |n| {
            n.extra.insert("k".into(), serde_json::json!(1));
        }),
        ("motion", frame_base, |n| n.motion = Some(Default::default())),
        ("text content", text_base, |n| with_text(n, |t| t.content.push('x'))),
        ("text size", text_base, |n| with_text(n, |t| t.style.font_size += 1.0)),
        ("text run", text_base, |n| with_text(n, |t| t.runs.push(crate::TextRun { start: 0, end: 1, style: t.style.clone() }))),
        ("text auto resize", text_base, |n| with_text(n, |t| t.max_lines = Some(2))),
        ("mask type", rect, |n| n.mask_type = crate::MaskType::Vector),
        ("stroke dashes", rect, |n| n.stroke.dashes.push(4.0)),
        ("stroke side weights", rect, |n| n.stroke.side_weights = Some([1.0, 2.0, 3.0, 4.0])),
        ("text paragraph", text_base, |n| with_text(n, |t| t.paragraph.indent = 12.0)),
        ("text direction", text_base, |n| with_text(n, |t| t.direction = crate::TextDirection::Rtl)),
        ("instance swap", instance_base, |n| {
            if let NodeKind::Instance { component, .. } = &mut n.kind {
                *component = Some(NodeId(99));
            }
        }),
        ("instance key", instance_base, |n| {
            if let NodeKind::Instance { component_key, .. } = &mut n.kind {
                *component_key = Some("key".into());
            }
        }),
        ("connector cap", connector_base, |n| {
            if let NodeKind::Connector(c) = &mut n.kind {
                c.end_cap = crate::ConnectorCap::Diamond;
            }
        }),
        ("vector paths", vector_base, |n| {
            if let NodeKind::Vector { paths, .. } = &mut n.kind {
                paths.push(Default::default());
            }
        }),
        ("fill geometry", vector_base, |n| n.fill_geometry = Some(Vec::new())),
        ("stroke geometry", vector_base, |n| n.stroke_geometry = Some(Vec::new())),
    ];
    for (what, base, change) in changes {
        let (mut doc, id) = document_with_node(base);
        let before = doc.get(id).unwrap().clone();
        assert_eq!(doc.fig_kept_node_matches_snapshot(id), Some(true), "{what}: unedited to begin with");
        change(doc.get_mut(id).unwrap());
        let after = doc.get(id).unwrap();
        let matches = doc.fig_kept_node_matches_snapshot(id).unwrap();
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
fn renumbering_keeps_an_unedited_node_that_names_another_equal_and_an_edited_one_different() {
    let mut doc = Document::new("high ids");
    let page = doc.add_page("P");
    let arena: Vec<u8> = (0..20).collect();
    let records: Vec<KeptRecord> = (0..3).map(|i| KeptRecord::new((9, i + 1), KeptRecordKind::Layer, i, 1)).collect();
    let fields: Vec<KeptField> = (0..3).map(|i| KeptField { no: 3, offset: i * 2, len: 2 }).collect();
    let data = KeptData::new(KeptArena::in_memory(arena), records, fields, Vec::new(), KeptCounts::default());
    let main = NodeId(u64::MAX);
    let mut m = Node::frame(main, Rect::new(0.0, 0.0, 1.0, 1.0));
    m.source_id = Some("9:1".into());
    doc.insert(page, m).unwrap();
    let instance = |id: u64, src: &str| {
        let mut n = Node::frame(NodeId(id), Rect::new(0.0, 0.0, 1.0, 1.0));
        n.kind = NodeKind::Instance { frame: Default::default(), component: Some(main), component_key: None };
        n.source_id = Some(src.into());
        n
    };
    doc.insert(page, instance(u64::MAX - 1, "9:2")).unwrap();
    doc.insert(page, instance(u64::MAX - 2, "9:3")).unwrap();
    doc.fig_kept = Arc::new(FigKeptStore::new(data));
    doc.take_fig_kept_snapshot();
    doc.get_mut(NodeId(u64::MAX - 2)).unwrap().name = "edited".into();
    assert!(doc.renumber_ids_for_headroom() > 0);
    let by_source = |doc: &Document, src: &str| doc.nodes.values().find(|n| n.source_id.as_deref() == Some(src)).map(|n| n.id).unwrap();
    let (unedited, edited) = (by_source(&doc, "9:2"), by_source(&doc, "9:3"));
    let NodeKind::Instance { component, .. } = &doc.get(unedited).unwrap().kind else { panic!("an instance") };
    assert_eq!(*component, Some(by_source(&doc, "9:1")), "the instance names the renumbered main");
    assert_eq!(doc.fig_kept_node_matches_snapshot(unedited), Some(true), "the renumbering is not an edit of the instance");
    assert_eq!(doc.fig_kept_node_matches_snapshot(edited), Some(false), "an edited instance stays edited");
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
    assert!(store.blob(1).is_none(), "an out-of-range blob slot is absent, not empty");
    assert_eq!(store.counts().dropped_blob_slots, 1);
    assert_eq!(store.blob_count(), 2, "the numbering of the table is kept");
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

/// `n` auto-layout columns, each with two children that sit where the file stored them (both at the top), so the solver moves
/// the second one. Every column and child has a record. `queued_by_loader` columns are on the queue when the snapshot is
/// taken, as the layout an import leaves is; the others are queued by the test as an edit would.
fn columns_with_store(n: u32, queued_by_loader: &[usize]) -> (Document, Vec<NodeId>, Vec<[NodeId; 2]>) {
    columns_with_store_from(n, queued_by_loader, 0)
}

/// [`columns_with_store`] with the id counter put at `first_id`, so a large value gives every node an id the renumber moves.
fn columns_with_store_from(n: u32, queued_by_loader: &[usize], first_id: u64) -> (Document, Vec<NodeId>, Vec<[NodeId; 2]>) {
    let total = n * 3;
    let arena: Vec<u8> = vec![0; total as usize];
    let records: Vec<KeptRecord> = (1..=total).map(|i| KeptRecord::new((1, i), KeptRecordKind::Layer, i - 1, 1)).collect();
    let fields: Vec<KeptField> = (0..total).map(|i| KeptField { no: 1, offset: i, len: 1 }).collect();
    let data = KeptData::new(KeptArena::in_memory(arena), records, fields, Vec::new(), KeptCounts::default());
    let mut doc = Document::new("columns");
    doc.next_id = first_id;
    let page = doc.add_page("P");
    let mut columns = Vec::new();
    let mut kids = Vec::new();
    for c in 0..n {
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
        column.source_id = Some(format!("1:{}", c * 3 + 1));
        doc.insert(page, column).unwrap();
        let mut pair = [NodeId(0); 2];
        for (i, kid) in pair.iter_mut().enumerate() {
            let id = doc.alloc_id();
            let mut r = Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0));
            r.source_id = Some(format!("1:{}", c * 3 + i as u32 + 2));
            doc.insert(parent, r).unwrap();
            *kid = id;
        }
        columns.push(parent);
        kids.push(pair);
    }
    doc.fig_kept = Arc::new(FigKeptStore::new(data));
    for &c in queued_by_loader {
        doc.pending_relayout.push(columns[c]);
    }
    doc.take_fig_kept_snapshot();
    (doc, columns, kids)
}

fn drain(doc: &mut Document) {
    while doc.flush_pending_relayout_step() > 0 {}
}

fn column_with_store() -> (Document, NodeId, [NodeId; 2]) {
    let (doc, columns, kids) = columns_with_store(1, &[0]);
    (doc, columns[0], kids[0])
}

#[test]
fn the_layout_an_import_queued_is_not_an_edit_and_a_second_pass_changes_nothing() {
    let (mut doc, parent, kids) = column_with_store();
    let y0 = doc.get(kids[1]).unwrap().transform.ty;
    drain(&mut doc);
    assert_ne!(doc.get(kids[1]).unwrap().transform.ty, y0, "the solver moved the second child");
    for id in [parent, kids[0], kids[1]] {
        assert_eq!(doc.fig_kept_node_matches_snapshot(id), Some(true), "the solver's own write is not an edit");
    }
    doc.pending_relayout.push(parent);
    drain(&mut doc);
    for id in [parent, kids[0], kids[1]] {
        assert_eq!(doc.fig_kept_node_matches_snapshot(id), Some(true), "a second pass is the same");
    }
    doc.get_mut(kids[0]).unwrap().name = "edited".into();
    assert_eq!(doc.fig_kept_node_matches_snapshot(kids[0]), Some(false), "an edit of one property changes it");
    assert_eq!(doc.fig_kept_node_matches_snapshot(kids[1]), Some(true));
}

#[test]
fn a_node_edited_before_the_loader_layout_stays_edited() {
    let (mut doc, parent, kids) = column_with_store();
    doc.get_mut(kids[1]).unwrap().name = "edited before the layout".into();
    drain(&mut doc);
    assert_eq!(doc.fig_kept_node_matches_snapshot(kids[1]), Some(false), "the edited node did not move forward");
    assert_eq!(doc.fig_kept_node_matches_snapshot(kids[0]), Some(true));
    assert_eq!(doc.fig_kept_node_matches_snapshot(parent), Some(true));
}

#[test]
fn the_layout_an_edit_queued_is_not_blessed_even_while_the_loader_layout_drains_beside_it() {
    // column 0 is the loader's, column 1 is queued by an edit during the same drain
    let (mut doc, columns, kids) = columns_with_store(2, &[0]);
    doc.get_mut(kids[1][0]).unwrap().size.y = 40.0;
    doc.pending_relayout.push(columns[1]);
    drain(&mut doc);
    assert_eq!(doc.fig_kept_node_matches_snapshot(kids[0][1]), Some(true), "the loader's layout moved its snapshots");
    assert_eq!(doc.fig_kept_node_matches_snapshot(kids[1][0]), Some(false));
    assert_eq!(doc.fig_kept_node_matches_snapshot(kids[1][1]), Some(false), "a node the edit's layout moved reads as edited");
}

#[test]
fn an_edit_inside_a_queued_loader_root_makes_its_layout_the_edits_and_blesses_nothing() {
    let (mut doc, parent, kids) = column_with_store();
    assert!(doc.loader_relayout_roots.contains(&parent));
    // an edit changes a child before the loader's layout drains, and queues nothing itself
    doc.get_mut(kids[0]).unwrap().size.y = 40.0;
    assert!(doc.loader_relayout_roots.is_empty(), "the root is no longer the loader's");
    drain(&mut doc);
    assert_eq!(doc.fig_kept_node_matches_snapshot(kids[0]), Some(false));
    assert_eq!(doc.fig_kept_node_matches_snapshot(kids[1]), Some(false), "the sibling the edit pushed down is not blessed");
}

#[test]
fn an_edit_in_another_column_leaves_a_queued_loader_root_alone() {
    let (mut doc, columns, kids) = columns_with_store(2, &[0]);
    doc.get_mut(kids[1][0]).unwrap().name = "elsewhere".into();
    assert!(doc.loader_relayout_roots.contains(&columns[0]));
    drain(&mut doc);
    assert_eq!(doc.fig_kept_node_matches_snapshot(kids[0][1]), Some(true));
}

#[test]
fn renumbering_the_ids_follows_the_queued_roots_and_their_tags() {
    // every id sits above the allocation limit, so the renumber really rewrites them
    let (mut doc, columns, _) = columns_with_store_from(1, &[0], u64::MAX - 100);
    let old_parent = columns[0];
    assert!(old_parent.0 > u64::MAX - 100);
    assert_eq!(doc.pending_relayout(), &[old_parent]);
    assert!(doc.loader_relayout_roots.contains(&old_parent));
    assert!(doc.renumber_ids_for_headroom() > 0);
    assert!(doc.get(old_parent).is_none(), "the node has a new id");
    let parent = doc.pages.iter().find_map(|p| doc.get(*p)?.children.first().copied()).expect("the column");
    assert_ne!(parent, old_parent);
    assert_eq!(doc.pending_relayout(), &[parent], "the queue follows the new id");
    assert_eq!(doc.loader_relayout_roots, std::collections::BTreeSet::from([parent]), "so does the tag, and the renumber does not drop it");
    let kids = doc.get(parent).unwrap().children.clone();
    drain(&mut doc);
    for id in [parent, kids[0], kids[1]] {
        assert_eq!(doc.fig_kept_node_matches_snapshot(id), Some(true), "the loader's layout still moves the snapshots after the renumber");
    }
}

/// A column whose second child already sits where the solver puts it, with the solve queued by the loader: a structural edit
/// that moves it is then the only thing that can change its place.
fn settled_column_queued_again() -> (Document, NodeId, [NodeId; 2]) {
    let (mut doc, parent, kids) = column_with_store();
    drain(&mut doc);
    doc.pending_relayout.push(parent);
    doc.take_fig_kept_snapshot();
    assert!(doc.loader_relayout_roots.contains(&parent));
    for id in [parent, kids[0], kids[1]] {
        assert_eq!(doc.fig_kept_node_matches_snapshot(id), Some(true));
    }
    (doc, parent, kids)
}

#[test]
fn removing_a_child_makes_the_queued_loader_layout_the_edits() {
    let (mut doc, parent, kids) = settled_column_queued_again();
    assert!(doc.loader_relayout_roots.contains(&parent));
    doc.remove(kids[0]).unwrap();
    assert!(doc.loader_relayout_roots.is_empty(), "the root is no longer the loader's");
    drain(&mut doc);
    assert_eq!(doc.fig_kept_node_matches_snapshot(kids[1]), Some(false), "the sibling the removal pulled up is not blessed");
}

#[test]
fn inserting_a_child_makes_the_queued_loader_layout_the_edits() {
    let (mut doc, parent, kids) = column_with_store();
    let id = doc.alloc_id();
    doc.insert(parent, Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
    assert!(doc.loader_relayout_roots.is_empty());
    drain(&mut doc);
    assert_eq!(doc.fig_kept_node_matches_snapshot(kids[0]), Some(true), "nothing moved the first child");
}

#[test]
fn reparenting_a_child_makes_the_queued_loader_layout_the_edits() {
    let (mut doc, parent, kids) = settled_column_queued_again();
    let page = doc.get(parent).unwrap().parent.unwrap();
    doc.reparent(kids[0], page, 0).unwrap();
    assert!(doc.loader_relayout_roots.is_empty());
    drain(&mut doc);
    assert_eq!(doc.fig_kept_node_matches_snapshot(kids[1]), Some(false));
}

#[test]
fn undoing_a_removal_makes_the_queued_loader_layout_the_edits() {
    let (mut doc, parent, kids) = column_with_store();
    let mut history = crate::History::default();
    history.exec(&mut doc, crate::Command::Remove { id: kids[0] }).unwrap();
    // the removal's own tag drop is not what this test is about: put the tag back as if the loader had queued it after
    doc.loader_relayout_roots.insert(parent);
    history.undo(&mut doc).unwrap();
    assert!(doc.loader_relayout_roots.is_empty(), "an undo that puts a child back is an edit");
}

#[test]
fn a_whole_tree_dirty_mark_from_an_edit_drops_every_tag_and_a_page_load_does_not() {
    let (mut doc, columns, _) = columns_with_store(2, &[0, 1]);
    assert_eq!(doc.loader_relayout_roots.len(), 2);
    doc.record_fig_kept_writes(|d| d.mark_tree_full_dirty());
    assert_eq!(doc.loader_relayout_roots.len(), 2, "a page load's own mark leaves the loader's roots");
    doc.mark_tree_full_dirty();
    assert!(doc.loader_relayout_roots.is_empty(), "an edit's mark drops them: any node may have changed");
    let _ = columns;
    let (mut doc, _, _) = columns_with_store(1, &[0]);
    doc.mark_full_dirty();
    assert!(doc.loader_relayout_roots.is_empty());
}

#[test]
fn an_edits_layout_after_the_open_has_settled_moves_nothing() {
    let (mut doc, parent, kids) = column_with_store();
    drain(&mut doc);
    doc.get_mut(kids[0]).unwrap().size.y = 40.0;
    doc.pending_relayout.push(parent);
    drain(&mut doc);
    assert_eq!(doc.fig_kept_node_matches_snapshot(kids[0]), Some(false));
    assert_eq!(doc.fig_kept_node_matches_snapshot(kids[1]), Some(false), "the second child moved because of the edit");
}

#[test]
fn edits_that_flush_through_any_entry_point_advance_no_snapshot() {
    // MCP, CLI, a spec, a library import: each queues through a solve and drains with the plain flush
    let (mut doc, columns, kids) = columns_with_store(1, &[]);
    doc.get_mut(kids[0][0]).unwrap().size.y = 40.0;
    doc.pending_relayout.push(columns[0]);
    doc.flush_pending_relayout();
    assert_eq!(doc.fig_kept_node_matches_snapshot(kids[0][1]), Some(false));
    assert!(doc.loader_relayout_roots.is_empty());
}

#[test]
fn the_leftovers_of_a_deferred_page_drained_after_the_open_still_advance_only_the_loaders_nodes() {
    let (mut doc, columns, kids) = columns_with_store(2, &[]);
    // a page load whose layout ran out of depth: the root it could not finish is queued by the loader
    let depth = crate::layout_apply::MAX_RELAYOUT_RESOLVE_DEPTH;
    let (_, written) = doc.record_fig_kept_writes(|d| {
        let mut budget = 10;
        d.relayout_top_down(columns[0], depth + 1, &mut budget);
    });
    doc.advance_fig_kept_snapshots(&written);
    assert_eq!(doc.pending_relayout(), &[columns[0]]);
    // an edit queued another root meanwhile
    doc.get_mut(kids[1][0]).unwrap().size.y = 40.0;
    doc.pending_relayout.push(columns[1]);
    // ten frames later both drain
    drain(&mut doc);
    assert_eq!(doc.fig_kept_node_matches_snapshot(kids[0][1]), Some(true), "the loader's leftover advanced");
    assert_eq!(doc.fig_kept_node_matches_snapshot(kids[1][1]), Some(false), "the edit's leftover did not");
}

#[test]
fn a_root_the_loader_left_queued_while_solving_a_loader_root_is_the_loaders_too() {
    let (mut doc, parent, _) = column_with_store();
    let queued = doc.pending_relayout().to_vec();
    assert_eq!(queued, vec![parent]);
    assert!(doc.loader_relayout_roots.contains(&parent));
    drain(&mut doc);
    assert!(doc.loader_relayout_roots.is_empty(), "a solved root is no longer tagged");
}

#[test]
fn a_loader_step_moves_snapshots_in_place_and_a_shared_store_is_copied_only_for_the_one_that_writes() {
    let (mut doc, ids) = document_with_store(3);
    let at = Arc::as_ptr(&doc.fig_kept);
    let (_, written) = doc.record_fig_kept_writes(|d| d.get_mut(ids[0]).unwrap().size.x += 40.0);
    doc.advance_fig_kept_snapshots(&written);
    assert_eq!(doc.fig_kept_node_matches_snapshot(ids[0]), Some(true));
    assert_eq!(Arc::as_ptr(&doc.fig_kept), at, "the store was written in place, not rebuilt");

    // a clone shares the store; the document that writes gets its own copy, the other keeps what it had
    let mut other = doc.clone();
    assert!(Arc::ptr_eq(&doc.fig_kept, &other.fig_kept));
    let (_, written) = doc.record_fig_kept_writes(|d| d.get_mut(ids[1]).unwrap().size.x += 40.0);
    doc.advance_fig_kept_snapshots(&written);
    assert!(!Arc::ptr_eq(&doc.fig_kept, &other.fig_kept));
    assert!(doc.fig_kept.shares_data_with(&other.fig_kept), "the arena and records are still shared");
    assert_eq!(doc.fig_kept_node_matches_snapshot(ids[1]), Some(true));
    other.get_mut(ids[1]).unwrap().size.x += 40.0;
    assert_eq!(other.fig_kept_node_matches_snapshot(ids[1]), Some(false), "the clone's snapshot did not move");
}

#[test]
fn a_layer_moved_under_another_parent_reads_as_moved_and_the_move_is_no_edit_of_its_value() {
    let (mut doc, parent, kids) = column_with_store();
    let page = doc.get(parent).unwrap().parent.unwrap();
    assert_eq!(doc.fig_kept_node_moved(kids[0]), Some(false));
    assert_eq!(doc.fig_kept_node_moved(parent), Some(false));
    doc.reparent(kids[0], page, 0).unwrap();
    assert_eq!(doc.fig_kept_node_moved(kids[0]), Some(true), "it sits under the page now");
    assert_eq!(doc.fig_kept_node_moved(kids[1]), Some(false), "its sibling did not move");
    assert_eq!(doc.fig_kept_node_matches_snapshot(kids[0]), Some(true), "the move is not part of the value the other keys are gated on");
    doc.reparent(kids[0], parent, 0).unwrap();
    assert_eq!(doc.fig_kept_node_moved(kids[0]), Some(false), "put back, it is where the file had it");
    assert_eq!(doc.fig_kept_node_moved(NodeId(u64::MAX)), None);
}
