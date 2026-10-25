//! "Compare changes": a layer now against the snapshot taken when it was marked Ready for dev.

use crate::app::App;
use crate::panels::{clipped, scrollbar, wheel_scroll};
use crate::theme;
use crate::widgets::{self, elide};
use designos_doc::*;
use egui::{Align2, Color32, Context, Id, Key, Modifiers, Order, Pos2, Rect, Sense, TextureHandle, Vec2};

pub struct CompareView {
    pub node: NodeId,
    pub name: String,
    pub changes: Vec<NodeChange>,
    before_png: Vec<u8>,
    after_png: Vec<u8>,
    tex: Option<(Option<TextureHandle>, Option<TextureHandle>)>,
    scroll: f32,
}

const W: f32 = 820.0;
const PREVIEW: f32 = 300.0;

fn render(doc: &Document, id: NodeId, scale: f32) -> Vec<u8> {
    designos_export::export_png(doc, id, scale)
}

/// One scale for both previews, so differences in size show.
fn preview_scale(old: &Document, new: &Document, id: NodeId) -> f32 {
    let dim = |d: &Document| d.get(id).map(|n| n.size.x.max(n.size.y)).unwrap_or(1.0);
    (PREVIEW * 2.0 / dim(old).max(dim(new)).max(1.0)).min(2.0)
}

/// Builds the comparison for `id` and opens the view.
pub fn open(app: &mut App, id: NodeId) {
    let Some((old_doc, old_nodes)) = app.dev_snapshot(id) else {
        app.show_toast("There is no snapshot to compare with", true);
        return;
    };
    let changes = diff_subtree(&old_nodes, &app.doc, id);
    let name = app.doc.get(id).map(|n| n.name.clone()).unwrap_or_default();
    let scale = preview_scale(&old_doc, &app.doc, id);
    let before_png = render(&old_doc, id, scale);
    let after_png = render(&app.export_doc(), id, scale);
    app.dev.compare = Some(CompareView { node: id, name, changes, before_png, after_png, tex: None, scroll: 0.0 });
}

fn texture(ctx: &Context, name: &str, png: &[u8]) -> Option<TextureHandle> {
    let img = image::load_from_memory(png).ok()?.to_rgba8();
    let ci = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
    Some(ctx.load_texture(name, ci, egui::TextureOptions::LINEAR))
}

fn pane(p: &egui::Painter, ctx: &Context, rect: Rect, title: &str, tex: Option<&TextureHandle>) {
    p.text(Pos2::new(rect.left(), rect.top() + 10.0), Align2::LEFT_CENTER, title, theme::font_body_strong(), theme::TEXT);
    let area = Rect::from_min_max(Pos2::new(rect.left(), rect.top() + 24.0), rect.max);
    p.rect_filled(area, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
    match tex {
        Some(t) => {
            let sz = t.size_vec2();
            let k = ((area.width() - 16.0) / sz.x).min((area.height() - 16.0) / sz.y).min(1.0);
            let r = Rect::from_center_size(area.center(), sz * k);
            p.image(t.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
        None => {
            p.text(area.center(), Align2::CENTER_CENTER, "No preview", theme::font_body(), theme::TEXT_TERTIARY);
        }
    }
    let _ = ctx;
}

/// Total height of the change list.
fn list_height(changes: &[NodeChange]) -> f32 {
    changes
        .iter()
        .map(|c| match &c.kind {
            ChangeKind::Modified(p) => 26.0 + p.len() as f32 * 20.0 + 6.0,
            _ => 26.0,
        })
        .sum()
}

/// Draws the Compare changes view when it is open. Call once per frame.
pub fn show(app: &mut App, ctx: &Context) {
    let Some(mut v) = app.dev.compare.take() else { return };
    let screen = ctx.content_rect();
    let esc = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
    if v.tex.is_none() {
        v.tex = Some((texture(ctx, "dev-before", &v.before_png), texture(ctx, "dev-after", &v.after_png)));
    }
    let h = (screen.height() - 80.0).clamp(380.0, 640.0);
    let rect = Rect::from_center_size(screen.center(), Vec2::new(W.min(screen.width() - 40.0), h));
    let mut close = esc;
    let mut remark = false;
    egui::Area::new(Id::new("dev-compare")).order(Order::Foreground).fixed_pos(screen.min).interactable(true).show(ctx, |ui| {
        let scrim = ui.allocate_rect(screen, Sense::click_and_drag());
        let p = ui.painter().clone();
        p.rect_filled(screen, 0.0, Color32::from_black_alpha(140));
        if scrim.clicked() && !rect.contains(scrim.interact_pointer_pos().unwrap_or(Pos2::ZERO)) {
            close = true;
        }
        widgets::menu_panel(&p, rect, theme::RADIUS_FRAME);
        let pad = 20.0;
        let title = format!("Compare changes \u{2022} {}", v.name);
        p.text(
            Pos2::new(rect.left() + pad, rect.top() + 28.0),
            Align2::LEFT_CENTER,
            elide(&p, &title, &theme::font_large_strong(), rect.width() - 80.0),
            theme::font_large_strong(),
            theme::TEXT,
        );
        let x = Rect::from_min_size(Pos2::new(rect.right() - 44.0, rect.top() + 14.0), Vec2::splat(28.0));
        if widgets::icon_button(ui, x, "compare-close", crate::icons::Icon::Close, false, "Close").clicked() {
            close = true;
        }
        let top = rect.top() + 56.0;
        let pw = (rect.width() - pad * 2.0 - 16.0) / 2.0;
        let ph = PREVIEW + 24.0;
        let (before, after) = v.tex.as_ref().map(|(b, a)| (b.as_ref(), a.as_ref())).unwrap_or((None, None));
        pane(&p, ctx, Rect::from_min_size(Pos2::new(rect.left() + pad, top), Vec2::new(pw, ph)), "When marked Ready for dev", before);
        pane(&p, ctx, Rect::from_min_size(Pos2::new(rect.left() + pad + pw + 16.0, top), Vec2::new(pw, ph)), "Now", after);

        let summary_y = top + ph + 14.0;
        let (mut added, mut removed, mut modified) = (0, 0, 0);
        for c in &v.changes {
            match c.kind {
                ChangeKind::Added => added += 1,
                ChangeKind::Removed => removed += 1,
                ChangeKind::Modified(_) => modified += 1,
            }
        }
        let summary = if v.changes.is_empty() { "No changes".to_string() } else { format!("{modified} changed, {added} added, {removed} removed") };
        p.text(Pos2::new(rect.left() + pad, summary_y + 8.0), Align2::LEFT_CENTER, summary, theme::font_body_strong(), theme::TEXT);

        let btn_h = 32.0;
        let list = Rect::from_min_max(Pos2::new(rect.left() + pad, summary_y + 22.0), Pos2::new(rect.right() - pad, rect.bottom() - pad - btn_h - 12.0));
        let content_h = list_height(&v.changes);
        wheel_scroll(ui, list, &mut v.scroll, content_h);
        let scroll = v.scroll;
        clipped(ui, list, |ui| {
            let p = ui.painter().clone();
            let mut y = list.top() - scroll;
            for c in &v.changes {
                let (mark, col, label) = match &c.kind {
                    ChangeKind::Added => ("+", theme::DEV_READY, "added"),
                    ChangeKind::Removed => ("-", theme::DANGER, "removed"),
                    ChangeKind::Modified(_) => ("\u{2022}", theme::DEV_CHANGED, "changed"),
                };
                p.text(Pos2::new(list.left() + 6.0, y + 13.0), Align2::LEFT_CENTER, mark, theme::font_body_strong(), col);
                p.text(
                    Pos2::new(list.left() + 22.0, y + 13.0),
                    Align2::LEFT_CENTER,
                    elide(&p, &c.name, &theme::font_body_strong(), list.width() - 120.0),
                    theme::font_body_strong(),
                    theme::TEXT,
                );
                p.text(Pos2::new(list.right() - 12.0, y + 13.0), Align2::RIGHT_CENTER, label, theme::font_caption(), theme::TEXT_SECONDARY);
                y += 26.0;
                if let ChangeKind::Modified(props) = &c.kind {
                    for pc in props {
                        p.text(Pos2::new(list.left() + 22.0, y + 10.0), Align2::LEFT_CENTER, &pc.property, theme::font_caption(), theme::TEXT_SECONDARY);
                        let before = elide(&p, &pc.before, &theme::font_caption(), 150.0);
                        let after = elide(&p, &pc.after, &theme::font_caption(), 150.0);
                        p.text(Pos2::new(list.left() + 170.0, y + 10.0), Align2::LEFT_CENTER, before, theme::font_caption(), theme::DANGER);
                        p.text(Pos2::new(list.left() + 330.0, y + 10.0), Align2::LEFT_CENTER, "\u{2192}", theme::font_caption(), theme::TEXT_TERTIARY);
                        p.text(Pos2::new(list.left() + 350.0, y + 10.0), Align2::LEFT_CENTER, after, theme::font_caption(), theme::DEV_READY);
                        y += 20.0;
                    }
                    y += 6.0;
                }
            }
        });
        scrollbar(ui, list, scroll, content_h);

        let by = rect.bottom() - pad - btn_h;
        let ok = Rect::from_min_size(Pos2::new(rect.right() - pad - 190.0, by), Vec2::new(190.0, btn_h));
        let cl = Rect::from_min_size(Pos2::new(ok.left() - 88.0, by), Vec2::new(80.0, btn_h));
        if crate::panels::inspect::small_button(ui, cl, "compare-done", "Close") {
            close = true;
        }
        let r = ui.interact(ok, Id::new("compare-remark"), Sense::click());
        p.rect_filled(ok, theme::cr_pill(ok.height()), if r.hovered() { theme::BG_BRAND_HOVER } else { theme::BG_BRAND });
        p.text(ok.center(), Align2::CENTER_CENTER, "Mark as ready for dev again", theme::font_body_strong(), theme::TEXT);
        if r.clicked() {
            remark = true;
        }
    });
    if remark {
        let id = v.node;
        app.dev_mark_ready(&[id]);
        return;
    }
    if !close {
        app.dev.compare = Some(v);
    }
}
