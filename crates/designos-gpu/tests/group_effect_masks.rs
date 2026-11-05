//! Noise and clipped texture on a group that paints nothing itself are masked by the group's
//! children at their own alpha, on the GPU as on the CPU; only a background blur widens the mask
//! to the children's full footprint. Both renderers take the mask from
//! `designos_doc::silhouette_source`, so a GPU that agrees with the CPU render proves nothing by
//! itself: each scene also pins the CPU pixels (the same digests as `designos-render`'s
//! `group_effect_masks` test). Glass is left out: on this scene the GPU glass differs from the
//! CPU glass in 1,854 of 8,000 pixels at tolerance 4, far over the budget below, so its mask is
//! covered by the unit tests of `silhouette_source`. Pixel tests need a GPU adapter and skip
//! without one.

mod common;

use designos_doc::*;
use designos_render::Viewport;

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
fn group_over_translucent_child(fx: Effect) -> common::T {
    let mut t = common::T::new();
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
    t
}

/// The CPU pixels equal `cpu_digest`, and the GPU draws the same scene within the budget of the
/// other GPU parity tests. The group's ellipse covers 2,827 of the 8,000 pixels, far over both
/// budgets, so a mask that differs between the renderers cannot hide as an edge effect.
fn parity(name: &str, fx: Effect, cpu_digest: u64) {
    let Some(mut g) = common::gpu() else { return };
    let t = group_over_translucent_child(fx);
    let c = common::compare(&mut g, name, &t.doc, t.page, Viewport::default(), SIZE, 4);
    assert_eq!(fnv1a(c.cpu.data()), cpu_digest, "{name}: pixels of the CPU scene changed");
    assert!(c.report_aa.ratio < 0.005, "{name}: mismatch with AA forgiven {:.3}%", c.report_aa.ratio * 100.0);
    assert!(c.report.ratio < 0.02, "{name}: strict mismatch {:.3}%", c.report.ratio * 100.0);
}

#[test]
fn noise_on_a_group_stays_masked_by_the_children_alpha() {
    let fx = Effect::Noise(NoiseEffect {
        mode: NoiseMode::Mono { color: BLACK },
        size: 1.0,
        size_y: None,
        density: 0.6,
        opacity: 1.0,
        blend: BlendMode::Normal,
        seed: 7,
        visible: true,
    });
    parity("group-noise-translucent-child", fx, 0x1c360c5c2e5fff46);
}

#[test]
fn clipped_texture_on_a_group_stays_masked_by_the_children_alpha() {
    let fx = Effect::Texture(TextureEffect { size: 4.0, size_y: None, radius: 3.0, clip_to_shape: true, seed: 3, visible: true });
    parity("group-texture-translucent-child", fx, 0x43cc2e696be2ddf5);
}
