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
            source_library_key: None,
            extensions: Default::default(),
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
        extensions: Default::default(),
        mode_extensions: Default::default(),
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
            source_library_key: None,
            extensions: Default::default(),
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
            extensions: Default::default(),
            mode_extensions: Default::default(),
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

#[test]
fn white_paint_with_stored_opacity_bound_to_translucent_color_variable_composites_at_twenty_percent() {
    let mut t = T::new();
    let page = t.page;
    t.node(page).kind = NodeKind::Page { background: Color::BLACK };
    t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(0.0, 0.0, 40.0, 40.0));
        n.fills = vec![Paint { opacity: 0.2, ..Paint::solid(Color::WHITE) }];
        n.variables.bindings.insert("fills/0".into(), "alpha-white-20".into());
        n
    });
    t.doc.variable_collections.insert(
        "c".into(),
        VariableCollection {
            id: "c".into(),
            name: "C".into(),
            modes: vec![VariableMode { id: "m".into(), name: "M".into() }],
            default_mode: "m".into(),
            variables: vec!["alpha-white-20".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    let alpha_white_20 = Color { r: 1.0, g: 1.0, b: 1.0, a: 0.2 };
    t.doc.variables.insert(
        "alpha-white-20".into(),
        Variable {
            id: "alpha-white-20".into(),
            name: "Component colors/Alpha/alpha-white-20".into(),
            collection: "c".into(),
            kind: VariableKind::Color,
            values: [("m".to_string(), VariableValue::Color(alpha_white_20))].into_iter().collect(),
            description: String::new(),
            scopes: vec![],
            hidden_from_publishing: false,
            key: None,
            code_syntax: Default::default(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        },
    );

    let pm = t.render(40, 40);
    // 20% white over black is ~51, not 4% (~10).
    assert!(near(px(&pm, 20, 20), [51, 51, 51, 255], 2), "got {:?}", px(&pm, 20, 20));
}

/// A heavy "I" on a mid-grey page: fill white bound to a translucent colour variable (stored
/// paint opacity 0.2, as an import writes it) and an opaque black drop shadow pushed 60 px to the
/// right so the shadow sample sits clear of the glyph. Returns the scene and the shadow offset.
fn translucent_text_with_shadow_scene() -> (T, f32) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| std::env::set_var("DESIGNOS_BUNDLED_FONTS_ONLY", "1"));
    const SHADOW_DX: f32 = 60.0;
    let mut t = T::new();
    let page = t.page;
    t.node(page).kind = NodeKind::Page { background: Color::rgb(128, 128, 128) };
    t.add(None, |id| {
        let style = TextStyle { font_size: 100.0, font_weight: 900, fills: vec![Paint { opacity: 0.2, ..Paint::solid(Color::WHITE) }], ..Default::default() };
        let mut n = Node::text(id, Rect::new(10.0, 10.0, 60.0, 120.0), "I", style);
        if let NodeKind::Text(tx) = &mut n.kind {
            tx.auto_resize = TextAutoResize::WidthAndHeight;
        }
        n.variables.bindings.insert("fills/0".into(), "alpha-white-20".into());
        n.effects = vec![Effect::DropShadow {
            color: Color::rgba(0, 0, 0, 255),
            offset: Vec2::new(SHADOW_DX, 0.0),
            radius: 0.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
        }];
        n
    });
    t.doc.variable_collections.insert(
        "c".into(),
        VariableCollection {
            id: "c".into(),
            name: "C".into(),
            modes: vec![VariableMode { id: "m".into(), name: "M".into() }],
            default_mode: "m".into(),
            variables: vec!["alpha-white-20".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    t.doc.variables.insert(
        "alpha-white-20".into(),
        Variable {
            id: "alpha-white-20".into(),
            name: "Component colors/Alpha/alpha-white-20".into(),
            collection: "c".into(),
            kind: VariableKind::Color,
            values: [("m".to_string(), VariableValue::Color(Color { r: 1.0, g: 1.0, b: 1.0, a: 0.2 }))].into_iter().collect(),
            description: String::new(),
            scopes: vec![],
            hidden_from_publishing: false,
            key: None,
            code_syntax: Default::default(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        },
    );
    (t, SHADOW_DX)
}

#[test]
fn text_drop_shadow_uses_the_bound_colour_alpha_not_full_ink() {
    let (t, dx) = translucent_text_with_shadow_scene();
    let pm = t.render(220, 140);
    if let Ok(dir) = std::env::var("DESIGNOS_TEST_EVIDENCE_DIR") {
        pm.save_png(std::path::Path::new(&dir).join("text-shadow-bound-alpha-cpu.png")).unwrap();
    }
    // glyph stem: the lightened pixels (20% white over 128 grey is ~153) on a mid-height row
    let row = 70;
    let lit: Vec<u32> = (0..220).filter(|&x| px(&pm, x, row)[0] > 140).collect();
    assert!(!lit.is_empty(), "glyph not found");
    let stem = (lit[0] + lit[lit.len() - 1]) / 2;
    assert!(near(px(&pm, stem, row), [153, 153, 153, 255], 3), "glyph got {:?}", px(&pm, stem, row));
    // 20% black over 128 grey is ~102; a fully inked shadow would be ~0
    let sh = px(&pm, stem + dx as u32, row);
    assert!(near(sh, [102, 102, 102, 255], 3), "shadow got {sh:?}");
}
