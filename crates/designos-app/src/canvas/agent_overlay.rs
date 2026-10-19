//! Paints each active agent session's cursor, label pill, and write-flash outlines on the canvas (`#682`).
//!
//! Headless renders and exports (`designos-render`, `--screenshot`) never run this overlay.
//! Idle invariant: requests a repaint only while a cursor or flash is actively moving, plus a
//! single one-shot `request_repaint_after` for the start of the 5 s fade-out. Never calls
//! `App::changed()` or bumps `TileView::rev`.

use crate::agent_presence::{flash_envelope, AgentCursorSize, CURSOR_FADE_IN_S, CURSOR_FADE_OUT_S, CURSOR_IDLE_HOLD_S};
use crate::app::App;
use crate::theme;
use crate::widgets;
use designos_doc::Vec2 as DV;
use egui::{Color32, CornerRadius, Pos2, Rect as ERect, Shape, Stroke, StrokeKind, Ui, Vec2};
use std::time::Duration;

fn with_alpha(c: Color32, alpha: f32) -> Color32 {
    let a = alpha.clamp(0.0, 1.0);
    let [r, g, b, base_a] = c.to_srgba_unmultiplied();
    let final_a = ((base_a as f32) * a).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_unmultiplied(r, g, b, final_a)
}

/// Paints write-flash outlines and agent cursors above the canvas content and below comment pins.
pub fn paint(app: &mut App, ui: &Ui, canvas_rect: ERect) {
    // Rule 8: headless `--screenshot` captures never draw cursors or flashes.
    if app.canvas.tiles.unlimited_budget {
        return;
    }
    // Rule 3: Hidden draws nothing and requests no frame.
    let size_mode = app.agent_presence.cursor_size;
    if size_mode == AgentCursorSize::Hidden || app.agent_presence.sessions.is_empty() {
        return;
    }

    let now_s = app.now;
    let dt = ui.input(|i| i.stable_dt).clamp(0.0, 0.25);
    app.agent_presence.step(now_s, dt);
    if app.agent_presence.sessions.is_empty() {
        return;
    }

    let painter = ui.painter_at(canvas_rect);
    let origin = canvas_rect.min;
    let view = app.view;
    let clamped_bounds = canvas_rect.shrink(theme::INSET_FRAME);

    let mut needs_immediate_repaint = false;
    let mut earliest_fade_wakeup_s: Option<f64> = None;
    let mut flash_record_idx = 0usize;

    // 1. Paint write flashes below the cursors (Rule 7: "drawn above the canvas, below the cursor").
    for session in &app.agent_presence.sessions {
        let base_color = session.color();
        for flash in &session.flashes {
            let elapsed = now_s - flash.start_s;
            if elapsed < 0.0 {
                // Waiting for the 300 ms cursor-travel delay.
                needs_immediate_repaint = true;
                continue;
            }
            let alpha = flash_envelope(elapsed);
            if alpha <= 0.0 {
                continue;
            }
            needs_immediate_repaint = true;
            if app.doc.get(flash.node_id).is_none() {
                continue;
            }
            let bb = super::fast_world_aabb(&app.doc, None, flash.node_id);
            let min_screen = view.to_screen(origin, DV::new(bb.x, bb.y));
            let max_screen = view.to_screen(origin, DV::new(bb.x + bb.w, bb.y + bb.h));
            let flash_rect = ERect::from_min_max(min_screen, max_screen);
            let stroke_color = with_alpha(base_color, alpha);
            painter.rect_stroke(flash_rect, CornerRadius::ZERO, Stroke::new(flash.stroke_px, stroke_color), StrokeKind::Outside);
            widgets::record(ui, &format!("agent-flash-{flash_record_idx}"), flash_rect);
            flash_record_idx += 1;
        }
    }

    // 2. Paint each session's cursor arrow and label pill.
    let arrow_scale = size_mode.pixels() / AgentCursorSize::Small.pixels();
    for (idx, session) in app.agent_presence.sessions.iter_mut().enumerate() {
        let since_first = (now_s - session.first_write_s).max(0.0);
        let since_last = (now_s - session.last_write_s).max(0.0);
        let travelling = (session.target_page - session.cursor_page).length() > 0.5;
        let fading_in = since_first < CURSOR_FADE_IN_S;
        let fading_out = (CURSOR_IDLE_HOLD_S..CURSOR_IDLE_HOLD_S + CURSOR_FADE_OUT_S).contains(&since_last);

        if travelling || fading_in || fading_out {
            needs_immediate_repaint = true;
        } else if since_last < CURSOR_IDLE_HOLD_S && session.fade_wakeup_scheduled_for != Some(session.last_write_s) {
            session.fade_wakeup_scheduled_for = Some(session.last_write_s);
            let wait_s = (CURSOR_IDLE_HOLD_S - since_last).max(0.001);
            earliest_fade_wakeup_s = Some(match earliest_fade_wakeup_s {
                Some(prev) => prev.min(wait_s),
                None => wait_s,
            });
        }

        let fade = session.fade;
        if fade <= 0.0 {
            continue;
        }

        let raw_screen = view.to_screen(origin, DV::new(session.cursor_page.x, session.cursor_page.y));
        let tip = Pos2::new(raw_screen.x.clamp(clamped_bounds.min.x, clamped_bounds.max.x), raw_screen.y.clamp(clamped_bounds.min.y, clamped_bounds.max.y));

        let fill_color = with_alpha(session.color(), fade);
        let text_color = with_alpha(theme::AGENT_LABEL_TEXT, fade);

        // Arrow glyph scaled by `arrow_scale` (`Small` = 1.0, `Medium` = 40/24, `Large` = 56/24).
        let arm_long = theme::INSET_POPOVER * arrow_scale;
        let arm_short = theme::INSET_FRAME * arrow_scale;
        let pts =
            vec![tip, tip + Vec2::new(0.0, arm_long), tip + Vec2::new(arm_short * 0.55, arm_short * 1.15), tip + Vec2::new(arm_short * 1.25, arm_short * 1.15)];
        painter.add(Shape::convex_polygon(pts, fill_color, Stroke::new(theme::CANVAS_OUTLINE_WIDTH, with_alpha(theme::BG_PRIMARY, fade))));

        // Label pill under-right of the cursor arrow (`CONTROL_DENSE` 24 px high, `radius_full`, dark label).
        let font = theme::font_caption_medium();
        let max_label_w = theme::PALETTE_TOP * 2.0;
        let elided = widgets::elide(&painter, &session.label, &font, max_label_w);
        let galley = painter.layout_no_wrap(elided, font, text_color);
        let pill_h = theme::CONTROL_DENSE;
        let pill_w = (galley.size().x + theme::INSET_FRAME * 2.0).max(pill_h);
        let pill_offset = Vec2::new(theme::INSET_FRAME * arrow_scale, theme::INSET_BUTTON * arrow_scale);
        let mut pill_min = tip + pill_offset;
        if pill_min.x + pill_w > clamped_bounds.max.x {
            pill_min.x = (clamped_bounds.max.x - pill_w).max(clamped_bounds.min.x);
        }
        if pill_min.y + pill_h > clamped_bounds.max.y {
            pill_min.y = (clamped_bounds.max.y - pill_h).max(clamped_bounds.min.y);
        }
        let pill_rect = ERect::from_min_size(pill_min, Vec2::new(pill_w, pill_h));
        painter.rect_filled(pill_rect, theme::cr_pill(pill_h), fill_color);
        painter.galley(pill_rect.center() - galley.size() * 0.5, galley, text_color);

        let union_rect = pill_rect.union(ERect::from_min_size(tip, Vec2::splat(arm_long)));
        widgets::record(ui, &format!("agent-cursor-{idx}"), union_rect);
        widgets::record(ui, &format!("agent-cursor-{}", session.id), union_rect);
    }

    if needs_immediate_repaint {
        ui.ctx().request_repaint();
    } else if let Some(wait_s) = earliest_fade_wakeup_s {
        ui.ctx().request_repaint_after(Duration::from_secs_f64(wait_s));
    }
}
