//! Effects section of the inspector: the "+" menu, one row per effect (type dropdown,
//! settings button, eye, minus) and the settings popovers for every effect type.

use super::properties::{open_picker, title, Pane, PickTarget};
use super::style_picker;
use crate::app::App;
use crate::icons::Icon;
use crate::theme;
use crate::widgets::field::{self, Prefix};
use crate::widgets::menu::{dropdown, dropdown_items_mixed, popup_menu, MenuItem, MenuResult};
use crate::widgets::picker::{blend_group_ends, BLEND_MODES};
use crate::widgets::{self, popover};
use designos_doc::Vec2 as DVec2;
use designos_doc::*;
use egui::{Color32, Id, Pos2, Rect, Ui, Vec2};

/// Which popover is open (shared with the stroke settings popover).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopKind {
    Effect(usize),
    Stroke,
    /// Type settings of text layers: OpenType features and paragraph settings.
    TextDetails,
}

#[derive(Clone, Debug)]
pub struct PopState {
    pub kind: PopKind,
    pub anchor: Rect,
    pub ids: Vec<NodeId>,
}

/// Menu order (matches Figma's "+" menu and the type dropdown of a row).
pub const KINDS: [(&str, Icon); 7] = [
    ("Inner shadow", Icon::InnerShadow),
    ("Drop shadow", Icon::DropShadow),
    ("Layer blur", Icon::LayerBlur),
    ("Background blur", Icon::BackgroundBlur),
    ("Noise", Icon::Noise),
    ("Texture", Icon::Texture),
    ("Glass", Icon::Glass),
];

/// Most effects of one type a layer takes through the "+" menu, as (index into [`KINDS`], limit, plural name).
/// Only the menu enforces it: imported or scripted layers may hold more and are never trimmed.
pub const EFFECT_CAPS: [(usize, usize, &str); 4] = [(0, 8, "inner shadows"), (1, 8, "drop shadows"), (2, 1, "layer blur"), (3, 1, "background blur")];

/// The cap on `kind` and what to call it, or `None` when the type is unlimited.
pub fn cap_of(kind: usize) -> Option<(usize, &'static str)> {
    EFFECT_CAPS.iter().find(|c| c.0 == kind).map(|c| (c.1, c.2))
}

/// True when `effects` already holds the most the "+" menu allows of `kind`.
pub fn at_cap(effects: &[Effect], kind: usize) -> bool {
    cap_of(kind).is_some_and(|(max, _)| effects.iter().filter(|e| kind_of(e) == kind).count() >= max)
}

pub fn kind_of(e: &Effect) -> usize {
    match e {
        Effect::InnerShadow { .. } => 0,
        Effect::DropShadow { .. } => 1,
        Effect::LayerBlur { .. } => 2,
        Effect::BackgroundBlur { .. } => 3,
        Effect::Noise(_) => 4,
        Effect::Texture(_) => 5,
        Effect::Glass(_) => 6,
        Effect::Shader { .. } => 7,
    }
}

pub fn set_visible(e: &mut Effect, v: bool) {
    match e {
        Effect::DropShadow { visible, .. }
        | Effect::InnerShadow { visible, .. }
        | Effect::LayerBlur { visible, .. }
        | Effect::BackgroundBlur { visible, .. }
        | Effect::Shader { visible, .. } => *visible = v,
        Effect::Noise(n) => n.visible = v,
        Effect::Texture(t) => t.visible = v,
        Effect::Glass(g) => g.visible = v,
    }
}

/// A fresh effect of `kind` (index into [`KINDS`]).
pub fn default_effect(kind: usize, seed: u32) -> Effect {
    let shadow = |inner: bool| {
        let (color, offset, radius, spread, visible) = (Color::rgba(0, 0, 0, 64), DVec2::new(0.0, 4.0), 4.0, 0.0, true);
        if inner {
            Effect::InnerShadow { color, offset, radius, spread, visible, blend: Default::default() }
        } else {
            Effect::DropShadow { color, offset, radius, spread, visible, show_behind: false, blend: Default::default() }
        }
    };
    match kind {
        0 => shadow(true),
        1 => shadow(false),
        2 => Effect::LayerBlur { radius: 4.0, visible: true },
        3 => Effect::BackgroundBlur { radius: 4.0, visible: true },
        4 => Effect::Noise(NoiseEffect {
            mode: NoiseMode::Mono { color: Color::BLACK },
            size: 1.0,
            size_y: None,
            density: 0.5,
            opacity: 0.25,
            blend: BlendMode::Normal,
            seed,
            visible: true,
        }),
        5 => Effect::Texture(TextureEffect { size: 2.0, size_y: None, radius: 2.0, clip_to_shape: true, seed, visible: true }),
        _ => Effect::Glass(GlassEffect {
            light_angle: 315.0,
            light_intensity: 0.5,
            refraction: 0.5,
            depth: 10.0,
            dispersion: 0.25,
            frost: 4.0,
            splay: 0.0,
            bevel: 0.0,
            visible: true,
        }),
    }
}

/// Converts `e` to another type, carrying over what the two types share (colour, offsets, radius).
pub fn convert(e: &Effect, kind: usize) -> Effect {
    if kind_of(e) == kind {
        return e.clone();
    }
    let mut out = default_effect(kind, 1);
    let visible = e.visible();
    let shared = match e {
        Effect::DropShadow { color, offset, radius, spread, .. } | Effect::InnerShadow { color, offset, radius, spread, .. } => {
            Some((*color, *offset, *radius, *spread))
        }
        Effect::LayerBlur { radius, .. } | Effect::BackgroundBlur { radius, .. } => Some((Color::rgba(0, 0, 0, 64), DVec2::new(0.0, 4.0), *radius, 0.0)),
        _ => None,
    };
    if let Some((color, offset, radius, spread)) = shared {
        match &mut out {
            Effect::DropShadow { color: c, offset: o, radius: r, spread: s, .. } | Effect::InnerShadow { color: c, offset: o, radius: r, spread: s, .. } => {
                (*c, *o, *r, *s) = (color, offset, radius, spread);
            }
            Effect::LayerBlur { radius: r, .. } | Effect::BackgroundBlur { radius: r, .. } => *r = radius,
            _ => {}
        }
    }
    if let Some(mode) = shadow_blend(e) {
        set_shadow_blend(&mut out, mode);
    }
    set_visible(&mut out, visible);
    out
}

/// "Effects" header with the "+" button (a `bg_selected` tile while its menu is open) and the menu.
fn header(app: &mut App, ui: &mut Ui, pane: &mut Pane, sel: &[NodeId], empty: bool) {
    title(ui, pane, "Effects", None, empty);
    let btn = pane.rect(theme::SECTION_TRAILING_X, -theme::SECTION_TITLE_HEIGHT + 8.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
    let open = app.props.fx_menu;
    widgets::record(ui, "fx-add", btn);
    if widgets::icon_button(ui, btn, "fx-add", Icon::Plus, open, "").clicked() {
        app.props.fx_menu = !open;
    }
    if !app.props.fx_menu {
        return;
    }
    // A type is full as soon as any selected layer holds its cap, so no layer is pushed past it.
    let full = |k: usize| sel.iter().any(|id| app.doc.get(*id).is_some_and(|n| at_cap(&n.effects, k)));
    let mut items: Vec<MenuItem> = KINDS
        .iter()
        .enumerate()
        .map(|(k, (n, i))| {
            let item = MenuItem { icon: Some(*i), ..MenuItem::new(*n) };
            match cap_of(k).filter(|_| full(k)) {
                Some((max, plural)) => item.disabled(true).tooltip(format!("Up to {max} {plural}")),
                None => item,
            }
        })
        .collect();
    if let Some(last) = items.last_mut() {
        last.separator_after = true;
    }
    items.push(MenuItem { icon: Some(Icon::Shader), disabled: true, ..MenuItem::new("Shader").shortcut("Soon") });
    let w = 168.0;
    // The menu opens below the button, lifted when its rows would reach past the inspector's visible area into the
    // footer bar under it.
    let menu_h = widgets::menu::menu_height(&items);
    let view = ui.clip_rect();
    let top = (btn.bottom() + 4.0).min(view.bottom() - menu_h - theme::SPACE_FACT).max(view.top());
    let pos = Pos2::new(btn.right() - w, top);
    match popup_menu(ui.ctx(), Id::new("fx-add-menu"), pos, w, &items, Some(btn)) {
        MenuResult::Picked(k) if k < KINDS.len() => {
            app.props.fx_menu = false;
            if full(k) {
                return;
            }
            let seed = app.doc.nodes.len() as u32 + 1;
            let e = default_effect(k, seed);
            // Adding to a style-linked layer leaves the style: the layer keeps its values and gets the new effect.
            app.edit_nodes(sel, |n| {
                n.style_refs.remove("effect");
                n.effects.push(e.clone());
            });
        }
        MenuResult::Picked(_) | MenuResult::Closed => app.props.fx_menu = false,
        MenuResult::Open => {}
    }
}

pub(super) fn section(app: &mut App, ui: &mut Ui, pane: &mut Pane, sel: &[NodeId], effects: &[Effect]) {
    let linked = style_picker::linked_style(app, sel, StyleProperty::Effect);
    header(app, ui, pane, sel, effects.is_empty() && linked.is_none());
    style_picker::header_button(app, ui, pane, sel, StyleProperty::Effect);
    if let Some(key) = linked {
        // A linked layer shows its style instead of the effects it copies.
        pane.y += 4.0;
        style_picker::linked_row(app, ui, pane, sel, StyleProperty::Effect, &key);
        pane.y += 8.0;
        return;
    }
    let names: Vec<String> = KINDS.iter().map(|(n, _)| n.to_string()).collect();
    if !effects.is_empty() {
        pane.y += 4.0;
    }
    for (i, e) in effects.iter().enumerate() {
        if matches!(e, Effect::Shader { .. }) {
            let row = pane.rect(theme::SECTION_CONTENT_X, 0.0, 159.0, theme::FIELD_HEIGHT);
            ui.painter().rect_filled(row, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
            // A field with a prefix icon: the icon is centred in the theme's prefix slot, the text starts after it.
            let icon_slot = Rect::from_min_size(row.min, Vec2::new(theme::FIELD_PREFIX_W, row.height()));
            crate::icons::paint(ui.painter(), Icon::Shader, icon_slot.center(), theme::FIELD_PREFIX_ICON, theme::TEXT_SECONDARY);
            let text_r = Rect::from_min_max(Pos2::new(row.left() + theme::FIELD_PREFIX_W, row.top()), row.max);
            widgets::field_text(ui.painter(), text_r, 0.0, 0.0, "Shader (not rendered)", theme::font_body(), theme::TEXT_SECONDARY);
        } else {
            let kind = kind_of(e);
            let dd = pane.rect(theme::SECTION_CONTENT_X, 0.0, 134.0, theme::FIELD_HEIGHT);
            if let Some(k) = dropdown(ui, dd, ("fx-kind", i), names[kind].as_str(), &names) {
                let new = convert(e, k);
                app.edit_nodes(sel, |n| {
                    if let Some(slot) = n.effects.get_mut(i) {
                        *slot = new.clone();
                    }
                });
            }
            let set_r = pane.rect(theme::RIGHT_NAME_ACTION_X[1] - 25.0, 0.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
            let open = app.props.pop.as_ref().is_some_and(|p| p.kind == PopKind::Effect(i));
            if widgets::icon_button(ui, set_r, ("fx-settings", i), Icon::StrokeSettings, open, "Effect settings").clicked() {
                app.props.pop = (!open).then(|| PopState { kind: PopKind::Effect(i), anchor: set_r, ids: sel.to_vec() });
            }
        }
        let eye_r = pane.rect(theme::RIGHT_NAME_ACTION_X[1] - 1.0 + 4.0, 0.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
        let visible = e.visible();
        if widgets::icon_button(ui, eye_r, ("fx-eye", i), if visible { Icon::Eye } else { Icon::EyeOff }, false, "").clicked() {
            app.edit_nodes(sel, |n| {
                if let Some(e) = n.effects.get_mut(i) {
                    set_visible(e, !visible);
                }
            });
        }
        let minus_r = pane.rect(theme::SECTION_TRAILING_X, 0.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
        widgets::record(ui, &format!("fx-minus-{i}"), minus_r);
        if widgets::icon_button(ui, minus_r, ("fx-minus", i), Icon::Minus, false, "").clicked() {
            app.props.pop = None;
            app.edit_nodes(sel, |n| {
                if i < n.effects.len() {
                    let old_len = n.effects.len();
                    n.effects.remove(i);
                    let origins: Vec<Option<usize>> = (0..old_len).filter(|&idx| idx != i).map(Some).collect();
                    super::var_bind::remap_indexed_bindings(&mut n.variables.bindings, "effects/", &origins);
                }
            });
        }
        pane.y += theme::FIELD_HEIGHT + 8.0;
    }
    if !effects.is_empty() {
        pane.y += 8.0;
    }
}

#[cfg(test)]
pub(crate) fn section_for_test(app: &mut App, ctx: &egui::Context, sel: &[NodeId], effects: &[Effect]) {
    let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
        let mut pane = super::properties::Pane { x0: 0.0, oy: 0.0, y: 0.0, title_end: 0.0 };
        section(app, ui, &mut pane, sel, effects);
    });
}

// ---------------------------------------------------------------- popover
/// What the effect settings rows edit: the effects of layers, or those of a shared effect style
/// (the "Edit style" popover).
#[derive(Clone, Copy)]
pub(super) enum FxTarget<'a> {
    Nodes(&'a [NodeId]),
    Style,
}

impl<'a> FxTarget<'a> {
    /// The colour picker target of a colour row; a style's colours are edited as hex only.
    fn picker(self, target: PickTarget) -> Option<(PickTarget, &'a [NodeId])> {
        match self {
            Self::Nodes(ids) => Some((target, ids)),
            Self::Style => None,
        }
    }
}

fn edit_effect(app: &mut App, tgt: FxTarget, i: usize, f: impl Fn(&mut Effect)) {
    match tgt {
        FxTarget::Nodes(ids) => app.edit_nodes(ids, |n| {
            if let Some(e) = n.effects.get_mut(i) {
                f(e);
            }
        }),
        FxTarget::Style => super::style_edit::edit(app, |s| {
            if let Style::Effect(fx) = s {
                if let Some(e) = fx.get_mut(i) {
                    f(e);
                }
            }
        }),
    }
}

fn num(ui: &mut Ui, r: Rect, key: impl widgets::Key, prefix: Prefix, v: f32, suffix: &str) -> Option<f32> {
    field::num_field(ui, r, key, prefix, Some(v), suffix)
}

fn pct(v: f32) -> f32 {
    (v * 100.0).round()
}

/// Colour row: swatch (opens the picker when `picker` names a target) and hex.
pub(super) fn color_row(app: &mut App, ui: &mut Ui, r: Rect, key: (usize, u8), c: Color, picker: Option<(PickTarget, &[NodeId])>) -> Option<Color> {
    let sw = Rect::from_min_size(Pos2::new(r.left() + 4.0, r.center().y - 8.0), Vec2::splat(theme::SWATCH_SIZE));
    let hex_r = Rect::from_min_max(Pos2::new(r.left() + 24.0, r.top()), r.right_bottom());
    let mut out = None;
    let id = ui.id().with(("fx-color", key));
    field::field_bg(ui, r, id.with("hex"));
    let c32 = Color32::from_rgb((c.r * 255.0).round() as u8, (c.g * 255.0).round() as u8, (c.b * 255.0).round() as u8);
    widgets::swatch(ui.painter(), sw, c32);
    if let Some((target, ids)) = picker {
        if ui.interact(sw.expand(2.0), id.with("sw"), egui::Sense::click()).clicked() {
            open_picker(app, target, c32, sw, ids.to_vec());
        }
    }
    if let Some(s) = field::edit_hex_text(ui, hex_r, id.with("hex"), &c.to_hex(), theme::font_body(), theme::TEXT, 0.0) {
        if let Some(nc) = Color::from_hex(&s) {
            out = Some(Color { a: c.a, ..nc });
        }
    }
    out
}

pub(super) fn popover_ui(app: &mut App, ui: &mut Ui, region: Rect) {
    let Some(st) = app.props.pop.clone() else { return };
    let ctx = ui.ctx().clone();
    match st.kind {
        PopKind::Effect(i) => {
            let Some(e) = app.doc.get(st.ids.first().copied().unwrap_or(NodeId(0))).and_then(|n| resolve_node(&app.doc, n).effects.get(i).cloned()) else {
                app.props.pop = None;
                return;
            };
            let rows = effect_rows(&e);
            let title = KINDS[kind_of(&e)].0;
            let ids = st.ids.clone();
            let pop = popover::show(&ctx, Id::new("inspector-popover"), st.anchor, region, 240.0, popover::height(rows), title, |ui, body| {
                effect_body(app, ui, body, &e, i, FxTarget::Nodes(&ids), 0);
            });
            if pop {
                app.props.pop = None;
            }
        }
        PopKind::Stroke => super::stroke::popover_ui(app, ui, region, &st),
        PopKind::TextDetails => super::typography::details_popover(app, ui, region, &st),
    }
}

/// Number of setting rows of the popover of effect `e`.
pub(crate) fn effect_rows(e: &Effect) -> usize {
    match e {
        Effect::DropShadow { .. } => 7,
        Effect::InnerShadow { .. } => 6,
        Effect::LayerBlur { .. } | Effect::BackgroundBlur { .. } => 1,
        // Mode, the colour rows, size, density and opacity: every mode draws its grain with the opacity.
        Effect::Noise(n) => {
            let colours = match n.mode {
                NoiseMode::Mono { .. } => 1,
                NoiseMode::Duo { .. } => 2,
                NoiseMode::Multi => 0,
            };
            let (mode, density_and_opacity) = (1, 2);
            mode + colours + size_rows(n.size_y).len() + density_and_opacity
        }
        // Size, radius and clip to shape.
        Effect::Texture(t) => size_rows(t.size_y).len() + 2,
        // Light angle, then the rows of [`GLASS_ROWS`].
        Effect::Glass(_) => 1 + GLASS_ROWS.len(),
        Effect::Shader { .. } => 0,
    }
}

/// Which side of a noise or texture size a field edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SizeAxis {
    /// The one "Size" row of a size whose two sides agree.
    Both,
    X,
    Y,
}

/// Smallest size a field sets, in px.
const MIN_SIZE: f32 = 0.1;

/// The size rows of a noise or texture: one "Size" row while no second size is stored, "Size X" and "Size Y"
/// while one is, so a stored value is never hidden. Rows follow what is stored, so a scrub that carries the two
/// sides through the same value keeps both rows (see [`scrubbed_size`]).
fn size_rows(size_y: Option<f32>) -> &'static [(&'static str, SizeAxis)] {
    if size_y.is_some() {
        &[("Size X", SizeAxis::X), ("Size Y", SizeAxis::Y)]
    } else {
        &[("Size", SizeAxis::Both)]
    }
}

/// `(size, size_y)` after `axis` was set to `v` by a typed value or a wheel step. The second size is kept only
/// while it differs from the first, so two sides that agree fall back to one size at once.
fn edited_size(size: f32, size_y: Option<f32>, axis: SizeAxis, v: f32) -> (f32, Option<f32>) {
    let v = v.max(MIN_SIZE);
    match axis {
        SizeAxis::Both => (v, None),
        SizeAxis::X => (v, size_y.filter(|y| *y != v)),
        SizeAxis::Y => (size, Some(v).filter(|y| *y != size)),
    }
}

/// `(size, size_y)` after one sample of a scrub on `axis`. The sides are written as they are, even when they
/// agree: the rows stay where they are for the pressed field to keep its drag, and the other side is not lost
/// when the dragged one passes through its value. A scrub that ends with the sides equal leaves both rows until
/// the next typed value or wheel step, so one scrub is one undo step.
fn scrubbed_size(size: f32, size_y: Option<f32>, axis: SizeAxis, v: f32) -> (f32, Option<f32>) {
    let v = v.max(MIN_SIZE);
    match axis {
        SizeAxis::Both => (v, None),
        SizeAxis::X => (v, size_y),
        SizeAxis::Y => (size, Some(v)),
    }
}

/// True while a numeric field of the window is being scrubbed (see [`field::SCRUB_FLAG`]).
fn scrubbing(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(Id::new(field::SCRUB_FLAG))).unwrap_or(false)
}

/// The size and the second size of a noise or texture.
fn size_pair(e: &mut Effect) -> Option<(&mut f32, &mut Option<f32>)> {
    match e {
        Effect::Noise(n) => Some((&mut n.size, &mut n.size_y)),
        Effect::Texture(t) => Some((&mut t.size, &mut t.size_y)),
        _ => None,
    }
}

/// Whose size rows these are: effect `index` of `tgt`, a texture's or a noise's.
#[derive(Clone, Copy)]
struct SizeTarget<'a> {
    tgt: FxTarget<'a>,
    index: usize,
    texture: bool,
}

impl SizeTarget<'_> {
    /// Field key of the row that edits `axis`.
    fn key(self, axis: SizeAxis) -> (&'static str, usize) {
        let name = match (self.texture, axis == SizeAxis::Y) {
            (false, false) => "fxp-nsize",
            (false, true) => "fxp-nsize-y",
            (true, false) => "fxp-tsize",
            (true, true) => "fxp-tsize-y",
        };
        (name, self.index)
    }
}

/// The size rows of a noise or texture, one popover row each. A scrub samples through [`scrubbed_size`], so it never
/// folds the two sides, and a typed value or a wheel step goes through [`edited_size`], which folds them at once
/// when they become equal.
fn size_rows_ui(app: &mut App, ui: &mut Ui, body: Rect, t: SizeTarget, (size, size_y): (f32, Option<f32>), next: &mut impl FnMut() -> usize) {
    for (label, axis) in size_rows(size_y) {
        let (axis, value) = (*axis, if *axis == SizeAxis::Y { size_y.unwrap_or(size) } else { size });
        popover::row(ui, body, next(), label, |ui, rc| {
            let Some(v) = num(ui, rc, t.key(axis), Prefix::Icon(size_row_icon(axis)), value, "") else { return };
            let scrubbed = scrubbing(ui.ctx());
            edit_effect(app, t.tgt, t.index, |e| {
                if let Some((size, size_y)) = size_pair(e) {
                    (*size, *size_y) = if scrubbed { scrubbed_size(*size, *size_y, axis, v) } else { edited_size(*size, *size_y, axis, v) };
                }
            });
        });
    }
}

/// The icon, key and prefix of a size row; the two sides of a split size get the two direction icons.
fn size_row_icon(axis: SizeAxis) -> Icon {
    match axis {
        SizeAxis::Both => Icon::Scale,
        SizeAxis::X => Icon::DirHorizontal,
        SizeAxis::Y => Icon::DirVertical,
    }
}

/// The first glass row, a dial with a number field.
const GLASS_ANGLE_LABEL: &str = "Light angle";

/// The glass rows under "Light angle", in popover order: label, icon, unit suffix and which value the row
/// edits (see [`glass_value`]). The value index of a row is its field key, so adding a row never renames the others.
const GLASS_ROWS: [(&str, Icon, &str, u8); 7] = [
    ("Light intensity", Icon::Sparkle, "%", 0),
    ("Refraction", Icon::Glass, "%", 1),
    ("Depth", Icon::DropShadow, "", 2),
    ("Dispersion", Icon::PaintLinear, "%", 3),
    ("Splay", Icon::Scale, "%", 5),
    ("Bevel", Icon::Radius, "", 6),
    ("Frost", Icon::BackgroundBlur, "", 4),
];

/// What the glass row `which` shows: 0..1 values as percent, lengths in px.
fn glass_value(g: &GlassEffect, which: u8) -> f32 {
    match which {
        0 => pct(g.light_intensity),
        1 => pct(g.refraction),
        2 => g.depth,
        3 => pct(g.dispersion),
        4 => g.frost,
        5 => pct(g.splay),
        _ => g.bevel,
    }
}

/// Sets the glass value `which` from the field value `v` (percent for 0..1 values).
fn set_glass_value(g: &mut GlassEffect, which: u8, v: f32) {
    let unit = (v / 100.0).clamp(0.0, 1.0);
    match which {
        0 => g.light_intensity = unit,
        1 => g.refraction = unit,
        2 => g.depth = v.max(0.0),
        3 => g.dispersion = unit,
        4 => g.frost = v.max(0.0),
        5 => g.splay = unit,
        _ => g.bevel = v.max(0.0),
    }
}

/// The setting rows of effect `e` (index `i`), starting at popover row `first_row`.
pub(super) fn effect_body(app: &mut App, ui: &mut Ui, body: Rect, e: &Effect, i: usize, tgt: FxTarget, first_row: usize) {
    let mut r = first_row;
    let mut next = || {
        r += 1;
        r - 1
    };
    let style_key = match tgt {
        FxTarget::Style => app.props.style_edit.as_ref().map(|s| s.key.clone()),
        FxTarget::Nodes(_) => None,
    };
    let shadow = match e {
        Effect::DropShadow { color, offset, radius, spread, show_behind, .. } => Some((*color, *offset, *radius, *spread, Some(*show_behind))),
        Effect::InnerShadow { color, offset, radius, spread, .. } => Some((*color, *offset, *radius, *spread, None)),
        _ => None,
    };
    if let Some((color, offset, radius, spread, behind)) = shadow {
        let x_prop = format!("effects/{i}/offsetX");
        popover::row(ui, body, next(), "X", |ui, rc| {
            let v = match (&style_key, tgt) {
                (Some(sk), _) => {
                    super::var_bind::style_numeric(app, ui, rc, sk, &x_prop, |ui, r| num(ui, r, ("fxp-x", i), Prefix::Icon(Icon::DirHorizontal), offset.x, ""))
                }
                (None, FxTarget::Nodes(ids)) => {
                    super::var_bind::numeric(app, ui, rc, ids, &[&x_prop], |ui, r| num(ui, r, ("fxp-x", i), Prefix::Icon(Icon::DirHorizontal), offset.x, ""))
                }
                (None, FxTarget::Style) => num(ui, rc, ("fxp-x", i), Prefix::Icon(Icon::DirHorizontal), offset.x, ""),
            };
            if let Some(v) = v {
                edit_effect(app, tgt, i, |e| set_shadow(e, |_, o, _, _| o.x = v));
            }
        });
        let y_prop = format!("effects/{i}/offsetY");
        popover::row(ui, body, next(), "Y", |ui, rc| {
            let v = match (&style_key, tgt) {
                (Some(sk), _) => {
                    super::var_bind::style_numeric(app, ui, rc, sk, &y_prop, |ui, r| num(ui, r, ("fxp-y", i), Prefix::Icon(Icon::DirVertical), offset.y, ""))
                }
                (None, FxTarget::Nodes(ids)) => {
                    super::var_bind::numeric(app, ui, rc, ids, &[&y_prop], |ui, r| num(ui, r, ("fxp-y", i), Prefix::Icon(Icon::DirVertical), offset.y, ""))
                }
                (None, FxTarget::Style) => num(ui, rc, ("fxp-y", i), Prefix::Icon(Icon::DirVertical), offset.y, ""),
            };
            if let Some(v) = v {
                edit_effect(app, tgt, i, |e| set_shadow(e, |_, o, _, _| o.y = v));
            }
        });
        let r_prop = format!("effects/{i}/radius");
        popover::row(ui, body, next(), "Blur", |ui, rc| {
            let v = match (&style_key, tgt) {
                (Some(sk), _) => {
                    super::var_bind::style_numeric(app, ui, rc, sk, &r_prop, |ui, r| num(ui, r, ("fxp-b", i), Prefix::Icon(Icon::LayerBlur), radius, ""))
                }
                (None, FxTarget::Nodes(ids)) => {
                    super::var_bind::numeric(app, ui, rc, ids, &[&r_prop], |ui, r| num(ui, r, ("fxp-b", i), Prefix::Icon(Icon::LayerBlur), radius, ""))
                }
                (None, FxTarget::Style) => num(ui, rc, ("fxp-b", i), Prefix::Icon(Icon::LayerBlur), radius, ""),
            };
            if let Some(v) = v {
                edit_effect(app, tgt, i, |e| set_shadow(e, |_, _, rr, _| *rr = v.max(0.0)));
            }
        });
        let s_prop = format!("effects/{i}/spread");
        popover::row(ui, body, next(), "Spread", |ui, rc| {
            let v = match (&style_key, tgt) {
                (Some(sk), _) => {
                    super::var_bind::style_numeric(app, ui, rc, sk, &s_prop, |ui, r| num(ui, r, ("fxp-s", i), Prefix::Icon(Icon::Scale), spread, ""))
                }
                (None, FxTarget::Nodes(ids)) => {
                    super::var_bind::numeric(app, ui, rc, ids, &[&s_prop], |ui, r| num(ui, r, ("fxp-s", i), Prefix::Icon(Icon::Scale), spread, ""))
                }
                (None, FxTarget::Style) => num(ui, rc, ("fxp-s", i), Prefix::Icon(Icon::Scale), spread, ""),
            };
            if let Some(v) = v {
                edit_effect(app, tgt, i, |e| set_shadow(e, |_, _, _, sp| *sp = v));
            }
        });
        let c_prop = format!("effects/{i}/color");
        popover::row(ui, body, next(), "Color", |ui, rc| match (&style_key, tgt) {
            (Some(sk), _) => {
                if !super::var_bind::style_pill_field(app, ui, rc, sk, &c_prop) {
                    let val_r = super::var_bind::style_value_rect(app, ui, rc, sk, &c_prop, VariableKind::Color);
                    if let Some(nc) = color_row(app, ui, val_r, (i, 0), color, tgt.picker(PickTarget::Shadow(i))) {
                        edit_effect(app, tgt, i, |e| set_shadow(e, |c, _, _, _| *c = nc));
                    }
                    super::var_bind::style_hover_button(app, ui, rc, sk, &c_prop, VariableKind::Color);
                }
            }
            (None, FxTarget::Nodes(ids)) => {
                if !super::var_bind::pill_field(app, ui, rc, ids, &[&c_prop]) {
                    let val_r = super::var_bind::value_rect(app, ui, rc, ids, &[&c_prop], VariableKind::Color);
                    if let Some(nc) = color_row(app, ui, val_r, (i, 0), color, tgt.picker(PickTarget::Shadow(i))) {
                        edit_effect(app, tgt, i, |e| set_shadow(e, |c, _, _, _| *c = nc));
                    }
                    super::var_bind::hover_button(app, ui, rc, ids, &[&c_prop], VariableKind::Color);
                }
            }
            (None, FxTarget::Style) => {
                if let Some(nc) = color_row(app, ui, rc, (i, 0), color, tgt.picker(PickTarget::Shadow(i))) {
                    edit_effect(app, tgt, i, |e| set_shadow(e, |c, _, _, _| *c = nc));
                }
            }
        });
        popover::row(ui, body, next(), "Blend", |ui, rc| {
            let blends: Vec<BlendMode> = match tgt {
                FxTarget::Nodes(ids) => ids.iter().filter_map(|id| app.doc.get(*id)).filter_map(|n| n.effects.get(i)).filter_map(shadow_blend).collect(),
                FxTarget::Style => shadow_blend(e).into_iter().collect(),
            };
            let (label, current) = blend_label(&blends);
            let blends_differ = blends.first().is_some_and(|f| blends.iter().any(|b| b != f));
            let modes = shadow_blend_modes();
            if let Some(k) = dropdown_items_mixed(ui, rc, ("fxp-blend", i), &label, &shadow_blend_items(&modes, current), BLEND_MENU_WIDTH, blends_differ) {
                let mode = modes[k].0;
                edit_effect(app, tgt, i, |e| set_shadow_blend(e, mode));
            }
        });
        if let Some(b) = behind {
            popover::row(ui, body, next(), "Show behind", |ui, rc| {
                if let Some(v) = popover::check(ui, rc, ("fxp-behind", i), b) {
                    edit_effect(app, tgt, i, |e| {
                        if let Effect::DropShadow { show_behind, .. } = e {
                            *show_behind = v;
                        }
                    });
                }
            });
        }
        return;
    }
    match e {
        Effect::LayerBlur { radius, .. } | Effect::BackgroundBlur { radius, .. } => {
            let r_prop = format!("effects/{i}/radius");
            popover::row(ui, body, next(), "Blur", |ui, rc| {
                let v = match (&style_key, tgt) {
                    (Some(sk), _) => super::var_bind::style_numeric(app, ui, rc, sk, &r_prop, |ui, r| {
                        num(ui, r, ("fxp-blur", i), Prefix::Icon(Icon::LayerBlur), *radius, "")
                    }),
                    (None, FxTarget::Nodes(ids)) => {
                        super::var_bind::numeric(app, ui, rc, ids, &[&r_prop], |ui, r| num(ui, r, ("fxp-blur", i), Prefix::Icon(Icon::LayerBlur), *radius, ""))
                    }
                    (None, FxTarget::Style) => num(ui, rc, ("fxp-blur", i), Prefix::Icon(Icon::LayerBlur), *radius, ""),
                };
                if let Some(v) = v {
                    edit_effect(app, tgt, i, |e| {
                        if let Effect::LayerBlur { radius, .. } | Effect::BackgroundBlur { radius, .. } = e {
                            *radius = v.max(0.0);
                        }
                    });
                }
            });
        }
        Effect::Noise(n) => {
            let modes = ["Monotone", "Duotone", "Multitone"].map(String::from);
            let cur = match n.mode {
                NoiseMode::Mono { .. } => 0,
                NoiseMode::Duo { .. } => 1,
                NoiseMode::Multi => 2,
            };
            popover::row(ui, body, next(), "Mode", |ui, rc| {
                if let Some(k) = dropdown(ui, rc, ("fxp-mode", i), &modes[cur], &modes) {
                    edit_effect(app, tgt, i, |e| {
                        if let Effect::Noise(n) = e {
                            let (a, b) = match &n.mode {
                                NoiseMode::Mono { color } => (*color, Color::WHITE),
                                NoiseMode::Duo { color, secondary } => (*color, *secondary),
                                NoiseMode::Multi => (Color::BLACK, Color::WHITE),
                            };
                            n.mode = match k {
                                0 => NoiseMode::Mono { color: a },
                                1 => NoiseMode::Duo { color: a, secondary: b },
                                _ => NoiseMode::Multi,
                            };
                        }
                    });
                }
            });
            let (c1, c2) = match &n.mode {
                NoiseMode::Mono { color } => (Some(*color), None),
                NoiseMode::Duo { color, secondary } => (Some(*color), Some(*secondary)),
                NoiseMode::Multi => (None, None),
            };
            if let Some(c) = c1 {
                popover::row(ui, body, next(), if c2.is_some() { "Color 1" } else { "Color" }, |ui, rc| {
                    if let Some(nc) = color_row(app, ui, rc, (i, 1), c, tgt.picker(PickTarget::NoiseColor(i, false))) {
                        edit_effect(app, tgt, i, |e| set_noise_color(e, false, nc));
                    }
                });
            }
            if let Some(c) = c2 {
                popover::row(ui, body, next(), "Color 2", |ui, rc| {
                    if let Some(nc) = color_row(app, ui, rc, (i, 2), c, tgt.picker(PickTarget::NoiseColor(i, true))) {
                        edit_effect(app, tgt, i, |e| set_noise_color(e, true, nc));
                    }
                });
            }
            size_rows_ui(app, ui, body, SizeTarget { tgt, index: i, texture: false }, (n.size, n.size_y), &mut next);
            popover::row(ui, body, next(), "Density", |ui, rc| {
                if let Some(v) = num(ui, rc, ("fxp-dens", i), Prefix::Icon(Icon::Noise), pct(n.density), "%") {
                    edit_effect(app, tgt, i, |e| {
                        if let Effect::Noise(n) = e {
                            n.density = (v / 100.0).clamp(0.0, 1.0);
                        }
                    });
                }
            });
            popover::row(ui, body, next(), "Opacity", |ui, rc| {
                if let Some(v) = num(ui, rc, ("fxp-nop", i), Prefix::Icon(Icon::Opacity), pct(n.opacity), "%") {
                    edit_effect(app, tgt, i, |e| {
                        if let Effect::Noise(n) = e {
                            n.opacity = (v / 100.0).clamp(0.0, 1.0);
                        }
                    });
                }
            });
        }
        Effect::Texture(t) => {
            size_rows_ui(app, ui, body, SizeTarget { tgt, index: i, texture: true }, (t.size, t.size_y), &mut next);
            popover::row(ui, body, next(), "Radius", |ui, rc| {
                if let Some(v) = num(ui, rc, ("fxp-trad", i), Prefix::Icon(Icon::Radius), t.radius, "") {
                    edit_effect(app, tgt, i, |e| {
                        if let Effect::Texture(t) = e {
                            t.radius = v.max(0.0);
                        }
                    });
                }
            });
            popover::row(ui, body, next(), "Clip to shape", |ui, rc| {
                if let Some(v) = popover::check(ui, rc, ("fxp-clip", i), t.clip_to_shape) {
                    edit_effect(app, tgt, i, |e| {
                        if let Effect::Texture(t) = e {
                            t.clip_to_shape = v;
                        }
                    });
                }
            });
        }
        Effect::Glass(g) => {
            popover::row(ui, body, next(), GLASS_ANGLE_LABEL, |ui, rc| {
                let dial_r = Rect::from_min_size(rc.min, Vec2::splat(24.0));
                let num_r = Rect::from_min_max(Pos2::new(rc.left() + 32.0, rc.top()), rc.right_bottom());
                let mut v = popover::dial(ui, dial_r, ("fxp-dial", i), g.light_angle);
                let angle_prop = format!("effects/{i}/lightAngle");
                let n = match (&style_key, tgt) {
                    (Some(sk), _) => super::var_bind::style_numeric(app, ui, num_r, sk, &angle_prop, |ui, r| {
                        field::num_field(ui, r, ("fxp-angle", i), Prefix::Icon(Icon::Rotation), Some(g.light_angle), "°")
                    }),
                    (None, FxTarget::Nodes(ids)) => super::var_bind::numeric(app, ui, num_r, ids, &[angle_prop.as_str()], |ui, r| {
                        field::num_field(ui, r, ("fxp-angle", i), Prefix::Icon(Icon::Rotation), Some(g.light_angle), "°")
                    }),
                    (None, FxTarget::Style) => field::num_field(ui, num_r, ("fxp-angle", i), Prefix::Icon(Icon::Rotation), Some(g.light_angle), "°"),
                };
                if let Some(n) = n {
                    v = Some(n.rem_euclid(360.0));
                }
                if let Some(v) = v {
                    edit_effect(app, tgt, i, |e| {
                        if let Effect::Glass(g) = e {
                            g.light_angle = v;
                        }
                    });
                }
            });
            for (label, icon, suffix, which) in GLASS_ROWS {
                popover::row(ui, body, next(), label, |ui, rc| {
                    let prop = match which {
                        0 => Some(format!("effects/{i}/lightIntensity")),
                        1 => Some(format!("effects/{i}/refraction")),
                        2 => Some(format!("effects/{i}/depth")),
                        3 => Some(format!("effects/{i}/dispersion")),
                        4 => Some(format!("effects/{i}/radius")),
                        _ => None,
                    };
                    let v = match (prop, &style_key, tgt) {
                        (Some(ref p), Some(sk), _) => super::var_bind::style_numeric(app, ui, rc, sk, p, |ui, r| {
                            num(ui, r, ("fxp-glass", i, which), Prefix::Icon(icon), glass_value(g, which), suffix)
                        }),
                        (Some(ref p), None, FxTarget::Nodes(ids)) if which == 4 => {
                            let frost_prop = format!("effects/{i}/frost");
                            super::var_bind::numeric(app, ui, rc, ids, &[p.as_str(), frost_prop.as_str()], |ui, r| {
                                num(ui, r, ("fxp-glass", i, which), Prefix::Icon(icon), glass_value(g, which), suffix)
                            })
                        }
                        (Some(ref p), None, FxTarget::Nodes(ids)) => super::var_bind::numeric(app, ui, rc, ids, &[p.as_str()], |ui, r| {
                            num(ui, r, ("fxp-glass", i, which), Prefix::Icon(icon), glass_value(g, which), suffix)
                        }),
                        _ => num(ui, rc, ("fxp-glass", i, which), Prefix::Icon(icon), glass_value(g, which), suffix),
                    };
                    if let Some(v) = v {
                        edit_effect(app, tgt, i, |e| {
                            if let Effect::Glass(g) = e {
                                set_glass_value(g, which, v);
                            }
                        });
                    }
                });
            }
        }
        _ => {}
    }
}

/// The blend mode of a shadow effect; other effect types have none.
fn shadow_blend(e: &Effect) -> Option<BlendMode> {
    match e {
        Effect::DropShadow { blend, .. } | Effect::InnerShadow { blend, .. } => Some(*blend),
        _ => None,
    }
}

fn set_shadow_blend(e: &mut Effect, mode: BlendMode) {
    if let Effect::DropShadow { blend, .. } | Effect::InnerShadow { blend, .. } = e {
        *blend = mode;
    }
}

/// The dropdown label for the blend modes of the selected shadows, and the mode they share (if any).
/// Modes outside the menu (imported linear burn / dodge, pass through) show their own name.
fn blend_label(blends: &[BlendMode]) -> (String, Option<BlendMode>) {
    let Some(&first) = blends.first() else { return ("Normal".into(), Some(BlendMode::Normal)) };
    if blends.iter().any(|b| *b != first) {
        return ("Mixed".into(), None);
    }
    let name = match first {
        // not offered for shadows, but an imported value still shows a readable name
        BlendMode::PassThrough => "Pass through".to_string(),
        _ => BLEND_MODES.iter().find(|(m, _)| *m == first).map_or_else(|| format!("{first:?}"), |(_, n)| n.to_string()),
    };
    (name, Some(first))
}

pub fn set_noise_color(e: &mut Effect, secondary: bool, c: Color) {
    if let Effect::Noise(n) = e {
        match (&mut n.mode, secondary) {
            (NoiseMode::Mono { color }, false) | (NoiseMode::Duo { color, .. }, false) => *color = Color { a: 1.0, ..c },
            (NoiseMode::Duo { secondary, .. }, true) => *secondary = Color { a: 1.0, ..c },
            _ => {}
        }
    }
}

pub fn set_shadow(e: &mut Effect, f: impl FnOnce(&mut Color, &mut DVec2, &mut f32, &mut f32)) {
    match e {
        Effect::DropShadow { color, offset, radius, spread, .. } | Effect::InnerShadow { color, offset, radius, spread, .. } => {
            f(color, offset, radius, spread)
        }
        _ => {}
    }
}

/// Blend modes offered for a shadow. Figma effects have no pass-through, so it is not
/// offered; an imported `PassThrough` is still kept and shown, it just can't be picked.
fn shadow_blend_modes() -> Vec<(BlendMode, &'static str)> {
    BLEND_MODES.iter().copied().filter(|(m, _)| *m != BlendMode::PassThrough).collect()
}

/// Same width as the paint blend menu.
const BLEND_MENU_WIDTH: f32 = 140.0;

/// The shadow Blend menu: one checkable row per mode, with Figma's group dividers.
fn shadow_blend_items(modes: &[(BlendMode, &'static str)], current: Option<BlendMode>) -> Vec<MenuItem> {
    modes
        .iter()
        .map(|(m, n)| {
            let item = MenuItem::new(*n).checked(current == Some(*m));
            if blend_group_ends(*m) {
                item.sep()
            } else {
                item
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn shadow_blend_menu_has_figma_groups_and_linear_modes() {
        use designos_doc::BlendMode as B;
        let modes = super::shadow_blend_modes();
        let items = super::shadow_blend_items(&modes, Some(B::LinearBurn));
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"Linear burn") && labels.contains(&"Linear dodge"), "{labels:?}");
        let groups: Vec<Vec<&str>> = items.split_inclusive(|i| i.separator_after).map(|g| g.iter().map(|i| i.label.as_str()).collect()).collect();
        assert_eq!(
            groups,
            vec![
                vec!["Normal"],
                vec!["Darken", "Multiply", "Linear burn", "Color burn"],
                vec!["Lighten", "Screen", "Linear dodge", "Color dodge"],
                vec!["Overlay", "Soft light", "Hard light"],
                vec!["Difference", "Exclusion"],
                vec!["Hue", "Saturation", "Color", "Luminosity"],
            ]
        );
        assert!(!items.last().unwrap().separator_after, "no divider after the last row");
        assert!(items.iter().filter(|i| i.checked).map(|i| i.label.as_str()).eq(["Linear burn"]), "imported Linear burn is checked by name");
        assert_eq!(super::BLEND_MENU_WIDTH, 140.0);
    }

    #[test]
    fn shadow_blend_row_hides_pass_through() {
        let modes = super::shadow_blend_modes();
        assert!(modes.iter().all(|(m, _)| *m != designos_doc::BlendMode::PassThrough));
        let others = super::BLEND_MODES.iter().filter(|(m, _)| *m != designos_doc::BlendMode::PassThrough).count();
        assert_eq!(modes.len(), others, "only pass-through is hidden");
        assert_eq!(super::blend_label(&[designos_doc::BlendMode::PassThrough]).0, "Pass through", "an imported value still shows");
    }

    use super::*;

    #[test]
    fn menu_order_and_kinds_agree() {
        for (k, _) in KINDS.iter().enumerate() {
            assert_eq!(kind_of(&default_effect(k, 7)), k);
        }
        assert_eq!(KINDS[0].0, "Inner shadow");
        assert_eq!(KINDS[6].0, "Glass");
    }

    #[test]
    fn convert_between_shadows_keeps_the_blend_mode() {
        let mut drop = default_effect(1, 1);
        set_shadow_blend(&mut drop, BlendMode::Multiply);
        assert_eq!(shadow_blend(&convert(&drop, 0)), Some(BlendMode::Multiply));
    }

    #[test]
    fn blend_label_is_mixed_when_the_selection_differs() {
        assert_eq!(blend_label(&[BlendMode::LinearBurn]).0, "Linear burn");
        assert_eq!(blend_label(&[BlendMode::LinearDodge]).0, "Linear dodge");
        assert_eq!(blend_label(&[BlendMode::PassThrough]), ("Pass through".to_string(), Some(BlendMode::PassThrough)));
        assert_eq!(blend_label(&[BlendMode::Multiply, BlendMode::Multiply]), ("Multiply".to_string(), Some(BlendMode::Multiply)));
        assert_eq!(blend_label(&[BlendMode::Multiply, BlendMode::Screen]), ("Mixed".to_string(), None));
        assert_eq!(blend_label(&[]).0, "Normal");
    }

    fn fx(json: serde_json::Value) -> Effect {
        serde_json::from_value(json).expect("an effect")
    }

    fn noise_with(mode: serde_json::Value, size_y: Option<f32>) -> Effect {
        let mut n = serde_json::json!({"mode": mode, "size": 3.0, "density": 0.5, "opacity": 0.5, "blend": "Normal", "seed": 1, "visible": true});
        if let Some(y) = size_y {
            n["size_y"] = serde_json::json!(y);
        }
        fx(serde_json::json!({"Noise": n}))
    }

    fn texture_with(size_y: Option<f32>) -> Effect {
        let mut t = serde_json::json!({"size": 3.0, "radius": 4.0, "clip_to_shape": true, "seed": 1, "visible": true});
        if let Some(y) = size_y {
            t["size_y"] = serde_json::json!(y);
        }
        fx(serde_json::json!({"Texture": t}))
    }

    #[test]
    fn popover_row_counts_follow_the_rows_shown() {
        let mono = serde_json::json!({"Mono": {"color": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 1.0}}});
        let duo = serde_json::json!({"Duo": {"color": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 1.0}, "secondary": {"r": 1.0, "g": 1.0, "b": 1.0, "a": 1.0}}});
        assert_eq!(effect_rows(&default_effect(6, 1)), 8, "glass");
        assert_eq!(effect_rows(&noise_with(mono.clone(), None)), 5, "mono shows the opacity it is drawn with");
        assert_eq!(effect_rows(&noise_with(duo, None)), 6, "duo shows the opacity too");
        assert_eq!(effect_rows(&noise_with(serde_json::json!("Multi"), None)), 4, "multitone has opacity and no colour rows");
        assert_eq!(effect_rows(&noise_with(mono, Some(7.0))), 6, "a second size adds one row");
        assert_eq!(effect_rows(&texture_with(None)), 3);
        assert_eq!(effect_rows(&texture_with(Some(1.5))), 4);
    }

    #[test]
    fn the_second_size_is_kept_only_while_it_differs() {
        // one row edits both sizes
        assert_eq!(edited_size(3.0, None, SizeAxis::Both, 5.0), (5.0, None));
        assert_eq!(edited_size(3.0, Some(7.0), SizeAxis::Both, 5.0), (5.0, None));
        // X and Y rows edit one side each and fall back to a single size once they agree
        assert_eq!(edited_size(3.0, Some(7.0), SizeAxis::X, 4.0), (4.0, Some(7.0)));
        assert_eq!(edited_size(3.0, Some(7.0), SizeAxis::Y, 2.0), (3.0, Some(2.0)));
        assert_eq!(edited_size(3.0, Some(7.0), SizeAxis::X, 7.0), (7.0, None));
        assert_eq!(edited_size(3.0, Some(7.0), SizeAxis::Y, 3.0), (3.0, None));
        // a size is never zero or negative
        assert_eq!(edited_size(3.0, None, SizeAxis::Both, 0.0), (0.1, None));
    }

    #[test]
    fn a_scrubbed_size_keeps_the_other_side_even_where_the_two_meet() {
        // X passes through Y's value and on
        assert_eq!(scrubbed_size(3.0, Some(7.0), SizeAxis::X, 7.0), (7.0, Some(7.0)));
        assert_eq!(scrubbed_size(7.0, Some(7.0), SizeAxis::X, 10.0), (10.0, Some(7.0)));
        // Y passes through X's value and on
        assert_eq!(scrubbed_size(3.0, Some(7.0), SizeAxis::Y, 3.0), (3.0, Some(3.0)));
        assert_eq!(scrubbed_size(3.0, Some(3.0), SizeAxis::Y, 1.0), (3.0, Some(1.0)));
        // the one row still writes one size, and no side goes below the minimum
        assert_eq!(scrubbed_size(3.0, None, SizeAxis::Both, 5.0), (5.0, None));
        assert_eq!(scrubbed_size(3.0, Some(7.0), SizeAxis::Y, -2.0), (3.0, Some(0.1)));
    }

    #[test]
    fn size_rows_follow_whether_a_second_size_is_stored() {
        assert_eq!(size_rows(None), [("Size", SizeAxis::Both)]);
        assert_eq!(size_rows(Some(7.0)), [("Size X", SizeAxis::X), ("Size Y", SizeAxis::Y)]);
    }

    #[test]
    fn every_glass_row_reads_and_writes_its_own_field() {
        let Effect::Glass(mut g) = default_effect(6, 1) else { unreachable!("kind 6 is glass") };
        let mut keys: Vec<u8> = GLASS_ROWS.iter().map(|r| r.3).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), GLASS_ROWS.len(), "each row edits a different value");
        for which in keys {
            set_glass_value(&mut g, which, 37.0);
            assert_eq!(glass_value(&g, which), 37.0, "row {which}");
        }
        // 0..1 values stop at the ends, lengths at zero
        set_glass_value(&mut g, 5, 250.0);
        assert_eq!(g.splay, 1.0);
        set_glass_value(&mut g, 5, -5.0);
        assert_eq!(g.splay, 0.0);
        set_glass_value(&mut g, 6, -3.0);
        assert_eq!(g.bevel, 0.0);
        set_glass_value(&mut g, 6, 3.5);
        assert_eq!(g.bevel, 3.5);
    }

    #[test]
    fn convert_keeps_shared_values_and_visibility() {
        let e = Effect::DropShadow {
            color: Color::rgba(1, 2, 3, 40),
            offset: DVec2::new(2.0, 3.0),
            radius: 9.0,
            spread: 1.0,
            visible: false,
            show_behind: false,
            blend: Default::default(),
        };
        let inner = convert(&e, 0);
        assert!(matches!(inner, Effect::InnerShadow { radius, visible: false, .. } if radius == 9.0));
        let blur = convert(&e, 2);
        assert!(matches!(blur, Effect::LayerBlur { radius, visible: false } if radius == 9.0));
        assert!(matches!(convert(&e, 6), Effect::Glass(g) if !g.visible));
    }

    #[test]
    fn shader_inspector_rows_render_label_and_toggle_visibility() {
        let raw_payload = serde_json::json!({
            "radius": 12.0,
            "backgroundShader": {
                "guid": {"sessionID": 10, "localID": 25},
                "assetRef": {"key": "shader_key_abc", "version": "1"}
            }
        });
        let mut eff = Effect::Shader { shader: designos_doc::ShaderRef { raw: raw_payload.clone() }, visible: true };
        assert_eq!(kind_of(&eff), 7);
        assert_eq!(effect_rows(&eff), 0);
        assert!(eff.visible());

        set_visible(&mut eff, false);
        assert!(!eff.visible());
        match &eff {
            Effect::Shader { shader, visible } => {
                assert!(!*visible);
                assert_eq!(shader.raw, raw_payload);
            }
            other => panic!("expected Effect::Shader, got {other:?}"),
        }
    }
}
