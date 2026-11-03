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

fn rect_subpath(x0: f32, y0: f32, x1: f32, y1: f32, reversed: bool) -> Vec<PathCmd> {
    if !reversed {
        vec![
            PathCmd::MoveTo(Vec2::new(x0, y0)),
            PathCmd::LineTo(Vec2::new(x1, y0)),
            PathCmd::LineTo(Vec2::new(x1, y1)),
            PathCmd::LineTo(Vec2::new(x0, y1)),
            PathCmd::Close,
        ]
    } else {
        vec![
            PathCmd::MoveTo(Vec2::new(x0, y0)),
            PathCmd::LineTo(Vec2::new(x0, y1)),
            PathCmd::LineTo(Vec2::new(x1, y1)),
            PathCmd::LineTo(Vec2::new(x1, y0)),
            PathCmd::Close,
        ]
    }
}

fn imported_stroked_icon(t: &mut T, at: Vec2) -> NodeId {
    let mut cmds = rect_subpath(1.0, 1.0, 19.0, 19.0, false);
    cmds.extend(rect_subpath(3.0, 3.0, 17.0, 17.0, true));
    let stroke_geo = vec![Path { cmds, fill_rule: FillRule::NonZero }];
    let centerline = vec![Path { cmds: rect_subpath(2.0, 2.0, 18.0, 18.0, false), fill_rule: FillRule::NonZero }];
    let net = VectorNetwork::from_paths(&centerline);
    t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Vector { paths: stroke_geo.clone(), network: Some(net) });
        n.transform = Affine::translate(at.x, at.y);
        n.size = Vec2::new(20.0, 20.0);
        n.fills.clear();
        n.stroke.paints = vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))];
        n.stroke.weight = 2.0;
        n.stroke.align = StrokeAlign::Center;
        n.stroke.cap = StrokeCap::Round;
        n.stroke.join = StrokeJoin::Round;
        n.stroke_geometry = Some(stroke_geo);
        n
    })
}

fn horizontal_runs(pm: &Pixmap, y: u32, x_range: std::ops::Range<u32>) -> Vec<u32> {
    let mut runs = Vec::new();
    let mut cur = 0u32;
    for x in x_range {
        let p = pm.pixel(x, y).unwrap();
        if p.red() < 128 && p.green() < 128 && p.blue() < 128 {
            cur += 1;
        } else if cur > 0 {
            runs.push(cur);
            cur = 0;
        }
    }
    if cur > 0 {
        runs.push(cur);
    }
    runs
}

#[test]
fn resized_imported_vector_keeps_stroke_weight_and_matches_cpu() {
    let mut t = T::new();
    let icon_2x = imported_stroked_icon(&mut t, Vec2::new(20.0, 20.0));
    let icon_2x1 = imported_stroked_icon(&mut t, Vec2::new(90.0, 20.0));
    resize(&mut t.doc, icon_2x, Vec2::new(40.0, 40.0));
    resize(&mut t.doc, icon_2x1, Vec2::new(40.0, 20.0));

    let cpu = cpu_render(&t);
    assert_eq!(horizontal_runs(&cpu, 40, 15..65), vec![2, 2], "CPU 2x plain resize keeps 2px stroke walls");
    assert_eq!(horizontal_runs(&cpu, 30, 85..135), vec![2, 2], "CPU 2x1 plain resize keeps 2px stroke walls");

    let Some(mut g) = gpu() else { return };
    let c = compare(&mut g, "resized_imported_vector", &t.doc, t.page, Viewport::default(), VIEW, 8);
    assert_eq!(horizontal_runs(&c.gpu, 40, 15..65), vec![2, 2], "GPU 2x plain resize keeps 2px stroke walls");
    assert_eq!(horizontal_runs(&c.gpu, 30, 85..135), vec![2, 2], "GPU 2x1 plain resize keeps 2px stroke walls");
    assert!(c.report.ratio < 0.02, "mismatch {:.3}%", c.report.ratio * 100.0);
    assert!(c.report.ssim > 0.97, "{:?}", c.report);
}

fn vertical_runs(pm: &Pixmap, x: u32, y_range: std::ops::Range<u32>) -> Vec<u32> {
    let mut runs = Vec::new();
    let mut cur = 0u32;
    for y in y_range {
        let p = pm.pixel(x, y).unwrap();
        if p.red() < 128 && p.green() < 128 && p.blue() < 128 {
            cur += 1;
        } else if cur > 0 {
            runs.push(cur);
            cur = 0;
        }
    }
    if cur > 0 {
        runs.push(cur);
    }
    runs
}

fn is_white(pm: &Pixmap, x: u32, y: u32) -> bool {
    let p = pm.pixel(x, y).unwrap();
    p.red() > 250 && p.green() > 250 && p.blue() > 250
}

/// A filled square next to a bent, stroke-only chain, the way Figma writes it: the fill outline is the
/// square, the network also holds the chain, and the baked stroke covers both at the given alignment.
fn square_with_bent_whisker(t: &mut T, at: Vec2, align: StrokeAlign) -> NodeId {
    let square = Path { cmds: rect_subpath(6.0, 2.0, 16.0, 12.0, false), fill_rule: FillRule::NonZero };
    let whisker = Path {
        cmds: vec![PathCmd::MoveTo(Vec2::new(2.0, 14.0)), PathCmd::LineTo(Vec2::new(2.0, 18.0)), PathCmd::LineTo(Vec2::new(18.0, 18.0))],
        fill_rule: FillRule::NonZero,
    };
    let net = VectorNetwork::from_paths(&[square.clone(), whisker]);
    assert!(net.is_valid() && net.has_stroke_only_segments());
    let stroke = Stroke { weight: 2.0, align, ..Stroke::default() };
    let bake = designos_render::vectorops::outline_stroke(&net.to_paths(), &stroke, Some(&net));
    t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Vector { paths: vec![square.clone()], network: Some(net) });
        n.transform = Affine::translate(at.x, at.y);
        n.size = Vec2::new(20.0, 20.0);
        n.fills = vec![Paint::solid(Color::rgb(0xF2, 0x4E, 0x1E))];
        n.stroke.paints = vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))];
        n.stroke.weight = 2.0;
        n.stroke.align = align;
        n.fill_geometry = Some(vec![square]);
        n.stroke_geometry = Some(bake);
        n
    })
}

#[test]
fn resized_vector_strokes_its_network_and_fills_its_outline_like_cpu() {
    let mut t = T::new();
    let icon = square_with_bent_whisker(&mut t, Vec2::new(20.0, 20.0), StrokeAlign::Center);
    resize(&mut t.doc, icon, Vec2::new(40.0, 40.0));

    // the chain's bottom run (x = 28) is one band of the stroke's own weight; the area between its two
    // legs and the diagonal (34, 53) is not a fill
    let cpu = cpu_render(&t);
    assert_eq!(vertical_runs(&cpu, 28, 15..95), vec![2], "CPU: one band on the stroke-only chain");
    assert!(is_white(&cpu, 34, 53), "CPU: the stroke-only chain is not filled");

    let Some(mut g) = gpu() else { return };
    let c = compare(&mut g, "resized_vector_network_stroke", &t.doc, t.page, Viewport::default(), VIEW, 8);
    assert_eq!(vertical_runs(&c.gpu, 28, 15..95), vec![2], "GPU: one band on the stroke-only chain");
    assert!(is_white(&c.gpu, 34, 53), "GPU: the stroke-only chain is not filled");
    assert!(c.report.ratio < 0.005, "mismatch {:.3}%", c.report.ratio * 100.0);
    assert!(c.report.ssim > 0.98, "{:?}", c.report);
}

fn is_ink(pm: &Pixmap, x: u32, y: u32) -> bool {
    let p = pm.pixel(x, y).unwrap();
    p.red() < 128 && p.green() < 128 && p.blue() < 128
}

#[test]
fn resized_inside_and_outside_strokes_with_an_open_chain_drop_their_bake_and_stroke_live_like_cpu() {
    for (align, name) in [(StrokeAlign::Inside, "resized_inside_chain"), (StrokeAlign::Outside, "resized_outside_chain")] {
        let mut t = T::new();
        let icon = square_with_bent_whisker(&mut t, Vec2::new(20.0, 20.0), align);
        resize(&mut t.doc, icon, Vec2::new(40.0, 40.0));
        assert!(t.doc.get(icon).unwrap().stroke_geometry.is_none(), "{align:?}: the baked outline is dropped");

        // the square's left edge sits at x = 32: an inside stroke lies to its right, an outside one to its left
        let (clear, stroked) = if align == StrokeAlign::Inside { (31, 33) } else { (33, 31) };
        let cpu = cpu_render(&t);
        assert!(!is_ink(&cpu, clear, 34) && is_ink(&cpu, stroked, 34), "CPU {align:?}: the stroke stays on its own side of the edge");
        assert_eq!(vertical_runs(&cpu, 28, 15..95), vec![2], "CPU {align:?}: one 2px centered band on the open chain");
        let Some(mut g) = gpu() else { return };
        let c = compare(&mut g, name, &t.doc, t.page, Viewport::default(), VIEW, 8);
        assert!(!is_ink(&c.gpu, clear, 34) && is_ink(&c.gpu, stroked, 34), "GPU {align:?}: the stroke stays on its own side of the edge");
        assert_eq!(vertical_runs(&c.gpu, 28, 15..95), vec![2], "GPU {align:?}: one 2px centered band on the open chain");
        assert!(c.report.ratio < 0.005, "{align:?}: mismatch {:.3}%", c.report.ratio * 100.0);
        assert!(c.report.ssim > 0.98, "{align:?}: {:?}", c.report);
    }
}

#[test]
fn file_type_network_resized_2x_strokes_outline_inside_and_fold_centered_like_cpu() {
    let outline = Path {
        cmds: vec![
            PathCmd::MoveTo(Vec2::new(0.0, 0.0)),
            PathCmd::LineTo(Vec2::new(20.0, 0.0)),
            PathCmd::LineTo(Vec2::new(32.0, 12.0)),
            PathCmd::LineTo(Vec2::new(32.0, 40.0)),
            PathCmd::LineTo(Vec2::new(0.0, 40.0)),
            PathCmd::Close,
        ],
        fill_rule: FillRule::NonZero,
    };
    let fold = Path {
        cmds: vec![PathCmd::MoveTo(Vec2::new(20.0, 0.0)), PathCmd::LineTo(Vec2::new(20.0, 12.0)), PathCmd::LineTo(Vec2::new(32.0, 12.0))],
        fill_rule: FillRule::NonZero,
    };
    let net = VectorNetwork::from_paths(&[outline.clone(), fold]);
    let stroke = Stroke {
        paints: vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))],
        weight: 1.5,
        align: StrokeAlign::Inside,
        cap: StrokeCap::Round,
        join: StrokeJoin::Round,
        ..Stroke::default()
    };
    let bake = designos_render::vectorops::outline_stroke(&net.to_paths(), &stroke, Some(&net));
    let mut t = T::new();
    let icon = t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Vector { paths: net.to_paths(), network: Some(net) });
        n.transform = Affine::translate(20.0, 20.0);
        n.size = Vec2::new(32.0, 40.0);
        n.fills = vec![Paint::solid(Color::WHITE)];
        n.stroke = stroke;
        n.fill_geometry = Some(vec![outline]);
        n.stroke_geometry = Some(bake);
        n
    });
    resize(&mut t.doc, icon, Vec2::new(64.0, 80.0));
    assert!(t.doc.get(icon).unwrap().stroke_geometry.is_none(), "file-type network drops its bake on 2x resize");

    let cpu = cpu_render(&t);
    assert!(!is_ink(&cpu, 19, 60) && is_ink(&cpu, 20, 60) && !is_ink(&cpu, 22, 60), "CPU: 1.5px inside stroke on left page edge");
    assert!(is_ink(&cpu, 60, 32) && is_white(&cpu, 58, 32) && is_white(&cpu, 61, 32), "CPU: 1.5px centered stroke on fold leg");

    let Some(mut g) = gpu() else { return };
    let c = compare(&mut g, "file_type_resized_2x", &t.doc, t.page, Viewport::default(), VIEW, 8);
    assert!(!is_ink(&c.gpu, 19, 60) && is_ink(&c.gpu, 20, 60) && !is_ink(&c.gpu, 22, 60), "GPU: 1.5px inside stroke on left page edge");
    assert!(is_ink(&c.gpu, 60, 32) && is_white(&c.gpu, 58, 32) && is_white(&c.gpu, 61, 32), "GPU: 1.5px centered stroke on fold leg");
    assert!(c.report.ratio < 0.005, "mismatch {:.3}%", c.report.ratio * 100.0);
    assert!(c.report.ssim > 0.98, "{:?}", c.report);
}

#[test]
fn mixed_miter_and_round_join_path_resized_2x_keeps_both_joins_like_cpu() {
    let path = Path {
        cmds: vec![
            PathCmd::MoveTo(Vec2::new(4.0, 16.0)),
            PathCmd::LineTo(Vec2::new(4.0, 4.0)),
            PathCmd::LineTo(Vec2::new(20.0, 4.0)),
            PathCmd::LineTo(Vec2::new(20.0, 16.0)),
        ],
        fill_rule: FillRule::NonZero,
    };
    let mut net = VectorNetwork::from_paths(std::slice::from_ref(&path));
    for v in &mut net.vertices {
        if (v.pos.x - 4.0).abs() < 1e-3 && (v.pos.y - 4.0).abs() < 1e-3 {
            v.join = Some(StrokeJoin::Miter);
        } else {
            v.join = Some(StrokeJoin::Round);
        }
    }
    let stroke = Stroke {
        paints: vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))],
        weight: 8.0,
        align: StrokeAlign::Center,
        cap: StrokeCap::None,
        join: StrokeJoin::Round,
        miter_limit: 4.0,
        ..Stroke::default()
    };
    let bake = designos_render::vectorops::outline_stroke(&net.to_paths(), &stroke, Some(&net));
    let mut t = T::new();
    let id = t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Vector { paths: net.to_paths(), network: Some(net) });
        n.transform = Affine::translate(10.0, 10.0);
        n.size = Vec2::new(24.0, 20.0);
        n.stroke = stroke;
        n.stroke_geometry = Some(bake);
        n
    });
    resize(&mut t.doc, id, Vec2::new(48.0, 40.0));
    assert!(t.doc.get(id).unwrap().stroke_geometry.is_none(), "mixed-join path drops its bake on 2x resize");

    let cpu = cpu_render(&t);
    assert!(is_ink(&cpu, 14, 14) && !is_ink(&cpu, 53, 14), "CPU: Miter corner sharp at (14, 14), Round corner rounded at (53, 14)");

    let Some(mut g) = gpu() else { return };
    let c = compare(&mut g, "mixed_join_resized_2x", &t.doc, t.page, Viewport::default(), VIEW, 8);
    assert!(is_ink(&c.gpu, 14, 14) && !is_ink(&c.gpu, 53, 14), "GPU: Miter corner sharp at (14, 14), Round corner rounded at (53, 14)");
    assert!(c.report.ratio < 0.005, "mismatch {:.3}%", c.report.ratio * 100.0);
    assert!(c.report.ssim > 0.98, "{:?}", c.report);
}

#[test]
fn equal_cap_open_stroke_draws_identically_on_cpu_and_gpu() {
    let seg = Path { cmds: vec![PathCmd::MoveTo(Vec2::new(20.0, 40.0)), PathCmd::LineTo(Vec2::new(80.0, 40.0))], fill_rule: FillRule::NonZero };
    let mut net = VectorNetwork::from_paths(std::slice::from_ref(&seg));
    for v in &mut net.vertices {
        v.cap = Some(StrokeCap::Round);
    }
    let mut t = T::new();
    let id = t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Vector { paths: net.to_paths(), network: Some(net) });
        n.size = Vec2::new(100.0, 80.0);
        n.stroke = Stroke {
            paints: vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))],
            weight: 12.0,
            align: StrokeAlign::Center,
            cap: StrokeCap::Round,
            join: StrokeJoin::Round,
            ..Stroke::default()
        };
        n
    });
    assert!(designos_render::vectorops::vertex_caps(t.doc.get(id).unwrap()).is_none(), "equal per-vertex caps use the node cap path");

    let Some(mut g) = gpu() else { return };
    let c = compare(&mut g, "equal_cap_open_stroke", &t.doc, t.page, Viewport::default(), VIEW, 8);
    assert_eq!(c.report_aa.mismatched, 0, "equal-cap open stroke has 0 AA-forgiven mismatched pixels between CPU and GPU");
    assert!(c.report.ratio < 0.005, "mismatch {:.3}%", c.report.ratio * 100.0);
    assert!(c.report.ssim > 0.99, "{:?}", c.report);
}

/// An open path `(0, 0) -> (40, 0) -> (40, 40) -> (80, 40)` at `(20, 20)`, stroked live at weight 10 with the node
/// join Bevel and its own join on each inner vertex (Miter at `(40, 0)`, Round at `(40, 40)`).
fn open_path_with_mixed_vertex_joins(t: &mut T, cap: StrokeCap) -> NodeId {
    let path = Path {
        cmds: vec![
            PathCmd::MoveTo(Vec2::new(0.0, 0.0)),
            PathCmd::LineTo(Vec2::new(40.0, 0.0)),
            PathCmd::LineTo(Vec2::new(40.0, 40.0)),
            PathCmd::LineTo(Vec2::new(80.0, 40.0)),
        ],
        fill_rule: FillRule::NonZero,
    };
    let mut net = VectorNetwork::from_paths(std::slice::from_ref(&path));
    for v in &mut net.vertices {
        let on = |x: f32, y: f32| (v.pos.x - x).abs() < 1e-3 && (v.pos.y - y).abs() < 1e-3;
        v.join = if on(40.0, 0.0) {
            Some(StrokeJoin::Miter)
        } else if on(40.0, 40.0) {
            Some(StrokeJoin::Round)
        } else {
            None
        };
    }
    t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Vector { paths: net.to_paths(), network: Some(net) });
        n.transform = Affine::translate(20.0, 20.0);
        n.size = Vec2::new(80.0, 40.0);
        n.fills.clear();
        n.stroke = Stroke {
            paints: vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))],
            weight: 10.0,
            align: StrokeAlign::Center,
            cap,
            join: StrokeJoin::Bevel,
            miter_limit: 4.0,
            ..Stroke::default()
        };
        n
    })
}

/// Both ends of the open path carry the node cap pointing away from the stroke: a Round cap reaches half a
/// weight past each end point, a Square cap fills the square beyond it.
#[track_caller]
fn assert_caps_point_outward(pm: &Pixmap, cap: StrokeCap, what: &str) {
    // the path starts at page (20, 20) heading right and ends at page (100, 60) heading right
    assert!(is_ink(pm, 16, 20), "{what} {cap:?}: the start cap reaches left of the start point");
    assert!(is_ink(pm, 104, 60), "{what} {cap:?}: the end cap reaches right of the end point");
    assert!(!is_ink(pm, 13, 20), "{what} {cap:?}: the start cap stops half a weight from the start point");
    assert!(!is_ink(pm, 106, 60), "{what} {cap:?}: the end cap stops half a weight from the end point");
    let square = cap == StrokeCap::Square;
    assert_eq!(is_ink(pm, 16, 15), square, "{what} {cap:?}: the start cap fills its corner only when square");
    assert_eq!(is_ink(pm, 104, 64), square, "{what} {cap:?}: the end cap fills its corner only when square");
}

fn assert_open_stroke_with_per_vertex_joins_caps_outward_like_cpu(cap: StrokeCap, name: &str) {
    let mut t = T::new();
    open_path_with_mixed_vertex_joins(&mut t, cap);
    let Some(mut g) = gpu() else {
        assert_caps_point_outward(&cpu_render(&t), cap, "CPU");
        return;
    };
    let c = compare(&mut g, name, &t.doc, t.page, Viewport::default(), VIEW, 8);
    assert_caps_point_outward(&c.gpu, cap, "GPU");
    assert_caps_point_outward(&c.cpu, cap, "CPU");
    assert!(c.report.ratio < 0.005, "{cap:?}: mismatch {:.3}%", c.report.ratio * 100.0);
    assert!(c.report.ssim > 0.98, "{cap:?}: {:?}", c.report);
}

#[test]
fn an_open_stroke_with_per_vertex_joins_caps_both_ends_pointing_outward_round_like_cpu() {
    assert_open_stroke_with_per_vertex_joins_caps_outward_like_cpu(StrokeCap::Round, "open_mixed_join_round_caps");
}

#[test]
fn an_open_stroke_with_per_vertex_joins_caps_both_ends_pointing_outward_square_like_cpu() {
    assert_open_stroke_with_per_vertex_joins_caps_outward_like_cpu(StrokeCap::Square, "open_mixed_join_square_caps");
}
