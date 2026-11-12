//! In a Display P3 document the eyedropper, the panel swatches and the scoped agent canvas read the
//! document's colours the way the canvas draws them.

use super::*;
use crate::panels::properties::sample_page;
use designos_doc::{Color as DColor, ColorProfile, Document, ImageParams, ImageScaleMode, Node as DNode, Paint, PaintKind, Rect as DRect};

const MID: DColor = DColor { r: 0.5, g: 0.4, b: 0.8, a: 1.0 };

fn doc_with_fill(fill: Paint) -> (Document, NodeId, NodeId) {
    let mut doc = Document::new("t");
    let page = doc.add_page("p");
    doc.get_mut(page).unwrap().kind = NodeKind::Page { background: DColor::WHITE };
    let id = doc.alloc_id();
    let mut n = DNode::rectangle(id, DRect::new(0.0, 0.0, 40.0, 40.0));
    n.fills = vec![fill];
    let id = doc.insert(page, n).unwrap();
    doc.color_profile = ColorProfile::DisplayP3;
    (doc, page, id)
}

fn pick_and_apply(doc: &mut Document, page: NodeId, id: NodeId, rounds: usize) -> Vec<[u8; 3]> {
    let mut seen = Vec::new();
    for _ in 0..rounds {
        let [r, g, b] = sample_page(doc, page, designos_doc::Vec2::new(20.0, 20.0)).unwrap();
        seen.push([r, g, b]);
        doc.get_mut(id).unwrap().fills = vec![Paint::solid(DColor::rgb(r, g, b))];
    }
    seen
}

#[test]
fn picking_a_swatch_in_a_p3_document_returns_its_stored_numbers_every_time() {
    let (mut doc, page, id) = doc_with_fill(Paint::solid(MID));
    let seen = pick_and_apply(&mut doc, page, id, 4);
    assert_eq!(seen[0], [128, 102, 204], "the stored P3 numbers");
    assert!(seen.iter().all(|s| *s == seen[0]), "four picks in a row give one colour: {seen:?}");
}

#[test]
fn picking_from_an_image_in_a_p3_document_is_stable() {
    let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([200, 60, 60, 255]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    img.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    let fill = Paint {
        kind: PaintKind::Image { image_ref: "img".into(), scale_mode: ImageScaleMode::Fill, params: ImageParams::default() },
        ..Paint::solid(DColor::WHITE)
    };
    let (mut doc, page, id) = doc_with_fill(fill);
    doc.images.insert("img".into(), bytes.into_inner());
    let seen = pick_and_apply(&mut doc, page, id, 4);
    assert_ne!(seen[0], [200, 60, 60], "the sRGB image bytes are converted into P3 numbers");
    assert!(seen.iter().all(|s| *s == seen[0]), "picks from an image give one colour: {seen:?}");
}

fn filled_near(s: &Sim, c: [u8; 3]) -> bool {
    s.shapes.iter().any(|cs| match &cs.shape {
        egui::Shape::Rect(r) => {
            (r.fill.r() as i32 - c[0] as i32).abs() <= 1 && (r.fill.g() as i32 - c[1] as i32).abs() <= 1 && (r.fill.b() as i32 - c[2] as i32).abs() <= 1
        }
        _ => false,
    })
}

fn filled_with(s: &Sim, c: egui::Color32) -> bool {
    s.shapes.iter().any(|cs| matches!(&cs.shape, egui::Shape::Rect(r) if r.fill == c))
}

#[test]
fn the_page_swatch_of_a_p3_document_shows_the_converted_colour() {
    let mut s = Sim::new();
    s.shell.app.selection.clear();
    let page = s.app().page;
    s.shell.app.doc.get_mut(page).unwrap().kind = NodeKind::Page { background: MID };
    s.shell.app.doc.color_profile = ColorProfile::DisplayP3;
    s.frame(vec![]);
    s.frame(vec![]);
    // the canvas pixel is (132,101,211); the swatch goes through 8-bit channels first, so it may differ by one level
    assert!(filled_near(&s, [132, 101, 211]), "the swatch shows the colour the canvas draws");
    assert!(!filled_with(&s, egui::Color32::from_rgb(128, 102, 204)), "never the raw P3 numbers, swatch or canvas backdrop");
    s.shell.app.doc.color_profile = ColorProfile::Srgb;
    s.frame(vec![]);
    s.frame(vec![]);
    assert!(filled_with(&s, egui::Color32::from_rgb(128, 102, 204)), "an sRGB document shows its numbers as they are");
}

#[test]
fn the_scoped_agent_canvas_keeps_the_document_profile() {
    let (doc, page, _) = doc_with_fill(Paint::solid(MID));
    let scoped = crate::live_canvas_scope::scoped_canvas(&doc, page, &[], "scoped");
    assert_eq!(scoped.color_profile, ColorProfile::DisplayP3);
}

#[test]
fn a_gradient_swatch_in_a_p3_document_shows_the_converted_stops() {
    let stops = vec![designos_doc::GradientStop { pos: 0.0, color: MID }, designos_doc::GradientStop { pos: 1.0, color: MID }];
    let paint = Paint::solid(DColor::WHITE);
    let paint = Paint {
        kind: PaintKind::LinearGradient {
            handles: [designos_doc::Vec2::new(0.0, 0.5), designos_doc::Vec2::new(1.0, 0.5), designos_doc::Vec2::new(0.0, 1.0)],
            stops,
        },
        ..paint
    };
    let vertex_colors = |profile: ColorProfile| {
        crate::widgets::set_document_profile(profile);
        let ctx = egui::Context::default();
        let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(100.0, 100.0))), ..Default::default() };
        let out = ctx.run_ui(raw, |ui| {
            crate::widgets::picker::paint_swatch(ui.painter(), egui::Rect::from_min_size(Pos2::new(10.0, 10.0), egui::vec2(24.0, 24.0)), &paint);
        });
        crate::widgets::set_document_profile(ColorProfile::Srgb);
        out.shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Mesh(m) => m.vertices.first().map(|v| v.color),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let p3 = vertex_colors(ColorProfile::DisplayP3);
    assert!(p3.iter().any(|c| (c.r() as i32 - 133).abs() <= 1 && (c.g() as i32 - 101).abs() <= 1 && (c.b() as i32 - 211).abs() <= 1), "{p3:?}");
    let srgb = vertex_colors(ColorProfile::Srgb);
    assert!(srgb.iter().any(|c| c.r() == 128 && c.g() == 102 && c.b() == 204), "{srgb:?}");
}

#[test]
fn swatches_are_right_in_the_frame_a_tab_switch_happens() {
    let mut s = Sim::new();
    s.shell.app.selection.clear();
    let page = s.app().page;
    s.shell.app.doc.get_mut(page).unwrap().kind = NodeKind::Page { background: MID };
    s.frame(vec![]);
    s.shell.app.open_new_design_tab();
    let page = s.app().page;
    s.shell.app.doc.get_mut(page).unwrap().kind = NodeKind::Page { background: MID };
    s.shell.app.doc.color_profile = ColorProfile::DisplayP3;
    s.shell.app.selection.clear();
    s.frame(vec![]);
    s.frame(vec![]);
    assert!(filled_near(&s, [132, 101, 211]), "the Display P3 tab shows the converted swatch");
    // click the first (sRGB) tab: the frame that switches must already draw its swatch with raw numbers
    let tab = s.rect_of("project-tab-0").center();
    s.mv(tab);
    s.button(true);
    s.button(false);
    assert_eq!(s.app().doc.color_profile, ColorProfile::Srgb, "the click switched to the sRGB tab");
    assert!(filled_with(&s, egui::Color32::from_rgb(128, 102, 204)), "the sRGB tab's swatch is raw in the switching frame");
    assert!(!filled_near(&s, [132, 101, 211]), "no converted swatch in the switching frame");
}

fn pick_profile_frame(s: &mut Sim, i: usize) {
    let ctl = s.rect_of("color-profile");
    s.click(ctl.center());
    let menu = s.rect_of("menu-level-0");
    let y = menu.top() + theme::MENU_PADDING + i as f32 * theme::MENU_ITEM_HEIGHT + theme::MENU_ITEM_HEIGHT / 2.0;
    s.mv(Pos2::new(ctl.left() + 20.0, y));
    s.button(true);
    s.button(false);
}

fn press_cmd_z(s: &mut Sim, shift: bool) {
    let is_mac = s.shell.app.terminal.platform_is_mac_override.unwrap_or(cfg!(target_os = "macos"));
    let modifiers = Modifiers { alt: false, ctrl: !is_mac, shift, mac_cmd: is_mac, command: true };
    s.frame(vec![Event::Key { key: Key::Z, physical_key: Some(Key::Z), pressed: true, repeat: false, modifiers }]);
}

fn press_enter(s: &mut Sim) {
    s.frame(vec![Event::Key { key: Key::Enter, physical_key: Some(Key::Enter), pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
}

fn release_enter(s: &mut Sim) {
    s.frame(vec![Event::Key { key: Key::Enter, physical_key: Some(Key::Enter), pressed: false, repeat: false, modifiers: Modifiers::NONE }]);
}

fn swatch_filled_with(s: &Sim, c: egui::Color32) -> bool {
    s.shapes.iter().any(|cs| matches!(&cs.shape, egui::Shape::Rect(r) if r.rect.size() == Vec2::splat(theme::PAGE_SWATCH) && r.fill == c))
}

#[test]
fn the_page_swatch_updates_in_the_same_frame_as_the_profile_dropdown_undo_and_redo() {
    let mut s = Sim::new();
    s.shell.app.selection.clear();
    let page = s.app().page;
    s.shell.app.doc.get_mut(page).unwrap().kind = NodeKind::Page { background: MID };
    s.frame(vec![]);
    assert!(swatch_filled_with(&s, egui::Color32::from_rgb(128, 102, 204)), "starts in sRGB with raw numbers");

    pick_profile_frame(&mut s, 1);
    assert_eq!(s.app().doc.color_profile, ColorProfile::DisplayP3);
    assert!(filled_near(&s, [132, 101, 211]), "the dropdown frame already draws the converted swatch");
    assert!(!swatch_filled_with(&s, egui::Color32::from_rgb(128, 102, 204)), "no stale raw swatch in the dropdown frame");

    press_cmd_z(&mut s, false);
    assert_eq!(s.app().doc.color_profile, ColorProfile::Srgb);
    assert!(swatch_filled_with(&s, egui::Color32::from_rgb(128, 102, 204)), "the undo frame already draws the raw swatch");
    assert!(!filled_near(&s, [132, 101, 211]), "no stale converted swatch in the undo frame");

    press_cmd_z(&mut s, true);
    assert_eq!(s.app().doc.color_profile, ColorProfile::DisplayP3);
    assert!(filled_near(&s, [132, 101, 211]), "the redo frame already draws the converted swatch");
    assert!(!swatch_filled_with(&s, egui::Color32::from_rgb(128, 102, 204)), "no stale raw swatch in the redo frame");

    // Edit -> Undo and Edit -> Redo from the main menu also update the swatch in the same frame.
    s.click(Pos2::new(28.0, 24.0));
    assert_eq!(s.app().ui.menu, Some(crate::app::MenuKind::Main));
    for _ in 0..4 {
        s.key(Key::ArrowDown);
    }
    s.key(Key::ArrowRight);
    press_enter(&mut s);
    assert_eq!(s.app().doc.color_profile, ColorProfile::Srgb);
    assert!(swatch_filled_with(&s, egui::Color32::from_rgb(128, 102, 204)), "Edit -> Undo updates the swatch in the same frame");
    assert!(!filled_near(&s, [132, 101, 211]), "no stale converted swatch in the Edit -> Undo frame");
    release_enter(&mut s);

    s.click(Pos2::new(28.0, 24.0));
    for _ in 0..4 {
        s.key(Key::ArrowDown);
    }
    s.key(Key::ArrowRight);
    press_enter(&mut s);
    assert_eq!(s.app().doc.color_profile, ColorProfile::DisplayP3);
    assert!(filled_near(&s, [132, 101, 211]), "Edit -> Redo updates the swatch in the same frame");
    assert!(!swatch_filled_with(&s, egui::Color32::from_rgb(128, 102, 204)), "no stale raw swatch in the Edit -> Redo frame");
    release_enter(&mut s);

    // The rail File menu (right-click) runs after the right panel: its Undo and Redo redraw the frame.
    for (target, raw) in [(ColorProfile::Srgb, true), (ColorProfile::DisplayP3, false)] {
        s.shell.app.ui.menu = Some(crate::app::MenuKind::FileMenu);
        s.shell.app.ui.menu_anchor = Some(egui::Rect::from_min_size(Pos2::new(8.0, 8.0), Vec2::splat(32.0)));
        s.frame(vec![]);
        for _ in 0..if raw { 10 } else { 1 } {
            s.key(Key::ArrowDown);
        }
        press_enter(&mut s);
        assert_eq!(s.app().doc.color_profile, target);
        assert_eq!(swatch_filled_with(&s, egui::Color32::from_rgb(128, 102, 204)), raw, "rail File menu Undo/Redo draws the new profile in the same frame");
        assert_eq!(filled_near(&s, [132, 101, 211]), !raw);
        release_enter(&mut s);
    }

    // Command palette Undo and Redo also update the swatch in the same frame.
    s.cmd_key(Key::K);
    assert_eq!(s.app().ui.menu, Some(crate::app::MenuKind::Palette));
    s.frame(vec![]);
    s.type_text("Undo");
    s.frame(vec![]);
    press_enter(&mut s);
    assert_eq!(s.app().doc.color_profile, ColorProfile::Srgb);
    assert!(swatch_filled_with(&s, egui::Color32::from_rgb(128, 102, 204)), "palette Undo updates the swatch in the same frame");
    assert!(!filled_near(&s, [132, 101, 211]), "no stale converted swatch in the palette Undo frame");
    release_enter(&mut s);

    s.cmd_key(Key::K);
    s.frame(vec![]);
    s.type_text("Redo");
    s.frame(vec![]);
    press_enter(&mut s);
    assert_eq!(s.app().doc.color_profile, ColorProfile::DisplayP3);
    assert!(filled_near(&s, [132, 101, 211]), "palette Redo updates the swatch in the same frame");
    assert!(!swatch_filled_with(&s, egui::Color32::from_rgb(128, 102, 204)), "no stale raw swatch in the palette Redo frame");
    release_enter(&mut s);
}

fn p3_doc_with_png(rgb: [u8; 3], key: &str) -> (Document, NodeId) {
    let img = image::RgbaImage::from_pixel(8, 8, image::Rgba([rgb[0], rgb[1], rgb[2], 255]));
    let mut bytes = std::io::Cursor::new(Vec::new());
    img.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    let fill = Paint {
        kind: PaintKind::Image { image_ref: key.into(), scale_mode: ImageScaleMode::Fill, params: ImageParams::default() },
        ..Paint::solid(DColor::WHITE)
    };
    let (mut doc, page, _) = doc_with_fill(fill);
    doc.images.insert(key.into(), bytes.into_inner());
    (doc, page)
}

#[test]
fn closing_a_tab_drops_its_converted_images_and_keeps_open_tabs() {
    let mut s = Sim::new();
    let (doc_a, page_a) = p3_doc_with_png([210, 50, 40], "tab-a");
    let (doc_b, page_b) = p3_doc_with_png([40, 180, 90], "tab-b");
    designos_render::retain_converted_images(std::iter::empty());

    s.shell.app.replace_document(doc_a, None);
    let base_bytes = designos_render::converted_image_bytes();
    let base_conv = designos_render::image_conversions();
    let _ = sample_page(&s.app().doc, page_a, designos_doc::Vec2::new(4.0, 4.0));
    let one_img_bytes = designos_render::converted_image_bytes() - base_bytes;
    assert_eq!(one_img_bytes, 8 * 8 * 4);
    assert_eq!(designos_render::image_conversions() - base_conv, 1);

    s.shell.app.open_new_design_tab();
    s.shell.app.replace_document(doc_b, None);
    let _ = sample_page(&s.app().doc, page_b, designos_doc::Vec2::new(4.0, 4.0));
    assert_eq!(designos_render::converted_image_bytes() - base_bytes, 2 * one_img_bytes);
    assert_eq!(designos_render::image_conversions() - base_conv, 2);

    // Closing the second tab drops its converted image while keeping the first tab's image cached.
    s.shell.app.close_tab(1);
    assert_eq!(designos_render::converted_image_bytes() - base_bytes, one_img_bytes);
    let _ = sample_page(&s.app().doc, page_a, designos_doc::Vec2::new(4.0, 4.0));
    assert_eq!(designos_render::image_conversions() - base_conv, 2, "the remaining open tab's image stays cached");

    // Replacing the remaining document drops its converted image too.
    s.shell.app.replace_document(Document::new("empty"), None);
    assert_eq!(designos_render::converted_image_bytes(), base_bytes);
}

#[test]
fn version_view_exit_and_tab_close_drop_converted_images_of_the_version() {
    let tmp = std::env::temp_dir().join(format!("designos_1805_ver_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let mut s = Sim::new();
    s.shell.app.attach_library(tmp.clone());
    s.shell.app.new_library_file(None);
    let (ver_doc, ver_page) = p3_doc_with_png([190, 60, 70], "ver-only-img");
    let (cur_doc, cur_page) = p3_doc_with_png([60, 120, 210], "cur-only-img");
    designos_render::retain_converted_images(std::iter::empty());

    s.shell.app.doc = ver_doc;
    s.shell.app.page = ver_page;
    assert!(s.shell.app.save_named_version("v1", ""));
    let lf_id = s.app().lib_file.as_ref().unwrap().id.clone();
    let v_n = s.app().lib.as_ref().unwrap().list_versions(&lf_id)[0].n;

    s.shell.app.doc = cur_doc;
    s.shell.app.page = cur_page;
    s.shell.app.save_library_file().unwrap();

    let base_bytes = designos_render::converted_image_bytes();
    let one_img_bytes = 8 * 8 * 4;
    let _ = sample_page(&s.app().doc, cur_page, designos_doc::Vec2::new(4.0, 4.0));
    assert_eq!(designos_render::converted_image_bytes() - base_bytes, one_img_bytes);

    // Open the version whose only image is `ver-only-img`, convert it, then go back to current.
    s.shell.app.view_version(v_n);
    let _ = sample_page(&s.app().doc, ver_page, designos_doc::Vec2::new(4.0, 4.0));
    assert_eq!(designos_render::converted_image_bytes() - base_bytes, 2 * one_img_bytes);
    s.shell.app.exit_version_view();
    assert_eq!(
        designos_render::converted_image_bytes() - base_bytes,
        one_img_bytes,
        "leaving version view drops the version's converted image and keeps the current document's"
    );

    // Open the version again, convert its image, then close the tab while the version is shown.
    s.shell.app.view_version(v_n);
    let _ = sample_page(&s.app().doc, ver_page, designos_doc::Vec2::new(4.0, 4.0));
    assert_eq!(designos_render::converted_image_bytes() - base_bytes, 2 * one_img_bytes);
    let active = s.app().tabs.active_idx;
    s.shell.app.close_tab(active);
    assert_eq!(
        designos_render::converted_image_bytes(),
        base_bytes,
        "closing a tab while a version is shown drops both the version's and the tab's converted images"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn use_disk_version_drops_converted_images_only_the_unsaved_doc_had() {
    let path = std::env::temp_dir().join(format!("designos_1805_disk_{}.designos", std::process::id()));
    let (unsaved_doc, unsaved_page) = p3_doc_with_png([220, 90, 30], "unsaved-only-img");
    designos_render::retain_converted_images(std::iter::empty());
    let mut disk_doc = Document::new("on-disk");
    disk_doc.add_page("Page 1");
    std::fs::write(&path, disk_doc.to_json().unwrap()).unwrap();

    let mut s = Sim::new();
    s.shell.app.replace_document(unsaved_doc, Some(path.clone()));
    let base_bytes = designos_render::converted_image_bytes();
    let _ = sample_page(&s.app().doc, unsaved_page, designos_doc::Vec2::new(4.0, 4.0));
    assert_eq!(designos_render::converted_image_bytes() - base_bytes, 8 * 8 * 4);

    s.shell.app.autosave.conflict_mtime = Some(std::time::SystemTime::UNIX_EPOCH);
    s.shell.app.use_disk_version(&s.ctx);
    assert_eq!(
        designos_render::converted_image_bytes(),
        base_bytes,
        "choosing the on-disk file drops converted images that only the discarded unsaved document had"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn undoing_an_added_image_and_closing_the_tab_drops_its_converted_pixmap() {
    let mut s = Sim::new();
    let (img_doc, _) = p3_doc_with_png([140, 200, 50], "undone-tab-img");
    designos_render::retain_converted_images(std::iter::empty());
    let img_bytes = img_doc.images.get("undone-tab-img").unwrap().clone();

    let mut doc = Document::new("undo-tab");
    let page = doc.add_page("Page 1");
    doc.color_profile = ColorProfile::DisplayP3;
    s.shell.app.replace_document(doc, None);
    let base_bytes = designos_render::converted_image_bytes();

    let rect_id = s.shell.app.doc.alloc_id();
    let mut node = DNode::rectangle(rect_id, DRect::new(0.0, 0.0, 20.0, 20.0));
    node.fills = vec![Paint {
        kind: PaintKind::Image { image_ref: "undone-tab-img".into(), scale_mode: ImageScaleMode::Fill, params: ImageParams::default() },
        ..Paint::solid(DColor::WHITE)
    }];
    let mut add = std::collections::BTreeMap::new();
    add.insert("undone-tab-img".to_string(), img_bytes);
    let cmd = designos_doc::Command::Batch(vec![
        designos_doc::Command::SetImages { add, remove: vec![] },
        designos_doc::Command::Insert { parent: page, index: 0, nodes: vec![node] },
    ]);
    s.shell.app.hist.exec(&mut s.shell.app.doc, cmd).unwrap();

    let _ = sample_page(&s.app().doc, page, designos_doc::Vec2::new(4.0, 4.0));
    assert_eq!(designos_render::converted_image_bytes() - base_bytes, 8 * 8 * 4);

    // Undo removes the image from `doc.images` and moves its bytes onto `hist`'s redo stack.
    s.shell.app.undo();
    assert!(!s.app().doc.images.contains_key("undone-tab-img"));

    // While the tab is still open, closing another tab keeps the redo-owned image pixmap cached.
    s.shell.app.open_new_design_tab();
    s.shell.app.close_tab(1);
    assert_eq!(designos_render::converted_image_bytes() - base_bytes, 8 * 8 * 4, "surviving tab's redo stack still owns the image bytes");

    let active = s.app().tabs.active_idx;
    s.shell.app.close_tab(active);
    assert_eq!(designos_render::converted_image_bytes(), base_bytes, "closing the tab after undoing its image addition drops the converted pixmap");
}

#[test]
fn two_open_tabs_sharing_one_image_keep_one_pixmap_after_one_closes() {
    let mut s = Sim::new();
    let (doc_a, page_a) = p3_doc_with_png([95, 150, 215], "shared-img");
    let (doc_b, page_b) = p3_doc_with_png([95, 150, 215], "shared-img");
    designos_render::retain_converted_images(std::iter::empty());

    s.shell.app.replace_document(doc_a, None);
    let base_bytes = designos_render::converted_image_bytes();
    let base_conv = designos_render::image_conversions();
    let _ = sample_page(&s.app().doc, page_a, designos_doc::Vec2::new(4.0, 4.0));
    assert_eq!(designos_render::converted_image_bytes() - base_bytes, 8 * 8 * 4);
    assert_eq!(designos_render::image_conversions() - base_conv, 1);

    s.shell.app.open_new_design_tab();
    s.shell.app.replace_document(doc_b, None);
    let _ = sample_page(&s.app().doc, page_b, designos_doc::Vec2::new(4.0, 4.0));
    assert_eq!(designos_render::converted_image_bytes() - base_bytes, 8 * 8 * 4, "shared image stores only one pixmap");
    assert_eq!(designos_render::image_conversions() - base_conv, 1);

    s.shell.app.close_tab(1);
    assert_eq!(designos_render::converted_image_bytes() - base_bytes, 8 * 8 * 4, "closing one tab keeps the shared pixmap");
    let _ = sample_page(&s.app().doc, page_a, designos_doc::Vec2::new(4.0, 4.0));
    assert_eq!(designos_render::image_conversions() - base_conv, 1, "remaining tab reuses the shared pixmap without reconverting");

    s.shell.app.replace_document(Document::new("empty"), None);
    assert_eq!(designos_render::converted_image_bytes(), base_bytes);
}

#[test]
fn closing_a_version_tab_keeps_a_parked_tabs_converted_image() {
    let tmp = std::env::temp_dir().join(format!("designos_1805_parked_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let mut s = Sim::new();
    let (doc_a, page_a) = p3_doc_with_png([11, 222, 33], "parked-a-img");
    designos_render::retain_converted_images(std::iter::empty());
    s.shell.app.replace_document(doc_a, None);
    let base_bytes = designos_render::converted_image_bytes();
    let one = 8 * 8 * 4;
    let _ = sample_page(&s.app().doc, page_a, designos_doc::Vec2::new(4.0, 4.0));
    assert_eq!(designos_render::converted_image_bytes() - base_bytes, one);

    s.shell.app.open_new_design_tab();
    s.shell.app.attach_library(tmp.clone());
    s.shell.app.new_library_file(None);
    let (ver_doc, ver_page) = p3_doc_with_png([190, 60, 70], "parked-ver-img");
    s.shell.app.doc = ver_doc;
    s.shell.app.page = ver_page;
    assert!(s.shell.app.save_named_version("v1", ""));
    let lf_id = s.app().lib_file.as_ref().unwrap().id.clone();
    let v_n = s.app().lib.as_ref().unwrap().list_versions(&lf_id)[0].n;
    let mut cur = Document::new("cur");
    let cp = cur.add_page("p");
    cur.color_profile = ColorProfile::DisplayP3;
    s.shell.app.doc = cur;
    s.shell.app.page = cp;
    s.shell.app.save_library_file().unwrap();
    assert_eq!(designos_render::converted_image_bytes() - base_bytes, one, "the parked tab is still cached before the version opens");

    s.shell.app.view_version(v_n);
    let active = s.app().tabs.active_idx;
    s.shell.app.close_tab(active);
    assert_eq!(designos_render::converted_image_bytes() - base_bytes, one, "the parked tab keeps its pixmap when the tab showing a version closes");
    let _ = std::fs::remove_dir_all(&tmp);
}

/// A library tab with an edit the debounced autosave has not written yet, next to a parked tab.
fn library_tab_with_unsaved_edit(s: &mut Sim, dir: &std::path::Path) -> (String, usize) {
    let _ = std::fs::remove_dir_all(dir);
    s.shell.app.replace_document(Document::new("other"), None);
    s.shell.app.open_new_design_tab();
    s.shell.app.attach_library(dir.to_path_buf());
    s.shell.app.new_library_file(None);
    s.shell.app.save_library_file().unwrap();
    let lf_id = s.app().lib_file.as_ref().unwrap().id.clone();
    let page = s.app().page;
    let id = s.shell.app.doc.alloc_id();
    let cmd = designos_doc::Command::Insert { parent: page, index: 0, nodes: vec![DNode::rectangle(id, DRect::new(0.0, 0.0, 20.0, 20.0))] };
    s.shell.app.hist.exec(&mut s.shell.app.doc, cmd).unwrap();
    let on_disk = s.app().lib.as_ref().unwrap().load_file(&lf_id).unwrap().nodes.len();
    (lf_id, on_disk)
}

#[test]
fn closing_a_library_tab_writes_its_unsaved_edit_once() {
    let tmp = std::env::temp_dir().join(format!("designos_1805_flush_{}", std::process::id()));
    let mut s = Sim::new();
    let (lf_id, before) = library_tab_with_unsaved_edit(&mut s, &tmp);
    let lib = s.app().lib.clone().unwrap();
    let project_id = s.app().lib_file.as_ref().unwrap().project_id.clone();
    let path = lib.path("projects").join(&project_id).join("files").join(format!("{lf_id}.designos"));
    let active = s.app().tabs.active_idx;
    s.shell.app.close_tab(active);
    let after = lib.load_file(&lf_id).unwrap().nodes.len();
    assert_eq!(after, before + 1, "the edit the autosave had not written yet reaches the file when its tab closes");

    // Nothing is left to write: a library tab whose document equals the file is not written again on close.
    s.shell.app.open_library_file(&lf_id);
    let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();
    let active = s.app().tabs.active_idx;
    s.shell.app.close_tab(active);
    assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), mtime, "no second write for an unchanged tab");
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn closing_a_library_tab_after_dont_save_writes_nothing() {
    let tmp = std::env::temp_dir().join(format!("designos_1805_refused_{}", std::process::id()));
    let mut s = Sim::new();
    let (lf_id, before) = library_tab_with_unsaved_edit(&mut s, &tmp);
    let lib = s.app().lib.clone().unwrap();
    s.shell.app.autosave.edits_refused = true;
    let active = s.app().tabs.active_idx;
    s.shell.app.close_tab(active);
    assert_eq!(lib.load_file(&lf_id).unwrap().nodes.len(), before, "edits the user refused to save are not written when the tab closes");
    let _ = std::fs::remove_dir_all(&tmp);
}
