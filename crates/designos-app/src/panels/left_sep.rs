//! Draggable, keyboard-accessible separator between the Pages and Layers sections in the left panel (`#1108`).

use crate::app::App;
use crate::{theme, widgets};
use egui::{CursorIcon, Event, EventFilter, Id, Key, PointerButton, Pos2, Rect, Sense, Stroke, StrokeKind, Ui};

#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub min_h: f32,
    pub max_h: f32,
    pub default_h: f32,
}

impl Bounds {
    pub fn clamp(self, h: f32) -> f32 {
        h.clamp(self.min_h, self.max_h)
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct SepDrag {
    start_y: f32,
    start_h: f32,
    moved: bool,
}

pub fn separator_id() -> Id {
    Id::new("pages-layers-separator")
}

fn drag_state_id() -> Id {
    Id::new("pages-layers-separator-drag")
}

pub fn compute_bounds(app: &App, inner: Rect, pages_list_top: f32) -> Bounds {
    let n = app.doc.pages.len().max(1);
    let content_h = n as f32 * theme::PAGE_ROW_HEIGHT;
    let default_h = n.min(theme::PAGES_MAX_VISIBLE_ROWS) as f32 * theme::PAGE_ROW_HEIGHT;
    let min_h = theme::PAGE_ROW_HEIGHT;
    let dev_h = if app.ui.dev_mode { theme::LEFT_DEV_READY_SECTION_STEP } else { 0.0 };
    let filter_h = if app.layers.filter_open { theme::LAYER_FILTER_ROW_HEIGHT } else { 0.0 };
    let overhead = theme::SPACE_INLINE + dev_h + theme::LAYER_HEADER_HEIGHT + filter_h + theme::LAYERS_MIN_HEIGHT;
    let max_by_panel = (inner.bottom() - pages_list_top - overhead).max(min_h);
    let max_h = content_h.min(max_by_panel).max(min_h);
    Bounds { min_h, max_h, default_h }
}

pub fn hit_rect_for(inner: Rect, pages_list_top: f32, list_h: f32) -> Rect {
    let sep_line_y = pages_list_top + list_h + theme::SPACE_INLINE - theme::BORDER_WIDTH;
    Rect::from_min_max(
        Pos2::new(inner.left(), sep_line_y - theme::PAGES_SEPARATOR_HIT_HALF),
        Pos2::new(inner.right(), sep_line_y + theme::PAGES_SEPARATOR_HIT_HALF),
    )
}

fn persist_pages_height(app: &App, height: Option<f32>) {
    if let Some(lib) = &app.lib {
        let _ = crate::comments::settings::save_pages_height(lib.root(), height);
    } else if !cfg!(test) {
        let root = designos_library::Library::default_root();
        let _ = crate::comments::settings::save_pages_height(&root, height);
    }
}

/// Processes pointer (including single-frame/slow-frame press+move+release gestures and double-click reset)
/// and keyboard events before `pages_list` and `layers::show` lay out, returning `(list_h, bounds, is_dragging)`.
pub fn prepare_height(app: &mut App, ui: &mut Ui, inner: Rect, pages_list_top: f32) -> (f32, Bounds, bool) {
    let bounds = compute_bounds(app, inner, pages_list_top);
    let mut cur_h = bounds.clamp(app.ui.pages_height.unwrap_or(bounds.default_h));
    let sep_id = separator_id();
    let state_id = drag_state_id();

    let mut drag: Option<SepDrag> = ui.ctx().data(|d| d.get_temp::<SepDrag>(state_id));
    let mut persist: Option<Option<f32>> = None;
    let double_clicked = ui.input(|i| i.pointer.button_double_clicked(PointerButton::Primary));
    let events = ui.input(|i| i.events.clone());

    for ev in &events {
        match ev {
            Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, .. } => {
                let hit = hit_rect_for(inner, pages_list_top, cur_h);
                if hit.contains(*pos) {
                    ui.memory_mut(|m| {
                        m.request_focus(sep_id);
                        m.set_focus_lock_filter(sep_id, EventFilter { vertical_arrows: true, ..Default::default() });
                    });
                    if double_clicked {
                        app.ui.pages_height = None;
                        cur_h = bounds.clamp(bounds.default_h);
                        drag = None;
                        persist = Some(None);
                    } else {
                        drag = Some(SepDrag { start_y: pos.y, start_h: cur_h, moved: false });
                    }
                } else if ui.memory(|m| m.has_focus(sep_id)) {
                    ui.memory_mut(|m| m.surrender_focus(sep_id));
                }
            }
            Event::PointerMoved(pos) => {
                if let Some(d) = drag.as_mut() {
                    let dy = pos.y - d.start_y;
                    if dy.abs() > 0.0 {
                        d.moved = true;
                        cur_h = bounds.clamp(d.start_h + dy);
                        app.ui.pages_height = Some(cur_h);
                    }
                }
            }
            Event::PointerButton { pos, button: PointerButton::Primary, pressed: false, .. } => {
                let hit = hit_rect_for(inner, pages_list_top, cur_h);
                if double_clicked && (hit.contains(*pos) || drag.is_some()) {
                    drag = None;
                    app.ui.pages_height = None;
                    cur_h = bounds.clamp(bounds.default_h);
                    persist = Some(None);
                } else if let Some(d) = drag.take() {
                    let dy = pos.y - d.start_y;
                    if d.moved || dy.abs() > 0.0 {
                        cur_h = bounds.clamp(d.start_h + dy);
                        app.ui.pages_height = Some(cur_h);
                        persist = Some(Some(cur_h));
                    }
                }
            }
            Event::PointerGone => {
                if let Some(d) = drag.take() {
                    if d.moved {
                        persist = Some(app.ui.pages_height);
                    }
                }
            }
            _ => {}
        }
    }

    if !ui.input(|i| i.pointer.primary_down()) {
        if let Some(d) = drag.take() {
            if d.moved {
                persist = Some(app.ui.pages_height);
            }
        }
    }

    if ui.memory(|m| m.has_focus(sep_id)) {
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(sep_id, EventFilter { vertical_arrows: true, ..Default::default() });
        });
        for ev in &events {
            if let Event::Key { key, pressed: true, modifiers, .. } = ev {
                let step = if modifiers.shift { theme::PAGES_SEPARATOR_STEP * theme::PAGES_SEPARATOR_SHIFT_MULT } else { theme::PAGES_SEPARATOR_STEP };
                match key {
                    Key::ArrowUp => {
                        cur_h = bounds.clamp(cur_h - step);
                        app.ui.pages_height = Some(cur_h);
                        persist = Some(Some(cur_h));
                    }
                    Key::ArrowDown => {
                        cur_h = bounds.clamp(cur_h + step);
                        app.ui.pages_height = Some(cur_h);
                        persist = Some(Some(cur_h));
                    }
                    Key::Home => {
                        cur_h = bounds.min_h;
                        app.ui.pages_height = Some(cur_h);
                        persist = Some(Some(cur_h));
                    }
                    Key::End => {
                        cur_h = bounds.max_h;
                        app.ui.pages_height = Some(cur_h);
                        persist = Some(Some(cur_h));
                    }
                    Key::Enter => {
                        app.ui.pages_height = None;
                        cur_h = bounds.clamp(bounds.default_h);
                        persist = Some(None);
                    }
                    _ => {}
                }
            }
        }
    }

    let is_dragging = drag.is_some();
    ui.ctx().data_mut(|d| match drag {
        Some(state) => {
            let _ = d.insert_temp(state_id, state);
        }
        None => {
            let _ = d.remove_temp::<SepDrag>(state_id);
        }
    });

    if let Some(height_to_save) = persist {
        persist_pages_height(app, height_to_save);
    }

    (cur_h, bounds, is_dragging)
}

/// Paints the 1 px separator line, 6 px interactive hit zone, `ResizeVertical` cursor + brand highlight,
/// visible focus ring, and AccessKit `Splitter` (`separator`) node.
pub fn paint_and_interact(ui: &mut Ui, inner: Rect, pages_list_top: f32, list_h: f32, bounds: Bounds, is_dragging: bool) {
    let hit_rect = hit_rect_for(inner, pages_list_top, list_h);
    let sep_line_y = hit_rect.center().y;
    let sep_id = separator_id();

    let resp = ui.interact(hit_rect, sep_id, Sense::click_and_drag());
    widgets::record(ui, "pages-layers-separator", hit_rect);

    widgets::hline(ui.painter(), inner.left(), inner.right(), sep_line_y, theme::BORDER);

    let pointer_in_hit = ui.input(|i| i.pointer.interact_pos().is_some_and(|p| hit_rect.contains(p)));
    if is_dragging || resp.hovered() || resp.dragged() || pointer_in_hit {
        ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical);
        ui.painter().hline(inner.left()..=inner.right(), sep_line_y, Stroke::new(theme::PAGES_SEPARATOR_HIGHLIGHT_STROKE, theme::BG_BRAND));
    }

    if ui.memory(|m| m.has_focus(sep_id)) {
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(sep_id, EventFilter { vertical_arrows: true, ..Default::default() });
        });
        ui.painter().rect_stroke(hit_rect, theme::cr(theme::RADIUS_MARK), Stroke::new(theme::BORDER_WIDTH, theme::BORDER_SELECTED), StrokeKind::Inside);
    }

    if crate::agent_a11y::is_accesskit_active() {
        ui.ctx().accesskit_node_builder(sep_id, |node| {
            node.set_role(egui::accesskit::Role::Splitter);
            node.set_label("Resize Pages and Layers");
            node.set_orientation(egui::accesskit::Orientation::Horizontal);
            node.set_numeric_value(f64::from(list_h));
            node.set_min_numeric_value(f64::from(bounds.min_h));
            node.set_max_numeric_value(f64::from(bounds.max_h));
            node.set_numeric_value_step(f64::from(theme::PAGES_SEPARATOR_STEP));
            node.set_numeric_value_jump(f64::from(theme::PAGES_SEPARATOR_STEP * theme::PAGES_SEPARATOR_SHIFT_MULT));
        });
    }
}
