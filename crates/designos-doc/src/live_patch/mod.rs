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

pub use record::live_ops_for_command;
pub use replay::{replay_live_patch, LivePatchReport, LivePatchState};

use crate::{CommentThread, Node, NodeId};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Format version written in every [`LivePatch`].
pub const LIVE_PATCH_VERSION: u32 = 1;

/// The edits of one agent tool call: one line of the session's `doc-patch.jsonl`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LivePatch {
    pub v: u32,
    pub ops: Vec<LiveOp>,
}

impl LivePatch {
    pub fn new(ops: Vec<LiveOp>) -> Self {
        Self { v: LIVE_PATCH_VERSION, ops }
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
    /// Comment threads added or changed (whole thread) and removed, by thread id.
    Comments {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        upsert: Vec<CommentThread>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        removed: Vec<String>,
    },
}

/// Fields that structural ops own; field updates never touch them.
pub(crate) const STRUCTURAL_FIELDS: [&str; 3] = ["id", "parent", "children"];

#[cfg(test)]
mod tests;
