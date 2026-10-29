//! Right panel shell: present, tabs, zoom dropdown.

use super::{clipped, properties, scrollbar, wheel_scroll};
use crate::app::{App, MenuKind, RightTab};
use crate::icons::{self, Icon};
use crate::proto::{frame_presets_in_category, FramePreset, PresetCategory};
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
    let top_btn_h = theme::RIGHT_BTN_SIZE;
    let av_r = Rect::from_min_size(Pos2::new(left + theme::INSET_CONTROL, rect.top() + theme::RIGHT_BTN_Y), Vec2::new(38.0, top_btn_h));
    let av_center = Pos2::new(left + 21.0, av_r.center().y);
    crate::widgets::record(ui, "right-top-avatar", av_r);
    let av_resp = ui.interact(av_r, ui.id().with("right-top-avatar"), Sense::click());
    if av_resp.hovered() {
        p.rect_filled(av_r, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
    }
    let active_user = std::env::var("USER").unwrap_or_else(|_| "jangtrinh".into());
    let initial = crate::comments::model::initial(&active_user);
    p.circle_filled(av_center, 10.0, crate::comments::ui::avatar_color(&active_user));
    p.text(av_center, Align2::CENTER_CENTER, initial, theme::font_caption_strong(), theme::TEXT_ON_BRAND);
    icons::paint(&p, Icon::ChevronDown, Pos2::new(av_center.x + 17.0, av_center.y), theme::CHEVRON_SMALL, theme::ICON_SECONDARY);
    if av_resp.on_hover_text(format!("Active user: {active_user}")).clicked() {
        let doc_title = if app.doc.name.is_empty() { "Untitled" } else { &app.doc.name };
        let stats = format!("User: {} · Doc: {} ({} pages, {} nodes)", active_user, doc_title, app.doc.pages.len(), app.doc.nodes.len());
        ui.ctx().copy_text(stats.clone());
        app.show_toast(stats, false);
    }

    // Present, its chevron, and Share. Share is 60 wide (the old 56 px inset plus one step)
    // and keeps an 8 px gap; the chevron's click target is the 24 px slot beside Present.
    let share_w = theme::RIGHT_PRESENT_FROM_RIGHT + theme::SPACE_FACT;
    let share_r = Rect::from_min_size(Pos2::new(right - theme::INSET_CONTROL - share_w, rect.top() + theme::RIGHT_BTN_Y), Vec2::new(share_w, top_btn_h));
    let chevron_right = share_r.left() - theme::INSET_CONTROL;
    let cb = Rect::from_min_max(
        Pos2::new(chevron_right - theme::RIGHT_PRESENT_CHEVRON_W, rect.top() + theme::RIGHT_BTN_Y),
        Pos2::new(chevron_right, rect.top() + theme::RIGHT_BTN_Y + top_btn_h),
    );
    let pb = Rect::from_min_size(Pos2::new(cb.left() - top_btn_h, cb.top()), Vec2::splat(top_btn_h));
    let hit = Rect::from_min_size(pb.right_top(), Vec2::new(theme::TARGET_MIN, top_btn_h));
    let pr = ui.interact(pb, ui.id().with("present"), Sense::click());
    crate::widgets::kit::paint_icon_bg(&p, pb, &pr, false, theme::cr_each(theme::RADIUS_CONTROL, 0, 0, theme::RADIUS_CONTROL));
    let present_glyph = if pr.hovered() { theme::ICON_QUATERNARY_HOVER } else { theme::ICON_QUATERNARY };
    icons::paint(&p, Icon::Play, pb.center(), theme::LAYER_ICON_SIZE, present_glyph);
    let cr = ui.interact(hit, ui.id().with("present-chev"), Sense::click());
    crate::widgets::kit::paint_icon_bg(&p, cb, &cr, false, theme::cr_each(0, theme::RADIUS_CONTROL, theme::RADIUS_CONTROL, 0));
    let chevron_glyph = if cr.hovered() { theme::ICON_QUATERNARY_HOVER } else { theme::ICON_QUATERNARY };
    icons::paint(&p, Icon::ChevronDown, cb.center(), theme::CHEVRON_SMALL, chevron_glyph);
    crate::widgets::record(ui, "present-button", pb);
    crate::widgets::record(ui, "present-chevron", cb);
    crate::widgets::record(ui, "present-chevron-hit", hit);
    if pr.clicked() || cr.clicked() {
        app.start_present();
    }

    let share_resp =
        crate::widgets::kit::button(ui, share_r, "share-btn", "Share", crate::widgets::kit::Hierarchy::Primary, crate::widgets::kit::Size::Xs, true);
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
        let resp = crate::widgets::kit::tab(ui, r, ("tab", label), label, sel);
        if resp.clicked() {
            app.ui.right_tab = Some(tab);
        }
        x += w + theme::SPACE_FACT;
    }

    // Zoom dropdown.
    let zr = Rect::from_min_size(Pos2::new(right - theme::RIGHT_ZOOM_FROM_RIGHT, ty), Vec2::new(theme::RIGHT_ZOOM_W, theme::RIGHT_TABS_H));
    let zresp = ui.interact(zr, ui.id().with("zoom"), Sense::click());
    let open = app.ui.menu == Some(MenuKind::Zoom);
    if open {
        let paint = crate::widgets::kit::select_look(crate::widgets::kit::State::Focused);
        crate::widgets::kit::paint_frame(&p, zr, theme::RADIUS_CONTROL, &paint);
    } else {
        widgets::button_bg(&p, zr, &zresp, false, theme::cr(theme::RADIUS_CONTROL));
    }
    p.text(Pos2::new(zr.left() + 8.0, zr.center().y), Align2::LEFT_CENTER, format!("{}%", (app.view.zoom * 100.0).round()), theme::font_body(), theme::TEXT);
    icons::paint(&p, Icon::ChevronDown, Pos2::new(zr.right() - 10.0, zr.center().y), theme::CHEVRON_SMALL, theme::ICON_QUATERNARY);
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
    let help_r = Rect::from_center_size(help_center, Vec2::splat(theme::RIGHT_BTN_SIZE));
    let help_resp = crate::widgets::kit::button_icon(
        ui,
        help_r,
        "right-help-btn",
        Icon::Help,
        crate::widgets::kit::Hierarchy::Icon,
        crate::widgets::kit::Size::Xs,
        true,
        false,
    );
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
        p.rect_filled(row_r, theme::cr(theme::RADIUS_SEGMENT), if resp.hovered() { theme::BG_TERTIARY } else { theme::BG_SECONDARY });
        icons::paint(&p, Icon::Dev, Pos2::new(row_r.left() + 14.0, row_r.center().y), 14.0, theme::DEV_READY);
        p.text(Pos2::new(row_r.left() + 28.0, row_r.top() + 13.0), Align2::LEFT_CENTER, *title, theme::font_body_strong(), theme::TEXT);
        p.text(Pos2::new(row_r.left() + 28.0, row_r.top() + 28.0), Align2::LEFT_CENTER, *subtitle, theme::font_caption(), theme::TEXT_SECONDARY);
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
        p.rect_filled(row_r, theme::cr(theme::RADIUS_SEGMENT), if resp.hovered() { theme::BG_TERTIARY } else { theme::BG_SECONDARY });
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
    p.rect_filled(open_r, theme::cr(theme::RADIUS_CONTROL), if open_resp.hovered() { theme::DEV_READY_HOVER } else { theme::DEV_READY });
    p.text(
        open_r.center(),
        Align2::CENTER_CENTER,
        "Manage & Run Plugins\u{2026}",
        theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold),
        theme::TEXT_ON_BRAND,
    );
    if open_resp.clicked() {
        app.open_plugins();
    }
    y += 34.0;

    let mcp_r = Rect::from_min_size(Pos2::new(region.left() + pad, y), Vec2::new(w, 28.0));
    widgets::record(ui, "dev-plugins-copy-mcp", mcp_r);
    let mcp_resp = ui.interact(mcp_r, ui.id().with("dev-plugins-copy-mcp"), Sense::click());
    let mcp_state = if mcp_resp.is_pointer_button_down_on() {
        crate::widgets::kit::State::Pressed
    } else if mcp_resp.hovered() {
        crate::widgets::kit::State::Hover
    } else {
        crate::widgets::kit::State::Default
    };
    let mcp_paint = crate::widgets::kit::button_look(crate::widgets::kit::Hierarchy::Secondary, mcp_state);
    crate::widgets::kit::paint_frame(&p, mcp_r, theme::RADIUS_CONTROL, &mcp_paint);
    p.text(mcp_r.center(), Align2::CENTER_CENTER, "Copy MCP Config", theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold), mcp_paint.text);
    if mcp_resp.clicked() {
        let mcp_json = "{\n  \"mcpServers\": {\n    \"designos-mcp\": {\n      \"command\": \"designos-mcp\",\n      \"args\": [\"--stdio\"]\n    }\n  }\n}";
        ui.ctx().copy_text(mcp_json.to_string());
        app.show_toast("Copied MCP server config JSON to clipboard", false);
    }
}

/// Frame preset categories in the order the inspector lists them.
const FRAME_PRESET_CATEGORIES: [(PresetCategory, &str); 7] = [
    (PresetCategory::Phone, "Phone"),
    (PresetCategory::Tablet, "Tablet"),
    (PresetCategory::Desktop, "Desktop"),
    (PresetCategory::Presentation, "Presentation"),
    (PresetCategory::Watch, "Watch"),
    (PresetCategory::Paper, "Paper"),
    (PresetCategory::SocialMedia, "Social Media"),
];
/// Inspector list metrics, all on the 4 px rhythm; rows are 24 px pointer targets.
const PRESET_PAD: f32 = 16.0;
const PRESET_TOP_PAD: f32 = 8.0;
const PRESET_BOTTOM_PAD: f32 = 16.0;
const PRESET_HEADING_H: f32 = 24.0;
const PRESET_ROW_H: f32 = 24.0;
const PRESET_CATEGORY_GAP: f32 = 8.0;
/// Text inset inside a row and the least space between a preset name and its size label.
const PRESET_ROW_INSET: f32 = 8.0;
const PRESET_LABEL_GAP: f32 = 8.0;

/// Design tab with the Frame tool and nothing selected: the preset catalogue, scrolled like the
/// Prototype tab. Clicking a row creates that frame.
fn show_frame_presets_inspector(app: &mut App, ui: &mut Ui, region: Rect) {
    let mut scroll = app.proto.presets_scroll;
    wheel_scroll(ui, region, &mut scroll, app.proto.presets_content_h);
    app.proto.presets_scroll = scroll;

    let mut chosen = None;
    let content_h = clipped(ui, region, |ui| {
        let p = ui.painter().clone();
        let left = region.left() + PRESET_PAD;
        let row_w = (region.width() - PRESET_PAD * 2.0).max(80.0);
        let top = region.top() - scroll;
        let mut off = PRESET_TOP_PAD;
        p.text(Pos2::new(left, top + off + PRESET_HEADING_H / 2.0), Align2::LEFT_CENTER, "Frame", theme::font_body_strong(), theme::TEXT);
        off += PRESET_HEADING_H;
        for (i, (category, label)) in FRAME_PRESET_CATEGORIES.into_iter().enumerate() {
            if i > 0 {
                off += PRESET_CATEGORY_GAP;
            }
            p.text(Pos2::new(left, top + off + PRESET_HEADING_H / 2.0), Align2::LEFT_CENTER, label, theme::font_caption_strong(), theme::TEXT_SECONDARY);
            off += PRESET_HEADING_H;
            for preset in frame_presets_in_category(category) {
                let row = Rect::from_min_size(Pos2::new(left, top + off), Vec2::new(row_w, PRESET_ROW_H));
                widgets::record(ui, &format!("frame-preset-{}", preset.name), row);
                // Rows outside the visible region are laid out but neither painted nor made clickable.
                if row.bottom() > region.top() && row.top() < region.bottom() && frame_preset_row(ui, &p, row, preset) {
                    chosen = Some(preset.name);
                }
                off += PRESET_ROW_H;
            }
        }
        off + PRESET_BOTTOM_PAD
    });
    app.proto.presets_content_h = content_h;
    scrollbar(ui, region, scroll, content_h);
    if let Some(s) = super::take_scrollbar_drag(ui, region) {
        app.proto.presets_scroll = s;
    }
    if let Some(name) = chosen {
        let _ = app.create_preset_frame(name);
    }
}

/// One catalogue row: the name (elided to leave room for the size label) and the size. Returns whether it was clicked.
fn frame_preset_row(ui: &mut Ui, p: &egui::Painter, row: Rect, preset: &FramePreset) -> bool {
    let resp = ui.interact(row, ui.id().with(("frame-preset", preset.name)), Sense::click());
    if resp.hovered() {
        p.rect_filled(row, theme::cr(theme::SPACE_FACT as u8), theme::BG_SECONDARY);
    }
    let size = format!("{}×{}", preset.width as i32, preset.height as i32);
    let (name_font, size_font) = (theme::font_body(), theme::font_caption());
    let name_room = row.width() - PRESET_ROW_INSET * 2.0 - widgets::text_width(p, &size, &size_font) - PRESET_LABEL_GAP;
    let name = widgets::elide(p, preset.name, &name_font, name_room);
    let elided = name != preset.name;
    p.text(Pos2::new(row.left() + PRESET_ROW_INSET, row.center().y), Align2::LEFT_CENTER, name, name_font, theme::TEXT);
    p.text(Pos2::new(row.right() - PRESET_ROW_INSET, row.center().y), Align2::RIGHT_CENTER, size, size_font, theme::TEXT_SECONDARY);
    let resp = if elided { resp.on_hover_text(preset.name) } else { resp };
    resp.clicked()
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

    /// The Frame preset inspector inside a window of `window` size: the right-hand rect spans the full
    /// height at the panel width, exactly as the shell lays it out.
    struct PresetInspector {
        ctx: egui::Context,
        app: App,
        screen: Rect,
        rect: Rect,
        region: Rect,
        t: f64,
        shapes: Vec<egui::epaint::ClippedShape>,
    }

    impl PresetInspector {
        fn new(window: Vec2) -> Self {
            let mut d = Document::new("t");
            d.add_page("p");
            let mut app = App::new(d);
            app.tool = crate::canvas::tools::Tool::Frame;
            let ctx = egui::Context::default();
            theme::install_fonts(&ctx);
            let screen = Rect::from_min_size(Pos2::ZERO, window);
            let rect = Rect::from_min_max(Pos2::new(screen.right() - (theme::RIGHT_PANEL_WIDTH + theme::BORDER_WIDTH), screen.top()), screen.max);
            let content_top = rect.top() + theme::RIGHT_CONTENT_TOP - 1.0;
            let footer_top = (rect.bottom() - 36.0).max(content_top + 1.0);
            let region = Rect::from_min_max(Pos2::new(rect.left() + 1.0, content_top + 1.0), Pos2::new(rect.right(), footer_top));
            let mut s = Self { ctx, app, screen, rect, region, t: 0.0, shapes: Vec::new() };
            s.frame(vec![]);
            s.frame(vec![]);
            s
        }

        fn frame(&mut self, events: Vec<egui::Event>) {
            self.t += 0.05;
            let raw = egui::RawInput { screen_rect: Some(self.screen), time: Some(self.t), events, ..Default::default() };
            let (app, rect) = (&mut self.app, self.rect);
            self.shapes = self.ctx.run_ui(raw, |ui| show(app, ui, rect)).shapes;
        }

        fn rect_named(&self, name: &str) -> Option<Rect> {
            self.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", name.to_string()))))
        }

        fn row(&self, preset: &str) -> Option<Rect> {
            self.rect_named(&format!("frame-preset-{preset}"))
        }

        /// One wheel notch over the inspector, then a few frames so the smoothed scroll settles.
        fn wheel(&mut self, dy: f32) {
            let pointer = egui::Event::PointerMoved(self.region.center());
            let notch = egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, dy),
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
            };
            self.frame(vec![pointer, notch]);
            for _ in 0..3 {
                self.frame(vec![]);
            }
        }

        /// Wheels the inspector until the whole row of `preset` is inside the visible region.
        fn bring_into_view(&mut self, preset: &str) -> Rect {
            for _ in 0..400 {
                let row = self.row(preset).unwrap_or_else(|| panic!("the row of preset {preset:?} is not in the inspector at all"));
                if row.top() >= self.region.top() && row.bottom() <= self.region.bottom() {
                    return row;
                }
                self.wheel(if row.top() < self.region.top() { 48.0 } else { -48.0 });
            }
            panic!("the row of preset {preset:?} never scrolled into the inspector");
        }

        /// Scrolls the list so the row of `preset` starts exactly at `top`.
        fn scroll_row_to(&mut self, preset: &str, top: f32) {
            let row = self.row(preset).unwrap_or_else(|| panic!("the row of preset {preset:?} is not in the inspector at all"));
            self.app.proto.presets_scroll += row.top() - top;
            self.frame(vec![]);
            self.frame(vec![]);
            let now = self.row(preset).expect("preset row").top();
            assert!((now - top).abs() < 0.01, "{preset:?} row starts at {now}, wanted {top}: the list cannot scroll that far");
        }

        fn click(&mut self, at: Pos2) {
            self.frame(vec![egui::Event::PointerMoved(at)]);
            for pressed in [true, false] {
                self.frame(vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE }]);
            }
            self.frame(vec![]);
        }

        /// Every text run painted last frame: string and logical rect.
        fn painted_text(&self) -> Vec<(String, Rect)> {
            fn collect(shape: &egui::Shape, out: &mut Vec<(String, Rect)>) {
                match shape {
                    egui::Shape::Text(t) => out.push((t.galley.job.text.clone(), Rect::from_min_size(t.pos, t.galley.size()))),
                    egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| collect(s, out)),
                    _ => {}
                }
            }
            let mut out = Vec::new();
            for clipped in &self.shapes {
                collect(&clipped.shape, &mut out);
            }
            out
        }
    }

    /// Clicks every catalogue row in a window of `window` size, scrolling to it first; each click must
    /// create that preset's frame, and the pointer target must stay at least 24 px tall.
    fn assert_every_preset_row_creates_its_frame(window: Vec2) {
        let mut ins = PresetInspector::new(window);
        for preset in crate::proto::FRAME_PRESETS {
            let row = ins.bring_into_view(preset.name);
            // f32 rect arithmetic at a fractional scroll offset lands within 1e-4 of the 24 px row height.
            assert!(row.height() >= 24.0 - 0.01, "{} row is {} px tall", preset.name, row.height());
            assert!(ins.region.contains(row.center()), "{} row center is outside the inspector", preset.name);
            let before = ins.app.doc.nodes.len();
            ins.click(row.center());
            assert_eq!(ins.app.doc.nodes.len(), before + 1, "clicking {} creates exactly one frame", preset.name);
            let id = *ins.app.selection.first().unwrap_or_else(|| panic!("clicking {} selects nothing", preset.name));
            let node = ins.app.doc.get(id).unwrap();
            assert_eq!((node.name.as_str(), node.size.x, node.size.y), (preset.name, preset.width, preset.height));
            ins.app.selection.clear();
            ins.frame(vec![]);
        }
        assert!(
            ins.rect_named("scrollbar-track").is_some(),
            "{} presets do not fit {} px, so the list needs a scrollbar",
            crate::proto::FRAME_PRESETS.len(),
            ins.region.height()
        );
    }

    #[test]
    fn every_frame_preset_row_is_clickable_after_scrolling_in_the_default_window() {
        assert_every_preset_row_creates_its_frame(Vec2::new(theme::DEFAULT_WINDOW.0, theme::DEFAULT_WINDOW.1));
    }

    #[test]
    fn every_frame_preset_row_is_clickable_after_scrolling_in_the_minimum_window() {
        assert_every_preset_row_creates_its_frame(Vec2::new(theme::MIN_WINDOW.0, theme::MIN_WINDOW.1));
    }

    #[test]
    fn frame_preset_list_scrolls_exactly_to_its_last_row() {
        let mut ins = PresetInspector::new(Vec2::new(theme::MIN_WINDOW.0, theme::MIN_WINDOW.1));
        let last = crate::proto::FRAME_PRESETS.last().expect("catalogue is not empty").name;
        for _ in 0..200 {
            ins.wheel(-120.0);
        }
        let row = ins.row(last).unwrap_or_else(|| panic!("the row of preset {last:?} is not in the inspector at all"));
        let slack = ins.region.bottom() - row.bottom();
        assert!((0.0..=24.0).contains(&slack), "the last row ends {slack} px above the footer: it must be fully visible without blank scroll space");
        for _ in 0..200 {
            ins.wheel(120.0);
        }
        let first = crate::proto::FRAME_PRESETS.first().expect("catalogue is not empty").name;
        let row = ins.row(first).expect("first preset row");
        assert!(row.top() >= ins.region.top() && row.top() - ins.region.top() <= 80.0, "scrolled back to the top, the first row sits right below the title");
    }

    /// The Prototype tab and the zoom control, as points in their lower half (where a row that sticks out
    /// above the list region would cover them).
    fn header_controls_lower_half(ins: &PresetInspector) -> [(&'static str, Pos2); 2] {
        let tab = ins.rect_named("right-tab-Prototype").expect("Prototype tab");
        let zoom = Rect::from_min_size(
            Pos2::new(ins.rect.right() - theme::RIGHT_ZOOM_FROM_RIGHT, ins.rect.top() + theme::RIGHT_TABS_Y),
            Vec2::new(theme::RIGHT_ZOOM_W, theme::RIGHT_TABS_H),
        );
        [
            ("Prototype tab", Pos2::new(tab.center().x, tab.top() + tab.height() * 0.75)),
            ("zoom control", Pos2::new(zoom.center().x, zoom.top() + zoom.height() * 0.75)),
        ]
    }

    #[test]
    fn header_tabs_and_zoom_still_work_while_a_preset_row_sticks_out_above_the_list() {
        for above in [1.0, 5.0, 10.0, 14.0, 23.0] {
            for which in 0..2 {
                let mut ins = PresetInspector::new(Vec2::new(theme::DEFAULT_WINDOW.0, theme::DEFAULT_WINDOW.1));
                ins.scroll_row_to("iPhone 17", ins.region.top() - above);
                let (name, at) = header_controls_lower_half(&ins)[which];
                if above == 23.0 {
                    let row = ins.row("iPhone 17").expect("preset row");
                    assert!(row.contains(at), "the straddling row {row:?} reaches the {name} at {at:?}");
                }
                let frames = ins.app.doc.nodes.len();
                ins.click(at);
                assert_eq!(ins.app.doc.nodes.len(), frames, "{name}: a click with a row {above} px above the list must not create a frame");
                match which {
                    0 => assert_eq!(ins.app.ui.right_tab, Some(RightTab::Prototype), "row {above} px above the list: the Prototype tab must switch"),
                    _ => assert_eq!(ins.app.ui.menu, Some(MenuKind::Zoom), "row {above} px above the list: the zoom control must open its menu"),
                }
            }
        }
    }

    #[test]
    fn a_preset_row_cut_by_the_list_edges_is_clickable_only_where_it_is_visible() {
        let mut ins = PresetInspector::new(Vec2::new(theme::DEFAULT_WINDOW.0, theme::DEFAULT_WINDOW.1));
        let x = ins.region.left() + 40.0;
        // Cut by the top edge: 10 px of the row are hidden above, 14 px show.
        ins.scroll_row_to("iPhone 17", ins.region.top() - 10.0);
        let frames = ins.app.doc.nodes.len();
        ins.click(Pos2::new(x, ins.region.top() + 6.0));
        assert_eq!(ins.app.doc.nodes.len(), frames + 1, "the visible part of a row cut by the top edge creates its frame");
        assert_eq!(ins.app.doc.get(ins.app.selection[0]).map(|n| n.name.as_str()), Some("iPhone 17"));
        // Probes sit 8 px outside the list: past egui's 5 px click radius, which lets a click just beside
        // any widget hit it, but well inside the 24 px row rect that an unclipped hit area would have.
        let mut ins = PresetInspector::new(Vec2::new(theme::DEFAULT_WINDOW.0, theme::DEFAULT_WINDOW.1));
        ins.scroll_row_to("iPhone 17", ins.region.top() - 23.0);
        let frames = ins.app.doc.nodes.len();
        ins.click(Pos2::new(x, ins.region.top() - 8.0));
        assert_eq!(ins.app.doc.nodes.len(), frames, "the strip between the tabs and the list takes no row clicks");
        // Cut by the bottom edge: the row starts 2 px above it, 22 px sit in the footer.
        let mut ins = PresetInspector::new(Vec2::new(theme::DEFAULT_WINDOW.0, theme::DEFAULT_WINDOW.1));
        ins.scroll_row_to("Instagram post", ins.region.bottom() - 2.0);
        let frames = ins.app.doc.nodes.len();
        ins.click(Pos2::new(x, ins.region.bottom() + 8.0));
        assert_eq!(ins.app.doc.nodes.len(), frames, "the footer below the list takes no row clicks");
        ins.click(Pos2::new(x, ins.region.bottom() - 1.0));
        assert_eq!(ins.app.doc.nodes.len(), frames + 1, "the visible 2 px of a row cut by the bottom edge create its frame");
    }

    #[test]
    fn frame_preset_scrollbar_track_click_jumps_to_the_end_of_the_list() {
        let mut ins = PresetInspector::new(Vec2::new(theme::MIN_WINDOW.0, theme::MIN_WINDOW.1));
        let last = crate::proto::FRAME_PRESETS.last().expect("catalogue is not empty").name;
        let track = ins.rect_named("scrollbar-track").expect("the list is taller than the inspector, so it shows a scrollbar");
        assert!(ins.row(last).is_some_and(|r| r.top() > ins.region.bottom()), "the last row starts below the fold");
        ins.click(Pos2::new(track.center().x, track.bottom() - 2.0));
        ins.frame(vec![]);
        let row = ins.row(last).expect("last preset row");
        assert!(
            row.top() >= ins.region.top() && row.bottom() <= ins.region.bottom(),
            "clicking the bottom of the track brings the last row into view, got {row:?} in {:?}",
            ins.region
        );
    }

    #[test]
    fn frame_preset_names_keep_a_grid_gap_to_their_size_label() {
        // Tall window: the whole catalogue is laid out without any scrolling.
        let mut ins = PresetInspector::new(Vec2::new(theme::DEFAULT_WINDOW.0, 2400.0));
        ins.frame(vec![]);
        let texts = ins.painted_text();
        let (mut cramped, mut elided) = (Vec::new(), 0);
        for preset in crate::proto::FRAME_PRESETS {
            let row = ins.row(preset.name).unwrap_or_else(|| panic!("no row for {}", preset.name));
            let mut in_row: Vec<&(String, Rect)> =
                texts.iter().filter(|(_, r)| r.center().y > row.top() && r.center().y < row.bottom() && r.left() >= row.left()).collect();
            in_row.sort_by(|a, b| a.1.left().total_cmp(&b.1.left()));
            assert_eq!(in_row.len(), 2, "{}: expected a name and a size label in the row, got {in_row:?}", preset.name);
            let ((name, name_r), (size, size_r)) = (in_row[0], in_row[1]);
            assert_eq!(size, &format!("{}×{}", preset.width as i32, preset.height as i32));
            assert!(name_r.left() >= row.left() && size_r.right() <= row.right(), "{}: text leaves its row", preset.name);
            let gap = size_r.left() - name_r.right();
            if gap < 8.0 {
                cramped.push(format!("{} ({gap:.1} px)", preset.name));
            }
            if name != preset.name {
                elided += 1;
                assert!(name.ends_with('…') && preset.name.starts_with(name.trim_end_matches('…')), "{}: elided as {name:?}", preset.name);
            }
        }
        assert!(cramped.is_empty(), "name and size label need at least 8 px between them, the rows below have less: {cramped:?}");
        assert!(elided >= 1, "the longest watch names do not fit the 240 px inspector next to their size and must be elided");
    }

    fn run_panel(dev_mode: bool) -> (egui::Context, egui::FullOutput) {
        let mut doc = Document::new("TestDoc");
        let _ = doc.add_page("Page 1");
        let mut app = App::new(doc);
        app.ui.dev_mode = dev_mode;
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(240.0, 800.0));
        let out = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| {
            show(&mut app, ui, screen);
        });
        (ctx, out)
    }

    fn named(ctx: &egui::Context, name: &str) -> Rect {
        ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", name.to_string())))).unwrap_or_else(|| panic!("missing recorded rect {name}"))
    }

    fn share_label_is_on_brand(out: &egui::FullOutput) -> bool {
        out.shapes.iter().any(|clipped| match &clipped.shape {
            egui::epaint::Shape::Text(text) if text.galley.job.text == "Share" => {
                text.galley.job.sections.first().is_some_and(|section| section.format.color == theme::TEXT_ON_BRAND)
            }
            _ => false,
        })
    }

    #[test]
    fn share_is_the_brand_primary_with_a_dark_label() {
        for dev_mode in [false, true] {
            let (ctx, out) = run_panel(dev_mode);
            let share = named(&ctx, "share-button");
            let fill = out.shapes.iter().any(|clipped| match &clipped.shape {
                egui::epaint::Shape::Rect(shape) => shape.fill == theme::BG_BRAND && shape.rect == share,
                _ => false,
            });
            assert!(fill, "dev_mode={dev_mode}: Share fill is the brand primary");
            assert!(share_label_is_on_brand(&out), "dev_mode={dev_mode}: Share label is the dark on-brand text");
        }
    }

    #[test]
    fn present_chevron_hit_box_meets_the_target_floor() {
        assert_eq!(theme::RIGHT_PRESENT_CHEVRON_W, 16.0);
        assert_eq!(theme::RIGHT_PRESENT_FROM_RIGHT, 56.0);
        let (ctx, _) = run_panel(false);
        let present = named(&ctx, "present-button");
        let visual = named(&ctx, "present-chevron");
        let hit = named(&ctx, "present-chevron-hit");
        let share = named(&ctx, "share-button");
        assert!(hit.width() + 0.01 >= theme::TARGET_MIN, "hit {} is below the target floor", hit.width());
        assert!((hit.height() - theme::RIGHT_BTN_SIZE).abs() < 0.5, "hit height {}", hit.height());
        assert!((hit.left() - present.right()).abs() < 0.5, "hit starts at the present button");
        assert!(hit.right() <= share.left() + 0.5, "hit ends before Share");
        assert!(!area_overlap(hit, present) && !area_overlap(hit, share), "hit overlaps a neighbour");
        assert!((visual.width() - theme::RIGHT_PRESENT_CHEVRON_W).abs() < 0.5, "visual chevron width {}", visual.width());
    }

    #[test]
    fn header_controls_are_xs_tall_and_do_not_overlap() {
        let (ctx, out) = run_panel(false);
        let panel = Rect::from_min_size(Pos2::ZERO, Vec2::new(240.0, 800.0));
        let mut rects = Vec::new();
        for name in ["right-top-avatar", "present-button", "present-chevron", "share-button"] {
            let rect = named(&ctx, name);
            assert!((rect.top() - (panel.top() + theme::RIGHT_BTN_Y)).abs() < 0.5, "{name} top {}", rect.top());
            assert!((rect.height() - theme::RIGHT_BTN_SIZE).abs() < 0.5, "{name} height {}", rect.height());
            rects.push(rect);
        }
        let share = named(&ctx, "share-button");
        assert!((panel.right() - share.right() - theme::INSET_CONTROL).abs() < 0.5, "Share ends {} px from the right edge", panel.right() - share.right());
        let tab = named(&ctx, "right-tab-Design");
        assert!((tab.top() - (panel.top() + theme::RIGHT_TABS_Y)).abs() < 0.5, "tab strip y {}", tab.top());
        let zoom = Rect::from_min_size(
            Pos2::new(panel.right() - theme::RIGHT_ZOOM_FROM_RIGHT, panel.top() + theme::RIGHT_TABS_Y),
            Vec2::new(theme::RIGHT_ZOOM_W, theme::RIGHT_TABS_H),
        );
        assert!((zoom.height() - theme::CONTROL_DENSE).abs() < 0.5);
        let zoom_label = out.shapes.iter().any(|clipped| match &clipped.shape {
            egui::epaint::Shape::Text(text) if text.galley.job.text.contains('%') => {
                let text_rect = Rect::from_min_size(text.pos, text.galley.size());
                zoom.top() <= text_rect.center().y && text_rect.center().y <= zoom.bottom()
            }
            _ => false,
        });
        assert!(zoom_label, "zoom control is 24 tall");
        for (i, a) in rects.iter().enumerate() {
            for b in rects.iter().skip(i + 1) {
                assert!(!area_overlap(*a, *b), "header controls overlap: {a:?} {b:?}");
            }
        }
    }

    fn area_overlap(a: Rect, b: Rect) -> bool {
        a.min.x.max(b.min.x) < a.max.x.min(b.max.x) && a.min.y.max(b.min.y) < a.max.y.min(b.max.y)
    }

    #[test]
    fn avatar_uses_identity_colours_and_a_dark_initial() {
        let user = std::env::var("USER").unwrap_or_else(|_| "jangtrinh".into());
        let disc = crate::comments::ui::avatar_color(&user);
        let initial = crate::comments::model::initial(&user);
        let (_, out) = run_panel(false);
        let circle = out.shapes.iter().any(|clipped| match &clipped.shape {
            egui::epaint::Shape::Circle(shape) => shape.fill == disc,
            _ => false,
        });
        assert!(circle, "avatar disc is the identity colour for {user}");
        let dark = out.shapes.iter().any(|clipped| match &clipped.shape {
            egui::epaint::Shape::Text(text) if text.galley.job.text == initial => {
                text.galley.job.sections.first().is_some_and(|section| section.format.color == theme::TEXT_ON_BRAND)
            }
            _ => false,
        });
        assert!(dark, "avatar initial is the dark on-brand text");
    }
}
