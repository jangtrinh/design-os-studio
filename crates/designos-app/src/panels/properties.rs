//! Right panel content: page settings when nothing is selected, otherwise the
//! inspector (alignment, position, layout, appearance, fill, stroke, effects).

use super::{clipped, scrollbar, var_bind, wheel_scroll};
use crate::app::App;
use crate::edit::Tx;
use crate::icons::{self, Icon};
use crate::ops::{apply_resize, Align};
use crate::styles::StyleKind;
use crate::theme;
use crate::util::v2;
use crate::widgets::field::{self, Prefix};
use crate::widgets::menu::{self, dropdown, MenuItem};
use crate::widgets::{self, fmt_num};
use designos_doc::Vec2 as DVec2;
use designos_doc::*;
use egui::{Align2, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickTarget {
    Fill(usize),
    Stroke(usize),
    PageBg,
    Shadow(usize),
    /// A colour from the Selection colors list (rgb of the colour being replaced).
    SelColor(u32),
    /// The colour of text (the edited character range, or every run of the selected text layers).
    TextFill,
    /// Colour of the i-th layout grid of a frame.
    GridColor(usize),
    /// Colour (or second colour) of the i-th effect, a noise effect.
    NoiseColor(usize, bool),
}

pub type EyedropTarget = PickTarget;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SelectedProperty {
    Fill(usize),
    Stroke(usize),
    Effect(usize),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EyedropperFormat {
    #[default]
    Hex,
    Rgb,
    Hsl,
    Hsb,
}

impl EyedropperFormat {
    pub fn next(self) -> Self {
        match self {
            Self::Hex => Self::Rgb,
            Self::Rgb => Self::Hsl,
            Self::Hsl => Self::Hsb,
            Self::Hsb => Self::Hex,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Hex => "HEX",
            Self::Rgb => "RGB",
            Self::Hsl => "HSL",
            Self::Hsb => "HSB",
        }
    }

    pub fn format_rgb(self, r: u8, g: u8, b: u8) -> String {
        match self {
            Self::Hex => format!("#{r:02X}{g:02X}{b:02X}"),
            Self::Rgb => format!("rgb({r}, {g}, {b})"),
            Self::Hsl => {
                let (rf, gf, bf) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
                let max = rf.max(gf).max(bf);
                let min = rf.min(gf).min(bf);
                let l = (max + min) * 0.5;
                let d = max - min;
                let (h, s) = if d.abs() < 1e-5 {
                    (0.0, 0.0)
                } else {
                    let s = d / (1.0 - (2.0 * l - 1.0).abs()).max(1e-5);
                    let h = if (max - rf).abs() < 1e-5 {
                        60.0 * (((gf - bf) / d).rem_euclid(6.0))
                    } else if (max - gf).abs() < 1e-5 {
                        60.0 * (((bf - rf) / d) + 2.0)
                    } else {
                        60.0 * (((rf - gf) / d) + 4.0)
                    };
                    (h, s)
                };
                format!("hsl({}°, {}%, {}%)", h.round() as i32, (s * 100.0).round() as i32, (l * 100.0).round() as i32)
            }
            Self::Hsb => {
                let (rf, gf, bf) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
                let max = rf.max(gf).max(bf);
                let min = rf.min(gf).min(bf);
                let d = max - min;
                let s = if max < 1e-5 { 0.0 } else { d / max };
                let h = if d.abs() < 1e-5 {
                    0.0
                } else if (max - rf).abs() < 1e-5 {
                    60.0 * (((gf - bf) / d).rem_euclid(6.0))
                } else if (max - gf).abs() < 1e-5 {
                    60.0 * (((bf - rf) / d) + 2.0)
                } else {
                    60.0 * (((rf - gf) / d) + 4.0)
                };
                format!("{} format · {}° {}% {}%", self.label(), h.round() as i32, (s * 100.0).round() as i32, (max * 100.0).round() as i32)
            }
        }
    }
}

pub struct Picker {
    pub target: PickTarget,
    pub tx: Tx,
    pub anchor: Rect,
    pub ids: Vec<NodeId>,
    /// The paint being edited (solid colour with opacity as alpha, or a gradient).
    pub paint: Paint,
    pub state: crate::widgets::picker::State,
    /// Swatch sources gathered when the picker opened.
    pub sources: Vec<(String, Vec<Color32>)>,
    /// Waiting for a click on the canvas to sample a colour.
    pub eyedrop: bool,
}

#[derive(Default)]
pub struct PropsState {
    pub scroll: f32,
    pub content_h: f32,
    pub picker: Option<Picker>,
    /// Active standalone or picker-linked canvas eyedropper target (`I` shortcut).
    pub eyedropper: Option<PickTarget>,
    /// Active eyedropper HUD color readout format (`Tab` cycles Hex -> Rgb -> Hsl -> Hsb).
    pub eyedropper_format: EyedropperFormat,
    /// Focused Fill / Stroke / Effect row in the right-hand Inspector (`Delete` removes, `Cmd+V` pastes fill).
    pub selected_property: Option<SelectedProperty>,
    /// Fill / Stroke / Effect row currently being dragged vertically to reorder.
    pub drag_property: Option<SelectedProperty>,
    /// Collapsed style folders in the Styles section.
    pub folded: std::collections::HashSet<String>,
    /// Page background opacity before the visibility toggle hid it.
    pub page_alpha_prev: f32,
    /// Open settings popover (effect or stroke).
    pub pop: Option<super::effects::PopState>,
    /// The Effects "+" menu is open.
    pub fx_menu: bool,
    /// The paint styles menu (4-dots button) is open: Some(for_stroke).
    pub styles_menu: Option<bool>,
    /// The style menu of the Effects (`Effect`) or Layout guide (`Grid`) header is open.
    pub style_menu: Option<StyleProperty>,
    /// The name of the style the Effects or Layout guide row shows is being edited.
    pub style_rename: Option<StyleProperty>,
    /// The "Edit style" popover.
    pub style_edit: Option<super::style_edit::StyleEdit>,
    /// Cached `(selection_hash, tile_rev, unique_colors, usage_counts)` for Selection colors.
    pub sel_colors_cache: Option<(u64, u64, Vec<Color>, Vec<usize>)>,
    /// Cached `(doc_node_count_and_epoch, sorted_component_ids)` for instance swap menus.
    pub all_components_cache: Option<(u64, Vec<NodeId>)>,
}

pub(super) struct Pane {
    pub(super) x0: f32,
    pub(super) oy: f32,
    pub(super) y: f32,
    /// `y` right after the latest section title: a label there follows a heading, not a field.
    pub(super) title_end: f32,
}

impl Pane {
    pub(super) fn rect(&self, x: f32, dy: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(Pos2::new(self.x0 + x, self.oy + self.y + dy), Vec2::new(w, h))
    }
    pub(super) fn col(&self, i: usize, dy: f32) -> Rect {
        self.rect(theme::FIELD_COL_X[i], dy, theme::FIELD_WIDTH_HALF, theme::FIELD_HEIGHT)
    }
    pub(super) fn full(&self, dy: f32) -> Rect {
        self.rect(theme::SECTION_CONTENT_X, dy, theme::SECTION_CONTENT_W, theme::FIELD_HEIGHT)
    }
}

pub(super) fn to_c32(c: Color, opacity: f32) -> Color32 {
    let f = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgba_unmultiplied(f(c.r), f(c.g), f(c.b), f(c.a * opacity))
}

#[cfg(test)]
fn from_c32(c: Color32) -> Color {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    Color::rgba(r, g, b, a)
}

pub(super) fn section_sep(ui: &Ui, pane: &mut Pane, first: bool) {
    if !first {
        widgets::hline(ui.painter(), pane.x0, pane.x0 + theme::RIGHT_PANEL_WIDTH, pane.oy + pane.y, theme::BORDER);
        pane.y += 1.0;
    }
}

/// Section title row; returns true when its action button was clicked.
pub(super) fn title(ui: &mut Ui, pane: &mut Pane, text: &str, action: Option<Icon>, dim: bool) -> bool {
    let row = pane.rect(0.0, 0.0, theme::RIGHT_PANEL_WIDTH - 1.0, theme::SECTION_TITLE_HEIGHT);
    ui.painter().text(
        Pos2::new(row.left() + theme::SECTION_CONTENT_X, row.center().y),
        Align2::LEFT_CENTER,
        text,
        theme::font_body_strong(),
        if dim { theme::TEXT_SECONDARY } else { theme::TEXT },
    );
    let mut clicked = false;
    if let Some(ic) = action {
        let b = pane.rect(theme::SECTION_TRAILING_X, 8.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
        clicked = widgets::icon_button(ui, b, ("title-action", text), ic, false, "").clicked();
    }
    pane.y += theme::SECTION_TITLE_HEIGHT;
    pane.title_end = pane.y;
    clicked
}

/// Fill / Stroke section header: title, the styles button (4 dots) and "+". Returns true when
/// "+" was clicked.
pub(super) fn paint_title(app: &mut App, ui: &mut Ui, pane: &mut Pane, sel: &[NodeId], text: &str, stroke: bool, dim: bool) -> bool {
    title(ui, pane, text, None, dim);
    let y = -theme::SECTION_TITLE_HEIGHT + 8.0;
    let plus = pane.rect(theme::SECTION_TRAILING_X, y, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
    let styles = pane.rect(theme::SECTION_TRAILING_X - 28.0, y, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
    let clicked = widgets::icon_button(ui, plus, ("title-action", text), Icon::Plus, false, "").clicked();
    let open = app.props.styles_menu == Some(stroke);
    if widgets::icon_button(ui, styles, ("title-styles", text), Icon::Styles, open, "Apply style").clicked() {
        app.props.styles_menu = (!open).then_some(stroke);
    }
    if open {
        let want = if stroke { StyleProperty::Stroke } else { StyleProperty::Fill };
        let entries: Vec<(String, String)> = crate::styles::style_entries(&app.doc, false)
            .into_iter()
            .filter(|e| app.doc.style_meta.get(&e.key).map(|m| m.property).unwrap_or(StyleProperty::Fill) == want)
            .map(|e| (e.key.clone(), app.doc.style_name(&e.key).to_string()))
            .collect();
        let mut items: Vec<crate::widgets::menu::MenuItem> = if entries.is_empty() {
            vec![crate::widgets::menu::MenuItem::new("No color styles").disabled(true)]
        } else {
            entries.iter().map(|(_, n)| crate::widgets::menu::MenuItem::new(n.clone())).collect()
        };
        // The style this selection is linked to can be edited from here.
        let linked = sel
            .first()
            .and_then(|s| app.doc.get(*s))
            .and_then(|n| n.style_refs.get(if stroke { "stroke" } else { "fill" }))
            .filter(|k| app.doc.styles.contains_key(*k))
            .cloned();
        if let Some(last) = items.last_mut() {
            last.separator_after = true;
        }
        let edit_row = items.len();
        let read_only = linked.as_deref().is_some_and(|k| super::style_edit::is_read_only(app, k));
        let edit_label = if read_only { super::style_edit::LIBRARY_STYLE_TIP } else { "Edit style" };
        items.push(crate::widgets::menu::MenuItem::new(edit_label).disabled(linked.is_none() || read_only));
        let pos = Pos2::new(styles.right() - 168.0, styles.bottom() + 4.0);
        match crate::widgets::menu::popup_menu(ui.ctx(), egui::Id::new(("styles-menu", stroke)), pos, 168.0, &items, Some(styles)) {
            crate::widgets::menu::MenuResult::Picked(i) => {
                app.props.styles_menu = None;
                if i == edit_row {
                    if let Some(key) = &linked {
                        super::style_edit::toggle(app, key, styles);
                    }
                } else if let Some((key, _)) = entries.get(i) {
                    app.apply_style(sel, key);
                }
            }
            crate::widgets::menu::MenuResult::Closed => app.props.styles_menu = None,
            crate::widgets::menu::MenuResult::Open => {}
        }
    }
    clicked
}

/// Label of a field group: `SPACE_GROUP` below the field above, `SPACE_FACT` above the field it names.
/// A row already ends with `SPACE_ROW`, so only the difference is added; right after a title nothing is.
pub(super) fn sublabel(ui: &Ui, pane: &mut Pane, text: &str) {
    if pane.y > pane.title_end {
        pane.y += theme::SPACE_GROUP - theme::SPACE_ROW;
    }
    let r = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, theme::LABEL_BOX_H);
    ui.painter().text(Pos2::new(r.left(), r.center().y), Align2::LEFT_CENTER, text, theme::font_caption_medium(), theme::TEXT_SECONDARY);
    pane.y += theme::LABEL_BOX_H + theme::SPACE_FACT;
}

/// Row advance after a 24px field row.
pub(super) fn row_end(pane: &mut Pane) {
    pane.y += theme::FIELD_HEIGHT + 4.0;
}

pub(super) fn segmented(ui: &mut Ui, pane: &Pane, x: f32, key: &str, items: &[Icon], selected: Option<usize>) -> Option<usize> {
    segmented_marks(ui, pane, x, key, items, selected.as_slice())
}

/// [`segmented`] where several segments can be lit at once (a direction plus the wrap toggle).
fn segmented_marks(ui: &mut Ui, pane: &Pane, x: f32, key: &str, items: &[Icon], marked: &[usize]) -> Option<usize> {
    let mut out = None;
    let n = items.len();
    let seg_w = if n == 4 { 22.0 } else { theme::SEGMENT_W };
    for (i, icon) in items.iter().enumerate() {
        let r = pane.rect(x + i as f32 * seg_w, 0.0, seg_w, theme::FIELD_HEIGHT);
        let radius = match (i == 0, i + 1 == n) {
            (true, true) => theme::cr(theme::RADIUS_CONTROL),
            (true, false) => theme::cr_each(theme::RADIUS_CONTROL, 0, 0, theme::RADIUS_CONTROL),
            (false, true) => theme::cr_each(0, theme::RADIUS_CONTROL, theme::RADIUS_CONTROL, 0),
            _ => theme::cr(0),
        };
        let resp = ui.interact(r, ui.id().with(("seg", key, i)), Sense::click());
        let sel = marked.contains(&i);
        let bg = if sel {
            theme::BG_TERTIARY
        } else if resp.hovered() {
            theme::BG_TERTIARY.gamma_multiply(0.6)
        } else {
            theme::BG_SECONDARY
        };
        ui.painter().rect_filled(r, radius, bg);
        icons::paint(ui.painter(), *icon, r.center(), 16.0, if sel { theme::ICON } else { theme::ICON_SECONDARY });
        if i > 0 {
            // 1px gap between neighbouring segments.
            widgets::vline(ui.painter(), r.left(), r.top(), r.bottom(), theme::BG);
        }
        if resp.clicked() {
            out = Some(i);
        }
    }
    out
}

pub(super) fn checkbox(ui: &mut Ui, pane: &mut Pane, key: &str, label: &str, value: bool) -> Option<bool> {
    let r = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, theme::FIELD_HEIGHT);
    let resp = ui.interact(r, ui.id().with(("chk", key)), Sense::click());
    let b = Rect::from_min_size(Pos2::new(r.left(), r.center().y - 7.0), Vec2::splat(14.0));
    let p = ui.painter();
    if value {
        p.rect_filled(b, theme::cr(theme::RADIUS_MARK), theme::BG_BRAND);
        icons::paint(p, Icon::Check, b.center(), 12.0, theme::TEXT);
    } else {
        p.rect_stroke(b, theme::cr(theme::RADIUS_MARK), Stroke::new(1.0, theme::TEXT_TERTIARY), StrokeKind::Inside);
    }
    p.text(Pos2::new(b.right() + 8.0, r.center().y), Align2::LEFT_CENTER, label, theme::font_body(), theme::TEXT);
    pane.y += theme::FIELD_HEIGHT + 4.0;
    resp.clicked().then_some(!value)
}

/// Common value of `f` over all selected nodes, or None if they differ.
fn common<T: PartialEq>(app: &App, sel: &[NodeId], f: impl Fn(&Node) -> T) -> Option<T> {
    let mut it = sel.iter().take(128).filter_map(|id| app.doc.get(*id)).map(f);
    let first = it.next()?;
    it.all(|v| v == first).then_some(first)
}

pub fn show(app: &mut App, ui: &mut Ui, region: Rect) {
    let mut scroll = app.props.scroll;
    wheel_scroll(ui, region, &mut scroll, app.props.content_h);
    app.props.scroll = scroll;
    let sel = app.selection.clone();
    let end = clipped(ui, region, |ui| {
        // Spec x values are relative to the panel's outer left edge (border at x=0).
        let mut pane = Pane { x0: region.left() - theme::BORDER_WIDTH, oy: region.top() - scroll, y: 0.0, title_end: 0.0 };
        if sel.is_empty() {
            page_section(app, ui, &mut pane);
            styles_section(app, ui, &mut pane);
        } else {
            selection_ui(app, ui, &mut pane, &sel);
        }
        pane.y + 24.0
    });
    app.props.content_h = end;
    scrollbar(ui, region, scroll, end);
    if let Some(s) = super::take_scrollbar_drag(ui, region) {
        app.props.scroll = s;
    }
    picker_popup(app, ui, region);
    super::effects::popover_ui(app, ui, region);
    super::style_edit::popover_ui(app, ui, region);
}

// ---------------------------------------------------------------- no selection
/// Page background row: swatch, hex and opacity in one 184x24 field group plus a visibility toggle.
fn page_section(app: &mut App, ui: &mut Ui, pane: &mut Pane) {
    let settings = title(ui, pane, "Page", Some(Icon::Opacity), false);
    let bg = match app.doc.get(app.page).map(|n| &n.kind) {
        Some(NodeKind::Page { background }) => *background,
        _ => Color::WHITE,
    };
    let g = pane.rect(theme::SECTION_CONTENT_X, theme::PAGE_FIELD_DY, theme::SECTION_CONTENT_W, theme::FIELD_HEIGHT);
    let hex_id = ui.id().with(("txt", "page-hex"));
    let op_id = ui.id().with(("num", "page-op"));
    let focused = ui.memory(|m| m.has_focus(hex_id) || m.has_focus(op_id));
    {
        let p = ui.painter();
        p.rect_filled(g, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
        if focused {
            p.rect_stroke(g, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, theme::BORDER_SELECTED), StrokeKind::Inside);
        } else if ui.rect_contains_pointer(g) {
            p.rect_stroke(g, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);
        }
    }
    let sw = Rect::from_center_size(Pos2::new(g.left() + theme::PAGE_SWATCH_CX, g.center().y), Vec2::splat(theme::PAGE_SWATCH));
    let swr = ui.interact(sw.expand(2.0), ui.id().with("page-swatch"), Sense::click());
    widgets::swatch(ui.painter(), sw, to_c32(bg, 1.0));
    if swr.clicked() || settings {
        open_picker(app, PickTarget::PageBg, to_c32(bg, 1.0), sw, vec![app.page]);
    }
    let hex_r = Rect::from_min_max(Pos2::new(g.left() + theme::PAGE_HEX_X, g.top()), Pos2::new(g.left() + theme::PAGE_DIVIDER_X, g.bottom()));
    if let Some(s) = field::edit_hex_text(ui, hex_r, hex_id, &bg.to_hex(), theme::font_body(), theme::TEXT, 0.0) {
        if let Some(c) = Color::from_hex(&s) {
            let page = app.page;
            app.edit_nodes(&[page], |n| n.kind = NodeKind::Page { background: Color { a: bg.a, ..c } });
        }
    }
    widgets::vline(ui.painter(), g.left() + theme::PAGE_DIVIDER_X, g.top(), g.bottom(), theme::BG);
    let op_r = Rect::from_min_max(Pos2::new(g.left() + theme::PAGE_DIVIDER_X + 1.0, g.top()), Pos2::new(g.right() - theme::PAGE_PERCENT_W, g.bottom()));
    let op_val = (bg.a * 100.0).round();
    if let Some(v) = field::edit_num_text(ui, op_r, op_id, Some(op_val), true, theme::font_body(), theme::TEXT, theme::PAGE_OPACITY_PAD, None) {
        let page = app.page;
        let a = (v / 100.0).clamp(0.0, 1.0);
        app.edit_nodes(&[page], |n| n.kind = NodeKind::Page { background: Color { a, ..bg } });
    }
    ui.painter().text(Pos2::new(g.right() - theme::PAGE_PERCENT_RIGHT, g.center().y), Align2::RIGHT_CENTER, "%", theme::font_body(), theme::TEXT);
    // Visibility toggle: hiding sets the background opacity to 0 and remembers the old value.
    let eye = pane.rect(theme::SECTION_TRAILING_X, theme::PAGE_FIELD_DY, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
    let hidden = bg.a <= 0.0;
    if widgets::icon_button(
        ui,
        eye,
        "page-visible",
        if hidden { Icon::EyeOff } else { Icon::Eye },
        false,
        if hidden { "Show background" } else { "Hide background" },
    )
    .clicked()
    {
        let page = app.page;
        let a = if hidden { app.props.page_alpha_prev.max(0.01) } else { 0.0 };
        if !hidden {
            app.props.page_alpha_prev = bg.a;
        }
        app.edit_nodes(&[page], |n| n.kind = NodeKind::Page { background: Color { a, ..bg } });
    }
    pane.y += theme::PAGE_FIELD_DY + theme::FIELD_HEIGHT + theme::PAGE_BOTTOM_PAD;
    section_sep(ui, pane, false);
}

fn styles_section(app: &mut App, ui: &mut Ui, pane: &mut Pane) {
    title(ui, pane, "Styles", Some(Icon::Plus), false);
    let before = pane.y;
    styles_list(app, ui, pane, &[]);
    if pane.y > before {
        pane.y += theme::STYLES_BOTTOM_PAD;
    }
    section_sep(ui, pane, false);
    title(ui, pane, "Export", Some(Icon::Plus), true);
    section_sep(ui, pane, false);
}

/// Colours of a paint (one for solids, the stops for gradients).
fn paint_colors(p: &Paint) -> Vec<Color32> {
    match &p.kind {
        PaintKind::Solid(c) => vec![to_c32(*c, p.opacity)],
        PaintKind::LinearGradient { stops, .. }
        | PaintKind::RadialGradient { stops, .. }
        | PaintKind::AngularGradient { stops, .. }
        | PaintKind::DiamondGradient { stops, .. } => stops.iter().map(|g| to_c32(g.color, p.opacity)).collect(),
        PaintKind::Image { .. } => vec![Color32::GRAY],
    }
}

/// Text and colour styles grouped in folders by their "/" prefix. Clicking a
/// row applies the style to `target`.
fn styles_list(app: &mut App, ui: &mut Ui, pane: &mut Pane, target: &[NodeId]) {
    let applied = |app: &App, prop: &str| target.first().and_then(|t| app.doc.get(*t)).and_then(|n| n.style_refs.get(prop).cloned());
    let mut shown_groups = 0;
    for (label, kind) in
        [("Text styles", StyleKind::Text), ("Color styles", StyleKind::Color), ("Effect styles", StyleKind::Effect), ("Grid styles", StyleKind::Grid)]
    {
        let entries = crate::styles::style_entries_of(&app.doc, kind);
        if entries.is_empty() {
            continue;
        }
        pane.y += if shown_groups == 0 { theme::STYLES_LABEL_GAP } else { theme::STYLES_GROUP_GAP };
        shown_groups += 1;
        let lr = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, theme::SUBLABEL_HEIGHT);
        ui.painter().text(Pos2::new(lr.left(), lr.center().y), Align2::LEFT_CENTER, label, theme::font_body(), theme::TEXT_SECONDARY);
        pane.y += theme::SUBLABEL_HEIGHT + theme::STYLES_ROW_GAP;
        let current = applied(app, kind.ref_key());
        let mut last_folder: Option<String> = None;
        for e in &entries {
            let folder_key = format!("{label}/{}", e.folder);
            if !e.folder.is_empty() && last_folder.as_deref() != Some(e.folder.as_str()) {
                let r = pane.rect(8.0, 0.0, theme::RIGHT_PANEL_WIDTH - 17.0, theme::STYLE_FOLDER_ROW_H);
                widgets::record(ui, &format!("folder:{folder_key}"), r);
                let resp = ui.interact(r, ui.id().with(("sfolder", &folder_key)), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
                }
                let open = !app.props.folded.contains(&folder_key);
                icons::paint(
                    ui.painter(),
                    if open { Icon::ChevronDown } else { Icon::ChevronRight },
                    Pos2::new(r.left() + 14.0, r.center().y),
                    12.0,
                    theme::ICON_TERTIARY,
                );
                ui.painter().text(Pos2::new(r.left() + 28.0, r.center().y), Align2::LEFT_CENTER, &e.folder, theme::font_body(), theme::TEXT_SECONDARY);
                if resp.clicked() {
                    if open {
                        app.props.folded.insert(folder_key.clone());
                    } else {
                        app.props.folded.remove(&folder_key);
                    }
                }
                pane.y += theme::STYLE_FOLDER_ROW_H;
            }
            last_folder = Some(e.folder.clone());
            if !e.folder.is_empty() && app.props.folded.contains(&folder_key) {
                continue;
            }
            let indent = if e.folder.is_empty() { 0.0 } else { theme::STYLE_INDENT };
            let r = pane.rect(8.0, 0.0, theme::RIGHT_PANEL_WIDTH - 17.0, theme::STYLE_ROW_H);
            widgets::record(ui, &format!("style:{}", e.key), r);
            let resp = ui.interact(r, ui.id().with((kind.ref_key(), &e.key)), Sense::click());
            let is_current = current.as_deref() == Some(e.key.as_str());
            if is_current {
                ui.painter().rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_SELECTED);
            } else if resp.hovered() {
                ui.painter().rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
            }
            if matches!(kind, StyleKind::Effect | StyleKind::Grid) {
                let icon = match app.doc.styles.get(&e.key) {
                    Some(Style::Effect(fx)) => fx.first().map_or(Icon::DropShadow, |f| super::effects::KINDS[super::effects::kind_of(f)].1),
                    _ => Icon::Grid2x2,
                };
                icons::paint(
                    ui.painter(),
                    icon,
                    Pos2::new(r.left() + theme::STYLE_SWATCH_CX + indent, r.center().y),
                    theme::STYLE_SWATCH,
                    theme::ICON_SECONDARY,
                );
                let nm = widgets::elide(ui.painter(), &e.leaf, &theme::font_body(), 130.0 - indent);
                ui.painter().text(Pos2::new(r.left() + theme::STYLE_LABEL_X + indent, r.center().y), Align2::LEFT_CENTER, nm, theme::font_body(), theme::TEXT);
            } else if kind == StyleKind::Text {
                let Some(Style::Text(t)) = app.doc.styles.get(&e.key).cloned() else { continue };
                ui.painter().text(Pos2::new(r.left() + 10.0 + indent, r.center().y), Align2::LEFT_CENTER, "Ag", theme::font_body_strong(), theme::TEXT);
                let nm = widgets::elide(ui.painter(), &e.leaf, &theme::font_body(), 110.0 - indent);
                ui.painter().text(Pos2::new(r.left() + 36.0 + indent, r.center().y), Align2::LEFT_CENTER, nm, theme::font_body(), theme::TEXT);
                let lh = match t.line_height {
                    LineHeight::Pixels(p) => fmt_num(p),
                    LineHeight::Percent(p) => fmt_num(t.font_size * p / 100.0),
                    LineHeight::Auto => fmt_num(t.font_size),
                };
                ui.painter().text(
                    Pos2::new(r.right() - 10.0, r.center().y),
                    Align2::RIGHT_CENTER,
                    format!("{}/{}", fmt_num(t.font_size), lh),
                    theme::font_body(),
                    theme::TEXT_TERTIARY,
                );
            } else {
                let Some(Style::Fill(paints)) = app.doc.styles.get(&e.key).cloned() else { continue };
                let c = Pos2::new(r.left() + theme::STYLE_SWATCH_CX + indent, r.center().y);
                let cols = paints.first().map(paint_colors).filter(|c| !c.is_empty()).unwrap_or_else(|| vec![Color32::GRAY]);
                widgets::swatch_circle(ui.painter(), c, theme::STYLE_SWATCH / 2.0, &cols);
                let nm = widgets::elide(ui.painter(), &e.leaf, &theme::font_body(), 130.0 - indent);
                ui.painter().text(Pos2::new(r.left() + theme::STYLE_LABEL_X + indent, r.center().y), Align2::LEFT_CENTER, nm, theme::font_body(), theme::TEXT);
            }
            // "Edit style" appears on the row under the pointer (and stays while its popover is open).
            let editing = app.props.style_edit.as_ref().is_some_and(|s| s.key == e.key);
            let mut edit_clicked = false;
            if editing || ui.input(|i| i.pointer.hover_pos()).is_some_and(|p| r.contains(p)) {
                let edit_r = Rect::from_min_size(
                    Pos2::new(r.right() - theme::PANEL_ACTION_BUTTON - 4.0, r.center().y - theme::PANEL_ACTION_BUTTON / 2.0),
                    Vec2::splat(theme::PANEL_ACTION_BUTTON),
                );
                widgets::record(ui, &format!("style-edit:{}", e.key), edit_r);
                ui.painter().rect_filled(
                    edit_r.expand2(Vec2::new(4.0, 0.0)),
                    theme::cr(theme::RADIUS_CONTROL),
                    if is_current { theme::BG_SELECTED } else { theme::BG_HOVER },
                );
                edit_clicked = super::style_edit::edit_button(app, ui, edit_r, ("style-edit", &e.key), &e.key);
                if edit_clicked {
                    super::style_edit::toggle(app, &e.key, edit_r);
                }
            }
            if resp.clicked() && !edit_clicked && !target.is_empty() {
                app.apply_style(target, &e.key);
            }
            pane.y += theme::STYLE_ROW_H;
        }
    }
    library_styles_list(app, ui, pane, target);
}

/// Styles of enabled libraries this file has not used yet, one group per
/// library. Clicking one copies it into the file and applies it to `target`.
fn library_styles_list(app: &mut App, ui: &mut Ui, pane: &mut Pane, target: &[NodeId]) {
    let all: Vec<crate::libraries::LibraryStyle> =
        app.library_styles().into_iter().filter(|s| s.local_key.is_none() && matches!(s.style, Style::Text(_) | Style::Fill(_))).collect();
    let mut libs: Vec<(String, String)> = all.iter().map(|s| (s.library_id.clone(), s.library_name.clone())).collect();
    libs.dedup();
    for (lib_id, lib_name) in libs {
        for (label, text) in [("Text styles", true), ("Color styles", false)] {
            let rows: Vec<&crate::libraries::LibraryStyle> = all.iter().filter(|s| s.library_id == lib_id && s.text == text).collect();
            if rows.is_empty() {
                continue;
            }
            pane.y += theme::STYLES_GROUP_GAP;
            let lr = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, theme::SUBLABEL_HEIGHT);
            let head = widgets::elide(ui.painter(), &format!("{lib_name} \u{b7} {label}"), &theme::font_body(), lr.width());
            ui.painter().text(Pos2::new(lr.left(), lr.center().y), Align2::LEFT_CENTER, head, theme::font_body(), theme::TEXT_SECONDARY);
            pane.y += theme::SUBLABEL_HEIGHT + theme::STYLES_ROW_GAP;
            for s in rows {
                let r = pane.rect(8.0, 0.0, theme::RIGHT_PANEL_WIDTH - 17.0, theme::STYLE_ROW_H);
                widgets::record(ui, &format!("libstyle:{}", s.name), r);
                let resp = ui.interact(r, ui.id().with(("libstyle", &s.library_id, &s.key)), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(r, theme::cr(theme::RADIUS_CONTROL), theme::BG_HOVER);
                }
                let (_, leaf) = crate::styles::split_name(&s.name);
                match &s.style {
                    Style::Fill(paints) => {
                        let c = Pos2::new(r.left() + theme::STYLE_SWATCH_CX, r.center().y);
                        let cols = paints.first().map(paint_colors).filter(|c| !c.is_empty()).unwrap_or_else(|| vec![Color32::GRAY]);
                        widgets::swatch_circle(ui.painter(), c, theme::STYLE_SWATCH / 2.0, &cols);
                        let nm = widgets::elide(ui.painter(), &leaf, &theme::font_body(), 130.0);
                        ui.painter().text(Pos2::new(r.left() + theme::STYLE_LABEL_X, r.center().y), Align2::LEFT_CENTER, nm, theme::font_body(), theme::TEXT);
                    }
                    _ => {
                        ui.painter().text(Pos2::new(r.left() + 10.0, r.center().y), Align2::LEFT_CENTER, "Ag", theme::font_body_strong(), theme::TEXT);
                        let nm = widgets::elide(ui.painter(), &leaf, &theme::font_body(), 110.0);
                        ui.painter().text(Pos2::new(r.left() + 36.0, r.center().y), Align2::LEFT_CENTER, nm, theme::font_body(), theme::TEXT);
                    }
                }
                if resp.clicked() && !target.is_empty() {
                    if let Some(key) = app.use_library_style(&s.library_id, &s.key) {
                        app.apply_style(target, &key);
                    }
                }
                pane.y += theme::STYLE_ROW_H;
            }
        }
    }
}

/// Unique colours of the selection subtree; editing one replaces it everywhere.
fn selection_colors_section(app: &mut App, ui: &mut Ui, pane: &mut Pane, sel: &[NodeId]) {
    if matches!(app.canvas.inter, crate::canvas::Interaction::Marquee { .. }) && sel.len() > 8 {
        return;
    }
    let sel_hash = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        sel.hash(&mut h);
        h.finish()
    };
    let rev = app.canvas.tiles.rev();
    let (cols, counts) = match &app.props.sel_colors_cache {
        Some((sh, r, cols, counts)) if *sh == sel_hash && *r == rev && !app.canvas.dirty => (cols.clone(), counts.clone()),
        _ => {
            let (cols, counts) = crate::styles::selection_colors_with_counts(&app.doc, sel, crate::styles::MAX_COLOR_WALK_NODES);
            app.props.sel_colors_cache = Some((sel_hash, rev, cols.clone(), counts.clone()));
            (cols, counts)
        }
    };
    let nested = sel.iter().take(64).any(|s| app.doc.get(*s).map(|n| !n.children.is_empty()).unwrap_or(false));
    if cols.is_empty() || !(cols.len() >= 2 || sel.len() > 1 || nested) {
        return;
    }
    section_sep(ui, pane, false);
    title(ui, pane, "Selection colors", None, false);
    for (i, c) in cols.iter().enumerate() {
        let sw = pane.rect(theme::SWATCH_ROW_X, 4.0, theme::SWATCH_SIZE, theme::SWATCH_SIZE);
        let hex_r = pane.rect(theme::SWATCH_ROW_X + theme::SWATCH_SIZE + 8.0, 0.0, 96.0, theme::FIELD_HEIGHT);
        let op_r = pane.rect(theme::SWATCH_ROW_X + theme::SWATCH_SIZE + 8.0 + 100.0, 0.0, 52.0, theme::FIELD_HEIGHT);
        widgets::record(ui, &format!("selsw:{i}"), sw);
        widgets::record(ui, &format!("selhex:{i}"), hex_r);
        let swr = ui.interact(sw, ui.id().with(("selsw", i)), Sense::click());
        widgets::swatch(ui.painter(), sw, to_c32(*c, 1.0));
        let hex_str = c.to_hex();
        if swr.clicked() {
            let rgb = crate::styles::rgb_key(c);
            open_picker(app, PickTarget::SelColor(rgb), to_c32(*c, 1.0), sw, sel.to_vec());
        }
        if let Some(s) = field::text_input(ui, hex_r, ("selhex", i), &hex_str) {
            if let Some(nc) = Color::from_hex(&s) {
                app.replace_selection_color(sel, *c, nc);
                app.props.sel_colors_cache = None;
            }
        }
        ui.painter().rect_filled(op_r, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
        let cnt = counts.get(i).copied().unwrap_or(1).max(1);
        let badge_txt = if cnt > 1 { format!("100% · {cnt}") } else { "100 %".to_string() };
        ui.painter().text(op_r.center(), Align2::CENTER_CENTER, badge_txt, theme::font_caption(), theme::TEXT_SECONDARY);
        pane.y += theme::FIELD_HEIGHT + 8.0;
    }
    pane.y += 4.0;
}

// ---------------------------------------------------------------- selection
fn selection_ui(app: &mut App, ui: &mut Ui, pane: &mut Pane, sel: &[NodeId]) {
    let Some(n0) = app.doc.get(sel[0]).map(|n| resolve_node(&app.doc, n).into_owned()) else { return };
    let multi = sel.len() > 1;
    let is_text = matches!(n0.kind, NodeKind::Text(_));
    let is_frame = n0.kind.frame_props().is_some();
    let is_line = matches!(n0.kind, NodeKind::Line);
    let parent_al = n0.parent.and_then(|p| app.doc.get(p)).and_then(|p| p.kind.frame_props()).map(|f| f.auto_layout.is_some()).unwrap_or(false);
    let parent_frame = n0.parent.and_then(|p| app.doc.get(p)).map(|p| p.kind.frame_props().is_some()).unwrap_or(false);

    // ---- header: name + quick toggles
    pane.y += theme::RIGHT_SELECTION_HEADER_Y - theme::RIGHT_CONTENT_TOP;
    let name_r = pane.rect(theme::RIGHT_NAME_FIELD_X - 1.0, 0.0, theme::RIGHT_NAME_FIELD_W, theme::FIELD_HEIGHT);
    if multi {
        ui.painter().text(
            Pos2::new(name_r.left() + 8.0, name_r.center().y),
            Align2::LEFT_CENTER,
            format!("{} layers", sel.len()),
            theme::font_large(),
            theme::TEXT,
        );
    } else {
        let id = ui.id().with("sel-name");
        if ui.rect_contains_pointer(name_r) || ui.memory(|m| m.has_focus(id)) {
            let col = if ui.memory(|m| m.has_focus(id)) { theme::BORDER_SELECTED } else { theme::BORDER };
            ui.painter().rect_stroke(name_r, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, col), StrokeKind::Inside);
        }
        let comp = matches!(n0.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_) | NodeKind::Instance { .. }) || n0.component.source.is_some();
        if let Some(s) = field::edit_text(ui, name_r, id, &n0.name, theme::font_large(), if comp { theme::TEXT_COMPONENT } else { theme::TEXT }, 8.0) {
            app.rename(sel[0], &s);
        }
    }
    let btns = [
        (Icon::Mask, "Use as mask", n0.is_mask),
        (if n0.locked { Icon::Lock } else { Icon::Unlock }, "Lock", n0.locked),
        (if n0.visible { Icon::Eye } else { Icon::EyeOff }, "Hide / show", !n0.visible),
    ];
    for (i, (icon, tip, on)) in btns.iter().enumerate() {
        let r = pane.rect(theme::RIGHT_NAME_ACTION_X[i] - 1.0, 0.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
        if widgets::icon_button(ui, r, ("hdr", i), *icon, *on, tip).clicked() {
            match i {
                0 => app.edit_nodes(sel, |n| n.is_mask = !on),
                1 => app.edit_nodes(sel, |n| n.locked = !on),
                _ => app.edit_nodes(sel, |n| n.visible = !n.visible),
            }
        }
    }
    pane.y += theme::FIELD_HEIGHT + 8.0;
    if is_frame && !multi {
        let slides_r = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, 26.0);
        widgets::record(ui, "copy-to-figma-slides-btn", slides_r);
        widgets::record(ui, "copy-to-slides", slides_r);
        let slides_resp = ui.interact(slides_r, ui.id().with("copy-to-figma-slides"), Sense::click());
        let slides_bg = if slides_resp.hovered() { Color32::from_rgb(0x38, 0x38, 0x38) } else { Color32::from_rgb(0x2C, 0x2C, 0x2C) };
        ui.painter().rect_filled(slides_r, theme::cr(6), slides_bg);
        ui.painter().rect_stroke(slides_r, theme::cr(6), Stroke::new(1.0, Color32::from_rgb(0x38, 0x38, 0x38)), StrokeKind::Inside);
        ui.painter().text(slides_r.center(), Align2::CENTER_CENTER, "Copy to Slide Deck", theme::font_caption(), theme::TEXT);
        if slides_resp.on_hover_text("Copy selected frame to Slide Deck").clicked() {
            if let Some(s) = app.copy_selection() {
                ui.ctx().copy_text(s);
            }
            app.show_toast("Copied selection to Slide Deck", false);
        }
        pane.y += 32.0;
    }
    section_sep(ui, pane, false);
    if !multi {
        super::components::sections(app, ui, pane, sel[0]);
    }
    // ---- Vertex: shown while the vector is in point editing mode
    if !multi && app.canvas.vedit.as_ref().is_some_and(|v| v.id == sel[0]) {
        super::vertex::section(app, ui, pane);
    }

    // ---- Position
    title(ui, pane, "Position", None, false);
    sublabel(ui, pane, "Alignment");
    let h = [Icon::AlignLeft, Icon::AlignHCenter, Icon::AlignRight];
    let v = [Icon::AlignTop, Icon::AlignVCenter, Icon::AlignBottom];
    let shift_align = ui.input(|i| i.modifiers.shift) && app.selection.len() >= 2;
    if let Some(i) = segmented(ui, pane, theme::SECTION_CONTENT_X, "alignh", &h, None) {
        let kind = [Align::Left, Align::HCenter, Align::Right][i];
        if shift_align {
            app.align_selection_as_group(kind);
        } else {
            app.align(kind);
        }
    }
    if let Some(i) = segmented(ui, pane, theme::SECTION_CONTENT_X + 3.0 * theme::SEGMENT_W + theme::SEGMENT_GROUP_GAP, "alignv", &v, None) {
        let kind = [Align::Top, Align::VCenter, Align::Bottom][i];
        if shift_align {
            app.align_selection_as_group(kind);
        } else {
            app.align(kind);
        }
    }
    let dist_r = pane.rect(theme::DISTRIBUTE_BTN_X, 0.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
    widgets::record(ui, "distribute", dist_r);
    if widgets::icon_button(ui, dist_r, "distribute", Icon::Distribute, false, "Distribute spacing").clicked() {
        app.distribute_selection();
    }
    row_end(pane);
    sublabel(ui, pane, "Position");
    let xs = common(app, sel, |n| n.transform.tx);
    let ys = common(app, sel, |n| n.transform.ty);
    if let Some(x) = field::num_field(ui, pane.col(0, 0.0), "x", Prefix::Text("X"), xs, "") {
        app.edit_nodes(sel, |n| n.transform.tx = x);
    }
    if let Some(y) = field::num_field(ui, pane.col(1, 0.0), "y", Prefix::Text("Y"), ys, "") {
        app.edit_nodes(sel, |n| n.transform.ty = y);
    }
    row_end(pane);
    sublabel(ui, pane, "Rotation");
    let rot = common(app, sel, |n| (-n.transform.rotation_rad().to_degrees() * 100.0).round() / 100.0);
    if let Some(deg) = field::num_field(ui, pane.col(0, 0.0), "rot", Prefix::Icon(Icon::Rotation), rot, "°") {
        if multi {
            // A group rotates as one body around its bounding-box centre.
            app.rotate_selection(-(deg - rot.unwrap_or(0.0)).to_radians());
        } else {
            let rad = -deg.to_radians();
            app.edit_nodes(sel, |n| n.transform = crate::canvas::handles::rotate_about_center(&n.transform, n.size, rad));
        }
    }
    for i in 0..3 {
        widgets::record(ui, &format!("xform:{i}"), pane.rect(theme::FIELD_COL_X[1] + i as f32 * theme::SEGMENT_W, 0.0, theme::SEGMENT_W, theme::FIELD_HEIGHT));
    }
    if let Some(i) = segmented(ui, pane, theme::FIELD_COL_X[1], "xform", &[Icon::Rotate90, Icon::FlipH, Icon::FlipV], None) {
        match i {
            0 => app.rotate_selection(-std::f32::consts::FRAC_PI_2),
            1 => app.flip_selection(true),
            _ => app.flip_selection(false),
        }
    }
    row_end(pane);
    pane.y += 4.0;

    let al_frame = is_frame && !multi && n0.kind.frame_props().map(|f| f.auto_layout.is_some()).unwrap_or(false);

    // ---- Auto layout (carries the dimensions of auto-layout frames)
    if is_frame && !multi {
        section_sep(ui, pane, false);
        let al = n0.kind.frame_props().and_then(|f| f.auto_layout.as_ref());
        if title(ui, pane, "Auto layout", Some(if al.is_some() { Icon::Minus } else { Icon::Plus }), al.is_none()) {
            app.toggle_auto_layout();
        }
        if let Some(a) = al {
            auto_layout_ui(app, ui, pane, sel, a, &n0, parent_al);
        }
    }

    // ---- Layout: dimensions, resizing and constraints of everything else
    if !al_frame {
        section_sep(ui, pane, false);
        title(ui, pane, "Layout", None, false);
        dimensions_ui(app, ui, pane, sel, is_line);
        if parent_al || is_text {
            resizing_ui(app, ui, pane, sel, &n0);
        }
        if parent_frame && !parent_al && !multi {
            sublabel(ui, pane, "Constraints");
            let hn = ["Left", "Right", "Left & Right", "Center", "Scale"].map(String::from);
            let vn = ["Top", "Bottom", "Top & Bottom", "Center", "Scale"].map(String::from);
            let order = [Constraint::Min, Constraint::Max, Constraint::Stretch, Constraint::Center, Constraint::Scale];
            let hi = order.iter().position(|c| *c == n0.constraints.horizontal).unwrap_or(0);
            let vi = order.iter().position(|c| *c == n0.constraints.vertical).unwrap_or(0);
            if let Some(i) = dropdown(ui, pane.col(0, 0.0), "con-h", &hn[hi], &hn) {
                app.edit_nodes(sel, |n| n.constraints.horizontal = order[i]);
            }
            if let Some(i) = dropdown(ui, pane.col(1, 0.0), "con-v", &vn[vi], &vn) {
                app.edit_nodes(sel, |n| n.constraints.vertical = order[i]);
            }
            row_end(pane);
        }
        pane.y += 4.0;
    }

    // ---- Typography
    if let NodeKind::Text(t) = &n0.kind {
        section_sep(ui, pane, false);
        title(ui, pane, "Typography", None, false);
        let var_pill = pane.rect(theme::SECTION_TRAILING_X, -26.0, 24.0, 20.0);
        widgets::record(ui, "typo-var-pill", var_pill);
        if ui.interact(var_pill, ui.id().with("typo-var-pill"), Sense::click()).on_hover_text("Apply typography variable or style").clicked() {
            app.open_variables();
        }
        super::typography::section(app, ui, pane, sel, t);
        pane.y += 4.0;
    }

    // ---- Appearance
    section_sep(ui, pane, false);
    title(ui, pane, "Appearance", None, false);
    let op = common(app, sel, |n| (n.opacity * 100.0).round());
    let op_r = pane.col(0, 0.0);
    if let Some(v) = var_bind::numeric(app, ui, op_r, sel, &["opacity"], |ui| field::num_field(ui, op_r, "opacity", Prefix::Icon(Icon::Opacity), op, "%")) {
        let o = (v / 100.0).clamp(0.0, 1.0);
        app.edit_nodes(sel, |n| n.opacity = o);
    }
    let has_radius = !matches!(n0.kind, NodeKind::Text(_) | NodeKind::Line | NodeKind::Group | NodeKind::Vector { .. });
    if has_radius {
        let rad = common(app, sel, |n| n.corner_radii[0]);
        let rad_r = pane.col(1, 0.0);
        let radius_keys = ["topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"];
        if let Some(v) = var_bind::numeric(app, ui, rad_r, sel, &radius_keys, |ui| field::num_field(ui, rad_r, "radius", Prefix::Icon(Icon::Radius), rad, "")) {
            let v = v.max(0.0);
            app.edit_nodes(sel, |n| n.corner_radii = [v; 4]);
        }
    }
    row_end(pane);
    if is_frame {
        let clip = n0.kind.frame_props().map(|f| f.clips_content).unwrap_or(false);
        if let Some(v) = checkbox(ui, pane, "clip", "Clip content", clip) {
            app.edit_nodes(sel, |n| {
                if let Some(f) = n.kind.frame_props_mut() {
                    f.clips_content = v;
                }
            });
        }
    }
    if is_frame {
        var_bind::mode_rows(app, ui, pane, sel);
    }
    pane.y += 4.0;

    // ---- Fill
    if matches!(n0.kind, NodeKind::Text(_)) {
        // Text colour lives in the text style, not in `Node::fills`.
        section_sep(ui, pane, false);
        title(ui, pane, "Fill", None, false);
        super::typography::fill_section(app, ui, pane, sel);
    } else if !is_line {
        section_sep(ui, pane, false);
        if paint_title(app, ui, pane, sel, "Fill", false, n0.fills.is_empty()) {
            let paint = Paint::solid(Color::from_hex(&format!("{:06X}", theme::DEFAULT_SHAPE_FILL)).unwrap_or(Color::WHITE));
            app.edit_nodes(sel, |n| n.fills.push(paint.clone()));
        }
        paint_rows(app, ui, pane, sel, &n0.fills, false);
        if !n0.fills.is_empty() {
            // 16px under the last row (8 of it came with the row).
            pane.y += 8.0;
        }
    }

    // ---- Stroke
    section_sep(ui, pane, false);
    super::stroke::section(app, ui, pane, sel, &n0);

    // ---- Effects
    section_sep(ui, pane, false);
    super::effects::section(app, ui, pane, sel, &n0.effects);

    // ---- Layout guide (frames)
    if is_frame && !multi {
        if let Some(fp) = n0.kind.frame_props() {
            super::layoutguide::section(app, ui, pane, sel, &fp.grids);
        }
    }

    // ---- Selection colors / Styles
    selection_colors_section(app, ui, pane, sel);
    if !app.doc.styles.is_empty() {
        section_sep(ui, pane, false);
        title(ui, pane, "Styles", None, false);
        styles_list(app, ui, pane, sel);
    }

    // ---- Export
    section_sep(ui, pane, false);
    pane.y += super::export::section(app, ui, pane.x0, pane.oy + pane.y, sel);
}

#[cfg(test)]
fn sizing_of(n: &Node, horizontal: bool) -> Sizing {
    sizing_of_with_parent(n, horizontal, Some(false))
}

fn sizing_of_with_parent(n: &Node, horizontal: bool, counter_horizontal: Option<bool>) -> Sizing {
    if let Some(al) = n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()) {
        if al.sizing_along(horizontal) == Sizing::Hug {
            return Sizing::Hug;
        }
    }
    if let NodeKind::Text(t) = &n.kind {
        if t.auto_resize == TextAutoResize::WidthAndHeight || (t.auto_resize == TextAutoResize::Height && !horizontal) {
            return Sizing::Hug;
        }
    }
    n.layout_child.effective_sizing(horizontal, counter_horizontal)
}

fn set_sizing(app: &mut App, sel: &[NodeId], horizontal: bool, s: Sizing) {
    // the parent flow's counter axis per node: a counter-axis write other than Fill ends an AlignSelf::Stretch
    let counter_axes: Vec<(NodeId, Option<bool>)> = sel
        .iter()
        .map(|id| (*id, app.doc.get(*id).and_then(|n| n.parent).and_then(|p| app.doc.get(p)).and_then(LayoutChild::counter_axis_is_horizontal)))
        .collect();
    app.edit_nodes(sel, |n| {
        let counter = counter_axes.iter().find(|(id, _)| *id == n.id).and_then(|(_, axis)| *axis);
        n.layout_child.set_axis_sizing(horizontal, if s == Sizing::Hug { Sizing::Fixed } else { s }, counter);
        if let Some(al) = n.kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()) {
            let primary_h = al.primary_is_horizontal();
            let target = if horizontal == primary_h { &mut al.primary_sizing } else { &mut al.counter_sizing };
            *target = if s == Sizing::Hug { Sizing::Hug } else { Sizing::Fixed };
        }
        if let NodeKind::Text(t) = &mut n.kind {
            if s == Sizing::Hug {
                t.auto_resize = if horizontal { TextAutoResize::WidthAndHeight } else { TextAutoResize::Height };
            } else if t.auto_resize != TextAutoResize::None {
                t.auto_resize = TextAutoResize::None;
            }
        }
    });
}

fn resize_selection(app: &mut App, sel: &[NodeId], w: Option<f32>, h: Option<f32>) {
    let mut tx = Tx::new();
    for id in sel {
        let Some(n) = app.doc.get(*id) else { continue };
        let size = v2(w.unwrap_or(n.size.x).max(1.0), h.unwrap_or(n.size.y).max(if matches!(n.kind, NodeKind::Line) { 0.0 } else { 1.0 }));
        let t = n.transform;
        apply_resize(&mut app.doc, &mut tx.ed, *id, size, t, true);
    }
    app.run_tx(tx);
}

fn dimensions_ui(app: &mut App, ui: &mut Ui, pane: &mut Pane, sel: &[NodeId], is_line: bool) {
    sublabel(ui, pane, "Dimensions");
    let ws = common(app, sel, |n| n.size.x);
    let hs = common(app, sel, |n| n.size.y);
    let (wr, hr) = (pane.col(0, 0.0), pane.col(1, 0.0));
    let w_new = var_bind::numeric(app, ui, wr, sel, &["width"], |ui| field::num_field(ui, wr, "w", Prefix::Text("W"), ws, ""));
    let h_new = if is_line { None } else { var_bind::numeric(app, ui, hr, sel, &["height"], |ui| field::num_field(ui, hr, "h", Prefix::Text("H"), hs, "")) };
    if w_new.is_some() || h_new.is_some() {
        resize_selection(app, sel, w_new, h_new);
    }
    row_end(pane);
}

fn resizing_ui(app: &mut App, ui: &mut Ui, pane: &mut Pane, sel: &[NodeId], n0: &Node) {
    sublabel(ui, pane, "Resizing");
    let opts = ["Fixed", "Hug", "Fill"].map(String::from);
    let order = [Sizing::Fixed, Sizing::Hug, Sizing::Fill];
    let counter_horiz = n0.parent.and_then(|p| app.doc.get(p)).and_then(LayoutChild::counter_axis_is_horizontal);
    let (hs, vs) = (sizing_of_with_parent(n0, true, counter_horiz), sizing_of_with_parent(n0, false, counter_horiz));
    if let Some(i) = dropdown(ui, pane.col(0, 0.0), "rs-h", &format!("W: {}", opts[order.iter().position(|s| *s == hs).unwrap_or(0)]), &opts) {
        set_sizing(app, sel, true, order[i]);
    }
    if let Some(i) = dropdown(ui, pane.col(1, 0.0), "rs-v", &format!("H: {}", opts[order.iter().position(|s| *s == vs).unwrap_or(0)]), &opts) {
        set_sizing(app, sel, false, order[i]);
    }
    row_end(pane);
    let parent_al = n0.parent.and_then(|p| app.doc.get(p)).and_then(|p| p.kind.frame_props()).and_then(|f| f.auto_layout.as_ref());
    let (parent_in_auto_layout, parent_grid) = (parent_al.is_some(), parent_al.is_some_and(|al| al.direction == LayoutDirection::Grid));
    if parent_grid {
        sublabel(ui, pane, "Grid Span");
        let cs = n0.layout_child.col_span.max(1) as f32;
        let rs = n0.layout_child.row_span.max(1) as f32;
        if let Some(v) = field::num_field(ui, pane.col(0, 0.0), "grid-col-span", Prefix::Text("CS"), Some(cs), "") {
            let span = v.round().clamp(1.0, designos_doc::MAX_GRID_TRACKS as f32) as u16;
            app.edit_nodes(sel, |n| n.layout_child.col_span = span);
        }
        if let Some(v) = field::num_field(ui, pane.col(1, 0.0), "grid-row-span", Prefix::Text("RS"), Some(rs), "") {
            let span = v.round().clamp(1.0, designos_doc::MAX_GRID_TRACKS as f32) as u16;
            app.edit_nodes(sel, |n| n.layout_child.row_span = span);
        }
        row_end(pane);
    }
    if parent_in_auto_layout && has_child_margins(n0) {
        sublabel(ui, pane, "Margin");
        let [mt, mr, mb, ml] = n0.layout_child.margin;
        if let Some(v) = field::num_field(ui, pane.col(0, 0.0), "margin-l", Prefix::Text("L"), Some(ml), "") {
            app.edit_nodes(sel, |n| n.layout_child.margin[3] = v);
        }
        if let Some(v) = field::num_field(ui, pane.col(1, 0.0), "margin-r", Prefix::Text("R"), Some(mr), "") {
            app.edit_nodes(sel, |n| n.layout_child.margin[1] = v);
        }
        row_end(pane);
        if let Some(v) = field::num_field(ui, pane.col(0, 0.0), "margin-t", Prefix::Text("T"), Some(mt), "") {
            app.edit_nodes(sel, |n| n.layout_child.margin[0] = v);
        }
        if let Some(v) = field::num_field(ui, pane.col(1, 0.0), "margin-b", Prefix::Text("B"), Some(mb), "") {
            app.edit_nodes(sel, |n| n.layout_child.margin[2] = v);
        }
        row_end(pane);
    }
}

pub fn has_child_margins(n: &Node) -> bool {
    n.layout_child.margin != [0.0; 4]
}

fn auto_layout_ui(app: &mut App, ui: &mut Ui, pane: &mut Pane, sel: &[NodeId], a: &AutoLayout, n0: &Node, parent_al: bool) {
    sublabel(ui, pane, "Flow");
    // Horizontal and Vertical are the direction, Wrap is a toggle on top of either, Grid replaces both.
    let grid = a.direction == LayoutDirection::Grid;
    let flow_marks: Vec<usize> = match (a.direction, a.wrap) {
        (LayoutDirection::Grid, _) => vec![3],
        (LayoutDirection::Vertical, true) => vec![1, 2],
        (LayoutDirection::Vertical, false) => vec![1],
        (LayoutDirection::Horizontal, true) => vec![0, 2],
        (LayoutDirection::Horizontal, false) => vec![0],
    };
    if let Some(i) =
        segmented_marks(ui, pane, theme::SECTION_CONTENT_X, "al-dir", &[Icon::DirHorizontal, Icon::DirVertical, Icon::Wrap, Icon::Grid2x2], &flow_marks)
    {
        update_al(app, sel, |al| match i {
            0 | 1 => {
                al.direction = if i == 0 { LayoutDirection::Horizontal } else { LayoutDirection::Vertical };
                // wrap survives a direction change; a grid has no wrap
                al.wrap = al.wrap && !grid;
            }
            2 => {
                if al.direction == LayoutDirection::Grid {
                    al.direction = LayoutDirection::Horizontal;
                }
                al.wrap = !al.wrap;
                if al.wrap {
                    al.primary_sizing = Sizing::Fixed;
                }
            }
            _ => {
                al.direction = LayoutDirection::Grid;
                al.wrap = false;
                if al.grid_columns == 0 {
                    al.grid_columns = 3;
                }
            }
        });
    }
    let gap_r = pane.col(1, 0.0);
    if let Some(v) = var_bind::numeric(app, ui, gap_r, sel, &["itemSpacing"], |ui| {
        field::num_field(ui, gap_r, "al-gap", Prefix::Icon(Icon::Spacing), Some(a.item_spacing), "")
    }) {
        update_al(app, sel, |al| al.item_spacing = v);
    }
    row_end(pane);
    if a.wrap && a.direction != LayoutDirection::Grid {
        wrap_lines_ui(app, ui, pane, sel, a);
    }
    if a.direction == LayoutDirection::Grid {
        sublabel(ui, pane, "Grid (Columns × Rows)");
        let cols = a.grid_columns.max(1) as f32;
        let rows = a.grid_rows as f32;
        if let Some(v) = field::num_field(ui, pane.col(0, 0.0), "grid-cols", Prefix::Text("C"), Some(cols), "") {
            let c = v.round().clamp(1.0, designos_doc::MAX_GRID_TRACKS as f32) as u32;
            update_al(app, sel, |al| al.grid_columns = c);
        }
        if let Some(v) = field::num_field(ui, pane.col(1, 0.0), "grid-rows", Prefix::Text("R"), Some(rows), "") {
            let r = v.round().clamp(0.0, designos_doc::MAX_GRID_TRACKS as f32) as u32;
            update_al(app, sel, |al| al.grid_rows = r);
        }
        row_end(pane);
        if let &[grid_id] = sel {
            grid_tracks_ui(app, ui, pane, grid_id);
        }
    }
    dimensions_ui(app, ui, pane, sel, false);
    let _ = parent_al;
    resizing_ui(app, ui, pane, sel, n0);
    sublabel(ui, pane, "Padding");
    let ph = a.padding[1].max(a.padding[3]);
    let pv = a.padding[0].max(a.padding[2]);
    let ph_r = pane.col(0, 0.0);
    if let Some(v) = var_bind::numeric(app, ui, ph_r, sel, &["paddingLeft", "paddingRight"], |ui| {
        field::num_field(ui, ph_r, "al-ph", Prefix::Icon(Icon::PaddingH), Some(ph), "")
    }) {
        update_al(app, sel, |al| {
            al.padding[1] = v;
            al.padding[3] = v;
        });
    }
    let pv_r = pane.col(1, 0.0);
    if let Some(v) = var_bind::numeric(app, ui, pv_r, sel, &["paddingTop", "paddingBottom"], |ui| {
        field::num_field(ui, pv_r, "al-pv", Prefix::Icon(Icon::PaddingV), Some(pv), "")
    }) {
        update_al(app, sel, |al| {
            al.padding[0] = v;
            al.padding[2] = v;
        });
    }
    row_end(pane);
    sublabel(ui, pane, "Alignment");
    let auto_spacing = matches!(a.primary_align, LayoutAlign::SpaceBetween | LayoutAlign::SpaceAround | LayoutAlign::SpaceEvenly);
    let pi = match a.primary_align {
        LayoutAlign::Center => 1,
        LayoutAlign::End => 2,
        _ => 0,
    };
    let ci = match a.counter_align {
        LayoutAlign::Center => 1,
        LayoutAlign::End => 2,
        _ => 0,
    };
    let align_row_r = pane.full(0.0);
    let al_icons = [Icon::AlignLeft, Icon::AlignHCenter, Icon::AlignRight];
    // with auto spacing the items are spread, so no packed alignment is lit
    if let Some(i) = segmented(ui, pane, theme::SECTION_CONTENT_X, "al-main", &al_icons, (!auto_spacing).then_some(pi)) {
        update_al(app, sel, |al| al.primary_align = [LayoutAlign::Start, LayoutAlign::Center, LayoutAlign::End][i]);
    }
    let sb = pane.rect(theme::SECTION_CONTENT_X + 3.0 * theme::SEGMENT_W + 4.0, 0.0, 24.0, theme::FIELD_HEIGHT);
    let sbr = widgets::icon_button(ui, sb, "al-space", Icon::Spacing, auto_spacing, "Auto spacing (double-click or X)");
    let hover_align = ui.rect_contains_pointer(align_row_r);
    let dbl_or_x = (hover_align && ui.input(|i| i.pointer.button_double_clicked(egui::PointerButton::Primary)))
        || (hover_align && !ui.ctx().egui_wants_keyboard_input() && ui.input(|i| i.key_pressed(egui::Key::X)));
    if dbl_or_x {
        let next = if auto_spacing { LayoutAlign::Start } else { LayoutAlign::SpaceBetween };
        update_al(app, sel, |al| al.primary_align = next);
        app.show_toast(if next == LayoutAlign::SpaceBetween { "Align: Auto spacing (Between)" } else { "Align: Packed" }, false);
    }
    auto_spacing_menu(app, ui, sel, a, sb, sbr.clicked() && !dbl_or_x);
    let v_icons = [Icon::AlignTop, Icon::AlignVCenter, Icon::AlignBottom];
    if let Some(i) = segmented(ui, pane, theme::FIELD_COL_X[1] + 16.0, "al-cross", &v_icons, Some(ci)) {
        update_al(app, sel, |al| al.counter_align = [LayoutAlign::Start, LayoutAlign::Center, LayoutAlign::End][i]);
    }
    row_end(pane);
    if a.layout_version < 1 {
        pane.y += 4.0;
        let up_r = pane.rect(theme::SECTION_CONTENT_X, 0.0, theme::SECTION_CONTENT_W, 24.0);
        widgets::record(ui, "al-upgrade-v1", up_r);
        let resp = ui.interact(up_r, ui.id().with("al-upgrade-v1"), Sense::click());
        let bg = if resp.hovered() { Color32::from_rgb(0x38, 0x38, 0x38) } else { Color32::from_rgb(0x2C, 0x2C, 0x2C) };
        ui.painter().rect_filled(up_r, theme::cr(theme::RADIUS_CONTROL), bg);
        ui.painter().rect_stroke(up_r, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, Color32::from_rgb(0x38, 0x38, 0x38)), StrokeKind::Inside);
        ui.painter().text(up_r.center(), Align2::CENTER_CENTER, "Update auto layout", theme::font_caption(), theme::TEXT);
        if resp.on_hover_text("Upgrade to Auto layout v1 (borders inside the box)").clicked() {
            app.upgrade_auto_layout_version();
        }
        pane.y += 26.0;
    }
    pane.y += 4.0;
}

/// Gap between the wrapped lines and how the lines spread across the counter axis.
fn wrap_lines_ui(app: &mut App, ui: &mut Ui, pane: &mut Pane, sel: &[NodeId], a: &AutoLayout) {
    sublabel(ui, pane, "Wrap");
    // horizontal wrap stacks rows (a vertical gap), vertical wrap stacks columns (a horizontal gap)
    let icon = if a.direction == LayoutDirection::Horizontal { Icon::PaddingV } else { Icon::PaddingH };
    if let Some(v) = field::num_field(ui, pane.col(0, 0.0), "al-counter-gap", Prefix::Icon(icon), Some(a.counter_spacing), "") {
        update_al(app, sel, |al| al.counter_spacing = v);
    }
    let between = a.counter_align_content == CounterAlignContent::SpaceBetween;
    let options = ["Auto", "Between"].map(String::from);
    if let Some(i) = dropdown(ui, pane.col(1, 0.0), "al-counter-content", &options[between as usize], &options) {
        update_al(app, sel, |al| al.counter_align_content = if i == 1 { CounterAlignContent::SpaceBetween } else { CounterAlignContent::Auto });
    }
    row_end(pane);
}

/// Rects of the widgets of one grid-track row of the inspector, left to right. The heading row of a track
/// list uses `label` and `add`; a track row uses `mode`, `value`, `move_btn` and `delete`. Every button is
/// the panel's 24 px action size, rows are 24 px tall, and nothing overlaps or leaves the section content.
struct GridTrackRow {
    label: Rect,
    add: Rect,
    mode: Rect,
    value: Rect,
    move_btn: Rect,
    delete: Rect,
}

fn grid_track_row(pane: &Pane) -> GridTrackRow {
    const GAP: f32 = 4.0;
    const MODE_W: f32 = 64.0;
    let (b, h) = (theme::PANEL_ACTION_BUTTON, theme::FIELD_HEIGHT);
    let left = theme::SECTION_CONTENT_X;
    let delete_x = left + theme::SECTION_CONTENT_W - b;
    let move_x = delete_x - GAP - b;
    let value_x = left + MODE_W + GAP;
    GridTrackRow {
        label: pane.rect(left, 0.0, delete_x - GAP - left, h),
        add: pane.rect(delete_x, 0.0, b, b),
        mode: pane.rect(left, 0.0, MODE_W, h),
        value: pane.rect(value_x, 0.0, move_x - GAP - value_x, h),
        move_btn: pane.rect(move_x, 0.0, b, b),
        delete: pane.rect(delete_x, 0.0, b, b),
    }
}

/// The move button of track `idx` in a list of `len` tracks (`len > 1`): its icon, tooltip and the track it swaps with.
fn track_move_button(idx: usize, len: usize) -> (Icon, &'static str, usize) {
    if idx + 1 < len {
        (Icon::ChevronDown, "Move down", idx + 1)
    } else {
        (Icon::ChevronUp, "Move up", idx - 1)
    }
}

/// Index of a track's sizing mode in the Flex / Fixed / Hug dropdown.
fn track_mode_index(track: designos_doc::GridTrack) -> usize {
    use designos_doc::GridTrack;
    match track {
        GridTrack::Flex(_) => 0,
        GridTrack::Fixed(_) => 1,
        GridTrack::Hug => 2,
    }
}

/// The track a mode-dropdown pick turns `track` into, or `None` when the pick is the mode it already has.
/// Re-picking the current mode must keep a custom size such as `Flex(2.0)` or `Fixed(80.0)`.
fn track_after_mode_pick(track: designos_doc::GridTrack, picked: usize) -> Option<designos_doc::GridTrack> {
    use designos_doc::GridTrack;
    if picked == track_mode_index(track) {
        return None;
    }
    Some(match picked {
        0 => GridTrack::Flex(1.0),
        1 => GridTrack::Fixed(120.0),
        _ => GridTrack::Hug,
    })
}

/// Column and row track lists of the selected grid: sizing mode, value, move and delete per track, `+` per list.
/// The rows listed are the ones the canvas draws a header pill for.
fn grid_tracks_ui(app: &mut App, ui: &mut Ui, pane: &mut Pane, grid_id: NodeId) {
    use crate::canvas::grids::{
        delete_grid_track, grid_track_delete_changes_document, insert_grid_track, move_grid_track, node_css_grid_overlay, resize_grid_track,
    };
    use designos_doc::GridTrack;

    let Some(ov) = node_css_grid_overlay(&app.doc, grid_id) else { return };
    let options = ["Flex", "Fixed", "Hug"].map(String::from);
    for (is_col, heading, tag, tracks) in [(true, "Columns", "col", &ov.col_tracks), (false, "Rows", "row", &ov.row_tracks)] {
        let head = grid_track_row(pane);
        ui.painter().text(
            Pos2::new(head.label.left(), head.label.center().y),
            Align2::LEFT_CENTER,
            heading,
            theme::font_caption_medium(),
            theme::TEXT_SECONDARY,
        );
        if tracks.len() < MAX_GRID_TRACKS
            && widgets::icon_button(ui, head.add, format!("grid-{tag}-add"), Icon::Plus, false, &format!("Add {tag} track")).clicked()
        {
            insert_grid_track(app, grid_id, is_col, tracks.len(), GridTrack::Flex(1.0));
        }
        row_end(pane);
        for (idx, &track) in tracks.iter().enumerate() {
            let row = grid_track_row(pane);
            let mode = track_mode_index(track);
            if let Some(picked) = dropdown(ui, row.mode, format!("grid-{tag}-mode-{idx}"), &options[mode], &options) {
                if let Some(next) = track_after_mode_pick(track, picked) {
                    resize_grid_track(app, grid_id, is_col, idx, next);
                }
            }
            match track {
                GridTrack::Flex(fr) => {
                    if let Some(v) = field::num_field(ui, row.value, format!("grid-{tag}-fr-{idx}"), Prefix::Text("fr"), Some(fr), "") {
                        resize_grid_track(app, grid_id, is_col, idx, GridTrack::Flex(v.max(0.1)));
                    }
                }
                GridTrack::Fixed(px) => {
                    if let Some(v) = field::num_field(ui, row.value, format!("grid-{tag}-px-{idx}"), Prefix::Text("px"), Some(px), "") {
                        resize_grid_track(app, grid_id, is_col, idx, GridTrack::Fixed(v.max(1.0)));
                    }
                }
                GridTrack::Hug => {}
            }
            if tracks.len() > 1 {
                let (icon, tip, next_idx) = track_move_button(idx, tracks.len());
                if widgets::icon_button(ui, row.move_btn, format!("grid-{tag}-mv-{idx}"), icon, false, tip).clicked() {
                    move_grid_track(app, grid_id, is_col, idx, next_idx);
                }
            }
            if grid_track_delete_changes_document(&app.doc, grid_id, is_col, idx, tracks.len()) {
                widgets::record(ui, &format!("grid-{tag}-del-{idx}"), row.delete);
                if widgets::icon_button(ui, row.delete, format!("grid-{tag}-del-{idx}"), Icon::Minus, false, "Delete track").clicked() {
                    delete_grid_track(app, grid_id, is_col, idx);
                }
            }
            row_end(pane);
        }
    }
}

/// The Auto spacing popup: Between, Around or Evenly, or back to packed items.
fn auto_spacing_menu(app: &mut App, ui: &mut Ui, sel: &[NodeId], a: &AutoLayout, anchor: Rect, toggle: bool) {
    let id = ui.id().with("al-auto-spacing-menu");
    let open = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false) != toggle;
    ui.data_mut(|d| d.insert_temp(id, open));
    if !open {
        return;
    }
    let modes = [LayoutAlign::SpaceBetween, LayoutAlign::SpaceAround, LayoutAlign::SpaceEvenly];
    let mut items = vec![MenuItem::new("Auto spacing").disabled(true).sep()];
    items.extend(["Between", "Around", "Evenly"].iter().zip(modes).map(|(label, m)| MenuItem::new(*label).checked(a.primary_align == m)));
    items.push(MenuItem::new("Packed").checked(!modes.contains(&a.primary_align)));
    match menu::popup_menu(ui.ctx(), id.with("popup"), anchor.left_bottom() + Vec2::new(0.0, 2.0), 120.0, &items, Some(anchor)) {
        menu::MenuResult::Picked(i) => {
            ui.data_mut(|d| d.insert_temp(id, false));
            if i == 0 {
                return;
            }
            let mode = modes.get(i - 1).copied();
            // Between / Around / Evenly follow CSS justify-content, which is layout version 1
            update_al(app, sel, move |al| {
                al.primary_align = mode.unwrap_or(LayoutAlign::Start);
                if mode.is_some() {
                    al.layout_version = al.layout_version.max(1);
                }
            });
        }
        menu::MenuResult::Closed => {
            ui.data_mut(|d| d.insert_temp(id, false));
        }
        menu::MenuResult::Open => {}
    }
}

fn update_al(app: &mut App, sel: &[NodeId], f: impl Fn(&mut AutoLayout)) {
    app.edit_nodes(sel, |n| {
        if let Some(al) = n.kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()) {
            f(al);
        }
    });
}

// ---------------------------------------------------------------- paints
fn paint_label(p: &Paint) -> String {
    match &p.kind {
        PaintKind::Solid(c) => c.to_hex(),
        PaintKind::LinearGradient { .. } => "Linear".into(),
        PaintKind::RadialGradient { .. } => "Radial".into(),
        PaintKind::AngularGradient { .. } => "Angular".into(),
        PaintKind::DiamondGradient { .. } => "Diamond".into(),
        PaintKind::Image { .. } => "Image".into(),
    }
}

fn paint_preview(p: &Paint) -> Color32 {
    match &p.kind {
        PaintKind::Solid(c) => to_c32(*c, p.opacity),
        PaintKind::LinearGradient { stops, .. }
        | PaintKind::RadialGradient { stops, .. }
        | PaintKind::AngularGradient { stops, .. }
        | PaintKind::DiamondGradient { stops, .. } => stops.first().map(|s| to_c32(s.color, p.opacity)).unwrap_or(Color32::GRAY),
        PaintKind::Image { .. } => Color32::from_gray(150),
    }
}

fn edit_paint_list(app: &mut App, sel: &[NodeId], stroke: bool, f: impl Fn(&mut Vec<Paint>)) {
    app.edit_nodes(sel, |n| {
        // Hand-editing a paint detaches it from its shared style.
        n.style_refs.remove(if stroke { "stroke" } else { "fill" });
        if stroke {
            f(&mut n.stroke.paints)
        } else {
            f(&mut n.fills)
        }
    });
}

/// Width of the swatch + hex + opacity field group of a paint row (the eye and minus follow it).
const PAINT_GROUP_W: f32 = 159.0;
/// x of the divider between the hex and opacity cells, from the group's left edge.
const PAINT_DIVIDER_X: f32 = 104.0;

pub(super) fn paint_rows(app: &mut App, ui: &mut Ui, pane: &mut Pane, sel: &[NodeId], paints: &[Paint], stroke: bool) {
    let linked: Option<String> = sel
        .first()
        .and_then(|s| app.doc.get(*s))
        .and_then(|n| n.style_refs.get(if stroke { "stroke" } else { "fill" }))
        .map(|k| app.doc.style_name(k).to_string());
    if !paints.is_empty() {
        // Figma sits the first row 4px under the title row.
        pane.y += 4.0;
    }
    let mut reorder_pair: Option<(usize, usize)> = None;
    for (i, p) in paints.iter().enumerate() {
        let key = (stroke, i);
        let prop = if stroke { SelectedProperty::Stroke(i) } else { SelectedProperty::Fill(i) };
        let row_r = pane.rect(2.0, -2.0, theme::RIGHT_PANEL_WIDTH - 5.0, theme::FIELD_HEIGHT + 4.0);
        let gutter_r = pane.rect(0.0, 0.0, theme::SWATCH_ROW_X, theme::FIELD_HEIGHT);
        let gutter_resp = ui.interact(gutter_r, ui.id().with(("prop-gutter", key)), Sense::click_and_drag());
        if gutter_resp.clicked() {
            app.props.selected_property = Some(prop);
        }
        if gutter_resp.drag_started() {
            app.props.selected_property = Some(prop);
            app.props.drag_property = Some(prop);
        }
        let g = pane.rect(theme::SWATCH_ROW_X, 0.0, PAINT_GROUP_W, theme::FIELD_HEIGHT);
        let sw = Rect::from_min_size(Pos2::new(g.left() + 4.0, g.center().y - theme::SWATCH_SIZE / 2.0), Vec2::splat(theme::SWATCH_SIZE));
        let hex_r = Rect::from_min_max(
            Pos2::new(g.left() + theme::FIELD_PREFIX_X + theme::FIELD_PREFIX_ICON, g.top()),
            Pos2::new(g.left() + PAINT_DIVIDER_X, g.bottom()),
        );
        let op_r = Rect::from_min_max(Pos2::new(g.left() + PAINT_DIVIDER_X + 1.0, g.top()), Pos2::new(g.right() - 16.0, g.bottom()));
        let eye_r = pane.rect(theme::RIGHT_NAME_ACTION_X[1] - 1.0 + 4.0, 0.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
        let minus_r = pane.rect(theme::SECTION_TRAILING_X, 0.0, theme::PANEL_ACTION_BUTTON, theme::PANEL_ACTION_BUTTON);
        widgets::record(ui, &format!("sw:{stroke}:{i}"), sw);
        // Group chrome: one rounded field with a hairline between hex and opacity.
        let hex_id = ui.id().with(("txt", ("hex", key)));
        let op_id = ui.id().with(("num", ("op", key)));
        let focused = ui.memory(|m| m.has_focus(hex_id) || m.has_focus(op_id));
        if focused {
            app.props.selected_property = Some(prop);
        }
        {
            let pt = ui.painter();
            if app.props.selected_property == Some(prop) {
                pt.rect_stroke(row_r, theme::cr(4), Stroke::new(1.0, theme::BORDER_SELECTED), StrokeKind::Inside);
            }
            pt.rect_filled(g, theme::cr(theme::RADIUS_CONTROL), theme::BG_SECONDARY);
            if focused {
                pt.rect_stroke(g, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, theme::BORDER_SELECTED), StrokeKind::Inside);
            } else if ui.rect_contains_pointer(g) {
                pt.rect_stroke(g, theme::cr(theme::RADIUS_CONTROL), Stroke::new(1.0, theme::BORDER), StrokeKind::Inside);
            }
        }
        if let Some(drag_prop) = app.props.drag_property {
            let drag_src = match (stroke, drag_prop) {
                (false, SelectedProperty::Fill(s)) => Some(s),
                (true, SelectedProperty::Stroke(s)) => Some(s),
                _ => None,
            };
            if let Some(src_i) = drag_src {
                let (ptr, released) = ui.ctx().input(|inp| (inp.pointer.interact_pos(), inp.pointer.any_released()));
                if let Some(pos) = ptr.filter(|pos| row_r.contains(*pos)) {
                    let line_y = if pos.y < row_r.center().y { row_r.top() } else { row_r.bottom() };
                    ui.painter().hline(row_r.left()..=row_r.right(), line_y, Stroke::new(2.0, theme::BORDER_SELECTED));
                    if released && src_i != i {
                        reorder_pair = Some((src_i, i));
                    }
                }
            }
        }
        let swr = ui.interact(sw.expand(2.0), ui.id().with(("sw", key)), Sense::click());
        widgets::picker::paint_swatch(ui.painter(), sw, p);
        let var_key = format!("{}/{i}", if stroke { "strokes" } else { "fills" });
        let bound = var_bind::pill_field(app, ui, Rect::from_min_max(hex_r.min, Pos2::new(g.right(), hex_r.bottom())), sel, &[var_key.as_str()]);
        if !bound {
            widgets::vline(ui.painter(), g.left() + PAINT_DIVIDER_X, g.top(), g.bottom(), theme::BG);
        }
        if swr.clicked() && !bound {
            app.props.selected_property = Some(prop);
            let target = if stroke { PickTarget::Stroke(i) } else { PickTarget::Fill(i) };
            open_picker(app, target, paint_preview(p), sw, sel.to_vec());
        }
        let font = theme::font_body();
        match &p.kind {
            _ if bound => {}
            _ if linked.is_some() => {
                widgets::field_text(ui.painter(), hex_r, 0.0, 4.0, linked.as_deref().unwrap_or(""), font.clone(), theme::TEXT);
            }
            PaintKind::Solid(c) => {
                if let Some(s) = field::edit_hex_text(ui, hex_r, hex_id, &c.to_hex(), font.clone(), theme::TEXT, 0.0) {
                    if let Some(nc) = Color::from_hex(&s) {
                        edit_paint_list(app, sel, stroke, |ps| {
                            if let Some(p) = ps.get_mut(i) {
                                p.kind = PaintKind::Solid(Color { a: c.a, ..nc });
                            }
                        });
                    }
                }
            }
            _ => {
                widgets::field_text(ui.painter(), hex_r, 0.0, 4.0, &paint_label(p), font.clone(), theme::TEXT);
            }
        }
        if !bound && matches!(p.kind, PaintKind::Solid(_)) {
            var_bind::hover_button(app, ui, hex_r, sel, &[var_key.as_str()], VariableKind::Color);
        }
        if !bound {
            // Opacity: "100" then a separate "%" in text_secondary.
            let pct_val = (p.opacity * 100.0).round();
            if let Some(v) = field::edit_num_text(ui, op_r, op_id, Some(pct_val), true, font.clone(), theme::TEXT, 8.0, None) {
                let o = (v / 100.0).clamp(0.0, 1.0);
                edit_paint_list(app, sel, stroke, |ps| {
                    if let Some(p) = ps.get_mut(i) {
                        p.opacity = o;
                    }
                });
            }
            let pct_x = g.right() - 8.0 - widgets::text_width(ui.painter(), "%", &font);
            widgets::text_on_baseline(ui.painter(), pct_x, widgets::centered_baseline(g, &font), "%", font, theme::TEXT_SECONDARY);
        }
        if widgets::icon_button(ui, eye_r, ("peye", key), if p.visible { Icon::Eye } else { Icon::EyeOff }, false, "").clicked() {
            edit_paint_list(app, sel, stroke, |ps| {
                if let Some(p) = ps.get_mut(i) {
                    p.visible = !p.visible;
                }
            });
        }
        if widgets::icon_button(ui, minus_r, ("pminus", key), Icon::Minus, false, "").clicked() {
            edit_paint_list(app, sel, stroke, |ps| {
                if i < ps.len() {
                    ps.remove(i);
                }
            });
        }
        pane.y += theme::FIELD_HEIGHT + 8.0;
    }
    if let Some((src_i, dst_i)) = reorder_pair {
        edit_paint_list(app, sel, stroke, |ps| {
            if src_i < ps.len() {
                let item = ps.remove(src_i);
                let target_idx = dst_i.min(ps.len());
                ps.insert(target_idx, item);
            }
        });
        app.props.selected_property = Some(if stroke { SelectedProperty::Stroke(dst_i) } else { SelectedProperty::Fill(dst_i) });
        app.props.drag_property = None;
    } else if ui.ctx().input(|inp| inp.pointer.any_released())
        && matches!((stroke, app.props.drag_property), (false, Some(SelectedProperty::Fill(_))) | (true, Some(SelectedProperty::Stroke(_))))
    {
        app.props.drag_property = None;
    }
}

// ---------------------------------------------------------------- color picker
fn node_paint(app: &App, target: PickTarget, ids: &[NodeId]) -> Option<Paint> {
    let n = app.doc.get(*ids.first()?)?;
    match target {
        PickTarget::Fill(i) => n.fills.get(i).cloned(),
        PickTarget::Stroke(i) => n.stroke.paints.get(i).cloned(),
        _ => None,
    }
}

/// Colours for the picker's swatch grid: this page, the whole document, colour styles, and colour variables.
fn swatch_sources(app: &App) -> Vec<(String, Vec<Color32>)> {
    type SwatchCacheEntry = ((u64, NodeId, u64, usize, usize), Vec<(String, Vec<Color32>)>);
    thread_local! {
        static SWATCH_CACHE: std::cell::RefCell<Option<SwatchCacheEntry>> = const { std::cell::RefCell::new(None) };
    }
    let key = (app.doc.runtime_id(), app.page, app.doc.mutation_epoch(), app.doc.styles.len(), app.doc.variables.len());
    if let Some(cached) = SWATCH_CACHE.with(|c| c.borrow().as_ref().filter(|(k, _)| *k == key).map(|(_, v)| v.clone())) {
        return cached;
    }
    let conv = |cs: Vec<Color>| cs.into_iter().map(|c| to_c32(c, 1.0)).collect::<Vec<_>>();
    let page = conv(crate::styles::selection_colors(&app.doc, &[app.page]));
    let doc = if app.doc.nodes.len() > 20_000 { page.clone() } else { conv(crate::styles::selection_colors(&app.doc, &app.doc.pages.clone())) };
    let styles: Vec<Color32> = crate::styles::style_entries(&app.doc, false)
        .iter()
        .filter_map(|e| match app.doc.styles.get(&e.key) {
            Some(Style::Fill(ps)) => ps.first().map(widgets::picker::preview_color),
            _ => None,
        })
        .collect();
    let mut vars: Vec<Color32> = Vec::new();
    if !app.doc.variable_collections.is_empty() {
        let store = VariableStore::from_doc(&app.doc);
        for coll in app.doc.variable_collections.values() {
            for vid in &coll.variables {
                if let Some(v) = app.doc.variables.get(vid).filter(|v| v.kind == VariableKind::Color) {
                    if let Some(VariableValue::Color(c)) = store.resolve(&v.id, &coll.default_mode) {
                        vars.push(to_c32(c, c.a));
                    }
                }
            }
        }
    }
    let mut out = vec![("On this page".to_string(), page), ("Document colors".to_string(), doc)];
    if !styles.is_empty() {
        out.push(("Color styles".to_string(), styles));
    }
    if !vars.is_empty() {
        out.push(("Color variables".to_string(), vars));
    }
    SWATCH_CACHE.with(|c| {
        *c.borrow_mut() = Some((key, out.clone()));
    });
    out
}

pub(super) fn open_picker(app: &mut App, target: PickTarget, color: Color32, anchor: Rect, ids: Vec<NodeId>) {
    let paint = node_paint(app, target, &ids).unwrap_or_else(|| {
        let [r, g, b, a] = color.to_srgba_unmultiplied();
        Paint { opacity: a as f32 / 255.0, ..Paint::solid(Color::rgba(r, g, b, 255)) }
    });
    let state = widgets::picker::State::new(&paint);
    let sources = swatch_sources(app);
    app.props.picker = Some(Picker { target, tx: Tx::new(), anchor, ids, paint, state, sources, eyedrop: false });
}

/// Solid colour (alpha included) the picker currently holds, for the solid-only targets.
fn picker_color(p: &Picker) -> Color {
    let [r, g, b, a] = widgets::picker::primary_rgba(&p.paint);
    Color::rgba(r, g, b, a)
}

fn apply_picker(doc: &mut Document, p: &mut Picker) {
    let c = picker_color(p);
    if let PickTarget::SelColor(old) = p.target {
        if let Some(old) = Color::from_hex(&format!("{old:06X}")) {
            crate::styles::replace_color(doc, &mut p.tx.ed, &p.ids, old, Color { a: 1.0, ..c });
        }
        return;
    }
    for id in &p.ids {
        let Some(n) = p.tx.ed.node(doc, *id) else { continue };
        match p.target {
            PickTarget::Fill(i) => {
                if let Some(pt) = n.fills.get_mut(i) {
                    let visible = pt.visible;
                    *pt = Paint { visible, ..p.paint.clone() };
                }
            }
            PickTarget::Stroke(i) => {
                if let Some(pt) = n.stroke.paints.get_mut(i) {
                    let visible = pt.visible;
                    *pt = Paint { visible, ..p.paint.clone() };
                }
            }
            PickTarget::SelColor(_) => {}
            PickTarget::GridColor(i) => {
                if let Some(fp) = n.kind.frame_props_mut() {
                    if let Some(g) = fp.grids.get_mut(i) {
                        *g = super::layoutguide::with_color(g, c);
                    }
                }
            }
            PickTarget::TextFill => {
                if let NodeKind::Text(t) = &mut n.kind {
                    let col = Color { a: 1.0, ..c };
                    let op = c.a;
                    crate::richtext::apply_all(t, |st| super::typography::set_solid(st, col, Some(op)));
                }
            }
            PickTarget::PageBg => n.kind = NodeKind::Page { background: c },
            PickTarget::Shadow(i) => {
                if let Some(e) = n.effects.get_mut(i) {
                    super::effects::set_shadow(e, |col, _, _, _| *col = c);
                }
            }
            PickTarget::NoiseColor(i, second) => {
                if let Some(e) = n.effects.get_mut(i) {
                    super::effects::set_noise_color(e, second, c);
                }
            }
        }
    }
}

/// Reads the page colour under `pos` (screen space) by rendering a 1x1 pixel of the page.
pub fn sample_canvas(app: &App, pos: Pos2) -> Option<[u8; 3]> {
    use designos_render::tiny_skia::Pixmap;
    let p = app.view.to_page(app.canvas_rect.min, pos);
    let vp = designos_render::Viewport { offset: DVec2::new(p.x - 0.5, p.y - 0.5), zoom: 1.0, dpr: 1.0 };
    let mut pm = Pixmap::new(1, 1)?;
    designos_render::render_page(&app.doc, app.page, &vp, &mut pm);
    let px = pm.pixel(0, 0)?.demultiply();
    Some([px.red(), px.green(), px.blue()])
}

/// Creates a colour style from the picker's paint and links it to the edited layers.
fn create_style(app: &mut App, pk: &mut Picker) {
    let stroke = matches!(pk.target, PickTarget::Stroke(_));
    let mut n = 1;
    while app.doc.styles.contains_key(&format!("Color style {n}")) {
        n += 1;
    }
    let key = format!("Color style {n}");
    // Creating the style and linking it to the layers is one undo step (the picker's transaction).
    let mut styles = app.doc.styles.clone();
    let mut style_meta = app.doc.style_meta.clone();
    styles.insert(key.clone(), Style::Fill(vec![Paint { visible: true, ..pk.paint.clone() }]));
    style_meta.insert(
        key.clone(),
        StyleMeta {
            name: key.clone(),
            key: None,
            description: String::new(),
            property: if stroke { StyleProperty::Stroke } else { StyleProperty::Fill },
            bindings: Default::default(),
        },
    );
    if let Err(e) = pk.tx.cmd(&mut app.doc, Command::SetStyles { styles, style_meta }) {
        app.show_toast(format!("Edit failed: {e}"), true);
        return;
    }
    for id in &pk.ids {
        if let Some(node) = pk.tx.ed.node(&mut app.doc, *id) {
            node.style_refs.insert(if stroke { "stroke" } else { "fill" }.into(), key.clone());
        }
    }
    app.show_toast(format!("Created \"{key}\""), false);
}

fn try_sample_color_token(app: &mut App, p: Pos2, target: PickTarget, ids: &[NodeId]) -> bool {
    let hit = app.hover.or_else(|| crate::canvas::interact::hit_under(app, app.view.to_page(app.canvas_rect.min, p), true));
    let Some(hid) = hit else { return false };
    let (style_key, var_id) = match app.doc.get(hid) {
        Some(n) => (n.style_refs.get("fill").cloned(), n.variables.bindings.get("fills/0").cloned()),
        None => return false,
    };
    if let Some(key) = style_key {
        app.apply_style(ids, &key);
        app.show_toast("Sampled color token", false);
        return true;
    }
    if let Some(vid) = var_id {
        let prop_key = match target {
            PickTarget::Stroke(i) => format!("strokes/{i}"),
            PickTarget::Fill(i) => format!("fills/{i}"),
            _ => "fills/0".to_string(),
        };
        app.bind_variable(ids, &[prop_key.as_str()], &vid);
        app.show_toast("Sampled color token", false);
        return true;
    }
    false
}

pub(super) fn picker_popup(app: &mut App, ui: &mut Ui, region: Rect) {
    use widgets::picker;
    let ctx = ui.ctx().clone();
    let Some(mut pk) = app.props.picker.take() else {
        if let Some(target) = app.props.eyedropper {
            ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
            let (clicked, ptr, esc, shift) =
                ctx.input(|i| (i.pointer.primary_pressed(), i.pointer.interact_pos(), i.key_pressed(egui::Key::Escape), i.modifiers.shift));
            if esc {
                app.props.eyedropper = None;
            } else if let (true, Some(p)) = (clicked, ptr) {
                if app.canvas_rect.contains(p) && !region.contains(p) {
                    let sel = app.selection.clone();
                    if !(shift && try_sample_color_token(app, p, target, &sel)) {
                        if let Some([r, g, b]) = sample_canvas(app, p) {
                            let col = Color::rgb(r, g, b);
                            app.edit_nodes(&sel, |n| match target {
                                PickTarget::Fill(i) => {
                                    if let Some(pt) = n.fills.get_mut(i) {
                                        pt.kind = PaintKind::Solid(col);
                                    } else {
                                        n.fills.push(Paint::solid(col));
                                    }
                                }
                                PickTarget::Stroke(i) => {
                                    if let Some(pt) = n.stroke.paints.get_mut(i) {
                                        pt.kind = PaintKind::Solid(col);
                                    } else {
                                        n.stroke.paints.push(Paint::solid(col));
                                    }
                                }
                                _ => {
                                    if let Some(pt) = n.fills.first_mut() {
                                        pt.kind = PaintKind::Solid(col);
                                    } else {
                                        n.fills.push(Paint::solid(col));
                                    }
                                }
                            });
                        }
                    }
                    app.props.eyedropper = None;
                }
            }
        }
        return;
    };
    let paint_types = matches!(pk.target, PickTarget::Fill(_) | PickTarget::Stroke(_));
    let mut styles: Vec<picker::StyleItem> = if paint_types {
        crate::styles::style_entries(&app.doc, false)
            .into_iter()
            .filter_map(|e| match app.doc.styles.get(&e.key) {
                Some(Style::Fill(ps)) => {
                    ps.first().map(|p| picker::StyleItem { name: app.doc.style_name(&e.key).to_string(), key: e.key.clone(), paint: p.clone(), library: None })
                }
                _ => None,
            })
            .collect()
    } else {
        Vec::new()
    };
    // Colour styles of enabled libraries and local Color Variables, grouped after the document's own styles.
    if paint_types && pk.state.tab == picker::Tab::Libraries {
        let store = VariableStore::from_doc(&app.doc);
        for coll in app.doc.variable_collections.values() {
            for vid in &coll.variables {
                if let Some(v) = app.doc.variables.get(vid).filter(|v| v.kind == VariableKind::Color) {
                    if let Some(VariableValue::Color(c)) = store.resolve(&v.id, &coll.default_mode) {
                        styles.push(picker::StyleItem {
                            name: v.name.clone(),
                            key: v.id.clone(),
                            paint: Paint { opacity: c.a, ..Paint::solid(Color { a: 1.0, ..c }) },
                            library: Some(("__vars__".to_string(), format!("Variables · {}", coll.name))),
                        });
                    }
                }
            }
        }
        for ls in app.library_styles().into_iter().filter(|s| !s.text && s.local_key.is_none()) {
            if let Style::Fill(ps) = &ls.style {
                if let Some(p) = ps.first() {
                    styles.push(picker::StyleItem {
                        name: ls.name.clone(),
                        key: ls.key.clone(),
                        paint: p.clone(),
                        library: Some((ls.library_id.clone(), ls.library_name.clone())),
                    });
                }
            }
        }
    }
    let env = picker::Env { paint_types, sources: &pk.sources, styles: &styles, can_add_style: paint_types };
    let h = picker::panel_height(&env, picker::mode_of(&pk.paint.kind), pk.state.tab, pk.state.source);
    let screen = ctx.content_rect();
    let x = (region.left() - picker::W).max(screen.left() + 8.0);
    let y = (pk.anchor.top() - 8.0).min(screen.bottom() - h - 16.0).max(screen.top() + 8.0);
    let mut out = picker::Out::default();
    let mut panel = Rect::NOTHING;
    egui::Area::new(egui::Id::new("color-picker")).order(egui::Order::Foreground).fixed_pos(Pos2::new(x, y)).show(&ctx, |ui| {
        let (r, o) = picker::show(ui, Pos2::new(x, y), &mut pk.state, &mut pk.paint, &env);
        panel = r;
        out = o;
    });
    let mut changed = out.changed;
    if let Some((key, bytes)) = out.generated_image {
        app.doc.images.insert(key, bytes);
        changed = true;
    }
    if out.eyedrop || app.props.eyedropper.is_some() {
        pk.eyedrop = true;
        app.props.eyedropper = Some(pk.target);
    }
    if out.want_image {
        if let Some(path) = rfd::FileDialog::new().add_filter("Images", &crate::canvas::interact::IMAGE_EXTENSIONS[..4]).pick_file() {
            match std::fs::read(&path).map_err(|_| "Cannot read image".to_string()).and_then(crate::canvas::interact::prepare_image) {
                Ok((bytes, _, _)) => {
                    let key = format!("img-{}", app.doc.images.len() + 1);
                    app.doc.images.insert(key.clone(), bytes);
                    pk.paint.kind = PaintKind::Image { image_ref: key, scale_mode: ImageScaleMode::Fill };
                    pk.paint.opacity = 1.0;
                    changed = true;
                }
                Err(e) => app.show_toast(e, true),
            }
        }
    }
    // Eyedropper: the next canvas click samples the colour under the pointer.
    if pk.eyedrop {
        ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
        let (clicked, ptr, esc, shift) =
            ctx.input(|i| (i.pointer.primary_pressed(), i.pointer.interact_pos(), i.key_pressed(egui::Key::Escape), i.modifiers.shift));
        if esc {
            pk.eyedrop = false;
            app.props.eyedropper = None;
        } else if let (true, Some(p)) = (clicked, ptr) {
            if app.canvas_rect.contains(p) && !panel.contains(p) {
                let ids = pk.ids.clone();
                if shift && try_sample_color_token(app, p, pk.target, &ids) {
                    pk.eyedrop = false;
                    app.props.eyedropper = None;
                } else {
                    if let Some([r, g, b]) = sample_canvas(app, p) {
                        let a = widgets::picker::active_rgba(&pk.paint, pk.state.sel)[3];
                        widgets::picker::set_active(&mut pk.paint, pk.state.sel, [r, g, b, a]);
                        changed = true;
                    }
                    pk.eyedrop = false;
                    app.props.eyedropper = None;
                }
            }
        }
    }
    gradient_handles(app, &ctx, &mut pk, &mut changed);
    if changed {
        if pk.target == PickTarget::TextFill && app.canvas.text.is_some() {
            // Colour of the characters being edited: goes through the text session.
            let c = picker_color(&pk);
            let col = Color { a: 1.0, ..c };
            crate::canvas::textsession::style_selection_as(app, |st| super::typography::set_solid(st, col, Some(c.a)), crate::textedit::EditKind::Style);
        } else {
            pk.tx.ed.revert(&mut app.doc);
            apply_picker(&mut app.doc, &mut pk);
            app.changed();
        }
    }
    if out.add_style {
        create_style(app, &mut pk);
        app.changed();
        out.close = true;
    }
    let mut remove = false;
    if out.remove {
        pk.tx.ed.revert(&mut app.doc);
        remove = true;
    }
    let (pressed, ptr, esc) = ctx.input(|i| (i.pointer.any_pressed(), i.pointer.interact_pos(), i.key_pressed(egui::Key::Escape)));
    let outside =
        !pk.eyedrop && !pk.state.menu_open && pressed && ptr.map(|p| !panel.contains(p) && !pk.anchor.contains(p) && !handle_hit(&pk, app, p)).unwrap_or(false);
    let esc = esc && !pk.eyedrop && !pk.state.menu_open;
    if let Some((lib, key)) = out.apply_library_style.take() {
        let ids = pk.ids.clone();
        if lib == "__vars__" {
            let prop_key = match pk.target {
                PickTarget::Stroke(i) => format!("strokes/{i}"),
                PickTarget::Fill(i) => format!("fills/{i}"),
                _ => "fills/0".to_string(),
            };
            app.run_tx(pk.tx);
            app.bind_variable(&ids, &[prop_key.as_str()], &key);
        } else {
            // Import the library style and apply it together with the picker's own edits: one undo step.
            app.apply_library_style(pk.tx, &ids, &lib, &key);
        }
        return;
    }
    if outside || esc || out.close || out.apply_style.is_some() || remove {
        app.props.eyedropper = None;
        let ids = pk.ids.clone();
        let target = pk.target;
        let tx = pk.tx;
        if remove {
            // Drop the paint in the same undo step.
            let mut tx = tx;
            for id in &ids {
                if let Some(n) = tx.ed.node(&mut app.doc, *id) {
                    match target {
                        PickTarget::Fill(i) if i < n.fills.len() => {
                            n.fills.remove(i);
                        }
                        PickTarget::Stroke(i) if i < n.stroke.paints.len() => {
                            n.stroke.paints.remove(i);
                        }
                        _ => {}
                    }
                }
            }
            app.run_tx(tx);
        } else {
            app.run_tx(tx);
        }
        if let Some(key) = out.apply_style {
            app.apply_style(&ids, &key);
        }
    } else {
        app.props.picker = Some(pk);
    }
}

/// Handles of the gradient on the canvas are in screen space; clicks on them must not close the picker.
fn handle_hit(pk: &Picker, app: &App, p: Pos2) -> bool {
    let pts = handle_points(pk, app);
    if pts.iter().any(|(_, c)| c.distance(p) < 12.0) {
        return true;
    }
    if pts.len() >= 2 {
        let a = pts[0].1;
        let b = pts[1].1;
        let ab = b - a;
        let len_sq = ab.length_sq();
        if len_sq > 1.0 {
            let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
            let proj = a + ab * t;
            if proj.distance(p) < 12.0 {
                return true;
            }
        }
    }
    false
}

/// Screen positions of the three gradient handles of the edited node.
fn handle_points(pk: &Picker, app: &App) -> Vec<(usize, Pos2)> {
    let Some(h) = widgets::picker::handles_of(&pk.paint.kind) else { return Vec::new() };
    let Some(id) = pk.ids.first() else { return Vec::new() };
    let Some(n) = app.doc.get(*id) else { return Vec::new() };
    let w = app.doc.world_transform(*id);
    h.iter()
        .enumerate()
        .map(|(i, v)| {
            let page = w.apply(DVec2::new(v.x * n.size.x, v.y * n.size.y));
            (i, app.view.to_screen(app.canvas_rect.min, page))
        })
        .collect()
}

/// On-canvas gradient editing: the start/end/width handles and interactive stops of the edited paint.
fn gradient_handles(app: &mut App, ctx: &egui::Context, pk: &mut Picker, changed: &mut bool) {
    let pts = handle_points(pk, app);
    if pts.len() != 3 || !matches!(pk.target, PickTarget::Fill(_) | PickTarget::Stroke(_)) {
        return;
    }
    let Some(id) = pk.ids.first().copied() else { return };
    let (Some(n), Some(inv)) = (app.doc.get(id), app.doc.world_transform(id).invert()) else { return };
    let size = n.size;
    let origin = app.canvas_rect.min;
    let view = app.view;
    let canvas = app.canvas_rect;
    let mut moved: Option<(usize, DVec2, bool)> = None;
    let mut stop_selected: Option<usize> = None;
    let mut stop_moved: Option<(usize, f32)> = None;
    let mut stop_duplicated: Option<(usize, f32)> = None;
    let mut line_inserted: Option<f32> = None;
    let mut any_handle_or_stop_interacted = false;

    egui::Area::new(egui::Id::new("gradient-handles")).order(egui::Order::Foreground).fixed_pos(canvas.min).interactable(true).show(ctx, |ui| {
        let p = ui.painter().with_clip_rect(canvas);
        let stroke = |w: f32, c: Color32| Stroke::new(w, c);
        p.line_segment([pts[0].1, pts[1].1], stroke(3.0, Color32::from_black_alpha(0x66)));
        p.line_segment([pts[0].1, pts[1].1], stroke(1.5, Color32::WHITE));
        p.line_segment([pts[0].1, pts[2].1], stroke(1.0, Color32::from_white_alpha(0xB2)));
        let stops = widgets::picker::stops_of(&pk.paint.kind).cloned().unwrap_or_default();
        let shift = ui.input(|inp| inp.modifiers.shift);
        let alt = ui.input(|inp| inp.modifiers.alt);

        for (i, c) in &pts {
            let r = Rect::from_center_size(*c, Vec2::splat(16.0));
            let resp = ui.interact(r, ui.id().with(("gh", *i)), Sense::click_and_drag());
            if resp.clicked() || resp.dragged() || resp.drag_started() {
                any_handle_or_stop_interacted = true;
            }
            if resp.dragged() {
                if let Some(mut pt) = resp.interact_pointer_pos() {
                    if shift && (*i == 0 || *i == 1) {
                        let anchor = if *i == 0 { pts[1].1 } else { pts[0].1 };
                        let d = pt - anchor;
                        let len = d.length();
                        if len > 1.0 {
                            let step = 15.0_f32.to_radians();
                            let snapped = (d.y.atan2(d.x) / step).round() * step;
                            pt = anchor + egui::vec2(snapped.cos() * len, snapped.sin() * len);
                        }
                    }
                    let local = inv.apply(view.to_page(origin, pt));
                    moved = Some((*i, DVec2::new(local.x / size.x.max(1e-3), local.y / size.y.max(1e-3)), shift));
                }
            }
            let fill = if *i == 0 {
                stops.first().map(|s| to_c32(s.color, 1.0))
            } else if *i == 1 {
                stops.last().map(|s| to_c32(s.color, 1.0))
            } else {
                None
            };
            let rad = if *i == 2 { 4.0 } else { 6.5 };
            p.circle_filled(*c, rad + 1.5, Color32::from_black_alpha(0x4D));
            p.circle_filled(*c, rad, Color32::WHITE);
            if let Some(f) = fill {
                p.circle_filled(*c, rad - 2.0, f);
            }
        }

        // Interactive stop squares along the gradient axis.
        let ab = pts[1].1 - pts[0].1;
        let ab_len_sq = ab.length_sq().max(1.0);
        for (si, s) in stops.iter().enumerate() {
            let c = pts[0].1 + ab * s.pos.clamp(0.0, 1.0);
            let sr = Rect::from_center_size(c, Vec2::splat(12.0));
            let sresp = ui.interact(sr, ui.id().with(("gh-stop", si)), Sense::click_and_drag());
            if sresp.clicked() || sresp.drag_started() || sresp.dragged() {
                any_handle_or_stop_interacted = true;
                stop_selected = Some(si);
            }
            if sresp.drag_started() && alt {
                if let Some(pt) = sresp.interact_pointer_pos() {
                    let t = ((pt - pts[0].1).dot(ab) / ab_len_sq).clamp(0.0, 1.0);
                    stop_duplicated = Some((si, t));
                }
            } else if sresp.dragged() {
                if let Some(pt) = sresp.interact_pointer_pos() {
                    let t = ((pt - pts[0].1).dot(ab) / ab_len_sq).clamp(0.0, 1.0);
                    stop_moved = Some((si, t));
                }
            }
            let sel = pk.state.sel == si;
            p.rect_filled(Rect::from_center_size(c, Vec2::splat(10.0)), theme::cr(3), if sel { Color32::WHITE } else { Color32::from_white_alpha(0xCC) });
            if sel {
                p.rect_stroke(Rect::from_center_size(c, Vec2::splat(12.0)), theme::cr(3), Stroke::new(1.5, theme::BORDER_SELECTED), StrokeKind::Outside);
            }
            p.rect_filled(Rect::from_center_size(c, Vec2::splat(6.0)), theme::cr(2), to_c32(s.color, 1.0));
        }

        // Click on the gradient line between stops to insert a new interpolated stop.
        if !any_handle_or_stop_interacted {
            let (clicked, ptr) = ui.input(|inp| (inp.pointer.primary_clicked(), inp.pointer.interact_pos()));
            if let (true, Some(pt)) = (clicked, ptr) {
                let t = ((pt - pts[0].1).dot(ab) / ab_len_sq).clamp(0.0, 1.0);
                let proj = pts[0].1 + ab * t;
                let near_endpoint = pts.iter().any(|(_, ep)| ep.distance(pt) < 12.0);
                let near_stop = stops.iter().any(|s| (pts[0].1 + ab * s.pos.clamp(0.0, 1.0)).distance(pt) < 10.0);
                if proj.distance(pt) < 8.0 && (0.02..=0.98).contains(&t) && !near_endpoint && !near_stop {
                    line_inserted = Some(t);
                }
            }
        }
    });

    if let Some(si) = stop_selected {
        pk.state.sel = si;
    }
    if let Some((si, t)) = stop_duplicated {
        if let Some(stops) = widgets::picker::stops_mut(&mut pk.paint.kind) {
            if let Some(src) = stops.get(si).cloned() {
                stops.push(GradientStop { pos: t, color: src.color });
                pk.state.sel = stops.len() - 1;
                *changed = true;
            }
        }
    } else if let Some((si, t)) = stop_moved {
        if let Some(stops) = widgets::picker::stops_mut(&mut pk.paint.kind) {
            let idx = pk.state.sel.min(stops.len().saturating_sub(1)).max(si.min(stops.len().saturating_sub(1)));
            if let Some(s) = stops.get_mut(idx) {
                s.pos = t;
                *changed = true;
            }
        }
    } else if let Some(t) = line_inserted {
        if let Some(stops) = widgets::picker::stops_mut(&mut pk.paint.kind) {
            let color = widgets::picker::stop_color_at(stops, t);
            stops.push(GradientStop { pos: t, color });
            stops.sort_by(|a, b| a.pos.partial_cmp(&b.pos).unwrap_or(std::cmp::Ordering::Equal));
            if let Some(idx) = stops.iter().position(|s| (s.pos - t).abs() < 1e-4) {
                pk.state.sel = idx;
            }
            *changed = true;
        }
    }
    if let Some((i, v, shift_snapped)) = moved {
        let v = DVec2::new(v.x, v.y);
        match &mut pk.paint.kind {
            PaintKind::LinearGradient { handles, .. }
            | PaintKind::RadialGradient { handles, .. }
            | PaintKind::AngularGradient { handles, .. }
            | PaintKind::DiamondGradient { handles, .. } => {
                if i == 0 && !shift_snapped {
                    // Moving the start/centre without Shift drags the whole gradient.
                    let d = DVec2::new(v.x - handles[0].x, v.y - handles[0].y);
                    for h in handles.iter_mut() {
                        *h = DVec2::new(h.x + d.x, h.y + d.y);
                    }
                } else {
                    handles[i] = v;
                }
                *changed = true;
            }
            _ => {}
        }
    }
}

fn find_node_in_page(doc: &Document, root: NodeId, pred: &dyn Fn(&Node) -> bool) -> Option<NodeId> {
    let n = doc.get(root)?;
    if !matches!(n.kind, NodeKind::Page { .. }) && pred(n) {
        return Some(root);
    }
    for &c in &n.children {
        if let Some(found) = find_node_in_page(doc, c, pred) {
            return Some(found);
        }
    }
    None
}

/// Opens an inspector popup on the current selection (`--panel`, for screenshots).
pub fn preset_popup(app: &mut App, spec: &str, screen: Vec2) {
    if spec == "variables" {
        app.vars.open = true;
        app.vars.full = true;
        return;
    }
    if spec == "picker-page" {
        // The page background picker, anchored on the Page row swatch.
        let bg = match app.doc.get(app.page).map(|n| &n.kind) {
            Some(NodeKind::Page { background }) => *background,
            _ => Color::WHITE,
        };
        let sw =
            Rect::from_min_size(Pos2::new(screen.x - theme::RIGHT_PANEL_WIDTH + 17.0 + theme::PAGE_SWATCH_CX - 6.0, 131.0), Vec2::splat(theme::PAGE_SWATCH));
        open_picker(app, PickTarget::PageBg, to_c32(bg, 1.0), sw, vec![app.page]);
        return;
    }
    if spec == "type-details" || spec == "edit-style-text" {
        let has_text = app.selection.first().and_then(|id| app.doc.get(*id)).is_some_and(|n| matches!(n.kind, NodeKind::Text(_)));
        if !has_text {
            if let Some(tid) = find_node_in_page(&app.doc, app.page, &|n| matches!(n.kind, NodeKind::Text(_))) {
                app.selection = vec![tid];
            }
        }
    } else if app.selection.is_empty() {
        if let Some(fid) = find_node_in_page(&app.doc, app.page, &|n| !n.fills.is_empty()).or_else(|| find_node_in_page(&app.doc, app.page, &|_| true)) {
            app.selection = vec![fid];
        }
    }
    let Some(id) = app.selection.first().copied() else { return };
    let sw = Rect::from_min_size(Pos2::new(screen.x - theme::RIGHT_PANEL_WIDTH + 29.0, 660.0), Vec2::splat(theme::SWATCH_SIZE));
    match spec {
        "picker" | "picker-gradient" => {
            if app.doc.get(id).is_some_and(|n| n.fills.is_empty()) {
                app.edit_nodes(&[id], |n| n.fills.push(Paint::solid(Color::rgb(0xD9, 0xD9, 0xD9))));
            }
            if spec == "picker-gradient" {
                app.edit_nodes(&[id], |n| {
                    if let Some(p) = n.fills.first_mut() {
                        *p = widgets::picker::convert(p, widgets::picker::Mode::Linear);
                    }
                });
            }
            let c = app.doc.get(id).and_then(|n| n.fills.first()).map(paint_preview).unwrap_or(Color32::GRAY);
            open_picker(app, PickTarget::Fill(0), c, sw, vec![id]);
        }
        "fill-d9" => {
            // Screenshot aid: the Figma reference fill (#D9D9D9, 100 %) for crop comparisons.
            app.edit_nodes(&[id], |n| n.fills = vec![Paint::solid(Color::rgb(0xD9, 0xD9, 0xD9))]);
        }
        "effects-menu" => {
            app.props.scroll = 1.0e6;
            app.props.fx_menu = true;
        }
        "effect-styles" | "grid-styles" => {
            // The style menu of the Effects / Layout guide header. A frame without guides gets the
            // usual 12 column ones, and with no style yet one is created from the selection.
            let prop = if spec == "grid-styles" { StyleProperty::Grid } else { StyleProperty::Effect };
            if prop == StyleProperty::Grid && app.doc.get(id).and_then(|n| n.kind.frame_props()).is_some_and(|f| f.grids.is_empty()) {
                let grid = LayoutGrid::columns(12, 20.0, 0.0, Color::rgba(255, 0, 0, 26));
                app.edit_nodes(&[id], |n| {
                    if let Some(f) = n.kind.frame_props_mut() {
                        f.grids.push(grid.clone());
                    }
                });
            }
            let kind = if prop == StyleProperty::Grid { StyleKind::Grid } else { StyleKind::Effect };
            if crate::styles::style_entries_of(&app.doc, kind).is_empty() {
                app.create_style_from(id, prop);
            }
            app.props.scroll = 1.0e6;
            app.props.style_menu = Some(prop);
        }
        "edit-style-color" | "edit-style-text" | "edit-style-effect" | "edit-style-grid" => {
            // The "Edit style" popover on the first style of its kind, linked to the selection. A
            // frame without guides gets the usual 12 column ones and a layer without effects a
            // drop shadow, so the style is created from real values like the effect-styles panel.
            app.props.scroll = 1.0e6;
            let kind = match spec {
                "edit-style-color" => StyleKind::Color,
                "edit-style-text" => StyleKind::Text,
                "edit-style-effect" => StyleKind::Effect,
                _ => StyleKind::Grid,
            };
            if kind == StyleKind::Effect && app.doc.get(id).is_some_and(|n| n.effects.is_empty()) {
                let fx = super::effects::default_effect(1, 1);
                app.edit_nodes(&[id], |n| n.effects.push(fx.clone()));
            }
            if kind == StyleKind::Grid && app.doc.get(id).and_then(|n| n.kind.frame_props()).is_some_and(|f| f.grids.is_empty()) {
                let grid = LayoutGrid::columns(12, 20.0, 0.0, Color::rgba(255, 0, 0, 26));
                app.edit_nodes(&[id], |n| {
                    if let Some(f) = n.kind.frame_props_mut() {
                        f.grids.push(grid.clone());
                    }
                });
            }
            if crate::styles::style_entries_of(&app.doc, kind).is_empty() {
                match kind {
                    StyleKind::Effect => app.create_style_from(id, StyleProperty::Effect),
                    StyleKind::Grid => app.create_style_from(id, StyleProperty::Grid),
                    _ => None,
                };
            }
            if let Some(entry) = crate::styles::style_entries_of(&app.doc, kind).into_iter().next() {
                app.apply_style(&[id], &entry.key);
                let anchor = Rect::from_min_size(Pos2::new(screen.x - 56.0, 420.0), Vec2::splat(24.0));
                super::style_edit::toggle(app, &entry.key, anchor);
            }
        }
        "stroke-settings" => {
            app.props.scroll = 1.0e6;
            if app.doc.get(id).is_some_and(|n| n.stroke.paints.is_empty()) {
                app.edit_nodes(&[id], |n| {
                    n.stroke.paints.push(Paint::solid(Color::BLACK));
                    n.stroke.weight = 1.0;
                });
            }
            let anchor = Rect::from_min_size(Pos2::new(screen.x - 56.0, 760.0), Vec2::splat(24.0));
            app.props.pop = Some(super::effects::PopState { kind: super::effects::PopKind::Stroke, anchor, ids: vec![id] });
        }
        "type-details" => {
            let anchor = Rect::from_min_size(Pos2::new(screen.x - 48.0, 330.0), Vec2::splat(24.0));
            app.props.pop = Some(super::effects::PopState { kind: super::effects::PopKind::TextDetails, anchor, ids: vec![id] });
        }
        "effect-noise" | "effect-texture" | "effect-glass" => {
            app.props.scroll = 1.0e6;
            let kind = match spec {
                "effect-noise" => 4,
                "effect-texture" => 5,
                _ => 6,
            };
            let e = super::effects::default_effect(kind, 1);
            app.edit_nodes(&[id], |n| {
                n.effects.push(e.clone());
                if kind == 6 {
                    // Glass shows what is behind it: make the fill a faint tint.
                    n.fills = vec![Paint { opacity: 0.12, ..Paint::solid(Color::WHITE) }];
                }
            });
            let i = app.doc.get(id).map(|n| n.effects.len().saturating_sub(1)).unwrap_or(0);
            let anchor = Rect::from_min_size(Pos2::new(screen.x - 80.0, 800.0), Vec2::splat(24.0));
            app.props.pop = Some(super::effects::PopState { kind: super::effects::PopKind::Effect(i), anchor, ids: vec![id] });
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_move_button_icon_and_tip_name_the_direction_the_track_goes() {
        let (icon, tip, to) = track_move_button(0, 3);
        assert_eq!((icon, tip, to), (Icon::ChevronDown, "Move down", 1));
        let (icon, tip, to) = track_move_button(1, 3);
        assert_eq!((icon, tip, to), (Icon::ChevronDown, "Move down", 2));
        // the last track has nowhere to go but up, so the button must point up with ChevronUp
        let (icon, tip, to) = track_move_button(2, 3);
        assert_eq!((icon, tip, to), (Icon::ChevronUp, "Move up", 1));
        let (icon, tip, to) = track_move_button(1, 2);
        assert_eq!((icon, tip, to), (Icon::ChevronUp, "Move up", 0));
    }

    #[test]
    fn picking_the_mode_a_track_already_has_keeps_its_custom_size() {
        use designos_doc::GridTrack;
        assert_eq!(track_after_mode_pick(GridTrack::Flex(2.0), 0), None);
        assert_eq!(track_after_mode_pick(GridTrack::Fixed(80.0), 1), None);
        assert_eq!(track_after_mode_pick(GridTrack::Hug, 2), None);
        // a different mode still applies its preset
        assert_eq!(track_after_mode_pick(GridTrack::Flex(2.0), 1), Some(GridTrack::Fixed(120.0)));
        assert_eq!(track_after_mode_pick(GridTrack::Fixed(80.0), 0), Some(GridTrack::Flex(1.0)));
        assert_eq!(track_after_mode_pick(GridTrack::Fixed(80.0), 2), Some(GridTrack::Hug));
        assert_eq!(track_after_mode_pick(GridTrack::Hug, 0), Some(GridTrack::Flex(1.0)));
    }

    #[test]
    fn color_conversion_roundtrip() {
        let c = Color::rgba(0x12, 0x34, 0x56, 255);
        let back = from_c32(to_c32(c, 1.0));
        assert_eq!(back.to_hex(), c.to_hex());
        let half = to_c32(c, 0.5);
        assert!((half.a() as i32 - 128).abs() <= 1);
    }

    #[test]
    fn sizing_reflects_hug_frames_and_text() {
        let mut n = Node::frame(NodeId(1), designos_doc::Rect::new(0.0, 0.0, 10.0, 10.0));
        n.kind = NodeKind::Frame(FrameProps { auto_layout: Some(AutoLayout { primary_sizing: Sizing::Hug, ..Default::default() }), ..Default::default() });
        assert_eq!(sizing_of(&n, true), Sizing::Hug);
        assert_eq!(sizing_of(&n, false), Sizing::Fixed);
    }

    #[test]
    fn child_margins_hidden_when_zero_and_visible_when_set() {
        let mut n = Node::rectangle(NodeId(2), designos_doc::Rect::new(0.0, 0.0, 40.0, 20.0));
        assert!(!has_child_margins(&n));
        n.layout_child.margin = [4.0, 0.0, 0.0, 0.0];
        assert!(has_child_margins(&n));
    }

    #[test]
    fn grid_frame_sizing_of_and_set_sizing_follow_horizontal_primary_convention() {
        let mut doc = designos_doc::Document::new("test");
        let page = doc.add_page("Page 1");
        let mut app = App::new(doc);
        let gid = app.doc.alloc_id();
        let mut g = Node::frame(gid, designos_doc::Rect::new(0.0, 0.0, 300.0, 200.0));
        g.kind = NodeKind::Frame(FrameProps {
            auto_layout: Some(AutoLayout {
                direction: LayoutDirection::Grid,
                grid_columns: 3,
                primary_sizing: Sizing::Hug,
                counter_sizing: Sizing::Fixed,
                ..Default::default()
            }),
            ..Default::default()
        });
        app.doc.insert(page, g).unwrap();

        let n = app.doc.get(gid).unwrap();
        assert_eq!(sizing_of(n, true), Sizing::Hug, "grid primary_sizing=Hug must report Width=Hug");
        assert_eq!(sizing_of(n, false), Sizing::Fixed, "grid counter_sizing=Fixed must report Height=Fixed");

        set_sizing(&mut app, &[gid], true, Sizing::Fixed);
        set_sizing(&mut app, &[gid], false, Sizing::Hug);
        let n = app.doc.get(gid).unwrap();
        let al = n.kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
        assert_eq!((al.primary_sizing, al.counter_sizing), (Sizing::Fixed, Sizing::Hug));
        assert_eq!((sizing_of(n, true), sizing_of(n, false)), (Sizing::Fixed, Sizing::Hug));
    }

    #[test]
    fn grid_track_rows_fit_the_section_without_overlap_at_the_pointer_floor() {
        let heading = Pane { x0: 0.0, oy: 0.0, y: 0.0, title_end: 0.0 };
        let mut below = Pane { x0: 0.0, oy: 0.0, y: 0.0, title_end: 0.0 };
        row_end(&mut below);
        let (head, row) = (grid_track_row(&heading), grid_track_row(&below));
        let named =
            [("heading label", head.label), ("add", head.add), ("mode", row.mode), ("value", row.value), ("move", row.move_btn), ("delete", row.delete)];
        let (left, right) = (theme::SECTION_CONTENT_X, theme::SECTION_CONTENT_X + theme::SECTION_CONTENT_W);
        for (name, r) in named {
            assert!(r.height() >= 24.0, "{name} is {} px tall", r.height());
            assert!(r.min.x >= left && r.max.x <= right, "{name} {r:?} leaves the section content {left}..{right}");
        }
        for (name, r) in [("add", head.add), ("move", row.move_btn), ("delete", row.delete)] {
            assert!(r.width() >= 24.0, "{name} button is {} px wide", r.width());
        }
        assert!(row.value.width() >= 40.0, "the value field keeps room for a number: {}", row.value.width());
        for (i, (a_name, a)) in named.iter().enumerate() {
            for (b_name, b) in &named[i + 1..] {
                let overlap = a.min.x < b.max.x && b.min.x < a.max.x && a.min.y < b.max.y && b.min.y < a.max.y;
                assert!(!overlap, "{a_name} {a:?} overlaps {b_name} {b:?}");
            }
        }
    }

    /// A `direction` flow frame of 200x100 holding one 30x20 rectangle imported the way `layoutAlign: STRETCH` is
    /// stored: Fill on the counter axis plus `AlignSelf::Stretch`. The child has not been laid out yet.
    fn app_with_imported_stretch_child(direction: LayoutDirection) -> (App, NodeId) {
        let mut doc = designos_doc::Document::new("test");
        let page = doc.add_page("Page 1");
        let mut app = App::new(doc);
        let fid = app.doc.alloc_id();
        let mut frame = Node::frame(fid, designos_doc::Rect::new(0.0, 0.0, 200.0, 100.0));
        let al = AutoLayout { direction, primary_sizing: Sizing::Fixed, counter_sizing: Sizing::Fixed, ..Default::default() };
        frame.kind = NodeKind::Frame(FrameProps { auto_layout: Some(al), ..Default::default() });
        let fid = app.doc.insert(page, frame).unwrap();
        let cid = app.doc.alloc_id();
        let mut child = Node::rectangle(cid, designos_doc::Rect::new(0.0, 0.0, 30.0, 20.0));
        match direction {
            LayoutDirection::Vertical => child.layout_child.horizontal = Sizing::Fill,
            _ => child.layout_child.vertical = Sizing::Fill,
        }
        child.layout_child.align_self = AlignSelf::Stretch;
        let cid = app.doc.insert(fid, child).unwrap();
        (app, cid)
    }

    #[test]
    fn leaving_fill_on_the_counter_axis_ends_an_imported_stretch() {
        // horizontal flow: height is the counter axis
        let (mut app, cid) = app_with_imported_stretch_child(LayoutDirection::Horizontal);
        set_sizing(&mut app, &[cid], false, Sizing::Fixed);
        let n = app.doc.get(cid).unwrap();
        assert_eq!(n.layout_child.align_self, AlignSelf::Auto);
        assert_eq!((n.size.x, n.size.y), (30.0, 20.0), "Fixed stops the stretch");

        // Hug on the counter axis ends it as well
        let (mut app, cid) = app_with_imported_stretch_child(LayoutDirection::Horizontal);
        set_sizing(&mut app, &[cid], false, Sizing::Hug);
        assert_eq!(app.doc.get(cid).unwrap().layout_child.align_self, AlignSelf::Auto);

        // vertical flow: width is the counter axis
        let (mut app, cid) = app_with_imported_stretch_child(LayoutDirection::Vertical);
        set_sizing(&mut app, &[cid], true, Sizing::Fixed);
        let n = app.doc.get(cid).unwrap();
        assert_eq!(n.layout_child.align_self, AlignSelf::Auto);
        assert_eq!((n.size.x, n.size.y), (30.0, 20.0));
    }

    #[test]
    fn sizing_writes_that_keep_the_stretch_axis_leave_the_stretch_alone() {
        // the main axis of a horizontal flow is not stretched, and Fill on the counter axis still is
        let (mut app, cid) = app_with_imported_stretch_child(LayoutDirection::Horizontal);
        set_sizing(&mut app, &[cid], true, Sizing::Fixed);
        set_sizing(&mut app, &[cid], false, Sizing::Fill);
        let n = app.doc.get(cid).unwrap();
        assert_eq!(n.layout_child.align_self, AlignSelf::Stretch);
        assert_eq!(n.size.y, 100.0);
    }

    #[test]
    fn margin_inspector_section_renders_in_running_frame_only_when_margins_are_set() {
        let (mut app, cid) = app_with_imported_stretch_child(LayoutDirection::Horizontal);
        app.selection = vec![cid];
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        crate::theme::install_fonts(&ctx);
        let region = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(240.0, 900.0));
        let raw = egui::RawInput { screen_rect: Some(region), ..Default::default() };

        // With zero margins, the Margin fields are not drawn.
        let _ = ctx.run_ui(raw.clone(), |ui| show(&mut app, ui, region));
        let field_rect = |ctx: &egui::Context, key: &str| ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("field-rect", format!("{key:?}")))));
        assert!(field_rect(&ctx, "margin-l").is_none());

        // With non-zero margins on the child, all four margin fields render in the Layout section.
        app.doc.get_mut(cid).unwrap().layout_child.margin = [4.0, 8.0, 12.0, 16.0];
        let _ = ctx.run_ui(raw, |ui| show(&mut app, ui, region));
        for key in ["margin-l", "margin-r", "margin-t", "margin-b"] {
            assert!(field_rect(&ctx, key).is_some(), "expected {key} field to render in properties panel");
        }
    }

    #[test]
    fn sizing_of_reports_fill_for_a_stretch_plus_fixed_child_on_the_counter_axis() {
        let mut n = Node::rectangle(NodeId(99), designos_doc::Rect::new(0.0, 0.0, 30.0, 20.0));
        n.layout_child.align_self = AlignSelf::Stretch;
        assert_eq!(sizing_of(&n, true), Sizing::Fixed);
        assert_eq!(sizing_of(&n, false), Sizing::Fill);
    }

    #[test]
    fn typing_dimensions_or_apply_resize_on_a_fill_or_stretch_child_ends_the_stretch() {
        let (mut app, cid) = app_with_imported_stretch_child(LayoutDirection::Horizontal);
        let fid = app.doc.get(cid).unwrap().parent.unwrap();
        app.doc.relayout_subtree(fid);
        assert_eq!(app.doc.get(cid).unwrap().size, v2(30.0, 100.0));

        resize_selection(&mut app, &[cid], None, Some(45.0));
        let n = app.doc.get(cid).unwrap();
        assert_eq!(n.layout_child.vertical, Sizing::Fixed);
        assert_eq!(n.layout_child.align_self, AlignSelf::Auto);
        assert_eq!(n.size, v2(30.0, 45.0), "typed height must stick instead of snapping back to 100");
    }

    #[test]
    fn setting_primary_axis_to_fill_on_stretch_plus_fixed_child_keeps_stretch_and_stretches_height() {
        let (mut app, cid) = app_with_imported_stretch_child(LayoutDirection::Horizontal);
        app.doc.get_mut(cid).unwrap().layout_child.vertical = Sizing::Fixed;
        set_sizing(&mut app, &[cid], true, Sizing::Fill);
        let n = app.doc.get(cid).unwrap();
        assert_eq!(n.layout_child.horizontal, Sizing::Fill);
        assert_eq!(n.layout_child.vertical, Sizing::Fixed);
        assert_eq!(n.layout_child.align_self, AlignSelf::Stretch);
        assert_eq!(n.size, v2(200.0, 100.0));

        // Flipping the parent direction via update_al does not double-stretch an imported Stretch+Fill child
        let (mut app2, cid2) = app_with_imported_stretch_child(LayoutDirection::Horizontal);
        let fid2 = app2.doc.get(cid2).unwrap().parent.unwrap();
        update_al(&mut app2, &[fid2], |al| al.direction = LayoutDirection::Vertical);
        let n2 = app2.doc.get(cid2).unwrap();
        assert_eq!(n2.size, v2(30.0, 100.0));
        assert_eq!(n2.layout_child.align_self, AlignSelf::Auto);
    }
}
