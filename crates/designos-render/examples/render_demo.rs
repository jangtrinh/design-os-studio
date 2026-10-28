//! Builds a document exercising every renderer feature and writes `out/render_demo.png`.

use designos_doc::*;
use designos_render::{render_page, tiny_skia::Pixmap, Viewport};

struct B {
    doc: Document,
    page: NodeId,
}

impl B {
    fn add(&mut self, parent: Option<NodeId>, f: impl FnOnce(NodeId) -> Node) -> NodeId {
        let id = self.doc.alloc_id();
        self.doc.insert(parent.unwrap_or(self.page), f(id)).unwrap()
    }
    fn node(&mut self, id: NodeId) -> &mut Node {
        self.doc.get_mut(id).unwrap()
    }
    fn label(&mut self, x: f32, y: f32, s: &str) {
        let style = TextStyle { font_size: 11.0, fills: vec![Paint::solid(Color::rgb(0x55, 0x55, 0x55))], ..Default::default() };
        self.add(None, |id| Node::text(id, Rect::new(x, y, 150.0, 14.0), s, style));
    }
}

type GradKind = fn([Vec2; 3], Vec<GradientStop>) -> PaintKind;

fn grad(kind: fn([Vec2; 3], Vec<GradientStop>) -> PaintKind, a: Color, b: Color, h: [Vec2; 3]) -> Paint {
    Paint {
        kind: kind(h, vec![GradientStop { pos: 0.0, color: a }, GradientStop { pos: 1.0, color: b }]),
        opacity: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    }
}

fn checker() -> Vec<u8> {
    let mut pm = Pixmap::new(32, 32).unwrap();
    for y in 0..32u32 {
        for x in 0..32u32 {
            let on = ((x / 8) + (y / 8)) % 2 == 0;
            let c: [u8; 4] = if on { [0xF2, 0x4E, 0x1E, 255] } else { [0x0A, 0xCF, 0x83, 255] };
            let i = ((y * 32 + x) * 4) as usize;
            pm.data_mut()[i..i + 4].copy_from_slice(&c);
        }
    }
    pm.encode_png().unwrap()
}

fn main() {
    let mut doc = Document::new("demo");
    let page = doc.add_page("Demo");
    doc.images.insert("checker".into(), checker());
    let mut b = B { doc, page };

    // Row 1: basic shapes
    let r = b.add(None, |id| Node::rectangle(id, Rect::new(20.0, 20.0, 100.0, 70.0)));
    b.node(r).corner_radii = [24.0, 4.0, 24.0, 4.0];
    b.node(r).fills = vec![Paint::solid(Color::rgb(0x18, 0xA0, 0xFB))];
    b.label(20.0, 94.0, "independent radii");

    let e = b.add(None, |id| Node::ellipse(id, Rect::new(140.0, 20.0, 70.0, 70.0)));
    b.node(e).fills = vec![Paint::solid(Color::rgb(0xF2, 0x4E, 0x1E))];
    b.label(140.0, 94.0, "ellipse");

    let pie = b.add(None, |id| Node::ellipse(id, Rect::new(230.0, 20.0, 70.0, 70.0)));
    b.node(pie).kind = NodeKind::Ellipse(ArcData { start: 0.5, end: 4.2, inner_radius: 0.0 });
    b.node(pie).fills = vec![Paint::solid(Color::rgb(0x0A, 0xCF, 0x83))];
    b.label(230.0, 94.0, "pie arc");

    let donut = b.add(None, |id| Node::ellipse(id, Rect::new(320.0, 20.0, 70.0, 70.0)));
    b.node(donut).kind = NodeKind::Ellipse(ArcData { start: 0.0, end: 5.0, inner_radius: 0.6 });
    b.node(donut).fills = vec![Paint::solid(Color::rgb(0xA2, 0x59, 0xFF))];
    b.label(320.0, 94.0, "donut arc");

    let poly = b.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Polygon { count: 6 });
        n.transform = Affine::translate(410.0, 20.0);
        n.size = Vec2::new(70.0, 70.0);
        n.fills = vec![Paint::solid(Color::rgb(0xFF, 0xC7, 0x00))];
        n
    });
    let _ = poly;
    b.label(410.0, 94.0, "hexagon");

    b.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Star { count: 5, inner_ratio: 0.45 });
        n.transform = Affine::translate(500.0, 20.0);
        n.size = Vec2::new(70.0, 70.0);
        n.fills = vec![Paint::solid(Color::rgb(0xFF, 0x4D, 0x8D))];
        n
    });
    b.label(500.0, 94.0, "star");

    b.add(None, |id| {
        let mut n = Node::new(
            id,
            NodeKind::Vector {
                paths: vec![Path {
                    cmds: vec![
                        PathCmd::MoveTo(Vec2::new(0.0, 35.0)),
                        PathCmd::CubicTo(Vec2::new(0.0, -10.0), Vec2::new(70.0, -10.0), Vec2::new(70.0, 35.0)),
                        PathCmd::QuadTo(Vec2::new(35.0, 90.0), Vec2::new(0.0, 35.0)),
                        PathCmd::Close,
                    ],
                    fill_rule: FillRule::NonZero,
                }],
                network: None,
            },
        );
        n.transform = Affine::translate(590.0, 20.0);
        n.size = Vec2::new(70.0, 70.0);
        n.fills = vec![Paint::solid(Color::rgb(0x1B, 0xC4, 0xC4))];
        n.stroke.paints = vec![Paint::solid(Color::rgb(0x11, 0x11, 0x11))];
        n.stroke.weight = 2.0;
        n.stroke.align = StrokeAlign::Center;
        n.stroke.join = StrokeJoin::Round;
        n
    });
    b.label(590.0, 94.0, "vector path");

    // Row 2: paints
    let h = [Vec2::new(0.0, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.0, 1.0)];
    let hc = [Vec2::new(0.5, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.5, 1.0)];
    let (c0, c1) = (Color::rgb(0xFF, 0x4D, 0x8D), Color::rgb(0x18, 0x5A, 0xFB));
    let kinds: [(GradKind, [Vec2; 3], &str); 4] = [
        (|h, s| PaintKind::LinearGradient { handles: h, stops: s }, h, "linear"),
        (|h, s| PaintKind::RadialGradient { handles: h, stops: s }, hc, "radial"),
        (|h, s| PaintKind::AngularGradient { handles: h, stops: s }, hc, "angular"),
        (|h, s| PaintKind::DiamondGradient { handles: h, stops: s }, hc, "diamond"),
    ];
    for (i, (k, hh, name)) in kinds.iter().enumerate() {
        let x = 20.0 + i as f32 * 90.0;
        let r = b.add(None, |id| Node::rectangle(id, Rect::new(x, 130.0, 80.0, 70.0)));
        b.node(r).fills = vec![grad(*k, c0, c1, *hh)];
        b.label(x, 204.0, name);
    }
    for (i, mode) in [ImageScaleMode::Fill, ImageScaleMode::Fit, ImageScaleMode::Tile].into_iter().enumerate() {
        let x = 400.0 + i as f32 * 90.0;
        let r = b.add(None, |id| Node::rectangle(id, Rect::new(x, 130.0, 80.0, 70.0)));
        b.node(r).fills =
            vec![Paint { kind: PaintKind::Image { image_ref: "checker".into(), scale_mode: mode }, opacity: 1.0, visible: true, blend: BlendMode::Normal }];
        b.label(x, 204.0, &format!("image {mode:?}"));
    }

    // Row 3: strokes
    for (i, align) in [StrokeAlign::Inside, StrokeAlign::Center, StrokeAlign::Outside].into_iter().enumerate() {
        let x = 30.0 + i as f32 * 100.0;
        let r = b.add(None, |id| Node::rectangle(id, Rect::new(x, 250.0, 70.0, 60.0)));
        let n = b.node(r);
        n.corner_radii = [12.0; 4];
        n.fills = vec![Paint::solid(Color::rgb(0xE8, 0xF3, 0xFF))];
        n.stroke.paints = vec![Paint::solid(Color::rgb(0x18, 0x5A, 0xFB))];
        n.stroke.weight = 8.0;
        n.stroke.align = align;
        b.label(x, 322.0, &format!("{align:?}"));
    }
    let l = b.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Line);
        n.transform = Affine::translate(340.0, 265.0);
        n.size = Vec2::new(120.0, 0.0);
        n.stroke.paints = vec![Paint::solid(Color::rgb(0x22, 0x22, 0x22))];
        n.stroke.weight = 6.0;
        n.stroke.align = StrokeAlign::Center;
        n.stroke.cap = StrokeCap::Round;
        n.stroke.dashes = vec![0.1, 14.0];
        n
    });
    let _ = l;
    b.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Line);
        n.transform = Affine::translate(340.0, 295.0);
        n.size = Vec2::new(120.0, 0.0);
        n.stroke.paints = vec![Paint::solid(Color::rgb(0xF2, 0x4E, 0x1E))];
        n.stroke.weight = 4.0;
        n.stroke.align = StrokeAlign::Center;
        n.stroke.dashes = vec![12.0, 6.0];
        n
    });
    b.label(340.0, 322.0, "dashes, round caps");
    let sides = b.add(None, |id| Node::rectangle(id, Rect::new(500.0, 250.0, 70.0, 60.0)));
    let n = b.node(sides);
    n.fills = vec![Paint::solid(Color::WHITE)];
    n.stroke.paints = vec![Paint::solid(Color::rgb(0x11, 0x11, 0x11))];
    n.stroke.side_weights = Some([0.0, 0.0, 4.0, 0.0]);
    n.stroke.align = StrokeAlign::Inside;
    b.label(500.0, 322.0, "bottom border only");

    // Row 4: effects
    let sh = b.add(None, |id| Node::rectangle(id, Rect::new(30.0, 370.0, 90.0, 60.0)));
    let n = b.node(sh);
    n.corner_radii = [10.0; 4];
    n.fills = vec![Paint::solid(Color::WHITE)];
    n.effects = vec![
        Effect::DropShadow {
            color: Color::rgba(0, 0, 0, 64),
            offset: Vec2::new(0.0, 10.0),
            radius: 20.0,
            spread: -4.0,
            visible: true,
            show_behind: false,
            blend: Default::default(),
        },
        Effect::DropShadow {
            color: Color::rgba(0, 0, 0, 38),
            offset: Vec2::new(0.0, 2.0),
            radius: 4.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
            blend: Default::default(),
        },
    ];
    b.label(30.0, 450.0, "drop shadows");

    let inn = b.add(None, |id| Node::rectangle(id, Rect::new(150.0, 370.0, 90.0, 60.0)));
    let n = b.node(inn);
    n.corner_radii = [10.0; 4];
    n.fills = vec![Paint::solid(Color::rgb(0xF5, 0xC8, 0x42))];
    n.effects = vec![Effect::InnerShadow {
        color: Color::rgba(0, 0, 0, 110),
        offset: Vec2::new(0.0, 6.0),
        radius: 8.0,
        spread: 0.0,
        visible: true,
        blend: Default::default(),
    }];
    b.label(150.0, 450.0, "inner shadow");

    let lb = b.add(None, |id| Node::ellipse(id, Rect::new(270.0, 370.0, 60.0, 60.0)));
    b.node(lb).fills = vec![Paint::solid(Color::rgb(0xF2, 0x4E, 0x1E))];
    b.node(lb).effects = vec![Effect::LayerBlur { radius: 8.0, visible: true }];
    b.label(270.0, 450.0, "layer blur");

    // background blur: stripes with a frosted pane on top
    let stripes = b.add(None, |id| Node::new(id, NodeKind::Group));
    for i in 0..8 {
        let r = b.add(Some(stripes), |id| Node::rectangle(id, Rect::new(360.0 + i as f32 * 12.0, 370.0, 6.0, 60.0)));
        b.node(r).fills = vec![Paint::solid(Color::rgb(0x11, 0x11, 0x11))];
    }
    let pane = b.add(None, |id| Node::rectangle(id, Rect::new(380.0, 380.0, 60.0, 40.0)));
    let n = b.node(pane);
    n.corner_radii = [8.0; 4];
    n.fills = vec![Paint::solid(Color::rgba(255, 255, 255, 60))];
    n.effects = vec![Effect::BackgroundBlur { radius: 10.0, visible: true }];
    b.label(360.0, 450.0, "background blur");

    // opacity / blend
    let g = b.add(None, |id| Node::new(id, NodeKind::Group));
    for (dx, c) in [(0.0, Color::rgb(0xFF, 0, 0)), (24.0, Color::rgb(0, 0x80, 0xFF))] {
        let r = b.add(Some(g), |id| Node::rectangle(id, Rect::new(500.0 + dx, 375.0, 50.0, 50.0)));
        b.node(r).fills = vec![Paint::solid(c)];
    }
    b.node(g).opacity = 0.6;
    let mul = b.add(None, |id| Node::ellipse(id, Rect::new(520.0, 385.0, 40.0, 40.0)));
    b.node(mul).fills = vec![Paint::solid(Color::rgb(0xFF, 0xE0, 0x00))];
    b.node(mul).blend = BlendMode::Multiply;
    b.label(500.0, 450.0, "group opacity, multiply");

    // Row 5: clip, mask, boolean
    let fr = b.add(None, |id| Node::frame(id, Rect::new(30.0, 500.0, 90.0, 60.0)));
    if let NodeKind::Frame(p) = &mut b.node(fr).kind {
        p.clips_content = true;
    }
    b.node(fr).corner_radii = [20.0; 4];
    b.node(fr).fills = vec![Paint::solid(Color::rgb(0xE5, 0xE5, 0xE5))];
    let child = b.add(Some(fr), |id| Node::ellipse(id, Rect::new(50.0, 20.0, 80.0, 80.0)));
    b.node(child).fills = vec![Paint::solid(Color::rgb(0x18, 0xA0, 0xFB))];
    b.label(30.0, 570.0, "clips_content");

    let mgroup = b.add(None, |id| Node::new(id, NodeKind::Group));
    let mask = b.add(Some(mgroup), |id| Node::new(id, NodeKind::Star { count: 5, inner_ratio: 0.5 }));
    b.node(mask).transform = Affine::translate(150.0, 495.0);
    b.node(mask).size = Vec2::new(70.0, 70.0);
    b.node(mask).is_mask = true;
    let masked = b.add(Some(mgroup), |id| Node::rectangle(id, Rect::new(150.0, 495.0, 70.0, 70.0)));
    b.node(masked).fills =
        vec![grad(|h, s| PaintKind::LinearGradient { handles: h, stops: s }, c0, c1, [Vec2::new(0.0, 0.0), Vec2::new(1.0, 1.0), Vec2::new(0.0, 1.0)])];
    b.label(150.0, 570.0, "mask");

    let ops = [BooleanOp::Union, BooleanOp::Subtract, BooleanOp::Intersect, BooleanOp::Exclude];
    for (i, op) in ops.into_iter().enumerate() {
        let x = 260.0 + i as f32 * 90.0;
        let bo = b.add(None, |id| {
            let mut n = Node::new(id, NodeKind::BooleanOp(op));
            n.transform = Affine::translate(x, 500.0);
            n.size = Vec2::new(70.0, 55.0);
            n.fills = vec![Paint::solid(Color::rgb(0x7B, 0x61, 0xFF))];
            n
        });
        b.add(Some(bo), |id| Node::ellipse(id, Rect::new(0.0, 0.0, 45.0, 45.0)));
        b.add(Some(bo), |id| Node::ellipse(id, Rect::new(25.0, 10.0, 45.0, 45.0)));
        b.label(x, 570.0, &format!("{op:?}"));
    }

    // Row 6: text
    let t1 = TextStyle { font_size: 28.0, font_weight: 700, fills: vec![Paint::solid(Color::rgb(0x11, 0x11, 0x11))], ..Default::default() };
    b.add(None, |id| Node::text(id, Rect::new(30.0, 610.0, 300.0, 36.0), "Hello, designos", t1));
    let t2 = TextStyle {
        font_size: 14.0,
        line_height: LineHeight::Pixels(20.0),
        letter_spacing: 0.4,
        align_h: TextAlignH::Left,
        fills: vec![Paint::solid(Color::rgb(0x33, 0x33, 0x33))],
        ..Default::default()
    };
    let wrapped = b.add(None, |id| {
        Node::text(
            id,
            Rect::new(30.0, 650.0, 220.0, 70.0),
            "Text wraps inside a fixed width box, with explicit line height and letter spacing applied.",
            t2.clone(),
        )
    });
    if let NodeKind::Text(p) = &mut b.node(wrapped).kind {
        p.auto_resize = TextAutoResize::Height;
    }
    let centred = TextStyle { align_h: TextAlignH::Center, align_v: TextAlignV::Center, font_size: 16.0, decoration: TextDecoration::Underline, ..t2.clone() };
    let c = b.add(None, |id| Node::text(id, Rect::new(280.0, 610.0, 200.0, 50.0), "centered + underline", centred));
    if let NodeKind::Text(p) = &mut b.node(c).kind {
        p.auto_resize = TextAutoResize::None;
    }
    let upper = TextStyle {
        case: TextCase::Upper,
        font_size: 12.0,
        letter_spacing: 1.5,
        fills: vec![grad(|h, s| PaintKind::LinearGradient { handles: h, stops: s }, c0, c1, h)],
        ..Default::default()
    };
    b.add(None, |id| Node::text(id, Rect::new(280.0, 675.0, 200.0, 16.0), "gradient uppercase", upper));
    let rich =
        b.add(None, |id| Node::text(id, Rect::new(500.0, 610.0, 200.0, 24.0), "bold red and regular", TextStyle { font_size: 16.0, ..Default::default() }));
    if let NodeKind::Text(p) = &mut b.node(rich).kind {
        p.runs.push(TextRun {
            start: 0,
            end: 8,
            style: TextStyle { font_size: 16.0, font_weight: 700, fills: vec![Paint::solid(Color::rgb(0xE0, 0x20, 0x20))], ..Default::default() },
        });
    }

    // looping angular gradient: first and last stop match, so the sweep has no seam
    let ring = b.add(None, |id| Node::rectangle(id, Rect::new(670.0, 130.0, 80.0, 70.0)));
    b.node(ring).fills = vec![Paint {
        kind: PaintKind::AngularGradient {
            handles: hc,
            stops: vec![GradientStop { pos: 0.0, color: c0 }, GradientStop { pos: 0.5, color: c1 }, GradientStop { pos: 1.0, color: c0 }],
        },
        opacity: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    }];
    b.label(670.0, 204.0, "angular, looped");

    // Row 7: corner smoothing, truncation, rounded polygons
    for (i, smoothing) in [0.0f32, 0.6, 1.0].into_iter().enumerate() {
        let x = 30.0 + i as f32 * 100.0;
        let r = b.add(None, |id| Node::rectangle(id, Rect::new(x, 770.0, 80.0, 80.0)));
        b.node(r).corner_radii = [28.0; 4];
        b.node(r).corner_smoothing = smoothing;
        b.node(r).fills = vec![Paint::solid(Color::rgb(0x18, 0xA0, 0xFB))];
        b.label(x, 856.0, &format!("smoothing {smoothing}"));
    }
    let long = "Truncated text keeps the first lines and ends the last visible one with an ellipsis like Figma does.";
    let tstyle =
        TextStyle { font_size: 13.0, line_height: LineHeight::Pixels(18.0), fills: vec![Paint::solid(Color::rgb(0x22, 0x22, 0x22))], ..Default::default() };
    for (i, (h, lines)) in [(54.0f32, Some(2u32)), (40.0, None)].into_iter().enumerate() {
        let x = 350.0 + i as f32 * 170.0;
        let t = b.add(None, |id| Node::text(id, Rect::new(x, 770.0, 150.0, h), long, tstyle.clone()));
        if let NodeKind::Text(p) = &mut b.node(t).kind {
            p.auto_resize = TextAutoResize::Truncate;
            p.max_lines = lines;
        }
        b.label(x, 856.0, if lines.is_some() { "truncate, max 2 lines" } else { "truncate, box height" });
    }
    for (i, (kind, name)) in
        [(NodeKind::Polygon { count: 6 }, "polygon radius"), (NodeKind::Star { count: 5, inner_ratio: 0.5 }, "star radius")].into_iter().enumerate()
    {
        let x = 30.0 + i as f32 * 100.0;
        b.add(None, |id| {
            let mut n = Node::new(id, kind);
            n.transform = Affine::translate(x, 880.0);
            n.size = Vec2::new(80.0, 80.0);
            n.corner_radii = [10.0, 0.0, 0.0, 0.0];
            n.fills = vec![Paint::solid(Color::rgb(0x7B, 0x61, 0xFF))];
            n
        });
        b.label(x, 966.0, name);
    }
    // blend and background blur inside an opacity group
    let burn_a = b.add(None, |id| Node::rectangle(id, Rect::new(260.0, 880.0, 80.0, 70.0)));
    b.node(burn_a).fills = vec![Paint::solid(Color::rgb(0xFF, 0xC8, 0x80))];
    let burn_b = b.add(None, |id| Node::ellipse(id, Rect::new(280.0, 890.0, 60.0, 60.0)));
    b.node(burn_b).fills = vec![Paint::solid(Color::rgb(0xA0, 0xA0, 0xFF))];
    b.node(burn_b).blend = BlendMode::LinearBurn;
    b.label(260.0, 966.0, "linear burn");

    let grp = b.add(None, |id| Node::new(id, NodeKind::Group));
    b.node(grp).opacity = 0.9;
    for i in 0..8 {
        let r = b.add(Some(grp), |id| Node::rectangle(id, Rect::new(380.0 + i as f32 * 12.0, 880.0, 6.0, 70.0)));
        b.node(r).fills = vec![Paint::solid(Color::rgb(0x11, 0x11, 0x11))];
    }
    let pane2 = b.add(Some(grp), |id| Node::rectangle(id, Rect::new(400.0, 895.0, 50.0, 40.0)));
    b.node(pane2).corner_radii = [8.0; 4];
    b.node(pane2).fills = vec![Paint::solid(Color::rgba(255, 255, 255, 60))];
    b.node(pane2).effects = vec![Effect::BackgroundBlur { radius: 10.0, visible: true }];
    b.label(380.0, 966.0, "bg blur in opacity group");

    let mut pm = Pixmap::new(780 * 2, 1000 * 2).unwrap();
    let vp = Viewport { offset: Vec2::ZERO, zoom: 1.0, dpr: 2.0 };
    let t0 = std::time::Instant::now();
    render_page(&b.doc, b.page, &vp, &mut pm);
    eprintln!("rendered in {:?}", t0.elapsed());
    std::fs::create_dir_all("out").unwrap();
    pm.save_png("out/render_demo.png").unwrap();
    println!("wrote out/render_demo.png");
}
