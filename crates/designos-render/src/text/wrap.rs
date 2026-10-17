//! `text-wrap: balance | pretty`: picks the width a paragraph breaks at.
//!
//! The shaper always breaks greedily at the width it is given. Balance and
//! pretty keep that breaker and only choose a narrower width for it, so the
//! renderer, the editor geometry and the measurement all agree on the lines
//! (one breaking function, three consumers). The alignment inside the box is
//! applied by the caller; the box itself keeps its full width.

use designos_doc::TextWrap;

/// What the chooser needs to know about one laid out line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct LineStat {
    /// Advance width of the line, trailing wrap whitespace excluded.
    pub width: f32,
    /// Whitespace separated words on the line.
    pub words: usize,
}

/// Balance is skipped past this many lines, as browsers do: long paragraphs gain little and
/// every probe is a full shaping pass.
const BALANCE_MAX_LINES: usize = 6;
/// Binary search steps for balance; the last step is below half a pixel for boxes up to ~2000px.
const BALANCE_STEPS: usize = 12;
/// Re-flows pretty may spend moving a word down to the last line.
const PRETTY_MAX_PASSES: usize = 8;
/// Narrowing pretty applies below the widest line, so that line's last word really moves.
const PRETTY_NARROW_BY: f32 = 0.5;

/// The width to break a paragraph at, never wider than `avail`.
///
/// `greedy` describes the paragraph broken at `avail`; `probe(w)` breaks it at `w` and
/// describes the result. Returns `avail` when nothing is gained or the line count would change.
pub(super) fn wrap_width(style: TextWrap, avail: f32, greedy: &[LineStat], probe: &mut dyn FnMut(f32) -> Vec<LineStat>) -> f32 {
    match style {
        TextWrap::Auto => avail,
        TextWrap::Balance => balance_width(avail, greedy.len(), probe),
        TextWrap::Pretty => pretty_width(avail, greedy, probe),
    }
}

/// The narrowest width that still breaks into `lines` lines: line widths come out as even as the
/// words allow (`text-wrap: balance`).
fn balance_width(avail: f32, lines: usize, probe: &mut dyn FnMut(f32) -> Vec<LineStat>) -> f32 {
    if !(2..=BALANCE_MAX_LINES).contains(&lines) {
        return avail;
    }
    // `hi` always breaks into `lines` lines (it starts as the greedy width); `lo` never does.
    let (mut lo, mut hi) = (0.0f32, avail);
    for _ in 0..BALANCE_STEPS {
        if hi - lo <= 0.5 {
            break;
        }
        let mid = (lo + hi) / 2.0;
        if probe(mid).len() == lines {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    hi
}

/// Narrows the width until the last line holds two or more words, keeping the line count
/// (`text-wrap: pretty`). A paragraph that cannot be fixed that way keeps its greedy breaks.
fn pretty_width(avail: f32, greedy: &[LineStat], probe: &mut dyn FnMut(f32) -> Vec<LineStat>) -> f32 {
    let lines = greedy.len();
    if lines < 2 || greedy[lines - 1].words >= 2 {
        return avail;
    }
    let mut width = avail;
    let mut stats = greedy.to_vec();
    for _ in 0..PRETTY_MAX_PASSES {
        // the widest line before the last one gives up its last word
        let widest = stats[..lines - 1].iter().map(|l| l.width).fold(0.0f32, f32::max);
        let next = (widest - PRETTY_NARROW_BY).min(width - PRETTY_NARROW_BY);
        if next <= 0.0 {
            break;
        }
        let tried = probe(next);
        if tried.len() != lines {
            break;
        }
        if tried[lines - 1].words >= 2 {
            return next;
        }
        width = next;
        stats = tried;
    }
    avail
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fixed advances: every character, spaces included, is 10px wide.
    const ADV: f32 = 10.0;

    /// Greedy breaking of `text` at `width` with fixed advances; the wrap space is dropped.
    fn greedy(text: &str, width: f32) -> Vec<LineStat> {
        let mut lines: Vec<LineStat> = Vec::new();
        let mut cur: Vec<usize> = Vec::new(); // word lengths on the current line
        let fits = |cur: &Vec<usize>, extra: usize| (cur.iter().sum::<usize>() + cur.len() + extra) as f32 * ADV <= width + 1e-3;
        for w in text.split(' ') {
            if !cur.is_empty() && !fits(&cur, w.len()) {
                lines.push(stat(&cur));
                cur.clear();
            }
            cur.push(w.len());
        }
        lines.push(stat(&cur));
        lines
    }

    fn stat(words: &[usize]) -> LineStat {
        LineStat { width: (words.iter().sum::<usize>() + words.len() - 1) as f32 * ADV, words: words.len() }
    }

    fn widths(lines: &[LineStat]) -> Vec<f32> {
        lines.iter().map(|l| l.width).collect()
    }

    fn spread(lines: &[LineStat]) -> f32 {
        let w = widths(lines);
        w.iter().cloned().fold(f32::MIN, f32::max) - w.iter().cloned().fold(f32::MAX, f32::min)
    }

    fn chosen(style: TextWrap, text: &str, avail: f32) -> (f32, Vec<LineStat>) {
        let before = greedy(text, avail);
        let w = wrap_width(style, avail, &before, &mut |w| greedy(text, w));
        (w, greedy(text, w))
    }

    // 6 + 6 + 6 + 6 + 2 letters, 200px box (20 characters).
    const BALANCE_TEXT: &str = "aaaaaa bbbbbb cccccc dddddd ee";

    #[test]
    fn balance_evens_out_the_lines_for_the_same_line_count() {
        // greedy: "aaaaaa bbbbbb cccccc" (20 chars = 200px) and "dddddd ee" (9 chars = 90px): spread 110
        let before = greedy(BALANCE_TEXT, 200.0);
        assert_eq!(widths(&before), vec![200.0, 90.0]);
        assert_eq!(spread(&before), 110.0);
        // balanced: "aaaaaa bbbbbb" (130px) and "cccccc dddddd ee" (160px): spread 30, from a width in 160..200
        let (w, after) = chosen(TextWrap::Balance, BALANCE_TEXT, 200.0);
        assert!((160.0..=160.5).contains(&w), "{w}");
        assert_eq!(widths(&after), vec![130.0, 160.0]);
        assert_eq!(spread(&after), 30.0);
    }

    #[test]
    fn balance_never_adds_lines_or_exceeds_the_box() {
        for avail in [160.0, 200.0, 260.0, 400.0] {
            let before = greedy(BALANCE_TEXT, avail);
            let (w, after) = chosen(TextWrap::Balance, BALANCE_TEXT, avail);
            assert_eq!(after.len(), before.len(), "avail {avail}");
            assert!(w <= avail, "avail {avail}: {w}");
            assert!(spread(&after) <= spread(&before), "avail {avail}");
        }
    }

    #[test]
    fn balance_leaves_one_line_and_long_paragraphs_alone() {
        assert_eq!(chosen(TextWrap::Balance, "aaa bbb", 200.0).0, 200.0);
        // eight lines of one 9-letter word each in a 100px box: past the cap
        let long = "aaaaaaaaa ".repeat(8);
        let long = long.trim_end();
        assert_eq!(greedy(long, 100.0).len(), 8);
        assert_eq!(chosen(TextWrap::Balance, long, 100.0).0, 100.0);
    }

    #[test]
    fn balance_keeps_a_word_wider_than_the_box_on_its_own_line() {
        // the 12-letter word (120px) never fits in 100px; the width cannot shrink below the greedy line count
        let (w, after) = chosen(TextWrap::Balance, "aa bbbbbbbbbbbb cc", 100.0);
        assert!(w <= 100.0);
        assert_eq!(after.len(), greedy("aa bbbbbbbbbbbb cc", 100.0).len());
    }

    // 7 letters x 4, 230px box (23 characters): greedy leaves "dddddd" alone on the last line.
    const PRETTY_TEXT: &str = "aaaaaaa bbbbbbb ccccccc ddddddd";

    #[test]
    fn pretty_pulls_a_word_down_so_the_last_line_is_not_a_single_word() {
        let before = greedy(PRETTY_TEXT, 230.0);
        assert_eq!(before.iter().map(|l| l.words).collect::<Vec<_>>(), vec![3, 1]);
        let (w, after) = chosen(TextWrap::Pretty, PRETTY_TEXT, 230.0);
        // "aaaaaaa bbbbbbb" and "ccccccc ddddddd": 150px each, from a width just under the old widest line
        assert_eq!(after.iter().map(|l| l.words).collect::<Vec<_>>(), vec![2, 2]);
        assert_eq!(widths(&after), vec![150.0, 150.0]);
        assert!(w < 230.0);
    }

    #[test]
    fn pretty_never_ends_with_a_one_word_line_on_the_fixture() {
        // word widths 70px: 2 words = 150px, 3 = 230px. At 160 greedy already ends on two words;
        // at 230 and 235 greedy leaves one word, which pretty fixes without adding a line.
        for (avail, lines) in [(160.0, 2), (230.0, 2), (235.0, 2)] {
            let (_, after) = chosen(TextWrap::Pretty, PRETTY_TEXT, avail);
            assert_eq!(after.len(), lines, "avail {avail}");
            assert!(after.last().unwrap().words >= 2, "avail {avail}: {after:?}");
        }
    }

    #[test]
    fn pretty_keeps_the_greedy_breaks_when_a_fix_would_add_a_line() {
        // three 9-letter words in 200px: "aaaaaaaaa bbbbbbbbb" / "ccccccccc"; narrowing to pull one down needs 3 lines
        let text = "aaaaaaaaa bbbbbbbbb ccccccccc";
        let before = greedy(text, 200.0);
        assert_eq!(before.iter().map(|l| l.words).collect::<Vec<_>>(), vec![2, 1]);
        let (w, after) = chosen(TextWrap::Pretty, text, 200.0);
        assert_eq!(w, 200.0);
        assert_eq!(after, before);
    }

    #[test]
    fn pretty_leaves_a_paragraph_alone_that_already_ends_well_or_is_one_line() {
        assert_eq!(chosen(TextWrap::Pretty, "aaaaaa bbbbbb cccccc dddddd ee", 160.0).0, 160.0);
        assert_eq!(chosen(TextWrap::Pretty, "aaaa bbbb", 200.0).0, 200.0);
        assert_eq!(chosen(TextWrap::Pretty, "aaaaaaaaaaaa", 50.0).0, 50.0);
    }

    #[test]
    fn auto_never_changes_the_width() {
        assert_eq!(chosen(TextWrap::Auto, BALANCE_TEXT, 200.0).0, 200.0);
    }

    // ---- through the shaper (bundled Inter), the same flow the renderer and the editor use

    use crate::text_geometry;
    use designos_doc as d;
    use designos_doc::{TextProps, Vec2};

    fn shaped(content: &str, wrap: TextWrap, align: d::TextAlignH, box_w: f32) -> (Vec<String>, Vec<f32>, Vec<(f32, f32)>) {
        let p = TextProps {
            content: content.into(),
            style: d::TextStyle { font_size: 16.0, line_height: d::LineHeight::Pixels(20.0), align_h: align, ..Default::default() },
            auto_resize: d::TextAutoResize::Height,
            text_wrap: wrap,
            ..Default::default()
        };
        let g = text_geometry(&p, Vec2::new(box_w, 400.0));
        let texts = g.lines.iter().map(|l| content[l.start..l.end].to_string()).collect();
        let widths = g.lines.iter().map(|l| l.width).collect();
        let spans = g.lines.iter().map(|l| (l.glyphs.first().map_or(0.0, |g| g.x), l.glyphs.last().map_or(0.0, |g| g.x + g.w))).collect();
        (texts, widths, spans)
    }
    fn range(w: &[f32]) -> f32 {
        w.iter().cloned().fold(f32::MIN, f32::max) - w.iter().cloned().fold(f32::MAX, f32::min)
    }

    const HEADLINE: &str = "The quick brown fox jumps over the lazy dog";
    const COPY: &str = "Ship small changes often and learn quickly from users";

    #[test]
    fn shaped_balance_narrows_the_gap_between_the_longest_and_shortest_line() {
        let (auto_t, auto_w, _) = shaped(HEADLINE, TextWrap::Auto, d::TextAlignH::Left, 230.0);
        let (bal_t, bal_w, _) = shaped(HEADLINE, TextWrap::Balance, d::TextAlignH::Left, 230.0);
        assert_eq!(auto_t, vec!["The quick brown fox jumps", "over the lazy dog"]);
        assert_eq!(bal_t, vec!["The quick brown fox", "jumps over the lazy dog"]);
        assert_eq!(auto_w.len(), bal_w.len());
        // measured with Inter 16px: 204.6 / 130.7 greedy against 154.5 / 180.8 balanced
        assert!(range(&auto_w) > 70.0 && range(&bal_w) < 30.0, "greedy {auto_w:?} balanced {bal_w:?}");
        assert!(bal_w.iter().all(|w| *w <= 230.0));
    }

    #[test]
    fn shaped_balance_keeps_centered_lines_centered_in_the_full_box() {
        let (_, _, spans) = shaped(HEADLINE, TextWrap::Balance, d::TextAlignH::Center, 230.0);
        for (i, (x0, x1)) in spans.iter().enumerate() {
            assert!(((x0 + x1) / 2.0 - 115.0).abs() < 1.0, "line {i} spans {x0}..{x1}");
        }
        let (_, _, right) = shaped(HEADLINE, TextWrap::Balance, d::TextAlignH::Right, 230.0);
        for (i, (_, x1)) in right.iter().enumerate() {
            assert!((x1 - 230.0).abs() < 1.0, "line {i} ends at {x1}");
        }
    }

    #[test]
    fn shaped_pretty_does_not_leave_a_single_word_on_the_last_line() {
        let (auto_t, _, _) = shaped(COPY, TextWrap::Auto, d::TextAlignH::Left, 200.0);
        assert_eq!(auto_t.last().unwrap(), "users", "greedy orphans the last word: {auto_t:?}");
        let (pretty_t, pretty_w, _) = shaped(COPY, TextWrap::Pretty, d::TextAlignH::Left, 200.0);
        assert_eq!(pretty_t, vec!["Ship small changes", "often and learn quickly", "from users"]);
        assert_eq!(pretty_t.len(), auto_t.len());
        assert!(pretty_w.iter().all(|w| *w <= 200.0));
    }

    #[test]
    fn shaped_auto_and_a_fitting_paragraph_are_untouched() {
        let (a, _, _) = shaped(HEADLINE, TextWrap::Auto, d::TextAlignH::Left, 230.0);
        let (b, _, _) = shaped(HEADLINE, TextWrap::Auto, d::TextAlignH::Left, 230.0);
        assert_eq!(a, b);
        for wrap in [TextWrap::Balance, TextWrap::Pretty] {
            let (t, _, _) = shaped("one line", wrap, d::TextAlignH::Left, 230.0);
            assert_eq!(t, vec!["one line"], "{wrap:?}");
        }
    }
}
