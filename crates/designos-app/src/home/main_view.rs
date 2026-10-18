//! Main area of the home screen: header, sort / view controls and the file
//! grid or list, plus the Trash view and empty states.

use super::time::{edited, relative, wall_ms};
use super::{Act, DropTo, Home, Section, Target, ViewMode};
use crate::icons::{self, Icon};
use crate::theme;
use crate::widgets;
use designos_library::{FileEntry, Library};
use egui::{Align2, Color32, Id, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Primary,
    Secondary,
    Danger,
}

pub fn show(lib: &Library, h: &mut Home, ui: &mut Ui, rect: Rect, acts: &mut Vec<Act>) {
    let p = ui.painter().clone();
    let pad = theme::HOME_PAD;
    let trash = h.section == Section::Trash && !h.searching();

    // ---- header: title / breadcrumb on the left, buttons on the right
    let head = Rect::from_min_size(rect.min, Vec2::new(rect.width(), theme::HOME_HEADER_H));
    breadcrumb(h, ui, &p, Pos2::new(rect.left() + pad, head.center().y), acts);
    let mut right = rect.right() - pad;
    if trash {
        if !h.snap.trash.is_empty() {
            let r = button(ui, &p, &mut right, head.center().y, "Empty trash", Kind::Danger, "home-empty-trash");
            if r.clicked() {
                acts.push(Act::EmptyTrash);
            }
        }
    } else {
        let r = button(ui, &p, &mut right, head.center().y, "New design file", Kind::Primary, "home-new-file");
        if r.clicked() {
            acts.push(Act::NewFile);
        }
        right -= 8.0;
        let r = button(ui, &p, &mut right, head.center().y, "New board", Kind::Secondary, "home-new-board");
        if r.clicked() {
            acts.push(Act::NewBoard);
        }
        right -= 8.0;
        let r = button(ui, &p, &mut right, head.center().y, "Import\u{2026}", Kind::Secondary, "home-import");
        if r.clicked() {
            acts.push(Act::Import);
        }
    }
    let mut top = head.bottom();

    // ---- toolbar: sort and view toggle
    if !trash {
        let y = top + 4.0;
        let label = h.sort.label();
        let w = widgets::text_width(&p, label, &theme::font_body()) + 40.0;
        let sr = Rect::from_min_size(Pos2::new(rect.left() + pad - 8.0, y), Vec2::new(w, 28.0));
        let resp = ui.interact(sr, Id::new("home-sort"), Sense::click());
        widgets::button_bg(&p, sr, &resp, h.sort_menu.is_some(), theme::cr(theme::RADIUS_CONTROL));
        p.text(Pos2::new(sr.left() + 8.0, sr.center().y), Align2::LEFT_CENTER, label, theme::font_body(), theme::TEXT);
        icons::paint(&p, Icon::ChevronDown, Pos2::new(sr.right() - 14.0, sr.center().y), theme::CHEVRON_SMALL, theme::ICON_SECONDARY);
        widgets::record(ui, "home-sort", sr);
        if resp.clicked() {
            h.sort_menu = if h.sort_menu.is_some() { None } else { Some(sr) };
        }
        let mut x = rect.right() - pad;
        for (mode, icon, key, tip) in
            [(ViewMode::List, Icon::ListView, "home-view-list", "List view"), (ViewMode::Grid, Icon::GridView, "home-view-grid", "Grid view")]
        {
            let br = Rect::from_min_size(Pos2::new(x - 28.0, y), Vec2::splat(28.0));
            x -= 28.0 + 2.0;
            if widgets::icon_button(ui, br, key, icon, h.view == mode, tip).clicked() {
                acts.push(Act::SetView(mode));
            }
            widgets::record(ui, key, br);
        }
        top += 36.0;
    }
    widgets::hline(&p, rect.left(), rect.right(), top, theme::BORDER);
    top += 1.0;

    let content = Rect::from_min_max(Pos2::new(rect.left(), top), rect.max);
    if trash {
        trash_view(h, ui, content, acts);
    } else {
        files_view(lib, h, ui, content, acts);
    }
}

/// Title, or clickable "Project / Folder / ..." path inside a project.
fn breadcrumb(h: &Home, ui: &mut Ui, p: &egui::Painter, at: Pos2, acts: &mut Vec<Act>) {
    let font = theme::font_title();
    let mut segs: Vec<(String, Section)> = vec![];
    match &h.section {
        Section::Project(pid, folder) if !h.searching() => {
            segs.push((h.snap.project_name(pid).to_string(), Section::Project(pid.clone(), None)));
            if let Some(pr) = h.snap.projects.iter().find(|x| &x.id == pid) {
                let mut chain = vec![];
                let mut cur = folder.clone();
                while let Some(fid) = cur {
                    let Some(f) = pr.folders.iter().find(|x| x.id == fid) else { break };
                    chain.push((f.name.clone(), Section::Project(pid.clone(), Some(f.id.clone()))));
                    cur = f.parent.clone();
                }
                chain.reverse();
                segs.extend(chain);
            }
        }
        _ => segs.push((h.title(), h.section.clone())),
    }
    let mut x = at.x;
    let last = segs.len() - 1;
    for (i, (label, section)) in segs.into_iter().enumerate() {
        let w = widgets::text_width(p, &label, &font);
        let r = Rect::from_min_size(Pos2::new(x, at.y - 14.0), Vec2::new(w, 28.0));
        let color = if i == last { theme::TEXT } else { theme::TEXT_SECONDARY };
        if i < last {
            let resp = ui.interact(r, Id::new(("home-crumb", i)), Sense::click());
            if resp.hovered() {
                p.rect_filled(r.expand2(Vec2::new(4.0, 0.0)), theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
            }
            if resp.clicked() {
                acts.push(Act::Go(section));
            }
        }
        p.text(Pos2::new(x, at.y), Align2::LEFT_CENTER, &label, font.clone(), color);
        x += w;
        if i < last {
            p.text(Pos2::new(x + 8.0, at.y), Align2::LEFT_CENTER, "/", font.clone(), theme::TEXT_TERTIARY);
            x += 24.0;
        }
    }
}

/// Right-aligned button; moves `right` to its left edge.
fn button(ui: &mut Ui, p: &egui::Painter, right: &mut f32, cy: f32, label: &str, kind: Kind, record: &str) -> Response {
    let font = theme::font_body_strong();
    let w = widgets::text_width(p, label, &font) + 28.0;
    let r = Rect::from_min_size(Pos2::new(*right - w, cy - 16.0), Vec2::new(w, 32.0));
    *right = r.left();
    let resp = ui.interact(r, Id::new(("home-button", record)), Sense::click());
    let radius = theme::cr(theme::RADIUS_CONTROL);
    let down = resp.is_pointer_button_down_on();
    match kind {
        Kind::Primary => {
            let bg = if down || resp.hovered() { theme::BG_BRAND_HOVER } else { theme::BG_BRAND };
            p.rect_filled(r, radius, bg);
            p.text(r.center(), Align2::CENTER_CENTER, label, font, Color32::WHITE);
        }
        Kind::Secondary => {
            let bg = if down {
                theme::BG_PRESSED
            } else if resp.hovered() {
                theme::BG_HOVER
            } else {
                Color32::TRANSPARENT
            };
            p.rect_filled(r, radius, bg);
            p.rect_stroke(r, radius, Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);
            p.text(r.center(), Align2::CENTER_CENTER, label, font, theme::TEXT);
        }
        Kind::Danger => {
            let bg = if resp.hovered() { theme::DANGER.gamma_multiply(0.2) } else { Color32::TRANSPARENT };
            p.rect_filled(r, radius, bg);
            p.rect_stroke(r, radius, Stroke::new(1.0, theme::DANGER), StrokeKind::Inside);
            p.text(r.center(), Align2::CENTER_CENTER, label, font, theme::DANGER);
        }
    }
    widgets::record(ui, record, r);
    resp
}

// ---------------------------------------------------------------- files

fn empty_state(h: &Home, ui: &Ui, area: Rect) {
    let (title, body) = if h.searching() {
        (format!("No files match \u{201c}{}\u{201d}", h.search.trim()), "Try a different name.".to_string())
    } else {
        match &h.section {
            Section::Recents => ("No files yet".into(), "Create a design file, or import a .fig, .svg or .designos file to get started.".into()),
            Section::Starred => ("Nothing starred".into(), "Right-click a file and choose Star to keep it handy here.".into()),
            Section::Project(..) => ("This project is empty".into(), "Create a design file here, or drag files in from other projects.".into()),
            Section::Trash => ("Trash is empty".into(), "Files and projects you delete wait here until you empty the trash.".into()),
        }
    };
    let p = ui.painter();
    let c = area.center();
    icons::paint(p, Icon::File, Pos2::new(c.x, c.y - 44.0), 32.0, theme::ICON_TERTIARY);
    p.text(Pos2::new(c.x, c.y), Align2::CENTER_CENTER, title, theme::font_large_strong(), theme::TEXT);
    let galley = p.layout(body, theme::font_body(), theme::TEXT_SECONDARY, 360.0_f32.min(area.width() - 48.0));
    let pos = Pos2::new(c.x - galley.size().x / 2.0, c.y + 18.0);
    p.galley(pos, galley, theme::TEXT_SECONDARY);
}

fn files_view(lib: &Library, h: &mut Home, ui: &mut Ui, content: Rect, acts: &mut Vec<Act>) {
    let files = h.files();
    let folders = h.folders();
    if files.is_empty() && folders.is_empty() {
        empty_state(h, ui, content);
        return;
    }
    let now = wall_ms();
    let pad = theme::HOME_PAD;
    let list = h.view == ViewMode::List;
    let inner_w = content.width() - pad * 2.0;
    let (cw, ch, gap_x, gap_y) = (theme::HOME_CARD_W, theme::HOME_CARD_H, theme::HOME_CARD_GAP, 24.0);
    let cols = (((inner_w + gap_x) / (cw + gap_x)).floor() as usize).max(1);
    let folder_rows = if list { folders.len() as f32 * theme::HOME_LIST_ROW_H } else { folders.len().div_ceil(cols) as f32 * (theme::HOME_FOLDER_H + 12.0) };
    let file_rows = if list { files.len() as f32 * theme::HOME_LIST_ROW_H + 28.0 } else { files.len().div_ceil(cols) as f32 * (ch + gap_y) };
    let content_h = pad + folder_rows + if folder_rows > 0.0 { 12.0 } else { 0.0 } + file_rows + pad;
    crate::panels::wheel_scroll(ui, content, &mut h.scroll, content_h);
    let scroll = h.scroll;
    let ptr = ui.input(|i| i.pointer.hover_pos());
    let dragging = h.drag.is_some();
    let mut budget = 4u32;
    let selected = h.selected.clone();
    let mut new_selected = None;
    let mut menu = None;
    let mut drag = None;
    let mut zones: Vec<(Rect, DropTo)> = vec![];
    let section = h.section.clone();
    let ctx = ui.ctx().clone();
    let project_id = if let Section::Project(p, _) = &section { Some(p.clone()) } else { None };
    crate::panels::clipped(ui, content, |ui| {
        let p = ui.painter().clone();
        let mut y = content.top() + pad - scroll;
        let x0 = content.left() + pad;
        // ---- folder tiles
        for (i, (fid, name)) in folders.iter().enumerate() {
            let r = if list {
                Rect::from_min_size(Pos2::new(x0, y + i as f32 * theme::HOME_LIST_ROW_H), Vec2::new(inner_w, theme::HOME_LIST_ROW_H))
            } else {
                Rect::from_min_size(
                    Pos2::new(x0 + (i % cols) as f32 * (cw + gap_x), y + (i / cols) as f32 * (theme::HOME_FOLDER_H + 12.0)),
                    Vec2::new(cw, theme::HOME_FOLDER_H),
                )
            };
            if r.bottom() < content.top() || r.top() > content.bottom() {
                continue;
            }
            let Some(pid) = project_id.clone() else { continue };
            let resp = ui.interact(r, Id::new(("home-folder", fid)), Sense::click());
            let over = dragging && ptr.is_some_and(|q| r.contains(q));
            let fill = if over {
                theme::BG_SELECTED_HOVER
            } else if resp.hovered() {
                theme::BG_HOVER
            } else {
                theme::BG_SECONDARY
            };
            p.rect_filled(
                r,
                theme::cr(theme::RADIUS_CONTROL),
                if list {
                    if over || resp.hovered() {
                        fill
                    } else {
                        Color32::TRANSPARENT
                    }
                } else {
                    fill
                },
            );
            icons::paint(&p, Icon::Folder, Pos2::new(r.left() + 24.0, r.center().y), 20.0, theme::ICON_SECONDARY);
            p.text(
                Pos2::new(r.left() + 44.0, r.center().y),
                Align2::LEFT_CENTER,
                widgets::elide(&p, name, &theme::font_body_strong(), r.width() - 60.0),
                theme::font_body_strong(),
                theme::TEXT,
            );
            widgets::record(ui, &format!("home-folder-{name}"), r);
            if resp.double_clicked() || resp.clicked() {
                acts.push(Act::Go(Section::Project(pid.clone(), Some(fid.clone()))));
            }
            if resp.secondary_clicked() {
                menu = ui.input(|i| i.pointer.interact_pos()).map(|at| (Target::Folder(pid.clone(), fid.clone()), at));
            }
            zones.push((r.intersect(content), DropTo::Folder(pid, fid.clone())));
        }
        y += folder_rows + if folder_rows > 0.0 { 12.0 } else { 0.0 };

        if list {
            // column header
            let hr = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(inner_w, 28.0));
            let cols_x = list_columns(hr.right());
            for (x, label) in [(hr.left() + 60.0, "Name"), (cols_x.0, "Project"), (cols_x.1, "Edited"), (cols_x.2, "Opened")] {
                p.text(Pos2::new(x, hr.center().y), Align2::LEFT_CENTER, label, theme::font_caption_medium(), theme::TEXT_TERTIARY);
            }
            y += 28.0;
        }
        for (i, f) in files.iter().enumerate() {
            let r = if list {
                Rect::from_min_size(Pos2::new(x0, y + i as f32 * theme::HOME_LIST_ROW_H), Vec2::new(inner_w, theme::HOME_LIST_ROW_H))
            } else {
                Rect::from_min_size(Pos2::new(x0 + (i % cols) as f32 * (cw + gap_x), y + (i / cols) as f32 * (ch + gap_y)), Vec2::new(cw, ch))
            };
            if r.bottom() < content.top() || r.top() > content.bottom() {
                continue;
            }
            let is_sel = selected.as_deref() == Some(f.id.as_str());
            let resp = ui.interact(r, Id::new(("home-card", &f.id)), Sense::click_and_drag());
            if resp.double_clicked() {
                acts.push(Act::Open(f.id.clone()));
            } else if resp.clicked() {
                new_selected = Some(f.id.clone());
            }
            if resp.secondary_clicked() {
                new_selected = Some(f.id.clone());
                menu = ui.input(|i| i.pointer.interact_pos()).map(|at| (Target::File(f.id.clone()), at));
            }
            if resp.drag_started() {
                drag = Some((f.id.clone(), f.name.clone()));
            }
            let tex = h.thumbs.get(&ctx, lib, f, &mut budget);
            let board = h.kinds.get(lib, f, &mut budget) == designos_doc::DocumentKind::Board;
            let project = h.snap.project_name(&f.project).to_string();
            let star_r = if list {
                Rect::from_min_size(Pos2::new(r.right() - 40.0, r.center().y - 12.0), Vec2::splat(24.0))
            } else {
                Rect::from_min_size(Pos2::new(r.left() + cw - 32.0, r.top() + 8.0), Vec2::splat(24.0))
            };
            let star = ui.interact(star_r, Id::new(("home-star", &f.id)), Sense::click());
            if star.clicked() {
                acts.push(Act::Star(f.id.clone(), !f.starred));
            }
            if list {
                list_row(&p, r, f, &project, tex, board, resp.hovered(), is_sel, now);
            } else {
                card(&p, r, f, &project, tex, board, resp.hovered(), is_sel, now);
            }
            widgets::record(ui, &format!("home-kind-{}", if board { "board" } else { "design" }), r);
            if resp.hovered() || star.hovered() || f.starred {
                if star.hovered() || (!list && resp.hovered()) {
                    p.circle_filled(star_r.center(), 12.0, Color32::from_black_alpha(if f.starred && !star.hovered() { 0 } else { 120 }));
                }
                paint_star(&p, star_r.center(), f.starred, star.hovered());
            }
            widgets::record(ui, &format!("home-card-{}", f.name), r);
            if dragging && selected.as_deref() == Some(f.id.as_str()) {
                p.rect_filled(r, theme::cr(theme::RADIUS_FRAME), Color32::from_black_alpha(90));
            }
        }
    });
    crate::panels::scrollbar(ui, content, scroll, content_h);
    h.drop_rects.extend(zones);
    if let Some(s) = new_selected {
        h.selected = Some(s);
    }
    if menu.is_some() {
        h.menu = menu;
    }
    if drag.is_some() {
        h.drag = drag;
    }
}

fn paint_star(p: &egui::Painter, c: Pos2, on: bool, hot: bool) {
    let col = if on {
        Color32::from_rgb(0xFF, 0xCD, 0x29)
    } else if hot {
        theme::ICON
    } else {
        theme::ICON_SECONDARY
    };
    if on {
        // filled star: draw the outline icon in a thicker pass
        icons::paint(p, Icon::Star, c, 14.0, col);
        icons::paint(p, Icon::Star, c, 11.0, col);
        icons::paint(p, Icon::Star, c, 8.0, col);
    } else {
        icons::paint(p, Icon::Star, c, 14.0, col);
    }
}

type Tex = Option<(egui::TextureId, Vec2)>;

/// Thumbnail well; the picture is fitted inside it, keeping its aspect ratio. A board's picture
/// is its own canvas and fills the well; a board without a picture shows the dotted canvas and a
/// sticky note icon.
fn thumb_area(p: &egui::Painter, area: Rect, tex: Tex, board: bool, radius: u8) {
    if board {
        p.rect_filled(area, theme::cr(radius), theme::rgb(theme::BOARD_BG));
        match tex {
            Some((t, _)) => {
                // the picture is its own canvas: it fills the well, with the well's rounded corners
                let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
                p.add(egui::epaint::RectShape::filled(area, theme::cr(radius), Color32::WHITE).with_texture(t, uv));
            }
            None => {
                let mut mesh = egui::Mesh::default();
                let mut y = area.top() + 8.0;
                while y < area.bottom() {
                    let mut x = area.left() + 8.0;
                    while x < area.right() {
                        mesh.add_colored_rect(Rect::from_center_size(Pos2::new(x, y), Vec2::splat(1.5)), theme::BOARD_DOT);
                        x += 16.0;
                    }
                    y += 16.0;
                }
                p.add(egui::Shape::mesh(mesh));
                icons::paint(p, Icon::Sticky, area.center(), 24.0, Color32::from_black_alpha(110));
            }
        }
        return;
    }
    p.rect_filled(area, theme::cr(radius), theme::HOME_THUMB_BG);
    match tex {
        Some((t, size)) => {
            let inner = area.shrink(if area.width() > 80.0 { 12.0 } else { 3.0 });
            let k = (inner.width() / size.x.max(1.0)).min(inner.height() / size.y.max(1.0)).min(2.0);
            let fit = Rect::from_center_size(inner.center(), size * k);
            p.image(t, fit, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
        None => icons::paint(p, Icon::File, area.center(), 24.0, theme::ICON_TERTIARY),
    }
}

/// The "Board" badge on a board's card: a sticky note icon and the word, bottom left of the picture.
fn board_badge(p: &egui::Painter, area: Rect) {
    let font = theme::font_caption_strong();
    let w = widgets::text_width(p, "Board", &font) + 28.0;
    let r = Rect::from_min_size(Pos2::new(area.left() + 8.0, area.bottom() - 8.0 - 20.0), Vec2::new(w, 20.0));
    p.rect_filled(r, theme::cr(10), theme::BG_MENU);
    icons::paint(p, Icon::Sticky, Pos2::new(r.left() + 10.0, r.center().y), 12.0, theme::ICON);
    p.text(Pos2::new(r.left() + 19.0, r.center().y), Align2::LEFT_CENTER, "Board", font, theme::TEXT);
}

/// A grid card: thumbnail, name, project and edit time.
#[allow(clippy::too_many_arguments)]
fn card(p: &egui::Painter, r: Rect, f: &FileEntry, project: &str, tex: Tex, board: bool, hover: bool, selected: bool, now: u64) {
    let thumb = Rect::from_min_size(r.min, Vec2::new(r.width(), theme::HOME_THUMB_H));
    thumb_area(p, thumb, tex, board, theme::RADIUS_FRAME);
    if board {
        board_badge(p, thumb);
    }
    let stroke = if selected {
        Stroke::new(2.0, theme::BORDER_SELECTED)
    } else if hover {
        Stroke::new(1.0, theme::TEXT_TERTIARY)
    } else {
        Stroke::new(1.0, theme::BORDER)
    };
    p.rect_stroke(thumb, theme::cr(theme::RADIUS_FRAME), stroke, StrokeKind::Inside);
    let tx = r.left() + 4.0;
    let w = r.width() - 8.0;
    let mut y = thumb.bottom() + 12.0;
    p.text(Pos2::new(tx, y), Align2::LEFT_CENTER, widgets::elide(p, &f.name, &theme::font_large_strong(), w), theme::font_large_strong(), theme::TEXT);
    y += 18.0;
    p.text(Pos2::new(tx, y), Align2::LEFT_CENTER, widgets::elide(p, project, &theme::font_body(), w), theme::font_body(), theme::TEXT_SECONDARY);
    y += 16.0;
    p.text(Pos2::new(tx, y), Align2::LEFT_CENTER, edited(now, f.modified), theme::font_body(), theme::TEXT_TERTIARY);
}

/// Column x positions of the list view: (project, edited, opened).
fn list_columns(right: f32) -> (f32, f32, f32) {
    (right - 520.0, right - 340.0, right - 180.0)
}

#[allow(clippy::too_many_arguments)]
fn list_row(p: &egui::Painter, r: Rect, f: &FileEntry, project: &str, tex: Tex, board: bool, hover: bool, selected: bool, now: u64) {
    if selected {
        p.rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_SELECTED);
    } else if hover {
        p.rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
    }
    let th = Rect::from_min_size(Pos2::new(r.left() + 8.0, r.top() + 5.0), Vec2::new(48.0, r.height() - 10.0));
    thumb_area(p, th, tex, board, theme::RADIUS_MARK);
    let cols = list_columns(r.right());
    let font = theme::font_body();
    p.text(
        Pos2::new(r.left() + 68.0, r.center().y),
        Align2::LEFT_CENTER,
        widgets::elide(p, &f.name, &theme::font_body_strong(), cols.0 - r.left() - 80.0 - if board { 20.0 } else { 0.0 }),
        theme::font_body_strong(),
        theme::TEXT,
    );
    if board {
        // the kind next to the name in the list
        let name_w = widgets::text_width(p, &f.name, &theme::font_body_strong()).min(cols.0 - r.left() - 100.0);
        icons::paint(p, Icon::Sticky, Pos2::new(r.left() + 68.0 + name_w + 14.0, r.center().y), 14.0, theme::ICON_SECONDARY);
    }
    p.text(Pos2::new(cols.0, r.center().y), Align2::LEFT_CENTER, widgets::elide(p, project, &font, 150.0), font.clone(), theme::TEXT_SECONDARY);
    p.text(Pos2::new(cols.1, r.center().y), Align2::LEFT_CENTER, relative(now, f.modified), font.clone(), theme::TEXT_SECONDARY);
    p.text(Pos2::new(cols.2, r.center().y), Align2::LEFT_CENTER, relative(now, f.opened), font, theme::TEXT_SECONDARY);
}

// ---------------------------------------------------------------- trash

fn trash_view(h: &mut Home, ui: &mut Ui, content: Rect, acts: &mut Vec<Act>) {
    if h.snap.trash.is_empty() {
        empty_state(h, ui, content);
        return;
    }
    let now = wall_ms();
    let pad = theme::HOME_PAD;
    let row_h = 52.0;
    let note_h = 36.0;
    let content_h = pad + note_h + h.snap.trash.len() as f32 * row_h + pad;
    crate::panels::wheel_scroll(ui, content, &mut h.scroll, content_h);
    let scroll = h.scroll;
    let items = h.snap.trash.clone();
    crate::panels::clipped(ui, content, |ui| {
        let p = ui.painter().clone();
        let x0 = content.left() + pad;
        let w = content.width() - pad * 2.0;
        let mut y = content.top() + pad - scroll;
        p.text(
            Pos2::new(x0, y + 10.0),
            Align2::LEFT_CENTER,
            "Items stay here until you empty the trash. Emptying it deletes them permanently.",
            theme::font_body(),
            theme::TEXT_SECONDARY,
        );
        y += note_h;
        for it in items.iter() {
            let r = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(w, row_h));
            y += row_h;
            if r.bottom() < content.top() || r.top() > content.bottom() {
                continue;
            }
            let resp = ui.interact(r, Id::new(("home-trash-row", &it.id)), Sense::hover());
            if resp.hovered() {
                p.rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
            }
            let is_file = it.kind == designos_library::TrashKind::File;
            icons::paint(&p, if is_file { Icon::File } else { Icon::Frame }, Pos2::new(r.left() + 24.0, r.center().y), 20.0, theme::ICON_SECONDARY);
            p.text(Pos2::new(r.left() + 48.0, r.center().y - 9.0), Align2::LEFT_CENTER, &it.name, theme::font_body_strong(), theme::TEXT);
            let from = match (&it.project_name, is_file) {
                (Some(pn), true) => format!("Deleted {} \u{b7} from {pn}", relative(now, it.trashed)),
                (_, false) => format!("Project \u{b7} deleted {}", relative(now, it.trashed)),
                _ => format!("Deleted {}", relative(now, it.trashed)),
            };
            p.text(Pos2::new(r.left() + 48.0, r.center().y + 9.0), Align2::LEFT_CENTER, from, theme::font_body(), theme::TEXT_SECONDARY);
            let mut right = r.right() - 8.0;
            let b = button(ui, &p, &mut right, r.center().y, "Restore", Kind::Secondary, &format!("home-restore-{}", it.name));
            if b.clicked() {
                acts.push(Act::Restore(it.id.clone()));
            }
        }
    });
    crate::panels::scrollbar(ui, content, scroll, content_h);
}
