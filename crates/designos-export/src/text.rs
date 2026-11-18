//! Text layout for SVG export. It is the renderer's layout itself (`designos_render::text_runs`:
//! same shaping, wrapping, paragraphs, lists, OpenType features, leading trim and writing
//! direction), reshaped into per-line spans so the writer can emit either `<text>` lines or
//! outlined glyph paths.

use designos_doc as d;
use designos_doc::PathCmd;
#[cfg(test)]
use designos_doc::{TextProps, Vec2};

/// Kept for API compatibility: fonts registered with `designos_render::register_font` are what
/// text export lays out with, so there is nothing more to do.
pub fn register_font(_data: Vec<u8>) {}

/// A run of glyphs of one style on one line.
#[derive(Clone, Debug)]
pub struct Span {
    /// Index into [`TextOut::styles`].
    pub style: usize,
    pub x: f32,
    pub text: String,
    /// Glyph outlines (and underline/strike bars) in node-local space.
    pub outline: Vec<PathCmd>,
}

#[derive(Clone, Debug)]
pub struct Line {
    pub baseline: f32,
    pub spans: Vec<Span>,
}

#[derive(Clone, Debug)]
pub struct TextOut {
    /// 0 = base style, i + 1 = `runs[i].style`.
    pub styles: Vec<d::TextStyle>,
    pub lines: Vec<Line>,
}

/// Lays out `props` in a box of `size`.
#[cfg(test)]
pub fn layout(props: &TextProps, size: Vec2) -> TextOut {
    from_runs(designos_render::text_runs(props, size))
}

/// Lays out a text node like the canvas draws it: with the glyph outlines Figma embedded in the file
/// while the node's font is not installed. `None` for a node that is not text.
pub fn layout_node(node: &d::Node) -> Option<TextOut> {
    designos_render::text_runs_for_node(node).map(from_runs)
}

fn from_runs(o: designos_render::TextRuns) -> TextOut {
    TextOut {
        styles: o.styles,
        lines: o
            .lines
            .into_iter()
            .map(|l| Line {
                baseline: l.baseline,
                spans: l.spans.into_iter().map(|s| Span { style: s.style, x: s.x, text: s.text, outline: s.outline }).collect(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{ListItem, ListKind, TextAutoResize};

    fn props(content: &str, auto: TextAutoResize) -> TextProps {
        TextProps {
            content: content.into(),
            style: d::TextStyle { font_size: 16.0, line_height: d::LineHeight::Pixels(20.0), ..Default::default() },
            auto_resize: auto,
            ..Default::default()
        }
    }

    #[test]
    fn lays_out_lines_with_outlines() {
        let mut p = props("Hello world and more", TextAutoResize::Height);
        let out = layout(&p, Vec2::new(60.0, 100.0));
        assert!(out.lines.len() >= 2, "wraps in a 60px box");
        assert!(out.lines[0].spans[0].outline.len() > 4);
        assert!((out.lines[1].baseline - out.lines[0].baseline - 20.0).abs() < 0.01);
        p.auto_resize = TextAutoResize::WidthAndHeight;
        assert_eq!(layout(&p, Vec2::new(60.0, 100.0)).lines.len(), 1);
    }

    #[test]
    fn list_markers_and_indent_are_exported() {
        let mut p = props("one\ntwo", TextAutoResize::WidthAndHeight);
        p.set_paragraph_lists(&[Some(ListItem { kind: ListKind::Bullet, level: 0 }), Some(ListItem { kind: ListKind::Numbered, level: 0 })]);
        let out = layout(&p, Vec2::ZERO);
        assert_eq!(out.lines.len(), 2);
        // marker span first, then the text indented by the marker column
        assert_eq!(out.lines[0].spans[0].text, "\u{2022}");
        assert_eq!(out.lines[1].spans[0].text, "1.");
        assert!(out.lines[0].spans[0].outline.len() > 3, "the bullet has an outline");
        assert!(out.lines[0].spans[1].x > 20.0 && out.lines[0].spans[1].text == "one");
    }
}
