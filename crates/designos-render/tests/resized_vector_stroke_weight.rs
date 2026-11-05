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
fn an_inside_or_outside_stroke_of_a_network_with_an_open_chain_drops_its_bake_and_strokes_live() {
    for align in [StrokeAlign::Inside, StrokeAlign::Outside] {
        let mut t = T::new();
        let icon = square_with_whisker(&mut t, true, align, true);
        plain_resize(&mut t.doc, icon, Vec2::new(40.0, 40.0));
        let n = t.doc.get(icon).unwrap();
        assert!(n.stroke_geometry.is_none(), "{align:?}: the baked outline is dropped on resize");

        // the square's left edge sits at x = 32; an inside stroke lies inside it and an outside one outside it
        let pm = t.render(100, 100);
        match align {
            StrokeAlign::Inside => assert!(!is_ink(&pm, 31, 34), "inside: a pixel just outside the square edge is not stroked"),
            _ => assert!(!is_ink(&pm, 33, 34), "outside: a pixel just inside the square edge is not stroked"),
        }
        let edge = if align == StrokeAlign::Inside { 33 } else { 31 };
        assert!(is_ink(&pm, edge, 34), "{align:?}: the stroke is drawn on its own side of the edge");
        assert_eq!(horizontal_ink_runs(&pm, 34, 25..60), vec![2, 2], "{align:?}: two 2px walls on the closed square");
        assert_eq!(vertical_ink_runs(&pm, 28, 15..95), vec![2], "{align:?}: one 2px centered band on the open chain");
    }
}

#[test]
fn file_type_network_resized_2x_strokes_closed_outline_inside_and_open_fold_centered() {
    let outline = Path {
        cmds: vec![
            PathCmd::MoveTo(Vec2::new(0.0, 0.0)),
            PathCmd::LineTo(Vec2::new(20.0, 0.0)),
            PathCmd::LineTo(Vec2::new(32.0, 12.0)),
            PathCmd::LineTo(Vec2::new(32.0, 40.0)),
            PathCmd::LineTo(Vec2::new(0.0, 40.0)),
            PathCmd::Close,
        ],
        fill_rule: FillRule::NonZero,
    };
    let fold = Path {
        cmds: vec![PathCmd::MoveTo(Vec2::new(20.0, 0.0)), PathCmd::LineTo(Vec2::new(20.0, 12.0)), PathCmd::LineTo(Vec2::new(32.0, 12.0))],
        fill_rule: FillRule::NonZero,
    };
    let net = VectorNetwork::from_paths(&[outline.clone(), fold]);
    let stroke = Stroke {
        paints: vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))],
        weight: 1.5,
        align: StrokeAlign::Inside,
        cap: StrokeCap::Round,
        join: StrokeJoin::Round,
        ..Stroke::default()
    };
    let bake = designos_render::vectorops::outline_stroke(&net.to_paths(), &stroke, Some(&net));
    let mut t = T::new();
    let icon = t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Vector { paths: net.to_paths(), network: Some(net) });
        n.transform = Affine::translate(20.0, 20.0);
        n.size = Vec2::new(32.0, 40.0);
        n.fills = vec![Paint::solid(Color::WHITE)];
        n.stroke = stroke;
        n.fill_geometry = Some(vec![outline]);
        n.stroke_geometry = Some(bake);
        n
    });
    plain_resize(&mut t.doc, icon, Vec2::new(64.0, 80.0));
    assert!(t.doc.get(icon).unwrap().stroke_geometry.is_none(), "file-type network drops its bake on 2x resize");

    let pm = t.render(120, 120);
    // Left page outline edge is at x = 20: inside stroke stays in [20, 21.5], never outside at x = 19 or doubled to x = 22.
    assert!(!is_ink(&pm, 19, 60), "nothing outside the left page edge");
    assert!(is_ink(&pm, 20, 60), "1.5px stroke inside the left page edge");
    assert!(!is_ink(&pm, 22, 60), "left page edge stroke does not thicken to 3px");
    // Open fold vertical leg is at x = 20 + 40 = 60, y = 20..44: centered 1.5px stroke, not 3px.
    assert!(is_ink(&pm, 60, 32), "fold leg is stroked at x = 60");
    assert_eq!(px(&pm, 58, 32), [255, 255, 255, 255], "fold leg is 1.5px centered, not 3px at x = 58");
    assert_eq!(px(&pm, 61, 32), [255, 255, 255, 255], "fold leg is 1.5px centered, not 3px at x = 61");
}

#[test]
fn mixed_miter_and_round_join_path_resized_2x_keeps_both_joins() {
    let path = Path {
        cmds: vec![
            PathCmd::MoveTo(Vec2::new(4.0, 16.0)),
            PathCmd::LineTo(Vec2::new(4.0, 4.0)),
            PathCmd::LineTo(Vec2::new(20.0, 4.0)),
            PathCmd::LineTo(Vec2::new(20.0, 16.0)),
        ],
        fill_rule: FillRule::NonZero,
    };
    let mut net = VectorNetwork::from_paths(std::slice::from_ref(&path));
    for v in &mut net.vertices {
        if (v.pos.x - 4.0).abs() < 1e-3 && (v.pos.y - 4.0).abs() < 1e-3 {
            v.join = Some(StrokeJoin::Miter);
        } else {
            v.join = Some(StrokeJoin::Round);
        }
    }
    let stroke = Stroke {
        paints: vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))],
        weight: 8.0,
        align: StrokeAlign::Center,
        cap: StrokeCap::None,
        join: StrokeJoin::Round,
        miter_limit: 4.0,
        ..Stroke::default()
    };
    let bake = designos_render::vectorops::outline_stroke(&net.to_paths(), &stroke, Some(&net));
    let mut t = T::new();
    let id = t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Vector { paths: net.to_paths(), network: Some(net) });
        n.transform = Affine::translate(10.0, 10.0);
        n.size = Vec2::new(24.0, 20.0);
        n.stroke = stroke;
        n.stroke_geometry = Some(bake);
        n
    });
    plain_resize(&mut t.doc, id, Vec2::new(48.0, 40.0));
    assert!(t.doc.get(id).unwrap().stroke_geometry.is_none(), "mixed-join path drops its bake on 2x resize");

    let pm = t.render(80, 60);
    // After 2x resize, Miter corner is at (18, 18) and Round corner is at (50, 18) with half-stroke 4.0.
    assert!(is_ink(&pm, 14, 14), "Miter corner keeps its sharp outer tip at (14, 14)");
    assert!(!is_ink(&pm, 53, 14), "Round corner rounds off its outer tip at (53, 14)");
}

#[test]
fn equal_cap_open_stroke_draws_through_the_same_path_as_node_cap() {
    let seg = Path { cmds: vec![PathCmd::MoveTo(Vec2::new(10.0, 20.0)), PathCmd::LineTo(Vec2::new(50.0, 20.0))], fill_rule: FillRule::NonZero };
    let mut net_eq = VectorNetwork::from_paths(std::slice::from_ref(&seg));
    for v in &mut net_eq.vertices {
        v.cap = Some(StrokeCap::Round);
    }
    let net_none = VectorNetwork::from_paths(std::slice::from_ref(&seg));
    let make = |net: VectorNetwork| {
        let mut t = T::new();
        let id = t.add(None, |id| {
            let mut n = Node::new(id, NodeKind::Vector { paths: net.to_paths(), network: Some(net) });
            n.size = Vec2::new(60.0, 40.0);
            n.stroke = Stroke {
                paints: vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))],
                weight: 12.0,
                align: StrokeAlign::Center,
                cap: StrokeCap::Round,
                join: StrokeJoin::Round,
                ..Stroke::default()
            };
            n
        });
        (t, id)
    };
    let (t_eq, id_eq) = make(net_eq);
    let (t_none, _) = make(net_none);
    assert!(
        designos_render::vectorops::vertex_caps(t_eq.doc.get(id_eq).unwrap()).is_none(),
        "equal per-vertex caps do not trigger the separate cap polygon path"
    );
    assert_eq!(t_eq.render(60, 40).data(), t_none.render(60, 40).data(), "equal-cap open stroke renders identical pixels");
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

/// An open path `(0, 0) -> (40, 0) -> (40, 40) -> (80, 40)` at `(20, 20)`, stroked live at weight 10 with the node
/// join Bevel and its own join on each inner vertex (Miter at `(40, 0)`, Round at `(40, 40)`).
fn open_path_with_mixed_vertex_joins(t: &mut T, cap: StrokeCap) -> NodeId {
    let path = Path {
        cmds: vec![
            PathCmd::MoveTo(Vec2::new(0.0, 0.0)),
            PathCmd::LineTo(Vec2::new(40.0, 0.0)),
            PathCmd::LineTo(Vec2::new(40.0, 40.0)),
            PathCmd::LineTo(Vec2::new(80.0, 40.0)),
        ],
        fill_rule: FillRule::NonZero,
    };
    let mut net = VectorNetwork::from_paths(std::slice::from_ref(&path));
    for v in &mut net.vertices {
        let on = |x: f32, y: f32| (v.pos.x - x).abs() < 1e-3 && (v.pos.y - y).abs() < 1e-3;
        v.join = if on(40.0, 0.0) {
            Some(StrokeJoin::Miter)
        } else if on(40.0, 40.0) {
            Some(StrokeJoin::Round)
        } else {
            None
        };
    }
    t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Vector { paths: net.to_paths(), network: Some(net) });
        n.transform = Affine::translate(20.0, 20.0);
        n.size = Vec2::new(80.0, 40.0);
        n.fills.clear();
        n.stroke = Stroke {
            paints: vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))],
            weight: 10.0,
            align: StrokeAlign::Center,
            cap,
            join: StrokeJoin::Bevel,
            miter_limit: 4.0,
            ..Stroke::default()
        };
        n
    })
}

/// Both ends of the open path carry the node cap pointing away from the stroke: a Round cap reaches half a
/// weight past each end point, a Square cap fills the square beyond it.
#[track_caller]
fn assert_caps_point_outward(pm: &Pixmap, cap: StrokeCap, what: &str) {
    // the path starts at page (20, 20) heading right and ends at page (100, 60) heading right
    assert!(is_ink(pm, 16, 20), "{what} {cap:?}: the start cap reaches left of the start point");
    assert!(is_ink(pm, 104, 60), "{what} {cap:?}: the end cap reaches right of the end point");
    assert!(!is_ink(pm, 13, 20), "{what} {cap:?}: the start cap stops half a weight from the start point");
    assert!(!is_ink(pm, 106, 60), "{what} {cap:?}: the end cap stops half a weight from the end point");
    let square = cap == StrokeCap::Square;
    assert_eq!(is_ink(pm, 16, 15), square, "{what} {cap:?}: the start cap fills its corner only when square");
    assert_eq!(is_ink(pm, 104, 64), square, "{what} {cap:?}: the end cap fills its corner only when square");
}

fn assert_open_stroke_with_per_vertex_joins_caps_outward(cap: StrokeCap) {
    let mut t = T::new();
    open_path_with_mixed_vertex_joins(&mut t, cap);
    assert_caps_point_outward(&t.render(130, 90), cap, "CPU");
}

#[test]
fn an_open_stroke_with_per_vertex_joins_caps_both_ends_pointing_outward_round() {
    assert_open_stroke_with_per_vertex_joins_caps_outward(StrokeCap::Round);
}

#[test]
fn an_open_stroke_with_per_vertex_joins_caps_both_ends_pointing_outward_square() {
    assert_open_stroke_with_per_vertex_joins_caps_outward(StrokeCap::Square);
}

fn polyline(points: &[(f32, f32)]) -> Vec<PathCmd> {
    let mut cmds = vec![PathCmd::MoveTo(Vec2::new(points[0].0, points[0].1))];
    cmds.extend(points[1..].iter().map(|p| PathCmd::LineTo(Vec2::new(p.0, p.1))));
    cmds
}

/// An open path stroked live at weight 10 at `(20, 20)`, node join Bevel and no cap, with a Miter join on the vertex at
/// `miter` and a Round join on the vertex at `round`.
fn open_stroke_with_a_miter_and_a_round_vertex(t: &mut T, cmds: Vec<PathCmd>, miter: (f32, f32), round: (f32, f32)) -> NodeId {
    let path = Path { cmds, fill_rule: FillRule::NonZero };
    let mut net = VectorNetwork::from_paths(std::slice::from_ref(&path));
    for v in &mut net.vertices {
        let on = |p: (f32, f32)| (v.pos.x - p.0).abs() < 1e-3 && (v.pos.y - p.1).abs() < 1e-3;
        v.join = if on(miter) {
            Some(StrokeJoin::Miter)
        } else if on(round) {
            Some(StrokeJoin::Round)
        } else {
            None
        };
    }
    let bounds = net.bounds();
    t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Vector { paths: net.to_paths(), network: Some(net) });
        n.transform = Affine::translate(20.0, 20.0);
        n.size = Vec2::new(bounds.x + bounds.w, bounds.y + bounds.h);
        n.fills.clear();
        n.stroke = Stroke {
            paints: vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))],
            weight: 10.0,
            align: StrokeAlign::Center,
            cap: StrokeCap::None,
            join: StrokeJoin::Bevel,
            miter_limit: 4.0,
            ..Stroke::default()
        };
        n
    })
}

/// A path with a Miter vertex and a Round vertex, and what each pixel centre (node space) must show: the Miter corner
/// reaches its square apex, the Round corner is an arc that is neither a bevel nor a square.
struct JoinCase {
    name: &'static str,
    cmds: Vec<PathCmd>,
    miter: (f32, f32),
    round: (f32, f32),
    /// `(what, x, y, ink)` in node space.
    probes: Vec<(&'static str, f32, f32, bool)>,
}

fn join_cases() -> Vec<JoinCase> {
    vec![
        JoinCase {
            name: "right_then_down_on_the_axis",
            cmds: polyline(&[(0.0, 0.0), (40.0, 0.0), (40.0, 40.0), (80.0, 40.0)]),
            miter: (40.0, 0.0),
            round: (40.0, 40.0),
            probes: vec![("miter apex", 44.5, -3.5, true), ("round arc", 36.5, 42.5, true), ("round outer corner", 35.5, 44.5, false)],
        },
        JoinCase {
            name: "right_then_down_off_the_axis",
            cmds: polyline(&[(8.0, 8.0), (48.0, 8.0), (48.0, 48.0), (88.0, 48.0)]),
            miter: (48.0, 8.0),
            round: (48.0, 48.0),
            probes: vec![("miter apex", 52.5, 4.5, true), ("round arc", 44.5, 50.5, true), ("round outer corner", 43.5, 52.5, false)],
        },
        JoinCase {
            name: "curve_then_right_then_down",
            cmds: vec![
                PathCmd::MoveTo(Vec2::new(0.0, 40.0)),
                PathCmd::CubicTo(Vec2::new(0.0, 16.0), Vec2::new(16.0, 0.0), Vec2::new(40.0, 0.0)),
                PathCmd::LineTo(Vec2::new(40.0, 40.0)),
                PathCmd::LineTo(Vec2::new(80.0, 40.0)),
            ],
            miter: (40.0, 0.0),
            round: (40.0, 40.0),
            probes: vec![
                ("curve", 11.5, 11.5, true),
                ("miter apex", 44.5, -3.5, true),
                ("round arc", 36.5, 42.5, true),
                ("round outer corner", 35.5, 44.5, false),
            ],
        },
        JoinCase {
            name: "down_then_right_on_the_axis",
            cmds: polyline(&[(0.0, 0.0), (0.0, 40.0), (40.0, 40.0), (40.0, 80.0)]),
            miter: (0.0, 40.0),
            round: (40.0, 40.0),
            probes: vec![("miter apex", -4.5, 44.5, true), ("round arc", 42.5, 36.5, true), ("round outer corner", 44.5, 35.5, false)],
        },
    ]
}

fn page_pixel(x: f32, y: f32) -> (u32, u32) {
    ((20.0 + x).floor() as u32, (20.0 + y).floor() as u32)
}

/// The probes of `case` that `pm` does not show as expected.
fn failed_join_probes(pm: &Pixmap, case: &JoinCase, what: &str) -> Vec<String> {
    case.probes
        .iter()
        .filter_map(|(probe, x, y, ink)| {
            let (px_x, px_y) = page_pixel(*x, *y);
            (is_ink(pm, px_x, px_y) != *ink)
                .then(|| format!("{what} {}: {probe} at node ({x}, {y}) should be {}", case.name, if *ink { "inked" } else { "white" }))
        })
        .collect()
}

#[test]
fn a_miter_vertex_between_round_vertices_keeps_its_sharp_apex_in_a_piecewise_stroke() {
    let mut failed = vec![];
    for case in join_cases() {
        let mut t = T::new();
        open_stroke_with_a_miter_and_a_round_vertex(&mut t, case.cmds.clone(), case.miter, case.round);
        failed.extend(failed_join_probes(&t.render(130, 110), &case, "CPU"));
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// An open `(10, 20) -> (60, 20)` line of weight 12 whose end vertices carry `cap` while the node's own cap is None.
fn line_with_vertex_caps(t: &mut T, cap: StrokeCap) -> NodeId {
    let seg = Path { cmds: vec![PathCmd::MoveTo(Vec2::new(10.0, 20.0)), PathCmd::LineTo(Vec2::new(60.0, 20.0))], fill_rule: FillRule::NonZero };
    let mut net = VectorNetwork::from_paths(std::slice::from_ref(&seg));
    let last = net.vertices.len() - 1;
    net.vertices[0].cap = Some(cap);
    net.vertices[last].cap = Some(cap);
    t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Vector { paths: net.to_paths(), network: Some(net) });
        n.size = Vec2::new(70.0, 40.0);
        n.fills.clear();
        n.stroke = Stroke {
            paints: vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))],
            weight: 12.0,
            align: StrokeAlign::Center,
            cap: StrokeCap::None,
            join: StrokeJoin::Miter,
            ..Stroke::default()
        };
        n
    })
}

/// First and last inked pixel of row `y` when the row is one unbroken run.
fn ink_span(pm: &Pixmap, y: u32) -> Option<(u32, u32)> {
    let xs: Vec<u32> = (0..pm.width()).filter(|x| is_ink(pm, *x, y)).collect();
    let (first, last) = (*xs.first()?, *xs.last()?);
    (xs.len() as u32 == last - first + 1).then_some((first, last))
}

#[test]
fn vertex_caps_of_an_open_line_reach_half_a_weight_past_both_ends() {
    let mut failed = vec![];
    for cap in [StrokeCap::Square, StrokeCap::Round] {
        let mut t = T::new();
        line_with_vertex_caps(&mut t, cap);
        // the line spans x = 10..60 and each cap adds half the weight: ink on x = 4..66
        let span = ink_span(&t.render(80, 40), 20);
        if span != Some((4, 65)) {
            failed.push(format!("CPU {cap:?}: ink on y = 20 spans {span:?}, expected Some((4, 65))"));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

#[test]
fn all_five_arrow_caps_draw_outward_heads_on_lines_and_open_vectors() {
    let seg = Path { cmds: vec![PathCmd::MoveTo(Vec2::new(20.0, 50.0)), PathCmd::LineTo(Vec2::new(80.0, 50.0))], fill_rule: FillRule::NonZero };
    let render_cap = |cap: StrokeCap, as_line: bool, end_only: bool| -> Pixmap {
        let mut t = T::new();
        t.add(None, |id| {
            let kind = if as_line {
                NodeKind::Line
            } else {
                let mut net = VectorNetwork::from_paths(std::slice::from_ref(&seg));
                if end_only {
                    net.vertices[0].cap = Some(StrokeCap::None);
                    net.vertices[1].cap = Some(cap);
                }
                NodeKind::Vector { paths: vec![seg.clone()], network: Some(net) }
            };
            let mut n = Node::new(id, kind);
            if as_line {
                n.transform = Affine::translate(20.0, 50.0);
                n.size = Vec2::new(60.0, 0.0);
            } else {
                n.size = Vec2::new(100.0, 100.0);
            }
            n.stroke = Stroke {
                paints: vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))],
                weight: 4.0,
                align: StrokeAlign::Center,
                cap: if end_only && !as_line { StrokeCap::None } else { cap },
                join: StrokeJoin::Round,
                ..Stroke::default()
            };
            n
        });
        t.render(110, 100)
    };

    for (as_line, end_only) in [(false, false), (true, false), (false, true)] {
        // 1. ArrowLines: 45-degree open V wings reaching backward from (80, 50) to ~(67.3, 37.3) and ~(67.3, 62.7).
        let pm = render_cap(StrokeCap::ArrowLines, as_line, end_only);
        assert!(is_ink(&pm, 70, 40) && is_ink(&pm, 70, 60), "ArrowLines (line={as_line}, end_only={end_only}): backward wings at (70, 40) and (70, 60)");
        assert!(!is_ink(&pm, 90, 40) && !is_ink(&pm, 90, 60), "ArrowLines: wings do not point forward of tip");
        if end_only {
            assert!(!is_ink(&pm, 30, 40), "ArrowLines end_only: start endpoint at (20, 50) has no wing");
        } else {
            assert!(is_ink(&pm, 30, 40) && is_ink(&pm, 30, 60), "ArrowLines node cap: start endpoint at (20, 50) has wings at (30, 40) and (30, 60)");
        }

        // 2. ArrowEquilateral: filled triangle tip at (80, 50), base at x = 60 with half-width ~11.55.
        let pm = render_cap(StrokeCap::ArrowEquilateral, as_line, end_only);
        assert!(is_ink(&pm, 62, 41) && is_ink(&pm, 62, 59), "ArrowEquilateral: wide base at x = 62, y = 41/59");
        assert!(!is_ink(&pm, 78, 41) && !is_ink(&pm, 78, 59), "ArrowEquilateral: narrows toward tip at x = 78");
        assert!(!is_ink(&pm, 90, 50), "ArrowEquilateral: does not extend past tip at x = 90");

        // 3. TriangleFilled: reversed filled triangle with wide base at x = 80 (y = 50 +- 11.55) tapering to x = 60 (y = 50 +- 2).
        let pm = render_cap(StrokeCap::TriangleFilled, as_line, end_only);
        assert!(is_ink(&pm, 78, 41) && is_ink(&pm, 78, 59), "TriangleFilled: wide front edge at x = 78, y = 41/59");
        assert!(!is_ink(&pm, 62, 41) && !is_ink(&pm, 62, 59), "TriangleFilled: narrow back waist at x = 62");

        // 4. DiamondFilled: filled diamond centered at (80, 50) with half-diagonal ~11.55.
        let pm = render_cap(StrokeCap::DiamondFilled, as_line, end_only);
        assert!(is_ink(&pm, 80, 40) && is_ink(&pm, 80, 60) && is_ink(&pm, 89, 50), "DiamondFilled: top/bottom/front vertices inked");
        assert!(!is_ink(&pm, 88, 42), "DiamondFilled: outside diamond diagonal edge at (88, 42) is white");

        // 5. CircleFilled: filled circle centered at (80, 50) with radius ~10.67.
        let pm = render_cap(StrokeCap::CircleFilled, as_line, end_only);
        assert!(is_ink(&pm, 80, 41) && is_ink(&pm, 80, 59) && is_ink(&pm, 89, 50), "CircleFilled: circle interior inked");
        assert!(is_ink(&pm, 86, 44), "CircleFilled: circle bulge at (86, 44) inked");
        assert!(!is_ink(&pm, 90, 40), "CircleFilled: outside circle corner (90, 40) is white");
    }

    // Also test vertical, diagonal, and cubic-curved open vectors with ArrowEquilateral.
    for cmds in [
        vec![PathCmd::MoveTo(Vec2::new(50.0, 20.0)), PathCmd::LineTo(Vec2::new(50.0, 80.0))],
        vec![PathCmd::MoveTo(Vec2::new(20.0, 20.0)), PathCmd::LineTo(Vec2::new(80.0, 80.0))],
        vec![PathCmd::MoveTo(Vec2::new(20.0, 60.0)), PathCmd::CubicTo(Vec2::new(40.0, 20.0), Vec2::new(60.0, 20.0), Vec2::new(80.0, 60.0))],
    ] {
        let p = Path { cmds, fill_rule: FillRule::NonZero };
        let net = VectorNetwork::from_paths(std::slice::from_ref(&p));
        let mut t_arrow = T::new();
        t_arrow.add(None, |id| {
            let mut n = Node::new(id, NodeKind::Vector { paths: vec![p.clone()], network: Some(net.clone()) });
            n.size = Vec2::new(100.0, 100.0);
            n.stroke = Stroke {
                paints: vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))],
                weight: 4.0,
                cap: StrokeCap::ArrowEquilateral,
                join: StrokeJoin::Round,
                ..Stroke::default()
            };
            n
        });
        let mut t_none = T::new();
        t_none.add(None, |id| {
            let mut n = Node::new(id, NodeKind::Vector { paths: vec![p.clone()], network: Some(net) });
            n.size = Vec2::new(100.0, 100.0);
            n.stroke = Stroke {
                paints: vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))],
                weight: 4.0,
                cap: StrokeCap::None,
                join: StrokeJoin::Round,
                ..Stroke::default()
            };
            n
        });
        let ink_count = |pm: &Pixmap| pm.pixels().iter().filter(|p| p.red() < 128).count();
        assert!(
            ink_count(&t_arrow.render(100, 100)) > ink_count(&t_none.render(100, 100)) + 80,
            "ArrowEquilateral adds arrowhead ink on vertical/diagonal/curved open subpaths"
        );
    }
}

fn figma_arrow_lines_bake(len: f32) -> Path {
    Path {
        cmds: vec![
            PathCmd::MoveTo(Vec2::new(-0.35355338, -0.35355338)),
            PathCmd::CubicTo(Vec2::new(-0.54881555, -0.15829125), Vec2::new(-0.54881555, 0.15829125), Vec2::new(-0.35355338, 0.35355338)),
            PathCmd::LineTo(Vec2::new(2.828427, 3.535534)),
            PathCmd::CubicTo(Vec2::new(3.0236893, 3.730796), Vec2::new(3.3402717, 3.730796), Vec2::new(3.535534, 3.535534)),
            PathCmd::CubicTo(Vec2::new(3.730796, 3.3402717), Vec2::new(3.730796, 3.0236893), Vec2::new(3.535534, 2.828427)),
            PathCmd::LineTo(Vec2::new(0.70710677, 0.0)),
            PathCmd::LineTo(Vec2::new(3.535534, -2.828427)),
            PathCmd::CubicTo(Vec2::new(3.730796, -3.0236893), Vec2::new(3.730796, -3.3402717), Vec2::new(3.535534, -3.535534)),
            PathCmd::CubicTo(Vec2::new(3.3402717, -3.730796), Vec2::new(3.0236893, -3.730796), Vec2::new(2.828427, -3.535534)),
            PathCmd::LineTo(Vec2::new(-0.35355338, -0.35355338)),
            PathCmd::Close,
            PathCmd::MoveTo(Vec2::new(len + 0.35355338, 0.35355338)),
            PathCmd::CubicTo(Vec2::new(len + 0.54881555, 0.15829125), Vec2::new(len + 0.54881555, -0.15829125), Vec2::new(len + 0.35355338, -0.35355338)),
            PathCmd::LineTo(Vec2::new(len - 2.828427, -3.535534)),
            PathCmd::CubicTo(Vec2::new(len - 3.0236893, -3.730796), Vec2::new(len - 3.3402717, -3.730796), Vec2::new(len - 3.535534, -3.535534)),
            PathCmd::CubicTo(Vec2::new(len - 3.730796, -3.3402717), Vec2::new(len - 3.730796, -3.0236893), Vec2::new(len - 3.535534, -2.828427)),
            PathCmd::LineTo(Vec2::new(len - 0.70710677, 0.0)),
            PathCmd::LineTo(Vec2::new(len - 3.535534, 2.828427)),
            PathCmd::CubicTo(Vec2::new(len - 3.730796, 3.0236893), Vec2::new(len - 3.730796, 3.3402717), Vec2::new(len - 3.535534, 3.535534)),
            PathCmd::CubicTo(Vec2::new(len - 3.3402717, 3.730796), Vec2::new(len - 3.0236893, 3.730796), Vec2::new(len - 2.828427, 3.535534)),
            PathCmd::LineTo(Vec2::new(len + 0.35355338, 0.35355338)),
            PathCmd::Close,
            PathCmd::MoveTo(Vec2::new(0.0, 0.5)),
            PathCmd::LineTo(Vec2::new(len, 0.5)),
            PathCmd::LineTo(Vec2::new(len, -0.5)),
            PathCmd::LineTo(Vec2::new(0.0, -0.5)),
            PathCmd::LineTo(Vec2::new(0.0, 0.5)),
            PathCmd::Close,
        ],
        fill_rule: FillRule::NonZero,
    }
}

#[test]
fn kit_arrow_vectors_match_figma_bake_at_1x_and_keep_stroke_weight_head_size_when_resized_2x() {
    // Exact geometry and Figma baked strokeGeometry of the 3 named kit ARROW_LINES vectors:
    // 1. 1143:83664 (NodeId(111958)): (0, 0) -> (80, 0), size=(80, 0), w=1.0, join=Miter, cap=ArrowLines
    // 2. 8222:316128 (NodeId(112139)): (0, 0) -> (23.999998, 0), size=(23.999998, 0), w=1.0, join=Miter, cap=ArrowLines
    // 3. 7749:50847 (NodeId(113459)): (0, 0) -> (23.999998, 0), size=(23.999998, 0), w=1.0, join=Miter, cap=ArrowLines
    for (name, len) in [("1143:83664", 80.0_f32), ("8222:316128", 23.999998_f32), ("7749:50847", 23.999998_f32)] {
        let seg = Path { cmds: vec![PathCmd::MoveTo(Vec2::new(0.0, 0.0)), PathCmd::LineTo(Vec2::new(len, 0.0))], fill_rule: FillRule::NonZero };
        let net = VectorNetwork::from_paths(std::slice::from_ref(&seg));
        let stroke = Stroke {
            paints: vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))],
            weight: 1.0,
            align: StrokeAlign::Center,
            cap: StrokeCap::ArrowLines,
            join: StrokeJoin::Miter,
            ..Stroke::default()
        };
        let figma_bake = figma_arrow_lines_bake(len);

        // 1x comparison: Figma's baked strokeGeometry vs our live-drawn stroke (stroke_geometry = None).
        let mut t_bake = T::new();
        let id = t_bake.add(None, |id| {
            let mut n = Node::new(id, NodeKind::Vector { paths: vec![seg.clone()], network: Some(net.clone()) });
            n.transform = Affine::translate(12.0, 20.0);
            n.size = Vec2::new(len, 0.0);
            n.stroke = stroke.clone();
            n.stroke_geometry = Some(vec![figma_bake]);
            n
        });
        let mut t_live = T::new();
        t_live.add(None, |id| {
            let mut n = Node::new(id, NodeKind::Vector { paths: vec![seg.clone()], network: Some(net.clone()) });
            n.transform = Affine::translate(12.0, 20.0);
            n.size = Vec2::new(len, 0.0);
            n.stroke = stroke.clone();
            n
        });
        let pm_bake_1x = t_bake.render(200, 40);
        let pm_live_1x = t_live.render(200, 40);
        let mismatched_1x = pm_bake_1x
            .pixels()
            .iter()
            .zip(pm_live_1x.pixels().iter())
            .filter(|(a, b)| a.red().abs_diff(b.red()) > 2 || a.green().abs_diff(b.green()) > 2 || a.blue().abs_diff(b.blue()) > 2)
            .count();
        assert_eq!(mismatched_1x, 0, "{name}: live-drawn 1x head matches Figma's baked strokeGeometry with 0 mismatched pixels");

        // 2x resize: drops the baked stroke_geometry and keeps head size relative to stroke weight = 1.0.
        plain_resize(&mut t_bake.doc, id, Vec2::new(len * 2.0, 0.0));
        assert!(t_bake.doc.get(id).unwrap().stroke_geometry.is_none(), "{name}: drops baked stroke_geometry on 2x resize");

        let seg_2x = Path { cmds: vec![PathCmd::MoveTo(Vec2::new(0.0, 0.0)), PathCmd::LineTo(Vec2::new(len * 2.0, 0.0))], fill_rule: FillRule::NonZero };
        let net_2x = VectorNetwork::from_paths(std::slice::from_ref(&seg_2x));
        let mut t_fresh_2x = T::new();
        t_fresh_2x.add(None, |id| {
            let mut n = Node::new(id, NodeKind::Vector { paths: vec![seg_2x], network: Some(net_2x) });
            n.transform = Affine::translate(12.0, 20.0);
            n.size = Vec2::new(len * 2.0, 0.0);
            n.stroke = stroke;
            n
        });
        assert_eq!(
            t_bake.render(200, 40).data(),
            t_fresh_2x.render(200, 40).data(),
            "{name}: resized 2x kit arrow vector renders identical pixels to a fresh 2x vector with 1.0px stroke weight and unscaled head"
        );
    }
}

#[test]
fn resized_2x_arrow_caps_keep_head_size_relative_to_stroke_weight_for_all_five_caps() {
    for cap in [StrokeCap::ArrowLines, StrokeCap::ArrowEquilateral, StrokeCap::TriangleFilled, StrokeCap::DiamondFilled, StrokeCap::CircleFilled] {
        let seg = Path { cmds: vec![PathCmd::MoveTo(Vec2::new(4.0, 15.0)), PathCmd::LineTo(Vec2::new(26.0, 15.0))], fill_rule: FillRule::NonZero };
        let net = VectorNetwork::from_paths(std::slice::from_ref(&seg));
        let stroke = Stroke {
            paints: vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))],
            weight: 2.0,
            align: StrokeAlign::Center,
            cap,
            join: StrokeJoin::Round,
            ..Stroke::default()
        };
        // Simulate an imported vector whose stroke_geometry has the 1x head baked in.
        let bake = designos_render::vectorops::outline_stroke(&net.to_paths(), &stroke, Some(&net));
        let mut t = T::new();
        let id = t.add(None, |id| {
            let mut n = Node::new(id, NodeKind::Vector { paths: vec![seg], network: Some(net) });
            n.transform = Affine::translate(20.0, 20.0);
            n.size = Vec2::new(30.0, 30.0);
            n.stroke = stroke.clone();
            n.stroke_geometry = Some(bake);
            n
        });
        plain_resize(&mut t.doc, id, Vec2::new(60.0, 60.0));
        assert!(t.doc.get(id).unwrap().stroke_geometry.is_none(), "{cap:?}: baked stroke_geometry dropped on 2x resize");

        let pm = t.render(120, 120);
        // After 2x resize, the segment is at y = 20 + 30 = 50, from x = 28 to x = 72, with stroke weight = 2.0.
        // At weight = 2.0, the arrowhead half-width is at most 5 / sqrt(3) * 2.0 = 5.77px (so y = 50 +- 6),
        // whereas a 2x-scaled bake would reach y = 50 +- 12 (y = 39 and y = 61).
        assert!((62..=74).any(|x| is_ink(&pm, x, 46)), "{cap:?}: head ink is present at 1x head span (y = 46)");
        assert!(!(60..=76).any(|x| is_ink(&pm, x, 39)), "{cap:?}: head does NOT scale 2x to y = 39");
    }
}

#[test]
fn per_vertex_round_and_square_caps_paint_full_span() {
    let seg = Path { cmds: vec![PathCmd::MoveTo(Vec2::new(10.0, 20.0)), PathCmd::LineTo(Vec2::new(60.0, 20.0))], fill_rule: FillRule::NonZero };
    for cap in [StrokeCap::Square, StrokeCap::Round] {
        let mut net = VectorNetwork::from_paths(std::slice::from_ref(&seg));
        for v in &mut net.vertices {
            v.cap = Some(cap);
        }
        let mut t = T::new();
        t.add(None, |id| {
            let mut n = Node::new(id, NodeKind::Vector { paths: net.to_paths(), network: Some(net) });
            n.size = Vec2::new(80.0, 40.0);
            n.stroke = Stroke {
                paints: vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))],
                weight: 12.0,
                align: StrokeAlign::Center,
                cap: StrokeCap::None,
                join: StrokeJoin::Round,
                ..Stroke::default()
            };
            n
        });
        let pm = t.render(80, 40);
        // Open line (10, 20)->(60, 20), weight 12 (r = 6), node cap None, both vertices `cap`:
        // start cap covers x = 4..10 and end cap covers x = 60..66, so y = 20 is painted across x = 4..66 (62px).
        assert_eq!(horizontal_ink_runs(&pm, 20, 0..80), vec![62], "{cap:?}: paints x = 4..66 (62px run) on y = 20");
        assert!(is_ink(&pm, 5, 20) && is_ink(&pm, 64, 20), "{cap:?}: both start (x=5) and end (x=64) caps are painted outward");
    }
}

#[test]
fn closed_subpath_and_interior_vertex_draw_no_arrowheads_and_scene_index_bounds_include_arrowheads() {
    // 1. Closed subpath with node arrow cap draws no arrowhead (identical to StrokeCap::None).
    let closed = Path { cmds: rect_subpath(20.0, 20.0, 60.0, 60.0, false), fill_rule: FillRule::NonZero };
    let net_closed = VectorNetwork::from_paths(std::slice::from_ref(&closed));
    let render_closed = |cap: StrokeCap| {
        let mut t = T::new();
        t.add(None, |id| {
            let mut n = Node::new(id, NodeKind::Vector { paths: vec![closed.clone()], network: Some(net_closed.clone()) });
            n.size = Vec2::new(80.0, 80.0);
            n.stroke = Stroke { paints: vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))], weight: 4.0, cap, join: StrokeJoin::Miter, ..Stroke::default() };
            n
        });
        t.render(80, 80)
    };
    assert_eq!(
        render_closed(StrokeCap::ArrowEquilateral).data(),
        render_closed(StrokeCap::None).data(),
        "closed subpath with ArrowEquilateral draws no arrowhead"
    );

    // 2. Interior vertex (degree 2) with arrow cap draws no arrowhead.
    let bent = Path {
        cmds: vec![PathCmd::MoveTo(Vec2::new(10.0, 40.0)), PathCmd::LineTo(Vec2::new(40.0, 40.0)), PathCmd::LineTo(Vec2::new(70.0, 40.0))],
        fill_rule: FillRule::NonZero,
    };
    let mut net_interior = VectorNetwork::from_paths(std::slice::from_ref(&bent));
    for v in &mut net_interior.vertices {
        if (v.pos.x - 40.0).abs() < 1e-3 {
            v.cap = Some(StrokeCap::DiamondFilled);
        } else {
            v.cap = Some(StrokeCap::None);
        }
    }
    let net_plain = VectorNetwork::from_paths(std::slice::from_ref(&bent));
    let render_bent = |net: VectorNetwork| {
        let mut t = T::new();
        t.add(None, |id| {
            let mut n = Node::new(id, NodeKind::Vector { paths: vec![bent.clone()], network: Some(net) });
            n.size = Vec2::new(80.0, 80.0);
            n.stroke = Stroke {
                paints: vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))],
                weight: 4.0,
                cap: StrokeCap::None,
                join: StrokeJoin::Round,
                ..Stroke::default()
            };
            n
        });
        t.render(80, 80)
    };
    assert_eq!(render_bent(net_interior).data(), render_bent(net_plain).data(), "interior vertex (degree 2) with DiamondFilled draws no arrowhead");

    // 3. SceneIndex bounds include the arrowhead overhang (up to 6 * weight).
    let mut t = T::new();
    let line_id = t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Line);
        n.transform = Affine::translate(20.0, 10.0);
        n.size = Vec2::new(60.0, 0.0);
        n.stroke = Stroke { paints: vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))], weight: 4.0, cap: StrokeCap::ArrowEquilateral, ..Stroke::default() };
        n
    });
    let mut idx = designos_render::SceneIndex::new();
    idx.ensure(&t.doc, t.page, 1);
    let mut hits = Vec::new();
    idx.query_rect((75.0, 20.0, 85.0, 24.0), &mut hits);
    assert!(hits.contains(&line_id), "SceneIndex query in the arrowhead wing region (y = 20..24, 10px away from y = 10 line) includes the arrow-capped line");
}

/// How the end caps of an open vector are set.
#[derive(Clone, Copy, Debug)]
enum CapSetup {
    /// `Stroke::cap` of the node, both ends.
    Node(StrokeCap),
    /// The cap of the first and of the last vertex, `Stroke::cap` stays `None`.
    Vertices(StrokeCap, StrokeCap),
}

impl CapSetup {
    fn ends(self) -> (StrokeCap, StrokeCap) {
        match self {
            CapSetup::Node(c) => (c, c),
            CapSetup::Vertices(a, b) => (a, b),
        }
    }
}

/// An open vector along `cmds`, stroked at `weight` with `join` and the caps of `setup`.
fn open_vector_with_caps(t: &mut T, cmds: Vec<PathCmd>, setup: CapSetup, weight: f32, join: StrokeJoin) -> NodeId {
    let path = Path { cmds, fill_rule: FillRule::NonZero };
    let mut net = VectorNetwork::from_paths(std::slice::from_ref(&path));
    if let CapSetup::Vertices(a, b) = setup {
        let last = net.vertices.len() - 1;
        net.vertices[0].cap = Some(a);
        net.vertices[last].cap = Some(b);
    }
    t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Vector { paths: vec![path], network: Some(net) });
        n.size = Vec2::new(240.0, 120.0);
        n.fills.clear();
        n.stroke = Stroke {
            paints: vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))],
            weight,
            align: StrokeAlign::Center,
            cap: if let CapSetup::Node(c) = setup { c } else { StrokeCap::None },
            join,
            ..Stroke::default()
        };
        n
    })
}

/// Points along the centerline of `cmds`, about a pixel apart.
fn centerline_points(cmds: &[PathCmd]) -> Vec<Vec2> {
    let mut pts = vec![];
    let mut at = Vec2::ZERO;
    for cmd in cmds {
        match *cmd {
            PathCmd::MoveTo(p) => {
                at = p;
                pts.push(p);
            }
            PathCmd::LineTo(p) => {
                let n = ((p.x - at.x).hypot(p.y - at.y) * 2.0).ceil().max(1.0) as usize;
                pts.extend((1..=n).map(|i| Vec2::new(at.x + (p.x - at.x) * i as f32 / n as f32, at.y + (p.y - at.y) * i as f32 / n as f32)));
                at = p;
            }
            PathCmd::CubicTo(c1, c2, p) => {
                let n = 400;
                pts.extend((1..=n).map(|i| {
                    let (t, u) = (i as f32 / n as f32, 1.0 - i as f32 / n as f32);
                    let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
                    Vec2::new(w[0] * at.x + w[1] * c1.x + w[2] * c2.x + w[3] * p.x, w[0] * at.y + w[1] * c1.y + w[2] * c2.y + w[3] * p.y)
                }));
                at = p;
            }
            _ => {}
        }
    }
    pts
}

/// Centerline points whose pixel holds no ink, leaving out the end of the line: 3px at a head (its tip is
/// thin) and 1.5px at a butt end (the last pixel is only partly covered).
fn unpainted_centerline_points(pm: &Pixmap, cmds: &[PathCmd], setup: CapSetup) -> Vec<(u32, u32)> {
    let pts = centerline_points(cmds);
    let (first, last) = (pts[0], pts[pts.len() - 1]);
    let (start_cap, end_cap) = setup.ends();
    let skip = |cap: StrokeCap| if cap == StrokeCap::None { 1.5 } else { 3.0 };
    let near = |p: Vec2, q: Vec2, within: f32| (p.x - q.x).hypot(p.y - q.y) < within;
    pts.iter()
        .filter(|p| !near(**p, first, skip(start_cap)) && !near(**p, last, skip(end_cap)))
        .map(|p| (p.x.floor() as u32, p.y.floor() as u32))
        .filter(|(x, y)| !is_ink(pm, *x, *y))
        .collect()
}

#[test]
fn a_filled_arrow_head_overlaps_its_shaft_with_no_gap_on_straight_and_curved_segments() {
    let line = vec![PathCmd::MoveTo(Vec2::new(10.0, 60.5)), PathCmd::LineTo(Vec2::new(210.0, 60.5))];
    let cubic = vec![PathCmd::MoveTo(Vec2::new(20.0, 100.0)), PathCmd::CubicTo(Vec2::new(60.0, 20.0), Vec2::new(180.0, 20.0), Vec2::new(220.0, 100.0))];
    // a curve whose last piece is a straight run
    let curve_then_line = vec![
        PathCmd::MoveTo(Vec2::new(20.0, 100.0)),
        PathCmd::CubicTo(Vec2::new(40.0, 30.0), Vec2::new(80.0, 30.0), Vec2::new(110.0, 70.0)),
        PathCmd::LineTo(Vec2::new(230.0, 70.0)),
    ];
    let line_then_curve = vec![
        PathCmd::MoveTo(Vec2::new(10.0, 70.0)),
        PathCmd::LineTo(Vec2::new(130.0, 70.0)),
        PathCmd::CubicTo(Vec2::new(170.0, 30.0), Vec2::new(200.0, 30.0), Vec2::new(225.0, 100.0)),
    ];
    let mut failed = vec![];
    for (shape, cmds) in [("straight", line), ("cubic", cubic), ("curve then line", curve_then_line), ("line then curve", line_then_curve)] {
        for cap in [StrokeCap::ArrowEquilateral, StrokeCap::TriangleFilled] {
            for setup in [CapSetup::Node(cap), CapSetup::Vertices(StrokeCap::None, cap), CapSetup::Vertices(cap, StrokeCap::None), CapSetup::Vertices(cap, cap)]
            {
                let mut t = T::new();
                open_vector_with_caps(&mut t, cmds.clone(), setup, 4.0, StrokeJoin::Round);
                let gaps = unpainted_centerline_points(&t.render(240, 120), &cmds, setup);
                if !gaps.is_empty() {
                    failed.push(format!("{shape} {setup:?}: {} unpainted centerline pixels, first {:?}, last {:?}", gaps.len(), gaps[0], gaps[gaps.len() - 1]));
                }
            }
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// An open `(4, 15) -> (26, 15)` vector in a 30x30 box at (20, 20), stroked at weight 2 with `cap`, `align` and `dashes`,
/// carrying the outline Figma baked for it.
fn imported_open_arrow_vector(t: &mut T, cap: StrokeCap, align: StrokeAlign, dashes: Vec<f32>) -> NodeId {
    let seg = Path { cmds: vec![PathCmd::MoveTo(Vec2::new(4.0, 15.0)), PathCmd::LineTo(Vec2::new(26.0, 15.0))], fill_rule: FillRule::NonZero };
    let net = VectorNetwork::from_paths(std::slice::from_ref(&seg));
    let stroke =
        Stroke { paints: vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))], weight: 2.0, align, cap, join: StrokeJoin::Round, dashes, ..Stroke::default() };
    let bake = designos_render::vectorops::outline_stroke(&net.to_paths(), &stroke, Some(&net));
    t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Vector { paths: vec![seg], network: Some(net) });
        n.transform = Affine::translate(20.0, 20.0);
        n.size = Vec2::new(30.0, 30.0);
        n.stroke = stroke;
        n.stroke_geometry = Some(bake);
        n
    })
}

/// Columns, counted back from the last column that has ink on the rows `y_rows`, to the first column holding the
/// widest vertical run of ink in `x_range`: the length of an arrow head whose base is its widest part.
fn head_length_to_base(pm: &Pixmap, y_rows: std::ops::Range<u32>, x_range: std::ops::Range<u32>) -> u32 {
    let tip = x_range.clone().rev().find(|x| y_rows.clone().any(|y| is_ink(pm, *x, y))).map_or(0, |x| x + 1);
    let widest = x_range.clone().map(|x| vertical_ink_runs(pm, x, 0..pm.height()).into_iter().max().unwrap_or(0)).max().unwrap_or(0);
    let base = x_range.into_iter().find(|x| vertical_ink_runs(pm, *x, 0..pm.height()).into_iter().max().unwrap_or(0) == widest).unwrap_or(tip);
    tip - base
}

#[test]
fn an_inside_or_outside_open_arrow_stroke_drops_its_bake_on_a_resize_and_keeps_its_head_at_five_weights() {
    let mut failed = vec![];
    for align in [StrokeAlign::Inside, StrokeAlign::Outside] {
        for cap in [StrokeCap::ArrowLines, StrokeCap::ArrowEquilateral, StrokeCap::TriangleFilled, StrokeCap::DiamondFilled, StrokeCap::CircleFilled] {
            let mut t = T::new();
            let id = imported_open_arrow_vector(&mut t, cap, align, vec![]);
            plain_resize(&mut t.doc, id, Vec2::new(60.0, 60.0));
            if t.doc.get(id).unwrap().stroke_geometry.is_some() {
                failed.push(format!("{cap:?} {align:?}: the baked outline is kept on a 2x resize"));
            }
        }
        let mut t = T::new();
        let id = imported_open_arrow_vector(&mut t, StrokeCap::ArrowEquilateral, align, vec![]);
        plain_resize(&mut t.doc, id, Vec2::new(60.0, 60.0));
        // the line runs x = 28..72 at y = 50 and the head at its end is 5 * 2 = 10px long, not the 20px of a 2x scaled bake
        let length = head_length_to_base(&t.render(120, 100), 49..51, 45..90);
        if !(9..=11).contains(&length) {
            failed.push(format!("ArrowEquilateral {align:?}: the head is {length}px long after the resize, expected 10"));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

#[test]
fn a_dashed_centered_arrow_stroke_keeps_its_scaled_bake_on_a_resize() {
    let mut t = T::new();
    let id = imported_open_arrow_vector(&mut t, StrokeCap::ArrowEquilateral, StrokeAlign::Center, vec![4.0, 2.0]);
    let extent = |paths: &[Path]| {
        let xs: Vec<f32> = paths
            .iter()
            .flat_map(|p| p.cmds.iter())
            .filter_map(|c| if let PathCmd::LineTo(p) | PathCmd::MoveTo(p) | PathCmd::CubicTo(_, _, p) = c { Some(p.x) } else { None })
            .collect();
        xs.iter().cloned().fold(f32::MIN, f32::max) - xs.iter().cloned().fold(f32::MAX, f32::min)
    };
    let before = extent(t.doc.get(id).unwrap().stroke_geometry.as_ref().unwrap());
    plain_resize(&mut t.doc, id, Vec2::new(60.0, 60.0));
    let after = extent(t.doc.get(id).unwrap().stroke_geometry.as_ref().expect("a dashed arrow keeps its baked outline"));
    assert!((after - 2.0 * before).abs() < 1e-3, "the kept outline is scaled with the shape: {before} -> {after}");
}

/// Ink in the square of `half` px around `(cx, cy)`, as a sum of pixel coverage (the stroke colour has red 0x18).
fn ink_amount(pm: &Pixmap, cx: f32, cy: f32, half: f32) -> f32 {
    let (x0, x1, y0, y1) = ((cx - half).floor() as u32, (cx + half).ceil() as u32, (cy - half).floor() as u32, (cy + half).ceil() as u32);
    (y0..y1).flat_map(|y| (x0..x1).map(move |x| (x, y))).map(|(x, y)| (255.0 - px(pm, x, y)[0] as f32) / (255.0 - 0x18 as f32)).sum()
}

/// An open `(20, 50.5) -> (end_x, 50.5)` vector of weight 9 whose last vertex carries `cap`, stroked with `join`.
fn heavy_arrow_line(t: &mut T, end_x: f32, cap: StrokeCap, join: StrokeJoin) {
    let cmds = vec![PathCmd::MoveTo(Vec2::new(20.0, 50.5)), PathCmd::LineTo(Vec2::new(end_x, 50.5))];
    open_vector_with_caps(t, cmds, CapSetup::Vertices(StrokeCap::None, cap), 9.0, join);
}

/// The area a circle of radius `r` takes off a corner of interior angle `angle` (radians).
fn rounded_off_area(r: f32, angle: f32) -> f32 {
    r * r * (1.0 / (angle / 2.0).tan() - (std::f32::consts::PI - angle) / 2.0)
}

#[test]
fn a_round_join_rounds_the_corners_of_the_filled_heads_and_any_other_join_keeps_them_sharp() {
    let w = 9.0_f32;
    let hw = 5.0 / 3.0_f32.sqrt() * w;
    let r = w / 3.0;
    let render = |end_x: f32, cap: StrokeCap, join: StrokeJoin| {
        let mut t = T::new();
        heavy_arrow_line(&mut t, end_x, cap, join);
        t.render(170, 100)
    };
    let mut failed = vec![];
    let mut check = |what: &str, ok: bool| {
        if !ok {
            failed.push(what.to_string());
        }
    };

    // ArrowEquilateral: tip at (120, 50.5); base corners at (75, 50.5 -+ hw); every corner is 60 degrees
    let (sharp, round) = (render(120.0, StrokeCap::ArrowEquilateral, StrokeJoin::Miter), render(120.0, StrokeCap::ArrowEquilateral, StrokeJoin::Round));
    // the pixel 1.5px inside the tip: painted by a sharp tip, outside the fillet (it starts 3px in) of a round one
    check("equilateral, Miter: the pixel at the sharp tip is painted", is_ink(&sharp, 118, 50));
    check("equilateral, Round: the pixel at the sharp tip is unpainted", !is_ink(&round, 118, 50));
    check("equilateral, Round: the pixel inside the fillet, 4.5px from the tip, is painted", is_ink(&round, 115, 50));
    for (name, (cx, cy)) in [("tip", (120.0, 50.5)), ("upper base corner", (75.0, 50.5 - hw)), ("lower base corner", (75.0, 50.5 + hw))] {
        let cut = ink_amount(&sharp, cx, cy, 7.0) - ink_amount(&round, cx, cy, 7.0);
        let want = rounded_off_area(r, std::f32::consts::FRAC_PI_3);
        check(&format!("equilateral {name}: Round takes {cut:.2}px of ink off the corner, expected {want:.2}"), (cut - want).abs() < 0.3 * want);
    }
    let other = render(120.0, StrokeCap::ArrowEquilateral, StrokeJoin::Bevel);
    check("equilateral, Bevel: the head stays as sharp as with Miter", sharp.data() == other.data());

    // DiamondFilled: front tip at x = 145.5, corners at 90 degrees; the fillet starts 1.24px in
    let end_x = 145.5 - hw;
    let (sharp, round) = (render(end_x, StrokeCap::DiamondFilled, StrokeJoin::Miter), render(end_x, StrokeCap::DiamondFilled, StrokeJoin::Round));
    check("diamond, Miter: the pixel at the sharp front corner is painted", is_ink(&sharp, 144, 50));
    check("diamond, Round: the pixel at the sharp front corner is unpainted", !is_ink(&round, 144, 50));
    check("diamond, Round: the pixel inside the fillet, 3px from the corner, is painted", is_ink(&round, 142, 50));
    // the back corner lies on the shaft, which covers the part that rounding takes off
    for (name, (cx, cy)) in [("front", (end_x + hw, 50.5)), ("top", (end_x, 50.5 - hw)), ("bottom", (end_x, 50.5 + hw))] {
        let cut = ink_amount(&sharp, cx, cy, 5.0) - ink_amount(&round, cx, cy, 5.0);
        let want = rounded_off_area(r, std::f32::consts::FRAC_PI_2);
        check(&format!("diamond {name} corner: Round takes {cut:.2}px of ink off the corner, expected {want:.2}"), (cut - want).abs() < 0.35 * want);
    }

    // TriangleFilled: the two outer corners at (120, 50.5 -+ hw) are rounded; the corners at the shaft are not
    let (sharp, round) = (render(120.0, StrokeCap::TriangleFilled, StrokeJoin::Miter), render(120.0, StrokeCap::TriangleFilled, StrokeJoin::Round));
    let side = ((hw - 0.5 * w).powi(2) + (5.0 * w).powi(2)).sqrt();
    let want = rounded_off_area(r, ((hw - 0.5 * w) / side).acos());
    for (name, (cx, cy)) in [("upper outer corner", (120.0, 50.5 - hw)), ("lower outer corner", (120.0, 50.5 + hw))] {
        let cut = ink_amount(&sharp, cx, cy, 7.0) - ink_amount(&round, cx, cy, 7.0);
        check(&format!("triangle {name}: Round takes {cut:.2}px of ink off the corner, expected {want:.2}"), (cut - want).abs() < 0.3 * want);
    }
    for (name, cy) in [("upper waist corner", 50.5 - 0.5 * w), ("lower waist corner", 50.5 + 0.5 * w)] {
        let cut = ink_amount(&sharp, 120.0 - 5.0 * w, cy, 3.0) - ink_amount(&round, 120.0 - 5.0 * w, cy, 3.0);
        check(&format!("triangle {name}: Round takes {cut:.2}px of ink off, expected none"), cut.abs() < 0.3);
    }
    check("triangle, Bevel: the head stays as sharp as with Miter", sharp.data() == render(120.0, StrokeCap::TriangleFilled, StrokeJoin::Bevel).data());
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}
