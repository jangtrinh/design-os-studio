//! Shared by the integration tests.
#![allow(dead_code)]

use designos_docs::Page;

pub const FIXTURES: &[(&str, &str)] = &[
    ("heading", include_str!("../fixtures/heading.md")),
    ("paragraph-inline", include_str!("../fixtures/paragraph-inline.md")),
    ("lists", include_str!("../fixtures/lists.md")),
    ("checklist", include_str!("../fixtures/checklist.md")),
    ("callout", include_str!("../fixtures/callout.md")),
    ("table", include_str!("../fixtures/table.md")),
    ("frame-embed", include_str!("../fixtures/frame-embed.md")),
    ("mermaid", include_str!("../fixtures/mermaid.md")),
    ("code", include_str!("../fixtures/code.md")),
    ("front-matter-and-all-blocks", include_str!("../fixtures/front-matter-and-all-blocks.md")),
    ("escapes-at-edges", include_str!("../fixtures/escapes-at-edges.md")),
    ("nested-lists", include_str!("../fixtures/nested-lists.md")),
];
pub const UNKNOWN: &str = include_str!("../fixtures/unknown-syntax.md");

pub fn names(page: &Page) -> Vec<&'static str> {
    page.blocks.iter().map(|b| b.kind().name()).collect()
}

pub fn parse(text: &str) -> Page {
    Page::parse(text)
}
