//! Figma UI3 fill / colour picker panel (`ui-spec.json` `color_picker`).
//!
//! The panel edits a [`Paint`]: a solid colour (opacity = alpha), or a gradient whose
//! selected stop is the colour being edited. It owns no document access; the host maps
//! the returned [`Out`] flags onto document edits.

use super::menu::{popup_menu, MenuItem, MenuResult};
use super::*;
use crate::theme;
use designos_doc::{Color, GradientStop, ImageScaleMode, Paint, PaintKind, Vec2 as DV};
use egui::{Mesh, Shape};

pub const W: f32 = 240.0;
const PAD: f32 = theme::INSET_POPOVER;
const AREA: f32 = 208.0;
const HEADER_H: f32 = 40.0;
const PAINT_ROW_H: f32 = 40.0;
const STOPS_H: f32 = 40.0;
const SWATCH: f32 = 16.0;
const SWATCH_PITCH: f32 = 24.0;
const PER_ROW: usize = 9;
const MAX_ROWS: usize = 4;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Tab {
    #[default]
    Custom,
    Libraries,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Fmt {
    #[default]
    Hex,
    Rgb,
    Css,
    Hsl,
    Hsb,
}

impl Fmt {
    pub const ALL: [Fmt; 5] = [Fmt::Hex, Fmt::Rgb, Fmt::Css, Fmt::Hsl, Fmt::Hsb];
    pub fn label(self) -> &'static str {
        match self {
            Fmt::Hex => "Hex",
            Fmt::Rgb => "RGB",
            Fmt::Css => "CSS",
            Fmt::Hsl => "HSL",
            Fmt::Hsb => "HSB",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Solid,
    Linear,
    Radial,
    Angular,
    Diamond,
    Image,
}

impl Mode {
    pub const ALL: [Mode; 6] = [Mode::Solid, Mode::Linear, Mode::Radial, Mode::Angular, Mode::Diamond, Mode::Image];
    pub fn icon(self) -> Icon {
        match self {
            Mode::Solid => Icon::PaintSolid,
            Mode::Linear => Icon::PaintLinear,
            Mode::Radial => Icon::PaintRadial,
            Mode::Angular => Icon::PaintAngular,
            Mode::Diamond => Icon::PaintDiamond,
            Mode::Image => Icon::PaintImage,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Mode::Solid => "Solid",
            Mode::Linear => "Linear",
            Mode::Radial => "Radial",
            Mode::Angular => "Angular",
            Mode::Diamond => "Diamond",
            Mode::Image => "Image",
        }
    }
    pub fn is_gradient(self) -> bool {
        matches!(self, Mode::Linear | Mode::Radial | Mode::Angular | Mode::Diamond)
    }
}

pub fn mode_of(k: &PaintKind) -> Mode {
    match k {
        PaintKind::Solid(_) => Mode::Solid,
        PaintKind::LinearGradient { .. } => Mode::Linear,
        PaintKind::RadialGradient { .. } => Mode::Radial,
        PaintKind::AngularGradient { .. } => Mode::Angular,
        PaintKind::DiamondGradient { .. } => Mode::Diamond,
        PaintKind::Image { .. } => Mode::Image,
    }
}

/// Default gradient handles in node-normalised space ([start/centre, end, width direction]).
pub fn default_handles(mode: Mode) -> [DV; 3] {
    match mode {
        Mode::Linear => [DV::new(0.0, 0.5), DV::new(1.0, 0.5), DV::new(0.0, 1.0)],
        _ => [DV::new(0.5, 0.5), DV::new(1.0, 0.5), DV::new(0.5, 1.0)],
    }
}

pub fn stops_of(k: &PaintKind) -> Option<&Vec<GradientStop>> {
    match k {
        PaintKind::LinearGradient { stops, .. }
        | PaintKind::RadialGradient { stops, .. }
        | PaintKind::AngularGradient { stops, .. }
        | PaintKind::DiamondGradient { stops, .. } => Some(stops),
        _ => None,
    }
}

pub fn stops_mut(k: &mut PaintKind) -> Option<&mut Vec<GradientStop>> {
    match k {
        PaintKind::LinearGradient { stops, .. }
        | PaintKind::RadialGradient { stops, .. }
        | PaintKind::AngularGradient { stops, .. }
        | PaintKind::DiamondGradient { stops, .. } => Some(stops),
        _ => None,
    }
}

pub fn handles_of(k: &PaintKind) -> Option<[DV; 3]> {
    match k {
        PaintKind::LinearGradient { handles, .. }
        | PaintKind::RadialGradient { handles, .. }
        | PaintKind::AngularGradient { handles, .. }
        | PaintKind::DiamondGradient { handles, .. } => Some(*handles),
        _ => None,
    }
}

/// The colour a paint is represented by (solid colour with opacity as alpha, or the first stop).
pub fn primary_rgba(p: &Paint) -> [u8; 4] {
    match &p.kind {
        PaintKind::Solid(c) => to_u8(*c, p.opacity),
        k => stops_of(k).and_then(|s| s.first()).map(|s| to_u8(s.color, 1.0)).unwrap_or([128, 128, 128, 255]),
    }
}

fn to_u8(c: Color, a: f32) -> [u8; 4] {
    let f = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    [f(c.r), f(c.g), f(c.b), f(c.a * a)]
}

fn from_u8(c: [u8; 4]) -> Color {
    Color::rgba(c[0], c[1], c[2], c[3])
}

fn c32(c: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

/// Converts `p` to paint type `to`, as Figma does: a solid colour becomes a gradient that
/// fades from that colour to transparent; a gradient becomes a solid of its first stop;
/// gradients of different shapes keep their stops.
pub fn convert(p: &Paint, to: Mode) -> Paint {
    if mode_of(&p.kind) == to {
        return p.clone();
    }
    let mut out = p.clone();
    let stops = match &p.kind {
        PaintKind::Solid(c) => {
            let c = Color { a: 1.0, ..*c };
            vec![GradientStop { pos: 0.0, color: Color { a: p.opacity.max(0.0), ..c } }, GradientStop { pos: 1.0, color: Color { a: 0.0, ..c } }]
        }
        k => stops_of(k)
            .cloned()
            .unwrap_or_else(|| vec![GradientStop { pos: 0.0, color: Color::BLACK }, GradientStop { pos: 1.0, color: Color { a: 0.0, ..Color::BLACK } }]),
    };
    out.kind = match to {
        Mode::Solid => {
            let first = stops.first().map(|s| s.color).unwrap_or(Color::BLACK);
            out.opacity = if matches!(p.kind, PaintKind::Solid(_)) { p.opacity } else { first.a };
            PaintKind::Solid(Color { a: 1.0, ..first })
        }
        Mode::Linear => {
            out.opacity = 1.0;
            PaintKind::LinearGradient { handles: default_handles(to), stops }
        }
        Mode::Radial => {
            out.opacity = 1.0;
            PaintKind::RadialGradient { handles: default_handles(to), stops }
        }
        Mode::Angular => {
            out.opacity = 1.0;
            PaintKind::AngularGradient { handles: default_handles(to), stops }
        }
        Mode::Diamond => {
            out.opacity = 1.0;
            PaintKind::DiamondGradient { handles: default_handles(to), stops }
        }
        Mode::Image => return p.clone(),
    };
    out
}

/// Colour being edited and its alpha, as straight 8-bit RGBA.
pub fn active_rgba(p: &Paint, sel: usize) -> [u8; 4] {
    match &p.kind {
        PaintKind::Solid(_) => primary_rgba(p),
        k => stops_of(k).and_then(|s| s.get(sel.min(s.len().saturating_sub(1)))).map(|s| to_u8(s.color, 1.0)).unwrap_or([128, 128, 128, 255]),
    }
}

pub fn set_active(p: &mut Paint, sel: usize, c: [u8; 4]) {
    match &mut p.kind {
        PaintKind::Solid(col) => {
            *col = Color::rgba(c[0], c[1], c[2], 255);
            p.opacity = c[3] as f32 / 255.0;
        }
        k => {
            if let Some(s) = stops_mut(k) {
                let i = sel.min(s.len().saturating_sub(1));
                if let Some(st) = s.get_mut(i) {
                    st.color = from_u8(c);
                }
            }
        }
    }
}

/// Colour of a gradient at `t` (linear in sRGB, like the renderer), for adding stops.
pub fn stop_color_at(stops: &[GradientStop], t: f32) -> Color {
    let mut s: Vec<&GradientStop> = stops.iter().collect();
    s.sort_by(|a, b| a.pos.total_cmp(&b.pos));
    let Some(first) = s.first() else { return Color::BLACK };
    let last = s[s.len() - 1];
    if t <= first.pos {
        return first.color;
    }
    if t >= last.pos {
        return last.color;
    }
    for w in s.windows(2) {
        if t >= w[0].pos && t <= w[1].pos {
            let f = if w[1].pos > w[0].pos { (t - w[0].pos) / (w[1].pos - w[0].pos) } else { 0.0 };
            let l = |a: f32, b: f32| a + (b - a) * f;
            return Color {
                r: l(w[0].color.r, w[1].color.r),
                g: l(w[0].color.g, w[1].color.g),
                b: l(w[0].color.b, w[1].color.b),
                a: l(w[0].color.a, w[1].color.a),
            };
        }
    }
    last.color
}

// ---------------------------------------------------------------- colour text formats
fn rgb_to_hsl(c: [u8; 4]) -> [f32; 3] {
    let (r, g, b) = (c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0);
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let l = (mx + mn) / 2.0;
    let d = mx - mn;
    if d <= f32::EPSILON {
        return [0.0, 0.0, l * 100.0];
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if mx == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if mx == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    [h * 60.0, s * 100.0, l * 100.0]
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> [u8; 3] {
    let (s, l) = ((s / 100.0).clamp(0.0, 1.0), (l / 100.0).clamp(0.0, 1.0));
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let f = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    [f(r), f(g), f(b)]
}

/// CSS text of a colour: `rgba(217, 217, 217, 1)`.
pub fn css_text(c: [u8; 4]) -> String {
    let a = c[3] as f32 / 255.0;
    format!("rgba({}, {}, {}, {})", c[0], c[1], c[2], fmt_num((a * 100.0).round() / 100.0))
}

/// Parses `#hex`, `hex`, `rgb(a)(...)` or `hsl(a)(...)` (commas or spaces, optional `/ alpha`).
pub fn parse_css(s: &str, keep_alpha: u8) -> Option<[u8; 4]> {
    let t = s.trim();
    let lower = t.to_ascii_lowercase();
    let func = lower
        .strip_prefix("rgba")
        .or_else(|| lower.strip_prefix("rgb"))
        .map(|r| (true, r))
        .or_else(|| lower.strip_prefix("hsla").or_else(|| lower.strip_prefix("hsl")).map(|r| (false, r)));
    if let Some((is_rgb, rest)) = func {
        let inner = rest.trim().strip_prefix('(')?.strip_suffix(')')?;
        let nums: Vec<f32> = inner
            .split([',', ' ', '/'])
            .map(|p| p.trim().trim_end_matches(['%', '°']))
            .filter(|p| !p.is_empty())
            .map(|p| p.trim_end_matches("deg").parse::<f32>().ok())
            .collect::<Option<_>>()?;
        if nums.len() < 3 {
            return None;
        }
        let a = nums.get(3).map(|a| (a.clamp(0.0, 1.0) * 255.0).round() as u8).unwrap_or(keep_alpha);
        let rgb = if is_rgb {
            [nums[0].clamp(0.0, 255.0).round() as u8, nums[1].clamp(0.0, 255.0).round() as u8, nums[2].clamp(0.0, 255.0).round() as u8]
        } else {
            hsl_to_rgb(nums[0], nums[1], nums[2])
        };
        return Some([rgb[0], rgb[1], rgb[2], a]);
    }
    let c = Color::from_hex(t)?;
    let u = to_u8(c, 1.0);
    // #RGBA / #RRGGBBAA and "transparent" carry their own alpha; plain hex / named colors keep the current one.
    let hex_part = t.trim_start_matches('#');
    let has_explicit_alpha =
        t.eq_ignore_ascii_case("transparent") || matches!(hex_part.len(), 4 | 5 | 7 | 8) && hex_part.chars().all(|ch| ch.is_ascii_hexdigit());
    Some([u[0], u[1], u[2], if has_explicit_alpha { u[3] } else { keep_alpha }])
}

// ---------------------------------------------------------------- HSB in gamma space
/// Hue / saturation / brightness / alpha, all 0..1, computed on the gamma-encoded sRGB values
/// (what Figma's HSB fields and colour area use; egui's own `Hsva` works in linear light).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsva {
    pub h: f32,
    pub s: f32,
    pub v: f32,
    pub a: f32,
}

impl Hsva {
    pub fn new(h: f32, s: f32, v: f32, a: f32) -> Self {
        Self { h, s, v, a }
    }

    pub fn from_srgba_unmultiplied(c: [u8; 4]) -> Self {
        let (r, g, b) = (c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0);
        let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
        let d = mx - mn;
        let h = if d <= f32::EPSILON {
            0.0
        } else if mx == r {
            ((g - b) / d).rem_euclid(6.0) / 6.0
        } else if mx == g {
            ((b - r) / d + 2.0) / 6.0
        } else {
            ((r - g) / d + 4.0) / 6.0
        };
        Self { h, s: if mx <= 0.0 { 0.0 } else { d / mx }, v: mx, a: c[3] as f32 / 255.0 }
    }

    pub fn to_srgba_unmultiplied(self) -> [u8; 4] {
        let h6 = self.h.rem_euclid(1.0) * 6.0;
        let (i, f) = (h6.floor() as i32, h6 - h6.floor());
        let (v, s) = (self.v.clamp(0.0, 1.0), self.s.clamp(0.0, 1.0));
        let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
        let (r, g, b) = match i {
            0 => (v, t, p),
            1 => (q, v, p),
            2 => (p, v, t),
            3 => (p, q, v),
            4 => (t, p, v),
            _ => (v, p, q),
        };
        let u = |x: f32| (x * 255.0).round().clamp(0.0, 255.0) as u8;
        [u(r), u(g), u(b), u(self.a)]
    }
}

// ---------------------------------------------------------------- state
pub struct State {
    pub tab: Tab,
    pub fmt: Fmt,
    /// Selected swatch source ("On this page", ...).
    pub source: usize,
    pub hsv: Hsva,
    /// Selected gradient stop.
    pub sel: usize,
    /// True while a dropdown of the panel is open (outside clicks must not close the picker).
    pub menu_open: bool,
}

impl State {
    pub fn new(p: &Paint) -> Self {
        let rgba = active_rgba(p, 0);
        Self { tab: Tab::Custom, fmt: Fmt::Hex, source: 0, hsv: Hsva::from_srgba_unmultiplied(rgba), sel: 0, menu_open: false }
    }

    /// Re-derives HSV from `rgba` unless it is what the HSV already encodes (keeps the hue
    /// of greys and blacks stable while dragging).
    fn sync(&mut self, rgba: [u8; 4]) {
        let cur = self.hsv.to_srgba_unmultiplied();
        if cur[..3] != rgba[..3] {
            let mut n = Hsva::from_srgba_unmultiplied(rgba);
            if n.s <= 0.0 || n.v <= 0.0 {
                n.h = self.hsv.h;
            }
            if n.v <= 0.0 {
                n.s = self.hsv.s;
            }
            self.hsv = n;
        }
        self.hsv.a = rgba[3] as f32 / 255.0;
    }
}

pub struct StyleItem {
    /// Style key in the document, or the asset key for a library style.
    pub key: String,
    pub name: String,
    pub paint: Paint,
    /// (library id, library name) for styles that still live in an enabled library.
    pub library: Option<(String, String)>,
}

const STYLE_ROW: f32 = 32.0;
const STYLE_HEAD: f32 = 24.0;
const STYLE_MAX_H: f32 = 12.0 * STYLE_ROW;

/// Rows of the Libraries tab: the document's own styles, then one section per library.
enum StyleRow<'a> {
    Head(&'a str),
    Item(&'a StyleItem),
}

fn style_rows(items: &[StyleItem]) -> Vec<StyleRow<'_>> {
    let mut rows: Vec<StyleRow> = items.iter().filter(|s| s.library.is_none()).map(StyleRow::Item).collect();
    let mut seen: Vec<&str> = Vec::new();
    for s in items {
        let Some((id, name)) = &s.library else { continue };
        if seen.contains(&id.as_str()) {
            continue;
        }
        seen.push(id);
        rows.push(StyleRow::Head(name));
        rows.extend(items.iter().filter(|o| o.library.as_ref().is_some_and(|(i, _)| i == id)).map(StyleRow::Item));
    }
    rows
}

fn style_body_height(items: &[StyleItem]) -> f32 {
    let h: f32 = style_rows(items).iter().map(|r| if matches!(r, StyleRow::Head(_)) { STYLE_HEAD } else { STYLE_ROW }).sum();
    h.clamp(STYLE_ROW, STYLE_MAX_H)
}

pub struct Env<'a> {
    /// Show the paint-type row (fills and strokes; page backgrounds and shadows are solid only).
    pub paint_types: bool,
    /// Swatch sources: name and colours.
    pub sources: &'a [(String, Vec<Color32>)],
    /// Document colour styles for the Libraries tab.
    pub styles: &'a [StyleItem],
    /// Header title when there is no tab strip.
    pub can_add_style: bool,
}

#[derive(Default)]
pub struct Out {
    pub changed: bool,
    pub add_style: bool,
    pub close: bool,
    pub eyedrop: bool,
    pub remove: bool,
    pub want_image: bool,
    /// Procedural 64x64 RGBA PNG generated in Mode::Image: (image_ref, png_bytes).
    pub generated_image: Option<(String, Vec<u8>)>,
    pub apply_style: Option<String>,
    /// (library id, asset key) of a library style to import and apply.
    pub apply_library_style: Option<(String, String)>,
}

/// Synthesizes a crisp 64x64 RGBA PNG in memory for the 3 studio procedural image presets.
pub fn synthesize_procedural_png(preset: usize) -> (String, Vec<u8>) {
    let key = match preset {
        0 => "proc-studio-grid",
        1 => "proc-gradient-mesh",
        _ => "proc-checkerboard",
    };
    let mut img = image::RgbaImage::new(64, 64);
    for y in 0..64u32 {
        for x in 0..64u32 {
            let px = match preset {
                0 => {
                    // Studio Grid: dark slate with crisp 16px major & 8px minor grid lines
                    if x % 16 == 0 || y % 16 == 0 {
                        [0x0D, 0x99, 0xFF, 0xFF]
                    } else if x % 8 == 0 || y % 8 == 0 {
                        [0x38, 0x38, 0x42, 0xFF]
                    } else {
                        [0x1E, 0x1E, 0x24, 0xFF]
                    }
                }
                1 => {
                    // Gradient Mesh: smooth 2D studio mesh
                    let r = ((x * 255) / 63) as u8;
                    let g = ((y * 220) / 63) as u8;
                    let b = (220u32.saturating_sub((x + y) * 2)) as u8;
                    [r, g, b, 0xFF]
                }
                _ => {
                    // Checkerboard: crisp 8x8 tile pattern
                    if ((x / 8) + (y / 8)) % 2 == 0 {
                        [0x2C, 0x2C, 0x2C, 0xFF]
                    } else {
                        [0x52, 0x52, 0x5B, 0xFF]
                    }
                }
            };
            img.put_pixel(x, y, image::Rgba(px));
        }
    }
    let mut png_bytes = Vec::new();
    let _ = image::DynamicImage::ImageRgba8(img).write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png);
    (key.to_string(), png_bytes)
}

const CONTRAST_BAR_H: f32 = 26.0;

fn srgb_to_linear(c: u8) -> f32 {
    let v = c as f32 / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

pub fn relative_luminance(rgb: [u8; 3]) -> f32 {
    0.2126 * srgb_to_linear(rgb[0]) + 0.7152 * srgb_to_linear(rgb[1]) + 0.0722 * srgb_to_linear(rgb[2])
}

pub fn wcag_contrast_ratio(l1: f32, l2: f32) -> f32 {
    let (hi, lo) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
    (hi + 0.05) / (lo + 0.05)
}

/// Total panel height for a layout.
pub fn panel_height(env: &Env, mode: Mode, tab: Tab, source: usize) -> f32 {
    let mut y = HEADER_H;
    if tab == Tab::Libraries {
        return y + 8.0 + style_body_height(env.styles) + 8.0;
    }
    if env.paint_types {
        y += PAINT_ROW_H;
    }
    if mode.is_gradient() {
        y += STOPS_H;
    } else if mode == Mode::Solid {
        y += CONTRAST_BAR_H;
    }
    y += PAD + AREA + 67.0 + 24.0 + 20.0 + 1.0 + 8.0 + 24.0;
    let n = env.sources.get(source).map(|s| s.1.len()).unwrap_or(0).min(PER_ROW * MAX_ROWS);
    if n > 0 {
        let rows = n.div_ceil(PER_ROW);
        y += 8.0 + rows as f32 * SWATCH_PITCH - (SWATCH_PITCH - SWATCH);
    }
    y + PAD
}

// ---------------------------------------------------------------- drawing helpers
/// Paints the four corner cut-outs of a rounded rect in `bg`, so a square mesh looks rounded.
fn round_corners(p: &Painter, r: Rect, radius: f32, bg: Color32) {
    let n = 8;
    let mut mesh = Mesh::default();
    for (corner, cx, cy, a0) in [
        (r.left_top(), r.left() + radius, r.top() + radius, std::f32::consts::PI),
        (r.right_top(), r.right() - radius, r.top() + radius, std::f32::consts::PI * 1.5),
        (r.right_bottom(), r.right() - radius, r.bottom() - radius, 0.0),
        (r.left_bottom(), r.left() + radius, r.bottom() - radius, std::f32::consts::FRAC_PI_2),
    ] {
        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(corner, bg);
        for i in 0..=n {
            let a = a0 + std::f32::consts::FRAC_PI_2 * i as f32 / n as f32;
            // Slightly over-reach so the AA edge of the mesh is covered.
            mesh.colored_vertex(Pos2::new(cx + (radius + 0.35) * a.cos(), cy + (radius + 0.35) * a.sin()), bg);
        }
        for i in 0..n as u32 {
            mesh.add_triangle(base, base + 1 + i, base + 2 + i);
        }
    }
    p.add(Shape::mesh(mesh));
}

fn checker(p: &Painter, r: Rect, cell: f32) {
    let p = p.with_clip_rect(r);
    p.rect_filled(r, 0.0, theme::CHECKER_A);
    let (nx, ny) = ((r.width() / cell).ceil() as i32, (r.height() / cell).ceil() as i32);
    for j in 0..ny {
        for i in 0..nx {
            if (i + j) % 2 == 1 {
                p.rect_filled(Rect::from_min_size(r.min + Vec2::new(i as f32 * cell, j as f32 * cell), Vec2::splat(cell)), 0.0, theme::CHECKER_B);
            }
        }
    }
}

fn quad_mesh(p: &Painter, r: Rect, tl: Color32, tr: Color32, br: Color32, bl: Color32) {
    let mut m = Mesh::default();
    m.colored_vertex(r.left_top(), tl);
    m.colored_vertex(r.right_top(), tr);
    m.colored_vertex(r.right_bottom(), br);
    m.colored_vertex(r.left_bottom(), bl);
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    p.add(Shape::mesh(m));
}

/// Saturation (x) / value (y) square for `hue`.
fn sv_area(p: &Painter, r: Rect, hue: f32) {
    const N: usize = 24;
    let mut m = Mesh::default();
    for j in 0..=N {
        for i in 0..=N {
            let (s, v) = (i as f32 / N as f32, 1.0 - j as f32 / N as f32);
            let c = Hsva::new(hue, s, v, 1.0).to_srgba_unmultiplied();
            m.colored_vertex(Pos2::new(r.left() + r.width() * i as f32 / N as f32, r.top() + r.height() * j as f32 / N as f32), c32(c));
        }
    }
    let w = (N + 1) as u32;
    for j in 0..N as u32 {
        for i in 0..N as u32 {
            let a = j * w + i;
            m.add_triangle(a, a + 1, a + w + 1);
            m.add_triangle(a, a + w + 1, a + w);
        }
    }
    p.add(Shape::mesh(m));
}

fn hue_bar(p: &Painter, r: Rect) {
    let mut m = Mesh::default();
    for i in 0..=6 {
        let x = r.left() + r.width() * i as f32 / 6.0;
        let c = c32(Hsva::new(i as f32 / 6.0, 1.0, 1.0, 1.0).to_srgba_unmultiplied());
        m.colored_vertex(Pos2::new(x, r.top()), c);
        m.colored_vertex(Pos2::new(x, r.bottom()), c);
    }
    for i in 0..6u32 {
        let a = i * 2;
        m.add_triangle(a, a + 2, a + 3);
        m.add_triangle(a, a + 3, a + 1);
    }
    p.add(Shape::mesh(m));
}

fn thumb_ring(p: &Painter, c: Pos2, outer: f32, ring: f32, fill: Option<Color32>) {
    p.circle_stroke(c, outer + 0.5, Stroke::new(1.0, Color32::from_black_alpha(0x4D)));
    if let Some(f) = fill {
        p.circle_filled(c, outer - ring, f);
    }
    p.circle_stroke(c, outer - ring / 2.0, Stroke::new(ring, Color32::WHITE));
}

/// Inspector swatch of a paint: the colour (checkerboard behind transparency), a left-to-right
/// preview of a gradient, or a neutral tile for images. Corner masks assume a `bg_secondary` row.
pub fn paint_swatch(p: &Painter, rect: Rect, paint: &Paint) {
    match &paint.kind {
        PaintKind::Solid(_) => super::swatch(p, rect, preview_color(paint)),
        k => {
            let stops = stops_of(k).cloned().unwrap_or_default();
            if stops.is_empty() {
                p.rect_filled(rect, theme::cr(theme::SWATCH_RADIUS), Color32::from_gray(0x80));
                return;
            }
            checker(p, rect, 4.0);
            let mut sorted = stops;
            sorted.sort_by(|a, b| a.pos.total_cmp(&b.pos));
            let mut cols: Vec<(f32, Color32)> = sorted.iter().map(|s| (s.pos.clamp(0.0, 1.0), c32(to_u8(s.color, paint.opacity)))).collect();
            if let Some(f) = cols.first().copied().filter(|f| f.0 > 0.0) {
                cols.insert(0, (0.0, f.1));
            }
            if let Some(l) = cols.last().copied().filter(|l| l.0 < 1.0) {
                cols.push((1.0, l.1));
            }
            let mut m = Mesh::default();
            for (t, c) in &cols {
                let x = rect.left() + t * rect.width();
                m.colored_vertex(Pos2::new(x, rect.top()), *c);
                m.colored_vertex(Pos2::new(x, rect.bottom()), *c);
            }
            for i in 0..cols.len().saturating_sub(1) as u32 {
                let a = i * 2;
                m.add_triangle(a, a + 2, a + 3);
                m.add_triangle(a, a + 3, a + 1);
            }
            p.add(Shape::mesh(m));
            round_corners(p, rect, theme::SWATCH_RADIUS as f32, theme::BG_SECONDARY);
            p.rect_stroke(rect, theme::cr(theme::SWATCH_RADIUS), Stroke::new(1.0, theme::BORDER_TRANSLUCENT), StrokeKind::Inside);
        }
    }
}

fn luminance(c: Color32) -> f32 {
    (0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32) / 255.0
}

/// Paint preview colour for a style row / swatch (first stop of gradients).
pub fn preview_color(p: &Paint) -> Color32 {
    c32(primary_rgba(p))
}

fn slider_pos(r: &Response, rect: Rect) -> Option<f32> {
    let pos = r.interact_pointer_pos()?;
    Some(((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0))
}

// ---------------------------------------------------------------- the panel
/// Draws the panel at `pos` (inside the caller's `Area`) and returns what happened.
pub fn show(ui: &mut Ui, pos: Pos2, st: &mut State, paint: &mut Paint, env: &Env) -> (Rect, Out) {
    let mut out = Out::default();
    let mode = mode_of(&paint.kind);
    let h = panel_height(env, mode, st.tab, st.source);
    let rect = Rect::from_min_size(pos, Vec2::new(W, h));
    ui.allocate_rect(rect, Sense::hover());
    st.menu_open = false;
    super::record(ui, "picker", rect);
    {
        let p = ui.painter();
        elevation_400(p, rect, theme::RADIUS_FRAME);
        p.rect_filled(rect, theme::cr(theme::RADIUS_FRAME), theme::BG);
        p.rect_stroke(rect, theme::cr(theme::RADIUS_FRAME), Stroke::new(1.0, theme::ELEVATION_400_RING), StrokeKind::Inside);
    }
    // ---- header: tabs, +, close
    let mut tab_x = rect.left() + 8.0;
    let tab_font = theme::font_body_strong();
    for (t, label) in [(Tab::Custom, "Custom"), (Tab::Libraries, "Libraries")] {
        if t == Tab::Libraries && !env.paint_types {
            continue;
        }
        let w = text_width(ui.painter(), label, &tab_font) + 16.0;
        let r = Rect::from_min_size(Pos2::new(tab_x, rect.top() + 8.0), Vec2::new(w, 24.0));
        super::record(ui, &format!("picker-tab:{label}"), r);
        let resp = ui.interact(r, ui.id().with(("picker-tab", label)), Sense::click());
        // Without Libraries (page background, shadows) the header is a plain title.
        let plain = !env.paint_types;
        let active = st.tab == t;
        if plain {
            let font = tab_font.clone();
            text_on_baseline(ui.painter(), rect.left() + PAD, centered_baseline(r, &font), label, font, theme::TEXT);
            continue;
        }
        if active {
            ui.painter().rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
        } else if resp.hovered() {
            ui.painter().rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_TRANSPARENT_HOVER);
        }
        text_on_baseline(
            ui.painter(),
            r.left() + 8.0,
            centered_baseline(r, &tab_font),
            label,
            tab_font.clone(),
            if active { theme::TEXT } else { theme::TEXT_SECONDARY },
        );
        if resp.clicked() {
            st.tab = t;
        }
        tab_x += w + 4.0;
    }
    let close_r = Rect::from_min_size(Pos2::new(rect.right() - 8.0 - 24.0, rect.top() + 8.0), Vec2::splat(24.0));
    if icon_button(ui, close_r, "picker-close", Icon::Close, false, "").clicked() {
        out.close = true;
    }
    if env.can_add_style {
        let add_r = close_r.translate(Vec2::new(-28.0, 0.0));
        super::record(ui, "picker-add-style", add_r);
        if icon_button(ui, add_r, "picker-add-style", Icon::Plus, false, "Create style").clicked() {
            out.add_style = true;
        }
    }
    hline(ui.painter(), rect.left(), rect.right(), rect.top() + HEADER_H - 1.0, theme::BORDER);

    if st.tab == Tab::Libraries {
        libraries(ui, rect, st, env, &mut out);
        return (rect, out);
    }
    let mut y = rect.top() + HEADER_H;

    // ---- paint type row
    if env.paint_types {
        for (i, m) in Mode::ALL.iter().enumerate() {
            let r = Rect::from_min_size(Pos2::new(rect.left() + 8.0 + i as f32 * 28.0, y + 8.0), Vec2::splat(24.0));
            let resp = ui.interact(r, ui.id().with(("picker-mode", i)), Sense::click());
            let active = *m == mode;
            button_bg(ui.painter(), r, &resp, active, theme::cr(theme::RADIUS_CONTROL));
            if active {
                ui.painter().rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
            }
            paint_icon_in(ui.painter(), r, m.icon(), 16.0, if active { theme::ICON } else { theme::ICON_SECONDARY });
            let resp = resp.on_hover_text(m.label());
            if resp.clicked() && !active {
                if *m == Mode::Image {
                    let (key, bytes) = synthesize_procedural_png(0);
                    paint.kind = PaintKind::Image { image_ref: key.clone(), scale_mode: ImageScaleMode::Fill };
                    paint.opacity = 1.0;
                    out.generated_image = Some((key, bytes));
                    out.changed = true;
                } else {
                    *paint = convert(paint, *m);
                    st.sel = 0;
                    out.changed = true;
                }
            }
        }
        // right side: remove paint
        let none_r = Rect::from_min_size(Pos2::new(rect.right() - 8.0 - 24.0, y + 8.0), Vec2::splat(24.0));
        if icon_button(ui, none_r, "picker-none", Icon::NoPaint, false, "Remove fill").clicked() {
            out.remove = true;
        }
        let blend_r = none_r.translate(Vec2::new(-28.0, 0.0));
        if blend_dropdown(ui, blend_r, paint, st) {
            out.changed = true;
        }
        y += PAINT_ROW_H;
        hline(ui.painter(), rect.left(), rect.right(), y - 1.0, theme::BORDER);
    }

    let mode = mode_of(&paint.kind);
    if mode == Mode::Image {
        let cur_scale = match &paint.kind {
            PaintKind::Image { scale_mode, .. } => *scale_mode,
            _ => ImageScaleMode::Fill,
        };
        let scale_label = match cur_scale {
            ImageScaleMode::Fill => "Scale: Fill",
            ImageScaleMode::Fit => "Scale: Fit",
            ImageScaleMode::Crop => "Scale: Crop",
            ImageScaleMode::Tile => "Scale: Tile",
        };
        let hdr_r = Rect::from_min_size(Pos2::new(rect.left() + PAD, y + 8.0), Vec2::new(AREA, 24.0));
        text_on_baseline(
            ui.painter(),
            hdr_r.left(),
            centered_baseline(hdr_r, &theme::font_body_strong()),
            "Image fill",
            theme::font_body_strong(),
            theme::TEXT,
        );
        let scale_r = Rect::from_min_size(Pos2::new(hdr_r.right() - 84.0, hdr_r.top()), Vec2::new(84.0, 24.0));
        super::record(ui, "picker-image-scale", scale_r);
        let scale_resp = ui.interact(scale_r, ui.id().with("picker-image-scale"), Sense::click());
        ui.painter().rect_filled(scale_r, theme::cr(theme::RADIUS_CONTROL), if scale_resp.hovered() { theme::BG_TERTIARY } else { theme::BG_SECONDARY });
        ui.painter().text(scale_r.center(), Align2::CENTER_CENTER, scale_label, theme::font_caption_medium(), theme::TEXT);
        if scale_resp.on_hover_text("Cycle image scale mode (Fill / Fit / Crop / Tile)").clicked() {
            let next = match cur_scale {
                ImageScaleMode::Fill => ImageScaleMode::Fit,
                ImageScaleMode::Fit => ImageScaleMode::Crop,
                ImageScaleMode::Crop => ImageScaleMode::Tile,
                ImageScaleMode::Tile => ImageScaleMode::Fill,
            };
            if let PaintKind::Image { scale_mode, .. } = &mut paint.kind {
                *scale_mode = next;
                out.changed = true;
            }
        }
        let mut iy = hdr_r.bottom() + 8.0;
        for (idx, label) in [(0usize, "Studio Grid"), (1, "Gradient Mesh"), (2, "Checkerboard")] {
            let btn_r = Rect::from_min_size(Pos2::new(rect.left() + PAD, iy), Vec2::new(AREA, 26.0));
            super::record(ui, &format!("picker-image-gen-{idx}"), btn_r);
            let bresp = ui.interact(btn_r, ui.id().with(("picker-image-gen", idx)), Sense::click());
            ui.painter().rect_filled(btn_r, theme::cr(theme::RADIUS_CONTROL), if bresp.hovered() { theme::BG_TERTIARY } else { theme::BG_SECONDARY });
            ui.painter().text(btn_r.center(), Align2::CENTER_CENTER, label, theme::font_body(), theme::TEXT);
            if bresp.clicked() {
                let (key, bytes) = synthesize_procedural_png(idx);
                paint.kind = PaintKind::Image { image_ref: key.clone(), scale_mode: cur_scale };
                paint.opacity = 1.0;
                out.generated_image = Some((key, bytes));
                out.changed = true;
            }
            iy += 30.0;
        }
        let disk_r = Rect::from_min_size(Pos2::new(rect.left() + PAD, iy), Vec2::new(AREA, 26.0));
        super::record(ui, "picker-image-disk", disk_r);
        let dresp = ui.interact(disk_r, ui.id().with("picker-image-disk"), Sense::click());
        ui.painter().rect_filled(disk_r, theme::cr(theme::RADIUS_CONTROL), if dresp.hovered() { theme::BG_TERTIARY } else { theme::BG_SECONDARY });
        ui.painter().text(disk_r.center(), Align2::CENTER_CENTER, "Choose from disk\u{2026}", theme::font_caption(), theme::TEXT_SECONDARY);
        if dresp.clicked() {
            out.want_image = true;
        }
        return (rect, out);
    }

    // ---- gradient stop bar + flip / rotate controls
    if mode.is_gradient() {
        let bar_w = AREA - 56.0;
        gradient_bar(ui, Rect::from_min_size(Pos2::new(rect.left() + PAD, y + 12.0), Vec2::new(bar_w, 16.0)), st, paint, &mut out);
        let flip_r = Rect::from_min_size(Pos2::new(rect.left() + PAD + bar_w + 4.0, y + 8.0), Vec2::splat(24.0));
        let rot_r = Rect::from_min_size(Pos2::new(flip_r.right() + 4.0, y + 8.0), Vec2::splat(24.0));
        if icon_button(ui, flip_r, "picker-grad-flip", Icon::FlipH, false, "Flip gradient").clicked() {
            if let Some(sv) = stops_mut(&mut paint.kind) {
                for s in sv.iter_mut() {
                    s.pos = (1.0 - s.pos).clamp(0.0, 1.0);
                }
                sv.sort_by(|a, b| a.pos.total_cmp(&b.pos));
                st.sel = sv.len().saturating_sub(1).saturating_sub(st.sel);
                out.changed = true;
            }
        }
        if icon_button(ui, rot_r, "picker-grad-rot", Icon::Rotate90, false, "Rotate gradient").clicked() {
            match &mut paint.kind {
                PaintKind::LinearGradient { handles, .. }
                | PaintKind::RadialGradient { handles, .. }
                | PaintKind::AngularGradient { handles, .. }
                | PaintKind::DiamondGradient { handles, .. } => {
                    for h in handles.iter_mut() {
                        let (dx, dy) = (h.x - 0.5, h.y - 0.5);
                        *h = DV::new(0.5 - dy, 0.5 + dx);
                    }
                    out.changed = true;
                }
                _ => {}
            }
        }
        y += STOPS_H;
    }
    let rgba = active_rgba(paint, st.sel);
    st.sync(rgba);

    let cur_lum = relative_luminance([rgba[0], rgba[1], rgba[2]]);
    let ref_lum = if cur_lum < 0.35 { 1.0 } else { relative_luminance([0x1E, 0x1E, 0x1E]) };
    if mode == Mode::Solid {
        let ratio = wcag_contrast_ratio(cur_lum, ref_lum);
        let bar_r = Rect::from_min_size(Pos2::new(rect.left() + PAD, y + 6.0), Vec2::new(AREA, 20.0));
        super::record(ui, "picker-contrast-bar", bar_r);
        let p = ui.painter();
        let icon_c = Pos2::new(bar_r.left() + 7.0, bar_r.center().y);
        let fg_col = Color32::from_rgb(0xE4, 0xE4, 0xE7);
        p.circle_stroke(icon_c, 5.5, Stroke::new(1.2, fg_col));
        p.rect_filled(Rect::from_min_max(Pos2::new(icon_c.x - 5.0, icon_c.y - 4.5), Pos2::new(icon_c.x, icon_c.y + 4.5)), 2.0, fg_col);
        p.text(Pos2::new(bar_r.left() + 18.0, bar_r.center().y), Align2::LEFT_CENTER, format!("{ratio:.2} : 1"), theme::font_body_strong(), fg_col);
        let (badge_txt, badge_bg, badge_fg, badge_w) = if ratio >= 7.0 {
            ("AAA", Color32::from_rgb(0x17, 0x30, 0x2A), Color32::from_rgb(0x4C, 0xC5, 0x8B), 44.0)
        } else if ratio >= 4.5 {
            ("AA", Color32::from_rgb(0x17, 0x30, 0x2A), Color32::from_rgb(0x4C, 0xC5, 0x8B), 38.0)
        } else if ratio >= 3.0 {
            ("AA Large", Color32::from_rgb(0x33, 0x27, 0x15), Color32::from_rgb(0xF5, 0xA5, 0x24), 64.0)
        } else {
            ("Low", Color32::from_rgb(0x36, 0x1D, 0x21), Color32::from_rgb(0xF3, 0x65, 0x7A), 40.0)
        };
        let badge_r = Rect::from_min_size(Pos2::new(bar_r.right() - badge_w, bar_r.top()), Vec2::new(badge_w, 20.0));
        p.rect_filled(badge_r, theme::cr(4), badge_bg);
        p.text(badge_r.center(), Align2::CENTER_CENTER, badge_txt, theme::font_caption_strong(), badge_fg);
        y += CONTRAST_BAR_H;
        hline(ui.painter(), rect.left(), rect.right(), y - 1.0, theme::BORDER);
    }

    // ---- colour area
    let area = Rect::from_min_size(Pos2::new(rect.left() + PAD, y + PAD), Vec2::splat(AREA));
    let mut new_hsv = st.hsv;
    let ar = ui.interact(area, ui.id().with("picker-area"), Sense::click_and_drag());
    if (ar.dragged() || ar.clicked() || ar.drag_started()) && ar.interact_pointer_pos().is_some() {
        if let Some(pt) = ar.interact_pointer_pos() {
            new_hsv.s = ((pt.x - area.left()) / area.width()).clamp(0.0, 1.0);
            new_hsv.v = 1.0 - ((pt.y - area.top()) / area.height()).clamp(0.0, 1.0);
        }
    }
    {
        let p = ui.painter();
        sv_area(p, area, st.hsv.h);
        round_corners(p, area, theme::RADIUS_CONTROL as f32, theme::BG);
        if mode == Mode::Solid {
            // Dotted 4.5:1 AA contrast threshold curve across 24 saturation steps
            let target_lum = if ref_lum > 0.5 { (ref_lum + 0.05) / 4.5 - 0.05 } else { 4.5 * (ref_lum + 0.05) - 0.05 };
            for step in 0..24 {
                let s = (step as f32 + 0.5) / 24.0;
                let max_rgb = Hsva::new(st.hsv.h, s, 1.0, 1.0).to_srgba_unmultiplied();
                let max_lum = relative_luminance([max_rgb[0], max_rgb[1], max_rgb[2]]);
                if target_lum > 0.005 && target_lum < max_lum {
                    let mut lo = 0.0f32;
                    let mut hi = 1.0f32;
                    for _ in 0..10 {
                        let mid = 0.5 * (lo + hi);
                        let c_mid = Hsva::new(st.hsv.h, s, mid, 1.0).to_srgba_unmultiplied();
                        if relative_luminance([c_mid[0], c_mid[1], c_mid[2]]) < target_lum {
                            lo = mid;
                        } else {
                            hi = mid;
                        }
                    }
                    let v_thresh = 0.5 * (lo + hi);
                    let dot_pos = Pos2::new(area.left() + s * area.width(), area.top() + (1.0 - v_thresh) * area.height());
                    p.circle_filled(dot_pos, 2.0, Color32::from_black_alpha(110));
                    p.circle_filled(dot_pos, 1.4, Color32::from_white_alpha(220));
                }
            }
        }
        let c = Pos2::new(area.left() + st.hsv.s * area.width(), area.top() + (1.0 - st.hsv.v) * area.height());
        thumb_ring(p, c, 7.0, 2.0, None);
    }

    // ---- sliders
    let sl_x = rect.left() + 52.0;
    let hue_c = area.bottom() + 19.0;
    let hue_r = Rect::from_min_max(Pos2::new(sl_x, hue_c - 6.0), Pos2::new(sl_x + 172.0, hue_c + 6.0));
    let al_r = hue_r.translate(Vec2::new(0.0, 28.0));
    let eye_r = Rect::from_min_size(Pos2::new(rect.left() + PAD, (hue_c + al_r.center().y) / 2.0 - 12.0), Vec2::splat(24.0));
    if icon_button(ui, eye_r, "picker-eyedrop", Icon::Eyedropper, false, "Pick color from canvas").clicked() {
        out.eyedrop = true;
    }
    let hr = ui.interact(hue_r.expand2(Vec2::new(8.0, 4.0)), ui.id().with("picker-hue"), Sense::click_and_drag());
    if hr.dragged() || hr.clicked() {
        if let Some(t) = slider_pos(&hr, hue_r) {
            new_hsv.h = t;
        }
    }
    let ar2 = ui.interact(al_r.expand2(Vec2::new(8.0, 4.0)), ui.id().with("picker-alpha"), Sense::click_and_drag());
    if ar2.dragged() || ar2.clicked() {
        if let Some(t) = slider_pos(&ar2, al_r) {
            new_hsv.a = t;
        }
    }
    {
        let p = ui.painter();
        hue_bar(p, hue_r);
        round_corners(p, hue_r, 6.0, theme::BG);
        let base = new_hsv.to_srgba_unmultiplied();
        checker(p, al_r, 4.0);
        quad_mesh(
            p,
            al_r,
            c32([base[0], base[1], base[2], 0]),
            c32([base[0], base[1], base[2], 255]),
            c32([base[0], base[1], base[2], 255]),
            c32([base[0], base[1], base[2], 0]),
        );
        round_corners(p, al_r, 6.0, theme::BG);
        let hx = hue_r.left() + 6.0 + new_hsv.h * (hue_r.width() - 12.0);
        thumb_ring(p, Pos2::new(hx, hue_r.center().y), 8.0, 3.0, Some(c32(Hsva::new(new_hsv.h, 1.0, 1.0, 1.0).to_srgba_unmultiplied())));
        let ax = al_r.left() + 6.0 + new_hsv.a * (al_r.width() - 12.0);
        thumb_ring(p, Pos2::new(ax, al_r.center().y), 8.0, 3.0, Some(c32(base)));
    }
    if new_hsv != st.hsv {
        st.hsv = new_hsv;
    }

    // ---- value row
    let vy = area.bottom() + 67.0;
    let row = Rect::from_min_size(Pos2::new(rect.left() + PAD, vy), Vec2::new(AREA, 24.0));
    let mut typed: Option<[u8; 4]> = None;
    // format dropdown
    {
        let dd = Rect::from_min_size(row.min, Vec2::new(55.0, 24.0));
        let labels: Vec<String> = Fmt::ALL.iter().map(|f| f.label().to_string()).collect();
        if let Some(i) = picker_dropdown(ui, dd, "picker-fmt", st.fmt.label(), &labels, &mut st.menu_open) {
            st.fmt = Fmt::ALL[i];
        }
    }
    let cur = {
        let a = (st.hsv.a * 255.0).round() as u8;
        let b = st.hsv.to_srgba_unmultiplied();
        [b[0], b[1], b[2], a]
    };
    let fields = Rect::from_min_max(Pos2::new(row.left() + 64.0, row.top()), row.right_bottom());
    let kind_rgba = |c: [u8; 4]| c;
    match st.fmt {
        Fmt::Hex => {
            let hex_r = Rect::from_min_size(fields.min, Vec2::new(89.0, 24.0));
            let op_r = Rect::from_min_max(Pos2::new(hex_r.right() + 1.0, fields.top()), fields.right_bottom());
            if let Some(s) = grouped_hex_text(
                ui,
                hex_r,
                "picker-hex",
                &Color::rgba(cur[0], cur[1], cur[2], 255).to_hex(),
                theme::cr_each(theme::RADIUS_CONTROL, 0, 0, theme::RADIUS_CONTROL),
                8.0,
            ) {
                if let Some(c) = parse_css(&s, cur[3]) {
                    typed = Some(kind_rgba(c));
                }
            }
            if let Some(a) = opacity_cell(ui, op_r, cur[3]) {
                typed = Some([cur[0], cur[1], cur[2], a]);
            }
        }
        Fmt::Css => {
            let r = Rect::from_min_size(fields.min, Vec2::new(fields.width(), 24.0));
            if let Some(s) = grouped_text(ui, r, "picker-css", &css_text(cur), theme::cr(theme::RADIUS_CONTROL), 8.0) {
                typed = parse_css(&s, cur[3]);
            }
        }
        Fmt::Rgb | Fmt::Hsl | Fmt::Hsb => {
            let vals: [f32; 3] = match st.fmt {
                Fmt::Rgb => [cur[0] as f32, cur[1] as f32, cur[2] as f32],
                Fmt::Hsl => rgb_to_hsl(cur),
                _ => [st.hsv.h * 360.0, st.hsv.s * 100.0, st.hsv.v * 100.0],
            };
            let w3 = (89.0 - 2.0) / 3.0;
            let mut edited: Option<(usize, f32)> = None;
            for (i, v) in vals.iter().enumerate() {
                let r = Rect::from_min_size(Pos2::new(fields.left() + i as f32 * (w3 + 1.0), fields.top()), Vec2::new(w3, 24.0));
                let radius = match i {
                    0 => theme::cr_each(theme::RADIUS_CONTROL, 0, 0, theme::RADIUS_CONTROL),
                    _ => theme::cr(0),
                };
                if let Some(n) = grouped_num(ui, r, ("picker-ch", i), v.round(), radius, 5.0) {
                    edited = Some((i, n));
                }
            }
            if let Some((i, n)) = edited {
                match st.fmt {
                    Fmt::Rgb => {
                        let mut c = cur;
                        c[i] = n.clamp(0.0, 255.0).round() as u8;
                        typed = Some(c);
                    }
                    Fmt::Hsl => {
                        let mut v = vals;
                        v[i] = n;
                        let rgb = hsl_to_rgb(v[0], v[1], v[2]);
                        typed = Some([rgb[0], rgb[1], rgb[2], cur[3]]);
                    }
                    _ => {
                        let mut h = st.hsv;
                        match i {
                            0 => h.h = (n / 360.0).rem_euclid(1.0),
                            1 => h.s = (n / 100.0).clamp(0.0, 1.0),
                            _ => h.v = (n / 100.0).clamp(0.0, 1.0),
                        }
                        st.hsv = h;
                        let b = h.to_srgba_unmultiplied();
                        typed = Some([b[0], b[1], b[2], cur[3]]);
                    }
                }
            }
            let op_r = Rect::from_min_max(Pos2::new(fields.left() + 89.0 + 1.0, fields.top()), fields.right_bottom());
            if let Some(a) = opacity_cell(ui, op_r, cur[3]) {
                typed = Some([typed.unwrap_or(cur)[0], typed.unwrap_or(cur)[1], typed.unwrap_or(cur)[2], a]);
            }
        }
    }
    if let Some(c) = typed {
        st.sync(c);
        st.hsv.a = c[3] as f32 / 255.0;
    }
    // Commit whatever HSV now says into the paint.
    let fin = {
        let b = st.hsv.to_srgba_unmultiplied();
        [b[0], b[1], b[2], (st.hsv.a * 255.0).round() as u8]
    };
    if fin != active_rgba(paint, st.sel) {
        set_active(paint, st.sel, fin);
        out.changed = true;
    }

    // ---- swatches
    let sep_y = vy + 24.0 + 20.0;
    hline(ui.painter(), rect.left(), rect.right(), sep_y, theme::BORDER);
    let dd = Rect::from_min_size(Pos2::new(rect.left() + PAD, sep_y + 1.0 + 8.0), Vec2::new(AREA, 24.0));
    let names: Vec<String> = env.sources.iter().map(|s| s.0.clone()).collect();
    if st.source >= names.len() {
        st.source = 0;
    }
    if !names.is_empty() {
        let label = names[st.source].clone();
        if let Some(i) = picker_dropdown(ui, dd, "picker-source", &label, &names, &mut st.menu_open) {
            st.source = i;
        }
    }
    let colors: &[Color32] = env.sources.get(st.source).map(|s| s.1.as_slice()).unwrap_or(&[]);
    let top = dd.bottom() + 8.0;
    for (i, col) in colors.iter().take(PER_ROW * MAX_ROWS).enumerate() {
        let r = Rect::from_min_size(
            Pos2::new(rect.left() + PAD + (i % PER_ROW) as f32 * SWATCH_PITCH, top + (i / PER_ROW) as f32 * SWATCH_PITCH),
            Vec2::splat(SWATCH),
        );
        let resp = ui.interact(r, ui.id().with(("picker-sw", i)), Sense::click());
        let p = ui.painter();
        let rad = theme::cr((SWATCH * 0.2).round() as u8);
        if col.a() < 255 {
            checker(&p.with_clip_rect(r), r, 4.0);
        }
        p.rect_filled(r, rad, *col);
        if luminance(*col) < 0.3 || col.a() < 255 {
            p.rect_stroke(r, rad, Stroke::new(2.0, Color32::from_white_alpha(0x1A)), StrokeKind::Inside);
        }
        if resp.hovered() {
            p.rect_stroke(r.expand(1.0), theme::cr(4), Stroke::new(1.0, theme::TEXT_TERTIARY), StrokeKind::Outside);
        }
        if resp.clicked() {
            let [r8, g8, b8, a8] = col.to_srgba_unmultiplied();
            let c = [r8, g8, b8, a8];
            st.sync(c);
            st.hsv.a = c[3] as f32 / 255.0;
            set_active(paint, st.sel, c);
            out.changed = true;
        }
    }
    (rect, out)
}

fn libraries(ui: &mut Ui, rect: Rect, st: &mut State, env: &Env, out: &mut Out) {
    let _ = st;
    let body = Rect::from_min_size(Pos2::new(rect.left(), rect.top() + HEADER_H + 8.0), Vec2::new(rect.width(), style_body_height(env.styles)));
    if env.styles.is_empty() {
        let r = Rect::from_min_size(Pos2::new(rect.left() + PAD, body.top()), Vec2::new(AREA, STYLE_ROW));
        text_on_baseline(ui.painter(), r.left(), centered_baseline(r, &theme::font_body()), "No color styles", theme::font_body(), theme::TEXT_TERTIARY);
        return;
    }
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
    egui::ScrollArea::vertical().id_salt("picker-styles").auto_shrink([false, false]).show(&mut child, |ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        for (i, row) in style_rows(env.styles).into_iter().enumerate() {
            match row {
                StyleRow::Head(name) => {
                    let (r, _) = ui.allocate_exact_size(Vec2::new(body.width(), STYLE_HEAD), Sense::hover());
                    let font = theme::font_caption();
                    let name = elide(ui.painter(), name, &font, r.width() - 24.0);
                    text_on_baseline(ui.painter(), r.left() + 16.0, centered_baseline(r, &font), &name, font, theme::TEXT_TERTIARY);
                }
                StyleRow::Item(s) => {
                    let (full, _) = ui.allocate_exact_size(Vec2::new(body.width(), STYLE_ROW), Sense::hover());
                    let r = full.shrink2(Vec2::new(8.0, 0.0));
                    let resp = ui.interact(r, ui.id().with(("picker-style", i)), Sense::click());
                    if resp.hovered() {
                        ui.painter().rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
                    }
                    let c = Pos2::new(r.left() + theme::STYLE_SWATCH_CX, r.center().y);
                    swatch_circle(ui.painter(), c, theme::STYLE_SWATCH / 2.0, &[preview_color(&s.paint)]);
                    let font = theme::font_body_strong();
                    let name = elide(ui.painter(), &s.name, &font, r.width() - 36.0);
                    text_on_baseline(ui.painter(), r.left() + theme::STYLE_LABEL_X, centered_baseline(r, &font), &name, font, theme::TEXT);
                    crate::widgets::record(ui, &format!("picker-style:{}", s.name), r);
                    if resp.clicked() {
                        match &s.library {
                            Some((lib, _)) => out.apply_library_style = Some((lib.clone(), s.key.clone())),
                            None => out.apply_style = Some(s.key.clone()),
                        }
                    }
                }
            }
        }
    });
}

/// 24px dropdown button with the picker's look (bg + 1px border). Returns the picked index.
fn picker_dropdown(ui: &mut Ui, rect: Rect, key: impl Key, label: &str, options: &[String], menu_open: &mut bool) -> Option<usize> {
    let id = ui.id().with(("dd", key));
    let r = ui.interact(rect, id, Sense::click());
    let open = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    let p = ui.painter();
    p.rect_filled(rect, theme::cr(theme::RADIUS_CONTROL), theme::BG);
    let border = if open { theme::BORDER_SELECTED } else { theme::BORDER };
    p.rect_stroke(
        rect,
        theme::cr(theme::RADIUS_CONTROL),
        Stroke::new(1.0, if r.hovered() && !open { theme::BORDER_STRONG.gamma_multiply(0.3) } else { border }),
        StrokeKind::Inside,
    );
    let font = theme::font_body_strong();
    let shown = elide(p, label, &font, rect.width() - 8.0 - 22.0);
    text_on_baseline(p, rect.left() + 8.0, centered_baseline(rect, &font), &shown, font, theme::TEXT);
    let chev = Rect::from_center_size(Pos2::new(rect.right() - 12.0, rect.center().y), Vec2::splat(12.0));
    paint_icon_in(p, chev, Icon::ChevronDown, 12.0, theme::ICON_SECONDARY);
    if r.clicked() {
        ui.data_mut(|d| d.insert_temp(id, !open));
    }
    let mut picked = None;
    if open {
        *menu_open = true;
        let items: Vec<MenuItem> = options.iter().map(|o| MenuItem::new(o.clone()).checked(o == label)).collect();
        match popup_menu(ui.ctx(), id.with("menu"), rect.left_bottom() + Vec2::new(0.0, 2.0), rect.width().max(120.0), &items, Some(rect)) {
            MenuResult::Picked(i) => {
                picked = Some(i);
                ui.data_mut(|d| d.insert_temp(id, false));
            }
            MenuResult::Closed => {
                ui.data_mut(|d| d.insert_temp(id, false));
            }
            MenuResult::Open => {}
        }
    }
    picked
}

fn blend_dropdown(ui: &mut Ui, rect: Rect, paint: &mut Paint, st: &mut State) -> bool {
    use designos_doc::BlendMode as B;
    const MODES: [(B, &str); 16] = [
        (B::Normal, "Normal"),
        (B::Darken, "Darken"),
        (B::Multiply, "Multiply"),
        (B::ColorBurn, "Color burn"),
        (B::Lighten, "Lighten"),
        (B::Screen, "Screen"),
        (B::ColorDodge, "Color dodge"),
        (B::Overlay, "Overlay"),
        (B::SoftLight, "Soft light"),
        (B::HardLight, "Hard light"),
        (B::Difference, "Difference"),
        (B::Exclusion, "Exclusion"),
        (B::Hue, "Hue"),
        (B::Saturation, "Saturation"),
        (B::Color, "Color"),
        (B::Luminosity, "Luminosity"),
    ];
    let id = ui.id().with("picker-blend");
    let r = ui.interact(rect, id, Sense::click());
    let open = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    button_bg(ui.painter(), rect, &r, open, theme::cr(theme::RADIUS_CONTROL));
    paint_icon_in(ui.painter(), rect, Icon::Blend, 16.0, if open || r.hovered() { theme::ICON } else { theme::ICON_SECONDARY });
    let r = r.on_hover_text("Blend mode");
    if r.clicked() {
        ui.data_mut(|d| d.insert_temp(id, !open));
    }
    let mut changed = false;
    if open {
        st.menu_open = true;
        let items: Vec<MenuItem> = MODES.iter().map(|(m, n)| MenuItem::new(*n).checked(*m == paint.blend)).collect();
        match popup_menu(ui.ctx(), id.with("menu"), rect.left_bottom() + Vec2::new(0.0, 2.0), 140.0, &items, Some(rect)) {
            MenuResult::Picked(i) => {
                paint.blend = MODES[i].0;
                changed = true;
                ui.data_mut(|d| d.insert_temp(id, false));
            }
            MenuResult::Closed => {
                ui.data_mut(|d| d.insert_temp(id, false));
            }
            MenuResult::Open => {}
        }
    }
    changed
}

/// Text field cell of the hex / channel group: bg_secondary with per-corner radius.
fn grouped_text(ui: &mut Ui, r: Rect, key: impl Key, display: &str, radius: egui::CornerRadius, pad: f32) -> Option<String> {
    let id = ui.id().with(("txt", key));
    let focused = ui.memory(|m| m.has_focus(id));
    ui.painter().rect_filled(r, radius, theme::BG_SECONDARY);
    if focused {
        ui.painter().rect_stroke(r, radius, Stroke::new(1.0, theme::BORDER_SELECTED), StrokeKind::Inside);
    } else if ui.rect_contains_pointer(r) {
        ui.painter().rect_stroke(r, radius, Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);
    }
    field::edit_text(ui, r, id, display, theme::font_body(), theme::TEXT, pad)
}

/// Hex field cell with `ArrowUp`/`ArrowDown` (`±1`) and `Shift+ArrowUp`/`ArrowDown` (`±10`) stepping.
fn grouped_hex_text(ui: &mut Ui, r: Rect, key: impl Key, display: &str, radius: egui::CornerRadius, pad: f32) -> Option<String> {
    let id = ui.id().with(("txt", key));
    let focused = ui.memory(|m| m.has_focus(id));
    ui.painter().rect_filled(r, radius, theme::BG_SECONDARY);
    if focused {
        ui.painter().rect_stroke(r, radius, Stroke::new(1.0, theme::BORDER_SELECTED), StrokeKind::Inside);
    } else if ui.rect_contains_pointer(r) {
        ui.painter().rect_stroke(r, radius, Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);
    }
    field::edit_hex_text(ui, r, id, display, theme::font_body(), theme::TEXT, pad)
}

/// Numeric channel cell (`RGB`/`HSL`/`HSB`) with `ArrowUp`/`ArrowDown` stepping and math expression evaluation.
fn grouped_num(ui: &mut Ui, r: Rect, key: impl Key, val: f32, radius: egui::CornerRadius, pad: f32) -> Option<f32> {
    let id = ui.id().with(("txt", key));
    let focused = ui.memory(|m| m.has_focus(id));
    ui.painter().rect_filled(r, radius, theme::BG_SECONDARY);
    if focused {
        ui.painter().rect_stroke(r, radius, Stroke::new(1.0, theme::BORDER_SELECTED), StrokeKind::Inside);
    } else if ui.rect_contains_pointer(r) {
        ui.painter().rect_stroke(r, radius, Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);
    }
    field::edit_num_text(ui, r, id, Some(val), false, theme::font_body(), theme::TEXT, pad, None)
}

/// Opacity cell: value + "%" in `text_secondary`. Returns the new alpha (0..=255).
fn opacity_cell(ui: &mut Ui, r: Rect, alpha: u8) -> Option<u8> {
    let id = ui.id().with(("num", "picker-opacity"));
    let focused = ui.memory(|m| m.has_focus(id));
    let radius = theme::cr_each(0, theme::RADIUS_CONTROL, theme::RADIUS_CONTROL, 0);
    ui.painter().rect_filled(r, radius, theme::BG_SECONDARY);
    if focused {
        ui.painter().rect_stroke(r, radius, Stroke::new(1.0, theme::BORDER_SELECTED), StrokeKind::Inside);
    } else if ui.rect_contains_pointer(r) {
        ui.painter().rect_stroke(r, radius, Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);
    }
    let text_r = Rect::from_min_max(r.min, Pos2::new(r.right() - 14.0, r.bottom()));
    let pct_val = (alpha as f32 / 255.0 * 100.0).round();
    let res = field::edit_num_text(ui, text_r, id, Some(pct_val), true, theme::font_body(), theme::TEXT, 8.0, None);
    let font = theme::font_body();
    text_on_baseline(ui.painter(), r.right() - 8.0 - text_width(ui.painter(), "%", &font), centered_baseline(r, &font), "%", font, theme::TEXT_SECONDARY);
    res.map(|v| (v.clamp(0.0, 100.0) / 100.0 * 255.0).round() as u8)
}

/// Gradient bar with draggable stop handles. Click empty bar to add a stop; drag a stop out
/// of the bar (30px or more away) to delete it.
fn gradient_bar(ui: &mut Ui, bar: Rect, st: &mut State, paint: &mut Paint, out: &mut Out) {
    let Some(stops) = stops_of(&paint.kind).cloned() else { return };
    st.sel = st.sel.min(stops.len().saturating_sub(1));
    // Bar preview.
    {
        let p = ui.painter();
        checker(p, bar, 4.0);
        let mut m = Mesh::default();
        let mut sorted = stops.clone();
        sorted.sort_by(|a, b| a.pos.total_cmp(&b.pos));
        let col = |c: Color| c32(to_u8(c, paint.opacity.max(0.0)));
        let mut cols: Vec<(f32, Color32)> = sorted.iter().map(|s| (s.pos.clamp(0.0, 1.0), col(s.color))).collect();
        if let Some(f) = cols.first().cloned() {
            if f.0 > 0.0 {
                cols.insert(0, (0.0, f.1));
            }
        }
        if let Some(l) = cols.last().cloned() {
            if l.0 < 1.0 {
                cols.push((1.0, l.1));
            }
        }
        for (t, c) in &cols {
            let x = bar.left() + t * bar.width();
            m.colored_vertex(Pos2::new(x, bar.top()), *c);
            m.colored_vertex(Pos2::new(x, bar.bottom()), *c);
        }
        for i in 0..cols.len().saturating_sub(1) as u32 {
            let a = i * 2;
            m.add_triangle(a, a + 2, a + 3);
            m.add_triangle(a, a + 3, a + 1);
        }
        p.add(Shape::mesh(m));
        round_corners(p, bar, theme::RADIUS_CONTROL as f32, theme::BG);
        p.rect_stroke(bar, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, theme::BORDER_TRANSLUCENT), StrokeKind::Inside);
    }
    // Add a stop by clicking the bar (stops sit above it in the z-order).
    let bar_resp = ui.interact(bar, ui.id().with("grad-bar"), Sense::click());
    if bar_resp.clicked() {
        if let Some(pt) = bar_resp.interact_pointer_pos() {
            let t = ((pt.x - bar.left()) / bar.width()).clamp(0.0, 1.0);
            let near = stops.iter().any(|s| ((s.pos - t) * bar.width()).abs() < 7.0);
            if !near {
                if let Some(s) = stops_mut(&mut paint.kind) {
                    let c = stop_color_at(s, t);
                    s.push(GradientStop { pos: t, color: c });
                    s.sort_by(|a, b| a.pos.total_cmp(&b.pos));
                    st.sel = s.iter().position(|x| x.pos == t).unwrap_or(0);
                    out.changed = true;
                }
            }
        }
    }
    // Handles.
    let mut remove: Option<usize> = None;
    for (i, s) in stops.iter().enumerate() {
        let c = Pos2::new(bar.left() + s.pos.clamp(0.0, 1.0) * bar.width(), bar.center().y);
        let hr = Rect::from_center_size(c, Vec2::new(14.0, 22.0));
        let resp = ui.interact(hr, ui.id().with(("grad-stop", i)), Sense::click_and_drag());
        if resp.clicked() || resp.drag_started() {
            st.sel = i;
        }
        if resp.dragged() {
            if let Some(pt) = resp.interact_pointer_pos() {
                if stops.len() > 2 && (pt.y - bar.center().y).abs() > 30.0 {
                    remove = Some(i);
                } else {
                    let t = ((pt.x - bar.left()) / bar.width()).clamp(0.0, 1.0);
                    // Stay between the neighbours so indices remain stable while dragging.
                    let lo = if i > 0 { stops[i - 1].pos } else { 0.0 };
                    let hi = if i + 1 < stops.len() { stops[i + 1].pos } else { 1.0 };
                    if let Some(sv) = stops_mut(&mut paint.kind) {
                        sv[i].pos = t.clamp(lo, hi);
                        out.changed = true;
                    }
                }
            }
        }
        let selected = st.sel == i;
        let p = ui.painter();
        let sq = Rect::from_center_size(c, Vec2::new(12.0, 16.0));
        p.rect_stroke(sq.expand(1.0), theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, Color32::from_black_alpha(0x4D)), StrokeKind::Outside);
        p.rect_filled(sq, theme::cr(4), Color32::WHITE);
        let inner = sq.shrink(if selected { 3.0 } else { 2.5 });
        checker(&p.with_clip_rect(inner), inner, 3.0);
        p.rect_filled(inner, theme::cr(2), c32(to_u8(s.color, 1.0)));
        if selected {
            p.rect_stroke(sq.expand(2.0), theme::cr(6), Stroke::new(1.0, theme::BORDER_SELECTED), StrokeKind::Outside);
        }
    }
    let del = ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) && !ui.memory(|m| m.focused().is_some());
    if del && stops.len() > 2 {
        remove = Some(st.sel);
    }
    if let Some(i) = remove {
        if let Some(sv) = stops_mut(&mut paint.kind) {
            if sv.len() > 2 && i < sv.len() {
                sv.remove(i);
                st.sel = st.sel.saturating_sub(1).min(sv.len() - 1);
                out.changed = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(r: u8, g: u8, b: u8, op: f32) -> Paint {
        Paint { opacity: op, ..Paint::solid(Color::rgb(r, g, b)) }
    }

    #[test]
    fn solid_to_gradient_keeps_color_as_first_stop() {
        let g = convert(&solid(10, 20, 30, 1.0), Mode::Linear);
        let PaintKind::LinearGradient { stops, handles } = &g.kind else { panic!() };
        assert_eq!(stops.len(), 2);
        assert_eq!(to_u8(stops[0].color, 1.0), [10, 20, 30, 255]);
        assert_eq!(stops[1].color.a, 0.0);
        assert_eq!(handles[0], DV::new(0.0, 0.5));
        assert_eq!(g.opacity, 1.0);
    }

    #[test]
    fn gradient_to_solid_takes_first_stop() {
        let g = convert(&solid(10, 20, 30, 0.5), Mode::Radial);
        let back = convert(&g, Mode::Solid);
        let PaintKind::Solid(c) = back.kind else { panic!() };
        assert_eq!(to_u8(c, 1.0), [10, 20, 30, 255]);
        // Alpha of the first stop (0.5) is carried by the paint opacity.
        assert!((back.opacity - 0.5).abs() < 0.01);
    }

    #[test]
    fn gradient_shape_switch_keeps_stops() {
        let g = convert(&solid(1, 2, 3, 1.0), Mode::Linear);
        let a = convert(&g, Mode::Angular);
        assert_eq!(stops_of(&a.kind), stops_of(&g.kind));
        assert_eq!(handles_of(&a.kind), Some(default_handles(Mode::Angular)));
    }

    #[test]
    fn active_color_edits_the_selected_stop() {
        let mut g = convert(&solid(10, 20, 30, 1.0), Mode::Linear);
        set_active(&mut g, 1, [200, 100, 50, 128]);
        assert_eq!(active_rgba(&g, 1), [200, 100, 50, 128]);
        assert_eq!(active_rgba(&g, 0), [10, 20, 30, 255]);
        let mut s = solid(1, 2, 3, 1.0);
        set_active(&mut s, 0, [9, 8, 7, 51]);
        assert_eq!(s.opacity, 0.2);
        assert_eq!(active_rgba(&s, 0), [9, 8, 7, 51]);
    }

    #[test]
    fn css_and_hsl_roundtrip() {
        assert_eq!(parse_css("#FF8000", 255), Some([255, 128, 0, 255]));
        assert_eq!(parse_css("ff8000", 77), Some([255, 128, 0, 77]));
        assert_eq!(parse_css("#F", 200), Some([255, 255, 255, 200]));
        assert_eq!(parse_css("#F0", 200), Some([240, 240, 240, 200]));
        assert_eq!(parse_css("#123", 200), Some([17, 34, 51, 200]));
        assert_eq!(parse_css("#1234", 200), Some([17, 34, 51, 68]));
        assert_eq!(parse_css("red", 255), Some([255, 0, 0, 255]));
        assert_eq!(parse_css("coral", 128), Some([255, 127, 80, 128]));
        assert_eq!(parse_css("transparent", 255), Some([0, 0, 0, 0]));
        assert_eq!(parse_css("rgb(10, 20, 30)", 255), Some([10, 20, 30, 255]));
        assert_eq!(parse_css("rgba(10 20 30 / 0.5)", 255), Some([10, 20, 30, 128]));
        assert_eq!(parse_css("nonsense", 255), None);
        assert_eq!(css_text([217, 217, 217, 255]), "rgba(217, 217, 217, 1)");
        let hsl = rgb_to_hsl([255, 0, 0, 255]);
        assert_eq!(hsl, [0.0, 100.0, 50.0]);
        assert_eq!(hsl_to_rgb(120.0, 100.0, 50.0), [0, 255, 0]);
        assert_eq!(parse_css("hsl(240, 100%, 50%)", 255), Some([0, 0, 255, 255]));
    }

    #[test]
    fn hsb_is_computed_in_gamma_space() {
        // Half saturation and half brightness of red: (128, 64, 64), as in Figma.
        assert_eq!(Hsva::new(0.0, 0.5, 0.5, 1.0).to_srgba_unmultiplied(), [128, 64, 64, 255]);
        for c in [[255, 0, 0, 255], [30, 30, 30, 255], [217, 217, 217, 255], [12, 140, 233, 255], [200, 100, 50, 255]] {
            assert_eq!(Hsva::from_srgba_unmultiplied(c).to_srgba_unmultiplied(), c);
        }
    }

    #[test]
    fn hue_survives_grey() {
        let mut st = State::new(&solid(255, 0, 0, 1.0));
        st.hsv.h = 0.6;
        st.sync([0, 0, 0, 255]);
        assert!((st.hsv.h - 0.6).abs() < 1e-6);
    }

    #[test]
    fn stop_color_interpolates() {
        let stops = vec![GradientStop { pos: 0.0, color: Color::BLACK }, GradientStop { pos: 1.0, color: Color::WHITE }];
        let c = stop_color_at(&stops, 0.5);
        assert!((c.r - 0.5).abs() < 1e-6);
    }

    #[test]
    fn procedural_png_presets_generate_valid_png_headers_and_distinct_textures() {
        let (k0, b0) = synthesize_procedural_png(0);
        let (k1, b1) = synthesize_procedural_png(1);
        let (k2, b2) = synthesize_procedural_png(2);
        assert_eq!(k0, "proc-studio-grid");
        assert_eq!(k1, "proc-gradient-mesh");
        assert_eq!(k2, "proc-checkerboard");
        for bytes in [&b0, &b1, &b2] {
            assert!(bytes.len() > 64);
            assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        }
        assert_ne!(b0, b1);
        assert_ne!(b1, b2);
    }
}
