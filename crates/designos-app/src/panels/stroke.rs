//! Stroke section of the inspector: paint rows, Position / Weight, the stroke settings
//! popover (caps, joins, miter, dashes) and the per-side weights toggle.

use super::effects::{PopKind, PopState};
use super::properties::{paint_rows, paint_title, row_end, Pane};
use super::var_bind;
use crate::app::App;
use crate::icons::Icon;
use crate::theme;
use crate::widgets::field::{self, Prefix};
use crate::widgets::menu::dropdown;
use crate::widgets::{self, popover};
use designos_doc::*;
use egui::{Id, Rect, Ui};

const POSITION_X: f32 = 17.0;
const POSITION_W: f32 = 76.0;
const WEIGHT_X: f32 = 101.0;
const WEIGHT_W: f32 = 72.0;
const SETTINGS_X: f32 = 184.0;

fn common<T: PartialEq>(app: &App, sel: &[NodeId], f: impl Fn(&Node) -> T) -> Option<T> {
    let mut it = sel.iter().take(128).filter_map(|id| app.doc.get(*id)).map(f);
    let first = it.next()?;
    it.all(|v| v == first).then_some(first)
}

fn supports_sides(n: &Node) -> bool {
    matches!(n.kind, NodeKind::Frame(_) | NodeKind::Rectangle | NodeKind::Component(_) | NodeKind::Instance { .. })
}

/// Small secondary label of the Position / Weight sub-row.
fn sub_label(ui: &Ui, pane: &Pane, x: f32, text: &str) {
    let r = pane.rect(x, 0.0, 80.0, 16.0);
    let font = theme::font_body();
    widgets::text_on_baseline(ui.painter(), r.left(), widgets::centered_baseline(r, &font), text, font, theme::TEXT_SECONDARY);
}

pub(super) fn section(app: &mut App, ui: &mut Ui, pane: &mut Pane, sel: &[NodeId], n0: &Node) {
    if paint_title(app, ui, pane, sel, "Stroke", true, n0.stroke.paints.is_empty()) {
        let paint = Paint::solid(Color::from_hex(&format!("{:06X}", theme::DEFAULT_STROKE)).unwrap_or(Color::BLACK));
        app.edit_nodes(sel, |n| n.stroke.paints.push(paint.clone()));
    }
    paint_rows(app, ui, pane, sel, &n0.stroke.paints, true);
    if n0.stroke.paints.is_empty() {
        return;
    }
    // Labels, then the controls row.
    sub_label(ui, pane, POSITION_X, "Position");
    sub_label(ui, pane, WEIGHT_X, "Weight");
    pane.y += 16.0 + 3.0;
    let aligns = ["Inside", "Center", "Outside"].map(String::from);
    let cur = match n0.stroke.align {
        StrokeAlign::Inside => 0,
        StrokeAlign::Center => 1,
        StrokeAlign::Outside => 2,
    };
    if let Some(i) = dropdown(ui, pane.rect(POSITION_X, 0.0, POSITION_W, theme::FIELD_HEIGHT), "stroke-align", &aligns[cur], &aligns) {
        let a = [StrokeAlign::Inside, StrokeAlign::Center, StrokeAlign::Outside][i];
        app.edit_nodes(sel, |n| n.stroke.align = a);
    }
    let sides_on = common(app, sel, |n| n.stroke.side_weights.is_some()).unwrap_or(false);
    let w_r = pane.rect(WEIGHT_X, 0.0, WEIGHT_W, theme::FIELD_HEIGHT);
    let sw_val = if sides_on { None } else { common(app, sel, |n| n.stroke.weight) };
    if let Some(w) =
        var_bind::numeric(app, ui, w_r, sel, &["strokeWeight"], |ui, r| field::num_field(ui, r, "stroke-w", Prefix::Icon(Icon::StrokeWeight), sw_val, ""))
    {
        let w = w.max(0.0);
        app.edit_nodes(sel, |n| {
            n.stroke.weight = w;
            if n.stroke.side_weights.is_some() {
                n.stroke.side_weights = Some([w; 4]);
            }
        });
    }
    let set_r = pane.rect(SETTINGS_X, 0.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
    let open = app.props.pop.as_ref().is_some_and(|p| p.kind == PopKind::Stroke);
    if widgets::icon_button(ui, set_r, "stroke-settings", Icon::StrokeSettings, open, "Stroke settings").clicked() {
        app.props.pop = (!open).then(|| PopState { kind: PopKind::Stroke, anchor: set_r, ids: sel.to_vec() });
    }
    if sel.iter().take(128).filter_map(|id| app.doc.get(*id)).all(supports_sides) {
        let sides_r = pane.rect(theme::SECTION_TRAILING_X, 0.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
        if widgets::icon_button(ui, sides_r, "stroke-sides", Icon::StrokeSides, sides_on, "Individual strokes").clicked() {
            app.edit_nodes(sel, |n| {
                n.stroke.side_weights = if sides_on { None } else { Some([n.stroke.weight; 4]) };
            });
        }
    }
    row_end(pane);
    if sides_on {
        const SIDE_KEYS: [&str; 4] = ["strokeTopWeight", "strokeRightWeight", "strokeBottomWeight", "strokeLeftWeight"];
        let w = common(app, sel, |n| n.stroke.side_weights).flatten().unwrap_or([0.0; 4]);
        for pair in [[("T", 0usize), ("R", 1)], [("B", 2), ("L", 3)]] {
            for (col, (name, idx)) in pair.into_iter().enumerate() {
                let rect = pane.col(col, 0.0);
                if let Some(v) = var_bind::numeric(app, ui, rect, sel, &[SIDE_KEYS[idx]], |ui, r| {
                    field::num_field(ui, r, ("stroke-side", idx), Prefix::Text(name), Some(w[idx]), "")
                }) {
                    app.edit_nodes(sel, |n| {
                        let mut s = n.stroke.side_weights.unwrap_or([n.stroke.weight; 4]);
                        s[idx] = v.max(0.0);
                        n.stroke.side_weights = Some(s);
                    });
                }
            }
            row_end(pane);
        }
    }
    pane.y += 14.0;
}

const CAPS: [(&str, StrokeCap); 3] = [("None", StrokeCap::None), ("Round", StrokeCap::Round), ("Square", StrokeCap::Square)];
const JOINS: [(&str, StrokeJoin); 3] = [("Miter", StrokeJoin::Miter), ("Bevel", StrokeJoin::Bevel), ("Round", StrokeJoin::Round)];

pub(super) fn popover_ui(app: &mut App, ui: &mut Ui, region: Rect, st: &PopState) {
    let ctx = ui.ctx().clone();
    let Some(first) = st.ids.first().and_then(|id| app.doc.get(*id)) else {
        app.props.pop = None;
        return;
    };
    let s = first.stroke.clone();
    let ids = st.ids.clone();
    let pop = popover::show(&ctx, Id::new("inspector-popover"), st.anchor, region, 240.0, popover::height(5), "Stroke settings", |ui, body| {
        let caps = CAPS.map(|c| c.0.to_string());
        let cur = CAPS.iter().position(|c| c.1 == s.cap).unwrap_or(0);
        popover::row(ui, body, 0, "Cap", |ui, r| {
            if let Some(i) = dropdown(ui, r, "strokep-cap", &caps[cur], &caps) {
                app.edit_nodes(&ids, |n| n.stroke.cap = CAPS[i].1);
            }
        });
        let joins = JOINS.map(|c| c.0.to_string());
        let cur = JOINS.iter().position(|c| c.1 == s.join).unwrap_or(0);
        popover::row(ui, body, 1, "Join", |ui, r| {
            if let Some(i) = dropdown(ui, r, "strokep-join", &joins[cur], &joins) {
                app.edit_nodes(&ids, |n| n.stroke.join = JOINS[i].1);
            }
        });
        popover::row(ui, body, 2, "Miter angle", |ui, r| {
            // Miter limit is stored as a ratio; the UI shows the equivalent corner angle.
            let angle = miter_angle(s.miter_limit);
            if let Some(a) = field::num_field(ui, r, "strokep-miter", Prefix::None, Some(angle), "°") {
                let limit = miter_limit(a);
                app.edit_nodes(&ids, |n| n.stroke.miter_limit = limit);
            }
        });
        let dash = s.dashes.first().copied().unwrap_or(0.0);
        let gap = s.dashes.get(1).copied().unwrap_or(dash);
        popover::row(ui, body, 3, "Dash", |ui, r| {
            if let Some(v) = field::num_field(ui, r, "strokep-dash", Prefix::None, Some(dash), "") {
                let v = v.max(0.0);
                app.edit_nodes(&ids, |n| n.stroke.dashes = if v <= 0.0 { Vec::new() } else { vec![v, n.stroke.dashes.get(1).copied().unwrap_or(v)] });
            }
        });
        popover::row(ui, body, 4, "Gap", |ui, r| {
            if let Some(v) = field::num_field(ui, r, "strokep-gap", Prefix::None, Some(gap), "") {
                let v = v.max(0.0);
                app.edit_nodes(&ids, |n| {
                    let d = n.stroke.dashes.first().copied().unwrap_or(0.0);
                    n.stroke.dashes = if d <= 0.0 { Vec::new() } else { vec![d, v] };
                });
            }
        });
    });
    if pop {
        app.props.pop = None;
    }
}

/// Corner angle (degrees) at which a miter join turns into a bevel: limit = 1 / sin(angle / 2).
pub fn miter_angle(limit: f32) -> f32 {
    if limit <= 1.0 {
        return 180.0;
    }
    ((1.0 / limit).asin().to_degrees() * 2.0 * 10.0).round() / 10.0
}

pub fn miter_limit(angle: f32) -> f32 {
    let a = angle.clamp(1.0, 180.0).to_radians() / 2.0;
    (1.0 / a.sin()).clamp(1.0, 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn miter_angle_roundtrips() {
        assert!((miter_limit(28.9) - 4.0).abs() < 0.05);
        assert!((miter_angle(4.0) - 28.9).abs() < 0.2);
        assert_eq!(miter_angle(1.0), 180.0);
    }
}
