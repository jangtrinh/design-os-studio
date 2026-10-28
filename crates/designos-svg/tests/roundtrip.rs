//! designos doc -> `export_svg` -> `import_svg`: node kinds and geometry survive,
//! and the re-imported document renders like the original and like resvg
//! drawing the exported markup.

use designos_doc::*;
use designos_export::{export_svg, SvgOptions};
use designos_svg::import_svg_named;
use std::f32::consts::TAU;

const TOLERANCE: i32 = 8;
const MAX_MISMATCH: f64 = 0.02;

fn solid(r: u8, g: u8, b: u8) -> Paint {
    Paint::solid(Color::rgb(r, g, b))
}

fn add(d: &mut Document, parent: NodeId, mut n: Node, name: &str) -> NodeId {
    n.name = name.into();
    d.insert(parent, n).unwrap()
}

fn sample() -> (Document, NodeId) {
    let mut d = Document::new("rt");
    let page = d.add_page("p");
    let id = d.alloc_id();
    let mut frame = Node::frame(id, Rect::new(0.0, 0.0, 220.0, 160.0));
    if let Some(f) = frame.kind.frame_props_mut() {
        f.clips_content = true;
    }
    let root = add(&mut d, page, frame, "Root");

    let id = d.alloc_id();
    let mut button = Node::rectangle(id, Rect::new(16.0, 16.0, 96.0, 36.0));
    button.corner_radii = [10.0; 4];
    button.fills = vec![solid(79, 70, 229)];
    button.effects = vec![Effect::DropShadow {
        color: Color::BLACK.with_alpha(0.3),
        offset: Vec2::new(0.0, 4.0),
        radius: 8.0,
        spread: 0.0,
        visible: true,
        show_behind: false,
        blend: Default::default(),
    }];
    add(&mut d, root, button, "Button");

    let id = d.alloc_id();
    let mut badge = Node::ellipse(id, Rect::new(130.0, 16.0, 64.0, 64.0));
    badge.fills = vec![Paint {
        kind: PaintKind::LinearGradient {
            handles: [Vec2::new(0.0, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.0, 1.5)],
            stops: vec![GradientStop { pos: 0.0, color: Color::rgb(239, 68, 68) }, GradientStop { pos: 1.0, color: Color::rgb(250, 204, 21) }],
        },
        opacity: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    }];
    badge.stroke = Stroke { paints: vec![solid(17, 24, 39)], weight: 3.0, align: StrokeAlign::Center, miter_limit: 4.0, ..Default::default() };
    add(&mut d, root, badge, "Badge");

    let id = d.alloc_id();
    let mut star = Node::new(id, NodeKind::Vector { paths: vec![], network: None });
    let pts: Vec<Vec2> = (0..10)
        .map(|i| {
            let a = -TAU / 4.0 + i as f32 * TAU / 10.0;
            let r = if i % 2 == 0 { 30.0 } else { 12.0 };
            Vec2::new(30.0 + r * a.cos(), 30.0 + r * a.sin())
        })
        .collect();
    // Local coordinates start at the path's own top-left, so size is its exact box.
    let (x0, x1) = pts.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.x), b.max(p.x)));
    let (y0, y1) = pts.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
    let pts: Vec<Vec2> = pts.iter().map(|p| Vec2::new(p.x - x0, p.y - y0)).collect();
    let mut cmds = vec![PathCmd::MoveTo(pts[0])];
    cmds.extend(pts[1..].iter().map(|p| PathCmd::LineTo(*p)));
    cmds.push(PathCmd::Close);
    star.kind = NodeKind::Vector { paths: vec![Path { cmds, fill_rule: FillRule::NonZero }], network: None };
    star.transform = Affine::translate(20.0, 76.0);
    star.size = Vec2::new(x1 - x0, y1 - y0);
    star.fills = vec![solid(34, 197, 94)];
    star.stroke = Stroke {
        paints: vec![solid(20, 83, 45)],
        weight: 2.0,
        align: StrokeAlign::Center,
        cap: StrokeCap::Round,
        join: StrokeJoin::Round,
        miter_limit: 4.0,
        dashes: vec![4.0, 2.0],
        ..Default::default()
    };
    add(&mut d, root, star, "Star");

    let id = d.alloc_id();
    let mut group = Node::new(id, NodeKind::Group);
    group.transform = Affine::translate(110.0, 92.0) * Affine::rotate(0.35);
    group.opacity = 0.6;
    group.size = Vec2::new(50.0, 30.0);
    let group = add(&mut d, root, group, "Pair");
    let id = d.alloc_id();
    let mut a = Node::rectangle(id, Rect::new(0.0, 0.0, 30.0, 20.0));
    a.fills = vec![solid(59, 130, 246)];
    add(&mut d, group, a, "A");
    let id = d.alloc_id();
    let mut b = Node::rectangle(id, Rect::new(20.0, 10.0, 30.0, 20.0));
    b.fills = vec![solid(236, 72, 153)];
    add(&mut d, group, b, "B");

    let id = d.alloc_id();
    let mut card = Node::rectangle(id, Rect::new(168.0, 100.0, 40.0, 44.0));
    card.fills = vec![solid(254, 243, 199)];
    card.stroke = Stroke { paints: vec![solid(180, 83, 9)], weight: 4.0, align: StrokeAlign::Inside, miter_limit: 4.0, ..Default::default() };
    add(&mut d, root, card, "Inside stroke");

    let id = d.alloc_id();
    let style = TextStyle { font_size: 16.0, fills: vec![solid(15, 23, 42)], ..Default::default() };
    let text = Node::text(id, Rect::new(16.0, 130.0, 90.0, 20.0), "Hi there", style);
    add(&mut d, root, text, "Greeting");
    (d, root)
}

fn kind_name(n: &Node) -> &'static str {
    match n.kind {
        NodeKind::Rectangle => "Rectangle",
        NodeKind::Ellipse(_) => "Ellipse",
        NodeKind::Vector { .. } => "Vector",
        NodeKind::Text(_) => "Text",
        _ => "container",
    }
}

/// (kind, world AABB x, y, w, h) of every drawn leaf, in paint order.
fn leaves(d: &Document, root: NodeId) -> Vec<(&'static str, String, [f32; 4])> {
    let mut out = Vec::new();
    d.walk(root, |n, _| {
        let k = kind_name(n);
        if k == "container" {
            return;
        }
        let w = d.world_transform(n.id);
        let corners = [Vec2::new(0.0, 0.0), Vec2::new(n.size.x, 0.0), Vec2::new(0.0, n.size.y), n.size].map(|p| w.apply(p));
        let (x0, x1) = corners.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.x), b.max(p.x)));
        let (y0, y1) = corners.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
        out.push((k, n.name.clone(), [x0, y0, x1 - x0, y1 - y0]));
    });
    out
}

fn roundtrip(d: &Document, root: NodeId) -> (String, Document, NodeId) {
    let svg = export_svg(d, root, &SvgOptions { outline_text: false });
    let mut back = Document::new("back");
    let page = back.add_page("p");
    let id = import_svg_named(&svg, "Root", &mut back, page, Vec2::ZERO).unwrap_or_else(|e| panic!("{e}\n{svg}"));
    (svg, back, id)
}

fn mismatch(a: &tiny_skia::Pixmap, b: &tiny_skia::Pixmap) -> f64 {
    assert_eq!((a.width(), a.height()), (b.width(), b.height()));
    let bad = a
        .data()
        .chunks_exact(4)
        .zip(b.data().chunks_exact(4))
        .filter(|(p, q)| p.iter().zip(q.iter()).any(|(x, y)| (*x as i32 - *y as i32).abs() > TOLERANCE))
        .count();
    bad as f64 / (a.width() * a.height()) as f64
}

#[test]
fn kinds_and_geometry_survive_the_roundtrip() {
    let (d, root) = sample();
    let (svg, back, back_root) = roundtrip(&d, root);
    let before = leaves(&d, root);
    let after = leaves(&back, back_root);
    // Every original leaf has a same-kind leaf at the same place.
    for (kind, name, r) in &before {
        let hit = after.iter().find(|(k, _, a)| {
            k == kind
                && if *kind == "Text" { (a[0] - r[0]).abs() < 0.6 && (a[1] - r[1]).abs() < 0.6 } else { a.iter().zip(r).all(|(x, y)| (x - y).abs() < 0.6) }
        });
        assert!(hit.is_some(), "{kind} \"{name}\" {r:?} lost; imported: {after:#?}\n{svg}");
    }
    // The only extras are the frame background and the clip's inner frames.
    assert!(after.len() <= before.len() + 3, "{after:#?}");

    // Details on the nodes that carry them.
    let find = |k: &str| back.nodes.values().find(|n| kind_name(n) == k && n.name != "Root").unwrap();
    let mut rect_radii = back.nodes.values().filter(|n| matches!(n.kind, NodeKind::Rectangle)).map(|n| n.corner_radii[0]).collect::<Vec<_>>();
    rect_radii.sort_by(f32::total_cmp);
    assert!(rect_radii.contains(&10.0), "button radius: {rect_radii:?}");
    let button = back.nodes.values().find(|n| matches!(n.kind, NodeKind::Rectangle) && n.corner_radii[0] == 10.0).unwrap();
    assert!(
        matches!(button.effects.first(), Some(Effect::DropShadow { radius, offset, show_behind: false, .. }) if (*radius - 8.0).abs() < 1e-3 && offset.y == 4.0),
        "{:?}",
        button.effects
    );
    let star = find("Vector");
    assert_eq!(star.stroke.dashes, vec![4.0, 2.0]);
    assert_eq!((star.stroke.cap, star.stroke.join), (StrokeCap::Round, StrokeJoin::Round));
    let NodeKind::Text(t) = &find("Text").kind else { unreachable!() };
    assert_eq!((t.content.as_str(), t.style.font_size), ("Hi there", 16.0));
    let badge = back.nodes.values().find(|n| matches!(n.kind, NodeKind::Ellipse(_))).unwrap();
    let PaintKind::LinearGradient { handles, stops } = &badge.fills[0].kind else { panic!("{:?}", badge.fills) };
    assert_eq!(stops.len(), 2);
    for (h, w) in handles.iter().zip([Vec2::new(0.0, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.0, 1.5)]) {
        assert!((h.x - w.x).abs() < 1e-3 && (h.y - w.y).abs() < 1e-3, "{handles:?}");
    }
    let pair = back.nodes.values().find(|n| matches!(n.kind, NodeKind::Group) && n.opacity < 1.0).expect("group");
    assert!((pair.opacity - 0.6).abs() < 1e-4, "{pair:?}\n{svg}");
    assert!((pair.transform.rotation_rad() - 0.35).abs() < 2e-3);
}

#[test]
fn reimported_document_renders_like_the_original_and_like_resvg() {
    let (d, root) = sample();
    let (svg, back, back_root) = roundtrip(&d, root);
    let original = designos_render::render_node(&d, root, 1.0).unwrap();
    let ours = designos_render::render_node(&back, back_root, 1.0).unwrap();

    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_str(&svg, &opt).unwrap();
    let size = tree.size().to_int_size();
    let mut reference = tiny_skia::Pixmap::new(size.width(), size.height()).unwrap();
    resvg::render(&tree, tiny_skia::Transform::identity(), &mut reference.as_mut());

    // The exported SVG has no text glyphs in `reference` (no fonts in this
    // resvg), so compare the text-free region of the canvas for that pair.
    let a = mismatch(&original, &ours);
    println!("original vs re-import: {:.3}%", a * 100.0);
    assert!(a < MAX_MISMATCH, "{:.3}%", a * 100.0);

    let mut ours_no_text = back.clone();
    let text_ids: Vec<_> = ours_no_text.nodes.values().filter(|n| matches!(n.kind, NodeKind::Text(_))).map(|n| n.id).collect();
    for id in text_ids {
        ours_no_text.get_mut(id).unwrap().visible = false;
    }
    let ours_no_text = designos_render::render_node(&ours_no_text, back_root, 1.0).unwrap();
    let b = mismatch(&reference, &ours_no_text);
    println!("resvg(export) vs re-import (no text): {:.3}%", b * 100.0);
    assert!(b < MAX_MISMATCH, "{:.3}%", b * 100.0);
}
