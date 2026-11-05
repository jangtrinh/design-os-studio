//! Noise, Texture and Glass effects.

mod common;
use common::*;
use designos_doc::*;

fn noise(mode: NoiseMode, size: f32, density: f32, opacity: f32, seed: u32) -> Effect {
    Effect::Noise(NoiseEffect { mode, size, size_y: None, density, opacity, blend: BlendMode::Normal, seed, visible: true })
}

fn card(t: &mut T, r: Rect, fill: Color, fx: Vec<Effect>) -> NodeId {
    let id = t.rect(None, r, fill);
    t.node(id).effects = fx;
    id
}

fn dark_count(pm: &designos_render::tiny_skia::Pixmap, x0: u32, y0: u32, w: u32, h: u32) -> usize {
    let mut n = 0;
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            if px(pm, x, y)[0] < 128 {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn mono_noise_covers_density_share_and_stays_inside_the_shape() {
    let mut t = T::new();
    card(&mut t, Rect::new(10.0, 10.0, 80.0, 80.0), Color::WHITE, vec![noise(NoiseMode::Mono { color: Color::BLACK }, 1.0, 0.5, 1.0, 7)]);
    let pm = t.render(100, 100);
    let share = dark_count(&pm, 10, 10, 80, 80) as f32 / 6400.0;
    assert!((share - 0.5).abs() < 0.06, "grain share {share}");
    // The page outside the card is untouched.
    assert_eq!(px(&pm, 5, 5), WHITE);
    assert_eq!(px(&pm, 95, 95), WHITE);
}

#[test]
fn noise_is_deterministic_and_seeded() {
    let mk = |seed| {
        let mut t = T::new();
        card(&mut t, Rect::new(0.0, 0.0, 60.0, 60.0), Color::WHITE, vec![noise(NoiseMode::Mono { color: Color::BLACK }, 2.0, 0.5, 1.0, seed)]);
        t.render(60, 60)
    };
    assert_eq!(mk(1).data(), mk(1).data());
    assert_ne!(mk(1).data(), mk(2).data());
}

#[test]
fn noise_grain_size_is_in_page_units() {
    let mut t = T::new();
    card(&mut t, Rect::new(0.0, 0.0, 64.0, 64.0), Color::WHITE, vec![noise(NoiseMode::Mono { color: Color::BLACK }, 4.0, 0.5, 1.0, 3)]);
    let pm = t.render(64, 64);
    // Every 4x4 grain is uniform.
    for gy in 0..16u32 {
        for gx in 0..16u32 {
            let first = px(&pm, gx * 4, gy * 4);
            for dy in 0..4 {
                for dx in 0..4 {
                    assert_eq!(px(&pm, gx * 4 + dx, gy * 4 + dy), first);
                }
            }
        }
    }
}

#[test]
fn duo_and_multi_noise_use_their_colors() {
    let mut t = T::new();
    card(
        &mut t,
        Rect::new(0.0, 0.0, 60.0, 60.0),
        Color::WHITE,
        vec![noise(NoiseMode::Duo { color: Color::rgb(255, 0, 0), secondary: Color::rgb(0, 0, 255) }, 1.0, 1.0, 1.0, 5)],
    );
    let pm = t.render(60, 60);
    let (mut red, mut blue) = (0, 0);
    for y in 0..60 {
        for x in 0..60 {
            let c = px(&pm, x, y);
            if c == [255, 0, 0, 255] {
                red += 1;
            } else if c == [0, 0, 255, 255] {
                blue += 1;
            } else {
                panic!("unexpected duo pixel {c:?}");
            }
        }
    }
    assert!(red > 1200 && blue > 1200, "red {red} blue {blue}");

    let mut t = T::new();
    card(&mut t, Rect::new(0.0, 0.0, 60.0, 60.0), Color::WHITE, vec![noise(NoiseMode::Multi, 1.0, 1.0, 1.0, 5)]);
    let pm = t.render(60, 60);
    let mut hues = std::collections::HashSet::new();
    for y in 0..60 {
        for x in 0..60 {
            let c = px(&pm, x, y);
            hues.insert((c[0] / 32, c[1] / 32, c[2] / 32));
        }
    }
    assert!(hues.len() > 12, "multi noise should vary in hue: {}", hues.len());
}

#[test]
fn noise_follows_the_rounded_shape_and_opacity() {
    let mut t = T::new();
    let id = card(&mut t, Rect::new(0.0, 0.0, 80.0, 80.0), Color::WHITE, vec![noise(NoiseMode::Mono { color: Color::BLACK }, 1.0, 1.0, 0.5, 2)]);
    t.node(id).corner_radii = [40.0; 4];
    let pm = t.render(80, 80);
    // The corner of the bounding box lies outside the circle: still white.
    assert_eq!(px(&pm, 1, 1), WHITE);
    // Inside, fully covered grains at 50 % black over white.
    let c = px(&pm, 40, 40);
    assert!(near(c, [128, 128, 128, 255], 3), "{c:?}");
}

#[test]
fn hidden_effects_draw_nothing() {
    let mut t = T::new();
    let e = Effect::Noise(NoiseEffect {
        visible: false,
        ..match noise(NoiseMode::Multi, 1.0, 1.0, 1.0, 1) {
            Effect::Noise(n) => n,
            _ => unreachable!(),
        }
    });
    card(&mut t, Rect::new(0.0, 0.0, 20.0, 20.0), Color::WHITE, vec![e]);
    let pm = t.render(20, 20);
    assert!((0..20).all(|y| (0..20).all(|x| px(&pm, x, y) == WHITE)));
}

fn texture(size: f32, radius: f32, clip: bool) -> Effect {
    Effect::Texture(TextureEffect { size, size_y: None, radius, clip_to_shape: clip, seed: 4, visible: true })
}

fn red_outside(pm: &designos_render::tiny_skia::Pixmap, inner: (u32, u32, u32, u32)) -> usize {
    let (x0, y0, x1, y1) = inner;
    let mut n = 0;
    for y in 0..pm.height() {
        for x in 0..pm.width() {
            let inside = x >= x0 && x < x1 && y >= y0 && y < y1;
            if !inside && px(pm, x, y)[1] < 200 {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn texture_roughens_edges_and_can_clip_to_the_shape() {
    let r = |clip| {
        let mut t = T::new();
        card(&mut t, Rect::new(20.0, 20.0, 60.0, 60.0), RED, vec![texture(6.0, 5.0, clip)]);
        t.render(100, 100)
    };
    let free = r(false);
    let clipped = r(true);
    // Unclipped: some red leaks outside the original rectangle.
    assert!(red_outside(&free, (20, 20, 80, 80)) > 20, "no displacement outside");
    // Clipped: nothing outside, but the edge pixels inside are no longer all red.
    assert_eq!(red_outside(&clipped, (20, 20, 80, 80)), 0);
    let mut holes = 0;
    for x in 20..80 {
        if px(&clipped, x, 21)[1] > 100 {
            holes += 1;
        }
    }
    assert!(holes > 0, "texture should carve the edge");
    // The interior is intact.
    assert_eq!(px(&clipped, 50, 50), [255, 0, 0, 255]);
}

#[test]
fn texture_with_zero_radius_changes_nothing() {
    let mut plain = T::new();
    card(&mut plain, Rect::new(20.0, 20.0, 60.0, 60.0), RED, vec![]);
    let mut tex = T::new();
    card(&mut tex, Rect::new(20.0, 20.0, 60.0, 60.0), RED, vec![texture(6.0, 0.0, true)]);
    assert_eq!(plain.render(100, 100).data(), tex.render(100, 100).data());
}

fn glass(frost: f32, intensity: f32, refraction: f32, dispersion: f32, angle: f32) -> Effect {
    Effect::Glass(GlassEffect {
        light_angle: angle,
        light_intensity: intensity,
        refraction,
        depth: 12.0,
        dispersion,
        frost,
        splay: 0.0,
        bevel: 0.0,
        visible: true,
    })
}

/// Black stripe at x 40..60 under a clear pane covering x 20..100.
fn glass_scene(fx: Effect) -> designos_render::tiny_skia::Pixmap {
    let mut t = T::new();
    t.rect(None, Rect::new(40.0, 0.0, 20.0, 120.0), Color::BLACK);
    card(&mut t, Rect::new(20.0, 20.0, 80.0, 80.0), Color::TRANSPARENT, vec![fx]);
    t.render(120, 120)
}

#[test]
fn glass_frosts_the_backdrop_only_inside_its_shape() {
    let pm = glass_scene(glass(10.0, 0.0, 0.0, 0.0, 0.0));
    // Inside the pane the sharp stripe edge is blurred...
    let inside = px(&pm, 41, 60);
    assert!(inside[0] > 30 && inside[0] < 230, "frosted: {inside:?}");
    // ...outside the pane it is still crisp.
    assert_eq!(px(&pm, 41, 5), [0, 0, 0, 255]);
    assert_eq!(px(&pm, 39, 5), WHITE);
}

#[test]
fn glass_highlight_sits_on_the_edge_facing_the_light() {
    // Over a white backdrop the highlight is invisible, so use a dark one.
    let mk = |angle: f32, intensity: f32| {
        let mut t = T::new();
        t.rect(None, Rect::new(0.0, 0.0, 120.0, 120.0), Color::BLACK);
        card(&mut t, Rect::new(20.0, 20.0, 80.0, 80.0), Color::TRANSPARENT, vec![glass(0.0, intensity, 0.0, 0.0, angle)]);
        t.render(120, 120)
    };
    // Angle 0: the light comes from the right.
    let lit = mk(0.0, 1.0);
    let dark = mk(0.0, 0.0);
    let right_lit = px(&lit, 97, 60)[0];
    let left_lit = px(&lit, 22, 60)[0];
    assert!(right_lit > left_lit + 40, "right {right_lit} left {left_lit}");
    assert!(right_lit > px(&dark, 97, 60)[0] + 40);
    // Deep inside nothing changes.
    assert_eq!(px(&lit, 60, 60), px(&dark, 60, 60));
}

#[test]
fn glass_dispersion_splits_colour_channels_at_the_edge() {
    let mk = |dispersion: f32| {
        let mut t = T::new();
        // A white block whose edge falls inside the bevel of the pane.
        t.rect(None, Rect::new(0.0, 0.0, 120.0, 120.0), Color::BLACK);
        t.rect(None, Rect::new(30.0, 0.0, 20.0, 120.0), Color::WHITE);
        card(&mut t, Rect::new(20.0, 20.0, 80.0, 80.0), Color::TRANSPARENT, vec![glass(0.0, 0.0, 1.0, dispersion, 0.0)]);
        t.render(120, 120)
    };
    let flat = mk(0.0);
    let split = mk(1.0);
    let spread = |pm: &designos_render::tiny_skia::Pixmap| (22..40).map(|x| (px(pm, x, 60)[0] as i32 - px(pm, x, 60)[2] as i32).abs()).max().unwrap();
    assert_eq!(spread(&flat), 0, "no dispersion: channels agree");
    assert!(spread(&split) > 30, "dispersion should separate red and blue");
}

#[test]
fn glass_is_deterministic() {
    let a = glass_scene(glass(6.0, 0.6, 0.5, 0.3, 315.0));
    let b = glass_scene(glass(6.0, 0.6, 0.5, 0.3, 315.0));
    assert_eq!(a.data(), b.data());
}
