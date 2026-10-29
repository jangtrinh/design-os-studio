//! Chrome, size and shadow checks for the assets, variables, libraries and check-designs surfaces.

use super::chrome_census::{chrome_colour_sites, clean_rust, texture_tint_line};
use super::*;
use crate::widgets::kit::{self, State};

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
fn k_surfaces_draw_no_shadow() {
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

#[test]
fn check_designs_contrast_choice_is_segmented() {
    let mut s = Sim::with(Options { fixture: Some(crate::check_designs::FIXTURE.into()), panel: Some("check-designs".into()), ..Options::default() });
    s.shell.app.check_designs.rules_open = true;
    s.frame(vec![]);
    for name in ["check-designs-contrast-aa", "check-designs-contrast-aaa"] {
        let rect = s.rect_of(name);
        assert!((rect.height() - 24.0).abs() < 0.5, "{name} stays 24 high, got {}", rect.height());
    }
    let src = include_str!("../../panels/check_designs.rs");
    assert!(src.contains("segmented_look"), "contrast tiles paint the segmented look");
    let selected = kit::segmented_look(State::Selected);
    assert_eq!(selected.fill, theme::BG_SELECTED);
    assert_eq!(selected.border, theme::BORDER_STRONG);
    assert_eq!(selected.text, theme::TEXT_SECONDARY);
    assert!(!src.contains("BG_BRAND"), "brand fill stays inside the kit painters");
}

#[test]
fn check_designs_uses_contract_roles() {
    let src = include_str!("../../panels/check_designs.rs");
    assert!(!src.contains("crate::terminal"), "rule icons use theme roles");
    let s = Sim::with(Options { fixture: Some(crate::check_designs::FIXTURE.into()), panel: Some("check-designs".into()), ..Options::default() });
    assert_eq!(s.rect_of("check-designs-panel").width(), 320.0);
    assert_eq!(s.rect_of("check-designs-fix-all").height(), theme::CONTROL_XS);
    assert_eq!(s.rect_of("check-designs-close").height(), theme::CONTROL_XS);
    let mut scanning = s;
    scanning.shell.app.check_designs.invalidate();
    scanning.shell.app.check_designs.slice_budget = std::time::Duration::ZERO;
    scanning.frame(vec![]);
    assert_eq!(scanning.rect_of("check-designs-progress-bar").height(), 8.0);
}

#[test]
fn assets_search_has_no_blue_outline() {
    let src = include_str!("../../panels/assets.rs");
    assert!(!src.contains("0x0D, 0x99, 0xFF"), "search still draws the blue outline");
    assert!(src.contains("paint_field"), "search paints through the field painter");
    let look = kit::field_look(State::Default, false);
    assert_eq!(look.border, theme::BORDER_STRONG);
    let s = Sim::with(Options { panel: Some("assets".into()), ..Options::default() });
    assert_eq!(s.rect_of("assets-search").height(), theme::FIELD_HEIGHT);
}

#[test]
fn variables_rows_keep_geometry() {
    assert_eq!(theme::VAR_HEAD_H, 40.0);
    assert_eq!(theme::VAR_ROW_H, 40.0);
    assert_eq!(theme::VAR_ROW_PITCH, 41.0);
    let s = Sim::with(Options { panel: Some("variables".into()), ..Options::default() });
    let coll = s.app().doc.variable_collections.values().find(|c| c.name == "Theme").expect("Theme");
    let light = coll.modes[0].id.clone();
    let field = s.rect_of(&format!("vars-num:Radius/Card:{light}"));
    assert_eq!(field.height(), theme::FIELD_HEIGHT);
}

#[test]
fn variable_picker_rows_are_menu_rows() {
    let src = include_str!("../../variables/widgets.rs");
    assert!(src.contains("MENU_ITEM_HEIGHT"), "picker rows use the menu row height");
    assert!(!src.contains("const PICK_ROW: f32 = 24.0"));
    assert!(!src.contains("const PICK_GROUP: f32 = 26.0"));
}

#[test]
fn libraries_dialog_follows_dialog_sizes() {
    let src = include_str!("../../libraries/modal.rs");
    assert!(src.contains("INSET_MODAL"), "dialog padding is the modal inset");
    assert!(!src.contains("const PAD: f32 = 20.0"));
    assert!(src.contains("BADGE_MD"), "change tags use the badge height");
    assert!(!src.contains("const CHANGE_ROW_H: f32 = 20.0"));
    let s = Sim::with(Options { panel: Some("libraries".into()), ..Options::default() });
    assert_eq!(s.rect_of("lib-search").height(), theme::CONTROL_SM);
    assert_eq!(s.rect_of("lib-publish").height(), theme::CONTROL_SM);
    assert_eq!(s.rect_of("lib-desc").height(), theme::CONTROL_SM);
}

#[test]
fn review_checkbox_uses_dark_tick() {
    let checked = kit::checkbox_look(State::Selected);
    assert_eq!(checked.fill, theme::BG_BRAND);
    assert_eq!(checked.icon, theme::TEXT_ON_BRAND);
    assert_eq!(theme::LAYER_ICON_SIZE, 16.0);
    let src = include_str!("../../libraries/review.rs");
    assert!(!src.contains("Color32::WHITE"));
    assert!(src.contains("CONTROL_SM"), "review rows are the small list row");
    assert!(src.contains("CONTROL_MD"), "review footer buttons are the dialog footer size");
}

fn shape_stroke_is(shape: &egui::Shape, color: egui::Color32) -> bool {
    match shape {
        egui::Shape::Rect(rect) => rect.stroke.width >= 1.5 && rect.stroke.color == color,
        egui::Shape::Vec(children) => children.iter().any(|child| shape_stroke_is(child, color)),
        _ => false,
    }
}

fn mesh_tinted(shape: &egui::Shape, rect: egui::Rect, color: egui::Color32) -> bool {
    match shape {
        egui::Shape::Mesh(mesh) => mesh.vertices.iter().any(|vertex| rect.contains(vertex.pos) && vertex.color == color),
        egui::Shape::Vec(children) => children.iter().any(|child| mesh_tinted(child, rect, color)),
        _ => false,
    }
}

#[test]
fn k_gap_assets_group_chevron() {
    let s = Sim::with(Options { panel: Some("assets".into()), ..Options::default() });
    let label = s.shapes.iter().find_map(|clipped| match &clipped.shape {
        egui::Shape::Text(text) if text.galley.text() == "Components" => Some(text.pos),
        _ => None,
    });
    let label = label.expect("assets page group");
    let icons = crate::lint::take_icons();
    let chevron =
        icons.into_iter().find(|icon| icon.name == "ChevronDown" && (icon.glyph.center().y - label.y).abs() <= 16.0 && icon.glyph.right() <= label.x + 2.0);
    let chevron = chevron.expect("group row paints a chevron");
    assert!(s.shapes.iter().any(|clipped| mesh_tinted(&clipped.shape, chevron.glyph, theme::ICON_QUATERNARY)), "group-row chevron is ICON_QUATERNARY");
}

#[test]
fn k_gap_review_button() {
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

#[test]
fn k_gap_variable_focus_ring() {
    let mut s = Sim::with(Options { panel: Some("variables".into()), ..Options::default() });
    // The variables surface fades in for 0.2s. The ring colour is the token once that fade is done.
    for _ in 0..8 {
        s.frame(vec![]);
    }
    s.ctx.memory_mut(|mem| mem.request_focus(egui::Id::new("vars-search")));
    s.frame(vec![]);
    assert!(s.ctx.memory(|mem| mem.has_focus(egui::Id::new("vars-search"))), "the variables search holds keyboard focus");
    assert!(s.shapes.iter().any(|clipped| shape_stroke_is(&clipped.shape, theme::FOCUS_RING)), "keyboard focus draws FOCUS_RING");
}

#[test]
fn variables_hscrollbar_still_drags() {
    let src = include_str!("../../variables/view.rs");
    let start = src.find("fn hscrollbar(").expect("hscrollbar");
    let rest = &src[start..];
    let end = rest[1..].find("\nfn ").map(|i| i + 1).unwrap_or(rest.len());
    assert!(rest[..end].contains("Sense::click_and_drag()"));
}
