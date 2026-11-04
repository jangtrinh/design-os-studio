//! Lays a page out for the reading column: every block becomes drawing primitives at fixed positions, so painting a
//! frame is a walk over the visible ones. The layout is cached by the reader and made again only when the page, the
//! column width or the project's file list changes.

use super::inline::{self, mono, LinkSpan, TextStyle};
use super::ops::{self, TreeSnapshot};
use crate::icons::Icon;
use crate::theme::{self, TextWeight};
use designos_docs::{plain_text, Align, Block, BlockKind, ChecklistItem, Inline, ListItem, Page};
use egui::text::LayoutJob;
use egui::{Color32, Galley, Pos2, Rect, Stroke, Vec2};
use std::sync::Arc;

pub const BODY: TextStyle = TextStyle { size: 14.0, line: 24.0, weight: TextWeight::Regular, color: theme::TEXT_SECONDARY };
const TITLE: TextStyle = TextStyle { size: 30.0, line: 36.0, weight: TextWeight::Semibold, color: theme::TEXT };
const DESCRIPTION: TextStyle = TextStyle { size: 16.0, line: 26.0, weight: TextWeight::Regular, color: theme::TEXT_TERTIARY };
const H2: TextStyle = TextStyle { size: 20.0, line: 28.0, weight: TextWeight::Semibold, color: theme::TEXT };
const H3: TextStyle = TextStyle { size: 16.0, line: 24.0, weight: TextWeight::Semibold, color: theme::TEXT };
const NOTE: TextStyle = TextStyle { size: 12.0, line: 18.0, weight: TextWeight::Regular, color: theme::TEXT_TERTIARY };
const LIST_INDENT: f32 = 24.0;
const ITEM_GAP: f32 = 4.0;
const CODE_LINE: f32 = 20.0;
const CODE_INSET: f32 = theme::SPACE_BLOCK;
const CELL_PAD_X: f32 = 12.0;
const CELL_PAD_Y: f32 = 8.0;
const ROW_MIN: f32 = 36.0;
const COL_MIN: f32 = 120.0;
const COL_MAX: f32 = 320.0;
const EMBED_HEADER: f32 = theme::CONTROL_SM;
const EMBED_BODY_INSET: f32 = theme::SPACE_BLOCK;
const EMBED_PLACEHOLDER_H: f32 = 160.0;
/// Space under a scrolled block for its scroll bar (8 gap + the 11 px bar zone).
pub const HBAR_ZONE: f32 = 19.0;

/// egui sets a glyph at the top of its row, CSS in the middle of its line box: where a style has a line height taller
/// than its font, the text is moved down by half the extra leading so it sits where the design puts it.
pub fn half_leading(galley: &Galley) -> f32 {
    galley.rows.first().and_then(|row| row.glyphs.first()).map_or(0.0, |g| ((g.line_height - g.font_height) / 2.0).max(0.0).round())
}

#[derive(Clone)]
pub enum Prim {
    Text {
        pos: Pos2,
        galley: Arc<Galley>,
        links: Vec<LinkSpan>,
        scroll: Option<usize>,
    },
    Rect {
        rect: Rect,
        fill: Color32,
        radius: u8,
        stroke: Option<Stroke>,
        scroll: Option<usize>,
    },
    Icon {
        icon: Icon,
        center: Pos2,
        size: f32,
        color: Color32,
    },
    Disc {
        center: Pos2,
        radius: f32,
        color: Color32,
    },
    Check {
        rect: Rect,
        checked: bool,
    },
    /// A horizontal scroll bar under a block that is wider than the column; `id` keys its offset.
    HBar {
        id: usize,
        track: Rect,
        view: Rect,
        content_w: f32,
    },
}

#[derive(Clone)]
pub struct Item {
    pub y: f32,
    pub h: f32,
    pub prims: Vec<Prim>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeadingPos {
    pub y: f32,
    pub level: u8,
    pub text: String,
    pub slug: String,
}

#[derive(Clone)]
pub struct Laid {
    pub items: Vec<Item>,
    pub height: f32,
    pub headings: Vec<HeadingPos>,
}

/// Margins around a block, CSS style: the gap between two blocks is the larger of the first's bottom and the
/// second's top.
#[derive(Clone, Copy)]
struct Spacing {
    top: f32,
    bottom: f32,
    /// Sits flush under a heading (the heading's own bottom margin is the whole gap).
    flush_after_heading: bool,
    heading: bool,
    table: bool,
}

const fn spacing(top: f32, bottom: f32) -> Spacing {
    Spacing { top, bottom, flush_after_heading: false, heading: false, table: false }
}

fn gap(prev: Spacing, next: Spacing) -> f32 {
    if prev.heading && next.flush_after_heading {
        prev.bottom
    } else if prev.table && next.heading && next.top >= 32.0 {
        48.0
    } else {
        prev.bottom.max(next.top)
    }
}

struct Ctx<'a> {
    shape: &'a dyn Fn(LayoutJob) -> Arc<Galley>,
    snapshot: &'a TreeSnapshot,
    width: f32,
    next_scroll_id: usize,
}

impl Ctx<'_> {
    fn text(&self, inlines: &[Inline], style: TextStyle, width: f32) -> (Arc<Galley>, Vec<LinkSpan>) {
        let (job, links) = inline::job(inlines, style, width);
        ((self.shape)(job), links)
    }

    /// Text that is not Markdown source (a front matter title, a note).
    fn plain(&self, text: &str, style: TextStyle, width: f32) -> Arc<Galley> {
        (self.shape)(inline::styled_plain(text, style, width))
    }
}

struct Cursor {
    y: f32,
    prev: Option<Spacing>,
    items: Vec<Item>,
}

impl Cursor {
    fn push(&mut self, sp: Spacing, build: impl FnOnce(f32) -> (Vec<Prim>, f32)) {
        if let Some(prev) = self.prev {
            self.y += gap(prev, sp);
        } else {
            self.y += sp.top;
        }
        let (prims, h) = build(self.y);
        self.items.push(Item { y: self.y, h, prims });
        self.y += h;
        self.prev = Some(sp);
    }
}

pub fn slug_of(text: &str) -> String {
    ops::slugify(text)
}

/// `notes` are one-line messages shown under the description (front matter problems, the size note).
pub fn layout(shape: &dyn Fn(LayoutJob) -> Arc<Galley>, rel: &str, page: &Page, snapshot: &TreeSnapshot, notes: &[String], width: f32) -> Laid {
    let mut ctx = Ctx { shape, snapshot, width, next_scroll_id: 0 };
    let meta = ops::meta_title_and_description(rel, page);
    let mut cur = Cursor { y: 0.0, prev: None, items: Vec::new() };
    let mut headings = Vec::new();
    let skip_block = meta.consumed_h1;

    // 24 px under the header: the first block keeps its own larger top margin (H2: 32) and never sits closer.
    cur.push(spacing(0.0, 24.0), |y| {
        let galley = ctx.plain(&meta.title, TITLE, width);
        let mut h = galley.size().y;
        let mut prims = vec![Prim::Text { pos: Pos2::new(0.0, y), galley, links: Vec::new(), scroll: None }];
        if let Some(desc) = meta.description.as_deref().filter(|d| !d.trim().is_empty()) {
            let g = ctx.plain(desc, DESCRIPTION, width);
            prims.push(Prim::Text { pos: Pos2::new(0.0, y + h + 12.0), galley: g.clone(), links: Vec::new(), scroll: None });
            h += 12.0 + g.size().y;
        }
        for note in notes {
            let g = ctx.plain(note, NOTE, width);
            prims.push(Prim::Text { pos: Pos2::new(0.0, y + h + 8.0), galley: g.clone(), links: Vec::new(), scroll: None });
            h += 8.0 + g.size().y;
        }
        (prims, h)
    });

    for (index, block) in page.blocks.iter().enumerate() {
        if Some(index) == skip_block || ops::renders_nothing(block.kind()) {
            continue;
        }
        layout_block(&mut ctx, &mut cur, block, &mut headings);
    }
    let height = cur.y;
    Laid { items: cur.items, height, headings }
}

fn layout_block(ctx: &mut Ctx, cur: &mut Cursor, block: &Block, headings: &mut Vec<HeadingPos>) {
    let width = ctx.width;
    match block.kind() {
        BlockKind::Heading { level, content } => {
            let (style, sp) = match level {
                1 => (TITLE, Spacing { heading: true, ..spacing(40.0, 16.0) }),
                2 => (H2, Spacing { heading: true, ..spacing(32.0, 16.0) }),
                _ => (H3, Spacing { heading: true, ..spacing(24.0, 8.0) }),
            };
            let text = plain_text(content);
            let (galley, links) = ctx.text(content, style, width);
            let top_y = std::cell::Cell::new(0.0);
            cur.push(sp, |y| {
                top_y.set(y);
                let h = galley.size().y;
                (vec![Prim::Text { pos: Pos2::new(0.0, y), galley, links, scroll: None }], h)
            });
            if *level >= 2 {
                headings.push(HeadingPos { y: top_y.get(), level: *level, slug: slug_of(&text), text });
            }
        }
        BlockKind::Paragraph(content) => {
            let (galley, links) = ctx.text(content, BODY, width);
            cur.push(Spacing { flush_after_heading: true, ..spacing(16.0, 0.0) }, |y| {
                let h = galley.size().y;
                (vec![Prim::Text { pos: Pos2::new(0.0, y), galley, links, scroll: None }], h)
            });
        }
        BlockKind::BulletList(items) => {
            let mut prims = Vec::new();
            let h = list_items(ctx, items, None, 0.0, width, 0.0, &mut prims);
            push_prebuilt(cur, Spacing { flush_after_heading: true, ..spacing(16.0, 0.0) }, prims, h);
        }
        BlockKind::NumberedList { start, items } => {
            let mut prims = Vec::new();
            let h = list_items(ctx, items, Some(*start), 0.0, width, 0.0, &mut prims);
            push_prebuilt(cur, Spacing { flush_after_heading: true, ..spacing(16.0, 0.0) }, prims, h);
        }
        BlockKind::Checklist(items) => {
            let mut prims = Vec::new();
            let h = check_items(ctx, items, 0.0, width, 0.0, &mut prims);
            push_prebuilt(cur, Spacing { flush_after_heading: true, ..spacing(16.0, 0.0) }, prims, h);
        }
        BlockKind::Callout { kind, body } => {
            let (icon, color) = callout_look(kind);
            let text_x = 16.0 + 16.0 + 8.0;
            let (galley, links) = ctx.text(body, BODY, (width - text_x - 16.0).max(40.0));
            cur.push(spacing(16.0, 0.0), |y| {
                let h = galley.size().y + theme::INSET_BUTTON * 2.0;
                let prims = vec![
                    Prim::Rect {
                        rect: Rect::from_min_size(Pos2::new(0.0, y), Vec2::new(width, h)),
                        fill: theme::BG_SECONDARY,
                        radius: theme::RADIUS_XL,
                        stroke: None,
                        scroll: None,
                    },
                    Prim::Icon { icon, center: Pos2::new(16.0 + 8.0, y + theme::INSET_BUTTON + 4.0 + 8.0), size: 16.0, color },
                    Prim::Text { pos: Pos2::new(text_x, y + theme::INSET_BUTTON), galley, links, scroll: None },
                ];
                (prims, h)
            });
        }
        BlockKind::Table { aligns, header, rows } => {
            let sp = Spacing { flush_after_heading: true, table: true, ..spacing(16.0, 0.0) };
            let id = ctx.next_scroll_id;
            ctx.next_scroll_id += 1;
            let (prims, h) = table(ctx, id, aligns, header, rows);
            push_prebuilt(cur, sp, prims, h);
        }
        BlockKind::FrameEmbed { name, file_id, .. } => {
            let file = ctx.snapshot.files.iter().find(|f| f.id == *file_id).map(|f| f.name.clone());
            let title = match file {
                Some(file) => format!("{name} · {file}"),
                None => name.clone(),
            };
            let sp = spacing(24.0, 24.0);
            let (prims, h) = embed_card(ctx, Icon::Frame, &title, EmbedBody::Placeholder);
            push_prebuilt(cur, sp, prims, h);
        }
        BlockKind::Mermaid { source } => {
            let sp = spacing(24.0, 24.0);
            let (prims, h) = embed_card(ctx, Icon::Connector, "Diagram", EmbedBody::Source(source));
            push_prebuilt(cur, sp, prims, h);
        }
        BlockKind::Code { code, .. } => {
            let id = ctx.next_scroll_id;
            ctx.next_scroll_id += 1;
            let (prims, h) = code_card(ctx, id, code);
            push_prebuilt(cur, spacing(16.0, 0.0), prims, h);
        }
        BlockKind::Raw(text) => {
            let t = text.trim();
            if matches!(t, "---" | "***" | "___") {
                cur.push(spacing(24.0, 24.0), |y| {
                    let rect = Rect::from_min_size(Pos2::new(0.0, y), Vec2::new(width, 1.0));
                    (vec![Prim::Rect { rect, fill: theme::BORDER, radius: 0, stroke: None, scroll: None }], 1.0)
                });
            } else {
                let id = ctx.next_scroll_id;
                ctx.next_scroll_id += 1;
                let (prims, h) = code_card(ctx, id, t);
                push_prebuilt(cur, spacing(16.0, 0.0), prims, h);
            }
        }
    }
}

/// Primitives built at y 0 move down to where the cursor puts the block.
fn push_prebuilt(cur: &mut Cursor, sp: Spacing, prims: Vec<Prim>, h: f32) {
    cur.push(sp, move |y| (prims.into_iter().map(|p| shifted(p, y)).collect(), h));
}

fn shifted(prim: Prim, dy: f32) -> Prim {
    let d = Vec2::new(0.0, dy);
    match prim {
        Prim::Text { pos, galley, links, scroll } => Prim::Text { pos: pos + d, galley, links, scroll },
        Prim::Rect { rect, fill, radius, stroke, scroll } => Prim::Rect { rect: rect.translate(d), fill, radius, stroke, scroll },
        Prim::Icon { icon, center, size, color } => Prim::Icon { icon, center: center + d, size, color },
        Prim::Disc { center, radius, color } => Prim::Disc { center: center + d, radius, color },
        Prim::Check { rect, checked } => Prim::Check { rect: rect.translate(d), checked },
        Prim::HBar { id, track, view, content_w } => Prim::HBar { id, track: track.translate(d), view: view.translate(d), content_w },
    }
}

fn callout_look(kind: &str) -> (Icon, Color32) {
    match kind.trim().to_ascii_lowercase().as_str() {
        "tip" => (Icon::Check, theme::SUCCESS),
        "warning" | "caution" => (Icon::Flag, theme::WARNING),
        "danger" | "important" => (Icon::Close, theme::DANGER),
        _ => (Icon::Help, theme::INFO),
    }
}

/// Lays bullet or numbered items at `x`. Returns the height; primitives are at `y0 + local y`.
fn list_items(ctx: &Ctx, items: &[ListItem], numbered: Option<u64>, x: f32, width: f32, y0: f32, out: &mut Vec<Prim>) -> f32 {
    let mut y = 0.0;
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            y += ITEM_GAP;
        }
        let text_w = (width - LIST_INDENT).max(40.0);
        let (galley, links) = ctx.text(&item.content, BODY, text_w);
        let first_line = BODY.line;
        match numbered {
            Some(start) => {
                let marker = format!("{}.", start + i as u64);
                let g = (ctx.shape)(inline::plain(&marker, theme::font_text(14.0, TextWeight::Regular), BODY.line, theme::TEXT_SECONDARY, f32::INFINITY));
                let w = g.size().x;
                out.push(Prim::Text { pos: Pos2::new(x + LIST_INDENT - 6.0 - w, y0 + y), galley: g, links: Vec::new(), scroll: None });
            }
            None => out.push(Prim::Disc {
                center: Pos2::new(x + theme::READER_BULLET_CENTER_X, y0 + y + first_line / 2.0),
                radius: 2.5,
                color: theme::TEXT_SECONDARY,
            }),
        }
        let text_h = galley.size().y;
        out.push(Prim::Text { pos: Pos2::new(x + LIST_INDENT, y0 + y), galley, links, scroll: None });
        y += text_h;
        y += children(ctx, &item.children, x + LIST_INDENT, text_w, y0 + y, out);
    }
    y
}

fn check_items(ctx: &Ctx, items: &[ChecklistItem], x: f32, width: f32, y0: f32, out: &mut Vec<Prim>) -> f32 {
    let mut y = 0.0;
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            y += ITEM_GAP;
        }
        let text_w = (width - LIST_INDENT).max(40.0);
        let (galley, links) = ctx.text(&item.content, BODY, text_w);
        let box_rect = Rect::from_center_size(Pos2::new(x + theme::READER_CHECKBOX_CENTER_X, y0 + y + BODY.line / 2.0), Vec2::splat(16.0));
        out.push(Prim::Check { rect: box_rect, checked: item.checked });
        let text_h = galley.size().y.max(BODY.line);
        out.push(Prim::Text { pos: Pos2::new(x + LIST_INDENT, y0 + y), galley, links, scroll: None });
        y += text_h;
        y += children(ctx, &item.children, x + LIST_INDENT, text_w, y0 + y, out);
    }
    y
}

/// Nested lists and extra paragraphs of an item, 4 px under its text. Returns the height they add.
fn children(ctx: &Ctx, kids: &[BlockKind], x: f32, width: f32, y0: f32, out: &mut Vec<Prim>) -> f32 {
    let mut y = 0.0;
    for kind in kids {
        y += ITEM_GAP;
        match kind {
            BlockKind::BulletList(items) => y += list_items(ctx, items, None, x, width, y0 + y, out),
            BlockKind::NumberedList { start, items } => y += list_items(ctx, items, Some(*start), x, width, y0 + y, out),
            BlockKind::Checklist(items) => y += check_items(ctx, items, x, width, y0 + y, out),
            BlockKind::Paragraph(content) => {
                let (galley, links) = ctx.text(content, BODY, width);
                let h = galley.size().y;
                out.push(Prim::Text { pos: Pos2::new(x, y0 + y), galley, links, scroll: None });
                y += h;
            }
            _ => y -= ITEM_GAP,
        }
    }
    y
}

fn code_card(ctx: &mut Ctx, scroll_id: usize, code: &str) -> (Vec<Prim>, f32) {
    let width = ctx.width;
    let style_font = mono(13.0);
    let lines: Vec<&str> = code.trim_end_matches('\n').split('\n').collect();
    let h = lines.len() as f32 * CODE_LINE + CODE_INSET * 2.0;
    let mut prims = vec![Prim::Rect {
        rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(width, h)),
        fill: theme::BG_SECONDARY,
        radius: theme::RADIUS_XL,
        stroke: None,
        scroll: None,
    }];
    let mut content_w: f32 = 0.0;
    for (i, line) in lines.iter().enumerate() {
        let galley = (ctx.shape)(inline::plain(line, style_font.clone(), CODE_LINE, theme::TEXT_SECONDARY, f32::INFINITY));
        content_w = content_w.max(galley.size().x);
        prims.push(Prim::Text { pos: Pos2::new(CODE_INSET, CODE_INSET + i as f32 * CODE_LINE), galley, links: Vec::new(), scroll: Some(scroll_id) });
    }
    let total_h = scroll_bar(&mut prims, scroll_id, width, h, content_w + CODE_INSET * 2.0);
    (prims, total_h)
}

/// Adds a scroll bar below a block of height `h` when its content is wider than the column. Returns the block's height with the bar.
fn scroll_bar(prims: &mut Vec<Prim>, id: usize, width: f32, h: f32, content_w: f32) -> f32 {
    if content_w <= width + 0.5 {
        return h;
    }
    let track = Rect::from_min_size(Pos2::new(0.0, h + 8.0), Vec2::new(width, 11.0));
    let view = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, h));
    prims.push(Prim::HBar { id, track, view, content_w });
    h + HBAR_ZONE
}

enum EmbedBody<'a> {
    Placeholder,
    Source(&'a str),
}

/// The card around a frame embed or a diagram: a header with the kind icon and title, then the body.
fn embed_card(ctx: &mut Ctx, icon: Icon, title: &str, body: EmbedBody) -> (Vec<Prim>, f32) {
    let width = ctx.width;
    let mut prims = Vec::new();
    let mut body_prims = Vec::new();
    let body_h = match body {
        EmbedBody::Placeholder => {
            let inner = Rect::from_min_size(
                Pos2::new(EMBED_BODY_INSET, EMBED_HEADER + EMBED_BODY_INSET),
                Vec2::new(width - EMBED_BODY_INSET * 2.0, EMBED_PLACEHOLDER_H),
            );
            body_prims.push(Prim::Rect { rect: inner, fill: theme::BG_SECONDARY, radius: theme::RADIUS_CONTROL, stroke: None, scroll: None });
            let label = (ctx.shape)(inline::plain("Rendering…", theme::font_text(14.0, TextWeight::Semibold), 20.0, theme::TEXT, f32::INFINITY));
            let size = label.size();
            body_prims.push(Prim::Text { pos: inner.center() - size / 2.0, galley: label, links: Vec::new(), scroll: None });
            EMBED_PLACEHOLDER_H + EMBED_BODY_INSET * 2.0
        }
        EmbedBody::Source(source) => {
            let lines: Vec<&str> = source.trim_end_matches('\n').split('\n').collect();
            let h = (lines.len() as f32 * CODE_LINE + EMBED_BODY_INSET * 2.0).max(120.0);
            body_prims.push(Prim::Rect {
                rect: Rect::from_min_size(Pos2::new(0.0, EMBED_HEADER), Vec2::new(width, h)),
                fill: theme::BG_SECONDARY,
                radius: 0,
                stroke: None,
                scroll: None,
            });
            for (i, line) in lines.iter().enumerate() {
                let number = (ctx.shape)(inline::plain(&(i + 1).to_string(), mono(13.0), CODE_LINE, theme::TEXT_TERTIARY, f32::INFINITY));
                let nw = number.size().x;
                let y = EMBED_HEADER + EMBED_BODY_INSET + i as f32 * CODE_LINE;
                body_prims.push(Prim::Text { pos: Pos2::new(EMBED_BODY_INSET + 24.0 - nw, y), galley: number, links: Vec::new(), scroll: None });
                let galley = (ctx.shape)(inline::plain(line, mono(13.0), CODE_LINE, theme::TEXT_SECONDARY, f32::INFINITY));
                body_prims.push(Prim::Text { pos: Pos2::new(EMBED_BODY_INSET + 24.0 + 16.0, y), galley, links: Vec::new(), scroll: None });
            }
            h
        }
    };
    let h = EMBED_HEADER + body_h;
    let card = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, h));
    prims.push(Prim::Rect { rect: card, fill: theme::BG, radius: theme::RADIUS_XL, stroke: Some(Stroke::new(1.0, theme::BORDER_TRANSLUCENT)), scroll: None });
    let header = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, EMBED_HEADER));
    prims.push(Prim::Rect { rect: header, fill: theme::BG_TERTIARY.gamma_multiply(0.6), radius: 0, stroke: None, scroll: None });
    prims.push(Prim::Icon { icon, center: Pos2::new(16.0 + 8.0, EMBED_HEADER / 2.0), size: 16.0, color: theme::ICON_QUATERNARY });
    let label = (ctx.shape)(inline::plain(title, theme::font_text(12.0, TextWeight::Medium), 18.0, theme::TEXT, f32::INFINITY));
    prims.push(Prim::Text { pos: Pos2::new(16.0 + 16.0 + 8.0, (EMBED_HEADER - 18.0) / 2.0), galley: label, links: Vec::new(), scroll: None });
    prims.extend(body_prims);
    (prims, h)
}

fn table(ctx: &mut Ctx, scroll_id: usize, aligns: &[Align], header: &[Vec<Inline>], rows: &[Vec<Vec<Inline>>]) -> (Vec<Prim>, f32) {
    let width = ctx.width;
    let cols = header.len().max(1);
    let head_style = TextStyle { size: 12.0, line: 18.0, weight: TextWeight::Semibold, color: theme::TEXT_SECONDARY };
    let body_style = TextStyle { size: 14.0, line: 20.0, weight: TextWeight::Regular, color: theme::TEXT_SECONDARY };
    let natural = |cell: &[Inline], style: TextStyle| -> f32 { ctx.text(cell, style, f32::INFINITY).0.size().x };
    let mut col_w = vec![COL_MIN; cols];
    for (c, w) in col_w.iter_mut().enumerate() {
        let mut widest = header.get(c).map(|h| natural(h, head_style)).unwrap_or(0.0);
        for row in rows {
            if let Some(cell) = row.get(c) {
                widest = widest.max(natural(cell, body_style));
            }
        }
        *w = (widest + CELL_PAD_X * 2.0).clamp(COL_MIN, COL_MAX);
    }
    let total_w: f32 = col_w.iter().sum();
    let mut xs = vec![0.0; cols];
    for c in 1..cols {
        xs[c] = xs[c - 1] + col_w[c - 1];
    }
    let mut prims = Vec::new();
    let mut y = 0.0;
    let mut row_ys = Vec::new();
    let all_rows = std::iter::once((header, head_style)).chain(rows.iter().map(|r| (r.as_slice(), body_style)));
    let mut text_prims = Vec::new();
    for (r, (cells, style)) in all_rows.enumerate() {
        let mut row_h = ROW_MIN;
        let mut laid = Vec::new();
        for (c, w) in col_w.iter().enumerate().take(cols) {
            let cell = cells.get(c).map(Vec::as_slice).unwrap_or(&[]);
            let (galley, links) = ctx.text(cell, style, (w - CELL_PAD_X * 2.0).max(20.0));
            row_h = row_h.max(galley.size().y + CELL_PAD_Y * 2.0);
            laid.push((galley, links));
        }
        for (c, (galley, links)) in laid.into_iter().enumerate() {
            let cell_w = col_w[c] - CELL_PAD_X * 2.0;
            let dx = match aligns.get(c) {
                Some(Align::Center) => (cell_w - galley.size().x) / 2.0,
                Some(Align::Right) => cell_w - galley.size().x,
                _ => 0.0,
            };
            let ty = if r == 0 { y + (row_h - galley.size().y) / 2.0 } else { y + CELL_PAD_Y };
            text_prims.push(Prim::Text { pos: Pos2::new(xs[c] + CELL_PAD_X + dx.max(0.0), ty), galley, links, scroll: Some(scroll_id) });
        }
        row_ys.push((y, row_h));
        y += row_h;
    }
    let table_h = y;
    let outer = Rect::from_min_size(Pos2::ZERO, Vec2::new(total_w, table_h));
    prims.push(Prim::Rect { rect: outer, fill: theme::BG, radius: theme::RADIUS_XL, stroke: None, scroll: Some(scroll_id) });
    let (hy, hh) = row_ys[0];
    prims.push(Prim::Rect {
        rect: Rect::from_min_size(Pos2::new(0.0, hy), Vec2::new(total_w, hh)),
        fill: theme::BG_SECONDARY,
        radius: 0,
        stroke: None,
        scroll: Some(scroll_id),
    });
    for (i, (ry, rh)) in row_ys.iter().enumerate() {
        if i + 1 < row_ys.len() {
            prims.push(Prim::Rect {
                rect: Rect::from_min_size(Pos2::new(0.0, ry + rh - 1.0), Vec2::new(total_w, 1.0)),
                fill: theme::BORDER,
                radius: 0,
                stroke: None,
                scroll: Some(scroll_id),
            });
        }
    }
    for x in xs.iter().take(cols).skip(1) {
        prims.push(Prim::Rect {
            rect: Rect::from_min_size(Pos2::new(x - theme::BORDER_WIDTH, 0.0), Vec2::new(1.0, table_h)),
            fill: theme::BORDER,
            radius: 0,
            stroke: None,
            scroll: Some(scroll_id),
        });
    }
    prims.push(Prim::Rect {
        rect: outer,
        fill: Color32::TRANSPARENT,
        radius: theme::RADIUS_XL,
        stroke: Some(Stroke::new(1.0, theme::BORDER)),
        scroll: Some(scroll_id),
    });
    prims.extend(text_prims);
    let total_h = scroll_bar(&mut prims, scroll_id, width, table_h, total_w);
    (prims, total_h)
}

#[cfg(test)]
#[path = "reader_layout_tests.rs"]
mod tests;
