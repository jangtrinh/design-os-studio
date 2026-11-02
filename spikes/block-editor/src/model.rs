//! THROWAWAY block model. Text and bold ranges are indexed in chars (egui cursors are char based).
//! Pure functions only, so the editing rules are unit-tested without a window.

use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Text,
    H1,
    H2,
    H3,
    Bullet,
    Numbered,
    Check,
    Callout,
    Code,
}

impl Kind {
    pub fn is_list(self) -> bool {
        matches!(self, Kind::Bullet | Kind::Numbered | Kind::Check)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub id: u64,
    pub kind: Kind,
    pub checked: bool,
    pub text: String,
    /// Sorted, non-overlapping, non-empty, non-adjacent char ranges.
    pub bold: Vec<Range<usize>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Doc {
    pub blocks: Vec<Block>,
    next_id: u64,
}

pub fn char_len(s: &str) -> usize {
    s.chars().count()
}

pub fn byte_at(s: &str, char_idx: usize) -> usize {
    s.char_indices().nth(char_idx).map(|(b, _)| b).unwrap_or(s.len())
}

pub fn normalize(ranges: &mut Vec<Range<usize>>, len: usize) {
    ranges.retain_mut(|r| {
        r.end = r.end.min(len);
        r.start < r.end
    });
    ranges.sort_by_key(|r| r.start);
    let mut out: Vec<Range<usize>> = Vec::new();
    for r in ranges.drain(..) {
        match out.last_mut() {
            Some(l) if r.start <= l.end => l.end = l.end.max(r.end),
            _ => out.push(r),
        }
    }
    *ranges = out;
}

impl Block {
    pub fn is_bold_at(&self, i: usize) -> bool {
        self.bold.iter().any(|r| r.contains(&i))
    }

    /// Replaces the char range with `new`; bold runs shift, and the new text is bold when the char
    /// before the insertion point is bold and the one after is too (typing inside a bold word).
    pub fn replace(&mut self, range: Range<usize>, new: &str) {
        let len = char_len(&self.text);
        let (s, e) = (range.start.min(len), range.end.min(len));
        let inherit = s > 0 && self.is_bold_at(s - 1) && (e >= len || self.is_bold_at(e));
        let new_len = char_len(new);
        let (bs, be) = (byte_at(&self.text, s), byte_at(&self.text, e));
        self.text.replace_range(bs..be, new);
        let removed = e - s;
        let mut out: Vec<Range<usize>> = Vec::new();
        for r in self.bold.drain(..) {
            let map = |p: usize| -> usize {
                if p <= s {
                    p
                } else if p >= e {
                    p - removed + new_len
                } else {
                    s
                }
            };
            out.push(map(r.start)..map(r.end));
        }
        if inherit && new_len > 0 {
            out.push(s..s + new_len);
        }
        normalize(&mut out, char_len(&self.text));
        self.bold = out;
    }

    pub fn toggle_bold(&mut self, range: Range<usize>) {
        let len = char_len(&self.text);
        let r = range.start.min(len)..range.end.min(len);
        if r.is_empty() {
            return;
        }
        let all = r.clone().all(|i| self.is_bold_at(i));
        if all {
            let mut out = Vec::new();
            for b in self.bold.drain(..) {
                if b.end <= r.start || b.start >= r.end {
                    out.push(b);
                } else {
                    if b.start < r.start {
                        out.push(b.start..r.start);
                    }
                    if b.end > r.end {
                        out.push(r.end..b.end);
                    }
                }
            }
            self.bold = out;
        } else {
            self.bold.push(r);
        }
        normalize(&mut self.bold, len);
    }

    /// Splits at `at`: this block keeps the head, the returned block gets the tail.
    fn split_tail(&mut self, at: usize, id: u64, kind: Kind) -> Block {
        let len = char_len(&self.text);
        let at = at.min(len);
        let tail_text: String = self.text.chars().skip(at).collect();
        let mut tail_bold: Vec<Range<usize>> =
            self.bold.iter().filter(|b| b.end > at).map(|b| b.start.max(at) - at..b.end - at).collect();
        normalize(&mut tail_bold, len - at);
        let head: String = self.text.chars().take(at).collect();
        self.text = head;
        normalize(&mut self.bold, at);
        Block { id, kind, checked: false, text: tail_text, bold: tail_bold }
    }
}

/// What Backspace at block start did, so the caller can place the caret.
#[derive(Debug, PartialEq)]
pub enum MergeOutcome {
    /// Block was a heading/list: it became plain text, caret stays at 0.
    BecameText,
    /// Joined into the previous block; caret goes to `(block, char)`.
    Merged(usize, usize),
    /// First block, nothing to do.
    Nothing,
}

impl Doc {
    pub fn new(blocks: Vec<(Kind, &str)>) -> Self {
        let mut d = Doc { blocks: vec![], next_id: 1 };
        for (k, t) in blocks {
            let id = d.alloc();
            d.blocks.push(Block { id, kind: k, checked: false, text: t.into(), bold: vec![] });
        }
        d
    }

    fn alloc(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id - 1
    }

    /// Enter at `at`. An empty list item leaves the list instead of splitting. Returns the block
    /// the caret goes to (always at char 0 unless the block merely changed kind).
    pub fn split(&mut self, idx: usize, at: usize) -> usize {
        let kind = self.blocks[idx].kind;
        if kind.is_list() && self.blocks[idx].text.is_empty() {
            self.blocks[idx].kind = Kind::Text;
            return idx;
        }
        let next_kind = if kind.is_list() || kind == Kind::Code || kind == Kind::Callout { kind } else { Kind::Text };
        let id = self.alloc();
        let tail = self.blocks[idx].split_tail(at, id, next_kind);
        self.blocks.insert(idx + 1, tail);
        idx + 1
    }

    pub fn backspace_at_start(&mut self, idx: usize) -> MergeOutcome {
        if self.blocks[idx].kind != Kind::Text {
            self.blocks[idx].kind = Kind::Text;
            return MergeOutcome::BecameText;
        }
        if idx == 0 {
            return MergeOutcome::Nothing;
        }
        let cur = self.blocks.remove(idx);
        let prev = &mut self.blocks[idx - 1];
        let join = char_len(&prev.text);
        let shift = |r: &Range<usize>| r.start + join..r.end + join;
        prev.text.push_str(&cur.text);
        prev.bold.extend(cur.bold.iter().map(shift));
        let len = char_len(&prev.text);
        normalize(&mut prev.bold, len);
        MergeOutcome::Merged(idx - 1, join)
    }

    pub fn insert_after(&mut self, idx: usize, kind: Kind) -> usize {
        let id = self.alloc();
        self.blocks.insert(idx + 1, Block { id, kind, checked: false, text: String::new(), bold: vec![] });
        idx + 1
    }
}

/// Markdown the phase-01 serialiser would produce for these blocks (bold as `**`).
pub fn to_markdown(doc: &Doc) -> String {
    let mut out = String::new();
    let mut n = 0;
    for b in &doc.blocks {
        n = if b.kind == Kind::Numbered { n + 1 } else { 0 };
        let prefix = match b.kind {
            Kind::Text => String::new(),
            Kind::H1 => "# ".into(),
            Kind::H2 => "## ".into(),
            Kind::H3 => "### ".into(),
            Kind::Bullet => "- ".into(),
            Kind::Numbered => format!("{n}. "),
            Kind::Check => format!("- [{}] ", if b.checked { "x" } else { " " }),
            Kind::Callout => "> ".into(),
            Kind::Code => "```\n".into(),
        };
        out.push_str(&prefix);
        let mut cur = 0;
        for r in &b.bold {
            out.extend(b.text.chars().skip(cur).take(r.start - cur));
            out.push_str("**");
            out.extend(b.text.chars().skip(r.start).take(r.end - r.start));
            out.push_str("**");
            cur = r.end;
        }
        out.extend(b.text.chars().skip(cur));
        if b.kind == Kind::Code {
            out.push_str("\n```");
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(d: &Doc) -> Vec<String> {
        d.blocks.iter().map(|b| b.text.clone()).collect()
    }

    #[test]
    fn enter_splits_and_carries_bold_tail() {
        let mut d = Doc::new(vec![(Kind::Text, "Pay by card")]);
        d.blocks[0].toggle_bold(7..11);
        let to = d.split(0, 7);
        assert_eq!(to, 1);
        assert_eq!(texts(&d), ["Pay by ", "card"]);
        assert!(d.blocks[0].bold.is_empty());
        assert_eq!(d.blocks[1].bold, vec![0..4]);
    }

    #[test]
    fn backspace_merges_and_restores_the_pre_split_block() {
        let mut d = Doc::new(vec![(Kind::Text, "Pay by card")]);
        d.blocks[0].toggle_bold(7..11);
        let before = d.blocks[0].clone();
        d.split(0, 7);
        assert_eq!(d.backspace_at_start(1), MergeOutcome::Merged(0, 7));
        assert_eq!(d.blocks.len(), 1);
        assert_eq!(d.blocks[0].text, before.text);
        assert_eq!(d.blocks[0].bold, before.bold);
    }

    #[test]
    fn heading_becomes_text_before_it_merges() {
        let mut d = Doc::new(vec![(Kind::Text, "a"), (Kind::H1, "b")]);
        assert_eq!(d.backspace_at_start(1), MergeOutcome::BecameText);
        assert_eq!(d.blocks.len(), 2);
        assert!(matches!(d.backspace_at_start(1), MergeOutcome::Merged(0, 1)));
    }

    #[test]
    fn empty_list_item_leaves_the_list_and_lists_continue() {
        let mut d = Doc::new(vec![(Kind::Bullet, "x")]);
        d.split(0, 1);
        assert_eq!(d.blocks[1].kind, Kind::Bullet);
        d.split(1, 0);
        assert_eq!((d.blocks.len(), d.blocks[1].kind), (2, Kind::Text));
    }

    #[test]
    fn typing_shifts_runs_and_inherits_inside_bold() {
        let mut b = Block { id: 1, kind: Kind::Text, checked: false, text: "abcdef".into(), bold: vec![2..4] };
        b.replace(3..3, "X"); // inside the bold run
        assert_eq!((b.text.as_str(), b.bold.clone()), ("abcXdef", vec![2..5]));
        b.replace(0..0, "ZZ"); // before it
        assert_eq!(b.bold, vec![4..7]);
        b.replace(7..7, "!"); // right after the run: not bold
        assert_eq!(b.bold, vec![4..7]);
    }

    #[test]
    fn toggle_bold_adds_and_removes_and_markdown_roundtrips_marks() {
        let mut d = Doc::new(vec![(Kind::Text, "Thanh toán bằng thẻ")]);
        d.blocks[0].toggle_bold(0..5);
        assert_eq!(to_markdown(&d), "**Thanh** toán bằng thẻ\n");
        d.blocks[0].toggle_bold(3..8);
        assert_eq!(d.blocks[0].bold, vec![0..8], "a partly bold selection becomes fully bold");
        d.blocks[0].toggle_bold(0..8);
        assert!(d.blocks[0].bold.is_empty());
    }

    #[test]
    fn multibyte_text_is_split_on_char_boundaries() {
        let mut d = Doc::new(vec![(Kind::Text, "thẻ ✓")]);
        d.split(0, 2);
        assert_eq!(texts(&d), ["th", "ẻ ✓"]);
    }
}
