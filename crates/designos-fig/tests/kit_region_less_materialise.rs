//! Kit check (ignored by default): giving the closed loops of every region-less imported vector their regions draws
//! exactly what the closed-loop fallback drew. Reads `KIT_CLONE` (or `DESIGNOS_KIT_PATH`), the kit clone; prints counts
//! and node keys only, never names.
//!
//! Each vector with a visible fill, a flagged network and no region is rasterised twice at 4x in its own box, once with
//! the fallback fill and once after `materialize_closed_loop_fill`, and the two pictures are compared byte for byte.
//! Expected: every one identical or empty, none kept (a refusal), none differing. The edit leg then clicks the largest
//! loop that has a face of its own and checks that no pixel outside that loop's box changed. The number of networks
//! with nested loops is printed: the size of the nested-ring bucket slice.

use designos_doc::*;
use designos_render::tiny_skia as sk;

fn to_sk(p: &Path) -> Option<sk::Path> {
    let mut b = sk::PathBuilder::new();
    for c in &p.cmds {
        match *c {
            PathCmd::MoveTo(v) => b.move_to(v.x, v.y),
            PathCmd::LineTo(v) => b.line_to(v.x, v.y),
            PathCmd::QuadTo(c1, v) => b.quad_to(c1.x, c1.y, v.x, v.y),
            PathCmd::CubicTo(c1, c2, v) => b.cubic_to(c1.x, c1.y, c2.x, c2.y, v.x, v.y),
            PathCmd::Close => b.close(),
        }
    }
    b.finish()
}

fn raster(paths: &[Path], ts: sk::Transform, w: u32, h: u32) -> Vec<u8> {
    let mut pm = sk::Pixmap::new(w, h).unwrap();
    let mut paint = sk::Paint::default();
    paint.set_color_rgba8(0, 0, 0, 255);
    paint.anti_alias = true;
    for p in paths {
        if let Some(sp) = to_sk(p) {
            let rule = if p.fill_rule == FillRule::EvenOdd { sk::FillRule::EvenOdd } else { sk::FillRule::Winding };
            pm.fill_path(&sp, &paint, rule, ts, None);
        }
    }
    pm.pixels().iter().map(|p| p.alpha()).collect()
}

fn box_of(paths: &[Path]) -> (f32, f32, f32, f32) {
    let b = designos_doc::vector::paths_bounds(paths);
    (b.x, b.y, b.x + b.w, b.y + b.h)
}

#[test]
#[ignore = "reads KIT_CLONE"]
fn materialising_every_region_less_kit_vector_draws_the_fallback_picture() {
    let kit = std::env::var_os("KIT_CLONE").or_else(|| std::env::var_os("DESIGNOS_KIT_PATH")).expect("KIT_CLONE");
    let bytes = std::fs::read(kit).unwrap();
    let mut doc = designos_fig::import_fig(&bytes).unwrap();
    doc.hydrate_all_pages();
    let (mut total, mut identical, mut refused, mut empty, mut unscored) = (0usize, 0usize, 0usize, 0usize, 0usize);
    let (mut multi, mut nested, mut edited, mut confined, mut no_face) = (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut differing: Vec<String> = Vec::new();
    for (id, n) in doc.nodes.iter() {
        let NodeKind::Vector { network: Some(net), .. } = &n.kind else { continue };
        if !net.fills_closed_loops_without_regions || !net.regions.is_empty() || !n.fills.iter().any(|f| f.visible && f.opacity > 0.0) {
            continue;
        }
        total += 1;
        let key = n.source_id.clone().unwrap_or_else(|| format!("{id:?}"));
        let before = net.fill_paths();
        if before.is_empty() {
            empty += 1;
            continue;
        }
        let mut after_net = net.clone();
        if !after_net.materialize_closed_loop_fill() || after_net.fills_closed_loops_without_regions {
            refused += 1;
            differing.push(format!("{key}: materialising was refused"));
            continue;
        }
        let after = after_net.fill_paths();
        let (bx0, by0, bx1, by1) = box_of(&before);
        let (x0, y0) = (bx0 - 2.0, by0 - 2.0);
        let scale: f32 = (2048.0 / (bx1 - bx0 + 4.0).max(by1 - by0 + 4.0)).min(4.0);
        let (w, h) = (((bx1 - bx0 + 4.0) * scale).ceil() as u32, ((by1 - by0 + 4.0) * scale).ceil() as u32);
        if w == 0 || h == 0 || w > 4096 || h > 4096 {
            unscored += 1;
            continue;
        }
        let ts = sk::Transform::from_row(scale, 0.0, 0.0, scale, -x0 * scale, -y0 * scale);
        let (pa, pb) = (raster(&before, ts, w, h), raster(&after, ts, w, h));
        if pa == pb {
            identical += 1;
        } else {
            let px = pa.iter().zip(&pb).filter(|(x, y)| x != y).count();
            differing.push(format!("{key}: {px} px differ after materialising, regions {}", after_net.regions.len()));
            continue;
        }
        // nesting: a loop whose box lies inside another loop's box
        let boxes: Vec<(f32, f32, f32, f32)> = after_net
            .regions
            .iter()
            .filter_map(|r| after_net.loop_polygon(&r.loops[0], 0.25))
            .map(|poly| poly.iter().fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |a, p| (a.0.min(p.x), a.1.min(p.y), a.2.max(p.x), a.3.max(p.y))))
            .collect();
        if boxes.len() >= 2 {
            multi += 1;
            let inside = |a: &(f32, f32, f32, f32), b: &(f32, f32, f32, f32)| a.0 >= b.0 && a.1 >= b.1 && a.2 <= b.2 && a.3 <= b.3;
            if boxes.iter().enumerate().any(|(i, a)| boxes.iter().enumerate().any(|(j, b)| i != j && inside(a, b))) {
                nested += 1;
            }
            // the edit leg: click the largest loop that has a face of its own
            let mut candidates: Vec<_> = after_net.faces().into_iter().filter(|f| f.loops.len() == 1 && after_net.region_of_face(f).is_some()).collect();
            candidates.sort_by(|a, b| b.area.total_cmp(&a.area));
            let Some(face) = candidates.into_iter().next() else {
                no_face += 1;
                continue;
            };
            let poly = after_net.face_contours(&face);
            let fb = poly[0].iter().fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |a, p| (a.0.min(p.x), a.1.min(p.y), a.2.max(p.x), a.3.max(p.y)));
            let mut clicked = after_net.clone();
            clicked.toggle_face(&face);
            let pc = raster(&clicked.fill_paths(), ts, w, h);
            let (cx0, cy0, cx1, cy1) =
                (((fb.0 - x0) * scale) as i64 - 3, ((fb.1 - y0) * scale) as i64 - 3, ((fb.2 - x0) * scale) as i64 + 3, ((fb.3 - y0) * scale) as i64 + 3);
            let mut moved_outside = 0usize;
            for yy in 0..h as i64 {
                for xx in 0..w as i64 {
                    if xx >= cx0 && xx <= cx1 && yy >= cy0 && yy <= cy1 {
                        continue;
                    }
                    let i = (yy * w as i64 + xx) as usize;
                    moved_outside += usize::from(pb[i] != pc[i]);
                }
            }
            edited += 1;
            if moved_outside == 0 {
                confined += 1;
            } else {
                differing.push(format!("{key}: the bucket moved {moved_outside} px outside the clicked loop"));
            }
        }
    }
    eprintln!("KIT region-less flagged vectors with a visible fill: {total}");
    eprintln!(
        "KIT identical: {identical}; empty fill (open chains only): {empty}; unscored (window): {unscored}; refused: {refused}; differing or unconfined: {}",
        differing.len() - refused
    );
    eprintln!("KIT networks with two or more loops: {multi}; with nested loops (a box inside a box): {nested}");
    eprintln!("KIT edit leg: clicked {edited}, confined {confined}, no loop with a face of its own: {no_face}");
    for d in &differing {
        eprintln!("KIT exception {d}");
    }
    assert_eq!(refused, 0, "no refusal");
    assert!(differing.is_empty(), "pictures differ");
    assert_eq!(identical + empty + unscored, total);
}
