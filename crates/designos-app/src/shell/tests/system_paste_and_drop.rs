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
    // PNG and JPEG are embedded as they came, WebP is converted to PNG
    let img = image::load_from_memory(bytes).expect("an embedded picture the renderer can decode");
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

// ---------------------------------------------------------------------------------------------- dropped files

fn tmp(name: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("designos_drop_test_{}_{name}", std::process::id()));
    p
}

fn encoded(format: image::ImageFormat, w: u32, h: u32) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    let img = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(w, h, image::Rgba([220, 80, 40, 255])));
    // JPEG has no alpha channel
    let img = if format == image::ImageFormat::Jpeg { image::DynamicImage::ImageRgb8(img.to_rgb8()) } else { img };
    img.write_to(&mut out, format).expect("encode");
    out.into_inner()
}

fn write_file(name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let p = tmp(name);
    std::fs::write(&p, bytes).unwrap();
    p
}

fn dropped(path: &std::path::Path) -> egui::DroppedFile {
    egui::DroppedFile { path: Some(path.to_path_buf()), ..Default::default() }
}

impl Sim {
    /// One frame with synthetic dropped files; the pointer rests where the last move left it.
    fn drop_on_window(&mut self, files: Vec<egui::DroppedFile>) {
        self.t += 0.05;
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 1088.0))),
            time: Some(self.t),
            dropped_files: files,
            modifiers: self.mods,
            ..Default::default()
        };
        let ctx = self.ctx.clone();
        let shell = &mut self.shell;
        let _ = ctx.run_ui(raw, |ui| shell.draw(ui));
        self.frame(vec![]);
    }
}

/// The image layer a drop made: id and the decoded size of its embedded picture.
fn image_layers(s: &Sim, before: &Ids) -> Vec<NodeId> {
    added(s, before)
        .into_iter()
        .filter(|id| matches!(s.app().doc.get(*id).map(|n| n.fills.first().map(|f| &f.kind)), Some(Some(PaintKind::Image { .. }))))
        .collect()
}

#[test]
fn dropping_a_png_places_one_selected_image_layer_at_the_pointer() {
    let path = write_file("photo.png", &encoded(image::ImageFormat::Png, 800, 600));
    let mut s = Sim::new();
    let before = ids(&s);
    let target = s.screen(60.0, 900.0);
    s.mv(target);
    s.drop_on_window(vec![dropped(&path)]);
    std::fs::remove_file(&path).ok();
    let (id, picture) = the_image_layer(&s, &before);
    assert_eq!(picture, (800, 600));
    let size = s.app().doc.get(id).unwrap().size;
    assert_eq!((size.x, size.y), (400.0, 300.0), "400 px on the longest side, like the Image tool");
    assert!(s.app().doc.get(id).unwrap().name.ends_with("photo"), "named after the file: {}", s.app().doc.get(id).unwrap().name);
    let (c, page) = (center_of(&s, id), s.app().view.to_page(s.app().canvas_rect.min, target));
    assert!((c.x - page.x).abs() <= 1.0 && (c.y - page.y).abs() <= 1.0, "({}, {}) vs drop ({}, {})", c.x, c.y, page.x, page.y);
    undoes_in_one_step(&mut s, &before);
}

#[test]
fn jpeg_and_webp_files_drop_as_image_layers_too() {
    for (name, format) in [("photo.jpg", image::ImageFormat::Jpeg), ("PHOTO.JPEG", image::ImageFormat::Jpeg), ("photo.webp", image::ImageFormat::WebP)] {
        let path = write_file(name, &encoded(format, 64, 32));
        let mut s = Sim::new();
        let before = ids(&s);
        let target = s.screen(60.0, 900.0);
        s.mv(target);
        s.drop_on_window(vec![dropped(&path)]);
        std::fs::remove_file(&path).ok();
        let (id, picture) = the_image_layer(&s, &before);
        assert_eq!(picture, (64, 32), "{name}");
        let size = s.app().doc.get(id).unwrap().size;
        assert_eq!((size.x, size.y), (64.0, 32.0), "{name}: a small picture keeps its own size");
    }
}

#[test]
fn several_dropped_files_are_offset_24px_each() {
    let paths: Vec<_> = (0..3).map(|i| write_file(&format!("many-{i}.png"), &encoded(image::ImageFormat::Png, 50, 50))).collect();
    let mut s = Sim::new();
    let before = ids(&s);
    let target = s.screen(60.0, 900.0);
    s.mv(target);
    s.drop_on_window(paths.iter().map(|p| dropped(p)).collect());
    for p in &paths {
        std::fs::remove_file(p).ok();
    }
    let layers = image_layers(&s, &before);
    assert_eq!(layers.len(), 3);
    let mut centers: Vec<DV> = layers.iter().map(|id| center_of(&s, *id)).collect();
    centers.sort_by(|a, b| a.x.total_cmp(&b.x));
    for pair in centers.windows(2) {
        assert!((pair[1].x - pair[0].x - 24.0).abs() <= 1.0 && (pair[1].y - pair[0].y - 24.0).abs() <= 1.0, "{pair:?}");
    }
}

#[test]
fn an_image_dropped_outside_the_canvas_lands_at_the_viewport_center() {
    let path = write_file("outside.png", &encoded(image::ImageFormat::Png, 40, 40));
    let mut s = Sim::new();
    let before = ids(&s);
    s.mv(Pos2::new(4.0, 4.0));
    assert!(!s.app().canvas_rect.contains(Pos2::new(4.0, 4.0)), "precondition: the pointer is off the canvas");
    s.drop_on_window(vec![dropped(&path)]);
    std::fs::remove_file(&path).ok();
    let (id, _) = the_image_layer(&s, &before);
    assert_centered_on_viewport(&s, id);
}

#[test]
fn image_bytes_without_a_path_drop_too() {
    let mut s = Sim::new();
    let before = ids(&s);
    let file = egui::DroppedFile { name: "logo.png".into(), bytes: Some(encoded(image::ImageFormat::Png, 30, 30).into()), ..Default::default() };
    s.mv(s.screen(60.0, 900.0));
    s.drop_on_window(vec![file]);
    let (id, _) = the_image_layer(&s, &before);
    assert_eq!(s.app().doc.get(id).unwrap().name, "logo");
}

#[test]
fn a_broken_image_file_says_so_and_adds_nothing() {
    let path = write_file("broken.png", b"this is not a picture");
    let mut s = Sim::new();
    let before = ids(&s);
    let doc_name = s.app().doc.name.clone();
    s.mv(s.screen(60.0, 900.0));
    s.drop_on_window(vec![dropped(&path)]);
    std::fs::remove_file(&path).ok();
    assert_eq!(ids(&s), before);
    assert_eq!(s.app().doc.name, doc_name, "the open document stays open");
    assert!(toast(&s).is_some_and(|(_, error)| error), "an error toast: {:?}", toast(&s));
}

#[test]
fn a_dropped_design_file_still_opens_even_beside_an_image() {
    let mut other = designos_doc::Document::new("Dropped design");
    other.add_page("Page 1");
    let design = write_file("design.designos", other.to_json().unwrap().as_bytes());
    let picture = write_file("beside.png", &encoded(image::ImageFormat::Png, 20, 20));
    let mut s = Sim::new();
    s.drop_on_window(vec![dropped(&picture), dropped(&design)]);
    std::fs::remove_file(&design).ok();
    std::fs::remove_file(&picture).ok();
    assert_eq!(s.app().doc.name, "Dropped design", "the non-image file keeps its old meaning");
    assert!(!s.app().doc.images.values().any(|b| b.starts_with(b"\x89PNG")), "no image layer was made");
}
