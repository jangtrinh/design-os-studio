//! SVG -> PNG through resvg 0.45, the same usvg major Studio's importer
//! (`crates/designos-svg`) uses. Fonts: the Inter bundled in
//! `crates/designos-fonts` is loaded into the fontdb; no system fonts, no
//! network, deterministic output.

use resvg::{tiny_skia, usvg};

static INTER: &[u8] = include_bytes!("../../../crates/designos-fonts/assets/inter/Inter[opsz,wght].ttf");

pub struct Raster {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Count of `<text>` runs the parser kept; 0 means labels were dropped.
    pub text_nodes: usize,
}

fn options() -> usvg::Options<'static> {
    let mut opt = usvg::Options::default();
    opt.fontdb_mut().load_font_data(INTER.to_vec());
    // Generic families (`sans-serif`, used by some engine-emitted <text>) resolve
    // to nothing in a fontdb without system fonts: map them all to Inter.
    opt.fontdb_mut().set_sans_serif_family("Inter");
    opt.fontdb_mut().set_serif_family("Inter");
    opt.fontdb_mut().set_monospace_family("Inter");
    opt.font_family = "Inter".into();
    opt
}

fn count_text(g: &usvg::Group) -> usize {
    g.children()
        .iter()
        .map(|n| match n {
            usvg::Node::Group(g) => count_text(g),
            usvg::Node::Text(_) => 1,
            _ => 0,
        })
        .sum()
}

/// `bg` is `#rrggbb`; `scale` 2.0 gives a retina capture.
pub fn svg_to_png(svg: &str, bg: &str, scale: f32) -> Result<Raster, String> {
    let tree = usvg::Tree::from_str(svg, &options()).map_err(|e| format!("usvg: {e}"))?;
    let size = tree.size();
    let (w, h) = ((size.width() * scale).ceil() as u32, (size.height() * scale).ceil() as u32);
    let mut pm = tiny_skia::Pixmap::new(w.max(1), h.max(1)).ok_or("pixmap alloc failed")?;
    let c = u32::from_str_radix(bg.trim_start_matches('#'), 16).map_err(|e| e.to_string())?;
    pm.fill(tiny_skia::Color::from_rgba8((c >> 16) as u8, (c >> 8) as u8, c as u8, 255));
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pm.as_mut());
    let png = pm.encode_png().map_err(|e| format!("png: {e}"))?;
    Ok(Raster { png, width: w, height: h, text_nodes: count_text(tree.root()) })
}
