//! Table edit operations: one pure function per edit, one undo step each.
//!
//! Every operation reads the document, builds the table as it should be afterwards, and returns one
//! [`Command::Batch`] that gets it there: the structural changes (tracks, merges, cell inserts and
//! removals) together with the geometry [`relayout`] derives for the new state. The caller runs the
//! command through `History::exec_user` / `exec_agent`, so an edit is exactly one undo step, and the
//! `History` position check finds the table whole at the end of it. The operations never mutate the
//! document and never allocate ids on it: new node ids come from [`Document::peek_next_id`], which
//! `Command::Insert` raises when it runs.
//!
//! A commit path outside `History` (a transaction, a live-patch replay) runs a returned command
//! through [`apply_checked`], which keeps the table-position rule for it.

use crate::table::{relayout, Grid};
use crate::*;
use std::collections::{BTreeSet, HashMap, HashSet};

/// Most rows or columns one table may have, a guard against a runaway agent call.
pub const TABLE_MAX_TRACKS: usize = 500;

/// Largest row height, column width or table side. Mirrors the `maximum` of the MCP `size` schema
/// (`designos-mcp` `engine.rs` `tools()`), so a table can never be sized beyond what an agent may ask for.
pub const TABLE_MAX_SIZE: f32 = 1_000_000.0;

/// Why a table operation was refused. Every message names the next action.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TableOpError {
    /// The node is missing or is not a table (or not a cell of this table).
    #[error("{0}")]
    NotATable(String),
    #[error("{0}")]
    IndexOutOfRange(String),
    /// The move would cut a merged block in two.
    #[error("{0}")]
    SplitsMerge(String),
    /// The merge would cut into a merged block that it does not contain.
    #[error("{0}")]
    OverlapsMerge(String),
    /// More than [`TABLE_MAX_TRACKS`] rows or columns.
    #[error("{0}")]
    TooLarge(String),
    /// `create_table` was given a parent that cannot hold a table.
    #[error("{0}")]
    BadParent(String),
    /// The table already lacks a cell for a live position, or has a cell where none belongs.
    #[error("{0}")]
    BrokenTable(String),
    /// The edit would leave the table as it is. Callers skip it instead of recording an empty undo step.
    #[error("{0}")]
    NoChange(String),
}

use TableOpError as E;

/// How a new table looks. `Default` is the plain table the model constants describe.
#[derive(Clone, Debug, PartialEq)]
pub struct TableStyle {
    pub column_width: f32,
    pub row_height: f32,
    /// Fills of the table node (its background).
    pub fills: Vec<Paint>,
    /// Grid line colour and weight, kept on the table node.
    pub stroke: Option<Stroke>,
    /// Fills every new cell starts with.
    pub cell_fills: Vec<Paint>,
}

impl Default for TableStyle {
    fn default() -> Self {
        Self { column_width: TABLE_DEFAULT_COLUMN_WIDTH, row_height: TABLE_DEFAULT_ROW_HEIGHT, fills: Vec::new(), stroke: None, cell_fills: Vec::new() }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Axis {
    Row,
    Col,
}

type Key = (TrackId, TrackId);

impl Axis {
    fn tracks(self, p: &TableProps) -> &Vec<TableTrack> {
        match self {
            Axis::Row => &p.rows,
            Axis::Col => &p.cols,
        }
    }
    fn tracks_mut(self, p: &mut TableProps) -> &mut Vec<TableTrack> {
        match self {
            Axis::Row => &mut p.rows,
            Axis::Col => &mut p.cols,
        }
    }
    fn other(self) -> Axis {
        match self {
            Axis::Row => Axis::Col,
            Axis::Col => Axis::Row,
        }
    }
    fn word(self) -> &'static str {
        match self {
            Axis::Row => "row",
            Axis::Col => "column",
        }
    }
    fn default_size(self) -> f32 {
        match self {
            Axis::Row => TABLE_DEFAULT_ROW_HEIGHT,
            Axis::Col => TABLE_DEFAULT_COLUMN_WIDTH,
        }
    }
    /// The cell key for a track on this axis and a track on the other one.
    fn key(self, line: TrackId, cross: TrackId) -> Key {
        match self {
            Axis::Row => (line, cross),
            Axis::Col => (cross, line),
        }
    }
    fn line_of(self, k: Key) -> TrackId {
        if self == Axis::Row {
            k.0
        } else {
            k.1
        }
    }
    fn merge_line(self, m: &TableMerge) -> TrackId {
        if self == Axis::Row {
            m.row
        } else {
            m.col
        }
    }
    fn merge_span(self, m: &TableMerge) -> usize {
        (if self == Axis::Row { m.row_span } else { m.col_span }) as usize
    }
    fn set_merge(self, m: &mut TableMerge, line: TrackId, span: usize) {
        match self {
            Axis::Row => (m.row, m.row_span) = (line, span as u32),
            Axis::Col => (m.col, m.col_span) = (line, span as u32),
        }
    }
}

fn table_of(doc: &Document, id: NodeId) -> Result<&Node, E> {
    match doc.get(id) {
        Some(n) if matches!(n.kind, NodeKind::Table(_)) => Ok(n),
        _ => Err(E::NotATable("This is not a table. Select a table and try again.".into())),
    }
}

fn props_of(node: &Node) -> &TableProps {
    match &node.kind {
        NodeKind::Table(p) => p,
        _ => unreachable!("table_of checked the kind"),
    }
}

/// Smallest free block of `n` node ids at or above the document's counter.
fn id_block(doc: &Document, n: usize) -> u64 {
    let mut base = doc.peek_next_id().0.max(1);
    while let Some((id, _)) = doc.nodes.range(NodeId(base)..NodeId(base.saturating_add(n as u64))).next_back() {
        base = id.0 + 1;
    }
    base
}

fn clamp_size(size: f32) -> f32 {
    if size.is_nan() {
        TABLE_MIN_TRACK
    } else {
        size.clamp(TABLE_MIN_TRACK, TABLE_MAX_SIZE)
    }
}

fn human_range(from: usize, to: usize, word: &str) -> String {
    if from == to {
        format!("{word} {}", from + 1)
    } else {
        format!("{word}s {}-{}", from + 1, to + 1)
    }
}

fn new_cell_node(id: NodeId, table: NodeId, key: Key, fills: Vec<Paint>) -> Node {
    let mut n = Node::new(id, NodeKind::TableCell(TableCellProps { row: key.0, col: key.1, text: default_cell_text("") }));
    n.name = n.kind.default_name().to_string();
    n.parent = Some(table);
    n.fills = fills;
    n
}

/// A table lifted out of the document into plain maps, edited there, and diffed back into a command.
struct Draft<'a> {
    doc: &'a Document,
    id: NodeId,
    table: Node,
    props: TableProps,
    /// Every cell node, by its row and column track.
    cells: HashMap<Key, Node>,
    next_node: u64,
}

impl<'a> Draft<'a> {
    fn open(doc: &'a Document, id: NodeId) -> Result<Self, E> {
        let table = table_of(doc, id)?.clone();
        check_table_positions(doc, id).map_err(E::BrokenTable)?;
        let props = props_of(&table).clone();
        let cells = table
            .children
            .iter()
            .filter_map(|&c| doc.get(c))
            .filter_map(|n| if let NodeKind::TableCell(c) = &n.kind { Some(((c.row, c.col), n.clone())) } else { None })
            .collect();
        Ok(Self { doc, id, table, props, cells, next_node: id_block(doc, 1) })
    }

    fn grid(&self) -> Grid {
        Grid::new(&self.props)
    }

    fn key_at(&self, pos: (usize, usize)) -> Key {
        (self.props.rows[pos.0].id, self.props.cols[pos.1].id)
    }

    fn fresh_id(&mut self) -> NodeId {
        while self.doc.nodes.contains_key(&NodeId(self.next_node)) {
            self.next_node += 1;
        }
        self.next_node += 1;
        NodeId(self.next_node - 1)
    }

    fn add_cell(&mut self, key: Key, fills: Vec<Paint>) {
        let id = self.fresh_id();
        self.cells.insert(key, new_cell_node(id, self.id, key, fills));
    }

    /// The fills of the cell at `pos`, or of the anchor that covers it.
    fn fills_at(&self, grid: &Grid, pos: (usize, usize)) -> Vec<Paint> {
        let pos = grid.covering_anchor(pos).unwrap_or(pos);
        self.cells.get(&self.key_at(pos)).map(|n| n.fills.clone()).unwrap_or_default()
    }

    fn cell_text_mut(&mut self, key: Key) -> Option<&mut TextProps> {
        match &mut self.cells.get_mut(&key)?.kind {
            NodeKind::TableCell(c) => Some(&mut c.text),
            _ => None,
        }
    }

    /// Lays the edited table out in a scratch document and turns the difference into one batch.
    fn finish(mut self) -> Result<Command, E> {
        let grid = self.grid();
        let broken = |what: &str| E::BrokenTable(format!("The table edit left {what}. Use a different table operation."));
        if grid.fixed > 0 || grid.rows == 0 || grid.cols == 0 {
            return Err(broken("an invalid merge or an empty table"));
        }
        let mut live: Vec<((usize, usize), Key)> = Vec::new();
        for pos in grid.live() {
            let key = self.key_at(pos);
            if !self.cells.contains_key(&key) {
                self.add_cell(key, Vec::new());
            }
            live.push((pos, key));
        }
        if live.len() != self.cells.len() {
            return Err(broken("a cell at a position that no longer exists"));
        }
        let pos_of_id: HashMap<NodeId, (usize, usize)> = live.iter().map(|(pos, key)| (self.cells[key].id, *pos)).collect();
        let mut fresh: Vec<((usize, usize), NodeId)> = pos_of_id.iter().filter(|(id, _)| self.doc.get(**id).is_none()).map(|(id, pos)| (*pos, *id)).collect();
        fresh.sort();
        let mut fresh = fresh.into_iter().peekable();

        // Child order: surviving children keep their place, new cells slot in by row-major position.
        let mut order: Vec<NodeId> = Vec::with_capacity(self.cells.len());
        let mut removed: Vec<NodeId> = Vec::new();
        for &child in &self.table.children {
            let is_cell = self.doc.get(child).is_some_and(|n| matches!(n.kind, NodeKind::TableCell(_)));
            match pos_of_id.get(&child) {
                Some(pos) => {
                    while fresh.peek().is_some_and(|(p, _)| p < pos) {
                        order.extend(fresh.next().map(|(_, id)| id));
                    }
                    order.push(child);
                }
                None if is_cell => removed.push(child),
                None => order.push(child),
            }
        }
        order.extend(fresh.map(|(_, id)| id));

        let mut scratch = Document::new("table-ops");
        let mut table = self.table.clone();
        table.kind = NodeKind::Table(self.props);
        table.children = order.clone();
        scratch.nodes.insert(self.id, table);
        for n in self.cells.into_values() {
            scratch.nodes.insert(n.id, n);
        }
        for cmd in relayout(&scratch, self.id) {
            cmd.apply(&mut scratch).map_err(|e| E::BrokenTable(format!("{e}")))?;
        }

        let mut cmds: Vec<Command> = removed.into_iter().map(|id| Command::Remove { id }).collect();
        let mut table = scratch.nodes[&self.id].clone();
        table.children = self.table.children.clone();
        if table != *self.doc.get(self.id).expect("opened above") {
            cmds.push(Command::Update { node: Box::new(table) });
        }
        let mut inserts = Vec::new();
        for (index, id) in order.iter().enumerate() {
            let Some(node) = scratch.nodes.get(id) else { continue };
            match self.doc.get(*id) {
                None => inserts.push(Command::Insert { parent: self.id, index, nodes: vec![node.clone()] }),
                Some(old) if old != node => cmds.push(Command::Update { node: Box::new(node.clone()) }),
                Some(_) => {}
            }
        }
        cmds.extend(inserts);
        if cmds.is_empty() {
            return Err(E::NoChange("Nothing to change in the table.".into()));
        }
        Ok(Command::Batch(cmds))
    }
}

/// A table of `rows` x `cols` cells, with its cells, as one `Insert` under `parent`.
///
/// `at` is the table's top-left in `parent`'s coordinates; the table goes on top of `parent`'s children.
/// Undo removes the table and all its cells in one step.
pub fn create_table(doc: &Document, parent: NodeId, at: Vec2, rows: usize, cols: usize, style: &TableStyle) -> Result<Command, E> {
    if rows == 0 || cols == 0 {
        return Err(E::IndexOutOfRange("A table needs at least one row and one column.".into()));
    }
    if rows > TABLE_MAX_TRACKS || cols > TABLE_MAX_TRACKS {
        return Err(E::TooLarge(format!("A table can have at most {TABLE_MAX_TRACKS} rows and {TABLE_MAX_TRACKS} columns. Make a smaller table.")));
    }
    let p = doc.get(parent).ok_or_else(|| E::BadParent("The place for the table no longer exists. Pick another place.".into()))?;
    if !p.kind.can_hold_children() || matches!(p.kind, NodeKind::Table(_)) {
        return Err(E::BadParent("A table cannot go inside this layer. Put it on a page or in a frame.".into()));
    }
    let index = p.children.len();
    let mut scratch = Document::new("table-ops");
    scratch.reserve_ids(NodeId(id_block(doc, 1 + rows * cols) - 1));
    let nodes = build_table(&mut scratch, rows, cols, at);
    let table_id = nodes[0].id;
    for mut n in nodes {
        if let NodeKind::Table(p) = &mut n.kind {
            p.rows.iter_mut().for_each(|t| t.size = clamp_size(style.row_height));
            p.cols.iter_mut().for_each(|t| t.size = clamp_size(style.column_width));
            n.fills = style.fills.clone();
            if let Some(s) = &style.stroke {
                n.stroke = s.clone();
            }
        } else {
            n.fills = style.cell_fills.clone();
        }
        scratch.nodes.insert(n.id, n);
    }
    for cmd in relayout(&scratch, table_id) {
        cmd.apply(&mut scratch).map_err(|e| E::BrokenTable(format!("{e}")))?;
    }
    let table = &scratch.nodes[&table_id];
    let nodes: Vec<Node> = std::iter::once(table.clone()).chain(table.children.iter().map(|c| scratch.nodes[c].clone())).collect();
    Ok(Command::Insert { parent, index, nodes })
}

/// Adds `count` default-size rows before row `at_index` (`at_index == rows` appends).
///
/// New cells copy the fills of the neighbouring row (the one above, or the one below for the first
/// row). A merge that spans the insertion point grows by `count`.
pub fn insert_rows(doc: &Document, table: NodeId, at_index: usize, count: usize) -> Result<Command, E> {
    insert_tracks(doc, table, Axis::Row, at_index, count)
}

/// Like [`insert_rows`], for columns.
pub fn insert_cols(doc: &Document, table: NodeId, at_index: usize, count: usize) -> Result<Command, E> {
    insert_tracks(doc, table, Axis::Col, at_index, count)
}

fn insert_tracks(doc: &Document, table: NodeId, axis: Axis, at: usize, count: usize) -> Result<Command, E> {
    let mut d = Draft::open(doc, table)?;
    let (word, len) = (axis.word(), axis.tracks(&d.props).len());
    if count == 0 {
        return Err(E::IndexOutOfRange(format!("Add at least one {word}.")));
    }
    if at > len {
        return Err(E::IndexOutOfRange(format!("The table has {len} {word}s. Pick a place from 1 to {}.", len + 1)));
    }
    if count > TABLE_MAX_TRACKS.saturating_sub(len) {
        return Err(E::TooLarge(format!("A table can have at most {TABLE_MAX_TRACKS} {word}s. Add fewer.")));
    }
    // The new cells copy the fills of the neighbouring track, read before anything changes.
    let grid = d.grid();
    let neighbour = at.saturating_sub(1);
    let cross_len = axis.other().tracks(&d.props).len();
    let inherit: Vec<Vec<Paint>> = (0..cross_len).map(|x| d.fills_at(&grid, if axis == Axis::Row { (neighbour, x) } else { (x, neighbour) })).collect();

    // A merge that spans the insertion point grows.
    let ids: Vec<TrackId> = axis.tracks(&d.props).iter().map(|t| t.id).collect();
    for m in &mut d.props.merges {
        let line = axis.merge_line(m);
        let (first, span) = (ids.iter().position(|id| *id == line).unwrap_or(0), axis.merge_span(m));
        if first < at && at < first + span {
            axis.set_merge(m, line, span + count);
        }
    }
    let fresh: Vec<TrackId> = (0..count).map(|_| d.props.alloc_track()).collect();
    for (i, id) in fresh.iter().enumerate() {
        axis.tracks_mut(&mut d.props).insert(at + i, TableTrack { id: *id, size: axis.default_size() });
    }
    let grid = d.grid();
    let cross: Vec<TrackId> = axis.other().tracks(&d.props).iter().map(|t| t.id).collect();
    for (i, line) in fresh.iter().enumerate() {
        for (x, cross_id) in cross.iter().enumerate() {
            let pos = if axis == Axis::Row { (at + i, x) } else { (x, at + i) };
            if !grid.covered.contains(&pos) {
                d.add_cell(axis.key(*line, *cross_id), inherit[x].clone());
            }
        }
    }
    d.finish()
}

/// Removes the rows with these ids and their cells. A merge shrinks by the rows it loses; when its
/// anchor row goes, the anchor cell (and its text) moves to the first row of the block that stays.
/// Removing every row removes the table.
pub fn delete_rows(doc: &Document, table: NodeId, ids: &[TrackId]) -> Result<Command, E> {
    delete_tracks(doc, table, Axis::Row, ids)
}

/// Like [`delete_rows`], for columns.
pub fn delete_cols(doc: &Document, table: NodeId, ids: &[TrackId]) -> Result<Command, E> {
    delete_tracks(doc, table, Axis::Col, ids)
}

fn delete_tracks(doc: &Document, table: NodeId, axis: Axis, ids: &[TrackId]) -> Result<Command, E> {
    let mut d = Draft::open(doc, table)?;
    let all: Vec<TrackId> = axis.tracks(&d.props).iter().map(|t| t.id).collect();
    let del: HashSet<TrackId> = ids.iter().copied().collect();
    if del.is_empty() {
        return Err(E::NoChange(format!("Select a {} to delete.", axis.word())));
    }
    if del.iter().any(|id| !all.contains(id)) {
        return Err(E::IndexOutOfRange(format!("A {} you selected is no longer in the table. Select it again.", axis.word())));
    }
    if all.iter().all(|id| del.contains(id)) {
        return Ok(Command::Batch(vec![Command::Remove { id: table }]));
    }
    let cross = axis.other();
    let mut kept = Vec::new();
    for mut m in std::mem::take(&mut d.props.merges) {
        let old = axis.merge_line(&m);
        let first = all.iter().position(|id| *id == old).unwrap_or(0);
        let alive: Vec<TrackId> = all[first..first + axis.merge_span(&m)].iter().filter(|id| !del.contains(id)).copied().collect();
        let Some(&new_line) = alive.first() else { continue };
        if new_line != old {
            let (from, to) = (axis.key(old, cross.merge_line(&m)), axis.key(new_line, cross.merge_line(&m)));
            if let Some(mut anchor) = d.cells.remove(&from) {
                if let NodeKind::TableCell(c) = &mut anchor.kind {
                    (c.row, c.col) = to;
                }
                d.cells.insert(to, anchor);
            }
        }
        if alive.len() * cross.merge_span(&m) > 1 {
            axis.set_merge(&mut m, new_line, alive.len());
            kept.push(m);
        }
    }
    d.props.merges = kept;
    d.cells.retain(|k, _| !del.contains(&axis.line_of(*k)));
    axis.tracks_mut(&mut d.props).retain(|t| !del.contains(&t.id));
    d.finish()
}

/// Moves row `id` so it ends at `to_index`. Refused with [`TableOpError::SplitsMerge`] when a merge
/// would no longer be one block.
pub fn move_row(doc: &Document, table: NodeId, id: TrackId, to_index: usize) -> Result<Command, E> {
    move_track(doc, table, Axis::Row, id, to_index)
}

/// Like [`move_row`], for columns.
pub fn move_col(doc: &Document, table: NodeId, id: TrackId, to_index: usize) -> Result<Command, E> {
    move_track(doc, table, Axis::Col, id, to_index)
}

fn move_track(doc: &Document, table: NodeId, axis: Axis, id: TrackId, to: usize) -> Result<Command, E> {
    let mut d = Draft::open(doc, table)?;
    let word = axis.word();
    let ids: Vec<TrackId> = axis.tracks(&d.props).iter().map(|t| t.id).collect();
    let from = ids.iter().position(|t| *t == id).ok_or_else(|| E::IndexOutOfRange(format!("This {word} is no longer in the table. Select it again.")))?;
    if to >= ids.len() {
        return Err(E::IndexOutOfRange(format!("The table has {} {word}s. Move it to a place from 1 to {}.", ids.len(), ids.len())));
    }
    if from == to {
        return Err(E::NoChange(format!("The {word} is already there.")));
    }
    let mut order = ids.clone();
    let moved = order.remove(from);
    order.insert(to, moved);
    let at_new: HashMap<TrackId, usize> = order.iter().enumerate().map(|(i, t)| (*t, i)).collect();
    for m in &d.props.merges {
        let first = ids.iter().position(|t| *t == axis.merge_line(m)).unwrap_or(0);
        let span = axis.merge_span(m);
        let start = at_new[&ids[first]];
        if (0..span).any(|k| at_new[&ids[first + k]] != start + k) {
            let range = human_range(first, first + span - 1, word);
            return Err(E::SplitsMerge(format!("Moving this {word} would split the merged cells in {range}. Unmerge them first.")));
        }
    }
    axis.tracks_mut(&mut d.props).sort_by_key(|t| at_new[&t.id]);
    d.finish()
}

/// Sets the height of row `id`, at least [`TABLE_MIN_TRACK`].
pub fn resize_row(doc: &Document, table: NodeId, id: TrackId, size: f32) -> Result<Command, E> {
    resize_track(doc, table, Axis::Row, id, size)
}

/// Sets the width of column `id`, at least [`TABLE_MIN_TRACK`].
pub fn resize_col(doc: &Document, table: NodeId, id: TrackId, size: f32) -> Result<Command, E> {
    resize_track(doc, table, Axis::Col, id, size)
}

fn resize_track(doc: &Document, table: NodeId, axis: Axis, id: TrackId, size: f32) -> Result<Command, E> {
    let mut d = Draft::open(doc, table)?;
    let word = axis.word();
    let track = axis.tracks_mut(&mut d.props).iter_mut().find(|t| t.id == id);
    track.ok_or_else(|| E::IndexOutOfRange(format!("This {word} is no longer in the table. Select it again.")))?.size = clamp_size(size);
    d.finish()
}

/// Scales every row and column so the tracks add up to `new_size`. Text is not scaled.
pub fn resize_table(doc: &Document, table: NodeId, new_size: Vec2) -> Result<Command, E> {
    let mut d = Draft::open(doc, table)?;
    if !(new_size.x.is_finite() && new_size.y.is_finite() && new_size.x > 0.0 && new_size.y > 0.0) {
        return Err(E::IndexOutOfRange("Give the table a width and a height above zero.".into()));
    }
    for (tracks, request) in [(&mut d.props.cols, new_size.x), (&mut d.props.rows, new_size.y)] {
        let goal = f64::from(request).min(f64::from(TABLE_MAX_SIZE));
        let sum: f64 = tracks.iter().map(|t| f64::from(clamp_size(t.size))).sum();
        tracks.iter_mut().for_each(|t| t.size = clamp_size((f64::from(clamp_size(t.size)) * goal / sum) as f32));
    }
    d.finish()
}

/// Merges the block from `(row_from, col_from)` to `(row_to, col_to)`, zero-based and inclusive, into
/// its top-left cell. The text of the cells it covers goes to the end of the anchor's text, one line
/// per cell with text. A merge the block fully contains is absorbed; one it only cuts into is refused
/// with [`TableOpError::OverlapsMerge`].
pub fn merge(doc: &Document, table: NodeId, rect: (usize, usize, usize, usize)) -> Result<Command, E> {
    let mut d = Draft::open(doc, table)?;
    let (r0, c0, r1, c1) = (rect.0.min(rect.2), rect.1.min(rect.3), rect.0.max(rect.2), rect.1.max(rect.3));
    let grid = d.grid();
    if r1 >= grid.rows || c1 >= grid.cols {
        return Err(E::IndexOutOfRange(format!("The table has {} rows and {} columns. Select cells inside it.", grid.rows, grid.cols)));
    }
    if (r0, c0) == (r1, c1) {
        return Err(E::NoChange("Select two or more cells to merge.".into()));
    }
    let mut anchors: Vec<_> = grid.anchors.iter().map(|(a, s)| (*a, *s)).collect();
    anchors.sort();
    let mut absorbed: HashSet<(usize, usize)> = HashSet::new();
    for ((ar, ac), (rs, cs)) in anchors {
        let (br, bc) = (ar + rs - 1, ac + cs - 1);
        if ar > r1 || br < r0 || ac > c1 || bc < c0 {
            continue;
        }
        if ar >= r0 && br <= r1 && ac >= c0 && bc <= c1 {
            if (ar, ac, br, bc) == (r0, c0, r1, c1) {
                return Err(E::NoChange("These cells are already merged.".into()));
            }
            absorbed.insert((ar, ac));
        } else {
            let (rows, cols) = (human_range(ar, br, "row"), human_range(ac, bc, "column"));
            return Err(E::OverlapsMerge(format!(
                "The selection cuts into the merged cells in {rows}, {cols}. Unmerge them first, or select the whole block."
            )));
        }
    }
    let anchor_key = d.key_at((r0, c0));
    let mut lines = Vec::new();
    for pos in grid.live().filter(|p| (r0..=r1).contains(&p.0) && (c0..=c1).contains(&p.1) && *p != (r0, c0)) {
        let key = d.key_at(pos);
        if let Some(Node { kind: NodeKind::TableCell(c), .. }) = d.cells.remove(&key) {
            if !c.text.content.is_empty() {
                lines.push(c.text.content);
            }
        }
    }
    if let Some(text) = d.cell_text_mut(anchor_key) {
        for line in lines {
            if !text.content.is_empty() {
                text.content.push('\n');
            }
            text.content.push_str(&line);
        }
    }
    d.props.merges.retain(|m| !absorbed.contains(&(grid.row_ix[&m.row], grid.col_ix[&m.col])));
    d.props.merges.push(TableMerge { row: anchor_key.0, col: anchor_key.1, row_span: (r1 - r0 + 1) as u32, col_span: (c1 - c0 + 1) as u32 });
    d.finish()
}

/// Splits the merged block whose anchor is `anchor_cell`. The covered positions get new empty cells
/// that carry the anchor's fills, so a coloured block stays coloured.
pub fn unmerge(doc: &Document, table: NodeId, anchor_cell: NodeId) -> Result<Command, E> {
    let mut d = Draft::open(doc, table)?;
    let not_in_table = || E::NotATable("This cell is not in the table. Select a cell of the table and try again.".into());
    let node = doc.get(anchor_cell).filter(|n| n.parent == Some(table)).ok_or_else(not_in_table)?;
    let NodeKind::TableCell(c) = &node.kind else { return Err(not_in_table()) };
    let (row, col) = (c.row, c.col);
    let at = d.props.merges.iter().position(|m| m.row == row && m.col == col);
    let m = d.props.merges.remove(at.ok_or_else(|| E::NoChange("This cell is not merged.".into()))?);
    let (r0, c0) = (d.props.row_index(row).unwrap_or(0), d.props.col_index(col).unwrap_or(0));
    for r in r0..r0 + m.row_span as usize {
        for c in c0..c0 + m.col_span as usize {
            if (r, c) != (r0, c0) {
                let key = d.key_at((r, c));
                d.add_cell(key, node.fills.clone());
            }
        }
    }
    d.finish()
}

/// Replaces the text of the cell at zero-based `(row_index, col_index)`, or of the anchor of the
/// merged block that covers that position. Text styling runs and lists are cleared, as for any
/// whole-text replacement.
pub fn set_cell_text(doc: &Document, table: NodeId, row_index: usize, col_index: usize, text: &str) -> Result<Command, E> {
    let mut d = Draft::open(doc, table)?;
    let grid = d.grid();
    if row_index >= grid.rows || col_index >= grid.cols {
        return Err(E::IndexOutOfRange(format!("The table has {} rows and {} columns. Pick a cell inside it.", grid.rows, grid.cols)));
    }
    let pos = (row_index, col_index);
    let key = d.key_at(grid.covering_anchor(pos).unwrap_or(pos));
    let cell = d.cell_text_mut(key).ok_or_else(|| E::BrokenTable("The cell is missing. Use a table operation to repair the table.".into()))?;
    cell.content = text.to_string();
    cell.runs.clear();
    cell.lists.clear();
    d.finish()
}

/// The cell after (or, with `backwards`, before) `cell` in row-major order over the anchors, the
/// order Tab walks. `None` past the last (or first) cell, or when `cell` is not in the table.
pub fn next_cell(doc: &Document, table: NodeId, cell: NodeId, backwards: bool) -> Option<NodeId> {
    let t = doc.get(table)?;
    let NodeKind::Table(p) = &t.kind else { return None };
    let grid = Grid::new(p);
    let by_pos: HashMap<(usize, usize), NodeId> = t
        .children
        .iter()
        .filter_map(|&id| match &doc.get(id)?.kind {
            NodeKind::TableCell(c) => Some((grid.position_of(c)?, id)),
            _ => None,
        })
        .collect();
    let order: Vec<NodeId> = grid.live().filter_map(|pos| by_pos.get(&pos).copied()).collect();
    let i = order.iter().position(|id| *id == cell)?;
    if backwards {
        i.checked_sub(1).map(|j| order[j])
    } else {
        order.get(i + 1).copied()
    }
}

/// Applies `cmd` to `doc` on a commit path that is not `History` (a transaction, a live-patch replay)
/// and keeps the table rule for it: the command is all-or-nothing, and when a table it touched is
/// left without one cell per live position the command is undone and the refusal returned.
/// Returns the inverse, like [`Command::apply`].
pub fn apply_checked(doc: &mut Document, cmd: Command) -> Result<Command, DocError> {
    let mut touched = BTreeSet::new();
    crate::table::touched_tables(doc, &cmd, &mut touched);
    let inverse = cmd.apply_atomic(doc)?;
    match crate::table::check_touched(doc, &touched) {
        Ok(()) => Ok(inverse),
        Err(e) => match inverse.apply(doc) {
            Ok(_) => Err(e),
            Err(re) => Err(DocError::Table(format!("{e} Rolling the edit back also failed: {re}"))),
        },
    }
}

#[cfg(test)]
#[path = "table_ops_tests.rs"]
mod tests;
