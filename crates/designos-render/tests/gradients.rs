mod common;
use common::*;
use designos_doc::*;

fn hue_deg(p: [u8; 4]) -> f32 {
    let (r, g, b) = (p[0] as f32, p[1] as f32, p[2] as f32);
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let d = mx - mn;
    if d < 1.0 {
        return 0.0;
    }
    let h = if mx == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if mx == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    h * 60.0
}

fn hsv(h: f32) -> Color {
    let x = 1.0 - ((h / 60.0) % 2.0 - 1.0).abs();
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        _ => (x, 0.0, 1.0),
    };
    Color { r, g, b, a: 1.0 }
}

const CENTRE: [Vec2; 3] = [Vec2::new(0.5, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.5, 1.0)];

/// A conic sweep: hue rises monotonically clockwise from the +x axis (the
/// handle direction), with the only discontinuity at the start angle.
#[test]
fn angular_gradient_sweeps_around_centre() {
    let mut t = T::new();
    let r = t.rect(None, Rect::new(0.0, 0.0, 100.0, 100.0), RED);
    let stops: Vec<GradientStop> = (0..6).map(|i| stop(i as f32 / 5.0, hsv(i as f32 * 60.0))).collect();
    t.node(r).fills = vec![paint(PaintKind::AngularGradient { handles: CENTRE, stops })];
    let pm = t.render(100, 100);
    let n = 72;
    let hues: Vec<f32> = (0..n)
        .map(|i| {
            let th = (i as f32 + 0.5) * std::f32::consts::TAU / n as f32;
            hue_deg(px(&pm, (50.0 + 40.0 * th.cos()) as u32, (50.0 + 40.0 * th.sin()) as u32))
        })
        .collect();
    for w in hues.windows(2) {
        let d = w[1] - w[0];
        assert!((-1.5..=9.0).contains(&d), "hue must rise smoothly, got {hues:?}");
    }
    // the seam sits between the last and first sample (the start angle)
    assert!(hues[0] < 10.0, "start of sweep is red: {}", hues[0]);
    assert!(hues[n - 1] > 285.0, "end of sweep is magenta: {}", hues[n - 1]);
    // quarter turns: below centre = t 0.25 (hue 75), left = 0.5 (150), above = 0.75 (225)
    assert!((hue_deg(px(&pm, 50, 90)) - 75.0).abs() < 6.0);
    assert!((hue_deg(px(&pm, 10, 50)) - 150.0).abs() < 6.0);
    assert!((hue_deg(px(&pm, 50, 10)) - 225.0).abs() < 6.0);
}

/// A looping angular gradient (first stop colour == last) has no seam at all.
#[test]
fn angular_gradient_loop_has_no_seam() {
    let mut t = T::new();
    let r = t.rect(None, Rect::new(0.0, 0.0, 100.0, 100.0), RED);
    t.node(r).fills = vec![paint(PaintKind::AngularGradient { handles: CENTRE, stops: vec![stop(0.0, RED), stop(0.5, BLUE), stop(1.0, RED)] })];
    let pm = t.render(100, 100);
    let above = px(&pm, 90, 49);
    let below = px(&pm, 90, 51);
    assert!(near(above, below, 12), "{above:?} vs {below:?}");
}

/// Rotating the handles rotates the start angle of the sweep.
#[test]
fn angular_gradient_start_follows_handles() {
    let mut t = T::new();
    let r = t.rect(None, Rect::new(0.0, 0.0, 100.0, 100.0), RED);
    // h1 points down: start angle is 6 o'clock
    let handles = [Vec2::new(0.5, 0.5), Vec2::new(0.5, 1.0), Vec2::new(0.0, 0.5)];
    t.node(r).fills = vec![paint(PaintKind::AngularGradient { handles, stops: vec![stop(0.0, RED), stop(1.0, BLUE)] })];
    let pm = t.render(100, 100);
    let left = px(&pm, 10, 50); // quarter turn after start
    let below_l = px(&pm, 49, 90);
    let below_r = px(&pm, 51, 90);
    assert!(near(left, [191, 0, 64, 255], 16), "{left:?}");
    assert!(below_l[0] > 240 && below_r[2] > 240, "seam below: {below_l:?} {below_r:?}");
}

#[test]
fn diamond_gradient_is_four_fold_symmetric() {
    let mut t = T::new();
    let r = t.rect(None, Rect::new(0.0, 0.0, 100.0, 100.0), RED);
    t.node(r).fills = vec![paint(PaintKind::DiamondGradient { handles: CENTRE, stops: vec![stop(0.0, RED), stop(1.0, BLUE)] })];
    let pm = t.render(100, 100);
    // |x|+|y| = const: axis points and diagonal points share a colour
    let a = px(&pm, 75, 50);
    for p in [px(&pm, 25, 50), px(&pm, 50, 25), px(&pm, 50, 75), px(&pm, 63, 63), px(&pm, 37, 37), px(&pm, 63, 37)] {
        assert!(near(a, p, 8), "{a:?} vs {p:?}");
    }
    let mut last = 300;
    for x in (50..99).step_by(7) {
        let r = px(&pm, x, 50)[0] as i32;
        assert!(r <= last, "red must fall outward");
        last = r;
    }
}

#[test]
fn radial_gradient_on_non_square_frame_centres_and_reaches_all_four_edges() {
    let mut t = T::new();
    let r = t.rect(None, Rect::new(0.0, 0.0, 200.0, 100.0), RED);
    // 90°-rotated handles from .fig radial gradient centered at (0.5, 0.5) with radius (0.5, 0.5)
    let handles = [Vec2::new(0.5, 0.5), Vec2::new(0.5, 1.0), Vec2::new(0.0, 0.5)];
    t.node(r).fills = vec![paint(PaintKind::RadialGradient { handles, stops: vec![stop(0.0, RED), stop(1.0, BLUE)] })];
    let pm = t.render(200, 100);
    // Centre (100, 50) is pure inner stop (RED)
    assert!(near(px(&pm, 100, 50), [255, 0, 0, 255], 6), "centre (100, 50): {:?}", px(&pm, 100, 50));
    // All four edge midpoints (0, 50), (199, 50), (100, 0), (100, 99) reach the outer stop (BLUE)
    for (x, y) in [(0, 50), (199, 50), (100, 0), (100, 99)] {
        assert!(near(px(&pm, x, y), [0, 0, 255, 255], 6), "edge ({x}, {y}): {:?}", px(&pm, x, y));
    }
    // Halfway along horizontal (50, 50), (150, 50) and vertical (100, 25), (100, 75) semi-axes are 50% mix
    for (x, y) in [(50, 50), (150, 50), (100, 25), (100, 75)] {
        assert!(near(px(&pm, x, y), [128, 0, 128, 255], 10), "mid ({x}, {y}): {:?}", px(&pm, x, y));
    }
}
