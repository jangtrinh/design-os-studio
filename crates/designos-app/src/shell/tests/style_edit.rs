//! "Edit style": change the values of a shared style from the style picker and see every linked
//! layer follow, as one undo step.

use super::*;
use crate::panels::style_picker;
use designos_doc::{Effect, ExternalAsset, LayoutGrid, LibraryAssetKind, Style, StyleProperty};

fn shadow(radius: f32) -> Effect {
    Effect::DropShadow {
        color: Color::rgba(0, 0, 0, 64),
        offset: DV::new(0.0, 4.0),
        radius,
        spread: 0.0,
        visible: true,
        show_behind: false,
        blend: Default::default(),
    }
}

fn columns(count: u32) -> LayoutGrid {
    LayoutGrid::columns(count, 20.0, 0.0, Color::rgba(255, 0, 0, 26))
}

/// Rect of a numeric field whose key is not a plain string (the key's `Debug` text).
fn raw_field_rect(s: &Sim, key_debug: &str) -> Rect {
    s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("field-rect", key_debug.to_string())))).unwrap_or_else(|| panic!("no field {key_debug}"))
}

fn type_into(s: &mut Sim, r: Rect, text: &str) {
    s.click(Pos2::new(r.left() + 40.0, r.center().y));
    s.type_text(text);
    s.key(Key::Enter);
}

/// Hovers the local-styles row of `key` and clicks its "Edit style" button.
fn edit_from_list(s: &mut Sim, key: &str) {
    s.scroll_inspector_down();
    let row = s.rect_of(&format!("style:{key}"));
    s.mv(row.center());
    s.frame(vec![]);
    let button = s.rect_of(&format!("style-edit:{key}"));
    s.click(button.center());
    s.frame(vec![]);
}

fn fills_of(s: &Sim, id: NodeId) -> Vec<Paint> {
    s.app().doc.get(id).unwrap().fills.clone()
}

#[test]
fn editing_a_color_style_from_the_list_updates_every_linked_layer_in_one_undo_step() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let (hero, card) = (s.by_name("Hero"), s.by_name("Card"));
    s.shell.app.apply_style(&[hero, card], "Brand/Primary");
    let before = fills_of(&s, hero);
    edit_from_list(&mut s, "Brand/Primary");
    assert_eq!(s.app().props.style_edit.as_ref().map(|e| e.key.as_str()), Some("Brand/Primary"));
    let field = s.field_rect("se-fill-opacity");
    type_into(&mut s, field, "50");
    for id in [hero, card] {
        assert!((fills_of(&s, id)[0].opacity - 0.5).abs() < 1e-4, "linked layer {id:?} follows the style");
    }
    assert!(matches!(&s.app().doc.styles["Brand/Primary"], Style::Fill(p) if (p[0].opacity - 0.5).abs() < 1e-4));
    s.cmd_key(Key::Z);
    assert_eq!(fills_of(&s, hero), before, "one undo takes the style and all of its layers back");
    assert_eq!(fills_of(&s, card), before);
    assert!(matches!(&s.app().doc.styles["Brand/Primary"], Style::Fill(p) if (p[0].opacity - 1.0).abs() < 1e-4));
    assert!(s.app().doc.get(hero).unwrap().style_refs.contains_key("fill"), "the link survives the edit and its undo");
}

#[test]
fn editing_a_text_style_from_the_list_restyles_every_linked_text_layer() {
    let mut s = Sim::with(Options { select: Some("Welcome back".into()), ..Default::default() });
    let t = s.by_name("Welcome back");
    s.shell.app.apply_style(&[t], "Body/Regular");
    let size_of = |s: &Sim| match &s.app().doc.get(t).unwrap().kind {
        NodeKind::Text(tp) => tp.style.font_size,
        k => panic!("{k:?}"),
    };
    assert_eq!(size_of(&s), 16.0);
    edit_from_list(&mut s, "Body/Regular");
    let field = s.field_rect("se-size");
    type_into(&mut s, field, "30");
    assert_eq!(size_of(&s), 30.0);
    assert!(matches!(&s.app().doc.styles["Body/Regular"], Style::Text(ts) if ts.font_size == 30.0));
    assert_eq!(s.app().doc.get(t).unwrap().style_refs.get("text").map(String::as_str), Some("Body/Regular"));
    s.cmd_key(Key::Z);
    assert_eq!(size_of(&s), 16.0);
    assert!(matches!(&s.app().doc.styles["Body/Regular"], Style::Text(ts) if ts.font_size == 16.0));
}

/// "Welcome" with a drop shadow and 12 column guides, both turned into styles, plus "Card" linked
/// to the effect style.
fn welcome_with_styles() -> (Sim, NodeId, NodeId) {
    let mut s = Sim::with(Options { select: Some("Welcome".into()), ..Default::default() });
    let (welcome, card) = (s.by_name("Welcome"), s.by_name("Card"));
    s.shell.app.edit_nodes(&[welcome], |n| {
        n.effects = vec![shadow(8.0)];
        if let Some(f) = n.kind.frame_props_mut() {
            f.grids = vec![columns(12)];
        }
    });
    assert!(s.shell.app.create_style_from(welcome, StyleProperty::Effect).is_some());
    assert!(s.shell.app.create_style_from(welcome, StyleProperty::Grid).is_some());
    s.shell.app.apply_style(&[card], "Effect style 1");
    s.scroll_inspector_down();
    (s, welcome, card)
}

#[test]
fn editing_an_effect_style_from_its_linked_row_updates_every_linked_layer_in_one_undo_step() {
    let (mut s, welcome, card) = welcome_with_styles();
    let button = s.rect_of("effect-style-edit");
    s.click(button.center());
    s.frame(vec![]);
    assert_eq!(s.app().props.style_edit.as_ref().map(|e| e.key.as_str()), Some("Effect style 1"));
    // Rows of a drop shadow: Name, X, Y, Blur, ...
    let blur = raw_field_rect(&s, "(\"fxp-b\", 0)");
    type_into(&mut s, blur, "20");
    for id in [welcome, card] {
        assert_eq!(s.app().doc.get(id).unwrap().effects, vec![shadow(20.0)], "linked layer {id:?} follows the style");
    }
    assert!(matches!(&s.app().doc.styles["Effect style 1"], Style::Effect(fx) if fx == &vec![shadow(20.0)]));
    s.cmd_key(Key::Z);
    for id in [welcome, card] {
        assert_eq!(s.app().doc.get(id).unwrap().effects, vec![shadow(8.0)]);
    }
    assert!(matches!(&s.app().doc.styles["Effect style 1"], Style::Effect(fx) if fx == &vec![shadow(8.0)]));
}

#[test]
fn editing_a_grid_style_from_the_style_menu_updates_the_linked_frame() {
    let (mut s, welcome, _card) = welcome_with_styles();
    // The menu rows: the style, then Create, Rename, Detach, Delete and Edit style below the separator.
    let r = s.rect_of("grid-style-button");
    s.click(r.center());
    let top = r.bottom() + 4.0 + theme::MENU_PADDING + 5.0 * theme::MENU_ITEM_HEIGHT + crate::widgets::menu::SEP_H;
    s.click(Pos2::new(r.right() - 60.0, top + theme::MENU_ITEM_HEIGHT / 2.0));
    assert_eq!(s.app().props.style_edit.as_ref().map(|e| e.key.as_str()), Some("Grid style 1"));
    s.frame(vec![]);
    let field = s.field_rect("se-grid-count");
    type_into(&mut s, field, "4");
    let grids = s.app().doc.get(welcome).unwrap().kind.frame_props().unwrap().grids.clone();
    assert_eq!(grids, vec![columns(4)]);
    assert!(matches!(&s.app().doc.styles["Grid style 1"], Style::Grid(g) if g == &vec![columns(4)]));
    s.cmd_key(Key::Z);
    let grids = s.app().doc.get(welcome).unwrap().kind.frame_props().unwrap().grids.clone();
    assert_eq!(grids, vec![columns(12)]);
}

#[test]
fn a_grid_style_count_field_never_leaves_a_guide_that_draws_nothing() {
    use crate::canvas::grids::stripes_aligned;
    let (mut s, welcome, _card) = welcome_with_styles();
    let r = s.rect_of("grid-style-button");
    s.click(r.center());
    let top = r.bottom() + 4.0 + theme::MENU_PADDING + 5.0 * theme::MENU_ITEM_HEIGHT + crate::widgets::menu::SEP_H;
    s.click(Pos2::new(r.right() - 60.0, top + theme::MENU_ITEM_HEIGHT / 2.0));
    s.frame(vec![]);
    let field = s.field_rect("se-grid-count");
    // 0 means "auto count", which needs a column width; this popover has no width field
    type_into(&mut s, field, "0");
    let grids = s.app().doc.get(welcome).unwrap().kind.frame_props().unwrap().grids.clone();
    let [LayoutGrid::Columns { count, gutter, offset, section_size, alignment, .. }] = grids.as_slice() else { panic!("{grids:?}") };
    let drawn = stripes_aligned(*count, *gutter, *offset, *section_size, *alignment, 1440.0);
    assert!(!drawn.is_empty(), "count {count}, section size {section_size}: the guide disappeared from a 1440px frame");
}

#[test]
fn a_drag_is_recorded_once_when_the_pointer_is_released() {
    let (mut s, welcome, card) = welcome_with_styles();
    let button = s.rect_of("effect-style-edit");
    s.click(button.center());
    s.frame(vec![]);
    let blur = raw_field_rect(&s, "(\"fxp-b\", 0)");
    let start = Pos2::new(blur.left() + 8.0, blur.center().y);
    s.mv(start);
    s.frame(vec![]);
    assert_eq!(s.cursor, egui::CursorIcon::ResizeHorizontal, "prefix label shows scrub cursor");
    s.button(true);
    for dx in [2.0, 6.0, 10.0] {
        s.mv(start + Vec2::new(dx, 0.0));
        assert_eq!(s.app().doc.get(card).unwrap().effects, vec![shadow(8.0)], "nothing is recorded while the pointer is down");
    }
    s.button(false);
    s.frame(vec![]);
    for id in [welcome, card] {
        assert_eq!(s.app().doc.get(id).unwrap().effects, vec![shadow(18.0)]);
    }
    s.cmd_key(Key::Z);
    for id in [welcome, card] {
        assert_eq!(s.app().doc.get(id).unwrap().effects, vec![shadow(8.0)], "one undo takes the whole drag back");
    }
}

#[test]
fn scrubbing_text_and_grid_style_fields_updates_linked_layers_in_one_undo_step() {
    let mut s = Sim::with(Options { select: Some("Welcome back".into()), ..Default::default() });
    let t = s.by_name("Welcome back");
    s.shell.app.apply_style(&[t], "Body/Regular");
    edit_from_list(&mut s, "Body/Regular");
    let size_field = s.field_rect("se-size");
    let a = Pos2::new(size_field.left() + 8.0, size_field.center().y);
    s.drag(a, a + Vec2::new(8.0, 0.0));
    let font_size = match &s.app().doc.get(t).unwrap().kind {
        NodeKind::Text(tp) => tp.style.font_size,
        k => panic!("{k:?}"),
    };
    assert_eq!(font_size, 24.0);
    s.cmd_key(Key::Z);
    let font_size = match &s.app().doc.get(t).unwrap().kind {
        NodeKind::Text(tp) => tp.style.font_size,
        k => panic!("{k:?}"),
    };
    assert_eq!(font_size, 16.0);

    let (mut s, welcome, _card) = welcome_with_styles();
    let r = s.rect_of("grid-style-button");
    s.click(r.center());
    let top = r.bottom() + 4.0 + theme::MENU_PADDING + 5.0 * theme::MENU_ITEM_HEIGHT + crate::widgets::menu::SEP_H;
    s.click(Pos2::new(r.right() - 60.0, top + theme::MENU_ITEM_HEIGHT / 2.0));
    s.frame(vec![]);
    let count_field = s.field_rect("se-grid-count");
    let c0 = Pos2::new(count_field.left() + 8.0, count_field.center().y);
    s.drag(c0, c0 + Vec2::new(-4.0, 0.0));
    let grids = s.app().doc.get(welcome).unwrap().kind.frame_props().unwrap().grids.clone();
    assert_eq!(grids, vec![columns(8)]);
    s.cmd_key(Key::Z);
    let grids = s.app().doc.get(welcome).unwrap().kind.frame_props().unwrap().grids.clone();
    assert_eq!(grids, vec![columns(12)]);
}

/// Marks style `key` as the local copy of a style published by a shared library.
fn make_library_style(s: &mut Sim, key: &str) {
    s.shell.app.doc.library.external.insert(
        format!("L1/{key}"),
        ExternalAsset { library_id: "L1".into(), version: 1, kind: LibraryAssetKind::Style, node: None, local_key: Some(key.into()) },
    );
}

#[test]
fn a_library_style_cannot_be_edited_from_the_list() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let hero = s.by_name("Hero");
    s.shell.app.apply_style(&[hero], "Brand/Primary");
    make_library_style(&mut s, "Brand/Primary");
    let before = s.app().doc.styles["Brand/Primary"].clone();
    edit_from_list(&mut s, "Brand/Primary");
    assert!(s.app().props.style_edit.is_none(), "the popover stays closed for a library style");
    assert_eq!(s.app().doc.styles["Brand/Primary"], before);
}

#[test]
fn a_library_style_cannot_be_edited_from_its_linked_row_or_the_style_menu() {
    let (mut s, ..) = welcome_with_styles();
    let sel = s.app().selection.clone();
    let props = [(StyleProperty::Effect, "Effect style 1"), (StyleProperty::Grid, "Grid style 1")];
    // A local style: the menu row reads "Edit style" and can be picked.
    for (prop, _) in props {
        let row = style_picker::edit_style_row(s.app(), &sel, prop);
        assert_eq!((row.label.as_str(), row.disabled), ("Edit style", false), "{prop:?}: a local style can be edited from the menu");
    }
    for (_, key) in props {
        make_library_style(&mut s, key);
    }
    // A library style: the same row keeps its label, is disabled, and gives its reason on hover.
    for (prop, _) in props {
        let row = style_picker::edit_style_row(s.app(), &sel, prop);
        assert!(row.disabled, "{prop:?}: the menu row is disabled for a library style");
        assert_eq!(row.label, "Edit style", "{prop:?}: the Edit style row keeps its normal label");
        assert_eq!(row.tooltip, "Edit in the library file", "{prop:?}: the Edit style row gives its reason on hover");
    }
    // The linked row's edit button opens nothing either.
    let button = s.rect_of("effect-style-edit");
    s.click(button.center());
    s.frame(vec![]);
    assert!(s.app().props.style_edit.is_none(), "the linked row's edit button does nothing");

    // In the colour-style menu (properties.rs), a linked library fill style keeps the normal
    // "Edit style" label (disabled) and shows "Edit in the library file" only as a hover tooltip.
    fn collect_texts(shape: &egui::epaint::Shape, out: &mut Vec<(String, Rect)>) {
        match shape {
            egui::epaint::Shape::Text(t) => out.push((t.galley.job.text.clone(), t.visual_bounding_rect())),
            egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| collect_texts(s, out)),
            _ => {}
        }
    }
    let painted_with_rects = |s: &Sim| -> Vec<(String, Rect)> {
        let mut out = Vec::new();
        for cs in &s.shapes {
            collect_texts(&cs.shape, &mut out);
        }
        out
    };
    let hero = s.by_name("Hero");
    s.shell.app.selection = vec![hero];
    s.shell.app.apply_style(&[hero], "Brand/Primary");
    make_library_style(&mut s, "Brand/Primary");
    s.shell.app.props.scroll = 0.0;
    s.frame(vec![]);
    s.frame(vec![]);
    s.shell.app.props.styles_menu = Some(false);
    s.mv(Pos2::new(10.0, 10.0));
    s.frame(vec![]);
    let items = painted_with_rects(&s);
    let edit_rect = items
        .iter()
        .find(|(t, _)| t == "Edit style")
        .map(|(_, r)| *r)
        .unwrap_or_else(|| panic!("colour-style menu should paint 'Edit style' label, got {:?}", items.iter().map(|(t, _)| t).collect::<Vec<_>>()));
    assert!(
        !items.iter().any(|(t, _)| t == "Edit in the library file"),
        "colour-style menu must not swap the row label to 'Edit in the library file' when not hovered"
    );
    let pt = edit_rect.center();
    s.mv(pt + Vec2::new(0.0, 1.0));
    s.mv(pt);
    for _ in 0..30 {
        s.frame(vec![]);
    }
    let hovered_items = painted_with_rects(&s);
    assert!(
        hovered_items.iter().any(|(t, _)| t == "Edit in the library file"),
        "hovering disabled 'Edit style' in colour-style menu should paint 'Edit in the library file'"
    );
}

#[test]
fn a_library_style_cannot_be_renamed_from_the_style_menu_or_linked_row() {
    let (mut s, ..) = welcome_with_styles();
    let sel = s.app().selection.clone();
    let props = [(StyleProperty::Effect, "Effect style 1"), (StyleProperty::Grid, "Grid style 1")];
    for (prop, _) in props {
        let row = style_picker::rename_style_row(s.app(), &sel, prop);
        assert_eq!((row.label.as_str(), row.disabled), ("Rename style", false), "{prop:?}: a local style can be renamed from the menu");
    }
    for (_, key) in props {
        make_library_style(&mut s, key);
    }
    for (prop, _) in props {
        let row = style_picker::rename_style_row(s.app(), &sel, prop);
        assert!(row.disabled, "{prop:?}: the Rename style row is disabled for a library style");
        assert_eq!(row.label, "Rename style", "{prop:?}: the Rename style row keeps its normal label");
        assert_eq!(row.tooltip, "Edit in the library file", "{prop:?}: the Rename style row gives its reason on hover");
    }
    // Double-clicking the linked row does not open the inline rename field for a library style.
    let linked = s.rect_of("effect-style-row");
    s.click(linked.center());
    s.click(linked.center());
    s.frame(vec![]);
    assert!(s.app().props.style_rename.is_none(), "double-clicking a library style's linked row does not start a rename");
    assert!(!s.shell.app.rename_style_to("Effect style 1", "Renamed"));
    assert_eq!(s.app().doc.style_name("Effect style 1"), "Effect style 1");
}

fn raw_field_prefix(s: &Sim, key_debug: &str) -> crate::widgets::field::Prefix {
    s.ctx
        .data(|d| d.get_temp::<crate::widgets::field::Prefix>(egui::Id::new(("field-prefix", key_debug.to_string()))))
        .unwrap_or_else(|| panic!("no field prefix {key_debug}"))
}

#[test]
fn popover_number_fields_use_distinct_non_duplicate_prefixes() {
    use crate::panels::effects::{default_effect, PopKind, PopState};
    use crate::widgets::field::Prefix;
    use std::collections::HashSet;

    let assert_distinct_icons = |s: &Sim, keys: &[&str], context: &str| {
        let mut seen = HashSet::new();
        for &k in keys {
            let p = raw_field_prefix(s, k);
            assert!(
                matches!(p, Prefix::Icon(_)),
                "{context}: field {k} should use a distinct icon prefix rather than repeating a row-label initial, got {p:?}"
            );
            assert!(seen.insert(p), "{context}: duplicate prefix {p:?} on field {k}");
        }
    };

    // 1. Layer Effects popover (`effects.rs:popover_ui` -> `effect_body`) across shadow, blur, noise, texture, and glass.
    let mut s = Sim::with(Options { select: Some("Welcome".into()), ..Default::default() });
    let welcome = s.by_name("Welcome");
    let anchor = egui::Rect::from_min_size(Pos2::new(1200.0, 300.0), Vec2::new(24.0, 24.0));

    let effect_cases: &[(Effect, &[&str], &str)] = &[
        (shadow(8.0), &["(\"fxp-x\", 0)", "(\"fxp-y\", 0)", "(\"fxp-b\", 0)", "(\"fxp-s\", 0)"], "layer DropShadow popover"),
        (default_effect(2, 1), &["(\"fxp-blur\", 0)"], "layer LayerBlur popover"),
        (default_effect(4, 1), &["(\"fxp-nsize\", 0)", "(\"fxp-dens\", 0)", "(\"fxp-nop\", 0)"], "layer Noise popover"),
        (default_effect(5, 1), &["(\"fxp-tsize\", 0)", "(\"fxp-trad\", 0)"], "layer Texture popover"),
        (
            default_effect(6, 1),
            &[
                "(\"fxp-angle\", 0)",
                "(\"fxp-glass\", 0, 0)",
                "(\"fxp-glass\", 0, 1)",
                "(\"fxp-glass\", 0, 2)",
                "(\"fxp-glass\", 0, 3)",
                "(\"fxp-glass\", 0, 4)",
            ],
            "layer Glass popover",
        ),
    ];

    for (fx, keys, label) in effect_cases {
        s.shell.app.edit_nodes(&[welcome], |n| n.effects = vec![fx.clone()]);
        s.shell.app.props.pop = Some(PopState { kind: PopKind::Effect(0), anchor, ids: vec![welcome] });
        s.frame(vec![]);
        assert_distinct_icons(&s, keys, label);
    }
    s.shell.app.props.pop = None;

    // 2. Edit style popover for Text style (`style_edit.rs:text_rows`).
    let mut s = Sim::with(Options { select: Some("Welcome back".into()), ..Default::default() });
    let t = s.by_name("Welcome back");
    s.shell.app.apply_style(&[t], "Body/Regular");
    edit_from_list(&mut s, "Body/Regular");
    assert_distinct_icons(&s, &["\"se-size\"", "\"se-line-height\"", "\"se-letter-spacing\""], "Text style edit popover");

    // 3. Edit style popover for Grid style (`style_edit_grid_rows.rs:grid_rows`).
    let (mut s, ..) = welcome_with_styles();
    let r = s.rect_of("grid-style-button");
    s.click(r.center());
    let top = r.bottom() + 4.0 + theme::MENU_PADDING + 5.0 * theme::MENU_ITEM_HEIGHT + crate::widgets::menu::SEP_H;
    s.click(Pos2::new(r.right() - 60.0, top + theme::MENU_ITEM_HEIGHT / 2.0));
    s.frame(vec![]);
    assert_distinct_icons(&s, &["\"se-grid-count\"", "\"se-grid-gutter\"", "\"se-grid-offset\"", "\"se-grid-opacity\""], "Grid style edit popover (Columns)");
}

#[test]
fn deleted_variable_bound_on_a_style_renders_detachable_pill_and_emits_broken_style_binding_lint() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let var_id = s.app().doc.variables.values().find(|v| v.name == "Brand/Primary").unwrap().id.clone();
    assert!(s.shell.app.bind_style_variable("Brand/Primary", "fills/0", &var_id));

    // Delete the bound variable so the style holds a dangling variable id.
    s.shell.app.doc.variables.remove(&var_id);
    assert_eq!(s.app().doc.style_meta["Brand/Primary"].bindings.get("fills/0"), Some(&var_id));

    // Both the document token linter and MCP lint report `broken-style-binding` (category `tokens`).
    let lint_report = designos_doc::openpencil::lint_document(&s.app().doc, None, "tokens");
    assert!(
        lint_report["issues"].as_array().unwrap().iter().any(|i| i["rule"] == "broken-style-binding" && i["category"] == "tokens"),
        "expected broken-style-binding in tokens lint report: {lint_report}"
    );
    let mut eng = designos_mcp::engine::Engine { doc: s.app().doc.clone(), ..Default::default() };
    let mcp_lint = eng.call("designos_lint", serde_json::json!({})).unwrap();
    assert!(
        mcp_lint["findings"].as_array().unwrap().iter().any(|f| f["rule"] == "broken-style-binding" && f["category"] == "tokens"),
        "expected broken-style-binding in MCP lint findings: {mcp_lint}"
    );

    // Opening Edit style still renders the detachable broken pill so the user can unbind it.
    s.shell.app.selection.clear();
    s.frame(vec![]);
    edit_from_list(&mut s, "Brand/Primary");
    let pill = s.rect_of("style-bound:Brand/Primary:fills/0");
    s.mv(pill.center());
    s.frame(vec![]);
    s.click(Pos2::new(pill.right() - 13.0, pill.center().y));
    assert!(!s.app().doc.style_meta["Brand/Primary"].bindings.contains_key("fills/0"), "clicking detach on the broken style pill unbinds the missing variable");
}

#[test]
fn editing_a_style_description_in_the_popover_survives_native_save_and_load() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    s.shell.app.selection.clear();
    s.frame(vec![]);
    edit_from_list(&mut s, "Brand/Primary");
    assert_eq!(s.app().props.style_edit.as_ref().map(|e| e.key.as_str()), Some("Brand/Primary"));

    let desc_rect = s.rect_of("se-description");
    type_into(&mut s, desc_rect, "Primary brand accent   ");
    assert_eq!(s.app().doc.style_meta["Brand/Primary"].description, "Primary brand accent");

    let tmp = std::env::temp_dir().join(format!("designos-style-desc-{}.designos", std::process::id()));
    let json = s.app().doc.to_json().unwrap();
    std::fs::write(&tmp, &json).unwrap();
    let loaded = designos_doc::Document::from_json(&std::fs::read_to_string(&tmp).unwrap()).unwrap();
    let _ = std::fs::remove_file(&tmp);
    assert_eq!(loaded.style_meta["Brand/Primary"].description, "Primary brand accent");

    // Setting the same text (after trailing whitespace trim) is a no-op that adds no undo step.
    assert!(!s.shell.app.set_style_description_to("Brand/Primary", "Primary brand accent   "));

    s.cmd_key(Key::Z);
    assert_eq!(crate::styles::style_description(&s.app().doc, "Brand/Primary"), "");

    // Library styles cannot have their description edited via App::set_style_description_to.
    make_library_style(&mut s, "Brand/Primary");
    assert!(!s.shell.app.set_style_description_to("Brand/Primary", "Blocked edit"));
    assert_eq!(crate::styles::style_description(&s.app().doc, "Brand/Primary"), "");
}

#[test]
fn style_picker_and_styles_list_show_description_tooltip_only_when_non_empty() {
    fn collect_texts(shape: &egui::epaint::Shape, out: &mut Vec<(String, Rect)>) {
        match shape {
            egui::epaint::Shape::Text(t) => out.push((t.galley.job.text.clone(), t.visual_bounding_rect())),
            egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| collect_texts(s, out)),
            _ => {}
        }
    }
    let painted_with_rects = |s: &Sim| -> Vec<(String, Rect)> {
        let mut out = Vec::new();
        for cs in &s.shapes {
            collect_texts(&cs.shape, &mut out);
        }
        out
    };

    let (mut s, welcome, _) = welcome_with_styles();
    let fx2 = s.shell.app.create_style_from(welcome, StyleProperty::Effect).unwrap();
    assert!(s.shell.app.set_style_description_to("Effect style 1", "Soft card drop shadow"));
    assert_eq!(s.app().doc.style_meta[&fx2].description, "");

    let btn = s.rect_of("effect-style-button");
    s.click(btn.center());
    s.frame(vec![]);
    let items = painted_with_rects(&s);
    let (_, fx1_menu_rect) = items.iter().rfind(|(t, _)| t == "Effect style 1").cloned().expect("Effect style 1 in picker menu");
    let (_, fx2_menu_rect) = items.iter().rfind(|(t, _)| t == &fx2).cloned().expect("second effect style in picker menu");

    let pt_menu_1 = fx1_menu_rect.center();
    s.mv(pt_menu_1 + Vec2::new(0.0, 1.0));
    s.mv(pt_menu_1);
    for _ in 0..30 {
        s.frame(vec![]);
    }
    assert!(
        painted_with_rects(&s).iter().any(|(t, _)| t == "Soft card drop shadow"),
        "hovering a style row with a description in the style picker menu should paint its description tooltip"
    );

    let pt_menu_2 = fx2_menu_rect.center();
    s.mv(pt_menu_2 + Vec2::new(0.0, 1.0));
    s.mv(pt_menu_2);
    for _ in 0..30 {
        s.frame(vec![]);
    }
    assert!(
        !painted_with_rects(&s).iter().any(|(t, _)| t == "Soft card drop shadow"),
        "hovering a style row without a description in the style picker menu must show no tooltip"
    );

    s.key(Key::Escape);

    // Colour-style menu (paint_title): non-empty description shows tooltip, folder header shows none.
    assert!(s.shell.app.set_style_description_to("Brand/Primary", "Brand primary fill"));
    let hero = s.by_name("Hero");
    s.shell.app.selection = vec![hero];
    s.shell.app.props.scroll = 0.0;
    s.frame(vec![]);
    s.frame(vec![]);
    s.shell.app.props.styles_menu = Some(false);
    s.mv(Pos2::new(10.0, 10.0));
    s.frame(vec![]);
    let color_items = painted_with_rects(&s);
    let (_, folder_rect) = color_items.iter().rfind(|(t, _)| t == "Brand").cloned().expect("Brand folder header in colour-style menu");
    let (_, primary_rect) = color_items.iter().rfind(|(t, _)| t.trim() == "Primary").cloned().expect("Primary row in colour-style menu");

    let pt_folder = folder_rect.center();
    s.mv(pt_folder + Vec2::new(0.0, 1.0));
    s.mv(pt_folder);
    for _ in 0..30 {
        s.frame(vec![]);
    }
    assert!(
        !painted_with_rects(&s).iter().any(|(t, _)| t == "Brand primary fill"),
        "hovering a folder header row in the colour-style menu must show no tooltip"
    );

    let pt_primary = primary_rect.center();
    s.mv(pt_primary + Vec2::new(0.0, 1.0));
    s.mv(pt_primary);
    for _ in 0..30 {
        s.frame(vec![]);
    }
    assert!(
        painted_with_rects(&s).iter().any(|(t, _)| t == "Brand primary fill"),
        "hovering a colour style row with a description in the colour-style menu should paint its description tooltip"
    );
    s.shell.app.props.styles_menu = None;

    s.shell.app.selection.clear();
    s.frame(vec![]);
    s.scroll_inspector_down();
    let fx1_row = s.rect_of("style:Effect style 1");
    let pt1 = Pos2::new(fx1_row.left() + 40.0, fx1_row.center().y);
    s.mv(pt1 + Vec2::new(0.0, 1.0));
    s.mv(pt1);
    for _ in 0..30 {
        s.frame(vec![]);
    }
    assert!(
        painted_with_rects(&s).iter().any(|(t, _)| t == "Soft card drop shadow"),
        "hovering a style row with a description in the Styles list should paint its description tooltip"
    );

    let fx2_row = s.rect_of(&format!("style:{fx2}"));
    let pt2 = Pos2::new(fx2_row.left() + 40.0, fx2_row.center().y);
    s.mv(pt2 + Vec2::new(0.0, 1.0));
    s.mv(pt2);
    for _ in 0..30 {
        s.frame(vec![]);
    }
    assert!(
        !painted_with_rects(&s).iter().any(|(t, _)| t == "Soft card drop shadow"),
        "hovering a style row without a description must not show the previous tooltip"
    );
}
