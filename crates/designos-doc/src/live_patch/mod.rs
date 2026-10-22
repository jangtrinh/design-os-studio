//! Node-level edit log exchanged between an agent session (`designos-mcp`) and the editor.
//!
//! For large files the editor hands the agent a scoped copy of the document: top-level frames
//! without their children, styles or comments. A whole document written back from that copy
//! would erase everything the copy left out. The agent side therefore sends the edits it actually
//! executed, as [`LiveOp`]s recorded from its [`Command`](crate::Command)s, and the editor replays
//! them on its full document. Layout passes the agent runs on its partial copy are not commands,
//! so they never reach the editor; the editor lays out the touched containers itself.

pub mod lease;
mod record;
mod replay;
mod replay_comments;
mod validate;

pub use lease::{
    is_root_frame, is_within_frames, root_frame_of, stamp_command_authorship, AffectedSet, Lease, LeasePurpose, LeaseTable, LeaseViolation, PLUGIN_DATA_RUN_KEY,
};
pub use record::{batch_doc_clones, is_independent_batch, live_ops_for_command, live_ops_for_independent};
pub use replay::{replay_live_patch, replay_live_patch_with_leases, LivePatchReport, LivePatchState};

use crate::{Comment, CommentAnchor, CommentThread, ExternalAsset, LibraryRef, Node, NodeId, PublishInfo, Style, StyleMeta, Variable, VariableCollection};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// Format version written in every [`LivePatch`].
pub const LIVE_PATCH_VERSION: u32 = 1;

/// The edits of one agent tool call: one line of the session's `doc-patch.jsonl`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LivePatch {
    pub v: u32,
    /// The editor canvas the agent's document was loaded from (`canvas.designos` modification
    /// time, ns since the Unix epoch). Ids the editor remapped hold only for patches with the same
    /// base: after the agent reloads a newer canvas it sees the editor's own ids.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<u64>,
    /// Optional undo-group label shown in Studio's History panel (e.g. `Agent: <run>`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Identifier of the agent run (`$DESIGNOS_AGENT_RUN_ID`) that produced this patch (`RD-3` / `#572`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    pub ops: Vec<LiveOp>,
}

impl LivePatch {
    pub fn new(ops: Vec<LiveOp>) -> Self {
        Self { v: LIVE_PATCH_VERSION, base: None, label: None, run_id: None, ops }
    }

    pub fn with_base(mut self, base: Option<u64>) -> Self {
        self.base = base;
        self
    }

    pub fn with_label(mut self, label: Option<String>) -> Self {
        self.label = label;
        self
    }

    pub fn with_run_id(mut self, run_id: Option<String>) -> Self {
        self.run_id = run_id.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        self
    }
}

/// The agent's session switched to another document (`designos_open_file`,
/// `designos_new_document`). One line of `doc-patch.jsonl`, written after the document itself
/// (`doc-sync.designos`) so the one log fixes the order: patches above it were made on the
/// previous document, patches below it on the new one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocEpoch {
    pub v: u32,
    /// Counts the switches of one agent session.
    pub doc_epoch: u64,
    /// Size and hash of the document bytes written for this switch. The file is overwritten by
    /// the next switch, so a reader that finds other bytes knows this switch was superseded.
    pub bytes: u64,
    pub hash: u64,
}

impl DocEpoch {
    pub fn of(doc_epoch: u64, doc_bytes: &[u8]) -> Self {
        Self { v: LIVE_PATCH_VERSION, doc_epoch, bytes: doc_bytes.len() as u64, hash: fnv1a(doc_bytes) }
    }

    /// True when `doc_bytes` is the document this switch published.
    pub fn matches(&self, doc_bytes: &[u8]) -> bool {
        self.bytes == doc_bytes.len() as u64 && self.hash == fnv1a(doc_bytes)
    }
}

/// FNV-1a, 64 bit. Both sides run the same build, and the hash only has to tell two documents apart.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3))
}

/// One line of `doc-patch.jsonl`.
#[derive(Clone, Debug, PartialEq)]
pub enum LiveLogLine {
    Patch(LivePatch),
    DocEpoch(DocEpoch),
}

impl LiveLogLine {
    /// Reads one log line. A line holding `doc_epoch` is a document switch, any other an edit.
    pub fn parse(line: &[u8]) -> Result<Self, serde_json::Error> {
        #[derive(Deserialize)]
        struct Probe {
            #[serde(default)]
            doc_epoch: Option<u64>,
        }
        if serde_json::from_slice::<Probe>(line)?.doc_epoch.is_some() {
            serde_json::from_slice(line).map(Self::DocEpoch)
        } else {
            serde_json::from_slice(line).map(Self::Patch)
        }
    }
}

/// Where an inserted or moved node goes among its new siblings, named by its neighbours
/// because the two sides may hold different subsets of the parent's children.
/// `before == None` means "on top" (last child).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiblingHint {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<NodeId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<NodeId>,
}

impl SiblingHint {
    /// The neighbours of position `index` in `siblings`.
    pub fn at(siblings: &[NodeId], index: usize) -> Self {
        let index = index.min(siblings.len());
        Self { after: index.checked_sub(1).map(|i| siblings[i]), before: siblings.get(index).copied() }
    }

    /// The index in `siblings` this hint points at: on top when nothing follows, otherwise just
    /// after `after` or just before `before`, falling back to on top.
    pub fn index_in(&self, siblings: &[NodeId]) -> usize {
        let Some(before) = self.before else { return siblings.len() };
        if let Some(i) = self.after.and_then(|a| siblings.iter().position(|c| *c == a)) {
            return i + 1;
        }
        siblings.iter().position(|c| *c == before).unwrap_or(siblings.len())
    }
}

/// One edit, replayable on a document that holds more than the agent saw.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveOp {
    /// A new subtree (root first) under an existing `parent`.
    Insert {
        parent: NodeId,
        at: SiblingHint,
        nodes: Vec<Node>,
    },
    /// Top-level `Node` fields (as serialized) that changed; `unset` fields return to their
    /// default. Never carries `id`, `parent` or `children`.
    Update {
        id: NodeId,
        #[serde(default, skip_serializing_if = "Map::is_empty")]
        set: Map<String, Value>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        unset: Vec<String>,
    },
    Reparent {
        id: NodeId,
        parent: NodeId,
        at: SiblingHint,
    },
    /// Removes the node and its whole subtree.
    Remove {
        id: NodeId,
    },
    /// Comment threads the agent created, edited or deleted, by thread id. The agent's copy can
    /// lack threads the editor has, so ids it picks may already be taken there.
    Comments {
        /// New threads, whole. An id the editor already uses gets a fresh one.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        added: Vec<CommentThread>,
        /// Edits to threads that existed before, as a delta, so replies made in the editor
        /// meanwhile are kept.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        changed: Vec<ThreadChange>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        removed: Vec<String>,
    },
    /// Shared styles added, updated or removed.
    Styles {
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        upserted: BTreeMap<String, Style>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        upserted_meta: BTreeMap<String, StyleMeta>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        removed: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        removed_meta: Vec<String>,
    },
    /// Variables and variable collections added, updated or removed.
    Variables {
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        upserted_collections: BTreeMap<String, VariableCollection>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        removed_collections: Vec<String>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        upserted_variables: BTreeMap<String, Variable>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        removed_variables: Vec<String>,
    },
    /// Library state and hidden library page changes.
    LibraryState {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        published: Option<Option<PublishInfo>>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        upserted_enabled: Vec<LibraryRef>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        removed_enabled: Vec<String>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        upserted_external: BTreeMap<String, ExternalAsset>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        removed_external: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        library_page: Option<Option<NodeId>>,
    },
    /// Embedded image blobs added or removed.
    Images {
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        add: BTreeMap<String, Vec<u8>>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        remove: Vec<String>,
    },
}

/// What changed in one existing comment thread.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ThreadChange {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<CommentAnchor>,
    /// New messages. A message id the editor's thread already uses gets a fresh one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub added: Vec<Comment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edited: Vec<Comment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed: Vec<String>,
}

/// Longest patch line either side accepts (one tool call's edits).
pub const MAX_LIVE_PATCH_LINE_BYTES: usize = 16 << 20;

/// Fields that structural ops own; field updates never touch them.
pub(crate) const STRUCTURAL_FIELDS: [&str; 3] = ["id", "parent", "children"];

#[cfg(test)]
mod builder_redirect_tests;
#[cfg(test)]
mod log_line_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod validate_tests;
