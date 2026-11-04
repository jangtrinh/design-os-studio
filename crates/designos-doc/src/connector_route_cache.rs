//! Thread-local, value-fingerprinted cache for [`crate::connector_route`] and the
//! synthetic whiteboard performance fixture.

use crate::board::Anchor;
use crate::{connector_anchors, route, ConnectorLine, Document, Magnet, NodeId, NodeKind, Rect, Route};
#[cfg(any(test, feature = "fixtures"))]
use crate::{sync_connector, ConnectorEnd, ConnectorProps, Node, ShapeKind};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};

/// Maximum number of connector route entries retained per thread.
pub const ROUTE_CACHE_CAP: usize = 8_192;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct AnchorBits {
    pos: [u32; 2],
    dir: [u32; 2],
    rect: Option<[u32; 4]>,
}

impl AnchorBits {
    #[inline]
    fn from_anchor(a: &Anchor) -> Self {
        Self {
            pos: [a.pos.x.to_bits(), a.pos.y.to_bits()],
            dir: [a.dir.x.to_bits(), a.dir.y.to_bits()],
            rect: a.rect.map(|r| [r.x.to_bits(), r.y.to_bits(), r.w.to_bits(), r.h.to_bits()]),
        }
    }
}

#[inline]
fn magnet_tag(m: Magnet) -> u8 {
    match m {
        Magnet::Auto => 0,
        Magnet::Top => 1,
        Magnet::Right => 2,
        Magnet::Bottom => 3,
        Magnet::Left => 4,
        Magnet::Center => 5,
    }
}

#[inline]
fn line_tag(line: ConnectorLine) -> u8 {
    match line {
        ConnectorLine::Elbowed => 0,
        ConnectorLine::Straight => 1,
        ConnectorLine::Curved => 2,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct RouteFingerprint {
    start_node: Option<NodeId>,
    end_node: Option<NodeId>,
    start_magnet: u8,
    end_magnet: u8,
    line: u8,
    start: AnchorBits,
    end: AnchorBits,
}

struct CachedRoute {
    fp: RouteFingerprint,
    bounds: Rect,
    route: Route,
}

#[derive(Default)]
struct RouteCache {
    entries: HashMap<NodeId, CachedRoute>,
    order: VecDeque<NodeId>,
    #[cfg_attr(not(any(test, feature = "fixtures")), allow(dead_code))]
    hits: u64,
    #[cfg_attr(not(any(test, feature = "fixtures")), allow(dead_code))]
    misses: u64,
}

impl RouteCache {
    fn insert_computed(&mut self, id: NodeId, fp: RouteFingerprint, computed: Route) -> (Rect, Route) {
        let bounds = computed.bounds();
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.fp = fp;
            entry.bounds = bounds;
            entry.route = computed.clone();
        } else {
            while self.entries.len() >= ROUTE_CACHE_CAP {
                if let Some(oldest) = self.order.pop_front() {
                    self.entries.remove(&oldest);
                } else {
                    self.entries.clear();
                    break;
                }
            }
            self.entries.insert(id, CachedRoute { fp, bounds, route: computed.clone() });
            self.order.push_back(id);
        }
        (bounds, computed)
    }

    fn get_or_compute(&mut self, id: NodeId, fp: RouteFingerprint, compute: impl FnOnce() -> Route) -> Route {
        if let Some(entry) = self.entries.get(&id) {
            if entry.fp == fp {
                self.hits += 1;
                return entry.route.clone();
            }
        }
        self.misses += 1;
        let computed = compute();
        self.insert_computed(id, fp, computed).1
    }

    fn get_or_compute_bounds(&mut self, id: NodeId, fp: RouteFingerprint, compute: impl FnOnce() -> Route) -> Rect {
        if let Some(entry) = self.entries.get(&id) {
            if entry.fp == fp {
                return entry.bounds;
            }
        }
        self.misses += 1;
        let computed = compute();
        self.insert_computed(id, fp, computed).0
    }

    #[cfg(any(test, feature = "fixtures"))]
    fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.hits = 0;
        self.misses = 0;
    }
}

thread_local! {
    static ROUTE_CACHE: RefCell<RouteCache> = RefCell::new(RouteCache::default());
}

/// Computes or retrieves the cached route of connector `id` in its parent's coordinate space.
pub fn cached_connector_route(doc: &Document, id: NodeId) -> Option<Route> {
    let n = doc.get(id)?;
    let NodeKind::Connector(c) = &n.kind else { return None };
    let (a, b) = connector_anchors(doc, id)?;
    let fp = RouteFingerprint {
        start_node: c.start.node,
        end_node: c.end.node,
        start_magnet: magnet_tag(c.start.magnet),
        end_magnet: magnet_tag(c.end.magnet),
        line: line_tag(c.line),
        start: AnchorBits::from_anchor(&a),
        end: AnchorBits::from_anchor(&b),
    };
    let line = c.line;
    Some(ROUTE_CACHE.with(|cache| cache.borrow_mut().get_or_compute(id, fp, || route(line, &a, &b))))
}

/// Returns the bounding box of the current route of connector `id` in its parent's coordinate
/// space, padded by `margin`, validated by the anchor fingerprint in the route cache entry.
pub fn connector_route_bounds(doc: &Document, id: NodeId, margin: f32) -> Option<Rect> {
    let n = doc.get(id)?;
    let NodeKind::Connector(c) = &n.kind else { return None };
    let (a, b) = connector_anchors(doc, id)?;
    let fp = RouteFingerprint {
        start_node: c.start.node,
        end_node: c.end.node,
        start_magnet: magnet_tag(c.start.magnet),
        end_magnet: magnet_tag(c.end.magnet),
        line: line_tag(c.line),
        start: AnchorBits::from_anchor(&a),
        end: AnchorBits::from_anchor(&b),
    };
    let line = c.line;
    let b = ROUTE_CACHE.with(|cache| cache.borrow_mut().get_or_compute_bounds(id, fp, || route(line, &a, &b)));
    Some(Rect::new(b.x - margin, b.y - margin, b.w + 2.0 * margin, b.h + 2.0 * margin))
}

/// Returns `(hits, misses)` recorded on the current thread's connector route cache.
#[cfg(any(test, feature = "fixtures"))]
pub fn route_cache_stats() -> (u64, u64) {
    ROUTE_CACHE.with(|cache| {
        let c = cache.borrow();
        (c.hits, c.misses)
    })
}

/// Returns the number of entries currently stored in the thread-local connector route cache.
#[cfg(any(test, feature = "fixtures"))]
pub fn route_cache_len() -> usize {
    ROUTE_CACHE.with(|cache| cache.borrow().entries.len())
}

/// Clears all entries and resets hit/miss counters on the current thread's connector route cache.
#[cfg(any(test, feature = "fixtures"))]
pub fn clear_route_cache() {
    ROUTE_CACHE.with(|cache| cache.borrow_mut().clear());
}

/// Synthetic benchmark scene with 200 frames, 2,000 whiteboard items, and 1,500 connectors.
#[cfg(any(test, feature = "fixtures"))]
#[derive(Clone, Debug)]
pub struct PerfWhiteboardScene {
    pub doc: Document,
    pub page: NodeId,
    pub frames: Vec<NodeId>,
    pub items: Vec<NodeId>,
    pub connectors: Vec<NodeId>,
    pub page_item_connectors: Vec<NodeId>,
    pub frame_connectors: Vec<NodeId>,
    pub inner_connectors: Vec<NodeId>,
    pub hub_50: NodeId,
    pub hub_150: NodeId,
}

/// Builds the `perf_whiteboard_2000` benchmark scene:
/// - 200 frames of 390 x 844 on the active page
/// - 2,000 whiteboard items on the active page (2,200 non-connector top-level nodes total)
/// - 1,500 connectors:
///   - 1,000 item-to-item connectors at page level (including one hub with 50 and one with 150)
///   - 300 frame-to-frame connectors at page level
///   - 200 connectors inside frames (1 per frame between two internal layers)
#[cfg(any(test, feature = "fixtures"))]
pub fn perf_whiteboard_2000() -> PerfWhiteboardScene {
    let mut doc = Document::new("Perf Whiteboard 2000");
    let page = doc.add_page("Canvas");

    let mut frames = Vec::with_capacity(200);
    let mut inner_targets = Vec::with_capacity(200);
    for idx in 0..200usize {
        let col = (idx % 20) as f32;
        let row = (idx / 20) as f32;
        let fx = col * 560.0;
        let fy = row * 1040.0;
        let fid = doc.alloc_id();
        let mut frame = Node::frame(fid, Rect::new(fx, fy, 390.0, 844.0));
        frame.name = format!("Screen {idx}");
        doc.insert(page, frame).unwrap();
        frames.push(fid);

        let ca_id = doc.alloc_id();
        let ca = Node::rectangle(ca_id, Rect::new(30.0, 60.0, 140.0, 80.0));
        doc.insert(fid, ca).unwrap();

        let cb_id = doc.alloc_id();
        let cb = Node::rectangle(cb_id, Rect::new(210.0, 640.0, 140.0, 80.0));
        doc.insert(fid, cb).unwrap();
        inner_targets.push((fid, ca_id, cb_id));
    }

    let mut items = Vec::with_capacity(2_000);
    for idx in 0..2_000usize {
        let col = (idx % 50) as f32;
        let row = (idx / 50) as f32;
        let wx = col * 240.0;
        let wy = 11_000.0 + row * 220.0;
        let id = doc.alloc_id();
        let node = if idx % 2 == 0 {
            Node::sticky(id, Rect::new(wx, wy, 140.0, 120.0), format!("Note {idx}"))
        } else {
            Node::shape_with_text(id, Rect::new(wx, wy, 140.0, 100.0), ShapeKind::RoundedRect, format!("Step {idx}"))
        };
        doc.insert(page, node).unwrap();
        items.push(id);
    }

    let hub_50 = items[0];
    let hub_150 = items[1];

    let mut connectors = Vec::with_capacity(1_500);
    let mut page_item_connectors = Vec::with_capacity(1_000);
    let mut frame_connectors = Vec::with_capacity(300);
    let mut inner_connectors = Vec::with_capacity(200);

    // 1) 50 connectors on hub_50
    for k in 0..50usize {
        let cid = doc.alloc_id();
        let props = ConnectorProps {
            start: ConnectorEnd::attached(hub_50, Magnet::Auto),
            end: ConnectorEnd::attached(items[2 + k], Magnet::Auto),
            ..Default::default()
        };
        doc.insert(page, Node::connector(cid, props)).unwrap();
        sync_connector(&mut doc, cid);
        page_item_connectors.push(cid);
        connectors.push(cid);
    }

    // 2) 150 connectors on hub_150
    for k in 0..150usize {
        let cid = doc.alloc_id();
        let props = ConnectorProps {
            start: ConnectorEnd::attached(hub_150, Magnet::Auto),
            end: ConnectorEnd::attached(items[52 + k], Magnet::Auto),
            ..Default::default()
        };
        doc.insert(page, Node::connector(cid, props)).unwrap();
        sync_connector(&mut doc, cid);
        page_item_connectors.push(cid);
        connectors.push(cid);
    }

    // 3) Remaining 800 item-to-item connectors at page level (disjoint from hub_50 and hub_150)
    for k in 0..800usize {
        let a = items[202 + (k * 2) % 1_796];
        let b = items[202 + (k * 2 + 1) % 1_796];
        let cid = doc.alloc_id();
        let props = ConnectorProps { start: ConnectorEnd::attached(a, Magnet::Auto), end: ConnectorEnd::attached(b, Magnet::Auto), ..Default::default() };
        doc.insert(page, Node::connector(cid, props)).unwrap();
        sync_connector(&mut doc, cid);
        page_item_connectors.push(cid);
        connectors.push(cid);
    }

    // 4) 300 frame-to-frame connectors at page level
    for k in 0..300usize {
        let fa = frames[k % 200];
        let fb = if k < 199 { frames[k + 1] } else { frames[(k + 20) % 200] };
        let cid = doc.alloc_id();
        let props = ConnectorProps { start: ConnectorEnd::attached(fa, Magnet::Auto), end: ConnectorEnd::attached(fb, Magnet::Auto), ..Default::default() };
        doc.insert(page, Node::connector(cid, props)).unwrap();
        sync_connector(&mut doc, cid);
        frame_connectors.push(cid);
        connectors.push(cid);
    }

    // 5) 200 connectors inside frames
    for (fid, ca, cb) in inner_targets {
        let cid = doc.alloc_id();
        let props = ConnectorProps { start: ConnectorEnd::attached(ca, Magnet::Bottom), end: ConnectorEnd::attached(cb, Magnet::Top), ..Default::default() };
        doc.insert(fid, Node::connector(cid, props)).unwrap();
        sync_connector(&mut doc, cid);
        inner_connectors.push(cid);
        connectors.push(cid);
    }

    PerfWhiteboardScene { doc, page, frames, items, connectors, page_item_connectors, frame_connectors, inner_connectors, hub_50, hub_150 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{connector_route, Affine, Vec2};

    fn two_sticky_doc() -> (Document, NodeId, NodeId, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let a_id = d.alloc_id();
        let a = d.insert(page, Node::sticky(a_id, Rect::new(0.0, 0.0, 100.0, 100.0), "a")).unwrap();
        let b_id = d.alloc_id();
        let b = d.insert(page, Node::sticky(b_id, Rect::new(300.0, 200.0, 100.0, 100.0), "b")).unwrap();
        let c_id = d.alloc_id();
        let props = ConnectorProps { start: ConnectorEnd::attached(a, Magnet::Auto), end: ConnectorEnd::attached(b, Magnet::Auto), ..Default::default() };
        let c = d.insert(page, Node::connector(c_id, props)).unwrap();
        sync_connector(&mut d, c);
        (d, a, b, c)
    }

    #[test]
    fn route_cache_returns_same_route_without_recompute() {
        let (d, _a, _b, c) = two_sticky_doc();
        clear_route_cache();
        assert_eq!(route_cache_stats(), (0, 0));

        let r1 = connector_route(&d, c).unwrap();
        assert_eq!(route_cache_stats(), (0, 1));

        let r2 = connector_route(&d, c).unwrap();
        assert_eq!(route_cache_stats(), (1, 1));
        assert_eq!(r1, r2);

        let cloned = d.clone();
        let r3 = connector_route(&cloned, c).unwrap();
        assert_eq!(route_cache_stats(), (2, 1));
        assert_eq!(r1, r3);
    }

    #[test]
    fn route_cache_misses_when_either_anchor_or_line_changes() {
        let (mut d, a, b, c) = two_sticky_doc();
        clear_route_cache();

        let r0 = connector_route(&d, c).unwrap();
        assert_eq!(route_cache_stats(), (0, 1));

        // 1. Start node rect position changes
        d.get_mut(a).unwrap().transform = Affine::translate(40.0, 60.0);
        let r1 = connector_route(&d, c).unwrap();
        assert_eq!(route_cache_stats(), (0, 2));
        assert_ne!(r1, r0);

        // 2. End node rect size changes
        d.get_mut(b).unwrap().size = Vec2::new(160.0, 140.0);
        let r2 = connector_route(&d, c).unwrap();
        assert_eq!(route_cache_stats(), (0, 3));
        assert_ne!(r2, r1);

        // 3. Magnet changes
        if let NodeKind::Connector(props) = &mut d.get_mut(c).unwrap().kind {
            props.start.magnet = Magnet::Top;
        }
        let r3 = connector_route(&d, c).unwrap();
        assert_eq!(route_cache_stats(), (0, 4));
        assert_ne!(r3, r2);

        // 4. Line style changes to Straight
        if let NodeKind::Connector(props) = &mut d.get_mut(c).unwrap().kind {
            props.line = ConnectorLine::Straight;
        }
        let r4 = connector_route(&d, c).unwrap();
        assert_eq!(route_cache_stats(), (0, 5));
        assert_eq!(r4.pts.len(), 2);
        assert!(r4.curve.is_none());
        assert_ne!(r4, r3);

        // 5. Line style changes to Curved
        if let NodeKind::Connector(props) = &mut d.get_mut(c).unwrap().kind {
            props.line = ConnectorLine::Curved;
        }
        let r5 = connector_route(&d, c).unwrap();
        assert_eq!(route_cache_stats(), (0, 6));
        assert!(r5.curve.is_some());
        assert_ne!(r5, r4);

        // Repeating without change hits the cache
        let r6 = connector_route(&d, c).unwrap();
        assert_eq!(route_cache_stats(), (1, 6));
        assert_eq!(r6, r5);
    }

    #[test]
    fn route_cache_never_returns_a_stale_route_after_direct_node_write() {
        let (mut d, a, b, c) = two_sticky_doc();
        clear_route_cache();

        let r0 = connector_route(&d, c).unwrap();
        assert_eq!(r0.end(), Vec2::new(300.0, 250.0));
        assert_eq!(route_cache_stats(), (0, 1));

        // Edit through Document::get_mut
        d.get_mut(b).unwrap().transform = Affine::translate(420.0, 260.0);
        let r1 = connector_route(&d, c).unwrap();
        assert_eq!(r1.end(), Vec2::new(420.0, 310.0));
        assert_ne!(r1, r0);
        assert_eq!(route_cache_stats(), (0, 2));

        // Edit directly through Document::nodes.get_mut (bypasses dirty log and epoch)
        d.nodes.get_mut(&b).unwrap().transform = Affine::translate(520.0, 340.0);
        let r2 = connector_route(&d, c).unwrap();
        assert_eq!(r2.end(), Vec2::new(520.0, 390.0));
        assert_ne!(r2, r1);
        assert_eq!(route_cache_stats(), (0, 3));

        // Direct size write through Document::nodes.get_mut on start node
        d.nodes.get_mut(&a).unwrap().size = Vec2::new(180.0, 140.0);
        let r3 = connector_route(&d, c).unwrap();
        assert_eq!(r3.start(), Vec2::new(180.0, 70.0));
        assert_ne!(r3, r2);
        assert_eq!(route_cache_stats(), (0, 4));
    }

    #[test]
    fn route_cache_is_bounded() {
        clear_route_cache();
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let a_id = d.alloc_id();
        let a = d.insert(page, Node::sticky(a_id, Rect::new(0.0, 0.0, 100.0, 100.0), "a")).unwrap();
        let b_id = d.alloc_id();
        let b = d.insert(page, Node::sticky(b_id, Rect::new(300.0, 200.0, 100.0, 100.0), "b")).unwrap();

        let total = ROUTE_CACHE_CAP + 1_024;
        let mut cids = Vec::with_capacity(total);
        for i in 0..total {
            let cid = d.alloc_id();
            let props = ConnectorProps {
                start: ConnectorEnd::attached(a, Magnet::Auto),
                end: ConnectorEnd::free(Vec2::new(300.0 + (i % 64) as f32 * 4.0, 200.0 + (i / 64) as f32 * 4.0)),
                ..Default::default()
            };
            d.nodes.insert(cid, Node::connector(cid, props));
            let _ = b;
            cids.push(cid);
        }

        for &cid in &cids {
            assert!(connector_route(&d, cid).is_some());
            assert!(route_cache_len() <= ROUTE_CACHE_CAP, "cache len {} exceeded {}", route_cache_len(), ROUTE_CACHE_CAP);
        }
        assert_eq!(route_cache_len(), ROUTE_CACHE_CAP);

        // Newest entry is still resident (cache hit)
        let (h0, m0) = route_cache_stats();
        let _ = connector_route(&d, *cids.last().unwrap()).unwrap();
        assert_eq!(route_cache_stats(), (h0 + 1, m0));
        assert_eq!(route_cache_len(), ROUTE_CACHE_CAP);

        // Oldest entry was evicted (cache miss) and re-inserting keeps len at ROUTE_CACHE_CAP
        let _ = connector_route(&d, cids[0]).unwrap();
        assert_eq!(route_cache_stats(), (h0 + 1, m0 + 1));
        assert_eq!(route_cache_len(), ROUTE_CACHE_CAP);
    }
}
