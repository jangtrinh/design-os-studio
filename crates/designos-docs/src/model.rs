//! The Markdown block model.
//!
//! A [`Page`] is front matter plus an ordered list of [`Block`]s. Each parsed block keeps the exact text it came
//! from, and the whitespace before it, so writing a page back reproduces every byte, including syntax this model
//! does not know (those blocks are [`BlockKind::Raw`]). Changing a block (through [`Block::kind_mut`]) drops its
//! kept text, and only that block is then written the way Studio writes it ([`Page::render_canonical`]).

use crate::error::Result;
use crate::front_matter::FrontMatter;
use crate::group::Group;

/// Inline content. `Text` is Markdown source, not unescaped text: `2 \* 3` stays `2 \* 3`, so no escaping
/// rules are needed to write it back. Anything inline the model does not structure (images, HTML, `~~strike~~`,
/// reference links, hard breaks) is simply part of a `Text`.
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChecklistItem {
    pub checked: bool,
    pub content: Vec<Inline>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockKind {
    /// Level 1 to 3 (ATX `#` form). Deeper or setext headings are `Raw`.
    Heading {
        level: u8,
        content: Vec<Inline>,
    },
    Paragraph(Vec<Inline>),
    BulletList(Vec<Vec<Inline>>),
    NumberedList {
        start: u64,
        items: Vec<Vec<Inline>>,
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
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Page {
    pub(crate) front: Option<FrontMatter>,
    pub blocks: Vec<Block>,
    /// Everything after the last block (normally the final newline).
    pub(crate) tail: String,
}

impl Page {
    /// Reads a page. Never fails on Markdown: unknown syntax becomes `Raw` blocks. It fails only on front matter
    /// that sets `title`, `description` or `group` to something Studio cannot use, naming the field and the fix.
    pub fn parse(text: &str) -> Result<Page> {
        crate::markdown::parse_page(text)
    }

    /// The page as Markdown. Unedited blocks come back byte for byte.
    pub fn render(&self) -> String {
        crate::render::render_page(self, false)
    }

    /// The page with every block written the way Studio writes new content, ignoring kept text. Raw blocks and
    /// front matter stay verbatim.
    pub fn render_canonical(&self) -> String {
        crate::render::render_page(self, true)
    }

    pub fn front_matter(&self) -> Option<&FrontMatter> {
        self.front.as_ref()
    }

    pub fn set_title(&mut self, title: Option<&str>) {
        self.front.get_or_insert_with(FrontMatter::empty).set("title", title);
    }

    pub fn set_description(&mut self, description: Option<&str>) {
        self.front.get_or_insert_with(FrontMatter::empty).set("description", description);
    }

    pub fn set_group(&mut self, group: Option<Group>) {
        self.front.get_or_insert_with(FrontMatter::empty).set("group", group.map(Group::as_str));
    }

    pub fn push_block(&mut self, kind: BlockKind) {
        self.insert_block(self.blocks.len(), kind);
    }

    /// Inserts a block written canonically, with a blank line around it.
    pub fn insert_block(&mut self, index: usize, kind: BlockKind) {
        let index = index.min(self.blocks.len());
        let mut block = Block::new(kind);
        if index == 0 {
            // The first block's gap is the leading whitespace of the page; it moves to the new first block and
            // the old one gets the canonical blank line.
            if let Some(old_first) = self.blocks.first_mut() {
                block.gap = old_first.gap.take();
            }
        }
        self.blocks.insert(index, block);
        if self.tail.is_empty() {
            self.tail = "\n".into();
        }
    }

    /// Removes a block; the next block inherits the removed block's leading whitespace when it was first.
    pub fn remove_block(&mut self, index: usize) -> Block {
        let removed = self.blocks.remove(index);
        if index == 0 {
            if let Some(next) = self.blocks.first_mut() {
                next.gap = removed.gap.clone();
            }
        }
        removed
    }
}
