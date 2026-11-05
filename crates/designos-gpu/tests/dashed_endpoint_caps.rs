//! A dashed open stroke caps every dash with the stroke's own cap, and the cap of an end vertex shows only at
//! that end of the path. The CPU and GPU renderers draw it the same way. Tests print a note and skip the GPU
//! half when the machine has no GPU adapter; the CPU half always runs.
//!
//! The line runs at `y = 20` from `x = 10`, weight 4, dashes `[10, 6]`, so the stroke fills rows 18..22 and
//! the dashes cover x = 10..20, 26..36, 42..52, 58..68, 74..84 and 90..94 on a line 84 long.

mod common;

use common::*;
use designos_doc::*;
use designos_render::tiny_skia::Pixmap;
use designos_render::{render_page, Viewport};

const INK: u8 = 0x18;
const VIEW: (u32, u32) = (120, 40);
const ROUND: StrokeCap = StrokeCap::Round;

/// A dashed line 84 long whose stroke cap is `Round` and whose end vertices carry `caps`.
fn dashed_round_line(caps: (StrokeCap, StrokeCap)) -> T {
    let mut t = T::new();
    t.add(None, |id| {
        let path = Path { cmds: vec![PathCmd::MoveTo(Vec2::ZERO), PathCmd::LineTo(Vec2::new(84.0, 0.0))], fill_rule: FillRule::NonZero };
        let mut net = VectorNetwork::from_paths(std::slice::from_ref(&path));
        net.vertices[0].cap = Some(caps.0);
        net.vertices[1].cap = Some(caps.1);
        let mut n = Node::new(id, NodeKind::Vector { paths: vec![path], network: Some(net) });
        n.transform = Affine::translate(10.0, 20.0);
        n.size = Vec2::new(84.0, 0.0);
        n.fills.clear();
        n.stroke = Stroke {
            paints: vec![Paint::solid(Color::rgb(INK, 0x1D, 0x27))],
            weight: 4.0,
            align: StrokeAlign::Center,
            cap: ROUND,
            join: StrokeJoin::Miter,
            dashes: vec![10.0, 6.0],
            ..Stroke::default()
        };
        n
    });
    t
}

/// Coverage of the stroke colour in pixel `(x, y)`: 0 on the white page, 1 where the stroke is solid.
fn coverage(pm: &Pixmap, x: u32, y: u32) -> f32 {
    (255.0 - pm.pixel(x, y).unwrap().red() as f32) / (255.0 - INK as f32)
}

fn cpu_render(t: &T) -> Pixmap {
    let mut pm = Pixmap::new(VIEW.0, VIEW.1).unwrap();
    render_page(&t.doc, t.page, &Viewport::default(), &mut pm);
    pm
}

/// A pixel of the scene and the coverage range `(least, most)` it must have: `(what, x, y, range)`.
type Probe = (&'static str, u32, u32, (f32, f32));

const SOLID: (f32, f32) = (0.9, 1.0);
const MOSTLY: (f32, f32) = (0.5, 1.0);
/// The corner pixel of a round cap is partly bare, the same pixel of a square cap is solid.
const ROUNDED_CORNER: (f32, f32) = (0.0, 0.6);
const BARE: (f32, f32) = (0.0, 0.05);

/// The probes that fail, as messages.
fn failed_probes(pm: &Pixmap, renderer: &str, probes: &[Probe]) -> Vec<String> {
    probes
        .iter()
        .filter(|(_, x, y, (least, most))| !(*least..=*most).contains(&coverage(pm, *x, *y)))
        .map(|(what, x, y, (least, most))| format!("{renderer} {what}: pixel ({x}, {y}) has coverage {}, expected {least}..={most}", coverage(pm, *x, *y)))
        .collect()
}

/// The CPU draws `probes`, and so does the GPU, and the two images agree. Both renderers run before anything is asserted.
#[track_caller]
fn check_cpu_and_gpu(scene: &str, t: &T, probes: &[Probe]) {
    let mut failed = failed_probes(&cpu_render(t), "CPU", probes);
    if let Some(mut g) = gpu() {
        let c = compare(&mut g, scene, &t.doc, t.page, Viewport::default(), VIEW, 8);
        failed.extend(failed_probes(&c.gpu, "GPU", probes));
        if c.report_aa.mismatched != 0 {
            failed.push(format!("{} AA-forgiven mismatched pixels between CPU and GPU", c.report_aa.mismatched));
        }
        // the round caps of the dashes antialias differently on the two renderers; no pixel differs beyond that
        if c.report.ratio >= 0.03 || c.report.ssim <= 0.99 {
            failed.push(format!("CPU and GPU differ: mismatch {:.3}%, {:?}", c.report.ratio * 100.0, c.report));
        }
    }
    assert!(failed.is_empty(), "{scene}:\n{}", failed.join("\n"));
}

#[test]
fn a_square_end_on_a_dashed_round_line_squares_only_the_last_dash_on_cpu_and_gpu() {
    let t = dashed_round_line((ROUND, StrokeCap::Square));
    check_cpu_and_gpu(
        "dashed_round_square_end",
        &t,
        &[
            ("inner dash end keeps its round cap", 21, 19, MOSTLY),
            ("inner dash end keeps its round cap, fifth dash", 85, 19, MOSTLY),
            ("end terminal is square: upper corner", 95, 18, SOLID),
            ("end terminal is square: lower corner", 95, 21, SOLID),
            ("start terminal stays round", 8, 18, ROUNDED_CORNER),
        ],
    );
}

#[test]
fn a_none_end_on_a_dashed_round_line_leaves_the_last_dash_butt_on_cpu_and_gpu() {
    let t = dashed_round_line((ROUND, StrokeCap::None));
    check_cpu_and_gpu(
        "dashed_round_none_end",
        &t,
        &[
            ("inner dash end keeps its round cap", 21, 19, MOSTLY),
            ("the last dash reaches the end of the path", 93, 19, SOLID),
            ("end terminal is butt", 94, 19, BARE),
        ],
    );
}

#[test]
fn a_line_arrow_end_on_a_dashed_round_line_draws_the_head_and_keeps_round_dashes_on_cpu_and_gpu() {
    let t = dashed_round_line((ROUND, StrokeCap::ArrowLines));
    check_cpu_and_gpu(
        "dashed_round_arrow_end",
        &t,
        &[
            ("upper wing of the head", 87, 13, MOSTLY),
            ("lower wing of the head", 87, 26, MOSTLY),
            ("inner dash end keeps its round cap", 21, 19, MOSTLY),
            ("inner dash end keeps its round cap, third dash", 53, 19, MOSTLY),
            ("start terminal stays round", 8, 18, ROUNDED_CORNER),
        ],
    );
}

#[test]
fn an_arrow_head_start_on_a_dashed_round_line_keeps_round_inner_dashes_on_cpu_and_gpu() {
    // the head is five weights long, so the shaft starts at x = 28 and its dashes cover 28..38, 44..54, 60..70, 76..86 and 92..94
    let t = dashed_round_line((StrokeCap::ArrowEquilateral, ROUND));
    check_cpu_and_gpu(
        "dashed_round_arrow_start",
        &t,
        &[
            ("the head is drawn at the start", 28, 12, SOLID),
            ("inner dash end keeps its round cap", 39, 19, MOSTLY),
            ("inner dash end keeps its round cap, third dash", 71, 19, MOSTLY),
        ],
    );
}
