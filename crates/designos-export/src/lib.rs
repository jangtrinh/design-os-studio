//! Export and developer-inspect output for designos documents.
//!
//! * [`export_png`] / [`export_jpg`] rasterise a layer through `designos_render`,
//!   growing the image to include shadows, blur and outside strokes like Figma.
//! * [`export_svg`] writes real vector SVG (see [`SvgOptions`]).
//! * [`css_for`] prints Dev Mode style CSS for a layer; [`tailwind_for`], [`swiftui_for`],
//!   [`uikit_for`], [`compose_for`], [`android_xml_for`] and [`react_for`] generate code for the
//!   other Dev Mode languages (Figma's native set is CSS, SwiftUI, UIKit, Compose and Android XML).
//!
//! [`export_pdf`] exports ordered frames (deck slides or selected design frames) as
//! vector PDF 1.4 pages, one page per frame. Pages preserve frame dimensions,
//! vector paths, gradients, clipping masks, and selectable text runs.

mod android_xml;
mod bounds;
mod codegen;
mod compose;
mod css;
mod geo;
mod pdf;
mod raster;
mod react;
mod slots;
mod svg;
mod swiftui;
mod tailwind;
mod text;
mod uikit;
mod util;

pub use android_xml::android_xml_for;
pub use bounds::render_bounds;
pub use codegen::css_var_display as css_variable_reference;
pub use codegen::{is_vector_asset, vector_assets};
pub use compose::compose_for;
pub use css::{css_for, font_family_css, paint_css, text_style_css};
pub use pdf::export_pdf;
pub use raster::{export_jpg, export_png, export_webp, render_export, rgba_bytes, ExportScale};
pub use react::{react_for, react_with, ReactOptions};
pub use svg::{export_svg, SvgOptions};
pub use swiftui::swiftui_for;
pub use tailwind::tailwind_for;
pub use text::register_font;
pub use uikit::uikit_for;
pub use util::css_color;

/// Output file format of an export setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExportFormat {
    Png,
    Jpg,
    Svg,
    Pdf,
}

impl ExportFormat {
    pub const ALL: [ExportFormat; 4] = [ExportFormat::Png, ExportFormat::Jpg, ExportFormat::Svg, ExportFormat::Pdf];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Png => "PNG",
            Self::Jpg => "JPG",
            Self::Svg => "SVG",
            Self::Pdf => "PDF",
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpg => "jpg",
            Self::Svg => "svg",
            Self::Pdf => "pdf",
        }
    }

    /// Whether the scale setting means anything for this format. PDF pages use the
    /// frame's own size, so they take no `@2x` suffix either.
    pub fn is_raster(&self) -> bool {
        !matches!(self, Self::Svg | Self::Pdf)
    }
}

/// One row of the inspector's Export section.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportSetting {
    pub scale: ExportScale,
    pub format: ExportFormat,
}

impl Default for ExportSetting {
    fn default() -> Self {
        Self { scale: ExportScale::Factor(1.0), format: ExportFormat::Png }
    }
}

impl ExportSetting {
    /// `name@2x.png` style file name for a layer called `layer`.
    pub fn file_name(&self, layer: &str) -> String {
        let suffix = if self.format.is_raster() { self.scale.suffix() } else { String::new() };
        format!("{}{suffix}.{}", sanitize_file_name(layer), self.format.extension())
    }

    /// Encoded bytes for `node`; empty when nothing can be rendered (including a PDF
    /// the exporter rejects, e.g. a non-frame or oversized layer).
    pub fn render(&self, doc: &designos_doc::Document, node: designos_doc::NodeId) -> Vec<u8> {
        let size = doc.get(node).map(|n| n.size).unwrap_or_default();
        let f = self.scale.factor(size);
        match self.format {
            ExportFormat::Png => export_png(doc, node, f),
            ExportFormat::Jpg => export_jpg(doc, node, f, 90),
            ExportFormat::Svg => export_svg(doc, node, &SvgOptions::default()).into_bytes(),
            ExportFormat::Pdf => export_pdf(doc, &[node]).unwrap_or_default(),
        }
    }

    /// Encoded bytes for a whole selection. PDF writes one multi-page document with
    /// one page per node, in the given order; the other formats are one file per
    /// layer and so take exactly one node. Errors name the reason.
    pub fn render_many(&self, doc: &designos_doc::Document, nodes: &[designos_doc::NodeId]) -> Result<Vec<u8>, String> {
        if self.format == ExportFormat::Pdf {
            return export_pdf(doc, nodes);
        }
        let [node] = nodes else {
            return Err(format!("{} exports one layer per file.", self.format.label()));
        };
        let bytes = self.render(doc, *node);
        if bytes.is_empty() {
            return Err("Nothing to export.".into());
        }
        Ok(bytes)
    }
}

/// Replaces characters that are invalid in file names on common platforms.
pub fn sanitize_file_name(name: &str) -> String {
    let s: String = name.chars().map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() { '_' } else { c }).collect();
    let s = s.trim().trim_matches('.').to_string();
    if s.is_empty() {
        "export".into()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::*;

    fn shadow_doc() -> (Document, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(40.0, 40.0, 20.0, 10.0));
        n.fills = vec![Paint::solid(Color::rgb(255, 0, 0))];
        d.insert(page, n).unwrap();
        (d, id)
    }

    fn decode(png: &[u8]) -> tiny_skia::Pixmap {
        tiny_skia::Pixmap::decode_png(png).unwrap()
    }

    #[test]
    fn png_at_2x_has_exact_size_and_pixels() {
        let (d, id) = shadow_doc();
        let pm = decode(&export_png(&d, id, 2.0));
        assert_eq!((pm.width(), pm.height()), (40, 20));
        let px = |x, y| {
            let c = pm.pixel(x, y).unwrap();
            (c.red(), c.green(), c.blue(), c.alpha())
        };
        assert_eq!(px(0, 0), (255, 0, 0, 255));
        assert_eq!(px(39, 19), (255, 0, 0, 255));
        assert_eq!(px(20, 10), (255, 0, 0, 255));
    }

    #[test]
    fn png_includes_shadow_overflow() {
        let (mut d, id) = shadow_doc();
        d.get_mut(id).unwrap().effects.push(Effect::DropShadow {
            color: Color::BLACK,
            offset: Vec2::new(0.0, 6.0),
            radius: 4.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
        });
        let b = render_bounds(&d, id);
        // blur 4 reaches 4px left/right; offset 6 pushes the bottom out to 10
        // and the top reach (4 - 6) is inside the layer
        assert_eq!((b.x, b.y, b.w, b.h), (-4.0, 0.0, 28.0, 20.0));
        let pm = decode(&export_png(&d, id, 2.0));
        assert_eq!((pm.width(), pm.height()), (56, 40));
        // the layer body starts at x = 4 * 2; its top-left pixel stays red
        let c = pm.pixel(8, 0).unwrap();
        assert_eq!((c.red(), c.green(), c.blue(), c.alpha()), (255, 0, 0, 255));
        // below the layer the shadow is visible, in the far corner it is transparent
        let below = pm.pixel(28, 24).unwrap();
        assert!(below.alpha() > 0 && below.red() < 40, "shadow pixel {:?}", (below.red(), below.alpha()));
        assert_eq!(pm.pixel(0, 39).unwrap().alpha(), 0);
    }

    #[test]
    fn jpg_flattens_on_white() {
        let (mut d, id) = shadow_doc();
        d.get_mut(id).unwrap().opacity = 0.0;
        d.get_mut(id).unwrap().visible = true;
        let jpg = export_jpg(&d, id, 1.0, 90);
        assert_eq!(&jpg[..2], &[0xFF, 0xD8]);
        let img = image::load_from_memory(&jpg).unwrap().to_rgb8();
        assert_eq!(img.dimensions(), (20, 10));
        let p = img.get_pixel(5, 5);
        assert!(p[0] > 245 && p[1] > 245 && p[2] > 245, "{p:?}");
    }

    #[test]
    fn webp_lossless_decoded_output_equals_png_pixels() {
        let (mut d, id) = shadow_doc();
        d.get_mut(id).unwrap().effects.push(Effect::DropShadow {
            color: Color::rgba(20, 40, 80, 180),
            offset: Vec2::new(2.0, 4.0),
            radius: 3.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
        });
        let png = export_png(&d, id, 2.0);
        let webp = export_webp(&d, id, 2.0);
        assert!(webp.len() >= 12 && &webp[..4] == b"RIFF" && &webp[8..12] == b"WEBP", "expected RIFF/WEBP header, got {} bytes", webp.len());
        let png_img = image::load_from_memory_with_format(&png, image::ImageFormat::Png).unwrap().to_rgba8();
        let webp_img = image::load_from_memory_with_format(&webp, image::ImageFormat::WebP).unwrap().to_rgba8();
        assert_eq!(webp_img.dimensions(), png_img.dimensions());
        assert_eq!(webp_img.as_raw(), png_img.as_raw());
    }

    #[test]
    fn setting_file_names() {
        let s = ExportSetting { scale: ExportScale::Factor(2.0), format: ExportFormat::Png };
        assert_eq!(s.file_name("Hero / Card"), "Hero _ Card@2x.png");
        let s = ExportSetting { scale: ExportScale::Width(512), format: ExportFormat::Svg };
        assert_eq!(s.file_name("Icon"), "Icon.svg");
        assert_eq!(ExportSetting::default().file_name("a"), "a.png");
    }

    #[test]
    fn setting_render_uses_constraint() {
        let (d, id) = shadow_doc();
        let s = ExportSetting { scale: ExportScale::Width(60), format: ExportFormat::Png };
        let pm = decode(&s.render(&d, id));
        assert_eq!((pm.width(), pm.height()), (60, 30));
    }
}
