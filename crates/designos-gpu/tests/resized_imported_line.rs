//! A resized imported Line keeps its stored stroke band (y in [-weight, 0]) in both renderers, whether
//! the line is resized directly or through a parent frame. The GPU half is skipped when the machine has
//! no GPU adapter; the CPU half always runs.

mod common;

use common::*;
use designos_doc::*;
use designos_render::tiny_skia::Pixmap;
use designos_render::{render_page, Viewport};

const WEIGHT: f32 = 10.0;
const VIEW: (u32, u32) = (280, 100);

fn line(t: &mut T, parent: Option<NodeId>, at: Vec2, w: f32) -> NodeId {
    t.add(parent, |id| {
        let mut n = Node::new(id, NodeKind::Line);
        n.transform = Affine::translate(at.x, at.y);
        n.size = Vec2::new(w, 0.0);
        n.fills.clear();
        n.stroke.paints = vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))];
        n.stroke.weight = WEIGHT;
        let cmds = vec![
            PathCmd::MoveTo(Vec2::new(0.0, -WEIGHT)),
            PathCmd::LineTo(Vec2::new(w, -WEIGHT)),
            PathCmd::LineTo(Vec2::new(w, 0.0)),
            PathCmd::LineTo(Vec2::new(0.0, 0.0)),
            PathCmd::Close,
        ];
        n.stroke_geometry = Some(vec![Path { cmds, fill_rule: FillRule::NonZero }]);
        n
    })
}

fn resize(doc: &mut Document, id: NodeId, size: Vec2) {
    let (old, transform) = {
        let n = doc.get(id).unwrap();
        (n.size, n.transform)
    };
    apply_constrained_geometry(doc.get_mut(id).unwrap(), transform, size);
    for (child, t, s) in plan_constraint_pass(doc, id, old, size, 64) {
        apply_constrained_geometry(doc.get_mut(child).unwrap(), t, s);
    }
}

/// Rows (min, max) holding dark ink.
fn rows(pm: &Pixmap) -> (u32, u32) {
    let (mut y0, mut y1) = (u32::MAX, 0);
    for y in 0..pm.height() {
        for x in 0..pm.width() {
            if pm.pixel(x, y).unwrap().red() < 128 {
                (y0, y1) = (y0.min(y), y1.max(y));
            }
        }
    }
    (y0, y1)
}

fn check(name: &str, t: &T) {
    let mut cpu = Pixmap::new(VIEW.0, VIEW.1).unwrap();
    render_page(&t.doc, t.page, &Viewport::default(), &mut cpu);
    assert_eq!(rows(&cpu), (30, 39), "{name}: CPU keeps the band above the line");
    let Some(mut g) = gpu() else { return };
    let c = compare(&mut g, name, &t.doc, t.page, Viewport::default(), VIEW, 8);
    assert_eq!(rows(&c.gpu), (30, 39), "{name}: GPU keeps the band above the line");
    assert!(c.report.ratio < 0.02, "{name}: mismatch {:.3}%", c.report.ratio * 100.0);
    assert!(c.report.ssim > 0.97, "{name}: {:?}", c.report);
}

#[test]
fn directly_resized_imported_line_matches_cpu() {
    let mut t = T::new();
    let id = line(&mut t, None, Vec2::new(20.0, 40.0), 100.0);
    resize(&mut t.doc, id, Vec2::new(200.0, 0.0));
    check("resized_line_direct", &t);
}

#[test]
fn imported_line_in_a_resized_frame_matches_cpu() {
    let mut t = T::new();
    let frame = t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Frame(FrameProps::default()));
        n.size = Vec2::new(100.0, 60.0);
        n.fills.clear();
        n
    });
    let id = line(&mut t, Some(frame), Vec2::new(20.0, 40.0), 100.0);
    t.node(id).constraints = Constraints { horizontal: Constraint::Scale, vertical: Constraint::Scale };
    resize(&mut t.doc, frame, Vec2::new(200.0, 60.0));
    check("resized_line_in_frame", &t);
}
