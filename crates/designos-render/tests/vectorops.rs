//! Geometry tests for outline stroke, flatten and the shape-builder arrangement.
//! Areas are measured by flattening the result and combining it, so they check
//! the filled region itself, not the path commands.

use designos_doc::vector::VectorNetwork;
use designos_doc::*;
use designos_render::vectorops::*;

fn v(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

fn line(a: Vec2, b: Vec2) -> Path {
    Path { cmds: vec![PathCmd::MoveTo(a), PathCmd::LineTo(b)], fill_rule: FillRule::NonZero }
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Path {
    Path {
        cmds: vec![PathCmd::MoveTo(v(x, y)), PathCmd::LineTo(v(x + w, y)), PathCmd::LineTo(v(x + w, y + h)), PathCmd::LineTo(v(x, y + h)), PathCmd::Close],
        fill_rule: FillRule::NonZero,
    }
}

fn stroke(weight: f32) -> Stroke {
    Stroke { weight, miter_limit: 4.0, align: StrokeAlign::Center, ..Default::default() }
}

fn area(paths: &[Path]) -> f64 {
    paths_area(paths, 0.02)
}

fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "{a} vs {b}");
}

#[test]
fn outline_of_a_line_honours_weight_and_caps() {
    let l = [line(v(0.0, 0.0), v(100.0, 0.0))];
    close(area(&outline_stroke(&l, &stroke(10.0), None)), 1000.0, 0.5);
    let sq = Stroke { cap: StrokeCap::Square, ..stroke(10.0) };
    close(area(&outline_stroke(&l, &sq, None)), 1100.0, 0.5);
    let round = Stroke { cap: StrokeCap::Round, ..stroke(10.0) };
    close(area(&outline_stroke(&l, &round, None)), 1000.0 + std::f64::consts::PI * 25.0, 1.0);
    assert!(outline_stroke(&l, &stroke(0.0), None).is_empty(), "no weight, no outline");
}

#[test]
fn outline_of_a_closed_rectangle_by_alignment() {
    let r = [rect(0.0, 0.0, 100.0, 50.0)];
    close(area(&outline_stroke(&r, &stroke(10.0), None)), 110.0 * 60.0 - 90.0 * 40.0, 1.0);
    let inside = Stroke { align: StrokeAlign::Inside, ..stroke(10.0) };
    close(area(&outline_stroke(&r, &inside, None)), 5000.0 - 80.0 * 30.0, 1.0);
    let outside = Stroke { align: StrokeAlign::Outside, ..stroke(10.0) };
    close(area(&outline_stroke(&r, &outside, None)), 120.0 * 70.0 - 5000.0, 1.0);
}

#[test]
fn joins_change_the_outer_corner() {
    let r = [rect(0.0, 0.0, 100.0, 100.0)];
    let miter = area(&outline_stroke(&r, &stroke(20.0), None));
    let bevel = area(&outline_stroke(&r, &Stroke { join: StrokeJoin::Bevel, ..stroke(20.0) }, None));
    let round = area(&outline_stroke(&r, &Stroke { join: StrokeJoin::Round, ..stroke(20.0) }, None));
    close(miter, 120.0 * 120.0 - 80.0 * 80.0, 1.0);
    close(bevel, miter - 4.0 * 0.5 * 10.0 * 10.0, 1.0);
    close(round, miter - 4.0 * (100.0 - std::f64::consts::PI * 100.0 / 4.0), 1.5);
}

#[test]
fn dashes_split_the_outline() {
    let l = [line(v(0.0, 0.0), v(100.0, 0.0))];
    let st = Stroke { dashes: vec![10.0, 10.0], ..stroke(4.0) };
    let out = outline_stroke(&l, &st, None);
    close(area(&out), 5.0 * 10.0 * 4.0, 1.0);
    let dashes = out.iter().flat_map(|p| &p.cmds).filter(|c| matches!(c, PathCmd::MoveTo(_))).count();
    assert_eq!(dashes, 5, "five separate dashes");
}

#[test]
fn outline_of_a_curve_keeps_curves() {
    let c = [Path { cmds: vec![PathCmd::MoveTo(v(0.0, 0.0)), PathCmd::CubicTo(v(0.0, 80.0), v(100.0, 80.0), v(100.0, 0.0))], fill_rule: FillRule::NonZero }];
    let out = outline_stroke(&c, &stroke(6.0), None);
    assert!(out.iter().flat_map(|p| &p.cmds).any(|c| matches!(c, PathCmd::QuadTo(..) | PathCmd::CubicTo(..))), "no polygon approximation");
    // length of the curve times the weight, within a few percent
    let mut len = 0.0f32;
    for (pts, _) in designos_doc::vector::flatten_path(&c[0], 0.01) {
        len += pts.windows(2).map(|w| ((w[1].x - w[0].x).powi(2) + (w[1].y - w[0].y).powi(2)).sqrt()).sum::<f32>();
    }
    close(area(&out), (len * 6.0) as f64, (len * 6.0 * 0.03) as f64);
}

#[test]
fn per_vertex_caps_override_the_node_cap() {
    let net = VectorNetwork::from_paths(&[line(v(0.0, 0.0), v(100.0, 0.0))]);
    let mut net = net;
    net.vertices[1].cap = Some(StrokeCap::Round);
    let st = stroke(10.0); // node cap: none
    let out = outline_stroke(&net.to_paths(), &st, Some(&net));
    close(area(&out), 1000.0 + std::f64::consts::PI * 25.0 / 2.0, 1.5);
    net.vertices[0].cap = Some(StrokeCap::Square);
    let out = outline_stroke(&net.to_paths(), &st, Some(&net));
    close(area(&out), 1000.0 + std::f64::consts::PI * 25.0 / 2.0 + 50.0, 1.5);
    // an explicit cap overrides a round node cap
    let net2 = VectorNetwork::from_paths(&[line(v(0.0, 0.0), v(100.0, 0.0))]);
    let mut net2 = net2;
    net2.vertices[0].cap = Some(StrokeCap::None);
    net2.vertices[1].cap = Some(StrokeCap::None);
    let round = Stroke { cap: StrokeCap::Round, ..stroke(10.0) };
    close(area(&outline_stroke(&net2.to_paths(), &round, Some(&net2))), 1000.0, 0.5);
}

// ---------------------------------------------------------------- flatten
fn doc_with(f: impl FnOnce(&mut Document, NodeId)) -> (Document, NodeId) {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    f(&mut d, page);
    (d, page)
}

fn add(d: &mut Document, parent: NodeId, n: Node) -> NodeId {
    d.insert(parent, n).unwrap()
}

#[test]
fn flatten_a_subtract_boolean_group_computes_the_geometry() {
    let (d, page) = doc_with(|d, page| {
        let b = d.alloc_id();
        let mut bn = Node::new(b, NodeKind::BooleanOp(BooleanOp::Subtract));
        bn.size = v(100.0, 100.0);
        let g = add(d, page, bn);
        let r1 = d.alloc_id();
        add(d, g, Node::rectangle(r1, Rect::new(0.0, 0.0, 100.0, 100.0)));
        let r2 = d.alloc_id();
        add(d, g, Node::rectangle(r2, Rect::new(50.0, 50.0, 100.0, 100.0)));
    });
    let g = d.get(page).unwrap().children[0];
    let f = flatten_nodes(&d, &[g], &Affine::IDENTITY).unwrap();
    close(area(&f.paths), 10000.0 - 2500.0, 1.0);
    // an L shape: six corners and nothing curved
    let corners: usize = f.paths.iter().flat_map(|p| &p.cmds).filter(|c| matches!(c, PathCmd::LineTo(_) | PathCmd::MoveTo(_))).count();
    assert_eq!(corners, 6);
}

#[test]
fn flatten_merges_a_selection_in_the_target_space() {
    let (d, page) = doc_with(|d, page| {
        let f = d.alloc_id();
        let frame = add(d, page, Node::frame(f, Rect::new(100.0, 100.0, 300.0, 300.0)));
        let a = d.alloc_id();
        add(d, frame, Node::rectangle(a, Rect::new(10.0, 10.0, 50.0, 50.0)));
        let b = d.alloc_id();
        add(d, frame, Node::ellipse(b, Rect::new(100.0, 100.0, 40.0, 40.0)));
    });
    let frame = d.get(page).unwrap().children[0];
    let kids = d.get(frame).unwrap().children.clone();
    // into the frame's world: the shapes keep their local coordinates
    let f = flatten_nodes(&d, &kids, &d.world_transform(frame)).unwrap();
    assert_eq!(f.style_from, 1, "the topmost shape donates its style");
    let b = designos_doc::vector::paths_bounds(&f.paths);
    close(b.x as f64, 10.0, 0.1);
    close(b.w as f64, 130.0, 0.5);
    close(area(&f.paths), 2500.0 + std::f64::consts::PI * 400.0, 3.0);
    // into the page's world: translated by the frame
    let g = flatten_nodes(&d, &kids, &Affine::IDENTITY).unwrap();
    close(designos_doc::vector::paths_bounds(&g.paths).x as f64, 110.0, 0.1);
    // as a network the result is editable
    let net = VectorNetwork::from_paths(&f.paths);
    assert_eq!(net.regions.len(), 2);
}

#[test]
fn flatten_skips_text_and_empty_selections() {
    let (d, page) = doc_with(|d, page| {
        let t = d.alloc_id();
        add(d, page, Node::text(t, Rect::new(0.0, 0.0, 10.0, 10.0), "hi", TextStyle::default()));
    });
    let t = d.get(page).unwrap().children[0];
    assert!(flatten_nodes(&d, &[t], &Affine::IDENTITY).is_none());
    assert!(flatten_nodes(&d, &[], &Affine::IDENTITY).is_none());
}

// ---------------------------------------------------------------- shape builder
fn shapes_of(p: &Path) -> Shapes {
    paths_to_shapes(std::slice::from_ref(p), 0.02)
}

#[test]
fn arrange_cuts_overlapping_rectangles_into_three_regions() {
    let a = shapes_of(&rect(0.0, 0.0, 100.0, 100.0));
    let b = shapes_of(&rect(50.0, 50.0, 100.0, 100.0));
    let cells = arrange(&[a, b]);
    assert_eq!(cells.len(), 3);
    let mut areas: Vec<f64> = cells.iter().map(|c| c.area).collect();
    areas.sort_by(f64::total_cmp);
    close(areas[0], 2500.0, 0.5);
    close(areas[1], 7500.0, 0.5);
    close(areas[2], 7500.0, 0.5);
    let lens = cells.iter().find(|c| c.members.len() == 2).unwrap();
    close(lens.area, 2500.0, 0.5);
    assert!(shapes_contain(&lens.shape, v(75.0, 75.0)));
    assert!(!shapes_contain(&lens.shape, v(25.0, 25.0)));
}

#[test]
fn disjoint_pieces_of_one_region_become_separate_cells() {
    // a plus-shaped cut: a wide bar crossing a tall bar leaves four arms; the wide
    // bar minus the tall one is two separate cells
    let wide = shapes_of(&rect(0.0, 40.0, 100.0, 20.0));
    let tall = shapes_of(&rect(40.0, 0.0, 20.0, 100.0));
    let cells = arrange(&[wide, tall]);
    assert_eq!(cells.len(), 5, "centre plus four arms");
}

#[test]
fn merging_cells_unions_them_and_cuts_the_remainders() {
    let a = shapes_of(&rect(0.0, 0.0, 100.0, 100.0));
    let b = shapes_of(&rect(50.0, 50.0, 100.0, 100.0));
    let originals = vec![a, b];
    let cells = arrange(&originals);
    // merge the lens with the part of A that is outside B: that is all of A
    let lens = cells.iter().position(|c| c.members.len() == 2).unwrap();
    let only_a = cells.iter().position(|c| c.members == vec![0]).unwrap();
    let mut group = vec![None; cells.len()];
    group[lens] = Some(0);
    group[only_a] = Some(0);
    let r = resolve_builder(&originals, &cells, &group, &vec![false; cells.len()]);
    assert_eq!(r.pieces.len(), 1);
    close(shapes_area(&r.pieces[0].0), 10000.0, 0.5);
    assert_eq!(r.pieces[0].1, 1, "style of the topmost member");
    close(shapes_area(r.remainders[0].as_ref().unwrap()), 0.0, 0.5);
    close(shapes_area(r.remainders[1].as_ref().unwrap()), 7500.0, 0.5);
}

#[test]
fn subtracting_the_lens_leaves_two_crescent_remainders() {
    let a = shapes_of(&rect(0.0, 0.0, 100.0, 100.0));
    let b = shapes_of(&rect(50.0, 50.0, 100.0, 100.0));
    let originals = vec![a, b];
    let cells = arrange(&originals);
    let lens = cells.iter().position(|c| c.members.len() == 2).unwrap();
    let mut deleted = vec![false; cells.len()];
    deleted[lens] = true;
    let r = resolve_builder(&originals, &cells, &vec![None; cells.len()], &deleted);
    assert!(r.pieces.is_empty());
    close(shapes_area(r.remainders[0].as_ref().unwrap()), 7500.0, 0.5);
    close(shapes_area(r.remainders[1].as_ref().unwrap()), 7500.0, 0.5);
    // untouched shapes report no change
    let none = resolve_builder(&originals, &cells, &vec![None; cells.len()], &vec![false; cells.len()]);
    assert!(none.remainders.iter().all(|r| r.is_none()) && none.pieces.is_empty());
}

#[test]
fn circles_arrange_into_a_lens_and_two_crescents_and_fit_back_to_curves() {
    let circle = |cx: f32, r: f32| {
        let k = 0.552_284_8 * r;
        Path {
            cmds: vec![
                PathCmd::MoveTo(v(cx + r, 0.0)),
                PathCmd::CubicTo(v(cx + r, k), v(cx + k, r), v(cx, r)),
                PathCmd::CubicTo(v(cx - k, r), v(cx - r, k), v(cx - r, 0.0)),
                PathCmd::CubicTo(v(cx - r, -k), v(cx - k, -r), v(cx, -r)),
                PathCmd::CubicTo(v(cx + k, -r), v(cx + r, -k), v(cx + r, 0.0)),
                PathCmd::Close,
            ],
            fill_rule: FillRule::NonZero,
        }
    };
    let originals = vec![shapes_of(&circle(0.0, 50.0)), shapes_of(&circle(60.0, 50.0))];
    let cells = arrange(&originals);
    assert_eq!(cells.len(), 3);
    let total: f64 = cells.iter().map(|c| c.area).sum();
    let union = shapes_area(&originals[0]) + shapes_area(&originals[1]) - cells.iter().find(|c| c.members.len() == 2).unwrap().area;
    close(total, union, 2.0);
    // a cell becomes few smooth curves again
    let lens = cells.iter().find(|c| c.members.len() == 2).unwrap();
    let paths = shapes_to_paths(&lens.shape, 0.1);
    let anchors = paths[0].cmds.iter().filter(|c| matches!(c, PathCmd::LineTo(_) | PathCmd::CubicTo(..))).count();
    assert!(anchors <= 10, "{anchors} segments for a lens");
    close(area(&paths), lens.area, lens.area * 0.01);
}

#[test]
fn offset_path_expands_100px_square_and_shrinks_on_negative_offset() {
    let sq = [rect(0.0, 0.0, 100.0, 100.0)];

    // AC 1: +10 on a 100 px square gives about 120^2 minus the corners
    let plus_round = offset_path(&sq, 10.0);
    let expected_round = 120.0 * 120.0 - 4.0 * (100.0 - std::f64::consts::PI * 25.0);
    close(area(&plus_round), expected_round, 2.0);

    let plus_bevel = offset_paths(&sq, 10.0, StrokeJoin::Bevel, 4.0);
    let expected_bevel = 120.0 * 120.0 - 4.0 * 50.0;
    close(area(&plus_bevel), expected_bevel, 1.5);

    let plus_miter = offset_paths(&sq, 10.0, StrokeJoin::Miter, 4.0);
    close(area(&plus_miter), 120.0 * 120.0, 1.5);

    // AC 2: A negative offset shrinks the shape
    let minus_10 = offset_path(&sq, -10.0);
    close(area(&minus_10), 80.0 * 80.0, 1.5);

    // Negative offset exceeding half the side length collapses the shape cleanly
    let collapsed = offset_path(&sq, -60.0);
    assert!(collapsed.is_empty());
}

/// The sub-paths of `paths` that are not closed, each as its command list.
fn open_subpaths(paths: &[Path]) -> Vec<Vec<PathCmd>> {
    let mut open = Vec::new();
    for p in paths {
        let mut cur: Vec<PathCmd> = Vec::new();
        for c in &p.cmds {
            if matches!(c, PathCmd::MoveTo(_)) {
                if cur.len() > 1 && !matches!(cur.last(), Some(PathCmd::Close)) {
                    open.push(std::mem::take(&mut cur));
                }
                cur.clear();
            }
            cur.push(*c);
        }
        if cur.len() > 1 && !matches!(cur.last(), Some(PathCmd::Close)) {
            open.push(cur);
        }
    }
    open
}

#[test]
fn offset_passes_an_open_segment_through_next_to_a_closed_square() {
    let seg = line(v(200.0, 0.0), v(300.0, 0.0));
    let input = [rect(0.0, 0.0, 100.0, 100.0), seg.clone()];

    let inset = offset_path(&input, -10.0);
    assert_eq!(open_subpaths(&inset), vec![seg.cmds.clone()], "an inset must keep the open segment as it was");
    close(area(&inset), 80.0 * 80.0, 1.5);

    let outset = offset_path(&input, 10.0);
    assert_eq!(open_subpaths(&outset), vec![seg.cmds.clone()], "an outset must keep the open segment as it was, not turn it into a ribbon");
    close(area(&outset), 120.0 * 120.0 - 4.0 * (100.0 - std::f64::consts::PI * 25.0), 2.0);
}

#[test]
fn offset_of_an_open_path_alone_returns_it_unchanged() {
    let l = [line(v(0.0, 0.0), v(100.0, 0.0))];
    for delta in [10.0, -10.0] {
        assert_eq!(offset_path(&l, delta), l.to_vec(), "delta {delta}");
    }
}

/// One path holding both squares as separate closed sub-paths (same winding, so they union).
fn two_overlapping_squares_in_one_path() -> Path {
    let (a, b) = (rect(0.0, 0.0, 100.0, 100.0), rect(50.0, 0.0, 100.0, 100.0));
    Path { cmds: a.cmds.into_iter().chain(b.cmds).collect(), fill_rule: FillRule::NonZero }
}

#[test]
fn inset_erodes_the_outline_of_overlapping_squares_not_their_inner_edges() {
    // the union is 150 x 100, so the inset is one 130 x 80 rectangle
    let one_path = [two_overlapping_squares_in_one_path()];
    let out = offset_path(&one_path, -10.0);
    close(area(&out), 130.0 * 80.0, 2.0);
    assert_eq!(count_subpaths(&out), (1, 0), "one region, not islands cut apart along the inner edges");

    let two_paths = [rect(0.0, 0.0, 100.0, 100.0), rect(50.0, 0.0, 100.0, 100.0)];
    let out = offset_path(&two_paths, -10.0);
    close(area(&out), 130.0 * 80.0, 2.0);
    assert_eq!(count_subpaths(&out), (1, 0));
}

#[test]
fn separate_paths_stay_separate_regions() {
    // two 100 x 100 squares in two paths, 10 px overlapping: each shrinks on its own
    let out = offset_path(&[rect(0.0, 0.0, 100.0, 100.0), rect(90.0, 0.0, 100.0, 100.0)], -10.0);
    assert_eq!(count_subpaths(&out), (2, 0), "two islands, not one merged silhouette");
    close(area(&out), 2.0 * 80.0 * 80.0, 2.0);
    // one path holding both subpaths is one filled region and shrinks as a whole
    let both = Path { cmds: rect(0.0, 0.0, 100.0, 100.0).cmds.into_iter().chain(rect(90.0, 0.0, 100.0, 100.0).cmds).collect(), fill_rule: FillRule::NonZero };
    let out = offset_path(&[both], -10.0);
    assert_eq!(count_subpaths(&out), (1, 0));
    close(area(&out), 170.0 * 80.0, 2.0);
}

#[test]
fn inset_ignores_a_path_that_lies_inside_another() {
    let nested = [rect(0.0, 0.0, 100.0, 100.0), rect(40.0, 40.0, 20.0, 20.0)];
    close(area(&offset_path(&nested, -10.0)), 80.0 * 80.0, 1.5);
    close(area(&offset_path(&nested, 10.0)), 120.0 * 120.0 - 4.0 * (100.0 - std::f64::consts::PI * 25.0), 2.0);
}

#[test]
fn offset_moves_the_boundary_of_a_hole_the_other_way() {
    let hole = Path {
        cmds: vec![
            PathCmd::MoveTo(v(30.0, 30.0)),
            PathCmd::LineTo(v(30.0, 70.0)),
            PathCmd::LineTo(v(70.0, 70.0)),
            PathCmd::LineTo(v(70.0, 30.0)),
            PathCmd::Close,
        ],
        fill_rule: FillRule::NonZero,
    };
    let outer = rect(0.0, 0.0, 100.0, 100.0);
    let holed = [Path { cmds: outer.cmds.into_iter().chain(hole.cmds).collect(), fill_rule: FillRule::NonZero }];
    close(area(&holed), 10000.0 - 1600.0, 1.0);
    // the ring around the hole is 30 wide: eroding 10 leaves 80 x 80 minus the hole grown to 60 x 60
    // with round corners (the region is pushed away from the hole by a disc)
    close(area(&offset_path(&holed, -10.0)), 6400.0 - (3600.0 - 4.0 * (100.0 - std::f64::consts::PI * 25.0)), 2.0);
    // growing 10 closes the hole down to 20 x 20
    close(area(&offset_path(&holed, 10.0)), 120.0 * 120.0 - 4.0 * (100.0 - std::f64::consts::PI * 25.0) - 400.0, 2.5);
}

#[test]
fn offset_of_a_circle_keeps_its_curve() {
    let (c, r, k) = (50.0, 50.0, 0.552_284_8 * 50.0);
    let circle = Path {
        cmds: vec![
            PathCmd::MoveTo(v(c + r, c)),
            PathCmd::CubicTo(v(c + r, c + k), v(c + k, c + r), v(c, c + r)),
            PathCmd::CubicTo(v(c - k, c + r), v(c - r, c + k), v(c - r, c)),
            PathCmd::CubicTo(v(c - r, c - k), v(c - k, c - r), v(c, c - r)),
            PathCmd::CubicTo(v(c + k, c - r), v(c + r, c - k), v(c + r, c)),
            PathCmd::Close,
        ],
        fill_rule: FillRule::NonZero,
    };
    let disc = |radius: f64| std::f64::consts::PI * radius * radius;
    close(area(&offset_path(std::slice::from_ref(&circle), 10.0)), disc(60.0), disc(60.0) * 0.005);
    close(area(&offset_path(std::slice::from_ref(&circle), -10.0)), disc(40.0), disc(40.0) * 0.005);
}

#[test]
fn a_fully_eroded_closed_part_returns_nothing_even_next_to_an_open_segment() {
    let input = [rect(0.0, 0.0, 100.0, 100.0), line(v(200.0, 0.0), v(300.0, 0.0))];
    assert!(offset_path(&input, -60.0).is_empty(), "a vanished fill must not leave only the open segment behind");
}
