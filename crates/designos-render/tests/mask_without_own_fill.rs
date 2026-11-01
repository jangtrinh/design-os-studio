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
