//! The tree's context menus. The shared popup menu has no destructive row, and this menu needs a red "Delete", so
//! it is drawn here with the same kit looks and geometry (`menu_surface`, 32 px rows, 8 px inset pills, 12/18 Medium
//! labels).

use crate::icons::{self, Icon};
use crate::theme;
use crate::widgets::kit;
use egui::{Align2, Area, Context, Id, Key, Order, Pos2, Rect, Sense, Vec2};

#[derive(Clone, Debug)]
pub struct Entry {
    pub label: String,
    pub icon: Option<Icon>,
    pub shortcut: Option<&'static str>,
    pub destructive: bool,
    pub sep_after: bool,
    pub disabled: bool,
}

impl Entry {
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into(), icon: None, shortcut: None, destructive: false, sep_after: false, disabled: false }
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }
    pub fn shortcut(mut self, s: &'static str) -> Self {
        self.shortcut = Some(s);
        self
    }
    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }
    pub fn sep(mut self) -> Self {
        self.sep_after = true;
        self
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Open,
    Picked(usize),
    Closed,
}

const SEP_H: f32 = theme::MENU_SEP_MARGIN * 2.0 + theme::BORDER_WIDTH;
const ICON_X: f32 = 16.0;
const LABEL_X_ICON: f32 = 40.0;
const LABEL_X_PLAIN: f32 = 16.0;

pub fn size(entries: &[Entry]) -> Vec2 {
    let seps = entries.iter().filter(|e| e.sep_after).count() as f32;
    Vec2::new(theme::MENU_WIDTH, theme::MENU_PADDING * 2.0 + entries.len() as f32 * theme::MENU_ITEM_HEIGHT + seps * SEP_H)
}

/// The menu's rect for a click at `pos`: its top-left at the pointer, moved up and left only to stay in the window.
pub fn placed(pos: Pos2, size: Vec2, screen: Rect) -> Rect {
    let x = pos.x.min(screen.right() - size.x).max(screen.left());
    let y = pos.y.min(screen.bottom() - size.y).max(screen.top());
    Rect::from_min_size(Pos2::new(x, y), size)
}

/// Draws the menu and reports what was done with it this frame.
pub fn show(ctx: &Context, id: Id, pos: Pos2, entries: &[Entry]) -> Outcome {
    let rect = placed(pos, size(entries), ctx.content_rect());
    let hot_id = id.with("hot");
    // The first enabled row is hot when the menu opens (the mockups' state); the pointer and the arrow keys move it.
    let mut hot: Option<usize> = ctx.data(|d| d.get_temp::<Option<usize>>(hot_id)).flatten().or_else(|| entries.iter().position(|e| !e.disabled));
    let (esc, down, up, enter) = ctx.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, Key::Escape),
            i.consume_key(egui::Modifiers::NONE, Key::ArrowDown),
            i.consume_key(egui::Modifiers::NONE, Key::ArrowUp),
            i.consume_key(egui::Modifiers::NONE, Key::Enter),
        )
    });
    if esc {
        ctx.data_mut(|d| d.remove_temp::<Option<usize>>(hot_id));
        return Outcome::Closed;
    }
    let n = entries.len();
    if down {
        hot = Some(hot.map_or(0, |h| (h + 1) % n));
    }
    if up {
        hot = Some(hot.map_or(n - 1, |h| (h + n - 1) % n));
    }
    let mut outcome = Outcome::Open;
    Area::new(id).order(Order::Foreground).fixed_pos(rect.min).interactable(true).fade_in(false).show(ctx, |ui| {
        ui.allocate_exact_size(rect.size(), Sense::hover());
        let p = ui.painter().clone();
        kit::menu_surface(&p, rect);
        let mut y = rect.top() + theme::MENU_PADDING;
        for (i, e) in entries.iter().enumerate() {
            let row = Rect::from_min_size(
                Pos2::new(rect.left() + theme::MENU_PILL_INSET, y),
                Vec2::new(rect.width() - theme::MENU_PILL_INSET * 2.0, theme::MENU_ITEM_HEIGHT),
            );
            let resp = ui.interact(row, id.with(("row", i)), Sense::click());
            if resp.hovered() && !e.disabled {
                hot = Some(i);
            }
            let is_hot = hot == Some(i) && !e.disabled;
            let look = kit::menu_row_look(e.disabled, is_hot, false);
            kit::paint_frame(&p, row, theme::RADIUS_CONTROL, &look);
            let (text_color, icon_color) = if e.destructive { (theme::TEXT_ERROR, theme::DANGER) } else { (look.text, look.icon) };
            if let Some(icon) = e.icon {
                icons::paint(&p, icon, Pos2::new(rect.left() + ICON_X + theme::MENU_ICON_SIZE / 2.0, row.center().y), theme::MENU_ICON_SIZE, icon_color);
            }
            let label_x = rect.left() + if e.icon.is_some() { LABEL_X_ICON } else { LABEL_X_PLAIN };
            p.text(Pos2::new(label_x, row.center().y), Align2::LEFT_CENTER, &e.label, theme::font_menu(), text_color);
            if let Some(s) = e.shortcut {
                p.text(Pos2::new(rect.right() - theme::MENU_SHORTCUT_PAD, row.center().y), Align2::RIGHT_CENTER, s, theme::font_menu(), theme::TEXT_QUATERNARY);
            }
            crate::widgets::labelled(ui, resp.id, egui::accesskit::Role::MenuItem, &e.label);
            crate::widgets::record(ui, &format!("project-menu-{}", e.label), row);
            if resp.clicked() && !e.disabled {
                outcome = Outcome::Picked(i);
            }
            y += theme::MENU_ITEM_HEIGHT;
            if e.sep_after {
                crate::widgets::hline(&p, rect.left() + theme::BORDER_WIDTH, rect.right() - theme::BORDER_WIDTH, y + theme::MENU_SEP_MARGIN, theme::BORDER);
                y += SEP_H;
            }
        }
    });
    if enter {
        if let Some(h) = hot.filter(|h| !entries[*h].disabled) {
            outcome = Outcome::Picked(h);
        }
    }
    let pressed_outside = ctx.input(|i| i.pointer.any_pressed()) && ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| !rect.contains(p));
    if outcome == Outcome::Open && pressed_outside {
        outcome = Outcome::Closed;
    }
    ctx.data_mut(|d| {
        if outcome == Outcome::Open {
            d.insert_temp(hot_id, hot);
        } else {
            d.remove_temp::<Option<usize>>(hot_id);
        }
    });
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_menu_is_as_tall_as_its_rows_separators_and_padding() {
        let entries = vec![Entry::new("Open"), Entry::new("Rename"), Entry::new("Delete").sep(), Entry::new("Last")];
        let s = size(&entries);
        assert_eq!(s.x, theme::MENU_WIDTH);
        assert_eq!(s.y, 16.0 + 4.0 * 32.0 + 17.0);
    }

    #[test]
    fn a_menu_near_the_edge_moves_just_enough_to_stay_in_the_window() {
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        let size = Vec2::new(200.0, 300.0);
        assert_eq!(placed(Pos2::new(100.0, 100.0), size, screen).min, Pos2::new(100.0, 100.0));
        assert_eq!(placed(Pos2::new(1400.0, 800.0), size, screen).min, Pos2::new(1240.0, 600.0));
    }
}
