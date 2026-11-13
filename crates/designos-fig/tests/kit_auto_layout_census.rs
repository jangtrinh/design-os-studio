use designos_doc::*;
use designos_fig::import_fig;
use std::collections::{BTreeMap, HashMap};

fn page_space_bounds(wt: Affine, size: Vec2) -> [f32; 4] {
    let corners = [wt.apply(Vec2::new(0.0, 0.0)), wt.apply(Vec2::new(size.x, 0.0)), wt.apply(Vec2::new(0.0, size.y)), wt.apply(Vec2::new(size.x, size.y))];
    let mut x0 = f32::INFINITY;
    let mut y0 = f32::INFINITY;
    let mut x1 = f32::NEG_INFINITY;
    let mut y1 = f32::NEG_INFINITY;
    for p in corners {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    [x0, y0, x1, y1]
}

fn collect_page_bounds(doc: &Document) -> HashMap<NodeId, [f32; 4]> {
    let mut out = HashMap::new();
    fn walk(doc: &Document, id: NodeId, parent_wt: Affine, is_page: bool, out: &mut HashMap<NodeId, [f32; 4]>) {
        let Some(n) = doc.get(id) else { return };
        let wt = if is_page { Affine::IDENTITY } else { parent_wt * n.transform };
        if !is_page {
            out.insert(id, page_space_bounds(wt, n.size));
        }
        for &cid in &n.children {
            walk(doc, cid, wt, false, out);
        }
    }
    for &page in &doc.pages {
        walk(doc, page, Affine::IDENTITY, true, &mut out);
    }
    out
}

fn max_edge_diff(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    (a[0] - b[0]).abs().max((a[1] - b[1]).abs()).max((a[2] - b[2]).abs()).max((a[3] - b[3]).abs())
}

/// A frame's solved size and the rect it gives each flow child.
type SolvedFrame = (NodeId, Vec2, Vec<(NodeId, Rect)>);

fn frame_free_space(doc: &Document, fid: NodeId) -> Option<(AutoLayout, f32, usize)> {
    let n = doc.get(fid)?;
    let resolved = resolve_node(doc, n);
    let al = resolved.kind.frame_props()?.auto_layout.clone()?;
    if al.direction == LayoutDirection::Grid {
        return Some((al, 0.0, n.children.len()));
    }
    let horiz = al.direction == LayoutDirection::Horizontal;
    let kids: Vec<_> = n.children.iter().filter_map(|&cid| doc.flow_child_input(cid, horiz)).collect();
    if kids.is_empty() {
        return Some((al, 0.0, 0));
    }
    let al_eff = al.with_stroke(&resolved.stroke);
    let [pt, pr, pb, pl] = al_eff.padding;
    let pm = if horiz { pl + pr } else { pt + pb };
    let own = layout_solver::SizeLimits::from(&n.layout_child);
    let (hug_w, hug_h) = layout_solver::hugged_axes(&al_eff);
    let hug_main = if horiz { hug_w } else { hug_h };
    let sum_main: f32 = kids
        .iter()
        .map(|k| {
            let nat = if horiz { k.limits.clamp_w(k.size.x) } else { k.limits.clamp_h(k.size.y) };
            let marg = if horiz { k.margin[3] + k.margin[1] } else { k.margin[0] + k.margin[2] };
            nat + marg
        })
        .sum();
    let gaps = al_eff.item_spacing * (kids.len() as f32 - 1.0);
    let mut frame_main = if horiz { own.clamp_w(n.size.x) } else { own.clamp_h(n.size.y) };
    if hug_main {
        let total = sum_main + gaps + pm;
        frame_main = if horiz { own.clamp_w(total) } else { own.clamp_h(total) };
    }
    let main_avail = (frame_main - pm).max(0.0);
    let free = main_avail - sum_main - gaps;
    Some((al, free, kids.len()))
}

/// Whether the layer, or an ancestor below its page, is hidden. Figma defers building invisible instance subtrees, so the
/// stored geometry in there is not a layout result: the census reports those layers apart from the visible ones.
fn in_hidden_subtree(doc: &Document, mut id: NodeId) -> bool {
    loop {
        let Some(n) = doc.get(id) else { return false };
        if matches!(n.kind, NodeKind::Page { .. }) {
            return false;
        }
        if !n.visible {
            return true;
        }
        match n.parent {
            Some(parent) => id = parent,
            None => return false,
        }
    }
}

#[test]
#[ignore = "reads kit path from DESIGNOS_KIT_PATH env var"]
fn kit_auto_layout_relayout_census() {
    let Some(kit_path) = std::env::var_os("DESIGNOS_KIT_PATH") else {
        eprintln!("DESIGNOS_KIT_PATH not set: skipped");
        return;
    };
    let bytes = std::fs::read(&kit_path).expect("read kit file");
    let mut doc = import_fig(&bytes).expect("import kit");
    let before_bounds = collect_page_bounds(&doc);
    let mut before_local: HashMap<NodeId, (Vec2, Vec2)> = HashMap::new();
    let mut frame_info: HashMap<NodeId, (AutoLayout, f32, usize)> = HashMap::new();
    for (&id, n) in &doc.nodes {
        before_local.insert(id, (n.transform.translation(), n.size));
        if let Some(info) = frame_free_space(&doc, id) {
            frame_info.insert(id, info);
        }
    }

    // Re-solve every auto-layout frame against the imported document state
    let mut auto_frames: Vec<NodeId> = frame_info.keys().copied().collect();
    auto_frames.sort_by_key(|id| id.0);
    let measure = designos_doc::text_measure::layout_measurer();
    let mut solved_frames: Vec<SolvedFrame> = Vec::with_capacity(auto_frames.len());
    for fid in auto_frames {
        let Some(root_node) = doc.get(fid) else { continue };
        let resolved = resolve_node(&doc, root_node);
        let Some(al) = resolved.kind.frame_props().and_then(|f| f.auto_layout.clone()) else { continue };
        if root_node.children.is_empty() {
            continue;
        }
        let frame = root_node.size;
        let own_limits = layout_solver::SizeLimits::from(&root_node.layout_child);
        let stroke = resolved.stroke.clone();
        let horiz = al.direction != LayoutDirection::Vertical;
        let (flow_ids, mut kids): (Vec<NodeId>, Vec<layout_solver::ChildIn>) =
            root_node.children.iter().filter_map(|cid| doc.flow_child_input(*cid, horiz).map(|k| (*cid, k))).unzip();
        let out = layout_solver::compute_then_refit_wrapped(&al, frame, own_limits, &stroke, &flow_ids, &mut kids, |id, w| {
            let measure = measure.as_deref()?;
            designos_doc::text_measure::height_resize_text_height(doc.get(id)?, w, measure)
        });
        let child_rects: Vec<(NodeId, Rect)> = flow_ids.into_iter().zip(out.rects).collect();
        solved_frames.push((fid, out.frame, child_rects));
    }
    for (fid, new_frame, child_rects) in solved_frames {
        if let Some(fnode) = doc.get_mut(fid) {
            fnode.size = new_frame;
        }
        for (cid, r) in child_rects {
            if let Some(cnode) = doc.get_mut(cid) {
                let write = designos_doc::slot_write(&cnode.transform, cnode.size, r);
                cnode.transform = write.transform;
                cnode.size = write.size;
            }
        }
    }

    let after_bounds = collect_page_bounds(&doc);

    let mut moved_ids: Vec<(NodeId, f32)> = Vec::new();
    for (&id, b0) in &before_bounds {
        if let Some(b1) = after_bounds.get(&id) {
            let d = max_edge_diff(b0, b1);
            if d > 0.5 {
                moved_ids.push((id, d));
            }
        }
    }
    moved_ids.sort_by_key(|(id, _)| id.0);

    // Which nodes had their own local rect (tx, ty, w, h) changed by > 0.5 px by their parent auto-layout frame?
    let mut local_moved_by_parent: BTreeMap<NodeId, Vec<(NodeId, f32)>> = BTreeMap::new();
    for (&id, &(t0, s0)) in &before_local {
        if !before_bounds.contains_key(&id) {
            continue;
        }
        let Some(n) = doc.get(id) else { continue };
        let Some(pid) = n.parent else { continue };
        if !frame_info.contains_key(&pid) {
            continue;
        }
        let t1 = n.transform.translation();
        let s1 = n.size;
        let r0 = [t0.x, t0.y, t0.x + s0.x, t0.y + s0.y];
        let r1 = [t1.x, t1.y, t1.x + s1.x, t1.y + s1.y];
        let d = max_edge_diff(&r0, &r1);
        if d > 0.5 {
            local_moved_by_parent.entry(pid).or_default().push((id, d));
        }
    }

    // Attribute each page-space moved layer to the nearest ancestor auto-layout frame (or self's parent)
    // whose direct child local rect moved by > 0.5 px (or top-most moving auto-layout ancestor).
    let is_end_or_center_neg = |fid: NodeId| -> bool {
        frame_info.get(&fid).is_some_and(|(al, free, _)| !al.wrap && matches!(al.primary_align, LayoutAlign::End | LayoutAlign::Center) && *free < -0.5)
    };

    let mut moved_in_end_center_neg = 0usize;
    let mut by_root_cause_frame: BTreeMap<NodeId, Vec<(NodeId, f32)>> = BTreeMap::new();
    let mut by_nearest_cause_frame: BTreeMap<NodeId, Vec<(NodeId, f32)>> = BTreeMap::new();

    for &(id, d) in &moved_ids {
        // Walk up from id to find all ancestor frames where that step's child moved locally > 0.5 px
        let mut cur = Some(id);
        let mut nearest_frame = None;
        let mut root_frame = None;
        let mut any_ec_neg = false;
        while let Some(cid) = cur {
            let Some(cn) = doc.get(cid) else { break };
            if let Some(pid) = cn.parent {
                if local_moved_by_parent.get(&pid).is_some_and(|v| v.iter().any(|(k, _)| *k == cid)) {
                    if nearest_frame.is_none() {
                        nearest_frame = Some(pid);
                    }
                    root_frame = Some(pid);
                    if is_end_or_center_neg(pid) {
                        any_ec_neg = true;
                    }
                }
                cur = Some(pid);
            } else {
                break;
            }
        }
        if any_ec_neg {
            moved_in_end_center_neg += 1;
        }
        let rf = root_frame.or_else(|| doc.get(id).and_then(|n| n.parent)).unwrap_or(id);
        let nf = nearest_frame.or_else(|| doc.get(id).and_then(|n| n.parent)).unwrap_or(id);
        by_root_cause_frame.entry(rf).or_default().push((id, d));
        by_nearest_cause_frame.entry(nf).or_default().push((id, d));
    }

    let direct_end_center_neg_children: usize = local_moved_by_parent.iter().filter(|(&fid, _)| is_end_or_center_neg(fid)).map(|(_, v)| v.len()).sum();
    let direct_end_center_neg_frames: usize = local_moved_by_parent.iter().filter(|(&fid, _)| is_end_or_center_neg(fid)).count();
    let total_end_center_neg_frames: usize = frame_info.keys().filter(|&&fid| before_bounds.contains_key(&fid) && is_end_or_center_neg(fid)).count();

    eprintln!("=== KIT AUTO-LAYOUT CENSUS ===");
    eprintln!("total_page_layers={}", before_bounds.len());
    eprintln!("total_auto_layout_frames_on_pages={}", frame_info.keys().filter(|id| before_bounds.contains_key(id)).count());
    eprintln!("total_end_or_center_neg_free_frames={}", total_end_center_neg_frames);
    eprintln!("total_moved_page_layers_gt_0_5px={}", moved_ids.len());
    let moved_in_hidden_subtree = moved_ids.iter().filter(|(id, _)| in_hidden_subtree(&doc, *id)).count();
    eprintln!("moved_in_hidden_subtree={} moved_visible={}", moved_in_hidden_subtree, moved_ids.len() - moved_in_hidden_subtree);
    eprintln!("moved_page_layers_caused_by_end_or_center_neg_free={}", moved_in_end_center_neg);
    eprintln!("direct_children_moved_in_end_or_center_neg_free={} (across {} frames)", direct_end_center_neg_children, direct_end_center_neg_frames);
    eprintln!(
        "total_direct_children_moved_locally_gt_0_5px={} (across {} frames)",
        local_moved_by_parent.values().map(|v| v.len()).sum::<usize>(),
        local_moved_by_parent.len()
    );

    let mut ranked_root: Vec<(NodeId, usize, f32)> = by_root_cause_frame
        .iter()
        .map(|(&fid, v)| {
            let max_d = v.iter().map(|(_, d)| *d).fold(0.0f32, f32::max);
            (fid, v.len(), max_d)
        })
        .collect();
    ranked_root.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.2.partial_cmp(&a.2).unwrap()).then_with(|| a.0 .0.cmp(&b.0 .0)));

    eprintln!("--- TOP 15 MOVERS BY ROOT CAUSE FRAME (page-space subtree layers moved) ---");
    for (rank, (fid, count, max_d)) in ranked_root.iter().take(15).enumerate() {
        let n = doc.get(*fid).unwrap();
        let (b_tx, b_sz) = before_local.get(fid).copied().unwrap_or_default();
        let (al, free, nkids) = frame_info.get(fid).cloned().unwrap();
        let direct_cnt = local_moved_by_parent.get(fid).map_or(0, |v| v.len());
        eprintln!(
            "#{:02} frame={} source_id={:?} name={:?} moved_subtree_layers={} direct_children_moved={} max_d={:.2} size={:.1}x{:.1} (before {:.1}x{:.1}) dir={:?} primary={:?}/{:?} counter={:?}/{:?} gap={:.1} c_gap={:.1} pad={:?} wrap={} free={:.2} flow_kids={}",
            rank + 1,
            fid.0,
            n.source_id,
            n.name,
            count,
            direct_cnt,
            max_d,
            n.size.x,
            n.size.y,
            b_sz.x,
            b_sz.y,
            al.direction,
            al.primary_align,
            al.primary_sizing,
            al.counter_align,
            al.counter_sizing,
            al.item_spacing,
            al.counter_spacing,
            al.padding,
            al.wrap,
            free,
            nkids,
        );
        let _ = b_tx;
    }

    let mut ranked_direct: Vec<(NodeId, usize, f32)> = local_moved_by_parent
        .iter()
        .map(|(&fid, v)| {
            let max_d = v.iter().map(|(_, d)| *d).fold(0.0f32, f32::max);
            (fid, v.len(), max_d)
        })
        .collect();
    ranked_direct.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.2.partial_cmp(&a.2).unwrap()).then_with(|| a.0 .0.cmp(&b.0 .0)));

    eprintln!("--- TOP 15 MOVERS BY DIRECT PARENT FRAME (direct children moved locally) ---");
    for (rank, (fid, count, max_d)) in ranked_direct.iter().take(15).enumerate() {
        let n = doc.get(*fid).unwrap();
        let (_, b_sz) = before_local.get(fid).copied().unwrap_or_default();
        let (al, free, nkids) = frame_info.get(fid).cloned().unwrap();
        let sub_cnt = by_nearest_cause_frame.get(fid).map_or(0, |v| v.len());
        eprintln!(
            "#{:02} frame={} source_id={:?} name={:?} direct_children_moved={} nearest_subtree_moved={} max_d={:.2} size={:.1}x{:.1} (before {:.1}x{:.1}) dir={:?} primary={:?}/{:?} counter={:?}/{:?} gap={:.1} c_gap={:.1} pad={:?} wrap={} free={:.2} flow_kids={}",
            rank + 1,
            fid.0,
            n.source_id,
            n.name,
            count,
            sub_cnt,
            max_d,
            n.size.x,
            n.size.y,
            b_sz.x,
            b_sz.y,
            al.direction,
            al.primary_align,
            al.primary_sizing,
            al.counter_align,
            al.counter_sizing,
            al.item_spacing,
            al.counter_spacing,
            al.padding,
            al.wrap,
            free,
            nkids,
        );
    }

    eprintln!("--- REMAINING END/CENTER NEGATIVE FREE-SPACE DIRECT MOVERS ---");
    for (&fid, kids_moved) in &local_moved_by_parent {
        if is_end_or_center_neg(fid) {
            let n = doc.get(fid).unwrap();
            let (_, b_sz) = before_local.get(&fid).copied().unwrap_or_default();
            let (al, free, nkids) = frame_info.get(&fid).cloned().unwrap();
            let sub_cnt = by_nearest_cause_frame.get(&fid).map_or(0, |v| v.len());
            let max_d = kids_moved.iter().map(|(_, d)| *d).fold(0.0f32, f32::max);
            eprintln!(
                "  frame={} source_id={:?} name={:?} direct_children_moved={} nearest_subtree_moved={} max_d={:.2} size={:.1}x{:.1} (before {:.1}x{:.1}) dir={:?} primary={:?}/{:?} counter={:?}/{:?} gap={:.1} pad={:?} stroke_inc={} free={:.2} flow_kids={}",
                fid.0,
                n.source_id,
                n.name,
                kids_moved.len(),
                sub_cnt,
                max_d,
                n.size.x,
                n.size.y,
                b_sz.x,
                b_sz.y,
                al.direction,
                al.primary_align,
                al.primary_sizing,
                al.counter_align,
                al.counter_sizing,
                al.item_spacing,
                al.padding,
                al.stroke_included,
                free,
                nkids,
            );
        }
    }

    if let Ok(dump_path) = std::env::var("DESIGNOS_CENSUS_DUMP") {
        let lines: Vec<String> = moved_ids.iter().map(|(id, d)| format!("{}\t{:.4}", id.0, d)).collect();
        std::fs::write(dump_path, lines.join("\n")).expect("write census dump");
    }
}
