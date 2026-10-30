use designos_doc::*;
use designos_render::{render_node, render_page, tiny_skia::Pixmap, Viewport};

struct T {
    doc: Document,
    page: NodeId,
}

impl T {
    fn new() -> Self {
        let mut doc = Document::new("t");
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
    fn render(&self, w: u32, h: u32) -> Pixmap {
        self.render_vp(w, h, &Viewport::default())
    }
    fn render_vp(&self, w: u32, h: u32, vp: &Viewport) -> Pixmap {
        let mut pm = Pixmap::new(w, h).unwrap();
        render_page(&self.doc, self.page, vp, &mut pm);
        pm
    }
    fn node(&mut self, id: NodeId) -> &mut Node {
        self.doc.get_mut(id).unwrap()
    }
}

fn px(pm: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let p = pm.pixel(x, y).unwrap();
    [p.red(), p.green(), p.blue(), p.alpha()]
}

fn near(a: [u8; 4], b: [u8; 4], tol: i32) -> bool {
    a.iter().zip(b.iter()).all(|(x, y)| (*x as i32 - *y as i32).abs() <= tol)
}

const WHITE: [u8; 4] = [255, 255, 255, 255];
const RED: Color = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
const BLUE: Color = Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };

#[test]
fn page_background_and_rect_fill() {
    let mut t = T::new();
    t.rect(None, Rect::new(10.0, 10.0, 20.0, 20.0), RED);
    let pm = t.render(50, 50);
    assert_eq!(px(&pm, 5, 5), WHITE);
    assert_eq!(px(&pm, 20, 20), [255, 0, 0, 255]);
    assert_eq!(px(&pm, 35, 20), WHITE);
}

#[test]
fn default_page_background() {
    let mut doc = Document::new("t");
    let page = doc.add_page("p");
    let mut pm = Pixmap::new(4, 4).unwrap();
    render_page(&doc, page, &Viewport::default(), &mut pm);
    assert_eq!(px(&pm, 1, 1), [0x5A, 0x5A, 0x5A, 255]);
}

#[test]
fn corner_radius_cuts_corner() {
    let mut t = T::new();
    let r = t.rect(None, Rect::new(10.0, 10.0, 40.0, 40.0), RED);
    t.node(r).corner_radii = [16.0, 0.0, 0.0, 0.0];
    let pm = t.render(60, 60);
    assert_eq!(px(&pm, 10, 10), WHITE, "rounded corner transparent");
    assert_eq!(px(&pm, 49, 10), [255, 0, 0, 255], "square corner filled");
    assert_eq!(px(&pm, 30, 30), [255, 0, 0, 255]);
}

#[test]
fn ellipse_and_arc() {
    let mut t = T::new();
    let e = t.add(None, |id| Node::ellipse(id, Rect::new(0.0, 0.0, 40.0, 40.0)));
    t.node(e).fills = vec![Paint::solid(RED)];
    let pm = t.render(40, 40);
    assert_eq!(px(&pm, 20, 20), [255, 0, 0, 255]);
    assert_eq!(px(&pm, 1, 1), WHITE);
    // half pie (right half): start 0 .. end pi rotates clockwise through the bottom
    if let NodeKind::Ellipse(a) = &mut t.node(e).kind {
        a.end = std::f32::consts::PI;
    }
    let pm = t.render(40, 40);
    assert_eq!(px(&pm, 20, 30), [255, 0, 0, 255], "bottom half filled");
    assert_eq!(px(&pm, 20, 8), WHITE, "top half empty");
    // donut
    if let NodeKind::Ellipse(a) = &mut t.node(e).kind {
        a.end = std::f32::consts::TAU;
        a.inner_radius = 0.5;
    }
    let pm = t.render(40, 40);
    assert_eq!(px(&pm, 20, 20), WHITE, "hole");
    assert_eq!(px(&pm, 20, 3), [255, 0, 0, 255], "ring");
}

#[test]
fn frame_clips_children() {
    let mut t = T::new();
    let f = t.add(None, |id| Node::frame(id, Rect::new(10.0, 10.0, 20.0, 20.0)));
    t.node(f).fills = vec![Paint::solid(BLUE)];
    if let NodeKind::Frame(p) = &mut t.node(f).kind {
        p.clips_content = true;
    }
    t.rect(Some(f), Rect::new(10.0, 10.0, 40.0, 40.0), RED);
    let pm = t.render(60, 60);
    assert_eq!(px(&pm, 15, 15), [0, 0, 255, 255]);
    assert_eq!(px(&pm, 25, 25), [255, 0, 0, 255]);
    assert_eq!(px(&pm, 35, 35), WHITE, "clipped");
    // without clipping the child overflows
    if let NodeKind::Frame(p) = &mut t.node(f).kind {
        p.clips_content = false;
    }
    let pm = t.render(60, 60);
    assert_eq!(px(&pm, 35, 35), [255, 0, 0, 255]);
}

#[test]
fn opacity_and_group_opacity() {
    let mut t = T::new();
    let r = t.rect(None, Rect::new(0.0, 0.0, 10.0, 10.0), RED);
    t.node(r).opacity = 0.5;
    let pm = t.render(10, 10);
    assert!(near(px(&pm, 5, 5), [255, 128, 128, 255], 2), "{:?}", px(&pm, 5, 5));

    // group opacity: overlapping children don't double-blend
    let mut t = T::new();
    let g = t.add(None, |id| Node::new(id, NodeKind::Group));
    t.rect(Some(g), Rect::new(0.0, 0.0, 10.0, 10.0), RED);
    t.rect(Some(g), Rect::new(0.0, 0.0, 10.0, 10.0), BLUE);
    t.node(g).opacity = 0.5;
    let pm = t.render(10, 10);
    assert!(near(px(&pm, 5, 5), [128, 128, 255, 255], 2), "{:?}", px(&pm, 5, 5));
}

#[test]
fn hidden_nodes_are_skipped() {
    let mut t = T::new();
    let r = t.rect(None, Rect::new(0.0, 0.0, 10.0, 10.0), RED);
    t.node(r).visible = false;
    assert_eq!(px(&t.render(10, 10), 5, 5), WHITE);
    t.node(r).visible = true;
    t.node(r).fills[0].visible = false;
    assert_eq!(px(&t.render(10, 10), 5, 5), WHITE);
}

#[test]
fn blend_multiply() {
    let mut t = T::new();
    t.rect(None, Rect::new(0.0, 0.0, 10.0, 10.0), Color::rgb(255, 128, 0));
    let r = t.rect(None, Rect::new(0.0, 0.0, 10.0, 10.0), Color::rgb(128, 128, 128));
    t.node(r).blend = BlendMode::Multiply;
    let pm = t.render(10, 10);
    assert!(near(px(&pm, 5, 5), [128, 64, 0, 255], 3), "{:?}", px(&pm, 5, 5));
}

#[test]
fn transforms_rotation_and_nesting() {
    let mut t = T::new();
    let g = t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Group);
        n.transform = Affine::translate(20.0, 0.0);
        n
    });
    t.rect(Some(g), Rect::new(0.0, 0.0, 10.0, 10.0), RED);
    let pm = t.render(40, 20);
    assert_eq!(px(&pm, 25, 5), [255, 0, 0, 255]);
    assert_eq!(px(&pm, 5, 5), WHITE);
    // 90 degree rotation about the origin: a 20x10 rect becomes 10 wide, 20 tall
    let mut t = T::new();
    let r = t.rect(None, Rect::new(0.0, 0.0, 20.0, 10.0), RED);
    t.node(r).transform = Affine::translate(30.0, 0.0) * Affine::rotate(std::f32::consts::FRAC_PI_2);
    let pm = t.render(40, 30);
    assert_eq!(px(&pm, 25, 15), [255, 0, 0, 255]);
    assert_eq!(px(&pm, 35, 15), WHITE);
}

#[test]
fn viewport_zoom_offset_dpr() {
    let mut t = T::new();
    t.rect(None, Rect::new(10.0, 10.0, 10.0, 10.0), RED);
    let vp = Viewport { offset: Vec2::new(5.0, 5.0), zoom: 2.0, dpr: 2.0 };
    let pm = t.render_vp(100, 100, &vp);
    // page (10..20) -> device ((10-5)*4 .. (20-5)*4) = 20..60
    assert_eq!(px(&pm, 19, 40), WHITE);
    assert_eq!(px(&pm, 21, 40), [255, 0, 0, 255]);
    assert_eq!(px(&pm, 59, 40), [255, 0, 0, 255]);
    assert_eq!(px(&pm, 61, 40), WHITE);
}

fn stop(pos: f32, c: Color) -> GradientStop {
    GradientStop { pos, color: c }
}

fn gradient_paint(kind: PaintKind) -> Paint {
    Paint { kind, opacity: 1.0, visible: true, blend: BlendMode::Normal }
}

#[test]
fn linear_gradient_endpoints() {
    let mut t = T::new();
    let r = t.rect(None, Rect::new(0.0, 0.0, 100.0, 20.0), RED);
    t.node(r).fills = vec![gradient_paint(PaintKind::LinearGradient {
        handles: [Vec2::new(0.0, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.0, 1.0)],
        stops: vec![stop(0.0, RED), stop(1.0, BLUE)],
    })];
    let pm = t.render(100, 20);
    assert!(near(px(&pm, 1, 10), [253, 0, 2, 255], 4), "{:?}", px(&pm, 1, 10));
    assert!(near(px(&pm, 98, 10), [3, 0, 252, 255], 6), "{:?}", px(&pm, 98, 10));
    assert!(near(px(&pm, 50, 10), [127, 0, 127, 255], 6), "{:?}", px(&pm, 50, 10));
}

#[test]
fn radial_angular_diamond_gradients_render() {
    for kind in 0..3 {
        let mut t = T::new();
        let r = t.rect(None, Rect::new(0.0, 0.0, 40.0, 40.0), RED);
        let handles = [Vec2::new(0.5, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.5, 1.0)];
        let stops = vec![stop(0.0, RED), stop(1.0, BLUE)];
        t.node(r).fills = vec![gradient_paint(match kind {
            0 => PaintKind::RadialGradient { handles, stops },
            1 => PaintKind::AngularGradient { handles, stops },
            _ => PaintKind::DiamondGradient { handles, stops },
        })];
        let pm = t.render(40, 40);
        let c = px(&pm, 20, 20);
        assert_eq!(c[3], 255);
        if kind != 1 {
            assert!(c[0] > 200 && c[2] < 60, "kind {kind} center {c:?}");
            let e = px(&pm, 38, 20);
            assert!(e[2] > 150, "kind {kind} edge {e:?}");
        } else {
            // angular: right of centre is t=0 (red), below is t=0.25
            assert!(px(&pm, 35, 21)[0] > 200);
            assert!(px(&pm, 20, 35)[2] > 40);
        }
    }
}

fn stroke_rect(t: &mut T, align: StrokeAlign) -> NodeId {
    let r = t.rect(None, Rect::new(20.0, 20.0, 40.0, 40.0), Color::TRANSPARENT);
    let n = t.node(r);
    n.fills.clear();
    n.stroke.paints = vec![Paint::solid(RED)];
    n.stroke.weight = 6.0;
    n.stroke.align = align;
    r
}

#[test]
fn stroke_alignment_extents() {
    // inside: covers x in 20..26
    let mut t = T::new();
    stroke_rect(&mut t, StrokeAlign::Inside);
    let pm = t.render(80, 80);
    assert_eq!(px(&pm, 22, 40), [255, 0, 0, 255]);
    assert_eq!(px(&pm, 17, 40), WHITE);
    assert_eq!(px(&pm, 30, 40), WHITE);
    // outside: covers 14..20
    let mut t = T::new();
    stroke_rect(&mut t, StrokeAlign::Outside);
    let pm = t.render(80, 80);
    assert_eq!(px(&pm, 17, 40), [255, 0, 0, 255]);
    assert_eq!(px(&pm, 22, 40), WHITE);
    // center: covers 17..23
    let mut t = T::new();
    stroke_rect(&mut t, StrokeAlign::Center);
    let pm = t.render(80, 80);
    assert_eq!(px(&pm, 18, 40), [255, 0, 0, 255]);
    assert_eq!(px(&pm, 22, 40), [255, 0, 0, 255]);
    assert_eq!(px(&pm, 15, 40), WHITE);
    assert_eq!(px(&pm, 25, 40), WHITE);
}

#[test]
fn per_side_stroke_weights() {
    let mut t = T::new();
    let r = stroke_rect(&mut t, StrokeAlign::Inside);
    t.node(r).stroke.side_weights = Some([0.0, 0.0, 8.0, 0.0]);
    let pm = t.render(80, 80);
    assert_eq!(px(&pm, 40, 56), [255, 0, 0, 255], "bottom");
    assert_eq!(px(&pm, 40, 22), WHITE, "top has none");
    assert_eq!(px(&pm, 22, 40), WHITE, "left has none");
}

#[test]
fn per_side_stroke_follows_rounded_corners() {
    // 160x156 rectangle, radius 12, side_weights [0, 1, 1, 1], Inside:
    // pixel on bottom-left arc at 45 degrees is painted, top edge row stays unpainted.
    let mut t = T::new();
    let r = t.rect(None, Rect::new(0.0, 0.0, 160.0, 156.0), Color::WHITE);
    let n = t.node(r);
    n.corner_radii = [12.0; 4];
    n.stroke.paints = vec![Paint::solid(RED)];
    n.stroke.weight = 1.0;
    n.stroke.side_weights = Some([0.0, 1.0, 1.0, 1.0]);
    n.stroke.align = StrokeAlign::Inside;
    let pm = t.render(160, 156);
    assert_eq!(px(&pm, 80, 0), WHITE, "top edge row stays unpainted when top weight is 0");
    let bl_arc = px(&pm, 4, 152);
    assert!(bl_arc[0] == 255 && bl_arc[1] < 200, "bottom-left corner arc at 45 degrees must be painted, got {bl_arc:?}");

    // Outside stroke on a rounded rectangle must not leave square bars outside the corner rounding.
    let mut t2 = T::new();
    let r2 = t2.rect(None, Rect::new(10.0, 10.0, 80.0, 80.0), Color::WHITE);
    let n2 = t2.node(r2);
    n2.corner_radii = [20.0; 4];
    n2.stroke.paints = vec![Paint::solid(RED)];
    n2.stroke.weight = 4.0;
    n2.stroke.side_weights = Some([0.0, 4.0, 4.0, 4.0]);
    n2.stroke.align = StrokeAlign::Outside;
    let pm2 = t2.render(100, 100);
    assert_eq!(px(&pm2, 10, 90), WHITE, "square corner outside the rounded arc must stay unpainted");
}

#[test]
fn dashed_line_and_caps() {
    let mut t = T::new();
    let l = t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Line);
        n.transform = Affine::translate(0.0, 10.0);
        n.size = Vec2::new(60.0, 0.0);
        n.stroke.paints = vec![Paint::solid(RED)];
        n.stroke.weight = 4.0;
        n.stroke.align = StrokeAlign::Center;
        n.stroke.dashes = vec![10.0, 10.0];
        n
    });
    let pm = t.render(60, 20);
    assert_eq!(px(&pm, 5, 10), [255, 0, 0, 255]);
    assert_eq!(px(&pm, 15, 10), WHITE);
    assert_eq!(px(&pm, 25, 10), [255, 0, 0, 255]);
    let _ = l;
}

#[test]
fn polygon_star_vector() {
    let mut t = T::new();
    let p = t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Polygon { count: 3 });
        n.size = Vec2::new(40.0, 40.0);
        n.fills = vec![Paint::solid(RED)];
        n
    });
    let pm = t.render(40, 40);
    assert_eq!(px(&pm, 20, 30), [255, 0, 0, 255]);
    assert_eq!(px(&pm, 2, 5), WHITE);
    assert_eq!(px(&pm, 20, 38), [255, 0, 0, 255], "triangle base reaches the box bottom");
    t.node(p).kind = NodeKind::Star { count: 5, inner_ratio: 0.4 };
    let pm = t.render(40, 40);
    assert_eq!(px(&pm, 20, 22), [255, 0, 0, 255]);
    assert_eq!(px(&pm, 2, 2), WHITE);

    let mut t = T::new();
    t.add(None, |id| {
        let mut n = Node::new(
            id,
            NodeKind::Vector {
                paths: vec![Path {
                    cmds: vec![
                        PathCmd::MoveTo(Vec2::new(0.0, 0.0)),
                        PathCmd::CubicTo(Vec2::new(0.0, 20.0), Vec2::new(40.0, 20.0), Vec2::new(40.0, 0.0)),
                        PathCmd::LineTo(Vec2::new(20.0, 40.0)),
                        PathCmd::Close,
                    ],
                    fill_rule: FillRule::NonZero,
                }],
                network: None,
            },
        );
        n.size = Vec2::new(40.0, 40.0);
        n.fills = vec![Paint::solid(BLUE)];
        n
    });
    let pm = t.render(40, 40);
    assert_eq!(px(&pm, 20, 15), [0, 0, 255, 255]);
    assert_eq!(px(&pm, 2, 38), WHITE);
}

#[test]
fn drop_shadow_present_and_blurred() {
    let mut t = T::new();
    let r = t.rect(None, Rect::new(30.0, 30.0, 40.0, 40.0), RED);
    t.node(r).effects = vec![Effect::DropShadow {
        color: Color::rgba(0, 0, 0, 128),
        offset: Vec2::new(0.0, 8.0),
        radius: 8.0,
        spread: 0.0,
        visible: true,
        show_behind: false,
        blend: Default::default(),
    }];
    let pm = t.render(100, 100);
    assert_eq!(px(&pm, 50, 50), [255, 0, 0, 255], "node on top of its shadow");
    let below = px(&pm, 50, 76);
    assert!(below[0] < 250 && below[0] > 100, "shadow below: {below:?}");
    assert_eq!(px(&pm, 50, 10), WHITE, "no shadow above");
    let far = px(&pm, 50, 95);
    assert!(far[0] > below[0], "shadow fades away: {far:?}");
    // spread grows the shadow
    if let Effect::DropShadow { spread, offset, radius, .. } = &mut t.node(r).effects[0] {
        *spread = 10.0;
        *offset = Vec2::ZERO;
        *radius = 0.0;
    }
    let pm = t.render(100, 100);
    assert!(px(&pm, 25, 50)[0] < 200, "spread shadow left of the rect");
}

#[test]
fn inner_shadow_darkens_edges_only() {
    let mut t = T::new();
    let r = t.rect(None, Rect::new(10.0, 10.0, 60.0, 60.0), Color::WHITE);
    t.node(r).effects = vec![Effect::InnerShadow {
        color: Color::rgba(0, 0, 0, 255),
        offset: Vec2::new(0.0, 6.0),
        radius: 6.0,
        spread: 0.0,
        visible: true,
        blend: Default::default(),
    }];
    let pm = t.render(80, 80);
    assert!(px(&pm, 40, 12)[0] < 120, "top edge shadowed: {:?}", px(&pm, 40, 12));
    assert!(px(&pm, 40, 45)[0] > 240, "centre/bottom clear: {:?}", px(&pm, 40, 45));
    assert_eq!(px(&pm, 40, 5), WHITE, "outside untouched");
}

#[test]
fn layer_blur_and_background_blur() {
    let mut t = T::new();
    let r = t.rect(None, Rect::new(20.0, 20.0, 20.0, 20.0), RED);
    t.node(r).effects = vec![Effect::LayerBlur { radius: 10.0, visible: true }];
    let pm = t.render(60, 60);
    let edge = px(&pm, 19, 30);
    assert!(edge[1] < 250, "blur bleeds outside the shape: {edge:?}");
    let c = px(&pm, 30, 30);
    assert!(c[1] > 5 && c[1] < 200, "centre softened: {c:?}");

    // background blur: black/white stripes behind a glass pane
    let mut t = T::new();
    t.rect(None, Rect::new(0.0, 0.0, 30.0, 60.0), Color::BLACK);
    let g = t.rect(None, Rect::new(10.0, 10.0, 40.0, 40.0), Color::rgba(255, 255, 255, 0));
    t.node(g).effects = vec![Effect::BackgroundBlur { radius: 12.0, visible: true }];
    let pm = t.render(60, 60);
    let inside = px(&pm, 29, 30);
    assert!(inside[0] > 40 && inside[0] < 215, "blurred edge inside pane: {inside:?}");
    assert_eq!(px(&pm, 5, 30), [0, 0, 0, 255], "outside pane stays sharp");
}

#[test]
fn mask_applies_to_following_siblings() {
    let mut t = T::new();
    let m = t.add(None, |id| Node::ellipse(id, Rect::new(0.0, 0.0, 40.0, 40.0)));
    t.node(m).is_mask = true;
    t.rect(None, Rect::new(0.0, 0.0, 40.0, 40.0), RED);
    let pm = t.render(40, 40);
    assert_eq!(px(&pm, 20, 20), [255, 0, 0, 255]);
    assert_eq!(px(&pm, 1, 1), WHITE, "outside the mask shape");
}

#[test]
fn boolean_subtract_union() {
    let mut t = T::new();
    let b = t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::BooleanOp(BooleanOp::Subtract));
        n.size = Vec2::new(60.0, 60.0);
        n.fills = vec![Paint::solid(RED)];
        n
    });
    t.rect(Some(b), Rect::new(0.0, 0.0, 60.0, 60.0), BLUE);
    t.rect(Some(b), Rect::new(20.0, 20.0, 20.0, 20.0), BLUE);
    let pm = t.render(60, 60);
    assert_eq!(px(&pm, 5, 5), [255, 0, 0, 255], "uses the boolean node's own fill");
    assert_eq!(px(&pm, 30, 30), WHITE, "hole");
    t.node(b).kind = NodeKind::BooleanOp(BooleanOp::Intersect);
    let pm = t.render(60, 60);
    assert_eq!(px(&pm, 5, 5), WHITE);
    assert_eq!(px(&pm, 30, 30), [255, 0, 0, 255]);
    t.node(b).kind = NodeKind::BooleanOp(BooleanOp::Exclude);
    let pm = t.render(60, 60);
    assert_eq!(px(&pm, 30, 30), WHITE);
    assert_eq!(px(&pm, 5, 5), [255, 0, 0, 255]);
}

fn png_bytes(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    let mut pm = Pixmap::new(w, h).unwrap();
    for y in 0..h {
        for x in 0..w {
            let c = f(x, y);
            let i = ((y * w + x) * 4) as usize;
            pm.data_mut()[i..i + 4].copy_from_slice(&c);
        }
    }
    pm.encode_png().unwrap()
}

#[test]
fn image_fill_modes() {
    let mut t = T::new();
    // 20x10 image: left half red, right half blue
    t.doc.images.insert("img".into(), png_bytes(20, 10, |x, _| if x < 10 { [255, 0, 0, 255] } else { [0, 0, 255, 255] }));
    let r = t.rect(None, Rect::new(0.0, 0.0, 40.0, 40.0), RED);
    t.node(r).fills = vec![gradient_paint(PaintKind::Image { image_ref: "img".into(), scale_mode: ImageScaleMode::Fill })];
    let pm = t.render(40, 40);
    // cover: scale 4 -> 80x40 centred, so the red/blue split is at x=20 and the left 20px are cropped
    assert!(near(px(&pm, 5, 20), [255, 0, 0, 255], 4), "{:?}", px(&pm, 5, 20));
    assert!(near(px(&pm, 35, 20), [0, 0, 255, 255], 4), "{:?}", px(&pm, 35, 20));
    // fit: scale 2 -> 40x20, centred vertically with letterbox
    if let PaintKind::Image { scale_mode, .. } = &mut t.node(r).fills[0].kind {
        *scale_mode = ImageScaleMode::Fit;
    }
    let pm = t.render(40, 40);
    assert_eq!(px(&pm, 5, 2), WHITE, "letterbox");
    assert!(near(px(&pm, 5, 20), [255, 0, 0, 255], 4), "{:?}", px(&pm, 5, 20));
    assert!(near(px(&pm, 35, 20), [0, 0, 255, 255], 4));
    // tile at native size
    if let PaintKind::Image { scale_mode, .. } = &mut t.node(r).fills[0].kind {
        *scale_mode = ImageScaleMode::Tile;
    }
    let pm = t.render(40, 40);
    assert!(near(px(&pm, 2, 2), [255, 0, 0, 255], 4));
    assert!(near(px(&pm, 22, 2), [255, 0, 0, 255], 4), "second tile repeats: {:?}", px(&pm, 22, 2));
}

#[test]
fn text_renders_ink() {
    let mut t = T::new();
    let style = TextStyle { font_size: 24.0, ..Default::default() };
    t.add(None, |id| Node::text(id, Rect::new(5.0, 5.0, 100.0, 30.0), "HHH", style));
    let pm = t.render(120, 40);
    let dark = (0..pm.width()).flat_map(|x| (0..pm.height()).map(move |y| (x, y))).filter(|(x, y)| px(&pm, *x, *y)[0] < 100).count();
    assert!(dark > 60, "expected black glyph pixels, got {dark}");
    // nothing drawn outside the text box area on the far right
    assert_eq!(px(&pm, 118, 20), WHITE);

    // colour + alignment: centred text puts ink around the middle of the box
    let mut t = T::new();
    let style = TextStyle { font_size: 20.0, align_h: TextAlignH::Center, fills: vec![Paint::solid(RED)], ..Default::default() };
    let n = t.add(None, |id| Node::text(id, Rect::new(0.0, 0.0, 200.0, 30.0), "I", style));
    if let NodeKind::Text(p) = &mut t.node(n).kind {
        p.auto_resize = TextAutoResize::None;
    }
    let pm = t.render(200, 30);
    let xs: Vec<u32> = (0..200).filter(|x| (0..30).any(|y| px(&pm, *x, y)[1] < 100)).collect();
    assert!(!xs.is_empty());
    let mid = xs.iter().sum::<u32>() as f32 / xs.len() as f32;
    assert!((mid - 100.0).abs() < 6.0, "centred ink around x=100, got {mid}");
}

#[test]
fn text_case_and_decoration() {
    let mut t = T::new();
    let style = TextStyle { font_size: 20.0, case: TextCase::Upper, decoration: TextDecoration::Underline, ..Default::default() };
    t.add(None, |id| Node::text(id, Rect::new(0.0, 0.0, 120.0, 30.0), "abc", style));
    let pm = t.render(120, 40);
    // underline: a long horizontal dark run somewhere in the lower part
    let mut found = false;
    for y in 10..35 {
        let run = (0..40).filter(|x| px(&pm, *x, y)[0] < 128).count();
        if run >= 30 {
            found = true;
        }
    }
    assert!(found, "underline row not found");
}

#[test]
fn render_node_exports_subtree() {
    let mut t = T::new();
    let f = t.add(None, |id| Node::frame(id, Rect::new(100.0, 100.0, 20.0, 10.0)));
    t.node(f).fills = vec![Paint::solid(RED)];
    let pm = render_node(&t.doc, f, 2.0).unwrap();
    assert_eq!((pm.width(), pm.height()), (40, 20));
    assert_eq!(px(&pm, 20, 10), [255, 0, 0, 255]);
}

#[test]
fn culling_offscreen_nodes_is_harmless() {
    let mut t = T::new();
    t.rect(None, Rect::new(10_000.0, 10_000.0, 10.0, 10.0), RED);
    t.rect(None, Rect::new(-500.0, 0.0, 510.0, 10.0), BLUE);
    let pm = t.render(20, 20);
    assert_eq!(px(&pm, 5, 5), [0, 0, 255, 255]);
    assert_eq!(px(&pm, 5, 15), WHITE);
}

#[test]
fn unknown_table_and_cell_paint_and_index_children() {
    let mut t = T::new();
    let table = t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Unknown { type_name: "TABLE".into() });
        n.transform = Affine::translate(10.0, 20.0);
        n.size = Vec2::new(60.0, 60.0);
        // An Unknown node's own fills and effects remain undrawn; only its children draw.
        n.fills = vec![Paint::solid(BLUE)];
        n
    });
    assert!(!t.node(table).kind.is_container());
    let cell = t.add(Some(table), |id| {
        let mut n = Node::new(id, NodeKind::Unknown { type_name: "TABLE_CELL".into() });
        n.transform = Affine::translate(5.0, 6.0);
        n.size = Vec2::new(40.0, 30.0);
        n.fills = vec![Paint::solid(BLUE)];
        n
    });
    let rect = t.rect(Some(cell), Rect::new(2.0, 4.0, 20.0, 10.0), RED);
    let childless_unknown = t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Unknown { type_name: "SLICE".into() });
        n.transform = Affine::translate(60.0, 60.0);
        n.size = Vec2::new(10.0, 10.0);
        n.fills = vec![Paint::solid(BLUE)];
        n
    });

    let mut idx = designos_render::SceneIndex::new();
    idx.ensure(&t.doc, t.page, 1);
    assert_eq!(idx.bounds_of(rect), Some((17.0, 30.0, 37.0, 40.0)));
    assert_eq!(idx.bounds_of(cell), Some((17.0, 30.0, 37.0, 40.0)));
    assert_eq!(idx.bounds_of(table), Some((17.0, 30.0, 37.0, 40.0)));
    assert_eq!(idx.bounds_of(childless_unknown), None);
    let mut hits = Vec::new();
    idx.query_top_level((17.0, 30.0, 37.0, 40.0), &mut hits);
    assert_eq!(hits, vec![table]);

    let pm = t.render(80, 80);
    assert_eq!(px(&pm, 25, 35), [255, 0, 0, 255], "child rectangle inside TABLE -> TABLE_CELL paints opaque red");
    assert_eq!(px(&pm, 12, 22), WHITE, "TABLE and TABLE_CELL boxes themselves remain undrawn");

    // Incremental subtree update on the child under Unknown ancestors matches a full rebuild.
    t.node(rect).transform.tx += 10.0;
    assert!(idx.update_subtree(&t.doc, rect, 2));
    let mut full = designos_render::SceneIndex::new();
    full.ensure(&t.doc, t.page, 2);
    for id in [table, cell, rect] {
        assert_eq!(idx.bounds_of(id), full.bounds_of(id));
    }
}

#[test]
fn auto_layout_reverse_z_paints_first_child_on_top() {
    let mut t = T::new();
    let row = t.add(None, |id| {
        let mut n = Node::frame(id, Rect::new(0.0, 0.0, 40.0, 20.0));
        n.fills.clear();
        if let NodeKind::Frame(fp) = &mut n.kind {
            fp.clips_content = false;
            fp.auto_layout = Some(AutoLayout { direction: LayoutDirection::Horizontal, item_spacing: -12.0, reverse_z: true, ..Default::default() });
        }
        n
    });
    t.rect(Some(row), Rect::new(0.0, 0.0, 20.0, 20.0), RED);
    t.rect(Some(row), Rect::new(0.0, 0.0, 20.0, 20.0), BLUE);
    t.doc.relayout_subtree(row);

    let pm = t.render(40, 20);
    // Child 0 sits at x=0..20 (RED), child 1 sits at x=8..28 (BLUE). With reverse_z, child 0 paints on top in x=8..20.
    assert_eq!(px(&pm, 4, 10), [255, 0, 0, 255], "left of overlap is child 0 (red)");
    assert_eq!(px(&pm, 14, 10), [255, 0, 0, 255], "overlap with reverse_z paints child 0 (red) on top");
    assert_eq!(px(&pm, 24, 10), [0, 0, 255, 255], "right of overlap is child 1 (blue)");

    // Turning reverse_z off paints child 1 (blue) on top in the overlap.
    t.node(row).kind.frame_props_mut().unwrap().auto_layout.as_mut().unwrap().reverse_z = false;
    let pm_normal = t.render(40, 20);
    assert_eq!(px(&pm_normal, 14, 10), [0, 0, 255, 255], "overlap without reverse_z paints child 1 (blue) on top");
}
