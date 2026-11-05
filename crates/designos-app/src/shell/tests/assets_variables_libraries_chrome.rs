//! Chrome, size, focus and shadow checks for the assets, variables, libraries and check-designs surfaces.
//! Most checks read the shapes a frame painted, so a wrong colour, size or missing ring fails them.

use super::chrome_census::{chrome_colour_sites, clean_rust, texture_tint_line};
use super::*;
use crate::widgets::kit::{self, State};
use egui::epaint::{RectShape, TextShape};
use egui::{Color32, Rect, Shape};
use std::path::PathBuf;

const SOURCES: &[(&str, &str)] = &[
    ("panels/assets.rs", include_str!("../../panels/assets.rs")),
    ("variables/mod.rs", include_str!("../../variables/mod.rs")),
    ("variables/view.rs", include_str!("../../variables/view.rs")),
    ("variables/popover.rs", include_str!("../../variables/popover.rs")),
    ("variables/widgets.rs", include_str!("../../variables/widgets.rs")),
    ("variables/library.rs", include_str!("../../variables/library.rs")),
    ("libraries/mod.rs", include_str!("../../libraries/mod.rs")),
    ("libraries/modal.rs", include_str!("../../libraries/modal.rs")),
    ("libraries/review.rs", include_str!("../../libraries/review.rs")),
    ("styles.rs", include_str!("../../styles.rs")),
    ("plugins/mod.rs", include_str!("../../plugins/mod.rs")),
    ("plugins/ui.rs", include_str!("../../plugins/ui.rs")),
    ("plugins/store.rs", include_str!("../../plugins/store.rs")),
    ("plugins/runtime.rs", include_str!("../../plugins/runtime.rs")),
    ("plugins/svg.rs", include_str!("../../plugins/svg.rs")),
    ("panels/check_designs.rs", include_str!("../../panels/check_designs.rs")),
];

fn live_sites(src: &str) -> Vec<usize> {
    let clean = clean_rust(src);
    let lines: Vec<&str> = clean.split('\n').collect();
    chrome_colour_sites(src).into_iter().filter(|n| lines.get(n - 1).is_none_or(|line| !texture_tint_line(line))).collect()
}

// ---- reading the painted frame

fn shapes_of(s: &Sim) -> Vec<&Shape> {
    fn walk<'a>(shape: &'a Shape, out: &mut Vec<&'a Shape>) {
        match shape {
            Shape::Vec(children) => children.iter().for_each(|child| walk(child, out)),
            other => out.push(other),
        }
    }
    let mut out = Vec::new();
    for clipped in &s.shapes {
        walk(&clipped.shape, &mut out);
    }
    out
}

fn rects_of(s: &Sim) -> Vec<&RectShape> {
    shapes_of(s)
        .into_iter()
        .filter_map(|shape| match shape {
            Shape::Rect(rect) => Some(rect),
            _ => None,
        })
        .collect()
}

fn texts_of(s: &Sim) -> Vec<&TextShape> {
    shapes_of(s)
        .into_iter()
        .filter_map(|shape| match shape {
            Shape::Text(text) => Some(text),
            _ => None,
        })
        .collect()
}

fn near(a: Rect, b: Rect) -> bool {
    a.min.distance(b.min) <= 0.6 && a.max.distance(b.max) <= 0.6
}

/// Fill colours of the rects painted exactly at `at`.
fn fills_at(s: &Sim, at: Rect) -> Vec<Color32> {
    rects_of(s).into_iter().filter(|r| near(r.rect, at) && r.fill != Color32::TRANSPARENT).map(|r| r.fill).collect()
}

/// (width, colour) of the strokes painted exactly at `at`.
fn strokes_at(s: &Sim, at: Rect) -> Vec<(f32, Color32)> {
    rects_of(s)
        .into_iter()
        .filter(|r| near(r.rect, at) && r.stroke.width > 0.0 && r.stroke.color != Color32::TRANSPARENT)
        .map(|r| (r.stroke.width, r.stroke.color))
        .collect()
}

fn text_colour(text: &TextShape) -> Color32 {
    text.galley.job.sections.first().map_or(text.fallback_color, |section| section.format.color)
}

/// Colour of the first text reading `label` whose centre lies inside `area`.
fn text_colour_in(s: &Sim, area: Rect, label: &str) -> Option<Color32> {
    texts_of(s).into_iter().find(|t| t.galley.text() == label && area.contains(Rect::from_min_size(t.pos, t.galley.size()).center())).map(text_colour)
}

fn mesh_has(s: &Sim, inside: Rect, colour: Color32) -> bool {
    shapes_of(s).into_iter().any(|shape| match shape {
        Shape::Mesh(mesh) => mesh.vertices.iter().any(|v| inside.contains(v.pos) && v.color == colour),
        _ => false,
    })
}

fn try_rect(s: &Sim, name: &str) -> Option<Rect> {
    s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", name.to_string()))))
}

/// The keyboard ring around `target`: a 2 px `FOCUS_RING` band from 2 to 4 px outside it.
fn ring_around(s: &Sim, target: Rect) -> bool {
    let want = target.expand(theme::SPACE_FACT);
    rects_of(s).into_iter().any(|r| near(r.rect, want) && r.stroke.color == theme::FOCUS_RING && r.stroke.width >= 1.5)
}

/// Gives keyboard focus to the focusable widget whose hit rect holds `target` most tightly, then reports whether the
/// ring surrounds `target`. (Tab is the canvas's sibling shortcut here, so focus is set directly.)
fn ring_after_focusing(s: &mut Sim, target: Rect) -> bool {
    let id = s.ctx.viewport(|vp| {
        let mut best: Option<(f32, egui::Id)> = None;
        for (_, entries) in vp.prev_pass.widgets.layers() {
            for w in entries {
                if w.enabled && w.sense.is_focusable() && w.sense.senses_click() && w.interact_rect.contains_rect(target) {
                    let area = w.interact_rect.area();
                    if best.is_none_or(|(a, _)| area < a) {
                        best = Some((area, w.id));
                    }
                }
            }
        }
        best.map(|(_, id)| id)
    });
    let Some(id) = id else { return false };
    s.ctx.memory_mut(|m| m.request_focus(id));
    frames(s, 2);
    ring_around(s, target)
}

fn frames(s: &mut Sim, n: usize) {
    for _ in 0..n {
        s.frame(vec![]);
    }
}

/// Surfaces fade in for 0.2 s. Their colours are the tokens once that fade is done.
fn settle(s: &mut Sim) {
    frames(s, 12);
}

fn temp_root(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("designos-k-chrome-{name}-{}", designos_library::new_id()));
    let _ = std::fs::remove_dir_all(&p);
    p
}

fn sim_in(root: &std::path::Path) -> Sim {
    let mut s = Sim::new();
    s.shell.app.libs.set_root(root.to_path_buf());
    s
}

/// Window B has enabled a library that window A published and uses one of its components and one of its variables. Then A
/// changed the component, added a variable to the collection B uses, and published again. B's Libraries dialog is open
/// and offers the update.
fn updated_library_pair(name: &str) -> (Sim, PathBuf) {
    updated_library_pair_named(name, "Untitled")
}

/// [`updated_library_pair`] where the library, which takes its name from the file that publishes it, is called `library`.
fn updated_library_pair_named(name: &str, library: &str) -> (Sim, PathBuf) {
    let root = temp_root(name);
    let mut a = sim_in(&root);
    a.shell.app.doc.name = library.to_string();
    a.shell.app.open_libraries();
    frames(&mut a, 3);
    a.shell.app.libs.description = "First version".into();
    let publish = a.rect_of("lib-publish");
    a.click(publish.center());
    frames(&mut a, 2);

    let mut b = sim_in(&root);
    b.shell.app.new_document();
    b.shell.app.open_libraries();
    frames(&mut b, 3);
    let toggle = b.rect_of(&format!("lib-toggle:{library}"));
    b.click(toggle.center());
    frames(&mut b, 2);
    let lib_id = b.app().doc.library.enabled[0].id.clone();
    let (component, variable) = {
        let snap = b.shell.app.latest_snapshot(&lib_id).expect("the library snapshot");
        let key = |name: &str| snap.assets.iter().find(|x| x.name == name).unwrap_or_else(|| panic!("library asset {name}")).key.clone();
        (key("Primary button"), key("Surface/Card"))
    };
    let instance = b.shell.app.drop_library_asset(&lib_id, &component, designos_doc::Vec2::new(40.0, 40.0)).expect("dropped component");
    b.shell.app.bind_library_variable(&[instance], &["fills/0"], &lib_id, &variable);

    let main = a.by_name("Primary button");
    a.shell.app.edit_nodes(&[main], |n| n.size.x += 30.0);
    let mut store = designos_doc::VariableStore::from_doc(&a.shell.app.doc);
    let collection = a.shell.app.doc.variable_collections.values().find(|c| c.name == "Theme").expect("Theme").id.clone();
    let added = store.add_variable(&collection, "Brand/Added", designos_doc::VariableKind::Number).expect("added variable");
    let mode = store.collections[&collection].default_mode.clone();
    store.set_value(&added, &mode, designos_doc::VariableValue::Number(4.0));
    assert!(a.shell.app.commit_store(store, &[], &[]));
    a.shell.app.refresh_libraries();
    frames(&mut a, 2);
    let publish = a.rect_of("lib-publish");
    a.click(publish.center());
    assert_eq!(a.app().doc.library.published.as_ref().map(|p| p.version), Some(2));
    frames(&mut b, 60);
    (b, root)
}

// ---- census and surfaces

#[test]
fn asset_variable_library_sources_paint_no_chrome_colour_literals() {
    let mut left = Vec::new();
    for (name, src) in SOURCES {
        let sites = live_sites(src);
        if !sites.is_empty() {
            left.push(format!("{name}: {sites:?}"));
        }
    }
    assert!(left.is_empty(), "chrome colour sites remain:\n{}", left.join("\n"));
}

#[test]
fn asset_variable_library_surfaces_draw_no_shadow() {
    for (name, src) in SOURCES {
        let clean = clean_rust(src);
        for needle in ["elevation_400(", "elevation_shadow(", "menu_panel(", "ELEVATION_400_RING"] {
            assert!(!clean.contains(needle), "{name} still contains {needle}");
        }
    }
    assert!(include_str!("../../libraries/modal.rs").contains("modal_surface"));
    assert!(include_str!("../../plugins/ui.rs").contains("modal_surface"));
    assert!(include_str!("../../libraries/review.rs").contains("popover_surface"));
    assert!(include_str!("../../variables/view.rs").contains("popover_surface"));
    assert!(include_str!("../../variables/widgets.rs").contains("popover_surface"));
    assert!(include_str!("../../variables/popover.rs").contains("popover_surface"));
    assert!(include_str!("../../panels/check_designs.rs").contains("popover_surface"));
}

// ---- Assets

#[test]
fn assets_search_has_no_blue_outline() {
    let s = Sim::with(Options { panel: Some("assets".into()), ..Options::default() });
    let field = s.rect_of("assets-search");
    assert_eq!(field.height(), theme::FIELD_HEIGHT);
    let strokes = strokes_at(&s, field);
    assert_eq!(strokes, vec![(theme::BORDER_WIDTH, theme::BORDER_STRONG)], "the field rests on the neutral border and nothing else");
    assert!(!ring_around(&s, field), "no ring while the field is not focused");
    assert!(
        !rects_of(&s).iter().any(|r| r.stroke.color == theme::INFO && r.stroke.width > 0.0 && field.expand(2.0).contains_rect(r.rect)),
        "the field draws no blue stroke"
    );
}

#[test]
fn assets_search_draws_the_focus_ring_when_focused() {
    let mut s = Sim::with(Options { panel: Some("assets".into()), ..Options::default() });
    let field = s.rect_of("assets-search");
    s.click(field.center());
    frames(&mut s, 2);
    assert!(ring_around(&s, field), "the focused field draws the ring");
    assert_eq!(strokes_at(&s, field), vec![(theme::BORDER_WIDTH * 2.0, theme::BORDER_SELECTED)], "the focused field widens its border");
}

#[test]
fn asset_group_rows_draw_no_chevron() {
    let s = Sim::with(Options { panel: Some("assets".into()), ..Options::default() });
    let label =
        texts_of(&s).into_iter().find(|t| t.galley.text() == "Components").map(|t| Rect::from_min_size(t.pos, t.galley.size())).expect("assets page group");
    let icons = crate::lint::take_icons();
    let chevron = icons
        .iter()
        .find(|icon| icon.name == "ChevronDown" && (icon.glyph.center().y - label.center().y).abs() <= 16.0 && icon.glyph.center().x <= label.left() + 24.0);
    assert!(chevron.is_none(), "the group rows do not fold, so they draw no chevron: {chevron:?}");
}

#[test]
fn assets_filter_pills_sit_six_below_the_search_field() {
    let s = Sim::with(Options { panel: Some("assets".into()), ..Options::default() });
    let field = s.rect_of("assets-search");
    let pill = s.rect_of("assets-filter:All");
    assert_eq!(pill.top() - field.bottom(), 6.0);
}

// ---- Variables

#[test]
fn variables_rows_keep_geometry() {
    assert_eq!(theme::VAR_HEAD_H, 40.0);
    assert_eq!(theme::VAR_ROW_H, 40.0);
    assert_eq!(theme::VAR_ROW_PITCH, 41.0);
    let s = Sim::with(Options { panel: Some("variables".into()), ..Options::default() });
    let light = theme_light_mode(&s);
    let field = s.rect_of(&format!("vars-num:Radius/Card:{light}"));
    assert_eq!(field.height(), theme::FIELD_HEIGHT);
}

fn theme_light_mode(s: &Sim) -> String {
    s.app().doc.variable_collections.values().find(|c| c.name == "Theme").expect("Theme").modes[0].id.clone()
}

#[test]
fn variables_value_cells_draw_nothing_at_rest_and_a_field_while_edited() {
    let mut s = Sim::with(Options { panel: Some("variables".into()), ..Options::default() });
    settle(&mut s);
    let light = theme_light_mode(&s);
    let cell = s.rect_of(&format!("vars-num:Radius/Card:{light}"));
    assert!(strokes_at(&s, cell).is_empty(), "no border at rest: {:?}", strokes_at(&s, cell));
    assert!(fills_at(&s, cell).is_empty(), "no fill at rest: {:?}", fills_at(&s, cell));

    s.mv(cell.center());
    assert_eq!(fills_at(&s, cell), vec![theme::BG_SECONDARY_HOVER], "the hover fill shows against the hovered row");
    assert!(strokes_at(&s, cell).is_empty(), "hover draws no border");

    s.click(cell.center());
    frames(&mut s, 2);
    assert_eq!(strokes_at(&s, cell), vec![(theme::BORDER_WIDTH * 2.0, theme::BORDER_SELECTED)], "the focused field look");
    assert!(ring_around(&s, cell), "the focus ring around the edited cell");
}

#[test]
fn variables_search_draws_the_focus_ring_when_focused() {
    let mut s = Sim::with(Options { panel: Some("variables".into()), ..Options::default() });
    // The variables surface fades in for 0.2s. The ring colour is the token once that fade is done.
    frames(&mut s, 8);
    let field = s.rect_of("vars-search");
    assert_eq!(strokes_at(&s, field), vec![(theme::BORDER_WIDTH, theme::BORDER_STRONG)], "the search field rests on the kit border");
    assert!(!ring_around(&s, field));
    s.ctx.memory_mut(|mem| mem.request_focus(egui::Id::new("vars-search")));
    s.frame(vec![]);
    assert!(s.ctx.memory(|mem| mem.has_focus(egui::Id::new("vars-search"))), "the variables search holds keyboard focus");
    assert!(ring_around(&s, field), "keyboard focus draws FOCUS_RING around the field");
    assert_eq!(strokes_at(&s, field), vec![(theme::BORDER_WIDTH * 2.0, theme::BORDER_SELECTED)]);
}

#[test]
fn variables_group_header_hover_is_visible() {
    let mut s = Sim::with(Options { panel: Some("variables".into()), ..Options::default() });
    frames(&mut s, 8);
    let label = s.rect_of("vars-header:Radius");
    let row_fill = |s: &Sim| {
        rects_of(s)
            .into_iter()
            .find(|r| r.rect.width() > 400.0 && r.rect.height() <= 41.5 && r.rect.contains(label.center()) && r.fill != Color32::TRANSPARENT)
            .map(|r| r.fill)
    };
    assert_eq!(row_fill(&s), Some(theme::BG_SECONDARY), "a group row rests on the secondary background");
    s.mv(label.center());
    assert_eq!(row_fill(&s), Some(theme::BG_SECONDARY_HOVER), "the hover fill differs from the resting fill");
}

#[test]
fn variables_create_button_is_centred_in_the_bottom_bar() {
    let s = Sim::with(Options { panel: Some("variables".into()), ..Options::default() });
    let create = s.rect_of("vars-create");
    let chip = s.rect_of("vars-scope:All");
    let panel = s.app().ui.regions.variables_full.expect("variables region");
    assert_eq!(create.height(), theme::FIELD_HEIGHT);
    assert!((create.center().y - chip.center().y).abs() < 0.5, "the button and the scope chips share one centre line: {create:?} vs {chip:?}");
    assert!(panel.bottom() - create.bottom() >= 4.0, "the button clears the window edge: bottom {} vs {}", create.bottom(), panel.bottom());
}

#[test]
fn variables_alias_pills_keep_their_size() {
    let mut s = Sim::with(Options { panel: Some("variables".into()), ..Options::default() });
    let mut store = designos_doc::VariableStore::from_doc(&s.app().doc);
    let theme_coll = s.app().doc.variable_collections.values().find(|c| c.name == "Theme").expect("Theme").clone();
    let light = theme_coll.modes[0].id.clone();
    let radius = s.app().doc.variables.values().find(|v| v.name == "Radius/Card").expect("Radius/Card").id.clone();
    let target = store.add_variable(&theme_coll.id, "Radius/Other", designos_doc::VariableKind::Number).expect("added number variable");
    assert!(store.set_alias(&radius, &light, &target), "Radius/Card may alias the other number variable");
    assert!(s.shell.app.commit_store(store, &[], &[]));
    frames(&mut s, 4);
    let pill = s.rect_of(&format!("vars-alias:Radius/Card:{light}"));
    assert_eq!(pill.height(), theme::BADGE_MD);
}

/// The switch track painted inside a recorded hit band: the band meets the pointer floor, the track keeps 36x20.
fn painted_switch_track(s: &Sim, band: Rect) -> Rect {
    assert!(band.height() >= theme::TARGET_MIN, "the switch hits on {} px", band.height());
    shapes_of(s)
        .into_iter()
        .find_map(|shape| match shape {
            Shape::Rect(r) if band.contains_rect(r.rect) && (r.rect.width() - band.width()).abs() < 0.5 && r.rect.height() < band.height() => Some(r.rect),
            _ => None,
        })
        .expect("a painted track inside the switch band")
}

#[test]
fn variables_boolean_cell_uses_the_kit_toggle() {
    let mut s = Sim::with(Options { panel: Some("variables".into()), ..Options::default() });
    settle(&mut s);
    let light = theme_light_mode(&s);
    let flag = s.app().doc.variables.values().find(|v| v.name == "Flags/Compact").expect("the demo boolean variable").id.clone();
    let track = painted_switch_track(&s, s.rect_of(&format!("vars-bool:Flags/Compact:{light}")));
    assert_eq!((track.width(), track.height()), (36.0, 20.0), "the kit switch track");
    let knob = shapes_of(&s)
        .into_iter()
        .find_map(|shape| match shape {
            Shape::Circle(c) if track.contains(c.center) => Some(c.radius),
            _ => None,
        })
        .expect("a knob inside the track");
    assert_eq!(knob, theme::LAYER_ICON_SIZE / 2.0);
    assert_eq!(track.height() - 2.0 * knob, 2.0 * theme::INSET_SEGMENT, "the knob keeps its inset from the track edge");
    assert!(ring_after_focusing(&mut s, track), "keyboard focus draws the ring around the switch");
    let before = s.app().doc.variables[&flag].values[&light].clone();
    s.key(Key::Space);
    frames(&mut s, 2);
    assert_ne!(s.app().doc.variables[&flag].values[&light], before, "Space flips the switch");
    let click_before = s.app().doc.variables[&flag].values[&light].clone();
    s.click(track.center());
    assert_ne!(s.app().doc.variables[&flag].values[&light], click_before, "a click on the switch flips it too");
}

#[test]
fn variable_picker_rows_are_menu_rows() {
    let mut s = Sim::with(Options { panel: Some("variables".into()), ..Options::default() });
    let colours: Vec<(String, String)> =
        s.app().doc.variables.values().filter(|v| v.kind == designos_doc::VariableKind::Color).map(|v| (v.id.clone(), v.name.clone())).collect();
    assert!(colours.len() >= 3, "the demo file has colour variables to pick from");
    let light = theme_light_mode(&s);
    let anchor = Rect::from_min_size(egui::pos2(700.0, 300.0), egui::Vec2::splat(20.0));
    s.shell.app.vars.alias_picker = Some(crate::variables::AliasPicker { var: colours[0].0.clone(), mode: light, anchor, query: String::new() });
    settle(&mut s);
    let rows: Vec<Rect> = colours.iter().skip(1).filter_map(|(_, name)| try_rect(&s, &format!("pick:{name}"))).collect();
    assert!(rows.len() >= 2, "the picker lists the other colour variables");
    for row in &rows {
        assert_eq!(row.height(), theme::MENU_ITEM_HEIGHT);
    }
}

fn open_edit_popover(s: &mut Sim) {
    let var = s.app().doc.variable_collections.values().flat_map(|c| c.variables.iter()).next().cloned().expect("a variable");
    let anchor = Rect::from_min_size(egui::pos2(700.0, 300.0), egui::Vec2::ZERO);
    s.shell.app.vars.edit = Some(crate::variables::EditPopover { var, anchor, scroll: 0.0 });
    settle(s);
}

#[test]
fn variables_edit_popover_closes_with_a_cross() {
    let mut s = Sim::with(Options { panel: Some("variables".into()), ..Options::default() });
    open_edit_popover(&mut s);
    let close = s.rect_of("var-pop-close");
    let icons = crate::lint::take_icons();
    let names: Vec<&str> = icons.iter().filter(|icon| close.contains_rect(icon.glyph)).map(|icon| icon.name.as_str()).collect();
    assert!(names.contains(&"Close"), "the popover's close button draws the cross: {names:?}");
    assert!(!names.contains(&"Minus"), "no minus on the close button: {names:?}");
}

#[test]
fn variables_popover_checkboxes_use_the_kit_checkbox() {
    let mut s = Sim::with(Options { panel: Some("variables".into()), ..Options::default() });
    open_edit_popover(&mut s);
    let row = s.rect_of("var-pop:hide");
    let side = egui::Vec2::splat(theme::LAYER_ICON_SIZE);
    let boxed = rects_of(&s).into_iter().find(|r| r.rect.size() == side && row.contains_rect(r.rect) && r.stroke.color == theme::BORDER_STRONG);
    let box_rect = boxed.expect("an unchecked 16 px box with the strong border sits in the row").rect;
    assert!(ring_after_focusing(&mut s, box_rect), "keyboard focus draws the ring around the box");
}

#[test]
fn variables_cell_colour_picker_keeps_the_field_outline() {
    let mut s = Sim::with(Options { panel: Some("variables".into()), ..Options::default() });
    let doc = &s.app().doc;
    let (collection, variable) = doc
        .variable_collections
        .values()
        .flat_map(|c| c.variables.iter().map(move |v| (c, v)))
        .find_map(|(c, v)| doc.variables.get(v).filter(|v| v.kind == designos_doc::VariableKind::Color).map(|v| (c.clone(), v.clone())))
        .expect("a colour variable");
    let anchor = Rect::from_min_size(egui::pos2(700.0, 300.0), egui::Vec2::splat(20.0));
    let before = designos_doc::VariableStore::from_doc(doc);
    let color = Color32::from_rgb(0x3b, 0x82, 0xf6);
    s.shell.app.vars.cell_picker = Some(crate::variables::CellPicker { var: variable.id, mode: collection.default_mode, color, anchor, before });
    settle(&mut s);
    let surface = rects_of(&s).into_iter().find(|r| r.rect.width() > 200.0 && r.rect.height() > 200.0 && r.stroke.color == theme::BORDER_MENU).map(|r| r.rect);
    let surface = surface.expect("the picker surface");
    let outlined = rects_of(&s)
        .into_iter()
        .filter(|r| surface.contains_rect(r.rect) && r.rect != surface && r.stroke.color == theme::BORDER_STRONG && r.stroke.width > 0.0)
        .count();
    assert!(outlined >= 1, "the picker's sliders and field keep the strong field outline, which reads as the kit input border");
}

#[test]
fn variables_hscrollbar_still_drags() {
    let mut s = Sim::with(Options { panel: Some("variables".into()), ..Options::default() });
    let collection = s.app().doc.variable_collections.values().find(|c| c.name == "Theme").expect("Theme").id.clone();
    let mut store = designos_doc::VariableStore::from_doc(&s.app().doc);
    for n in 0..10 {
        store.add_mode(&collection, &format!("Mode {n}")).expect("added mode");
    }
    assert!(s.shell.app.commit_store(store, &[], &[]));
    s.shell.app.vars.collection = Some(collection);
    frames(&mut s, 6);
    let track = s.rect_of("vars-hscrollbar");
    assert_eq!(s.app().vars.scroll_x, 0.0, "the table starts at its left edge");
    let y = track.center().y;
    s.mv(egui::pos2(track.left() + 20.0, y));
    s.button(true);
    s.mv(egui::pos2(track.center().x, y));
    let mid = s.app().vars.scroll_x;
    assert!(mid > 0.0, "the thumb follows the pointer while the button is still down: {mid}");
    s.mv(egui::pos2(track.right() - 4.0, y));
    assert!(s.app().vars.scroll_x > mid, "dragging further scrolls further");
    s.button(false);
}

// ---- Libraries

#[test]
fn libraries_this_file_controls_do_not_overlap() {
    let root = temp_root("overlap");
    let mut s = sim_in(&root);
    s.shell.app.open_libraries();
    settle(&mut s);
    let rects: Vec<Rect> = s.app().libs.changes.iter().take(4).filter_map(|c| try_rect(&s, &format!("lib-change:{}", c.name))).collect();
    assert!(rects.len() >= 2, "the file has at least two changes to publish, got {}", rects.len());
    for pair in rects.windows(2) {
        assert!(pair[0].bottom() <= pair[1].top() + 0.5, "change rows do not intersect: {:?} then {:?}", pair[0], pair[1]);
    }
    let desc = s.rect_of("lib-desc");
    let publish = s.rect_of("lib-publish");
    assert!(desc.bottom() + theme::SPACE_INLINE <= publish.top() + 0.5, "8 px between the description and the buttons: {desc:?} / {publish:?}");
    assert_eq!(rects[0].height(), theme::BADGE_MD);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn libraries_dialog_follows_dialog_sizes() {
    let root = temp_root("sizes");
    let mut s = sim_in(&root);
    s.shell.app.open_libraries();
    settle(&mut s);
    let desc = s.rect_of("lib-desc");
    assert_eq!(desc.width(), 560.0 - 2.0 * theme::INSET_MODAL, "padding is the modal inset");
    assert_eq!(desc.height(), theme::CONTROL_SM);
    assert_eq!(s.rect_of("lib-search").height(), theme::CONTROL_SM);
    assert_eq!(s.rect_of("lib-publish").height(), theme::CONTROL_SM);
    assert_eq!(s.rect_of("libraries-close").height(), theme::CONTROL_SM);
    let first = s.app().libs.changes.first().map(|c| c.name.clone()).expect("a change row");
    assert_eq!(s.rect_of(&format!("lib-change:{first}")).height(), theme::BADGE_MD);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn libraries_search_sits_centred_on_its_label_with_the_list_below() {
    let (mut b, root) = updated_library_pair("search");
    settle(&mut b);
    let search = b.rect_of("lib-search");
    let label = texts_of(&b)
        .into_iter()
        .filter(|t| t.galley.text() == "Libraries" && t.pos.x > 530.0 && t.pos.x < 560.0)
        .map(|t| Rect::from_min_size(t.pos, t.galley.size()))
        .min_by(|x, y| (x.center().y - search.center().y).abs().total_cmp(&(y.center().y - search.center().y).abs()))
        .expect("the Libraries label");
    assert!((label.center().y - search.center().y).abs() <= 1.0, "label centre {} vs field centre {}", label.center().y, search.center().y);
    let row = b.rect_of("lib-row:Untitled");
    assert_eq!(row.top() - search.bottom(), theme::SPACE_INLINE, "the list starts 8 px below the search field");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn libraries_toggle_uses_the_kit_toggle() {
    let (mut b, root) = updated_library_pair("toggle");
    settle(&mut b);
    let track = painted_switch_track(&b, b.rect_of("lib-toggle:Untitled"));
    assert_eq!((track.width(), track.height()), (36.0, 20.0), "the kit switch track");
    let knob = shapes_of(&b)
        .into_iter()
        .find_map(|shape| match shape {
            Shape::Circle(c) if track.contains(c.center) => Some(c.radius),
            _ => None,
        })
        .expect("a knob inside the track");
    assert_eq!(track.height() - 2.0 * knob, 2.0 * theme::INSET_SEGMENT, "the knob keeps its inset from the track edge");
    assert!(ring_after_focusing(&mut b, track), "keyboard focus draws the ring around the switch");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn libraries_update_button_fits_its_label() {
    let (mut b, root) = updated_library_pair("update-button");
    settle(&mut b);
    let button = b.rect_of("lib-update:Untitled");
    assert_eq!(button.height(), theme::CONTROL_SM);
    let font = theme::font_text(theme::FONT_SIZE_TEXT_SM, theme::TextWeight::Semibold);
    let label_w = crate::widgets::text_width(&b.ctx.layer_painter(egui::LayerId::background()), "Update available", &font);
    assert!(button.width() >= label_w + 2.0 * theme::INSET_BUTTON - 0.5, "the button is {} wide for a {label_w} label", button.width());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn library_names_stop_short_of_the_update_button() {
    let long = "Design system with a very long library name that cannot fit on one row beside its controls";
    let (mut b, root) = updated_library_pair_named("long-name", long);
    settle(&mut b);
    let button = b.rect_of(&format!("lib-update:{long}"));
    let row = b.rect_of(&format!("lib-row:{long}"));
    let beside: Vec<Rect> = texts_of(&b)
        .into_iter()
        .map(|t| Rect::from_min_size(t.pos, t.galley.size()))
        .filter(|r| row.contains(r.center()) && !button.expand(2.0).contains(r.center()))
        .collect();
    assert!(beside.len() >= 2, "the row paints a name and a counts line, got {}", beside.len());
    for text in &beside {
        assert!(text.right() <= button.left(), "a text in the row ends at {} under the button that starts at {}", text.right(), button.left());
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn libraries_fields_draw_the_focus_ring_when_focused() {
    let root = temp_root("fields");
    let mut s = sim_in(&root);
    s.shell.app.open_libraries();
    settle(&mut s);
    for key in ["lib-search", "lib-desc"] {
        let field = s.rect_of(key);
        assert_eq!(strokes_at(&s, field), vec![(theme::BORDER_WIDTH, theme::BORDER_STRONG)], "{key} rests on the kit border");
        assert!(!ring_around(&s, field), "{key} has no ring at rest");
        s.ctx.memory_mut(|mem| mem.request_focus(egui::Id::new(key)));
        s.frame(vec![]);
        assert!(ring_around(&s, field), "{key} draws the ring while focused");
        assert_eq!(strokes_at(&s, field), vec![(theme::BORDER_WIDTH * 2.0, theme::BORDER_SELECTED)], "{key} widens the border");
        s.ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::new(key)));
        s.frame(vec![]);
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn plugin_url_field_draws_the_focus_ring_when_focused() {
    let mut s = Sim::with(Options { panel: Some("plugin-install".into()), ..Options::default() });
    settle(&mut s);
    let field = s.rect_of("plugin-url");
    s.ctx.memory_mut(|mem| mem.request_focus(egui::Id::new("plugin-url")));
    s.frame(vec![]);
    assert!(ring_around(&s, field), "the URL field draws the ring while focused");
    assert_eq!(strokes_at(&s, field), vec![(theme::BORDER_WIDTH * 2.0, theme::BORDER_SELECTED)]);
}

// ---- Library review

fn open_review(b: &mut Sim) {
    b.shell.app.close_libraries();
    b.shell.app.libs.invalidate();
    b.shell.app.open_updates_review();
    settle(b);
}

#[test]
fn review_checkbox_uses_dark_tick() {
    let (mut b, root) = updated_library_pair("review-checkbox");
    open_review(&mut b);
    let row = b.rect_of("lib-upd:Primary button");
    assert_eq!(row.height(), theme::CONTROL_SM);
    assert_eq!(b.rect_of("library-update-selected").height(), theme::CONTROL_MD);
    assert_eq!(b.rect_of("library-update-all").height(), theme::CONTROL_MD);

    let check = b.rect_of("lib-check:Primary button");
    let side = egui::Vec2::splat(theme::LAYER_ICON_SIZE);
    let box_of =
        |s: &Sim| rects_of(s).into_iter().find(|r| r.rect.size() == side && check.expand(4.0).contains_rect(r.rect)).map(|r| (r.rect, r.fill, r.stroke.color));
    let (unchecked, fill, stroke) = box_of(&b).expect("the unchecked box");
    assert!(fill == Color32::TRANSPARENT && stroke == theme::BORDER_STRONG, "unchecked: the strong border, no fill");
    assert!(ring_after_focusing(&mut b, unchecked), "keyboard focus draws the ring around the box");

    b.click(check.center());
    frames(&mut b, 2);
    let (checked, fill, _) = box_of(&b).expect("the checked box");
    assert_eq!(fill, theme::BG_BRAND, "checked: the brand fill");
    assert!(mesh_has(&b, checked, theme::TEXT_ON_BRAND), "the tick is the dark label colour on brand");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn review_change_labels_are_status_chips() {
    let (mut b, root) = updated_library_pair("review-chips");
    open_review(&mut b);
    let updates: Vec<designos_doc::Update> = b.app().libs.pending_all().iter().flat_map(|g| g.updates.iter().cloned()).collect();
    assert!(updates.iter().any(|u| u.change == designos_doc::UpdateKind::Changed), "a changed asset is listed");
    assert!(updates.iter().any(|u| u.change == designos_doc::UpdateKind::Added), "an added asset is listed");
    for u in &updates {
        let row = b.rect_of(&format!("lib-upd:{}", u.name));
        let (label, fill, text) = match u.change {
            designos_doc::UpdateKind::Changed => ("Changed", theme::BG_WARNING_PRIMARY, theme::BADGE_WARNING_TEXT),
            designos_doc::UpdateKind::Added => ("New", theme::BG_SUCCESS_PRIMARY, theme::BADGE_SUCCESS_TEXT),
            designos_doc::UpdateKind::Removed => ("Missing from library", theme::BG_ERROR_PRIMARY, theme::BADGE_ERROR_TEXT),
        };
        let chip = rects_of(&b).into_iter().find(|r| r.fill == fill && row.contains_rect(r.rect)).map(|r| r.rect);
        let chip = chip.unwrap_or_else(|| panic!("{} paints a {label} chip with the status fill", u.name));
        assert_eq!(chip.height(), theme::BADGE_MD);
        assert_eq!(text_colour_in(&b, chip, label), Some(text), "{label} reads in the status text colour");
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn library_review_banner_uses_the_kit_button() {
    let src = include_str!("../../libraries/review.rs");
    let start = src.find("pub fn show_banner").expect("banner");
    let end = src[start..].find("\nfn checkbox").expect("checkbox follows the banner");
    let banner = &src[start..start + end];
    assert!(banner.contains("kit::button"), "the review control goes through the kit button");
    let unqualified = banner.match_indices("button(").any(|(at, _)| {
        let prev = banner.as_bytes()[..at].last().copied().unwrap_or(b' ');
        !(prev.is_ascii_alphanumeric() || prev == b'_' || prev == b':')
    });
    assert!(!unqualified, "the review control does not call the unqualified button");
}

// ---- Check designs

fn check_designs_sim() -> Sim {
    Sim::with(Options { fixture: Some(crate::check_designs::FIXTURE.into()), panel: Some("check-designs".into()), ..Options::default() })
}

#[test]
fn check_designs_contrast_choice_is_segmented() {
    let mut s = check_designs_sim();
    s.shell.app.check_designs.rules_open = true;
    settle(&mut s);
    let level = s.app().doc.check_designs.contrast_level;
    let tiles = [("check-designs-contrast-aa", "AA", designos_doc::ContrastLevel::AA), ("check-designs-contrast-aaa", "AAA", designos_doc::ContrastLevel::AAA)];
    for (name, label, tile_level) in tiles {
        let rect = s.rect_of(name);
        assert!((rect.height() - 24.0).abs() < 0.5, "{name} stays 24 high, got {}", rect.height());
        let want = kit::segmented_look(if tile_level == level { State::Selected } else { State::Default });
        let fills = fills_at(&s, rect);
        let strokes = strokes_at(&s, rect);
        if tile_level == level {
            assert_eq!(fills, vec![theme::BG_SELECTED], "{name} is selected: the selected fill");
            assert_eq!(strokes, vec![(theme::BORDER_WIDTH, theme::BORDER_STRONG)], "{name} is selected: the strong border");
        } else {
            assert!(fills.is_empty() && strokes.is_empty(), "{name} rests with no fill and no border: {fills:?} {strokes:?}");
        }
        assert_eq!(text_colour_in(&s, rect, label), Some(want.text), "{name} label colour");
    }
    let tile_rects: Vec<Rect> = tiles.iter().map(|(n, ..)| s.rect_of(n)).collect();
    assert!(!rects_of(&s).iter().any(|r| r.fill == theme::BG_BRAND && tile_rects.iter().any(|t| r.rect.intersects(*t))), "no brand fill on the contrast tiles");
    let aaa = s.rect_of("check-designs-contrast-aaa");
    assert!(ring_after_focusing(&mut s, aaa), "keyboard focus draws the ring around a contrast tile");
}

#[test]
fn check_designs_rules_checkboxes_use_the_kit_checkbox() {
    let mut s = check_designs_sim();
    s.shell.app.check_designs.rules_open = true;
    settle(&mut s);
    let row = s.rect_of(&format!("check-designs-rule-{}", crate::check_designs::Rule::HardCodedValue.figma_id()));
    let side = egui::Vec2::splat(theme::LAYER_ICON_SIZE);
    let boxed = rects_of(&s).into_iter().find(|r| r.rect.size() == side && row.contains_rect(r.rect) && r.fill == theme::BG_BRAND);
    let boxed = boxed.expect("an enabled rule paints a brand box").rect;
    assert!(mesh_has(&s, boxed, theme::TEXT_ON_BRAND), "the tick is the dark label colour");
    assert!(ring_after_focusing(&mut s, boxed), "keyboard focus draws the ring around the box");
}

#[test]
fn check_designs_uses_contract_roles() {
    let src = include_str!("../../panels/check_designs.rs");
    assert!(!src.contains("crate::terminal"), "rule icons use theme roles");
    let mut s = check_designs_sim();
    settle(&mut s);
    let panel = s.rect_of("check-designs-panel");
    assert_eq!(panel.width(), 320.0);
    assert_eq!(s.rect_of("check-designs-fix-all").height(), theme::CONTROL_XS);
    assert_eq!(s.rect_of("check-designs-close").height(), theme::CONTROL_XS);

    // rule icons take the status roles
    let icons = crate::lint::take_icons();
    let mut seen = std::collections::BTreeSet::new();
    for (name, want) in [("Variables", theme::WARNING), ("Text", theme::WARNING), ("Eye", theme::DANGER), ("Component", theme::INFO)] {
        for icon in icons.iter().filter(|i| i.name == name && (14.0..=14.5).contains(&i.glyph.width()) && panel.contains_rect(i.glyph)) {
            assert!(mesh_has(&s, icon.glyph, want), "the {name} rule icon is drawn in its status colour");
            seen.insert(name);
        }
    }
    assert!(seen.len() >= 2, "at least two rule icon kinds were checked against their status role, got {seen:?}");

    // group counts are gray badges
    let badge = rects_of(&s)
        .into_iter()
        .find(|r| r.fill == theme::BADGE_NEUTRAL_BG && r.rect.height() == theme::BADGE_SM)
        .map(|r| r.rect)
        .expect("a group count badge");
    assert_eq!(strokes_at(&s, badge), vec![(theme::BORDER_WIDTH, theme::BADGE_NEUTRAL_BORDER)]);

    let mut scanning = s;
    scanning.shell.app.check_designs.invalidate();
    scanning.shell.app.check_designs.slice_budget = std::time::Duration::ZERO;
    scanning.frame(vec![]);
    assert_eq!(scanning.rect_of("check-designs-progress-bar").height(), 8.0);
}

#[test]
fn libraries_description_hint_sits_inside_the_field() {
    let root = temp_root("desc-hint");
    let mut s = sim_in(&root);
    s.shell.app.open_libraries();
    settle(&mut s);
    let field = s.rect_of("lib-desc");
    let hint = texts_of(&s)
        .into_iter()
        .find(|t| t.galley.job.text.starts_with("Describe what changed") && field.contains(t.pos + egui::vec2(1.0, 1.0)))
        .expect("the description hint is painted in the field");
    assert!(hint.pos.x >= field.left() + theme::INSET_CONTROL - 0.5, "the hint starts {} px from the field edge", hint.pos.x - field.left());
    let _ = std::fs::remove_dir_all(root);
}
