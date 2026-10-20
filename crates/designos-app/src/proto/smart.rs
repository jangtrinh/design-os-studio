//! Smart Animate (best effort): pairs layers of two frames by name and
//! path, then interpolates transform, size, opacity, corner radius, fills and
//! strokes. Layers without a partner fade in or out. If nothing pairs up the
//! caller falls back to a dissolve.

use designos_doc::*;
use std::collections::{HashMap, HashSet};

/// Copy of `root` and its descendants (ids kept) as the only frame of a fresh
/// document, so it can be rendered or morphed without touching the original.
/// Returns `None` when the highest id in `doc` is `u64::MAX`, leaving no id for the snapshot page.
pub fn subtree_doc(doc: &Document, root: NodeId) -> Option<Document> {
    let mut out = Document::new("snapshot");
    out.images = doc.images.clone();
    let max = doc.nodes.keys().next_back().map(|k| k.0).unwrap_or(0);
    let next = max.checked_add(1)?;
    let pid = NodeId(next);
    let mut page = Node::new(pid, NodeKind::Page { background: Color::TRANSPARENT });
    page.children = vec![root];
    out.nodes.insert(pid, page);
    out.pages.push(pid);
    out.reserve_ids(pid);
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let Some(n) = doc.get(id) else { continue };
        let mut c = n.clone();
        if id == root {
            c.parent = Some(pid);
        }
        stack.extend(n.children.iter().copied());
        out.nodes.insert(id, c);
    }
    out.reserve_ids(NodeId(next));
    Some(out)
}

fn descendants(doc: &Document, root: NodeId) -> Vec<(String, NodeId)> {
    // (path of names from the root, id); names joined with a separator no layer name uses
    let mut out = Vec::new();
    fn rec(doc: &Document, id: NodeId, path: &str, out: &mut Vec<(String, NodeId)>) {
        let Some(n) = doc.get(id) else { return };
        for c in &n.children {
            let Some(k) = doc.get(*c) else { continue };
            let p = format!("{path}\u{1}{}", k.name);
            out.push((p.clone(), *c));
            rec(doc, *c, &p, out);
        }
    }
    rec(doc, root, "", &mut out);
    out
}

/// Pairs `(to, from)` of layers considered the same.
pub fn match_layers(from_doc: &Document, from_root: NodeId, to_doc: &Document, to_root: NodeId) -> Vec<(NodeId, NodeId)> {
    let fd = descendants(from_doc, from_root);
    let td = descendants(to_doc, to_root);
    let kind_eq = |a: NodeId, b: NodeId| match (from_doc.get(a), to_doc.get(b)) {
        (Some(x), Some(y)) => std::mem::discriminant(&x.kind) == std::mem::discriminant(&y.kind),
        _ => false,
    };
    // pass 1: identical path; the n-th sibling with a repeated name pairs with the n-th
    let mut by_path: HashMap<(String, usize), NodeId> = HashMap::new();
    let mut seen: HashMap<String, usize> = HashMap::new();
    for (p, id) in &fd {
        let n = seen.entry(p.clone()).or_insert(0);
        by_path.insert((p.clone(), *n), *id);
        *n += 1;
    }
    let mut pairs = Vec::new();
    let mut used_from: HashSet<NodeId> = HashSet::new();
    let mut matched_to: HashSet<NodeId> = HashSet::new();
    let mut seen: HashMap<String, usize> = HashMap::new();
    for (p, id) in &td {
        let n = seen.entry(p.clone()).or_insert(0);
        if let Some(f) = by_path.get(&(p.clone(), *n)).copied().filter(|f| kind_eq(*f, *id)) {
            pairs.push((*id, f));
            used_from.insert(f);
            matched_to.insert(*id);
        }
        *n += 1;
    }
    // pass 2: layers that moved to another parent pair by a name that is unique among the leftovers
    let name = |doc: &Document, id: NodeId| doc.get(id).map(|n| n.name.clone()).unwrap_or_default();
    let mut left_from: HashMap<String, Vec<NodeId>> = HashMap::new();
    for (_, id) in fd.iter().filter(|(_, id)| !used_from.contains(id)) {
        left_from.entry(name(from_doc, *id)).or_default().push(*id);
    }
    let mut left_to: HashMap<String, Vec<NodeId>> = HashMap::new();
    for (_, id) in td.iter().filter(|(_, id)| !matched_to.contains(id)) {
        left_to.entry(name(to_doc, *id)).or_default().push(*id);
    }
    for (nm, tos) in &left_to {
        if let (Some(froms), [t]) = (left_from.get(nm), tos.as_slice()) {
            if let [f] = froms.as_slice() {
                if kind_eq(*f, *t) {
                    pairs.push((*t, *f));
                }
            }
        }
    }
    pairs
}

struct Pair {
    id: NodeId,
    from: Node,
    to: Node,
}

struct Fade {
    id: NodeId,
    opacity: f32,
    fade_in: bool,
}

pub struct SmartPlan {
    base: Document,
    root: NodeId,
    pairs: Vec<Pair>,
    fades: Vec<Fade>,
}

impl SmartPlan {
    /// None when no layer pairs up or when node ids are exhausted (nothing to morph: use a dissolve).
    pub fn build(from_doc: &Document, from_root: NodeId, to_doc: &Document, to_root: NodeId) -> Option<SmartPlan> {
        from_doc.nodes.keys().next_back().map(|k| k.0).unwrap_or(0).checked_add(1)?;
        let matched = match_layers(from_doc, from_root, to_doc, to_root);
        if matched.is_empty() {
            return None;
        }
        let mut base = subtree_doc(to_doc, to_root)?;
        let mut pairs = vec![Pair { id: to_root, from: from_doc.get(from_root)?.clone(), to: to_doc.get(to_root)?.clone() }];
        let to_of: HashMap<NodeId, NodeId> = matched.iter().map(|(t, f)| (*f, *t)).collect();
        let is_matched_to: HashSet<NodeId> = matched.iter().map(|(t, _)| *t).collect();
        for (t, f) in &matched {
            if let (Some(fnode), Some(tnode)) = (from_doc.get(*f), to_doc.get(*t)) {
                pairs.push(Pair { id: *t, from: fnode.clone(), to: tnode.clone() });
            }
        }
        let mut fades = Vec::new();
        // layers only in the destination fade in (outermost ones; children ride along)
        for (_, id) in descendants(to_doc, to_root) {
            if is_matched_to.contains(&id) {
                continue;
            }
            let parent_unmatched = to_doc.get(id).and_then(|n| n.parent).is_some_and(|p| p != to_root && !is_matched_to.contains(&p));
            if !parent_unmatched {
                fades.push(Fade { id, opacity: to_doc.get(id).map(|n| n.opacity).unwrap_or(1.0), fade_in: true });
            }
        }
        // layers only in the source are copied in and fade out
        let from_matched: HashSet<NodeId> = matched.iter().map(|(_, f)| *f).collect();
        for (_, id) in descendants(from_doc, from_root) {
            if from_matched.contains(&id) {
                continue;
            }
            let Some(fnode) = from_doc.get(id) else { continue };
            let parent = fnode.parent.unwrap_or(from_root);
            if parent != from_root && !from_matched.contains(&parent) {
                continue;
            }
            let attach = if parent == from_root { to_root } else { *to_of.get(&parent).unwrap_or(&to_root) };
            let attach_is_root_fallback = parent != from_root && !to_of.contains_key(&parent);
            let nid = clone_into(
                from_doc,
                id,
                &mut base,
                attach,
                attach_is_root_fallback.then(|| from_doc.world_transform(parent).invert().map(|i| i * from_doc.world_transform(id))).flatten(),
            )?;
            fades.push(Fade { id: nid, opacity: fnode.opacity, fade_in: false });
        }
        Some(SmartPlan { base, root: to_root, pairs, fades })
    }

    /// The morphed frame at eased progress `t` (0 = source look, 1 = destination look).
    pub fn apply(&mut self, t: f32) -> (&Document, NodeId) {
        for p in &self.pairs {
            let Some(n) = self.base.get_mut(p.id) else { continue };
            let (a, b) = (&p.from, &p.to);
            if p.id != self.root {
                n.transform = lerp_affine(&a.transform, &b.transform, t);
            }
            n.size = Vec2::new(lerp(a.size.x, b.size.x, t), lerp(a.size.y, b.size.y, t));
            n.opacity = lerp(a.opacity, b.opacity, t).clamp(0.0, 1.0);
            for i in 0..4 {
                n.corner_radii[i] = lerp(a.corner_radii[i], b.corner_radii[i], t).max(0.0);
            }
            n.fills = lerp_paints(&a.fills, &b.fills, t);
            n.stroke.paints = lerp_paints(&a.stroke.paints, &b.stroke.paints, t);
            n.stroke.weight = lerp(a.stroke.weight, b.stroke.weight, t).max(0.0);
            if let (NodeKind::Text(x), NodeKind::Text(y), NodeKind::Text(m)) = (&a.kind, &b.kind, &mut n.kind) {
                m.style.font_size = lerp(x.style.font_size, y.style.font_size, t).max(1.0);
                m.style.fills = lerp_paints(&x.style.fills, &y.style.fills, t);
            }
        }
        for f in &self.fades {
            if let Some(n) = self.base.get_mut(f.id) {
                let k = if f.fade_in { t } else { 1.0 - t };
                n.opacity = (f.opacity * k).clamp(0.0, 1.0);
            }
        }
        (&self.base, self.root)
    }

    #[cfg(test)]
    pub fn paired(&self) -> usize {
        self.pairs.len().saturating_sub(1)
    }
}

/// Copies `src` and its subtree into `dst` under `parent` with fresh ids.
fn clone_into(src_doc: &Document, src: NodeId, dst: &mut Document, parent: NodeId, retransform: Option<Affine>) -> Option<NodeId> {
    fn rec(src_doc: &Document, src: NodeId, dst: &mut Document, parent: NodeId) -> Option<NodeId> {
        dst.peek_next_id().0.checked_add(1)?;
        let mut n = src_doc.get(src)?.clone();
        let id = dst.alloc_id();
        let kids = std::mem::take(&mut n.children);
        n.id = id;
        n.parent = Some(parent);
        dst.nodes.insert(id, n);
        if let Some(p) = dst.get_mut(parent) {
            p.children.push(id);
        }
        for k in kids {
            rec(src_doc, k, dst, id)?;
        }
        Some(id)
    }
    let id = rec(src_doc, src, dst, parent)?;
    if let Some(t) = retransform {
        if let Some(n) = dst.get_mut(id) {
            n.transform = t;
        }
    }
    Some(id)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let c = |x: f32| x.clamp(0.0, 1.0);
    Color { r: c(lerp(a.r, b.r, t)), g: c(lerp(a.g, b.g, t)), b: c(lerp(a.b, b.b, t)), a: c(lerp(a.a, b.a, t)) }
}

/// Interpolates translation, rotation and scale (skew is dropped).
pub fn lerp_affine(a: &Affine, b: &Affine, t: f32) -> Affine {
    let dec = |m: &Affine| {
        let sx = m.a.hypot(m.b);
        let det = m.a * m.d - m.b * m.c;
        let sy = if sx > 1e-9 { det / sx } else { 0.0 };
        (m.tx, m.ty, m.b.atan2(m.a), sx, sy)
    };
    let (ax, ay, ar, asx, asy) = dec(a);
    let (bx, by, br, bsx, bsy) = dec(b);
    let mut dr = br - ar;
    while dr > std::f32::consts::PI {
        dr -= std::f32::consts::TAU;
    }
    while dr < -std::f32::consts::PI {
        dr += std::f32::consts::TAU;
    }
    let (r, sx, sy) = (ar + dr * t, lerp(asx, bsx, t), lerp(asy, bsy, t));
    let (s, c) = r.sin_cos();
    Affine { a: sx * c, b: sx * s, c: -sy * s, d: sy * c, tx: lerp(ax, bx, t), ty: lerp(ay, by, t) }
}

fn lerp_paints(a: &[Paint], b: &[Paint], t: f32) -> Vec<Paint> {
    if a.is_empty() && b.is_empty() {
        return Vec::new();
    }
    if a.is_empty() {
        return b.iter().map(|p| Paint { opacity: p.opacity * t.clamp(0.0, 1.0), ..p.clone() }).collect();
    }
    if b.is_empty() {
        return a.iter().map(|p| Paint { opacity: p.opacity * (1.0 - t).clamp(0.0, 1.0), ..p.clone() }).collect();
    }
    if a.len() == b.len() {
        let mut out = Vec::new();
        for (x, y) in a.iter().zip(b) {
            match (&x.kind, &y.kind) {
                (PaintKind::Solid(c1), PaintKind::Solid(c2)) => {
                    out.push(Paint {
                        kind: PaintKind::Solid(lerp_color(*c1, *c2, t)),
                        opacity: lerp(x.opacity, y.opacity, t).clamp(0.0, 1.0),
                        visible: y.visible,
                        blend: y.blend,
                    });
                }
                (PaintKind::LinearGradient { handles: h1, stops: s1 }, PaintKind::LinearGradient { handles: h2, stops: s2 }) if s1.len() == s2.len() => {
                    let handles = [0, 1, 2].map(|i| Vec2::new(lerp(h1[i].x, h2[i].x, t), lerp(h1[i].y, h2[i].y, t)));
                    let stops = s1.iter().zip(s2).map(|(p, q)| GradientStop { pos: lerp(p.pos, q.pos, t), color: lerp_color(p.color, q.color, t) }).collect();
                    out.push(Paint {
                        kind: PaintKind::LinearGradient { handles, stops },
                        opacity: lerp(x.opacity, y.opacity, t).clamp(0.0, 1.0),
                        visible: y.visible,
                        blend: y.blend,
                    });
                }
                _ => out.push(if t < 0.5 { x.clone() } else { y.clone() }),
            }
        }
        return out;
    }
    if t < 0.5 {
        a.to_vec()
    } else {
        b.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_frames() -> (Document, NodeId, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let mut ids = Vec::new();
        for (i, name) in ["A", "B"].iter().enumerate() {
            let id = d.alloc_id();
            let mut f = Node::frame(id, Rect::new(i as f32 * 500.0, 0.0, 400.0, 800.0));
            f.name = (*name).into();
            ids.push(d.insert(page, f).unwrap());
        }
        (d, ids[0], ids[1])
    }

    fn rect(d: &mut Document, parent: NodeId, name: &str, r: Rect, color: Color) -> NodeId {
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, r);
        n.name = name.into();
        n.fills = vec![Paint::solid(color)];
        d.insert(parent, n).unwrap()
    }

    #[test]
    fn layers_pair_by_path_and_kind() {
        let (mut d, a, b) = two_frames();
        let fa = rect(&mut d, a, "Card", Rect::new(0.0, 0.0, 10.0, 10.0), Color::BLACK);
        let fb = rect(&mut d, b, "Card", Rect::new(50.0, 50.0, 20.0, 20.0), Color::WHITE);
        let _only_a = rect(&mut d, a, "Gone", Rect::new(0.0, 0.0, 5.0, 5.0), Color::BLACK);
        let _only_b = rect(&mut d, b, "New", Rect::new(0.0, 0.0, 5.0, 5.0), Color::BLACK);
        // same name, different kind: no pair
        let ida = d.alloc_id();
        let mut e = Node::ellipse(ida, Rect::new(0.0, 0.0, 5.0, 5.0));
        e.name = "Dot".into();
        d.insert(a, e).unwrap();
        rect(&mut d, b, "Dot", Rect::new(0.0, 0.0, 5.0, 5.0), Color::BLACK);
        let m = match_layers(&d, a, &d, b);
        assert_eq!(m, vec![(fb, fa)]);
    }

    #[test]
    fn repeated_names_pair_in_order() {
        let (mut d, a, b) = two_frames();
        let a1 = rect(&mut d, a, "Item", Rect::new(0.0, 0.0, 1.0, 1.0), Color::BLACK);
        let a2 = rect(&mut d, a, "Item", Rect::new(0.0, 10.0, 1.0, 1.0), Color::BLACK);
        let b1 = rect(&mut d, b, "Item", Rect::new(0.0, 0.0, 1.0, 1.0), Color::BLACK);
        let b2 = rect(&mut d, b, "Item", Rect::new(0.0, 10.0, 1.0, 1.0), Color::BLACK);
        let m = match_layers(&d, a, &d, b);
        assert!(m.contains(&(b1, a1)) && m.contains(&(b2, a2)));
    }

    #[test]
    fn reparented_unique_names_still_pair() {
        let (mut d, a, b) = two_frames();
        let fa = rect(&mut d, a, "Avatar", Rect::new(0.0, 0.0, 1.0, 1.0), Color::BLACK);
        let gid = d.alloc_id();
        let g = d.insert(b, Node::frame(gid, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
        let fb = rect(&mut d, g, "Avatar", Rect::new(0.0, 0.0, 1.0, 1.0), Color::BLACK);
        assert_eq!(match_layers(&d, a, &d, b), vec![(fb, fa)]);
    }

    #[test]
    fn nothing_in_common_means_no_plan() {
        let (mut d, a, b) = two_frames();
        rect(&mut d, a, "X", Rect::new(0.0, 0.0, 1.0, 1.0), Color::BLACK);
        rect(&mut d, b, "Y", Rect::new(0.0, 0.0, 1.0, 1.0), Color::BLACK);
        assert!(SmartPlan::build(&d, a, &d, b).is_none());
    }

    #[test]
    fn interpolates_geometry_opacity_and_color() {
        let (mut d, a, b) = two_frames();
        rect(&mut d, a, "Card", Rect::new(0.0, 0.0, 100.0, 40.0), Color::rgb(0, 0, 0));
        let fb = rect(&mut d, b, "Card", Rect::new(100.0, 60.0, 200.0, 80.0), Color::rgb(255, 255, 255));
        d.get_mut(fb).unwrap().opacity = 0.5;
        let mut plan = SmartPlan::build(&d, a, &d, b).unwrap();
        assert_eq!(plan.paired(), 1);
        let (doc, root) = plan.apply(0.5);
        let card = doc.get(doc.get(root).unwrap().children[0]).unwrap();
        assert_eq!(card.transform.translation(), Vec2::new(50.0, 30.0));
        assert_eq!(card.size, Vec2::new(150.0, 60.0));
        assert!((card.opacity - 0.75).abs() < 1e-5);
        assert!(matches!(card.fills[0].kind, PaintKind::Solid(c) if (c.r - 0.5).abs() < 1e-4));
        // endpoints reproduce the two looks
        let (doc, root) = plan.apply(0.0);
        let card = doc.get(doc.get(root).unwrap().children[0]).unwrap();
        assert_eq!(card.size, Vec2::new(100.0, 40.0));
        let (doc, root) = plan.apply(1.0);
        let card = doc.get(doc.get(root).unwrap().children[0]).unwrap();
        assert_eq!(card.size, Vec2::new(200.0, 80.0));
        assert!((card.opacity - 0.5).abs() < 1e-5);
    }

    #[test]
    fn unmatched_layers_cross_fade() {
        let (mut d, a, b) = two_frames();
        rect(&mut d, a, "Keep", Rect::new(0.0, 0.0, 10.0, 10.0), Color::BLACK);
        rect(&mut d, b, "Keep", Rect::new(0.0, 0.0, 10.0, 10.0), Color::BLACK);
        rect(&mut d, a, "Gone", Rect::new(5.0, 5.0, 10.0, 10.0), Color::BLACK);
        rect(&mut d, b, "New", Rect::new(7.0, 7.0, 10.0, 10.0), Color::BLACK);
        let mut plan = SmartPlan::build(&d, a, &d, b).unwrap();
        let (doc, root) = plan.apply(0.25);
        let kids: Vec<&Node> = doc.get(root).unwrap().children.iter().map(|c| doc.get(*c).unwrap()).collect();
        let by = |n: &str| kids.iter().find(|k| k.name == n).unwrap();
        assert!((by("New").opacity - 0.25).abs() < 1e-5, "fades in");
        assert!((by("Gone").opacity - 0.75).abs() < 1e-5, "fades out");
        assert_eq!(by("Gone").transform.translation(), Vec2::new(5.0, 5.0), "keeps its place");
        assert_eq!(by("Keep").opacity, 1.0);
    }

    #[test]
    fn works_across_documents_for_change_to() {
        let (mut d, a, _b) = two_frames();
        rect(&mut d, a, "Label", Rect::new(0.0, 0.0, 10.0, 10.0), Color::BLACK);
        let before = subtree_doc(&d, a).unwrap();
        d.get_mut(d.get(a).unwrap().children[0]).unwrap().size = Vec2::new(30.0, 30.0);
        let mut plan = SmartPlan::build(&before, a, &d, a).unwrap();
        let (doc, root) = plan.apply(0.5);
        assert_eq!(doc.get(doc.get(root).unwrap().children[0]).unwrap().size, Vec2::new(20.0, 20.0));
    }

    #[test]
    fn subtree_doc_is_renderable_and_isolated() {
        let (mut d, a, _b) = two_frames();
        rect(&mut d, a, "R", Rect::new(0.0, 0.0, 50.0, 50.0), Color::rgb(255, 0, 0));
        let s = subtree_doc(&d, a).unwrap();
        assert_eq!(s.nodes.len(), 3, "page + frame + child");
        let pm = designos_render::render_node(&s, a, 0.1).unwrap();
        assert_eq!((pm.width(), pm.height()), (40, 80));
        // original untouched
        assert!(d.get(a).unwrap().parent.is_some());
        assert_eq!(s.get(a).unwrap().parent, Some(s.pages[0]));
    }

    #[test]
    fn max_id_overflow_in_smart_animate_uses_the_dissolve_fallback() {
        let (mut d, a, b) = two_frames();
        let mut overflowing = Node::rectangle(NodeId(u64::MAX), Rect::new(0.0, 0.0, 10.0, 10.0));
        overflowing.name = "MaxChild".into();
        d.insert(a, overflowing).unwrap();

        assert!(subtree_doc(&d, a).is_none(), "subtree_doc must return None rather than wrap on u64::MAX");
        assert!(SmartPlan::build(&d, a, &d, b).is_none(), "SmartPlan::build must return None (dissolve fallback) when node ids reach u64::MAX");
    }

    #[test]
    fn affine_lerp_interpolates_rotation_by_angle() {
        let a = Affine::translate(0.0, 0.0);
        let b = Affine::translate(10.0, 0.0) * Affine::rotate(std::f32::consts::FRAC_PI_2);
        let m = lerp_affine(&a, &b, 0.5);
        assert!((m.rotation_rad() - std::f32::consts::FRAC_PI_4).abs() < 1e-4);
        assert!((m.a.hypot(m.b) - 1.0).abs() < 1e-4, "no shrinking mid-rotation");
        assert!((m.tx - 5.0).abs() < 1e-4);
        // the long way round is avoided
        let c = Affine::rotate(3.0);
        let d = Affine::rotate(-3.0);
        let mid = lerp_affine(&c, &d, 0.5);
        assert!(mid.rotation_rad().abs() > 3.0 || (mid.rotation_rad().abs() - std::f32::consts::PI).abs() < 0.2);
    }

    #[test]
    fn paint_lerp_handles_added_and_removed_fills() {
        let solid = vec![Paint::solid(Color::BLACK)];
        let faded_in = lerp_paints(&[], &solid, 0.25);
        assert!((faded_in[0].opacity - 0.25).abs() < 1e-6);
        let faded_out = lerp_paints(&solid, &[], 0.25);
        assert!((faded_out[0].opacity - 0.75).abs() < 1e-6);
        assert!(lerp_paints(&[], &[], 0.5).is_empty());
    }
}
