//! Rows of the "Edit style" popover for a layout guide style: type, size or count / gutter /
//! margin, colour and opacity of the guide being edited.

use super::effects;
use super::layoutguide;
use super::style_edit::edit;
use crate::app::App;
use crate::widgets::field::{self, Prefix};
use crate::widgets::menu::dropdown;
use crate::widgets::popover;
use designos_doc::*;
use egui::{Rect, Ui};

pub(super) fn grid_rows(app: &mut App, ui: &mut Ui, body: Rect, g: &LayoutGrid, item: usize, first: usize) {
    let set = |app: &mut App, f: &dyn Fn(&mut LayoutGrid)| {
        edit(app, |s| {
            if let Some(g) = if let Style::Grid(gs) = s { gs.get_mut(item) } else { None } {
                f(g);
            }
        })
    };
    let color = layoutguide::color_of(g);
    let mut row = first;
    popover::row(ui, body, row, "Type", |ui, rc| {
        let names = layoutguide::KINDS.map(String::from);
        if let Some(k) = dropdown(ui, rc, "se-grid-kind", layoutguide::KINDS[layoutguide::kind_index(g)], &names) {
            set(app, &|g| *g = layoutguide::default_grid(k, color));
        }
    });
    row += 1;
    match g {
        LayoutGrid::Grid { size, .. } => {
            popover::row(ui, body, row, "Size", |ui, rc| {
                if let Some(v) = field::num_field(ui, rc, "se-grid-size", Prefix::Text("S"), Some(*size), "") {
                    set(app, &|g| {
                        if let LayoutGrid::Grid { size, .. } = g {
                            *size = v.max(1.0);
                        }
                    });
                }
            });
            row += 1;
        }
        LayoutGrid::Columns { count, gutter, offset, .. } | LayoutGrid::Rows { count, gutter, offset, .. } => {
            let (count, gutter, offset) = (*count as f32, *gutter, *offset);
            popover::row(ui, body, row, "Count", |ui, rc| {
                if let Some(v) = field::num_field(ui, rc, "se-grid-count", Prefix::Text("#"), Some(count), "") {
                    set(app, &|g| {
                        if let LayoutGrid::Columns { count, .. } | LayoutGrid::Rows { count, .. } = g {
                            *count = v.round().clamp(1.0, 64.0) as u32;
                        }
                    });
                }
            });
            popover::row(ui, body, row + 1, "Gutter", |ui, rc| {
                if let Some(v) = field::num_field(ui, rc, "se-grid-gutter", Prefix::Text("G"), Some(gutter), "") {
                    set(app, &|g| {
                        if let LayoutGrid::Columns { gutter, .. } | LayoutGrid::Rows { gutter, .. } = g {
                            *gutter = v.max(0.0);
                        }
                    });
                }
            });
            popover::row(ui, body, row + 2, "Margin", |ui, rc| {
                if let Some(v) = field::num_field(ui, rc, "se-grid-offset", Prefix::Text("M"), Some(offset), "") {
                    set(app, &|g| {
                        if let LayoutGrid::Columns { offset, .. } | LayoutGrid::Rows { offset, .. } = g {
                            *offset = v.max(0.0);
                        }
                    });
                }
            });
            row += 3;
        }
    }
    popover::row(ui, body, row, "Color", |ui, rc| {
        if let Some(nc) = effects::color_row(app, ui, rc, (item, 7), color, None) {
            set(app, &|g| *g = layoutguide::with_color(g, nc));
        }
    });
    popover::row(ui, body, row + 1, "Opacity", |ui, rc| {
        if let Some(v) = field::num_field(ui, rc, "se-grid-opacity", Prefix::Icon(crate::icons::Icon::Opacity), Some((color.a * 100.0).round()), "%") {
            set(app, &|g| *g = layoutguide::with_color(g, Color { a: (v / 100.0).clamp(0.0, 1.0), ..color }));
        }
    });
}
