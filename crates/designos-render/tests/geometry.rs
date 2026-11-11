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
    let pm = t.render(20, 20);
    // Centre at (10, 10) must keep its red fill
    assert_eq!(px(&pm, 10, 10), [255, 0, 0, 255], "interior fill must not be covered by outside stroke_geometry");
    // Outside ring at (5, 10) (1 px left of the ellipse's x=6 edge) must be painted with the white stroke
    assert_eq!(px(&pm, 5, 10), [255, 255, 255, 255], "outside ring must be stroked");
}
