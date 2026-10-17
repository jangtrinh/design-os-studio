//! Linear and radial gradients as PDF shadings, built the way the renderer builds them: colour stops
//! live in a unit space (axis `(0,0)..(1,0)` for linear, circle of radius 1 at the origin for radial),
//! and the three gradient handles map that space onto the node. The writer draws the shading under that
//! matrix, so skewed and elliptical gradients come out as on the canvas.

use super::n5;
use crate::util::num;
use designos_doc::{Color, GradientStop, Vec2};

/// Positions closer than this are one position.
const EPS: f32 = 1e-5;

pub(super) enum Plan {
    /// One colour stop: the renderer paints it as a solid colour.
    Flat(Color),
    Shaded(Shaded),
}

pub(super) struct Shaded {
    /// Shading dictionary in the unit space.
    pub dict: String,
    /// `cm` operands that map the unit space onto the node.
    pub matrix: String,
    /// Opacity shared by all stops (stops of different opacity are flattened by the page checks).
    pub alpha: f32,
}

/// Shading for a gradient paint on a node of `size`, or `None` when the renderer paints nothing
/// (no stops, or handles that make the gradient space singular).
pub(super) fn plan(radial: bool, stops: &[GradientStop], handles: &[Vec2; 3], size: Vec2) -> Option<Plan> {
    let mut sorted: Vec<(f32, Color)> = stops.iter().map(|s| (if s.pos.is_nan() { 0.0 } else { s.pos.clamp(0.0, 1.0) }, s.color)).collect();
    sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
    match sorted.as_slice() {
        [] => return None,
        [(_, color)] => return Some(Plan::Flat(*color)),
        _ => {}
    }
    let [h0, h1, h2] = *handles;
    let m = [size.x * (h1.x - h0.x), size.y * (h1.y - h0.y), size.x * (h2.x - h0.x), size.y * (h2.y - h0.y), size.x * h0.x, size.y * h0.y];
    let det = m[0] * m[3] - m[1] * m[2];
    if !m.iter().all(|v| v.is_finite()) || det.abs() < 1e-9 {
        return None;
    }
    let function = function_dict(&sorted);
    let coords = if radial { "/ShadingType 3 /Coords [0 0 0 0 0 1]" } else { "/ShadingType 2 /Coords [0 0 1 0]" };
    let dict = format!("<< {coords} /ColorSpace /DeviceRGB /Extend [true true] /Function {function} >>");
    let matrix = m.map(n5).join(" ");
    Some(Plan::Shaded(Shaded { dict, matrix, alpha: sorted[0].1.a }))
}

fn linear_segment(a: Color, b: Color) -> String {
    format!("<< /FunctionType 2 /Domain [0 1] /C0 [{} {} {}] /C1 [{} {} {}] /N 1 >>", num(a.r), num(a.g), num(a.b), num(b.r), num(b.g), num(b.b))
}

/// Colour as a function of the position along the gradient, over `[0 1]`: flat before the first stop and
/// after the last, linear between stops, a jump where two stops share a position.
fn function_dict(stops: &[(f32, Color)]) -> String {
    let (first, last) = (stops[0], stops[stops.len() - 1]);
    // (start, end, colour at start, colour at end)
    let mut segments: Vec<(f32, f32, Color, Color)> = Vec::new();
    if first.0 > EPS {
        segments.push((0.0, first.0, first.1, first.1));
    }
    for pair in stops.windows(2) {
        if pair[1].0 - pair[0].0 > EPS {
            segments.push((pair[0].0, pair[1].0, pair[0].1, pair[1].1));
        }
    }
    if last.0 < 1.0 - EPS {
        segments.push((last.0, 1.0, last.1, last.1));
    }
    if let [(_, _, a, b)] = segments.as_slice() {
        return linear_segment(*a, *b);
    }
    let functions = segments.iter().map(|(_, _, a, b)| linear_segment(*a, *b)).collect::<Vec<_>>().join(" ");
    let bounds = segments.iter().skip(1).map(|(start, ..)| num(*start)).collect::<Vec<_>>().join(" ");
    let encode = vec!["0 1"; segments.len()].join(" ");
    format!("<< /FunctionType 3 /Domain [0 1] /Functions [{functions}] /Bounds [{bounds}] /Encode [{encode}] >>")
}
