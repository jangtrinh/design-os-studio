//! Evidence probe (ignored by default): how far the live (no stored outline) stroke of every LINE layer of
//! the Untitled UI PRO v8.0 kit (`KIT_CLONE` or `DESIGNOS_KIT_PATH`) is from the band Figma stored.
//!
//! Each LINE that keeps a valid stored outline is rendered twice on the CPU renderer: as imported (Figma's
//! band, the ground truth) and with the outline dropped (the live stroke). The count of pixels whose
//! coverage differs by more than 25 percent is printed in total and per cap. LINEs that import with no
//! stored outline already draw live; they are counted separately.
//!
//! With `LINE_EVIDENCE_DIR` and `LINE_EVIDENCE_TAG` set, a contact sheet of two unrotated lines (stored
//! band on the left, live stroke on the right, red hairline on the layer's own axis) is written there.
//! Run with `--ignored --nocapture`. Nothing the probe writes is kit data to commit.

use designos_doc::*;
use designos_fig::import_fig;
use designos_render::render_node_into;
use designos_render::tiny_skia::{Color as SkColor, Paint as SkPaint, Pixmap, PixmapPaint, Rect as SkRect, Transform};
use std::collections::BTreeMap;

const MAX_SIDE: f32 = 2400.0;

/// Parent-space window around `n` covering its band with a margin, rendered at `scale` pixels per unit.
fn render_window(doc: &Document, n: &Node, local: (f32, f32, f32, f32), scale: f32) -> Option<Pixmap> {
    let m = n.transform;
    let corners = [(local.0, local.1), (local.2, local.1), (local.2, local.3), (local.0, local.3)].map(|(x, y)| m.apply(Vec2::new(x, y)));
    let x0 = corners.iter().map(|p| p.x).fold(f32::MAX, f32::min);
    let y0 = corners.iter().map(|p| p.y).fold(f32::MAX, f32::min);
    let x1 = corners.iter().map(|p| p.x).fold(f32::MIN, f32::max);
    let y1 = corners.iter().map(|p| p.y).fold(f32::MIN, f32::max);
    let (w, h) = (((x1 - x0) * scale).ceil() as u32, ((y1 - y0) * scale).ceil() as u32);
    if w == 0 || h == 0 || w as f32 > MAX_SIDE || h as f32 > MAX_SIDE {
        return None;
    }
    let mut pm = Pixmap::new(w, h)?;
    let ts = Transform::from_row(scale, 0.0, 0.0, scale, -x0 * scale, -y0 * scale);
    render_node_into(doc, n.id, ts, ts, &mut pm);
    Some(pm)
}

fn changed_pixels(a: &Pixmap, b: &Pixmap) -> u64 {
    a.pixels().iter().zip(b.pixels()).filter(|(p, q)| (i32::from(p.alpha()) - i32::from(q.alpha())).abs() > 64).count() as u64
}

fn is_unrotated(n: &Node) -> bool {
    let t = n.transform;
    (t.a - 1.0).abs() < 1e-4 && (t.d - 1.0).abs() < 1e-4 && t.b.abs() < 1e-4 && t.c.abs() < 1e-4
}

fn sheet(stored: &Pixmap, live: &Pixmap, axis_px: f32) -> Pixmap {
    let (w, h) = (stored.width() + live.width() + 24, stored.height().max(live.height()));
    let mut out = Pixmap::new(w, h).unwrap();
    out.fill(SkColor::WHITE);
    for (pm, x) in [(stored, 0i32), (live, stored.width() as i32 + 24)] {
        out.draw_pixmap(x, 0, pm.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
        let mut red = SkPaint::default();
        red.set_color_rgba8(230, 40, 40, 255);
        if let Some(r) = SkRect::from_xywh(x as f32, axis_px - 0.5, pm.width() as f32, 1.0) {
            out.fill_rect(r, &red, Transform::identity(), None);
        }
    }
    out
}

#[test]
#[ignore = "reads KIT_CLONE or DESIGNOS_KIT_PATH env var"]
fn kit_live_line_stroke_against_the_stored_band() {
    let path = std::env::var_os("KIT_CLONE").or_else(|| std::env::var_os("DESIGNOS_KIT_PATH")).expect("set KIT_CLONE or DESIGNOS_KIT_PATH");
    let mut doc = import_fig(&std::fs::read(path).expect("read the kit")).expect("import the kit");
    doc.hydrate_all_pages();
    let mut ids: Vec<NodeId> = doc.nodes.iter().filter(|(_, n)| matches!(n.kind, NodeKind::Line)).map(|(id, _)| *id).collect();
    ids.sort_by_key(|i| i.0);

    let (mut lines_without_outline, mut without_outline_px) = (0usize, 0u64);
    let (mut compared, mut skipped, mut total_changed) = (0usize, 0usize, 0u64);
    let mut by_cap: BTreeMap<String, (usize, u64)> = BTreeMap::new();
    let mut shown: Vec<(NodeId, bool)> = Vec::new();
    for id in ids {
        let n = doc.get(id).unwrap().clone();
        let (w, len) = (n.stroke.weight.max(0.0), n.size.x);
        if !n.stroke.paints.iter().any(|p| p.visible) || w <= 0.0 {
            continue;
        }
        let local = (-w - 2.0, -w - 2.0, len + w + 2.0, w + 2.0);
        let scale = (MAX_SIDE / (len + 2.0 * w + 8.0)).min(4.0);
        if n.live_stroke_outline().is_none() {
            lines_without_outline += 1;
            if let Some(pm) = render_window(&doc, &n, local, scale) {
                without_outline_px += pm.pixels().iter().filter(|p| p.alpha() > 64).count() as u64;
            }
            continue;
        }
        let Some(stored) = render_window(&doc, &n, local, scale) else {
            skipped += 1;
            continue;
        };
        let saved = (n.stroke_geometry.clone(), n.stroke_geometry_for.clone());
        {
            let m = doc.get_mut(id).unwrap();
            m.drop_stroke_outline();
        }
        let live = render_window(&doc, &n, local, scale);
        {
            let m = doc.get_mut(id).unwrap();
            m.stroke_geometry = saved.0;
            m.stroke_geometry_for = saved.1;
        }
        let Some(live) = live else {
            skipped += 1;
            continue;
        };
        let d = changed_pixels(&stored, &live);
        compared += 1;
        total_changed += d;
        let e = by_cap.entry(format!("{:?} w{}", n.stroke.cap, w)).or_default();
        e.0 += 1;
        e.1 += d;
        if is_unrotated(&n) && len > 30.0 && shown.len() < 2 && shown.iter().all(|(_, round)| *round != (n.stroke.cap == StrokeCap::Round)) {
            shown.push((id, n.stroke.cap == StrokeCap::Round));
        }
    }
    eprintln!("KIT LINE layers: {} compared, {} skipped (window too large), {} with no stored outline", compared, skipped, lines_without_outline);
    eprintln!("KIT LINE layers with no stored outline: {lines_without_outline}, inked pixels (>25% coverage) in their live render: {without_outline_px}");
    eprintln!("KIT live vs stored band: {total_changed} pixels differ over {compared} lines");
    for (k, (n, d)) in &by_cap {
        eprintln!("KIT   {k}: {n} lines, {d} differing pixels");
    }

    if let (Some(dir), Some(tag)) = (std::env::var_os("LINE_EVIDENCE_DIR"), std::env::var_os("LINE_EVIDENCE_TAG")) {
        for (id, round) in shown {
            let n = doc.get(id).unwrap().clone();
            let w = n.stroke.weight;
            let local = (-6.0, -w - 6.0, 26.0, 6.0);
            let scale = 16.0;
            let stored = render_window(&doc, &n, local, scale).expect("stored window");
            let saved = (n.stroke_geometry.clone(), n.stroke_geometry_for.clone());
            doc.get_mut(id).unwrap().drop_stroke_outline();
            let live = render_window(&doc, &n, local, scale).expect("live window");
            let m = doc.get_mut(id).unwrap();
            m.stroke_geometry = saved.0;
            m.stroke_geometry_for = saved.1;
            let axis = (n.transform.apply(Vec2::new(0.0, 0.0)).y - (n.transform.apply(Vec2::new(0.0, local.1)).y)) * scale;
            let name = format!("line-live-stroke-{}-{tag}.png", if round { "round" } else { "none" }, tag = tag.to_string_lossy());
            sheet(&stored, &live, axis).save_png(std::path::Path::new(&dir).join(&name)).expect("save the sheet");
            eprintln!("KIT wrote {name}: layer {:?}, weight {w}, cap {:?}", n.name, n.stroke.cap);
        }
    }
}
