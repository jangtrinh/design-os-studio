//! "Edit style" popover (Figma UI3): change the values of a shared fill, stroke, text, effect or
//! layout guide style. Every change reaches all layers linked to the style as one undo step.
//!
//! Values dragged or typed are collected in [`StyleEdit::pending`] and recorded once the pointer
//! is up, so scrubbing a number is a single history entry however long the drag.

use super::effects::{self, FxTarget};
use super::{layoutguide, typography};
use crate::app::App;
use crate::richtext;
use crate::widgets::field::{self, Prefix};
use crate::widgets::menu::dropdown;
use crate::widgets::popover;
use designos_doc::*;
use egui::{Id, Rect, Ui};

/// The open popover: which style it edits and what it shows.
pub struct StyleEdit {
    pub key: String,
    /// The button that opened it; the popover sits next to it.
    pub anchor: Rect,
    /// Which fill, effect or layout guide of the style the rows show.
    pub item: usize,
    /// Values changed but not recorded yet.
    pub pending: Option<Style>,
}

/// Opens the popover for style `key` next to `anchor`; clicking the opener again closes it.
pub(super) fn toggle(app: &mut App, key: &str, anchor: Rect) {
    if app.props.style_edit.as_ref().is_some_and(|s| s.key == key) {
        app.props.style_edit = None;
        return;
    }
    app.props.pop = None;
    app.props.style_menu = None;
    app.props.styles_menu = None;
    app.props.style_edit = Some(StyleEdit { key: key.to_string(), anchor, item: 0, pending: None });
}

/// Applies `f` to the values being edited (the pending ones, else the stored ones).
pub fn edit(app: &mut App, f: impl FnOnce(&mut Style)) {
    let Some(st) = app.props.style_edit.as_mut() else { return };
    let Some(mut style) = st.pending.take().or_else(|| app.doc.styles.get(&st.key).cloned()) else { return };
    f(&mut style);
    st.pending = Some(style);
}

/// How many fills, effects or guides the style holds (text styles are a single item).
fn item_count(style: &Style) -> usize {
    match style {
        Style::Fill(p) => p.len(),
        Style::Effect(e) => e.len(),
        Style::Grid(g) => g.len(),
        Style::Text(_) => 1,
    }
}

fn paint_kind_name(p: &Paint) -> &'static str {
    match p.kind {
        PaintKind::Solid(_) => "Solid",
        PaintKind::LinearGradient { .. } => "Linear gradient",
        PaintKind::RadialGradient { .. } => "Radial gradient",
        PaintKind::AngularGradient { .. } => "Angular gradient",
        PaintKind::DiamondGradient { .. } => "Diamond gradient",
        PaintKind::Image { .. } => "Image",
    }
}

fn kind_rows(style: &Style, item: usize) -> usize {
    match style {
        // Colour (or paint type) and opacity.
        Style::Fill(_) => 2,
        Style::Text(_) => 5,
        Style::Effect(fx) => fx.get(item).map_or(0, effects::effect_rows),
        Style::Grid(g) => match g.get(item) {
            Some(LayoutGrid::Grid { .. }) => 4,
            Some(_) => 6,
            None => 0,
        },
    }
}

pub(super) fn popover_ui(app: &mut App, ui: &mut Ui, region: Rect) {
    let Some(st) = app.props.style_edit.as_ref() else { return };
    let (key, anchor, item) = (st.key.clone(), st.anchor, st.item);
    let Some(stored) = app.doc.styles.get(&key) else {
        app.props.style_edit = None;
        return;
    };
    let style = st.pending.clone().unwrap_or_else(|| stored.clone());
    let count = item_count(&style);
    let item = item.min(count.saturating_sub(1));
    let rows = 1 + usize::from(count > 1) + kind_rows(&style, item);
    let ctx = ui.ctx().clone();
    let close = popover::show(&ctx, Id::new("style-edit-popover"), anchor, region, 240.0, popover::height(rows), "Edit style", |ui, body| {
        body_ui(app, ui, body, &key, &style, item, count);
    });
    // Record what changed once the pointer is up (typed values arrive with it up, drags end with it).
    if close || !ctx.input(|i| i.pointer.any_down()) {
        if let Some(pending) = app.props.style_edit.as_mut().and_then(|s| s.pending.take()) {
            if app.doc.styles.get(&key) != Some(&pending) {
                let text = matches!(pending, Style::Text(_));
                if app.update_style_values(&key, pending) && text {
                    app.fonts.load_for(&app.doc);
                }
            }
        }
    }
    if close {
        app.props.style_edit = None;
    }
}

fn body_ui(app: &mut App, ui: &mut Ui, body: Rect, key: &str, style: &Style, item: usize, count: usize) {
    popover::row(ui, body, 0, "Name", |ui, rc| {
        if let Some(name) = field::text_input(ui, rc, "se-name", app.doc.style_name(key)) {
            app.rename_style_to(key, &name);
        }
    });
    let mut first = 1;
    if count > 1 {
        let (label, names): (&str, Vec<String>) = match style {
            Style::Fill(p) => ("Fill", p.iter().enumerate().map(|(i, p)| format!("{} \u{b7} {}", i + 1, paint_kind_name(p))).collect()),
            Style::Effect(fx) => {
                ("Effect", fx.iter().enumerate().map(|(i, e)| format!("{} \u{b7} {}", i + 1, effects::KINDS[effects::kind_of(e)].0)).collect())
            }
            Style::Grid(g) => {
                ("Guide", g.iter().enumerate().map(|(i, g)| format!("{} \u{b7} {}", i + 1, layoutguide::KINDS[layoutguide::kind_index(g)])).collect())
            }
            Style::Text(_) => ("", Vec::new()),
        };
        popover::row(ui, body, first, label, |ui, rc| {
            if let Some(i) = dropdown(ui, rc, "se-item", &names[item], &names) {
                if let Some(st) = app.props.style_edit.as_mut() {
                    st.item = i;
                }
            }
        });
        first += 1;
    }
    match style {
        Style::Fill(paints) => fill_rows(app, ui, body, paints.get(item), item, first),
        Style::Text(t) => text_rows(app, ui, body, t, first),
        Style::Effect(fx) => {
            if let Some(e) = fx.get(item) {
                effects::effect_body(app, ui, body, e, item, FxTarget::Style, first);
            }
        }
        Style::Grid(g) => {
            if let Some(g) = g.get(item) {
                grid_rows(app, ui, body, g, item, first);
            }
        }
    }
}

fn fill_rows(app: &mut App, ui: &mut Ui, body: Rect, paint: Option<&Paint>, item: usize, first: usize) {
    let Some(p) = paint else { return };
    let set = |app: &mut App, f: &dyn Fn(&mut Paint)| {
        edit(app, |s| {
            if let Some(p) = if let Style::Fill(ps) = s { ps.get_mut(item) } else { None } {
                f(p);
            }
        })
    };
    if let PaintKind::Solid(c) = &p.kind {
        popover::row(ui, body, first, "Color", |ui, rc| {
            if let Some(nc) = effects::color_row(app, ui, rc, (item, 9), *c, None) {
                set(app, &|p| p.kind = PaintKind::Solid(nc));
            }
        });
    } else {
        // Gradient stops and images are edited on a layer; the popover shows what the paint is.
        popover::row(ui, body, first, "Type", |ui, rc| {
            ui.painter().text(rc.left_center(), egui::Align2::LEFT_CENTER, paint_kind_name(p), crate::theme::font_body(), crate::theme::TEXT);
        });
    }
    popover::row(ui, body, first + 1, "Opacity", |ui, rc| {
        if let Some(v) = field::num_field(ui, rc, "se-fill-opacity", Prefix::None, Some((p.opacity * 100.0).round()), "%") {
            set(app, &|p| p.opacity = (v / 100.0).clamp(0.0, 1.0));
        }
    });
}

fn text_rows(app: &mut App, ui: &mut Ui, body: Rect, t: &TextStyle, first: usize) {
    let set = |app: &mut App, f: &dyn Fn(&mut TextStyle)| {
        edit(app, |s| {
            if let Style::Text(ts) = s {
                f(ts);
            }
        })
    };
    popover::row(ui, body, first, "Font", |ui, rc| {
        if let Some(family) = typography::family_picker(ui, rc, &t.font_family) {
            set(app, &|ts| ts.font_family = family.clone());
        }
    });
    popover::row(ui, body, first + 1, "Weight", |ui, rc| {
        let names: Vec<String> = typography::WEIGHTS.iter().map(|w| w.0.to_string()).collect();
        if let Some(i) = dropdown(ui, rc, "se-weight", typography::weight_name(t.font_weight), &names) {
            set(app, &|ts| ts.font_weight = typography::WEIGHTS[i].1);
        }
    });
    popover::row(ui, body, first + 2, "Size", |ui, rc| {
        if let Some(v) = field::num_field(ui, rc, "se-size", Prefix::None, Some(t.font_size), "") {
            set(app, &|ts| ts.font_size = v.max(1.0));
        }
    });
    let line = richtext::line_height_px(t.line_height, t.font_size).unwrap_or_else(|| (t.font_size * 1.21 * 10.0).round() / 10.0);
    popover::row(ui, body, first + 3, "Line height", |ui, rc| {
        if let Some(v) = field::num_field(ui, rc, "se-line-height", Prefix::None, Some(line), "") {
            set(app, &|ts| ts.line_height = LineHeight::Pixels(v.max(1.0)));
        }
    });
    popover::row(ui, body, first + 4, "Letter spacing", |ui, rc| {
        if let Some(v) = field::num_field(ui, rc, "se-letter-spacing", Prefix::None, Some(t.letter_spacing), "") {
            set(app, &|ts| ts.letter_spacing = v);
        }
    });
}

fn grid_rows(app: &mut App, ui: &mut Ui, body: Rect, g: &LayoutGrid, item: usize, first: usize) {
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
                if let Some(v) = field::num_field(ui, rc, "se-grid-size", Prefix::None, Some(*size), "") {
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
                if let Some(v) = field::num_field(ui, rc, "se-grid-count", Prefix::None, Some(count), "") {
                    set(app, &|g| {
                        if let LayoutGrid::Columns { count, .. } | LayoutGrid::Rows { count, .. } = g {
                            *count = v.round().clamp(1.0, 64.0) as u32;
                        }
                    });
                }
            });
            popover::row(ui, body, row + 1, "Gutter", |ui, rc| {
                if let Some(v) = field::num_field(ui, rc, "se-grid-gutter", Prefix::None, Some(gutter), "") {
                    set(app, &|g| {
                        if let LayoutGrid::Columns { gutter, .. } | LayoutGrid::Rows { gutter, .. } = g {
                            *gutter = v.max(0.0);
                        }
                    });
                }
            });
            popover::row(ui, body, row + 2, "Margin", |ui, rc| {
                if let Some(v) = field::num_field(ui, rc, "se-grid-offset", Prefix::None, Some(offset), "") {
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
        if let Some(v) = field::num_field(ui, rc, "se-grid-opacity", Prefix::None, Some((color.a * 100.0).round()), "%") {
            set(app, &|g| *g = layoutguide::with_color(g, Color { a: (v / 100.0).clamp(0.0, 1.0), ..color }));
        }
    });
}
