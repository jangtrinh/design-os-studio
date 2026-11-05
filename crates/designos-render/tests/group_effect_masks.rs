//! Noise, glass and clipped texture on a group that paints nothing itself are masked by the
//! group's children at their own alpha. Only a background blur widens that mask to the
//! children's full footprint (see `backdrop.rs`). The digests pin the exact pixels each effect
//! renders for a group over a 40% child, so a change of the shared mask rule shows up here.

mod common;

use common::*;
use designos_doc::*;
use designos_render::tiny_skia::Pixmap;

const BLACK: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
const SIZE: (u32, u32) = (100, 80);

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Six black stripes on the white page, then a 60x60 group at (10, 10) carrying `fx` over a
/// 60x60 ellipse filled with 40% white.
fn group_over_translucent_child(fx: Effect) -> Pixmap {
    let mut t = T::new();
    for i in 0..6 {
        t.rect(None, Rect::new(5.0 + 15.0 * i as f32, 0.0, 6.0, 80.0), BLACK);
    }
    let g = t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Group);
        n.size = Vec2::new(60.0, 60.0);
        n.transform = Affine::translate(10.0, 10.0);
        n.effects = vec![fx];
        n
    });
    t.add(Some(g), |id| {
        let mut n = Node::ellipse(id, Rect::new(0.0, 0.0, 60.0, 60.0));
        n.fills = vec![Paint { opacity: 0.4, ..Paint::solid(Color::WHITE) }];
        n
    });
    t.render(SIZE.0, SIZE.1)
}

fn noise() -> Effect {
    Effect::Noise(NoiseEffect {
        mode: NoiseMode::Mono { color: BLACK },
        size: 1.0,
        size_y: None,
        density: 0.6,
        opacity: 1.0,
        blend: BlendMode::Normal,
        seed: 7,
        visible: true,
    })
}

fn glass() -> Effect {
    Effect::Glass(GlassEffect {
        light_angle: 45.0,
        light_intensity: 0.5,
        refraction: 0.5,
        depth: 8.0,
        dispersion: 0.2,
        frost: 4.0,
        splay: 0.0,
        bevel: 0.0,
        visible: true,
    })
}

fn texture() -> Effect {
    Effect::Texture(TextureEffect { size: 4.0, size_y: None, radius: 3.0, clip_to_shape: true, seed: 3, visible: true })
}

#[test]
fn noise_on_a_group_stays_masked_by_the_children_alpha() {
    let pm = group_over_translucent_child(noise());
    assert_eq!(fnv1a(pm.data()), 0x1c360c5c2e5fff46, "pixels of the noise scene changed");
}

#[test]
fn glass_on_a_group_stays_masked_by_the_children_alpha() {
    let pm = group_over_translucent_child(glass());
    assert_eq!(fnv1a(pm.data()), 0x8185fab0000fc8a6, "pixels of the glass scene changed");
}

#[test]
fn clipped_texture_on_a_group_stays_masked_by_the_children_alpha() {
    let pm = group_over_translucent_child(texture());
    assert_eq!(fnv1a(pm.data()), 0x43cc2e696be2ddf5, "pixels of the texture scene changed");
}
