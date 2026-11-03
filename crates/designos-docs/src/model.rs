//! The Markdown block model.
//!
//! A [`Page`] is front matter plus an ordered list of [`Block`]s. Each parsed block keeps the exact text it came
//! from, and the whitespace before it, so writing a page back reproduces every byte, including syntax this model
//! does not know (those blocks are [`BlockKind::Raw`]). Changing a block (through [`Block::kind_mut`]) drops its
//! kept text, and only that block is then written the way Studio writes it ([`Page::render_canonical`]).

use crate::front_matter::FrontMatter;
use crate::hash::ContentHash;

/// Inline content. `Text` is Markdown source, not unescaped text: `2 \* 3` stays `2 \* 3`, so no escaping
/// rules are needed to write it back. Typed plain text goes through [`crate::plain_inlines`] first. Anything inline
/// the model does not structure (images, HTML, `~~strike~~`, reference links, hard breaks) is simply part of a `Text`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Inline {
    Text(String),
    Bold(Vec<Inline>),
    Italic(Vec<Inline>),
    /// The code itself, without the backticks.
    Code(String),
    /// `[content](target)`; `target` is everything between the parentheses (URL and optional title) as written.
    Link {
        content: Vec<Inline>,
        target: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    None,
    Left,
    Center,
    Right,
}

/// One bullet or numbered item: its own text, then nested lists and extra paragraphs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListItem {
    pub content: Vec<Inline>,
    /// Nested lists and further paragraphs of this item, in order (`BulletList`, `NumberedList`, `Checklist`,
    /// `Paragraph`). Indenting an item moves it into the previous item's children; outdenting moves it out.
    pub children: Vec<BlockKind>,
}

impl ListItem {
    pub fn new(content: Vec<Inline>) -> Self {
        Self { content, children: Vec::new() }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChecklistItem {
    pub checked: bool,
    pub content: Vec<Inline>,
    pub children: Vec<BlockKind>,
}

impl ChecklistItem {
    pub fn new(checked: bool, content: Vec<Inline>) -> Self {
        Self { checked, content, children: Vec::new() }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockKind {
    /// Level 1 to 3 (ATX `#` form). Deeper or setext headings are `Raw`.
    Heading {
        level: u8,
        content: Vec<Inline>,
    },
    Paragraph(Vec<Inline>),
    BulletList(Vec<ListItem>),
    NumberedList {
        start: u64,
        items: Vec<ListItem>,
    },
    Checklist(Vec<ChecklistItem>),
    /// `> [!kind]` followed by `> ` lines of one paragraph.
    Callout {
        kind: String,
        body: Vec<Inline>,
    },
    /// Cells hold inline content only. `header.len()` columns; every row has the same count.
    Table {
        aligns: Vec<Align>,
        header: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
    },
    /// `![name](designos:<file-id>#<node-id>)` alone in its paragraph.
    FrameEmbed {
        name: String,
        file_id: String,
        node_id: String,
    },
    Mermaid {
        source: String,
    },
    /// A fenced code block; `info` is the text after the opening fence.
    Code {
        info: String,
        code: String,
    },
    /// Any Markdown the model does not know, kept verbatim.
    Raw(String),
}

impl BlockKind {
    /// Stable snake_case name, for logs, tests and MCP block descriptions.
    pub fn name(&self) -> &'static str {
        match self {
            BlockKind::Heading { .. } => "heading",
            BlockKind::Paragraph(_) => "paragraph",
            BlockKind::BulletList(_) => "bullet_list",
            BlockKind::NumberedList { .. } => "numbered_list",
            BlockKind::Checklist(_) => "checklist",
            BlockKind::Callout { .. } => "callout",
            BlockKind::Table { .. } => "table",
            BlockKind::FrameEmbed { .. } => "frame_embed",
            BlockKind::Mermaid { .. } => "mermaid",
            BlockKind::Code { .. } => "code",
            BlockKind::Raw(_) => "raw",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    kind: BlockKind,
    /// Exact whitespace between the previous block (or the front matter) and this block. `None` for a block built
    /// in code: it gets the canonical blank line.
    pub(crate) gap: Option<String>,
    /// Exact text of the block as parsed; `None` once edited or when built in code.
    pub(crate) source: Option<String>,
}

impl Block {
    pub fn new(kind: BlockKind) -> Self {
        Self { kind, gap: None, source: None }
    }

    pub(crate) fn parsed(kind: BlockKind, gap: String, source: String) -> Self {
        Self { kind, gap: Some(gap), source: Some(source) }
    }

    pub fn kind(&self) -> &BlockKind {
        &self.kind
    }

    /// Mutable access to the content. The text kept from parsing is dropped, because it would no longer match.
    pub fn kind_mut(&mut self) -> &mut BlockKind {
        self.source = None;
        &mut self.kind
    }

    /// The exact text this block was parsed from, while it is unedited.
    pub fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }

    /// SHA-256 of the block's text: the text it was parsed from, or the canonical text once edited. Two reads of
    /// an unchanged block give the same hash wherever the block sits in the page, so a writer can check the one
    /// block it means to change (block-level compare-and-swap) while other blocks are edited around it.
    ///
    /// An edited block's hash is final only after the page is saved and read again: the page may write the block
    /// with another line ending or another list marker than the in-memory canonical text used here (to match the
    /// file or to keep neighbouring lists apart). A hash taken from memory can therefore differ from the one read
    /// back after saving. That is a false conflict (the writer re-reads and retries), never a missed one.
    pub fn hash(&self) -> ContentHash {
        match &self.source {
            Some(source) => ContentHash::of(source.as_bytes()),
            None => ContentHash::of(crate::render::render_kind(&self.kind, &[]).as_bytes()),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Page {
    /// The file started with a byte order mark; it is written back first.
    pub(crate) bom: bool,
    /// The file uses `\r\n` line endings; new and edited text follows it.
    pub(crate) crlf: bool,
    pub(crate) front: Option<FrontMatter>,
    pub blocks: Vec<Block>,
    /// Everything after the last block (normally the final newline).
    pub(crate) tail: String,
}
