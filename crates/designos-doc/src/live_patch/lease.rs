//! Frame-ownership leases and write-guard verification shared by `designos-mcp` and Studio (`RD-3` / `#572`).

use super::LiveOp;
use crate::{Command, Document, NodeId, NodeKind};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Plugin-data key stamped on every node created or updated by an agent run (`RD-3 §3.4`).
pub const PLUGIN_DATA_RUN_KEY: &str = "designos:run";

/// Why a frame lease was granted (`RD-3 §3.1`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeasePurpose {
    #[default]
    Split,
    SideBySide,
    Iterate,
    Repair,
    Variant,
}

impl LeasePurpose {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Split => "split",
            Self::SideBySide => "side_by_side",
            Self::Iterate => "iterate",
            Self::Repair => "repair",
            Self::Variant => "variant",
        }
    }
}

/// Exclusive ownership lease binding one agent run (`run_id`) to one or more root frames (`RD-3 §3.1`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lease {
    pub run_id: String,
    #[serde(default)]
    pub frames: Vec<NodeId>,
    #[serde(default)]
    pub purpose: LeasePurpose,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lead_run_id: Option<String>,
    #[serde(default)]
    pub allow_shared: bool,
    #[serde(default)]
    pub granted_ms: u64,
}

impl Lease {
    pub fn new(run_id: impl Into<String>, frames: Vec<NodeId>, purpose: LeasePurpose) -> Self {
        Self { run_id: run_id.into(), frames, purpose, lead_run_id: None, allow_shared: false, granted_ms: 0 }
    }

    pub fn with_lead(mut self, lead_run_id: Option<String>) -> Self {
        self.lead_run_id = lead_run_id.filter(|s| !s.trim().is_empty());
        self
    }

    pub fn with_allow_shared(mut self, allow_shared: bool) -> Self {
        self.allow_shared = allow_shared;
        self
    }

    pub fn with_granted_ms(mut self, granted_ms: u64) -> Self {
        self.granted_ms = granted_ms;
        self
    }
}

/// Nodes and shared document state touched by a mutating MCP call or [`LiveOp`] (`RD-3 §3.2`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AffectedSet {
    /// Every node updated, deleted, or moved (including old and new parents), and the parent of every insert.
    pub nodes: Vec<NodeId>,
    /// JSON Pointer paths of document-level shared writes (`variables`, `styles`).
    pub shared_paths: Vec<String>,
}

impl AffectedSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn for_nodes(nodes: impl IntoIterator<Item = NodeId>) -> Self {
        let mut out = Vec::new();
        for id in nodes {
            if id.0 > 0 && !out.contains(&id) {
                out.push(id);
            }
        }
        Self { nodes: out, shared_paths: Vec::new() }
    }

    pub fn add_node(&mut self, id: NodeId) {
        if id.0 > 0 && !self.nodes.contains(&id) {
            self.nodes.push(id);
        }
    }

    pub fn add_shared_path(&mut self, path: impl Into<String>) {
        let p = path.into();
        if !self.shared_paths.contains(&p) {
            self.shared_paths.push(p);
        }
    }

    /// Computes the affected set of a [`LiveOp`] during Studio live-patch replay (`RD-3 §3.2`).
    pub fn for_live_op(doc: &Document, op: &LiveOp, resolve: impl Fn(NodeId) -> NodeId) -> Self {
        match op {
            LiveOp::Insert { parent, .. } => Self::for_nodes([resolve(*parent)]),
            LiveOp::Update { id, .. } => Self::for_nodes([resolve(*id)]),
            LiveOp::Reparent { id, parent, .. } => {
                let target_id = resolve(*id);
                let target_parent = resolve(*parent);
                let old_parent = doc.get(target_id).and_then(|n| n.parent);
                Self::for_nodes([Some(target_id), old_parent, Some(target_parent)].into_iter().flatten())
            }
            LiveOp::Remove { id } => {
                let target_id = resolve(*id);
                let old_parent = doc.get(target_id).and_then(|n| n.parent);
                Self::for_nodes([Some(target_id), old_parent].into_iter().flatten())
            }
            LiveOp::Variables { .. } => {
                let mut s = Self::new();
                s.add_shared_path("/variables");
                s
            }
            LiveOp::Styles { .. } => {
                let mut s = Self::new();
                s.add_shared_path("/styles");
                s
            }
            LiveOp::Comments { .. } | LiveOp::LibraryState { .. } | LiveOp::Images { .. } => Self::new(),
        }
    }

    /// Computes the affected set of a [`Command`].
    pub fn for_command(doc: &Document, cmd: &Command) -> Self {
        let mut set = Self::new();
        Self::collect_command(doc, cmd, &mut set);
        set
    }

    fn collect_command(doc: &Document, cmd: &Command, out: &mut Self) {
        match cmd {
            Command::Insert { parent, .. } => out.add_node(*parent),
            Command::Update { node } => out.add_node(node.id),
            Command::Remove { id } => {
                out.add_node(*id);
                if let Some(p) = doc.get(*id).and_then(|n| n.parent) {
                    out.add_node(p);
                }
            }
            Command::Reparent { id, parent, .. } => {
                out.add_node(*id);
                if let Some(old_p) = doc.get(*id).and_then(|n| n.parent) {
                    out.add_node(old_p);
                }
                out.add_node(*parent);
            }
            Command::SetVariables { .. } => out.add_shared_path("/variables"),
            Command::SetStyles { .. } => out.add_shared_path("/styles"),
            Command::Batch(cmds) => {
                for c in cmds {
                    Self::collect_command(doc, c, out);
                }
            }
            _ => {}
        }
    }
}

/// Violation returned by [`LeaseTable::allows`] or [`LeaseTable::grant`] (`RD-3 §3.2`, `§5`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LeaseViolation {
    OutsideOwnedFrames { run_id: String, node_id: Option<NodeId>, owner_run_id: Option<String>, valid_frames: Vec<NodeId>, path: String },
    FrameLeased { frame_id: NodeId, owner_run_id: String },
}

impl LeaseViolation {
    pub fn code(&self) -> &'static str {
        match self {
            Self::OutsideOwnedFrames { .. } => "OUTSIDE_OWNED_FRAMES",
            Self::FrameLeased { .. } => "FRAME_LEASED",
        }
    }

    pub fn valid_frames(&self) -> &[NodeId] {
        match self {
            Self::OutsideOwnedFrames { valid_frames, .. } => valid_frames,
            Self::FrameLeased { .. } => &[],
        }
    }

    pub fn path(&self) -> &str {
        match self {
            Self::OutsideOwnedFrames { path, .. } => path,
            Self::FrameLeased { .. } => "/frames",
        }
    }
}

impl fmt::Display for LeaseViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutsideOwnedFrames { run_id, node_id, owner_run_id, valid_frames, path } => {
                let valid: Vec<u64> = valid_frames.iter().map(|n| n.0).collect();
                match (node_id, owner_run_id) {
                    (Some(nid), Some(owner)) => write!(
                        f,
                        "OUTSIDE_OWNED_FRAMES: run '{run_id}' cannot edit node {} (leased to run '{owner}'); valid frames: {valid:?}",
                        nid.0
                    ),
                    (Some(nid), None) => write!(
                        f,
                        "OUTSIDE_OWNED_FRAMES: run '{run_id}' cannot edit node {} outside its owned frames {valid:?}",
                        nid.0
                    ),
                    (None, _) => write!(
                        f,
                        "OUTSIDE_OWNED_FRAMES: run '{run_id}' cannot write shared document state at '{path}' without split lease allow_shared=true; valid frames: {valid:?}"
                    ),
                }
            }
            Self::FrameLeased { frame_id, owner_run_id } => {
                write!(f, "FRAME_LEASED: frame {} is already leased to active run '{owner_run_id}'", frame_id.0)
            }
        }
    }
}

/// True when `id` is a root frame or top-level container eligible for a frame lease (`E15-7` / `RD-3 §3.1`).
pub fn is_root_frame(doc: &Document, id: NodeId) -> bool {
    let Some(node) = doc.get(id) else { return false };
    let is_eligible_kind =
        matches!(node.kind, NodeKind::Frame(_) | NodeKind::Section | NodeKind::Component(_) | NodeKind::ComponentSet(_) | NodeKind::Instance { .. });
    if !is_eligible_kind {
        return false;
    }
    let Some(parent_id) = node.parent else { return false };
    let Some(parent) = doc.get(parent_id) else { return false };
    match parent.kind {
        NodeKind::Page { .. } => true,
        NodeKind::Section => parent.parent.and_then(|gp| doc.get(gp)).is_some_and(|gn| matches!(gn.kind, NodeKind::Page { .. })),
        _ => false,
    }
}

/// Returns the root frame containing `id` (or `Some(id)` when `id` itself is a root frame).
pub fn root_frame_of(doc: &Document, id: NodeId) -> Option<NodeId> {
    let mut cur = Some(id);
    let mut depth = 0usize;
    while let Some(cid) = cur {
        if depth > 256 {
            break;
        }
        depth += 1;
        if is_root_frame(doc, cid) {
            return Some(cid);
        }
        cur = doc.get(cid).and_then(|n| n.parent);
    }
    None
}

/// True when `node_id` is `frame_id` or a descendant of `frame_id` in `doc`.
pub fn is_within_frames(doc: &Document, node_id: NodeId, frames: &[NodeId]) -> bool {
    if frames.is_empty() {
        return false;
    }
    let mut cur = Some(node_id);
    let mut depth = 0usize;
    while let Some(cid) = cur {
        if depth > 256 {
            break;
        }
        depth += 1;
        if frames.contains(&cid) {
            return true;
        }
        cur = doc.get(cid).and_then(|n| n.parent);
    }
    false
}

/// Stamps `plugin_data["designos:run"] = run_id` on every node inserted or updated by `cmd` (`RD-3 §3.4`).
pub fn stamp_command_authorship(cmd: &mut Command, run_id: &str) {
    let clean = run_id.trim();
    if clean.is_empty() {
        return;
    }
    match cmd {
        Command::Insert { nodes, .. } | Command::InsertPage { nodes, .. } => {
            for n in nodes {
                n.plugin_data.insert(PLUGIN_DATA_RUN_KEY.to_string(), clean.to_string());
            }
        }
        Command::Update { node } => {
            node.plugin_data.insert(PLUGIN_DATA_RUN_KEY.to_string(), clean.to_string());
        }
        Command::Batch(cmds) => {
            for c in cmds {
                stamp_command_authorship(c, clean);
            }
        }
        _ => {}
    }
}

/// Session lease table written to `<session_dir>/leases.json` by Studio and checked by both the
/// MCP write guard and Studio's replay guard (`RD-3 §3.1`, `§3.2`, `§4`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaseTable {
    #[serde(default)]
    pub leases: Vec<Lease>,
}

impl LeaseTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_leases(leases: Vec<Lease>) -> Self {
        Self { leases }
    }

    pub fn is_empty(&self) -> bool {
        self.leases.is_empty()
    }

    /// Returns the lease held by `run_id`, if any.
    pub fn get(&self, run_id: &str) -> Option<&Lease> {
        self.leases.iter().find(|l| l.run_id == run_id)
    }

    /// Returns the lease that owns `frame_id`, if any.
    pub fn owner_of_frame(&self, frame_id: NodeId) -> Option<&Lease> {
        self.leases.iter().find(|l| l.frames.contains(&frame_id))
    }

    /// Returns the `(Lease, frame_id)` owning `node_id` (or an ancestor frame of `node_id`), if any.
    pub fn owner_of_node(&self, doc: &Document, node_id: NodeId) -> Option<(&Lease, NodeId)> {
        let mut cur = Some(node_id);
        let mut depth = 0usize;
        while let Some(cid) = cur {
            if depth > 256 {
                break;
            }
            depth += 1;
            if let Some(lease) = self.owner_of_frame(cid) {
                return Some((lease, cid));
            }
            cur = doc.get(cid).and_then(|n| n.parent);
        }
        None
    }

    /// Grants or replaces the lease for `lease.run_id`. Fails with [`LeaseViolation::FrameLeased`]
    /// if any requested frame is already leased to another run (`RD-3 §3.1`, `§5`).
    pub fn grant(&mut self, lease: Lease) -> Result<(), LeaseViolation> {
        for &fid in &lease.frames {
            if let Some(existing) = self.leases.iter().find(|l| l.run_id != lease.run_id && l.frames.contains(&fid)) {
                return Err(LeaseViolation::FrameLeased { frame_id: fid, owner_run_id: existing.run_id.clone() });
            }
        }
        if let Some(slot) = self.leases.iter_mut().find(|l| l.run_id == lease.run_id) {
            *slot = lease;
        } else {
            self.leases.push(lease);
        }
        Ok(())
    }

    /// Removes and returns the lease for `run_id`, if present.
    pub fn release(&mut self, run_id: &str) -> Option<Lease> {
        let pos = self.leases.iter().position(|l| l.run_id == run_id)?;
        Some(self.leases.remove(pos))
    }

    /// Drops frames that no longer exist in `doc`, and removes leases whose frame list becomes empty (`RD-3 §8.5`).
    pub fn prune_missing_frames(&mut self, doc: &Document) -> bool {
        let before: usize = self.leases.iter().map(|l| l.frames.len()).sum();
        let before_leases = self.leases.len();
        for lease in &mut self.leases {
            lease.frames.retain(|&fid| doc.get(fid).is_some());
        }
        self.leases.retain(|l| !l.frames.is_empty());
        let after: usize = self.leases.iter().map(|l| l.frames.len()).sum();
        before != after || before_leases != self.leases.len()
    }

    /// Root frames in `doc` that are not leased to any run.
    pub fn unleased_root_frames(&self, doc: &Document) -> Vec<NodeId> {
        let mut out = Vec::new();
        for &page_id in &doc.pages {
            let Some(page) = doc.get(page_id) else { continue };
            for &child_id in &page.children {
                if is_root_frame(doc, child_id) && self.owner_of_frame(child_id).is_none() {
                    out.push(child_id);
                }
                if let Some(sec) = doc.get(child_id).filter(|n| matches!(n.kind, NodeKind::Section)) {
                    for &grand_id in &sec.children {
                        if is_root_frame(doc, grand_id) && self.owner_of_frame(grand_id).is_none() {
                            out.push(grand_id);
                        }
                    }
                }
            }
        }
        out
    }

    /// Enforces the two-layer frame ownership rule (`RD-3 §3.2`, `§4`, `§8.1`, `§8.2`, `§8.5`):
    /// - If `run_id` holds a lease (with at least one frame still present in `doc`), every existing
    ///   node in `affected.nodes` must be a descendant-or-self of one of its frames, and any shared
    ///   document write (`variables`, `styles`) requires `purpose == Split && allow_shared`.
    /// - If `run_id` holds no lease, it may not touch any node inside a frame leased to another run.
    /// - Nodes in `affected.nodes` that do not exist in `doc` are ignored here so the caller's normal
    ///   `NODE_NOT_FOUND` validation handles deleted or nonexistent node IDs (`§8.5`).
    pub fn allows(&self, run_id: &str, doc: &Document, affected: &AffectedSet) -> Result<(), LeaseViolation> {
        let caller_lease = if !run_id.is_empty() { self.get(run_id) } else { None };
        if let Some(lease) = caller_lease {
            let active_frames: Vec<NodeId> = lease.frames.iter().copied().filter(|&f| doc.get(f).is_some()).collect();
            if let Some(shared_path) = affected.shared_paths.first() {
                if !(lease.purpose == LeasePurpose::Split && lease.allow_shared) {
                    return Err(LeaseViolation::OutsideOwnedFrames {
                        run_id: run_id.to_string(),
                        node_id: None,
                        owner_run_id: None,
                        valid_frames: active_frames,
                        path: shared_path.clone(),
                    });
                }
            }
            // If all frames originally leased to this run were deleted from `doc` (§8.5),
            // let nonexistent targets fall through to `NODE_NOT_FOUND` while still blocking other existing nodes.
            for &node_id in &affected.nodes {
                if doc.get(node_id).is_none() {
                    continue;
                }
                if !is_within_frames(doc, node_id, &active_frames) {
                    let owner = self.owner_of_node(doc, node_id).map(|(l, _)| l.run_id.clone()).filter(|r| r != run_id);
                    return Err(LeaseViolation::OutsideOwnedFrames {
                        run_id: run_id.to_string(),
                        node_id: Some(node_id),
                        owner_run_id: owner,
                        valid_frames: active_frames,
                        path: "/node_id".to_string(),
                    });
                }
            }
            return Ok(());
        }

        // Caller holds no lease (e.g., lead agent): cannot touch any node inside a frame leased to another run (§8.1).
        for &node_id in &affected.nodes {
            if doc.get(node_id).is_none() {
                continue;
            }
            for other in &self.leases {
                if !run_id.is_empty() && other.run_id == run_id {
                    continue;
                }
                let other_active: Vec<NodeId> = other.frames.iter().copied().filter(|&f| doc.get(f).is_some()).collect();
                if is_within_frames(doc, node_id, &other_active) {
                    return Err(LeaseViolation::OutsideOwnedFrames {
                        run_id: run_id.to_string(),
                        node_id: Some(node_id),
                        owner_run_id: Some(other.run_id.clone()),
                        valid_frames: self.unleased_root_frames(doc),
                        path: "/node_id".to_string(),
                    });
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live_patch::{live_ops_for_command, replay_live_patch_with_leases, LivePatch, LivePatchState, SiblingHint};
    use crate::{Command, Document, History, Node, NodeId, Rect};
    use serde_json::Map;

    fn make_three_frame_doc() -> (Document, NodeId, [NodeId; 3]) {
        let mut doc = Document::new("rd3-test");
        let page = doc.add_page("Page 1");
        let f0 = doc.alloc_id();
        let f1 = doc.alloc_id();
        let f2 = doc.alloc_id();
        doc.insert(page, Node::frame(f0, Rect::new(0.0, 0.0, 400.0, 300.0))).unwrap();
        doc.insert(page, Node::frame(f1, Rect::new(500.0, 0.0, 400.0, 300.0))).unwrap();
        doc.insert(page, Node::frame(f2, Rect::new(1000.0, 0.0, 400.0, 300.0))).unwrap();
        (doc, page, [f0, f1, f2])
    }

    #[test]
    fn replay_guard_skips_foreign_ops() {
        let (mut doc, _page, [frame_a, frame_b, _frame_c]) = make_three_frame_doc();
        let child_in_b = doc.alloc_id();
        doc.insert(frame_b, Node::rectangle(child_in_b, Rect::new(10.0, 10.0, 50.0, 50.0))).unwrap();

        let mut leases = LeaseTable::new();
        leases.grant(Lease::new("agent-a", vec![frame_a], LeasePurpose::Split)).unwrap();
        leases.grant(Lease::new("agent-b", vec![frame_b], LeasePurpose::Split)).unwrap();

        // Hand-written patch line with run_id = "agent-a" attempting to mutate a node inside frame_b.
        let mut set = Map::new();
        set.insert("name".to_string(), serde_json::json!("HackedByA"));
        let patch = LivePatch::new(vec![LiveOp::Update { id: child_in_b, set, unset: Vec::new() }])
            .with_run_id(Some("agent-a".to_string()))
            .with_label(Some("update_node".to_string()));

        let mut state = LivePatchState::default();
        let report = replay_live_patch_with_leases(&mut doc, &patch, &mut state, &leases, &mut |d, c| c.apply(d).map(|_| ())).unwrap();

        assert_eq!(report.applied, 0);
        assert_eq!(report.skipped.len(), 1);
        assert!(report.skipped[0].contains("OUTSIDE_OWNED_FRAMES"));
        assert_ne!(doc.get(child_in_b).unwrap().name, "HackedByA");
    }

    #[test]
    fn three_agents_concurrently_no_lost_edits() {
        let (mut studio_doc, _page, [f_a, f_b, f_c]) = make_three_frame_doc();
        let mut leases = LeaseTable::new();
        leases.grant(Lease::new("agent-a", vec![f_a], LeasePurpose::Split)).unwrap();
        leases.grant(Lease::new("agent-b", vec![f_b], LeasePurpose::Split)).unwrap();
        leases.grant(Lease::new("agent-c", vec![f_c], LeasePurpose::Split)).unwrap();

        // Three agent copies starting from the same initial snapshot (causing overlapping allocated NodeIds).
        let mut doc_a = studio_doc.clone();
        let mut doc_b = studio_doc.clone();
        let mut doc_c = studio_doc.clone();

        let mut interleaved_patches: Vec<LivePatch> = Vec::with_capacity(150);
        for batch_idx in 0..50 {
            for (agent_doc, frame_id, run_id) in [(&mut doc_a, f_a, "agent-a"), (&mut doc_b, f_b, "agent-b"), (&mut doc_c, f_c, "agent-c")] {
                let nid = agent_doc.alloc_id();
                let mut node = Node::rectangle(nid, Rect::new(batch_idx as f32 * 4.0, 0.0, 12.0, 12.0));
                node.name = format!("{run_id}-rect-{batch_idx}");
                let idx = agent_doc.get(frame_id).unwrap().children.len();
                let cmd = Command::Insert { parent: frame_id, index: idx, nodes: vec![node] };
                let ops = live_ops_for_command(agent_doc, &cmd).unwrap();
                cmd.apply(agent_doc).unwrap();
                interleaved_patches.push(LivePatch::new(ops).with_run_id(Some(run_id.to_string())).with_label(Some("create_node".to_string())));
            }
        }
        assert_eq!(interleaved_patches.len(), 150);

        let mut state = LivePatchState::default();
        let mut studio_hist = History::default();
        for patch in &interleaved_patches {
            let rid = patch.run_id.clone();
            let lbl = patch.label.clone();
            let report = replay_live_patch_with_leases(&mut studio_doc, patch, &mut state, &leases, &mut |d, c| {
                studio_hist.exec_for_run(d, c, rid.clone(), lbl.clone())
            })
            .unwrap();
            assert_eq!(report.skipped.len(), 0);
            assert_eq!(report.applied, 1);
        }

        assert_eq!(studio_doc.get(f_a).unwrap().children.len(), 50);
        assert_eq!(studio_doc.get(f_b).unwrap().children.len(), 50);
        assert_eq!(studio_doc.get(f_c).unwrap().children.len(), 50);
    }

    #[test]
    fn each_run_undoes_as_its_own_step() {
        let (mut studio_doc, _page, [f_a, f_b, f_c]) = make_three_frame_doc();
        let mut leases = LeaseTable::new();
        leases.grant(Lease::new("agent-a", vec![f_a], LeasePurpose::Split)).unwrap();
        leases.grant(Lease::new("agent-b", vec![f_b], LeasePurpose::Split)).unwrap();
        leases.grant(Lease::new("agent-c", vec![f_c], LeasePurpose::Split)).unwrap();

        let mut state = LivePatchState::default();
        let mut studio_hist = History::default();

        for round in 0..5 {
            for (frame_id, run_id) in [(f_a, "agent-a"), (f_b, "agent-b"), (f_c, "agent-c")] {
                let nid = studio_doc.alloc_id();
                let mut node = Node::rectangle(nid, Rect::new(round as f32 * 10.0, 0.0, 20.0, 20.0));
                node.name = format!("{run_id}-{round}");
                let patch = LivePatch::new(vec![LiveOp::Insert { parent: frame_id, at: SiblingHint::default(), nodes: vec![node] }])
                    .with_run_id(Some(run_id.to_string()))
                    .with_label(Some("create_node".to_string()));

                replay_live_patch_with_leases(&mut studio_doc, &patch, &mut state, &leases, &mut |d, c| {
                    studio_hist.exec_for_run(d, c, Some(run_id.to_string()), Some("create_node".to_string()))
                })
                .unwrap();
            }
        }

        assert_eq!(studio_doc.get(f_a).unwrap().children.len(), 5);
        assert_eq!(studio_doc.get(f_b).unwrap().children.len(), 5);
        assert_eq!(studio_doc.get(f_c).unwrap().children.len(), 5);

        // Undo run B only -> B's 5 steps are reverted; A and C stay intact.
        let reverted = studio_hist.undo_run(&mut studio_doc, "agent-b").unwrap();
        assert_eq!(reverted, 5);
        assert_eq!(studio_doc.get(f_a).unwrap().children.len(), 5);
        assert_eq!(studio_doc.get(f_b).unwrap().children.len(), 0);
        assert_eq!(studio_doc.get(f_c).unwrap().children.len(), 5);
    }

    #[test]
    fn authorship_recorded() {
        let (mut doc, _page, [frame_a, _frame_b, _frame_c]) = make_three_frame_doc();
        let mut leases = LeaseTable::new();
        leases.grant(Lease::new("agent-a", vec![frame_a], LeasePurpose::Split)).unwrap();

        let nid = doc.alloc_id();
        let node = Node::rectangle(nid, Rect::new(0.0, 0.0, 100.0, 40.0));
        let patch = LivePatch::new(vec![LiveOp::Insert { parent: frame_a, at: SiblingHint::default(), nodes: vec![node] }])
            .with_run_id(Some("agent-a".to_string()))
            .with_label(Some("create_node".to_string()));

        let mut state = LivePatchState::default();
        let report = replay_live_patch_with_leases(&mut doc, &patch, &mut state, &leases, &mut |d, c| c.apply(d).map(|_| ())).unwrap();
        assert_eq!(report.applied, 1);
        let inserted_id = report.inserted_roots[0];
        let created = doc.get(inserted_id).unwrap();
        assert_eq!(created.plugin_data.get(PLUGIN_DATA_RUN_KEY).map(String::as_str), Some("agent-a"));
    }
}
