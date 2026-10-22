//! Three-way structural diff engine (`T10.2`, Issue `#344`).
//!
//! Computes changes between a common `base` [`Document`] and two descendants (`ours` and `theirs`):
//! per-layer `Added`, `Removed`, `Moved`, `Changed`, and `MovedAndChanged` classifications,
//! longest-common-subsequence (LCS) sibling move detection, per-property-group conflict detection,
//! `IdCollision` detection, and non-node entity diffs (`styles`, `variables`, `collections`,
//! `flows`, `comments`).

use crate::{Affine, Comment, CommentThread, Document, Flow, Node, NodeId, NodeKind, Style, StyleMeta, TextProps, Variable, VariableCollection, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::{Hash, Hasher};
use std::time::Instant;

/// Floating-point tolerance (`1e-3`) used when comparing `transform` and `size` components
/// so layout rounding noise does not trigger false changes or conflicts (`T10.2` §8.5).
pub const FLOAT_TOLERANCE: f32 = 1e-3;

/// Property groups on a [`Node`] that serve as the atomic unit of change and conflict (`T10.2` §3.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Group {
    Name,
    Visible,
    Locked,
    Opacity,
    Blend,
    Transform,
    Size,
    Fills,
    Stroke,
    Effects,
    CornerRadii,
    Constraints,
    LayoutChild,
    Mask,
    StyleRefs,
    Variables,
    Component,
    Prototype,
    Dev,
    ExportSettings,
    Kind,
    TextContent,
    TextStyle,
    Frame,
    Vector,
    PluginData,
    Other,
    Position,
}

impl Group {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Visible => "visible",
            Self::Locked => "locked",
            Self::Opacity => "opacity",
            Self::Blend => "blend",
            Self::Transform => "transform",
            Self::Size => "size",
            Self::Fills => "fills",
            Self::Stroke => "stroke",
            Self::Effects => "effects",
            Self::CornerRadii => "corner_radii",
            Self::Constraints => "constraints",
            Self::LayoutChild => "layout_child",
            Self::Mask => "mask",
            Self::StyleRefs => "style_refs",
            Self::Variables => "variables",
            Self::Component => "component",
            Self::Prototype => "prototype",
            Self::Dev => "dev",
            Self::ExportSettings => "export_settings",
            Self::Kind => "kind",
            Self::TextContent => "text.content",
            Self::TextStyle => "text.style",
            Self::Frame => "frame",
            Self::Vector => "vector",
            Self::PluginData => "plugin_data",
            Self::Other => "other",
            Self::Position => "position",
        }
    }
}

impl std::fmt::Display for Group {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Classification of how a single [`Node`] changed on one side relative to `base` (`T10.2` §3.3, §4).
///
/// For top-level pages (`parent == None`), the parent id in `from` / `to` is `NodeId(0)`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeChange {
    Added,
    Removed,
    Moved { from: (NodeId, usize), to: (NodeId, usize) },
    Changed { groups: BTreeSet<Group> },
    MovedAndChanged { from: (NodeId, usize), to: (NodeId, usize), groups: BTreeSet<Group> },
}

impl NodeChange {
    pub fn is_added(&self) -> bool {
        matches!(self, Self::Added)
    }

    pub fn is_removed(&self) -> bool {
        matches!(self, Self::Removed)
    }

    pub fn is_moved(&self) -> bool {
        matches!(self, Self::Moved { .. } | Self::MovedAndChanged { .. })
    }

    pub fn groups(&self) -> Option<&BTreeSet<Group>> {
        match self {
            Self::Changed { groups } | Self::MovedAndChanged { groups, .. } => Some(groups),
            _ => None,
        }
    }
}

/// Classification of how a non-node entity changed on one side relative to `base` (`T10.2` §3.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EntityChange {
    Added,
    Removed,
    Changed,
}

/// Changes on one descendant (`ours` or `theirs`) relative to `base` (`T10.2` §4).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SideChanges {
    pub nodes: BTreeMap<NodeId, NodeChange>,
    pub styles: BTreeMap<String, EntityChange>,
    pub variables: BTreeMap<String, EntityChange>,
    pub collections: BTreeMap<String, EntityChange>,
    pub flows: BTreeMap<String, EntityChange>,
    pub comments: BTreeMap<String, EntityChange>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub guides: BTreeMap<NodeId, EntityChange>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub images: BTreeMap<String, EntityChange>,
}

/// Kind of conflict on a layer or non-node entity (`T10.2` §3.4, §10).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ConflictKind {
    /// Both sides modified one or more of the same property groups (including `Group::Position`)
    /// to different values.
    ModifyModify,
    /// One side removed the entity while the other side modified it or added children under it.
    DeleteModify,
    /// One side added or re-pointed a node whose target component/dependency was removed on the other side.
    DependsOnRemoved,
}

/// A per-layer conflict between `ours` and `theirs` (`T10.2` §3.4, R5.18).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conflict {
    pub node: NodeId,
    pub groups: BTreeSet<Group>,
    pub kind: ConflictKind,
}

/// Key identifying a non-node entity for conflict reporting and resolution (`T10.2` §3.5, `T10.3` §3.2).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EntityKey {
    Style(String),
    Variable(String),
    Collection(String),
    Flow(String),
    Comment(String),
    Guides(NodeId),
    Image(String),
}

/// A conflict on a non-node entity (`styles`, `variables`, `collections`, `flows`, `comments`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityConflict {
    pub key: EntityKey,
    pub kind: ConflictKind,
}

/// Summary statistics for a [`Diff3`] run (`T10.2` §5, §8.3).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffStats {
    pub scanned: usize,
    pub changed_ours: usize,
    pub changed_theirs: usize,
    pub conflicts: usize,
    pub elapsed_ms: u64,
    /// Conflicts on mirror nodes inside an instance subtree, grouped under their enclosing instance id (`T10.2` §8.3).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub instance_conflicts: BTreeMap<NodeId, Vec<NodeId>>,
}

/// Complete three-way diff result between `base`, `ours`, and `theirs` (`T10.2` §4).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diff3 {
    pub ours: SideChanges,
    pub theirs: SideChanges,
    pub conflicts: Vec<Conflict>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entity_conflicts: Vec<EntityConflict>,
    pub id_collisions: Vec<NodeId>,
    pub stats: DiffStats,
}

/// Stable key for a [`Flow`] in [`SideChanges::flows`].
pub fn flow_key(flow: &Flow) -> String {
    format!("{}:{}", flow.page.0, flow.start.0)
}

// ============================================================================
// Zero-allocation streaming HashSerializer (CLAUDE.md §4.1, T10.2 §2 & §3.6)
// ============================================================================

#[derive(Default)]
struct FastHasher {
    state: u64,
}

impl FastHasher {
    const SEED: u64 = 0x517cc1b727220a95;

    #[inline]
    fn new() -> Self {
        Self { state: 0xcbf29ce484222325 }
    }

    #[inline]
    fn mix(&mut self, word: u64) {
        self.state = (self.state.rotate_left(5) ^ word).wrapping_mul(Self::SEED);
    }
}

impl Hasher for FastHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.state
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut i = 0;
        while i + 8 <= bytes.len() {
            let word = u64::from_le_bytes(bytes[i..i + 8].try_into().unwrap());
            self.mix(word);
            i += 8;
        }
        if i < bytes.len() {
            let mut tail = 0u64;
            for (shift, &b) in bytes[i..].iter().enumerate() {
                tail |= (b as u64) << (shift * 8);
            }
            self.mix(tail);
        }
        self.mix(bytes.len() as u64);
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.mix(i as u64);
    }
    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.mix(i as u64);
    }
    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.mix(i as u64);
    }
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.mix(i);
    }
    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.mix(i as u64);
    }
    #[inline]
    fn write_i8(&mut self, i: i8) {
        self.mix(i as u8 as u64);
    }
    #[inline]
    fn write_i16(&mut self, i: i16) {
        self.mix(i as u16 as u64);
    }
    #[inline]
    fn write_i32(&mut self, i: i32) {
        self.mix(i as u32 as u64);
    }
    #[inline]
    fn write_i64(&mut self, i: i64) {
        self.mix(i as u64);
    }
    #[inline]
    fn write_isize(&mut self, i: isize) {
        self.mix(i as u64);
    }
}

#[derive(Debug)]
struct HashError;

impl std::fmt::Display for HashError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("hash error")
    }
}

impl std::error::Error for HashError {}

impl serde::ser::Error for HashError {
    fn custom<T: std::fmt::Display>(_msg: T) -> Self {
        Self
    }
}

struct HashSerializer<'a>(&'a mut FastHasher);

impl<'a> serde::Serializer for HashSerializer<'a> {
    type Ok = ();
    type Error = HashError;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    #[inline]
    fn serialize_bool(self, v: bool) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    #[inline]
    fn serialize_i8(self, v: i8) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    #[inline]
    fn serialize_i16(self, v: i16) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    #[inline]
    fn serialize_i32(self, v: i32) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    #[inline]
    fn serialize_i64(self, v: i64) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    #[inline]
    fn serialize_i128(self, v: i128) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    #[inline]
    fn serialize_u8(self, v: u8) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    #[inline]
    fn serialize_u16(self, v: u16) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    #[inline]
    fn serialize_u32(self, v: u32) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    #[inline]
    fn serialize_u64(self, v: u64) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    #[inline]
    fn serialize_u128(self, v: u128) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    #[inline]
    fn serialize_f32(self, v: f32) -> Result<(), Self::Error> {
        v.to_bits().hash(self.0);
        Ok(())
    }
    #[inline]
    fn serialize_f64(self, v: f64) -> Result<(), Self::Error> {
        v.to_bits().hash(self.0);
        Ok(())
    }
    #[inline]
    fn serialize_char(self, v: char) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    #[inline]
    fn serialize_str(self, v: &str) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    #[inline]
    fn serialize_bytes(self, v: &[u8]) -> Result<(), Self::Error> {
        v.hash(self.0);
        Ok(())
    }
    #[inline]
    fn serialize_none(self) -> Result<(), Self::Error> {
        0u8.hash(self.0);
        Ok(())
    }
    #[inline]
    fn serialize_some<T: ?Sized + serde::Serialize>(self, value: &T) -> Result<(), Self::Error> {
        1u8.hash(self.0);
        value.serialize(self)
    }
    #[inline]
    fn serialize_unit(self) -> Result<(), Self::Error> {
        Ok(())
    }
    #[inline]
    fn serialize_unit_struct(self, name: &'static str) -> Result<(), Self::Error> {
        name.hash(self.0);
        Ok(())
    }
    #[inline]
    fn serialize_unit_variant(self, _name: &'static str, variant_index: u32, variant: &'static str) -> Result<(), Self::Error> {
        variant_index.hash(self.0);
        variant.hash(self.0);
        Ok(())
    }
    #[inline]
    fn serialize_newtype_struct<T: ?Sized + serde::Serialize>(self, _name: &'static str, value: &T) -> Result<(), Self::Error> {
        value.serialize(self)
    }
    #[inline]
    fn serialize_newtype_variant<T: ?Sized + serde::Serialize>(
        self,
        _name: &'static str,
        variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        variant_index.hash(self.0);
        variant.hash(self.0);
        value.serialize(self)
    }
    #[inline]
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        len.hash(self.0);
        Ok(self)
    }
    #[inline]
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(self)
    }
    #[inline]
    fn serialize_tuple_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Ok(self)
    }
    #[inline]
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        variant_index.hash(self.0);
        variant.hash(self.0);
        Ok(self)
    }
    #[inline]
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        len.hash(self.0);
        Ok(self)
    }
    #[inline]
    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(self)
    }
    #[inline]
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        variant_index.hash(self.0);
        variant.hash(self.0);
        Ok(self)
    }
}

impl<'a> serde::ser::SerializeSeq for HashSerializer<'a> {
    type Ok = ();
    type Error = HashError;
    #[inline]
    fn serialize_element<T: ?Sized + serde::Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(HashSerializer(self.0))
    }
    #[inline]
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl<'a> serde::ser::SerializeTuple for HashSerializer<'a> {
    type Ok = ();
    type Error = HashError;
    #[inline]
    fn serialize_element<T: ?Sized + serde::Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(HashSerializer(self.0))
    }
    #[inline]
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl<'a> serde::ser::SerializeTupleStruct for HashSerializer<'a> {
    type Ok = ();
    type Error = HashError;
    #[inline]
    fn serialize_field<T: ?Sized + serde::Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(HashSerializer(self.0))
    }
    #[inline]
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl<'a> serde::ser::SerializeTupleVariant for HashSerializer<'a> {
    type Ok = ();
    type Error = HashError;
    #[inline]
    fn serialize_field<T: ?Sized + serde::Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(HashSerializer(self.0))
    }
    #[inline]
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl<'a> serde::ser::SerializeMap for HashSerializer<'a> {
    type Ok = ();
    type Error = HashError;
    #[inline]
    fn serialize_key<T: ?Sized + serde::Serialize>(&mut self, key: &T) -> Result<(), Self::Error> {
        key.serialize(HashSerializer(self.0))
    }
    #[inline]
    fn serialize_value<T: ?Sized + serde::Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        value.serialize(HashSerializer(self.0))
    }
    #[inline]
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl<'a> serde::ser::SerializeStruct for HashSerializer<'a> {
    type Ok = ();
    type Error = HashError;
    #[inline]
    fn serialize_field<T: ?Sized + serde::Serialize>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error> {
        key.hash(self.0);
        value.serialize(HashSerializer(self.0))
    }
    #[inline]
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl<'a> serde::ser::SerializeStructVariant for HashSerializer<'a> {
    type Ok = ();
    type Error = HashError;
    #[inline]
    fn serialize_field<T: ?Sized + serde::Serialize>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error> {
        key.hash(self.0);
        value.serialize(HashSerializer(self.0))
    }
    #[inline]
    fn end(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Computes a 64-bit streaming hash of a node's property groups (excluding `parent` and `children`,
/// which belong to the `Position` / move pass).
fn hash_node_props(node: &Node) -> u64 {
    let mut h = FastHasher::new();
    node.name.hash(&mut h);
    node.visible.hash(&mut h);
    node.locked.hash(&mut h);
    node.opacity.to_bits().hash(&mut h);
    let _ = node.blend.serialize(HashSerializer(&mut h));
    for v in [node.transform.a, node.transform.b, node.transform.c, node.transform.d, node.transform.tx, node.transform.ty] {
        v.to_bits().hash(&mut h);
    }
    node.size.x.to_bits().hash(&mut h);
    node.size.y.to_bits().hash(&mut h);
    for r in node.corner_radii {
        r.to_bits().hash(&mut h);
    }
    node.corner_smoothing.to_bits().hash(&mut h);
    node.is_mask.hash(&mut h);
    node.lock_aspect.hash(&mut h);
    let _ = node.kind.serialize(HashSerializer(&mut h));
    if !node.fills.is_empty() {
        1u8.hash(&mut h);
        let _ = node.fills.serialize(HashSerializer(&mut h));
    }
    // Always hashed: a default-looking stroke (no paints, weight 1) still carries align, cap, join,
    // miter limit and dashes, and `node_group_eq` compares all of them.
    2u8.hash(&mut h);
    let _ = node.stroke.serialize(HashSerializer(&mut h));
    if !node.effects.is_empty() {
        3u8.hash(&mut h);
        let _ = node.effects.serialize(HashSerializer(&mut h));
    }
    let _ = node.constraints.serialize(HashSerializer(&mut h));
    let _ = node.layout_child.serialize(HashSerializer(&mut h));
    if !node.style_refs.is_empty() {
        4u8.hash(&mut h);
        let _ = node.style_refs.serialize(HashSerializer(&mut h));
    }
    if node.fill_geometry.is_some() || node.stroke_geometry.is_some() {
        5u8.hash(&mut h);
        let _ = node.fill_geometry.serialize(HashSerializer(&mut h));
        let _ = node.stroke_geometry.serialize(HashSerializer(&mut h));
    }
    if !node.component.is_empty() {
        6u8.hash(&mut h);
        let _ = node.component.serialize(HashSerializer(&mut h));
    }
    if !node.prototype.is_empty() {
        7u8.hash(&mut h);
        let _ = node.prototype.serialize(HashSerializer(&mut h));
    }
    if !node.variables.is_empty() {
        8u8.hash(&mut h);
        let _ = node.variables.serialize(HashSerializer(&mut h));
    }
    if !node.export_settings.is_empty() {
        9u8.hash(&mut h);
        let _ = node.export_settings.serialize(HashSerializer(&mut h));
    }
    if !node.dev.is_empty() {
        10u8.hash(&mut h);
        let _ = node.dev.serialize(HashSerializer(&mut h));
    }
    if node.source_id.is_some()
        || node.slide.is_some()
        || node.hyperlink.is_some()
        || !node.plugin_data.is_empty()
        || node.detached_symbol_id.is_some()
        || !node.fig_metadata.is_empty()
    {
        11u8.hash(&mut h);
        node.source_id.hash(&mut h);
        let _ = node.slide.serialize(HashSerializer(&mut h));
        node.hyperlink.hash(&mut h);
        let _ = node.plugin_data.serialize(HashSerializer(&mut h));
        node.detached_symbol_id.hash(&mut h);
        let _ = node.fig_metadata.serialize(HashSerializer(&mut h));
    }
    h.finish()
}

/// Computes per-node 64-bit hashes across all nodes in `doc` using a worker thread pool when the
/// document is large (`T10.2` §3.6).
fn compute_doc_node_hashes(doc: &Document) -> HashMap<NodeId, u64> {
    let entries: Vec<(NodeId, &Node)> = doc.nodes.iter().map(|(&id, n)| (id, n)).collect();
    let len = entries.len();
    if len == 0 {
        return HashMap::new();
    }
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 16);
    if len < 2_048 || threads <= 1 {
        let mut out = HashMap::with_capacity(len);
        for (id, node) in entries {
            out.insert(id, hash_node_props(node));
        }
        return out;
    }

    let chunk_size = len.div_ceil(threads);
    let chunk_vecs: Vec<Vec<(NodeId, u64)>> = std::thread::scope(|s| {
        let handles: Vec<_> = entries
            .chunks(chunk_size)
            .map(|chunk| {
                s.spawn(move || {
                    let mut local = Vec::with_capacity(chunk.len());
                    for &(id, node) in chunk {
                        local.push((id, hash_node_props(node)));
                    }
                    local
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().expect("diff3 worker thread panicked")).collect()
    });

    let mut out = HashMap::with_capacity(len);
    for chunk in chunk_vecs {
        for (id, hash) in chunk {
            out.insert(id, hash);
        }
    }
    out
}

// ============================================================================
// Group-level Node Comparison with Tolerance (T10.2 §3.2, §8.1, §8.5)
// ============================================================================

#[inline]
fn approx_eq_f32(a: f32, b: f32) -> bool {
    if a.to_bits() == b.to_bits() {
        return true;
    }
    (a - b).abs() < FLOAT_TOLERANCE
}

#[inline]
pub fn approx_eq_transform(a: Affine, b: Affine) -> bool {
    approx_eq_f32(a.a, b.a)
        && approx_eq_f32(a.b, b.b)
        && approx_eq_f32(a.c, b.c)
        && approx_eq_f32(a.d, b.d)
        && approx_eq_f32(a.tx, b.tx)
        && approx_eq_f32(a.ty, b.ty)
}

#[inline]
pub fn approx_eq_size(a: Vec2, b: Vec2) -> bool {
    approx_eq_f32(a.x, b.x) && approx_eq_f32(a.y, b.y)
}

fn eq_text_style_without_content(a: Option<&TextProps>, b: Option<&TextProps>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(ta), Some(tb)) => {
            ta.style == tb.style
                && ta.runs == tb.runs
                && ta.auto_resize == tb.auto_resize
                && ta.max_lines == tb.max_lines
                && ta.paragraph == tb.paragraph
                && ta.lists == tb.lists
                && ta.direction == tb.direction
                && ta.auto_rename == tb.auto_rename
                && ta.decoration_skip_ink == tb.decoration_skip_ink
                && ta.text_wrap == tb.text_wrap
        }
        _ => false,
    }
}

fn eq_vector_group(a: &Node, b: &Node) -> bool {
    if a.fill_geometry != b.fill_geometry || a.stroke_geometry != b.stroke_geometry {
        return false;
    }
    match (&a.kind, &b.kind) {
        (NodeKind::Vector { paths: pa, network: na }, NodeKind::Vector { paths: pb, network: nb }) => pa == pb && na == nb,
        (NodeKind::Vector { .. }, _) | (_, NodeKind::Vector { .. }) => false,
        _ => true,
    }
}

fn eq_component_group(a: &Node, b: &Node) -> bool {
    if a.component != b.component {
        return false;
    }
    let inst_a = match &a.kind {
        NodeKind::Instance { component, component_key, .. } => Some((*component, component_key.as_deref())),
        _ => None,
    };
    let inst_b = match &b.kind {
        NodeKind::Instance { component, component_key, .. } => Some((*component, component_key.as_deref())),
        _ => None,
    };
    inst_a == inst_b
}

/// Compares `NodeKind` discriminant and kind-specific payloads that are NOT covered by
/// `Group::Frame`, `Group::TextContent`, `Group::TextStyle`, `Group::Vector`, or `Group::Component`.
///
/// This ensures converting a `Frame` to a `Component` (Edge Case 1, §8.1) only triggers `Group::Kind`
/// without falsely triggering `Group::Frame`.
fn eq_kind_group(a: &NodeKind, b: &NodeKind) -> bool {
    match (a, b) {
        (NodeKind::Page { background: ba }, NodeKind::Page { background: bb }) => ba == bb,
        (NodeKind::Frame(_), NodeKind::Frame(_)) => true,
        (NodeKind::Group, NodeKind::Group) => true,
        (NodeKind::Section, NodeKind::Section) => true,
        (NodeKind::Rectangle, NodeKind::Rectangle) => true,
        (NodeKind::Ellipse(aa), NodeKind::Ellipse(ab)) => aa == ab,
        (NodeKind::Line, NodeKind::Line) => true,
        (NodeKind::Polygon { count: ca }, NodeKind::Polygon { count: cb }) => ca == cb,
        (NodeKind::Star { count: ca, inner_ratio: ra }, NodeKind::Star { count: cb, inner_ratio: rb }) => ca == cb && ra == rb,
        (NodeKind::Vector { .. }, NodeKind::Vector { .. }) => true,
        (NodeKind::Text(_), NodeKind::Text(_)) => true,
        (NodeKind::BooleanOp(oa), NodeKind::BooleanOp(ob)) => oa == ob,
        (NodeKind::Component(_), NodeKind::Component(_)) => true,
        (NodeKind::ComponentSet(_), NodeKind::ComponentSet(_)) => true,
        (NodeKind::Instance { .. }, NodeKind::Instance { .. }) => true,
        (NodeKind::Sticky(sa), NodeKind::Sticky(sb)) => sa.color == sb.color && sa.author == sb.author && sa.show_author == sb.show_author,
        (NodeKind::Connector(ca), NodeKind::Connector(cb)) => {
            ca.start == cb.start && ca.end == cb.end && ca.line == cb.line && ca.start_cap == cb.start_cap && ca.end_cap == cb.end_cap
        }
        (NodeKind::ShapeWithText(sa), NodeKind::ShapeWithText(sb)) => sa.shape == sb.shape,
        (NodeKind::Stamp { kind: ka }, NodeKind::Stamp { kind: kb }) => ka == kb,
        (NodeKind::Unknown { type_name: ta }, NodeKind::Unknown { type_name: tb }) => ta == tb,
        _ => false,
    }
}

fn eq_other_group(a: &Node, b: &Node) -> bool {
    a.source_id == b.source_id
        && a.slide == b.slide
        && a.lock_aspect == b.lock_aspect
        && a.hyperlink == b.hyperlink
        && a.detached_symbol_id == b.detached_symbol_id
        && a.fig_metadata == b.fig_metadata
}

/// Returns whether a specific property [`Group`] is equal on nodes `a` and `b`.
pub fn node_group_eq(a: &Node, b: &Node, group: Group) -> bool {
    match group {
        Group::Name => a.name == b.name,
        Group::Visible => a.visible == b.visible,
        Group::Locked => a.locked == b.locked,
        Group::Opacity => approx_eq_f32(a.opacity, b.opacity),
        Group::Blend => a.blend == b.blend,
        Group::Transform => approx_eq_transform(a.transform, b.transform),
        Group::Size => approx_eq_size(a.size, b.size),
        Group::Fills => a.fills == b.fills,
        Group::Stroke => a.stroke == b.stroke,
        Group::Effects => a.effects == b.effects,
        Group::CornerRadii => a.corner_radii == b.corner_radii && approx_eq_f32(a.corner_smoothing, b.corner_smoothing),
        Group::Constraints => a.constraints == b.constraints,
        Group::LayoutChild => a.layout_child == b.layout_child,
        Group::Mask => a.is_mask == b.is_mask,
        Group::StyleRefs => a.style_refs == b.style_refs,
        Group::Variables => a.variables == b.variables,
        Group::Component => eq_component_group(a, b),
        Group::Prototype => a.prototype == b.prototype,
        Group::Dev => a.dev == b.dev,
        Group::ExportSettings => a.export_settings == b.export_settings,
        Group::Kind => eq_kind_group(&a.kind, &b.kind),
        Group::TextContent => a.kind.any_text().map(|t| &t.content) == b.kind.any_text().map(|t| &t.content),
        Group::TextStyle => eq_text_style_without_content(a.kind.any_text(), b.kind.any_text()),
        Group::Frame => a.kind.frame_props() == b.kind.frame_props(),
        Group::Vector => eq_vector_group(a, b),
        Group::PluginData => a.plugin_data == b.plugin_data,
        Group::Other => eq_other_group(a, b),
        Group::Position => a.parent == b.parent,
    }
}

const PROPERTY_GROUPS: [Group; 27] = [
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

/// Returns all property [`Group`]s (excluding `Position`) that differ between `a` and `b`.
pub fn diff_node_groups(a: &Node, b: &Node) -> BTreeSet<Group> {
    let mut groups = BTreeSet::new();
    for g in PROPERTY_GROUPS {
        if !node_group_eq(a, b, g) {
            groups.insert(g);
        }
    }
    groups
}

// ============================================================================
// LCS Sibling Move Detection (T10.2 §3.3)
// ============================================================================

/// Computes the Longest Common Subsequence (LCS) of two slices of `NodeId`.
fn lcs_node_ids(a: &[NodeId], b: &[NodeId]) -> BTreeSet<NodeId> {
    let n = a.len();
    let m = b.len();
    if n == 0 || m == 0 {
        return BTreeSet::new();
    }
    let mut dp = vec![0u32; (n + 1) * (m + 1)];
    let stride = m + 1;
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i * stride + j] = if a[i] == b[j] { 1 + dp[(i + 1) * stride + (j + 1)] } else { dp[(i + 1) * stride + j].max(dp[i * stride + (j + 1)]) };
        }
    }
    let mut keep = BTreeSet::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i] == b[j] {
            keep.insert(a[i]);
            i += 1;
            j += 1;
        } else if dp[(i + 1) * stride + j] >= dp[i * stride + (j + 1)] {
            i += 1;
        } else {
            j += 1;
        }
    }
    keep
}

/// Builds a map of `node_id -> (parent_or_root, index_in_parent)` for every node in `doc`.
/// Top-level pages use `NodeId(0)` as their parent id.
fn build_positions(doc: &Document) -> HashMap<NodeId, (NodeId, usize)> {
    let mut pos = HashMap::with_capacity(doc.nodes.len());
    for (idx, &page_id) in doc.pages.iter().enumerate() {
        pos.insert(page_id, (NodeId(0), idx));
    }
    for (&parent_id, node) in &doc.nodes {
        for (idx, &child_id) in node.children.iter().enumerate() {
            pos.insert(child_id, (parent_id, idx));
        }
    }
    pos
}

type NodePos = (NodeId, usize);
type NodeMove = (NodePos, NodePos);

/// Computes all moved nodes between `base` and `side`:
/// - Any node present in both whose `parent` changed.
/// - Any node present in both under the same `parent` that is not part of the LCS of shared siblings.
fn detect_moves(base: &Document, side: &Document, base_pos: &HashMap<NodeId, NodePos>, side_pos: &HashMap<NodeId, NodePos>) -> BTreeMap<NodeId, NodeMove> {
    let mut moves = BTreeMap::new();

    // 1. Check nodes whose parent changed.
    for (&id, &b_loc) in base_pos {
        if let Some(&s_loc) = side_pos.get(&id) {
            if b_loc.0 != s_loc.0 {
                moves.insert(id, (b_loc, s_loc));
            }
        }
    }

    // 2. Check sibling reorderings on parents whose children list changed.
    let mut check_sibling_list = |parent_id: NodeId, base_children: &[NodeId], side_children: &[NodeId]| {
        if base_children == side_children {
            return;
        }
        let side_set: BTreeSet<NodeId> = side_children.iter().copied().collect();
        let base_set: BTreeSet<NodeId> = base_children.iter().copied().collect();
        let common_base: Vec<NodeId> = base_children.iter().copied().filter(|id| side_set.contains(id)).collect();
        let common_side: Vec<NodeId> = side_children.iter().copied().filter(|id| base_set.contains(id)).collect();
        if common_base == common_side {
            return;
        }
        let lcs = lcs_node_ids(&common_base, &common_side);
        for id in common_base {
            if !lcs.contains(&id) {
                if let (Some(&b_loc), Some(&s_loc)) = (base_pos.get(&id), side_pos.get(&id)) {
                    debug_assert_eq!(b_loc.0, parent_id);
                    moves.insert(id, (b_loc, s_loc));
                }
            }
        }
    };

    check_sibling_list(NodeId(0), &base.pages, &side.pages);
    for (&parent_id, base_parent) in &base.nodes {
        if let Some(side_parent) = side.nodes.get(&parent_id) {
            check_sibling_list(parent_id, &base_parent.children, &side_parent.children);
        }
    }

    moves
}

// ============================================================================
// Non-node Entity Diffs (T10.2 §3.5, §8.6)
// ============================================================================

fn diff_styles_side(base: &Document, side: &Document) -> BTreeMap<String, EntityChange> {
    let mut out = BTreeMap::new();
    for (k, b_style) in &base.styles {
        match side.styles.get(k) {
            None => {
                out.insert(k.clone(), EntityChange::Removed);
            }
            Some(s_style) => {
                if b_style != s_style || base.style_meta.get(k) != side.style_meta.get(k) {
                    out.insert(k.clone(), EntityChange::Changed);
                }
            }
        }
    }
    for k in side.styles.keys() {
        if !base.styles.contains_key(k) {
            out.insert(k.clone(), EntityChange::Added);
        }
    }
    out
}

fn style_entry_eq(a: &Document, b: &Document, key: &str) -> bool {
    let sa: Option<(&Style, Option<&StyleMeta>)> = a.styles.get(key).map(|s| (s, a.style_meta.get(key)));
    let sb: Option<(&Style, Option<&StyleMeta>)> = b.styles.get(key).map(|s| (s, b.style_meta.get(key)));
    sa == sb
}

fn diff_collections_side(base: &BTreeMap<String, VariableCollection>, side: &BTreeMap<String, VariableCollection>) -> BTreeMap<String, EntityChange> {
    let mut out = BTreeMap::new();
    for (k, b_col) in base {
        match side.get(k) {
            None => {
                out.insert(k.clone(), EntityChange::Removed);
            }
            Some(s_col) if b_col != s_col => {
                out.insert(k.clone(), EntityChange::Changed);
            }
            _ => {}
        }
    }
    for k in side.keys() {
        if !base.contains_key(k) {
            out.insert(k.clone(), EntityChange::Added);
        }
    }
    out
}

fn diff_variables_side(base: &BTreeMap<String, Variable>, side: &BTreeMap<String, Variable>) -> BTreeMap<String, EntityChange> {
    let mut out = BTreeMap::new();
    for (k, b_var) in base {
        match side.get(k) {
            None => {
                out.insert(k.clone(), EntityChange::Removed);
            }
            Some(s_var) if b_var != s_var => {
                out.insert(k.clone(), EntityChange::Changed);
            }
            _ => {}
        }
    }
    for k in side.keys() {
        if !base.contains_key(k) {
            out.insert(k.clone(), EntityChange::Added);
        }
    }
    out
}

/// Checks whether two modifications to the same [`Variable`] conflict (`T10.2` §8.6).
///
/// Changing variable values in *different* modes on each side is NOT a conflict.
fn variable_changes_conflict(base: &Variable, ours: &Variable, theirs: &Variable) -> bool {
    if (base.name != ours.name && base.name != theirs.name && ours.name != theirs.name)
        || (base.collection != ours.collection && base.collection != theirs.collection && ours.collection != theirs.collection)
        || (base.kind != ours.kind && base.kind != theirs.kind && ours.kind != theirs.kind)
        || (base.description != ours.description && base.description != theirs.description && ours.description != theirs.description)
        || (base.scopes != ours.scopes && base.scopes != theirs.scopes && ours.scopes != theirs.scopes)
        || (base.hidden_from_publishing != ours.hidden_from_publishing
            && base.hidden_from_publishing != theirs.hidden_from_publishing
            && ours.hidden_from_publishing != theirs.hidden_from_publishing)
        || (base.key != ours.key && base.key != theirs.key && ours.key != theirs.key)
        || (base.code_syntax != ours.code_syntax && base.code_syntax != theirs.code_syntax && ours.code_syntax != theirs.code_syntax)
    {
        return true;
    }

    let all_modes: BTreeSet<&String> = base.values.keys().chain(ours.values.keys()).chain(theirs.values.keys()).collect();
    for mode in all_modes {
        let b_val = base.values.get(mode);
        let o_val = ours.values.get(mode);
        let t_val = theirs.values.get(mode);
        if b_val != o_val && b_val != t_val && o_val != t_val {
            return true;
        }
    }
    false
}

fn flows_by_key(flows: &[Flow]) -> BTreeMap<String, &Flow> {
    let mut map = BTreeMap::new();
    for f in flows {
        map.insert(flow_key(f), f);
    }
    map
}

fn comments_by_id(comments: &[CommentThread]) -> BTreeMap<String, &CommentThread> {
    let mut map = BTreeMap::new();
    for c in comments {
        map.insert(c.id.clone(), c);
    }
    map
}

/// Checks whether two modifications to the same [`CommentThread`] conflict (`T10.2` §3.5).
///
/// Diffs by thread metadata (`anchor`, `resolved`) and per-message `id` (`Comment::id`).
fn comment_threads_conflict(base: &CommentThread, ours: &CommentThread, theirs: &CommentThread) -> bool {
    if (base.anchor != ours.anchor && base.anchor != theirs.anchor && ours.anchor != theirs.anchor)
        || (base.resolved != ours.resolved && base.resolved != theirs.resolved && ours.resolved != theirs.resolved)
    {
        return true;
    }
    let b_msgs: BTreeMap<&str, &Comment> = base.messages.iter().map(|m| (m.id.as_str(), m)).collect();
    let o_msgs: BTreeMap<&str, &Comment> = ours.messages.iter().map(|m| (m.id.as_str(), m)).collect();
    let t_msgs: BTreeMap<&str, &Comment> = theirs.messages.iter().map(|m| (m.id.as_str(), m)).collect();
    let all_ids: BTreeSet<&str> = b_msgs.keys().chain(o_msgs.keys()).chain(t_msgs.keys()).copied().collect();
    for id in all_ids {
        let bm = b_msgs.get(id);
        let om = o_msgs.get(id);
        let tm = t_msgs.get(id);
        if bm != om && bm != tm && om != tm {
            return true;
        }
    }
    false
}

// ============================================================================
// Enclosing Instance Lookup & Subtree Helpers (T10.2 §3.4, §8.3)
// ============================================================================

fn enclosing_instance_of(doc: &Document, start: NodeId) -> Option<NodeId> {
    let mut cur = doc.nodes.get(&start)?.parent;
    let mut visited = 0usize;
    while let Some(pid) = cur {
        if visited > doc.nodes.len() {
            break;
        }
        visited += 1;
        let parent = doc.nodes.get(&pid)?;
        if matches!(parent.kind, NodeKind::Instance { .. }) {
            return Some(pid);
        }
        cur = parent.parent;
    }
    None
}

fn has_modified_or_added_descendant_in(root: NodeId, side_doc: &Document, side_changes: &BTreeMap<NodeId, NodeChange>) -> bool {
    let mut stack = vec![root];
    let mut visited = BTreeSet::new();
    while let Some(curr) = stack.pop() {
        if !visited.insert(curr) {
            continue;
        }
        if curr != root {
            if let Some(ch) = side_changes.get(&curr) {
                if !ch.is_removed() {
                    return true;
                }
            }
        }
        if let Some(node) = side_doc.nodes.get(&curr) {
            for &child in &node.children {
                stack.push(child);
            }
        }
    }
    false
}

fn action_references_target(action: &crate::Action, target: NodeId) -> bool {
    match action {
        crate::Action::Navigate { dest, .. }
        | crate::Action::OpenOverlay { dest, .. }
        | crate::Action::SwapOverlay { dest }
        | crate::Action::ScrollTo { dest, .. }
        | crate::Action::ChangeTo { dest }
        | crate::Action::MediaControl { dest, .. } => *dest == target,
        crate::Action::Conditional { branches, otherwise } => {
            branches.iter().any(|(_, actions)| actions.iter().any(|a| action_references_target(a, target)))
                || otherwise.iter().any(|a| action_references_target(a, target))
        }
        crate::Action::SetVariable { .. }
        | crate::Action::SetVariableMode { .. }
        | crate::Action::CloseOverlay
        | crate::Action::Back
        | crate::Action::OpenUrl { .. } => false,
    }
}

fn prop_value_references_target(val: &crate::PropValue, target: NodeId) -> bool {
    match val {
        crate::PropValue::Instance(id) => *id == target,
        crate::PropValue::Slot(ids) => ids.contains(&target),
        crate::PropValue::Bool(_) | crate::PropValue::Text(_) | crate::PropValue::Variant(_) => false,
    }
}

fn node_references_target(node: &Node, target: NodeId) -> bool {
    match &node.kind {
        NodeKind::Instance { component: Some(comp_id), .. } if *comp_id == target => return true,
        NodeKind::Connector(conn) if conn.start.node == Some(target) || conn.end.node == Some(target) => return true,
        _ => {}
    }
    if node.component.source == Some(target) {
        return true;
    }
    for prop in &node.component.props {
        if prop_value_references_target(&prop.default, target) {
            return true;
        }
        if let crate::PropKind::InstanceSwap { preferred } = &prop.kind {
            if preferred.contains(&target) {
                return true;
            }
        }
    }
    if node.component.values.values().any(|v| prop_value_references_target(v, target)) {
        return true;
    }
    if node.component.slot_configs.values().any(|cfg| cfg.preferred.contains(&target)) {
        return true;
    }
    if node.prototype.fixed_children.contains(&target) {
        return true;
    }
    if node.prototype.reactions.iter().any(|r| r.actions.iter().any(|a| action_references_target(a, target))) {
        return true;
    }
    false
}

fn has_referencing_node_outside_subtree(
    root: NodeId,
    side_doc: &Document,
    other_side_changes: &BTreeMap<NodeId, NodeChange>,
    side_changes: &BTreeMap<NodeId, NodeChange>,
) -> bool {
    let mut subtree = BTreeSet::new();
    let mut stack = vec![root];
    while let Some(curr) = stack.pop() {
        if !subtree.insert(curr) {
            continue;
        }
        if let Some(node) = side_doc.nodes.get(&curr) {
            for &child in &node.children {
                stack.push(child);
            }
        }
    }
    for (&nid, node) in &side_doc.nodes {
        if subtree.contains(&nid) {
            continue;
        }
        if matches!(other_side_changes.get(&nid), Some(NodeChange::Removed)) && !side_changes.contains_key(&nid) {
            continue;
        }
        if node_references_target(node, root) {
            return true;
        }
    }
    false
}

fn paints_reference_image(paints: &[crate::Paint], hash: &str) -> bool {
    paints.iter().any(|p| matches!(&p.kind, crate::PaintKind::Image { image_ref, .. } if image_ref == hash))
}

fn text_references_image(tp: &TextProps, hash: &str) -> bool {
    paints_reference_image(&tp.style.fills, hash) || tp.runs.iter().any(|r| paints_reference_image(&r.style.fills, hash))
}

fn effective_side_references_image(
    hash: &str,
    primary: &Document,
    primary_nodes: &BTreeMap<NodeId, NodeChange>,
    other: &Document,
    other_nodes: &BTreeMap<NodeId, NodeChange>,
) -> bool {
    for (&nid, p_node) in &primary.nodes {
        if matches!(other_nodes.get(&nid), Some(NodeChange::Removed)) && !primary_nodes.contains_key(&nid) {
            continue;
        }
        let p_groups = primary_nodes.get(&nid).and_then(NodeChange::groups);
        let o_groups = other_nodes.get(&nid).and_then(NodeChange::groups);
        let o_node = other.nodes.get(&nid);

        let fills_ref = if o_groups.is_some_and(|g| g.contains(&Group::Fills)) && !p_groups.is_some_and(|g| g.contains(&Group::Fills)) {
            o_node.is_some_and(|n| paints_reference_image(&n.fills, hash))
        } else {
            paints_reference_image(&p_node.fills, hash)
        };
        if fills_ref {
            return true;
        }

        let stroke_ref = if o_groups.is_some_and(|g| g.contains(&Group::Stroke)) && !p_groups.is_some_and(|g| g.contains(&Group::Stroke)) {
            o_node.is_some_and(|n| paints_reference_image(&n.stroke.paints, hash))
        } else {
            paints_reference_image(&p_node.stroke.paints, hash)
        };
        if stroke_ref {
            return true;
        }

        let text_ref = if o_groups.is_some_and(|g| g.contains(&Group::TextStyle)) && !p_groups.is_some_and(|g| g.contains(&Group::TextStyle)) {
            o_node.and_then(|n| n.kind.any_text()).is_some_and(|tp| text_references_image(tp, hash))
        } else {
            p_node.kind.any_text().is_some_and(|tp| text_references_image(tp, hash))
        };
        if text_ref {
            return true;
        }
    }
    false
}

// ============================================================================
// Three-Way Diff Entrypoint (T10.2 §3.1)
// ============================================================================

/// Computes the three-way diff between `base`, `ours` (main), and `theirs` (branch).
pub fn diff3(base: &Document, ours: &Document, theirs: &Document) -> Diff3 {
    let started = Instant::now();

    // 1. Compute per-node 64-bit property hashes in parallel across the three documents.
    let (base_hashes, ours_hashes, theirs_hashes) = std::thread::scope(|s| {
        let h_base = s.spawn(|| compute_doc_node_hashes(base));
        let h_ours = s.spawn(|| compute_doc_node_hashes(ours));
        let h_theirs = s.spawn(|| compute_doc_node_hashes(theirs));
        (
            h_base.join().expect("base hash worker panicked"),
            h_ours.join().expect("ours hash worker panicked"),
            h_theirs.join().expect("theirs hash worker panicked"),
        )
    });

    // 2. Build parent/index tables and LCS sibling moves for both sides.
    let base_pos = build_positions(base);
    let ours_pos = build_positions(ours);
    let theirs_pos = build_positions(theirs);

    let ours_moves = detect_moves(base, ours, &base_pos, &ours_pos);
    let theirs_moves = detect_moves(base, theirs, &base_pos, &theirs_pos);

    let mut ours_nodes: BTreeMap<NodeId, NodeChange> = BTreeMap::new();
    let mut theirs_nodes: BTreeMap<NodeId, NodeChange> = BTreeMap::new();

    // 3. Classify every node present in `base` against `ours` and `theirs`.
    for (&id, base_node) in &base.nodes {
        let b_hash = base_hashes.get(&id).copied().unwrap_or(0);

        // Classify on `ours`
        match ours.nodes.get(&id) {
            None => {
                ours_nodes.insert(id, NodeChange::Removed);
            }
            Some(ours_node) => {
                let o_hash = ours_hashes.get(&id).copied().unwrap_or(0);
                let groups = if b_hash == o_hash { BTreeSet::new() } else { diff_node_groups(base_node, ours_node) };
                let moved = ours_moves.get(&id).copied();
                match (moved, groups.is_empty()) {
                    (Some((from, to)), true) => {
                        ours_nodes.insert(id, NodeChange::Moved { from, to });
                    }
                    (Some((from, to)), false) => {
                        ours_nodes.insert(id, NodeChange::MovedAndChanged { from, to, groups });
                    }
                    (None, false) => {
                        ours_nodes.insert(id, NodeChange::Changed { groups });
                    }
                    (None, true) => {}
                }
            }
        }

        // Classify on `theirs`
        match theirs.nodes.get(&id) {
            None => {
                theirs_nodes.insert(id, NodeChange::Removed);
            }
            Some(theirs_node) => {
                let t_hash = theirs_hashes.get(&id).copied().unwrap_or(0);
                let groups = if b_hash == t_hash { BTreeSet::new() } else { diff_node_groups(base_node, theirs_node) };
                let moved = theirs_moves.get(&id).copied();
                match (moved, groups.is_empty()) {
                    (Some((from, to)), true) => {
                        theirs_nodes.insert(id, NodeChange::Moved { from, to });
                    }
                    (Some((from, to)), false) => {
                        theirs_nodes.insert(id, NodeChange::MovedAndChanged { from, to, groups });
                    }
                    (None, false) => {
                        theirs_nodes.insert(id, NodeChange::Changed { groups });
                    }
                    (None, true) => {}
                }
            }
        }
    }

    // 4. Classify nodes added on `ours` and `theirs`, and detect `IdCollision`s (§3.4).
    let mut id_collisions = Vec::new();
    for &id in ours.nodes.keys() {
        if !base.nodes.contains_key(&id) {
            ours_nodes.insert(id, NodeChange::Added);
            if theirs.nodes.contains_key(&id) {
                id_collisions.push(id);
            }
        }
    }
    for &id in theirs.nodes.keys() {
        if !base.nodes.contains_key(&id) {
            theirs_nodes.insert(id, NodeChange::Added);
        }
    }

    // 5. Detect per-layer conflicts (§3.4, §8).
    let mut conflicts = Vec::new();
    let mut instance_conflicts: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();

    // Collect parents where BOTH sides reordered shared children differently (§8.4).
    let mut conflicting_reordered_parents: BTreeSet<NodeId> = BTreeSet::new();
    let check_parent_reorder_conflict = |parent_id: NodeId, b_list: &[NodeId], o_list: &[NodeId], t_list: &[NodeId]| -> bool {
        let o_any_moved = b_list.iter().any(|id| ours_moves.get(id).is_some_and(|&(from, to)| from.0 == parent_id && to.0 == parent_id));
        let t_any_moved = b_list.iter().any(|id| theirs_moves.get(id).is_some_and(|&(from, to)| from.0 == parent_id && to.0 == parent_id));
        if !o_any_moved || !t_any_moved {
            return false;
        }
        let shared: BTreeSet<NodeId> = b_list.iter().copied().filter(|id| o_list.contains(id) && t_list.contains(id)).collect();
        let o_order: Vec<NodeId> = o_list.iter().copied().filter(|id| shared.contains(id)).collect();
        let t_order: Vec<NodeId> = t_list.iter().copied().filter(|id| shared.contains(id)).collect();
        o_order != t_order
    };
    if check_parent_reorder_conflict(NodeId(0), &base.pages, &ours.pages, &theirs.pages) {
        conflicting_reordered_parents.insert(NodeId(0));
    }
    for (&pid, b_node) in &base.nodes {
        if let (Some(o_node), Some(t_node)) = (ours.nodes.get(&pid), theirs.nodes.get(&pid)) {
            if check_parent_reorder_conflict(pid, &b_node.children, &o_node.children, &t_node.children) {
                conflicting_reordered_parents.insert(pid);
            }
        }
    }

    for &id in base.nodes.keys() {
        let o_change = ours_nodes.get(&id);
        let t_change = theirs_nodes.get(&id);

        match (o_change, t_change) {
            (Some(NodeChange::Removed), Some(tc)) if !tc.is_removed() => {
                let mut groups = tc.groups().cloned().unwrap_or_default();
                if tc.is_moved() {
                    groups.insert(Group::Position);
                }
                conflicts.push(Conflict { node: id, groups, kind: ConflictKind::DeleteModify });
            }
            (Some(NodeChange::Removed), None) => {
                if has_modified_or_added_descendant_in(id, theirs, &theirs_nodes) {
                    conflicts.push(Conflict { node: id, groups: BTreeSet::new(), kind: ConflictKind::DeleteModify });
                }
            }
            (Some(oc), Some(NodeChange::Removed)) if !oc.is_removed() => {
                let mut groups = oc.groups().cloned().unwrap_or_default();
                if oc.is_moved() {
                    groups.insert(Group::Position);
                }
                conflicts.push(Conflict { node: id, groups, kind: ConflictKind::DeleteModify });
            }
            (None, Some(NodeChange::Removed)) => {
                if has_modified_or_added_descendant_in(id, ours, &ours_nodes) || has_referencing_node_outside_subtree(id, ours, &theirs_nodes, &ours_nodes) {
                    conflicts.push(Conflict { node: id, groups: BTreeSet::new(), kind: ConflictKind::DeleteModify });
                }
            }
            (Some(oc), Some(tc)) if !oc.is_removed() && !tc.is_removed() => {
                let (Some(ours_node), Some(theirs_node)) = (ours.nodes.get(&id), theirs.nodes.get(&id)) else {
                    continue;
                };
                let mut conflict_groups = BTreeSet::new();

                if let (Some(og), Some(tg)) = (oc.groups(), tc.groups()) {
                    for &g in og.intersection(tg) {
                        if !node_group_eq(ours_node, theirs_node, g) {
                            conflict_groups.insert(g);
                        }
                    }
                }

                // Check position conflicts:
                // (a) both moved `id` to different parents or different indices, OR
                // (b) `id` moved as part of a conflicting sibling reorder on the same parent.
                match (ours_moves.get(&id), theirs_moves.get(&id)) {
                    (Some(&(_, o_to)), Some(&(_, t_to))) if o_to != t_to => {
                        conflict_groups.insert(Group::Position);
                    }
                    (Some(&(o_from, o_to)), None) if o_from.0 == o_to.0 && conflicting_reordered_parents.contains(&o_from.0) => {
                        conflict_groups.insert(Group::Position);
                    }
                    (None, Some(&(t_from, t_to))) if t_from.0 == t_to.0 && conflicting_reordered_parents.contains(&t_from.0) => {
                        conflict_groups.insert(Group::Position);
                    }
                    _ => {}
                }

                if !conflict_groups.is_empty() {
                    conflicts.push(Conflict { node: id, groups: conflict_groups, kind: ConflictKind::ModifyModify });
                }
            }
            (Some(_), None) => {
                if let Some(&(o_from, o_to)) = ours_moves.get(&id) {
                    if o_from.0 == o_to.0 && conflicting_reordered_parents.contains(&o_from.0) {
                        let mut conflict_groups = BTreeSet::new();
                        conflict_groups.insert(Group::Position);
                        conflicts.push(Conflict { node: id, groups: conflict_groups, kind: ConflictKind::ModifyModify });
                    }
                }
            }
            (None, Some(_)) => {
                if let Some(&(t_from, t_to)) = theirs_moves.get(&id) {
                    if t_from.0 == t_to.0 && conflicting_reordered_parents.contains(&t_from.0) {
                        let mut conflict_groups = BTreeSet::new();
                        conflict_groups.insert(Group::Position);
                        conflicts.push(Conflict { node: id, groups: conflict_groups, kind: ConflictKind::ModifyModify });
                    }
                }
            }
            _ => {}
        }
    }

    // Also check if `theirs` added an Instance pointing at a Component that `ours` removed (`T10.3` §8.2).
    // (When `theirs` removes a component that `ours` still references — via an existing or newly-added
    // node — `has_referencing_node_outside_subtree` above already records `DeleteModify` on the component.)
    for (&id, change) in &theirs_nodes {
        if change.is_added() {
            if let Some(NodeKind::Instance { component: Some(comp_id), .. }) = theirs.nodes.get(&id).map(|n| &n.kind) {
                if matches!(ours_nodes.get(comp_id), Some(NodeChange::Removed)) {
                    let mut groups = BTreeSet::new();
                    groups.insert(Group::Component);
                    conflicts.push(Conflict { node: id, groups, kind: ConflictKind::DependsOnRemoved });
                }
            }
        }
    }

    // Group mirror conflicts under their enclosing instance (`T10.2` §8.3).
    for c in &conflicts {
        if let Some(inst_id) =
            enclosing_instance_of(base, c.node).or_else(|| enclosing_instance_of(ours, c.node)).or_else(|| enclosing_instance_of(theirs, c.node))
        {
            instance_conflicts.entry(inst_id).or_default().push(c.node);
        }
    }

    // 6. Diff non-node entities (`styles`, `variables`, `collections`, `flows`, `comments`, `guides`, `images`).
    let ours_styles = diff_styles_side(base, ours);
    let theirs_styles = diff_styles_side(base, theirs);
    let ours_variables = diff_variables_side(&base.variables, &ours.variables);
    let theirs_variables = diff_variables_side(&base.variables, &theirs.variables);
    let ours_collections = diff_collections_side(&base.variable_collections, &ours.variable_collections);
    let theirs_collections = diff_collections_side(&base.variable_collections, &theirs.variable_collections);

    let base_flows = flows_by_key(&base.flows);
    let ours_flows_map = flows_by_key(&ours.flows);
    let theirs_flows_map = flows_by_key(&theirs.flows);
    let diff_flows = |side_map: &BTreeMap<String, &Flow>| -> BTreeMap<String, EntityChange> {
        let mut out = BTreeMap::new();
        for (k, bf) in &base_flows {
            match side_map.get(k) {
                None => {
                    out.insert(k.clone(), EntityChange::Removed);
                }
                Some(sf) if *bf != *sf => {
                    out.insert(k.clone(), EntityChange::Changed);
                }
                _ => {}
            }
        }
        for k in side_map.keys() {
            if !base_flows.contains_key(k) {
                out.insert(k.clone(), EntityChange::Added);
            }
        }
        out
    };
    let ours_flows = diff_flows(&ours_flows_map);
    let theirs_flows = diff_flows(&theirs_flows_map);

    let base_comments = comments_by_id(&base.comments);
    let ours_comments_map = comments_by_id(&ours.comments);
    let theirs_comments_map = comments_by_id(&theirs.comments);
    let diff_comments = |side_map: &BTreeMap<String, &CommentThread>| -> BTreeMap<String, EntityChange> {
        let mut out = BTreeMap::new();
        for (k, bc) in &base_comments {
            match side_map.get(k) {
                None => {
                    out.insert(k.clone(), EntityChange::Removed);
                }
                Some(sc) if *bc != *sc => {
                    out.insert(k.clone(), EntityChange::Changed);
                }
                _ => {}
            }
        }
        for k in side_map.keys() {
            if !base_comments.contains_key(k) {
                out.insert(k.clone(), EntityChange::Added);
            }
        }
        out
    };
    let ours_comments = diff_comments(&ours_comments_map);
    let theirs_comments = diff_comments(&theirs_comments_map);

    let diff_guides = |side_guides: &BTreeMap<NodeId, Vec<crate::Guide>>| -> BTreeMap<NodeId, EntityChange> {
        let mut out = BTreeMap::new();
        for (&page_id, bg) in &base.guides {
            match side_guides.get(&page_id) {
                None => {
                    out.insert(page_id, EntityChange::Removed);
                }
                Some(sg) if bg != sg => {
                    out.insert(page_id, EntityChange::Changed);
                }
                _ => {}
            }
        }
        for &page_id in side_guides.keys() {
            if !base.guides.contains_key(&page_id) {
                out.insert(page_id, EntityChange::Added);
            }
        }
        out
    };
    let ours_guides = diff_guides(&ours.guides);
    let theirs_guides = diff_guides(&theirs.guides);

    let diff_images = |side_images: &BTreeMap<String, Vec<u8>>| -> BTreeMap<String, EntityChange> {
        let mut out = BTreeMap::new();
        for (k, bi) in &base.images {
            match side_images.get(k) {
                None => {
                    out.insert(k.clone(), EntityChange::Removed);
                }
                Some(si) if bi != si => {
                    out.insert(k.clone(), EntityChange::Changed);
                }
                _ => {}
            }
        }
        for k in side_images.keys() {
            if !base.images.contains_key(k) {
                out.insert(k.clone(), EntityChange::Added);
            }
        }
        out
    };
    let ours_images = diff_images(&ours.images);
    let theirs_images = diff_images(&theirs.images);

    let mut entity_conflicts = Vec::new();

    // Style conflicts
    for (k, &oc) in &ours_styles {
        if let Some(&tc) = theirs_styles.get(k) {
            match (oc, tc) {
                (EntityChange::Removed, EntityChange::Changed) | (EntityChange::Changed, EntityChange::Removed) => {
                    entity_conflicts.push(EntityConflict { key: EntityKey::Style(k.clone()), kind: ConflictKind::DeleteModify });
                }
                (EntityChange::Changed, EntityChange::Changed) | (EntityChange::Added, EntityChange::Added) => {
                    if !style_entry_eq(ours, theirs, k) {
                        entity_conflicts.push(EntityConflict { key: EntityKey::Style(k.clone()), kind: ConflictKind::ModifyModify });
                    }
                }
                _ => {}
            }
        }
    }

    // Variable conflicts (respecting Edge Case 6: changes in different modes do NOT conflict)
    for (k, &oc) in &ours_variables {
        if let Some(&tc) = theirs_variables.get(k) {
            match (oc, tc) {
                (EntityChange::Removed, EntityChange::Changed) | (EntityChange::Changed, EntityChange::Removed) => {
                    entity_conflicts.push(EntityConflict { key: EntityKey::Variable(k.clone()), kind: ConflictKind::DeleteModify });
                }
                (EntityChange::Changed, EntityChange::Changed) => {
                    if let (Some(bv), Some(ov), Some(tv)) = (base.variables.get(k), ours.variables.get(k), theirs.variables.get(k)) {
                        if variable_changes_conflict(bv, ov, tv) {
                            entity_conflicts.push(EntityConflict { key: EntityKey::Variable(k.clone()), kind: ConflictKind::ModifyModify });
                        }
                    }
                }
                (EntityChange::Added, EntityChange::Added) => {
                    if ours.variables.get(k) != theirs.variables.get(k) {
                        entity_conflicts.push(EntityConflict { key: EntityKey::Variable(k.clone()), kind: ConflictKind::ModifyModify });
                    }
                }
                _ => {}
            }
        }
    }

    // Collection conflicts
    for (k, &oc) in &ours_collections {
        if let Some(&tc) = theirs_collections.get(k) {
            match (oc, tc) {
                (EntityChange::Removed, EntityChange::Changed) | (EntityChange::Changed, EntityChange::Removed) => {
                    entity_conflicts.push(EntityConflict { key: EntityKey::Collection(k.clone()), kind: ConflictKind::DeleteModify });
                }
                (EntityChange::Changed, EntityChange::Changed) | (EntityChange::Added, EntityChange::Added) => {
                    if ours.variable_collections.get(k) != theirs.variable_collections.get(k) {
                        entity_conflicts.push(EntityConflict { key: EntityKey::Collection(k.clone()), kind: ConflictKind::ModifyModify });
                    }
                }
                _ => {}
            }
        }
    }

    // Flow conflicts
    for (k, &oc) in &ours_flows {
        if let Some(&tc) = theirs_flows.get(k) {
            match (oc, tc) {
                (EntityChange::Removed, EntityChange::Changed) | (EntityChange::Changed, EntityChange::Removed) => {
                    entity_conflicts.push(EntityConflict { key: EntityKey::Flow(k.clone()), kind: ConflictKind::DeleteModify });
                }
                (EntityChange::Changed, EntityChange::Changed) | (EntityChange::Added, EntityChange::Added) => {
                    if ours_flows_map.get(k) != theirs_flows_map.get(k) {
                        entity_conflicts.push(EntityConflict { key: EntityKey::Flow(k.clone()), kind: ConflictKind::ModifyModify });
                    }
                }
                _ => {}
            }
        }
    }

    // Comment thread conflicts
    for (k, &oc) in &ours_comments {
        if let Some(&tc) = theirs_comments.get(k) {
            match (oc, tc) {
                (EntityChange::Removed, EntityChange::Changed) | (EntityChange::Changed, EntityChange::Removed) => {
                    entity_conflicts.push(EntityConflict { key: EntityKey::Comment(k.clone()), kind: ConflictKind::DeleteModify });
                }
                (EntityChange::Changed, EntityChange::Changed) => {
                    if let (Some(bc), Some(oc_t), Some(tc_t)) = (base_comments.get(k), ours_comments_map.get(k), theirs_comments_map.get(k)) {
                        if comment_threads_conflict(bc, oc_t, tc_t) {
                            entity_conflicts.push(EntityConflict { key: EntityKey::Comment(k.clone()), kind: ConflictKind::ModifyModify });
                        }
                    }
                }
                (EntityChange::Added, EntityChange::Added) => {
                    if ours_comments_map.get(k) != theirs_comments_map.get(k) {
                        entity_conflicts.push(EntityConflict { key: EntityKey::Comment(k.clone()), kind: ConflictKind::ModifyModify });
                    }
                }
                _ => {}
            }
        }
    }

    // Guide conflicts (per shared page in `base.nodes`)
    for (&page_id, &oc) in &ours_guides {
        if !base.nodes.contains_key(&page_id) {
            continue;
        }
        if let Some(&tc) = theirs_guides.get(&page_id) {
            match (oc, tc) {
                (EntityChange::Removed, EntityChange::Changed) | (EntityChange::Changed, EntityChange::Removed) => {
                    if ours.nodes.contains_key(&page_id) && theirs.nodes.contains_key(&page_id) {
                        entity_conflicts.push(EntityConflict { key: EntityKey::Guides(page_id), kind: ConflictKind::DeleteModify });
                    }
                }
                (EntityChange::Changed, EntityChange::Changed) | (EntityChange::Added, EntityChange::Added) => {
                    if ours.guides.get(&page_id) != theirs.guides.get(&page_id) {
                        entity_conflicts.push(EntityConflict { key: EntityKey::Guides(page_id), kind: ConflictKind::ModifyModify });
                    }
                }
                _ => {}
            }
        }
    }

    // Image conflicts (including when one side removes an image hash still referenced on the other side)
    let all_image_keys: BTreeSet<&String> = ours_images.keys().chain(theirs_images.keys()).collect();
    for k in all_image_keys {
        match (ours_images.get(k).copied(), theirs_images.get(k).copied()) {
            (Some(EntityChange::Removed), Some(EntityChange::Changed)) | (Some(EntityChange::Changed), Some(EntityChange::Removed)) => {
                entity_conflicts.push(EntityConflict { key: EntityKey::Image(k.clone()), kind: ConflictKind::DeleteModify });
            }
            (Some(EntityChange::Changed), Some(EntityChange::Changed)) | (Some(EntityChange::Added), Some(EntityChange::Added)) => {
                if ours.images.get(k) != theirs.images.get(k) {
                    entity_conflicts.push(EntityConflict { key: EntityKey::Image(k.clone()), kind: ConflictKind::ModifyModify });
                }
            }
            (None, Some(EntityChange::Removed)) => {
                if effective_side_references_image(k, ours, &ours_nodes, theirs, &theirs_nodes) {
                    entity_conflicts.push(EntityConflict { key: EntityKey::Image(k.clone()), kind: ConflictKind::DeleteModify });
                }
            }
            (Some(EntityChange::Removed), None) => {
                if effective_side_references_image(k, theirs, &theirs_nodes, ours, &ours_nodes) {
                    entity_conflicts.push(EntityConflict { key: EntityKey::Image(k.clone()), kind: ConflictKind::DeleteModify });
                }
            }
            _ => {}
        }
    }

    let scanned = base.nodes.len().max(ours.nodes.len()).max(theirs.nodes.len());
    let changed_ours = ours_nodes.len() + ours_styles.len() + ours_variables.len() + ours_collections.len() + ours_flows.len() + ours_comments.len();
    let changed_theirs =
        theirs_nodes.len() + theirs_styles.len() + theirs_variables.len() + theirs_collections.len() + theirs_flows.len() + theirs_comments.len();
    let total_conflicts = conflicts.len() + entity_conflicts.len();
    let elapsed_ms = started.elapsed().as_millis() as u64;

    Diff3 {
        ours: SideChanges {
            nodes: ours_nodes,
            styles: ours_styles,
            variables: ours_variables,
            collections: ours_collections,
            flows: ours_flows,
            comments: theirs_or_ours(ours_comments),
            guides: ours_guides,
            images: ours_images,
        },
        theirs: SideChanges {
            nodes: theirs_nodes,
            styles: theirs_styles,
            variables: theirs_variables,
            collections: theirs_collections,
            flows: theirs_flows,
            comments: theirs_or_ours(theirs_comments),
            guides: theirs_guides,
            images: theirs_images,
        },
        conflicts,
        entity_conflicts,
        id_collisions,
        stats: DiffStats { scanned, changed_ours, changed_theirs, conflicts: total_conflicts, elapsed_ms, instance_conflicts },
    }
}

#[inline]
fn theirs_or_ours<T>(v: T) -> T {
    v
}
