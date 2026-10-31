//! Sim tests for issue #1026: editing a style-owned Typography field of a linked text layer
//! detaches the text style in the same undo step, while non-style-owned fields (alignment, fill)
//! preserve the link, and undo/redo restore and reapply both the value and the link.

use super::*;
use crate::theme;
use designos_doc::{LineHeight, PaintKind, TextAlignH};

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
fn font_weight_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo() {
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
    assert_eq!(link_of(&s, id), None, "editing font weight must detach the text style");
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "weight edit + style detach must be one undo step");

    s.cmd_key(Key::Z);
    assert_eq!(text_of(&s, id).style.font_weight, 400, "undo restores font weight");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "undo restores text style link");

    s.cmd_shift_key(Key::Z);
    assert_eq!(text_of(&s, id).style.font_weight, 700, "redo reapplies font weight");
    assert_eq!(link_of(&s, id), None, "redo detaches text style link again");
}

#[test]
fn font_style_italic_edit_detaches_linked_text_style_in_one_undo_step_with_undo_and_redo() {
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
    assert_eq!(link_of(&s, id), None, "editing font style (italic) must detach the text style");
    assert_eq!(s.app().hist.undo_depth(), depth_before_italic + 1, "italic edit + style detach must be one undo step");

    s.cmd_key(Key::Z);
    assert!(!text_of(&s, id).style.italic, "undo restores italic = false");
    assert_eq!(link_of(&s, id).as_deref(), Some("Body/Regular"), "undo restores text style link");

    s.cmd_shift_key(Key::Z);
    assert!(text_of(&s, id).style.italic, "redo reapplies italic = true");
    assert_eq!(link_of(&s, id), None, "redo detaches text style link again");
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
