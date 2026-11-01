//! SVG filters -> designos effects.
//!
//! Understood forms:
//! * `feDropShadow`;
//! * a lone `feGaussianBlur` on the source graphic (layer blur);
//! * the feMorphology / feOffset / feGaussianBlur / feComposite / feColorMatrix
//!   chains that Figma (and `designos-export`) write for drop and inner shadows.
//!
//! Everything else in a filter is ignored. SVG `stdDeviation` is half of a
//! designos blur radius.

use designos_doc::{Color, Effect, Vec2};
use usvg::filter::{CompositeOperator, Input, Kind, MorphologyOperator};

fn blend_mode(mode: usvg::BlendMode) -> designos_doc::BlendMode {
    use designos_doc::BlendMode as D;
    use usvg::BlendMode as U;
    match mode {
        U::Multiply => D::Multiply,
        U::Screen => D::Screen,
        U::Overlay => D::Overlay,
        U::Darken => D::Darken,
        U::Lighten => D::Lighten,
        U::ColorDodge => D::ColorDodge,
        U::ColorBurn => D::ColorBurn,
        U::HardLight => D::HardLight,
        U::SoftLight => D::SoftLight,
        U::Difference => D::Difference,
        U::Exclusion => D::Exclusion,
        U::Hue => D::Hue,
        U::Saturation => D::Saturation,
        U::Color => D::Color,
        U::Luminosity => D::Luminosity,
        U::Normal => D::Normal,
    }
}

pub fn effects_of(filters: &[std::sync::Arc<usvg::filter::Filter>]) -> Vec<Effect> {
    let mut out = Vec::new();
    for f in filters {
        out.extend(one_filter(f.primitives()));
    }
    out
}

#[derive(Default)]
struct Chain {
    dx: f32,
    dy: f32,
    blur: f32,
    spread: f32,
    knockout: bool,
    inner: bool,
    started: bool,
}

fn one_filter(prims: &[usvg::filter::Primitive]) -> Vec<Effect> {
    // A filter that is only a blur of the source is a layer blur.
    if let [only] = prims {
        if let Kind::GaussianBlur(b) = only.kind() {
            if matches!(b.input(), Input::SourceGraphic) {
                let sd = (b.std_dev_x().get() + b.std_dev_y().get()) / 2.0;
                return if sd > 0.0 { vec![Effect::LayerBlur { radius: sd * 2.0, visible: true }] } else { vec![] };
            }
        }
    }
    let mut out = Vec::new();
    let mut chain = Chain::default();
    for p in prims {
        match p.kind() {
            Kind::Blend(b) => {
                let Input::Reference(name) = b.input1() else { continue };
                let Some(index) = name.strip_prefix("innerShadow").or_else(|| name.strip_prefix("shadow")).and_then(|n| n.parse::<usize>().ok()) else {
                    continue;
                };
                if let Some(Effect::DropShadow { blend, .. } | Effect::InnerShadow { blend, .. }) = out.get_mut(index) {
                    *blend = blend_mode(b.mode());
                }
            }
            Kind::DropShadow(s) => {
                let sd = (s.std_dev_x().get() + s.std_dev_y().get()) / 2.0;
                let c = s.color();
                out.push(Effect::DropShadow {
                    color: Color { r: c.red as f32 / 255.0, g: c.green as f32 / 255.0, b: c.blue as f32 / 255.0, a: s.opacity().get() },
                    offset: Vec2::new(s.dx(), s.dy()),
                    radius: sd * 2.0,
                    spread: 0.0,
                    visible: true,
                    show_behind: true,
                    blend: Default::default(),
                });
            }
            Kind::Morphology(m) => {
                let r = (m.radius_x().get() + m.radius_y().get()) / 2.0;
                chain.spread = if m.operator() == MorphologyOperator::Dilate { r } else { -r };
            }
            Kind::Offset(o) => {
                chain.dx = o.dx();
                chain.dy = o.dy();
                chain.started = true;
            }
            Kind::GaussianBlur(b) if chain.started && !matches!(b.input(), Input::SourceGraphic) => {
                chain.blur = (b.std_dev_x().get() + b.std_dev_y().get()) / 2.0 * 2.0;
            }
            Kind::Composite(c) if chain.started => match c.operator() {
                CompositeOperator::Out => chain.knockout = true,
                CompositeOperator::Arithmetic { .. } => chain.inner = true,
                _ => {}
            },
            Kind::ColorMatrix(m) if chain.started => {
                if let usvg::filter::ColorMatrixKind::Matrix(v) = m.kind() {
                    // Colorizing matrix: constant RGB in the last column, alpha scale in row 4.
                    // (The alpha-hardening matrix has a huge alpha gain and no color.)
                    let alpha = v[18];
                    if alpha > 0.0 && alpha < 50.0 {
                        let color = Color { r: v[4].clamp(0.0, 1.0), g: v[9].clamp(0.0, 1.0), b: v[14].clamp(0.0, 1.0), a: alpha.clamp(0.0, 1.0) };
                        let c = std::mem::take(&mut chain);
                        out.push(if c.inner {
                            // An inner shadow's morphology erodes the hole; spread is its size.
                            Effect::InnerShadow {
                                color,
                                offset: Vec2::new(c.dx, c.dy),
                                radius: c.blur,
                                spread: c.spread.abs(),
                                visible: true,
                                blend: Default::default(),
                            }
                        } else {
                            Effect::DropShadow {
                                color,
                                offset: Vec2::new(c.dx, c.dy),
                                radius: c.blur,
                                spread: c.spread,
                                visible: true,
                                show_behind: !c.knockout,
                                blend: Default::default(),
                            }
                        });
                    }
                }
            }
            _ => {}
        }
    }
    out
}
