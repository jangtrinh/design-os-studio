//! Far-left icon rail.

use crate::app::{App, MenuKind, RailItem};
use crate::icons::{self, Icon};
use crate::theme;
use crate::widgets;
use egui::{Align2, Pos2, Rect, Sense, Ui, Vec2};

/// Rail entries in order; the flag marks the item that starts a new group (separator above it).
/// (`RailItem::Specs` temporarily hidden from the UI rail).
pub const ITEMS: [(RailItem, Icon, &str, bool); 6] = [
    (RailItem::File, Icon::File, "File", false),
    (RailItem::Agents, Icon::Sparkle, "Agents", false),
    (RailItem::Assets, Icon::Assets, "Assets", false),
    (RailItem::Tools, Icon::Tools, "Tools", false),
    (RailItem::Variables, Icon::Variables, "Variables", true),
    (RailItem::Plugins, Icon::Component, "Plugins", false),
];

/// Top y (relative to the rail top) of each item's 56px cell.
pub fn cell_tops() -> Vec<f32> {
    let mut y = theme::RAIL_FIRST_Y;
    ITEMS
        .iter()
        .map(|(_, _, _, gap)| {
            if *gap {
                y += theme::RAIL_GROUP_GAP;
            }
            let top = y;
            y += theme::RAIL_ITEM_SIZE;
            top
        })
        .collect()
}

pub fn show(app: &mut App, ui: &mut Ui, rect: Rect) {
    let p = ui.painter().clone();
    p.rect_filled(rect, 0.0, theme::BG);
    widgets::vline(&p, rect.right() - theme::BORDER_WIDTH, rect.top(), rect.bottom(), theme::BORDER);
    let inner_w = theme::RAIL_WIDTH - theme::BORDER_WIDTH;

    // Logo / main menu button.
    let mb = Rect::from_min_size(
        Pos2::new(rect.left() + theme::RAIL_MENU_BUTTON_X, rect.top() + theme::RAIL_MENU_BUTTON_Y),
        Vec2::splat(theme::RAIL_MENU_BUTTON_SIZE),
    );
    let r = ui.interact(mb, ui.id().with("rail-menu"), Sense::click());
    widgets::record(ui, "rail-menu", mb);
    let menu_open = app.ui.menu == Some(MenuKind::Main);
    widgets::button_bg(&p, mb, &r, menu_open, theme::cr(theme::RADIUS_CONTROL));
    let menu_fg = if r.hovered() || menu_open { theme::ICON } else { theme::ICON_SECONDARY };
    icons::paint(&p, Icon::ListView, mb.center(), 18.0, menu_fg);
    if r.clicked() {
        toggle(app, MenuKind::Main, mb);
    }
    separator(&p, rect, rect.top() + theme::RAIL_SEPARATOR_Y, inner_w);

    for ((item, icon, label, gap_before), top) in ITEMS.iter().zip(cell_tops()) {
        let y = rect.top() + top;
        if *gap_before {
            separator(&p, rect, y - theme::RAIL_GROUP_GAP / 2.0, inner_w);
        }
        let ir = Rect::from_min_size(Pos2::new(rect.left(), y), Vec2::new(inner_w, theme::RAIL_ITEM_SIZE));
        let resp = ui.interact(ir, ui.id().with(("rail", *label)), Sense::click());
        widgets::record(ui, &format!("rail-{label}"), ir);
        let selected = if app.plugins.open {
            *item == RailItem::Plugins
        } else if app.vars.open {
            *item == RailItem::Variables
        } else {
            app.ui.rail == Some(*item)
                || (*item == RailItem::File && (app.ui.menu == Some(MenuKind::FileMenu) || (app.ui.rail.is_none() && !app.ui.left_collapsed)))
        };
        // Active item: 32x32 rounded tile behind the icon.
        let ic = Pos2::new(ir.center().x, ir.top() + theme::RAIL_ICON_CENTER_Y);
        if selected {
            p.rect_filled(Rect::from_center_size(ic, Vec2::splat(theme::RAIL_TILE)), theme::cr(theme::RADIUS_CONTROL), theme::BG_SELECTED);
        } else if resp.hovered() {
            p.rect_filled(Rect::from_center_size(ic, Vec2::splat(theme::RAIL_TILE)), theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
        }
        let icon_col = if selected && *item == RailItem::Agents { theme::ICON_SELECTED } else { theme::ICON };
        match *item {
            RailItem::Agents => paint_agents_rail_star(&p, ic, icon_col),
            RailItem::Assets => paint_assets_rail_circle_plus(&p, ic, icon_col),
            RailItem::Tools => paint_tools_rail_box(&p, ic, icon_col),
            _ => icons::paint(&p, *icon, ic, theme::RAIL_ICON_SIZE, icon_col),
        }
        let font = if selected { theme::font_caption_strong() } else { theme::font_caption() };
        p.text(Pos2::new(ir.center().x, ir.top() + theme::RAIL_LABEL_CENTER_Y), Align2::CENTER_CENTER, *label, font, theme::TEXT);
        if resp.secondary_clicked() && *item == RailItem::File {
            toggle(app, MenuKind::FileMenu, ir);
        }
        if resp.clicked() {
            if *item != RailItem::Specs {
                app.specs.open = false;
            }
            if *item != RailItem::Plugins {
                app.plugins.open = false;
            }
            if *item == RailItem::Plugins {
                app.open_plugins();
                continue;
            }
            let was_vars = app.vars.open;
            if *item != RailItem::Variables && app.vars.open {
                // leaving the variables view returns to the editor
                app.close_variables();
            }
            if *item == RailItem::File {
                if app.ui.rail.is_some() || app.ui.left_collapsed || was_vars {
                    app.ui.rail = None;
                    app.ui.left_collapsed = false;
                    app.ui.menu = None;
                } else {
                    app.ui.left_collapsed = true;
                    app.ui.menu = None;
                }
            } else if *item == RailItem::Agents {
                if app.ui.rail == Some(RailItem::Agents) && !app.ui.left_collapsed {
                    app.ui.rail = None;
                } else {
                    app.ui.rail = Some(RailItem::Agents);
                    app.ui.left_collapsed = false;
                    app.terminal.ensure_default_tab(app.file_path.as_deref());
                }
            } else if *item == RailItem::Specs {
                if app.ui.rail == Some(RailItem::Specs) && app.specs.open {
                    app.ui.rail = None;
                    app.specs.open = false;
                } else {
                    app.ui.rail = Some(RailItem::Specs);
                    app.ui.left_collapsed = false;
                    app.specs.open = true;
                    app.specs.ensure_loaded();
                }
            } else if *item == RailItem::Tools {
                if app.ui.rail == Some(RailItem::Tools) && !app.ui.left_collapsed {
                    app.ui.rail = None;
                    app.terminal.dock_open = false;
                } else {
                    app.ui.rail = Some(RailItem::Tools);
                    app.ui.left_collapsed = false;
                    app.terminal.dock_open = true;
                    app.terminal.ensure_default_tab(app.file_path.as_deref());
                }
            } else {
                if *item == RailItem::Variables {
                    app.ui.left_collapsed = false;
                    app.toggle_variables();
                    continue;
                }
                if app.ui.rail == Some(*item) && !app.ui.left_collapsed {
                    app.ui.rail = None;
                } else {
                    app.ui.rail = Some(*item);
                    app.ui.left_collapsed = false;
                }
            }
        }
    }

    // Bottom: Agent Terminal quick toggle (`>_`) + Figma Sync/Updates button with blue indicator dot.
    let tr = Rect::from_min_size(
        Pos2::new(rect.left() + theme::RAIL_MENU_BUTTON_X, rect.bottom() - 48.0 - theme::RAIL_MENU_BUTTON_SIZE),
        Vec2::splat(theme::RAIL_MENU_BUTTON_SIZE),
    );
    widgets::record(ui, "rail-terminal", tr);
    if widgets::icon_button(ui, tr, "rail-terminal", Icon::Terminal, app.terminal.dock_open, "Agent Terminal (Claude / Codex / Shell)").clicked() {
        app.terminal.dock_open = !app.terminal.dock_open;
        if app.terminal.dock_open {
            app.ui.rail = Some(RailItem::Agents);
            app.ui.left_collapsed = false;
            app.terminal.ensure_default_tab(app.file_path.as_deref());
        }
    }

    let br = Rect::from_min_size(
        Pos2::new(rect.left() + theme::RAIL_MENU_BUTTON_X, rect.bottom() - 8.0 - theme::RAIL_MENU_BUTTON_SIZE),
        Vec2::splat(theme::RAIL_MENU_BUTTON_SIZE),
    );
    widgets::record(ui, "rail-help", br);
    if widgets::icon_button(ui, br, "help", Icon::Rotation, false, "Updates & Shortcuts").clicked() {
        crate::panels::menus::run(app, ui.ctx(), crate::panels::menus::Action::Shortcuts);
    }
    p.circle_filled(Pos2::new(br.center().x + 6.0, br.center().y - 6.0), 2.5, egui::Color32::from_rgb(0x0D, 0x99, 0xFF));
}

fn paint_agents_rail_star(p: &egui::Painter, c: Pos2, col: egui::Color32) {
    icons::paint(p, Icon::Sparkle, c, theme::RAIL_ICON_SIZE, col);
}

fn paint_assets_rail_circle_plus(p: &egui::Painter, c: Pos2, col: egui::Color32) {
    icons::paint(p, Icon::Assets, c, theme::RAIL_ICON_SIZE, col);
}

fn paint_tools_rail_box(p: &egui::Painter, c: Pos2, col: egui::Color32) {
    icons::paint(p, Icon::Tools, c, theme::RAIL_ICON_SIZE, col);
}

/// 1px #444 line inset from both edges of the rail.
fn separator(p: &egui::Painter, rect: Rect, y: f32, inner_w: f32) {
    widgets::hline(p, rect.left() + theme::RAIL_SEPARATOR_INSET, rect.left() + inner_w - theme::RAIL_SEPARATOR_INSET, y, theme::BORDER);
}

fn toggle(app: &mut App, kind: MenuKind, anchor: Rect) {
    if app.ui.menu == Some(kind) {
        app.ui.menu = None;
    } else {
        app.ui.menu = Some(kind);
        app.ui.menu_anchor = Some(anchor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_follow_the_measured_rail_layout() {
        // File, Agents, Assets, Tools every 56px, then a 16px gap before Variables and Plugins.
        assert_eq!(cell_tops(), vec![56.0, 112.0, 168.0, 224.0, 296.0, 352.0]);
        assert_eq!(ITEMS[0].0, RailItem::File);
        assert_eq!(ITEMS[1].0, RailItem::Agents);
        assert_eq!(ITEMS[2].0, RailItem::Assets);
        assert_eq!(ITEMS[3].0, RailItem::Tools);
        assert_eq!(ITEMS[4].0, RailItem::Variables);
        assert_eq!(ITEMS[5].0, RailItem::Plugins);
        // separators sit at y=48 (under the logo) and 288 (middle of the group gap)
        assert_eq!(theme::RAIL_SEPARATOR_Y, 48.0);
        assert_eq!(cell_tops()[4] - theme::RAIL_GROUP_GAP / 2.0, 288.0);
    }
}
