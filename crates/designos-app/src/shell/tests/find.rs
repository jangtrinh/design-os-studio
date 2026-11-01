//! UI tests for Find and replace, the layers name filter and the selection commands.

use super::*;
use crate::panels::menus::Action;
use designos_doc::find::MatchCriteria;

fn text_content(s: &Sim, id: NodeId) -> String {
    match &s.app().doc.get(id).unwrap().kind {
        NodeKind::Text(t) => t.content.clone(),
        _ => panic!("not a text layer"),
    }
}

fn with_mods(s: &mut Sim, mods: Modifiers, key: Key) {
    s.mods = mods;
    s.key(key);
    s.mods = Modifiers::NONE;
}

fn open_find(s: &mut Sim, query: &str) {
    s.cmd_key(Key::F);
    assert!(s.app().find.open, "Ctrl+F opens the panel");
    s.type_text(query);
    s.frame(vec![]);
}

#[test]
fn ctrl_f_finds_and_replaces_all_in_one_undo() {
    let mut s = Sim::new();
    let title = s.by_name("Quarterly report");
    let body = s.by_name("Revenue grew 18% over the last quarter.");
    open_find(&mut s, "quarter");
    assert_eq!(s.app().find.results.len(), 2, "{:?}", s.app().find.results);
    assert!(s.app().find.results.iter().any(|m| m.node == title) && s.app().find.results.iter().any(|m| m.node == body));

    // Open the replace row, fill it in and press "Replace all".
    s.click(s.rect_of("find:replace-toggle").center());
    assert!(s.app().find.show_replace);
    s.click(s.rect_of("find:replace").center());
    s.type_text("period");
    s.frame(vec![]);
    assert_eq!(s.app().find.replacement, "period");
    s.click(s.rect_of("find:replace-all").center());

    assert_eq!(text_content(&s, title), "periodly report");
    assert_eq!(text_content(&s, body), "Revenue grew 18% over the last period.");
    assert_eq!(s.app().doc.get(title).unwrap().name, "periodly report", "a layer named after its text follows it");
    assert!(s.app().find.results.is_empty(), "nothing left to find");
    // The layer is refit to the new text.
    let w = s.app().doc.get(body).unwrap().size.x;
    assert!(w > 100.0, "{w}");

    // One undo restores both layers.
    s.key(Key::Escape);
    s.frame(vec![]);
    assert!(!s.app().find.open, "Esc closes the panel");
    s.cmd_key(Key::Z);
    assert_eq!(text_content(&s, title), "Quarterly report");
    assert_eq!(text_content(&s, body), "Revenue grew 18% over the last quarter.");
    assert!(!s.app().hist.can_undo(), "replace all was a single step");
    s.cmd_key(Key::Y);
    assert_eq!(text_content(&s, title), "periodly report");
}

#[test]
fn enter_and_shift_enter_step_through_results_and_select_them() {
    let mut s = Sim::new();
    open_find(&mut s, "quarter");
    let hits: Vec<NodeId> = s.app().find.results.iter().map(|m| m.node).collect();
    assert_eq!(hits.len(), 2);
    s.key(Key::Enter);
    assert_eq!(s.app().selection, vec![hits[0]]);
    assert_eq!(s.app().find.current, Some(0));
    s.key(Key::Enter);
    assert_eq!(s.app().selection, vec![hits[1]]);
    s.key(Key::Enter);
    assert_eq!(s.app().selection, vec![hits[0]], "wraps around");
    with_mods(&mut s, Modifiers::SHIFT, Key::Enter);
    assert_eq!(s.app().selection, vec![hits[1]], "Shift+Enter goes back");
    // The match is brought into view.
    let b = crate::util::world_aabb(&s.app().doc, hits[1]);
    let c = s.app().view.to_screen(s.app().canvas_rect.min, crate::util::rect_center(&b));
    assert!(s.app().canvas_rect.contains(c), "{c:?} outside {:?}", s.app().canvas_rect);
    // Clicking a result row selects that layer.
    s.click(s.rect_of("find:hit0").center());
    assert_eq!(s.app().selection, vec![hits[0]]);
}

#[test]
fn options_change_what_is_found() {
    let mut s = Sim::new();
    open_find(&mut s, "QUARTER");
    assert_eq!(s.app().find.results.len(), 2);
    s.click(s.rect_of("find:case").center());
    assert!(s.app().find.opts.match_case);
    assert!(s.app().find.results.is_empty(), "case sensitive: no upper-case QUARTER");
    s.click(s.rect_of("find:case").center());
    // Whole word: "quarter" no longer matches the start of "Quarterly".
    s.click(s.rect_of("find:search").center());
    s.cmd_key(Key::A);
    s.type_text("quarter");
    s.click(s.rect_of("find:word").center());
    assert!(s.app().find.opts.whole_word);
    assert_eq!(s.app().find.results.len(), 1, "only \"quarter.\" in the sentence");
    // An invalid regex reports instead of matching.
    s.click(s.rect_of("find:word").center());
    s.click(s.rect_of("find:regex").center());
    s.click(s.rect_of("find:search").center());
    s.cmd_key(Key::A);
    s.type_text("(");
    s.frame(vec![]);
    assert!(s.app().find.error.is_some());
    assert!(s.app().find.results.is_empty());
}

#[test]
fn replace_one_replaces_the_current_result_and_advances() {
    let mut s = Sim::new();
    open_find(&mut s, "quarter");
    s.shell.app.find.show_replace = true;
    s.shell.app.find.replacement = "term".into();
    s.frame(vec![]);
    s.key(Key::Enter); // first result
    let first = s.app().find.results[0].node;
    let second = s.app().find.results[1].node;
    s.click(s.rect_of("find:replace-one").center());
    assert_eq!(s.app().find.results.len(), 1);
    assert!(text_content(&s, first).to_lowercase().contains("term"));
    assert!(!text_content(&s, second).contains("term"));
    assert_eq!(s.app().selection, vec![second], "moved on to the remaining match");
}

#[test]
fn find_can_search_all_pages_and_layer_names() {
    let mut s = Sim::new();
    open_find(&mut s, "button");
    // "Primary button" lives on the Components page; the current page has the "Button" layer.
    let here = s.app().find.results.len();
    s.click(s.rect_of("find:scope").center());
    assert_eq!(s.app().find.search_in, crate::find::SearchIn::AllPages);
    assert!(s.app().find.results.len() > here, "{} vs {here}", s.app().find.results.len());
    let comp_page = s.app().doc.pages[1];
    let on_other = s.app().find.results.iter().position(|m| s.app().doc.page_of(m.node) == Some(comp_page)).expect("a hit on the Components page");
    s.shell.app.find_goto(on_other);
    assert_eq!(s.app().page, comp_page, "navigating switches page");
}

#[test]
fn esc_in_the_field_closes_the_panel_and_returns_the_layers() {
    let mut s = Sim::new();
    open_find(&mut s, "x");
    s.key(Key::Escape);
    assert!(!s.app().find.open);
    s.frame(vec![]);
    // The layers tree is back (its search icon is drawn).
    let _ = s.rect_of("layers-search");
}

// ---- layer name filter

#[test]
fn layer_filter_narrows_the_tree_and_clears_with_escape() {
    let mut s = Sim::new();
    let all = crate::panels::layers::visible_rows(s.app()).len();
    s.click(s.rect_of("layers-search").center());
    assert!(s.app().layers.filter_open);
    s.type_text("tile");
    s.frame(vec![]);
    let rows = crate::panels::layers::visible_rows(s.app());
    let names: Vec<String> = rows.iter().map(|r| s.app().doc.get(r.id).unwrap().name.clone()).collect();
    assert_eq!(names, ["Dashboard", "Tile C", "Tile B", "Tile A"], "matches and their parent only");
    assert!(rows.len() < all);
    // A layer can be selected from the filtered list.
    let tile_a = s.by_name("Tile A");
    let first_row_top = s.rect_of("layers-filter").bottom();
    let y = first_row_top + 3.0 * theme::LAYER_ROW_HEIGHT + theme::LAYER_ROW_HEIGHT / 2.0;
    s.click(Pos2::new(120.0, y));
    assert_eq!(s.app().selection, vec![tile_a]);
    // Escape in the field clears the filter and closes it.
    s.click(s.rect_of("layers-filter").center());
    s.key(Key::Escape);
    assert!(!s.app().layers.filter_open && s.app().layers.filter.is_empty());
    assert!(crate::panels::layers::visible_rows(s.app()).len() >= all, "the whole tree is back");
}

// ---- selection commands

#[test]
fn select_same_fill_from_the_palette_picks_matching_layers_on_the_page() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let hero = s.by_name("Hero");
    let progress = s.by_name("Progress"); // the other brand-blue layer on this page
    s.cmd_key(Key::K);
    s.type_text("same fill");
    s.frame(vec![]);
    s.key(Key::Enter);
    let sel = s.app().selection.clone();
    assert!(sel.contains(&hero) && sel.contains(&progress), "{sel:?}");
    // Everything picked has the brand fill and lives on this page; the main component's blue
    // "Bg" on the Components page stays out, the "Get started" instance's copy comes along.
    let page = s.app().page;
    let brand = s.app().doc.get(hero).unwrap().fills.clone();
    for id in &sel {
        let n = s.app().doc.get(*id).unwrap();
        assert_eq!(n.fills, brand, "{}", n.name);
        assert_eq!(s.app().doc.page_of(*id), Some(page), "{}", n.name);
    }
    assert!(sel.len() >= 3);
}

#[test]
fn select_same_font_stroke_and_instance_actions() {
    let mut s = Sim::with(Options { select: Some("Welcome back".into()), ..Default::default() });
    let ctx = s.ctx.clone();
    crate::panels::menus::run(&mut s.shell.app, &ctx, Action::SelectSame(MatchCriteria::Font));
    // Only the heading uses 32px / 700.
    assert_eq!(s.app().selection, vec![s.by_name("Welcome back")]);
    // Stroke: the Avatar has the only stroke on the page.
    s.shell.app.select_only(s.by_name("Avatar"));
    crate::panels::menus::run(&mut s.shell.app, &ctx, Action::SelectSame(MatchCriteria::Stroke));
    assert_eq!(s.app().selection, vec![s.by_name("Avatar")]);
    // Instances of the Primary button.
    s.shell.app.select_only(s.by_name("Get started"));
    crate::panels::menus::run(&mut s.shell.app, &ctx, Action::SelectSame(MatchCriteria::Instance));
    let mut sel = s.app().selection.clone();
    sel.sort();
    let mut want = vec![s.by_name("Get started"), s.by_name("Learn more")];
    want.sort();
    assert_eq!(sel, want);
}

#[test]
fn ctrl_alt_a_selects_matching_layers() {
    let mut s = Sim::with(Options { select: Some("Tile A".into()), ..Default::default() });
    let ctx = s.ctx.clone();
    let tile_a = s.by_name("Tile A");
    s.shell.app.duplicate_selection();
    let copy_in_same_frame = s.app().selection[0];
    assert_ne!(copy_in_same_frame, tile_a);
    s.shell.app.select_only(tile_a);
    crate::panels::menus::run(&mut s.shell.app, &ctx, crate::panels::menus::Action::SelectSame(designos_doc::find::MatchCriteria::Properties));
    let mut same_props = s.app().selection.clone();
    same_props.sort();
    let mut want_props = vec![tile_a, copy_in_same_frame];
    want_props.sort();
    assert_eq!(same_props, want_props, "the duplicate has the same type and properties; Tile B and C differ");

    // Remove the in-frame duplicate, duplicate the enclosing top-level Dashboard frame,
    // and verify Ctrl+Alt+A selects the matching Tile A across top-level frames.
    s.shell.app.select_only(copy_in_same_frame);
    s.shell.app.delete_selection();
    let dashboard = s.by_name("Dashboard");
    s.shell.app.select_only(dashboard);
    s.shell.app.duplicate_selection();
    let dup_dashboard = s.app().selection[0];
    let matching_tile_a = s
        .app()
        .doc
        .get(dup_dashboard)
        .unwrap()
        .children
        .iter()
        .copied()
        .find(|&id| s.app().doc.get(id).unwrap().name == "Tile A")
        .expect("duplicated Dashboard has Tile A");
    s.shell.app.select_only(tile_a);
    s.mods = Modifiers { ctrl: true, alt: true, command: true, ..Default::default() };
    s.key(Key::A);
    s.mods = Modifiers::NONE;
    let mut sel = s.app().selection.clone();
    sel.sort();
    let mut want = vec![tile_a, matching_tile_a];
    want.sort();
    assert_eq!(sel, want, "Ctrl+Alt+A selects matching layers across top-level frames");
}

#[test]
fn select_all_inverse_children_and_parent_shortcuts() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let welcome = s.by_name("Welcome");
    let hero = s.by_name("Hero");
    let kids = s.app().doc.get(welcome).unwrap().children.clone();
    // Ctrl+A stays inside the parent of the selection.
    s.cmd_key(Key::A);
    assert_eq!(s.app().selection, kids);
    // Ctrl+Shift+A inverts inside the parent.
    s.shell.app.select_only(hero);
    s.mods = Modifiers { shift: true, command: true, ctrl: true, ..Default::default() };
    s.key(Key::A);
    s.mods = Modifiers::NONE;
    let mut inv = s.app().selection.clone();
    let mut want: Vec<NodeId> = kids.iter().copied().filter(|k| *k != hero).collect();
    inv.sort();
    want.sort();
    assert_eq!(inv, want);
    // Enter selects the children, Shift+Enter and \ go back up.
    s.shell.app.select_only(welcome);
    s.key(Key::Enter);
    assert_eq!(s.app().selection, kids);
    with_mods(&mut s, Modifiers::SHIFT, Key::Enter);
    assert_eq!(s.app().selection, vec![welcome]);
    s.key(Key::Enter);
    s.key(Key::Backslash);
    assert_eq!(s.app().selection, vec![welcome]);
    // A top-level layer has no selectable parent: the selection stays.
    s.key(Key::Backslash);
    assert_eq!(s.app().selection, vec![welcome]);
    // Enter on a text layer still starts editing it.
    s.shell.app.select_only(s.by_name("Welcome back"));
    s.key(Key::Enter);
    assert!(s.app().canvas.text.is_some());
}

#[test]
fn edit_menu_and_context_menu_list_the_selection_commands() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let ctx = s.ctx.clone();
    let labels = crate::panels::context::labels(s.app());
    assert!(labels.contains(&"Select".to_string()), "{labels:?}");
    // The palette lists every one of them with its shortcut.
    s.cmd_key(Key::K);
    for (q, shortcut) in [
        ("select inverse", "Ctrl+Shift+A"),
        ("select matching", "Ctrl+Alt+A"),
        ("select children", "Enter"),
        ("select parent", "Shift+Enter"),
        ("find and replace", "Ctrl+F"),
        ("same instance", ""),
    ] {
        let cmds = crate::panels::menus::palette_entries(s.app(), q);
        assert!(!cmds.is_empty(), "{q} is in the palette");
        if !shortcut.is_empty() {
            assert!(cmds.iter().any(|(_, sc)| sc == shortcut), "{q}: {cmds:?}");
        }
    }
    s.key(Key::Escape);
    let _ = ctx;
    // The right-click menu on a layer offers the submenu.
    let hero = s.by_name("Hero");
    let c = crate::util::rect_center(&crate::util::world_aabb(&s.app().doc, hero));
    let p = s.screen(c.x, c.y);
    s.right_click(p);
    assert!(s.app().ui.ctx_menu.is_some());
}

/// Number of filled polygons / rects on screen painted with `color`.
fn filled_with(s: &Sim, color: egui::Color32) -> Vec<Rect> {
    let mut out = Vec::new();
    for cs in &s.shapes {
        match &cs.shape {
            egui::Shape::Path(p) if p.fill == color => out.push(Rect::from_points(&p.points)),
            egui::Shape::Rect(r) if r.fill == color => out.push(r.rect),
            _ => {}
        }
    }
    out
}

#[test]
fn matches_are_highlighted_on_the_canvas_text() {
    let mut s = Sim::new();
    let canvas_boxes =
        |s: &Sim| -> Vec<Rect> { filled_with(s, crate::theme::FIND_CANVAS_MATCH).into_iter().filter(|r| s.app().canvas_rect.intersects(*r)).collect() };
    assert!(canvas_boxes(&s).is_empty(), "no search, no highlight");
    open_find(&mut s, "quarter");
    s.frame(vec![]);
    let boxes = canvas_boxes(&s);
    assert_eq!(boxes.len(), 2, "one box per match in view: {boxes:?}");
    // each box sits exactly where the editor geometry puts the match
    for m in s.app().find.results.clone() {
        let n = s.app().doc.get(m.node).unwrap();
        let NodeKind::Text(t) = &n.kind else { panic!() };
        let rects = crate::textmeasure::selection_rects(t, n.size, m.range.clone());
        assert_eq!(rects.len(), 1);
        let w = s.app().doc.world_transform(m.node);
        let (x, y, rw, rh) = rects[0];
        let a = s.app().view.to_screen(s.app().canvas_rect.min, w.apply(DV::new(x, y)));
        let b = s.app().view.to_screen(s.app().canvas_rect.min, w.apply(DV::new(x + rw, y + rh)));
        let want = Rect::from_two_pos(a, b);
        assert!(boxes.iter().any(|r| (r.min - want.min).length() < 0.6 && (r.max - want.max).length() < 0.6), "{want:?} not in {boxes:?}");
    }
    // the box covers only the matched word, not the whole layer
    let body = s.app().doc.get(s.by_name("Revenue grew 18% over the last quarter.")).unwrap().size.x * s.app().view.zoom;
    assert!(boxes.iter().all(|r| r.width() < body * 0.5), "{boxes:?}");
    // a different query moves the highlight; closing the panel removes it
    s.cmd_key(Key::A);
    s.type_text("Revenue");
    s.frame(vec![]);
    s.frame(vec![]);
    assert_eq!(canvas_boxes(&s).len(), 1);
    s.key(Key::Escape);
    s.frame(vec![]);
    assert!(canvas_boxes(&s).is_empty(), "closing Find clears the highlights");
}

#[test]
fn entering_edit_mode_after_next_match_selects_the_match() {
    let mut s = Sim::new();
    open_find(&mut s, "quarter");
    s.key(Key::Enter);
    s.key(Key::Enter);
    let m = s.app().find.results[s.app().find.current.unwrap()].clone();
    let id = m.node;
    assert_eq!(s.app().selection, vec![id]);
    crate::canvas::textsession::begin_text_edit(&mut s.shell.app, id);
    let sess = s.app().canvas.text.as_ref().unwrap();
    assert_eq!(sess.range(), m.range, "the caret selection is the match");
    assert_eq!(text_content(&s, id)[sess.range()].to_lowercase(), "quarter");
    // typing replaces just that word
    let before = text_content(&s, id);
    crate::canvas::textsession::insert_text(&mut s.shell.app, "period", crate::textedit::EditKind::Typing);
    let after = text_content(&s, id);
    assert_eq!(after, format!("{}period{}", &before[..m.range.start], &before[m.range.end..]));
    crate::canvas::textsession::finish_text(&mut s.shell.app, true);
    // without a search, entering selects everything as before
    s.shell.app.close_find();
    crate::canvas::textsession::begin_text_edit(&mut s.shell.app, id);
    assert_eq!(s.app().canvas.text.as_ref().unwrap().range(), 0..text_content(&s, id).len());
}
