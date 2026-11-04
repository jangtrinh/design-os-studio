use designos_doc::*;
use designos_fig::{export_rest_json, image_refs, import_rest_json, import_rest_json_with_report, import_variables_json};

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
    assert!(matches!(&r.fills[4].kind, PaintKind::Image { image_ref, scale_mode: ImageScaleMode::Fit, .. } if image_ref == "deadbeefcafe"));
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
            size_y: None,
            density: 0.4,
            opacity: 0.3,
            blend: BlendMode::Multiply,
            seed: 9,
            visible: true,
        }),
        Effect::Texture(TextureEffect { size: 3.0, size_y: None, radius: 5.0, clip_to_shape: false, seed: 2, visible: false }),
        Effect::Glass(GlassEffect {
            light_angle: 120.0,
            light_intensity: 0.6,
            refraction: 0.7,
            depth: 14.0,
            dispersion: 0.2,
            frost: 6.0,
            splay: 0.0,
            bevel: 0.0,
            visible: true,
        }),
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

fn rest_with_effects(effects: serde_json::Value) -> String {
    serde_json::json!({"name": "f", "document": {"id": "0:0", "type": "DOCUMENT", "children": [{"id": "0:1", "type": "CANVAS", "name": "p", "children": [
        {"id": "1:1", "type": "RECTANGLE", "name": "r", "size": {"x": 10, "y": 10}, "effects": effects}]}]}})
    .to_string()
}

fn saved_effects(doc: &Document) -> Vec<serde_json::Value> {
    find(doc, "r").effects.iter().map(|e| serde_json::to_value(e).unwrap()).collect()
}

#[test]
fn rest_round_trip_keeps_all_glass_noise_and_texture_fields() {
    use serde_json::json;
    let rest = json!([
        {"type": "NOISE", "noiseType": "MONOTONE", "color": {"r": 0.25, "g": 0.5, "b": 0.75, "a": 1.0}, "noiseSize": {"x": 3.0, "y": 7.0},
         "density": 0.5, "opacity": 0.5, "blendMode": "OVERLAY", "seed": 42, "visible": true},
        {"type": "NOISE", "noiseType": "DUOTONE", "color": {"r": 1.0, "g": 0.0, "b": 0.0, "a": 1.0}, "secondaryColor": {"r": 0.0, "g": 0.0, "b": 1.0, "a": 0.5},
         "noiseSize": {"x": 2.0, "y": 0.5}, "density": 0.25, "opacity": 0.75, "blendMode": "MULTIPLY", "seed": 9, "visible": false},
        {"type": "NOISE", "noiseType": "MULTITONE", "noiseSize": 1.5, "density": 1.0, "opacity": 0.25, "blendMode": "NORMAL", "seed": 77, "visible": true},
        {"type": "TEXTURE", "noiseSize": {"x": 3.0, "y": 7.0}, "radius": 6.5, "clipToShape": false, "seed": 13, "visible": true},
        {"type": "GLASS", "lightAngle": 135.0, "lightIntensity": 0.75, "refraction": 0.5, "depth": 20.0, "dispersion": 0.5, "radius": 4.0,
         "reflectionDistance": 0.25, "bevelSize": 4.0, "visible": true}
    ]);
    let expected = vec![
        json!({"Noise": {"mode": {"Mono": {"color": {"r": 0.25, "g": 0.5, "b": 0.75, "a": 1.0}}}, "size": 3.0, "size_y": 7.0, "density": 0.5, "opacity": 0.5,
               "blend": "Overlay", "seed": 42, "visible": true}}),
        json!({"Noise": {"mode": {"Duo": {"color": {"r": 1.0, "g": 0.0, "b": 0.0, "a": 1.0}, "secondary": {"r": 0.0, "g": 0.0, "b": 1.0, "a": 0.5}}},
               "size": 2.0, "size_y": 0.5, "density": 0.25, "opacity": 0.75, "blend": "Multiply", "seed": 9, "visible": false}}),
        json!({"Noise": {"mode": "Multi", "size": 1.5, "density": 1.0, "opacity": 0.25, "blend": "Normal", "seed": 77, "visible": true}}),
        json!({"Texture": {"size": 3.0, "size_y": 7.0, "radius": 6.5, "clip_to_shape": false, "seed": 13, "visible": true}}),
        json!({"Glass": {"light_angle": 135.0, "light_intensity": 0.75, "refraction": 0.5, "depth": 20.0, "dispersion": 0.5, "frost": 4.0,
               "splay": 0.25, "bevel": 4.0, "visible": true}}),
    ];
    let doc = import_rest_json(&rest_with_effects(rest)).unwrap();
    assert_eq!(saved_effects(&doc), expected);
    let exported = export_rest_json(&doc);
    let back = import_rest_json(&serde_json::to_string(&exported).unwrap()).unwrap();
    assert_eq!(saved_effects(&back), expected, "export then import");
    let written = &exported["document"]["children"][0]["children"][0]["effects"];
    assert_eq!(written[0]["noiseSize"], json!({"x": 3.0, "y": 7.0}));
    assert_eq!(written[2]["noiseSize"], json!(1.5), "equal sizes stay one number");
    assert_eq!(written[3]["noiseSize"], json!({"x": 3.0, "y": 7.0}));
    assert_eq!((&written[4]["reflectionDistance"], &written[4]["bevelSize"]), (&json!(0.25), &json!(4.0)));
}

#[test]
fn a_rest_noise_size_pair_with_equal_sides_is_one_size() {
    let rest = serde_json::json!([{"type": "TEXTURE", "noiseSize": {"x": 4.0, "y": 4.0}, "radius": 2.0, "clipToShape": true, "seed": 1, "visible": true}]);
    let doc = import_rest_json(&rest_with_effects(rest)).unwrap();
    assert_eq!(saved_effects(&doc), vec![serde_json::json!({"Texture": {"size": 4.0, "radius": 2.0, "clip_to_shape": true, "seed": 1, "visible": true}})]);
}

#[test]
fn both_noise_type_spellings_import_and_export_the_full_name() {
    for (spelling, full) in
        [("MONO", "MONOTONE"), ("MONOTONE", "MONOTONE"), ("DUO", "DUOTONE"), ("DUOTONE", "DUOTONE"), ("MULTI", "MULTITONE"), ("MULTITONE", "MULTITONE")]
    {
        let rest = serde_json::json!([{"type": "NOISE", "noiseType": spelling, "noiseSize": 1.0, "density": 0.5, "opacity": 0.5, "seed": 1}]);
        let doc = import_rest_json(&rest_with_effects(rest)).unwrap();
        let Effect::Noise(nz) = &find(&doc, "r").effects[0] else { panic!("not a noise") };
        let want_mono = full == "MONOTONE";
        assert_eq!(matches!(nz.mode, NoiseMode::Mono { .. }), want_mono, "{spelling}");
        let exported = export_rest_json(&doc);
        assert_eq!(exported["document"]["children"][0]["children"][0]["effects"][0]["noiseType"], full, "{spelling}");
    }
}

#[test]
fn rest_import_keeps_bindings_to_variables_missing_from_file() {
    let json = r#"{
      "name": "MissingVariable",
      "document": {"type": "DOCUMENT", "children": [{"type": "CANVAS", "name": "P", "children": [
        {"id": "1:1", "type": "RECTANGLE", "name": "Gradient",
         "absoluteBoundingBox": {"x": 0, "y": 0, "width": 40, "height": 40},
         "fills": [{"type": "GRADIENT_LINEAR", "gradientHandlePositions": [
           {"x": 0, "y": 0.5}, {"x": 1, "y": 0.5}, {"x": 0, "y": 1}],
           "gradientStops": [{"position": 0, "color": {"r": 1, "g": 0, "b": 0, "a": 1},
             "boundVariables": {"color": {"type": "VARIABLE_ALIAS", "id": "VariableID:9:9"}}},
             {"position": 1, "color": {"r": 0, "g": 0, "b": 1, "a": 1}}]}]
        }
      ]}]}
    }"#;
    let (doc, report) = import_rest_json_with_report(json).unwrap();
    let gradient = find(&doc, "Gradient");
    assert!(doc.variables.is_empty());
    assert_eq!(report.missing_variable_bindings, 1);
    assert_eq!(gradient.variables.bindings.get("fills/0/stops/0").map(String::as_str), Some("VariableID:9:9"));
}

#[test]
fn rest_import_bindings_resolve_after_separate_variables_import() {
    let file = r#"{
      "name": "SeparateVariables",
      "document": {"type": "DOCUMENT", "children": [{"type": "CANVAS", "name": "P", "children": [
        {"id": "1:1", "type": "RECTANGLE", "name": "Gradient",
         "absoluteBoundingBox": {"x": 0, "y": 0, "width": 40, "height": 40},
         "fills": [{"type": "GRADIENT_LINEAR", "gradientHandlePositions": [
           {"x": 0, "y": 0.5}, {"x": 1, "y": 0.5}, {"x": 0, "y": 1}],
           "gradientStops": [{"position": 0, "color": {"r": 1, "g": 0, "b": 0, "a": 1},
             "boundVariables": {"color": {"type": "VARIABLE_ALIAS", "id": "VariableID:9:9"}}}]}]
        }
      ]}]}
    }"#;
    let vars = r#"{
      "meta": {
        "variableCollections": {"VariableCollectionId:9:1": {
          "id": "VariableCollectionId:9:1", "name": "Theme",
          "modes": [{"modeId": "9:0", "name": "Default"}],
          "defaultModeId": "9:0", "variableIds": ["VariableID:9:9"]
        }},
        "variables": {"VariableID:9:9": {
          "id": "VariableID:9:9", "name": "Accent", "variableCollectionId": "VariableCollectionId:9:1",
          "resolvedType": "COLOR", "valuesByMode": {"9:0": {"r": 0.2, "g": 0.4, "b": 0.6, "a": 1}}
        }}
      }
    }"#;
    let (mut doc, report) = import_rest_json_with_report(file).unwrap();
    assert_eq!(report.missing_variable_bindings, 1);
    let gradient_id = find(&doc, "Gradient").id;
    assert_eq!(doc.get(gradient_id).unwrap().variables.bindings.get("fills/0/stops/0").map(String::as_str), Some("VariableID:9:9"));
    assert_eq!(doc.bound_value(gradient_id, "fills/0/stops/0"), None);
    assert_eq!(import_variables_json(&mut doc, vars).unwrap(), 1);
    assert_eq!(doc.bound_value(gradient_id, "fills/0/stops/0"), Some(VariableValue::Color(Color { r: 0.2, g: 0.4, b: 0.6, a: 1.0 })));
}

#[test]
fn rest_import_keeps_bindings_to_variables_present_in_file() {
    let json = r#"{
      "name": "PresentVariable",
      "meta": {
        "variableCollections": {"VariableCollectionId:9:1": {
          "id": "VariableCollectionId:9:1", "name": "Theme",
          "modes": [{"modeId": "9:0", "name": "Default"}],
          "defaultModeId": "9:0", "variableIds": ["VariableID:9:9"]
        }},
        "variables": {"VariableID:9:9": {
          "id": "VariableID:9:9", "name": "Accent", "variableCollectionId": "VariableCollectionId:9:1",
          "resolvedType": "COLOR", "valuesByMode": {"9:0": {"r": 1, "g": 0, "b": 0, "a": 1}}
        }}
      },
      "document": {"type": "DOCUMENT", "children": [{"type": "CANVAS", "name": "P", "children": [
        {"id": "1:1", "type": "RECTANGLE", "name": "Gradient",
         "absoluteBoundingBox": {"x": 0, "y": 0, "width": 40, "height": 40},
         "fills": [{"type": "GRADIENT_LINEAR", "gradientHandlePositions": [
           {"x": 0, "y": 0.5}, {"x": 1, "y": 0.5}, {"x": 0, "y": 1}],
           "gradientStops": [{"position": 0, "color": {"r": 1, "g": 0, "b": 0, "a": 1},
             "boundVariables": {"color": {"type": "VARIABLE_ALIAS", "id": "VariableID:9:9"}}}]}]
        }
      ]}]}
    }"#;
    let doc = import_rest_json(json).unwrap();
    let gradient = find(&doc, "Gradient");
    assert_eq!(gradient.variables.bindings.get("fills/0/stops/0").map(String::as_str), Some("VariableID:9:9"));
}

fn rest_file_with_nodes(nodes: &str) -> String {
    format!(r#"{{"name": "Missing", "document": {{"type": "DOCUMENT", "children": [{{"type": "CANVAS", "name": "P", "children": [{nodes}]}}]}}}}"#)
}

#[test]
fn a_solid_fill_bound_to_a_missing_variable_counts_as_one_missing_variable() {
    let json = rest_file_with_nodes(
        r#"{"id": "1:1", "type": "RECTANGLE", "name": "Solid",
            "absoluteBoundingBox": {"x": 0, "y": 0, "width": 40, "height": 40},
            "fills": [{"type": "SOLID", "color": {"r": 1, "g": 0, "b": 0, "a": 1},
              "boundVariables": {"color": {"type": "VARIABLE_ALIAS", "id": "VariableID:9:9"}}}]}"#,
    );
    let (doc, report) = import_rest_json_with_report(&json).unwrap();
    assert!(doc.variables.is_empty());
    assert_eq!(report.missing_variable_bindings, 1);
    assert_eq!(find(&doc, "Solid").variables.bindings.get("fills/0").map(String::as_str), Some("VariableID:9:9"));
}

#[test]
fn a_style_and_the_layer_using_it_bound_to_the_same_missing_variable_count_once() {
    let json = r#"{
      "name": "StyledGradient",
      "styles": {"S:grad": {"key": "k1", "name": "Brand/Gradient", "styleType": "FILL", "description": "",
        "fills": [{"type": "GRADIENT_LINEAR", "gradientHandlePositions": [{"x": 0, "y": 0.5}, {"x": 1, "y": 0.5}, {"x": 0, "y": 1}],
          "gradientStops": [{"position": 0, "color": {"r": 1, "g": 0, "b": 0, "a": 1},
            "boundVariables": {"color": {"type": "VARIABLE_ALIAS", "id": "VariableID:9:9"}}}]}]}},
      "document": {"type": "DOCUMENT", "children": [{"type": "CANVAS", "name": "P", "children": [
        {"id": "1:1", "type": "RECTANGLE", "name": "Gradient",
         "absoluteBoundingBox": {"x": 0, "y": 0, "width": 40, "height": 40},
         "styles": {"fill": "S:grad"},
         "fills": [{"type": "GRADIENT_LINEAR", "gradientHandlePositions": [{"x": 0, "y": 0.5}, {"x": 1, "y": 0.5}, {"x": 0, "y": 1}],
           "gradientStops": [{"position": 0, "color": {"r": 1, "g": 0, "b": 0, "a": 1},
             "boundVariables": {"color": {"type": "VARIABLE_ALIAS", "id": "VariableID:9:9"}}}]}]}
      ]}]}
    }"#;
    let (doc, report) = import_rest_json_with_report(json).unwrap();
    let on_layer = find(&doc, "Gradient").variables.bindings.get("fills/0/stops/0").map(String::as_str);
    let on_style = doc.style_meta.get("S:grad").and_then(|m| m.bindings.get("fills/0/stops/0")).map(String::as_str);
    assert_eq!((on_layer, on_style), (Some("VariableID:9:9"), Some("VariableID:9:9")), "the binding sits on both the layer and the style");
    assert_eq!(report.missing_variable_bindings, 1);
}

#[test]
fn two_layers_bound_to_the_same_missing_variable_count_once() {
    let rect = |id: &str, name: &str| {
        format!(
            r#"{{"id": "{id}", "type": "RECTANGLE", "name": "{name}",
                "absoluteBoundingBox": {{"x": 0, "y": 0, "width": 40, "height": 40}},
                "fills": [{{"type": "SOLID", "color": {{"r": 1, "g": 0, "b": 0, "a": 1}},
                  "boundVariables": {{"color": {{"type": "VARIABLE_ALIAS", "id": "VariableID:9:9"}}}}}}]}}"#
        )
    };
    let json = rest_file_with_nodes(&format!("{},{}", rect("1:1", "First"), rect("1:2", "Second")));
    let (doc, report) = import_rest_json_with_report(&json).unwrap();
    assert_eq!(find(&doc, "First").variables.bindings.get("fills/0"), find(&doc, "Second").variables.bindings.get("fills/0"));
    assert_eq!(report.missing_variable_bindings, 1);
}

#[test]
fn every_kind_of_binding_to_a_missing_variable_counts_once_per_variable() {
    let json = rest_file_with_nodes(
        r#"{"id": "1:1", "type": "FRAME", "name": "Card",
            "absoluteBoundingBox": {"x": 0, "y": 0, "width": 100, "height": 100},
            "layoutMode": "VERTICAL", "itemSpacing": 8,
            "boundVariables": {"itemSpacing": {"type": "VARIABLE_ALIAS", "id": "VariableID:9:11"}},
            "fills": [{"type": "SOLID", "color": {"r": 1, "g": 1, "b": 1, "a": 1},
              "boundVariables": {"color": {"type": "VARIABLE_ALIAS", "id": "VariableID:9:10"}}}],
            "strokes": [{"type": "SOLID", "color": {"r": 0, "g": 0, "b": 0, "a": 1},
              "boundVariables": {"color": {"type": "VARIABLE_ALIAS", "id": "VariableID:9:12"}}}],
            "effects": [{"type": "DROP_SHADOW", "visible": true, "color": {"r": 0, "g": 0, "b": 0, "a": 0.25},
              "offset": {"x": 0, "y": 2}, "radius": 4, "spread": 0,
              "boundVariables": {"radius": {"type": "VARIABLE_ALIAS", "id": "VariableID:9:13"}}}],
            "children": [{"id": "1:2", "type": "TEXT", "name": "Label", "characters": "Hi",
              "absoluteBoundingBox": {"x": 0, "y": 0, "width": 40, "height": 20},
              "style": {"fontFamily": "Inter", "fontSize": 16},
              "boundVariables": {"fontSize": [{"type": "VARIABLE_ALIAS", "id": "VariableID:9:14"}]}}]}"#,
    );
    let (doc, report) = import_rest_json_with_report(&json).unwrap();
    let card = find(&doc, "Card");
    let kept: std::collections::BTreeSet<&str> = card.variables.bindings.values().map(String::as_str).collect();
    assert_eq!(kept, ["VariableID:9:10", "VariableID:9:11", "VariableID:9:12", "VariableID:9:13"].into_iter().collect(), "{:?}", card.variables.bindings);
    assert_eq!(find(&doc, "Label").variables.bindings.get("fontSize").map(String::as_str), Some("VariableID:9:14"));
    assert_eq!(report.missing_variable_bindings, 5);
}

#[test]
fn a_binding_to_a_variable_in_the_file_counts_no_missing_variable() {
    let json = r#"{
      "name": "PresentSolid",
      "meta": {
        "variableCollections": {"VariableCollectionId:9:1": {
          "id": "VariableCollectionId:9:1", "name": "Theme",
          "modes": [{"modeId": "9:0", "name": "Default"}],
          "defaultModeId": "9:0", "variableIds": ["VariableID:9:9"]
        }},
        "variables": {"VariableID:9:9": {
          "id": "VariableID:9:9", "name": "Accent", "variableCollectionId": "VariableCollectionId:9:1",
          "resolvedType": "COLOR", "valuesByMode": {"9:0": {"r": 1, "g": 0, "b": 0, "a": 1}}
        }}
      },
      "document": {"type": "DOCUMENT", "children": [{"type": "CANVAS", "name": "P", "children": [
        {"id": "1:1", "type": "RECTANGLE", "name": "Solid",
         "absoluteBoundingBox": {"x": 0, "y": 0, "width": 40, "height": 40},
         "fills": [{"type": "SOLID", "color": {"r": 1, "g": 0, "b": 0, "a": 1},
           "boundVariables": {"color": {"type": "VARIABLE_ALIAS", "id": "VariableID:9:9"}}}]}
      ]}]}
    }"#;
    let (doc, report) = import_rest_json_with_report(json).unwrap();
    assert_eq!(find(&doc, "Solid").variables.bindings.get("fills/0").map(String::as_str), Some("VariableID:9:9"));
    assert_eq!(report.missing_variable_bindings, 0);
}
