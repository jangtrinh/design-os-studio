//! Stroke section of the inspector: paint rows, Position / Weight, the stroke settings
//! popover (caps, joins, miter, dashes) and the per-side weights toggle.

use super::effects::{PopKind, PopState};
use super::properties::{paint_rows, paint_title, row_end, Pane};
use super::var_bind;
use crate::app::App;
use crate::icons::Icon;
use crate::theme;
use crate::widgets::field::{self, Prefix};
use crate::widgets::menu::{dropdown, dropdown_mixed};
use crate::widgets::{self, popover};
use designos_doc::*;
use egui::{Id, Rect, Ui};

const POSITION_X: f32 = 17.0;
const POSITION_W: f32 = 76.0;
const WEIGHT_X: f32 = 101.0;
const WEIGHT_W: f32 = 72.0;
const SETTINGS_X: f32 = 184.0;
const SUB_LABEL_ADVANCE: f32 = 19.0;

/// The options of the Start point and End point dropdowns, in the order Figma lists them.
pub(crate) const ENDPOINT_CAPS: [(&str, StrokeCap); 8] = [
    ("None", StrokeCap::None),
    ("Line arrow", StrokeCap::ArrowLines),
    ("Triangle arrow", StrokeCap::ArrowEquilateral),
    ("Reversed triangle", StrokeCap::TriangleFilled),
    ("Circle arrow", StrokeCap::CircleFilled),
    ("Diamond arrow", StrokeCap::DiamondFilled),
    ("Round", StrokeCap::Round),
    ("Square", StrokeCap::Square),
];

fn common<T: PartialEq>(app: &App, sel: &[NodeId], f: impl Fn(&Node) -> T) -> Option<T> {
    let mut it = sel.iter().take(128).filter_map(|id| app.doc.get(*id)).map(f);
    let first = it.next()?;
    it.all(|v| v == first).then_some(first)
}

fn supports_sides(n: &Node) -> bool {
    matches!(n.kind, NodeKind::Frame(_) | NodeKind::Rectangle | NodeKind::Component(_) | NodeKind::Instance { .. })
}

fn open_ends_of(n: &Node) -> Option<Vec<(usize, usize)>> {
    match &n.kind {
        NodeKind::Line => Some(vec![(0, 1)]),
        NodeKind::Vector { paths, network } => {
            let ends = match network.as_ref().filter(|net| net.is_valid()) {
                Some(net) => net.open_chain_ends(),
                None => VectorNetwork::from_paths(paths).open_chain_ends(),
            };
            (!ends.is_empty()).then_some(ends)
        }
        _ => None,
    }
}

fn endpoint_cap_of(n: &Node, is_end: bool) -> Option<StrokeCap> {
    match &n.kind {
        NodeKind::Line => Some(n.stroke.cap),
        NodeKind::Vector { network, .. } => {
            let ends = open_ends_of(n)?;
            if let Some(net) = network.as_ref().filter(|net| net.is_valid()) {
                let mut it = ends.into_iter().map(|(s, e)| {
                    let idx = if is_end { e } else { s };
                    net.vertices.get(idx).and_then(|v| v.cap).unwrap_or(n.stroke.cap)
                });
                let first = it.next()?;
                it.all(|c| c == first).then_some(first)
            } else {
                Some(n.stroke.cap)
            }
        }
        _ => None,
    }
}

fn endpoint_cap_label(cap: Option<StrokeCap>) -> &'static str {
    match cap {
        Some(c) => ENDPOINT_CAPS.iter().find(|&&(_, v)| v == c).map(|&(name, _)| name).unwrap_or("None"),
        None => "Mixed",
    }
}

fn ensure_open_vector_network(n: &mut Node) {
    if matches!(n.kind, NodeKind::Line) {
        let path = Path { cmds: vec![PathCmd::MoveTo(Vec2::ZERO), PathCmd::LineTo(Vec2::new(n.size.x, 0.0))], fill_rule: FillRule::NonZero };
        let net = VectorNetwork::from_paths(std::slice::from_ref(&path));
        n.kind = NodeKind::Vector { paths: vec![path], network: Some(net) };
        n.fills.clear();
    } else if let NodeKind::Vector { paths, network } = &mut n.kind {
        if network.as_ref().is_none_or(|net| !net.is_valid()) {
            *network = Some(VectorNetwork::from_paths(paths));
        }
    }
}

fn set_node_endpoint_cap(n: &mut Node, is_end: bool, chosen: StrokeCap) {
    if open_ends_of(n).is_none() {
        return;
    }
    ensure_open_vector_network(n);
    let default_cap = n.stroke.cap;
    if let NodeKind::Vector { paths, network: Some(net) } = &mut n.kind {
        for (s, e) in net.open_chain_ends() {
            let sc = net.vertices[s].cap.unwrap_or(default_cap);
            let ec = net.vertices[e].cap.unwrap_or(default_cap);
            net.vertices[s].cap = Some(if is_end { sc } else { chosen });
            net.vertices[e].cap = Some(if is_end { chosen } else { ec });
        }
        *paths = net.to_paths();
    }
    n.stroke_geometry = None;
    n.fig_metadata.remove("strokeCap");
    n.fig_metadata.remove("strokeCapSource");
}

/// Writes `cap` onto both ends of every open chain of the vector network of `n`, if it has a valid one.
fn set_open_end_caps(n: &mut Node, cap: StrokeCap) {
    let NodeKind::Vector { network: Some(net), .. } = &mut n.kind else { return };
    if !net.is_valid() {
        return;
    }
    for (s, e) in net.open_chain_ends() {
        net.vertices[s].cap = Some(cap);
        net.vertices[e].cap = Some(cap);
    }
}

fn swap_node_endpoints(n: &mut Node) {
    if open_ends_of(n).is_none() {
        return;
    }
    ensure_open_vector_network(n);
    let default_cap = n.stroke.cap;
    if let NodeKind::Vector { paths, network: Some(net) } = &mut n.kind {
        for (s, e) in net.open_chain_ends() {
            let sc = net.vertices[s].cap.unwrap_or(default_cap);
            let ec = net.vertices[e].cap.unwrap_or(default_cap);
            net.vertices[s].cap = Some(ec);
            net.vertices[e].cap = Some(sc);
        }
        *paths = net.to_paths();
    }
    n.stroke_geometry = None;
    n.fig_metadata.remove("strokeCap");
    n.fig_metadata.remove("strokeCapSource");
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
    let open_ids: Vec<NodeId> =
        sel.iter().take(128).copied().filter(|id| app.doc.get(*id).is_some_and(|n| !n.stroke.paints.is_empty() && open_ends_of(n).is_some())).collect();
    if !open_ids.is_empty() {
        // One row per point, its label above a dropdown as wide as the section: the longest option name
        // ("Reversed triangle") does not fit the half-width columns of the Position / Weight row.
        let endpoint_opts = ENDPOINT_CAPS.map(|(name, _)| name.to_string());
        let start_cap = common(app, &open_ids, |n| endpoint_cap_of(n, false)).flatten();
        let end_cap = common(app, &open_ids, |n| endpoint_cap_of(n, true)).flatten();

        sub_label(ui, pane, theme::SECTION_CONTENT_X, "Start point");
        pane.y += SUB_LABEL_ADVANCE;
        let start_r = pane.full(0.0);
        let swap_r = pane.rect(theme::SECTION_TRAILING_X, 0.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
        widgets::record(ui, "stroke-start-point", start_r);
        if let Some(i) = dropdown_mixed(ui, start_r, "stroke-start-point", endpoint_cap_label(start_cap), &endpoint_opts, start_cap.is_none()) {
            let chosen = ENDPOINT_CAPS[i].1;
            app.edit_nodes(&open_ids, |n| set_node_endpoint_cap(n, false, chosen));
        }
        widgets::record(ui, "stroke-swap-points", swap_r);
        if widgets::icon_button(ui, swap_r, "stroke-swap-points", Icon::FlipH, false, "Swap end points").clicked() {
            app.edit_nodes(&open_ids, swap_node_endpoints);
        }
        row_end(pane);

        sub_label(ui, pane, theme::SECTION_CONTENT_X, "End point");
        pane.y += SUB_LABEL_ADVANCE;
        let end_r = pane.full(0.0);
        widgets::record(ui, "stroke-end-point", end_r);
        if let Some(i) = dropdown_mixed(ui, end_r, "stroke-end-point", endpoint_cap_label(end_cap), &endpoint_opts, end_cap.is_none()) {
            let chosen = ENDPOINT_CAPS[i].1;
            app.edit_nodes(&open_ids, |n| set_node_endpoint_cap(n, true, chosen));
        }
        row_end(pane);
    } else {
        #[cfg(test)]
        ui.ctx().data_mut(|d| {
            for k in ["stroke-start-point", "stroke-end-point", "stroke-swap-points"] {
                d.remove::<Rect>(Id::new(("test-rect", k)));
            }
        });
    }
    pane.y += 14.0;
}

const CAPS: [(&str, StrokeCap); 3] = [("None", StrokeCap::None), ("Round", StrokeCap::Round), ("Square", StrokeCap::Square)];
const JOINS: [(&str, StrokeJoin); 3] = [("Miter", StrokeJoin::Miter), ("Bevel", StrokeJoin::Bevel), ("Round", StrokeJoin::Round)];

fn has_dashes(s: &Stroke) -> bool {
    !s.dashes.is_empty() && s.dashes.iter().all(|d| *d >= 0.0) && s.dashes.iter().sum::<f32>() > 0.0
}

fn stroked_ids(app: &App, ids: &[NodeId]) -> Vec<NodeId> {
    let v: Vec<NodeId> = ids.iter().copied().filter(|id| app.doc.get(*id).is_some_and(|n| !n.stroke.paints.is_empty())).collect();
    if v.is_empty() {
        ids.to_vec()
    } else {
        v
    }
}

fn cap_label(cap: Option<StrokeCap>) -> &'static str {
    match cap {
        Some(c) => CAPS.iter().find(|&&(_, v)| v == c).map(|&(name, _)| name).unwrap_or("None"),
        None => "Mixed",
    }
}

pub(super) fn popover_ui(app: &mut App, ui: &mut Ui, region: Rect, st: &PopState) {
    let ctx = ui.ctx().clone();
    let ids = stroked_ids(app, &st.ids);
    let Some(first) = ids.first().and_then(|id| app.doc.get(*id)) else {
        app.props.pop = None;
        return;
    };
    let s = first.stroke.clone();
    let common_cap = common(app, &ids, |n| n.stroke.cap);
    let show_dash_cap = ids.iter().take(128).filter_map(|id| app.doc.get(*id)).any(|n| has_dashes(&n.stroke));
    let rows = if show_dash_cap { 6 } else { 5 };
    let pop = popover::show(&ctx, Id::new("inspector-popover"), st.anchor, region, 240.0, popover::height(rows), "Stroke settings", |ui, body| {
        let caps = CAPS.map(|c| c.0.to_string());
        let cap_lbl = cap_label(common_cap);
        popover::row(ui, body, 0, "Cap", |ui, r| {
            if let Some(i) = dropdown_mixed(ui, r, "strokep-cap", cap_lbl, &caps, common_cap.is_none()) {
                app.edit_nodes(&ids, |n| {
                    n.stroke.cap = CAPS[i].1;
                    // a solid open path ends in its end points: they follow the cap
                    if !has_dashes(&n.stroke) {
                        set_open_end_caps(n, CAPS[i].1);
                    }
                    n.stroke_geometry = None;
                });
            }
        });
        let joins = JOINS.map(|c| c.0.to_string());
        let cur = JOINS.iter().position(|c| c.1 == s.join).unwrap_or(0);
        popover::row(ui, body, 1, "Join", |ui, r| {
            if let Some(i) = dropdown(ui, r, "strokep-join", &joins[cur], &joins) {
                app.edit_nodes(&ids, |n| {
                    n.stroke.join = JOINS[i].1;
                    n.stroke_geometry = None;
                });
            }
        });
        popover::row(ui, body, 2, "Miter angle", |ui, r| {
            // Miter limit is stored as a ratio; the UI shows the equivalent corner angle.
            let angle = miter_angle(s.miter_limit);
            if let Some(a) = field::num_field(ui, r, "strokep-miter", Prefix::None, Some(angle), "°") {
                let limit = miter_limit(a);
                app.edit_nodes(&ids, |n| {
                    n.stroke.miter_limit = limit;
                    n.stroke_geometry = None;
                });
            }
        });
        let dash = s.dashes.first().copied().unwrap_or(0.0);
        let gap = s.dashes.get(1).copied().unwrap_or(dash);
        popover::row(ui, body, 3, "Dash", |ui, r| {
            if let Some(v) = field::num_field(ui, r, "strokep-dash", Prefix::None, Some(dash), "") {
                let v = v.max(0.0);
                app.edit_nodes(&ids, |n| {
                    n.stroke.dashes = if v <= 0.0 { Vec::new() } else { vec![v, n.stroke.dashes.get(1).copied().unwrap_or(v)] };
                    n.stroke_geometry = None;
                });
            }
        });
        popover::row(ui, body, 4, "Gap", |ui, r| {
            if let Some(v) = field::num_field(ui, r, "strokep-gap", Prefix::None, Some(gap), "") {
                let v = v.max(0.0);
                app.edit_nodes(&ids, |n| {
                    let d = n.stroke.dashes.first().copied().unwrap_or(0.0);
                    n.stroke.dashes = if d <= 0.0 { Vec::new() } else { vec![d, v] };
                    n.stroke_geometry = None;
                });
            }
        });
        if show_dash_cap {
            popover::row(ui, body, 5, "Dash cap", |ui, r| {
                if let Some(i) = dropdown_mixed(ui, r, "strokep-dash-cap", cap_lbl, &caps, common_cap.is_none()) {
                    app.edit_nodes(&ids, |n| {
                        n.stroke.cap = CAPS[i].1;
                        n.stroke_geometry = None;
                    });
                }
            });
        }
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

    #[test]
    fn stroke_panel_dash_cap_visibility_mixed_and_multi_selection_single_undo() {
        let mut doc = Document::new("t");
        let page = doc.add_page("p");
        let mut mk_rect = |name: &str, stroked: bool, cap: StrokeCap| -> NodeId {
            let id = doc.alloc_id();
            let mut n = Node::rectangle(id, designos_doc::Rect::new(0.0, 0.0, 40.0, 40.0));
            n.name = name.into();
            n.stroke.paints = if stroked { vec![Paint::solid(Color::BLACK)] } else { Vec::new() };
            n.stroke.cap = cap;
            doc.insert(page, n).unwrap()
        };
        let r1 = mk_rect("r1", true, StrokeCap::None);
        let r2 = mk_rect("r2", true, StrokeCap::Square);
        let r3_unstroked = mk_rect("r3", false, StrokeCap::None);

        let mut app = App::new(doc);
        app.selection = vec![r1, r2, r3_unstroked];
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(1000.0, 700.0));
        let region = egui::Rect::from_min_max(egui::Pos2::new(760.0, 40.0), egui::Pos2::new(1000.0, 700.0));
        let st = PopState {
            kind: PopKind::Stroke,
            anchor: egui::Rect::from_min_size(egui::Pos2::new(940.0, 120.0), egui::Vec2::splat(24.0)),
            ids: vec![r1, r2, r3_unstroked],
        };

        let run_frame = |app: &mut App, events: Vec<egui::Event>, t: f64| -> Vec<(String, egui::Rect)> {
            let raw = egui::RawInput { screen_rect: Some(screen), time: Some(t), events, ..Default::default() };
            let shapes = ctx.run_ui(raw, |ui| popover_ui(app, ui, region, &st)).shapes;
            fn collect(shape: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
                match shape {
                    egui::Shape::Text(t) => out.push((t.galley.job.text.clone(), egui::Rect::from_min_size(t.pos, t.galley.size()))),
                    egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
                    _ => {}
                }
            }
            let mut out = Vec::new();
            for c in &shapes {
                collect(&c.shape, &mut out);
            }
            out
        };

        // 1. While dashes are empty, "Dash cap" must not appear in the stroke settings popover.
        let texts_no_dash = run_frame(&mut app, vec![], 0.1);
        assert!(!texts_no_dash.iter().any(|(s, _)| s == "Dash cap"), "Dash cap row must be hidden when stroke has no dashes, got {texts_no_dash:?}");

        // 2. Enable dashes on r1 (None) and r2 (Square), plus baked stroke_geometry on r1.
        app.doc.get_mut(r1).unwrap().stroke.dashes = vec![8.0, 4.0];
        app.doc.get_mut(r1).unwrap().stroke_geometry = Some(vec![Path { cmds: Vec::new(), fill_rule: FillRule::NonZero }]);
        app.doc.get_mut(r2).unwrap().stroke.dashes = vec![8.0, 4.0];

        let texts_mixed = run_frame(&mut app, vec![], 0.2);
        let (_, dash_cap_label_rect) = texts_mixed
            .iter()
            .find(|(s, _)| s == "Dash cap")
            .cloned()
            .unwrap_or_else(|| panic!("expected 'Dash cap' row when dashes are set, got {texts_mixed:?}"));

        let dd_list: Vec<(Id, egui::Rect)> = ctx.data(|d| d.get_temp(Id::new("test-dd-list"))).unwrap_or_default();
        let (_, dash_cap_dd_rect) =
            dd_list.iter().find(|(_, r)| (r.center().y - dash_cap_label_rect.center().y).abs() < 12.0).copied().expect("Dash cap dropdown rect");
        let dd_text = texts_mixed.iter().find(|(_, r)| dash_cap_dd_rect.contains(r.center())).map(|(s, _)| s.as_str()).unwrap_or("");
        assert_eq!(dd_text, "Mixed", "disagreeing caps (None vs Square) must display 'Mixed' in the Dash cap dropdown");

        // 3. Click the Dash cap dropdown to open it, then pick "Round" (option index 1).
        let click = |app: &mut App, at: egui::Pos2, t0: f64| {
            run_frame(app, vec![egui::Event::PointerMoved(at)], t0);
            run_frame(
                app,
                vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE }],
                t0 + 0.02,
            );
            run_frame(
                app,
                vec![egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE }],
                t0 + 0.04,
            );
            run_frame(app, vec![], t0 + 0.06);
        };
        click(&mut app, dash_cap_dd_rect.center(), 0.3);
        let round_row_rect: egui::Rect =
            ctx.data(|d| d.get_temp(Id::new(("test-rect", "menu-row-0-1".to_string())))).expect("Round menu item row rect when Dash cap dropdown is open");
        click(&mut app, round_row_rect.center(), 0.5);

        assert_eq!(app.doc.get(r1).unwrap().stroke.cap, StrokeCap::Round, "r1 updated to Round");
        assert!(app.doc.get(r1).unwrap().stroke_geometry.is_none(), "stale stroke_geometry cleared on cap edit");
        assert_eq!(app.doc.get(r2).unwrap().stroke.cap, StrokeCap::Round, "r2 updated to Round");
        assert_eq!(app.doc.get(r3_unstroked).unwrap().stroke.cap, StrokeCap::None, "unstroked layer r3 left untouched");

        let texts_after = run_frame(&mut app, vec![], 0.7);
        let dd_text_after = texts_after.iter().find(|(_, r)| dash_cap_dd_rect.contains(r.center())).map(|(s, _)| s.as_str()).unwrap_or("");
        assert_eq!(dd_text_after, "Round", "Dash cap dropdown shows 'Round' after selection agrees");

        // 4. One undo step restores both r1 and r2 to their prior caps.
        app.undo();
        assert_eq!(app.doc.get(r1).unwrap().stroke.cap, StrokeCap::None, "r1 restored by single undo");
        assert_eq!(app.doc.get(r2).unwrap().stroke.cap, StrokeCap::Square, "r2 restored by single undo");
    }

    use egui::Pos2;

    /// Row of the Start point / End point dropdowns that sets `cap`.
    fn option_of(cap: StrokeCap) -> usize {
        ENDPOINT_CAPS.iter().position(|&(_, c)| c == cap).expect("the dropdowns offer the cap")
    }

    fn end_vertex_caps(n: &designos_doc::Node) -> Option<(Option<StrokeCap>, Option<StrokeCap>)> {
        let NodeKind::Vector { network: Some(net), .. } = &n.kind else { return None };
        let &(s, e) = net.open_chain_ends().first()?;
        Some((net.vertices[s].cap, net.vertices[e].cap))
    }

    fn shapes_contain_text(shapes: &[egui::epaint::ClippedShape], needle: &str) -> bool {
        fn check(s: &egui::epaint::Shape, needle: &str) -> bool {
            match s {
                egui::epaint::Shape::Text(t) => t.galley.job.text.contains(needle),
                egui::epaint::Shape::Vec(v) => v.iter().any(|sub| check(sub, needle)),
                _ => false,
            }
        }
        shapes.iter().any(|cs| check(&cs.shape, needle))
    }

    fn is_ink(pm: &designos_render::tiny_skia::Pixmap, x: u32, y: u32) -> bool {
        let p = pm.pixel(x, y).unwrap();
        p.red() < 200 || p.green() < 200 || p.blue() < 200
    }

    #[test]
    fn stroke_panel_start_and_end_points_set_each_arrow_cap_in_one_undo_step_and_render_cpu_and_gpu() {
        let mut doc = Document::new("stroke-endpoints-arrows");
        let page = doc.add_page("Page 1");
        doc.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::WHITE };
        let mut app = App::new(doc);
        app.page = page;

        let line_id = app.doc.alloc_id();
        let mut line = designos_doc::Node::new(line_id, NodeKind::Line);
        line.name = "Open Line".into();
        line.transform = designos_doc::Affine::translate(20.0, 50.0);
        line.size = designos_doc::Vec2::new(60.0, 0.0);
        line.fills.clear();
        line.stroke.paints = vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))];
        line.stroke.weight = 4.0;
        line.stroke.align = StrokeAlign::Center;
        line.stroke.cap = StrokeCap::None;
        line.stroke.join = StrokeJoin::Round;
        let line = app.doc.insert(page, line).unwrap();
        app.selection = vec![line];

        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::theme::install_fonts(&ctx);
        let region = Rect::from_min_size(Pos2::new(0.0, 0.0), egui::Vec2::new(240.0, 900.0));
        let test_rect = |ctx: &egui::Context, key: &str| ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", key.to_string()))));
        let now = std::cell::Cell::new(1.0f64);
        let step = |app: &mut App, events: Vec<egui::Event>| -> egui::FullOutput {
            now.set(now.get() + 0.1);
            let raw = egui::RawInput { screen_rect: Some(region), time: Some(now.get()), events, ..Default::default() };
            ctx.run_ui(raw, |ui| super::super::properties::show(app, ui, region))
        };
        let click = |app: &mut App, pos: Pos2| {
            now.set(now.get() + 0.5);
            let _ = step(app, vec![egui::Event::PointerMoved(pos)]);
            let _ = step(app, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE }]);
            let _ = step(app, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE }]);
            let _ = step(app, vec![]);
        };

        let _ = step(&mut app, vec![]);
        let start_r = test_rect(&ctx, "stroke-start-point").expect("stroke-start-point dropdown rendered for open line");
        let end_r = test_rect(&ctx, "stroke-end-point").expect("stroke-end-point dropdown rendered for open line");
        let _swap_r = test_rect(&ctx, "stroke-swap-points").expect("stroke-swap-points button rendered for open line");
        assert!(start_r.height() >= crate::theme::TARGET_MIN && end_r.height() >= crate::theme::TARGET_MIN);

        let arrow_options = [
            (StrokeCap::ArrowLines, (70, 40), (30, 40)),
            (StrokeCap::ArrowEquilateral, (62, 41), (38, 41)),
            (StrokeCap::TriangleFilled, (78, 41), (22, 41)),
            (StrokeCap::DiamondFilled, (80, 40), (20, 40)),
            (StrokeCap::CircleFilled, (80, 41), (20, 41)),
        ];

        let mut gpu = designos_gpu::HeadlessGpu::new().ok().filter(|g| {
            let name = g.ctx.adapter_info.name.to_lowercase();
            g.ctx.adapter_info.device_type != designos_gpu::wgpu::DeviceType::Cpu
                && !name.contains("llvmpipe")
                && !name.contains("lavapipe")
                && !name.contains("software")
        });
        let vp = designos_render::Viewport { offset: designos_doc::Vec2::ZERO, zoom: 1.0, ..designos_render::Viewport::default() };

        for (expected_cap, end_probe, start_probe) in arrow_options {
            let opt_idx = option_of(expected_cap);
            let _ = step(&mut app, vec![]);
            let end_r = test_rect(&ctx, "stroke-end-point").expect("stroke-end-point dropdown");
            click(&mut app, end_r.center());
            let row_r = test_rect(&ctx, &format!("menu-row-0-{opt_idx}")).expect("endpoint menu row rect");
            click(&mut app, row_r.center());

            let n = app.doc.get(line).unwrap();
            assert_eq!(n.stroke.cap, StrokeCap::None, "node cap stays None when setting endpoint cap {expected_cap:?}");
            assert_eq!(
                end_vertex_caps(n),
                Some((Some(StrokeCap::None), Some(expected_cap))),
                "end vertex receives {expected_cap:?} while start vertex stays None"
            );

            let mut cpu = designos_render::tiny_skia::Pixmap::new(110, 100).unwrap();
            designos_render::render_page(&app.doc, page, &vp, &mut cpu);
            assert!(is_ink(&cpu, end_probe.0, end_probe.1), "CPU draws {expected_cap:?} at end probe {end_probe:?}");
            assert!(!is_ink(&cpu, start_probe.0, start_probe.1), "CPU leaves start probe {start_probe:?} uninked for {expected_cap:?}");

            if let Some(g) = gpu.as_mut() {
                let frame = g.compile(&app.doc, page, opt_idx as u64 + 1, &vp, (110, 100));
                let gpu_pm = g.render_frame(&frame, (110, 100));
                assert!(is_ink(&gpu_pm, end_probe.0, end_probe.1), "GPU draws {expected_cap:?} at end probe {end_probe:?}");
                assert!(!is_ink(&gpu_pm, start_probe.0, start_probe.1), "GPU leaves start probe {start_probe:?} uninked for {expected_cap:?}");
            }

            // Every change is one undo step.
            app.undo();
            let prev = app.doc.get(line).unwrap();
            assert_ne!(end_vertex_caps(prev).and_then(|(_, e)| e), Some(expected_cap), "one undo step reverts {expected_cap:?}");
            app.redo();
            assert_eq!(end_vertex_caps(app.doc.get(line).unwrap()), Some((Some(StrokeCap::None), Some(expected_cap))));
        }
    }

    #[test]
    fn stroke_panel_swap_button_exchanges_start_and_end_caps_in_one_undo_step() {
        let mut doc = Document::new("stroke-endpoints-swap");
        let page = doc.add_page("Page 1");
        doc.get_mut(page).unwrap().kind = NodeKind::Page { background: Color::WHITE };
        let mut app = App::new(doc);
        app.page = page;

        let line_id = app.doc.alloc_id();
        let mut line = designos_doc::Node::new(line_id, NodeKind::Line);
        line.transform = designos_doc::Affine::translate(20.0, 50.0);
        line.size = designos_doc::Vec2::new(60.0, 0.0);
        line.fills.clear();
        line.stroke.paints = vec![Paint::solid(Color::rgb(0x18, 0x1D, 0x27))];
        line.stroke.weight = 4.0;
        let line = app.doc.insert(page, line).unwrap();
        app.selection = vec![line];

        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::theme::install_fonts(&ctx);
        let region = Rect::from_min_size(Pos2::new(0.0, 0.0), egui::Vec2::new(240.0, 900.0));
        let test_rect = |ctx: &egui::Context, key: &str| ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", key.to_string()))));
        let now = std::cell::Cell::new(1.0f64);
        let step = |app: &mut App, events: Vec<egui::Event>| -> egui::FullOutput {
            now.set(now.get() + 0.1);
            let raw = egui::RawInput { screen_rect: Some(region), time: Some(now.get()), events, ..Default::default() };
            ctx.run_ui(raw, |ui| super::super::properties::show(app, ui, region))
        };
        let click = |app: &mut App, pos: Pos2| {
            now.set(now.get() + 0.5);
            let _ = step(app, vec![egui::Event::PointerMoved(pos)]);
            let _ = step(app, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE }]);
            let _ = step(app, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE }]);
            let _ = step(app, vec![]);
        };

        let _ = step(&mut app, vec![]);
        let start_r = test_rect(&ctx, "stroke-start-point").expect("start point dropdown");

        // Pick Round on start and Line arrow on end.
        click(&mut app, start_r.center());
        let row1 = test_rect(&ctx, &format!("menu-row-0-{}", option_of(StrokeCap::Round))).expect("Round menu row");
        click(&mut app, row1.center());
        let end_r = test_rect(&ctx, "stroke-end-point").expect("end point dropdown");
        click(&mut app, end_r.center());
        let row3 = test_rect(&ctx, &format!("menu-row-0-{}", option_of(StrokeCap::ArrowLines))).expect("Line arrow menu row");
        click(&mut app, row3.center());
        assert_eq!(end_vertex_caps(app.doc.get(line).unwrap()), Some((Some(StrokeCap::Round), Some(StrokeCap::ArrowLines))));

        // Swap exchanges start and end in one undo step.
        let swap_r = test_rect(&ctx, "stroke-swap-points").expect("swap button");
        click(&mut app, swap_r.center());
        assert_eq!(
            end_vertex_caps(app.doc.get(line).unwrap()),
            Some((Some(StrokeCap::ArrowLines), Some(StrokeCap::Round))),
            "swap exchanges start and end caps"
        );
        app.undo();
        assert_eq!(
            end_vertex_caps(app.doc.get(line).unwrap()),
            Some((Some(StrokeCap::Round), Some(StrokeCap::ArrowLines))),
            "single undo restores pre-swap caps"
        );
    }

    #[test]
    fn stroke_panel_mixed_open_selection_shows_mixed_and_applies_to_all_in_one_undo_step() {
        let mut doc = Document::new("stroke-endpoints-mixed");
        let page = doc.add_page("Page 1");
        let mut app = App::new(doc);
        app.page = page;

        let mk_open_vec = |app: &mut App, end_cap: StrokeCap| -> NodeId {
            let id = app.doc.alloc_id();
            let path = designos_doc::Path {
                cmds: vec![designos_doc::PathCmd::MoveTo(designos_doc::Vec2::ZERO), designos_doc::PathCmd::LineTo(designos_doc::Vec2::new(80.0, 0.0))],
                fill_rule: designos_doc::FillRule::NonZero,
            };
            let mut net = designos_doc::VectorNetwork::from_paths(std::slice::from_ref(&path));
            net.vertices[0].cap = Some(StrokeCap::None);
            net.vertices[1].cap = Some(end_cap);
            let mut n = designos_doc::Node::new(id, NodeKind::Vector { paths: vec![path], network: Some(net) });
            n.size = designos_doc::Vec2::new(80.0, 0.0);
            n.fills.clear();
            n.stroke.paints = vec![Paint::solid(Color::WHITE)];
            n.stroke.weight = 2.0;
            app.doc.insert(page, n).unwrap()
        };

        let v1 = mk_open_vec(&mut app, StrokeCap::ArrowLines);
        let v2 = mk_open_vec(&mut app, StrokeCap::DiamondFilled);
        app.selection = vec![v1, v2];

        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::theme::install_fonts(&ctx);
        let region = Rect::from_min_size(Pos2::new(0.0, 0.0), egui::Vec2::new(240.0, 900.0));
        let test_rect = |ctx: &egui::Context, key: &str| ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", key.to_string()))));
        let step = |app: &mut App, events: Vec<egui::Event>| -> egui::FullOutput {
            let raw = egui::RawInput { screen_rect: Some(region), events, ..Default::default() };
            ctx.run_ui(raw, |ui| super::super::properties::show(app, ui, region))
        };
        let click = |app: &mut App, pos: Pos2| {
            let _ = step(app, vec![egui::Event::PointerMoved(pos)]);
            let _ = step(app, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE }]);
            let _ = step(app, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE }]);
        };

        let out = step(&mut app, vec![]);
        let end_r = test_rect(&ctx, "stroke-end-point").expect("end point dropdown rendered for open selection");
        assert!(shapes_contain_text(&out.shapes, "Mixed"), "differing end caps across selection must show Mixed");

        // Pick Triangle arrow on end point: applies to both v1 and v2 in one undo step.
        click(&mut app, end_r.center());
        let _ = step(&mut app, vec![]);
        let row4 = test_rect(&ctx, &format!("menu-row-0-{}", option_of(StrokeCap::ArrowEquilateral))).expect("Triangle arrow menu row");
        click(&mut app, row4.center());
        assert_eq!(end_vertex_caps(app.doc.get(v1).unwrap()), Some((Some(StrokeCap::None), Some(StrokeCap::ArrowEquilateral))));
        assert_eq!(end_vertex_caps(app.doc.get(v2).unwrap()), Some((Some(StrokeCap::None), Some(StrokeCap::ArrowEquilateral))));

        app.undo();
        assert_eq!(end_vertex_caps(app.doc.get(v1).unwrap()), Some((Some(StrokeCap::None), Some(StrokeCap::ArrowLines))));
        assert_eq!(end_vertex_caps(app.doc.get(v2).unwrap()), Some((Some(StrokeCap::None), Some(StrokeCap::DiamondFilled))));
    }

    #[test]
    fn stroke_panel_hides_endpoint_controls_for_closed_paths() {
        let mut doc = Document::new("stroke-endpoints-closed");
        let page = doc.add_page("Page 1");
        let mut app = App::new(doc);
        app.page = page;

        let rect_id = app.doc.alloc_id();
        let mut rect = designos_doc::Node::rectangle(rect_id, designos_doc::Rect::new(0.0, 0.0, 100.0, 80.0));
        rect.stroke.paints = vec![Paint::solid(Color::WHITE)];
        let rect = app.doc.insert(page, rect).unwrap();

        let closed_vec_id = app.doc.alloc_id();
        let closed_path = designos_doc::Path {
            cmds: vec![
                designos_doc::PathCmd::MoveTo(designos_doc::Vec2::ZERO),
                designos_doc::PathCmd::LineTo(designos_doc::Vec2::new(50.0, 0.0)),
                designos_doc::PathCmd::LineTo(designos_doc::Vec2::new(25.0, 40.0)),
                designos_doc::PathCmd::Close,
            ],
            fill_rule: designos_doc::FillRule::NonZero,
        };
        let closed_net = designos_doc::VectorNetwork::from_paths(std::slice::from_ref(&closed_path));
        let mut closed_vec = designos_doc::Node::new(closed_vec_id, NodeKind::Vector { paths: vec![closed_path], network: Some(closed_net) });
        closed_vec.size = designos_doc::Vec2::new(50.0, 40.0);
        closed_vec.stroke.paints = vec![Paint::solid(Color::WHITE)];
        let closed_vec = app.doc.insert(page, closed_vec).unwrap();

        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::theme::install_fonts(&ctx);
        let region = Rect::from_min_size(Pos2::new(0.0, 0.0), egui::Vec2::new(240.0, 900.0));
        let test_rect = |ctx: &egui::Context, key: &str| ctx.data(|d| d.get_temp::<Rect>(Id::new(("test-rect", key.to_string()))));
        let step = |app: &mut App| {
            let raw = egui::RawInput { screen_rect: Some(region), ..Default::default() };
            let _ = ctx.run_ui(raw, |ui| super::super::properties::show(app, ui, region));
        };

        for sel in [vec![rect], vec![closed_vec], vec![rect, closed_vec]] {
            app.selection = sel;
            step(&mut app);
            assert!(test_rect(&ctx, "stroke-start-point").is_none(), "closed path must not offer stroke-start-point");
            assert!(test_rect(&ctx, "stroke-end-point").is_none(), "closed path must not offer stroke-end-point");
            assert!(test_rect(&ctx, "stroke-swap-points").is_none(), "closed path must not offer stroke-swap-points");
        }
    }
}

#[cfg(test)]
#[path = "tests/stroke_endpoints.rs"]
mod endpoint_tests;
