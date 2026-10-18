use designos_doc::*;
use designos_fig::{export_rest_json, image_refs, import_rest_json};

const FIXTURE: &str = include_str!("fixtures/rest_file.json");

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

#[test]
fn pages_and_structure() {
    let doc = import_rest_json(FIXTURE).unwrap();
    assert_eq!(doc.name, "Fixture file");
    assert_eq!(doc.pages.len(), 2);
    let p0 = doc.get(doc.pages[0]).unwrap();
    assert_eq!(p0.name, "Cover");
    assert!(matches!(p0.kind, NodeKind::Page { background } if approx(background.r, 0.96)));
    assert_eq!(p0.source_id.as_deref(), Some("0:1"));
    assert_eq!(p0.children.len(), 2);
    assert_eq!(doc.get(doc.pages[1]).unwrap().name, "Second page");
    // every non-page node has a parent that lists it
    for n in doc.nodes.values() {
        if let Some(p) = n.parent {
            assert!(doc.get(p).unwrap().children.contains(&n.id));
        }
    }
}

#[test]
fn frame_properties() {
    let doc = import_rest_json(FIXTURE).unwrap();
    let card = find(&doc, "Card");
    assert_eq!(card.source_id.as_deref(), Some("1:1"));
    assert!(card.locked && card.visible);
    assert!(approx(card.opacity, 0.8));
    assert_eq!(card.blend, BlendMode::PassThrough);
    assert_eq!(card.transform.translation(), Vec2::new(100.0, 200.0));
    assert_eq!(card.size, Vec2::new(320.0, 400.0));
    assert_eq!(card.corner_radii, [16.0, 16.0, 4.0, 4.0]);
    assert!(approx(card.corner_smoothing, 0.6));
    assert_eq!(card.constraints, Constraints { horizontal: Constraint::Stretch, vertical: Constraint::Max });
    assert_eq!(card.stroke.side_weights, Some([1.0, 2.0, 3.0, 4.0]));
    assert_eq!(card.stroke.align, StrokeAlign::Center);
    assert_eq!(card.stroke.cap, StrokeCap::Round);
    assert_eq!(card.stroke.join, StrokeJoin::Bevel);
    assert_eq!(card.stroke.dashes, vec![4.0, 2.0]);
    // miter angle 40deg -> 1/sin(20deg)
    assert!(approx(card.stroke.miter_limit, 1.0 / 20f32.to_radians().sin()));
    assert_eq!(card.effects.len(), 4);
    assert!(matches!(card.effects[0], Effect::DropShadow { show_behind: true, radius, spread, .. } if radius == 12.0 && spread == 2.0));
    assert!(matches!(card.effects[1], Effect::InnerShadow { visible: false, .. }));
    assert!(matches!(card.effects[3], Effect::BackgroundBlur { radius, .. } if radius == 20.0));
    assert_eq!(card.style_refs["fill"], "S:fill1");
    assert!(matches!(doc.styles["S:fill1"], Style::Fill(ref p) if p.len() == 1));
    assert!(matches!(doc.styles["S:fx1"], Style::Effect(ref e) if e.len() == 4));

    let f = card.kind.frame_props().unwrap();
    assert!(f.clips_content);
    let al = f.auto_layout.as_ref().unwrap();
    assert_eq!(al.direction, LayoutDirection::Vertical);
    assert_eq!(al.primary_align, LayoutAlign::SpaceBetween);
    assert_eq!(al.counter_align, LayoutAlign::Center);
    assert_eq!(al.primary_sizing, Sizing::Hug);
    assert_eq!(al.counter_sizing, Sizing::Fixed);
    assert_eq!(al.padding, [8.0, 20.0, 12.0, 16.0]);
    assert_eq!((al.item_spacing, al.counter_spacing), (10.0, 6.0));
    assert!(al.stroke_included && al.reverse_z && !al.wrap);
    assert_eq!(f.grids.len(), 3);
    assert!(matches!(f.grids[0], LayoutGrid::Columns { count: 4, .. }));
    assert!(matches!(f.grids[1], LayoutGrid::Rows { count: 3, .. }));
    assert!(matches!(f.grids[2], LayoutGrid::Grid { size, .. } if size == 8.0));
}

#[test]
fn child_layout_paints_and_relative_positions() {
    let doc = import_rest_json(FIXTURE).unwrap();
    let r = find(&doc, "Gradient rect");
    // no relativeTransform: derived from the absolute boxes (116-100, 208-200)
    assert_eq!(r.transform.translation(), Vec2::new(16.0, 8.0));
    assert!(r.is_mask);
    assert_eq!(r.blend, BlendMode::Multiply);
    assert_eq!(r.layout_child.horizontal, Sizing::Fill);
    assert_eq!(r.layout_child.vertical, Sizing::Fixed);
    assert_eq!((r.layout_child.min_w, r.layout_child.max_w, r.layout_child.min_h), (Some(100.0), Some(500.0), None));
    assert_eq!(r.corner_radii, [8.0; 4]);
    // 6 usable paints: linear, radial, angular, diamond, image, image (EMOJI dropped)
    assert_eq!(r.fills.len(), 6);
    match &r.fills[0].kind {
        PaintKind::LinearGradient { handles, stops } => {
            assert_eq!(handles[0], Vec2::new(0.0, 0.5));
            assert_eq!(handles[1], Vec2::new(1.0, 0.5));
            assert_eq!(stops.len(), 2);
            assert_eq!(stops[1].color, Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 });
        }
        k => panic!("{k:?}"),
    }
    assert!(approx(r.fills[0].opacity, 0.9));
    assert!(matches!(r.fills[1].kind, PaintKind::RadialGradient { .. }));
    assert!(matches!(r.fills[2].kind, PaintKind::AngularGradient { .. }));
    assert!(matches!(r.fills[3].kind, PaintKind::DiamondGradient { .. }));
    assert!(matches!(&r.fills[4].kind, PaintKind::Image { image_ref, scale_mode: ImageScaleMode::Fit } if image_ref == "deadbeefcafe"));
    assert!(!r.fills[5].visible);
    assert_eq!(image_refs(&doc), vec!["0123456789ab".to_string(), "deadbeefcafe".to_string()]);

    let pie = find(&doc, "Pie");
    assert!(pie.layout_child.absolute);
    assert!(matches!(pie.kind, NodeKind::Ellipse(a) if approx(a.start, 0.5) && approx(a.end, 3.5) && approx(a.inner_radius, 0.25)));
}

#[test]
fn shapes_and_vectors() {
    let doc = import_rest_json(FIXTURE).unwrap();
    assert!(matches!(find(&doc, "Rule").kind, NodeKind::Line));
    assert_eq!(find(&doc, "Rule").stroke.cap, StrokeCap::Square);
    assert!(matches!(find(&doc, "Hexagon").kind, NodeKind::Polygon { count: 6 }));
    assert!(matches!(find(&doc, "Star").kind, NodeKind::Star { count: 5, .. }));
    match &find(&doc, "Heart").kind {
        NodeKind::Vector { paths, .. } => {
            assert_eq!(paths.len(), 2);
            assert_eq!(paths[0].fill_rule, FillRule::NonZero);
            assert_eq!(paths[1].fill_rule, FillRule::EvenOdd);
            // arcs became cubics; the path closes
            assert!(paths[0].cmds.iter().any(|c| matches!(c, PathCmd::CubicTo(..))));
            assert_eq!(*paths[0].cmds.last().unwrap(), PathCmd::Close);
            assert_eq!(paths[1].cmds.len(), 4);
        }
        k => panic!("{k:?}"),
    }
    let u = find(&doc, "Union");
    assert!(matches!(u.kind, NodeKind::BooleanOp(BooleanOp::Subtract)));
    assert_eq!(u.children.len(), 2);
    assert_eq!(doc.get(u.children[1]).unwrap().name, "B");
    assert_eq!(find(&doc, "In group").parent, Some(find(&doc, "Group").id));
    assert!(matches!(&find(&doc, "Slice").kind, NodeKind::Unknown { type_name } if type_name == "SLICE"));
    assert!(matches!(&find(&doc, "Sticky").kind, NodeKind::Sticky(_)), "FigJam stickies map to the board kind");
    assert!(matches!(find(&doc, "Section").kind, NodeKind::Section));
}

#[test]
fn text_runs_use_utf16_indices() {
    let doc = import_rest_json(FIXTURE).unwrap();
    let t = find(&doc, "Title");
    let NodeKind::Text(tp) = &t.kind else { panic!() };
    let s = "Hi \u{1F600} x\u{e9}!";
    assert_eq!(tp.content, s);
    assert_eq!(tp.style.font_family, "Inter");
    assert_eq!(tp.style.font_weight, 600);
    assert_eq!(tp.style.font_size, 20.0);
    assert_eq!(tp.style.align_h, TextAlignH::Center);
    assert_eq!(tp.style.align_v, TextAlignV::Bottom);
    assert_eq!(tp.style.case, TextCase::Upper);
    assert_eq!(tp.style.decoration, TextDecoration::Strikethrough);
    assert_eq!(tp.style.line_height, LineHeight::Percent(140.0));
    assert_eq!(tp.style.letter_spacing, 0.5);
    assert_eq!(tp.auto_resize, TextAutoResize::Height);
    assert!(matches!(tp.style.fills[0].kind, PaintKind::Solid(c) if approx(c.b, 0.3)));
    assert!(t.fills.is_empty(), "text color lives in TextStyle::fills");
    assert!(matches!(doc.styles["S:text1"], Style::Text(ref ts) if ts.font_size == 20.0));
    assert_eq!(t.style_refs["text"], "S:text1");

    // UTF-16 overrides: [0,0,0,1,1,0,2,2] over "Hi " + emoji(2 units) + " " + "xé" + "!"
    // units: H0 i1 sp2 emoji3-4 sp5 x6 é7 !8 (the last char has no override entry)
    assert_eq!(tp.runs.len(), 2);
    let r0 = &tp.runs[0];
    assert_eq!(&s[r0.start..r0.end], "\u{1F600}");
    assert_eq!(r0.style.font_weight, 700);
    assert!(r0.style.italic);
    assert!(matches!(r0.style.fills[0].kind, PaintKind::Solid(c) if c.r == 1.0));
    assert_eq!(r0.style.font_size, 20.0, "inherits the base style");
    let r1 = &tp.runs[1];
    assert_eq!(&s[r1.start..r1.end], "x\u{e9}");
    assert_eq!(r1.style.font_size, 12.0);
    assert_eq!(r1.style.line_height, LineHeight::Pixels(16.0));
    assert_eq!(r1.style.decoration, TextDecoration::Underline);
}

#[test]
fn components_instances_and_auto_layout_children() {
    let doc = import_rest_json(FIXTURE).unwrap();
    let set = find(&doc, "Variants");
    assert!(matches!(set.kind, NodeKind::ComponentSet(_)));
    let comp = find(&doc, "Button");
    assert!(matches!(comp.kind, NodeKind::Component(_)));
    let inst = find(&doc, "Button instance");
    match &inst.kind {
        NodeKind::Instance { component, component_key, frame } => {
            assert_eq!(*component, Some(comp.id));
            assert_eq!(component_key.as_deref(), Some("abc123"));
            assert!(frame.auto_layout.is_some());
        }
        k => panic!("{k:?}"),
    }
    // REST instances carry their own (overridden) children; they are not replaced.
    assert_eq!(inst.children.len(), 1);
    let NodeKind::Text(t) = &doc.get(inst.children[0]).unwrap().kind else { panic!() };
    assert_eq!(t.content, "Overridden");
    // grow + hug derivation inside the horizontal component
    let label = find(&doc, "Label");
    assert_eq!(label.layout_child.grow, 1.0);
    assert_eq!(label.layout_child.horizontal, Sizing::Fill);
    let al = comp.kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
    assert_eq!((al.primary_sizing, al.counter_sizing), (Sizing::Hug, Sizing::Hug));
}

#[test]
fn export_roundtrip_is_stable() {
    let a = import_rest_json(FIXTURE).unwrap();
    let json_a = export_rest_json(&a);
    let b = import_rest_json(&json_a.to_string()).unwrap();
    assert_eq!(a.nodes, b.nodes, "import(export(doc)) must equal doc");
    assert_eq!(a.pages, b.pages);
    assert_eq!(a.name, b.name);
    assert_eq!(a.styles.len(), b.styles.len());
    for (k, v) in &a.styles {
        assert!(format!("{v:?}") == format!("{:?}", b.styles[k]), "style {k}");
    }
    // and exporting again is a fixed point
    assert_eq!(json_a, export_rest_json(&b));
}

#[test]
fn tolerates_minimal_and_odd_input() {
    assert!(import_rest_json("{").is_err());
    assert!(import_rest_json("{}").is_err());
    let d = import_rest_json(r#"{"document":{"type":"DOCUMENT","children":[{"type":"CANVAS","children":[{"type":"FRAME"},{"type":"TEXT","characters":"é","characterStyleOverrides":[1,1,1,1],"styleOverrideTable":{"1":{"fontSize":5}}},{"id":7}]}]}}"#).unwrap();
    assert_eq!(d.pages.len(), 1);
    assert_eq!(d.nodes.len(), 3);
    // deep nesting is rejected, not a stack overflow
    let deep = format!("{}1{}", "[".repeat(5000), "]".repeat(5000));
    assert!(import_rest_json(&deep).is_err());
}

fn doc_with_effects(effects: Vec<Effect>) -> Document {
    let mut doc = Document::new("fx");
    let page = doc.add_page("p");
    let id = doc.alloc_id();
    let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0));
    n.effects = effects;
    doc.insert(page, n).unwrap();
    doc
}

#[test]
fn noise_texture_and_glass_effects_roundtrip() {
    let effects = vec![
        Effect::Noise(NoiseEffect {
            mode: NoiseMode::Duo { color: Color::rgb(255, 0, 0), secondary: Color::rgb(0, 0, 255) },
            size: 2.0,
            density: 0.4,
            opacity: 0.3,
            blend: BlendMode::Multiply,
            seed: 9,
            visible: true,
        }),
        Effect::Texture(TextureEffect { size: 3.0, radius: 5.0, clip_to_shape: false, seed: 2, visible: false }),
        Effect::Glass(GlassEffect { light_angle: 120.0, light_intensity: 0.6, refraction: 0.7, depth: 14.0, dispersion: 0.2, frost: 6.0, visible: true }),
    ];
    let doc = doc_with_effects(effects.clone());
    let json = serde_json::to_string(&export_rest_json(&doc)).unwrap();
    let back = import_rest_json(&json).unwrap();
    let n = back.nodes.values().find(|n| matches!(n.kind, NodeKind::Rectangle)).unwrap();
    assert_eq!(n.effects.len(), 3);
    for (a, b) in n.effects.iter().zip(&effects) {
        assert_eq!(std::mem::discriminant(a), std::mem::discriminant(b));
    }
    let Effect::Noise(nz) = &n.effects[0] else { panic!() };
    assert!(matches!(nz.mode, NoiseMode::Duo { .. }) && nz.seed == 9 && approx(nz.density, 0.4) && nz.blend == BlendMode::Multiply);
    let Effect::Texture(t) = &n.effects[1] else { panic!() };
    assert!(!t.clip_to_shape && !t.visible && approx(t.radius, 5.0));
    let Effect::Glass(g) = &n.effects[2] else { panic!() };
    assert!(approx(g.light_angle, 120.0) && approx(g.frost, 6.0) && approx(g.depth, 14.0));
}

#[test]
fn new_effects_tolerate_missing_fields() {
    let json = r#"{"name":"f","document":{"id":"0:0","type":"DOCUMENT","children":[{"id":"0:1","type":"CANVAS","name":"p","children":[
        {"id":"1:1","type":"RECTANGLE","name":"r","size":{"x":10,"y":10},
         "effects":[{"type":"NOISE"},{"type":"TEXTURE"},{"type":"GLASS"}]}]}]}}"#;
    let doc = import_rest_json(json).unwrap();
    let r = find(&doc, "r");
    assert!(matches!(r.effects[0], Effect::Noise(_)));
    assert!(matches!(r.effects[1], Effect::Texture(_)));
    assert!(matches!(r.effects[2], Effect::Glass(_)));
}
