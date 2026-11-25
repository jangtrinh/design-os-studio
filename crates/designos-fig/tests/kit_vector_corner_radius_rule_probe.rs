//! Kit probe (ignored): how a vector's node `cornerRadius` reaches the vertices of its network. Figma's stored
//! `fillGeometry` is the oracle. For every kit vector with a network, a non-zero node `cornerRadius` and a stored
//! fill outline, Studio's live outline under each candidate rule is compared with the stored one (the scoreboard's
//! comparison: within 0.5 px, Hausdorff, at 4x), and the match count per rule is written out.
//!
//! - `KIT_CLONE` (or `DESIGNOS_KIT_PATH`): the kit clone to read.
//! - `PROBE_OUT`: file for the report.
//! - `PROBE_DUMP`: comma list of guids whose vertices, style entries and verdicts are listed in full.
#![allow(dead_code)]

mod kit_fidelity;

use designos_doc::*;
use kit_fidelity::geometry::{compare, truth, Clip, Verdict};
use kit_fidelity::keys::NodeKeys;
use kit_fidelity::raster::figma_paths;
use kit_fidelity::raw::{self, f32_of};
use kit_fidelity::scratch::{live_nodes, Pass};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// What a rule knows about one vertex.
struct Vtx {
    /// Radius the style table gives this vertex, when its entry has the field (an explicit 0 included).
    explicit: Option<f32>,
    /// The vertex has a style entry with other fields but no radius.
    styled_without_radius: bool,
    degree: usize,
    /// Every segment end at this vertex has a zero tangent.
    straight: bool,
    /// The turn at the vertex bends toward the inside of its loop (same sign as the loop's area).
    convex: Option<bool>,
    /// The radius the importer gave the vertex.
    imported: f32,
}

struct Rule {
    name: &'static str,
    /// The radius of a vertex under the rule, given the node radius.
    radius: fn(&Vtx, f32) -> f32,
}

fn rules() -> Vec<Rule> {
    vec![
        Rule { name: "A table only (today)", radius: |v, _| v.explicit.unwrap_or(0.0) },
        Rule { name: "B node radius on every vertex", radius: |_, r| r },
        Rule { name: "C node radius where the table has no radius field", radius: |v, r| v.explicit.unwrap_or(r) },
        Rule {
            name: "D as C, only where the vertex has no style entry at all",
            radius: |v, r| if v.explicit.is_none() && !v.styled_without_radius { r } else { v.explicit.unwrap_or(0.0) },
        },
        Rule { name: "E as C, only between straight segments", radius: |v, r| v.explicit.unwrap_or(if v.straight { r } else { 0.0 }) },
        Rule { name: "F as C, only convex corners", radius: |v, r| v.explicit.unwrap_or(if v.convex == Some(true) { r } else { 0.0 }) },
        Rule { name: "G as C, only degree 2", radius: |v, r| v.explicit.unwrap_or(if v.degree == 2 { r } else { 0.0 }) },
        Rule { name: "H as C, only degree 2 and straight", radius: |v, r| v.explicit.unwrap_or(if v.degree == 2 && v.straight { r } else { 0.0 }) },
        Rule {
            name: "I as C, only degree 2, straight and convex",
            radius: |v, r| v.explicit.unwrap_or(if v.degree == 2 && v.straight && v.convex == Some(true) { r } else { 0.0 }),
        },
        Rule { name: "K as the importer ships it", radius: |v, _| v.imported },
        Rule {
            name: "J node radius on every vertex the table does not set to 0",
            radius: |v, r| if v.explicit == Some(0.0) { 0.0 } else { v.explicit.unwrap_or(r) },
        },
    ]
}

struct Info {
    node_radius: f32,
    style_ids: Vec<u32>,
    table: Vec<kit_fidelity_kiwi::KiwiValue>,
}

mod kit_fidelity_kiwi {
    pub use designos_fig::kiwi::KiwiValue;
}

fn vertex_styles(blob: &[u8]) -> Vec<u32> {
    let u = |o: usize| blob.get(o..o + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]));
    let n = u(0).unwrap_or(0) as usize;
    (0..n).filter_map(|i| u(12 + i * 12)).collect()
}

fn verdict(v: &Verdict) -> &'static str {
    match v {
        Verdict::Match => "match",
        Verdict::Mismatch(_) => "MISS",
        Verdict::Unscored(_) => "unscored",
    }
}

fn describe(v: &Verdict) -> String {
    match v {
        Verdict::Match => "match".into(),
        Verdict::Mismatch(s) => format!("MISS ({s})"),
        Verdict::Unscored(s) => format!("unscored ({s})"),
    }
}

fn convexity(net: &VectorNetwork, vtx: usize) -> Option<bool> {
    // the loop a vertex of degree 2 sits in: walk both ways to a closed chain and take the shoelace sign
    let segs: Vec<usize> = (0..net.segments.len()).filter(|&s| net.segments[s].a == vtx || net.segments[s].b == vtx).collect();
    if segs.len() != 2 {
        return None;
    }
    let other = |s: usize, from: usize| if net.segments[s].a == from { net.segments[s].b } else { net.segments[s].a };
    let (mut chain, mut cur, mut seg) = (vec![vtx], other(segs[0], vtx), segs[0]);
    while cur != vtx {
        chain.push(cur);
        let next = (0..net.segments.len()).find(|&s| s != seg && (net.segments[s].a == cur || net.segments[s].b == cur))?;
        cur = other(next, cur);
        seg = next;
        if chain.len() > net.vertices.len() {
            return None;
        }
    }
    let area: f32 = (0..chain.len())
        .map(|i| {
            let (p, q) = (net.vertices[chain[i]].pos, net.vertices[chain[(i + 1) % chain.len()]].pos);
            p.x * q.y - q.x * p.y
        })
        .sum();
    let n = chain.len();
    let (p, c, q) = (net.vertices[chain[n - 1]].pos, net.vertices[chain[0]].pos, net.vertices[chain[1 % n]].pos);
    let cross = (c.x - p.x) * (q.y - c.y) - (c.y - p.y) * (q.x - c.x);
    Some(cross * area > 0.0)
}

#[test]
#[ignore = "reads KIT_CLONE, writes PROBE_OUT"]
fn kit_vector_corner_radius_rules() {
    let kit = std::env::var_os("KIT_CLONE").or_else(|| std::env::var_os("DESIGNOS_KIT_PATH")).expect("KIT_CLONE");
    let out = std::env::var("PROBE_OUT").expect("PROBE_OUT");
    let dump: Vec<String> = std::env::var("PROBE_DUMP").map(|s| s.split(',').map(String::from).collect()).unwrap_or_default();
    let bytes = std::fs::read(&kit).unwrap();
    let r = raw::decode(&bytes);
    let geo = truth(&r);
    let mut info: BTreeMap<String, Info> = BTreeMap::new();
    for rec in r.live() {
        let Some(g) = raw::record_guid(rec) else { continue };
        let Some(vd) = rec.get("vectorData") else { continue };
        let Some(bi) = vd.get("vectorNetworkBlob").and_then(raw::u64_of) else { continue };
        let Some(b) = r.blobs.get(bi as usize) else { continue };
        let node_radius = f32_of(rec, "cornerRadius").unwrap_or(0.0);
        let table = vd.get("styleOverrideTable").and_then(|t| t.as_array()).map(|a| a.to_vec()).unwrap_or_default();
        info.insert(g, Info { node_radius, style_ids: vertex_styles(b), table });
    }
    let blobs = r.blobs;
    drop(r.records);
    let mut doc = designos_fig::import_fig(&bytes).unwrap();
    doc.hydrate_all_pages();
    let keys = NodeKeys::build(&doc);
    let rules = rules();
    let mut log = String::new();
    let mut counts: Vec<[u32; 3]> = vec![[0; 3]; rules.len()];
    let mut counts_visible: Vec<[u32; 3]> = vec![[0; 3]; rules.len()];
    let mut total = 0;
    let mut by_class: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    for t in &geo {
        if t.fill.is_empty() {
            continue;
        }
        let Some(inf) = info.get(&t.guid) else { continue };
        if inf.node_radius <= 0.0 {
            continue;
        }
        let Some(&id) = keys.by_source.get(&t.guid) else { continue };
        let Ok(nodes) = live_nodes(&doc, id) else { continue };
        let root = &nodes[0];
        let NodeKind::Vector { network: Some(net), .. } = &root.kind else { continue };
        let fig = figma_paths(&t.fill, &blobs);
        let hidden = keys.is_hidden(id);
        total += 1;
        let vtxs: Vec<Vtx> = (0..net.vertices.len())
            .map(|i| {
                let style = inf.style_ids.get(i).copied().unwrap_or(0);
                let entry = inf.table.iter().find(|e| e.get("styleID").and_then(raw::u64_of) == Some(u64::from(style)));
                let explicit = entry.and_then(|e| f32_of(e, "cornerRadius"));
                let styled_without_radius = entry.is_some() && explicit.is_none();
                let straight = net.segments.iter().filter(|s| s.a == i || s.b == i).all(|s| if s.a == i { s.ta == Vec2::ZERO } else { s.tb == Vec2::ZERO });
                Vtx { explicit, styled_without_radius, degree: net.degree(i), straight, convex: convexity(net, i), imported: net.vertices[i].corner_radius }
            })
            .collect();
        let mut row = String::new();
        let mut verdicts = Vec::new();
        for (k, rule) in rules.iter().enumerate() {
            let mut n2 = net.clone();
            for (v, x) in n2.vertices.iter_mut().zip(&vtxs) {
                v.corner_radius = (rule.radius)(x, inf.node_radius);
            }
            let mut c = nodes.clone();
            c[0].kind = NodeKind::Vector { paths: n2.to_paths(), network: Some(n2) };
            let v = compare(&c, Pass::Fill, &fig, Clip::None);
            let idx = match v {
                Verdict::Match => 0,
                Verdict::Mismatch(_) => 1,
                Verdict::Unscored(_) => 2,
            };
            counts[k][idx] += 1;
            if !hidden {
                counts_visible[k][idx] += 1;
            }
            let _ = write!(row, " {}={}", &rule.name[..1], verdict(&v));
            verdicts.push(describe(&v));
        }
        let class: String = verdicts.iter().zip(&rules).map(|(v, r)| if v.starts_with("match") { &r.name[..1] } else { "-" }).collect();
        by_class.entry(class.clone()).or_default().push(0);
        let _ = writeln!(
            log,
            "VEC {} R={} verts={} regions={} hidden={}{}  matches:[{}]",
            t.guid,
            inf.node_radius,
            net.vertices.len(),
            net.regions.len(),
            hidden,
            row,
            class
        );
        if dump.contains(&t.guid) {
            let _ = writeln!(log, "  DUMP {}", t.guid);
            for (i, (v, x)) in net.vertices.iter().zip(&vtxs).enumerate() {
                let _ = writeln!(
                    log,
                    "   v{i} pos=({:.2},{:.2}) style={} table_radius={:?} styled_no_radius={} degree={} straight={} convex={:?} imported_radius={}",
                    v.pos.x,
                    v.pos.y,
                    inf.style_ids.get(i).copied().unwrap_or(0),
                    x.explicit,
                    x.styled_without_radius,
                    x.degree,
                    x.straight,
                    x.convex,
                    v.corner_radius
                );
            }
            for (i, s) in net.segments.iter().enumerate() {
                let _ = writeln!(log, "   s{i} {}->{} ta=({:.2},{:.2}) tb=({:.2},{:.2})", s.a, s.b, s.ta.x, s.ta.y, s.tb.x, s.tb.y);
            }
            let _ = writeln!(log, "   table={:?}", inf.table);
            for (k, v) in verdicts.iter().enumerate() {
                let _ = writeln!(log, "   rule {}: {v}", rules[k].name);
            }
            for (i, odd) in &t.fill {
                let _ = writeln!(log, "   stored fill odd={odd} path={:?}", designos_fig::commands_blob_to_path(&blobs[*i], *odd));
            }
        }
    }
    let _ = writeln!(log, "\nvectors with a network, a node radius and a stored fill: {total}");
    for (k, rule) in rules.iter().enumerate() {
        let c = counts[k];
        let cv = counts_visible[k];
        let _ = writeln!(log, "RULE {:<62} match {:4} miss {:4} unscored {:3} | visible only: match {:4} miss {:4}", rule.name, c[0], c[1], c[2], cv[0], cv[1]);
    }
    for (class, v) in by_class {
        let _ = writeln!(log, "CLASS {:>5} vectors match under exactly: {class}", v.len());
    }
    std::fs::write(out, log).unwrap();
}
