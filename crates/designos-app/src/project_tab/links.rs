//! Links between pages: resolving a Markdown link to a page path, and retargeting links when a page is renamed.

use designos_docs::{BlockKind, ChecklistItem, DocsStore, Expect, Inline, ListItem, Page};

/// Where a link inside the page at `page_rel` points: `Some(page path)` for a link to another page of the docs
/// folder (`other.md`, `../spec/x.md#part`), `None` for anything else (a web link, an anchor, an embed).
pub fn page_target(page_rel: &str, target: &str) -> Option<String> {
    let (dest, _) = split_target(target);
    let dest = dest.split('#').next().unwrap_or("");
    if dest.is_empty() || dest.contains(':') || !dest.ends_with(".md") {
        return None;
    }
    resolve(dir_of(page_rel), dest)
}

/// The in-page anchor of a link (`#fees` in `[x](#fees)` or `[x](other.md#fees)`).
pub fn anchor_of(target: &str) -> Option<String> {
    let (dest, _) = split_target(target);
    dest.split_once('#').map(|(_, a)| a.to_string()).filter(|a| !a.is_empty())
}

/// `(destination, optional title)` of the text between a link's parentheses.
fn split_target(target: &str) -> (&str, &str) {
    let t = target.trim();
    if let Some(rest) = t.strip_prefix('<') {
        if let Some((dest, after)) = rest.split_once('>') {
            return (dest, after.trim());
        }
    }
    match t.split_once(char::is_whitespace) {
        Some((dest, title)) => (dest, title.trim()),
        None => (t, ""),
    }
}

fn dir_of(rel: &str) -> &str {
    rel.rsplit_once('/').map(|(d, _)| d).unwrap_or("")
}

/// `dir` + relative `dest`, normalised (`.` and `..` resolved). `None` when it climbs out of the docs folder.
fn resolve(dir: &str, dest: &str) -> Option<String> {
    let mut parts: Vec<&str> = if dest.starts_with('/') || dir.is_empty() { Vec::new() } else { dir.split('/').collect() };
    for seg in dest.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// The path to write in a link on a page in `from_dir` so it points at `to_rel`.
fn relative(from_dir: &str, to_rel: &str) -> String {
    let from: Vec<&str> = from_dir.split('/').filter(|s| !s.is_empty()).collect();
    let to: Vec<&str> = to_rel.split('/').collect();
    let common = from.iter().zip(to.iter()).take_while(|(a, b)| a == b).count().min(to.len().saturating_sub(1));
    let mut out: Vec<&str> = std::iter::repeat_n("..", from.len() - common).collect();
    out.extend(&to[common..]);
    out.join("/")
}

/// Points every link on `page` (at `page_rel`) that resolves to `old_rel` at `new_rel`. Returns how many changed.
pub fn retarget(page: &mut Page, page_rel: &str, old_rel: &str, new_rel: &str) -> usize {
    let mut total = 0;
    for block in page.blocks.iter_mut() {
        let mut kind = block.kind().clone();
        let n = retarget_kind(&mut kind, page_rel, old_rel, new_rel);
        if n > 0 {
            *block.kind_mut() = kind;
            total += n;
        }
    }
    total
}

fn retarget_kind(kind: &mut BlockKind, page_rel: &str, old: &str, new: &str) -> usize {
    match kind {
        BlockKind::Heading { content, .. } | BlockKind::Paragraph(content) => retarget_inlines(content, page_rel, old, new),
        BlockKind::Callout { body, .. } => retarget_inlines(body, page_rel, old, new),
        BlockKind::BulletList(items) | BlockKind::NumberedList { items, .. } => items.iter_mut().map(|i| retarget_item(i, page_rel, old, new)).sum(),
        BlockKind::Checklist(items) => items.iter_mut().map(|i| retarget_check(i, page_rel, old, new)).sum(),
        BlockKind::Table { header, rows, .. } => {
            header.iter_mut().chain(rows.iter_mut().flatten()).map(|cell| retarget_inlines(cell, page_rel, old, new)).sum()
        }
        BlockKind::FrameEmbed { .. } | BlockKind::Mermaid { .. } | BlockKind::Code { .. } | BlockKind::Raw(_) => 0,
    }
}

fn retarget_item(item: &mut ListItem, page_rel: &str, old: &str, new: &str) -> usize {
    retarget_inlines(&mut item.content, page_rel, old, new) + item.children.iter_mut().map(|c| retarget_kind(c, page_rel, old, new)).sum::<usize>()
}

fn retarget_check(item: &mut ChecklistItem, page_rel: &str, old: &str, new: &str) -> usize {
    retarget_inlines(&mut item.content, page_rel, old, new) + item.children.iter_mut().map(|c| retarget_kind(c, page_rel, old, new)).sum::<usize>()
}

fn retarget_inlines(nodes: &mut [Inline], page_rel: &str, old: &str, new: &str) -> usize {
    let mut n = 0;
    for node in nodes {
        match node {
            Inline::Bold(c) | Inline::Italic(c) => n += retarget_inlines(c, page_rel, old, new),
            Inline::Link { content, target } => {
                n += retarget_inlines(content, page_rel, old, new);
                if page_target(page_rel, target).as_deref() == Some(old) {
                    let (dest, title) = split_target(target);
                    let anchor = dest.split_once('#').map(|(_, a)| format!("#{a}")).unwrap_or_default();
                    let path = relative(dir_of(page_rel), new);
                    *target = if title.is_empty() { format!("{path}{anchor}") } else { format!("{path}{anchor} {title}") };
                    n += 1;
                }
            }
            Inline::Text(_) | Inline::Code(_) => {}
        }
    }
    n
}

/// Retargets links to `old_rel` on every other page of the docs folder. Returns the number of links changed and a
/// warning for each page that could not be rewritten.
pub fn retarget_all(store: &DocsStore, old_rel: &str, new_rel: &str) -> (usize, Vec<String>) {
    let mut total = 0;
    let mut warnings = Vec::new();
    let Ok(files) = store.list_page_files() else {
        return (0, vec!["Links to the renamed page could not be updated: the docs folder cannot be listed".into()]);
    };
    for rel in files.iter().filter(|r| r.as_str() != old_rel) {
        let Ok(read) = store.read_page(rel) else { continue };
        let mut page = read.value;
        let n = retarget(&mut page, rel, old_rel, new_rel);
        if n == 0 {
            continue;
        }
        match store.write_page(rel, &page, Expect::Hash(read.hash)) {
            Ok(_) => total += n,
            Err(e) => warnings.push(format!("Links in {rel} were not updated: {e}")),
        }
    }
    (total, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_to_a_page_resolves_against_the_folder_of_the_page_that_holds_it() {
        assert_eq!(page_target("spec/a.md", "b.md").as_deref(), Some("spec/b.md"));
        assert_eq!(page_target("spec/a.md", "../overview.md#goals").as_deref(), Some("overview.md"));
        assert_eq!(page_target("overview.md", "./spec/b.md \"Title\"").as_deref(), Some("spec/b.md"));
        assert_eq!(page_target("overview.md", "https://x.dev/a.md"), None);
        assert_eq!(page_target("overview.md", "#goals"), None);
        assert_eq!(page_target("overview.md", "../escape.md"), None);
    }

    #[test]
    fn a_new_link_path_is_relative_to_the_page_that_holds_the_link() {
        assert_eq!(relative("", "spec/b.md"), "spec/b.md");
        assert_eq!(relative("spec", "spec/b.md"), "b.md");
        assert_eq!(relative("spec", "overview.md"), "../overview.md");
        assert_eq!(relative("discovery", "spec/b.md"), "../spec/b.md");
    }

    #[test]
    fn retargeting_changes_links_in_every_block_shape_and_leaves_other_links() {
        let text = "See [the flow](spec/flow.md#fees \"Fees\") and [web](https://a.dev/flow.md).\n\n- item [x](spec/flow.md)\n  - nested [y](spec/flow.md)\n\n| a |\n|---|\n| [z](spec/flow.md) |\n";
        let mut page = Page::parse(text);
        let n = retarget(&mut page, "overview.md", "spec/flow.md", "spec/payment-flow.md");
        assert_eq!(n, 4);
        let out = page.render();
        assert!(out.contains("[the flow](spec/payment-flow.md#fees \"Fees\")"), "{out}");
        assert!(out.contains("[web](https://a.dev/flow.md)"), "{out}");
        assert!(out.contains("[z](spec/payment-flow.md)"), "{out}");
    }
}
