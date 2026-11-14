mod common;
use common::*;
use designos_doc::*;

fn rect_path(x: f32, y: f32, w: f32, h: f32) -> Path {
    let v = Vec2::new;
    Path {
        cmds: vec![PathCmd::MoveTo(v(x, y)), PathCmd::LineTo(v(x + w, y)), PathCmd::LineTo(v(x + w, y + h)), PathCmd::LineTo(v(x, y + h)), PathCmd::Close],
        fill_rule: FillRule::NonZero,
    }
}

#[test]
fn fill_geometry_replaces_derived_shape() {
    let mut t = T::new();
    let r = t.rect(None, Rect::new(10.0, 10.0, 40.0, 40.0), RED);
    // only the left half is covered by the precomputed outline
    t.node(r).fill_geometry = Some(vec![rect_path(0.0, 0.0, 20.0, 40.0)]);
    let pm = t.render(60, 60);
    assert_eq!(px(&pm, 15, 30), [255, 0, 0, 255], "inside geometry");
    assert_eq!(px(&pm, 40, 30), WHITE, "inside the node box but outside the geometry");
}

#[test]
fn boolean_op_uses_precomputed_geometry() {
    let mut t = T::new();
    let b = t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::BooleanOp(BooleanOp::Subtract));
        n.transform = Affine::translate(10.0, 10.0);
        n.size = Vec2::new(40.0, 40.0);
        n.fills = vec![Paint::solid(RED)];
        n
    });
    // children whose boolean result would be empty-ish: same ellipse subtracted from itself
    t.add(Some(b), |id| Node::ellipse(id, Rect::new(0.0, 0.0, 40.0, 40.0)));
    t.add(Some(b), |id| Node::ellipse(id, Rect::new(0.0, 0.0, 40.0, 40.0)));
    let pm = t.render(60, 60);
    assert_eq!(px(&pm, 30, 30), WHITE, "derived subtract of identical shapes is empty");
    // Figma already computed the result: a plain square
    t.node(b).fill_geometry = Some(vec![rect_path(5.0, 5.0, 30.0, 30.0)]);
    let pm = t.render(60, 60);
    assert_eq!(px(&pm, 30, 30), [255, 0, 0, 255]);
    assert_eq!(px(&pm, 12, 12), WHITE);
}

#[test]
fn boolean_operands_use_resolved_variable_values() {
    let mut t = T::new();
    let frame = t.add(None, |id| Node::frame(id, Rect::new(0.0, 0.0, 100.0, 40.0)));
    let boolean = t.add(Some(frame), |id| {
        let mut n = Node::new(id, NodeKind::BooleanOp(BooleanOp::Union));
        n.size = Vec2::new(100.0, 40.0);
        n.fills = vec![Paint::solid(RED)];
        n
    });
    let operand = t.add(Some(boolean), |id| {
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 20.0, 40.0));
        n.fills = vec![Paint::solid(RED)];
        n.variables.bindings.insert("width".into(), "operand-width".into());
        n
    });
    t.node(boolean).children = vec![operand];
    t.doc.variable_collections.insert(
        "theme".into(),
        VariableCollection {
            id: "theme".into(),
            name: "Theme".into(),
            modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
            default_mode: "light".into(),
            variables: vec!["operand-width".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    t.doc.variables.insert(
        "operand-width".into(),
        Variable {
            id: "operand-width".into(),
            name: "operand-width".into(),
            collection: "theme".into(),
            kind: VariableKind::Number,
            values: [("light".into(), VariableValue::Number(20.0)), ("dark".into(), VariableValue::Number(60.0))].into_iter().collect(),
            description: String::new(),
            scopes: vec![],
            hidden_from_publishing: false,
            key: None,
            extensions: Default::default(),
            code_syntax: Default::default(),
            mode_extensions: Default::default(),
        },
    );
    t.node(frame).variables.modes.insert("theme".into(), "dark".into());

    let rendered = t.render(100, 40);
    let resolved = designos_doc::apply_bound_variables(&t.doc);
    let mut expected = designos_render::tiny_skia::Pixmap::new(100, 40).unwrap();
    designos_render::render_page(&resolved, resolved.pages[0], &designos_render::Viewport::default(), &mut expected);
    assert_eq!(px(&rendered, 50, 20), px(&expected, 50, 20), "boolean operands must use resolved width");
    assert_eq!(px(&expected, 50, 20), [255, 0, 0, 255]);
}

fn nested_boolean_operand(key: &str, kind: VariableKind, value: VariableValue) -> T {
    let mut t = T::new();
    let frame = t.add(None, |id| Node::frame(id, Rect::new(0.0, 0.0, 60.0, 40.0)));
    let boolean = t.add(Some(frame), |id| {
        let mut n = Node::new(id, NodeKind::BooleanOp(BooleanOp::Union));
        n.size = Vec2::new(60.0, 40.0);
        n.fills = vec![Paint::solid(RED)];
        n
    });
    let group = t.add(Some(boolean), |id| {
        let mut n = Node::new(id, NodeKind::Group);
        n.transform = Affine::translate(10.0, 0.0);
        n
    });
    t.add(Some(group), |id| {
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 40.0));
        n.visible = key != "visible";
        n.variables.bindings.insert(key.into(), "operand-value".into());
        n
    });
    t.doc.variable_collections.insert(
        "theme".into(),
        VariableCollection {
            id: "theme".into(),
            name: "Theme".into(),
            modes: vec![VariableMode { id: "dark".into(), name: "Dark".into() }],
            default_mode: "dark".into(),
            variables: vec!["operand-value".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    t.doc.variables.insert(
        "operand-value".into(),
        Variable {
            id: "operand-value".into(),
            name: "operand-value".into(),
            collection: "theme".into(),
            kind,
            values: [("dark".into(), value)].into_iter().collect(),
            description: String::new(),
            scopes: vec![],
            hidden_from_publishing: false,
            key: None,
            code_syntax: Default::default(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        },
    );
    t
}

#[test]
fn nested_boolean_operands_resolve_visibility_without_mutating_document() {
    let mut t = nested_boolean_operand("visible", VariableKind::Boolean, VariableValue::Boolean(true));
    let before = t.doc.to_json().unwrap();
    assert_eq!(px(&t.render(60, 40), 30, 20), [255, 0, 0, 255], "the resolved visible operand contributes through its translated group");
    assert_eq!(t.doc.to_json().unwrap(), before);
    t.doc.variables.clear();
    assert_eq!(px(&t.render(60, 40), 30, 20), WHITE, "an unresolved visibility binding keeps the stored hidden state");
}

#[test]
fn nested_boolean_operands_resolve_corner_radius() {
    let mut t = nested_boolean_operand("topLeftRadius", VariableKind::Number, VariableValue::Number(20.0));
    let pm = t.render(60, 40);
    assert_eq!(px(&pm, 11, 1), WHITE, "the bound radius rounds the translated operand's top-left corner");
    assert_eq!(px(&pm, 30, 20), [255, 0, 0, 255]);
    t.doc.variables.clear();
    assert_eq!(px(&t.render(60, 40), 11, 1), [255, 0, 0, 255], "an unresolved radius binding preserves the square stored corner");
}

#[test]
fn stroke_geometry_is_filled_with_stroke_paints() {
    let mut t = T::new();
    let r = t.rect(None, Rect::new(10.0, 10.0, 40.0, 40.0), RED);
    let n = t.node(r);
    n.fills.clear();
    n.stroke.paints = vec![Paint::solid(BLUE)];
    n.stroke.weight = 4.0;
    // a thick bar through the middle, nowhere near the node outline
    n.stroke_geometry = Some(vec![rect_path(0.0, 18.0, 40.0, 6.0)]);
    n.stamp_stroke_geometry();
    let pm = t.render(60, 60);
    assert_eq!(px(&pm, 30, 30), [0, 0, 255, 255], "bar filled with stroke paint");
    assert_eq!(px(&pm, 30, 11), WHITE, "outline not stroked");
    assert_eq!(px(&pm, 30, 48), WHITE, "outline not stroked");
}

#[test]
fn stroke_geometry_honours_even_odd_fill_rule() {
    let mut t = T::new();
    let r = t.rect(None, Rect::new(0.0, 0.0, 40.0, 40.0), RED);
    let n = t.node(r);
    n.fills.clear();
    n.stroke.paints = vec![Paint::solid(BLUE)];
    // ring: outer minus inner via even-odd
    let mut ring = rect_path(0.0, 0.0, 40.0, 40.0);
    ring.cmds.extend(rect_path(10.0, 10.0, 20.0, 20.0).cmds);
    ring.fill_rule = FillRule::EvenOdd;
    n.stroke_geometry = Some(vec![ring]);
    n.stamp_stroke_geometry();
    let pm = t.render(40, 40);
    assert_eq!(px(&pm, 5, 20), [0, 0, 255, 255]);
    assert_eq!(px(&pm, 20, 20), WHITE, "hole");
}

#[test]
fn outside_stroke_with_imported_outer_disc_keeps_fill_in_centre() {
    let mut t = T::new();
    // 8 px ellipse at (6, 6)..(14, 14), centre at (10, 10)
    let e = t.add(None, |id| Node::ellipse(id, Rect::new(6.0, 6.0, 8.0, 8.0)));
    let n = t.node(e);
    n.fills = vec![Paint::solid(RED)];
    n.stroke.paints = vec![Paint::solid(Color::WHITE)];
    n.stroke.weight = 1.5;
    n.stroke.align = StrokeAlign::Outside;
    // Imported stroke_geometry covering the full outer disc (r = 5.5, local -1.5..9.5)
    n.stroke_geometry = Some(vec![rect_path(-1.5, -1.5, 11.0, 11.0)]);
    n.stamp_stroke_geometry();
    let pm = t.render(20, 20);
    // Centre at (10, 10) must keep its red fill
    assert_eq!(px(&pm, 10, 10), [255, 0, 0, 255], "interior fill must not be covered by outside stroke_geometry");
    // Outside ring at (5, 10) (1 px left of the ellipse's x=6 edge) must be painted with the white stroke
    assert_eq!(px(&pm, 5, 10), [255, 255, 255, 255], "outside ring must be stroked");
}

#[test]
fn stale_stroke_outline_draws_live_stroke_for_weight_size_dashes_and_cap() {
    let make_rect_doc = |mutate: fn(&mut Node), keep_outline: bool| {
        let mut t = T::new();
        let r = t.rect(None, Rect::new(10.0, 10.0, 40.0, 40.0), RED);
        let n = t.node(r);
        n.fills.clear();
        n.stroke.paints = vec![Paint::solid(BLUE)];
        n.stroke.weight = 2.0;
        n.stroke.align = StrokeAlign::Center;
        if keep_outline {
            // Stored outline is a horizontal bar across the centre, distinct from any perimeter stroke.
            n.stroke_geometry = Some(vec![rect_path(0.0, 17.0, 40.0, 6.0)]);
            n.stamp_stroke_geometry();
        }
        mutate(n);
        t.render(70, 70).take()
    };

    // 1. Direct weight edit
    let with_stale_weight = make_rect_doc(|n| n.stroke.weight = 6.0, true);
    let live_weight = make_rect_doc(|n| n.stroke.weight = 6.0, false);
    assert_eq!(with_stale_weight, live_weight, "direct stroke.weight edit must draw live stroke instead of stale outline");

    // 2. Direct size edit
    let with_stale_size = make_rect_doc(|n| n.size = Vec2::new(50.0, 40.0), true);
    let live_size = make_rect_doc(|n| n.size = Vec2::new(50.0, 40.0), false);
    assert_eq!(with_stale_size, live_size, "direct size edit must draw live stroke instead of stale outline");

    // 3. Direct dashes edit
    let with_stale_dashes = make_rect_doc(|n| n.stroke.dashes = vec![4.0, 4.0], true);
    let live_dashes = make_rect_doc(|n| n.stroke.dashes = vec![4.0, 4.0], false);
    assert_eq!(with_stale_dashes, live_dashes, "direct stroke.dashes edit must draw live stroke instead of stale outline");

    // 4. Direct cap edit on an open vector path
    let make_open_doc = |keep_outline: bool| {
        let mut t = T::new();
        let v = t.add(None, |id| {
            let mut n = Node::new(
                id,
                NodeKind::Vector {
                    paths: vec![Path {
                        cmds: vec![PathCmd::MoveTo(Vec2::new(0.0, 20.0)), PathCmd::LineTo(Vec2::new(40.0, 20.0))],
                        fill_rule: FillRule::NonZero,
                    }],
                    network: None,
                },
            );
            n.transform = Affine::translate(10.0, 10.0);
            n.size = Vec2::new(40.0, 40.0);
            n.stroke.paints = vec![Paint::solid(BLUE)];
            n.stroke.weight = 6.0;
            n.stroke.cap = StrokeCap::None;
            if keep_outline {
                n.stroke_geometry = Some(vec![rect_path(15.0, 0.0, 10.0, 40.0)]);
                n.stamp_stroke_geometry();
            }
            n.stroke.cap = StrokeCap::Square;
            n
        });
        let _ = v;
        t.render(70, 70).take()
    };
    assert_eq!(make_open_doc(true), make_open_doc(false), "direct stroke.cap edit must draw live stroke instead of stale outline");
}

#[test]
fn unchanged_layer_with_stored_stroke_outline_and_loaded_unstamped_doc_render_stored_outline() {
    let mut t = T::new();
    let r = t.rect(None, Rect::new(10.0, 10.0, 40.0, 40.0), RED);
    let n = t.node(r);
    n.fills.clear();
    n.stroke.paints = vec![Paint::solid(BLUE)];
    n.stroke.weight = 4.0;
    n.stroke_geometry = Some(vec![rect_path(0.0, 18.0, 40.0, 6.0)]);
    n.stamp_stroke_geometry();
    let expected = t.render(60, 60);
    assert_eq!(px(&expected, 30, 30), [0, 0, 255, 255], "unchanged stamped outline must draw the stored outline");
    assert_eq!(px(&expected, 30, 11), WHITE, "unchanged stamped outline must not draw live perimeter stroke");

    // Clear the stamp and round-trip through JSON to simulate a document saved without stamps.
    t.node(r).stroke_geometry_for = None;
    let json = t.doc.to_json().unwrap();
    t.doc = Document::from_json(&json).unwrap();
    let from_unstamped = t.render(60, 60);
    assert_eq!(from_unstamped.data(), expected.data(), "document loaded without stamps must stamp and draw the stored outline identically");
}

#[test]
fn parked_size_outline_loaded_without_stamp_restores_and_renders_child_outline() {
    fn strip_stamp_keys(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::Object(map) => {
                map.remove("stroke_geometry_for");
                for child in map.values_mut() {
                    strip_stamp_keys(child);
                }
            }
            serde_json::Value::Array(arr) => {
                for item in arr {
                    strip_stamp_keys(item);
                }
            }
            _ => {}
        }
    }

    let apply_cmd = |doc: &mut Document, cmd: Command| {
        cmd.apply(doc).unwrap();
    };
    let mut t = T::new();

    // Variant State=A (with Frame child "Badge" whose stroke weight is 4, while instance default stroke weight is 1).
    let a = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(0.0, 0.0, 40.0, 40.0));
        n.name = "State=A".into();
        n.fills.clear();
        n
    });
    let _badge_main = t.add(Some(a), |id| {
        let mut n = Node::frame(id, Rect::new(0.0, 0.0, 20.0, 20.0));
        n.name = "Badge".into();
        n.fills.clear();
        n.stroke.paints = vec![Paint::solid(BLUE)];
        n.stroke.weight = 4.0;
        n
    });
    let cmd = create_component(&mut t.doc, a);
    apply_cmd(&mut t.doc, cmd);

    // Variant State=B (without "Badge" child).
    let b = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(100.0, 0.0, 40.0, 40.0));
        n.name = "State=B".into();
        n.fills.clear();
        n
    });
    let cmd = create_component(&mut t.doc, b);
    apply_cmd(&mut t.doc, cmd);

    let cmd = combine_as_variants(&mut t.doc, &[a, b]);
    apply_cmd(&mut t.doc, cmd);

    // Create instance at (0, 0); hide the component set so only the instance renders in (0..40, 0..40).
    let set_id = t.doc.get(a).unwrap().parent.unwrap();
    t.node(set_id).visible = false;
    let cmd = create_instance(&mut t.doc, a, t.page, Vec2::new(0.0, 0.0));
    apply_cmd(&mut t.doc, cmd);
    let inst = *t.doc.get(t.page).unwrap().children.last().unwrap();
    assert_eq!(t.doc.get(inst).unwrap().stroke.weight, 1.0);

    let badge_inst = t.doc.get(inst).unwrap().children[0];
    {
        let n = t.node(badge_inst);
        assert_eq!(n.stroke.weight, 4.0);
        n.size = Vec2::new(40.0, 40.0);
        n.stroke.weight = 8.0;
        n.stroke_geometry = Some(vec![rect_path(0.0, 17.0, 40.0, 6.0)]);
        n.stamp_stroke_geometry();
        n.component.overrides.insert(OverrideField::Size);
        n.component.overrides.insert(OverrideField::Stroke);
    }
    let expected = t.render(40, 40);
    assert_eq!(px(&expected, 20, 20), [0, 0, 255, 255], "child outline bar must be drawn before variant switch");
    assert_eq!(px(&expected, 20, 2), WHITE, "live perimeter stroke must not be drawn when child outline is valid");

    // Switch to State=B so the child's Size + Stroke overrides are parked on the instance.
    let cmd = set_instance_prop(&mut t.doc, inst, "State", Some(PropValue::Variant("B".into())));
    apply_cmd(&mut t.doc, cmd);
    assert_eq!(t.doc.get(inst).unwrap().component.parked_overrides.len(), 2);

    // Strip all stamps in JSON to simulate a document saved without stamps.
    let mut val: serde_json::Value = serde_json::from_str(&t.doc.to_json().unwrap()).unwrap();
    strip_stamp_keys(&mut val);
    let json = serde_json::to_string(&val).unwrap();
    assert!(!json.contains("\"stroke_geometry_for\""));
    t.doc = Document::from_json(&json).unwrap();

    // Restore the parked overrides by switching back to State=A.
    let cmd = set_instance_prop(&mut t.doc, inst, "State", Some(PropValue::Variant("A".into())));
    apply_cmd(&mut t.doc, cmd);

    let after_restore = t.render(40, 40);
    assert_eq!(after_restore.data(), expected.data(), "restored parked size outline + stroke override must render pixel-equal to origin/main");

    // Save and reload: the restored stamp must match weight 8 so the stored band is still drawn.
    let saved_json = t.doc.to_json().unwrap();
    t.doc = Document::from_json(&saved_json).unwrap();
    let after_reload = t.render(40, 40);
    assert_eq!(after_reload.data(), expected.data(), "saved and reloaded document after restore must still draw the stored outline");

    // A parked size outline that already carries a `Some` stamp (parked after this change) keeps it as is.
    let badge_restored = t.doc.get(inst).unwrap().children[0];
    let stamp_for_8 = t.doc.get(badge_restored).unwrap().stroke_geometry_for.clone();
    assert!(stamp_for_8.is_some());
    t.node(badge_restored).stroke.weight = 12.0;
    let cmd = set_instance_prop(&mut t.doc, inst, "State", Some(PropValue::Variant("B".into())));
    apply_cmd(&mut t.doc, cmd);
    let cmd = set_instance_prop(&mut t.doc, inst, "State", Some(PropValue::Variant("A".into())));
    apply_cmd(&mut t.doc, cmd);
    let badge_after_second_restore = t.doc.get(inst).unwrap().children[0];
    assert_eq!(
        t.doc.get(badge_after_second_restore).unwrap().stroke_geometry_for,
        stamp_for_8,
        "a parked size outline that already carries a Some stamp must keep it as is across park and restore"
    );
    assert!(
        t.doc.get(badge_after_second_restore).unwrap().live_stroke_outline().is_none(),
        "stale outline parked with a Some stamp must remain invalid after restore"
    );
}
