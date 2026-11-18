//! Part 1, shape and stroke geometry: Studio's live path for a layer (its stored outline dropped, drawn by the CPU
//! renderer with one opaque paint) against the `fillGeometry` / `strokeGeometry` Figma stored for it. Both are
//! rasterised at 4x in the layer's own space and must lie within 0.5 px of each other (Hausdorff distance, tested by
//! dilation: every covered pixel of one has a covered pixel of the other within 2 raster px).
//!
//! Figma stores an INSIDE or OUTSIDE stroke as the centred band of twice the weight and clips it with the fill outline
//! when it draws (the renderer does the same with a stored band). The stored band is clipped the same way here, with
//! the fill outline Figma stored for the layer.

use super::board::{broken_comparator, Part};
use super::keys::NodeKeys;
use super::raster::{alpha, coverage, covered, figma_paths, within};
use super::raw::{field_str, geometry_of, record_guid, Raw};
use super::scratch::{studio_coverage, Pass};
use designos_doc::*;
use designos_render::tiny_skia as sk;

pub const SCALE: f32 = 4.0;
pub const MAX_SIDE: u32 = 4096;

pub struct GeoTruth {
    pub guid: String,
    pub ty: String,
    pub align: String,
    pub fill: Vec<(usize, bool)>,
    pub stroke: Vec<(usize, bool)>,
}

pub fn truth(raw: &Raw) -> Vec<GeoTruth> {
    raw.live()
        .filter(|r| field_str(r, "type") != Some("TEXT"))
        .filter_map(|r| {
            let (fill, stroke) = (geometry_of(r, "fillGeometry"), geometry_of(r, "strokeGeometry"));
            if fill.is_empty() && stroke.is_empty() {
                return None;
            }
            let (ty, align) = (field_str(r, "type").unwrap_or("?").to_string(), field_str(r, "strokeAlign").unwrap_or("CENTER").to_string());
            Some(GeoTruth { guid: record_guid(r)?, ty, align, fill, stroke })
        })
        .collect()
}

pub enum Verdict {
    Match,
    /// How it misses: which outline pokes out of the other, and Studio's covered area over Figma's.
    Mismatch(String),
    Unscored(&'static str),
}

/// What the stored outline is clipped by before it is compared: nothing, or the fill outline kept or cut away.
pub enum Clip<'a> {
    None,
    Inside(&'a [(sk::Path, bool)]),
    Outside(&'a [(sk::Path, bool)]),
}

/// Compares Studio's live outline of `nodes[0]` (with `nodes[1..]`, its descendants, for a boolean) with Figma's.
pub fn compare(nodes: &[Node], pass: Pass, figma: &[(sk::Path, bool)], clip: Clip) -> Verdict {
    let root = &nodes[0];
    let w = root.stroke.side_weights.map_or(root.stroke.weight, |s| s.iter().copied().fold(root.stroke.weight, f32::max));
    let m = 2.0 + 2.0 * w.max(0.0) * root.stroke.miter_limit.clamp(1.0, 4.0);
    let (mut x0, mut y0, mut x1, mut y1) = (0.0f32.min(root.size.x), 0.0f32.min(root.size.y), root.size.x.max(0.0), root.size.y.max(0.0));
    for (p, _) in figma {
        let b = p.bounds();
        (x0, y0, x1, y1) = (x0.min(b.left()), y0.min(b.top()), x1.max(b.right()), y1.max(b.bottom()));
    }
    let (x0, y0) = (x0 - m, y0 - m);
    let (pw, ph) = (((x1 + m - x0) * SCALE).ceil(), ((y1 + m - y0) * SCALE).ceil());
    if !(pw.is_finite() && ph.is_finite()) || pw > MAX_SIDE as f32 || ph > MAX_SIDE as f32 {
        return Verdict::Unscored("window over 4096 px at 4x");
    }
    let (pw, ph) = (pw as u32, ph as u32);
    let ts = sk::Transform::from_row(SCALE, 0.0, 0.0, SCALE, -x0 * SCALE, -y0 * SCALE);
    let Some(mut fig) = coverage(figma, pw, ph, ts) else { return Verdict::Unscored("empty window") };
    if let Clip::Inside(c) | Clip::Outside(c) = &clip {
        let Some(c) = coverage(c, pw, ph, ts) else { return Verdict::Unscored("empty window") };
        let inside = matches!(clip, Clip::Inside(_));
        for (s, c) in fig.iter_mut().zip(c) {
            let keep = if inside { u32::from(c) } else { 255 - u32::from(c) };
            *s = (u32::from(*s) * keep / 255) as u8;
        }
    }
    let Some(studio) = studio_coverage(nodes, pass, pw, ph, ts) else { return Verdict::Unscored("empty window") };
    let (a, b) = (covered(&fig), covered(&alpha(&studio)));
    let (ea, eb) = (!a.iter().any(|x| *x), !b.iter().any(|x| *x));
    if ea && eb {
        return Verdict::Unscored("both outlines cover nothing at 4x");
    }
    let r = (0.5 * SCALE) as i32;
    let fig_in = !ea && !eb && within(&a, &b, pw as usize, ph as usize, r);
    let studio_in = !ea && !eb && within(&b, &a, pw as usize, ph as usize, r);
    if broken_comparator() || (fig_in && studio_in) {
        return Verdict::Match;
    }
    let area = |m: &[bool]| m.iter().filter(|x| **x).count() as f32;
    let ratio = area(&b) / area(&a).max(1.0);
    let side = match (fig_in, studio_in) {
        (false, true) => "Figma's outline pokes out of Studio's",
        (true, false) => "Studio's outline pokes out of Figma's",
        _ => "both poke out",
    };
    Verdict::Mismatch(format!("{side}, area ratio {:.1}", (ratio * 10.0).round() / 10.0))
}

pub fn buckets(root: &Node, t: &GeoTruth, pass: Pass) -> Vec<String> {
    let pre = if pass == Pass::Fill { "fill" } else { "stroke" };
    let mut b = vec![format!("{pre}: total"), format!("{pre}: type {}", t.ty)];
    if pass == Pass::Stroke {
        let s = &root.stroke;
        b.push(format!("stroke: align {}", t.align));
        b.push(format!("stroke: cap {:?}", s.cap));
        b.push(format!("stroke: join {:?}", s.join));
        b.push(if s.dashes.iter().any(|d| *d > 0.0) { "stroke: dashed".into() } else { "stroke: solid".into() });
        if s.side_weights.is_some() {
            b.push("stroke: per-side weights".into());
        }
    }
    if root.corner_radii.iter().any(|r| *r > 0.0) {
        b.push(format!("{pre}: corner radius"));
    }
    if root.corner_smoothing > 0.0 {
        b.push(format!("{pre}: corner smoothing"));
    }
    if let NodeKind::Vector { network: Some(net), .. } = &root.kind {
        b.push(format!("{pre}: vector network regions {}", if net.regions.is_empty() { "none" } else { "some" }));
    }
    b
}

pub fn part(truth: &[GeoTruth], blobs: &[Vec<u8>], doc: &Document, keys: &NodeKeys) -> Part {
    let mut p = Part::new("geometry", "Shape and stroke geometry", "live outline within 0.5 px of the stored one (Hausdorff, coverage at 4x, by dilation)");
    p.note("a vector's live path is the one a vertex edit gives it: its network's paths; a vector without a network is not measured");
    p.note("an INSIDE or OUTSIDE stored stroke band is clipped by the layer's stored fill outline before the comparison");
    let mut jobs: Vec<(&GeoTruth, NodeId, Pass, Vec<Node>)> = Vec::new();
    for t in truth {
        let Some(&id) = keys.by_source.get(&t.guid) else {
            p.unscored("record with no layer", "the record did not become a layer on a page");
            continue;
        };
        match super::scratch::live_nodes(doc, id) {
            Ok(nodes) => {
                if !t.fill.is_empty() {
                    jobs.push((t, id, Pass::Fill, nodes.clone()));
                }
                if !t.stroke.is_empty() {
                    jobs.push((t, id, Pass::Stroke, nodes));
                }
            }
            Err(why) => p.unscored(why, why),
        }
    }
    eprintln!("geometry comparisons to run: {}", jobs.len());
    let results = super::scratch::run_parallel(jobs.len(), |i| {
        let (t, _, pass, nodes) = &jobs[i];
        if *pass == Pass::Fill {
            return compare(nodes, *pass, &figma_paths(&t.fill, blobs), Clip::None);
        }
        let fill = figma_paths(&t.fill, blobs);
        let clip = match t.align.as_str() {
            "INSIDE" | "OUTSIDE" if fill.is_empty() => return Verdict::Unscored("inside or outside stroke with no stored fill outline to clip by"),
            "INSIDE" => Clip::Inside(&fill),
            "OUTSIDE" => Clip::Outside(&fill),
            _ => Clip::None,
        };
        compare(nodes, *pass, &figma_paths(&t.stroke, blobs), clip)
    });
    for ((t, id, pass, nodes), v) in jobs.iter().zip(results) {
        let hidden = keys.is_hidden(*id);
        match v {
            Verdict::Unscored(why) => p.unscored(why, why),
            Verdict::Mismatch(how) if !hidden => {
                let what =
                    if *pass == Pass::Fill { format!("failing fill, type {}", t.ty) } else { format!("failing stroke, type {}, align {}", t.ty, t.align) };
                p.detail(&what, &how);
                p.record(item(t, *pass), hidden, false, &buckets(&nodes[0], t, *pass));
            }
            v => p.record(item(t, *pass), hidden, matches!(v, Verdict::Match), &buckets(&nodes[0], t, *pass)),
        }
    }
    p
}

fn item(t: &GeoTruth, pass: Pass) -> String {
    format!("{}#{}", t.guid, if pass == Pass::Fill { "fill" } else { "stroke" })
}
