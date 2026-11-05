//! Hug column + Fill-width wrapped text takes the height measured at the solved width.

use crate::text_measure::{TextMeasure, ThreadLayoutMeasurerGuard};
use crate::{Document, NodeId, NodeKind, NodeSpec, TextAutoResize, TextProps, Vec2};
use serde_json::json;
use std::sync::Arc;

const BODY: &str = "The quick brown fox jumps over the lazy dog and keeps running across the field until nightfall.";

struct QaMeasure;

impl TextMeasure for QaMeasure {
    fn measure(&self, tp: &TextProps, wrap_width: Option<f32>) -> Vec2 {
        match wrap_width {
            None => {
                if tp.content == "Title" {
                    Vec2::new(41.0, 25.0)
                } else {
                    Vec2::new(tp.content.len() as f32 * 8.0, 17.0)
                }
            }
            Some(w) => {
                let h = if (w - 280.0).abs() < 1.0 {
                    51.0
                } else if (w - 400.0).abs() < 1.0 {
                    34.0
                } else if (w - 40.0).abs() < 1.0 {
                    322.0
                } else {
                    // 120 (W=140) and any other width fall back to the QA W=140 height.
                    102.0
                };
                Vec2::new(w, h)
            }
        }
    }
}

fn card_spec(width: f32) -> NodeSpec {
    serde_json::from_value(json!({
        "kind": "frame",
        "name": "Card",
        "width": width,
        "height": 40,
        "width_sizing": "fixed",
        "height_sizing": "hug",
        "layout": {"direction": "column", "padding": [10, 10, 10, 10], "gap": 8},
        "children": [
            {"kind": "text", "name": "Title", "text": "Title", "font_size": 20, "font_family": "Inter"},
            {"kind": "text", "name": "Body", "text": BODY, "font_size": 14, "font_family": "Inter", "width_sizing": "fill"}
        ]
    }))
    .expect("card spec")
}

fn find(doc: &Document, name: &str) -> NodeId {
    doc.nodes.values().find(|n| n.name == name).map(|n| n.id).unwrap_or_else(|| panic!("no node {name}"))
}

fn assert_near(got: f32, want: f32, label: &str) {
    assert!((got - want).abs() < 0.51, "{label}: got {got} want {want}");
}

fn build_card(width: f32) -> (ThreadLayoutMeasurerGuard, Document, NodeId, NodeId) {
    let guard = ThreadLayoutMeasurerGuard::install(Arc::new(QaMeasure));
    let mut doc = Document::new("qa-856");
    let page = doc.add_page("Page 1");
    let report = doc.apply_spec_with_measure(page, &card_spec(width), &[], Some(&QaMeasure)).expect("apply_spec");
    let card = report.root_node_id;
    let body = find(&doc, "Body");
    assert_eq!(
        doc.get(body).and_then(|n| match &n.kind {
            NodeKind::Text(t) => Some(t.auto_resize),
            _ => None,
        }),
        Some(TextAutoResize::Height)
    );
    (guard, doc, card, body)
}

#[test]
fn apply_spec_relayout_matches_qa_wrapped_heights() {
    let cases: [(f32, f32, f32); 4] = [(300.0, 51.0, 104.0), (420.0, 34.0, 87.0), (60.0, 322.0, 375.0), (140.0, 102.0, 155.0)];
    for (width, body_h, frame_h) in cases {
        let (_guard, doc, card, body) = build_card(width);
        let b = doc.get(body).unwrap();
        let f = doc.get(card).unwrap();
        assert_near(b.size.x, (width - 20.0).max(1.0), &format!("W={width} body width"));
        assert_near(b.size.y, body_h, &format!("W={width} body height"));
        assert_near(f.size.x, width, &format!("W={width} frame width"));
        assert_near(f.size.y, frame_h, &format!("W={width} frame height"));
    }
}

#[test]
fn width_write_then_relayout_matches_qa_wrapped_heights() {
    let (_guard, mut doc, card, body) = build_card(300.0);
    for (width, body_h, frame_h) in [(140.0, 102.0, 155.0), (420.0, 34.0, 87.0), (60.0, 322.0, 375.0), (300.0, 51.0, 104.0)] {
        doc.get_mut(card).unwrap().size.x = width;
        doc.relayout_subtree(card);
        doc.flush_pending_relayout();
        assert_near(doc.get(body).unwrap().size.y, body_h, &format!("relayout W={width} body height"));
        assert_near(doc.get(card).unwrap().size.y, frame_h, &format!("relayout W={width} frame height"));
    }
}
