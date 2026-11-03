//! Reading, writing and editing a [`Page`] as a whole.

use crate::error::{DocsError, Result};
use crate::front_matter::FrontMatter;
use crate::group::Group;
use crate::hash::ContentHash;
use crate::model::{Block, BlockKind, Page};

impl Page {
    /// Reads a page. Never fails: unknown Markdown becomes `Raw` blocks, and front matter values Studio cannot use
    /// are left unset and listed in [`FrontMatter::problems`], so the body is always readable.
    pub fn parse(text: &str) -> Page {
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

    /// The hash of every block, in order (see [`Block::hash`]).
    pub fn block_hashes(&self) -> Vec<ContentHash> {
        self.blocks.iter().map(Block::hash).collect()
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
            self.tail = if self.crlf { "\r\n" } else { "\n" }.into();
        }
    }

    /// Removes a block; the next block inherits the removed block's leading whitespace when it was first.
    pub fn remove_block(&mut self, index: usize) -> Result<Block> {
        if index >= self.blocks.len() {
            return Err(DocsError::BlockIndex { index, len: self.blocks.len() });
        }
        let removed = self.blocks.remove(index);
        if index == 0 {
            if let Some(next) = self.blocks.first_mut() {
                next.gap = removed.gap.clone();
            }
        }
        Ok(removed)
    }
}
