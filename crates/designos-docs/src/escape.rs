//! Plain text typed by a person or an agent to Markdown source that reads back as exactly that text.
//!
//! `Inline::Text` holds Markdown source, so text typed into a block must be escaped before it is stored in one:
//! otherwise `- not a list` becomes a list and `# tag` a heading when the page is read again. [`plain_text`]
//! (crate::plain_text) is the inverse.
//!
//! What is normalised, because Markdown cannot keep it: each line loses its leading and trailing spaces and tabs,
//! blank lines disappear (use [`paragraphs_from_plain`] to keep paragraphs apart), and NUL characters are dropped.
//! Headings and table cells hold one line, so their newlines become spaces.

use crate::model::{BlockKind, Inline};

/// The block the text is going into; it decides whether newlines survive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextContext {
    Paragraph,
    Heading,
    ListItem,
    TableCell,
    CalloutBody,
}

impl TextContext {
    fn single_line(self) -> bool {
        matches!(self, TextContext::Heading | TextContext::TableCell)
    }
}

/// The Markdown source for `text` typed into a block of kind `ctx`.
pub fn escape_plain(text: &str, ctx: TextContext) -> String {
    let lines = clean_lines(text);
    let separator = if ctx.single_line() { " " } else { "\n" };
    lines.iter().map(|l| escape_line(l)).collect::<Vec<_>>().join(separator)
}

/// `escape_plain` as inline content (empty for blank text), ready for a block's `content`, `body` or cell.
pub fn plain_inlines(text: &str, ctx: TextContext) -> Vec<Inline> {
    let escaped = escape_plain(text, ctx);
    if escaped.is_empty() {
        Vec::new()
    } else {
        vec![Inline::Text(escaped)]
    }
}

/// One paragraph per run of non-blank lines.
pub fn paragraphs_from_plain(text: &str) -> Vec<BlockKind> {
    let normalised = text.replace("\r\n", "\n").replace('\r', "\n").replace('\0', "");
    let mut paragraphs: Vec<Vec<&str>> = vec![Vec::new()];
    for line in normalised.split('\n') {
        if line.trim_matches([' ', '\t']).is_empty() {
            if paragraphs.last().is_some_and(|p| !p.is_empty()) {
                paragraphs.push(Vec::new());
            }
        } else if let Some(last) = paragraphs.last_mut() {
            last.push(line);
        }
    }
    paragraphs.into_iter().filter(|p| !p.is_empty()).map(|p| BlockKind::Paragraph(plain_inlines(&p.join("\n"), TextContext::Paragraph))).collect()
}

fn clean_lines(text: &str) -> Vec<String> {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\0', "")
        .split('\n')
        .map(|l| l.trim_matches([' ', '\t']).to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Whether a line, read at the start of a line, would begin or end a block (heading, quote, bullet, number, rule,
/// setext underline, fence).
pub(crate) fn line_starts_block(line: &str) -> bool {
    let digits = line.len() - line.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let rule = line.len() >= 3 && line.starts_with(['*', '_']) && line.chars().all(|c| line.starts_with(c));
    line.starts_with(['#', '>', '-', '+', '=', '<'])
        || (digits > 0 && line[digits..].starts_with(['.', ')']))
        || line.starts_with("~~~")
        || line.starts_with("```")
        || line.starts_with('|')
        || line == "*"
        || line.starts_with("* ")
        || rule
}

fn escape_line(line: &str) -> String {
    let mut out = String::with_capacity(line.len() + 4);
    // A character at the start of a line that would begin a block: heading, quote, bullet, setext underline.
    let mut chars = line.chars().peekable();
    if let Some(&first) = chars.peek() {
        if matches!(first, '#' | '>' | '-' | '+' | '=') {
            out.push('\\');
        }
    }
    let digits = line.len() - line.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let numbered = digits > 0 && line[digits..].starts_with(['.', ')']);
    let tilde_fence = line.starts_with("~~~");
    for (i, c) in line.char_indices() {
        let special = matches!(c, '\\' | '*' | '_' | '`' | '[' | ']' | '<' | '&' | '|') || (numbered && i == digits) || (tilde_fence && i == 0);
        if special {
            out.push('\\');
        }
        out.push(c);
    }
    out
}
