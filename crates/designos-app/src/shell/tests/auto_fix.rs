//! End-to-end tests for E15-14: Auto-fix settings (On / Report only / Off, per-rule toggles)
//! and the Auto-fix diff report in the Check designs panel + Agents panel Review link.

use super::*;
use crate::check_designs::{can_revert_fix_row, FixRow};
use designos_doc::live_patch::{live_ops_for_command, LivePatch};
use designos_doc::{AutoFixMode, AutoFixSettings, Command, Node};

fn check_designs_sim() -> Sim {
    Sim::with(Options { panel: Some("check-designs".into()), ..Options::default() })
}

fn update_ops(doc: &designos_doc::Document, nodes: Vec<Node>) -> Vec<designos_doc::live_patch::LiveOp> {
    let cmd = if nodes.len() == 1 {
        Command::Update { node: Box::new(nodes.into_iter().next().unwrap()) }
    } else {
        Command::Batch(nodes.into_iter().map(|n| Command::Update { node: Box::new(n) }).collect())
    };
    live_ops_for_command(doc, &cmd).expect("live ops")
}

#[test]
fn report_only_stops_changes_and_lists() {
    let mut s = check_designs_sim();
    let hero = s.by_name("Hero");
    let orig_w = s.app().doc.get(hero).unwrap().size.x;

    // Open Auto-fix settings and switch "This file" mode to "Report only".
    let toggle = s.rect_of("check-designs-auto-fix-settings-toggle");
    s.click(toggle.center());
    s.frame(vec![]);
    let report_only_btn = s.rect_of("check-designs-auto-fix-mode-report-only");
    s.click(report_only_btn.center());
    assert_eq!(s.app().effective_auto_fix().mode, AutoFixMode::ReportOnly);
    assert_eq!(s.app().doc.auto_fix.as_ref().unwrap().mode, AutoFixMode::ReportOnly);

    // Run a report-only agent pass: ops is empty, auto_fix_report lists 1 available fix.
    let fix_row = FixRow {
        node: hero,
        node_name: "Hero".to_string(),
        property: "width".to_string(),
        before: serde_json::json!(orig_w),
        after: serde_json::json!(orig_w + 4.0),
        rule: "snap-8pt-grid".to_string(),
    };
    let patch = LivePatch::new(vec![]).with_auto_fix_report(vec![fix_row.clone()]);
    crate::live_patch_apply::apply_live_patches(&mut s.shell.app, &[patch]);
    s.frame(vec![]);

    // 1. Node's value did not change.
    assert_eq!(s.app().doc.get(hero).unwrap().size.x, orig_w, "Report only must not mutate the document");

    // 2. Report lists the fix as report_only ("Fixes available").
    let last = s.app().check_designs.last_auto_fix.as_ref().expect("last_auto_fix recorded");
    assert!(last.report_only, "Report-only pass must record report_only = true");
    assert_eq!(last.rows, vec![fix_row]);
    let row_rect = s.rect_of("check-designs-auto-fix-row-0");
    assert!(row_rect.width() > 0.0);

    // 3. Agents panel notice records (1, true) ("1 fix available · Review") and clicking Review opens Check designs.
    assert_eq!(s.app().terminal.auto_fix_notice, Some((1, true)));
    s.shell.app.check_designs.open = false;
    s.shell.app.ui.rail = Some(crate::app::RailItem::Agents);
    s.shell.app.ui.left_collapsed = false;
    s.frame(vec![]);
    let review_btn = s.rect_of("agents-auto-fix-review");
    s.click(review_btn.center());
    assert!(s.app().check_designs.open, "Clicking Review in Agents panel opens Check designs");
    let dir = s.app().terminal.session_dir.clone();
    s.shell.app.terminal.shutdown();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn row_click_selects_and_zooms() {
    let mut s = check_designs_sim();
    let card = s.by_name("Card");
    let orig_w = s.app().doc.get(card).unwrap().size.x;

    // Pan/zoom away first so we can observe fit_selection framing the node.
    s.shell.app.selection.clear();
    s.shell.app.view.offset = DV::new(-2000.0, -2000.0);
    s.shell.app.view.zoom = 0.25;

    let mut updated = s.app().doc.get(card).unwrap().clone();
    updated.size.x = orig_w + 8.0;
    let ops = update_ops(&s.app().doc, vec![updated]);
    let fix_row = FixRow {
        node: card,
        node_name: "Card".to_string(),
        property: "width".to_string(),
        before: serde_json::json!(orig_w),
        after: serde_json::json!(orig_w + 8.0),
        rule: "snap-8pt-grid".to_string(),
    };
    let patch = LivePatch::new(ops).with_label("Auto-fix (1)").with_auto_fix_report(vec![fix_row]);
    crate::live_patch_apply::apply_live_patches(&mut s.shell.app, &[patch]);
    s.frame(vec![]);

    let row_rect = s.rect_of("check-designs-auto-fix-row");
    // Click the left label area of the diff row (away from the Revert icon on the right).
    s.click(Pos2::new(row_rect.left() + 24.0, row_rect.center().y));
    assert_eq!(s.app().selection, vec![card], "Clicking a diff row selects its node");
    assert!(
        (s.app().view.offset.x - (-2000.0)).abs() > 1.0 || (s.app().view.zoom - 0.25).abs() > 0.01,
        "Clicking a diff row frames the selected node on canvas"
    );
}

#[test]
fn settings_persist_across_restart() {
    let tmp = std::env::temp_dir().join(format!("designos_autofix_persist_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let mut s = check_designs_sim();
    s.shell.app.attach_library(tmp.clone());
    s.frame(vec![]);

    // Expand Auto-fix settings.
    let toggle = s.rect_of("check-designs-auto-fix-settings-toggle");
    s.click(toggle.center());
    s.frame(vec![]);

    // Toggle "sizing-conflict" off and switch mode to "Report only" for This file.
    let file_rule = s.rect_of("check-designs-auto-fix-file-rule-sizing-conflict");
    s.click(file_rule.center());
    s.frame(vec![]);
    let file_mode = s.rect_of("check-designs-auto-fix-file-mode-report-only");
    s.click(file_mode.center());
    s.frame(vec![]);

    // Toggle "hardcoded-value" off and switch mode to "Off" for My default (user settings.json).
    let user_rule = s.rect_of("check-designs-auto-fix-user-rule-hardcoded-value");
    s.click(user_rule.center());
    s.frame(vec![]);
    let user_mode = s.rect_of("check-designs-auto-fix-user-mode-off");
    s.click(user_mode.center());
    s.frame(vec![]);

    // Save the .designos file to disk and reload it.
    let doc_path = tmp.join("project.designos");
    let saved_path = crate::io::save_path(&s.app().doc, &doc_path).unwrap();
    let reloaded_doc = crate::io::load_path(&saved_path).unwrap();
    let doc_af = reloaded_doc.auto_fix.expect("file auto_fix persisted in .designos");
    assert_eq!(doc_af.mode, AutoFixMode::ReportOnly);
    assert!(!doc_af.rule_enabled("sizing-conflict"));

    // Verify user settings.json persisted across a fresh App attach_library.
    let mut s2 = check_designs_sim();
    s2.shell.app.attach_library(tmp.clone());
    assert_eq!(s2.app().check_designs.user_auto_fix.mode, AutoFixMode::Off);
    assert!(!s2.app().check_designs.user_auto_fix.rule_enabled("hardcoded-value"));
    assert!(!s2.app().check_designs.settings_unreadable);

    // Corrupt settings.json and verify the inline notice is rendered when settings are open.
    std::fs::write(tmp.join("settings.json"), b"{invalid json").unwrap();
    let mut s3 = check_designs_sim();
    s3.shell.app.attach_library(tmp.clone());
    assert!(s3.app().check_designs.settings_unreadable);
    assert_eq!(s3.app().check_designs.user_auto_fix, AutoFixSettings::default());
    s3.shell.app.check_designs.settings_open = true;
    s3.frame(vec![]);
    let notice = s3.rect_of("check-designs-auto-fix-settings-notice");
    assert!(notice.width() > 0.0);

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn revert_one_fix_is_one_undo_step() {
    let mut s = check_designs_sim();
    let hero = s.by_name("Hero");
    let card = s.by_name("Card");
    let hero_w0 = s.app().doc.get(hero).unwrap().size.x;
    let card_w0 = s.app().doc.get(card).unwrap().size.x;

    let mut hero_after = s.app().doc.get(hero).unwrap().clone();
    hero_after.size.x = hero_w0 + 16.0;
    let mut card_after = s.app().doc.get(card).unwrap().clone();
    card_after.size.x = card_w0 + 24.0;
    let ops = update_ops(&s.app().doc, vec![hero_after, card_after]);

    let row0 = FixRow {
        node: hero,
        node_name: "Hero".to_string(),
        property: "width".to_string(),
        before: serde_json::json!(hero_w0),
        after: serde_json::json!(hero_w0 + 16.0),
        rule: "snap-8pt-grid".to_string(),
    };
    let row1 = FixRow {
        node: card,
        node_name: "Card".to_string(),
        property: "width".to_string(),
        before: serde_json::json!(card_w0),
        after: serde_json::json!(card_w0 + 24.0),
        rule: "snap-8pt-grid".to_string(),
    };
    let patch = LivePatch::new(ops).with_label("Auto-fix (2)").with_auto_fix_report(vec![row0, row1]);

    crate::live_patch_apply::apply_live_patches(&mut s.shell.app, &[patch]);
    s.frame(vec![]);

    assert_eq!(s.app().hist.peek_undo_label(), Some("Auto-fix (2)"));
    assert_eq!(s.app().doc.get(hero).unwrap().size.x, hero_w0 + 16.0);
    assert_eq!(s.app().doc.get(card).unwrap().size.x, card_w0 + 24.0);

    // Click Revert on row 0 (Hero).
    let revert0 = s.rect_of("check-designs-auto-fix-revert-0");
    s.click(revert0.center());

    // Only Hero returns to before; Card keeps its auto-fixed width.
    assert_eq!(s.app().doc.get(hero).unwrap().size.x, hero_w0, "Reverting row 0 restores Hero's width");
    assert_eq!(s.app().doc.get(card).unwrap().size.x, card_w0 + 24.0, "Card's auto-fix remains intact");

    // One Cmd+Z undoes the single-row revert and re-applies Hero's fix.
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(hero).unwrap().size.x, hero_w0 + 16.0, "Cmd+Z re-applies the reverted fix");
    assert_eq!(s.app().doc.get(card).unwrap().size.x, card_w0 + 24.0);
}

#[test]
fn revert_disabled_after_drift() {
    let mut s = check_designs_sim();
    let hero = s.by_name("Hero");
    let hero_w0 = s.app().doc.get(hero).unwrap().size.x;

    let mut hero_after = s.app().doc.get(hero).unwrap().clone();
    hero_after.size.x = hero_w0 + 16.0;
    let ops = update_ops(&s.app().doc, vec![hero_after]);
    let row0 = FixRow {
        node: hero,
        node_name: "Hero".to_string(),
        property: "width".to_string(),
        before: serde_json::json!(hero_w0),
        after: serde_json::json!(hero_w0 + 16.0),
        rule: "snap-8pt-grid".to_string(),
    };
    let patch = LivePatch::new(ops).with_label("Auto-fix (1)").with_auto_fix_report(vec![row0.clone()]);
    crate::live_patch_apply::apply_live_patches(&mut s.shell.app, &[patch]);
    s.frame(vec![]);

    assert!(can_revert_fix_row(&s.app().doc, &row0), "Precondition: Revert is enabled before drift");

    // Manually edit the same property (width) on Hero to a different value.
    s.shell.app.edit_nodes(&[hero], |n| {
        n.size.x = hero_w0 + 40.0;
    });
    s.frame(vec![]);

    assert!(!can_revert_fix_row(&s.app().doc, &row0), "Revert must be disabled after the field drifted");

    // Clicking the disabled Revert button leaves the drifted width untouched.
    let revert0 = s.rect_of("check-designs-auto-fix-revert-0");
    s.click(revert0.center());
    assert_eq!(s.app().doc.get(hero).unwrap().size.x, hero_w0 + 40.0, "Disabled Revert must not overwrite drifted value");
}

#[test]
fn zero_fake_ui_gate() {
    let mut s = check_designs_sim();
    let hero = s.by_name("Hero");
    let hero_w0 = s.app().doc.get(hero).unwrap().size.x;

    // Seed an applied auto-fix pass so the diff row and revert button are rendered.
    let mut hero_after = s.app().doc.get(hero).unwrap().clone();
    hero_after.size.x = hero_w0 + 8.0;
    let ops = update_ops(&s.app().doc, vec![hero_after]);
    let row0 = FixRow {
        node: hero,
        node_name: "Hero".to_string(),
        property: "width".to_string(),
        before: serde_json::json!(hero_w0),
        after: serde_json::json!(hero_w0 + 8.0),
        rule: "snap-8pt-grid".to_string(),
    };
    let patch = LivePatch::new(ops).with_label("Auto-fix (1)").with_auto_fix_report(vec![row0]);
    crate::live_patch_apply::apply_live_patches(&mut s.shell.app, &[patch]);
    s.frame(vec![]);

    // 1. Header and count badge are recorded.
    assert!(s.rect_of("check-designs-auto-fix-header").width() > 0.0);
    assert!(s.rect_of("check-designs-auto-fix-count").width() > 0.0);

    // 2. Clicking the diff row selects the target node.
    s.shell.app.selection.clear();
    let row_rect = s.rect_of("check-designs-auto-fix-row");
    s.click(Pos2::new(row_rect.left() + 24.0, row_rect.center().y));
    assert_eq!(s.app().selection, vec![hero]);

    // 3. Clicking the Revert button reverts the fix on the document.
    let revert_rect = s.rect_of("check-designs-auto-fix-revert");
    s.click(revert_rect.center());
    assert_eq!(s.app().doc.get(hero).unwrap().size.x, hero_w0);

    // 4. Clicking the settings toggle opens the settings section.
    assert!(!s.app().check_designs.settings_open);
    let settings_toggle = s.rect_of("check-designs-auto-fix-settings-toggle");
    s.click(settings_toggle.center());
    assert!(s.app().check_designs.settings_open);
    s.frame(vec![]);

    // 5. Every mode segment ("on", "report-only", "off") and rule checkbox mutates real state.
    let mode_report = s.rect_of("check-designs-auto-fix-mode-report-only");
    s.click(mode_report.center());
    assert_eq!(s.app().effective_auto_fix().mode, AutoFixMode::ReportOnly);
    s.frame(vec![]);

    let mode_off = s.rect_of("check-designs-auto-fix-mode-off");
    s.click(mode_off.center());
    assert_eq!(s.app().effective_auto_fix().mode, AutoFixMode::Off);
    s.frame(vec![]);

    let mode_on = s.rect_of("check-designs-auto-fix-mode-on");
    s.click(mode_on.center());
    assert_eq!(s.app().effective_auto_fix().mode, AutoFixMode::On);
    s.frame(vec![]);

    for rule in designos_doc::SAFE_AUTO_FIX_RULES {
        let chk = s.rect_of(&format!("check-designs-auto-fix-rule-{rule}"));
        s.click(chk.center());
        assert!(!s.app().effective_auto_fix().rule_enabled(rule), "toggling {rule} off");
        s.frame(vec![]);
        let chk = s.rect_of(&format!("check-designs-auto-fix-rule-{rule}"));
        s.click(chk.center());
        assert!(s.app().effective_auto_fix().rule_enabled(rule), "toggling {rule} back on");
        s.frame(vec![]);
    }
}
