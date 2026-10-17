//! Structural repair for documents deserialised from external bytes.
//!
//! A hand-edited or corrupted file can carry cyclic or shared children, dangling ids, a stale
//! id counter or an absurdly deep tree. Those make `world_transform` / `page_of` spin and the
//! recursive walkers overflow the stack, so every load path repairs the tree once, up front.
//! Repair never deletes a node: anything that cannot stay in its tree is detached as a root.

use crate::{Document, NodeId};
use std::collections::{BTreeMap, BTreeSet, HashSet};

/// Deepest nesting kept below a root; deeper subtrees are detached (recursive walkers stay bounded).
pub const MAX_TREE_DEPTH: usize = 512;

/// Node ids must stay at least this far below `u64::MAX`, so a document always has room to
/// allocate new ids above them; a file that leaves less is treated as damaged.
pub const MIN_ID_HEADROOM: u64 = 1 << 32;

/// What [`Document::validate_and_repair`] changed; all zero for a well-formed document.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RepairReport {
    /// Child, page or library-page ids that pointed at no node.
    pub dangling_ids: usize,
    /// Children entries dropped because the node was already claimed (cycle, self-child, duplicate).
    pub duplicate_claims: usize,
    /// Subtrees detached because they nested deeper than [`MAX_TREE_DEPTH`].
    pub depth_detached: usize,
    /// Nodes whose `parent` field was rewritten from the children lists.
    pub parents_rebuilt: usize,
    /// Nodes whose stored id differed from their map key.
    pub ids_realigned: usize,
    /// The id counter was behind the largest node id.
    pub next_id_raised: bool,
    /// A node id sits within [`MIN_ID_HEADROOM`] of `u64::MAX`, so ids cannot keep being allocated above it.
    pub id_space_exhausted: bool,
    /// Numbers stored as `null` (how older versions saved a NaN or infinity) that were read as
    /// their neutral value; see [`crate::finite_json`]. Not a node-graph defect, so
    /// [`RepairReport::structural_summary`] leaves it out.
    pub non_finite_numbers: usize,
}

impl RepairReport {
    pub fn is_clean(&self) -> bool {
        *self == Self::default()
    }

    /// Names the node-graph defects that were repaired (dangling ids, cycles or duplicate claims,
    /// over-deep nesting, wrong parent links, mismatched node ids), or `None` when there were none.
    /// A merely stale id counter is not a graph defect: the repair raises it and nothing is lost.
    ///
    /// [`RepairReport::non_finite_numbers`] is left out on purpose. A caller that refuses a
    /// repaired document does so because the repair guessed at structure it could not know (which
    /// parent, which duplicate). A number saved as `null` has no data to lose: the value was
    /// already NaN or infinite when it was written, and it reads back as its neutral value. Refusing
    /// would lock agents out of every file an older release saved with one, so those callers still
    /// open the file and report the count to the user instead.
    pub fn structural_summary(&self) -> Option<String> {
        let mut parts: Vec<String> = [
            ("dangling ids", self.dangling_ids),
            ("duplicate or cyclic child claims", self.duplicate_claims),
            ("subtrees nested too deep", self.depth_detached),
            ("wrong parent links", self.parents_rebuilt),
            ("mismatched node ids", self.ids_realigned),
        ]
        .into_iter()
        .filter(|(_, n)| *n > 0)
        .map(|(what, n)| format!("{n} {what}"))
        .collect();
        if self.id_space_exhausted {
            parts.push("node ids too close to the maximum to allocate new ones".to_string());
        }
        (!parts.is_empty()).then(|| parts.join(", "))
    }
}

impl Document {
    /// Makes the node tree satisfy its invariants: every referenced id exists, `parent` mirrors
    /// the children lists, each node has one parent, no cycles, bounded depth, and `alloc_id`
    /// hands out fresh ids. Idempotent; a well-formed document is left untouched.
    pub fn validate_and_repair(&mut self) -> RepairReport {
        let mut report = RepairReport::default();

        for (key, node) in self.nodes.iter_mut() {
            if node.id != *key {
                node.id = *key;
                report.ids_realigned += 1;
            }
        }

        let mut seen_pages = BTreeSet::new();
        let before = self.pages.len();
        let nodes = &self.nodes;
        self.pages.retain(|p| nodes.contains_key(p) && seen_pages.insert(*p));
        report.dangling_ids += before - self.pages.len();
        if self.library_page.is_some_and(|lp| !self.nodes.contains_key(&lp)) {
            self.library_page = None;
            report.dangling_ids += 1;
        }

        // Every page and the library page is a root. Mark all of them claimed before walking any
        // subtree, so a page listed under another page's node is dropped as a duplicate claim
        // instead of becoming that node's child.
        let mut visited = HashSet::new();
        let mut roots: Vec<NodeId> = self.pages.clone();
        roots.extend(self.library_page);
        roots.retain(|root| visited.insert(*root));
        for root in roots {
            self.claim_subtree(root, &mut visited, &mut report);
        }

        // Nodes no page reaches: real orphans first, then whatever is left (cycles, depth casualties).
        let unvisited = |visited: &HashSet<NodeId>, nodes: &BTreeMap<NodeId, crate::Node>, orphans: bool| -> Vec<NodeId> {
            nodes.iter().filter(|(id, n)| !visited.contains(*id) && orphans == n.parent.is_none_or(|p| !nodes.contains_key(&p))).map(|(id, _)| *id).collect()
        };
        for orphans in [true, false] {
            for id in unvisited(&visited, &self.nodes, orphans) {
                if visited.insert(id) {
                    self.claim_subtree(id, &mut visited, &mut report);
                }
            }
        }

        let largest = self.nodes.keys().next_back().map_or(0, |id| id.0);
        if self.next_id < largest {
            self.next_id = largest;
            report.next_id_raised = true;
        }
        report.id_space_exhausted = largest > u64::MAX - MIN_ID_HEADROOM;
        if !report.is_clean() {
            self.mark_full_dirty();
        }
        report
    }

    /// Claims everything below `root` (already marked in `visited` by the caller), depth-first and
    /// without recursion. A child already claimed elsewhere is dropped from its list; the first
    /// claim wins.
    fn claim_subtree(&mut self, root: NodeId, visited: &mut HashSet<NodeId>, report: &mut RepairReport) {
        if let Some(n) = self.nodes.get_mut(&root) {
            if n.parent.take().is_some() {
                report.parents_rebuilt += 1;
            }
        }
        let mut stack = vec![(root, 0usize)];
        while let Some((id, depth)) = stack.pop() {
            let Some(node) = self.nodes.get_mut(&id) else { continue };
            let listed = std::mem::take(&mut node.children);
            let mut kept = Vec::with_capacity(listed.len());
            for child in listed {
                if !self.nodes.contains_key(&child) {
                    report.dangling_ids += 1;
                } else if visited.contains(&child) {
                    report.duplicate_claims += 1;
                } else if depth + 1 > MAX_TREE_DEPTH {
                    report.depth_detached += 1;
                } else {
                    visited.insert(child);
                    kept.push(child);
                    stack.push((child, depth + 1));
                    let c = self.nodes.get_mut(&child).expect("checked above");
                    if c.parent != Some(id) {
                        c.parent = Some(id);
                        report.parents_rebuilt += 1;
                    }
                }
            }
            self.nodes.get_mut(&id).expect("claimed node exists").children = kept;
        }
    }
}
