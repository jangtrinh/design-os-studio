//! Inline Markdown (bold, italic, code, links) as one egui `LayoutJob`, so a paragraph wraps as a whole.

use crate::theme;
use designos_docs::{plain_text, Inline};
use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId, Stroke};

/// Size, line height, weight and colour of a run of reading text.
#[derive(Clone, Copy, Debug)]
pub struct TextStyle {
    pub size: f32,
    pub line: f32,
    pub weight: theme::TextWeight,
    pub color: Color32,
}

/// A link inside a laid-out text: its span in characters and the Markdown target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkSpan {
    pub start: usize,
    pub end: usize,
    pub target: String,
}

#[derive(Clone, Default)]
struct Marks {
    bold: bool,
    italic: bool,
    link: Option<String>,
}

struct Builder<'a> {
    job: &'a mut LayoutJob,
    base: TextStyle,
    chars: usize,
    links: Vec<LinkSpan>,
}

/// Lays `inlines` out in `style`. Returns the job (wrapped at `width`; `f32::INFINITY` for no wrapping) and the links.
pub fn job(inlines: &[Inline], style: TextStyle, width: f32) -> (LayoutJob, Vec<LinkSpan>) {
    let mut job = LayoutJob::default();
    job.wrap.max_width = width;
    let mut b = Builder { job: &mut job, base: style, chars: 0, links: Vec::new() };
    b.walk(inlines, &Marks::default());
    let links = b.links;
    if job.text.is_empty() {
        // An empty paragraph still has a line, so give the job one section with the right height.
        job.append("", 0.0, format_for(&style, &Marks::default()));
    }
    (job, links)
}

/// Roboto Mono at `size` (the app's monospace family).
pub fn mono(size: f32) -> FontId {
    FontId::new(size, egui::FontFamily::Monospace)
}

/// Text that is not Markdown source, in `style`, wrapped at `width`.
pub fn styled_plain(text: &str, style: TextStyle, width: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = width;
    job.append(text, 0.0, format_for(&style, &Marks::default()));
    job
}

/// Plain text in one style (a table header, a code line), wrapped at `width`.
pub fn plain(text: &str, font: FontId, line: f32, color: Color32, width: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = width;
    job.append(text, 0.0, TextFormat { font_id: font, color, line_height: Some(line), ..Default::default() });
    job
}

fn format_for(style: &TextStyle, marks: &Marks) -> TextFormat {
    let weight = if marks.bold { theme::TextWeight::Semibold } else { style.weight };
    let linked = marks.link.is_some();
    let color = if marks.bold || linked { theme::TEXT } else { style.color };
    TextFormat {
        font_id: theme::font_text(style.size, weight),
        color,
        line_height: Some(style.line),
        italics: marks.italic,
        underline: if linked { Stroke::new(1.0, theme::TEXT) } else { Stroke::NONE },
        ..Default::default()
    }
}

impl Builder<'_> {
    fn push(&mut self, text: &str, format: TextFormat, link: &Option<String>) {
        if text.is_empty() {
            return;
        }
        let start = self.chars;
        let count = text.chars().count();
        self.chars += count;
        self.job.append(text, 0.0, format);
        if let Some(target) = link {
            match self.links.last_mut() {
                Some(last) if last.end == start && last.target == *target => last.end = self.chars,
                _ => self.links.push(LinkSpan { start, end: self.chars, target: target.clone() }),
            }
        }
    }

    fn walk(&mut self, nodes: &[Inline], marks: &Marks) {
        for node in nodes {
            match node {
                Inline::Text(_) => {
                    let text = plain_text(std::slice::from_ref(node));
                    self.push(&text, format_for(&self.base, marks), &marks.link);
                }
                Inline::Code(code) => {
                    let mut format = format_for(&self.base, marks);
                    format.font_id = mono(13.0);
                    format.color = theme::TEXT;
                    format.background = theme::BG_TERTIARY;
                    format.expand_bg = 2.0;
                    self.push(code, format, &marks.link);
                }
                Inline::Bold(inner) => self.walk(inner, &Marks { bold: true, ..marks.clone() }),
                Inline::Italic(inner) => self.walk(inner, &Marks { italic: true, ..marks.clone() }),
                Inline::Link { content, target } => self.walk(content, &Marks { link: Some(target.clone()), ..marks.clone() }),
            }
        }
    }
}

/// The link under `char_index`, if any.
pub fn link_at(links: &[LinkSpan], char_index: usize) -> Option<&LinkSpan> {
    links.iter().find(|l| char_index >= l.start && char_index < l.end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style() -> TextStyle {
        TextStyle { size: 14.0, line: 24.0, weight: theme::TextWeight::Regular, color: theme::TEXT_SECONDARY }
    }

    #[test]
    fn marks_become_sections_and_links_keep_their_character_span() {
        let nodes = vec![
            Inline::Text("See ".into()),
            Inline::Bold(vec![Inline::Text("bold".into())]),
            Inline::Text(" and ".into()),
            Inline::Link { content: vec![Inline::Text("the ".into()), Inline::Italic(vec![Inline::Text("flow".into())])], target: "spec/flow.md".into() },
            Inline::Text(" with ".into()),
            Inline::Code("x < 2".into()),
        ];
        let (job, links) = job(&nodes, style(), 400.0);
        assert_eq!(job.text, "See bold and the flow with x < 2");
        assert_eq!(links, vec![LinkSpan { start: 13, end: 21, target: "spec/flow.md".into() }]);
        assert_eq!(link_at(&links, 13).map(|l| l.target.as_str()), Some("spec/flow.md"));
        assert!(link_at(&links, 12).is_none() && link_at(&links, 21).is_none());
        let bold = job.sections.iter().find(|s| job.text[s.byte_range.start.0..s.byte_range.end.0] == *"bold").unwrap();
        assert_eq!(bold.format.color, theme::TEXT);
        let code = job.sections.last().unwrap();
        assert_eq!(code.format.background, theme::BG_TERTIARY);
    }

    #[test]
    fn escapes_are_resolved_and_an_empty_paragraph_still_has_a_line() {
        let (job, _) = job(&[Inline::Text("2 \\* 3".into())], style(), 400.0);
        assert_eq!(job.text, "2 * 3");
        let (empty, links) = super::job(&[], style(), 400.0);
        assert!(links.is_empty());
        assert_eq!(empty.sections.len(), 1);
    }
}
