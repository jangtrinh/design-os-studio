//! Sim tests for issue #1026: editing a style-owned Typography field of a linked text layer
//! detaches the text style in the same undo step, while non-style-owned fields (alignment, fill)
//! preserve the link, and undo/redo restore and reapply both the value and the link.

use super::*;
use crate::theme;
use designos_doc::{LeadingTrim, LineHeight, PaintKind, TextAlignH, TextDecoration};

impl Sim {
    fn seed_families(&self) {
        let gen = designos_render::font_generation();
        let list = std::sync::Arc::new(vec!["Inter".to_string(), "TestLato".to_string()]);
        self.ctx.data_mut(|d| d.insert_temp(egui::Id::new("typo-families"), (gen, list)));
    }

    fn cmd_shift_key(&mut self, key: Key) {
        let is_mac = self.shell.app.terminal.platform_is_mac_override.unwrap_or(cfg!(target_os = "macos"));
        self.mods = Modifiers { alt: false, ctrl: !is_mac, shift: true, mac_cmd: is_mac, command: true };
        self.key(key);
        self.mods = Modifiers::NONE;
    }

    fn cmd_alt_key(&mut self, key: Key) {
        let is_mac = self.shell.app.terminal.platform_is_mac_override.unwrap_or(cfg!(target_os = "macos"));
        self.mods = Modifiers { alt: true, ctrl: !is_mac, shift: false, mac_cmd: is_mac, command: true };
        self.key(key);
        self.mods = Modifiers::NONE;
    }

    fn cmd_alt_shift_key(&mut self, key: Key) {
        let is_mac = self.shell.app.terminal.platform_is_mac_override.unwrap_or(cfg!(target_os = "macos"));
        self.mods = Modifiers { alt: true, ctrl: !is_mac, shift: true, mac_cmd: is_mac, command: true };
        self.key(key);
        self.mods = Modifiers::NONE;
    }

    fn alt_key(&mut self, key: Key) {
        self.mods = Modifiers { alt: true, ..Modifiers::NONE };
        self.key(key);
        self.mods = Modifiers::NONE;
    }
}

fn link_of(s: &Sim, id: NodeId) -> Option<String> {
    s.app().doc.get(id).unwrap().style_refs.get("text").cloned()
}

/// Selects `"Continue"` and links it to the demo file's `"Body/Regular"` text style via the Typography style menu.
fn linked_sim() -> (Sim, NodeId) {
    let mut s = Sim::with(Options { select: Some("Continue".into()), ..Options::default() });
    s.seed_families();
    let id = s.by_name("Continue");
    let btn = s.rect_of("text-style-button");
    s.click(btn.center());
    // Row 0 is the "Body" folder header; row 1 is "Body/Regular" (Inter, 400, 16px, line-height 24px, letter-spacing 0px).
    let row = s.rect_of("menu-row-0-1");
    s.click(row.center());
    s.frame(vec![]);
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"));
    assert!(s.rect_of("text-style-row").is_positive(), "linked style row must be visible");
    (s, id)
}

#[test]
fn font_family_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo() {
    let (mut s, id) = linked_sim();
    let depth = s.app().hist.undo_depth();

    s.seed_families();
    let fam_r = s.field_rect("font-family");
    s.click(fam_r.center());
    // In the open family picker popup, row 0 is "Inter" and row 1 is "TestLato".
    let row1 = Pos2::new(fam_r.center().x, fam_r.bottom() + 2.0 + 8.0 + theme::FIELD_HEIGHT + 4.0 + theme::MENU_ITEM_HEIGHT * 1.5);
    s.click(row1);

    assert_eq!(text_of(&s, id).style.font_family, "TestLato");
    assert_eq!(link_of(&s, id), None, "editing font family must detach the text style");
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "family edit + style detach must be one undo step");

    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id).style.font_family, "Inter", "undo restores font family");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "undo restores text style link");

    s.cmd_shift_key(Key::Z);
    assert_eq!(text_of(&s, id).style.font_family, "TestLato", "redo reapplies font family");
    assert_eq!(link_of(&s, id), None, "redo detaches text style link again");
}

#[test]
fn font_weight_edit_keeps_linked_text_style_as_an_override_in_one_undo_step_with_undo_and_redo() {
    let (mut s, id) = linked_sim();
    let depth = s.app().hist.undo_depth();

    // Font weight dropdown (col 0 on the same row as font-size)
    let fam_r = s.field_rect("font-family");
    let size_r = s.field_rect("font-size");
    let weight_pos = Pos2::new(fam_r.left() + 24.0, size_r.center().y);
    s.click(weight_pos);
    // Index 6 in WEIGHTS is ("Bold", 700).
    let bold_row = s.rect_of("menu-row-0-6");
    s.click(bold_row.center());

    assert_eq!(text_of(&s, id).style.font_weight, 700);
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "editing font weight keeps the text style: it is an override");
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "weight edit + override record must be one undo step");

    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id).style.font_weight, 400, "undo restores font weight");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "undo restores text style link");

    s.cmd_shift_key(Key::Z);
    assert_eq!(text_of(&s, id).style.font_weight, 700, "redo reapplies font weight");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "redo keeps the text style link");
}

#[test]
fn font_style_italic_edit_keeps_linked_text_style_as_an_override_in_one_undo_step_with_undo_and_redo() {
    let (mut s, id) = linked_sim();
    let depth_before_italic = s.app().hist.undo_depth();

    let fam_r = s.field_rect("font-family");
    let style_btn = s.rect_of("text-style-button");
    let more_btn = Pos2::new(style_btn.right() + theme::SPACE_FACT + theme::PANEL_ACTION_BUTTON + 2.0 + theme::PANEL_ACTION_BUTTON * 0.5, style_btn.center().y);
    s.click(more_btn);
    let lh_r = s.field_rect("line-height");
    // Row 4 is alignment (+28px), row 5 is decoration/case (+28px), row 6 is Normal/Italic (+28px); Italic is segment 1 (x + 48..96).
    let italic_pos = Pos2::new(fam_r.left() + 48.0 + 24.0, lh_r.bottom() + 4.0 + 28.0 * 2.0 + theme::FIELD_HEIGHT * 0.5);
    s.click(italic_pos);

    assert!(text_of(&s, id).style.italic, "clicking Italic sets italic = true");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "editing font style (italic) keeps the text style: it is an override");
    assert_eq!(s.app().hist.undo_depth(), depth_before_italic + 1, "italic edit + override record must be one undo step");

    s.cmd_key(Key::Z);
    assert!(!text_of(&s, id).style.italic, "undo restores italic = false");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "undo restores text style link");

    s.cmd_shift_key(Key::Z);
    assert!(text_of(&s, id).style.italic, "redo reapplies italic = true");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "redo keeps the text style link");
}

#[test]
fn font_size_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo() {
    let (mut s, id) = linked_sim();
    let depth = s.app().hist.undo_depth();

    let r = s.field_rect("font-size");
    s.click(Pos2::new(r.left() + 30.0, r.center().y));
    s.type_text("24");
    s.key(Key::Enter);

    assert_eq!(text_of(&s, id).style.font_size, 24.0);
    assert_eq!(link_of(&s, id), None, "editing font size must detach the text style");
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "size edit + style detach must be one undo step");

    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id).style.font_size, 16.0, "undo restores font size");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "undo restores text style link");

    s.cmd_shift_key(Key::Z);
    assert_eq!(text_of(&s, id).style.font_size, 24.0, "redo reapplies font size");
    assert_eq!(link_of(&s, id), None, "redo detaches text style link again");
}

#[test]
fn line_height_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo() {
    let (mut s, id) = linked_sim();
    let depth = s.app().hist.undo_depth();

    let r = s.field_rect("line-height");
    s.click(Pos2::new(r.left() + 30.0, r.center().y));
    s.type_text("32");
    s.key(Key::Enter);

    assert_eq!(text_of(&s, id).style.line_height, LineHeight::Pixels(32.0));
    assert_eq!(link_of(&s, id), None, "editing line height must detach the text style");
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "line-height edit + style detach must be one undo step");

    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id).style.line_height, LineHeight::Pixels(24.0), "undo restores line height");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "undo restores text style link");

    s.cmd_shift_key(Key::Z);
    assert_eq!(text_of(&s, id).style.line_height, LineHeight::Pixels(32.0), "redo reapplies line height");
    assert_eq!(link_of(&s, id), None, "redo detaches text style link again");
}

#[test]
fn letter_spacing_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo() {
    let (mut s, id) = linked_sim();
    let depth = s.app().hist.undo_depth();

    let r = s.field_rect("letter-spacing");
    s.click(Pos2::new(r.left() + 30.0, r.center().y));
    s.type_text("1.5");
    s.key(Key::Enter);

    assert_eq!(text_of(&s, id).style.letter_spacing, 1.5);
    assert_eq!(link_of(&s, id), None, "editing letter spacing must detach the text style");
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "letter-spacing edit + style detach must be one undo step");

    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id).style.letter_spacing, 0.0, "undo restores letter spacing");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "undo restores text style link");

    s.cmd_shift_key(Key::Z);
    assert_eq!(text_of(&s, id).style.letter_spacing, 1.5, "redo reapplies letter spacing");
    assert_eq!(link_of(&s, id), None, "redo detaches text style link again");
}

#[test]
fn alignment_and_fill_edits_keep_the_link_while_style_owned_edit_detaches() {
    let (mut s, id) = linked_sim();

    // 1. Alignment (Center is segment 1 in `text-align`)
    let fam_r = s.field_rect("font-family");
    let lh_r = s.field_rect("line-height");
    let align_center = Pos2::new(fam_r.left() + theme::SEGMENT_W * 1.5, lh_r.bottom() + 4.0 + theme::FIELD_HEIGHT * 0.5);
    s.click(align_center);
    assert_eq!(text_of(&s, id).style.align_h, TextAlignH::Center);
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "alignment edit must keep the text style link");

    // 2. Fill colour (`text-hex`) and opacity (`text-op`)
    let hex_r = s.rect_of("text-hex");
    s.click(hex_r.center());
    s.type_text("2563EB");
    s.key(Key::Enter);
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "fill colour edit must keep the text style link");
    let fill = &text_of(&s, id).style.fills[0];
    assert_eq!(fill.kind, PaintKind::Solid(Color::rgb(0x25, 0x63, 0xEB)));

    let op_r = s.field_rect("text-op");
    s.click(op_r.center());
    s.type_text("80");
    s.key(Key::Enter);
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "fill opacity edit must keep the text style link");
    assert!((text_of(&s, id).style.fills[0].opacity - 0.8).abs() < 1e-4);

    // 3. Subsequent style-owned edit detaches the link; undo restores the link while preserving alignment & fill
    let size_r = s.field_rect("font-size");
    s.click(Pos2::new(size_r.left() + 30.0, size_r.center().y));
    s.type_text("20");
    s.key(Key::Enter);
    assert_eq!(text_of(&s, id).style.font_size, 20.0);
    assert_eq!(link_of(&s, id), None, "style-owned edit after alignment/fill must detach the link");

    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id).style.font_size, 16.0);
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "undoing the size edit restores the link");
    assert_eq!(text_of(&s, id).style.align_h, TextAlignH::Center, "alignment edit remains intact");
    assert_eq!(text_of(&s, id).style.fills[0].kind, PaintKind::Solid(Color::rgb(0x25, 0x63, 0xEB)));
}

#[test]
fn active_text_session_typography_edit_detaches_while_alignment_and_fill_keep_link() {
    let (mut s, id) = linked_sim();
    crate::canvas::textsession::begin_text_edit(&mut s.shell.app, id);
    s.frame(vec![]);
    s.frame(vec![]);

    // Alignment and fill inside an active text session keep the link
    let fam_r = s.field_rect("font-family");
    let lh_r = s.field_rect("line-height");
    let align_right = Pos2::new(fam_r.left() + theme::SEGMENT_W * 2.5, lh_r.bottom() + 4.0 + theme::FIELD_HEIGHT * 0.5);
    s.click(align_right);
    assert_eq!(text_of(&s, id).style.align_h, TextAlignH::Right);
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "in-session alignment edit must keep the link");

    let hex_r = s.rect_of("text-hex");
    s.click(hex_r.center());
    s.type_text("10B981");
    s.key(Key::Enter);
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "in-session fill edit must keep the link");

    // Style-owned edit inside the active text session detaches the link in the same session undo step
    let size_r = s.field_rect("font-size");
    s.click(Pos2::new(size_r.left() + 30.0, size_r.center().y));
    s.type_text("22");
    s.key(Key::Enter);
    assert_eq!(text_of(&s, id).style.font_size, 22.0);
    assert_eq!(link_of(&s, id), None, "in-session font-size edit must detach the text style");

    // In-session undo/redo restores and reapplies both value and link
    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id).style.font_size, 16.0);
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "in-session undo restores the text style link");

    s.cmd_shift_key(Key::Z);
    assert_eq!(text_of(&s, id).style.font_size, 22.0);
    assert_eq!(link_of(&s, id), None, "in-session redo detaches the text style link again");
}

#[test]
fn cmd_b_outside_session_keeps_linked_text_style_as_an_override_in_one_undo_step_with_undo_and_redo() {
    let (mut s, id) = linked_sim();
    let depth = s.app().hist.undo_depth();

    s.cmd_key(Key::B);
    assert_eq!(text_of(&s, id).style.font_weight, 700);
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "Cmd+B outside a session keeps the text style: it is an override");
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "Cmd+B + override record must be one undo step");

    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id).style.font_weight, 400, "undo restores font weight");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "undo restores text style link");

    s.cmd_shift_key(Key::Z);
    assert_eq!(text_of(&s, id).style.font_weight, 700, "redo reapplies font weight");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "redo keeps the text style link");
}

#[test]
fn cmd_b_in_session_keeps_linked_text_style_as_an_override_in_one_undo_step_with_undo_and_redo() {
    let (mut s, id) = linked_sim();
    let depth = s.app().hist.undo_depth();
    crate::canvas::textsession::begin_text_edit(&mut s.shell.app, id);
    s.frame(vec![]);
    s.frame(vec![]);

    s.cmd_key(Key::B);
    assert_eq!(text_of(&s, id).style.font_weight, 700);
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "Cmd+B in a session keeps the text style: it is an override");

    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id).style.font_weight, 400, "in-session undo restores font weight");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "in-session undo restores text style link");
    assert!(!s.app().canvas.text.as_ref().unwrap().log.can_undo(), "Cmd+B in session is one session undo step");

    s.cmd_shift_key(Key::Z);
    assert_eq!(text_of(&s, id).style.font_weight, 700, "in-session redo reapplies font weight");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "in-session redo keeps the text style link");

    s.key(Key::Escape);
    s.frame(vec![]);
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "committing session adds one document undo step");

    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id).style.font_weight, 400, "document undo restores font weight");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "document undo restores text style link");
}

#[test]
fn cmd_i_outside_session_keeps_linked_text_style_as_an_override_in_one_undo_step_with_undo_and_redo() {
    let (mut s, id) = linked_sim();
    let depth = s.app().hist.undo_depth();

    s.cmd_key(Key::I);
    assert!(text_of(&s, id).style.italic);
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "Cmd+I outside a session keeps the text style: it is an override");
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "Cmd+I + override record must be one undo step");

    s.cmd_key(Key::Z);
    assert!(!text_of(&s, id).style.italic, "undo restores italic");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "undo restores text style link");

    s.cmd_shift_key(Key::Z);
    assert!(text_of(&s, id).style.italic, "redo reapplies italic");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "redo keeps the text style link");
}

#[test]
fn cmd_i_in_session_keeps_linked_text_style_as_an_override_in_one_undo_step_with_undo_and_redo() {
    let (mut s, id) = linked_sim();
    let depth = s.app().hist.undo_depth();
    crate::canvas::textsession::begin_text_edit(&mut s.shell.app, id);
    s.frame(vec![]);
    s.frame(vec![]);

    s.cmd_key(Key::I);
    assert!(text_of(&s, id).style.italic);
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "Cmd+I in a session keeps the text style: it is an override");

    s.cmd_key(Key::Z);
    assert!(!text_of(&s, id).style.italic, "in-session undo restores italic");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "in-session undo restores text style link");
    assert!(!s.app().canvas.text.as_ref().unwrap().log.can_undo(), "Cmd+I in session is one session undo step");

    s.cmd_shift_key(Key::Z);
    assert!(text_of(&s, id).style.italic, "in-session redo reapplies italic");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "in-session redo keeps the text style link");

    s.key(Key::Escape);
    s.frame(vec![]);
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "committing session adds one document undo step");

    s.cmd_key(Key::Z);
    assert!(!text_of(&s, id).style.italic, "document undo restores italic");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "document undo restores text style link");
}

#[test]
fn cmd_u_outside_session_keeps_linked_text_style_as_an_override_in_one_undo_step_with_undo_and_redo() {
    let (mut s, id) = linked_sim();
    let depth = s.app().hist.undo_depth();

    s.cmd_key(Key::U);
    assert_eq!(text_of(&s, id).style.decoration, TextDecoration::Underline);
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "Cmd+U outside a session keeps the text style: it is an override");
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "Cmd+U + override record must be one undo step");

    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id).style.decoration, TextDecoration::None, "undo restores decoration");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "undo restores text style link");

    s.cmd_shift_key(Key::Z);
    assert_eq!(text_of(&s, id).style.decoration, TextDecoration::Underline, "redo reapplies decoration");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "redo keeps the text style link");
}

#[test]
fn cmd_u_in_session_keeps_linked_text_style_as_an_override_in_one_undo_step_with_undo_and_redo() {
    let (mut s, id) = linked_sim();
    let depth = s.app().hist.undo_depth();
    crate::canvas::textsession::begin_text_edit(&mut s.shell.app, id);
    s.frame(vec![]);
    s.frame(vec![]);

    s.cmd_key(Key::U);
    assert_eq!(text_of(&s, id).style.decoration, TextDecoration::Underline);
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "Cmd+U in a session keeps the text style: it is an override");

    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id).style.decoration, TextDecoration::None, "in-session undo restores decoration");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "in-session undo restores text style link");
    assert!(!s.app().canvas.text.as_ref().unwrap().log.can_undo(), "Cmd+U in session is one session undo step");

    s.cmd_shift_key(Key::Z);
    assert_eq!(text_of(&s, id).style.decoration, TextDecoration::Underline, "in-session redo reapplies decoration");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "in-session redo keeps the text style link");

    s.key(Key::Escape);
    s.frame(vec![]);
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "committing session adds one document undo step");

    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id).style.decoration, TextDecoration::None, "document undo restores decoration");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "document undo restores text style link");
}

#[test]
fn menu_increase_font_size_outside_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo() {
    let (mut s, id) = linked_sim();
    let depth = s.app().hist.undo_depth();

    let ctx = s.ctx.clone();
    crate::panels::menus::run(&mut s.shell.app, &ctx, crate::panels::menus::Action::TextSizeUp);
    s.frame(vec![]);
    assert_eq!(text_of(&s, id).style.font_size, 17.0);
    assert_eq!(link_of(&s, id), None, "Text menu > Increase font size outside a session must detach the text style");
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "menu size up + style detach must be one undo step");

    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id).style.font_size, 16.0, "undo restores font size");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "undo restores text style link");

    s.cmd_shift_key(Key::Z);
    assert_eq!(text_of(&s, id).style.font_size, 17.0, "redo reapplies font size");
    assert_eq!(link_of(&s, id), None, "redo detaches text style link again");
}

#[test]
fn menu_increase_font_size_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo() {
    let (mut s, id) = linked_sim();
    let depth = s.app().hist.undo_depth();
    crate::canvas::textsession::begin_text_edit(&mut s.shell.app, id);
    s.frame(vec![]);
    s.frame(vec![]);

    let ctx = s.ctx.clone();
    crate::panels::menus::run(&mut s.shell.app, &ctx, crate::panels::menus::Action::TextSizeUp);
    s.frame(vec![]);
    assert_eq!(text_of(&s, id).style.font_size, 17.0);
    assert_eq!(link_of(&s, id), None, "Text menu > Increase font size in a session must detach the text style");

    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id).style.font_size, 16.0, "in-session undo restores font size");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "in-session undo restores text style link");
    assert!(!s.app().canvas.text.as_ref().unwrap().log.can_undo(), "menu size up in session is one session undo step");

    s.cmd_shift_key(Key::Z);
    assert_eq!(text_of(&s, id).style.font_size, 17.0, "in-session redo reapplies font size");
    assert_eq!(link_of(&s, id), None, "in-session redo detaches text style link again");

    s.key(Key::Escape);
    s.frame(vec![]);
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "committing session adds one document undo step");

    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id).style.font_size, 16.0, "document undo restores font size");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "document undo restores text style link");
}

fn open_type_details(s: &mut Sim) {
    let style_btn = s.rect_of("text-style-button");
    let details_btn = Pos2::new(style_btn.right() + theme::SPACE_FACT + theme::PANEL_ACTION_BUTTON * 0.5, style_btn.center().y);
    s.click(details_btn);
}

#[test]
fn paragraph_spacing_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo() {
    let (mut s, id) = linked_sim();
    open_type_details(&mut s);
    let depth = s.app().hist.undo_depth();

    let r = s.field_rect("det-pspacing");
    s.click(Pos2::new(r.left() + 30.0, r.center().y));
    s.type_text("12");
    s.key(Key::Enter);

    assert_eq!(text_of(&s, id).style.paragraph_spacing, 12.0);
    assert_eq!(link_of(&s, id), None, "editing paragraph spacing must detach the text style");
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "paragraph spacing edit + style detach must be one undo step");

    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id).style.paragraph_spacing, 0.0, "undo restores paragraph spacing");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "undo restores text style link");

    s.cmd_shift_key(Key::Z);
    assert_eq!(text_of(&s, id).style.paragraph_spacing, 12.0, "redo reapplies paragraph spacing");
    assert_eq!(link_of(&s, id), None, "redo detaches text style link again");
}

#[test]
fn paragraph_indent_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo() {
    let (mut s, id) = linked_sim();
    open_type_details(&mut s);
    let depth = s.app().hist.undo_depth();

    let r = s.field_rect("det-indent");
    s.click(Pos2::new(r.left() + 30.0, r.center().y));
    s.type_text("8");
    s.key(Key::Enter);

    assert_eq!(text_of(&s, id).paragraph.indent, 8.0);
    assert_eq!(link_of(&s, id), None, "editing paragraph indent must detach the text style");
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "paragraph indent edit + style detach must be one undo step");

    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id).paragraph.indent, 0.0, "undo restores paragraph indent");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "undo restores text style link");

    s.cmd_shift_key(Key::Z);
    assert_eq!(text_of(&s, id).paragraph.indent, 8.0, "redo reapplies paragraph indent");
    assert_eq!(link_of(&s, id), None, "redo detaches text style link again");
}

#[test]
fn list_spacing_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo() {
    let (mut s, id) = linked_sim();
    open_type_details(&mut s);
    let depth = s.app().hist.undo_depth();

    let r = s.field_rect("det-lspacing");
    s.click(Pos2::new(r.left() + 30.0, r.center().y));
    s.type_text("6");
    s.key(Key::Enter);

    assert_eq!(text_of(&s, id).paragraph.list_spacing, 6.0);
    assert_eq!(link_of(&s, id), None, "editing list spacing must detach the text style");
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "list spacing edit + style detach must be one undo step");

    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id).paragraph.list_spacing, 0.0, "undo restores list spacing");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "undo restores text style link");

    s.cmd_shift_key(Key::Z);
    assert_eq!(text_of(&s, id).paragraph.list_spacing, 6.0, "redo reapplies list spacing");
    assert_eq!(link_of(&s, id), None, "redo detaches text style link again");
}

#[test]
fn vertical_trim_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo() {
    let (mut s, id) = linked_sim();
    open_type_details(&mut s);
    let depth = s.app().hist.undo_depth();

    let lspacing = s.field_rect("det-lspacing");
    let trim_pos = lspacing.center() + Vec2::new(0.0, 3.0 * crate::widgets::popover::ROW_H);
    s.click(trim_pos);
    let cap_row = s.rect_of("menu-row-0-1");
    s.click(cap_row.center());

    assert_eq!(text_of(&s, id).paragraph.leading_trim, LeadingTrim::CapHeight);
    assert_eq!(link_of(&s, id), None, "editing vertical trim must detach the text style");
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "vertical trim edit + style detach must be one undo step");

    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id).paragraph.leading_trim, LeadingTrim::None, "undo restores vertical trim");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "undo restores text style link");

    s.cmd_shift_key(Key::Z);
    assert_eq!(text_of(&s, id).paragraph.leading_trim, LeadingTrim::CapHeight, "redo reapplies vertical trim");
    assert_eq!(link_of(&s, id), None, "redo detaches text style link again");
}

#[test]
fn repicking_same_font_family_outside_session_keeps_linked_text_style() {
    let (mut s, id) = linked_sim();
    let depth = s.app().hist.undo_depth();

    s.seed_families();
    let fam_r = s.field_rect("font-family");
    s.click(fam_r.center());
    // Row 0 in the open family picker popup is "Inter" (the already-active family of Body/Regular).
    let row0 = Pos2::new(fam_r.center().x, fam_r.bottom() + 2.0 + 8.0 + theme::FIELD_HEIGHT + 4.0 + theme::MENU_ITEM_HEIGHT * 0.5);
    s.click(row0);

    assert_eq!(text_of(&s, id).style.font_family, "Inter");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "re-picking the same font family outside a session must keep the text style link");
    assert_eq!(s.app().hist.undo_depth(), depth, "no-op family repick must not push an undo step");
}

#[test]
fn repicking_same_font_family_in_session_keeps_linked_text_style() {
    let (mut s, id) = linked_sim();
    crate::canvas::textsession::begin_text_edit(&mut s.shell.app, id);
    s.frame(vec![]);
    s.frame(vec![]);
    assert!(!s.app().canvas.text.as_ref().unwrap().range().is_empty(), "session selection covers the text");

    s.seed_families();
    let fam_r = s.field_rect("font-family");
    s.click(fam_r.center());
    let row0 = Pos2::new(fam_r.center().x, fam_r.bottom() + 2.0 + 8.0 + theme::FIELD_HEIGHT + 4.0 + theme::MENU_ITEM_HEIGHT * 0.5);
    s.click(row0);

    assert_eq!(text_of(&s, id).style.font_family, "Inter");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "re-picking the same font family in a session must keep the text style link");
    assert!(!s.app().canvas.text.as_ref().unwrap().log.can_undo(), "no-op family repick in a session must not record a session undo step");
}

#[test]
fn paragraph_spacing_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo() {
    let (mut s, id) = linked_sim();
    let depth = s.app().hist.undo_depth();
    crate::canvas::textsession::begin_text_edit(&mut s.shell.app, id);
    s.frame(vec![]);
    s.frame(vec![]);
    open_type_details(&mut s);

    let r = s.field_rect("det-pspacing");
    s.click(Pos2::new(r.left() + 30.0, r.center().y));
    s.type_text("12");
    s.key(Key::Enter);

    assert_eq!(text_of(&s, id).style.paragraph_spacing, 12.0);
    assert_eq!(link_of(&s, id), None, "editing paragraph spacing in a session must detach the text style");

    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id).style.paragraph_spacing, 0.0, "in-session undo restores paragraph spacing");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "in-session undo restores text style link");
    assert!(!s.app().canvas.text.as_ref().unwrap().log.can_undo(), "paragraph spacing edit in session is one session undo step");

    s.cmd_shift_key(Key::Z);
    assert_eq!(text_of(&s, id).style.paragraph_spacing, 12.0, "in-session redo reapplies paragraph spacing");
    assert_eq!(link_of(&s, id), None, "in-session redo detaches text style link again");

    s.key(Key::Escape);
    s.frame(vec![]);
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "committing session adds one document undo step");

    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id).style.paragraph_spacing, 0.0, "document undo restores paragraph spacing");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "document undo restores text style link");
}

#[test]
fn paragraph_indent_in_session_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo() {
    let (mut s, id) = linked_sim();
    let depth = s.app().hist.undo_depth();
    crate::canvas::textsession::begin_text_edit(&mut s.shell.app, id);
    s.frame(vec![]);
    s.frame(vec![]);
    open_type_details(&mut s);

    let r = s.field_rect("det-indent");
    s.click(Pos2::new(r.left() + 30.0, r.center().y));
    s.type_text("8");
    s.key(Key::Enter);

    assert_eq!(text_of(&s, id).paragraph.indent, 8.0);
    assert_eq!(link_of(&s, id), None, "editing paragraph indent in a session must detach the text style");

    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id).paragraph.indent, 0.0, "in-session undo restores paragraph indent");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "in-session undo restores text style link");
    assert!(!s.app().canvas.text.as_ref().unwrap().log.can_undo(), "paragraph indent edit in session is one session undo step");

    s.cmd_shift_key(Key::Z);
    assert_eq!(text_of(&s, id).paragraph.indent, 8.0, "in-session redo reapplies paragraph indent");
    assert_eq!(link_of(&s, id), None, "in-session redo detaches text style link again");

    s.key(Key::Escape);
    s.frame(vec![]);
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "committing session adds one document undo step");

    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id).paragraph.indent, 0.0, "document undo restores paragraph indent");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "document undo restores text style link");
}

#[test]
fn run_merge_into_base_on_cmd_i_keeps_linked_text_style_as_an_override() {
    let (mut s, id) = linked_sim();
    s.shell.app.edit_nodes(&[id], |n| {
        if let NodeKind::Text(t) = &mut n.kind {
            t.style.italic = true;
            let mut rs = t.style.clone();
            rs.italic = false;
            t.runs = vec![designos_doc::TextRun { start: 0, end: 3, style: rs }];
        }
    });
    s.frame(vec![]);
    let depth = s.app().hist.undo_depth();

    s.cmd_key(Key::I);
    assert!(text_of(&s, id).style.italic);
    assert!(text_of(&s, id).runs.is_empty(), "making the non-italic run italic merges it into the italic base");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "merging a run into the base via Cmd+I only changes italic: the style stays");
    assert_eq!(s.app().hist.undo_depth(), depth + 1);

    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id).runs.len(), 1);
    assert!(!text_of(&s, id).runs[0].style.italic);
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "undo restores the run and text style link");
}

#[test]
fn run_only_italic_change_on_cmd_i_keeps_linked_text_style() {
    let (mut s, id) = linked_sim();
    s.shell.app.edit_nodes(&[id], |n| {
        if let NodeKind::Text(t) = &mut n.kind {
            t.style.italic = true;
            let mut rs = t.style.clone();
            rs.italic = false;
            rs.font_weight = 700;
            t.runs = vec![designos_doc::TextRun { start: 0, end: 3, style: rs }];
        }
    });
    s.frame(vec![]);
    let depth = s.app().hist.undo_depth();

    s.cmd_key(Key::I);
    assert!(text_of(&s, id).style.italic);
    assert_eq!(text_of(&s, id).runs.len(), 1);
    assert!(text_of(&s, id).runs[0].style.italic, "Cmd+I makes the bold non-italic run italic");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "italic on a single run is an override: the style stays");
    assert_eq!(s.app().hist.undo_depth(), depth + 1);

    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id).runs.len(), 1);
    assert!(!text_of(&s, id).runs[0].style.italic);
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "undo restores the run style and text style link");
}

#[test]
fn the_override_pill_sits_inside_the_style_field_inset_from_its_right_edge_and_centred() {
    let (mut s, id) = linked_sim();
    let recorded = |s: &Sim| s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", "text-style-overrides".to_string())))).is_some();
    assert!(!recorded(&s), "no pill without an override");
    s.cmd_key(Key::B);
    s.cmd_key(Key::U);
    s.frame(vec![]);
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"));
    let (row, pill) = (s.rect_of("text-style-row"), s.rect_of("text-style-overrides"));
    assert!(pill.is_positive(), "bold and underline show a count pill");
    assert!((row.right() - theme::INSET_CONTROL - pill.right()).abs() < 0.5, "pill {pill:?} sits INSET_CONTROL from the field {row:?} right edge");
    assert!((pill.center().y - row.center().y).abs() < 0.5, "pill is centred on the field vertically");
    assert!(row.contains_rect(pill), "pill is inside the field");
}

#[test]
fn the_capture_preset_leaves_a_linked_text_with_two_overrides_on_the_built_in_document() {
    let mut s = Sim::with(Options { select: Some("Continue".into()), ..Options::default() });
    let id = s.by_name("Continue");
    crate::panels::properties::preset_popup(&mut s.shell.app, "text-style-overrides:row", egui::vec2(1440.0, 900.0));
    s.frame(vec![]);
    s.frame(vec![]);
    let doc = &s.app().doc;
    assert_eq!(designos_doc::text_style_overrides(doc, doc.get(id).unwrap()).labels(), vec!["Weight", "Decoration"], "the pill reads 2");
    assert!(link_of(&s, id).is_some());
}

#[test]
fn mixed_text_and_sticky_selection_shortcuts_edit_both_and_detach_linked_text_style() {
    let (mut s, id) = linked_sim();
    let mut page = id;
    while let Some(p) = s.app().doc.get(page).and_then(|n| n.parent) {
        page = p;
    }
    let sid = s.shell.app.doc.alloc_id();
    let mut sp = designos_doc::StickyProps::default();
    sp.text.content = "note".into();
    let mut n = designos_doc::Node::new(sid, NodeKind::Sticky(sp));
    n.size = designos_doc::Vec2 { x: 200.0, y: 200.0 };
    s.shell.app.doc.insert(page, n).unwrap();
    s.shell.app.selection = vec![id, sid];
    s.frame(vec![]);

    let sticky_style = |s: &Sim| match &s.app().doc.get(sid).unwrap().kind {
        NodeKind::Sticky(p) => (p.text.style.font_size, p.text.style.letter_spacing),
        _ => unreachable!(),
    };
    let depth = s.app().hist.undo_depth();
    assert_eq!(sticky_style(&s), (32.0, 0.0));
    assert_eq!(text_of(&s, id).style.font_size, 16.0);

    s.cmd_shift_key(Key::Period);
    assert_eq!(text_of(&s, id).style.font_size, 17.0);
    assert_eq!(sticky_style(&s), (33.0, 0.0), "Cmd+Shift+. on a [Text, Sticky] selection must also step the sticky's font size");
    assert_eq!(link_of(&s, id), None, "Cmd+Shift+. must detach the linked Text node's style");
    assert_eq!(s.app().hist.undo_depth(), depth + 1);

    s.mods = Modifiers { alt: true, ..Modifiers::NONE };
    s.key(Key::ArrowRight);
    s.mods = Modifiers::NONE;
    assert_eq!(text_of(&s, id).style.letter_spacing, 0.5);
    assert_eq!(sticky_style(&s), (33.0, 0.5), "Alt+ArrowRight on a [Text, Sticky] selection must also step the sticky's letter spacing");
    assert_eq!(s.app().hist.undo_depth(), depth + 2);
}

#[test]
fn set_style_fill_change_outside_session_keeps_linked_text_style() {
    let (mut s, id) = linked_sim();
    let depth = s.app().hist.undo_depth();
    let red = Paint::solid(Color::rgb(255, 0, 0));

    crate::panels::typography::set_style(&mut s.shell.app, &[id], |st| st.fills = vec![red.clone()]);
    s.frame(vec![]);

    assert_eq!(text_of(&s, id).style.fills, vec![red]);
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "fill change via set_style outside a session must keep the text style link");
    assert_eq!(s.app().hist.undo_depth(), depth + 1);
}

#[test]
fn set_style_fill_change_in_session_keeps_linked_text_style() {
    let (mut s, id) = linked_sim();
    crate::canvas::textsession::begin_text_edit(&mut s.shell.app, id);
    s.frame(vec![]);
    s.frame(vec![]);
    let red = Paint::solid(Color::rgb(255, 0, 0));

    crate::panels::typography::set_style(&mut s.shell.app, &[id], |st| st.fills = vec![red.clone()]);
    s.frame(vec![]);

    assert_eq!(text_of(&s, id).style.fills, vec![red]);
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "fill change via set_style in a session must keep the text style link");
    assert!(s.app().canvas.text.as_ref().unwrap().log.can_undo(), "fill change in a session records a session undo step");
}

#[test]
fn cmd_shift_period_in_a_text_session_changes_font_size_once() {
    let (mut s, id) = linked_sim();
    crate::canvas::textsession::begin_text_edit(&mut s.shell.app, id);
    s.frame(vec![]);
    s.frame(vec![]);

    assert_eq!(text_of(&s, id).style.font_size, 16.0);
    s.cmd_shift_key(Key::Period);

    assert_eq!(text_of(&s, id).style.font_size, 17.0, "one Cmd+Shift+. event must be owned by exactly one text-session handler");
}

#[test]
fn cmd_shift_comma_in_a_text_session_changes_font_size_once() {
    let (mut s, id) = linked_sim();
    crate::canvas::textsession::begin_text_edit(&mut s.shell.app, id);
    s.frame(vec![]);
    s.frame(vec![]);

    assert_eq!(text_of(&s, id).style.font_size, 16.0);
    s.cmd_shift_key(Key::Comma);

    assert_eq!(text_of(&s, id).style.font_size, 15.0, "one Cmd+Shift+, event must be owned by exactly one text-session handler");
}

/// Opens a text session on the linked label and lets the frame settle.
fn linked_session_sim() -> (Sim, NodeId) {
    let (mut s, id) = linked_sim();
    crate::canvas::textsession::begin_text_edit(&mut s.shell.app, id);
    s.frame(vec![]);
    s.frame(vec![]);
    (s, id)
}

#[test]
fn cmd_alt_l_in_a_text_session_aligns_the_text_left() {
    let (mut s, id) = linked_session_sim();
    crate::canvas::textsession::base_style(&mut s.shell.app, |st| st.align_h = TextAlignH::Right);
    assert_eq!(text_of(&s, id).style.align_h, TextAlignH::Right);

    s.cmd_alt_key(Key::L);

    assert_eq!(text_of(&s, id).style.align_h, TextAlignH::Left, "the alignment chord must reach the open text session");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "alignment is not style-owned, so the link stays");
}

#[test]
fn alt_arrow_up_in_a_text_session_steps_the_line_height_once() {
    let (mut s, id) = linked_session_sim();
    assert_eq!(text_of(&s, id).style.line_height, LineHeight::Pixels(24.0));

    s.alt_key(Key::ArrowUp);

    assert_eq!(text_of(&s, id).style.line_height, LineHeight::Pixels(25.0), "one Alt+Up chord steps the line height by one pixel");
}

#[test]
fn alt_arrow_right_in_a_text_session_steps_the_letter_spacing_once() {
    let (mut s, id) = linked_session_sim();
    assert_eq!(text_of(&s, id).style.letter_spacing, 0.0);

    s.alt_key(Key::ArrowRight);

    assert_eq!(text_of(&s, id).style.letter_spacing, 0.5, "one Alt+Right chord steps the letter spacing by half a pixel");
}

#[test]
fn cmd_alt_shift_period_in_a_text_session_steps_the_size_by_ten_once() {
    let (mut s, id) = linked_session_sim();
    assert_eq!(text_of(&s, id).style.font_size, 16.0);

    s.cmd_alt_shift_key(Key::Period);

    assert_eq!(text_of(&s, id).style.font_size, 26.0, "one Cmd+Alt+Shift+. chord steps the size by ten, once");
}

#[test]
fn cmd_alt_shift_comma_in_a_text_session_steps_the_size_down_by_ten_once() {
    let (mut s, id) = linked_session_sim();
    assert_eq!(text_of(&s, id).style.font_size, 16.0);

    s.cmd_alt_shift_key(Key::Comma);

    assert_eq!(text_of(&s, id).style.font_size, 6.0, "one Cmd+Alt+Shift+, chord steps the size down by ten, once");
}

/// A page-level text linked to Body/Regular, selected, with the Scale tool armed.
fn linked_page_text(s: &mut Sim) -> NodeId {
    s.frame(vec![]);
    let center = {
        let app = s.app();
        app.view.to_page(app.canvas_rect.min, app.canvas_rect.center())
    };
    let page = s.app().page;
    let nid = s.shell.app.doc.alloc_id();
    let mut n = designos_doc::Node::text(
        nid,
        designos_doc::Rect::new(center.x - 80.0, center.y - 16.0, 160.0, 32.0),
        "Hello",
        designos_doc::TextStyle { font_family: "Inter".into(), font_size: 16.0, ..Default::default() },
    );
    n.style_refs.insert("text".into(), "Body/Regular".into());
    if let designos_doc::NodeKind::Text(t) = &mut n.kind {
        t.auto_resize = designos_doc::TextAutoResize::None;
    }
    let id = s.shell.app.doc.insert(page, n).unwrap();
    s.shell.app.selection = vec![id];
    s.shell.app.tool = Tool::Scale;
    s.shell.app.changed();
    s.frame(vec![]);
    id
}

fn corner_on_screen(s: &Sim, corner: usize) -> Pos2 {
    let (corners, _) = crate::canvas::interact::selection_box(s.app()).unwrap();
    s.app().view.to_screen(s.app().canvas_rect.min, corners[corner])
}

#[test]
fn k_scale_on_linked_text_detaches_in_one_undo_step() {
    let (mut s, _) = linked_sim();
    let id = linked_page_text(&mut s);
    let before = text_of(&s, id).style.font_size;
    let depth = s.app().hist.undo_depth();
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"));

    let br = corner_on_screen(&s, 2);
    s.drag(br, br + Vec2::new(160.0, 32.0));

    let scaled = text_of(&s, id).style.font_size;
    assert!(scaled > before + 1.0, "K scale changes the font size: {before} -> {scaled}");
    assert_eq!(link_of(&s, id), None, "a scaled font size detaches the text style");
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "the size change and the detach are one undo step");

    s.cmd_key(Key::Z);
    assert!((text_of(&s, id).style.font_size - before).abs() < 0.01, "undo restores the font size");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "undo restores the text style link");
}

#[test]
fn paste_properties_carries_the_source_text_style_link() {
    let (mut s, target) = linked_sim();
    let before_size = text_of(&s, target).style.font_size;
    s.shell.app.doc.styles.insert(
        "Heading".into(),
        designos_doc::Style::Text(designos_doc::TextStyle { font_family: "Inter".into(), font_size: 32.0, font_weight: 400, ..Default::default() }),
    );
    let page = s.app().page;
    let sid = s.shell.app.doc.alloc_id();
    let mut src = designos_doc::Node::text(
        sid,
        designos_doc::Rect::new(20.0, 20.0, 160.0, 40.0),
        "Source",
        designos_doc::TextStyle { font_family: "Inter".into(), font_size: 32.0, font_weight: 700, ..Default::default() },
    );
    src.style_refs.insert("text".into(), "Heading".into());
    src.text_style_overrides.insert(designos_doc::OwnedField::Weight);
    let sid = s.shell.app.doc.insert(page, src).unwrap();

    s.shell.app.selection = vec![sid];
    s.shell.app.copy_properties();
    s.shell.app.selection = vec![target];
    let depth = s.app().hist.undo_depth();
    s.shell.app.paste_properties();

    assert_eq!(link_of(&s, target).as_deref(), Some("Heading"), "paste gives the target the source's text style link");
    assert!((text_of(&s, target).style.font_size - 32.0).abs() < 0.01);
    assert_eq!(text_of(&s, target).style.font_weight, 700);
    let labels = designos_doc::text_style_overrides(&s.app().doc, s.app().doc.get(target).unwrap()).labels();
    assert!(labels.contains(&"Weight"), "the source weight override travels with the link: {labels:?}");
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "the pasted link and values are one undo step");

    s.cmd_key(Key::Z);
    assert_eq!(link_of(&s, target).as_deref(), Some("Body/Regular"), "undo restores the target's link");
    assert!((text_of(&s, target).style.font_size - before_size).abs() < 0.01, "undo restores the target's size");
}

fn unlinked_source(s: &mut Sim, style: designos_doc::TextStyle) -> NodeId {
    let page = s.app().page;
    let sid = s.shell.app.doc.alloc_id();
    let src = designos_doc::Node::text(sid, designos_doc::Rect::new(20.0, 400.0, 120.0, 24.0), "Src", style);
    s.shell.app.doc.insert(page, src).unwrap()
}

#[test]
fn paste_properties_from_unlinked_source_detaches_only_when_a_style_value_changes() {
    let (mut s, target) = linked_sim();
    let base = text_of(&s, target).style.clone();

    let mut fill = base.clone();
    fill.fills = vec![Paint::solid(Color::rgb(255, 0, 0))];
    let sid = unlinked_source(&mut s, fill);
    s.shell.app.selection = vec![sid];
    s.shell.app.copy_properties();
    s.shell.app.selection = vec![target];
    s.shell.app.paste_properties();
    assert_eq!(text_of(&s, target).style.fills, vec![Paint::solid(Color::rgb(255, 0, 0))]);
    assert_eq!(link_of(&s, target).as_deref(), Some("Body/Regular"), "a fill is not a text-style value, so the link stays");
    s.cmd_key(Key::Z);

    let mut weight = text_of(&s, target).style.clone();
    weight.font_weight = 700;
    let sid = unlinked_source(&mut s, weight);
    s.shell.app.selection = vec![sid];
    s.shell.app.copy_properties();
    s.shell.app.selection = vec![target];
    s.shell.app.paste_properties();
    assert_eq!(text_of(&s, target).style.font_weight, 700);
    assert_eq!(link_of(&s, target).as_deref(), Some("Body/Regular"), "weight stays linked as an override");
    assert!(
        designos_doc::text_style_overrides(&s.app().doc, s.app().doc.get(target).unwrap()).labels().contains(&"Weight"),
        "the pasted weight is recorded as an override"
    );
    s.cmd_key(Key::Z);

    let mut sized = text_of(&s, target).style.clone();
    sized.font_size = 40.0;
    let sid = unlinked_source(&mut s, sized);
    s.shell.app.selection = vec![sid];
    s.shell.app.copy_properties();
    s.shell.app.selection = vec![target];
    if let Some(tp) = s.shell.app.doc.get_mut(target).and_then(|n| n.kind.any_text_mut()) {
        tp.auto_resize = designos_doc::TextAutoResize::WidthAndHeight;
    }
    let before = s.app().doc.get(target).unwrap().size;
    let depth = s.app().hist.undo_depth();
    s.shell.app.paste_properties();
    assert!((text_of(&s, target).style.font_size - 40.0).abs() < 0.01);
    let pasted = s.app().doc.get(target).unwrap().size;
    assert!(pasted.x > before.x, "an auto-size text widens with the pasted 40px size ({before:?} -> {pasted:?})");
    assert_eq!(pasted, crate::layout::fit_text(&text_of(&s, target), pasted), "the text box is refit to the pasted size");
    assert_eq!(link_of(&s, target), None, "a pasted size is a style value, so the link goes");
    assert_eq!(s.app().hist.undo_depth(), depth + 1);

    s.cmd_key(Key::Z);
    assert_eq!(link_of(&s, target).as_deref(), Some("Body/Regular"), "undo restores the link");
    assert!((text_of(&s, target).style.font_size - base.font_size).abs() < 0.01, "undo restores the size");
}

#[test]
fn repicking_the_same_family_keeps_the_text_style_link() {
    let (mut s, id) = linked_sim();
    let family = text_of(&s, id).style.font_family.clone();
    s.shell.app.edit_nodes(&[id], |n| {
        if let designos_doc::NodeKind::Text(t) = &mut n.kind {
            let end = t.content.len().min(4);
            t.runs = vec![designos_doc::TextRun { start: 0, end, style: t.style.clone() }];
            n.size = crate::layout::fit_text(t, n.size);
        }
    });
    let depth = s.app().hist.undo_depth();
    let runs = text_of(&s, id).runs.clone();
    assert!(!runs.is_empty(), "the layer keeps a run that repeats the base style");

    crate::panels::typography::pick_family(&mut s.shell.app, &[id], &family);

    assert_eq!(text_of(&s, id).style.font_family, family);
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "the same family changes no style value");
    assert_eq!(s.app().hist.undo_depth(), depth, "a re-pick that changes nothing records no history step");
    assert_eq!(text_of(&s, id).runs, runs, "the redundant run stays");

    crate::canvas::textsession::begin_text_edit(&mut s.shell.app, id);
    s.frame(vec![]);
    s.frame(vec![]);
    crate::panels::typography::pick_family(&mut s.shell.app, &[id], &family);

    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"));
    assert!(!s.app().canvas.text.as_ref().unwrap().log.can_undo(), "the same re-pick inside a session records no session step");
    assert_eq!(text_of(&s, id).runs, runs);
}
