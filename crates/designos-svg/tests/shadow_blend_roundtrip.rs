//! End-to-end SVG round-trip and CPU render parity tests for shadow blend modes.

use designos_doc::*;
use designos_export::{export_svg, SvgOptions};
use designos_svg::import_svg_named;

const TOLERANCE: i32 = 8;
const MAX_MISMATCH: f64 = 0.02;

fn build_shadow_scene(drop_blend: BlendMode, inner_blend: BlendMode) -> (Document, NodeId) {
    let mut doc = Document::new("shadow-blend");
    let page = doc.add_page("Page 1");

    let root_id = doc.alloc_id();
    let mut root = Node::frame(root_id, Rect::new(0.0, 0.0, 160.0, 120.0));
    root.name = "Root".into();
    if let Some(f) = root.kind.frame_props_mut() {
        f.clips_content = true;
    }
    root.fills = vec![Paint::solid(Color::rgb(120, 200, 240))];
    let root_id = doc.insert(page, root).unwrap();

    let card_id = doc.alloc_id();
    let mut card = Node::rectangle(card_id, Rect::new(24.0, 20.0, 96.0, 64.0));
    card.name = "Card".into();
    card.corner_radii = [8.0; 4];
    card.fills = vec![Paint::solid(Color::rgb(240, 180, 80))];
    card.effects = vec![
        Effect::DropShadow {
            color: Color::rgba(200, 40, 120, 210),
            offset: Vec2::new(10.0, 12.0),
            radius: 6.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
            blend: drop_blend,
        },
        Effect::InnerShadow { color: Color::rgba(40, 80, 220, 210), offset: Vec2::new(8.0, 10.0), radius: 6.0, spread: 0.0, visible: true, blend: inner_blend },
    ];
    doc.insert(root_id, card).unwrap();

    (doc, root_id)
}

fn roundtrip_svg(doc: &Document, root_id: NodeId) -> (String, Document, NodeId) {
    let svg = export_svg(doc, root_id, &SvgOptions { outline_text: false });
    let mut back = Document::new("back");
    let page = back.add_page("Page 1");
    let back_root = import_svg_named(&svg, "Root", &mut back, page, Vec2::ZERO).unwrap_or_else(|e| panic!("import_svg_named failed: {e}\n{svg}"));
    (svg, back, back_root)
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
fn svg_roundtrip_preserves_multiply_and_screen_shadow_blend_modes() {
    for (mode, token) in [(BlendMode::Multiply, "multiply"), (BlendMode::Screen, "screen")] {
        let (doc, root_id) = build_shadow_scene(mode, mode);
        let (svg, back, _back_root) = roundtrip_svg(&doc, root_id);
        assert!(svg.contains(&format!("feBlend mode=\"{token}\"")), "expected feBlend mode=\"{token}\" in exported SVG:\n{svg}");
        let card = back
            .nodes
            .values()
            .find(|n| matches!(n.kind, NodeKind::Rectangle) && n.effects.len() == 2)
            .unwrap_or_else(|| panic!("expected reimported Card with 2 effects in {:#?}\n{svg}", back.nodes));
        assert!(matches!(card.effects[0], Effect::DropShadow { blend, .. } if blend == mode), "expected DropShadow blend {mode:?}, got {:?}", card.effects[0]);
        assert!(
            matches!(card.effects[1], Effect::InnerShadow { blend, .. } if blend == mode),
            "expected InnerShadow blend {mode:?}, got {:?}",
            card.effects[1]
        );
    }
}

#[test]
fn svg_roundtrip_unsupported_shadow_blend_modes_fall_back_to_normal() {
    for mode in [BlendMode::LinearBurn, BlendMode::PassThrough] {
        let (doc, root_id) = build_shadow_scene(mode, mode);
        let (svg, back, _back_root) = roundtrip_svg(&doc, root_id);
        assert!(svg.contains("<feBlend mode=\"normal\" in=\"shadow0\""), "expected drop shadow feBlend mode=\"normal\" fallback for {mode:?}:\n{svg}");
        assert!(svg.contains("<feBlend mode=\"normal\" in=\"innerShadow1\""), "expected inner shadow feBlend mode=\"normal\" fallback for {mode:?}:\n{svg}");
        let card = back.nodes.values().find(|n| matches!(n.kind, NodeKind::Rectangle) && n.effects.len() == 2).expect("reimported Card");
        assert!(matches!(card.effects[0], Effect::DropShadow { blend: BlendMode::Normal, .. }));
        assert!(matches!(card.effects[1], Effect::InnerShadow { blend: BlendMode::Normal, .. }));
    }
}

#[test]
fn cpu_render_of_svg_reimported_multiply_shadow_node_matches_original() {
    let (doc, root_id) = build_shadow_scene(BlendMode::Multiply, BlendMode::Multiply);
    let (_svg, back, back_root) = roundtrip_svg(&doc, root_id);

    let original_pm = designos_render::render_node(&doc, root_id, 1.0).expect("render original");
    let reimported_pm = designos_render::render_node(&back, back_root, 1.0).expect("render reimported");

    let diff = mismatch(&original_pm, &reimported_pm);
    assert!(diff < MAX_MISMATCH, "expected SVG-reimported Multiply shadow CPU render to match original within {MAX_MISMATCH}, got mismatch={diff:.4}");
}
