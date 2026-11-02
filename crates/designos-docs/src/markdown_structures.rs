//! The container blocks of the model (lists, callouts, tables), read from the parser's events. Each returns `None`
//! when the block is not exactly the v1 shape, and the caller keeps it as a `Raw` block.

use crate::inline::{matching_end, parse_inlines, Ev};
use crate::markdown::{events, trimmed_span};
use crate::model::{Align, BlockKind, ChecklistItem, Inline};
use pulldown_cmark::{Alignment, Event, Tag};

type Item = (Option<bool>, Vec<Inline>);

pub(crate) fn list(first_number: Option<u64>, inner: &[Ev], src: &str) -> Option<BlockKind> {
    let mut items: Vec<Item> = Vec::new();
    let mut i = 0;
    while i < inner.len() {
        let Event::Start(Tag::Item) = inner[i].0 else { return None };
        let end = matching_end(inner, i);
        items.push(list_item(&inner[i + 1..end], &inner[i].1, src)?);
        i = end + 1;
    }
    if items.is_empty() {
        return None;
    }
    let checks = items.iter().filter(|(c, _)| c.is_some()).count();
    match (first_number, checks) {
        (None, 0) => Some(BlockKind::BulletList(items.into_iter().map(|(_, c)| c).collect())),
        (None, n) if n == items.len() => {
            Some(BlockKind::Checklist(items.into_iter().map(|(c, content)| ChecklistItem { checked: c.unwrap_or(false), content }).collect()))
        }
        (Some(start), 0) => Some(BlockKind::NumberedList { start, items: items.into_iter().map(|(_, c)| c).collect() }),
        _ => None,
    }
}

/// One list item holding a single run of inline content, optionally with a task marker.
fn list_item(inner: &[Ev], item: &std::ops::Range<usize>, src: &str) -> Option<Item> {
    // The task marker leads a tight item and sits inside the paragraph of a loose one (blank lines between items).
    let marker = |evs: &[Ev]| match evs.first() {
        Some((Event::TaskListMarker(done), _)) => Some(*done),
        _ => None,
    };
    let mut check = marker(inner);
    let mut inner = if check.is_some() { &inner[1..] } else { inner };
    if let Some((Event::Start(Tag::Paragraph), _)) = inner.first() {
        if matching_end(inner, 0) == inner.len() - 1 {
            inner = &inner[1..inner.len() - 1];
            if check.is_none() {
                check = marker(inner);
                if check.is_some() {
                    inner = &inner[1..];
                }
            }
        }
    }
    if inner.iter().any(|(e, _)| matches!(e, Event::Start(t) if is_block(t))) {
        return None;
    }
    let text = src.get(item.clone())?;
    let after_marker = strip_item_marker(text)?;
    let mut lo = item.start + (text.len() - after_marker.len());
    if check.is_some() {
        if !src.get(lo..)?.starts_with('[') {
            return None;
        }
        lo += 3; // `[ ]` / `[x]`
    }
    let hi = item.start + text.trim_end().len();
    let gap = src.get(lo..hi.max(lo))?;
    lo += gap.len() - gap.trim_start_matches([' ', '\t']).len();
    if lo >= hi {
        return Some((check, Vec::new()));
    }
    Some((check, parse_inlines(inner, src, lo, hi)))
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
            parse_inlines(inner, &body, lo, hi)
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
            Event::Start(Tag::TableHead) => header = Some(cells(&inner[i + 1..end], src)?),
            Event::Start(Tag::TableRow) => rows.push(cells(&inner[i + 1..end], src)?),
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

fn cells(row: &[Ev], src: &str) -> Option<Vec<Vec<Inline>>> {
    let mut out = Vec::new();
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
        i = end + 1;
    }
    Some(out)
}
