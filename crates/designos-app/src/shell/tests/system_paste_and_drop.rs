//! What other apps put on the system clipboard, and image files dropped on the canvas, become layers.
//! The clipboard is scripted through `App::system_html`, `App::system_text` and `App::system_image`, never the OS
//! pasteboard. Cmd+V sequences follow what egui-winit really emits: a text paste event on the press when the
//! clipboard has text, and only the V release when it holds an image alone (see `figma_paste.rs`).

use super::*;
use crate::panels::menus::{self, Action};
use crate::system_paste::ClipImage;
use designos_doc::{PaintKind, Vec2 as DV};
use std::collections::BTreeSet;

type Ids = BTreeSet<NodeId>;

fn solid(width: u32, height: u32) -> ClipImage {
    let rgba = (0..width * height).flat_map(|i| [(i % 251) as u8, 40, 200, 255]).collect();
    ClipImage { width, height, rgba }
}

fn wide_image() -> Option<ClipImage> {
    Some(solid(800, 600))
}

fn small_image() -> Option<ClipImage> {
    Some(solid(120, 80))
}

/// An image whose pixel buffer is shorter than its size says.
fn torn_image() -> Option<ClipImage> {
    Some(ClipImage { width: 10, height: 10, rgba: vec![0; 7] })
}

fn some_words() -> Option<String> {
    Some("Release notes for Friday".into())
}

fn an_image_url() -> Option<String> {
    Some("https://example.com/photo.png".into())
}

fn only_blanks() -> Option<String> {
    Some(" \n\t ".into())
}

fn figma_payload() -> Option<String> {
    Some(crate::clipboard::tests::figma_html(&designos_fig::testkit::sample_fig(designos_fig::container::Compression::Zstd, true)))
}

fn ids(s: &Sim) -> Ids {
    s.app().doc.nodes.keys().copied().collect()
}

fn added(s: &Sim, before: &Ids) -> Vec<NodeId> {
    s.app().doc.nodes.keys().filter(|k| !before.contains(k)).copied().collect()
}

fn named(s: &Sim, name: &str) -> usize {
    s.app().doc.nodes.values().filter(|n| n.name == name).count()
}

fn toast(s: &Sim) -> Option<(String, bool)> {
    s.app().toast.as_ref().map(|t| (t.text.clone(), t.error))
}

fn key_v_release() -> Event {
    Event::Key { key: Key::V, physical_key: Some(Key::V), pressed: false, repeat: false, modifiers: Modifiers::COMMAND }
}

/// One real Cmd+V: the press frame carries the clipboard's plain text (when it has some), the release frame the V release.
fn ctrl_v(s: &mut Sim, text: Option<&str>) {
    s.mods = Modifiers::COMMAND;
    s.frame(text.map(|t| vec![Event::Paste(t.into())]).unwrap_or_default());
    s.frame(vec![key_v_release()]);
    s.mods = Modifiers::NONE;
}

fn edit_paste(s: &mut Sim) {
    let ctx = s.ctx.clone();
    menus::run(&mut s.shell.app, &ctx, Action::Paste);
    s.shell.app.settle_paste_jobs();
}

fn viewport_center(s: &Sim) -> DV {
    let a = s.app();
    a.view.to_page(a.canvas_rect.min, a.canvas_rect.center())
}

fn center_of(s: &Sim, id: NodeId) -> DV {
    let d = &s.app().doc;
    let n = d.get(id).unwrap();
    d.world_transform(id).apply(DV::new(n.size.x / 2.0, n.size.y / 2.0))
}

fn assert_centered_on_viewport(s: &Sim, id: NodeId) {
    let (c, v) = (center_of(s, id), viewport_center(s));
    assert!((c.x - v.x).abs() <= 1.0 && (c.y - v.y).abs() <= 1.0, "center ({}, {}) vs viewport center ({}, {})", c.x, c.y, v.x, v.y);
}

/// The one new layer, which must be an image layer; returns its id and the decoded size of the embedded picture.
fn the_image_layer(s: &Sim, before: &Ids) -> (NodeId, (u32, u32)) {
    let new = added(s, before);
    assert_eq!(new.len(), 1, "one new layer, got {new:?}");
    let n = s.app().doc.get(new[0]).unwrap();
    assert!(matches!(n.kind, NodeKind::Rectangle), "{:?}", n.kind);
    let PaintKind::Image { image_ref, .. } = &n.fills[0].kind else { panic!("not an image fill: {:?}", n.fills) };
    let bytes = s.app().doc.images.get(image_ref).expect("the picture is embedded");
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).expect("embedded as PNG");
    assert_eq!(s.app().selection, vec![new[0]], "the new layer is selected");
    (new[0], (img.width(), img.height()))
}

fn undoes_in_one_step(s: &mut Sim, before: &Ids) {
    s.cmd_key(Key::Z);
    assert_eq!(&ids(s), before, "one undo takes the paste away");
}

// ---------------------------------------------------------------------------------------------- text

#[test]
fn plain_text_from_another_app_becomes_one_text_layer_at_the_viewport_center() {
    let mut s = Sim::new();
    let before = ids(&s);
    ctrl_v(&mut s, Some("Release notes for Friday"));
    let new = added(&s, &before);
    assert_eq!(new.len(), 1, "{new:?}");
    let n = s.app().doc.get(new[0]).unwrap();
    let NodeKind::Text(t) = &n.kind else { panic!("not a text layer: {:?}", n.kind) };
    assert_eq!(t.content, "Release notes for Friday");
    assert_eq!(n.name, "Release notes for Friday");
    assert_eq!(s.app().selection, new);
    assert_centered_on_viewport(&s, new[0]);
    undoes_in_one_step(&mut s, &before);
}

#[test]
fn edit_paste_reads_the_system_text_for_a_text_layer() {
    let mut s = Sim::new();
    s.shell.app.system_text = some_words;
    // an in-app copy made earlier is older than the words on the system clipboard
    let button = s.by_name("Button");
    s.shell.app.selection = vec![button];
    s.shell.app.copy_selection().expect("copied");
    let (before, buttons) = (ids(&s), named(&s, "Button"));
    edit_paste(&mut s);
    let new = added(&s, &before);
    assert_eq!(new.len(), 1, "{new:?}");
    assert!(matches!(&s.app().doc.get(new[0]).unwrap().kind, NodeKind::Text(t) if t.content == "Release notes for Friday"));
    assert_eq!(named(&s, "Button"), buttons, "the stale in-app layer did not come back");
    undoes_in_one_step(&mut s, &before);
}

#[test]
fn long_text_wraps_at_400px_instead_of_running_off_the_screen() {
    let mut s = Sim::new();
    let before = ids(&s);
    let long = "A sentence that goes on and on without any line break, ".repeat(12);
    ctrl_v(&mut s, Some(&long));
    let new = added(&s, &before);
    assert_eq!(new.len(), 1);
    let n = s.app().doc.get(new[0]).unwrap();
    assert!((n.size.x - 400.0).abs() < 0.5, "width {}", n.size.x);
    assert!(n.size.y > 40.0, "it wraps onto several lines: height {}", n.size.y);
    assert_centered_on_viewport(&s, new[0]);
}

#[test]
fn text_made_only_of_whitespace_pastes_nothing_and_never_the_stale_copy() {
    let mut s = Sim::new();
    let button = s.by_name("Button");
    s.shell.app.selection = vec![button];
    s.shell.app.copy_selection().expect("copied");
    let (before, buttons) = (ids(&s), named(&s, "Button"));
    ctrl_v(&mut s, Some(" \n\t "));
    assert_eq!(ids(&s), before);
    assert_eq!(named(&s, "Button"), buttons);
    assert_eq!(toast(&s), Some(("Nothing to paste".into(), false)));
    s.shell.app.system_text = only_blanks;
    s.shell.app.toast = None;
    edit_paste(&mut s);
    assert_eq!(ids(&s), before);
    assert_eq!(toast(&s), Some(("Nothing to paste".into(), false)));
}

#[test]
fn two_pastes_in_one_frame_land_in_order() {
    let mut s = Sim::new();
    let before = ids(&s);
    s.mods = Modifiers::COMMAND;
    s.frame(vec![Event::Paste("first".into()), Event::Paste("second".into())]);
    s.mods = Modifiers::NONE;
    let page = s.app().doc.get(s.app().page).unwrap().children.clone();
    let order: Vec<String> = page.iter().filter(|id| !before.contains(id)).map(|id| s.app().doc.get(*id).unwrap().name.clone()).collect();
    assert_eq!(order, vec!["first", "second"]);
}

// ---------------------------------------------------------------------------------------------- images

#[test]
fn an_image_alone_on_the_clipboard_becomes_one_image_layer_capped_at_400px() {
    let mut s = Sim::new();
    s.shell.app.system_image = wide_image;
    let before = ids(&s);
    // egui-winit sends no text paste event for an image-only clipboard: only the V release shows the chord
    ctrl_v(&mut s, None);
    let (id, picture) = the_image_layer(&s, &before);
    assert_eq!(picture, (800, 600), "the full picture is embedded");
    let size = s.app().doc.get(id).unwrap().size;
    assert_eq!((size.x, size.y), (400.0, 300.0), "400 px on the longest side, like the Image tool");
    assert_centered_on_viewport(&s, id);
    undoes_in_one_step(&mut s, &before);
}

#[test]
fn a_small_image_keeps_its_own_size() {
    let mut s = Sim::new();
    s.shell.app.system_image = small_image;
    let before = ids(&s);
    ctrl_v(&mut s, None);
    let (id, _) = the_image_layer(&s, &before);
    let size = s.app().doc.get(id).unwrap().size;
    assert_eq!((size.x, size.y), (120.0, 80.0));
}

#[test]
fn an_image_wins_over_the_url_a_browser_puts_beside_it() {
    let mut s = Sim::new();
    s.shell.app.system_image = wide_image;
    let before = ids(&s);
    ctrl_v(&mut s, Some("https://example.com/photo.png"));
    let (id, _) = the_image_layer(&s, &before);
    assert!(matches!(s.app().doc.get(id).unwrap().kind, NodeKind::Rectangle));
    assert!(!s.app().doc.nodes.values().any(|n| matches!(&n.kind, NodeKind::Text(t) if t.content.starts_with("https://"))), "the URL is not pasted as text");
    undoes_in_one_step(&mut s, &before);
}

#[test]
fn edit_paste_pastes_the_clipboard_image_too() {
    let mut s = Sim::new();
    s.shell.app.system_image = wide_image;
    s.shell.app.system_text = an_image_url;
    let before = ids(&s);
    edit_paste(&mut s);
    let (id, picture) = the_image_layer(&s, &before);
    assert_eq!(picture, (800, 600));
    assert_centered_on_viewport(&s, id);
    undoes_in_one_step(&mut s, &before);
}

#[test]
fn a_newer_system_image_beats_the_stale_in_app_copy_by_ctrl_v_and_edit_paste() {
    let mut s = Sim::new();
    let button = s.by_name("Button");
    s.shell.app.selection = vec![button];
    s.shell.app.copy_selection().expect("copied");
    s.shell.app.system_image = small_image;
    let (before, buttons) = (ids(&s), named(&s, "Button"));
    ctrl_v(&mut s, None);
    the_image_layer(&s, &before);
    assert_eq!(named(&s, "Button"), buttons);
    let after_first = ids(&s);
    edit_paste(&mut s);
    assert_eq!(added(&s, &after_first).len(), 1);
    assert_eq!(named(&s, "Button"), buttons);
}

#[test]
fn a_figma_payload_and_svg_markup_win_over_an_image_beside_them() {
    let mut s = Sim::new();
    s.shell.app.system_image = wide_image;
    s.shell.app.system_html = figma_payload;
    let before = ids(&s);
    ctrl_v(&mut s, None);
    let names: Vec<String> = added(&s, &before).iter().map(|id| s.app().doc.get(*id).unwrap().name.clone()).collect();
    assert!(names.iter().any(|n| n == "Button"), "the Figma layers came in: {names:?}");
    assert!(!names.iter().any(|n| n == "Pasted image"), "{names:?}");

    let mut s = Sim::new();
    s.shell.app.system_image = wide_image;
    let before = ids(&s);
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><rect width="20" height="20" fill="#336699"/></svg>"##;
    ctrl_v(&mut s, Some(svg));
    let new = added(&s, &before);
    assert!(!new.is_empty() && !new.iter().any(|id| s.app().doc.get(*id).unwrap().name == "Pasted image"), "vector markup stays vector");
}

#[test]
fn an_image_that_cannot_be_encoded_says_so_and_adds_nothing() {
    let mut s = Sim::new();
    s.shell.app.system_image = torn_image;
    let before = ids(&s);
    ctrl_v(&mut s, None);
    assert_eq!(ids(&s), before);
    assert_eq!(toast(&s), Some(("Cannot paste image".into(), true)));
}

// ------------------------------------------------------------------------- nothing usable, and the in-app copy

#[test]
fn nothing_on_the_clipboard_and_nothing_copied_says_so() {
    let mut s = Sim::new();
    let before = ids(&s);
    ctrl_v(&mut s, None);
    assert_eq!(ids(&s), before);
    assert_eq!(toast(&s), Some(("Nothing to paste".into(), false)));
    s.shell.app.toast = None;
    edit_paste(&mut s);
    assert_eq!(ids(&s), before);
    assert_eq!(toast(&s), Some(("Nothing to paste".into(), false)));
}

#[test]
fn when_the_system_clipboard_cannot_be_read_the_in_app_copy_still_pastes() {
    let mut s = Sim::new();
    let button = s.by_name("Button");
    s.shell.app.selection = vec![button];
    s.shell.app.copy_selection().expect("copied");
    let buttons = named(&s, "Button");
    edit_paste(&mut s);
    assert_eq!(named(&s, "Button"), buttons + 1);
    ctrl_v(&mut s, None);
    assert_eq!(named(&s, "Button"), buttons + 2);
}
