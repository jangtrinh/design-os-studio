//! The record of what a user edit changed (`AgentBusyFrames::record_user_node_update`, fed by
//! `node_field_diff`) must hold exactly the (node, field) entries it always held, and must not build JSON
//! to find them: a palette operation hands it tens of thousands of nodes on the UI thread.
//!
//! `value_diff_oracle` is the diff as it was written when it turned both nodes into `serde_json::Value`s.
//! It is kept here only as the reference the current diff has to agree with.

use super::*;
use crate::app::App;
use crate::edit::Tx;
use crate::test_alloc_count::allocations_in;
use designos_doc::{
    Affine, BlendMode, Color, ComponentLink, ComponentPropDef, Constraint, Constraints, DevInfo, DevStatus, Effect, ExportPreset, FrameProps, LayoutChild,
    Node, NodeId, NodeKind, NodeVariables, OverrideField, Paint, Path, PathCmd, PropKind, PropValue, PrototypeProps, SlideProps, SlotPropConfig, Stroke,
    TextProps, Vec2,
};
use std::collections::BTreeSet;

/// The field names `record_user_node_update` records for `before` and `after`, sorted.
fn diff_node_fields(before: &Node, after: &Node) -> Vec<String> {
    let mut changed = Vec::new();
    crate::node_field_diff::for_each_changed_field(before, after, |field| changed.push(field.to_owned()));
    changed.sort();
    changed
}

fn value_diff_oracle(before: &Node, after: &Node) -> Vec<String> {
    let (Ok(serde_json::Value::Object(b_map)), Ok(serde_json::Value::Object(a_map))) = (serde_json::to_value(before), serde_json::to_value(after)) else {
        return Vec::new();
    };
    let mut changed = Vec::new();
    for (k, v_after) in &a_map {
        if matches!(k.as_str(), "id" | "parent" | "children") {
            continue;
        }
        if b_map.get(k) != Some(v_after) {
            changed.push(k.clone());
        }
    }
    for k in b_map.keys() {
        if matches!(k.as_str(), "id" | "parent" | "children") {
            continue;
        }
        if !a_map.contains_key(k) && !changed.contains(k) {
            changed.push(k.clone());
        }
    }
    changed.sort();
    changed
}

fn assert_same_as_value_diff(before: &Node, after: &Node, what: &str) {
    let expected = value_diff_oracle(before, after);
    assert_eq!(diff_node_fields(before, after), expected, "diff_node_fields: {what}");
    let mut frames = AgentBusyFrames::default();
    frames.record_user_node_update(before, after);
    let recorded: Vec<(NodeId, String)> = frames.user_edited_fields.keys().cloned().collect();
    let wanted: Vec<(NodeId, String)> = expected.iter().map(|f| (after.id, f.clone())).collect();
    assert_eq!(recorded, wanted, "record_user_node_update: {what}");
}

fn path_with(points: usize) -> Path {
    let mut cmds = vec![PathCmd::MoveTo(Vec2::new(0.0, 0.0))];
    for i in 0..points {
        let t = i as f32;
        cmds.push(PathCmd::CubicTo(Vec2::new(t, t + 1.0), Vec2::new(t + 2.0, t + 3.0), Vec2::new(t + 4.0, t + 5.0)));
    }
    cmds.push(PathCmd::Close);
    Path { cmds, ..Default::default() }
}

fn drop_shadow() -> Effect {
    Effect::DropShadow {
        color: Color::WHITE,
        offset: Vec2::new(1.0, 2.0),
        radius: 4.0,
        spread: 0.0,
        visible: true,
        show_behind: false,
        blend: BlendMode::Normal,
    }
}

/// A node whose every serialized field holds a non-default value, so each field is present until an edit resets it.
fn populated() -> Node {
    let mut n = Node::new(NodeId(7), NodeKind::Frame(FrameProps { clips_content: true, ..Default::default() }));
    n.source_id = Some("12:34".into());
    n.name = "Populated".into();
    n.parent = Some(NodeId(1));
    n.children = vec![NodeId(8), NodeId(9)];
    n.visible = false;
    n.locked = true;
    n.opacity = 0.5;
    n.blend = BlendMode::Multiply;
    n.transform = Affine::translate(3.0, 4.0);
    n.size = Vec2::new(10.0, 20.0);
    n.fills = vec![Paint::solid(Color::rgb(1, 2, 3))];
    n.stroke = Stroke { paints: vec![Paint::solid(Color::WHITE)], weight: 2.0, miter_limit: 4.0, ..Default::default() };
    n.effects = vec![drop_shadow()];
    n.corner_radii = [1.0, 2.0, 3.0, 4.0];
    n.corner_smoothing = 0.6;
    n.constraints = Constraints { horizontal: Constraint::Max, vertical: Constraint::Stretch };
    n.layout_child = LayoutChild { absolute: true, ..Default::default() };
    n.is_mask = true;
    n.style_refs.insert("fill".into(), "S:1".into());
    n.fill_geometry = Some(vec![path_with(2)]);
    n.stroke_geometry = Some(vec![path_with(1)]);
    n.component = ComponentLink { description: "Main".into(), ..Default::default() };
    n.prototype = PrototypeProps { fixed_children: vec![NodeId(8)], ..Default::default() };
    n.variables.bindings.insert("fills".into(), "V:1".into());
    n.export_settings = vec![ExportPreset { suffix: "@2x".into(), ..Default::default() }];
    n.dev = DevInfo { status: Some(DevStatus::ReadyForDev), ..Default::default() };
    n.slide = Some(SlideProps { notes: "notes".into() });
    n.lock_aspect = true;
    n.hyperlink = Some("https://example.com".into());
    n.plugin_data.insert("plugin:key".into(), "value".into());
    n.detached_symbol_id = Some("1:1".into());
    n.fig_metadata.insert("version".into(), "1".into());
    n
}

type Edit = fn(&mut Node);

/// For each serialized field of `Node` (all but `id`, `parent`, `children`): edits that reset it to its
/// skipped default, move it to other values, and push it to values whose JSON form is `null`.
fn field_edits() -> Vec<(&'static str, Vec<Edit>)> {
    vec![
        ("source_id", vec![|n| n.source_id = None, |n| n.source_id = Some("99:1".into())]),
        ("name", vec![|n| n.name.clear(), |n| n.name = "Other".into()]),
        (
            "kind",
            vec![|n| n.kind = NodeKind::Group, |n| n.kind = NodeKind::Rectangle, |n| {
                if let NodeKind::Frame(f) = &mut n.kind {
                    f.clips_content = false;
                }
            }],
        ),
        ("visible", vec![|n| n.visible = true]),
        ("locked", vec![|n| n.locked = false]),
        ("opacity", vec![|n| n.opacity = 1.0, |n| n.opacity = 0.25, |n| n.opacity = f32::NAN, |n| n.opacity = f32::INFINITY]),
        ("blend", vec![|n| n.blend = BlendMode::PassThrough, |n| n.blend = BlendMode::Screen]),
        ("transform", vec![|n| n.transform = Affine::IDENTITY, |n| n.transform.tx = -0.0, |n| n.transform.ty = f32::NAN]),
        ("size", vec![|n| n.size = Vec2::ZERO, |n| n.size = Vec2::new(1.0, 2.0), |n| n.size.x = f32::NEG_INFINITY]),
        (
            "fills",
            vec![|n| n.fills.clear(), |n| n.fills = vec![Paint::solid(Color::WHITE), Paint::solid(Color::rgb(9, 9, 9))], |n| {
                if let Some(p) = n.fills.first_mut() {
                    p.opacity = f32::NAN;
                }
            }],
        ),
        (
            "stroke",
            vec![
                |n| n.stroke = Stroke { weight: 1.0, miter_limit: 4.0, ..Default::default() },
                |n| n.stroke.weight = 3.0,
                |n| n.stroke.weight = f32::NAN,
                |n| n.stroke.dashes = vec![1.0, 2.0],
            ],
        ),
        ("effects", vec![|n| n.effects.clear(), |n| n.effects = vec![drop_shadow(), drop_shadow()]]),
        ("corner_radii", vec![|n| n.corner_radii = [0.0; 4], |n| n.corner_radii = [5.0; 4], |n| n.corner_radii[2] = f32::NAN]),
        ("corner_smoothing", vec![|n| n.corner_smoothing = 0.0, |n| n.corner_smoothing = -0.0, |n| n.corner_smoothing = 0.2]),
        (
            "constraints",
            vec![|n| n.constraints = Constraints::default(), |n| n.constraints = Constraints { horizontal: Constraint::Center, vertical: Constraint::Min }],
        ),
        ("layout_child", vec![|n| n.layout_child = LayoutChild::default(), |n| n.layout_child = LayoutChild { grow: 1.0, ..Default::default() }]),
        ("is_mask", vec![|n| n.is_mask = false]),
        ("style_refs", vec![|n| n.style_refs.clear(), |n| n.style_refs.insert("stroke".into(), "S:2".into()).map_or((), |_| ())]),
        ("fill_geometry", vec![|n| n.fill_geometry = None, |n| n.fill_geometry = Some(Vec::new()), |n| n.fill_geometry = Some(vec![path_with(3)])]),
        ("stroke_geometry", vec![|n| n.stroke_geometry = None, |n| n.stroke_geometry = Some(Vec::new()), |n| n.stroke_geometry = Some(vec![path_with(3)])]),
        (
            "component",
            vec![
                |n| n.component = ComponentLink::default(),
                |n| n.component.description = "Other".into(),
                |n| n.component.is_slot = true,
                // a flattened map of unmapped keys inside a map value, and a number JSON cannot hold inside it
                |n| {
                    let extra = [("unmapped".to_string(), serde_json::json!({"a": [1, 2.5, null], "b": "text"}))].into_iter().collect();
                    n.component.slot_configs.insert("Body".into(), SlotPropConfig { min_children: Some(1), extra, ..Default::default() });
                },
                // newtype variants of an enum, as map values
                |n| {
                    n.component.values.insert("Items".into(), PropValue::Slot(vec![NodeId(3)]));
                    n.component.values.insert("Label".into(), PropValue::Text("text".into()));
                },
                // struct variants and an enum with data, in a list
                |n| {
                    n.component.props = vec![
                        ComponentPropDef {
                            name: "Size".into(),
                            kind: PropKind::Variant { options: vec!["sm".into(), "lg".into()] },
                            default: PropValue::Variant("sm".into()),
                        },
                        ComponentPropDef {
                            name: "Icon".into(),
                            kind: PropKind::InstanceSwap { preferred: vec![NodeId(2)] },
                            default: PropValue::Instance(NodeId(2)),
                        },
                    ];
                },
                // a map keyed by a unit-variant enum and a set of them
                |n| {
                    n.component.bindings.insert(OverrideField::Visible, "Show".into());
                    n.component.overrides.insert(OverrideField::Fills);
                },
                |n| n.component.grid_align_overrides = [true, false],
            ],
        ),
        ("prototype", vec![|n| n.prototype = PrototypeProps::default(), |n| n.prototype.fixed_children = vec![NodeId(8), NodeId(9)]]),
        ("variables", vec![|n| n.variables = NodeVariables::default(), |n| n.variables.modes.insert("C:1".into(), "M:2".into()).map_or((), |_| ())]),
        (
            "export_settings",
            vec![|n| n.export_settings.clear(), |n| {
                n.export_settings = vec![ExportPreset::default(), ExportPreset { suffix: "@3x".into(), ..Default::default() }]
            }],
        ),
        ("dev", vec![|n| n.dev = DevInfo::default(), |n| n.dev.status = Some(DevStatus::Completed)]),
        ("slide", vec![|n| n.slide = None, |n| n.slide = Some(SlideProps { notes: "other".into() })]),
        ("lock_aspect", vec![|n| n.lock_aspect = false]),
        ("hyperlink", vec![|n| n.hyperlink = None, |n| n.hyperlink = Some("https://example.org".into())]),
        ("plugin_data", vec![|n| n.plugin_data.clear(), |n| n.plugin_data.insert("a:b".into(), "c".into()).map_or((), |_| ())]),
        ("detached_symbol_id", vec![|n| n.detached_symbol_id = None, |n| n.detached_symbol_id = Some("2:2".into())]),
        ("fig_metadata", vec![|n| n.fig_metadata.clear(), |n| n.fig_metadata.insert("publishID".into(), "p".into()).map_or((), |_| ())]),
    ]
}

/// Edits of the fields a user edit never reports: identity and tree position.
fn ignored_edits() -> Vec<(&'static str, Edit)> {
    vec![
        ("id", |n| n.id = NodeId(99)),
        ("parent", |n| n.parent = None),
        ("parent moved", |n| n.parent = Some(NodeId(5))),
        ("children cleared", |n| n.children.clear()),
        ("children appended", |n| n.children.push(NodeId(100))),
    ]
}

/// One of every `NodeKind`. The `match` has no wildcard arm: a new variant does not compile until it is listed here.
fn every_kind() -> Vec<NodeKind> {
    use designos_doc::{ArcData, BooleanOp, ConnectorProps, ShapeWithTextProps, StickyProps, VectorNetwork};
    let kinds = vec![
        NodeKind::Page { background: Color::WHITE },
        NodeKind::Frame(FrameProps::default()),
        NodeKind::Group,
        NodeKind::Section,
        NodeKind::Rectangle,
        NodeKind::Ellipse(ArcData::default()),
        NodeKind::Line,
        NodeKind::Polygon { count: 5 },
        NodeKind::Star { count: 5, inner_ratio: 0.4 },
        NodeKind::Vector { paths: vec![path_with(4)], network: None },
        NodeKind::Vector { paths: vec![path_with(2)], network: Some(VectorNetwork::default()) },
        NodeKind::Text(TextProps { content: "hello".into(), ..Default::default() }),
        NodeKind::BooleanOp(BooleanOp::Union),
        NodeKind::Component(FrameProps::default()),
        NodeKind::ComponentSet(FrameProps::default()),
        NodeKind::Instance { frame: FrameProps::default(), component: Some(NodeId(3)), component_key: Some("key".into()) },
        NodeKind::Instance { frame: FrameProps::default(), component: None, component_key: None },
        NodeKind::Sticky(StickyProps::default()),
        NodeKind::Connector(ConnectorProps::default()),
        NodeKind::ShapeWithText(ShapeWithTextProps::default()),
        NodeKind::Stamp { kind: "star".into() },
        NodeKind::Unknown { type_name: "WIDGET".into() },
    ];
    for k in &kinds {
        match k {
            NodeKind::Page { .. }
            | NodeKind::Frame(_)
            | NodeKind::Group
            | NodeKind::Section
            | NodeKind::Rectangle
            | NodeKind::Ellipse(_)
            | NodeKind::Line
            | NodeKind::Polygon { .. }
            | NodeKind::Star { .. }
            | NodeKind::Vector { .. }
            | NodeKind::Text(_)
            | NodeKind::BooleanOp(_)
            | NodeKind::Component(_)
            | NodeKind::ComponentSet(_)
            | NodeKind::Instance { .. }
            | NodeKind::Sticky(_)
            | NodeKind::Connector(_)
            | NodeKind::ShapeWithText(_)
            | NodeKind::Stamp { .. }
            | NodeKind::Unknown { .. } => {}
        }
    }
    kinds
}

/// Tiny deterministic generator so a failing combination can be replayed from its seed.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

#[test]
fn the_corpus_edits_every_field_a_populated_node_serializes() {
    let Ok(serde_json::Value::Object(map)) = serde_json::to_value(populated()) else { panic!("a node serializes to an object") };
    let serialized: BTreeSet<&str> = map.keys().map(String::as_str).filter(|k| !matches!(*k, "id" | "parent" | "children")).collect();
    let edited: BTreeSet<&str> = field_edits().iter().map(|(f, _)| *f).collect();
    assert_eq!(serialized, edited, "every serialized field of a populated node needs edits here, and every edit names a serialized field");
    assert_eq!(serialized.len(), 31);
}

#[test]
fn every_field_edit_is_reported_as_the_value_diff_reported_it() {
    let base = populated();
    for (field, edits) in field_edits() {
        let mut reported_alone = false;
        for (i, edit) in edits.iter().enumerate() {
            let mut after = base.clone();
            edit(&mut after);
            assert_same_as_value_diff(&base, &after, &format!("{field} edit {i}"));
            assert_same_as_value_diff(&after, &base, &format!("{field} edit {i}, reversed"));
            reported_alone |= value_diff_oracle(&base, &after) == [field.to_string()];
        }
        assert!(reported_alone, "no edit of `{field}` changed that field and no other: the corpus does not exercise it");
    }
}

#[test]
fn an_edit_of_identity_or_tree_position_reports_nothing() {
    let base = populated();
    for (what, edit) in ignored_edits() {
        let mut after = base.clone();
        edit(&mut after);
        assert!(value_diff_oracle(&base, &after).is_empty(), "{what}: the reference reports nothing");
        assert_same_as_value_diff(&base, &after, what);
        // together with a real change, only the real change is reported
        let mut both = after.clone();
        both.name = "Both".into();
        assert_eq!(diff_node_fields(&base, &both), vec!["name".to_string()], "{what} next to a rename");
    }
}

#[test]
fn a_change_between_any_two_kinds_is_reported_as_the_value_diff_reported_it() {
    let kinds = every_kind();
    assert!(kinds.len() >= 20);
    let with_kind = |kind: &NodeKind| {
        let mut n = populated();
        n.kind = kind.clone();
        n
    };
    for a in &kinds {
        for b in &kinds {
            let (na, nb) = (with_kind(a), with_kind(b));
            assert_same_as_value_diff(&na, &nb, &format!("{a:?} -> {b:?}"));
            let expected: Vec<String> = if a == b { Vec::new() } else { vec!["kind".to_string()] };
            assert_eq!(diff_node_fields(&na, &nb), expected, "{a:?} -> {b:?}");
        }
    }
}

#[test]
fn random_combinations_of_edits_are_reported_as_the_value_diff_reported_them() {
    let edits = field_edits();
    let kinds = every_kind();
    let mut rng = Lcg(0x5EED_0993);
    let mut seen_fields = BTreeSet::new();
    for round in 0..3_000 {
        let make = |rng: &mut Lcg| {
            let mut n = populated();
            for _ in 0..rng.below(7) {
                let (_, variants) = &edits[rng.below(edits.len())];
                variants[rng.below(variants.len())](&mut n);
            }
            if rng.below(4) == 0 {
                n.kind = kinds[rng.below(kinds.len())].clone();
            }
            n
        };
        let (a, b) = (make(&mut rng), make(&mut rng));
        seen_fields.extend(value_diff_oracle(&a, &b));
        assert_same_as_value_diff(&a, &b, &format!("round {round} of seed 0x5EED0993"));
    }
    assert_eq!(seen_fields.len(), 31, "the random pairs differ in every field at least once, saw {seen_fields:?}");
}

#[test]
fn floats_the_json_form_cannot_tell_apart_are_not_a_change() {
    let base = populated();
    let numeric: Vec<(&str, Edit)> = vec![
        ("opacity", |n| n.opacity = f32::NAN),
        ("transform.tx", |n| n.transform.tx = f32::NAN),
        ("size.x", |n| n.size.x = f32::INFINITY),
        ("corner_radii[1]", |n| n.corner_radii[1] = f32::NAN),
        ("stroke.weight", |n| n.stroke.weight = f32::NAN),
        ("fills[0].opacity", |n| n.fills[0].opacity = f32::NAN),
    ];
    for (what, to_nan) in &numeric {
        let (mut a, mut b) = (base.clone(), base.clone());
        to_nan(&mut a);
        to_nan(&mut b);
        // NaN is not equal to itself, yet both nodes serialize the same `null`
        assert!(diff_node_fields(&a, &b).is_empty(), "{what}: NaN on both sides is no change");
        assert_same_as_value_diff(&a, &b, what);
    }
    // infinity and NaN are both `null` in JSON, so moving between them is no change either
    let (mut a, mut b) = (base.clone(), base.clone());
    a.opacity = f32::NAN;
    b.opacity = f32::INFINITY;
    assert!(diff_node_fields(&a, &b).is_empty());
    assert_same_as_value_diff(&a, &b, "NaN to infinity");
    // and a real value next to a non-finite one is a change, in both directions
    let (mut a, mut b) = (base.clone(), base.clone());
    a.opacity = f32::NAN;
    b.opacity = 0.75;
    assert_eq!(diff_node_fields(&a, &b), vec!["opacity".to_string()]);
    assert_eq!(diff_node_fields(&b, &a), vec!["opacity".to_string()]);
    assert_same_as_value_diff(&a, &b, "NaN to a number");
    // -0.0 equals 0.0, in the nodes and in their JSON
    let (mut a, mut b) = (base.clone(), base.clone());
    a.transform.tx = 0.0;
    b.transform.tx = -0.0;
    a.corner_smoothing = 0.0;
    b.corner_smoothing = -0.0;
    assert!(diff_node_fields(&a, &b).is_empty());
    assert_same_as_value_diff(&a, &b, "signed zero");
}

#[test]
fn values_that_differ_but_serialize_alike_are_not_a_change() {
    let base = populated();

    // a span of 0 and a span of 1 are both left out of the JSON (`skip_serializing_if`), next to a field that is not
    let span = |n: u16| -> Node {
        let mut node = base.clone();
        node.layout_child = LayoutChild { grow: 1.0, col_span: n, row_span: n, ..Default::default() };
        node
    };
    assert_ne!(span(0), span(1), "the nodes differ as values");
    assert!(value_diff_oracle(&span(0), &span(1)).is_empty(), "and not as JSON");
    assert!(diff_node_fields(&span(0), &span(1)).is_empty());
    assert_same_as_value_diff(&span(0), &span(1), "span 0 against 1");
    // a span of 2 is written, so it is a change
    assert_eq!(diff_node_fields(&span(1), &span(2)), vec!["layout_child".to_string()]);
    assert_same_as_value_diff(&span(1), &span(2), "span 1 against 2");

    // `Some(NaN)` and `None` are both `null`
    let limit = |min_w: Option<f32>| -> Node {
        let mut node = base.clone();
        node.layout_child = LayoutChild { grow: 1.0, min_w, ..Default::default() };
        node
    };
    assert_ne!(limit(Some(f32::NAN)), limit(None));
    assert!(diff_node_fields(&limit(Some(f32::NAN)), &limit(None)).is_empty());
    assert_same_as_value_diff(&limit(Some(f32::NAN)), &limit(None), "Some(NaN) against None");
    assert_eq!(diff_node_fields(&limit(Some(1.5)), &limit(None)), vec!["layout_child".to_string()]);
    assert_same_as_value_diff(&limit(Some(1.5)), &limit(None), "Some(1.5) against None");

    // a NaN keeps two equal matrices apart as values, and a signed zero inside them must not tell them apart as JSON
    let matrix = |tx: f32| -> Node {
        let mut node = base.clone();
        node.transform = Affine { a: f32::NAN, b: 0.0, c: 0.0, d: 1.0, tx, ty: 0.0 };
        node
    };
    assert_ne!(matrix(0.0), matrix(-0.0));
    assert!(diff_node_fields(&matrix(0.0), &matrix(-0.0)).is_empty());
    assert_same_as_value_diff(&matrix(0.0), &matrix(-0.0), "NaN with a signed zero");
    assert_eq!(diff_node_fields(&matrix(0.0), &matrix(1.0)), vec!["transform".to_string()]);
}

#[test]
fn a_field_that_serializes_as_absent_is_a_change_against_one_that_serializes_as_null() {
    // `fill_geometry: Some(vec![])` is `[]` and `None` is absent; an empty `fills` is absent, a one-paint `fills` is not
    let base = populated();
    let (mut a, mut b) = (base.clone(), base.clone());
    a.fill_geometry = None;
    b.fill_geometry = Some(Vec::new());
    assert_eq!(diff_node_fields(&a, &b), vec!["fill_geometry".to_string()]);
    a.fills.clear();
    assert_eq!(diff_node_fields(&a, &b), vec!["fill_geometry".to_string(), "fills".to_string()]);
    assert_same_as_value_diff(&a, &b, "absent against present");
}

fn fixture_nodes() -> Vec<Node> {
    let landing = designos_doc::Document::from_json(include_str!("../../tests/fixtures/designos-studio-landing.designos")).expect("the landing file loads");
    let rich = designos_fig::import_fig(include_bytes!("../../../designos-fig/tests/fixtures/legacy-0.4.0-rich.fig")).expect("the rich .fig imports");
    let grids = designos_fig::import_fig(include_bytes!("../../../designos-fig/tests/fixtures/legacy-0.4.0-grids.fig")).expect("the grids .fig imports");
    let mut out: Vec<Node> = Vec::new();
    for doc in [&landing, &rich, &grids] {
        out.extend(doc.nodes.values().cloned());
    }
    out
}

#[test]
fn nodes_of_real_documents_are_reported_as_the_value_diff_reported_them() {
    let nodes = fixture_nodes();
    assert!(nodes.len() > 100, "the fixtures hold {} nodes", nodes.len());
    let kinds: BTreeSet<String> = nodes.iter().map(|n| format!("{:?}", std::mem::discriminant(&n.kind))).collect();
    assert!(kinds.len() >= 5, "the fixtures cover {} kinds of node", kinds.len());
    let mut fields = BTreeSet::new();
    let n = nodes.len();
    for (i, a) in nodes.iter().enumerate() {
        for b in [&nodes[(i + 1) % n], &nodes[(i * 37 + 11) % n], &nodes[(i * 101 + 5) % n]] {
            fields.extend(value_diff_oracle(a, b));
            assert_same_as_value_diff(a, b, &format!("fixture node {i} against another"));
        }
        // one real node edited in one field, the shape of a palette operation
        let mut renamed = a.clone();
        renamed.name.push_str(" renamed");
        assert_eq!(diff_node_fields(a, &renamed), vec!["name".to_string()], "node {i}");
        let mut hidden = a.clone();
        hidden.visible = !hidden.visible;
        assert_same_as_value_diff(a, &hidden, &format!("fixture node {i} visibility"));
        assert_eq!(diff_node_fields(a, a), Vec::<String>::new(), "node {i} against itself");
    }
    assert!(fields.len() >= 8, "the fixture pairs differ in {} distinct fields", fields.len());
}

#[test]
fn a_user_edit_through_the_app_records_what_the_value_diff_finds_in_each_touched_node() {
    let nodes = fixture_nodes();
    let mut doc = designos_doc::Document::new("Record");
    let page = doc.add_page("Page 1");
    let ids: Vec<NodeId> = (0..40)
        .map(|i| {
            let id = doc.alloc_id();
            let mut n = nodes[(i * 13) % nodes.len()].clone();
            n.id = id;
            n.parent = None;
            n.children.clear();
            doc.insert(page, n).expect("insert");
            id
        })
        .collect();
    let mut app = App::new(doc);
    let mut tx = Tx::new();
    for (i, id) in ids.iter().enumerate() {
        let n = tx.ed.node(&mut app.doc, *id).expect("node");
        match i % 5 {
            0 => n.name = format!("Renamed {i}"),
            1 => n.opacity = 0.5,
            2 => n.transform.tx += 17.0,
            3 => n.fills = vec![Paint::solid(Color::rgb(7, 7, 7))],
            _ => {} // touched, left alone
        }
    }
    let before = tx.ed.before_nodes().clone();
    assert!(app.run_tx(tx));
    let mut expected: BTreeSet<(NodeId, String)> = BTreeSet::new();
    for (id, was) in &before {
        let now = app.doc.get(*id).expect("still there");
        for f in value_diff_oracle(was, now) {
            expected.insert((*id, f));
        }
    }
    assert!(!expected.is_empty());
    let recorded: BTreeSet<(NodeId, String)> = app.agent_busy_frames.user_edited_fields.keys().cloned().collect();
    assert_eq!(recorded, expected, "what the app recorded for the step");
}

#[test]
fn nodes_a_step_touched_and_left_alone_add_no_entries_and_no_work() {
    let mut doc = designos_doc::Document::new("Untouched");
    let page = doc.add_page("Page 1");
    let ids: Vec<NodeId> = (0..300)
        .map(|i| {
            let id = doc.alloc_id();
            doc.insert(page, Node::rectangle(id, designos_doc::Rect::new(i as f32, 0.0, 10.0, 10.0))).expect("insert");
            id
        })
        .collect();
    let mut app = App::new(doc);
    let mut tx = Tx::new();
    for id in &ids {
        tx.ed.node(&mut app.doc, *id).expect("node");
    }
    tx.ed.node(&mut app.doc, ids[123]).expect("node").name = "Renamed".into();
    assert!(app.run_tx(tx));
    let recorded: Vec<(NodeId, String)> = app.agent_busy_frames.user_edited_fields.keys().cloned().collect();
    assert_eq!(recorded, vec![(ids[123], "name".to_string())], "only the node that changed, and only the field that changed");

    // the same for the record alone: no entry and no allocation for a node that did not change
    let nodes: Vec<Node> = ids.iter().map(|id| app.doc.get(*id).expect("node").clone()).collect();
    let twins = nodes.clone();
    let mut frames = AgentBusyFrames::default();
    let ((), allocations) = allocations_in(|| {
        for (a, b) in nodes.iter().zip(&twins) {
            frames.record_user_node_update(a, b);
        }
    });
    assert!(frames.user_edited_fields.is_empty() && frames.both_changed.is_empty());
    assert_eq!(allocations, 0, "an unchanged node costs a comparison, not an allocation");

    let (passes_before, encoded_before) = (crate::node_field_diff::field_passes_on_this_thread(), crate::node_field_diff::encoded_bytes_on_this_thread());
    for (a, b) in nodes.iter().zip(&twins) {
        frames.record_user_node_update(a, b);
    }
    assert_eq!(crate::node_field_diff::field_passes_on_this_thread(), passes_before, "an unchanged node is not compared field by field");
    assert_eq!(crate::node_field_diff::encoded_bytes_on_this_thread(), encoded_before, "an unchanged node is not encoded at all");
}

/// A node the way a large page holds them: geometry in `kind`, in `fill_geometry` and in `stroke_geometry`.
fn heavy_vector(id: u64) -> Node {
    let mut n = Node::new(NodeId(id), NodeKind::Vector { paths: vec![path_with(400), path_with(300)], network: None });
    n.name = format!("Vector {id}");
    n.fills = vec![Paint::solid(Color::rgb(1, 2, 3)), Paint::solid(Color::rgb(4, 5, 6))];
    n.fill_geometry = Some(vec![path_with(200)]);
    n.stroke_geometry = Some(vec![path_with(200)]);
    n.effects = vec![drop_shadow(), drop_shadow()];
    n.plugin_data.insert("plugin:key".into(), "value".repeat(40));
    n.fig_metadata.insert("version".into(), "1".repeat(100));
    n
}

fn heavy_text(id: u64) -> Node {
    let mut n = Node::new(NodeId(id), NodeKind::Text(TextProps { content: "lorem ipsum ".repeat(60), ..Default::default() }));
    n.name = format!("Text {id}");
    n
}

#[test]
fn recording_a_renamed_heavy_node_allocates_a_bounded_number_of_times_and_builds_no_json() {
    let count = 500usize;
    let befores: Vec<Node> = (0..count as u64).map(|i| if i % 2 == 0 { heavy_vector(10 + i) } else { heavy_text(10 + i) }).collect();
    let afters: Vec<Node> = befores
        .iter()
        .map(|n| {
            let mut a = n.clone();
            a.name = format!("{} renamed", n.name);
            a
        })
        .collect();
    let mut frames = AgentBusyFrames::default();
    // the first records size the reused buffers; the measured ones do not grow them
    for (b, a) in befores.iter().zip(&afters).take(4) {
        frames.record_user_node_update(b, a);
    }
    frames.user_edited_fields.clear();
    let ((), allocations) = allocations_in(|| {
        for (b, a) in befores.iter().zip(&afters) {
            frames.record_user_node_update(b, a);
        }
    });
    assert_eq!(frames.user_edited_fields.len(), count, "one entry per renamed node");
    eprintln!("[RECORD COST] {count} renamed heavy nodes: {allocations} allocations ({:.2} per node)", allocations as f64 / count as f64);
    // Inherent per entry: the field-name `String` of the key and the map's own growth. Two `serde_json::Value`s of a heavy
    // node cost hundreds of allocations each.
    assert!(
        allocations <= 3 * count,
        "{allocations} allocations for {count} changed nodes ({} per node): the record builds more than its entries",
        allocations / count
    );
}

#[test]
fn only_the_fields_that_changed_are_encoded_not_the_geometry_around_them() {
    let before = heavy_vector(10);
    let whole_node_bytes = serde_json::to_vec(&before).expect("serialize").len();
    assert!(whole_node_bytes > 20_000, "the node is heavy: {whole_node_bytes} bytes of JSON");

    let encoded = |a: &Node, b: &Node| {
        let start = crate::node_field_diff::encoded_bytes_on_this_thread();
        let fields = diff_node_fields(a, b);
        (fields, crate::node_field_diff::encoded_bytes_on_this_thread() - start)
    };
    let mut renamed = before.clone();
    renamed.name = "Vector renamed".into();
    let (fields, bytes) = encoded(&before, &renamed);
    assert_eq!(fields, vec!["name".to_string()]);
    assert!(bytes < 100, "a rename encodes the two names ({bytes} bytes), not the {whole_node_bytes} bytes of the node");

    let mut hidden = before.clone();
    hidden.visible = false;
    let (fields, bytes) = encoded(&before, &hidden);
    assert_eq!(fields, vec!["visible".to_string()]);
    assert!(bytes < 100, "a visibility change encodes a boolean on each side ({bytes} bytes)");

    // a changed `kind` is encoded because it is the field that changed, and only it
    let mut reshaped = before.clone();
    if let NodeKind::Vector { paths, .. } = &mut reshaped.kind {
        paths.truncate(1);
    }
    let (fields, kind_only) = encoded(&before, &reshaped);
    assert_eq!(fields, vec!["kind".to_string()]);

    // the same node with every heavy field changed encodes far more than the one field did
    let mut everything = reshaped.clone();
    everything.fill_geometry = Some(vec![path_with(100)]);
    everything.stroke_geometry = Some(vec![path_with(100)]);
    everything.fills.clear();
    everything.effects.clear();
    everything.plugin_data.clear();
    everything.fig_metadata.clear();
    let (fields, all_fields) = encoded(&before, &everything);
    assert_eq!(fields.len(), 7, "{fields:?}");
    assert!(kind_only > 1_000, "the changed `kind` itself is encoded: {kind_only} bytes");
    assert!(kind_only < all_fields, "{kind_only} bytes for one changed field, {all_fields} for seven");
}

#[test]
#[ignore = "opt-in real-file benchmark: set DESIGNOS_BENCH_FILE"]
fn ac01_ac04_measure_on_a_real_file() {
    let Some(raw_path) = std::env::var_os("DESIGNOS_BENCH_FILE").filter(|v| !v.is_empty()) else {
        eprintln!("[BENCH] skipped: DESIGNOS_BENCH_FILE is unset");
        return;
    };
    let path = std::path::PathBuf::from(raw_path);
    let basename = path.file_name().and_then(|s| s.to_str()).unwrap_or("<unknown>").to_owned();
    let meta_before = std::fs::metadata(&path).unwrap_or_else(|e| panic!("stat DESIGNOS_BENCH_FILE {}: {e}", path.display()));

    let mut doc = crate::io::load_path(&path).unwrap_or_else(|e| panic!("load DESIGNOS_BENCH_FILE {}: {e}", path.display()));
    let page = match std::env::var("DESIGNOS_BENCH_PAGE").ok().filter(|s| !s.is_empty()) {
        Some(wanted) => doc.pages.iter().copied().find(|&p| doc.get(p).is_some_and(|n| n.name == wanted)).unwrap_or_else(|| {
            let available: Vec<String> = doc.pages.iter().filter_map(|&p| doc.get(p).map(|n| n.name.clone())).collect();
            panic!("DESIGNOS_BENCH_PAGE={wanted:?} not found in {basename}; available pages: {available:?}")
        }),
        None => doc.pages.iter().copied().max_by_key(|&p| doc.page_node_count(p)).expect("document has at least one page"),
    };
    doc.ensure_page_hydrated(page);

    let page_name = doc.get(page).map(|n| n.name.clone()).unwrap_or_default();
    let nodes = doc.page_node_count(page);

    let mut app = App::new(doc);
    app.page = page;
    app.selection.clear();

    let mut page_nodes = Vec::with_capacity(nodes);
    if let Some(pg) = app.doc.get(page) {
        for root in pg.children.clone() {
            app.doc.walk(root, |n, _| page_nodes.push(n.id));
        }
    }

    let t0 = std::time::Instant::now();
    let mut tx = Tx::new();
    for &id in &page_nodes {
        tx.ed.touch(&app.doc, id);
    }
    let queue_was_idle = app.doc.pending_relayout().is_empty();
    let _fixed = crate::cmdk::auto_fix_design_gate(&mut app.doc, page, &[], None);
    if queue_was_idle {
        tx.ed.rebase_layout_epoch(&app.doc);
    }
    let t_record = std::time::Instant::now();
    let (_committed, tx_allocs) = allocations_in(|| app.run_tx(tx));
    let record_loop_ms = t_record.elapsed().as_secs_f64() * 1000.0;
    let autofix_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let user_edited_fields = app.agent_busy_frames.user_edited_fields.len();

    let meta_after = std::fs::metadata(&path).unwrap_or_else(|e| panic!("stat DESIGNOS_BENCH_FILE after run {}: {e}", path.display()));
    assert_eq!(
        (meta_after.len(), meta_after.modified().ok()),
        (meta_before.len(), meta_before.modified().ok()),
        "ac01_ac04_measure_on_a_real_file must never write the input file"
    );

    eprintln!(
        "[BENCH] file={basename} page={page_name} nodes={nodes} autofix_ms={autofix_ms:.2} record_loop_ms={record_loop_ms:.2} tx_allocs={tx_allocs} user_edited_fields={user_edited_fields}"
    );
}

#[test]
fn record_user_node_update_sets_both_changed_when_the_agent_wrote_the_same_field_this_turn() {
    let mut before = populated();
    before.id = NodeId(40);
    let mut after = before.clone();
    after.opacity = 0.2;
    after.name = "Renamed".into();
    let mut frames = AgentBusyFrames::default();
    frames.start_turn(3);
    frames.agent_edited_fields.insert((NodeId(40), "opacity".into()), 3);
    frames.agent_edited_fields.insert((NodeId(40), "name".into()), 2); // an earlier turn: not concurrent
    frames.record_user_node_update(&before, &after);
    assert_eq!(frames.both_changed_for_turn(3), vec![(NodeId(40), "opacity".to_string())]);
    let user: Vec<_> = frames.user_edited_fields.iter().map(|((id, f), t)| (*id, f.clone(), *t)).collect();
    assert_eq!(user, vec![(NodeId(40), "name".to_string(), 3), (NodeId(40), "opacity".to_string(), 3)]);
}

const SYNTHETIC_KIT_PAGE_TOTAL_NODES: usize = 21_978;

fn make_text_node(id: NodeId, x: f32, y: f32, content: impl Into<String>, font_size: f32) -> Node {
    use designos_doc::{Rect, TextStyle};
    Node::text(id, Rect::new(x, y, 120.0, 20.0), content, TextStyle { font_size, ..Default::default() })
}

/// Builds a synthetic 21,978-node document shaped like a design-system kit page:
/// 1 `Page` root + 21,977 nested auto-layout section frames, component cards, header/body/footer groups,
/// interactive buttons, vector icons, and text nodes with fills.
fn build_synthetic_kit_page_21978_nodes() -> (Document, NodeId, Vec<NodeId>) {
    use designos_doc::{AutoLayout, Rect};

    let mut doc = Document::new("Synthetic Kit Page");
    let page = doc.add_page("Components");
    let mut card_ids = Vec::with_capacity(1_600);

    // 39 section frames * 40 component cards * (1 card + 13 children) = 21,840 nodes + 39 sections + 1 page = 21,880 nodes.
    for sec_idx in 0..39u32 {
        let sec_id = doc.alloc_id();
        let mut sec = Node::frame(sec_id, Rect::new(0.0, sec_idx as f32 * 420.0, 1440.0, 400.0));
        sec.name = format!("Section {sec_idx}");
        sec.fills = vec![Paint::solid(Color::rgb(18, 18, 22))];
        if let Some(fp) = sec.kind.frame_props_mut() {
            fp.auto_layout = Some(AutoLayout { item_spacing: 16.0, padding: [16.0, 16.0, 16.0, 16.0], ..Default::default() });
        }
        doc.insert(page, sec).expect("insert section");

        for card_idx in 0..40u32 {
            let ordinal = sec_idx * 40 + card_idx;
            let needs_fix = ordinal % 16 == 0;
            let card_id = doc.alloc_id();
            let mut card = Node::frame(card_id, Rect::new(card_idx as f32 * 260.0, 0.0, 240.0, 320.0));
            card.name = format!("Card {ordinal}");
            card.fills = vec![Paint::solid(Color::rgb(28, 28, 34))];
            if let Some(fp) = card.kind.frame_props_mut() {
                fp.auto_layout = Some(AutoLayout {
                    // Off-4px spacing on `needs_fix` cards triggers `auto_fix_design_gate`
                    item_spacing: if needs_fix { 10.0 } else { 12.0 },
                    padding: if needs_fix { [14.0, 16.0, 14.0, 16.0] } else { [16.0, 16.0, 16.0, 16.0] },
                    ..Default::default()
                });
            }
            doc.insert(sec_id, card).expect("insert card");
            card_ids.push(card_id);

            // 1. Header frame + 2 texts + 1 badge frame + 1 badge text (5 nodes)
            let header_id = doc.alloc_id();
            let mut header = Node::frame(header_id, Rect::new(0.0, 0.0, 208.0, 48.0));
            header.name = "Header".into();
            doc.insert(card_id, header).expect("insert header");

            let title_id = doc.alloc_id();
            let mut title = make_text_node(title_id, 0.0, 0.0, format!("Component {ordinal}"), 14.0);
            title.fills = vec![Paint::solid(Color::rgb(244, 244, 246))];
            doc.insert(header_id, title).expect("insert title");

            let subtitle_id = doc.alloc_id();
            let mut subtitle = make_text_node(subtitle_id, 0.0, 20.0, "Subtitle caption", if needs_fix { 10.0 } else { 12.0 });
            subtitle.fills = vec![Paint::solid(Color::rgb(160, 160, 172))];
            doc.insert(header_id, subtitle).expect("insert subtitle");

            let badge_id = doc.alloc_id();
            let mut badge = Node::frame(badge_id, Rect::new(148.0, 0.0, 60.0, 24.0));
            badge.name = "Status Badge".into();
            badge.fills = vec![Paint::solid(Color::rgb(34, 197, 94))];
            doc.insert(header_id, badge).expect("insert badge");

            let badge_txt_id = doc.alloc_id();
            let badge_txt = make_text_node(badge_txt_id, 8.0, 4.0, "Active", 12.0);
            doc.insert(badge_id, badge_txt).expect("insert badge text");

            // 2. Body frame + 1 description text + 1 vector icon + 1 divider rect (4 nodes)
            let body_id = doc.alloc_id();
            let mut body = Node::frame(body_id, Rect::new(0.0, 60.0, 208.0, 140.0));
            body.name = "Body".into();
            doc.insert(card_id, body).expect("insert body");

            let desc_id = doc.alloc_id();
            let mut desc = make_text_node(desc_id, 0.0, 0.0, "Nested kit card body text", 12.0);
            desc.fills = vec![Paint::solid(Color::rgb(212, 212, 216))];
            doc.insert(body_id, desc).expect("insert desc");

            let icon_id = doc.alloc_id();
            let mut icon = Node::new(icon_id, NodeKind::Vector { paths: vec![path_with(8)], network: None });
            icon.name = "Icon Vector".into();
            icon.size = Vec2::new(20.0, 20.0);
            icon.fills = vec![Paint::solid(Color::rgb(59, 130, 246))];
            doc.insert(body_id, icon).expect("insert icon");

            let div_id = doc.alloc_id();
            let mut div = Node::rectangle(div_id, Rect::new(0.0, 120.0, 208.0, 1.0));
            div.name = "Divider".into();
            div.fills = vec![Paint::solid(Color::rgb(63, 63, 70))];
            doc.insert(body_id, div).expect("insert divider");

            // 3. Footer frame + 1 button frame + 1 button text + 1 meta text (4 nodes)
            let footer_id = doc.alloc_id();
            let mut footer = Node::frame(footer_id, Rect::new(0.0, 212.0, 208.0, 48.0));
            footer.name = "Footer".into();
            doc.insert(card_id, footer).expect("insert footer");

            let btn_id = doc.alloc_id();
            let mut btn = Node::frame(btn_id, Rect::new(0.0, 0.0, 96.0, if needs_fix { 28.0 } else { 36.0 }));
            btn.name = "Button Primary".into();
            btn.fills = vec![Paint::solid(Color::rgb(37, 99, 235))];
            doc.insert(footer_id, btn).expect("insert button");

            let btn_txt_id = doc.alloc_id();
            let btn_txt = make_text_node(btn_txt_id, 16.0, 8.0, "Action", 12.0);
            doc.insert(btn_id, btn_txt).expect("insert button text");

            let meta_id = doc.alloc_id();
            let meta = make_text_node(meta_id, 112.0, 8.0, "v8.0", 12.0);
            doc.insert(footer_id, meta).expect("insert meta");
        }
    }

    // Pad with nested [Frame -> Text] pairs to reach exactly 21,978 nodes in `doc.nodes`.
    while doc.nodes.len() < SYNTHETIC_KIT_PAGE_TOTAL_NODES {
        let id = doc.alloc_id();
        if doc.nodes.len() + 1 < SYNTHETIC_KIT_PAGE_TOTAL_NODES {
            let mut chip = Node::frame(id, Rect::new(0.0, 0.0, 64.0, 32.0));
            chip.name = "Chip".into();
            chip.fills = vec![Paint::solid(Color::rgb(39, 39, 42))];
            doc.insert(page, chip).expect("insert chip");
            let label_id = doc.alloc_id();
            let label = make_text_node(label_id, 8.0, 6.0, "Tag", 12.0);
            doc.insert(id, label).expect("insert tag");
        } else {
            let mut rect = Node::rectangle(id, Rect::new(0.0, 0.0, 32.0, 32.0));
            rect.fills = vec![Paint::solid(Color::rgb(39, 39, 42))];
            doc.insert(page, rect).expect("insert final rect");
        }
    }
    assert_eq!(doc.nodes.len(), SYNTHETIC_KIT_PAGE_TOTAL_NODES);
    (doc, page, card_ids)
}

#[test]
fn ac01_synthetic_kit_page_21978_nodes_autofix_and_record_loop_under_200ms() {
    let (doc, page, card_ids) = build_synthetic_kit_page_21978_nodes();
    let mut app = App::new(doc);
    app.page = page;
    app.selection.clear();

    // 1. Run the palette auto-fix (#988) path (`run_auto_fix_design_gate`) on the 21,978-node synthetic page.
    let passes_before_autofix = crate::node_field_diff::field_passes_on_this_thread();
    let t0 = std::time::Instant::now();
    let outcome = crate::cmdk::run_auto_fix_design_gate(&mut app);
    let autofix_elapsed = t0.elapsed();
    let autofix_passes = crate::node_field_diff::field_passes_on_this_thread() - passes_before_autofix;
    let autofix_edited_len = app.agent_busy_frames.user_edited_fields.len();
    assert!(matches!(outcome, crate::cmdk::AutoFixOutcome::Committed { .. }), "expected Committed, got {outcome:?}");

    // 2. Run a whole-page `run_tx` touching all 21,977 page nodes and updating 100 card fills.
    let all_page_nodes: Vec<NodeId> = app.doc.nodes.keys().copied().filter(|&id| id != page).collect();
    assert_eq!(all_page_nodes.len(), SYNTHETIC_KIT_PAGE_TOTAL_NODES - 1);
    let mut tx = Tx::new();
    for &id in &all_page_nodes {
        tx.ed.touch(&app.doc, id);
    }
    for &id in card_ids.iter().take(100) {
        tx.ed.node(&mut app.doc, id).expect("card").fills = vec![Paint::solid(Color::rgb(16, 185, 129))];
    }
    let passes_before_tx = crate::node_field_diff::field_passes_on_this_thread();
    let t1 = std::time::Instant::now();
    let (committed, tx_allocs) = allocations_in(|| app.run_tx(tx));
    let tx_elapsed = t1.elapsed();
    let tx_passes = crate::node_field_diff::field_passes_on_this_thread() - passes_before_tx;
    assert!(committed);

    eprintln!(
        "[AC-01] nodes={} autofix_ms={:.2} autofix_passes={} autofix_user_edited_fields={} whole_page_run_tx_ms={:.2} tx_passes={} tx_allocs={}",
        app.doc.nodes.len(),
        autofix_elapsed.as_secs_f64() * 1000.0,
        autofix_passes,
        autofix_edited_len,
        tx_elapsed.as_secs_f64() * 1000.0,
        tx_passes,
        tx_allocs,
    );

    assert_eq!(
        tx_passes, 100,
        "R1: unchanged nodes on the 21,978-node page must be dropped before the record loop (expected 100 field passes, got {tx_passes})"
    );
    assert!(
        tx_allocs < 2_500,
        "R1/R2: whole-page run_tx with 100 changed nodes allocated {tx_allocs} times (expected < 2,500 without cloning 21,977 before_nodes)"
    );
}

#[test]
fn ac02_tx_touching_10000_nodes_changing_10_drops_unchanged_before_record_loop_and_builds_no_json() {
    use designos_doc::Rect;

    let mut doc = Document::new("AC-02");
    let page = doc.add_page("Page 1");
    let mut ids = Vec::with_capacity(10_000);
    for i in 0..10_000u32 {
        let id = doc.alloc_id();
        let mut node = if i % 3 == 0 {
            Node::frame(id, Rect::new(0.0, 0.0, 120.0, 48.0))
        } else if i % 3 == 1 {
            make_text_node(id, 0.0, 0.0, format!("Label {i}"), 13.0)
        } else {
            Node::rectangle(id, Rect::new(0.0, 0.0, 64.0, 32.0))
        };
        node.fills = vec![Paint::solid(Color::rgb(40, 44, 52))];
        doc.insert(page, node).expect("insert");
        ids.push(id);
    }

    let mut app = App::new(doc);
    app.page = page;

    // Warm up thread-local tape buffers so buffer growth is not conflated with per-tx allocations.
    if let (Some(a), Some(b)) = (app.doc.get(ids[0]).cloned(), app.doc.get(ids[1]).cloned()) {
        app.agent_busy_frames.record_user_node_update(&a, &b);
        app.agent_busy_frames.user_edited_fields.clear();
    }

    let mut tx = Tx::new();
    for &id in &ids {
        tx.ed.touch(&app.doc, id);
    }
    let changed_ids: Vec<NodeId> = (0..10).map(|k| ids[k * 1_000 + 7]).collect();
    for &id in &changed_ids {
        tx.ed.node(&mut app.doc, id).expect("node").fills = vec![Paint::solid(Color::rgb(239, 68, 68))];
    }

    let passes_before = crate::node_field_diff::field_passes_on_this_thread();
    let (committed, allocs) = allocations_in(|| app.run_tx(tx));
    let passes_delta = crate::node_field_diff::field_passes_on_this_thread() - passes_before;
    assert!(committed);

    let recorded: Vec<(NodeId, String)> = app.agent_busy_frames.user_edited_fields.keys().cloned().collect();
    let mut expected: Vec<(NodeId, String)> = changed_ids.iter().map(|&id| (id, "fills".to_string())).collect();
    expected.sort();
    assert_eq!(recorded, expected, "AC-02: only the 10 changed nodes are recorded");
    assert_eq!(passes_delta, 10, "AC-02 / R1: only the 10 changed nodes reach the field diff pass");
    eprintln!("[AC-02] 10,000 touched / 10 changed: run_tx allocations = {allocs}, field_passes = {passes_delta}");
    assert!(
        allocs < 300,
        "AC-02: run_tx allocated {allocs} times for 10 changed nodes out of 10,000 touched (unchanged nodes must be dropped before cloning/reverting/recording)"
    );
}

#[test]
fn ac03_user_edits_fill_then_new_agent_turn_sees_exact_same_fields_as_old_behaviour() {
    use designos_doc::Rect;

    let mut doc = Document::new("AC-03");
    let page = doc.add_page("Page 1");
    let ids: Vec<NodeId> = (0..200u32)
        .map(|i| {
            let id = doc.alloc_id();
            let mut n = Node::rectangle(id, Rect::new(i as f32 * 16.0, 0.0, 12.0, 12.0));
            n.fills = vec![Paint::solid(Color::rgb(20, 30, 40))];
            doc.insert(page, n).expect("insert");
            id
        })
        .collect();

    let snapshot_before: std::collections::BTreeMap<NodeId, Node> = ids.iter().map(|&id| (id, doc.get(id).expect("node").clone())).collect();

    let mut app = App::new(doc);
    app.page = page;

    let mut tx = Tx::new();
    for &id in &ids {
        tx.ed.touch(&app.doc, id);
    }
    tx.ed.node(&mut app.doc, ids[5]).expect("node").fills = vec![Paint::solid(Color::rgb(250, 204, 21))];
    tx.ed.node(&mut app.doc, ids[42]).expect("node").fills = vec![Paint::solid(Color::rgb(56, 189, 248))];
    tx.ed.node(&mut app.doc, ids[42]).expect("node").opacity = 0.75;

    let passes_before = crate::node_field_diff::field_passes_on_this_thread();
    let (committed, tx_allocs) = allocations_in(|| app.run_tx(tx));
    let passes_delta = crate::node_field_diff::field_passes_on_this_thread() - passes_before;
    assert!(committed);
    assert_eq!(passes_delta, 2, "only the 2 modified nodes are diffed");
    assert!(
        tx_allocs < 80,
        "AC-03 / R1: touching 200 nodes and editing 2 must drop the 198 unchanged nodes before cloning/reverting in run_tx (got {tx_allocs} allocations)"
    );

    // Compare against the old serde_json::Value diff oracle across all 200 touched nodes (R4).
    let mut oracle_fields = Vec::new();
    for &id in &ids {
        let before = &snapshot_before[&id];
        let after = app.doc.get(id).expect("node");
        for f in value_diff_oracle(before, after) {
            oracle_fields.push((id, f));
        }
    }
    oracle_fields.sort();
    let actual_fields: Vec<(NodeId, String)> = app.agent_busy_frames.user_edited_fields.keys().cloned().collect();
    assert_eq!(actual_fields, oracle_fields, "R4 / AC-03: user_edited_fields must match value_diff_oracle");

    // Now the agent turn runs in the same turn window and writes `fills` on `ids[5]` and `name` on `ids[10]`.
    let state = LivePatchState::default();
    let mut set_5 = serde_json::Map::new();
    set_5.insert("fills".into(), serde_json::json!([]));
    let mut set_10 = serde_json::Map::new();
    set_10.insert("name".into(), serde_json::json!("Agent Renamed"));
    let ops = vec![LiveOp::Update { id: ids[5], set: set_5, unset: vec![] }, LiveOp::Update { id: ids[10], set: set_10, unset: vec![] }];
    app.agent_busy_frames.record_agent_patch_ops(&ops, &state);
    assert_eq!(
        app.agent_busy_frames.both_changed_for_turn(app.agent_busy_frames.current_turn),
        vec![(ids[5], "fills".to_string())],
        "AC-03: agent turn sees the user's fill edit on ids[5] in both_changed"
    );
}

#[test]
fn ac04_synthetic_page_whole_page_edit_reports_user_edited_fields_len() {
    let (doc, page, _card_ids) = build_synthetic_kit_page_21978_nodes();
    let before_snapshot: std::collections::BTreeMap<NodeId, Node> = doc.nodes.clone();

    let mut app = App::new(doc);
    app.page = page;
    app.selection.clear();

    let passes_before = crate::node_field_diff::field_passes_on_this_thread();
    let outcome = crate::cmdk::run_auto_fix_design_gate(&mut app);
    let passes_delta = crate::node_field_diff::field_passes_on_this_thread() - passes_before;
    assert!(matches!(outcome, crate::cmdk::AutoFixOutcome::Committed { .. }));

    let mut oracle_entries = Vec::new();
    for (&id, before) in &before_snapshot {
        if id == page {
            continue;
        }
        if let Some(after) = app.doc.get(id) {
            for field in value_diff_oracle(before, after) {
                oracle_entries.push((id, field));
            }
        }
    }
    oracle_entries.sort();

    let actual_entries: Vec<(NodeId, String)> = app.agent_busy_frames.user_edited_fields.keys().cloned().collect();
    eprintln!("[AC-04] synthetic 21,978-node page whole-page edit: len(user_edited_fields) = {}, field_passes = {}", actual_entries.len(), passes_delta,);
    assert_eq!(actual_entries, oracle_entries, "AC-04 / R4: user_edited_fields on the 21,978-node synthetic page must exactly match value_diff_oracle");

    // Also verify `retain_changed` on a whole-page edit of the 21,978-node synthetic page does zero heap allocations in `properties_changed`.
    let mut tx = Tx::new();
    for &id in before_snapshot.keys().filter(|&&id| id != page) {
        tx.ed.touch(&app.doc, id);
    }
    for &id in before_snapshot.keys().filter(|&&id| id != page).take(588) {
        tx.ed.node(&mut app.doc, id).expect("node").opacity = 0.9;
    }
    let ((), retain_allocs) = allocations_in(|| tx.ed.retain_changed(&app.doc));
    assert_eq!(
        retain_allocs, 0,
        "AC-04 / R1: retain_changed (properties_changed) across 21,977 nodes with 588 changed must not clone nodes (got {retain_allocs} allocations)"
    );
}

#[test]
fn edge_case_empty_transaction_records_nothing() {
    use designos_doc::Rect;

    let mut doc = Document::new("Empty Tx");
    let page = doc.add_page("Page 1");
    let ids: Vec<NodeId> = (0..1_000u32)
        .map(|i| {
            let id = doc.alloc_id();
            doc.insert(page, Node::rectangle(id, Rect::new(i as f32 * 10.0, 0.0, 8.0, 8.0))).expect("insert");
            id
        })
        .collect();
    let mut app = App::new(doc);
    app.page = page;

    // 1. Completely empty Tx
    let passes_before = crate::node_field_diff::field_passes_on_this_thread();
    let (committed_empty, allocs_empty) = allocations_in(|| app.run_tx(Tx::new()));
    assert!(!committed_empty);
    assert_eq!(allocs_empty, 0, "empty Tx allocates nothing in run_tx");
    assert!(app.agent_busy_frames.user_edited_fields.is_empty());

    // 2. Tx that touches 1,000 nodes but changes none of them
    let mut tx = Tx::new();
    for &id in &ids {
        tx.ed.touch(&app.doc, id);
    }
    let (committed_noop, allocs_noop) = allocations_in(|| app.run_tx(tx));
    assert!(!committed_noop);
    assert_eq!(crate::node_field_diff::field_passes_on_this_thread(), passes_before, "no-op transaction must not run field diff passes");
    assert!(app.agent_busy_frames.user_edited_fields.is_empty());
    assert_eq!(
        allocs_noop, 0,
        "no-op transaction touching 1,000 unchanged nodes must drop them before cloning or reverting in run_tx (got {allocs_noop} allocations)"
    );
}

#[test]
fn edge_case_deleted_node_in_tx_records_deletion_without_field_diff() {
    use designos_doc::{Command, Rect};

    let mut doc = Document::new("Delete in Tx");
    let page = doc.add_page("Page 1");
    let parent_frame = doc.alloc_id();
    doc.insert(page, Node::frame(parent_frame, Rect::new(0.0, 0.0, 200.0, 200.0))).expect("insert frame");
    let child_to_delete = doc.alloc_id();
    doc.insert(parent_frame, Node::rectangle(child_to_delete, Rect::new(10.0, 10.0, 50.0, 50.0))).expect("insert child");
    let sibling = doc.alloc_id();
    doc.insert(parent_frame, Node::rectangle(sibling, Rect::new(70.0, 10.0, 50.0, 50.0))).expect("insert sibling");

    let mut app = App::new(doc);
    app.page = page;

    let mut tx = Tx::new();
    // Touch parent frame, deleted child, and untouched sibling before removing the child.
    tx.ed.touch(&app.doc, parent_frame);
    tx.ed.touch(&app.doc, child_to_delete);
    tx.ed.touch(&app.doc, sibling);
    tx.cmd(&mut app.doc, Command::Remove { id: child_to_delete }).expect("remove child");

    let passes_before = crate::node_field_diff::field_passes_on_this_thread();
    assert!(app.run_tx(tx));
    let passes_delta = crate::node_field_diff::field_passes_on_this_thread() - passes_before;

    assert!(app.doc.get(child_to_delete).is_none(), "deleted node is removed");
    assert!(app.agent_busy_frames.user_edited_fields.is_empty(), "deleting a node records no field updates");
    assert_eq!(passes_delta, 0, "neither the deleted child nor the parent frame whose `children` changed should run a field diff pass");
}

#[test]
fn edge_case_new_node_in_tx_records_fields_as_before() {
    use designos_doc::{Command, Rect};

    let mut doc = Document::new("New Node in Tx");
    let page = doc.add_page("Page 1");
    let parent_frame = doc.alloc_id();
    doc.insert(page, Node::frame(parent_frame, Rect::new(0.0, 0.0, 300.0, 300.0))).expect("insert frame");

    let mut app = App::new(doc);
    app.page = page;

    // 1. Insert a brand-new node via Command::Insert while parent_frame is touched in `tx.ed`.
    let new_plain = app.doc.alloc_id();
    let mut node_plain = Node::rectangle(new_plain, Rect::new(10.0, 10.0, 40.0, 40.0));
    node_plain.fills = vec![Paint::solid(Color::rgb(100, 116, 139))];

    // 2. Insert a second new node via Command::Insert and then modify its `fills` via `tx.ed.node` in the same Tx.
    let new_edited = app.doc.alloc_id();
    let mut node_edited = Node::rectangle(new_edited, Rect::new(60.0, 10.0, 40.0, 40.0));
    node_edited.fills = vec![Paint::solid(Color::rgb(100, 116, 139))];
    let node_edited_before = node_edited.clone();

    let mut tx = Tx::new();
    tx.ed.touch(&app.doc, parent_frame);
    tx.cmd(&mut app.doc, Command::Insert { parent: parent_frame, index: 0, nodes: vec![node_plain] }).expect("insert plain");
    tx.cmd(&mut app.doc, Command::Insert { parent: parent_frame, index: 1, nodes: vec![node_edited] }).expect("insert edited");
    tx.ed.node(&mut app.doc, new_edited).expect("new_edited").fills = vec![Paint::solid(Color::rgb(14, 165, 233))];

    let passes_before = crate::node_field_diff::field_passes_on_this_thread();
    assert!(app.run_tx(tx));
    let passes_delta = crate::node_field_diff::field_passes_on_this_thread() - passes_before;

    let expected_edited_fields = value_diff_oracle(&node_edited_before, app.doc.get(new_edited).expect("new_edited"));
    let recorded: Vec<(NodeId, String)> = app.agent_busy_frames.user_edited_fields.keys().cloned().collect();
    let expected: Vec<(NodeId, String)> = expected_edited_fields.into_iter().map(|f| (new_edited, f)).collect();
    assert_eq!(recorded, expected, "new node edited in tx records the exact same fields as before");
    assert_eq!(passes_delta, 1, "only new_edited is diffed; parent_frame (children-only change) is dropped");
}

#[test]
fn edge_case_undo_of_large_edit_has_bounded_cost_and_does_not_rerecord_edited_fields() {
    use designos_doc::Rect;

    let mut doc = Document::new("Undo Large Edit");
    let page = doc.add_page("Page 1");
    let ids: Vec<NodeId> = (0..10_000u32)
        .map(|_i| {
            let id = doc.alloc_id();
            let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 40.0));
            n.fills = vec![Paint::solid(Color::rgb(30, 41, 59))];
            doc.insert(page, n).expect("insert");
            id
        })
        .collect();

    let mut app = App::new(doc);
    app.page = page;

    let mut tx = Tx::new();
    for &id in &ids {
        tx.ed.touch(&app.doc, id);
    }
    let changed_ids: Vec<NodeId> = (0..10).map(|k| ids[k * 1_000]).collect();
    for &id in &changed_ids {
        tx.ed.node(&mut app.doc, id).expect("node").opacity = 0.4;
    }

    let (committed, tx_allocs) = allocations_in(|| app.run_tx(tx));
    assert!(committed);
    assert_eq!(app.agent_busy_frames.user_edited_fields.len(), 10);

    let passes_before_undo = crate::node_field_diff::field_passes_on_this_thread();
    let ((), undo_allocs) = allocations_in(|| app.undo());
    let passes_during_undo = crate::node_field_diff::field_passes_on_this_thread() - passes_before_undo;

    for &id in &changed_ids {
        assert_eq!(app.doc.get(id).expect("node").opacity, 1.0, "undo restores the 10 changed nodes");
    }
    assert_eq!(passes_during_undo, 0, "undo does not run node_field_diff");
    assert_eq!(app.agent_busy_frames.user_edited_fields.len(), 10, "undo leaves user_edited_fields intact for the turn");
    assert!(
        tx_allocs + undo_allocs < 500,
        "large edit (10,000 touched, 10 changed) + undo allocated {} + {} = {} times (expected < 500)",
        tx_allocs,
        undo_allocs,
        tx_allocs + undo_allocs
    );
}

#[test]
fn edge_case_agent_turn_running_during_edit_loses_no_fields() {
    use designos_doc::Rect;

    let mut doc = Document::new("Agent Turn Running During Edit");
    let page = doc.add_page("Page 1");
    let ids: Vec<NodeId> = (0..1_000u32)
        .map(|_i| {
            let id = doc.alloc_id();
            let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 50.0, 50.0));
            n.fills = vec![Paint::solid(Color::rgb(51, 65, 85))];
            doc.insert(page, n).expect("insert");
            id
        })
        .collect();

    let mut app = App::new(doc);
    app.page = page;

    // Agent turn 5 is active and has already patched `fills` on `ids[0]` and `opacity` on `ids[1]`.
    app.agent_busy_frames.start_turn(5);
    let state = LivePatchState::default();
    let mut set_0 = serde_json::Map::new();
    set_0.insert("fills".into(), serde_json::json!([]));
    let mut set_1 = serde_json::Map::new();
    set_1.insert("opacity".into(), serde_json::json!(0.8));
    app.agent_busy_frames
        .record_agent_patch_ops(&[LiveOp::Update { id: ids[0], set: set_0, unset: vec![] }, LiveOp::Update { id: ids[1], set: set_1, unset: vec![] }], &state);

    // While agent turn 5 is running, the user runs a transaction touching all 1,000 nodes and editing `ids[0].fills` and `ids[2].name`.
    let mut tx = Tx::new();
    for &id in &ids {
        tx.ed.touch(&app.doc, id);
    }
    tx.ed.node(&mut app.doc, ids[0]).expect("node 0").fills = vec![Paint::solid(Color::rgb(249, 115, 22))];
    tx.ed.node(&mut app.doc, ids[2]).expect("node 2").name = "User Edited Mid-Turn".into();

    let (committed, allocs) = allocations_in(|| app.run_tx(tx));
    assert!(committed);
    assert!(allocs < 200, "mid-turn user edit touching 1,000 nodes and changing 2 allocated {allocs} times (expected < 200)");

    // Agent turn 5 continues and edits `ids[2].name`.
    let mut set_2 = serde_json::Map::new();
    set_2.insert("name".into(), serde_json::json!("Agent Overwrote Name"));
    app.agent_busy_frames.record_agent_patch_ops(&[LiveOp::Update { id: ids[2], set: set_2, unset: vec![] }], &state);

    assert_eq!(
        app.agent_busy_frames.both_changed_for_turn(5),
        vec![(ids[0], "fills".to_string()), (ids[2], "name".to_string())],
        "both agent-before-user and user-before-agent field edits in the active turn are recorded in both_changed"
    );
    let user_turn_5: Vec<(NodeId, String)> =
        app.agent_busy_frames.user_edited_fields.iter().filter(|(_, &t)| t == 5).map(|((id, f), _)| (*id, f.clone())).collect();
    assert_eq!(user_turn_5, vec![(ids[0], "fills".to_string()), (ids[2], "name".to_string())]);
}
