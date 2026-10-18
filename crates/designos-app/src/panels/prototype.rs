//! Right panel, Prototype tab: flows, device and background when nothing is
//! selected; interactions and scroll behaviour for a selected layer, plus the
//! interaction editor popover.

use super::properties::{checkbox, open_picker, picker_popup, section_sep, sublabel, title, Pane, PickTarget};
use super::proto_actions;
use super::{clipped, scrollbar, wheel_scroll};
use crate::app::App;
use crate::icons::{self, Icon};
use crate::proto::{self, easing, player::top_level_frames};
use crate::theme;
use crate::widgets::field::{self, Prefix};
use crate::widgets::menu::dropdown;
use crate::widgets::{self, record};
use designos_doc::*;
use egui::{Align2, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

pub fn show(app: &mut App, ui: &mut Ui, region: Rect) {
    let mut scroll = app.proto.scroll;
    wheel_scroll(ui, region, &mut scroll, app.proto.content_h);
    app.proto.scroll = scroll;
    let sel = app.selection.clone();
    // The editor follows the selection.
    if let Some((n, i)) = app.proto.editing {
        let valid = sel == [n] && app.doc.get(n).is_some_and(|x| i < x.prototype.reactions.len());
        if !valid {
            app.proto.editing = None;
        }
    }
    let end = clipped(ui, region, |ui| {
        let mut pane = Pane { x0: region.left() - theme::BORDER_WIDTH, oy: region.top() - scroll, y: 0.0, title_end: 0.0 };
        if sel.is_empty() {
            let _ = title(ui, &mut pane, "Interactions", Some(Icon::Plus), false);
            show_prototype_settings_btn(app, ui, &mut pane);
            section_sep(ui, &mut pane, false);
            prototype_guide_cards(ui, &mut pane);
            flows_section(app, ui, &mut pane);
            device_section(app, ui, &mut pane);
            background_section(app, ui, &mut pane);
        } else {
            selection_sections(app, ui, &mut pane, &sel);
        }
        pane.y + 24.0
    });
    app.proto.content_h = end;
    scrollbar(ui, region, scroll, end);
    if let Some(s) = super::take_scrollbar_drag(ui, region) {
        app.proto.scroll = s;
    }
    picker_popup(app, ui, region);
    popover(app, ui, region);
}

fn show_prototype_settings_btn(app: &mut App, ui: &mut Ui, pane: &mut Pane) {
    let btn_r = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, 28.0);
    record(ui, "proto-show-settings", btn_r);
    let resp = ui.interact(btn_r, ui.id().with("proto-show-settings"), Sense::click());
    ui.painter().rect_filled(btn_r, theme::cr(6), if resp.hovered() { Color32::from_rgb(0x38, 0x38, 0x38) } else { Color32::from_rgb(0x2C, 0x2C, 0x2C) });
    ui.painter().rect_stroke(btn_r, theme::cr(6), Stroke::new(1.0, Color32::from_rgb(0x38, 0x38, 0x38)), StrokeKind::Inside);
    ui.painter().text(btn_r.center(), Align2::CENTER_CENTER, "Show prototype settings", theme::font_body_strong(), Color32::WHITE);
    if resp.clicked() {
        app.selection.clear();
    }
    pane.y += 34.0;
}

fn prototype_guide_cards(ui: &mut Ui, pane: &mut Pane) {
    for (key, heading, body_text) in [
        (
            "proto-guide-connection",
            "Creating a connection",
            "Select a frame or object in a frame and use the circular node to drag a connection to another frame.",
        ),
        (
            "proto-guide-running",
            "Running your prototype",
            "Use the play button in the toolbar to play your prototype. If there are no connections, the play button can be used to play a presentation of your frames.",
        ),
    ] {
        let dismiss_id = egui::Id::new(("proto-guide-dismissed", key));
        if ui.ctx().data(|d| d.get_temp::<bool>(dismiss_id).unwrap_or(false)) {
            continue;
        }
        pane.y += 8.0;
        let hdr_r = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, 22.0);
        record(ui, key, hdr_r);
        ui.painter().text(Pos2::new(hdr_r.left(), hdr_r.center().y), Align2::LEFT_CENTER, heading, theme::font_body_strong(), Color32::WHITE);
        let close_r = Rect::from_min_size(Pos2::new(hdr_r.right() - 20.0, hdr_r.center().y - 10.0), Vec2::splat(20.0));
        record(ui, &format!("{key}-dismiss"), close_r);
        if widgets::icon_button(ui, close_r, (key, "dismiss"), Icon::Close, false, "Dismiss tip").clicked() {
            ui.ctx().data_mut(|d| d.insert_temp(dismiss_id, true));
        }
        pane.y += 24.0;
        let text_r = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, 48.0);
        let galley = ui.painter().layout(
            body_text.to_string(),
            theme::font_body(),
            Color32::from_rgb(0xB3, 0xB3, 0xB3),
            text_r.width(),
        );
        let h = galley.size().y;
        ui.painter().galley(text_r.min, galley, Color32::from_rgb(0xB3, 0xB3, 0xB3));
        pane.y += h + 12.0;
        section_sep(ui, pane, false);
    }
}

fn hint(ui: &Ui, pane: &mut Pane, text: &str) {
    let r = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, 32.0);
    let galley = ui.painter().layout(text.to_string(), theme::font_body(), theme::TEXT_TERTIARY, r.width());
    let h = galley.size().y;
    ui.painter().galley(r.min, galley, theme::TEXT_TERTIARY);
    pane.y += h + 12.0;
}

// ---------------------------------------------------------------- nothing selected
fn flows_section(app: &mut App, ui: &mut Ui, pane: &mut Pane) {
    let add = title(ui, pane, "Flow starting points", Some(Icon::Plus), false);
    record(ui, "proto-flows-add", pane.rect(theme::SECTION_TRAILING_X, -32.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON));
    let here: Vec<(usize, Flow)> = app.doc.flows.iter().cloned().enumerate().filter(|(_, f)| f.page == app.page).collect();
    if add {
        let used: Vec<NodeId> = app.doc.flows.iter().map(|f| f.start).collect();
        match top_level_frames(&app.doc, app.page).into_iter().find(|f| !used.contains(f)) {
            Some(f) => app.proto_add_flow(f),
            None => app.show_toast("Every frame on this page already starts a flow", false),
        }
    }
    if here.is_empty() {
        hint(ui, pane, "Select a frame and add it as a flow starting point to present from it.");
    }
    for (i, f) in here {
        let row = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W - 28.0, theme::FIELD_HEIGHT);
        record(ui, &format!("proto-flow-{i}"), row);
        icons::paint(ui.painter(), Icon::Flag, Pos2::new(row.left() + 8.0, row.center().y), 14.0, theme::PROTO_BLUE);
        let name_r = Rect::from_min_max(Pos2::new(row.left() + 20.0, row.top()), Pos2::new(row.right() - 4.0, row.bottom()));
        if let Some(s) = field::text_input(ui, name_r, ("flow-name", i), &f.name) {
            let name = s.trim().to_string();
            if !name.is_empty() {
                let mut flows = app.doc.flows.clone();
                flows[i].name = name;
                app.proto_set_flows(flows);
            }
        }
        let play_r = pane.rect(theme::SECTION_TRAILING_X - 26.0, 0.0, theme::PANEL_ACTION_BUTTON, theme::FIELD_HEIGHT);
        record(ui, &format!("proto-flow-play-{i}"), play_r);
        if widgets::icon_button(ui, play_r, ("flow-play", i), Icon::Play, false, "Present this flow").clicked() {
            app.select_only(f.start);
            app.start_present();
            return;
        }
        let del = pane.rect(theme::SECTION_TRAILING_X, 0.0, theme::PANEL_ACTION_BUTTON, theme::FIELD_HEIGHT);
        record(ui, &format!("proto-flow-del-{i}"), del);
        if widgets::icon_button(ui, del, ("flow-del", i), Icon::Minus, false, "Remove starting point").clicked() {
            let mut flows = app.doc.flows.clone();
            flows.remove(i);
            app.proto_set_flows(flows);
            return;
        }
        pane.y += theme::FIELD_HEIGHT + 2.0;
        let start = app.doc.get(f.start).map(|n| n.name.clone()).unwrap_or_else(|| "(missing)".into());
        let r = pane.rect(theme::SECTION_CONTENT_X + 20.0, 0.0, theme::SECTION_CONTENT_W - 20.0, 16.0);
        record(ui, &format!("proto-flow-start-{i}"), r);
        let start_resp = ui.interact(r, ui.id().with(("flow-start-jump", i)), Sense::click());
        let start_col = if start_resp.hovered() { theme::PROTO_BLUE } else { theme::TEXT_TERTIARY };
        ui.painter().text(Pos2::new(r.left(), r.center().y), Align2::LEFT_CENTER, format!("Starts at {start}"), theme::font_caption(), start_col);
        if start_resp.on_hover_text("Click to select & focus starting frame").clicked() {
            app.select_only(f.start);
            app.fit_selection();
        }
        pane.y += 20.0;
    }
    pane.y += 4.0;
    section_sep(ui, pane, false);
}

fn device_section(app: &mut App, ui: &mut Ui, pane: &mut Pane) {
    title(ui, pane, "Device", None, false);
    let names: Vec<String> =
        proto::DEVICES.iter().map(|d| if d.1 > 0.0 { format!("{} ({}×{})", d.0, d.1 as i32, d.2 as i32) } else { d.0.to_string() }).collect();
    let stored = app.doc.prototype_device.clone();
    // A device from elsewhere (not one of the presets) is shown by name and size.
    let label = match proto::device_index(&stored) {
        Some(i) => names[i].clone(),
        None => {
            let sz = stored.size.map(|s| format!(" ({}×{})", s.x as i32, s.y as i32)).unwrap_or_default();
            format!("{}{sz}", stored.preset.clone().unwrap_or_default())
        }
    };
    let r = pane.full(0.0);
    record(ui, "proto-device", r);
    if let Some(i) = dropdown(ui, r, "proto-device", &label, &names) {
        let device = proto::device_for(i, &stored);
        app.set_prototype_device(device);
    }
    pane.y += theme::FIELD_HEIGHT + 12.0;
    section_sep(ui, pane, false);
}

fn background_section(app: &mut App, ui: &mut Ui, pane: &mut Pane) {
    title(ui, pane, "Background", None, false);
    let bg = match app.doc.get(app.page).map(|n| &n.kind) {
        Some(NodeKind::Page { background }) => *background,
        _ => Color::WHITE,
    };
    let c32 = Color32::from_rgb((bg.r * 255.0).round() as u8, (bg.g * 255.0).round() as u8, (bg.b * 255.0).round() as u8);
    let sw = pane.rect(theme::SWATCH_ROW_X, 4.0, theme::SWATCH_SIZE, theme::SWATCH_SIZE);
    let hex_r = pane.rect(theme::SWATCH_ROW_X + theme::SWATCH_SIZE + 8.0, 0.0, theme::HEX_FIELD_W + 24.0, theme::FIELD_HEIGHT);
    record(ui, "proto-bg-swatch", sw);
    let swr = ui.interact(sw, ui.id().with("proto-bg"), Sense::click());
    widgets::swatch(ui.painter(), sw, c32);
    if swr.clicked() {
        open_picker(app, PickTarget::PageBg, c32, sw, vec![app.page]);
    }
    if let Some(s) = field::text_input(ui, hex_r, "proto-bg-hex", &bg.to_hex()) {
        if let Some(c) = Color::from_hex(&s) {
            let page = app.page;
            app.edit_nodes(&[page], |n| n.kind = NodeKind::Page { background: Color { a: bg.a, ..c } });
        }
    }
    pane.y += 36.0;
}

// ---------------------------------------------------------------- selection
fn selection_sections(app: &mut App, ui: &mut Ui, pane: &mut Pane, sel: &[NodeId]) {
    let single = if let [one] = sel { Some(*one) } else { None };

    // Flow starting point for a top-level frame.
    if let Some(id) = single {
        let top_level = app.doc.get(id).and_then(|n| n.parent).and_then(|p| app.doc.get(p)).is_some_and(|p| matches!(p.kind, NodeKind::Page { .. }));
        let frame_like = app.doc.get(id).is_some_and(|n| matches!(n.kind, NodeKind::Frame(_) | NodeKind::Component(_) | NodeKind::Section));
        if top_level && frame_like {
            let existing = app.doc.flows.iter().position(|f| f.start == id);
            let add = title(ui, pane, "Flow starting point", if existing.is_none() { Some(Icon::Plus) } else { None }, false);
            record(ui, "proto-flow-add", pane.rect(theme::SECTION_TRAILING_X, -32.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON));
            if add {
                app.proto_add_flow(id);
            }
            if let Some(i) = existing {
                let row = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, theme::FIELD_HEIGHT);
                icons::paint(ui.painter(), Icon::Flag, Pos2::new(row.left() + 8.0, row.center().y), 14.0, theme::PROTO_BLUE);
                let name_r = Rect::from_min_max(Pos2::new(row.left() + 20.0, row.top()), Pos2::new(row.right() - 4.0, row.bottom()));
                let cur = app.doc.flows[i].name.clone();
                if let Some(s) = field::text_input(ui, name_r, "sel-flow-name", &cur) {
                    if !s.trim().is_empty() {
                        let mut flows = app.doc.flows.clone();
                        flows[i].name = s.trim().to_string();
                        app.proto_set_flows(flows);
                    }
                }
                let del = pane.rect(theme::SECTION_TRAILING_X, 0.0, theme::PANEL_ACTION_BUTTON, theme::FIELD_HEIGHT);
                if widgets::icon_button(ui, del, "sel-flow-del", Icon::Minus, false, "Remove starting point").clicked() {
                    let mut flows = app.doc.flows.clone();
                    flows.remove(i);
                    app.proto_set_flows(flows);
                }
                pane.y += theme::FIELD_HEIGHT + 12.0;
            } else {
                pane.y += 4.0;
            }
            section_sep(ui, pane, false);
        }
    }

    // Interactions.
    let add = title(ui, pane, "Interactions", single.map(|_| Icon::Plus), false);
    record(ui, "proto-add", pane.rect(theme::SECTION_TRAILING_X, -32.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON));
    show_prototype_settings_btn(app, ui, pane);
    match single {
        None => hint(ui, pane, "Select a single layer to edit its interactions."),
        Some(id) => {
            if add {
                app.proto_add_reaction(id);
            }
            let rs = app.doc.get(id).map(|n| n.prototype.reactions.clone()).unwrap_or_default();
            if rs.is_empty() {
                section_sep(ui, pane, false);
                prototype_guide_cards(ui, pane);
            } else {
                for (i, r) in rs.iter().enumerate() {
                    reaction_row(app, ui, pane, id, i, r);
                }
                pane.y += 4.0;
                section_sep(ui, pane, false);
            }
        }
    }
    if single.is_none() {
        section_sep(ui, pane, false);
    }

    // Video fill: playback options (the poster frame is the layer's image fill).
    if let Some(id) = single {
        if let Some(v) = app.doc.get(id).and_then(|n| n.prototype.videos.first().cloned()) {
            video_section(app, ui, pane, id, &v);
        }
    }

    // Scroll behaviour.
    title(ui, pane, "Scroll behavior", None, false);
    let frames: Vec<NodeId> = sel
        .iter()
        .copied()
        .filter(|i| app.doc.get(*i).is_some_and(|n| matches!(n.kind, NodeKind::Frame(_) | NodeKind::Component(_) | NodeKind::Instance { .. })))
        .collect();
    if !frames.is_empty() {
        sublabel(ui, pane, "Overflow");
        let cur: Option<Overflow> = {
            let mut it = frames.iter().filter_map(|i| app.doc.get(*i)).map(|n| n.prototype.overflow);
            let first = it.next();
            first.filter(|f| it.all(|o| o == *f))
        };
        let names: Vec<String> = proto::OVERFLOWS.iter().map(|o| o.0.to_string()).collect();
        let label = cur.and_then(|c| proto::OVERFLOWS.iter().find(|o| o.1 == c)).map(|o| o.0).unwrap_or("Mixed");
        let r = pane.full(0.0);
        record(ui, "proto-overflow", r);
        if let Some(i) = dropdown(ui, r, "proto-overflow", label, &names) {
            let v = proto::OVERFLOWS[i].1;
            app.proto_edit(&frames, |p| p.overflow = v);
        }
        pane.y += theme::FIELD_HEIGHT + 8.0;
    }
    // Fixed position inside a scrolling parent (any non-top-level layer).
    let inner: Vec<(NodeId, NodeId)> = sel
        .iter()
        .filter_map(|i| Some((*i, app.doc.get(*i)?.parent?)))
        .filter(|(_, p)| app.doc.get(*p).is_some_and(|n| !matches!(n.kind, NodeKind::Page { .. })))
        .collect();
    if !inner.is_empty() {
        sublabel(ui, pane, "Position");
        let fixed: Vec<bool> = inner.iter().map(|(i, p)| app.doc.get(*p).is_some_and(|n| n.prototype.fixed_children.contains(i))).collect();
        let cur = if fixed.iter().all(|f| *f) {
            Some(true)
        } else if fixed.iter().all(|f| !*f) {
            Some(false)
        } else {
            None
        };
        let names = vec!["Scroll with parent".to_string(), "Fixed (stay in place)".to_string()];
        let label = match cur {
            Some(true) => &names[1],
            Some(false) => &names[0],
            None => "Mixed",
        };
        let r = pane.full(0.0);
        record(ui, "proto-position", r);
        if let Some(i) = dropdown(ui, r, "proto-position", label, &names) {
            let want_fixed = i == 1;
            let parents: Vec<NodeId> = inner.iter().map(|(_, p)| *p).collect();
            let kids: Vec<(NodeId, NodeId)> = inner.clone();
            app.edit_nodes(&parents, |n| {
                for (c, p) in &kids {
                    if *p != n.id {
                        continue;
                    }
                    n.prototype.fixed_children.retain(|x| x != c);
                    if want_fixed {
                        n.prototype.fixed_children.push(*c);
                    }
                }
            });
        }
        pane.y += theme::FIELD_HEIGHT + 8.0;
    }
    if frames.is_empty() && inner.is_empty() {
        hint(ui, pane, "Select a frame to set how its content scrolls.");
    }
}

/// One line about the video file: codec, size, length, or why it only shows its poster.
fn video_status(app: &App, ui: &Ui, source: &str) -> String {
    let Some(bytes) = app.doc.images.get(source) else { return "Video file is missing".into() };
    let id = egui::Id::new(("video-status", source, bytes.len()));
    if let Some(s) = ui.data(|d| d.get_temp::<String>(id)) {
        return s;
    }
    let s = match crate::video::probe(bytes) {
        None => "Not a video file".to_string(),
        Some(i) => match i.unsupported_reason() {
            Some(r) => format!("{r} (shows the poster frame)"),
            None => format!("{} {}x{}, {:.1}s", i.codec.name(), i.width, i.height, i.duration),
        },
    };
    ui.data_mut(|d| d.insert_temp(id, s.clone()));
    s
}

fn video_section(app: &mut App, ui: &mut Ui, pane: &mut Pane, id: NodeId, v: &VideoFill) {
    title(ui, pane, "Video", None, false);
    let status = video_status(app, ui, &v.source);
    let ok = !status.starts_with("Unsupported") && !status.contains("missing") && !status.starts_with("Not a");
    let galley = ui.painter().layout(status, theme::font_caption(), if ok { theme::TEXT_TERTIARY } else { theme::DANGER }, theme::SECTION_CONTENT_W);
    let r = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, galley.size().y);
    record(ui, "proto-video-status", r);
    let h = galley.size().y;
    ui.painter().galley(r.min, galley, theme::TEXT_TERTIARY);
    pane.y += h + 8.0;
    for (key, label, value) in [("autoplay", "Autoplay", v.autoplay), ("loop", "Loop", v.looping), ("muted", "Muted", v.muted)] {
        record(ui, &format!("proto-video-{key}"), pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, theme::FIELD_HEIGHT));
        if let Some(nv) = checkbox(ui, pane, &format!("video-{key}"), label, value) {
            app.proto_edit(&[id], |p| {
                if let Some(f) = p.videos.first_mut() {
                    match key {
                        "autoplay" => f.autoplay = nv,
                        "loop" => f.looping = nv,
                        _ => f.muted = nv,
                    }
                }
            });
        }
    }
    pane.y += 4.0;
    section_sep(ui, pane, false);
}

fn reaction_row(app: &mut App, ui: &mut Ui, pane: &mut Pane, id: NodeId, i: usize, r: &Reaction) {
    let row = pane.rect(8.0, 0.0, theme::RIGHT_PANEL_WIDTH - 16.0, theme::PROTO_ROW_HEIGHT);
    record(ui, &format!("proto-row-{i}"), row);
    let resp = ui.interact(row, ui.id().with(("proto-row", i)), Sense::click());
    let open = app.proto.editing == Some((id, i));
    let p = ui.painter();
    if open {
        p.rect_filled(row, theme::cr(theme::RADIUS_CONTROL), theme::BG_SELECTED);
    } else if resp.hovered() {
        p.rect_filled(row, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
    }
    icons::paint(p, Icon::Play, Pos2::new(row.left() + 16.0, row.center().y), 12.0, theme::PROTO_BLUE);
    let text_w = row.width() - 32.0 - 36.0;
    let t1 = widgets::elide(p, &proto::trigger_label(&r.trigger), &theme::font_body_strong(), text_w);
    p.text(Pos2::new(row.left() + 32.0, row.top() + 14.0), Align2::LEFT_CENTER, t1, theme::font_body_strong(), theme::TEXT);
    let a = r.actions.iter().map(|a| proto::action_label(&app.doc, a)).collect::<Vec<_>>().join(", ");
    let t2 = widgets::elide(p, &a, &theme::font_body(), text_w);
    p.text(Pos2::new(row.left() + 32.0, row.top() + 30.0), Align2::LEFT_CENTER, t2, theme::font_body(), theme::TEXT_SECONDARY);
    let del = Rect::from_min_size(Pos2::new(row.right() - 28.0, row.center().y - 12.0), Vec2::splat(24.0));
    record(ui, &format!("proto-row-del-{i}"), del);
    let mut deleted = false;
    let pointer_in_row = ui.input(|i| i.pointer.hover_pos()).is_some_and(|p| row.contains(p));
    if pointer_in_row || open {
        deleted = widgets::icon_button(ui, del, ("proto-del", i), Icon::Minus, false, "Delete interaction").clicked();
    }
    if deleted {
        app.proto_edit(&[id], |p| {
            if i < p.reactions.len() {
                p.reactions.remove(i);
            }
        });
        app.proto.editing = None;
    } else if resp.clicked() {
        app.proto.editing = if open { None } else { Some((id, i)) };
        app.proto.editing_anchor = Some(row);
    }
    pane.y += theme::PROTO_ROW_HEIGHT + 2.0;
}

// ---------------------------------------------------------------- interaction editor
const LABEL_W: f32 = 72.0;

/// "Label ......[control]" row; returns the control rect.
pub(super) fn label_ctl(ui: &Ui, pane: &Pane, label: &str) -> Rect {
    let r = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, theme::FIELD_HEIGHT);
    ui.painter().text(Pos2::new(r.left(), r.center().y), Align2::LEFT_CENTER, label, theme::font_body(), theme::TEXT_SECONDARY);
    Rect::from_min_max(Pos2::new(r.left() + LABEL_W, r.top()), r.max)
}

/// A small tertiary note under a field.
fn hint_line(ui: &Ui, pane: &mut Pane, text: &str) {
    let r = pane.rect(theme::SECTION_CONTENT_X, -4.0, theme::SECTION_CONTENT_W, 14.0);
    let galley = ui.painter().layout(text.to_string(), theme::font_caption(), theme::TEXT_TERTIARY, r.width());
    let h = galley.size().y;
    ui.painter().galley(r.min, galley, theme::TEXT_TERTIARY);
    pane.y += h + 6.0;
}

pub(super) fn next_row(pane: &mut Pane) {
    pane.y += theme::FIELD_HEIGHT + 8.0;
}

fn name_of(doc: &Document, id: NodeId) -> String {
    doc.get(id).map(|n| n.name.clone()).unwrap_or_else(|| "(missing)".into())
}

pub(super) fn dest_dropdown(ui: &mut Ui, pane: &mut Pane, doc: &Document, key: &str, label: &str, cur: NodeId, options: &[NodeId]) -> Option<NodeId> {
    let r = label_ctl(ui, pane, label);
    record(ui, &format!("proto-pop-{key}"), r);
    let names: Vec<String> = options.iter().map(|o| name_of(doc, *o)).collect();
    let picked = dropdown(ui, r, ("pop-dest", key), &name_of(doc, cur), &names).map(|i| options[i]);
    next_row(pane);
    picked
}

/// Layers below a top-level frame, for Scroll to.
fn layers_below(doc: &Document, top: NodeId, cap: usize) -> Vec<NodeId> {
    let mut out = Vec::new();
    doc.walk(top, |n, _| {
        if n.id != top && out.len() < cap {
            out.push(n.id);
        }
    });
    out
}

/// Components that an instance of `src` can change to: siblings in its set, else all components.
pub(super) fn variants_of(doc: &Document, src: NodeId) -> Vec<NodeId> {
    let mut cur = Some(src);
    while let Some(id) = cur {
        let Some(n) = doc.get(id) else { break };
        if let NodeKind::Instance { component: Some(c), .. } = &n.kind {
            let set = doc.get(*c).and_then(|c| c.parent).filter(|p| matches!(doc.get(*p).map(|n| &n.kind), Some(NodeKind::ComponentSet(_))));
            if let Some(set) = set {
                return doc.get(set).map(|s| s.children.clone()).unwrap_or_default();
            }
            break;
        }
        cur = n.parent;
    }
    doc.nodes.values().filter(|n| matches!(n.kind, NodeKind::Component(_))).map(|n| n.id).take(100).collect()
}

fn popover(app: &mut App, ui: &mut Ui, region: Rect) {
    let Some((node, idx)) = app.proto.editing else { return };
    let Some(cur) = app.doc.get(node).and_then(|n| n.prototype.reactions.get(idx)).cloned() else {
        app.proto.editing = None;
        return;
    };
    let ctx = ui.ctx().clone();
    let anchor = app.proto.editing_anchor.unwrap_or(region);
    let w = theme::PROTO_POPOVER_WIDTH;
    let h = app.proto.popover_h.max(120.0);
    let top = (anchor.top() - 8.0).clamp(region.top() - 40.0, (ctx.content_rect().bottom() - h - 8.0).max(region.top() - 40.0));
    let pos = Pos2::new(region.left() - w - 8.0, top);
    let frames = proto::destination_frames(&app.doc, node);
    let own_top = proto::top_frame(&app.doc, node);
    let mut r = cur.clone();
    let mut delete = false;
    let mut measured = 0.0;
    let area = egui::Area::new(egui::Id::new("proto-popover")).order(egui::Order::Foreground).fixed_pos(pos).show(&ctx, |ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
        widgets::elevation_shadow(ui.painter(), rect, theme::RADIUS_FRAME);
        ui.painter().rect_filled(rect, theme::cr(theme::RADIUS_FRAME), theme::BG_MENU);
        ui.painter().rect_stroke(rect, theme::cr(theme::RADIUS_FRAME), Stroke::new(1.0, theme::BORDER_MENU), StrokeKind::Inside);
        record(ui, "proto-popover", rect);
        let mut pane = Pane { x0: rect.left() - 1.0, oy: rect.top(), y: 0.0, title_end: 0.0 };
        delete = title(ui, &mut pane, "Interaction details", Some(Icon::Minus), false);

        // Trigger.
        let tr = label_ctl(ui, &pane, "Trigger");
        record(ui, "proto-pop-trigger", tr);
        let names: Vec<String> = proto::TRIGGERS.iter().map(|s| s.to_string()).collect();
        let ti = proto::trigger_index(&r.trigger);
        if let Some(i) = dropdown(ui, tr, "pop-trigger", proto::TRIGGERS[ti], &names) {
            if i != ti {
                r.trigger = proto::trigger_of_index(i, &r.trigger);
            }
        }
        next_row(&mut pane);
        if let Some(d) = proto::trigger_delay(&r.trigger) {
            let c = label_ctl(ui, &pane, "Delay");
            record(ui, "proto-pop-delay", c);
            if let Some(v) = field::num_field(ui, c, "pop-delay", Prefix::None, Some(d as f32), " ms") {
                r.trigger = proto::with_delay(&r.trigger, v.max(0.0) as u32);
            }
            next_row(&mut pane);
        }
        if let Trigger::KeyPress { keys } = &r.trigger {
            let c = label_ctl(ui, &pane, "Keys");
            record(ui, "proto-pop-keys", c);
            if let Some(s) = field::text_input_ph(ui, c, "pop-keys", keys, "e.g. Ctrl+K") {
                r.trigger = Trigger::KeyPress { keys: s.trim().to_string() };
            }
            next_row(&mut pane);
            hint_line(ui, &mut pane, "Modifiers: Ctrl, Alt, Shift. Join keys with +.");
        }
        if let Trigger::GamepadPress { device, buttons } = r.trigger.clone() {
            let c = label_ctl(ui, &pane, "Controller");
            record(ui, "proto-pop-gamepad-device", c);
            let names: Vec<String> = proto::GAMEPAD_DEVICES.iter().map(|d| d.0.to_string()).collect();
            let cur = proto::gamepad_device_index(device);
            if let Some(i) = dropdown(ui, c, "pop-gamepad-device", proto::GAMEPAD_DEVICES[cur].0, &names) {
                r.trigger = Trigger::GamepadPress { device: proto::GAMEPAD_DEVICES[i].1, buttons: buttons.clone() };
            }
            next_row(&mut pane);
            let c = label_ctl(ui, &pane, "Buttons");
            record(ui, "proto-pop-gamepad-buttons", c);
            if let Some(s) = field::text_input_ph(ui, c, "pop-gamepad-buttons", &buttons, "e.g. A or LB+A") {
                r.trigger = Trigger::GamepadPress { device, buttons: s.trim().to_string() };
            }
            next_row(&mut pane);
            hint_line(ui, &mut pane, "A B X Y LB RB LT RT Back Start DPadUp/Down/Left/Right");
        }
        widgets::hline(ui.painter(), pane.x0, pane.x0 + w, pane.oy + pane.y, theme::BORDER_MENU);
        pane.y += 9.0;

        // Actions (run in order).
        let cx = proto_actions::EditCtx { node, frames: &frames, own_top, depth: 0, allow_empty: false };
        proto_actions::actions_editor(app, ui, &mut pane, cx, &mut r.actions, "");
        let add_r = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, theme::FIELD_HEIGHT);
        record(ui, "proto-pop-add-action", add_r);
        let ar = ui.interact(add_r, ui.id().with("pop-add-action"), Sense::click());
        icons::paint(
            ui.painter(),
            Icon::Plus,
            Pos2::new(add_r.left() + 8.0, add_r.center().y),
            12.0,
            if ar.hovered() { theme::ICON } else { theme::ICON_SECONDARY },
        );
        ui.painter().text(Pos2::new(add_r.left() + 22.0, add_r.center().y), Align2::LEFT_CENTER, "Add action", theme::font_body(), theme::TEXT_SECONDARY);
        if ar.clicked() {
            let fallback = frames.iter().copied().find(|f| Some(*f) != own_top).or(own_top).unwrap_or(node);
            r.actions.push(Action::Navigate { dest: fallback, preserve_scroll: false });
        }
        next_row(&mut pane);

        // Transition.
        if r.actions.iter().any(proto::action_animates) {
            widgets::hline(ui.painter(), pane.x0, pane.x0 + w, pane.oy + pane.y, theme::BORDER_MENU);
            pane.y += 9.0;
            transition_ui(ui, &mut pane, &mut r.transition);
        }
        pane.y += 8.0;
        measured = pane.y;
        rect
    });
    // Remember the height for next frame's background.
    if (measured - app.proto.popover_h).abs() > 0.5 {
        app.proto.popover_h = measured;
        ctx.request_repaint();
    }
    if delete {
        app.proto_edit(&[node], |p| {
            if idx < p.reactions.len() {
                p.reactions.remove(idx);
            }
        });
        app.proto.editing = None;
        return;
    }
    if r != cur {
        if r.actions.is_empty() {
            r.actions.push(Action::Back);
        }
        let new = r.clone();
        app.proto_edit(&[node], |p| {
            if let Some(slot) = p.reactions.get_mut(idx) {
                *slot = new.clone();
            }
        });
    }
    // Clicking anywhere that is not an egui popup/menu or the row that opened it closes the editor.
    let (pressed, ptr, esc) = ctx.input(|i| (i.pointer.any_pressed(), i.pointer.interact_pos(), i.key_pressed(egui::Key::Escape)));
    let over_area = widgets::pointer_over_popup(&ctx) || ptr.is_some_and(|p| area.response.rect.contains(p));
    let on_anchor = ptr.is_some_and(|p| app.proto.editing_anchor.is_some_and(|a| a.contains(p)));
    if esc || (pressed && !over_area && !on_anchor) {
        app.proto.editing = None;
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn action_params(app: &mut App, ui: &mut Ui, pane: &mut Pane, node: NodeId, frames: &[NodeId], own_top: Option<NodeId>, ai: &str, a: &mut Action) {
    let key = ai;
    match a {
        Action::Navigate { dest, preserve_scroll } => {
            if let Some(d) = dest_dropdown(ui, pane, &app.doc, &format!("nav-{key}"), "Destination", *dest, frames) {
                *dest = d;
            }
            if let Some(v) = checkbox(ui, pane, &format!("pop-preserve-{key}"), "Preserve scroll position", *preserve_scroll) {
                *preserve_scroll = v;
            }
        }
        Action::OpenOverlay { dest, position, offset, close_on_click_outside, background } => {
            let overlays: Vec<NodeId> = frames.iter().copied().filter(|f| Some(*f) != own_top).collect();
            if let Some(d) = dest_dropdown(ui, pane, &app.doc, &format!("ov-{key}"), "Overlay", *dest, &overlays) {
                *dest = d;
            }
            // Position: 3x3 grid plus a Manual toggle.
            let r = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, 3.0 * 24.0 + 8.0);
            ui.painter().text(Pos2::new(r.left(), r.top() + 12.0), Align2::LEFT_CENTER, "Position", theme::font_body(), theme::TEXT_SECONDARY);
            let grid = [
                [Some(OverlayPosition::TopLeft), Some(OverlayPosition::TopCenter), Some(OverlayPosition::TopRight)],
                [None, Some(OverlayPosition::Center), None],
                [Some(OverlayPosition::BottomLeft), Some(OverlayPosition::BottomCenter), Some(OverlayPosition::BottomRight)],
            ];
            for (ry, row) in grid.iter().enumerate() {
                for (cx, cell) in row.iter().enumerate() {
                    let c = Rect::from_min_size(Pos2::new(r.left() + LABEL_W + cx as f32 * 28.0, r.top() + ry as f32 * 28.0), Vec2::splat(24.0));
                    let Some(pos) = cell else { continue };
                    record(ui, &format!("proto-pop-pos-{}", proto::position_name(*pos)), c);
                    let resp = ui.interact(c, ui.id().with(("pop-pos", ai, ry, cx)), Sense::click());
                    let sel = *position == *pos;
                    ui.painter().rect_filled(
                        c,
                        theme::cr(theme::RADIUS_MARK),
                        if sel {
                            theme::BG_SELECTED
                        } else if resp.hovered() {
                            theme::BG_TERTIARY
                        } else {
                            theme::BG_SECONDARY
                        },
                    );
                    // a tiny overlay thumbnail inside the cell
                    let thumb = Rect::from_center_size(c.center(), Vec2::new(8.0, 8.0));
                    let off = Vec2::new((cx as f32 - 1.0) * 5.0, (ry as f32 - 1.0) * 5.0);
                    ui.painter().rect_filled(thumb.translate(off), 1.0, if sel { Color32::WHITE } else { theme::ICON_SECONDARY });
                    if resp.on_hover_text(proto::position_name(*pos)).clicked() {
                        *position = *pos;
                    }
                }
            }
            let mb =
                Rect::from_min_size(Pos2::new(r.left() + LABEL_W + 3.0 * 28.0, r.top() + 28.0), Vec2::new(r.right() - (r.left() + LABEL_W + 3.0 * 28.0), 24.0));
            record(ui, "proto-pop-pos-Manual", mb);
            let mr = ui.interact(mb, ui.id().with(("pop-manual", ai)), Sense::click());
            let msel = *position == OverlayPosition::Manual;
            ui.painter().rect_filled(
                mb,
                theme::cr(theme::RADIUS_MARK),
                if msel {
                    theme::BG_SELECTED
                } else if mr.hovered() {
                    theme::BG_TERTIARY
                } else {
                    theme::BG_SECONDARY
                },
            );
            ui.painter().text(mb.center(), Align2::CENTER_CENTER, "Manual", theme::font_caption(), if msel { Color32::WHITE } else { theme::TEXT_SECONDARY });
            if mr.clicked() {
                *position = OverlayPosition::Manual;
            }
            pane.y += 3.0 * 28.0 + 8.0;
            if *position == OverlayPosition::Manual {
                let x = Rect::from_min_size(Pos2::new(pane.x0 + theme::SECTION_CONTENT_X + LABEL_W, pane.oy + pane.y), Vec2::new(52.0, 24.0));
                let y = x.translate(Vec2::new(60.0, 0.0));
                if let Some(v) = field::num_field(ui, x, ("pop-ox", ai), Prefix::Text("X"), Some(offset.x), "") {
                    offset.x = v;
                }
                if let Some(v) = field::num_field(ui, y, ("pop-oy", ai), Prefix::Text("Y"), Some(offset.y), "") {
                    offset.y = v;
                }
                next_row(pane);
            }
            if let Some(v) = checkbox(ui, pane, &format!("pop-outside-{key}"), "Close when clicking outside", *close_on_click_outside) {
                *close_on_click_outside = v;
            }
            if let Some(v) = checkbox(ui, pane, &format!("pop-bg-{key}"), "Add background behind overlay", background.is_some()) {
                *background = if v { Some(Color::rgba(0, 0, 0, 102)) } else { None };
            }
            if let Some(bg) = *background {
                let hex = Rect::from_min_size(Pos2::new(pane.x0 + theme::SECTION_CONTENT_X + LABEL_W, pane.oy + pane.y), Vec2::new(72.0, 24.0));
                let op = Rect::from_min_size(hex.right_top() + Vec2::new(8.0, 0.0), Vec2::new(48.0, 24.0));
                widgets::swatch(
                    ui.painter(),
                    Rect::from_min_size(Pos2::new(pane.x0 + theme::SECTION_CONTENT_X, pane.oy + pane.y + 4.0), Vec2::splat(16.0)),
                    Color32::from_rgba_unmultiplied((bg.r * 255.0) as u8, (bg.g * 255.0) as u8, (bg.b * 255.0) as u8, (bg.a * 255.0) as u8),
                );
                if let Some(s) = field::text_input(ui, hex, ("pop-bg-hex", ai), &bg.to_hex()) {
                    if let Some(c) = Color::from_hex(&s) {
                        *background = Some(Color { a: bg.a, ..c });
                    }
                }
                if let Some(v) = field::num_field(ui, op, ("pop-bg-op", ai), Prefix::None, Some((bg.a * 100.0).round()), "%") {
                    *background = Some(Color { a: (v / 100.0).clamp(0.0, 1.0), ..bg });
                }
                next_row(pane);
            }
        }
        Action::SwapOverlay { dest } => {
            let overlays: Vec<NodeId> = frames.iter().copied().filter(|f| Some(*f) != own_top).collect();
            if let Some(d) = dest_dropdown(ui, pane, &app.doc, &format!("swap-{key}"), "Overlay", *dest, &overlays) {
                *dest = d;
            }
        }
        Action::ScrollTo { dest, offset } => {
            let opts = own_top.map(|t| layers_below(&app.doc, t, 200)).unwrap_or_default();
            if let Some(d) = dest_dropdown(ui, pane, &app.doc, &format!("scroll-{key}"), "Scroll to", *dest, &opts) {
                *dest = d;
            }
            let x = Rect::from_min_size(Pos2::new(pane.x0 + theme::SECTION_CONTENT_X + LABEL_W, pane.oy + pane.y), Vec2::new(52.0, 24.0));
            let y = x.translate(Vec2::new(60.0, 0.0));
            ui.painter().text(
                Pos2::new(pane.x0 + theme::SECTION_CONTENT_X, x.center().y),
                Align2::LEFT_CENTER,
                "Offset",
                theme::font_body(),
                theme::TEXT_SECONDARY,
            );
            if let Some(v) = field::num_field(ui, x, ("pop-sx", ai), Prefix::Text("X"), Some(offset.x), "") {
                offset.x = v;
            }
            if let Some(v) = field::num_field(ui, y, ("pop-sy", ai), Prefix::Text("Y"), Some(offset.y), "") {
                offset.y = v;
            }
            next_row(pane);
        }
        Action::OpenUrl { url, new_tab } => {
            let c = label_ctl(ui, pane, "URL");
            record(ui, "proto-pop-url", c);
            if let Some(s) = field::text_input(ui, c, ("pop-url", ai), url) {
                *url = s.trim().to_string();
            }
            next_row(pane);
            if let Some(v) = checkbox(ui, pane, &format!("pop-newtab-{key}"), "Open in new tab", *new_tab) {
                *new_tab = v;
            }
        }
        Action::ChangeTo { dest } => {
            let opts = variants_of(&app.doc, node);
            if let Some(d) = dest_dropdown(ui, pane, &app.doc, &format!("change-{key}"), "Change to", *dest, &opts) {
                *dest = d;
            }
        }
        _ => {}
    }
}

fn transition_ui(ui: &mut Ui, pane: &mut Pane, tr: &mut Option<Transition>) {
    let c = label_ctl(ui, pane, "Animation");
    record(ui, "proto-pop-animation", c);
    let names: Vec<String> = proto::ANIMATIONS.iter().map(|s| s.to_string()).collect();
    let ai = proto::animation_index(tr);
    if let Some(i) = dropdown(ui, c, "pop-animation", proto::ANIMATIONS[ai], &names) {
        if i != ai {
            *tr = proto::transition_of_index(i, tr);
        }
    }
    next_row(pane);
    let Some(t) = tr.as_mut() else { return };
    if let Some(d) = proto::transition_direction(t.kind) {
        let c = label_ctl(ui, pane, "Direction");
        for (i, (dir, glyph)) in [(Direction::Left, "←"), (Direction::Right, "→"), (Direction::Top, "↑"), (Direction::Bottom, "↓")].into_iter().enumerate()
        {
            let b = Rect::from_min_size(Pos2::new(c.left() + i as f32 * 28.0, c.top()), Vec2::new(26.0, 24.0));
            record(ui, &format!("proto-pop-dir-{}", proto::direction_name(dir)), b);
            let resp = ui.interact(b, ui.id().with(("pop-dir", i)), Sense::click());
            let sel = d == dir;
            ui.painter().rect_filled(
                b,
                theme::cr(theme::RADIUS_MARK),
                if sel {
                    theme::BG_SELECTED
                } else if resp.hovered() {
                    theme::BG_TERTIARY
                } else {
                    theme::BG_SECONDARY
                },
            );
            ui.painter().text(b.center(), Align2::CENTER_CENTER, glyph, theme::font_body(), if sel { Color32::WHITE } else { theme::TEXT_SECONDARY });
            if resp.on_hover_text(proto::direction_name(dir)).clicked() {
                t.kind = proto::with_direction(t.kind, dir);
            }
        }
        next_row(pane);
    }
    if t.kind == TransitionKind::SmartAnimate {
        let r = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, 28.0);
        ui.painter().text(
            Pos2::new(r.left(), r.top() + 6.0),
            Align2::LEFT_CENTER,
            "Matches layers by name and position",
            theme::font_caption(),
            theme::TEXT_TERTIARY,
        );
        pane.y += 20.0;
    }
    // Easing: preset dropdown + curve preview.
    let c = label_ctl(ui, pane, "Easing");
    record(ui, "proto-pop-easing", c);
    let mut names: Vec<String> = easing::PRESETS.iter().map(|p| p.0.to_string()).collect();
    names.push("Custom bezier".into());
    names.push("Custom spring".into());
    let cur_name = easing::preset_name(&t.easing);
    if let Some(i) = dropdown(ui, c, "pop-easing", &cur_name, &names) {
        t.easing = match i {
            i if i < easing::PRESETS.len() => easing::PRESETS[i].1,
            i if i == easing::PRESETS.len() => Easing::Bezier([0.4, 0.0, 0.2, 1.0]),
            _ => Easing::Spring { mass: 1.0, stiffness: 200.0, damping: 18.0 },
        };
    }
    next_row(pane);
    let prev = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, 64.0);
    record(ui, "proto-pop-curve", prev);
    curve_preview(ui, prev, &t.easing);
    pane.y += 72.0;
    match &mut t.easing {
        Easing::Bezier(c4) => {
            for row in 0..2 {
                let base = pane.rect(theme::SECTION_CONTENT_X, 0.0, 88.0, 24.0);
                for col in 0..2 {
                    let r = base.translate(Vec2::new(col as f32 * 96.0, 0.0));
                    let k = row * 2 + col;
                    let label = ["x1", "y1", "x2", "y2"][k];
                    if let Some(v) = field::num_field(ui, r, ("pop-bz", k), Prefix::Text(label), Some(c4[k]), "") {
                        c4[k] = if k % 2 == 0 { v.clamp(0.0, 1.0) } else { v.clamp(-2.0, 3.0) };
                    }
                }
                next_row(pane);
            }
        }
        Easing::Spring { mass, stiffness, damping } => {
            let items: [(&str, &mut f32); 3] = [("Mass", mass), ("Stiffness", stiffness), ("Damping", damping)];
            for (i, (label, v)) in items.into_iter().enumerate() {
                let c = label_ctl(ui, pane, label);
                if let Some(nv) = field::num_field(ui, c, ("pop-spring", i), Prefix::None, Some(*v), "") {
                    *v = nv.max(0.1);
                }
                next_row(pane);
            }
        }
        _ => {}
    }
    let c = label_ctl(ui, pane, "Duration");
    record(ui, "proto-pop-duration", c);
    if matches!(t.easing, Easing::Spring { .. }) {
        let ms = easing::effective_duration_ms(&t.easing, t.duration_ms);
        ui.painter().text(
            Pos2::new(c.left() + 8.0, c.center().y),
            Align2::LEFT_CENTER,
            format!("{ms} ms (from spring)"),
            theme::font_body(),
            theme::TEXT_TERTIARY,
        );
    } else if let Some(v) = field::num_field(ui, c, "pop-duration", Prefix::None, Some(t.duration_ms as f32), " ms") {
        t.duration_ms = v.clamp(0.0, 60_000.0) as u32;
    }
    next_row(pane);
}

/// The easing curve drawn over a 1x1 plot (time right, progress up).
pub fn curve_preview(ui: &Ui, rect: Rect, e: &Easing) {
    let p = ui.painter();
    p.rect_filled(rect, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
    let inner = rect.shrink2(Vec2::new(12.0, 14.0));
    p.line_segment([inner.left_bottom(), inner.right_top()], Stroke::new(1.0, theme::BORDER));
    let pts = curve_points(e, inner, 48);
    p.add(egui::Shape::line(pts, Stroke::new(2.0, theme::PROTO_BLUE)));
    for q in [inner.left_bottom(), inner.right_top()] {
        p.circle_filled(q, 2.5, theme::TEXT_SECONDARY);
    }
}

/// Polyline of `ease(e, t)` mapped into `r` (y grows upward from the bottom edge).
pub fn curve_points(e: &Easing, r: Rect, n: usize) -> Vec<Pos2> {
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            Pos2::new(r.left() + t * r.width(), r.bottom() - easing::ease(e, t) * r.height())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curve_points_span_the_plot() {
        let r = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(100.0, 50.0));
        let pts = curve_points(&Easing::Linear, r, 10);
        assert_eq!(pts.len(), 11);
        assert_eq!(pts[0], r.left_bottom());
        assert_eq!(pts[10], r.right_top());
        let mid = curve_points(&Easing::EaseIn, r, 10)[5];
        assert!(mid.y > r.center().y, "ease in is below the diagonal at the middle");
    }

    #[test]
    fn ui3_prototype_assets_and_right_header_layout() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let f = d.alloc_id();
        let f = d.insert(page, Node::frame(f, designos_doc::Rect::new(0.0, 0.0, 200.0, 100.0))).unwrap();
        let mut app = App::new(d);
        app.selection = vec![f];
        app.ui.right_tab = Some(crate::app::RightTab::Prototype);
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))), ..Default::default() };
        let _ = ctx.run_ui(raw, |ui| {
            crate::panels::assets::show(&mut app, ui, Rect::from_min_size(Pos2::new(40.0, 0.0), Vec2::new(240.0, 700.0)));
            crate::panels::right::show(&mut app, ui, Rect::from_min_size(Pos2::new(960.0, 0.0), Vec2::new(240.0, 700.0)));
        });
        let get_rect = |name: &str| ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", name.to_string()))));
        assert!(get_rect("assets-libraries").is_some());
        assert!(get_rect("assets-search").is_some());
        assert!(get_rect("assets-created-in-file").is_some());
        assert!(get_rect("assets-add-more-libraries").is_some());
        assert!(get_rect("proto-show-settings").is_some());
        assert!(get_rect("proto-guide-connection").is_some());
        assert!(get_rect("proto-guide-running").is_some());
        assert!(get_rect("help-button").is_some());
        let pb = get_rect("present-button").unwrap();
        let sb = get_rect("share-button").unwrap();
        assert!(pb.left() < sb.left(), "Present button should be to the left of Share button in Figma UI3");
    }
}
