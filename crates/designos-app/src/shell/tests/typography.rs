//! The Type settings popover: OpenType features and paragraph settings.

use super::*;
use designos_doc::{LeadingTrim, TextAutoResize, TextDirection};

fn texts_in_rect(s: &Sim, rect: Rect) -> Vec<String> {
    crate::lint::scan_shapes(&s.shapes).0.into_iter().filter(|t| rect.intersects(t.rect)).map(|t| t.text).collect()
}

fn popover_sim() -> Sim {
    Sim::with(Options { select: Some("Continue".into()), panel: Some("type-details".into()), ..Options::default() })
}

fn click_rect(s: &mut Sim, r: Rect) {
    s.click(r.center());
}

fn has_no_rect(s: &Sim, name: &str) -> bool {
    s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", name.to_string())))).is_none()
}

fn more_open(s: &Sim) -> bool {
    crate::lint::scan_shapes(&s.shapes).0.iter().any(|t| t.text == "Italic")
}

#[test]
fn feature_checks_toggle_on_the_layer() {
    let mut s = popover_sim();
    let id = s.by_name("Continue");
    assert!(text_of(&s, id).style.features.is_empty());
    let case = s.rect_of("det-case");
    click_rect(&mut s, case);
    assert_eq!(text_of(&s, id).style.features, vec![("case".to_string(), 1)]);
    click_rect(&mut s, case);
    assert!(text_of(&s, id).style.features.is_empty(), "clicking again returns to the font default");
    // ligatures and contextual alternates are on by default: unchecking writes an explicit 0
    let calt = s.rect_of("det-calt");
    click_rect(&mut s, calt);
    assert_eq!(text_of(&s, id).style.features, vec![("calt".to_string(), 0)]);
    click_rect(&mut s, calt);
    assert!(text_of(&s, id).style.features.is_empty());
    // a stylistic set Inter has
    let ss = s.rect_of("det-ss-1");
    click_rect(&mut s, ss);
    assert_eq!(text_of(&s, id).style.features, vec![("ss01".to_string(), 1)]);
    // one undo step per change
    s.cmd_key(Key::Z);
    assert!(text_of(&s, id).style.features.is_empty());
}

#[test]
fn popover_lists_only_the_stylistic_sets_the_font_has() {
    let s = popover_sim();
    for set in 1..=8 {
        s.rect_of(&format!("det-ss-{set}"));
    }
    let nine = s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", "det-ss-9".to_string()))));
    assert!(nine.is_none(), "Inter has ss01..ss08 only");
}

#[test]
fn paragraph_settings_apply_to_the_layer() {
    let mut s = popover_sim();
    let id = s.by_name("Continue");
    let r = s.field_rect("det-indent");
    s.click(Pos2::new(r.left() + 30.0, r.center().y));
    s.type_text("12");
    s.key(Key::Enter);
    assert_eq!(text_of(&s, id).paragraph.indent, 12.0);
    let r = s.field_rect("det-lspacing");
    s.click(Pos2::new(r.left() + 30.0, r.center().y));
    s.type_text("6");
    s.key(Key::Enter);
    assert_eq!(text_of(&s, id).paragraph.list_spacing, 6.0);
    let r = s.field_rect("det-pspacing");
    s.click(Pos2::new(r.left() + 30.0, r.center().y));
    s.type_text("9");
    s.key(Key::Enter);
    assert_eq!(text_of(&s, id).style.paragraph_spacing, 9.0);
    let hang = s.rect_of("det-hangp");
    click_rect(&mut s, hang);
    assert!(text_of(&s, id).paragraph.hanging_punctuation);
    // text direction: the segmented control's third cell is RTL
    let dir = s.rect_of("det-dir");
    s.click(Pos2::new(dir.right() - 10.0, dir.center().y));
    assert_eq!(text_of(&s, id).direction, TextDirection::Rtl);
    s.click(Pos2::new(dir.left() + 10.0, dir.center().y));
    assert_eq!(text_of(&s, id).direction, TextDirection::Auto);
    assert_eq!(text_of(&s, id).paragraph.leading_trim, LeadingTrim::None);
}

#[test]
fn features_follow_the_selected_range_while_editing() {
    let mut s = popover_sim();
    let id = s.by_name("Continue");
    // open a text session, select "Con" and set a feature through the popover
    crate::canvas::textsession::begin_text_edit(&mut s.shell.app, id);
    {
        let sess = s.shell.app.canvas.text.as_mut().unwrap();
        sess.caret = 3;
        sess.anchor = 0;
    }
    s.shell.app.props.pop = Some(crate::panels::effects::PopState {
        kind: crate::panels::effects::PopKind::TextDetails,
        anchor: Rect::from_min_size(Pos2::new(1550.0, 330.0), Vec2::splat(24.0)),
        ids: vec![id],
    });
    s.frame(vec![]);
    s.frame(vec![]);
    let case = s.rect_of("det-case");
    click_rect(&mut s, case);
    let t = text_of(&s, id);
    assert!(t.style.features.is_empty(), "the base style is untouched");
    assert_eq!(t.runs.len(), 1);
    assert_eq!((t.runs[0].start, t.runs[0].end), (0, 3));
    assert_eq!(t.runs[0].style.features, vec![("case".to_string(), 1)]);
}

#[test]
fn tab_in_a_text_session_nests_list_items_and_never_moves_egui_focus() {
    use eframe::App;
    let tab = |m: Modifiers| RawInput {
        events: vec![Event::Key { key: Key::Tab, physical_key: Some(Key::Tab), pressed: true, repeat: false, modifiers: m }],
        ..Default::default()
    };
    let mut s = Sim::new();
    s.key(Key::T);
    let p = s.screen(300.0, 820.0);
    s.click(p);
    s.type_text("a");
    let id = s.app().selection[0];
    s.cmd_key(Key::A);
    s.mods = Modifiers { command: true, shift: true, ..Default::default() };
    s.key(Key::Num8);
    s.mods = Modifiers::NONE;
    assert!(text_of(&s, id).has_lists());
    // the platform hook takes Tab out of the raw input before egui sees it
    let mut raw = tab(Modifiers::NONE);
    s.shell.raw_input_hook(&s.ctx.clone(), &mut raw);
    assert!(raw.events.is_empty(), "egui never gets the Tab press");
    s.frame(vec![]);
    assert_eq!(text_of(&s, id).paragraph_lists()[0].map(|i| i.level), Some(1), "the list item moved one level in");
    // Shift+Tab goes back out
    let mut raw = tab(Modifiers::SHIFT);
    s.shell.raw_input_hook(&s.ctx.clone(), &mut raw);
    s.frame(vec![]);
    assert_eq!(text_of(&s, id).paragraph_lists()[0].map(|i| i.level), Some(0));
    s.key(Key::Escape);
    // outside a session Tab is left alone
    let mut raw = tab(Modifiers::NONE);
    s.shell.raw_input_hook(&s.ctx.clone(), &mut raw);
    assert_eq!(raw.events.len(), 1);
}

#[test]
fn typography_header_has_no_var_pill_and_clicking_left_of_style_button_changes_nothing() {
    let mut s = Sim::with(Options { select: Some("Continue".into()), ..Options::default() });
    assert!(has_no_rect(&s, "typo-var-pill"));
    let style_btn = s.rect_of("text-style-button");
    let left_of_style = Rect::from_min_size(Pos2::new(style_btn.left() - 24.0, style_btn.top()), Vec2::splat(24.0));
    let undo_before = s.app().hist.undo_depth();
    let more_before = more_open(&s);
    let fill_top_before = s.rect_of("text-sw").top();
    assert!(!s.app().vars.open);
    assert!(!more_before);

    click_rect(&mut s, left_of_style);

    assert!(has_no_rect(&s, "typo-var-pill"));
    assert!(!s.app().vars.open, "vars.open stays false");
    assert_eq!(s.app().hist.undo_depth(), undo_before, "undo stack is unchanged");
    assert!(!s.app().hist.can_undo(), "undo stack is unchanged");
    assert_eq!(more_open(&s), more_before, "More toggle is unchanged");
    assert_eq!(s.rect_of("text-sw").top(), fill_top_before, "More rows stay collapsed");
}

#[test]
fn truncate_text_toggle_shows_mixed_and_writes_every_selected_text_layer_in_one_undo_step() {
    let mut s = popover_sim();
    let id1 = s.by_name("Continue");
    let id2 = s.by_name("Welcome back");
    let sz1 = designos_doc::Vec2::new(120.0, 36.0);
    let sz2 = designos_doc::Vec2::new(210.0, 44.0);
    {
        let n1 = s.shell.app.doc.get_mut(id1).unwrap();
        n1.size = sz1;
        if let NodeKind::Text(t) = &mut n1.kind {
            t.auto_resize = TextAutoResize::Truncate;
        }
        let n2 = s.shell.app.doc.get_mut(id2).unwrap();
        n2.size = sz2;
        if let NodeKind::Text(t) = &mut n2.kind {
            t.auto_resize = TextAutoResize::WidthAndHeight;
        }
    }
    s.shell.app.selection = vec![id1, id2];
    if let Some(pop) = s.shell.app.props.pop.as_mut() {
        pop.ids = vec![id1, id2];
    }
    s.frame(vec![]);
    s.frame(vec![]);

    let trunc_r = s.rect_of("det-truncate");
    assert!(texts_in_rect(&s, trunc_r).iter().any(|t| t == "Mixed"), "mixed selection must show Mixed on Truncate text row");
    assert!(has_no_rect(&s, "det-max-lines"), "Max lines stays hidden while Truncate text is Mixed");

    let undo_before = s.app().hist.undo_depth();
    click_rect(&mut s, trunc_r);
    assert_eq!(text_of(&s, id1).auto_resize, TextAutoResize::Truncate);
    assert_eq!(text_of(&s, id2).auto_resize, TextAutoResize::Truncate);
    assert_eq!(s.app().doc.get(id1).unwrap().size, sz1, "turning truncation on keeps id1 size");
    assert_eq!(s.app().doc.get(id2).unwrap().size, sz2, "turning truncation on keeps id2 size");
    assert!(!texts_in_rect(&s, s.rect_of("det-truncate")).iter().any(|t| t == "Mixed"));
    assert_eq!(s.app().hist.undo_depth(), undo_before + 1, "one undo step per click");

    // Clicking again when every selected layer truncates turns all off (fixed-size mode) and keeps sizes.
    let trunc_r = s.rect_of("det-truncate");
    click_rect(&mut s, trunc_r);
    assert_eq!(text_of(&s, id1).auto_resize, TextAutoResize::None);
    assert_eq!(text_of(&s, id2).auto_resize, TextAutoResize::None);
    assert_eq!(s.app().doc.get(id1).unwrap().size, sz1, "turning truncation off keeps id1 size");
    assert_eq!(s.app().doc.get(id2).unwrap().size, sz2, "turning truncation off keeps id2 size");
    assert_eq!(s.app().hist.undo_depth(), undo_before + 2, "one undo step per click");

    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id1).auto_resize, TextAutoResize::Truncate);
    assert_eq!(text_of(&s, id2).auto_resize, TextAutoResize::Truncate);
    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id1).auto_resize, TextAutoResize::Truncate);
    assert_eq!(text_of(&s, id2).auto_resize, TextAutoResize::WidthAndHeight);
}

#[test]
fn max_lines_appears_only_while_truncating() {
    let mut s = popover_sim();
    let id = s.by_name("Continue");
    assert_eq!(text_of(&s, id).auto_resize, TextAutoResize::WidthAndHeight);
    let trunc_r = s.rect_of("det-truncate");
    assert!(has_no_rect(&s, "det-max-lines"), "Max lines must not exist while Truncate text is off");
    assert!(!crate::lint::scan_shapes(&s.shapes).0.iter().any(|t| t.text == "Max lines"), "Max lines label must not paint while Truncate text is off");

    click_rect(&mut s, trunc_r);
    assert_eq!(text_of(&s, id).auto_resize, TextAutoResize::Truncate);
    let max_r = s.field_rect("det-max-lines");
    assert!(crate::lint::scan_shapes(&s.shapes).0.iter().any(|t| t.text == "Max lines"), "Max lines label paints while Truncate text is on");
    assert!((max_r.top() - s.rect_of("det-truncate").top() - crate::widgets::popover::ROW_H).abs() < 1.0, "Max lines sits directly under Truncate text");

    let trunc_r = s.rect_of("det-truncate");
    click_rect(&mut s, trunc_r);
    assert_eq!(text_of(&s, id).auto_resize, TextAutoResize::None);
    assert!(has_no_rect(&s, "det-max-lines"), "Max lines disappears when Truncate text is turned off");
    assert!(!crate::lint::scan_shapes(&s.shapes).0.iter().any(|t| t.text == "Max lines"));
}

#[test]
fn max_lines_commits_integer_at_least_one_shows_mixed_and_clearing_stores_none() {
    let mut s = popover_sim();
    let id1 = s.by_name("Continue");
    let id2 = s.by_name("Welcome back");
    {
        if let Some(NodeKind::Text(t)) = s.shell.app.doc.get_mut(id1).map(|n| &mut n.kind) {
            t.auto_resize = TextAutoResize::Truncate;
            t.max_lines = Some(2);
        }
        if let Some(NodeKind::Text(t)) = s.shell.app.doc.get_mut(id2).map(|n| &mut n.kind) {
            t.auto_resize = TextAutoResize::Truncate;
            t.max_lines = Some(4);
        }
    }
    s.shell.app.selection = vec![id1, id2];
    if let Some(pop) = s.shell.app.props.pop.as_mut() {
        pop.ids = vec![id1, id2];
    }
    s.frame(vec![]);
    s.frame(vec![]);

    let r = s.field_rect("det-max-lines");
    assert!(texts_in_rect(&s, r).iter().any(|t| t == "Mixed"), "mixed max_lines must show Mixed");

    let undo_before = s.app().hist.undo_depth();
    s.click(Pos2::new(r.left() + 30.0, r.center().y));
    s.type_text("3");
    s.key(Key::Enter);
    assert_eq!(text_of(&s, id1).max_lines, Some(3));
    assert_eq!(text_of(&s, id2).max_lines, Some(3));
    assert_eq!(s.app().hist.undo_depth(), undo_before + 1, "one undo step per commit");

    // Integer clamping at least 1
    let r = s.field_rect("det-max-lines");
    s.click(Pos2::new(r.left() + 30.0, r.center().y));
    s.type_text("0");
    s.key(Key::Enter);
    assert_eq!(text_of(&s, id1).max_lines, Some(1));
    assert_eq!(text_of(&s, id2).max_lines, Some(1));
    assert_eq!(s.app().hist.undo_depth(), undo_before + 2);

    // Clearing stores None (truncate at the box height)
    let r = s.field_rect("det-max-lines");
    s.click(Pos2::new(r.left() + 30.0, r.center().y));
    s.key(Key::Backspace);
    s.key(Key::Enter);
    assert_eq!(text_of(&s, id1).max_lines, None);
    assert_eq!(text_of(&s, id2).max_lines, None);
    assert_eq!(s.app().hist.undo_depth(), undo_before + 3);

    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id1).max_lines, Some(1));
    assert_eq!(text_of(&s, id2).max_lines, Some(1));
}

#[test]
fn setting_max_lines_clears_max_height_on_text_layer_in_same_undo_step() {
    let mut s = popover_sim();
    let id = s.by_name("Continue");
    {
        let n = s.shell.app.doc.get_mut(id).unwrap();
        n.layout_child.max_h = Some(48.0);
        n.variables.bindings.insert("maxHeight".into(), "var/max-h".into());
        if let NodeKind::Text(t) = &mut n.kind {
            t.auto_resize = TextAutoResize::Truncate;
            t.max_lines = None;
        }
    }
    s.frame(vec![]);
    s.frame(vec![]);

    let undo_before = s.app().hist.undo_depth();
    let r = s.field_rect("det-max-lines");
    s.click(Pos2::new(r.left() + 30.0, r.center().y));
    s.type_text("2");
    s.key(Key::Enter);

    assert_eq!(text_of(&s, id).max_lines, Some(2));
    assert_eq!(s.app().doc.get(id).unwrap().layout_child.max_h, None, "setting Max lines clears max_h on the text layer");
    assert!(!s.app().doc.get(id).unwrap().variables.bindings.contains_key("maxHeight"), "setting Max lines detaches maxHeight binding");
    assert_eq!(s.app().hist.undo_depth(), undo_before + 1, "cleared in the same undo step");

    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id).max_lines, None);
    assert_eq!(s.app().doc.get(id).unwrap().layout_child.max_h, Some(48.0));
    assert_eq!(s.app().doc.get(id).unwrap().variables.bindings.get("maxHeight").map(String::as_str), Some("var/max-h"));
}

/// Marks `ids` as truncating text layers with the given `max_lines` and selects them in the open popover.
fn select_truncating(s: &mut Sim, ids: &[(NodeId, Option<u32>)]) {
    for (id, max_lines) in ids {
        if let Some(NodeKind::Text(t)) = s.shell.app.doc.get_mut(*id).map(|n| &mut n.kind) {
            t.auto_resize = TextAutoResize::Truncate;
            t.max_lines = *max_lines;
        }
    }
    let selection: Vec<NodeId> = ids.iter().map(|(id, _)| *id).collect();
    s.shell.app.selection = selection.clone();
    if let Some(pop) = s.shell.app.props.pop.as_mut() {
        pop.ids = selection;
    }
    s.frame(vec![]);
    s.frame(vec![]);
}

#[test]
fn truncate_text_leaves_non_text_layers_in_the_selection_alone() {
    use designos_doc::{AutoLayout, FrameProps, LayoutDirection, Node, Sizing};
    let mut s = popover_sim();
    let text_id = s.by_name("Continue");
    let card = s.by_name("Card");
    let group = {
        let doc = &mut s.shell.app.doc;
        let hugging = AutoLayout { direction: LayoutDirection::Horizontal, primary_sizing: Sizing::Hug, counter_sizing: Sizing::Hug, ..AutoLayout::default() };
        let gid = doc.alloc_id();
        let mut g = Node::frame(gid, designos_doc::Rect::new(0.0, 0.0, 40.0, 20.0));
        g.name = "Hug group".into();
        g.kind = NodeKind::Frame(FrameProps { auto_layout: Some(hugging), ..FrameProps::default() });
        g.layout_child.horizontal = Sizing::Hug;
        g.layout_child.vertical = Sizing::Hug;
        let gid = doc.insert(card, g).expect("the card takes the hug group");
        let rid = doc.alloc_id();
        doc.insert(gid, Node::rectangle(rid, designos_doc::Rect::new(0.0, 0.0, 40.0, 20.0))).expect("the group takes its rectangle");
        doc.relayout_subtree(card);
        let t = doc.get_mut(text_id).unwrap();
        t.layout_child.horizontal = Sizing::Hug;
        t.layout_child.vertical = Sizing::Hug;
        gid
    };
    s.shell.app.selection = vec![text_id, group];
    if let Some(pop) = s.shell.app.props.pop.as_mut() {
        pop.ids = vec![text_id, group];
    }
    s.frame(vec![]);
    s.frame(vec![]);

    let hug_axes = |s: &Sim, id: NodeId| {
        let n = s.app().doc.get(id).unwrap();
        let al = n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).map(|a| (a.primary_sizing, a.counter_sizing));
        (n.layout_child, al, n.size)
    };
    let (group_child_before, group_flow_before, group_size_before) = hug_axes(&s, group);
    assert_eq!((group_child_before.horizontal, group_child_before.vertical), (Sizing::Hug, Sizing::Hug), "the sibling starts hugging on both axes");
    assert_eq!(group_flow_before, Some((Sizing::Hug, Sizing::Hug)), "the sibling's own auto layout starts hugging on both axes");
    let text_child_before = s.app().doc.get(text_id).unwrap().layout_child;
    assert_eq!((text_child_before.horizontal, text_child_before.vertical), (Sizing::Hug, Sizing::Hug), "the text layer starts hugging on both axes");

    let undo_before = s.app().hist.undo_depth();
    let trunc_r = s.rect_of("det-truncate");
    click_rect(&mut s, trunc_r);

    assert_eq!(text_of(&s, text_id).auto_resize, TextAutoResize::Truncate, "the text layer truncates");
    let text_child = s.app().doc.get(text_id).unwrap().layout_child;
    assert_eq!((text_child.horizontal, text_child.vertical), (Sizing::Fixed, Sizing::Fixed), "the text layer keeps its box by freezing hug sizing");
    assert_eq!(hug_axes(&s, group), (group_child_before, group_flow_before, group_size_before), "a non-text layer in the selection keeps its sizing");
    assert_eq!(s.app().hist.undo_depth(), undo_before + 1, "one undo step per click");
}

#[test]
fn max_lines_keeps_its_value_when_the_field_is_emptied_and_the_same_digits_are_typed_again() {
    let mut s = popover_sim();
    let id = s.by_name("Continue");
    select_truncating(&mut s, &[(id, Some(12))]);

    let undo_before = s.app().hist.undo_depth();
    let r = s.field_rect("det-max-lines");
    s.click(Pos2::new(r.left() + 30.0, r.center().y));
    s.key(Key::Backspace);
    s.type_text("12");
    s.key(Key::Enter);

    assert_eq!(text_of(&s, id).max_lines, Some(12), "retyping the same digits leaves the limit alone");
    assert_eq!(s.app().hist.undo_depth(), undo_before, "an unchanged commit adds no undo step");
}

#[test]
fn mixed_max_lines_is_not_written_when_backspace_is_followed_by_leaving_the_field() {
    let mut s = popover_sim();
    let id1 = s.by_name("Continue");
    let id2 = s.by_name("Welcome back");
    select_truncating(&mut s, &[(id1, Some(2)), (id2, Some(4))]);
    let r = s.field_rect("det-max-lines");
    assert!(texts_in_rect(&s, r).iter().any(|t| t == "Mixed"), "differing limits show Mixed");

    let undo_before = s.app().hist.undo_depth();
    s.click(Pos2::new(r.left() + 30.0, r.center().y));
    s.key(Key::Backspace);
    // a click on the empty part of the row, left of the field, takes focus away while the popover stays open
    s.click(Pos2::new(r.left() - 40.0, r.center().y));
    s.frame(vec![]);

    assert!(s.app().props.pop.is_some(), "the popover is still open after leaving the field");
    assert_eq!(text_of(&s, id1).max_lines, Some(2));
    assert_eq!(text_of(&s, id2).max_lines, Some(4));
    assert_eq!(s.app().hist.undo_depth(), undo_before, "leaving the field adds no undo step");
    assert!(texts_in_rect(&s, s.field_rect("det-max-lines")).iter().any(|t| t == "Mixed"), "the field still shows Mixed");
}

#[test]
fn turning_truncation_off_keeps_max_lines_and_turning_it_on_shows_the_old_limit() {
    let mut s = popover_sim();
    let id = s.by_name("Continue");
    select_truncating(&mut s, &[(id, Some(3))]);
    let r = s.field_rect("det-max-lines");
    assert!(texts_in_rect(&s, r).iter().any(|t| t == "3"), "the field shows the stored limit while truncating");

    let trunc_r = s.rect_of("det-truncate");
    click_rect(&mut s, trunc_r);
    assert_eq!(text_of(&s, id).auto_resize, TextAutoResize::None);
    assert_eq!(text_of(&s, id).max_lines, Some(3), "turning truncation off keeps the stored limit");
    assert!(has_no_rect(&s, "det-max-lines"), "Max lines is hidden while truncation is off");

    let trunc_r = s.rect_of("det-truncate");
    click_rect(&mut s, trunc_r);
    assert_eq!(text_of(&s, id).auto_resize, TextAutoResize::Truncate);
    assert_eq!(text_of(&s, id).max_lines, Some(3), "turning truncation back on keeps the stored limit");
    let r = s.field_rect("det-max-lines");
    assert!(texts_in_rect(&s, r).iter().any(|t| t == "3"), "the field shows the old limit again");
}

fn apply_textures_delta(atlas: &mut std::collections::HashMap<egui::TextureId, image::RgbaImage>, delta: &egui::TexturesDelta) {
    for (tid, d) in &delta.set {
        let [w, h] = d.image.size();
        let src: Vec<[u8; 4]> = match &d.image {
            egui::ImageData::Color(c) => c.pixels.iter().map(|p| p.to_array()).collect(),
        };
        if let Some([ox, oy]) = d.pos {
            if let Some(existing) = atlas.get_mut(tid) {
                for y in 0..h {
                    for x in 0..w {
                        let px = src[y * w + x];
                        if (ox + x) < existing.width() as usize && (oy + y) < existing.height() as usize {
                            existing.put_pixel((ox + x) as u32, (oy + y) as u32, image::Rgba(px));
                        }
                    }
                }
            }
        } else {
            let mut img = image::RgbaImage::new(w as u32, h as u32);
            for y in 0..h {
                for x in 0..w {
                    img.put_pixel(x as u32, y as u32, image::Rgba(src[y * w + x]));
                }
            }
            atlas.insert(*tid, img);
        }
    }
}

fn rasterize_shapes(
    ctx: &egui::Context,
    shapes: Vec<egui::epaint::ClippedShape>,
    atlas: &std::collections::HashMap<egui::TextureId, image::RgbaImage>,
    width: u32,
    height: u32,
) -> image::RgbaImage {
    let mut fb = image::RgbaImage::from_pixel(width, height, image::Rgba([0x18, 0x18, 0x1B, 0xFF]));
    let prims = ctx.tessellate(shapes, 1.0);
    for cp in prims {
        let egui::epaint::Primitive::Mesh(mesh) = cp.primitive else { continue };
        let clip_min_x = cp.clip_rect.min.x.floor().max(0.0) as i32;
        let clip_min_y = cp.clip_rect.min.y.floor().max(0.0) as i32;
        let clip_max_x = cp.clip_rect.max.x.ceil().min(width as f32) as i32;
        let clip_max_y = cp.clip_rect.max.y.ceil().min(height as f32) as i32;
        let tex = atlas.get(&mesh.texture_id);
        for tri in mesh.indices.chunks_exact(3) {
            let v0 = &mesh.vertices[tri[0] as usize];
            let v1 = &mesh.vertices[tri[1] as usize];
            let v2 = &mesh.vertices[tri[2] as usize];
            let min_x = v0.pos.x.min(v1.pos.x).min(v2.pos.x).floor().max(clip_min_x as f32) as i32;
            let max_x = v0.pos.x.max(v1.pos.x).max(v2.pos.x).ceil().min(clip_max_x as f32) as i32;
            let min_y = v0.pos.y.min(v1.pos.y).min(v2.pos.y).floor().max(clip_min_y as f32) as i32;
            let max_y = v0.pos.y.max(v1.pos.y).max(v2.pos.y).ceil().min(clip_max_y as f32) as i32;
            let denom = (v1.pos.y - v2.pos.y) * (v0.pos.x - v2.pos.x) + (v2.pos.x - v1.pos.x) * (v0.pos.y - v2.pos.y);
            if denom.abs() < 1e-6 {
                continue;
            }
            let inv_denom = 1.0 / denom;
            let c0 = v0.color.to_array().map(|c| c as f32);
            let c1 = v1.color.to_array().map(|c| c as f32);
            let c2 = v2.color.to_array().map(|c| c as f32);
            for py in min_y..max_y {
                for px in min_x..max_x {
                    let p = Pos2::new(px as f32 + 0.5, py as f32 + 0.5);
                    let w0 = ((v1.pos.y - v2.pos.y) * (p.x - v2.pos.x) + (v2.pos.x - v1.pos.x) * (p.y - v2.pos.y)) * inv_denom;
                    let w1 = ((v2.pos.y - v0.pos.y) * (p.x - v2.pos.x) + (v0.pos.x - v2.pos.x) * (p.y - v2.pos.y)) * inv_denom;
                    let w2 = 1.0 - w0 - w1;
                    if w0 >= -1e-4 && w1 >= -1e-4 && w2 >= -1e-4 {
                        let mut col = [0.0f32; 4];
                        for k in 0..4 {
                            col[k] = (w0 * c0[k] + w1 * c1[k] + w2 * c2[k]).clamp(0.0, 255.0);
                        }
                        if let Some(t) = tex {
                            let u = (w0 * v0.uv.x + w1 * v1.uv.x + w2 * v2.uv.x).clamp(0.0, 1.0);
                            let v = (w0 * v0.uv.y + w1 * v1.uv.y + w2 * v2.uv.y).clamp(0.0, 1.0);
                            let tx = ((u * t.width() as f32).floor() as u32).min(t.width().saturating_sub(1));
                            let ty = ((v * t.height() as f32).floor() as u32).min(t.height().saturating_sub(1));
                            let tp = t.get_pixel(tx, ty).0;
                            for k in 0..4 {
                                col[k] *= tp[k] as f32 / 255.0;
                            }
                        }
                        let src_a = col[3] / 255.0;
                        if src_a > 0.0 {
                            let dst = fb.get_pixel(px as u32, py as u32).0;
                            let inv_a = 1.0 - src_a;
                            let out_px = [
                                (col[0] + dst[0] as f32 * inv_a).round().clamp(0.0, 255.0) as u8,
                                (col[1] + dst[1] as f32 * inv_a).round().clamp(0.0, 255.0) as u8,
                                (col[2] + dst[2] as f32 * inv_a).round().clamp(0.0, 255.0) as u8,
                                255,
                            ];
                            fb.put_pixel(px as u32, py as u32, image::Rgba(out_px));
                        }
                    }
                }
            }
        }
    }
    fb
}

#[test]
fn headless_popover_render_truncate_off_and_on_max_lines_2() {
    let mut s = Sim::built(Options { select: Some("Continue".into()), panel: Some("type-details".into()), ..Options::default() });
    let mut atlas = std::collections::HashMap::new();
    let run = |s: &mut Sim, atlas: &mut std::collections::HashMap<egui::TextureId, image::RgbaImage>| {
        s.t += 0.05;
        let mut raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 1088.0))),
            time: Some(s.t),
            modifiers: s.mods,
            ..Default::default()
        };
        let ctx = s.ctx.clone();
        eframe::App::raw_input_hook(&mut s.shell, &ctx, &mut raw);
        let out = ctx.run_ui(raw, |ui| s.shell.draw(ui));
        apply_textures_delta(atlas, &out.textures_delta);
        s.shapes = out.shapes;
    };
    run(&mut s, &mut atlas);
    run(&mut s, &mut atlas);

    let id = s.by_name("Continue");
    {
        let n = s.shell.app.doc.get_mut(id).unwrap();
        n.size = designos_doc::Vec2::new(140.0, 44.0);
        if let NodeKind::Text(t) = &mut n.kind {
            t.content = "The quick brown fox jumps over the lazy dog and keeps running across the field.".into();
            t.auto_resize = TextAutoResize::None;
            t.max_lines = None;
        }
    }
    run(&mut s, &mut atlas);
    run(&mut s, &mut atlas);

    let (_, boxes_off) = crate::lint::scan_shapes(&s.shapes);
    let pop_off =
        *boxes_off.iter().find(|b| (b.width() - 312.0).abs() < 1.0 && b.height() > 700.0).expect("Type settings popover rect when Truncate text is off");
    let img_off = rasterize_shapes(&s.ctx, s.shapes.clone(), &atlas, 1600, 1088);
    let crop_off =
        image::imageops::crop_imm(&img_off, pop_off.min.x.max(0.0) as u32, pop_off.min.y.max(0.0) as u32, pop_off.width() as u32, pop_off.height() as u32)
            .to_image();

    // Turn on Truncate text and set Max lines = 2.
    {
        let n = s.shell.app.doc.get_mut(id).unwrap();
        if let NodeKind::Text(t) = &mut n.kind {
            t.auto_resize = TextAutoResize::Truncate;
            t.max_lines = Some(2);
        }
    }
    run(&mut s, &mut atlas);
    run(&mut s, &mut atlas);

    let (_, boxes_on) = crate::lint::scan_shapes(&s.shapes);
    let pop_on = *boxes_on.iter().find(|b| (b.width() - 312.0).abs() < 1.0 && b.height() > 700.0).expect("Type settings popover rect when Truncate text is on");
    assert!((pop_on.height() - pop_off.height() - crate::widgets::popover::ROW_H).abs() < 1.0, "popover grows by 1 row when Max lines is shown");

    let img_on = rasterize_shapes(&s.ctx, s.shapes.clone(), &atlas, 1600, 1088);
    let crop_on = image::imageops::crop_imm(&img_on, pop_on.min.x.max(0.0) as u32, pop_on.min.y.max(0.0) as u32, pop_on.width() as u32, pop_on.height() as u32)
        .to_image();

    if let Ok(ev_dir) = std::env::var("DESIGNOS_1467_EVIDENCE_DIR") {
        let ev = std::path::Path::new(&ev_dir);
        let _ = std::fs::create_dir_all(ev);
        crop_off.save(ev.join("type-settings-truncate-off-crop.png")).expect("save off crop");
        crop_on.save(ev.join("type-settings-truncate-on-max-lines-2-crop.png")).expect("save on crop");
        if let Ok(full_dir) = std::env::var("DESIGNOS_1467_FULL_DIR") {
            let fd = std::path::Path::new(&full_dir);
            let _ = std::fs::create_dir_all(fd);
            img_off.save(fd.join("1467-popover-truncate-off.png")).expect("save full off");
            img_on.save(fd.join("1467-popover-truncate-on-max-lines-2.png")).expect("save full on");
        }
    }
}
