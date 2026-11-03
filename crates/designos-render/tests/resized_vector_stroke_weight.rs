//! Resizing an imported stroked vector scales its path
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
    imported_stroke_only_icon_in(t, None, at, with_network)
}

fn imported_stroke_only_icon_in(t: &mut T, parent: Option<NodeId>, at: Vec2, with_network: bool) -> NodeId {
    let mut cmds = rect_subpath(1.0, 1.0, 19.0, 19.0, false);
    cmds.extend(rect_subpath(3.0, 3.0, 17.0, 17.0, true));
    let stroke_geo = vec![Path { cmds, fill_rule: FillRule::NonZero }];

    let network = with_network.then(|| {
        let centerline = vec![Path { cmds: rect_subpath(2.0, 2.0, 18.0, 18.0, false), fill_rule: FillRule::NonZero }];
        VectorNetwork::from_paths(&centerline)
    });

    t.add(parent, |id| {
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
fn unresized_imported_vector_renders_unchanged() {
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

    // Optional capture of the kit icon when DESIGNOS_KIT_FIG_PATH and DESIGNOS_KIT_CAPTURE_DIR are set.
    if let (Ok(kit_path), Ok(ev_dir)) = (std::env::var("DESIGNOS_KIT_FIG_PATH"), std::env::var("DESIGNOS_KIT_CAPTURE_DIR")) {
        let tag = std::env::var("DESIGNOS_KIT_CAPTURE_TAG").unwrap_or_else(|_| "after".into());
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

/// A closed 20x20 outline icon on a page frame turned into a main component, plus one instance of it at
/// `(20, 20)`. Returns `(component, instance, main icon, instance icon)`.
fn component_with_instance(t: &mut T) -> (NodeId, NodeId, NodeId, NodeId) {
    let frame = t.add(None, |id| Node::frame(id, Rect::new(200.0, 0.0, 60.0, 60.0)));
    let main_icon = imported_stroke_only_icon_in(t, Some(frame), Vec2::ZERO, true);
    create_component(&mut t.doc, frame).apply(&mut t.doc).unwrap();
    let page = t.page;
    create_instance(&mut t.doc, frame, page, Vec2::new(20.0, 20.0)).apply(&mut t.doc).unwrap();
    let instance = *t.doc.get(page).unwrap().children.last().unwrap();
    let inst_icon = t.doc.get(instance).unwrap().children[0];
    (frame, instance, main_icon, inst_icon)
}

fn vector_geometry(n: &Node) -> (&[Path], Option<&VectorNetwork>) {
    let NodeKind::Vector { paths, network } = &n.kind else { panic!("expected a vector") };
    (paths, network.as_ref())
}

#[test]
fn syncing_the_main_keeps_the_plain_resize_of_an_instance_child_with_a_size_override() {
    let mut t = T::new();
    let (comp, _instance, main_icon, inst_icon) = component_with_instance(&mut t);

    // the instance child is resized on its own, which leaves it without a baked stroke
    plain_resize(&mut t.doc, inst_icon, Vec2::new(40.0, 40.0));
    t.node(inst_icon).component.overrides.insert(OverrideField::Size);
    assert!(t.doc.get(inst_icon).unwrap().stroke_geometry.is_none(), "the resize left the instance child without a bake");

    sync_instances(&mut t.doc, comp).apply(&mut t.doc).unwrap();

    // what a plain resize of the main's own icon to the same size gives
    let mut expected = t.doc.get(main_icon).unwrap().clone();
    plain_resize_node(&mut expected, Vec2::new(40.0, 40.0));
    let got = t.doc.get(inst_icon).unwrap();
    assert_eq!(got.size, Vec2::new(40.0, 40.0));
    assert_eq!(vector_geometry(got), vector_geometry(&expected), "paths and network are the centerline of the plain resize");
    assert_eq!(got.stroke_geometry, expected.stroke_geometry);

    // drawn as one band of the stroke's own weight per edge, not the scaled ring's two
    let pm = t.render(100, 100);
    assert_eq!(horizontal_ink_runs(&pm, 40, 15..65), vec![2, 2], "one 2px band on each vertical edge");
    assert_eq!(vertical_ink_runs(&pm, 40, 15..65), vec![2, 2], "one 2px band on each horizontal edge");
}

fn plain_resize_node(n: &mut Node, new_size: Vec2) {
    let tf = n.transform;
    apply_constrained_geometry(n, tf, new_size);
}

/// Imported like Figma writes a filled square, with a stroked whisker next to it when `whisker` is set: the fill
/// outline is the square, the network also holds an open, bent chain (stroke only), and the baked stroke covers both
/// at the given alignment.
fn square_with_whisker(t: &mut T, fills: bool, align: StrokeAlign, whisker: bool) -> NodeId {
    let square = Path { cmds: rect_subpath(6.0, 2.0, 16.0, 12.0, false), fill_rule: FillRule::NonZero };
    let mut chains = vec![square.clone()];
    if whisker {
        chains.push(Path {
            cmds: vec![PathCmd::MoveTo(Vec2::new(2.0, 14.0)), PathCmd::LineTo(Vec2::new(2.0, 18.0)), PathCmd::LineTo(Vec2::new(18.0, 18.0))],
            fill_rule: FillRule::NonZero,
        });
    }
    let net = VectorNetwork::from_paths(&chains);
    assert!(net.is_valid() && net.has_stroke_only_segments() == whisker, "the whisker is a stroke-only chain");
    let stroke = Stroke { weight: 2.0, align, ..Stroke::default() };
    let bake = designos_render::vectorops::outline_stroke(&net.to_paths(), &stroke, Some(&net));
    t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Vector { paths: vec![square.clone()], network: Some(net) });
        n.transform = Affine::translate(20.0, 20.0);
        n.size = Vec2::new(20.0, 20.0);
        n.fills = if fills { vec![Paint::solid(Color::rgb(0xF2, 0x4E, 0x1E))] } else { vec![] };
        n.stroke.paints = vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))];
        n.stroke.weight = 2.0;
        n.stroke.align = align;
        n.fill_geometry = Some(vec![square]);
        n.stroke_geometry = Some(bake);
        n
    })
}

fn bake_bounds(n: &Node) -> (f32, f32, f32, f32) {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for cmd in n.stroke_geometry.as_ref().expect("a baked stroke").iter().flat_map(|p| &p.cmds) {
        if let PathCmd::MoveTo(p) | PathCmd::LineTo(p) = *cmd {
            (x0, y0, x1, y1) = (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y));
        } else if let PathCmd::QuadTo(c, p) = *cmd {
            for q in [c, p] {
                (x0, y0, x1, y1) = (x0.min(q.x), y0.min(q.y), x1.max(q.x), y1.max(q.y));
            }
        } else if let PathCmd::CubicTo(c1, c2, p) = *cmd {
            for q in [c1, c2, p] {
                (x0, y0, x1, y1) = (x0.min(q.x), y0.min(q.y), x1.max(q.x), y1.max(q.y));
            }
        }
    }
    (x0, y0, x1, y1)
}

fn is_ink(pm: &Pixmap, x: u32, y: u32) -> bool {
    let p = px(pm, x, y);
    p[0] < 128 && p[1] < 128 && p[2] < 128
}

#[test]
fn stroke_follows_the_network_while_the_fill_keeps_the_outline() {
    let mut t = T::new();
    let icon = square_with_whisker(&mut t, true, StrokeAlign::Center, true);
    plain_resize(&mut t.doc, icon, Vec2::new(40.0, 40.0));
    assert!(t.doc.get(icon).unwrap().stroke_geometry.is_none());

    // the whisker's bottom run, off the square: one band of the stroke's own weight
    let pm = t.render(100, 100);
    assert_eq!(vertical_ink_runs(&pm, 28, 15..95), vec![2], "the stroke-only chain is stroked once at its own weight");

    // the fill is the plain 2x scale of the square, untouched by the network
    let mut no_stroke = t.doc.clone();
    no_stroke.get_mut(icon).unwrap().stroke.paints.clear();
    let mut reference = T::new();
    reference.add(None, |id| {
        let mut n = Node::new(
            id,
            NodeKind::Vector { paths: vec![Path { cmds: rect_subpath(12.0, 4.0, 32.0, 24.0, false), fill_rule: FillRule::NonZero }], network: None },
        );
        n.transform = Affine::translate(20.0, 20.0);
        n.size = Vec2::new(40.0, 40.0);
        n.fills = vec![Paint::solid(Color::rgb(0xF2, 0x4E, 0x1E))];
        n
    });
    let mut filled = Pixmap::new(100, 100).unwrap();
    render_page(&no_stroke, t.page, &Viewport::default(), &mut filled);
    assert_eq!(filled.data(), reference.render(100, 100).data(), "the fill pixels equal a plain 2x scale of the square");
}

#[test]
fn an_inside_or_outside_stroke_of_a_network_with_an_open_chain_keeps_its_scaled_bake() {
    for align in [StrokeAlign::Inside, StrokeAlign::Outside] {
        let mut t = T::new();
        let icon = square_with_whisker(&mut t, true, align, true);
        let (x0, y0, x1, y1) = bake_bounds(t.doc.get(icon).unwrap());
        plain_resize(&mut t.doc, icon, Vec2::new(40.0, 40.0));
        let n = t.doc.get(icon).unwrap();
        assert_eq!(bake_bounds(n), (x0 * 2.0, y0 * 2.0, x1 * 2.0, y1 * 2.0), "{align:?}: the baked outline stays and is scaled with the shape");

        // the square's left edge sits at x = 32; the bake of an inside stroke lies inside it and an outside one outside it
        let pm = t.render(100, 100);
        match align {
            StrokeAlign::Inside => assert!(!is_ink(&pm, 31, 34), "inside: a pixel just outside the square edge is not stroked"),
            _ => assert!(!is_ink(&pm, 33, 34), "outside: a pixel just inside the square edge is not stroked"),
        }
        let edge = if align == StrokeAlign::Inside { 33 } else { 31 };
        assert!(is_ink(&pm, edge, 34), "{align:?}: the stroke is drawn on its own side of the edge");
    }
}

#[test]
fn an_inside_stroke_of_a_closed_network_still_drops_its_bake_and_stays_inside() {
    let mut t = T::new();
    let icon = square_with_whisker(&mut t, true, StrokeAlign::Inside, false);
    plain_resize(&mut t.doc, icon, Vec2::new(40.0, 40.0));
    assert!(t.doc.get(icon).unwrap().stroke_geometry.is_none(), "the live stroke draws a closed inside stroke");
    let pm = t.render(100, 100);
    assert_eq!(horizontal_ink_runs(&pm, 34, 25..60), vec![2, 2], "one 2px band inside each vertical edge");
    assert!(!is_ink(&pm, 31, 34), "nothing outside the edge");
}

/// With `DESIGNOS_KIT_FIG_PATH` and `DESIGNOS_KIT_RENDER_HASHES_OUT` set, renders the unresized kit vectors one by
/// one (the first ones in the file, and the ones that keep Figma's fill outline without a baked stroke) and writes
/// one `id name hash` line each. The lines of two builds can be compared: a line that differs is a vector that
/// renders other pixels.
#[test]
fn unresized_kit_vector_renders_are_hashed_into_a_file() {
    let (Ok(kit_path), Ok(out)) = (std::env::var("DESIGNOS_KIT_FIG_PATH"), std::env::var("DESIGNOS_KIT_RENDER_HASHES_OUT")) else { return };
    let bytes = std::fs::read(&kit_path).expect("read kit .fig");
    let doc = designos_fig::import_fig(&bytes).expect("import kit .fig");
    let mut ids: Vec<_> = doc.nodes.values().filter(|n| matches!(n.kind, NodeKind::Vector { .. })).map(|n| n.id).collect();
    ids.sort_by_key(|id| id.0);
    let keeps_outline = |id: &NodeId| doc.get(*id).is_some_and(|n| n.fill_geometry.is_some() && n.stroke_geometry.is_none());
    let mut sample: Vec<NodeId> = ids.iter().copied().take(300).collect();
    sample.extend(ids.iter().copied().filter(keeps_outline).take(300));
    sample.sort_by_key(|id| id.0);
    sample.dedup();

    let mut lines = String::new();
    for id in sample {
        let mut node = doc.get(id).unwrap().clone();
        let mut d = Document::new("kit-vector");
        let page = d.add_page("p");
        d.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::WHITE };
        let name = node.name.clone();
        node.id = d.alloc_id();
        node.parent = Some(page);
        node.transform = Affine::translate(4.0, 4.0);
        let zoom = 2.0f32;
        let (w, h) = (((node.size.x + 8.0) * zoom).ceil().clamp(1.0, 2048.0) as u32, ((node.size.y + 8.0) * zoom).ceil().clamp(1.0, 2048.0) as u32);
        d.insert(page, node).unwrap();
        let mut pm = Pixmap::new(w, h).unwrap();
        render_page(&d, page, &Viewport { offset: Vec2::ZERO, zoom, ..Viewport::default() }, &mut pm);
        let hash = pm.data().iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3));
        lines.push_str(&format!("{} {} {hash:016x}\n", id.0, name.replace(' ', "_")));
    }
    std::fs::write(out, lines).unwrap();
}
