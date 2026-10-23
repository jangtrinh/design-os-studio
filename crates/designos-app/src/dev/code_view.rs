//! Code and prompt blocks of the Inspect panel.
//!
//! Code keeps one row per source line, so a token is never split: a line wider than the box scrolls sideways (trackpad,
//! Shift + wheel, dragging the code, or the thumb that shows how much is hidden). The MCP prompt is prose plus a link and
//! wraps inside its box, so all of it reads without scrolling.

use super::highlight::{self, Span};
use super::Language;
use crate::panels::inspect::{Cur, PAD};
use crate::theme;
use crate::widgets::{self, text_width};
use egui::text::{LayoutJob, TextFormat};
use egui::{Align2, CursorIcon, Id, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use std::borrow::Cow;
use std::hash::{DefaultHasher, Hash, Hasher};

/// How far the code box is scrolled sideways, and which code that offset belongs to.
#[derive(Clone, Copy)]
struct SideScroll {
    code: u64,
    x: f32,
}

fn mono_char_w(p: &egui::Painter) -> f32 {
    text_width(p, "MMMMMMMMMM", &theme::font_mono()) / 10.0
}

fn code_key(lang: Language, code: &str) -> u64 {
    let mut h = DefaultHasher::new();
    lang.label().hash(&mut h);
    code.hash(&mut h);
    h.finish()
}

/// Where the code box, its text area and its scroll thumb sit.
struct SideView {
    /// The whole box.
    r: Rect,
    /// The text area: the box without its side insets.
    view: Rect,
    /// How far the code can scroll; 0 when every line fits.
    max_x: f32,
    thumb_w: f32,
}

impl SideView {
    fn new(r: Rect, view: Rect, content_w: f32) -> Self {
        let max_x = (content_w - view.width()).max(0.0);
        // as wide as the share of the code the box shows, but never narrower than a pointer target
        let thumb_w = (view.width() * view.width() / content_w.max(1.0)).clamp(theme::TARGET_MIN.min(view.width()), view.width());
        Self { r, view, max_x, thumb_w }
    }

    /// How far the thumb can travel along the text area.
    fn travel(&self) -> f32 {
        self.view.width() - self.thumb_w
    }

    /// The thumb for the code scrolled to `x`: a bar one 4px step tall with one step of padding above and below it.
    fn thumb(&self, x: f32) -> Rect {
        let share = if self.max_x > 0.0 { x / self.max_x } else { 0.0 };
        let top = self.r.bottom() - theme::SPACE_FACT - theme::SPACE_FACT;
        Rect::from_min_size(Pos2::new(self.view.left() + self.travel() * share, top), Vec2::new(self.thumb_w, theme::SPACE_FACT))
    }

    /// The pointer target of the thumb: as wide as the thumb, `TARGET_MIN` tall, reaching from the bottom of the box up
    /// over the lower part of the last row.
    fn grab(&self, x: f32) -> Rect {
        let thumb = self.thumb(x);
        Rect::from_min_max(Pos2::new(thumb.left(), self.r.bottom() - theme::TARGET_MIN), Pos2::new(thumb.right(), self.r.bottom()))
    }
}

/// The sideways offset of the code for this frame, kept inside `0..=max_x`. Dragging the thumb moves it with the pointer,
/// scaled to the thumb's track; dragging anywhere else on the box pans the code; so does a mostly sideways wheel or
/// trackpad swipe over it. A new code (another layer or language) starts at the first column. Returns the offset and
/// whether the pointer is on the thumb.
fn side_scroll(ui: &Ui, side: &SideView, key: u64) -> (f32, bool) {
    let state_id = Id::new("dev-code-side-scroll");
    let mut x = ui.ctx().data(|d| d.get_temp::<SideScroll>(state_id)).filter(|s| s.code == key).map_or(0.0, |s| s.x);
    let mut on_thumb = false;
    if side.max_x > 0.0 {
        let pan = ui.interact(side.r, ui.id().with("dev-code-pan"), Sense::drag());
        // added after the pan, so it takes the pointer where the two overlap
        let thumb = ui.interact(side.grab(x), ui.id().with("dev-code-thumb"), Sense::drag());
        if thumb.dragged() {
            if side.travel() > 0.0 {
                x += thumb.drag_delta().x * side.max_x / side.travel();
            }
        } else if pan.dragged() {
            x -= pan.drag_delta().x;
        }
        // a mostly vertical swipe belongs to the panel, which scrolls up and down
        let swipe = ui.input(|i| i.smooth_scroll_delta);
        if ui.rect_contains_pointer(side.r) && swipe.x.abs() > swipe.y.abs() {
            x -= swipe.x;
        }
        on_thumb = thumb.hovered() || thumb.dragged();
        if pan.dragged() {
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        } else if pan.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::Grab);
        }
    }
    x = x.clamp(0.0, side.max_x);
    ui.ctx().data_mut(|d| d.insert_temp(state_id, SideScroll { code: key, x }));
    (x, on_thumb)
}

/// The part of the span `s` (starting `left` px from the origin of the painter) that falls inside `view`, with the x it
/// starts at. A span that is far wider than the box lays out only what shows.
fn visible_part<'a>(s: &'a Span, left: f32, cw: f32, view: Rect) -> Option<(Cow<'a, str>, f32)> {
    let n = s.text.chars().count();
    let from = ((view.left() - left) / cw).floor().max(0.0) as usize;
    let to = (((view.right() - left) / cw).ceil().max(0.0) as usize).min(n);
    if from >= to {
        return None;
    }
    let text = if from == 0 && to == n { Cow::Borrowed(s.text.as_str()) } else { Cow::Owned(s.text.chars().skip(from).take(to - from).collect()) };
    Some((text, left + from as f32 * cw))
}

/// Syntax-highlighted code, one row per source line, scrolling sideways when a line is wider than the box.
pub fn code_block(ui: &mut Ui, c: &mut Cur, lang: Language, code: &str) {
    let font = theme::font_mono();
    let line_h = theme::CODE_LINE_HEIGHT;
    let inset = theme::INSET_CONTROL;
    let cw = mono_char_w(ui.painter());
    let rows = highlight::highlight(lang, code);
    let widest = rows.iter().map(|row| highlight::line_len(row)).max().unwrap_or(0);
    let content_w = widest as f32 * cw;
    let box_w = c.w - 2.0 * PAD;
    // a box with a hidden part keeps one more 4px step under the code, for the thumb and its padding
    let strip = if content_w > box_w - 2.0 * inset { theme::SPACE_FACT } else { 0.0 };
    let h = rows.len() as f32 * line_h + 2.0 * inset + strip;
    let r = c.rect(PAD, 0.0, box_w, h);
    widgets::record(ui, "dev-code-block", r);
    // the code never runs into the box's padding or its rounded corners
    let view = Rect::from_min_max(Pos2::new(r.left() + inset, r.top()), Pos2::new(r.right() - inset, r.bottom()));
    let side = SideView::new(r, view, content_w);
    let (x, on_thumb) = side_scroll(ui, &side, code_key(lang, code));

    ui.painter().rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
    let shown = ui.clip_rect();
    let clip = ui.painter().with_clip_rect(view.intersect(shown));
    let first = ((shown.top() - r.top() - inset) / line_h).floor().max(0.0) as usize;
    for (i, row) in rows.iter().enumerate().skip(first) {
        let y = r.top() + inset + i as f32 * line_h;
        if y > shown.bottom() {
            break;
        }
        let mut col = 0usize;
        for s in row {
            let left = view.left() - x + col as f32 * cw;
            col += s.text.chars().count();
            if let Some((text, at)) = visible_part(s, left, cw, view) {
                clip.text(Pos2::new(at, y + line_h / 2.0), Align2::LEFT_CENTER, text, font.clone(), highlight::color(s.tok));
            }
        }
    }
    if side.max_x > 0.0 {
        // how much of the code the box shows, under the last row; it drags like the panel's own scrollbar thumb
        let thumb = side.thumb(x);
        widgets::record(ui, "dev-code-thumb", thumb);
        widgets::record(ui, "dev-code-thumb-grab", side.grab(x));
        let color = if on_thumb { theme::TEXT_SECONDARY } else { theme::TEXT_TERTIARY };
        ui.painter().rect_filled(thumb, theme::cr(theme::RADIUS_MARK), color);
    }
    c.y += h + theme::SPACE_INLINE;
}

/// The MCP prompt in a framed box: `lead` in plain text over `link` in the link colour. It wraps to the box width (a
/// link longer than the box breaks where it must), and the box grows to hold all of it.
pub fn prompt_box(ui: &mut Ui, c: &mut Cur, lead: &str, link: &str) {
    let inset = theme::INSET_CONTROL;
    let box_w = c.w - 2.0 * PAD;
    let format = |color| TextFormat { font_id: theme::font_mono(), color, line_height: Some(theme::CODE_LINE_HEIGHT), ..Default::default() };
    let mut job = LayoutJob::default();
    job.wrap.max_width = box_w - 2.0 * inset;
    job.append(&format!("{lead}\n"), 0.0, format(theme::TEXT));
    job.append(link, 0.0, format(theme::INFO));
    let galley = ui.painter().layout_job(job);
    let r = c.rect(PAD, 0.0, box_w, inset + galley.size().y + inset);
    widgets::record(ui, "dev-mcp-prompt", r);
    ui.painter().rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
    ui.painter().rect_stroke(r, theme::cr(theme::RADIUS_CONTROL), Stroke::new(theme::BORDER_WIDTH, theme::BORDER_STRONG), StrokeKind::Inside);
    ui.painter().galley(r.min + Vec2::splat(inset), galley, theme::TEXT);
    c.y += r.height();
}
