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
    // the star's points sit on its inscribed circle, so the flattened vector is the star's own bounds, not the 100 x 100 box
    let (origin, flat_size) = {
        let n = t.doc.get(star).unwrap();
        (Vec2::new(n.transform.tx, n.transform.ty), n.size)
    };
    let box_ = Rect::new(origin.x, origin.y, flat_size.x, flat_size.y);
    assert_ink_fills(&cpu_render(&t), box_, "before the resize");

    resize(&mut t.doc, star, Vec2::new(200.0, 70.0));

    let box_ = Rect::new(origin.x, origin.y, 200.0, 70.0);
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

fn polyline(points: &[(f32, f32)]) -> Vec<PathCmd> {
    let mut cmds = vec![PathCmd::MoveTo(Vec2::new(points[0].0, points[0].1))];
    cmds.extend(points[1..].iter().map(|p| PathCmd::LineTo(Vec2::new(p.0, p.1))));
    cmds
}

/// An open path stroked live at weight 10 at `(20, 20)`, node join Bevel and no cap, with a Miter join on the vertex at
/// `miter` and a Round join on the vertex at `round`.
fn open_stroke_with_a_miter_and_a_round_vertex(t: &mut T, cmds: Vec<PathCmd>, miter: (f32, f32), round: (f32, f32)) -> NodeId {
    let path = Path { cmds, fill_rule: FillRule::NonZero };
    let mut net = VectorNetwork::from_paths(std::slice::from_ref(&path));
    for v in &mut net.vertices {
        let on = |p: (f32, f32)| (v.pos.x - p.0).abs() < 1e-3 && (v.pos.y - p.1).abs() < 1e-3;
        v.join = if on(miter) {
            Some(StrokeJoin::Miter)
        } else if on(round) {
            Some(StrokeJoin::Round)
        } else {
            None
        };
    }
    let bounds = net.bounds();
    t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Vector { paths: net.to_paths(), network: Some(net) });
        n.transform = Affine::translate(20.0, 20.0);
        n.size = Vec2::new(bounds.x + bounds.w, bounds.y + bounds.h);
        n.fills.clear();
        n.stroke = Stroke {
            paints: vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))],
            weight: 10.0,
            align: StrokeAlign::Center,
            cap: StrokeCap::None,
            join: StrokeJoin::Bevel,
            miter_limit: 4.0,
            ..Stroke::default()
        };
        n
    })
}

/// A path with a Miter vertex and a Round vertex, and what each pixel centre (node space) must show: the Miter corner
/// reaches its square apex, the Round corner is an arc that is neither a bevel nor a square.
struct JoinCase {
    name: &'static str,
    cmds: Vec<PathCmd>,
    miter: (f32, f32),
    round: (f32, f32),
    /// `(what, x, y, ink)` in node space.
    probes: Vec<(&'static str, f32, f32, bool)>,
}

fn join_cases() -> Vec<JoinCase> {
    vec![
        JoinCase {
            name: "right_then_down_on_the_axis",
            cmds: polyline(&[(0.0, 0.0), (40.0, 0.0), (40.0, 40.0), (80.0, 40.0)]),
            miter: (40.0, 0.0),
            round: (40.0, 40.0),
            probes: vec![("miter apex", 44.5, -3.5, true), ("round arc", 36.5, 42.5, true), ("round outer corner", 35.5, 44.5, false)],
        },
        JoinCase {
            name: "right_then_down_off_the_axis",
            cmds: polyline(&[(8.0, 8.0), (48.0, 8.0), (48.0, 48.0), (88.0, 48.0)]),
            miter: (48.0, 8.0),
            round: (48.0, 48.0),
            probes: vec![("miter apex", 52.5, 4.5, true), ("round arc", 44.5, 50.5, true), ("round outer corner", 43.5, 52.5, false)],
        },
        JoinCase {
            name: "curve_then_right_then_down",
            cmds: vec![
                PathCmd::MoveTo(Vec2::new(0.0, 40.0)),
                PathCmd::CubicTo(Vec2::new(0.0, 16.0), Vec2::new(16.0, 0.0), Vec2::new(40.0, 0.0)),
                PathCmd::LineTo(Vec2::new(40.0, 40.0)),
                PathCmd::LineTo(Vec2::new(80.0, 40.0)),
            ],
            miter: (40.0, 0.0),
            round: (40.0, 40.0),
            probes: vec![
                ("curve", 11.5, 11.5, true),
                ("miter apex", 44.5, -3.5, true),
                ("round arc", 36.5, 42.5, true),
                ("round outer corner", 35.5, 44.5, false),
            ],
        },
        JoinCase {
            name: "down_then_right_on_the_axis",
            cmds: polyline(&[(0.0, 0.0), (0.0, 40.0), (40.0, 40.0), (40.0, 80.0)]),
            miter: (0.0, 40.0),
            round: (40.0, 40.0),
            probes: vec![("miter apex", -4.5, 44.5, true), ("round arc", 42.5, 36.5, true), ("round outer corner", 44.5, 35.5, false)],
        },
    ]
}

fn page_pixel(x: f32, y: f32) -> (u32, u32) {
    ((20.0 + x).floor() as u32, (20.0 + y).floor() as u32)
}

/// The probes of `case` that `pm` does not show as expected.
fn failed_join_probes(pm: &Pixmap, case: &JoinCase, what: &str) -> Vec<String> {
    case.probes
        .iter()
        .filter_map(|(probe, x, y, ink)| {
            let (px_x, px_y) = page_pixel(*x, *y);
            (is_ink(pm, px_x, px_y) != *ink)
                .then(|| format!("{what} {}: {probe} at node ({x}, {y}) should be {}", case.name, if *ink { "inked" } else { "white" }))
        })
        .collect()
}

#[test]
fn a_miter_vertex_between_round_vertices_keeps_its_sharp_apex_like_cpu() {
    let mut failed = vec![];
    for case in join_cases() {
        let mut t = T::new();
        open_stroke_with_a_miter_and_a_round_vertex(&mut t, case.cmds.clone(), case.miter, case.round);
        let Some(mut g) = gpu() else {
            failed.extend(failed_join_probes(&cpu_render(&t), &case, "CPU"));
            continue;
        };
        let c = compare(&mut g, &format!("miter_round_joins_{}", case.name), &t.doc, t.page, Viewport::default(), VIEW, 8);
        failed.extend(failed_join_probes(&c.gpu, &case, "GPU"));
        failed.extend(failed_join_probes(&c.cpu, &case, "CPU"));
        if c.report.ratio >= 0.005 || c.report.ssim <= 0.98 {
            failed.push(format!("{}: CPU and GPU differ, mismatch {:.3}% {:?}", case.name, c.report.ratio * 100.0, c.report));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

#[test]
fn all_five_arrow_caps_match_cpu_on_lines_and_open_vectors() {
    let seg = Path { cmds: vec![PathCmd::MoveTo(Vec2::new(20.0, 50.0)), PathCmd::LineTo(Vec2::new(80.0, 50.0))], fill_rule: FillRule::NonZero };
    for (cap, name, ink_pt, clear_pt) in [
        (StrokeCap::ArrowLines, "arrow_lines", (70, 40), (90, 40)),
        (StrokeCap::ArrowEquilateral, "arrow_equilateral", (62, 41), (78, 41)),
        (StrokeCap::TriangleFilled, "triangle_filled", (78, 41), (62, 41)),
        (StrokeCap::DiamondFilled, "diamond_filled", (80, 40), (88, 42)),
        (StrokeCap::CircleFilled, "circle_filled", (86, 44), (90, 40)),
    ] {
        for as_line in [false, true] {
            let mut t = T::new();
            t.add(None, |id| {
                let kind = if as_line {
                    NodeKind::Line
                } else {
                    let net = VectorNetwork::from_paths(std::slice::from_ref(&seg));
                    NodeKind::Vector { paths: vec![seg.clone()], network: Some(net) }
                };
                let mut n = Node::new(id, kind);
                if as_line {
                    n.transform = Affine::translate(20.0, 50.0);
                    n.size = Vec2::new(60.0, 0.0);
                } else {
                    n.size = Vec2::new(100.0, 100.0);
                }
                n.stroke = Stroke {
                    paints: vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))],
                    weight: 4.0,
                    align: StrokeAlign::Center,
                    cap,
                    join: StrokeJoin::Round,
                    ..Stroke::default()
                };
                n
            });
            let cpu = cpu_render(&t);
            assert!(is_ink(&cpu, ink_pt.0, ink_pt.1), "CPU {name} (line={as_line}): expected head ink at {ink_pt:?}");
            assert!(!is_ink(&cpu, clear_pt.0, clear_pt.1), "CPU {name} (line={as_line}): expected clear pixel at {clear_pt:?}");

            let Some(mut g) = gpu() else { return };
            let label = format!("{name}_{}", if as_line { "line" } else { "vec" });
            let c = compare(&mut g, &label, &t.doc, t.page, Viewport::default(), VIEW, 8);
            assert!(is_ink(&c.gpu, ink_pt.0, ink_pt.1), "GPU {label}: expected head ink at {ink_pt:?}");
            assert!(!is_ink(&c.gpu, clear_pt.0, clear_pt.1), "GPU {label}: expected clear pixel at {clear_pt:?}");
            assert_eq!(c.report_aa.mismatched, 0, "{label}: 0 AA-forgiven mismatched pixels between CPU and GPU");
            assert!(c.report.ratio < 0.005, "{label}: mismatch {:.3}%", c.report.ratio * 100.0);
            assert!(c.report.ssim > 0.99, "{label}: {:?}", c.report);
        }
    }
}

#[test]
fn resized_2x_arrow_vectors_drop_bake_keep_head_size_and_match_cpu() {
    for (cap, name) in [
        (StrokeCap::ArrowLines, "resized_arrow_lines"),
        (StrokeCap::ArrowEquilateral, "resized_arrow_equilateral"),
        (StrokeCap::TriangleFilled, "resized_triangle_filled"),
        (StrokeCap::DiamondFilled, "resized_diamond_filled"),
        (StrokeCap::CircleFilled, "resized_circle_filled"),
    ] {
        let seg = Path { cmds: vec![PathCmd::MoveTo(Vec2::new(4.0, 15.0)), PathCmd::LineTo(Vec2::new(26.0, 15.0))], fill_rule: FillRule::NonZero };
        let net = VectorNetwork::from_paths(std::slice::from_ref(&seg));
        let stroke = Stroke {
            paints: vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))],
            weight: 2.0,
            align: StrokeAlign::Center,
            cap,
            join: StrokeJoin::Round,
            ..Stroke::default()
        };
        let bake = designos_render::vectorops::outline_stroke(&net.to_paths(), &stroke, Some(&net));
        let mut t = T::new();
        let id = t.add(None, |id| {
            let mut n = Node::new(id, NodeKind::Vector { paths: vec![seg], network: Some(net) });
            n.transform = Affine::translate(20.0, 20.0);
            n.size = Vec2::new(30.0, 30.0);
            n.stroke = stroke;
            n.stroke_geometry = Some(bake);
            n
        });
        resize(&mut t.doc, id, Vec2::new(60.0, 60.0));
        assert!(t.doc.get(id).unwrap().stroke_geometry.is_none(), "{name}: drops baked stroke_geometry on 2x resize");

        let cpu = cpu_render(&t);
        assert!((62..=74).any(|x| is_ink(&cpu, x, 46)), "CPU {name}: head ink at 1x head span (y = 46)");
        assert!(!(60..=76).any(|x| is_ink(&cpu, x, 39)), "CPU {name}: head does not scale 2x to y = 39");

        let Some(mut g) = gpu() else { return };
        let c = compare(&mut g, name, &t.doc, t.page, Viewport::default(), VIEW, 8);
        assert!((62..=74).any(|x| is_ink(&c.gpu, x, 46)), "GPU {name}: head ink at 1x head span (y = 46)");
        assert!(!(60..=76).any(|x| is_ink(&c.gpu, x, 39)), "GPU {name}: head does not scale 2x to y = 39");
        assert!(c.report_aa.mismatched <= 10, "{name}: expected <= 10 AA-forgiven mismatched pixels on 2px diagonal wings, got {}", c.report_aa.mismatched);
        assert!(c.report.ratio < 0.005, "{name}: mismatch {:.3}%", c.report.ratio * 100.0);
        assert!(c.report.ssim > 0.99, "{name}: {:?}", c.report);
    }
}

fn figma_arrow_lines_bake(len: f32) -> Path {
    Path {
        cmds: vec![
            PathCmd::MoveTo(Vec2::new(-0.35355338, -0.35355338)),
            PathCmd::CubicTo(Vec2::new(-0.54881555, -0.15829125), Vec2::new(-0.54881555, 0.15829125), Vec2::new(-0.35355338, 0.35355338)),
            PathCmd::LineTo(Vec2::new(2.828427, 3.535534)),
            PathCmd::CubicTo(Vec2::new(3.0236893, 3.730796), Vec2::new(3.3402717, 3.730796), Vec2::new(3.535534, 3.535534)),
            PathCmd::CubicTo(Vec2::new(3.730796, 3.3402717), Vec2::new(3.730796, 3.0236893), Vec2::new(3.535534, 2.828427)),
            PathCmd::LineTo(Vec2::new(0.70710677, 0.0)),
            PathCmd::LineTo(Vec2::new(3.535534, -2.828427)),
            PathCmd::CubicTo(Vec2::new(3.730796, -3.0236893), Vec2::new(3.730796, -3.3402717), Vec2::new(3.535534, -3.535534)),
            PathCmd::CubicTo(Vec2::new(3.3402717, -3.730796), Vec2::new(3.0236893, -3.730796), Vec2::new(2.828427, -3.535534)),
            PathCmd::LineTo(Vec2::new(-0.35355338, -0.35355338)),
            PathCmd::Close,
            PathCmd::MoveTo(Vec2::new(len + 0.35355338, 0.35355338)),
            PathCmd::CubicTo(Vec2::new(len + 0.54881555, 0.15829125), Vec2::new(len + 0.54881555, -0.15829125), Vec2::new(len + 0.35355338, -0.35355338)),
            PathCmd::LineTo(Vec2::new(len - 2.828427, -3.535534)),
            PathCmd::CubicTo(Vec2::new(len - 3.0236893, -3.730796), Vec2::new(len - 3.3402717, -3.730796), Vec2::new(len - 3.535534, -3.535534)),
            PathCmd::CubicTo(Vec2::new(len - 3.730796, -3.3402717), Vec2::new(len - 3.730796, -3.0236893), Vec2::new(len - 3.535534, -2.828427)),
            PathCmd::LineTo(Vec2::new(len - 0.70710677, 0.0)),
            PathCmd::LineTo(Vec2::new(len - 3.535534, 2.828427)),
            PathCmd::CubicTo(Vec2::new(len - 3.730796, 3.0236893), Vec2::new(len - 3.730796, 3.3402717), Vec2::new(len - 3.535534, 3.535534)),
            PathCmd::CubicTo(Vec2::new(len - 3.3402717, 3.730796), Vec2::new(len - 3.0236893, 3.730796), Vec2::new(len - 2.828427, 3.535534)),
            PathCmd::LineTo(Vec2::new(len + 0.35355338, 0.35355338)),
            PathCmd::Close,
            PathCmd::MoveTo(Vec2::new(0.0, 0.5)),
            PathCmd::LineTo(Vec2::new(len, 0.5)),
            PathCmd::LineTo(Vec2::new(len, -0.5)),
            PathCmd::LineTo(Vec2::new(0.0, -0.5)),
            PathCmd::LineTo(Vec2::new(0.0, 0.5)),
            PathCmd::Close,
        ],
        fill_rule: FillRule::NonZero,
    }
}

fn stitch_vertical(panels: &[&Pixmap]) -> Pixmap {
    let w = panels.iter().map(|p| p.width()).max().unwrap_or(1);
    let sep = 2u32;
    let total_h: u32 = panels.iter().map(|p| p.height()).sum::<u32>() + sep * (panels.len().saturating_sub(1) as u32);
    let mut out = Pixmap::new(w, total_h).unwrap();
    out.fill(designos_render::tiny_skia::Color::from_rgba8(220, 224, 230, 255));
    let mut y_off = 0u32;
    for p in panels {
        for y in 0..p.height() {
            for x in 0..p.width() {
                out.pixels_mut()[((y_off + y) * w + x) as usize] = p.pixels()[(y * p.width() + x) as usize];
            }
        }
        y_off += p.height() + sep;
    }
    out
}

#[test]
fn kit_arrow_vectors_match_figma_bake_at_1x_and_keep_head_size_when_resized_2x_on_cpu_and_gpu() {
    let crops_dir = std::env::var("DESIGNOS_KIT_ARROW_CROPS_DIR").ok();
    let vp = Viewport { offset: Vec2::ZERO, zoom: 2.0, ..Viewport::default() };
    for (slug, len) in [("1143-83664", 80.0_f32), ("8222-316128", 23.999998_f32), ("7749-50847", 23.999998_f32)] {
        let seg = Path { cmds: vec![PathCmd::MoveTo(Vec2::new(0.0, 0.0)), PathCmd::LineTo(Vec2::new(len, 0.0))], fill_rule: FillRule::NonZero };
        let net = VectorNetwork::from_paths(std::slice::from_ref(&seg));
        let stroke = Stroke {
            paints: vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))],
            weight: 1.0,
            align: StrokeAlign::Center,
            cap: StrokeCap::ArrowLines,
            join: StrokeJoin::Miter,
            ..Stroke::default()
        };
        let figma_bake = figma_arrow_lines_bake(len);
        let view_1x = (((len + 16.0) * 2.0).ceil() as u32, 36u32);
        let view_2x = (((len * 2.0 + 16.0) * 2.0).ceil() as u32, 48u32);

        let mut t_bake_1x = T::new();
        let id_head = t_bake_1x.add(None, |id| {
            let mut n = Node::new(id, NodeKind::Vector { paths: vec![seg.clone()], network: Some(net.clone()) });
            n.transform = Affine::translate(8.0, 9.0);
            n.size = Vec2::new(len, 0.0);
            n.stroke = stroke.clone();
            n.stroke_geometry = Some(vec![figma_bake.clone()]);
            n
        });
        let mut t_live_1x = T::new();
        t_live_1x.add(None, |id| {
            let mut n = Node::new(id, NodeKind::Vector { paths: vec![seg.clone()], network: Some(net.clone()) });
            n.transform = Affine::translate(8.0, 9.0);
            n.size = Vec2::new(len, 0.0);
            n.stroke = stroke.clone();
            n
        });

        // Base behaviour on 2x resize (unmodelled cap kept its scaled bake via `apply_constrained_geometry`):
        let mut t_base_2x = T::new();
        let id_base = t_base_2x.add(None, |id| {
            let mut n = Node::new(id, NodeKind::Vector { paths: vec![seg.clone()], network: Some(net.clone()) });
            n.transform = Affine::translate(8.0, 12.0);
            n.size = Vec2::new(len, 0.0);
            n.stroke = Stroke { cap: StrokeCap::None, align: StrokeAlign::Inside, dashes: vec![4.0, 2.0], ..stroke.clone() };
            n.stroke_geometry = Some(vec![figma_bake]);
            n
        });
        resize(&mut t_base_2x.doc, id_base, Vec2::new(len * 2.0, 0.0));
        assert!(t_base_2x.doc.get(id_base).unwrap().stroke_geometry.is_some(), "unmodelled base keeps scaled bake on 2x resize");

        let (Some(mut g1), Some(mut g2), Some(mut g3), Some(mut g4)) = (gpu(), gpu(), gpu(), gpu()) else { return };
        let c_bake_1x = compare(&mut g1, &format!("kit_{slug}_bake_1x"), &t_bake_1x.doc, t_bake_1x.page, vp, view_1x, 8);
        let c_live_1x = compare(&mut g2, &format!("kit_{slug}_live_1x"), &t_live_1x.doc, t_live_1x.page, vp, view_1x, 8);
        assert!(c_live_1x.report_aa.mismatched <= 10, "{slug} 1x live: expected <= 10 AA-forgiven mismatched pixels on 2px wings");
        assert!(c_live_1x.report.ssim > 0.99, "{slug} 1x live: {:?}", c_live_1x.report);
        assert!(c_bake_1x.cpu.data() == c_live_1x.cpu.data(), "{slug} 1x CPU: live-drawn ArrowLines matches Figma's baked strokeGeometry pixel-for-pixel");
        assert!(c_bake_1x.gpu.data() == c_live_1x.gpu.data(), "{slug} 1x GPU: live-drawn ArrowLines matches Figma's baked strokeGeometry pixel-for-pixel");

        // Now resize `t_bake_1x` 2x on head (drops bake and draws live at weight = 1.0):
        t_bake_1x.node(id_head).transform = Affine::translate(8.0, 12.0);
        resize(&mut t_bake_1x.doc, id_head, Vec2::new(len * 2.0, 0.0));
        assert!(t_bake_1x.doc.get(id_head).unwrap().stroke_geometry.is_none(), "{slug}: head drops bake on 2x resize");

        let c_base_2x = compare(&mut g3, &format!("kit_{slug}_base_2x"), &t_base_2x.doc, t_base_2x.page, vp, view_2x, 8);
        let c_head_2x = compare(&mut g4, &format!("kit_{slug}_head_2x"), &t_bake_1x.doc, t_bake_1x.page, vp, view_2x, 8);
        assert!(c_head_2x.report_aa.mismatched <= 10, "{slug} 2x head: expected <= 10 AA-forgiven mismatched pixels on 2px wings");
        assert!(c_head_2x.report.ssim > 0.99, "{slug} 2x head: {:?}", c_head_2x.report);
        assert!(c_base_2x.cpu.data() != c_head_2x.cpu.data(), "{slug} 2x: live head keeps 1.0px stroke weight and head size instead of scaling the bake 2x");

        if let Some(dir) = &crops_dir {
            let p1 = stitch_vertical(&[&c_bake_1x.cpu, &c_live_1x.cpu, &c_bake_1x.gpu, &c_live_1x.gpu]);
            p1.save_png(format!("{dir}/kit-{slug}-1x-bake-vs-live.png")).unwrap();
            let p2 = stitch_vertical(&[&c_base_2x.cpu, &c_head_2x.cpu, &c_base_2x.gpu, &c_head_2x.gpu]);
            p2.save_png(format!("{dir}/kit-{slug}-2x-base-vs-head.png")).unwrap();
        }
    }
}

#[test]
fn per_vertex_round_and_square_caps_paint_full_span_on_cpu_and_gpu() {
    let seg = Path { cmds: vec![PathCmd::MoveTo(Vec2::new(10.0, 20.0)), PathCmd::LineTo(Vec2::new(60.0, 20.0))], fill_rule: FillRule::NonZero };
    for (cap, name) in [(StrokeCap::Square, "per_vertex_square_cap"), (StrokeCap::Round, "per_vertex_round_cap")] {
        let mut net = VectorNetwork::from_paths(std::slice::from_ref(&seg));
        for v in &mut net.vertices {
            v.cap = Some(cap);
        }
        let mut t = T::new();
        t.add(None, |id| {
            let mut n = Node::new(id, NodeKind::Vector { paths: net.to_paths(), network: Some(net) });
            n.size = Vec2::new(80.0, 40.0);
            n.stroke = Stroke {
                paints: vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))],
                weight: 12.0,
                align: StrokeAlign::Center,
                cap: StrokeCap::None,
                join: StrokeJoin::Round,
                ..Stroke::default()
            };
            n
        });
        let cpu = cpu_render(&t);
        assert_eq!(horizontal_runs(&cpu, 20, 0..80), vec![62], "CPU {name}: paints x = 4..66 (62px run) on y = 20");
        assert!(is_ink(&cpu, 5, 20) && is_ink(&cpu, 64, 20), "CPU {name}: both start (x=5) and end (x=64) caps are painted outward");

        let Some(mut g) = gpu() else { return };
        let c = compare(&mut g, name, &t.doc, t.page, Viewport::default(), VIEW, 8);
        assert_eq!(horizontal_runs(&c.gpu, 20, 0..80), vec![62], "GPU {name}: paints x = 4..66 (62px run) on y = 20");
        assert!(is_ink(&c.gpu, 5, 20) && is_ink(&c.gpu, 64, 20), "GPU {name}: both start (x=5) and end (x=64) caps are painted outward");
        assert_eq!(c.report_aa.mismatched, 0, "{name}: 0 AA-forgiven mismatched pixels between CPU and GPU");
        assert!(c.report.ratio < 0.005, "{name}: mismatch {:.3}%", c.report.ratio * 100.0);
        assert!(c.report.ssim > 0.99, "{name}: {:?}", c.report);
    }
}

/// An open vector along `cmds` of weight `weight` and `join`, whose first and last vertex carry `caps` (`Stroke::cap` stays `None`).
fn open_vector_with_vertex_caps(t: &mut T, cmds: Vec<PathCmd>, caps: (StrokeCap, StrokeCap), weight: f32, join: StrokeJoin) -> NodeId {
    let path = Path { cmds, fill_rule: FillRule::NonZero };
    let mut net = VectorNetwork::from_paths(std::slice::from_ref(&path));
    let last = net.vertices.len() - 1;
    net.vertices[0].cap = Some(caps.0);
    net.vertices[last].cap = Some(caps.1);
    t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Vector { paths: vec![path], network: Some(net) });
        n.size = Vec2::new(240.0, 120.0);
        n.fills.clear();
        n.stroke = Stroke {
            paints: vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))],
            weight,
            align: StrokeAlign::Center,
            cap: StrokeCap::None,
            join,
            ..Stroke::default()
        };
        n
    })
}

/// Columns of row `y` in `x_range` that hold no ink.
fn unpainted_columns(pm: &Pixmap, y: u32, x_range: std::ops::Range<u32>) -> Vec<u32> {
    x_range.filter(|x| !is_ink(pm, *x, y)).collect()
}

#[test]
fn a_filled_arrow_head_overlaps_its_shaft_with_no_gap_on_cpu_and_gpu() {
    // each shape has a straight run along one row; the pixels of that row, from the start to the end, hold ink
    // (3px next to a head tip and 2px next to a butt end are left out)
    let line = (vec![PathCmd::MoveTo(Vec2::new(10.0, 60.5)), PathCmd::LineTo(Vec2::new(210.0, 60.5))], 60, 10u32, 210u32);
    let curve_then_line = (
        vec![
            PathCmd::MoveTo(Vec2::new(20.0, 100.0)),
            PathCmd::CubicTo(Vec2::new(40.0, 30.0), Vec2::new(80.0, 30.0), Vec2::new(110.0, 70.0)),
            PathCmd::LineTo(Vec2::new(230.0, 70.0)),
        ],
        70,
        112u32,
        230u32,
    );
    for (shape, (cmds, row, run_start, run_end)) in [("line", line), ("curve_then_line", curve_then_line)] {
        for (cap, cap_name) in [(StrokeCap::ArrowEquilateral, "equilateral"), (StrokeCap::TriangleFilled, "triangle")] {
            for (caps, setup) in [((StrokeCap::None, cap), "end"), ((cap, StrokeCap::None), "start"), ((cap, cap), "both")] {
                let mut t = T::new();
                open_vector_with_vertex_caps(&mut t, cmds.clone(), caps, 4.0, StrokeJoin::Round);
                // the run starts at the path start only on the line; on the curved shape it starts where the straight tail does
                let from = run_start + if shape == "line" && caps.0 != StrokeCap::None { 3 } else { 2 };
                let to = run_end - if caps.1 != StrokeCap::None { 3 } else { 2 };
                let label = format!("{shape}_{cap_name}_{setup}");
                let cpu = cpu_render(&t);
                let gaps = unpainted_columns(&cpu, row, from..to);
                assert!(gaps.is_empty(), "CPU {label}: {} unpainted pixels on the centerline row, first at x = {:?}", gaps.len(), gaps.first());

                let Some(mut g) = gpu() else { return };
                let c = compare(&mut g, &label, &t.doc, t.page, Viewport::default(), VIEW, 8);
                let gaps = unpainted_columns(&c.gpu, row, from..to);
                assert!(gaps.is_empty(), "GPU {label}: {} unpainted pixels on the centerline row, first at x = {:?}", gaps.len(), gaps.first());
                // a curved edge antialiases differently on the two renderers; no pixel differs beyond that
                assert_eq!(c.report_aa.mismatched, 0, "{label}: AA-forgiven mismatched pixels between CPU and GPU");
                assert!(c.report.ratio < 0.01, "{label}: mismatch {:.3}%", c.report.ratio * 100.0);
                assert!(c.report.ssim > 0.99, "{label}: {:?}", c.report);
            }
        }
    }
}

/// Columns from the last one with ink on `rows` back to the first column holding the widest vertical run of ink.
fn head_length_to_base(pm: &Pixmap, rows: std::ops::Range<u32>, x_range: std::ops::Range<u32>) -> u32 {
    let widest = |x: u32| vertical_runs(pm, x, 0..pm.height()).into_iter().max().unwrap_or(0);
    let tip = x_range.clone().rev().find(|x| rows.clone().any(|y| is_ink(pm, *x, y))).map_or(0, |x| x + 1);
    let max = x_range.clone().map(widest).max().unwrap_or(0);
    tip - x_range.into_iter().find(|x| widest(*x) == max).unwrap_or(tip)
}

#[test]
fn a_resized_inside_or_outside_open_arrow_stroke_drops_its_bake_and_keeps_a_head_of_five_weights_on_cpu_and_gpu() {
    for (align, align_name) in [(StrokeAlign::Inside, "inside"), (StrokeAlign::Outside, "outside")] {
        let seg = Path { cmds: vec![PathCmd::MoveTo(Vec2::new(4.0, 15.0)), PathCmd::LineTo(Vec2::new(26.0, 15.0))], fill_rule: FillRule::NonZero };
        let net = VectorNetwork::from_paths(std::slice::from_ref(&seg));
        let stroke = Stroke {
            paints: vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))],
            weight: 2.0,
            align,
            cap: StrokeCap::ArrowEquilateral,
            join: StrokeJoin::Round,
            ..Stroke::default()
        };
        let bake = designos_render::vectorops::outline_stroke(&net.to_paths(), &stroke, Some(&net));
        let mut t = T::new();
        let id = t.add(None, |id| {
            let mut n = Node::new(id, NodeKind::Vector { paths: vec![seg], network: Some(net) });
            n.transform = Affine::translate(20.0, 20.0);
            n.size = Vec2::new(30.0, 30.0);
            n.stroke = stroke;
            n.stroke_geometry = Some(bake);
            n
        });
        resize(&mut t.doc, id, Vec2::new(60.0, 60.0));
        assert!(t.doc.get(id).unwrap().stroke_geometry.is_none(), "{align_name}: the baked outline is dropped on a 2x resize");

        // the line runs x = 28..72 at y = 50 and the head at its end is 5 * 2 = 10px long, not the 20px of a 2x scaled bake
        let cpu = cpu_render(&t);
        let length = head_length_to_base(&cpu, 49..51, 45..90);
        assert!((9..=11).contains(&length), "CPU {align_name}: the head is {length}px long, expected 10");

        let Some(mut g) = gpu() else { return };
        let label = format!("resized_open_arrow_{align_name}");
        let c = compare(&mut g, &label, &t.doc, t.page, Viewport::default(), VIEW, 8);
        let length = head_length_to_base(&c.gpu, 49..51, 45..90);
        assert!((9..=11).contains(&length), "GPU {align_name}: the head is {length}px long, expected 10");
        assert!(c.report.ratio < 0.005, "{label}: mismatch {:.3}%", c.report.ratio * 100.0);
        assert!(c.report.ssim > 0.99, "{label}: {:?}", c.report);
    }
}

/// Ink in the square of `half` px around `(cx, cy)`, as a sum of pixel coverage (the stroke colour has red 0x18).
fn ink_amount(pm: &Pixmap, cx: f32, cy: f32, half: f32) -> f32 {
    let (x0, x1, y0, y1) = ((cx - half).floor() as u32, (cx + half).ceil() as u32, (cy - half).floor() as u32, (cy + half).ceil() as u32);
    (y0..y1).flat_map(|y| (x0..x1).map(move |x| (x, y))).map(|(x, y)| (255.0 - pm.pixel(x, y).unwrap().red() as f32) / (255.0 - 0x18 as f32)).sum()
}

/// The area a circle of radius `r` takes off a corner of interior angle `angle` (radians).
fn rounded_off_area(r: f32, angle: f32) -> f32 {
    r * r * (1.0 / (angle / 2.0).tan() - (std::f32::consts::PI - angle) / 2.0)
}

#[test]
fn a_round_join_rounds_the_corners_of_the_filled_heads_and_a_miter_join_keeps_them_sharp_on_cpu_and_gpu() {
    let w = 9.0_f32;
    let hw = 5.0 / 3.0_f32.sqrt() * w;
    let r = w / 3.0;
    let scene = |end_x: f32, cap: StrokeCap, join: StrokeJoin| {
        let mut t = T::new();
        let cmds = vec![PathCmd::MoveTo(Vec2::new(20.0, 50.5)), PathCmd::LineTo(Vec2::new(end_x, 50.5))];
        open_vector_with_vertex_caps(&mut t, cmds, (StrokeCap::None, cap), w, join);
        t
    };
    let mut failed = vec![];
    // (cap, end x, probe pixel at the sharp corner, probe pixel inside the fillet, corners with their interior angle and the window half-size)
    let tip = std::f32::consts::FRAC_PI_3;
    let right = std::f32::consts::FRAC_PI_2;
    let diamond_end = 145.5 - hw;
    let outer = ((hw - 0.5 * w) / ((hw - 0.5 * w).powi(2) + (5.0 * w).powi(2)).sqrt()).acos();
    // the diamond's back corner lies on the shaft, which covers the part that rounding takes off
    #[allow(clippy::type_complexity)]
    let cases: Vec<(StrokeCap, f32, &str, Option<((u32, u32), (u32, u32))>, Vec<(&str, (f32, f32), f32, f32)>)> = vec![
        (
            StrokeCap::ArrowEquilateral,
            120.0,
            "equilateral",
            Some(((118, 50), (115, 50))),
            vec![("tip", (120.0, 50.5), tip, 7.0), ("upper base corner", (75.0, 50.5 - hw), tip, 7.0), ("lower base corner", (75.0, 50.5 + hw), tip, 7.0)],
        ),
        (
            StrokeCap::DiamondFilled,
            diamond_end,
            "diamond",
            Some(((144, 50), (142, 50))),
            vec![
                ("front", (diamond_end + hw, 50.5), right, 5.0),
                ("top", (diamond_end, 50.5 - hw), right, 5.0),
                ("bottom", (diamond_end, 50.5 + hw), right, 5.0),
            ],
        ),
        (
            StrokeCap::TriangleFilled,
            120.0,
            "triangle",
            None,
            vec![("upper outer corner", (120.0, 50.5 - hw), outer, 7.0), ("lower outer corner", (120.0, 50.5 + hw), outer, 7.0)],
        ),
    ];
    for (cap, end_x, name, probes, corners) in cases {
        let (sharp_t, round_t) = (scene(end_x, cap, StrokeJoin::Miter), scene(end_x, cap, StrokeJoin::Round));
        // a GPU keeps geometry by node id, so every scene gets a GPU of its own
        let Some(mut g) = gpu() else { return };
        let sharp = compare(&mut g, &format!("{name}_head_miter"), &sharp_t.doc, sharp_t.page, Viewport::default(), VIEW, 8);
        let Some(mut g) = gpu() else { return };
        let round = compare(&mut g, &format!("{name}_head_round"), &round_t.doc, round_t.page, Viewport::default(), VIEW, 8);
        for (renderer, sharp_pm, round_pm) in [("CPU", &sharp.cpu, &round.cpu), ("GPU", &sharp.gpu, &round.gpu)] {
            if let Some((apex, inside)) = probes {
                if !is_ink(sharp_pm, apex.0, apex.1) {
                    failed.push(format!("{renderer} {name}, Miter: the pixel at the sharp corner {apex:?} is unpainted"));
                }
                if is_ink(round_pm, apex.0, apex.1) {
                    failed.push(format!("{renderer} {name}, Round: the pixel at the sharp corner {apex:?} is painted"));
                }
                if !is_ink(round_pm, inside.0, inside.1) {
                    failed.push(format!("{renderer} {name}, Round: the pixel inside the fillet {inside:?} is unpainted"));
                }
            }
            for (corner, (cx, cy), angle, half) in &corners {
                let cut = ink_amount(sharp_pm, *cx, *cy, *half) - ink_amount(round_pm, *cx, *cy, *half);
                let want = rounded_off_area(r, *angle);
                if (cut - want).abs() >= 0.35 * want {
                    failed.push(format!("{renderer} {name} {corner}: Round takes {cut:.2}px of ink off the corner, expected {want:.2}"));
                }
            }
        }
        for (label, c) in [("miter", &sharp), ("round", &round)] {
            if c.report.ratio >= 0.01 || c.report.ssim <= 0.99 {
                failed.push(format!("{name} {label}: CPU and GPU differ: {:.3}% mismatched, {:?}", c.report.ratio * 100.0, c.report));
            }
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}
