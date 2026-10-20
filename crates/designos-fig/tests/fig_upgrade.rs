//! `.fig` import of geometry, styles, grids, text clamping, line height and instance overrides.
use designos_doc::*;
use designos_fig::container::Compression;
use designos_fig::testkit::upgrade_fig;
use designos_fig::{export_rest_json, import_fig, import_rest_json};

fn doc() -> Document {
    import_fig(&upgrade_fig(Compression::Zstd)).unwrap()
}

fn find<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

fn child<'a>(doc: &'a Document, parent: &Node, name: &str) -> &'a Node {
    parent.children.iter().map(|c| doc.get(*c).unwrap()).find(|n| n.name == name).unwrap_or_else(|| panic!("{} has no child {name}", parent.name))
}

fn text(n: &Node) -> &TextProps {
    match &n.kind {
        NodeKind::Text(t) => t,
        _ => panic!("{} is not text", n.name),
    }
}

fn solid(n: &[Paint]) -> Color {
    match n[0].kind {
        PaintKind::Solid(c) => c,
        _ => panic!("not solid"),
    }
}

#[test]
fn fill_and_stroke_geometry_from_blobs() {
    let d = doc();
    let b = find(&d, "Bool");
    let fg = b.fill_geometry.as_ref().expect("fill geometry on boolean op");
    assert_eq!(fg[0].fill_rule, FillRule::NonZero);
    assert_eq!(fg[0].cmds.len(), 4); // M L L Z
    let sg = b.stroke_geometry.as_ref().expect("stroke geometry on boolean op");
    assert_eq!(sg[0].fill_rule, FillRule::EvenOdd, "kiwi WindingRule ODD is even-odd");
    assert_eq!(sg[0].cmds.len(), 5); // M L L L Z
    assert!(find(&d, "Star").fill_geometry.is_some());
    assert!(find(&d, "Star").stroke_geometry.is_none());
    assert!(find(&d, "Card").fill_geometry.is_none());
}

#[test]
fn style_nodes_become_style_meta() {
    let d = doc();
    let bool_node = find(&d, "Bool");
    let fill = &bool_node.style_refs["fill"];
    let stroke = &bool_node.style_refs["stroke"];
    assert_eq!(fill, "2:100");
    assert_eq!(
        d.style_meta[fill],
        StyleMeta {
            name: "Brand/Primary".into(),
            key: Some("key-primary".into()),
            description: "Main brand fill".into(),
            property: StyleProperty::Fill,
            bindings: Default::default(),
        }
    );
    // A FILL style used only through styleIdForStrokeFill is a stroke style.
    assert_eq!(d.style_meta[stroke].property, StyleProperty::Stroke);
    assert_eq!(d.style_meta[stroke].key, None);
    assert_eq!(d.style_name(stroke), "Border/Strong");
    let text_style = &find(&d, "Clamp").style_refs["text"];
    assert_eq!(d.style_meta[text_style].property, StyleProperty::Text);
    assert_eq!(d.style_meta[text_style].description, "Body copy");
    assert!(matches!(d.styles[text_style], Style::Text(_)));
}

#[test]
fn linear_blend_modes_map_in_fig() {
    let d = doc();
    assert_eq!(find(&d, "Bool").blend, BlendMode::LinearBurn);
    assert_eq!(find(&d, "Star").blend, BlendMode::LinearDodge);
    assert_eq!(child(&d, find(&d, "Card A"), "Bg").blend, BlendMode::LinearDodge, "via an instance override");
}

#[test]
fn text_truncation_and_max_lines() {
    let d = doc();
    let t = text(find(&d, "Clamp"));
    assert_eq!(t.auto_resize, TextAutoResize::Truncate);
    assert_eq!(t.max_lines, Some(2));
    let p = text(find(&d, "Plain"));
    assert_eq!((p.auto_resize, p.max_lines), (TextAutoResize::None, None));
}

#[test]
fn line_height_semantics() {
    let d = doc();
    let lh = |n: &str| text(find(&d, n)).style.line_height;
    assert_eq!(lh("Clamp"), LineHeight::Percent(100.0), "PERCENT 100 is 100% of the font size, not auto");
    assert_eq!(lh("LhPct"), LineHeight::Percent(150.0));
    assert_eq!(lh("LhAuto"), LineHeight::Auto, "RAW 1 is auto");
    assert_eq!(lh("LhNone"), LineHeight::Auto, "absent is auto");
    assert_eq!(lh("LhPx"), LineHeight::Pixels(18.0));
}

#[test]
fn layout_grids() {
    let d = doc();
    let g = &find(&d, "Grids").kind.frame_props().unwrap().grids;
    assert_eq!(g.len(), 3, "the invisible rows grid is preserved with visible=false");
    assert!(
        matches!(&g[0], LayoutGrid::Columns { count: 4, gutter, offset, color, visible: true, .. } if *gutter == 8.0 && *offset == 2.0 && (color.a - 0.1).abs() < 1e-6)
    );
    assert!(matches!(&g[1], LayoutGrid::Grid { size, visible: true, .. } if *size == 10.0));
    assert!(matches!(&g[2], LayoutGrid::Rows { count: 3, gutter, visible: false, .. } if *gutter == 4.0));
}

#[test]
fn instance_overrides_apply_to_the_copy_only() {
    let d = doc();
    let comp = find(&d, "Card");
    assert_eq!(text(child(&d, comp, "Label")).content, "Old", "the component is untouched");

    let a = find(&d, "Card A");
    assert_eq!(a.children.len(), 3);
    let label = child(&d, a, "Label");
    let t = text(label);
    assert_eq!(t.content, "New \u{1F600}");
    assert_eq!(solid(&t.style.fills), Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 });
    assert!((t.style.letter_spacing - 5.0).abs() < 1e-4, "50% of the target's 10px font");
    assert_eq!(label.opacity, 0.75);
    assert_eq!(label.source_id.as_deref(), Some("I2:40;2:11"));

    let bg = child(&d, a, "Bg");
    assert!(!bg.visible);
    assert_eq!(bg.corner_radii, [4.0; 4]);
    // derivedSymbolData: size/transform/geometry, but never names.
    assert_eq!(bg.size, Vec2::new(200.0, 80.0));
    assert_eq!(bg.transform.translation(), Vec2::new(5.0, 6.0));
    assert!(bg.fill_geometry.is_some());

    let inner = child(&d, a, "Inner");
    let dot = child(&d, inner, "Dot");
    assert_eq!(dot.size, Vec2::new(7.0, 7.0));
    assert_eq!(solid(&dot.fills), Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 });
    assert_eq!(comp_dot(&d).size, Vec2::new(5.0, 5.0));

    // Another instance of the same component is independent.
    let b = find(&d, "Card B");
    assert_eq!(text(child(&d, b, "Label")).content, "Old");
    assert!(child(&d, b, "Bg").visible);
}

fn comp_dot(d: &Document) -> &Node {
    let comp = find(d, "Card");
    child(d, child(d, comp, "Inner"), "Dot")
}

#[test]
fn nested_instances_resolve_multi_step_paths() {
    let d = doc();
    // The wrapper's own nested instance carries its override ...
    let wrapper = find(&d, "Wrapper");
    let card_in = child(&d, wrapper, "CardIn");
    assert_eq!(text(child(&d, card_in, "Label")).content, "Inner text");
    // ... and an instance of the wrapper can override through two levels.
    let wi = find(&d, "WrapInst");
    let copy_in = child(&d, wi, "CardIn");
    assert!(matches!(copy_in.kind, NodeKind::Instance { .. }));
    assert_eq!(text(child(&d, copy_in, "Label")).content, "Deep");
    // The wrapper's nested copy is not affected by the outer override.
    assert_eq!(text(child(&d, card_in, "Label")).content, "Inner text");
}

#[test]
fn ids_are_deterministic_across_imports() {
    let bytes = upgrade_fig(Compression::Deflate);
    let a = import_fig(&bytes).unwrap();
    let b = import_fig(&bytes).unwrap();
    assert_eq!(a.nodes.keys().collect::<Vec<_>>(), b.nodes.keys().collect::<Vec<_>>());
    let map = |d: &Document| d.nodes.iter().map(|(k, n)| (*k, n.source_id.clone(), n.name.clone(), n.parent)).collect::<Vec<_>>();
    assert_eq!(map(&a), map(&b));
    assert_eq!(a.nodes, b.nodes);
    // Clone ids are dense and above every imported id; further allocation does not collide.
    let mut a = a;
    let fresh = a.alloc_id();
    assert!(!a.nodes.contains_key(&fresh));
    assert!(fresh > *a.nodes.keys().next_back().unwrap());
    // All instance copies carry the Figma `I<instance>;<node>` source id form.
    assert!(a.nodes.values().any(|n| n.source_id.as_deref().is_some_and(|s| s.starts_with("I2:30;"))));
}

#[test]
fn fig_round_trips_through_rest_export() {
    let d = doc();
    let json = export_rest_json(&d);
    let again = import_rest_json(&json.to_string()).unwrap();
    let visible = |d: &Document| d.nodes.keys().filter(|id| !is_library_node(d, **id)).count();
    assert_eq!(visible(&d), visible(&again));
    assert_eq!(d.style_meta, again.style_meta);
    assert_eq!(json, export_rest_json(&again), "export is a fixed point after re-import");
    let bool_node = find(&again, "Bool");
    assert_eq!(bool_node.fill_geometry, find(&d, "Bool").fill_geometry);
    assert_eq!(bool_node.stroke_geometry, find(&d, "Bool").stroke_geometry);
    assert_eq!(text(find(&again, "Clamp")).max_lines, Some(2));
    assert_eq!(text(find(&again, "Clamp")).auto_resize, TextAutoResize::Truncate);
    assert_eq!(text(find(&again, "Clamp")).style.line_height, LineHeight::Percent(100.0));
    assert_eq!(text(find(&again, "LhAuto")).style.line_height, LineHeight::Auto);
}

#[test]
fn corrupted_override_data_never_panics() {
    let bytes = upgrade_fig(Compression::Deflate);
    let mut x = 0x9E3779B9u32;
    for _ in 0..300 {
        let mut b = bytes.clone();
        for _ in 0..3 {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            let i = x as usize % b.len();
            b[i] ^= (x >> 8) as u8 | 1;
        }
        let _ = import_fig(&b);
    }
}

#[test]
fn instance_children_link_to_their_component_nodes_and_record_overrides() {
    let d = doc();
    let comp = find(&d, "Card");
    let a = find(&d, "Card A");
    assert_eq!(a.component.source, Some(comp.id), "instance root points at its component");
    let label = child(&d, a, "Label");
    assert_eq!(label.component.source, Some(child(&d, comp, "Label").id));
    for f in [OverrideField::Characters, OverrideField::Opacity] {
        assert!(label.component.overrides.contains(&f), "{f:?} in {:?}", label.component.overrides);
    }
    let bg = child(&d, a, "Bg");
    assert_eq!(bg.component.source, Some(child(&d, comp, "Bg").id));
    assert!(bg.component.overrides.contains(&OverrideField::Visible));
    assert!(bg.component.overrides.contains(&OverrideField::CornerRadii));
    assert!(!bg.component.overrides.contains(&OverrideField::Size), "derived sizes are not user overrides");
    // Untouched copies carry sources but no overrides.
    let b = find(&d, "Card B");
    let bl = child(&d, b, "Label");
    assert_eq!(bl.component.source, Some(child(&d, comp, "Label").id));
    assert!(bl.component.overrides.is_empty());
    // Two levels deep: the nested copy mirrors the wrapper's nested instance.
    let wrapper = find(&d, "Wrapper");
    let wi = find(&d, "WrapInst");
    let nested_main = child(&d, wrapper, "CardIn");
    let nested_copy = child(&d, wi, "CardIn");
    assert_eq!(nested_copy.component.source, Some(nested_main.id));
    assert_eq!(child(&d, nested_copy, "Label").component.source, Some(child(&d, nested_main, "Label").id));
    assert!(child(&d, nested_copy, "Label").component.overrides.contains(&OverrideField::Characters));
}

#[test]
fn fig_text_lists_paragraph_settings_and_features() {
    let d = doc();
    let t = text(find(&d, "Listy"));
    assert_eq!(t.content, "one\ntwo\nthree");
    let items = t.paragraph_lists();
    assert_eq!(items[0], Some(ListItem { kind: ListKind::Bullet, level: 0 }), "indentation 1 is the outermost level");
    assert_eq!(items[1], Some(ListItem { kind: ListKind::Numbered, level: 1 }));
    assert_eq!(items[2], None);
    assert_eq!(t.direction, TextDirection::Rtl);
    assert_eq!(t.paragraph.leading_trim, LeadingTrim::CapHeight);
    assert_eq!(t.paragraph.indent, 6.0);
    assert_eq!(t.style.features, vec![("liga".to_string(), 0), ("ss01".to_string(), 1), ("tnum".to_string(), 1)]);
    // plain text layers are unaffected
    let p = text(find(&d, "Plain"));
    assert!(p.lists.is_empty() && p.style.features.is_empty() && p.direction == TextDirection::Auto);
}

#[test]
fn text_lists_survive_rest_export_and_reimport() {
    let d = doc();
    let json = export_rest_json(&d);
    let listy = json.to_string();
    assert!(listy.contains("\"lineTypes\":[\"UNORDERED\",\"ORDERED\",\"NONE\"]"), "{listy}");
    assert!(listy.contains("\"lineIndentations\":[1,2,0]"));
    assert!(listy.contains("\"opentypeFlags\":{\"LIGA\":0,\"SS01\":1,\"TNUM\":1}"));
    let again = import_rest_json(&listy).unwrap();
    let (a, b) = (text(find(&d, "Listy")), text(find(&again, "Listy")));
    assert_eq!(a.lists, b.lists);
    assert_eq!(a.paragraph, b.paragraph);
    assert_eq!(a.direction, b.direction);
    assert_eq!(a.style.features, b.style.features);
    assert_eq!(json, export_rest_json(&again), "still a fixed point");
}

#[test]
fn rest_text_node_reads_figma_line_types() {
    let json = serde_json::json!({
        "name": "f", "document": {"id": "0:0", "type": "DOCUMENT", "children": [{
            "id": "0:1", "name": "p", "type": "CANVAS", "backgroundColor": {"r":1,"g":1,"b":1,"a":1},
            "children": [{
                "id": "1:1", "name": "T", "type": "TEXT", "characters": "a\nb\nc",
                "lineTypes": ["ORDERED", "ORDERED", "NONE"], "lineIndentations": [1, 1, 0],
                "style": {"fontFamily": "Inter", "fontSize": 12, "listSpacing": 4, "hangingList": true,
                          "opentypeFlags": {"TNUM": 1}},
                "absoluteBoundingBox": {"x": 0, "y": 0, "width": 50, "height": 50}
            }]
        }]}
    });
    let d = import_rest_json(&json.to_string()).unwrap();
    let t = text(find(&d, "T"));
    assert_eq!(t.lists.len(), 1);
    assert_eq!(t.list_markers().iter().map(|m| m.map(|m| m.number)).collect::<Vec<_>>(), vec![Some(1), Some(2), None]);
    assert_eq!((t.paragraph.list_spacing, t.paragraph.hanging_list), (4.0, true));
    assert_eq!(t.style.features, vec![("tnum".to_string(), 1)]);
}
