//! The design-system scan on in-memory documents: one test per rule of the scan file. Each test builds a document
//! where the rule decides the answer, so dropping the rule from the scan turns it red.

mod common;

use common::scan_world::*;
use designos_doc::*;
use serde_json::json;

fn color(r: f32) -> VariableValue {
    VariableValue::Color(Color { r, g: 0.0, b: 0.0, a: 1.0 })
}

// ---------------------------------------------------------------- ids

#[test]
fn ids_are_the_figma_ids_kept_from_the_file_never_studio_node_ids() {
    let mut w = World::new();
    let page = w.page("Kit", "0:1");
    let comp = w.component(page, "Button", "900:7");
    let home_frame = w.frame(page, "Home", "901:8");
    w.collection("VariableCollectionId:1:1", "Tokens", None, &[("1:2", "Light")], &[("VariableID:1:3", "brand", color(1.0))]);
    assert!(comp.0 < 900 && home_frame.0 < 900, "precondition: Studio's own ids are small numbers, unlike the Figma ids");
    let scan = scan_json(&w.doc);
    assert_eq!(component(&scan, "Button")["id"], "900:7");
    assert_eq!(component(&scan, "Button")["page"], json!({"id": "0:1", "name": "Kit"}));
    assert_eq!(frame(&scan, "Home")["id"], "901:8");
    assert_eq!(scan["tokens"][0]["id"], "VariableID:1:3");
}

#[test]
fn a_component_with_no_figma_id_is_refused_by_name_not_given_a_studio_id() {
    let mut w = World::new();
    let page = w.page("Kit", "0:1");
    let comp = w.component(page, "Button", "900:7");
    w.doc.get_mut(comp).unwrap().source_id = None;
    let err = designos_fig::scan_design_system(&w.doc).unwrap_err().to_string();
    assert!(err.contains("Button") && err.contains(".fig"), "{err}");
}

// ---------------------------------------------------------------- tokens

#[test]
fn tokens_hold_the_variables_of_local_collections_only() {
    let mut w = World::new();
    w.page("Kit", "0:1");
    w.collection(
        "c-local",
        "Local tokens",
        None,
        &[("m1", "Light"), ("m2", "Dark")],
        &[("v-a", "color/a", color(1.0)), ("v-b", "space/4", VariableValue::Number(4.0))],
    );
    w.collection("c-remote", "Library tokens", Some("lib-key"), &[("m3", "Default")], &[("v-remote", "color/remote", color(0.5))]);
    w.collection("c-gone", "Deleted", None, &[("m4", "Default")], &[("v-gone", "color/gone", color(0.2))]);
    w.doc.variable_collections.get_mut("c-gone").unwrap().extensions.insert(SOFT_DELETED_EXTENSION_KEY.into(), json!(true));
    w.doc.variables.get_mut("v-b").unwrap().extensions.insert(SOFT_DELETED_EXTENSION_KEY.into(), json!(true));
    let scan = scan_json(&w.doc);
    let ids: Vec<&str> = scan["tokens"].as_array().unwrap().iter().map(|t| t["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["v-a"], "no variable of a remote or deleted collection, and no deleted variable");
    assert_eq!(scan["counts"]["tokens"], 1);
}

#[test]
fn a_token_has_its_collection_name_its_type_and_a_value_for_each_mode_by_mode_name() {
    let mut w = World::new();
    w.page("Kit", "0:1");
    w.collection("c1", "Theme", None, &[("m-light", "Light"), ("m-dark", "Dark")], &[("v1", "color/bg", VariableValue::Alias("v0".into()))]);
    w.doc.variables.get_mut("v1").unwrap().values.insert("m-dark".into(), VariableValue::Color(Color { r: 0.2, g: 0.5, b: 1.0, a: 1.0 }));
    let scan = scan_json(&w.doc);
    assert_eq!(
        scan["tokens"][0],
        json!({
            "id": "v1", "name": "color/bg", "type": "COLOR", "collection": "Theme",
            "valuesByMode": {"Light": {"type": "VARIABLE_ALIAS", "id": "v0"}, "Dark": {"r": 0.2, "g": 0.5, "b": 1, "a": 1}},
        })
    );
    // The kernel falls back to the first mode in the object when none is named Light or Default: it is the declared first.
    let text = designos_fig::scan_design_system(&w.doc).unwrap().to_json_text();
    assert!(text.find("\"Light\"").unwrap() < text.find("\"Dark\"").unwrap(), "modes keep their declared order");
}

// ---------------------------------------------------------------- the instance boundary

/// A component whose own nodes bind `own`, whose only instance binds `on_instance` and holds, inside its copy of the
/// main component, a node binding `inside` and a nested instance.
fn boundary_world() -> (World, NodeId, NodeId) {
    let mut w = World::new();
    let page = w.page("Kit", "0:1");
    let inner_main = w.component(page, "Inner", "10:1");
    let nested_main = w.component(page, "Nested", "10:2");
    let card = w.component(page, "Card", "10:3");
    let group = w.add(card, NodeKind::Group, "Group", "10:4");
    let body = w.frame(group, "Body", "10:5");
    w.bind(body, "fills/0", "var-own");
    let inst = w.instance(body, "Inner instance", "10:6", Some(inner_main));
    w.bind(inst, "fills/0", "var-on-instance");
    let inside = w.frame(inst, "Inside", "10:7");
    w.bind(inside, "fills/0", "var-inside");
    w.instance(inside, "Nested instance", "10:8", Some(nested_main));
    let screens = w.page("Screens", "0:2");
    let home = w.frame(screens, "Home", "20:1");
    let home_inst = w.instance(home, "Card", "20:2", Some(card));
    w.bind(home_inst, "fills/0", "var-home-instance");
    let hi_inside = w.frame(home_inst, "Inside", "20:3");
    w.bind(hi_inside, "fills/0", "var-home-inside");
    w.instance(hi_inside, "Nested", "20:4", Some(nested_main));
    (w, card, home)
}

#[test]
fn the_walk_stops_at_an_instance_for_bound_variables_in_components_and_frames() {
    let (w, _, _) = boundary_world();
    let scan = scan_json(&w.doc);
    assert_eq!(
        component(&scan, "Card")["boundVariables"],
        json!(["var-on-instance", "var-own"]),
        "the instance node itself is read; what is inside it belongs to its own component"
    );
    // A frame's bindings are not listed: the scan reports them per component, and a frame has instances only.
    assert!(frame(&scan, "Home").get("boundVariables").is_none());
}

#[test]
fn the_walk_stops_at_an_instance_for_the_instances_of_components_and_frames() {
    let (w, _, _) = boundary_world();
    let scan = scan_json(&w.doc);
    assert_eq!(component(&scan, "Card")["instances"], json!([{"component": "10:1", "count": 1}]), "the nested instance is not counted");
    assert_eq!(frame(&scan, "Home")["instances"], json!([{"component": "10:3", "count": 1}]), "the nested instance is not counted");
}

// ---------------------------------------------------------------- bound variables of a component

#[test]
fn bound_variables_come_from_every_place_a_binding_lives_sorted_and_unique() {
    let mut w = World::new();
    let page = w.page("Kit", "0:1");
    let comp = w.component(page, "Chip", "1:1");
    let label = w.add(
        comp,
        NodeKind::Text(TextProps {
            content: "x".into(),
            style: TextStyle::default(),
            runs: vec![],
            auto_resize: TextAutoResize::WidthAndHeight,
            max_lines: None,
            ..Default::default()
        }),
        "Label",
        "1:2",
    );
    for (node, key, var) in [
        (comp, "fills/0", "v-fill"),
        (comp, "fills/1/stops/0", "v-gradient-stop"),
        (comp, "strokes/0", "v-stroke"),
        (comp, "effects/0/color", "v-effect"),
        (comp, "layoutGrids/0/color", "v-grid"),
        (comp, "itemSpacing", "v-spacing"),
        (comp, "componentProperties/Show icon", "v-property"),
        (label, "fontSize", "v-text"),
        (label, "fills/0", "v-fill"),
    ] {
        w.bind(node, key, var);
    }
    // A variable of a remote collection is listed like any other.
    w.collection("c-remote", "Library", Some("lib"), &[("m", "Default")], &[("v-remote", "remote", color(0.1))]);
    w.bind(label, "fills/1", "v-remote");
    let scan = scan_json(&w.doc);
    assert_eq!(
        component(&scan, "Chip")["boundVariables"],
        json!(["v-effect", "v-fill", "v-gradient-stop", "v-grid", "v-property", "v-remote", "v-spacing", "v-stroke", "v-text"])
    );
}

#[test]
fn a_component_set_takes_the_union_of_its_variants_bound_variables() {
    let mut w = World::new();
    let page = w.page("Kit", "0:1");
    let set = w.component_set(page, "Button", "2:1");
    let small = w.component(set, "Size=S", "2:2");
    let large = w.component(set, "Size=L", "2:3");
    w.bind(small, "fills/0", "v-small");
    w.bind(large, "fills/0", "v-large");
    w.bind(large, "itemSpacing", "v-shared");
    w.bind(small, "itemSpacing", "v-shared");
    let scan = scan_json(&w.doc);
    assert_eq!(component(&scan, "Button")["boundVariables"], json!(["v-large", "v-shared", "v-small"]));
    assert_eq!(scan["counts"]["components"], 1, "the variants are covered by their set");
}

// ---------------------------------------------------------------- instances of a component

#[test]
fn a_component_set_sums_the_instances_of_its_variants_and_each_resolves_to_a_set_or_a_component() {
    let mut w = World::new();
    let page = w.page("Kit", "0:1");
    let icon = w.component(page, "Icon", "3:1");
    let badge_set = w.component_set(page, "Badge", "3:2");
    let badge_a = w.component(badge_set, "Tone=A", "3:3");
    let badge_b = w.component(badge_set, "Tone=B", "3:4");
    let button_set = w.component_set(page, "Button", "3:5");
    let small = w.component(button_set, "Size=S", "3:6");
    let large = w.component(button_set, "Size=L", "3:7");
    for (variant, copies) in [(small, 2), (large, 1)] {
        let slot = w.frame(variant, "Row", "3:90");
        for i in 0..copies {
            w.instance(slot, "Icon", &format!("3:{}", 100 + i), Some(icon));
        }
        w.instance(slot, "Badge A", "3:110", Some(badge_a));
    }
    let extra = w.frame(large, "Extra", "3:91");
    w.instance(extra, "Badge B", "3:111", Some(badge_b));
    let scan = scan_json(&w.doc);
    assert_eq!(
        component(&scan, "Button")["instances"],
        json!([{"component": "3:1", "count": 3}, {"component": "3:2", "count": 3}]),
        "icons: 2 + 1; badge variants A and B both resolve to the set 3:2: 1 + 1 + 1"
    );
    assert_eq!(component(&scan, "Icon")["instances"], json!([]));
}

// ---------------------------------------------------------------- frames

#[test]
fn frames_are_direct_children_of_a_page_or_of_a_section_at_any_depth_and_name_the_innermost_section() {
    let mut w = World::new();
    let page = w.page("Screens", "0:2");
    w.frame(page, "Top", "4:1");
    let outer = w.section(page, "Outer", "4:2");
    w.frame(outer, "In outer", "4:3");
    let inner = w.section(outer, "Inner", "4:4");
    w.frame(inner, "In inner", "4:5");
    let deep = w.section(inner, "Deep", "4:6");
    w.frame(deep, "In deep", "4:7");
    w.frame(outer, "After inner", "4:8");
    // Not frames of the page: one inside a frame, one inside a group, and the main components.
    let top = w.frame(page, "Holder", "4:9");
    w.frame(top, "Nested frame", "4:10");
    let group = w.add(page, NodeKind::Group, "Group", "4:11");
    w.frame(group, "In group", "4:12");
    w.component(page, "Comp", "4:13");
    w.component_set(page, "Set", "4:14");
    let scan = scan_json(&w.doc);
    let seen: Vec<(String, Option<String>)> = scan["frames"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| (f["name"].as_str().unwrap().to_string(), f.get("section").map(|s| s.as_str().unwrap().to_string())))
        .collect();
    let named = |n: &str, s: Option<&str>| (n.to_string(), s.map(String::from));
    assert_eq!(
        seen,
        [
            named("Top", None),
            named("In outer", Some("Outer")),
            named("In inner", Some("Inner")),
            named("In deep", Some("Deep")),
            named("After inner", Some("Outer")),
            named("Holder", None),
        ],
        "in document order, the section being the innermost enclosing one"
    );
    assert_eq!(frame(&scan, "Top")["page"], json!({"id": "0:2", "name": "Screens"}));
}

#[test]
fn a_frame_has_its_size_to_two_decimals_and_counts_hidden_instances() {
    let mut w = World::new();
    let page = w.page("Screens", "0:2");
    let comp = w.component(page, "Icon", "5:1");
    let home = w.frame(page, "Home", "5:2");
    w.doc.get_mut(home).unwrap().size = Vec2::new(375.0, 812.126);
    let shown = w.instance(home, "Shown", "5:3", Some(comp));
    let hidden = w.instance(home, "Hidden", "5:4", Some(comp));
    w.doc.get_mut(hidden).unwrap().visible = false;
    let _ = shown;
    let scan = scan_json(&w.doc);
    let home = frame(&scan, "Home");
    assert_eq!((home["w"].clone(), home["h"].clone()), (json!(375), json!(812.13)));
    assert_eq!(home["instances"], json!([{"component": "5:1", "count": 2}]), "the host senses, the kernel judges");
}

// ---------------------------------------------------------------- resolving an instance

#[test]
fn an_instance_that_cannot_be_resolved_has_no_component_and_is_counted_apart() {
    let mut w = World::new();
    let page = w.page("Screens", "0:2");
    let comp = w.component(page, "Icon", "6:1");
    let home = w.frame(page, "Home", "6:2");
    w.instance(home, "Fine", "6:3", Some(comp));
    w.instance(home, "No main", "6:4", None);
    w.instance(home, "Main gone", "6:5", Some(NodeId(987_654)));
    // A main component the document no longer holds under any parent names nothing.
    let orphan = w.doc.alloc_id();
    let mut orphan_node = Node::new(orphan, NodeKind::Component(FrameProps::default()));
    orphan_node.source_id = Some("6:90".into());
    w.doc.nodes.insert(orphan, orphan_node);
    w.instance(home, "Removed main", "6:6", Some(orphan));
    // Something that is not a component at all.
    let plain = w.frame(page, "Plain", "6:7");
    w.instance(home, "Points at a frame", "6:8", Some(plain));
    let scan = scan_json(&w.doc);
    let home = frame(&scan, "Home");
    assert_eq!(home["instances"], json!([{"component": "6:1", "count": 1}]), "no entry without a component");
    assert_eq!(home["unresolvedInstances"], 4);
    // Written only above zero.
    let clean = frame(&scan, "Plain");
    assert!(clean.get("unresolvedInstances").is_none(), "{clean}");
}

#[test]
fn a_library_component_resolves_to_the_key_of_its_set_else_its_own_key() {
    let mut w = World::new();
    let screens = w.page("Screens", "0:2");
    let lib = ensure_library_page(&mut w.doc);
    // A set with a key: its variant resolves to the set's key.
    let set = w.component_set(lib, "Button", "7:1");
    w.doc.get_mut(set).unwrap().fig_metadata.insert("componentKey".into(), "aaaa-set".into());
    let variant = w.component(set, "Size=S", "7:2");
    w.doc.get_mut(variant).unwrap().fig_metadata.insert("componentKey".into(), "bbbb-variant".into());
    // A set with no key: the variant's own key.
    let keyless_set = w.component_set(lib, "Chip", "7:3");
    let keyless_variant = w.component(keyless_set, "Tone=A", "7:4");
    w.doc.get_mut(keyless_variant).unwrap().fig_metadata.insert("componentKey".into(), "cccc-variant".into());
    // A plain component: its own key.
    let plain = w.component(lib, "Icon", "7:5");
    w.doc.get_mut(plain).unwrap().fig_metadata.insert("componentKey".into(), "dddd-plain".into());
    // A stand-in of a component from another file: the key it is listed under.
    let stand_in = w.component(lib, "Remote", "7:6");
    w.doc.library.external.insert(
        local_key("remote:file", "eeee-remote"),
        ExternalAsset { library_id: "remote:file".into(), version: 0, kind: LibraryAssetKind::Component, node: Some(stand_in), local_key: None },
    );
    // An unpublished symbol or set on the internal canvas has no component key, so it resolves to its Figma source id.
    let unpub_set = w.component_set(lib, "InternalSet", "7:7");
    let unpub_variant = w.component(unpub_set, "Size=M", "7:8");
    let unpub_plain = w.component(lib, "InternalPlain", "7:9");
    // A library component with neither a key nor a Figma source id cannot be named.
    let nameless = w.component(lib, "Nameless", "7:10");
    w.doc.get_mut(nameless).unwrap().source_id = None;
    let home = w.frame(screens, "Home", "7:11");
    for (i, main) in [variant, keyless_variant, plain, stand_in, unpub_variant, unpub_plain, nameless].into_iter().enumerate() {
        w.instance(home, "i", &format!("7:{}", 20 + i), Some(main));
    }
    let scan = scan_json(&w.doc);
    let home = frame(&scan, "Home");
    assert_eq!(
        home["instances"],
        json!([
            {"component": "7:7", "count": 1},
            {"component": "7:9", "count": 1},
            {"component": "aaaa-set", "count": 1},
            {"component": "cccc-variant", "count": 1},
            {"component": "dddd-plain", "count": 1},
            {"component": "eeee-remote", "count": 1},
        ])
    );
    assert_eq!(home["unresolvedInstances"], 1);
    // The library page is not a page of the file: nothing on it is scanned as the file's own.
    assert_eq!(scan["components"], json!([]));
}

// ---------------------------------------------------------------- order and determinism

#[test]
fn the_output_is_sorted_and_byte_identical_between_runs_and_carries_no_time() {
    let mut w = World::new();
    let page = w.page("Kit", "0:1");
    let c9 = w.component(page, "Nine", "9:1");
    let c10 = w.component(page, "Ten", "10:1");
    let host = w.component(page, "Host", "11:1");
    for main in [c10, c9, c10] {
        w.instance(host, "i", "11:9", Some(main));
    }
    for (key, var) in [("fills/0", "v-b"), ("strokes/0", "v-a"), ("effects/0/color", "v-c")] {
        w.bind(host, key, var);
    }
    let first = designos_fig::scan_design_system(&w.doc).unwrap().to_json_text();
    let second = designos_fig::scan_design_system(&w.doc).unwrap().to_json_text();
    assert_eq!(first, second, "two runs on one document are byte-identical");
    let scan: serde_json::Value = serde_json::from_str(&first).unwrap();
    assert_eq!(component(&scan, "Host")["boundVariables"], json!(["v-a", "v-b", "v-c"]));
    assert_eq!(
        component(&scan, "Host")["instances"],
        json!([{"component": "10:1", "count": 2}, {"component": "9:1", "count": 1}]),
        "sorted by the component text, as the plugin sorts"
    );
    for forbidden in ["time", "date", "ms\"", "elapsed", "duration"] {
        assert!(!first.to_lowercase().contains(forbidden), "no timestamp and no timing in the output: {forbidden}");
    }
}

// ---------------------------------------------------------------- the fields that exist today

#[test]
fn the_fields_the_plugin_already_writes_keep_their_names_and_shape() {
    let mut w = World::new();
    let page = w.page("Kit", "0:1");
    let set = w.component_set(page, "Button", "8:1");
    w.doc.get_mut(set).unwrap().fig_metadata.insert("componentKey".into(), "key-set".into());
    w.doc.get_mut(set).unwrap().component.props.push(ComponentPropDef {
        name: "Size".into(),
        kind: PropKind::Variant { options: vec!["Small".into(), "Large".into()] },
        default: PropValue::Variant("Small".into()),
    });
    w.doc.get_mut(set).unwrap().component.props.push(ComponentPropDef { name: "Label".into(), kind: PropKind::Text, default: PropValue::Text("x".into()) });
    w.component(set, "Size=Small", "8:2");
    let plain = w.component(page, "Icon", "8:3");
    let _ = plain;
    w.collection("c1", "Tokens", None, &[("m1", "Light")], &[("v1", "space/4", VariableValue::Number(4.0))]);
    let mut scan = scan_json(&w.doc);
    // Drop the keys this feature adds: what is left is what the plugin writes today.
    for c in scan["components"].as_array_mut().unwrap() {
        c.as_object_mut().unwrap().remove("boundVariables");
        c.as_object_mut().unwrap().remove("instances");
    }
    let sensing_keys: Vec<String> = ["frames", "sensing"].iter().map(|s| s.to_string()).collect();
    for k in &sensing_keys {
        scan.as_object_mut().unwrap().remove(k);
    }
    assert_eq!(
        scan,
        json!({
            "components": [
                {"id": "8:1", "key": "key-set", "name": "Button", "type": "COMPONENT_SET", "page": {"id": "0:1", "name": "Kit"},
                 "variantAxes": {"Size": ["Small", "Large"]}, "width": 100},
                {"id": "8:3", "name": "Icon", "type": "COMPONENT", "page": {"id": "0:1", "name": "Kit"}, "width": 100},
            ],
            "tokens": [{"id": "v1", "name": "space/4", "type": "FLOAT", "collection": "Tokens", "valuesByMode": {"Light": 4}}],
            "styles": [],
            "counts": {"components": 2, "tokens": 1, "styles": 0},
        })
    );
}

#[test]
fn components_carry_their_width_and_their_innermost_enclosing_section_when_inside_one() {
    let mut w = World::new();
    let page = w.page("Kit", "0:1");
    let top = w.component(page, "Hero", "8:10");
    w.doc.get_mut(top).unwrap().size = Vec2::new(1440.0, 900.0);
    let outer = w.section(page, "01 · Atoms", "8:11");
    let in_outer = w.component(outer, "Badge", "8:12");
    w.doc.get_mut(in_outer).unwrap().size = Vec2::new(64.5, 24.0);
    let inner = w.section(outer, "00 · Icons", "8:13");
    let holder = w.frame(inner, "Grid", "8:14");
    let in_inner = w.component(holder, "Arrow", "8:15");
    w.doc.get_mut(in_inner).unwrap().size = Vec2::new(24.0, 24.0);
    let set = w.component_set(inner, "Button", "8:16");
    w.doc.get_mut(set).unwrap().size = Vec2::new(240.0, 48.0);
    w.component(set, "Size=M", "8:17");

    let scan = scan_json(&w.doc);
    let hero = component(&scan, "Hero");
    assert!(hero.get("section").is_none(), "omitted when not inside a section: {hero}");
    assert_eq!(hero["width"], json!(1440));
    assert_eq!((component(&scan, "Badge")["section"].clone(), component(&scan, "Badge")["width"].clone()), (json!("01 · Atoms"), json!(64.5)));
    assert_eq!((component(&scan, "Arrow")["section"].clone(), component(&scan, "Arrow")["width"].clone()), (json!("00 · Icons"), json!(24)));
    assert_eq!((component(&scan, "Button")["section"].clone(), component(&scan, "Button")["width"].clone()), (json!("00 · Icons"), json!(240)));
}

#[test]
fn local_styles_are_listed_as_paint_text_and_effect_without_library_or_grid_styles() {
    let mut w = World::new();
    w.page("Kit", "0:1");
    let meta = |name: &str, property: StyleProperty| StyleMeta { name: name.into(), property, ..Default::default() };
    for (id, style, m) in [
        ("30:1", Style::Effect(vec![]), meta("Shadow/Card", StyleProperty::Effect)),
        ("30:2", Style::Text(TextStyle::default()), meta("Body", StyleProperty::Text)),
        ("30:3", Style::Fill(vec![]), meta("Brand", StyleProperty::Fill)),
        ("30:4", Style::Fill(vec![]), meta("Hairline", StyleProperty::Stroke)),
        ("30:5", Style::Grid(vec![]), meta("12 columns", StyleProperty::Grid)),
        ("key:abc", Style::Fill(vec![]), meta("From a library", StyleProperty::Fill)),
    ] {
        w.doc.styles.insert(id.into(), style);
        w.doc.style_meta.insert(id.into(), m);
    }
    // A style a node points at but the file never defined has no entry of its own.
    w.doc.styles.insert("30:9".into(), Style::Fill(vec![]));
    let scan = scan_json(&w.doc);
    assert_eq!(
        scan["styles"],
        json!([
            {"id": "30:3", "name": "Brand", "type": "PAINT"},
            {"id": "30:4", "name": "Hairline", "type": "PAINT"},
            {"id": "30:2", "name": "Body", "type": "TEXT"},
            {"id": "30:1", "name": "Shadow/Card", "type": "EFFECT"},
        ])
    );
    assert_eq!(scan["counts"]["styles"], 4);
}

// ---------------------------------------------------------------- sensing never loses the rest

#[test]
fn a_failure_of_the_sensing_step_keeps_every_other_field_and_says_so() {
    let mut w = World::new();
    let page = w.page("Screens", "0:2");
    let comp = w.component(page, "Icon", "9:1");
    w.bind(comp, "fills/0", "v1");
    w.collection("c1", "Tokens", None, &[("m1", "Light")], &[("v1", "brand", color(1.0))]);
    let complete = scan_json(&w.doc);
    assert_eq!(complete["sensing"], json!({"status": "complete"}));

    // A frame with no Figma id cannot be named: sensing is skipped, the rest is what it was.
    let broken = w.frame(page, "Draft", "9:2");
    w.doc.get_mut(broken).unwrap().source_id = None;
    let skipped = scan_json(&w.doc);
    assert_eq!(skipped["sensing"]["status"], "skipped");
    assert!(skipped["sensing"]["reason"].as_str().unwrap().contains("Draft"), "{}", skipped["sensing"]);
    assert!(skipped.get("frames").is_none(), "no partial list");
    assert!(component(&skipped, "Icon").get("boundVariables").is_none() && component(&skipped, "Icon").get("instances").is_none());
    for key in ["tokens", "styles", "counts"] {
        assert_eq!(skipped[key], complete[key], "{key} is what the scan gave before sensing existed");
    }
    let mut bare = component(&complete, "Icon").clone();
    bare.as_object_mut().unwrap().remove("boundVariables");
    bare.as_object_mut().unwrap().remove("instances");
    assert_eq!(component(&skipped, "Icon"), &bare);
}

#[test]
fn a_document_with_no_variables_and_no_frames_has_empty_lists_and_a_complete_sensing() {
    let mut w = World::new();
    w.page("Empty", "0:1");
    assert_eq!(
        scan_json(&w.doc),
        json!({
            "components": [], "tokens": [], "styles": [], "frames": [],
            "counts": {"components": 0, "tokens": 0, "styles": 0},
            "sensing": {"status": "complete"},
        })
    );
}

// ---------------------------------------------------------------- unresolved, definitions in frames, self-reference

#[test]
fn a_component_and_a_set_report_their_unresolved_instances_written_only_above_zero() {
    let mut w = World::new();
    let page = w.page("Kit", "0:1");
    let icon = w.component(page, "Icon", "11:1");
    let card = w.component(page, "Card", "11:2");
    w.instance(card, "Fine", "11:3", Some(icon));
    w.instance(card, "Lost", "11:4", None);
    let set = w.component_set(page, "Button", "11:5");
    let small = w.component(set, "Size=S", "11:6");
    let large = w.component(set, "Size=L", "11:7");
    w.instance(small, "Lost", "11:8", None);
    w.instance(large, "Gone", "11:9", Some(NodeId(987_654)));
    w.instance(large, "Fine", "11:10", Some(icon));
    let scan = scan_json(&w.doc);
    assert_eq!(component(&scan, "Card")["unresolvedInstances"], 1);
    assert_eq!(component(&scan, "Card")["instances"], json!([{"component": "11:1", "count": 1}]), "no entry without a component");
    assert_eq!(component(&scan, "Button")["unresolvedInstances"], 2, "a set sums its variants");
    assert!(component(&scan, "Icon").get("unresolvedInstances").is_none(), "written only when above 0");
}

#[test]
fn a_frame_walk_neither_counts_nor_enters_a_component_or_set_it_meets() {
    let mut w = World::new();
    let page = w.page("Screens", "0:2");
    let icon = w.component(page, "Icon", "12:1");
    let home = w.frame(page, "Home", "12:2");
    w.instance(home, "Direct", "12:3", Some(icon));
    let inner = w.component(home, "Inner main", "12:4");
    w.instance(inner, "Inside the component", "12:5", Some(icon));
    w.instance(inner, "Lost inside the component", "12:6", None);
    let inner_set = w.component_set(home, "Inner set", "12:7");
    let variant = w.component(inner_set, "A=1", "12:8");
    w.instance(variant, "Inside the set", "12:9", Some(icon));
    let scan = scan_json(&w.doc);
    let home = frame(&scan, "Home");
    assert_eq!(home["instances"], json!([{"component": "12:1", "count": 1}]), "only the direct instance");
    assert!(home.get("unresolvedInstances").is_none(), "{home}");
}

#[test]
fn a_component_or_set_never_lists_itself_in_its_own_instances() {
    let mut w = World::new();
    let page = w.page("Kit", "0:1");
    let icon = w.component(page, "Icon", "13:1");
    let set = w.component_set(page, "Button", "13:2");
    let small = w.component(set, "Size=S", "13:3");
    let large = w.component(set, "Size=L", "13:4");
    w.instance(large, "Uses its sibling", "13:5", Some(small));
    w.instance(large, "Uses the icon", "13:6", Some(icon));
    let selfish = w.component(page, "Selfish", "13:7");
    w.instance(selfish, "Itself", "13:8", Some(selfish));
    let scan = scan_json(&w.doc);
    assert_eq!(component(&scan, "Button")["instances"], json!([{"component": "13:1", "count": 1}]), "the sibling variant resolves to the set itself");
    assert_eq!(component(&scan, "Selfish")["instances"], json!([]));
}
