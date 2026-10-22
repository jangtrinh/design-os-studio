//! A vector or a boolean group draws the geometry it stores, so a resized one must be drawn at its new
//! size by both renderers. Tests print a note and skip the GPU half when the machine has no GPU adapter;
//! the CPU half (the drawn ink reaches the new box) always runs.

mod common;

use common::*;
use designos_doc::*;
use designos_render::tiny_skia::Pixmap;
use designos_render::vectorops::{flatten_nodes, node_paths};
use designos_render::{render_page, Viewport};

const VIEW: (u32, u32) = (260, 140);

/// What the editor's resize does to the document: the node's own box and geometry, then the constraint
/// pass that carries a boolean group's operands (or a frame's children) along.
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

/// Corners of the pixels that are not the white page: (min x, min y, max x, max y).
fn painted(pm: &Pixmap) -> (u32, u32, u32, u32) {
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for y in 0..pm.height() {
        for x in 0..pm.width() {
            let p = pm.pixel(x, y).unwrap();
            if p.red() < 250 || p.green() < 250 || p.blue() < 250 {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            }
        }
    }
    (x0, y0, x1, y1)
}

fn cpu_render(t: &T) -> Pixmap {
    let mut pm = Pixmap::new(VIEW.0, VIEW.1).unwrap();
    render_page(&t.doc, t.page, &Viewport::default(), &mut pm);
    pm
}

/// The ink of the page spans `box_` to within a pixel and a half of antialiasing.
#[track_caller]
fn assert_ink_fills(pm: &Pixmap, box_: Rect, what: &str) {
    let (x0, y0, x1, y1) = painted(pm);
    let near = |got: u32, want: f32| (got as f32 - want).abs() <= 1.5;
    assert!(
        near(x0, box_.x) && near(y0, box_.y) && near(x1 + 1, box_.x + box_.w) && near(y1 + 1, box_.y + box_.h),
        "{what}: ink spans ({x0}, {y0}) to ({x1}, {y1}), the box is {box_:?}"
    );
}

/// A star flattened into a vector the way Ctrl+E does it: paths from the render crate, fitted to the node box.
fn flattened_star(t: &mut T, at: Vec2, side: f32) -> NodeId {
    let star = t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Star { count: 5, inner_ratio: 0.4 });
        n.transform = Affine::translate(at.x, at.y);
        n.size = Vec2::new(side, side);
        n.fills = vec![Paint::solid(Color::rgb(0x18, 0xA0, 0xFB))];
        n
    });
    let flat = flatten_nodes(&t.doc, &[star], &Affine::IDENTITY).expect("a star flattens");
    let mut net = VectorNetwork::from_paths(&flat.paths);
    let bb = net.bounds();
    net.translate(Vec2::new(-bb.x, -bb.y));
    let vector = t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Vector { paths: net.to_paths(), network: Some(net) });
        n.transform = Affine::translate(bb.x, bb.y);
        n.size = Vec2::new(bb.w, bb.h);
        n.fills = vec![Paint::solid(Color::rgb(0x18, 0xA0, 0xFB))];
        n
    });
    t.doc.remove(star).unwrap();
    vector
}

/// Parity lock: the doc-level resize already scales a vector on its own, so this passes without the editor fix.
/// The editor's resize is covered by the app tests.
#[test]
fn resized_flattened_star_matches_cpu() {
    let mut t = T::new();
    let star = flattened_star(&mut t, Vec2::new(20.0, 20.0), 100.0);
    let box_ = Rect::new(20.0, 20.0, 100.0, 100.0);
    assert_ink_fills(&cpu_render(&t), box_, "before the resize");

    resize(&mut t.doc, star, Vec2::new(200.0, 70.0));

    let box_ = Rect::new(20.0, 20.0, 200.0, 70.0);
    assert_ink_fills(&cpu_render(&t), box_, "CPU, after the resize");
    let Some(mut g) = gpu() else { return };
    let c = compare(&mut g, "resized_star", &t.doc, t.page, Viewport::default(), VIEW, 8);
    assert_ink_fills(&c.gpu, box_, "GPU, after the resize");
    assert!(c.report.ratio < 0.02, "mismatch {:.3}%", c.report.ratio * 100.0);
    assert!(c.report.ssim > 0.97, "{:?}", c.report);
}

/// A union the way the editor builds one: a group whose operands live in its own space and no stored result.
fn union_of_two_rects(t: &mut T) -> NodeId {
    let group = t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::BooleanOp(BooleanOp::Union));
        n.transform = Affine::translate(20.0, 20.0);
        n.size = Vec2::new(90.0, 70.0);
        n.fills = vec![Paint::solid(Color::rgb(0xF2, 0x4E, 0x1E))];
        n
    });
    for (name, r) in [("A", Rect::new(0.0, 0.0, 60.0, 40.0)), ("B", Rect::new(30.0, 20.0, 60.0, 50.0))] {
        let id = t.add(Some(group), |id| Node::rectangle(id, r));
        t.node(id).name = name.into();
    }
    group
}

#[test]
fn resized_union_matches_cpu() {
    let mut t = T::new();
    let group = union_of_two_rects(&mut t);
    assert_ink_fills(&cpu_render(&t), Rect::new(20.0, 20.0, 90.0, 70.0), "before the resize");

    resize(&mut t.doc, group, Vec2::new(180.0, 105.0));

    let box_ = Rect::new(20.0, 20.0, 180.0, 105.0);
    assert_ink_fills(&cpu_render(&t), box_, "CPU, after the resize");
    let Some(mut g) = gpu() else { return };
    let c = compare(&mut g, "resized_union", &t.doc, t.page, Viewport::default(), VIEW, 8);
    assert_ink_fills(&c.gpu, box_, "GPU, after the resize");
    assert!(c.report.ratio < 0.02, "mismatch {:.3}%", c.report.ratio * 100.0);
    assert!(c.report.ssim > 0.97, "{:?}", c.report);
}

/// An ellipse or a star the way a `.fig` import leaves it: the primitive plus Figma's outline of it at the old size,
/// which both renderers prefer to the primitive. `native` leaves the outline out.
fn shape(t: &mut T, kind: NodeKind, at: Vec2, size: Vec2, native: bool) -> NodeId {
    let id = t.add(None, |id| {
        let mut n = Node::new(id, kind);
        n.transform = Affine::translate(at.x, at.y);
        n.size = size;
        n.fills = vec![Paint::solid(Color::rgb(0x18, 0xA0, 0xFB))];
        n
    });
    if !native {
        let outline = node_paths(&t.doc, t.doc.get(id).unwrap()).expect("a shape has an outline");
        t.node(id).fill_geometry = Some(outline);
    }
    id
}

fn oval() -> NodeKind {
    NodeKind::Ellipse(ArcData::default())
}

fn star() -> NodeKind {
    NodeKind::Star { count: 5, inner_ratio: 0.4 }
}

#[test]
fn resized_imported_ellipse_and_star_match_cpu() {
    let (old, new) = (Vec2::new(100.0, 100.0), Vec2::new(110.0, 60.0));
    let mut t = T::new();
    let ids = [shape(&mut t, oval(), Vec2::new(10.0, 10.0), old, false), shape(&mut t, star(), Vec2::new(130.0, 10.0), old, false)];
    for id in ids {
        resize(&mut t.doc, id, new);
    }
    // the same two shapes made at the new size, with no imported outline to go stale
    let mut native = T::new();
    shape(&mut native, oval(), Vec2::new(10.0, 10.0), new, true);
    shape(&mut native, star(), Vec2::new(130.0, 10.0), new, true);

    let (resized, fresh) = (cpu_render(&t), cpu_render(&native));
    assert_eq!(painted(&resized), painted(&fresh), "the resized shapes are drawn at their new size");
    assert_eq!(resized.data(), fresh.data(), "and exactly as the same shapes made at that size");
    let Some(mut g) = gpu() else { return };
    let c = compare(&mut g, "resized_imported_shapes", &t.doc, t.page, Viewport::default(), VIEW, 8);
    assert_eq!(painted(&c.gpu), painted(&fresh), "GPU, after the resize");
    assert!(c.report.ratio < 0.02, "mismatch {:.3}%", c.report.ratio * 100.0);
    assert!(c.report.ssim > 0.97, "{:?}", c.report);
}
