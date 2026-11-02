//! Markdown to blocks. pulldown-cmark finds where each top-level block starts and ends and what it is; this
//! module never reconstructs a block from events. Each block keeps its source slice, and the whitespace between
//! blocks is kept as the next block's gap, so the file is the concatenation of what was read. Anything that does
//! not fit the v1 block types exactly becomes `Raw`.

use crate::error::Result;
use crate::front_matter::FrontMatter;
use crate::inline::{matching_end, parse_inlines, Ev};
use crate::markdown_structures::{callout, list, table};
use crate::model::{Block, BlockKind, Page};
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag};

fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS
}

pub(crate) fn events(src: &str) -> Vec<Ev<'_>> {
    Parser::new_ext(src, options()).into_offset_iter().collect()
}

pub(crate) fn parse_page(text: &str) -> Result<Page> {
    let (front, body) = FrontMatter::split(text)?;
    let (blocks, tail) = parse_blocks(body);
    Ok(Page { front, blocks, tail })
}

fn parse_blocks(src: &str) -> (Vec<Block>, String) {
    let evs = events(src);
    let mut blocks: Vec<Block> = Vec::new();
    let mut cursor = 0usize;
    let mut gap_for_next = String::new();
    let mut i = 0;
    while i < evs.len() {
        let last = if matches!(evs[i].0, Event::Start(_)) { matching_end(&evs, i) } else { i };
        let range = &evs[i].1;
        let content_start = range.start.max(cursor);
        let end = trim_newlines(src, content_start, range.end.max(content_start));
        // The block's text includes the indentation before it (it matters for indented code and raw blocks).
        let line_start = src[..content_start].rfind('\n').map_or(0, |n| n + 1).max(cursor);
        let start = if src[line_start..content_start].chars().all(|c| c == ' ' || c == '\t') { line_start } else { content_start };
        // Text between blocks: blank lines, or content the parser consumed without an event (link definitions).
        gap_for_next = split_gap(&src[cursor..start], gap_for_next, &mut blocks);
        let source = &src[start..end];
        let kind = classify(&evs[i..=last], src, content_start, end, start);
        blocks.push(Block::parsed(kind, std::mem::take(&mut gap_for_next), source.to_string()));
        cursor = end;
        i = last + 1;
    }
    let leftover = split_gap(&src[cursor..], gap_for_next, &mut blocks);
    (blocks, leftover)
}

fn trim_newlines(src: &str, start: usize, mut end: usize) -> usize {
    while end > start && matches!(src.as_bytes()[end - 1], b'\n' | b'\r') {
        end -= 1;
    }
    end
}

/// Turns the text between two blocks into whitespace and `Raw` blocks for any non-blank lines in it. Returns the
/// whitespace that precedes whatever comes next. `carry` is whitespace already collected before this text.
fn split_gap(gap: &str, carry: String, blocks: &mut Vec<Block>) -> String {
    let mut pending = carry;
    let mut run = String::new();
    let flush = |run: &mut String, pending: &mut String, blocks: &mut Vec<Block>| {
        if run.is_empty() {
            return;
        }
        let kept = run.trim_end_matches(['\n', '\r']).len();
        let text = run[..kept].to_string();
        let newline = run[kept..].to_string();
        blocks.push(Block::parsed(BlockKind::Raw(text.clone()), std::mem::take(pending), text));
        *pending = newline;
        run.clear();
    };
    for line in gap.split_inclusive('\n') {
        if line.trim().is_empty() {
            flush(&mut run, &mut pending, blocks);
            pending.push_str(line);
        } else {
            run.push_str(line);
        }
    }
    flush(&mut run, &mut pending, blocks);
    pending
}

/// `start` is where the block's own text begins; `raw_start` includes the indentation before it.
fn classify(evs: &[Ev], src: &str, start: usize, end: usize, raw_start: usize) -> BlockKind {
    let raw = || BlockKind::Raw(src[raw_start..end].to_string());
    let Some((first, _)) = evs.first() else { return raw() };
    let inner = if evs.len() >= 2 { &evs[1..evs.len() - 1] } else { &[][..] };
    let typed = match first {
        Event::Start(Tag::Paragraph) => paragraph(inner, src, start, end),
        Event::Start(Tag::Heading { level, .. }) => heading(*level, src, start, end, inner),
        Event::Start(Tag::List(first_number)) => list(*first_number, inner, src),
        Event::Start(Tag::BlockQuote(_)) => callout(&src[start..end]),
        Event::Start(Tag::Table(aligns)) => table(aligns, inner, src),
        Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(_))) => Some(code(&src[start..end], inner)),
        _ => None,
    };
    typed.unwrap_or_else(raw)
}

fn paragraph(inner: &[Ev], src: &str, start: usize, end: usize) -> Option<BlockKind> {
    if let Some(embed) = frame_embed(inner, src) {
        return Some(embed);
    }
    let (lo, hi) = trimmed_span(src, start, end);
    Some(BlockKind::Paragraph(parse_inlines(inner, src, lo, hi)))
}

/// `src[start..end]` without its leading and trailing whitespace, as absolute offsets.
pub(crate) fn trimmed_span(src: &str, start: usize, end: usize) -> (usize, usize) {
    let text = &src[start..end];
    let lo = start + (text.len() - text.trim_start().len());
    (lo, (start + text.trim_end().len()).max(lo))
}

/// `![name](designos:file#node)` and nothing else in the paragraph.
fn frame_embed(inner: &[Ev], src: &str) -> Option<BlockKind> {
    let [(Event::Start(Tag::Image { dest_url, .. }), range), .., (Event::End(_), _)] = inner else { return None };
    if matching_end(inner, 0) != inner.len() - 1 {
        return None;
    }
    let rest = dest_url.strip_prefix("designos:")?;
    let (file_id, node_id) = rest.split_once('#')?;
    let id_ok = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':'));
    if !id_ok(file_id) || !id_ok(node_id) {
        return None;
    }
    let text = src.get(range.clone())?;
    let name = text.strip_prefix("![")?.strip_suffix(&format!("](designos:{rest})"))?;
    Some(BlockKind::FrameEmbed { name: name.to_string(), file_id: file_id.to_string(), node_id: node_id.to_string() })
}

fn heading(level: HeadingLevel, src: &str, start: usize, end: usize, inner: &[Ev]) -> Option<BlockKind> {
    let level = match level {
        HeadingLevel::H1 => 1u8,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        _ => return None,
    };
    let text = &src[start..end];
    let marks = "#".repeat(level as usize);
    let after = text.strip_prefix(&marks)?;
    if !after.is_empty() && !after.starts_with([' ', '\t']) {
        return None; // setext heading, or `#` followed by more `#`
    }
    let lo = start + marks.len() + (after.len() - after.trim_start_matches([' ', '\t']).len());
    let mut hi = start + text.trim_end().len();
    // Optional closing `###` preceded by a space.
    let body = src[lo.min(hi)..hi].trim_end_matches('#');
    let closed = lo + body.len() < hi && (body.is_empty() || body.ends_with([' ', '\t']));
    if closed {
        hi = lo + body.trim_end().len();
    }
    let lo = lo.min(hi);
    Some(BlockKind::Heading { level, content: parse_inlines(inner, src, lo, hi) })
}

/// `block` is the whole fenced block; its info string is taken from the opening line as written (the parser
/// would hand back an unescaped copy that no longer reads the same).
fn code(block: &str, inner: &[Ev]) -> BlockKind {
    let mut code = String::new();
    for (ev, _) in inner {
        if let Event::Text(t) = ev {
            code.push_str(t);
        }
    }
    if code.ends_with('\n') {
        code.pop();
    }
    let opening = block.lines().next().unwrap_or("").trim_start();
    let fence_char = opening.chars().next().unwrap_or('`');
    let info = opening.trim_start_matches(fence_char).trim().to_string();
    if info.eq_ignore_ascii_case("mermaid") {
        BlockKind::Mermaid { source: code }
    } else {
        BlockKind::Code { info, code }
    }
}
