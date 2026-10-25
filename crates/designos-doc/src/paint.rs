use crate::Vec2;
use serde::{Deserialize, Serialize};

/// Straight (non-premultiplied) sRGB color, components 0..=1.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const BLACK: Self = Self { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const WHITE: Self = Self { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    pub const TRANSPARENT: Self = Self { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };

    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::rgba(r, g, b, 255)
    }
    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a: a as f32 / 255.0 }
    }
    /// Parses 1..8 digit hex (`#F`, `#F0`, `#123`, `#1234`, `#RRGGBB`, `#RRGGBBAA`),
    /// `"transparent"`, or CSS named colors (Figma module `640649`).
    pub fn from_hex(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }
        if !s.starts_with('#') {
            let lower = s.to_ascii_lowercase();
            let named = match lower.as_str() {
                "transparent" => Some(Self::TRANSPARENT),
                "black" => Some(Self::rgb(0, 0, 0)),
                "white" => Some(Self::rgb(255, 255, 255)),
                "red" => Some(Self::rgb(255, 0, 0)),
                "green" => Some(Self::rgb(0, 128, 0)),
                "blue" => Some(Self::rgb(0, 0, 255)),
                "yellow" => Some(Self::rgb(255, 255, 0)),
                "cyan" | "aqua" => Some(Self::rgb(0, 255, 255)),
                "magenta" | "fuchsia" => Some(Self::rgb(255, 0, 255)),
                "gray" | "grey" => Some(Self::rgb(128, 128, 128)),
                "lightgray" | "lightgrey" => Some(Self::rgb(211, 211, 211)),
                "darkgray" | "darkgrey" => Some(Self::rgb(169, 169, 169)),
                "dimgray" | "dimgrey" => Some(Self::rgb(105, 105, 105)),
                "slategray" | "slategrey" => Some(Self::rgb(112, 128, 144)),
                "silver" => Some(Self::rgb(192, 192, 192)),
                "maroon" => Some(Self::rgb(128, 0, 0)),
                "olive" => Some(Self::rgb(128, 128, 0)),
                "lime" => Some(Self::rgb(0, 255, 0)),
                "teal" => Some(Self::rgb(0, 128, 128)),
                "navy" => Some(Self::rgb(0, 0, 128)),
                "purple" => Some(Self::rgb(128, 0, 128)),
                "rebeccapurple" => Some(Self::rgb(102, 51, 153)),
                "orange" => Some(Self::rgb(255, 165, 0)),
                "orangered" => Some(Self::rgb(255, 69, 0)),
                "coral" => Some(Self::rgb(255, 127, 80)),
                "tomato" => Some(Self::rgb(255, 99, 71)),
                "salmon" => Some(Self::rgb(250, 128, 114)),
                "gold" => Some(Self::rgb(255, 215, 0)),
                "pink" => Some(Self::rgb(255, 192, 203)),
                "hotpink" => Some(Self::rgb(255, 105, 180)),
                "deeppink" => Some(Self::rgb(255, 20, 147)),
                "violet" => Some(Self::rgb(238, 130, 238)),
                "plum" => Some(Self::rgb(221, 160, 221)),
                "orchid" => Some(Self::rgb(218, 112, 214)),
                "indigo" => Some(Self::rgb(75, 0, 130)),
                "blueviolet" => Some(Self::rgb(138, 43, 226)),
                "dodgerblue" => Some(Self::rgb(30, 144, 255)),
                "deepskyblue" => Some(Self::rgb(0, 191, 255)),
                "skyblue" => Some(Self::rgb(135, 206, 235)),
                "lightblue" => Some(Self::rgb(173, 216, 230)),
                "steelblue" => Some(Self::rgb(70, 130, 180)),
                "royalblue" => Some(Self::rgb(65, 105, 225)),
                "midnightblue" => Some(Self::rgb(25, 25, 112)),
                "turquoise" => Some(Self::rgb(64, 224, 208)),
                "aquamarine" => Some(Self::rgb(127, 255, 212)),
                "springgreen" => Some(Self::rgb(0, 255, 127)),
                "limegreen" => Some(Self::rgb(50, 205, 50)),
                "forestgreen" => Some(Self::rgb(34, 139, 34)),
                "seagreen" => Some(Self::rgb(46, 139, 87)),
                "yellowgreen" => Some(Self::rgb(154, 205, 50)),
                "chartreuse" => Some(Self::rgb(127, 255, 0)),
                "brown" => Some(Self::rgb(165, 42, 42)),
                "firebrick" => Some(Self::rgb(178, 34, 34)),
                "crimson" => Some(Self::rgb(220, 20, 60)),
                "darkred" => Some(Self::rgb(139, 0, 0)),
                "chocolate" => Some(Self::rgb(210, 105, 30)),
                "saddlebrown" => Some(Self::rgb(139, 69, 19)),
                "sienna" => Some(Self::rgb(160, 82, 45)),
                "peru" => Some(Self::rgb(205, 133, 63)),
                "tan" => Some(Self::rgb(210, 180, 140)),
                "wheat" => Some(Self::rgb(245, 222, 179)),
                "beige" => Some(Self::rgb(245, 245, 220)),
                "ivory" => Some(Self::rgb(255, 255, 240)),
                "linen" => Some(Self::rgb(250, 240, 230)),
                "lavender" => Some(Self::rgb(230, 230, 250)),
                "aliceblue" => Some(Self::rgb(240, 248, 255)),
                "ghostwhite" => Some(Self::rgb(248, 248, 255)),
                "mintcream" => Some(Self::rgb(245, 255, 250)),
                "honeydew" => Some(Self::rgb(240, 255, 240)),
                "seashell" => Some(Self::rgb(255, 245, 238)),
                "snow" => Some(Self::rgb(255, 250, 250)),
                "whitesmoke" => Some(Self::rgb(245, 245, 245)),
                "gainsboro" => Some(Self::rgb(220, 220, 220)),
                "khaki" => Some(Self::rgb(240, 230, 140)),
                _ => None,
            };
            if named.is_some() {
                return named;
            }
        }
        let s = s.trim_start_matches('#');
        let nib = |i: usize| u8::from_str_radix(s.get(i..i + 1)?, 16).ok();
        let byte = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
        match s.len() {
            1 => {
                let v = nib(0)? * 17;
                Some(Self::rgb(v, v, v))
            }
            2 => {
                let v = byte(0)?;
                Some(Self::rgb(v, v, v))
            }
            3 => Some(Self::rgb(nib(0)? * 17, nib(1)? * 17, nib(2)? * 17)),
            4 => Some(Self::rgba(nib(0)? * 17, nib(1)? * 17, nib(2)? * 17, nib(3)? * 17)),
            5 => Some(Self::rgba(nib(0)? * 17, nib(1)? * 17, nib(2)? * 17, byte(3)?)),
            6 => Some(Self::rgb(byte(0)?, byte(2)?, byte(4)?)),
            7 => Some(Self::rgba(byte(0)?, byte(2)?, byte(4)?, nib(6)? * 17)),
            8 => Some(Self::rgba(byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
            _ => None,
        }
    }
    pub fn to_u8(&self) -> [u8; 4] {
        let c = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        [c(self.r), c(self.g), c(self.b), c(self.a)]
    }
    pub fn to_hex(&self) -> String {
        let [r, g, b, _] = self.to_u8();
        format!("{r:02X}{g:02X}{b:02X}")
    }
    pub fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlendMode {
    #[default]
    Normal,
    PassThrough,
    Darken,
    Multiply,
    ColorBurn,
    LinearBurn,
    Lighten,
    Screen,
    ColorDodge,
    LinearDodge,
    Overlay,
    SoftLight,
    HardLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GradientStop {
    pub pos: f32,
    pub color: Color,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageScaleMode {
    #[default]
    Fill,
    Fit,
    Crop,
    Tile,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PaintKind {
    Solid(Color),
    /// Handles in node-normalized space (0..1 of the node's size), Figma convention:
    /// [start, end, width-direction].
    LinearGradient {
        handles: [Vec2; 3],
        stops: Vec<GradientStop>,
    },
    RadialGradient {
        handles: [Vec2; 3],
        stops: Vec<GradientStop>,
    },
    AngularGradient {
        handles: [Vec2; 3],
        stops: Vec<GradientStop>,
    },
    DiamondGradient {
        handles: [Vec2; 3],
        stops: Vec<GradientStop>,
    },
    /// `image_ref` keys into [`crate::Document::images`].
    Image {
        image_ref: String,
        scale_mode: ImageScaleMode,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Paint {
    pub kind: PaintKind,
    pub opacity: f32,
    pub visible: bool,
    pub blend: BlendMode,
}

impl Paint {
    pub fn solid(c: Color) -> Self {
        Self { kind: PaintKind::Solid(c), opacity: 1.0, visible: true, blend: BlendMode::Normal }
    }

    /// The coverage this paint leaves in an effect silhouette (drop shadow, inner shadow,
    /// background blur, glass, noise): a solid colour contributes its own alpha, every other
    /// kind is opaque, and the paint opacity scales either. A colour variable keeps its alpha on
    /// the colour, so reading only `opacity` would ink the silhouette fully.
    pub fn silhouette_alpha(&self) -> f32 {
        let base = match &self.kind {
            PaintKind::Solid(c) => c.a,
            _ => 1.0,
        };
        base * self.opacity
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrokeAlign {
    #[default]
    Inside,
    Center,
    Outside,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrokeCap {
    #[default]
    None,
    Round,
    Square,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrokeJoin {
    #[default]
    Miter,
    Bevel,
    Round,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub paints: Vec<Paint>,
    pub weight: f32,
    /// Per-side weights (top, right, bottom, left) for rectangles/frames.
    pub side_weights: Option<[f32; 4]>,
    pub align: StrokeAlign,
    pub cap: StrokeCap,
    pub join: StrokeJoin,
    pub miter_limit: f32,
    pub dashes: Vec<f32>,
}

impl Stroke {
    /// Inward border widths `[top, right, bottom, left]` that sit inside the node's box.
    /// Returns `[0.0; 4]` when the stroke has no visible paint or is aligned `Outside`.
    pub fn layout_inset(&self) -> [f32; 4] {
        if !self.paints.iter().any(|p| p.visible && p.opacity > 0.0) {
            return [0.0; 4];
        }
        let factor = match self.align {
            StrokeAlign::Inside => 1.0,
            StrokeAlign::Center => 0.5,
            StrokeAlign::Outside => return [0.0; 4],
        };
        let sides = self.side_weights.unwrap_or([self.weight; 4]);
        sides.map(|w| (w * factor).max(0.0))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Effect {
    DropShadow {
        color: Color,
        offset: Vec2,
        radius: f32,
        spread: f32,
        visible: bool,
        show_behind: bool,
    },
    InnerShadow {
        color: Color,
        offset: Vec2,
        radius: f32,
        spread: f32,
        visible: bool,
    },
    LayerBlur {
        radius: f32,
        visible: bool,
    },
    BackgroundBlur {
        radius: f32,
        visible: bool,
    },
    /// Grain over the layer's shape.
    Noise(NoiseEffect),
    /// Displaces the layer's edges with a rough, paper-like texture.
    Texture(TextureEffect),
    /// Frosted, refractive glass over what is behind the layer.
    Glass(GlassEffect),
}

impl Effect {
    pub fn visible(&self) -> bool {
        match self {
            Effect::DropShadow { visible, .. }
            | Effect::InnerShadow { visible, .. }
            | Effect::LayerBlur { visible, .. }
            | Effect::BackgroundBlur { visible, .. } => *visible,
            Effect::Noise(e) => e.visible,
            Effect::Texture(e) => e.visible,
            Effect::Glass(e) => e.visible,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NoiseMode {
    /// One color at `opacity`.
    Mono { color: Color },
    /// Two colors mixed per grain.
    Duo { color: Color, secondary: Color },
    /// Random hue per grain.
    Multi,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NoiseEffect {
    pub mode: NoiseMode,
    /// Grain size in px.
    pub size: f32,
    /// Share of covered pixels, 0..1.
    pub density: f32,
    pub opacity: f32,
    pub blend: BlendMode,
    /// Deterministic seed so renders are reproducible.
    pub seed: u32,
    pub visible: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextureEffect {
    /// Texture scale in px.
    pub size: f32,
    /// Edge displacement in px.
    pub radius: f32,
    /// Keep the textured result inside the original shape.
    pub clip_to_shape: bool,
    pub seed: u32,
    pub visible: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GlassEffect {
    /// Light direction in degrees (0 = from the right, clockwise).
    pub light_angle: f32,
    /// Highlight strength, 0..1.
    pub light_intensity: f32,
    /// How strongly the backdrop bends at the edges, 0..1.
    pub refraction: f32,
    /// Bevel depth in px.
    pub depth: f32,
    /// Chromatic split at the edges, 0..1.
    pub dispersion: f32,
    /// Backdrop blur radius in px.
    pub frost: f32,
    pub visible: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlignH {
    #[default]
    Left,
    Center,
    Right,
    Justified,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlignV {
    #[default]
    Top,
    Center,
    Bottom,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAutoResize {
    #[default]
    None,
    WidthAndHeight,
    Height,
    Truncate,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextCase {
    #[default]
    Original,
    Upper,
    Lower,
    Title,
    SmallCaps,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextDecoration {
    #[default]
    None,
    Underline,
    Strikethrough,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum LineHeight {
    /// The font's natural line height (ascent + descent + line gap).
    Auto,
    Pixels(f32),
    /// Percent of the font size: `Percent(120.0)` = 1.2 × font_size.
    Percent(f32),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextStyle {
    pub font_family: String,
    /// CSS weight 100..=900.
    pub font_weight: u16,
    pub italic: bool,
    pub font_size: f32,
    pub line_height: LineHeight,
    /// Letter spacing in px.
    pub letter_spacing: f32,
    pub paragraph_spacing: f32,
    pub align_h: TextAlignH,
    pub align_v: TextAlignV,
    pub case: TextCase,
    pub decoration: TextDecoration,
    /// Text colour lives here; `Node::fills` is unused for text nodes.
    pub fills: Vec<Paint>,
    /// OpenType feature settings, e.g. `("tnum", 1)`, `("liga", 0)`, `("ss01", 1)`.
    ///
    /// They reach the shaper (cosmic-text / harfrust) per style run. Limitation: cosmic-text shapes
    /// a word as one run with the features of its first character, so a feature set on part of a
    /// word applies to all of it (or to none of it when the word starts without it); on whole
    /// words it is exact.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<(String, u32)>,
    /// Variable font axis settings, e.g. `("wght", 550.0)`, `("wdth", 85.0)`, `("slnt", -6.0)`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variation_axes: Vec<(String, f32)>,
}

impl TextStyle {
    pub fn get_axis(&self, tag: &str) -> Option<f32> {
        self.variation_axes.iter().find(|(k, _)| k.eq_ignore_ascii_case(tag)).map(|(_, v)| *v)
    }

    pub fn set_axis(&mut self, tag: &str, value: f32) {
        if let Some((_, v)) = self.variation_axes.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(tag)) {
            *v = value;
        } else {
            self.variation_axes.push((tag.to_ascii_lowercase(), value));
        }
    }

    pub fn remove_axis(&mut self, tag: &str) {
        self.variation_axes.retain(|(k, _)| !k.eq_ignore_ascii_case(tag));
    }

    pub fn is_feature_enabled(&self, tag: &str) -> bool {
        self.features.iter().find(|(k, _)| k.eq_ignore_ascii_case(tag)).is_some_and(|(_, v)| *v != 0)
    }

    pub fn set_feature(&mut self, tag: &str, enabled: bool) {
        let norm = tag.to_ascii_lowercase();
        if enabled {
            if let Some((_, v)) = self.features.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(&norm)) {
                *v = 1;
            } else {
                self.features.push((norm, 1));
            }
        } else if matches!(norm.as_str(), "liga" | "calt" | "kern" | "clig") {
            if let Some((_, v)) = self.features.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(&norm)) {
                *v = 0;
            } else {
                self.features.push((norm, 0));
            }
        } else {
            self.features.retain(|(k, _)| !k.eq_ignore_ascii_case(&norm));
        }
    }
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_family: "Inter".into(),
            font_weight: 400,
            italic: false,
            font_size: 12.0,
            line_height: LineHeight::Auto,
            letter_spacing: 0.0,
            paragraph_spacing: 0.0,
            align_h: TextAlignH::Left,
            align_v: TextAlignV::Top,
            case: TextCase::Original,
            decoration: TextDecoration::None,
            fills: vec![Paint::solid(Color::BLACK)],
            features: Vec::new(),
            variation_axes: Vec::new(),
        }
    }
}

/// A run of characters (byte range into the text) with style overrides.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextRun {
    pub start: usize,
    pub end: usize,
    pub style: TextStyle,
}

fn default_true() -> bool {
    true
}

fn is_true(v: &bool) -> bool {
    *v
}

fn is_default_grid_align(a: &LayoutGridAlign) -> bool {
    *a == LayoutGridAlign::Stretch
}

fn is_zero_f32(v: &f32) -> bool {
    *v == 0.0
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutGridAlign {
    #[default]
    Stretch,
    Min,
    Center,
    Max,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum LayoutGrid {
    Grid {
        size: f32,
        color: Color,
        #[serde(default = "default_true", skip_serializing_if = "is_true")]
        visible: bool,
    },
    Columns {
        count: u32,
        gutter: f32,
        offset: f32,
        #[serde(default, skip_serializing_if = "is_zero_f32")]
        section_size: f32,
        #[serde(default, skip_serializing_if = "is_default_grid_align")]
        alignment: LayoutGridAlign,
        color: Color,
        #[serde(default = "default_true", skip_serializing_if = "is_true")]
        visible: bool,
    },
    Rows {
        count: u32,
        gutter: f32,
        offset: f32,
        #[serde(default, skip_serializing_if = "is_zero_f32")]
        section_size: f32,
        #[serde(default, skip_serializing_if = "is_default_grid_align")]
        alignment: LayoutGridAlign,
        color: Color,
        #[serde(default = "default_true", skip_serializing_if = "is_true")]
        visible: bool,
    },
}

impl LayoutGrid {
    pub fn grid(size: f32, color: Color) -> Self {
        Self::Grid { size, color, visible: true }
    }

    pub fn columns(count: u32, gutter: f32, offset: f32, color: Color) -> Self {
        Self::Columns { count, gutter, offset, section_size: 0.0, alignment: LayoutGridAlign::Stretch, color, visible: true }
    }

    pub fn rows(count: u32, gutter: f32, offset: f32, color: Color) -> Self {
        Self::Rows { count, gutter, offset, section_size: 0.0, alignment: LayoutGridAlign::Stretch, color, visible: true }
    }

    pub fn is_visible(&self) -> bool {
        match self {
            Self::Grid { visible, .. } | Self::Columns { visible, .. } | Self::Rows { visible, .. } => *visible,
        }
    }

    pub fn set_visible(&mut self, v: bool) {
        match self {
            Self::Grid { visible, .. } | Self::Columns { visible, .. } | Self::Rows { visible, .. } => *visible = v,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_hex_smart_expansion_and_named_colors() {
        assert_eq!(Color::from_hex(""), None);
        assert_eq!(Color::from_hex("   "), None);
        assert_eq!(Color::from_hex("transparent"), Some(Color::TRANSPARENT));
        assert_eq!(Color::from_hex("  TRANSPARENT "), Some(Color::TRANSPARENT));
        assert_eq!(Color::from_hex("#F"), Some(Color::rgb(0xFF, 0xFF, 0xFF)));
        assert_eq!(Color::from_hex("F"), Some(Color::rgb(0xFF, 0xFF, 0xFF)));
        assert_eq!(Color::from_hex("#F0"), Some(Color::rgb(0xF0, 0xF0, 0xF0)));
        assert_eq!(Color::from_hex("80"), Some(Color::rgb(0x80, 0x80, 0x80)));
        assert_eq!(Color::from_hex("#123"), Some(Color::rgb(0x11, 0x22, 0x33)));
        assert_eq!(Color::from_hex("#1234"), Some(Color::rgba(0x11, 0x22, 0x33, 0x44)));
        assert_eq!(Color::from_hex("#12345"), Some(Color::rgba(0x11, 0x22, 0x33, 0x45)));
        assert_eq!(Color::from_hex("#123456"), Some(Color::rgb(0x12, 0x34, 0x56)));
        assert_eq!(Color::from_hex("#1234567"), Some(Color::rgba(0x12, 0x34, 0x56, 0x77)));
        assert_eq!(Color::from_hex("#12345678"), Some(Color::rgba(0x12, 0x34, 0x56, 0x78)));
        assert_eq!(Color::from_hex("red"), Some(Color::rgb(255, 0, 0)));
        assert_eq!(Color::from_hex("blue"), Some(Color::rgb(0, 0, 255)));
        assert_eq!(Color::from_hex("coral"), Some(Color::rgb(255, 127, 80)));
        assert_eq!(Color::from_hex("rebeccapurple"), Some(Color::rgb(102, 51, 153)));
    }
}
