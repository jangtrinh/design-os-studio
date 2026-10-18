//! Slide navigation and speaker notes for native presentation documents.

use crate::{app::App, comments::ui as buttons, theme, widgets};
use designos_doc::NodeId;
use egui::{Align2, Id, Pos2, Rect, Sense, Stroke, StrokeKind, TextEdit, Ui, Vec2};

pub fn mode_bar(app: &mut App, ui: &mut Ui, rect: Rect) {
    ui.painter().rect_filled(rect, 0.0, theme::BG);
    let half = rect.width() / 2.0;
    for (i, label) in ["Slides", "Layers"].iter().enumerate() {
        let button = Rect::from_min_size(rect.min + Vec2::new(i as f32 * half, 0.0), Vec2::new(half, rect.height()));
        let active = app.decks.design_mode == (i == 1);
        if buttons::button(ui, button.shrink(2.0), ("deck-mode", i), label, active, true).clicked() {
            app.decks.design_mode = i == 1;
        }
        widgets::record(ui, if i == 0 { "deck-slides-mode" } else { "deck-layers-mode" }, button);
    }
}

pub fn sidebar(app: &mut App, ui: &mut Ui, rect: Rect) {
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    child.set_clip_rect(rect);
    let ui = &mut child;
    ui.painter().rect_filled(rect, 0.0, theme::BG);
    let pad = theme::PANEL_PADDING_X;
    let slides = app.deck_slides();
    let active = app.active_slide();
    let editable = app.deck_editable();
    let title = Pos2::new(rect.left() + pad, rect.top() + theme::PANEL_TITLE_ROW_HEIGHT / 2.0);
    ui.painter().text(title, Align2::LEFT_CENTER, format!("Slides · {}", slides.len()), theme::font_body_strong(), theme::TEXT);
    let overview = Rect::from_min_size(Pos2::new(rect.right() - pad - 74.0, rect.top() + 6.0), Vec2::new(74.0, 28.0));
    if buttons::button(ui, overview, "deck-overview", "Overview", app.decks.grid, !slides.is_empty()).clicked() {
        app.decks.grid = !app.decks.grid;
        if app.decks.grid {
            app.fit_all();
        } else if let Some(id) = active {
            app.select_slide(id);
        }
    }
    widgets::record(ui, "deck-overview", overview);
    let controls_y = rect.bottom() - 140.0;
    let list_rect = Rect::from_min_max(Pos2::new(rect.left() + pad, rect.top() + 44.0), Pos2::new(rect.right() - pad, controls_y - 8.0));
    let mut list_ui = ui.new_child(egui::UiBuilder::new().max_rect(list_rect));
    list_ui.set_clip_rect(list_rect);
    let mut chosen = None;
    egui::ScrollArea::vertical().id_salt("deck-slides-list").auto_shrink([false, false]).show(&mut list_ui, |ui| {
        ui.set_width(list_rect.width());
        if slides.is_empty() {
            ui.label(egui::RichText::new("Add a slide to start your presentation.").font(theme::font_body()).color(theme::TEXT_SECONDARY));
        }
        for (index, id) in slides.iter().enumerate() {
            let width = ui.available_width().max(40.0);
            let height = width * 9.0 / 16.0 + 28.0;
            let (card, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
            widgets::record(ui, &format!("deck-slide-{}", index + 1), card);
            if response.clicked() {
                chosen = Some(*id);
            }
            if ui.is_rect_visible(card) {
                let selected = active == Some(*id);
                let p = ui.painter();
                p.rect_filled(
                    card,
                    theme::cr(theme::RADIUS_CONTROL),
                    if selected {
                        theme::BG_SELECTED
                    } else if response.hovered() {
                        theme::BG_HOVER
                    } else {
                        theme::BG_SECONDARY
                    },
                );
                let preview = Rect::from_min_max(card.min + Vec2::splat(4.0), Pos2::new(card.right() - 4.0, card.bottom() - 28.0));
                p.rect_filled(preview, theme::cr(theme::RADIUS_MARK), theme::BG_TERTIARY);
                if let Some(texture) = thumbnail(app, ui, *id, preview.width()) {
                    p.image(texture.id(), preview, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), egui::Color32::WHITE);
                }
                p.rect_stroke(
                    card,
                    theme::cr(theme::RADIUS_CONTROL),
                    Stroke::new(if selected { 2.0 } else { 1.0 }, if selected { theme::BORDER_SELECTED } else { theme::BORDER }),
                    StrokeKind::Inside,
                );
                let name = app.doc.get(*id).map_or("Slide", |node| node.name.as_str());
                let label = widgets::elide(p, &format!("{}   {}", index + 1, name), &theme::font_body(), width - 16.0);
                p.text(Pos2::new(card.left() + 8.0, card.bottom() - 14.0), Align2::LEFT_CENTER, label, theme::font_body(), theme::TEXT);
            }
            ui.add_space(8.0);
        }
    });
    if let Some(id) = chosen {
        flush_notes(app);
        app.decks.grid = false;
        app.select_slide(id);
    }
    let available = (rect.width() - pad * 2.0 - 8.0) / 2.0;
    let position = active.and_then(|id| slides.iter().position(|slide| *slide == id));
    for (index, (key, label, enabled)) in [
        ("deck-add", "+ Add slide", true),
        ("deck-duplicate", "Duplicate", active.is_some()),
        ("deck-up", "Move up", position.is_some_and(|i| i > 0)),
        ("deck-down", "Move down", position.is_some_and(|i| i + 1 < slides.len())),
    ]
    .into_iter()
    .enumerate()
    {
        let control = Rect::from_min_size(
            Pos2::new(rect.left() + pad + (index % 2) as f32 * (available + 8.0), controls_y + (index / 2) as f32 * 32.0),
            Vec2::new(available, 28.0),
        );
        if buttons::button(ui, control, key, label, index == 0, enabled && editable).clicked() {
            flush_notes(app);
            match index {
                0 => {
                    app.add_slide();
                }
                1 => {
                    app.duplicate_slide();
                }
                2 => app.move_slide(-1),
                _ => app.move_slide(1),
            }
        }
        widgets::record(ui, key, control);
    }
    let delete = Rect::from_min_size(Pos2::new(rect.left() + pad, controls_y + 64.0), Vec2::new(rect.width() - pad * 2.0, 28.0));
    if buttons::button(ui, delete, "deck-delete", "Delete slide", false, active.is_some() && editable).clicked() {
        flush_notes(app);
        app.delete_slide();
    }
    widgets::record(ui, "deck-delete", delete);
    for (index, (key, label, all)) in [("deck-export-slide-pdf", "Slide PDF", false), ("deck-export-all-pdf", "Deck PDF", true)].into_iter().enumerate() {
        let control = Rect::from_min_size(Pos2::new(rect.left() + pad + index as f32 * (available + 8.0), controls_y + 96.0), Vec2::new(available, 28.0));
        let enabled = app.deck_pdf_slides(all).is_ok();
        if buttons::button(ui, control, key, label, false, enabled)
            .on_hover_text(if all { "Export every slide across all pages as PDF" } else { "Export the current slide as PDF" })
            .clicked()
        {
            app.export_deck_pdf(all);
        }
        widgets::record(ui, key, control);
    }
}

/// Save a pending draft before changing documents or slide selection.
pub fn flush_notes(app: &mut App) {
    if app.decks.notes_draft == app.decks.notes_saved {
        return;
    }
    if let Some(id) = app.decks.notes_for {
        let draft = app.decks.notes_draft.clone();
        let changed = app.doc.get(id).and_then(|node| node.slide.as_ref()).is_some_and(|slide| slide.notes != draft);
        if changed {
            app.set_slide_notes(id, draft);
        }
        if app.doc.get(id).and_then(|node| node.slide.as_ref()).is_some_and(|slide| slide.notes == app.decks.notes_draft) {
            app.decks.notes_saved = app.decks.notes_draft.clone();
        }
    }
}

pub fn notes(app: &mut App, ui: &mut Ui, rect: Rect) {
    let active = app.active_slide();
    let editable = app.deck_editable();
    if active != app.decks.notes_for {
        flush_notes(app);
        app.decks.notes_for = active;
        app.decks.notes_draft =
            active.and_then(|id| app.doc.get(id)).and_then(|node| node.slide.as_ref()).map_or_else(String::new, |slide| slide.notes.clone());
        app.decks.notes_saved = app.decks.notes_draft.clone();
    } else if app.decks.notes_draft == app.decks.notes_saved {
        // Reflect Undo/Redo without discarding an unsaved typing session.
        let saved = active.and_then(|id| app.doc.get(id)).and_then(|node| node.slide.as_ref()).map_or_else(String::new, |slide| slide.notes.clone());
        app.decks.notes_draft.clone_from(&saved);
        app.decks.notes_saved = saved;
    }
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    child.set_clip_rect(rect);
    let ui = &mut child;
    let p = ui.painter();
    p.rect_filled(rect, 0.0, theme::BG);
    p.line_segment([rect.left_top(), rect.right_top()], Stroke::new(1.0, theme::BORDER));
    let pad = theme::PANEL_PADDING_X;
    p.text(Pos2::new(rect.left() + pad, rect.top() + 20.0), Align2::LEFT_CENTER, "Speaker notes", theme::font_body_strong(), theme::TEXT);
    let save = Rect::from_min_size(Pos2::new(rect.right() - pad - 78.0, rect.top() + 6.0), Vec2::new(78.0, 28.0));
    if buttons::button(ui, save, "deck-save", "Save notes", true, active.is_some() && editable).clicked() {
        flush_notes(app);
    }
    widgets::record(ui, "deck-save", save);
    let field = Rect::from_min_max(Pos2::new(rect.left() + pad, rect.top() + 40.0), Pos2::new(rect.right() - pad, rect.bottom() - 12.0));
    ui.add_enabled_ui(active.is_some() && editable, |ui| {
        ui.put(
            field,
            TextEdit::multiline(&mut app.decks.notes_draft)
                .id(Id::new("deck-notes"))
                .font(theme::font_body())
                .text_color(theme::TEXT)
                .hint_text("Add talking points. Save notes or select another slide to keep them.")
                .desired_width(field.width())
                .min_size(field.size()),
        );
    });
    widgets::record(ui, "deck-notes", field);
}

#[derive(Clone)]
struct Thumbnail {
    texture: egui::TextureHandle,
    checked: f64,
}

fn thumbnail(app: &App, ui: &Ui, id: NodeId, width: f32) -> Option<egui::TextureHandle> {
    let key = Id::new(("deck-thumbnail", id));
    let now = ui.input(|input| input.time);
    let cached = ui.ctx().data_mut(|data| data.get_temp::<Thumbnail>(key));
    if let Some(cached) = &cached {
        if now - cached.checked < 0.5 {
            return Some(cached.texture.clone());
        }
    }
    let node = app.doc.get(id)?;
    let scale = (width * ui.ctx().pixels_per_point() / node.size.x.max(1.0)).clamp(0.01, 0.5);
    let pixmap = designos_render::render_node(&app.doc, id, scale)?;
    let image = egui::ColorImage::from_rgba_premultiplied([pixmap.width() as usize, pixmap.height() as usize], pixmap.data());
    let texture = ui.ctx().load_texture(format!("slide-{}", id.0), image, egui::TextureOptions::LINEAR);
    ui.ctx().data_mut(|data| data.insert_temp(key, Thumbnail { texture: texture.clone(), checked: now }));
    Some(texture)
}
