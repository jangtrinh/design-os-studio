//! Native plugin library and URL installation dialog.

use crate::{
    app::App,
    comments::ui as buttons,
    icons::{self, Icon},
    theme, widgets,
};
use egui::{Align2, Color32, Context, Id, Key, Modifiers, Order, Pos2, Rect, Sense, TextEdit, Vec2};

const PAD: f32 = theme::INSET_MODAL;

pub fn show(app: &mut App, ctx: &Context) {
    if !app.plugins.open {
        return;
    }
    let screen = ctx.content_rect();
    let rect = Rect::from_center_size(
        screen.center(),
        Vec2::new(560.0_f32.min((screen.width() - 24.0).max(200.0)), 440.0_f32.min((screen.height() - 24.0).max(200.0))),
    );
    let url_id = Id::new("plugin-url");
    let focused = ctx.memory(|m| m.has_focus(url_id));
    let (escape, enter) = ctx.input_mut(|i| (i.consume_key(Modifiers::NONE, Key::Escape), focused && i.consume_key(Modifiers::NONE, Key::Enter)));
    let mut close = false;
    let mut launch = None;
    let mut stop = false;
    let mut cancel = false;
    let mut submit = enter && app.plugins.install_form && !app.plugins.pending && !app.plugins.url.trim().is_empty();
    if escape {
        if app.plugins.install_form && !app.plugins.pending {
            cancel = true;
        } else {
            close = true;
        }
    }
    egui::Area::new(Id::new("plugins-modal")).order(Order::Foreground).fixed_pos(screen.min).interactable(true).show(ctx, |ui| {
        ui.allocate_rect(screen, Sense::click_and_drag());
        let p = ui.painter().clone();
        p.rect_filled(screen, 0.0, Color32::from_black_alpha(140));
        widgets::menu_panel(&p, rect, theme::RADIUS_FRAME);
        let left = rect.left() + PAD;
        let width = rect.width() - PAD * 2.0;
        let title = if app.plugins.install_form { "Install plugin" } else { "Plugins" };
        p.text(Pos2::new(left, rect.top() + 36.0), Align2::LEFT_CENTER, title, theme::font_title(), theme::TEXT);
        let close_rect = Rect::from_min_size(Pos2::new(rect.right() - PAD - 52.0, rect.top() + 20.0), Vec2::new(52.0, 32.0));
        if buttons::button(ui, close_rect, "plugin-close", "Close", false, true).clicked() {
            close = true;
        }
        widgets::record(ui, "plugin-close", close_rect);
        let body = Rect::from_min_max(Pos2::new(left, rect.top() + 72.0), Pos2::new(rect.right() - PAD, rect.bottom() - 76.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
        child.set_clip_rect(body);
        egui::ScrollArea::vertical().id_salt("plugins-content").auto_shrink([false, false]).show(&mut child, |ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 12.0);
            if app.plugins.install_form {
                paragraph(ui, "Paste a manifest URL from jangtrinh/design-os-plugins.", theme::TEXT_SECONDARY);
                ui.add_space(4.0);
                paragraph(ui, "Plugin URL", theme::TEXT);
                let (field, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 38.0), Sense::hover());
                ui.painter().rect_filled(field, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
                let output = ui
                    .add_enabled_ui(!app.plugins.pending, |ui| {
                        ui.put(
                            field,
                            TextEdit::singleline(&mut app.plugins.url)
                                .id(url_id)
                                .font(theme::font_body())
                                .text_color(theme::TEXT)
                                .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(10, 0)))
                                .vertical_align(egui::Align::Center)
                                .hint_text("https://github.com/jangtrinh/design-os-plugins/blob/main/plugins/icon-browser/manifest.json")
                                .desired_width(field.width())
                                .min_size(field.size()),
                        )
                    })
                    .inner;
                widgets::record(ui, "plugin-url", field);
                if app.plugins.fresh {
                    output.request_focus();
                    app.plugins.fresh = false;
                }
                paragraph(
                    ui,
                    "For private plugin repositories, set DESIGNOS_PLUGINS_TOKEN (or DESIGNOS_PLUGINS_TOKEN) to a GitHub token with read access.",
                    theme::TEXT_TERTIARY,
                );
                paragraph(
                    ui,
                    "Install downloads the bundle. Open launches its browser panel with the permissions listed in your library.",
                    theme::TEXT_SECONDARY,
                );
                if app.plugins.pending {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        paragraph(ui, "Downloading and checking plugin files…", theme::TEXT_BRAND);
                    });
                }
            } else if app.plugins.installed.is_empty() {
                let (icon_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 60.0), Sense::hover());
                icons::paint(ui.painter(), Icon::Tools, icon_rect.center(), 28.0, theme::ICON_SECONDARY);
                paragraph(ui, "Your plugin library starts here", theme::TEXT);
                paragraph(ui, "Install a plugin from jangtrinh/design-os-plugins to keep its package on this device.", theme::TEXT_SECONDARY);
                paragraph(ui, "Browser plugins can insert SVG icons into the open document. Each insertion supports Undo.", theme::TEXT_TERTIARY);
            } else {
                paragraph(ui, &format!("{} installed on this device", app.plugins.installed.len()), theme::TEXT_SECONDARY);
                for (index, plugin) in app.plugins.installed.iter().enumerate() {
                    let (card, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 132.0), Sense::hover());
                    let p = ui.painter();
                    p.rect_filled(card, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
                    icons::paint(p, Icon::Tools, Pos2::new(card.left() + 26.0, card.top() + 27.0), 20.0, theme::ICON_SECONDARY);
                    let name = widgets::elide(p, &format!("{} {}", plugin.name, plugin.version), &theme::font_body_strong(), card.width() - 150.0);
                    p.text(Pos2::new(card.left() + 48.0, card.top() + 26.0), Align2::LEFT_CENTER, name, theme::font_body_strong(), theme::TEXT);
                    p.text(
                        Pos2::new(card.left() + 14.0, card.top() + 53.0),
                        Align2::LEFT_CENTER,
                        if plugin.runtime_supported { "Browser panel · SVG insertion with Undo" } else { "Installed · runtime not supported" },
                        theme::font_body(),
                        theme::TEXT_SECONDARY,
                    );
                    let source = widgets::elide(p, &plugin.source_url, &theme::font_caption(), card.width() - 28.0);
                    p.text(Pos2::new(card.left() + 14.0, card.top() + 75.0), Align2::LEFT_CENTER, source, theme::font_caption(), theme::TEXT_TERTIARY);
                    ui.interact(card, ui.id().with(("plugin-card", index)), Sense::hover()).on_hover_text(&plugin.source_url);
                    widgets::record(ui, &format!("plugin-card-{index}"), card);
                    let permissions = if plugin.permissions.is_empty() { "No document permissions".to_string() } else { plugin.permissions.join(", ") };
                    p.text(
                        Pos2::new(card.left() + 14.0, card.top() + 98.0),
                        Align2::LEFT_CENTER,
                        widgets::elide(p, &permissions, &theme::font_caption(), card.width() - 28.0),
                        theme::font_caption(),
                        theme::TEXT_SECONDARY,
                    );
                    p.text(
                        Pos2::new(card.left() + 14.0, card.top() + 117.0),
                        Align2::LEFT_CENTER,
                        widgets::elide(p, &plugin.network.join(", "), &theme::font_caption(), card.width() - 28.0),
                        theme::font_caption(),
                        theme::TEXT_TERTIARY,
                    );
                    let running = app.plugins.running_id.as_deref() == Some(plugin.id.as_str());
                    let control = Rect::from_min_size(Pos2::new(card.right() - 78.0, card.top() + 10.0), Vec2::new(64.0, 32.0));
                    let key = format!("plugin-open-{index}");
                    if buttons::button(ui, control, &key, if running { "Stop" } else { "Open" }, !running, plugin.runtime_supported).clicked() {
                        if running {
                            stop = true;
                        } else {
                            launch = Some(plugin.clone());
                        }
                    }
                    widgets::record(ui, &key, control);
                }
            }
            if let Some(error) = &app.plugins.error {
                paragraph(ui, error, theme::DANGER);
            }
        });
        let footer_y = rect.bottom() - PAD - 34.0;
        let primary_label = if app.plugins.pending {
            "Installing…"
        } else if app.plugins.install_form {
            "Install"
        } else {
            "Install plugin"
        };
        let primary_w = buttons::button_width(&p, primary_label).max(96.0);
        let primary = Rect::from_min_size(Pos2::new(rect.right() - PAD - primary_w, footer_y), Vec2::new(primary_w, 34.0));
        let key = if app.plugins.install_form { "plugin-install-submit" } else { "plugin-install-new" };
        let enabled = !app.plugins.pending && (!app.plugins.install_form || !app.plugins.url.trim().is_empty());
        if buttons::button(ui, primary, key, primary_label, true, enabled).clicked() {
            if app.plugins.install_form {
                submit = true;
            } else {
                app.plugins.install_form = true;
                app.plugins.fresh = true;
                app.plugins.error = None;
            }
        }
        widgets::record(ui, key, primary);
        if app.plugins.install_form {
            let secondary = Rect::from_min_size(Pos2::new(primary.left() - 80.0, footer_y), Vec2::new(72.0, 34.0));
            if buttons::button(ui, secondary, "plugin-install-cancel", "Cancel", false, !app.plugins.pending).clicked() {
                cancel = true;
            }
            widgets::record(ui, "plugin-install-cancel", secondary);
        }
    });
    if stop {
        app.stop_plugin();
    } else if let Some(plugin) = launch {
        app.open_plugin(&plugin, ctx);
    } else if close {
        app.plugins.open = false;
    } else if cancel {
        app.plugins.install_form = false;
        app.plugins.error = None;
    } else if submit {
        app.install_plugin(ctx);
    }
}

fn paragraph(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.add(egui::Label::new(egui::RichText::new(text).font(theme::font_body()).color(color)).wrap());
}
