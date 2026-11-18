//! A layer copied into a one-page scratch document with its stored outlines dropped, so the CPU renderer draws the
//! live path Studio builds after an edit. Variables and styles are resolved first; effects, opacity and masks are
//! cleared and one opaque paint is forced on the fill or the stroke.

use designos_doc::*;
use designos_render::tiny_skia as sk;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pass {
    Fill,
    Stroke,
}

/// The layer and, for a boolean, its descendants (parents before children), resolved, with the vector paths rebuilt
/// from their networks. `Err` names why the live path cannot be built without the stored outline.
pub fn live_nodes(doc: &Document, id: NodeId) -> Result<Vec<Node>, &'static str> {
    let root = doc.get(id).ok_or("record with no layer")?;
    if matches!(root.kind, NodeKind::Text(_)) {
        return Err("text layer");
    }
    let is_bool = matches!(root.kind, NodeKind::BooleanOp(_));
    let mut out = Vec::new();
    let mut stack = vec![id];
    while let Some(cur) = stack.pop() {
        let n = doc.get(cur).ok_or("record with no layer")?;
        let mut r = resolve_node(doc, n).into_owned();
        if let NodeKind::Vector { paths, network } = &mut r.kind {
            match network {
                Some(net) => *paths = net.to_paths(),
                None => return Err("vector with no network (its live path is the stored outline)"),
            }
        }
        if is_bool {
            stack.extend(n.children.iter().rev().copied());
        } else {
            r.children.clear();
        }
        out.push(r);
    }
    Ok(out)
}

fn sanitise(n: &mut Node, is_root: bool, pass: Pass) {
    n.variables = Default::default();
    n.style_refs.clear();
    n.effects.clear();
    n.opacity = 1.0;
    n.blend = BlendMode::Normal;
    n.is_mask = false;
    n.visible = true;
    n.fill_geometry = None;
    n.drop_stroke_outline();
    n.motion = None;
    if is_root {
        n.transform = Affine::IDENTITY;
        n.component = Default::default();
        if let NodeKind::Instance { frame, .. } | NodeKind::Component(frame) | NodeKind::ComponentSet(frame) = &n.kind {
            n.kind = NodeKind::Frame(frame.clone());
        }
        let solid = Paint::solid(Color::BLACK);
        match pass {
            Pass::Fill => {
                n.fills = vec![solid];
                n.stroke.paints.clear();
            }
            Pass::Stroke => {
                n.fills.clear();
                n.stroke.paints = vec![solid];
            }
        }
    }
}

/// Coverage of Studio's live outline in a `w` x `h` raster, `ts` mapping layer space to pixels.
pub fn studio_coverage(nodes: &[Node], pass: Pass, w: u32, h: u32, ts: sk::Transform) -> Option<sk::Pixmap> {
    let mut doc = Document::new("scratch");
    let page = doc.add_page("scratch");
    let mut ids = std::collections::HashMap::new();
    let mut next = 1_000_000_000u64;
    for (i, n) in nodes.iter().enumerate() {
        let mut c = n.clone();
        let parent = if i == 0 { page } else { *ids.get(&n.parent?)? };
        let new = NodeId(next);
        next += 1;
        ids.insert(n.id, new);
        c.id = new;
        c.children.clear();
        sanitise(&mut c, i == 0, pass);
        doc.insert(parent, c).ok()?;
    }
    let mut pm = sk::Pixmap::new(w, h)?;
    designos_render::render_node_into(&doc, NodeId(1_000_000_000), ts, ts, &mut pm);
    Some(pm)
}

/// Runs `f(0..n)` on every core but four, results in index order.
pub fn run_parallel<T: Send>(n: usize, f: impl Fn(usize) -> T + Sync) -> Vec<T> {
    let threads = std::thread::available_parallelism().map_or(4, |c| c.get()).saturating_sub(4).max(1);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let mut out: Vec<(usize, T)> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if i >= n {
                            break mine;
                        }
                        mine.push((i, f(i)));
                    }
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().expect("worker")).collect()
    });
    out.sort_by_key(|(i, _)| *i);
    out.into_iter().map(|(_, t)| t).collect()
}
