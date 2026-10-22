//! "Check designs" panel: lists design-system findings across Figma's rule set, with a one-click
//! Fix per finding, Fix all, per-finding Ignore / Dismiss, and a Rules & contrast threshold popover.

use crate::app::App;
use crate::check_designs::{can_revert_fix_row, AutoFixReport, Finding, FixRow, Phase, Report, Rule, MAX_AUTO_FIX_ROWS};
use crate::icons::{self, Icon};
use crate::terminal::{STATUS_BAD_TEXT, STATUS_INFO_TEXT, STATUS_OK_TEXT, STATUS_WARN_TEXT};
use crate::theme;
use crate::widgets;
use designos_doc::{AutoFixMode, AutoFixSettings, ContrastLevel, IgnoreKey, NodeId, SAFE_AUTO_FIX_RULES};
use egui::{Align2, Area, Color32, Context, FontId, Id, Order, Pos2, Rect, Sense, Stroke, StrokeKind, TextFormat, Ui, Vec2};

const PANEL_W: f32 = 320.0;
const PAD: f32 = theme::SPACE_SECTION;
const HEADER_H: f32 = 44.0;
const SUMMARY_H: f32 = 44.0;
const GROUP_H: f32 = theme::CONTROL_STANDARD;
/// A row is name + a message of at most two lines, so every row has the same height and a long
/// list only lays out the rows in view.
const ROW_H: f32 = 60.0;
const PROGRESS_H: f32 = 32.0;
const IGNORED_BAR_H: f32 = 32.0;
const IGNORED_ROW_H: f32 = 44.0;
const EMPTY_H: f32 = 112.0;
const BTN_H: f32 = theme::CONTROL_DENSE;
const FIX_W: f32 = 44.0;
const RESTORE_W: f32 = 56.0;
const FIX_ALL_W: f32 = 64.0;
const ICON_COL: f32 = 24.0;
const MARGIN: f32 = theme::SPACE_SECTION;
const HAIRLINE: f32 = theme::BORDER_WIDTH;
const ONE: f32 = theme::BORDER_WIDTH;
const GAP: f32 = theme::SPACE_INLINE;
const FACT_GAP: f32 = theme::SPACE_FACT;
const SEG_INSET: f32 = theme::INSET_SEGMENT;
const ICON_SM: f32 = 12.0;
const CHECK_BOX_SIZE: f32 = 14.0;
const STATUS_PILL_W: f32 = 64.0;
const STATUS_PILL_H: f32 = 16.0;
const STATUS_PILL_OFFSET: f32 = 40.0;
const VALUE_COL_RIGHT_PAD: f32 = 32.0;
const LABEL_MAX_W: f32 = 148.0;
const DIFF_MAX_W: f32 = 108.0;
const SEG_COUNT: f32 = 3.0;
const CHEVRON_OFFSET: f32 = 6.0;
const TITLE_AFTER_CHEVRON: f32 = 16.0;
const CHECK_TEXT_OFFSET: f32 = 22.0;
/// Rules popover: width, checkbox row height and the x offsets of its checkbox and label.
const POPOVER_W: f32 = 240.0;
const POPOVER_ROW_H: f32 = 26.0;
const POPOVER_CHECK_X: f32 = 14.0;
const POPOVER_LABEL_X: f32 = 28.0;
const CONTRAST_BTN_W: f32 = 40.0;
/// Row layout: the baselines of the name and the message, and the gap between two icon buttons.
const ROW_NAME_Y: f32 = 16.0;
const ROW_MESSAGE_Y: f32 = 26.0;
const ICON_BTN_GAP: f32 = 2.0;
/// Empty state: the check icon, the heading and the note under it.
const EMPTY_ICON_Y: f32 = 28.0;
const EMPTY_TITLE_Y: f32 = 52.0;
const EMPTY_NOTE_Y: f32 = 66.0;
/// Progress row: where the bar and its label sit, and the bar's thickness.
const PROGRESS_BAR_Y: f32 = 7.0;
const PROGRESS_LABEL_Y: f32 = 15.0;
const PROGRESS_BAR_H: f32 = 4.0;

const POPOVER_RULES: [(Rule, &str); 9] = [
    (Rule::HardCodedValue, "Colours & contrast"),
    (Rule::DetachedInstance, "Detached instances"),
    (Rule::LibraryAssets, "Library assets"),
    (Rule::TextStyles, "Text styles"),
    (Rule::CornerRadius, "Corner radius"),
    (Rule::Padding, "Padding"),
    (Rule::HorizontalSpacing, "Horizontal spacing"),
    (Rule::VerticalSpacing, "Vertical spacing"),
    (Rule::TypographyAndStroke, "Typography and stroke"),
];

/// What the user did this frame; applied after drawing so the document is not borrowed while painting.
enum Act {
    Close,
    ToggleRules,
    ToggleRule(Rule, bool),
    SetContrast(ContrastLevel),
    FixAll,
    Fix(usize),
    Ignore(usize),
    Dismiss(usize),
    ToggleShowIgnored,
    Restore(IgnoreKey),
    Focus(NodeId),
    RevertAutoFix(usize),
    ToggleAutoFixSettings,
    SetFileMode(AutoFixMode),
    SetFileRule(&'static str, bool),
    SetUserMode(AutoFixMode),
    SetUserRule(&'static str, bool),
}

fn rule_icon(rule: Rule) -> (Icon, Color32) {
    match rule {
        Rule::HardCodedValue | Rule::CornerRadius | Rule::Padding | Rule::HorizontalSpacing | Rule::VerticalSpacing | Rule::TypographyAndStroke => {
            (Icon::Variables, STATUS_WARN_TEXT)
        }
        Rule::TextStyles => (Icon::Text, STATUS_WARN_TEXT),
        Rule::Contrast => (Icon::Eye, STATUS_BAD_TEXT),
        Rule::DetachedInstance | Rule::LibraryAssets => (Icon::Component, STATUS_INFO_TEXT),
    }
}

/// Width of the buttons at a row's right edge: Ignore and Dismiss, plus Fix when the row has one.
fn actions_width(has_fix: bool) -> f32 {
    2.0 * (BTN_H + 4.0) + if has_fix { FIX_W + 4.0 } else { 0.0 }
}

/// Width the message wraps at in a row, given whether the row has a Fix button.
fn message_width(has_fix: bool) -> f32 {
    PANEL_W - PAD - ICON_COL - PAD - actions_width(has_fix)
}

fn message_job(f: &Finding, color: Color32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::simple(f.message.clone(), theme::font_body(), color, message_width(f.fix.is_some()));
    job.wrap.max_rows = 2;
    job
}

fn auto_fix_section_height(last: Option<&AutoFixReport>, settings_open: bool, settings_unreadable: bool) -> f32 {
    let rows_count = last.map_or(0, |r| r.rows.len());
    let shown_rows = rows_count.min(MAX_AUTO_FIX_ROWS);
    let rows_h = if rows_count == 0 {
        GROUP_H
    } else {
        let more_h = if rows_count > MAX_AUTO_FIX_ROWS { BTN_H } else { 0.0 };
        (shown_rows as f32) * GROUP_H + more_h
    };
    let rule_rows_h = (SAFE_AUTO_FIX_RULES.len() as f32) * BTN_H;
    let scope_block_h = BTN_H + GROUP_H + rule_rows_h;
    let unreadable_h = if settings_unreadable { BTN_H } else { 0.0 };
    let settings_body_h = if settings_open { unreadable_h + scope_block_h + scope_block_h + GAP } else { 0.0 };
    GROUP_H + BTN_H + rows_h + GROUP_H + settings_body_h
}

fn list_height(report: &Report, show_ignored: bool, scanning: bool, auto_fix_h: f32) -> f32 {
    let notes_h = (report.unreadable_libraries.len() as f32) * 28.0 + if scanning { PROGRESS_H } else { 0.0 };
    let ignored_h = if report.ignored.is_empty() {
        0.0
    } else if show_ignored {
        IGNORED_BAR_H + (report.ignored.len() as f32) * IGNORED_ROW_H
    } else {
        IGNORED_BAR_H
    };
    let findings_h = if report.findings.is_empty() {
        let base_h = if report.complete { EMPTY_H } else { 0.0 };
        (base_h + notes_h + ignored_h).max(PROGRESS_H)
    } else {
        let groups = Rule::ALL.iter().filter(|r| report.of(**r).next().is_some()).count() as f32;
        groups * GROUP_H + report.findings.len() as f32 * ROW_H + notes_h + ignored_h
    };
    findings_h + auto_fix_h
}

pub fn show(app: &mut App, ctx: &Context) {
    if !app.check_designs.open {
        return;
    }
    // A drag or a scrub changes the document every frame; the list catches up when the pointer is released.
    let interacting = ctx.input(|i| i.pointer.any_down());
    app.refresh_check_designs(interacting, ctx);
    let progress_info = app.check_designs.cursor.as_ref().map(|c| (c.phase, c.scanned, c.total, c.updating));
    let scanning = progress_info.is_some();
    let report = std::mem::take(&mut app.check_designs.report);
    let scope_label = app.check_designs.scope_label.clone();
    let rules_open = app.check_designs.rules_open;
    let show_ignored = app.check_designs.show_ignored;
    let settings = app.doc.check_designs.clone();
    let last_auto_fix = app.check_designs.last_auto_fix.clone();
    let settings_open = app.check_designs.settings_open;
    let settings_unreadable = app.check_designs.settings_unreadable;
    let scroll_to_auto_fix = std::mem::take(&mut app.check_designs.scroll_to_auto_fix);
    let effective_af = app.effective_auto_fix();
    let user_af = app.check_designs.user_auto_fix.clone();
    let auto_fix_h = auto_fix_section_height(last_auto_fix.as_ref(), settings_open, settings_unreadable);
    let canvas = app.canvas_rect;
    let max_h = (canvas.height() - 2.0 * MARGIN).max(HEADER_H + SUMMARY_H + 96.0);
    let list_h = list_height(&report, show_ignored, scanning, auto_fix_h).min(max_h - HEADER_H - SUMMARY_H);
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

        // Header: title, Rules popover button, and Close.
        p.text(Pos2::new(rect.left() + PAD, rect.top() + HEADER_H / 2.0), Align2::LEFT_CENTER, "Check designs", theme::font_large_strong(), theme::TEXT);
        let close = Rect::from_center_size(Pos2::new(rect.right() - PAD - BTN_H / 2.0, rect.top() + HEADER_H / 2.0), Vec2::splat(BTN_H));
        widgets::record(ui, "check-designs-close", close);
        if widgets::icon_button(ui, close, "check-designs-close", Icon::Close, false, "Close").clicked() {
            act = Some(Act::Close);
        }
        let rules_btn = Rect::from_center_size(Pos2::new(close.left() - theme::SPACE_FACT - BTN_H / 2.0, rect.top() + HEADER_H / 2.0), Vec2::splat(BTN_H));
        widgets::record(ui, "check-designs-rules", rules_btn);
        if widgets::icon_button(ui, rules_btn, "check-designs-rules", Icon::Sliders, rules_open, "Rules & contrast level").clicked() {
            act = Some(Act::ToggleRules);
        }
        widgets::hline(&p, rect.left(), rect.right(), rect.top() + HEADER_H, theme::BORDER_SUBTLE);

        // Summary: scope, counts, Fix all.
        let top = rect.top() + HEADER_H;
        let fixable = report.fixable().len();
        let can_fix_all = !scanning && fixable > 0;
        let counts = if scanning && !report.complete {
            match progress_info {
                Some((Phase::Prepare, _, _, _)) => "Preparing…".to_string(),
                Some((_, scanned, total, _)) => format!("Checking… {scanned} of {total} layers"),
                None => String::new(),
            }
        } else if report.findings.is_empty() {
            format!("No issues · {} layers checked", report.scanned)
        } else {
            let n = report.findings.len();
            format!("{n} {} · {} layers checked", if n == 1 { "issue" } else { "issues" }, report.scanned)
        };
        let text_w = PANEL_W - 2.0 * PAD - FIX_ALL_W - GAP;
        let scope_txt = widgets::elide(&p, &scope_label, &theme::font_body_medium(), text_w);
        p.text(Pos2::new(rect.left() + PAD, top + 14.0), Align2::LEFT_CENTER, scope_txt, theme::font_body_medium(), theme::TEXT);
        let counts_txt = widgets::elide(&p, &counts, &theme::font_body(), text_w);
        p.text(Pos2::new(rect.left() + PAD, top + 30.0), Align2::LEFT_CENTER, counts_txt, theme::font_body(), theme::TEXT_SECONDARY);
        let fix_all = Rect::from_min_size(Pos2::new(rect.right() - PAD - FIX_ALL_W, top + (SUMMARY_H - BTN_H) / 2.0), Vec2::new(FIX_ALL_W, BTN_H));
        widgets::record(ui, "check-designs-fix-all", fix_all);
        let resp = ui.interact(fix_all, Id::new("check-designs-fix-all"), if can_fix_all { Sense::click() } else { Sense::hover() });
        let (bg, fg) = match (can_fix_all, resp.hovered()) {
            (true, true) => (theme::BG_BRAND_HOVER, Color32::WHITE),
            (true, false) => (theme::BG_BRAND, Color32::WHITE),
            (false, _) => (theme::BG_TERTIARY, theme::TEXT_DISABLED),
        };
        p.rect_filled(fix_all, theme::cr(theme::RADIUS_CONTROL), bg);
        p.text(fix_all.center(), Align2::CENTER_CENTER, "Fix all", theme::font_body_medium(), fg);
        if scanning {
            resp.on_hover_text("Wait for the check to finish");
        } else if can_fix_all && resp.on_hover_text(format!("Apply {fixable} fixes as one undo step")).clicked() {
            act = Some(Act::FixAll);
        }
        widgets::hline(&p, rect.left(), rect.right(), top + SUMMARY_H, theme::BORDER_SUBTLE);

        // List.
        let list_rect = Rect::from_min_size(Pos2::new(rect.left(), top + SUMMARY_H), Vec2::new(PANEL_W, list_h));
        ui.scope_builder(egui::UiBuilder::new().max_rect(list_rect), |ui| {
            egui::ScrollArea::vertical().id_salt("check-designs-list").max_height(list_h).auto_shrink([false, false]).show(ui, |ui| {
                ui.set_width(PANEL_W);
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                if let Some((phase, scanned, total, updating)) = progress_info {
                    progress_row(ui, phase, scanned, total, updating);
                }
                if let Some(a) =
                    auto_fix_section(ui, &app.doc, last_auto_fix.as_ref(), settings_open, settings_unreadable, &effective_af, &user_af, scroll_to_auto_fix)
                {
                    act = Some(a);
                }
                for lib_name in &report.unreadable_libraries {
                    unreadable_library_note(ui, lib_name);
                }
                if report.findings.is_empty() && report.complete {
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
                if !report.ignored.is_empty() {
                    if let Some(a) = ignored_footer(ui, &report.ignored, show_ignored) {
                        act = Some(a);
                    }
                }
            });
        });
    });

    if rules_open {
        let pop_pos = Pos2::new(pos.x + PANEL_W - POPOVER_W - PAD, pos.y + HEADER_H + theme::SPACE_FACT);
        Area::new(Id::new("designos-check-designs-rules-popover")).order(Order::Tooltip).fixed_pos(pop_pos).show(ctx, |ui| {
            if let Some(a) = rules_popover(ui, &settings) {
                act = Some(a);
            }
        });
    }

    match act {
        Some(Act::Close) => {
            app.check_designs.open = false;
            app.check_designs.rules_open = false;
            app.check_designs.cursor = None;
        }
        Some(Act::ToggleRules) => {
            app.check_designs.rules_open = !app.check_designs.rules_open;
        }
        Some(Act::ToggleRule(rule, on)) => {
            app.set_check_designs_rule(rule, on);
        }
        Some(Act::SetContrast(level)) => {
            app.set_contrast_level(level);
        }
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
        Some(Act::Ignore(i)) => {
            if let Some(f) = report.findings.get(i) {
                app.check_designs_ignore(f);
            }
        }
        Some(Act::Dismiss(i)) => {
            if let Some(f) = report.findings.get(i).cloned() {
                app.check_designs.report = report;
                app.check_designs_dismiss(&f);
                return;
            }
        }
        Some(Act::ToggleShowIgnored) => {
            app.check_designs.show_ignored = !app.check_designs.show_ignored;
        }
        Some(Act::Restore(key)) => {
            app.check_designs_restore(&key);
        }
        Some(Act::FixAll) => {
            let n = app.check_designs_fix_all(&report.findings);
            app.show_toast(if n == 0 { "Nothing could be fixed".to_string() } else { format!("Fixed {n} in one step · Cmd+Z to undo") }, n == 0);
            app.check_designs.invalidate();
        }
        Some(Act::RevertAutoFix(i)) => {
            if app.revert_auto_fix_row(i) {
                app.show_toast("Reverted auto-fix · Cmd+Z to undo", false);
                app.check_designs.invalidate();
            } else {
                app.show_toast("Changed since the fix", true);
            }
        }
        Some(Act::ToggleAutoFixSettings) => {
            app.check_designs.settings_open = !app.check_designs.settings_open;
            app.check_designs.scroll_to_auto_fix = app.check_designs.settings_open;
        }
        Some(Act::SetFileMode(mode)) => {
            app.set_file_auto_fix_mode(mode);
        }
        Some(Act::SetFileRule(rule, enabled)) => {
            app.set_file_auto_fix_rule(rule, enabled);
        }
        Some(Act::SetUserMode(mode)) => {
            app.set_user_auto_fix_mode(mode);
        }
        Some(Act::SetUserRule(rule, enabled)) => {
            app.set_user_auto_fix_rule(rule, enabled);
        }
        None => {}
    }
    app.check_designs.report = report;
}

fn rules_popover(ui: &mut Ui, settings: &designos_doc::CheckDesignsSettings) -> Option<Act> {
    let pop_w = POPOVER_W;
    let row_h = POPOVER_ROW_H;
    let contrast_h = 36.0;
    let inset = theme::INSET_FRAME;
    let pop_h = inset + (POPOVER_RULES.len() as f32) * row_h + inset + contrast_h + inset;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(pop_w, pop_h), Sense::click());
    widgets::record(ui, "check-designs-rules-popover", rect);
    let p = ui.painter().clone();
    widgets::elevation_400(&p, rect, theme::RADIUS_FRAME);
    p.rect_filled(rect, theme::cr(theme::RADIUS_FRAME), theme::BG_MENU);
    p.rect_stroke(rect, theme::cr(theme::RADIUS_FRAME), Stroke::new(1.0, theme::BORDER_MENU), StrokeKind::Inside);

    let mut act = None;
    for (idx, (rule, label)) in POPOVER_RULES.iter().enumerate() {
        let y = rect.top() + inset + (idx as f32) * row_h;
        let r = Rect::from_min_size(Pos2::new(rect.left() + inset, y), Vec2::new(pop_w - 2.0 * inset, row_h));
        let id_str = format!("check-designs-rule-{}", rule.figma_id());
        widgets::record(ui, &id_str, r);
        let resp = ui.interact(r, Id::new(&id_str), Sense::click());
        if resp.hovered() {
            p.rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
        }
        let enabled = !settings.disabled_rules.contains(rule.figma_id());
        let box_rect = Rect::from_center_size(Pos2::new(r.left() + POPOVER_CHECK_X, r.center().y), Vec2::splat(14.0));
        if enabled {
            p.rect_filled(box_rect, theme::cr(theme::RADIUS_MARK), theme::BG_BRAND);
            icons::paint(&p, Icon::Check, box_rect.center(), 10.0, Color32::WHITE);
        } else {
            p.rect_stroke(box_rect, theme::cr(theme::RADIUS_MARK), Stroke::new(1.0, theme::BORDER_MENU), StrokeKind::Inside);
        }
        p.text(Pos2::new(r.left() + POPOVER_LABEL_X, r.center().y), Align2::LEFT_CENTER, *label, theme::font_body(), theme::TEXT);
        if resp.clicked() {
            act = Some(Act::ToggleRule(*rule, !enabled));
        }
    }

    let sep_y = rect.top() + inset + (POPOVER_RULES.len() as f32) * row_h + theme::SPACE_FACT;
    widgets::hline(&p, rect.left() + inset, rect.right() - inset, sep_y, theme::BORDER_SUBTLE);

    let ctrl_y = sep_y + theme::SPACE_FACT + (contrast_h - BTN_H) / 2.0;
    p.text(
        Pos2::new(rect.left() + theme::SPACE_GROUP, ctrl_y + BTN_H / 2.0),
        Align2::LEFT_CENTER,
        "Contrast",
        theme::font_body_medium(),
        theme::TEXT_SECONDARY,
    );
    let aaa_x = rect.right() - theme::SPACE_GROUP - CONTRAST_BTN_W;
    let aa_x = aaa_x - theme::SPACE_FACT - CONTRAST_BTN_W;
    let aa_rect = Rect::from_min_size(Pos2::new(aa_x, ctrl_y), Vec2::new(CONTRAST_BTN_W, BTN_H));
    let aaa_rect = Rect::from_min_size(Pos2::new(aaa_x, ctrl_y), Vec2::new(CONTRAST_BTN_W, BTN_H));
    for (btn_rect, id_str, label, level) in
        [(aa_rect, "check-designs-contrast-aa", "AA", ContrastLevel::AA), (aaa_rect, "check-designs-contrast-aaa", "AAA", ContrastLevel::AAA)]
    {
        widgets::record(ui, id_str, btn_rect);
        let resp = ui.interact(btn_rect, Id::new(id_str), Sense::click());
        let active = settings.contrast_level == level;
        let bg = if active {
            theme::BG_BRAND
        } else if resp.hovered() {
            theme::BG_HOVER
        } else {
            theme::BG_TERTIARY
        };
        let fg = if active { Color32::WHITE } else { theme::TEXT };
        p.rect_filled(btn_rect, theme::cr(theme::RADIUS_CONTROL), bg);
        p.text(btn_rect.center(), Align2::CENTER_CENTER, label, theme::font_caption_medium(), fg);
        if resp.clicked() {
            act = Some(Act::SetContrast(level));
        }
    }
    act
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
    icons::paint(&p, icon, Pos2::new(rect.left() + PAD + theme::SPACE_INLINE, rect.top() + ROW_NAME_Y), 14.0, tint);
    let text_x = rect.left() + PAD + ICON_COL;
    let has_fix = f.fix.is_some();
    let from_lib = f.is_from_library();
    let badge_w = if from_lib { 48.0 } else { 0.0 };
    let actions_w = actions_width(has_fix);
    let name_max_w = (PANEL_W - PAD - ICON_COL - PAD - actions_w - badge_w).max(40.0);
    let name = widgets::elide(&p, &f.node_name, &theme::font_body_medium(), name_max_w);
    p.text(Pos2::new(text_x, rect.top() + ROW_NAME_Y), Align2::LEFT_CENTER, &name, theme::font_body_medium(), theme::TEXT);
    if from_lib {
        let nw = widgets::text_width(&p, &name, &theme::font_body_medium());
        let badge = Rect::from_min_size(Pos2::new(text_x + nw + 6.0, rect.top() + theme::SPACE_INLINE), Vec2::new(42.0, 16.0));
        p.rect_filled(badge, theme::cr(theme::RADIUS_MARK), theme::BG_TERTIARY);
        p.text(badge.center(), Align2::CENTER_CENTER, "Library", theme::font_caption(), theme::TEXT_SECONDARY);
    }
    let galley = p.layout_job(message_job(f, theme::TEXT_SECONDARY));
    p.galley(Pos2::new(text_x, rect.top() + ROW_MESSAGE_Y), galley, theme::TEXT_SECONDARY);
    widgets::hline(&p, rect.left() + PAD, rect.right() - PAD, rect.bottom() - HAIRLINE, theme::BORDER_SUBTLE);

    let mut act = row_resp.clicked().then_some(Act::Focus(f.node));
    let mut right_edge = rect.right() - PAD;
    if has_fix {
        let btn = Rect::from_min_size(Pos2::new(right_edge - FIX_W, rect.top() + theme::SPACE_INLINE), Vec2::new(FIX_W, BTN_H));
        widgets::record(ui, &format!("check-designs-fix-{idx}"), btn);
        let resp = ui.interact(btn, Id::new(("check-designs-fix", idx)), Sense::click());
        p.rect_filled(btn, theme::cr(theme::RADIUS_CONTROL), if resp.hovered() { theme::BG_PRESSED } else { theme::BG_TERTIARY });
        p.text(btn.center(), Align2::CENTER_CENTER, "Fix", theme::font_body_medium(), theme::TEXT);
        if resp.on_hover_text(f.fix_label.clone()).clicked() {
            act = Some(Act::Fix(idx));
        }
        right_edge = btn.left() - theme::SPACE_FACT;
    }

    let dismiss_btn = Rect::from_min_size(Pos2::new(right_edge - BTN_H, rect.top() + theme::SPACE_INLINE), Vec2::splat(BTN_H));
    widgets::record(ui, &format!("check-designs-dismiss-{idx}"), dismiss_btn);
    if widgets::icon_button(ui, dismiss_btn, format!("check-designs-dismiss-{idx}"), Icon::Close, false, "Dismiss for this session").clicked() {
        act = Some(Act::Dismiss(idx));
    }
    right_edge = dismiss_btn.left() - ICON_BTN_GAP;

    let ignore_btn = Rect::from_min_size(Pos2::new(right_edge - BTN_H, rect.top() + theme::SPACE_INLINE), Vec2::splat(BTN_H));
    widgets::record(ui, &format!("check-designs-ignore-{idx}"), ignore_btn);
    if widgets::icon_button(ui, ignore_btn, format!("check-designs-ignore-{idx}"), Icon::EyeOff, false, "Ignore in this file").clicked() {
        act = Some(Act::Ignore(idx));
    }
    act
}

fn ignored_footer(ui: &mut Ui, ignored: &[Finding], show_ignored: bool) -> Option<Act> {
    let mut act = None;
    let (bar, _) = ui.allocate_exact_size(Vec2::new(PANEL_W, IGNORED_BAR_H), Sense::hover());
    widgets::record(ui, "check-designs-ignored-footer", bar);
    let p = ui.painter().clone();
    widgets::hline(&p, bar.left(), bar.right(), bar.top(), theme::BORDER_SUBTLE);
    p.text(
        Pos2::new(bar.left() + PAD, bar.center().y),
        Align2::LEFT_CENTER,
        format!("{} ignored", ignored.len()),
        theme::font_body_medium(),
        theme::TEXT_SECONDARY,
    );
    let toggle_btn = Rect::from_min_size(Pos2::new(bar.right() - PAD - 48.0, bar.center().y - BTN_H / 2.0), Vec2::new(48.0, BTN_H));
    widgets::record(ui, "check-designs-toggle-ignored", toggle_btn);
    let resp = ui.interact(toggle_btn, Id::new("check-designs-toggle-ignored"), Sense::click());
    p.rect_filled(toggle_btn, theme::cr(theme::RADIUS_CONTROL), if resp.hovered() { theme::BG_HOVER } else { theme::BG_TERTIARY });
    p.text(toggle_btn.center(), Align2::CENTER_CENTER, if show_ignored { "Hide" } else { "Show" }, theme::font_caption_medium(), theme::TEXT);
    if resp.clicked() {
        act = Some(Act::ToggleShowIgnored);
    }

    if show_ignored {
        for (idx, f) in ignored.iter().enumerate() {
            let (r, _) = ui.allocate_exact_size(Vec2::new(PANEL_W, IGNORED_ROW_H), Sense::hover());
            widgets::record(ui, &format!("check-designs-ignored-row-{idx}"), r);
            let rp = ui.painter().clone();
            let txt = widgets::elide(&rp, &format!("{} · {}", f.node_name, f.message), &theme::font_caption(), PANEL_W - 2.0 * PAD - RESTORE_W - 8.0);
            rp.text(Pos2::new(r.left() + PAD, r.center().y), Align2::LEFT_CENTER, txt, theme::font_caption(), theme::TEXT_SECONDARY);
            let btn = Rect::from_min_size(Pos2::new(r.right() - PAD - RESTORE_W, r.center().y - BTN_H / 2.0), Vec2::new(RESTORE_W, BTN_H));
            widgets::record(ui, &format!("check-designs-restore-{idx}"), btn);
            let bresp = ui.interact(btn, Id::new(("check-designs-restore", idx)), Sense::click());
            rp.rect_filled(btn, theme::cr(theme::RADIUS_CONTROL), if bresp.hovered() { theme::BG_PRESSED } else { theme::BG_TERTIARY });
            rp.text(btn.center(), Align2::CENTER_CENTER, "Restore", theme::font_caption_medium(), theme::TEXT);
            if bresp.clicked() {
                act = Some(Act::Restore(f.ignore_key()));
            }
            widgets::hline(&rp, r.left() + PAD, r.right() - PAD, r.bottom() - HAIRLINE, theme::BORDER_SUBTLE);
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
    widgets::hline(p, rect.left() + PAD, rect.right() - PAD, rect.bottom() - HAIRLINE, theme::BORDER_SUBTLE);
}

fn empty_state(ui: &mut Ui, scanned: usize) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(PANEL_W, EMPTY_H), Sense::hover());
    let p = ui.painter();
    icons::paint(p, Icon::Check, Pos2::new(rect.center().x, rect.top() + EMPTY_ICON_Y), 20.0, STATUS_OK_TEXT);
    p.text(Pos2::new(rect.center().x, rect.top() + EMPTY_TITLE_Y), Align2::CENTER_CENTER, "No issues found", theme::font_body_strong(), theme::TEXT);
    let note = format!("Enabled design rules were checked on {scanned} layers.");
    let galley = p.layout(note, theme::font_body(), theme::TEXT_SECONDARY, PANEL_W - 2.0 * PAD - 24.0);
    p.galley(Pos2::new(rect.center().x - galley.size().x / 2.0, rect.top() + EMPTY_NOTE_Y), galley, theme::TEXT_SECONDARY);
}

fn progress_row(ui: &mut Ui, phase: Phase, scanned: usize, total: usize, updating: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(PANEL_W, PROGRESS_H), Sense::hover());
    widgets::record(ui, "check-designs-progress", rect);
    let p = ui.painter().clone();
    let bar_w = PANEL_W - 2.0 * PAD;
    let bar = Rect::from_min_size(Pos2::new(rect.left() + PAD, rect.top() + PROGRESS_BAR_Y), Vec2::new(bar_w, PROGRESS_BAR_H));
    widgets::record(ui, "check-designs-progress-bar", bar);
    p.rect_filled(bar, theme::cr(theme::RADIUS_MARK), theme::BG_TERTIARY);
    let frac = if phase == Phase::Prepare || total == 0 { 0.08 } else { (scanned as f32 / total as f32).clamp(0.0, 1.0) };
    let fill = Rect::from_min_size(bar.min, Vec2::new((bar_w * frac).max(PROGRESS_BAR_H), PROGRESS_BAR_H));
    p.rect_filled(fill, theme::cr(theme::RADIUS_MARK), theme::BG_BRAND);

    let prop_font = theme::font_caption();
    let mono_font = FontId::new(theme::FONT_SIZE_CAPTION, egui::FontFamily::Monospace);
    let mut job = egui::text::LayoutJob::default();
    let prop_fmt = TextFormat { font_id: prop_font, color: theme::TEXT_SECONDARY, ..Default::default() };
    let mono_fmt = TextFormat { font_id: mono_font, color: theme::TEXT, ..Default::default() };
    if updating {
        if phase == Phase::Prepare {
            job.append("Updating…", 0.0, prop_fmt);
        } else {
            job.append("Updating… ", 0.0, prop_fmt.clone());
            job.append(&scanned.to_string(), 0.0, mono_fmt.clone());
            job.append(" of ", 0.0, prop_fmt.clone());
            job.append(&total.to_string(), 0.0, mono_fmt);
            job.append(" layers", 0.0, prop_fmt);
        }
    } else if phase == Phase::Prepare {
        job.append("Preparing…", 0.0, prop_fmt);
    } else {
        job.append("Checking… ", 0.0, prop_fmt.clone());
        job.append(&scanned.to_string(), 0.0, mono_fmt.clone());
        job.append(" of ", 0.0, prop_fmt.clone());
        job.append(&total.to_string(), 0.0, mono_fmt);
        job.append(" layers", 0.0, prop_fmt);
    }
    let galley = p.layout_job(job);
    p.galley(Pos2::new(rect.left() + PAD, rect.top() + PROGRESS_LABEL_Y), galley, theme::TEXT_SECONDARY);
    widgets::hline(&p, rect.left() + PAD, rect.right() - PAD, rect.bottom() - HAIRLINE, theme::BORDER_SUBTLE);
}

#[allow(clippy::too_many_arguments)]
fn auto_fix_section(
    ui: &mut Ui,
    doc: &designos_doc::Document,
    last: Option<&AutoFixReport>,
    settings_open: bool,
    settings_unreadable: bool,
    effective: &AutoFixSettings,
    user: &AutoFixSettings,
    scroll_to_section: bool,
) -> Option<Act> {
    let mut act = None;
    auto_fix_header(ui, last, scroll_to_section);
    auto_fix_subheader(ui, last);

    match last {
        None => auto_fix_empty_row(ui, "No auto-fix passes yet"),
        Some(rep) if rep.rows.is_empty() => auto_fix_empty_row(ui, "No issues found"),
        Some(rep) => {
            let shown = rep.rows.len().min(MAX_AUTO_FIX_ROWS);
            let applied = !rep.report_only;
            for (idx, fix_row) in rep.rows.iter().take(shown).enumerate() {
                if let Some(a) = auto_fix_diff_row(ui, doc, idx, fix_row, applied) {
                    act = Some(a);
                }
            }
            if rep.rows.len() > MAX_AUTO_FIX_ROWS {
                let extra = rep.rows.len() - MAX_AUTO_FIX_ROWS;
                auto_fix_more_footer(ui, extra);
            }
        }
    }

    if auto_fix_settings_header(ui, settings_open, effective.mode) {
        act = Some(Act::ToggleAutoFixSettings);
    }

    if settings_open {
        if settings_unreadable {
            auto_fix_unreadable_notice(ui);
        }
        auto_fix_scope_label(ui, "This file");
        if let Some(mode) = auto_fix_mode_segmented(ui, "file", effective.mode, true) {
            act = Some(Act::SetFileMode(mode));
        }
        for rule in SAFE_AUTO_FIX_RULES {
            if let Some(enabled) = auto_fix_rule_checkbox(ui, "file", rule, effective.rule_enabled(rule), true) {
                act = Some(Act::SetFileRule(rule, enabled));
            }
        }

        auto_fix_scope_label(ui, "My default");
        if let Some(mode) = auto_fix_mode_segmented(ui, "user", user.mode, false) {
            act = Some(Act::SetUserMode(mode));
        }
        for rule in SAFE_AUTO_FIX_RULES {
            if let Some(enabled) = auto_fix_rule_checkbox(ui, "user", rule, user.rule_enabled(rule), false) {
                act = Some(Act::SetUserRule(rule, enabled));
            }
        }
    }

    act
}

fn mode_label(mode: AutoFixMode) -> &'static str {
    match mode {
        AutoFixMode::On => "On",
        AutoFixMode::ReportOnly => "Report only",
        AutoFixMode::Off => "Off",
    }
}

fn auto_fix_header(ui: &mut Ui, last: Option<&AutoFixReport>, scroll_to_section: bool) {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(PANEL_W, GROUP_H), Sense::hover());
    if scroll_to_section {
        resp.scroll_to_me(Some(egui::Align::TOP));
    }
    widgets::record(ui, "check-designs-auto-fix-header", rect);
    let p = ui.painter();
    widgets::hline(p, rect.left(), rect.right(), rect.top(), theme::BORDER_SUBTLE);
    let title = "Auto-fix";
    let font = theme::font_body_strong();
    p.text(Pos2::new(rect.left() + PAD, rect.center().y), Align2::LEFT_CENTER, title, font.clone(), theme::TEXT_SECONDARY);
    let count = last.map_or(0, |r| r.rows.len());
    let tw = widgets::text_width(p, title, &font);
    let badge_x = rect.left() + PAD + tw + GAP;
    let badge = Rect::from_min_size(Pos2::new(badge_x, rect.center().y - GAP), Vec2::new(ICON_COL, PAD));
    widgets::record(ui, "check-designs-auto-fix-count", badge);
    p.rect_filled(badge, theme::cr(theme::RADIUS_FRAME), theme::BG_TERTIARY);
    p.text(badge.center(), Align2::CENTER_CENTER, count.to_string(), theme::font_caption_medium(), theme::TEXT);
}

fn auto_fix_subheader(ui: &mut Ui, last: Option<&AutoFixReport>) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(PANEL_W, BTN_H), Sense::hover());
    let p = ui.painter();
    let label = match last {
        Some(rep) if rep.report_only => "Fixes available",
        _ => "Last agent pass",
    };
    p.text(Pos2::new(rect.left() + PAD, rect.center().y), Align2::LEFT_CENTER, label, theme::font_caption_medium(), theme::TEXT_SECONDARY);
}

fn auto_fix_empty_row(ui: &mut Ui, msg: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(PANEL_W, GROUP_H), Sense::hover());
    widgets::record(ui, "check-designs-auto-fix-empty", rect);
    let p = ui.painter();
    p.text(Pos2::new(rect.left() + PAD, rect.center().y), Align2::LEFT_CENTER, msg, theme::font_body(), theme::TEXT_SECONDARY);
    widgets::hline(p, rect.left() + PAD, rect.right() - PAD, rect.bottom() - ONE, theme::BORDER_SUBTLE);
}

fn auto_fix_diff_row(ui: &mut Ui, doc: &designos_doc::Document, idx: usize, fix_row: &FixRow, applied: bool) -> Option<Act> {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(PANEL_W, GROUP_H), Sense::hover());
    widgets::record(ui, &format!("check-designs-auto-fix-row-{idx}"), rect);
    if idx == 0 {
        widgets::record(ui, "check-designs-auto-fix-row", rect);
    }
    let row_resp = ui.interact(rect, Id::new(("check-designs-auto-fix-row", idx)), Sense::click());
    let p = ui.painter().clone();
    if row_resp.hovered() {
        p.rect_filled(rect, 0.0, theme::BG_HOVER);
    }

    let node_name = doc.get(fix_row.node).map_or_else(
        || {
            if fix_row.node_name.is_empty() {
                format!("#{}", fix_row.node.0)
            } else {
                fix_row.node_name.clone()
            }
        },
        |n| n.name.clone(),
    );
    let label = format!("{} · {node_name}", fix_row.rule);
    let elided_label = widgets::elide(&p, &label, &theme::font_caption_medium(), LABEL_MAX_W);
    p.text(Pos2::new(rect.left() + PAD, rect.center().y), Align2::LEFT_CENTER, elided_label, theme::font_caption_medium(), theme::TEXT);

    let before_str = format_fix_value(&fix_row.before);
    let after_str = format_fix_value(&fix_row.after);
    let diff_str = format!("{before_str} → {after_str}");
    let elided_diff = widgets::elide(&p, &diff_str, &theme::font_caption(), DIFF_MAX_W);
    let diff_right = if applied { rect.right() - PAD - VALUE_COL_RIGHT_PAD } else { rect.right() - PAD };
    p.text(Pos2::new(diff_right, rect.center().y), Align2::RIGHT_CENTER, elided_diff, theme::font_caption(), theme::TEXT_SECONDARY);

    let mut act = row_resp.clicked().then_some(Act::Focus(fix_row.node));

    if applied {
        let can_revert = can_revert_fix_row(doc, fix_row);
        let btn = Rect::from_min_size(Pos2::new(rect.right() - PAD - ICON_COL, rect.center().y - ICON_SM), Vec2::new(ICON_COL, ICON_COL));
        widgets::record(ui, &format!("check-designs-auto-fix-revert-{idx}"), btn);
        if idx == 0 {
            widgets::record(ui, "check-designs-auto-fix-revert", btn);
        }
        let btn_resp = ui.interact(btn, Id::new(("check-designs-auto-fix-revert", idx)), Sense::click());
        if btn_resp.hovered() && can_revert {
            p.rect_filled(btn, theme::cr(theme::RADIUS_MARK), theme::BG_TERTIARY);
        }
        let tint = if can_revert { theme::TEXT } else { theme::TEXT_DISABLED };
        icons::paint(&p, Icon::Rotate90, btn.center(), ICON_SM, tint);
        let tip = if can_revert { "Revert this fix" } else { "Changed since the fix" };
        if btn_resp.on_hover_text(tip).clicked() && can_revert {
            act = Some(Act::RevertAutoFix(idx));
        }
    }

    widgets::hline(&p, rect.left() + PAD, rect.right() - PAD, rect.bottom() - ONE, theme::BORDER_SUBTLE);
    act
}

fn auto_fix_more_footer(ui: &mut Ui, extra: usize) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(PANEL_W, BTN_H), Sense::hover());
    widgets::record(ui, "check-designs-auto-fix-more", rect);
    let p = ui.painter();
    let msg = format!("+{extra} more");
    p.text(Pos2::new(rect.left() + PAD, rect.center().y), Align2::LEFT_CENTER, msg, theme::font_caption(), theme::TEXT_SECONDARY);
}

fn auto_fix_settings_header(ui: &mut Ui, settings_open: bool, mode: AutoFixMode) -> bool {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(PANEL_W, GROUP_H), Sense::hover());
    widgets::record(ui, "check-designs-auto-fix-settings-toggle", rect);
    let resp = ui.interact(rect, Id::new("check-designs-auto-fix-settings-toggle"), Sense::click());
    let p = ui.painter().clone();
    if resp.hovered() {
        p.rect_filled(rect, 0.0, theme::BG_HOVER);
    }
    let chev = if settings_open { Icon::ChevronDown } else { Icon::ChevronRight };
    icons::paint(&p, chev, Pos2::new(rect.left() + PAD + CHEVRON_OFFSET, rect.center().y), ICON_SM, theme::TEXT_SECONDARY);
    p.text(
        Pos2::new(rect.left() + PAD + TITLE_AFTER_CHEVRON, rect.center().y),
        Align2::LEFT_CENTER,
        "Auto-fix settings",
        theme::font_body_medium(),
        theme::TEXT,
    );
    let pill = Rect::from_center_size(Pos2::new(rect.right() - PAD - STATUS_PILL_OFFSET, rect.center().y), Vec2::new(STATUS_PILL_W, STATUS_PILL_H));
    p.rect_filled(pill, theme::cr(theme::RADIUS_MARK), theme::BG_TERTIARY);
    p.text(pill.center(), Align2::CENTER_CENTER, mode_label(mode), theme::font_caption(), theme::TEXT_SECONDARY);
    resp.clicked()
}

fn auto_fix_unreadable_notice(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(PANEL_W, BTN_H), Sense::hover());
    widgets::record(ui, "check-designs-auto-fix-settings-notice", rect);
    let p = ui.painter();
    p.text(
        Pos2::new(rect.left() + PAD, rect.center().y),
        Align2::LEFT_CENTER,
        "Couldn't read settings.json — using defaults.",
        theme::font_caption(),
        STATUS_WARN_TEXT,
    );
}

fn auto_fix_scope_label(ui: &mut Ui, title: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(PANEL_W, BTN_H), Sense::hover());
    let p = ui.painter();
    p.text(Pos2::new(rect.left() + PAD, rect.center().y), Align2::LEFT_CENTER, title, theme::font_caption_medium(), theme::TEXT_SECONDARY);
}

fn auto_fix_mode_segmented(ui: &mut Ui, scope: &str, active: AutoFixMode, alias_default_ids: bool) -> Option<AutoFixMode> {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(PANEL_W, GROUP_H), Sense::hover());
    let track = Rect::from_min_max(Pos2::new(rect.left() + PAD, rect.top() + FACT_GAP), Pos2::new(rect.right() - PAD, rect.bottom() - FACT_GAP));
    ui.painter().rect_filled(track, theme::cr(theme::RADIUS_CONTROL), theme::BG_TERTIARY);
    let inner = track.shrink(SEG_INSET);
    let seg_w = inner.width() / SEG_COUNT;
    let modes = [(AutoFixMode::On, "on"), (AutoFixMode::ReportOnly, "report-only"), (AutoFixMode::Off, "off")];
    let mut chosen = None;
    for (i, (mode, slug)) in modes.iter().enumerate() {
        let x0 = inner.left() + (i as f32) * seg_w;
        let seg_rect = Rect::from_min_size(Pos2::new(x0, inner.top()), Vec2::new(seg_w, inner.height()));
        if paint_mode_segment(ui, scope, slug, *mode, active == *mode, seg_rect, alias_default_ids) {
            chosen = Some(*mode);
        }
    }
    chosen
}

fn paint_mode_segment(ui: &mut Ui, scope: &str, slug: &str, mode: AutoFixMode, is_active: bool, seg_rect: Rect, alias_default_ids: bool) -> bool {
    let id_str = format!("check-designs-auto-fix-{scope}-mode-{slug}");
    widgets::record(ui, &id_str, seg_rect);
    if alias_default_ids {
        widgets::record(ui, &format!("check-designs-auto-fix-mode-{slug}"), seg_rect);
    }
    let resp = ui.interact(seg_rect, Id::new(id_str), Sense::click());
    let p = ui.painter();
    if is_active {
        p.rect_filled(seg_rect, theme::cr(theme::RADIUS_SEGMENT), theme::BG_ELEVATED);
    }
    let fg = if is_active { theme::TEXT } else { theme::TEXT_SECONDARY };
    p.text(seg_rect.center(), Align2::CENTER_CENTER, mode_label(mode), theme::font_caption_medium(), fg);
    resp.clicked() && !is_active
}

fn auto_fix_rule_checkbox(ui: &mut Ui, scope: &str, rule: &'static str, enabled: bool, alias_default_ids: bool) -> Option<bool> {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(PANEL_W, BTN_H), Sense::hover());
    let id_str = format!("check-designs-auto-fix-{scope}-rule-{rule}");
    widgets::record(ui, &id_str, rect);
    if alias_default_ids {
        widgets::record(ui, &format!("check-designs-auto-fix-rule-{rule}"), rect);
    }
    let resp = ui.interact(rect, Id::new(id_str), Sense::click());
    let p = ui.painter().clone();
    if resp.hovered() {
        p.rect_filled(rect, 0.0, theme::BG_HOVER);
    }
    let box_rect = Rect::from_center_size(Pos2::new(rect.left() + PAD + CHEVRON_OFFSET, rect.center().y), Vec2::new(CHECK_BOX_SIZE, CHECK_BOX_SIZE));
    let fill = if enabled { theme::BG_BRAND } else { theme::BG_TERTIARY };
    p.rect_filled(box_rect, theme::cr(theme::RADIUS_MARK), fill);
    if enabled {
        icons::paint(&p, Icon::Check, box_rect.center(), ICON_SM, Color32::WHITE);
    }
    p.text(Pos2::new(rect.left() + PAD + CHECK_TEXT_OFFSET, rect.center().y), Align2::LEFT_CENTER, rule, theme::font_caption(), theme::TEXT);
    resp.clicked().then_some(!enabled)
}

fn format_fix_value(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                if (f.fract()).abs() < f64::EPSILON {
                    format!("{}px", f as i64)
                } else {
                    format!("{f:.1}px")
                }
            } else {
                n.to_string()
            }
        }
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "none".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The message column starts at `PAD + ICON_COL` and must end before the leftmost action button.
    #[test]
    fn a_wrapped_message_never_runs_under_the_row_buttons() {
        for has_fix in [false, true] {
            let message_right = PAD + ICON_COL + message_width(has_fix);
            let buttons_left = PANEL_W - PAD - actions_width(has_fix);
            assert!(message_right <= buttons_left, "has_fix={has_fix}: message ends at {message_right}, buttons start at {buttons_left}");
        }
    }
}
