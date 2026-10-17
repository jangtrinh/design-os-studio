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

pub use record::live_ops_for_command;
pub use replay::{replay_live_patch, LivePatchReport, LivePatchState};

use crate::{Comment, CommentAnchor, CommentThread, Node, NodeId};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

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
    pub ops: Vec<LiveOp>,
}

impl LivePatch {
    pub fn new(ops: Vec<LiveOp>) -> Self {
        Self { v: LIVE_PATCH_VERSION, base: None, ops }
    }

    pub fn with_base(mut self, base: Option<u64>) -> Self {
        self.base = base;
        self
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
mod tests;
