//! A lint run cut short by a time budget and resumed by its cursor always moves forward, and the
//! pieces add up to the run made in one go: what the contrast rule needs around a text is rebuilt
//! by every call, so a call that ended before scanning anything would be resumed from the same place.

use super::*;
use crate::{Color, Document, Node, NodeId, Paint, Rect, TextStyle};
use std::time::Duration;

/// Shorter than the time it takes to ready the contrast rule around the first text of the page.
const TINY: Duration = Duration::from_micros(50);

/// A page of `cards` frames, each with a pale text on white (a contrast finding) and a rectangle.
fn wide_page(cards: usize) -> (Document, NodeId) {
    let mut doc = Document::new("wide");
    let page = doc.add_page("Page");
    for i in 0..cards {
        let id = doc.alloc_id();
        let card = doc.insert(page, Node::frame(id, Rect::new((i % 40) as f32 * 120.0, (i / 40) as f32 * 80.0, 100.0, 60.0))).unwrap();
        let id = doc.alloc_id();
        let style = TextStyle { font_size: 14.0, fills: vec![Paint::solid(Color::rgb(0xEE, 0xEE, 0xEE))], ..Default::default() };
        doc.insert(card, Node::text(id, Rect::new(8.0, 8.0, 40.0, 16.0), "Pale", style)).unwrap();
        let id = doc.alloc_id();
        doc.insert(card, Node::rectangle(id, Rect::new(8.0, 30.0, 40.0, 16.0))).unwrap();
    }
    (doc, page)
}

fn sorted(mut findings: Vec<Finding>) -> Vec<Finding> {
    findings.sort_by(|a, b| (a.node, a.rule).cmp(&(b.node, b.rule)));
    findings
}

#[test]
fn calls_with_a_tiny_time_budget_resumed_by_cursor_reach_the_end_with_the_same_findings() {
    let (doc, page) = wide_page(200);
    let config = LintConfig::from_preset(LintPreset::Agent);
    let whole = lint(&doc, &config, &ApproxLintEnv);
    assert!(whole.findings.iter().any(|f| f.rule == "contrast"), "the page must produce contrast findings");

    let mut cursor: Option<String> = None;
    let (mut scanned, mut findings, mut calls) = (0usize, Vec::new(), 0usize);
    loop {
        calls += 1;
        assert!(calls < 3_000, "the calls do not reach the end");
        let mut budget = Budget::with_limits(usize::MAX, Some(TINY));
        budget.cursor = cursor.clone();
        let stats = lint_in_slices_with_budget(&doc, &[page], &config, &ApproxLintEnv, 512, TINY, &mut budget);
        assert!(stats.report.scanned > 0, "call {calls} scanned nothing from {cursor:?}");
        scanned += stats.report.scanned;
        findings.extend(stats.report.findings);
        if stats.report.complete {
            break;
        }
        let next = stats.report.cursor.clone();
        assert!(next.is_some() && next != cursor, "call {calls} did not move the cursor from {cursor:?}");
        cursor = next;
    }
    assert_eq!(scanned, whole.scanned);
    assert_eq!(sorted(findings), sorted(whole.findings));
}

#[test]
fn a_single_slice_call_with_a_time_budget_still_scans_a_node() {
    // The first layer is a text with a thousand siblings to ready the contrast rule around.
    let mut doc = Document::new("flat");
    let page = doc.add_page("Page");
    let id = doc.alloc_id();
    let style = TextStyle { font_size: 14.0, fills: vec![Paint::solid(Color::rgb(0xEE, 0xEE, 0xEE))], ..Default::default() };
    doc.insert(page, Node::text(id, Rect::new(0.0, 0.0, 40.0, 16.0), "Pale", style)).unwrap();
    for i in 0..1000 {
        let id = doc.alloc_id();
        doc.insert(page, Node::rectangle(id, Rect::new(i as f32 * 20.0, 100.0, 10.0, 10.0))).unwrap();
    }
    let config = LintConfig::from_preset(LintPreset::Agent);
    let mut budget = Budget::with_limits(usize::MAX, Some(Duration::ZERO));
    let report = lint_subtree(&doc, &[page], &config, &ApproxLintEnv, &mut budget);
    assert!(report.scanned > 0);
}
