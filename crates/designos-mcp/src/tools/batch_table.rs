//! Batch `table` operation for `designos_apply_batch`.
//!
//! Dispatches `create`, `set_cell_text`, `insert_rows`, `insert_cols`, `delete_rows`,
//! `delete_cols`, `move_row`, `move_col`, `resize_row`, `resize_col`, `merge`, `unmerge`,
//! and `resize` onto `designos_doc::table_ops` inside the batch's single trial clone and undo group.

use crate::tools::batch::{op_failed_error, validate_ref_pre_run, wrap_op_err, NodeRef};
use crate::tools::bounded_text;
use crate::tools::errors::{ErrorCode, ToolError};
use designos_doc::table_ops::{
    create_table, delete_cols, delete_rows, insert_cols, insert_rows, merge, move_col, move_row, resize_col, resize_row, resize_table, set_cell_text, unmerge,
    TABLE_MAX_SIZE,
};
use designos_doc::{
    Command, Document, DocumentKind, NodeId, NodeKind, TableOpError, TableProps, TableStyle, TrackId, Vec2, TABLE_DEFAULT_COLUMN_WIDTH,
    TABLE_DEFAULT_ROW_HEIGHT, TABLE_MAX_TRACKS, TABLE_MIN_TRACK,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeSet;

pub const TABLE_ACTIONS: &[&str] = &[
    "create",
    "set_cell_text",
    "insert_rows",
    "insert_cols",
    "delete_rows",
    "delete_cols",
    "move_row",
    "move_col",
    "resize_row",
    "resize_col",
    "merge",
    "unmerge",
    "resize",
];

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TableOp {
    #[allow(dead_code)]
    pub op: String,
    #[serde(rename = "as")]
    pub alias: Option<String>,
    pub action: String,
    #[serde(alias = "parent_id")]
    pub parent: Option<NodeRef>,
    #[serde(alias = "node_id")]
    pub node: Option<NodeRef>,
    pub rows: Option<i64>,
    pub cols: Option<i64>,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub col_width: Option<f32>,
    pub row_height: Option<f32>,
    pub name: Option<String>,
    pub index: Option<usize>,
    pub row: Option<i64>,
    pub col: Option<i64>,
    pub text: Option<String>,
    pub at: Option<i64>,
    pub count: Option<i64>,
    pub from_index: Option<i64>,
    pub to_index: Option<i64>,
    pub size: Option<f32>,
    pub row_span: Option<i64>,
    pub col_span: Option<i64>,
}

/// Parses `"r<row>c<col>"` (0-based indices) for `@<as>/r<row>c<col>` handle paths.
pub fn parse_table_cell_segment(seg: &str) -> Option<(usize, usize)> {
    let rest = seg.strip_prefix('r')?;
    let (r_str, c_str) = rest.split_once('c')?;
    if r_str.is_empty() || c_str.is_empty() {
        return None;
    }
    Some((r_str.parse().ok()?, c_str.parse().ok()?))
}

/// Resolves a 0-based `(row, col)` in `table_id` to its cell `NodeId` (or covering merge anchor).
pub fn resolve_table_cell(doc: &Document, table_id: NodeId, row: usize, col: usize) -> Option<NodeId> {
    let table_node = doc.get(table_id)?;
    let NodeKind::Table(props) = &table_node.kind else { return None };
    if row >= props.rows.len() || col >= props.cols.len() {
        return None;
    }
    let (anchor_row, anchor_col) = covering_merge_tracks(props, row, col).unwrap_or((props.rows[row].id, props.cols[col].id));
    table_node
        .children
        .iter()
        .copied()
        .find(|&cid| matches!(doc.get(cid).map(|n| &n.kind), Some(NodeKind::TableCell(cp)) if cp.row == anchor_row && cp.col == anchor_col))
}

fn covering_merge_tracks(props: &TableProps, row: usize, col: usize) -> Option<(TrackId, TrackId)> {
    for m in &props.merges {
        let (Some(r0), Some(c0)) = (props.row_index(m.row), props.col_index(m.col)) else { continue };
        let r1 = r0 + m.row_span.max(1) as usize;
        let c1 = c0 + m.col_span.max(1) as usize;
        if (r0..r1).contains(&row) && (c0..c1).contains(&col) {
            return Some((m.row, m.col));
        }
    }
    None
}

fn req_non_neg(v: Option<i64>, idx: usize, field: &str) -> Result<usize, ToolError> {
    let path = format!("/ops/{idx}/{field}");
    let raw = v.ok_or_else(|| op_failed_error(idx, "table", ErrorCode::MissingField, &path, format!("missing integer field '{field}'")))?;
    usize::try_from(raw).map_err(|_| op_failed_error(idx, "table", ErrorCode::ValueOutOfRange, &path, format!("'{field}' must be a non-negative integer")))
}

fn req_pos(v: Option<i64>, idx: usize, field: &str) -> Result<usize, ToolError> {
    let path = format!("/ops/{idx}/{field}");
    let raw = v.ok_or_else(|| op_failed_error(idx, "table", ErrorCode::MissingField, &path, format!("missing integer field '{field}'")))?;
    if raw < 1 {
        return Err(op_failed_error(idx, "table", ErrorCode::ValueOutOfRange, &path, format!("'{field}' must be >= 1")));
    }
    Ok(raw as usize)
}

pub fn parse_and_validate(val: Value, raw_op: &Value, idx: usize, declared_as: &BTreeSet<String>, has_prev: bool) -> Result<TableOp, ToolError> {
    let op_path = format!("/ops/{idx}");
    let a: TableOp = serde_json::from_value(val).map_err(|e| {
        let cause = crate::tools::errors::from_serde_error_with_args(&e, &op_path, false, Some(raw_op));
        ToolError::batch_op_failed(idx, "table", cause)
    })?;
    if !TABLE_ACTIONS.contains(&a.action.as_str()) {
        let cause = ToolError::new(
            ErrorCode::InvalidArguments,
            format!("{op_path}/action"),
            format!("unsupported table action '{}'; choose one of {}", a.action, TABLE_ACTIONS.join(", ")),
        )
        .with_valid_options(TABLE_ACTIONS.to_vec());
        return Err(ToolError::batch_op_failed(idx, "table", cause));
    }
    if a.action == "create" {
        if let Some(p) = &a.parent {
            if !matches!(p, NodeRef::Id(0)) {
                validate_ref_pre_run(p, declared_as, has_prev, idx, "table", "parent")?;
            }
        }
        let rows = req_pos(a.rows, idx, "rows")?;
        let cols = req_pos(a.cols, idx, "cols")?;
        if rows > TABLE_MAX_TRACKS {
            return Err(op_failed_error(
                idx,
                "table",
                ErrorCode::ValueOutOfRange,
                &format!("{op_path}/rows"),
                format!("rows must be between 1 and {TABLE_MAX_TRACKS}"),
            ));
        }
        if cols > TABLE_MAX_TRACKS {
            return Err(op_failed_error(
                idx,
                "table",
                ErrorCode::ValueOutOfRange,
                &format!("{op_path}/cols"),
                format!("cols must be between 1 and {TABLE_MAX_TRACKS}"),
            ));
        }
        for (f, v) in [("x", a.x), ("y", a.y)] {
            if !v.is_finite() || v.abs() > TABLE_MAX_SIZE {
                return Err(op_failed_error(idx, "table", ErrorCode::ValueOutOfRange, &format!("{op_path}/{f}"), format!("{f} must be within +/-1000000")));
            }
        }
        for (f, v) in [("width", a.width), ("height", a.height)] {
            if let Some(val) = v {
                if !(val.is_finite() && val > 0.0 && val <= TABLE_MAX_SIZE) {
                    return Err(op_failed_error(
                        idx,
                        "table",
                        ErrorCode::ValueOutOfRange,
                        &format!("{op_path}/{f}"),
                        format!("{f} must be > 0 and <= 1000000"),
                    ));
                }
            }
        }
        if let Some(cw) = a.col_width {
            if !cw.is_finite() || !(TABLE_MIN_TRACK..=TABLE_MAX_SIZE).contains(&cw) {
                return Err(op_failed_error(
                    idx,
                    "table",
                    ErrorCode::ValueOutOfRange,
                    &format!("{op_path}/col_width"),
                    format!("col_width must be >= {TABLE_MIN_TRACK}"),
                ));
            }
        }
        if let Some(rh) = a.row_height {
            if !rh.is_finite() || !(TABLE_MIN_TRACK..=TABLE_MAX_SIZE).contains(&rh) {
                return Err(op_failed_error(
                    idx,
                    "table",
                    ErrorCode::ValueOutOfRange,
                    &format!("{op_path}/row_height"),
                    format!("row_height must be >= {TABLE_MIN_TRACK}"),
                ));
            }
        }
        if let Some(name) = &a.name {
            bounded_text("name", name, 1024).map_err(|e| wrap_op_err(idx, "table", &op_path, e))?;
        }
        return Ok(a);
    }

    let nr = a.node.as_ref().ok_or_else(|| op_failed_error(idx, "table", ErrorCode::MissingField, &format!("{op_path}/node"), "missing 'node' field"))?;
    validate_ref_pre_run(nr, declared_as, has_prev, idx, "table", "node")?;

    match a.action.as_str() {
        "set_cell_text" => {
            req_non_neg(a.row, idx, "row")?;
            req_non_neg(a.col, idx, "col")?;
            let t = a.text.as_deref().ok_or_else(|| op_failed_error(idx, "table", ErrorCode::MissingField, &format!("{op_path}/text"), "missing 'text'"))?;
            bounded_text("text", t, 100_000).map_err(|e| wrap_op_err(idx, "table", &op_path, e))?;
        }
        "insert_rows" | "insert_cols" | "delete_rows" | "delete_cols" => {
            req_non_neg(a.at, idx, "at")?;
            if let Some(c) = a.count {
                if !(1..=TABLE_MAX_TRACKS as i64).contains(&c) {
                    return Err(op_failed_error(
                        idx,
                        "table",
                        ErrorCode::ValueOutOfRange,
                        &format!("{op_path}/count"),
                        format!("count must be between 1 and {TABLE_MAX_TRACKS}"),
                    ));
                }
            }
        }
        "move_row" | "move_col" => {
            req_non_neg(a.from_index, idx, "from_index")?;
            req_non_neg(a.to_index, idx, "to_index")?;
        }
        "resize_row" | "resize_col" => {
            req_non_neg(a.at, idx, "at")?;
            let sz = a.size.ok_or_else(|| op_failed_error(idx, "table", ErrorCode::MissingField, &format!("{op_path}/size"), "missing 'size'"))?;
            if !sz.is_finite() || !(TABLE_MIN_TRACK..=TABLE_MAX_SIZE).contains(&sz) {
                return Err(op_failed_error(
                    idx,
                    "table",
                    ErrorCode::ValueOutOfRange,
                    &format!("{op_path}/size"),
                    format!("size must be >= {TABLE_MIN_TRACK}"),
                ));
            }
        }
        "merge" => {
            req_non_neg(a.row, idx, "row")?;
            req_non_neg(a.col, idx, "col")?;
            let rs = req_pos(a.row_span, idx, "row_span")?;
            let cs = req_pos(a.col_span, idx, "col_span")?;
            if rs.saturating_mul(cs) < 2 {
                return Err(op_failed_error(
                    idx,
                    "table",
                    ErrorCode::InvalidArguments,
                    &op_path,
                    "merge requires at least 2 cells (row_span * col_span >= 2). Increase row_span or col_span.",
                ));
            }
        }
        "unmerge" => {
            req_non_neg(a.row, idx, "row")?;
            req_non_neg(a.col, idx, "col")?;
        }
        "resize" => {
            if a.width.is_none() && a.height.is_none() {
                return Err(op_failed_error(idx, "table", ErrorCode::MissingField, &op_path, "resize requires at least one of 'width' or 'height'"));
            }
            for (f, v) in [("width", a.width), ("height", a.height)] {
                if let Some(val) = v {
                    if !(val.is_finite() && val > 0.0 && val <= TABLE_MAX_SIZE) {
                        return Err(op_failed_error(
                            idx,
                            "table",
                            ErrorCode::ValueOutOfRange,
                            &format!("{op_path}/{f}"),
                            format!("{f} must be > 0 and <= 1000000"),
                        ));
                    }
                }
            }
        }
        _ => {}
    }
    Ok(a)
}

fn check_file_kind(doc: &Document, idx: usize) -> Result<(), ToolError> {
    if doc.kind == DocumentKind::Deck {
        return Err(op_failed_error(
            idx,
            "table",
            ErrorCode::InvalidArguments,
            &format!("/ops/{idx}"),
            "Tables are supported only in Design and Board documents, not Slides. Open or create a Design or Board document first.",
        ));
    }
    Ok(())
}

fn is_valid_table_parent(doc: &Document, parent: NodeId) -> bool {
    let Some(p) = doc.get(parent) else { return false };
    if !matches!(p.kind, NodeKind::Page { .. } | NodeKind::Frame(_) | NodeKind::Group | NodeKind::Section) {
        return false;
    }
    let mut cur = p.parent;
    while let Some(anc_id) = cur {
        let Some(anc) = doc.get(anc_id) else { break };
        if matches!(
            anc.kind,
            NodeKind::Component(_)
                | NodeKind::ComponentSet(_)
                | NodeKind::Instance { .. }
                | NodeKind::BooleanOp(_)
                | NodeKind::Table(_)
                | NodeKind::TableCell(_)
        ) {
            return false;
        }
        cur = anc.parent;
    }
    true
}

pub fn execute_create(doc: &Document, parent: NodeId, a: &TableOp, idx: usize) -> Result<(NodeId, Command), ToolError> {
    check_file_kind(doc, idx)?;
    let p_path = format!("/ops/{idx}/parent");
    if doc.get(parent).is_none() {
        return Err(op_failed_error(idx, "table", ErrorCode::NodeNotFound, &p_path, format!("node {} not found", parent.0)));
    }
    if !is_valid_table_parent(doc, parent) {
        return Err(op_failed_error(
            idx,
            "table",
            ErrorCode::NotAContainer,
            &p_path,
            "A table can only be created inside a page, frame, group, or section (not a component, instance, boolean op, or table).",
        ));
    }
    let rows = a.rows.unwrap_or(3) as usize;
    let cols = a.cols.unwrap_or(3) as usize;
    let col_w = a.col_width.unwrap_or_else(|| a.width.map(|w| (w / cols as f32).max(TABLE_MIN_TRACK)).unwrap_or(TABLE_DEFAULT_COLUMN_WIDTH));
    let row_h = a.row_height.unwrap_or_else(|| a.height.map(|h| (h / rows as f32).max(TABLE_MIN_TRACK)).unwrap_or(TABLE_DEFAULT_ROW_HEIGHT));
    let style = TableStyle { column_width: col_w, row_height: row_h, ..TableStyle::default() };
    let mut cmd = create_table(doc, parent, Vec2::new(a.x, a.y), rows, cols, &style).map_err(|e| map_table_err(idx, e))?;
    let Command::Insert { index, nodes, .. } = &mut cmd else {
        return Err(op_failed_error(idx, "table", ErrorCode::InternalError, &format!("/ops/{idx}"), "create_table did not return Insert"));
    };
    if let Some(name) = &a.name {
        nodes[0].name = name.clone();
    }
    if let Some(req_idx) = a.index {
        let max_idx = *index;
        *index = req_idx.min(max_idx);
    }
    let table_id = nodes[0].id;
    Ok((table_id, cmd))
}

pub fn execute_mutation(doc: &Document, table_id: NodeId, a: &TableOp, idx: usize) -> Result<Option<Command>, ToolError> {
    check_file_kind(doc, idx)?;
    let op_path = format!("/ops/{idx}");
    let node_path = format!("{op_path}/node");
    let table_node =
        doc.get(table_id).ok_or_else(|| op_failed_error(idx, "table", ErrorCode::NodeNotFound, &node_path, format!("node {} not found", table_id.0)))?;
    let NodeKind::Table(props) = &table_node.kind else {
        return Err(op_failed_error(
            idx,
            "table",
            ErrorCode::InvalidArguments,
            &node_path,
            format!("node {} is not a Table; pass a table node ID or '@<as>' handle", table_id.0),
        ));
    };
    let rows_len = props.rows.len();
    let cols_len = props.cols.len();

    let res = match a.action.as_str() {
        "set_cell_text" => {
            let r = a.row.unwrap() as usize;
            let c = a.col.unwrap() as usize;
            if r >= rows_len || c >= cols_len {
                return Err(op_failed_error(
                    idx,
                    "table",
                    ErrorCode::ValueOutOfRange,
                    &op_path,
                    format!("cell ({r}, {c}) is out of bounds for {rows_len}x{cols_len} table"),
                ));
            }
            set_cell_text(doc, table_id, r, c, a.text.as_deref().unwrap_or(""))
        }
        "insert_rows" => {
            let at = a.at.unwrap() as usize;
            let count = a.count.unwrap_or(1) as usize;
            if at > rows_len || rows_len + count > TABLE_MAX_TRACKS {
                return Err(op_failed_error(
                    idx,
                    "table",
                    ErrorCode::ValueOutOfRange,
                    &op_path,
                    format!("insert_rows at={at}, count={count} exceeds 0..={rows_len} or max {TABLE_MAX_TRACKS} rows"),
                ));
            }
            insert_rows(doc, table_id, at, count)
        }
        "insert_cols" => {
            let at = a.at.unwrap() as usize;
            let count = a.count.unwrap_or(1) as usize;
            if at > cols_len || cols_len + count > TABLE_MAX_TRACKS {
                return Err(op_failed_error(
                    idx,
                    "table",
                    ErrorCode::ValueOutOfRange,
                    &op_path,
                    format!("insert_cols at={at}, count={count} exceeds 0..={cols_len} or max {TABLE_MAX_TRACKS} cols"),
                ));
            }
            insert_cols(doc, table_id, at, count)
        }
        "delete_rows" => {
            let at = a.at.unwrap() as usize;
            let count = a.count.unwrap_or(1) as usize;
            if at >= rows_len || at + count > rows_len {
                return Err(op_failed_error(
                    idx,
                    "table",
                    ErrorCode::ValueOutOfRange,
                    &op_path,
                    format!("delete_rows range {at}..{} out of 0..{rows_len}", at + count),
                ));
            }
            if count >= rows_len {
                return Err(op_failed_error(
                    idx,
                    "table",
                    ErrorCode::InvalidArguments,
                    &op_path,
                    "A table must keep at least one row. Delete fewer rows, or delete the table node.",
                ));
            }
            let ids: Vec<TrackId> = props.rows[at..at + count].iter().map(|t| t.id).collect();
            delete_rows(doc, table_id, &ids)
        }
        "delete_cols" => {
            let at = a.at.unwrap() as usize;
            let count = a.count.unwrap_or(1) as usize;
            if at >= cols_len || at + count > cols_len {
                return Err(op_failed_error(
                    idx,
                    "table",
                    ErrorCode::ValueOutOfRange,
                    &op_path,
                    format!("delete_cols range {at}..{} out of 0..{cols_len}", at + count),
                ));
            }
            if count >= cols_len {
                return Err(op_failed_error(
                    idx,
                    "table",
                    ErrorCode::InvalidArguments,
                    &op_path,
                    "A table must keep at least one column. Delete fewer columns, or delete the table node.",
                ));
            }
            let ids: Vec<TrackId> = props.cols[at..at + count].iter().map(|t| t.id).collect();
            delete_cols(doc, table_id, &ids)
        }
        "move_row" => {
            let from = a.from_index.unwrap() as usize;
            let to = a.to_index.unwrap() as usize;
            if from >= rows_len || to >= rows_len {
                return Err(op_failed_error(
                    idx,
                    "table",
                    ErrorCode::ValueOutOfRange,
                    &op_path,
                    format!("move_row from_index={from}, to_index={to} out of 0..{rows_len}"),
                ));
            }
            move_row(doc, table_id, props.rows[from].id, to)
        }
        "move_col" => {
            let from = a.from_index.unwrap() as usize;
            let to = a.to_index.unwrap() as usize;
            if from >= cols_len || to >= cols_len {
                return Err(op_failed_error(
                    idx,
                    "table",
                    ErrorCode::ValueOutOfRange,
                    &op_path,
                    format!("move_col from_index={from}, to_index={to} out of 0..{cols_len}"),
                ));
            }
            move_col(doc, table_id, props.cols[from].id, to)
        }
        "resize_row" => {
            let at = a.at.unwrap() as usize;
            if at >= rows_len {
                return Err(op_failed_error(idx, "table", ErrorCode::ValueOutOfRange, &op_path, format!("resize_row at={at} out of 0..{rows_len}")));
            }
            resize_row(doc, table_id, props.rows[at].id, a.size.unwrap())
        }
        "resize_col" => {
            let at = a.at.unwrap() as usize;
            if at >= cols_len {
                return Err(op_failed_error(idx, "table", ErrorCode::ValueOutOfRange, &op_path, format!("resize_col at={at} out of 0..{cols_len}")));
            }
            resize_col(doc, table_id, props.cols[at].id, a.size.unwrap())
        }
        "merge" => {
            let r = a.row.unwrap() as usize;
            let c = a.col.unwrap() as usize;
            let rs = a.row_span.unwrap() as usize;
            let cs = a.col_span.unwrap() as usize;
            if r >= rows_len || c >= cols_len || r + rs > rows_len || c + cs > cols_len {
                return Err(op_failed_error(
                    idx,
                    "table",
                    ErrorCode::ValueOutOfRange,
                    &op_path,
                    format!("merge ({r}, {c}, {rs}x{cs}) out of {rows_len}x{cols_len}"),
                ));
            }
            merge(doc, table_id, (r, c, r + rs - 1, c + cs - 1))
        }
        "unmerge" => {
            let r = a.row.unwrap() as usize;
            let c = a.col.unwrap() as usize;
            if r >= rows_len || c >= cols_len {
                return Err(op_failed_error(idx, "table", ErrorCode::ValueOutOfRange, &op_path, format!("unmerge ({r}, {c}) out of {rows_len}x{cols_len}")));
            }
            let Some((ar, ac)) = covering_merge_tracks(props, r, c) else {
                return Err(op_failed_error(
                    idx,
                    "table",
                    ErrorCode::InvalidArguments,
                    &op_path,
                    format!("Cell at ({r}, {c}) is not merged. Specify a merged cell to unmerge."),
                ));
            };
            let anchor_id = table_node
                .children
                .iter()
                .copied()
                .find(|&cid| matches!(doc.get(cid).map(|n| &n.kind), Some(NodeKind::TableCell(cp)) if cp.row == ar && cp.col == ac))
                .ok_or_else(|| op_failed_error(idx, "table", ErrorCode::InvalidArguments, &op_path, "Merged anchor cell not found in table."))?;
            unmerge(doc, table_id, anchor_id)
        }
        "resize" => {
            let target_size = Vec2::new(a.width.unwrap_or(table_node.size.x), a.height.unwrap_or(table_node.size.y));
            resize_table(doc, table_id, target_size)
        }
        _ => unreachable!(),
    };

    match res {
        Ok(cmd) => Ok(Some(cmd)),
        Err(TableOpError::NoChange(_)) => Ok(None),
        Err(e) => Err(map_table_err(idx, e)),
    }
}

fn map_table_err(idx: usize, err: TableOpError) -> ToolError {
    let path = format!("/ops/{idx}");
    let code = match &err {
        TableOpError::IndexOutOfRange(_) | TableOpError::TooLarge(_) => ErrorCode::ValueOutOfRange,
        TableOpError::BadParent(_) => ErrorCode::NotAContainer,
        TableOpError::NotATable(_)
        | TableOpError::SplitsMerge(_)
        | TableOpError::OverlapsMerge(_)
        | TableOpError::BrokenTable(_)
        | TableOpError::NoChange(_) => ErrorCode::InvalidArguments,
    };
    op_failed_error(idx, "table", code, &path, err.to_string())
}
