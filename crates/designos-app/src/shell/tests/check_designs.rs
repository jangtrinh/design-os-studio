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

#[test]
fn the_panel_lists_every_finding_grouped_with_a_fix_button_each() {
    let s = sim();
    let r = report(&s);
    assert_eq!(r.findings.len(), 7, "{:#?}", r.findings);
    assert!(s.app().check_designs.open);
    assert_eq!(s.app().check_designs.report, r, "the panel shows the audit of the page");
    let panel = s.rect_of("check-designs-panel");
    for (i, f) in s.app().check_designs.report.findings.iter().enumerate() {
        let row = s.rect_of(&format!("check-designs-row-{i}"));
        assert!(panel.contains_rect(row), "row {i} ({}) sits inside the panel", f.node_name);
        let fix = s.rect_of(&format!("check-designs-fix-{i}"));
        assert!(fix.width() >= 24.0 && fix.height() >= 24.0, "Fix button {i} meets the 24px floor");
    }
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
    assert_eq!(report(&s).findings.len(), 6);
    assert!(report(&s).findings.iter().all(|f| f != &first), "the fixed finding is gone");
    assert_ne!(s.app().doc.get(first.node).cloned(), before);
    s.frame(vec![]);
    assert_eq!(s.app().check_designs.report.findings.len(), 6, "the panel refreshed itself");
    s.cmd_key(Key::Z);
    assert_eq!(report(&s).findings.len(), 7);
    assert_eq!(s.app().doc.get(first.node).cloned(), before);
    assert!(!s.app().hist.can_undo(), "exactly one undo step");
}

#[test]
fn fix_all_clears_the_list_in_one_undo_step() {
    let mut s = sim();
    let nodes: Vec<NodeId> = s.app().doc.nodes.keys().copied().collect();
    let before: Vec<_> = nodes.iter().map(|id| s.app().doc.get(*id).cloned()).collect();
    let fix_all = s.rect_of("check-designs-fix-all");
    s.click(fix_all.center());
    assert!(report(&s).findings.is_empty(), "{:#?}", report(&s).findings);
    s.frame(vec![]);
    assert!(s.app().check_designs.report.findings.is_empty());
    s.cmd_key(Key::Z);
    assert_eq!(report(&s).findings.len(), 7);
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
    assert_eq!(s.app().check_designs.report.findings.len(), 7, "the scope was fixed when the panel opened");
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
