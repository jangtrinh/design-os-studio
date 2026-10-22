//! Fixes must change what is painted, never destroy an edit, and never be offered where the audit
//! cannot be sure. Each test pins one way a fix or a finding used to be wrong.

use super::*;
use designos_doc::{Rect, TextRun, Vec2};

fn text_props(app: &App, id: NodeId) -> designos_doc::TextProps {
    match &app.doc.get(id).unwrap().kind {
        NodeKind::Text(t) => t.clone(),
        other => panic!("not a text layer: {other:?}"),
    }
}

fn set_content(f: &mut Fx, id: NodeId, content: &str) {
    let NodeKind::Text(t) = &mut f.doc.get_mut(id).unwrap().kind else { panic!("not a text layer") };
    t.content = content.into();
}

fn expected_ratio(text: Color, bg: Color) -> String {
    format!("{:.2}", (contrast_ratio(text, bg) * 100.0).floor() / 100.0)
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    Color { r: a.r * t + b.r * (1.0 - t), g: a.g * t + b.g * (1.0 - t), b: a.b * t + b.b * (1.0 - t), a: 1.0 }
}

// ---------------------------------------------------------------- text runs

#[test]
fn the_contrast_fix_also_recolours_the_style_runs_because_the_renderer_paints_the_runs() {
    let mut f = fx();
    let frame = f.frame(f.page, "Card", Some(Color::WHITE));
    let t = f.text(frame, "Low contrast", 14.0, 400, gray(0x77));
    let NodeKind::Text(tp) = &mut f.doc.get_mut(t).unwrap().kind else { panic!("text") };
    tp.runs = vec![TextRun { start: 0, end: tp.content.len(), style: tp.style.clone() }];
    let mut app = f.app();
    let finding = fix_of(&app, Rule::Contrast);
    let Some(Fix::SetTextColor { color }) = finding.fix else { panic!("no colour fix: {finding:?}") };
    assert!(app.check_designs_fix(&finding));
    let tp = text_props(&app, t);
    assert_eq!(tp.style.fills[0].kind, PaintKind::Solid(color));
    assert_eq!(tp.runs[0].style.fills[0].kind, PaintKind::Solid(color), "the run covers the whole text, so it is what is painted");
    assert!(audit(&app.doc, &Scope::Page(app.page)).findings.is_empty());
    app.undo();
    let tp = text_props(&app, t);
    assert_eq!(tp.style.fills[0].kind, PaintKind::Solid(gray(0x77)));
    assert_eq!(tp.runs[0].style.fills[0].kind, PaintKind::Solid(gray(0x77)), "undo restores the run too");
}

// ---------------------------------------------------------------- reconnect

fn component_with_label(f: &mut Fx, source_id: &str) -> NodeId {
    let comp = component(f, source_id);
    f.text(comp, "Button", 14.0, 400, gray(0x10));
    comp
}

fn label_of(f: &Fx, frame: NodeId) -> NodeId {
    f.doc.get(frame).unwrap().children[0]
}

#[test]
fn a_detached_frame_whose_label_was_edited_is_reported_without_a_reconnect() {
    let mut f = fx();
    let comp = component_with_label(&mut f, "12:34");
    let frame = detached_copy(&mut f, comp, "Save", "guid:12:34");
    let label = label_of(&f, frame);
    set_content(&mut f, label, "Save changes");
    let found = only(&f.report(), Rule::DetachedInstance);
    assert_eq!(found.node, frame);
    assert_eq!(found.fix, None, "an instance would replace \"Save changes\" with \"Button\"");
    assert!(found.fix_label.is_empty());
    assert!(found.message.starts_with("Detached from \"Button\""), "{}", found.message);
}

#[test]
fn a_detached_frame_that_was_resized_is_reported_without_a_reconnect() {
    let mut f = fx();
    let comp = component_with_label(&mut f, "12:34");
    let frame = detached_copy(&mut f, comp, "Save", "guid:12:34");
    f.doc.get_mut(frame).unwrap().size = Vec2::new(200.0, 60.0);
    assert_eq!(only(&f.report(), Rule::DetachedInstance).fix, None);
}

#[test]
fn an_untouched_detached_copy_reconnects_and_keeps_its_own_name_and_size() {
    let mut f = fx();
    let comp = component_with_label(&mut f, "12:34");
    let frame = detached_copy(&mut f, comp, "Save", "guid:12:34");
    let size = f.doc.get(frame).unwrap().size;
    let mut app = f.app();
    let finding = fix_of(&app, Rule::DetachedInstance);
    assert_eq!(finding.fix, Some(Fix::Reconnect { component: comp }));
    assert!(app.check_designs_fix(&finding));
    let page = app.doc.get(app.page).unwrap();
    let inst = page.children.iter().filter_map(|c| app.doc.get(*c)).find(|n| matches!(n.kind, NodeKind::Instance { .. })).expect("an instance");
    assert_eq!(inst.name, "Save");
    assert_eq!(inst.size, size);
}

#[test]
fn a_reconnect_found_before_an_edit_is_refused_after_it() {
    let mut f = fx();
    let comp = component_with_label(&mut f, "12:34");
    let frame = detached_copy(&mut f, comp, "Save", "guid:12:34");
    let mut app = f.app();
    let finding = fix_of(&app, Rule::DetachedInstance);
    assert!(finding.fix.is_some());
    let label = app.doc.get(frame).unwrap().children[0];
    let NodeKind::Text(t) = &mut app.doc.get_mut(label).unwrap().kind else { panic!("text") };
    t.content = "Save changes".into();
    assert!(!app.check_designs_fix(&finding), "the edit made since the audit would be lost");
    assert!(app.doc.get(frame).is_some());
    assert!(!app.hist.can_undo());
}

#[test]
fn fix_all_leaves_an_edited_detached_frame_alone() {
    let mut f = fx();
    let comp = component_with_label(&mut f, "12:34");
    let edited = detached_copy(&mut f, comp, "Edited", "guid:12:34");
    let label = label_of(&f, edited);
    set_content(&mut f, label, "Save changes");
    let clean = detached_copy(&mut f, comp, "Clean", "guid:12:34");
    let mut app = f.app();
    let findings: Vec<_> = audit(&app.doc, &Scope::Page(app.page)).findings.into_iter().filter(|x| x.rule == Rule::DetachedInstance).collect();
    assert_eq!(findings.len(), 2);
    assert_eq!(app.check_designs_fix_all(&findings), 1, "only the untouched copy is safe");
    assert!(app.doc.get(edited).is_some(), "the edited frame is still there");
    assert!(app.doc.get(clean).is_none(), "the clean copy became an instance");
}

// ---------------------------------------------------------------- what is behind the text

#[test]
fn text_over_an_earlier_sibling_that_overlaps_it_is_not_checked() {
    let mut f = fx();
    let frame = f.frame(f.page, "Card", Some(Color::WHITE));
    let panel = f.rect(frame, "Dark panel", gray(0x10));
    f.doc.get_mut(panel).unwrap().size = Vec2::new(150.0, 60.0);
    f.text(frame, "White on dark", 14.0, 400, Color::WHITE);
    assert!(f.report().findings.is_empty(), "{:#?}", f.report().findings);
}

#[test]
fn text_over_an_earlier_sibling_of_one_of_its_ancestors_is_not_checked() {
    let mut f = fx();
    // Painted first, so the dark panel is behind the card and everything in it.
    let panel = f.rect(f.page, "Dark panel", gray(0x10));
    f.doc.get_mut(panel).unwrap().size = Vec2::new(500.0, 500.0);
    let card = f.frame(f.page, "Card", None);
    f.doc.get_mut(card).unwrap().transform.tx = 0.0;
    let inner = f.frame(card, "Inner", None);
    // #666666 on the page colour (#5A5A5A) would fail, but the page is not what is behind it.
    f.text(inner, "Grey on dark", 14.0, 400, gray(0x66));
    assert!(f.report().findings.is_empty(), "{:#?}", f.report().findings);
    // The same text with nothing behind it is checked, so the skip above is the sibling's doing.
    let clear = f.frame(f.page, "Clear", None);
    f.doc.get_mut(clear).unwrap().transform.tx = 900.0;
    let t = f.text(clear, "Grey on page", 14.0, 400, gray(0x66));
    assert_eq!(only(&f.report(), Rule::Contrast).node, t);
}

#[test]
fn layers_that_do_not_cover_the_text_do_not_stop_the_check() {
    let mut f = fx();
    let frame = f.frame(f.page, "Card", Some(Color::WHITE));
    let far = f.rect(frame, "Far away", gray(0x10));
    f.doc.get_mut(far).unwrap().transform.tx = 150.0;
    f.doc.get_mut(far).unwrap().transform.ty = 90.0;
    let hidden = f.rect(frame, "Hidden panel", gray(0x10));
    f.doc.get_mut(hidden).unwrap().size = Vec2::new(150.0, 60.0);
    f.doc.get_mut(hidden).unwrap().visible = false;
    let t = f.text(frame, "White on white", 14.0, 400, Color::WHITE);
    // A later sibling is painted above the text, not behind it.
    let above = f.rect(frame, "Above", gray(0x10));
    f.doc.get_mut(above).unwrap().size = Vec2::new(150.0, 60.0);
    assert_eq!(only(&f.report(), Rule::Contrast).node, t);
}

// ---------------------------------------------------------------- variables

#[test]
fn a_variable_that_holds_in_every_mode_wins_over_one_that_only_matches_in_this_mode() {
    let mut f = fx();
    f.rect(f.page, "Card", BLUE);
    f.color_var("Accent/Modal", BLUE, gray(0x20));
    let steady = f.color_var("Brand/Primary", BLUE, BLUE);
    let found = only(&f.report(), Rule::HardCodedValue);
    assert_eq!(found.fix, Some(Fix::BindVariable { key: "fills/0".into(), variable: steady }));
    assert_eq!(found.fix_label, "Bind to variable Brand/Primary");
}

#[test]
fn two_equally_good_variables_report_the_match_without_a_fix() {
    let mut f = fx();
    let r = f.rect(f.page, "Card", BLUE);
    f.color_var("Brand/Primary", BLUE, BLUE);
    f.color_var("Brand/Link", BLUE, BLUE);
    let found = only(&f.report(), Rule::HardCodedValue);
    assert_eq!(found.fix, None);
    assert!(found.fix_label.is_empty());
    assert_eq!(found.message, "Fill #1A73E8 matches 2 variables: \"Brand/Link\", \"Brand/Primary\"");
    let mut app = f.app();
    let report = audit(&app.doc, &Scope::Page(app.page));
    assert!(report.fixable().is_empty());
    assert_eq!(app.check_designs_fix_all(&report.findings), 0);
    assert!(app.doc.get(r).unwrap().variables.bindings.is_empty());
}

#[test]
fn two_variables_that_both_change_with_the_mode_are_also_ambiguous() {
    let mut f = fx();
    f.rect(f.page, "Card", BLUE);
    f.color_var("Accent/One", BLUE, gray(0x20));
    f.color_var("Accent/Two", BLUE, gray(0x30));
    assert_eq!(only(&f.report(), Rule::HardCodedValue).fix, None);
}

// ---------------------------------------------------------------- opacity

#[test]
fn an_ancestors_opacity_dims_the_backdrop_of_the_frames_inside_it() {
    let mut f = fx();
    let outer = f.frame(f.page, "Outer", None);
    f.doc.get_mut(outer).unwrap().opacity = 0.5;
    let inner = f.frame(outer, "Inner", Some(Color::WHITE));
    let t = f.text(inner, "Dim", 14.0, 400, gray(0x40));
    let page = gray(0x5A);
    // The whole group is composited at 50% over the page: #404040 text on white shows as a 50% mix with the page.
    let expected = expected_ratio(mix(gray(0x40), page, 0.5), mix(Color::WHITE, page, 0.5));
    let found = only(&f.report(), Rule::Contrast);
    assert_eq!(found.node, t);
    assert_eq!(found.message, format!("Contrast {expected}:1 is below 4.5:1 (WCAG AA)"));
}

#[test]
fn an_ancestors_opacity_scales_the_text_alpha_too() {
    let mut f = fx();
    let frame = f.frame(f.page, "Card", Some(Color::WHITE));
    f.doc.get_mut(frame).unwrap().opacity = 0.5;
    f.text(frame, "Dim", 14.0, 400, gray(0x77));
    let page = gray(0x5A);
    let expected = expected_ratio(mix(gray(0x77), page, 0.5), mix(Color::WHITE, page, 0.5));
    let found = only(&f.report(), Rule::Contrast);
    assert_eq!(found.message, format!("Contrast {expected}:1 is below 4.5:1 (WCAG AA)"));
}

#[test]
fn the_contrast_fix_reaches_aa_under_an_ancestor_with_opacity() {
    let mut f = fx();
    let outer = f.frame(f.page, "Outer", None);
    f.doc.get_mut(outer).unwrap().opacity = 0.5;
    let inner = f.frame(outer, "Inner", Some(Color::WHITE));
    f.text(inner, "Dim", 14.0, 400, gray(0x40));
    let mut app = f.app();
    let finding = fix_of(&app, Rule::Contrast);
    assert!(finding.fix.is_some(), "a darker colour can still reach 4.5:1 here");
    assert!(app.check_designs_fix(&finding));
    assert!(audit(&app.doc, &Scope::Page(app.page)).findings.is_empty(), "the compound contrast of the new colour passes");
}

// ---------------------------------------------------------------- stale colour

#[test]
fn a_stale_stored_colour_is_refused_and_the_stored_colour_is_what_gets_applied() {
    let mut f = fx();
    let frame = f.frame(f.page, "Card", Some(Color::WHITE));
    let t = f.text(frame, "Low", 14.0, 400, gray(0x77));
    let mut app = f.app();
    let finding = fix_of(&app, Rule::Contrast);
    let Some(Fix::SetTextColor { color }) = finding.fix.clone() else { panic!("no colour fix") };
    let stale = Finding { fix: Some(Fix::SetTextColor { color: gray(0x90) }), ..finding.clone() };
    assert!(!app.check_designs_fix(&stale), "#909090 is not what the audit suggested for this layer now");
    assert_eq!(text_props(&app, t).style.fills[0].kind, PaintKind::Solid(gray(0x77)));
    assert!(!app.hist.can_undo());
    assert!(app.check_designs_fix(&finding));
    assert_eq!(text_props(&app, t).style.fills[0].kind, PaintKind::Solid(color));
}

// ---------------------------------------------------------------- panel scope

#[test]
fn deleting_the_page_the_panel_checks_moves_the_panel_to_the_page_you_land_on() {
    let mut f = fx();
    let second = f.doc.add_page("Second");
    let r = f.doc.alloc_id();
    let mut card = Node::rectangle(r, Rect::new(0.0, 0.0, 40.0, 40.0));
    card.fills = vec![Paint::solid(BLUE)];
    f.doc.insert(second, card).unwrap();
    f.color_var("Color/Primary", BLUE, BLUE);
    let first = f.page;
    let mut app = f.app();
    app.open_check_designs();
    app.refresh_check_designs(false, &egui::Context::default());
    assert_eq!(app.check_designs.scope, Some(Scope::Page(first)));
    assert!(app.check_designs.report.findings.is_empty());
    assert!(app.delete_page(first));
    assert_eq!(app.page, second);
    app.refresh_check_designs(false, &egui::Context::default());
    assert_eq!(app.check_designs.scope, Some(Scope::Page(second)));
    assert_eq!(app.check_designs.scope_label, "Page: Second");
    assert_eq!(app.check_designs.report.findings.len(), 1, "the panel lists the page that is on screen");
}

#[test]
fn a_page_scope_on_a_page_that_still_exists_is_not_moved() {
    let mut f = fx();
    let second = f.doc.add_page("Second");
    let first = f.page;
    let mut app = f.app();
    app.open_check_designs();
    app.switch_page(second);
    app.refresh_check_designs(false, &egui::Context::default());
    assert_eq!(app.check_designs.scope, Some(Scope::Page(first)), "the scope stays fixed while its page exists");
}

// ---------------------------------------------------------------- resolving detached frames

#[test]
fn detached_symbols_resolve_to_main_components_by_source_id_only() {
    let mut f = fx();
    let comp = component(&mut f, "12:34");
    let lookalike = f.frame(f.page, "Not a component", None);
    f.doc.get_mut(lookalike).unwrap().source_id = Some("56:78".into());
    let components = super::super::detached::Components::new(&f.doc);
    assert_eq!(components.resolve("guid:12:34"), Some(comp));
    assert_eq!(components.resolve("12:34"), Some(comp), "a bare id resolves too");
    assert_eq!(components.resolve("assetRef:12:34"), None, "a library component is not in this file");
    assert_eq!(components.resolve("56:78"), None, "only a main component can be the target");
    assert_eq!(components.resolve("99:99"), None);
}

#[test]
fn a_source_id_shared_by_two_components_resolves_to_the_lower_id() {
    let mut f = fx();
    let first = component(&mut f, "12:34");
    let second = component(&mut f, "12:34");
    assert!(first < second);
    assert_eq!(super::super::detached::Components::new(&f.doc).resolve("guid:12:34"), Some(first));
}

#[test]
fn every_detached_frame_in_a_big_file_finds_its_own_component() {
    let mut f = fx();
    let mut expected = Vec::new();
    for i in 0..60 {
        let comp = component(&mut f, &format!("1:{i}"));
        let copy = detached_copy(&mut f, comp, &format!("Copy {i}"), &format!("guid:1:{i}"));
        expected.push((copy, comp));
    }
    let report = f.report();
    for (copy, comp) in expected {
        let found = report.findings.iter().find(|x| x.node == copy).expect("a finding per detached frame");
        assert_eq!(found.fix, Some(Fix::Reconnect { component: comp }));
    }
}
