//! The container blocks of the model (lists, callouts, tables), read from the parser's events. Each returns `None`
//! when the block is not exactly the v1 shape, and the caller keeps it as a `Raw` block.

use crate::inline::{matching_end, parse_inlines, Ev};
use crate::markdown::{events, paragraph, tidy_continuations, trimmed_span};
use crate::model::{Align, BlockKind, ChecklistItem, Inline, ListItem};
use pulldown_cmark::{Alignment, Event, Tag};
use std::ops::Range;

struct ParsedItem {
    check: Option<bool>,
    content: Vec<Inline>,
    children: Vec<BlockKind>,
}

/// A list (and, through its items, the lists nested in it). Items hold their own text, nested lists and further
/// paragraphs; anything else inside an item (code, quotes, tables) keeps the whole list `Raw`.
pub(crate) fn list(first_number: Option<u64>, inner: &[Ev], src: &str, range: &Range<usize>) -> Option<BlockKind> {
    let mut items: Vec<ParsedItem> = Vec::new();
    let mut cursor = range.start;
    let mut i = 0;
    while i < inner.len() {
        let Event::Start(Tag::Item) = inner[i].0 else { return None };
        let end = matching_end(inner, i);
        let item = &inner[i].1;
        // Between items only whitespace may lie: anything else (a link definition) would vanish on a rewrite.
        if !src.get(cursor..item.start.max(cursor))?.trim().is_empty() {
            return None;
        }
        cursor = item.end.max(cursor);
        items.push(list_item(&inner[i + 1..end], item, src)?);
        i = end + 1;
    }
    if !src.get(cursor..range.end.max(cursor))?.trim().is_empty() {
        return None;
    }
    if items.is_empty() {
        return None;
    }
    let checks = items.iter().filter(|i| i.check.is_some()).count();
    match (first_number, checks) {
        (None, 0) => Some(BlockKind::BulletList(items.into_iter().map(|i| ListItem { content: i.content, children: i.children }).collect())),
        (None, n) if n == items.len() => Some(BlockKind::Checklist(
            items.into_iter().map(|i| ChecklistItem { checked: i.check.unwrap_or(false), content: i.content, children: i.children }).collect(),
        )),
        (Some(start), 0) => {
            Some(BlockKind::NumberedList { start, items: items.into_iter().map(|i| ListItem { content: i.content, children: i.children }).collect() })
        }
        _ => None,
    }
}

fn list_item(inner: &[Ev], item: &Range<usize>, src: &str) -> Option<ParsedItem> {
    // The task marker leads a tight item and sits inside the paragraph of a loose one (blank lines between items).
    let marker = |evs: &[Ev]| match evs.first() {
        Some((Event::TaskListMarker(done), _)) => Some(*done),
        _ => None,
    };
    let mut check = marker(inner);
    let rest = if check.is_some() { &inner[1..] } else { inner };

    // Where the item's own text ends and its child blocks begin.
    let (content_events, content_end, after): (&[Ev], usize, &[Ev]) = match rest.first() {
        Some((Event::Start(Tag::Paragraph), range)) => {
            let end = matching_end(rest, 0);
            let mut content = &rest[1..end];
            if check.is_none() {
                if let Some(done) = marker(content) {
                    check = Some(done);
                    content = &content[1..];
                }
            }
            (content, range.end, &rest[end + 1..])
        }
        _ => {
            let boundary = rest.iter().position(|(e, _)| matches!(e, Event::Start(t) if is_block(t))).unwrap_or(rest.len());
            let content_end = rest.get(boundary).map_or(item.end, |(_, r)| r.start);
            (&rest[..boundary], content_end, &rest[boundary..])
        }
    };
    let rest = after;

    let full = src.get(item.clone())?;
    // The item's range may begin with the indentation in front of its marker.
    let text = full.trim_start_matches([' ', '\t']);
    let indent = full.len() - text.len();
    let after_marker = strip_item_marker(text)?;
    let mut lo = item.start + indent + (text.len() - after_marker.len());
    if check.is_some() {
        if !src.get(lo..)?.starts_with('[') {
            return None;
        }
        lo += 3; // `[ ]` / `[x]`
    }
    let bound = content_end.min(item.end).max(lo);
    let span = src.get(lo..bound)?;
    lo += span.len() - span.trim_start_matches([' ', '\t']).len();
    let hi = lo.max(bound - (span.len() - span.trim_end().len()));
    let content = if lo >= hi { Vec::new() } else { tidy_continuations(parse_inlines(content_events, src, lo, hi))? };
    // An item whose first line is empty but whose text starts on the next line cannot be written back the same way
    // (`- ` and then the text would read as an item holding a paragraph), so such a list stays raw.
    if !content.is_empty() && after_marker.starts_with(['\n', '\r']) {
        return None;
    }

    // Text the parser consumed without an event (a link definition inside the item) would be lost on a rewrite.
    let mut children = Vec::new();
    let mut cursor = content_end.min(item.end);
    let mut i = 0;
    while i < rest.len() {
        let Event::Start(tag) = &rest[i].0 else { return None };
        let end = matching_end(rest, i);
        let child_inner = &rest[i + 1..end];
        let range = &rest[i].1;
        if !src.get(cursor..range.start.max(cursor))?.trim().is_empty() {
            return None;
        }
        cursor = range.end.max(cursor);
        match tag {
            Tag::List(first) => children.push(list(*first, child_inner, src, range)?),
            Tag::Paragraph => children.push(paragraph(child_inner, src, range.start, range.end)?),
            _ => return None,
        }
        i = end + 1;
    }
    if !src.get(cursor..item.end.max(cursor))?.trim().is_empty() {
        return None;
    }
    Some(ParsedItem { check, content, children })
}

fn is_block(tag: &Tag) -> bool {
    matches!(
        tag,
        Tag::Paragraph
            | Tag::Heading { .. }
            | Tag::BlockQuote(_)
            | Tag::CodeBlock(_)
            | Tag::HtmlBlock
            | Tag::List(_)
            | Tag::Table(_)
            | Tag::FootnoteDefinition(_)
    )
}

/// The item text after its bullet (`-`, `*`, `+`) or number (`1.`, `1)`).
fn strip_item_marker(text: &str) -> Option<&str> {
    let after = if let Some(rest) = text.strip_prefix(['-', '*', '+']) {
        rest
    } else {
        let digits = text.len() - text.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        if digits == 0 {
            return None;
        }
        text[digits..].strip_prefix(['.', ')'])?
    };
    Some(after.trim_start_matches([' ', '\t']))
}

/// `> [!kind]` then `> ` lines forming exactly one paragraph.
pub(crate) fn callout(block: &str) -> Option<BlockKind> {
    let mut lines = block.split('\n').map(|l| l.trim_end_matches('\r'));
    let head = lines.next()?.strip_prefix('>')?.trim();
    let kind = head.strip_prefix("[!")?.strip_suffix(']')?;
    if kind.is_empty() || !kind.starts_with(|c: char| c.is_ascii_alphabetic()) || !kind.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return None;
    }
    let mut body = Vec::new();
    for line in lines {
        let rest = line.strip_prefix('>')?;
        body.push(rest.strip_prefix(' ').unwrap_or(rest));
    }
    let body = body.join("\n");
    let evs = events(&body);
    let content = match evs.as_slice() {
        [] => Vec::new(),
        [(Event::Start(Tag::Paragraph), _), inner @ .., (Event::End(_), _)] if matching_end(&evs, 0) == evs.len() - 1 => {
            let (lo, hi) = trimmed_span(&body, 0, body.len());
            tidy_continuations(parse_inlines(inner, &body, lo, hi))?
        }
        _ => return None,
    };
    Some(BlockKind::Callout { kind: kind.to_string(), body: content })
}

pub(crate) fn table(aligns: &[Alignment], inner: &[Ev], src: &str) -> Option<BlockKind> {
    let mut header: Option<Vec<Vec<Inline>>> = None;
    let mut rows: Vec<Vec<Vec<Inline>>> = Vec::new();
    let mut i = 0;
    while i < inner.len() {
        let end = matching_end(inner, i);
        match inner[i].0 {
            Event::Start(Tag::TableHead) => header = Some(cells(&inner[i + 1..end], &inner[i].1, src)?),
            Event::Start(Tag::TableRow) => rows.push(cells(&inner[i + 1..end], &inner[i].1, src)?),
            _ => return None,
        }
        i = end + 1;
    }
    let header = header?;
    if header.len() != aligns.len() || rows.iter().any(|r| r.len() != header.len()) {
        return None;
    }
    let aligns = aligns
        .iter()
        .map(|a| match a {
            Alignment::None => Align::None,
            Alignment::Left => Align::Left,
            Alignment::Center => Align::Center,
            Alignment::Right => Align::Right,
        })
        .collect();
    Some(BlockKind::Table { aligns, header, rows })
}

/// The cells of one row. The parser drops cells beyond the header's width (and splits a code span at a bare `|`),
/// so a row whose line has text after its last cell is not representable: `None` keeps the table raw rather than
/// losing that text the first time a cell is edited.
fn cells(row: &[Ev], row_range: &Range<usize>, src: &str) -> Option<Vec<Vec<Inline>>> {
    let mut out = Vec::new();
    let mut last_end = row_range.start;
    let mut i = 0;
    while i < row.len() {
        let Event::Start(Tag::TableCell) = row[i].0 else { return None };
        let end = matching_end(row, i);
        let range = &row[i].1;
        let text = src.get(range.clone())?;
        // The cell's own range is its text between the pipes; trim the padding around it.
        let lo = range.start + (text.len() - text.trim_start().len());
        let hi = range.start + text.trim_end().len();
        out.push(if lo >= hi { Vec::new() } else { parse_inlines(&row[i + 1..end], src, lo, hi) });
        last_end = range.end;
        i = end + 1;
    }
    let after = src.get(last_end..row_range.end.max(last_end))?;
    let rest_of_line = after.split('\n').next().unwrap_or("");
    if !rest_of_line.trim_matches(['|', ' ', '\t', '\r']).is_empty() {
        return None;
    }
    Some(out)
}
