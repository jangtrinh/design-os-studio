mod common;
use common::*;
use designos_doc::*;

const BLACK: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };

/// White page with a 10px black stripe at x 10..20 and a clear (alpha 0) pane
/// over x 0..40 that blurs its backdrop. `wrap` decides where the pane lives.
fn scene(wrap: impl FnOnce(&mut T) -> NodeId) -> Pixmap {
    let mut t = T::new();
    t.rect(None, Rect::new(10.0, 0.0, 10.0, 40.0), BLACK);
    let parent = wrap(&mut t);
    let pane = t.add(Some(parent), |id| {
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 40.0));
        n.fills = vec![Paint::solid(Color::TRANSPARENT)];
        n.effects = vec![Effect::BackgroundBlur { radius: 8.0, visible: true }];
        n
    });
    let _ = pane;
    t.render(60, 40)
}

use designos_render::tiny_skia::Pixmap;

#[test]
fn background_blur_at_top_level_blurs_page_content() {
    let pm = scene(|t| t.page);
    let c = px(&pm, 15, 20);
    assert!(c[0] > 40 && c[0] < 200, "blurred stripe centre: {c:?}");
    assert_eq!(px(&pm, 55, 20), WHITE);
}

#[test]
fn background_blur_inside_opacity_group_sees_parent_canvas() {
    let pm = scene(|t| {
        let g = t.add(None, |id| Node::new(id, NodeKind::Group));
        t.node(g).opacity = 0.9;
        g
    });
    let c = px(&pm, 15, 20);
    // without a backdrop the group layer is empty and the stripe stays solid black
    assert!(c[0] > 40, "stripe must be blurred inside the group: {c:?}");
    assert!(c[0] < 215, "but still visible: {c:?}");
    assert_eq!(px(&pm, 55, 20), WHITE);
}

#[test]
fn background_blur_in_nested_groups_and_sees_layer_siblings() {
    let mut t = T::new();
    let outer = t.add(None, |id| Node::new(id, NodeKind::Group));
    t.node(outer).opacity = 0.8;
    // the stripe lives in the same group, below the blurred pane
    t.rect(Some(outer), Rect::new(10.0, 0.0, 10.0, 40.0), BLACK);
    let inner = t.add(Some(outer), |id| Node::new(id, NodeKind::Group));
    t.node(inner).opacity = 0.9;
    t.add(Some(inner), |id| {
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 40.0));
        n.fills = vec![Paint::solid(Color::TRANSPARENT)];
        n.effects = vec![Effect::BackgroundBlur { radius: 8.0, visible: true }];
        n
    });
    let pm = t.render(60, 40);
    let c = px(&pm, 15, 20);
    assert!(c[0] > 60, "stripe blurred through two layers: {c:?}");
    assert!(c[0] < 215, "{c:?}");
}

#[test]
fn background_blur_on_translucent_node_itself() {
    // the blurred node has its own opacity (so it is drawn in a layer)
    let mut t = T::new();
    t.rect(None, Rect::new(10.0, 0.0, 10.0, 40.0), BLACK);
    t.add(None, |id| {
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 40.0));
        n.fills = vec![Paint::solid(Color::rgba(255, 255, 255, 40))];
        n.opacity = 0.8;
        n.effects = vec![Effect::BackgroundBlur { radius: 8.0, visible: true }];
        n
    });
    let pm = t.render(60, 40);
    let c = px(&pm, 15, 20);
    assert!(c[0] > 60, "{c:?}");
}

#[test]
fn background_blur_without_a_visible_fill_leaves_backdrop_sharp() {
    let mut t = T::new();
    t.rect(None, Rect::new(10.0, 0.0, 10.0, 40.0), BLACK);
    let pane = t.add(None, |id| {
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 40.0));
        n.fills.clear();
        n.effects = vec![Effect::BackgroundBlur { radius: 8.0, visible: true }];
        n
    });
    let pm = t.render(60, 40);
    assert_eq!(px(&pm, 15, 20), [0, 0, 0, 255], "a fill-less backdrop blur must not alter the backdrop");
    assert_eq!(px(&pm, 25, 20), WHITE, "the area outside the stripe remains unchanged");

    t.node(pane).fills = vec![Paint { opacity: 0.0, ..Paint::solid(Color::WHITE) }];
    let pm_zero_opacity_fill = t.render(60, 40);
    let c_zero = px(&pm_zero_opacity_fill, 15, 20);
    assert!(c_zero[0] > 40 && c_zero[0] < 200, "a visible fill with zero paint opacity still blurs the backdrop: {c_zero:?}");

    t.node(pane).fills.clear();
    t.node(pane).stroke.paints = vec![Paint::solid(BLACK)];
    t.node(pane).stroke.weight = 2.0;
    let pm_stroke_only = t.render(60, 40);
    let c_stroke = px(&pm_stroke_only, 15, 20);
    assert!(c_stroke[0] > 40 && c_stroke[0] < 200, "a stroke-only layer still blurs the backdrop: {c_stroke:?}");
}

/// White page, black stripe at x 10..20, and a group without any paint of its own that holds a
/// 40x40 rectangle filled white at 0.4 opacity. `blur` puts a background blur on the group.
fn paintless_group_over_stripe(blur: bool) -> Pixmap {
    let mut t = T::new();
    t.rect(None, Rect::new(10.0, 0.0, 10.0, 40.0), BLACK);
    let group = t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Group);
        n.size = Vec2::new(40.0, 40.0);
        if blur {
            n.effects = vec![Effect::BackgroundBlur { radius: 8.0, visible: true }];
        }
        n
    });
    t.add(Some(group), |id| {
        let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 40.0));
        n.fills = vec![Paint { opacity: 0.4, ..Paint::solid(Color::WHITE) }];
        n
    });
    t.render(60, 40)
}

#[test]
fn background_blur_on_a_paintless_group_blurs_through_its_children() {
    let sharp = px(&paintless_group_over_stripe(false), 15, 20);
    let pm = paintless_group_over_stripe(true);
    let blurred = px(&pm, 15, 20);
    assert!(blurred[0] >= sharp[0] + 8, "the stripe under the child must be blurred: sharp {sharp:?}, with blur {blurred:?}");
    assert_eq!(blurred[3], 255, "the translucent child over an opaque page must remain opaque: {blurred:?}");
    assert_eq!(px(&pm, 55, 20), WHITE, "outside the group the page is untouched");
}

/// White page, black stripe at x 10..20, and a text node with no fill on its base style. The
/// only fill sits on the run covering the whole string. `blur` puts a background blur on it.
fn text_with_run_fill_over_stripe(run_fill: Paint, blur: bool) -> Pixmap {
    let mut t = T::new();
    t.rect(None, Rect::new(10.0, 0.0, 10.0, 40.0), BLACK);
    t.add(None, |id| {
        let base = TextStyle { font_size: 40.0, font_weight: 800, fills: Vec::new(), ..Default::default() };
        let mut n = Node::text(id, Rect::new(0.0, 0.0, 60.0, 40.0), "MM", base.clone());
        let NodeKind::Text(tp) = &mut n.kind else { unreachable!() };
        tp.runs = vec![TextRun { start: 0, end: 2, style: TextStyle { fills: vec![run_fill], ..base } }];
        if blur {
            n.effects = vec![Effect::BackgroundBlur { radius: 8.0, visible: true }];
        }
        n
    });
    t.render(60, 40)
}

/// The brightest red channel on the stripe columns, which are black unless something paints over them.
fn brightest_on_stripe(pm: &Pixmap) -> u8 {
    (10..20).flat_map(|x| (0..40).map(move |y| (x, y))).map(|(x, y)| px(pm, x, y)[0]).max().unwrap()
}

#[test]
fn background_blur_on_text_counts_a_visible_run_fill() {
    let invisible_ink = Paint { opacity: 0.0, ..Paint::solid(Color::WHITE) };
    assert_eq!(brightest_on_stripe(&text_with_run_fill_over_stripe(invisible_ink.clone(), false)), 0, "fixture: nothing paints on the stripe");
    let blurred = brightest_on_stripe(&text_with_run_fill_over_stripe(invisible_ink, true));
    assert!(blurred > 40, "the stripe under the glyphs must be blurred: brightest {blurred}");

    let hidden = Paint { visible: false, ..Paint::solid(Color::WHITE) };
    assert_eq!(
        brightest_on_stripe(&text_with_run_fill_over_stripe(hidden, true)),
        0,
        "a hidden run fill leaves the text with no paint: the stripe stays sharp"
    );
}

#[test]
fn background_blur_descendant_uses_resolved_radius_inside_opacity_group() {
    for masked in [false, true] {
        let mut t = T::new();
        t.rect(None, Rect::new(10.0, 0.0, 10.0, 40.0), BLACK);
        let group = t.add(None, |id| {
            let mut n = Node::new(id, NodeKind::Group);
            n.size = Vec2::new(40.0, 40.0);
            n.opacity = if masked { 1.0 } else { 0.9 };
            n
        });
        if masked {
            t.add(Some(group), |id| {
                let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 40.0));
                n.fills = vec![Paint::solid(Color::WHITE)];
                n.is_mask = true;
                n
            });
        }
        let pane = t.add(Some(group), |id| {
            let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 40.0));
            n.fills = vec![Paint::solid(Color::TRANSPARENT)];
            n.effects = vec![Effect::BackgroundBlur { radius: 0.0, visible: true }];
            n.visible = false;
            n.variables.bindings.insert("effects/0/radius".into(), "blur-radius".into());
            n.variables.bindings.insert("visible".into(), "blur-visible".into());
            n
        });
        t.doc.variable_collections.insert(
            "theme".into(),
            VariableCollection {
                id: "theme".into(),
                name: "Theme".into(),
                modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
                default_mode: "light".into(),
                variables: vec!["blur-radius".into(), "blur-visible".into()],
                key: None,
                source_library_key: None,
                extensions: Default::default(),
            },
        );
        t.doc.variables.insert(
            "blur-visible".into(),
            Variable {
                id: "blur-visible".into(),
                name: "blur-visible".into(),
                collection: "theme".into(),
                kind: VariableKind::Boolean,
                values: [("light".into(), VariableValue::Boolean(false)), ("dark".into(), VariableValue::Boolean(true))].into_iter().collect(),
                description: String::new(),
                scopes: vec![],
                hidden_from_publishing: false,
                key: None,
                extensions: Default::default(),
                code_syntax: Default::default(),
                mode_extensions: Default::default(),
            },
        );
        t.doc.variables.insert(
            "blur-radius".into(),
            Variable {
                id: "blur-radius".into(),
                name: "blur-radius".into(),
                collection: "theme".into(),
                kind: VariableKind::Number,
                values: [("light".into(), VariableValue::Number(0.0)), ("dark".into(), VariableValue::Number(8.0))].into_iter().collect(),
                description: String::new(),
                scopes: vec![],
                hidden_from_publishing: false,
                key: None,
                extensions: Default::default(),
                code_syntax: Default::default(),
                mode_extensions: Default::default(),
            },
        );
        t.node(group).variables.modes.insert("theme".into(), "dark".into());

        t.node(group).variables.modes.insert("theme".into(), "light".into());
        let light_actual = t.render(60, 40);
        let light_resolved = designos_doc::apply_bound_variables(&t.doc);
        let mut light_expected = Pixmap::new(60, 40).unwrap();
        designos_render::render_page(&light_resolved, light_resolved.pages[0], &designos_render::Viewport::default(), &mut light_expected);
        assert_eq!(px(&light_actual, 15, 20), px(&light_expected, 15, 20), "light mode keeps the stored hidden pane hidden");
        assert_eq!(px(&light_actual, 15, 20), [0, 0, 0, 255]);

        t.node(group).variables.modes.insert("theme".into(), "dark".into());

        let actual = t.render(60, 40);
        let resolved = designos_doc::apply_bound_variables(&t.doc);
        let mut expected = Pixmap::new(60, 40).unwrap();
        designos_render::render_page(&resolved, resolved.pages[0], &designos_render::Viewport::default(), &mut expected);
        if let Ok(dir) = std::env::var("DESIGNOS_BLUR_EVIDENCE_DIR") {
            std::fs::create_dir_all(&dir).unwrap();
            actual.save_png(std::path::Path::new(&dir).join(format!("cpu-actual-masked-{masked}.png"))).unwrap();
            expected.save_png(std::path::Path::new(&dir).join(format!("cpu-resolved-expected-masked-{masked}.png"))).unwrap();
        }
        assert_eq!(px(&actual, 15, 20), px(&expected, 15, 20), "descendant background blur must use resolved radius");
        assert!(px(&expected, 15, 20)[0] > 40, "fixture must visibly blur the stripe");
        assert_eq!(t.node(pane).effects[0], Effect::BackgroundBlur { radius: 0.0, visible: true });
        assert!(!t.node(pane).visible);

        t.node(pane).variables.bindings.clear();
        let unbound = t.render(60, 40);
        assert_eq!(px(&unbound, 15, 20), [0, 0, 0, 255], "unbound hidden pane preserves stored visibility");
    }
}
