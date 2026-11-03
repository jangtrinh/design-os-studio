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

use designos_doc::{BlendMode, Color, Effect, Vec2};
use usvg::filter::{CompositeOperator, Input, Kind, MorphologyOperator};

fn blend_of(b: usvg::BlendMode) -> BlendMode {
    use usvg::BlendMode as U;
    match b {
        U::Normal => BlendMode::Normal,
        U::Multiply => BlendMode::Multiply,
        U::Screen => BlendMode::Screen,
        U::Overlay => BlendMode::Overlay,
        U::Darken => BlendMode::Darken,
        U::Lighten => BlendMode::Lighten,
        U::ColorDodge => BlendMode::ColorDodge,
        U::ColorBurn => BlendMode::ColorBurn,
        U::HardLight => BlendMode::HardLight,
        U::SoftLight => BlendMode::SoftLight,
        U::Difference => BlendMode::Difference,
        U::Exclusion => BlendMode::Exclusion,
        U::Hue => BlendMode::Hue,
        U::Saturation => BlendMode::Saturation,
        U::Color => BlendMode::Color,
        U::Luminosity => BlendMode::Luminosity,
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
    let mut shadow_results: Vec<(String, usize, bool)> = Vec::new();
    for p in prims {
        match p.kind() {
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
                        let idx = out.len();
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
                        shadow_results.push((p.result().to_string(), idx, false));
                    }
                }
            }
            Kind::Blend(b) => {
                let pos = match b.input1() {
                    Input::Reference(r) => shadow_results.iter().rposition(|(name, _, _)| name == r),
                    _ => None,
                }
                .or_else(|| match b.input2() {
                    Input::Reference(r) => shadow_results.iter().rposition(|(name, _, has_blend)| !*has_blend && name == r),
                    _ => None,
                });
                if let Some(pos) = pos {
                    let (_, idx, has_blend) = &mut shadow_results[pos];
                    if let Some(Effect::DropShadow { blend, .. } | Effect::InnerShadow { blend, .. }) = out.get_mut(*idx) {
                        *blend = blend_of(b.mode());
                        *has_blend = true;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::BlendMode;

    #[test]
    fn parses_blend_mode_from_drop_and_inner_shadow_filter_chains() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="30" viewBox="0 0 40 30">
  <defs>
    <filter id="shadow0" x="-16" y="-12" width="72" height="62" filterUnits="userSpaceOnUse" color-interpolation-filters="sRGB">
      <feFlood flood-opacity="0" result="BackgroundImageFix"/>
      <feColorMatrix in="SourceAlpha" type="matrix" values="0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 127 0" result="hardAlpha"/>
      <feOffset in="hardAlpha" dx="0" dy="4" result="offset0"/>
      <feGaussianBlur in="offset0" stdDeviation="4" result="blur0"/>
      <feComposite in="blur0" in2="hardAlpha" operator="out" result="knockout0"/>
      <feColorMatrix in="knockout0" type="matrix" values="0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0.5 0" result="shadow0"/>
      <feBlend mode="multiply" in="shadow0" in2="BackgroundImageFix" result="shadow0"/>
      <feMerge result="withShadows">
        <feMergeNode in="shadow0"/>
        <feMergeNode in="SourceGraphic"/>
      </feMerge>
      <feOffset in="hardAlpha" dx="1" dy="2" result="innerOffset1"/>
      <feGaussianBlur in="innerOffset1" stdDeviation="2" result="innerBlur1"/>
      <feComposite in="hardAlpha" in2="innerBlur1" operator="arithmetic" k2="1" k3="-1" result="innerCut1"/>
      <feColorMatrix in="innerCut1" type="matrix" values="0 0 0 0 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0.5 0" result="innerShadow1"/>
      <feBlend mode="screen" in="innerShadow1" in2="withShadows" result="withInner1"/>
    </filter>
  </defs>
  <g filter="url(#shadow0)">
    <rect width="40" height="30" fill="#D9D9D9"/>
  </g>
</svg>"##;
        let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).unwrap();
        let usvg::Node::Group(g) = &tree.root().children()[0] else {
            panic!("expected group");
        };
        let effs = effects_of(g.filters());
        assert_eq!(effs.len(), 2);
        assert!(matches!(effs[0], Effect::DropShadow { blend: BlendMode::Multiply, .. }), "got {:?}", effs[0]);
        assert!(matches!(effs[1], Effect::InnerShadow { blend: BlendMode::Screen, .. }), "got {:?}", effs[1]);
    }
}
