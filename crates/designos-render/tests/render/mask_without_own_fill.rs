//! A mask that is a frame (or instance) with children takes its alpha from the
//! fills of the frame plus its drawn subtree. Only a childless shape with no
//! visible fill stands in as an opaque stencil.

use designos_doc::*;
use designos_render::{render_page, tiny_skia::Pixmap, Viewport};

const WHITE: [u8; 4] = [255, 255, 255, 255];
const RED: Color = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
const BLUE: Color = Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };

struct T {
    doc: Document,
    page: NodeId,
}

impl T {
    fn new() -> Self {
        let mut doc = Document::new("mask without own fill");
        let page = doc.add_page("p");
        doc.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::WHITE };
        Self { doc, page }
    }

    fn add(&mut self, parent: Option<NodeId>, f: impl FnOnce(NodeId) -> Node) -> NodeId {
        let id = self.doc.alloc_id();
        self.doc.insert(parent.unwrap_or(self.page), f(id)).unwrap()
    }

    fn rect(&mut self, parent: Option<NodeId>, r: Rect, c: Color) -> NodeId {
        self.add(parent, |id| {
            let mut n = Node::rectangle(id, r);
            n.fills = vec![Paint::solid(c)];
            n
        })
    }

    /// A mask frame with no fills of its own.
    fn mask_frame(&mut self, r: Rect) -> NodeId {
        self.add(None, |id| {
            let mut n = Node::frame(id, r);
            n.fills.clear();
            n.is_mask = true;
            n
        })
    }

    fn render(&self, size: u32) -> Pixmap {
        let mut pm = Pixmap::new(size, size).unwrap();
        render_page(&self.doc, self.page, &Viewport::default(), &mut pm);
        pm
    }
}

fn px(pm: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let p = pm.pixel(x, y).unwrap();
    [p.red(), p.green(), p.blue(), p.alpha()]
}

#[test]
fn frame_mask_without_fill_uses_the_alpha_of_its_children() {
    let mut t = T::new();
    let m = t.mask_frame(Rect::new(0.0, 0.0, 80.0, 80.0));
    // the stencil child covers only the top-left quarter of the mask frame
    t.rect(Some(m), Rect::new(0.0, 0.0, 40.0, 40.0), BLUE);
    t.rect(None, Rect::new(0.0, 0.0, 80.0, 80.0), RED);
    let pm = t.render(80);
    assert_eq!(px(&pm, 20, 20), [255, 0, 0, 255], "inside the child of the mask");
    assert_eq!(px(&pm, 60, 60), WHITE, "inside the mask frame but outside its child: not an opaque box");
    assert_eq!(px(&pm, 60, 20), WHITE, "right of the child");
}

#[test]
fn frame_mask_without_fill_keeps_partial_alpha_of_children() {
    let mut t = T::new();
    let m = t.mask_frame(Rect::new(0.0, 0.0, 80.0, 80.0));
    let half = Color { a: 0.5, ..BLUE };
    t.rect(Some(m), Rect::new(0.0, 0.0, 80.0, 80.0), half);
    t.rect(None, Rect::new(0.0, 0.0, 80.0, 80.0), RED);
    let pm = t.render(80);
    let p = px(&pm, 40, 40);
    assert!((p[1] as i32 - 128).abs() <= 2 && (p[2] as i32 - 128).abs() <= 2 && p[0] == 255, "red at half alpha over white, got {p:?}");
}

#[test]
fn frame_mask_with_children_still_adds_its_own_fill() {
    let mut t = T::new();
    let m = t.mask_frame(Rect::new(0.0, 0.0, 80.0, 80.0));
    t.node(m).fills = vec![Paint::solid(BLUE)];
    t.rect(Some(m), Rect::new(0.0, 0.0, 10.0, 10.0), BLUE);
    t.rect(None, Rect::new(0.0, 0.0, 80.0, 80.0), RED);
    let pm = t.render(80);
    assert_eq!(px(&pm, 60, 60), [255, 0, 0, 255], "the frame's own fill is part of the mask");
}

#[test]
fn childless_shape_without_fill_still_masks_as_an_opaque_stencil() {
    let mut t = T::new();
    let m = t.add(None, |id| Node::ellipse(id, Rect::new(0.0, 0.0, 80.0, 80.0)));
    t.node(m).fills.clear();
    t.node(m).is_mask = true;
    t.rect(None, Rect::new(0.0, 0.0, 80.0, 80.0), RED);
    let pm = t.render(80);
    assert_eq!(px(&pm, 40, 40), [255, 0, 0, 255], "inside the stencil");
    assert_eq!(px(&pm, 2, 2), WHITE, "outside the ellipse");
}

#[test]
fn childless_frame_without_fill_masks_as_an_opaque_box() {
    let mut t = T::new();
    t.mask_frame(Rect::new(0.0, 0.0, 40.0, 40.0));
    t.rect(None, Rect::new(0.0, 0.0, 80.0, 80.0), RED);
    let pm = t.render(80);
    assert_eq!(px(&pm, 20, 20), [255, 0, 0, 255], "inside the box");
    assert_eq!(px(&pm, 60, 60), WHITE, "outside the box");
}

#[test]
fn mask_frame_box_matches_alpha_only_from_children_rule() {
    let mut frame = Node::frame(NodeId(1), Rect::new(0.0, 0.0, 40.0, 40.0));
    frame.fills.clear();
    assert!(designos_render::mask_frame_box(&frame).is_none(), "childless frame gets its own fill from mask_stencil");

    frame.children = vec![NodeId(2)];
    let stand_in = designos_render::mask_frame_box(&frame).expect("fill-less frame with children has a stand-in box");
    assert!(stand_in.children.is_empty());
    assert_eq!(stand_in.fills, vec![Paint::solid(Color::BLACK)]);

    frame.fills = vec![Paint::solid(BLUE)];
    assert!(designos_render::mask_frame_box(&frame).is_none(), "frame with a visible fill already has its own alpha");

    let mut group = Node::new(NodeId(3), NodeKind::Group);
    group.children = vec![NodeId(4)];
    assert!(designos_render::mask_frame_box(&group).is_none(), "group has no box of its own");
}

impl T {
    fn node(&mut self, id: NodeId) -> &mut Node {
        self.doc.get_mut(id).unwrap()
    }
}

#[test]
fn reverse_z_auto_layout_draws_masked_background_pattern_behind_earlier_siblings() {
    let mut t = T::new();
    let hero = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(0.0, 0.0, 80.0, 80.0));
        if let NodeKind::Frame(fp) = &mut n.kind {
            fp.auto_layout = Some(AutoLayout { direction: LayoutDirection::Vertical, reverse_z: true, ..Default::default() });
        }
        n
    });
    // Child 0: heading box at (20, 20, 40, 40) in solid BLUE
    t.rect(Some(hero), Rect::new(20.0, 20.0, 40.0, 40.0), BLUE);
    // Child 1: absolute background pattern with a fill-less 50%-alpha mask and a RED pattern square
    let pat = t.add(Some(hero), |id| {
        let mut n = Node::frame(id, Rect::new(0.0, 0.0, 80.0, 80.0));
        n.fills.clear();
        n.layout_child.absolute = true;
        n
    });
    let mask = t.add(Some(pat), |id| {
        let mut n = Node::frame(id, Rect::new(0.0, 0.0, 80.0, 80.0));
        n.fills.clear();
        n.is_mask = true;
        n
    });
    t.rect(Some(mask), Rect::new(0.0, 0.0, 80.0, 80.0), Color { a: 0.5, ..BLUE });
    t.rect(Some(pat), Rect::new(0.0, 0.0, 80.0, 80.0), RED);

    let pm = t.render(80);
    // Inside the heading box (30, 30): solid BLUE, not tinted by the faded RED pattern
    assert_eq!(px(&pm, 30, 30), [0, 0, 255, 255], "heading stays solid over the masked background pattern");
    // Outside the heading box (10, 10): faded RED over white stays visible
    let bg = px(&pm, 10, 10);
    assert!(bg[0] == 255 && (bg[1] as i32 - 128).abs() <= 2 && (bg[2] as i32 - 128).abs() <= 2, "faded pattern remains visible behind heading: {bg:?}");
}

#[test]
fn vector_mask_with_zero_opacity_fill_still_clips_to_geometry_coverage() {
    let build = |mask_type: MaskType| {
        let mut t = T::new();
        let m = t.add(None, |id| {
            let mut n = Node::rectangle(id, Rect::new(10.0, 10.0, 40.0, 40.0));
            n.is_mask = true;
            n.mask_type = mask_type;
            n.opacity = 0.25;
            let mut transparent_fill = Paint::solid(Color::TRANSPARENT);
            transparent_fill.opacity = 0.0;
            n.fills = vec![transparent_fill];
            n.effects = vec![Effect::DropShadow {
                color: Color::BLACK,
                offset: Vec2::new(10.0, 10.0),
                radius: 8.0,
                spread: 4.0,
                visible: true,
                blend: BlendMode::Normal,
                show_behind: true,
            }];
            n
        });
        let _ = m;
        t.rect(None, Rect::new(0.0, 0.0, 80.0, 80.0), RED);
        t.render(80)
    };

    // Alpha mask with 0%-opacity fill clips away everything (white background everywhere)
    let alpha_pm = build(MaskType::Alpha);
    assert_eq!(px(&alpha_pm, 30, 30), WHITE, "Alpha mask with 0%-opacity fill hides content");

    // Vector mask with 0%-opacity fill and effects ignores both and clips strictly to geometry coverage
    let vec_pm = build(MaskType::Vector);
    assert_eq!(px(&vec_pm, 30, 30), [255, 0, 0, 255], "Vector mask with 0%-opacity fill keeps full content inside geometry");
    assert_eq!(px(&vec_pm, 5, 5), WHITE, "outside geometry top-left");
    assert_eq!(px(&vec_pm, 55, 55), WHITE, "outside geometry where drop shadow would land if not ignored");
}

#[test]
fn luminance_mask_over_white_to_black_ramp_keeps_white_hides_black_and_halves_mid_grey() {
    let mut doc = Document::new("luminance-mask");
    let page = doc.add_page("p");
    // Transparent page background so pixel alpha directly measures the masked coverage
    doc.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::TRANSPARENT };

    let mask_id = doc.alloc_id();
    let mut mask = Node::frame(mask_id, Rect::new(0.0, 0.0, 100.0, 60.0));
    mask.fills.clear();
    mask.is_mask = true;
    mask.mask_type = MaskType::Luminance;
    doc.insert(page, mask).unwrap();

    // Row 0 (y = 0..30): white-to-black linear gradient ramp across x = 0..100
    let ramp_id = doc.alloc_id();
    let mut ramp = Node::rectangle(ramp_id, Rect::new(0.0, 0.0, 100.0, 30.0));
    ramp.fills = vec![Paint {
        visible: true,
        opacity: 1.0,
        blend: BlendMode::Normal,
        kind: PaintKind::LinearGradient {
            handles: [Vec2::new(0.0, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.0, 1.0)],
            stops: vec![GradientStop { pos: 0.0, color: Color::WHITE }, GradientStop { pos: 1.0, color: Color::BLACK }],
        },
    }];
    doc.insert(mask_id, ramp).unwrap();

    // Row 1 (y = 30..60): explicit swatches for White (x=0..25), 50% Grey (x=25..50), Black (x=50..75), Pure Green (x=75..100)
    for (x, col) in [(0.0, Color::WHITE), (25.0, Color::rgb(128, 128, 128)), (50.0, Color::BLACK), (75.0, Color { r: 0.0, g: 1.0, b: 0.0, a: 1.0 })] {
        let sid = doc.alloc_id();
        let mut sw = Node::rectangle(sid, Rect::new(x, 30.0, 25.0, 30.0));
        sw.fills = vec![Paint::solid(col)];
        doc.insert(mask_id, sw).unwrap();
    }

    // Masked sibling: solid RED covering the entire 100x60 region
    let target_id = doc.alloc_id();
    let mut target = Node::rectangle(target_id, Rect::new(0.0, 0.0, 100.0, 60.0));
    target.fills = vec![Paint::solid(RED)];
    doc.insert(page, target).unwrap();

    let mut pm = Pixmap::new(100, 60).unwrap();
    render_page(&doc, page, &Viewport::default(), &mut pm);

    // 1. Ramp probes (y = 15): near x = 0 (white) keeps content, x = 50 (~50% grey) keeps ~half, near x = 99 (black) hides
    let ramp_white = px(&pm, 0, 15);
    let ramp_mid = px(&pm, 50, 15);
    let ramp_black = px(&pm, 99, 15);
    assert!(ramp_white[3] >= 250, "ramp white end keeps content: {ramp_white:?}");
    assert!((ramp_mid[3] as i32 - 128).abs() <= 6, "ramp midpoint keeps ~half content: {ramp_mid:?}");
    assert!(ramp_black[3] <= 5, "ramp black end hides content: {ramp_black:?}");

    // 2. Exact swatch probes (y = 45):
    assert_eq!(px(&pm, 12, 45), [255, 0, 0, 255], "white swatch keeps 100% content");
    let mid_grey = px(&pm, 37, 45);
    assert!((mid_grey[3] as i32 - 128).abs() <= 1, "50% grey swatch keeps ~half (128/255): {mid_grey:?}");
    assert_eq!(px(&pm, 62, 45), [0, 0, 0, 0], "black swatch hides content completely");
    let green = px(&pm, 87, 45);
    let expected_green_alpha = (designos_render::LUMINANCE_MASK_COEFFS[1] * 255.0).round() as i32;
    assert!((green[3] as i32 - expected_green_alpha).abs() <= 1, "pure green uses Rec. 709 G weight (0.7152): {green:?}");
}

#[test]
fn vector_mask_uses_own_outline_for_shape_frame_group_and_text() {
    // 1. Frame mask with own fill and a small 40x40 child: Vector coverage is the frame's 100x100 box.
    {
        let mut t = T::new();
        let mask_frame = t.add(None, |id| {
            let mut n = Node::frame(id, Rect::new(0.0, 0.0, 100.0, 100.0));
            n.is_mask = true;
            n.mask_type = MaskType::Vector;
            n.fills = vec![Paint::solid(Color::WHITE)];
            n
        });
        let stand_in = designos_render::mask_frame_box(t.doc.get(mask_frame).unwrap());
        t.rect(Some(mask_frame), Rect::new(0.0, 0.0, 40.0, 40.0), Color::WHITE);
        assert!(
            designos_render::mask_frame_box(t.doc.get(mask_frame).unwrap()).is_some() && stand_in.is_none(),
            "vector frame with children returns its own box from mask_frame_box even when it has a visible fill"
        );
        t.rect(None, Rect::new(0.0, 0.0, 100.0, 100.0), RED);
        let pm = t.render(100);
        assert_eq!(px(&pm, 20, 20), [255, 0, 0, 255], "inside child and frame");
        assert_eq!(px(&pm, 80, 80), [255, 0, 0, 255], "inside frame box outside child");
    }

    // 2. Rounded frame mask with no own fill and an unclipped 100x100 child at 50% opacity:
    //    Vector coverage is the frame's rounded box at full opacity, ignoring the child.
    {
        let mut t = T::new();
        let mask_frame = t.add(None, |id| {
            let mut n = Node::frame(id, Rect::new(0.0, 0.0, 100.0, 100.0));
            if let NodeKind::Frame(fp) = &mut n.kind {
                fp.clips_content = false;
            }
            n.fills.clear();
            n.corner_radii = [40.0; 4];
            n.is_mask = true;
            n.mask_type = MaskType::Vector;
            n
        });
        t.add(Some(mask_frame), |id| {
            let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 100.0, 100.0));
            n.opacity = 0.5;
            n.fills = vec![Paint::solid(Color::BLACK)];
            n
        });
        t.rect(None, Rect::new(0.0, 0.0, 100.0, 100.0), RED);
        let pm = t.render(100);
        assert_eq!(px(&pm, 50, 50), [255, 0, 0, 255], "center of rounded vector frame is fully opaque");
        assert_eq!(px(&pm, 2, 2), WHITE, "outside rounded corner of vector frame is clipped despite unclipped child");
    }

    // 3. Group mask: Vector coverage is the union of its descendants' outlines, ignoring their fills and opacity.
    {
        let mut t = T::new();
        let mask_group = t.add(None, |id| {
            let mut n = Node::new(id, NodeKind::Group);
            n.size = Vec2::new(100.0, 100.0);
            n.fills = vec![Paint::solid(Color::WHITE)];
            n.is_mask = true;
            n.mask_type = MaskType::Vector;
            n
        });
        t.add(Some(mask_group), |id| {
            let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 40.0));
            n.opacity = 0.25;
            let mut p = Paint::solid(Color::TRANSPARENT);
            p.opacity = 0.0;
            n.fills = vec![p];
            n
        });
        let nested_group = t.add(Some(mask_group), |id| {
            let mut n = Node::new(id, NodeKind::Group);
            n.size = Vec2::new(40.0, 40.0);
            n
        });
        t.add(Some(nested_group), |id| {
            let mut n = Node::rectangle(id, Rect::new(60.0, 60.0, 30.0, 30.0));
            n.fills.clear();
            n
        });
        let hidden_nested_group = t.add(Some(mask_group), |id| {
            let mut n = Node::new(id, NodeKind::Group);
            n.size = Vec2::new(40.0, 40.0);
            n.visible = false;
            n
        });
        t.add(Some(hidden_nested_group), |id| {
            let mut n = Node::rectangle(id, Rect::new(0.0, 60.0, 30.0, 30.0));
            n.fills = vec![Paint::solid(Color::BLACK)];
            n
        });
        let group_ref = t.doc.get(mask_group).unwrap();
        assert!(designos_render::mask_stencil(group_ref).fills.is_empty(), "vector group stencil clears group fills");
        assert!(designos_render::mask_frame_box(group_ref).is_none(), "vector group has no box of its own");

        t.rect(None, Rect::new(0.0, 0.0, 100.0, 100.0), RED);
        let pm = t.render(100);
        assert_eq!(px(&pm, 20, 20), [255, 0, 0, 255], "first group child outline is covered at full opacity");
        assert_eq!(px(&pm, 75, 75), [255, 0, 0, 255], "nested group child outline is covered at full opacity");
        assert_eq!(px(&pm, 15, 75), WHITE, "child inside hidden nested group is not covered");
        assert_eq!(px(&pm, 50, 50), WHITE, "gap between group descendants is not covered");
    }

    // 4. Text mask with 0%-opacity text style fill: Vector coverage uses glyph outlines.
    {
        let mut t = T::new();
        t.add(None, |id| {
            let mut n = Node::text(id, Rect::new(4.0, 4.0, 90.0, 60.0), "H", designos_doc::TextStyle::default());
            n.is_mask = true;
            n.mask_type = MaskType::Vector;
            if let NodeKind::Text(tp) = &mut n.kind {
                tp.style.font_size = 48.0;
                let mut p = Paint::solid(Color::BLACK);
                p.opacity = 0.0;
                tp.style.fills = vec![p];
            }
            n
        });
        t.rect(None, Rect::new(0.0, 0.0, 100.0, 100.0), RED);
        let pm = t.render(100);
        let mut glyph_hits = 0usize;
        for y in 0..60 {
            for x in 0..60 {
                if px(&pm, x, y) == [255, 0, 0, 255] {
                    glyph_hits += 1;
                }
            }
        }
        assert!(glyph_hits > 20, "vector text mask with transparent text fill must reveal glyph pixels, got {glyph_hits}");
        assert_eq!(px(&pm, 95, 95), WHITE, "outside text bounds remains unmasked background");
    }

    // 5. BooleanOp (Union) mask with 0%-opacity fills on both the boolean and its two operands:
    //    Vector coverage keeps the boolean operands and covers their union at full opacity.
    {
        let mut t = T::new();
        let bo = t.add(None, |id| {
            let mut n = Node::new(id, NodeKind::BooleanOp(designos_doc::BooleanOp::Union));
            n.size = Vec2::new(100.0, 100.0);
            let mut p = Paint::solid(Color::BLACK);
            p.opacity = 0.0;
            n.fills = vec![p];
            n.is_mask = true;
            n.mask_type = MaskType::Vector;
            n
        });
        t.add(Some(bo), |id| {
            let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 40.0));
            n.fills.clear();
            n
        });
        t.add(Some(bo), |id| {
            let mut n = Node::rectangle(id, Rect::new(30.0, 30.0, 40.0, 40.0));
            n.fills.clear();
            n
        });
        t.rect(None, Rect::new(0.0, 0.0, 100.0, 100.0), RED);
        let pm = t.render(100);
        assert_eq!(px(&pm, 15, 15), [255, 0, 0, 255], "first boolean operand covered at full opacity");
        assert_eq!(px(&pm, 55, 55), [255, 0, 0, 255], "second boolean operand covered at full opacity");
        assert_eq!(px(&pm, 85, 15), WHITE, "outside boolean union is clipped");
    }
}

#[test]
fn luminance_mask_rgb_channels_and_shader_coefficients_match_rec709() {
    for (col, expected_alpha, label) in [
        (Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 }, 54i32, "pure red"),
        (Color { r: 0.0, g: 1.0, b: 0.0, a: 1.0 }, 182i32, "pure green"),
        (Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 }, 18i32, "pure blue"),
    ] {
        let mut doc = Document::new(label);
        let page = doc.add_page("p");
        doc.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::TRANSPARENT };

        let mid = doc.alloc_id();
        let mut m = Node::rectangle(mid, Rect::new(0.0, 0.0, 32.0, 32.0));
        m.is_mask = true;
        m.mask_type = MaskType::Luminance;
        m.fills = vec![Paint::solid(col)];
        doc.insert(page, m).unwrap();

        let tid = doc.alloc_id();
        let mut target = Node::rectangle(tid, Rect::new(0.0, 0.0, 32.0, 32.0));
        target.fills = vec![Paint::solid(Color::WHITE)];
        doc.insert(page, target).unwrap();

        let mut pm = Pixmap::new(32, 32).unwrap();
        render_page(&doc, page, &Viewport::default(), &mut pm);
        let a = px(&pm, 16, 16)[3] as i32;
        assert!((a - expected_alpha).abs() <= 1, "{label}: expected alpha {expected_alpha}, got {a}");
    }
}
