//! Vector networks: Figma's vector model.
//!
//! A [`VectorNetwork`] is a graph of vertices joined by bézier segments. Unlike
//! a path, a vertex can join more than two segments, and filled areas are
//! explicit [`Region`]s, loops of segments. Every vertex carries its own corner
//! radius, stroke cap and handle mirroring.
//!
//! `NodeKind::Vector::paths` stays the rendered geometry; [`VectorNetwork::to_paths`]
//! derives it (applying corner radii) and [`VectorNetwork::from_paths`] goes
//! the other way. All of this is pure and has no UI or renderer dependency.

use crate::vector_fit::*;
pub use crate::vector_fit::{cubic_point, fit_polyline, Anchor, Cubic};
use crate::{Affine, FillRule, Path, PathCmd, Rect, StrokeCap, StrokeJoin, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const EPS: f32 = 1e-4;

/// How the two handles of a vertex follow each other when one is dragged
/// (Figma's `handleMirroring`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HandleMirror {
    /// Handles move independently.
    #[default]
    None,
    /// Handles stay collinear, lengths are free (asymmetric).
    Angle,
    /// Handles stay collinear and equally long (mirrored).
    AngleAndLength,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vertex {
    pub pos: Vec2,
    /// Radius of the rounded corner at this vertex; 0 is sharp.
    #[serde(default)]
    pub corner_radius: f32,
    /// Cap of the stroke when the vertex ends a path; `None` inherits the node's cap.
    #[serde(default)]
    pub cap: Option<StrokeCap>,
    /// Join of the stroke at this vertex; `None` inherits the node's join.
    #[serde(default)]
    pub join: Option<StrokeJoin>,
    #[serde(default)]
    pub mirror: HandleMirror,
}

impl Vertex {
    pub fn new(pos: Vec2) -> Self {
        Self { pos, corner_radius: 0.0, cap: None, join: None, mirror: HandleMirror::None }
    }
}

/// A bézier from vertex `a` to vertex `b`. Tangents are handle offsets
/// relative to their vertex; a zero tangent means no handle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    pub a: usize,
    pub b: usize,
    #[serde(default)]
    pub ta: Vec2,
    #[serde(default)]
    pub tb: Vec2,
}

/// An area of the network: loops of segment indices. Several loops form holes
/// according to `rule`. An unfilled region is a face the user unfilled: it paints nothing and its
/// loops are only stroked, but it still counts as a region, so the closed-loop fallback of a network
/// without regions does not bring the fill back.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Region {
    pub rule: FillRule,
    pub loops: Vec<Vec<usize>>,
    /// Whether the area is painted. Written only when false: files from before it existed read as filled.
    #[serde(default = "filled_by_default", skip_serializing_if = "is_filled")]
    pub filled: bool,
}

fn filled_by_default() -> bool {
    true
}

fn is_filled(filled: &bool) -> bool {
    *filled
}

/// Which end of a segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum End {
    Start,
    End,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VectorNetwork {
    pub vertices: Vec<Vertex>,
    pub segments: Vec<Segment>,
    pub regions: Vec<Region>,
}

/// A bounded area of the planar network: an outer loop plus holes.
#[derive(Clone, Debug, PartialEq)]
pub struct Face {
    /// Segment indices; the outer boundary first, then the boundaries of nested components.
    pub loops: Vec<Vec<usize>>,
    pub area: f32,
}

impl Face {
    /// Sorted unique segment indices, to compare against regions.
    pub fn key(&self) -> Vec<usize> {
        self.loops.iter().flatten().copied().collect::<BTreeSet<_>>().into_iter().collect()
    }
}

fn near(a: Vec2, b: Vec2) -> bool {
    dist(a, b) < EPS
}

fn sorted_key(loops: &[Vec<usize>]) -> Vec<usize> {
    loops.iter().flatten().copied().collect::<BTreeSet<_>>().into_iter().collect()
}

/// A loop traversed in order: `(segment, reversed)`.
type Oriented = Vec<(usize, bool)>;

impl VectorNetwork {
    // ------------------------------------------------------------ queries
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty() && self.vertices.is_empty()
    }

    /// Whether every index is in range and every number finite. Deserialised or imported
    /// networks are checked with this before anything indexes into them.
    pub fn is_valid(&self) -> bool {
        let (nv, ns) = (self.vertices.len(), self.segments.len());
        let ok = |p: Vec2| p.x.is_finite() && p.y.is_finite();
        self.vertices.iter().all(|v| ok(v.pos) && v.corner_radius.is_finite())
            && self.segments.iter().all(|s| s.a < nv && s.b < nv && ok(s.ta) && ok(s.tb))
            && self.regions.iter().all(|r| r.loops.iter().flatten().all(|i| *i < ns))
    }

    pub fn pos(&self, v: usize) -> Vec2 {
        self.vertices[v].pos
    }

    /// The segment as a cubic from `a` to `b`.
    pub fn cubic(&self, seg: usize) -> Cubic {
        let s = &self.segments[seg];
        let (pa, pb) = (self.vertices[s.a].pos, self.vertices[s.b].pos);
        [pa, add(pa, s.ta), add(pb, s.tb), pb]
    }

    pub fn is_straight(&self, seg: usize) -> bool {
        let s = &self.segments[seg];
        len(s.ta) < EPS && len(s.tb) < EPS
    }

    /// Segment ends meeting at vertex `v`. A self-loop contributes both ends.
    pub fn ends_at(&self, v: usize) -> Vec<(usize, End)> {
        let mut out = vec![];
        for (i, s) in self.segments.iter().enumerate() {
            if s.a == v {
                out.push((i, End::Start));
            }
            if s.b == v {
                out.push((i, End::End));
            }
        }
        out
    }

    pub fn degree(&self, v: usize) -> usize {
        self.ends_at(v).len()
    }

    /// True for vertices that end a path (at most one segment).
    pub fn is_endpoint(&self, v: usize) -> bool {
        self.degree(v) <= 1
    }

    pub fn tangent(&self, seg: usize, end: End) -> Vec2 {
        match end {
            End::Start => self.segments[seg].ta,
            End::End => self.segments[seg].tb,
        }
    }

    pub fn set_tangent(&mut self, seg: usize, end: End, t: Vec2) {
        let t = if len(t) < EPS { Vec2::ZERO } else { t };
        match end {
            End::Start => self.segments[seg].ta = t,
            End::End => self.segments[seg].tb = t,
        }
    }

    pub fn vertex_of(&self, seg: usize, end: End) -> usize {
        match end {
            End::Start => self.segments[seg].a,
            End::End => self.segments[seg].b,
        }
    }

    /// The point of segment `seg` at parameter `t`.
    pub fn point_on(&self, seg: usize, t: f32) -> Vec2 {
        cubic_point(&self.cubic(seg), t)
    }

    /// Closest point on segment `seg` to `p`: `(t, distance)`.
    pub fn nearest_on(&self, seg: usize, p: Vec2) -> (f32, f32) {
        cubic_nearest(&self.cubic(seg), p)
    }

    /// The nearest segment to `p` within `max_dist`: `(segment, t, distance)`.
    pub fn hit_segment(&self, p: Vec2, max_dist: f32) -> Option<(usize, f32, f32)> {
        (0..self.segments.len())
            .map(|i| (i, self.nearest_on(i, p)))
            .filter(|(_, (_, d))| *d <= max_dist)
            .min_by(|a, b| a.1 .1.total_cmp(&b.1 .1))
            .map(|(i, (t, d))| (i, t, d))
    }

    // ------------------------------------------------------------ conversion
    /// Builds a network from paths. Closed sub-paths become regions (one per
    /// path, using the path's fill rule); open sub-paths are plain segments.
    pub fn from_paths(paths: &[Path]) -> Self {
        let mut net = VectorNetwork::default();
        for path in paths {
            let mut loops: Vec<Vec<usize>> = vec![];
            let mut start: Option<usize> = None;
            let mut last: usize = 0;
            let mut segs: Vec<usize> = vec![];
            let mut restart_at: Option<Vec2> = None;
            let finish_open = |segs: &mut Vec<usize>| segs.clear();
            let new_vertex = |net: &mut VectorNetwork, p: Vec2| -> usize {
                net.vertices.push(Vertex::new(p));
                net.vertices.len() - 1
            };
            for cmd in &path.cmds {
                match *cmd {
                    PathCmd::MoveTo(p) => {
                        finish_open(&mut segs);
                        let vi = new_vertex(&mut net, p);
                        start = Some(vi);
                        last = vi;
                        restart_at = None;
                    }
                    PathCmd::LineTo(_) | PathCmd::QuadTo(..) | PathCmd::CubicTo(..) => {
                        if start.is_none() {
                            // a path that starts without MoveTo continues from its first point, or begins there
                            let p0 = restart_at.take().unwrap_or(match *cmd {
                                PathCmd::LineTo(p) | PathCmd::QuadTo(_, p) | PathCmd::CubicTo(_, _, p) => p,
                                _ => Vec2::ZERO,
                            });
                            let vi = new_vertex(&mut net, p0);
                            start = Some(vi);
                            last = vi;
                            if matches!(*cmd, PathCmd::LineTo(p) if near(p, p0)) {
                                continue;
                            }
                        }
                        let prev = net.vertices[last].pos;
                        let (ta, tb, p) = match *cmd {
                            PathCmd::LineTo(p) => (Vec2::ZERO, Vec2::ZERO, p),
                            PathCmd::QuadTo(c, p) => {
                                let c1 = add(prev, mul(sub(c, prev), 2.0 / 3.0));
                                let c2 = add(p, mul(sub(c, p), 2.0 / 3.0));
                                (sub(c1, prev), sub(c2, p), p)
                            }
                            PathCmd::CubicTo(c1, c2, p) => (sub(c1, prev), sub(c2, p), p),
                            _ => unreachable!(),
                        };
                        let vi = new_vertex(&mut net, p);
                        net.segments.push(Segment { a: last, b: vi, ta, tb });
                        segs.push(net.segments.len() - 1);
                        last = vi;
                    }
                    PathCmd::Close => {
                        let Some(s) = start else { continue };
                        if !segs.is_empty() {
                            if last != s && near(net.vertices[last].pos, net.vertices[s].pos) {
                                // the end duplicates the start: merge it into the start vertex
                                let dup = last;
                                for sg in net.segments.iter_mut() {
                                    if sg.b == dup {
                                        sg.b = s;
                                    }
                                }
                                if dup + 1 == net.vertices.len() {
                                    net.vertices.pop();
                                }
                            } else if last != s {
                                net.segments.push(Segment { a: last, b: s, ta: Vec2::ZERO, tb: Vec2::ZERO });
                                segs.push(net.segments.len() - 1);
                            }
                            loops.push(std::mem::take(&mut segs));
                        }
                        restart_at = Some(net.vertices[s].pos);
                        start = None;
                    }
                }
            }
            if !loops.is_empty() {
                net.regions.push(Region { rule: path.fill_rule, loops, filled: true });
            }
        }
        net.compact();
        net.detect_mirroring();
        net
    }

    /// Like [`Self::from_paths`], then merges vertices that coincide within `tol`.
    pub fn from_paths_welded(paths: &[Path], tol: f32) -> Self {
        let mut net = Self::from_paths(paths);
        net.weld(tol);
        net
    }

    /// Merges vertices closer than `tol` (path ends meeting, shared corners).
    #[allow(clippy::needless_range_loop)]
    pub fn weld(&mut self, tol: f32) {
        let n = self.vertices.len();
        let mut map: Vec<usize> = (0..n).collect();
        for i in 0..n {
            if map[i] != i {
                continue;
            }
            for j in i + 1..n {
                if map[j] == j && dist(self.vertices[i].pos, self.vertices[j].pos) <= tol {
                    map[j] = i;
                }
            }
        }
        if map.iter().enumerate().all(|(i, m)| i == *m) {
            return;
        }
        for s in &mut self.segments {
            s.a = map[s.a];
            s.b = map[s.b];
        }
        // segments collapsed to a point disappear
        let dead: BTreeSet<usize> =
            self.segments.iter().enumerate().filter(|(_, s)| s.a == s.b && len(s.ta) < EPS && len(s.tb) < EPS).map(|(i, _)| i).collect();
        self.remove_segments_keep_vertices(&dead);
        self.compact();
    }

    /// Detects mirroring of imported / drawn vertices from their handles.
    fn detect_mirroring(&mut self) {
        for v in 0..self.vertices.len() {
            let ends = self.ends_at(v);
            if ends.len() != 2 {
                continue;
            }
            let (t1, t2) = (self.tangent(ends[0].0, ends[0].1), self.tangent(ends[1].0, ends[1].1));
            if len(t1) < EPS || len(t2) < EPS {
                continue;
            }
            let (n1, n2) = (norm(t1), norm(t2));
            if cross(n1, n2).abs() < 1e-3 && dot(n1, n2) < 0.0 {
                self.vertices[v].mirror = if (len(t1) - len(t2)).abs() < 1e-2 * len(t1).max(1.0) { HandleMirror::AngleAndLength } else { HandleMirror::Angle };
            }
        }
    }

    /// Removes vertices without segments, fixes loops, drops empty regions.
    pub fn compact(&mut self) {
        let mut used = vec![false; self.vertices.len()];
        for s in &self.segments {
            used[s.a] = true;
            used[s.b] = true;
        }
        if used.iter().all(|u| *u) {
            return;
        }
        let mut map = vec![usize::MAX; self.vertices.len()];
        let mut next = 0;
        for (i, u) in used.iter().enumerate() {
            if *u {
                map[i] = next;
                next += 1;
            }
        }
        let old = std::mem::take(&mut self.vertices);
        self.vertices = old.into_iter().enumerate().filter(|(i, _)| used[*i]).map(|(_, v)| v).collect();
        for s in &mut self.segments {
            s.a = map[s.a];
            s.b = map[s.b];
        }
    }

    /// Orders a loop of segments into a traversal.
    pub fn orient_loop(&self, lp: &[usize]) -> Option<Oriented> {
        let n = lp.len();
        if n == 0 || lp.iter().any(|s| *s >= self.segments.len()) {
            return None;
        }
        let seg = |i: usize| &self.segments[lp[i]];
        if n == 1 {
            let s = seg(0);
            return (s.a == s.b).then(|| vec![(lp[0], false)]);
        }
        // the first segment ends at a vertex shared with the second
        let (s0, s1) = (seg(0), seg(1));
        let forward = s0.b == s1.a || s0.b == s1.b;
        let reversed_ok = s0.a == s1.a || s0.a == s1.b;
        if !forward && !reversed_ok {
            return None;
        }
        let mut out = vec![(lp[0], !forward)];
        let mut cur = if forward { s0.b } else { s0.a };
        for &sidx in &lp[1..] {
            let s = &self.segments[sidx];
            if s.a == cur {
                out.push((sidx, false));
                cur = s.b;
            } else if s.b == cur {
                out.push((sidx, true));
                cur = s.a;
            } else {
                return None;
            }
        }
        let start = if forward { s0.a } else { s0.b };
        (cur == start).then_some(out)
    }

    fn oriented_cubic(&self, (seg, rev): (usize, bool)) -> Cubic {
        let c = self.cubic(seg);
        if rev {
            [c[3], c[2], c[1], c[0]]
        } else {
            c
        }
    }

    fn start_vertex(&self, (seg, rev): (usize, bool)) -> usize {
        if rev {
            self.segments[seg].b
        } else {
            self.segments[seg].a
        }
    }

    /// Regions and stroke-only chains, with corner radii applied: the filled
    /// loops (one `Path` per region) and the rest as one open/closed set.
    pub fn to_paths_split(&self) -> (Vec<Path>, Option<Path>) {
        if !self.is_valid() {
            return (vec![], None);
        }
        let mut regions = vec![];
        let mut covered = vec![false; self.segments.len()];
        // an unfilled region paints nothing and its loops are stroke only, like segments that no region covers
        for r in self.regions.iter().filter(|r| r.filled) {
            let mut cmds = vec![];
            for lp in &r.loops {
                if let Some(o) = self.orient_loop(lp) {
                    for (s, _) in &o {
                        covered[*s] = true;
                    }
                    self.emit_chain(&o, true, &mut cmds);
                }
            }
            if !cmds.is_empty() {
                regions.push(Path { cmds, fill_rule: r.rule });
            }
        }
        let mut cmds = vec![];
        for (chain, closed) in self.leftover_chains(&covered) {
            self.emit_chain(&chain, closed, &mut cmds);
        }
        let rest = (!cmds.is_empty()).then_some(Path { cmds, fill_rule: FillRule::NonZero });
        (regions, rest)
    }

    /// The geometry to render: filled loops, then the remaining segments.
    pub fn to_paths(&self) -> Vec<Path> {
        let (mut r, rest) = self.to_paths_split();
        r.extend(rest);
        r
    }

    /// The geometry the fill paints: the filled regions. A network with no regions at all (an unfilled region
    /// counts as one) fills each closed loop instead (an open chain stays unfilled): Figma stores a fill outline for exactly those loops, the one
    /// kind of region-less network a `.fig` file holds a visible fill for.
    pub fn fill_paths(&self) -> Vec<Path> {
        if self.regions.is_empty() && self.is_valid() {
            return self.closed_loop_paths();
        }
        self.to_paths_split().0
    }

    /// One path per closed loop of segments no region covers, filled with the non-zero rule.
    fn closed_loop_paths(&self) -> Vec<Path> {
        let covered = vec![false; self.segments.len()];
        self.leftover_chains(&covered)
            .into_iter()
            .filter(|(_, closed)| *closed)
            .filter_map(|(chain, _)| {
                let mut cmds = vec![];
                self.emit_chain(&chain, true, &mut cmds);
                (!cmds.is_empty()).then_some(Path { cmds, fill_rule: FillRule::NonZero })
            })
            .collect()
    }

    /// True when some segments are not part of any filled region, so the
    /// stroke geometry differs from the fill geometry.
    pub fn has_stroke_only_segments(&self) -> bool {
        self.to_paths_split().1.is_some()
    }

    /// Chains of segments not covered by a region, walked through degree-2 vertices.
    fn leftover_chains(&self, covered: &[bool]) -> Vec<(Oriented, bool)> {
        let n = self.vertices.len();
        let mut at: Vec<Vec<(usize, End)>> = vec![vec![]; n];
        for (i, s) in self.segments.iter().enumerate() {
            if covered[i] {
                continue;
            }
            at[s.a].push((i, End::Start));
            at[s.b].push((i, End::End));
        }
        let mut seen = vec![false; self.segments.len()];
        let mut out: Vec<(Oriented, bool)> = vec![];
        let walk = |first: usize, from_end: End, seen: &mut Vec<bool>, stop_at: Option<usize>| -> (Oriented, bool) {
            // travel along `first` starting at its `from_end`
            let mut chain: Oriented = vec![];
            let mut seg = first;
            let mut end = from_end;
            let mut closed = false;
            loop {
                seen[seg] = true;
                let rev = end == End::End;
                chain.push((seg, rev));
                let to = if rev { self.segments[seg].a } else { self.segments[seg].b };
                if Some(to) == stop_at {
                    closed = true;
                    break;
                }
                if at[to].len() != 2 {
                    break;
                }
                // continue on the other segment end at `to`
                let next = at[to].iter().find(|(s2, e2)| !(*s2 == seg && *e2 == if rev { End::Start } else { End::End }));
                match next {
                    Some(&(s2, e2)) if !seen[s2] => {
                        seg = s2;
                        end = e2;
                    }
                    _ => break,
                }
            }
            (chain, closed)
        };
        // chains from vertices that do not have exactly two covered-free segments
        for ends in &at {
            if ends.len() == 2 || ends.is_empty() {
                continue;
            }
            for &(s, e) in ends {
                if !seen[s] {
                    let (c, closed) = walk(s, e, &mut seen, None);
                    out.push((c, closed));
                }
            }
        }
        // pure cycles
        for s in 0..self.segments.len() {
            if covered[s] || seen[s] {
                continue;
            }
            let start_v = self.segments[s].a;
            let (c, closed) = walk(s, End::Start, &mut seen, Some(start_v));
            out.push((c, closed));
        }
        out
    }

    /// Start and end vertex of every open stroke-only chain, in the order they
    /// appear as open sub-paths of [`Self::to_paths`].
    pub fn open_chain_ends(&self) -> Vec<(usize, usize)> {
        let mut covered = vec![false; self.segments.len()];
        for r in self.regions.iter().filter(|r| r.filled) {
            for lp in &r.loops {
                if self.orient_loop(lp).is_some() {
                    for s in lp {
                        covered[*s] = true;
                    }
                }
            }
        }
        self.leftover_chains(&covered)
            .into_iter()
            .filter(|(_, closed)| !closed)
            .map(|(chain, _)| {
                let first = self.start_vertex(chain[0]);
                let (ls, lr) = *chain.last().unwrap();
                (first, if lr { self.segments[ls].a } else { self.segments[ls].b })
            })
            .collect()
    }

    /// Appends the path of a traversal. `closed` ends with `Close`.
    fn emit_chain(&self, chain: &[(usize, bool)], closed: bool, cmds: &mut Vec<PathCmd>) {
        let n = chain.len();
        if n == 0 {
            return;
        }
        let cubics: Vec<Cubic> = chain.iter().map(|&o| self.oriented_cubic(o)).collect();
        let starts: Vec<usize> = chain.iter().map(|&o| self.start_vertex(o)).collect();
        // trims[k] = (t where segment k starts, t where it ends) and arcs before segment k
        let mut t_start = vec![0.0f32; n];
        let mut t_end = vec![1.0f32; n];
        let mut arcs: Vec<Option<Cubic>> = vec![None; n];
        let junctions: Vec<usize> = if closed { (0..n).collect() } else { (1..n).collect() };
        let lens: Vec<f32> = cubics.iter().map(cubic_length).collect();
        for &k in &junctions {
            let prev = (k + n - 1) % n;
            let vtx = starts[k];
            let r = self.vertices[vtx].corner_radius;
            if r.is_nan() || r <= EPS {
                continue;
            }
            let cp = &cubics[prev];
            let cn = &cubics[k];
            let pt = cn[0];
            let d1 = first_dir(&[cp[2], cp[1], cp[0]], pt);
            let d2 = first_dir(&[cn[1], cn[2], cn[3]], pt);
            if d1 == Vec2::ZERO || d2 == Vec2::ZERO {
                continue;
            }
            let cosphi = dot(d1, d2).clamp(-1.0, 1.0);
            let phi = cosphi.acos();
            if !(0.02..=std::f32::consts::PI - 0.02).contains(&phi) {
                continue;
            }
            let half_tan = (phi / 2.0).tan();
            // how much of each neighbour this corner may consume
            let share = |idx: usize, other_vertex: usize| -> f32 {
                let other_rounded = (closed || (idx > 0 && idx + 1 < n)) && self.vertices[other_vertex].corner_radius > EPS;
                lens[idx] * if other_rounded { 0.5 } else { 1.0 }
            };
            let prev_other = starts[prev];
            let next_other = starts[(k + 1) % n];
            let avail_p = share(prev, prev_other);
            let avail_n = share(k, next_other);
            let t = (r / half_tan).min(avail_p).min(avail_n);
            if t <= EPS {
                continue;
            }
            let rr = t * half_tan;
            let tp = cubic_param_at_distance(&[cp[3], cp[2], cp[1], cp[0]], t);
            let tn = cubic_param_at_distance(cn, t);
            let a = cubic_point(cp, 1.0 - tp);
            let b = cubic_point(cn, tn);
            let kk = 4.0 / 3.0 * ((std::f32::consts::PI - phi) / 4.0).tan() * rr;
            let ta = norm(cubic_deriv(cp, 1.0 - tp));
            let tb = norm(cubic_deriv(cn, tn));
            t_end[prev] = t_end[prev].min(1.0 - tp);
            t_start[k] = t_start[k].max(tn);
            arcs[k] = Some([a, add(a, mul(ta, kk)), sub(b, mul(tb, kk)), b]);
        }
        let seg_cmd = |c: &Cubic| {
            if is_flat(c, 1e-3) {
                PathCmd::LineTo(c[3])
            } else {
                PathCmd::CubicTo(c[1], c[2], c[3])
            }
        };
        let trimmed = |k: usize| -> Cubic {
            if t_start[k] <= 0.0 && t_end[k] >= 1.0 {
                cubics[k]
            } else {
                cubic_sub(&cubics[k], t_start[k], t_end[k].max(t_start[k]))
            }
        };
        let arc_cmd = |a: &Cubic| PathCmd::CubicTo(a[1], a[2], a[3]);
        let start = match (closed, arcs[0]) {
            (true, Some(a)) => a[3],
            _ => trimmed(0)[0],
        };
        cmds.push(PathCmd::MoveTo(start));
        for k in 0..n {
            if k > 0 {
                if let Some(a) = &arcs[k] {
                    cmds.push(arc_cmd(a));
                }
            }
            let cmd = seg_cmd(&trimmed(k));
            // a straight edge back to the start is what `Close` draws anyway
            if closed && k + 1 == n && arcs[0].is_none() && matches!(cmd, PathCmd::LineTo(_)) {
                continue;
            }
            cmds.push(cmd);
        }
        if closed {
            if let Some(a) = &arcs[0] {
                cmds.push(arc_cmd(a));
            }
            cmds.push(PathCmd::Close);
        }
    }

    pub fn bounds(&self) -> Rect {
        paths_bounds(&self.to_paths())
    }

    pub fn transformed(&self, a: &Affine) -> Self {
        let mut out = self.clone();
        for v in &mut out.vertices {
            v.pos = a.apply(v.pos);
        }
        let lin = |t: Vec2| Vec2::new(a.a * t.x + a.c * t.y, a.b * t.x + a.d * t.y);
        for s in &mut out.segments {
            s.ta = lin(s.ta);
            s.tb = lin(s.tb);
        }
        out
    }

    pub fn translate(&mut self, d: Vec2) {
        for v in &mut self.vertices {
            v.pos = add(v.pos, d);
        }
    }

    // ------------------------------------------------------------ editing
    pub fn add_vertex(&mut self, pos: Vec2) -> usize {
        self.vertices.push(Vertex::new(pos));
        self.vertices.len() - 1
    }

    /// Joins two vertices with a segment (no-op if they are already joined).
    pub fn connect(&mut self, a: usize, b: usize) -> Option<usize> {
        if a == b || self.segments.iter().any(|s| (s.a == a && s.b == b) || (s.a == b && s.b == a)) {
            return None;
        }
        self.segments.push(Segment { a, b, ta: Vec2::ZERO, tb: Vec2::ZERO });
        Some(self.segments.len() - 1)
    }

    /// Splits segment `seg` at `t`, keeping its shape. Returns the new vertex.
    pub fn split_segment(&mut self, seg: usize, t: f32) -> usize {
        let t = t.clamp(0.001, 0.999);
        let curved = !self.is_straight(seg);
        let c = self.cubic(seg);
        let (l, r) = cubic_split(&c, t);
        let s = self.segments[seg].clone();
        let nv = self.add_vertex(l[3]);
        // first half keeps the index, second half is new
        self.segments[seg] = Segment { a: s.a, b: nv, ta: sub(l[1], l[0]), tb: sub(l[2], l[3]) };
        if !curved {
            self.segments[seg].ta = Vec2::ZERO;
            self.segments[seg].tb = Vec2::ZERO;
        }
        let new_seg = self.segments.len();
        let (rta, rtb) = if curved { (sub(r[1], r[0]), sub(r[2], r[3])) } else { (Vec2::ZERO, Vec2::ZERO) };
        self.segments.push(Segment { a: nv, b: s.b, ta: rta, tb: rtb });
        if curved {
            self.vertices[nv].mirror = HandleMirror::Angle;
        }
        // regions follow
        let orientations: Vec<(usize, usize, bool)> = self
            .regions
            .iter()
            .enumerate()
            .flat_map(|(ri, r)| r.loops.iter().enumerate().map(move |(li, l)| (ri, li, l)))
            .filter(|(_, _, l)| l.contains(&seg))
            .map(|(ri, li, l)| {
                let rev = self.orient_loop_prior(l, seg, &s).unwrap_or(false);
                (ri, li, rev)
            })
            .collect();
        for (ri, li, rev) in orientations {
            let lp = &mut self.regions[ri].loops[li];
            let pos = lp.iter().position(|x| *x == seg).unwrap();
            // traversal a->b: [seg, new]; b->a: [new, seg]
            lp.insert(if rev { pos } else { pos + 1 }, new_seg);
        }
        nv
    }

    /// Whether `lp` traverses `seg` backwards, judged before the split changed it.
    fn orient_loop_prior(&self, lp: &[usize], seg: usize, original: &Segment) -> Option<bool> {
        // after the split `seg` ends at the new vertex; compare the neighbours of `seg` in the loop
        let n = lp.len();
        let i = lp.iter().position(|x| *x == seg)?;
        if n == 1 {
            return Some(false);
        }
        let next = lp[(i + 1) % n];
        let prev = lp[(i + n - 1) % n];
        let touches = |sidx: usize, v: usize| sidx < self.segments.len() && sidx != seg && (self.segments[sidx].a == v || self.segments[sidx].b == v);
        let fwd = touches(next, original.b) && touches(prev, original.a);
        let rev = touches(next, original.a) && touches(prev, original.b);
        match (fwd, rev) {
            (true, _) => Some(false),
            (false, true) => Some(true),
            _ => Some(false),
        }
    }

    /// Removes the segments (keeps all vertices) and repairs regions.
    fn remove_segments_keep_vertices(&mut self, dead: &BTreeSet<usize>) {
        if dead.is_empty() {
            return;
        }
        let mut map = vec![usize::MAX; self.segments.len()];
        let mut next = 0;
        for (i, slot) in map.iter_mut().enumerate() {
            if !dead.contains(&i) {
                *slot = next;
                next += 1;
            }
        }
        let old = std::mem::take(&mut self.segments);
        self.segments = old.into_iter().enumerate().filter(|(i, _)| !dead.contains(i)).map(|(_, s)| s).collect();
        for r in &mut self.regions {
            r.loops.retain(|l| !l.iter().any(|s| dead.contains(s)));
            for l in &mut r.loops {
                for s in l.iter_mut() {
                    *s = map[*s];
                }
            }
        }
        self.regions.retain(|r| !r.loops.is_empty());
    }

    pub fn delete_segment(&mut self, seg: usize) {
        self.remove_segments_keep_vertices(&BTreeSet::from([seg]));
        self.compact();
    }

    /// Deletes a vertex. A vertex joining two segments is dissolved and the
    /// neighbours are connected; endpoints and junctions lose their segments.
    pub fn delete_vertex(&mut self, v: usize) {
        let ends = self.ends_at(v);
        let distinct: BTreeSet<usize> = ends.iter().map(|e| e.0).collect();
        if ends.len() == 2 && distinct.len() == 2 {
            let (s1, e1) = ends[0];
            let (s2, e2) = ends[1];
            // build the merged segment running other1 -> other2 along s1 then s2
            let other1 = self.vertex_of(s1, if e1 == End::Start { End::End } else { End::Start });
            let other2 = self.vertex_of(s2, if e2 == End::Start { End::End } else { End::Start });
            let t1 = self.tangent(s1, if e1 == End::Start { End::End } else { End::Start });
            let t2 = self.tangent(s2, if e2 == End::Start { End::End } else { End::Start });
            let merged = Segment { a: other1, b: other2, ta: t1, tb: t2 };
            // loops: replace s1 and s2 by the merged segment (stored in s1's slot)
            self.segments[s1] = merged;
            for r in &mut self.regions {
                for l in &mut r.loops {
                    let mut out = Vec::with_capacity(l.len());
                    for &x in l.iter() {
                        if x == s2 {
                            continue;
                        }
                        out.push(x);
                    }
                    *l = out;
                }
                // a loop reduced to fewer than two straight segments encloses nothing
                let straight: Vec<bool> = r.loops.iter().map(|l| l.len() < 2).collect();
                let mut i = 0;
                r.loops.retain(|_| {
                    let keep = !straight[i];
                    i += 1;
                    keep
                });
            }
            let loop_len_ok: Vec<bool> = self.regions.iter().map(|r| r.loops.iter().all(|l| l.len() >= 3 || l.iter().any(|s| !self.is_straight(*s)))).collect();
            let mut i = 0;
            self.regions.retain(|r| {
                let keep = loop_len_ok[i] && !r.loops.is_empty();
                i += 1;
                keep
            });
            self.remove_segments_keep_vertices(&BTreeSet::from([s2]));
        } else {
            let dead: BTreeSet<usize> = distinct;
            self.remove_segments_keep_vertices(&dead);
        }
        self.remove_vertex_slot(v);
    }

    /// Removes a vertex that no segment uses any more, shifting indices.
    fn remove_vertex_slot(&mut self, v: usize) {
        // segments that still reference it (should not happen) are dropped too
        let dead: BTreeSet<usize> = self.segments.iter().enumerate().filter(|(_, s)| s.a == v || s.b == v).map(|(i, _)| i).collect();
        self.remove_segments_keep_vertices(&dead);
        self.vertices.remove(v);
        for s in &mut self.segments {
            if s.a > v {
                s.a -= 1;
            }
            if s.b > v {
                s.b -= 1;
            }
        }
    }

    /// Deletes several vertices (indices refer to the network before the call).
    pub fn delete_vertices(&mut self, vs: &BTreeSet<usize>) {
        for &v in vs.iter().rev() {
            if v < self.vertices.len() {
                self.delete_vertex(v);
            }
        }
        self.compact();
    }

    /// Joins vertices `a` and `b` into one at their midpoint. Returns the
    /// merged vertex. Faces that appear (a closed path) are filled.
    pub fn join_vertices(&mut self, a: usize, b: usize) -> Option<usize> {
        if a == b || a >= self.vertices.len() || b >= self.vertices.len() {
            return None;
        }
        let before: BTreeSet<Vec<usize>> = self.faces().iter().map(Face::key).collect();
        let mid = lerp(self.pos(a), self.pos(b), 0.5);
        // keep absolute handle positions when the vertex moves
        for s in &mut self.segments {
            if s.a == b {
                s.a = a;
            }
            if s.b == b {
                s.b = a;
            }
        }
        self.vertices[a].pos = mid;
        if self.vertices[a].cap.is_none() {
            self.vertices[a].cap = self.vertices[b].cap;
        }
        // a segment that now starts and ends at `a` without handles is degenerate
        let dead: BTreeSet<usize> =
            self.segments.iter().enumerate().filter(|(_, s)| s.a == s.b && len(s.ta) < EPS && len(s.tb) < EPS).map(|(i, _)| i).collect();
        self.remove_segments_keep_vertices(&dead);
        // drop vertex b
        self.vertices.remove(b);
        for s in &mut self.segments {
            if s.a > b {
                s.a -= 1;
            }
            if s.b > b {
                s.b -= 1;
            }
        }
        let merged = if a > b { a - 1 } else { a };
        self.fill_new_faces(&before);
        Some(merged)
    }

    /// Fills every face that was not part of `before`.
    pub fn fill_new_faces(&mut self, before: &BTreeSet<Vec<usize>>) {
        for f in self.faces() {
            let key = f.key();
            if !before.contains(&key) && !self.regions.iter().any(|r| sorted_key(&r.loops) == key) {
                self.regions.push(Region { rule: FillRule::EvenOdd, loops: f.loops, filled: true });
            }
        }
    }

    /// Cuts the network at vertex `v`: the vertex is duplicated and the
    /// segments (except the first) move to the copy. Returns the copy.
    pub fn split_vertex(&mut self, v: usize) -> Option<usize> {
        let ends = self.ends_at(v);
        if ends.len() < 2 {
            return None;
        }
        let nv = self.vertices.len();
        let mut copy = self.vertices[v].clone();
        copy.mirror = HandleMirror::None;
        self.vertices.push(copy);
        self.vertices[v].mirror = HandleMirror::None;
        let affected: BTreeSet<usize> = ends.iter().map(|e| e.0).collect();
        for &(s, e) in ends.iter().skip(1) {
            match e {
                End::Start => self.segments[s].a = nv,
                End::End => self.segments[s].b = nv,
            }
        }
        // regions whose loops passed through v are no longer closed
        let broken: BTreeSet<usize> = affected;
        self.regions.retain(|r| !r.loops.iter().flatten().any(|s| broken.contains(s)));
        Some(nv)
    }

    /// Cuts a path at a point of a segment. Returns the two new endpoints.
    pub fn cut_at(&mut self, seg: usize, t: f32) -> Option<(usize, usize)> {
        let v = self.split_segment(seg, t);
        let w = self.split_vertex(v)?;
        Some((v, w))
    }

    // ------------------------------------------------------------ handles
    /// After the handle at `(seg, end)` changed, moves the vertex's other
    /// handle according to the vertex's mirroring.
    pub fn enforce_mirror(&mut self, seg: usize, end: End) {
        let v = self.vertex_of(seg, end);
        let mode = self.vertices[v].mirror;
        if mode == HandleMirror::None {
            return;
        }
        let ends = self.ends_at(v);
        if ends.len() != 2 {
            return;
        }
        let other = if ends[0] == (seg, end) { ends[1] } else { ends[0] };
        let moved = self.tangent(seg, end);
        let cur = self.tangent(other.0, other.1);
        let new = match mode {
            HandleMirror::AngleAndLength => mul(moved, -1.0),
            HandleMirror::Angle => {
                if len(cur) < EPS {
                    Vec2::ZERO
                } else {
                    mul(norm(moved), -len(cur))
                }
            }
            HandleMirror::None => cur,
        };
        self.set_tangent(other.0, other.1, new);
    }

    /// Sets the mirroring of a vertex and snaps its handles to it.
    pub fn set_mirror(&mut self, v: usize, mode: HandleMirror) {
        self.vertices[v].mirror = mode;
        let ends = self.ends_at(v);
        if mode == HandleMirror::None || ends.len() != 2 {
            return;
        }
        // the longer handle is the reference
        let l0 = len(self.tangent(ends[0].0, ends[0].1));
        let l1 = len(self.tangent(ends[1].0, ends[1].1));
        let r = if l0 >= l1 { ends[0] } else { ends[1] };
        if len(self.tangent(r.0, r.1)) < EPS {
            return;
        }
        self.enforce_mirror(r.0, r.1);
    }

    /// Removes the handles of a vertex (makes it a sharp corner).
    pub fn straighten(&mut self, v: usize) {
        for (s, e) in self.ends_at(v) {
            self.set_tangent(s, e, Vec2::ZERO);
        }
    }

    // ------------------------------------------------------------ faces
    /// All bounded areas of the planar network, holes included.
    pub fn faces(&self) -> Vec<Face> {
        let ns = self.segments.len();
        if ns == 0 || !self.is_valid() {
            return vec![];
        }
        // half-edge h: segment h/2, forward when h even
        let dir_of = |h: usize| -> Vec2 {
            let c = self.oriented_cubic((h / 2, h % 2 == 1));
            first_dir(&[c[1], c[2], c[3]], c[0])
        };
        let origin = |h: usize| -> usize {
            let s = &self.segments[h / 2];
            if h.is_multiple_of(2) {
                s.a
            } else {
                s.b
            }
        };
        let dest = |h: usize| origin(h ^ 1);
        let mut out_of: Vec<Vec<usize>> = vec![vec![]; self.vertices.len()];
        for h in 0..ns * 2 {
            out_of[origin(h)].push(h);
        }
        for list in &mut out_of {
            list.sort_by(|&x, &y| {
                let (ax, ay) = (dir_of(x), dir_of(y));
                ay.y.atan2(ay.x).total_cmp(&ax.y.atan2(ax.x)).reverse()
            });
        }
        let next = |h: usize| -> usize {
            let w = dest(h);
            let twin = h ^ 1;
            let list = &out_of[w];
            let i = list.iter().position(|x| *x == twin).unwrap_or(0);
            list[(i + list.len() - 1) % list.len()]
        };
        let mut visited = vec![false; ns * 2];
        struct Cycle {
            segs: Vec<usize>,
            poly: Vec<Vec2>,
            area: f32,
        }
        let mut cycles: Vec<Cycle> = vec![];
        for h0 in 0..ns * 2 {
            if visited[h0] {
                continue;
            }
            let mut hs = vec![];
            let mut h = h0;
            while !visited[h] {
                visited[h] = true;
                hs.push(h);
                h = next(h);
                if hs.len() > ns * 2 + 2 {
                    break;
                }
            }
            // a bridge or dangling segment is walked there and back: cut the walk apart at
            // every such pair so that each piece is one simple boundary
            let mut pieces: Vec<Vec<usize>> = vec![];
            split_walk(hs, &mut pieces);
            for hs in pieces {
                if hs.is_empty() {
                    continue;
                }
                let mut poly = vec![];
                for &h in &hs {
                    let c = self.oriented_cubic((h / 2, h % 2 == 1));
                    poly.push(c[0]);
                    let mut pts = vec![];
                    flatten_cubic(&c, 0.25, &mut pts);
                    pts.pop();
                    poly.extend(pts);
                }
                let mut area = 0.0;
                for i in 0..poly.len() {
                    let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
                    area += cross(p, q);
                }
                area *= 0.5;
                cycles.push(Cycle { segs: hs.iter().map(|h| h / 2).collect(), poly, area });
            }
        }
        let mut faces: Vec<(Face, Vec<Vec2>)> = vec![];
        for c in cycles.iter().filter(|c| c.area > 1e-3) {
            faces.push((Face { loops: vec![c.segs.clone()], area: c.area }, c.poly.clone()));
        }
        // holes: a clockwise boundary (the outside of a component) lies in the smallest face
        // that does not share a segment with it; the faces sharing segments are its own inside
        for c in cycles.iter().filter(|c| c.area < -1e-3) {
            let probe = if c.poly.len() >= 2 { lerp(c.poly[0], c.poly[1], 0.5) } else { c.poly[0] };
            let mut best: Option<usize> = None;
            for (i, (f, poly)) in faces.iter().enumerate() {
                let shares = f.loops.iter().flatten().any(|s| c.segs.contains(s));
                if shares || !point_in_polygon(probe, poly) {
                    continue;
                }
                if best.is_none_or(|b| f.area < faces[b].0.area) {
                    best = Some(i);
                }
            }
            if let Some(i) = best {
                faces[i].0.loops.push(c.segs.clone());
            }
        }
        faces.into_iter().map(|(f, _)| f).collect()
    }

    /// Flattened contours of a face (outer boundary first), for hit testing and drawing.
    pub fn face_contours(&self, face: &Face) -> Vec<Vec<Vec2>> {
        face.loops.iter().filter_map(|lp| self.loop_polygon(lp, 0.25)).collect()
    }

    /// The polygon of a loop of segments.
    pub fn loop_polygon(&self, lp: &[usize], tol: f32) -> Option<Vec<Vec2>> {
        let o = self.orient_loop(lp)?;
        let mut poly = vec![];
        for &x in &o {
            let c = self.oriented_cubic(x);
            poly.push(c[0]);
            let mut pts = vec![];
            flatten_cubic(&c, tol, &mut pts);
            pts.pop();
            poly.extend(pts);
        }
        Some(poly)
    }

    /// The smallest face containing `p`.
    pub fn face_at(&self, p: Vec2) -> Option<Face> {
        self.faces()
            .into_iter()
            .filter(|f| {
                let c = self.face_contours(f);
                point_in_even_odd(p, &c)
            })
            .min_by(|a, b| a.area.total_cmp(&b.area))
    }

    /// The region that covers exactly the segments of the face, filled or not.
    pub fn region_of_face(&self, face: &Face) -> Option<usize> {
        let key = face.key();
        self.regions.iter().position(|r| sorted_key(&r.loops) == key)
    }

    /// Fills or unfills a face; returns whether it is filled afterwards. A face with a region flips the
    /// region's `filled`, so unfilling the only loop of a network does not leave it without regions.
    pub fn toggle_face(&mut self, face: &Face) -> bool {
        match self.region_of_face(face) {
            Some(i) => {
                self.regions[i].filled = !self.regions[i].filled;
                self.regions[i].filled
            }
            None => {
                self.regions.push(Region { rule: FillRule::EvenOdd, loops: face.loops.clone(), filled: true });
                true
            }
        }
    }

    // ------------------------------------------------------------ simplify
    /// Reduces the number of vertices of runs between junctions, endpoints
    /// and corner-rounded vertices while staying within `tol`.
    /// Returns how many vertices were removed.
    pub fn simplify(&mut self, tol: f32) -> usize {
        let before = self.vertices.len();
        let ns = self.segments.len();
        // runs of segments through plain degree-2 vertices
        let breaks = |net: &VectorNetwork, v: usize| -> bool { net.degree(v) != 2 || net.vertices[v].corner_radius > EPS };
        let mut used = vec![false; ns];
        struct Run {
            segs: Oriented,
            closed: bool,
        }
        let mut runs: Vec<Run> = vec![];
        let ends_of: Vec<Vec<(usize, End)>> = (0..self.vertices.len()).map(|v| self.ends_at(v)).collect();
        let walk = |first: usize, start_end: End, used: &mut Vec<bool>, net: &VectorNetwork| -> Run {
            let mut segs: Oriented = vec![];
            let mut seg = first;
            let mut end = start_end;
            let start_v = net.vertex_of(first, start_end);
            let mut closed = false;
            loop {
                used[seg] = true;
                let rev = end == End::End;
                segs.push((seg, rev));
                let to = if rev { net.segments[seg].a } else { net.segments[seg].b };
                if to == start_v {
                    closed = true;
                    break;
                }
                if breaks(net, to) {
                    break;
                }
                let next = ends_of[to].iter().find(|(s2, e2)| !(*s2 == seg && *e2 == if rev { End::Start } else { End::End }));
                match next {
                    Some(&(s2, e2)) if !used[s2] => {
                        seg = s2;
                        end = e2;
                    }
                    _ => break,
                }
            }
            Run { segs, closed }
        };
        for (v, ends) in ends_of.iter().enumerate() {
            if !breaks(self, v) {
                continue;
            }
            for &(s, e) in ends {
                if !used[s] {
                    let r = walk(s, e, &mut used, self);
                    runs.push(r);
                }
            }
        }
        for s in 0..ns {
            if !used[s] {
                let r = walk(s, End::Start, &mut used, self);
                runs.push(r);
            }
        }
        // refit every run
        let mut new_net = VectorNetwork { vertices: self.vertices.clone(), segments: vec![], regions: vec![] };
        let mut run_new_segs: Vec<Vec<usize>> = vec![]; // new segments per run, in run direction
        let mut seg_run: Vec<Option<(usize, bool)>> = vec![None; ns];
        for (ri, run) in runs.iter().enumerate() {
            for &(s, rev) in &run.segs {
                seg_run[s] = Some((ri, rev));
            }
            let mut poly = vec![self.oriented_cubic(run.segs[0])[0]];
            for &o in &run.segs {
                flatten_cubic(&self.oriented_cubic(o), (tol * 0.1).max(0.01), &mut poly);
            }
            let first_v = self.start_vertex(run.segs[0]);
            let last_seg = *run.segs.last().unwrap();
            let last_v = if last_seg.1 { self.segments[last_seg.0].a } else { self.segments[last_seg.0].b };
            let anchors = if run.segs.len() <= 1 && !run.closed { vec![] } else { fit_polyline(&poly, run.closed && first_v == last_v, tol, 40.0) };
            let mut ids: Vec<usize> = vec![];
            if anchors.len() < 2 {
                // nothing to simplify: copy as is
                for &o in &run.segs {
                    let c = self.oriented_cubic(o);
                    let (a, b) = (self.start_vertex(o), if o.1 { self.segments[o.0].a } else { self.segments[o.0].b });
                    new_net.segments.push(Segment { a, b, ta: sub(c[1], c[0]), tb: sub(c[2], c[3]) });
                    ids.push(new_net.segments.len() - 1);
                }
                run_new_segs.push(ids);
                continue;
            }
            let closed_loop = run.closed && first_v == last_v;
            // vertices: first/last keep their ids; interior anchors are new
            let mut vids: Vec<usize> = vec![first_v];
            for a in &anchors[1..anchors.len() - if closed_loop { 0 } else { 1 }] {
                new_net.vertices.push(Vertex::new(a.pos));
                vids.push(new_net.vertices.len() - 1);
            }
            if !closed_loop {
                vids.push(last_v);
            }
            let n_curves = if closed_loop { anchors.len() } else { anchors.len() - 1 };
            for i in 0..n_curves {
                let (a0, a1) = (&anchors[i], &anchors[(i + 1) % anchors.len()]);
                let (va, vb) = (vids[i], if closed_loop && i + 1 == anchors.len() { vids[0] } else { vids[i + 1] });
                let (pa, pb) = (new_net.vertices[va].pos, new_net.vertices[vb].pos);
                let ta = a0.hout.map(|h| sub(h, pa)).unwrap_or(Vec2::ZERO);
                let tb = a1.hin.map(|h| sub(h, pb)).unwrap_or(Vec2::ZERO);
                new_net.segments.push(Segment { a: va, b: vb, ta, tb });
                ids.push(new_net.segments.len() - 1);
            }
            // smooth interior anchors
            for &vi in vids.iter().skip(1).take(anchors.len().saturating_sub(2)) {
                let hs = new_net.ends_at(vi);
                if hs.len() == 2 && hs.iter().all(|&(s, e)| len(new_net.tangent(s, e)) > EPS) {
                    new_net.vertices[vi].mirror = HandleMirror::Angle;
                }
            }
            run_new_segs.push(ids);
        }
        // regions
        for r in &self.regions {
            let mut loops = vec![];
            for lp in &r.loops {
                let Some(o) = self.orient_loop(lp) else { continue };
                let mut nl: Vec<usize> = vec![];
                let mut emitted: BTreeSet<usize> = BTreeSet::new();
                for &(s, rev) in &o {
                    let Some((ri, run_rev)) = seg_run[s] else { continue };
                    if !emitted.insert(ri) {
                        continue;
                    }
                    let ids = &run_new_segs[ri];
                    if rev == run_rev {
                        nl.extend(ids.iter().copied());
                    } else {
                        nl.extend(ids.iter().rev().copied());
                    }
                }
                loops.push(nl);
            }
            if loops.iter().all(|l| !l.is_empty()) {
                new_net.regions.push(Region { rule: r.rule, loops, filled: r.filled });
            }
        }
        *self = new_net;
        self.compact();
        before.saturating_sub(self.vertices.len())
    }
}

/// Splits a closed walk of half-edges at every pair that traverses a segment both ways
/// (half-edge `h` and its twin `h ^ 1`): the walk between them is the boundary of whatever
/// hangs off the segment, the rest continues around the face.
fn split_walk(hs: Vec<usize>, out: &mut Vec<Vec<usize>>) {
    let pos: std::collections::HashMap<usize, usize> = hs.iter().enumerate().map(|(i, h)| (*h, i)).collect();
    for (i, h) in hs.iter().enumerate() {
        if let Some(&j) = pos.get(&(h ^ 1)) {
            if j != i {
                let (a, b) = (i.min(j), i.max(j));
                let inner = hs[a + 1..b].to_vec();
                let mut outer = hs[..a].to_vec();
                outer.extend_from_slice(&hs[b + 1..]);
                split_walk(inner, out);
                split_walk(outer, out);
                return;
            }
        }
    }
    out.push(hs);
}

/// First direction (unit) from `origin` towards the first candidate that is not on it.
fn first_dir(cands: &[Vec2], origin: Vec2) -> Vec2 {
    for c in cands {
        let d = sub(*c, origin);
        if len(d) > EPS {
            return norm(d);
        }
    }
    Vec2::ZERO
}

/// Bounding box of paths, sampling the curves.
pub fn paths_bounds(paths: &[Path]) -> Rect {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    let mut grow = |p: Vec2| {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    };
    for path in paths {
        let mut cur = Vec2::ZERO;
        let mut start = Vec2::ZERO;
        for c in &path.cmds {
            match *c {
                PathCmd::MoveTo(p) => {
                    cur = p;
                    start = p;
                    grow(p);
                }
                PathCmd::LineTo(p) => {
                    cur = p;
                    grow(p);
                }
                PathCmd::QuadTo(c1, p) => {
                    let cc = [cur, lerp(cur, c1, 2.0 / 3.0), lerp(p, c1, 2.0 / 3.0), p];
                    for i in 0..=16 {
                        grow(cubic_point(&cc, i as f32 / 16.0));
                    }
                    cur = p;
                }
                PathCmd::CubicTo(c1, c2, p) => {
                    let cc = [cur, c1, c2, p];
                    for i in 0..=24 {
                        grow(cubic_point(&cc, i as f32 / 24.0));
                    }
                    cur = p;
                }
                PathCmd::Close => cur = start,
            }
        }
    }
    if x0 > x1 {
        return Rect::new(0.0, 0.0, 0.0, 0.0);
    }
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

/// Crossing-number point-in-polygon.
pub fn point_in_polygon(p: Vec2, poly: &[Vec2]) -> bool {
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Even-odd containment across several contours.
pub fn point_in_even_odd(p: Vec2, contours: &[Vec<Vec2>]) -> bool {
    contours.iter().filter(|c| point_in_polygon(p, c)).count() % 2 == 1
}

/// Signed area of a polygon (positive when counter-clockwise in a y-up system).
pub fn polygon_area(poly: &[Vec2]) -> f32 {
    let mut a = 0.0;
    for i in 0..poly.len() {
        a += cross(poly[i], poly[(i + 1) % poly.len()]);
    }
    a * 0.5
}

/// Cubic béziers of a path in order, closed sub-paths included (the closing
/// line is added).
pub fn path_curves(path: &Path) -> Vec<(Vec<Cubic>, bool)> {
    let mut out = vec![];
    let mut cur: Vec<Cubic> = vec![];
    let mut p = Vec2::ZERO;
    let mut start = Vec2::ZERO;
    let mut open = false;
    let flush = |cur: &mut Vec<Cubic>, closed: bool, out: &mut Vec<(Vec<Cubic>, bool)>| {
        if !cur.is_empty() {
            out.push((std::mem::take(cur), closed));
        }
    };
    for c in &path.cmds {
        match *c {
            PathCmd::MoveTo(q) => {
                flush(&mut cur, false, &mut out);
                p = q;
                start = q;
                open = true;
            }
            PathCmd::LineTo(q) => {
                if !open {
                    start = q;
                    p = q;
                    open = true;
                    continue;
                }
                cur.push([p, p, q, q]);
                p = q;
            }
            PathCmd::QuadTo(c1, q) => {
                cur.push([p, lerp(p, c1, 2.0 / 3.0), lerp(q, c1, 2.0 / 3.0), q]);
                p = q;
            }
            PathCmd::CubicTo(c1, c2, q) => {
                cur.push([p, c1, c2, q]);
                p = q;
            }
            PathCmd::Close => {
                if dist(p, start) > EPS {
                    cur.push([p, p, start, start]);
                }
                flush(&mut cur, true, &mut out);
                p = start;
            }
        }
    }
    flush(&mut cur, false, &mut out);
    out
}

/// Flattened polylines of a path: `(points, closed)` per sub-path.
pub fn flatten_path(path: &Path, tol: f32) -> Vec<(Vec<Vec2>, bool)> {
    path_curves(path)
        .into_iter()
        .map(|(curves, closed)| {
            let mut pts = vec![curves[0][0]];
            for c in &curves {
                flatten_cubic(c, tol, &mut pts);
            }
            if closed && pts.len() > 1 && near(pts[0], pts[pts.len() - 1]) {
                pts.pop();
            }
            (pts, closed)
        })
        .collect()
}

/// Rebuilds a path from fitted anchors.
pub fn anchors_to_path(anchors: &[Anchor], closed: bool, rule: FillRule) -> Path {
    let mut cmds = vec![];
    let Some(first) = anchors.first() else { return Path { cmds, fill_rule: rule } };
    cmds.push(PathCmd::MoveTo(first.pos));
    let seg = |cmds: &mut Vec<PathCmd>, a: &Anchor, b: &Anchor| {
        if a.hout.is_some() || b.hin.is_some() {
            cmds.push(PathCmd::CubicTo(a.hout.unwrap_or(a.pos), b.hin.unwrap_or(b.pos), b.pos));
        } else {
            cmds.push(PathCmd::LineTo(b.pos));
        }
    };
    for w in anchors.windows(2) {
        seg(&mut cmds, &w[0], &w[1]);
    }
    if closed && anchors.len() > 1 {
        let last = anchors.last().unwrap();
        // a straight edge back to the start is what `Close` draws
        if last.hout.is_some() || first.hin.is_some() {
            seg(&mut cmds, last, first);
        }
        cmds.push(PathCmd::Close);
    }
    Path { cmds, fill_rule: rule }
}

/// Merges `paths` into one list of curve-fitted paths from polylines
/// (used after boolean operations produced polygons).
pub fn fit_polygons(contours: &[Vec<Vec2>], rule: FillRule, tol: f32) -> Path {
    let mut cmds: Vec<PathCmd> = vec![];
    for c in contours {
        let anchors = fit_polyline(c, true, tol, 35.0);
        if anchors.len() < 2 {
            continue;
        }
        cmds.extend(anchors_to_path(&anchors, true, rule).cmds);
    }
    Path { cmds, fill_rule: rule }
}

/// All segments of `net` keyed by the unordered pair of vertices (utility for tests and diffing).
pub fn segment_pairs(net: &VectorNetwork) -> BTreeMap<(usize, usize), usize> {
    let mut m = BTreeMap::new();
    for s in &net.segments {
        *m.entry((s.a.min(s.b), s.a.max(s.b))).or_insert(0) += 1;
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f32, y: f32) -> Vec2 {
        Vec2::new(x, y)
    }

    fn rect_path(x: f32, y: f32, w: f32, h: f32) -> Path {
        Path {
            cmds: vec![PathCmd::MoveTo(v(x, y)), PathCmd::LineTo(v(x + w, y)), PathCmd::LineTo(v(x + w, y + h)), PathCmd::LineTo(v(x, y + h)), PathCmd::Close],
            fill_rule: FillRule::NonZero,
        }
    }

    fn line_path(pts: &[Vec2]) -> Path {
        let mut cmds = vec![PathCmd::MoveTo(pts[0])];
        cmds.extend(pts[1..].iter().map(|p| PathCmd::LineTo(*p)));
        Path { cmds, fill_rule: FillRule::NonZero }
    }

    /// Sum of the absolute areas of the closed sub-paths.
    fn area(paths: &[Path]) -> f32 {
        paths.iter().flat_map(|p| flatten_path(p, 0.01)).filter(|(_, closed)| *closed).map(|(pts, _)| polygon_area(&pts).abs()).sum()
    }

    #[test]
    fn open_and_closed_paths_round_trip() {
        let open = line_path(&[v(0.0, 0.0), v(100.0, 0.0), v(100.0, 100.0)]);
        let closed = Path {
            cmds: vec![
                PathCmd::MoveTo(v(0.0, 0.0)),
                PathCmd::CubicTo(v(10.0, -20.0), v(40.0, -20.0), v(50.0, 0.0)),
                PathCmd::LineTo(v(50.0, 50.0)),
                PathCmd::Close,
            ],
            fill_rule: FillRule::EvenOdd,
        };
        for p in [open, closed] {
            let net = VectorNetwork::from_paths(std::slice::from_ref(&p));
            assert_eq!(net.to_paths(), vec![p.clone()], "lossless");
        }
        let n = VectorNetwork::from_paths(&[rect_path(0.0, 0.0, 10.0, 10.0)]);
        assert_eq!((n.vertices.len(), n.segments.len(), n.regions.len()), (4, 4, 1));
    }

    #[test]
    fn duplicate_closing_vertex_is_merged_and_quads_become_cubics() {
        let p = Path {
            cmds: vec![
                PathCmd::MoveTo(v(0.0, 0.0)),
                PathCmd::LineTo(v(10.0, 0.0)),
                PathCmd::LineTo(v(10.0, 10.0)),
                PathCmd::LineTo(v(0.0, 0.0)),
                PathCmd::Close,
            ],
            fill_rule: FillRule::NonZero,
        };
        let n = VectorNetwork::from_paths(&[p]);
        assert_eq!((n.vertices.len(), n.segments.len()), (3, 3));
        let q = Path { cmds: vec![PathCmd::MoveTo(v(0.0, 0.0)), PathCmd::QuadTo(v(30.0, 30.0), v(60.0, 0.0))], fill_rule: FillRule::NonZero };
        let n = VectorNetwork::from_paths(&[q]);
        assert!(!n.is_straight(0));
        assert!((n.point_on(0, 0.5).y - 15.0).abs() < 1e-3, "quad control (30,30) gives apex 15");
    }

    #[test]
    fn corner_radius_cuts_the_exact_amount_of_area() {
        let mut n = VectorNetwork::from_paths(&[rect_path(0.0, 0.0, 100.0, 100.0)]);
        assert!((area(&n.to_paths()) - 10000.0).abs() < 1.0);
        for vtx in &mut n.vertices {
            vtx.corner_radius = 10.0;
        }
        let a = area(&n.to_paths());
        let want = 10000.0 - (4.0 - std::f32::consts::PI) * 100.0;
        assert!((a - want).abs() < 1.0, "{a} vs {want}");
        // one corner only
        let mut n = VectorNetwork::from_paths(&[rect_path(0.0, 0.0, 100.0, 100.0)]);
        n.vertices[2].corner_radius = 20.0;
        let want = 10000.0 - (1.0 - std::f32::consts::FRAC_PI_4) * 400.0;
        assert!((area(&n.to_paths()) - want).abs() < 1.0);
        // an oversized radius is clamped to the available edge and never inverts
        let mut n = VectorNetwork::from_paths(&[rect_path(0.0, 0.0, 40.0, 40.0)]);
        for vtx in &mut n.vertices {
            vtx.corner_radius = 500.0;
        }
        let a = area(&n.to_paths());
        assert!(a > 0.0 && a < 1600.0, "{a}");
    }

    #[test]
    fn corner_radius_on_open_path_applies_to_interior_vertices_only() {
        let mut n = VectorNetwork::from_paths(&[line_path(&[v(0.0, 0.0), v(50.0, 0.0), v(50.0, 50.0)])]);
        for vtx in &mut n.vertices {
            vtx.corner_radius = 10.0;
        }
        let p = n.to_paths();
        assert_eq!(p[0].cmds.first(), Some(&PathCmd::MoveTo(v(0.0, 0.0))), "ends stay put");
        assert!(matches!(p[0].cmds.last(), Some(PathCmd::LineTo(q)) if *q == v(50.0, 50.0)));
        assert!(p[0].cmds.iter().any(|c| matches!(c, PathCmd::CubicTo(..))), "the middle corner is an arc");
    }

    fn two_squares() -> VectorNetwork {
        // squares sharing the edge x = 50: a 3x2 grid of vertices
        let mut n = VectorNetwork::default();
        for p in [v(0.0, 0.0), v(50.0, 0.0), v(100.0, 0.0), v(0.0, 50.0), v(50.0, 50.0), v(100.0, 50.0)] {
            n.add_vertex(p);
        }
        for (a, b) in [(0, 1), (1, 2), (3, 4), (4, 5), (0, 3), (1, 4), (2, 5)] {
            n.connect(a, b);
        }
        n
    }

    #[test]
    fn faces_of_a_network_with_a_shared_edge() {
        let n = two_squares();
        let faces = n.faces();
        assert_eq!(faces.len(), 2, "{faces:?}");
        assert!(faces.iter().all(|f| (f.area - 2500.0).abs() < 1.0));
        assert_eq!(n.degree(1), 3, "a vertex can join three segments");
        let f = n.face_at(v(25.0, 25.0)).unwrap();
        assert_eq!(f.key().len(), 4);
        assert!(n.face_at(v(500.0, 5.0)).is_none());
    }

    #[test]
    fn faces_report_holes_of_nested_loops() {
        let n = VectorNetwork::from_paths(&[rect_path(0.0, 0.0, 100.0, 100.0), rect_path(30.0, 30.0, 40.0, 40.0)]);
        let faces = n.faces();
        assert_eq!(faces.len(), 2);
        let outer = faces.iter().find(|f| f.area > 5000.0).unwrap();
        assert_eq!(outer.loops.len(), 2, "the outer face has the inner square as a hole");
        let at_ring = n.face_at(v(10.0, 10.0)).unwrap();
        assert_eq!(at_ring.loops.len(), 2);
        let at_inner = n.face_at(v(50.0, 50.0)).unwrap();
        assert!((at_inner.area - 1600.0).abs() < 1.0, "the smallest containing face wins");
    }

    #[test]
    fn paint_bucket_fills_and_unfills_faces() {
        let mut n = two_squares();
        assert!(n.regions.is_empty());
        let f = n.face_at(v(25.0, 25.0)).unwrap();
        assert!(n.toggle_face(&f));
        assert_eq!(n.regions.len(), 1);
        let (fills, rest) = n.to_paths_split();
        assert_eq!(fills.len(), 1);
        assert!((area(&fills) - 2500.0).abs() < 1.0);
        assert!(rest.is_some(), "the other segments are stroke only");
        assert!(n.has_stroke_only_segments());
        let f = n.face_at(v(25.0, 25.0)).unwrap();
        assert_eq!(n.region_of_face(&f), Some(0));
        assert!(!n.toggle_face(&f));
        assert_eq!(n.regions.len(), 1, "the region stays, unfilled");
        assert!(!n.regions[0].filled);
        assert!(n.fill_paths().is_empty(), "an unfilled region paints nothing");
        assert!(n.has_stroke_only_segments(), "its loop is only stroked now");
        assert!(n.toggle_face(&f), "toggling again fills it");
        assert!(n.regions[0].filled);
        assert!(!n.toggle_face(&f));
        for p in [v(25.0, 25.0), v(75.0, 25.0)] {
            let f = n.face_at(p).unwrap();
            n.toggle_face(&f);
        }
        assert_eq!(n.regions.len(), 2);
        assert!(n.regions.iter().all(|r| r.filled));
        assert!((area(&n.fill_paths()) - 5000.0).abs() < 1.0);
    }

    #[test]
    fn unfilling_the_only_loop_leaves_it_unfilled_rather_than_falling_back_to_the_closed_loop_rule() {
        let mut n = VectorNetwork::from_paths(&[rect_path(0.0, 0.0, 50.0, 50.0)]);
        assert_eq!(n.fill_paths().len(), 1);
        let f = n.face_at(v(25.0, 25.0)).unwrap();
        assert!(!n.toggle_face(&f));
        assert!(n.fill_paths().is_empty());
        assert!(n.has_stroke_only_segments());
        let (fills, rest) = n.to_paths_split();
        assert!(fills.is_empty() && rest.is_some());
        assert_eq!(n.open_chain_ends(), vec![], "a closed loop has no ends");
        // the flag is written only when it is not the default, and a network without it reads as filled
        let json = serde_json::to_string(&n).unwrap();
        assert!(json.contains("\"filled\":false"), "{json}");
        let back: VectorNetwork = serde_json::from_str(&json).unwrap();
        assert_eq!(back, n);
        assert!(back.fill_paths().is_empty());
        n.toggle_face(&f);
        let json = serde_json::to_string(&n).unwrap();
        assert!(!json.contains("filled"), "{json}");
        let old: VectorNetwork = serde_json::from_str(&json).unwrap();
        assert!(old.regions[0].filled);
    }

    #[test]
    fn a_network_without_regions_fills_its_closed_loops_and_not_its_open_chains() {
        let mut n = VectorNetwork::from_paths(&[rect_path(0.0, 0.0, 50.0, 50.0), rect_path(100.0, 0.0, 50.0, 50.0)]);
        n.regions.clear();
        // two closed loops and nothing else: both fill, one path each
        let fills = n.fill_paths();
        assert_eq!(fills.len(), 2);
        assert!((area(&fills) - 5000.0).abs() < 1.0);
        // the stroke geometry is unchanged: the loops are still the leftover chains of `to_paths`
        assert!(n.has_stroke_only_segments());
        // an open chain next to them has no area
        let a = n.add_vertex(v(200.0, 0.0));
        let b = n.add_vertex(v(260.0, 0.0));
        let c = n.add_vertex(v(260.0, 60.0));
        n.connect(a, b);
        n.connect(b, c);
        assert_eq!(n.fill_paths().len(), 2, "the open chain is not filled");
        // once any region exists the regions alone are filled
        let f = n.face_at(v(25.0, 25.0)).unwrap();
        n.toggle_face(&f);
        assert_eq!(n.fill_paths().len(), 1);
    }

    #[test]
    fn splitting_a_segment_keeps_shape_and_updates_regions() {
        let mut n = VectorNetwork::from_paths(&[Path {
            cmds: vec![
                PathCmd::MoveTo(v(0.0, 0.0)),
                PathCmd::CubicTo(v(0.0, 60.0), v(100.0, 60.0), v(100.0, 0.0)),
                PathCmd::LineTo(v(50.0, -40.0)),
                PathCmd::Close,
            ],
            fill_rule: FillRule::NonZero,
        }]);
        let before = area(&n.to_paths());
        let mid = n.point_on(0, 0.4);
        let nv = n.split_segment(0, 0.4);
        assert!(dist(n.pos(nv), mid) < 1e-3);
        assert_eq!(n.segments.len(), 4);
        assert_eq!(n.regions[0].loops[0].len(), 4, "the loop includes the new segment");
        assert!(n.orient_loop(&n.regions[0].loops[0]).is_some());
        assert!((area(&n.to_paths()) - before).abs() < 0.5, "shape preserved");
        assert_eq!(n.vertices[nv].mirror, HandleMirror::Angle);
        // a loop stored in the opposite direction still stays valid
        let mut m = VectorNetwork::from_paths(&[rect_path(0.0, 0.0, 10.0, 10.0)]);
        m.regions[0].loops[0].reverse();
        m.split_segment(1, 0.5);
        assert!(m.orient_loop(&m.regions[0].loops[0]).is_some());
    }

    #[test]
    fn deleting_a_vertex_joins_its_neighbours() {
        let mut n = VectorNetwork::from_paths(&[rect_path(0.0, 0.0, 100.0, 100.0)]);
        n.delete_vertex(1);
        assert_eq!((n.vertices.len(), n.segments.len()), (3, 3));
        assert!((area(&n.to_paths()) - 5000.0).abs() < 1.0, "a triangle remains");
        n.delete_vertex(0);
        assert!(n.regions.is_empty(), "two straight segments enclose nothing");
        let mut t = two_squares();
        t.delete_vertex(1);
        assert_eq!(t.segments.len(), 4, "a junction loses all its segments");
        let mut o = VectorNetwork::from_paths(&[line_path(&[v(0.0, 0.0), v(5.0, 0.0), v(9.0, 3.0)])]);
        o.delete_vertex(2);
        assert_eq!((o.vertices.len(), o.segments.len()), (2, 1), "an endpoint disappears with its segment");
    }

    #[test]
    fn joining_the_ends_of_an_open_path_closes_and_fills_it() {
        let mut n = VectorNetwork::from_paths(&[line_path(&[v(0.0, 0.0), v(100.0, 0.0), v(100.0, 100.0), v(0.5, 100.0), v(0.0, 0.5)])]);
        assert!(n.regions.is_empty());
        let m = n.join_vertices(0, 4).unwrap();
        assert_eq!(n.vertices.len(), 4);
        assert_eq!(n.degree(m), 2);
        assert_eq!(n.regions.len(), 1, "closing creates a filled region");
        assert!((area(&n.to_paths()) - 10000.0).abs() < 100.0);
    }

    #[test]
    fn joining_two_paths_makes_one_continuous_path() {
        let a = line_path(&[v(0.0, 0.0), v(10.0, 0.0)]);
        let b = line_path(&[v(10.5, 0.0), v(20.0, 5.0)]);
        let mut n = VectorNetwork::from_paths(&[a, b]);
        let m = n.join_vertices(1, 2).unwrap();
        assert_eq!(n.degree(m), 2);
        let paths = n.to_paths();
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].cmds.len(), 3, "MoveTo plus two LineTo");
    }

    #[test]
    fn cutting_a_path_at_a_point_separates_it() {
        let mut n = VectorNetwork::from_paths(&[line_path(&[v(0.0, 0.0), v(100.0, 0.0)])]);
        let (a, b) = n.cut_at(0, 0.25).unwrap();
        assert_eq!(n.pos(a), n.pos(b));
        assert_eq!(n.degree(a), 1);
        assert_eq!(n.degree(b), 1);
        assert_eq!(n.to_paths()[0].cmds.iter().filter(|c| matches!(c, PathCmd::MoveTo(_))).count(), 2, "two separate sub-paths");
        let mut c = VectorNetwork::from_paths(&[rect_path(0.0, 0.0, 10.0, 10.0)]);
        c.split_vertex(2).unwrap();
        assert!(c.regions.is_empty(), "cutting a closed loop opens it");
        assert_eq!(c.vertices.len(), 5);
    }

    #[test]
    fn mirroring_follows_the_dragged_handle() {
        let p = Path {
            cmds: vec![
                PathCmd::MoveTo(v(0.0, 50.0)),
                PathCmd::CubicTo(v(20.0, 50.0), v(30.0, 0.0), v(50.0, 0.0)),
                PathCmd::CubicTo(v(70.0, 0.0), v(80.0, 50.0), v(100.0, 50.0)),
            ],
            fill_rule: FillRule::NonZero,
        };
        let mut n = VectorNetwork::from_paths(&[p]);
        assert_eq!(n.vertices[1].mirror, HandleMirror::AngleAndLength, "detected from the handles");
        n.set_tangent(1, End::Start, v(10.0, 10.0));
        n.enforce_mirror(1, End::Start);
        assert_eq!(n.tangent(0, End::End), v(-10.0, -10.0));
        // angle only keeps the other handle's length
        n.set_mirror(1, HandleMirror::Angle);
        n.set_tangent(1, End::Start, v(0.0, 4.0));
        n.enforce_mirror(1, End::Start);
        let t = n.tangent(0, End::End);
        assert!((len(t) - len(v(-10.0, -10.0))).abs() < 1e-3 && t.x.abs() < 1e-4 && t.y < 0.0, "{t:?}");
        // independent handles stay put
        n.set_mirror(1, HandleMirror::None);
        let keep = n.tangent(0, End::End);
        n.set_tangent(1, End::Start, v(7.0, 7.0));
        n.enforce_mirror(1, End::Start);
        assert_eq!(n.tangent(0, End::End), keep);
        n.straighten(1);
        assert_eq!(n.tangent(0, End::End), Vec2::ZERO);
        assert_eq!(n.tangent(1, End::Start), Vec2::ZERO);
    }

    fn seg_dist(p: Vec2, a: Vec2, b: Vec2) -> f32 {
        let ab = sub(b, a);
        let l2 = dot(ab, ab);
        let t = if l2 < 1e-9 { 0.0 } else { (dot(sub(p, a), ab) / l2).clamp(0.0, 1.0) };
        dist(p, add(a, mul(ab, t)))
    }

    #[test]
    fn simplify_reduces_nodes_within_tolerance() {
        let pts: Vec<Vec2> = (0..=120).map(|i| v(i as f32 * 2.0, 30.0 * (i as f32 * 0.1).sin())).collect();
        let mut n = VectorNetwork::from_paths(&[line_path(&pts)]);
        assert_eq!(n.vertices.len(), 121);
        let tol = 0.5;
        let removed = n.simplify(tol);
        assert!(removed > 90, "removed {removed}");
        assert!(n.vertices.len() < 30, "{}", n.vertices.len());
        let poly: Vec<Vec2> = flatten_path(&n.to_paths()[0], 0.02).remove(0).0;
        for p in &pts {
            let d = poly.windows(2).map(|s| seg_dist(*p, s[0], s[1])).fold(f32::MAX, f32::min);
            assert!(d <= tol * 1.5, "deviation {d} at {p:?}");
        }
        assert_eq!(n.pos(0), pts[0], "endpoints are preserved");
        assert!(n.vertices.iter().any(|x| x.pos == pts[120]));
        let mut m = VectorNetwork::from_paths(&[line_path(&pts)]);
        m.simplify(4.0);
        assert!(m.vertices.len() >= 2);
        assert!(m.vertices.len() <= n.vertices.len(), "a looser tolerance keeps fewer nodes: {} vs {}", m.vertices.len(), n.vertices.len());
        // zero-ish tolerance keeps almost everything
        let mut z = VectorNetwork::from_paths(&[line_path(&pts)]);
        z.simplify(0.001);
        assert!(z.vertices.len() > n.vertices.len());
    }

    #[test]
    fn simplify_keeps_junctions_corners_and_regions() {
        let pts: Vec<Vec2> = (0..64)
            .map(|i| {
                let a = i as f32 * std::f32::consts::TAU / 64.0;
                v(100.0 + 50.0 * a.cos(), 100.0 + 50.0 * a.sin())
            })
            .collect();
        let mut p = line_path(&pts);
        p.cmds.push(PathCmd::Close);
        let mut n = VectorNetwork::from_paths(&[p]);
        let a0 = area(&n.to_paths());
        n.simplify(0.3);
        assert!(n.vertices.len() <= 10, "{}", n.vertices.len());
        assert_eq!(n.regions.len(), 1);
        assert!(n.orient_loop(&n.regions[0].loops[0]).is_some());
        assert!((area(&n.to_paths()) - a0).abs() / a0 < 0.01);
        let mut sq = VectorNetwork::from_paths(&[rect_path(0.0, 0.0, 10.0, 10.0)]);
        assert_eq!(sq.simplify(0.5), 0, "a square keeps its corners");
        let mut t = two_squares();
        t.simplify(0.5);
        assert_eq!(t.vertices.len(), 6, "junctions survive");
    }

    #[test]
    fn weld_merges_coincident_vertices() {
        let a = line_path(&[v(0.0, 0.0), v(10.0, 0.0)]);
        let b = line_path(&[v(10.0, 0.0), v(10.0, 10.0)]);
        let n = VectorNetwork::from_paths_welded(&[a, b], 0.01);
        assert_eq!(n.vertices.len(), 3);
        assert_eq!(n.degree(1), 2);
    }

    #[test]
    fn bounds_cover_curves() {
        let n = VectorNetwork::from_paths(&[Path {
            cmds: vec![PathCmd::MoveTo(v(0.0, 0.0)), PathCmd::CubicTo(v(0.0, 60.0), v(100.0, 60.0), v(100.0, 0.0))],
            fill_rule: FillRule::NonZero,
        }]);
        let b = n.bounds();
        assert!(b.h > 40.0 && b.w >= 99.9);
    }

    fn assert_consistent(net: &VectorNetwork, what: &str) {
        assert!(net.is_valid(), "{what}: indices out of range: {net:?}");
        for r in &net.regions {
            for l in &r.loops {
                assert!(net.orient_loop(l).is_some(), "{what}: a region loop is not a closed traversal: {l:?} in {net:?}");
            }
        }
        // nothing derived may panic
        let _ = (net.to_paths(), net.bounds(), net.faces(), net.open_chain_ends());
    }

    #[test]
    fn random_edit_sequences_keep_the_network_consistent() {
        let mut seed = 0x1234_5678_9abc_def0u64;
        let mut rnd = |n: usize| -> usize {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((seed >> 33) as usize) % n.max(1)
        };
        for round in 0..60 {
            let mut net = match round % 3 {
                0 => two_squares(),
                1 => VectorNetwork::from_paths(&[rect_path(0.0, 0.0, 100.0, 100.0), rect_path(30.0, 30.0, 40.0, 40.0)]),
                _ => VectorNetwork::from_paths(&[line_path(&[v(0.0, 0.0), v(40.0, 10.0), v(80.0, 0.0), v(100.0, 50.0)]), rect_path(10.0, 60.0, 50.0, 30.0)]),
            };
            for step in 0..80 {
                let (nv, ns) = (net.vertices.len(), net.segments.len());
                let mut what = format!("round {round} step {step}");
                let op = rnd(10);
                what.push_str(&format!(" op {op} on {nv}v/{ns}s before: {net:?}"));
                match op {
                    0 if ns > 0 => {
                        net.split_segment(rnd(ns), 0.1 + 0.8 * rnd(100) as f32 / 100.0);
                    }
                    1 if nv > 0 => net.delete_vertex(rnd(nv)),
                    2 if nv > 1 => {
                        let (a, b) = (rnd(nv), rnd(nv));
                        net.join_vertices(a, b);
                    }
                    3 if ns > 0 => {
                        net.cut_at(rnd(ns), 0.5);
                    }
                    4 if nv > 0 => {
                        net.split_vertex(rnd(nv));
                    }
                    5 => {
                        net.simplify(0.5 + rnd(4) as f32);
                    }
                    6 if nv > 1 => {
                        net.connect(rnd(nv), rnd(nv));
                    }
                    7 => {
                        if let Some(f) = net.face_at(v(rnd(100) as f32, rnd(100) as f32)) {
                            net.toggle_face(&f);
                        }
                    }
                    8 if ns > 0 => net.delete_segment(rnd(ns)),
                    _ if nv > 0 => {
                        let i = rnd(nv);
                        net.vertices[i].corner_radius = rnd(30) as f32;
                        let ends = net.ends_at(i);
                        if let Some(&(s, e)) = ends.first() {
                            net.set_tangent(s, e, v(rnd(40) as f32 - 20.0, rnd(40) as f32 - 20.0));
                            net.enforce_mirror(s, e);
                        }
                    }
                    _ => {}
                }
                net.compact();
                assert_consistent(&net, &what);
            }
        }
    }

    #[test]
    fn broken_networks_derive_nothing_instead_of_panicking() {
        let mut net = two_squares();
        net.segments[0].b = 99;
        assert!(!net.is_valid());
        assert!(net.to_paths().is_empty() && net.faces().is_empty());
        let mut net = two_squares();
        net.regions.push(Region { rule: FillRule::NonZero, filled: true, loops: vec![vec![42]] });
        assert!(!net.is_valid());
        let mut net = two_squares();
        net.vertices[0].pos = v(f32::NAN, 0.0);
        assert!(!net.is_valid());
        let mut net = VectorNetwork::from_paths(&[rect_path(0.0, 0.0, 10.0, 10.0)]);
        net.vertices[1].corner_radius = f32::NAN;
        assert!(!net.is_valid(), "a NaN radius is not a valid network");
    }

    #[test]
    fn network_serializes_with_defaults() {
        let n = two_squares();
        let s = serde_json::to_string(&n).unwrap();
        let back: VectorNetwork = serde_json::from_str(&s).unwrap();
        assert_eq!(back, n);
        let vx: Vertex = serde_json::from_str(r#"{"pos":{"x":1.0,"y":2.0}}"#).unwrap();
        assert_eq!(vx.corner_radius, 0.0);
        assert_eq!(vx.mirror, HandleMirror::None);
    }

    #[test]
    fn transformed_scales_handles_too() {
        let n = VectorNetwork::from_paths(&[Path {
            cmds: vec![PathCmd::MoveTo(v(0.0, 0.0)), PathCmd::CubicTo(v(0.0, 10.0), v(10.0, 10.0), v(10.0, 0.0))],
            fill_rule: FillRule::NonZero,
        }]);
        let n = n.transformed(&Affine::scale(2.0, 3.0));
        assert_eq!(n.pos(1), v(20.0, 0.0));
        assert_eq!(n.segments[0].ta, v(0.0, 30.0));
    }
}
