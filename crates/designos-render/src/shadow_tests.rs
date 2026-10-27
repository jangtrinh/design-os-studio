//! Accuracy and cache tests for the analytic rounded-rectangle shadow.

use crate::blur::gaussian_blur_reference;
use crate::effects::drop_shadow_with;
use crate::geom::rounded_rect;
use crate::shadow::*;
use designos_doc as d;
use tiny_skia as sk;

struct Case {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    scale: f32,
    radius: f32,
    spread: f32,
    off: (f32, f32),
    fill_alpha: f32,
    show_behind: bool,
}

/// Silhouette closure mirroring `Renderer::draw_silhouette` for a solid
/// rounded rectangle.
fn sil_for(c: &Case) -> impl Fn(&mut sk::Pixmap, f32, f32, f32, d::SilhouetteMode) + '_ {
    move |pm, dx, dy, grow, mode| {
        let opaque = mode.ignores_alpha();
        let path = rounded_rect(c.w, c.h, [c.r; 4]).unwrap();
        let ts = sk::Transform::from_row(c.scale, 0.0, 0.0, c.scale, c.x, c.y).post_translate(dx, dy);
        let black = |a: f32| {
            let mut p = sk::Paint { anti_alias: true, ..Default::default() };
            p.set_color(sk::Color::from_rgba(0.0, 0.0, 0.0, a).unwrap());
            p
        };
        pm.fill_path(&path, &black(if opaque { 1.0 } else { c.fill_alpha }), sk::FillRule::Winding, ts, None);
        if grow != 0.0 {
            let st = sk::Stroke { width: grow.abs() * 2.0, line_join: sk::LineJoin::Miter, miter_limit: 4.0, ..Default::default() };
            let mut p = black(1.0);
            if grow < 0.0 {
                p.blend_mode = sk::BlendMode::Clear;
            }
            pm.stroke_path(&path, &p, &st, ts, None);
        }
    }
}

fn run(c: &Case, fast: bool) -> sk::Pixmap {
    let mut target = sk::Pixmap::new(420, 320).unwrap();
    target.fill(sk::Color::from_rgba8(240, 240, 250, 255));
    let color = d::Color { r: 0.1, g: 0.2, b: 0.6, a: 0.45 };
    let sil = sil_for(c);
    let offset = d::Vec2::new(c.off.0, c.off.1);
    let (dw, dh) = (c.w * c.scale, c.h * c.scale);
    let bbox = (c.x, c.y, c.x + dw, c.y + dh);
    if fast {
        let rr = RRect { x: c.x, y: c.y, w: dw, h: dh, r: c.r.min(c.w.min(c.h) / 2.0) * c.scale };
        if !drop_shadow_rrect(&mut target, None, rr, c.scale, &color, offset, c.radius, c.spread, c.show_behind, d::BlendMode::Normal, 1.0, c.fill_alpha, &sil)
        {
            // declined (sigma below the blur threshold): the generic path draws
            return run(c, false);
        }
    } else {
        let reference = |pm: &mut sk::Pixmap, s: f32, _o: (i32, i32)| gaussian_blur_reference(pm, s);
        drop_shadow_with(&reference, &mut target, None, bbox, c.scale, &color, offset, c.radius, c.spread, c.show_behind, d::BlendMode::Normal, 1.0, &sil);
    }
    target
}

/// Largest channel difference. With the node knocked out of its shadow,
/// pixels on the node's anti-aliased outline (and their neighbours) are
/// skipped: tiny-skia rasterises curved edges slightly differently depending
/// on the integer offset they are drawn at (up to ~16 levels of coverage), so
/// the knock-out itself is not reproducible between two regions.
fn delta(a: &sk::Pixmap, b: &sk::Pixmap, c: &Case) -> u8 {
    let (w, h) = (a.width() as usize, a.height() as usize);
    let mut skip = vec![false; w * h];
    if !c.show_behind {
        let mut node = sk::Pixmap::new(w as u32, h as u32).unwrap();
        sil_for(c)(&mut node, 0.0, 0.0, 0.0, d::SilhouetteMode::Knockout);
        for y in 0..h {
            for x in 0..w {
                let al = node.data()[(y * w + x) * 4 + 3];
                if al != 0 && al != 255 {
                    for yy in y.saturating_sub(1)..(y + 2).min(h) {
                        for xx in x.saturating_sub(1)..(x + 2).min(w) {
                            skip[yy * w + xx] = true;
                        }
                    }
                }
            }
        }
    }
    a.data()
        .as_chunks::<4>()
        .0
        .iter()
        .zip(b.data().as_chunks::<4>().0.iter())
        .enumerate()
        .filter(|(i, _)| !skip[*i])
        .map(|(_, (p, q))| p.iter().zip(q).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0))
        .max()
        .unwrap_or(0)
}

/// The analytic nine-patch shadow stays within 2 levels of the generic path
/// (rendered with the original blur) over many radii, spreads, sub-pixel
/// positions, scales and sizes, including shapes too small for a plateau and
/// shadows clipped by the target.
#[test]
fn rrect_shadow_matches_generic() {
    let mut worst = (0u8, String::new());
    let mut count = 0;
    for &(w, h) in &[(150.0f32, 90.0f32), (500.0, 400.0)] {
        for &r in &[0.0f32, 20.0, 1000.0] {
            for &radius in &[2.0f32, 40.0, 90.0] {
                for &(spread, fill_alpha, show_behind) in &[(0.0f32, 1.0f32, false), (6.0, 1.0, true), (-3.0, 0.5, false)] {
                    for &(fx, scale) in &[(0.37f32, 1.0f32), (0.81, 2.0)] {
                        let c = Case {
                            x: 60.0 + fx,
                            y: 50.0 + fx * 0.7,
                            w: w / scale.max(1.0),
                            h: h / scale.max(1.0),
                            r,
                            scale,
                            radius,
                            spread,
                            off: (4.0, 8.0),
                            fill_alpha,
                            show_behind,
                        };
                        let d = delta(&run(&c, true), &run(&c, false), &c);
                        count += 1;
                        if d > 2 {
                            eprintln!(
                                "delta {d}: w {w} h {h} r {r} radius {radius} spread {spread} alpha {fill_alpha} behind {show_behind} fx {fx} scale {scale}"
                            );
                        }
                        if d > worst.0 {
                            worst =
                                (d, format!("w {w} h {h} r {r} radius {radius} spread {spread} alpha {fill_alpha} behind {show_behind} fx {fx} scale {scale}"));
                        }
                    }
                }
            }
        }
    }
    eprintln!("{count} cases, worst delta {} ({})", worst.0, worst.1);
    assert!(worst.0 <= 2, "worst delta {} ({})", worst.0, worst.1);
}

/// Identical shadows share one cached patch regardless of position.
#[test]
fn identical_shadows_hit_the_cache() {
    let mut target = sk::Pixmap::new(400, 300).unwrap();
    let color = d::Color { r: 0.0, g: 0.0, b: 0.0, a: 0.25 };
    // a geometry no other test uses, so concurrent tests cannot interfere
    let (w, h, radius) = (101.0f32, 61.0f32, 13.0f32);
    let before = thread_patch_builds();
    for i in 0..20 {
        let c = Case {
            x: 20.0 + i as f32 * 3.0,
            y: 30.0 + i as f32 * 2.0,
            w,
            h,
            r: 8.0,
            scale: 1.0,
            radius,
            spread: 0.0,
            off: (0.0, 4.0),
            fill_alpha: 1.0,
            show_behind: true,
        };
        let sil = sil_for(&c);
        let rr = RRect { x: c.x, y: c.y, w: c.w, h: c.h, r: c.r };
        assert!(drop_shadow_rrect(&mut target, None, rr, 1.0, &color, d::Vec2::new(0.0, 4.0), radius, 0.0, true, d::BlendMode::Normal, 1.0, 1.0, &sil));
    }
    assert_eq!(thread_patch_builds() - before, 1, "20 identical shadows must build one patch");
}

/// Wide shapes compress their plateau: the cached patch must not grow with the
/// shape.
#[test]
fn patch_size_independent_of_shape_size() {
    let small = patch_dims(10.0, 410.0, 25, 40).unwrap();
    let big = patch_dims(10.0, 4010.0, 25, 40).unwrap();
    assert_eq!(small, big);
}
