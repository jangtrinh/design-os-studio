//! Live selection chip in the Agents panel header and dock bar (`#654`).
//!
//! Displays `page · N selected · first names` (or `page · none selected` when nothing is
//! selected), updates in the same frame as `app.selection`, writes nothing to the PTY when
//! selection changes, and on click writes `selection.spec.json` and pastes one fixed sentence
//! into the active agent tab without pressing Enter.

use crate::{theme, widgets};
use designos_doc::{Document, NodeId};
use egui::{Align2, FontId, Id, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use std::cell::RefCell;
use std::hash::{Hash, Hasher};

/// Maximum number of layer names shown in the chip before `+N` overflow.
pub const CHIP_MAX_NAMES: usize = 2;

/// Maximum number of selected layers whose full `NodeSpec` is inlined into `selection.spec.json`.
pub const CHIP_SPEC_CAP: usize = 10;

/// Maximum byte size of `selection.spec.json` before remaining inline `spec` entries are dropped.
pub const CHIP_FILE_BYTES_CAP: usize = 256 * 1024;

/// In-memory summary of the current page and canvas selection passed to the terminal painters.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SelectionSummary {
    pub page: String,
    pub count: usize,
    pub names: Vec<String>,
}

#[derive(Default)]
struct CachedSummary {
    key: Option<(u64, u64, u64)>,
    summary: SelectionSummary,
}

thread_local! {
    static SUMMARY_CACHE: RefCell<CachedSummary> = RefCell::new(CachedSummary::default());
}

fn first_line_trimmed(s: &str) -> String {
    s.lines().next().unwrap_or("").trim().to_string()
}

fn selection_hash(selection: &[NodeId]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    selection.hash(&mut h);
    h.finish()
}

impl SelectionSummary {
    /// Empty selection summary for `page`.
    pub fn empty(page: &str) -> Self {
        let clean = first_line_trimmed(page);
        Self { page: if clean.is_empty() { "Page 1".to_string() } else { clean }, count: 0, names: Vec::new() }
    }

    /// Builds a `SelectionSummary` from `doc`, `page`, and `selection`, reading at most
    /// [`CHIP_MAX_NAMES`] layer names so even a 10,000-layer selection only reads `K` names.
    pub fn from_doc(doc: &Document, page: NodeId, selection: &[NodeId]) -> Self {
        let page_name = doc.get(page).map(|n| first_line_trimmed(&n.name)).filter(|s| !s.is_empty()).unwrap_or_else(|| "Page 1".to_string());
        let mut names = Vec::with_capacity(selection.len().min(CHIP_MAX_NAMES));
        for id in selection {
            if names.len() >= CHIP_MAX_NAMES {
                break;
            }
            if let Some(node) = doc.get(*id) {
                let line = first_line_trimmed(&node.name);
                names.push(if line.is_empty() { node.kind.default_name().to_string() } else { line });
            }
        }
        Self { page: page_name, count: selection.len(), names }
    }

    /// Formats the full, un-elided chip label.
    pub fn chip_text(&self) -> String {
        selection_chip_text(&self.page, self.count, &self.names)
    }
}

/// Calls `f` with a cached [`SelectionSummary`] keyed by `(doc.mutation_epoch(), page.0, selection_hash)`,
/// allocating nothing when the selection, page, and document epoch are unchanged.
pub fn with_selection_summary<R>(doc: &Document, page: NodeId, selection: &[NodeId], f: impl FnOnce(&SelectionSummary) -> R) -> R {
    let key = (doc.mutation_epoch(), page.0, selection_hash(selection));
    SUMMARY_CACHE.with(|cell| {
        let mut cache = cell.borrow_mut();
        if cache.key != Some(key) {
            cache.summary = SelectionSummary::from_doc(doc, page, selection);
            cache.key = Some(key);
        }
        let summary_ref = &cache.summary;
        f(summary_ref)
    })
}

/// Formats the selection chip text:
/// - `count == 0` -> `"{page} · none selected"`
/// - `count > 0`  -> `"{page} · {count} selected · {first_names}"` (with `+{overflow}` when `count > 2`).
pub fn selection_chip_text<S: AsRef<str>>(page: &str, count: usize, names: &[S]) -> String {
    let page_clean = first_line_trimmed(page);
    let page_str = if page_clean.is_empty() { "Page 1" } else { page_clean.as_str() };
    if count == 0 {
        return format!("{page_str} · none selected");
    }
    let names_part = format_names_part(count, names);
    if names_part.is_empty() {
        format!("{page_str} · {count} selected")
    } else {
        format!("{page_str} · {count} selected · {names_part}")
    }
}

fn format_names_part<S: AsRef<str>>(count: usize, names: &[S]) -> String {
    let shown: Vec<String> = names.iter().take(CHIP_MAX_NAMES).map(|s| first_line_trimmed(s.as_ref())).filter(|s| !s.is_empty()).collect();
    if shown.is_empty() {
        return String::new();
    }
    let joined = shown.join(", ");
    if count > shown.len() {
        format!("{joined} +{}", count - shown.len())
    } else {
        joined
    }
}

/// Elides the chip text to fit `max_w` pixels in `font`, shrinking the layer names part first
/// and the page name second so `· N selected` remains visible as long as possible.
pub fn elide_selection_chip_text(p: &Painter, summary: &SelectionSummary, font: &FontId, max_w: f32) -> String {
    let full = summary.chip_text();
    if widgets::text_width(p, &full, font) <= max_w {
        return full;
    }
    let ellipsis_w = widgets::text_width(p, "…", font);
    let page_clean = first_line_trimmed(&summary.page);
    let page_str = if page_clean.is_empty() { "Page 1" } else { page_clean.as_str() };

    if summary.count == 0 {
        let suffix = " · none selected";
        let suffix_w = widgets::text_width(p, suffix, font);
        if max_w >= suffix_w + ellipsis_w {
            let elided_page = widgets::elide(p, page_str, font, max_w - suffix_w);
            return format!("{elided_page}{suffix}");
        }
        return widgets::elide(p, &full, font, max_w);
    }

    let names_part = format_names_part(summary.count, &summary.names);
    if !names_part.is_empty() {
        let prefix = format!("{page_str} · {} selected · ", summary.count);
        let prefix_w = widgets::text_width(p, &prefix, font);
        if max_w >= prefix_w + ellipsis_w {
            let elided_names = widgets::elide(p, &names_part, font, max_w - prefix_w);
            return format!("{prefix}{elided_names}");
        }
        let tail = format!(" · {} selected · …", summary.count);
        let tail_w = widgets::text_width(p, &tail, font);
        if max_w >= tail_w + ellipsis_w {
            let elided_page = widgets::elide(p, page_str, font, max_w - tail_w);
            return format!("{elided_page}{tail}");
        }
    }

    widgets::elide(p, &full, font, max_w)
}

/// Paints the `CONTROL_DENSE` (24px) selection pill inside `bounds` (either left-aligned or
/// right-aligned depending on `align_right`). Returns `(pill_rect, clicked)`.
pub fn paint_selection_chip(ui: &mut Ui, id_salt: &'static str, summary: &SelectionSummary, bounds: Rect, align_right: bool) -> (Rect, bool) {
    let p = ui.painter().clone();
    let font = theme::font_caption();
    let h = theme::CONTROL_DENSE;
    let max_pill_w = bounds.width().max(theme::TARGET_MIN);
    let max_text_w = (max_pill_w - 2.0 * theme::INSET_CONTROL).max(0.0);
    let shown = elide_selection_chip_text(&p, summary, &font, max_text_w);
    let text_w = widgets::text_width(&p, &shown, &font);
    let pill_w = (text_w + 2.0 * theme::INSET_CONTROL).clamp(theme::TARGET_MIN.min(max_pill_w), max_pill_w);
    let x = if align_right { bounds.right() - pill_w } else { bounds.left() };
    let y = bounds.center().y - h * 0.5;
    let pill_rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(pill_w, h));
    let radius = theme::cr_pill(h);

    let has_selection = summary.count > 0;
    let sense = if has_selection { Sense::click() } else { Sense::hover() };
    let resp = ui.interact(pill_rect, Id::new(id_salt), sense);

    let (fill, stroke, text_col) = if has_selection {
        let bg = if resp.hovered() { theme::BG_SELECTED_HOVER } else { theme::BG_SELECTED };
        (bg, None, theme::TEXT)
    } else {
        let bg = if resp.hovered() { theme::BG_HOVER } else { theme::BG_SECONDARY };
        (bg, Some(Stroke::new(theme::BORDER_WIDTH, theme::BORDER)), theme::TEXT_SECONDARY)
    };

    p.rect_filled(pill_rect, radius, fill);
    if let Some(st) = stroke {
        p.rect_stroke(pill_rect, radius, st, StrokeKind::Inside);
    }
    p.text(Pos2::new(pill_rect.left() + theme::INSET_CONTROL, pill_rect.center().y), Align2::LEFT_CENTER, &shown, font, text_col);

    widgets::record(ui, "agent-selection-chip", pill_rect);
    widgets::record(ui, id_salt, pill_rect);
    #[cfg(test)]
    ui.ctx().data_mut(|d| {
        d.insert_temp(Id::new(("test-text", "agent-selection-chip".to_string())), shown.clone());
        d.insert_temp(Id::new(("test-text", id_salt.to_string())), shown);
    });

    let clicked = has_selection && resp.clicked();
    let tip = if has_selection { "Send the selection to the agent" } else { "Select layers to send them" };
    let _ = resp.on_hover_text(tip);
    (pill_rect, clicked)
}

/// Builds the JSON payload written to `selection.spec.json` when the user clicks the chip:
/// `{ "page": { "id", "name" }, "count": N, "nodes": [ { "node_id", "name", "kind", "spec"? } ] }`.
///
/// Filters `selection` to layers that still exist in `doc` at click time. Includes `"spec"`
/// for at most [`CHIP_SPEC_CAP`] layers as long as the serialized JSON fits within
/// [`CHIP_FILE_BYTES_CAP`] bytes. Returns `None` if no selected layers exist in `doc`.
pub fn build_selection_spec_json(doc: &Document, page: NodeId, selection: &[NodeId]) -> Result<Option<(String, usize)>, String> {
    let valid_ids: Vec<NodeId> = selection.iter().copied().filter(|id| doc.get(*id).is_some()).collect();
    if valid_ids.is_empty() {
        return Ok(None);
    }
    let page_name = doc.get(page).map(|n| n.name.clone()).unwrap_or_else(|| "Page 1".to_string());
    let mut nodes: Vec<serde_json::Value> = valid_ids
        .iter()
        .filter_map(|id| {
            let n = doc.get(*id)?;
            Some(serde_json::json!({
                "node_id": id.0,
                "name": n.name,
                "kind": n.kind.default_name().to_ascii_lowercase(),
            }))
        })
        .collect();

    let count = nodes.len();
    let make_root = |nodes_slice: &[serde_json::Value]| -> serde_json::Value {
        serde_json::json!({
            "page": {
                "id": page.0,
                "name": page_name,
            },
            "count": count,
            "nodes": nodes_slice,
        })
    };

    for (idx, id) in valid_ids.iter().take(CHIP_SPEC_CAP).enumerate() {
        let Ok(spec) = doc.export_spec(*id) else { continue };
        let Ok(spec_val) = serde_json::to_value(&spec) else { continue };
        if let Some(obj) = nodes[idx].as_object_mut() {
            obj.insert("spec".to_string(), spec_val);
        }
        let candidate = serde_json::to_string_pretty(&make_root(&nodes)).map_err(|e| e.to_string())?;
        if candidate.len() > CHIP_FILE_BYTES_CAP {
            if let Some(obj) = nodes[idx].as_object_mut() {
                obj.remove("spec");
            }
            break;
        }
    }

    let final_json = serde_json::to_string_pretty(&make_root(&nodes)).map_err(|e| e.to_string())?;
    Ok(Some((final_json, count)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Node, Rect as DocRect};

    #[test]
    fn chip_text_formats() {
        assert_eq!(selection_chip_text("Page 1", 0, &[] as &[&str]), "Page 1 · none selected");
        assert_eq!(selection_chip_text("Page 1", 1, &["Hero"]), "Page 1 · 1 selected · Hero");
        assert_eq!(selection_chip_text("Page 1", 2, &["A", "B"]), "Page 1 · 2 selected · A, B");
        assert_eq!(selection_chip_text("Page 1", 3, &["A", "B", "C"]), "Page 1 · 3 selected · A, B +1");
    }

    #[test]
    fn chip_elides_to_width() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx);

        let summary = SelectionSummary {
            page: "Marketing Landing Page".to_string(),
            count: 3,
            names: vec!["Primary Hero Banner Frame".to_string(), "Secondary Call To Action Button".to_string()],
        };
        let font = theme::font_caption();

        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 200.0))), ..Default::default() };
        let _ = ctx.run_ui(raw, |ui| {
            let p = ui.painter();
            let full = summary.chip_text();
            let full_w = widgets::text_width(p, &full, &font);
            let prefix = "Marketing Landing Page · 3 selected · ";
            let prefix_w = widgets::text_width(p, prefix, &font);

            // Narrow enough to clip names, wide enough to keep the full page name + count.
            let mid_w = prefix_w + 45.0;
            assert!(mid_w < full_w);
            let elided_names_first = elide_selection_chip_text(p, &summary, &font, mid_w);
            assert!(elided_names_first.starts_with(prefix), "page and count must stay intact while names shrink first: {elided_names_first:?}");
            assert!(elided_names_first.ends_with('…'), "elided chip text must end with ellipsis: {elided_names_first:?}");
            assert!(widgets::text_width(p, &elided_names_first, &font) <= mid_w + 0.5);

            // Even narrower: page name shrinks next while keeping `· 3 selected · …`.
            let narrow_w = prefix_w - 25.0;
            let elided_page_second = elide_selection_chip_text(p, &summary, &font, narrow_w);
            assert!(elided_page_second.contains("· 3 selected"), "count stays visible while page shrinks: {elided_page_second:?}");
            assert!(elided_page_second.ends_with('…'), "must end with ellipsis: {elided_page_second:?}");
            assert!(widgets::text_width(p, &elided_page_second, &font) <= narrow_w + 0.5);
        });
    }

    #[test]
    fn large_selection_is_capped() {
        let mut doc = Document::new("Large Selection Doc");
        let page = doc.add_page("Page 1");
        let mut sel = Vec::with_capacity(200);
        for i in 0..200 {
            let id = doc.alloc_id();
            let mut f = Node::frame(id, DocRect::new((i as f32) * 12.0, 0.0, 320.0, 200.0));
            f.name = format!("Layer {i}");
            doc.insert(page, f).unwrap();
            sel.push(id);
        }

        let summary = SelectionSummary::from_doc(&doc, page, &sel);
        assert_eq!(summary.count, 200);
        assert_eq!(summary.names, vec!["Layer 0".to_string(), "Layer 1".to_string()]);
        assert_eq!(summary.chip_text(), "Page 1 · 200 selected · Layer 0, Layer 1 +198");

        let (json, count) = build_selection_spec_json(&doc, page, &sel).unwrap().expect("payload built");
        assert_eq!(count, 200);
        assert!(json.len() <= CHIP_FILE_BYTES_CAP, "file size {} exceeds 256 KB cap", json.len());
        let val: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        let nodes = val["nodes"].as_array().expect("nodes array");
        assert_eq!(nodes.len(), 200);
        for (i, entry) in nodes.iter().enumerate() {
            assert_eq!(entry["node_id"].as_u64(), Some(sel[i].0));
            assert_eq!(entry["name"].as_str(), Some(format!("Layer {i}").as_str()));
            if i < CHIP_SPEC_CAP {
                assert!(entry.get("spec").is_some(), "first 10 layers must include inline spec (layer {i})");
            } else {
                assert!(entry.get("spec").is_none(), "layers past 10 must omit inline spec (layer {i})");
            }
        }
    }
}
