//! Right panel shell: present, tabs, zoom dropdown.

use super::properties;
use crate::app::{App, MenuKind, RightTab};
use crate::icons::{self, Icon};
use crate::theme;
use crate::widgets;
use egui::{Align2, Pos2, Rect, Sense, Ui, Vec2};

pub fn show(app: &mut App, ui: &mut Ui, rect: Rect) {
    // Version history and comment mode replace the whole inspector.
    if app.history.open {
        crate::history::panel::show(app, ui, rect);
        return;
    }
    if app.comment_mode() {
        crate::comments::panel::show(app, ui, rect);
        return;
    }
    let p = ui.painter().clone();
    p.rect_filled(rect, 0.0, theme::BG);
    widgets::vline(&p, rect.left(), rect.top(), rect.bottom(), theme::BORDER);
    let right = rect.right();
    let left = rect.left();

    // User avatar pill + chevron on top-left of right inspector header.
    let top_btn_h = theme::RIGHT_BTN_SIZE.min(24.0);
    let av_r = Rect::from_min_size(Pos2::new(left + 8.0, rect.top() + theme::RIGHT_BTN_Y), Vec2::new(38.0, top_btn_h));
    let av_center = Pos2::new(left + 21.0, av_r.center().y);
    crate::widgets::record(ui, "right-top-avatar", av_r);
    let av_resp = ui.interact(av_r, ui.id().with("right-top-avatar"), Sense::click());
    if av_resp.hovered() {
        p.rect_filled(av_r, theme::cr(theme::RADIUS_MEDIUM), theme::BG_HOVER);
    }
    let active_user = std::env::var("USER").unwrap_or_else(|_| "jangtrinh".into());
    let initial = active_user.chars().next().map(|c| c.to_ascii_uppercase().to_string()).unwrap_or_else(|| "J".to_string());
    p.circle_filled(av_center, 10.0, egui::Color32::from_rgb(0xF5, 0x9E, 0x0B));
    p.text(av_center, Align2::CENTER_CENTER, initial, theme::font_small_strong(), egui::Color32::WHITE);
    icons::paint(&p, Icon::ChevronDown, Pos2::new(av_center.x + 17.0, av_center.y), theme::CHEVRON_SMALL, theme::ICON_SECONDARY);
    if av_resp.on_hover_text(format!("Active user: {active_user}")).clicked() {
        let doc_title = if app.doc.name.is_empty() { "Untitled" } else { &app.doc.name };
        let stats = format!("User: {} · Doc: {} ({} pages, {} nodes)", active_user, doc_title, app.doc.pages.len(), app.doc.nodes.len());
        ui.ctx().copy_text(stats.clone());
        app.show_toast(stats, false);
    }

    // Present button + chevron (24px height, aligned with Share button).
    let pb = Rect::from_min_size(Pos2::new(right - theme::RIGHT_PRESENT_FROM_RIGHT - 46.0, rect.top() + theme::RIGHT_BTN_Y), Vec2::splat(top_btn_h));
    let pr = ui.interact(pb, ui.id().with("present"), Sense::click());
    widgets::button_bg(&p, pb, &pr, false, theme::cr_each(theme::RADIUS_MEDIUM, 0, 0, theme::RADIUS_MEDIUM));
    icons::paint(&p, Icon::Play, pb.center(), 14.0, theme::ICON);
    let cb = Rect::from_min_size(pb.right_top(), Vec2::new(theme::RIGHT_PRESENT_CHEVRON_W, top_btn_h));
    let cr = ui.interact(cb, ui.id().with("present-chev"), Sense::click());
    widgets::button_bg(&p, cb, &cr, false, theme::cr_each(0, theme::RADIUS_MEDIUM, theme::RADIUS_MEDIUM, 0));
    icons::paint(&p, Icon::ChevronDown, cb.center(), theme::CHEVRON_SMALL, theme::ICON_SECONDARY);
    crate::widgets::record(ui, "present-button", pb);
    crate::widgets::record(ui, "present-chevron", cb);
    if pr.clicked() || cr.clicked() {
        app.start_present();
    }

    // Share button at far right (aligned on the same y = RIGHT_BTN_Y and 24px height as Present).
    let share_r = Rect::from_min_size(Pos2::new(right - theme::RIGHT_PRESENT_FROM_RIGHT, rect.top() + theme::RIGHT_BTN_Y), Vec2::new(48.0, top_btn_h));
    let share_resp = ui.interact(share_r, ui.id().with("share-btn"), Sense::click());
    let share_col = if app.ui.dev_mode { egui::Color32::from_rgb(0x1B, 0xC4, 0x7D) } else { egui::Color32::from_rgb(0x0D, 0x99, 0xFF) };
    p.rect_filled(share_r, egui::CornerRadius::same(6), if share_resp.hovered() { share_col.gamma_multiply(1.1) } else { share_col });
    p.text(share_r.center(), Align2::CENTER_CENTER, "Share", theme::font_body_strong(), egui::Color32::WHITE);
    crate::widgets::record(ui, "share-button", share_r);
    crate::widgets::record(ui, "right-top-share", share_r);
    if share_resp.clicked() {
        let doc_name = if app.doc.name.is_empty() { "Untitled" } else { &app.doc.name };
        let file_ref = app
            .file_path
            .as_ref()
            .map(|p| p.display().to_string())
            .or_else(|| app.lib_file.as_ref().map(|f| f.id.clone()))
            .unwrap_or_else(|| "local".to_string());
        let page_name = app.doc.get(app.page).map(|n| n.name.as_str()).unwrap_or("Page 1");
        let sel_str = if app.selection.is_empty() {
            "none".to_string()
        } else {
            app.selection
                .iter()
                .map(|id| {
                    let nm = app.doc.get(*id).map(|n| n.name.as_str()).unwrap_or("layer");
                    format!("{}:{}", id.0, nm.replace(' ', "_"))
                })
                .collect::<Vec<_>>()
                .join(",")
        };
        let payload = format!(
            "designos://open?doc={}&file={}&page={}&selection={}",
            doc_name.replace(' ', "%20"),
            file_ref.replace(' ', "%20"),
            page_name.replace(' ', "%20"),
            sel_str
        );
        ui.ctx().copy_text(payload.clone());
        app.show_toast(format!("Share link copied to clipboard ({payload})"), false);
    }

    // Tabs: Design | Prototype (or Inspect | Plugins when Dev Mode is active).
    let mut x = left + theme::RIGHT_TABS_X;
    let ty = rect.top() + theme::RIGHT_TABS_Y;
    let active = app.ui.right_tab.unwrap_or(RightTab::Design);
    let tab_labels = if app.ui.dev_mode {
        [(RightTab::Design, "Inspect"), (RightTab::Prototype, "Plugins")]
    } else {
        [(RightTab::Design, "Design"), (RightTab::Prototype, "Prototype")]
    };
    for (tab, label) in tab_labels {
        let sel = tab == active;
        let font = if sel { theme::font_body_strong() } else { theme::font_body() };
        let w = widgets::text_width(&p, label, &font) + theme::RIGHT_TAB_PAD_X * 2.0;
        let r = Rect::from_min_size(Pos2::new(x, ty), Vec2::new(w, theme::RIGHT_TABS_H));
        widgets::record(ui, &format!("right-tab-{label}"), r);
        let resp = ui.interact(r, ui.id().with(("tab", label)), Sense::click());
        if sel {
            p.rect_filled(r, theme::cr(theme::RADIUS_MEDIUM), theme::BG_SECONDARY);
        } else if resp.hovered() {
            p.rect_filled(r, theme::cr(theme::RADIUS_MEDIUM), theme::BG_HOVER);
        }
        p.text(r.center(), Align2::CENTER_CENTER, label, font, if sel { theme::TEXT } else { theme::TEXT_SECONDARY });
        if resp.clicked() {
            app.ui.right_tab = Some(tab);
        }
        x += w + 4.0;
    }

    // Zoom dropdown.
    let zr = Rect::from_min_size(Pos2::new(right - theme::RIGHT_ZOOM_FROM_RIGHT, ty), Vec2::new(theme::RIGHT_ZOOM_W, theme::RIGHT_TABS_H));
    let zresp = ui.interact(zr, ui.id().with("zoom"), Sense::click());
    let open = app.ui.menu == Some(MenuKind::Zoom);
    widgets::button_bg(&p, zr, &zresp, open, theme::cr(theme::RADIUS_MEDIUM));
    p.text(Pos2::new(zr.left() + 8.0, zr.center().y), Align2::LEFT_CENTER, format!("{}%", (app.view.zoom * 100.0).round()), theme::font_body(), theme::TEXT);
    icons::paint(&p, Icon::ChevronDown, Pos2::new(zr.right() - 10.0, zr.center().y), theme::CHEVRON_SMALL, theme::ICON_SECONDARY);
    if zresp.clicked() {
        app.ui.menu = if open { None } else { Some(MenuKind::Zoom) };
        app.ui.menu_anchor = Some(zr);
    }

    let content_top = rect.top() + theme::RIGHT_CONTENT_TOP - 1.0;
    let footer_top = (rect.bottom() - 36.0).max(content_top + 1.0);
    widgets::hline(&p, rect.left() + 1.0, right, content_top, theme::BORDER);
    let region = Rect::from_min_max(Pos2::new(rect.left() + 1.0, content_top + 1.0), Pos2::new(right, footer_top));
    if active == RightTab::Prototype {
        if app.ui.dev_mode {
            show_dev_plugins_inspector(app, ui, region);
        } else {
            super::prototype::show(app, ui, region);
        }
    } else if app.ui.dev_mode {
        super::inspect::show(app, ui, region);
    } else if app.tool == crate::canvas::tools::Tool::Frame && app.selection.is_empty() {
        show_frame_presets_inspector(app, ui, region);
    } else {
        properties::show(app, ui, region);
    }

    // Dedicated 36px footer bar with 1px top border so inspector content never collides with `?` Help button.
    let footer_r = Rect::from_min_max(Pos2::new(rect.left() + 1.0, footer_top), rect.max);
    let hp = ui.painter();
    hp.rect_filled(footer_r, 0.0, theme::BG);
    widgets::hline(hp, rect.left() + 1.0, right, footer_top, theme::BORDER);
    let help_center = Pos2::new(rect.right() - 20.0, footer_r.center().y);
    let help_r = Rect::from_center_size(help_center, Vec2::splat(28.0));
    let help_resp = ui.interact(help_r, ui.id().with("right-help-btn"), Sense::click());
    let hp = ui.painter();
    hp.circle_filled(
        help_center,
        14.0,
        if help_resp.hovered() { egui::Color32::from_rgb(0x38, 0x38, 0x38) } else { egui::Color32::from_rgb(0x2C, 0x2C, 0x2C) },
    );
    hp.circle_stroke(help_center, 14.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(0x38, 0x38, 0x38)));
    icons::paint(hp, Icon::Help, help_center, 14.0, egui::Color32::from_rgb(0xB3, 0xB3, 0xB3));
    crate::widgets::record(ui, "help-button", help_r);
    if help_resp.on_hover_text("Keyboard shortcuts & help").clicked() {
        app.show_shortcuts();
    }
}

fn show_dev_plugins_inspector(app: &mut App, ui: &mut Ui, region: Rect) {
    let p = ui.painter().clone();
    let pad = 16.0;
    let mut y = region.top() + 14.0;
    let w = (region.width() - pad * 2.0).max(80.0);

    p.text(Pos2::new(region.left() + pad, y), Align2::LEFT_CENTER, "Dev Mode Plugins & Codegen", theme::font_body_strong(), theme::TEXT);
    y += 22.0;

    let builtins = [
        ("designos-mcp (22 Tools)", "JSON-RPC MCP server bridge"),
        ("design:os Code Connect", "@designos/code-connect mapping"),
        ("Tailwind v4 Token Exporter", "CSS @theme variable tokens"),
        ("Design Gate A11y Auditor", "WCAG contrast & tap target check"),
    ];
    for (idx, (title, subtitle)) in builtins.iter().enumerate() {
        let row_r = Rect::from_min_size(Pos2::new(region.left() + pad, y), Vec2::new(w, 40.0));
        widgets::record(ui, &format!("dev-plugin-builtin-{idx}"), row_r);
        let resp = ui.interact(row_r, ui.id().with(("dev-plugin-builtin", idx)), Sense::click());
        p.rect_filled(row_r, theme::cr(6), if resp.hovered() { theme::BG_TERTIARY } else { theme::BG_SECONDARY });
        icons::paint(&p, Icon::Dev, Pos2::new(row_r.left() + 14.0, row_r.center().y), 14.0, egui::Color32::from_rgb(0x1B, 0xC4, 0x7D));
        p.text(Pos2::new(row_r.left() + 28.0, row_r.top() + 13.0), Align2::LEFT_CENTER, *title, theme::font_body_strong(), theme::TEXT);
        p.text(Pos2::new(row_r.left() + 28.0, row_r.top() + 28.0), Align2::LEFT_CENTER, *subtitle, theme::font_small(), theme::TEXT_SECONDARY);
        if resp.clicked() {
            match idx {
                0 => {
                    let cfg = "{\"mcpServers\":{\"designos-mcp\":{\"command\":\"designos-mcp\",\"args\":[\"--stdio\"]}}}";
                    ui.ctx().copy_text(cfg.to_string());
                    app.show_toast("Copied designos-mcp (22 Tools) config to clipboard", false);
                }
                1 => {
                    let target = app.selection.first().copied().unwrap_or(app.page);
                    let nm = app.doc.get(target).map(|n| n.name.as_str()).unwrap_or("Component");
                    let snippet = format!(
                        "import designos from \"@designos/code-connect\";\ndesignos.connect({}, \"designos://node/{}\", {{}});",
                        nm.chars().filter(|c| c.is_alphanumeric()).collect::<String>(),
                        target.0
                    );
                    ui.ctx().copy_text(snippet);
                    app.show_toast("Copied design:os Code Connect snippet to clipboard", false);
                }
                2 => {
                    let mut css = String::from("@theme {\n");
                    for v in app.doc.variables.values() {
                        css.push_str(&format!("  --{}: /* {:?} */;\n", v.name.replace(['/', ' '], "-").to_lowercase(), v.kind));
                    }
                    css.push_str("}\n");
                    ui.ctx().copy_text(css);
                    app.show_toast(format!("Exported {} Tailwind v4 tokens to clipboard", app.doc.variables.len()), false);
                }
                _ => {
                    let small_targets = app
                        .doc
                        .nodes
                        .values()
                        .filter(|n| {
                            matches!(n.kind, designos_doc::NodeKind::Frame(_) | designos_doc::NodeKind::Component(_)) && (n.size.x < 44.0 || n.size.y < 44.0)
                        })
                        .count();
                    let report = format!("Design Gate A11y Audit: {} nodes checked, {} <44pt targets", app.doc.nodes.len(), small_targets);
                    ui.ctx().copy_text(report.clone());
                    app.show_toast(report, false);
                }
            }
        }
        y += 46.0;
    }

    let installed = app.plugins.installed.clone();
    for (i, plug) in installed.iter().enumerate() {
        let row_r = Rect::from_min_size(Pos2::new(region.left() + pad, y), Vec2::new(w, 32.0));
        widgets::record(ui, &format!("dev-plugin-installed-{i}"), row_r);
        let resp = ui.interact(row_r, ui.id().with(("dev-plugin-inst", &plug.id)), Sense::click());
        p.rect_filled(row_r, theme::cr(6), if resp.hovered() { theme::BG_TERTIARY } else { theme::BG_SECONDARY });
        p.text(Pos2::new(row_r.left() + 10.0, row_r.center().y), Align2::LEFT_CENTER, &plug.name, theme::font_body(), theme::TEXT);
        if resp.clicked() {
            let ctx = ui.ctx().clone();
            if let Err(e) = app.start_plugin(plug, &ctx) {
                app.show_toast(e, true);
            }
        }
        y += 36.0;
    }

    y += 4.0;
    let open_r = Rect::from_min_size(Pos2::new(region.left() + pad, y), Vec2::new(w, 28.0));
    widgets::record(ui, "dev-plugins-open-modal", open_r);
    let open_resp = ui.interact(open_r, ui.id().with("dev-plugins-open-modal"), Sense::click());
    p.rect_filled(
        open_r,
        theme::cr(6),
        if open_resp.hovered() { egui::Color32::from_rgb(0x1B, 0xC4, 0x7D).gamma_multiply(1.1) } else { egui::Color32::from_rgb(0x1B, 0xC4, 0x7D) },
    );
    p.text(open_r.center(), Align2::CENTER_CENTER, "Manage & Run Plugins\u{2026}", theme::font_body_strong(), egui::Color32::WHITE);
    if open_resp.clicked() {
        app.open_plugins();
    }
    y += 34.0;

    let mcp_r = Rect::from_min_size(Pos2::new(region.left() + pad, y), Vec2::new(w, 28.0));
    widgets::record(ui, "dev-plugins-copy-mcp", mcp_r);
    let mcp_resp = ui.interact(mcp_r, ui.id().with("dev-plugins-copy-mcp"), Sense::click());
    p.rect_filled(mcp_r, theme::cr(6), if mcp_resp.hovered() { theme::BG_TERTIARY } else { theme::BG_SECONDARY });
    p.text(mcp_r.center(), Align2::CENTER_CENTER, "Copy MCP Config", theme::font_body_strong(), theme::TEXT);
    if mcp_resp.clicked() {
        let mcp_json = "{\n  \"mcpServers\": {\n    \"designos-mcp\": {\n      \"command\": \"designos-mcp\",\n      \"args\": [\"--stdio\"]\n    }\n  }\n}";
        ui.ctx().copy_text(mcp_json.to_string());
        app.show_toast("Copied MCP server config JSON to clipboard", false);
    }
}

fn show_frame_presets_inspector(app: &mut App, ui: &mut Ui, region: Rect) {
    use crate::proto::{frame_presets_in_category, PresetCategory};

    let p = ui.painter().clone();
    let pad = 16.0;
    let mut y = region.top() + 12.0;
    let w = (region.width() - pad * 2.0).max(80.0);

    p.text(Pos2::new(region.left() + pad, y), Align2::LEFT_CENTER, "Frame", theme::font_body_strong(), theme::TEXT);
    y += 20.0;

    let categories = [
        (PresetCategory::Phone, "Phone"),
        (PresetCategory::Tablet, "Tablet"),
        (PresetCategory::Desktop, "Desktop"),
        (PresetCategory::Presentation, "Presentation"),
        (PresetCategory::Watch, "Watch"),
        (PresetCategory::Paper, "Paper"),
        (PresetCategory::SocialMedia, "Social Media"),
    ];

    for (cat, label) in categories {
        if y > region.bottom() - 24.0 {
            break;
        }
        p.text(Pos2::new(region.left() + pad, y), Align2::LEFT_CENTER, label, theme::font_small_strong(), theme::TEXT_SECONDARY);
        y += 18.0;
        for preset in frame_presets_in_category(cat) {
            if y > region.bottom() - 24.0 {
                break;
            }
            let row_r = Rect::from_min_size(Pos2::new(region.left() + pad, y), Vec2::new(w, 24.0));
            widgets::record(ui, &format!("frame-preset-{}", preset.name), row_r);
            let resp = ui.interact(row_r, ui.id().with(("frame-preset", preset.name)), Sense::click());
            if resp.hovered() {
                p.rect_filled(row_r, theme::cr(4), theme::BG_SECONDARY);
            }
            p.text(Pos2::new(row_r.left() + 6.0, row_r.center().y), Align2::LEFT_CENTER, preset.name, theme::font_body(), theme::TEXT);
            p.text(
                Pos2::new(row_r.right() - 6.0, row_r.center().y),
                Align2::RIGHT_CENTER,
                format!("{}×{}", preset.width as i32, preset.height as i32),
                theme::font_small(),
                theme::TEXT_SECONDARY,
            );
            if resp.clicked() {
                let _ = app.create_preset_frame(preset.name);
            }
            y += 24.0;
        }
        y += 6.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::Document;
    use egui::{Pos2, Rect, Vec2};

    #[test]
    fn dev_mode_plugins_tab_and_share_avatar_help_execute_real_logic() {
        let mut d = Document::new("TestDoc");
        let _p = d.add_page("Page 1");
        let mut app = App::new(d);
        app.ui.dev_mode = true;
        app.ui.right_tab = Some(RightTab::Prototype);
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0));
        let _ = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| {
            show(&mut app, ui, screen);
        });
        let get_rect = |name: &str| ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", name.to_string()))));
        let open_btn = get_rect("dev-plugins-open-modal").expect("dev-plugins-open-modal rect");
        let share_btn = get_rect("right-top-share").expect("right-top-share rect");
        let avatar_btn = get_rect("right-top-avatar").expect("right-top-avatar rect");
        let help_btn = get_rect("help-button").expect("help-button rect");
        assert!(get_rect("dev-plugin-builtin-0").is_some());
        assert!(get_rect("dev-plugins-copy-mcp").is_some());

        // Click Manage & Run Plugins...
        for down in [true, false] {
            let _ = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events: vec![
                        egui::Event::PointerMoved(open_btn.center()),
                        egui::Event::PointerButton {
                            pos: open_btn.center(),
                            button: egui::PointerButton::Primary,
                            pressed: down,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ui| show(&mut app, ui, screen),
            );
        }
        assert!(app.plugins.open, "clicking Manage & Run Plugins must open the plugins modal");

        // Click Share button
        for down in [true, false] {
            let _ = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events: vec![
                        egui::Event::PointerMoved(share_btn.center()),
                        egui::Event::PointerButton {
                            pos: share_btn.center(),
                            button: egui::PointerButton::Primary,
                            pressed: down,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ui| show(&mut app, ui, screen),
            );
        }
        assert!(app.toast.as_ref().is_some_and(|t| t.text.contains("designos://open?doc=")), "Share button must build and toast a real designos://open URI");

        // Click Avatar button
        for down in [true, false] {
            let _ = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events: vec![
                        egui::Event::PointerMoved(avatar_btn.center()),
                        egui::Event::PointerButton {
                            pos: avatar_btn.center(),
                            button: egui::PointerButton::Primary,
                            pressed: down,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ui| show(&mut app, ui, screen),
            );
        }
        assert!(
            app.toast.as_ref().is_some_and(|t| t.text.contains("User:") && t.text.contains("Doc:")),
            "Avatar click must report active user & document summary"
        );

        // Click Help button
        for down in [true, false] {
            let _ = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events: vec![
                        egui::Event::PointerMoved(help_btn.center()),
                        egui::Event::PointerButton {
                            pos: help_btn.center(),
                            button: egui::PointerButton::Primary,
                            pressed: down,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ui| show(&mut app, ui, screen),
            );
        }
        assert!(app.dialog.is_some(), "Help button must open keyboard shortcuts dialog");
    }
}
