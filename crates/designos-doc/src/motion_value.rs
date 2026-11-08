//! Keyframe values and how two of them blend (`KeyframeValueData` in the kiwi schema).

use crate::{oklab_to_srgb, oklch_to_srgb, srgb_to_oklab, srgb_to_oklch, Color, Vec2};
use serde::{Deserialize, Serialize};

/// The colour space a colour keyframe blends in (`InterpolationColorSpace`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ColorSpace {
    /// Blend the sRGB components as they are.
    #[default]
    Default,
    Oklab,
    Oklch,
    Hsl,
    SrgbLinear,
}

/// The value of one keyframe. A kind that Motion does not blend yet (text, paint, gradient, colour point) is
/// kept as `Raw` so it survives a round trip; blending two of them holds the first.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum KeyValue {
    Float(f32),
    Color(Color),
    Vector(Vec2),
    Bool(bool),
    Circle {
        center: Vec2,
        radius: f32,
    },
    Line {
        a: Vec2,
        b: Vec2,
    },
    CirclePoint {
        center: Vec2,
        radius: f32,
        angle: f32,
    },
    Rotation3D {
        x: f32,
        y: f32,
        z: f32,
        translate_z: f32,
    },
    /// A kiwi value this model does not interpret: its `KeyframeValueType` name and the value as JSON.
    Raw {
        kind: String,
        json: serde_json::Value,
    },
}

fn lerp(a: f32, b: f32, u: f32) -> f32 {
    a + (b - a) * u
}

fn lerp2(a: Vec2, b: Vec2, u: f32) -> Vec2 {
    Vec2::new(lerp(a.x, b.x, u), lerp(a.y, b.y, u))
}

/// Blends `a` into `b` at already-eased progress `u` (`0.0` gives `a`, `1.0` gives `b`; an overshooting `u` goes
/// past `b`). Two values of different kinds, and `Raw` values, do not blend: the result is `a` until `u` reaches
/// `1.0`, then `b`. A `Bool` steps the same way. `space` only matters for colours.
pub fn interpolate(a: &KeyValue, b: &KeyValue, u: f32, space: ColorSpace) -> KeyValue {
    use KeyValue::*;
    match (a, b) {
        (Float(x), Float(y)) => Float(lerp(*x, *y, u)),
        (Vector(x), Vector(y)) => Vector(lerp2(*x, *y, u)),
        (Color(x), Color(y)) => Color(blend_color(*x, *y, u, space)),
        (Circle { center: c0, radius: r0 }, Circle { center: c1, radius: r1 }) => Circle { center: lerp2(*c0, *c1, u), radius: lerp(*r0, *r1, u) },
        (Line { a: a0, b: b0 }, Line { a: a1, b: b1 }) => Line { a: lerp2(*a0, *a1, u), b: lerp2(*b0, *b1, u) },
        (CirclePoint { center: c0, radius: r0, angle: g0 }, CirclePoint { center: c1, radius: r1, angle: g1 }) => {
            CirclePoint { center: lerp2(*c0, *c1, u), radius: lerp(*r0, *r1, u), angle: lerp(*g0, *g1, u) }
        }
        (Rotation3D { x: x0, y: y0, z: z0, translate_z: t0 }, Rotation3D { x: x1, y: y1, z: z1, translate_z: t1 }) => {
            Rotation3D { x: lerp(*x0, *x1, u), y: lerp(*y0, *y1, u), z: lerp(*z0, *z1, u), translate_z: lerp(*t0, *t1, u) }
        }
        _ => {
            if u >= 1.0 {
                b.clone()
            } else {
                a.clone()
            }
        }
    }
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Hue in degrees blended along the shorter arc; a hueless colour (chroma or saturation ~ 0) takes the other hue.
fn lerp_hue(h0: f32, c0: f32, h1: f32, c1: f32, u: f32) -> f32 {
    const EPS: f32 = 1e-4;
    let (h0, h1) = match (c0 < EPS, c1 < EPS) {
        (true, false) => (h1, h1),
        (false, true) => (h0, h0),
        _ => (h0, h1),
    };
    let d = (h1 - h0 + 540.0).rem_euclid(360.0) - 180.0;
    (h0 + d * u).rem_euclid(360.0)
}

fn rgb_to_hsl(c: Color) -> [f32; 3] {
    let (max, min) = (c.r.max(c.g).max(c.b), c.r.min(c.g).min(c.b));
    let l = (max + min) / 2.0;
    let d = max - min;
    if d < 1e-6 {
        return [0.0, 0.0, l];
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == c.r {
        ((c.g - c.b) / d).rem_euclid(6.0)
    } else if max == c.g {
        (c.b - c.r) / d + 2.0
    } else {
        (c.r - c.g) / d + 4.0
    };
    [h * 60.0, s, l]
}

fn hsl_to_rgb(h: f32, s: f32, l: f32, a: f32) -> Color {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    Color { r: r + m, g: g + m, b: b + m, a }
}

fn blend_color(a: Color, b: Color, u: f32, space: ColorSpace) -> Color {
    let alpha = lerp(a.a, b.a, u);
    match space {
        ColorSpace::Default => Color { r: lerp(a.r, b.r, u), g: lerp(a.g, b.g, u), b: lerp(a.b, b.b, u), a: alpha },
        ColorSpace::SrgbLinear => {
            let ch = |x: f32, y: f32| linear_to_srgb(lerp(srgb_to_linear(x), srgb_to_linear(y), u));
            Color { r: ch(a.r, b.r), g: ch(a.g, b.g), b: ch(a.b, b.b), a: alpha }
        }
        ColorSpace::Oklab => {
            let (p, q) = (srgb_to_oklab(a.r, a.g, a.b), srgb_to_oklab(b.r, b.g, b.b));
            let [r, g, bl] = oklab_to_srgb(lerp(p[0], q[0], u), lerp(p[1], q[1], u), lerp(p[2], q[2], u));
            Color { r, g, b: bl, a: alpha }
        }
        ColorSpace::Oklch => {
            let (p, q) = (srgb_to_oklch(a.r, a.g, a.b), srgb_to_oklch(b.r, b.g, b.b));
            let h = lerp_hue(p[2], p[1], q[2], q[1], u);
            let [r, g, bl] = oklch_to_srgb(lerp(p[0], q[0], u), lerp(p[1], q[1], u), h);
            Color { r, g, b: bl, a: alpha }
        }
        ColorSpace::Hsl => {
            let (p, q) = (rgb_to_hsl(a), rgb_to_hsl(b));
            let h = lerp_hue(p[0], p[1], q[0], q[1], u);
            hsl_to_rgb(h, lerp(p[1], q[1], u), lerp(p[2], q[2], u), alpha)
        }
    }
}
