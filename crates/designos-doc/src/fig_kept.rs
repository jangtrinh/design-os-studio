//! Figma's own records of a `.fig` open, kept so a later save can write back what Studio cannot model.
//!
//! A `.fig` open keeps the inflated message payload once, as an arena ([`KeptArena`]), and for every record of it the
//! `(field number, offset, length)` of each field worth keeping. A value is read where Figma wrote it: nothing is copied
//! per record. A payload past the RAM threshold lives in a spill file that is memory-mapped.
//!
//! The store is not part of a [`Node`]: it is keyed by [`NodeId`], so a duplicate, a paste or a node an agent creates has a
//! new id and never inherits what Figma wrote for another node. It is immutable once built; a snapshot of the nodes (see
//! [`NodeSnapshot`]) replaces the node table and shares everything else, so a document clone costs one `Arc` clone.
//! Nothing here changes what a save writes.

use crate::fig_kept_arena::KeptArena;
use crate::fig_kept_snapshot::NodeSnapshot;
use crate::{Document, Node, NodeId};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

/// One field of a kept record: the field number of the file's own schema, and where its value (the bytes after the field
/// number, up to the next one) sits in the arena.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeptField {
    pub no: u32,
    pub offset: u32,
    pub len: u32,
}

/// Where the bytes of one entry of the file's blob table sit in the arena.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlobSlot {
    pub offset: u32,
    pub len: u32,
}

impl BlobSlot {
    /// A slot that names no bytes: the entry of the blob table it stood for pointed outside the arena.
    pub const ABSENT: BlobSlot = BlobSlot { offset: u32::MAX, len: u32::MAX };
}

/// What a record is: a layer, whose structural fields (`guid`, `phase`, `parentIndex`) the tree writer owns and are not
/// kept, or any other record (a page, the document, a style, a variable), kept whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeptRecordKind {
    Layer,
    Other,
}

/// A kept record: the file's guid for it and its fields, as a run of the field table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeptRecord {
    pub guid: (u32, u32),
    pub kind: KeptRecordKind,
    first: u32,
    count: u32,
}

impl KeptRecord {
    pub fn new(guid: (u32, u32), kind: KeptRecordKind, first: u32, count: u32) -> Self {
        Self { guid, kind, first, count }
    }

    /// Where this record's run starts in the field table, and how long it is.
    pub fn first_index(&self) -> usize {
        self.first as usize
    }

    pub fn field_count(&self) -> usize {
        self.count as usize
    }
}

/// What the capture counted, for the import report.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeptCounts {
    /// Records the file holds.
    pub records_seen: usize,
    /// Records the store keeps.
    pub records_kept: usize,
    /// Fields the store keeps ranges for.
    pub fields_kept: usize,
    /// Fields left out because the file's definition of them is not the one Studio writes with, so their bytes could not be
    /// written back as they are.
    pub kept_fields_not_conformed: usize,
    /// Fields of the file's heavy kind (`editInfo`, `derivedImmutableFrameData`) left out because keeping them would have taken
    /// the decoder past its value or zero-size cap: they were not charged and are not kept.
    pub kept_fields_over_decode_cap: usize,
    /// What the decoder's value cap had left when the decode finished, and what keeping the heavy fields charged beyond a plain
    /// decode (0 when they did not fit and were left out).
    pub decode_values_left: usize,
    pub kept_extra_values_charged: usize,
    /// Entries of the blob table whose slot pointed outside the arena: they read as absent.
    pub dropped_blob_slots: usize,
    /// Records left out because they passed a bound (their size, their field count, the arena's size or the disk budget) or
    /// held a range the arena does not.
    pub dropped_kept_records: usize,
}

/// Everything the store shares between a document and its clones.
#[derive(Debug, Default)]
pub struct KeptData {
    arena: KeptArena,
    records: Vec<KeptRecord>,
    fields: Vec<KeptField>,
    blobs: Vec<BlobSlot>,
    by_guid: HashMap<(u32, u32), u32>,
    counts: KeptCounts,
}

impl KeptData {
    /// Builds the store's shared part. A record or blob slot that names bytes outside the arena is dropped and counted,
    /// never trusted: what built the ranges is a decoder, but the store does not rely on it.
    pub fn new(arena: KeptArena, records: Vec<KeptRecord>, mut fields: Vec<KeptField>, blobs: Vec<BlobSlot>, mut counts: KeptCounts) -> Self {
        let size = arena.len() as u64;
        let fits = |offset: u32, len: u32| u64::from(offset) + u64::from(len) <= size;
        let mut kept = Vec::with_capacity(records.len());
        let mut by_guid = HashMap::with_capacity(records.len());
        for rec in records {
            let run = (rec.first as usize).checked_add(rec.count as usize).and_then(|end| fields.get(rec.first as usize..end));
            let sound = run.is_some_and(|run| run.iter().all(|f| fits(f.offset, f.len)));
            if !sound {
                counts.dropped_kept_records += 1;
                counts.records_kept = counts.records_kept.saturating_sub(1);
                continue;
            }
            by_guid.entry(rec.guid).or_insert(kept.len() as u32);
            kept.push(rec);
        }
        let mut blobs: Vec<BlobSlot> = blobs
            .into_iter()
            .map(|b| {
                if fits(b.offset, b.len) {
                    b
                } else {
                    counts.dropped_blob_slots += 1;
                    BlobSlot::ABSENT
                }
            })
            .collect();
        // the vectors grew by doubling while the capture ran; what is kept for the life of the document holds no spare room
        fields.shrink_to_fit();
        kept.shrink_to_fit();
        blobs.shrink_to_fit();
        by_guid.shrink_to_fit();
        Self { arena, records: kept, fields, blobs, by_guid, counts }
    }
}

/// A node's entry: the record kept for it, and the node's value when the snapshot was taken.
#[derive(Clone, Copy, Debug)]
struct KeptNode {
    record: u32,
    snapshot: NodeSnapshot,
}

/// A document's store of Figma's records. See the module documentation.
#[derive(Clone, Debug, Default)]
pub struct FigKeptStore {
    data: Arc<KeptData>,
    nodes: HashMap<NodeId, KeptNode>,
}

/// A kept record and the arena it points into.
#[derive(Clone, Copy)]
pub struct KeptRecordRef<'a> {
    data: &'a KeptData,
    rec: &'a KeptRecord,
}

impl<'a> KeptRecordRef<'a> {
    pub fn guid(&self) -> (u32, u32) {
        self.rec.guid
    }

    pub fn kind(&self) -> KeptRecordKind {
        self.rec.kind
    }

    fn run(&self) -> &'a [KeptField] {
        &self.data.fields[self.rec.first as usize..self.rec.first as usize + self.rec.count as usize]
    }

    /// The kept fields, each as its field number and the bytes of its value, in the order of the file.
    pub fn fields(&self) -> impl Iterator<Item = (u32, &'a [u8])> + 'a {
        let bytes = self.data.arena.bytes();
        self.run().iter().map(move |f| (f.no, &bytes[f.offset as usize..f.offset as usize + f.len as usize]))
    }

    /// The bytes of the value of field `no`, when the record keeps it.
    pub fn field(&self, no: u32) -> Option<&'a [u8]> {
        self.fields().find(|(n, _)| *n == no).map(|(_, bytes)| bytes)
    }
}

/// What the store holds in memory, counted. A mapped arena holds none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RetainedBytes {
    pub arena_memory: usize,
    pub arena_mapped: usize,
    pub records: usize,
    pub ranges: usize,
    pub blob_slots: usize,
    pub guid_index: usize,
    pub node_table: usize,
    pub snapshots: usize,
}

impl RetainedBytes {
    /// Everything counted except the mapped arena.
    pub fn total_in_memory(&self) -> usize {
        self.arena_memory + self.records + self.ranges + self.blob_slots + self.guid_index + self.node_table + self.snapshots
    }
}

impl FigKeptStore {
    pub fn new(data: KeptData) -> Self {
        Self { data: Arc::new(data), nodes: HashMap::new() }
    }

    /// Whether the store keeps no record.
    pub fn is_empty(&self) -> bool {
        self.data.records.is_empty()
    }

    pub fn counts(&self) -> KeptCounts {
        self.data.counts
    }

    pub fn arena(&self) -> &KeptArena {
        &self.data.arena
    }

    /// Whether `other` shares this store's records and arena: a clone of the document, or a later snapshot of it.
    pub fn shares_data_with(&self, other: &FigKeptStore) -> bool {
        Arc::ptr_eq(&self.data, &other.data)
    }

    /// How many records the store keeps.
    pub fn len(&self) -> usize {
        self.data.records.len()
    }

    /// Every kept record, in the order of the file.
    pub fn records(&self) -> impl Iterator<Item = KeptRecordRef<'_>> {
        self.data.records.iter().map(|rec| KeptRecordRef { data: &self.data, rec })
    }

    /// The record the file holds under `guid`.
    pub fn record_by_guid(&self, guid: (u32, u32)) -> Option<KeptRecordRef<'_>> {
        let idx = *self.data.by_guid.get(&guid)?;
        self.record(idx)
    }

    fn record(&self, idx: u32) -> Option<KeptRecordRef<'_>> {
        Some(KeptRecordRef { data: &self.data, rec: self.data.records.get(idx as usize)? })
    }

    /// The record kept for node `id`.
    pub fn record_of_node(&self, id: NodeId) -> Option<KeptRecordRef<'_>> {
        self.record(self.nodes.get(&id)?.record)
    }

    /// The bytes of entry `index` of the file's blob table.
    pub fn blob(&self, index: usize) -> Option<&[u8]> {
        let slot = self.data.blobs.get(index).filter(|s| **s != BlobSlot::ABSENT)?;
        self.data.arena.bytes().get(slot.offset as usize..slot.offset as usize + slot.len as usize)
    }

    /// How many entries the file's blob table has.
    pub fn blob_count(&self) -> usize {
        self.data.blobs.len()
    }

    /// The value node `id` had when the snapshot was taken.
    pub fn node_snapshot(&self, id: NodeId) -> Option<&NodeSnapshot> {
        self.nodes.get(&id).map(|n| &n.snapshot)
    }

    /// How many nodes have a record and a snapshot.
    pub fn snapshot_count(&self) -> usize {
        self.nodes.len()
    }

    /// The memory the store holds, counted by part.
    pub fn retained_bytes(&self) -> RetainedBytes {
        let d = &self.data;
        let table = |len_capacity: usize, entry: usize| len_capacity * (entry + 1);
        let snapshots = self.nodes.len() * std::mem::size_of::<NodeSnapshot>();
        RetainedBytes {
            arena_memory: d.arena.memory_bytes(),
            arena_mapped: if d.arena.is_file_backed() { d.arena.len() } else { 0 },
            records: d.records.capacity() * std::mem::size_of::<KeptRecord>(),
            ranges: d.fields.capacity() * std::mem::size_of::<KeptField>(),
            blob_slots: d.blobs.capacity() * std::mem::size_of::<BlobSlot>(),
            guid_index: table(d.by_guid.capacity(), std::mem::size_of::<((u32, u32), u32)>()),
            // a table entry holds its snapshot, which is counted apart
            node_table: table(self.nodes.capacity(), std::mem::size_of::<(NodeId, KeptNode)>()).saturating_sub(snapshots),
            snapshots,
        }
    }

    /// The same store with every node key `f` maps; the records and the arena are shared. `snapshot_of(old, new)` gives a node a
    /// new snapshot where its renumbering changed what the snapshot hashes (a node that names other nodes by id).
    pub fn remapped(&self, f: impl Fn(NodeId) -> NodeId, snapshot_of: impl Fn(NodeId, NodeId) -> Option<NodeSnapshot>) -> FigKeptStore {
        let nodes = self
            .nodes
            .iter()
            .map(|(id, n)| {
                let new = f(*id);
                (new, KeptNode { snapshot: snapshot_of(*id, new).unwrap_or(n.snapshot), ..*n })
            })
            .collect();
        FigKeptStore { data: self.data.clone(), nodes }
    }

    fn with_nodes(&self, nodes: HashMap<NodeId, KeptNode>) -> FigKeptStore {
        FigKeptStore { data: self.data.clone(), nodes }
    }
}

/// The guid a node's `source_id` names (`"session:local"`), or `None` for a node with another id: a copy made from an
/// instance carries an `I...;` path, and a node made in Studio has none.
fn guid_of(node: &Node) -> Option<(u32, u32)> {
    let (session, local) = node.source_id.as_deref()?.split_once(':')?;
    let digits = |s: &str| (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse::<u32>().ok()).flatten();
    Some((digits(session)?, digits(local)?))
}

impl Document {
    /// Takes the value of every node a record was kept for, replacing the snapshots there were. The import calls it at its
    /// end. The layout it left queued is the loader's, not an edit's: those roots are tagged, so when they are solved later
    /// (see [`Self::flush_pending_relayout_step`]) the nodes the solver writes move their snapshot forward, and a layout an
    /// edit queued does not. A node reads as the file's own only through its source id, and a guid two nodes share goes to
    /// the one with the smaller id.
    pub fn take_fig_kept_snapshot(&mut self) {
        if self.fig_kept.is_empty() {
            return;
        }
        let queued = self.pending_relayout.clone();
        self.loader_relayout_roots.extend(queued);
        let store = self.fig_kept.clone();
        let mut seen = std::collections::HashSet::new();
        let mut wanted: Vec<(NodeId, u32, &Node)> = Vec::new();
        for (id, node) in &self.nodes {
            let Some(guid) = guid_of(node) else { continue };
            let Some(&record) = store.data.by_guid.get(&guid) else { continue };
            if seen.insert(guid) {
                wanted.push((*id, record, node));
            }
        }
        let nodes = snapshot_nodes(&wanted);
        self.fig_kept = Arc::new(store.with_nodes(nodes));
    }

    /// Whether node `id` still has the value its snapshot took: `Some(true)` for an unedited node, `Some(false)` for an edited
    /// one, `None` when no record was kept for it.
    pub fn fig_kept_node_matches_snapshot(&self, id: NodeId) -> Option<bool> {
        let snap = self.fig_kept.node_snapshot(id)?;
        Some(self.nodes.get(&id).is_some_and(|n| snap.matches(n)))
    }

    /// Moves forward the snapshot of each node a page loader wrote into (`before` holds each such node as it was just before its
    /// first write), but only of a node that still had its snapshot's value then: a node edited by anyone else stays different
    /// from its snapshot whatever the loader does to it afterwards. The same rule the page loader applies to the content of
    /// main components.
    pub(crate) fn advance_fig_kept_snapshots(&mut self, before: &BTreeMap<NodeId, Node>) {
        if before.is_empty() || self.fig_kept.snapshot_count() == 0 {
            return;
        }
        let mut moved: Vec<(NodeId, NodeSnapshot)> = Vec::new();
        for (id, was) in before {
            let Some(snap) = self.fig_kept.node_snapshot(*id) else { continue };
            if !snap.matches(was) {
                continue;
            }
            if let Some(now) = self.nodes.get(id) {
                moved.push((*id, NodeSnapshot::of(now)));
            }
        }
        if moved.is_empty() {
            return;
        }
        // in place while this document holds the only handle; a document that shares the store with a clone copies it once
        let store = Arc::make_mut(&mut self.fig_kept);
        for (id, snapshot) in moved {
            if let Some(n) = store.nodes.get_mut(&id) {
                n.snapshot = snapshot;
            }
        }
    }

    /// Runs a page load `f`, and returns the nodes it wrote as they were before: those are the nodes whose snapshot may move
    /// forward. With no snapshot to move, nothing is recorded and the load costs what it did.
    /// A layout `f` queues is the loader's: its roots are tagged (see [`Self::take_fig_kept_snapshot`]).
    pub(crate) fn record_fig_kept_writes<R>(&mut self, f: impl FnOnce(&mut Document) -> R) -> (R, BTreeMap<NodeId, Node>) {
        let was = std::mem::replace(&mut self.loader_solving, true);
        let out = if self.fig_kept.snapshot_count() == 0 { (f(self), BTreeMap::new()) } else { self.record_node_writes(f) };
        self.loader_solving = was;
        out
    }
}

/// Snapshots `wanted`, on worker threads when there are many.
fn snapshot_nodes(wanted: &[(NodeId, u32, &Node)]) -> HashMap<NodeId, KeptNode> {
    let one = |&(id, record, node): &(NodeId, u32, &Node)| (id, KeptNode { record, snapshot: NodeSnapshot::of(node) });
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 16);
    if wanted.len() < 2_048 || threads <= 1 {
        return wanted.iter().map(one).collect();
    }
    let chunk = wanted.len().div_ceil(threads);
    let parts: Vec<Vec<(NodeId, KeptNode)>> = std::thread::scope(|s| {
        let handles: Vec<_> = wanted.chunks(chunk).map(|c| s.spawn(move || c.iter().map(one).collect::<Vec<_>>())).collect();
        handles.into_iter().map(|h| h.join().expect("snapshot worker")).collect()
    });
    let mut out = HashMap::with_capacity(wanted.len());
    out.extend(parts.into_iter().flatten());
    out
}
