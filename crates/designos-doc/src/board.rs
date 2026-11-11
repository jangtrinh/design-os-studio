//! Board (whiteboard) model: sticky notes, connectors, shapes with text and stamps.
//!
//! A [`DocumentKind::Board`] file uses the same node tree as a design file; the extra
//! [`NodeKind`] variants carry their own props, defined here, together with the pure
//! geometry the renderers and the editor share: sticky layout and text fitting,
//! magnets and connector routing, shape outlines and the stamp artwork.

use crate::*;
use serde::{Deserialize, Serialize};

/// Whether a file is a design file or a board (whiteboard) file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DocumentKind {
    #[default]
    Design,
    Board,
    /// A presentation built from ordered slide frames.
    Deck,
}

// --------------------------------------------------------------------- props

/// Default sticky note edge length.
pub const STICKY_SIZE: f32 = 240.0;
/// Default sticky note fill (pale yellow).
pub const STICKY_YELLOW: Color = Color { r: 1.0, g: 0.898, b: 0.561, a: 1.0 };
/// Largest sticky note text size; the text shrinks from here as it grows.
pub const STICKY_FONT_MAX: f32 = 32.0;
/// Smallest text size before a sticky note's text is clipped.
pub const STICKY_FONT_MIN: f32 = 8.0;

/// A sticky note. The note is drawn as a rounded square in `color` with a soft shadow.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StickyProps {
    pub color: Color,
    pub author: Option<String>,
    /// Text content. `text.style.font_size` is the largest size; renderers shrink it to fit.
    pub text: TextProps,
    pub show_author: bool,
    /// Keys this version does not know, kept so a file saved by a newer release (for example a
    /// vote count) keeps them through a load and a save.
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl Default for StickyProps {
    fn default() -> Self {
        Self {
            color: STICKY_YELLOW,
            author: None,
            text: board_text(STICKY_FONT_MAX, TextAlignH::Left, TextAlignV::Top),
            show_author: false,
            extra: BTreeMap::new(),
        }
    }
}

/// Text props with the board look: Inter medium, near-black, wrapping at the box width.
pub fn board_text(font_size: f32, h: TextAlignH, v: TextAlignV) -> TextProps {
    TextProps {
        content: String::new(),
        style: TextStyle { font_size, font_weight: 500, align_h: h, align_v: v, fills: vec![Paint::solid(Color::rgb(0x1E, 0x1E, 0x1E))], ..Default::default() },
        runs: Vec::new(),
        auto_resize: TextAutoResize::None,
        max_lines: None,
        ..Default::default()
    }
}

/// Which side of a node a connector end attaches to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Magnet {
    /// The side that faces the other end.
    #[default]
    Auto,
    Top,
    Right,
    Bottom,
    Left,
    Center,
}

impl Magnet {
    pub const SIDES: [Magnet; 4] = [Magnet::Top, Magnet::Right, Magnet::Bottom, Magnet::Left];

    /// Unit vector pointing out of the node at this magnet (zero for `Auto` / `Center`).
    pub fn outward(self) -> Vec2 {
        match self {
            Magnet::Top => Vec2::new(0.0, -1.0),
            Magnet::Right => Vec2::new(1.0, 0.0),
            Magnet::Bottom => Vec2::new(0.0, 1.0),
            Magnet::Left => Vec2::new(-1.0, 0.0),
            Magnet::Auto | Magnet::Center => Vec2::ZERO,
        }
    }

    pub fn opposite(self) -> Magnet {
        match self {
            Magnet::Top => Magnet::Bottom,
            Magnet::Bottom => Magnet::Top,
            Magnet::Left => Magnet::Right,
            Magnet::Right => Magnet::Left,
            m => m,
        }
    }

    /// Midpoint of the side (or the center) of `r`. `Auto` is the center.
    pub fn point_on(self, r: Rect) -> Vec2 {
        let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
        match self {
            Magnet::Top => Vec2::new(cx, r.y),
            Magnet::Right => Vec2::new(r.x + r.w, cy),
            Magnet::Bottom => Vec2::new(cx, r.y + r.h),
            Magnet::Left => Vec2::new(r.x, cy),
            Magnet::Auto | Magnet::Center => Vec2::new(cx, cy),
        }
    }
}

/// One end of a connector.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConnectorEnd {
    /// The node this end is attached to; `None` is a free end at `point`.
    pub node: Option<NodeId>,
    pub magnet: Magnet,
    /// Position in the connector's parent space: where a free end sits, and, for an
    /// attached end, where it was last resolved (used when the node goes away).
    pub point: Vec2,
}

impl ConnectorEnd {
    pub fn free(point: Vec2) -> Self {
        Self { node: None, magnet: Magnet::Auto, point }
    }
    pub fn attached(node: NodeId, magnet: Magnet) -> Self {
        Self { node: Some(node), magnet, point: Vec2::ZERO }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConnectorLine {
    #[default]
    Elbowed,
    Straight,
    Curved,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConnectorCap {
    #[default]
    None,
    /// Open chevron.
    Arrow,
    /// Filled triangle.
    Triangle,
    Circle,
    Diamond,
}

impl ConnectorCap {
    pub const ALL: [ConnectorCap; 5] = [ConnectorCap::None, ConnectorCap::Arrow, ConnectorCap::Triangle, ConnectorCap::Circle, ConnectorCap::Diamond];
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConnectorProps {
    pub start: ConnectorEnd,
    pub end: ConnectorEnd,
    pub line: ConnectorLine,
    pub start_cap: ConnectorCap,
    pub end_cap: ConnectorCap,
    /// Text drawn at the midpoint over a background.
    pub label: Option<TextProps>,
}

impl Default for ConnectorProps {
    fn default() -> Self {
        Self {
            start: ConnectorEnd::free(Vec2::ZERO),
            end: ConnectorEnd::free(Vec2::new(100.0, 0.0)),
            line: ConnectorLine::Elbowed,
            start_cap: ConnectorCap::None,
            end_cap: ConnectorCap::Arrow,
            label: None,
        }
    }
}

/// Outline of a shape-with-text node.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShapeKind {
    #[default]
    Square,
    RoundedRect,
    Ellipse,
    Diamond,
    Triangle,
    TriangleDown,
    Parallelogram,
    Star,
    Hexagon,
    Pentagon,
}

impl ShapeKind {
    pub const ALL: [ShapeKind; 10] = [
        ShapeKind::Square,
        ShapeKind::RoundedRect,
        ShapeKind::Ellipse,
        ShapeKind::Diamond,
        ShapeKind::Triangle,
        ShapeKind::TriangleDown,
        ShapeKind::Parallelogram,
        ShapeKind::Star,
        ShapeKind::Hexagon,
        ShapeKind::Pentagon,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ShapeKind::Square => "Square",
            ShapeKind::RoundedRect => "Rounded rectangle",
            ShapeKind::Ellipse => "Ellipse",
            ShapeKind::Diamond => "Diamond",
            ShapeKind::Triangle => "Triangle",
            ShapeKind::TriangleDown => "Inverted triangle",
            ShapeKind::Parallelogram => "Parallelogram",
            ShapeKind::Star => "Star",
            ShapeKind::Hexagon => "Hexagon",
            ShapeKind::Pentagon => "Pentagon",
        }
    }

    /// Outline in a `w` x `h` box, or `None` for the kinds drawn as a native rectangle / ellipse.
    pub fn path(self, w: f32, h: f32) -> Option<Path> {
        let pts: Vec<(f32, f32)> = match self {
            ShapeKind::Square | ShapeKind::RoundedRect | ShapeKind::Ellipse => return None,
            ShapeKind::Diamond => vec![(0.5, 0.0), (1.0, 0.5), (0.5, 1.0), (0.0, 0.5)],
            ShapeKind::Triangle => vec![(0.5, 0.0), (1.0, 1.0), (0.0, 1.0)],
            ShapeKind::TriangleDown => vec![(0.0, 0.0), (1.0, 0.0), (0.5, 1.0)],
            ShapeKind::Parallelogram => vec![(0.22, 0.0), (1.0, 0.0), (0.78, 1.0), (0.0, 1.0)],
            ShapeKind::Hexagon => vec![(0.25, 0.0), (0.75, 0.0), (1.0, 0.5), (0.75, 1.0), (0.25, 1.0), (0.0, 0.5)],
            ShapeKind::Pentagon => vec![(0.5, 0.0), (1.0, 0.382), (0.809, 1.0), (0.191, 1.0), (0.0, 0.382)],
            ShapeKind::Star => (0..10)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                    let r = if i % 2 == 0 { 1.0 } else { 0.382 };
                    // the unit-radius star spans x in +-0.951 and y in -1..0.809: stretch it over the box
                    ((r * a.cos() + 0.951_057) / 1.902_113, (r * a.sin() + 1.0) / 1.809_017)
                })
                .collect(),
        };
        let cmds = polygon_cmds(&pts.iter().map(|(x, y)| Vec2::new(x * w, y * h)).collect::<Vec<_>>());
        Some(Path { cmds, fill_rule: FillRule::NonZero })
    }

    /// The box (as fractions of the shape size) text is laid out in.
    pub fn text_box(self) -> (f32, f32, f32, f32) {
        match self {
            ShapeKind::Square | ShapeKind::RoundedRect => (0.0, 0.0, 1.0, 1.0),
            ShapeKind::Ellipse => (0.146, 0.146, 0.708, 0.708),
            ShapeKind::Diamond => (0.25, 0.25, 0.5, 0.5),
            ShapeKind::Triangle => (0.25, 0.4, 0.5, 0.5),
            ShapeKind::TriangleDown => (0.25, 0.1, 0.5, 0.5),
            ShapeKind::Parallelogram => (0.15, 0.0, 0.7, 1.0),
            ShapeKind::Star => (0.3, 0.3, 0.4, 0.4),
            ShapeKind::Hexagon => (0.15, 0.1, 0.7, 0.8),
            ShapeKind::Pentagon => (0.18, 0.25, 0.64, 0.6),
        }
    }
}

fn polygon_cmds(pts: &[Vec2]) -> Vec<PathCmd> {
    let mut cmds = Vec::with_capacity(pts.len() + 1);
    for (i, p) in pts.iter().enumerate() {
        cmds.push(if i == 0 { PathCmd::MoveTo(*p) } else { PathCmd::LineTo(*p) });
    }
    cmds.push(PathCmd::Close);
    cmds
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShapeWithTextProps {
    pub shape: ShapeKind,
    pub text: TextProps,
    /// Keys this version does not know, kept so a file saved by a newer release (for example a
    /// vote count) keeps them through a load and a save.
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl Default for ShapeWithTextProps {
    fn default() -> Self {
        Self { shape: ShapeKind::Square, text: board_text(20.0, TextAlignH::Center, TextAlignV::Center), extra: BTreeMap::new() }
    }
}

// ------------------------------------------------------------------- stickies

/// Where a sticky note's text goes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StickyLayout {
    pub corner: f32,
    pub pad: f32,
    /// Box the body text is fitted into (node-local).
    pub text_box: Rect,
    /// Author line box, when shown.
    pub author_box: Option<Rect>,
    pub author_size: f32,
}

/// Layout of a sticky note of `size`.
pub fn sticky_layout(size: Vec2, show_author: bool) -> StickyLayout {
    let m = size.x.min(size.y).max(1.0);
    let pad = (m * 0.075).max(6.0);
    let corner = (m * 0.035).max(2.0);
    let author_size = (m * 0.055).clamp(8.0, 16.0);
    let author_h = if show_author { author_size * 1.5 } else { 0.0 };
    let text_box = Rect::new(pad, pad, (size.x - 2.0 * pad).max(1.0), (size.y - 2.0 * pad - author_h).max(1.0));
    let author_box = show_author.then(|| Rect::new(pad, size.y - pad - author_h, (size.x - 2.0 * pad).max(1.0), author_h));
    StickyLayout { corner, pad, text_box, author_box, author_size }
}

/// Largest font size in `[min, max]` (0.25 steps) for which `fits(size)` holds, found by
/// bisection; `min` when nothing fits. `fits` must be monotone: true for small sizes.
pub fn fit_font_size(max: f32, min: f32, mut fits: impl FnMut(f32) -> bool) -> f32 {
    let (mut lo, mut hi) = (min.min(max), max.max(min));
    if fits(hi) {
        return hi;
    }
    if !fits(lo) {
        return lo;
    }
    while hi - lo > 0.25 {
        let mid = (lo + hi) / 2.0;
        if fits(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo * 4.0).floor() / 4.0
}

// ------------------------------------------------------------------- stamps

/// The built-in stamp designs.
pub const STAMP_KINDS: [&str; 5] = ["thumbs_up", "heart", "star", "question", "plus_one"];

pub fn stamp_label(kind: &str) -> &'static str {
    match kind {
        "thumbs_up" => "Thumbs up",
        "heart" => "Heart",
        "star" => "Star",
        "question" => "Question",
        "plus_one" => "Plus one",
        _ => "Stamp",
    }
}

/// One drawn layer of a stamp in the unit square (scale by the node size).
#[derive(Clone, Debug, PartialEq)]
pub struct StampLayer {
    pub paths: Vec<Path>,
    pub fill: Option<Color>,
    /// Stroke color and weight (fraction of the stamp size); round caps and joins.
    pub stroke: Option<(Color, f32)>,
}

fn v(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

/// An ellipse as four cubic arcs.
pub fn ellipse_path(cx: f32, cy: f32, rx: f32, ry: f32) -> Path {
    const K: f32 = 0.552_284_8;
    let cmds = vec![
        PathCmd::MoveTo(v(cx + rx, cy)),
        PathCmd::CubicTo(v(cx + rx, cy + ry * K), v(cx + rx * K, cy + ry), v(cx, cy + ry)),
        PathCmd::CubicTo(v(cx - rx * K, cy + ry), v(cx - rx, cy + ry * K), v(cx - rx, cy)),
        PathCmd::CubicTo(v(cx - rx, cy - ry * K), v(cx - rx * K, cy - ry), v(cx, cy - ry)),
        PathCmd::CubicTo(v(cx + rx * K, cy - ry), v(cx + rx, cy - ry * K), v(cx + rx, cy)),
        PathCmd::Close,
    ];
    Path { cmds, fill_rule: FillRule::NonZero }
}

fn rect_path(x: f32, y: f32, w: f32, h: f32) -> Path {
    Path { cmds: polygon_cmds(&[v(x, y), v(x + w, y), v(x + w, y + h), v(x, y + h)]), fill_rule: FillRule::NonZero }
}

fn poly_path(pts: &[Vec2]) -> Path {
    Path { cmds: polygon_cmds(pts), fill_rule: FillRule::NonZero }
}

fn line_path(pts: &[Vec2]) -> Path {
    let cmds = pts.iter().enumerate().map(|(i, p)| if i == 0 { PathCmd::MoveTo(*p) } else { PathCmd::LineTo(*p) }).collect();
    Path { cmds, fill_rule: FillRule::NonZero }
}

/// The artwork of a stamp, bottom layer first. The first layer is always the round badge.
/// Unknown kinds draw an empty badge.
pub fn stamp_layers(kind: &str) -> Vec<StampLayer> {
    let white = Color::WHITE;
    let badge =
        |c: u32| StampLayer { paths: vec![ellipse_path(0.5, 0.5, 0.5, 0.5)], fill: Some(Color::rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)), stroke: None };
    let fill = |paths: Vec<Path>| StampLayer { paths, fill: Some(white), stroke: None };
    match kind {
        "heart" => {
            let heart = Path {
                cmds: vec![
                    PathCmd::MoveTo(v(0.5, 0.74)),
                    PathCmd::CubicTo(v(0.2, 0.55), v(0.22, 0.28), v(0.38, 0.27)),
                    PathCmd::CubicTo(v(0.45, 0.265), v(0.49, 0.31), v(0.5, 0.36)),
                    PathCmd::CubicTo(v(0.51, 0.31), v(0.55, 0.265), v(0.62, 0.27)),
                    PathCmd::CubicTo(v(0.78, 0.28), v(0.8, 0.55), v(0.5, 0.74)),
                    PathCmd::Close,
                ],
                fill_rule: FillRule::NonZero,
            };
            vec![badge(0xF2_4E_5E), fill(vec![heart])]
        }
        "star" => {
            let mut pts = Vec::new();
            for i in 0..10 {
                let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                let r = if i % 2 == 0 { 0.3 } else { 0.125 };
                pts.push(v(0.5 + r * a.cos(), 0.53 + r * a.sin()));
            }
            vec![badge(0xFF_A8_00), fill(vec![poly_path(&pts)])]
        }
        "question" => {
            let hook = line_path(&[v(0.37, 0.38), v(0.4, 0.31), v(0.5, 0.27), v(0.6, 0.31), v(0.63, 0.39), v(0.58, 0.46), v(0.5, 0.52), v(0.5, 0.6)]);
            vec![
                badge(0x84_5E_F7),
                StampLayer { paths: vec![hook], fill: None, stroke: Some((white, 0.075)) },
                fill(vec![ellipse_path(0.5, 0.72, 0.047, 0.047)]),
            ]
        }
        "plus_one" => {
            let plus = vec![rect_path(0.2, 0.465, 0.26, 0.07), rect_path(0.295, 0.37, 0.07, 0.26)];
            let one = line_path(&[v(0.58, 0.42), v(0.67, 0.34), v(0.67, 0.66)]);
            vec![badge(0x1F_B3_6B), fill(plus), StampLayer { paths: vec![one], fill: None, stroke: Some((white, 0.07)) }]
        }
        // thumbs up
        "thumbs_up" => {
            let cuff = rect_path(0.24, 0.45, 0.12, 0.27);
            let hand = Path {
                cmds: vec![
                    PathCmd::MoveTo(v(0.4, 0.46)),
                    PathCmd::LineTo(v(0.47, 0.46)),
                    PathCmd::CubicTo(v(0.52, 0.4), v(0.52, 0.32), v(0.52, 0.28)),
                    PathCmd::CubicTo(v(0.52, 0.25), v(0.55, 0.245), v(0.57, 0.27)),
                    PathCmd::CubicTo(v(0.6, 0.31), v(0.58, 0.4), v(0.57, 0.44)),
                    PathCmd::LineTo(v(0.7, 0.44)),
                    PathCmd::CubicTo(v(0.75, 0.44), v(0.77, 0.47), v(0.76, 0.51)),
                    PathCmd::LineTo(v(0.72, 0.67)),
                    PathCmd::CubicTo(v(0.71, 0.71), v(0.69, 0.72), v(0.65, 0.72)),
                    PathCmd::LineTo(v(0.4, 0.72)),
                    PathCmd::Close,
                ],
                fill_rule: FillRule::NonZero,
            };
            vec![badge(0x2D_7F_F9), fill(vec![cuff, hand])]
        }
        _ => vec![badge(0x80_80_80)],
    }
}

// ------------------------------------------------------------------- routing

/// Distance an elbow route leaves a node before turning, and the clearance it keeps.
pub const ELBOW_STUB: f32 = 24.0;
const BEND_COST: f32 = 40.0;

/// One resolved end of a connector, in the connector's parent space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    pub pos: Vec2,
    /// Unit vector out of the node at `pos`; zero for a free end or a center magnet.
    pub dir: Vec2,
    /// Bounding box of the attached node, to route around.
    pub rect: Option<Rect>,
}

/// The routed line of a connector.
#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    /// Polyline for elbowed and straight lines (for curved lines: just both ends).
    pub pts: Vec<Vec2>,
    /// Control points of a curved line (start, c1, c2, end).
    pub curve: Option<[Vec2; 4]>,
}

fn sub(a: Vec2, b: Vec2) -> Vec2 {
    v(a.x - b.x, a.y - b.y)
}
fn add(a: Vec2, b: Vec2) -> Vec2 {
    v(a.x + b.x, a.y + b.y)
}
fn mul(a: Vec2, k: f32) -> Vec2 {
    v(a.x * k, a.y * k)
}
fn len(a: Vec2) -> f32 {
    (a.x * a.x + a.y * a.y).sqrt()
}
fn unit(a: Vec2) -> Vec2 {
    let l = len(a);
    if l < 1e-6 {
        Vec2::ZERO
    } else {
        mul(a, 1.0 / l)
    }
}

fn gap_between(a: Rect, b: Rect) -> (f32, f32) {
    let gx = (b.x - (a.x + a.w)).max(a.x - (b.x + b.w));
    let gy = (b.y - (a.y + a.h)).max(a.y - (b.y + b.h));
    (gx, gy)
}

/// The side of `r` facing `p`.
pub fn side_facing(r: Rect, p: Vec2) -> Magnet {
    let c = v(r.x + r.w / 2.0, r.y + r.h / 2.0);
    let dx = (p.x - c.x) / (r.w / 2.0).max(1e-3);
    let dy = (p.y - c.y) / (r.h / 2.0).max(1e-3);
    if dx.abs() >= dy.abs() {
        if dx >= 0.0 {
            Magnet::Right
        } else {
            Magnet::Left
        }
    } else if dy >= 0.0 {
        Magnet::Bottom
    } else {
        Magnet::Top
    }
}

/// Sides two boxes connect at when both ends are `Auto`: the facing sides of the axis
/// with the larger gap, or along the dominant center offset when they overlap.
pub fn auto_sides(a: Rect, b: Rect) -> (Magnet, Magnet) {
    let (gx, gy) = gap_between(a, b);
    let (ca, cb) = (v(a.x + a.w / 2.0, a.y + a.h / 2.0), v(b.x + b.w / 2.0, b.y + b.h / 2.0));
    let horizontal = if gx > 0.0 || gy > 0.0 {
        gx >= gy
    } else {
        (cb.x - ca.x).abs() / ((a.w + b.w) / 2.0).max(1e-3) >= (cb.y - ca.y).abs() / ((a.h + b.h) / 2.0).max(1e-3)
    };
    if horizontal {
        if cb.x >= ca.x {
            (Magnet::Right, Magnet::Left)
        } else {
            (Magnet::Left, Magnet::Right)
        }
    } else if cb.y >= ca.y {
        (Magnet::Bottom, Magnet::Top)
    } else {
        (Magnet::Top, Magnet::Bottom)
    }
}

/// Resolves both ends of a connector given the (parent-space) boxes of the nodes they
/// attach to (`None` for a free end at the given point).
pub fn resolve_anchors(start: (Option<Rect>, Magnet, Vec2), end: (Option<Rect>, Magnet, Vec2)) -> (Anchor, Anchor) {
    let center = |r: Rect| v(r.x + r.w / 2.0, r.y + r.h / 2.0);
    let (sm, em) = match (start.0, end.0) {
        (Some(a), Some(b)) => {
            let (da, db) = auto_sides(a, b);
            (if start.1 == Magnet::Auto { da } else { start.1 }, if end.1 == Magnet::Auto { db } else { end.1 })
        }
        (Some(a), None) => (if start.1 == Magnet::Auto { side_facing(a, end.2) } else { start.1 }, end.1),
        (None, Some(b)) => (start.1, if end.1 == Magnet::Auto { side_facing(b, start.2) } else { end.1 }),
        (None, None) => (start.1, end.1),
    };
    let mk = |e: (Option<Rect>, Magnet, Vec2), m: Magnet, other: Vec2| match e.0 {
        Some(r) => Anchor { pos: m.point_on(r), dir: m.outward(), rect: Some(r) },
        None => {
            // a free end enters facing the other end along the dominant axis
            let d = sub(other, e.2);
            let dir = if d.x.abs() >= d.y.abs() { v(-d.x.signum(), 0.0) } else { v(0.0, -d.y.signum()) };
            Anchor { pos: e.2, dir: if len(d) < 1e-3 { Vec2::ZERO } else { dir }, rect: None }
        }
    };
    let sp = start.0.map(|r| m_point(sm, r)).unwrap_or(start.2);
    let ep = end.0.map(|r| m_point(em, r)).unwrap_or(end.2);
    let _ = center;
    (mk(start, sm, ep), mk(end, em, sp))
}

fn m_point(m: Magnet, r: Rect) -> Vec2 {
    m.point_on(r)
}

/// Routes a line between two anchors.
pub fn route(line: ConnectorLine, a: &Anchor, b: &Anchor) -> Route {
    match line {
        ConnectorLine::Straight => Route { pts: vec![a.pos, b.pos], curve: None },
        ConnectorLine::Curved => {
            let d = len(sub(b.pos, a.pos));
            let k = (d * 0.4).clamp(24.0, 160.0);
            let (da, db) = (curve_dir(a, b.pos), curve_dir(b, a.pos));
            let c1 = add(a.pos, mul(da, k));
            let c2 = add(b.pos, mul(db, k));
            Route { pts: vec![a.pos, b.pos], curve: Some([a.pos, c1, c2, b.pos]) }
        }
        ConnectorLine::Elbowed => Route { pts: elbow(a, b), curve: None },
    }
}

fn curve_dir(a: &Anchor, other: Vec2) -> Vec2 {
    if len(a.dir) > 0.5 {
        a.dir
    } else {
        unit(sub(other, a.pos))
    }
}

#[derive(Clone, Copy)]
struct Box4 {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
}

fn inflate(r: Rect, m: f32) -> Box4 {
    Box4 { x0: r.x - m, y0: r.y - m, x1: r.x + r.w + m, y1: r.y + r.h + m }
}

/// Whether the axis-aligned segment a-b passes through the open interior of `b4`.
fn seg_blocked(a: Vec2, b: Vec2, o: &Box4) -> bool {
    const E: f32 = 1e-3;
    if (a.y - b.y).abs() < E {
        let y = a.y;
        y > o.y0 + E && y < o.y1 - E && a.x.min(b.x) < o.x1 - E && a.x.max(b.x) > o.x0 + E
    } else {
        let x = a.x;
        x > o.x0 + E && x < o.x1 - E && a.y.min(b.y) < o.y1 - E && a.y.max(b.y) > o.y0 + E
    }
}

fn dir_index(d: Vec2) -> Option<usize> {
    if d.x > 0.5 {
        Some(0)
    } else if d.y > 0.5 {
        Some(1)
    } else if d.x < -0.5 {
        Some(2)
    } else if d.y < -0.5 {
        Some(3)
    } else {
        None
    }
}

#[cfg(test)]
const DIRS: [(i32, i32); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];

#[cfg(test)]
fn dedup_sorted(mut v: Vec<f32>) -> Vec<f32> {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    v.dedup_by(|a, b| (*a - *b).abs() < 0.01);
    v
}

fn nearest(v: &[f32], x: f32) -> usize {
    v.iter().enumerate().min_by(|a, b| (a.1 - x).abs().partial_cmp(&(b.1 - x).abs()).unwrap_or(std::cmp::Ordering::Equal)).map(|(i, _)| i).unwrap_or(0)
}

const MAX_ROUTE_COORD: f32 = 1e9;

fn valid_coord(x: f32) -> bool {
    x.is_finite() && x.abs() <= MAX_ROUTE_COORD
}

fn sanitize_coord(x: f32) -> f32 {
    if x.is_finite() {
        x.clamp(-MAX_ROUTE_COORD, MAX_ROUTE_COORD)
    } else {
        0.0
    }
}

fn valid_rect(r: Rect) -> bool {
    valid_coord(r.x) && valid_coord(r.y) && valid_coord(r.w) && valid_coord(r.h)
}

const MAX_GRID_COORDS: usize = 32;
const MAX_GRID_STATES: usize = MAX_GRID_COORDS * MAX_GRID_COORDS * 5;

#[inline(always)]
fn dedup_in_place(v: &mut Vec<f32>) {
    v.sort_unstable_by(|a, b| a.total_cmp(b));
    v.dedup_by(|a, b| (*a - *b).abs() < 0.01);
}

#[inline(always)]
fn add_mid_lanes_in_place(v: &mut Vec<f32>) {
    dedup_in_place(v);
    let n = v.len();
    for k in 0..n.saturating_sub(1) {
        let mid = (v[k] + v[k + 1]) * 0.5;
        v.push(mid);
    }
    dedup_in_place(v);
}

#[inline(always)]
fn pack_heap_entry(cost: f32, state: usize) -> u64 {
    ((cost.to_bits() as u64) << 32) | (state as u32 as u64)
}

#[inline(always)]
fn heap_push(heap: &mut [u64; MAX_GRID_STATES], heap_len: &mut usize, item: u64) {
    let mut idx = *heap_len;
    *heap_len = idx + 1;
    while idx > 0 {
        let parent = (idx - 1) >> 1;
        if item < heap[parent] {
            heap[idx] = heap[parent];
            idx = parent;
        } else {
            break;
        }
    }
    heap[idx] = item;
}

#[inline(always)]
fn heap_pop(heap: &mut [u64; MAX_GRID_STATES], heap_len: &mut usize) -> Option<u64> {
    let len = *heap_len;
    if len == 0 {
        return None;
    }
    let top = heap[0];
    let new_len = len - 1;
    *heap_len = new_len;
    if new_len > 0 {
        let last = heap[new_len];
        let mut idx = 0usize;
        loop {
            let left = (idx << 1) + 1;
            if left >= new_len {
                break;
            }
            let right = left + 1;
            let smallest = if right < new_len && heap[right] < heap[left] { right } else { left };
            if heap[smallest] < last {
                heap[idx] = heap[smallest];
                idx = smallest;
            } else {
                break;
            }
        }
        heap[idx] = last;
    }
    Some(top)
}

/// Orthogonal route that leaves and enters the nodes straight and does not cross their boxes.
fn elbow(a: &Anchor, b: &Anchor) -> Vec<Vec2> {
    elbow_with_steps(a, b).0
}

fn elbow_with_steps(a: &Anchor, b: &Anchor) -> (Vec<Vec2>, usize) {
    if !valid_coord(a.pos.x)
        || !valid_coord(a.pos.y)
        || !valid_coord(b.pos.x)
        || !valid_coord(b.pos.y)
        || !valid_coord(a.dir.x)
        || !valid_coord(a.dir.y)
        || !valid_coord(b.dir.x)
        || !valid_coord(b.dir.y)
        || !a.rect.is_none_or(valid_rect)
        || !b.rect.is_none_or(valid_rect)
    {
        return (vec![v(sanitize_coord(a.pos.x), sanitize_coord(a.pos.y)), v(sanitize_coord(b.pos.x), sanitize_coord(b.pos.y))], 0);
    }
    let rect_a = a.rect;
    let rect_b = b.rect;

    // clearance kept around the boxes: half of a small gap, at most one stub
    let clear = match (rect_a, rect_b) {
        (Some(ra), Some(rb)) => {
            let (gx, gy) = gap_between(ra, rb);
            let g = gx.max(gy);
            if g > 0.0 {
                (g / 2.0).clamp(4.0, ELBOW_STUB)
            } else {
                ELBOW_STUB / 2.0
            }
        }
        _ => ELBOW_STUB,
    };
    let stub_a = if rect_a.is_some() { clear } else { 0.0 };
    let stub_b = if rect_b.is_some() { clear } else { 0.0 };
    let p0 = add(a.pos, mul(a.dir, stub_a));
    let p1 = add(b.pos, mul(b.dir, stub_b));
    let mut obstacles = [Box4 { x0: 0.0, y0: 0.0, x1: 0.0, y1: 0.0 }; 2];
    let mut n_obs = 0usize;
    if let Some(ra) = rect_a {
        obstacles[n_obs] = inflate(ra, clear);
        n_obs += 1;
    }
    if let Some(rb) = rect_b {
        obstacles[n_obs] = inflate(rb, clear);
        n_obs += 1;
    }
    let obs = &obstacles[..n_obs];
    let mid = v((p0.x + p1.x) / 2.0, (p0.y + p1.y) / 2.0);
    let want = dir_index(v(-b.dir.x, -b.dir.y));
    let start_dir = dir_index(a.dir).unwrap_or(4);

    let mut xs = Vec::with_capacity(MAX_GRID_COORDS);
    let mut ys = Vec::with_capacity(MAX_GRID_COORDS);
    xs.extend_from_slice(&[p0.x, p1.x, mid.x, a.pos.x, b.pos.x, p0.x + a.dir.x * clear, p1.x + b.dir.x * clear]);
    ys.extend_from_slice(&[p0.y, p1.y, mid.y, a.pos.y, b.pos.y, p0.y + a.dir.y * clear, p1.y + b.dir.y * clear]);
    for o in obs {
        xs.extend_from_slice(&[o.x0, o.x1, o.x0 - clear, o.x1 + clear]);
        ys.extend_from_slice(&[o.y0, o.y1, o.y0 - clear, o.y1 + clear]);
    }
    add_mid_lanes_in_place(&mut xs);
    add_mid_lanes_in_place(&mut ys);

    let (nx, ny) = (xs.len(), ys.len());
    debug_assert!(nx <= MAX_GRID_COORDS && ny <= MAX_GRID_COORDS);
    let (i0, j0) = (nearest(&xs, p0.x), nearest(&ys, p0.y));
    let (i1, j1) = (nearest(&xs, p1.x), nearest(&ys, p1.y));

    let mut dx_len = [0.0f32; MAX_GRID_COORDS];
    let mut dy_len = [0.0f32; MAX_GRID_COORDS];
    let mut bend_x = [0.0f32; MAX_GRID_COORDS];
    let mut bend_y = [0.0f32; MAX_GRID_COORDS];
    let mut h_blocked = [0u32; MAX_GRID_COORDS];
    let mut v_blocked = [0u32; MAX_GRID_COORDS];

    for i in 0..nx {
        bend_x[i] = 0.01 * (xs[i] - mid.x).abs();
        if i + 1 < nx {
            let d = xs[i + 1] - xs[i];
            dx_len[i] = (d * d).sqrt();
        }
    }
    for j in 0..ny {
        bend_y[j] = BEND_COST + 0.01 * (ys[j] - mid.y).abs();
        if j + 1 < ny {
            let d = ys[j + 1] - ys[j];
            dy_len[j] = (d * d).sqrt();
        }
    }
    const E: f32 = 1e-3;
    for o in obs {
        let mut y_open = 0u32;
        let mut y_vseg = 0u32;
        for j in 0..ny {
            let y = ys[j];
            if y > o.y0 + E && y < o.y1 - E {
                y_open |= 1u32 << j;
            }
            if j + 1 < ny && y < o.y1 - E && ys[j + 1] > o.y0 + E {
                y_vseg |= 1u32 << j;
            }
        }
        for i in 0..nx {
            let x = xs[i];
            if i + 1 < nx && x < o.x1 - E && xs[i + 1] > o.x0 + E {
                h_blocked[i] |= y_open;
            }
            if x > o.x0 + E && x < o.x1 - E {
                v_blocked[i] |= y_vseg;
            }
        }
    }

    let n_states = nx * ny * 5;
    let mut dist = [f32::INFINITY; MAX_GRID_STATES];
    let mut prev = [u16::MAX; MAX_GRID_STATES];
    let mut done = [0u64; MAX_GRID_STATES.div_ceil(64)];
    let mut heap = [0u64; MAX_GRID_STATES];
    let mut heap_len = 0usize;

    let start_state = (i0 * ny + j0) * 5 + start_dir;
    dist[start_state] = 0.0;
    heap_push(&mut heap, &mut heap_len, pack_heap_entry(0.0, start_state));

    let mut found = None;
    let mut steps = 0usize;
    let stride_x = ny * 5;

    while let Some(entry) = heap_pop(&mut heap, &mut heap_len) {
        let best = (entry as u32) as usize;
        let done_word = best >> 6;
        let done_bit = 1u64 << (best & 63);
        if (done[done_word] & done_bit) != 0 {
            continue;
        }
        steps += 1;
        done[done_word] |= done_bit;

        let (cell_idx, d) = (best / 5, best % 5);
        let (i, j) = (cell_idx / ny, cell_idx % ny);
        if i == i1 && j == j1 && want.is_none_or(|w| w == d || d == 4) {
            found = Some(best);
            break;
        }
        let base = cell_idx * 5;
        let best_dist = dist[best];
        let bend_extra = bend_x[i] + bend_y[j];

        // nd = 0: (+1, 0)
        if d != 2 && i + 1 < nx && ((h_blocked[i] >> j) & 1) == 0 {
            let ns = base + stride_x;
            if (done[ns >> 6] & (1u64 << (ns & 63))) == 0 {
                let mut c = best_dist + dx_len[i];
                if d != 4 && d != 0 {
                    c += bend_extra;
                }
                if c < dist[ns] - 1e-4 {
                    dist[ns] = c;
                    prev[ns] = best as u16;
                    heap_push(&mut heap, &mut heap_len, pack_heap_entry(c, ns));
                }
            }
        }
        // nd = 1: (0, +1)
        if d != 3 && j + 1 < ny && ((v_blocked[i] >> j) & 1) == 0 {
            let ns = base + 5 + 1;
            if (done[ns >> 6] & (1u64 << (ns & 63))) == 0 {
                let mut c = best_dist + dy_len[j];
                if d != 4 && d != 1 {
                    c += bend_extra;
                }
                if c < dist[ns] - 1e-4 {
                    dist[ns] = c;
                    prev[ns] = best as u16;
                    heap_push(&mut heap, &mut heap_len, pack_heap_entry(c, ns));
                }
            }
        }
        // nd = 2: (-1, 0)
        if d != 0 && i > 0 && ((h_blocked[i - 1] >> j) & 1) == 0 {
            let ns = base - stride_x + 2;
            if (done[ns >> 6] & (1u64 << (ns & 63))) == 0 {
                let mut c = best_dist + dx_len[i - 1];
                if d != 4 && d != 2 {
                    c += bend_extra;
                }
                if c < dist[ns] - 1e-4 {
                    dist[ns] = c;
                    prev[ns] = best as u16;
                    heap_push(&mut heap, &mut heap_len, pack_heap_entry(c, ns));
                }
            }
        }
        // nd = 3: (0, -1)
        if d != 1 && j > 0 && ((v_blocked[i] >> (j - 1)) & 1) == 0 {
            let ns = base - 5 + 3;
            if (done[ns >> 6] & (1u64 << (ns & 63))) == 0 {
                let mut c = best_dist + dy_len[j - 1];
                if d != 4 && d != 3 {
                    c += bend_extra;
                }
                if c < dist[ns] - 1e-4 {
                    dist[ns] = c;
                    prev[ns] = best as u16;
                    heap_push(&mut heap, &mut heap_len, pack_heap_entry(c, ns));
                }
            }
        }
    }

    let _ = n_states;
    let mut pts = Vec::with_capacity(8);
    pts.push(a.pos);
    match found {
        Some(mut st) => {
            let base_len = pts.len();
            while st != u16::MAX as usize {
                let cell_idx = st / 5;
                pts.push(v(xs[cell_idx / ny], ys[cell_idx % ny]));
                st = prev[st] as usize;
            }
            pts[base_len..].reverse();
        }
        None => {
            pts.push(p0);
            if a.dir.x.abs() > 0.5 || (a.dir == Vec2::ZERO && (p1.x - p0.x).abs() >= (p1.y - p0.y).abs()) {
                pts.push(v(mid.x, p0.y));
                pts.push(v(mid.x, p1.y));
            } else {
                pts.push(v(p0.x, mid.y));
                pts.push(v(p1.x, mid.y));
            }
            pts.push(p1);
        }
    }
    pts.push(b.pos);
    (simplify(pts), steps)
}

/// Drops repeated and collinear points.
fn simplify(pts: Vec<Vec2>) -> Vec<Vec2> {
    let mut out: Vec<Vec2> = Vec::with_capacity(pts.len());
    for p in pts {
        if out.last().is_some_and(|l| len(sub(*l, p)) < 0.01) {
            continue;
        }
        while out.len() >= 2 {
            let (a, b) = (out[out.len() - 2], out[out.len() - 1]);
            let cross = (b.x - a.x) * (p.y - b.y) - (b.y - a.y) * (p.x - b.x);
            let dot = (b.x - a.x) * (p.x - b.x) + (b.y - a.y) * (p.y - b.y);
            if cross.abs() < 0.01 && dot > 0.0 {
                out.pop();
            } else {
                break;
            }
        }
        out.push(p);
    }
    out
}

fn cubic_at(c: &[Vec2; 4], t: f32) -> Vec2 {
    let u = 1.0 - t;
    v(
        u * u * u * c[0].x + 3.0 * u * u * t * c[1].x + 3.0 * u * t * t * c[2].x + t * t * t * c[3].x,
        u * u * u * c[0].y + 3.0 * u * u * t * c[1].y + 3.0 * u * t * t * c[2].y + t * t * t * c[3].y,
    )
}

impl Route {
    /// The route as a polyline (curves flattened).
    pub fn polyline(&self) -> Vec<Vec2> {
        match &self.curve {
            Some(c) => (0..=32).map(|i| cubic_at(c, i as f32 / 32.0)).collect(),
            None => self.pts.clone(),
        }
    }

    pub fn path(&self) -> Path {
        match &self.curve {
            Some(c) => Path { cmds: vec![PathCmd::MoveTo(c[0]), PathCmd::CubicTo(c[1], c[2], c[3])], fill_rule: FillRule::NonZero },
            None => line_path(&self.pts),
        }
    }

    pub fn translated(&self, d: Vec2) -> Route {
        Route { pts: self.pts.iter().map(|p| add(*p, d)).collect(), curve: self.curve.map(|c| c.map(|p| add(p, d))) }
    }

    pub fn start(&self) -> Vec2 {
        self.pts.first().copied().unwrap_or(Vec2::ZERO)
    }
    pub fn end(&self) -> Vec2 {
        self.pts.last().copied().unwrap_or(Vec2::ZERO)
    }

    /// Unit direction of travel leaving the start.
    pub fn start_tangent(&self) -> Vec2 {
        match &self.curve {
            Some(c) => unit(sub(c[1], c[0])),
            None => match self.pts.as_slice() {
                [a, b, ..] => unit(sub(*b, *a)),
                _ => v(1.0, 0.0),
            },
        }
    }

    /// Unit direction of travel arriving at the end.
    pub fn end_tangent(&self) -> Vec2 {
        match &self.curve {
            Some(c) => unit(sub(c[3], c[2])),
            None => match self.pts.as_slice() {
                [.., a, b] => unit(sub(*b, *a)),
                _ => v(1.0, 0.0),
            },
        }
    }

    /// Bounding box of the line.
    pub fn bounds(&self) -> Rect {
        let pl = self.polyline();
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in &pl {
            x0 = x0.min(p.x);
            y0 = y0.min(p.y);
            x1 = x1.max(p.x);
            y1 = y1.max(p.y);
        }
        if x0 > x1 {
            return Rect::default();
        }
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }

    /// The point halfway along the line (where the label sits).
    pub fn midpoint(&self) -> Vec2 {
        let pl = self.polyline();
        let total: f32 = pl.windows(2).map(|w| len(sub(w[1], w[0]))).sum();
        let mut left = total / 2.0;
        for w in pl.windows(2) {
            let l = len(sub(w[1], w[0]));
            if l >= left && l > 0.0 {
                let t = left / l;
                return v(w[0].x + (w[1].x - w[0].x) * t, w[0].y + (w[1].y - w[0].y) * t);
            }
            left -= l;
        }
        pl.last().copied().unwrap_or(Vec2::ZERO)
    }

    /// Distance from `p` to the line.
    pub fn distance_to(&self, p: Vec2) -> f32 {
        let pl = self.polyline();
        let mut best = f32::MAX;
        for w in pl.windows(2) {
            let (a, b) = (w[0], w[1]);
            let ab = sub(b, a);
            let l2 = ab.x * ab.x + ab.y * ab.y;
            let t = if l2 < 1e-9 { 0.0 } else { (((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / l2).clamp(0.0, 1.0) };
            let q = v(a.x + ab.x * t, a.y + ab.y * t);
            best = best.min(len(sub(p, q)));
        }
        if pl.len() == 1 {
            best = len(sub(p, pl[0]));
        }
        best
    }

    /// Whether any segment passes through the open interior of `r`.
    pub fn crosses_rect(&self, r: Rect) -> bool {
        let o = Box4 { x0: r.x, y0: r.y, x1: r.x + r.w, y1: r.y + r.h };
        self.pts.windows(2).any(|w| seg_blocked(w[0], w[1], &o))
    }
}

/// Size of a connector end cap for a stroke of `weight`.
pub fn cap_size(weight: f32) -> f32 {
    (weight * 4.0).max(12.0)
}

/// Outline of an end cap whose tip is at `tip`, `dir` the direction the line travels into
/// it. `Arrow` is an open chevron (stroke it); the other caps are closed shapes (fill them).
pub fn cap_path(cap: ConnectorCap, tip: Vec2, dir: Vec2, weight: f32) -> Option<Path> {
    let s = cap_size(weight);
    let d = if len(dir) < 0.5 { v(1.0, 0.0) } else { unit(dir) };
    let n = v(-d.y, d.x);
    let at = |along: f32, across: f32| add(tip, add(mul(d, along), mul(n, across)));
    match cap {
        ConnectorCap::None => None,
        ConnectorCap::Arrow => Some(line_path(&[at(-s, s * 0.6), at(0.0, 0.0), at(-s, -s * 0.6)])),
        ConnectorCap::Triangle => Some(poly_path(&[at(0.0, 0.0), at(-s, s * 0.6), at(-s, -s * 0.6)])),
        ConnectorCap::Diamond => Some(poly_path(&[at(0.0, 0.0), at(-s * 0.6, s * 0.45), at(-s * 1.2, 0.0), at(-s * 0.6, -s * 0.45)])),
        ConnectorCap::Circle => {
            let c = at(-s * 0.4, 0.0);
            Some(ellipse_path(c.x, c.y, s * 0.4, s * 0.4))
        }
    }
}

// ------------------------------------------------------------- document glue

/// Bounding box (in the parent's space) of `id`'s transformed local box.
fn rect_in_parent(doc: &Document, id: NodeId, parent_inv: Affine) -> Option<Rect> {
    let n = doc.get(id)?;
    let resolved = resolve_node(doc, n);
    let m = parent_inv * doc.world_transform(id);
    let corners = [m.apply(v(0.0, 0.0)), m.apply(v(resolved.size.x, 0.0)), m.apply(v(resolved.size.x, resolved.size.y)), m.apply(v(0.0, resolved.size.y))];
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for c in corners {
        x0 = x0.min(c.x);
        y0 = y0.min(c.y);
        x1 = x1.max(c.x);
        y1 = y1.max(c.y);
    }
    Some(Rect::new(x0, y0, x1 - x0, y1 - y0))
}

/// Approximate width of a connector label (for bounds; the renderer measures exactly).
pub fn label_size_estimate(label: &TextProps) -> Vec2 {
    let fs = label.style.font_size.max(1.0);
    let lines: Vec<&str> = label.content.split('\n').collect();
    let w = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as f32 * fs * 0.6;
    Vec2::new(w + fs, lines.len() as f32 * fs * 1.3 + fs * 0.4)
}

/// Resolved anchors of connector `id` in its parent's space.
pub fn connector_anchors(doc: &Document, id: NodeId) -> Option<(Anchor, Anchor)> {
    let n = doc.get(id)?;
    let NodeKind::Connector(c) = &n.kind else { return None };
    let pinv = n.parent.map(|p| doc.world_transform(p)).unwrap_or(Affine::IDENTITY).invert().unwrap_or(Affine::IDENTITY);
    let end_of = |e: &ConnectorEnd| -> (Option<Rect>, Magnet, Vec2) {
        let r = e.node.filter(|t| *t != id).and_then(|t| rect_in_parent(doc, t, pinv));
        (r, e.magnet, e.point)
    };
    Some(resolve_anchors(end_of(&c.start), end_of(&c.end)))
}

/// The route of connector `id` in its parent's space.
pub fn connector_route(doc: &Document, id: NodeId) -> Option<Route> {
    crate::connector_route_cache::cached_connector_route(doc, id)
}

/// Bounds of the connector's drawn extent (line, caps and label) given its stroke weight.
pub fn connector_extent(route: &Route, props: &ConnectorProps, weight: f32) -> Rect {
    let mut r = route.bounds();
    let pad = cap_size(weight) * 1.3 + weight;
    r = Rect::new(r.x - pad, r.y - pad, r.w + 2.0 * pad, r.h + 2.0 * pad);
    if let Some(l) = &props.label {
        if !l.content.is_empty() {
            let s = label_size_estimate(l);
            let m = route.midpoint();
            r = r.union(&Rect::new(m.x - s.x / 2.0, m.y - s.y / 2.0, s.x, s.y));
        }
    }
    r
}

/// Where connector `id` sits after its ends moved: new transform and size (a pure
/// translation covering the line, caps and label) plus the resolved end points.
#[derive(Clone, Debug, PartialEq)]
pub struct ConnectorGeom {
    pub transform: Affine,
    pub size: Vec2,
    pub start_point: Vec2,
    pub end_point: Vec2,
}

pub fn connector_geometry(doc: &Document, id: NodeId) -> Option<ConnectorGeom> {
    let n = doc.get(id)?;
    let NodeKind::Connector(c) = &n.kind else { return None };
    let route = connector_route(doc, id)?;
    let ext = connector_extent(&route, c, n.stroke.weight);
    Some(ConnectorGeom {
        transform: Affine::translate(ext.x, ext.y),
        size: Vec2::new(ext.w.max(1.0), ext.h.max(1.0)),
        start_point: route.start(),
        end_point: route.end(),
    })
}

impl ConnectorGeom {
    /// Writes this placement into the connector node `n`.
    pub fn apply(&self, n: &mut Node) {
        n.transform = self.transform;
        n.size = self.size;
        if let NodeKind::Connector(c) = &mut n.kind {
            c.start.point = self.start_point;
            c.end.point = self.end_point;
        }
    }
}

/// Re-derives transform, size and stored end points of connector `id` from its ends.
pub fn sync_connector(doc: &mut Document, id: NodeId) -> bool {
    let Some(g) = connector_geometry(doc, id) else { return false };
    let Some(n) = doc.get_mut(id) else { return false };
    g.apply(n);
    true
}

/// Connectors on the page of `of` (or all pages when `None`) with an end attached to one of `nodes`.
pub fn connectors_attached_to(doc: &Document, nodes: &[NodeId]) -> Vec<NodeId> {
    with_index(doc, |idx| {
        let mut out = Vec::new();
        for &n in nodes {
            out.extend_from_slice(idx.connectors_for_end(n));
        }
        out.sort_unstable();
        out.dedup();
        out
    })
}

/// Re-routes every connector attached to (a descendant of) `moved`; returns the touched connectors.
pub fn reroute_attached(doc: &mut Document, moved: &[NodeId]) -> Vec<NodeId> {
    let ids = connectors_touching(doc, moved);
    for &id in &ids {
        sync_connector(doc, id);
    }
    ids
}

impl Node {
    /// A sticky note at `r` (position and size) with `text`.
    pub fn sticky(id: NodeId, r: Rect, text: impl Into<String>) -> Self {
        let mut props = StickyProps::default();
        props.text.content = text.into();
        let mut n = Self::new(id, NodeKind::Sticky(props));
        n.transform = Affine::translate(r.x, r.y);
        n.size = Vec2::new(r.w, r.h);
        n
    }

    /// A shape with centered text.
    pub fn shape_with_text(id: NodeId, r: Rect, shape: ShapeKind, text: impl Into<String>) -> Self {
        let mut props = ShapeWithTextProps { shape, ..Default::default() };
        props.text.content = text.into();
        let mut n = Self::new(id, NodeKind::ShapeWithText(props));
        n.transform = Affine::translate(r.x, r.y);
        n.size = Vec2::new(r.w, r.h);
        n.fills = vec![Paint::solid(Color::WHITE)];
        n.stroke = Stroke {
            paints: vec![Paint::solid(Color::rgb(0x1E, 0x1E, 0x1E))],
            weight: 2.0,
            align: StrokeAlign::Center,
            miter_limit: 4.0,
            ..Default::default()
        };
        n
    }

    /// A connector between two ends; `sync_connector` places it.
    pub fn connector(id: NodeId, props: ConnectorProps) -> Self {
        let mut n = Self::new(id, NodeKind::Connector(props));
        n.stroke = Stroke {
            paints: vec![Paint::solid(Color::rgb(0x1E, 0x1E, 0x1E))],
            weight: 2.0,
            align: StrokeAlign::Center,
            join: StrokeJoin::Round,
            cap: StrokeCap::Round,
            miter_limit: 4.0,
            ..Default::default()
        };
        n
    }

    pub fn stamp(id: NodeId, at: Vec2, kind: &str, size: f32) -> Self {
        let mut n = Self::new(id, NodeKind::Stamp { kind: kind.to_string() });
        n.transform = Affine::translate(at.x, at.y);
        n.size = Vec2::new(size, size);
        n.name = stamp_label(kind).to_string();
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::new(x, y, w, h)
    }

    fn anchors(a: Rect, b: Rect) -> (Anchor, Anchor) {
        resolve_anchors((Some(a), Magnet::Auto, Vec2::ZERO), (Some(b), Magnet::Auto, Vec2::ZERO))
    }

    fn assert_orthogonal(pts: &[Vec2]) {
        for w in pts.windows(2) {
            assert!((w[0].x - w[1].x).abs() < 0.02 || (w[0].y - w[1].y).abs() < 0.02, "diagonal segment {:?} -> {:?}", w[0], w[1]);
        }
    }

    #[test]
    fn magnets_resolve_to_facing_sides() {
        let a = rect(0.0, 0.0, 100.0, 100.0);
        let right = rect(300.0, 20.0, 100.0, 60.0);
        let (pa, pb) = anchors(a, right);
        assert_eq!(pa.pos, v(100.0, 50.0));
        assert_eq!(pa.dir, v(1.0, 0.0));
        assert_eq!(pb.pos, v(300.0, 50.0));
        assert_eq!(pb.dir, v(-1.0, 0.0));
        let below = rect(20.0, 300.0, 60.0, 60.0);
        let (pa, pb) = anchors(a, below);
        assert_eq!((pa.pos, pb.pos), (v(50.0, 100.0), v(50.0, 300.0)));
        // explicit magnets win over auto
        let (pa, _) = resolve_anchors((Some(a), Magnet::Top, Vec2::ZERO), (Some(right), Magnet::Auto, Vec2::ZERO));
        assert_eq!(pa.pos, v(50.0, 0.0));
        assert_eq!(Magnet::Left.point_on(a), v(0.0, 50.0));
        assert_eq!(Magnet::Center.point_on(a), v(50.0, 50.0));
        assert_eq!(side_facing(a, v(400.0, 40.0)), Magnet::Right);
        assert_eq!(side_facing(a, v(60.0, -90.0)), Magnet::Top);
    }

    #[test]
    fn straight_route_joins_the_anchors() {
        let (a, b) = anchors(rect(0.0, 0.0, 100.0, 100.0), rect(300.0, 0.0, 100.0, 100.0));
        let r = route(ConnectorLine::Straight, &a, &b);
        assert_eq!(r.pts, vec![v(100.0, 50.0), v(300.0, 50.0)]);
        assert_eq!(r.start_tangent(), v(1.0, 0.0));
    }

    #[test]
    fn elbow_between_aligned_boxes_is_a_straight_line() {
        let (a, b) = anchors(rect(0.0, 0.0, 100.0, 100.0), rect(300.0, 0.0, 100.0, 100.0));
        let r = route(ConnectorLine::Elbowed, &a, &b);
        assert_eq!(r.pts, vec![v(100.0, 50.0), v(300.0, 50.0)]);
    }

    #[test]
    fn elbow_between_offset_boxes_has_one_z_in_the_gap() {
        let ra = rect(0.0, 0.0, 100.0, 100.0);
        let rb = rect(300.0, 200.0, 100.0, 100.0);
        let (a, b) = anchors(ra, rb);
        let r = route(ConnectorLine::Elbowed, &a, &b);
        assert_orthogonal(&r.pts);
        assert_eq!(r.pts.first(), Some(&v(100.0, 50.0)));
        assert_eq!(r.pts.last(), Some(&v(300.0, 250.0)));
        assert_eq!(r.pts.len(), 4, "{:?}", r.pts);
        // the vertical leg runs in the middle of the gap
        assert!((r.pts[1].x - 200.0).abs() < 1.0, "{:?}", r.pts);
        assert!(!r.crosses_rect(ra) && !r.crosses_rect(rb));
    }

    #[test]
    fn elbow_goes_around_a_box_it_would_cross() {
        // both ends attach on the far sides, so the line has to wrap around the boxes
        let ra = rect(0.0, 0.0, 100.0, 100.0);
        let rb = rect(200.0, 0.0, 100.0, 100.0);
        let (a, b) = resolve_anchors((Some(ra), Magnet::Left, Vec2::ZERO), (Some(rb), Magnet::Right, Vec2::ZERO));
        let r = route(ConnectorLine::Elbowed, &a, &b);
        assert_orthogonal(&r.pts);
        assert!(!r.crosses_rect(ra), "crosses source: {:?}", r.pts);
        assert!(!r.crosses_rect(rb), "crosses target: {:?}", r.pts);
        // leaves a to the left and enters b from the right
        assert!(r.pts[1].x < 0.0, "{:?}", r.pts);
        assert!(r.pts[r.pts.len() - 2].x > 300.0, "{:?}", r.pts);
    }

    #[test]
    fn elbow_same_side_loops_around_without_crossing() {
        let ra = rect(0.0, 0.0, 100.0, 100.0);
        let rb = rect(0.0, 200.0, 100.0, 100.0);
        let (a, b) = resolve_anchors((Some(ra), Magnet::Top, Vec2::ZERO), (Some(rb), Magnet::Top, Vec2::ZERO));
        let r = route(ConnectorLine::Elbowed, &a, &b);
        assert_orthogonal(&r.pts);
        assert!(!r.crosses_rect(ra) && !r.crosses_rect(rb), "{:?}", r.pts);
    }

    #[test]
    fn elbow_to_a_free_end_is_orthogonal_and_avoids_the_source() {
        let ra = rect(0.0, 0.0, 100.0, 100.0);
        let (a, b) = resolve_anchors((Some(ra), Magnet::Auto, Vec2::ZERO), (None, Magnet::Auto, v(-120.0, 260.0)));
        let r = route(ConnectorLine::Elbowed, &a, &b);
        assert_orthogonal(&r.pts);
        assert!(!r.crosses_rect(ra), "{:?}", r.pts);
        assert_eq!(r.end(), v(-120.0, 260.0));
    }

    #[test]
    fn curved_route_leaves_along_the_magnet() {
        let (a, b) = anchors(rect(0.0, 0.0, 100.0, 100.0), rect(300.0, 200.0, 100.0, 100.0));
        let r = route(ConnectorLine::Curved, &a, &b);
        let c = r.curve.unwrap();
        assert_eq!(c[0], v(100.0, 50.0));
        assert_eq!(c[3], v(300.0, 250.0));
        assert!(c[1].x > c[0].x && (c[1].y - c[0].y).abs() < 1e-3);
        assert!(c[2].x < c[3].x && (c[2].y - c[3].y).abs() < 1e-3);
        assert_eq!(r.start_tangent(), v(1.0, 0.0));
        assert_eq!(r.end_tangent(), v(1.0, 0.0));
        let mid = r.midpoint();
        assert!((mid.x - 200.0).abs() < 2.0 && (mid.y - 150.0).abs() < 2.0, "{mid:?}");
    }

    #[test]
    fn route_distance_and_midpoint() {
        let r = Route { pts: vec![v(0.0, 0.0), v(100.0, 0.0), v(100.0, 100.0)], curve: None };
        assert!((r.distance_to(v(50.0, 7.0)) - 7.0).abs() < 1e-4);
        assert!((r.distance_to(v(107.0, 60.0)) - 7.0).abs() < 1e-4);
        assert_eq!(r.midpoint(), v(100.0, 0.0));
        assert_eq!(r.bounds(), Rect::new(0.0, 0.0, 100.0, 100.0));
    }

    #[test]
    fn fit_font_size_is_monotone_and_bounded() {
        // text height grows with size: n chars of 0.5em in a 100 box fit when lines*size <= 100
        let height = |size: f32, chars: f32| {
            let per_line = (100.0 / (size * 0.5)).floor().max(1.0);
            (chars / per_line).ceil() * size * 1.2
        };
        let fit = |chars: f32| fit_font_size(32.0, 8.0, |s| height(s, chars) <= 100.0);
        assert_eq!(fit(1.0), 32.0, "short text keeps the largest size");
        let (a, b, c) = (fit(20.0), fit(60.0), fit(200.0));
        assert!(a > b && b > c, "{a} {b} {c}");
        assert!(height(a, 20.0) <= 100.0 && height(b, 60.0) <= 100.0);
        assert_eq!(fit(100_000.0), 8.0, "never below the minimum");
    }

    #[test]
    fn sticky_layout_reserves_the_author_line() {
        let plain = sticky_layout(v(240.0, 240.0), false);
        let with = sticky_layout(v(240.0, 240.0), true);
        assert!(plain.author_box.is_none() && with.author_box.is_some());
        assert!(with.text_box.h < plain.text_box.h);
        let a = with.author_box.unwrap();
        assert!(a.y >= with.text_box.y + with.text_box.h - 0.01 && a.y + a.h <= 240.0);
    }

    #[test]
    fn stamps_have_a_badge_and_glyph_layers() {
        for k in STAMP_KINDS {
            let layers = stamp_layers(k);
            assert!(layers.len() >= 2, "{k}");
            assert!(layers[0].fill.is_some());
            for l in &layers {
                assert!(l.paths.iter().all(|p| !p.cmds.is_empty()), "{k}");
            }
        }
        assert_eq!(stamp_layers("nonsense").len(), 1);
    }

    #[test]
    fn shape_paths_stay_inside_the_box() {
        for k in ShapeKind::ALL {
            let Some(p) = k.path(200.0, 100.0) else { continue };
            for c in &p.cmds {
                if let PathCmd::MoveTo(q) | PathCmd::LineTo(q) = c {
                    assert!(q.x >= -0.5 && q.x <= 200.5 && q.y >= -0.5 && q.y <= 100.5, "{k:?} {q:?}");
                }
            }
            let (x, y, w, h) = k.text_box();
            assert!(x >= 0.0 && y >= 0.0 && x + w <= 1.0001 && y + h <= 1.0001, "{k:?}");
        }
    }

    #[test]
    fn caps_point_along_the_line() {
        let tip = v(100.0, 0.0);
        let tri = cap_path(ConnectorCap::Triangle, tip, v(1.0, 0.0), 2.0).unwrap();
        let PathCmd::MoveTo(first) = tri.cmds[0] else { panic!() };
        assert_eq!(first, tip);
        assert!(tri.cmds.iter().all(|c| match c {
            PathCmd::MoveTo(p) | PathCmd::LineTo(p) => p.x <= 100.0 + 1e-3,
            _ => true,
        }));
        assert!(cap_path(ConnectorCap::None, tip, v(1.0, 0.0), 2.0).is_none());
    }

    fn doc_with_two() -> (Document, NodeId, NodeId, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let a = d.alloc_id();
        let a = d.insert(page, Node::sticky(a, Rect::new(0.0, 0.0, 100.0, 100.0), "a")).unwrap();
        let b = d.alloc_id();
        let b = d.insert(page, Node::sticky(b, Rect::new(300.0, 200.0, 100.0, 100.0), "b")).unwrap();
        (d, page, a, b)
    }

    #[test]
    fn connector_follows_its_nodes() {
        let (mut d, page, a, b) = doc_with_two();
        let cid = d.alloc_id();
        let props = ConnectorProps { start: ConnectorEnd::attached(a, Magnet::Auto), end: ConnectorEnd::attached(b, Magnet::Auto), ..Default::default() };
        let c = d.insert(page, Node::connector(cid, props)).unwrap();
        assert!(sync_connector(&mut d, c));
        let r = connector_route(&d, c).unwrap();
        assert_eq!(r.start(), v(100.0, 50.0));
        assert_eq!(r.end(), v(300.0, 250.0));
        let n = d.get(c).unwrap();
        // the node box covers the line
        assert!(n.transform.tx <= 100.0 && n.transform.tx + n.size.x >= 300.0);
        // move b and re-route
        d.get_mut(b).unwrap().transform = Affine::translate(300.0, 400.0);
        let touched = reroute_attached(&mut d, &[b]);
        assert_eq!(touched, vec![c]);
        let r = connector_route(&d, c).unwrap();
        // the nodes are now further apart vertically, so the line connects bottom to top
        assert_eq!(r.start(), v(50.0, 100.0));
        assert_eq!(r.end(), v(350.0, 400.0));
        let NodeKind::Connector(p) = &d.get(c).unwrap().kind else { panic!() };
        assert_eq!(p.end.point, v(350.0, 400.0), "resolved point is remembered");
        // a deleted target leaves a free end at the last point
        d.remove(b).unwrap();
        let r = connector_route(&d, c).unwrap();
        assert_eq!(r.end(), v(350.0, 400.0));
    }

    #[test]
    fn connector_anchors_use_resolved_attached_node_size_without_mutating_document() {
        let mut d = Document::new("resolved connector size");
        let page = d.add_page("p");
        let frame_id = d.alloc_id();
        let frame = d.insert(page, Node::frame(frame_id, Rect::new(0.0, 0.0, 500.0, 300.0))).unwrap();

        let group_id = d.alloc_id();
        let mut group = Node::new(group_id, NodeKind::Group);
        group.transform = Affine::translate(10.0, 5.0);
        let group = d.insert(frame, group).unwrap();
        let target_id = d.alloc_id();
        let mut target = Node::rectangle(target_id, Rect::new(10.0, 25.0, 40.0, 50.0));
        target.variables.bindings.insert("width".into(), "target-width".into());
        target.variables.bindings.insert("height".into(), "target-height".into());
        let target = d.insert(group, target).unwrap();
        let peer_id = d.alloc_id();
        let peer = d.insert(frame, Node::rectangle(peer_id, Rect::new(300.0, 30.0, 40.0, 50.0))).unwrap();

        d.variable_collections.insert(
            "viewport".into(),
            VariableCollection {
                id: "viewport".into(),
                name: "Viewport".into(),
                modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
                default_mode: "light".into(),
                variables: vec![],
                key: None,
                source_library_key: None,
                extensions: Default::default(),
            },
        );
        let number = |id: &str, light: f32, dark: f32| Variable {
            id: id.into(),
            name: id.into(),
            collection: "viewport".into(),
            kind: VariableKind::Number,
            values: [("light".into(), VariableValue::Number(light)), ("dark".into(), VariableValue::Number(dark))].into_iter().collect(),
            description: String::new(),
            scopes: vec![],
            hidden_from_publishing: false,
            key: None,
            code_syntax: Default::default(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        };
        d.variables.insert("target-width".into(), number("target-width", 40.0, 80.0));
        d.variables.insert("target-height".into(), number("target-height", 50.0, 100.0));
        d.get_mut(frame).unwrap().variables.modes.insert("viewport".into(), "dark".into());

        let connector_id = d.alloc_id();
        let connector = d
            .insert(
                frame,
                Node::connector(
                    connector_id,
                    ConnectorProps {
                        start: ConnectorEnd::attached(target, Magnet::Right),
                        end: ConnectorEnd::attached(peer, Magnet::Left),
                        line: ConnectorLine::Straight,
                        ..Default::default()
                    },
                ),
            )
            .unwrap();
        let free_id = d.alloc_id();
        let free = d
            .insert(
                frame,
                Node::connector(
                    free_id,
                    ConnectorProps {
                        start: ConnectorEnd::attached(target, Magnet::Bottom),
                        end: ConnectorEnd::free(v(450.0, 150.0)),
                        line: ConnectorLine::Straight,
                        ..Default::default()
                    },
                ),
            )
            .unwrap();
        let before = d.clone();
        let (start, end) = connector_anchors(&d, connector).unwrap();
        assert_eq!(start.pos, v(100.0, 80.0), "right anchor must use dark-mode width and height");
        assert_eq!(end.pos, v(300.0, 55.0), "peer anchor remains based on its stored size");
        let (bottom, free_end) = connector_anchors(&d, free).unwrap();
        assert_eq!(bottom.pos, v(60.0, 130.0), "bottom anchor must use resolved width and height through nested transforms");
        assert_eq!(free_end.pos, v(450.0, 150.0), "free endpoint keeps its parent-space point");
        assert_eq!(d.to_json().unwrap(), before.to_json().unwrap(), "resolving connector anchors must not mutate the document");
    }

    #[test]
    fn kind_roundtrips_through_json() {
        let (mut d, page, a, b) = doc_with_two();
        d.kind = DocumentKind::Board;
        let cid = d.alloc_id();
        let mut props = ConnectorProps { start: ConnectorEnd::attached(a, Magnet::Right), end: ConnectorEnd::attached(b, Magnet::Left), ..Default::default() };
        props.label = Some(board_text(14.0, TextAlignH::Center, TextAlignV::Center));
        d.insert(page, Node::connector(cid, props)).unwrap();
        let sid = d.alloc_id();
        d.insert(page, Node::stamp(sid, v(0.0, 0.0), "heart", 64.0)).unwrap();
        let tid = d.alloc_id();
        d.insert(page, Node::shape_with_text(tid, Rect::new(0.0, 0.0, 10.0, 10.0), ShapeKind::Star, "x")).unwrap();
        let back = Document::from_json(&d.to_json().unwrap()).unwrap();
        assert_eq!(back.kind, DocumentKind::Board);
        assert_eq!(back.get(cid), d.get(cid));
        assert_eq!(back.get(sid), d.get(sid));
        assert_eq!(back.get(tid), d.get(tid));
        // files without a kind are design files
        let mut v: serde_json::Value = serde_json::from_str(&d.to_json().unwrap()).unwrap();
        v.as_object_mut().unwrap().remove("kind");
        let old: Document = serde_json::from_value(v).unwrap();
        assert_eq!(old.kind, DocumentKind::Design);
    }

    fn seeded_anchor_corpus(count: usize) -> Vec<(Anchor, Anchor)> {
        let magnets = [Magnet::Auto, Magnet::Top, Magnet::Right, Magnet::Bottom, Magnet::Left, Magnet::Center];
        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        fn next_u32(state: &mut u64) -> u32 {
            *state ^= *state >> 12;
            *state ^= *state << 25;
            *state ^= *state >> 27;
            (state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32
        }
        fn next_f32(state: &mut u64, lo: f32, hi: f32) -> f32 {
            let t = (next_u32(state) as f64 / u32::MAX as f64) as f32;
            lo + (hi - lo) * t
        }

        let mut out = Vec::with_capacity(count);
        for idx in 0..count {
            if idx < 24 {
                // Symmetric detour cases where top and bottom routes around both obstacles tie on cost.
                let y = 100.0 + (idx as f32) * 15.0;
                let w = 80.0 + (idx % 4) as f32 * 20.0;
                let h = 60.0 + (idx % 3) as f32 * 20.0;
                let gap = 40.0 + (idx % 5) as f32 * 30.0;
                let ra = Rect::new(0.0, y - h / 2.0, w, h);
                let rb = Rect::new(w + gap, y - h / 2.0, w, h);
                let (sm, em) = if idx % 2 == 0 { (Magnet::Left, Magnet::Right) } else { (Magnet::Right, Magnet::Left) };
                out.push(resolve_anchors((Some(ra), sm, v(0.0, 0.0)), (Some(rb), em, v(0.0, 0.0))));
                continue;
            }
            let sample_dim = |state: &mut u64, slot: usize, max_pos: f32| -> f32 {
                match slot % 12 {
                    0 => 0.0,
                    1 => -next_f32(state, 1.0, 160.0),
                    _ => next_f32(state, 24.0, max_pos),
                }
            };
            let mode = idx % 10;
            let ra = if mode < 8 {
                Some(Rect::new(
                    next_f32(&mut state, -600.0, 600.0),
                    next_f32(&mut state, -600.0, 600.0),
                    sample_dim(&mut state, idx, 260.0),
                    sample_dim(&mut state, idx / 3, 220.0),
                ))
            } else {
                None
            };
            let rb = if mode < 7 {
                Some(Rect::new(
                    next_f32(&mut state, -600.0, 600.0),
                    next_f32(&mut state, -600.0, 600.0),
                    sample_dim(&mut state, idx + 5, 260.0),
                    sample_dim(&mut state, (idx + 5) / 3, 220.0),
                ))
            } else {
                None
            };
            let sm = magnets[(next_u32(&mut state) as usize) % magnets.len()];
            let em = magnets[(next_u32(&mut state) as usize) % magnets.len()];
            let pa = v(next_f32(&mut state, -600.0, 600.0), next_f32(&mut state, -600.0, 600.0));
            let pb = v(next_f32(&mut state, -600.0, 600.0), next_f32(&mut state, -600.0, 600.0));
            out.push(resolve_anchors((ra, sm, pa), (rb, em, pb)));
        }
        out
    }

    fn elbow_linear_scan(a: &Anchor, b: &Anchor) -> Vec<Vec2> {
        let clear = match (a.rect, b.rect) {
            (Some(ra), Some(rb)) => {
                let (gx, gy) = gap_between(ra, rb);
                let g = gx.max(gy);
                if g > 0.0 {
                    (g / 2.0).clamp(4.0, ELBOW_STUB)
                } else {
                    ELBOW_STUB / 2.0
                }
            }
            _ => ELBOW_STUB,
        };
        let stub_a = if a.rect.is_some() { clear } else { 0.0 };
        let stub_b = if b.rect.is_some() { clear } else { 0.0 };
        let p0 = add(a.pos, mul(a.dir, stub_a));
        let p1 = add(b.pos, mul(b.dir, stub_b));
        let obstacles: Vec<Box4> = [a.rect, b.rect].iter().flatten().map(|r| inflate(*r, clear)).collect();
        let mid = v((p0.x + p1.x) / 2.0, (p0.y + p1.y) / 2.0);
        let mut xs = vec![p0.x, p1.x, mid.x, a.pos.x, b.pos.x];
        let mut ys = vec![p0.y, p1.y, mid.y, a.pos.y, b.pos.y];
        xs.extend([p0.x + a.dir.x * clear, p1.x + b.dir.x * clear]);
        ys.extend([p0.y + a.dir.y * clear, p1.y + b.dir.y * clear]);
        for o in &obstacles {
            xs.extend([o.x0, o.x1, o.x0 - clear, o.x1 + clear]);
            ys.extend([o.y0, o.y1, o.y0 - clear, o.y1 + clear]);
        }
        let (xs0, ys0) = (dedup_sorted(xs), dedup_sorted(ys));
        let mid_lanes = |s: &[f32]| s.windows(2).map(|w| (w[0] + w[1]) / 2.0).collect::<Vec<_>>();
        let mut xs = xs0.clone();
        xs.extend(mid_lanes(&xs0));
        let mut ys = ys0.clone();
        ys.extend(mid_lanes(&ys0));
        let (xs, ys) = (dedup_sorted(xs), dedup_sorted(ys));
        let (nx, ny) = (xs.len(), ys.len());
        let at = |i: usize, j: usize| v(xs[i], ys[j]);
        let (i0, j0) = (nearest(&xs, p0.x), nearest(&ys, p0.y));
        let (i1, j1) = (nearest(&xs, p1.x), nearest(&ys, p1.y));
        let want = dir_index(v(-b.dir.x, -b.dir.y));
        let start_dir = dir_index(a.dir).unwrap_or(4);

        let idx = |i: usize, j: usize, d: usize| (i * ny + j) * 5 + d;
        let mut dist = vec![f32::INFINITY; nx * ny * 5];
        let mut prev = vec![usize::MAX; nx * ny * 5];
        let mut done = vec![false; nx * ny * 5];
        let mut reached = Vec::with_capacity(256);
        let start = idx(i0, j0, start_dir);
        dist[start] = 0.0;
        reached.push(start);
        let mut found = None;
        for _ in 0..nx * ny * 5 {
            let mut best = usize::MAX;
            let mut bd = f32::INFINITY;
            for &s in &reached {
                let d = dist[s];
                if !done[s] && (d < bd || (d == bd && s < best)) {
                    bd = d;
                    best = s;
                }
            }
            if best == usize::MAX {
                break;
            }
            done[best] = true;
            let (cell, d) = (best / 5, best % 5);
            let (i, j) = (cell / ny, cell % ny);
            if i == i1 && j == j1 && want.is_none_or(|w| w == d || d == 4) {
                found = Some(best);
                break;
            }
            for (nd, (dx, dy)) in DIRS.iter().enumerate() {
                if d != 4 && (nd + 2) % 4 == d {
                    continue;
                }
                let (ni, nj) = (i as i32 + dx, j as i32 + dy);
                if ni < 0 || nj < 0 || ni >= nx as i32 || nj >= ny as i32 {
                    continue;
                }
                let (ni, nj) = (ni as usize, nj as usize);
                let ns = idx(ni, nj, nd);
                if done[ns] {
                    continue;
                }
                let (pa, pb) = (at(i, j), at(ni, nj));
                let mut c = dist[best] + len(sub(pb, pa));
                if d != 4 && nd != d {
                    c += BEND_COST + 0.01 * ((pa.x - mid.x).abs() + (pa.y - mid.y).abs());
                }
                if c < dist[ns] - 1e-4 {
                    if obstacles.iter().any(|o| seg_blocked(pa, pb, o)) {
                        continue;
                    }
                    if dist[ns].is_infinite() {
                        reached.push(ns);
                    }
                    dist[ns] = c;
                    prev[ns] = best;
                }
            }
        }
        let mut pts = vec![a.pos];
        match found {
            Some(mut s) => {
                let mut chain = Vec::new();
                while s != usize::MAX {
                    let cell = s / 5;
                    chain.push(at(cell / ny, cell % ny));
                    s = prev[s];
                }
                chain.reverse();
                pts.extend(chain);
            }
            None => {
                pts.push(p0);
                if a.dir.x.abs() > 0.5 || (a.dir == Vec2::ZERO && (p1.x - p0.x).abs() >= (p1.y - p0.y).abs()) {
                    pts.push(v(mid.x, p0.y));
                    pts.push(v(mid.x, p1.y));
                } else {
                    pts.push(v(p0.x, mid.y));
                    pts.push(v(p1.x, mid.y));
                }
                pts.push(p1);
            }
        }
        pts.push(b.pos);
        simplify(pts)
    }

    #[test]
    fn elbow_heap_matches_linear_scan_on_3000_seeded_cases() {
        let cited_a = Anchor { pos: v(10.0, 20.0), dir: v(1.0, 0.0), rect: Some(Rect::new(10.0, 20.0, 0.0, 0.0)) };
        let cited_b = Anchor { pos: v(200.0, 120.0), dir: v(-1.0, 0.0), rect: Some(Rect::new(200.0, 120.0, 0.0, 0.0)) };
        let (cited_got, cited_steps) = elbow_with_steps(&cited_a, &cited_b);
        let cited_want = elbow_linear_scan(&cited_a, &cited_b);
        assert_eq!(cited_got, cited_want, "cited zero-size rect case diverged between heap and linear scan");
        assert!(cited_steps <= 16_000);

        let corpus = seeded_anchor_corpus(3_000);
        for (idx, (a, b)) in corpus.iter().enumerate() {
            let (got, state_steps) = elbow_with_steps(a, b);
            let want = elbow_linear_scan(a, b);
            assert_eq!(got, want, "seeded case {idx} diverged between heap and linear scan");
            assert!(state_steps <= 16_000, "seeded case {idx} inspected {state_steps} states (expected binary-heap pop bound <= 16_000)");
        }
    }

    #[test]
    fn elbow_route_p95_under_100us_on_seeded_corpus() {
        let corpus = seeded_anchor_corpus(500);
        for (a, b) in corpus.iter().take(100) {
            std::hint::black_box(elbow(a, b));
        }
        let mut samples_us: Vec<f64> = Vec::with_capacity(corpus.len());
        for (a, b) in &corpus {
            let mut best_us = f64::INFINITY;
            for _ in 0..3 {
                let t0 = std::time::Instant::now();
                let pts = std::hint::black_box(elbow(a, b));
                let elapsed_us = t0.elapsed().as_nanos() as f64 / 1_000.0;
                assert!(pts.len() >= 2);
                if elapsed_us < best_us {
                    best_us = elapsed_us;
                }
            }
            samples_us.push(best_us);
        }
        samples_us.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
        let p50_us = samples_us[samples_us.len() / 2];
        let p95_idx = ((samples_us.len() as f64 * 0.95).ceil() as usize).saturating_sub(1).min(samples_us.len() - 1);
        let p95_us = samples_us[p95_idx];
        let scale = if cfg!(debug_assertions) || std::env::var_os("CI").is_some() { 15.0 } else { 1.0 };
        let budget_us = 100.0 * scale;
        eprintln!("[elbow_route_p95_under_100us_on_seeded_corpus] n={} p50={p50_us:.2}us p95={p95_us:.2}us budget={budget_us:.0}us", samples_us.len());
        assert!(p95_us <= budget_us, "elbow p95 {p95_us:.2}us exceeded {budget_us:.0}us budget");
    }

    #[test]
    fn degenerate_rects_route_without_panic_or_hang() {
        let finite_cases = [
            // Zero-size rects
            (
                Anchor { pos: v(10.0, 20.0), dir: v(1.0, 0.0), rect: Some(Rect::new(10.0, 20.0, 0.0, 0.0)) },
                Anchor { pos: v(200.0, 120.0), dir: v(-1.0, 0.0), rect: Some(Rect::new(200.0, 120.0, 0.0, 0.0)) },
            ),
            // Negative-size rects
            (
                Anchor { pos: v(50.0, 50.0), dir: v(0.0, 1.0), rect: Some(Rect::new(50.0, 50.0, -40.0, -30.0)) },
                Anchor { pos: v(150.0, 180.0), dir: v(0.0, -1.0), rect: Some(Rect::new(150.0, 180.0, -10.0, -20.0)) },
            ),
            // Identical overlapping rects
            (
                Anchor { pos: v(100.0, 50.0), dir: v(1.0, 0.0), rect: Some(Rect::new(0.0, 0.0, 100.0, 100.0)) },
                Anchor { pos: v(0.0, 50.0), dir: v(-1.0, 0.0), rect: Some(Rect::new(0.0, 0.0, 100.0, 100.0)) },
            ),
        ];

        for (idx, (a, b)) in finite_cases.iter().enumerate() {
            let r = route(ConnectorLine::Elbowed, a, b);
            assert_eq!(r.pts, elbow_linear_scan(a, b), "finite degenerate case {idx} diverged from linear scan");
            for pt in &r.pts {
                assert!(pt.x.is_finite() && pt.y.is_finite(), "finite case {idx} produced non-finite point {pt:?}");
            }
        }

        let fallback_cases = [
            // Huge coordinates that would overflow f32 length squares
            (
                Anchor { pos: v(-1e25, -1e25), dir: v(1.0, 0.0), rect: Some(Rect::new(-1e25, -1e25, 1e25, 1e25)) },
                Anchor { pos: v(1e25, 1e25), dir: v(-1.0, 0.0), rect: Some(Rect::new(1e25, 1e25, 1e25, 1e25)) },
            ),
            // Non-finite rect and anchor fields
            (
                Anchor { pos: v(0.0, 50.0), dir: v(1.0, 0.0), rect: Some(Rect::new(0.0, 0.0, f32::NAN, 100.0)) },
                Anchor { pos: v(200.0, 50.0), dir: v(-1.0, 0.0), rect: Some(Rect::new(200.0, 0.0, f32::INFINITY, 100.0)) },
            ),
            (
                Anchor { pos: v(f32::NAN, f32::INFINITY), dir: v(f32::NAN, 0.0), rect: None },
                Anchor { pos: v(f32::NEG_INFINITY, 40.0), dir: v(0.0, f32::INFINITY), rect: None },
            ),
        ];

        for (idx, (a, b)) in fallback_cases.iter().enumerate() {
            let r = route(ConnectorLine::Elbowed, a, b);
            assert_eq!(r.pts.len(), 2, "fallback case {idx} expected straight 2-point fallback");
            for pt in &r.pts {
                assert!(pt.x.is_finite() && pt.y.is_finite(), "fallback case {idx} produced non-finite point {pt:?}");
            }
        }
    }
}
