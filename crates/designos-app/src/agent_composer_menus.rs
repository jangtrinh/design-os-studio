//! The composer's attach menu and permission mode menu, hand-built popups that close on Esc and on a press outside.

use egui::{Align2, Id, Key, Pos2, Rect, Sense, Ui, Vec2};

use crate::agent_composer_toolbar::mode_label;
use crate::terminal::PermissionMode;
use crate::theme;
use crate::widgets::{self, kit};

pub enum AttachPick {
    Selection,
    Prd,
    Skills,
}

pub(super) const ATTACH_PASS: &str = "agents-composer-attach-menu-opened-pass";
pub(super) const MODE_PASS: &str = "agents-composer-mode-menu-opened-pass";
/// Widths of the composer's popup menus: the attach menu is narrow, the permission mode menu is wide, and the agent menu takes
/// what the panel allows between the two.
pub(crate) const MENU_W_NARROW: f32 = 196.0;
pub(crate) const MENU_W_WIDE: f32 = 232.0;
const ADD_SELECTION_TIP: &str = "Sends the selection and its unresolved comments to the agent";

#[derive(Clone, Copy)]
struct AttachRow<'a> {
    id: &'a str,
    label: &'a str,
    caption: Option<&'a str>,
    checked: bool,
    tip: Option<&'a str>,
}

pub(super) fn attach_menu(ui: &mut Ui, anchor: Rect, prd_on: bool, skills_n: usize) -> (Option<AttachPick>, bool) {
    let skills_caption = (skills_n > 0).then(|| format!("{skills_n} on"));
    let h = theme::INSET_FRAME * 2.0 + 3.0 * theme::MENU_ITEM_HEIGHT;
    let w = MENU_W_NARROW;
    let rect = Rect::from_min_size(Pos2::new(anchor.left(), anchor.top() - theme::SPACE_ROW - h), Vec2::new(w, h));
    menu_frame(ui, rect, "agents-composer-attach-menu");
    let mut pick = None;
    let rows = [
        AttachRow { id: "agents-composer-add-selection", label: "Add selection", caption: None, checked: false, tip: Some(ADD_SELECTION_TIP) },
        AttachRow { id: "agents-composer-attach-prd", label: "Attach PRD", caption: None, checked: prd_on, tip: None },
        AttachRow { id: "agents-composer-skills-item", label: "Skills…", caption: skills_caption.as_deref(), checked: false, tip: None },
    ];
    for (i, row) in rows.iter().enumerate() {
        let AttachRow { id, label, caption, checked, tip } = *row;
        let row = Rect::from_min_size(
            Pos2::new(rect.left() + theme::INSET_FRAME, rect.top() + theme::INSET_FRAME + i as f32 * theme::MENU_ITEM_HEIGHT),
            Vec2::new(w - theme::INSET_FRAME * 2.0, theme::MENU_ITEM_HEIGHT),
        );
        if menu_row(ui, row, id, label, caption, checked, tip) {
            pick = Some(match i {
                0 => AttachPick::Selection,
                1 => AttachPick::Prd,
                _ => AttachPick::Skills,
            });
        }
    }
    let stay = pick.is_none() && !close_menu(ui, rect, anchor, ATTACH_PASS);
    (pick, stay)
}

pub(super) fn mode_menu(ui: &mut Ui, anchor: Rect, current: PermissionMode) -> (Option<PermissionMode>, bool) {
    let modes = [PermissionMode::Skip, PermissionMode::Ask, PermissionMode::AskRisky];
    let h = theme::INSET_FRAME * 2.0 + (modes.len() as f32 + 1.0) * theme::MENU_ITEM_HEIGHT;
    let w = MENU_W_WIDE;
    let rect = Rect::from_min_size(Pos2::new(anchor.right() - w, anchor.top() - theme::SPACE_ROW - h), Vec2::new(w, h));
    menu_frame(ui, rect, "agents-composer-mode-menu");
    let mut pick = None;
    for (i, mode) in modes.iter().enumerate() {
        let row = Rect::from_min_size(
            Pos2::new(rect.left() + theme::INSET_FRAME, rect.top() + theme::INSET_FRAME + i as f32 * theme::MENU_ITEM_HEIGHT),
            Vec2::new(w - theme::INSET_FRAME * 2.0, theme::MENU_ITEM_HEIGHT),
        );
        if menu_row(ui, row, mode_id(*mode), mode_label(*mode), None, *mode == current, Some(mode_tip(*mode))) {
            pick = Some(*mode);
        }
    }
    let cap = Rect::from_min_size(
        Pos2::new(rect.left() + theme::INSET_FRAME, rect.bottom() - theme::INSET_FRAME - theme::MENU_ITEM_HEIGHT),
        Vec2::new(w - theme::INSET_FRAME * 2.0, theme::MENU_ITEM_HEIGHT),
    );
    ui.painter().text(
        Pos2::new(cap.left() + theme::SPACE_INLINE, cap.center().y),
        Align2::LEFT_CENTER,
        "Applies to new agent tabs.",
        theme::font_caption(),
        theme::TEXT_TERTIARY,
    );
    let stay = pick.is_none() && !close_menu(ui, rect, anchor, MODE_PASS);
    (pick, stay)
}

fn mode_tip(mode: PermissionMode) -> &'static str {
    match mode {
        PermissionMode::Skip => "Launch CLIs with their auto-approve flag. Fastest inside a trusted project.",
        PermissionMode::Ask => "Launch CLIs without skip flags and surface tool approvals in the Agents panel.",
        PermissionMode::AskRisky => "Reads and additions run. A change on one target can be allowed for this session. Deletes always ask.",
    }
}

fn mode_id(mode: PermissionMode) -> &'static str {
    match mode {
        PermissionMode::Ask => "agents-composer-mode-ask",
        PermissionMode::Skip => "agents-composer-mode-skip",
        PermissionMode::AskRisky => "agents-composer-mode-ask-risky",
    }
}

fn menu_frame(ui: &mut Ui, rect: Rect, id: &str) {
    kit::menu_surface(ui.painter(), rect);
    widgets::record(ui, id, rect);
    crate::widgets::popup_open::mark_open(ui.ctx());
}

fn menu_row(ui: &mut Ui, rect: Rect, id: &str, label: &str, caption: Option<&str>, checked: bool, tip: Option<&str>) -> bool {
    let resp = ui.interact(rect, Id::new(id), Sense::click());
    let look = kit::menu_row_look(false, resp.hovered() || resp.has_focus(), checked);
    kit::paint_frame(ui.painter(), rect, theme::RADIUS_CONTROL, &look);
    let p = ui.painter().clone();
    let mut right = rect.right() - theme::SPACE_INLINE;
    if checked {
        let mark_box = Rect::from_center_size(Pos2::new(right - theme::LAYER_ICON_SIZE * 0.5, rect.center().y), Vec2::splat(theme::LAYER_ICON_SIZE));
        widgets::paint_icon_in(&p, mark_box, crate::icons::Icon::Check, theme::LAYER_ICON_SIZE, look.mark);
        right -= theme::LAYER_ICON_SIZE + theme::SPACE_ROW;
    }
    if let Some(caption) = caption {
        let cap_w = widgets::text_width(&p, caption, &theme::font_caption());
        p.text(Pos2::new(right, rect.center().y), Align2::RIGHT_CENTER, caption, theme::font_caption(), theme::TEXT_TERTIARY);
        right -= cap_w + theme::SPACE_ROW;
    }
    let shown = widgets::elide(&p, label, &theme::font_caption(), (right - rect.left() - theme::SPACE_INLINE).max(theme::TARGET_MIN));
    p.text(Pos2::new(rect.left() + theme::SPACE_INLINE, rect.center().y), Align2::LEFT_CENTER, shown, theme::font_caption(), look.text);
    if let Some(tip) = tip {
        resp.clone().on_hover_text(tip);
    }
    widgets::record(ui, id, rect);
    resp.clicked()
}

/// Remembers where an anchor was drawn this frame. The stored value is overwritten in place, so a frame after the first allocates
/// nothing (`insert_temp` boxes a new value every call).
pub(crate) fn remember<T: Clone + Send + Sync + 'static>(ctx: &egui::Context, key: &'static str, value: T) {
    ctx.data_mut(|d| {
        let slot = d.get_temp_mut_or_insert_with(Id::new(key), || value.clone());
        *slot = value;
    });
}

pub(super) fn stamp_pass(ui: &Ui, id: &str) {
    let pass = ui.ctx().cumulative_pass_nr();
    ui.ctx().data_mut(|d| d.insert_temp(Id::new(id), pass));
}

/// True when `Esc` or a press outside the menu and its `anchor` asks it to close. A press on the anchor is left to the anchor's own
/// click, which toggles the menu: closing here as well would let that click open it again.
fn close_menu(ui: &Ui, rect: Rect, anchor: Rect, pass_id: &str) -> bool {
    if ui.input(|i| i.key_pressed(Key::Escape)) {
        return true;
    }
    let opened_this_pass = ui.ctx().data(|d| d.get_temp::<u64>(Id::new(pass_id))) == Some(ui.ctx().cumulative_pass_nr());
    if opened_this_pass {
        return false;
    }
    if ui.input(|i| i.pointer.any_pressed()) {
        if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
            return !rect.contains(pos) && !anchor.contains(pos);
        }
    }
    false
}
