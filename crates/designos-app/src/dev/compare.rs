//! "Compare changes": a layer now against the snapshot taken when it was marked Ready for dev.

use crate::app::App;
use crate::panels::{clipped, scrollbar, wheel_scroll};
use crate::theme;
use crate::widgets::{self, elide};
use designos_doc::*;
use egui::{Align2, Color32, Context, Id, Key, Modifiers, Order, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle, Vec2};

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
/// Height of the title row; the title and the close button share its centre line.
const HEADER_H: f32 = 56.0;
/// The footer buttons never get narrower than these, so a short label does not shrink them.
const MIN_PRIMARY_W: f32 = 190.0;
const MIN_CLOSE_W: f32 = 80.0;
/// Inset of the dialog content from its edge, on all four sides.
const PAD: f32 = 20.0;

/// The close button, centred on the title row.
fn close_rect(dialog: Rect) -> Rect {
    let y = dialog.top() + (HEADER_H - theme::CONTROL_SM) * 0.5;
    Rect::from_min_size(Pos2::new(dialog.right() - PAD - theme::CONTROL_SM, y), Vec2::splat(theme::CONTROL_SM))
}

fn render(doc: &Document, id: NodeId, scale: f32) -> Vec<u8> {
    designos_export::export_png_srgb(doc, id, scale)
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
    p.rect_stroke(area, theme::cr(theme::RADIUS_CONTROL), Stroke::new(theme::BORDER_WIDTH, theme::BORDER), StrokeKind::Inside);
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
        if scrim.clicked() && !rect.contains(scrim.interact_pointer_pos().unwrap_or(Pos2::ZERO)) {
            close = true;
        }
        crate::widgets::kit::modal_surface(&p, screen, rect);
        let pad = PAD;
        let title = format!("Compare changes \u{2022} {}", v.name);
        p.text(
            Pos2::new(rect.left() + pad, rect.top() + HEADER_H * 0.5),
            Align2::LEFT_CENTER,
            elide(&p, &title, &theme::font_large_strong(), rect.width() - 80.0),
            theme::font_large_strong(),
            theme::TEXT,
        );
        let x = close_rect(rect);
        if crate::widgets::kit::button_icon(
            ui,
            x,
            "compare-close",
            crate::icons::Icon::Close,
            crate::widgets::kit::Hierarchy::Close,
            crate::widgets::kit::Size::Sm,
            true,
            false,
        )
        .on_hover_text("Close")
        .clicked()
        {
            close = true;
        }
        let top = rect.top() + HEADER_H;
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

        let btn_h = theme::CONTROL_MD;
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
                        p.text(Pos2::new(list.left() + 170.0, y + 10.0), Align2::LEFT_CENTER, before, theme::font_caption(), theme::TEXT_ERROR);
                        p.text(Pos2::new(list.left() + 330.0, y + 10.0), Align2::LEFT_CENTER, "\u{2192}", theme::font_caption(), theme::TEXT_TERTIARY);
                        p.text(Pos2::new(list.left() + 350.0, y + 10.0), Align2::LEFT_CENTER, after, theme::font_caption(), theme::TEXT_SUCCESS);
                        y += 20.0;
                    }
                    y += 6.0;
                }
            }
        });
        scrollbar(ui, list, scroll, content_h);

        let by = rect.bottom() - pad - btn_h;
        let primary_label = "Mark as ready for dev again";
        let primary_font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold);
        let primary_w = (widgets::text_width(&p, primary_label, &primary_font).ceil() + 2.0 * theme::INSET_CONTROL).max(MIN_PRIMARY_W);
        let close_w = (widgets::text_width(&p, "Close", &primary_font).ceil() + 2.0 * theme::INSET_CONTROL).max(MIN_CLOSE_W);
        let ok = Rect::from_min_size(Pos2::new(rect.right() - pad - primary_w, by), Vec2::new(primary_w, btn_h));
        let cl = Rect::from_min_size(Pos2::new(ok.left() - theme::SPACE_INLINE - close_w, by), Vec2::new(close_w, btn_h));
        widgets::record(ui, "btn-\"compare-done\"", cl);
        if crate::widgets::kit::button(ui, cl, "compare-done", "Close", crate::widgets::kit::Hierarchy::Secondary, crate::widgets::kit::Size::Md, true)
            .clicked()
        {
            close = true;
        }
        if crate::widgets::kit::button(ui, ok, "compare-remark", primary_label, crate::widgets::kit::Hierarchy::Primary, crate::widgets::kit::Size::Md, true)
            .clicked()
        {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_before_and_after_previews_of_a_display_p3_document_show_the_canvas_colours() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let mut n = Node::rectangle(id, designos_doc::Rect::new(0.0, 0.0, 20.0, 20.0));
        n.fills = vec![designos_doc::Paint::solid(designos_doc::Color { r: 0.5, g: 0.4, b: 0.8, a: 1.0 })];
        let id = d.insert(page, n).unwrap();
        d.color_profile = designos_doc::ColorProfile::DisplayP3;
        let png = render(&d, id, 1.0);
        let px = image::load_from_memory(&png).unwrap().to_rgba8().get_pixel(10, 10).0;
        assert!((px[0] as i32 - 132).abs() <= 1 && (px[1] as i32 - 101).abs() <= 1 && (px[2] as i32 - 211).abs() <= 1, "{px:?}");
    }

    /// Draws the dialog once on a `screen` and returns the recorded footer button, the title's centre y and the dialog rect.
    fn drawn(screen: Vec2) -> (Rect, f32, Rect) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let f = d.insert(page, Node::frame(id, designos_doc::Rect::new(0.0, 0.0, 200.0, 100.0))).unwrap();
        let mut app = App::new(d);
        app.dev.compare =
            Some(CompareView { node: f, name: "Card".into(), changes: Vec::new(), before_png: Vec::new(), after_png: Vec::new(), tex: None, scroll: 0.0 });
        let ctx = Context::default();
        theme::install_fonts(&ctx);
        let mut out = None;
        for i in 0..3 {
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, screen)), time: Some(f64::from(i) * 0.1), ..Default::default() };
            let full = ctx.run_ui(raw, |ui| show(&mut app, &ui.ctx().clone()));
            let title = full.shapes.iter().find_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) if t.galley.text().starts_with("Compare changes") => Some(t.pos.y + t.galley.rect.height() * 0.5),
                _ => None,
            });
            let footer = ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", "btn-\"compare-done\"".to_string()))));
            out = title.zip(footer);
        }
        let (title_y, footer) = out.expect("the dialog paints its title and footer");
        let h = (screen.y - 80.0).clamp(380.0, 640.0);
        (footer, title_y, Rect::from_center_size(Rect::from_min_size(Pos2::ZERO, screen).center(), Vec2::new(W.min(screen.x - 40.0), h)))
    }

    #[test]
    fn the_footer_buttons_sit_as_far_from_the_bottom_edge_as_from_the_side_edge() {
        let (footer, _, dialog) = drawn(Vec2::new(1000.0, 800.0));
        assert_eq!(dialog.bottom() - footer.bottom(), PAD, "bottom inset");
        assert_eq!(footer.height(), theme::CONTROL_MD);
    }

    #[test]
    fn the_close_button_is_centred_on_the_title_row() {
        let (_, title_y, dialog) = drawn(Vec2::new(1000.0, 800.0));
        let close = close_rect(dialog);
        assert!((close.center().y - title_y).abs() < 0.5, "close centre {} vs title centre {title_y}", close.center().y);
        assert_eq!(dialog.right() - close.right(), PAD, "side inset");
        assert_eq!(close.top() - dialog.top(), dialog.top() + HEADER_H - close.bottom(), "even above and below");
    }
}
