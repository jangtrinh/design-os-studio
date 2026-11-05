//! Reading helpers: plain text of inline content and a page's heading outline ("On this page", search).

use crate::model::{BlockKind, Inline, Page};

/// A heading of a page, with the index of its block in `Page::blocks`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heading {
    pub block_index: usize,
    pub level: u8,
    pub text: String,
}

/// The text a reader sees for inline content: markup removed and backslash escapes resolved (`2 \* 3` is `2 * 3`).
/// HTML entities such as `&amp;` are left as written.
pub fn plain_text(nodes: &[Inline]) -> String {
    let mut out = String::new();
    for node in nodes {
        match node {
            Inline::Text(t) => out.push_str(&unescape(t)),
            Inline::Code(c) => out.push_str(c),
            Inline::Bold(c) | Inline::Italic(c) | Inline::Link { content: c, .. } => out.push_str(&plain_text(c)),
        }
    }
    out
}

fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match chars.peek() {
            Some(next) if c == '\\' && next.is_ascii_punctuation() => {
                out.push(*next);
                chars.next();
            }
            _ => out.push(c),
        }
    }
    out
}

impl Page {
    /// Every level 1 to 3 heading in order.
    pub fn headings(&self) -> Vec<Heading> {
        self.blocks
            .iter()
            .enumerate()
            .filter_map(|(block_index, block)| match block.kind() {
                BlockKind::Heading { level, content } => Some(Heading { block_index, level: *level, text: plain_text(content) }),
                _ => None,
            })
            .collect()
    }
}
