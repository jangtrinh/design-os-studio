//! Text inside an instance made with Figma's scale tool (`uniformScaleFactor`).
//!
//! Figma keeps such a text linked to its text style and draws the style's values multiplied by the
//! instance's factor: a 14 px / 20 px style draws at 11.2 px / 16 px in a 0.8 instance. Studio stores the
//! multiplied values on the layer and the factor in [`TextProps::uniform_scale`], so whatever writes a
//! style value or a typography variable into the layer multiplies it by that factor too.

use crate::{detach_text_style_bindings, resolve_node, Document, LineHeight, Node, NodeKind, OwnedFieldSet, TextProps, TextStyle};

/// Variable binding keys of the text lengths a uniform scale multiplies. The family and weight are not lengths.
pub const SCALED_TEXT_METRIC_KEYS: [&str; 6] = ["fontSize", "lineHeight", "letterSpacing", "paragraphSpacing", "paragraphIndent", "listSpacing"];

/// Whether a value bound to `key` on a text layer is a length its uniform scale multiplies.
pub fn is_scaled_text_metric_key(key: &str) -> bool {
    SCALED_TEXT_METRIC_KEYS.contains(&key)
}

/// Whether `s` is a factor worth applying: finite, positive and not 1.
fn applies(s: f32) -> bool {
    s.is_finite() && s > 0.0 && s != 1.0
}

impl TextStyle {
    /// Multiplies the lengths of this style by `s`: the size, a pixel line height (a percent or automatic
    /// one follows the size by itself), letter and paragraph spacing, indent and list spacing. A factor that
    /// is not finite and positive changes nothing.
    pub fn scale_metrics(&mut self, s: f32) {
        if !applies(s) {
            return;
        }
        self.font_size *= s;
        if let LineHeight::Pixels(px) = &mut self.line_height {
            *px *= s;
        }
        self.letter_spacing *= s;
        self.paragraph_spacing *= s;
        self.paragraph_indent *= s;
        self.list_spacing *= s;
    }
}

impl TextProps {
    /// Multiplies every length of this text by `s`: the base style, each character range, and the paragraph
    /// indent and list spacing. [`Self::uniform_scale`] is left as it is; a factor that is not finite and
    /// positive changes nothing.
    pub fn scale_metrics(&mut self, s: f32) {
        if !applies(s) {
            return;
        }
        self.style.scale_metrics(s);
        for r in &mut self.runs {
            r.style.scale_metrics(s);
        }
        self.paragraph.indent *= s;
        self.paragraph.list_spacing *= s;
    }

    /// Takes this layer into a scaled instance: multiplies its lengths by `s` and records the factor.
    pub fn apply_uniform_scale(&mut self, s: f32) {
        if !applies(s) {
            return;
        }
        self.scale_metrics(s);
        self.uniform_scale *= s;
    }

    /// [`Self::uniform_scale`] when it is finite and positive, else 1 (a value read from a file is not trusted).
    pub fn uniform_scale_factor(&self) -> f32 {
        if self.uniform_scale.is_finite() && self.uniform_scale > 0.0 {
            self.uniform_scale
        } else {
            1.0
        }
    }

    /// The factor that turns lengths stored on `from` into this layer's: its own factor over `from`'s.
    pub fn scale_relative_to(&self, from: &TextProps) -> f32 {
        let r = self.uniform_scale_factor() / from.uniform_scale_factor();
        if r.is_finite() && r > 0.0 {
            r
        } else {
            1.0
        }
    }
}

/// Key of the `style_refs` entry that links a text layer to its text style.
const TEXT_STYLE_REF: &str = "text";

impl TextProps {
    /// Whether this text draws its style at a factor other than 1 (see [`Self::uniform_scale`]).
    pub fn has_uniform_scale(&self) -> bool {
        applies(self.uniform_scale_factor())
    }
}

/// The text `node` draws right now, when it is a text layer with a factor (see [`TextProps::uniform_scale`]): its
/// lengths with the variables and the linked style applied, at the scale. `None` for any other layer.
pub fn drawn_scaled_text(doc: &Document, node: &Node) -> Option<TextProps> {
    let NodeKind::Text(own) = &node.kind else { return None };
    if !own.has_uniform_scale() {
        return None;
    }
    match resolve_node(doc, node).into_owned().kind {
        NodeKind::Text(drawn) => Some(drawn),
        _ => None,
    }
}

/// `node` as it must be once it sits outside every instance: a text layer with a factor takes the values it draws
/// now as its own (size, line height, letter and paragraph spacing, indent, list spacing and the family, weight and
/// style of the base and of each run), and loses the text style link, the typography variable bindings, the style
/// overrides and the factor. Its colour, and every other variable binding, stay. `None` when `node` is not a text
/// layer with a factor, which has nothing to take.
pub fn baked_uniform_scale(doc: &Document, node: &Node) -> Option<Node> {
    let drawn = drawn_scaled_text(doc, node)?;
    let mut out = node.clone();
    let NodeKind::Text(t) = &mut out.kind else { return None };
    let held = |own: &TextStyle, drawn: &TextStyle| TextStyle { fills: own.fills.clone(), ..drawn.clone() };
    t.style = held(&t.style, &drawn.style);
    if t.runs.len() == drawn.runs.len() {
        for (run, d) in t.runs.iter_mut().zip(&drawn.runs) {
            run.style = held(&run.style, &d.style);
        }
    }
    t.paragraph = drawn.paragraph;
    t.uniform_scale = 1.0;
    detach_text_style_bindings(&mut out);
    out.style_refs.remove(TEXT_STYLE_REF);
    out.text_style_overrides = OwnedFieldSet::EMPTY;
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextRun;

    fn style(size: f32, lh: LineHeight) -> TextStyle {
        TextStyle {
            font_size: size,
            line_height: lh,
            letter_spacing: 2.0,
            paragraph_spacing: 10.0,
            paragraph_indent: 5.0,
            list_spacing: 4.0,
            ..Default::default()
        }
    }

    #[test]
    fn scaling_multiplies_lengths_and_leaves_relative_line_heights() {
        let mut px = style(14.0, LineHeight::Pixels(20.0));
        px.scale_metrics(0.8);
        assert!((px.font_size - 11.2).abs() < 1e-5);
        assert!(matches!(px.line_height, LineHeight::Pixels(v) if (v - 16.0).abs() < 1e-5));
        assert!((px.letter_spacing - 1.6).abs() < 1e-5 && (px.paragraph_spacing - 8.0).abs() < 1e-5);
        assert!((px.paragraph_indent - 4.0).abs() < 1e-5 && (px.list_spacing - 3.2).abs() < 1e-5);
        for lh in [LineHeight::Percent(150.0), LineHeight::Auto] {
            let mut s = style(14.0, lh);
            s.scale_metrics(0.8);
            assert_eq!(s.line_height, lh, "a relative line height follows the scaled size by itself");
        }
    }

    #[test]
    fn a_factor_that_is_not_finite_and_positive_changes_nothing() {
        for bad in [f32::INFINITY, f32::NAN, 0.0, -1.0] {
            let mut t = TextProps { style: style(14.0, LineHeight::Pixels(20.0)), ..Default::default() };
            let before = t.clone();
            t.apply_uniform_scale(bad);
            assert_eq!(t, before, "factor {bad}");
        }
    }

    #[test]
    fn every_run_and_the_paragraph_scale_and_the_factor_compounds() {
        let mut t = TextProps {
            content: "ab".into(),
            style: style(14.0, LineHeight::Pixels(20.0)),
            runs: vec![TextRun { start: 0, end: 1, style: style(20.0, LineHeight::Percent(120.0)) }],
            ..Default::default()
        };
        t.paragraph.indent = 10.0;
        t.paragraph.list_spacing = 6.0;
        t.apply_uniform_scale(0.5);
        t.apply_uniform_scale(0.8);
        assert!((t.uniform_scale - 0.4).abs() < 1e-6);
        assert!((t.style.font_size - 5.6).abs() < 1e-5);
        assert!((t.runs[0].style.font_size - 8.0).abs() < 1e-5);
        assert_eq!(t.runs[0].style.line_height, LineHeight::Percent(120.0));
        assert!((t.paragraph.indent - 4.0).abs() < 1e-5 && (t.paragraph.list_spacing - 2.4).abs() < 1e-5);
        let main = TextProps::default();
        assert!((t.scale_relative_to(&main) - 0.4).abs() < 1e-6);
        assert_eq!(main.scale_relative_to(&main), 1.0);
        let broken = TextProps { uniform_scale: f32::NAN, ..Default::default() };
        assert_eq!(broken.uniform_scale_factor(), 1.0);
        assert_eq!(TextProps { uniform_scale: -2.0, ..Default::default() }.uniform_scale_factor(), 1.0);
    }
}
