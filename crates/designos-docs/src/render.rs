//! Blocks to Markdown. Unedited blocks are written from their kept source; everything else (and everything in
//! canonical mode) is written the way Studio writes new content, with the page's own line endings.

use crate::inline::{render_cell_inlines, render_inlines};
use crate::model::{Align, Block, BlockKind, Inline, Page};

pub(crate) fn render_page(page: &Page, canonical: bool) -> String {
    let eol = if page.crlf { "\r\n" } else { "\n" };
    let mut out = String::new();
    if page.bom {
        out.push('\u{FEFF}');
    }
    if let Some(front) = &page.front {
        out.push_str(front.raw());
    }
    let mut prev_text: Option<String> = None;
    let mut after_rewritten_list = false;
    let mut after_table = false;
    let mut prev_rewritten = false;
    for (i, block) in page.blocks.iter().enumerate() {
        let mut fixed = fixed_text(block, canonical);
        // Behind a rewritten list (whose marker width may have changed), a block kept with its indentation could
        // read as part of the list's last item, so it is written without it.
        if after_rewritten_list && !matches!(block.kind(), BlockKind::Raw(_)) && fixed.is_some_and(|t| t.starts_with([' ', '\t'])) {
            fixed = None;
        }
        after_rewritten_list = fixed.is_none() && matches!(block.kind(), BlockKind::BulletList(_) | BlockKind::NumberedList { .. } | BlockKind::Checklist(_));
        // What the neighbours see of this block: its own text, without any separator comment around it.
        let mut as_neighbour = None;
        let text = match fixed {
            Some(text) => text.to_string(),
            None => {
                let next = page.blocks.get(i + 1).and_then(|b| fixed_text(b, canonical));
                let avoid = markers_to_avoid(block.kind(), prev_text.as_deref(), next);
                let text = to_eol(&render_kind(block.kind(), &avoid), eol);
                let (before, after) = list_separators(block.kind(), &avoid, prev_text.as_deref(), next);
                if before || after {
                    as_neighbour = Some(text.clone());
                }
                let lead = if before { format!("{SEPARATOR}{eol}{eol}") } else { String::new() };
                let trail = if after { format!("{eol}{eol}{SEPARATOR}") } else { String::new() };
                format!("{lead}{text}{trail}")
            }
        };
        let mut gap = if canonical { None } else { block.gap.as_deref() };
        // Right under a table row, a rewritten block with no blank line before it would be read as another row.
        if fixed.is_none() && after_table && gap.is_some_and(|g| g.matches('\n').count() < 2) {
            gap = None;
        }
        // A block kept from parsing may start right where the previous block's source ended (the parser gave that
        // block the blank line and indentation in front of it). Once that block is rewritten those characters are
        // gone, so the canonical gap is needed to keep the two apart.
        if prev_rewritten && gap.is_some_and(|g| !g.contains('\n')) {
            gap = None;
        }
        prev_rewritten = fixed.is_none();
        after_table = matches!(block.kind(), BlockKind::Table { .. });
        let mut lead = match gap {
            Some(g) => g.to_string(),
            None => to_eol(canonical_gap(i, page.front.is_some()), eol),
        };
        if gap.is_none() && i == 0 && page.front.is_none() && text.starts_with("---") {
            lead = eol.to_string(); // generated text that opens with `---` would be read as front matter
        }
        out.push_str(&lead);
        out.push_str(&text);
        prev_text = Some(as_neighbour.unwrap_or(text));
    }
    if canonical && !page.blocks.is_empty() {
        out.push_str(eol);
    } else {
        out.push_str(&page.tail);
    }
    out
}

/// The text a block is written as when it is not rewritten: its kept source, or the text of a raw block.
fn fixed_text(block: &Block, canonical: bool) -> Option<&str> {
    match (&block.source, block.kind()) {
        (Some(source), _) if !canonical => Some(source),
        (_, BlockKind::Raw(text)) => Some(text),
        _ => None,
    }
}

fn to_eol(text: &str, eol: &str) -> String {
    if eol == "\n" {
        text.to_string()
    } else {
        text.replace("\r\n", "\n").replace('\n', eol)
    }
}

/// A blank line between blocks; one after the front matter; nothing before the first block of a page without it.
fn canonical_gap(index: usize, has_front_matter: bool) -> &'static str {
    match (index, has_front_matter) {
        (0, false) => "",
        (0, true) => "\n",
        _ => "\n\n",
    }
}

/// Written between two numbered lists that no delimiter can keep apart.
const SEPARATOR: &str = "<!-- -->";

/// A numbered list between a `.` list and a `)` list has no free delimiter: it takes `.` and an HTML comment keeps
/// it from merging with whichever neighbour also uses `.`. Returns whether a comment is needed before and after.
fn list_separators(kind: &BlockKind, avoid: &[char], prev: Option<&str>, next: Option<&str>) -> (bool, bool) {
    let both_taken = avoid.contains(&'.') && avoid.contains(&')');
    if !both_taken || !matches!(kind, BlockKind::NumberedList { .. }) {
        return (false, false);
    }
    let uses_dot = |t: Option<&str>| t.and_then(leading_list_marker).is_some_and(|(f, m)| f == ListFamily::Numbered && m == '.');
    (uses_dot(prev), uses_dot(next))
}

#[derive(PartialEq, Clone, Copy)]
pub(crate) enum ListFamily {
    Bullet,
    Numbered,
}

/// The list markers a rewritten list must not use because a neighbour already has them: two lists that follow
/// each other read as one when they share a bullet or delimiter.
fn markers_to_avoid(kind: &BlockKind, prev_text: Option<&str>, next_text: Option<&str>) -> Vec<char> {
    let family = match kind {
        BlockKind::BulletList(_) | BlockKind::Checklist(_) => ListFamily::Bullet,
        BlockKind::NumberedList { .. } => ListFamily::Numbered,
        _ => return Vec::new(),
    };
    [prev_text, next_text].into_iter().flatten().filter_map(leading_list_marker).filter(|(f, _)| *f == family).map(|(_, marker)| marker).collect()
}

/// The list marker a block's text starts with: the bullet character, or the `.` / `)` after the number.
pub(crate) fn leading_list_marker(text: &str) -> Option<(ListFamily, char)> {
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

fn bullet_marker(avoid: &[char]) -> char {
    ['-', '*', '+'].into_iter().find(|m| !avoid.contains(m)).unwrap_or('-')
}

fn number_delimiter(avoid: &[char]) -> char {
    ['.', ')'].into_iter().find(|m| !avoid.contains(m)).unwrap_or('.')
}

/// `avoid` lists the bullets or delimiters that neighbouring lists already use.
pub(crate) fn render_kind(kind: &BlockKind, avoid: &[char]) -> String {
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
        BlockKind::BulletList(items) => {
            let bullet = format!("{} ", bullet_marker(avoid));
            join_items(items.iter().map(|i| Item { marker: bullet.clone(), indent: bullet.len(), content: &i.content, children: &i.children }))
        }
        BlockKind::NumberedList { start, items } => {
            let delimiter = number_delimiter(avoid);
            join_items(items.iter().enumerate().map(|(n, i)| {
                let marker = format!("{}{delimiter} ", start + n as u64);
                Item { indent: marker.len(), marker, content: &i.content, children: &i.children }
            }))
        }
        BlockKind::Checklist(items) => {
            let bullet = format!("{} ", bullet_marker(avoid));
            join_items(items.iter().map(|i| Item {
                marker: format!("{bullet}[{}] ", if i.checked { 'x' } else { ' ' }),
                indent: bullet.len(),
                content: &i.content,
                children: &i.children,
            }))
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

/// List items with their nested lists and paragraphs. Children are indented by the width of the item's bullet
/// or number, which is what keeps them inside the item.
struct Item<'a> {
    marker: String,
    /// Columns the item's children are indented by.
    indent: usize,
    content: &'a [Inline],
    children: &'a [BlockKind],
}

fn newline_outside_text(nodes: &[Inline]) -> bool {
    nodes.iter().any(|n| match n {
        Inline::Text(_) => false,
        Inline::Code(c) => c.contains('\n'),
        Inline::Bold(c) | Inline::Italic(c) => newline_outside_text(c),
        Inline::Link { content, target } => target.contains('\n') || newline_outside_text(content),
    })
}

fn join_items<'a>(items: impl Iterator<Item = Item<'a>>) -> String {
    let mut lines: Vec<String> = Vec::new();
    for Item { marker, indent, content, children } in items {
        let indent = " ".repeat(indent);
        // Continuation lines sit under the item's text; the reader drops that indentation again.
        // (Not inside a link target or code span, where the spaces would become part of the target or code.)
        let continuation = if newline_outside_text(content) { "\n".to_string() } else { format!("\n{indent}") };
        let body = render_inlines(content).split('\n').map(str::to_string).collect::<Vec<_>>().join(&continuation);
        let mut text = format!("{marker}{body}");
        let mut prev_child: Option<String> = None;
        let mut prev_child_kind: Option<&BlockKind> = None;
        for child in children {
            let avoid = markers_to_avoid(child, prev_child.as_deref(), None);
            let rendered = render_kind(child, &avoid);
            // A list that cannot interrupt a paragraph (it does not start at 1, or its first item is empty) is read
            // as the paragraph's text when it sits right under paragraph text, so a blank line goes in between.
            let cannot_interrupt = match child {
                BlockKind::NumberedList { start, items } => *start != 1 || items.first().is_some_and(|i| i.content.is_empty()),
                BlockKind::BulletList(items) => items.first().is_some_and(|i| i.content.is_empty()),
                _ => false,
            };
            let after_text = match prev_child_kind {
                None => !content.is_empty(),
                Some(kind) => matches!(kind, BlockKind::Paragraph(_)),
            };
            let separator = if matches!(child, BlockKind::Paragraph(_)) || (cannot_interrupt && after_text) { "\n\n" } else { "\n" };
            prev_child_kind = Some(child);
            text.push_str(separator);
            text.push_str(&rendered.split('\n').map(|l| if l.is_empty() { String::new() } else { format!("{indent}{l}") }).collect::<Vec<_>>().join("\n"));
            prev_child = Some(rendered);
        }
        lines.push(text);
    }
    lines.join("\n")
}

fn render_table(aligns: &[Align], header: &[Vec<Inline>], rows: &[Vec<Vec<Inline>>]) -> String {
    let row = |cells: &[Vec<Inline>]| format!("| {} |", cells.iter().map(|c| render_cell_inlines(c)).collect::<Vec<_>>().join(" | "));
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
