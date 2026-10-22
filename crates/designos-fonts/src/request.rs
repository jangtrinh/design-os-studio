use designos_doc::{Document, Style, TextProps, TextStyle};
use std::collections::BTreeSet;

/// One font face a document needs.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FontRequest {
    pub family: String,
    /// CSS weight 100..=900.
    pub weight: u16,
    pub italic: bool,
}

impl FontRequest {
    pub fn new(family: impl Into<String>, weight: u16, italic: bool) -> Self {
        Self { family: family.into(), weight, italic }
    }

    fn of(style: &TextStyle) -> Self {
        Self::new(style.font_family.clone(), style.font_weight, style.italic)
    }
}

/// Every family/weight/italic combination used by text nodes and their runs.
pub fn requests_for(doc: &Document) -> BTreeSet<FontRequest> {
    let mut out = BTreeSet::new();
    for style in doc.styles.values() {
        if let Style::Text(ts) = style {
            out.insert(FontRequest::of(ts));
        }
    }
    let limit = if doc.nodes.len() > 25_000 { 8_192 } else { usize::MAX };
    for node in doc.nodes.values().take(limit) {
        if let Some(t) = node.kind.any_text() {
            out.insert(FontRequest::of(&t.style));
            for run in &t.runs {
                out.insert(FontRequest::of(&run.style));
            }
        }
    }
    out
}

/// The faces a text uses, borrowed: its base style and every run.
fn faces(t: &TextProps) -> BTreeSet<(&str, u16, bool)> {
    std::iter::once(&t.style).chain(t.runs.iter().map(|r| &r.style)).map(|s| (s.font_family.as_str(), s.font_weight, s.italic)).collect()
}

/// Faces `after` uses that `before` did not: what an edit just introduced and the renderer cannot
/// draw until it is fetched. Empty for typing and for edits that leave the faces alone, and then
/// it allocates no strings.
pub fn new_faces(before: &TextProps, after: &TextProps) -> Vec<FontRequest> {
    let known = faces(before);
    faces(after).into_iter().filter(|f| !known.contains(f)).map(|(family, weight, italic)| FontRequest::new(family, weight, italic)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::TextRun;

    fn text(family: &str, runs: &[(u16, bool)]) -> TextProps {
        let base = TextStyle { font_family: family.into(), ..Default::default() };
        let runs = runs
            .iter()
            .enumerate()
            .map(|(i, (w, italic))| TextRun { start: i, end: i + 1, style: TextStyle { font_weight: *w, italic: *italic, ..base.clone() } })
            .collect();
        TextProps { content: "abc".into(), style: base, runs, ..Default::default() }
    }

    #[test]
    fn a_bold_run_is_a_new_face() {
        let before = text("Lato", &[]);
        let after = text("Lato", &[(700, false)]);
        assert_eq!(new_faces(&before, &after), vec![FontRequest::new("Lato", 700, false)]);
    }

    #[test]
    fn italic_and_family_changes_are_new_faces() {
        let before = text("Lato", &[(700, false)]);
        let after = text("Lato", &[(700, true), (400, true)]);
        assert_eq!(new_faces(&before, &after), vec![FontRequest::new("Lato", 400, true), FontRequest::new("Lato", 700, true)]);
        let mut other = text("Lato", &[]);
        other.style.font_family = "Roboto".into();
        assert_eq!(new_faces(&text("Lato", &[]), &other), vec![FontRequest::new("Roboto", 400, false)]);
    }

    #[test]
    fn faces_already_in_use_and_dropped_faces_are_not_new() {
        let both = text("Lato", &[(700, false), (700, false)]);
        assert!(new_faces(&both, &both).is_empty());
        assert!(new_faces(&both, &text("Lato", &[])).is_empty(), "toggling bold off asks for nothing");
        assert!(new_faces(&text("Lato", &[(700, false)]), &text("Lato", &[(700, false), (700, false)])).is_empty());
    }
}
