//! A `.fig` Line stores its stroke as an outline band at y in [-weight, 0]. A resize must keep that band
//! where it is (relative to the line) instead of redrawing the line as a primitive stroked around y = 0.

mod common;

use common::*;
use designos_doc::*;
use designos_render::tiny_skia::Pixmap;
use designos_render::{render_page, Viewport};

const WEIGHT: f32 = 10.0;
const ORIGIN: Vec2 = Vec2 { x: 20.0, y: 40.0 };

fn band(x0: f32, x1: f32) -> Vec<Path> {
    let cmds = vec![
        PathCmd::MoveTo(Vec2::new(x0, -WEIGHT)),
        PathCmd::LineTo(Vec2::new(x1, -WEIGHT)),
        PathCmd::LineTo(Vec2::new(x1, 0.0)),
        PathCmd::LineTo(Vec2::new(x0, 0.0)),
        PathCmd::Close,
    ];
    vec![Path { cmds, fill_rule: FillRule::NonZero }]
}

/// A Line as the `.fig` import leaves it: a primitive plus Figma's stroke band at [-weight, 0].
fn line(t: &mut T, parent: Option<NodeId>, at: Vec2, w: f32, imported: bool) -> NodeId {
    t.add(parent, |id| {
        let mut n = Node::new(id, NodeKind::Line);
        n.transform = Affine::translate(at.x, at.y);
        n.size = Vec2::new(w, 0.0);
        n.fills.clear();
        n.stroke.paints = vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))];
        n.stroke.weight = WEIGHT;
        if imported {
            n.stroke_geometry = Some(band(0.0, w));
        }
        n
    })
}

fn resize(doc: &mut Document, id: NodeId, size: Vec2) {
    let (old, transform) = {
        let n = doc.get(id).unwrap();
        (n.size, n.transform)
    };
    apply_constrained_geometry(doc.get_mut(id).unwrap(), transform, size);
    for (child, t, s) in plan_constraint_pass(doc, id, old, size, 64) {
        apply_constrained_geometry(doc.get_mut(child).unwrap(), t, s);
    }
}

fn render(t: &T) -> Pixmap {
    let mut pm = Pixmap::new(280, 100).unwrap();
    render_page(&t.doc, t.page, &Viewport::default(), &mut pm);
    pm
}

/// Rows (min, max) and columns (min, max) that hold ink.
fn ink(pm: &Pixmap) -> (u32, u32, u32, u32) {
    let (mut y0, mut y1, mut x0, mut x1) = (u32::MAX, 0, u32::MAX, 0);
    for y in 0..pm.height() {
        for x in 0..pm.width() {
            let p = pm.pixel(x, y).unwrap();
            if p.red() < 128 {
                (y0, y1, x0, x1) = (y0.min(y), y1.max(y), x0.min(x), x1.max(x));
            }
        }
    }
    (y0, y1, x0, x1)
}

fn save(pm: &Pixmap, name: &str) {
    if let Ok(dir) = std::env::var("P1_953_EVIDENCE") {
        pm.save_png(std::path::Path::new(&dir).join(name)).unwrap();
    }
}

#[test]
fn direct_resize_keeps_the_imported_line_band() {
    let mut t = T::new();
    let id = line(&mut t, None, ORIGIN, 100.0, true);
    let before = render(&t);
    save(&before, "direct-before.png");
    assert_eq!(ink(&before), (30, 39, 20, 119), "the band sits above the line");

    resize(&mut t.doc, id, Vec2::new(200.0, 0.0));
    let after = render(&t);
    save(&after, "direct-after.png");
    assert_eq!(ink(&after), (30, 39, 20, 219), "same rows, only wider");
}

#[test]
fn parent_resize_matches_a_direct_resize() {
    // SCALE doubles the line's x offset as well as its length; the direct twin starts where that lands
    let mut direct = T::new();
    let id = line(&mut direct, None, Vec2::new(ORIGIN.x * 2.0, ORIGIN.y), 100.0, true);
    resize(&mut direct.doc, id, Vec2::new(200.0, 0.0));

    let mut t = T::new();
    let frame = t.add(None, |id| {
        let mut n = Node::new(id, NodeKind::Frame(FrameProps::default()));
        n.size = Vec2::new(100.0, 60.0);
        n.fills.clear();
        n
    });
    let id = line(&mut t, Some(frame), ORIGIN, 100.0, true);
    t.node(id).constraints = Constraints { horizontal: Constraint::Scale, vertical: Constraint::Scale };
    resize(&mut t.doc, frame, Vec2::new(200.0, 60.0));
    assert_eq!(t.doc.get(id).unwrap().size.x, 200.0, "the constraint pass widened the line");
    let (a, b) = (render(&direct), render(&t));
    save(&b, "parent-after.png");
    assert_eq!(ink(&b), (30, 39, 40, 239), "the band stays above the line");
    assert_eq!(ink(&b), ink(&a), "a parent resize draws the line like a direct one");
    assert_eq!(a.data(), b.data(), "pixel for pixel");
}

#[test]
fn studio_line_without_an_outline_is_unchanged() {
    let mut t = T::new();
    let id = line(&mut t, None, ORIGIN, 100.0, false);
    resize(&mut t.doc, id, Vec2::new(200.0, 0.0));
    assert!(t.doc.get(id).unwrap().stroke_geometry.is_none());
    let mut fresh = T::new();
    line(&mut fresh, None, ORIGIN, 200.0, false);
    assert_eq!(render(&t).data(), render(&fresh).data(), "drawn as a line made at that width");
    // centred on its own y
    assert_eq!(ink(&render(&t)), (35, 44, 20, 219));
}

fn render_wide(t: &T) -> Pixmap {
    let mut pm = Pixmap::new(360, 100).unwrap();
    render_page(&t.doc, t.page, &Viewport::default(), &mut pm);
    pm
}

/// Runs of dark pixels along one row.
fn runs_on_row(pm: &Pixmap, y: u32) -> usize {
    let mut runs = 0;
    let mut inside = false;
    for x in 0..pm.width() {
        let dark = pm.pixel(x, y).unwrap().red() < 128;
        if dark && !inside {
            runs += 1;
        }
        inside = dark;
    }
    runs
}

#[test]
fn dashed_line_keeps_its_dash_period_after_a_resize() {
    // a 2 px line with 4/4 dashes: the bake holds 13 dash rectangles of the old length
    let mut t = T::new();
    let id = line(&mut t, None, ORIGIN, 100.0, true);
    let dashes: Vec<Path> = (0..13)
        .map(|i| {
            let x = i as f32 * 8.0;
            let end = (x + 4.0).min(100.0);
            let cmds = vec![
                PathCmd::MoveTo(Vec2::new(x, -2.0)),
                PathCmd::LineTo(Vec2::new(end, -2.0)),
                PathCmd::LineTo(Vec2::new(end, 0.0)),
                PathCmd::LineTo(Vec2::new(x, 0.0)),
                PathCmd::Close,
            ];
            Path { cmds, fill_rule: FillRule::NonZero }
        })
        .collect();
    let n = t.node(id);
    n.stroke.weight = 2.0;
    n.stroke.dashes = vec![4.0, 4.0];
    n.stroke_geometry = Some(dashes);
    resize(&mut t.doc, id, Vec2::new(300.0, 0.0));
    let runs = runs_on_row(&render_wide(&t), 39);
    assert!((36..=39).contains(&runs), "300 px of 4/4 dashes is about 38 dashes, not {runs}");
}

#[test]
fn zero_length_line_resized_draws_its_new_extent() {
    let mut t = T::new();
    let id = line(&mut t, None, ORIGIN, 0.0, true);
    resize(&mut t.doc, id, Vec2::new(40.0, 0.0));
    let (_, _, x0, x1) = ink(&render(&t));
    assert_eq!((x0, x1), (20, 59), "ink spans the 0..40 extent");
}

#[test]
fn arrow_cap_line_resized_is_drawn_live_like_a_fresh_one() {
    let mut t = T::new();
    let id = line(&mut t, None, ORIGIN, 100.0, true);
    t.node(id).stroke.cap = StrokeCap::ArrowLines;
    resize(&mut t.doc, id, Vec2::new(200.0, 0.0));
    assert!(t.doc.get(id).unwrap().stroke_geometry.is_none(), "the outline is cleared");
    let mut fresh = T::new();
    let f = line(&mut fresh, None, ORIGIN, 200.0, false);
    fresh.node(f).stroke.cap = StrokeCap::ArrowLines;
    assert_eq!(render(&t).data(), render(&fresh).data());
}
