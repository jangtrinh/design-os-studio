//! Rich text algebra over `TextProps`: per-range styling by splitting and
//! merging `TextRun`s, and content edits (splice) that keep the runs aligned.
//!
//! Model recap: `TextProps::style` is the base style (and the only source of
//! paragraph level fields: alignment and paragraph spacing). `runs` hold FULL
//! styles over UTF-8 byte ranges of `content` that differ from the base; where
//! runs overlap the later one wins (as in the renderer). All functions here
//! keep runs normalised: sorted, non-overlapping, non-empty, never equal to the
//! base style, with equal neighbours merged, and ranges on char boundaries.
//!
//! Lists: `TextProps::lists` marks whole paragraphs as bullet or numbered items at a nesting
//! level; the renderer draws the markers. Content edits keep the list ranges aligned (see
//! `designos_doc::lists_for_splice`), so Enter inside an item continues the list.

use designos_doc::{LineHeight, Paint, TextCase, TextDecoration, TextProps, TextRun, TextStyle};
use std::ops::Range;

/// A maximal byte range of content sharing one effective style.
#[derive(Clone, Debug, PartialEq)]
pub struct Seg {
    pub start: usize,
    pub end: usize,
    pub style: TextStyle,
}

/// Effective style of every byte of the content, as ordered gap-free segments
/// (empty for empty content). Adjacent equal styles are merged.
pub fn segments(t: &TextProps) -> Vec<Seg> {
    let len = t.content.len();
    if len == 0 {
        return vec![];
    }
    let mut cuts = vec![0, len];
    for r in &t.runs {
        cuts.push(r.start.min(len));
        cuts.push(r.end.min(len));
    }
    cuts.sort_unstable();
    cuts.dedup();
    let mut out: Vec<Seg> = Vec::new();
    for w in cuts.windows(2) {
        let (a, b) = (w[0], w[1]);
        if a == b {
            continue;
        }
        let mut style = &t.style;
        for r in &t.runs {
            if a >= r.start && a < r.end {
                style = &r.style;
            }
        }
        match out.last_mut() {
            Some(last) if last.style == *style => last.end = b,
            _ => out.push(Seg { start: a, end: b, style: style.clone() }),
        }
    }
    out
}

/// Rebuilds `runs` (and possibly the base style) from gap-free segments.
/// When every segment shares one style it becomes the base style (keeping the
/// base paragraph fields) and `runs` is cleared.
pub fn set_segments(t: &mut TextProps, mut segs: Vec<Seg>) {
    segs.retain(|s| s.end > s.start);
    // merge equal neighbours
    let mut merged: Vec<Seg> = Vec::new();
    for s in segs {
        match merged.last_mut() {
            Some(last) if last.style == s.style && last.end == s.start => last.end = s.end,
            _ => merged.push(s),
        }
    }
    if merged.len() == 1 && merged[0].start == 0 && merged[0].end == t.content.len() {
        let old = &t.style;
        let mut base = merged[0].style.clone();
        base.align_h = old.align_h;
        base.align_v = old.align_v;
        base.paragraph_spacing = old.paragraph_spacing;
        t.style = base;
        t.runs.clear();
        return;
    }
    t.runs = merged.into_iter().filter(|s| s.style != t.style).map(|s| TextRun { start: s.start, end: s.end, style: s.style }).collect();
}

/// Splits segments so that `at` is a boundary.
fn split_at(segs: &mut Vec<Seg>, at: usize) {
    if let Some(i) = segs.iter().position(|s| s.start < at && at < s.end) {
        let right = Seg { start: at, end: segs[i].end, style: segs[i].style.clone() };
        segs[i].end = at;
        segs.insert(i + 1, right);
    }
}

fn clamp_range(t: &TextProps, r: Range<usize>) -> Range<usize> {
    let len = t.content.len();
    let mut a = r.start.min(len);
    let mut b = r.end.min(len);
    while a > 0 && !t.content.is_char_boundary(a) {
        a -= 1;
    }
    while b < len && !t.content.is_char_boundary(b) {
        b += 1;
    }
    a..b.max(a)
}

/// Wraps a style edit so a new `font_weight` carries an existing `wght` axis along. The shaper
/// reads the axis before `font_weight`, so without this a weight edit on text with a `wght` axis
/// (imported from `.fig`, or set on the Variable tab) changes the panel but not the canvas. An edit
/// that sets the axis itself (the Variable tab) keeps its own value.
pub fn weight_in_step(f: impl Fn(&mut TextStyle)) -> impl Fn(&mut TextStyle) {
    move |st: &mut TextStyle| {
        let (weight, axis) = (st.font_weight, st.get_axis("wght"));
        f(st);
        if axis.is_some() && st.font_weight != weight && st.get_axis("wght") == axis {
            st.set_axis("wght", f32::from(st.font_weight));
        }
    }
}

/// Applies `f` to the style of every byte in `range` (clamped to char
/// boundaries). Returns false when the range is empty (nothing to style).
pub fn apply_style(t: &mut TextProps, range: Range<usize>, f: impl Fn(&mut TextStyle)) -> bool {
    let f = weight_in_step(f);
    let range = clamp_range(t, range);
    if range.is_empty() {
        return false;
    }
    let mut segs = segments(t);
    split_at(&mut segs, range.start);
    split_at(&mut segs, range.end);
    for s in &mut segs {
        if s.start >= range.start && s.end <= range.end {
            f(&mut s.style);
        }
    }
    set_segments(t, segs);
    true
}

/// Applies `f` to the whole text: the base style and every run.
pub fn apply_all(t: &mut TextProps, f: impl Fn(&mut TextStyle)) {
    let f = weight_in_step(f);
    let len = t.content.len();
    if len == 0 {
        f(&mut t.style);
        t.runs.clear();
        return;
    }
    // Paragraph level fields live on the base, so it is edited directly too.
    let mut segs = segments(t);
    for s in &mut segs {
        f(&mut s.style);
    }
    let mut base = t.style.clone();
    f(&mut base);
    t.style = base;
    set_segments(t, segs);
}

/// The effective style at byte `at` (the char starting there; the base style for empty text).
pub fn style_at(t: &TextProps, at: usize) -> TextStyle {
    let mut style = &t.style;
    for r in &t.runs {
        if at >= r.start && at < r.end {
            style = &r.style;
        }
    }
    style.clone()
}

/// Style used for typing at `caret`: the char before it (or after, at the start).
pub fn style_for_typing(t: &TextProps, caret: usize) -> TextStyle {
    if t.content.is_empty() {
        return t.style.clone();
    }
    if caret > 0 {
        let prev = t.content[..caret.min(t.content.len())].char_indices().next_back().map(|(i, _)| i).unwrap_or(0);
        style_at(t, prev)
    } else {
        style_at(t, 0)
    }
}

/// Replaces `range` of the content with `new` and keeps runs aligned. The new
/// text gets `style` when given, otherwise the style of the first replaced
/// char (or of the char before an insertion point). Returns the byte offset
/// just after the inserted text.
pub fn splice(t: &mut TextProps, range: Range<usize>, new: &str, style: Option<TextStyle>) -> usize {
    let range = clamp_range(t, range);
    let inherit = if range.is_empty() { style_for_typing(t, range.start) } else { style_at(t, range.start) };
    let new_style = style.unwrap_or(inherit);
    let mut segs = segments(t);
    split_at(&mut segs, range.start);
    split_at(&mut segs, range.end);
    let delta = new.len() as isize - range.len() as isize;
    let lists = designos_doc::lists_for_splice(t, range.clone(), new);
    let mut out: Vec<Seg> = Vec::with_capacity(segs.len() + 1);
    let mut inserted = false;
    for s in segs {
        if s.end <= range.start {
            out.push(s);
        } else if s.start >= range.end {
            if !inserted {
                out.push(Seg { start: range.start, end: range.start + new.len(), style: new_style.clone() });
                inserted = true;
            }
            out.push(Seg { start: (s.start as isize + delta) as usize, end: (s.end as isize + delta) as usize, style: s.style });
        }
        // else: inside the replaced range, dropped
    }
    if !inserted {
        out.push(Seg { start: range.start, end: range.start + new.len(), style: new_style });
    }
    t.content.replace_range(range.clone(), new);
    set_segments(t, out);
    if let Some(l) = lists {
        t.set_paragraph_lists(&l);
    }
    range.start + new.len()
}

/// A property that is either the same over a range or mixed.
pub type Mixed<T> = Option<T>;

/// Per property values over a range (`None` = mixed), for the inspector.
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub family: Mixed<String>,
    pub weight: Mixed<u16>,
    pub italic: Mixed<bool>,
    pub size: Mixed<f32>,
    pub line_height: Mixed<LineHeight>,
    pub letter_spacing: Mixed<f32>,
    pub fills: Mixed<Vec<Paint>>,
    pub decoration: Mixed<TextDecoration>,
    pub case: Mixed<TextCase>,
    /// OpenType feature settings (sorted by tag).
    pub features: Mixed<Vec<(String, u32)>>,
}

fn same<T: PartialEq + Clone>(items: &[&TextStyle], f: impl Fn(&TextStyle) -> T) -> Mixed<T> {
    let first = f(items.first()?);
    items.iter().all(|s| f(s) == first).then_some(first)
}

/// Summary of the styles over `range`; a collapsed range reads the style at the caret.
pub fn summary(t: &TextProps, range: Range<usize>) -> Summary {
    let range = clamp_range(t, range);
    let styles: Vec<TextStyle> = if range.is_empty() || t.content.is_empty() {
        vec![if t.content.is_empty() { t.style.clone() } else { style_for_typing(t, range.start) }]
    } else {
        segments(t).into_iter().filter(|s| s.start < range.end && s.end > range.start).map(|s| s.style).collect()
    };
    let refs: Vec<&TextStyle> = styles.iter().collect();
    Summary {
        family: same(&refs, |s| s.font_family.clone()),
        weight: same(&refs, |s| s.font_weight),
        italic: same(&refs, |s| s.italic),
        size: same(&refs, |s| s.font_size),
        line_height: same(&refs, |s| s.line_height),
        letter_spacing: same(&refs, |s| s.letter_spacing),
        fills: same(&refs, |s| s.fills.clone()),
        decoration: same(&refs, |s| s.decoration),
        case: same(&refs, |s| s.case),
        features: same(&refs, |s| {
            let mut f = s.features.clone();
            f.sort();
            f
        }),
    }
}

/// Font size in px a line height resolves to for a given size (for display).
pub fn line_height_px(lh: LineHeight, size: f32) -> Option<f32> {
    match lh {
        LineHeight::Auto => None,
        LineHeight::Pixels(p) => Some(p),
        LineHeight::Percent(p) => Some(size * p / 100.0),
    }
}

/// [`line_height_px`] for a field: automatic line height shows as the font's usual 121 % of the size.
pub fn display_line_height_px(lh: LineHeight, size: f32) -> f32 {
    line_height_px(lh, size).unwrap_or_else(|| (size * 1.21 * 10.0).round() / 10.0)
}

// ------------------------------------------------------------------ OpenType features
/// The value of OpenType feature `tag` in a style's feature list, if it is set.
pub fn feature_value(features: &[(String, u32)], tag: &str) -> Option<u32> {
    features.iter().find(|(t, _)| t == tag).map(|(_, v)| *v)
}

/// Sets (`Some`) or removes (`None`, back to the font's default) feature `tag`; the list stays sorted.
pub fn set_feature(features: &mut Vec<(String, u32)>, tag: &str, value: Option<u32>) {
    features.retain(|(t, _)| t != tag);
    if let Some(v) = value {
        features.push((tag.to_string(), v));
    }
    features.sort();
}

// ------------------------------------------------------------------ lists
pub use designos_doc::ListKind;
use designos_doc::{ListItem, MAX_LIST_LEVEL};

/// Byte ranges (excluding the newline) of the paragraphs touched by `range`.
pub fn paragraphs_in(content: &str, range: Range<usize>) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start = 0;
    for piece in content.split('\n') {
        let end = start + piece.len();
        let touches = if range.is_empty() { range.start >= start && range.start <= end } else { range.start <= end && range.end > start };
        if touches {
            out.push(start..end);
        }
        start = end + 1;
    }
    out
}

/// Indices of the paragraphs touched by `range`.
fn para_indices(content: &str, range: Range<usize>) -> Range<usize> {
    let all = designos_doc::paragraph_ranges(content);
    let touched = paragraphs_in(content, range);
    match (touched.first(), touched.last()) {
        (Some(a), Some(b)) => {
            let first = all.iter().position(|p| p.start == a.start).unwrap_or(0);
            let last = all.iter().position(|p| p.start == b.start).unwrap_or(first);
            first..last + 1
        }
        _ => 0..0,
    }
}

/// The list kind shared by every paragraph touched by `range` (`None` if any is not a list item
/// or they differ).
pub fn list_kind_in(t: &TextProps, range: Range<usize>) -> Option<ListKind> {
    let items = t.paragraph_lists();
    let idx = para_indices(&t.content, range);
    let first = items.get(idx.start).copied().flatten()?.kind;
    idx.clone().all(|i| items.get(i).copied().flatten().is_some_and(|it| it.kind == first)).then_some(first)
}

/// Toggles a list of `kind` on the paragraphs touched by `range`: removes the list if all of them
/// already have that kind, otherwise makes them items of that kind (keeping their nesting level).
pub fn toggle_list(t: &mut TextProps, range: Range<usize>, kind: ListKind) {
    let idx = para_indices(&t.content, range.clone());
    if idx.is_empty() {
        return;
    }
    let all_have = list_kind_in(t, range) == Some(kind);
    let mut items = t.paragraph_lists();
    for i in idx {
        items[i] = if all_have { None } else { Some(ListItem { kind, level: items[i].map_or(0, |it| it.level) }) };
    }
    t.set_paragraph_lists(&items);
}

/// Indents (`delta` > 0) or outdents list items touched by `range`. Paragraphs that are not list
/// items are left alone. Returns whether anything changed.
pub fn change_level(t: &mut TextProps, range: Range<usize>, delta: i8) -> bool {
    let idx = para_indices(&t.content, range);
    let mut items = t.paragraph_lists();
    let mut changed = false;
    for i in idx {
        if let Some(it) = items[i].as_mut() {
            let level = (it.level as i16 + delta as i16).clamp(0, MAX_LIST_LEVEL as i16) as u8;
            changed |= level != it.level;
            it.level = level;
        }
    }
    if changed {
        t.set_paragraph_lists(&items);
    }
    changed
}

/// What Enter does at a collapsed caret in an empty list item: outdent a nested item or end the
/// list. Returns whether the paragraph holding `caret` was such an item (and was changed).
pub fn exit_empty_item(t: &mut TextProps, caret: usize) -> bool {
    let idx = para_indices(&t.content, caret..caret);
    let Some(i) = idx.clone().next() else { return false };
    let paras = designos_doc::paragraph_ranges(&t.content);
    let mut items = t.paragraph_lists();
    match items.get(i).copied().flatten() {
        Some(it) if paras[i].is_empty() => {
            items[i] = if it.level > 0 { Some(ListItem { level: it.level - 1, ..it }) } else { None };
            t.set_paragraph_lists(&items);
            true
        }
        _ => false,
    }
}

/// Backspace at the very start of a list item's text: outdent a nested item or turn the item into a
/// plain paragraph. Returns whether `caret` was at such a position.
pub fn backspace_at_item_start(t: &mut TextProps, caret: usize) -> bool {
    let idx = para_indices(&t.content, caret..caret);
    let Some(i) = idx.clone().next() else { return false };
    let paras = designos_doc::paragraph_ranges(&t.content);
    if paras[i].start != caret {
        return false;
    }
    let mut items = t.paragraph_lists();
    match items.get(i).copied().flatten() {
        Some(it) => {
            items[i] = if it.level > 0 { Some(ListItem { level: it.level - 1, ..it }) } else { None };
            t.set_paragraph_lists(&items);
            true
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Color, TextAutoResize};

    fn props(s: &str) -> TextProps {
        TextProps {
            content: s.into(),
            style: TextStyle::default(),
            runs: vec![],
            auto_resize: TextAutoResize::WidthAndHeight,
            max_lines: None,
            ..Default::default()
        }
    }

    fn bold(s: &mut TextStyle) {
        s.font_weight = 700;
    }

    /// Runs invariants that every operation must keep.
    fn check(t: &TextProps) {
        let mut prev_end = 0;
        for r in &t.runs {
            assert!(r.start < r.end, "empty run {r:?}");
            assert!(r.start >= prev_end, "unsorted/overlapping {:?}", t.runs);
            assert!(r.end <= t.content.len());
            assert!(t.content.is_char_boundary(r.start) && t.content.is_char_boundary(r.end), "{r:?} in {:?}", t.content);
            assert_ne!(r.style, t.style, "run equal to base");
            prev_end = r.end;
        }
        for w in t.runs.windows(2) {
            assert!(!(w[0].end == w[1].start && w[0].style == w[1].style), "unmerged neighbours");
        }
    }

    fn rd(t: &TextProps) -> Vec<(usize, usize, u16)> {
        t.runs.iter().map(|r| (r.start, r.end, r.style.font_weight)).collect()
    }

    #[test]
    fn style_middle_splits_and_whole_merges_into_base() {
        let mut t = props("hello world");
        assert!(apply_style(&mut t, 2..5, bold));
        check(&t);
        assert_eq!(rd(&t), vec![(2, 5, 700)]);
        assert_eq!(segments(&t).len(), 3);
        // extend over neighbours: merges into one run
        apply_style(&mut t, 5..8, bold);
        assert_eq!(rd(&t), vec![(2, 8, 700)]);
        // un-bold part of it
        apply_style(&mut t, 4..6, |s| s.font_weight = 400);
        assert_eq!(rd(&t), vec![(2, 4, 700), (6, 8, 700)]);
        // bold everything: runs vanish and the base style carries it
        apply_style(&mut t, 0..11, bold);
        assert!(t.runs.is_empty());
        assert_eq!(t.style.font_weight, 700);
        check(&t);
    }

    #[test]
    fn empty_range_and_clamping() {
        let mut t = props("abc");
        assert!(!apply_style(&mut t, 2..2, bold));
        assert!(apply_style(&mut t, 1..99, bold));
        assert_eq!(rd(&t), vec![(1, 3, 700)]);
        assert!(!apply_style(&mut props(""), 0..0, bold));
    }

    #[test]
    fn multibyte_and_emoji_ranges_stay_on_char_boundaries() {
        let mut t = props("aé😀中b");
        // é is 2 bytes (1..3), 😀 is 4 (3..7), 中 is 3 (7..10)
        assert!(apply_style(&mut t, 2..4, bold), "mid-char offsets are widened");
        check(&t);
        assert_eq!(rd(&t), vec![(1, 7, 700)]);
        let mut t = props("aé😀中b");
        apply_style(&mut t, 4..8, bold);
        check(&t);
        assert_eq!(rd(&t), vec![(3, 10, 700)]);
        // splice inside a run with emoji
        let mut t = props("a😀b");
        apply_style(&mut t, 1..5, bold);
        let after = splice(&mut t, 5..5, "é", None);
        assert_eq!(after, 7);
        assert_eq!(t.content, "a😀éb");
        assert_eq!(rd(&t), vec![(1, 7, 700)], "typing after bold text stays bold");
        check(&t);
    }

    #[test]
    fn splice_shifts_and_inherits() {
        let mut t = props("abcdef");
        apply_style(&mut t, 2..4, bold); // cd
                                         // insert before the bold run: plain
        splice(&mut t, 0..0, "XX", None);
        assert_eq!(t.content, "XXabcdef");
        assert_eq!(rd(&t), vec![(4, 6, 700)]);
        // insert in the middle of the run: bold, run grows
        splice(&mut t, 5..5, "__", None);
        assert_eq!(t.content, "XXabc__def");
        assert_eq!(rd(&t), vec![(4, 8, 700)]);
        // delete across the run boundary
        splice(&mut t, 3..6, "", None);
        assert_eq!(t.content, "XXa_def");
        check(&t);
        assert_eq!(rd(&t), vec![(3, 5, 700)]);
        // replace everything
        splice(&mut t, 0..8, "new", None);
        assert_eq!(t.content, "new");
        assert!(t.runs.is_empty());
        // explicit style for typed text
        let mut b = t.style.clone();
        b.font_weight = 700;
        splice(&mut t, 3..3, "!", Some(b));
        assert_eq!(rd(&t), vec![(3, 4, 700)]);
        check(&t);
    }

    #[test]
    fn replace_selection_takes_first_char_style() {
        let mut t = props("abcd");
        apply_style(&mut t, 0..2, bold);
        splice(&mut t, 1..3, "ZZZ", None);
        assert_eq!(t.content, "aZZZd");
        assert_eq!(rd(&t), vec![(0, 4, 700)]);
        check(&t);
    }

    #[test]
    fn delete_to_empty_clears_runs() {
        let mut t = props("ab");
        apply_style(&mut t, 0..1, bold);
        splice(&mut t, 0..2, "", None);
        assert!(t.runs.is_empty() && t.content.is_empty());
    }

    #[test]
    fn overlapping_legacy_runs_normalise_with_later_winning() {
        let mut t = props("abcdef");
        let mut a = t.style.clone();
        a.font_size = 20.0;
        let mut b = t.style.clone();
        b.font_size = 30.0;
        t.runs = vec![TextRun { start: 0, end: 4, style: a }, TextRun { start: 2, end: 6, style: b }];
        let segs = segments(&t);
        assert_eq!(segs.iter().map(|s| (s.start, s.end, s.style.font_size)).collect::<Vec<_>>(), vec![(0, 2, 20.0), (2, 6, 30.0)]);
        apply_style(&mut t, 0..1, |s| s.italic = true);
        check(&t);
    }

    #[test]
    fn summary_reports_mixed() {
        let mut t = props("hello world");
        apply_style(&mut t, 0..5, |s| {
            s.font_size = 24.0;
            s.font_weight = 700;
        });
        let all = summary(&t, 0..11);
        assert_eq!(all.size, None);
        assert_eq!(all.weight, None);
        assert_eq!(all.family, Some("Inter".to_string()));
        let first = summary(&t, 0..5);
        assert_eq!((first.size, first.weight), (Some(24.0), Some(700)));
        let caret_end = summary(&t, 11..11);
        assert_eq!(caret_end.size, Some(12.0));
        let caret_mid = summary(&t, 3..3);
        assert_eq!(caret_mid.size, Some(24.0), "caret reads the char before it");
        assert_eq!(summary(&props(""), 0..0).size, Some(12.0));
    }

    #[test]
    fn colour_decoration_and_case_apply_per_range() {
        let mut t = props("one two");
        apply_style(&mut t, 4..7, |s| {
            s.fills = vec![Paint::solid(Color::rgba(255, 0, 0, 255))];
            s.decoration = TextDecoration::Underline;
            s.case = TextCase::Upper;
        });
        check(&t);
        let s = summary(&t, 0..7);
        assert_eq!((s.fills, s.decoration, s.case), (None, None, None));
        let s = summary(&t, 4..7);
        assert_eq!((s.decoration, s.case), (Some(TextDecoration::Underline), Some(TextCase::Upper)));
    }

    #[test]
    fn apply_all_edits_base_and_runs() {
        let mut t = props("ab cd");
        apply_style(&mut t, 0..2, |s| s.font_size = 30.0);
        apply_all(&mut t, |s| s.font_weight = 700);
        check(&t);
        assert_eq!(t.style.font_weight, 700);
        assert_eq!(t.runs.len(), 1);
        assert_eq!((t.runs[0].style.font_size, t.runs[0].style.font_weight), (30.0, 700));
        let mut e = props("");
        apply_all(&mut e, |s| s.font_size = 40.0);
        assert_eq!(e.style.font_size, 40.0);
    }

    #[test]
    fn base_alignment_survives_merging_into_base() {
        let mut t = props("abc");
        t.style.align_h = designos_doc::TextAlignH::Center;
        apply_style(&mut t, 0..3, bold);
        assert_eq!(t.style.align_h, designos_doc::TextAlignH::Center);
        assert_eq!(t.style.font_weight, 700);
    }

    fn kinds(t: &TextProps) -> Vec<Option<(ListKind, u8)>> {
        t.paragraph_lists().iter().map(|i| i.map(|i| (i.kind, i.level))).collect()
    }

    #[test]
    fn lists_toggle_without_touching_the_text() {
        let mut t = props("one\ntwo\nthree");
        let n = t.content.len();
        toggle_list(&mut t, 0..n, ListKind::Bullet);
        assert_eq!(t.content, "one\ntwo\nthree", "no literal prefixes any more");
        assert_eq!(kinds(&t), vec![Some((ListKind::Bullet, 0)); 3]);
        assert_eq!(list_kind_in(&t, 0..n), Some(ListKind::Bullet));
        // toggling again removes
        toggle_list(&mut t, 0..n, ListKind::Bullet);
        assert!(t.lists.is_empty());
        check(&t);
    }

    #[test]
    fn numbered_lists_switch_kind_and_touch_only_selected_paragraphs() {
        let mut t = props("a\nb\nc");
        toggle_list(&mut t, 0..5, ListKind::Numbered);
        assert_eq!(kinds(&t), vec![Some((ListKind::Numbered, 0)); 3]);
        toggle_list(&mut t, 0..5, ListKind::Bullet);
        assert_eq!(kinds(&t), vec![Some((ListKind::Bullet, 0)); 3], "a different kind replaces the list");
        let mut t = props("x\ny");
        toggle_list(&mut t, 3..3, ListKind::Bullet);
        assert_eq!(kinds(&t), vec![None, Some((ListKind::Bullet, 0))]);
        // a mixed selection becomes the kind first
        let mut t = props("x\ny");
        toggle_list(&mut t, 3..3, ListKind::Bullet);
        toggle_list(&mut t, 0..3, ListKind::Bullet);
        assert_eq!(kinds(&t), vec![Some((ListKind::Bullet, 0)); 2]);
    }

    #[test]
    fn levels_change_with_tab_and_stay_in_range() {
        let mut t = props("a\nb\nplain");
        toggle_list(&mut t, 0..3, ListKind::Numbered);
        assert!(change_level(&mut t, 2..3, 1));
        assert_eq!(kinds(&t), vec![Some((ListKind::Numbered, 0)), Some((ListKind::Numbered, 1)), None]);
        assert!(!change_level(&mut t, 4..9, 1), "plain paragraphs have no level");
        assert!(change_level(&mut t, 2..3, -1));
        assert!(!change_level(&mut t, 2..3, -1), "level 0 is the floor");
        for _ in 0..20 {
            change_level(&mut t, 0..1, 1);
        }
        assert_eq!(kinds(&t)[0], Some((ListKind::Numbered, 8)));
    }

    #[test]
    fn enter_and_backspace_rules() {
        let mut t = props("one");
        toggle_list(&mut t, 0..0, ListKind::Bullet);
        // splitting keeps the list
        splice(&mut t, 3..3, "\n", None);
        splice(&mut t, 4..4, "\n", None);
        assert_eq!(t.content, "one\n\n");
        assert_eq!(kinds(&t), vec![Some((ListKind::Bullet, 0)); 3]);
        // an empty item: Enter ends the list for it
        assert!(exit_empty_item(&mut t, 4));
        assert_eq!(kinds(&t), vec![Some((ListKind::Bullet, 0)), None, Some((ListKind::Bullet, 0))]);
        assert!(!exit_empty_item(&mut t, 0), "not an empty item");
        // nested empty items outdent first
        let mut n = props("a");
        toggle_list(&mut n, 0..0, ListKind::Bullet);
        splice(&mut n, 1..1, "\n", None);
        change_level(&mut n, 2..2, 1);
        assert!(exit_empty_item(&mut n, 2));
        assert_eq!(kinds(&n)[1], Some((ListKind::Bullet, 0)));
        // backspace at the start of an item unlists it
        let mut b = props("ab");
        toggle_list(&mut b, 0..0, ListKind::Numbered);
        assert!(!backspace_at_item_start(&mut b, 1));
        assert!(backspace_at_item_start(&mut b, 0));
        assert!(b.lists.is_empty());
    }

    #[test]
    fn list_membership_follows_edits_and_styles() {
        let mut t = props("é\n😀");
        apply_style(&mut t, 0..2, bold);
        let n = t.content.len();
        toggle_list(&mut t, 0..n, ListKind::Bullet);
        check(&t);
        // typing in the first item shifts the range of the second
        splice(&mut t, 0..0, "x", None);
        assert_eq!(kinds(&t), vec![Some((ListKind::Bullet, 0)); 2]);
        // deleting the newline merges the items into one
        let nl = t.content.find('\n').unwrap();
        splice(&mut t, nl..nl + 1, "", None);
        assert_eq!(kinds(&t), vec![Some((ListKind::Bullet, 0))]);
        check(&t);
    }

    #[test]
    fn paragraph_ranges() {
        let c = "ab\n\ncd";
        assert_eq!(paragraphs_in(c, 0..0), vec![0..2]);
        assert_eq!(paragraphs_in(c, 3..3), vec![3..3]);
        assert_eq!(paragraphs_in(c, 1..5), vec![0..2, 3..3, 4..6]);
        assert_eq!(paragraphs_in(c, 2..2), vec![0..2]);
    }

    fn with_wght(weight: u16, axis: f32) -> TextProps {
        let mut t = props("hello world");
        t.style.font_weight = weight;
        t.style.set_axis("wght", axis);
        t
    }

    #[test]
    fn a_weight_edit_carries_the_wght_axis_on_a_range_and_on_the_whole_text() {
        let mut t = with_wght(400, 400.0);
        apply_style(&mut t, 0..5, bold);
        assert_eq!(t.runs.len(), 1);
        assert_eq!((t.runs[0].style.font_weight, t.runs[0].style.get_axis("wght")), (700, Some(700.0)));
        assert_eq!(t.style.get_axis("wght"), Some(400.0), "the rest of the text keeps its axis");
        apply_all(&mut t, bold);
        assert_eq!((t.style.font_weight, t.style.get_axis("wght")), (700, Some(700.0)));
        apply_all(&mut t, |s| s.font_weight = 400);
        assert_eq!(t.style.get_axis("wght"), Some(400.0));
        check(&t);
    }

    #[test]
    fn a_weight_edit_adds_no_axis_and_other_edits_leave_the_axis_alone() {
        let mut plain = props("hello");
        apply_all(&mut plain, bold);
        assert!(plain.style.variation_axes.is_empty());
        let mut t = with_wght(400, 400.0);
        apply_all(&mut t, |s| s.font_size = 40.0);
        assert_eq!(t.style.get_axis("wght"), Some(400.0));
    }

    #[test]
    fn an_edit_that_sets_the_axis_itself_keeps_its_value() {
        let mut t = with_wght(400, 400.0);
        apply_all(&mut t, |s| {
            s.set_axis("wght", 549.6);
            s.font_weight = 550;
        });
        assert_eq!((t.style.font_weight, t.style.get_axis("wght")), (550, Some(549.6)), "the Variable tab writes both and owns the fraction");
    }
}
