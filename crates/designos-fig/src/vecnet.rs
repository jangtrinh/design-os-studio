//! Vector networks in Figma's shapes: the plugin-API / REST JSON form
//! (`vertices`, `segments` with `tangentStart`/`tangentEnd`, `regions` with
//! `loops`) and the binary `vectorNetworkBlob` of `.fig` files.

use crate::kiwi::clean_f32;
use designos_doc::vector::{Region, Segment, Vertex};
use designos_doc::{FillRule, HandleMirror, StrokeCap, StrokeJoin, Vec2, VectorNetwork};
use serde_json::{json, Map, Value};

fn num(v: &Value, k: &str) -> Option<f32> {
    v.get(k).and_then(Value::as_f64).map(|f| f as f32)
}

pub(crate) fn cap_from(s: &str) -> Option<StrokeCap> {
    match s {
        "NONE" => Some(StrokeCap::None),
        "ROUND" => Some(StrokeCap::Round),
        "SQUARE" => Some(StrokeCap::Square),
        "ARROW_LINES" => Some(StrokeCap::ArrowLines),
        "ARROW_EQUILATERAL" => Some(StrokeCap::ArrowEquilateral),
        "TRIANGLE_FILLED" => Some(StrokeCap::TriangleFilled),
        "DIAMOND_FILLED" => Some(StrokeCap::DiamondFilled),
        "CIRCLE_FILLED" => Some(StrokeCap::CircleFilled),
        _ => None,
    }
}

pub(crate) fn cap_name(c: StrokeCap) -> &'static str {
    match c {
        StrokeCap::None => "NONE",
        StrokeCap::Round => "ROUND",
        StrokeCap::Square => "SQUARE",
        StrokeCap::ArrowLines => "ARROW_LINES",
        StrokeCap::ArrowEquilateral => "ARROW_EQUILATERAL",
        StrokeCap::TriangleFilled => "TRIANGLE_FILLED",
        StrokeCap::DiamondFilled => "DIAMOND_FILLED",
        StrokeCap::CircleFilled => "CIRCLE_FILLED",
    }
}

fn join_from(s: &str) -> Option<StrokeJoin> {
    match s {
        "MITER" => Some(StrokeJoin::Miter),
        "BEVEL" => Some(StrokeJoin::Bevel),
        "ROUND" => Some(StrokeJoin::Round),
        _ => None,
    }
}

fn join_name(j: StrokeJoin) -> &'static str {
    match j {
        StrokeJoin::Miter => "MITER",
        StrokeJoin::Bevel => "BEVEL",
        StrokeJoin::Round => "ROUND",
    }
}

fn mirror_from(s: &str) -> HandleMirror {
    match s {
        "ANGLE" => HandleMirror::Angle,
        "ANGLE_AND_LENGTH" => HandleMirror::AngleAndLength,
        _ => HandleMirror::None,
    }
}

fn mirror_name(m: HandleMirror) -> &'static str {
    match m {
        HandleMirror::None => "NONE",
        HandleMirror::Angle => "ANGLE",
        HandleMirror::AngleAndLength => "ANGLE_AND_LENGTH",
    }
}

fn point(v: &Value, k: &str) -> Vec2 {
    v.get(k).map(|p| Vec2::new(num(p, "x").unwrap_or(0.0), num(p, "y").unwrap_or(0.0))).unwrap_or(Vec2::ZERO)
}

/// Parses a `vectorNetwork` object; `None` for malformed data (indices out of range).
pub(crate) fn network_from_json(v: &Value) -> Option<VectorNetwork> {
    let verts = v.get("vertices")?.as_array()?;
    let segs = v.get("segments").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    let regs = v.get("regions").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    let mut net = VectorNetwork::default();
    for x in verts {
        let mut vx = Vertex::new(Vec2::new(num(x, "x")?, num(x, "y")?));
        vx.corner_radius = num(x, "cornerRadius").unwrap_or(0.0).max(0.0);
        vx.cap = x.get("strokeCap").and_then(Value::as_str).and_then(cap_from);
        vx.join = x.get("strokeJoin").and_then(Value::as_str).and_then(join_from);
        vx.mirror = x.get("handleMirroring").and_then(Value::as_str).map(mirror_from).unwrap_or_default();
        net.vertices.push(vx);
    }
    for s in segs {
        let a = s.get("start")?.as_u64()? as usize;
        let b = s.get("end")?.as_u64()? as usize;
        if a >= net.vertices.len() || b >= net.vertices.len() {
            return None;
        }
        net.segments.push(Segment { a, b, ta: point(s, "tangentStart"), tb: point(s, "tangentEnd") });
    }
    for r in regs {
        let rule = match r.get("windingRule").and_then(Value::as_str) {
            Some("EVENODD" | "ODD") => FillRule::EvenOdd,
            _ => FillRule::NonZero,
        };
        let mut loops = vec![];
        for l in r.get("loops")?.as_array()? {
            let idx: Vec<usize> = l.as_array()?.iter().filter_map(|x| x.as_u64().map(|u| u as usize)).collect();
            if idx.iter().any(|i| *i >= net.segments.len()) {
                return None;
            }
            loops.push(idx);
        }
        net.regions.push(Region { rule, loops });
    }
    Some(net)
}

/// The REST / plugin-API form of a network.
pub(crate) fn network_to_json(net: &VectorNetwork) -> Value {
    let p = |t: Vec2| json!({"x": clean_f32(t.x), "y": clean_f32(t.y)});
    let vertices: Vec<Value> = net
        .vertices
        .iter()
        .map(|v| {
            let mut m = Map::new();
            m.insert("x".into(), clean_f32(v.pos.x));
            m.insert("y".into(), clean_f32(v.pos.y));
            if let Some(c) = v.cap {
                m.insert("strokeCap".into(), cap_name(c).into());
            }
            if let Some(j) = v.join {
                m.insert("strokeJoin".into(), join_name(j).into());
            }
            if v.corner_radius > 0.0 {
                m.insert("cornerRadius".into(), clean_f32(v.corner_radius));
            }
            if v.mirror != HandleMirror::None {
                m.insert("handleMirroring".into(), mirror_name(v.mirror).into());
            }
            Value::Object(m)
        })
        .collect();
    let segments: Vec<Value> = net.segments.iter().map(|s| json!({"start": s.a, "end": s.b, "tangentStart": p(s.ta), "tangentEnd": p(s.tb)})).collect();
    let regions: Vec<Value> =
        net.regions.iter().map(|r| json!({"windingRule": if r.rule == FillRule::EvenOdd { "EVENODD" } else { "NONZERO" }, "loops": r.loops})).collect();
    json!({"vertices": vertices, "segments": segments, "regions": regions})
}

/// Decodes the binary network of a `.fig` vector: a header of three counts,
/// vertices (`style`, x, y), segments (`style`, start, start tangent, end, end
/// tangent) and regions (`style << 1 | nonzero`, loops of segment indices).
/// Coordinates are scaled by `scale` (node size over the normalised size).
/// Returns the network and the style id of every vertex.
pub(crate) fn network_from_blob(b: &[u8], scale: (f32, f32)) -> Option<(VectorNetwork, Vec<u32>)> {
    let mut pos = 0usize;
    let u32_at = |p: &mut usize| -> Option<u32> {
        let s = b.get(*p..*p + 4)?;
        *p += 4;
        Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    };
    let f32_at = |p: &mut usize| -> Option<f32> { u32_at(p).map(f32::from_bits).filter(|f| f.is_finite()) };
    let nv = u32_at(&mut pos)? as usize;
    let ns = u32_at(&mut pos)? as usize;
    let nr = u32_at(&mut pos)? as usize;
    // every record is at least 12 bytes: refuse absurd counts before allocating
    if nv.checked_mul(12)?.checked_add(ns.checked_mul(28)?)? > b.len() || nr > b.len() {
        return None;
    }
    let mut net = VectorNetwork::default();
    let mut styles = vec![];
    for _ in 0..nv {
        let style = u32_at(&mut pos)?;
        let (x, y) = (f32_at(&mut pos)?, f32_at(&mut pos)?);
        net.vertices.push(Vertex::new(Vec2::new(x * scale.0, y * scale.1)));
        styles.push(style);
    }
    for _ in 0..ns {
        let _style = u32_at(&mut pos)?;
        let a = u32_at(&mut pos)? as usize;
        let ta = Vec2::new(f32_at(&mut pos)? * scale.0, f32_at(&mut pos)? * scale.1);
        let bb = u32_at(&mut pos)? as usize;
        let tb = Vec2::new(f32_at(&mut pos)? * scale.0, f32_at(&mut pos)? * scale.1);
        if a >= nv || bb >= nv {
            return None;
        }
        net.segments.push(Segment { a, b: bb, ta, tb });
    }
    for _ in 0..nr {
        let style_rule = u32_at(&mut pos)?;
        let loops_n = u32_at(&mut pos)? as usize;
        if loops_n > b.len() {
            return None;
        }
        let mut loops = vec![];
        for _ in 0..loops_n {
            let cnt = u32_at(&mut pos)? as usize;
            if cnt > b.len() {
                return None;
            }
            let mut l = vec![];
            for _ in 0..cnt {
                let i = u32_at(&mut pos)? as usize;
                if i >= ns {
                    return None;
                }
                l.push(i);
            }
            loops.push(l);
        }
        net.regions.push(Region { rule: if style_rule & 1 == 1 { FillRule::NonZero } else { FillRule::EvenOdd }, loops });
    }
    Some((net, styles))
}

/// Applies a `.fig` style override table (`styleID`, `strokeCap`, `strokeJoin`,
/// `cornerRadius`, `handleMirroring`) to the vertices that use each style.
pub(crate) fn apply_style_table(net: &mut VectorNetwork, styles: &[u32], table: &[Value]) {
    for entry in table {
        let Some(id) = entry.get("styleID").and_then(Value::as_u64) else { continue };
        for (v, s) in net.vertices.iter_mut().zip(styles) {
            if *s as u64 != id {
                continue;
            }
            if let Some(c) = entry.get("strokeCap").and_then(Value::as_str).and_then(cap_from) {
                v.cap = Some(c);
            }
            if let Some(j) = entry.get("strokeJoin").and_then(Value::as_str).and_then(join_from) {
                v.join = Some(j);
            }
            if let Some(r) = num(entry, "cornerRadius") {
                v.corner_radius = r.max(0.0);
            }
            if let Some(m) = entry.get("handleMirroring").and_then(Value::as_str) {
                v.mirror = mirror_from(m);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type SegRec = (u32, u32, [f32; 2], u32, [f32; 2]);

    fn blob(verts: &[(u32, f32, f32)], segs: &[SegRec], regions: &[(u32, Vec<Vec<u32>>)]) -> Vec<u8> {
        let mut b = vec![];
        for n in [verts.len(), segs.len(), regions.len()] {
            b.extend((n as u32).to_le_bytes());
        }
        for (s, x, y) in verts {
            b.extend(s.to_le_bytes());
            b.extend(x.to_le_bytes());
            b.extend(y.to_le_bytes());
        }
        for (s, a, ta, e, tb) in segs {
            b.extend(s.to_le_bytes());
            b.extend(a.to_le_bytes());
            b.extend(ta[0].to_le_bytes());
            b.extend(ta[1].to_le_bytes());
            b.extend(e.to_le_bytes());
            b.extend(tb[0].to_le_bytes());
            b.extend(tb[1].to_le_bytes());
        }
        for (rule, loops) in regions {
            b.extend(rule.to_le_bytes());
            b.extend((loops.len() as u32).to_le_bytes());
            for l in loops {
                b.extend((l.len() as u32).to_le_bytes());
                for i in l {
                    b.extend(i.to_le_bytes());
                }
            }
        }
        b
    }

    #[test]
    fn blob_decodes_vertices_segments_and_regions() {
        let b = blob(
            &[(0, 0.0, 0.0), (1, 10.0, 0.0), (0, 10.0, 10.0)],
            &[(0, 0, [0.0, 0.0], 1, [0.0, 0.0]), (0, 1, [0.0, 3.0], 2, [0.0, -3.0]), (0, 2, [0.0, 0.0], 0, [0.0, 0.0])],
            &[(1, vec![vec![0, 1, 2]])],
        );
        let (net, styles) = network_from_blob(&b, (2.0, 1.0)).unwrap();
        assert_eq!((net.vertices.len(), net.segments.len(), net.regions.len()), (3, 3, 1));
        assert_eq!(net.vertices[1].pos, Vec2::new(20.0, 0.0), "scaled by the normalised size");
        assert_eq!(net.segments[1].ta, Vec2::new(0.0, 3.0));
        assert_eq!(net.regions[0].rule, FillRule::NonZero);
        assert_eq!(net.regions[0].loops, vec![vec![0, 1, 2]]);
        assert_eq!(styles, vec![0, 1, 0]);
        // style table: vertices of style 1 get a round cap and a corner radius
        let mut net = net;
        apply_style_table(&mut net, &styles, &[json!({"styleID": 1, "strokeCap": "ROUND", "cornerRadius": 4})]);
        assert_eq!(net.vertices[1].cap, Some(StrokeCap::Round));
        assert_eq!(net.vertices[1].corner_radius, 4.0);
        assert_eq!(net.vertices[0].cap, None);
        assert_eq!(net.to_paths().len(), 1);
    }

    #[test]
    fn corrupt_blobs_are_rejected_not_panicked_on() {
        assert!(network_from_blob(&[], (1.0, 1.0)).is_none());
        assert!(network_from_blob(&[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], (1.0, 1.0)).is_none(), "truncated vertex");
        let mut huge = vec![];
        for _ in 0..3 {
            huge.extend(u32::MAX.to_le_bytes());
        }
        assert!(network_from_blob(&huge, (1.0, 1.0)).is_none());
        // segment pointing at a missing vertex
        let b = blob(&[(0, 0.0, 0.0)], &[(0, 0, [0.0; 2], 5, [0.0; 2])], &[]);
        assert!(network_from_blob(&b, (1.0, 1.0)).is_none());
        // region pointing at a missing segment
        let b = blob(&[(0, 0.0, 0.0)], &[], &[(1, vec![vec![3]])]);
        assert!(network_from_blob(&b, (1.0, 1.0)).is_none());
    }

    #[test]
    fn json_round_trip_keeps_per_vertex_properties() {
        let mut net = VectorNetwork::default();
        for p in [Vec2::new(0.0, 0.0), Vec2::new(50.0, 0.0), Vec2::new(50.0, 50.0)] {
            net.add_vertex(p);
        }
        net.connect(0, 1);
        net.connect(1, 2);
        net.connect(2, 0);
        net.segments[0].ta = Vec2::new(10.0, -5.0);
        net.vertices[1].corner_radius = 6.0;
        net.vertices[2].cap = Some(StrokeCap::Square);
        net.vertices[0].mirror = HandleMirror::Angle;
        net.regions.push(Region { rule: FillRule::EvenOdd, loops: vec![vec![0, 1, 2]] });
        let j = network_to_json(&net);
        assert_eq!(j["vertices"][1]["cornerRadius"], 6.0);
        assert_eq!(j["vertices"][2]["strokeCap"], "SQUARE");
        assert_eq!(j["vertices"][0]["handleMirroring"], "ANGLE");
        assert_eq!(j["regions"][0]["windingRule"], "EVENODD");
        assert_eq!(network_from_json(&j).unwrap(), net);
        // out-of-range indices are refused
        assert!(network_from_json(&json!({"vertices": [{"x": 0, "y": 0}], "segments": [{"start": 0, "end": 3}]})).is_none());
    }
}
