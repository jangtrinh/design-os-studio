//! Structural repair for documents deserialised from external bytes.
//!
//! A hand-edited or corrupted file can carry cyclic or shared children, dangling ids, a stale
//! id counter or an absurdly deep tree. Those make `world_transform` / `page_of` spin and the
//! recursive walkers overflow the stack, so every load path repairs the tree once, up front.
//! Repair never deletes a node: anything that cannot stay in its tree is detached as a root.

use crate::table::{apply_relayout, Grid};
use crate::{
    default_cell_text, Document, Node, NodeId, NodeKind, TableCellProps, TableProps, TableTrack, TABLE_DEFAULT_COLUMN_WIDTH, TABLE_DEFAULT_ROW_HEIGHT,
    TABLE_MIN_TRACK,
};
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
    /// Connectors that sat in an auto-layout frame as part of its flow and are now positioned
    /// absolutely, as every connector made since connectors joined design files is. Not a
    /// node-graph defect, so [`RepairReport::structural_summary`] leaves it out: nothing was
    /// guessed or lost, and a caller that refuses repaired graphs still opens the file.
    pub connectors_made_absolute: usize,
    /// What the table pass fixed. Left out of [`RepairReport::structural_summary`] like the two counts above:
    /// every fix keeps the text it finds (a cell outside a table becomes a text layer, a duplicate or covered
    /// cell folds into the cell that stays), so a caller that refuses repaired graphs still opens the file.
    pub tables: TableRepairs,
    /// Motion tracks with an empty property name, dropped on load (no other track is touched). Left out of
    /// [`RepairReport::structural_summary`]: nothing in the node graph was guessed, and a caller that refuses
    /// repaired graphs still opens the file.
    pub motion_tracks_dropped: usize,
    /// Layers inside a nested component copy that linked to the layer of the innermost main instead of the layer one
    /// level up (how an earlier release read a Studio-written `.fig`), relinked on load so that a change made on them
    /// survives an edit of a main component. Left out of [`RepairReport::structural_summary`]: no layer, value or flag
    /// changed, only which layer a copy layer follows, so a caller that refuses repaired graphs still opens the file.
    /// Also left out of [`RepairReport::is_clean`], so it never reads as damage.
    pub copy_links_relinked: usize,
}

/// What the table pass of [`Document::validate_and_repair`] changed; all zero for well-formed tables.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TableRepairs {
    /// Cells whose parent is not a table, now text layers with the same text.
    pub cells_to_text: usize,
    /// Rows or columns added to a table that had none of one kind.
    pub tracks_added: usize,
    /// Rows or columns given a new id because another track of the same table already used theirs.
    pub tracks_reidentified: usize,
    /// Track sizes that were not finite or not above zero, now the minimum size.
    pub track_sizes_clamped: usize,
    /// Duplicate cells at one position, folded into the first one in child order.
    pub duplicate_cells_folded: usize,
    /// Cells with a row or column the table does not have that kept their text and moved to a free position.
    pub cells_relocated: usize,
    /// Cells with a row or column the table does not have and no text, removed.
    pub cells_removed_empty: usize,
    /// Empty cells added where a live position had none.
    pub cells_added: usize,
    /// Merges dropped (unknown anchor, overlap) or clamped (span past the table edge).
    pub merges_fixed: usize,
    /// Cells inside a merged block, folded into its anchor.
    pub covered_cells_folded: usize,
    /// Children of a table that are not cells (a rectangle, a nested table), moved to the table's parent, just after it.
    pub children_moved_out: usize,
}

impl TableRepairs {
    fn total(&self) -> usize {
        self.cells_to_text
            + self.tracks_added
            + self.tracks_reidentified
            + self.track_sizes_clamped
            + self.duplicate_cells_folded
            + self.cells_relocated
            + self.cells_removed_empty
            + self.cells_added
            + self.merges_fixed
            + self.covered_cells_folded
            + self.children_moved_out
    }
}

impl RepairReport {
    /// True when the load changed nothing a person would call damage. [`RepairReport::copy_links_relinked`] does not
    /// count: it only moves which layer a copy layer follows, so a healthy file saved by an earlier release is still
    /// clean after it (callers that print "repaired a damaged document" stay quiet for it).
    pub fn is_clean(&self) -> bool {
        Self { copy_links_relinked: 0, ..*self } == Self::default()
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
            if let Some(m) = node.motion.as_deref_mut() {
                let before = m.tracks.len();
                m.tracks.retain(|t| !t.property.kiwi_name().is_empty());
                report.motion_tracks_dropped += before - m.tracks.len();
                if m.is_empty() {
                    node.motion = None;
                }
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
        report.tables = self.repair_tables();
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
            let parent_is_auto_layout = node.kind.frame_props().is_some_and(|fp| fp.auto_layout.is_some());
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
                    if parent_is_auto_layout && matches!(c.kind, crate::NodeKind::Connector(_)) && !c.layout_child.absolute {
                        c.layout_child.absolute = true;
                        report.connectors_made_absolute += 1;
                    }
                }
            }
            self.nodes.get_mut(&id).expect("claimed node exists").children = kept;
        }
    }

    /// Table pass of [`Self::validate_and_repair`]: a cell outside a table becomes a text layer, and
    /// each table is made whole again (see [`repair_table`]). Idempotent. A table is laid out again
    /// only when a fix touched it, because a load has no text measurer and would drop row growth.
    fn repair_tables(&mut self) -> TableRepairs {
        let mut rep = TableRepairs::default();
        let in_table =
            |nodes: &BTreeMap<NodeId, Node>, parent: Option<NodeId>| parent.and_then(|p| nodes.get(&p)).is_some_and(|p| matches!(p.kind, NodeKind::Table(_)));
        let stray: Vec<NodeId> =
            self.nodes.values().filter(|n| matches!(n.kind, NodeKind::TableCell(_)) && !in_table(&self.nodes, n.parent)).map(|n| n.id).collect();
        for id in stray {
            let n = self.nodes.get_mut(&id).expect("listed above");
            if let NodeKind::TableCell(c) = std::mem::replace(&mut n.kind, NodeKind::Group) {
                n.kind = NodeKind::Text(c.text);
                rep.cells_to_text += 1;
            }
        }
        let tables: Vec<NodeId> = self.nodes.values().filter(|n| matches!(n.kind, NodeKind::Table(_))).map(|n| n.id).collect();
        for id in tables {
            let before = rep.total();
            repair_table(self, id, &mut rep);
            if rep.total() != before {
                apply_relayout(self, id);
            }
        }
        rep
    }
}

/// Makes one table whole: positive finite track sizes, unique track ids, at least one row and one
/// column, merges that fit, and exactly one cell at every live position. Cell text is never dropped.
fn repair_table(doc: &mut Document, id: NodeId, rep: &mut TableRepairs) {
    let Some(NodeKind::Table(original)) = doc.nodes.get(&id).map(|n| &n.kind) else { return };
    let mut props: TableProps = original.clone();
    for t in props.rows.iter_mut().chain(props.cols.iter_mut()) {
        if !(t.size.is_finite() && t.size > 0.0) {
            t.size = TABLE_MIN_TRACK;
            rep.track_sizes_clamped += 1;
        }
    }
    for rows in [true, false] {
        let mut seen = HashSet::new();
        for i in 0..if rows { props.rows.len() } else { props.cols.len() } {
            let track_id = if rows { props.rows[i].id } else { props.cols[i].id };
            if !seen.insert(track_id) {
                let fresh = props.alloc_track();
                seen.insert(fresh);
                if rows {
                    props.rows[i].id = fresh;
                } else {
                    props.cols[i].id = fresh;
                }
                rep.tracks_reidentified += 1;
            }
        }
    }
    if props.rows.is_empty() {
        let track_id = props.alloc_track();
        props.rows.push(TableTrack { id: track_id, size: TABLE_DEFAULT_ROW_HEIGHT });
        rep.tracks_added += 1;
    }
    if props.cols.is_empty() {
        let track_id = props.alloc_track();
        props.cols.push(TableTrack { id: track_id, size: TABLE_DEFAULT_COLUMN_WIDTH });
        rep.tracks_added += 1;
    }
    let grid = Grid::new(&props);
    if grid.fixed > 0 {
        rep.merges_fixed += grid.fixed;
        props.merges = grid.merges.clone();
    }

    let children = doc.nodes[&id].children.clone();
    let mut kept: BTreeMap<(usize, usize), NodeId> = BTreeMap::new();
    let mut folds: Vec<(NodeId, (usize, usize))> = Vec::new();
    let mut unknown: Vec<NodeId> = Vec::new();
    let mut foreign: Vec<NodeId> = Vec::new();
    for cid in children {
        let Some(NodeKind::TableCell(c)) = doc.nodes.get(&cid).map(|n| &n.kind) else {
            foreign.push(cid);
            continue;
        };
        match grid.position_of(c) {
            None => unknown.push(cid),
            Some(pos) => {
                if let Some(anchor) = grid.covering_anchor(pos) {
                    folds.push((cid, anchor));
                    rep.covered_cells_folded += 1;
                } else if let std::collections::btree_map::Entry::Vacant(slot) = kept.entry(pos) {
                    slot.insert(cid);
                } else {
                    folds.push((cid, pos));
                    rep.duplicate_cells_folded += 1;
                }
            }
        }
    }
    let mut removed: Vec<NodeId> = Vec::new();
    for cid in unknown {
        let Some(NodeKind::TableCell(c)) = doc.nodes.get_mut(&cid).map(|n| &mut n.kind) else { continue };
        if c.text.content.is_empty() {
            removed.push(cid);
            rep.cells_removed_empty += 1;
        } else if let Some(pos) = grid.live().find(|p| !kept.contains_key(p)) {
            c.row = props.rows[pos.0].id;
            c.col = props.cols[pos.1].id;
            kept.insert(pos, cid);
            rep.cells_relocated += 1;
        } else {
            folds.push((cid, (0, 0)));
            rep.cells_relocated += 1;
        }
    }
    for pos in grid.live().filter(|p| !kept.contains_key(p)).collect::<Vec<_>>() {
        let cell_id = doc.alloc_id();
        let text = default_cell_text("");
        let mut cell = Node::new(cell_id, NodeKind::TableCell(TableCellProps { row: props.rows[pos.0].id, col: props.cols[pos.1].id, text }));
        cell.name = cell.kind.default_name().to_string();
        cell.parent = Some(id);
        doc.nodes.insert(cell_id, cell);
        doc.nodes.get_mut(&id).expect("table exists").children.push(cell_id);
        kept.insert(pos, cell_id);
        rep.cells_added += 1;
    }
    for (cid, target) in folds {
        let moved = match doc.nodes.get_mut(&cid).map(|n| &mut n.kind) {
            Some(NodeKind::TableCell(c)) => std::mem::take(&mut c.text.content),
            _ => continue,
        };
        if !moved.is_empty() {
            if let Some(NodeKind::TableCell(t)) = kept.get(&target).and_then(|k| doc.nodes.get_mut(k)).map(|n| &mut n.kind) {
                if !t.text.content.is_empty() {
                    t.text.content.push('\n');
                }
                t.text.content.push_str(&moved);
            }
        }
        removed.push(cid);
    }
    for cid in removed {
        let _ = doc.remove(cid);
    }
    // Anything that is not a cell moves to the table's parent, directly after the table.
    // A moved child keeps its place on the canvas: the table's own transform joins the child's.
    if let Some(parent) = doc.nodes[&id].parent {
        let table_transform = doc.nodes[&id].transform;
        for (n, cid) in foreign.into_iter().enumerate() {
            let at = doc.nodes[&parent].children.iter().position(|c| *c == id).map_or(usize::MAX, |i| i + 1 + n);
            if doc.reparent(cid, parent, at).is_ok() {
                if let Some(child) = doc.nodes.get_mut(&cid) {
                    child.transform = table_transform * child.transform;
                }
                rep.children_moved_out += 1;
            }
        }
    }
    if let Some(NodeKind::Table(p)) = doc.nodes.get_mut(&id).map(|n| &mut n.kind) {
        *p = props;
    }
}
