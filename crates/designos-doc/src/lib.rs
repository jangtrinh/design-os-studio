//! designos document model.
//!
//! This crate is the contract shared by every other crate: importers produce a
//! [`Document`], the renderer draws it, the app edits it through [`Command`]s.
//! Coordinates are in Figma units (1 unit = 1 CSS px at 100% zoom), y down.

#![allow(
    clippy::field_reassign_with_default,
    clippy::needless_range_loop,
    clippy::question_mark,
    clippy::manual_is_multiple_of,
    clippy::collapsible_if,
    clippy::match_like_matches_macro,
    clippy::needless_borrow,
    clippy::redundant_guards
)]

mod board;
pub mod branch_diff;
#[cfg(test)]
mod branch_diff_tests;
pub mod branch_merge;
pub mod changelog_features;
mod comments;
#[cfg(test)]
mod component_cycle_tests;
mod components;
mod dev;
pub mod diff;
pub mod find;
pub mod finite_json;
#[cfg(test)]
mod finite_json_tests;
mod geom;
mod history;
mod layout_apply;
pub mod layout_constraints;
pub mod layout_solver;
mod layout_solver_grid;
mod layout_solver_grid_placement;
mod layout_solver_types;
mod library;
mod library_ops;
pub mod lint;
pub mod live_patch;
mod load_repair;
#[cfg(test)]
mod load_repair_tests;
mod node;
pub mod openpencil;
mod paint;
mod prototype;
pub mod query;
mod renumber_ids;
#[cfg(test)]
mod renumber_ids_tests;
mod settings;
mod silhouette;
#[cfg(test)]
mod slot_model_tests;
pub mod spec;
pub mod spec_bind;
#[cfg(test)]
mod spec_bind_reference_forms_tests;
#[cfg(test)]
mod spec_bind_rollback_label_tests;
mod spec_data;
pub use spec_data::SpecDataSnapshot;
pub mod spec_instance;
mod styles_ops;
mod text_fills;
pub mod text_measure;
mod textmodel;
mod variables;
mod variables_apply;
mod variables_edit;
pub mod vector;
mod vector_fit;
mod writer_command;

pub use board::*;
pub use branch_diff::{diff3, Conflict, ConflictKind, Diff3, DiffStats, EntityChange, EntityConflict, EntityKey, Group, SideChanges};
pub use branch_merge::{
    merge, merge_branch, validate_merged_references, ConflictSide, DanglingReference, MergeError, MergePlan, MergeReport, Resolution, Side, UnresolvedItem,
};
pub use changelog_features::*;
pub use comments::*;
pub use components::*;
pub use dev::*;
pub use diff::*;
pub use geom::*;
pub use history::*;
pub use layout_apply::RELAYOUT_NOTICE_PREFIX;
pub use layout_constraints::*;
pub use library::*;
pub use library_ops::*;
pub use load_repair::*;
pub use node::*;
pub use openpencil::*;
pub use paint::*;
pub use prototype::*;
pub use settings::*;
pub use silhouette::{silhouette_source, FilllessKnockout, SilhouetteMode, SilhouetteSource, FILLLESS_KNOCKOUT};
pub use spec::*;
pub use spec_bind::*;
pub use spec_instance::*;
pub use styles_ops::*;
pub use text_fills::set_text_fills_everywhere;
pub use textmodel::*;
pub use variables::*;
pub use variables_apply::*;
pub use variables_edit::*;
pub use vector::{HandleMirror, VectorNetwork};
pub use vector_fit::{fit_polyline, Anchor};
pub use writer_command::*;

/// Shared spec test fixtures (`tokens_doc`, `button_library`, `card_with_slots`).
pub mod fixtures {
    pub use crate::spec_bind::fixtures::*;
    pub use crate::spec_instance::fixtures::*;
}

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Stable node identifier. Imported Figma ids ("12:34") are kept in
/// [`Node::source_id`]; `NodeId` is our own dense key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

#[derive(Debug, thiserror::Error)]
pub enum DocError {
    #[error("node {0:?} not found")]
    NotFound(NodeId),
    #[error("cannot move node {0:?} into its own subtree")]
    Cycle(NodeId),
    #[error("node {0:?} cannot have children")]
    NotAContainer(NodeId),
}

const DIRTY_RING_CAP: usize = 512;
const REMOVED_RING_CAP: usize = 64;

static NEXT_DOC_RUNTIME_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[derive(Clone, Debug)]
struct DirtyLog {
    runtime_id: u64,
    epoch: u64,
    geom_full_epoch: u64,
    tree_full_epoch: u64,
    evicted_epoch: u64,
    write_pos: u64,
    ring: [(u64, NodeId); DIRTY_RING_CAP],
    removed_evicted_epoch: u64,
    removed_write_pos: u64,
    removed_ring: [(u64, NodeId); REMOVED_RING_CAP],
}

impl Default for DirtyLog {
    fn default() -> Self {
        Self {
            runtime_id: NEXT_DOC_RUNTIME_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            epoch: 1,
            geom_full_epoch: 1,
            tree_full_epoch: 1,
            evicted_epoch: 0,
            write_pos: 0,
            ring: [(0, NodeId(0)); DIRTY_RING_CAP],
            removed_evicted_epoch: 0,
            removed_write_pos: 0,
            removed_ring: [(0, NodeId(0)); REMOVED_RING_CAP],
        }
    }
}

/// A whole file: pages at the root, each page a tree of nodes.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Document {
    pub name: String,
    /// Design file or board (whiteboard) file; decides the toolbar and panels.
    #[serde(default)]
    pub kind: DocumentKind,
    pub nodes: BTreeMap<NodeId, Node>,
    /// Page node ids in display order. Each page is a node with [`NodeKind::Page`].
    pub pages: Vec<NodeId>,
    pub styles: BTreeMap<String, Style>,
    /// Display metadata for entries of `styles`, same keys.
    #[serde(default)]
    pub style_meta: BTreeMap<String, StyleMeta>,
    /// Prototype flows (starting points), in display order.
    #[serde(default)]
    pub flows: Vec<Flow>,
    /// Variables (design tokens) by id.
    #[serde(default)]
    pub variables: BTreeMap<String, Variable>,
    /// Variable collections (with their modes) by id.
    #[serde(default)]
    pub variable_collections: BTreeMap<String, VariableCollection>,
    /// Ruler guides per page.
    #[serde(default)]
    pub guides: BTreeMap<NodeId, Vec<Guide>>,
    /// Device used when presenting the prototype.
    #[serde(default)]
    pub prototype_device: PrototypeDevice,
    /// Comment threads on the canvas.
    #[serde(default)]
    pub comments: Vec<CommentThread>,
    /// Published / enabled libraries and local copies of library assets.
    #[serde(default)]
    pub library: LibraryState,
    /// Hidden page holding copies of library components; not in `pages`.
    #[serde(default)]
    pub library_page: Option<NodeId>,
    /// Check designs rule toggles, contrast level, and ignored findings.
    #[serde(default, skip_serializing_if = "CheckDesignsSettings::is_empty")]
    pub check_designs: CheckDesignsSettings,
    /// Embedded images keyed by content hash (Figma `imageRef`).
    #[serde(default, with = "serde_images", skip_serializing_if = "BTreeMap::is_empty")]
    pub images: BTreeMap<String, Vec<u8>>,
    /// Active agent work-in-progress flags keyed by root frame [`NodeId`] (`E15-7` / `#592`).
    ///
    /// Kept outside [`Node`] so setting or clearing a flag never creates a [`Command::Update`]
    /// undo step on [`History`].
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub in_progress: BTreeMap<NodeId, InProgress>,
    next_id: u64,
    /// Where the exhausted-counter fallback resumes scanning, so one id is never handed out twice
    /// before its node is inserted. Runtime only.
    #[serde(skip)]
    gap_cursor: u64,
    #[serde(skip)]
    dirty: DirtyLog,
    /// Subtrees whose size was changed by an auto-layout parent during [`Self::relayout_subtree`]
    /// after [`MAX_RELAYOUT_RESOLVES`] or [`MAX_RELAYOUT_RESOLVE_DEPTH`] was reached.
    #[serde(skip)]
    pub(crate) pending_relayout: Vec<NodeId>,
    /// Count of layout writes made to this document outside any history step; see [`Self::layout_epoch`].
    #[serde(skip)]
    layout_epoch: u64,
    /// Optional on-demand hydrator that expands deferred component instances on inactive pages when switched to or exported.
    #[serde(skip)]
    pub page_hydrator: Option<std::sync::Arc<std::sync::Mutex<dyn PageHydrator>>>,
}

/// Default staleness threshold for [`InProgress`] flags in minutes (`agent.in_progress_stale_minutes`, E15-7 §3.3).
pub const IN_PROGRESS_DEFAULT_STALE_MINUTES: u32 = 15;

/// Minimum allowed value for `agent.in_progress_stale_minutes` (`1` minute, E15-7 §3.3).
pub const IN_PROGRESS_MIN_STALE_MINUTES: u32 = 1;

/// Maximum allowed value for `agent.in_progress_stale_minutes` (`1440` minutes / 24 hours, E15-7 §3.3).
pub const IN_PROGRESS_MAX_STALE_MINUTES: u32 = 1440;

/// Default staleness threshold for [`InProgress`] flags in seconds (`15` minutes, E15-7 §3.3).
pub const IN_PROGRESS_DEFAULT_STALE_S: u32 = IN_PROGRESS_DEFAULT_STALE_MINUTES * 60;

/// Default staleness threshold for [`InProgress`] flags in milliseconds (`15` minutes).
pub const IN_PROGRESS_DEFAULT_STALE_MS: u64 = (IN_PROGRESS_DEFAULT_STALE_S as u64) * 1000;

static GLOBAL_IN_PROGRESS_STALE_MINUTES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
std::thread_local! {
    static THREAD_IN_PROGRESS_STALE_MINUTES: std::cell::Cell<Option<u32>> = const { std::cell::Cell::new(None) };
}

/// Sets (or resets when `None`) the `agent.in_progress_stale_minutes` setting (clamped to `1..=1440`, E15-7 §3.3).
pub fn set_in_progress_stale_minutes(minutes: Option<u32>) {
    let clamped = minutes.map(|m| m.clamp(IN_PROGRESS_MIN_STALE_MINUTES, IN_PROGRESS_MAX_STALE_MINUTES));
    THREAD_IN_PROGRESS_STALE_MINUTES.with(|c| c.set(clamped));
    GLOBAL_IN_PROGRESS_STALE_MINUTES.store(clamped.unwrap_or(0), std::sync::atomic::Ordering::SeqCst);
}

/// Active `agent.in_progress_stale_minutes` setting (clamped to `1..=1440`, default `15`).
pub fn in_progress_stale_minutes() -> u32 {
    if let Some(m) = THREAD_IN_PROGRESS_STALE_MINUTES.with(|c| c.get()) {
        return m.clamp(IN_PROGRESS_MIN_STALE_MINUTES, IN_PROGRESS_MAX_STALE_MINUTES);
    }
    let global = GLOBAL_IN_PROGRESS_STALE_MINUTES.load(std::sync::atomic::Ordering::SeqCst);
    if global != 0 {
        return global.clamp(IN_PROGRESS_MIN_STALE_MINUTES, IN_PROGRESS_MAX_STALE_MINUTES);
    }
    if let Ok(env_val) = std::env::var("DESIGNOS_IN_PROGRESS_STALE_MINUTES") {
        if let Ok(parsed) = env_val.trim().parse::<u32>() {
            return parsed.clamp(IN_PROGRESS_MIN_STALE_MINUTES, IN_PROGRESS_MAX_STALE_MINUTES);
        }
    }
    IN_PROGRESS_DEFAULT_STALE_MINUTES
}

/// Effective default staleness threshold in seconds from `agent.in_progress_stale_minutes` (`minutes * 60`).
pub fn default_in_progress_stale_s() -> u32 {
    in_progress_stale_minutes().saturating_mul(60)
}

/// Effective default staleness threshold in milliseconds from `agent.in_progress_stale_minutes` (`minutes * 60_000`).
pub fn default_in_progress_stale_ms() -> u64 {
    u64::from(default_in_progress_stale_s()).saturating_mul(1000)
}

/// Injectable clock trait (`fn now_ms()`) so tests do not sleep (E15-7 §10).
pub trait Clock: Send + Sync + std::fmt::Debug {
    fn now_ms(&self) -> u64;
}

/// Default wall-clock implementation of [`Clock`], honouring [`set_mock_now_ms`] in tests.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        now_ms()
    }
}

static GLOBAL_MOCK_NOW_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
std::thread_local! {
    static THREAD_MOCK_NOW_MS: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
}

/// Returns the current Unix timestamp in milliseconds, or the injected test clock when set.
pub fn now_ms() -> u64 {
    if let Some(ms) = THREAD_MOCK_NOW_MS.with(|c| c.get()) {
        return ms;
    }
    let global = GLOBAL_MOCK_NOW_MS.load(std::sync::atomic::Ordering::SeqCst);
    if global != 0 {
        return global;
    }
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Overrides (or clears when `None`) the clock read by [`now_ms`] for deterministic tests.
pub fn set_mock_now_ms(ms: Option<u64>) {
    THREAD_MOCK_NOW_MS.with(|c| c.set(ms));
    GLOBAL_MOCK_NOW_MS.store(ms.unwrap_or(0), std::sync::atomic::Ordering::SeqCst);
}

/// Advances the injected test clock by `delta_ms` milliseconds and returns the new timestamp.
pub fn advance_mock_now_ms(delta_ms: u64) -> u64 {
    let next = now_ms().saturating_add(delta_ms);
    set_mock_now_ms(Some(next));
    next
}

/// Like [`set_mock_now_ms`], but only the calling thread sees the injected clock (`None` clears it).
///
/// [`set_mock_now_ms`] also writes a process-wide clock, so every parallel test on the real clock
/// reads the mock until it is cleared. A flag stamped under that mock and read after it is cleared
/// looks decades old and is dropped as stale. Tests that run next to others use this setter.
pub fn set_thread_mock_now_ms(ms: Option<u64>) {
    THREAD_MOCK_NOW_MS.with(|c| c.set(ms));
}

/// Like [`advance_mock_now_ms`], but moves only the calling thread's injected clock.
pub fn advance_thread_mock_now_ms(delta_ms: u64) -> u64 {
    let next = now_ms().saturating_add(delta_ms);
    set_thread_mock_now_ms(Some(next));
    next
}

/// Transient agent work-in-progress marker for a root frame or section (`E15-7` / `#592`).
#[derive(Clone, Debug, Default, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(from = "InProgressWire")]
pub struct InProgress {
    /// Identifier of the agent run that owns the flag (`$DESIGNOS_AGENT_RUN_ID`; `None` = imported or user-set).
    #[serde(default)]
    pub run_id: Option<String>,
    /// Agent label shown on the canvas badge (`"Claude Code"`, `"Codex"`, etc.).
    #[serde(default)]
    pub agent: Option<String>,
    /// Unix timestamp in milliseconds when the flag was set.
    #[serde(default)]
    pub since_ms: u64,
    /// Optional per-flag staleness timeout in seconds (defaults to `agent.in_progress_stale_minutes`, 15 min).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_after_s: Option<u32>,
    /// Optional short status note (`"Building header..."`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

impl PartialEq for InProgress {
    fn eq(&self, other: &Self) -> bool {
        self.run_id == other.run_id
            && self.agent == other.agent
            && self.since_ms == other.since_ms
            && self.stale_after_s == other.stale_after_s
            && self.status == other.status
    }
}

impl InProgress {
    pub fn new(agent: Option<String>, status: Option<String>, since_ms: u64) -> Self {
        Self {
            run_id: None,
            agent: agent.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
            since_ms,
            stale_after_s: None,
            status: status.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
        }
    }

    /// Canonical timestamp in epoch milliseconds.
    pub fn timestamp_ms(&self) -> u64 {
        self.since_ms
    }

    /// Display text for the canvas badge (`"Agent working"` or `"<agent> working"`, E15-7 §7).
    pub fn badge_text(&self) -> String {
        match self.agent.as_deref().map(str::trim).filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("agent")) {
            Some(a) => format!("{a} working"),
            None => "Agent working".to_string(),
        }
    }

    /// Effective expiry timestamp in epoch milliseconds, or `None` when `run_id` and `stale_after_s`
    /// are both `None` (imported/user-set flags without an explicit timeout never go stale, E15-7 §3.3).
    pub fn expiry_ms(&self, now_ms: u64, default_max_age_ms: u64) -> Option<u64> {
        let effective_since = self.since_ms.min(now_ms);
        if let Some(s) = self.stale_after_s {
            Some(effective_since.saturating_add(u64::from(s).saturating_mul(1000)))
        } else if self.run_id.is_some() {
            Some(effective_since.saturating_add(default_max_age_ms))
        } else {
            None
        }
    }

    /// True when this entry has expired at `now_ms` (treating future `since_ms` as `now_ms`, E15-7 §8.5).
    pub fn is_stale(&self, now_ms: u64, default_max_age_ms: u64) -> bool {
        self.expiry_ms(now_ms, default_max_age_ms).is_some_and(|exp| now_ms >= exp)
    }
}

fn canonical_agent_name(s: &str) -> String {
    let lower = s.trim().to_ascii_lowercase();
    match lower.as_str() {
        "claude" | "claude code" | "claude-code" => "claude".to_string(),
        "codex" | "codex cli" | "codex-cli" => "codex".to_string(),
        "agy" | "antigravity" | "google antigravity" => "agy".to_string(),
        "grok" | "grok cli" | "grok-cli" => "grok".to_string(),
        "zsh" | "sh" | "bash" | "shell" | "zsh/sh" => "shell".to_string(),
        "designos" | "agent guide" => "designos".to_string(),
        _ => lower,
    }
}

fn agent_names_match(a: &str, b: &str) -> bool {
    let ca = canonical_agent_name(a);
    let cb = canonical_agent_name(b);
    !ca.is_empty() && ca == cb
}

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
struct InProgressWire {
    #[serde(default)]
    run_id: Option<String>,
    #[serde(default, alias = "agent_name")]
    agent: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    started_at_ms: Option<u64>,
    #[serde(default)]
    since_ms: Option<u64>,
    #[serde(default)]
    stale_after_s: Option<u32>,
}

impl From<InProgressWire> for InProgress {
    fn from(w: InProgressWire) -> Self {
        let since_ms = w.since_ms.or(w.started_at_ms).unwrap_or(0).min(now_ms());
        Self {
            run_id: w.run_id.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
            agent: w.agent.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
            since_ms,
            stale_after_s: w.stale_after_s,
            status: w.status.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
        }
    }
}

/// On-demand hydrator for deferred component instances on inactive pages of large multi-page `.fig` documents.
pub trait PageHydrator: Send + Sync + std::fmt::Debug {
    fn hydrate_page(&mut self, doc: &mut Document, page: NodeId) -> bool;
    fn hydrate_all(&mut self, doc: &mut Document) -> bool;
    fn is_hydrated(&self, page: NodeId) -> bool;
    fn unexpanded_instances(&self) -> usize {
        0
    }
    fn take_unexpanded_instances(&mut self) -> usize {
        0
    }
    fn last_error(&self) -> Option<&str> {
        None
    }
    fn take_last_error(&mut self) -> Option<String> {
        None
    }
}

mod serde_images {
    use serde::de::{self, MapAccess, SeqAccess, Visitor};
    use serde::ser::SerializeMap;
    use serde::{Deserializer, Serializer};
    use std::collections::BTreeMap;
    use std::fmt;

    const HEX: &[u8; 16] = b"0123456789abcdef";

    fn to_hex_prefixed(bytes: &[u8]) -> String {
        let mut out = Vec::with_capacity(4 + bytes.len() * 2);
        out.extend_from_slice(b"hex:");
        for &b in bytes {
            out.push(HEX[(b >> 4) as usize]);
            out.push(HEX[(b & 0x0f) as usize]);
        }
        // SAFETY: `out` only contains ASCII hex characters.
        unsafe { String::from_utf8_unchecked(out) }
    }

    fn from_hex_str(s: &str) -> Result<Vec<u8>, String> {
        let hex = s.strip_prefix("hex:").unwrap_or(s);
        if hex.len() % 2 != 0 {
            return Err("odd-length hex image payload".into());
        }
        let mut out = Vec::with_capacity(hex.len() / 2);
        let bytes = hex.as_bytes();
        let val = |c: u8| -> Option<u8> {
            match c {
                b'0'..=b'9' => Some(c - b'0'),
                b'a'..=b'f' => Some(c - b'a' + 10),
                b'A'..=b'F' => Some(c - b'A' + 10),
                _ => None,
            }
        };
        let mut i = 0;
        while i < bytes.len() {
            let hi = val(bytes[i]).ok_or_else(|| "invalid hex digit".to_string())?;
            let lo = val(bytes[i + 1]).ok_or_else(|| "invalid hex digit".to_string())?;
            out.push((hi << 4) | lo);
            i += 2;
        }
        Ok(out)
    }

    struct ImageBytes(Vec<u8>);

    impl<'de> serde::Deserialize<'de> for ImageBytes {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            struct BytesVisitor;
            impl<'de> Visitor<'de> for BytesVisitor {
                type Value = ImageBytes;
                fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    f.write_str("a hex string or byte array")
                }
                fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                    from_hex_str(v).map(ImageBytes).map_err(E::custom)
                }
                fn visit_bytes<E: de::Error>(self, v: &[u8]) -> Result<Self::Value, E> {
                    Ok(ImageBytes(v.to_vec()))
                }
                fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                    let mut out = Vec::with_capacity(seq.size_hint().unwrap_or(0));
                    while let Some(b) = seq.next_element::<u8>()? {
                        out.push(b);
                    }
                    Ok(ImageBytes(out))
                }
            }
            deserializer.deserialize_any(BytesVisitor)
        }
    }

    pub fn serialize<S: Serializer>(map: &BTreeMap<String, Vec<u8>>, serializer: S) -> Result<S::Ok, S::Error> {
        let mut m = serializer.serialize_map(Some(map.len()))?;
        for (k, v) in map {
            let hex = to_hex_prefixed(v);
            m.serialize_entry(k, &hex)?;
        }
        m.end()
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<BTreeMap<String, Vec<u8>>, D::Error> {
        struct MapVisitor;
        impl<'de> Visitor<'de> for MapVisitor {
            type Value = BTreeMap<String, Vec<u8>>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a map of image hash to hex string or byte array")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut access: M) -> Result<Self::Value, M::Error> {
                let mut map = BTreeMap::new();
                while let Some((k, ImageBytes(v))) = access.next_entry::<String, ImageBytes>()? {
                    map.insert(k, v);
                }
                Ok(map)
            }
        }
        deserializer.deserialize_map(MapVisitor)
    }
}

impl Document {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), ..Default::default() }
    }

    /// Clones the document structure, styles, variables, comments, and library state without
    /// copying embedded `images` byte buffers (used by background bootstrap indexing).
    pub fn clone_without_images(&self) -> Self {
        Self {
            name: self.name.clone(),
            kind: self.kind,
            nodes: self.nodes.clone(),
            pages: self.pages.clone(),
            styles: self.styles.clone(),
            style_meta: self.style_meta.clone(),
            flows: self.flows.clone(),
            variables: self.variables.clone(),
            variable_collections: self.variable_collections.clone(),
            guides: self.guides.clone(),
            prototype_device: self.prototype_device.clone(),
            comments: self.comments.clone(),
            library: self.library.clone(),
            library_page: self.library_page,
            check_designs: self.check_designs.clone(),
            images: BTreeMap::new(),
            in_progress: self.in_progress.clone(),
            next_id: self.next_id,
            gap_cursor: self.gap_cursor,
            dirty: self.dirty.clone(),
            pending_relayout: self.pending_relayout.clone(),
            layout_epoch: self.layout_epoch,
            page_hydrator: self.page_hydrator.clone(),
        }
    }

    /// Ensures deferred component instances on `page` are expanded on demand.
    /// Returns `true` if any deferred instances were expanded.
    pub fn ensure_page_hydrated(&mut self, page: NodeId) -> bool {
        let Some(h) = self.page_hydrator.clone() else { return false };
        let Ok(mut guard) = h.lock() else { return false };
        if guard.is_hydrated(page) {
            return false;
        }
        let changed = guard.hydrate_page(self, page);
        if changed {
            self.mark_tree_full_dirty();
        }
        changed
    }

    /// Expands all remaining deferred pages (e.g., before full-document export).
    pub fn hydrate_all_pages(&mut self) -> bool {
        let Some(h) = self.page_hydrator.clone() else { return false };
        let Ok(mut guard) = h.lock() else { return false };
        let changed = guard.hydrate_all(self);
        if guard.unexpanded_instances() == 0 && guard.last_error().is_none() {
            self.page_hydrator = None;
        }
        if changed {
            self.mark_tree_full_dirty();
        }
        changed
    }

    /// Number of instances skipped by deferred page hydration due to hitting the clone budget.
    pub fn hydration_unexpanded_instances(&self) -> usize {
        let Some(h) = self.page_hydrator.as_ref() else { return 0 };
        let Ok(guard) = h.lock() else { return 0 };
        guard.unexpanded_instances()
    }

    /// Drains and returns the count of instances skipped by deferred page hydration.
    pub fn take_hydration_unexpanded_instances(&self) -> usize {
        let Some(h) = self.page_hydrator.as_ref() else { return 0 };
        let Ok(mut guard) = h.lock() else { return 0 };
        guard.take_unexpanded_instances()
    }

    /// Last error recorded during deferred page hydration, if any.
    pub fn hydration_error(&self) -> Option<String> {
        let h = self.page_hydrator.as_ref()?;
        let guard = h.lock().ok()?;
        guard.last_error().map(String::from)
    }

    /// Drains and returns the last error recorded during deferred page hydration, if any.
    pub fn take_hydration_error(&self) -> Option<String> {
        let h = self.page_hydrator.as_ref()?;
        let mut guard = h.lock().ok()?;
        guard.take_last_error()
    }

    /// Hands out a fresh id. The counter never wraps: once it is exhausted (a hostile or
    /// corrupted file can carry ids near `u64::MAX`) ids come from the gaps below the largest
    /// node id instead. Loaders that can renumber (`renumber_ids_for_headroom`) make this a last
    /// resort; the fallback still never returns an id it already returned, even when several ids
    /// are allocated before any of them is inserted.
    pub fn alloc_id(&mut self) -> NodeId {
        match self.next_id.checked_add(1) {
            Some(next) => {
                self.next_id = next;
                NodeId(next)
            }
            None => self.next_gap_id(),
        }
    }

    /// Smallest id (never 0) at or above the cursor that no node uses; advances the cursor past it.
    fn next_gap_id(&mut self) -> NodeId {
        let mut candidate = self.gap_cursor.max(1);
        for id in self.nodes.range(NodeId(candidate)..).map(|(id, _)| id) {
            if id.0 != candidate {
                break;
            }
            // saturating: all of 1..=u64::MAX in use would need more memory than exists.
            candidate = candidate.saturating_add(1);
        }
        self.gap_cursor = candidate.saturating_add(1);
        NodeId(candidate)
    }

    pub fn peek_next_id(&self) -> NodeId {
        NodeId(self.next_id + 1)
    }

    /// Allocate safely after importing a document whose stored counter may be
    /// stale, preserving previously allocated IDs and reporting exhaustion.
    pub fn try_alloc_id(&mut self) -> Option<NodeId> {
        let largest = self.nodes.keys().next_back().map_or(0, |id| id.0);
        self.next_id = self.next_id.max(largest).checked_add(1)?;
        Some(NodeId(self.next_id))
    }

    /// Ensures future [`Self::alloc_id`] calls return ids above `id`, so
    /// callers can assign deterministic ids themselves.
    pub fn reserve_ids(&mut self, id: NodeId) {
        self.next_id = self.next_id.max(id.0);
    }

    /// Display name of a shared style: its metadata name (or a typed `"Unnamed ... style"`
    /// fallback when empty or whitespace-only), else the key.
    pub fn style_name<'a>(&'a self, key: &'a str) -> &'a str {
        if let Some(m) = self.style_meta.get(key) {
            if !m.name.trim().is_empty() {
                m.name.as_str()
            } else {
                Self::unnamed_style_label(m.property)
            }
        } else if key.trim().is_empty() && self.styles.contains_key(key) {
            Self::unnamed_style_label(style_property(self, key))
        } else {
            key
        }
    }

    /// Fallback display label for a table style whose name is empty or whitespace-only.
    pub const fn unnamed_style_label(prop: StyleProperty) -> &'static str {
        match prop {
            StyleProperty::Fill | StyleProperty::Stroke => "Unnamed fill style",
            StyleProperty::Text => "Unnamed text style",
            StyleProperty::Effect => "Unnamed effect style",
            StyleProperty::Grid => "Unnamed grid style",
        }
    }

    /// Whether `key` is a table style whose stored name is empty or whitespace-only.
    pub fn is_unnamed_style(&self, key: &str) -> bool {
        self.style_meta.get(key).is_some_and(|m| m.name.trim().is_empty())
            || (!self.style_meta.contains_key(key) && self.styles.contains_key(key) && key.trim().is_empty())
    }

    /// Whether `key` is an orphan style reference (seeded only from a layer's `style_refs`
    /// after a soft-deleted, `REMOVED`, or missing style node, with no entry in `style_meta`).
    pub fn is_orphan_style(&self, key: &str) -> bool {
        is_orphan_style_in(&self.styles, &self.style_meta, key)
    }

    /// Whether `key` is a live style in the file's own style table (not an orphan reference).
    pub fn is_table_style(&self, key: &str) -> bool {
        self.styles.contains_key(key) && !is_orphan_style_in(&self.styles, &self.style_meta, key)
    }

    /// Record a per-(rule, node) lint suppression in the unified `check_designs.ignored` store.
    pub fn ignore_lint(&mut self, rule_id: &str, node: NodeId) {
        let canonical = crate::lint::canonical_rule_id(rule_id).unwrap_or(rule_id);
        self.check_designs.ignored.insert(IgnoreKey { node, rule: canonical.to_string(), property: String::new() });
    }

    /// Remove a per-(rule, node) lint suppression (matching either canonical or alias rule IDs).
    pub fn unignore_lint(&mut self, rule_id: &str, node: NodeId) {
        let target_canonical = crate::lint::canonical_rule_id(rule_id);
        self.check_designs.ignored.retain(|key| {
            if key.node != node {
                return true;
            }
            if key.rule == rule_id {
                return false;
            }
            if let Some(tc) = target_canonical {
                if crate::lint::canonical_rule_id(&key.rule) == Some(tc) {
                    return false;
                }
            }
            true
        });
    }

    /// Check whether a finding on `(rule_id, node)` with an optional `property` key is suppressed in
    /// `check_designs.ignored`.
    ///
    /// - Whole-node ignores (`k.property.is_empty()`) match when `k.rule == rule_id` or when `k.rule`
    ///   maps to the same canonical rule.
    /// - Per-property ignores (`!k.property.is_empty()`, such as `"fills/0"`) only match when the
    ///   caller queries that exact `property` (including Check designs per-property `"RECOMMEND_VARIABLES"`
    ///   ignores on `"hardcoded-color"`).
    pub fn is_lint_property_ignored(&self, rule_id: &str, node: NodeId, property: &str) -> bool {
        if self.check_designs.ignored.is_empty() {
            return false;
        }
        if !property.is_empty() && self.check_designs.ignored.contains(&IgnoreKey { node, rule: rule_id.to_string(), property: property.to_string() }) {
            return true;
        }
        if self.check_designs.ignored.contains(&IgnoreKey { node, rule: rule_id.to_string(), property: String::new() }) {
            return true;
        }
        let Some(target_canonical) = crate::lint::canonical_rule_id(rule_id) else {
            return false;
        };
        if self.check_designs.ignored.contains(&IgnoreKey { node, rule: target_canonical.to_string(), property: String::new() }) {
            return true;
        }
        if !property.is_empty() {
            if self.check_designs.ignored.contains(&IgnoreKey { node, rule: target_canonical.to_string(), property: property.to_string() }) {
                return true;
            }
            if target_canonical == "hardcoded-color"
                && self.check_designs.ignored.contains(&IgnoreKey { node, rule: "RECOMMEND_VARIABLES".to_string(), property: property.to_string() })
            {
                return true;
            }
        }
        self.check_designs.ignored.iter().any(|k| {
            if k.node != node || crate::lint::canonical_rule_id(&k.rule) != Some(target_canonical) {
                return false;
            }
            if k.property.is_empty() {
                true
            } else {
                !property.is_empty() && k.property == property
            }
        })
    }

    /// Check whether a whole-node finding on `(rule_id, node)` is suppressed in `check_designs.ignored`.
    pub fn is_lint_ignored(&self, rule_id: &str, node: NodeId) -> bool {
        self.is_lint_property_ignored(rule_id, node, "")
    }

    /// Check whether a finding on `(rule_id, node, property)` is suppressed in `check_designs.ignored`.
    pub fn is_lint_ignored_property(&self, rule_id: &str, node: NodeId, property: Option<&str>) -> bool {
        let target_canonical = crate::lint::canonical_rule_id(rule_id);
        let prop = property.unwrap_or("").trim();
        self.check_designs.ignored.iter().any(|k| {
            if k.node != node {
                return false;
            }
            // A legacy Contrast ignore stored `{rule: "RECOMMEND_VARIABLES", property: ""}`:
            // it suppresses `low-contrast-text` / `CONTRAST` (empty property), never token properties on `hardcoded-value`.
            if k.rule.eq_ignore_ascii_case("RECOMMEND_VARIABLES") && k.property.is_empty() {
                return prop.is_empty() && (rule_id.eq_ignore_ascii_case("CONTRAST") || target_canonical == Some("low-contrast-text"));
            }
            let rule_matches = k.rule.eq_ignore_ascii_case(rule_id)
                || target_canonical.is_some_and(|tc| crate::lint::canonical_rule_id(&k.rule) == Some(tc))
                || (target_canonical == Some("hardcoded-value") && prop == "text" && k.rule.eq_ignore_ascii_case("REQUIRE_TEXT_STYLES"));
            if !rule_matches {
                return false;
            }
            if k.property.is_empty() {
                return true;
            }
            if prop.is_empty() {
                return false;
            }
            k.property == prop
                || (k.property == "cornerRadius" && matches!(prop, "topLeftRadius" | "topRightRadius" | "bottomRightRadius" | "bottomLeftRadius"))
                || (k.property == "padding" && matches!(prop, "paddingTop" | "paddingRight" | "paddingBottom" | "paddingLeft"))
        })
    }

    #[inline]
    pub fn runtime_id(&self) -> u64 {
        self.dirty.runtime_id
    }

    /// Gives this document an id of its own, for a clone that is opened as a file of its own (a duplicated tab):
    /// a clone otherwise has the id of the document it came from.
    pub fn renew_runtime_id(&mut self) {
        self.dirty.runtime_id = NEXT_DOC_RUNTIME_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    #[inline]
    pub fn mutation_epoch(&self) -> u64 {
        self.dirty.epoch
    }

    #[inline]
    pub fn bump_mutation_epoch(&mut self) {
        self.dirty.epoch += 1;
    }

    #[inline]
    pub fn mark_full_dirty(&mut self) {
        self.dirty.epoch += 1;
        self.dirty.geom_full_epoch = self.dirty.epoch;
        self.dirty.tree_full_epoch = self.dirty.epoch;
    }

    #[inline]
    pub fn mark_tree_full_dirty(&mut self) {
        self.dirty.epoch += 1;
        self.dirty.tree_full_epoch = self.dirty.epoch;
    }

    #[inline]
    pub fn mark_node_dirty(&mut self, id: NodeId) {
        self.dirty.epoch += 1;
        let ep = self.dirty.epoch;
        let full = self.dirty.geom_full_epoch.max(self.dirty.tree_full_epoch);
        if self.dirty.write_pos > 0 {
            let last = ((self.dirty.write_pos - 1) as usize) % DIRTY_RING_CAP;
            if self.dirty.ring[last].1 == id && self.dirty.ring[last].0 > full {
                self.dirty.ring[last].0 = ep;
                return;
            }
        }
        let idx = (self.dirty.write_pos as usize) % DIRTY_RING_CAP;
        self.dirty.evicted_epoch = self.dirty.evicted_epoch.max(self.dirty.ring[idx].0);
        self.dirty.ring[idx] = (ep, id);
        self.dirty.write_pos += 1;
    }

    #[inline]
    pub fn mark_node_removed(&mut self, id: NodeId) {
        self.dirty.epoch += 1;
        let ep = self.dirty.epoch;
        let idx = (self.dirty.removed_write_pos as usize) % REMOVED_RING_CAP;
        self.dirty.removed_evicted_epoch = self.dirty.removed_evicted_epoch.max(self.dirty.removed_ring[idx].0);
        self.dirty.removed_ring[idx] = (ep, id);
        self.dirty.removed_write_pos += 1;
    }

    /// Appends the unique [`NodeId`]s modified or inserted into `dirty_out` and
    /// removed [`NodeId`]s into `removed_out` since `since_epoch`, returning `true`
    /// if no full-tree/full-geom invalidation or ring eviction occurred since `since_epoch`.
    pub fn dirty_changes_since(&self, since_epoch: u64, dirty_out: &mut Vec<NodeId>, removed_out: &mut Vec<NodeId>) -> bool {
        dirty_out.clear();
        removed_out.clear();
        if since_epoch > self.dirty.epoch
            || since_epoch < self.dirty.geom_full_epoch
            || since_epoch < self.dirty.tree_full_epoch
            || since_epoch < self.dirty.evicted_epoch
            || since_epoch < self.dirty.removed_evicted_epoch
        {
            return false;
        }
        if since_epoch == self.dirty.epoch {
            return true;
        }
        let r_count = (self.dirty.removed_write_pos as usize).min(REMOVED_RING_CAP);
        for i in 0..r_count {
            let (ep, id) = self.dirty.removed_ring[i];
            if ep > since_epoch && !removed_out.contains(&id) {
                removed_out.push(id);
            }
        }
        let count = (self.dirty.write_pos as usize).min(DIRTY_RING_CAP);
        for i in 0..count {
            let (ep, id) = self.dirty.ring[i];
            if ep > since_epoch && !dirty_out.contains(&id) {
                dirty_out.push(id);
            }
        }
        true
    }

    /// Appends the unique [`NodeId`]s modified via [`Self::get_mut`] since
    /// `since_epoch` into `out` and returns `true` if no removals or
    /// ring-evicted mutations occurred since `since_epoch`.
    pub fn dirty_nodes_since(&self, since_epoch: u64, out: &mut Vec<NodeId>) -> bool {
        let mut removed = Vec::new();
        self.dirty_changes_since(since_epoch, out, &mut removed) && removed.is_empty()
    }

    /// Returns `true` if `id` is guaranteed not to have had its geometry or
    /// global styles/variables mutated since `since_epoch`.
    #[inline]
    pub fn is_node_clean_since(&self, id: NodeId, since_epoch: u64) -> bool {
        if since_epoch == self.dirty.epoch {
            return true;
        }
        if since_epoch > self.dirty.epoch || since_epoch < self.dirty.geom_full_epoch || since_epoch < self.dirty.evicted_epoch {
            return false;
        }
        let count = (self.dirty.write_pos as usize).min(DIRTY_RING_CAP);
        for i in 0..count {
            let (ep, dirty_id) = self.dirty.ring[i];
            if ep > since_epoch && dirty_id == id {
                return false;
            }
        }
        true
    }

    pub fn add_page(&mut self, name: impl Into<String>) -> NodeId {
        let id = self.alloc_id();
        let mut page = Node::new(id, NodeKind::Page { background: Color::rgb(0x5A, 0x5A, 0x5A) });
        page.name = name.into();
        self.nodes.insert(id, page);
        self.pages.push(id);
        self.mark_tree_full_dirty();
        id
    }

    /// Inserts `node` as the last (topmost) child of `parent`.
    pub fn insert(&mut self, parent: NodeId, mut node: Node) -> Result<NodeId, DocError> {
        let p = self.nodes.get_mut(&parent).ok_or(DocError::NotFound(parent))?;
        if !p.kind.can_hold_children() {
            return Err(DocError::NotAContainer(parent));
        }
        let id = node.id;
        p.children.push(id);
        node.parent = Some(parent);
        self.next_id = self.next_id.max(id.0);
        self.nodes.insert(id, node);
        self.mark_node_dirty(parent);
        self.mark_node_dirty(id);
        Ok(id)
    }

    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(&id)
    }

    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        if self.nodes.contains_key(&id) {
            self.mark_node_dirty(id);
        }
        self.nodes.get_mut(&id)
    }

    /// Removes `id` and its subtree, returning the removed nodes (root first).
    pub fn remove(&mut self, id: NodeId) -> Result<Vec<Node>, DocError> {
        let parent = self.nodes.get(&id).ok_or(DocError::NotFound(id))?.parent;
        if let Some(p) = parent.and_then(|p| self.nodes.get_mut(&p)) {
            p.children.retain(|c| *c != id);
        }
        self.pages.retain(|p| *p != id);
        let mut out = Vec::new();
        let mut stack = vec![id];
        while let Some(n) = stack.pop() {
            self.in_progress.remove(&n);
            if let Some(node) = self.nodes.remove(&n) {
                stack.extend(node.children.iter().rev().copied());
                out.push(node);
            }
        }
        if out.len() <= REMOVED_RING_CAP && parent.is_some() {
            if let Some(p) = parent {
                self.mark_node_dirty(p);
            }
            for r in &out {
                self.mark_node_removed(r.id);
            }
        } else {
            self.mark_tree_full_dirty();
        }
        Ok(out)
    }

    /// Returns the current timestamp in milliseconds via [`now_ms`] (honouring the injected test clock).
    pub fn now_ms(&self) -> u64 {
        now_ms()
    }

    /// Returns the active (non-stale, existing-node) [`InProgress`] record for `id`, if flagged.
    pub fn get_in_progress(&self, id: NodeId) -> Option<&InProgress> {
        let now = now_ms();
        let default_ms = default_in_progress_stale_ms();
        self.in_progress.get(&id).filter(|ip| self.nodes.contains_key(&id) && !ip.is_stale(now, default_ms))
    }

    /// Sets or clears the [`InProgress`] flag on `id` without touching [`History`].
    pub fn set_in_progress(&mut self, id: NodeId, state: Option<InProgress>) -> Option<InProgress> {
        let prev = match state {
            Some(mut st) => {
                st.since_ms = st.since_ms.min(now_ms());
                self.in_progress.insert(id, st)
            }
            None => self.in_progress.remove(&id),
        };
        self.bump_mutation_epoch();
        prev
    }

    /// Validates that `id` is a root frame or section eligible for an [`InProgress`] flag
    /// (a `Frame`, `Section`, `Component`, `ComponentSet`, or `Instance` whose parent is a `Page`
    /// or a top-level `Section`). On failure, returns `Err(Some(root_frame_id))` when `id` lies inside
    /// a valid root frame ancestor, or `Err(None)` otherwise.
    pub fn is_in_progress_root(&self, id: NodeId) -> Result<(), Option<NodeId>> {
        let is_eligible_kind = |n: &Node| {
            matches!(n.kind, NodeKind::Frame(_) | NodeKind::Section | NodeKind::Component(_) | NodeKind::ComponentSet(_) | NodeKind::Instance { .. })
        };
        let is_root_container = |pid: NodeId| -> bool {
            let Some(p) = self.nodes.get(&pid) else { return false };
            match p.kind {
                NodeKind::Page { .. } => true,
                NodeKind::Section => p.parent.and_then(|gp| self.nodes.get(&gp)).is_some_and(|gn| matches!(gn.kind, NodeKind::Page { .. })),
                _ => false,
            }
        };
        let Some(node) = self.nodes.get(&id) else { return Err(None) };
        if is_eligible_kind(node) && node.parent.is_some_and(is_root_container) {
            return Ok(());
        }
        let mut cur = node.parent;
        let mut nearest_root = None;
        while let Some(cid) = cur {
            let Some(cn) = self.nodes.get(&cid) else { break };
            if is_eligible_kind(cn) && cn.parent.is_some_and(is_root_container) {
                nearest_root = Some(cid);
                break;
            }
            cur = cn.parent;
        }
        Err(nearest_root)
    }

    /// Clears all [`InProgress`] flags owned by `agent_or_run_id` (matching either `run_id` or `agent`,
    /// or all flags when `agent_or_run_id` is `"*"` or empty), returning the cleared [`NodeId`]s.
    pub fn clear_in_progress_for_agent(&mut self, agent_or_run_id: &str) -> Vec<NodeId> {
        let key = agent_or_run_id.trim();
        let match_all = key.is_empty() || key == "*";
        let mut matched_run_ids = std::collections::BTreeSet::new();
        if !match_all {
            matched_run_ids.insert(key.to_string());
            for ip in self.in_progress.values() {
                if ip.agent.as_deref().is_some_and(|a| agent_names_match(a, key)) {
                    if let Some(r) = &ip.run_id {
                        matched_run_ids.insert(r.clone());
                    }
                }
            }
        }
        let removed: Vec<NodeId> = self
            .in_progress
            .iter()
            .filter(|(_, ip)| {
                match_all || ip.run_id.as_ref().is_some_and(|r| matched_run_ids.contains(r)) || ip.agent.as_deref().is_some_and(|a| agent_names_match(a, key))
            })
            .map(|(id, _)| *id)
            .collect();
        for id in &removed {
            self.in_progress.remove(id);
        }
        if !removed.is_empty() {
            self.bump_mutation_epoch();
        }
        removed
    }

    /// Clears all [`InProgress`] flags owned by `run_id` or `agent` (including sibling flags sharing
    /// the same `run_id` as a flag matching `agent`), returning the cleared [`NodeId`]s (E15-7 §3.2).
    pub fn clear_in_progress_for_run_or_agent(&mut self, run_id: Option<&str>, agent: Option<&str>) -> Vec<NodeId> {
        let rid = run_id.map(str::trim).filter(|s| !s.is_empty());
        let ag = agent.map(str::trim).filter(|s| !s.is_empty());
        let mut matched_run_ids = std::collections::BTreeSet::new();
        if let Some(r) = rid {
            matched_run_ids.insert(r.to_string());
        }
        if let Some(a) = ag {
            for ip in self.in_progress.values() {
                if ip.agent.as_deref().is_some_and(|ia| agent_names_match(ia, a)) {
                    if let Some(r) = &ip.run_id {
                        matched_run_ids.insert(r.clone());
                    }
                }
            }
        }
        let removed: Vec<NodeId> = self
            .in_progress
            .iter()
            .filter(|(_, ip)| {
                ip.run_id.as_ref().is_some_and(|r| matched_run_ids.contains(r))
                    || ag.is_some_and(|a| ip.agent.as_deref().is_some_and(|ia| agent_names_match(ia, a)))
            })
            .map(|(id, _)| *id)
            .collect();
        for id in &removed {
            self.in_progress.remove(id);
        }
        if !removed.is_empty() {
            self.bump_mutation_epoch();
        }
        removed
    }

    /// Clears [`InProgress`] flags whose age at `now_ms` exceeds their `stale_after_s` (or
    /// `default_max_age_ms` when `stale_after_s` is `None`), returning the cleared [`NodeId`]s.
    pub fn clear_stale_in_progress(&mut self, now_ms: u64, default_max_age_ms: u64) -> Vec<NodeId> {
        let removed: Vec<NodeId> =
            self.in_progress.iter().filter(|(id, ip)| !self.nodes.contains_key(id) || ip.is_stale(now_ms, default_max_age_ms)).map(|(id, _)| *id).collect();
        for id in &removed {
            self.in_progress.remove(id);
        }
        if !removed.is_empty() {
            self.bump_mutation_epoch();
        }
        removed
    }

    /// Prunes any [`InProgress`] entries whose target node no longer exists or that are stale at [`now_ms`].
    pub fn prune_in_progress(&mut self) -> Vec<NodeId> {
        self.clear_stale_in_progress(now_ms(), default_in_progress_stale_ms())
    }

    /// Moves `id` under `new_parent` at `index` (clamped).
    pub fn reparent(&mut self, id: NodeId, new_parent: NodeId, index: usize) -> Result<(), DocError> {
        if self.is_ancestor(id, new_parent) || id == new_parent {
            return Err(DocError::Cycle(id));
        }
        if !self.nodes.get(&new_parent).ok_or(DocError::NotFound(new_parent))?.kind.can_hold_children() {
            return Err(DocError::NotAContainer(new_parent));
        }
        let old = self.nodes.get(&id).ok_or(DocError::NotFound(id))?.parent;
        if let Some(p) = old.and_then(|p| self.nodes.get_mut(&p)) {
            p.children.retain(|c| *c != id);
        }
        let p = self.nodes.get_mut(&new_parent).unwrap();
        let index = index.min(p.children.len());
        p.children.insert(index, id);
        self.nodes.get_mut(&id).unwrap().parent = Some(new_parent);
        if let Some(old_p) = old {
            self.mark_node_dirty(old_p);
        }
        if old != Some(new_parent) {
            self.mark_node_dirty(new_parent);
        }
        self.mark_node_dirty(id);
        Ok(())
    }

    pub fn is_ancestor(&self, ancestor: NodeId, mut of: NodeId) -> bool {
        while let Some(p) = self.nodes.get(&of).and_then(|n| n.parent) {
            if p == ancestor {
                return true;
            }
            of = p;
        }
        false
    }

    /// Absolute transform (page space) of a node.
    pub fn world_transform(&self, id: NodeId) -> Affine {
        let mut chain = Vec::new();
        let mut cur = Some(id);
        while let Some(c) = cur {
            let Some(n) = self.nodes.get(&c) else { break };
            chain.push(n.transform);
            cur = n.parent;
        }
        chain.iter().rev().fold(Affine::IDENTITY, |acc, t| acc * *t)
    }

    /// Page that contains `id`.
    pub fn page_of(&self, mut id: NodeId) -> Option<NodeId> {
        loop {
            let n = self.nodes.get(&id)?;
            match n.parent {
                Some(p) => id = p,
                None => return matches!(n.kind, NodeKind::Page { .. }).then_some(id),
            }
        }
    }

    /// Pre-order traversal of a subtree, back-to-front paint order.
    pub fn walk(&self, root: NodeId, mut f: impl FnMut(&Node, usize)) {
        fn rec(doc: &Document, id: NodeId, depth: usize, f: &mut dyn FnMut(&Node, usize)) {
            if let Some(n) = doc.nodes.get(&id) {
                f(n, depth);
                for c in &n.children {
                    rec(doc, *c, depth + 1, f);
                }
            }
        }
        rec(self, root, 0, &mut f);
    }

    /// Counts the number of nodes in `page`'s subtree in $O(|T_{\text{page}}|)$ time.
    pub fn page_node_count(&self, page: NodeId) -> usize {
        let mut count = 0;
        self.walk(page, |_, _| count += 1);
        count
    }

    /// Collects all [`NodeId`]s reachable from `self.pages` (following
    /// `n.children`, `NodeKind::Instance { component: Some(cid), .. }`,
    /// [`ComponentLink::source`], and [`PropValue::Instance`] /
    /// [`PropKind::InstanceSwap`]) and removes any unreachable orphan nodes
    /// from `self.nodes`, returning the number of removed nodes.
    pub fn prune_unreachable(&mut self) -> usize {
        let mut reachable = std::collections::BTreeSet::new();
        let mut stack: Vec<NodeId> = self.pages.clone();
        if let Some(lp) = self.library_page {
            stack.push(lp);
        }
        while let Some(id) = stack.pop() {
            if !reachable.insert(id) {
                continue;
            }
            if let Some(n) = self.nodes.get(&id) {
                stack.extend(n.children.iter().copied());
                if let NodeKind::Instance { component: Some(cid), .. } = &n.kind {
                    stack.push(*cid);
                }
                if let Some(src) = n.component.source {
                    stack.push(src);
                }
                for prop in &n.component.props {
                    if let PropValue::Instance(cid) = prop.default {
                        stack.push(cid);
                    }
                    if let PropKind::InstanceSwap { preferred } = &prop.kind {
                        stack.extend(preferred.iter().copied());
                    }
                }
                for val in n.component.values.values() {
                    if let PropValue::Instance(cid) = val {
                        stack.push(*cid);
                    }
                }
                for cfg in n.component.slot_configs.values() {
                    stack.extend(cfg.preferred.iter().copied());
                }
            }
        }
        let before = self.nodes.len();
        self.nodes.retain(|id, _| reachable.contains(id));
        self.in_progress.retain(|id, _| reachable.contains(id));
        let removed = before - self.nodes.len();
        if removed > 0 {
            if !self.check_designs.ignored.is_empty() {
                self.check_designs.ignored.retain(|k| reachable.contains(&k.node));
            }
            self.mark_tree_full_dirty();
        }
        removed
    }

    /// Compact JSON of the document. A number that is not finite (NaN, infinity) is written as its
    /// neutral value instead (see [`finite_json`]), so the text always parses back; a file used to
    /// fail to open with `invalid type: null, expected f32` after one such number was saved.
    pub fn to_json(&self) -> serde_json::Result<String> {
        let (json, replaced) = self.to_json_counted()?;
        if replaced > 0 {
            static WARNED: std::sync::Once = std::sync::Once::new();
            WARNED.call_once(|| eprintln!("designos-doc: wrote {replaced} non-finite numbers as their neutral value (reported once per run)"));
        }
        Ok(json)
    }

    /// Like [`Document::to_json`], also returning how many non-finite numbers were replaced.
    pub fn to_json_counted(&self) -> serde_json::Result<(String, usize)> {
        let now = now_ms();
        // Flags of deleted layers and stale flags never reach the file: serialize through a view that filters them.
        if self.in_progress.iter().any(|(id, ip)| !self.nodes.contains_key(id) || ip.is_stale(now, IN_PROGRESS_DEFAULT_STALE_MS)) {
            #[derive(Serialize)]
            struct DocumentSerializeView<'a> {
                name: &'a String,
                kind: &'a DocumentKind,
                nodes: &'a BTreeMap<NodeId, Node>,
                pages: &'a Vec<NodeId>,
                styles: &'a BTreeMap<String, Style>,
                style_meta: &'a BTreeMap<String, StyleMeta>,
                flows: &'a Vec<Flow>,
                variables: &'a BTreeMap<String, Variable>,
                variable_collections: &'a BTreeMap<String, VariableCollection>,
                guides: &'a BTreeMap<NodeId, Vec<Guide>>,
                prototype_device: &'a PrototypeDevice,
                comments: &'a Vec<CommentThread>,
                library: &'a LibraryState,
                library_page: &'a Option<NodeId>,
                #[serde(skip_serializing_if = "CheckDesignsSettings::is_empty")]
                check_designs: &'a CheckDesignsSettings,
                #[serde(with = "serde_images", skip_serializing_if = "BTreeMap::is_empty")]
                images: &'a BTreeMap<String, Vec<u8>>,
                #[serde(skip_serializing_if = "BTreeMap::is_empty")]
                in_progress: BTreeMap<NodeId, &'a InProgress>,
                next_id: u64,
            }
            let filtered_in_progress: BTreeMap<NodeId, &InProgress> = self
                .in_progress
                .iter()
                .filter(|(id, ip)| self.nodes.contains_key(id) && !ip.is_stale(now, IN_PROGRESS_DEFAULT_STALE_MS))
                .map(|(id, ip)| (*id, ip))
                .collect();
            let view = DocumentSerializeView {
                name: &self.name,
                kind: &self.kind,
                nodes: &self.nodes,
                pages: &self.pages,
                styles: &self.styles,
                style_meta: &self.style_meta,
                flows: &self.flows,
                variables: &self.variables,
                variable_collections: &self.variable_collections,
                guides: &self.guides,
                prototype_device: &self.prototype_device,
                comments: &self.comments,
                library: &self.library,
                library_page: &self.library_page,
                check_designs: &self.check_designs,
                images: &self.images,
                in_progress: filtered_in_progress,
                next_id: self.next_id,
            };
            return finite_json::to_string_finite(&view);
        }
        // Ignore keys of deleted layers are written as they are and dropped when the file is read
        // back (`from_json_with_report`): pruning here would clone the whole document, images
        // included, on every save after an ignored layer was deleted.
        let now = now_ms();
        if self.in_progress.iter().any(|(id, ip)| !self.nodes.contains_key(id) || ip.is_stale(now, IN_PROGRESS_DEFAULT_STALE_MS)) {
            let mut clean = self.clone();
            clean.in_progress.retain(|id, ip| clean.nodes.contains_key(id) && !ip.is_stale(now, IN_PROGRESS_DEFAULT_STALE_MS));
            return finite_json::to_string_finite(&clean);
        }
        finite_json::to_string_finite(self)
    }

    /// How many non-finite numbers [`Document::to_json_counted`] would replace, counted without
    /// building the JSON text.
    pub fn count_non_finite(&self) -> serde_json::Result<usize> {
        finite_json::count_non_finite(self)
    }

    /// Replaces any non-finite numbers in the document with their neutral values (`0` or `1`) in
    /// place without touching unaffected entries, embedded image blobs, `page_hydrator`, or
    /// running [`Document::migrate_text`]. Returns how many numbers were replaced.
    pub fn neutralize_non_finite(&mut self) -> usize {
        let mut replaced = 0;
        for node in self.nodes.values_mut() {
            replaced += finite_json::neutralize_in_place(node);
        }
        for style in self.styles.values_mut() {
            replaced += finite_json::neutralize_in_place(style);
        }
        for meta in self.style_meta.values_mut() {
            replaced += finite_json::neutralize_in_place(meta);
        }
        for flow in &mut self.flows {
            replaced += finite_json::neutralize_in_place(flow);
        }
        for var in self.variables.values_mut() {
            replaced += finite_json::neutralize_in_place(var);
        }
        for col in self.variable_collections.values_mut() {
            replaced += finite_json::neutralize_in_place(col);
        }
        for guides in self.guides.values_mut() {
            replaced += finite_json::neutralize_in_place(guides);
        }
        replaced += finite_json::neutralize_in_place(&mut self.prototype_device);
        for comment in &mut self.comments {
            replaced += finite_json::neutralize_in_place(comment);
        }
        replaced += finite_json::neutralize_in_place(&mut self.library);
        if replaced > 0 {
            self.mark_full_dirty();
        }
        replaced
    }

    /// Parses a native document. Text written by older versions that emulated lists with
    /// literal bullet and "1. " prefixes is converted to real lists. A malformed node graph is
    /// repaired in place (see [`Document::validate_and_repair`]) and noted on stderr, and a `null`
    /// where a number belongs (a NaN saved by an older version) is read as the neutral value.
    pub fn from_json(s: &str) -> serde_json::Result<Self> {
        let (doc, repaired) = Self::from_json_with_report(s)?;
        if !repaired.is_clean() {
            eprintln!("designos-doc: repaired a damaged document on load: {repaired:?}");
        }
        Ok(doc)
    }

    /// Like [`Document::from_json`] but silent: returns what the load-time repair changed so a
    /// caller that must not accept a repaired document (for example the MCP server) can refuse it.
    pub fn from_json_with_report(s: &str) -> serde_json::Result<(Self, RepairReport)> {
        // A `null` in a number slot is how older versions saved a NaN or infinity; it is read as the
        // neutral value and counted (see [`finite_json`]).
        let paragraph_fields: BTreeMap<String, u8> = serde_json::from_str::<serde_json::Value>(s)
            .ok()
            .and_then(|v| v.get("styles").and_then(|v| v.as_object()).cloned())
            .map(|styles| {
                styles
                    .into_iter()
                    .filter_map(|(key, value)| {
                        let obj = value.get("Text").or_else(|| value.get("text"))?.as_object()?;
                        let mut bits = 0;
                        if obj.contains_key("paragraph_indent") {
                            bits |= 1;
                        }
                        if obj.contains_key("list_spacing") {
                            bits |= 2;
                        }
                        if obj.contains_key("leading_trim") {
                            bits |= 4;
                        }
                        Some((key, bits))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let (mut doc, non_finite_numbers): (Self, usize) = finite_json::from_str_repairing(s)?;
        for (key, bits) in paragraph_fields {
            let meta = doc.style_meta.entry(key).or_default();
            meta.text_paragraph_fields = Some(meta.text_paragraph_fields.unwrap_or(0) | bits);
        }
        let mut report = doc.validate_and_repair();
        let now = now_ms();
        doc.in_progress.retain(|id, _| doc.nodes.contains_key(id));
        for ip in doc.in_progress.values_mut() {
            if ip.since_ms > now {
                ip.since_ms = now;
            }
        }
        report.non_finite_numbers = non_finite_numbers;
        doc.check_designs.ignored.retain(|k| doc.nodes.contains_key(&k.node));
        doc.migrate_text();
        Ok((doc, report))
    }

    /// Converts literal list prefixes of every text node to real lists (see [`migrate_literal_lists`])
    /// and migrates legacy `gridChildHorizontalAlign` / `gridChildVerticalAlign` metadata keys onto [`LayoutChild`].
    pub fn migrate_text(&mut self) {
        for node in self.nodes.values_mut() {
            if let NodeKind::Text(t) = &mut node.kind {
                migrate_literal_lists(t);
            }
            node.migrate_grid_child_align_metadata();
        }
        self.infer_grid_align_axis_overrides();
        self.mark_full_dirty();
    }

    /// Files saved before per-axis grid alignment overrides existed carry a `Layout` override with both axis
    /// flags clear, which component sync read as "nothing on these axes is overridden". Flags each axis whose
    /// value differs from the linked main child, so an overridden alignment survives the next sync. Nodes that
    /// already have a flag set are left alone, so the pass is idempotent.
    fn infer_grid_align_axis_overrides(&mut self) {
        let inferred: Vec<(NodeId, [bool; 2])> = self
            .nodes
            .values()
            .filter(|n| n.component.grid_align_overrides == [false, false] && n.component.overrides.contains(&OverrideField::Layout))
            .filter_map(|n| {
                let main = self.nodes.get(&n.component.source?)?;
                let axes = components::record_grid_align_overrides(main, n);
                (axes != [false, false]).then_some((n.id, axes))
            })
            .collect();
        for (id, axes) in inferred {
            if let Some(n) = self.nodes.get_mut(&id) {
                n.component.grid_align_overrides = axes;
            }
        }
    }
}

/// Which node property a shared style applies to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StyleProperty {
    #[default]
    Fill,
    Stroke,
    Text,
    Effect,
    Grid,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StyleMeta {
    /// Name as shown in the UI, may contain "/" for grouping ("Brand/Primary").
    pub name: String,
    /// Library key (Figma style `key`), for linking to published libraries.
    pub key: Option<String>,
    pub description: String,
    /// Property the style is used for (a `Style::Fill` can be a stroke style).
    pub property: StyleProperty,
    /// Property key -> variable id bound on this style.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bindings: BTreeMap<String, String>,
    /// Figma `sortPosition` fractional-indexing key ordering styles within a property.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort_position: Option<String>,
    /// Presence bits for paragraph fields in text styles loaded from legacy JSON (1=indent, 2=list spacing, 4=leading trim).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_paragraph_fields: Option<u8>,
}

/// Returns paragraph-field ownership after replacing a text style.
///
/// `Some(0)` means an old style did not store any of the fields introduced after
/// the original text-style format. A newly-created style owns all three. For
/// an old style, changing one of the fields promotes only that field, so an
/// unrelated style edit keeps legacy consumer values intact.
pub fn text_style_paragraph_fields_after_update(previous: Option<&Style>, next: &Style, existing: Option<u8>) -> Option<u8> {
    let Style::Text(next) = next else { return existing };
    let Some(previous) = previous else { return existing };
    let Style::Text(previous) = previous else { return existing };
    let Some(mut bits) = existing else { return Some(0b111) };
    if previous.paragraph_indent != next.paragraph_indent {
        bits |= 0b001;
    }
    if previous.list_spacing != next.list_spacing {
        bits |= 0b010;
    }
    if previous.leading_trim != next.leading_trim {
        bits |= 0b100;
    }
    Some(bits)
}

/// Shared (library) style referenced by nodes via `style_refs`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Style {
    Fill(Vec<Paint>),
    Text(TextStyle),
    Effect(Vec<Effect>),
    Grid(Vec<LayoutGrid>),
}

/// Decides whether `key` is an orphan style reference from the style tables alone,
/// without touching `Document::nodes`.
fn is_orphan_style_in(styles: &BTreeMap<String, Style>, style_meta: &BTreeMap<String, StyleMeta>, key: &str) -> bool {
    if style_meta.contains_key(key) {
        return false;
    }
    if !styles.contains_key(key) {
        return true;
    }
    let is_guid = key
        .split_once(':')
        .is_some_and(|(a, b)| !a.is_empty() && !b.is_empty() && a.bytes().all(|c| c.is_ascii_digit()) && b.bytes().all(|c| c.is_ascii_digit()));
    is_guid || key.starts_with("key:") || key.starts_with("S:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_reparent_remove() {
        let mut d = Document::new("t");
        let page = d.add_page("Page 1");
        let fid = d.alloc_id();
        let frame = d.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
        let rid = d.alloc_id();
        let rect = d.insert(page, Node::rectangle(rid, Rect::new(10.0, 10.0, 20.0, 20.0))).unwrap();
        d.reparent(rect, frame, 0).unwrap();
        assert_eq!(d.get(frame).unwrap().children, vec![rect]);
        assert!(d.reparent(frame, rect, 0).is_err());
        assert_eq!(d.page_of(rect), Some(page));
        let removed = d.remove(frame).unwrap();
        assert_eq!(removed.len(), 2);
        assert!(d.get(rect).is_none());
    }

    #[test]
    fn a_clone_shares_the_runtime_id_until_it_is_renewed_and_a_loaded_document_gets_its_own() {
        let mut d = Document::new("t");
        d.add_page("Page 1");
        let mut copy = d.clone();
        assert_eq!(copy.runtime_id(), d.runtime_id());
        copy.renew_runtime_id();
        assert_ne!(copy.runtime_id(), d.runtime_id());
        let (loaded, _) = Document::from_json_with_report(&d.to_json().unwrap()).unwrap();
        assert_ne!(loaded.runtime_id(), d.runtime_id());
        assert_ne!(loaded.runtime_id(), copy.runtime_id());
    }

    #[test]
    fn unknown_layer_holds_children_for_tree_edits_without_becoming_a_container() {
        let mut d = Document::new("t");
        let page = d.add_page("Page 1");
        let uid = d.alloc_id();
        let unknown = d.insert(page, Node::new(uid, NodeKind::Unknown { type_name: "SLIDE".into() })).unwrap();
        let kind = &d.get(unknown).unwrap().kind;
        assert!(kind.can_hold_children());
        assert!(!kind.is_container(), "an unmapped layer must not pick up clipping, layout or hit-testing as a container");
        assert!(kind.frame_props().is_none());

        let rid = d.alloc_id();
        let rect = d.insert(unknown, Node::rectangle(rid, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        let fid = d.alloc_id();
        let frame = d.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 50.0, 50.0))).unwrap();
        d.reparent(rect, frame, 0).unwrap();
        d.reparent(rect, unknown, 0).unwrap();
        assert_eq!(d.get(unknown).unwrap().children, vec![rect]);

        let mut h = History::default();
        h.exec(&mut d, Command::Remove { id: rect }).unwrap();
        assert!(d.get(unknown).unwrap().children.is_empty());
        assert!(h.undo(&mut d).unwrap());
        assert_eq!(d.get(unknown).unwrap().children, vec![rect]);
        assert_eq!(d.get(rect).unwrap().parent, Some(unknown));

        // Leaves still refuse children.
        let lid = d.alloc_id();
        assert!(matches!(d.insert(rect, Node::rectangle(lid, Rect::new(0.0, 0.0, 1.0, 1.0))), Err(DocError::NotAContainer(p)) if p == rect));
        assert!(matches!(d.reparent(frame, rect, 0), Err(DocError::NotAContainer(p)) if p == rect));
    }

    #[test]
    fn json_roundtrip() {
        let mut d = Document::new("t");
        let page = d.add_page("P");
        let id = d.alloc_id();
        d.insert(page, Node::rectangle(id, Rect::new(0.0, 0.0, 5.0, 5.0))).unwrap();
        let back = Document::from_json(&d.to_json().unwrap()).unwrap();
        assert_eq!(back.nodes.len(), 2);
    }

    #[test]
    fn component_link_roundtrip_and_default() {
        let mut d = Document::new("t");
        let page = d.add_page("P");
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 5.0, 5.0));
        n.component.source = Some(NodeId(99));
        n.component.overrides.insert(OverrideField::Fills);
        n.component.bindings.insert(OverrideField::Visible, "Show icon".into());
        d.insert(page, n).unwrap();
        let json = d.to_json().unwrap();
        let back = Document::from_json(&json).unwrap();
        assert_eq!(back.get(id).unwrap().component, d.get(id).unwrap().component);
        // Plain nodes omit the field, and files without it still load.
        assert!(!json.contains("\"component\": {}"));
        let page_json = serde_json::to_string(back.get(page).unwrap()).unwrap();
        assert!(!page_json.contains("\"component\""));
    }

    #[test]
    fn images_compact_and_legacy_roundtrip() {
        let mut d = Document::new("t");
        d.add_page("P");
        d.images.insert("hash1".into(), vec![0x89, 0x50, 0x4e, 0x47]);
        let json = d.to_json().unwrap();
        assert!(json.contains("\"hex:89504e47\""), "images serialize as compact hex: {json}");
        let back = Document::from_json(&json).unwrap();
        assert_eq!(back.images.get("hash1").unwrap(), &vec![0x89, 0x50, 0x4e, 0x47]);

        // Legacy array format still loads cleanly.
        let legacy = json.replace("\"hex:89504e47\"", "[137, 80, 78, 71]");
        let from_legacy = Document::from_json(&legacy).unwrap();
        assert_eq!(from_legacy.images.get("hash1").unwrap(), &vec![0x89, 0x50, 0x4e, 0x47]);
    }

    #[test]
    fn node_default_fields_omitted_and_roundtrip() {
        let mut d = Document::new("t");
        let page = d.add_page("P");
        let id = d.alloc_id();
        d.insert(page, Node::rectangle(id, Rect::new(1.0, 2.0, 30.0, 40.0))).unwrap();
        let rect_json = serde_json::to_string(d.get(id).unwrap()).unwrap();
        for omitted in
            ["\"source_id\"", "\"children\"", "\"effects\"", "\"style_refs\"", "\"fill_geometry\"", "\"stroke_geometry\"", "\"corner_radii\"", "\"stroke\""]
        {
            assert!(!rect_json.contains(omitted), "{omitted} should be omitted on default node: {rect_json}");
        }
        let back = Document::from_json(&d.to_json().unwrap()).unwrap();
        assert_eq!(back.get(id), d.get(id));
    }

    #[test]
    fn prune_unreachable_preserves_reachable_and_removes_orphans() {
        let mut d = Document::new("t");
        let page = d.add_page("Page 1");
        let fid = d.alloc_id();
        let frame = d.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
        let rid = d.alloc_id();
        let rect = d.insert(frame, Node::rectangle(rid, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();

        // Detached component masters & referenced targets (e.g. from hidden/imported library nodes)
        let comp_id = d.alloc_id();
        let comp_child_id = d.alloc_id();
        let swap_default_id = d.alloc_id();
        let swap_pref_id = d.alloc_id();
        let swap_val_id = d.alloc_id();
        let source_target_id = d.alloc_id();

        let mut comp_node = Node::new(comp_id, NodeKind::Component(FrameProps::default()));
        comp_node.children.push(comp_child_id);
        comp_node.component.props.push(ComponentPropDef {
            name: "Icon".into(),
            kind: PropKind::InstanceSwap { preferred: vec![swap_pref_id] },
            default: PropValue::Instance(swap_default_id),
        });
        let mut comp_child = Node::rectangle(comp_child_id, Rect::new(0.0, 0.0, 8.0, 8.0));
        comp_child.parent = Some(comp_id);

        d.nodes.insert(comp_id, comp_node);
        d.nodes.insert(comp_child_id, comp_child);
        d.nodes.insert(swap_default_id, Node::new(swap_default_id, NodeKind::Component(FrameProps::default())));
        d.nodes.insert(swap_pref_id, Node::new(swap_pref_id, NodeKind::Component(FrameProps::default())));
        d.nodes.insert(swap_val_id, Node::new(swap_val_id, NodeKind::Component(FrameProps::default())));
        d.nodes.insert(source_target_id, Node::rectangle(source_target_id, Rect::new(0.0, 0.0, 4.0, 4.0)));

        let inst_id = d.alloc_id();
        let mut inst = Node::new(inst_id, NodeKind::Instance { frame: FrameProps::default(), component: Some(comp_id), component_key: None });
        inst.component.source = Some(source_target_id);
        inst.component.values.insert("Icon".into(), PropValue::Instance(swap_val_id));
        d.insert(page, inst).unwrap();

        // Unreachable orphan subtree
        let orphan_parent = d.alloc_id();
        let orphan_child = d.alloc_id();
        let mut op = Node::frame(orphan_parent, Rect::new(0.0, 0.0, 50.0, 50.0));
        op.children.push(orphan_child);
        let mut oc = Node::rectangle(orphan_child, Rect::new(0.0, 0.0, 10.0, 10.0));
        oc.parent = Some(orphan_parent);
        d.nodes.insert(orphan_parent, op);
        d.nodes.insert(orphan_child, oc);

        let removed = d.prune_unreachable();
        assert_eq!(removed, 2);

        for kept in [page, frame, rect, inst_id, comp_id, comp_child_id, swap_default_id, swap_pref_id, swap_val_id, source_target_id] {
            assert!(d.get(kept).is_some(), "expected node {kept:?} to be preserved");
        }
        assert!(d.get(orphan_parent).is_none());
        assert!(d.get(orphan_child).is_none());
    }

    #[test]
    fn page_node_count_measures_subtree() {
        let mut d = Document::new("t");
        let p1 = d.add_page("Page 1");
        let p2 = d.add_page("Page 2");
        assert_eq!(d.page_node_count(p1), 1);
        assert_eq!(d.page_node_count(p2), 1);
        assert_eq!(d.page_node_count(NodeId(9999)), 0);

        let fid = d.alloc_id();
        let frame = d.insert(p1, Node::frame(fid, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
        let r1 = d.alloc_id();
        d.insert(frame, Node::rectangle(r1, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        let r2 = d.alloc_id();
        d.insert(frame, Node::rectangle(r2, Rect::new(20.0, 0.0, 10.0, 10.0))).unwrap();

        assert_eq!(d.page_node_count(p1), 4);
        assert_eq!(d.page_node_count(p2), 1);
    }

    #[test]
    fn killed_agent_flag_goes_stale() {
        set_thread_mock_now_ms(Some(1_000_000));
        let mut d = Document::new("t");
        let page = d.add_page("Page 1");
        let fid = d.alloc_id();
        let frame = d.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 320.0, 240.0))).unwrap();

        d.set_in_progress(
            frame,
            Some(InProgress {
                run_id: Some("run_killed".into()),
                agent: Some("Claude Code".into()),
                since_ms: now_ms(),
                stale_after_s: Some(60),
                status: Some("Building header...".into()),
            }),
        );
        assert!(d.get_in_progress(frame).is_some());
        assert_eq!(d.get_in_progress(frame).unwrap().badge_text(), "Claude Code working");

        // Advance injected clock by 61 seconds without sleeping.
        advance_thread_mock_now_ms(61_000);
        assert!(d.get_in_progress(frame).is_none(), "stale flag must be hidden from readers");

        // Next save / prune removes the stale entry from the persisted map.
        let json = d.to_json().unwrap();
        assert!(!json.contains("run_killed"), "stale flag must be pruned on save: {json}");
        let reloaded = Document::from_json(&json).unwrap();
        assert!(reloaded.in_progress.is_empty());

        // Clock skew: future since_ms is treated as now_ms().
        set_thread_mock_now_ms(Some(2_000_000));
        d.set_in_progress(frame, Some(InProgress { run_id: Some("run_skew".into()), agent: None, since_ms: 9_000_000, stale_after_s: Some(60), status: None }));
        assert_eq!(d.get_in_progress(frame).unwrap().badge_text(), "Agent working");
        advance_thread_mock_now_ms(61_000);
        assert!(d.get_in_progress(frame).is_none(), "clock-skewed flag must expire 60s after effective now");
        set_thread_mock_now_ms(None);
    }

    #[test]
    fn undo_does_not_touch_flag() {
        set_thread_mock_now_ms(Some(1_000_000));
        let mut d = Document::new("t");
        let page = d.add_page("Page 1");
        let fid = d.alloc_id();
        let frame = d.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 400.0, 300.0))).unwrap();

        let mut h = History::default();
        let mut after = d.get(frame).unwrap().clone();
        after.name = "Renamed Hero".into();
        h.exec(&mut d, Command::Update { node: Box::new(after) }).unwrap();

        // Flag the frame outside History, then undo the previous node edit.
        let ip = InProgress { run_id: Some("run_1".into()), agent: Some("Codex".into()), since_ms: now_ms(), stale_after_s: None, status: None };
        d.set_in_progress(frame, Some(ip.clone()));
        assert!(h.undo(&mut d).unwrap());
        assert_eq!(d.get(frame).unwrap().name, "Frame");
        assert_eq!(d.get_in_progress(frame), Some(&ip), "undoing node edit must leave in_progress flag untouched");

        // Edge case §8.2: undoing the insert that created a flagged frame prunes the entry;
        // redo restores the frame without the flag.
        let fid2 = d.alloc_id();
        let frame2_node = Node::frame(fid2, Rect::new(500.0, 0.0, 300.0, 200.0));
        h.exec(&mut d, Command::Insert { parent: page, index: 1, nodes: vec![frame2_node] }).unwrap();
        d.set_in_progress(fid2, Some(ip));
        assert!(d.get_in_progress(fid2).is_some());
        assert!(h.undo(&mut d).unwrap());
        assert!(!d.in_progress.contains_key(&fid2));
        assert!(h.redo(&mut d).unwrap());
        assert!(d.get(fid2).is_some());
        assert!(d.get_in_progress(fid2).is_none(), "redo must not restore transient work state");
        set_thread_mock_now_ms(None);
    }

    #[test]
    fn not_root_refused() {
        let mut d = Document::new("t");
        let page = d.add_page("Page 1");
        let fid = d.alloc_id();
        let frame = d.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 320.0, 240.0))).unwrap();
        let tid = d.alloc_id();
        let text = d.insert(frame, Node::text(tid, Rect::new(10.0, 10.0, 100.0, 24.0), "Title", TextStyle::default())).unwrap();
        let sid = d.alloc_id();
        let section = d.insert(page, Node::new(sid, NodeKind::Section)).unwrap();
        let sfid = d.alloc_id();
        let sec_frame = d.insert(section, Node::frame(sfid, Rect::new(0.0, 0.0, 200.0, 200.0))).unwrap();

        assert_eq!(d.is_in_progress_root(frame), Ok(()));
        assert_eq!(d.is_in_progress_root(section), Ok(()));
        assert_eq!(d.is_in_progress_root(sec_frame), Ok(()));
        assert_eq!(d.is_in_progress_root(text), Err(Some(frame)));
        assert_eq!(d.is_in_progress_root(page), Err(None));
    }

    #[test]
    fn clear_in_progress_matches_cli_binary_aliases_and_flag_run_id() {
        let mut d = Document::new("t");
        let page = d.add_page("Page 1");
        let f1_id = d.alloc_id();
        let f1 = d.insert(page, Node::frame(f1_id, Rect::new(0.0, 0.0, 320.0, 240.0))).unwrap();
        let f2_id = d.alloc_id();
        let f2 = d.insert(page, Node::frame(f2_id, Rect::new(400.0, 0.0, 320.0, 240.0))).unwrap();

        d.set_in_progress(
            f1,
            Some(InProgress {
                run_id: Some("mcp-run-42".into()),
                agent: Some("Claude Code".into()),
                since_ms: 1_000,
                stale_after_s: Some(300),
                status: Some("Building hero".into()),
            }),
        );
        d.set_in_progress(
            f2,
            Some(InProgress {
                run_id: Some("mcp-run-42".into()),
                agent: None,
                since_ms: 1_000,
                stale_after_s: Some(300),
                status: Some("Building footer".into()),
            }),
        );

        let cleared = d.clear_in_progress_for_run_or_agent(Some("terminal-session-id"), Some("claude"));
        assert_eq!(cleared.len(), 2);
        assert!(d.in_progress.is_empty());
    }

    #[test]
    fn run_end_clear_without_agent_matches_only_the_finishing_run_id() {
        set_thread_mock_now_ms(Some(1_000_000));
        let mut d = Document::new("t");
        let page = d.add_page("Page 1");
        let f1 = d.alloc_id();
        let frame1 = d.insert(page, Node::frame(f1, Rect::new(0.0, 0.0, 200.0, 200.0))).unwrap();
        let f2 = d.alloc_id();
        let frame2 = d.insert(page, Node::frame(f2, Rect::new(240.0, 0.0, 200.0, 200.0))).unwrap();
        let f3 = d.alloc_id();
        let frame3 = d.insert(page, Node::frame(f3, Rect::new(480.0, 0.0, 200.0, 200.0))).unwrap();

        d.set_in_progress(
            frame1,
            Some(InProgress { run_id: Some("p1-tab-1-1-1".into()), agent: Some("claude".into()), since_ms: now_ms(), stale_after_s: Some(900), status: None }),
        );
        d.set_in_progress(
            frame2,
            Some(InProgress { run_id: Some("p1-tab-1-2-2".into()), agent: Some("claude".into()), since_ms: now_ms(), stale_after_s: Some(900), status: None }),
        );
        d.set_in_progress(frame3, Some(InProgress { run_id: None, agent: Some("claude".into()), since_ms: now_ms(), stale_after_s: None, status: None }));

        let cleared = d.clear_in_progress_for_run_or_agent(Some("p1-tab-1-1-1"), None);
        assert_eq!(cleared, vec![frame1]);
        assert!(d.get_in_progress(frame1).is_none());
        assert!(d.get_in_progress(frame2).is_some(), "other concurrent tab's flag must stay active");
        assert!(d.get_in_progress(frame3).is_some(), "run_id: None entry must never be cleared by a run id");
        set_thread_mock_now_ms(None);
    }

    #[test]
    fn in_progress_stale_minutes_setting_controls_default_staleness() {
        set_thread_mock_now_ms(Some(10_000_000));
        set_in_progress_stale_minutes(Some(2));
        assert_eq!(in_progress_stale_minutes(), 2);
        assert_eq!(default_in_progress_stale_s(), 120);
        assert_eq!(default_in_progress_stale_ms(), 120_000);

        let mut d = Document::new("t");
        let page = d.add_page("Page 1");
        let fid = d.alloc_id();
        let frame = d.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 200.0, 200.0))).unwrap();
        d.set_in_progress(
            frame,
            Some(InProgress { run_id: Some("run_cfg".into()), agent: Some("claude".into()), since_ms: now_ms(), stale_after_s: None, status: None }),
        );
        advance_thread_mock_now_ms(119_000);
        assert!(d.get_in_progress(frame).is_some());
        advance_thread_mock_now_ms(2_000);
        assert!(d.get_in_progress(frame).is_none());
        assert_eq!(d.prune_in_progress(), vec![frame]);

        set_in_progress_stale_minutes(Some(0));
        assert_eq!(in_progress_stale_minutes(), IN_PROGRESS_MIN_STALE_MINUTES);
        set_in_progress_stale_minutes(Some(5000));
        assert_eq!(in_progress_stale_minutes(), IN_PROGRESS_MAX_STALE_MINUTES);
        set_in_progress_stale_minutes(None);
        assert_eq!(in_progress_stale_minutes(), IN_PROGRESS_DEFAULT_STALE_MINUTES);
        set_thread_mock_now_ms(None);
    }

    #[test]
    fn thread_mock_clock_is_invisible_to_other_threads() {
        // No other test uses this value, so a sibling test's process-wide mock cannot be mistaken for it.
        const SENTINEL_MS: u64 = 123_456_789;
        set_thread_mock_now_ms(Some(SENTINEL_MS));
        assert_eq!(now_ms(), SENTINEL_MS);
        assert_eq!(advance_thread_mock_now_ms(1_000), SENTINEL_MS + 1_000);
        assert_eq!(now_ms(), SENTINEL_MS + 1_000);

        let seen_elsewhere = std::thread::spawn(now_ms).join().unwrap();
        assert!(seen_elsewhere != SENTINEL_MS && seen_elsewhere != SENTINEL_MS + 1_000, "the thread mock leaked to another thread: {seen_elsewhere}");

        set_thread_mock_now_ms(None);
        assert_ne!(now_ms(), SENTINEL_MS + 1_000, "clearing must restore this thread's own clock");
    }

    #[test]
    fn us01_ac02_round2_is_table_style_and_is_orphan_style_do_not_walk_200k_nodes() {
        let mut doc = Document::new("t");
        let page = doc.add_page("p");
        let fx = Style::Effect(vec![Effect::LayerBlur { radius: 4.0, visible: true }]);

        let mut children = Vec::with_capacity(199_999);
        for i in 2..=200_000u64 {
            let id = NodeId(i);
            let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0));
            n.parent = Some(page);
            if i > 199_981 {
                let idx = i - 199_982;
                n.style_refs.insert("effect".into(), format!("S:orphan-{idx:02}"));
            }
            doc.nodes.insert(id, n);
            children.push(id);
        }
        doc.reserve_ids(NodeId(200_000));
        doc.nodes.get_mut(&page).unwrap().children = children;
        assert_eq!(doc.nodes.len(), 200_000);

        let mut keys = Vec::with_capacity(50);
        for i in 0..25 {
            let k = if i < 15 { format!("S:live-{i:02}") } else { format!("2:{}", 100 + i) };
            doc.styles.insert(k.clone(), fx.clone());
            doc.style_meta.insert(k.clone(), StyleMeta { name: format!("Elevation/{i:02}"), property: StyleProperty::Effect, ..Default::default() });
            keys.push((k, true));
        }
        for i in 0..20 {
            let k = format!("S:orphan-{i:02}");
            doc.styles.insert(k.clone(), fx.clone());
            keys.push((k, false));
        }
        for i in 0..5 {
            let k = format!("1186:{}", 100_350 + i);
            doc.styles.insert(k.clone(), fx.clone());
            keys.push((k, false));
        }
        assert_eq!(doc.styles.len(), 50);

        let budget = if cfg!(debug_assertions) { std::time::Duration::from_millis(50) } else { std::time::Duration::from_millis(5) };
        let t0 = std::time::Instant::now();
        for (k, expected_table) in &keys {
            assert_eq!(doc.is_table_style(k), *expected_table, "is_table_style({k:?}) mismatch");
            assert_eq!(doc.is_orphan_style(k), !*expected_table, "is_orphan_style({k:?}) mismatch");
        }
        let elapsed = t0.elapsed();
        assert!(elapsed < budget, "is_table_style across 50 keys on 200,000 nodes took {elapsed:?} (budget {budget:?})");

        // Prove by construction (Rust borrow checker): holding `&mut doc.nodes` while checking
        // every key through `is_orphan_style_in(&doc.styles, &doc.style_meta, k)` compiles only
        // because `doc.nodes` is never borrowed or touched.
        let nodes_exclusive = &mut doc.nodes;
        for (k, expected_table) in &keys {
            assert_eq!(!is_orphan_style_in(&doc.styles, &doc.style_meta, k), *expected_table);
        }
        assert_eq!(nodes_exclusive.len(), 200_000);
    }
}
