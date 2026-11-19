//! Whether the face a text style asks for (family, weight and style) is the face Studio shapes it with.

use designos_doc::*;

/// Faces Studio embeds, as `designos-fonts/src/assets.rs` lists them (that crate is not a dependency of this one):
/// family, italic, and the weight range of the variable font. Embedded faces shadow installed copies of the family.
const BUNDLED: [(&str, bool, u16, u16); 3] = [("Inter", false, 100, 900), ("Inter", true, 100, 900), ("Roboto Mono", false, 100, 700)];

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FontState {
    /// The family resolves to itself and the face covers the weight and the style.
    Present,
    /// An installed (not embedded) face of the family: its weight and style are not read by this test.
    WeightUnverified,
    /// Another family, or the embedded family without that weight or style.
    Missing,
}

impl FontState {
    pub fn scope(self) -> &'static str {
        match self {
            FontState::Present => "",
            FontState::WeightUnverified => "font weight unverified / ",
            FontState::Missing => "missing font / ",
        }
    }
}

pub fn font_state(st: &TextStyle) -> FontState {
    let Some(r) = designos_render::resolve_font(&st.font_family, st.font_weight, st.italic) else { return FontState::Missing };
    if !r.family.eq_ignore_ascii_case(&st.font_family) {
        return FontState::Missing;
    }
    if !r.bundled {
        return FontState::WeightUnverified;
    }
    let covers =
        |&(f, italic, lo, hi): &(&str, bool, u16, u16)| f.eq_ignore_ascii_case(&r.family) && italic == st.italic && (lo..=hi).contains(&st.font_weight);
    if BUNDLED.iter().any(covers) {
        FontState::Present
    } else {
        FontState::Missing
    }
}

/// Whether shaping draws the embedded variable Inter wider at weight 700 than at 400: if not, a weight counted as
/// present is drawn at the default weight.
pub fn variable_weight_applied() -> bool {
    let width = |w: u16| {
        let mut t = TextProps { content: "Hamburgefonstiv".into(), auto_resize: TextAutoResize::WidthAndHeight, ..TextProps::default() };
        t.style.font_family = "Inter".into();
        t.style.font_weight = w;
        designos_render::text_geometry(&t, Vec2::new(1000.0, 100.0)).width
    };
    width(700) > width(400) + 0.5
}
