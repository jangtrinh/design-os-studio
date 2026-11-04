//! The reading column: lays the open page out once, then paints, scrolls and hit-tests it.

use super::inline::{link_at, LinkSpan};
use super::links;
use super::ops::TreeSnapshot;
use super::reader_layout::{self, half_leading, HeadingPos, Laid, Prim};
use super::service::LoadedPage;
use crate::icons;
use crate::theme;
use crate::widgets::kit;
use designos_docs::ContentHash;
use egui::{Context, CursorIcon, Pos2, Rect, Sense, Ui, Vec2};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// Room under the last block, so the end of a page is never flush with the window.
const BOTTOM_PAD: f32 = 96.0;
/// A heading counts as the one being read once it is within this distance of the column top.
const SPY_LINE: f32 = theme::SPACE_REGION + 1.0;
const SCROLL_SECONDS: f64 = 0.2;
const HBAR_THUMB_H: f32 = 7.0;

#[derive(Clone, Debug, PartialEq)]
pub enum ReaderAction {
    /// A link to another page of this project's docs.
    OpenPage {
        rel: String,
        anchor: Option<String>,
    },
    OpenUrl(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct LayoutKey {
    /// The bytes of the open page: a re-read with the same content is not a change.
    page: ContentHash,
    width: i32,
    ppp: i32,
    /// What the layout takes from the project: the names of its design files (an embed shows its file's name). Edits to
    /// other pages change the snapshot but not this, so they never lay the open page out again.
    files: u64,
    notes: u64,
}

struct ScrollAnim {
    from: f32,
    to: f32,
    start: f64,
}

#[derive(Default)]
pub struct Reader {
    pub scroll: f32,
    anim: Option<ScrollAnim>,
    laid: Option<(LayoutKey, Arc<Laid>)>,
    hscroll: HashMap<usize, f32>,
    /// The heading the reader clicked in "On this page"; it stays active until the page is scrolled by hand.
    spy_override: Option<usize>,
    pending_anchor: Option<String>,
    /// Content height and viewport height of the last frame, for clamping a scroll target.
    content_h: f32,
    viewport_h: f32,
}

fn files_hash(snapshot: &TreeSnapshot) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for f in &snapshot.files {
        (&f.id, &f.name).hash(&mut h);
    }
    h.finish()
}

fn notes_hash(notes: &[String]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    notes.hash(&mut h);
    h.finish()
}

impl Reader {
    /// A different page is shown: back to the top.
    pub fn reset(&mut self) {
        self.scroll = 0.0;
        self.anim = None;
        self.laid = None;
        self.hscroll.clear();
        self.spy_override = None;
    }

    pub fn headings(&self) -> &[HeadingPos] {
        self.laid.as_ref().map(|(_, l)| l.headings.as_slice()).unwrap_or(&[])
    }

    /// The heading nearest the top of the column.
    pub fn active_heading(&self) -> Option<usize> {
        let headings = self.headings();
        if headings.is_empty() {
            return None;
        }
        if let Some(i) = self.spy_override.filter(|i| *i < headings.len()) {
            return Some(i);
        }
        let max = (self.content_h - self.viewport_h).max(0.0);
        if max > 0.0 && self.scroll >= max - 0.5 {
            return Some(headings.len() - 1);
        }
        let line = self.scroll + SPY_LINE;
        Some(headings.iter().rposition(|h| h.y <= line).unwrap_or(0))
    }

    /// Scrolls so heading `index` sits at the top of the column, smoothly unless motion is off.
    pub fn scroll_to_heading(&mut self, ctx: &Context, index: usize) {
        let Some(y) = self.headings().get(index).map(|h| h.y) else { return };
        self.spy_override = Some(index);
        let max = (self.content_h - self.viewport_h).max(0.0);
        let target = y.clamp(0.0, max);
        if ctx.global_style().animation_time <= 0.0 {
            self.scroll = target;
            self.anim = None;
        } else {
            self.anim = Some(ScrollAnim { from: self.scroll, to: target, start: ctx.input(|i| i.time) });
            ctx.request_repaint();
        }
    }

    /// Jumps to the heading whose slug is `anchor` once the page is laid out.
    pub fn scroll_to_anchor(&mut self, anchor: String) {
        self.pending_anchor = Some(anchor);
    }

    fn ensure_layout(&mut self, ctx: &Context, page: &LoadedPage, snapshot: &Arc<TreeSnapshot>, notes: &[String], width: f32) {
        let key = LayoutKey {
            page: page.hash,
            width: width.round() as i32,
            ppp: (ctx.pixels_per_point() * 100.0).round() as i32,
            files: files_hash(snapshot),
            notes: notes_hash(notes),
        };
        if self.laid.as_ref().is_some_and(|(k, _)| *k == key) {
            return;
        }
        let shaper = ctx.clone();
        let shape = move |job: egui::text::LayoutJob| shaper.fonts_mut(|f| f.layout_job(job));
        let laid = reader_layout::layout(&shape, &page.rel, &page.page, snapshot, notes, width);
        self.laid = Some((key, Arc::new(laid)));
    }

    /// Paints the page and returns what the reader clicked.
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        ui: &mut Ui,
        main: Rect,
        column: Rect,
        page: &LoadedPage,
        snapshot: &Arc<TreeSnapshot>,
        notes: &[String],
        interactive: bool,
    ) -> Vec<ReaderAction> {
        let ctx = ui.ctx().clone();
        self.ensure_layout(&ctx, page, snapshot, notes, column.width());
        let Some((_, laid)) = self.laid.clone() else { return Vec::new() };
        self.viewport_h = main.height();
        self.content_h = theme::SPACE_REGION + laid.height + BOTTOM_PAD;
        if let Some(anchor) = self.pending_anchor.take() {
            if let Some(i) = laid.headings.iter().position(|h| h.slug == anchor) {
                self.scroll_to_heading(&ctx, i);
            }
        }
        self.step_scroll(ui, main);
        let top = column.top() - self.scroll;
        let clip = Rect::from_min_max(Pos2::new(column.left(), main.top()), Pos2::new(column.right(), main.bottom()));
        let painter = ui.painter().with_clip_rect(clip);
        let pointer = ui.input(|i| i.pointer.hover_pos()).filter(|p| interactive && ui.rect_contains_pointer(clip) && clip.contains(*p));
        let clicked = interactive && ui.input(|i| i.pointer.primary_clicked());
        let mut actions = Vec::new();
        let mut hand = false;
        let visible = (main.top() - top, main.bottom() - top);
        for item in laid.items.iter().filter(|i| i.y + i.h >= visible.0 && i.y <= visible.1) {
            for prim in &item.prims {
                match prim {
                    Prim::Text { pos, galley, links, scroll } => {
                        let at = Pos2::new(column.left() + pos.x - self.offset(*scroll), top + pos.y + half_leading(galley));
                        painter.galley(at, galley.clone(), theme::TEXT_SECONDARY);
                        if let (Some(p), false) = (pointer, links.is_empty()) {
                            if let Some(link) = hit_link(galley, links, at, p) {
                                hand = true;
                                if clicked {
                                    actions.push(link_action(&page.rel, &link.target));
                                }
                            }
                        }
                    }
                    Prim::Rect { rect, fill, radius, stroke, scroll } => {
                        let r = rect.translate(Vec2::new(column.left() - self.offset(*scroll), top));
                        if *fill != egui::Color32::TRANSPARENT {
                            painter.rect_filled(r, theme::cr(*radius), *fill);
                        }
                        if let Some(s) = stroke {
                            painter.rect_stroke(r, theme::cr(*radius), *s, egui::StrokeKind::Inside);
                        }
                    }
                    Prim::Icon { icon, center, size, color } => {
                        icons::paint(&painter, *icon, Pos2::new(column.left() + center.x, top + center.y), *size, *color);
                    }
                    Prim::Disc { center, radius, color } => {
                        painter.circle_filled(Pos2::new(column.left() + center.x, top + center.y), *radius, *color);
                    }
                    Prim::Check { rect, checked } => {
                        let r = rect.translate(Vec2::new(column.left(), top));
                        let look = kit::checkbox_look(if *checked { kit::State::Selected } else { kit::State::Default });
                        kit::paint_frame(&painter, r, theme::SPACE_FACT as u8, &look);
                        if *checked {
                            icons::paint(&painter, icons::Icon::Check, r.center(), theme::FONT_SIZE_TEXT_XS, look.icon);
                        }
                    }
                    Prim::HBar { id, track, view, content_w } => {
                        let shift = Vec2::new(column.left(), top);
                        self.hbar(ui, &painter, *id, track.translate(shift), view.translate(shift), *content_w, interactive);
                    }
                }
            }
        }
        if hand {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
        }
        crate::panels::scrollbar(ui, main, self.scroll, self.content_h);
        if let Some(s) = crate::panels::take_scrollbar_drag(ui, main) {
            self.scroll = s;
            self.spy_override = None;
        }
        actions
    }

    fn offset(&self, scroll: Option<usize>) -> f32 {
        scroll.and_then(|id| self.hscroll.get(&id).copied()).unwrap_or(0.0)
    }

    fn step_scroll(&mut self, ui: &Ui, main: Rect) {
        let ctx = ui.ctx();
        let max = (self.content_h - main.height()).max(0.0);
        if let Some(anim) = &self.anim {
            let t = ((ctx.input(|i| i.time) - anim.start) / SCROLL_SECONDS).clamp(0.0, 1.0) as f32;
            let eased = 1.0 - (1.0 - t).powi(3);
            self.scroll = anim.from + (anim.to - anim.from) * eased;
            if t >= 1.0 {
                self.anim = None;
            } else {
                ctx.request_repaint();
            }
        }
        // The wheel (and a drag of the scroll bar) takes over from an animation and from a clicked heading.
        let wheel = ui.rect_contains_pointer(main) && ui.input(|i| i.smooth_scroll_delta.y != 0.0);
        if wheel {
            self.anim = None;
            self.spy_override = None;
        }
        let mut s = self.scroll;
        crate::panels::wheel_scroll(ui, main, &mut s, self.content_h);
        self.scroll = s.clamp(0.0, max);
    }

    /// The scroll bar under a table or code block, and the wheel / drag that moves it.
    #[allow(clippy::too_many_arguments)]
    fn hbar(&mut self, ui: &mut Ui, painter: &egui::Painter, id: usize, track: Rect, block: Rect, content_w: f32, interactive: bool) {
        let viewport = track.width();
        let max = (content_w - viewport).max(0.0);
        let mut offset = self.hscroll.get(&id).copied().unwrap_or(0.0).clamp(0.0, max);
        let thumb_w = (viewport * viewport / content_w).clamp(24.0, viewport);
        let resp = ui.interact(track, ui.id().with(("project-hbar", id)), Sense::click_and_drag());
        if interactive && (resp.dragged() || resp.clicked()) {
            if let Some(pos) = resp.interact_pointer_pos() {
                let t = ((pos.x - track.left() - thumb_w / 2.0) / (viewport - thumb_w).max(1.0)).clamp(0.0, 1.0);
                offset = t * max;
            }
        }
        if interactive && ui.rect_contains_pointer(block.union(track)) {
            let dx = ui.input(|i| i.smooth_scroll_delta.x);
            offset = (offset - dx).clamp(0.0, max);
        }
        self.hscroll.insert(id, offset);
        let t = if max > 0.0 { offset / max } else { 0.0 };
        let thumb =
            Rect::from_min_size(Pos2::new(track.left() + (viewport - thumb_w) * t, track.top() + theme::INSET_SEGMENT), Vec2::new(thumb_w, HBAR_THUMB_H));
        painter.rect_filled(thumb, theme::cr(theme::READER_HBAR_THUMB_RADIUS), theme::VAR_SCROLL_THUMB);
    }
}

fn hit_link<'a>(galley: &egui::Galley, links: &'a [LinkSpan], at: Pos2, pointer: Pos2) -> Option<&'a LinkSpan> {
    let local = pointer - at.to_vec2();
    let rect = Rect::from_min_size(Pos2::ZERO, galley.size());
    if !rect.contains(local) {
        return None;
    }
    let cursor = galley.cursor_from_pos(local.to_vec2());
    link_at(links, cursor.index.0)
}

fn link_action(page_rel: &str, target: &str) -> ReaderAction {
    if let Some(rel) = links::page_target(page_rel, target) {
        return ReaderAction::OpenPage { rel, anchor: links::anchor_of(target) };
    }
    let dest = target.split_whitespace().next().unwrap_or("").trim_matches(['<', '>']);
    if let Some(anchor) = dest.strip_prefix('#') {
        return ReaderAction::OpenPage { rel: page_rel.to_string(), anchor: Some(anchor.to_string()) };
    }
    ReaderAction::OpenUrl(dest.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_become_page_jumps_anchors_or_urls() {
        assert_eq!(link_action("overview.md", "spec/a.md#fees"), ReaderAction::OpenPage { rel: "spec/a.md".into(), anchor: Some("fees".into()) });
        assert_eq!(link_action("spec/a.md", "#fees"), ReaderAction::OpenPage { rel: "spec/a.md".into(), anchor: Some("fees".into()) });
        assert_eq!(link_action("a.md", "https://x.dev/y \"t\""), ReaderAction::OpenUrl("https://x.dev/y".into()));
    }
}

#[cfg(test)]
mod layout_cache_tests {
    use super::*;
    use crate::tabs::project_tab::ops::FileInfo;
    use designos_docs::{ContentHash, Page};

    fn loaded(text: &str, rev: u64) -> LoadedPage {
        LoadedPage { rel: "spec/a.md".into(), hash: ContentHash::of(text.as_bytes()), page: Arc::new(Page::parse(text)), rev, oversize: false }
    }

    /// A context whose fonts are ready, and the reader's layout of `page` against `snapshot`.
    fn laid_once(reader: &mut Reader, ctx: &Context, page: &LoadedPage, snapshot: &Arc<TreeSnapshot>) -> Arc<Laid> {
        reader.ensure_layout(ctx, page, snapshot, &[], 690.0);
        reader.laid.as_ref().expect("laid").1.clone()
    }

    fn ready_ctx() -> Context {
        let ctx = Context::default();
        crate::theme::install_fonts(&ctx);
        let _ = ctx.run_ui(egui::RawInput::default(), |_| {});
        ctx
    }

    #[test]
    fn a_new_snapshot_that_changed_other_pages_does_not_lay_the_open_page_out_again() {
        let ctx = ready_ctx();
        let mut reader = Reader::default();
        let page = loaded("---\ntitle: A\n---\n\n## One\n\ntext\n", 1);
        let before = Arc::new(TreeSnapshot::empty());
        let first = laid_once(&mut reader, &ctx, &page, &before);
        // Another page was edited: a new snapshot (a new Arc), the same design files, the same open page.
        let mut other = TreeSnapshot::empty();
        other.manifest.group_mut(designos_docs::Group::Spec).pages.push("spec/other.md".into());
        let after = Arc::new(other);
        let second = laid_once(&mut reader, &ctx, &page, &after);
        assert!(Arc::ptr_eq(&first, &second), "the layout was reused");
        // Re-reading the same bytes (a touch) is not a change either.
        let reread = loaded("---\ntitle: A\n---\n\n## One\n\ntext\n", 2);
        assert!(Arc::ptr_eq(&first, &laid_once(&mut reader, &ctx, &reread, &after)));
    }

    #[test]
    fn a_changed_page_a_changed_width_or_a_renamed_design_file_lays_out_again() {
        let ctx = ready_ctx();
        let mut reader = Reader::default();
        let snapshot = Arc::new(TreeSnapshot::empty());
        let first = laid_once(&mut reader, &ctx, &loaded("## One\n", 1), &snapshot);
        assert!(!Arc::ptr_eq(&first, &laid_once(&mut reader, &ctx, &loaded("## One\n\nmore\n", 2), &snapshot)), "new content");
        let before = reader.laid.as_ref().unwrap().1.clone();
        reader.ensure_layout(&ctx, &loaded("## One\n\nmore\n", 2), &snapshot, &[], 500.0);
        assert!(!Arc::ptr_eq(&before, &reader.laid.as_ref().unwrap().1), "new width");
        let mut renamed = TreeSnapshot::empty();
        renamed.files.push(FileInfo { id: "f1".into(), name: "Checkout-web".into() });
        let before = reader.laid.as_ref().unwrap().1.clone();
        reader.ensure_layout(&ctx, &loaded("## One\n\nmore\n", 2), &Arc::new(renamed), &[], 500.0);
        assert!(!Arc::ptr_eq(&before, &reader.laid.as_ref().unwrap().1), "an embed shows its file's name");
    }
}
