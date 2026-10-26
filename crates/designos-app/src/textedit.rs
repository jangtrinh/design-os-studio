//! Pure text-editing helpers: grapheme / word / paragraph navigation over byte
//! offsets and the in-session undo log with burst coalescing.

use designos_doc::TextProps;
use unicode_segmentation::UnicodeSegmentation;

/// Previous grapheme boundary before `at`.
pub fn prev_grapheme(s: &str, at: usize) -> usize {
    s[..at.min(s.len())].grapheme_indices(true).next_back().map(|(i, _)| i).unwrap_or(0)
}

/// Next grapheme boundary after `at`.
pub fn next_grapheme(s: &str, at: usize) -> usize {
    let at = at.min(s.len());
    s[at..].graphemes(true).next().map(|g| at + g.len()).unwrap_or(s.len())
}

fn is_word_seg(w: &str) -> bool {
    w.chars().any(|c| c.is_alphanumeric() || c == '_')
}

/// Ctrl+Left target: start of the previous word.
pub fn prev_word(s: &str, at: usize) -> usize {
    let at = at.min(s.len());
    let mut target = 0;
    for (i, w) in s.split_word_bound_indices() {
        if i >= at {
            break;
        }
        if is_word_seg(w) {
            target = i;
        }
    }
    target
}

/// Ctrl+Right target: end of the current/next word.
pub fn next_word(s: &str, at: usize) -> usize {
    let at = at.min(s.len());
    for (i, w) in s.split_word_bound_indices() {
        let end = i + w.len();
        if end > at && is_word_seg(w) {
            return end;
        }
    }
    s.len()
}

/// Byte range of the word (or whitespace / punctuation run) under `at`, for double-click.
pub fn word_at(s: &str, at: usize) -> std::ops::Range<usize> {
    if s.is_empty() {
        return 0..0;
    }
    let at = at.min(s.len());
    let mut best = s.len()..s.len();
    for (i, w) in s.split_word_bound_indices() {
        let end = i + w.len();
        if at >= i && at < end {
            return i..end;
        }
        if end == s.len() && at == end {
            best = i..end;
        }
    }
    best
}

/// Byte range of the paragraph (newline excluded) under `at`, for triple-click.
pub fn paragraph_at(s: &str, at: usize) -> std::ops::Range<usize> {
    let at = at.min(s.len());
    let start = s[..at].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let end = s[at..].find('\n').map(|i| at + i).unwrap_or(s.len());
    start..end
}

/// Text with control characters removed (tabs become spaces, `\r\n` newlines), for paste.
pub fn clean_input(s: &str) -> String {
    s.replace("\r\n", "\n")
        .replace('\r', "\n")
        .chars()
        .filter_map(|c| {
            if c == '\t' {
                Some(' ')
            } else if c.is_control() && c != '\n' {
                None
            } else {
                Some(c)
            }
        })
        .collect()
}

/// What kind of edit produced an undo step (decides coalescing).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditKind {
    Typing,
    Deleting,
    /// Inspector style edits (scrubbing a size, dragging a colour): coalesced per burst.
    Style,
    /// Anything that always stands alone: paste, cut, shortcuts, lists, Enter.
    Other,
}

/// Editor state captured for undo.
#[derive(Clone, Debug, PartialEq)]
pub struct Snap {
    pub props: TextProps,
    /// The text style the layer is linked to, restored together with the text values.
    pub style_ref: Option<String>,
    pub caret: usize,
    pub anchor: usize,
}

/// Undo/redo log of one text editing session. Typing coalesces into one step
/// per word / burst; the caller records the state *before* each edit.
#[derive(Default)]
pub struct UndoLog {
    undo: Vec<Snap>,
    redo: Vec<Snap>,
    last: Option<(EditKind, f64)>,
}

/// A pause longer than this starts a new undo step.
pub const BURST_SECONDS: f64 = 1.0;

impl UndoLog {
    /// Records `before` unless this edit continues the previous burst.
    /// `new_word` forces a step (the typed char starts a new word).
    pub fn record(&mut self, before: Snap, kind: EditKind, now: f64, new_word: bool) {
        let continues = match (self.last, kind) {
            (Some((k, t)), EditKind::Typing | EditKind::Deleting | EditKind::Style) => k == kind && now - t < BURST_SECONDS && !new_word,
            _ => false,
        };
        self.last = Some((kind, now));
        self.redo.clear();
        if !continues {
            self.undo.push(before);
        }
    }

    /// Breaks the current burst (caret moves, selection changes).
    pub fn break_burst(&mut self) {
        self.last = None;
    }

    pub fn undo(&mut self, current: Snap) -> Option<Snap> {
        let s = self.undo.pop()?;
        self.redo.push(current);
        self.last = None;
        Some(s)
    }

    pub fn redo(&mut self, current: Snap) -> Option<Snap> {
        let s = self.redo.pop()?;
        self.undo.push(current);
        self.last = None;
        Some(s)
    }

    #[cfg(test)]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
}

/// Whether typing `typed` at `caret` begins a new word (previous char is a
/// space/newline and the new one is not).
pub fn starts_new_word(content: &str, caret: usize, typed: &str) -> bool {
    let prev = content[..caret.min(content.len())].chars().next_back();
    let first = typed.chars().next();
    match (prev, first) {
        (Some(p), Some(c)) => p.is_whitespace() && !c.is_whitespace(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{TextAutoResize, TextStyle};

    fn snap(s: &str) -> Snap {
        Snap {
            props: TextProps {
                content: s.into(),
                style: TextStyle::default(),
                runs: vec![],
                auto_resize: TextAutoResize::WidthAndHeight,
                max_lines: None,
                ..Default::default()
            },
            style_ref: None,
            caret: s.len(),
            anchor: s.len(),
        }
    }

    #[test]
    fn graphemes_cover_emoji_and_combining_marks() {
        let s = "a\u{0301}👨‍👩‍👧b"; // a + combining acute, family emoji (ZWJ sequence), b
        let first = next_grapheme(s, 0);
        assert_eq!(&s[..first], "a\u{0301}");
        let second = next_grapheme(s, first);
        assert_eq!(&s[first..second], "👨‍👩‍👧");
        assert_eq!(prev_grapheme(s, second), first);
        assert_eq!(prev_grapheme(s, first), 0);
        assert_eq!(next_grapheme(s, s.len()), s.len());
        assert_eq!(prev_grapheme(s, 0), 0);
    }

    #[test]
    fn word_navigation() {
        let s = "hello  big-world, ok";
        assert_eq!(next_word(s, 0), 5);
        assert_eq!(next_word(s, 5), 10);
        assert_eq!(prev_word(s, s.len()), 18);
        assert_eq!(prev_word(s, 18), 11);
        assert_eq!(prev_word(s, 5), 0);
        assert_eq!(prev_word(s, 0), 0);
        assert_eq!(next_word(s, s.len()), s.len());
        let cjk = "héllo wörld";
        assert_eq!(next_word(cjk, 0), "héllo".len());
        assert_eq!(prev_word(cjk, cjk.len()), "héllo ".len());
    }

    #[test]
    fn word_and_paragraph_selection() {
        let s = "one two\nthree";
        assert_eq!(word_at(s, 1), 0..3);
        assert_eq!(word_at(s, 4), 4..7);
        assert_eq!(word_at(s, 3), 3..4, "the gap is selected too");
        assert_eq!(word_at(s, s.len()), 8..13);
        assert_eq!(word_at("", 0), 0..0);
        assert_eq!(paragraph_at(s, 2), 0..7);
        assert_eq!(paragraph_at(s, 9), 8..13);
        assert_eq!(paragraph_at(s, 7), 0..7, "a caret at the line end stays in its paragraph");
        assert_eq!(paragraph_at("a\n\nb", 2), 2..2);
    }

    #[test]
    fn clean_input_normalises() {
        assert_eq!(clean_input("a\r\nb\tc\u{7}d\re"), "a\nb cd\ne");
        assert_eq!(clean_input("😀é"), "😀é");
    }

    #[test]
    fn typing_coalesces_per_word_and_burst() {
        let mut log = UndoLog::default();
        // "ab" typed quickly: one step
        log.record(snap(""), EditKind::Typing, 0.0, false);
        log.record(snap("a"), EditKind::Typing, 0.1, false);
        // space then new word: new step
        log.record(snap("ab"), EditKind::Typing, 0.2, false);
        log.record(snap("ab "), EditKind::Typing, 0.3, true);
        log.record(snap("ab c"), EditKind::Typing, 0.4, false);
        // a pause starts another
        log.record(snap("ab cd"), EditKind::Typing, 2.0, false);
        // deleting is its own burst
        log.record(snap("ab cde"), EditKind::Deleting, 2.1, false);
        log.record(snap("ab cd"), EditKind::Deleting, 2.2, false);
        let cur = snap("ab c");
        let s = log.undo(cur).unwrap();
        assert_eq!(s.props.content, "ab cde");
        assert_eq!(log.undo(snap("x")).unwrap().props.content, "ab cd");
        assert_eq!(log.undo(snap("x")).unwrap().props.content, "ab ");
        assert_eq!(log.undo(snap("x")).unwrap().props.content, "");
        assert!(log.undo(snap("x")).is_none());
    }

    #[test]
    fn other_edits_always_stand_alone_and_redo_works() {
        let mut log = UndoLog::default();
        log.record(snap("a"), EditKind::Other, 0.0, false);
        log.record(snap("b"), EditKind::Other, 0.1, false);
        let back = log.undo(snap("c")).unwrap();
        assert_eq!(back.props.content, "b");
        let fwd = log.redo(snap("b")).unwrap();
        assert_eq!(fwd.props.content, "c");
        // a new edit clears redo
        log.undo(snap("c"));
        log.record(snap("b"), EditKind::Typing, 1.0, false);
        assert!(log.redo(snap("z")).is_none());
        // typing right after an undo is a fresh step
        log.break_burst();
        log.record(snap("q"), EditKind::Typing, 1.05, false);
        assert!(log.can_undo());
    }

    #[test]
    fn new_word_detection() {
        assert!(starts_new_word("ab ", 3, "c"));
        assert!(!starts_new_word("ab", 2, "c"));
        assert!(!starts_new_word("ab", 2, " "));
        assert!(!starts_new_word("", 0, "a"));
    }
}
