//! "Edit variable" popover: name, description, scoping (Figma's scope names mapped onto
//! `Variable::scopes`), "Hide from publishing" and the value in every mode.

use super::view::{value_cell, Frame, Geom};
use crate::app::App;
use crate::icons::{self, Icon};
use crate::theme;
use crate::widgets::{self, field};
use designos_doc::*;
use egui::{Align2, Context, Id, Key, Order, Pos2, Rect, Sense, Stroke, Ui, Vec2};

/// (group, label, Figma scope id) for the properties a variable of `kind` can be scoped to.
pub fn scope_options(kind: VariableKind) -> &'static [(&'static str, &'static str, &'static str)] {
    match kind {
        VariableKind::Color => &[
            ("Fill", "All fills", "ALL_FILLS"),
            ("Fill", "Frame", "FRAME_FILL"),
            ("Fill", "Shape", "SHAPE_FILL"),
            ("Fill", "Text", "TEXT_FILL"),
            ("Stroke", "Stroke", "STROKE_COLOR"),
            ("Effects", "Effects", "EFFECT_COLOR"),
        ],
        VariableKind::Number => &[
            ("Layout", "Width and height", "WIDTH_HEIGHT"),
            ("Layout", "Gap", "GAP"),
            ("Layout", "Corner radius", "CORNER_RADIUS"),
            ("Appearance", "Opacity", "OPACITY"),
            ("Stroke", "Stroke", "STROKE_FLOAT"),
            ("Effects", "Effects", "EFFECT_FLOAT"),
            ("Typography", "Font size", "FONT_SIZE"),
            ("Typography", "Font weight", "FONT_WEIGHT"),
            ("Typography", "Line height", "LINE_HEIGHT"),
            ("Typography", "Letter spacing", "LETTER_SPACING"),
        ],
        VariableKind::String => {
            &[("Text", "Text content", "TEXT_CONTENT"), ("Typography", "Font family", "FONT_FAMILY"), ("Typography", "Font style", "FONT_STYLE")]
        }
        VariableKind::Boolean => &[],
    }
}

/// Scope list after clicking the "Show in all supported properties" checkbox.
pub fn toggle_all(kind: VariableKind, scopes: &[String]) -> Vec<String> {
    if scope_all(scopes) {
        scope_options(kind).iter().map(|o| o.2.to_string()).collect()
    } else {
        vec![]
    }
}

/// Scope list after clicking one property's checkbox.
pub fn toggle_scope(kind: VariableKind, scopes: &[String], id: &str) -> Vec<String> {
    let all: Vec<String> = scope_options(kind).iter().map(|o| o.2.to_string()).collect();
    let mut cur = if scope_all(scopes) { all.clone() } else { scopes.to_vec() };
    if let Some(i) = cur.iter().position(|s| s == id) {
        if cur.len() > 1 {
            cur.remove(i);
        }
    } else {
        cur.push(id.to_string());
    }
    if all.iter().all(|a| cur.contains(a)) {
        vec![]
    } else {
        cur
    }
}

fn scope_all(scopes: &[String]) -> bool {
    scopes.is_empty() || scopes.iter().any(|s| s == "ALL_SCOPES")
}

fn checkbox(ui: &mut Ui, row: Rect, key: &str, label: &str, checked: bool) -> bool {
    let r = ui.interact(row, Id::new(("var-pop-check", key)), Sense::click());
    let p = ui.painter();
    if r.hovered() {
        p.rect_filled(row, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
    }
    let b = Rect::from_center_size(Pos2::new(row.left() + 16.0, row.center().y), Vec2::splat(14.0));
    if checked {
        p.rect_filled(b, theme::cr(3), theme::BG_BRAND);
        icons::paint(p, Icon::Check, b.center(), 12.0, theme::TEXT);
    } else {
        p.rect_stroke(b, theme::cr(3), Stroke::new(1.0, theme::ICON_TERTIARY), egui::StrokeKind::Inside);
    }
    p.text(Pos2::new(row.left() + 32.0, row.center().y), Align2::LEFT_CENTER, label, theme::font_body(), theme::TEXT);
    widgets::record(ui, &format!("var-pop:{key}"), row);
    r.clicked()
}

pub fn show(app: &mut App, ctx: &Context, g: &Geom, fr: &mut Frame) {
    let Some(mut ep) = app.vars.edit.take() else { return };
    let Some(var) = fr.work.variables.get(&ep.var).cloned() else { return };
    let Some(coll) = fr.work.collections.get(&var.collection).cloned() else { return };
    let screen = ctx.content_rect();
    let w = theme::VAR_POPOVER_W;
    let options = scope_options(var.kind);
    // content height
    let mut h = 40.0 + 8.0 + 2.0 * 52.0 + 8.0;
    if !options.is_empty() {
        h += 24.0 + 24.0 + options.len() as f32 * 24.0 + 8.0;
    }
    h += 24.0 + 16.0 + coll.modes.len() as f32 * 40.0 + 8.0;
    let max_h = (screen.height() - 16.0).min(560.0);
    let ph = h.min(max_h);
    let want = Pos2::new(ep.anchor.left() + 8.0, ep.anchor.top() - 8.0);
    let pos = Pos2::new(want.x.min(screen.right() - w - 8.0).max(screen.left() + 8.0), want.y.min(screen.bottom() - ph - 8.0).max(screen.top() + 8.0));
    let rect = Rect::from_min_size(pos, Vec2::new(w, ph));
    let max_scroll = (h - ph).max(0.0);
    ep.scroll = ep.scroll.clamp(0.0, max_scroll);
    let mut close = false;
    let nested = app.vars.cell_picker.is_some() || app.vars.alias_picker.is_some();

    egui::Area::new(Id::new("vars-edit-popover")).order(Order::Foreground).fixed_pos(pos).constrain(false).show(ctx, |ui| {
        let (full, _) = ui.allocate_exact_size(rect.size(), Sense::click_and_drag());
        let p = ui.painter().clone();
        widgets::menu_panel(&p, full, theme::RADIUS_FRAME);
        // header
        p.text(Pos2::new(full.left() + 16.0, full.top() + 20.0), Align2::LEFT_CENTER, "Edit variable", theme::font_body_strong(), theme::TEXT);
        let xr = Rect::from_center_size(Pos2::new(full.right() - 20.0, full.top() + 20.0), Vec2::splat(24.0));
        if widgets::icon_button(ui, xr, "var-pop-close", Icon::Minus, false, "").clicked() {
            close = true;
        }
        super::widgets::cross_glyph(&p, xr.center(), 4.0, theme::ICON_SECONDARY);
        widgets::hline(&p, full.left(), full.right(), full.top() + 39.0, theme::BORDER_MENU);
        if ui.rect_contains_pointer(full) {
            ep.scroll = (ep.scroll - ui.input(|i| i.smooth_scroll_delta.y)).clamp(0.0, max_scroll);
        }
        let body = Rect::from_min_max(full.min + Vec2::new(0.0, 40.0), full.max);
        crate::panels::clipped(ui, body, |ui| {
            let p = ui.painter().clone();
            let x = full.left() + 16.0;
            let fw = w - 32.0;
            let mut y = body.top() + 8.0 - ep.scroll;
            // name
            p.text(Pos2::new(x, y + 12.0), Align2::LEFT_CENTER, "Name", theme::font_body_strong(), theme::TEXT_SECONDARY);
            y += 24.0;
            let nr = Rect::from_min_size(Pos2::new(x, y), Vec2::new(fw, 24.0));
            widgets::record(ui, "var-pop:name", nr);
            if let Some(s) = field::text_input(ui, nr, ("var-pop-name", &var.id), &var.name) {
                fr.work.rename_variable(&var.id, &s);
            }
            y += 28.0;
            p.text(Pos2::new(x, y + 12.0), Align2::LEFT_CENTER, "Description", theme::font_body_strong(), theme::TEXT_SECONDARY);
            y += 24.0;
            let dr = Rect::from_min_size(Pos2::new(x, y), Vec2::new(fw, 24.0));
            widgets::record(ui, "var-pop:description", dr);
            if let Some(s) = field::text_input(ui, dr, ("var-pop-desc", &var.id), &var.description) {
                if let Some(v) = fr.work.variables.get_mut(&var.id) {
                    v.description = s;
                }
            }
            y += 36.0;
            // scoping
            if !options.is_empty() {
                widgets::hline(&p, full.left(), full.right(), y - 4.0, theme::BORDER_MENU);
                p.text(Pos2::new(x, y + 16.0), Align2::LEFT_CENTER, "Scoping", theme::font_body_strong(), theme::TEXT_SECONDARY);
                y += 32.0;
                let all = scope_all(&var.scopes);
                let row = Rect::from_min_size(Pos2::new(full.left() + 8.0, y), Vec2::new(w - 16.0, 24.0));
                if checkbox(ui, row, "all", "Show in all supported properties", all) {
                    let next = toggle_all(var.kind, &var.scopes);
                    if let Some(v) = fr.work.variables.get_mut(&var.id) {
                        v.scopes = next;
                    }
                }
                y += 24.0;
                for (_, label, id) in options {
                    let on = all || var.scopes.iter().any(|s| s == id);
                    let row = Rect::from_min_size(Pos2::new(full.left() + 8.0, y), Vec2::new(w - 16.0, 24.0));
                    let r = Rect::from_min_max(row.min + Vec2::new(16.0, 0.0), row.max);
                    if checkbox(ui, r, id, label, on) {
                        let next = toggle_scope(var.kind, &var.scopes, id);
                        if let Some(v) = fr.work.variables.get_mut(&var.id) {
                            v.scopes = next;
                        }
                    }
                    y += 24.0;
                }
                y += 8.0;
            }
            widgets::hline(&p, full.left(), full.right(), y - 4.0, theme::BORDER_MENU);
            let row = Rect::from_min_size(Pos2::new(full.left() + 8.0, y + 4.0), Vec2::new(w - 16.0, 24.0));
            if checkbox(ui, row, "hide", "Hide from publishing", var.hidden_from_publishing) {
                if let Some(v) = fr.work.variables.get_mut(&var.id) {
                    v.hidden_from_publishing = !v.hidden_from_publishing;
                }
            }
            y += 36.0;
            // values
            widgets::hline(&p, full.left(), full.right(), y - 4.0, theme::BORDER_MENU);
            p.text(Pos2::new(x, y + 12.0), Align2::LEFT_CENTER, "Value", theme::font_body_strong(), theme::TEXT_SECONDARY);
            y += 28.0;
            for mode in &coll.modes {
                let label = if coll.modes.len() == 1 { String::new() } else { mode.name.clone() };
                if !label.is_empty() {
                    p.text(Pos2::new(x, y + 8.0), Align2::LEFT_CENTER, &label, theme::font_body(), theme::TEXT_SECONDARY);
                }
                let cell = Rect::from_min_size(Pos2::new(full.left() + 8.0, y + if label.is_empty() { 0.0 } else { 8.0 }), Vec2::new(w - 24.0, 32.0));
                if let Some(cur) = fr.work.variables.get(&var.id).cloned() {
                    value_cell(app, ui, cell, &cur, &mode.id, false, false, fr);
                }
                y += 40.0;
            }
        });
    });

    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) && !nested {
        close = true;
    }
    let (pressed, ptr) = ctx.input(|i| (i.pointer.any_pressed(), i.pointer.interact_pos()));
    if pressed && !nested && ptr.is_some_and(|p| !rect.contains(p)) {
        close = true;
    }
    let _ = g;
    if !close {
        app.vars.edit = Some(ep);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_toggles_keep_a_valid_list() {
        let k = VariableKind::Color;
        // all supported -> uncheck "all" lists every property
        let s = toggle_all(k, &[]);
        assert_eq!(s.len(), scope_options(k).len());
        // unchecking one property from "all"
        let s2 = toggle_scope(k, &[], "STROKE_COLOR");
        assert!(!s2.contains(&"STROKE_COLOR".to_string()) && s2.len() == scope_options(k).len() - 1);
        // re-checking everything collapses back to "all"
        assert!(toggle_scope(k, &s2, "STROKE_COLOR").is_empty());
        // the last property cannot be unchecked
        assert_eq!(toggle_scope(k, &["ALL_FILLS".to_string()], "ALL_FILLS"), vec!["ALL_FILLS".to_string()]);
        assert!(toggle_all(k, &s).is_empty());
    }
}
