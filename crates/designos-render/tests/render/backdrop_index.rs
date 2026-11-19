//! Rendering a window of a page through its backdrop index draws the same pixels as walking every
//! layer of the page, and a lint pass that samples backdrops through the index finds exactly what a
//! pass without it finds.

use designos_doc::lint::{lint, Backdrop, LintConfig, LintEnv, LintPreset, TextMetrics};
use designos_doc::{Color, Document, Effect, FrameProps, Node, NodeId, NodeKind, Paint, Rect, StrokeAlign, TextAutoResize, TextProps, TextStyle, Vec2};
use designos_render::{render_backdrop, render_backdrop_indexed, BackdropIndex, RenderLintEnv};

/// A small deterministic generator, so a failing seed can be replayed.
struct Lcg(u64);

impl Lcg {
    fn below(&mut self, n: u32) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as u32) % n
    }

    fn between(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.below(1000) as f32 / 1000.0)
    }
}

fn gray(v: u8) -> Color {
    Color::rgb(v, v, v)
}

/// A page of random layers under random frames: fills, strokes, shadows, blurs, background blurs,
/// opacities, rotations, clips and, sometimes, a mask at the top level. Returns the texts too.
fn random_page(seed: u64, top_mask: bool) -> (Document, NodeId, Vec<NodeId>) {
    let mut rng = Lcg(seed);
    let mut doc = Document::new("scene");
    let page = doc.add_page("Page");
    let mut containers = vec![page];
    let mut texts = Vec::new();
    for i in 0..70 {
        let parent = containers[rng.below(containers.len() as u32) as usize];
        let kind = rng.below(7);
        let id = doc.alloc_id();
        let rect = Rect::new(rng.between(-40.0, 360.0), rng.between(-40.0, 360.0), rng.between(10.0, 220.0), rng.between(4.0, 140.0));
        let mut n = match kind {
            0..=2 => {
                let mut f = Node::frame(id, rect);
                f.fills = if rng.below(2) == 0 { vec![Paint::solid(gray(0xEE))] } else { Vec::new() };
                f.kind = NodeKind::Frame(FrameProps { clips_content: rng.below(2) == 0, ..Default::default() });
                f
            }
            3..=5 => {
                let mut r = Node::rectangle(id, rect);
                r.fills = vec![Paint::solid(if rng.below(3) == 0 { Color { a: 0.6, ..gray(0x30) } } else { gray(0x30 + 20 * (i % 6) as u8) })];
                r
            }
            _ => {
                let style = TextStyle { font_size: 14.0, fills: vec![Paint::solid(gray(0x22))], ..TextStyle::default() };
                let mut t = Node::text(id, Rect { w: 48.0, h: 18.0, ..rect }, "Label", style);
                if let NodeKind::Text(TextProps { auto_resize, .. }) = &mut t.kind {
                    *auto_resize = TextAutoResize::None;
                }
                t
            }
        };
        match rng.below(10) {
            0 => n.opacity = 0.5,
            1 => n.visible = false,
            2 => n.transform = n.transform * designos_doc::Affine::rotate(rng.between(-0.5, 0.5)),
            _ => {}
        }
        if kind != 6 {
            if rng.below(3) == 0 {
                n.stroke.paints = vec![Paint::solid(Color::BLACK)];
                n.stroke.weight = rng.between(0.0, 12.0);
                n.stroke.align = [StrokeAlign::Inside, StrokeAlign::Center, StrokeAlign::Outside][rng.below(3) as usize];
            }
            match rng.below(9) {
                0 => {
                    n.effects = vec![Effect::DropShadow {
                        color: Color { a: 0.5, ..Color::BLACK },
                        offset: Vec2::new(rng.between(-10.0, 10.0), rng.between(-10.0, 10.0)),
                        radius: rng.between(0.0, 20.0),
                        spread: rng.between(0.0, 6.0),
                        visible: true,
                        show_behind: false,
                        blend: Default::default(),
                    }]
                }
                1 => n.effects = vec![Effect::LayerBlur { radius: rng.between(1.0, 14.0), visible: true }],
                2 => n.effects = vec![Effect::BackgroundBlur { radius: rng.between(1.0, 14.0), visible: true }],
                _ => {}
            }
        }
        let is_container = matches!(n.kind, NodeKind::Frame(_));
        if kind == 6 {
            texts.push(id);
        }
        if let Ok(id) = doc.insert(parent, n) {
            if is_container {
                containers.push(id);
            }
        }
    }
    if top_mask {
        let id = doc.alloc_id();
        let mut m = Node::rectangle(id, Rect::new(0.0, 0.0, 200.0, 200.0));
        m.is_mask = true;
        let children = doc.get(page).map(|p| p.children.len()).unwrap_or(0);
        if doc.insert(page, m).is_ok() && children > 3 {
            // Move the mask below some of the layers, so it acts on the ones drawn after it.
            if let Some(p) = doc.get_mut(page) {
                let last = p.children.pop();
                if let Some(last) = last {
                    p.children.insert(children / 2, last);
                }
            }
        }
    }
    (doc, page, texts)
}

#[test]
fn a_window_rendered_through_the_index_has_the_pixels_of_the_full_render() {
    let mut windows = 0usize;
    for seed in 0..60 {
        let (doc, page, texts) = random_page(seed, seed % 4 == 0);
        let index = BackdropIndex::new(&doc, page);
        let mut rng = Lcg(seed + 1000);
        for k in 0..12 {
            let rect = match texts.get(k).and_then(|t| doc.get(*t)) {
                Some(t) if k % 2 == 0 => {
                    let w = doc.world_transform(t.id);
                    Rect::new(w.tx, w.ty, t.size.x.max(8.0), t.size.y.max(8.0))
                }
                _ => Rect::new(rng.between(-60.0, 380.0), rng.between(-60.0, 380.0), rng.between(6.0, 120.0), rng.between(6.0, 90.0)),
            };
            let scale = if k % 3 == 0 { 1.0 } else { 0.5 };
            for hide in [Vec::new(), texts.get(k).copied().into_iter().collect::<Vec<_>>()] {
                let full = render_backdrop(&doc, page, &hide, rect, scale).expect("full render");
                let fast = render_backdrop_indexed(&doc, page, &hide, rect, scale, &index).expect("indexed render");
                assert_eq!(fast.data(), full.data(), "seed {seed} window {rect:?} hiding {hide:?}");
                windows += 1;
            }
        }
    }
    assert!(windows > 1000);
}

/// Stripes of alternating colours, so that a blur reads something different on each side of a pixel.
fn striped_page() -> (Document, NodeId) {
    let mut doc = Document::new("stripes");
    let page = doc.add_page("Page");
    for i in 0..40 {
        let id = doc.alloc_id();
        let mut stripe = Node::rectangle(id, Rect::new(i as f32 * 5.0, 0.0, 5.0, 120.0));
        stripe.fills = vec![Paint::solid(if i % 2 == 0 { Color::BLACK } else { Color::WHITE })];
        doc.insert(page, stripe).unwrap();
    }
    (doc, page)
}

/// What a blur draws into a window comes from the layers around it, so the layers within the blur's
/// reach are drawn though they miss the window itself.
#[test]
fn layers_that_blur_into_the_window_are_drawn_though_they_miss_it() {
    for background in [true, false] {
        let (mut doc, page) = striped_page();
        let id = doc.alloc_id();
        let mut lens = Node::rectangle(id, Rect::new(80.0, 20.0, 60.0, 60.0));
        lens.fills = vec![Paint::solid(Color { a: 0.2, ..Color::WHITE })];
        lens.effects =
            vec![if background { Effect::BackgroundBlur { radius: 12.0, visible: true } } else { Effect::LayerBlur { radius: 12.0, visible: true } }];
        doc.insert(page, lens).unwrap();
        let index = BackdropIndex::new(&doc, page);
        for rect in [Rect::new(100.0, 40.0, 10.0, 10.0), Rect::new(82.0, 22.0, 12.0, 12.0), Rect::new(125.0, 60.0, 14.0, 14.0)] {
            let full = render_backdrop(&doc, page, &[], rect, 1.0).unwrap();
            let fast = render_backdrop_indexed(&doc, page, &[], rect, 1.0, &index).unwrap();
            assert_eq!(fast.data(), full.data(), "background blur {background}, window {rect:?}");
        }
    }
}

/// A lint environment that has the renderer sample backdrops without the index: the reference.
struct WithoutIndex;

impl LintEnv for WithoutIndex {
    fn has_text_engine(&self) -> bool {
        true
    }

    fn text_metrics(&self, node: &Node, props: &TextProps, size: Vec2) -> Option<TextMetrics> {
        RenderLintEnv.text_metrics(node, props, size)
    }

    fn backdrop(&self, doc: &Document, node: NodeId, rect: Rect) -> Option<Backdrop> {
        RenderLintEnv.backdrop(doc, node, rect)
    }
}

#[test]
fn a_lint_pass_through_the_index_finds_what_a_pass_without_it_finds() {
    let config = LintConfig::from_preset(LintPreset::Agent);
    let mut contrast = 0usize;
    for seed in 0..40 {
        let (doc, _, _) = random_page(seed, seed % 5 == 0);
        let fast = lint(&doc, &config, &RenderLintEnv);
        let exact = lint(&doc, &config, &WithoutIndex);
        assert_eq!(fast.findings, exact.findings, "seed {seed}");
        contrast += fast.findings.iter().filter(|f| f.rule == "contrast").count();
    }
    assert!(contrast > 20, "the scenes must produce contrast findings: {contrast}");
}
