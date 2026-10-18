//! Grid tracks and cell anchors in Figma's own shape.
//!
//! Figma keys every track by a GUID: `gridColumns` / `gridRows` map the track ids to a fractional-index position,
//! `gridColumnsSizing` / `gridRowsSizing` map the same ids to a size, and a child's `gridColumnAnchor` /
//! `gridRowAnchor` is the id of its parent's track. Studio keeps tracks by index, so the exporter gives each track
//! of each grid a fresh id ([`allocate`]) and the importer maps the ids back to indexes in position order.
//!
//! Three things have no home in Figma's schema and travel as carrier entries (see [`super::carrier`]): an
//! auto-track that is more than "rows" (`gridAutoTracks` is only `NONE` / `ROWS`), a track count past
//! [`TRACK_ENTRY_CAP`], and a cell anchor that names no track of its parent.

use super::carrier::Carried;
use super::{finite_or, fl, format_grid_auto_tracks, grid_row_tracks_for_export, ExportCtx};
use crate::kiwi::KiwiValue as V;
use crate::testkit::{en, guid, msg};
use designos_doc::*;
use serde_json::{json, Value};

/// Tracks written per axis and grid. A grid with more tracks keeps its real count in a carrier entry.
const TRACK_ENTRY_CAP: usize = 256;
/// Session of the GUIDs given to tracks (nodes 1, styles 2, variables 3, component properties 4).
const TRACK_SESSION: u32 = 5;
/// Track ids of one grid frame; `*_total` is the real track count, `cols` / `rows` hold at most [`TRACK_ENTRY_CAP`].
#[derive(Default, Clone)]
pub(super) struct GridTrackIds {
    cols: Vec<(u32, u32)>,
    rows: Vec<(u32, u32)>,
    cols_total: usize,
    rows_total: usize,
}

/// Gives every track of every grid frame a GUID. Called once while the export context is built.
pub(super) fn allocate(cx: &mut ExportCtx) {
    let doc = cx.doc;
    for (&id, n) in &doc.nodes {
        let Some(al) = n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()) else { continue };
        let cols_total = al.grid_columns as usize;
        let rows_total = grid_row_tracks_for_export(doc, n, al).map_or(0, |(count, _)| count as usize);
        if cols_total == 0 && rows_total == 0 {
            continue;
        }
        let mut fresh = |total: usize| -> Vec<(u32, u32)> { (0..total.min(TRACK_ENTRY_CAP)).map(|_| cx.alloc_guid(TRACK_SESSION)).collect() };
        let ids = GridTrackIds { cols: fresh(cols_total), rows: fresh(rows_total), cols_total, rows_total };
        cx.grid_tracks.insert(id, ids);
    }
}

/// Fractional-index position of track `i` of `n`: one character from `!` while it fits, two characters otherwise,
/// so the strings sort in track order.
fn position(i: usize, n: usize) -> String {
    const FIRST: u8 = b'!';
    const SPAN: usize = 94;
    let ch = |k: usize| char::from(FIRST + k as u8);
    if n <= SPAN {
        ch(i).to_string()
    } else {
        format!("{}{}", ch(i / SPAN), ch(i % SPAN))
    }
}

fn position_map(ids: &[(u32, u32)]) -> V {
    let entries = ids.iter().enumerate().map(|(i, &(s, l))| msg(vec![("id", guid(s, l)), ("position", V::String(position(i, ids.len())))])).collect();
    msg(vec![("entries", V::Array(entries))])
}

fn sizing_function(t: GridTrack) -> V {
    let (ty, value) = match t {
        GridTrack::Flex(w) => ("FLEX", finite_or(w, 1.0)),
        GridTrack::Fixed(px) => ("FIXED", finite_or(px, 0.0)),
        GridTrack::Hug => ("HUG", 1.0),
    };
    msg(vec![("type", en(ty)), ("value", fl(value))])
}

/// A track is written with the same sizing as its minimum and maximum, as Figma does.
fn size_map(ids: &[(u32, u32)], tracks: &[GridTrack]) -> V {
    let entries = ids
        .iter()
        .zip(tracks)
        .map(|(&(s, l), &t)| msg(vec![("id", guid(s, l)), ("trackSize", msg(vec![("minSizing", sizing_function(t)), ("maxSizing", sizing_function(t))]))]))
        .collect();
    msg(vec![("entries", V::Array(entries))])
}

/// The grid fields of a frame (or of an override that restates its layout), appended to `out`; whatever the
/// schema cannot hold goes to `carry`.
pub(super) fn push_frame_fields(cx: &ExportCtx, id: NodeId, n: &Node, al: &AutoLayout, out: &mut Vec<(&'static str, V)>, carry: &mut Carried) {
    let ids = cx.grid_tracks.get(&id);
    if al.grid_columns > 0 {
        if let Some(ids) = ids {
            out.push(("gridColumns", position_map(&ids.cols)));
            out.push(("gridColumnsSizing", size_map(&ids.cols, al.grid_col_tracks.as_slice())));
            if ids.cols_total > ids.cols.len() {
                carry.push(("gridColumns", json!(al.grid_columns)));
            }
        }
    }
    if let (Some(ids), Some((count, tracks))) = (ids, grid_row_tracks_for_export(cx.doc, n, al)) {
        out.push(("gridRows", position_map(&ids.rows)));
        out.push(("gridRowsSizing", size_map(&ids.rows, &tracks)));
        if ids.rows_total > ids.rows.len() {
            carry.push(("gridRows", json!(count)));
        }
    }
    match al.grid_auto_tracks {
        GridAutoTracks::None => {}
        GridAutoTracks::Rows => out.push(("gridAutoTracks", en("ROWS"))),
        GridAutoTracks::Track(_) => {
            out.push(("gridAutoTracks", en("ROWS")));
            if let Some(text) = format_grid_auto_tracks(al.grid_auto_tracks) {
                carry.push(("gridAutoTracks", json!(text)));
            }
        }
    }
}

/// The cell anchors and spans of a grid child. `parent` holds the track ids of the grid the child sits in.
pub(super) fn push_child_fields(parent: Option<&GridTrackIds>, lc: &LayoutChild, out: &mut Vec<(&'static str, V)>, carry: &mut Carried) {
    let anchor = |index: Option<u16>, ids: Option<&Vec<(u32, u32)>>, total: usize| -> Result<V, Value> {
        let index = index.expect("anchor present");
        match ids.filter(|v| v.len() == total).and_then(|v| v.get(usize::from(index))) {
            Some(&(s, l)) => Ok(guid(s, l)),
            None => Err(json!(index)),
        }
    };
    if lc.grid_column.is_some() {
        match anchor(lc.grid_column, parent.map(|p| &p.cols), parent.map_or(0, |p| p.cols_total)) {
            Ok(g) => out.push(("gridColumnAnchor", g)),
            Err(v) => carry.push(("gridColumnAnchorIndex", v)),
        }
    }
    if lc.grid_row.is_some() {
        match anchor(lc.grid_row, parent.map(|p| &p.rows), parent.map_or(0, |p| p.rows_total)) {
            Ok(g) => out.push(("gridRowAnchor", g)),
            Err(v) => carry.push(("gridRowAnchorIndex", v)),
        }
    }
    if lc.col_span > 1 {
        out.push(("gridColumnSpan", V::Uint(u32::from(lc.col_span))));
    }
    if lc.row_span > 1 {
        out.push(("gridRowSpan", V::Uint(u32::from(lc.row_span))));
    }
}

/// Track ids that a reader resolves the anchors of an override on `n` (a node inside instance `inst`) against.
///
/// A reader looks first at the tracks the override entry of `n`'s parent restates (the instance itself always
/// does), then at the tracks of the grids in the file, so a parent that restates its layout names its own tracks
/// and any other parent names the tracks of the component's grid it was copied from.
pub(super) fn override_parent_ids<'a>(cx: &'a ExportCtx, inst: NodeId, n: &Node) -> Option<&'a GridTrackIds> {
    let parent_id = n.parent?;
    let parent = cx.doc.get(parent_id)?;
    let restates = parent_id == inst || parent.component.overrides.contains(&OverrideField::Layout);
    if restates {
        cx.grid_tracks.get(&parent_id)
    } else {
        parent.component.source.and_then(|s| cx.grid_tracks.get(&s))
    }
}

/// The ids of the grid `n` sits in, for a node written as a node change of its own.
pub(super) fn parent_ids<'a>(cx: &'a ExportCtx, n: &Node) -> Option<&'a GridTrackIds> {
    n.parent.and_then(|p| cx.grid_tracks.get(&p))
}
