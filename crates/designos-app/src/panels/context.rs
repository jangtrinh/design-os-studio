//! Right-click context menu for the selection (canvas and layers).

use super::menus::{self, Action};
use crate::app::App;
use crate::widgets::menu::{popup_tree, MenuItem, TreeResult};
use designos_doc::find::MatchCriteria;
use designos_doc::NodeKind;
use egui::{Context, Id};

/// A menu entry: leaves run `act`; entries with `kids` open a submenu.
struct Entry {
    item: MenuItem,
    act: Option<Action>,
    kids: Vec<(MenuItem, Action)>,
}

fn leaf(item: MenuItem, act: Action) -> Entry {
    Entry { item, act: Some(act), kids: vec![] }
}

/// The "Select" submenu: structural selection and the "same ..." family.
fn select_submenu(has: bool) -> Entry {
    let same = |label: &str, c: MatchCriteria| (MenuItem::new(label).disabled(!has), Action::SelectSame(c));
    let mut kids = vec![
        (MenuItem::new("Select children").shortcut("Enter").disabled(!has), Action::SelectChildren),
        (MenuItem::new("Select parent").shortcut("Shift+Enter").disabled(!has), Action::SelectParent),
        (MenuItem::new("Select inverse").shortcut("Ctrl+Shift+A"), Action::SelectInverse),
        (MenuItem::new("Select matching layers").shortcut("Ctrl+Alt+A").disabled(!has).sep(), Action::SelectMatching),
    ];
    kids.extend([
        same("Select all with same fill", MatchCriteria::Fill),
        same("Select all with same stroke", MatchCriteria::Stroke),
        same("Select all with same effect", MatchCriteria::Effect),
        same("Select all with same font", MatchCriteria::Font),
        same("Select all with same instance", MatchCriteria::Instance),
    ]);
    Entry { item: MenuItem::new("Select").disabled(!has).sep(), act: None, kids }
}

/// The "Copy/Paste" submenu: property copy/paste and positional paste variants.
fn copy_paste_submenu(has: bool, has_clip: bool) -> Entry {
    let kids = vec![
        (MenuItem::new("Copy properties").shortcut("Ctrl+Alt+C").disabled(!has), Action::CopyProperties),
        (MenuItem::new("Paste properties").shortcut("Ctrl+Alt+V").disabled(!has).sep(), Action::PasteProperties),
        (MenuItem::new("Paste over selection").shortcut("Ctrl+Shift+V").disabled(!has_clip), Action::PasteOverSelection),
        (MenuItem::new("Paste to replace").shortcut("Ctrl+Alt+Shift+R").disabled(!has || !has_clip), Action::PasteToReplace),
    ];
    Entry { item: MenuItem::new("Copy/Paste").sep(), act: None, kids }
}

/// Entries given the current selection.
fn entries(app: &App) -> Vec<Entry> {
    let has = !app.selection.is_empty();
    let has_clip = app.clip.is_some();
    let insts = app.selected_instances();
    let has_inst = !insts.is_empty();
    let any_container =
        app.selection.iter().any(|id| app.doc.get(*id).is_some_and(|n| matches!(n.kind, NodeKind::Group | NodeKind::Frame(_) | NodeKind::Section)));
    let is_grid_table = app.selection.iter().any(|id| {
        app.doc.get(*id).is_some_and(|n| {
            let self_grid = n.kind.frame_props().and_then(|fp| fp.auto_layout.as_ref()).is_some_and(|al| al.direction == designos_doc::LayoutDirection::Grid);
            let parent_grid = n
                .parent
                .and_then(|p| app.doc.get(p))
                .and_then(|pn| pn.kind.frame_props())
                .and_then(|fp| fp.auto_layout.as_ref())
                .is_some_and(|al| al.direction == designos_doc::LayoutDirection::Grid);
            self_grid || parent_grid || n.name.to_ascii_lowercase().contains("table")
        })
    });
    let can_create = app
        .selection
        .iter()
        .any(|id| app.doc.get(*id).is_some_and(|n| !matches!(n.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_) | NodeKind::Page { .. })));
    let can_mark = app.selection.iter().any(|id| crate::dev::markable(app, *id));
    let has_status = app.selection.iter().any(|id| app.doc.get(*id).is_some_and(|n| n.dev.status.is_some()));
    let mut list = vec![
        leaf(MenuItem::new("Duplicate").shortcut("Ctrl+D").disabled(!has), Action::Duplicate),
        leaf(MenuItem::new("Group selection").shortcut("Ctrl+G").disabled(!has), Action::Group),
        leaf(MenuItem::new("Frame selection").shortcut("Ctrl+Alt+G").disabled(!has), Action::FrameSelection),
        leaf(MenuItem::new("Ungroup").shortcut("Ctrl+Shift+G").disabled(!any_container), Action::Ungroup),
        leaf(MenuItem::new("Add auto layout").shortcut("Shift+A").disabled(!has).sep(), Action::AutoLayout),
        leaf(MenuItem::new("Create component").shortcut("Ctrl+Alt+K").disabled(!can_create), Action::CreateComponent),
        leaf(MenuItem::new("Combine as variants").shortcut("Ctrl+Shift+K").disabled(!has), Action::CombineVariants),
        leaf(MenuItem::new("Detach instance").shortcut("Ctrl+Alt+B").disabled(!has_inst), Action::Detach),
        leaf(MenuItem::new("Go to main component").disabled(!has_inst), Action::GoToMain),
        leaf(MenuItem::new("Reset all changes").disabled(!has_inst).sep(), Action::ResetInstance),
        leaf(MenuItem::new("Bring to front").shortcut("Ctrl+Shift+]").disabled(!has), Action::BringToFront),
        leaf(MenuItem::new("Send to back").shortcut("Ctrl+Shift+[").disabled(!has).sep(), Action::SendToBack),
        select_submenu(has),
        copy_paste_submenu(has, has_clip),
        leaf(MenuItem::new("Mark as ready for dev").disabled(!can_mark), Action::MarkReadyForDev),
    ];
    if is_grid_table {
        list.push(leaf(MenuItem::new("Insert row"), Action::InsertTableRow));
        list.push(leaf(MenuItem::new("Delete row"), Action::DeleteTableRow));
        list.push(leaf(MenuItem::new("Insert column"), Action::InsertTableColumn));
        list.push(leaf(MenuItem::new("Delete column").sep(), Action::DeleteTableColumn));
    }
    if has_status {
        list.push(leaf(MenuItem::new("Mark as completed"), Action::MarkCompleted));
        list.push(leaf(MenuItem::new("Remove dev status"), Action::ClearDevStatus));
    }
    let can_convert_to_slot = app.selection.iter().any(|id| {
        app.doc.get(*id).is_some_and(|n| {
            matches!(n.kind, NodeKind::Frame(_))
                && !designos_doc::is_linked(n)
                && !n.component.is_slot
                && designos_doc::enclosing_component(&app.doc, *id).is_some_and(|c| c != *id)
        })
    });
    if can_convert_to_slot {
        list.push(leaf(MenuItem::new("Convert to slot"), Action::ConvertToSlot));
    }
    list.push(leaf(MenuItem::new("Delete").shortcut("Del").disabled(!has), Action::Delete));
    list
}

/// The action behind an index path of the open menu.
fn resolve(entries: &[Entry], path: &[usize]) -> Option<Action> {
    let e = entries.get(*path.first()?)?;
    match path.get(1) {
        None => e.act,
        Some(k) => e.kids.get(*k).map(|k| k.1),
    }
}

pub fn show(app: &mut App, ctx: &Context) {
    show_layer_picker(app, ctx);
    let Some(at) = app.ui.ctx_menu else { return };
    let entries = entries(app);
    let list: Vec<MenuItem> = entries
        .iter()
        .map(|e| {
            let mut it = e.item.clone();
            it.children = e.kids.iter().map(|k| k.0.clone()).collect();
            it
        })
        .collect();
    match popup_tree(ctx, Id::new("selection-menu"), at, 230.0, &list, None) {
        TreeResult::Picked(path) => {
            app.ui.ctx_menu = None;
            if let Some(a) = resolve(&entries, &path) {
                menus::run(app, ctx, a);
            }
        }
        TreeResult::Closed => app.ui.ctx_menu = None,
        TreeResult::Open => {}
    }
}

fn show_layer_picker(app: &mut App, ctx: &Context) {
    let Some((at, ids)) = app.ui.layer_picker.clone() else { return };
    if ids.is_empty() {
        app.ui.layer_picker = None;
        return;
    }
    let w = 220.0;
    let row_h = 26.0;
    let header_h = 26.0;
    let h = header_h + ids.len() as f32 * row_h + 6.0;
    let screen = ctx.content_rect();
    let x = at.x.clamp(screen.left() + 4.0, (screen.right() - w - 4.0).max(screen.left() + 4.0));
    let y = at.y.clamp(screen.top() + 4.0, (screen.bottom() - h - 4.0).max(screen.top() + 4.0));
    let r = egui::Rect::from_min_size(egui::Pos2::new(x, y), egui::Vec2::new(w, h));

    let mut picked: Option<designos_doc::NodeId> = None;
    let mut hovered_any = None;

    egui::Area::new(Id::new("layer-picker-menu")).order(egui::Order::Tooltip).fixed_pos(r.min).show(ctx, |ui| {
        let (_, _) = ui.allocate_exact_size(r.size(), egui::Sense::hover());
        let p = ui.painter();
        crate::widgets::elevation_shadow(p, r, crate::theme::RADIUS_FRAME);
        p.rect_filled(r, crate::theme::cr(crate::theme::RADIUS_FRAME), crate::theme::BG_MENU);
        p.rect_stroke(r, crate::theme::cr(crate::theme::RADIUS_FRAME), egui::Stroke::new(1.0, crate::theme::BORDER_MENU), egui::StrokeKind::Inside);
        p.text(
            egui::Pos2::new(r.left() + 12.0, r.top() + header_h * 0.5),
            egui::Align2::LEFT_CENTER,
            "Select layer",
            crate::theme::font_caption(),
            crate::theme::TEXT_SECONDARY,
        );
        p.hline(r.left() + 1.0..=r.right() - 1.0, r.top() + header_h, egui::Stroke::new(1.0, crate::theme::BORDER_MENU));

        for (idx, &id) in ids.iter().enumerate() {
            let Some(node) = app.doc.get(id) else { continue };
            let rr = egui::Rect::from_min_size(
                egui::Pos2::new(r.left() + 4.0, r.top() + header_h + 3.0 + idx as f32 * row_h),
                egui::Vec2::new(w - 8.0, row_h - 2.0),
            );
            let resp = ui.interact(rr, Id::new(("layer-picker-row", id)), egui::Sense::click());
            let hov = resp.hovered();
            let sel = app.is_selected(id);
            if hov {
                hovered_any = Some(id);
                p.rect_filled(rr, crate::theme::cr(4), crate::theme::BG_MENU_SELECTED);
            } else if sel {
                p.rect_filled(rr, crate::theme::cr(4), crate::theme::BG_SELECTED);
            }
            let icon = super::layers::kind_icon(&node.kind);
            let is_comp = matches!(node.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_) | NodeKind::Instance { .. });
            let icon_col = if hov {
                egui::Color32::WHITE
            } else if is_comp {
                crate::theme::COMPONENT
            } else {
                crate::theme::TEXT_SECONDARY
            };
            crate::icons::paint(p, icon, egui::Pos2::new(rr.left() + 12.0, rr.center().y), crate::theme::LAYER_ICON_SIZE, icon_col);
            let text_col = if hov { egui::Color32::WHITE } else { crate::theme::TEXT };
            p.text(egui::Pos2::new(rr.left() + 26.0, rr.center().y), egui::Align2::LEFT_CENTER, &node.name, crate::theme::font_body(), text_col);
            if resp.clicked() {
                picked = Some(id);
            }
        }
    });

    app.hover_layer = hovered_any;
    if let Some(id) = picked {
        app.select_only(id);
        app.hover_layer = None;
        app.ui.layer_picker = None;
    } else {
        let close = ctx.input(|i| i.key_pressed(egui::Key::Escape) || (i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !r.contains(p))));
        if close {
            app.hover_layer = None;
            app.ui.layer_picker = None;
        }
    }
}

#[cfg(test)]
pub fn labels(app: &App) -> Vec<String> {
    entries(app).into_iter().map(|e| e.item.label).collect()
}
