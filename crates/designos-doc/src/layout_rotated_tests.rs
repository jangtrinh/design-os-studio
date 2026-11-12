//! Auto layout sizes and places a rotated or flipped child by its bounding box.

use crate::lint::rules::aabb_of_transformed_rect;
use crate::{
    bind_node_variable, Affine, AutoLayout, BindableProperty, Document, LayoutChild, LayoutDirection, Node, NodeId, NodeKind, Rect, Sizing, TextStyle,
    Variable, VariableCollection, VariableKind, VariableMode, VariableValue,
};

/// A quarter turn clockwise `turns` times, with exact trigonometric values.
fn quarter(turns: i32) -> Affine {
    let (s, c) = match turns.rem_euclid(4) {
        0 => (0.0, 1.0),
        1 => (1.0, 0.0),
        2 => (0.0, -1.0),
        _ => (-1.0, 0.0),
    };
    Affine { a: c, b: s, c: -s, d: c, tx: 0.0, ty: 0.0 }
}

fn flow_frame(doc: &mut Document, page: NodeId, direction: LayoutDirection, sizing: Sizing, padding: f32) -> NodeId {
    let id = doc.alloc_id();
    let mut f = Node::frame(id, Rect::new(0.0, 0.0, 300.0, 300.0));
    if let NodeKind::Frame(fp) = &mut f.kind {
        fp.auto_layout = Some(AutoLayout { direction, primary_sizing: sizing, counter_sizing: sizing, padding: [padding; 4], ..AutoLayout::default() });
    }
    doc.insert(page, f).unwrap();
    id
}

fn add_rect(doc: &mut Document, parent: NodeId, w: f32, h: f32, tf: Affine) -> NodeId {
    let id = doc.alloc_id();
    let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, w, h));
    n.transform = tf;
    doc.insert(parent, n).unwrap();
    id
}

fn bbox(doc: &Document, id: NodeId) -> Rect {
    let n = doc.get(id).unwrap();
    aabb_of_transformed_rect(n.transform, n.size)
}

fn near(got: f32, want: f32, label: &str) {
    assert!((got - want).abs() < 0.01, "{label}: got {got} want {want}");
}

#[test]
fn rotated_text_in_a_horizontal_hug_frame_takes_the_width_of_its_bounding_box() {
    let mut doc = Document::new("rot");
    let page = doc.add_page("Page 1");
    let frame = flow_frame(&mut doc, page, LayoutDirection::Horizontal, Sizing::Hug, 4.0);
    let id = doc.alloc_id();
    let mut label = Node::text(id, Rect::new(0.0, 0.0, 72.0, 18.0), "Y axis", TextStyle::default());
    label.transform = quarter(3);
    doc.insert(frame, label).unwrap();

    doc.relayout_subtree(frame);

    let f = doc.get(frame).unwrap();
    near(f.size.x, 18.0 + 8.0, "frame width");
    near(f.size.y, 72.0 + 8.0, "frame height");
    let b = bbox(&doc, id);
    near(b.x, 4.0, "bbox x");
    near(b.y, 4.0, "bbox y");
    near(b.w, 18.0, "bbox w");
    near(b.h, 72.0, "bbox h");
    near(doc.get(id).unwrap().size.x, 72.0, "local width is untouched");
}

#[test]
fn every_quarter_turn_and_flip_lands_its_box_in_the_solved_slot() {
    for turns in 0..4 {
        for flip in [false, true] {
            let mut doc = Document::new("rot");
            let page = doc.add_page("Page 1");
            let frame = flow_frame(&mut doc, page, LayoutDirection::Horizontal, Sizing::Hug, 0.0);
            let mut tf = quarter(turns);
            if flip {
                tf.a = -tf.a;
                tf.b = -tf.b;
            }
            let first = add_rect(&mut doc, frame, 50.0, 20.0, tf);
            let second = add_rect(&mut doc, frame, 30.0, 30.0, Affine::IDENTITY);
            doc.relayout_subtree(frame);

            let ext_w = if turns % 2 == 1 { 20.0 } else { 50.0 };
            let ext_h = if turns % 2 == 1 { 50.0 } else { 20.0 };
            let label = format!("turns {turns} flip {flip}");
            let b = bbox(&doc, first);
            near(b.x, 0.0, &format!("{label} bbox x"));
            near(b.y, 0.0, &format!("{label} bbox y"));
            near(b.w, ext_w, &format!("{label} bbox w"));
            near(b.h, ext_h, &format!("{label} bbox h"));
            near(bbox(&doc, second).x, ext_w, &format!("{label} next sibling x"));
            near(doc.get(frame).unwrap().size.x, ext_w + 30.0, &format!("{label} frame width"));
        }
    }
}

#[test]
fn arbitrary_angle_child_is_laid_out_by_its_bounding_box() {
    let mut doc = Document::new("rot");
    let page = doc.add_page("Page 1");
    let frame = flow_frame(&mut doc, page, LayoutDirection::Vertical, Sizing::Hug, 10.0);
    let tilted = add_rect(&mut doc, frame, 40.0, 10.0, Affine::rotate(30f32.to_radians()));
    let below = add_rect(&mut doc, frame, 20.0, 20.0, Affine::IDENTITY);
    doc.relayout_subtree(frame);

    let (sin, cos) = 30f32.to_radians().sin_cos();
    let (ew, eh) = (40.0 * cos + 10.0 * sin, 40.0 * sin + 10.0 * cos);
    let b = bbox(&doc, tilted);
    near(b.x, 10.0, "bbox x");
    near(b.y, 10.0, "bbox y");
    near(b.w, ew, "bbox w");
    near(b.h, eh, "bbox h");
    near(bbox(&doc, below).y, 10.0 + eh, "sibling below the box");
    near(doc.get(frame).unwrap().size.x, ew + 20.0, "frame width");
}

#[test]
fn fill_on_a_quarter_turned_child_stretches_its_bounding_box_and_resizes_the_local_box() {
    let mut doc = Document::new("rot");
    let page = doc.add_page("Page 1");
    let frame = flow_frame(&mut doc, page, LayoutDirection::Vertical, Sizing::Fixed, 0.0);
    let id = add_rect(&mut doc, frame, 72.0, 18.0, quarter(3));
    doc.get_mut(id).unwrap().layout_child = LayoutChild { horizontal: Sizing::Fill, ..LayoutChild::default() };
    doc.relayout_subtree(frame);

    let b = bbox(&doc, id);
    near(b.x, 0.0, "bbox x");
    near(b.w, 300.0, "box fills the frame width");
    near(b.h, 72.0, "bbox h");
    let n = doc.get(id).unwrap();
    near(n.size.y, 300.0, "local height follows the filled axis");
    near(n.size.x, 72.0, "local width stays");
}

#[test]
fn absolute_positioned_rotated_child_is_not_touched() {
    let mut doc = Document::new("rot");
    let page = doc.add_page("Page 1");
    let frame = flow_frame(&mut doc, page, LayoutDirection::Horizontal, Sizing::Hug, 0.0);
    let flow = add_rect(&mut doc, frame, 40.0, 40.0, Affine::IDENTITY);
    let abs = add_rect(&mut doc, frame, 72.0, 18.0, Affine { tx: 55.0, ty: 66.0, ..quarter(3) });
    doc.get_mut(abs).unwrap().layout_child = LayoutChild { absolute: true, ..LayoutChild::default() };
    doc.relayout_subtree(frame);

    near(doc.get(frame).unwrap().size.x, 40.0, "frame ignores the absolute child");
    near(bbox(&doc, flow).x, 0.0, "flow child");
    let t = doc.get(abs).unwrap().transform;
    near(t.tx, 55.0, "absolute tx");
    near(t.ty, 66.0, "absolute ty");
}

#[test]
fn relayout_is_idempotent_for_rotated_children() {
    let mut doc = Document::new("rot");
    let page = doc.add_page("Page 1");
    let frame = flow_frame(&mut doc, page, LayoutDirection::Horizontal, Sizing::Hug, 6.0);
    let id = add_rect(&mut doc, frame, 72.0, 18.0, quarter(1));
    doc.relayout_subtree(frame);
    let once = (doc.get(id).unwrap().transform, doc.get(id).unwrap().size, doc.get(frame).unwrap().size);
    doc.relayout_subtree(frame);
    let twice = (doc.get(id).unwrap().transform, doc.get(id).unwrap().size, doc.get(frame).unwrap().size);
    assert_eq!(once, twice);
}

fn turn(deg: f32) -> Affine {
    Affine::rotate(deg.to_radians())
}

fn state(doc: &Document, frame: NodeId, id: NodeId) -> (Affine, crate::Vec2, crate::Vec2) {
    (doc.get(id).unwrap().transform, doc.get(id).unwrap().size, doc.get(frame).unwrap().size)
}

fn finite_and_sane(doc: &Document, id: NodeId, label: &str) {
    let n = doc.get(id).unwrap();
    assert!(
        n.size.x.is_finite() && n.size.y.is_finite() && n.size.x > 0.0 && n.size.y > 0.0 && n.size.x < 1000.0 && n.size.y < 1000.0,
        "{label}: {:?}",
        n.size
    );
}

/// Hundreds of relayouts leave the child, and the frame, exactly where the first one put them.
fn assert_stable(doc: &mut Document, frame: NodeId, id: NodeId, label: &str) {
    doc.relayout_subtree(frame);
    let first = state(doc, frame, id);
    for _ in 0..200 {
        doc.relayout_subtree(frame);
    }
    assert_eq!(first, state(doc, frame, id), "{label}");
    finite_and_sane(doc, id, label);
}

#[test]
fn float_trig_quarter_turns_are_stable_over_200_relayouts() {
    for deg in [90.0, 180.0, 270.0, -90.0] {
        let mut doc = Document::new("rot");
        let page = doc.add_page("Page 1");
        let hug = flow_frame(&mut doc, page, LayoutDirection::Horizontal, Sizing::Hug, 3.0);
        let a = add_rect(&mut doc, hug, 72.0, 18.0, turn(deg));
        assert_stable(&mut doc, hug, a, &format!("hug {deg}"));
        let b = bbox(&doc, a);
        near(b.x, 3.0, "hug bbox x");
        near(b.y, 3.0, "hug bbox y");

        let fixed = flow_frame(&mut doc, page, LayoutDirection::Vertical, Sizing::Fixed, 0.0);
        let f = add_rect(&mut doc, fixed, 72.0, 18.0, turn(deg));
        doc.get_mut(f).unwrap().layout_child = LayoutChild { horizontal: Sizing::Fill, ..LayoutChild::default() };
        doc.get_mut(f).unwrap().layout_child.vertical = Sizing::Fixed;
        assert_stable(&mut doc, fixed, f, &format!("fill {deg}"));
        let b = bbox(&doc, f);
        near(b.x, 0.0, "fill bbox x");
        let n = doc.get(f).unwrap();
        let swapped = deg != 180.0;
        near(if swapped { n.size.y } else { n.size.x }, 300.0, "the filled axis spans the frame");
    }
}

#[test]
fn flipped_child_with_fill_is_stable_and_fills() {
    for (a, d) in [(-1.0f32, 1.0f32), (1.0, -1.0), (-1.0, -1.0)] {
        let mut doc = Document::new("rot");
        let page = doc.add_page("Page 1");
        let frame = flow_frame(&mut doc, page, LayoutDirection::Vertical, Sizing::Fixed, 0.0);
        let id = add_rect(&mut doc, frame, 50.0, 20.0, Affine { a, b: 0.0, c: 0.0, d, tx: 50.0, ty: 20.0 });
        doc.get_mut(id).unwrap().layout_child = LayoutChild { horizontal: Sizing::Fill, ..LayoutChild::default() };
        assert_stable(&mut doc, frame, id, "flip fill");
        let b = bbox(&doc, id);
        near(b.x, 0.0, "bbox x");
        near(b.w, 300.0, "bbox w");
    }
}

#[test]
fn tilted_children_ignore_fill_and_limits_and_never_grow() {
    let cases: [(&str, LayoutChild); 4] = [
        ("fill", LayoutChild { horizontal: Sizing::Fill, ..LayoutChild::default() }),
        ("min_w", LayoutChild { min_w: Some(200.0), ..LayoutChild::default() }),
        ("max_w", LayoutChild { max_w: Some(20.0), ..LayoutChild::default() }),
        ("stretch", LayoutChild { align_self: crate::AlignSelf::Stretch, vertical: Sizing::Fill, ..LayoutChild::default() }),
    ];
    for deg in [10.0, 30.0, 44.0, 44.99, 45.0, 45.01, 60.0, 120.0, 134.9, 150.0] {
        for (name, lc) in cases {
            for frame_w in [300.0, 5.0] {
                let mut doc = Document::new("rot");
                let page = doc.add_page("Page 1");
                let frame = flow_frame(&mut doc, page, LayoutDirection::Vertical, Sizing::Fixed, 0.0);
                doc.get_mut(frame).unwrap().size = crate::Vec2::new(frame_w, 300.0);
                let id = add_rect(&mut doc, frame, 40.0, 10.0, turn(deg));
                doc.get_mut(id).unwrap().layout_child = lc;
                assert_stable(&mut doc, frame, id, &format!("{name} {deg} frame {frame_w}"));
                let n = doc.get(id).unwrap();
                assert_eq!(n.size, crate::Vec2::new(40.0, 10.0), "{name} {deg}: size untouched");
                let b = bbox(&doc, id);
                near(b.x, 0.0, "bbox x");
                near(b.y, 0.0, "bbox y");
            }
        }
    }
}

#[test]
fn tilted_fill_child_does_not_squeeze_its_siblings() {
    let mut doc = Document::new("rot");
    let page = doc.add_page("Page 1");
    let frame = flow_frame(&mut doc, page, LayoutDirection::Horizontal, Sizing::Fixed, 0.0);
    let tilted = add_rect(&mut doc, frame, 40.0, 10.0, turn(30.0));
    doc.get_mut(tilted).unwrap().layout_child = LayoutChild { horizontal: Sizing::Fill, ..LayoutChild::default() };
    let next = add_rect(&mut doc, frame, 20.0, 20.0, Affine::IDENTITY);
    doc.relayout_subtree(frame);
    let (sin, cos) = 30f32.to_radians().sin_cos();
    near(bbox(&doc, next).x, 40.0 * cos + 10.0 * sin, "the sibling sits right after the tilted box");
}

fn width_variable(doc: &mut Document, value: f32) {
    doc.variable_collections.insert(
        "col".into(),
        VariableCollection {
            id: "col".into(),
            name: "Sizes".into(),
            modes: vec![VariableMode { id: "m".into(), name: "M".into() }],
            default_mode: "m".into(),
            variables: vec!["w".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    doc.variables.insert(
        "w".into(),
        Variable {
            id: "w".into(),
            name: "size/w".into(),
            collection: "col".into(),
            kind: VariableKind::Number,
            values: [("m".to_string(), VariableValue::Number(value))].into_iter().collect(),
            description: String::new(),
            scopes: Vec::new(),
            hidden_from_publishing: false,
            key: None,
            code_syntax: Default::default(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        },
    );
}

#[test]
fn width_binding_on_a_quarter_turned_child_drives_the_local_width() {
    for deg in [0.0, 90.0, 270.0] {
        let mut doc = Document::new("rot");
        let page = doc.add_page("Page 1");
        width_variable(&mut doc, 100.0);
        let frame = flow_frame(&mut doc, page, LayoutDirection::Horizontal, Sizing::Hug, 0.0);
        let id = add_rect(&mut doc, frame, 72.0, 18.0, turn(deg));
        bind_node_variable(&mut doc, id, BindableProperty::Width, "w").unwrap();
        assert_stable(&mut doc, frame, id, &format!("bound {deg}"));
        let n = doc.get(id).unwrap();
        near(n.size.x, 100.0, &format!("{deg}: local width follows the variable"));
        near(n.size.y, 18.0, &format!("{deg}: local height stays"));
        let b = bbox(&doc, id);
        let (ew, eh) = if deg == 0.0 { (100.0, 18.0) } else { (18.0, 100.0) };
        near(b.w, ew, "bbox w");
        near(b.h, eh, "bbox h");
    }
}
