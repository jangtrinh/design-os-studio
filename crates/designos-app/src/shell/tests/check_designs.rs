//! The Check designs panel end to end on the shipped fixture: findings are listed, one click fixes
//! one finding as one undo step, Fix all is one undo step, and a row click selects its layer.

use super::*;
use crate::check_designs::{audit, Report, Scope, FIXTURE};

fn sim() -> Sim {
    Sim::with(Options { fixture: Some(FIXTURE.into()), panel: Some("check-designs".into()), ..Options::default() })
}

fn report(s: &Sim) -> Report {
    audit(&s.app().doc, &Scope::Page(s.app().page))
}

/// Whether the panel drew a rect under `name` this frame.
fn has_rect(s: &Sim, name: &str) -> bool {
    s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", name.to_string())))).is_some()
}

#[test]
fn the_panel_lists_every_finding_grouped_with_a_fix_button_on_each_fixable_row() {
    let s = sim();
    let r = report(&s);
    assert_eq!(r.findings.len(), 8, "{:#?}", r.findings);
    assert!(s.app().check_designs.open);
    assert_eq!(s.app().check_designs.report, r, "the panel shows the audit of the page");
    let panel = s.rect_of("check-designs-panel");
    for (i, f) in s.app().check_designs.report.findings.iter().enumerate() {
        let row = s.rect_of(&format!("check-designs-row-{i}"));
        assert!(panel.contains_rect(row), "row {i} ({}) sits inside the panel", f.node_name);
        if f.fix.is_none() {
            assert!(!has_rect(&s, &format!("check-designs-fix-{i}")), "row {i} ({}) has no safe fix, so it has no Fix button", f.node_name);
            continue;
        }
        let fix = s.rect_of(&format!("check-designs-fix-{i}"));
        assert!(fix.width() >= 24.0 && fix.height() >= 24.0, "Fix button {i} meets the 24px floor");
    }
    assert_eq!(r.findings.iter().filter(|f| f.fix.is_none()).count(), 1, "the resized detached frame is the one row without a fix");
    let fix_all = s.rect_of("check-designs-fix-all");
    assert!(fix_all.width() >= 24.0 && fix_all.height() >= 24.0);
}

#[test]
fn one_fix_click_fixes_one_finding_and_undoes_in_one_step() {
    let mut s = sim();
    let first = s.app().check_designs.report.findings[0].clone();
    let before = s.app().doc.get(first.node).cloned();
    let fix = s.rect_of("check-designs-fix-0");
    s.click(fix.center());
    assert_eq!(report(&s).findings.len(), 7);
    assert!(report(&s).findings.iter().all(|f| f != &first), "the fixed finding is gone");
    assert_ne!(s.app().doc.get(first.node).cloned(), before);
    s.frame(vec![]);
    assert_eq!(s.app().check_designs.report.findings.len(), 7, "the panel refreshed itself");
    s.cmd_key(Key::Z);
    assert_eq!(report(&s).findings.len(), 8);
    assert_eq!(s.app().doc.get(first.node).cloned(), before);
    assert!(!s.app().hist.can_undo(), "exactly one undo step");
}

#[test]
fn fix_all_fixes_every_safe_finding_in_one_undo_step_and_leaves_the_rest() {
    let mut s = sim();
    let nodes: Vec<NodeId> = s.app().doc.nodes.keys().copied().collect();
    let before: Vec<_> = nodes.iter().map(|id| s.app().doc.get(*id).cloned()).collect();
    let fix_all = s.rect_of("check-designs-fix-all");
    s.click(fix_all.center());
    let left: Vec<_> = report(&s).findings.iter().map(|f| f.node_name.clone()).collect();
    assert_eq!(left, ["Cancel"], "only the finding with no safe fix is left");
    s.frame(vec![]);
    assert_eq!(s.app().check_designs.report.findings.len(), 1);
    assert!(s.app().check_designs.report.fixable().is_empty());
    s.cmd_key(Key::Z);
    assert_eq!(report(&s).findings.len(), 8);
    let after: Vec<_> = nodes.iter().map(|id| s.app().doc.get(*id).cloned()).collect();
    assert_eq!(after, before, "one undo restores every layer");
    assert!(!s.app().hist.can_undo());
}

#[test]
fn clicking_a_row_selects_its_layer_without_changing_the_list() {
    let mut s = sim();
    let target = s.app().check_designs.report.findings[2].node;
    let row = s.rect_of("check-designs-row-2");
    s.click(Pos2::new(row.left() + 40.0, row.top() + 12.0));
    assert_eq!(s.app().selection, vec![target]);
    s.frame(vec![]);
    assert_eq!(s.app().check_designs.report.findings.len(), 8, "the scope was fixed when the panel opened");
}

#[test]
fn close_hides_the_panel_and_the_actions_row_reopens_it() {
    let mut s = sim();
    let close = s.rect_of("check-designs-close");
    s.click(close.center());
    assert!(!s.app().check_designs.open);
    s.shell.app.cmdk.open = true;
    s.shell.app.cmdk.active_tab = 3;
    s.frame(vec![]);
    s.frame(vec![]);
    let row = s.rect_of("cmdk-check-designs");
    s.click(row.center());
    assert!(s.app().check_designs.open, "the Check designs action opens the panel");
    assert!(!s.app().cmdk.open, "and closes the Actions bar");
}

#[test]
fn a_clean_file_shows_the_empty_state_with_a_disabled_fix_all() {
    let mut s = Sim::with(Options { panel: Some("check-designs".into()), ..Options::default() });
    s.shell.app.open_check_designs();
    s.shell.app.check_designs.scope = Some(Scope::Layers(vec![]));
    s.shell.app.check_designs.invalidate();
    s.frame(vec![]);
    s.frame(vec![]);
    assert!(s.app().check_designs.report.findings.is_empty());
    let fix_all = s.rect_of("check-designs-fix-all");
    s.click(fix_all.center());
    assert!(!s.app().hist.can_undo(), "a disabled Fix all does nothing");
}

#[test]
fn fix_all_disabled_while_scanning() {
    let mut s = sim();
    assert_eq!(s.app().check_designs.report.findings.len(), 8);
    // Force a new incremental scan that stays in-flight by setting a 0ns slice budget and invalidating.
    s.shell.app.check_designs.invalidate();
    s.shell.app.check_designs.slice_budget = std::time::Duration::ZERO;
    s.frame(vec![]);
    assert!(s.app().check_designs.is_scanning(), "scan is still in flight after one micro-slice");
    assert!(has_rect(&s, "check-designs-progress"), "progress row is rendered while scanning");

    let fix_all = s.rect_of("check-designs-fix-all");
    s.click(fix_all.center());
    assert!(!s.app().hist.can_undo(), "Fix all is disabled while scanning and does not push an undo step");
    assert_eq!(report(&s).findings.len(), 8, "no findings were modified while scanning");
}
