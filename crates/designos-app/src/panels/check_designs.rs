//! "Check designs" panel: lists hard-coded values, contrast failures and detached instances, with a
//! one-click Fix per finding and Fix all. Clicking a row selects and frames the layer.

use crate::app::App;
use crate::check_designs::{Finding, Report, Rule, MAX_SCANNED_LAYERS};
use crate::icons::{self, Icon};
use crate::terminal::{STATUS_BAD_TEXT, STATUS_INFO_TEXT, STATUS_OK_TEXT, STATUS_WARN_BG, STATUS_WARN_BORDER, STATUS_WARN_TEXT};
use crate::theme;
use crate::widgets;
use designos_doc::NodeId;
use egui::{Align2, Area, Color32, Context, Id, Order, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

const PANEL_W: f32 = 320.0;
const PAD: f32 = 12.0;
const HEADER_H: f32 = 44.0;
const SUMMARY_H: f32 = 44.0;
const GROUP_H: f32 = 28.0;
/// A row is name + a message of at most two lines, so every row has the same height and a long
/// list only lays out the rows in view.
const ROW_H: f32 = 60.0;
const EMPTY_H: f32 = 112.0;
const NOTE_H: f32 = 44.0;
const BTN_H: f32 = 24.0;
const FIX_W: f32 = 44.0;
const FIX_ALL_W: f32 = 64.0;
const ICON_COL: f32 = 24.0;
const MARGIN: f32 = 12.0;

/// What the user did this frame; applied after drawing so the document is not borrowed while painting.
enum Act {
    Close,
    FixAll,
    Fix(usize),
    Focus(NodeId),
}

fn rule_icon(rule: Rule) -> (Icon, Color32) {
    match rule {
        Rule::HardCodedValue => (Icon::Variables, STATUS_WARN_TEXT),
        Rule::Contrast => (Icon::Eye, STATUS_BAD_TEXT),
        Rule::DetachedInstance => (Icon::Component, STATUS_INFO_TEXT),
    }
}

/// Width the message wraps at in a row, given whether the row has a Fix button.
fn message_width(has_fix: bool) -> f32 {
    PANEL_W - PAD - ICON_COL - PAD - if has_fix { FIX_W + 8.0 } else { 0.0 }
}

fn message_job(f: &Finding, color: Color32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::simple(f.message.clone(), theme::font_body(), color, message_width(f.fix.is_some()));
    job.wrap.max_rows = 2;
    job
}

fn list_height(report: &Report) -> f32 {
    let notes_h = (report.unreadable_libraries.len() as f32) * 28.0 + if report.truncated { NOTE_H } else { 0.0 };
    if report.findings.is_empty() {
        return EMPTY_H + notes_h;
    }
    let groups = Rule::ALL.iter().filter(|r| report.of(**r).next().is_some()).count() as f32;
    groups * GROUP_H + report.findings.len() as f32 * ROW_H + notes_h
}

pub fn show(app: &mut App, ctx: &Context) {
    if !app.check_designs.open {
        return;
    }
    // A drag or a scrub changes the document every frame; the list catches up when the pointer is released.
    let interacting = ctx.input(|i| i.pointer.any_down());
    app.refresh_check_designs(interacting);
    let report = std::mem::take(&mut app.check_designs.report);
    let scope_label = app.check_designs.scope_label.clone();
    let canvas = app.canvas_rect;
    let max_h = (canvas.height() - 2.0 * MARGIN).max(HEADER_H + SUMMARY_H + 96.0);
    let list_h = list_height(&report).min(max_h - HEADER_H - SUMMARY_H);
    let size = Vec2::new(PANEL_W, HEADER_H + SUMMARY_H + list_h);
    let pos = Pos2::new((canvas.right() - PANEL_W - MARGIN).max(canvas.left() + MARGIN), canvas.top() + MARGIN);
    let mut act: Option<Act> = None;

    Area::new(Id::new("designos-check-designs")).order(Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
        let (rect, _) = ui.allocate_exact_size(size, Sense::click());
        widgets::record(ui, "check-designs-panel", rect);
        let p = ui.painter().clone();
        widgets::elevation_400(&p, rect, theme::RADIUS_FRAME);
        p.rect_filled(rect, theme::cr(theme::RADIUS_FRAME), theme::BG_MENU);
        p.rect_stroke(rect, theme::cr(theme::RADIUS_FRAME), Stroke::new(1.0, theme::BORDER_MENU), StrokeKind::Inside);

        // Header: title and close.
        p.text(Pos2::new(rect.left() + PAD, rect.top() + HEADER_H / 2.0), Align2::LEFT_CENTER, "Check designs", theme::font_large_strong(), theme::TEXT);
        let close = Rect::from_center_size(Pos2::new(rect.right() - PAD - BTN_H / 2.0, rect.top() + HEADER_H / 2.0), Vec2::splat(BTN_H));
        widgets::record(ui, "check-designs-close", close);
        if widgets::icon_button(ui, close, "check-designs-close", Icon::Close, false, "Close").clicked() {
            act = Some(Act::Close);
        }
        widgets::hline(&p, rect.left(), rect.right(), rect.top() + HEADER_H, theme::BORDER_SUBTLE);

        // Summary: scope, counts, Fix all.
        let top = rect.top() + HEADER_H;
        let fixable = report.fixable().len();
        let counts = if report.findings.is_empty() {
            format!("No issues · {} layers checked", report.scanned)
        } else {
            let n = report.findings.len();
            format!("{n} {} · {} layers checked", if n == 1 { "issue" } else { "issues" }, report.scanned)
        };
        let text_w = PANEL_W - 2.0 * PAD - FIX_ALL_W - 8.0;
        let scope_txt = widgets::elide(&p, &scope_label, &theme::font_body_medium(), text_w);
        p.text(Pos2::new(rect.left() + PAD, top + 14.0), Align2::LEFT_CENTER, scope_txt, theme::font_body_medium(), theme::TEXT);
        let counts_txt = widgets::elide(&p, &counts, &theme::font_body(), text_w);
        p.text(Pos2::new(rect.left() + PAD, top + 30.0), Align2::LEFT_CENTER, counts_txt, theme::font_body(), theme::TEXT_SECONDARY);
        let fix_all = Rect::from_min_size(Pos2::new(rect.right() - PAD - FIX_ALL_W, top + (SUMMARY_H - BTN_H) / 2.0), Vec2::new(FIX_ALL_W, BTN_H));
        widgets::record(ui, "check-designs-fix-all", fix_all);
        let resp = ui.interact(fix_all, Id::new("check-designs-fix-all"), if fixable > 0 { Sense::click() } else { Sense::hover() });
        let (bg, fg) = match (fixable > 0, resp.hovered()) {
            (true, true) => (theme::BG_BRAND_HOVER, Color32::WHITE),
            (true, false) => (theme::BG_BRAND, Color32::WHITE),
            (false, _) => (theme::BG_TERTIARY, theme::TEXT_DISABLED),
        };
        p.rect_filled(fix_all, theme::cr(theme::RADIUS_CONTROL), bg);
        p.text(fix_all.center(), Align2::CENTER_CENTER, "Fix all", theme::font_body_medium(), fg);
        if fixable > 0 && resp.on_hover_text(format!("Apply {fixable} fixes as one undo step")).clicked() {
            act = Some(Act::FixAll);
        }
        widgets::hline(&p, rect.left(), rect.right(), top + SUMMARY_H, theme::BORDER_SUBTLE);

        // List.
        let list_rect = Rect::from_min_size(Pos2::new(rect.left(), top + SUMMARY_H), Vec2::new(PANEL_W, list_h));
        ui.scope_builder(egui::UiBuilder::new().max_rect(list_rect), |ui| {
            egui::ScrollArea::vertical().id_salt("check-designs-list").max_height(list_h).auto_shrink([false, false]).show(ui, |ui| {
                ui.set_width(PANEL_W);
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                for lib_name in &report.unreadable_libraries {
                    unreadable_library_note(ui, lib_name);
                }
                if report.findings.is_empty() {
                    empty_state(ui, report.scanned);
                }
                for rule in Rule::ALL {
                    let items: Vec<(usize, &Finding)> = report.findings.iter().enumerate().filter(|(_, f)| f.rule == rule).collect();
                    if items.is_empty() {
                        continue;
                    }
                    group_header(ui, rule, items.len());
                    for (idx, f) in items {
                        if let Some(a) = row(ui, idx, f) {
                            act = Some(a);
                        }
                    }
                }
                if report.truncated {
                    truncation_note(ui);
                }
            });
        });
    });

    match act {
        Some(Act::Close) => app.check_designs.open = false,
        Some(Act::Focus(id)) => {
            if app.doc.get(id).is_some() {
                app.select_only(id);
                app.fit_selection();
            }
        }
        Some(Act::Fix(i)) => {
            if let Some(f) = report.findings.get(i) {
                if app.check_designs_fix(f) {
                    app.show_toast(format!("{} · Cmd+Z to undo", f.fix_label), false);
                } else {
                    app.show_toast("That layer changed since the check. The list is refreshed.", true);
                }
                app.check_designs.invalidate();
            }
        }
        Some(Act::FixAll) => {
            let n = app.check_designs_fix_all(&report.findings);
            app.show_toast(if n == 0 { "Nothing could be fixed".to_string() } else { format!("Fixed {n} in one step · Cmd+Z to undo") }, n == 0);
            app.check_designs.invalidate();
        }
        None => {}
    }
    app.check_designs.report = report;
}

fn group_header(ui: &mut Ui, rule: Rule, count: usize) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(PANEL_W, GROUP_H), Sense::hover());
    let p = ui.painter();
    let title = rule.title();
    let font = theme::font_body_strong();
    p.text(Pos2::new(rect.left() + PAD, rect.center().y), Align2::LEFT_CENTER, title, font.clone(), theme::TEXT_SECONDARY);
    let tw = widgets::text_width(p, title, &font);
    let badge = Rect::from_center_size(Pos2::new(rect.left() + PAD + tw + 8.0 + 10.0, rect.center().y), Vec2::new(20.0, 16.0));
    p.rect_filled(badge, theme::cr(theme::RADIUS_FRAME), theme::BG_TERTIARY);
    p.text(badge.center(), Align2::CENTER_CENTER, count.to_string(), theme::font_caption_medium(), theme::TEXT);
}

fn row(ui: &mut Ui, idx: usize, f: &Finding) -> Option<Act> {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(PANEL_W, ROW_H), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return None;
    }
    widgets::record(ui, &format!("check-designs-row-{idx}"), rect);
    let row_resp = ui.interact(rect, Id::new(("check-designs-row", idx)), Sense::click());
    let p = ui.painter().clone();
    if row_resp.hovered() {
        p.rect_filled(rect, 0.0, theme::BG_HOVER);
    }
    let (icon, tint) = rule_icon(f.rule);
    icons::paint(&p, icon, Pos2::new(rect.left() + PAD + 8.0, rect.top() + 16.0), 14.0, tint);
    let text_x = rect.left() + PAD + ICON_COL;
    let has_fix = f.fix.is_some();
    let from_lib = f.is_from_library();
    let badge_w = if from_lib { 48.0 } else { 0.0 };
    let name = widgets::elide(&p, &f.node_name, &theme::font_body_medium(), (message_width(has_fix) - badge_w).max(40.0));
    p.text(Pos2::new(text_x, rect.top() + 16.0), Align2::LEFT_CENTER, &name, theme::font_body_medium(), theme::TEXT);
    if from_lib {
        let nw = widgets::text_width(&p, &name, &theme::font_body_medium());
        let badge = Rect::from_min_size(Pos2::new(text_x + nw + 6.0, rect.top() + 8.0), Vec2::new(42.0, 16.0));
        p.rect_filled(badge, theme::cr(theme::RADIUS_MARK), theme::BG_TERTIARY);
        p.text(badge.center(), Align2::CENTER_CENTER, "Library", theme::font_caption(), theme::TEXT_SECONDARY);
    }
    let galley = p.layout_job(message_job(f, theme::TEXT_SECONDARY));
    p.galley(Pos2::new(text_x, rect.top() + 26.0), galley, theme::TEXT_SECONDARY);
    widgets::hline(&p, rect.left() + PAD, rect.right() - PAD, rect.bottom() - 1.0, theme::BORDER_SUBTLE);

    let mut act = row_resp.clicked().then_some(Act::Focus(f.node));
    if has_fix {
        let btn = Rect::from_min_size(Pos2::new(rect.right() - PAD - FIX_W, rect.top() + 8.0), Vec2::new(FIX_W, BTN_H));
        widgets::record(ui, &format!("check-designs-fix-{idx}"), btn);
        let resp = ui.interact(btn, Id::new(("check-designs-fix", idx)), Sense::click());
        p.rect_filled(btn, theme::cr(theme::RADIUS_CONTROL), if resp.hovered() { theme::BG_PRESSED } else { theme::BG_TERTIARY });
        p.text(btn.center(), Align2::CENTER_CENTER, "Fix", theme::font_body_medium(), theme::TEXT);
        if resp.on_hover_text(f.fix_label.clone()).clicked() {
            act = Some(Act::Fix(idx));
        }
    }
    act
}

fn unreadable_library_note(ui: &mut Ui, name: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(PANEL_W, 28.0), Sense::hover());
    widgets::record(ui, "check-designs-unreadable-library", rect);
    let p = ui.painter();
    let note = format!("Library \"{name}\" couldn't be read; its suggestions are missing");
    let txt = widgets::elide(p, &note, &theme::font_caption(), PANEL_W - 2.0 * PAD);
    p.text(Pos2::new(rect.left() + PAD, rect.center().y), Align2::LEFT_CENTER, txt, theme::font_caption(), theme::TEXT_SECONDARY);
    widgets::hline(p, rect.left() + PAD, rect.right() - PAD, rect.bottom() - 1.0, theme::BORDER_SUBTLE);
}

fn empty_state(ui: &mut Ui, scanned: usize) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(PANEL_W, EMPTY_H), Sense::hover());
    let p = ui.painter();
    icons::paint(p, Icon::Check, Pos2::new(rect.center().x, rect.top() + 28.0), 20.0, STATUS_OK_TEXT);
    p.text(Pos2::new(rect.center().x, rect.top() + 52.0), Align2::CENTER_CENTER, "No issues found", theme::font_body_strong(), theme::TEXT);
    let note = format!("Hard-coded values, contrast and detached instances were checked on {scanned} layers.");
    let galley = p.layout(note, theme::font_body(), theme::TEXT_SECONDARY, PANEL_W - 2.0 * PAD - 24.0);
    p.galley(Pos2::new(rect.center().x - galley.size().x / 2.0, rect.top() + 66.0), galley, theme::TEXT_SECONDARY);
}

fn truncation_note(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(PANEL_W, NOTE_H), Sense::hover());
    let p = ui.painter();
    let inner = rect.shrink2(Vec2::new(PAD, 6.0));
    p.rect_filled(inner, theme::cr(theme::RADIUS_CONTROL), STATUS_WARN_BG);
    p.rect_stroke(inner, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, STATUS_WARN_BORDER), StrokeKind::Inside);
    let note = format!("Stopped after {MAX_SCANNED_LAYERS} layers. Select fewer layers to check the rest.");
    let galley = p.layout(note, theme::font_body(), STATUS_WARN_TEXT, inner.width() - theme::SPACE_BLOCK);
    p.galley(Pos2::new(inner.left() + theme::SPACE_INLINE, inner.center().y - galley.size().y / 2.0), galley, STATUS_WARN_TEXT);
}
