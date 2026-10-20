//! UI icons: the official Phosphor Icons set (`@phosphor-icons/core`, MIT license,
//! vendored in `assets/icons/phosphor`), rasterised once per (icon, pixel size) with
//! `resvg` into an alpha mask that is uploaded as an egui texture and tinted at paint
//! time. Rendering happens at the device pixel ratio, so glyphs are crisp at 1x and 2x.
//! Only the brand logo is drawn from our brand asset.

use egui::{Color32, Context, Painter, Pos2, Rect, TextureHandle, TextureOptions, Vec2};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    Logo,
    File,
    Specs,
    Assets,
    Tools,
    Variables,
    Help,
    ChevronUp,
    ChevronDown,
    ChevronRight,
    Plus,
    Minus,
    Search,
    Eye,
    EyeOff,
    Lock,
    Unlock,
    Sidebar,
    Page,
    Check,
    // tools
    Move,
    Hand,
    Scale,
    Frame,
    AutoLayoutH,
    AutoLayoutV,
    Section,
    Slice,
    Rectangle,
    Line,
    Arrow,
    Ellipse,
    Polygon,
    Star,
    Image,
    Pen,
    Pencil,
    Text,
    Comment,
    // board tools and layer kinds
    Sticky,
    Shapes,
    Connector,
    Marker,
    Highlighter,
    Stamp,
    // layer kinds
    Group,
    Vector,
    Component,
    ComponentSet,
    Instance,
    Boolean,
    // property icons
    AlignLeft,
    AlignHCenter,
    AlignRight,
    AlignTop,
    AlignVCenter,
    AlignBottom,
    Rotation,
    Radius,
    Opacity,
    Mask,
    DirHorizontal,
    DirVertical,
    Wrap,
    Spacing,
    PaddingH,
    PaddingV,
    Dev,
    Design,
    Export,
    Sparkle,
    TextLeft,
    TextCenter,
    TextRight,
    Distribute,
    Rotate90,
    FlipH,
    FlipV,
    Flag,
    Play,
    // projects library
    Folder,
    Trash,
    Clock,
    GridView,
    ListView,
    /// Libraries (a closed book).
    Book,
    // color picker
    Eyedropper,
    Close,
    Styles,
    PaintSolid,
    PaintLinear,
    PaintRadial,
    PaintAngular,
    PaintDiamond,
    PaintImage,
    Blend,
    NoPaint,
    // effects and stroke
    InnerShadow,
    DropShadow,
    LayerBlur,
    BackgroundBlur,
    Noise,
    Texture,
    Glass,
    Shader,
    StrokeSettings,
    StrokeSides,
    StrokeWeight,
    Terminal,
    Grid2x2,
    // navigation & agent chrome
    Home,
    Wand,
    StarFour,
    BookOpen,
    Microphone,
    ArrowUp,
    ArrowSquareOut,
    Sliders,
}

impl Icon {
    /// Every variant, for tests and tooling.
    #[cfg(test)]
    pub const ALL: &'static [Icon] = &[
        Icon::Logo,
        Icon::File,
        Icon::Specs,
        Icon::Assets,
        Icon::Tools,
        Icon::Variables,
        Icon::Help,
        Icon::Folder,
        Icon::Trash,
        Icon::Clock,
        Icon::GridView,
        Icon::ListView,
        Icon::Book,
        Icon::ChevronUp,
        Icon::ChevronDown,
        Icon::ChevronRight,
        Icon::Plus,
        Icon::Minus,
        Icon::Search,
        Icon::Eye,
        Icon::EyeOff,
        Icon::Lock,
        Icon::Unlock,
        Icon::Sidebar,
        Icon::Page,
        Icon::Check,
        Icon::Move,
        Icon::Hand,
        Icon::Scale,
        Icon::Frame,
        Icon::AutoLayoutH,
        Icon::AutoLayoutV,
        Icon::Section,
        Icon::Slice,
        Icon::Rectangle,
        Icon::Line,
        Icon::Arrow,
        Icon::Ellipse,
        Icon::Polygon,
        Icon::Star,
        Icon::Image,
        Icon::Pen,
        Icon::Pencil,
        Icon::Text,
        Icon::Comment,
        Icon::Sticky,
        Icon::Shapes,
        Icon::Connector,
        Icon::Marker,
        Icon::Highlighter,
        Icon::Stamp,
        Icon::Group,
        Icon::Vector,
        Icon::Component,
        Icon::ComponentSet,
        Icon::Instance,
        Icon::Boolean,
        Icon::AlignLeft,
        Icon::AlignHCenter,
        Icon::AlignRight,
        Icon::AlignTop,
        Icon::AlignVCenter,
        Icon::AlignBottom,
        Icon::Rotation,
        Icon::Radius,
        Icon::Opacity,
        Icon::Mask,
        Icon::DirHorizontal,
        Icon::DirVertical,
        Icon::Wrap,
        Icon::Spacing,
        Icon::PaddingH,
        Icon::PaddingV,
        Icon::Dev,
        Icon::Design,
        Icon::Export,
        Icon::Sparkle,
        Icon::TextLeft,
        Icon::TextCenter,
        Icon::TextRight,
        Icon::Distribute,
        Icon::Rotate90,
        Icon::FlipH,
        Icon::FlipV,
        Icon::Flag,
        Icon::Play,
        Icon::Eyedropper,
        Icon::Close,
        Icon::Styles,
        Icon::PaintSolid,
        Icon::PaintLinear,
        Icon::PaintRadial,
        Icon::PaintAngular,
        Icon::PaintDiamond,
        Icon::PaintImage,
        Icon::Blend,
        Icon::NoPaint,
        Icon::InnerShadow,
        Icon::DropShadow,
        Icon::LayerBlur,
        Icon::BackgroundBlur,
        Icon::Noise,
        Icon::Texture,
        Icon::Glass,
        Icon::Shader,
        Icon::StrokeSettings,
        Icon::StrokeSides,
        Icon::StrokeWeight,
        Icon::Terminal,
        Icon::Grid2x2,
        Icon::Home,
        Icon::Wand,
        Icon::StarFour,
        Icon::BookOpen,
        Icon::Microphone,
        Icon::ArrowUp,
        Icon::ArrowSquareOut,
        Icon::Sliders,
    ];

    /// Name of the vendored Phosphor regular icon that stands in for this one (`None` for the logo).
    pub fn phosphor_name(self) -> Option<&'static str> {
        Some(match self {
            Icon::Logo => return None,
            Icon::File => "file",
            Icon::Specs => "book-open-text",
            Icon::Folder => "folder",
            Icon::Trash => "trash",
            Icon::Clock => "clock",
            Icon::GridView => "squares-four",
            Icon::ListView => "list",
            Icon::Book => "book",
            Icon::Assets => "plus-circle",
            Icon::Tools => "toolbox",
            Icon::Variables => "hexagon",
            Icon::Help => "question",
            Icon::ChevronUp => "caret-up",
            Icon::ChevronDown => "caret-down",
            Icon::ChevronRight => "caret-right",
            Icon::Plus => "plus",
            Icon::Minus => "minus",
            Icon::Search => "magnifying-glass",
            Icon::Eye => "eye",
            Icon::EyeOff => "eye-slash",
            Icon::Lock => "lock",
            Icon::Unlock => "lock-open",
            Icon::Sidebar => "sidebar-simple",
            Icon::Page => "file-text",
            Icon::Check => "check",
            Icon::Move => "cursor",
            Icon::Hand => "hand",
            Icon::Scale => "arrows-out",
            Icon::Frame => "frame-corners",
            Icon::AutoLayoutH => "columns",
            Icon::AutoLayoutV => "rows",
            Icon::Section => "selection-all",
            Icon::Slice => "knife",
            Icon::Rectangle => "square",
            Icon::Line => "line-segment",
            Icon::Arrow => "arrow-up-right",
            Icon::Ellipse => "circle",
            Icon::Polygon => "triangle",
            Icon::Star => "star",
            Icon::Image => "image",
            Icon::Pen => "pen-nib",
            Icon::Pencil => "pencil-simple",
            Icon::Text => "text-t",
            Icon::Comment => "chat-circle",
            Icon::Sticky => "note",
            Icon::Shapes => "shapes",
            Icon::Connector => "arrow-elbow-down-right",
            Icon::Marker => "pencil-line",
            Icon::Highlighter => "highlighter",
            Icon::Stamp => "stamp",
            Icon::Group => "selection-plus",
            Icon::Vector => "bezier-curve",
            Icon::Component => "diamonds-four",
            Icon::ComponentSet => "intersect-three",
            Icon::Instance => "diamond",
            Icon::Boolean => "unite",
            Icon::AlignLeft => "align-left",
            Icon::AlignHCenter => "align-center-horizontal",
            Icon::AlignRight => "align-right",
            Icon::AlignTop => "align-top",
            Icon::AlignVCenter => "align-center-vertical",
            Icon::AlignBottom => "align-bottom",
            Icon::Rotation => "arrow-clockwise",
            Icon::Radius => "corners-out",
            Icon::Opacity => "drop",
            Icon::Mask => "circle-half",
            Icon::DirHorizontal => "arrow-right",
            Icon::DirVertical => "arrow-down",
            Icon::Wrap => "arrow-u-down-left",
            Icon::Spacing => "arrows-horizontal",
            Icon::PaddingH => "arrows-out-line-horizontal",
            Icon::PaddingV => "arrows-out-line-vertical",
            Icon::Dev => "code",
            Icon::Design => "cursor",
            Icon::Export => "download-simple",
            Icon::Sparkle => "sparkle",
            Icon::TextLeft => "text-align-left",
            Icon::TextCenter => "text-align-center",
            Icon::TextRight => "text-align-right",
            Icon::Distribute => "align-center-vertical-simple",
            Icon::Rotate90 => "arrows-clockwise",
            Icon::FlipH => "flip-horizontal",
            Icon::FlipV => "flip-vertical",
            Icon::Flag => "flag",
            Icon::Play => "play",
            Icon::Eyedropper => "eyedropper",
            Icon::Close => "x",
            Icon::Styles => "dots-nine",
            Icon::PaintSolid => "square",
            Icon::PaintLinear => "gradient",
            Icon::PaintRadial => "record",
            Icon::PaintAngular => "arrow-clockwise",
            Icon::PaintDiamond => "diamond",
            Icon::PaintImage => "image",
            Icon::Blend => "drop",
            Icon::NoPaint => "prohibit",
            Icon::InnerShadow => "square-half-bottom",
            Icon::DropShadow => "stack",
            Icon::LayerBlur => "dots-six",
            Icon::BackgroundBlur => "checkerboard",
            Icon::Noise => "dice-five",
            Icon::Texture => "waves",
            Icon::Glass => "circle-half-tilt",
            Icon::Shader => "sparkle",
            Icon::StrokeSettings => "sliders-horizontal",
            Icon::StrokeSides => "bounding-box",
            Icon::StrokeWeight => "text-align-justify",
            Icon::Terminal => "terminal-window",
            Icon::Grid2x2 => "grid-four",
            Icon::Home => "house",
            Icon::Wand => "magic-wand",
            Icon::StarFour => "star-four",
            Icon::BookOpen => "book-open",
            Icon::Microphone => "microphone",
            Icon::ArrowUp => "arrow-up",
            Icon::ArrowSquareOut => "arrow-square-out",
            Icon::Sliders => "sliders-horizontal",
        })
    }

    /// Backwards-compatible alias for [`Self::phosphor_name`].
    #[allow(dead_code)]
    pub fn lucide_name(self) -> Option<&'static str> {
        self.phosphor_name()
    }
}

macro_rules! phosphor {
    ($($name:literal),* $(,)?) => {
        /// The vendored Phosphor SVG sources (`viewBox="0 0 256 256"`), by Phosphor icon name.
        pub const PHOSPHOR: &[(&str, &str)] = &[
            $(($name, include_str!(concat!("../assets/icons/phosphor/", $name, ".svg")))),*
        ];
    };
}

phosphor!(
    "align-bottom",
    "align-center-horizontal",
    "align-center-vertical",
    "align-center-vertical-simple",
    "align-left",
    "align-right",
    "align-top",
    "arrow-clockwise",
    "arrow-down",
    "arrow-elbow-down-right",
    "arrow-right",
    "arrow-square-out",
    "arrow-u-down-left",
    "arrow-up",
    "arrow-up-right",
    "arrows-clockwise",
    "arrows-horizontal",
    "arrows-out",
    "arrows-out-line-horizontal",
    "arrows-out-line-vertical",
    "bezier-curve",
    "book",
    "book-open",
    "book-open-text",
    "bounding-box",
    "caret-down",
    "caret-right",
    "caret-up",
    "chat-circle",
    "check",
    "checkerboard",
    "circle",
    "circle-half",
    "circle-half-tilt",
    "clock",
    "code",
    "columns",
    "corners-out",
    "cursor",
    "diamond",
    "diamonds-four",
    "dice-five",
    "dots-nine",
    "dots-six",
    "download-simple",
    "drop",
    "eye",
    "eye-slash",
    "eyedropper",
    "file",
    "file-text",
    "flag",
    "flip-horizontal",
    "flip-vertical",
    "folder",
    "frame-corners",
    "gradient",
    "grid-four",
    "hand",
    "hexagon",
    "highlighter",
    "house",
    "image",
    "intersect-three",
    "knife",
    "line-segment",
    "list",
    "lock",
    "lock-open",
    "magic-wand",
    "magnifying-glass",
    "microphone",
    "minus",
    "note",
    "pen-nib",
    "pencil-line",
    "pencil-simple",
    "play",
    "plus",
    "plus-circle",
    "prohibit",
    "question",
    "record",
    "rows",
    "selection-all",
    "selection-plus",
    "shapes",
    "sidebar-simple",
    "sliders-horizontal",
    "sparkle",
    "square",
    "square-half-bottom",
    "squares-four",
    "stack",
    "stamp",
    "star",
    "star-four",
    "terminal-window",
    "text-align-center",
    "text-align-justify",
    "text-align-left",
    "text-align-right",
    "text-t",
    "toolbox",
    "trash",
    "triangle",
    "unite",
    "waves",
    "x",
);

/// Backwards-compatible alias for [`PHOSPHOR`].
#[allow(dead_code)]
pub const LUCIDE: &[(&str, &str)] = PHOSPHOR;

/// SVG source of the Phosphor icon behind `icon`.
pub fn svg_source(icon: Icon) -> Option<&'static str> {
    let name = icon.phosphor_name()?;
    PHOSPHOR.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
}

/// Rasterises `icon` into a `px` x `px` coverage mask (one alpha byte per pixel).
pub fn rasterize(icon: Icon, px: u32) -> Option<Vec<u8>> {
    use resvg::{tiny_skia, usvg};
    let src = svg_source(icon)?;
    let svg = if src.contains("currentColor") {
        src.replace("currentColor", "#ffffff")
    } else {
        src.replace("viewBox=\"0 0 256 256\"", "viewBox=\"0 0 256 256\" fill=\"#ffffff\"")
    };
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).ok()?;
    let mut pm = tiny_skia::Pixmap::new(px, px)?;
    let s = px as f32 / tree.size().width();
    resvg::render(&tree, tiny_skia::Transform::from_scale(s, s), &mut pm.as_mut());
    Some(pm.pixels().iter().map(|p| p.alpha()).collect())
}

type CacheMap = HashMap<(Icon, u32), TextureHandle>;

#[derive(Clone, Default)]
struct Cache(Arc<Mutex<CacheMap>>);

fn texture(ctx: &Context, icon: Icon, px: u32) -> Option<TextureHandle> {
    let cache = ctx.data_mut(|d| d.get_temp_mut_or_insert_with::<Cache>(egui::Id::new("designos-icon-cache"), Cache::default).clone());
    let mut map = cache.0.lock().ok()?;
    if let Some(t) = map.get(&(icon, px)) {
        return Some(t.clone());
    }
    let alpha = rasterize(icon, px)?;
    let pixels = alpha.iter().map(|&a| Color32::from_rgba_premultiplied(a, a, a, a)).collect();
    let img = egui::ColorImage { size: [px as usize; 2], source_size: Vec2::splat(px as f32), pixels };
    let h = ctx.load_texture(format!("icon-{icon:?}-{px}"), img, TextureOptions::LINEAR);
    map.insert((icon, px), h.clone());
    Some(h)
}

/// Where the glyph of `icon` lands when painted centred at `center` with side `size`:
/// pixel-snapped so a stroke never straddles a device pixel boundary.
pub fn glyph_rect(center: Pos2, size: f32, ppp: f32) -> Rect {
    let px = (size * ppp).round().max(1.0);
    let side = px / ppp;
    let snap = |v: f32| (v * ppp).round() / ppp;
    Rect::from_min_size(Pos2::new(snap(center.x - side / 2.0), snap(center.y - side / 2.0)), Vec2::splat(side))
}

/// Paints `icon` centered at `center`, fitted in a square of side `size`.
pub fn paint(p: &Painter, icon: Icon, center: Pos2, size: f32, col: Color32) {
    if icon == Icon::Logo {
        paint_logo(p, center, size);
        return;
    }
    let ppp = p.ctx().pixels_per_point();
    let rect = glyph_rect(center, size, ppp);
    #[cfg(test)]
    crate::lint::note_icon(p, icon, rect);
    let px = (size * ppp).round().max(1.0) as u32;
    if let Some(tex) = texture(p.ctx(), icon, px) {
        p.image(tex.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), col);
    }
}

/// The DESIGN:OS Studio brand mark: the orange spherical mark from `assets/icon/png/icon_256.png`.
fn paint_logo(p: &Painter, center: Pos2, size: f32) {
    let rect = Rect::from_center_size(center, Vec2::splat(size));
    let ctx = p.ctx();
    let id = egui::Id::new("designos-logo-tex");
    let cached = ctx.data_mut(|d| d.get_temp::<Option<TextureHandle>>(id));
    let tex = match cached {
        Some(t) => t,
        None => {
            const PNG: &[u8] = include_bytes!("../../../assets/icon/png/icon_256.png");
            let loaded = image::load_from_memory_with_format(PNG, image::ImageFormat::Png).ok().map(|img| {
                let rgba = img.into_rgba8();
                let (w, h) = rgba.dimensions();
                let cimg = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], rgba.as_raw());
                ctx.load_texture("designos-logo", cimg, TextureOptions::LINEAR)
            });
            ctx.data_mut(|d| d.insert_temp(id, loaded.clone()));
            loaded
        }
    };
    if let Some(tex) = tex {
        p.image(tex.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_has_a_parsable_asset() {
        for &icon in Icon::ALL {
            if icon == Icon::Logo {
                assert!(icon.phosphor_name().is_none());
                assert!(icon.lucide_name().is_none());
                continue;
            }
            let src = svg_source(icon).unwrap_or_else(|| panic!("{icon:?} has no vendored Phosphor svg"));
            let mask = rasterize(icon, 16).unwrap_or_else(|| panic!("{icon:?} does not parse"));
            assert_eq!(mask.len(), 256);
            assert!(mask.iter().any(|&a| a > 0), "{icon:?} rendered empty");
            assert!(src.contains("viewBox=\"0 0 256 256\""), "{icon:?} is not a 256x256 Phosphor SVG");
        }
    }

    #[test]
    fn all_lists_every_variant_once() {
        let mut seen = std::collections::HashSet::new();
        for i in Icon::ALL {
            assert!(seen.insert(*i), "{i:?} listed twice");
        }
        assert_eq!(seen.len(), 118);
    }

    #[test]
    fn no_vendored_svg_is_unused() {
        for (name, _) in PHOSPHOR {
            assert!(Icon::ALL.iter().any(|i| i.phosphor_name() == Some(*name)), "{name} is vendored in assets/icons/phosphor but unused");
        }
    }

    #[test]
    fn glyph_rect_is_pixel_snapped() {
        for ppp in [1.0f32, 1.5, 2.0] {
            let r = glyph_rect(Pos2::new(100.3, 50.7), 16.0, ppp);
            assert!(((r.min.x * ppp) - (r.min.x * ppp).round()).abs() < 1e-3);
            assert!(((r.min.y * ppp) - (r.min.y * ppp).round()).abs() < 1e-3);
            assert!(((r.width() * ppp) - (r.width() * ppp).round()).abs() < 1e-3);
        }
    }
}
