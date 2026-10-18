//! Annotation callouts and status badges drawn over the canvas in Dev mode.

use super::data;
use super::sections::{status_color, status_pill};
use super::AnnotEdit;
use crate::app::App;
use crate::panels::inspect::page_aabb;
use crate::theme;
use crate::widgets::{elide, text_width};
use designos_doc::*;
use egui::{Align2, Id, Pos2, Rect, Sense, Stroke, Ui, Vec2};

/// Layout of one callout, in screen space.
#[derive(Clone, Debug, PartialEq)]
pub struct Callout {
    pub rect: Rect,
    /// Point on the layer the connector starts at.
    pub anchor: Pos2,
    /// Point on the label the connector ends at.
    pub attach: Pos2,
}

/// Places a label of `size` next to the layer rectangle `node` inside `view`, away from `taken`.
/// Right of the layer by default, left when there is no room; shifted down to avoid other labels.
pub fn place(node: Rect, size: Vec2, view: Rect, taken: &[Rect]) -> Callout {
    const GAP: f32 = 32.0;
    let mut x = node.right() + GAP;
    let mut on_right = true;
    if x + size.x > view.right() - 8.0 && node.left() - GAP - size.x >= view.left() + 8.0 {
        x = node.left() - GAP - size.x;
        on_right = false;
    }
    x = x.clamp(view.left() + 8.0, (view.right() - 8.0 - size.x).max(view.left() + 8.0));
    let mut y = node.top().clamp(view.top() + 8.0, (view.bottom() - 8.0 - size.y).max(view.top() + 8.0));
    for _ in 0..64 {
        let r = Rect::from_min_size(Pos2::new(x, y), size);
        match taken.iter().find(|t| t.expand(4.0).intersects(r)) {
            Some(t) => y = t.bottom() + 8.0,
            None => break,
        }
    }
    let rect = Rect::from_min_size(Pos2::new(x, y), size);
    let cy = rect.center().y.clamp(node.top().min(node.bottom()), node.bottom().max(node.top()));
    let (anchor, attach) = if on_right {
        (Pos2::new(node.right(), cy), Pos2::new(rect.left(), rect.center().y))
    } else {
        (Pos2::new(node.left(), cy), Pos2::new(rect.right(), rect.center().y))
    };
    Callout { rect, anchor, attach }
}

fn label_lines(app: &App, id: NodeId, a: &Annotation) -> Vec<(String, Option<String>)> {
    let mut lines = vec![(if a.text.is_empty() { "Annotation".to_string() } else { a.text.clone() }, None)];
    if let Some(n) = app.doc.get(id) {
        for l in &a.properties {
            if let Some(v) = data::property_value(&app.doc, n, l) {
                lines.push((l.clone(), Some(v)));
            }
        }
    }
    lines
}

/// Draws the callouts and badges of the current page. Call after the other canvas overlays.
pub fn overlay(app: &mut App, ui: &Ui, rect: Rect) {
    if !app.ui.dev_mode {
        return;
    }
    let mut ids: Vec<NodeId> = Vec::new();
    app.doc.walk(app.page, |n, _| {
        if n.visible && (n.dev.status.is_some() || !n.dev.annotations.is_empty()) {
            ids.push(n.id);
        }
    });
    if ids.is_empty() {
        return;
    }
    let p = ui.painter().with_clip_rect(rect);
    let mut taken: Vec<Rect> = Vec::new();
    let mut select: Option<(NodeId, Option<usize>)> = None;
    let small = theme::font_canvas();
    let strong = theme::font_canvas_medium();
    for id in ids {
        let Some(b) = page_aabb(&app.doc, id) else { continue };
        let a = app.view.to_screen(rect.min, designos_doc::Vec2::new(b.left(), b.top()));
        let z = app.view.to_screen(rect.min, designos_doc::Vec2::new(b.right(), b.bottom()));
        let sr = Rect::from_two_pos(a, z);
        if !sr.intersects(rect) {
            continue;
        }
        let Some(n) = app.doc.get(id) else { continue };
        if let Some(s) = n.dev.status {
            let at = Pos2::new(sr.left(), sr.top() - 24.0);
            let w = text_width(&p, s.label(), &theme::font_canvas_medium()) + 26.0;
            let pill = Rect::from_min_size(at, Vec2::new(w, 20.0));
            if rect.intersects(pill) {
                p.rect_filled(pill, theme::cr(10), theme::BG);
                status_pill(&p, at, s);
                p.rect_stroke(pill, theme::cr(10), Stroke::new(1.0, status_color(s)), egui::StrokeKind::Inside);
                let resp = ui.interact(pill, Id::new(("dev-badge", id.0)), Sense::click());
                if resp.clicked() {
                    select = Some((id, None));
                }
            }
        }
        let annotations = n.dev.annotations.clone();
        for (i, an) in annotations.iter().enumerate() {
            let lines = label_lines(app, id, an);
            let mut w: f32 = 0.0;
            for (k, (l, v)) in lines.iter().enumerate() {
                let tw = match v {
                    Some(v) => text_width(&p, l, &small) + 12.0 + text_width(&p, v, &small),
                    None => text_width(&p, l, if k == 0 { &strong } else { &small }),
                };
                w = w.max(tw);
            }
            let size = Vec2::new((w + 20.0).clamp(80.0, 260.0), 12.0 + lines.len() as f32 * 16.0);
            let c = place(sr, size, rect, &taken);
            taken.push(c.rect);
            let selected = app.selection.contains(&id);
            p.line_segment([c.anchor, c.attach], Stroke::new(1.5, theme::ANNOTATION_LINE));
            p.circle_filled(c.anchor, 4.0, theme::ANNOTATION_LINE);
            p.circle_stroke(c.anchor, 4.0, Stroke::new(1.0, theme::ANNOTATION_TEXT));
            p.rect_filled(c.rect, theme::cr(6), theme::ANNOTATION_BG);
            if selected {
                p.rect_stroke(c.rect, theme::cr(6), Stroke::new(1.5, theme::BORDER_SELECTED), egui::StrokeKind::Outside);
            }
            let mut y = c.rect.top() + 14.0;
            for (k, (l, v)) in lines.iter().enumerate() {
                let x = c.rect.left() + 10.0;
                match v {
                    Some(v) => {
                        p.text(Pos2::new(x, y), Align2::LEFT_CENTER, l, small.clone(), theme::ANNOTATION_TEXT.gamma_multiply(0.65));
                        p.text(Pos2::new(c.rect.right() - 10.0, y), Align2::RIGHT_CENTER, elide(&p, v, &small, 140.0), small.clone(), theme::ANNOTATION_TEXT);
                    }
                    None => {
                        let font = if k == 0 { strong.clone() } else { small.clone() };
                        p.text(Pos2::new(x, y), Align2::LEFT_CENTER, elide(&p, l, &font, size.x - 20.0), font, theme::ANNOTATION_TEXT);
                    }
                }
                y += 16.0;
            }
            let resp = ui.interact(c.rect, Id::new(("dev-callout", id.0, i)), Sense::click());
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() {
                select = Some((id, Some(i)));
            }
        }
    }
    if let Some((id, edit)) = select {
        app.select_only(id);
        if let Some(i) = edit {
            if let Some(a) = app.doc.get(id).and_then(|n| n.dev.annotations.get(i)).cloned() {
                app.dev.annot = Some(AnnotEdit { node: id, index: Some(i), text: a.text, props: a.properties });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h))
    }

    #[test]
    fn callout_sits_right_of_the_layer_and_connects_to_its_edge() {
        let c = place(r(100.0, 100.0, 200.0, 100.0), Vec2::new(120.0, 40.0), r(0.0, 0.0, 900.0, 700.0), &[]);
        assert_eq!(c.rect.left(), 332.0);
        assert_eq!(c.rect.top(), 100.0);
        assert_eq!(c.anchor, Pos2::new(300.0, 120.0));
        assert_eq!(c.attach, Pos2::new(332.0, 120.0));
    }

    #[test]
    fn callout_moves_left_without_room_and_avoids_other_labels() {
        let view = r(0.0, 0.0, 500.0, 400.0);
        let c = place(r(300.0, 100.0, 150.0, 50.0), Vec2::new(120.0, 40.0), view, &[]);
        assert!(c.rect.right() < 300.0, "{:?}", c.rect);
        assert_eq!(c.anchor.x, 300.0);
        assert_eq!(c.attach.x, c.rect.right());
        let first = place(r(10.0, 100.0, 100.0, 50.0), Vec2::new(120.0, 40.0), view, &[]);
        let second = place(r(10.0, 100.0, 100.0, 50.0), Vec2::new(120.0, 40.0), view, &[first.rect]);
        assert!(!first.rect.expand(1.0).intersects(second.rect), "{:?} {:?}", first.rect, second.rect);
        assert!(second.rect.top() > first.rect.bottom());
    }

    #[test]
    fn labels_list_the_pinned_property_values() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let f = d.alloc_id();
        let f = d.insert(page, Node::frame(f, designos_doc::Rect::new(0.0, 0.0, 312.0, 200.0))).unwrap();
        let app = App::new(d);
        let a = Annotation { text: "Keep square".into(), properties: vec!["Width".into(), "Fill".into(), "Font".into()], category: None };
        let lines = label_lines(&app, f, &a);
        assert_eq!(
            lines,
            vec![("Keep square".into(), None), ("Width".into(), Some("312px".into())), ("Fill".into(), Some("#FFFFFF".into()))],
            "properties that do not apply to the layer are skipped"
        );
    }
}
