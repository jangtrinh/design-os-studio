//! The bottom toolbar of a board: Move, Hand, Marker / Highlighter, Sticky, Shape, Connector, Text,
//! Section and Stamp, with popovers for the brush, the shape and the stamp.

use super::{color32, color_of, rgb_of, ToolMenu};
use crate::app::App;
use crate::canvas::tools::Tool;
use crate::icons::{self, Icon};
use crate::panels::toolbar::tool_icon;
use crate::theme;
use crate::widgets;
use designos_doc::{ShapeKind, STAMP_KINDS};
use egui::{Align2, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

struct Slot {
    tools: &'static [Tool],
    /// A dropdown chevron next to the button.
    menu: Option<ToolMenu>,
    /// A divider before the slot.
    divider: bool,
}

const SLOTS: [Slot; 9] = [
    Slot { tools: &[Tool::Move], menu: None, divider: false },
    Slot { tools: &[Tool::Hand], menu: None, divider: false },
    Slot { tools: &[Tool::Marker, Tool::Highlighter], menu: Some(ToolMenu::Brush), divider: true },
    Slot { tools: &[Tool::Sticky], menu: None, divider: false },
    Slot { tools: &[Tool::BoardShape], menu: Some(ToolMenu::Shape), divider: false },
    Slot { tools: &[Tool::Connector], menu: None, divider: false },
    Slot { tools: &[Tool::Text], menu: None, divider: false },
    Slot { tools: &[Tool::Section], menu: None, divider: false },
    Slot { tools: &[Tool::Stamp], menu: Some(ToolMenu::Stamp), divider: false },
];

const SLOT_GAP: f32 = 4.0;
const DIVIDER_GAP: f32 = 8.0;

fn slot_width(s: &Slot) -> f32 {
    theme::TOOL_BUTTON + if s.menu.is_some() { theme::TOOL_CHEVRON_W } else { 0.0 }
}

/// Total toolbar width.
pub fn toolbar_width() -> f32 {
    let mut w = theme::TOOLBELT_PADDING * 2.0;
    for (i, s) in SLOTS.iter().enumerate() {
        if i > 0 {
            w += SLOT_GAP;
        }
        if s.divider {
            w += DIVIDER_GAP * 2.0 + theme::BORDER_WIDTH;
        }
        w += slot_width(s);
    }
    w
}

pub fn toolbar_rect(screen: Rect) -> Rect {
    let w = toolbar_width();
    Rect::from_min_size(
        Pos2::new(screen.center().x - w / 2.0, screen.bottom() - theme::TOOLBELT_BOTTOM_MARGIN - theme::TOOLBELT_HEIGHT),
        Vec2::new(w, theme::TOOLBELT_HEIGHT),
    )
}

/// The tool a slot's button stands for right now.
fn shown_tool(app: &App, s: &Slot) -> Tool {
    if s.menu == Some(ToolMenu::Brush) {
        if app.tool.is_brush() {
            app.tool
        } else {
            app.board.brush
        }
    } else {
        s.tools[0]
    }
}

/// Paints the toolbar and handles its buttons.
pub fn show(app: &mut App, ui: &mut Ui, screen: Rect) {
    let bar = toolbar_rect(screen);
    let p = ui.painter().clone();
    widgets::shadow(&p, bar, theme::RADIUS_FRAME, Vec2::new(0.0, 3.0), 8.0, theme::SHADOW_COLOR);
    widgets::shadow(&p, bar, theme::RADIUS_FRAME, Vec2::new(0.0, 1.0), 3.0, theme::SHADOW_COLOR_STRONG);
    p.rect_filled(bar, theme::cr(theme::RADIUS_FRAME), theme::BG_TOOLBAR);
    p.rect_stroke(bar.shrink(0.5), theme::cr(theme::RADIUS_FRAME), Stroke::new(1.0, theme::INNER_HIGHLIGHT), StrokeKind::Inside);
    ui.interact(bar, ui.id().with("board-toolbar-block"), Sense::click_and_drag());

    let y = bar.top() + (bar.height() - theme::TOOL_BUTTON) / 2.0;
    let mut x = bar.left() + theme::TOOLBELT_PADDING;
    for (i, s) in SLOTS.iter().enumerate() {
        if i > 0 {
            x += SLOT_GAP;
        }
        if s.divider {
            x += DIVIDER_GAP;
            widgets::vline(&p, x, bar.top() + 8.0, bar.bottom() - 8.0, theme::BORDER);
            x += DIVIDER_GAP + theme::BORDER_WIDTH;
        }
        let tool = shown_tool(app, s);
        let selected = s.tools.contains(&app.tool);
        let br = Rect::from_min_size(Pos2::new(x, y), Vec2::splat(theme::TOOL_BUTTON));
        let resp = ui.interact(br, ui.id().with(("board-tool", i)), Sense::click());
        let fill = if selected {
            theme::BG_TOOLBAR_SELECTED
        } else if resp.hovered() {
            theme::BG_TOOLBAR_HOVER
        } else {
            Color32::TRANSPARENT
        };
        if fill != Color32::TRANSPARENT {
            p.rect_filled(br, theme::cr(theme::RADIUS_CONTROL), fill);
        }
        icons::paint(&p, tool_icon(tool), br.center(), theme::TOOL_ICON_SIZE, if selected { Color32::WHITE } else { theme::ICON });
        if resp.clicked() {
            app.board.tool_menu = None;
            app.set_tool(tool);
        }
        let tip = if tool.shortcut().is_empty() { tool.label().to_string() } else { format!("{}   {}", tool.label(), tool.shortcut()) };
        resp.on_hover_text(tip);
        widgets::record(ui, &format!("board-tool-{tool:?}"), br);
        x += theme::TOOL_BUTTON;
        if let Some(menu) = s.menu {
            let cr = Rect::from_min_size(Pos2::new(x, y), Vec2::new(theme::TOOL_CHEVRON_W, theme::TOOL_BUTTON));
            let cresp = ui.interact(cr, ui.id().with(("board-chev", i)), Sense::click());
            let open = app.board.tool_menu.map(|m| m.0) == Some(menu);
            if open || cresp.hovered() {
                p.rect_filled(cr, theme::cr(theme::RADIUS_CONTROL), theme::BG_TOOLBAR_HOVER);
            }
            icons::paint(
                &p,
                Icon::ChevronDown,
                Pos2::new(br.right() + theme::TOOL_CHEVRON_CENTER_DX, cr.center().y),
                theme::TOOL_CHEVRON_ICON,
                theme::ICON_SECONDARY,
            );
            if cresp.clicked() {
                app.board.tool_menu = if open { None } else { Some((menu, cr)) };
            }
            widgets::record(ui, &format!("board-chevron-{menu:?}"), cr);
            x += theme::TOOL_CHEVRON_W;
        }
    }
    show_timer(app, ui, bar);
    popup(app, ui);
}

/// Gap between the toolbar and the timer pill, and between the pill and its controls.
const TIMER_GAP: f32 = 8.0;
/// Width of the timer pill; its height is the toolbar's.
const TIMER_PILL_W: f32 = 84.0;
/// Padding around the timer controls.
const TIMER_HUD_PAD: f32 = 8.0;
/// Gap between two timer controls.
const TIMER_HUD_GAP: f32 = 8.0;
/// Size of one timer control.
const TIMER_BTN: Vec2 = Vec2::new(64.0, 28.0);

fn show_timer(app: &mut App, ui: &mut Ui, bar: Rect) {
    let now = ui.input(|i| i.time);
    if app.board.timer.tick(now) {
        ui.ctx().request_repaint();
    }

    let toggle_r = Rect::from_min_size(Pos2::new(bar.right() + TIMER_GAP, bar.top()), Vec2::new(TIMER_PILL_W, bar.height()));
    widgets::record(ui, "board-timer-toggle", toggle_r);
    let resp = ui.interact(toggle_r, ui.id().with("board-timer-toggle"), Sense::click());
    let p = ui.painter().clone();
    widgets::shadow(&p, toggle_r, theme::RADIUS_FRAME, Vec2::new(0.0, 2.0), 6.0, theme::SHADOW_COLOR);
    let bg = if app.board.timer.running {
        theme::BG_TOOLBAR_SELECTED
    } else if app.board.timer.open || resp.hovered() {
        theme::BG_TOOLBAR_HOVER
    } else {
        theme::BG_TOOLBAR
    };
    p.rect_filled(toggle_r, theme::cr(theme::RADIUS_FRAME), bg);
    p.rect_stroke(toggle_r.shrink(0.5), theme::cr(theme::RADIUS_FRAME), Stroke::new(1.0, theme::INNER_HIGHLIGHT), StrokeKind::Inside);
    icons::paint(
        &p,
        Icon::Clock,
        Pos2::new(toggle_r.left() + 18.0, toggle_r.center().y),
        14.0,
        if app.board.timer.running { Color32::WHITE } else { theme::ICON },
    );
    let label = app.board.timer.format_mm_ss();
    p.text(
        Pos2::new(toggle_r.left() + 32.0, toggle_r.center().y),
        Align2::LEFT_CENTER,
        label,
        theme::font_mono(),
        if app.board.timer.running { Color32::WHITE } else { theme::TEXT },
    );
    if resp.on_hover_text("Facilitator timer").clicked() {
        app.board.timer.open = !app.board.timer.open;
    }

    if app.board.timer.open {
        timer_hud(app, ui, toggle_r, now);
    }
}

/// The Start / Pause, +1 min and Reset controls above the timer pill. They live in a layer of their
/// own, above the canvas chrome, so a contextual bar pinned over the toolbar row can neither cover
/// them nor swallow their clicks; the tool popovers stay above them.
fn timer_hud(app: &mut App, ui: &mut Ui, pill: Rect, now: f64) {
    let ctx = ui.ctx().clone();
    let size = Vec2::new(TIMER_HUD_PAD * 2.0 + TIMER_BTN.x * 3.0 + TIMER_HUD_GAP * 2.0, TIMER_HUD_PAD * 2.0 + TIMER_BTN.y);
    let hud_r = Rect::from_min_size(Pos2::new(pill.right() - size.x, pill.top() - TIMER_GAP - size.y), size);
    // `default_size` makes the very first frame already use the real size; the stock guess is far
    // taller than the controls and would block the toolbar underneath for a few frames
    egui::Area::new(egui::Id::new("board-timer-hud")).order(egui::Order::Middle).fixed_pos(hud_r.min).default_size(size).show(&ctx, |ui| {
        ui.allocate_rect(hud_r, Sense::click_and_drag());
        widgets::record(ui, "board-timer-hud", hud_r);
        let p = ui.painter().clone();
        widgets::menu_panel(&p, hud_r, theme::RADIUS_FRAME);
        let button = |col: f32| {
            Rect::from_min_size(Pos2::new(hud_r.left() + TIMER_HUD_PAD + col * (TIMER_BTN.x + TIMER_HUD_GAP), hud_r.top() + TIMER_HUD_PAD), TIMER_BTN)
        };

        let start_r = button(0.0);
        widgets::record(ui, "board-timer-start-pause", start_r);
        let sresp = ui.interact(start_r, ui.id().with("board-timer-start-pause"), Sense::click());
        let running = app.board.timer.running;
        p.rect_filled(start_r, theme::cr(theme::RADIUS_CONTROL), if running { theme::BG_HOVER } else { theme::BG_BRAND });
        p.text(start_r.center(), Align2::CENTER_CENTER, if running { "Pause" } else { "Start" }, theme::font_body_strong(), Color32::WHITE);
        if sresp.clicked() {
            app.board.timer.toggle(now);
        }

        let add_r = button(1.0);
        widgets::record(ui, "board-timer-add-min", add_r);
        let aresp = ui.interact(add_r, ui.id().with("board-timer-add-min"), Sense::click());
        p.rect_filled(add_r, theme::cr(theme::RADIUS_CONTROL), if aresp.hovered() { theme::BG_HOVER } else { theme::BG_SECONDARY });
        p.text(add_r.center(), Align2::CENTER_CENTER, "+1 min", theme::font_body(), theme::TEXT);
        if aresp.clicked() {
            app.board.timer.add_minute();
        }

        let reset_r = button(2.0);
        widgets::record(ui, "board-timer-reset", reset_r);
        let rresp = ui.interact(reset_r, ui.id().with("board-timer-reset"), Sense::click());
        p.rect_filled(reset_r, theme::cr(theme::RADIUS_CONTROL), if rresp.hovered() { theme::BG_HOVER } else { theme::BG_SECONDARY });
        p.text(reset_r.center(), Align2::CENTER_CENTER, "Reset", theme::font_body(), theme::TEXT_SECONDARY);
        if rresp.clicked() {
            app.board.timer.reset();
        }
    });
}

/// Cell size of the popover grids.
const CELL: f32 = 40.0;

/// The open popover above the toolbar.
fn popup(app: &mut App, ui: &mut Ui) {
    let Some((menu, anchor)) = app.board.tool_menu else { return };
    let ctx = ui.ctx().clone();
    let pad = 8.0;
    let (cols, rows) = match menu {
        ToolMenu::Brush => (4, 3),
        ToolMenu::Shape => (5, 2),
        ToolMenu::Stamp => (5, 1),
    };
    let size = match menu {
        ToolMenu::Brush => Vec2::new(8.0 * 26.0 + pad * 2.0, 2.0 * 32.0 + 34.0 + pad * 2.0),
        _ => Vec2::new(cols as f32 * CELL + pad * 2.0, rows as f32 * CELL + pad * 2.0),
    };
    let screen = ctx.content_rect();
    let pos = Pos2::new((anchor.center().x - size.x / 2.0).clamp(screen.left() + 8.0, screen.right() - size.x - 8.0), anchor.top() - 8.0 - size.y);
    let rect = Rect::from_min_size(pos, size);
    let mut picked: Option<Pick> = None;
    let area = egui::Area::new(egui::Id::new("board-tool-menu")).order(egui::Order::Foreground).fixed_pos(rect.min).show(&ctx, |ui| {
        ui.allocate_rect(rect, Sense::hover());
        let p = ui.painter().clone();
        widgets::menu_panel(&p, rect, theme::RADIUS_FRAME);
        let inner = rect.shrink(pad);
        match menu {
            ToolMenu::Brush => brush_menu(app, ui, inner, &mut picked),
            ToolMenu::Shape => {
                for (i, k) in ShapeKind::ALL.iter().enumerate() {
                    let cell = Rect::from_min_size(inner.min + Vec2::new((i % cols) as f32 * CELL, (i / cols) as f32 * CELL), Vec2::splat(CELL));
                    let sel = app.board.shape == *k;
                    let resp = ui.interact(cell, ui.id().with(("board-shape", i)), Sense::click());
                    cell_bg(&p, cell, sel, resp.hovered());
                    if let Some(t) = app.board.previews.shape(ui.ctx(), *k, color_of(0xE6E6E6), 24.0, 18.0) {
                        p.image(
                            t,
                            Rect::from_center_size(cell.center(), Vec2::new(24.0, 18.0)),
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    }
                    widgets::record(ui, &format!("board-shape-{k:?}"), cell);
                    if resp.on_hover_text(k.label()).clicked() {
                        picked = Some(Pick::Shape(*k));
                    }
                }
            }
            ToolMenu::Stamp => {
                for (i, k) in STAMP_KINDS.iter().enumerate() {
                    let cell = Rect::from_min_size(inner.min + Vec2::new(i as f32 * CELL, 0.0), Vec2::splat(CELL));
                    let sel = app.board.stamp == *k;
                    let resp = ui.interact(cell, ui.id().with(("board-stamp", i)), Sense::click());
                    cell_bg(&p, cell, sel, resp.hovered());
                    if let Some(t) = app.board.previews.stamp(ui.ctx(), k, 28.0) {
                        p.image(
                            t,
                            Rect::from_center_size(cell.center(), Vec2::splat(28.0)),
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    }
                    widgets::record(ui, &format!("board-stamp-{k}"), cell);
                    if resp.on_hover_text(designos_doc::stamp_label(k)).clicked() {
                        picked = Some(Pick::Stamp((*k).to_string()));
                    }
                }
            }
        }
    });
    let layer = area.response.layer_id;
    let (pressed, ptr, esc) = ctx.input(|i| (i.pointer.any_pressed(), i.pointer.interact_pos(), i.key_pressed(egui::Key::Escape)));
    let other_popup = |p: Pos2| ctx.layer_id_at(p).is_some_and(|l| l.order == egui::Order::Foreground && l != layer);
    let outside = pressed && ptr.is_some_and(|p| !rect.contains(p) && !anchor.contains(p) && !other_popup(p));
    match picked {
        Some(Pick::Shape(k)) => {
            app.board.shape = k;
            app.board.tool_menu = None;
            app.set_tool(Tool::BoardShape);
        }
        Some(Pick::Stamp(k)) => {
            app.board.stamp = k;
            app.board.tool_menu = None;
            app.set_tool(Tool::Stamp);
        }
        Some(Pick::Brush(t)) => {
            app.board.brush = t;
            app.set_tool(t);
        }
        Some(Pick::Ink(c)) => app.board.ink = color_of(c),
        None => {
            if outside || esc {
                app.board.tool_menu = None;
            }
        }
    }
}

enum Pick {
    Shape(ShapeKind),
    Stamp(String),
    Brush(Tool),
    Ink(u32),
}

fn cell_bg(p: &egui::Painter, cell: Rect, selected: bool, hover: bool) {
    let r = cell.shrink(2.0);
    if selected {
        p.rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_SELECTED);
    } else if hover {
        p.rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
    }
}

/// Marker / highlighter rows and the ink colours.
fn brush_menu(app: &App, ui: &mut Ui, inner: Rect, picked: &mut Option<Pick>) {
    let p = ui.painter().clone();
    for (i, t) in [Tool::Marker, Tool::Highlighter].into_iter().enumerate() {
        let row = Rect::from_min_size(inner.min + Vec2::new(0.0, i as f32 * 32.0), Vec2::new(inner.width(), 32.0));
        let sel = app.board.brush == t;
        let resp = ui.interact(row, ui.id().with(("board-brush", i)), Sense::click());
        if sel {
            p.rect_filled(row, theme::cr(theme::RADIUS_CONTROL), theme::BG_SELECTED);
        } else if resp.hovered() {
            p.rect_filled(row, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
        }
        icons::paint(&p, tool_icon(t), Pos2::new(row.left() + 18.0, row.center().y), 16.0, theme::ICON);
        p.text(Pos2::new(row.left() + 36.0, row.center().y), Align2::LEFT_CENTER, t.label(), theme::font_body(), theme::TEXT);
        p.text(Pos2::new(row.right() - 8.0, row.center().y), Align2::RIGHT_CENTER, t.shortcut(), theme::font_body(), theme::TEXT_TERTIARY);
        widgets::record(ui, &format!("board-brush-{t:?}"), row);
        if resp.clicked() {
            *picked = Some(Pick::Brush(t));
        }
    }
    let y = inner.top() + 2.0 * 32.0 + 6.0;
    p.hline(inner.x_range(), y - 3.0, Stroke::new(1.0, theme::BORDER));
    for (i, c) in theme::BOARD_INK.iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(inner.left() + i as f32 * 26.0, y), Vec2::splat(26.0));
        let resp = ui.interact(r, ui.id().with(("board-ink", i)), Sense::click());
        let sel = rgb_of(app.board.ink) == *c;
        swatch(&p, r.center(), *c, sel, resp.hovered());
        widgets::record(ui, &format!("board-ink-{i}"), r);
        if resp.clicked() {
            *picked = Some(Pick::Ink(*c));
        }
    }
}

/// A round colour swatch with a ring when selected.
pub fn swatch(p: &egui::Painter, c: Pos2, rgb: u32, selected: bool, hover: bool) {
    let r = theme::BOARD_SWATCH / 2.0;
    p.circle_filled(c, r, color32(color_of(rgb)));
    p.circle_stroke(c, r - 0.5, Stroke::new(1.0, theme::BORDER_TRANSLUCENT));
    if selected {
        p.circle_stroke(c, r + 3.0, Stroke::new(2.0, theme::BG_BRAND));
    } else if hover {
        p.circle_stroke(c, r + 3.0, Stroke::new(1.0, theme::TEXT_TERTIARY));
    }
}
