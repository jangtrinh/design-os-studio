//! Tables: ordered rows and columns with stable ids, merges, and one pure layout function.
//!
//! A [`NodeKind::Table`] node holds the tracks ([`TableProps`]); its children are
//! [`NodeKind::TableCell`] nodes, one per live `(row, column)` position. A merge keeps one cell, the
//! anchor at its top-left, and the cells it covers do not exist. Cell geometry is stored on the cell
//! node and derived: [`relayout`] is the only writer, so every table edit includes its commands in
//! the same undo batch. [`check_table_positions`] and [`check_child_kind`] keep a table whole while
//! the app runs, and the load repair in `load_repair.rs` fixes a table that arrives broken.

use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};

/// Cell text inset as `(horizontal, vertical)` px (owner question OQ-2, recommended option).
pub const TABLE_CELL_PADDING: (f32, f32) = (12.0, 8.0);

/// The text area of a cell of `size`, in the cell's own space: inside [`TABLE_CELL_PADDING`] on every
/// side and never narrower or shorter than 1 px. The renderer fits the text into it and the editor lays the
/// text out in it, so both read it from here.
pub fn table_cell_text_rect(size: Vec2) -> Rect {
    let (px, py) = TABLE_CELL_PADDING;
    Rect::new(px, py, (size.x - 2.0 * px).max(1.0), (size.y - 2.0 * py).max(1.0))
}
/// Smallest track size a repair writes (owner question OQ-1, recommended option).
pub const TABLE_MIN_TRACK: f32 = 24.0;
/// Width of a new column (OQ-1, recommended option).
pub const TABLE_DEFAULT_COLUMN_WIDTH: f32 = 160.0;
/// Height of a new row (OQ-1, recommended option).
pub const TABLE_DEFAULT_ROW_HEIGHT: f32 = 44.0;

/// Identity of a row or column. Mirrors the Kiwi `GUID` (`sessionID`, `localID`) so an imported
/// table keeps its ids; tracks made in Studio use session 0 and the table's `next_track` counter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TrackId {
    pub session: u32,
    pub local: u32,
}

/// One row or column: its id and the size the user set (`order = Vec order`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableTrack {
    pub id: TrackId,
    pub size: f32,
}

/// A merged block. `row` / `col` name the anchor, the top-left cell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableMerge {
    pub row: TrackId,
    pub col: TrackId,
    pub row_span: u32,
    pub col_span: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableProps {
    pub rows: Vec<TableTrack>,
    pub cols: Vec<TableTrack>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub merges: Vec<TableMerge>,
    /// Allocator for native [`TrackId`]s (session 0).
    pub next_track: u32,
}

/// The cell at one `(row, column)` position of its parent table, with its text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableCellProps {
    pub row: TrackId,
    pub col: TrackId,
    pub text: TextProps,
}

impl TableProps {
    /// `rows` x `cols` tracks of the default size, with native ids.
    pub fn grid(rows: usize, cols: usize) -> Self {
        let mut p = Self { rows: Vec::new(), cols: Vec::new(), merges: Vec::new(), next_track: 1 };
        for _ in 0..rows {
            let id = p.alloc_track();
            p.rows.push(TableTrack { id, size: TABLE_DEFAULT_ROW_HEIGHT });
        }
        for _ in 0..cols {
            let id = p.alloc_track();
            p.cols.push(TableTrack { id, size: TABLE_DEFAULT_COLUMN_WIDTH });
        }
        p
    }

    /// A fresh native track id, never one already used by a row or column of this table.
    pub fn alloc_track(&mut self) -> TrackId {
        loop {
            let id = TrackId { session: 0, local: self.next_track };
            self.next_track = self.next_track.wrapping_add(1).max(1);
            if !self.rows.iter().chain(&self.cols).any(|t| t.id == id) {
                return id;
            }
        }
    }

    pub fn row_index(&self, id: TrackId) -> Option<usize> {
        self.rows.iter().position(|t| t.id == id)
    }

    pub fn col_index(&self, id: TrackId) -> Option<usize> {
        self.cols.iter().position(|t| t.id == id)
    }
}

/// Text of a new cell: Inter Medium 16 (owner ruling OQ-13), fixed width, height follows the content.
pub fn default_cell_text(content: &str) -> TextProps {
    let style = TextStyle { font_family: "Inter".into(), font_weight: 500, font_size: 16.0, ..TextStyle::default() };
    TextProps { content: content.to_string(), style, auto_resize: TextAutoResize::Height, ..Default::default() }
}

fn track_len(size: f32) -> f32 {
    if size.is_finite() && size > 0.0 {
        size
    } else {
        TABLE_MIN_TRACK
    }
}

fn track_index(tracks: &[TableTrack]) -> HashMap<TrackId, usize> {
    let mut ix = HashMap::with_capacity(tracks.len());
    for (i, t) in tracks.iter().enumerate() {
        ix.entry(t.id).or_insert(i);
    }
    ix
}

/// Index view of a table's tracks and merges. Merges that do not fit (unknown anchor, overlap, past
/// the edge) are fixed on the way in and counted in `fixed`; `merges` is the sanitised list.
pub(crate) struct Grid {
    pub rows: usize,
    pub cols: usize,
    pub row_ix: HashMap<TrackId, usize>,
    pub col_ix: HashMap<TrackId, usize>,
    /// Anchor position -> `(row_span, col_span)`.
    pub anchors: HashMap<(usize, usize), (usize, usize)>,
    /// Positions a merge covers, anchors excluded.
    pub covered: HashSet<(usize, usize)>,
    pub merges: Vec<TableMerge>,
    pub fixed: usize,
}

impl Grid {
    pub fn new(p: &TableProps) -> Self {
        let (rows, cols) = (p.rows.len(), p.cols.len());
        let (row_ix, col_ix) = (track_index(&p.rows), track_index(&p.cols));
        let mut g = Self { rows, cols, row_ix, col_ix, anchors: HashMap::new(), covered: HashSet::new(), merges: Vec::new(), fixed: 0 };
        let mut taken: HashSet<(usize, usize)> = HashSet::new();
        for m in &p.merges {
            let (Some(&r0), Some(&c0)) = (g.row_ix.get(&m.row), g.col_ix.get(&m.col)) else {
                g.fixed += 1;
                continue;
            };
            let rs = (m.row_span.max(1) as usize).min(rows - r0);
            let cs = (m.col_span.max(1) as usize).min(cols - c0);
            if rs != m.row_span as usize || cs != m.col_span as usize {
                g.fixed += 1;
            }
            let block = || (r0..r0 + rs).flat_map(move |r| (c0..c0 + cs).map(move |c| (r, c)));
            if block().any(|pos| taken.contains(&pos)) {
                g.fixed += 1;
                continue;
            }
            taken.extend(block());
            g.covered.extend(block().filter(|pos| *pos != (r0, c0)));
            g.anchors.insert((r0, c0), (rs, cs));
            g.merges.push(TableMerge { row: m.row, col: m.col, row_span: rs as u32, col_span: cs as u32 });
        }
        g
    }

    pub fn span(&self, pos: (usize, usize)) -> (usize, usize) {
        self.anchors.get(&pos).copied().unwrap_or((1, 1))
    }

    pub fn live_positions(&self) -> usize {
        self.rows * self.cols - self.covered.len()
    }

    /// Row-major live positions (anchors and unmerged cells).
    pub fn live(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        (0..self.rows).flat_map(move |r| (0..self.cols).map(move |c| (r, c))).filter(|pos| !self.covered.contains(pos))
    }

    /// The anchor of the merged block that covers `pos`, if `pos` is a covered position.
    pub fn covering_anchor(&self, pos: (usize, usize)) -> Option<(usize, usize)> {
        if !self.covered.contains(&pos) {
            return None;
        }
        self.anchors.iter().find(|(a, s)| pos.0 >= a.0 && pos.0 < a.0 + s.0 && pos.1 >= a.1 && pos.1 < a.1 + s.1).map(|(a, _)| *a)
    }

    pub fn position_of(&self, c: &TableCellProps) -> Option<(usize, usize)> {
        Some((*self.row_ix.get(&c.row)?, *self.col_ix.get(&c.col)?))
    }
}

struct CellSlot<'a> {
    pos: (usize, usize),
    span: (usize, usize),
    node: &'a Node,
    props: &'a TableCellProps,
}

/// The geometry [`relayout`] would store: the cell commands plus the table's own size.
///
/// Pure. Returns one `Command::Update` per cell whose transform or size changes, plus one for the
/// table's size (the sum of its tracks). A merge anchor spans its tracks. With a layout measurer
/// installed a row grows to fit its tallest single-row cell, and the last row of a merged block grows by
/// what the block's text lacks (owner ruling OQ-6); the stored
/// `TableTrack.size` keeps the user's height, so a `.fig` round trip never ratchets rows taller.
pub fn relayout(doc: &Document, table_id: NodeId) -> Vec<Command> {
    let Some(table) = doc.get(table_id) else { return Vec::new() };
    let NodeKind::Table(props) = &table.kind else { return Vec::new() };
    let grid = Grid::new(props);
    let slots: Vec<CellSlot> = table
        .children
        .iter()
        .filter_map(|&id| {
            let node = doc.get(id)?;
            let NodeKind::TableCell(c) = &node.kind else { return None };
            let pos = grid.position_of(c).filter(|p| !grid.covered.contains(p))?;
            Some(CellSlot { pos, span: grid.span(pos), node, props: c })
        })
        .collect();

    let col_w: Vec<f32> = props.cols.iter().map(|t| track_len(t.size)).collect();
    let mut row_h: Vec<f32> = props.rows.iter().map(|t| track_len(t.size)).collect();
    if let Some(measurer) = text_measure::layout_measurer() {
        let needed = |s: &CellSlot| {
            let width = col_w[s.pos.1..s.pos.1 + s.span.1].iter().sum::<f32>() - 2.0 * TABLE_CELL_PADDING.0;
            text_measure::intrinsic_text_size(&s.props.text, Some(width.max(1.0)), Some(measurer.as_ref())).y + 2.0 * TABLE_CELL_PADDING.1
        };
        for s in slots.iter().filter(|s| s.span.0 == 1 && !s.props.text.content.is_empty()) {
            row_h[s.pos.0] = row_h[s.pos.0].max(needed(s));
        }
        // A merged block with taller text than its rows grows its last row by the shortfall. Blocks
        // are settled bottom edge first so the result does not depend on child order.
        let mut blocks: Vec<&CellSlot> = slots.iter().filter(|s| s.span.0 > 1 && !s.props.text.content.is_empty()).collect();
        blocks.sort_by_key(|s| (s.pos.0 + s.span.0, s.pos.0, s.pos.1));
        for s in blocks {
            let last = s.pos.0 + s.span.0 - 1;
            let have: f32 = row_h[s.pos.0..=last].iter().sum();
            row_h[last] += (needed(s) - have).max(0.0);
        }
    }
    let offsets = |sizes: &[f32]| -> Vec<f32> {
        let mut acc = 0.0;
        let mut out = Vec::with_capacity(sizes.len() + 1);
        for s in sizes {
            out.push(acc);
            acc += s;
        }
        out.push(acc);
        out
    };
    let (col_off, row_off) = (offsets(&col_w), offsets(&row_h));

    let mut cmds = Vec::new();
    for s in &slots {
        let transform = Affine::translate(col_off[s.pos.1], row_off[s.pos.0]);
        let size = Vec2::new(col_off[s.pos.1 + s.span.1] - col_off[s.pos.1], row_off[s.pos.0 + s.span.0] - row_off[s.pos.0]);
        if s.node.transform != transform || s.node.size != size {
            let mut n = s.node.clone();
            n.transform = transform;
            n.size = size;
            cmds.push(Command::Update { node: Box::new(n) });
        }
    }
    let size = Vec2::new(col_off[props.cols.len()], row_off[props.rows.len()]);
    if table.size != size {
        let mut n = table.clone();
        n.size = size;
        cmds.push(Command::Update { node: Box::new(n) });
    }
    cmds
}

/// A table's text being measured ahead of its layout, a slice of cells at a time, so a big table's cold
/// cost (a table's worth of shaping) can be spread over frames and the layout itself then only reads
/// the measurer's cache. The jobs are listed once, in [`TableWarm::new`]; each [`TableWarm::step`] then
/// does its share, so the work for a table is O(cells) however many slices it takes.
pub struct TableWarm {
    table: NodeId,
    /// A cell with text and the width its text wraps at, as [`relayout`] measures it.
    jobs: Vec<(NodeId, f32)>,
    next: usize,
}

impl TableWarm {
    /// The jobs of `table_id`. `None` when there is no such table or no layout measurer to warm.
    pub fn new(doc: &Document, table_id: NodeId) -> Option<Self> {
        let table = doc.get(table_id)?;
        let NodeKind::Table(props) = &table.kind else { return None };
        text_measure::layout_measurer()?;
        let grid = Grid::new(props);
        let col_w: Vec<f32> = props.cols.iter().map(|t| track_len(t.size)).collect();
        let jobs = table
            .children
            .iter()
            .filter_map(|&id| {
                let NodeKind::TableCell(c) = &doc.get(id)?.kind else { return None };
                if c.text.content.is_empty() {
                    return None;
                }
                let pos = grid.position_of(c).filter(|p| !grid.covered.contains(p))?;
                let span = grid.span(pos);
                let width = col_w[pos.1..pos.1 + span.1].iter().sum::<f32>() - 2.0 * TABLE_CELL_PADDING.0;
                Some((id, width.max(1.0)))
            })
            .collect();
        Some(Self { table: table_id, jobs, next: 0 })
    }

    pub fn table(&self) -> NodeId {
        self.table
    }

    /// Measures the next `count` cells. Returns whether every cell has been measured.
    pub fn step(&mut self, doc: &Document, count: usize) -> bool {
        let Some(measurer) = text_measure::layout_measurer() else {
            self.next = self.jobs.len();
            return true;
        };
        let end = (self.next + count).min(self.jobs.len());
        for &(id, width) in &self.jobs[self.next..end] {
            if let Some(NodeKind::TableCell(c)) = doc.get(id).map(|n| &n.kind) {
                text_measure::intrinsic_text_size(&c.text, Some(width), Some(measurer.as_ref()));
            }
        }
        self.next = end;
        self.done()
    }

    pub fn done(&self) -> bool {
        self.next >= self.jobs.len()
    }
}

/// Applies [`relayout`] for `table_id` straight to `doc` (load repair, tests).
pub(crate) fn apply_relayout(doc: &mut Document, table_id: NodeId) {
    apply_table_relayout(doc, table_id);
}

/// Applies [`relayout`] for `table_id` straight to `doc`, outside any undo step, and says whether it
/// moved anything. For writes that are not an edit, such as measuring again once a font has loaded.
pub fn apply_table_relayout(doc: &mut Document, table_id: NodeId) -> bool {
    let cmds = relayout(doc, table_id);
    let changed = !cmds.is_empty();
    for cmd in cmds {
        let _ = cmd.apply(doc);
    }
    changed
}

/// The anchor cell under `local`, a point in the table's own coordinates; `None` outside the table.
pub fn cell_at(doc: &Document, table_id: NodeId, local: Vec2) -> Option<NodeId> {
    let table = doc.get(table_id)?;
    if !matches!(table.kind, NodeKind::Table(_)) {
        return None;
    }
    table.children.iter().copied().find(|&id| {
        doc.get(id).is_some_and(|n| {
            matches!(n.kind, NodeKind::TableCell(_))
                && local.x >= n.transform.tx
                && local.x < n.transform.tx + n.size.x
                && local.y >= n.transform.ty
                && local.y < n.transform.ty + n.size.y
        })
    })
}

/// Nodes for a new `rows` x `cols` table at `origin` (table first, then its cells in row-major order),
/// ready for `Command::Insert`. Cells have the default text style and no fills; the table has default
/// track sizes and no styling.
pub fn build_table(doc: &mut Document, rows: usize, cols: usize, origin: Vec2) -> Vec<Node> {
    let props = TableProps::grid(rows, cols);
    let table_id = doc.alloc_id();
    let mut table = Node::new(table_id, NodeKind::Table(props.clone()));
    table.name = table.kind.default_name().to_string();
    table.transform = Affine::translate(origin.x, origin.y);
    table.size = Vec2::new(cols as f32 * TABLE_DEFAULT_COLUMN_WIDTH, rows as f32 * TABLE_DEFAULT_ROW_HEIGHT);
    let mut cells = Vec::with_capacity(rows * cols);
    for (r, row) in props.rows.iter().enumerate() {
        for (c, col) in props.cols.iter().enumerate() {
            let id = doc.alloc_id();
            let mut cell = Node::new(id, NodeKind::TableCell(TableCellProps { row: row.id, col: col.id, text: default_cell_text("") }));
            cell.name = cell.kind.default_name().to_string();
            cell.parent = Some(table_id);
            cell.transform = Affine::translate(c as f32 * TABLE_DEFAULT_COLUMN_WIDTH, r as f32 * TABLE_DEFAULT_ROW_HEIGHT);
            cell.size = Vec2::new(TABLE_DEFAULT_COLUMN_WIDTH, TABLE_DEFAULT_ROW_HEIGHT);
            table.children.push(id);
            cells.push(cell);
        }
    }
    let mut out = vec![table];
    out.extend(cells);
    out
}

const NEXT_ACTION: &str = "Use a table operation to add or delete rows, columns or merges.";

/// Tree-membership rule for tables, checked by `Command::Insert` and `Command::Reparent` before they
/// mutate: a `Table` holds only `TableCell` nodes, and a `TableCell` lives only under a `Table`.
pub(crate) fn check_child_kind(parent: &NodeKind, child: &NodeKind) -> Result<(), DocError> {
    match (matches!(parent, NodeKind::Table(_)), matches!(child, NodeKind::TableCell(_))) {
        (true, false) => Err(DocError::Table(format!("A table holds only cells. {NEXT_ACTION}"))),
        (false, true) => Err(DocError::Table(format!("A table cell can live only inside a table. {NEXT_ACTION}"))),
        _ => Ok(()),
    }
}

fn is_table_kind(k: &NodeKind) -> bool {
    matches!(k, NodeKind::Table(_) | NodeKind::TableCell(_))
}

/// Applies [`check_child_kind`] to every parent-child pair an `Insert` payload installs: the root
/// under `parent`, and each node under the payload node whose `children` lists it.
pub(crate) fn check_insert_payload(parent: &NodeKind, nodes: &[Node]) -> Result<(), DocError> {
    let Some(root) = nodes.first() else { return Ok(()) };
    check_child_kind(parent, &root.kind)?;
    if !nodes.iter().any(|n| is_table_kind(&n.kind)) {
        return Ok(());
    }
    let kinds: HashMap<NodeId, &NodeKind> = nodes.iter().map(|n| (n.id, &n.kind)).collect();
    for n in nodes {
        for child in n.children.iter().filter_map(|c| kinds.get(c)) {
            check_child_kind(&n.kind, child)?;
        }
    }
    Ok(())
}

/// A table or a cell never becomes another kind, and no other node becomes one.
pub(crate) fn check_kind_change(old: &NodeKind, new: &NodeKind) -> Result<(), DocError> {
    if (is_table_kind(old) || is_table_kind(new)) && std::mem::discriminant(old) != std::mem::discriminant(new) {
        return Err(DocError::Table(format!("A table or a cell cannot change into another kind. {NEXT_ACTION}")));
    }
    Ok(())
}

/// Whether `cmd` can be refused by a table rule, which is when `History` keeps a copy of it.
pub(crate) fn involves_tables(doc: &Document, cmd: &Command) -> bool {
    let table_kind = |id: NodeId| doc.get(id).is_some_and(|n| is_table_kind(&n.kind));
    match cmd {
        Command::Insert { parent, nodes, .. } => table_kind(*parent) || nodes.iter().any(|n| is_table_kind(&n.kind)),
        Command::Remove { id } => doc.get(*id).and_then(|n| n.parent).is_some_and(table_kind),
        Command::Reparent { id, parent, .. } => table_kind(*id) || table_kind(*parent),
        Command::Update { node } => is_table_kind(&node.kind) || table_kind(node.id),
        Command::Batch(cmds) => cmds.iter().any(|c| involves_tables(doc, c)),
        _ => false,
    }
}

/// Whole-table check: one cell at every live `(row, column)` position, none at a dead or covered one.
/// Children that are not cells are ignored (the kind check keeps them out).
///
/// `History` runs this after every step that touches a table. A commit path that applies commands
/// outside `History` (a transaction, a live-patch replay) must call it before it commits.
pub fn check_table_positions(doc: &Document, table_id: NodeId) -> Result<(), String> {
    let Some(table) = doc.get(table_id) else { return Ok(()) };
    let NodeKind::Table(props) = &table.kind else { return Ok(()) };
    let grid = Grid::new(props);
    if grid.rows == 0 || grid.cols == 0 {
        return Err(format!("A table needs at least one row and one column. {NEXT_ACTION}"));
    }
    if grid.fixed > 0 || grid.row_ix.len() != grid.rows || grid.col_ix.len() != grid.cols {
        return Err(format!("The table's rows, columns or merges are inconsistent. {NEXT_ACTION}"));
    }
    let mut seen: HashSet<(usize, usize)> = HashSet::with_capacity(table.children.len());
    for c in table.children.iter().filter_map(|&id| doc.get(id)).filter_map(|n| if let NodeKind::TableCell(c) = &n.kind { Some(c) } else { None }) {
        let Some(pos) = grid.position_of(c) else {
            return Err(format!("A cell sits at a row or column that no longer exists. {NEXT_ACTION}"));
        };
        if grid.covered.contains(&pos) {
            return Err(format!("A cell sits inside a merged block. {NEXT_ACTION}"));
        }
        if !seen.insert(pos) {
            return Err(format!("Two cells share one position. {NEXT_ACTION}"));
        }
    }
    if seen.len() != grid.live_positions() {
        return Err(format!("A table needs one cell for every row and column. {NEXT_ACTION}"));
    }
    Ok(())
}

/// Tables whose rows, columns, merges or cell membership `cmd` changes, collected before `cmd` runs
/// (a removed cell's parent is gone afterwards). An `Update` that leaves tracks, merges and a cell's
/// position alone is not a structural change and touches nothing, so a text edit costs no scan.
pub(crate) fn touched_tables(doc: &Document, cmd: &Command, out: &mut BTreeSet<NodeId>) {
    let table_of = |id: NodeId| doc.get(id).filter(|n| matches!(n.kind, NodeKind::Table(_))).map(|n| n.id);
    match cmd {
        Command::Insert { parent, nodes, .. } => {
            out.extend(table_of(*parent));
            out.extend(nodes.iter().filter(|n| matches!(n.kind, NodeKind::Table(_))).map(|n| n.id));
        }
        Command::Remove { id } => out.extend(doc.get(*id).and_then(|n| n.parent).and_then(table_of)),
        Command::Reparent { id, parent, .. } => {
            out.extend(doc.get(*id).and_then(|n| n.parent).and_then(table_of));
            out.extend(table_of(*parent));
        }
        Command::Update { node } => match (doc.get(node.id).map(|c| &c.kind), &node.kind) {
            (Some(NodeKind::Table(old)), NodeKind::Table(new)) => {
                if old.rows != new.rows || old.cols != new.cols || old.merges != new.merges {
                    out.insert(node.id);
                }
            }
            (Some(NodeKind::TableCell(old)), NodeKind::TableCell(new)) => {
                if old.row != new.row || old.col != new.col {
                    out.extend(doc.get(node.id).and_then(|n| n.parent).and_then(table_of));
                }
            }
            (_, NodeKind::Table(_)) => {
                out.insert(node.id);
            }
            _ => {}
        },
        Command::Batch(cmds) => cmds.iter().for_each(|c| touched_tables(doc, c, out)),
        _ => {}
    }
}

/// Tables that have a cell whose text `cmd` changes (before the command runs). Every writer of cell
/// text (find and replace, paste, lint fix, the editor) goes through such an update, so
/// `History` re-solves these tables in the same step and no text is left hidden by a stored row height.
pub(crate) fn text_edited_tables(doc: &Document, cmd: &Command, out: &mut BTreeSet<NodeId>) {
    match cmd {
        Command::Update { node } => {
            if let (Some(old), NodeKind::TableCell(new)) = (doc.get(node.id), &node.kind) {
                if matches!(&old.kind, NodeKind::TableCell(o) if o.text != new.text) {
                    out.extend(old.parent.filter(|p| doc.get(*p).is_some_and(|t| matches!(t.kind, NodeKind::Table(_)))));
                }
            }
        }
        Command::Batch(cmds) => cmds.iter().for_each(|c| text_edited_tables(doc, c, out)),
        _ => {}
    }
}

/// Checks every table in `touched` that still exists after the step.
pub(crate) fn check_touched(doc: &Document, touched: &BTreeSet<NodeId>) -> Result<(), DocError> {
    touched.iter().try_for_each(|&id| check_table_positions(doc, id).map_err(DocError::Table))
}

#[cfg(test)]
#[path = "table_tests.rs"]
mod tests;
