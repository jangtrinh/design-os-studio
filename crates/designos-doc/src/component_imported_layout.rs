//! Keeps the geometry an imported `.fig` stored for an instance across a main sync that does not change Studio's own
//! solve of that instance.
//!
//! Figma stores the laid-out sizes, positions and outlines of a resized instance's layers. Studio's sync lays the
//! instance out again with its own solver, which does not always land on Figma's numbers, so a sync with no change to
//! the main moved layers (#1919). The rule here:
//!
//! *An imported top-level instance keeps its stored geometry exactly as long as Studio's canonical solve of the whole
//! instance (the sync a record-free instance gets) gives the result it gave right after import.*
//!
//! The sync itself always runs unchanged. Around it, [`before_sync`] notes the geometry of each recorded instance the
//! sync reaches, and [`after_sync`] either writes that geometry back (only the layout fields, see [`hash_layer`]) or
//! drops the record. The result can therefore differ from a record-free sync only in the layout fields, and only while
//! Studio's own solve of the instance is unchanged since import. Nothing about the solver's inputs is enumerated: any
//! input that moves the solve (stroke insets, variable values, modes, nested hug frames, text measure) moves the hash.

use crate::branch_diff::StableHasher;
use crate::components::{instance_root_of, sync_one, Ctx};
use std::collections::BTreeSet;

use crate::{Affine, Document, ImportedLayout, Node, NodeId, NodeKind, Path, StretchedAxes, StrokeGeometryStamp, Vec2};

/// Shape layers whose baked outline is drawn for the shape parameters their `kind` holds.
fn is_parametric_shape(kind: &NodeKind) -> bool {
    matches!(kind, NodeKind::Rectangle | NodeKind::Ellipse(_) | NodeKind::Star { .. } | NodeKind::Polygon { .. } | NodeKind::Line | NodeKind::BooleanOp(_))
}

/// Hashes what decides whether the stored geometry of one layer is still right.
///
/// The layout fields (the ones [`LayoutFields`] puts back): size, position (not on the instance root, whose position is
/// its own and never synced), the auto-layout stretch record, the baked fill and stroke outlines, and a vector's paths.
/// Then what those fields mean: corner radii and a shape's parameters, for which a baked outline was drawn. The layer's
/// identity is the main layer it mirrors and its child count, so a copy of an instance hashes like the original.
fn hash_layer(h: &mut StableHasher, n: &Node, is_root: bool) {
    h.add(&n.component.source);
    h.add(&(n.children.len() as u64));
    h.add(&n.size);
    if !is_root {
        h.add(&n.transform);
    }
    h.add(&n.layout_child.stretched);
    h.add(&n.fill_geometry);
    h.add(&n.stroke_geometry);
    h.add(&n.stroke_geometry_for);
    if matches!(n.kind, NodeKind::Vector { .. }) {
        h.add(&n.kind);
    }
    h.add(&meaning_hash(n));
}

/// What the baked outlines of `n` were drawn for, apart from the layout fields themselves: corner radii and the
/// parameters of a shape. (A vector's paths are a layout field: Figma stores them resized with the instance.)
fn meaning_hash(n: &Node) -> u64 {
    let mut h = StableHasher::new();
    h.add(&n.corner_radii);
    h.add(&n.corner_smoothing);
    if is_parametric_shape(&n.kind) {
        h.add(&n.kind);
    }
    h.finish()
}

/// The layers of the subtree under `root`, parents before children, in child order.
fn subtree(doc: &Document, root: NodeId) -> Option<Vec<NodeId>> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let n = doc.get(id)?;
        out.push(id);
        stack.extend(n.children.iter().rev().copied());
    }
    Some(out)
}

/// Hash of the layout of the subtree under `root`; `None` when a listed child is missing.
fn layout_hash(doc: &Document, root: NodeId) -> Option<u64> {
    let mut h = StableHasher::new();
    for id in subtree(doc, root)? {
        hash_layer(&mut h, doc.get(id)?, id == root);
    }
    Some(h.finish())
}

/// Studio's canonical solve of instance `holder` (the sync a record-free instance gets), hashed like [`layout_hash`].
/// The document is left as it was, id counter included: the trial's inserts and updates are all undone.
fn canonical_solve_hash(doc: &mut Document, holder: NodeId) -> Option<u64> {
    let (next_id, gap_cursor, layout_epoch) = (doc.next_id, doc.gap_cursor, doc.layout_epoch);
    let mut c = Ctx::new(doc);
    sync_one(&mut c, holder);
    let hash = layout_hash(c.s, holder);
    drop(c.finish());
    doc.next_id = next_id;
    doc.gap_cursor = gap_cursor;
    doc.layout_epoch = layout_epoch;
    hash
}

/// Whether `id` is an instance root that no other instance holds.
fn is_holder(doc: &Document, id: NodeId) -> bool {
    doc.get(id).is_some_and(|n| matches!(n.kind, NodeKind::Instance { .. })) && instance_root_of(doc, id) == Some(id)
}

/// Records, on each top-level instance in `holders`, the two hashes a later main sync needs to keep the geometry the
/// instance holds now. Called by an importer right after it wrote a file's stored geometry into these instances and
/// settled everything a solve reads (variables, styles, style metadata). An instance whose stored geometry Studio's
/// sync already reproduces gets no record: a sync cannot move it, so there is nothing to keep.
pub fn record_imported_layouts(doc: &mut Document, holders: impl IntoIterator<Item = NodeId>) {
    let mut ids: Vec<NodeId> = holders.into_iter().collect();
    ids.sort_unstable();
    ids.dedup();
    for id in ids {
        let record = if is_holder(doc, id) {
            match (layout_hash(doc, id), canonical_solve_hash(doc, id)) {
                (Some(stored), Some(solved)) if stored != solved => Some(ImportedLayout { stored, solved }),
                _ => None,
            }
        } else {
            None
        };
        if let Some(n) = doc.get_mut(id) {
            n.component.imported_layout = record;
        }
    }
}

/// Records the instances an importer requested ([`ImportedLayout::REQUESTED`]), see [`record_imported_layouts`].
pub fn record_requested_imported_layouts(doc: &mut Document) {
    let requested: Vec<NodeId> = doc.nodes.values().filter(|n| n.component.imported_layout.is_some_and(|r| r.is_requested())).map(|n| n.id).collect();
    if !requested.is_empty() {
        record_imported_layouts(doc, requested);
    }
}

/// Hash of the document-wide data a solve reads besides the layers: variables, their collections, shared styles and
/// their metadata. An importer that records geometry later (when a deferred page opens) records only while this is
/// what it was at import, since a stored geometry is only true for the values it was laid out with.
pub fn layout_inputs_outside_layers_hash(doc: &Document) -> u64 {
    let mut h = StableHasher::new();
    h.add(&doc.variables);
    h.add(&doc.variable_collections);
    h.add(&doc.styles);
    h.add(&doc.style_meta);
    h.finish()
}

/// The layout fields of one layer, as written back when its stored geometry is kept.
struct LayoutFields {
    id: NodeId,
    size: Vec2,
    transform: Affine,
    stretched: StretchedAxes,
    fill_geometry: Option<Vec<Path>>,
    stroke_geometry: Option<Vec<Path>>,
    stroke_geometry_for: Option<StrokeGeometryStamp>,
    vector: Option<NodeKind>,
    meaning: u64,
}

impl LayoutFields {
    fn of(n: &Node) -> Self {
        Self {
            id: n.id,
            size: n.size,
            transform: n.transform,
            stretched: n.layout_child.stretched,
            fill_geometry: n.fill_geometry.clone(),
            stroke_geometry: n.stroke_geometry.clone(),
            stroke_geometry_for: n.stroke_geometry_for.clone(),
            vector: matches!(n.kind, NodeKind::Vector { .. }).then(|| n.kind.clone()),
            meaning: meaning_hash(n),
        }
    }

    fn write_to(&self, n: &mut Node, is_root: bool) {
        n.size = self.size;
        if !is_root {
            n.transform = self.transform;
        }
        n.layout_child.stretched = self.stretched;
        n.fill_geometry.clone_from(&self.fill_geometry);
        n.stroke_geometry.clone_from(&self.stroke_geometry);
        n.stroke_geometry_for.clone_from(&self.stroke_geometry_for);
        if let Some(kind) = &self.vector {
            n.kind = kind.clone();
        }
    }
}

/// A recorded instance a sync is about to reach.
pub(crate) struct Held {
    holder: NodeId,
    record: ImportedLayout,
    /// The layout of every layer as it was before the sync, when the instance still held its imported geometry.
    before: Option<Vec<LayoutFields>>,
}

/// Notes, before a sync of `targets`, the geometry of every recorded instance the sync reaches. Cheap when no record
/// is reached: one walk up from each target.
pub(crate) fn before_sync(doc: &Document, targets: &[NodeId]) -> Vec<Held> {
    let mut holders: BTreeSet<NodeId> = BTreeSet::new();
    let mut held: Vec<Held> = Vec::new();
    for &t in targets {
        let Some(holder) = instance_root_of(doc, t) else { continue };
        if !holders.insert(holder) {
            continue;
        }
        let Some(record) = doc.get(holder).and_then(|n| n.component.imported_layout) else { continue };
        let before = (layout_hash(doc, holder) == Some(record.stored))
            .then(|| subtree(doc, holder))
            .flatten()
            .and_then(|ids| ids.iter().map(|id| doc.get(*id).map(LayoutFields::of)).collect::<Option<Vec<_>>>());
        held.push(Held { holder, record, before });
    }
    held
}

/// Number of main components around `id`: an instance inside a main is laid out before the instances of that main.
fn mains_around(doc: &Document, id: NodeId) -> usize {
    let mut n = 0;
    let mut cur = doc.get(id).and_then(|x| x.parent);
    while let Some(p) = cur {
        let Some(pn) = doc.get(p) else { break };
        if matches!(pn.kind, NodeKind::Component(_)) {
            n += 1;
        }
        cur = pn.parent;
    }
    n
}

/// After the sync of `targets`: for each instance [`before_sync`] noted, writes its stored geometry back when the
/// instance held it before the sync, Studio's canonical solve of it is the one recorded at import, and the sync left
/// its layers and their shape parameters as they were; otherwise drops the record, and the sync's own layout stands.
///
/// The solve is checked on the document as the sync left it, so the instance sees its references already synced, as
/// at import. Instances inside a main go first, so an instance of that main sees them back on their stored geometry.
pub(crate) fn after_sync(c: &mut Ctx, mut held: Vec<Held>) {
    if held.is_empty() {
        return;
    }
    held.sort_by_key(|h| (std::cmp::Reverse(mains_around(c.s, h.holder)), h.holder.0));
    for h in held {
        let keep = h.before.as_ref().is_some_and(|before| {
            canonical_solve_hash(c.s, h.holder) == Some(h.record.solved)
                && subtree(c.s, h.holder).is_some_and(|ids| {
                    ids.len() == before.len() && ids.iter().zip(before).all(|(id, b)| *id == b.id && c.s.get(*id).is_some_and(|n| meaning_hash(n) == b.meaning))
                })
        });
        if keep {
            for b in h.before.iter().flatten() {
                if let Some(mut n) = c.s.get(b.id).cloned() {
                    b.write_to(&mut n, b.id == h.holder);
                    c.update(n);
                }
            }
        } else if let Some(mut n) = c.s.get(h.holder).cloned() {
            n.component.imported_layout = None;
            c.update(n);
        }
    }
}
