//! Floating bottom-center toolbar.

use crate::app::{App, MenuKind};
use crate::canvas::tools::{Tool, GROUPS};
use crate::icons::{self, Icon};
use crate::theme;
use crate::widgets;
use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};

pub fn tool_icon(t: Tool) -> Icon {
    match t {
        Tool::Move => Icon::Move,
        Tool::Hand => Icon::Hand,
        Tool::Scale => Icon::Scale,
        Tool::ShapeBuilder => Icon::Boolean,
        Tool::Frame => Icon::Frame,
        Tool::Section => Icon::Section,
        Tool::Slice => Icon::Slice,
        Tool::Rectangle => Icon::Rectangle,
        Tool::Line => Icon::Line,
        Tool::Arrow => Icon::Arrow,
        Tool::Ellipse => Icon::Ellipse,
        Tool::Polygon => Icon::Polygon,
        Tool::Star => Icon::Star,
        Tool::Image => Icon::Image,
        Tool::Pen => Icon::Pen,
        Tool::Pencil => Icon::Pencil,
        Tool::Text => Icon::Text,
        Tool::Comment => Icon::Comment,
        Tool::Sticky => Icon::Sticky,
        Tool::BoardShape => Icon::Shapes,
        Tool::Connector => Icon::Connector,
        Tool::Stamp => Icon::Stamp,
        Tool::Marker => Icon::Marker,
        Tool::Highlighter => Icon::Highlighter,
    }
}

/// Groups with a dropdown chevron (the text tool keeps one for its sub-menu).
pub fn has_chevron(group: &[Tool]) -> bool {
    group.len() > 1 || group.contains(&Tool::Text)
}

/// Width of the mode switcher group.
pub fn mode_switch_width() -> f32 {
    theme::MODE_SEGMENTS as f32 * theme::MODE_SWITCH_W + theme::MODE_SWITCH_PAD * 2.0
}

/// Total toolbar width for the current group set.
pub fn toolbar_width() -> f32 {
    let mut w = theme::TOOLBELT_PADDING * 2.0;
    for (i, g) in GROUPS.iter().enumerate() {
        if i > 0 {
            w += theme::TOOL_GROUP_GAP;
        }
        w += theme::TOOL_BUTTON;
        if has_chevron(g) {
            w += theme::TOOL_CHEVRON_GAP + theme::TOOL_CHEVRON_W;
        }
    }
    w + theme::TOOL_GROUP_GAP
        + theme::TOOL_BUTTON
        + theme::TOOLBAR_DIVIDER_GAP_BEFORE
        + theme::BORDER_WIDTH
        + theme::TOOLBAR_DIVIDER_GAP_AFTER
        + mode_switch_width()
}

pub fn toolbar_rect(screen: Rect) -> Rect {
    let w = toolbar_width();
    Rect::from_min_size(
        Pos2::new(screen.center().x - w / 2.0, screen.bottom() - theme::TOOLBELT_BOTTOM_MARGIN - theme::TOOLBELT_HEIGHT),
        Vec2::new(w, theme::TOOLBELT_HEIGHT),
    )
}

/// The 32px button of toolbar group `gi`.
pub fn group_button_rect(screen: Rect, gi: usize) -> Rect {
    let bar = toolbar_rect(screen);
    let mut x = bar.left() + theme::TOOLBELT_PADDING;
    let y = bar.top() + (bar.height() - theme::TOOL_BUTTON) / 2.0;
    for (i, g) in GROUPS.iter().enumerate() {
        if i > 0 {
            x += theme::TOOL_GROUP_GAP;
        }
        if i == gi {
            break;
        }
        x += theme::TOOL_BUTTON;
        if has_chevron(g) {
            x += theme::TOOL_CHEVRON_GAP + theme::TOOL_CHEVRON_W;
        }
    }
    Rect::from_min_size(Pos2::new(x, y), Vec2::splat(theme::TOOL_BUTTON))
}

/// The chevron hit area right of group `gi`'s button.
pub fn chevron_rect(screen: Rect, gi: usize) -> Rect {
    let b = group_button_rect(screen, gi);
    Rect::from_min_size(Pos2::new(b.right() + theme::TOOL_CHEVRON_GAP, b.top()), Vec2::new(theme::TOOL_CHEVRON_W, theme::TOOL_BUTTON))
}

/// Top-left of group `gi`'s dropdown of height `h`: above the button, slightly overlapping the bar.
pub fn tool_menu_pos(screen: Rect, gi: usize, h: f32) -> Pos2 {
    let b = group_button_rect(screen, gi);
    Pos2::new(b.right() + theme::TOOL_MENU_DX, toolbar_rect(screen).top() + theme::TOOL_MENU_OVERLAP - h)
}

pub fn show(app: &mut App, ui: &mut Ui, screen: Rect) {
    if app.ui.dev_mode {
        show_dev_mode_toolbar(app, ui, screen);
        return;
    }
    let figma_blue = Color32::from_rgb(0x0D, 0x99, 0xFF);
    let bar = toolbar_rect(screen);
    let p = ui.painter().clone();

    // 1:1 Figma UI3 Contextual Floating Action Bar (`figma_live_editor.png`):
    // Rendered 8px above the main bottom toolbar when a layer/frame is selected in Design Mode
    // and no popup menu, toast, popover, or Actions modal is covering that region.
    if !app.selection.is_empty()
        && app.canvas.vedit.is_none()
        && app.ui.menu.is_none()
        && !app.cmdk.open
        && app.toast.is_none()
        && app.props.pop.is_none()
        && app.props.picker.is_none()
    {
        let ctx_w = 128.0;
        let ctx_h = 34.0;
        let ctx_bar = Rect::from_min_size(Pos2::new((screen.center().x - ctx_w * 0.5).round(), bar.top() - 8.0 - ctx_h), Vec2::new(ctx_w, ctx_h));
        widgets::record(ui, "contextual-floating-bar", ctx_bar);
        widgets::shadow(&p, ctx_bar, 8, Vec2::new(0.0, 2.0), 6.0, theme::SHADOW_COLOR);
        p.rect_filled(ctx_bar, theme::cr(8), theme::BG_TOOLBAR);
        p.rect_stroke(ctx_bar.shrink(0.5), theme::cr(8), Stroke::new(1.0, Color32::from_rgb(0x38, 0x38, 0x38)), egui::StrokeKind::Inside);
        ui.interact(ctx_bar, ui.id().with("ctx-toolbar-block"), Sense::click_and_drag());

        let mut cx = ctx_bar.left() + 4.0;
        let cy = ctx_bar.top() + 3.0;
        let ctx_items: [(Icon, &str, &str); 2] = [(Icon::Rectangle, "ctx-select-area", "Select area"), (Icon::Pen, "ctx-edit-prompt", "Edit with prompt")];
        for (icon, key, tip) in ctx_items {
            let br = Rect::from_min_size(Pos2::new(cx, cy), Vec2::splat(28.0));
            widgets::record(ui, key, br);
            let resp = ui.interact(br, ui.id().with(key), Sense::click());
            if resp.hovered() {
                p.rect_filled(br, theme::cr(theme::RADIUS_CONTROL), theme::BG_TOOLBAR_HOVER);
            }
            icons::paint(&p, icon, br.center(), 15.0, theme::ICON);
            if resp.on_hover_text(tip).clicked() {
                apply_contextual_action(app, key);
            }
            cx += 30.0;
        }
        cx += 3.0;
        widgets::vline(&p, cx, ctx_bar.top() + 6.0, ctx_bar.bottom() - 6.0, theme::BORDER);
        cx += 5.0;
        let more_r = Rect::from_min_size(Pos2::new(cx, cy), Vec2::new(48.0, 28.0));
        let more_resp = ui.interact(more_r, ui.id().with("ctx-more"), Sense::click());
        if more_resp.hovered() {
            p.rect_filled(more_r, theme::cr(theme::RADIUS_CONTROL), theme::BG_TOOLBAR_HOVER);
        }
        p.text(Pos2::new(more_r.left() + 6.0, more_r.center().y), egui::Align2::LEFT_CENTER, "More", theme::font_caption(), theme::TEXT);
        icons::paint(&p, Icon::ChevronDown, Pos2::new(more_r.right() - 9.0, more_r.center().y), theme::TOOL_CHEVRON_ICON, theme::ICON_SECONDARY);
        if more_resp.on_hover_text("More actions   Ctrl+K").clicked() {
            app.cmdk.open = true;
            app.cmdk.focus_requested = true;
        }
    }

    widgets::shadow(&p, bar, theme::RADIUS_FRAME, Vec2::new(0.0, 3.0), 8.0, theme::SHADOW_COLOR);
    widgets::shadow(&p, bar, theme::RADIUS_FRAME, Vec2::new(0.0, 1.0), 3.0, theme::SHADOW_COLOR_STRONG);
    p.rect_filled(bar, theme::cr(theme::RADIUS_FRAME), theme::BG_TOOLBAR);
    p.rect_stroke(bar.shrink(0.5), theme::cr(theme::RADIUS_FRAME), Stroke::new(1.0, theme::INNER_HIGHLIGHT), egui::StrokeKind::Inside);
    // Block canvas interaction under the bar.
    ui.interact(bar, ui.id().with("toolbar-block"), Sense::click_and_drag());

    let mut x = bar.left() + theme::TOOLBELT_PADDING;
    let y = bar.top() + (bar.height() - theme::TOOL_BUTTON) / 2.0;
    for (gi, group) in GROUPS.iter().enumerate() {
        if gi > 0 {
            x += theme::TOOL_GROUP_GAP;
        }
        let pick = app.group_pick[gi].min(group.len() - 1);
        let tool = group[pick];
        let selected = group.contains(&app.tool);
        let shown = if selected { app.tool } else { tool };
        let br = Rect::from_min_size(Pos2::new(x, y), Vec2::splat(theme::TOOL_BUTTON));
        let resp = ui.interact(br, ui.id().with(("tool", gi)), Sense::click());
        let has_chev = has_chevron(group);
        // Only the 32x32 icon button takes the selected fill: a neutral fill, never an accent (P2).
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
        icons::paint(&p, tool_icon(shown), br.center(), theme::TOOL_ICON_SIZE, if selected { Color32::WHITE } else { theme::ICON });
        if resp.clicked() {
            app.set_tool(tool);
        }
        let tip = if tool.shortcut().is_empty() { tool.label().to_string() } else { format!("{}   {}", tool.label(), tool.shortcut()) };
        resp.on_hover_text(tip);
        x += theme::TOOL_BUTTON;
        if has_chev {
            x += theme::TOOL_CHEVRON_GAP;
            let cr = Rect::from_min_size(Pos2::new(x, y), Vec2::new(theme::TOOL_CHEVRON_W, theme::TOOL_BUTTON));
            let cresp = ui.interact(cr, ui.id().with(("tool-chev", gi)), Sense::click());
            let open = app.ui.menu == Some(MenuKind::ToolGroup(gi));
            // The chevron stays transparent unless hovered or its menu is open.
            if open || cresp.hovered() {
                p.rect_filled(cr, theme::cr(theme::RADIUS_CONTROL), theme::BG_TOOLBAR_HOVER);
            }
            let cc = Pos2::new(br.right() + theme::TOOL_CHEVRON_CENTER_DX, cr.center().y);
            icons::paint(&p, Icon::ChevronDown, cc, theme::TOOL_CHEVRON_ICON, theme::ICON_SECONDARY);
            if cresp.clicked() {
                app.ui.menu = if open { None } else { Some(MenuKind::ToolGroup(gi)) };
                app.ui.menu_anchor = Some(cr);
            }
            x += theme::TOOL_CHEVRON_W;
        }
    }

    // Actions button (`figma_ui3_actions_cmdk.png`: fills #0D99FF Figma Blue when Actions is open).
    x += theme::TOOL_GROUP_GAP;
    let ar = Rect::from_min_size(Pos2::new(x, y), Vec2::splat(theme::TOOL_BUTTON));
    widgets::record(ui, "toolbar-actions-btn", ar);
    let aresp = ui.interact(ar, ui.id().with("actions"), Sense::click());
    let actions_open = app.cmdk.open || app.ui.menu == Some(MenuKind::Palette);
    if actions_open {
        p.rect_filled(ar, theme::cr(theme::RADIUS_CONTROL), figma_blue);
    } else if aresp.hovered() {
        p.rect_filled(ar, theme::cr(theme::RADIUS_CONTROL), theme::BG_TOOLBAR_HOVER);
    }
    icons::paint(&p, Icon::Sparkle, ar.center(), theme::TOOL_ICON_SIZE, if actions_open { Color32::WHITE } else { theme::ICON });
    if aresp.on_hover_text("Actions   Ctrl+K").clicked() {
        super::menus::run(app, ui.ctx(), super::menus::Action::OpenPalette);
    }
    x += theme::TOOL_BUTTON + theme::TOOLBAR_DIVIDER_GAP_BEFORE;

    // Full-height 1px divider before the mode switcher.
    widgets::vline(&p, x, bar.top() + 1.0, bar.bottom() - 1.0, theme::BORDER);
    x += theme::BORDER_WIDTH + theme::TOOLBAR_DIVIDER_GAP_AFTER;

    // Mode switcher: 3 Figma UI3 segments (Draw | Design | Dev mode) in the right pill.
    let sw = Rect::from_min_size(Pos2::new(x, y), Vec2::new(mode_switch_width(), theme::TOOL_BUTTON));
    p.rect_filled(sw, theme::cr(theme::RADIUS_CONTROL), theme::BG_TOOLBAR_MODE_SWITCHER);
    let modes = [(Icon::Pencil, "Draw"), (Icon::Design, "Design"), (Icon::Dev, "Dev mode")];
    let seg_w = (mode_switch_width() - theme::MODE_SWITCH_PAD * 2.0) / modes.len() as f32;
    for (i, (icon, label)) in modes.into_iter().enumerate() {
        let r = Rect::from_min_size(
            Pos2::new(x + theme::MODE_SWITCH_PAD + i as f32 * seg_w, y + theme::MODE_SWITCH_PAD),
            Vec2::new(seg_w, theme::TOOL_BUTTON - theme::MODE_SWITCH_PAD * 2.0),
        );
        let mode_key = match i {
            0 => "toolbar-mode-draw",
            1 => "toolbar-mode-design",
            _ => "toolbar-mode-dev",
        };
        widgets::record(ui, mode_key, r);
        let resp = ui.interact(r, ui.id().with(("mode", i)), Sense::click());
        let sel = if app.ui.dev_mode {
            i == 2
        } else if matches!(app.tool, Tool::Pencil | Tool::Pen) {
            i == 0
        } else {
            i == 1
        };
        let dev_green = Color32::from_rgb(0x1B, 0xC4, 0x7D);
        if sel {
            // Active segment: Dev Mode uses Figma UI3 #1BC47D green; Design/Draw use darker inset tile with #0D99FF icon.
            let active_bg = if i == 2 { dev_green } else { theme::BG_TOOLBAR };
            p.rect_filled(r, theme::cr(theme::RADIUS_SEGMENT), active_bg);
        } else if resp.hovered() {
            p.rect_filled(r, theme::cr(theme::RADIUS_SEGMENT), theme::BG_TOOLBAR_HOVER);
        }
        let icon_col = if sel && i == 2 {
            Color32::WHITE
        } else if sel {
            figma_blue
        } else if i == 2 && resp.hovered() {
            dev_green
        } else {
            theme::ICON_SECONDARY
        };
        icons::paint(&p, icon, r.center(), 16.0, icon_col);
        if resp.on_hover_text(label).clicked() {
            match i {
                0 => {
                    app.ui.dev_mode = false;
                    app.ui.right_tab = Some(crate::app::RightTab::Design);
                    app.set_tool(Tool::Pencil);
                }
                1 => {
                    app.ui.dev_mode = false;
                    app.ui.right_tab = Some(crate::app::RightTab::Design);
                    if matches!(app.tool, Tool::Pencil | Tool::Pen) {
                        app.set_tool(Tool::Move);
                    }
                }
                _ => {
                    app.ui.dev_mode = true;
                    app.ui.right_tab = Some(crate::app::RightTab::Design);
                }
            }
        }
    }
}

/// 1:1 Figma UI3 Dev Mode 5-tool bottom floating dock + floating `↩ Re-center  ⇧2` pill (`Gap C`).
fn show_dev_mode_toolbar(app: &mut App, ui: &mut Ui, screen: Rect) {
    let dev_green = Color32::from_rgb(0x1B, 0xC4, 0x7D);
    let dev_toggle_w = 36.0;
    let tools_count = 5usize;
    let w = theme::TOOLBELT_PADDING * 2.0
        + tools_count as f32 * theme::TOOL_BUTTON
        + (tools_count - 1) as f32 * theme::TOOL_GROUP_GAP
        + theme::TOOLBAR_DIVIDER_GAP_BEFORE
        + theme::BORDER_WIDTH
        + theme::TOOLBAR_DIVIDER_GAP_AFTER
        + dev_toggle_w;

    let bar = Rect::from_min_size(
        Pos2::new(screen.center().x - w / 2.0, screen.bottom() - theme::TOOLBELT_BOTTOM_MARGIN - theme::TOOLBELT_HEIGHT),
        Vec2::new(w, theme::TOOLBELT_HEIGHT),
    );
    let p = ui.painter().clone();

    // Floating `↩ Re-center  ⇧2` pill 8px above the Dev Mode bottom toolbar
    let recenter_rect = Rect::from_min_size(Pos2::new(bar.center().x - 59.0, bar.top() - 8.0 - 28.0), Vec2::new(118.0, 28.0));
    widgets::record(ui, "dev-recenter", recenter_rect);
    let recenter_resp = ui.interact(recenter_rect, ui.id().with("dev-recenter"), Sense::click());
    widgets::shadow(&p, recenter_rect, 6, Vec2::new(0.0, 2.0), 6.0, theme::SHADOW_COLOR);
    p.rect_filled(recenter_rect, theme::cr(6), if recenter_resp.hovered() { Color32::from_rgb(0x36, 0x36, 0x36) } else { Color32::from_rgb(0x2C, 0x2C, 0x2C) });
    p.rect_stroke(recenter_rect, theme::cr(6), Stroke::new(1.0, Color32::from_rgb(0x38, 0x38, 0x38)), egui::StrokeKind::Inside);
    p.text(
        Pos2::new(recenter_rect.left() + 10.0, recenter_rect.center().y),
        egui::Align2::LEFT_CENTER,
        "\u{21A9} Re-center",
        theme::font_caption_strong(),
        theme::TEXT,
    );
    p.text(
        Pos2::new(recenter_rect.right() - 10.0, recenter_rect.center().y),
        egui::Align2::RIGHT_CENTER,
        "\u{21E7}2",
        theme::font_caption(),
        theme::TEXT_SECONDARY,
    );
    if recenter_resp.on_hover_text("Re-center canvas   Shift+2").clicked() {
        if app.selection.is_empty() {
            app.fit_all();
        } else {
            app.fit_selection();
        }
    }

    // Bottom floating dock container
    widgets::shadow(&p, bar, theme::RADIUS_FRAME, Vec2::new(0.0, 3.0), 8.0, theme::SHADOW_COLOR);
    widgets::shadow(&p, bar, theme::RADIUS_FRAME, Vec2::new(0.0, 1.0), 3.0, theme::SHADOW_COLOR_STRONG);
    p.rect_filled(bar, theme::cr(theme::RADIUS_FRAME), theme::BG_TOOLBAR);
    p.rect_stroke(bar.shrink(0.5), theme::cr(theme::RADIUS_FRAME), Stroke::new(1.0, theme::INNER_HIGHLIGHT), egui::StrokeKind::Inside);
    ui.interact(bar, ui.id().with("toolbar-block-dev"), Sense::click_and_drag());

    let mut x = bar.left() + theme::TOOLBELT_PADDING;
    let y = bar.top() + (bar.height() - theme::TOOL_BUTTON) / 2.0;

    let dev_tools: [(Icon, &str, bool); 5] = [
        (Icon::Move, "Select   V", app.tool == Tool::Move && app.dev.annot.is_none()),
        (Icon::Pen, "Annotate   Y", app.dev.annot.is_some()),
        (Icon::Spacing, "Measure   M", app.ui.rulers),
        (Icon::Comment, "Comment   C", app.tool == Tool::Comment),
        (Icon::Sparkle, "Actions   Ctrl+K", app.cmdk.open),
    ];

    for (idx, (icon, tip, active)) in dev_tools.into_iter().enumerate() {
        if idx > 0 {
            x += theme::TOOL_GROUP_GAP;
        }
        let br = Rect::from_min_size(Pos2::new(x, y), Vec2::splat(theme::TOOL_BUTTON));
        let resp = ui.interact(br, ui.id().with(("dev-tool", idx)), Sense::click());
        let fill = if active {
            theme::BG_TOOLBAR_SELECTED
        } else if resp.hovered() {
            theme::BG_TOOLBAR_HOVER
        } else {
            Color32::TRANSPARENT
        };
        if fill != Color32::TRANSPARENT {
            p.rect_filled(br, theme::cr(theme::RADIUS_CONTROL), fill);
        }
        let icon_col = if active { Color32::WHITE } else { theme::ICON };
        icons::paint(&p, icon, br.center(), theme::TOOL_ICON_SIZE, icon_col);
        if resp.on_hover_text(tip).clicked() {
            match idx {
                0 => {
                    app.dev.annot = None;
                    app.set_tool(Tool::Move);
                }
                1 => {
                    if app.dev.annot.is_some() {
                        app.dev.annot = None;
                    } else if let Some(&id) = app.selection.first() {
                        app.dev.annot = Some(crate::dev::AnnotEdit { node: id, index: None, text: String::new(), props: Vec::new() });
                    } else {
                        app.set_tool(Tool::Comment);
                    }
                }
                2 => {
                    app.ui.rulers = !app.ui.rulers;
                }
                3 => {
                    app.dev.annot = None;
                    app.set_tool(Tool::Comment);
                }
                _ => {
                    app.cmdk.open = !app.cmdk.open;
                    if app.cmdk.open {
                        app.cmdk.last_audit = Some(crate::cmdk::run_design_gate_audit(&app.doc, app.page, &app.selection));
                    }
                }
            }
        }
        x += theme::TOOL_BUTTON;
    }

    x += theme::TOOLBAR_DIVIDER_GAP_BEFORE;
    widgets::vline(&p, x, bar.top() + 1.0, bar.bottom() - 1.0, theme::BORDER);
    x += theme::BORDER_WIDTH + theme::TOOLBAR_DIVIDER_GAP_AFTER;

    // Solid green `</>` Dev Mode toggle button
    let toggle_r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(dev_toggle_w, theme::TOOL_BUTTON));
    widgets::record(ui, "dev-mode-toggle", toggle_r);
    let toggle_resp = ui.interact(toggle_r, ui.id().with("dev-mode-toggle"), Sense::click());
    p.rect_filled(toggle_r, theme::cr(theme::RADIUS_CONTROL), if toggle_resp.hovered() { Color32::from_rgb(0x22, 0xD4, 0x89) } else { dev_green });
    icons::paint(&p, Icon::Dev, toggle_r.center(), 16.0, Color32::WHITE);
    if toggle_resp.on_hover_text("Exit Dev Mode   Shift+D").clicked() {
        app.ui.dev_mode = false;
        app.ui.right_tab = Some(crate::app::RightTab::Design);
    }
}

/// Executes a real document/tool mutation when a button in the Figma UI3
/// Selection Contextual Bar (`ctx-select-area`, `ctx-edit-prompt`) is clicked.
pub fn apply_contextual_action(app: &mut App, key: &str) {
    let sel = app.selection.clone();
    match key {
        "ctx-select-area" => {
            if sel.len() == 1 {
                if let Some(children) = app.doc.get(sel[0]).map(|n| n.children.clone()) {
                    if !children.is_empty() {
                        let count = children.len();
                        app.selection = children;
                        app.changed();
                        app.show_toast(format!("Selected {count} child layer(s) inside area"), false);
                        return;
                    }
                }
            }
            app.set_tool(Tool::Slice);
            app.show_toast("Slice area tool active — drag on canvas to select region", false);
        }
        "ctx-edit-prompt" => {
            if app.cmdk.query.trim().is_empty() {
                if let Some(first_name) = sel.first().and_then(|id| app.doc.get(*id)).map(|n| n.name.trim().to_string()).filter(|s| !s.is_empty()) {
                    app.cmdk.query = first_name;
                }
            }
            app.cmdk.active_tab = 0;
            app.cmdk.open = true;
            app.cmdk.focus_requested = true;
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_is_centered_and_above_bottom() {
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 1088.0));
        let r = toolbar_rect(screen);
        assert!((r.center().x - 800.0).abs() < 0.5);
        assert_eq!(r.bottom(), 1088.0 - theme::TOOLBELT_BOTTOM_MARGIN);
        assert_eq!(r.height(), theme::TOOLBELT_HEIGHT);
    }

    #[test]
    fn toolbelt_is_512_wide_like_figma() {
        assert_eq!(toolbar_width(), 512.0);
        assert_eq!(mode_switch_width(), 122.0);
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let r = toolbar_rect(screen);
        assert_eq!((r.left(), r.right()), (144.0, 656.0));
    }

    #[test]
    fn every_tool_has_an_icon() {
        for g in GROUPS {
            for t in *g {
                let _ = tool_icon(*t);
            }
        }
    }

    #[test]
    fn end_to_end_figma_ui3_userflows_uf01_to_uf05() {
        use crate::app::{RailItem, RightTab};
        use designos_doc::{Document, Node};

        let mut d = Document::new("FlowTest");
        let page = d.add_page("Page 1");
        let f = d.alloc_id();
        let f = d.insert(page, Node::frame(f, designos_doc::Rect::new(0.0, 0.0, 320.0, 200.0))).unwrap();
        let mut app = App::new(d);
        app.selection = vec![f];

        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 900.0));

        let run_frame = |app: &mut App, click: Option<Pos2>| {
            let mut events = Vec::new();
            if let Some(pos) = click {
                events.push(egui::Event::PointerMoved(pos));
                events.push(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
                events.push(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
            }
            let raw = egui::RawInput { screen_rect: Some(screen), events, ..Default::default() };
            let _ = ctx.run_ui(raw, |ui| {
                crate::panels::rail::show(app, ui, Rect::from_min_size(Pos2::new(0.0, 32.0), Vec2::new(40.0, 868.0)));
                crate::panels::left::show(app, ui, Rect::from_min_size(Pos2::new(40.0, 32.0), Vec2::new(240.0, 868.0)));
                crate::panels::toolbar::show(app, ui, screen);
            });
        };
        let get_rect = |name: &str| ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", name.to_string()))));

        // Initial frame: File (Pages & Layers) is active.
        run_frame(&mut app, None);
        assert_eq!(app.ui.rail, None);

        // UF-01 & UF-02: Click Agents in Left Rail -> opens full-height Agents Left Side Panel (top < 60.0).
        let agents_btn = get_rect("rail-Agents").expect("rail-Agents rect");
        run_frame(&mut app, Some(agents_btn.center()));
        assert_eq!(app.ui.rail, Some(RailItem::Agents));
        run_frame(&mut app, None);
        let agents_hdr = get_rect("agents-runtime-add-btn").or_else(|| get_rect("agents-header-back")).expect("agents header rect");
        assert!(agents_hdr.top() < 60.0, "Agents header sits at top of Left Side Panel without duplicate file_header");

        // UF-01 & UF-02: Click Assets in Left Rail -> opens full-height Assets Left Side Panel without duplicate file_header.
        let assets_btn = get_rect("rail-Assets").expect("rail-Assets rect");
        run_frame(&mut app, Some(assets_btn.center()));
        assert_eq!(app.ui.rail, Some(RailItem::Assets));
        run_frame(&mut app, None);
        let assets_lib_btn = get_rect("assets-libraries").expect("assets-libraries rect");
        assert!(assets_lib_btn.top() < 60.0, "Assets header sits at top of Left Side Panel without duplicate file_header");

        // UF-01: Click Variables -> opens Variables view; then click File -> closes Variables and returns to File (Pages & Layers) in 1 click!
        let vars_btn = get_rect("rail-Variables").expect("rail-Variables rect");
        run_frame(&mut app, Some(vars_btn.center()));
        assert!(app.vars.open);
        let file_btn = get_rect("rail-File").expect("rail-File rect");
        run_frame(&mut app, Some(file_btn.center()));
        assert!(!app.vars.open, "Variables view closed in 1 click when clicking File");
        assert_eq!(app.ui.rail, None, "Returned to File (Pages & Layers) in 1 click");
        assert!(!app.ui.left_collapsed);

        // UF-03: Bottom Toolbar 3-button Mode Switcher (Draw | Design | Dev mode) <-> Right Panel bidirectional sync.
        run_frame(&mut app, None);
        let draw_mode_btn = get_rect("toolbar-mode-draw").expect("toolbar-mode-draw");
        run_frame(&mut app, Some(draw_mode_btn.center()));
        assert_eq!(app.tool, Tool::Pencil);
        assert!(!app.ui.dev_mode);

        let design_mode_btn = get_rect("toolbar-mode-design").expect("toolbar-mode-design");
        run_frame(&mut app, Some(design_mode_btn.center()));
        assert_eq!(app.tool, Tool::Move);
        assert_eq!(app.ui.right_tab, Some(RightTab::Design));

        let dev_mode_btn = get_rect("toolbar-mode-dev").expect("toolbar-mode-dev");
        run_frame(&mut app, Some(dev_mode_btn.center()));
        assert!(app.ui.dev_mode);
        run_frame(&mut app, None);
        let dev_exit_btn = get_rect("dev-mode-toggle").expect("dev-mode-toggle");
        run_frame(&mut app, Some(dev_exit_btn.center()));
        assert!(!app.ui.dev_mode);
        assert_eq!(app.ui.right_tab, Some(RightTab::Design));

        // UF-04: Home <-> Multi-Tab Document Workspace roundtrip preserves open tabs & selection.
        let tmp_lib = std::env::temp_dir().join(format!("designos-uf04-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp_lib);
        app.attach_library(tmp_lib.clone());
        let mut tabs = std::mem::take(&mut app.tabs);
        tabs.sync_from_app(&app);
        app.tabs = tabs;
        app.go_home();
        assert!(app.home.is_some());
        assert!(!app.tabs.entries.is_empty());
        app.switch_tab(0);
        assert!(app.home.is_none());
        assert_eq!(app.selection, vec![f]);
        let _ = std::fs::remove_dir_all(&tmp_lib);

        // UF-05: Applying a NodeSpec via execute_terminal_actions mutates the Canvas with undo history.
        let spec_json = r#"{"name":"Pricing Card","preset":"easeui-card"}"#.to_string();
        crate::shell::execute_terminal_actions(&mut app, vec![crate::terminal::TerminalAction::ApplySpecJson(spec_json)]);
        assert!(app.hist.can_undo(), "Applying NodeSpec records undo history");
    }

    #[test]
    fn contextual_floating_bar_buttons_mutate_selection_and_tool_state() {
        use designos_doc::{Color, Document, Node, Paint};

        let mut d = Document::new("CtxBarTest");
        let page = d.add_page("Page 1");
        let frame_id = d.alloc_id();
        let mut frame = Node::frame(frame_id, designos_doc::Rect::new(0.0, 0.0, 200.0, 120.0));
        frame.name = "Hero Card".into();
        frame.fills = vec![Paint::solid(Color::from_hex("#18181B").unwrap())];
        if let Some(fp) = frame.kind.frame_props_mut() {
            fp.clips_content = false;
        }
        let frame_id = d.insert(page, frame).unwrap();

        let child_id = d.alloc_id();
        let mut child = Node::frame(child_id, designos_doc::Rect::new(10.0, 10.0, 100.0, 80.0));
        child.fills = vec![Paint::solid(Color::from_hex("#E36323").unwrap())];
        let child_id = d.insert(frame_id, child).unwrap();

        let mut app = App::new(d);
        app.selection = vec![frame_id];

        // 1. Select area on container selects its children; on leaf node activates Tool::Slice
        apply_contextual_action(&mut app, "ctx-select-area");
        assert_eq!(app.selection, vec![child_id]);
        apply_contextual_action(&mut app, "ctx-select-area");
        assert_eq!(app.tool, Tool::Slice);

        // 2. Edit with prompt opens CmdK seeded with selected node name
        app.selection = vec![frame_id];
        apply_contextual_action(&mut app, "ctx-edit-prompt");
        assert!(app.cmdk.open);
        assert_eq!(app.cmdk.query, "Hero Card");
    }

    #[test]
    fn retired_contextual_actions_do_not_touch_the_document() {
        use designos_doc::{Color, Document, Node, Paint};

        let mut d = Document::new("Retired");
        let page = d.add_page("Page 1");
        let id = d.alloc_id();
        let mut frame = Node::frame(id, designos_doc::Rect::new(0.0, 0.0, 200.0, 120.0));
        frame.fills = vec![Paint::solid(Color::from_hex("#18181B").unwrap())];
        let id = d.insert(page, frame).unwrap();
        let mut app = App::new(d);
        app.selection = vec![id];
        for key in ["ctx-crop", "ctx-remove-bg"] {
            apply_contextual_action(&mut app, key);
        }
        assert_eq!(app.doc.get(id).unwrap().fills.len(), 1, "no layer fill may be removed by a retired action");
        assert!(!app.hist.can_undo());
    }
}
