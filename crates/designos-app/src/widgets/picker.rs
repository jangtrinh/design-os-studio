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
    Oklch,
    Oklab,
}

impl Fmt {
    pub const ALL: [Fmt; 7] = [Fmt::Hex, Fmt::Rgb, Fmt::Css, Fmt::Hsl, Fmt::Hsb, Fmt::Oklch, Fmt::Oklab];
    pub fn label(self) -> &'static str {
        match self {
            Fmt::Hex => "Hex",
            Fmt::Rgb => "RGB",
            Fmt::Css => "CSS",
            Fmt::Hsl => "HSL",
            Fmt::Hsb => "HSB",
            Fmt::Oklch => "OKLCH",
            Fmt::Oklab => "OKLAB",
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
        PaintKind::Solid(_) | PaintKind::Shader(_) => Mode::Solid,
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

/// CSS text of a colour: `rgb(217, 217, 217)`.
pub fn css_text(c: [u8; 4]) -> String {
    format!("rgb({}, {}, {})", c[0], c[1], c[2])
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
        let parts: Vec<&str> = inner.split([',', ' ', '/']).map(str::trim).filter(|p| !p.is_empty()).collect();
        let nums: Vec<f32> = parts
            .iter()
            .map(|p| p.trim_end_matches(['%', '°']))
            .filter(|p| !p.is_empty())
            .map(|p| p.trim_end_matches("deg").parse::<f32>().ok())
            .collect::<Option<_>>()?;
        if nums.len() < 3 {
            return None;
        }
        // A percent alpha (`/ 25%`) is 0..100%, as in CSS; a bare alpha is 0..1.
        let a = nums
            .get(3)
            .map(|a| {
                let unit = if parts[3].ends_with('%') { a / 100.0 } else { *a };
                (unit.clamp(0.0, 1.0) * 255.0).round() as u8
            })
            .unwrap_or(keep_alpha);
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
#[derive(Clone)]
pub struct State {
    pub tab: Tab,
    pub fmt: Fmt,
    /// Selected swatch source ("On this page", ...).
    pub source: usize,
    pub hsv: Hsva,
    /// Last OKLCH `[L, C, H_deg]` triple (`L` in `0..=1`, `C` in `0..=0.4`, `H` in `0..360`).
    pub oklch: [f32; 3],
    /// Last OKLAB `[L, a, b]` triple (`L` in `0..=1`, `a` and `b` in `-0.4..=0.4`).
    pub oklab: [f32; 3],
    /// Selected gradient stop.
    pub sel: usize,
    /// True while a dropdown of the panel is open (outside clicks must not close the picker).
    pub menu_open: bool,
    /// Provenance of each stop in the current gradient stop list relative to the start of the
    /// active picker transaction (`Some(orig_idx)` for stops that existed at the start of `pk.tx`,
    /// or `None` for newly inserted stops).
    pub stop_origins: Vec<Option<usize>>,
    /// Selected node size and decoded image dimensions `(node_size, (iw, ih))` for `Mode::Image`.
    pub image_frame: Option<(DV, (u32, u32))>,
    /// Rotate clicks (clockwise positive) that the host has not applied yet. The host turns each selected node's own
    /// Crop frame by them, because a frame depends on the node's size, and drops the frames kept outside Crop.
    pub image_turns: i32,
}

impl State {
    pub fn new(p: &Paint) -> Self {
        let rgba = active_rgba(p, 0);
        let rgb = [rgba[0], rgba[1], rgba[2]];
        let oklab = designos_doc::srgb_u8_to_oklab(rgb);
        let oklch = designos_doc::oklab_to_oklch(oklab[0], oklab[1], oklab[2]);
        let n = stops_of(&p.kind).map(|s| s.len()).unwrap_or(0);
        Self {
            tab: Tab::Custom,
            fmt: Fmt::Hex,
            source: 0,
            hsv: Hsva::from_srgba_unmultiplied(rgba),
            oklch,
            oklab,
            sel: 0,
            menu_open: false,
            stop_origins: (0..n).map(Some).collect(),
            image_frame: None,
            image_turns: 0,
        }
    }

    pub fn reset_stop_origins(&mut self, p: &Paint) {
        let n = stops_of(&p.kind).map(|s| s.len()).unwrap_or(0);
        self.stop_origins = (0..n).map(Some).collect();
    }

    /// Re-derives HSV and OKLCH/OKLAB from `rgba` unless it is what they already encode
    /// (keeps typed precision and the hue of greys and blacks stable).
    pub(crate) fn sync(&mut self, rgba: [u8; 4]) {
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
        self.sync_ok([rgba[0], rgba[1], rgba[2]]);
    }

    pub(crate) fn sync_ok(&mut self, rgb: [u8; 3]) {
        if designos_doc::oklch_to_srgb_u8(self.oklch[0], self.oklch[1], self.oklch[2]) != rgb {
            let mut lch = designos_doc::srgb_u8_to_oklch(rgb);
            if lch[1] < 1e-4 {
                lch[2] = self.oklch[2];
            }
            self.oklch = [lch[0].clamp(0.0, 1.0), lch[1].clamp(0.0, 0.4), lch[2]];
        }
        if designos_doc::oklab_to_srgb_u8(self.oklab[0], self.oklab[1], self.oklab[2]) != rgb {
            let lab = designos_doc::srgb_u8_to_oklab(rgb);
            self.oklab = [lab[0].clamp(0.0, 1.0), lab[1].clamp(-0.4, 0.4), lab[2].clamp(-0.4, 0.4)];
        }
    }

    pub(crate) fn set_oklch_channel(&mut self, i: usize, n: f32) -> [u8; 3] {
        self.oklch[i] = n;
        let lab = designos_doc::oklch_to_oklab(self.oklch[0], self.oklch[1], self.oklch[2]);
        self.oklab = [lab[0].clamp(0.0, 1.0), lab[1].clamp(-0.4, 0.4), lab[2].clamp(-0.4, 0.4)];
        designos_doc::oklch_to_srgb_u8(self.oklch[0], self.oklch[1], self.oklch[2])
    }

    pub(crate) fn set_oklab_channel(&mut self, i: usize, n: f32) -> [u8; 3] {
        self.oklab[i] = n;
        let mut lch = designos_doc::oklab_to_oklch(self.oklab[0], self.oklab[1], self.oklab[2]);
        if lch[1] < 1e-4 {
            lch[2] = self.oklch[2];
        }
        self.oklch = [lch[0].clamp(0.0, 1.0), lch[1].clamp(0.0, 0.4), lch[2]];
        designos_doc::oklab_to_srgb_u8(self.oklab[0], self.oklab[1], self.oklab[2])
    }

    #[cfg(test)]
    pub(crate) fn ok_cells(&self, fmt: Fmt) -> [String; 3] {
        let t = match fmt {
            Fmt::Oklab => self.oklab,
            _ => self.oklch,
        };
        [fmt_ok_channel(0, t[0]), fmt_ok_channel(1, t[1]), fmt_ok_channel(2, t[2])]
    }

    /// The `[L, C, H]` (OKLCH) or `[L, a, b]` (OKLAB) triple of `fmt`.
    fn ok_triple(&self, fmt: Fmt) -> [f32; 3] {
        match fmt {
            Fmt::Oklab => self.oklab,
            _ => self.oklch,
        }
    }

    pub(crate) fn ok_text(&self, fmt: Fmt) -> String {
        let t = match fmt {
            Fmt::Oklab => self.oklab,
            _ => self.oklch,
        };
        format!("{} {} {}", fmt_ok_token(0, t[0]), fmt_ok_token(1, t[1]), fmt_ok_token(2, t[2]))
    }

    pub(crate) fn apply_ok_input(&mut self, fmt: Fmt, input: &str, keep_alpha: u8) -> Option<[u8; 4]> {
        let mut s = input.trim();
        if s.is_empty() {
            return None;
        }
        let mut active_fmt = fmt;
        let mut is_func = false;
        let lower = s.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("oklch") {
            let rest_orig = &s[s.len() - rest.len()..].trim();
            s = rest_orig.strip_prefix('(')?.strip_suffix(')')?.trim();
            active_fmt = Fmt::Oklch;
            is_func = true;
        } else if let Some(rest) = lower.strip_prefix("oklab") {
            let rest_orig = &s[s.len() - rest.len()..].trim();
            s = rest_orig.strip_prefix('(')?.strip_suffix(')')?.trim();
            active_fmt = Fmt::Oklab;
            is_func = true;
        }
        let (ch_part, alpha) = if let Some((lhs, rhs)) = s.split_once('/') {
            if rhs.contains('/') {
                return None;
            }
            let a_str = rhs.trim();
            if a_str.is_empty() {
                return None;
            }
            let alpha = keep_alpha;
            let a = ((alpha as f32 / 255.0) * 100.0).round() / 100.0;
            let shown_a = fmt_num(a);
            let a_u8 = if keep_alpha < 255 && a_str == shown_a {
                keep_alpha
            } else if let Some(pct_body) = a_str.strip_suffix('%') {
                if pct_body.contains('%') {
                    return None;
                }
                let pct = field::eval_number_expr(pct_body, Some(keep_alpha as f32 / 255.0 * 100.0), false)?;
                ((pct.clamp(0.0, 100.0) / 100.0) * 255.0).round() as u8
            } else {
                if a_str.contains('%') {
                    return None;
                }
                let a_val = field::eval_number_expr(a_str, Some(keep_alpha as f32 / 255.0), false)?;
                (a_val.clamp(0.0, 1.0) * 255.0).round() as u8
            };
            (lhs.trim(), a_u8)
        } else {
            (s, keep_alpha)
        };
        let tokens: Vec<&str> = ch_part.split(|c: char| c.is_ascii_whitespace() || c == ',').filter(|t| !t.is_empty()).collect();
        if tokens.len() != 3 {
            return None;
        }
        let cur_triple = match active_fmt {
            Fmt::Oklab => self.oklab,
            _ => self.oklch,
        };
        let mut parsed = [0.0_f32; 3];
        let mut changed_mask = [false; 3];
        for i in 0..3 {
            let tok = tokens[i].trim();
            parsed[i] = if is_func && i == 0 && !tok.ends_with('%') {
                if tok.contains('%') {
                    return None;
                }
                field::eval_number_expr(tok, Some(cur_triple[0]), false)?.clamp(0.0, 1.0)
            } else {
                parse_ok_channel(active_fmt, i, tok, cur_triple[i])?
            };
            let exact_3dec = i > 0
                && ((cur_triple[i] * 1000.0).round() - cur_triple[i] * 1000.0).abs() < 1e-4
                && ((cur_triple[i] * 100.0).round() - cur_triple[i] * 100.0).abs() > 1e-4;
            let disp_tok = if exact_3dec { designos_export::util::num(cur_triple[i]) } else { fmt_ok_token(i, cur_triple[i]) };
            let disp_ch = if is_func && i == 0 || exact_3dec { disp_tok.clone() } else { fmt_ok_channel(i, cur_triple[i]) };
            changed_mask[i] = tok != disp_tok && tok != disp_ch;
        }
        let mut rgb = match active_fmt {
            Fmt::Oklab => designos_doc::oklab_to_srgb_u8(self.oklab[0], self.oklab[1], self.oklab[2]),
            _ => designos_doc::oklch_to_srgb_u8(self.oklch[0], self.oklch[1], self.oklch[2]),
        };
        for i in 0..3 {
            if changed_mask[i] {
                rgb = match active_fmt {
                    Fmt::Oklab => self.set_oklab_channel(i, parsed[i]),
                    _ => self.set_oklch_channel(i, parsed[i]),
                };
            }
        }
        Some([rgb[0], rgb[1], rgb[2], alpha])
    }
}

fn fmt_ok_token(ch: usize, val: f32) -> String {
    if ch == 0 {
        format!("{}%", fmt_ok_channel(0, val))
    } else {
        fmt_ok_channel(ch, val)
    }
}

fn fmt_ok_channel(ch: usize, val: f32) -> String {
    if ch == 0 {
        fmt_num(val * 100.0)
    } else {
        fmt_num(val)
    }
}

fn parse_ok_channel(fmt: Fmt, ch: usize, input: &str, cur: f32) -> Option<f32> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    match (fmt, ch) {
        (Fmt::Oklch | Fmt::Oklab, 0) => {
            let body = s.strip_suffix('%').unwrap_or(s);
            if body.contains('%') {
                return None;
            }
            let pct = field::eval_number_expr(body, Some(cur * 100.0), false)?;
            Some((pct / 100.0).clamp(0.0, 1.0))
        }
        (Fmt::Oklch, 1) => {
            if let Some(body) = s.strip_suffix('%') {
                if body.contains('%') {
                    return None;
                }
                let pct = field::eval_number_expr(body, Some(cur / 0.004), false)?;
                Some((pct * 0.004).clamp(0.0, 0.4))
            } else {
                if s.contains('%') {
                    return None;
                }
                let raw = field::eval_number_expr(s, Some(cur), false)?;
                Some(raw.clamp(0.0, 0.4))
            }
        }
        (Fmt::Oklch, 2) => {
            if s.contains('%') {
                return None;
            }
            let s = s.strip_suffix('°').unwrap_or(s).trim();
            let lower = s.to_ascii_lowercase();
            let s = if lower.ends_with("deg") { s[..s.len() - 3].trim() } else { s };
            let deg = field::eval_number_expr(s, Some(cur), false)?;
            let wrapped = deg.rem_euclid(360.0);
            Some(if wrapped >= 360.0 { 0.0 } else { wrapped })
        }
        (Fmt::Oklab, 1 | 2) => {
            if let Some(body) = s.strip_suffix('%') {
                if body.contains('%') {
                    return None;
                }
                let pct = field::eval_number_expr(body, Some(cur / 0.004), false)?;
                Some((pct * 0.004).clamp(-0.4, 0.4))
            } else {
                if s.contains('%') {
                    return None;
                }
                let raw = field::eval_number_expr(s, Some(cur), false)?;
                Some(raw.clamp(-0.4, 0.4))
            }
        }
        _ => None,
    }
}

pub struct StyleItem {
    /// Style key in the document, or the asset key for a library style.
    pub key: String,
    pub name: String,
    pub paint: Paint,
    /// Hover text from the style description. Empty when there is nothing to show.
    pub description: String,
    /// (library id, library name) for styles that still live in an enabled library.
    pub library: Option<(String, String)>,
}

static SCREENSHOT_LIBRARIES_HOVER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Headless `--panel fill-picker-libraries:hover` paints the first described row's tooltip.
pub fn set_screenshot_libraries_hover(on: bool) {
    SCREENSHOT_LIBRARIES_HOVER.store(on, std::sync::atomic::Ordering::Relaxed);
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
    pub commit_tx: bool,
    pub stop_list_changed: bool,
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

/// Image rows (rotate, tile scale, the seven adjustments) are a full control tall with a 1 px gap; the studio preset buttons
/// below them keep their own 29 px step.
const IMAGE_ROW_H: f32 = theme::CONTROL_XS;
/// The gap between two image rows is one hairline.
const IMAGE_ROW_STEP: f32 = IMAGE_ROW_H + theme::BORDER_WIDTH;
const IMAGE_PRESET_STEP: f32 = 29.0;
/// Adjustment sliders, in panel order.
const ADJ_LABELS: [&str; 7] = ["Exposure", "Contrast", "Saturation", "Temperature", "Tint", "Highlights", "Shadows"];
/// Rows that step by `IMAGE_ROW_STEP` in the tallest layout (Tile scale mode): rotate, tile scale and the adjustments.
const IMAGE_STEPPED_ROWS: f32 = (2 + ADJ_LABELS.len()) as f32;
const IMAGE_PANEL_BODY_H: f32 = 468.0 + IMAGE_STEPPED_ROWS * (IMAGE_ROW_STEP - IMAGE_PRESET_STEP);
const IMAGE_BTN_H: f32 = theme::TARGET_MIN;
const IMAGE_PRESET_H: f32 = 26.0;
const IMAGE_HDR_STEP: f32 = 26.0;
const IMAGE_TOP_PAD: f32 = 8.0;
const IMAGE_SCALE_W: f32 = 84.0;
const IMAGE_LABEL_W: f32 = 78.0;
const IMAGE_VAL_W: f32 = 48.0;
const IMAGE_VAL_GAP: f32 = 52.0;
const IMAGE_RESET_W: f32 = 44.0;
const IMAGE_ROT_STEP: f32 = 28.0;
const IMAGE_ROT_ANGLE_X: f32 = 142.0;
const IMAGE_TRACK_H: f32 = 4.0;
const IMAGE_THUMB_R: f32 = 5.0;
const IMAGE_CELL_PAD: f32 = 6.0;
const IMAGE_STROKE_W: f32 = 1.0;

/// Total panel height for a layout.
pub fn panel_height(env: &Env, mode: Mode, tab: Tab, source: usize) -> f32 {
    let mut y = HEADER_H;
    if tab == Tab::Libraries {
        return y + 8.0 + style_body_height(env.styles) + 8.0;
    }
    if env.paint_types {
        y += PAINT_ROW_H;
    }
    if mode == Mode::Image {
        return y + IMAGE_PANEL_BODY_H;
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
    checker_dim(p, r, cell, false);
}

fn checker_dim(p: &Painter, r: Rect, cell: f32, dim: bool) {
    let p = p.with_clip_rect(r);
    let ca = if dim { kit::half_color(theme::CHECKER_A) } else { theme::CHECKER_A };
    let cb = if dim { kit::half_color(theme::CHECKER_B) } else { theme::CHECKER_B };
    p.rect_filled(r, 0.0, ca);
    let (nx, ny) = ((r.width() / cell).ceil() as i32, (r.height() / cell).ceil() as i32);
    for j in 0..ny {
        for i in 0..nx {
            if (i + j) % 2 == 1 {
                p.rect_filled(Rect::from_min_size(r.min + Vec2::new(i as f32 * cell, j as f32 * cell), Vec2::splat(cell)), 0.0, cb);
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
fn sv_area(p: &Painter, r: Rect, hue: f32, dim: bool) {
    const N: usize = 24;
    let mut m = Mesh::default();
    for j in 0..=N {
        for i in 0..=N {
            let (s, v) = (i as f32 / N as f32, 1.0 - j as f32 / N as f32);
            let c = c32(Hsva::new(hue, s, v, 1.0).to_srgba_unmultiplied());
            let col = if dim { kit::half_color(c) } else { c };
            m.colored_vertex(Pos2::new(r.left() + r.width() * i as f32 / N as f32, r.top() + r.height() * j as f32 / N as f32), col);
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

fn hue_bar(p: &Painter, r: Rect, dim: bool) {
    let mut m = Mesh::default();
    for i in 0..=6 {
        let x = r.left() + r.width() * i as f32 / 6.0;
        let c = c32(Hsva::new(i as f32 / 6.0, 1.0, 1.0, 1.0).to_srgba_unmultiplied());
        let col = if dim { kit::half_color(c) } else { c };
        m.colored_vertex(Pos2::new(x, r.top()), col);
        m.colored_vertex(Pos2::new(x, r.bottom()), col);
    }
    for i in 0..6u32 {
        let a = i * 2;
        m.add_triangle(a, a + 2, a + 3);
        m.add_triangle(a, a + 3, a + 1);
    }
    p.add(Shape::mesh(m));
}

fn thumb_ring(p: &Painter, c: Pos2, outer: f32, ring: f32, fill: Option<Color32>, dim: bool) {
    let shadow = theme::CONTENT_HANDLE_RING;
    p.circle_stroke(c, outer + 0.5, Stroke::new(1.0, if dim { kit::half_color(shadow) } else { shadow }));
    if let Some(f) = fill {
        p.circle_filled(c, outer - ring, if dim { kit::half_color(f) } else { f });
    }
    let white = theme::CONTENT_HANDLE_FILL;
    p.circle_stroke(c, outer - ring / 2.0, Stroke::new(ring, if dim { kit::half_color(white) } else { white }));
}

/// Inspector swatch of a paint: the colour (checkerboard behind transparency), a left-to-right
/// preview of a gradient, or a neutral tile for images. Corner masks assume a `bg_secondary` row.
pub fn paint_swatch(p: &Painter, rect: Rect, paint: &Paint) {
    match &paint.kind {
        PaintKind::Solid(_) => super::swatch(p, rect, preview_color(paint)),
        k => {
            let stops = stops_of(k).cloned().unwrap_or_default();
            if stops.is_empty() {
                p.rect_filled(rect, theme::cr(theme::SWATCH_RADIUS), theme::SWATCH_PLACEHOLDER);
                return;
            }
            checker(p, rect, 4.0);
            let mut sorted = stops;
            sorted.sort_by(|a, b| a.pos.total_cmp(&b.pos));
            let mut cols: Vec<(f32, Color32)> =
                sorted.iter().map(|s| (s.pos.clamp(0.0, 1.0), crate::widgets::document_color32(c32(to_u8(s.color, paint.opacity))))).collect();
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

fn mono_font() -> FontId {
    let f = theme::font_mono;
    f()
}

fn image_pct_cell(ui: &mut Ui, r: Rect, key: impl Key, display: &str) -> Option<String> {
    let id = ui.id().with(("txt", key));
    let radius = theme::cr(theme::RADIUS_CONTROL);
    let focused = ui.memory(|m| m.has_focus(id));
    ui.painter().rect_filled(r, radius, theme::BG_SECONDARY);
    if focused {
        ui.painter().rect_stroke(r, radius, Stroke::new(IMAGE_STROKE_W, theme::BORDER_SELECTED), StrokeKind::Inside);
    } else if ui.rect_contains_pointer(r) {
        ui.painter().rect_stroke(r, radius, Stroke::new(IMAGE_STROKE_W, theme::BORDER), StrokeKind::Inside);
    }
    field::edit_text(ui, r, id, display, mono_font(), theme::TEXT, IMAGE_CELL_PAD)
}

fn paint_image_slider(p: &Painter, r: Rect, norm_t: f32, bimodal: bool) {
    let cy = r.center().y;
    let track = Rect::from_min_max(Pos2::new(r.left(), cy - IMAGE_TRACK_H * 0.5), Pos2::new(r.right(), cy + IMAGE_TRACK_H * 0.5));
    p.rect_filled(track, theme::cr(theme::RADIUS_MARK), theme::BG_SECONDARY);
    let t = norm_t.clamp(0.0, 1.0);
    let thumb_x = r.left() + r.width() * t;
    if bimodal {
        let mid_x = r.center().x;
        let (x0, x1) = if thumb_x >= mid_x { (mid_x, thumb_x) } else { (thumb_x, mid_x) };
        if (x1 - x0) > 0.5 {
            let active_r = Rect::from_min_max(Pos2::new(x0, cy - IMAGE_TRACK_H * 0.5), Pos2::new(x1, cy + IMAGE_TRACK_H * 0.5));
            p.rect_filled(active_r, theme::cr(theme::RADIUS_MARK), theme::TEXT_SECONDARY);
        }
    } else if thumb_x > r.left() + 0.5 {
        let active_r = Rect::from_min_max(Pos2::new(r.left(), cy - IMAGE_TRACK_H * 0.5), Pos2::new(thumb_x, cy + IMAGE_TRACK_H * 0.5));
        p.rect_filled(active_r, theme::cr(theme::RADIUS_MARK), theme::TEXT_SECONDARY);
    }
    let center = Pos2::new(thumb_x, cy);
    p.circle_filled(center, IMAGE_THUMB_R, theme::TEXT);
}

fn parse_pct_value(text: &str, cur_pct: f32, min_pct: f32, max_pct: f32) -> Option<f32> {
    let trimmed = text.trim();
    let body = trimmed.strip_suffix('%').unwrap_or(trimmed).trim();
    let v = field::eval_number_expr(body, Some(cur_pct), false)?;
    Some((v.clamp(min_pct, max_pct)) / 100.0)
}

fn show_image_mode(ui: &mut Ui, rect: Rect, y: f32, st: &mut State, paint: &mut Paint, out: &mut Out) {
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
    let hdr_r = Rect::from_min_size(Pos2::new(rect.left() + PAD, y + IMAGE_TOP_PAD), Vec2::new(AREA, IMAGE_BTN_H));
    text_on_baseline(ui.painter(), hdr_r.left(), centered_baseline(hdr_r, &theme::font_body_strong()), "Image fill", theme::font_body_strong(), theme::TEXT);
    let scale_r = Rect::from_min_size(Pos2::new(hdr_r.right() - IMAGE_SCALE_W, hdr_r.top()), Vec2::new(IMAGE_SCALE_W, IMAGE_BTN_H));
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
        // Without a known node and picture size no frame is invented: the paint keeps the transform it has.
        match st.image_frame {
            Some((node_sz, img_sz)) => designos_render::image_adjust::switch_image_scale_mode(paint, next, node_sz, img_sz),
            None => designos_render::image_adjust::set_image_scale_mode(paint, next),
        }
        out.changed = true;
        out.commit_tx = true;
    }

    let mut iy = hdr_r.bottom() + IMAGE_TOP_PAD;
    let cur_scale = match &paint.kind {
        PaintKind::Image { scale_mode, .. } => *scale_mode,
        _ => ImageScaleMode::Fill,
    };

    // ---- Rotate row (shown for every scale mode)
    {
        let row_r = Rect::from_min_size(Pos2::new(rect.left() + PAD, iy), Vec2::new(AREA, IMAGE_ROW_H));
        text_on_baseline(ui.painter(), row_r.left(), centered_baseline(row_r, &theme::font_body()), "Rotate", theme::font_body(), theme::TEXT_SECONDARY);
        let btn_y = row_r.center().y - IMAGE_BTN_H * 0.5;
        let rot_l_r = Rect::from_min_size(Pos2::new(row_r.left() + IMAGE_LABEL_W, btn_y), Vec2::splat(IMAGE_BTN_H));
        let rot_r_r = Rect::from_min_size(Pos2::new(rot_l_r.left() + IMAGE_ROT_STEP, btn_y), Vec2::splat(IMAGE_BTN_H));
        super::record(ui, "picker-image-rot-left", rot_l_r);
        super::record(ui, "picker-image-rot-right", rot_r_r);
        let click_l = icon_button(ui, rot_l_r, "picker-image-rot-left", Icon::Wrap, false, "Rotate 90° counter-clockwise").clicked();
        let click_r = icon_button(ui, rot_r_r, "picker-image-rot-right", Icon::Rotate90, false, "Rotate 90° clockwise").clicked();
        if let PaintKind::Image { scale_mode, params, .. } = &mut paint.kind {
            if click_l || click_r {
                let delta = if click_r { 90.0 } else { -90.0 };
                let stepped = (params.rotation + delta).rem_euclid(360.0);
                params.rotation = if (stepped - 360.0).abs() < 1e-4 { 0.0 } else { stepped };
                // The host turns each node's own Crop frame, which depends on the node's size, and drops it elsewhere.
                st.image_turns += if click_r { 1 } else { -1 };
                if *scale_mode != ImageScaleMode::Crop {
                    params.transform = None;
                }
                out.changed = true;
                out.commit_tx = true;
            }
            let deg = (params.rotation.rem_euclid(360.0).round() as i32) % 360;
            let angle_str = format!("{deg}°");
            let mfont = mono_font();
            text_on_baseline(ui.painter(), row_r.left() + IMAGE_ROT_ANGLE_X, centered_baseline(row_r, &mfont), &angle_str, mfont, theme::TEXT);
        }
        iy += IMAGE_ROW_STEP;
    }

    // ---- Tile scale row (shown for ImageScaleMode::Tile only)
    if cur_scale == ImageScaleMode::Tile {
        let row_r = Rect::from_min_size(Pos2::new(rect.left() + PAD, iy), Vec2::new(AREA, IMAGE_ROW_H));
        text_on_baseline(
            ui.painter(),
            row_r.left(),
            centered_baseline(row_r, &theme::font_caption()),
            "Tile scale",
            theme::font_caption(),
            theme::TEXT_SECONDARY,
        );
        let s_r = Rect::from_min_max(Pos2::new(row_r.left() + IMAGE_LABEL_W, row_r.top()), Pos2::new(row_r.right() - IMAGE_VAL_GAP, row_r.bottom()));
        let v_r = Rect::from_min_size(Pos2::new(row_r.right() - IMAGE_VAL_W, row_r.top()), Vec2::new(IMAGE_VAL_W, IMAGE_ROW_H));
        super::record(ui, "picker-image-tile-slider", s_r);
        super::record(ui, "picker-image-tile-val", v_r);
        let s_resp = ui.interact(s_r, ui.id().with("picker-image-tile-slider"), Sense::click_and_drag());
        if let PaintKind::Image { params, .. } = &mut paint.kind {
            if s_resp.dragged() || s_resp.clicked() {
                if let Some(t) = slider_pos(&s_resp, s_r) {
                    let next_ts = (0.1 + t * 3.9).clamp(0.01, 100.0);
                    params.tile_scale = (next_ts * 100.0).round() / 100.0;
                    out.changed = true;
                }
            }
            if s_resp.drag_stopped() || s_resp.clicked() {
                out.commit_tx = true;
            }
            let norm_t = ((params.tile_scale - 0.1) / 3.9).clamp(0.0, 1.0);
            paint_image_slider(ui.painter(), s_r, norm_t, false);
            let pct_num = (params.tile_scale * 100.0).round();
            let disp = format!("{}%", pct_num as i32);
            if let Some(committed) = image_pct_cell(ui, v_r, "img-tile-scale", &disp) {
                if let Some(parsed) = parse_pct_value(&committed, pct_num, 1.0, 10000.0) {
                    params.tile_scale = parsed;
                    out.changed = true;
                    out.commit_tx = true;
                }
            }
        }
        iy += IMAGE_ROW_STEP;
    }

    // ---- Adjustments group header + 7 sliders
    let has_adj = match &paint.kind {
        PaintKind::Image { params, .. } => designos_render::image_adjust::has_rendered_adjustments(&params.adjust),
        _ => false,
    };
    let adj_hdr_r = Rect::from_min_size(Pos2::new(rect.left() + PAD, iy), Vec2::new(AREA, IMAGE_BTN_H));
    text_on_baseline(
        ui.painter(),
        adj_hdr_r.left(),
        centered_baseline(adj_hdr_r, &theme::font_body_strong()),
        "Adjustments",
        theme::font_body_strong(),
        theme::TEXT,
    );
    if has_adj {
        let reset_r = Rect::from_min_size(Pos2::new(adj_hdr_r.right() - IMAGE_RESET_W, adj_hdr_r.top()), Vec2::new(IMAGE_RESET_W, IMAGE_BTN_H));
        super::record(ui, "picker-image-adj-reset", reset_r);
        let r_resp = ui.interact(reset_r, ui.id().with("picker-image-adj-reset"), Sense::click());
        if r_resp.hovered() {
            ui.painter().rect_filled(reset_r, theme::cr(theme::RADIUS_CONTROL), theme::BG_TRANSPARENT_HOVER);
        }
        ui.painter().text(reset_r.center(), Align2::CENTER_CENTER, "Reset", theme::font_caption_medium(), theme::TEXT_SECONDARY);
        if r_resp.clicked() {
            if let PaintKind::Image { params, .. } = &mut paint.kind {
                params.adjust.exposure = 0.0;
                params.adjust.contrast = 0.0;
                params.adjust.vibrance = 0.0;
                params.adjust.temperature = 0.0;
                params.adjust.tint = 0.0;
                params.adjust.highlights = 0.0;
                params.adjust.shadows = 0.0;
                out.changed = true;
                out.commit_tx = true;
            }
        }
    }
    iy += IMAGE_HDR_STEP;

    for (idx, label) in ADJ_LABELS.iter().copied().enumerate() {
        let row_r = Rect::from_min_size(Pos2::new(rect.left() + PAD, iy), Vec2::new(AREA, IMAGE_ROW_H));
        text_on_baseline(ui.painter(), row_r.left(), centered_baseline(row_r, &theme::font_caption()), label, theme::font_caption(), theme::TEXT_SECONDARY);
        let s_r = Rect::from_min_max(Pos2::new(row_r.left() + IMAGE_LABEL_W, row_r.top()), Pos2::new(row_r.right() - IMAGE_VAL_GAP, row_r.bottom()));
        let v_r = Rect::from_min_size(Pos2::new(row_r.right() - IMAGE_VAL_W, row_r.top()), Vec2::new(IMAGE_VAL_W, IMAGE_ROW_H));
        super::record(ui, &format!("picker-image-adj-slider:{label}"), s_r);
        super::record(ui, &format!("picker-image-adj-val:{label}"), v_r);
        let s_resp = ui.interact(s_r, ui.id().with(("picker-image-adj-slider", idx)), Sense::click_and_drag());
        if let PaintKind::Image { params, .. } = &mut paint.kind {
            let range_max = if idx == 1 { designos_render::image_adjust::CONTRAST_LIMIT } else { 1.0 };
            let slot: &mut f32 = match idx {
                0 => &mut params.adjust.exposure,
                1 => &mut params.adjust.contrast,
                2 => &mut params.adjust.vibrance,
                3 => &mut params.adjust.temperature,
                4 => &mut params.adjust.tint,
                5 => &mut params.adjust.highlights,
                _ => &mut params.adjust.shadows,
            };
            if s_resp.double_clicked() {
                *slot = 0.0;
                out.changed = true;
                out.commit_tx = true;
            } else if s_resp.dragged() || s_resp.clicked() {
                if let Some(t) = slider_pos(&s_resp, s_r) {
                    let pct_norm = ((t * 2.0 - 1.0) * 100.0).round() / 100.0;
                    *slot = (pct_norm * range_max).clamp(-range_max, range_max);
                    out.changed = true;
                }
            }
            if s_resp.drag_stopped() || s_resp.clicked() {
                out.commit_tx = true;
            }
            let norm_t = ((*slot / range_max) * 0.5 + 0.5).clamp(0.0, 1.0);
            paint_image_slider(ui.painter(), s_r, norm_t, true);
            let cur_pct = ((*slot / range_max) * 100.0).round();
            let disp = format!("{}%", cur_pct as i32);
            if let Some(committed) = image_pct_cell(ui, v_r, ("img-adj-val", idx), &disp) {
                if let Some(parsed) = parse_pct_value(&committed, cur_pct, -100.0, 100.0) {
                    *slot = (parsed * range_max).clamp(-range_max, range_max);
                    out.changed = true;
                    out.commit_tx = true;
                }
            }
        }
        iy += IMAGE_ROW_STEP;
    }

    // ---- Procedural studio presets + Choose from disk
    for (idx, label) in [(0usize, "Studio Grid"), (1, "Gradient Mesh"), (2, "Checkerboard")] {
        let btn_r = Rect::from_min_size(Pos2::new(rect.left() + PAD, iy), Vec2::new(AREA, IMAGE_PRESET_H));
        super::record(ui, &format!("picker-image-gen-{idx}"), btn_r);
        let bresp = ui.interact(btn_r, ui.id().with(("picker-image-gen", idx)), Sense::click());
        ui.painter().rect_filled(btn_r, theme::cr(theme::RADIUS_CONTROL), if bresp.hovered() { theme::BG_TERTIARY } else { theme::BG_SECONDARY });
        ui.painter().text(btn_r.center(), Align2::CENTER_CENTER, label, theme::font_body(), theme::TEXT);
        if bresp.clicked() {
            let (key, bytes) = synthesize_procedural_png(idx);
            paint.kind = PaintKind::Image { image_ref: key.clone(), scale_mode: cur_scale, params: Default::default() };
            paint.opacity = 1.0;
            out.generated_image = Some((key, bytes));
            out.changed = true;
            out.commit_tx = true;
        }
        iy += IMAGE_PRESET_STEP;
    }
    let disk_r = Rect::from_min_size(Pos2::new(rect.left() + PAD, iy), Vec2::new(AREA, IMAGE_PRESET_H));
    super::record(ui, "picker-image-disk", disk_r);
    let dresp = ui.interact(disk_r, ui.id().with("picker-image-disk"), Sense::click());
    ui.painter().rect_filled(disk_r, theme::cr(theme::RADIUS_CONTROL), if dresp.hovered() { theme::BG_TERTIARY } else { theme::BG_SECONDARY });
    ui.painter().text(disk_r.center(), Align2::CENTER_CENTER, "Choose from disk\u{2026}", theme::font_caption(), theme::TEXT_SECONDARY);
    if dresp.clicked() {
        out.want_image = true;
    }
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
                    paint.kind = PaintKind::Image { image_ref: key.clone(), scale_mode: ImageScaleMode::Fill, params: Default::default() };
                    paint.opacity = 1.0;
                    st.reset_stop_origins(paint);
                    out.generated_image = Some((key, bytes));
                    out.changed = true;
                } else {
                    *paint = convert(paint, *m);
                    st.sel = 0;
                    st.reset_stop_origins(paint);
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
        show_image_mode(ui, rect, y, st, paint, &mut out);
        return (rect, out);
    }

    // ---- gradient stop bar + flip / rotate controls
    if mode.is_gradient() {
        crate::panels::var_bind::picker_keep_open(ui.ctx(), st);
        let bar_w = AREA - 56.0;
        gradient_bar(ui, Rect::from_min_size(Pos2::new(rect.left() + PAD, y + 12.0), Vec2::new(bar_w, 16.0)), st, paint, &mut out);
        let flip_r = Rect::from_min_size(Pos2::new(rect.left() + PAD + bar_w + 4.0, y + 8.0), Vec2::splat(24.0));
        let rot_r = Rect::from_min_size(Pos2::new(flip_r.right() + 4.0, y + 8.0), Vec2::splat(24.0));
        if icon_button(ui, flip_r, "picker-grad-flip", Icon::FlipH, false, "Flip gradient").clicked() {
            if let Some(sv) = stops_mut(&mut paint.kind) {
                if st.stop_origins.len() != sv.len() {
                    st.stop_origins = (0..sv.len()).map(Some).collect();
                }
                let mut pairs: Vec<(GradientStop, Option<usize>)> = sv.iter().copied().zip(st.stop_origins.iter().copied()).collect();
                for (s, _) in pairs.iter_mut() {
                    s.pos = (1.0 - s.pos).clamp(0.0, 1.0);
                }
                pairs.sort_by(|a, b| a.0.pos.total_cmp(&b.0.pos));
                *sv = pairs.iter().map(|(s, _)| *s).collect();
                st.stop_origins = pairs.into_iter().map(|(_, orig)| orig).collect();
                st.sel = sv.len().saturating_sub(1).saturating_sub(st.sel);
                out.changed = true;
                out.stop_list_changed = true;
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
    let stop_bound = mode.is_gradient() && crate::panels::var_bind::picker_stop_is_bound(ui.ctx(), st.sel);

    let cur_lum = relative_luminance([rgba[0], rgba[1], rgba[2]]);
    let ref_lum = if cur_lum < 0.35 { 1.0 } else { relative_luminance([0x1E, 0x1E, 0x1E]) };
    if mode == Mode::Solid {
        let ratio = wcag_contrast_ratio(cur_lum, ref_lum);
        let bar_r = Rect::from_min_size(Pos2::new(rect.left() + PAD, y + 6.0), Vec2::new(AREA, 20.0));
        super::record(ui, "picker-contrast-bar", bar_r);
        let p = ui.painter();
        let icon_c = Pos2::new(bar_r.left() + 7.0, bar_r.center().y);
        let fg_col = theme::TEXT;
        p.circle_stroke(icon_c, 5.5, Stroke::new(1.2, fg_col));
        p.rect_filled(Rect::from_min_max(Pos2::new(icon_c.x - 5.0, icon_c.y - 4.5), Pos2::new(icon_c.x, icon_c.y + 4.5)), 2.0, fg_col);
        p.text(Pos2::new(bar_r.left() + 18.0, bar_r.center().y), Align2::LEFT_CENTER, format!("{ratio:.2} : 1"), theme::font_body_strong(), fg_col);
        let (badge_txt, badge_bg, badge_fg, badge_edge, badge_w) = if ratio >= 7.0 {
            ("AAA", theme::BG_SUCCESS_PRIMARY, theme::BADGE_SUCCESS_TEXT, theme::BADGE_SUCCESS_BORDER, 44.0)
        } else if ratio >= 4.5 {
            ("AA", theme::BG_SUCCESS_PRIMARY, theme::BADGE_SUCCESS_TEXT, theme::BADGE_SUCCESS_BORDER, 38.0)
        } else if ratio >= 3.0 {
            ("AA Large", theme::BG_WARNING_PRIMARY, theme::BADGE_WARNING_TEXT, theme::BADGE_WARNING_BORDER, 64.0)
        } else {
            ("Low", theme::BG_ERROR_PRIMARY, theme::BADGE_ERROR_TEXT, theme::BADGE_ERROR_BORDER, 40.0)
        };
        let badge_r = Rect::from_min_size(Pos2::new(bar_r.right() - badge_w, bar_r.top()), Vec2::new(badge_w, 20.0));
        p.rect_filled(badge_r, theme::cr(theme::RADIUS_SEGMENT), badge_bg);
        p.rect_stroke(badge_r, theme::cr(theme::RADIUS_SEGMENT), Stroke::new(theme::BORDER_WIDTH, badge_edge), StrokeKind::Inside);
        p.text(badge_r.center(), Align2::CENTER_CENTER, badge_txt, theme::font_caption_strong(), badge_fg);
        y += CONTRAST_BAR_H;
        hline(ui.painter(), rect.left(), rect.right(), y - 1.0, theme::BORDER);
    }

    // ---- colour area
    let area = Rect::from_min_size(Pos2::new(rect.left() + PAD, y + PAD), Vec2::splat(AREA));
    let mut new_hsv = st.hsv;
    let drag_sense = if stop_bound { Sense::hover() } else { Sense::click_and_drag() };
    let ar = ui.interact(area, ui.id().with("picker-area"), drag_sense);
    if !stop_bound && (ar.dragged() || ar.clicked() || ar.drag_started()) && ar.interact_pointer_pos().is_some() {
        if let Some(pt) = ar.interact_pointer_pos() {
            new_hsv.s = ((pt.x - area.left()) / area.width()).clamp(0.0, 1.0);
            new_hsv.v = 1.0 - ((pt.y - area.top()) / area.height()).clamp(0.0, 1.0);
        }
    }
    {
        let p = ui.painter();
        sv_area(p, area, st.hsv.h, stop_bound);
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
                    p.circle_filled(dot_pos, 2.0, theme::CONTENT_MARK_SHADOW);
                    p.circle_filled(dot_pos, 1.4, theme::CONTENT_MARK_LIGHT);
                }
            }
        }
        let c = Pos2::new(area.left() + st.hsv.s * area.width(), area.top() + (1.0 - st.hsv.v) * area.height());
        thumb_ring(p, c, 7.0, 2.0, None, stop_bound);
    }

    // ---- sliders
    let sl_x = rect.left() + 52.0;
    let hue_c = area.bottom() + 19.0;
    let hue_r = Rect::from_min_max(Pos2::new(sl_x, hue_c - 6.0), Pos2::new(sl_x + 172.0, hue_c + 6.0));
    let al_r = hue_r.translate(Vec2::new(0.0, 28.0));
    let eye_r = Rect::from_min_size(Pos2::new(rect.left() + PAD, (hue_c + al_r.center().y) / 2.0 - 12.0), Vec2::splat(24.0));
    if kit::button_icon(ui, eye_r, "picker-eyedrop", Icon::Eyedropper, kit::Hierarchy::Icon, kit::Size::Xs, !stop_bound, false)
        .on_hover_text("Pick color from canvas")
        .clicked()
        && !stop_bound
    {
        out.eyedrop = true;
    }
    let hr = ui.interact(hue_r.expand2(Vec2::new(8.0, 4.0)), ui.id().with("picker-hue"), drag_sense);
    if !stop_bound && (hr.dragged() || hr.clicked()) {
        if let Some(t) = slider_pos(&hr, hue_r) {
            new_hsv.h = t;
        }
    }
    let ar2 = ui.interact(al_r.expand2(Vec2::new(8.0, 4.0)), ui.id().with("picker-alpha"), drag_sense);
    if !stop_bound && (ar2.dragged() || ar2.clicked()) {
        if let Some(t) = slider_pos(&ar2, al_r) {
            new_hsv.a = t;
        }
    }
    {
        let p = ui.painter();
        hue_bar(p, hue_r, stop_bound);
        round_corners(p, hue_r, 6.0, theme::BG);
        let base = new_hsv.to_srgba_unmultiplied();
        checker_dim(p, al_r, 4.0, stop_bound);
        let c_start = c32([base[0], base[1], base[2], 0]);
        let c_end = c32([base[0], base[1], base[2], 255]);
        let (c0, c1) = if stop_bound { (kit::half_color(c_start), kit::half_color(c_end)) } else { (c_start, c_end) };
        quad_mesh(p, al_r, c0, c1, c1, c0);
        round_corners(p, al_r, 6.0, theme::BG);
        let hx = hue_r.left() + 6.0 + new_hsv.h * (hue_r.width() - 12.0);
        thumb_ring(p, Pos2::new(hx, hue_r.center().y), 8.0, 3.0, Some(c32(Hsva::new(new_hsv.h, 1.0, 1.0, 1.0).to_srgba_unmultiplied())), stop_bound);
        let ax = al_r.left() + 6.0 + new_hsv.a * (al_r.width() - 12.0);
        thumb_ring(p, Pos2::new(ax, al_r.center().y), 8.0, 3.0, Some(c32(base)), stop_bound);
    }
    if new_hsv != st.hsv {
        st.hsv = new_hsv;
    }

    // ---- value row
    let vy = area.bottom() + 67.0;
    let row = Rect::from_min_size(Pos2::new(rect.left() + PAD, vy), Vec2::new(AREA, 24.0));
    let pill_drawn = mode.is_gradient() && crate::panels::var_bind::picker_stop_pill_field(ui, st, row, st.sel);
    if !stop_bound && !pill_drawn {
        let mut typed: Option<[u8; 4]> = None;
        // format dropdown sized to the longest label in Fmt::ALL
        let dd_w = {
            let p = ui.painter();
            let font = theme::font_body_strong();
            let max_label_w = Fmt::ALL.iter().map(|f| text_width(p, f.label(), &font)).fold(0.0_f32, f32::max);
            (max_label_w + 4.0 + 20.0).ceil()
        };
        let dd = Rect::from_min_size(row.min, Vec2::new(dd_w, 24.0));
        {
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
        st.sync_ok([cur[0], cur[1], cur[2]]);
        let fields = Rect::from_min_max(Pos2::new(dd.right() + 4.0, row.top()), row.right_bottom());
        let op_w = 43.0;
        let ch_group_w = fields.width() - op_w - 1.0;
        let kind_rgba = |c: [u8; 4]| c;
        match st.fmt {
            Fmt::Hex => {
                let hex_r = Rect::from_min_size(fields.min, Vec2::new(ch_group_w, 24.0));
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
                if let Some(s) = grouped_text(ui, r, "picker-css", &css_text(cur), theme::cr(theme::RADIUS_CONTROL), 3.0) {
                    typed = parse_css(&s, cur[3]);
                }
            }
            Fmt::Oklch | Fmt::Oklab => {
                let r = Rect::from_min_size(fields.min, Vec2::new(fields.width(), 24.0));
                let disp = st.ok_text(st.fmt);
                if let Some(s) = grouped_ok_text(ui, r, "picker-ok", st, &disp, theme::cr(theme::RADIUS_CONTROL), 3.0) {
                    typed = st.apply_ok_input(st.fmt, &s, cur[3]);
                }
            }
            Fmt::Rgb | Fmt::Hsl | Fmt::Hsb => {
                let vals: [f32; 3] = match st.fmt {
                    Fmt::Rgb => [cur[0] as f32, cur[1] as f32, cur[2] as f32],
                    Fmt::Hsl => rgb_to_hsl(cur),
                    _ => [st.hsv.h * 360.0, st.hsv.s * 100.0, st.hsv.v * 100.0],
                };
                let w3 = (ch_group_w - 2.0) / 3.0;
                let mut edited: Option<(usize, f32)> = None;
                for (i, v) in vals.iter().enumerate() {
                    let r = Rect::from_min_size(Pos2::new(fields.left() + i as f32 * (w3 + 1.0), fields.top()), Vec2::new(w3, 24.0));
                    let radius = match i {
                        0 => theme::cr_each(theme::RADIUS_CONTROL, 0, 0, theme::RADIUS_CONTROL),
                        _ => theme::cr(0),
                    };
                    if let Some(n) = grouped_num(ui, r, ("picker-ch", i), v.round(), radius, 3.0) {
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
                let op_r = Rect::from_min_max(Pos2::new(fields.left() + ch_group_w + 1.0, fields.top()), fields.right_bottom());
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
        if mode.is_gradient() {
            crate::panels::var_bind::picker_stop_hover_button(ui, st, row, st.sel);
        }
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
    let swatch_sense = if stop_bound { Sense::hover() } else { Sense::click() };
    for (i, col) in colors.iter().take(PER_ROW * MAX_ROWS).enumerate() {
        let r = Rect::from_min_size(
            Pos2::new(rect.left() + PAD + (i % PER_ROW) as f32 * SWATCH_PITCH, top + (i / PER_ROW) as f32 * SWATCH_PITCH),
            Vec2::splat(SWATCH),
        );
        let resp = ui.interact(r, ui.id().with(("picker-sw", i)), swatch_sense);
        let p = ui.painter();
        let rad = theme::cr((SWATCH * 0.2).round() as u8);
        if col.a() < 255 {
            checker_dim(&p.with_clip_rect(r), r, 4.0, stop_bound);
        }
        p.rect_filled(r, rad, if stop_bound { kit::half_color(*col) } else { *col });
        if luminance(*col) < 0.3 || col.a() < 255 {
            let stroke_col = theme::BORDER_TRANSLUCENT;
            p.rect_stroke(r, rad, Stroke::new(2.0, if stop_bound { kit::half_color(stroke_col) } else { stroke_col }), StrokeKind::Inside);
        }
        if !stop_bound && resp.hovered() {
            p.rect_stroke(r.expand(1.0), theme::cr(4), Stroke::new(1.0, theme::TEXT_TERTIARY), StrokeKind::Outside);
        }
        if !stop_bound && resp.clicked() {
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
    let mut shot_tip: Option<(Rect, String)> = None;
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
                    let force_tip = shot_tip.is_none() && !s.description.is_empty() && SCREENSHOT_LIBRARIES_HOVER.load(std::sync::atomic::Ordering::Relaxed);
                    if resp.hovered() || force_tip {
                        ui.painter().rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
                    }
                    let c = Pos2::new(r.left() + theme::STYLE_SWATCH_CX, r.center().y);
                    swatch_circle(ui.painter(), c, theme::STYLE_SWATCH / 2.0, &[preview_color(&s.paint)]);
                    let font = theme::font_body_strong();
                    let name = elide(ui.painter(), &s.name, &font, r.width() - 36.0);
                    text_on_baseline(ui.painter(), r.left() + theme::STYLE_LABEL_X, centered_baseline(r, &font), &name, font, theme::TEXT);
                    crate::widgets::record(ui, &format!("picker-style:{}", s.name), r);
                    if !s.description.is_empty() {
                        resp.clone().on_hover_text(&s.description);
                    }
                    if force_tip {
                        shot_tip = Some((r, s.description.clone()));
                    }
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
    if let Some((r, tip_text)) = shot_tip {
        egui::Area::new(egui::Id::new("picker-libraries-hover-tip"))
            .order(egui::Order::Tooltip)
            .fixed_pos(Pos2::new(r.left() + theme::MENU_PADDING, r.bottom()))
            .interactable(false)
            .show(ui.ctx(), |ui| {
                kit::tooltip_frame().show(ui, |ui| {
                    ui.label(tip_text);
                });
            });
    }
}

/// 24px dropdown button with the picker's look (bg + 1px border). Returns the picked index.
fn picker_dropdown(ui: &mut Ui, rect: Rect, key: &str, label: &str, options: &[String], menu_open: &mut bool) -> Option<usize> {
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
    let (left_pad, right_reserve, chev_right) = if key == "picker-fmt" { (4.0, 20.0, 10.0) } else { (8.0, 22.0, 12.0) };
    let font = theme::font_body_strong();
    let shown = elide(p, label, &font, rect.width() - left_pad - right_reserve);
    text_on_baseline(p, rect.left() + left_pad, centered_baseline(rect, &font), &shown, font, theme::TEXT);
    let chev = Rect::from_center_size(Pos2::new(rect.right() - chev_right, rect.center().y), Vec2::splat(12.0));
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

/// The blend modes the editor offers, in Figma's menu order, with their labels.
pub(crate) const BLEND_MODES: [(designos_doc::BlendMode, &str); 18] = {
    use designos_doc::BlendMode as B;
    [
        (B::Normal, "Normal"),
        (B::Darken, "Darken"),
        (B::Multiply, "Multiply"),
        (B::LinearBurn, "Linear burn"),
        (B::ColorBurn, "Color burn"),
        (B::Lighten, "Lighten"),
        (B::Screen, "Screen"),
        (B::LinearDodge, "Linear dodge"),
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
    ]
};

/// Whether Figma's blend menu draws a group divider after `mode`.
pub(crate) fn blend_group_ends(mode: designos_doc::BlendMode) -> bool {
    use designos_doc::BlendMode as B;
    matches!(mode, B::Normal | B::PassThrough | B::ColorBurn | B::ColorDodge | B::HardLight | B::Exclusion)
}

fn blend_dropdown(ui: &mut Ui, rect: Rect, paint: &mut Paint, st: &mut State) -> bool {
    const MODES: [(designos_doc::BlendMode, &str); 18] = BLEND_MODES;
    let id = ui.id().with("picker-blend");
    crate::widgets::record(ui, "picker-blend", rect);
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

/// Steps the OKLCH or OKLAB component under `caret_char` (or to the left of a separator/space;
/// at the start, the first component) by `dir * mult * base_step` and returns
/// `(rewritten_text, new_caret_char)`.
///
/// A plain `L C H` / `L a b` triple is rewritten whole in the field's own format. An `oklch(...)` or
/// `oklab(...)` text keeps its form: the component steps in the function's own space, with that
/// space's steps and clamps, and only its characters change, so the commit converts the function to
/// the field's space the way Enter does.
fn step_ok_text(st: &State, text: &str, caret_char: usize, dir: f32, mult: f32) -> Option<(String, usize)> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let total_chars = chars.len();
    let byte_of = |ci: usize| chars.get(ci).map(|&(b, _)| b).unwrap_or(text.len());
    let mut trim_start = 0;
    while trim_start < total_chars && chars[trim_start].1.is_whitespace() {
        trim_start += 1;
    }
    let mut trim_end = total_chars;
    while trim_end > trim_start && chars[trim_end - 1].1.is_whitespace() {
        trim_end -= 1;
    }
    if trim_start >= trim_end {
        return None;
    }
    let trimmed = &text[byte_of(trim_start)..byte_of(trim_end)];
    let lower = trimmed.to_ascii_lowercase();
    let (active_fmt, is_func, inner_start, inner_end) = if lower.starts_with("oklch") || lower.starts_with("oklab") {
        let af = if lower.starts_with("oklch") { Fmt::Oklch } else { Fmt::Oklab };
        let mut p = trim_start + 5;
        while p < trim_end && chars[p].1.is_whitespace() {
            p += 1;
        }
        if p >= trim_end || chars[p].1 != '(' || chars[trim_end - 1].1 != ')' {
            return None;
        }
        (af, true, p + 1, trim_end - 1)
    } else {
        (st.fmt, false, trim_start, trim_end)
    };
    let mut slash_pos = None;
    for (ci, &(_, ch)) in chars.iter().enumerate().take(inner_end).skip(inner_start) {
        if ch == '/' {
            if slash_pos.is_some() {
                return None;
            }
            slash_pos = Some(ci);
        }
    }
    let (ch_end, alpha_suffix) = if let Some(sp) = slash_pos {
        if caret_char > sp {
            return None;
        }
        let a_str = text[byte_of(sp + 1)..byte_of(inner_end)].trim();
        if a_str.is_empty() {
            return None;
        }
        (sp, format!(" / {a_str}"))
    } else {
        (inner_end, String::new())
    };
    let mut tokens: Vec<(usize, usize, &str)> = Vec::with_capacity(3);
    let mut i = inner_start;
    while i < ch_end {
        while i < ch_end && (chars[i].1.is_ascii_whitespace() || chars[i].1 == ',') {
            i += 1;
        }
        if i >= ch_end {
            break;
        }
        let start = i;
        while i < ch_end && !chars[i].1.is_ascii_whitespace() && chars[i].1 != ',' {
            i += 1;
        }
        tokens.push((start, i, &text[byte_of(start)..byte_of(i)]));
    }
    if tokens.len() != 3 {
        return None;
    }
    let cur_triple = st.ok_triple(active_fmt);
    let idx = if caret_char < tokens[1].0 {
        0
    } else if caret_char < tokens[2].0 {
        1
    } else {
        2
    };
    let (si, ei, _) = tokens[idx];
    let rel = caret_char.saturating_sub(si).min(ei - si);
    let mut parsed = [0.0_f32; 3];
    for (k, &(_, _, tok)) in tokens.iter().enumerate() {
        parsed[k] = if is_func && k == 0 && !tok.ends_with('%') {
            if tok.contains('%') {
                return None;
            }
            field::eval_number_expr(tok, Some(cur_triple[0]), false)?.clamp(0.0, 1.0)
        } else {
            parse_ok_channel(active_fmt, k, tok, cur_triple[k])?
        };
    }
    let stepped = stepped_ok_token(active_fmt, idx, parsed[idx], dir, mult);
    if is_func {
        let next_text = format!("{}{}{}", &text[..byte_of(si)], stepped, &text[byte_of(ei)..]);
        return Some((next_text, si + rel.min(stepped.chars().count())));
    }
    let out_toks: [String; 3] = std::array::from_fn(|k| {
        if k == idx {
            stepped.clone()
        } else {
            let tok = tokens[k].2;
            if tok == fmt_ok_token(k, cur_triple[k]) || tok == fmt_ok_channel(k, cur_triple[k]) {
                fmt_ok_token(k, cur_triple[k])
            } else if k == 0 {
                format!("{}%", fmt_num(parsed[0] * 100.0))
            } else if k == 2 && active_fmt == Fmt::Oklch {
                fmt_num(parsed[2])
            } else {
                designos_export::util::num(parsed[k])
            }
        }
    });
    let next_text = format!("{} {} {}{}", out_toks[0], out_toks[1], out_toks[2], alpha_suffix);
    let len0 = out_toks[0].chars().count();
    let len1 = out_toks[1].chars().count();
    let len2 = out_toks[2].chars().count();
    let (new_start, new_len) = match idx {
        0 => (0, len0),
        1 => (len0 + 1, len1),
        _ => (len0 + 1 + len1 + 1, len2),
    };
    let new_caret = new_start + rel.min(new_len);
    Some((next_text, new_caret))
}

/// Component `k` of `fmt`'s triple, whose parsed value is `value` (`L` in `0..=1`), stepped by `dir * mult`
/// base steps (L 1 point, C / a / b 0.01, H 1 degree), clamped (H wraps), as field text.
fn stepped_ok_token(fmt: Fmt, k: usize, value: f32, dir: f32, mult: f32) -> String {
    match (fmt, k) {
        (Fmt::Oklch | Fmt::Oklab, 0) => {
            let l_pct = value * 100.0;
            let next_pct = (((l_pct + dir * (1.0 * mult)) * 100.0).round() / 100.0).clamp(0.0, 100.0);
            format!("{}%", fmt_num(next_pct))
        }
        (Fmt::Oklch, 1) => {
            let next_c = (((value + dir * (0.01 * mult)) * 1000.0).round() / 1000.0).clamp(0.0, 0.4);
            designos_export::util::num(next_c)
        }
        (Fmt::Oklch, 2) => {
            let raw_h = ((value + dir * (1.0 * mult)) * 100.0).round() / 100.0;
            let mut next_h = ((raw_h.rem_euclid(360.0)) * 100.0).round() / 100.0;
            if next_h >= 360.0 || next_h == 0.0 {
                next_h = 0.0;
            }
            fmt_num(next_h)
        }
        (Fmt::Oklab, 1 | 2) => {
            let next_ab = (((value + dir * (0.01 * mult)) * 1000.0).round() / 1000.0).clamp(-0.4, 0.4);
            designos_export::util::num(next_ab)
        }
        _ => fmt_ok_token(k, value),
    }
}

/// Keeps the keyboard focus on the field `id` when an arrow key goes to it, where egui would move the focus
/// to the next widget. Escape is not part of the filter: it drops the focus as in every other field, so the
/// shell's Escape action still runs.
fn keep_focus_on_arrows(ctx: &egui::Context, id: Id) {
    ctx.memory_mut(|m| {
        m.move_focus(egui::FocusDirection::None);
        m.set_focus_lock_filter(id, egui::EventFilter { horizontal_arrows: true, vertical_arrows: true, ..Default::default() });
    });
}

/// OKLCH / OKLAB field cell with `ArrowUp`/`ArrowDown` (`×1`, `Shift ×10`, `Alt ×0.1`) per-component stepping.
fn grouped_ok_text(ui: &mut Ui, r: Rect, key: impl Key, st: &State, display: &str, radius: egui::CornerRadius, pad: f32) -> Option<String> {
    let id = ui.id().with(("txt", key));
    let focused = ui.memory(|m| m.has_focus(id));
    ui.painter().rect_filled(r, radius, theme::BG_SECONDARY);
    if focused {
        ui.painter().rect_stroke(r, radius, Stroke::new(1.0, theme::BORDER_SELECTED), StrokeKind::Inside);
    } else if ui.rect_contains_pointer(r) {
        ui.painter().rect_stroke(r, radius, Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);
    }
    let step_id = id.with("ok_step");
    let mut stepped: Option<(String, egui::text::CCursorRange)> = None;
    if focused {
        let mut arrow: Option<(f32, egui::Modifiers)> = None;
        ui.input_mut(|i| {
            if let Some(pos) = i.events.iter().position(|e| matches!(e, egui::Event::Key { key: egui::Key::ArrowUp | egui::Key::ArrowDown, pressed: true, .. }))
            {
                if let egui::Event::Key { key, modifiers, .. } = i.events.remove(pos) {
                    let dir = if key == egui::Key::ArrowUp { 1.0 } else { -1.0 };
                    arrow = Some((dir, modifiers));
                }
            }
        });
        if let Some((dir, m)) = arrow {
            let mult = if m.alt {
                0.1
            } else if m.shift {
                10.0
            } else {
                1.0
            };
            let cur_buf = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| display.to_string());
            let caret_char = egui::widgets::text_edit::TextEdit::load_state(ui.ctx(), id)
                .and_then(|s| s.cursor.char_range())
                .map(|r| r.primary.index.0.min(r.secondary.index.0))
                .unwrap_or(0);
            if let Some((next_text, next_caret)) = step_ok_text(st, &cur_buf, caret_char, dir, mult) {
                let range = egui::text::CCursorRange::one(egui::text::CCursor::new(next_caret));
                stepped = Some((next_text, range));
            }
        }
    } else {
        ui.data_mut(|d| d.remove_temp::<(Fmt, String, String)>(step_id));
    }
    if let Some((next_text, next_range)) = stepped {
        let mut preview = st.clone();
        let _ = preview.apply_ok_input(st.fmt, &next_text, 255);
        let next_disp = preview.ok_text(st.fmt);
        ui.data_mut(|d| {
            d.insert_temp(id, next_text.clone());
            d.insert_temp(id.with("seed"), next_text.clone());
            d.insert_temp(step_id, (st.fmt, next_disp, next_text.clone()));
        });
        let res = field::edit_text(ui, r, id, &next_text, theme::font_body(), theme::TEXT, pad);
        ui.memory_mut(|m| m.request_focus(id));
        keep_focus_on_arrows(ui.ctx(), id);
        let mut state = egui::widgets::text_edit::TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
        state.cursor.set_char_range(Some(next_range));
        state.store(ui.ctx(), id);
        return res.or(Some(next_text));
    }
    let live_display = ui.data(|d| d.get_temp::<(Fmt, String, String)>(step_id)).and_then(|(saved_fmt, saved_disp, saved_text)| {
        if focused && saved_fmt == st.fmt && saved_disp == display {
            Some(saved_text)
        } else {
            None
        }
    });
    let eff_display = live_display.as_deref().unwrap_or(display);
    let res = field::edit_text(ui, r, id, eff_display, theme::font_body(), theme::TEXT, pad);
    if ui.memory(|m| m.has_focus(id)) {
        keep_focus_on_arrows(ui.ctx(), id);
    }
    res
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
    let res = field::edit_num_text(ui, text_r, id, Some(pct_val), true, theme::font_body(), theme::TEXT, 4.0, None);
    let font = theme::font_body();
    text_on_baseline(ui.painter(), r.right() - 4.0 - text_width(ui.painter(), "%", &font), centered_baseline(r, &font), "%", font, theme::TEXT_SECONDARY);
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
                    if st.stop_origins.len() != s.len() {
                        st.stop_origins = (0..s.len()).map(Some).collect();
                    }
                    let c = stop_color_at(s, t);
                    let mut pairs: Vec<(GradientStop, Option<usize>)> = s.iter().copied().zip(st.stop_origins.iter().copied()).collect();
                    pairs.push((GradientStop { pos: t, color: c }, None));
                    pairs.sort_by(|a, b| a.0.pos.total_cmp(&b.0.pos));
                    st.sel = pairs.iter().position(|(x, orig)| orig.is_none() && x.pos == t).unwrap_or(0);
                    *s = pairs.iter().map(|(x, _)| *x).collect();
                    st.stop_origins = pairs.into_iter().map(|(_, orig)| orig).collect();
                    out.changed = true;
                    out.stop_list_changed = true;
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
        p.rect_stroke(sq.expand(1.0), theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, theme::CONTENT_HANDLE_RING), StrokeKind::Outside);
        p.rect_filled(sq, theme::cr(4), theme::CONTENT_HANDLE_FILL);
        let inner = sq.shrink(if selected { 3.0 } else { 2.5 });
        checker(&p.with_clip_rect(inner), inner, 3.0);
        p.rect_filled(inner, theme::cr(theme::RADIUS_MARK), c32(to_u8(s.color, 1.0)));
        if selected {
            p.rect_stroke(sq.expand(2.0), theme::cr(theme::RADIUS_SEGMENT), Stroke::new(1.0, theme::BORDER_SELECTED), StrokeKind::Outside);
        }
    }
    let del = ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) && !ui.memory(|m| m.focused().is_some());
    if del && stops.len() > 2 {
        remove = Some(st.sel);
    }
    if let Some(i) = remove {
        if let Some(sv) = stops_mut(&mut paint.kind) {
            if sv.len() > 2 && i < sv.len() {
                if st.stop_origins.len() != sv.len() {
                    st.stop_origins = (0..sv.len()).map(Some).collect();
                }
                sv.remove(i);
                st.stop_origins.remove(i);
                ui.ctx().stop_dragging();
                st.sel = st.sel.saturating_sub(1).min(sv.len() - 1);
                out.changed = true;
                out.stop_list_changed = true;
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
        assert_eq!(parse_css("rgb(10 20 30 / 25%)", 255), Some([10, 20, 30, 64]));
        assert_eq!(parse_css("rgba(10, 20, 30, 50%)", 255), Some([10, 20, 30, 128]));
        assert_eq!(parse_css("hsl(240 100% 50% / 50%)", 255), Some([0, 0, 255, 128]));
        assert_eq!(parse_css("nonsense", 255), None);
        assert_eq!(css_text([217, 217, 217, 255]), "rgb(217, 217, 217)");
        assert_eq!(css_text([217, 217, 217, 128]), "rgb(217, 217, 217)");
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

    /// Every shape of one picker frame, flattened.
    fn picker_shapes(paint: &mut Paint) -> Vec<egui::Shape> {
        fn flatten(shape: &egui::Shape, out: &mut Vec<egui::Shape>) {
            match shape {
                egui::Shape::Vec(v) => v.iter().for_each(|s| flatten(s, out)),
                other => out.push(other.clone()),
            }
        }
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let mut st = State::new(paint);
        let mut raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 700.0))), ..Default::default() };
        raw.time = Some(0.0);
        let sources: Vec<(String, Vec<Color32>)> = vec![("On this page".into(), vec![])];
        let env = Env { paint_types: true, sources: &sources, styles: &[], can_add_style: false };
        let full = ctx.run_ui(raw, |ui| {
            let _ = show(ui, Pos2::ZERO, &mut st, paint, &env);
        });
        let mut out = Vec::new();
        for cs in &full.shapes {
            flatten(&cs.shape, &mut out);
        }
        out
    }

    fn text_color_of(shapes: &[egui::Shape], text: &str) -> Option<Color32> {
        shapes.iter().find_map(|s| match s {
            egui::Shape::Text(t) if t.galley.text() == text => t.galley.job.sections.first().map(|sec| sec.format.color),
            _ => None,
        })
    }

    fn has_badge_fill(shapes: &[egui::Shape], fill: Color32) -> bool {
        shapes.iter().any(|s| matches!(s, egui::Shape::Rect(r) if r.fill == fill && (r.rect.height() - 20.0).abs() < 0.5 && r.rect.width() < 70.0))
    }

    fn has_badge_edge(shapes: &[egui::Shape], edge: Color32) -> bool {
        shapes.iter().any(|s| matches!(s, egui::Shape::Rect(r) if r.stroke.color == edge && (r.rect.height() - 20.0).abs() < 0.5 && r.rect.width() < 70.0))
    }

    #[test]
    fn contrast_badges_use_status_roles() {
        // (grey level, badge text, background, border-free text colour)
        let rows = [
            (255u8, "AAA", theme::BG_SUCCESS_PRIMARY, theme::BADGE_SUCCESS_TEXT, theme::BADGE_SUCCESS_BORDER),
            (0x80, "AA Large", theme::BG_WARNING_PRIMARY, theme::BADGE_WARNING_TEXT, theme::BADGE_WARNING_BORDER),
            (0x9A, "Low", theme::BG_ERROR_PRIMARY, theme::BADGE_ERROR_TEXT, theme::BADGE_ERROR_BORDER),
        ];
        for (g, label, bg, fg, edge) in rows {
            let shapes = picker_shapes(&mut solid(g, g, g, 1.0));
            assert!(has_badge_fill(&shapes, bg), "{label}: badge background is the status role");
            assert!(has_badge_edge(&shapes, edge), "{label}: badge border is the status role");
            assert_eq!(text_color_of(&shapes, label), Some(fg), "{label}: badge text is the status role");
            let ratio = shapes.iter().find_map(|s| match s {
                egui::Shape::Text(t) if t.galley.text().ends_with(" : 1") => t.galley.job.sections.first().map(|sec| sec.format.color),
                _ => None,
            });
            assert_eq!(ratio, Some(theme::TEXT), "{label}: ratio text is TEXT");
        }
        // AA shares the success roles with AAA.
        let aa = (0..=255u8).find(|g| {
            let l = relative_luminance([*g, *g, *g]);
            let ratio = wcag_contrast_ratio(l, relative_luminance([0x1E, 0x1E, 0x1E]));
            l >= 0.35 && (4.5..7.0).contains(&ratio)
        });
        {
            let g = aa.expect("a grey in the AA band");
            let shapes = picker_shapes(&mut solid(g, g, g, 1.0));
            assert!(has_badge_fill(&shapes, theme::BG_SUCCESS_PRIMARY));
            assert!(has_badge_edge(&shapes, theme::BADGE_SUCCESS_BORDER));
            assert_eq!(text_color_of(&shapes, "AA"), Some(theme::BADGE_SUCCESS_TEXT));
        }
    }

    #[test]
    fn track_handles_keep_a_white_fill_and_a_dark_ring() {
        let stops = vec![GradientStop { pos: 0.0, color: Color::BLACK }, GradientStop { pos: 1.0, color: Color::WHITE }];
        let mut paint = Paint { kind: PaintKind::LinearGradient { stops, handles: default_handles(Mode::Linear) }, ..Paint::solid(Color::BLACK) };
        let shapes = picker_shapes(&mut paint);
        let handle = |s: &egui::Shape| matches!(s, egui::Shape::Rect(r) if r.fill == theme::CONTENT_HANDLE_FILL && (r.rect.width() - 12.0).abs() < 0.5 && (r.rect.height() - 16.0).abs() < 0.5);
        assert!(shapes.iter().filter(|s| handle(s)).count() >= 2, "one white 12x16 handle per stop");
        let ring = |s: &egui::Shape| matches!(s, egui::Shape::Rect(r) if r.stroke.color == theme::CONTENT_HANDLE_RING && (r.rect.width() - 14.0).abs() < 0.5);
        assert!(shapes.iter().filter(|s| ring(s)).count() >= 2, "a dark ring around each handle");
        // The colour area, hue and alpha thumbs are rings: a white stroke over a dark casing.
        let circle_stroke = |color: Color32| shapes.iter().filter(|s| matches!(s, egui::Shape::Circle(c) if c.stroke.color == color)).count();
        let solid_shapes = picker_shapes(&mut solid(200, 40, 40, 1.0));
        let thumbs = |color: Color32| solid_shapes.iter().filter(|s| matches!(s, egui::Shape::Circle(c) if c.stroke.color == color)).count();
        assert!(thumbs(theme::CONTENT_HANDLE_FILL) >= 3, "white ring on the area, hue and alpha thumbs");
        assert!(thumbs(theme::CONTENT_HANDLE_RING) >= 3, "dark casing around the area, hue and alpha thumbs");
        assert!(circle_stroke(theme::BG_BRAND) == 0, "the orange slider handle is never used on a track");
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

    fn test_ctx() -> egui::Context {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::theme::install_fonts(&ctx);
        ctx
    }

    fn run_picker_frame(ctx: &egui::Context, events: Vec<egui::Event>, paint: &mut Paint, st: &mut State) -> Out {
        let mut raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 600.0))), events, ..Default::default() };
        raw.time = Some(0.0);
        let mut out = Out::default();
        let sources: Vec<(String, Vec<Color32>)> = vec![("On this page".into(), vec![])];
        let env = Env { paint_types: true, sources: &sources, styles: &[], can_add_style: false };
        let _ = ctx.run_ui(raw, |ui| {
            let (_, o) = show(ui, Pos2::ZERO, st, paint, &env);
            out = o;
        });
        out
    }

    fn collect_visible_texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
        fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                egui::Shape::Text(t) => {
                    let col = t.override_text_color.or_else(|| t.galley.job.sections.first().map(|s| s.format.color)).unwrap_or(t.fallback_color);
                    if col.a() > 0 {
                        out.push(t.galley.text().to_string());
                    }
                }
                _ => {}
            }
        }
        let mut out = Vec::new();
        for cs in shapes {
            walk(&cs.shape, &mut out);
        }
        out
    }

    fn visible_texts(ctx: &egui::Context, paint: &mut Paint, st: &mut State) -> Vec<String> {
        let mut raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 600.0))), ..Default::default() };
        raw.time = Some(0.0);
        let sources: Vec<(String, Vec<Color32>)> = vec![("On this page".into(), vec![])];
        let env = Env { paint_types: true, sources: &sources, styles: &[], can_add_style: false };
        let full = ctx.run_ui(raw, |ui| {
            let _ = show(ui, Pos2::ZERO, st, paint, &env);
        });
        collect_visible_texts(&full.shapes)
    }

    fn closed_format_dropdown_fits_all_labels(ctx: &egui::Context, paint: &mut Paint, st: &mut State) -> bool {
        let mut raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 600.0))), ..Default::default() };
        raw.time = Some(0.0);
        let sources: Vec<(String, Vec<Color32>)> = vec![("On this page".into(), vec![])];
        let env = Env { paint_types: true, sources: &sources, styles: &[], can_add_style: false };
        let mut max_label_w = 0.0_f32;
        let full = ctx.run_ui(raw, |ui| {
            let p = ui.painter();
            let font = theme::font_body_strong();
            for f in Fmt::ALL {
                max_label_w = max_label_w.max(text_width(p, f.label(), &font));
            }
            let _ = show(ui, Pos2::ZERO, st, paint, &env);
        });
        fn walk_rects(shape: &egui::Shape, out: &mut Vec<Rect>) {
            match shape {
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk_rects(s, out)),
                egui::Shape::Rect(r) => out.push(r.rect),
                _ => {}
            }
        }
        let mut rects = Vec::new();
        for cs in &full.shapes {
            walk_rects(&cs.shape, &mut rects);
        }
        let dd_w = rects
            .into_iter()
            .find(|r| (r.min.x - PAD).abs() < 0.5 && (r.height() - 24.0).abs() < 0.5 && r.width() > 30.0 && r.width() < 100.0)
            .map(|r| r.width())
            .unwrap_or(0.0);
        dd_w >= (max_label_w + 24.0).ceil()
    }

    fn commit_picker_ok(ctx: &egui::Context, text: &str, paint: &mut Paint, st: &mut State) -> Out {
        let _ = run_picker_frame(ctx, vec![], paint, st);
        let mut cell_id = None;
        let sources: Vec<(String, Vec<Color32>)> = vec![("On this page".into(), vec![])];
        let env = Env { paint_types: true, sources: &sources, styles: &[], can_add_style: false };
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let id = ui.id().with(("txt", "picker-ok"));
            cell_id = Some(id);
            let _ = show(ui, Pos2::ZERO, st, paint, &env);
            ui.memory_mut(|m| m.request_focus(id));
        });
        let id = cell_id.unwrap();
        let _ = run_picker_frame(ctx, vec![], paint, st);
        ctx.data_mut(|d| d.insert_temp(id, text.to_string()));
        run_picker_frame(
            ctx,
            vec![egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }],
            paint,
            st,
        )
    }

    fn assert_visible_mode(ctx: &egui::Context, paint: &mut Paint, st: &mut State, expected: &[&str]) {
        let texts = visible_texts(ctx, paint, st);
        assert!(texts.iter().all(|t| !t.contains('…')), "mode {:?} painted an elided string in visible texts: {texts:?}", st.fmt);
        for &exp in expected {
            assert!(texts.iter().any(|t| t == exp), "mode {:?} missing expected visible text {exp:?} in {texts:?}", st.fmt);
        }
        assert!(closed_format_dropdown_fits_all_labels(ctx, paint, st), "mode {:?} closed format dropdown is too narrow for longest label", st.fmt);
    }

    #[test]
    fn oklch_and_oklab_format_list_and_default() {
        assert_eq!(Fmt::ALL, [Fmt::Hex, Fmt::Rgb, Fmt::Css, Fmt::Hsl, Fmt::Hsb, Fmt::Oklch, Fmt::Oklab]);
        assert_eq!(Fmt::Oklch.label(), "OKLCH");
        assert_eq!(Fmt::Oklab.label(), "OKLAB");
        let st = State::new(&solid(255, 0, 0, 1.0));
        assert_eq!(st.fmt, Fmt::Hex);
    }

    #[test]
    fn red_shows_expected_oklch_and_oklab_cells() {
        let mut paint = solid(255, 0, 0, 1.0);
        let mut st = State::new(&paint);
        assert_eq!(st.ok_cells(Fmt::Oklch), ["62.8", "0.26", "29.23"]);
        assert_eq!(st.ok_cells(Fmt::Oklab), ["62.8", "0.22", "0.13"]);

        let ctx = test_ctx();
        st.fmt = Fmt::Oklch;
        assert_visible_mode(&ctx, &mut paint, &mut st, &["OKLCH", "62.8% 0.26 29.23"]);

        st.fmt = Fmt::Oklab;
        assert_visible_mode(&ctx, &mut paint, &mut st, &["OKLAB", "62.8% 0.22 0.13"]);

        let mut half = solid(255, 0, 0, 0.5);
        let mut st_half = State::new(&half);
        st_half.fmt = Fmt::Oklch;
        assert_visible_mode(&ctx, &mut half, &mut st_half, &["OKLCH", "62.8% 0.26 29.23"]);
        st_half.fmt = Fmt::Oklab;
        assert_visible_mode(&ctx, &mut half, &mut st_half, &["OKLAB", "62.8% 0.22 0.13"]);
    }

    #[test]
    fn visible_text_hex_mode() {
        let ctx = test_ctx();
        let mut paint = solid(255, 0, 0, 1.0);
        let mut st = State::new(&paint);
        st.fmt = Fmt::Hex;
        assert_visible_mode(&ctx, &mut paint, &mut st, &["Hex", "FF0000", "100", "%"]);
    }

    #[test]
    fn visible_text_rgb_at_255_255_255() {
        let ctx = test_ctx();
        let mut paint = solid(255, 255, 255, 1.0);
        let mut st = State::new(&paint);
        st.fmt = Fmt::Rgb;
        assert_visible_mode(&ctx, &mut paint, &mut st, &["RGB", "255", "100", "%"]);
        let texts = visible_texts(&ctx, &mut paint, &mut st);
        assert_eq!(texts.iter().filter(|t| t.as_str() == "255").count(), 3, "expected three 255 cells in {texts:?}");
    }

    #[test]
    fn visible_text_hsl_mode() {
        let ctx = test_ctx();
        let mut paint = solid(255, 0, 0, 1.0);
        let mut st = State::new(&paint);
        st.fmt = Fmt::Hsl;
        assert_visible_mode(&ctx, &mut paint, &mut st, &["HSL", "0", "100", "50", "%"]);
        let texts = visible_texts(&ctx, &mut paint, &mut st);
        assert_eq!(texts.iter().filter(|t| t.as_str() == "100").count(), 2, "expected S=100 and opacity=100 in {texts:?}");
    }

    #[test]
    fn visible_text_hsb_at_full_s_b() {
        let ctx = test_ctx();
        let mut paint = solid(255, 0, 0, 1.0);
        let mut st = State::new(&paint);
        st.fmt = Fmt::Hsb;
        assert_visible_mode(&ctx, &mut paint, &mut st, &["HSB", "0", "100", "%"]);
        let texts = visible_texts(&ctx, &mut paint, &mut st);
        assert_eq!(texts.iter().filter(|t| t.as_str() == "100").count(), 3, "expected S=100, B=100 and opacity=100 in {texts:?}");

        st.hsv.h = 1.0;
        assert_visible_mode(&ctx, &mut paint, &mut st, &["HSB", "360", "100", "%"]);
    }

    #[test]
    fn visible_text_css_mode() {
        let ctx = test_ctx();
        let mut paint = solid(255, 0, 0, 1.0);
        let mut st = State::new(&paint);
        st.fmt = Fmt::Css;
        assert_visible_mode(&ctx, &mut paint, &mut st, &["CSS", "rgb(255, 0, 0)"]);

        let mut white = solid(255, 255, 255, 1.0);
        let mut st_w = State::new(&white);
        st_w.fmt = Fmt::Css;
        assert_visible_mode(&ctx, &mut white, &mut st_w, &["CSS", "rgb(255, 255, 255)"]);
    }

    #[test]
    fn visible_text_oklch_mode() {
        let ctx = test_ctx();
        let mut paint = solid(255, 0, 0, 1.0);
        let mut st = State::new(&paint);
        st.fmt = Fmt::Oklch;
        assert_visible_mode(&ctx, &mut paint, &mut st, &["OKLCH", "62.8% 0.26 29.23"]);
    }

    #[test]
    fn visible_text_oklab_mode() {
        let ctx = test_ctx();
        let mut paint = solid(255, 0, 0, 1.0);
        let mut st = State::new(&paint);
        st.fmt = Fmt::Oklab;
        assert_visible_mode(&ctx, &mut paint, &mut st, &["OKLAB", "62.8% 0.22 0.13"]);
    }

    #[test]
    fn value_cell_at_rest_has_no_alpha_and_never_elides_across_grid() {
        let ctx = test_ctx();
        for alpha in [128_u8, 140_u8] {
            let op = alpha as f32 / 255.0;
            for [r, g, b] in [[52, 120, 200], [255, 0, 0], [204, 204, 204], [51, 17, 204], [34, 85, 68]] {
                for fmt in Fmt::ALL {
                    let mut paint = solid(r, g, b, op);
                    let mut st = State::new(&paint);
                    st.fmt = fmt;
                    let texts = visible_texts(&ctx, &mut paint, &mut st);
                    assert!(texts.iter().all(|t| !t.contains('…')), "mode {fmt:?} elided at [{r},{g},{b},{alpha}]: {texts:?}");
                    assert!(texts.iter().all(|t| !t.contains('/') && !t.starts_with("rgba(")), "mode {fmt:?} showed alpha in value cell at rest: {texts:?}");
                }
            }
        }

        let mut raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 600.0))), ..Default::default() };
        raw.time = Some(0.0);
        let _ = ctx.run_ui(raw, |ui| {
            let p = ui.painter();
            let font = theme::font_body();
            let single_budget = 138.0 - 6.0;
            let hex_budget = 94.0 - 16.0;
            let ch_budget = 30.0 - 6.0;
            for r_step in 0..16_u8 {
                let r = r_step * 17;
                for g_step in 0..16_u8 {
                    let g = g_step * 17;
                    for b_step in 0..16_u8 {
                        let b = b_step * 17;
                        let mut st = State::new(&solid(r, g, b, 1.0));
                        st.sync([r, g, b, 255]);
                        for alpha in [128_u8, 140_u8] {
                            let lch = st.ok_text(Fmt::Oklch);
                            let lab = st.ok_text(Fmt::Oklab);
                            let css = css_text([r, g, b, alpha]);
                            assert!(!lch.contains('/'), "OKLCH at-rest text must omit alpha: {lch}");
                            assert!(!lab.contains('/'), "OKLAB at-rest text must omit alpha: {lab}");
                            assert!(!css.starts_with("rgba("), "CSS at-rest text must omit alpha: {css}");
                            assert!(text_width(p, &lch, &font) <= single_budget, "OKLCH overflow: {lch}");
                            assert!(text_width(p, &lab, &font) <= single_budget, "OKLAB overflow: {lab}");
                            assert!(text_width(p, &css, &font) <= single_budget, "CSS overflow: {css}");
                            let hex = Color::rgba(r, g, b, 255).to_hex();
                            assert!(text_width(p, &hex, &font) <= hex_budget, "Hex overflow: {hex}");
                            let hsl = rgb_to_hsl([r, g, b, alpha]);
                            let hsb = [st.hsv.h * 360.0, st.hsv.s * 100.0, st.hsv.v * 100.0];
                            for v in [r as f32, g as f32, b as f32, hsl[0], hsl[1], hsl[2], hsb[0], hsb[1], hsb[2]] {
                                let s = fmt_num(v.round());
                                assert!(text_width(p, &s, &font) <= ch_budget, "channel overflow: {s}");
                            }
                        }
                    }
                }
            }
        });
    }

    #[test]
    fn parse_oklch_and_oklab_channel_rules_and_clamps() {
        // C percentage and relative percentage expressions
        let c_pct = parse_ok_channel(Fmt::Oklch, 1, "50%", 0.1).expect("C 50%");
        assert!((c_pct - 0.2).abs() < 1e-6, "expected 0.2, got {c_pct}");
        let c_rel = parse_ok_channel(Fmt::Oklch, 1, "+=10%", 0.1).expect("C +=10%");
        assert!((c_rel - 0.14).abs() < 1e-6, "expected 0.14, got {c_rel}");

        // OKLAB a and b percentage mapping
        let a_pct = parse_ok_channel(Fmt::Oklab, 1, "50%", 0.0).expect("a 50%");
        let b_pct = parse_ok_channel(Fmt::Oklab, 2, "-50%", 0.0).expect("b -50%");
        assert!((a_pct - 0.2).abs() < 1e-6, "expected 0.2, got {a_pct}");
        assert!((b_pct - (-0.2)).abs() < 1e-6, "expected -0.2, got {b_pct}");

        // C 0.2577 reaches the conversion at full f32 precision (0.2577, not 0.26)
        let c_raw = parse_ok_channel(Fmt::Oklch, 1, "0.2577", 0.1).expect("C 0.2577");
        assert!((c_raw - 0.2577).abs() < 1e-7, "expected 0.2577, got {c_raw}");
        let mut st = State::new(&solid(255, 0, 0, 1.0));
        let _ = st.set_oklch_channel(1, c_raw);
        assert!((st.oklch[1] - 0.2577).abs() < 1e-7);

        // L 50 and L 50% -> 0.5
        for fmt in [Fmt::Oklch, Fmt::Oklab] {
            let l_plain = parse_ok_channel(fmt, 0, "50", 0.6).expect("L 50");
            let l_pct = parse_ok_channel(fmt, 0, "50%", 0.6).expect("L 50%");
            assert!((l_plain - 0.5).abs() < 1e-6);
            assert!((l_pct - 0.5).abs() < 1e-6);
        }

        // H wraps modulo 360 and accepts deg or degree symbol
        let h_wrap = parse_ok_channel(Fmt::Oklch, 2, "390", 0.0).expect("H 390");
        let h_deg = parse_ok_channel(Fmt::Oklch, 2, "30deg", 0.0).expect("H 30deg");
        let h_sym = parse_ok_channel(Fmt::Oklch, 2, "30°", 0.0).expect("H 30°");
        assert!((h_wrap - 30.0).abs() < 1e-6);
        assert!((h_deg - 30.0).abs() < 1e-6);
        assert!((h_sym - 30.0).abs() < 1e-6);

        // Clamps for C, a, b, and L
        let c_clamp = parse_ok_channel(Fmt::Oklch, 1, "0.9", 0.1).expect("C 0.9");
        let a_clamp = parse_ok_channel(Fmt::Oklab, 1, "-1", 0.0).expect("a -1");
        let b_clamp = parse_ok_channel(Fmt::Oklab, 2, "1", 0.0).expect("b 1");
        let l_clamp = parse_ok_channel(Fmt::Oklch, 0, "150", 0.5).expect("L 150");
        assert!((c_clamp - 0.4).abs() < 1e-6);
        assert!((a_clamp - (-0.4)).abs() < 1e-6);
        assert!((b_clamp - 0.4).abs() < 1e-6);
        assert!((l_clamp - 1.0).abs() < 1e-6);

        // Arithmetic expressions delegate to field::eval_number_expr
        let l_expr = parse_ok_channel(Fmt::Oklch, 0, "20 + 30", 0.6).expect("L 20 + 30");
        let c_expr = parse_ok_channel(Fmt::Oklch, 1, "0.1 + 0.15", 0.1).expect("C 0.1 + 0.15");
        let h_expr = parse_ok_channel(Fmt::Oklch, 2, "360 + 30", 0.0).expect("H 360 + 30");
        assert!((l_expr - 0.5).abs() < 1e-6);
        assert!((c_expr - 0.25).abs() < 1e-6);
        assert!((h_expr - 30.0).abs() < 1e-6);

        // Single-cell pasted oklch(...) and oklab(...) with deg and / alpha
        let ctx = test_ctx();
        let mut paint = solid(0, 0, 0, 1.0);
        let mut st_cell = State::new(&paint);
        st_cell.fmt = Fmt::Oklch;
        let out_paste = commit_picker_ok(&ctx, "oklch(62.8% 0.26 29.23deg / 50%)", &mut paint, &mut st_cell);
        assert!(out_paste.changed, "pasting oklch(...) must commit colour and alpha");
        assert_eq!(active_rgba(&paint, 0)[3], 128);
    }

    #[test]
    fn alpha_only_or_retyped_channels_preserve_rgb_and_alpha_precision() {
        let ctx = test_ctx();
        let mut paint = solid(52, 120, 200, 1.0);
        let mut st = State::new(&paint);
        st.fmt = Fmt::Oklch;
        assert_eq!(st.ok_text(Fmt::Oklch), "56.93% 0.14 254.47");
        let orig_oklch = st.oklch;

        // Retyping the shown channels with No alpha change is a no-op and keeps exact channels.
        let out_noop = commit_picker_ok(&ctx, "56.93%, 0.14, 254.47", &mut paint, &mut st);
        assert!(!out_noop.changed, "retyping unchanged displayed channels must not mark out.changed");
        assert_eq!(active_rgba(&paint, 0), [52, 120, 200, 255]);
        assert_eq!(st.oklch, orig_oklch);

        // Appending / 0.5 to the shown OKLCH string updates alpha only and keeps [52, 120, 200].
        let out_alpha = commit_picker_ok(&ctx, "56.93% 0.14 254.47 / 0.5", &mut paint, &mut st);
        assert!(out_alpha.changed, "appending / 0.5 must update alpha");
        assert_eq!(active_rgba(&paint, 0), [52, 120, 200, 128]);
        assert_eq!(st.oklch, orig_oklch);

        // Retyping bare L (without %) and / 0.5 also preserves the unchanged channels.
        set_active(&mut paint, 0, [52, 120, 200, 255]);
        st.sync([52, 120, 200, 255]);
        let out_bare_l = commit_picker_ok(&ctx, "56.93 0.14 254.47 / 0.5", &mut paint, &mut st);
        assert!(out_bare_l.changed);
        assert_eq!(active_rgba(&paint, 0), [52, 120, 200, 128]);
        assert_eq!(st.oklch, orig_oklch);

        // Editing L while keeping the 2-decimal alpha "/ 0.55" preserves alpha = 141 (not 140).
        set_active(&mut paint, 0, [52, 120, 200, 141]);
        st.sync([52, 120, 200, 141]);
        let out_keep_a = commit_picker_ok(&ctx, "70% 0.14 254.47 / 0.55", &mut paint, &mut st);
        assert!(out_keep_a.changed);
        assert_eq!(active_rgba(&paint, 0)[3], 141, "unchanged 2-decimal / 0.55 must keep alpha 141");

        // Editing alpha from 141 to / 0.5 commits alpha 128.
        let out_change_a = commit_picker_ok(&ctx, "70% 0.14 254.47 / 0.5", &mut paint, &mut st);
        assert!(out_change_a.changed);
        assert_eq!(active_rgba(&paint, 0)[3], 128);
    }

    #[test]
    fn pasted_css_oklch_and_oklab_functions_accept_unit_and_percent_lightness() {
        let ctx = test_ctx();
        let mut paint = solid(0, 0, 0, 1.0);
        let mut st = State::new(&paint);
        st.fmt = Fmt::Oklch;

        // CSS bare L in 0..1: oklch(0.985 0 0) -> near-white [250, 250, 250, 255].
        let out_white = commit_picker_ok(&ctx, "oklch(0.985 0 0)", &mut paint, &mut st);
        assert!(out_white.changed);
        assert_eq!(active_rgba(&paint, 0), [250, 250, 250, 255]);

        // CSS percent L in 0..100%: oklch(62.8% 0.26 29.23) -> red [255, 0, 0, 255].
        let out_red = commit_picker_ok(&ctx, "oklch(62.8% 0.26 29.23)", &mut paint, &mut st);
        assert!(out_red.changed);
        assert_eq!(active_rgba(&paint, 0), [255, 0, 0, 255]);

        // CSS bare L with chromatic channels: oklch(0.637 0.237 25.331).
        let out_tw = commit_picker_ok(&ctx, "oklch(0.637 0.237 25.331)", &mut paint, &mut st);
        assert!(out_tw.changed);
        let exp_tw = designos_doc::oklch_to_srgb_u8(0.637, 0.237, 25.331);
        assert_eq!(&active_rgba(&paint, 0)[..3], &exp_tw);

        // Cross-format paste: pasting oklab(...) while in OKLCH mode applies OKLAB conversion.
        st.fmt = Fmt::Oklch;
        let out_cross = commit_picker_ok(&ctx, "oklab(0.5 0.1 -0.1)", &mut paint, &mut st);
        assert!(out_cross.changed);
        let exp_lab = designos_doc::oklab_to_srgb_u8(0.5, 0.1, -0.1);
        assert_eq!(&active_rgba(&paint, 0)[..3], &exp_lab);

        // Comma-separated tokens parse and commit.
        let out_comma = commit_picker_ok(&ctx, "70%, 0.15, 145", &mut paint, &mut st);
        assert!(out_comma.changed);
        assert_eq!(&active_rgba(&paint, 0)[..3], &designos_doc::oklch_to_srgb_u8(0.70, 0.15, 145.0));

        // CSS bare L > 1 clamps to 1.0: oklch(62.8 0.26 29.23) gives st.oklch[0] == 1.0.
        let out_clamp_l = commit_picker_ok(&ctx, "oklch(62.8 0.26 29.23)", &mut paint, &mut st);
        assert!(out_clamp_l.changed);
        assert_eq!(st.oklch[0], 1.0);

        // Pasting bare L matching the 0..100 channel string on [52, 120, 200] still clamps L to 1.0.
        let mut paint_blue = solid(52, 120, 200, 1.0);
        let mut st_blue = State::new(&paint_blue);
        st_blue.fmt = Fmt::Oklch;
        let out_bare_match = commit_picker_ok(&ctx, "oklch(56.93 0.14 254.47)", &mut paint_blue, &mut st_blue);
        assert!(out_bare_match.changed);
        assert_eq!(st_blue.oklch[0], 1.0);

        // Fewer or more than 3 channel tokens are rejected.
        assert!(st.apply_ok_input(Fmt::Oklch, "62.8% 0.26", 255).is_none());
        assert!(st.apply_ok_input(Fmt::Oklch, "62.8% 0.26 29.23 0.5", 255).is_none());
    }

    #[test]
    fn oklch_edit_round_trips_within_one_unit_of_last_shown_digit() {
        let ctx = test_ctx();
        for (input, exp_l, exp_c, exp_h) in [
            ("62.8% 0.26 29.23", 62.8_f32, 0.26_f32, 29.23_f32),
            ("56.93% 0.14 254.47", 56.93, 0.14, 254.47),
            ("86.64% 0.29 142.5", 86.64, 0.29, 142.5),
            ("45.2% 0.31 264.05", 45.2, 0.31, 264.05),
            ("75% 0.12 80", 75.0, 0.12, 80.0),
            ("100% 0 0", 100.0, 0.0, 0.0),
            ("0% 0 0", 0.0, 0.0, 0.0),
        ] {
            let mut paint = solid(0, 0, 0, 1.0);
            let mut st = State::new(&paint);
            st.fmt = Fmt::Oklch;
            let out = commit_picker_ok(&ctx, input, &mut paint, &mut st);
            assert!(out.changed || input == "0% 0 0", "expected commit for {input}");
            let live_cells = st.ok_cells(Fmt::Oklch);
            let live_l: f32 = live_cells[0].parse().unwrap();
            let live_c: f32 = live_cells[1].parse().unwrap();
            let live_h: f32 = live_cells[2].parse().unwrap();
            assert!((live_l - exp_l).abs() <= 0.01, "{input}: live L {live_l} vs {exp_l}");
            assert!((live_c - exp_c).abs() <= 0.01, "{input}: live C {live_c} vs {exp_c}");
            assert!((live_h - exp_h).abs() <= 0.01, "{input}: live H {live_h} vs {exp_h}");
        }
    }

    #[test]
    fn dropdown_insets_keep_source_at_main_and_fmt_chevron_gap() {
        let ctx = test_ctx();
        let mut paint = solid(255, 0, 0, 1.0);
        let mut st = State::new(&paint);
        st.fmt = Fmt::Oklch;
        let mut raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 600.0))), ..Default::default() };
        raw.time = Some(0.0);
        let sources: Vec<(String, Vec<Color32>)> = vec![("On this page".into(), vec![])];
        let env = Env { paint_types: true, sources: &sources, styles: &[], can_add_style: false };
        let full = ctx.run_ui(raw, |ui| {
            let _ = show(ui, Pos2::ZERO, &mut st, &mut paint, &env);
        });
        fn find_text(shapes: &[egui::epaint::ClippedShape], target: &str) -> Option<Rect> {
            fn walk(shape: &egui::Shape, target: &str) -> Option<Rect> {
                match shape {
                    egui::Shape::Vec(v) => v.iter().find_map(|s| walk(s, target)),
                    egui::Shape::Text(t) if t.galley.text() == target => Some(Rect::from_min_size(t.pos, t.galley.size())),
                    _ => None,
                }
            }
            shapes.iter().find_map(|cs| walk(&cs.shape, target))
        }
        fn find_mesh_in(shapes: &[egui::epaint::ClippedShape], x_min: f32, x_max: f32, cy: f32) -> Option<Rect> {
            fn walk(shape: &egui::Shape, x_min: f32, x_max: f32, cy: f32) -> Option<Rect> {
                match shape {
                    egui::Shape::Vec(v) => v.iter().find_map(|s| walk(s, x_min, x_max, cy)),
                    egui::Shape::Mesh(m) => {
                        let b = m.calc_bounds();
                        if b.left() >= x_min && b.right() <= x_max && (b.center().y - cy).abs() < 6.0 && (b.width() - 12.0).abs() < 1.0 {
                            Some(b)
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            }
            shapes.iter().find_map(|cs| walk(&cs.shape, x_min, x_max, cy))
        }
        let source_rect = find_text(&full.shapes, "On this page").expect("On this page text");
        assert!((source_rect.left() - (PAD + 8.0)).abs() < 0.1, "On this page must start at PAD + 8.0 (24.0), got {}", source_rect.left());
        let fmt_rect = find_text(&full.shapes, "OKLCH").expect("OKLCH text");
        let fmt_chev = find_mesh_in(&full.shapes, PAD, PAD + 66.0, fmt_rect.center().y).expect("format dropdown chevron mesh");
        assert!(
            fmt_chev.left() - fmt_rect.right() >= 4.0,
            "OKLCH label right ({}) must keep >= 4.0px gap to chevron left ({})",
            fmt_rect.right(),
            fmt_chev.left()
        );
    }

    #[test]
    fn editing_oklch_and_oklab_channels_commits_srgb_rgba() {
        let ctx = test_ctx();
        let mut paint = solid(255, 0, 0, 1.0);
        let mut st = State::new(&paint);
        st.fmt = Fmt::Oklch;

        // Edit L in OKLCH to 70%
        let out_l = commit_picker_ok(&ctx, "70% 0.26 29.23", &mut paint, &mut st);
        assert!(out_l.changed, "editing OKLCH L must mark out.changed");
        let expected_after_l = designos_doc::oklch_to_srgb_u8(0.70, st.oklch[1], st.oklch[2]);
        assert_eq!(&active_rgba(&paint, 0)[..3], &expected_after_l);

        // Edit C in OKLCH to 0.15
        let out_c = commit_picker_ok(&ctx, "70% 0.15 29.23", &mut paint, &mut st);
        assert!(out_c.changed, "editing OKLCH C must mark out.changed");
        let expected_after_c = designos_doc::oklch_to_srgb_u8(0.70, 0.15, st.oklch[2]);
        assert_eq!(&active_rgba(&paint, 0)[..3], &expected_after_c);

        // Edit H in OKLCH to 145deg
        let out_h = commit_picker_ok(&ctx, "70% 0.15 145deg", &mut paint, &mut st);
        assert!(out_h.changed, "editing OKLCH H must mark out.changed");
        let expected_after_h = designos_doc::oklch_to_srgb_u8(0.70, 0.15, 145.0);
        assert_eq!(&active_rgba(&paint, 0)[..3], &expected_after_h);

        // Typing C = 0.2577 through the UI keeps full f32 precision (0.2577, not rounded to 0.26)
        let _ = commit_picker_ok(&ctx, "70% 0.2577 145", &mut paint, &mut st);
        assert!((st.oklch[1] - 0.2577).abs() < 1e-7, "UI commit of 0.2577 must keep full f32 precision");
        assert_eq!(st.ok_cells(Fmt::Oklch)[1], "0.26");
        assert_eq!(&active_rgba(&paint, 0)[..3], &designos_doc::oklch_to_srgb_u8(0.70, 0.2577, 145.0));

        // Switch to OKLAB and edit L, a, b + / alpha
        st.fmt = Fmt::Oklab;
        let out_lab = commit_picker_ok(&ctx, "60% 0.12 -0.18 / 0.5", &mut paint, &mut st);
        assert!(out_lab.changed, "editing OKLAB cell must mark out.changed");
        let expected_lab = designos_doc::oklab_to_srgb_u8(0.60, 0.12, -0.18);
        assert_eq!(&active_rgba(&paint, 0)[..3], &expected_lab);
        assert_eq!(active_rgba(&paint, 0)[3], 128);
    }

    #[test]
    fn oklch_state_preserves_typed_values_and_grey_hue() {
        let ctx = test_ctx();
        let mut paint = solid(255, 0, 0, 1.0);
        let mut st = State::new(&paint);
        st.fmt = Fmt::Oklch;
        let red_hue = st.oklch[2];
        assert!((red_hue - 29.233_9).abs() < 1e-3);

        // C to 0 (grey) and back to 0.1 keeps the hue.
        let _ = commit_picker_ok(&ctx, "62.8% 0 29.23", &mut paint, &mut st);
        assert!((st.oklch[1] - 0.0).abs() < 1e-6);
        assert!((st.oklch[2] - red_hue).abs() < 1e-4, "hue must survive when C is 0");
        assert_eq!(st.ok_cells(Fmt::Oklch)[2], "29.23");

        let _ = commit_picker_ok(&ctx, "62.8% 0.1 29.23", &mut paint, &mut st);
        assert!((st.oklch[1] - 0.1).abs() < 1e-6);
        assert!((st.oklch[2] - red_hue).abs() < 1e-4, "hue must still be red's hue after restoring C to 0.1");
        assert_eq!(&active_rgba(&paint, 0)[..3], &designos_doc::oklch_to_srgb_u8(st.oklch[0], 0.1, red_hue));

        // Red in OKLCH, then outside st.sync([128,128,128,255]) -> H still reads 29.23
        let mut paint_sync = solid(255, 0, 0, 1.0);
        let mut st_sync = State::new(&paint_sync);
        st_sync.fmt = Fmt::Oklch;
        st_sync.sync([128, 128, 128, 255]);
        assert!((st_sync.oklch[2] - red_hue).abs() < 1e-4, "outside sync to grey must preserve red hue");
        assert_eq!(st_sync.ok_cells(Fmt::Oklch)[2], "29.23");
        set_active(&mut paint_sync, 0, [128, 128, 128, 255]);
        assert_visible_mode(&ctx, &mut paint_sync, &mut st_sync, &["OKLCH", "59.99% 0 29.23"]);

        // OKLAB a=0 then b=0 keeps H (90 deg after a=0)
        let mut st_lab = State::new(&solid(255, 0, 0, 1.0));
        let _ = st_lab.set_oklab_channel(1, 0.0);
        assert!((st_lab.oklch[2] - 90.0).abs() < 1e-4, "a=0 with b>0 must set H=90");
        let _ = st_lab.set_oklab_channel(2, 0.0);
        assert!((st_lab.oklch[2] - 90.0).abs() < 1e-4, "b=0 after a=0 must preserve H=90");

        // After typing L = 50, the cell shows "50% 0.26 29.23" across subsequent frames.
        let mut paint2 = solid(255, 0, 0, 1.0);
        let mut st2 = State::new(&paint2);
        st2.fmt = Fmt::Oklch;
        let _ = commit_picker_ok(&ctx, "50% 0.26 29.23", &mut paint2, &mut st2);
        assert_eq!(st2.ok_cells(Fmt::Oklch)[0], "50");
        assert_visible_mode(&ctx, &mut paint2, &mut st2, &["OKLCH", "50% 0.26 29.23"]);
    }

    fn focus_picker_ok_at(ctx: &egui::Context, paint: &mut Paint, st: &mut State, caret: usize) -> Id {
        let _ = run_picker_frame(ctx, vec![], paint, st);
        let mut cell_id = None;
        let sources: Vec<(String, Vec<Color32>)> = vec![("On this page".into(), vec![])];
        let env = Env { paint_types: true, sources: &sources, styles: &[], can_add_style: false };
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let id = ui.id().with(("txt", "picker-ok"));
            cell_id = Some(id);
            let _ = show(ui, Pos2::ZERO, st, paint, &env);
            ui.memory_mut(|m| m.request_focus(id));
        });
        let id = cell_id.unwrap();
        let _ = run_picker_frame(ctx, vec![], paint, st);
        let mut state = egui::widgets::text_edit::TextEdit::load_state(ctx, id).unwrap_or_default();
        state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(caret))));
        state.store(ctx, id);
        id
    }

    fn press_picker_key(ctx: &egui::Context, key: egui::Key, modifiers: egui::Modifiers, paint: &mut Paint, st: &mut State) -> Out {
        run_picker_frame(ctx, vec![egui::Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers }], paint, st)
    }

    fn picker_ok_buf_and_caret(ctx: &egui::Context, id: Id) -> (String, usize) {
        let buf = ctx.data(|d| d.get_temp::<String>(id)).unwrap_or_default();
        let caret = egui::widgets::text_edit::TextEdit::load_state(ctx, id).and_then(|s| s.cursor.char_range()).map(|r| r.primary.index.0).unwrap_or(0);
        (buf, caret)
    }

    #[test]
    fn oklch_arrow_keys_step_l_c_h_with_plain_shift_and_alt_modifiers() {
        let ctx = test_ctx();
        let shift = egui::Modifiers { shift: true, ..egui::Modifiers::NONE };
        let alt = egui::Modifiers { alt: true, ..egui::Modifiers::NONE };

        // Start from [52, 120, 200, 255] -> OKLCH "56.93% 0.14 254.47"
        let mut paint = solid(52, 120, 200, 1.0);
        let mut st = State::new(&paint);
        st.fmt = Fmt::Oklch;
        assert_eq!(st.ok_text(Fmt::Oklch), "56.93% 0.14 254.47");

        // 1. L component (caret at char 2 inside "56.93%"): plain ±1%, Shift ±10%, Alt ±0.1%
        let id = focus_picker_ok_at(&ctx, &mut paint, &mut st, 2);
        let out_l_up = press_picker_key(&ctx, egui::Key::ArrowUp, egui::Modifiers::NONE, &mut paint, &mut st);
        assert!(out_l_up.changed, "ArrowUp on L must update paint immediately");
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "57.93% 0.14 254.47");
        assert!((st.oklch[0] - 0.5793).abs() < 1e-5);

        let _ = press_picker_key(&ctx, egui::Key::ArrowDown, egui::Modifiers::NONE, &mut paint, &mut st);
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "56.93% 0.14 254.47");
        assert!((st.oklch[0] - 0.5693).abs() < 1e-5);

        let _ = press_picker_key(&ctx, egui::Key::ArrowUp, shift, &mut paint, &mut st);
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "66.93% 0.14 254.47");
        assert!((st.oklch[0] - 0.6693).abs() < 1e-5);

        let _ = press_picker_key(&ctx, egui::Key::ArrowDown, shift, &mut paint, &mut st);
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "56.93% 0.14 254.47");

        let _ = press_picker_key(&ctx, egui::Key::ArrowUp, alt, &mut paint, &mut st);
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "57.03% 0.14 254.47");
        assert!((st.oklch[0] - 0.5703).abs() < 1e-5);

        let _ = press_picker_key(&ctx, egui::Key::ArrowDown, alt, &mut paint, &mut st);
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "56.93% 0.14 254.47");

        // 2. C component (caret at char 9 inside "0.14"): plain ±0.01, Shift ±0.1, Alt ±0.001
        let id = focus_picker_ok_at(&ctx, &mut paint, &mut st, 9);
        let out_c_up = press_picker_key(&ctx, egui::Key::ArrowUp, egui::Modifiers::NONE, &mut paint, &mut st);
        assert!(out_c_up.changed, "ArrowUp on C must update paint immediately");
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "56.93% 0.15 254.47");
        assert!((st.oklch[1] - 0.15).abs() < 1e-5);

        let _ = press_picker_key(&ctx, egui::Key::ArrowDown, egui::Modifiers::NONE, &mut paint, &mut st);
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "56.93% 0.14 254.47");
        assert!((st.oklch[1] - 0.14).abs() < 1e-5);

        let _ = press_picker_key(&ctx, egui::Key::ArrowUp, shift, &mut paint, &mut st);
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "56.93% 0.24 254.47");
        assert!((st.oklch[1] - 0.24).abs() < 1e-5);

        let _ = press_picker_key(&ctx, egui::Key::ArrowDown, shift, &mut paint, &mut st);
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "56.93% 0.14 254.47");

        let _ = press_picker_key(&ctx, egui::Key::ArrowUp, alt, &mut paint, &mut st);
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "56.93% 0.141 254.47");
        assert!((st.oklch[1] - 0.141).abs() < 1e-5);

        let _ = press_picker_key(&ctx, egui::Key::ArrowDown, alt, &mut paint, &mut st);
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "56.93% 0.14 254.47");
        assert!((st.oklch[1] - 0.14).abs() < 1e-5);

        // 3. H component (caret at char 14 inside "254.47"): plain ±1°, Shift ±10°, Alt ±0.1°
        let id = focus_picker_ok_at(&ctx, &mut paint, &mut st, 14);
        let out_h_up = press_picker_key(&ctx, egui::Key::ArrowUp, egui::Modifiers::NONE, &mut paint, &mut st);
        assert!(out_h_up.changed, "ArrowUp on H must update paint immediately");
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "56.93% 0.14 255.47");
        assert!((st.oklch[2] - 255.47).abs() < 1e-4);

        let _ = press_picker_key(&ctx, egui::Key::ArrowDown, egui::Modifiers::NONE, &mut paint, &mut st);
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "56.93% 0.14 254.47");
        assert!((st.oklch[2] - 254.47).abs() < 1e-4);

        let _ = press_picker_key(&ctx, egui::Key::ArrowUp, shift, &mut paint, &mut st);
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "56.93% 0.14 264.47");
        assert!((st.oklch[2] - 264.47).abs() < 1e-4);

        let _ = press_picker_key(&ctx, egui::Key::ArrowDown, shift, &mut paint, &mut st);
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "56.93% 0.14 254.47");

        let _ = press_picker_key(&ctx, egui::Key::ArrowUp, alt, &mut paint, &mut st);
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "56.93% 0.14 254.57");
        assert!((st.oklch[2] - 254.57).abs() < 1e-4);

        let _ = press_picker_key(&ctx, egui::Key::ArrowDown, alt, &mut paint, &mut st);
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "56.93% 0.14 254.47");
        assert!((st.oklch[2] - 254.47).abs() < 1e-4);
    }

    #[test]
    fn oklab_arrow_keys_step_a_and_b_by_hundredths_and_clamp() {
        let ctx = test_ctx();
        let shift = egui::Modifiers { shift: true, ..egui::Modifiers::NONE };
        let alt = egui::Modifiers { alt: true, ..egui::Modifiers::NONE };

        // Start from [52, 120, 200, 255] -> OKLAB "56.93% -0.04 -0.14"
        let mut paint = solid(52, 120, 200, 1.0);
        let mut st = State::new(&paint);
        st.fmt = Fmt::Oklab;
        assert_eq!(st.ok_text(Fmt::Oklab), "56.93% -0.04 -0.14");

        // Step `a` (caret at char 9 inside "-0.04"): +0.01 -> -0.03, -0.01 -> -0.04, Shift +0.1 -> 0.06, Alt +0.001 -> -0.039
        let id = focus_picker_ok_at(&ctx, &mut paint, &mut st, 9);
        let out_a = press_picker_key(&ctx, egui::Key::ArrowUp, egui::Modifiers::NONE, &mut paint, &mut st);
        assert!(out_a.changed, "ArrowUp on OKLAB a must update paint immediately");
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "56.93% -0.03 -0.14");
        assert!((st.oklab[1] - (-0.03)).abs() < 1e-5);

        let _ = press_picker_key(&ctx, egui::Key::ArrowDown, egui::Modifiers::NONE, &mut paint, &mut st);
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "56.93% -0.04 -0.14");
        assert!((st.oklab[1] - (-0.04)).abs() < 1e-5);

        let _ = press_picker_key(&ctx, egui::Key::ArrowUp, shift, &mut paint, &mut st);
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "56.93% 0.06 -0.14");
        assert!((st.oklab[1] - 0.06).abs() < 1e-5);

        let _ = press_picker_key(&ctx, egui::Key::ArrowDown, shift, &mut paint, &mut st);
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "56.93% -0.04 -0.14");

        let _ = press_picker_key(&ctx, egui::Key::ArrowUp, alt, &mut paint, &mut st);
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "56.93% -0.039 -0.14");
        assert!((st.oklab[1] - (-0.039)).abs() < 1e-5);

        let _ = press_picker_key(&ctx, egui::Key::ArrowDown, alt, &mut paint, &mut st);
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "56.93% -0.04 -0.14");

        // Step `b` (caret at char 15 inside "-0.14"): +0.01 -> -0.13, -0.01 -> -0.14
        let id = focus_picker_ok_at(&ctx, &mut paint, &mut st, 15);
        let out_b = press_picker_key(&ctx, egui::Key::ArrowUp, egui::Modifiers::NONE, &mut paint, &mut st);
        assert!(out_b.changed, "ArrowUp on OKLAB b must update paint immediately");
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "56.93% -0.04 -0.13");
        assert!((st.oklab[2] - (-0.13)).abs() < 1e-5);

        let _ = press_picker_key(&ctx, egui::Key::ArrowDown, egui::Modifiers::NONE, &mut paint, &mut st);
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "56.93% -0.04 -0.14");
        assert!((st.oklab[2] - (-0.14)).abs() < 1e-5);

        // Clamp `b` at -0.4 and `a` at +0.4
        for _ in 0..3 {
            let _ = press_picker_key(&ctx, egui::Key::ArrowDown, shift, &mut paint, &mut st);
        }
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "56.93% -0.04 -0.4");
        assert!((st.oklab[2] - (-0.4)).abs() < 1e-5);
        let id = focus_picker_ok_at(&ctx, &mut paint, &mut st, 9);
        for _ in 0..5 {
            let _ = press_picker_key(&ctx, egui::Key::ArrowUp, shift, &mut paint, &mut st);
        }
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "56.93% 0.4 -0.4");
        assert!((st.oklab[1] - 0.4).abs() < 1e-5);
    }

    #[test]
    fn oklch_hue_wraps_from_359_to_0_and_lightness_stops_at_100() {
        let ctx = test_ctx();
        let mut paint = solid(255, 0, 0, 1.0);
        let mut st = State::new(&paint);
        st.fmt = Fmt::Oklch;
        let _ = commit_picker_ok(&ctx, "99% 0.26 359", &mut paint, &mut st);
        assert_eq!(st.ok_text(Fmt::Oklch), "99% 0.26 359");

        // Caret on H ("359", char 10): ArrowUp wraps 359 -> 0, ArrowDown wraps 0 -> 359
        let id = focus_picker_ok_at(&ctx, &mut paint, &mut st, 10);
        let _ = press_picker_key(&ctx, egui::Key::ArrowUp, egui::Modifiers::NONE, &mut paint, &mut st);
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "99% 0.26 0");
        assert!((st.oklch[2] - 0.0).abs() < 1e-5);

        let _ = press_picker_key(&ctx, egui::Key::ArrowDown, egui::Modifiers::NONE, &mut paint, &mut st);
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "99% 0.26 359");
        assert!((st.oklch[2] - 359.0).abs() < 1e-5);

        // Caret on L ("99%", char 1): ArrowUp steps to 100%, second ArrowUp stops at 100%
        let id = focus_picker_ok_at(&ctx, &mut paint, &mut st, 1);
        let _ = press_picker_key(&ctx, egui::Key::ArrowUp, egui::Modifiers::NONE, &mut paint, &mut st);
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "100% 0.26 359");
        assert!((st.oklch[0] - 1.0).abs() < 1e-5);

        let _ = press_picker_key(&ctx, egui::Key::ArrowUp, egui::Modifiers::NONE, &mut paint, &mut st);
        assert_eq!(picker_ok_buf_and_caret(&ctx, id).0, "100% 0.26 359");
        assert!((st.oklch[0] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn oklch_and_oklab_caret_stays_on_stepped_component_and_respects_separators() {
        let ctx = test_ctx();
        let mut paint = solid(52, 120, 200, 1.0);
        let mut st = State::new(&paint);
        st.fmt = Fmt::Oklch;
        assert_eq!(st.ok_text(Fmt::Oklch), "56.93% 0.14 254.47");

        // At start (char 0) -> steps first component (L) and caret stays on L (0..=6)
        let id = focus_picker_ok_at(&ctx, &mut paint, &mut st, 0);
        let _ = press_picker_key(&ctx, egui::Key::ArrowUp, egui::Modifiers::NONE, &mut paint, &mut st);
        let (buf, caret) = picker_ok_buf_and_caret(&ctx, id);
        assert_eq!(buf, "57.93% 0.14 254.47");
        assert!(caret <= 6, "caret at start must stay on L component, got {caret}");

        // On space between L and C (char 6, right after '%'): nudges component to its left (L)
        let id = focus_picker_ok_at(&ctx, &mut paint, &mut st, 6);
        let _ = press_picker_key(&ctx, egui::Key::ArrowUp, egui::Modifiers::NONE, &mut paint, &mut st);
        let (buf, caret) = picker_ok_buf_and_caret(&ctx, id);
        assert_eq!(buf, "58.93% 0.14 254.47");
        assert!(caret <= 6, "caret on separator after L must stay on L, got {caret}");

        // On space between C and H (char 11, right after "0.14"): nudges component to its left (C)
        // and consecutive ArrowUp presses without re-positioning keep stepping C.
        let id = focus_picker_ok_at(&ctx, &mut paint, &mut st, 11);
        for expected_c in ["0.15", "0.16", "0.17"] {
            let _ = press_picker_key(&ctx, egui::Key::ArrowUp, egui::Modifiers::NONE, &mut paint, &mut st);
            let (buf, caret) = picker_ok_buf_and_caret(&ctx, id);
            assert_eq!(buf, format!("58.93% {expected_c} 254.47"));
            assert!((7..=11).contains(&caret), "caret must stay on C component (7..=11), got {caret}");
        }

        // On H across width change ("359" -> "0" -> "1" -> "2"): caret stays on H
        let _ = commit_picker_ok(&ctx, "58.93% 0.17 359", &mut paint, &mut st);
        let id = focus_picker_ok_at(&ctx, &mut paint, &mut st, 15);
        for expected_h in ["0", "1", "2"] {
            let _ = press_picker_key(&ctx, egui::Key::ArrowUp, egui::Modifiers::NONE, &mut paint, &mut st);
            let (buf, caret) = picker_ok_buf_and_caret(&ctx, id);
            assert_eq!(buf, format!("58.93% 0.17 {expected_h}"));
            assert!(caret >= 12, "caret must stay on H component (>= 12), got {caret}");
        }
    }

    #[test]
    fn oklch_and_oklab_arrow_key_press_is_one_undo_step() {
        let mut doc = designos_doc::Document::new("oklch-undo");
        let page = doc.add_page("Page 1");
        let mut app = crate::app::App::new(doc);
        app.page = page;
        let rect_id = app.doc.alloc_id();
        let mut node = designos_doc::Node::rectangle(rect_id, designos_doc::Rect::new(0.0, 0.0, 100.0, 100.0));
        let initial_color = Color::rgb(52, 120, 200);
        node.fills = vec![Paint::solid(initial_color)];
        let rect_id = app.doc.insert(page, node).unwrap();
        app.selection = vec![rect_id];

        let ctx = test_ctx();
        let region = Rect::from_min_size(Pos2::ZERO, Vec2::new(240.0, 900.0));
        let step_ui = |app: &mut crate::app::App, events: Vec<egui::Event>| {
            let raw = egui::RawInput { screen_rect: Some(region), events, ..Default::default() };
            let _ = ctx.run_ui(raw, |ui| crate::panels::properties::show(app, ui, region));
        };

        for (fmt, cell_id) in [
            (Fmt::Hex, Id::new("color-picker").with(("txt", "picker-hex"))),
            (Fmt::Rgb, Id::new("color-picker").with(("txt", ("picker-ch", 0usize)))),
            (Fmt::Oklch, Id::new("color-picker").with(("txt", "picker-ok"))),
            (Fmt::Oklab, Id::new("color-picker").with(("txt", "picker-ok"))),
        ] {
            // Open fill picker in `fmt` mode
            step_ui(&mut app, vec![]);
            let sw_rect = ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", "sw:false:0".to_string())))).expect("fill swatch");
            step_ui(&mut app, vec![egui::Event::PointerMoved(sw_rect.center())]);
            for pressed in [true, false] {
                step_ui(
                    &mut app,
                    vec![egui::Event::PointerButton { pos: sw_rect.center(), button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }],
                );
            }
            app.props.picker.as_mut().expect("picker open").state.fmt = fmt;
            step_ui(&mut app, vec![]);

            // Focus the value cell inside "color-picker" at char 0 and send one ArrowUp key press
            ctx.memory_mut(|m| m.request_focus(cell_id));
            step_ui(&mut app, vec![]);
            {
                let mut state = egui::widgets::text_edit::TextEdit::load_state(&ctx, cell_id).unwrap_or_default();
                state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(0))));
                state.store(&ctx, cell_id);
            }
            step_ui(
                &mut app,
                vec![egui::Event::Key {
                    key: egui::Key::ArrowUp,
                    physical_key: Some(egui::Key::ArrowUp),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );

            let stepped_fill = match app.doc.get(rect_id).unwrap().fills[0].kind {
                PaintKind::Solid(c) => c,
                _ => panic!("expected solid fill"),
            };
            assert_ne!(stepped_fill, initial_color, "one ArrowUp press in {fmt:?} must update the node fill immediately");

            // Close picker with Escape and verify one undo step restores initial_color
            ctx.memory_mut(|m| m.surrender_focus(cell_id));
            step_ui(
                &mut app,
                vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: Some(egui::Key::Escape),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            assert!(app.props.picker.is_none(), "Escape closes picker");
            assert!(app.hist.can_undo(), "one key press in {fmt:?} must record an undo step");
            app.undo();
            let restored_fill = match app.doc.get(rect_id).unwrap().fills[0].kind {
                PaintKind::Solid(c) => c,
                _ => panic!("expected solid fill"),
            };
            assert_eq!(restored_fill, initial_color, "one undo step must restore the initial fill colour after {fmt:?} step");
            assert!(!app.hist.can_undo(), "only one undo step must have been recorded for {fmt:?}");
        }
    }

    /// What one key press left behind in the OKLCH / OKLAB field.
    struct OkPress {
        buf: String,
        caret: usize,
        rgba: [u8; 4],
        st: State,
        changed: bool,
        focused: bool,
    }

    /// A fresh picker on a solid `rgb` fill in `fmt`, its OKLCH / OKLAB field focused for several frames and holding
    /// `text` with the caret at `caret`; presses each key of `keys` in its own frame and reports the field after each.
    fn press_in_ok_field(fmt: Fmt, rgb: [u8; 3], text: &str, caret: usize, keys: &[(egui::Key, egui::Modifiers)]) -> Vec<OkPress> {
        let ctx = test_ctx();
        let mut paint = solid(rgb[0], rgb[1], rgb[2], 1.0);
        let mut st = State::new(&paint);
        st.fmt = fmt;
        let id = focus_picker_ok_at(&ctx, &mut paint, &mut st, caret);
        ctx.data_mut(|d| d.insert_temp(id, text.to_string()));
        keys.iter()
            .map(|&(key, modifiers)| {
                let out = press_picker_key(&ctx, key, modifiers, &mut paint, &mut st);
                let (buf, caret) = picker_ok_buf_and_caret(&ctx, id);
                OkPress { buf, caret, rgba: active_rgba(&paint, 0), st: st.clone(), changed: out.changed, focused: ctx.memory(|m| m.has_focus(id)) }
            })
            .collect()
    }

    /// The colour and state Enter commits on `text` in a fresh picker on a solid `rgb` fill in `fmt`.
    fn enter_commits(fmt: Fmt, rgb: [u8; 3], text: &str) -> ([u8; 4], State) {
        let ctx = test_ctx();
        let mut paint = solid(rgb[0], rgb[1], rgb[2], 1.0);
        let mut st = State::new(&paint);
        st.fmt = fmt;
        let _ = commit_picker_ok(&ctx, text, &mut paint, &mut st);
        (active_rgba(&paint, 0), st)
    }

    const UP: (egui::Key, egui::Modifiers) = (egui::Key::ArrowUp, egui::Modifiers::NONE);
    const DOWN: (egui::Key, egui::Modifiers) = (egui::Key::ArrowDown, egui::Modifiers::NONE);
    const SHIFT_UP: (egui::Key, egui::Modifiers) = (egui::Key::ArrowUp, egui::Modifiers { shift: true, ..egui::Modifiers::NONE });
    const SHIFT_DOWN: (egui::Key, egui::Modifiers) = (egui::Key::ArrowDown, egui::Modifiers { shift: true, ..egui::Modifiers::NONE });

    /// Every press committed exactly what Enter commits on the text the field then holds.
    fn assert_commits_like_enter(fmt: Fmt, rgb: [u8; 3], presses: &[OkPress]) {
        for p in presses {
            let (rgba, st) = enter_commits(fmt, rgb, &p.buf);
            assert_eq!(p.rgba, rgba, "the colour after a step must be the one Enter commits on {:?}", p.buf);
            for i in 0..3 {
                assert!((p.st.oklch[i] - st.oklch[i]).abs() < 1e-4, "OKLCH[{i}] after a step on {:?}: {:?} vs Enter {:?}", p.buf, p.st.oklch, st.oklch);
                assert!((p.st.oklab[i] - st.oklab[i]).abs() < 1e-4, "OKLAB[{i}] after a step on {:?}: {:?} vs Enter {:?}", p.buf, p.st.oklab, st.oklab);
            }
        }
    }

    const BLUE: [u8; 3] = [52, 120, 200];

    #[test]
    fn an_oklab_function_in_the_oklch_field_keeps_its_form_and_steps_in_oklab() {
        // Caret on L (char 7 of "oklab(50% 0.2 -0.1)"): two presses, wrapper kept both times.
        let p = press_in_ok_field(Fmt::Oklch, BLUE, "oklab(50% 0.2 -0.1)", 7, &[UP, UP]);
        assert_eq!(p[0].buf, "oklab(51% 0.2 -0.1)");
        assert_eq!(p[1].buf, "oklab(52% 0.2 -0.1)");
        for q in &p {
            assert!(q.changed && q.focused);
            assert!((6..=9).contains(&q.caret), "caret must stay on L, got {}", q.caret);
        }
        assert!((p[0].st.oklab[0] - 0.51).abs() < 1e-5 && (p[0].st.oklab[1] - 0.2).abs() < 1e-5 && (p[0].st.oklab[2] + 0.1).abs() < 1e-5);
        // The bare L step in OKLCH would read b = -0.1 as a hue and land on 359.9.
        assert!((p[0].st.oklch[2] - 359.9).abs() > 1.0, "hue {} must come from a = 0.2, b = -0.1", p[0].st.oklch[2]);
        assert_commits_like_enter(Fmt::Oklch, BLUE, &p);

        // Caret on b (char 16): b steps by 0.01 in OKLAB, where OKLCH would step a hue by 1 degree.
        let p = press_in_ok_field(Fmt::Oklch, BLUE, "oklab(50% 0.2 -0.1)", 16, &[UP, DOWN, DOWN]);
        assert_eq!(p[0].buf, "oklab(50% 0.2 -0.09)");
        assert_eq!(p[1].buf, "oklab(50% 0.2 -0.1)");
        assert_eq!(p[2].buf, "oklab(50% 0.2 -0.11)");
        for q in &p {
            assert!((14..=19).contains(&q.caret), "caret must stay on b, got {}", q.caret);
        }
        assert_commits_like_enter(Fmt::Oklch, BLUE, &p);

        // a and b clamp at +-0.4 in OKLAB.
        let p = press_in_ok_field(Fmt::Oklch, BLUE, "oklab(50% 0.2 -0.4)", 16, &[DOWN]);
        assert_eq!(p[0].buf, "oklab(50% 0.2 -0.4)");
        let p = press_in_ok_field(Fmt::Oklch, BLUE, "oklab(50% 0.4 -0.1)", 11, &[SHIFT_UP]);
        assert_eq!(p[0].buf, "oklab(50% 0.4 -0.1)");
        assert_commits_like_enter(Fmt::Oklch, BLUE, &p);
    }

    #[test]
    fn an_oklch_function_in_the_oklab_field_keeps_its_form_and_steps_in_oklch() {
        let p = press_in_ok_field(Fmt::Oklab, BLUE, "oklch(50% 0.2 120)", 7, &[UP, UP]);
        assert_eq!(p[0].buf, "oklch(51% 0.2 120)");
        assert_eq!(p[1].buf, "oklch(52% 0.2 120)");
        assert!((p[0].st.oklch[0] - 0.51).abs() < 1e-5 && (p[0].st.oklch[1] - 0.2).abs() < 1e-5 && (p[0].st.oklch[2] - 120.0).abs() < 1e-3);
        assert_commits_like_enter(Fmt::Oklab, BLUE, &p);

        // Caret on H (char 15): a hue steps by 1 degree and wraps, where OKLAB would clamp b at 0.4.
        let p = press_in_ok_field(Fmt::Oklab, BLUE, "oklch(50% 0.2 120)", 15, &[UP, UP, DOWN]);
        assert_eq!(p[0].buf, "oklch(50% 0.2 121)");
        assert_eq!(p[1].buf, "oklch(50% 0.2 122)");
        assert_eq!(p[2].buf, "oklch(50% 0.2 121)");
        for q in &p {
            assert!((14..=17).contains(&q.caret), "caret must stay on H, got {}", q.caret);
        }
        assert_commits_like_enter(Fmt::Oklab, BLUE, &p);
        let p = press_in_ok_field(Fmt::Oklab, BLUE, "oklch(50% 0.2 359)", 15, &[UP]);
        assert_eq!(p[0].buf, "oklch(50% 0.2 0)");
        assert_commits_like_enter(Fmt::Oklab, BLUE, &p);

        // Caret on C (char 11): C clamps at 0.4.
        let p = press_in_ok_field(Fmt::Oklab, BLUE, "oklch(50% 0.4 120)", 11, &[UP]);
        assert_eq!(p[0].buf, "oklch(50% 0.4 120)");
    }

    #[test]
    fn a_function_in_its_own_space_keeps_its_wrapper_name_and_alpha() {
        let p = press_in_ok_field(Fmt::Oklch, BLUE, "oklch(50% 0.2 120 / 0.5)", 7, &[UP]);
        assert_eq!(p[0].buf, "oklch(51% 0.2 120 / 0.5)");
        assert_eq!(p[0].rgba[3], 128, "the alpha of the text goes through the commit");
        assert_commits_like_enter(Fmt::Oklch, BLUE, &p);

        let p = press_in_ok_field(Fmt::Oklab, BLUE, "OKLAB(50% 0.2 -0.1)", 11, &[UP]);
        assert_eq!(p[0].buf, "OKLAB(50% 0.21 -0.1)");
        assert_commits_like_enter(Fmt::Oklab, BLUE, &p);

        let p = press_in_ok_field(Fmt::Oklch, BLUE, "oklch( 50%, 0.2, 120 )", 9, &[UP]);
        assert_eq!(p[0].buf, "oklch( 51%, 0.2, 120 )");
    }

    #[test]
    fn oklch_chroma_stops_at_0_4_going_up_and_at_0_going_down() {
        let up = press_in_ok_field(Fmt::Oklch, BLUE, "56.93% 0.14 254.47", 9, &[SHIFT_UP, SHIFT_UP, SHIFT_UP, UP, SHIFT_UP, UP]);
        let bufs: Vec<&str> = up.iter().map(|p| p.buf.as_str()).collect();
        assert_eq!(bufs, ["56.93% 0.24 254.47", "56.93% 0.34 254.47", "56.93% 0.4 254.47", "56.93% 0.4 254.47", "56.93% 0.4 254.47", "56.93% 0.4 254.47"]);
        assert!((up[5].st.oklch[1] - 0.4).abs() < 1e-6);
        assert!(up.iter().all(|p| p.focused));

        let down = press_in_ok_field(Fmt::Oklch, BLUE, "56.93% 0.14 254.47", 9, &[SHIFT_DOWN, DOWN, DOWN, DOWN, SHIFT_DOWN, DOWN]);
        let bufs: Vec<&str> = down.iter().map(|p| p.buf.as_str()).collect();
        assert_eq!(bufs, ["56.93% 0.04 254.47", "56.93% 0.03 254.47", "56.93% 0.02 254.47", "56.93% 0.01 254.47", "56.93% 0 254.47", "56.93% 0 254.47"]);
        assert!(down[5].st.oklch[1].abs() < 1e-6);
    }

    #[test]
    fn a_caret_after_the_slash_steps_nothing_and_a_caret_on_it_steps_the_last_component() {
        let text = "56.93% 0.14 254.47 / 0.5";
        let before = State::new(&solid(52, 120, 200, 1.0));
        // Char 21 is the "0" of the alpha.
        let p = press_in_ok_field(Fmt::Oklch, BLUE, text, 21, &[UP, SHIFT_DOWN]);
        for q in &p {
            assert_eq!(q.buf, text, "a step must not touch the text when the caret is in the alpha");
            assert!(!q.changed, "no step, no change");
            assert!(q.focused, "the field keeps the focus");
            assert_eq!(q.st.oklch, before.oklch);
            assert_eq!(q.st.oklab, before.oklab);
            assert_eq!(q.rgba, [52, 120, 200, 255]);
        }
        // Char 19 is the slash itself: it belongs to the component on its left.
        let p = press_in_ok_field(Fmt::Oklch, BLUE, text, 19, &[UP]);
        assert_eq!(p[0].buf, "56.93% 0.14 255.47 / 0.5");
        assert!(p[0].changed);
    }

    fn apply_textures_delta(atlas: &mut std::collections::HashMap<egui::TextureId, image::RgbaImage>, delta: &egui::TexturesDelta) {
        for (tid, d) in &delta.set {
            let [w, h] = d.image.size();
            let src: Vec<[u8; 4]> = match &d.image {
                egui::ImageData::Color(c) => c.pixels.iter().map(|p| p.to_array()).collect(),
            };
            if let Some([ox, oy]) = d.pos {
                if let Some(existing) = atlas.get_mut(tid) {
                    for y in 0..h {
                        for x in 0..w {
                            let px = src[y * w + x];
                            if (ox + x) < existing.width() as usize && (oy + y) < existing.height() as usize {
                                existing.put_pixel((ox + x) as u32, (oy + y) as u32, image::Rgba(px));
                            }
                        }
                    }
                }
            } else {
                let mut img = image::RgbaImage::new(w as u32, h as u32);
                for y in 0..h {
                    for x in 0..w {
                        img.put_pixel(x as u32, y as u32, image::Rgba(src[y * w + x]));
                    }
                }
                atlas.insert(*tid, img);
            }
        }
    }

    fn rasterize_shapes(
        ctx: &egui::Context,
        shapes: Vec<egui::epaint::ClippedShape>,
        atlas: &std::collections::HashMap<egui::TextureId, image::RgbaImage>,
        width: u32,
        height: u32,
    ) -> image::RgbaImage {
        let mut fb = image::RgbaImage::from_pixel(width, height, image::Rgba([0x18, 0x18, 0x1B, 0xFF]));
        let prims = ctx.tessellate(shapes, 1.0);
        for cp in prims {
            let egui::epaint::Primitive::Mesh(mesh) = cp.primitive else { continue };
            let clip_min_x = cp.clip_rect.min.x.floor().max(0.0) as i32;
            let clip_min_y = cp.clip_rect.min.y.floor().max(0.0) as i32;
            let clip_max_x = cp.clip_rect.max.x.ceil().min(width as f32) as i32;
            let clip_max_y = cp.clip_rect.max.y.ceil().min(height as f32) as i32;
            let tex = atlas.get(&mesh.texture_id);
            for tri in mesh.indices.chunks_exact(3) {
                let v0 = &mesh.vertices[tri[0] as usize];
                let v1 = &mesh.vertices[tri[1] as usize];
                let v2 = &mesh.vertices[tri[2] as usize];
                let min_x = v0.pos.x.min(v1.pos.x).min(v2.pos.x).floor().max(clip_min_x as f32) as i32;
                let max_x = v0.pos.x.max(v1.pos.x).max(v2.pos.x).ceil().min(clip_max_x as f32) as i32;
                let min_y = v0.pos.y.min(v1.pos.y).min(v2.pos.y).floor().max(clip_min_y as f32) as i32;
                let max_y = v0.pos.y.max(v1.pos.y).max(v2.pos.y).ceil().min(clip_max_y as f32) as i32;
                let denom = (v1.pos.y - v2.pos.y) * (v0.pos.x - v2.pos.x) + (v2.pos.x - v1.pos.x) * (v0.pos.y - v2.pos.y);
                if denom.abs() < 1e-6 {
                    continue;
                }
                let inv_denom = 1.0 / denom;
                let c0 = v0.color.to_array().map(|c| c as f32);
                let c1 = v1.color.to_array().map(|c| c as f32);
                let c2 = v2.color.to_array().map(|c| c as f32);
                for py in min_y..max_y {
                    for px in min_x..max_x {
                        let p = Pos2::new(px as f32 + 0.5, py as f32 + 0.5);
                        let w0 = ((v1.pos.y - v2.pos.y) * (p.x - v2.pos.x) + (v2.pos.x - v1.pos.x) * (p.y - v2.pos.y)) * inv_denom;
                        let w1 = ((v2.pos.y - v0.pos.y) * (p.x - v2.pos.x) + (v0.pos.x - v2.pos.x) * (p.y - v2.pos.y)) * inv_denom;
                        let w2 = 1.0 - w0 - w1;
                        if w0 >= -1e-4 && w1 >= -1e-4 && w2 >= -1e-4 {
                            let mut col = [0.0f32; 4];
                            for k in 0..4 {
                                col[k] = (w0 * c0[k] + w1 * c1[k] + w2 * c2[k]).clamp(0.0, 255.0);
                            }
                            if let Some(t) = tex {
                                let u = (w0 * v0.uv.x + w1 * v1.uv.x + w2 * v2.uv.x).clamp(0.0, 1.0);
                                let v = (w0 * v0.uv.y + w1 * v1.uv.y + w2 * v2.uv.y).clamp(0.0, 1.0);
                                let tx = ((u * t.width() as f32).floor() as u32).min(t.width().saturating_sub(1));
                                let ty = ((v * t.height() as f32).floor() as u32).min(t.height().saturating_sub(1));
                                let tp = t.get_pixel(tx, ty).0;
                                for k in 0..4 {
                                    col[k] *= tp[k] as f32 / 255.0;
                                }
                            }
                            let src_a = col[3] / 255.0;
                            if src_a > 0.0 {
                                let dst = fb.get_pixel(px as u32, py as u32).0;
                                let inv_a = 1.0 - src_a;
                                let out_px = [
                                    (col[0] + dst[0] as f32 * inv_a).round().clamp(0.0, 255.0) as u8,
                                    (col[1] + dst[1] as f32 * inv_a).round().clamp(0.0, 255.0) as u8,
                                    (col[2] + dst[2] as f32 * inv_a).round().clamp(0.0, 255.0) as u8,
                                    255,
                                ];
                                fb.put_pixel(px as u32, py as u32, image::Rgba(out_px));
                            }
                        }
                    }
                }
            }
        }
        fb
    }

    /// The fill picker of a selected rectangle in OKLCH, drawn by the software rasterizer; the OKLCH field is focused.
    struct HeadlessPicker {
        app: crate::app::App,
        ctx: egui::Context,
        atlas: std::collections::HashMap<egui::TextureId, image::RgbaImage>,
        ok_id: Id,
    }

    impl HeadlessPicker {
        const SCREEN: Vec2 = Vec2::new(500.0, 900.0);

        /// Opens the picker on a `52, 120, 200` fill with the OKLCH field focused, its caret at `caret`, holding `text`
        /// (the field's own text when `None`).
        fn open(text: Option<&str>, caret: usize) -> Self {
            let mut doc = designos_doc::Document::new("oklch-render");
            let page = doc.add_page("Page 1");
            let mut app = crate::app::App::new(doc);
            app.page = page;
            let rect_id = app.doc.alloc_id();
            let mut node = designos_doc::Node::rectangle(rect_id, designos_doc::Rect::new(0.0, 0.0, 100.0, 100.0));
            node.fills = vec![Paint::solid(Color::rgb(52, 120, 200))];
            let rect_id = app.doc.insert(page, node).unwrap();
            app.selection = vec![rect_id];
            let mut h = Self { app, ctx: test_ctx(), atlas: std::collections::HashMap::new(), ok_id: Id::new("color-picker").with(("txt", "picker-ok")) };

            let _ = h.frame(vec![]);
            let sw_rect = h.ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", "sw:false:0".to_string())))).expect("fill swatch");
            let _ = h.frame(vec![egui::Event::PointerMoved(sw_rect.center())]);
            for pressed in [true, false] {
                let _ = h.frame(vec![egui::Event::PointerButton {
                    pos: sw_rect.center(),
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }]);
            }
            h.app.props.picker.as_mut().expect("picker open").state.fmt = Fmt::Oklch;
            let _ = h.frame(vec![]);
            h.ctx.memory_mut(|m| m.request_focus(h.ok_id));
            let _ = h.frame(vec![]);
            let _ = h.frame(vec![]);
            if let Some(text) = text {
                h.ctx.data_mut(|d| d.insert_temp(h.ok_id, text.to_string()));
            }
            let mut state = egui::widgets::text_edit::TextEdit::load_state(&h.ctx, h.ok_id).unwrap_or_default();
            state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(caret))));
            state.store(&h.ctx, h.ok_id);
            h
        }

        fn frame(&mut self, events: Vec<egui::Event>) -> Vec<egui::epaint::ClippedShape> {
            let region = Rect::from_min_size(Pos2::new(260.0, 0.0), Vec2::new(240.0, 900.0));
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Self::SCREEN)), events, ..Default::default() };
            let app = &mut self.app;
            let out = self.ctx.run_ui(raw, |ui| crate::panels::properties::show(app, ui, region));
            apply_textures_delta(&mut self.atlas, &out.textures_delta);
            out.shapes
        }

        fn press(&mut self, key: egui::Key) {
            let _ = self.frame(vec![egui::Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }]);
        }

        /// The whole window and the picker panel cut out of it, for the frame that drew `shapes`.
        fn capture(&self, shapes: Vec<egui::epaint::ClippedShape>) -> (image::RgbaImage, image::RgbaImage) {
            let picker_rect = self.ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", "picker".to_string())))).expect("picker rect");
            let full = rasterize_shapes(&self.ctx, shapes, &self.atlas, Self::SCREEN.x as u32, Self::SCREEN.y as u32);
            let crop = image::imageops::crop_imm(
                &full,
                picker_rect.min.x.max(0.0) as u32,
                picker_rect.min.y.max(0.0) as u32,
                picker_rect.width() as u32,
                picker_rect.height() as u32,
            )
            .to_image();
            (full, crop)
        }
    }

    /// Saves the picker crops under `before_name` / `after_name` when `DESIGNOS_PICKER_EVIDENCE_DIR` is set, and the
    /// whole windows (`window-` in front of the name) when `DESIGNOS_PICKER_FULL_DIR` is set too.
    fn save_headless_picker_evidence(
        before: &(image::RgbaImage, image::RgbaImage),
        after: &(image::RgbaImage, image::RgbaImage),
        before_name: &str,
        after_name: &str,
    ) {
        let Ok(ev_dir) = std::env::var("DESIGNOS_PICKER_EVIDENCE_DIR") else { return };
        let ev = std::path::Path::new(&ev_dir);
        before.1.save(ev.join(before_name)).expect("save before crop");
        after.1.save(ev.join(after_name)).expect("save after crop");
        if let Ok(full_dir) = std::env::var("DESIGNOS_PICKER_FULL_DIR") {
            let fd = std::path::Path::new(&full_dir);
            before.0.save(fd.join(format!("window-{before_name}"))).expect("save full before");
            after.0.save(fd.join(format!("window-{after_name}"))).expect("save full after");
        }
    }

    #[test]
    fn headless_picker_render_oklch_before_and_after_three_arrowup_on_h() {
        // Caret on H (char 14 of "56.93% 0.14 254.47").
        let mut h = HeadlessPicker::open(None, 14);
        let shapes = h.frame(vec![]);
        let (buf_before, caret_before) = picker_ok_buf_and_caret(&h.ctx, h.ok_id);
        assert_eq!(buf_before, "56.93% 0.14 254.47");
        assert_eq!(caret_before, 14);
        let before = h.capture(shapes);

        for _ in 0..3 {
            h.press(egui::Key::ArrowUp);
        }
        let shapes = h.frame(vec![]);
        let (buf_after, caret_after) = picker_ok_buf_and_caret(&h.ctx, h.ok_id);
        assert_eq!(buf_after, "56.93% 0.14 257.47");
        assert_eq!(caret_after, 14);
        let after = h.capture(shapes);

        assert_eq!(before.1.dimensions(), after.1.dimensions());
        assert_ne!(before.1.as_raw(), after.1.as_raw(), "rendered picker crop must visually change after 3 ArrowUp presses on H");
        save_headless_picker_evidence(&before, &after, "picker-oklch-before-3-arrowup-h.png", "picker-oklch-after-3-arrowup-h.png");
    }

    #[test]
    fn headless_picker_render_oklab_function_in_the_oklch_field_before_and_after_arrowup_on_l() {
        // Caret on L (char 7 of "oklab(50% 0.2 -0.1)").
        let mut h = HeadlessPicker::open(Some("oklab(50% 0.2 -0.1)"), 7);
        let shapes = h.frame(vec![]);
        let (buf_before, caret_before) = picker_ok_buf_and_caret(&h.ctx, h.ok_id);
        assert_eq!(buf_before, "oklab(50% 0.2 -0.1)");
        assert_eq!(caret_before, 7);
        let before = h.capture(shapes);

        h.press(egui::Key::ArrowUp);
        let shapes = h.frame(vec![]);
        let (buf_after, caret_after) = picker_ok_buf_and_caret(&h.ctx, h.ok_id);
        assert_eq!(buf_after, "oklab(51% 0.2 -0.1)");
        assert_eq!(caret_after, 7);
        let after = h.capture(shapes);

        assert_ne!(before.1.as_raw(), after.1.as_raw(), "the panel must show the stepped function text and its colour");
        save_headless_picker_evidence(&before, &after, "picker-oklch-field-oklab-function-before.png", "picker-oklch-field-oklab-function-after-arrowup-l.png");
    }

    fn test_image_paint(scale_mode: ImageScaleMode) -> Paint {
        Paint {
            kind: PaintKind::Image { image_ref: "proc-studio-grid".into(), scale_mode, params: designos_doc::ImageParams::default() },
            opacity: 1.0,
            visible: true,
            blend: designos_doc::BlendMode::Normal,
        }
    }

    fn click_rect(ctx: &egui::Context, rect: Rect, paint: &mut Paint, st: &mut State) {
        let c = rect.center();
        let _ = run_picker_frame(ctx, vec![egui::Event::PointerMoved(c)], paint, st);
        let _ = run_picker_frame(
            ctx,
            vec![egui::Event::PointerButton { pos: c, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE }],
            paint,
            st,
        );
        let _ = run_picker_frame(
            ctx,
            vec![egui::Event::PointerButton { pos: c, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE }],
            paint,
            st,
        );
    }

    #[test]
    fn picker_adjustment_row_edits_through_the_paint_path() {
        let mut doc = designos_doc::Document::new("img-adj-test");
        let page = doc.add_page("Page 1");
        let (_, png_bytes) = synthesize_procedural_png(0);
        doc.images.insert("proc-studio-grid".into(), png_bytes);
        let mut app = crate::app::App::new(doc);
        app.page = page;
        let rect_id = app.doc.alloc_id();
        let mut node = designos_doc::Node::rectangle(rect_id, designos_doc::Rect::new(0.0, 0.0, 100.0, 100.0));
        node.fills = vec![test_image_paint(ImageScaleMode::Fill)];
        let rect_id = app.doc.insert(page, node).unwrap();
        app.selection = vec![rect_id];

        let mut h = HeadlessPicker { app, ctx: test_ctx(), atlas: std::collections::HashMap::new(), ok_id: Id::new("color-picker").with(("txt", "picker-ok")) };

        let _ = h.frame(vec![]);
        let sw_rect = h.ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", "sw:false:0".to_string())))).expect("fill swatch");
        let _ = h.frame(vec![egui::Event::PointerMoved(sw_rect.center())]);
        for pressed in [true, false] {
            let _ = h.frame(vec![egui::Event::PointerButton {
                pos: sw_rect.center(),
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            }]);
        }
        let shapes = h.frame(vec![]);
        let texts = collect_visible_texts(&shapes);
        let expected_labels = ["Adjustments", "Exposure", "Contrast", "Saturation", "Temperature", "Tint", "Highlights", "Shadows"];
        for label in expected_labels {
            assert!(texts.iter().any(|t| t == label), "missing label {label:?} in image picker texts: {texts:?}");
        }
        assert!(!texts.iter().any(|t| t == "Reset"), "Reset must be hidden when all seven adjustments are 0.0: {texts:?}");

        let exp_slider =
            h.ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", "picker-image-adj-slider:Exposure".to_string())))).expect("Exposure slider rect");
        let start = exp_slider.center();
        let target = Pos2::new(exp_slider.left() + exp_slider.width() * 0.75, exp_slider.center().y);

        assert!(!h.app.hist.can_undo(), "no undo step before dragging Exposure");
        let _ = h.frame(vec![
            egui::Event::PointerMoved(start),
            egui::Event::PointerButton { pos: start, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
        ]);
        let _ = h.frame(vec![egui::Event::PointerMoved(target)]);

        let fill_during = &h.app.doc.get(rect_id).unwrap().fills[0];
        let PaintKind::Image { params, .. } = &fill_during.kind else { panic!("expected image paint") };
        assert!((params.adjust.exposure - 0.5).abs() < 0.05, "expected exposure ~0.5 during drag, got {}", params.adjust.exposure);
        assert!(!h.app.hist.can_undo(), "dragging must not commit an undo step before pointer release");

        // Release pointer: must commit exactly one undo step.
        let _ =
            h.frame(vec![egui::Event::PointerButton { pos: target, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE }]);
        assert!(h.app.hist.can_undo(), "releasing adjustment slider must commit one undo step");
        let shapes_after_drag = h.frame(vec![]);
        let texts_after = collect_visible_texts(&shapes_after_drag);
        assert!(texts_after.iter().any(|t| t == "Reset"), "Reset button must appear when Exposure is non-zero: {texts_after:?}");
        let cap_after = h.capture(shapes_after_drag);

        // Also set Saturation (vibrance) to non-zero, then click Reset and verify all 7 reset to 0.0.
        if let Some(pk) = h.app.props.picker.as_mut() {
            if let PaintKind::Image { params, .. } = &mut pk.paint.kind {
                params.adjust.vibrance = -0.4;
                params.adjust.contrast = 0.25;
            }
        }
        let _ = h.frame(vec![]);
        let reset_r = h.ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", "picker-image-adj-reset".to_string())))).expect("Reset button rect");
        let _ = h.frame(vec![egui::Event::PointerMoved(reset_r.center())]);
        for pressed in [true, false] {
            let _ = h.frame(vec![egui::Event::PointerButton {
                pos: reset_r.center(),
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            }]);
        }
        let shapes_reset = h.frame(vec![]);
        let fill_reset = &h.app.doc.get(rect_id).unwrap().fills[0];
        let PaintKind::Image { params, .. } = &fill_reset.kind else { panic!("expected image paint") };
        assert_eq!(params.adjust.exposure, 0.0);
        assert_eq!(params.adjust.contrast, 0.0);
        assert_eq!(params.adjust.vibrance, 0.0);
        assert_eq!(params.adjust.temperature, 0.0);
        assert_eq!(params.adjust.tint, 0.0);
        assert_eq!(params.adjust.highlights, 0.0);
        assert_eq!(params.adjust.shadows, 0.0);
        let texts_reset = collect_visible_texts(&shapes_reset);
        assert!(!texts_reset.iter().any(|t| t == "Reset"), "Reset must hide again after clearing all seven adjustments");
        let cap_reset = h.capture(shapes_reset);
        assert_ne!(cap_after.1.as_raw(), cap_reset.1.as_raw(), "rendered image picker crop must change after Reset");
        save_headless_picker_evidence(&cap_after, &cap_reset, "picker-image-adjustments-fill-exposure.png", "picker-image-adjustments-reset.png");
    }

    #[test]
    fn picker_image_rows_are_a_full_control_tall_and_fit_the_panel() {
        let ctx = test_ctx();
        let mut paint = test_image_paint(ImageScaleMode::Tile);
        let mut st = State::new(&paint);
        let _ = visible_texts(&ctx, &mut paint, &mut st);
        let rect_of = |name: &str| ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", name.to_string())))).unwrap_or_else(|| panic!("no rect {name}"));
        assert_eq!(IMAGE_ROW_H, theme::CONTROL_XS, "image rows are 32 tall");
        for name in ["picker-image-tile-val", "picker-image-adj-val:Exposure", "picker-image-adj-val:Shadows"] {
            assert_eq!(rect_of(name).height(), theme::CONTROL_XS, "{name} value cell is 32 tall");
        }
        let panel = rect_of("picker");
        let disk = rect_of("picker-image-disk");
        assert!(disk.bottom() <= panel.bottom(), "the last image control {disk:?} must end inside the panel {panel:?}");
        let rows = [rect_of("picker-image-adj-val:Exposure"), rect_of("picker-image-adj-val:Contrast")];
        assert_eq!(rows[1].top() - rows[0].top(), IMAGE_ROW_STEP, "rows step by their height plus a 1 px gap");
    }

    #[test]
    fn picker_rotate_buttons_step_by_90_degrees() {
        let ctx = test_ctx();
        let mut paint = test_image_paint(ImageScaleMode::Fill);
        let mut st = State::new(&paint);
        let texts = visible_texts(&ctx, &mut paint, &mut st);
        assert!(texts.iter().any(|t| t == "Rotate"), "missing Rotate row label in {texts:?}");
        assert!(texts.iter().any(|t| t == "0°"), "missing 0° angle label in {texts:?}");

        let rot_right = ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", "picker-image-rot-right".to_string())))).expect("rotate-right button rect");
        let rot_left = ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", "picker-image-rot-left".to_string())))).expect("rotate-left button rect");

        for expected in [90.0_f32, 180.0, 270.0, 0.0] {
            click_rect(&ctx, rot_right, &mut paint, &mut st);
            let PaintKind::Image { params, .. } = &paint.kind else { panic!("expected image paint") };
            assert!((params.rotation - expected).abs() < 1e-5, "rotate-right expected {expected}, got {}", params.rotation);
        }
        for expected in [270.0_f32, 180.0, 90.0, 0.0] {
            click_rect(&ctx, rot_left, &mut paint, &mut st);
            let PaintKind::Image { params, .. } = &paint.kind else { panic!("expected image paint") };
            assert!((params.rotation - expected).abs() < 1e-5, "rotate-left expected {expected}, got {}", params.rotation);
        }
    }

    #[test]
    fn picker_tile_scale_row_shown_only_in_tile_mode_and_edits_scale() {
        let ctx = test_ctx();
        for mode in [ImageScaleMode::Fill, ImageScaleMode::Fit, ImageScaleMode::Crop] {
            let mut paint = test_image_paint(mode);
            let mut st = State::new(&paint);
            let texts = visible_texts(&ctx, &mut paint, &mut st);
            assert!(!texts.iter().any(|t| t == "Tile scale"), "Tile scale must be hidden in {mode:?}: {texts:?}");
        }

        let mut paint = test_image_paint(ImageScaleMode::Tile);
        let mut st = State::new(&paint);
        let texts = visible_texts(&ctx, &mut paint, &mut st);
        assert!(texts.iter().any(|t| t == "Tile scale"), "Tile scale must be visible in Tile mode: {texts:?}");

        let tile_slider = ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", "picker-image-tile-slider".to_string())))).expect("tile scale slider rect");
        let target = Pos2::new(tile_slider.left() + tile_slider.width() * 0.5, tile_slider.center().y);
        let _ = run_picker_frame(
            &ctx,
            vec![
                egui::Event::PointerMoved(target),
                egui::Event::PointerButton { pos: target, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
            ],
            &mut paint,
            &mut st,
        );
        let _ = run_picker_frame(
            &ctx,
            vec![egui::Event::PointerButton { pos: target, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE }],
            &mut paint,
            &mut st,
        );
        let PaintKind::Image { params, .. } = &paint.kind else { panic!("expected image paint") };
        assert!((params.tile_scale - 1.0).abs() > 0.05, "dragging Tile scale slider must update params.tile_scale from 1.0, got {}", params.tile_scale);
    }

    // ---- Crop frames per node, rotation in Crop, and the picker stamping

    type Rgba = [u8; 4];
    const RED_PX: Rgba = [255, 0, 0, 255];
    const GREEN_PX: Rgba = [0, 255, 0, 255];
    const BLUE_PX: Rgba = [0, 0, 255, 255];
    const YELLOW_PX: Rgba = [255, 255, 0, 255];

    fn encode_png(w: u32, h: u32, f: impl Fn(u32, u32) -> Rgba) -> Vec<u8> {
        let img = image::RgbaImage::from_fn(w, h, |x, y| image::Rgba(f(x, y)));
        let mut out = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).expect("encode png");
        out
    }

    /// Four colour blocks, red green over blue yellow.
    fn quadrant_png(side: u32) -> Vec<u8> {
        encode_png(side, side, |x, y| match (x * 2 / side, y * 2 / side) {
            (0, 0) => RED_PX,
            (1, 0) => GREEN_PX,
            (0, 1) => BLUE_PX,
            _ => YELLOW_PX,
        })
    }

    fn image_paint_of(mode: ImageScaleMode, params: designos_doc::ImageParams) -> Paint {
        Paint {
            kind: PaintKind::Image { image_ref: "picture".into(), scale_mode: mode, params },
            opacity: 1.0,
            visible: true,
            blend: designos_doc::BlendMode::Normal,
        }
    }

    impl HeadlessPicker {
        /// Opens the fill picker on the first fill of every node in a row of image rectangles, all selected.
        fn open_image(image: Vec<u8>, nodes: &[((f32, f32), ImageScaleMode, designos_doc::ImageParams)]) -> (Self, Vec<designos_doc::NodeId>) {
            let mut doc = designos_doc::Document::new("image-picker");
            let page = doc.add_page("Page 1");
            doc.images.insert("picture".into(), image);
            let mut app = crate::app::App::new(doc);
            app.page = page;
            let mut ids = Vec::new();
            let mut x = 0.0;
            for ((w, h), mode, params) in nodes {
                let id = app.doc.alloc_id();
                let mut node = designos_doc::Node::rectangle(id, designos_doc::Rect::new(x, 0.0, *w, *h));
                node.fills = vec![image_paint_of(*mode, *params)];
                ids.push(app.doc.insert(page, node).unwrap());
                x += *w + 20.0;
            }
            app.selection = ids.clone();
            let mut h = Self { app, ctx: test_ctx(), atlas: std::collections::HashMap::new(), ok_id: Id::new("color-picker").with(("txt", "picker-ok")) };
            let _ = h.frame(vec![]);
            h.click_key("sw:false:0");
            assert!(h.app.props.picker.is_some(), "the fill swatch click opens the picker");
            let _ = h.frame(vec![]);
            (h, ids)
        }

        /// Closes the picker and opens it again on the first fill of `ids`, which become the selection.
        fn reopen_on(&mut self, ids: &[designos_doc::NodeId]) {
            self.app.props.picker = None;
            self.app.selection = ids.to_vec();
            let _ = self.frame(vec![]);
            self.click_key("sw:false:0");
            assert!(self.app.props.picker.is_some(), "the fill swatch click opens the picker again");
            let _ = self.frame(vec![]);
        }

        fn rect_of(&self, key: &str) -> Rect {
            self.ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", key.to_string())))).unwrap_or_else(|| panic!("no recorded rect {key}"))
        }

        fn click_key(&mut self, key: &str) {
            let c = self.rect_of(key).center();
            let _ = self.frame(vec![egui::Event::PointerMoved(c)]);
            for pressed in [true, false] {
                let _ =
                    self.frame(vec![egui::Event::PointerButton { pos: c, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }]);
            }
            let _ = self.frame(vec![]);
        }

        /// Drags the slider recorded as `key` from its centre to `frac` of its width and releases.
        fn drag_slider(&mut self, key: &str, frac: f32) {
            let r = self.rect_of(key);
            let (from, to) = (r.center(), Pos2::new(r.left() + r.width() * frac, r.center().y));
            let _ = self.frame(vec![
                egui::Event::PointerMoved(from),
                egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
            ]);
            let _ = self.frame(vec![egui::Event::PointerMoved(to)]);
            let _ = self.frame(vec![egui::Event::PointerButton {
                pos: to,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }]);
            let _ = self.frame(vec![]);
        }

        fn image_params(&self, id: designos_doc::NodeId) -> (ImageScaleMode, designos_doc::ImageParams) {
            match &self.app.doc.get(id).unwrap().fills[0].kind {
                PaintKind::Image { scale_mode, params, .. } => (*scale_mode, *params),
                other => panic!("expected an image fill, got {other:?}"),
            }
        }

        fn pixels(&self, id: designos_doc::NodeId) -> designos_render::tiny_skia::Pixmap {
            designos_render::render_node(&self.app.doc, id, 1.0).expect("render node")
        }

        /// What the node looks like when its fill is `mode` with `params`, whatever it holds now.
        fn reference_pixels(&self, id: designos_doc::NodeId, mode: ImageScaleMode, params: designos_doc::ImageParams) -> designos_render::tiny_skia::Pixmap {
            let mut doc = self.app.doc.clone();
            doc.get_mut(id).unwrap().fills = vec![image_paint_of(mode, params)];
            designos_render::render_node(&doc, id, 1.0).expect("render reference")
        }
    }

    fn rgba_at(pm: &designos_render::tiny_skia::Pixmap, x: u32, y: u32) -> Rgba {
        let p = pm.pixel(x, y).unwrap();
        [p.red(), p.green(), p.blue(), p.alpha()]
    }

    fn assert_same_pixels(got: &designos_render::tiny_skia::Pixmap, want: &designos_render::tiny_skia::Pixmap, tolerance: u8, what: &str) {
        assert_eq!((got.width(), got.height()), (want.width(), want.height()), "{what}: size");
        let worst = got.data().iter().zip(want.data()).map(|(a, b)| a.abs_diff(*b)).max().unwrap_or(0);
        assert!(worst <= tolerance, "{what}: pixels differ by up to {worst} levels");
    }

    #[test]
    fn rotating_a_cropped_picture_turns_the_pixels_a_quarter_turn_each_click() {
        let quad = encode_png(2, 2, |x, y| match (x, y) {
            (0, 0) => RED_PX,
            (1, 0) => GREEN_PX,
            (0, 1) => BLUE_PX,
            _ => YELLOW_PX,
        });
        let (mut h, ids) = HeadlessPicker::open_image(quad, &[((2.0, 2.0), ImageScaleMode::Crop, designos_doc::ImageParams::default())]);
        let id = ids[0];
        let at = |h: &HeadlessPicker| {
            let pm = h.pixels(id);
            [rgba_at(&pm, 0, 0), rgba_at(&pm, 1, 0), rgba_at(&pm, 0, 1), rgba_at(&pm, 1, 1)]
        };
        assert_eq!(at(&h), [RED_PX, GREEN_PX, BLUE_PX, YELLOW_PX], "the identity crop shows the picture as it is");

        h.click_key("picker-image-rot-right");
        assert_eq!(at(&h), [BLUE_PX, RED_PX, YELLOW_PX, GREEN_PX], "rotate right turns the pixels clockwise");
        assert!((h.image_params(id).1.rotation - 90.0).abs() < 1e-4, "the angle label follows the turn");

        h.click_key("picker-image-rot-left");
        assert_eq!(at(&h), [RED_PX, GREEN_PX, BLUE_PX, YELLOW_PX], "rotate left undoes it");

        h.click_key("picker-image-rot-left");
        assert_eq!(at(&h), [GREEN_PX, YELLOW_PX, RED_PX, BLUE_PX], "rotate left again turns the pixels counter-clockwise");
        assert!((h.image_params(id).1.rotation - 270.0).abs() < 1e-4);
    }

    #[test]
    fn a_fit_picture_turned_a_quarter_then_cropped_then_rotated_shows_the_half_turn_framing() {
        let params90 = designos_doc::ImageParams { rotation: 90.0, ..Default::default() };
        let (mut h, ids) = HeadlessPicker::open_image(quadrant_png(100), &[((200.0, 100.0), ImageScaleMode::Fit, params90)]);
        let id = ids[0];
        let fit90 = h.reference_pixels(id, ImageScaleMode::Fit, params90);

        h.click_key("picker-image-scale");
        assert_eq!(h.image_params(id).0, ImageScaleMode::Crop, "Fit goes to Crop on the next click");
        assert_same_pixels(&h.pixels(id), &fit90, 1, "Crop keeps the Fit framing at 90 degrees");

        h.click_key("picker-image-rot-right");
        let fit180 = h.reference_pixels(id, ImageScaleMode::Fit, designos_doc::ImageParams { rotation: 180.0, ..Default::default() });
        assert_same_pixels(&h.pixels(id), &fit180, 1, "one more quarter turn in Crop is the half-turn framing");
        assert!((h.image_params(id).1.rotation - 180.0).abs() < 1e-4);
    }

    #[test]
    fn cropping_several_nodes_gives_each_its_own_framing_in_one_undo_step() {
        let fit = designos_doc::ImageParams::default();
        let (mut h, ids) =
            HeadlessPicker::open_image(quadrant_png(100), &[((200.0, 100.0), ImageScaleMode::Fit, fit), ((100.0, 200.0), ImageScaleMode::Fit, fit)]);
        let own_framing: Vec<_> = ids.iter().map(|id| h.reference_pixels(*id, ImageScaleMode::Fit, fit)).collect();
        let depth = h.app.hist.undo_depth();

        h.click_key("picker-image-scale");
        for (i, id) in ids.iter().enumerate() {
            assert_eq!(h.image_params(*id).0, ImageScaleMode::Crop, "node {i} is cropped");
            assert_same_pixels(&h.pixels(*id), &own_framing[i], 1, &format!("node {i} keeps its own framing"));
        }
        assert_eq!(h.app.hist.undo_depth(), depth + 1, "switching both nodes is one undo step");

        h.app.undo();
        for id in &ids {
            assert_eq!(h.image_params(*id).0, ImageScaleMode::Fit, "one undo puts both nodes back");
        }
        assert_eq!(h.app.hist.undo_depth(), depth);
    }

    #[test]
    fn editing_an_adjustment_keeps_every_nodes_own_crop_frame() {
        let fit = designos_doc::ImageParams::default();
        let (mut h, ids) =
            HeadlessPicker::open_image(quadrant_png(100), &[((200.0, 100.0), ImageScaleMode::Fit, fit), ((100.0, 200.0), ImageScaleMode::Fit, fit)]);
        h.click_key("picker-image-scale");
        let frames: Vec<_> = ids.iter().map(|id| h.image_params(*id).1.transform).collect();
        assert!(frames.iter().all(|f| f.is_some()), "both nodes were given a frame: {frames:?}");
        assert_ne!(frames[0], frames[1], "the two nodes have different frames");

        h.drag_slider("picker-image-adj-slider:Exposure", 0.75);
        for (i, id) in ids.iter().enumerate() {
            let (mode, params) = h.image_params(*id);
            assert_eq!(mode, ImageScaleMode::Crop);
            assert!(params.adjust.exposure > 0.3, "node {i} got the exposure: {}", params.adjust.exposure);
            assert_eq!(params.transform, frames[i], "node {i} keeps its own frame while a slider moves");
        }
    }

    #[test]
    fn a_picture_that_cannot_be_decoded_gets_no_invented_crop_frame() {
        let (mut h, ids) = HeadlessPicker::open_image(b"not a picture".to_vec(), &[((120.0, 80.0), ImageScaleMode::Fit, designos_doc::ImageParams::default())]);
        h.click_key("picker-image-scale");
        let (mode, params) = h.image_params(ids[0]);
        assert_eq!(mode, ImageScaleMode::Crop);
        assert_eq!(params.transform, None, "an unknown picture size leaves the crop at the identity");
    }

    #[test]
    fn rotating_several_cropped_nodes_turns_each_about_its_own_centre() {
        let fit = designos_doc::ImageParams::default();
        let (mut h, ids) =
            HeadlessPicker::open_image(quadrant_png(100), &[((200.0, 100.0), ImageScaleMode::Fit, fit), ((100.0, 200.0), ImageScaleMode::Fit, fit)]);
        h.click_key("picker-image-scale");
        h.click_key("picker-image-rot-right");
        let turned = designos_doc::ImageParams { rotation: 90.0, ..Default::default() };
        for (i, id) in ids.iter().enumerate() {
            // a square picture keeps its footprint when it turns, so the turned frame is the Fit framing at 90 degrees
            let want = h.reference_pixels(*id, ImageScaleMode::Fit, turned);
            assert_same_pixels(&h.pixels(*id), &want, 1, &format!("node {i} turned about its own centre"));
            assert!((h.image_params(*id).1.rotation - 90.0).abs() < 1e-4, "node {i} shows 90");
        }
    }

    #[test]
    fn picker_rotate_counts_the_turn_for_the_host_and_outside_crop_drops_the_frame() {
        let ctx = test_ctx();
        let mut paint = test_image_paint(ImageScaleMode::Crop);
        let mut st = State::new(&paint);
        let _ = visible_texts(&ctx, &mut paint, &mut st);
        let right = ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", "picker-image-rot-right".to_string())))).expect("rotate-right rect");
        let left = ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", "picker-image-rot-left".to_string())))).expect("rotate-left rect");
        click_rect(&ctx, right, &mut paint, &mut st);
        click_rect(&ctx, right, &mut paint, &mut st);
        click_rect(&ctx, left, &mut paint, &mut st);
        assert_eq!(st.image_turns, 1, "two clockwise clicks and one counter-clockwise click are one net turn");
        let PaintKind::Image { params, .. } = &paint.kind else { panic!("image paint") };
        assert!((params.rotation - 90.0).abs() < 1e-4, "the angle label follows the clicks");

        let mut fill = test_image_paint(ImageScaleMode::Fill);
        if let PaintKind::Image { params, .. } = &mut fill.kind {
            params.transform = Some([0.5, 0.0, 0.0, 0.0, 0.5, 0.0]);
        }
        let mut st = State::new(&fill);
        let _ = visible_texts(&ctx, &mut fill, &mut st);
        click_rect(&ctx, right, &mut fill, &mut st);
        let PaintKind::Image { params, .. } = &fill.kind else { panic!("image paint") };
        assert_eq!((st.image_turns, params.transform), (1, None), "outside Crop a turn drops a stale frame and is still counted for the host");
    }

    /// The picker of a tiled picture with adjustments: the Tile scale row, the angle row and the Reset action show.
    /// The window and the picker panel are saved into `DESIGNOS_PICKER_EVIDENCE_DIR` when it is set.
    #[test]
    fn picker_of_a_tiled_picture_shows_the_tile_scale_row_the_adjustments_and_reset() {
        let (mut h, ids) = HeadlessPicker::open_image(quadrant_png(64), &[((200.0, 120.0), ImageScaleMode::Tile, designos_doc::ImageParams::default())]);
        h.drag_slider("picker-image-adj-slider:Exposure", 0.75);
        h.drag_slider("picker-image-adj-slider:Contrast", 0.25);
        h.drag_slider("picker-image-tile-slider", 0.5);
        let shapes = h.frame(vec![]);
        let texts = collect_visible_texts(&shapes);
        for want in ["Scale: Tile", "Rotate", "Tile scale", "Adjustments", "Reset", "50%", "-50%"] {
            assert!(texts.iter().any(|t| t == want), "missing {want:?} in {texts:?}");
        }
        let (mode, params) = h.image_params(ids[0]);
        assert_eq!(mode, ImageScaleMode::Tile);
        assert!((params.adjust.exposure - 0.5).abs() < 0.05 && (params.adjust.contrast + 0.15).abs() < 0.02, "adjustments: {:?}", params.adjust);
        assert!(params.tile_scale > 1.5, "tile scale {}", params.tile_scale);
        let cap = h.capture(shapes);
        if let Ok(dir) = std::env::var("DESIGNOS_PICKER_EVIDENCE_DIR") {
            cap.1.save(std::path::Path::new(&dir).join("r1-picker-tile.png")).expect("save picker");
        }
    }

    /// The picker of a cropped picture after a rotate click, beside the node before and after the click. Pieces are
    /// saved into `DESIGNOS_PICKER_EVIDENCE_DIR` when it is set.
    #[test]
    fn picker_of_a_cropped_picture_shows_the_turned_angle_and_the_node_turns() {
        let (mut h, ids) = HeadlessPicker::open_image(quadrant_png(64), &[((200.0, 120.0), ImageScaleMode::Crop, designos_doc::ImageParams::default())]);
        let id = ids[0];
        let before = h.pixels(id);
        h.click_key("picker-image-rot-right");
        let after = h.pixels(id);
        let shapes = h.frame(vec![]);
        let texts = collect_visible_texts(&shapes);
        assert!(texts.iter().any(|t| t == "Scale: Crop") && texts.iter().any(|t| t == "90°"), "crop picker texts: {texts:?}");
        assert_ne!(before.data(), after.data(), "the picture on the node turned");
        let cap = h.capture(shapes);
        if let Ok(dir) = std::env::var("DESIGNOS_PICKER_EVIDENCE_DIR") {
            let dir = std::path::Path::new(&dir);
            before.save_png(dir.join("r1-crop-rotate-node-before.png")).expect("save before");
            after.save_png(dir.join("r1-crop-rotate-node-after.png")).expect("save after");
            cap.1.save(dir.join("r1-crop-rotate-picker.png")).expect("save picker");
        }
    }

    #[test]
    fn leaving_crop_and_coming_back_gives_each_node_its_own_frame_again() {
        let fit = designos_doc::ImageParams::default();
        let (mut h, ids) =
            HeadlessPicker::open_image(quadrant_png(100), &[((200.0, 100.0), ImageScaleMode::Fit, fit), ((100.0, 200.0), ImageScaleMode::Fit, fit)]);
        let own_framing: Vec<_> = ids.iter().map(|id| h.reference_pixels(*id, ImageScaleMode::Fit, fit)).collect();
        h.click_key("picker-image-scale");
        let frames: Vec<_> = ids.iter().map(|id| h.image_params(*id).1.transform).collect();
        assert_ne!(frames[0], frames[1], "each node has its own frame in Crop");
        // Tile, Fill, Fit: the picker holds the first node's frame all along, and no node may take it over.
        for mode in [ImageScaleMode::Tile, ImageScaleMode::Fill, ImageScaleMode::Fit] {
            h.click_key("picker-image-scale");
            for (i, id) in ids.iter().enumerate() {
                let (now, params) = h.image_params(*id);
                assert_eq!(now, mode);
                assert_eq!(params.transform, frames[i], "node {i} still stores its own frame in {mode:?}");
            }
        }
        h.click_key("picker-image-scale");
        for (i, id) in ids.iter().enumerate() {
            assert_eq!(h.image_params(*id).0, ImageScaleMode::Crop);
            assert_same_pixels(&h.pixels(*id), &own_framing[i], 1, &format!("node {i} is framed by its own frame after the round trip"));
        }
    }

    #[test]
    fn leaving_crop_keeps_each_nodes_own_angle_and_coming_back_restores_it() {
        let crop = designos_doc::ImageParams::default();
        let (mut h, ids) =
            HeadlessPicker::open_image(quadrant_png(100), &[((200.0, 100.0), ImageScaleMode::Crop, crop), ((100.0, 200.0), ImageScaleMode::Crop, crop)]);
        let (a, b) = (ids[0], ids[1]);
        // B alone is turned with the Crop rotate; A stays at 0 degrees. Then both are selected, A first.
        h.reopen_on(&[b]);
        h.click_key("picker-image-rot-right");
        h.reopen_on(&[a, b]);
        assert_eq!((h.image_params(a).1.rotation, h.image_params(b).1.rotation), (0.0, 90.0));
        let in_crop = [h.pixels(a), h.pixels(b)];
        assert_ne!(
            h.reference_pixels(b, ImageScaleMode::Tile, designos_doc::ImageParams::default()).data(),
            h.reference_pixels(b, ImageScaleMode::Tile, designos_doc::ImageParams { rotation: 90.0, ..Default::default() }).data(),
            "a quarter turn shows in Tile"
        );
        let depth = h.app.hist.undo_depth();

        // Crop, Tile: the node turned to 90 degrees keeps 90 and draws its Tile a quarter turn.
        h.click_key("picker-image-scale");
        assert_eq!(h.app.hist.undo_depth(), depth + 1, "switching both nodes to Tile is one undo step");
        let tile_a = h.reference_pixels(a, ImageScaleMode::Tile, designos_doc::ImageParams::default());
        let tile_b = h.reference_pixels(b, ImageScaleMode::Tile, designos_doc::ImageParams { rotation: 90.0, ..Default::default() });
        assert_eq!(h.image_params(a).0, ImageScaleMode::Tile);
        assert_eq!((h.image_params(a).1.rotation, h.image_params(b).1.rotation), (0.0, 90.0), "each node keeps its own angle in Tile");
        assert_same_pixels(&h.pixels(a), &tile_a, 1, "node A in Tile");
        assert_same_pixels(&h.pixels(b), &tile_b, 1, "node B in Tile shows its quarter turn");
        if let Ok(dir) = std::env::var("DESIGNOS_PICKER_EVIDENCE_DIR") {
            let dir = std::path::Path::new(&dir);
            // "b-tile-at-0" is what Tile drew for B when the first node's angle of 0 was written over its own.
            let b_at_zero = h.reference_pixels(b, ImageScaleMode::Tile, designos_doc::ImageParams::default());
            for (name, pm) in
                [("a-crop", &in_crop[0]), ("b-crop", &in_crop[1]), ("a-tile", &h.pixels(a)), ("b-tile", &h.pixels(b)), ("b-tile-at-0", &b_at_zero)]
            {
                pm.save_png(dir.join(format!("r2-two-node-angles-{name}.png"))).expect("save node render");
            }
        }

        // Tile, Fill, Fit, Crop: one undo step each, angles untouched all along.
        for (i, mode) in [ImageScaleMode::Fill, ImageScaleMode::Fit, ImageScaleMode::Crop].into_iter().enumerate() {
            h.click_key("picker-image-scale");
            assert_eq!(h.image_params(a).0, mode);
            assert_eq!(h.app.hist.undo_depth(), depth + 2 + i, "one undo step for the switch to {mode:?}");
            assert_eq!((h.image_params(a).1.rotation, h.image_params(b).1.rotation), (0.0, 90.0), "angles in {mode:?}");
        }
        // A never stored a frame (identity crop), so entering Crop from Fit frames the picture as Fit showed it.
        assert_same_pixels(
            &h.pixels(a),
            &h.reference_pixels(a, ImageScaleMode::Fit, designos_doc::ImageParams::default()),
            1,
            "node A is framed as its Fit was",
        );
        // B stored the frame of its turn, so its Crop pixels come back exactly.
        assert_same_pixels(&h.pixels(b), &in_crop[1], 1, "node B is back to its Crop pixels");
        assert_eq!(h.image_params(b).1.rotation, 90.0, "the angle label of B reads 90 again");
        if let Ok(dir) = std::env::var("DESIGNOS_PICKER_EVIDENCE_DIR") {
            let dir = std::path::Path::new(&dir);
            h.pixels(a).save_png(dir.join("r2-two-node-angles-a-back.png")).expect("save a");
            h.pixels(b).save_png(dir.join("r2-two-node-angles-b-back.png")).expect("save b");
        }
    }
}
