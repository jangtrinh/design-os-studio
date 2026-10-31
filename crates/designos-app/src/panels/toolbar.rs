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

/// Top-left of the dropdown of height `h` that hangs off `chevron`: above the group's button, slightly overlapping the
/// bar. It is placed from the chevron that opened it, so it stays against that chevron wherever the toolbar is laid out
/// (the window, or a deck's canvas).
pub fn tool_menu_pos(chevron: Rect, h: f32) -> Pos2 {
    let bar_top = chevron.top() - (theme::TOOLBELT_HEIGHT - theme::TOOL_BUTTON) / 2.0;
    Pos2::new(chevron.left() - theme::TOOL_CHEVRON_GAP + theme::TOOL_MENU_DX, bar_top + theme::TOOL_MENU_OVERLAP - h)
}

/// Whether the primary button went down inside `rect` this frame and no popup, floating panel or dialog (a layer above
/// the one the toolbar is drawn in) was over that spot.
///
/// Read from the button events, not from a widget response: egui resolves a press against the pointer position at the
/// END of the frame, so a press followed by a move in the same frame (a slow frame or a quick flick) never reaches the
/// widget it began on.
fn primary_pressed_in(ui: &Ui, rect: Rect) -> bool {
    let presses: Vec<Pos2> = ui.input(|i| {
        i.events
            .iter()
            .filter_map(|e| match e {
                egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, .. } => Some(*pos),
                _ => None,
            })
            .collect()
    });
    let (ctx, own) = (ui.ctx(), ui.layer_id());
    presses.into_iter().any(|p| rect.contains(p) && ctx.layer_id_at(p).is_none_or(|l| l == own))
}

fn tool_look(resp: &egui::Response, selected: bool) -> crate::widgets::kit::Look {
    if selected && resp.hovered() && !resp.is_pointer_button_down_on() {
        crate::widgets::kit::selected_hover(
            crate::widgets::kit::button_look(crate::widgets::kit::Hierarchy::Tool, crate::widgets::kit::State::Selected),
            crate::widgets::kit::button_look(crate::widgets::kit::Hierarchy::Tool, crate::widgets::kit::State::Hover),
        )
    } else {
        crate::widgets::kit::button_look(crate::widgets::kit::Hierarchy::Tool, crate::widgets::kit::state_of(resp, selected, true))
    }
}

fn mark_focus(ui: &Ui, p: &egui::Painter, id: egui::Id, rect: Rect, radius: f32) {
    if crate::widgets::kit::keyboard_focus(ui, id) {
        crate::widgets::kit::focus_ring(p, rect, radius);
    }
}

/// Selection actions above the main toolbar. Hidden while a menu, toast, or popover covers that band.
fn contextual_bar(app: &mut App, ui: &mut Ui, screen: Rect, bar: Rect) {
    if app.selection.is_empty()
        || app.canvas.vedit.is_some()
        || app.ui.menu.is_some()
        || app.cmdk.open
        || app.toast.is_some()
        || app.props.pop.is_some()
        || app.props.picker.is_some()
    {
        return;
    }
    let p = ui.painter().clone();
    let more_font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold);
    let more_w =
        (theme::INSET_CONTROL + widgets::text_width(&p, "More", &more_font) + theme::SPACE_FACT + theme::LAYER_ICON_SIZE + theme::INSET_CONTROL).ceil();
    let step = theme::CONTROL_XS + theme::INSET_SEGMENT;
    let ctx_w = theme::SPACE_FACT + step * 2.0 + 3.0 + 5.0 + more_w + theme::SPACE_FACT;
    let ctx_h = theme::CONTROL_MD;
    let ctx_bar = Rect::from_min_size(Pos2::new((screen.center().x - ctx_w * 0.5).round(), bar.top() - theme::SPACE_INLINE - ctx_h), Vec2::new(ctx_w, ctx_h));
    widgets::record(ui, "contextual-floating-bar", ctx_bar);
    let surface = crate::widgets::kit::Look {
        fill: theme::BG_TOOLBAR,
        border: theme::INNER_HIGHLIGHT,
        border_w: theme::BORDER_WIDTH,
        text: Color32::TRANSPARENT,
        icon: Color32::TRANSPARENT,
        mark: Color32::TRANSPARENT,
    };
    crate::widgets::kit::paint_frame(&p, ctx_bar, theme::RADIUS_XL, &surface);
    ui.interact(ctx_bar, ui.id().with("ctx-toolbar-block"), Sense::click_and_drag());

    let mut cx = ctx_bar.left() + theme::SPACE_FACT;
    let cy = ctx_bar.top() + (ctx_h - theme::CONTROL_XS) * 0.5;
    let ctx_items: [(Icon, &str, &str); 2] = [(Icon::Rectangle, "ctx-select-area", "Select area"), (Icon::Pen, "ctx-edit-prompt", "Edit with prompt")];
    for (icon, key, tip) in ctx_items {
        let br = Rect::from_min_size(Pos2::new(cx, cy), Vec2::splat(theme::CONTROL_XS));
        widgets::record(ui, key, br);
        let resp = ui.interact(br, ui.id().with(key), Sense::click());
        let look = crate::widgets::kit::button_look(crate::widgets::kit::Hierarchy::Icon, crate::widgets::kit::state_of(&resp, false, true));
        crate::widgets::kit::paint_frame(&p, br, theme::RADIUS_CONTROL, &look);
        icons::paint(&p, icon, br.center(), theme::LAYER_ICON_SIZE, look.icon);
        mark_focus(ui, &p, resp.id, br, theme::RADIUS_CONTROL as f32);
        if resp.on_hover_text(tip).clicked() {
            apply_contextual_action(app, key);
        }
        cx += step;
    }
    cx += 3.0;
    widgets::vline(&p, cx, ctx_bar.top() + 6.0, ctx_bar.bottom() - 6.0, theme::BORDER);
    cx += 5.0;
    let more_r = Rect::from_min_size(Pos2::new(cx, cy), Vec2::new(more_w, theme::CONTROL_XS));
    let more_resp = ui.interact(more_r, ui.id().with("ctx-more"), Sense::click());
    let more_look = crate::widgets::kit::button_look(crate::widgets::kit::Hierarchy::Tertiary, crate::widgets::kit::state_of(&more_resp, false, true));
    crate::widgets::kit::paint_frame(&p, more_r, theme::RADIUS_CONTROL, &more_look);
    p.text(Pos2::new(more_r.left() + theme::INSET_CONTROL, more_r.center().y), egui::Align2::LEFT_CENTER, "More", more_font, more_look.text);
    let chevron_at = Pos2::new(more_r.right() - theme::INSET_CONTROL - theme::LAYER_ICON_SIZE * 0.5, more_r.center().y);
    icons::paint(&p, Icon::ChevronDown, chevron_at, theme::LAYER_ICON_SIZE, more_look.icon);
    mark_focus(ui, &p, more_resp.id, more_r, theme::RADIUS_CONTROL as f32);
    if more_resp.on_hover_text("More actions   Ctrl+K").clicked() {
        app.cmdk.open = true;
        app.cmdk.focus_requested = true;
    }
}

pub fn show(app: &mut App, ui: &mut Ui, screen: Rect) {
    if app.ui.dev_mode {
        show_dev_mode_toolbar(app, ui, screen);
        return;
    }
    let bar = toolbar_rect(screen);
    let p = ui.painter().clone();
    contextual_bar(app, ui, screen, bar);

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
        let look = tool_look(&resp, selected);
        crate::widgets::kit::paint_frame(&p, br, theme::RADIUS_CONTROL, &look);
        icons::paint(&p, tool_icon(shown), br.center(), theme::TOOL_ICON_SIZE, look.icon);
        mark_focus(ui, &p, resp.id, br, theme::RADIUS_CONTROL as f32);
        if resp.clicked() {
            app.set_tool(tool);
        }
        let tip = if tool.shortcut().is_empty() { tool.label().to_string() } else { format!("{}   {}", tool.label(), tool.shortcut()) };
        resp.on_hover_text(tip);
        x += theme::TOOL_BUTTON;
        if has_chev {
            x += theme::TOOL_CHEVRON_GAP;
            let cr = Rect::from_min_size(Pos2::new(x, y), Vec2::new(theme::TOOL_CHEVRON_W, theme::TOOL_BUTTON));
            let hit = Rect::from_min_size(cr.min, Vec2::new(theme::TOOL_CHEVRON_W + theme::TOOL_GROUP_GAP, theme::TOOL_BUTTON));
            let cresp = ui.interact(hit, ui.id().with(("tool-chev", gi)), Sense::click());
            widgets::record_with(ui, || format!("tool-chevron-{gi}"), cr);
            widgets::record_with(ui, || format!("tool-chevron-hit-{gi}"), hit);
            #[cfg(test)]
            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(("test-chevron-id", gi)), cresp.id));
            let open = app.ui.menu == Some(MenuKind::ToolGroup(gi));
            // The painted chevron stays 16px. The press target is the wider hit. An open menu uses the hover fill.
            let chev_state = if cresp.is_pointer_button_down_on() {
                crate::widgets::kit::State::Pressed
            } else if open || cresp.hovered() {
                crate::widgets::kit::State::Hover
            } else {
                crate::widgets::kit::State::Default
            };
            let chev_look = crate::widgets::kit::button_look(crate::widgets::kit::Hierarchy::Tool, chev_state);
            crate::widgets::kit::paint_frame(&p, cr, theme::RADIUS_CONTROL, &chev_look);
            mark_focus(ui, &p, cresp.id, cr, theme::RADIUS_CONTROL as f32);
            let cc = Pos2::new(br.right() + theme::TOOL_CHEVRON_CENTER_DX, cr.center().y);
            icons::paint(&p, Icon::ChevronDown, cc, theme::TOOL_CHEVRON_ICON, chev_look.icon);
            // The menu opens on the press, so the same press can slide onto a row and pick it on release; a second
            // press on the chevron closes it. Space or Enter on the focused chevron and a screen reader's activation
            // reach `clicked` without a pointer click and toggle it too.
            if primary_pressed_in(ui, hit) || (cresp.clicked() && !cresp.clicked_by(egui::PointerButton::Primary)) {
                app.ui.menu = if open { None } else { Some(MenuKind::ToolGroup(gi)) };
            }
            if app.ui.menu == Some(MenuKind::ToolGroup(gi)) {
                app.ui.menu_anchor = Some(cr);
            }
            x += theme::TOOL_CHEVRON_W;
        }
    }

    x += theme::TOOL_GROUP_GAP;
    let ar = Rect::from_min_size(Pos2::new(x, y), Vec2::splat(theme::TOOL_BUTTON));
    widgets::record(ui, "toolbar-actions-btn", ar);
    let aresp = ui.interact(ar, ui.id().with("actions"), Sense::click());
    let actions_open = app.cmdk.open || app.ui.menu == Some(MenuKind::Palette);
    let actions_look = tool_look(&aresp, actions_open);
    crate::widgets::kit::paint_frame(&p, ar, theme::RADIUS_CONTROL, &actions_look);
    icons::paint(&p, Icon::Sparkle, ar.center(), theme::TOOL_ICON_SIZE, actions_look.icon);
    mark_focus(ui, &p, aresp.id, ar, theme::RADIUS_CONTROL as f32);
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
        let seg = if sel && i == 2 {
            crate::widgets::kit::Look {
                fill: theme::DEV_READY,
                border: Color32::TRANSPARENT,
                border_w: theme::BORDER_WIDTH,
                text: theme::TEXT_ON_BRAND,
                icon: theme::TEXT_ON_BRAND,
                mark: Color32::TRANSPARENT,
            }
        } else if sel {
            let mut seg = crate::widgets::kit::segmented_look(crate::widgets::kit::State::Selected);
            seg.border = Color32::TRANSPARENT;
            seg
        } else if resp.hovered() {
            crate::widgets::kit::segmented_look(crate::widgets::kit::State::Hover)
        } else {
            crate::widgets::kit::segmented_look(crate::widgets::kit::State::Default)
        };
        crate::widgets::kit::paint_frame(&p, r, theme::RADIUS_SEGMENT, &seg);
        mark_focus(ui, &p, resp.id, r, theme::RADIUS_SEGMENT as f32);
        icons::paint(&p, icon, r.center(), theme::LAYER_ICON_SIZE, seg.icon);
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
                _ => app.set_dev_mode(true),
            }
        }
    }
}

/// 1:1 Figma UI3 Dev Mode 5-tool bottom floating dock + floating `↩ Re-center  ⇧2` pill (`Gap C`).
fn show_dev_mode_toolbar(app: &mut App, ui: &mut Ui, screen: Rect) {
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

    // Floating re-center control above the Dev Mode bottom toolbar.
    let label_font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold);
    let hint_font = theme::font_body();
    let label = "\u{21A9} Re-center";
    let hint = "\u{21E7}2";
    let measured = 10.0 + widgets::text_width(&p, label, &label_font) + theme::SPACE_FACT + widgets::text_width(&p, hint, &hint_font) + 10.0;
    let recenter_w = if measured > 118.0 { measured.ceil() } else { 118.0 };
    let recenter_h = theme::TOOL_BUTTON;
    let recenter_rect = Rect::from_min_size(
        Pos2::new((bar.center().x - recenter_w * 0.5).round(), bar.top() - theme::SPACE_INLINE - recenter_h),
        Vec2::new(recenter_w, recenter_h),
    );
    widgets::record(ui, "dev-recenter", recenter_rect);
    let recenter_resp = ui.interact(recenter_rect, ui.id().with("dev-recenter"), Sense::click());
    let recenter_look = crate::widgets::kit::button_look(crate::widgets::kit::Hierarchy::Secondary, crate::widgets::kit::state_of(&recenter_resp, false, true));
    crate::widgets::kit::paint_frame(&p, recenter_rect, theme::RADIUS_CONTROL, &recenter_look);
    p.text(Pos2::new(recenter_rect.left() + 10.0, recenter_rect.center().y), egui::Align2::LEFT_CENTER, label, label_font, recenter_look.text);
    p.text(Pos2::new(recenter_rect.right() - 10.0, recenter_rect.center().y), egui::Align2::RIGHT_CENTER, hint, hint_font, theme::TEXT_QUATERNARY);
    mark_focus(ui, &p, recenter_resp.id, recenter_rect, theme::RADIUS_CONTROL as f32);
    if recenter_resp.on_hover_text("Re-center canvas   Shift+2").clicked() {
        if app.selection.is_empty() {
            app.fit_all();
        } else {
            app.fit_selection();
        }
    }

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
        let look = tool_look(&resp, active);
        crate::widgets::kit::paint_frame(&p, br, theme::RADIUS_CONTROL, &look);
        icons::paint(&p, icon, br.center(), theme::TOOL_ICON_SIZE, look.icon);
        mark_focus(ui, &p, resp.id, br, theme::RADIUS_CONTROL as f32);
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
    let toggle_fill = if toggle_resp.hovered() { theme::DEV_READY_HOVER } else { theme::DEV_READY };
    p.rect_filled(toggle_r, theme::cr(theme::RADIUS_CONTROL), toggle_fill);
    icons::paint(&p, Icon::Dev, toggle_r.center(), theme::LAYER_ICON_SIZE, theme::TEXT_ON_BRAND);
    mark_focus(ui, &p, toggle_resp.id, toggle_r, theme::RADIUS_CONTROL as f32);
    if toggle_resp.on_hover_text("Exit Dev Mode   Shift+D").clicked() {
        app.set_dev_mode(false);
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
        let agents_hdr = get_rect("agents-new-chat-btn").or_else(|| get_rect("agents-header-back")).expect("agents header rect");
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

    /// Runs two frames (an area is hit-tested from its second frame on); the second one carries a primary press at
    /// `press`, with a floating panel over `panel` unless it is empty. Returns whether the toolbar saw the press in `rect`.
    fn press_seen(rect: Rect, press: Pos2, panel: Rect) -> bool {
        let ctx = egui::Context::default();
        let mut seen = false;
        for frame in 0..2 {
            let events = if frame == 1 {
                vec![egui::Event::PointerButton { pos: press, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE }]
            } else {
                vec![]
            };
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))), events, ..Default::default() };
            let _ = ctx.run_ui(raw, |ui| {
                if panel.is_positive() {
                    egui::Area::new(egui::Id::new("floating-panel")).order(egui::Order::Middle).fixed_pos(panel.min).show(ui.ctx(), |ui| {
                        ui.allocate_exact_size(panel.size(), Sense::click());
                    });
                }
                seen = primary_pressed_in(ui, rect);
            });
        }
        seen
    }

    #[test]
    fn a_press_on_a_floating_panel_over_the_chevron_is_not_a_chevron_press() {
        let chevron = Rect::from_min_size(Pos2::new(400.0, 600.0), Vec2::new(16.0, 32.0));
        let on_chevron = chevron.center();
        let panel = Rect::from_min_size(Pos2::new(380.0, 560.0), Vec2::new(120.0, 100.0));
        assert!(press_seen(chevron, on_chevron, Rect::NOTHING), "nothing over the chevron: the press is its");
        assert!(!press_seen(chevron, on_chevron, panel), "a panel over the chevron takes the press");
        assert!(!press_seen(chevron, Pos2::new(300.0, 620.0), Rect::NOTHING), "a press beside the chevron is not its");
    }

    const TOOLBAR_SCREEN: Vec2 = Vec2::new(1440.0, 900.0);

    fn paint_toolbar(app: &mut App) -> (egui::Context, Vec<egui::epaint::ClippedShape>) {
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let screen = Rect::from_min_size(Pos2::ZERO, TOOLBAR_SCREEN);
        let raw = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        let shapes = ctx.run_ui(raw, |ui| show(app, ui, screen)).shapes;
        (ctx, shapes)
    }

    #[test]
    fn chevron_hit_box_meets_the_target_floor() {
        let mut app = App::new(designos_doc::Document::new("Chevron"));
        let (ctx, _) = paint_toolbar(&mut app);
        let named = |name: &str| ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", name.to_string()))));
        let screen = Rect::from_min_size(Pos2::ZERO, TOOLBAR_SCREEN);
        assert_eq!(toolbar_width(), 512.0);
        let mut checked = 0;
        for (gi, group) in GROUPS.iter().enumerate() {
            if !has_chevron(group) {
                continue;
            }
            let hit = named(&format!("tool-chevron-hit-{gi}")).expect("chevron hit");
            let visual = named(&format!("tool-chevron-{gi}")).expect("chevron visual");
            assert!(hit.width() + 0.5 >= theme::TARGET_MIN, "chevron {gi} hit is at least 24 wide");
            assert!((visual.width() - theme::TOOL_CHEVRON_W).abs() < 0.5, "the painted chevron {gi} stays 16 wide");
            assert_eq!(visual, chevron_rect(screen, gi), "the layout helper and the painted chevron {gi} agree");
            // the tool buttons are not recorded, so the next control is found from the layout the helper mirrors
            let next = if gi + 1 < GROUPS.len() { group_button_rect(screen, gi + 1) } else { named("toolbar-actions-btn").expect("actions button") };
            assert!(hit.right() <= next.left() + 0.5, "the hit of chevron {gi} ends at {} but the next control starts at {}", hit.right(), next.left());
            checked += 1;
        }
        assert!(checked >= 2, "the toolbar has chevron groups to check, found {checked}");
    }

    #[test]
    fn dev_toggle_label_is_dark_on_the_dev_fill() {
        let mut app = App::new(designos_doc::Document::new("DevToggle"));
        app.ui.dev_mode = true;
        let (ctx, shapes) = paint_toolbar(&mut app);
        let toggle = ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", "dev-mode-toggle".to_string())))).expect("dev toggle");
        fn walk_fill(shape: &egui::Shape, toggle: Rect, fills: &mut Vec<egui::Color32>, glyphs: &mut Vec<egui::Color32>) {
            match shape {
                egui::Shape::Rect(r) if r.rect == toggle => fills.push(r.fill),
                egui::Shape::Mesh(mesh) => {
                    if mesh.vertices.iter().any(|v| toggle.contains(v.pos)) {
                        glyphs.extend(mesh.vertices.iter().map(|v| v.color));
                    }
                }
                egui::Shape::Vec(inner) => inner.iter().for_each(|s| walk_fill(s, toggle, fills, glyphs)),
                _ => {}
            }
        }
        let (mut fills, mut glyphs) = (Vec::new(), Vec::new());
        for clipped in &shapes {
            walk_fill(&clipped.shape, toggle, &mut fills, &mut glyphs);
        }
        assert!(fills.contains(&theme::DEV_READY), "dev toggle fill is the ready role, got {fills:?}");
        assert!(glyphs.contains(&theme::TEXT_ON_BRAND), "dev glyph is the dark label, got {glyphs:?}");
    }
}
