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
        Self::new(style.font_family.clone(), face_weight(style.font_weight), style.italic)
    }
}

/// The 100-step face a weight is drawn from. Fonts ship in steps of 100, and the cache stores one
/// file per requested weight, so a weight between steps (a Variable tab value such as 401) asks
/// for the nearest step instead of fetching a copy of the same file for every value.
fn face_weight(weight: u16) -> u16 {
    (weight.saturating_add(50) / 100 * 100).clamp(100, 900)
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
    std::iter::once(&t.style).chain(t.runs.iter().map(|r| &r.style)).map(|s| (s.font_family.as_str(), face_weight(s.font_weight), s.italic)).collect()
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
    fn a_weight_between_the_steps_asks_for_the_nearest_100_step_face() {
        let at = |w: u16| text("Lato", &[(w, false)]);
        assert!(new_faces(&at(400), &at(401)).is_empty(), "the Variable tab steps 401, 402, ... without a new face");
        assert_eq!(new_faces(&at(400), &at(520)), vec![FontRequest::new("Lato", 500, false)]);
        assert_eq!(new_faces(&at(400), &at(1000)), vec![FontRequest::new("Lato", 900, false)]);
        let mut doc = Document::new("w");
        let page = doc.add_page("p");
        let id = doc.alloc_id();
        doc.insert(
            page,
            designos_doc::Node::text(
                id,
                designos_doc::Rect::new(0.0, 0.0, 1.0, 1.0),
                "x",
                TextStyle { font_family: "Lato".into(), font_weight: 450, ..Default::default() },
            ),
        )
        .unwrap();
        assert_eq!(requests_for(&doc).into_iter().collect::<Vec<_>>(), vec![FontRequest::new("Lato", 500, false)]);
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
