//! The sibling index changes which layers the scans for layers around a text look at, never what
//! they find: on arbitrary scenes of frames, clips, strokes, effects, opacities, masks, transforms,
//! reversed paint order and variable-bound sizes, the indexed scans return what the reference scans
//! (every sibling of every ancestor, in paint order) return.

use super::{covering_layer_painted_after, has_layer_behind, has_plain_overlay_sibling_in_filled_frame, layer_painted_after, reference, Behind};
use crate::lint::rules::contrast::{backdrop, SubtreeBounds};
use crate::lint::SamplePoint;
use crate::{
    Affine, AutoLayout, BlendMode, Color, Document, Effect, FrameProps, Node, NodeId, NodeKind, Paint, Rect, StrokeAlign, TextStyle, Variable,
    VariableCollection, VariableKind, VariableMode, VariableValue, Vec2,
};

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

/// An empty document with one page and a number variable `wide` (300) to bind widths to.
fn scene_with_a_width_variable() -> (Document, NodeId) {
    let mut doc = Document::new("scene");
    let page = doc.add_page("Page");
    doc.variable_collections.insert(
        "c".into(),
        VariableCollection {
            id: "c".into(),
            name: "C".into(),
            modes: vec![VariableMode { id: "light".into(), name: "Light".into() }],
            default_mode: "light".into(),
            variables: vec![],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    doc.variables.insert(
        "wide".into(),
        Variable {
            id: "wide".into(),
            name: "wide".into(),
            collection: "c".into(),
            kind: VariableKind::Number,
            values: [("light".to_string(), VariableValue::Number(300.0))].into_iter().collect(),
            description: String::new(),
            scopes: vec![],
            hidden_from_publishing: false,
            key: None,
            code_syntax: Default::default(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        },
    );
    (doc, page)
}

/// A page of up to 90 random layers under random frames, and the texts among them.
fn random_scene(seed: u64) -> (Document, Vec<NodeId>) {
    let mut rng = Lcg(seed);
    let (mut doc, page) = scene_with_a_width_variable();
    let mut containers = vec![page];
    let mut texts = Vec::new();
    for _ in 0..90 {
        let parent = containers[rng.below(containers.len() as u32) as usize];
        let kind = rng.below(7);
        let id = doc.alloc_id();
        let rect = Rect::new(rng.between(-60.0, 420.0), rng.between(-60.0, 420.0), rng.between(10.0, 260.0), rng.between(0.0, 160.0));
        let mut n = match kind {
            0..=2 => {
                let mut f = Node::frame(id, rect);
                if rng.below(2) == 0 {
                    f.fills = vec![Paint::solid(gray(0xEE))];
                } else {
                    f.fills.clear();
                }
                let mut props = FrameProps { clips_content: rng.below(2) == 0, ..Default::default() };
                if rng.below(5) == 0 {
                    props.auto_layout = Some(AutoLayout { reverse_z: true, ..Default::default() });
                }
                f.kind = NodeKind::Frame(props);
                f
            }
            3..=5 => {
                let mut r = Node::rectangle(id, rect);
                r.fills = vec![Paint::solid(if rng.below(3) == 0 { Color { a: 0.5, ..gray(0x40) } } else { gray(0x40) })];
                r
            }
            _ => Node::text(
                id,
                Rect { w: 40.0, h: 16.0, ..rect },
                "Label",
                TextStyle { font_size: 14.0, fills: vec![Paint::solid(Color::WHITE)], ..Default::default() },
            ),
        };
        if rng.below(5) == 0 {
            n.transform = n.transform * Affine::scale(rng.between(0.5, 2.0), rng.between(0.5, 2.0));
        }
        if rng.below(12) == 0 {
            n.transform = n.transform * Affine::rotate(rng.between(-0.6, 0.6));
        }
        match rng.below(8) {
            0 => n.opacity = 0.0,
            1 => n.opacity = 0.5,
            2 => n.visible = false,
            _ => {}
        }
        if kind != 6 {
            if rng.below(14) == 0 {
                n.is_mask = true;
            }
            if rng.below(9) == 0 {
                n.blend = BlendMode::Multiply;
            }
            if rng.below(6) == 0 {
                n.variables.bindings.insert("width".into(), "wide".into());
            }
            if rng.below(3) == 0 {
                n.stroke.paints = vec![Paint::solid(Color::BLACK)];
                n.stroke.weight = rng.between(0.0, 30.0);
                n.stroke.align = [StrokeAlign::Inside, StrokeAlign::Center, StrokeAlign::Outside][rng.below(3) as usize];
            }
            match rng.below(6) {
                0 => {
                    n.effects = vec![Effect::DropShadow {
                        color: Color::BLACK,
                        offset: Vec2::new(rng.between(-20.0, 20.0), rng.between(-20.0, 20.0)),
                        radius: rng.between(0.0, 30.0),
                        spread: rng.between(-5.0, 10.0),
                        visible: true,
                        show_behind: false,
                    }]
                }
                1 => n.effects = vec![Effect::LayerBlur { radius: rng.between(0.0, 30.0), visible: true }],
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
    (doc, texts)
}

#[derive(Default)]
struct Seen {
    behind: [usize; 3],
    after: [usize; 2],
    covering: [usize; 2],
    overlay: [usize; 2],
}

fn compare_text(doc: &Document, t: NodeId, bounds: &SubtreeBounds, seen: &mut Seen, seed: u64) {
    let parent = doc.get(t).and_then(|n| n.parent);
    let analytic = backdrop(doc, t).and_then(|b| b.occluder);
    for occluder in [None, parent, analytic] {
        let want = reference::has_layer_behind(doc, t, occluder, Some(bounds));
        assert_eq!(has_layer_behind(doc, t, occluder, Some(bounds)), want, "has_layer_behind seed {seed} text {t:?} occluder {occluder:?}");
        assert_eq!(
            has_layer_behind(doc, t, occluder, None),
            reference::has_layer_behind(doc, t, occluder, None),
            "has_layer_behind without bounds, seed {seed} text {t:?}"
        );
        if occluder.is_none() {
            seen.behind[match want {
                Behind::Nothing => 0,
                Behind::Unknown => 1,
                Behind::Cover { .. } => 2,
            }] += 1;
        }
        let want = reference::has_plain_overlay_sibling_in_filled_frame(doc, t, occluder, Some(bounds));
        assert_eq!(
            has_plain_overlay_sibling_in_filled_frame(doc, t, occluder, Some(bounds)),
            want,
            "has_plain_overlay_sibling_in_filled_frame seed {seed} text {t:?} occluder {occluder:?}"
        );
        assert_eq!(
            has_plain_overlay_sibling_in_filled_frame(doc, t, occluder, None),
            reference::has_plain_overlay_sibling_in_filled_frame(doc, t, occluder, None),
            "has_plain_overlay_sibling_in_filled_frame without bounds, seed {seed} text {t:?}"
        );
        seen.overlay[usize::from(want)] += 1;
    }

    let want = reference::layer_painted_after(doc, t, Some(bounds));
    assert_eq!(layer_painted_after(doc, t, Some(bounds)), want, "layer_painted_after seed {seed} text {t:?}");
    assert_eq!(layer_painted_after(doc, t, None), reference::layer_painted_after(doc, t, None), "layer_painted_after without bounds, seed {seed} text {t:?}");
    seen.after[usize::from(want.is_some())] += 1;

    let Some(page_box) = crate::lint::rules::contrast::extent::page_box(doc, t) else { return };
    let (cx, cy) = (page_box.x + page_box.w * 0.5, page_box.y + page_box.h * 0.5);
    for (dx, dy) in [(0.0, 0.0), (-page_box.w * 0.4, 0.0), (page_box.w * 0.4, page_box.h * 0.4), (200.0, -150.0)] {
        let pt = SamplePoint { x: (cx + dx).round(), y: (cy + dy).round() };
        let want = reference::covering_layer_painted_after(doc, t, pt, Some(bounds));
        assert_eq!(covering_layer_painted_after(doc, t, pt, Some(bounds)), want, "covering_layer_painted_after seed {seed} text {t:?} at {pt:?}");
        assert_eq!(
            covering_layer_painted_after(doc, t, pt, None),
            reference::covering_layer_painted_after(doc, t, pt, None),
            "covering_layer_painted_after without bounds, seed {seed} text {t:?} at {pt:?}"
        );
        seen.covering[usize::from(want.is_some())] += 1;
    }
}

#[test]
fn the_sibling_index_never_changes_what_is_found_around_a_text() {
    let mut seen = Seen::default();
    for seed in 0..300 {
        let (doc, texts) = random_scene(seed);
        let bounds = SubtreeBounds::default();
        for t in texts {
            compare_text(&doc, t, &bounds, &mut seen, seed);
        }
    }
    assert!(seen.behind.iter().all(|n| *n > 30), "the scenes must exercise every answer behind a text: {:?}", seen.behind);
    assert!(seen.after.iter().all(|n| *n > 30), "the scenes must exercise both answers in front of a text: {:?}", seen.after);
    assert!(seen.covering.iter().all(|n| *n > 30), "the scenes must exercise both answers for a covering layer: {:?}", seen.covering);
    assert!(seen.overlay.iter().all(|n| *n > 5), "the scenes must exercise both answers for an overlay: {:?}", seen.overlay);
}

/// The index built a piece at a time under a tiny budget is the index built in one go.
#[test]
fn an_index_built_across_several_budgets_matches_the_one_built_at_once() {
    use std::time::{Duration, Instant};
    for seed in 0..60 {
        let (doc, texts) = random_scene(seed);
        let whole = SubtreeBounds::default();
        let pieces = SubtreeBounds::default();
        for t in texts {
            let mut rounds = 0;
            while !pieces.warm_for_text(&doc, t, Some(Instant::now() + Duration::ZERO)) {
                rounds += 1;
                assert!(rounds < 100_000, "warming makes progress under a zero budget, seed {seed}");
            }
            let (mut a, mut b) = (Seen::default(), Seen::default());
            compare_text(&doc, t, &whole, &mut a, seed);
            compare_text(&doc, t, &pieces, &mut b, seed);
        }
    }
}

/// A layer whose width comes from a variable is found by the covering scan through its resolved
/// width, though its stored box is too small to reach the sample.
#[test]
fn a_covering_layer_sized_by_a_variable_is_still_found() {
    let (mut doc, page) = scene_with_a_width_variable();
    let under_id = doc.alloc_id();
    let style = TextStyle { fills: vec![Paint::solid(Color::BLACK)], ..Default::default() };
    let under = doc.insert(page, Node::text(under_id, Rect::new(100.0, 10.0, 40.0, 16.0), "Label", style)).unwrap();
    let cover_id = doc.alloc_id();
    let mut cover = Node::rectangle(cover_id, Rect::new(0.0, 0.0, 10.0, 100.0));
    cover.fills = vec![Paint::solid(Color::WHITE)];
    cover.variables.bindings.insert("width".into(), "wide".into());
    doc.insert(page, cover).unwrap();
    let bounds = SubtreeBounds::default();
    let pt = SamplePoint { x: 120.0, y: 18.0 };
    let got = covering_layer_painted_after(&doc, under, pt, Some(&bounds));
    assert_eq!(got, reference::covering_layer_painted_after(&doc, under, pt, Some(&bounds)));
    assert!(got.is_some(), "the resolved 300 wide layer covers the sample");
}
