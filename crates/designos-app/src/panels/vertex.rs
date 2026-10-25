//! Inspector section shown while a vector is in point editing mode: position,
//! corner radius, stroke cap and handle mirroring of the selected vertices,
//! plus Join / Split / Simplify.

use super::properties::{row_end, section_sep, sublabel, title, Pane};
use crate::app::App;
use crate::canvas::vector as vec;
use crate::icons::Icon;
use crate::theme;
use crate::widgets::field::{self, Prefix};
use crate::widgets::menu::dropdown;
use crate::widgets::{self};
use designos_doc::{HandleMirror, StrokeCap};
use egui::{Align2, Rect, Sense, Ui};

/// Entries of the mirroring dropdown. "Straight" removes the handles.
const MIRROR: [&str; 4] = ["Straight", "Mirror angle and length", "Mirror angle", "No mirroring"];
const CAPS: [(&str, Option<StrokeCap>); 4] =
    [("Inherit", None), ("None", Some(StrokeCap::None)), ("Round", Some(StrokeCap::Round)), ("Square", Some(StrokeCap::Square))];

fn text_button(ui: &mut Ui, rect: Rect, key: &str, label: &str, enabled: bool) -> bool {
    widgets::record(ui, key, rect);
    let resp = ui.interact(rect, ui.id().with(("vtx-btn", key)), if enabled { Sense::click() } else { Sense::hover() });
    let bg = if enabled && resp.hovered() { theme::BG_TERTIARY } else { theme::BG_SECONDARY };
    ui.painter().rect_filled(rect, theme::cr(theme::RADIUS_CONTROL), bg);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, theme::font_body(), if enabled { theme::TEXT } else { theme::TEXT_SECONDARY });
    enabled && resp.clicked()
}

/// The vertex section; no-op unless the selected vector is being edited.
pub(super) fn section(app: &mut App, ui: &mut Ui, pane: &mut Pane) {
    let Some(v) = app.canvas.vedit.as_ref() else { return };
    let n_sel = v.sel.len();
    let n_vertices = v.net.vertices.len();
    let title_text = match n_sel {
        0 => "Vertex".to_string(),
        1 => "Vertex".to_string(),
        n => format!("{n} vertices"),
    };
    title(ui, pane, &title_text, None, n_sel == 0);
    if n_sel == 0 {
        let r = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, 32.0);
        ui.painter().text(
            egui::Pos2::new(r.left(), r.top() + 8.0),
            Align2::LEFT_CENTER,
            format!("{n_vertices} vertices. Select one to edit it."),
            theme::font_caption_medium(),
            theme::TEXT_SECONDARY,
        );
        pane.y += 28.0;
    } else {
        sublabel(ui, pane, "Position");
        let xs = vec::common_vertex(app, |v| v.pos.x);
        let ys = vec::common_vertex(app, |v| v.pos.y);
        if let Some(x) = field::num_field(ui, pane.col(0, 0.0), "vtx-x", Prefix::Text("X"), xs, "") {
            vec::move_selected_to(app, Some(x), None);
        }
        if let Some(y) = field::num_field(ui, pane.col(1, 0.0), "vtx-y", Prefix::Text("Y"), ys, "") {
            vec::move_selected_to(app, None, Some(y));
        }
        row_end(pane);

        sublabel(ui, pane, "Corner radius");
        let radius = vec::common_vertex(app, |v| v.corner_radius);
        if let Some(r) = field::num_field(ui, pane.col(0, 0.0), "vtx-radius", Prefix::Icon(Icon::Radius), radius, "") {
            let r = r.max(0.0);
            vec::set_vertices(app, |v| v.corner_radius = r);
        }
        row_end(pane);

        sublabel(ui, pane, "Stroke cap");
        let cap = vec::common_vertex(app, |v| v.cap);
        let caps: Vec<String> = CAPS.iter().map(|c| c.0.to_string()).collect();
        let cur = cap.and_then(|c| CAPS.iter().position(|x| x.1 == c)).map(|i| caps[i].as_str()).unwrap_or("Mixed");
        let cap_r = pane.full(0.0);
        widgets::record(ui, "vtx-cap", cap_r);
        if let Some(i) = dropdown(ui, cap_r, "vtx-cap", cur, &caps) {
            let c = CAPS[i].1;
            vec::set_vertices(app, |v| v.cap = c);
        }
        row_end(pane);

        sublabel(ui, pane, "Handles");
        let mode = vec::common_vertex(app, |v| v.mirror);
        let straight = app
            .canvas
            .vedit
            .as_ref()
            .is_some_and(|v| v.sel.iter().all(|i| v.net.ends_at(*i).iter().all(|(s, e)| v.net.tangent(*s, *e) == designos_doc::Vec2::ZERO)));
        let shown = if straight {
            MIRROR[0]
        } else {
            match mode {
                Some(HandleMirror::AngleAndLength) => MIRROR[1],
                Some(HandleMirror::Angle) => MIRROR[2],
                Some(HandleMirror::None) => MIRROR[3],
                None => "Mixed",
            }
        };
        let options: Vec<String> = MIRROR.iter().map(|s| s.to_string()).collect();
        let mr = pane.full(0.0);
        widgets::record(ui, "vtx-mirror", mr);
        if let Some(i) = dropdown(ui, mr, "vtx-mirror", shown, &options) {
            match i {
                0 => {
                    vec::straighten_selected(app);
                }
                1 => {
                    vec::set_mirror(app, HandleMirror::AngleAndLength);
                }
                2 => {
                    vec::set_mirror(app, HandleMirror::Angle);
                }
                _ => {
                    vec::set_mirror(app, HandleMirror::None);
                }
            }
        }
        row_end(pane);
    }
    // path commands
    let third = (theme::SECTION_CONTENT_W - 8.0) / 3.0;
    let btn = |i: usize, pane: &Pane| pane.rect(theme::SECTION_CONTENT_X + i as f32 * (third + 4.0), 0.0, third, theme::FIELD_HEIGHT);
    if text_button(ui, btn(0, pane), "vtx-join", "Join", n_sel == 2) {
        vec::join_selected(app);
    }
    let seg_sel = app.canvas.vedit.as_ref().is_some_and(|v| v.seg.is_some());
    if text_button(ui, btn(1, pane), "vtx-split", "Split", n_sel > 0 || seg_sel) {
        vec::split_selected(app);
    }
    if text_button(ui, btn(2, pane), "vtx-simplify", "Simplify", true) {
        vec::simplify(app, crate::vecops::SIMPLIFY_TOLERANCE);
    }
    row_end(pane);
    pane.y += 4.0;
    section_sep(ui, pane, false);
}
