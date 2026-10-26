//! Far-left icon rail.

use crate::app::{App, MenuKind, RailItem};
use crate::icons::{self, Icon};
use crate::theme;
use crate::widgets;
use egui::{Pos2, Rect, Sense, Ui, Vec2};

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
    let mb =
        Rect::from_min_size(Pos2::new(rect.left() + theme::INSET_FRAME, rect.top() + theme::RAIL_MENU_BUTTON_Y), Vec2::splat(theme::RAIL_MENU_BUTTON_SIZE));
    let r = ui.interact(mb, ui.id().with("rail-menu"), Sense::click());
    widgets::record(ui, "rail-menu", mb);
    widgets::labelled(ui, r.id, egui::accesskit::Role::Button, "Main menu");
    let menu_open = app.ui.menu == Some(MenuKind::Main);
    widgets::button_bg(&p, mb, &r, menu_open, theme::cr(theme::RADIUS_CONTROL));
    let menu_fg = if r.hovered() || menu_open { theme::ICON } else { theme::ICON_SECONDARY };
    icons::paint(&p, Icon::ListView, mb.center(), 18.0, menu_fg);
    let r = rail_tooltip(r, "Main menu");
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
        widgets::labelled(ui, resp.id, egui::accesskit::Role::Button, label);
        let selected = if app.plugins.open {
            *item == RailItem::Plugins
        } else if app.vars.open {
            *item == RailItem::Variables
        } else {
            app.ui.rail == Some(*item)
                || (*item == RailItem::File && (app.ui.menu == Some(MenuKind::FileMenu) || (app.ui.rail.is_none() && !app.ui.left_collapsed)))
        };
        // Active item: 32x32 rounded tile behind the icon.
        let ic = ir.center();
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
        let resp = rail_tooltip(resp, label);
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
                app.toggle_agents_panel();
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

    // Bottom: Figma Sync/Updates button with blue indicator dot.
    let br = Rect::from_min_size(
        Pos2::new(rect.left() + theme::INSET_FRAME, rect.bottom() - 8.0 - theme::RAIL_MENU_BUTTON_SIZE),
        Vec2::splat(theme::RAIL_MENU_BUTTON_SIZE),
    );
    widgets::record(ui, "rail-help", br);
    let help_resp = widgets::icon_button(ui, br, "help", Icon::Rotation, false, "Updates & Shortcuts");
    widgets::labelled(ui, help_resp.id, egui::accesskit::Role::Button, "Updates & Shortcuts");
    if help_resp.has_focus() && !help_resp.hovered() {
        egui::Tooltip::for_widget(&help_resp).show(|ui| {
            ui.set_max_width(ui.spacing().tooltip_width);
            ui.add(egui::Label::new("Updates & Shortcuts"));
        });
    }
    if help_resp.clicked() {
        crate::panels::menus::run(app, ui.ctx(), crate::panels::menus::Action::Shortcuts);
    }
    p.circle_filled(Pos2::new(br.center().x + 6.0, br.center().y - 6.0), 2.5, egui::Color32::from_rgb(0x0D, 0x99, 0xFF));
}

fn rail_tooltip(resp: egui::Response, label: &str) -> egui::Response {
    if resp.has_focus() {
        egui::Tooltip::for_widget(&resp).show(|ui| {
            ui.set_max_width(ui.spacing().tooltip_width);
            ui.add(egui::Label::new(label));
        });
        resp
    } else {
        resp.on_hover_text(label)
    }
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
    widgets::hline(p, rect.left() + theme::INSET_FRAME, rect.left() + inner_w - theme::INSET_FRAME, y, theme::BORDER);
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
    use designos_doc::Document;
    use egui::{Context, Event, Key, Modifiers, RawInput, Shape};

    fn collect_texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
        fn walk(shape: &Shape, out: &mut Vec<String>) {
            match shape {
                Shape::Text(t) => out.push(t.galley.text().to_owned()),
                Shape::Vec(inner) => inner.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        for cs in shapes {
            walk(&cs.shape, &mut out);
        }
        out
    }

    fn collect_rects(shapes: &[egui::epaint::ClippedShape]) -> Vec<(Rect, egui::Color32)> {
        fn walk(shape: &Shape, out: &mut Vec<(Rect, egui::Color32)>) {
            match shape {
                Shape::Rect(r) => out.push((r.rect, r.fill)),
                Shape::Vec(inner) => inner.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        for cs in shapes {
            walk(&cs.shape, &mut out);
        }
        out
    }

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

    #[test]
    fn rail_paints_icons_only_without_label_text() {
        let ctx = Context::default();
        theme::install_fonts(&ctx);
        let mut app = App::new(Document::new("RailTest"));
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        let rail_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(theme::RAIL_WIDTH + theme::BORDER_WIDTH, 900.0));

        let mut shapes = Vec::new();
        for i in 0..2 {
            let raw = RawInput {
                screen_rect: Some(screen),
                time: Some(i as f64 * 0.05),
                events: vec![Event::PointerMoved(Pos2::new(700.0, 500.0))],
                ..Default::default()
            };
            let out = ctx.run_ui(raw, |ui| show(&mut app, ui, rail_rect));
            shapes = out.shapes;
        }

        let painted_texts = collect_texts(&shapes);
        assert!(painted_texts.is_empty(), "rail buttons must show icons only with no painted label text, found: {painted_texts:?}");
    }

    #[test]
    fn rail_shows_tooltip_on_hover_and_on_keyboard_focus() {
        let ctx = Context::default();
        theme::install_fonts(&ctx);
        let mut app = App::new(Document::new("RailTooltipTest"));
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        let rail_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(theme::RAIL_WIDTH + theme::BORDER_WIDTH, 900.0));
        let mut t = 0.0;

        let mut step = |app: &mut App, events: Vec<Event>, dt: f64| -> Vec<egui::epaint::ClippedShape> {
            t += dt;
            let raw = RawInput { screen_rect: Some(screen), time: Some(t), events, ..Default::default() };
            ctx.run_ui(raw, |ui| show(app, ui, rail_rect)).shapes
        };

        let _ = step(&mut app, vec![Event::PointerMoved(Pos2::new(700.0, 500.0))], 0.05);
        let _ = step(&mut app, vec![], 0.05);

        // 1. Hover each rail item and verify only its tooltip name is painted.
        for (_, _, label, _) in ITEMS {
            let item_rect = ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", format!("rail-{label}"))))).expect("recorded rail item rect");
            let _ = step(&mut app, vec![Event::PointerMoved(item_rect.center())], 0.05);
            let mut shapes = Vec::new();
            for _ in 0..5 {
                shapes = step(&mut app, vec![], 0.2);
            }
            let texts = collect_texts(&shapes);
            assert_eq!(texts, vec![label.to_string()], "hovering rail-{label} must show only tooltip {label:?}, got texts: {texts:?}");
        }

        // 2. Move pointer away from the rail and step through buttons with Tab; verify Tab order and focus tooltips.
        let _ = step(&mut app, vec![Event::PointerMoved(Pos2::new(700.0, 500.0))], 0.05);
        for _ in 0..5 {
            let _ = step(&mut app, vec![], 0.2);
        }

        let expected_focus_order = ["Main menu", "File", "Agents", "Assets", "Tools", "Variables", "Plugins", "Updates & Shortcuts"];
        for expected_label in expected_focus_order {
            let _ = step(
                &mut app,
                vec![Event::Key { key: Key::Tab, physical_key: Some(Key::Tab), pressed: true, repeat: false, modifiers: Modifiers::NONE }],
                0.05,
            );
            let shapes = step(&mut app, vec![], 0.05);
            let texts = collect_texts(&shapes);
            assert_eq!(
                texts,
                vec![expected_label.to_string()],
                "keyboard focus via Tab must show only tooltip {expected_label:?} in order, got texts: {texts:?}"
            );
        }
    }

    #[test]
    fn rail_buttons_preserve_accesskit_names() {
        let ctx = Context::default();
        theme::install_fonts(&ctx);
        crate::agent_a11y::enable_accesskit(&ctx);
        let mut app = App::new(Document::new("RailA11yTest"));
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        let rail_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(theme::RAIL_WIDTH + theme::BORDER_WIDTH, 900.0));

        let raw = RawInput { screen_rect: Some(screen), time: Some(0.05), ..Default::default() };
        let out = ctx.run_ui(raw, |ui| {
            crate::agent_a11y::sync_frame(ui.ctx());
            show(&mut app, ui, rail_rect);
        });
        let update = out.platform_output.accesskit_update.expect("AccessKit TreeUpdate expected");

        for expected_name in ["File", "Agents", "Assets", "Tools", "Variables", "Plugins", "Main menu", "Updates & Shortcuts"] {
            let has_button = update.nodes.iter().any(|(_, node)| node.role() == egui::accesskit::Role::Button && node.label() == Some(expected_name));
            assert!(has_button, "expected AccessKit Role::Button with label {expected_name:?} on rail button");
        }
    }

    #[test]
    fn rail_width_and_active_tile_and_divider() {
        assert_eq!(theme::RAIL_WIDTH, 48.0, "icon-only rail width comes from theme tokens (32px tile + 2 * 8px inset)");
        assert_eq!(theme::RAIL_WIDTH, theme::RAIL_TILE + 2.0 * theme::INSET_FRAME);

        let ctx = Context::default();
        theme::install_fonts(&ctx);
        let mut app = App::new(Document::new("RailTileTest"));
        app.ui.rail = Some(RailItem::Agents);
        app.ui.left_collapsed = false;
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));
        let rail_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(theme::RAIL_WIDTH + theme::BORDER_WIDTH, 900.0));

        let raw = RawInput { screen_rect: Some(screen), time: Some(0.05), ..Default::default() };
        let out = ctx.run_ui(raw, |ui| show(&mut app, ui, rail_rect));

        let agents_rect = ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", "rail-Agents".to_string())))).expect("rail-Agents rect");
        let rects = collect_rects(&out.shapes);
        let active_tiles: Vec<Rect> = rects
            .iter()
            .filter(|(r, fill)| *fill == theme::BG_SELECTED && (r.width() - theme::RAIL_TILE).abs() < 0.01 && (r.height() - theme::RAIL_TILE).abs() < 0.01)
            .map(|(r, _)| *r)
            .collect();
        assert_eq!(active_tiles.len(), 1, "exactly one active 32x32 tile must be painted");
        assert!(agents_rect.contains(active_tiles[0].center()), "active tile {:?} must sit inside the active Agents cell {:?}", active_tiles[0], agents_rect);

        // Divider between Tools (index 3) and Variables (index 4) at y = 288.0.
        let divider_y = cell_tops()[4] - theme::RAIL_GROUP_GAP / 2.0;
        assert!(
            rects.iter().any(|(r, col)| {
                (r.top() - divider_y).abs() < 0.01 && (r.height() - theme::BORDER_WIDTH).abs() < 0.01 && r.width() > 0.0 && *col == theme::BORDER
            }),
            "divider between Tools and Variables at y={divider_y} must stay present, got rects: {rects:?}"
        );
    }
}
