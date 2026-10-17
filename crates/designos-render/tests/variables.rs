mod common;

use common::*;
use designos_doc::*;
use designos_render::{render_node, TileBudget, TileCache, Viewport};

const LIGHT: Color = Color { r: 0.9, g: 0.9, b: 0.9, a: 1.0 };
const DARK: Color = Color { r: 0.1, g: 0.1, b: 0.1, a: 1.0 };

/// A frame whose fill and width are bound to a Theme collection.
fn themed() -> (T, NodeId, NodeId) {
    let mut t = T::new();
    let frame = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(10.0, 10.0, 60.0, 60.0));
        n.fills = vec![Paint::solid(Color::rgb(255, 0, 255))];
        n
    });
    let child = t.rect(Some(frame), Rect::new(5.0, 5.0, 20.0, 20.0), RED);
    t.node(child).variables.bindings.insert("fills/0".into(), "bg".into());
    t.node(frame).variables.bindings.insert("fills/0".into(), "bg".into());
    t.node(frame).variables.bindings.insert("width".into(), "w".into());
    let d = &mut t.doc;
    d.variable_collections.insert(
        "theme".into(),
        VariableCollection {
            id: "theme".into(),
            name: "Theme".into(),
            modes: vec![VariableMode { id: "l".into(), name: "Light".into() }, VariableMode { id: "d".into(), name: "Dark".into() }],
            default_mode: "l".into(),
            variables: vec!["bg".into(), "w".into()],
            key: None,
        },
    );
    let mk = |id: &str, kind, l, dk| Variable {
        id: id.into(),
        name: id.into(),
        collection: "theme".into(),
        kind,
        values: [("l".to_string(), l), ("d".to_string(), dk)].into_iter().collect(),
        description: String::new(),
        scopes: vec![],
        hidden_from_publishing: false,
        key: None,

        code_syntax: Default::default(),
    };
    d.variables.insert("bg".into(), mk("bg", VariableKind::Color, VariableValue::Color(LIGHT), VariableValue::Color(DARK)));
    d.variables.insert("w".into(), mk("w", VariableKind::Number, VariableValue::Number(60.0), VariableValue::Number(30.0)));
    (t, frame, child)
}

#[test]
fn switching_a_frames_mode_changes_rendered_pixels() {
    let (mut t, frame, _) = themed();
    let light = t.render(100, 100);
    assert!(near(px(&light, 60, 60), [230, 230, 230, 255], 2));
    t.node(frame).variables.modes.insert("theme".into(), "d".into());
    let dark = t.render(100, 100);
    assert!(near(px(&dark, 25, 60), [26, 26, 26, 255], 2), "{:?}", px(&dark, 25, 60));
    // the width variable shrank the frame: the right part is page background again
    assert_eq!(px(&dark, 60, 60), WHITE);
    assert_ne!(light.data(), dark.data());
    let node = render_node(&t.doc, frame, 1.0).unwrap();
    assert_eq!(node.width(), 30);
}

#[test]
fn tile_cache_notices_variable_value_and_mode_edits() {
    let (mut t, frame, _) = themed();
    let vp = Viewport::default();
    let mut cache = TileCache::new();
    cache.prefetch_ring = 0;
    let read = |c: &mut TileCache, t: &T, rev| {
        let f = c.render_tiles(&t.doc, t.page, &vp, (256, 256), rev, TileBudget::unlimited());
        assert_eq!(f.pending(), 0);
        let pm = f.tiles[0].pixmap.clone();
        (px(&pm, 60, 60), px(&pm, 25, 60), f.rendered)
    };
    let (a, _, n) = read(&mut cache, &t, 1);
    assert!(n >= 1 && near(a, [230, 230, 230, 255], 2));
    // same revision: nothing re-rendered
    assert_eq!(read(&mut cache, &t, 1).2, 0);

    // edit the variable's value only: no node changed, yet the tile must refresh
    t.doc.variables.get_mut("bg").unwrap().values.insert("l".into(), VariableValue::Color(Color::rgb(0, 128, 0)));
    let (b, _, n) = read(&mut cache, &t, 2);
    assert!(n >= 1);
    assert!(near(b, [0, 128, 0, 255], 2), "{b:?}");

    // switching the mode on the frame dirties the frame and its bound child
    t.node(frame).variables.modes.insert("theme".into(), "d".into());
    let (c, w, n) = read(&mut cache, &t, 3);
    assert!(n >= 1);
    assert!(near(w, [26, 26, 26, 255], 2), "{w:?}");
    assert_eq!(c, WHITE);
}

#[test]
fn per_side_stroke_renders_with_fill_geometry_and_visible_variable_binding() {
    let mut t = T::new();
    let frame = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(10.0, 10.0, 60.0, 40.0));
        n.fills = vec![];
        // Precomputed fill_geometry on a Frame (as in .fig v106 files) must still keep shape.is_rect = true
        // so per-side border weights draw only the active side instead of stroking all 4 sides.
        n.fill_geometry = Some(vec![Path {
            cmds: vec![
                PathCmd::MoveTo(Vec2::new(0.0, 0.0)),
                PathCmd::LineTo(Vec2::new(60.0, 0.0)),
                PathCmd::LineTo(Vec2::new(60.0, 40.0)),
                PathCmd::LineTo(Vec2::new(0.0, 40.0)),
                PathCmd::Close,
            ],
            fill_rule: FillRule::NonZero,
        }]);
        n.stroke.paints = vec![Paint::solid(RED)];
        n.stroke.weight = 4.0;
        n.stroke.align = StrokeAlign::Inside;
        n.stroke.side_weights = Some([0.0, 0.0, 4.0, 0.0]);
        n.variables.bindings.insert("visible".into(), "vis".into());
        n
    });
    t.doc.variable_collections.insert(
        "c".into(),
        VariableCollection {
            id: "c".into(),
            name: "C".into(),
            modes: vec![VariableMode { id: "on".into(), name: "On".into() }, VariableMode { id: "off".into(), name: "Off".into() }],
            default_mode: "on".into(),
            variables: vec!["vis".into()],
            key: None,
        },
    );
    t.doc.variables.insert(
        "vis".into(),
        Variable {
            id: "vis".into(),
            name: "vis".into(),
            collection: "c".into(),
            kind: VariableKind::Boolean,
            values: [("on".to_string(), VariableValue::Boolean(true)), ("off".to_string(), VariableValue::Boolean(false))].into_iter().collect(),
            description: String::new(),
            scopes: vec![],
            hidden_from_publishing: false,
            key: None,

            code_syntax: Default::default(),
        },
    );

    let pm_on = t.render(100, 80);
    // Bottom border at y=48 (inside 10..50) is RED; top border at y=11 and left border at x=11 are untouched (WHITE).
    assert_eq!(px(&pm_on, 40, 48), [255, 0, 0, 255]);
    assert_eq!(px(&pm_on, 40, 11), WHITE);
    assert_eq!(px(&pm_on, 11, 30), WHITE);

    // Switching mode to "off" resolves `visible = false` via the bound boolean variable.
    t.node(frame).variables.modes.insert("c".into(), "off".into());
    let pm_off = t.render(100, 80);
    assert_eq!(px(&pm_off, 40, 48), WHITE);
}
