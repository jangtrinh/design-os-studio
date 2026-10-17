//! A frame exported to PDF must show what the canvas shows for the same frame. Every test builds a
//! document, exports it with `export_pdf`, and compares the page a viewer would paint with what the CPU
//! renderer draws for the node. Where the vector writer cannot express a feature the page may be a
//! flattened image instead, but it may never be silently wrong.

mod pdf_probe;

use designos_doc::*;
use designos_export::export_pdf;
use pdf_probe::{page_pixels, PageKind, Pdf, Pixels};

const SCALE: f32 = 2.0;

/// The exported page next to the canvas, both addressed in design pixels.
struct Compared {
    kind: PageKind,
    page: Pixels,
    canvas: Pixels,
    bytes: Vec<u8>,
}

fn export(doc: &Document, frame: NodeId) -> Compared {
    let bytes = export_pdf(doc, &[frame]).expect("the frame exports");
    let (kind, page) = page_pixels(&bytes, 0, SCALE);
    let pm = designos_render::render_node(doc, frame, SCALE).expect("the frame renders");
    let canvas = Pixels::from_pixmap_over_white(&pm, pm.width() as f32 / SCALE, pm.height() as f32 / SCALE);
    Compared { kind, page, canvas, bytes }
}

fn spread(a: [u8; 3], b: [u8; 3]) -> u8 {
    a.iter().zip(&b).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0)
}

impl Compared {
    fn assert_agree_at(&self, label: &str, points: &[(f32, f32)], tolerance: u8) {
        for &(x, y) in points {
            let (want, got) = (self.canvas.at(x, y), self.page.at(x, y));
            assert!(spread(want, got) <= tolerance, "{label}: at ({x}, {y}) the canvas draws {want:?} but the {:?} page shows {got:?}", self.kind);
        }
    }

    /// The canvas itself draws `want` at `p`, so a passing comparison cannot be two equally blank pages.
    fn assert_canvas_shows(&self, label: &str, p: (f32, f32), want: [u8; 3], tolerance: u8) {
        let got = self.canvas.at(p.0, p.1);
        assert!(spread(want, got) <= tolerance, "{label}: expected the canvas to draw {want:?} at {p:?}, found {got:?}");
    }

    fn assert_whole_page_agrees(&self, label: &str, w: f32, h: f32) {
        let (bad, total) = differing_pixels(&self.canvas, &self.page, w, h, 12);
        assert!(bad * 200 <= total, "{label}: {bad} of {total} sampled pixels differ between the canvas and the {:?} page", self.kind);
    }
}

/// Samples on a 2 px grid of a `w` x `h` frame where `a` and `b` differ by more than `tolerance`, and the sample count.
fn differing_pixels(a: &Pixels, b: &Pixels, w: f32, h: f32, tolerance: u8) -> (usize, usize) {
    let (mut bad, mut total) = (0, 0);
    let mut y = 1.0;
    while y < h {
        let mut x = 1.0;
        while x < w {
            total += 1;
            if spread(a.at(x, y), b.at(x, y)) > tolerance {
                bad += 1;
            }
            x += 2.0;
        }
        y += 2.0;
    }
    (bad, total)
}

fn solid(r: u8, g: u8, b: u8) -> Paint {
    Paint::solid(Color::rgb(r, g, b))
}

fn gradient(kind: fn(Vec<GradientStop>, [Vec2; 3]) -> PaintKind, stops: Vec<GradientStop>, handles: [Vec2; 3]) -> Paint {
    Paint { kind: kind(stops, handles), opacity: 1.0, visible: true, blend: BlendMode::Normal }
}

fn linear(stops: Vec<GradientStop>, handles: [Vec2; 3]) -> PaintKind {
    PaintKind::LinearGradient { stops, handles }
}

fn radial(stops: Vec<GradientStop>, handles: [Vec2; 3]) -> PaintKind {
    PaintKind::RadialGradient { stops, handles }
}

fn angular(stops: Vec<GradientStop>, handles: [Vec2; 3]) -> PaintKind {
    PaintKind::AngularGradient { stops, handles }
}

fn diamond(stops: Vec<GradientStop>, handles: [Vec2; 3]) -> PaintKind {
    PaintKind::DiamondGradient { stops, handles }
}

fn stop(pos: f32, color: Color) -> GradientStop {
    GradientStop { pos, color }
}

fn unit_handles() -> [Vec2; 3] {
    [Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0)]
}

fn centered_handles() -> [Vec2; 3] {
    [Vec2::new(0.5, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.5, 1.0)]
}

/// A document with one page and one white frame at the origin.
fn frame_doc(w: f32, h: f32) -> (Document, NodeId, NodeId) {
    let mut doc = Document::new("fidelity");
    let page = doc.add_page("P");
    let id = doc.alloc_id();
    let frame = doc.insert(page, Node::frame(id, Rect::new(0.0, 0.0, w, h))).unwrap();
    (doc, page, frame)
}

fn rect(doc: &mut Document, parent: NodeId, r: Rect, fill: Paint) -> NodeId {
    let id = doc.alloc_id();
    let mut n = Node::rectangle(id, r);
    n.fills = vec![fill];
    doc.insert(parent, n).unwrap()
}

fn text_node(doc: &mut Document, parent: NodeId, r: Rect, content: &str, size: f32) -> NodeId {
    let id = doc.alloc_id();
    let style = TextStyle { font_size: size, ..Default::default() };
    doc.insert(parent, Node::text(id, r, content, style)).unwrap()
}

fn photo_png(rgb: [u8; 3]) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([rgb[0], rgb[1], rgb[2], 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

fn image_paint(image_ref: &str) -> Paint {
    Paint { kind: PaintKind::Image { image_ref: image_ref.into(), scale_mode: ImageScaleMode::Fill }, opacity: 1.0, visible: true, blend: BlendMode::Normal }
}

// ---------------------------------------------------------------------------------------------
// Variables

fn variable(id: &str, kind: VariableKind, light: VariableValue, dark: VariableValue) -> Variable {
    Variable {
        id: id.into(),
        name: id.into(),
        collection: "c".into(),
        kind,
        values: [("light".to_string(), light), ("dark".to_string(), dark)].into_iter().collect(),
        description: String::new(),
        scopes: vec![],
        hidden_from_publishing: false,
        key: None,
        code_syntax: Default::default(),
    }
}

/// A 100 x 100 frame whose width, fill and caption are bound to variables (light: 120 wide, teal, "Light").
fn variable_doc() -> (Document, NodeId) {
    let (mut doc, _page, frame) = frame_doc(100.0, 100.0);
    doc.variable_collections.insert(
        "c".into(),
        VariableCollection {
            id: "c".into(),
            name: "C".into(),
            modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
            default_mode: "light".into(),
            variables: vec![],
            key: None,
        },
    );
    let teal = Color::rgb(20, 120, 140);
    let orange = Color::rgb(230, 120, 20);
    for v in [
        variable("w", VariableKind::Number, VariableValue::Number(120.0), VariableValue::Number(240.0)),
        variable("bg", VariableKind::Color, VariableValue::Color(teal), VariableValue::Color(orange)),
        variable("txt", VariableKind::String, VariableValue::String("Light".into()), VariableValue::String("Dark".into())),
    ] {
        doc.variables.insert(v.id.clone(), v);
    }
    let caption = text_node(&mut doc, frame, Rect::new(10.0, 40.0, 80.0, 20.0), "hi", 14.0);
    let f = doc.get_mut(frame).unwrap();
    f.variables.bindings.insert("width".into(), "w".into());
    f.variables.bindings.insert("fills/0".into(), "bg".into());
    doc.get_mut(caption).unwrap().variables.bindings.insert("characters".into(), "txt".into());
    (doc, frame)
}

fn page_text(bytes: &[u8]) -> String {
    let pdf = Pdf::parse(bytes);
    pdf.pages().iter().flat_map(|p| pdf.text_runs(p)).collect::<Vec<_>>().concat()
}

#[test]
fn bound_variables_size_color_and_caption_the_page_like_the_canvas() {
    let (doc, frame) = variable_doc();
    let out = export(&doc, frame);
    let pdf = Pdf::parse(&out.bytes);
    let page = &pdf.pages()[0];
    assert!(
        (page.width - 90.0).abs() < 0.01 && (page.height - 75.0).abs() < 0.01,
        "the bound width of 120 px makes a 90 x 75 pt page, got {} x {}",
        page.width,
        page.height
    );
    assert_eq!(out.kind, PageKind::Vector);
    out.assert_canvas_shows("bound fill", (110.0, 10.0), [20, 120, 140], 2);
    out.assert_agree_at("bound fill and width", &[(110.0, 10.0), (50.0, 10.0), (115.0, 90.0)], 3);
    let text = page_text(&out.bytes);
    assert!(text.contains("Light") && !text.contains("hi"), "the caption is the bound string, found {text:?}");
}

#[test]
fn ancestor_mode_switches_the_bound_values_on_the_page() {
    let (mut doc, frame) = variable_doc();
    doc.get_mut(frame).unwrap().variables.modes.insert("c".into(), "dark".into());
    let out = export(&doc, frame);
    let pdf = Pdf::parse(&out.bytes);
    assert!((pdf.pages()[0].width - 180.0).abs() < 0.01, "dark mode is 240 px wide, i.e. 180 pt");
    out.assert_canvas_shows("dark fill", (200.0, 10.0), [230, 120, 20], 2);
    out.assert_agree_at("dark fill", &[(200.0, 10.0), (230.0, 90.0)], 3);
    assert!(page_text(&out.bytes).contains("Dark"));
}

// ---------------------------------------------------------------------------------------------
// Content the vector writer has to carry over or flatten

#[test]
fn image_filled_shape_appears_on_the_page() {
    let (mut doc, _page, frame) = frame_doc(200.0, 100.0);
    doc.images.insert("photo".into(), photo_png([0, 200, 0]));
    rect(&mut doc, frame, Rect::new(20.0, 10.0, 160.0, 80.0), image_paint("photo"));
    let out = export(&doc, frame);
    out.assert_canvas_shows("photo", (100.0, 50.0), [0, 200, 0], 3);
    out.assert_agree_at("photo", &[(100.0, 50.0), (30.0, 20.0), (170.0, 80.0), (5.0, 5.0)], 4);
}

#[test]
fn image_stroke_appears_on_the_page() {
    let (mut doc, _page, frame) = frame_doc(120.0, 100.0);
    doc.images.insert("photo".into(), photo_png([10, 30, 220]));
    let id = rect(&mut doc, frame, Rect::new(20.0, 20.0, 80.0, 60.0), solid(255, 255, 255));
    let n = doc.get_mut(id).unwrap();
    n.stroke.paints = vec![image_paint("photo")];
    n.stroke.weight = 10.0;
    n.stroke.align = StrokeAlign::Center;
    let out = export(&doc, frame);
    out.assert_canvas_shows("image stroke", (60.0, 20.0), [10, 30, 220], 3);
    out.assert_agree_at("image stroke", &[(60.0, 20.0), (20.0, 50.0), (60.0, 50.0)], 4);
}

#[test]
fn drop_shadow_appears_on_the_page() {
    let (mut doc, _page, frame) = frame_doc(300.0, 200.0);
    let card = rect(&mut doc, frame, Rect::new(50.0, 40.0, 100.0, 60.0), solid(230, 60, 60));
    doc.get_mut(card).unwrap().effects.push(Effect::DropShadow {
        color: Color::rgba(0, 0, 0, 200),
        offset: Vec2::new(0.0, 12.0),
        radius: 6.0,
        spread: 0.0,
        visible: true,
        show_behind: false,
    });
    let out = export(&doc, frame);
    let below = out.canvas.at(100.0, 106.0);
    assert!(below[0] < 160, "the canvas draws the shadow under the card, found {below:?}");
    out.assert_agree_at("drop shadow", &[(100.0, 106.0), (100.0, 70.0), (20.0, 20.0)], 4);
}

#[test]
fn sticky_note_shadow_appears_on_the_page() {
    let (mut doc, _page, frame) = frame_doc(300.0, 220.0);
    let id = doc.alloc_id();
    doc.insert(frame, Node::sticky(id, Rect::new(60.0, 40.0, 120.0, 120.0), "Hello")).unwrap();
    let out = export(&doc, frame);
    let under = out.canvas.at(120.0, 162.0);
    assert!(under[0] < 250, "the canvas draws the sticky's contact shadow, found {under:?}");
    out.assert_agree_at("sticky shadow", &[(120.0, 162.0), (120.0, 100.0), (250.0, 200.0)], 4);
}

// ---------------------------------------------------------------------------------------------
// Gradients

#[test]
fn gradient_stops_keep_their_positions() {
    let (mut doc, _page, frame) = frame_doc(200.0, 40.0);
    let stops = vec![stop(0.25, Color::rgb(255, 0, 0)), stop(0.75, Color::rgb(0, 0, 255))];
    rect(&mut doc, frame, Rect::new(0.0, 0.0, 200.0, 40.0), gradient(linear, stops, unit_handles()));
    let out = export(&doc, frame);
    out.assert_canvas_shows("flat head", (20.0, 20.0), [255, 0, 0], 3);
    out.assert_canvas_shows("flat tail", (180.0, 20.0), [0, 0, 255], 3);
    out.assert_agree_at("stop positions", &[(20.0, 20.0), (60.0, 20.0), (100.0, 20.0), (140.0, 20.0), (180.0, 20.0)], 3);
    assert_eq!(out.kind, PageKind::Vector, "plain two-stop gradients stay vector");
}

#[test]
fn gradient_fading_to_transparent_matches_the_canvas() {
    let (mut doc, _page, frame) = frame_doc(200.0, 40.0);
    let stops = vec![stop(0.0, Color::rgba(255, 0, 0, 255)), stop(1.0, Color::rgba(255, 0, 0, 0))];
    rect(&mut doc, frame, Rect::new(0.0, 0.0, 200.0, 40.0), gradient(linear, stops, unit_handles()));
    let out = export(&doc, frame);
    let tail = out.canvas.at(190.0, 20.0);
    assert!(tail[1] > 200, "the canvas fades out to the white page, found {tail:?}");
    out.assert_agree_at("alpha ramp", &[(10.0, 20.0), (60.0, 20.0), (100.0, 20.0), (190.0, 20.0)], 4);
}

#[test]
fn elliptical_radial_gradient_follows_its_handles() {
    let (mut doc, _page, frame) = frame_doc(200.0, 100.0);
    let stops = vec![stop(0.0, Color::rgb(255, 0, 0)), stop(1.0, Color::rgb(0, 0, 255))];
    rect(&mut doc, frame, Rect::new(0.0, 0.0, 200.0, 100.0), gradient(radial, stops, centered_handles()));
    let out = export(&doc, frame);
    let top = out.canvas.at(100.0, 3.0);
    assert!(top[2] > 200, "the ellipse reaches its end color 50 px above the center, found {top:?}");
    out.assert_agree_at("ellipse", &[(100.0, 3.0), (190.0, 50.0), (150.0, 50.0), (100.0, 50.0), (100.0, 80.0)], 4);
    assert_eq!(out.kind, PageKind::Vector, "radial gradients stay vector");
}

#[test]
fn skewed_linear_gradient_follows_its_width_handle() {
    let (mut doc, _page, frame) = frame_doc(200.0, 100.0);
    let stops = vec![stop(0.0, Color::rgb(255, 0, 0)), stop(1.0, Color::rgb(0, 0, 255))];
    let handles = [Vec2::new(0.0, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.5, 1.0)];
    rect(&mut doc, frame, Rect::new(0.0, 0.0, 200.0, 100.0), gradient(linear, stops, handles));
    let out = export(&doc, frame);
    out.assert_agree_at("skewed basis", &[(20.0, 20.0), (20.0, 80.0), (100.0, 20.0), (100.0, 80.0), (180.0, 20.0), (180.0, 80.0)], 4);
    assert_eq!(out.kind, PageKind::Vector);
}

#[test]
fn angular_and_diamond_gradients_match_the_canvas() {
    for (name, kind) in [("angular", angular as fn(_, _) -> _), ("diamond", diamond as fn(_, _) -> _)] {
        let (mut doc, _page, frame) = frame_doc(200.0, 100.0);
        let stops = vec![stop(0.0, Color::rgb(255, 0, 0)), stop(1.0, Color::rgb(0, 0, 255))];
        rect(&mut doc, frame, Rect::new(0.0, 0.0, 200.0, 100.0), gradient(kind, stops, centered_handles()));
        let out = export(&doc, frame);
        out.assert_agree_at(name, &[(150.0, 50.0), (100.0, 80.0), (50.0, 50.0), (100.0, 20.0), (120.0, 60.0)], 4);
    }
}

#[test]
fn gradient_stroke_matches_the_canvas() {
    let (mut doc, _page, frame) = frame_doc(200.0, 100.0);
    let id = rect(&mut doc, frame, Rect::new(30.0, 20.0, 140.0, 60.0), solid(255, 255, 255));
    let stops = vec![stop(0.0, Color::rgb(255, 0, 0)), stop(1.0, Color::rgb(0, 0, 255))];
    let n = doc.get_mut(id).unwrap();
    n.stroke.paints = vec![gradient(linear, stops, unit_handles())];
    n.stroke.weight = 10.0;
    n.stroke.align = StrokeAlign::Center;
    let out = export(&doc, frame);
    out.assert_agree_at("gradient stroke", &[(40.0, 20.0), (100.0, 20.0), (160.0, 20.0), (100.0, 80.0), (30.0, 50.0)], 6);
}

// ---------------------------------------------------------------------------------------------
// Strokes and shapes

fn stroked_rect(doc: &mut Document, parent: NodeId, r: Rect, stroke: Stroke) -> NodeId {
    let id = rect(doc, parent, r, solid(240, 240, 240));
    doc.get_mut(id).unwrap().stroke = stroke;
    id
}

fn red_stroke(weight: f32, align: StrokeAlign) -> Stroke {
    Stroke { paints: vec![solid(200, 30, 30)], weight, align, miter_limit: 4.0, ..Default::default() }
}

#[test]
fn side_weight_stroke_draws_only_the_weighted_sides() {
    let (mut doc, _page, frame) = frame_doc(200.0, 120.0);
    let mut stroke = red_stroke(0.0, StrokeAlign::Inside);
    stroke.side_weights = Some([0.0, 0.0, 4.0, 0.0]);
    stroked_rect(&mut doc, frame, Rect::new(20.0, 20.0, 160.0, 80.0), stroke);
    let out = export(&doc, frame);
    out.assert_canvas_shows("bottom band", (100.0, 98.0), [200, 30, 30], 3);
    out.assert_agree_at("side weights", &[(100.0, 98.0), (100.0, 94.0), (100.0, 60.0), (21.0, 60.0), (100.0, 21.0)], 3);
    assert_eq!(out.kind, PageKind::Vector, "per-side borders stay vector");
}

#[test]
fn outside_stroke_stays_outside_the_shape() {
    let (mut doc, _page, frame) = frame_doc(200.0, 120.0);
    stroked_rect(&mut doc, frame, Rect::new(30.0, 30.0, 100.0, 60.0), red_stroke(6.0, StrokeAlign::Outside));
    let out = export(&doc, frame);
    out.assert_canvas_shows("outside band", (80.0, 27.0), [200, 30, 30], 3);
    out.assert_canvas_shows("fill untouched", (80.0, 33.0), [240, 240, 240], 3);
    out.assert_agree_at("outside stroke", &[(80.0, 27.0), (80.0, 33.0), (133.0, 60.0), (127.0, 60.0), (80.0, 93.0), (80.0, 87.0), (80.0, 60.0)], 3);
    assert_eq!(out.kind, PageKind::Vector, "outside strokes stay vector");
}

#[test]
fn inside_and_centered_strokes_match_the_canvas() {
    for align in [StrokeAlign::Inside, StrokeAlign::Center] {
        let (mut doc, _page, frame) = frame_doc(200.0, 120.0);
        stroked_rect(&mut doc, frame, Rect::new(30.0, 30.0, 100.0, 60.0), red_stroke(6.0, align));
        let out = export(&doc, frame);
        out.assert_agree_at("stroke alignment", &[(80.0, 24.0), (80.0, 27.0), (80.0, 33.0), (80.0, 36.0), (80.0, 60.0)], 3);
        assert_eq!(out.kind, PageKind::Vector);
    }
}

#[test]
fn miter_limit_of_the_stroke_is_honoured() {
    let (mut doc, _page, frame) = frame_doc(200.0, 100.0);
    let id = doc.alloc_id();
    let path = Path {
        cmds: vec![PathCmd::MoveTo(Vec2::new(20.0, 10.0)), PathCmd::LineTo(Vec2::new(170.0, 50.0)), PathCmd::LineTo(Vec2::new(20.0, 90.0)), PathCmd::Close],
        fill_rule: FillRule::NonZero,
    };
    let mut n = Node::new(id, NodeKind::Vector { paths: vec![path], network: None });
    n.size = Vec2::new(200.0, 100.0);
    n.stroke =
        Stroke { paints: vec![solid(200, 30, 30)], weight: 4.0, align: StrokeAlign::Center, join: StrokeJoin::Miter, miter_limit: 1.0, ..Default::default() };
    doc.insert(frame, n).unwrap();
    let out = export(&doc, frame);
    out.assert_canvas_shows("beveled tip", (175.0, 50.0), [255, 255, 255], 3);
    out.assert_agree_at("miter limit", &[(175.0, 50.0), (168.0, 50.0), (100.0, 30.0)], 3);
}

#[test]
fn corner_smoothing_matches_the_canvas() {
    let build = |smoothing: f32| {
        let (mut doc, _page, frame) = frame_doc(300.0, 220.0);
        let id = rect(&mut doc, frame, Rect::new(10.0, 10.0, 280.0, 200.0), solid(200, 30, 30));
        let n = doc.get_mut(id).unwrap();
        n.corner_radii = [70.0; 4];
        n.corner_smoothing = smoothing;
        (doc, frame)
    };
    let (doc, frame) = build(1.0);
    let out = export(&doc, frame);
    // the smoothed outline differs from a plain rounded rectangle, so this is not a vacuous comparison
    let (plain_doc, plain_frame) = build(0.0);
    let plain = export(&plain_doc, plain_frame);
    let (apart, _) = differing_pixels(&out.canvas, &plain.canvas, 300.0, 220.0, 12);
    assert!(apart >= 30, "smoothing changes the outline ({apart} sampled pixels differ from the plain rounded rectangle)");
    let (bad, _) = differing_pixels(&out.canvas, &out.page, 300.0, 220.0, 12);
    assert!(bad <= 4, "{bad} sampled pixels of the smoothed corners differ between the canvas and the {:?} page", out.kind);
    assert_eq!(out.kind, PageKind::Vector);
}

#[test]
fn precomputed_stroke_outline_is_filled_like_the_canvas() {
    let (mut doc, _page, frame) = frame_doc(120.0, 100.0);
    let id = rect(&mut doc, frame, Rect::new(20.0, 20.0, 80.0, 60.0), solid(240, 240, 240));
    let ring = |cmds: &[(f32, f32)]| {
        let mut out: Vec<PathCmd> =
            cmds.iter().enumerate().map(|(i, (x, y))| if i == 0 { PathCmd::MoveTo(Vec2::new(*x, *y)) } else { PathCmd::LineTo(Vec2::new(*x, *y)) }).collect();
        out.push(PathCmd::Close);
        out
    };
    let mut cmds = ring(&[(0.0, 0.0), (80.0, 0.0), (80.0, 60.0), (0.0, 60.0)]);
    cmds.extend(ring(&[(6.0, 6.0), (6.0, 54.0), (74.0, 54.0), (74.0, 6.0)]));
    let n = doc.get_mut(id).unwrap();
    n.stroke = red_stroke(4.0, StrokeAlign::Inside);
    n.stroke_geometry = Some(vec![Path { cmds, fill_rule: FillRule::NonZero }]);
    let out = export(&doc, frame);
    out.assert_canvas_shows("outline band", (60.0, 25.0), [200, 30, 30], 3);
    out.assert_canvas_shows("past the outline", (60.0, 28.0), [240, 240, 240], 3);
    out.assert_agree_at("stroke geometry", &[(60.0, 25.0), (60.0, 28.0), (24.0, 50.0), (60.0, 50.0)], 3);
    assert_eq!(out.kind, PageKind::Vector, "a precomputed stroke outline is a plain vector fill");
}

#[test]
fn degenerate_dash_pattern_is_not_written() {
    let (mut doc, _page, frame) = frame_doc(120.0, 60.0);
    let id = doc.alloc_id();
    let mut l = Node::new(id, NodeKind::Line);
    l.transform = Affine::translate(10.0, 30.0);
    l.size = Vec2::new(100.0, 0.0);
    l.stroke =
        Stroke { paints: vec![solid(200, 30, 30)], weight: 4.0, align: StrokeAlign::Center, dashes: vec![0.0, 0.0], miter_limit: 4.0, ..Default::default() };
    doc.insert(frame, l).unwrap();
    let out = export(&doc, frame);
    out.assert_agree_at("solid stroke", &[(60.0, 30.0), (20.0, 30.0)], 3);
    let pdf = Pdf::parse(&out.bytes);
    let content = String::from_utf8(pdf.pages()[0].content.clone()).unwrap();
    assert!(!content.contains("] 0 d"), "the canvas ignores a zero-length dash pattern, so the page must not ask for one:\n{content}");
}

fn boolean_doc(op: BooleanOp) -> (Document, NodeId) {
    let (mut doc, _page, frame) = frame_doc(300.0, 150.0);
    let id = doc.alloc_id();
    let mut b = Node::new(id, NodeKind::BooleanOp(op));
    b.size = Vec2::new(300.0, 150.0);
    b.fills = vec![solid(200, 30, 30)];
    let b = doc.insert(frame, b).unwrap();
    rect(&mut doc, b, Rect::new(20.0, 20.0, 100.0, 100.0), solid(10, 10, 10));
    rect(&mut doc, b, Rect::new(70.0, 20.0, 100.0, 100.0), solid(10, 10, 10));
    (doc, frame)
}

#[test]
fn intersect_boolean_fills_only_the_overlap() {
    let (doc, frame) = boolean_doc(BooleanOp::Intersect);
    let out = export(&doc, frame);
    out.assert_canvas_shows("overlap", (95.0, 70.0), [200, 30, 30], 3);
    out.assert_canvas_shows("left only", (45.0, 70.0), [255, 255, 255], 3);
    out.assert_agree_at("intersect", &[(95.0, 70.0), (45.0, 70.0), (145.0, 70.0)], 3);
    assert_eq!(out.kind, PageKind::Vector, "boolean results stay vector");
}

#[test]
fn subtract_boolean_fills_the_remainder_of_the_first_shape() {
    let (doc, frame) = boolean_doc(BooleanOp::Subtract);
    let out = export(&doc, frame);
    out.assert_canvas_shows("remainder", (45.0, 70.0), [200, 30, 30], 3);
    out.assert_canvas_shows("cutter only", (145.0, 70.0), [255, 255, 255], 3);
    out.assert_agree_at("subtract", &[(45.0, 70.0), (95.0, 70.0), (145.0, 70.0)], 3);
    assert_eq!(out.kind, PageKind::Vector);
}

#[test]
fn union_and_exclude_booleans_match_the_canvas() {
    for op in [BooleanOp::Union, BooleanOp::Exclude] {
        let (doc, frame) = boolean_doc(op);
        let out = export(&doc, frame);
        out.assert_agree_at("boolean", &[(45.0, 70.0), (95.0, 70.0), (145.0, 70.0)], 3);
    }
}

// ---------------------------------------------------------------------------------------------
// Text

#[test]
fn non_latin_text_is_extractable_as_written() {
    let (mut doc, _page, frame) = frame_doc(400.0, 100.0);
    text_node(&mut doc, frame, Rect::new(10.0, 10.0, 380.0, 40.0), "Tiếng Việt €100 – “ok”", 24.0);
    let bytes = export_pdf(&doc, &[frame]).unwrap();
    let text = page_text(&bytes);
    assert!(text.contains("Tiếng Việt €100 – “ok”"), "the selectable text is what the layer says, found {text:?}");
    assert!(!text.contains('?') && !text.contains('\u{FFFD}'), "no placeholder characters, found {text:?}");
    // a real extractor agrees when one is installed (hosted runners do not ship poppler)
    let tmp = std::env::temp_dir().join(format!("designos-vector-pdf-text-{}.pdf", std::process::id()));
    std::fs::write(&tmp, &bytes).unwrap();
    let extractor = std::process::Command::new("pdftotext").arg(&tmp).arg("-").output();
    let _ = std::fs::remove_file(&tmp);
    if let Ok(output) = extractor {
        let extracted = String::from_utf8_lossy(&output.stdout);
        assert!(extracted.contains("Tiếng Việt €100"), "pdftotext output: {extracted:?}");
    }
}

#[test]
fn ascii_text_keeps_its_plain_text_run() {
    let (mut doc, _page, frame) = frame_doc(300.0, 60.0);
    text_node(&mut doc, frame, Rect::new(10.0, 10.0, 280.0, 30.0), "Plain (text) with \\ marks", 20.0);
    let bytes = export_pdf(&doc, &[frame]).unwrap();
    assert!(page_text(&bytes).contains("Plain (text) with \\ marks"));
    let out = export(&doc, frame);
    out.assert_whole_page_agrees("ascii text outlines", 300.0, 60.0);
    assert_eq!(out.kind, PageKind::Vector);
}

#[test]
fn outlined_text_matches_the_canvas() {
    let (mut doc, _page, frame) = frame_doc(200.0, 120.0);
    let t = text_node(&mut doc, frame, Rect::new(10.0, 10.0, 180.0, 100.0), "Hi", 90.0);
    let n = doc.get_mut(t).unwrap();
    n.stroke.paints = vec![solid(200, 30, 30)];
    n.stroke.weight = 4.0;
    let out = export(&doc, frame);
    out.assert_whole_page_agrees("stroked text", 200.0, 120.0);
}

// ---------------------------------------------------------------------------------------------
// Masks, opacity, blending

#[test]
fn translucent_mask_fades_the_content_it_masks() {
    let (mut doc, _page, frame) = frame_doc(200.0, 100.0);
    let id = doc.alloc_id();
    let mut m = Node::ellipse(id, Rect::new(20.0, 10.0, 100.0, 80.0));
    m.is_mask = true;
    m.opacity = 0.5;
    doc.insert(frame, m).unwrap();
    rect(&mut doc, frame, Rect::new(0.0, 0.0, 200.0, 100.0), solid(255, 0, 0));
    let out = export(&doc, frame);
    out.assert_canvas_shows("half transparent mask", (70.0, 50.0), [255, 128, 128], 4);
    out.assert_agree_at("alpha mask", &[(70.0, 50.0), (170.0, 50.0), (10.0, 5.0)], 4);
}

#[test]
fn text_mask_clips_to_its_glyphs() {
    let (mut doc, _page, frame) = frame_doc(200.0, 100.0);
    let t = text_node(&mut doc, frame, Rect::new(10.0, 0.0, 180.0, 100.0), "W", 90.0);
    doc.get_mut(t).unwrap().is_mask = true;
    rect(&mut doc, frame, Rect::new(0.0, 0.0, 200.0, 100.0), solid(255, 0, 0));
    let out = export(&doc, frame);
    out.assert_whole_page_agrees("text mask", 200.0, 100.0);
}

#[test]
fn second_mask_narrows_the_first() {
    let (mut doc, _page, frame) = frame_doc(200.0, 100.0);
    for r in [Rect::new(0.0, 0.0, 120.0, 100.0), Rect::new(80.0, 0.0, 120.0, 100.0)] {
        let id = rect(&mut doc, frame, r, solid(10, 10, 10));
        doc.get_mut(id).unwrap().is_mask = true;
    }
    rect(&mut doc, frame, Rect::new(0.0, 0.0, 200.0, 100.0), solid(255, 0, 0));
    let out = export(&doc, frame);
    out.assert_canvas_shows("inside both masks", (100.0, 50.0), [255, 0, 0], 3);
    out.assert_canvas_shows("only the second mask", (160.0, 50.0), [255, 255, 255], 3);
    out.assert_agree_at("nested masks", &[(40.0, 50.0), (100.0, 50.0), (160.0, 50.0)], 3);
    assert_eq!(out.kind, PageKind::Vector, "opaque shape masks stay vector clips");
}

#[test]
fn frame_opacity_fades_its_page() {
    let (mut doc, _page, frame) = frame_doc(100.0, 100.0);
    let f = doc.get_mut(frame).unwrap();
    f.fills = vec![solid(0, 0, 0)];
    f.opacity = 0.5;
    let out = export(&doc, frame);
    out.assert_canvas_shows("half black", (50.0, 50.0), [128, 128, 128], 3);
    out.assert_agree_at("frame opacity", &[(50.0, 50.0), (5.0, 95.0)], 3);
    assert_eq!(out.kind, PageKind::Vector, "a childless single-paint frame fades by paint alpha");
}

#[test]
fn group_opacity_flattens_before_fading() {
    let (mut doc, _page, frame) = frame_doc(200.0, 100.0);
    let id = doc.alloc_id();
    let mut g = Node::new(id, NodeKind::Group);
    g.size = Vec2::new(200.0, 100.0);
    g.opacity = 0.5;
    let g = doc.insert(frame, g).unwrap();
    rect(&mut doc, g, Rect::new(20.0, 20.0, 100.0, 60.0), solid(0, 0, 0));
    rect(&mut doc, g, Rect::new(80.0, 20.0, 100.0, 60.0), solid(0, 0, 0));
    let out = export(&doc, frame);
    out.assert_canvas_shows("overlap fades like the rest", (100.0, 50.0), [128, 128, 128], 3);
    out.assert_agree_at("group opacity", &[(50.0, 50.0), (100.0, 50.0), (150.0, 50.0)], 3);
}

#[test]
fn blend_modes_match_the_canvas() {
    let (mut doc, _page, frame) = frame_doc(200.0, 100.0);
    rect(&mut doc, frame, Rect::new(20.0, 20.0, 100.0, 60.0), solid(255, 160, 0));
    let b = rect(&mut doc, frame, Rect::new(80.0, 30.0, 100.0, 60.0), solid(0, 120, 255));
    doc.get_mut(b).unwrap().blend = BlendMode::Multiply;
    let out = export(&doc, frame);
    let overlap = out.canvas.at(100.0, 50.0);
    assert!(overlap[1] < 100, "multiply darkens the overlap, found {overlap:?}");
    out.assert_agree_at("node blend", &[(100.0, 50.0), (150.0, 70.0), (40.0, 40.0)], 3);
}

#[test]
fn paint_blend_modes_match_the_canvas() {
    let (mut doc, _page, frame) = frame_doc(200.0, 100.0);
    rect(&mut doc, frame, Rect::new(20.0, 20.0, 100.0, 60.0), solid(255, 160, 0));
    let b = rect(&mut doc, frame, Rect::new(80.0, 30.0, 100.0, 60.0), solid(0, 120, 255));
    doc.get_mut(b).unwrap().fills[0].blend = BlendMode::Multiply;
    let out = export(&doc, frame);
    let overlap = out.canvas.at(100.0, 50.0);
    assert!(overlap[1] < 100, "multiply darkens the overlap, found {overlap:?}");
    out.assert_agree_at("paint blend", &[(100.0, 50.0), (150.0, 70.0)], 3);
}

// ---------------------------------------------------------------------------------------------
// Whole documents

#[test]
fn raster_fallback_page_keeps_selectable_text() {
    let (mut doc, _page, frame) = frame_doc(300.0, 120.0);
    doc.images.insert("photo".into(), photo_png([0, 200, 0]));
    rect(&mut doc, frame, Rect::new(10.0, 10.0, 120.0, 100.0), image_paint("photo"));
    text_node(&mut doc, frame, Rect::new(150.0, 40.0, 140.0, 30.0), "Caption beside the photo", 14.0);
    let out = export(&doc, frame);
    assert_eq!(out.kind, PageKind::Raster, "an image fill is flattened into the page");
    out.assert_agree_at("photo page", &[(70.0, 60.0), (200.0, 100.0)], 4);
    assert!(page_text(&out.bytes).contains("Caption beside the photo"), "the flattened page still carries its text");
}

#[test]
fn card_with_text_gradient_and_strokes_matches_the_canvas() {
    let (mut doc, _page, frame) = frame_doc(360.0, 240.0);
    doc.get_mut(frame).unwrap().fills = vec![solid(245, 246, 250)];
    let card = rect(&mut doc, frame, Rect::new(20.0, 20.0, 320.0, 200.0), solid(255, 255, 255));
    {
        let n = doc.get_mut(card).unwrap();
        n.corner_radii = [16.0; 4];
        n.stroke = Stroke { paints: vec![solid(200, 205, 215)], weight: 1.0, align: StrokeAlign::Inside, miter_limit: 4.0, ..Default::default() };
    }
    let bar = rect(
        &mut doc,
        frame,
        Rect::new(36.0, 36.0, 288.0, 14.0),
        gradient(linear, vec![stop(0.0, Color::rgb(40, 110, 230)), stop(0.6, Color::rgb(90, 180, 120)), stop(1.0, Color::rgb(240, 190, 60))], unit_handles()),
    );
    doc.get_mut(bar).unwrap().corner_radii = [7.0; 4];
    let id = doc.alloc_id();
    doc.insert(frame, Node::ellipse(id, Rect::new(40.0, 70.0, 56.0, 56.0))).unwrap();
    text_node(&mut doc, frame, Rect::new(110.0, 76.0, 200.0, 24.0), "Quarterly report", 20.0);
    text_node(&mut doc, frame, Rect::new(110.0, 104.0, 200.0, 18.0), "Updated 18 Oct", 13.0);
    let line = doc.alloc_id();
    let mut l = Node::new(line, NodeKind::Line);
    l.transform = Affine::translate(40.0, 150.0);
    l.size = Vec2::new(280.0, 0.0);
    l.stroke =
        Stroke { paints: vec![solid(120, 130, 150)], weight: 2.0, align: StrokeAlign::Center, dashes: vec![6.0, 4.0], miter_limit: 4.0, ..Default::default() };
    doc.insert(frame, l).unwrap();
    let out = export(&doc, frame);
    assert_eq!(out.kind, PageKind::Vector, "a card of shapes, text, gradient and strokes is written as vector content");
    out.assert_whole_page_agrees("card", 360.0, 240.0);
}

#[test]
fn rounded_root_frame_without_clipping_shows_children_in_its_corners() {
    let (mut doc, _page, frame) = frame_doc(160.0, 120.0);
    doc.get_mut(frame).unwrap().corner_radii = [40.0; 4];
    rect(&mut doc, frame, Rect::new(0.0, 0.0, 60.0, 60.0), solid(200, 30, 30));
    let out = export(&doc, frame);
    out.assert_canvas_shows("child in the corner", (3.0, 3.0), [200, 30, 30], 3);
    out.assert_agree_at("page corner", &[(3.0, 3.0), (3.0, 117.0), (80.0, 60.0)], 3);
}
