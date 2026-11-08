//! Editing an existing table on the canvas: selecting cells, typing, Tab, rows and columns, merge.
//!
//! The model and every edit live in `designos_doc::table_ops`; this module only turns pointer and key
//! input into one of those commands and runs it as one undo step. A cell is a real child node, so a
//! block of cells is a selection of cell ids and a whole row is the block that spans every column.

pub mod grips;
pub mod menu;
pub mod overlay;
pub mod select;

use crate::app::App;
use crate::edit::Tx;
use designos_doc::table_ops::{self, TableOpError};
use designos_doc::{Affine, Command, Document, NodeId, NodeKind, TableProps, TrackId, Vec2};

/// Where a cell sits: its table, and its first and last row and column (inclusive).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellSpan {
    pub id: NodeId,
    pub r0: usize,
    pub c0: usize,
    pub r1: usize,
    pub c1: usize,
}

/// A table as drawn: track edges in table-local px, derived from the cells' stored geometry.
pub struct TableGeo {
    pub props: TableProps,
    pub cells: Vec<CellSpan>,
    /// `cols + 1` x offsets and `rows + 1` y offsets.
    pub col_x: Vec<f32>,
    pub row_y: Vec<f32>,
    pub world: Affine,
}

/// Fills the unknown edges (a row no cell starts on) in proportion to the stored track sizes.
fn fill_edges(known: &mut [Option<f32>], sizes: &[f32]) {
    known[0].get_or_insert(0.0);
    let mut a = 0;
    for b in 1..known.len() {
        if known[b].is_none() {
            continue;
        }
        let (ya, yb) = (known[a].unwrap_or(0.0), known[b].unwrap_or(0.0));
        let total: f32 = sizes[a..b].iter().sum::<f32>().max(1e-3);
        let mut acc = 0.0;
        for i in a + 1..b {
            acc += sizes[i - 1];
            known[i] = Some(ya + (yb - ya) * acc / total);
        }
        a = b;
    }
    for i in a + 1..known.len() {
        let prev = known[i - 1].unwrap_or(0.0);
        known[i] = Some(prev + sizes[i - 1]);
    }
}

impl TableGeo {
    pub fn of(doc: &Document, table: NodeId) -> Option<Self> {
        let node = doc.get(table)?;
        let NodeKind::Table(props) = &node.kind else { return None };
        let (rows, cols) = (props.rows.len(), props.cols.len());
        let mut cells = Vec::new();
        let (mut ry, mut cx) = (vec![None; rows + 1], vec![None; cols + 1]);
        for &id in &node.children {
            let Some(n) = doc.get(id) else { continue };
            let NodeKind::TableCell(c) = &n.kind else { continue };
            let (Some(r0), Some(c0)) = (props.row_index(c.row), props.col_index(c.col)) else { continue };
            let m = props.merges.iter().find(|m| m.row == c.row && m.col == c.col);
            let (rs, cs) = m.map_or((1, 1), |m| ((m.row_span as usize).max(1), (m.col_span as usize).max(1)));
            let (r1, c1) = ((r0 + rs).min(rows) - 1, (c0 + cs).min(cols) - 1);
            ry[r0] = Some(n.transform.ty);
            ry[r1 + 1] = Some(n.transform.ty + n.size.y);
            cx[c0] = Some(n.transform.tx);
            cx[c1 + 1] = Some(n.transform.tx + n.size.x);
            cells.push(CellSpan { id, r0, c0, r1, c1 });
        }
        fill_edges(&mut ry, &props.rows.iter().map(|t| t.size).collect::<Vec<_>>());
        fill_edges(&mut cx, &props.cols.iter().map(|t| t.size).collect::<Vec<_>>());
        let flat = |v: Vec<Option<f32>>| v.into_iter().map(|x| x.unwrap_or(0.0)).collect();
        Some(Self { props: props.clone(), cells, col_x: flat(cx), row_y: flat(ry), world: doc.world_transform(table) })
    }

    pub fn span_of(&self, cell: NodeId) -> Option<CellSpan> {
        self.cells.iter().copied().find(|c| c.id == cell)
    }

    /// The cell whose block covers `(row, col)`.
    pub fn cell_covering(&self, row: usize, col: usize) -> Option<CellSpan> {
        self.cells.iter().copied().find(|c| (c.r0..=c.r1).contains(&row) && (c.c0..=c.c1).contains(&col))
    }

    /// Rows and columns for a table-local point, clamped to the table; `None` outside it.
    pub fn track_at(&self, local: Vec2) -> Option<(usize, usize)> {
        let find = |edges: &[f32], v: f32| (v >= edges[0] && v < *edges.last()?).then(|| edges.iter().rposition(|e| *e <= v)).flatten();
        Some((find(&self.row_y, local.y)?, find(&self.col_x, local.x)?))
    }

    /// Smallest block that contains `a` and `b` and cuts no merge.
    pub fn block(&self, a: CellSpan, b: CellSpan) -> (usize, usize, usize, usize) {
        let mut r = (a.r0.min(b.r0), a.c0.min(b.c0), a.r1.max(b.r1), a.c1.max(b.c1));
        loop {
            let mut grown = r;
            for c in &self.cells {
                let overlaps = c.r0 <= r.2 && c.r1 >= r.0 && c.c0 <= r.3 && c.c1 >= r.1;
                if overlaps {
                    grown = (grown.0.min(c.r0), grown.1.min(c.c0), grown.2.max(c.r1), grown.3.max(c.c1));
                }
            }
            if grown == r {
                return r;
            }
            r = grown;
        }
    }

    /// Cells whose block lies inside the rectangle, in row-major order.
    pub fn cells_in(&self, r: (usize, usize, usize, usize)) -> Vec<NodeId> {
        let mut v: Vec<&CellSpan> = self.cells.iter().filter(|c| c.r0 >= r.0 && c.r1 <= r.2 && c.c0 >= r.1 && c.c1 <= r.3).collect();
        v.sort_by_key(|c| (c.r0, c.c0));
        v.into_iter().map(|c| c.id).collect()
    }

    pub fn row_id(&self, i: usize) -> TrackId {
        self.props.rows[i].id
    }

    pub fn col_id(&self, i: usize) -> TrackId {
        self.props.cols[i].id
    }
}

/// The table that holds `id` (the table itself, or the table of a cell).
pub fn table_of(doc: &Document, id: NodeId) -> Option<NodeId> {
    let n = doc.get(id)?;
    match n.kind {
        NodeKind::Table(_) => Some(id),
        NodeKind::TableCell(_) => n.parent.filter(|p| matches!(doc.get(*p).map(|t| &t.kind), Some(NodeKind::Table(_)))),
        _ => None,
    }
}

/// The table and rectangle of rows and columns the selection covers, when it is cells of one table.
pub fn selected_block(app: &App) -> Option<(NodeId, (usize, usize, usize, usize))> {
    let first = *app.selection.first()?;
    if !matches!(app.doc.get(first)?.kind, NodeKind::TableCell(_)) {
        return None;
    }
    let table = table_of(&app.doc, first)?;
    let geo = TableGeo::of(&app.doc, table)?;
    let mut r: Option<(usize, usize, usize, usize)> = None;
    for id in &app.selection {
        let s = geo.span_of(*id)?;
        r = Some(r.map_or((s.r0, s.c0, s.r1, s.c1), |r| (r.0.min(s.r0), r.1.min(s.c0), r.2.max(s.r1), r.3.max(s.c1))));
    }
    r.map(|r| (table, r))
}

/// What the context menu and the keyboard can do with the selected cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableAction {
    InsertRowAbove,
    InsertRowBelow,
    InsertColLeft,
    InsertColRight,
    DeleteRow,
    DeleteCol,
    MergeCells,
    UnmergeCells,
    ClearContent,
}

impl TableAction {
    pub const ALL: [TableAction; 9] = [
        Self::InsertRowAbove,
        Self::InsertRowBelow,
        Self::InsertColLeft,
        Self::InsertColRight,
        Self::DeleteRow,
        Self::DeleteCol,
        Self::MergeCells,
        Self::UnmergeCells,
        Self::ClearContent,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::InsertRowAbove => "Insert row above",
            Self::InsertRowBelow => "Insert row below",
            Self::InsertColLeft => "Insert column left",
            Self::InsertColRight => "Insert column right",
            Self::DeleteRow => "Delete row",
            Self::DeleteCol => "Delete column",
            Self::MergeCells => "Merge cells",
            Self::UnmergeCells => "Unmerge cells",
            Self::ClearContent => "Clear content",
        }
    }
}

/// Why an action did nothing: `Nothing` leaves the table as it is and is not worth a message, `Refused` is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    Nothing(String),
    Refused(String),
}

impl Refusal {
    pub fn message(&self) -> &str {
        match self {
            Self::Nothing(m) | Self::Refused(m) => m,
        }
    }
}

fn refusal(e: TableOpError) -> Refusal {
    match e {
        TableOpError::NoChange(m) => Refusal::Nothing(m),
        other => Refusal::Refused(other.to_string()),
    }
}

/// The command `action` would run on the current selection, or why it cannot. Pure: the menu uses it to
/// decide which entries are enabled, so an entry is never live without effect.
pub fn plan(app: &App, action: TableAction) -> Result<(Command, &'static str), Refusal> {
    let (table, (r0, c0, r1, c1)) = selected_block(app).ok_or_else(|| Refusal::Refused("Select a cell of a table first.".into()))?;
    let doc = &app.doc;
    let geo = TableGeo::of(doc, table).ok_or_else(|| Refusal::Refused("This table is broken. Undo the last edit.".into()))?;
    let (rows, cols) = (geo.props.rows.len(), geo.props.cols.len());
    let ids = |range: std::ops::RangeInclusive<usize>, rows_axis: bool| -> Vec<TrackId> {
        range.map(|i| if rows_axis { geo.row_id(i) } else { geo.col_id(i) }).collect()
    };
    let cmd = match action {
        TableAction::InsertRowAbove => table_ops::insert_rows(doc, table, r0, 1),
        TableAction::InsertRowBelow => table_ops::insert_rows(doc, table, r1 + 1, 1),
        TableAction::InsertColLeft => table_ops::insert_cols(doc, table, c0, 1),
        TableAction::InsertColRight => table_ops::insert_cols(doc, table, c1 + 1, 1),
        TableAction::DeleteRow => table_ops::delete_rows(doc, table, &ids(r0..=r1, true)),
        TableAction::DeleteCol => table_ops::delete_cols(doc, table, &ids(c0..=c1, false)),
        TableAction::MergeCells => table_ops::merge(doc, table, (r0, c0, r1, c1)),
        TableAction::UnmergeCells => {
            let anchor = app.selection.iter().copied().find(|id| geo.span_of(*id).is_some_and(|s| (s.r0, s.c0) != (s.r1, s.c1)));
            match anchor {
                Some(a) => table_ops::unmerge(doc, table, a),
                None => Err(TableOpError::NoChange("This cell is not merged.".into())),
            }
        }
        TableAction::ClearContent => clear_content(doc, table, &geo, &app.selection),
    }
    .map_err(refusal)?;
    let _ = (rows, cols);
    Ok((cmd, action.label()))
}

fn clear_content(doc: &Document, table: NodeId, geo: &TableGeo, sel: &[NodeId]) -> Result<Command, TableOpError> {
    let mut cmds = Vec::new();
    for id in sel {
        let Some(n) = doc.get(*id) else { continue };
        if let NodeKind::TableCell(c) = &n.kind {
            if !c.text.content.is_empty() {
                let mut nn = n.clone();
                if let NodeKind::TableCell(c) = &mut nn.kind {
                    c.text.content.clear();
                    c.text.runs.clear();
                    c.text.lists.clear();
                }
                cmds.push(Command::Update { node: Box::new(nn) });
            }
        }
    }
    if cmds.is_empty() {
        return Err(TableOpError::NoChange("The selected cells are already empty.".into()));
    }
    let _ = (table, geo);
    Ok(Command::Batch(cmds))
}

/// Runs `cmd` as one undo step with `label`. A refused command leaves the document as it was and says why.
pub fn commit(app: &mut App, table: NodeId, cmd: Command, label: &str) -> bool {
    let parent = app.doc.get(table).and_then(|n| n.parent);
    let mut tx = Tx::new();
    if let Err(e) = tx.cmd(&mut app.doc, cmd) {
        tx.cancel(&mut app.doc);
        app.show_toast(e.to_string(), true);
        return false;
    }
    // rows that grew for text shrink back to the height the user set once the text is gone
    for fix in designos_doc::table::relayout(&app.doc, table) {
        if let Err(e) = tx.cmd(&mut app.doc, fix) {
            tx.cancel(&mut app.doc);
            app.show_toast(e.to_string(), true);
            return false;
        }
    }
    reflow_parents(app, &mut tx, table, parent);
    app.canvas.dirty = true;
    app.run_tx_labeled(tx, Some(label.to_string()))
}

/// Runs a menu or keyboard action on the selected cells. `NoChange` does nothing and records no step.
pub fn run(app: &mut App, action: TableAction) -> bool {
    match plan(app, action) {
        Ok((cmd, label)) => {
            let sel_table = selected_block(app).map(|b| b.0);
            let ok = commit(app, sel_table.unwrap_or(NodeId(0)), cmd, label);
            if ok {
                after_action(app, action, sel_table);
            }
            ok
        }
        Err(Refusal::Nothing(_)) => false,
        Err(Refusal::Refused(msg)) => {
            app.show_toast(msg, true);
            false
        }
    }
}

/// Keeps the selection on what exists after the step: deleted cells leave the table selected.
fn after_action(app: &mut App, action: TableAction, table: Option<NodeId>) {
    let Some(table) = table else { return };
    let gone = app.selection.iter().any(|id| app.doc.get(*id).is_none());
    if gone || matches!(action, TableAction::DeleteRow | TableAction::DeleteCol) {
        if app.doc.get(table).is_some() {
            app.select_only(table);
        } else {
            app.clear_selection();
        }
    }
    app.sanitize();
}

/// The open table drag, when there is one.
pub enum TableDragView<'a> {
    None,
    Drag(&'a grips::TableDrag),
}

pub fn drag_view(app: &App) -> TableDragView<'_> {
    match &app.canvas.inter {
        crate::canvas::Interaction::Table(d) => TableDragView::Drag(d),
        _ => TableDragView::None,
    }
}

/// Lays out the auto-layout frames around a table whose size changed, into the same transaction, so a
/// Hug frame grows with its table and one undo gives both back.
pub fn reflow_parents(app: &mut App, tx: &mut Tx, table: NodeId, parent: Option<NodeId>) {
    if app.doc.get(table).is_some() {
        crate::layout::relayout_up(&mut app.doc, &mut tx.ed, table);
    } else if let Some(parent) = parent {
        // the table is gone: its parent is laid out too, not only what is above it
        crate::layout::relayout_around(&mut app.doc, &mut tx.ed, parent);
    }
}
