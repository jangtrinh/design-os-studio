//! The subtree bounds only make the scan for layers behind a text faster: with or without them the
//! answer is the same, on arbitrary trees of frames, clips, strokes, effects, opacities and
//! transforms.

use super::*;
use crate::check_designs::contrast::{has_layer_behind, Behind, SubtreeBounds};
use designos_doc::{Effect, StrokeAlign, Vec2};

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

/// A page of up to 90 random layers under random frames, and the texts among them.
fn random_page(seed: u64) -> (Fx, Vec<NodeId>) {
    let mut rng = Lcg(seed);
    let mut f = fx();
    white_page(&mut f);
    let mut containers = vec![f.page];
    let mut texts = Vec::new();
    for _ in 0..90 {
        let parent = containers[rng.below(containers.len() as u32) as usize];
        let kind = rng.below(6);
        let id = match kind {
            0..=2 => f.frame(parent, "Frame", (rng.below(2) == 0).then_some(gray(0xEE))),
            3 | 4 => f.rect(parent, "Rect", gray(0x40)),
            _ => f.text(parent, "Text", 14.0, 400, Color::WHITE),
        };
        let (w, h) = if kind == 5 { (40.0, 16.0) } else { (rng.between(10.0, 260.0), rng.between(0.0, 160.0)) };
        resize(&mut f, id, w, h);
        place(&mut f, id, rng.between(-60.0, 420.0), rng.between(-60.0, 420.0));
        let n = f.doc.get_mut(id).unwrap();
        if rng.below(5) == 0 {
            n.transform.a = rng.between(0.5, 2.0);
            n.transform.d = rng.between(0.5, 2.0);
        }
        match rng.below(8) {
            0 => n.opacity = 0.0,
            1 => n.opacity = 0.5,
            2 => n.visible = false,
            _ => {}
        }
        if kind == 5 {
            texts.push(id);
            continue;
        }
        if let Some(frame) = n.kind.frame_props_mut() {
            frame.clips_content = rng.below(2) == 0;
        }
        if rng.below(3) == 0 {
            n.stroke.paints = vec![Paint::solid(Color::BLACK)];
            n.stroke.weight = rng.between(0.0, 40.0);
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
        if kind <= 2 {
            containers.push(id);
        }
    }
    (f, texts)
}

#[test]
fn the_bounds_never_change_what_is_found_behind_a_text() {
    let (mut behind, mut clear) = (0, 0);
    for seed in 0..300 {
        let (f, texts) = random_page(seed);
        let bounds = SubtreeBounds::default();
        for t in texts {
            let parent = f.doc.get(t).and_then(|n| n.parent);
            for occluder in [None, parent.filter(|p| *p != f.page)] {
                let exact = has_layer_behind(&f.doc, t, occluder, None);
                let fast = has_layer_behind(&f.doc, t, occluder, Some(&bounds));
                assert_eq!(fast, exact, "seed {seed}, text {t:?}, occluder {occluder:?}");
                if occluder.is_none() {
                    if exact != Behind::Nothing {
                        behind += 1;
                    } else {
                        clear += 1;
                    }
                }
            }
        }
    }
    assert!(behind > 100 && clear > 100, "the scenes must exercise both answers: {behind} behind, {clear} clear");
}

/// `rows` rows of a frame that does not clip, holding 9 rectangles and 10 white texts that touch
/// none of them: 20 layers per row, 10 texts per row failing AA on the white page.
fn long_list(rows: usize) -> Fx {
    let mut f = fx();
    white_page(&mut f);
    let list = f.frame(f.page, "List", None);
    resize(&mut f, list, 1000.0, 40.0 * rows as f32);
    place(&mut f, list, 0.0, 0.0);
    f.doc.get_mut(list).unwrap().kind.frame_props_mut().unwrap().clips_content = false;
    for k in 0..rows {
        let row = f.frame(list, "Row", None);
        resize(&mut f, row, 1000.0, 40.0);
        place(&mut f, row, 0.0, 40.0 * k as f32);
        f.doc.get_mut(row).unwrap().kind.frame_props_mut().unwrap().clips_content = false;
        for i in 0..9 {
            let r = f.rect(row, "R", gray(0xEE));
            resize(&mut f, r, 40.0, 20.0);
            place(&mut f, r, 100.0 * i as f32, 0.0);
        }
        for i in 0..10 {
            let t = f.text(row, "Label", 14.0, 400, Color::WHITE);
            resize(&mut f, t, 40.0, 12.0);
            place(&mut f, t, 100.0 * i as f32, 24.0);
        }
    }
    f
}

#[test]
fn a_long_list_of_rows_is_checked_to_the_end() {
    // The scan visits at most 4096 layers per text. Without the bounds, the rows past the 215th
    // used that up on the rows before them and were skipped as "unknown".
    let report = long_list(300).report();
    assert!(!report.truncated);
    assert_eq!(report.of(Rule::Contrast).count(), 3000);
}
