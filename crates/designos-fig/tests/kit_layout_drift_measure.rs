//! Measurement harness (ignored): re-solves every auto-layout frame of an imported kit against its imported state and
//! writes one CSV row per frame with direct movers plus one per moved child, with counterfactual solves that tell which
//! input is responsible. Reads `DESIGNOS_KIT_PATH`, writes into `DESIGNOS_DRIFT_OUT` (a directory).
use designos_doc::*;
use designos_fig::import_fig;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

fn page_space_bounds(wt: Affine, size: Vec2) -> [f32; 4] {
    let c = [wt.apply(Vec2::new(0.0, 0.0)), wt.apply(Vec2::new(size.x, 0.0)), wt.apply(Vec2::new(0.0, size.y)), wt.apply(Vec2::new(size.x, size.y))];
    let (mut x0, mut y0, mut x1, mut y1) = (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for p in c {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    [x0, y0, x1, y1]
}

fn collect_page_bounds(doc: &Document) -> HashMap<NodeId, [f32; 4]> {
    let mut out = HashMap::new();
    fn walk(doc: &Document, id: NodeId, pwt: Affine, is_page: bool, out: &mut HashMap<NodeId, [f32; 4]>) {
        let Some(n) = doc.get(id) else { return };
        let wt = if is_page { Affine::IDENTITY } else { pwt * n.transform };
        if !is_page {
            out.insert(id, page_space_bounds(wt, n.size));
        }
        for &c in &n.children {
            walk(doc, c, wt, false, out);
        }
    }
    for &p in &doc.pages {
        walk(doc, p, Affine::IDENTITY, true, &mut out);
    }
    out
}

fn edge_diff(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    (a[0] - b[0]).abs().max((a[1] - b[1]).abs()).max((a[2] - b[2]).abs()).max((a[3] - b[3]).abs())
}

#[derive(Clone, Copy, Default)]
struct Opts {
    imported_sizes: bool,
    fixed_frame: bool,
    no_refit: bool,
    no_limits: bool,
    counter_gap_from_item: bool,
    stretch_to_auto: bool,
    rot_bbox: bool,
}

type Solved = (Vec2, Vec<(NodeId, Rect)>);

fn solve(doc: &Document, fid: NodeId, o: Opts) -> Option<Solved> {
    let root = doc.get(fid)?;
    let resolved = resolve_node(doc, root);
    let mut al = resolved.kind.frame_props()?.auto_layout.clone()?;
    if root.children.is_empty() {
        return None;
    }
    if o.counter_gap_from_item && al.wrap && al.counter_spacing == 0.0 {
        al.counter_spacing = al.item_spacing;
    }
    if o.fixed_frame {
        al.primary_sizing = Sizing::Fixed;
        al.counter_sizing = Sizing::Fixed;
    }
    let frame = root.size;
    let own = layout_solver::SizeLimits::from(&root.layout_child);
    let stroke = resolved.stroke.clone();
    let horiz = al.direction != LayoutDirection::Vertical;
    let (ids, mut kids): (Vec<NodeId>, Vec<layout_solver::ChildIn>) =
        root.children.iter().filter_map(|c| doc.flow_child_input(*c, horiz).map(|k| (*c, k))).unzip();
    for (id, k) in ids.iter().zip(kids.iter_mut()) {
        if o.rot_bbox {
            let c = doc.get(*id)?;
            let b = page_space_bounds(c.transform, c.size);
            k.size = Vec2::new(b[2] - b[0], b[3] - b[1]);
        }
        if o.imported_sizes {
            k.size = doc.get(*id)?.size;
        }
        if o.stretch_to_auto && k.align_self == AlignSelf::Stretch {
            k.align_self = AlignSelf::Auto;
        }
        if o.no_limits {
            k.limits = Default::default();
        }
    }
    let measure = designos_doc::text_measure::layout_measurer();
    let out = layout_solver::compute_then_refit_wrapped(&al, frame, own, &stroke, &ids, &mut kids, |id, w| {
        if o.no_refit {
            return None;
        }
        let m = measure.as_deref()?;
        designos_doc::text_measure::height_resize_text_height(doc.get(id)?, w, m)
    });
    Some((out.frame, ids.into_iter().zip(out.rects).collect()))
}

fn moved_direct(doc: &Document, s: &Solved) -> (usize, usize, f32) {
    let (mut n, mut npos, mut mx) = (0, 0, 0.0f32);
    for (cid, r) in &s.1 {
        let Some(c) = doc.get(*cid) else { continue };
        let t = c.transform.translation();
        let r0 = page_space_bounds(c.transform, c.size);
        let r1 = [r.x, r.y, r.x + r.w, r.y + r.h];
        let d = edge_diff(&r0, &r1);
        if d > 0.5 {
            n += 1;
            mx = mx.max(d);
        }
        if (t.x - r.x).abs().max((t.y - r.y).abs()) > 0.5 {
            npos += 1;
        }
    }
    (n, npos, mx)
}

fn moved_direct_aabb(doc: &Document, s: &Solved) -> usize {
    s.1.iter()
        .filter(|(cid, r)| {
            let Some(c) = doc.get(*cid) else { return false };
            let b = page_space_bounds(c.transform, c.size);
            edge_diff(&b, &[r.x, r.y, r.x + r.w, r.y + r.h]) > 0.5
        })
        .count()
}

fn variant_count(doc: &Document, fid: NodeId, o: Opts) -> usize {
    solve(doc, fid, o).map_or(0, |s| moved_direct(doc, &s).0)
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

fn kind_label(n: &Node) -> &'static str {
    match &n.kind {
        NodeKind::Frame(_) => "frame",
        NodeKind::Group => "group",
        NodeKind::Text(_) => "text",
        NodeKind::Instance { .. } => "instance",
        NodeKind::Component(_) => "component",
        NodeKind::ComponentSet(_) => "component_set",
        NodeKind::Rectangle => "rect",
        NodeKind::Ellipse(_) => "ellipse",
        NodeKind::Vector { .. } => "vector",
        NodeKind::BooleanOp(_) => "boolop",
        NodeKind::Line => "line",
        _ => "other",
    }
}

fn q(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "'"))
}

fn page_of(doc: &Document, mut id: NodeId) -> (String, bool) {
    let mut in_inst = false;
    loop {
        let Some(n) = doc.get(id) else { return (String::new(), in_inst) };
        if matches!(n.kind, NodeKind::Page { .. }) {
            return (n.name.clone(), in_inst);
        }
        if matches!(n.kind, NodeKind::Instance { .. }) {
            in_inst = true;
        }
        match n.parent {
            Some(p) => id = p,
            None => return (String::new(), in_inst),
        }
    }
}

#[test]
#[ignore = "reads kit path from DESIGNOS_KIT_PATH, writes DESIGNOS_DRIFT_OUT"]
fn kit_layout_drift_measure() {
    let Some(kit) = std::env::var_os("DESIGNOS_KIT_PATH") else { return };
    let out_dir = std::path::PathBuf::from(std::env::var_os("DESIGNOS_DRIFT_OUT").expect("DESIGNOS_DRIFT_OUT"));
    if std::env::var_os("DESIGNOS_DRIFT_MEASURER").is_some() {
        designos_doc::text_measure::set_layout_measurer(Some(std::sync::Arc::new(designos_render::RenderTextMeasure)));
    }
    let doc = import_fig(&std::fs::read(&kit).expect("read kit")).expect("import");
    let before = collect_page_bounds(&doc);

    // Solve every frame against the imported state, as the census does.
    let mut ids: Vec<NodeId> = doc.nodes.keys().copied().collect();
    ids.sort_by_key(|i| i.0);
    let mut base: HashMap<NodeId, Solved> = HashMap::new();
    for &fid in &ids {
        if let Some(s) = solve(&doc, fid, Opts::default()) {
            base.insert(fid, s);
        }
    }
    // Local rect of every direct child that moved, and page-space mover attribution (nearest-to-root cause frame).
    let mut local_moved: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    for (fid, s) in &base {
        for (cid, r) in &s.1 {
            let c = doc.get(*cid).unwrap();
            if edge_diff(&page_space_bounds(c.transform, c.size), &[r.x, r.y, r.x + r.w, r.y + r.h]) > 0.5 {
                local_moved.entry(*fid).or_default().push(*cid);
            }
        }
    }
    // Page-space after-state.
    let mut after_doc = doc.clone();
    for fid in &ids {
        let Some(s) = base.get(fid) else { continue };
        if let Some(f) = after_doc.get_mut(*fid) {
            f.size = s.0;
        }
        for (cid, r) in &s.1 {
            if let Some(c) = after_doc.get_mut(*cid) {
                let write = designos_doc::slot_write(&c.transform, c.size, *r);
                c.transform = write.transform;
                c.size = write.size;
            }
        }
    }
    let after = collect_page_bounds(&after_doc);
    let mut moved: Vec<(NodeId, f32)> = Vec::new();
    for (id, b0) in &before {
        if let Some(b1) = after.get(id) {
            let d = edge_diff(b0, b1);
            if d > 0.5 {
                moved.push((*id, d));
            }
        }
    }
    let mut root_count: BTreeMap<NodeId, (usize, f32)> = BTreeMap::new();
    let mut nearest_count: BTreeMap<NodeId, usize> = BTreeMap::new();
    for &(id, d) in &moved {
        let (mut cur, mut root, mut near) = (Some(id), None, None);
        while let Some(c) = cur {
            let Some(n) = doc.get(c) else { break };
            if let Some(p) = n.parent {
                if local_moved.get(&p).is_some_and(|v| v.contains(&c)) {
                    near.get_or_insert(p);
                    root = Some(p);
                }
            }
            cur = n.parent;
        }
        let rf = root.or_else(|| doc.get(id).and_then(|n| n.parent)).unwrap_or(id);
        let e = root_count.entry(rf).or_default();
        e.0 += 1;
        e.1 = e.1.max(d);
        *nearest_count.entry(near.or_else(|| doc.get(id).and_then(|n| n.parent)).unwrap_or(id)).or_default() += 1;
    }

    let mut frames = String::from(
        "frame_id,frame_source,name,page,in_instance,frame_kind,dir,wrap,primary_align,counter_align,primary_sizing,counter_sizing,gap,counter_gap,pad_t,pad_r,pad_b,pad_l,stroke_included,layout_version,own_limits,n_children,n_flow,n_absolute,n_hidden,n_text,n_instance,n_frame,n_fill_main,n_fill_cross,n_limits,n_margin,n_align_self,n_rotated,imp_w,imp_h,solved_w,solved_h,direct_moved,direct_moved_pos,max_delta,root_subtree_moved,nearest_subtree_moved,v_imported_sizes,v_fixed_frame,v_both,v_no_refit,v_no_limits,v_all,v_counter_gap,v_stretch_auto,v_cg_stretch\n",
    );
    let mut kids_csv =
        String::from("frame_id,frame_source,page,child_id,child_source,child_name,child_kind,imp_x,imp_y,imp_w,imp_h,new_x,new_y,new_w,new_h,delta\n");
    let mut summary_total = 0usize;
    for (fid, (rc, rmax)) in &root_count {
        summary_total += rc;
        let Some(n) = doc.get(*fid) else { continue };
        let Some(s) = base.get(fid) else {
            let (page, ii) = page_of(&doc, *fid);
            let _ = writeln!(
                frames,
                "{},{},{},{},{},{},NOT_AUTOLAYOUT,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,{},{},,,,,,,",
                fid.0,
                q(n.source_id.as_deref().unwrap_or("")),
                q(&n.name),
                q(&page),
                ii,
                kind_label(n),
                rc,
                nearest_count.get(fid).unwrap_or(&0)
            );
            let _ = rmax;
            continue;
        };
        let resolved = resolve_node(&doc, n);
        let al = resolved.kind.frame_props().and_then(|f| f.auto_layout.clone()).unwrap();
        let own = layout_solver::SizeLimits::from(&n.layout_child);
        let (page, ii) = page_of(&doc, *fid);
        let (dm, dmp, mx) = moved_direct(&doc, s);
        let (mut n_abs, mut n_hid, mut n_text, mut n_inst, mut n_frame, mut n_fm, mut n_fc, mut n_lim, mut n_mar, mut n_as, mut n_rot) =
            (0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        for cid in &n.children {
            let c = doc.get(*cid).unwrap();
            if c.layout_child.absolute {
                n_abs += 1;
            }
            if !c.visible {
                n_hid += 1;
            }
            match kind_label(c) {
                "text" => n_text += 1,
                "instance" => n_inst += 1,
                "frame" | "component" => n_frame += 1,
                _ => {}
            }
            let lc = &c.layout_child;
            if lc.horizontal == Sizing::Fill || lc.grow > 0.0 {
                n_fm += 1;
            }
            if lc.vertical == Sizing::Fill || lc.align_self == AlignSelf::Stretch {
                n_fc += 1;
            }
            if lc.min_w.is_some() || lc.max_w.is_some() || lc.min_h.is_some() || lc.max_h.is_some() {
                n_lim += 1;
            }
            if lc.margin != [0.0; 4] {
                n_mar += 1;
            }
            if lc.align_self != AlignSelf::Auto {
                n_as += 1;
            }
            if c.transform.a.abs() < 0.999 || c.transform.d.abs() < 0.999 || c.transform.b.abs() > 1e-3 {
                n_rot += 1;
            }
        }
        let variant = |o: Opts| solve(&doc, *fid, o).map_or(-1, |s| moved_direct(&doc, &s).0 as i64);
        let v1 = variant(Opts { imported_sizes: true, ..Default::default() });
        let v2 = variant(Opts { fixed_frame: true, ..Default::default() });
        let v3 = variant(Opts { imported_sizes: true, fixed_frame: true, ..Default::default() });
        let v4 = variant(Opts { no_refit: true, ..Default::default() });
        let v5 = variant(Opts { no_limits: true, ..Default::default() });
        let v6 = variant(Opts { imported_sizes: true, fixed_frame: true, no_refit: true, no_limits: true, ..Default::default() });
        let v7 = variant(Opts { counter_gap_from_item: true, ..Default::default() });
        let v8 = variant(Opts { stretch_to_auto: true, ..Default::default() });
        let v9 = variant(Opts { counter_gap_from_item: true, stretch_to_auto: true, ..Default::default() });
        let _ = writeln!(
            frames,
            "{},{},{},{},{},{},{:?},{},{:?},{:?},{:?},{:?},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.1},{:.1},{:.1},{:.1},{},{},{:.2},{},{},{},{},{},{},{},{},{},{},{}",
            fid.0, q(n.source_id.as_deref().unwrap_or("")), q(&n.name), q(&page), ii, kind_label(n), al.direction, al.wrap, al.primary_align, al.counter_align, al.primary_sizing, al.counter_sizing,
            al.item_spacing, al.counter_spacing, al.padding[0], al.padding[1], al.padding[2], al.padding[3], al.stroke_included, al.layout_version,
            own.min_w.is_some() || own.max_w.is_some() || own.min_h.is_some() || own.max_h.is_some(),
            n.children.len(), s.1.len(), n_abs, n_hid, n_text, n_inst, n_frame, n_fm, n_fc, n_lim, n_mar, n_as, n_rot,
            n.size.x, n.size.y, s.0.x, s.0.y, dm, dmp, mx, rc, nearest_count.get(fid).unwrap_or(&0), v1, v2, v3, v4, v5, v6, v7, v8, v9
        );
        for (cid, r) in &s.1 {
            let c = doc.get(*cid).unwrap();
            let t = c.transform.translation();
            let d = edge_diff(&page_space_bounds(c.transform, c.size), &[r.x, r.y, r.x + r.w, r.y + r.h]);
            if d > 0.5 {
                let _ = writeln!(
                    kids_csv,
                    "{},{},{},{},{},{},{},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2}",
                    fid.0,
                    q(n.source_id.as_deref().unwrap_or("")),
                    q(&page),
                    cid.0,
                    q(c.source_id.as_deref().unwrap_or("")),
                    q(&c.name),
                    kind_label(c),
                    t.x,
                    t.y,
                    c.size.x,
                    c.size.y,
                    r.x,
                    r.y,
                    r.w,
                    r.h,
                    d
                );
            }
        }
    }
    // Regression check: frames that do not move today but would under a candidate change.
    let mut regress = String::from("frame_id,frame_source,name,page,baseline,counter_gap,stretch_auto,both\n");
    let mut rot = String::from("frame_id,frame_source,name,page,baseline_moved,aabb_baseline_moved,aabb_rot_bbox_moved\n");
    for fid in &ids {
        let Some(s0) = base.get(fid) else { continue };
        let n = doc.get(*fid).unwrap();
        let b0 = moved_direct(&doc, s0).0;
        if b0 == 0 {
            let a = variant_count(&doc, *fid, Opts { counter_gap_from_item: true, ..Default::default() });
            let b = variant_count(&doc, *fid, Opts { stretch_to_auto: true, ..Default::default() });
            let c = variant_count(&doc, *fid, Opts { counter_gap_from_item: true, stretch_to_auto: true, ..Default::default() });
            if a > 0 || b > 0 || c > 0 {
                let (page, _) = page_of(&doc, *fid);
                let _ = writeln!(regress, "{},{},{},{},{},{},{},{}", fid.0, q(n.source_id.as_deref().unwrap_or("")), q(&n.name), q(&page), b0, a, b, c);
            }
        }
        let has_rot =
            n.children.iter().any(|c| doc.get(*c).is_some_and(|c| c.transform.a.abs() < 0.999 || c.transform.d.abs() < 0.999 || c.transform.b.abs() > 1e-3));
        if has_rot {
            let ab = moved_direct_aabb(&doc, s0);
            let ar = solve(&doc, *fid, Opts { rot_bbox: true, ..Default::default() }).map_or(usize::MAX, |s| moved_direct_aabb(&doc, &s));
            let (page, _) = page_of(&doc, *fid);
            let _ = writeln!(rot, "{},{},{},{},{},{},{}", fid.0, q(n.source_id.as_deref().unwrap_or("")), q(&n.name), q(&page), b0, ab, ar);
        }
    }
    std::fs::create_dir_all(&out_dir).unwrap();
    let mut selfsz = String::from("frame_id,frame_source,name,page,in_instance,dir,wrap,primary_sizing,counter_sizing,n_flow,n_hidden,n_text,n_absolute,own_limits,imp_w,imp_h,solved_w,solved_h,dw,dh,subtree_layers,pad_t,pad_r,pad_b,pad_l,gap,stroke_included,layout_version,stroke_inset_sum,sum_flow_w,sum_flow_h,max_flow_w,max_flow_h\n");
    for fid in &ids {
        let Some(s0) = base.get(fid) else { continue };
        let n = doc.get(*fid).unwrap();
        if (s0.0.x - n.size.x).abs() > 0.5 || (s0.0.y - n.size.y).abs() > 0.5 {
            let al = resolve_node(&doc, n).kind.frame_props().and_then(|f| f.auto_layout.clone()).unwrap();
            let own = layout_solver::SizeLimits::from(&n.layout_child);
            let (page, ii) = page_of(&doc, *fid);
            let hid = n.children.iter().filter(|c| doc.get(**c).is_some_and(|c| !c.visible)).count();
            let txt = n.children.iter().filter(|c| doc.get(**c).is_some_and(|c| matches!(c.kind, NodeKind::Text(_)))).count();
            let abs = n.children.iter().filter(|c| doc.get(**c).is_some_and(|c| c.layout_child.absolute)).count();
            let mut sub = 0usize;
            let mut stack = vec![*fid];
            while let Some(x) = stack.pop() {
                for c in &doc.get(x).unwrap().children {
                    sub += 1;
                    stack.push(*c);
                }
            }
            let resolved = resolve_node(&doc, n);
            let inset = resolved.stroke.layout_inset();
            let (mut sw, mut sh, mut mw, mut mh) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
            for (cid, _) in &s0.1 {
                let c = doc.get(*cid).unwrap();
                sw += c.size.x;
                sh += c.size.y;
                mw = mw.max(c.size.x);
                mh = mh.max(c.size.y);
            }
            let _ = writeln!(
                selfsz,
                "{},{},{},{},{},{:?},{},{:?},{:?},{},{},{},{},{},{:.1},{:.1},{:.1},{:.1},{:.1},{:.1},{},{},{},{},{},{},{},{},{:.1},{:.1},{:.1},{:.1},{:.1}",
                fid.0,
                q(n.source_id.as_deref().unwrap_or("")),
                q(&n.name),
                q(&page),
                ii,
                al.direction,
                al.wrap,
                al.primary_sizing,
                al.counter_sizing,
                s0.1.len(),
                hid,
                txt,
                abs,
                own.min_w.is_some() || own.max_w.is_some() || own.min_h.is_some() || own.max_h.is_some(),
                n.size.x,
                n.size.y,
                s0.0.x,
                s0.0.y,
                s0.0.x - n.size.x,
                s0.0.y - n.size.y,
                sub,
                al.padding[0],
                al.padding[1],
                al.padding[2],
                al.padding[3],
                al.item_spacing,
                al.stroke_included,
                al.layout_version,
                inset.iter().sum::<f32>(),
                sw,
                sh,
                mw,
                mh
            );
        }
    }
    std::fs::create_dir_all(&out_dir).unwrap();
    std::fs::write(out_dir.join("self-resize.csv"), selfsz).unwrap();
    std::fs::write(out_dir.join("regressions.csv"), regress).unwrap();
    std::fs::write(out_dir.join("rotated-children.csv"), rot).unwrap();
    std::fs::create_dir_all(&out_dir).unwrap();
    std::fs::write(out_dir.join("frames.csv"), frames).unwrap();
    std::fs::write(out_dir.join("moved-children.csv"), kids_csv).unwrap();
    // Every (frame, child) pair whose own solve moves the child, independent of ancestor attribution: diffing this set
    // between two solver versions shows exactly which frames start or stop moving.
    let mut pairs: Vec<(u64, u64)> = local_moved.iter().flat_map(|(f, cs)| cs.iter().map(move |c| (f.0, c.0))).collect();
    pairs.sort_unstable();
    let mut local = String::from("frame_id,child_id\n");
    for (f, c) in pairs {
        let _ = writeln!(local, "{f},{c}");
    }
    std::fs::write(out_dir.join("local-movers.csv"), local).unwrap();
    eprintln!("total_page_layers={} moved={} attributed={}", before.len(), moved.len(), summary_total);
    let moved_in_hidden_subtree = moved.iter().filter(|(id, _)| in_hidden_subtree(&doc, *id)).count();
    eprintln!("moved_in_hidden_subtree={} moved_visible={}", moved_in_hidden_subtree, moved.len() - moved_in_hidden_subtree);
}
