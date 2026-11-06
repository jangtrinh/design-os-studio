//! The "Whiteboard" group of the Design toolbar: a divider, a group button and an inline row that opens beside it.
//!
//! The group is collapsed by default. `W` or a click on its button toggles it, and the choice is kept per user in the
//! library's `settings.json`. The row opens and closes in [`ANIM_SECS`], or at once when the OS asks for reduced motion.
//! When the window is too narrow for the whole row, the tools that do not fit move into a `...` menu.
//!
//! The row lists only tools that work in a Design file today. Sticky note, Shape with text, Stamp and Marker create
//! whiteboard items a Design document cannot hold yet, so they are not offered here (no control that does nothing).

use crate::app::{App, MenuKind};
use crate::canvas::tools::Tool;
use crate::icons::{self, Icon};
use crate::theme;
use crate::widgets;
use designos_doc::DocumentKind;
use egui::{Pos2, Rect, Sense, Ui, Vec2};
use std::path::{Path, PathBuf};

/// Key of the expanded state in `settings.json`.
pub const SETTING_KEY: &str = "toolbar.whiteboard_group_open";
/// Longest the row may take to open or close.
pub const ANIM_SECS: f32 = 0.12;
/// Tools of the inline row, in order. Each one is live in a Design file (connectors since the Design connector work,
/// Section from the Frame group).
pub const ROW_TOOLS: [Tool; 2] = [Tool::Connector, Tool::Section];
/// Room kept free on each side of the bar.
const SIDE_MARGIN: f32 = theme::TOOLBELT_BOTTOM_MARGIN;
/// One row slot: the gap before a button, then the button.
const SLOT_W: f32 = theme::TOOL_GROUP_GAP + theme::TOOL_BUTTON;
/// Width the divider and the group button add to the bar. The divider keeps the gaps of the one before the mode switcher
/// (`TOOLBAR_DIVIDER_GAP_BEFORE`, `TOOLBAR_DIVIDER_GAP_AFTER`); the gap that follows the button is the one the bar already
/// had before the Actions button.
pub const COLLAPSED_EXTRA: f32 = theme::TOOLBAR_DIVIDER_GAP_BEFORE + theme::BORDER_WIDTH + theme::TOOLBAR_DIVIDER_GAP_AFTER + theme::TOOL_BUTTON;

/// The Whiteboard group's state, kept on [`App`].
#[derive(Debug, Default)]
pub struct WbGroup {
    /// Whether the row is expanded.
    pub open: bool,
    /// Whether the state was read, and from which settings root.
    loaded: bool,
    loaded_root: Option<PathBuf>,
    /// Animation length chosen when the group was last toggled.
    anim_secs: Option<f32>,
    /// Test hook: `Some(b)` stands in for the OS reduced-motion setting.
    pub reduced_motion_override: Option<bool>,
    /// How many of [`ROW_TOOLS`] fit in the row at the last paint; the rest are in the `...` menu.
    shown: usize,
}

impl WbGroup {
    /// Reads the saved state from `root` once per root. A missing, unreadable or corrupt file means collapsed.
    pub fn ensure_loaded(&mut self, root: Option<&Path>) {
        if self.loaded && self.loaded_root.as_deref() == root {
            return;
        }
        self.open = root.and_then(|r| crate::comments::settings::load_flag(r, SETTING_KEY)).unwrap_or(false);
        self.loaded = true;
        self.loaded_root = root.map(Path::to_path_buf);
    }

    /// The tools that did not fit in the row at the last paint.
    pub fn overflow(&self) -> &'static [Tool] {
        &ROW_TOOLS[self.shown.min(ROW_TOOLS.len())..]
    }

    /// Flips the state and saves it to `root`. The new state applies even when the save fails; the error is returned.
    pub fn toggle(&mut self, root: Option<&Path>) -> Result<(), String> {
        self.open = !self.open;
        self.anim_secs = Some(if self.reduced_motion() { 0.0 } else { ANIM_SECS });
        match root {
            Some(r) => crate::comments::settings::save_flag(r, SETTING_KEY, self.open),
            None => Ok(()),
        }
    }

    fn reduced_motion(&self) -> bool {
        self.reduced_motion_override.unwrap_or_else(os_reduced_motion)
    }
}

/// Whether the OS asks apps to reduce motion. `DESIGNOS_REDUCE_MOTION=1` forces it on. Read at each toggle, so a change
/// in the system settings applies without a restart. Windows has no reader yet: it reports false.
fn os_reduced_motion() -> bool {
    if let Ok(v) = std::env::var("DESIGNOS_REDUCE_MOTION") {
        return matches!(v.trim(), "1" | "true" | "yes" | "on");
    }
    platform_reduced_motion()
}

/// `NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceMotion`: the "Reduce motion" accessibility switch.
#[cfg(target_os = "macos")]
fn platform_reduced_motion() -> bool {
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2::{msg_send, rc::Retained};
    let Some(class) = AnyClass::get(c"NSWorkspace") else { return false };
    // SAFETY: both selectors exist on NSWorkspace (macOS 10.12+) and take no arguments; `sharedWorkspace` returns a
    // retained singleton and the property returns a BOOL.
    unsafe {
        let workspace: Option<Retained<AnyObject>> = msg_send![class, sharedWorkspace];
        match workspace {
            Some(ws) => msg_send![&*ws, accessibilityDisplayShouldReduceMotion],
            None => false,
        }
    }
}

#[cfg(target_os = "linux")]
fn platform_reduced_motion() -> bool {
    std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "enable-animations"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "false")
        .unwrap_or(false)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn platform_reduced_motion() -> bool {
    false
}

/// Whether the Design toolbar carries the group in the active document. Board and Deck toolbars never do.
pub fn available(app: &App) -> bool {
    app.doc.kind == DocumentKind::Design && !app.ui.dev_mode
}

/// How the row's tools split between the bar and the `...` menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowFit {
    /// Tools shown as buttons.
    pub shown: usize,
    /// Whether a `...` button follows them.
    pub more: bool,
}

impl RowFit {
    /// Slots the expanded row occupies.
    pub fn slots(self) -> usize {
        self.shown + usize::from(self.more)
    }

    /// Width of the expanded row.
    pub fn width(self) -> f32 {
        self.slots() as f32 * SLOT_W
    }
}

/// Splits [`ROW_TOOLS`] for `room` px of row. All fit: no `...`. Otherwise `...` takes one slot and the tools that
/// still fit in front of it stay as buttons.
pub fn fit_row(room: f32) -> RowFit {
    let n = ROW_TOOLS.len();
    if n as f32 * SLOT_W <= room {
        return RowFit { shown: n, more: false };
    }
    let shown = (((room - SLOT_W) / SLOT_W).floor().max(0.0) as usize).min(n);
    RowFit { shown, more: true }
}

/// Room for the row in a bar of the base width inside `screen`.
fn row_room(screen: Rect, base_width: f32) -> f32 {
    screen.width() - SIDE_MARGIN * 2.0 - base_width - COLLAPSED_EXTRA
}

/// Extra width the group adds to the bar at open fraction `t`.
pub fn extra_width(screen: Rect, base_width: f32, t: f32) -> f32 {
    COLLAPSED_EXTRA + fit_row(row_room(screen, base_width)).width() * t.clamp(0.0, 1.0)
}

fn anim_id() -> egui::Id {
    egui::Id::new("wb-group-open-fraction")
}

/// The open fraction (0 collapsed, 1 expanded) to draw this frame.
pub fn open_fraction(app: &App, ctx: &egui::Context) -> f32 {
    let secs = app.wb_group.anim_secs.unwrap_or(ANIM_SECS);
    ctx.animate_bool_with_time_and_easing(anim_id(), app.wb_group.open, secs, egui::emath::easing::cubic_out)
}

/// Toggles the group and saves the choice; a failed save is reported in a toast.
pub fn toggle(app: &mut App) {
    let root = app.lib.as_ref().map(|l| l.root());
    app.wb_group.ensure_loaded(root);
    if let Err(e) = app.wb_group.toggle(root) {
        app.show_toast(e, true);
    }
}

/// Where the group is drawn this frame.
#[derive(Clone, Copy)]
pub struct GroupSpot {
    /// The bar, already widened for the group.
    pub bar: Rect,
    /// Left edge of the divider.
    pub x: f32,
    /// Top of the 32 px buttons.
    pub y: f32,
    /// The area the bar is centred in.
    pub screen: Rect,
    /// Width of the bar without the group.
    pub base_width: f32,
    /// Open fraction, 0 to 1.
    pub t: f32,
}

/// Paints the divider, the group button and the row, starting at the right edge of the last tool. Returns the x after the
/// group (before the gap that precedes the next control).
pub fn show(app: &mut App, ui: &mut Ui, spot: GroupSpot) -> f32 {
    let GroupSpot { bar, mut x, y, screen, base_width, t } = spot;
    let p = ui.painter().clone();
    x += theme::TOOLBAR_DIVIDER_GAP_BEFORE;
    widgets::vline(&p, x, bar.top() + theme::BORDER_WIDTH, bar.bottom() - theme::BORDER_WIDTH, theme::BORDER);
    x += theme::BORDER_WIDTH + theme::TOOLBAR_DIVIDER_GAP_AFTER;

    let br = Rect::from_min_size(Pos2::new(x, y), Vec2::splat(theme::TOOL_BUTTON));
    widgets::record(ui, "toolbar-whiteboard-group", br);
    let resp = ui.interact(br, ui.id().with("wb-group"), Sense::click());
    let look = super::toolbar::tool_look(&resp, app.wb_group.open);
    crate::widgets::kit::paint_frame(&p, br, theme::RADIUS_CONTROL, &look);
    icons::paint(&p, Icon::Sticky, br.center(), theme::TOOL_ICON_SIZE, look.icon);
    super::toolbar::mark_focus(ui, &p, resp.id, br, theme::RADIUS_CONTROL as f32);
    if resp.on_hover_text("Whiteboard   W").clicked() {
        toggle(app);
    }
    x += theme::TOOL_BUTTON;

    let fit = fit_row(row_room(screen, base_width));
    app.wb_group.shown = fit.shown;
    if t <= 0.0 {
        if app.ui.menu == Some(MenuKind::WhiteboardMore) {
            app.ui.menu = None;
        }
        return x;
    }
    // The row grows from the group button; its slots are clipped to the part opened so far.
    let clip = Rect::from_min_size(Pos2::new(x, bar.top()), Vec2::new(fit.width() * t, bar.height()));
    let cp = p.with_clip_rect(clip.intersect(ui.clip_rect()));
    let settled = t >= 1.0;
    for (i, tool) in ROW_TOOLS.iter().take(fit.shown).enumerate() {
        let r = Rect::from_min_size(Pos2::new(x + theme::TOOL_GROUP_GAP + i as f32 * SLOT_W, y), Vec2::splat(theme::TOOL_BUTTON));
        let selected = app.tool == *tool;
        let id = ui.id().with(("wb-row", i));
        let resp = if settled { ui.interact(r, id, Sense::click()) } else { ui.interact(Rect::NOTHING, id, Sense::hover()) };
        let look = super::toolbar::tool_look(&resp, selected);
        crate::widgets::kit::paint_frame(&cp, r, theme::RADIUS_CONTROL, &look);
        icons::paint(&cp, super::toolbar::tool_icon(*tool), r.center(), theme::TOOL_ICON_SIZE, look.icon);
        if settled {
            widgets::record_with(ui, || format!("toolbar-whiteboard-tool-{i}"), r);
            super::toolbar::mark_focus(ui, &p, resp.id, r, theme::RADIUS_CONTROL as f32);
            let tip = format!("{}   {}", tool.label(), tool.shortcut());
            if resp.on_hover_text(tip).clicked() {
                app.set_tool(*tool);
            }
        }
    }
    if fit.more {
        let r = Rect::from_min_size(Pos2::new(x + theme::TOOL_GROUP_GAP + fit.shown as f32 * SLOT_W, y), Vec2::splat(theme::TOOL_BUTTON));
        let open = app.ui.menu == Some(MenuKind::WhiteboardMore);
        let id = ui.id().with("wb-more");
        let resp = if settled { ui.interact(r, id, Sense::click()) } else { ui.interact(Rect::NOTHING, id, Sense::hover()) };
        let look = super::toolbar::tool_look(&resp, open);
        crate::widgets::kit::paint_frame(&cp, r, theme::RADIUS_CONTROL, &look);
        icons::paint(&cp, Icon::Ellipsis, r.center(), theme::TOOL_ICON_SIZE, look.icon);
        if settled {
            widgets::record(ui, "toolbar-whiteboard-more", r);
            super::toolbar::mark_focus(ui, &p, resp.id, r, theme::RADIUS_CONTROL as f32);
            if resp.on_hover_text("More whiteboard tools").clicked() {
                app.ui.menu = if open { None } else { Some(MenuKind::WhiteboardMore) };
            }
            if app.ui.menu == Some(MenuKind::WhiteboardMore) {
                app.ui.menu_anchor = Some(r);
            }
        }
    } else if app.ui.menu == Some(MenuKind::WhiteboardMore) {
        app.ui.menu = None;
    }
    x + fit.width() * t
}

#[cfg(test)]
#[path = "toolbar_whiteboard_tests.rs"]
mod tests;
