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
    // layer ids are handed out as the pastes land, so their order is the order the pastes were made in
    let order: Vec<String> = added(&s, &before).iter().map(|id| s.app().doc.get(*id).unwrap().name.clone()).collect();
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
    assert_eq!(toast(&s), Some(("Copied image is unreadable. Copy it again.".into(), true)));
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

// ------------------------------------------------------------------------ the window keeps drawing for a waiting paste

/// A clipboard image that takes a while to read, as a large screenshot does.
fn slow_image() -> Option<ClipImage> {
    std::thread::sleep(std::time::Duration::from_millis(200));
    Some(solid(40, 40))
}

impl Sim {
    /// One frame like `Sim::frame`, but it does not wait for pending pastes. Returns how long egui may wait before
    /// the next frame: `Duration::MAX` when nothing asks for one.
    fn frame_without_settling(&mut self, events: Vec<Event>) -> std::time::Duration {
        self.t += 0.05;
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 1088.0))),
            time: Some(self.t),
            events,
            modifiers: self.mods,
            ..Default::default()
        };
        let ctx = self.ctx.clone();
        let shell = &mut self.shell;
        let out = ctx.run_ui(raw, |ui| shell.draw(ui));
        out.viewport_output.get(&egui::ViewportId::ROOT).map(|v| v.repaint_delay).unwrap_or(std::time::Duration::MAX)
    }

    /// Draws frames while the only thing the window waits for is the clipboard worker, which keeps asking for frames.
    fn draw_until_idle_or_waiting_paste(&mut self) {
        for _ in 0..5 {
            let delay = self.frame_without_settling(vec![]);
            assert!(delay <= std::time::Duration::from_millis(50), "a waiting paste keeps the window drawing");
        }
    }

    /// Draws frames until the window is idle, as a real one is when the user does nothing: no frame asks for the next.
    fn draw_until_idle(&mut self) {
        let started = std::time::Instant::now();
        while self.frame_without_settling(vec![]) != std::time::Duration::MAX {
            assert!(started.elapsed() < std::time::Duration::from_secs(10), "the window never went idle");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

#[test]
fn a_paste_waiting_for_the_clipboard_worker_keeps_the_window_drawing_until_it_lands() {
    use std::time::Duration;
    let mut s = Sim::new();
    s.shell.app.system_image = slow_image;
    let before = ids(&s);
    s.draw_until_idle();
    // Cmd+V on an image-only clipboard: the V release starts the worker after this frame's own poll of the jobs, and
    // an idle window draws nothing more by itself.
    s.mods = Modifiers::COMMAND;
    let delay = s.frame_without_settling(vec![key_v_release()]);
    s.mods = Modifiers::NONE;
    assert!(!s.shell.app.paste_jobs.is_empty(), "precondition: the worker is still reading");
    assert!(delay <= Duration::from_millis(50), "the frame that started the paste must ask for the next one soon, it waits {delay:?}");
    // Frames as the window draws them until the paste lands. egui follows an input event with one more frame by
    // itself, so the chain is only kept alive by the paste when every frame, with no input, asks for the next.
    let started = std::time::Instant::now();
    let mut frames_without_input = 0;
    while !s.shell.app.paste_jobs.is_empty() {
        assert!(started.elapsed() < Duration::from_secs(10), "the paste never landed");
        std::thread::sleep(Duration::from_millis(10));
        let delay = s.frame_without_settling(vec![]);
        frames_without_input += 1;
        if !s.shell.app.paste_jobs.is_empty() {
            assert!(delay <= Duration::from_millis(50), "frame {frames_without_input} after the paste started leaves the paste waiting {delay:?}");
        }
    }
    assert!(frames_without_input >= 2, "precondition: the worker outlasted several frames");
    the_image_layer(&s, &before);
}

// ------------------------------------------------------------------------------- where the layer lands

/// World-space box of a layer: x, y, width, height.
fn world_box(s: &Sim, id: NodeId) -> (f32, f32, f32, f32) {
    let r = crate::util::world_aabb(&s.app().doc, id);
    (r.x, r.y, r.w, r.h)
}

/// The share of a layer's box that shows: its box cut by every ancestor frame that clips its content.
fn visible_share(s: &Sim, id: NodeId) -> f32 {
    let (mut x0, mut y0, w, h) = world_box(s, id);
    let (mut x1, mut y1) = (x0 + w, y0 + h);
    let mut up = s.app().doc.get(id).and_then(|n| n.parent);
    while let Some(p) = up {
        let n = s.app().doc.get(p).unwrap();
        if n.kind.frame_props().is_some_and(|f| f.clips_content) {
            let (px, py, pw, ph) = world_box(s, p);
            (x0, y0, x1, y1) = (x0.max(px), y0.max(py), x1.min(px + pw), y1.min(py + ph));
        }
        up = n.parent;
    }
    ((x1 - x0).max(0.0) * (y1 - y0).max(0.0)) / (w * h)
}

/// Frames as Figma imports them: every top-level frame clips what is inside it.
fn clip_every_frame(s: &mut Sim) {
    let page = s.app().page;
    for id in s.app().doc.get(page).unwrap().children.clone() {
        if let Some(f) = s.shell.app.doc.get_mut(id).and_then(|n| n.kind.frame_props_mut()) {
            f.clips_content = true;
        }
    }
    s.shell.app.changed();
}

fn frame_contains(s: &Sim, frame: NodeId, p: DV) -> bool {
    let (x, y, w, h) = world_box(s, frame);
    p.x >= x && p.x <= x + w && p.y >= y && p.y <= y + h
}

fn parent_of(s: &Sim, id: NodeId) -> NodeId {
    s.app().doc.get(id).unwrap().parent.unwrap()
}

#[test]
fn a_pasted_image_goes_into_the_frame_under_the_viewport_center_and_stays_visible() {
    for clipping in [false, true] {
        let mut s = Sim::new();
        if clipping {
            clip_every_frame(&mut s);
        }
        s.shell.app.system_image = wide_image;
        let dashboard = s.by_name("Dashboard");
        let before = ids(&s);
        ctrl_v(&mut s, None);
        let (id, _) = the_image_layer(&s, &before);
        let center = viewport_center(&s);
        assert!(frame_contains(&s, dashboard, center), "precondition: the viewport center is over the Dashboard");
        assert_eq!(parent_of(&s, id), dashboard, "clipping frames: {clipping}: the frame under the center, not the one under the layer's corner");
        let share = visible_share(&s, id);
        assert!(share >= 0.9, "clipping frames: {clipping}: visible share {share}");
        if !clipping {
            assert_centered_on_viewport(&s, id);
        }
    }
}

#[test]
fn an_image_dropped_on_a_frame_goes_into_that_frame_and_stays_visible() {
    for clipping in [false, true] {
        let path = write_file(&format!("onto-frame-{clipping}.png"), &encoded(image::ImageFormat::Png, 800, 600));
        let mut s = Sim::new();
        if clipping {
            clip_every_frame(&mut s);
        }
        let dashboard = s.by_name("Dashboard");
        let (x, y, _, _) = world_box(&s, dashboard);
        let before = ids(&s);
        // 50 px inside the Dashboard's top-left corner, where a layer centered on the pointer sticks out to the left
        s.mv(s.screen(x + 50.0, y + 50.0));
        s.drop_on_window(vec![dropped(&path)]);
        std::fs::remove_file(&path).ok();
        let (id, _) = the_image_layer(&s, &before);
        assert_eq!(parent_of(&s, id), dashboard, "clipping frames: {clipping}");
        let share = visible_share(&s, id);
        assert!(share >= 0.9, "clipping frames: {clipping}: visible share {share}");
    }
}

#[test]
fn with_a_frame_selected_text_and_images_go_into_it_even_when_the_viewport_center_is_elsewhere() {
    // Welcome is a clipping frame beside the viewport center, which is over the Dashboard
    for kind in ["text", "long text", "small image", "large image"] {
        let mut s = Sim::new();
        clip_every_frame(&mut s);
        let welcome = s.by_name("Welcome");
        s.shell.app.selection = vec![welcome];
        assert!(!frame_contains(&s, welcome, viewport_center(&s)), "precondition: the viewport center is outside the Welcome frame");
        let before = ids(&s);
        match kind {
            "text" => ctrl_v(&mut s, Some("Release notes for Friday")),
            "long text" => ctrl_v(&mut s, Some(&"A sentence that goes on and on without any line break, ".repeat(12))),
            "small image" => {
                s.shell.app.system_image = small_image;
                ctrl_v(&mut s, None);
            }
            _ => {
                s.shell.app.system_image = wide_image;
                ctrl_v(&mut s, None);
            }
        }
        let new = added(&s, &before);
        assert_eq!(new.len(), 1, "{kind}: {new:?}");
        assert_eq!(parent_of(&s, new[0]), welcome, "{kind}: pasted into the selected frame");
        let (wx, wy, ww, wh) = world_box(&s, welcome);
        let (c, centre) = (center_of(&s, new[0]), DV::new(wx + ww / 2.0, wy + wh / 2.0));
        assert!(
            (c.x - centre.x).abs() <= 1.0 && (c.y - centre.y).abs() <= 1.0,
            "{kind}: centered in the frame, ({}, {}) vs ({}, {})",
            c.x,
            c.y,
            centre.x,
            centre.y
        );
        // an image wider than the frame (400 px against 360 px) can only show 90% of itself
        let share = visible_share(&s, new[0]);
        assert!(share >= 0.89, "{kind}: visible share {share} (frame {ww}x{wh})");
        let _ = wy;
    }
}

#[test]
fn with_a_frame_selected_that_holds_the_viewport_center_the_layer_stays_at_the_center() {
    let mut s = Sim::new();
    let dashboard = s.by_name("Dashboard");
    s.shell.app.selection = vec![dashboard];
    let before = ids(&s);
    ctrl_v(&mut s, Some("Release notes for Friday"));
    let new = added(&s, &before);
    assert_eq!(parent_of(&s, new[0]), dashboard);
    assert_centered_on_viewport(&s, new[0]);
}

#[test]
fn a_layer_selected_inside_a_frame_pastes_into_that_frame() {
    let mut s = Sim::new();
    clip_every_frame(&mut s);
    // a text layer of the Welcome frame; it holds no layers of its own, so the paste goes to its frame
    let heading = s.app().doc.nodes.values().find(|n| matches!(&n.kind, NodeKind::Text(t) if t.content == "Welcome back")).map(|n| n.id).expect("the heading");
    let its_frame = parent_of(&s, heading);
    s.shell.app.selection = vec![heading];
    let before = ids(&s);
    ctrl_v(&mut s, Some("Release notes for Friday"));
    let new = added(&s, &before);
    assert_eq!(parent_of(&s, new[0]), its_frame);
    assert!(visible_share(&s, new[0]) >= 0.9);
}

#[test]
fn a_frame_selected_inside_a_clipping_frame_takes_the_paste_and_the_layer_is_centered_in_it() {
    let mut s = Sim::new();
    clip_every_frame(&mut s);
    // the demo's Button is a frame inside Welcome; the viewport center is outside both
    let (button, welcome) = (s.by_name("Button"), s.by_name("Welcome"));
    assert!(s.app().doc.get(button).unwrap().kind.is_container() && parent_of(&s, button) == welcome, "precondition: a frame inside Welcome");
    s.shell.app.selection = vec![button];
    let before = ids(&s);
    ctrl_v(&mut s, Some("Release notes for Friday"));
    let new = added(&s, &before);
    assert_eq!(parent_of(&s, new[0]), button, "the selected frame, not its parent");
    let (bx, by, bw, bh) = world_box(&s, button);
    let (c, middle) = (center_of(&s, new[0]), DV::new(bx + bw / 2.0, by + bh / 2.0));
    assert!((c.x - middle.x).abs() <= 1.0 && (c.y - middle.y).abs() <= 1.0, "centered in the Button: ({}, {}) vs ({}, {})", c.x, c.y, middle.x, middle.y);
    assert!(visible_share(&s, new[0]) >= 0.9);
}

// ----------------------------------------------------- a clipping frame further up hides what sticks out of it

/// What the nested fixtures put near the right edge of the clipping `Welcome` frame (360 px wide at x 120): a 60 px
/// box whose center is 30 px from that edge, so a 120 px layer centered on it would stick out by 30 px.
struct Nested {
    /// Where a drop lands: the non-clipping frame the layer joins.
    frame: NodeId,
    /// A layer to select so that a paste joins the box: the frame itself, or a rectangle inside the group.
    selectable: NodeId,
    welcome: NodeId,
}

/// `group`: a Figma-style group inside the screen, holding a non-clipping frame and a rectangle. Otherwise a
/// Studio-drawn frame, which does not clip, straight in the screen. Either way the screen clips and the box does not.
fn nested_in_clipping_screen(s: &mut Sim, group: bool) -> Nested {
    clip_every_frame(s);
    let welcome = s.by_name("Welcome");
    let app = &mut s.shell.app;
    let add = |app: &mut crate::app::App, parent: NodeId, mut n: designos_doc::Node, name: &str, at: (f32, f32)| {
        n.name = name.into();
        n.transform = designos_doc::Affine::translate(at.0, at.1);
        n.size = designos_doc::Vec2::new(60.0, 60.0);
        app.doc.insert(parent, n).unwrap()
    };
    let nested = if group {
        let id = app.doc.alloc_id();
        let g = add(app, welcome, designos_doc::Node::new(id, NodeKind::Group), "Icon group", (300.0, 300.0));
        let (fid, rid) = (app.doc.alloc_id(), app.doc.alloc_id());
        let frame = add(app, g, designos_doc::Node::frame(fid, designos_doc::Rect::new(0.0, 0.0, 60.0, 60.0)), "Badge", (0.0, 0.0));
        let chip = add(app, g, designos_doc::Node::rectangle(rid, designos_doc::Rect::new(0.0, 0.0, 60.0, 60.0)), "Chip", (0.0, 0.0));
        Nested { frame, selectable: chip, welcome }
    } else {
        let id = app.doc.alloc_id();
        let frame = add(app, welcome, designos_doc::Node::frame(id, designos_doc::Rect::new(0.0, 0.0, 60.0, 60.0)), "Badge row", (300.0, 300.0));
        Nested { frame, selectable: frame, welcome }
    };
    app.changed();
    s.frame(vec![]);
    nested
}

fn check_nested_paste(group: bool, text: bool) {
    let mut s = Sim::new();
    let n = nested_in_clipping_screen(&mut s, group);
    let joined = if group { parent_of(&s, n.selectable) } else { n.frame };
    assert!(!s.app().doc.get(joined).unwrap().kind.frame_props().is_some_and(|f| f.clips_content), "precondition: the box it joins does not clip");
    s.shell.app.selection = vec![n.selectable];
    let before = ids(&s);
    if text {
        ctrl_v(&mut s, Some("Release notes for Friday"));
    } else {
        s.shell.app.system_image = small_image;
        ctrl_v(&mut s, None);
    }
    let new = added(&s, &before);
    assert_eq!(new.len(), 1, "{new:?}");
    assert_eq!(parent_of(&s, new[0]), joined, "it joins the box, not the screen");
    let share = visible_share(&s, new[0]);
    assert!(share >= 0.99, "visible share {share}");
    // inside the screen, which is what hides it
    let (x, _, w, _) = world_box(&s, new[0]);
    let (wx, _, ww, _) = world_box(&s, n.welcome);
    assert!(x >= wx - 0.5 && x + w <= wx + ww + 0.5, "x {x}..{} against the screen {wx}..{}", x + w, wx + ww);
}

fn check_nested_drop(group: bool) {
    let path = write_file(&format!("nested-{group}.png"), &encoded(image::ImageFormat::Png, 120, 80));
    let mut s = Sim::new();
    let n = nested_in_clipping_screen(&mut s, group);
    let (x, y, w, h) = world_box(&s, n.frame);
    let before = ids(&s);
    s.mv(s.screen(x + w / 2.0, y + h / 2.0));
    s.drop_on_window(vec![dropped(&path)]);
    std::fs::remove_file(&path).ok();
    let (id, _) = the_image_layer(&s, &before);
    assert_eq!(parent_of(&s, id), n.frame, "the frame under the pointer");
    let share = visible_share(&s, id);
    assert!(share >= 0.99, "visible share {share}");
}

#[test]
fn pasted_text_in_a_group_inside_a_clipping_frame_is_never_partly_hidden() {
    check_nested_paste(true, true);
}

#[test]
fn a_pasted_image_in_a_group_inside_a_clipping_frame_is_never_partly_hidden() {
    check_nested_paste(true, false);
}

#[test]
fn pasted_text_in_a_non_clipping_frame_inside_a_clipping_frame_is_never_partly_hidden() {
    check_nested_paste(false, true);
}

#[test]
fn a_pasted_image_in_a_non_clipping_frame_inside_a_clipping_frame_is_never_partly_hidden() {
    check_nested_paste(false, false);
}

#[test]
fn an_image_dropped_on_a_non_clipping_frame_in_a_group_inside_a_clipping_frame_is_never_partly_hidden() {
    check_nested_drop(true);
}

#[test]
fn an_image_dropped_on_a_non_clipping_frame_inside_a_clipping_frame_is_never_partly_hidden() {
    check_nested_drop(false);
}

/// A 60x40 picture: it fits the 100 px wide overlap below.
fn narrow_image() -> Option<ClipImage> {
    Some(solid(60, 40))
}

/// A frame (`frame`) or rectangle at `at` in `parent`, `size` big; `clips` is the frame's own setting.
fn add_layer(app: &mut crate::app::App, parent: NodeId, frame: bool, name: &str, at: (f32, f32), size: (f32, f32), clips: bool) -> NodeId {
    let (id, rect) = (app.doc.alloc_id(), designos_doc::Rect::new(0.0, 0.0, size.0, size.1));
    let mut n = if frame { designos_doc::Node::frame(id, rect) } else { designos_doc::Node::rectangle(id, rect) };
    n.name = name.into();
    n.transform = designos_doc::Affine::translate(at.0, at.1);
    if let Some(f) = n.kind.frame_props_mut() {
        f.clips_content = clips;
    }
    app.doc.insert(parent, n).unwrap()
}

/// Two clipping frames that only partly overlap, with the frame the paste joins inside the inner one. The `Welcome`
/// screen clips (world 120..480 x 120..760). A clipping `Panel` sticks out past its right edge (world 380..620 x
/// 300..500), so only 380..480 x 300..500 of it shows. A non-clipping `Strip` inside the panel (world 400..600 x
/// 320..480) is the frame the paste joins, through the `Chip` rectangle selected in it. Returns the strip, the chip,
/// the panel and the screen.
fn strip_in_a_panel_sticking_out_of_a_screen(s: &mut Sim) -> (NodeId, NodeId, NodeId, NodeId) {
    clip_every_frame(s);
    let welcome = s.by_name("Welcome");
    let app = &mut s.shell.app;
    let panel = add_layer(app, welcome, true, "Panel", (260.0, 180.0), (240.0, 200.0), true);
    let strip = add_layer(app, panel, true, "Strip", (20.0, 20.0), (200.0, 160.0), false);
    let chip = add_layer(app, strip, false, "Chip", (10.0, 10.0), (20.0, 20.0), false);
    app.changed();
    s.frame(vec![]);
    (strip, chip, panel, welcome)
}

fn check_paste_into_the_overlap_of_two_clipping_frames(text: bool) {
    let mut s = Sim::new();
    let (strip, chip, panel, welcome) = strip_in_a_panel_sticking_out_of_a_screen(&mut s);
    // the part that shows is the overlap of the two clipping frames, narrower than the panel and the strip
    let ((wx, wy, ww, wh), (px, py, pw, ph)) = (world_box(&s, welcome), world_box(&s, panel));
    let overlap = (wx.max(px), wy.max(py), (wx + ww).min(px + pw), (wy + wh).min(py + ph));
    assert!(overlap.2 < px + pw && overlap.0 < world_box(&s, strip).0 + world_box(&s, strip).2, "precondition: the panel sticks out of the screen");
    s.shell.app.selection = vec![chip];
    let before = ids(&s);
    if text {
        ctrl_v(&mut s, Some("Release notes for Friday"));
    } else {
        s.shell.app.system_image = narrow_image;
        ctrl_v(&mut s, None);
    }
    let new = added(&s, &before);
    assert_eq!(new.len(), 1, "{new:?}");
    assert_eq!(parent_of(&s, new[0]), strip, "it joins the strip");
    let (x, y, w, h) = world_box(&s, new[0]);
    assert!(
        x >= overlap.0 - 0.5 && x + w <= overlap.2 + 0.5 && y >= overlap.1 - 0.5 && y + h <= overlap.3 + 0.5,
        "inside the overlap {overlap:?}, not just inside the panel: ({x}, {y}) {w}x{h}"
    );
    let share = visible_share(&s, new[0]);
    assert!(share >= 0.99, "visible share {share}");
}

#[test]
fn pasted_text_lands_in_the_overlap_of_a_clipping_panel_sticking_out_of_a_clipping_screen() {
    check_paste_into_the_overlap_of_two_clipping_frames(true);
}

#[test]
fn a_pasted_image_lands_in_the_overlap_of_a_clipping_panel_sticking_out_of_a_clipping_screen() {
    check_paste_into_the_overlap_of_two_clipping_frames(false);
}

// --------------------------------------------------------------------------- Edit > Paste while typing

#[test]
fn edit_paste_during_a_text_edit_pastes_into_the_text_and_adds_no_layer() {
    let mut s = Sim::new();
    s.key(Key::T);
    let p = s.screen(300.0, 820.0);
    s.click(p);
    s.type_text("a");
    let id = s.app().selection[0];
    assert!(s.app().canvas.text.is_some(), "precondition: a text session is open");
    s.shell.app.system_text = some_words;
    s.shell.app.system_image = wide_image;
    let before = ids(&s);
    edit_paste(&mut s);
    assert_eq!(ids(&s), before, "no new layer, and an image has no place in text");
    assert!(matches!(&s.app().doc.get(id).unwrap().kind, NodeKind::Text(t) if t.content == "aRelease notes for Friday"));
    assert!(s.app().canvas.text.is_some(), "the session stays open");
}

// ------------------------------------------------------------------ a clipboard read that never answers

static RELEASE_HUNG_READ: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// A clipboard owner that never hands over the image (until the test lets go of it).
fn hung_image() -> Option<ClipImage> {
    let started = std::time::Instant::now();
    while !RELEASE_HUNG_READ.load(std::sync::atomic::Ordering::SeqCst) && started.elapsed() < std::time::Duration::from_secs(30) {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    None
}

#[test]
fn a_clipboard_read_that_never_answers_times_out_with_a_toast_and_blocks_nothing() {
    RELEASE_HUNG_READ.store(false, std::sync::atomic::Ordering::SeqCst);
    let mut s = Sim::new();
    s.shell.app.system_image = hung_image;
    s.mods = Modifiers::COMMAND;
    s.frame_without_settling(vec![key_v_release()]);
    s.mods = Modifiers::NONE;
    assert_eq!(s.shell.app.paste_jobs.len(), 1, "precondition: the read is waiting");
    s.draw_until_idle_or_waiting_paste();
    // ten seconds later
    s.shell.app.expire_paste_jobs();
    s.frame_without_settling(vec![]);
    assert!(s.shell.app.paste_jobs.is_empty(), "the hung read is given up");
    assert_eq!(toast(&s), Some(("Clipboard did not answer. Try again.".into(), true)));
    s.draw_until_idle();
    // a later paste is not held behind it
    s.shell.app.system_image = no_system_image;
    let before = ids(&s);
    ctrl_v(&mut s, Some("Release notes for Friday"));
    assert_eq!(added(&s, &before).len(), 1);
    RELEASE_HUNG_READ.store(true, std::sync::atomic::Ordering::SeqCst);
}

#[test]
fn a_clipboard_read_that_times_out_still_pastes_the_text_it_had() {
    RELEASE_HUNG_READ.store(false, std::sync::atomic::Ordering::SeqCst);
    let mut s = Sim::new();
    s.shell.app.system_image = hung_image;
    let before = ids(&s);
    // the text came with the paste event; only the image never arrives
    s.mods = Modifiers::COMMAND;
    s.frame_without_settling(vec![Event::Paste("Release notes for Friday".into())]);
    s.mods = Modifiers::NONE;
    assert_eq!(s.shell.app.paste_jobs.len(), 1, "precondition: the read is waiting");
    s.shell.app.expire_paste_jobs();
    s.frame_without_settling(vec![]);
    assert!(s.shell.app.paste_jobs.is_empty());
    let new = added(&s, &before);
    assert_eq!(new.len(), 1, "the text became a layer: {new:?}");
    assert!(matches!(&s.app().doc.get(new[0]).unwrap().kind, NodeKind::Text(t) if t.content == "Release notes for Friday"));
    assert_eq!(toast(&s), None, "there is nothing to retry");
    RELEASE_HUNG_READ.store(true, std::sync::atomic::Ordering::SeqCst);
}

#[test]
fn a_paste_still_waiting_when_the_tab_changes_never_lands_in_another_document() {
    let mut s = Sim::new();
    s.shell.app.system_image = slow_image;
    s.mods = Modifiers::COMMAND;
    s.frame_without_settling(vec![key_v_release()]);
    s.mods = Modifiers::NONE;
    assert_eq!(s.shell.app.paste_jobs.len(), 1, "precondition: the read is waiting");
    let first_tab_nodes = s.count();
    let second = s.shell.app.open_new_design_tab();
    let second_tab_nodes = s.count();
    let started = std::time::Instant::now();
    while !s.shell.app.paste_jobs.is_empty() {
        assert!(started.elapsed() < std::time::Duration::from_secs(10), "the paste never finished");
        std::thread::sleep(std::time::Duration::from_millis(10));
        s.frame_without_settling(vec![]);
    }
    assert_eq!(s.count(), second_tab_nodes, "the new tab's document is untouched");
    s.shell.app.switch_tab(second.saturating_sub(1));
    assert_eq!(s.count(), first_tab_nodes, "and so is the tab the paste started in");
}

// --------------------------------------------------------------------------------------- copy and undo

#[test]
fn several_dropped_files_are_one_undo_step_and_all_selected() {
    let png = |i: usize| write_file(&format!("one-step-{i}.png"), &encoded(image::ImageFormat::Png, 50, 50));
    let svg = write_file(
        "one-step.svg",
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><rect width="20" height="20" fill="#336699"/></svg>"##.as_bytes(),
    );
    let paths = [png(0), svg, png(1)];
    let mut s = Sim::new();
    let before = ids(&s);
    let depth = s.app().hist.undo_depth();
    s.mv(s.screen(60.0, 900.0));
    s.drop_on_window(paths.iter().map(|p| dropped(p)).collect());
    for p in &paths {
        std::fs::remove_file(p).ok();
    }
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "one history step for the whole drop");
    assert_eq!(image_layers(&s, &before).len(), 2);
    assert_eq!(s.app().selection.len(), 3, "every placed layer is selected");
    undoes_in_one_step(&mut s, &before);
}
