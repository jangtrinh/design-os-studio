//! Acceptance tests for #945: resizing an imported stroked vector scales its path
//! (`paths` and `network`) while keeping its `stroke.weight` and without drawing
//! a stale or doubled stroke outline.

mod common;

use common::*;
use designos_doc::*;
use designos_render::diff_image::render_diff_image;
use designos_render::tiny_skia::Pixmap;
use designos_render::{render_page, Viewport};

/// Builds a closed rectangular path from `(x0, y0)` to `(x1, y1)`.
fn rect_subpath(x0: f32, y0: f32, x1: f32, y1: f32, reversed: bool) -> Vec<PathCmd> {
    if !reversed {
        vec![
            PathCmd::MoveTo(Vec2::new(x0, y0)),
            PathCmd::LineTo(Vec2::new(x1, y0)),
            PathCmd::LineTo(Vec2::new(x1, y1)),
            PathCmd::LineTo(Vec2::new(x0, y1)),
            PathCmd::Close,
        ]
    } else {
        vec![
            PathCmd::MoveTo(Vec2::new(x0, y0)),
            PathCmd::LineTo(Vec2::new(x0, y1)),
            PathCmd::LineTo(Vec2::new(x1, y1)),
            PathCmd::LineTo(Vec2::new(x1, y0)),
            PathCmd::Close,
        ]
    }
}

/// Constructs an imported `.fig` stroke-only vector icon of size `20x20` at `at`
/// with `stroke.weight = 2.0`:
/// - `network`: centerline square `(2, 2)..(18, 18)` (when `with_network` is true)
/// - `stroke_geometry`: baked 2px stroke ring `(1, 1)..(19, 19)` minus `(3, 3)..(17, 17)`
/// - `paths`: clone of `stroke_geometry` (matching `designos-fig`'s `rest_import.rs` when
///   `fillGeometry` is empty and `strokeGeometry` is non-empty).
fn imported_stroke_only_icon(t: &mut T, at: Vec2, with_network: bool) -> NodeId {
    let mut cmds = rect_subpath(1.0, 1.0, 19.0, 19.0, false);
    cmds.extend(rect_subpath(3.0, 3.0, 17.0, 17.0, true));
    let stroke_geo = vec![Path { cmds, fill_rule: FillRule::NonZero }];

    let network = with_network.then(|| {
        let centerline = vec![Path { cmds: rect_subpath(2.0, 2.0, 18.0, 18.0, false), fill_rule: FillRule::NonZero }];
        VectorNetwork::from_paths(&centerline)
    });

    t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Vector { paths: stroke_geo.clone(), network });
        n.name = "Icon".into();
        n.transform = Affine::translate(at.x, at.y);
        n.size = Vec2::new(20.0, 20.0);
        n.fills.clear();
        n.stroke.paints = vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))];
        n.stroke.weight = 2.0;
        n.stroke.align = StrokeAlign::Center;
        n.stroke.cap = StrokeCap::Round;
        n.stroke.join = StrokeJoin::Round;
        n.stroke_geometry = Some(stroke_geo);
        n
    })
}

/// Plain resize of `id` to `new_size` via `apply_constrained_geometry`.
fn plain_resize(doc: &mut Document, id: NodeId, new_size: Vec2) {
    let tf = doc.get(id).unwrap().transform;
    apply_constrained_geometry(doc.get_mut(id).unwrap(), tf, new_size);
}

/// Contiguous inked pixel run lengths along a horizontal scanline `y` in `x_range`.
fn horizontal_ink_runs(pm: &Pixmap, y: u32, x_range: std::ops::Range<u32>) -> Vec<u32> {
    let mut runs = Vec::new();
    let mut cur = 0u32;
    for x in x_range {
        let p = px(pm, x, y);
        let inked = p[0] < 128 && p[1] < 128 && p[2] < 128;
        if inked {
            cur += 1;
        } else if cur > 0 {
            runs.push(cur);
            cur = 0;
        }
    }
    if cur > 0 {
        runs.push(cur);
    }
    runs
}

/// Contiguous inked pixel run lengths along a vertical scanline `x` in `y_range`.
fn vertical_ink_runs(pm: &Pixmap, x: u32, y_range: std::ops::Range<u32>) -> Vec<u32> {
    let mut runs = Vec::new();
    let mut cur = 0u32;
    for y in y_range {
        let p = px(pm, x, y);
        let inked = p[0] < 128 && p[1] < 128 && p[2] < 128;
        if inked {
            cur += 1;
        } else if cur > 0 {
            runs.push(cur);
            cur = 0;
        }
    }
    if cur > 0 {
        runs.push(cur);
    }
    runs
}

#[test]
fn plain_resize_2x_and_2x1_keeps_stroke_weight_on_both_axes() {
    // 1x baseline: 20x20 icon at (20, 20) draws two 2px walls horizontally and vertically.
    let mut t1 = T::new();
    let _icon_1x = imported_stroke_only_icon(&mut t1, Vec2::new(20.0, 20.0), true);
    let pm_1x = t1.render(100, 100);
    assert_eq!(horizontal_ink_runs(&pm_1x, 30, 15..45), vec![2, 2], "1x icon has 2px left/right stroke walls");
    assert_eq!(vertical_ink_runs(&pm_1x, 30, 15..45), vec![2, 2], "1x icon has 2px top/bottom stroke walls");

    // (a1) Plain 2x2 resize (20x20 -> 40x40): stroke band stays 2px (not 4px).
    let mut t2 = T::new();
    let icon_2x = imported_stroke_only_icon(&mut t2, Vec2::new(20.0, 20.0), true);
    plain_resize(&mut t2.doc, icon_2x, Vec2::new(40.0, 40.0));
    let pm_2x = t2.render(100, 100);
    assert_eq!(horizontal_ink_runs(&pm_2x, 40, 15..65), vec![2, 2], "2x plain-resized icon keeps 2px left/right stroke walls instead of scaling to 4px");
    assert_eq!(vertical_ink_runs(&pm_2x, 40, 15..65), vec![2, 2], "2x plain-resized icon keeps 2px top/bottom stroke walls instead of scaling to 4px");

    // (a2) Plain 2x1 non-uniform resize (20x20 -> 40x20): even 2px stroke on both axes.
    let mut t_non_uniform = T::new();
    let icon_2x1 = imported_stroke_only_icon(&mut t_non_uniform, Vec2::new(20.0, 20.0), true);
    plain_resize(&mut t_non_uniform.doc, icon_2x1, Vec2::new(40.0, 20.0));
    let pm_2x1 = t_non_uniform.render(100, 100);
    let h_runs = horizontal_ink_runs(&pm_2x1, 30, 15..65);
    let v_runs = vertical_ink_runs(&pm_2x1, 40, 15..45);
    assert_eq!(h_runs, vec![2, 2], "2x1 plain-resized icon has 2px vertical stroke walls");
    assert_eq!(v_runs, vec![2, 2], "2x1 plain-resized icon has 2px horizontal stroke walls");
    assert_eq!(h_runs, v_runs, "2x1 plain-resized icon has an even stroke on both axes");
}

#[test]
fn stroke_only_vector_whose_outline_is_its_path_draws_once_after_resize() {
    // Subcase 1: imported `.fig` stroke-only vector with a valid centerline `network`
    // and `paths == stroke_geometry`. After 2x resize, `paths` is rebuilt from `network`
    // and `stroke_geometry` is dropped, so the stroke is drawn once at 2px (not doubled
    // into 4 runs `[2, 2, 2, 2]` and not thickened to `[4, 4]`).
    let mut t_with_net = T::new();
    let id_with_net = imported_stroke_only_icon(&mut t_with_net, Vec2::new(20.0, 20.0), true);
    plain_resize(&mut t_with_net.doc, id_with_net, Vec2::new(40.0, 40.0));
    let pm_with_net = t_with_net.render(100, 100);
    assert_eq!(horizontal_ink_runs(&pm_with_net, 40, 15..65), vec![2, 2], "stroke-only vector with network draws one 2px band per side after resize");

    // Subcase 2: stroke-only vector without a centerline `network` (`network: None`)
    // where `paths == stroke_geometry` (the outline is its only geometry). After 2x
    // resize, the outline is filled once (2 walls across the scanline, not 0 / lost
    // and not 4 concentric walls / double-stroked).
    let mut t_outline_only = T::new();
    let id_outline_only = imported_stroke_only_icon(&mut t_outline_only, Vec2::new(20.0, 20.0), false);
    plain_resize(&mut t_outline_only.doc, id_outline_only, Vec2::new(40.0, 40.0));
    let pm_outline_only = t_outline_only.render(100, 100);
    let runs = horizontal_ink_runs(&pm_outline_only, 40, 15..65);
    assert_eq!(runs.len(), 2, "outline-only vector without network draws its stroke once per side (got runs {runs:?})");
}

#[test]
fn null_control_unresized_imported_vector_renders_unchanged() {
    let mut t = T::new();
    let id = imported_stroke_only_icon(&mut t, Vec2::new(20.0, 20.0), true);
    let pm = t.render(60, 60);
    assert_eq!(horizontal_ink_runs(&pm, 30, 15..45), vec![2, 2]);
    assert_eq!(vertical_ink_runs(&pm, 30, 15..45), vec![2, 2]);
    // Calling apply_constrained_geometry with the same size is a no-op and produces 0 changed pixels.
    let before_doc = t.doc.clone();
    let n = t.doc.get(id).unwrap();
    let (tf, sz) = (n.transform, n.size);
    apply_constrained_geometry(t.doc.get_mut(id).unwrap(), tf, sz);
    let diff = render_diff_image(&before_doc, &t.doc, t.page, Rect::new(0.0, 0.0, 60.0, 60.0), 1.0).unwrap();
    assert_eq!(diff.changed_pixels, 0, "unresized imported vector renders identical pixels");

    // Optional evidence capture when DESIGNOS_KIT_FIG_PATH and DESIGNOS_EVIDENCE_DIR are set.
    if let (Ok(kit_path), Ok(ev_dir)) = (std::env::var("DESIGNOS_KIT_FIG_PATH"), std::env::var("DESIGNOS_EVIDENCE_DIR")) {
        let tag = std::env::var("DESIGNOS_EVIDENCE_TAG").unwrap_or_else(|_| "after".into());
        let bytes = std::fs::read(&kit_path).expect("read kit .fig");
        let doc = designos_fig::import_fig(&bytes).expect("import kit .fig");
        // Find the first stroke-only 20x18 or 20x20 imported "Icon" vector with both network and stroke_geometry.
        let sample = doc
            .nodes
            .values()
            .find(|n| {
                n.name == "Icon"
                    && n.fills.is_empty()
                    && (n.stroke.weight - 2.0).abs() < 0.01
                    && n.stroke_geometry.is_some()
                    && matches!(&n.kind, NodeKind::Vector { network: Some(net), .. } if net.is_valid())
            })
            .expect("kit contains stroke-only 2px Icon vector")
            .clone();

        let render_isolated = |mut icon: Node, resize_to: Option<Vec2>| -> Pixmap {
            let mut d = Document::new("kit-icon");
            let p = d.add_page("p");
            d.get_mut(p).unwrap().kind = NodeKind::Page { background: Color::WHITE };
            icon.id = d.alloc_id();
            icon.parent = Some(p);
            icon.transform = Affine::translate(10.0, 10.0);
            let id = icon.id;
            d.insert(p, icon).unwrap();
            if let Some(sz) = resize_to {
                let tf = d.get(id).unwrap().transform;
                apply_constrained_geometry(d.get_mut(id).unwrap(), tf, sz);
            }
            let sz = d.get(id).unwrap().size;
            let zoom = 4.0f32;
            let w = ((sz.x + 20.0) * zoom).ceil() as u32;
            let h = ((sz.y + 20.0) * zoom).ceil() as u32;
            let mut pm = Pixmap::new(w, h).unwrap();
            let vp = Viewport { offset: Vec2::ZERO, zoom, ..Viewport::default() };
            render_page(&d, p, &vp, &mut pm);
            pm
        };

        let out = std::path::Path::new(&ev_dir);
        std::fs::create_dir_all(out).unwrap();
        let pm_1x = render_isolated(sample.clone(), None);
        pm_1x.save_png(out.join(format!("kit-icon-1x-{tag}.png"))).unwrap();
        let sz_2x = Vec2::new(sample.size.x * 2.0, sample.size.y * 2.0);
        let pm_2x = render_isolated(sample, Some(sz_2x));
        pm_2x.save_png(out.join(format!("kit-icon-2x-{tag}.png"))).unwrap();
    }
}
