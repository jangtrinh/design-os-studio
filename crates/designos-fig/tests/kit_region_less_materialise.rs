//! Kit check (ignored by default): giving the closed loops of every region-less imported vector their regions draws
//! exactly what the closed-loop fallback drew. Reads `KIT_CLONE` (or `DESIGNOS_KIT_PATH`), the kit clone; prints counts
//! and node keys only, never names.
//!
//! Each vector with a visible fill, a flagged network and no region is rasterised twice at 4x in its own box, once with
//! the fallback fill and once after `materialize_closed_loop_fill`, and the two pictures are compared byte for byte.

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

#[test]
#[ignore = "reads KIT_CLONE"]
fn materialising_every_region_less_kit_vector_draws_the_fallback_picture() {
    let kit = std::env::var_os("KIT_CLONE").or_else(|| std::env::var_os("DESIGNOS_KIT_PATH")).expect("KIT_CLONE");
    let bytes = std::fs::read(kit).unwrap();
    let mut doc = designos_fig::import_fig(&bytes).unwrap();
    doc.hydrate_all_pages();
    let (mut total, mut identical, mut kept, mut empty, mut unscored) = (0usize, 0usize, 0usize, 0usize, 0usize);
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
        after_net.materialize_closed_loop_fill();
        if after_net.fills_closed_loops_without_regions {
            // nothing could be decided with room to spare: the fallback stays, and the picture is the same by construction
            kept += 1;
            differing.push(format!("{key}: kept the fallback (undecidable)"));
            continue;
        }
        let after = after_net.fill_paths();
        let b = designos_doc::vector::paths_bounds(&before);
        let (x0, y0) = (b.x - 2.0, b.y - 2.0);
        let scale: f32 = (2048.0 / (b.w + 4.0).max(b.h + 4.0)).min(4.0);
        let (w, h) = (((b.w + 4.0) * scale).ceil() as u32, ((b.h + 4.0) * scale).ceil() as u32);
        if w == 0 || h == 0 || w > 4096 || h > 4096 {
            unscored += 1;
            continue;
        }
        let ts = sk::Transform::from_row(scale, 0.0, 0.0, scale, -x0 * scale, -y0 * scale);
        let (pa, pb) = (raster(&before, ts, w, h), raster(&after, ts, w, h));
        if pa == pb {
            identical += 1;
        } else {
            let worst = pa.iter().zip(&pb).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0);
            let px = pa.iter().zip(&pb).filter(|(x, y)| x != y).count();
            differing.push(format!("{key}: {px} px differ, worst alpha {worst}, regions {}", after_net.regions.len()));
        }
    }
    eprintln!("KIT region-less flagged vectors with a visible fill: {total}");
    eprintln!(
        "KIT identical: {identical}; fallback kept: {kept}; empty fill (open chains only): {empty}; unscored (window): {unscored}; differing: {}",
        differing.len() - kept
    );
    for d in &differing {
        eprintln!("KIT exception {d}");
    }
    assert_eq!(identical + empty + unscored + kept + (differing.len() - kept), total);
    assert!(differing.len() == kept, "pictures differ after materialising");
}
