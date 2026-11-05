//! Synthetic connector performance benchmarks on the `perf_whiteboard_2000` fixture.
//!
//! Run with:
//! `cargo test -p designos-gpu --test connector_perf --release --locked -- --nocapture`

use designos_doc::fixtures::perf_whiteboard_2000;
use designos_doc::{
    clear_connector_index, clear_route_cache, connector_route, connector_route_bounds, plan_reroute, reroute_attached, resolve_anchors, route,
    route_cache_stats, sync_connector, with_index, Affine, ConnectorEnd, ConnectorLine, ConnectorProps, Document, Magnet, Node, NodeId, NodeKind, Rect, Vec2,
};
use designos_render::board::lower;
use designos_render::SceneIndex;
use std::time::Instant;

fn percentiles_us(mut samples_us: Vec<f64>) -> (f64, f64) {
    assert!(!samples_us.is_empty());
    samples_us.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p50 = samples_us[samples_us.len() / 2];
    let p95_idx = ((samples_us.len() as f64 * 0.95).ceil() as usize).saturating_sub(1).min(samples_us.len() - 1);
    let p95 = samples_us[p95_idx];
    (p50, p95)
}

/// Prints every per-sample time (in call order) when `CONNECTOR_PERF_SAMPLES` is set, so a single
/// outlier such as a cold first call stays visible next to the p50/p95 summary.
fn trace_samples(label: &str, samples_us: &[f64]) {
    if std::env::var_os("CONNECTOR_PERF_SAMPLES").is_none() {
        return;
    }
    let ms: Vec<String> = samples_us.iter().map(|us| format!("{:.3}", us / 1_000.0)).collect();
    eprintln!("samples {label} (ms, call order): [{}]", ms.join(", "));
}

type AnchorPair = ((Option<Rect>, Magnet, Vec2), (Option<Rect>, Magnet, Vec2));

fn seeded_elbow_endpoints(count: usize) -> Vec<AnchorPair> {
    let magnets = [Magnet::Auto, Magnet::Top, Magnet::Right, Magnet::Bottom, Magnet::Left, Magnet::Center];
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    fn next_u32(state: &mut u64) -> u32 {
        *state ^= *state >> 12;
        *state ^= *state << 25;
        *state ^= *state >> 27;
        (state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32
    }
    fn next_f32(state: &mut u64, lo: f32, hi: f32) -> f32 {
        let t = (next_u32(state) as f64 / u32::MAX as f64) as f32;
        lo + (hi - lo) * t
    }

    let mut out = Vec::with_capacity(count);
    for idx in 0..count {
        let mode = idx % 10;
        let ra = if mode < 8 {
            Some(Rect::new(
                next_f32(&mut state, -600.0, 600.0),
                next_f32(&mut state, -600.0, 600.0),
                next_f32(&mut state, 24.0, 260.0),
                next_f32(&mut state, 24.0, 220.0),
            ))
        } else {
            None
        };
        let rb = if mode < 7 {
            Some(Rect::new(
                next_f32(&mut state, -600.0, 600.0),
                next_f32(&mut state, -600.0, 600.0),
                next_f32(&mut state, 24.0, 260.0),
                next_f32(&mut state, 24.0, 220.0),
            ))
        } else {
            None
        };
        let sm = magnets[(next_u32(&mut state) as usize) % magnets.len()];
        let em = magnets[(next_u32(&mut state) as usize) % magnets.len()];
        let pa = Vec2::new(next_f32(&mut state, -600.0, 600.0), next_f32(&mut state, -600.0, 600.0));
        let pb = Vec2::new(next_f32(&mut state, -600.0, 600.0), next_f32(&mut state, -600.0, 600.0));
        out.push(((ra, sm, pa), (rb, em, pb)));
    }
    out
}

/// Lowers all 1,500 connectors in `doc` to their rendered nodes (`designos_render::board::lower`),
/// matching per-frame connector lowering during canvas pan/zoom.
fn lower_all_connectors(doc: &Document, connectors: &[NodeId]) -> usize {
    let mut count = 0usize;
    for &cid in connectors {
        if let Some(n) = doc.get(cid) {
            if let Some(lowered) = std::hint::black_box(lower(doc, n)) {
                count += lowered.children.len() + 1;
            }
        }
    }
    count
}

/// Route-bounds prefiltered connector hit check matching `canvas::hit::connector_hit`.
fn prefiltered_connector_hit(doc: &Document, id: NodeId, parent_local: Vec2, tol: f32, checks: &mut usize) -> bool {
    let Some(n) = doc.get(id) else { return false };
    let margin = n.stroke.weight / 2.0 + tol * 3.0;
    let Some(box_bounds) = connector_route_bounds(doc, id, margin) else {
        return false;
    };
    if !box_bounds.contains(parent_local) {
        return false;
    }
    *checks += 1;
    let Some(r) = connector_route(doc, id) else { return false };
    r.distance_to(parent_local) <= margin
}

/// Top-level hover hit query using `SceneIndex`: every page-level connector is a candidate and is
/// rejected or tested through its cached route bounds, matching `canvas::hit::hit_in`.
fn hover_probe_with_index(doc: &Document, _page: NodeId, idx: &SceneIndex, p: Vec2, tol: f32) -> (Option<NodeId>, usize) {
    let mut cands = Vec::new();
    let m = tol * 3.0 + 4.0;
    idx.query_top_level_with_connectors((p.x - m, p.y - m, p.x + m, p.y + m), &mut cands);
    let mut polyline_checks = 0usize;
    for &cid in cands.iter().rev() {
        let Some(n) = doc.get(cid) else { continue };
        if !n.visible || n.locked {
            continue;
        }
        match &n.kind {
            NodeKind::Connector(_) => {
                if prefiltered_connector_hit(doc, cid, p, tol, &mut polyline_checks) {
                    return (Some(cid), polyline_checks);
                }
            }
            _ => {
                let local = n.transform.invert().unwrap_or(Affine::IDENTITY).apply(p);
                if local.x >= -tol && local.y >= -tol && local.x <= n.size.x + tol && local.y <= n.size.y + tol {
                    return (Some(cid), polyline_checks);
                }
            }
        }
    }
    (None, polyline_checks)
}

/// Marquee selection sweep (`500 x 500` rect) using `SceneIndex` and stored-box prefiltering.
fn marquee_sweep_with_index(doc: &Document, idx: &SceneIndex, r: Rect) -> usize {
    let mut cands = Vec::new();
    idx.query_top_level((r.x, r.y, r.x + r.w, r.y + r.h), &mut cands);
    let mut selected = 0usize;
    for &cid in &cands {
        let Some(n) = doc.get(cid) else { continue };
        if !n.visible || n.locked {
            continue;
        }
        let Some((x0, y0, x1, y1)) = idx.bounds_of(cid) else { continue };
        let bb = Rect::new(x0, y0, x1 - x0, y1 - y0);
        if r.intersects(&bb) {
            selected += 1;
        }
    }
    selected
}

#[test]
fn perf_scene_whiteboard_2000_budgets() {
    let mut scene = perf_whiteboard_2000();
    assert_eq!(scene.frames.len(), 200);
    assert_eq!(scene.items.len(), 2_000);
    assert_eq!(scene.connectors.len(), 1_500);
    assert_eq!(scene.page_item_connectors.len(), 1_000);
    assert_eq!(scene.frame_connectors.len(), 300);
    assert_eq!(scene.inner_connectors.len(), 200);

    // Add 4 connectors to a dedicated single-drag node and 80 connectors across 20 multi-drag nodes
    // (using items[1810..1920], which have 0 initial connectors) so all hub and multi-node drag scenarios run cleanly.
    let single_drag_node = scene.items[1_810];
    let mut single_drag_connectors = Vec::with_capacity(4);
    for k in 0..4usize {
        let cid = scene.doc.alloc_id();
        let props = ConnectorProps {
            start: ConnectorEnd::attached(single_drag_node, Magnet::Auto),
            end: ConnectorEnd::attached(scene.items[1_811 + k], Magnet::Auto),
            ..Default::default()
        };
        scene.doc.insert(scene.page, Node::connector(cid, props)).unwrap();
        sync_connector(&mut scene.doc, cid);
        single_drag_connectors.push(cid);
    }

    let multi_drag_nodes: Vec<NodeId> = (0..20usize).map(|i| scene.items[1_820 + i]).collect();
    let mut multi_drag_connectors = Vec::with_capacity(80);
    for k in 0..80usize {
        let src = multi_drag_nodes[k % 20];
        let dst = scene.items[1_840 + k];
        let cid = scene.doc.alloc_id();
        let props = ConnectorProps { start: ConnectorEnd::attached(src, Magnet::Auto), end: ConnectorEnd::attached(dst, Magnet::Auto), ..Default::default() };
        scene.doc.insert(scene.page, Node::connector(cid, props)).unwrap();
        sync_connector(&mut scene.doc, cid);
        multi_drag_connectors.push(cid);
    }

    // 1. Single elbow route on seeded corpus
    let corpus = seeded_elbow_endpoints(500);
    for (s, e) in corpus.iter().take(100) {
        let (a, b) = resolve_anchors(*s, *e);
        std::hint::black_box(route(ConnectorLine::Elbowed, &a, &b));
    }
    let mut elbow_samples_us = Vec::with_capacity(corpus.len());
    for (s, e) in &corpus {
        let (a, b) = resolve_anchors(*s, *e);
        let mut best_us = f64::INFINITY;
        for _ in 0..3 {
            let t0 = Instant::now();
            let r = std::hint::black_box(route(ConnectorLine::Elbowed, &a, &b));
            let us = t0.elapsed().as_nanos() as f64 / 1_000.0;
            assert!(r.pts.len() >= 2);
            if us < best_us {
                best_us = us;
            }
        }
        elbow_samples_us.push(best_us);
    }
    let (elbow_p50_us, elbow_p95_us) = percentiles_us(elbow_samples_us);

    // 2. Steady-state pan/zoom frame (1,500 connectors, 0 moved)
    clear_route_cache();
    let warm_count = lower_all_connectors(&scene.doc, &scene.connectors);
    assert!(warm_count >= 1_500);
    let (hits_before_pan, misses_before_pan) = route_cache_stats();
    let mut pan_samples_us = Vec::with_capacity(20);
    for _ in 0..20 {
        let t0 = Instant::now();
        let c = lower_all_connectors(&scene.doc, &scene.connectors);
        let us = t0.elapsed().as_nanos() as f64 / 1_000.0;
        assert_eq!(c, warm_count);
        pan_samples_us.push(us);
    }
    let (hits_after_pan, misses_after_pan) = route_cache_stats();
    let pan_hits = hits_after_pan - hits_before_pan;
    let pan_misses = misses_after_pan - misses_before_pan;
    trace_samples("2 pan/zoom", &pan_samples_us);
    let (pan_p50_us, pan_p95_us) = percentiles_us(pan_samples_us);

    // 3. Dragging 1 node with 4 attached connectors
    let mut drag4_samples_us = Vec::with_capacity(12);
    let mut drag4_max_misses = 0u64;
    for step in 1..=12usize {
        scene.doc.get_mut(single_drag_node).unwrap().transform.tx += (step as f32) * 3.0;
        let (_, m0) = route_cache_stats();
        let t0 = Instant::now();
        let touched = reroute_attached(&mut scene.doc, &[single_drag_node]);
        let us = t0.elapsed().as_nanos() as f64 / 1_000.0;
        let (_, m1) = route_cache_stats();
        assert_eq!(touched.len(), single_drag_connectors.len());
        drag4_max_misses = drag4_max_misses.max(m1 - m0);
        drag4_samples_us.push(us);
    }
    trace_samples("3 drag 4 conn", &drag4_samples_us);
    let (drag4_p50_us, drag4_p95_us) = percentiles_us(drag4_samples_us);

    // 4. Move 1 node with 50 connectors on the 2,000-node board (via plan_reroute)
    with_index(&scene.doc, |_| {});
    let mut drag50_samples_us = Vec::with_capacity(12);
    let mut drag50_max_misses = 0u64;
    for step in 1..=12usize {
        scene.doc.get_mut(scene.hub_50).unwrap().transform.ty += (step as f32) * 2.5;
        let (_, m0) = route_cache_stats();
        let t0 = Instant::now();
        let plan = plan_reroute(&scene.doc, &[scene.hub_50], None);
        for (cid, g) in &plan.updates {
            if let Some(n) = scene.doc.get_mut(*cid) {
                g.apply(n);
            }
        }
        let us = t0.elapsed().as_nanos() as f64 / 1_000.0;
        let (_, m1) = route_cache_stats();
        assert_eq!(plan.updates.len(), 50);
        drag50_max_misses = drag50_max_misses.max(m1 - m0);
        drag50_samples_us.push(us);
    }
    trace_samples("4 move 50 conn", &drag50_samples_us);
    let (drag50_p50_us, drag50_p95_us) = percentiles_us(drag50_samples_us);

    // 5. Move 1 node with 150 connectors: visible-only during drag + rest at commit
    let drag150_vp = Rect::new(8_000.0, 11_000.0, 4_500.0, 2_000.0);
    let mut drag150_vis_samples_us = Vec::with_capacity(12);
    let mut drag150_commit_samples_us = Vec::with_capacity(12);
    let mut drag150_max_misses = 0u64;
    for step in 1..=12usize {
        scene.doc.get_mut(scene.hub_150).unwrap().transform.tx += (step as f32) * 2.0;
        let (_, m0) = route_cache_stats();
        let t0 = Instant::now();
        let vis_plan = plan_reroute(&scene.doc, &[scene.hub_150], Some(drag150_vp));
        for (cid, g) in &vis_plan.updates {
            if let Some(n) = scene.doc.get_mut(*cid) {
                g.apply(n);
            }
        }
        let vis_us = t0.elapsed().as_nanos() as f64 / 1_000.0;
        assert!(!vis_plan.updates.is_empty());
        assert!(vis_plan.deferred > 0);
        assert_eq!(vis_plan.updates.len() + vis_plan.deferred, 150);

        let t1 = Instant::now();
        let commit_plan = plan_reroute(&scene.doc, &[scene.hub_150], None);
        for (cid, g) in &commit_plan.updates {
            if let Some(n) = scene.doc.get_mut(*cid) {
                g.apply(n);
            }
        }
        let commit_us = t1.elapsed().as_nanos() as f64 / 1_000.0;
        let (_, m1) = route_cache_stats();
        assert_eq!(commit_plan.updates.len(), vis_plan.deferred);
        drag150_max_misses = drag150_max_misses.max(m1 - m0);
        drag150_vis_samples_us.push(vis_us);
        drag150_commit_samples_us.push(commit_us);
    }
    trace_samples("5a move 150 conn visible", &drag150_vis_samples_us);
    trace_samples("5b move 150 conn commit", &drag150_commit_samples_us);
    let (drag150_vis_p50_us, drag150_vis_p95_us) = percentiles_us(drag150_vis_samples_us);
    let (drag150_p50_us, drag150_p95_us) = percentiles_us(drag150_commit_samples_us);

    // 6. Multi-selection drag of 20 nodes with 80 attached connectors
    let mut drag80_samples_us = Vec::with_capacity(12);
    let mut drag80_max_misses = 0u64;
    for step in 1..=12usize {
        for &nid in &multi_drag_nodes {
            scene.doc.get_mut(nid).unwrap().transform.ty += (step as f32) * 1.5;
        }
        let (_, m0) = route_cache_stats();
        let t0 = Instant::now();
        let touched = reroute_attached(&mut scene.doc, &multi_drag_nodes);
        let us = t0.elapsed().as_nanos() as f64 / 1_000.0;
        let (_, m1) = route_cache_stats();
        assert_eq!(touched.len(), 80);
        drag80_max_misses = drag80_max_misses.max(m1 - m0);
        drag80_samples_us.push(us);
    }
    trace_samples("6 multi-drag 80 conn", &drag80_samples_us);
    let (drag80_p50_us, drag80_p95_us) = percentiles_us(drag80_samples_us);

    // Remove the temporary drag test connectors so the page has the exact 1,500 fixture connectors for hover/marquee
    for cid in single_drag_connectors.into_iter().chain(multi_drag_connectors.into_iter()) {
        let _ = scene.doc.remove(cid);
    }

    // 7. Pointer-move hover hit-test on page with 2,200 nodes + 1,000 page-level connectors
    let mut idx = SceneIndex::new();
    idx.ensure(&scene.doc, scene.page, 1);
    let _ = lower_all_connectors(&scene.doc, &scene.connectors);

    let mut hover_pts = Vec::with_capacity(200);
    for k in 0..100usize {
        let col = (k % 25) as f32;
        let row = 4.0 + (k / 5) as f32;
        hover_pts.push(Vec2::new(col * 480.0 + 70.0, 11_000.0 + row * 220.0 + 60.0));
    }
    for k in 0..100usize {
        hover_pts.push(Vec2::new(-1_000.0 - k as f32 * 40.0, -1_000.0 - k as f32 * 40.0));
    }
    let mut hover_samples_us = Vec::with_capacity(hover_pts.len());
    let mut max_polyline_checks = 0usize;
    for &pt in &hover_pts {
        let t0 = Instant::now();
        let (hit, checks) = std::hint::black_box(hover_probe_with_index(&scene.doc, scene.page, &idx, pt, 2.0));
        let us = t0.elapsed().as_nanos() as f64 / 1_000.0;
        let _ = hit;
        max_polyline_checks = max_polyline_checks.max(checks);
        hover_samples_us.push(us);
    }
    let (hover_p50_us, hover_p95_us) = percentiles_us(hover_samples_us);

    // The first hover after the route cache was emptied computes every route it has not seen (reported, not asserted).
    clear_route_cache();
    let t0 = Instant::now();
    std::hint::black_box(hover_probe_with_index(&scene.doc, scene.page, &idx, hover_pts[0], 2.0));
    let hover_cold_first_us = t0.elapsed().as_nanos() as f64 / 1_000.0;
    let (_, cold_misses) = route_cache_stats();

    // 8. Marquee selection sweep (500 x 500 canvas rect) over 1,500 connectors
    let mut marquee_samples_us = Vec::with_capacity(50);
    for k in 0..50usize {
        let rx = (k % 10) as f32 * 900.0;
        let ry = 11_000.0 + (k / 10) as f32 * 900.0;
        let rect = Rect::new(rx, ry, 500.0, 500.0);
        let t0 = Instant::now();
        let count = std::hint::black_box(marquee_sweep_with_index(&scene.doc, &idx, rect));
        let us = t0.elapsed().as_nanos() as f64 / 1_000.0;
        let _ = count;
        marquee_samples_us.push(us);
    }
    let (marquee_p50_us, marquee_p95_us) = percentiles_us(marquee_samples_us);

    // 9. Move a frame with 1,000 internal connectors (0 updates)
    let mut internal_doc = Document::new("frame_1000_internal");
    let internal_page = internal_doc.add_page("Page 1");
    let internal_frame = internal_doc.alloc_id();
    internal_doc.insert(internal_page, Node::frame(internal_frame, Rect::new(0.0, 0.0, 2_000.0, 2_000.0))).unwrap();
    let ia = internal_doc.alloc_id();
    internal_doc.insert(internal_frame, Node::rectangle(ia, Rect::new(20.0, 20.0, 80.0, 60.0))).unwrap();
    let ib = internal_doc.alloc_id();
    internal_doc.insert(internal_frame, Node::rectangle(ib, Rect::new(300.0, 200.0, 80.0, 60.0))).unwrap();
    for _ in 0..1_000usize {
        let cid = internal_doc.alloc_id();
        let props = ConnectorProps { start: ConnectorEnd::attached(ia, Magnet::Auto), end: ConnectorEnd::attached(ib, Magnet::Auto), ..Default::default() };
        internal_doc.insert(internal_frame, Node::connector(cid, props)).unwrap();
        sync_connector(&mut internal_doc, cid);
    }
    with_index(&internal_doc, |_| {});
    let mut frame1000_samples_us = Vec::with_capacity(20);
    for step in 1..=20usize {
        internal_doc.get_mut(internal_frame).unwrap().transform.tx += (step as f32) * 5.0;
        let t0 = Instant::now();
        let plan = plan_reroute(&internal_doc, &[internal_frame], None);
        let us = t0.elapsed().as_nanos() as f64 / 1_000.0;
        assert!(plan.updates.is_empty());
        frame1000_samples_us.push(us);
    }
    trace_samples("9 move frame 1000 internal", &frame1000_samples_us);
    let (frame1000_p50_us, frame1000_p95_us) = percentiles_us(frame1000_samples_us);

    // 10. Index refresh after 1 node edit
    with_index(&scene.doc, |_| {});
    let mut refresh_samples_us = Vec::with_capacity(50);
    for step in 0..50usize {
        let nid = scene.items[200 + (step % 100)];
        scene.doc.get_mut(nid).unwrap().transform.tx += 1.0;
        let t0 = Instant::now();
        with_index(&scene.doc, |idx| {
            std::hint::black_box(idx.is_empty());
        });
        let us = t0.elapsed().as_nanos() as f64 / 1_000.0;
        refresh_samples_us.push(us);
    }
    let (refresh_p50_us, refresh_p95_us) = percentiles_us(refresh_samples_us);

    // 11. Full index rebuild on 112k-node synthetic doc
    let mut doc_112k = Document::new("synthetic_112k_bench");
    let page_112k = doc_112k.add_page("Page 1");
    let fa_112k = doc_112k.alloc_id();
    doc_112k.insert(page_112k, Node::rectangle(fa_112k, Rect::new(0.0, 0.0, 80.0, 60.0))).unwrap();
    let fb_112k = doc_112k.alloc_id();
    doc_112k.insert(page_112k, Node::rectangle(fb_112k, Rect::new(240.0, 0.0, 80.0, 60.0))).unwrap();
    let fc_112k = doc_112k.alloc_id();
    doc_112k
        .insert(
            page_112k,
            Node::connector(
                fc_112k,
                ConnectorProps {
                    start: ConnectorEnd::attached(fa_112k, Magnet::Auto),
                    end: ConnectorEnd::attached(fb_112k, Magnet::Auto),
                    ..Default::default()
                },
            ),
        )
        .unwrap();
    for i in 0..112_000usize {
        let rid = doc_112k.alloc_id();
        let mut n = Node::rectangle(rid, Rect::new((i % 200) as f32 * 20.0, (i / 200) as f32 * 20.0, 10.0, 10.0));
        n.parent = Some(page_112k);
        doc_112k.nodes.insert(rid, n);
    }
    let mut rebuild112k_samples_us = Vec::with_capacity(7);
    for _ in 0..7usize {
        clear_connector_index();
        let t0 = Instant::now();
        with_index(&doc_112k, |idx| {
            assert!(!idx.is_empty());
        });
        let us = t0.elapsed().as_nanos() as f64 / 1_000.0;
        rebuild112k_samples_us.push(us);
    }
    let (rebuild112k_p50_us, rebuild112k_p95_us) = percentiles_us(rebuild112k_samples_us);

    eprintln!("\n=== perf_whiteboard_2000 summary ===");
    eprintln!("1. Single elbow route (seeded 500):        p50 = {elbow_p50_us:7.2} us | p95 = {elbow_p95_us:7.2} us (budget <= 100 us)");
    eprintln!(
        "2. Steady-state pan/zoom (1,500 conn):     p50 = {:7.3} ms | p95 = {:7.3} ms | hits={pan_hits} misses={pan_misses} (budget <= 3.5 ms, 0 misses)",
        pan_p50_us / 1_000.0,
        pan_p95_us / 1_000.0
    );
    eprintln!(
        "3. Drag 1 node (4 conn):                   p50 = {:7.3} ms | p95 = {:7.3} ms | max_misses={drag4_max_misses} (budget <= 1.0 ms)",
        drag4_p50_us / 1_000.0,
        drag4_p95_us / 1_000.0
    );
    eprintln!(
        "4. Move 1 node (50 conn):                  p50 = {:7.3} ms | p95 = {:7.3} ms | max_misses={drag50_max_misses} (budget <= 4.0 ms)",
        drag50_p50_us / 1_000.0,
        drag50_p95_us / 1_000.0
    );
    eprintln!(
        "5a. Move 1 node (150 conn, visible drag):  p50 = {:7.3} ms | p95 = {:7.3} ms (budget <= 6.0 ms)",
        drag150_vis_p50_us / 1_000.0,
        drag150_vis_p95_us / 1_000.0
    );
    eprintln!(
        "5b. Move 1 node (150 conn, commit rest):   p50 = {:7.3} ms | p95 = {:7.3} ms | max_misses={drag150_max_misses} (budget <= 12.0 ms)",
        drag150_p50_us / 1_000.0,
        drag150_p95_us / 1_000.0
    );
    eprintln!(
        "6. Multi-drag 20 nodes (80 conn):          p50 = {:7.3} ms | p95 = {:7.3} ms | max_misses={drag80_max_misses} (budget <= 7.5 ms)",
        drag80_p50_us / 1_000.0,
        drag80_p95_us / 1_000.0
    );
    eprintln!(
        "7. Hover hit-test (2,200 nodes + 1k conn): p50 = {hover_p50_us:7.2} us | p95 = {hover_p95_us:7.2} us | max_checks={max_polyline_checks} (budget <= 1000 us)"
    );
    eprintln!("   Hover, first probe on an emptied route cache: {hover_cold_first_us:9.2} us, {cold_misses} routes computed (not a budget)");
    eprintln!(
        "8. Marquee 500x500 sweep (1,500 conn):     p50 = {:7.3} ms | p95 = {:7.3} ms (budget <= 1.5 ms)",
        marquee_p50_us / 1_000.0,
        marquee_p95_us / 1_000.0
    );
    eprintln!(
        "9. Move frame (1,000 internal conn):       p50 = {:7.3} ms | p95 = {:7.3} ms | updates=0 (budget <= 0.5 ms)",
        frame1000_p50_us / 1_000.0,
        frame1000_p95_us / 1_000.0
    );
    eprintln!("10. Index refresh after 1 node edit:       p50 = {refresh_p50_us:7.2} us | p95 = {refresh_p95_us:7.2} us (budget <= 50 us)");
    eprintln!(
        "11. Full index rebuild (112k nodes):       p50 = {:7.3} ms | p95 = {:7.3} ms (budget <= 25.0 ms)\n",
        rebuild112k_p50_us / 1_000.0,
        rebuild112k_p95_us / 1_000.0
    );

    // Structural & cache-invalidation invariants hold in both debug and release builds.
    assert_eq!(pan_hits, 20 * 1_500, "steady-state pan/zoom must hit the route cache on all 1,500 connectors per frame");
    assert_eq!(pan_misses, 0, "steady-state pan/zoom must record 0 route recomputations");
    assert!(drag4_max_misses <= 4, "single-node drag with 4 connectors recomputed {drag4_max_misses} routes");
    assert!(drag50_max_misses <= 50, "50-connector hub drag recomputed {drag50_max_misses} routes");
    assert!(drag150_max_misses <= 150, "150-connector stress hub drag recomputed {drag150_max_misses} routes");
    assert!(drag80_max_misses <= 80, "80-connector multi-drag recomputed {drag80_max_misses} routes");
    assert!(max_polyline_checks <= 8, "hover probe checked {max_polyline_checks} connector polylines (expected <= 8)");

    // Wall-clock p95 budgets are scaled by 15 in debug builds and CI runners.
    let scale = if cfg!(debug_assertions) || std::env::var_os("CI").is_some() { 15.0 } else { 1.0 };
    assert!(elbow_p95_us <= 100.0 * scale, "elbow p95 {elbow_p95_us:.2}us exceeded budget");
    assert!(pan_p95_us <= 3_500.0 * scale, "pan/zoom p95 {:.3}ms exceeded budget", pan_p95_us / 1_000.0);
    assert!(drag4_p95_us <= 1_000.0 * scale, "4-connector drag p95 {:.3}ms exceeded budget", drag4_p95_us / 1_000.0);
    assert!(drag50_p95_us <= 4_000.0 * scale, "50-connector hub drag p95 {:.3}ms exceeded budget", drag50_p95_us / 1_000.0);
    assert!(drag150_vis_p95_us <= 6_000.0 * scale, "150-connector visible drag p95 {:.3}ms exceeded budget", drag150_vis_p95_us / 1_000.0);
    assert!(drag150_p95_us <= 12_000.0 * scale, "150-connector commit p95 {:.3}ms exceeded budget", drag150_p95_us / 1_000.0);
    assert!(drag80_p95_us <= 7_500.0 * scale, "80-connector multi-drag p95 {:.3}ms exceeded budget", drag80_p95_us / 1_000.0);
    assert!(hover_p95_us <= 1_000.0 * scale, "hover hit-test p95 {hover_p95_us:.2}us exceeded budget");
    assert!(marquee_p95_us <= 1_500.0 * scale, "marquee 500x500 p95 {:.3}ms exceeded budget", marquee_p95_us / 1_000.0);
    assert!(frame1000_p95_us <= 500.0 * scale, "frame 1,000 internal connectors p95 {:.3}ms exceeded budget", frame1000_p95_us / 1_000.0);
    assert!(refresh_p95_us <= 50.0 * scale, "index refresh p95 {refresh_p95_us:.2}us exceeded budget");
    assert!(rebuild112k_p95_us <= 25_000.0 * scale, "112k-node index rebuild p95 {:.3}ms exceeded budget", rebuild112k_p95_us / 1_000.0);
}
