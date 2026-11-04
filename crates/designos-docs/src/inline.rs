//! Inline content: structure is read from the CommonMark parser's events, but all text is taken from the source
//! itself, so nothing the parser normalises (escapes, entities, line breaks) can be lost.

use crate::model::Inline;
use pulldown_cmark::{Event, LinkType, Tag};
use std::ops::Range;

pub(crate) type Ev<'a> = (Event<'a>, Range<usize>);

/// Index of the `End` that closes the `Start` at `start`.
pub(crate) fn matching_end(events: &[Ev], start: usize) -> usize {
    let mut depth = 0usize;
    for (i, (ev, _)) in events.iter().enumerate().skip(start) {
        match ev {
            Event::Start(_) => depth += 1,
            Event::End(_) => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
            _ => {}
        }
    }
    events.len().saturating_sub(1)
}

/// Inline nodes for `events`, which lie inside `src[lo..hi]`. The text between structured nodes becomes `Text`.
pub(crate) fn parse_inlines(events: &[Ev], src: &str, lo: usize, hi: usize) -> Vec<Inline> {
    let mut out = Vec::new();
    let mut cursor = lo;
    let mut i = 0;
    while i < events.len() {
        let (ev, range) = &events[i];
        let in_bounds = range.start >= cursor && range.end <= hi && range.start <= range.end;
        match ev {
            Event::Start(tag) => {
                let end = matching_end(events, i);
                if in_bounds {
                    if let Some(node) = structured(tag, &events[i + 1..end], range, src) {
                        push_text(&mut out, &src[cursor..range.start]);
                        out.push(node);
                        cursor = range.end;
                    }
                }
                i = end + 1;
            }
            Event::Code(code) if in_bounds && src[range.clone()].starts_with('`') && src[range.clone()].ends_with('`') => {
                push_text(&mut out, &src[cursor..range.start]);
                out.push(Inline::Code(code.to_string()));
                cursor = range.end;
                i += 1;
            }
            _ => i += 1,
        }
    }
    if cursor < hi {
        push_text(&mut out, &src[cursor..hi]);
    }
    out
}

fn push_text(out: &mut Vec<Inline>, text: &str) {
    if !text.is_empty() {
        out.push(Inline::Text(text.to_string()));
    }
}

/// A node for `tag` when its source has the delimiters we expect; otherwise `None` and the whole span stays text.
fn structured(tag: &Tag, inner: &[Ev], range: &Range<usize>, src: &str) -> Option<Inline> {
    let text = src.get(range.clone())?;
    match tag {
        Tag::Strong => {
            let marker = ["**", "__"].into_iter().find(|m| text.starts_with(m) && text.ends_with(m) && text.len() >= 4)?;
            non_empty(Inline::Bold(parse_inlines(inner, src, range.start + marker.len(), range.end - marker.len())))
        }
        Tag::Emphasis => {
            let marker = ['*', '_'].into_iter().find(|m| text.starts_with(*m) && text.ends_with(*m) && text.len() >= 2)?;
            non_empty(Inline::Italic(parse_inlines(inner, src, range.start + marker.len_utf8(), range.end - marker.len_utf8())))
        }
        Tag::Link { link_type: LinkType::Inline, .. } => {
            if !text.starts_with('[') {
                return None;
            }
            let text_end = inner.iter().map(|(_, r)| r.end).max().unwrap_or(range.start + 1).max(range.start + 1);
            let tail = src.get(text_end..range.end)?;
            let target = tail.strip_prefix("](")?.strip_suffix(')')?;
            Some(Inline::Link { content: parse_inlines(inner, src, range.start + 1, text_end), target: target.to_string() })
        }
        _ => None,
    }
}

/// Emphasis with nothing inside (`****`) is not emphasis when written back, so it stays plain text.
fn non_empty(node: Inline) -> Option<Inline> {
    match &node {
        Inline::Bold(c) | Inline::Italic(c) if c.is_empty() => None,
        _ => Some(node),
    }
}

/// Canonical Markdown for inline nodes.
pub(crate) fn render_inlines(nodes: &[Inline]) -> String {
    render(nodes, false)
}

/// Like `render_inlines`, for a table cell: a `|` that is not already escaped is written `\|`, in text, code and
/// link targets, because a bare pipe ends the cell.
pub(crate) fn render_cell_inlines(nodes: &[Inline]) -> String {
    render(nodes, true)
}

fn render(nodes: &[Inline], cell: bool) -> String {
    let pipes = |s: &str| if cell { escape_pipes(s) } else { s.to_string() };
    let mut out = String::new();
    for node in nodes {
        match node {
            Inline::Text(t) => out.push_str(&pipes(t)),
            Inline::Bold(c) => {
                let inner = render(c, cell);
                let mark = emphasis_mark(&inner).to_string().repeat(2);
                out.push_str(&format!("{mark}{inner}{mark}"));
            }
            Inline::Italic(c) => {
                let inner = render(c, cell);
                let mark = emphasis_mark(&inner);
                out.push_str(&format!("{mark}{inner}{mark}"));
            }
            Inline::Code(c) => out.push_str(&render_code_span(&pipes(c))),
            Inline::Link { content, target } => {
                out.push('[');
                out.push_str(&render(content, cell));
                out.push_str("](");
                out.push_str(&pipes(target));
                out.push(')');
            }
        }
    }
    out
}

/// `|` becomes `\|` unless a backslash already escapes it.
pub(crate) fn escape_pipes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut backslashes = 0usize;
    for c in s.chars() {
        if c == '|' && backslashes.is_multiple_of(2) {
            out.push('\\');
        }
        out.push(c);
        backslashes = if c == '\\' { backslashes + 1 } else { 0 };
    }
    out
}

/// `*` unless the emphasised text itself starts or ends with a `*` (typed as `_..._`), where `*` would run into it.
fn emphasis_mark(inner: &str) -> char {
    if inner.starts_with('*') || inner.ends_with('*') {
        '_'
    } else {
        '*'
    }
}

fn render_code_span(code: &str) -> String {
    let longest_run = code.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let ticks = "`".repeat(longest_run + 1);
    let all_space = code.chars().all(|c| c == ' ');
    let pad = code.starts_with('`') || code.ends_with('`') || (code.starts_with(' ') && code.ends_with(' ') && !all_space);
    if pad {
        format!("{ticks} {code} {ticks}")
    } else {
        format!("{ticks}{code}{ticks}")
    }
}
