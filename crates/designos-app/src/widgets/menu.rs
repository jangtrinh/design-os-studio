//! Popup menus (Figma UI3 style) with submenus and keyboard navigation, and dropdown fields.
//!
//! Geometry (all from the ui-spec `menus` block): 8px vertical panel padding, 24px rows,
//! highlight pills inset 8px with radius 5, check column at x=12, icon box at x=32 (or x=12
//! without a check column), labels at x=60 with icon / x=16 without, shortcuts right-aligned
//! 16px from the edge, 24px chevrons 8px from the edge, 1px separators with 8px margins.

use super::*;
use egui::{Area, Key, Modifiers, Order};

#[derive(Clone, Default)]
pub struct MenuItem {
    pub label: String,
    pub shortcut: String,
    pub checked: bool,
    /// Reserves the check column in the panel (set by `checked`).
    pub checkable: bool,
    pub disabled: bool,
    pub separator_after: bool,
    pub icon: Option<Icon>,
    pub tooltip: String,
    pub children: Vec<MenuItem>,
}

impl MenuItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into(), ..Default::default() }
    }
    pub fn shortcut(mut self, s: &str) -> Self {
        self.shortcut = s.into();
        self
    }
    pub fn checked(mut self, c: bool) -> Self {
        self.checked = c;
        self.checkable = true;
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    pub fn sep(mut self) -> Self {
        self.separator_after = true;
        self
    }
    pub fn icon(mut self, i: Icon) -> Self {
        self.icon = Some(i);
        self
    }
    pub fn tooltip(mut self, t: impl Into<String>) -> Self {
        self.tooltip = t.into();
        self
    }
    fn has_sub(&self) -> bool {
        !self.children.is_empty()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum MenuResult {
    Open,
    Picked(usize),
    Closed,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TreeResult {
    Open,
    /// Indices from the top level down to the picked leaf.
    Picked(Vec<usize>),
    Closed,
}

/// Vertical space of a separator between two rows (margin + 1px line + margin).
pub const SEP_H: f32 = theme::MENU_SEP_MARGIN * 2.0 + theme::BORDER_WIDTH;
/// Width given to submenu panels at minimum.
const SUBMENU_MIN_W: f32 = 190.0;

/// Highlight path: `hl[k]` is the highlighted row of open level `k`; level `k + 1` is open
/// when it exists.
#[derive(Clone, Default)]
struct MenuState {
    hl: Vec<Option<usize>>,
    /// Scroll offset of every open level; only a level taller than the screen is ever non-zero.
    scroll: Vec<f32>,
    /// Level whose highlighted row has to be brought into view this frame.
    reveal: Option<usize>,
    /// The primary button went down while this menu was up and has not come up yet. Only a release that ends such
    /// a press picks a row, so the release of the click that opened the menu never does.
    armed: bool,
    /// The menu is being shown for the first time (or again after it was gone): what opened it, a key press included,
    /// has not been handled by it yet.
    fresh: bool,
    seen: u64,
}

/// Stored highlight state; a menu opened fresh starts with its checked row highlighted.
fn load_state(ctx: &egui::Context, id: Id, items: &[MenuItem]) -> MenuState {
    let now = ctx.cumulative_pass_nr();
    let stored = ctx.data(|d| d.get_temp::<MenuState>(id)).filter(|s| now <= s.seen + 3);
    let fresh = stored.is_none();
    let mut st = stored.unwrap_or_default();
    st.fresh = fresh;
    if st.hl.is_empty() {
        st.hl.push(if fresh { items.iter().position(|i| i.checked && !i.disabled) } else { None });
        st.reveal = st.hl[0].map(|_| 0);
    }
    st
}

fn store_state(ctx: &egui::Context, id: Id, mut st: MenuState) {
    st.seen = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| d.insert_temp(id, st));
}

/// Opens the menu `id` with the given highlight path (used by `--menu main/File`).
pub fn preset_path(ctx: &egui::Context, id: Id, path: &[usize]) {
    let mut st = MenuState { hl: path.iter().map(|i| Some(*i)).collect(), ..Default::default() };
    if !st.hl.is_empty() {
        st.hl.push(None);
    }
    store_state(ctx, id, st);
}

/// Walks the frame's button events in arrival order and returns where the last press (of any button) went down and
/// where the primary button went up to end a press the menu saw. `armed` carries "a primary press is still down"
/// from frame to frame. A press or release that `covered` says a higher layer (a toast with an action, say) took does
/// not count: that layer's gesture is not the menu's.
///
/// egui's own click needs the release within 6 px and 0.8 s of the press and on the widget that took the press, and
/// it resolves widgets with the pointer position at the END of the frame. A menu that picked on that click dropped
/// picks after a small move or a slow frame, and never saw a press on the opening button slide onto a row. The raw
/// events know each position, whatever the window batched into one frame.
fn primary_gesture(events: &[egui::Event], armed: &mut bool, covered: impl Fn(Pos2) -> bool) -> (Option<Pos2>, Option<Pos2>) {
    let (mut press, mut release) = (None, None);
    for ev in events {
        if let egui::Event::PointerButton { pos, button, pressed, .. } = ev {
            if *pressed {
                press = Some(*pos);
            }
            if *button == egui::PointerButton::Primary {
                if *pressed {
                    *armed = !covered(*pos);
                } else if std::mem::take(armed) && !covered(*pos) {
                    release = Some(*pos);
                }
            }
        }
    }
    (press, release)
}

/// The panel of one menu level: size, per-row tops (relative to the panel top).
struct Layout {
    w: f32,
    h: f32,
    tops: Vec<f32>,
    has_check: bool,
}

impl Layout {
    fn new(ctx: &egui::Context, items: &[MenuItem], min_w: f32) -> Self {
        let font = theme::font_menu();
        let has_check = items.iter().any(|i| i.checkable);
        let mut tops = Vec::with_capacity(items.len());
        let mut y = theme::MENU_PADDING;
        for it in items {
            tops.push(y);
            y += theme::MENU_ITEM_HEIGHT + if it.separator_after { SEP_H } else { 0.0 };
        }
        let mut need = 0.0f32;
        ctx.fonts_mut(|f| {
            for it in items {
                let lw = f.layout_no_wrap(it.label.clone(), font.clone(), Color32::WHITE).size().x;
                let sw = if it.shortcut.is_empty() { 0.0 } else { f.layout_no_wrap(it.shortcut.clone(), font.clone(), Color32::WHITE).size().x };
                let right = if it.has_sub() {
                    theme::MENU_LABEL_GAP + theme::MENU_CHEVRON_BOX + theme::MENU_CHEVRON_PAD
                } else if sw > 0.0 {
                    theme::MENU_LABEL_GAP + sw + theme::MENU_SHORTCUT_PAD
                } else {
                    theme::MENU_SHORTCUT_PAD
                };
                need = need.max(label_x(it, has_check) + lw + right);
            }
        });
        Self { w: need.ceil().max(min_w), h: y + theme::MENU_PADDING, tops, has_check }
    }

    /// Whether the level is taller than the `limit` it may fill, so that it scrolls.
    fn scrolls(&self, limit: f32) -> bool {
        self.h > limit
    }

    fn panel_h(&self, limit: f32) -> f32 {
        self.h.min(limit)
    }

    fn max_scroll(&self, limit: f32) -> f32 {
        (self.h - limit).max(0.0)
    }

    /// `scroll` moved by the least amount that brings row `i` fully between the panel's vertical paddings.
    fn scroll_to_row(&self, i: usize, scroll: f32, limit: f32) -> f32 {
        let (top, bottom) = (self.tops[i], self.tops[i] + theme::MENU_ITEM_HEIGHT);
        scroll.max(bottom + theme::MENU_PADDING - limit).min(top - theme::MENU_PADDING).clamp(0.0, self.max_scroll(limit))
    }
}

/// Screen rect of every open level. A level taller than the screen is clamped to it, and a submenu hangs
/// off its parent's row at the parent's current scroll.
fn place_levels(lays: &[Layout], st: &MenuState, pos: Pos2, screen: Rect) -> Vec<Rect> {
    let limit = screen.height() - 4.0;
    let mut rects: Vec<Rect> = Vec::with_capacity(lays.len());
    for (k, lay) in lays.iter().enumerate() {
        let size = Vec2::new(lay.w, lay.panel_h(limit));
        let mut p = if k == 0 {
            pos
        } else {
            let parent: Rect = rects[k - 1];
            let row_top = parent.top() + lays[k - 1].tops[st.hl[k - 1].unwrap_or(0)] - st.scroll[k - 1];
            let mut x = parent.right() - theme::SUBMENU_OVERLAP;
            if x + size.x > screen.right() - 2.0 {
                x = parent.left() + theme::SUBMENU_OVERLAP - size.x;
            }
            Pos2::new(x, row_top - theme::MENU_PADDING)
        };
        p.x = p.x.min(screen.right() - size.x - 2.0).max(screen.left() + 2.0);
        p.y = p.y.min(screen.bottom() - size.y - 2.0).max(screen.top() + 2.0);
        rects.push(Rect::from_min_size(p, size));
    }
    rects
}

/// The deepest open level under the pointer; it owns hover and the wheel.
fn level_under(ptr: Option<Pos2>, rects: &[Rect]) -> Option<usize> {
    ptr.and_then(|p| (0..rects.len()).rev().find(|k| rects[*k].contains(p)))
}

fn label_x(it: &MenuItem, has_check: bool) -> f32 {
    match (it.icon.is_some(), has_check) {
        (true, true) => theme::MENU_LABEL_X_ICON,
        (true, false) => theme::MENU_ICON_X_PLAIN + theme::MENU_ICON_BOX + theme::MENU_ICON_LABEL_GAP,
        (false, true) => theme::MENU_LABEL_X_CHECK,
        (false, false) => theme::MENU_LABEL_X_PLAIN,
    }
}

fn next_enabled(items: &[MenuItem], cur: Option<usize>, dir: i32) -> Option<usize> {
    let n = items.len() as i32;
    if n == 0 || items.iter().all(|i| i.disabled) {
        return None;
    }
    let mut i = match (cur, dir) {
        (Some(c), _) => c as i32,
        (None, 1) => -1,
        (None, _) => n,
    };
    loop {
        i = (i + dir).rem_euclid(n);
        if !items[i as usize].disabled {
            return Some(i as usize);
        }
    }
}

fn first_enabled(items: &[MenuItem]) -> Option<usize> {
    items.iter().position(|i| !i.disabled)
}

/// The item lists of every open level, plus the validated highlight path.
fn levels<'a>(items: &'a [MenuItem], st: &mut MenuState) -> Vec<&'a [MenuItem]> {
    let mut out = vec![items];
    let mut k = 0;
    while k + 1 < st.hl.len() {
        let child = st.hl[k].and_then(|i| out[k].get(i)).filter(|it| it.has_sub());
        match child {
            Some(it) => out.push(&it.children),
            None => {
                st.hl.truncate(k + 1);
                break;
            }
        }
        k += 1;
    }
    out
}

/// Shows a floating menu at `pos`. `anchor` is the button that opened it (a
/// press on it does not count as an outside click so the button can toggle).
pub fn popup_menu(ctx: &egui::Context, id: Id, pos: Pos2, min_w: f32, items: &[MenuItem], anchor: Option<Rect>) -> MenuResult {
    match popup_tree(ctx, id, pos, min_w, items, anchor) {
        TreeResult::Open => MenuResult::Open,
        TreeResult::Closed => MenuResult::Closed,
        TreeResult::Picked(p) => MenuResult::Picked(p[0]),
    }
}

/// Like `popup_menu` but with submenus: returns the full index path of the picked leaf.
pub fn popup_tree(ctx: &egui::Context, id: Id, pos: Pos2, min_w: f32, items: &[MenuItem], anchor: Option<Rect>) -> TreeResult {
    let mut st = load_state(ctx, id, items);
    let before = st.hl.clone();
    let screen = ctx.content_rect();
    let mut result = TreeResult::Open;

    // ---- keyboard
    let (down, up, right, left, enter, esc) = ctx.input_mut(|i| {
        let n = Modifiers::NONE;
        (
            i.consume_key(n, Key::ArrowDown),
            i.consume_key(n, Key::ArrowUp),
            i.consume_key(n, Key::ArrowRight),
            i.consume_key(n, Key::ArrowLeft),
            i.consume_key(n, Key::Enter) || i.consume_key(n, Key::Space),
            i.consume_key(n, Key::Escape),
        )
    });
    // The Enter or Space that opened the menu (on a focused toolbar chevron) is the opener's, not a pick of the
    // highlighted row.
    let enter = enter && !st.fresh;
    {
        let lv = levels(items, &mut st);
        let d = st.hl.len() - 1;
        let cur = st.hl[d];
        let list = lv[d];
        if down {
            st.hl[d] = next_enabled(list, cur, 1);
        } else if up {
            st.hl[d] = next_enabled(list, cur, -1);
        }
        if down || up {
            st.reveal = Some(d);
        }
        let open_sub = |st: &mut MenuState| {
            if let Some(it) = cur.and_then(|i| list.get(i)).filter(|it| it.has_sub() && !it.disabled) {
                st.hl.push(first_enabled(&it.children));
            }
        };
        if right {
            open_sub(&mut st);
        }
        if left && d > 0 {
            st.hl.pop();
        }
        if enter {
            if let Some(i) = cur {
                let it = &list[i];
                if it.has_sub() {
                    open_sub(&mut st);
                } else if !it.disabled {
                    let mut path: Vec<usize> = st.hl[..d].iter().map(|h| h.unwrap_or(0)).collect();
                    path.push(i);
                    result = TreeResult::Picked(path);
                }
            }
        }
        if esc {
            if d > 0 {
                st.hl.pop();
            } else {
                result = TreeResult::Closed;
            }
        }
    }

    // ---- layout of every open level
    let lv = levels(items, &mut st);
    // What the buttons did this frame. A press or release that landed on a layer above this menu's own levels (a toast)
    // belongs to that layer; the app's own background ui and the panels under the menu do not cover it. The events are
    // copied out first: `layer_id_at` takes the context lock.
    let events = ctx.input(|i| i.events.clone());
    let own_layers: Vec<egui::LayerId> = (0..lv.len()).map(|k| egui::LayerId::new(Order::Foreground, id.with(("lvl", k)))).collect();
    let covered = |p: Pos2| ctx.layer_id_at(p).is_some_and(|l| l.order >= Order::Foreground && !own_layers.contains(&l));
    let (press_pos, release_pos) = primary_gesture(&events, &mut st.armed, covered);
    let lays: Vec<Layout> = lv.iter().enumerate().map(|(k, list)| Layout::new(ctx, list, if k == 0 { min_w } else { SUBMENU_MIN_W })).collect();
    let limit = screen.height() - 4.0;
    st.scroll.resize(lays.len(), 0.0);
    for (s, lay) in st.scroll.iter_mut().zip(&lays) {
        *s = s.clamp(0.0, lay.max_scroll(limit));
    }
    let ptr = ctx.input(|i| i.pointer.hover_pos());
    let mut rects = place_levels(&lays, &st, pos, screen);
    // The wheel scrolls the level under the pointer; a keyboard move scrolls its row into view.
    let scroll_before = st.scroll.clone();
    let mut wheeled = false;
    if let Some(k) = level_under(ptr, &rects).filter(|k| lays[*k].scrolls(limit)) {
        let dy = ctx.input(|i| i.smooth_scroll_delta.y);
        if dy != 0.0 {
            st.scroll[k] = (st.scroll[k] - dy).clamp(0.0, lays[k].max_scroll(limit));
            ctx.input_mut(|i| i.smooth_scroll_delta = Vec2::ZERO);
            wheeled = true;
        }
    }
    if let Some(k) = st.reveal.take() {
        if let (Some(lay), Some(Some(i))) = (lays.get(k), st.hl.get(k)) {
            if lay.scrolls(limit) {
                st.scroll[k] = lay.scroll_to_row(*i, st.scroll[k], limit);
            }
        }
    }
    if st.scroll != scroll_before {
        rects = place_levels(&lays, &st, pos, screen);
        ctx.request_repaint();
    }

    // ---- paint + interaction
    let moved = wheeled || ctx.input(|i| i.pointer.delta() != Vec2::ZERO || i.pointer.any_pressed());
    // A press of any button outside every level (and off the button that opened the menu) closes it, and the gesture
    // it starts picks nothing, wherever it is released.
    let press_outside = press_pos.is_some_and(|p| !rects.iter().any(|r| r.contains(p)) && !anchor.is_some_and(|a| a.contains(p)));
    let release_pos = release_pos.filter(|_| !press_outside);
    let release_owner = release_pos.and_then(|p| level_under(Some(p), &rects));
    let font = theme::font_menu();
    let mut hover_hit: Option<(usize, usize)> = None;
    let mut clicked: Option<(usize, usize)> = None;
    // Topmost (deepest) panel under the pointer owns hover.
    let owner = level_under(ptr, &rects);
    for (k, list) in lv.iter().enumerate() {
        let rect = rects[k];
        let lay = &lays[k];
        // A level taller than the screen scrolls: rows are clipped to, and only hit inside, the band between the paddings.
        let capped = lay.scrolls(limit);
        let scroll = if capped { st.scroll[k] } else { 0.0 };
        let band = rect.shrink2(Vec2::new(0.0, theme::MENU_PADDING));
        Area::new(id.with(("lvl", k))).order(Order::Foreground).fixed_pos(rect.min).interactable(true).show(ctx, |ui| {
            ui.allocate_exact_size(rect.size(), Sense::hover());
            #[cfg(test)]
            record(ui, &format!("menu-level-{k}"), rect);
            let p = ui.painter().clone();
            menu_panel(&p, rect, theme::RADIUS_FRAME);
            let p = if capped { p.with_clip_rect(band) } else { p };
            for (i, it) in list.iter().enumerate() {
                let top = rect.top() + lay.tops[i] - scroll;
                let row = Rect::from_min_size(Pos2::new(rect.left(), top), Vec2::new(rect.width(), theme::MENU_ITEM_HEIGHT));
                #[cfg(test)]
                record(ui, &format!("menu-row-{k}-{i}"), row);
                #[cfg(test)]
                record(ui, &format!("menu-check-{k}-{i}"), if it.checked { row } else { Rect::NOTHING });
                let hit = if capped { row.intersect(band) } else { row };
                if !hit.is_positive() {
                    if it.separator_after {
                        hline(&p, rect.left(), rect.right(), row.bottom() + theme::MENU_SEP_MARGIN, theme::BORDER_MENU);
                    }
                    continue;
                }
                let pill = Rect::from_min_size(
                    Pos2::new(rect.left() + theme::MENU_PILL_INSET, top),
                    Vec2::new(rect.width() - theme::MENU_PILL_INSET * 2.0, theme::MENU_ITEM_HEIGHT),
                );
                let resp = ui.interact(hit, id.with(("row", k, i)), if it.disabled { Sense::hover() } else { Sense::click() });
                if !it.tooltip.is_empty() {
                    let _ = resp.clone().on_hover_text(&it.tooltip);
                }
                let lit = st.hl.get(k).copied().flatten() == Some(i) && !it.disabled;
                if owner == Some(k) && ptr.is_some_and(|p| hit.contains(p)) {
                    hover_hit = Some((k, i));
                }
                // A release over the row picks it, however far or long the pointer travelled since the press. egui gives a
                // click within 6 px to the row that was PRESSED, which can be the neighbour of the one released on, so
                // a pointer click is not taken from `clicked`; Space, Enter and a screen reader's activation still are.
                let released_here = release_owner == Some(k) && release_pos.is_some_and(|p| hit.contains(p));
                let activated = resp.clicked() && !resp.clicked_by(egui::PointerButton::Primary);
                if (released_here || activated) && !it.disabled {
                    clicked = Some((k, i));
                }
                if lit {
                    p.rect_filled(pill, theme::cr(theme::RADIUS_CONTROL), theme::BG_MENU_SELECTED);
                }
                let col = if it.disabled { theme::TEXT_DISABLED } else { theme::TEXT };
                let cy = row.center().y;
                if it.checked {
                    icons::paint(&p, Icon::Check, Pos2::new(rect.left() + theme::MENU_CHECK_X + theme::MENU_CHECK_BOX / 2.0, cy), theme::MENU_ICON_SIZE, col);
                }
                if let Some(ic) = it.icon {
                    let ix = if lay.has_check { theme::MENU_ICON_X_CHECKED } else { theme::MENU_ICON_X_PLAIN };
                    icons::paint(&p, ic, Pos2::new(rect.left() + ix + theme::MENU_ICON_BOX / 2.0, cy), theme::MENU_ICON_SIZE, col);
                }
                p.text(Pos2::new(rect.left() + label_x(it, lay.has_check), cy), Align2::LEFT_CENTER, &it.label, font.clone(), col);
                if it.has_sub() {
                    let c = if it.disabled {
                        theme::TEXT_DISABLED
                    } else if lit {
                        theme::MENU_SHORTCUT_HIGHLIGHT
                    } else {
                        theme::ICON_SECONDARY
                    };
                    icons::paint(
                        &p,
                        Icon::ChevronRight,
                        Pos2::new(rect.right() - theme::MENU_CHEVRON_PAD - theme::MENU_CHEVRON_BOX / 2.0, cy),
                        theme::MENU_CHEVRON_SIZE,
                        c,
                    );
                } else if !it.shortcut.is_empty() {
                    let c = if it.disabled {
                        theme::TEXT_DISABLED
                    } else if lit {
                        theme::MENU_SHORTCUT_HIGHLIGHT
                    } else {
                        theme::TEXT_SECONDARY
                    };
                    p.text(Pos2::new(rect.right() - theme::MENU_SHORTCUT_PAD, cy), Align2::RIGHT_CENTER, &it.shortcut, font.clone(), c);
                }
                if it.separator_after {
                    hline(&p, rect.left(), rect.right(), row.bottom() + theme::MENU_SEP_MARGIN, theme::BORDER_MENU);
                }
            }
            if capped {
                scroll_thumb(ui.painter(), band, lay.h - theme::MENU_PADDING * 2.0, scroll, lay.max_scroll(limit));
            }
        });
    }

    // ---- hover follows the mouse (only when it moved, so the keyboard can take over). Not on a frame with a pick: the
    // pick reads the highlight path to name the row it ran, and the pointer may already be over another parent.
    if moved && clicked.is_none() {
        if let Some((k, i)) = hover_hit {
            let it = &lv[k][i];
            st.hl.truncate(k + 1);
            if it.disabled {
                st.hl[k] = None;
            } else if st.hl[k] != Some(i) || (it.has_sub() && st.hl.len() == k + 1) {
                st.hl[k] = Some(i);
                if it.has_sub() {
                    st.hl.push(None);
                }
            }
        }
    }
    if let Some((k, i)) = clicked {
        let it = &lv[k][i];
        if it.has_sub() {
            st.hl.truncate(k + 1);
            st.hl[k] = Some(i);
            if st.hl.len() == k + 1 {
                st.hl.push(None);
            }
        } else {
            let mut path: Vec<usize> = st.hl[..k].iter().map(|h| h.unwrap_or(0)).collect();
            path.push(i);
            result = TreeResult::Picked(path);
        }
    }

    // ---- outside press closes
    if result == TreeResult::Open && press_outside {
        result = TreeResult::Closed;
    }
    if result != TreeResult::Open {
        st.armed = false;
    }
    if st.hl != before {
        ctx.request_repaint();
    }
    store_state(ctx, id, st);
    // An open menu holds the keys from a surface that draws before it (`widgets::popup_open`); one that closed here does not.
    if result == TreeResult::Open {
        super::popup_open::mark_open(ctx);
    }
    result
}

/// Position marker on the right edge of a scrolling level: `band` is the visible rows area, `content_h` the height of all rows.
fn scroll_thumb(p: &egui::Painter, band: Rect, content_h: f32, scroll: f32, max_scroll: f32) {
    let h = (band.height() * band.height() / content_h).max(theme::MENU_ITEM_HEIGHT);
    let t = if max_scroll > 0.0 { scroll / max_scroll } else { 0.0 };
    let y = band.top() + (band.height() - h) * t;
    p.rect_filled(Rect::from_min_size(Pos2::new(band.right() - 6.0, y), Vec2::new(4.0, h)), theme::cr(2), theme::TEXT_TERTIARY);
}

/// A dropdown field: shows `label` with a chevron; returns the picked option. The option equal to `label` is checked.
pub fn dropdown(ui: &mut Ui, rect: Rect, key: impl crate::widgets::Key, label: &str, options: &[String]) -> Option<usize> {
    let checked: Vec<bool> = options.iter().map(|o| o == label).collect();
    dropdown_checked(ui, rect, key, label, options, &checked)
}

/// Like `dropdown`, but `checked[i]` decides whether option `i` shows a check mark, so a multi-select
/// list marks every selected row regardless of what the closed field shows.
pub fn dropdown_checked(ui: &mut Ui, rect: Rect, key: impl crate::widgets::Key, label: &str, options: &[String], checked: &[bool]) -> Option<usize> {
    let id = ui.id().with(("dd", key));
    let r = ui.interact(rect, id, Sense::click());
    let open = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    let p = ui.painter();
    p.rect_filled(rect, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
    if r.hovered() || open {
        p.rect_stroke(rect, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, if open { theme::BORDER_SELECTED } else { theme::BORDER }), StrokeKind::Inside);
    }
    let font = theme::font_menu();
    let shown = elide(p, label, &font, rect.width() - 28.0);
    p.text(Pos2::new(rect.left() + 8.0, rect.center().y), Align2::LEFT_CENTER, shown, font, theme::TEXT);
    icons::paint(p, Icon::ChevronDown, Pos2::new(rect.right() - 12.0, rect.center().y), 12.0, theme::ICON_SECONDARY);
    if r.clicked() {
        ui.data_mut(|d| d.insert_temp(id, !open));
    }
    let mut picked = None;
    if open {
        let items: Vec<MenuItem> =
            options.iter().enumerate().map(|(i, o)| MenuItem::new(o.clone()).checked(checked.get(i).copied().unwrap_or(false))).collect();
        match popup_menu(ui.ctx(), id.with("menu"), rect.left_bottom() + Vec2::new(0.0, 2.0), rect.width(), &items, Some(rect)) {
            MenuResult::Picked(i) => {
                picked = Some(i);
                ui.data_mut(|d| d.insert_temp(id, false));
            }
            MenuResult::Closed => {
                ui.data_mut(|d| d.insert_temp(id, false));
            }
            MenuResult::Open => {}
        }
    }
    picked
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::DEVICES;

    const SCREEN: Vec2 = Vec2::new(900.0, 600.0);

    /// A popup driven frame by frame inside a window of `SCREEN` size.
    struct Popup {
        ctx: egui::Context,
        id: Id,
        pos: Pos2,
        items: Vec<MenuItem>,
        t: f64,
        picked: Option<usize>,
        /// The full index path of the last pick (`picked` is its first index).
        path: Option<Vec<usize>>,
        closed: bool,
        /// A layer above the menu, like a toast with an action, drawn over this rect on every frame.
        cover: Option<Rect>,
    }

    impl Popup {
        fn new(items: Vec<MenuItem>, pos: Pos2) -> Self {
            let ctx = egui::Context::default();
            crate::theme::install_fonts(&ctx);
            let mut p = Self { ctx, id: Id::new("test-menu"), pos, items, t: 0.0, picked: None, path: None, closed: false, cover: None };
            p.frame(vec![]);
            p.frame(vec![]);
            p
        }

        /// The device dropdown's rows: 45 labels, the last one "Apple Watch 38mm".
        fn devices(checked: Option<&str>, pos: Pos2) -> Self {
            let items = DEVICES
                .iter()
                .map(|d| {
                    let label = if d.1 > 0.0 { format!("{} ({}×{})", d.0, d.1 as i32, d.2 as i32) } else { d.0.to_string() };
                    MenuItem::new(label).checked(checked == Some(d.0))
                })
                .collect();
            Self::new(items, pos)
        }

        fn frame(&mut self, events: Vec<egui::Event>) {
            self.t += 0.05;
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)), time: Some(self.t), events, ..Default::default() };
            let (ctx, id, pos, items, cover) = (self.ctx.clone(), self.id, self.pos, &self.items, self.cover);
            let mut result = TreeResult::Open;
            let _ = self.ctx.run_ui(raw, |_| {
                result = popup_tree(&ctx, id, pos, 160.0, items, None);
                if let Some(c) = cover {
                    Area::new(Id::new("test-cover")).order(Order::Tooltip).fixed_pos(c.min).show(&ctx, |ui| {
                        ui.allocate_exact_size(c.size(), Sense::click());
                    });
                }
            });
            match result {
                TreeResult::Picked(path) => {
                    self.picked = Some(path[0]);
                    self.path = Some(path);
                }
                TreeResult::Closed => self.closed = true,
                TreeResult::Open => {}
            }
        }

        fn button(at: Pos2, pressed: bool) -> egui::Event {
            egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
        }

        fn rect(&self, name: &str) -> Rect {
            self.ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", name.to_string())))).unwrap_or_else(|| panic!("nothing recorded for {name}"))
        }

        fn panel(&self) -> Rect {
            self.rect("menu-level-0")
        }

        fn row(&self, i: usize) -> Rect {
            self.rect(&format!("menu-row-0-{i}"))
        }

        /// Where a row can be seen: inside the panel's vertical padding and inside the screen.
        fn band(&self) -> Rect {
            self.panel().shrink2(Vec2::new(0.0, theme::MENU_PADDING)).intersect(Rect::from_min_size(Pos2::ZERO, SCREEN))
        }

        fn row_is_visible(&self, i: usize) -> bool {
            let (row, band) = (self.row(i), self.band());
            row.top() >= band.top() - 0.01 && row.bottom() <= band.bottom() + 0.01
        }

        fn highlighted(&self) -> Option<usize> {
            self.ctx.data(|d| d.get_temp::<MenuState>(self.id)).and_then(|s| s.hl[0])
        }

        fn wheel(&mut self, dy: f32) {
            let at = self.panel().center();
            let notch = egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, dy),
                phase: egui::TouchPhase::Move,
                modifiers: Modifiers::NONE,
            };
            self.frame(vec![egui::Event::PointerMoved(at), notch]);
            for _ in 0..3 {
                self.frame(vec![]);
            }
        }

        fn key(&mut self, key: Key) {
            for pressed in [true, false] {
                self.frame(vec![egui::Event::Key { key, physical_key: Some(key), pressed, repeat: false, modifiers: Modifiers::NONE }]);
            }
        }

        /// A bare pass that draws nothing, like the Agents panel asking before the popups of the same frame draw: does an open
        /// popup hold the keys from it?
        fn holds_the_keys_in_the_next_pass(&mut self) -> bool {
            self.t += 0.05;
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)), time: Some(self.t), ..Default::default() };
            let mut held = false;
            let _ = self.ctx.run_ui(raw, |ui| held = crate::widgets::popup_open::open_recently(ui.ctx()));
            held
        }

        fn click(&mut self, at: Pos2) {
            self.frame(vec![egui::Event::PointerMoved(at)]);
            for pressed in [true, false] {
                self.frame(vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE }]);
            }
        }
    }

    #[test]
    fn a_menu_that_closed_in_a_frame_stops_holding_the_keys_from_the_next_frame() {
        let mut m = Popup::new(vec![MenuItem::new("One"), MenuItem::new("Two")], Pos2::new(100.0, 100.0));
        assert!(m.holds_the_keys_in_the_next_pass(), "an open menu holds the keys: without this the check below proves nothing");
        // The press frame alone: `key` spends a second frame that draws the closed menu again.
        let press = egui::Event::Key { key: Key::Escape, physical_key: Some(Key::Escape), pressed: true, repeat: false, modifiers: Modifiers::NONE };
        m.frame(vec![press]);
        assert!(m.closed, "Esc closes the menu");
        assert!(!m.holds_the_keys_in_the_next_pass(), "the menu closed in the last frame, so it must not hold the keys in this one");
    }

    #[test]
    fn a_menu_taller_than_the_screen_is_clamped_to_the_screen_height() {
        let m = Popup::devices(None, Pos2::new(700.0, 120.0));
        assert_eq!(DEVICES.len(), 45);
        let panel = m.panel();
        assert_eq!((panel.top(), panel.height()), (2.0, SCREEN.y - 4.0), "the panel fills the screen minus the 2 px edge margins, got {panel:?}");
        assert!(panel.bottom() <= SCREEN.y - 2.0);
    }

    #[test]
    fn scrolling_to_the_end_reveals_the_last_row_and_a_click_picks_it() {
        let mut m = Popup::devices(None, Pos2::new(700.0, 120.0));
        assert!(!m.row_is_visible(44), "the last row starts below the visible rows");
        for _ in 0..200 {
            if m.row_is_visible(44) {
                break;
            }
            m.wheel(-48.0);
        }
        assert!(m.row_is_visible(44), "wheeling over the menu must bring the last row into view, it is at {:?} in {:?}", m.row(44), m.panel());
        assert_eq!(m.items[44].label, "Apple Watch 38mm (136×170)");
        m.click(m.row(44).center());
        assert_eq!(m.picked, Some(44));
    }

    #[test]
    fn rows_scrolled_out_of_the_panel_cannot_be_clicked() {
        let mut m = Popup::devices(None, Pos2::new(700.0, 120.0));
        for _ in 0..200 {
            m.wheel(-48.0);
        }
        let above = m.row(0);
        assert!(above.bottom() < m.panel().top(), "row 0 scrolled out above the panel, got {above:?}");
        let band = m.band();
        m.click(Pos2::new(above.center().x, band.top() - 4.0));
        assert_eq!(m.picked, None, "a click in the panel's padding above the rows picks nothing, even where a half-hidden row's rect reaches into it");
        let half_hidden =
            (0..DEVICES.len()).find(|i| m.row(*i).top() < band.top() && m.row(*i).bottom() > band.top()).expect("a row is cut by the band's top edge");
        m.click(Pos2::new(above.center().x, band.top() + 1.0));
        assert_eq!(m.picked, Some(half_hidden), "the visible part of a half-hidden row is clickable");
    }

    #[test]
    fn arrow_keys_keep_the_highlighted_row_in_view() {
        let mut m = Popup::devices(None, Pos2::new(700.0, 120.0));
        for want in 0..DEVICES.len() {
            m.key(Key::ArrowDown);
            assert_eq!(m.highlighted(), Some(want));
            assert!(m.row_is_visible(want), "row {want} is highlighted but at {:?} outside {:?}", m.row(want), m.band());
        }
        m.key(Key::ArrowDown);
        assert_eq!(m.highlighted(), Some(0), "ArrowDown wraps to the first row");
        assert!(m.row_is_visible(0), "the wrap scrolls back to the top");
        m.key(Key::ArrowUp);
        assert_eq!(m.highlighted(), Some(44), "ArrowUp wraps to the last row");
        assert!(m.row_is_visible(44), "the wrap scrolls down to the last row");
    }

    #[test]
    fn a_tall_menu_opens_scrolled_to_its_checked_row() {
        let checked = DEVICES.iter().position(|d| d.0 == "Apple Watch 45mm").expect("device preset");
        let m = Popup::devices(Some("Apple Watch 45mm"), Pos2::new(700.0, 120.0));
        assert_eq!(m.highlighted(), Some(checked));
        assert!(m.row_is_visible(checked), "the checked row is at {:?} outside {:?}", m.row(checked), m.band());
    }

    /// The panel rect the menu code produced before levels could be capped: size from the layout, then
    /// the 2 px screen-edge clamp on both axes.
    fn uncapped_panel(m: &Popup) -> Rect {
        let lay = Layout::new(&m.ctx, &m.items, 160.0);
        let screen = Rect::from_min_size(Pos2::ZERO, SCREEN);
        let x = m.pos.x.min(screen.right() - lay.w - 2.0).max(screen.left() + 2.0);
        let y = m.pos.y.min(screen.bottom() - lay.h - 2.0).max(screen.top() + 2.0);
        Rect::from_min_size(Pos2::new(x, y), Vec2::new(lay.w, lay.h))
    }

    fn short_items() -> Vec<MenuItem> {
        vec![
            MenuItem::new("Cut").shortcut("⌘X"),
            MenuItem::new("Copy").shortcut("⌘C").sep(),
            MenuItem::new("Paste"),
            MenuItem::new("Select all").disabled(true),
            MenuItem::new("Delete"),
        ]
    }

    #[test]
    fn a_menu_that_fits_keeps_its_exact_geometry_and_ignores_the_wheel() {
        for pos in [Pos2::new(100.0, 100.0), Pos2::new(880.0, 590.0), Pos2::new(-20.0, -20.0)] {
            let mut m = Popup::new(short_items(), pos);
            let panel = m.panel();
            assert_eq!(panel, uncapped_panel(&m), "panel rect for a menu opened at {pos:?}");
            assert_eq!(panel.height(), 2.0 * theme::MENU_PADDING + 5.0 * theme::MENU_ITEM_HEIGHT + SEP_H);
            for i in 0..5 {
                let top = panel.top() + theme::MENU_PADDING + i as f32 * theme::MENU_ITEM_HEIGHT + if i >= 2 { SEP_H } else { 0.0 };
                assert_eq!(m.row(i), Rect::from_min_size(Pos2::new(panel.left(), top), Vec2::new(panel.width(), theme::MENU_ITEM_HEIGHT)), "row {i}");
            }
            let rows: Vec<Rect> = (0..5).map(|i| m.row(i)).collect();
            m.wheel(-120.0);
            assert_eq!(m.panel(), panel, "a wheel notch does not move a menu that fits");
            assert_eq!((0..5).map(|i| m.row(i)).collect::<Vec<_>>(), rows, "a wheel notch does not scroll a menu that fits");
        }
        let mut m = Popup::new(short_items(), Pos2::new(100.0, 100.0));
        m.click(m.row(2).center());
        assert_eq!(m.picked, Some(2));
        m.picked = None;
        m.click(m.row(3).center());
        assert_eq!(m.picked, None, "a disabled row stays unclickable");
    }

    #[test]
    fn a_release_over_a_row_picks_it_whatever_happened_since_the_press() {
        // Press on one row, wait 1.25 s, release 40 px away on another: far past egui's 6 px and 0.8 s click.
        let mut m = Popup::new(short_items(), Pos2::new(100.0, 100.0));
        let (from, to) = (m.row(0).center(), m.row(1).center() + Vec2::new(40.0, 0.0));
        m.frame(vec![egui::Event::PointerMoved(from), Popup::button(from, true)]);
        for _ in 0..25 {
            m.frame(vec![]);
        }
        m.frame(vec![egui::Event::PointerMoved(to), Popup::button(to, false)]);
        assert_eq!(m.picked, Some(1));
        // The same gesture inside one frame.
        let mut m = Popup::new(short_items(), Pos2::new(100.0, 100.0));
        m.frame(vec![egui::Event::PointerMoved(from), Popup::button(from, true), egui::Event::PointerMoved(to), Popup::button(to, false)]);
        assert_eq!(m.picked, Some(1));
    }

    #[test]
    fn a_release_over_a_disabled_row_or_off_the_rows_picks_nothing() {
        let mut m = Popup::new(short_items(), Pos2::new(100.0, 100.0));
        let on = m.row(0).center();
        let disabled = m.row(3).center();
        m.frame(vec![egui::Event::PointerMoved(on), Popup::button(on, true), egui::Event::PointerMoved(disabled), Popup::button(disabled, false)]);
        assert_eq!(m.picked, None, "row 3 is disabled");
        let off = Pos2::new(400.0, 400.0);
        m.frame(vec![egui::Event::PointerMoved(on), Popup::button(on, true), egui::Event::PointerMoved(off), Popup::button(off, false)]);
        assert_eq!((m.picked, m.closed), (None, false), "a press inside the menu that ends outside leaves it open");
    }

    #[test]
    fn a_release_whose_press_the_menu_never_saw_picks_nothing() {
        let mut m = Popup::new(short_items(), Pos2::new(100.0, 100.0));
        let on = m.row(1).center();
        m.frame(vec![egui::Event::PointerMoved(on), Popup::button(on, false)]);
        assert_eq!(m.picked, None);
    }

    #[test]
    fn a_press_outside_closes_the_menu_and_its_release_over_a_row_picks_nothing() {
        let mut m = Popup::new(short_items(), Pos2::new(100.0, 100.0));
        let (outside, on) = (Pos2::new(600.0, 500.0), m.row(1).center());
        m.frame(vec![egui::Event::PointerMoved(outside), Popup::button(outside, true), egui::Event::PointerMoved(on), Popup::button(on, false)]);
        assert!(m.closed);
        assert_eq!(m.picked, None);
    }

    #[test]
    fn primary_gesture_pairs_each_release_with_the_press_before_it() {
        let (a, b) = (Pos2::new(1.0, 1.0), Pos2::new(2.0, 2.0));
        let secondary = |pressed| egui::Event::PointerButton { pos: a, button: egui::PointerButton::Secondary, pressed, modifiers: Modifiers::NONE };
        let mut armed = false;
        assert_eq!(primary_gesture(&[Popup::button(b, false), secondary(false)], &mut armed, |_| false), (None, None), "no press, no pick");
        assert!(!armed);
        assert_eq!(primary_gesture(&[secondary(true), secondary(false)], &mut armed, |_| false), (Some(a), None), "another button arms nothing");
        assert!(!armed);
        assert_eq!(primary_gesture(&[Popup::button(a, true), Popup::button(b, false)], &mut armed, |_| false), (Some(a), Some(b)));
        assert!(!armed, "the release ends the press");
        assert_eq!(primary_gesture(&[Popup::button(a, true)], &mut armed, |_| false), (Some(a), None));
        assert!(armed, "the press is still down at the end of the frame");
        // A release at the start of the next frame ends that press; the press after it starts a new one.
        assert_eq!(primary_gesture(&[Popup::button(b, false), Popup::button(a, true), secondary(true)], &mut armed, |_| false), (Some(a), Some(b)));
        assert!(armed);
        // A press a higher layer took arms nothing, and a release over it is not the menu's.
        let over = |p: Pos2| p == a;
        assert_eq!(primary_gesture(&[Popup::button(a, true), Popup::button(b, false)], &mut armed, over), (Some(a), None));
        assert!(!armed);
        assert_eq!(primary_gesture(&[Popup::button(b, true), Popup::button(a, false)], &mut armed, over), (Some(b), None));
        assert!(!armed, "the release ends the press even when it lands on the higher layer");
    }

    #[test]
    fn a_secondary_press_outside_closes_the_menu() {
        let mut m = Popup::new(short_items(), Pos2::new(100.0, 100.0));
        let outside = Pos2::new(600.0, 500.0);
        m.frame(vec![egui::Event::PointerButton { pos: outside, button: egui::PointerButton::Secondary, pressed: true, modifiers: Modifiers::NONE }]);
        assert!(m.closed);
    }

    /// Two parents, each with three leaves: `A > a0..a2` and `B > b0..b2`.
    fn two_submenus() -> Vec<MenuItem> {
        let leaves = |p: &str| (0..3).map(|i| MenuItem::new(format!("{p}{i}"))).collect::<Vec<_>>();
        let parent = |name: &str, kids: Vec<MenuItem>| MenuItem { children: kids, ..MenuItem::new(name) };
        vec![parent("A", leaves("a")), parent("B", leaves("b"))]
    }

    #[test]
    fn a_release_in_a_submenu_picks_that_row_even_when_the_pointer_drifts_to_another_parent_in_the_same_frame() {
        let mut m = Popup::new(two_submenus(), Pos2::new(100.0, 100.0));
        let (a, b) = (m.row(0).center(), m.row(1).center());
        // Hover A so its submenu opens (the first move only places the pointer), press on a0, then release on a1 as
        // the pointer carries on over B.
        m.frame(vec![egui::Event::PointerMoved(a + Vec2::new(-3.0, 0.0))]);
        m.frame(vec![egui::Event::PointerMoved(a)]);
        m.frame(vec![]);
        let (a0, a1) = (m.rect("menu-row-1-0").center(), m.rect("menu-row-1-1").center());
        m.frame(vec![egui::Event::PointerMoved(a0), Popup::button(a0, true)]);
        m.frame(vec![egui::Event::PointerMoved(a1), Popup::button(a1, false), egui::Event::PointerMoved(b)]);
        assert_eq!(m.path, Some(vec![0, 1]), "the row under the release runs, not one of B's");
    }

    #[test]
    fn a_release_over_a_row_covered_by_a_higher_layer_picks_nothing() {
        let mut m = Popup::new(short_items(), Pos2::new(100.0, 100.0));
        let (row0, row2) = (m.row(0).center(), m.row(2).center());
        // A toast with an action, drawn above the menu, over row 2.
        m.cover = Some(m.row(2).shrink2(Vec2::new(-20.0, 0.0)));
        m.frame(vec![]);
        m.frame(vec![]);
        m.frame(vec![egui::Event::PointerMoved(row2), Popup::button(row2, true), Popup::button(row2, false)]);
        assert_eq!(m.picked, None, "the toast takes the click, the hidden row stays unpicked");
        // A press on the toast arms nothing: dragging off it onto a visible row picks nothing either.
        m.frame(vec![egui::Event::PointerMoved(row2), Popup::button(row2, true), egui::Event::PointerMoved(row0), Popup::button(row0, false)]);
        assert_eq!(m.picked, None);
        // Without the toast the same gesture picks row 2.
        m.cover = None;
        m.frame(vec![]);
        m.frame(vec![egui::Event::PointerMoved(row2), Popup::button(row2, true), Popup::button(row2, false)]);
        assert_eq!(m.picked, Some(2));
    }

    #[test]
    fn a_slip_across_a_row_border_picks_the_row_under_the_release() {
        // Press 2 px inside the top of Copy and release 2 px up in Cut: egui calls it a click on Copy (within 6 px),
        // but the highlight and the release are on Cut.
        let mut m = Popup::new(short_items(), Pos2::new(100.0, 100.0));
        let (border, x) = (m.row(1).top(), m.row(1).center().x);
        m.frame(vec![egui::Event::PointerMoved(Pos2::new(x, border + 2.0)), Popup::button(Pos2::new(x, border + 2.0), true)]);
        m.frame(vec![egui::Event::PointerMoved(Pos2::new(x, border - 2.0)), Popup::button(Pos2::new(x, border - 2.0), false)]);
        assert_eq!(m.picked, Some(0));
    }
}
