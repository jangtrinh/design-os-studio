//! "Layout guide" section of the inspector: add and edit the layout grids
//! (`FrameProps::grids`) of the selected frame. Grids are drawn over the frame
//! on the canvas (View > Layout guides, Ctrl+Shift+4).

use super::properties::{open_picker, row_end, section_sep, title, Pane, PickTarget};
use super::style_picker;
use crate::app::App;
use crate::icons::Icon;
use crate::theme;
use crate::widgets::field::{self, Prefix};
use crate::widgets::menu::dropdown;
use crate::widgets::{self};
use designos_doc::*;
use egui::{Color32, Sense, Ui};

pub(super) const KINDS: [&str; 3] = ["Grid", "Columns", "Rows"];

fn red() -> Color {
    Color::rgba(255, 0, 0, 26)
}

pub(super) fn color_of(g: &LayoutGrid) -> Color {
    match g {
        LayoutGrid::Grid { color, .. } | LayoutGrid::Columns { color, .. } | LayoutGrid::Rows { color, .. } => *color,
    }
}

pub(super) fn kind_index(g: &LayoutGrid) -> usize {
    match g {
        LayoutGrid::Grid { .. } => 0,
        LayoutGrid::Columns { .. } => 1,
        LayoutGrid::Rows { .. } => 2,
    }
}

/// A grid of kind `k` (0 grid, 1 columns, 2 rows) with Figma-like defaults, keeping `color`.
pub fn default_grid(k: usize, color: Color) -> LayoutGrid {
    match k {
        0 => LayoutGrid::grid(10.0, color),
        1 => LayoutGrid::columns(5, 20.0, 0.0, color),
        _ => LayoutGrid::rows(5, 20.0, 0.0, color),
    }
}

/// The grid with its colour replaced.
pub fn with_color(g: &LayoutGrid, c: Color) -> LayoutGrid {
    let mut g = g.clone();
    match &mut g {
        LayoutGrid::Grid { color, .. } | LayoutGrid::Columns { color, .. } | LayoutGrid::Rows { color, .. } => *color = c,
    }
    g
}

pub(super) fn align_index(a: LayoutGridAlign) -> usize {
    match a {
        LayoutGridAlign::Stretch => 0,
        LayoutGridAlign::Min => 1,
        LayoutGridAlign::Center => 2,
        LayoutGridAlign::Max => 3,
    }
}

pub(super) fn align_from_index(idx: usize) -> LayoutGridAlign {
    match idx {
        1 => LayoutGridAlign::Min,
        2 => LayoutGridAlign::Center,
        3 => LayoutGridAlign::Max,
        _ => LayoutGridAlign::Stretch,
    }
}

/// The "#" field of a columns / rows guide. Auto count (0) lays out fixed-width stripes, so it
/// needs a width: a Stretch guide ignores its width and fills the frame, so it becomes a
/// start-aligned guide and keeps the width the field shows.
pub(super) fn set_count(g: &mut LayoutGrid, v: u32) {
    if let LayoutGrid::Columns { count, section_size, alignment, .. } | LayoutGrid::Rows { count, section_size, alignment, .. } = g {
        *count = v;
        if v == 0 {
            if *section_size <= 0.0 {
                *section_size = 60.0;
            }
            if *alignment == LayoutGridAlign::Stretch {
                *alignment = LayoutGridAlign::Min;
            }
        }
    }
}

fn edit_grids(app: &mut App, sel: &[NodeId], f: impl Fn(&mut Vec<LayoutGrid>)) {
    app.edit_nodes(sel, |n| {
        if let Some(fp) = n.kind.frame_props_mut() {
            f(&mut fp.grids);
        }
    });
}

fn c32(c: &Color) -> Color32 {
    let f = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgba_unmultiplied(f(c.r), f(c.g), f(c.b), f(c.a))
}

pub(super) fn section(app: &mut App, ui: &mut Ui, pane: &mut Pane, sel: &[NodeId], grids: &[LayoutGrid]) {
    section_sep(ui, pane, false);
    let linked = style_picker::linked_style(app, sel, StyleProperty::Grid);
    if title(ui, pane, "Layout guide", Some(Icon::Plus), grids.is_empty() && linked.is_none()) {
        // first a square grid, later ones columns (Figma's order of defaults)
        let k = if grids.is_empty() { 0 } else { 1 };
        // Adding to a style-linked frame leaves the style: the frame keeps its guides and gets the new one.
        app.edit_nodes(sel, |n| {
            n.style_refs.remove("grid");
            if let Some(fp) = n.kind.frame_props_mut() {
                fp.grids.push(default_grid(k, red()));
            }
        });
    }
    style_picker::header_button(app, ui, pane, sel, StyleProperty::Grid);
    if let Some(key) = linked {
        // A linked frame shows its style instead of the guides it copies.
        pane.y += 4.0;
        style_picker::linked_row(app, ui, pane, sel, StyleProperty::Grid, &key);
        pane.y += 4.0;
        return;
    }
    for (i, g) in grids.iter().enumerate() {
        let is_stripes = matches!(g, LayoutGrid::Columns { .. } | LayoutGrid::Rows { .. });
        let dd_w = if is_stripes { 78.0 } else { 128.0 };
        let dd = pane.rect(theme::SECTION_CONTENT_X, 0.0, dd_w, theme::FIELD_HEIGHT);
        let names = KINDS.map(String::from);
        if let Some(k) = dropdown(ui, dd, ("lg-kind", i), KINDS[kind_index(g)], &names) {
            let new = default_grid(k, color_of(g));
            edit_grids(app, sel, |gs| {
                if let Some(slot) = gs.get_mut(i) {
                    *slot = new.clone();
                }
            });
        }
        if let LayoutGrid::Columns { alignment, .. } | LayoutGrid::Rows { alignment, .. } = g {
            let is_cols = matches!(g, LayoutGrid::Columns { .. });
            let align = *alignment;
            let align_names =
                if is_cols { ["Stretch", "Left", "Center", "Right"].map(String::from) } else { ["Stretch", "Top", "Center", "Bottom"].map(String::from) };
            let align_dd = pane.rect(theme::SECTION_CONTENT_X + 82.0, 0.0, 78.0, theme::FIELD_HEIGHT);
            if let Some(a_idx) = dropdown(ui, align_dd, ("lg-align", i), &align_names[align_index(align)], &align_names) {
                let next_align = align_from_index(a_idx);
                edit_grids(app, sel, |gs| match gs.get_mut(i) {
                    Some(LayoutGrid::Columns { alignment, section_size, .. }) | Some(LayoutGrid::Rows { alignment, section_size, .. }) => {
                        *alignment = next_align;
                        if next_align != LayoutGridAlign::Stretch && *section_size <= 0.0 {
                            *section_size = 60.0;
                        }
                    }
                    _ => {}
                });
            }
        }
        let vis = g.is_visible();
        let eye = pane.rect(theme::SECTION_TRAILING_X - theme::PANEL_ACTION_BUTTON - 4.0, 0.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
        let eye_icon = if vis { Icon::Eye } else { Icon::EyeOff };
        if widgets::icon_button(ui, eye, ("lg-vis", i), eye_icon, !vis, "").clicked() {
            edit_grids(app, sel, |gs| {
                if let Some(slot) = gs.get_mut(i) {
                    slot.set_visible(!vis);
                }
            });
        }
        let minus = pane.rect(theme::SECTION_TRAILING_X, 0.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
        if widgets::icon_button(ui, minus, ("lg-minus", i), Icon::Minus, false, "").clicked() {
            edit_grids(app, sel, |gs| {
                if i < gs.len() {
                    gs.remove(i);
                }
            });
        }
        pane.y += theme::FIELD_HEIGHT + 8.0;

        match g {
            LayoutGrid::Grid { size, .. } => {
                if let Some(v) = field::num_field(ui, pane.col(0, 0.0), ("lg-size", i), Prefix::Text("S"), Some(*size), "") {
                    let v = v.max(1.0);
                    edit_grids(app, sel, |gs| {
                        if let Some(LayoutGrid::Grid { size, .. }) = gs.get_mut(i) {
                            *size = v;
                        }
                    });
                }
                row_end(pane);
            }
            LayoutGrid::Columns { count, gutter, offset, section_size, alignment, .. }
            | LayoutGrid::Rows { count, gutter, offset, section_size, alignment, .. } => {
                let is_cols = matches!(g, LayoutGrid::Columns { .. });
                let (c, gt, off, sec, align) = (*count, *gutter, *offset, *section_size, *alignment);
                let count_suffix = if c == 0 { " Auto" } else { "" };
                if let Some(v) = field::num_field(ui, pane.col(0, 0.0), ("lg-count", i), Prefix::Text("#"), Some(c as f32), count_suffix) {
                    let v = v.round().clamp(0.0, 64.0) as u32;
                    edit_grids(app, sel, |gs| {
                        if let Some(g) = gs.get_mut(i) {
                            set_count(g, v);
                        }
                    });
                }
                if let Some(v) = field::num_field(ui, pane.col(1, 0.0), ("lg-gutter", i), Prefix::Text("G"), Some(gt), "") {
                    let v = v.max(0.0);
                    edit_grids(app, sel, |gs| match gs.get_mut(i) {
                        Some(LayoutGrid::Columns { gutter, .. }) | Some(LayoutGrid::Rows { gutter, .. }) => *gutter = v,
                        _ => {}
                    });
                }
                row_end(pane);
                if let Some(v) = field::num_field(ui, pane.col(0, 0.0), ("lg-offset", i), Prefix::Text("M"), Some(off), "") {
                    let v = v.max(0.0);
                    edit_grids(app, sel, |gs| match gs.get_mut(i) {
                        Some(LayoutGrid::Columns { offset, .. }) | Some(LayoutGrid::Rows { offset, .. }) => *offset = v,
                        _ => {}
                    });
                }
                let size_prefix = if is_cols { "W" } else { "H" };
                let size_suffix = if align == LayoutGridAlign::Stretch { " Auto" } else { "" };
                if let Some(v) = field::num_field(ui, pane.col(1, 0.0), ("lg-sec-size", i), Prefix::Text(size_prefix), Some(sec), size_suffix) {
                    let v = v.max(0.0);
                    edit_grids(app, sel, |gs| match gs.get_mut(i) {
                        Some(LayoutGrid::Columns { section_size, .. }) | Some(LayoutGrid::Rows { section_size, .. }) => *section_size = v,
                        _ => {}
                    });
                }
                row_end(pane);
            }
        }

        // colour: swatch, hex, opacity
        let col = color_of(g);
        let sw = pane.rect(theme::SWATCH_ROW_X, 4.0, theme::SWATCH_SIZE, theme::SWATCH_SIZE);
        let hex_r = pane.rect(theme::SWATCH_ROW_X + theme::SWATCH_SIZE + 8.0, 0.0, 96.0, theme::FIELD_HEIGHT);
        let op_r = pane.rect(theme::SWATCH_ROW_X + theme::SWATCH_SIZE + 8.0 + 96.0 + 4.0, 0.0, theme::OPACITY_FIELD_W, theme::FIELD_HEIGHT);
        widgets::swatch(ui.painter(), sw, c32(&col));
        if ui.interact(sw, ui.id().with(("lg-sw", i)), Sense::click()).clicked() {
            open_picker(app, PickTarget::GridColor(i), c32(&col), sw, sel.to_vec());
        }
        if let Some(s) = field::text_input(ui, hex_r, ("lg-hex", i), &Color { a: 1.0, ..col }.to_hex()) {
            if let Some(nc) = Color::from_hex(&s) {
                edit_grids(app, sel, |gs| {
                    if let Some(slot) = gs.get_mut(i) {
                        *slot = with_color(slot, Color { a: col.a, ..nc });
                    }
                });
            }
        }
        if let Some(v) = field::num_field(ui, op_r, ("lg-op", i), Prefix::None, Some((col.a * 100.0).round()), "%") {
            let a = (v / 100.0).clamp(0.0, 1.0);
            edit_grids(app, sel, |gs| {
                if let Some(slot) = gs.get_mut(i) {
                    *slot = with_color(slot, Color { a, ..col });
                }
            });
        }
        pane.y += theme::FIELD_HEIGHT + 8.0;
    }
    pane.y += 4.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_convert_and_keep_the_colour() {
        let c = Color::rgba(10, 20, 30, 40);
        for k in 0..3 {
            let g = default_grid(k, c);
            assert_eq!(kind_index(&g), k);
            assert_eq!(color_of(&g), c);
        }
        let g = with_color(&default_grid(1, c), red());
        assert_eq!(color_of(&g), red());
        assert!(matches!(g, LayoutGrid::Columns { count: 5, .. }));
    }

    #[test]
    fn auto_count_on_a_stretch_guide_draws_the_width_the_field_shows() {
        use crate::canvas::grids::stripes_aligned;
        // inspector sequence: new Columns guide, W = 300, then count 0
        let mut g = default_grid(1, red());
        if let LayoutGrid::Columns { section_size, .. } = &mut g {
            *section_size = 300.0;
        }
        set_count(&mut g, 0);
        let LayoutGrid::Columns { count, gutter, offset, section_size, alignment, .. } = g else { panic!("{g:?}") };
        assert_eq!((count, section_size), (0, 300.0));
        assert_eq!(alignment, LayoutGridAlign::Min, "auto count applies to fixed-width alignments only");
        let drawn = stripes_aligned(count, gutter, offset, section_size, alignment, 1000.0);
        assert!(!drawn.is_empty() && drawn.iter().all(|&(_, w)| w == 300.0), "painted widths {drawn:?}");
        assert_eq!(drawn[0].0, offset, "start-aligned");
    }
}
