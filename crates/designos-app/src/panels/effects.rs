//! Effects section of the inspector: the "+" menu, one row per effect (type dropdown,
//! settings button, eye, minus) and the settings popovers for every effect type.

use super::properties::{open_picker, title, Pane, PickTarget};
use super::style_picker;
use crate::app::App;
use crate::icons::Icon;
use crate::theme;
use crate::widgets::field::{self, Prefix};
use crate::widgets::menu::{dropdown, popup_menu, MenuItem, MenuResult};
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

pub fn kind_of(e: &Effect) -> usize {
    match e {
        Effect::InnerShadow { .. } => 0,
        Effect::DropShadow { .. } => 1,
        Effect::LayerBlur { .. } => 2,
        Effect::BackgroundBlur { .. } => 3,
        Effect::Noise(_) => 4,
        Effect::Texture(_) => 5,
        Effect::Glass(_) => 6,
    }
}

pub fn set_visible(e: &mut Effect, v: bool) {
    match e {
        Effect::DropShadow { visible, .. }
        | Effect::InnerShadow { visible, .. }
        | Effect::LayerBlur { visible, .. }
        | Effect::BackgroundBlur { visible, .. } => *visible = v,
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
            Effect::InnerShadow { color, offset, radius, spread, visible }
        } else {
            Effect::DropShadow { color, offset, radius, spread, visible, show_behind: false }
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
            density: 0.5,
            opacity: 0.25,
            blend: BlendMode::Normal,
            seed,
            visible: true,
        }),
        5 => Effect::Texture(TextureEffect { size: 2.0, radius: 2.0, clip_to_shape: true, seed, visible: true }),
        _ => Effect::Glass(GlassEffect { light_angle: 315.0, light_intensity: 0.5, refraction: 0.5, depth: 10.0, dispersion: 0.25, frost: 4.0, visible: true }),
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
    set_visible(&mut out, visible);
    out
}

/// "Effects" header with the "+" button (a `bg_selected` tile while its menu is open) and the menu.
fn header(app: &mut App, ui: &mut Ui, pane: &mut Pane, sel: &[NodeId], empty: bool) {
    title(ui, pane, "Effects", None, empty);
    let btn = pane.rect(theme::SECTION_TRAILING_X, -theme::SECTION_TITLE_HEIGHT + 8.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
    let open = app.props.fx_menu;
    if widgets::icon_button(ui, btn, "fx-add", Icon::Plus, open, "").clicked() {
        app.props.fx_menu = !open;
    }
    if !app.props.fx_menu {
        return;
    }
    let mut items: Vec<MenuItem> = KINDS.iter().map(|(n, i)| MenuItem { icon: Some(*i), ..MenuItem::new(*n) }).collect();
    if let Some(last) = items.last_mut() {
        last.separator_after = true;
    }
    items.push(MenuItem { icon: Some(Icon::Shader), disabled: true, ..MenuItem::new("Shader").shortcut("Soon") });
    let w = 168.0;
    let pos = Pos2::new(btn.right() - w, btn.bottom() + 4.0);
    match popup_menu(ui.ctx(), Id::new("fx-add-menu"), pos, w, &items, Some(btn)) {
        MenuResult::Picked(k) if k < KINDS.len() => {
            app.props.fx_menu = false;
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
        if widgets::icon_button(ui, minus_r, ("fx-minus", i), Icon::Minus, false, "").clicked() {
            app.props.pop = None;
            app.edit_nodes(sel, |n| {
                if i < n.effects.len() {
                    n.effects.remove(i);
                }
            });
        }
        pane.y += theme::FIELD_HEIGHT + 8.0;
    }
    if !effects.is_empty() {
        pane.y += 8.0;
    }
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
            let Some(e) = app.doc.get(st.ids.first().copied().unwrap_or(NodeId(0))).and_then(|n| n.effects.get(i)).cloned() else {
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
pub(super) fn effect_rows(e: &Effect) -> usize {
    match e {
        Effect::DropShadow { .. } => 6,
        Effect::InnerShadow { .. } => 5,
        Effect::LayerBlur { .. } | Effect::BackgroundBlur { .. } => 1,
        Effect::Noise(n) => match n.mode {
            NoiseMode::Mono { .. } => 5,
            NoiseMode::Duo { .. } => 6,
            NoiseMode::Multi => 4,
        },
        Effect::Texture(_) => 3,
        Effect::Glass(_) => 6,
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
            let v = if let Some(sk) = &style_key {
                super::var_bind::style_numeric(app, ui, rc, sk, &x_prop, |ui| num(ui, rc, ("fxp-x", i), Prefix::Icon(Icon::DirHorizontal), offset.x, ""))
            } else {
                num(ui, rc, ("fxp-x", i), Prefix::Icon(Icon::DirHorizontal), offset.x, "")
            };
            if let Some(v) = v {
                edit_effect(app, tgt, i, |e| set_shadow(e, |_, o, _, _| o.x = v));
            }
        });
        let y_prop = format!("effects/{i}/offsetY");
        popover::row(ui, body, next(), "Y", |ui, rc| {
            let v = if let Some(sk) = &style_key {
                super::var_bind::style_numeric(app, ui, rc, sk, &y_prop, |ui| num(ui, rc, ("fxp-y", i), Prefix::Icon(Icon::DirVertical), offset.y, ""))
            } else {
                num(ui, rc, ("fxp-y", i), Prefix::Icon(Icon::DirVertical), offset.y, "")
            };
            if let Some(v) = v {
                edit_effect(app, tgt, i, |e| set_shadow(e, |_, o, _, _| o.y = v));
            }
        });
        let r_prop = format!("effects/{i}/radius");
        popover::row(ui, body, next(), "Blur", |ui, rc| {
            let v = if let Some(sk) = &style_key {
                super::var_bind::style_numeric(app, ui, rc, sk, &r_prop, |ui| num(ui, rc, ("fxp-b", i), Prefix::Icon(Icon::LayerBlur), radius, ""))
            } else {
                num(ui, rc, ("fxp-b", i), Prefix::Icon(Icon::LayerBlur), radius, "")
            };
            if let Some(v) = v {
                edit_effect(app, tgt, i, |e| set_shadow(e, |_, _, rr, _| *rr = v.max(0.0)));
            }
        });
        let s_prop = format!("effects/{i}/spread");
        popover::row(ui, body, next(), "Spread", |ui, rc| {
            let v = if let Some(sk) = &style_key {
                super::var_bind::style_numeric(app, ui, rc, sk, &s_prop, |ui| num(ui, rc, ("fxp-s", i), Prefix::Icon(Icon::Scale), spread, ""))
            } else {
                num(ui, rc, ("fxp-s", i), Prefix::Icon(Icon::Scale), spread, "")
            };
            if let Some(v) = v {
                edit_effect(app, tgt, i, |e| set_shadow(e, |_, _, _, sp| *sp = v));
            }
        });
        let c_prop = format!("effects/{i}/color");
        popover::row(ui, body, next(), "Color", |ui, rc| {
            if let Some(sk) = &style_key {
                if !super::var_bind::style_pill_field(app, ui, rc, sk, &c_prop) {
                    if let Some(nc) = color_row(app, ui, rc, (i, 0), color, tgt.picker(PickTarget::Shadow(i))) {
                        edit_effect(app, tgt, i, |e| set_shadow(e, |c, _, _, _| *c = nc));
                    }
                    super::var_bind::style_hover_button(app, ui, rc, sk, &c_prop, VariableKind::Color);
                }
            } else if let Some(nc) = color_row(app, ui, rc, (i, 0), color, tgt.picker(PickTarget::Shadow(i))) {
                edit_effect(app, tgt, i, |e| set_shadow(e, |c, _, _, _| *c = nc));
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
                let v = if let Some(sk) = &style_key {
                    super::var_bind::style_numeric(app, ui, rc, sk, &r_prop, |ui| num(ui, rc, ("fxp-blur", i), Prefix::Icon(Icon::LayerBlur), *radius, ""))
                } else {
                    num(ui, rc, ("fxp-blur", i), Prefix::Icon(Icon::LayerBlur), *radius, "")
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
            popover::row(ui, body, next(), "Size", |ui, rc| {
                if let Some(v) = num(ui, rc, ("fxp-nsize", i), Prefix::Icon(Icon::Scale), n.size, "") {
                    edit_effect(app, tgt, i, |e| {
                        if let Effect::Noise(n) = e {
                            n.size = v.max(0.1);
                        }
                    });
                }
            });
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
            popover::row(ui, body, next(), "Size", |ui, rc| {
                if let Some(v) = num(ui, rc, ("fxp-tsize", i), Prefix::Icon(Icon::Scale), t.size, "") {
                    edit_effect(app, tgt, i, |e| {
                        if let Effect::Texture(t) = e {
                            t.size = v.max(0.1);
                        }
                    });
                }
            });
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
            popover::row(ui, body, next(), "Light angle", |ui, rc| {
                let dial_r = Rect::from_min_size(rc.min, Vec2::splat(24.0));
                let num_r = Rect::from_min_max(Pos2::new(rc.left() + 32.0, rc.top()), rc.right_bottom());
                let mut v = popover::dial(ui, dial_r, ("fxp-dial", i), g.light_angle);
                if let Some(n) = field::num_field(ui, num_r, ("fxp-angle", i), Prefix::Icon(Icon::Rotation), Some(g.light_angle), "°") {
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
            let sliders: [(&str, Icon, f32, &str, u8); 5] = [
                ("Intensity", Icon::Sparkle, pct(g.light_intensity), "%", 0),
                ("Refraction", Icon::Glass, pct(g.refraction), "%", 1),
                ("Depth", Icon::DropShadow, g.depth, "", 2),
                ("Dispersion", Icon::PaintLinear, pct(g.dispersion), "%", 3),
                ("Frost", Icon::BackgroundBlur, g.frost, "", 4),
            ];
            for (label, icon, val, suffix, which) in sliders {
                popover::row(ui, body, next(), label, |ui, rc| {
                    if let Some(v) = num(ui, rc, ("fxp-glass", i, which), Prefix::Icon(icon), val, suffix) {
                        edit_effect(app, tgt, i, |e| {
                            if let Effect::Glass(g) = e {
                                let unit = (v / 100.0).clamp(0.0, 1.0);
                                match which {
                                    0 => g.light_intensity = unit,
                                    1 => g.refraction = unit,
                                    2 => g.depth = v.max(0.0),
                                    3 => g.dispersion = unit,
                                    _ => g.frost = v.max(0.0),
                                }
                            }
                        });
                    }
                });
            }
        }
        _ => {}
    }
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

#[cfg(test)]
mod tests {
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
    fn convert_keeps_shared_values_and_visibility() {
        let e =
            Effect::DropShadow { color: Color::rgba(1, 2, 3, 40), offset: DVec2::new(2.0, 3.0), radius: 9.0, spread: 1.0, visible: false, show_behind: false };
        let inner = convert(&e, 0);
        assert!(matches!(inner, Effect::InnerShadow { radius, visible: false, .. } if radius == 9.0));
        let blur = convert(&e, 2);
        assert!(matches!(blur, Effect::LayerBlur { radius, visible: false } if radius == 9.0));
        assert!(matches!(convert(&e, 6), Effect::Glass(g) if !g.visible));
    }
}
