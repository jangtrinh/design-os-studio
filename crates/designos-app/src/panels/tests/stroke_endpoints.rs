//! The Start point and End point controls on dashed strokes, the Cap and Dash cap rows, the order of the
//! options, and the room their names have in the Stroke section.
//!
//! The dashed line runs at `y = 20` from `x = 10`, weight 4, dashes `[10, 6]`, so the stroke fills rows 18..22.
//! On a line 84 long the dashes cover x = 10..20, 26..36, 42..52, 58..68, 74..84 and 90..94: the path ends
//! inside the last dash, so the cap of the end point shows.

use super::*;
use egui::Pos2;
use std::cell::Cell;

const INK: u8 = 0x18;
const VIEW: (u32, u32) = (120, 40);

/// The Stroke section in a 240 px panel, driven by frames and pointer events.
struct Panel {
    ctx: egui::Context,
    region: Rect,
    now: Cell<f64>,
}

impl Panel {
    fn new() -> Self {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::theme::install_fonts(&ctx);
        Self { ctx, region: Rect::from_min_size(Pos2::ZERO, egui::Vec2::new(crate::theme::RIGHT_PANEL_WIDTH, 900.0)), now: Cell::new(1.0) }
    }

    fn frame(&self, app: &mut App, events: Vec<egui::Event>) -> egui::FullOutput {
        self.now.set(self.now.get() + 0.1);
        let raw = egui::RawInput { screen_rect: Some(self.region), time: Some(self.now.get()), events, ..Default::default() };
        self.ctx.run_ui(raw, |ui| super::super::properties::show(app, ui, self.region))
    }

    fn click(&self, app: &mut App, pos: Pos2) {
        self.now.set(self.now.get() + 0.5);
        let button = |pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        self.frame(app, vec![egui::Event::PointerMoved(pos)]);
        self.frame(app, vec![button(true)]);
        self.frame(app, vec![button(false)]);
        self.frame(app, vec![]);
    }

    fn rect(&self, key: &str) -> Option<Rect> {
        self.ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", key.to_string()))))
    }

    /// Opens the dropdown recorded as `dropdown` and picks the row that sets `cap`.
    fn pick(&self, app: &mut App, dropdown: &str, cap: StrokeCap) {
        self.frame(app, vec![]);
        let at = self.rect(dropdown).unwrap_or_else(|| panic!("{dropdown} is drawn")).center();
        self.click(app, at);
        let row = self.rect(&format!("menu-row-0-{}", option_of(cap))).unwrap_or_else(|| panic!("the {cap:?} row of {dropdown} is drawn"));
        self.click(app, row.center());
    }
}

/// Row of the Start point / End point dropdowns that sets `cap`.
fn option_of(cap: StrokeCap) -> usize {
    ENDPOINT_CAPS.iter().position(|&(_, c)| c == cap).expect("the dropdowns offer the cap")
}

fn end_caps(app: &App, id: NodeId) -> Option<(Option<StrokeCap>, Option<StrokeCap>)> {
    let NodeKind::Vector { network: Some(net), .. } = &app.doc.get(id)?.kind else { return None };
    let &(s, e) = net.open_chain_ends().first()?;
    Some((net.vertices[s].cap, net.vertices[e].cap))
}

/// A document with one open `Line` of the given length, in a Round-capped, dashed or solid stroke.
fn line_app(len: f32, cap: StrokeCap, dashes: Vec<f32>) -> (App, NodeId, NodeId) {
    let mut doc = Document::new("stroke-endpoints-dashed");
    let page = doc.add_page("Page 1");
    doc.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::WHITE };
    let mut app = App::new(doc);
    app.page = page;
    let id = app.doc.alloc_id();
    let mut line = Node::new(id, NodeKind::Line);
    line.transform = Affine::translate(10.0, 20.0);
    line.size = Vec2::new(len, 0.0);
    line.fills.clear();
    line.stroke = Stroke {
        paints: vec![Paint::solid(Color::rgb(INK, 0x1D, 0x27))],
        weight: 4.0,
        align: StrokeAlign::Center,
        cap,
        join: StrokeJoin::Miter,
        dashes,
        ..Stroke::default()
    };
    let line = app.doc.insert(page, line).unwrap();
    app.selection = vec![line];
    (app, page, line)
}

fn headless_gpu() -> Option<designos_gpu::HeadlessGpu> {
    designos_gpu::HeadlessGpu::new().ok().filter(|g| {
        let name = g.ctx.adapter_info.name.to_lowercase();
        g.ctx.adapter_info.device_type != designos_gpu::wgpu::DeviceType::Cpu
            && !name.contains("llvmpipe")
            && !name.contains("lavapipe")
            && !name.contains("software")
    })
}

/// Coverage of the stroke colour in pixel `(x, y)`: 0 on the white page, 1 where the stroke is solid.
fn coverage(pm: &designos_render::tiny_skia::Pixmap, x: u32, y: u32) -> f32 {
    (255.0 - pm.pixel(x, y).unwrap().red() as f32) / (255.0 - INK as f32)
}

/// A pixel of the scene and the coverage range `(least, most)` it must have: `(what, x, y, range)`.
type Probe = (&'static str, u32, u32, (f32, f32));

const SOLID: (f32, f32) = (0.9, 1.0);
const MOSTLY: (f32, f32) = (0.5, 1.0);
/// The corner pixel of a round cap is partly bare, the same pixel of a square cap is solid.
const ROUNDED_CORNER: (f32, f32) = (0.0, 0.6);

/// The CPU draws `probes`, and so does the GPU when the machine has one.
#[track_caller]
fn assert_drawn(app: &App, page: NodeId, gpu: &mut Option<designos_gpu::HeadlessGpu>, key: u64, what: &str, probes: &[Probe]) {
    let vp = designos_render::Viewport { offset: Vec2::ZERO, zoom: 1.0, ..designos_render::Viewport::default() };
    let mut cpu = designos_render::tiny_skia::Pixmap::new(VIEW.0, VIEW.1).unwrap();
    designos_render::render_page(&app.doc, page, &vp, &mut cpu);
    let mut images = vec![("CPU", cpu)];
    if let Some(g) = gpu.as_mut() {
        let frame = g.compile(&app.doc, page, key, &vp, VIEW);
        images.push(("GPU", g.render_frame(&frame, VIEW)));
    }
    let failed: Vec<String> = images
        .iter()
        .flat_map(|(renderer, pm)| {
            probes.iter().filter(move |(_, x, y, (least, most))| !(*least..=*most).contains(&coverage(pm, *x, *y))).map(move |(name, x, y, (least, most))| {
                format!("{renderer} {name}: pixel ({x}, {y}) has coverage {}, expected {least}..={most}", coverage(pm, *x, *y))
            })
        })
        .collect();
    assert!(failed.is_empty(), "{what}:\n{}", failed.join("\n"));
}

const ROUND_INNER_DASH_ENDS: [Probe; 2] =
    [("the inner end of the first dash is round", 21, 19, MOSTLY), ("the inner end of the fifth dash is round", 85, 19, MOSTLY)];

#[test]
fn end_point_edits_on_a_dashed_round_line_keep_the_dash_cap_and_draw_each_terminal_on_cpu_and_gpu() {
    let (mut app, page, line) = line_app(84.0, StrokeCap::Round, vec![10.0, 6.0]);
    let panel = Panel::new();
    let mut gpu = headless_gpu();

    // End point Square: the last dash ends square, the first one starts round, the inner dash ends stay round.
    panel.pick(&mut app, "stroke-end-point", StrokeCap::Square);
    assert_eq!(app.doc.get(line).unwrap().stroke.cap, StrokeCap::Round, "the dash cap is not touched by an end point edit");
    assert_eq!(end_caps(&app, line), Some((Some(StrokeCap::Round), Some(StrokeCap::Square))));
    let mut probes = ROUND_INNER_DASH_ENDS.to_vec();
    probes.extend([
        ("the end terminal is square: upper corner", 95, 18, SOLID),
        ("the end terminal is square: lower corner", 95, 21, SOLID),
        ("the start terminal is round", 8, 18, ROUNDED_CORNER),
    ]);
    assert_drawn(&app, page, &mut gpu, 1, "End point Square", &probes);

    // Swap: the start terminal is square now and the end terminal round, the dashes are as round as before.
    let swap = panel.rect("stroke-swap-points").expect("swap button").center();
    panel.click(&mut app, swap);
    assert_eq!(app.doc.get(line).unwrap().stroke.cap, StrokeCap::Round, "swapping the end points keeps the dash cap");
    assert_eq!(end_caps(&app, line), Some((Some(StrokeCap::Square), Some(StrokeCap::Round))));
    let mut probes = ROUND_INNER_DASH_ENDS.to_vec();
    probes.extend([
        ("the start terminal is square: upper corner", 8, 18, SOLID),
        ("the start terminal is square: lower corner", 8, 21, SOLID),
        ("the end terminal is round", 95, 18, ROUNDED_CORNER),
    ]);
    assert_drawn(&app, page, &mut gpu, 2, "swapped end points", &probes);

    // Every edit was one undo step: two undos bring the plain dashed line back.
    for _ in 0..2 {
        app.undo();
    }
    assert!(matches!(app.doc.get(line).unwrap().kind, NodeKind::Line), "undo restores the line");
    assert_eq!(app.doc.get(line).unwrap().stroke.cap, StrokeCap::Round);
}

#[test]
fn a_line_arrow_end_point_on_a_dashed_round_line_draws_the_head_and_keeps_round_dashes_on_cpu_and_gpu() {
    let (mut app, page, line) = line_app(84.0, StrokeCap::Round, vec![10.0, 6.0]);
    let panel = Panel::new();
    let mut gpu = headless_gpu();
    panel.pick(&mut app, "stroke-end-point", StrokeCap::ArrowLines);
    assert_eq!(app.doc.get(line).unwrap().stroke.cap, StrokeCap::Round);
    assert_eq!(end_caps(&app, line), Some((Some(StrokeCap::Round), Some(StrokeCap::ArrowLines))));
    let mut probes = ROUND_INNER_DASH_ENDS[..1].to_vec();
    probes.extend([
        ("the upper wing of the head", 87, 13, MOSTLY),
        ("the lower wing of the head", 87, 26, MOSTLY),
        ("the start terminal is round", 8, 18, ROUNDED_CORNER),
    ]);
    assert_drawn(&app, page, &mut gpu, 3, "End point Line arrow", &probes);
}

#[test]
fn swapping_the_end_points_of_a_dashed_round_line_keeps_the_dashes_round_on_cpu_and_gpu() {
    let (mut app, page, line) = line_app(84.0, StrokeCap::Round, vec![10.0, 6.0]);
    let panel = Panel::new();
    let mut gpu = headless_gpu();
    panel.frame(&mut app, vec![]);
    let swap = panel.rect("stroke-swap-points").expect("swap button").center();
    panel.click(&mut app, swap);
    assert_eq!(app.doc.get(line).unwrap().stroke.cap, StrokeCap::Round, "swapping the end points keeps the dash cap");
    let mut probes = ROUND_INNER_DASH_ENDS.to_vec();
    probes.extend([("the start terminal is round", 8, 18, ROUNDED_CORNER), ("the end terminal is round", 95, 18, ROUNDED_CORNER)]);
    assert_drawn(&app, page, &mut gpu, 5, "swapped end points on a plain dashed line", &probes);
}

#[test]
fn a_none_end_point_on_a_dashed_round_line_leaves_the_last_dash_butt_and_keeps_the_dash_cap() {
    let (mut app, page, line) = line_app(84.0, StrokeCap::Round, vec![10.0, 6.0]);
    let panel = Panel::new();
    let mut gpu = headless_gpu();
    panel.pick(&mut app, "stroke-end-point", StrokeCap::None);
    assert_eq!(app.doc.get(line).unwrap().stroke.cap, StrokeCap::Round);
    assert_eq!(end_caps(&app, line), Some((Some(StrokeCap::Round), Some(StrokeCap::None))));
    let mut probes = ROUND_INNER_DASH_ENDS.to_vec();
    probes.extend([("the last dash reaches the end of the path", 93, 19, SOLID), ("the end terminal is butt", 94, 19, (0.0, 0.05))]);
    assert_drawn(&app, page, &mut gpu, 4, "End point None", &probes);
}

/// Frames of the Stroke settings popover of one node, driven by pointer events.
struct Popover {
    ctx: egui::Context,
    now: Cell<f64>,
    state: PopState,
}

impl Popover {
    fn new(id: NodeId) -> Self {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::theme::install_fonts(&ctx);
        let anchor = Rect::from_min_size(Pos2::new(940.0, 120.0), egui::Vec2::splat(24.0));
        Self { ctx, now: Cell::new(1.0), state: PopState { kind: PopKind::Stroke, anchor, ids: vec![id] } }
    }

    fn frame(&self, app: &mut App, events: Vec<egui::Event>) -> Vec<(String, Rect)> {
        self.now.set(self.now.get() + 0.1);
        let screen = Rect::from_min_size(Pos2::ZERO, egui::Vec2::new(1000.0, 700.0));
        let region = Rect::from_min_max(Pos2::new(760.0, 40.0), Pos2::new(1000.0, 700.0));
        let raw = egui::RawInput { screen_rect: Some(screen), time: Some(self.now.get()), events, ..Default::default() };
        let shapes = self.ctx.run_ui(raw, |ui| popover_ui(app, ui, region, &self.state)).shapes;
        let mut out = Vec::new();
        for clipped in &shapes {
            collect_text(&clipped.shape, &mut out);
        }
        out
    }

    fn click(&self, app: &mut App, pos: Pos2) {
        self.now.set(self.now.get() + 0.5);
        let button = |pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        self.frame(app, vec![egui::Event::PointerMoved(pos)]);
        self.frame(app, vec![button(true)]);
        self.frame(app, vec![button(false)]);
        self.frame(app, vec![]);
    }

    /// Picks `option` of the dropdown that sits on the row labelled `label`.
    fn pick(&self, app: &mut App, label: &str, option: usize) {
        // the popover is laid out on its first frame and painted from the second
        self.frame(app, vec![]);
        let texts = self.frame(app, vec![]);
        let (_, label_rect) = texts.iter().find(|(text, _)| text == label).unwrap_or_else(|| panic!("the popover has a {label} row, got {texts:?}"));
        let dropdowns: Vec<(Id, Rect)> = self.ctx.data(|d| d.get_temp(Id::new("test-dd-list"))).unwrap_or_default();
        let (_, dropdown) = dropdowns.iter().find(|(_, r)| (r.center().y - label_rect.center().y).abs() < 12.0).copied().expect("the row has a dropdown");
        self.click(app, dropdown.center());
        let row = self.ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", format!("menu-row-0-{option}"))))).expect("the dropdown is open");
        self.click(app, row.center());
    }
}

fn collect_text(shape: &egui::Shape, out: &mut Vec<(String, Rect)>) {
    match shape {
        egui::Shape::Text(t) => out.push((t.galley.job.text.clone(), Rect::from_min_size(t.pos, t.galley.size()))),
        egui::Shape::Vec(v) => v.iter().for_each(|s| collect_text(s, out)),
        _ => {}
    }
}

#[test]
fn the_cap_row_gives_both_end_points_of_a_solid_open_line_the_same_cap_in_one_undo_step() {
    let (mut app, _page, line) = line_app(60.0, StrokeCap::None, vec![]);
    let panel = Panel::new();
    panel.pick(&mut app, "stroke-start-point", StrokeCap::Round);
    panel.pick(&mut app, "stroke-end-point", StrokeCap::ArrowLines);
    assert_eq!(end_caps(&app, line), Some((Some(StrokeCap::Round), Some(StrokeCap::ArrowLines))));

    let popover = Popover::new(line);
    popover.pick(&mut app, "Cap", CAPS.iter().position(|&(_, c)| c == StrokeCap::Square).unwrap());
    assert_eq!(app.doc.get(line).unwrap().stroke.cap, StrokeCap::Square);
    assert_eq!(end_caps(&app, line), Some((Some(StrokeCap::Square), Some(StrokeCap::Square))), "both end points follow the Cap row");

    app.undo();
    assert_eq!(app.doc.get(line).unwrap().stroke.cap, StrokeCap::None, "one undo restores the cap");
    assert_eq!(end_caps(&app, line), Some((Some(StrokeCap::Round), Some(StrokeCap::ArrowLines))), "one undo restores both end points");
}

#[test]
fn the_dash_cap_row_writes_only_the_stroke_cap_of_a_dashed_open_line() {
    let (mut app, _page, line) = line_app(84.0, StrokeCap::Round, vec![10.0, 6.0]);
    let panel = Panel::new();
    panel.pick(&mut app, "stroke-end-point", StrokeCap::Square);
    assert_eq!(end_caps(&app, line), Some((Some(StrokeCap::Round), Some(StrokeCap::Square))));

    let popover = Popover::new(line);
    popover.pick(&mut app, "Dash cap", CAPS.iter().position(|&(_, c)| c == StrokeCap::None).unwrap());
    assert_eq!(app.doc.get(line).unwrap().stroke.cap, StrokeCap::None, "the dashes are butt now");
    assert_eq!(end_caps(&app, line), Some((Some(StrokeCap::Round), Some(StrokeCap::Square))), "the end points keep their caps");
}

#[test]
fn the_endpoint_options_come_in_figmas_order() {
    let expected = [
        ("None", StrokeCap::None),
        ("Line arrow", StrokeCap::ArrowLines),
        ("Triangle arrow", StrokeCap::ArrowEquilateral),
        ("Reversed triangle", StrokeCap::TriangleFilled),
        ("Circle arrow", StrokeCap::CircleFilled),
        ("Diamond arrow", StrokeCap::DiamondFilled),
        ("Round", StrokeCap::Round),
        ("Square", StrokeCap::Square),
    ];
    assert_eq!(ENDPOINT_CAPS, expected);

    // the open menu lists them top to bottom in that order, and each row sets the cap it names
    let (mut app, _page, line) = line_app(60.0, StrokeCap::None, vec![]);
    let panel = Panel::new();
    for (i, (name, cap)) in expected.into_iter().enumerate() {
        panel.frame(&mut app, vec![]);
        let end = panel.rect("stroke-end-point").expect("End point dropdown");
        panel.click(&mut app, end.center());
        let out = panel.frame(&mut app, vec![]);
        let rows: Vec<Rect> = (0..expected.len()).map(|r| panel.rect(&format!("menu-row-0-{r}")).expect("menu row")).collect();
        assert!(rows.windows(2).all(|w| w[0].bottom() <= w[1].top() + 0.5), "the menu rows run top to bottom: {rows:?}");
        // the menu is painted over the panel: its text is the last painted in the row
        let listed: Vec<String> = rows.iter().map(|r| texts_in(&out, *r).pop().map(|(t, _)| t).unwrap_or_default()).collect();
        assert_eq!(listed, expected.map(|(n, _)| n.to_string()), "the menu lists every option in order");
        panel.click(&mut app, rows[option_of(cap)].center());
        assert_eq!(end_caps(&app, line).map(|c| c.1), Some(Some(cap)), "{name} (row {i}) sets {cap:?}");
    }
}

/// Texts painted inside `rect`.
fn texts_in(out: &egui::FullOutput, rect: Rect) -> Vec<(String, Rect)> {
    let mut all = Vec::new();
    for clipped in &out.shapes {
        collect_text(&clipped.shape, &mut all);
    }
    all.into_iter().filter(|(_, r)| rect.contains(r.center())).collect()
}

#[test]
fn every_endpoint_option_is_readable_in_full_in_its_control_and_in_the_open_menu() {
    let (mut app, _page, line) = line_app(60.0, StrokeCap::None, vec![]);
    let panel = Panel::new();
    // the text of a closed control may use the width of the control less the arrow, or it is cut short with an ellipsis
    let room = |r: Rect| r.width() - 28.0;
    for (name, cap) in ENDPOINT_CAPS {
        app.edit_nodes(&[line], |n| {
            set_node_endpoint_cap(n, false, cap);
            set_node_endpoint_cap(n, true, cap);
        });
        panel.frame(&mut app, vec![]);
        let out = panel.frame(&mut app, vec![]);
        // the rows keep their place when the line becomes a vector, so the rects are read after each edit
        let (start, end) = (panel.rect("stroke-start-point").unwrap(), panel.rect("stroke-end-point").unwrap());
        for (point, r) in [("Start point", start), ("End point", end)] {
            let shown = texts_in(&out, r);
            let label =
                shown.iter().find(|(t, _)| t.contains(name) || t.contains('…')).unwrap_or_else(|| panic!("{point} shows {name}, got {shown:?} in {r:?}"));
            assert_eq!(label.0, name, "{point} shows {name} in full");
            assert!(label.1.width() <= room(r), "{point}: {name} is {} px wide, the control has {} px", label.1.width(), room(r));
            eprintln!("{point}: {name} is {:.1} px wide, the control has {:.1} px of room", label.1.width(), room(r));
        }
    }

    // the open menu shows every name unclipped
    let end = panel.rect("stroke-end-point").unwrap();
    panel.click(&mut app, end.center());
    let out = panel.frame(&mut app, vec![]);
    for (i, (name, _)) in ENDPOINT_CAPS.into_iter().enumerate() {
        let row = panel.rect(&format!("menu-row-0-{i}")).expect("menu row");
        // the menu is painted over the panel: its text is the last painted in the row
        let (text, at) = texts_in(&out, row).pop().expect("the row has text");
        assert_eq!(text, name, "the open menu lists {name} in full");
        assert!(row.contains_rect(at), "{name} fits its row: {at:?} in {row:?}");
    }
}

#[test]
fn the_endpoint_rows_sit_one_under_the_other_inside_the_panel_with_reachable_targets() {
    let (mut app, _page, _line) = line_app(60.0, StrokeCap::None, vec![]);
    let panel = Panel::new();
    panel.frame(&mut app, vec![]);
    let (start, end, swap) = (panel.rect("stroke-start-point").unwrap(), panel.rect("stroke-end-point").unwrap(), panel.rect("stroke-swap-points").unwrap());
    assert!(start.bottom() < end.top(), "the End point row sits under the Start point row: {start:?} {end:?}");
    for (name, r) in [("Start point", start), ("End point", end), ("swap", swap)] {
        assert!(r.height() >= crate::theme::TARGET_MIN && r.width() >= crate::theme::TARGET_MIN, "{name} is at least {} px: {r:?}", crate::theme::TARGET_MIN);
        assert!(panel.region.contains_rect(r), "{name} sits inside the panel: {r:?}");
    }
    assert!(!start.intersects(swap) && !end.intersects(swap), "the swap button covers no dropdown");
}
