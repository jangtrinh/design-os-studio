//! "Edit variable" popover: name, description, scoping (Figma's scope names mapped onto
//! `Variable::scopes`), "Hide from publishing" and the value in every mode.

use super::view::{value_cell, Frame, Geom};
use crate::app::App;
use crate::icons::Icon;
use crate::theme;
use crate::widgets::{self, field};
use designos_doc::*;
use egui::{Align2, Context, Id, Key, Order, Pos2, Rect, Sense, Ui, Vec2};

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
    if has_scope(&cur, id) {
        let rest: Vec<String> = cur.iter().filter(|s| !same_scope(s, id)).cloned().collect();
        if !rest.is_empty() {
            cur = rest;
        }
    } else {
        cur.push(id.to_string());
    }
    if all.iter().all(|a| has_scope(&cur, a)) {
        vec![]
    } else {
        cur
    }
}

fn scope_all(scopes: &[String]) -> bool {
    scopes.is_empty() || scopes.iter().any(|s| s == "ALL_SCOPES")
}

/// Whether `scope` is the scope the row `id` stands for. The stroke colour scope is `STROKE` in
/// the `.fig` file and `STROKE_COLOR` in REST, so the Stroke row stands for both spellings.
fn same_scope(scope: &str, id: &str) -> bool {
    scope == id || (id == "STROKE_COLOR" && scope == "STROKE")
}

fn has_scope(scopes: &[String], id: &str) -> bool {
    scopes.iter().any(|s| same_scope(s, id))
}

/// Whether the checkbox for scope `id` is on for a variable with these `scopes`.
pub fn scope_on(scopes: &[String], id: &str) -> bool {
    scope_all(scopes) || has_scope(scopes, id)
}

/// A scope row: the kit checkbox is the focusable control, and the row takes clicks on its label.
fn checkbox(ui: &mut Ui, row: Rect, key: &str, label: &str, checked: bool) -> bool {
    let row_hit = ui.interact(row, Id::new(("var-pop-check", key)), Sense::CLICK);
    let p = ui.painter().clone();
    if ui.rect_contains_pointer(row) {
        p.rect_filled(row, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
    }
    let box_left = row.left() + 16.0 - theme::LAYER_ICON_SIZE / 2.0;
    let hit = Rect::from_min_max(Pos2::new(box_left, row.top()), row.max);
    let box_hit = crate::widgets::kit::checkbox(ui, hit, ("var-pop-box", key), checked, true);
    p.text(Pos2::new(row.left() + 32.0, row.center().y), Align2::LEFT_CENTER, label, theme::font_body(), theme::TEXT);
    widgets::record(ui, &format!("var-pop:{key}"), row);
    row_hit.clicked() || box_hit.clicked()
}

pub fn show(app: &mut App, ctx: &Context, g: &Geom, fr: &mut Frame) {
    let Some(mut ep) = app.vars.edit.take() else { return };
    let Some(var) = fr.work.variables.get(&ep.var).cloned() else { return };
    let Some(coll) = fr.work.collections.get(&var.collection).cloned() else { return };
    let screen = ctx.content_rect();
    let w = theme::VAR_POPOVER_W;
    let options = scope_options(var.kind);
    // content height
    let row_h = theme::MENU_ITEM_HEIGHT;
    let mut h = 40.0 + 8.0 + 2.0 * 52.0 + 8.0;
    if !options.is_empty() {
        h += 32.0 + row_h + options.len() as f32 * row_h + 8.0;
    }
    h += row_h + 16.0 + coll.modes.len() as f32 * 40.0 + 8.0;
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
        crate::widgets::kit::popover_surface(&p, full);
        // header
        p.text(Pos2::new(full.left() + 16.0, full.top() + 20.0), Align2::LEFT_CENTER, "Edit variable", theme::font_body_strong(), theme::TEXT);
        let xr = Rect::from_center_size(Pos2::new(full.right() - 20.0, full.top() + 20.0), Vec2::splat(theme::CONTROL_XS));
        widgets::record(ui, "var-pop-close", xr);
        if widgets::icon_button(ui, xr, "var-pop-close", Icon::Close, false, "Close").clicked() {
            close = true;
        }
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
                let row = Rect::from_min_size(Pos2::new(full.left() + 8.0, y), Vec2::new(w - 16.0, row_h));
                if checkbox(ui, row, "all", "Show in all supported properties", all) {
                    let next = toggle_all(var.kind, &var.scopes);
                    if let Some(v) = fr.work.variables.get_mut(&var.id) {
                        v.scopes = next;
                    }
                }
                y += row_h;
                for (_, label, id) in options {
                    let on = scope_on(&var.scopes, id);
                    let row = Rect::from_min_size(Pos2::new(full.left() + 8.0, y), Vec2::new(w - 16.0, row_h));
                    let r = Rect::from_min_max(row.min + Vec2::new(16.0, 0.0), row.max);
                    if checkbox(ui, r, id, label, on) {
                        let next = toggle_scope(var.kind, &var.scopes, id);
                        if let Some(v) = fr.work.variables.get_mut(&var.id) {
                            v.scopes = next;
                        }
                    }
                    y += row_h;
                }
                y += 8.0;
            }
            widgets::hline(&p, full.left(), full.right(), y - 4.0, theme::BORDER_MENU);
            let row = Rect::from_min_size(Pos2::new(full.left() + 8.0, y + 4.0), Vec2::new(w - 16.0, row_h));
            if checkbox(ui, row, "hide", "Hide from publishing", var.hidden_from_publishing) {
                if let Some(v) = fr.work.variables.get_mut(&var.id) {
                    v.hidden_from_publishing = !v.hidden_from_publishing;
                }
            }
            y += row_h + 16.0;
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
        // Open: the Agents panel draws before this popover and must leave it `Esc` and `Enter`.
        crate::widgets::popup_open::mark_open(ctx);
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

    fn strings(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn the_stroke_row_is_on_for_either_stroke_colour_spelling() {
        for spelling in ["STROKE", "STROKE_COLOR"] {
            assert!(scope_on(&strings(&["ALL_FILLS", spelling]), "STROKE_COLOR"), "{spelling} keeps the Stroke row on");
            assert!(scope_on(&strings(&[spelling]), "STROKE_COLOR"), "{spelling} alone keeps the Stroke row on");
            assert!(!scope_on(&strings(&[spelling]), "ALL_FILLS"), "{spelling} leaves the fill rows off");
        }
        assert!(!scope_on(&strings(&["ALL_FILLS"]), "STROKE_COLOR"));
        assert!(scope_on(&[], "STROKE_COLOR"), "no scopes means every row is on");
    }

    #[test]
    fn toggling_the_stroke_row_off_removes_both_spellings() {
        let k = VariableKind::Color;
        assert_eq!(toggle_scope(k, &strings(&["ALL_FILLS", "STROKE"]), "STROKE_COLOR"), strings(&["ALL_FILLS"]));
        assert_eq!(toggle_scope(k, &strings(&["ALL_FILLS", "STROKE_COLOR"]), "STROKE_COLOR"), strings(&["ALL_FILLS"]));
        assert_eq!(toggle_scope(k, &strings(&["STROKE", "ALL_FILLS", "STROKE_COLOR"]), "STROKE_COLOR"), strings(&["ALL_FILLS"]));
        // the last property cannot be unchecked, whichever spelling holds it
        assert_eq!(toggle_scope(k, &strings(&["STROKE"]), "STROKE_COLOR"), strings(&["STROKE"]));
        assert_eq!(toggle_scope(k, &strings(&["STROKE", "STROKE_COLOR"]), "STROKE_COLOR"), strings(&["STROKE", "STROKE_COLOR"]));
    }

    #[test]
    fn toggling_the_stroke_row_on_adds_the_rest_spelling() {
        let k = VariableKind::Color;
        assert_eq!(toggle_scope(k, &strings(&["ALL_FILLS"]), "STROKE_COLOR"), strings(&["ALL_FILLS", "STROKE_COLOR"]));
    }

    #[test]
    fn every_row_on_with_the_editor_spelling_collapses_to_all() {
        let k = VariableKind::Color;
        let nearly_all = strings(&["ALL_FILLS", "FRAME_FILL", "SHAPE_FILL", "TEXT_FILL", "STROKE"]);
        assert!(toggle_scope(k, &nearly_all, "EFFECT_COLOR").is_empty());
    }
}
