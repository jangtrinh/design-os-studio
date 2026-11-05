use crate::{LeadingTrim, Vec2};
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

impl BlendMode {
    pub fn is_normal(&self) -> bool {
        *self == BlendMode::Normal
    }
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

fn is_zero_f32(v: &f32) -> bool {
    *v == 0.0
}

fn is_one_f32(v: &f32) -> bool {
    (*v - 1.0).abs() < f32::EPSILON
}

fn default_tile_scale() -> f32 {
    1.0
}

/// Per-paint image colour adjustments matching `PaintFilterMessage` (fields 1..10).
/// Each adjustment ranges over `[-1.0, 1.0]` with `0.0` as neutral.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ImageAdjust {
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub tint: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub shadows: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub highlights: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub detail: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub exposure: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub vignette: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub temperature: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub vibrance: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub contrast: f32,
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub brightness: f32,
}

impl ImageAdjust {
    pub fn is_default(&self) -> bool {
        self.tint == 0.0
            && self.shadows == 0.0
            && self.highlights == 0.0
            && self.detail == 0.0
            && self.exposure == 0.0
            && self.vignette == 0.0
            && self.temperature == 0.0
            && self.vibrance == 0.0
            && self.contrast == 0.0
            && self.brightness == 0.0
    }
}

/// Crop transform, rotation, tile scale, colour adjustments, and round-trip metadata for [`PaintKind::Image`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageParams {
    /// Crop affine `[m00, m01, m02, m10, m11, m12]` in normalised UV space (`0..1` of node -> `0..1` of image).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<[f32; 6]>,
    /// Clockwise rotation in degrees (`0`, `90`, `180`, `270`, mod `360`).
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub rotation: f32,
    /// Tile scale factor (`1.0` = 100% native pixel dimensions).
    #[serde(default = "default_tile_scale", skip_serializing_if = "is_one_f32")]
    pub tile_scale: f32,
    /// Colour adjustments.
    #[serde(default, skip_serializing_if = "ImageAdjust::is_default")]
    pub adjust: ImageAdjust,
    /// Round-tripped `.fig` `imageShouldColorManage` flag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_manage: Option<bool>,
    /// Round-tripped `.fig` `(originalImageWidth, originalImageHeight)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_size: Option<(u32, u32)>,
}

impl Default for ImageParams {
    fn default() -> Self {
        Self { transform: None, rotation: 0.0, tile_scale: 1.0, adjust: ImageAdjust::default(), color_manage: None, original_size: None }
    }
}

impl ImageParams {
    pub fn is_default(&self) -> bool {
        self.transform.is_none()
            && self.rotation == 0.0
            && is_one_f32(&self.tile_scale)
            && self.adjust.is_default()
            && self.color_manage.is_none()
            && self.original_size.is_none()
    }
}

/// Smallest tile scale an image fill renders at.
pub const TILE_SCALE_MIN: f32 = 0.01;
/// Largest tile scale an image fill renders at.
pub const TILE_SCALE_MAX: f32 = 100.0;

/// Clamps a tile scale into `TILE_SCALE_MIN..=TILE_SCALE_MAX`. Zero and negative
/// values land on the minimum; a non-finite value falls back to `1.0` (native size).
pub fn clamp_tile_scale(raw: f32) -> f32 {
    if raw.is_finite() {
        raw.clamp(TILE_SCALE_MIN, TILE_SCALE_MAX)
    } else {
        1.0
    }
}

/// Clockwise quarter turns (`0..4`) an image fill rotation renders at: the nearest
/// multiple of 90 degrees, modulo 360. A non-finite rotation is `0`.
pub fn image_quarter_turns(rotation: f32) -> u8 {
    const ROTATION_STEP_DEG: f32 = 90.0;
    if rotation.is_finite() {
        ((rotation / ROTATION_STEP_DEG).round() as i32).rem_euclid(4) as u8
    } else {
        0
    }
}

/// Maps a node-local point `(x, y)` to image UV `(u, v)` (`0..1` over the decoded
/// picture, before any quarter turn): `u = u.0 * x + u.1 * y + u.2` and likewise
/// for `v`. The CPU and GPU renderers and the image jobs all read this one mapping.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageUvMap {
    pub u: (f32, f32, f32),
    pub v: (f32, f32, f32),
    /// Node units per image pixel, used to pick a mip level.
    pub scale: f32,
    /// Clockwise quarter turns baked into `u` and `v`. Always `0` for Crop.
    pub quarter_turns: u8,
}

impl ImageUvMap {
    /// Image UV of the node-local point `(x, y)`.
    pub fn apply(&self, x: f32, y: f32) -> (f32, f32) {
        (self.u.0 * x + self.u.1 * y + self.u.2, self.v.0 * x + self.v.1 * y + self.v.2)
    }
}

/// Rotates an unrotated `(x, y) -> (u, v)` affine clockwise by `q` quarter turns around `(0.5, 0.5)`.
fn rotate_uv_quarter_turns(q: u8, u: (f32, f32, f32), v: (f32, f32, f32)) -> ((f32, f32, f32), (f32, f32, f32)) {
    match q {
        1 => (v, (-u.0, -u.1, 1.0 - u.2)),
        2 => ((-u.0, -u.1, 1.0 - u.2), (-v.0, -v.1, 1.0 - v.2)),
        3 => ((-v.0, -v.1, 1.0 - v.2), u),
        _ => (u, v),
    }
}

/// Node-local to image-UV mapping of an image fill on a node of `size` showing an
/// `iw` x `ih` picture. Fill, Fit and Tile rotate by `params.rotation` (odd quarter
/// turns swap the picture's width and height before scaling); Tile scales by the
/// clamped `tile_scale`; Crop maps node UV through `params.transform` (identity when
/// absent) and ignores rotation. `None` when nothing is drawn: a degenerate size or
/// picture, or a singular or non-finite Crop transform.
pub fn image_uv_map(size: Vec2, iw: f32, ih: f32, scale_mode: ImageScaleMode, params: &ImageParams) -> Option<ImageUvMap> {
    if !iw.is_finite() || !ih.is_finite() || iw <= 0.0 || ih <= 0.0 || !size.x.is_finite() || !size.y.is_finite() {
        return None;
    }
    let q = image_quarter_turns(params.rotation);
    let (rw, rh) = if q % 2 != 0 { (ih, iw) } else { (iw, ih) };
    match scale_mode {
        ImageScaleMode::Fill | ImageScaleMode::Fit => {
            let s = if matches!(scale_mode, ImageScaleMode::Fill) { (size.x / rw).max(size.y / rh) } else { (size.x / rw).min(size.y / rh) };
            if !s.is_finite() || s <= 0.0 {
                return None;
            }
            let tx = (size.x - rw * s) * 0.5;
            let ty = (size.y - rh * s) * 0.5;
            let u0 = (1.0 / (rw * s), 0.0, -tx / (rw * s));
            let v0 = (0.0, 1.0 / (rh * s), -ty / (rh * s));
            let (u, v) = rotate_uv_quarter_turns(q, u0, v0);
            Some(ImageUvMap { u, v, scale: s, quarter_turns: q })
        }
        ImageScaleMode::Tile => {
            let ts_factor = clamp_tile_scale(params.tile_scale);
            let u0 = (1.0 / (rw * ts_factor), 0.0, 0.0);
            let v0 = (0.0, 1.0 / (rh * ts_factor), 0.0);
            let (u, v) = rotate_uv_quarter_turns(q, u0, v0);
            Some(ImageUvMap { u, v, scale: ts_factor, quarter_turns: q })
        }
        ImageScaleMode::Crop => {
            if size.x <= 0.0 || size.y <= 0.0 {
                return None;
            }
            let m = params.transform.unwrap_or([1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
            if !m.iter().all(|v| v.is_finite()) {
                return None;
            }
            let det = m[0] * m[4] - m[1] * m[3];
            if !det.is_finite() || det.abs() <= 1e-12 {
                return None;
            }
            let u = (m[0] / size.x, m[1] / size.y, m[2]);
            let v = (m[3] / size.x, m[4] / size.y, m[5]);
            let sx = (u.0 * iw).hypot(v.0 * ih).max(1e-6);
            let sy = (u.1 * iw).hypot(v.1 * ih).max(1e-6);
            let s = (1.0 / sx).max(1.0 / sy);
            if !s.is_finite() || s <= 0.0 {
                return None;
            }
            Some(ImageUvMap { u, v, scale: s, quarter_turns: 0 })
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShaderRef {
    pub raw: serde_json::Value,
}

impl Default for ShaderRef {
    fn default() -> Self {
        Self { raw: serde_json::Value::Object(serde_json::Map::new()) }
    }
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
        #[serde(default, skip_serializing_if = "ImageParams::is_default")]
        params: ImageParams,
    },
    Shader(ShaderRef),
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
    /// background blur, glass, noise): a solid colour contributes its own alpha, a custom
    /// shader contributes nothing (it is not rendered), every other kind is opaque, and the
    /// paint opacity scales either. A colour variable keeps its alpha on the colour, so reading
    /// only `opacity` would ink the silhouette fully.
    pub fn silhouette_alpha(&self) -> f32 {
        let base = match &self.kind {
            PaintKind::Solid(c) => c.a,
            PaintKind::Shader(_) => 0.0,
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
    ArrowLines,
    ArrowEquilateral,
    TriangleFilled,
    DiamondFilled,
    CircleFilled,
}

impl StrokeCap {
    /// True when this cap is one of the 5 arrowhead / endpoint marker caps.
    pub fn is_arrow(self) -> bool {
        matches!(self, Self::ArrowLines | Self::ArrowEquilateral | Self::TriangleFilled | Self::DiamondFilled | Self::CircleFilled)
    }
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
        /// How the shadow composites over what is behind it. Files saved
        /// before this field existed load as `Normal`.
        #[serde(default, skip_serializing_if = "BlendMode::is_normal")]
        blend: BlendMode,
    },
    InnerShadow {
        color: Color,
        offset: Vec2,
        radius: f32,
        spread: f32,
        visible: bool,
        #[serde(default, skip_serializing_if = "BlendMode::is_normal")]
        blend: BlendMode,
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
    Shader {
        #[serde(flatten)]
        shader: ShaderRef,
        #[serde(default = "default_true")]
        visible: bool,
    },
}

impl Effect {
    pub fn visible(&self) -> bool {
        match self {
            Effect::DropShadow { visible, .. }
            | Effect::InnerShadow { visible, .. }
            | Effect::LayerBlur { visible, .. }
            | Effect::BackgroundBlur { visible, .. }
            | Effect::Shader { visible, .. } => *visible,
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
    /// Grain size in px along X. Also the size along Y while `size_y` is `None`.
    pub size: f32,
    /// Grain size in px along Y when it differs from `size`; `None` means the same as `size`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_y: Option<f32>,
    /// Share of covered pixels, 0..1.
    pub density: f32,
    /// Strength of the grain, 0..1. Every mode draws its grain with it.
    pub opacity: f32,
    pub blend: BlendMode,
    /// Deterministic seed so renders are reproducible.
    pub seed: u32,
    pub visible: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextureEffect {
    /// Texture scale in px along X. Also the scale along Y while `size_y` is `None`.
    pub size: f32,
    /// Texture scale in px along Y when it differs from `size`; `None` means the same as `size`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_y: Option<f32>,
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
    /// Refraction radius in px: how far from the edge the backdrop bends.
    pub depth: f32,
    /// Chromatic split at the edges, 0..1.
    pub dispersion: f32,
    /// Backdrop blur radius in px.
    pub frost: f32,
    /// How far the bending direction turns from the edge normal toward the shape centre, 0..1.
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub splay: f32,
    /// Width in px of the bevel band along the edge.
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub bevel: f32,
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
    /// First-line indent in px. Shared text styles carry this value; text
    /// layers keep the canonical copy in [`TextProps::paragraph`].
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub paragraph_indent: f32,
    /// Extra spacing between consecutive list items in px. Shared text styles
    /// carry this value; text layers keep the canonical copy in
    /// [`TextProps::paragraph`].
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub list_spacing: f32,
    /// Figma's vertical trim setting for a shared text style.
    #[serde(default, skip_serializing_if = "is_default_leading_trim")]
    pub leading_trim: LeadingTrim,
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
    /// Set the CSS weight and keep an existing variable-font weight axis in sync.
    /// Styles without a `wght` axis keep their current axis list.
    pub fn set_font_weight(&mut self, weight: u16) {
        self.font_weight = weight;
        if self.get_axis("wght").is_some() {
            self.set_axis("wght", f32::from(weight));
        }
    }

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
            paragraph_indent: 0.0,
            list_spacing: 0.0,
            leading_trim: LeadingTrim::None,
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

fn is_default_leading_trim(v: &LeadingTrim) -> bool {
    *v == LeadingTrim::default()
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

    #[test]
    fn old_designos_file_loads_with_default_params_and_saves_unchanged() {
        let json = r#"{"Image":{"image_ref":"hash123","scale_mode":"Fill"}}"#;
        let kind: PaintKind = serde_json::from_str(json).unwrap();
        assert_eq!(kind, PaintKind::Image { image_ref: "hash123".into(), scale_mode: ImageScaleMode::Fill, params: ImageParams::default() });
        let saved = serde_json::to_string(&kind).unwrap();
        assert_eq!(saved, json);
    }
    #[test]
    fn tile_scale_clamps_into_range_and_non_finite_falls_back_to_native() {
        assert_eq!(clamp_tile_scale(0.0), TILE_SCALE_MIN);
        assert_eq!(clamp_tile_scale(-4.0), TILE_SCALE_MIN);
        assert_eq!(clamp_tile_scale(0.001), TILE_SCALE_MIN);
        assert_eq!(clamp_tile_scale(2.5), 2.5);
        assert_eq!(clamp_tile_scale(1.0e6), TILE_SCALE_MAX);
        for raw in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(clamp_tile_scale(raw), 1.0, "{raw}");
        }
    }

    #[test]
    fn quarter_turns_round_to_the_nearest_right_angle_modulo_a_full_turn() {
        for (degrees, turns) in [(0.0, 0), (89.0, 1), (90.0, 1), (180.0, 2), (270.0, 3), (360.0, 0), (450.0, 1), (-90.0, 3), (44.0, 0), (f32::NAN, 0)] {
            assert_eq!(image_quarter_turns(degrees), turns, "{degrees}");
        }
    }

    #[test]
    fn image_uv_map_places_the_picture_like_each_scale_mode() {
        let params = ImageParams::default();
        // Fill: a 400x200 picture covers a 100x100 node, centred: the node shows u in 0.25..0.75.
        let fill = image_uv_map(Vec2::new(100.0, 100.0), 400.0, 200.0, ImageScaleMode::Fill, &params).unwrap();
        assert_eq!((fill.apply(0.0, 0.0), fill.apply(100.0, 100.0)), ((0.25, 0.0), (0.75, 1.0)));
        // Fit: the whole picture sits in the middle of the node.
        let fit = image_uv_map(Vec2::new(100.0, 100.0), 400.0, 200.0, ImageScaleMode::Fit, &params).unwrap();
        assert_eq!((fit.apply(0.0, 25.0), fit.apply(100.0, 75.0)), ((0.0, 0.0), (1.0, 1.0)));
        // A quarter turn clockwise: the top-left of the node shows the bottom-left of the picture.
        let turned = image_uv_map(Vec2::new(100.0, 100.0), 400.0, 200.0, ImageScaleMode::Fit, &ImageParams { rotation: 90.0, ..params }).unwrap();
        assert_eq!(turned.quarter_turns, 1);
        let (u, v) = turned.apply(25.0, 0.0);
        assert!((u - 0.0).abs() < 1e-6 && (v - 1.0).abs() < 1e-6, "({u}, {v})");
        // Crop ignores rotation and maps node UV through the transform.
        let crop = ImageParams { transform: Some([0.5, 0.0, 0.25, 0.0, 0.5, 0.25]), rotation: 90.0, ..params };
        let crop = image_uv_map(Vec2::new(100.0, 100.0), 400.0, 200.0, ImageScaleMode::Crop, &crop).unwrap();
        assert_eq!((crop.quarter_turns, crop.apply(0.0, 0.0), crop.apply(100.0, 100.0)), (0, (0.25, 0.25), (0.75, 0.75)));
    }

    #[test]
    fn image_uv_map_draws_nothing_for_degenerate_input() {
        let size = Vec2::new(100.0, 100.0);
        let singular = ImageParams { transform: Some([1.0, 2.0, 0.0, 2.0, 4.0, 0.0]), ..Default::default() };
        assert_eq!(image_uv_map(size, 10.0, 10.0, ImageScaleMode::Crop, &singular), None);
        assert_eq!(image_uv_map(size, 0.0, 10.0, ImageScaleMode::Fill, &ImageParams::default()), None);
        assert_eq!(image_uv_map(Vec2::new(f32::NAN, 10.0), 10.0, 10.0, ImageScaleMode::Fit, &ImageParams::default()), None);
        let tile = image_uv_map(size, 10.0, 10.0, ImageScaleMode::Tile, &ImageParams { tile_scale: 0.0, ..Default::default() }).unwrap();
        assert_eq!(tile.scale, TILE_SCALE_MIN, "tile scale 0 is drawn at the minimum");
    }

    #[test]
    fn old_designos_file_without_shaders_loads_and_saves_unchanged() {
        let mut doc = crate::Document::new("Test");
        let page = doc.add_page("Page 1");
        let frame_id = doc.alloc_id();
        doc.insert(page, crate::Node::frame(frame_id, crate::Rect::new(0.0, 0.0, 120.0, 80.0))).unwrap();
        let json = doc.to_json().unwrap();
        let loaded = crate::Document::from_json(&json).unwrap();
        assert_eq!(loaded.to_json().unwrap(), json);
    }

    #[test]
    fn designos_json_round_trip_keeps_shader() {
        let mut doc = crate::Document::new("Test");
        let page = doc.add_page("Page 1");
        let frame_id = doc.alloc_id();
        let mut frame = crate::Node::frame(frame_id, crate::Rect::new(0.0, 0.0, 120.0, 80.0));
        let raw = serde_json::json!({
            "customEffectId": {
                "guid": { "sessionID": 7, "localID": 19 },
                "assetRef": { "key": "shader-key", "version": "1" }
            },
            "componentPropAssignments": [
                {
                    "defID": { "sessionID": 1, "localID": 2 },
                    "value": { "floatValue": 0.5 }
                }
            ]
        });
        frame.fills.push(Paint { kind: PaintKind::Shader(ShaderRef { raw: raw.clone() }), opacity: 0.75, visible: true, blend: BlendMode::Normal });
        frame.effects.push(Effect::Shader { shader: ShaderRef { raw: raw.clone() }, visible: false });
        doc.insert(page, frame).unwrap();

        let json = doc.to_json().unwrap();
        assert!(json.contains(r#""Shader":{"raw":"#), "{json}");
        let loaded = crate::Document::from_json(&json).unwrap();
        let loaded_frame = loaded.get(frame_id).unwrap();
        assert_eq!(loaded_frame.fills.len(), 2);
        assert_eq!(loaded_frame.fills[1].kind, PaintKind::Shader(ShaderRef { raw: raw.clone() }));
        assert_eq!(loaded_frame.effects, vec![Effect::Shader { shader: ShaderRef { raw }, visible: false }]);
    }
}
