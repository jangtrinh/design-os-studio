//! Node-level edit log exchanged between an agent session (`designos-mcp`) and the editor.
//!
//! For large files the editor hands the agent a scoped copy of the document: top-level frames
//! without their children, styles or comments. A whole document written back from that copy
//! would erase everything the copy left out. The agent side therefore sends the edits it actually
//! executed, as [`LiveOp`]s recorded from its [`Command`](crate::Command)s, and the editor replays
//! them on its full document. Layout passes the agent runs on its partial copy are not commands,
//! so they never reach the editor; the editor lays out the touched containers itself.

mod record;
mod replay;
mod replay_comments;
mod replay_tokens;
mod validate;

pub use record::{batch_doc_clones, is_independent_batch, live_ops_for_command, live_ops_for_independent};
pub use replay::{replay_live_patch, LivePatchReport, LivePatchState, TouchKind};

use crate::{
    Comment, CommentAnchor, CommentThread, ExternalAsset, InProgress, LibraryRef, Node, NodeId, PublishInfo, Style, StyleMeta, Variable, VariableCollection,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// Format version written in every [`LivePatch`] and [`DocEpoch`].
///
/// Version 2 is the v0.6 log: the `variables`, `styles`, `library_state`, `images` and `set_in_progress`
/// ops and the `label`, `session_id` and `agent_label` fields. A build reads every version from
/// [`MIN_LIVE_PATCH_VERSION`] up to this one and refuses a newer one ([`log_version_refusal`]).
pub const LIVE_PATCH_VERSION: u32 = 2;

/// Oldest format version this build still reads.
pub const MIN_LIVE_PATCH_VERSION: u32 = 1;

/// What to do about a log written by a newer build: Studio and `designos-mcp` must be the same version.
pub const UPDATE_BOTH_HINT: &str = "Update Studio and designos-mcp to the same version.";

/// Why this build cannot read a log line written in format version `v`, or `None` when it can.
/// A newer log means Studio and `designos-mcp` are different builds, so the text names the fix.
pub fn log_version_refusal(v: u32) -> Option<String> {
    if v > LIVE_PATCH_VERSION {
        Some(format!("Log version {v} is newer than this build reads. {UPDATE_BOTH_HINT}"))
    } else if v < MIN_LIVE_PATCH_VERSION {
        Some(format!("Log version {v} is not supported."))
    } else {
        None
    }
}

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
    /// Optional agent session / run identifier (`#682`, `#572`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Optional human-readable agent label (`Claude`, `Codex`, `Grok`, `Antigravity`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_label: Option<String>,
    pub ops: Vec<LiveOp>,
}

impl LivePatch {
    pub fn new(ops: Vec<LiveOp>) -> Self {
        Self { v: LIVE_PATCH_VERSION, base: None, label: None, session_id: None, agent_label: None, ops }
    }

    pub fn with_base(mut self, base: Option<u64>) -> Self {
        self.base = base;
        self
    }

    pub fn with_label(mut self, label: Option<String>) -> Self {
        self.label = label;
        self
    }

    pub fn with_agent(mut self, session_id: impl Into<String>, agent_label: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self.agent_label = Some(agent_label.into());
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
    /// An edit written in a format version newer than this build reads. Its ops are not parsed: a
    /// newer writer may use ops this build has never heard of, and the reader must still say "newer"
    /// instead of "unreadable". Never replay it; report [`log_version_refusal`].
    Newer {
        v: u32,
    },
}

impl LiveLogLine {
    /// Reads one log line. A line holding `doc_epoch` is a document switch, any other an edit. An
    /// edit in a newer format version is named, not parsed. A switch always parses, so a reader
    /// that cannot honour it still stops following the agent's previous document.
    pub fn parse(line: &[u8]) -> Result<Self, serde_json::Error> {
        #[derive(Deserialize)]
        struct Probe {
            #[serde(default)]
            v: Option<u32>,
            #[serde(default)]
            doc_epoch: Option<u64>,
        }
        let probe = serde_json::from_slice::<Probe>(line)?;
        if probe.doc_epoch.is_some() {
            serde_json::from_slice(line).map(Self::DocEpoch)
        } else if let Some(v) = probe.v.filter(|v| *v > LIVE_PATCH_VERSION) {
            Ok(Self::Newer { v })
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
    /// Variables and collections the agent added, replaced or removed, by id. The agent's copy can
    /// lack variables the editor has (a scoped canvas), so the editor merges these into its own
    /// maps instead of taking the agent's maps whole.
    Variables {
        #[serde(default, skip_serializing_if = "Vec::is_empty", alias = "collections_set")]
        collections: Vec<VariableCollection>,
        #[serde(default, skip_serializing_if = "Vec::is_empty", alias = "variables_set")]
        variables: Vec<Variable>,
        #[serde(default, skip_serializing_if = "Vec::is_empty", alias = "collections_removed")]
        removed_collections: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty", alias = "variables_removed")]
        removed_variables: Vec<String>,
    },
    /// Shared styles and their metadata the agent added, replaced or removed, by style key.
    Styles {
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty", alias = "upserted")]
        styles_set: BTreeMap<String, Style>,
        #[serde(default, skip_serializing_if = "Vec::is_empty", alias = "removed")]
        styles_removed: Vec<String>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty", alias = "upserted_meta")]
        meta_set: BTreeMap<String, StyleMeta>,
        #[serde(default, skip_serializing_if = "Vec::is_empty", alias = "removed_meta")]
        meta_removed: Vec<String>,
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
    /// Sets or clears a transient work-in-progress flag on `node` without creating a [`Command`](crate::Command)
    /// or undo step (`E15-7` / `#592`).
    SetInProgress {
        #[serde(alias = "id")]
        node: NodeId,
        #[serde(default, alias = "value", skip_serializing_if = "Option::is_none")]
        state: Option<InProgress>,
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
#[cfg(test)]
mod version_tests;
