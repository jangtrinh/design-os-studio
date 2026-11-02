//! Blocks to Markdown. Unedited blocks are written from their kept source; everything else (and everything in
//! canonical mode) is written the way Studio writes new content.

use crate::inline::render_inlines;
use crate::model::{Align, BlockKind, Inline, Page};

pub(crate) fn render_page(page: &Page, canonical: bool) -> String {
    let mut out = String::new();
    if let Some(front) = &page.front {
        out.push_str(front.raw());
    }
    let mut prev_text: Option<String> = None;
    for (i, block) in page.blocks.iter().enumerate() {
        let gap = if canonical { None } else { block.gap.as_deref() };
        let text = match (&block.source, canonical) {
            (Some(source), false) => source.clone(),
            _ => render_kind(block.kind(), list_marker_to_avoid(block.kind(), prev_text.as_deref())),
        };
        let mut lead = gap.unwrap_or(canonical_gap(i, page.front.is_some()));
        if gap.is_none() && i == 0 && page.front.is_none() && text.starts_with("---") {
            lead = "\n"; // generated text that opens with `---` would be read as front matter
        }
        out.push_str(lead);
        out.push_str(&text);
        prev_text = Some(text);
    }
    if canonical && !page.blocks.is_empty() {
        out.push('\n');
    } else {
        out.push_str(&page.tail);
    }
    out
}

/// A blank line between blocks; one after the front matter; nothing before the first block of a page without it.
fn canonical_gap(index: usize, has_front_matter: bool) -> &'static str {
    match (index, has_front_matter) {
        (0, false) => "",
        (0, true) => "\n",
        _ => "\n\n",
    }
}

#[derive(PartialEq)]
enum ListFamily {
    Bullet,
    Numbered,
}

/// Two lists that follow each other must not read as one: a list directly after another list of the same family
/// uses the other bullet (`*`) or delimiter (`)`) when the first one ends up with the default.
fn list_marker_to_avoid(kind: &BlockKind, prev_text: Option<&str>) -> Option<char> {
    let family = match kind {
        BlockKind::BulletList(_) | BlockKind::Checklist(_) => ListFamily::Bullet,
        BlockKind::NumberedList { .. } => ListFamily::Numbered,
        _ => return None,
    };
    match leading_list_marker(prev_text?) {
        Some((prev_family, marker)) if prev_family == family => Some(marker),
        _ => None,
    }
}

/// The list marker a block's text starts with: the bullet character, or the `.` / `)` after the number.
fn leading_list_marker(text: &str) -> Option<(ListFamily, char)> {
    let t = text.trim_start();
    let first = t.chars().next()?;
    let (family, marker, rest) = if matches!(first, '-' | '*' | '+') {
        (ListFamily::Bullet, first, &t[1..])
    } else {
        let digits = t.len() - t.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        let delimiter = t[digits..].chars().next().filter(|c| digits > 0 && matches!(c, '.' | ')'))?;
        (ListFamily::Numbered, delimiter, &t[digits + 1..])
    };
    (rest.is_empty() || rest.starts_with([' ', '\t', '\n', '\r'])).then_some((family, marker))
}

pub(crate) fn render_kind(kind: &BlockKind, avoid_marker: Option<char>) -> String {
    let bullet = if avoid_marker == Some('-') { "* " } else { "- " };
    let delimiter = if avoid_marker == Some('.') { ')' } else { '.' };
    match kind {
        BlockKind::Heading { level, content } => {
            let marks = "#".repeat((*level).clamp(1, 3) as usize);
            let mut text = render_inlines(content);
            // A trailing run of `#` would read back as a closing sequence and vanish.
            if text.ends_with('#') && !text.ends_with("\\#") {
                text.pop();
                text.push_str("\\#");
            }
            if text.is_empty() {
                marks
            } else {
                format!("{marks} {text}")
            }
        }
        BlockKind::Paragraph(content) => render_inlines(content),
        BlockKind::BulletList(items) => items.iter().map(|c| item_line(bullet, c)).collect::<Vec<_>>().join("\n"),
        BlockKind::NumberedList { start, items } => {
            items.iter().enumerate().map(|(i, c)| item_line(&format!("{}{delimiter} ", start + i as u64), c)).collect::<Vec<_>>().join("\n")
        }
        BlockKind::Checklist(items) => {
            items.iter().map(|item| item_line(&format!("{bullet}[{}] ", if item.checked { 'x' } else { ' ' }), &item.content)).collect::<Vec<_>>().join("\n")
        }
        BlockKind::Callout { kind, body } => {
            let mut out = format!("> [!{kind}]");
            if !body.is_empty() {
                for line in render_inlines(body).split('\n') {
                    out.push_str("\n> ");
                    out.push_str(line);
                }
            }
            out
        }
        BlockKind::Table { aligns, header, rows } => render_table(aligns, header, rows),
        BlockKind::FrameEmbed { name, file_id, node_id } => format!("![{name}](designos:{file_id}#{node_id})"),
        BlockKind::Mermaid { source } => fenced("mermaid", source),
        BlockKind::Code { info, code } => fenced(info, code),
        BlockKind::Raw(text) => text.clone(),
    }
}

fn item_line(marker: &str, content: &[Inline]) -> String {
    format!("{marker}{}", render_inlines(content))
}

fn render_table(aligns: &[Align], header: &[Vec<Inline>], rows: &[Vec<Vec<Inline>>]) -> String {
    let row = |cells: &[Vec<Inline>]| format!("| {} |", cells.iter().map(|c| render_inlines(c)).collect::<Vec<_>>().join(" | "));
    let rule: Vec<&str> = aligns
        .iter()
        .map(|a| match a {
            Align::None => "---",
            Align::Left => ":---",
            Align::Center => ":---:",
            Align::Right => "---:",
        })
        .collect();
    let mut lines = vec![row(header), format!("| {} |", rule.join(" | "))];
    lines.extend(rows.iter().map(|r| row(r)));
    lines.join("\n")
}

/// A fenced block whose fence is longer than any backtick run inside it.
fn fenced(info: &str, code: &str) -> String {
    let longest = code.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(longest.max(2) + 1);
    if code.is_empty() {
        format!("{fence}{info}\n{fence}")
    } else {
        format!("{fence}{info}\n{code}\n{fence}")
    }
}
