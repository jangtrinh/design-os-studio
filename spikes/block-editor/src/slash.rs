//! THROWAWAY slash menu model: items, accent- and case-insensitive filter, sections.

use crate::model::Kind;

pub struct Item {
    pub label: &'static str,
    pub hint: &'static str,
    pub section: &'static str,
    pub kind: Option<Kind>, // None = needs a later phase (frame, state, diagram, table, divider)
}

pub const ITEMS: &[Item] = &[
    Item { label: "Text", hint: "", section: "Basic", kind: Some(Kind::Text) },
    Item { label: "Heading 1", hint: "#", section: "Basic", kind: Some(Kind::H1) },
    Item { label: "Heading 2", hint: "##", section: "Basic", kind: Some(Kind::H2) },
    Item { label: "Heading 3", hint: "###", section: "Basic", kind: Some(Kind::H3) },
    Item { label: "Bulleted list", hint: "-", section: "Basic", kind: Some(Kind::Bullet) },
    Item { label: "Numbered list", hint: "1.", section: "Basic", kind: Some(Kind::Numbered) },
    Item { label: "Checklist", hint: "[]", section: "Basic", kind: Some(Kind::Check) },
    Item { label: "Callout", hint: ">", section: "Basic", kind: Some(Kind::Callout) },
    Item { label: "Code", hint: "```", section: "Basic", kind: Some(Kind::Code) },
    Item { label: "State", hint: "", section: "Design", kind: None },
    Item { label: "Table", hint: "", section: "Data", kind: None },
];

/// Lower-cases and strips Vietnamese / Latin diacritics (no normalisation crate in the spike).
pub fn fold(s: &str) -> String {
    const GROUPS: &[(&str, char)] = &[
        ("àáảãạăằắẳẵặâầấẩẫậ", 'a'),
        ("èéẻẽẹêềếểễệ", 'e'),
        ("ìíỉĩị", 'i'),
        ("òóỏõọôồốổỗộơờớởỡợ", 'o'),
        ("ùúủũụưừứửữự", 'u'),
        ("ỳýỷỹỵ", 'y'),
        ("đ", 'd'),
    ];
    s.chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| GROUPS.iter().find(|(set, _)| set.contains(c)).map(|(_, base)| *base).unwrap_or(c))
        .collect()
}

/// Indices into `ITEMS` matching `query`, in menu order.
pub fn filter(query: &str) -> Vec<usize> {
    let q = fold(query.trim());
    (0..ITEMS.len()).filter(|&i| q.is_empty() || fold(ITEMS[i].label).contains(&q)).collect()
}

/// Finds an open slash command: the text before the caret ends with `/query` where the slash starts
/// the block or follows a space. Returns `(slash char index, query)`.
pub fn trigger(text: &str, caret: usize) -> Option<(usize, String)> {
    let before: Vec<char> = text.chars().take(caret).collect();
    let slash = before.iter().rposition(|&c| c == '/')?;
    if slash > 0 && before[slash - 1] != ' ' {
        return None;
    }
    let query: String = before[slash + 1..].iter().collect();
    if query.contains(' ') && filter(&query).is_empty() {
        return None;
    }
    Some((slash, query))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_is_accent_and_case_insensitive() {
        assert_eq!(filter("").len(), ITEMS.len());
        assert_eq!(filter("TA").len(), 2, "State and Table, as in the mockup");
        assert!(filter("zz").is_empty());
        assert_eq!(fold("Đường Thẻ ÁÀ"), "duong the aa");
        assert_eq!(filter("hêading").len(), 3, "accented query still matches");
    }

    #[test]
    fn trigger_needs_start_or_space_before_slash() {
        assert_eq!(trigger("/ta", 3), Some((0, "ta".into())));
        assert_eq!(trigger("see /h", 6), Some((4, "h".into())));
        assert_eq!(trigger("a/b", 3), None);
        assert_eq!(trigger("/", 1), Some((0, String::new())));
        assert_eq!(trigger("no slash", 8), None);
    }
}
