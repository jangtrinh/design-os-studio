//! SVG import through the window: paste, drop, open, and the copy-as-SVG cycle.

use super::*;
use designos_doc::NodeKind;
use std::path::PathBuf;

const ICON: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="32" viewBox="0 0 48 32">
  <rect id="body" width="48" height="32" rx="6" fill="#4F46E5"/>
  <circle cx="24" cy="16" r="8" fill="#FFFFFF"/>
</svg>"##;

fn tmp(name: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("designos_svg_test_{}_{name}", std::process::id()));
    p
}

impl Sim {
    /// One frame with synthetic dropped files.
    fn drop_files(&mut self, files: Vec<egui::DroppedFile>) {
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
        let out = ctx.run_ui(raw, |ui| shell.draw(ui));
        self.cursor = out.platform_output.cursor_icon;
        self.frame(vec![]);
    }

    fn app_mut(&mut self) -> &mut App {
        &mut self.shell.app
    }

    fn new_ids(&self, before: &std::collections::BTreeSet<NodeId>) -> Vec<NodeId> {
        self.app().doc.nodes.keys().filter(|k| !before.contains(k)).copied().collect()
    }

    fn ids(&self) -> std::collections::BTreeSet<NodeId> {
        self.app().doc.nodes.keys().copied().collect()
    }

    /// Page-space center of a node's top-level box.
    fn center_of(&self, id: NodeId) -> (f32, f32) {
        let d = &self.app().doc;
        let n = d.get(id).unwrap();
        let c = d.world_transform(id).apply(DV::new(n.size.x / 2.0, n.size.y / 2.0));
        (c.x, c.y)
    }

    fn viewport_center(&self) -> (f32, f32) {
        let a = self.app();
        let c = a.view.to_page(a.canvas_rect.min, a.canvas_rect.center());
        (c.x, c.y)
    }
}

#[test]
fn pasting_svg_text_creates_layers_at_the_viewport_center_as_one_undo_step() {
    let mut s = Sim::new();
    let before = s.ids();
    let history = s.app().hist.can_undo();
    s.frame(vec![Event::Paste(ICON.to_string())]);
    let added = s.new_ids(&before);
    assert_eq!(added.len(), 3, "frame + rect + circle: {added:?}");

    // selected: the new frame
    let sel = s.app().selection.clone();
    assert_eq!(sel.len(), 1);
    let root = s.app().doc.get(sel[0]).unwrap();
    assert!(matches!(root.kind, NodeKind::Frame(_)));
    assert_eq!(root.name, "svg");
    assert_eq!((root.size.x, root.size.y), (48.0, 32.0));
    let kinds: Vec<_> = root.children.iter().map(|c| std::mem::discriminant(&s.app().doc.get(*c).unwrap().kind)).collect();
    assert_eq!(kinds, vec![std::mem::discriminant(&NodeKind::Rectangle), std::mem::discriminant(&NodeKind::Ellipse(Default::default()))]);

    // centered on the viewport
    let (cx, cy) = s.center_of(sel[0]);
    let (vx, vy) = s.viewport_center();
    assert!((cx - vx).abs() <= 1.0 && (cy - vy).abs() <= 1.0, "({cx},{cy}) vs viewport ({vx},{vy})");

    // one undo removes everything, one redo brings it back
    s.cmd_key(Key::Z);
    assert_eq!(s.ids(), before);
    assert_eq!(s.app().hist.can_undo(), history);
    s.cmd_key(Key::Y);
    assert_eq!(s.new_ids(&before).len(), 3);
}

#[test]
fn pasting_other_text_makes_a_text_layer_and_bad_svg_reports_an_error() {
    let mut s = Sim::new();
    let before = s.ids();
    s.frame(vec![Event::Paste("just some words".to_string())]);
    let added = s.new_ids(&before);
    assert_eq!(added.len(), 1, "plain text becomes exactly one text layer");
    assert!(matches!(&s.app().doc.get(added[0]).unwrap().kind, NodeKind::Text(t) if t.content == "just some words"));
    let before = s.ids();
    s.frame(vec![Event::Paste("<svg xmlns=\"http://www.w3.org/2000/svg\"".to_string())]);
    assert_eq!(s.ids(), before, "broken SVG markup adds nothing, not even as text");
    let toast = s.app().toast.as_ref().expect("an error toast");
    assert!(toast.error && toast.text.contains("SVG"), "{}", toast.text);
}

#[test]
fn pasting_into_a_selected_frame_nests_the_layers() {
    let mut s = Sim::new();
    let hero = s.by_name("Hero");
    s.app_mut().selection = vec![hero];
    let before = s.ids();
    s.frame(vec![Event::Paste(ICON.to_string())]);
    let root = s.app().selection[0];
    assert!(s.new_ids(&before).contains(&root));
    assert_eq!(s.app().doc.get(root).unwrap().parent.map(|p| p == hero || s.app().doc.get(hero).unwrap().parent == Some(p)), Some(true));
}

#[test]
fn dropping_an_svg_file_places_it_at_the_drop_point() {
    let path = tmp("drop-me.svg");
    std::fs::write(&path, ICON).unwrap();
    let mut s = Sim::new();
    let before = s.ids();
    // an empty spot of the canvas
    let target = s.screen(60.0, 900.0);
    s.mv(target);
    s.drop_files(vec![egui::DroppedFile { path: Some(path.clone()), ..Default::default() }]);
    std::fs::remove_file(&path).ok();
    let added = s.new_ids(&before);
    assert_eq!(added.len(), 3);
    let root = s.app().selection[0];
    let n = s.app().doc.get(root).unwrap();
    assert!(n.name.starts_with("designos_svg_test_") && n.name.ends_with("drop-me"), "named after the file: {}", n.name);
    let (cx, cy) = s.center_of(root);
    let page = s.app().view.to_page(s.app().canvas_rect.min, target);
    assert!((cx - page.x).abs() <= 1.0 && (cy - page.y).abs() <= 1.0, "({cx},{cy}) vs drop ({},{})", page.x, page.y);
    s.cmd_key(Key::Z);
    assert_eq!(s.ids(), before);
}

#[test]
fn dropping_svg_bytes_without_a_path_works_too() {
    let mut s = Sim::new();
    let before = s.ids();
    let f = egui::DroppedFile { name: "logo.svg".into(), bytes: Some(ICON.as_bytes().into()), ..Default::default() };
    s.drop_files(vec![f]);
    let root = s.app().selection[0];
    assert_eq!(s.app().doc.get(root).unwrap().name, "logo");
    assert_eq!(s.new_ids(&before).len(), 3);
}

#[test]
fn opening_an_svg_creates_a_new_document_with_the_svg_on_page_1() {
    let path = tmp("opened.svg");
    std::fs::write(&path, ICON).unwrap();
    let mut s = Sim::new();
    s.app_mut().open_path(path.clone());
    std::fs::remove_file(&path).ok();
    let app = s.app();
    assert_eq!(app.doc.pages.len(), 1);
    let page = app.doc.get(app.doc.pages[0]).unwrap();
    assert_eq!(page.name, "Page 1");
    assert_eq!(page.children.len(), 1);
    let frame = app.doc.get(page.children[0]).unwrap();
    assert!(app.doc.name.ends_with("opened") && frame.name == app.doc.name);
    assert_eq!((frame.transform.tx, frame.transform.ty, frame.size.x, frame.size.y), (0.0, 0.0, 48.0, 32.0));
    assert_eq!(frame.children.len(), 2);
    // never saved over the .svg
    assert!(app.file_path.is_none());
    assert!(!app.hist.can_undo());
}

#[test]
fn a_broken_svg_file_leaves_the_document_alone_and_says_why() {
    let path = tmp("broken.svg");
    std::fs::write(&path, "<svg").unwrap();
    let mut s = Sim::new();
    let before = s.ids();
    s.app_mut().open_path(path.clone());
    std::fs::remove_file(&path).ok();
    assert_eq!(s.ids(), before);
    assert!(s.app().toast.as_ref().is_some_and(|t| t.error && t.text.contains("Cannot import SVG")));
}

#[test]
fn copy_as_svg_then_paste_round_trips_the_layer() {
    let mut s = Sim::new();
    let button = s.by_name("Button");
    let (orig_w, orig_h) = {
        let n = s.app().doc.get(button).unwrap();
        (n.size.x, n.size.y)
    };
    // exactly what the export panel's "Copy as SVG" puts on the clipboard
    let svg = designos_export::export_svg(&s.app().export_doc(), button, &designos_export::SvgOptions::default());
    assert!(designos_svg::looks_like_svg(&svg));

    let before = s.ids();
    s.frame(vec![Event::Paste(svg)]);
    let added = s.new_ids(&before);
    assert!(added.len() >= 2, "{added:?}");
    let root = s.app().selection[0];
    let frame = s.app().doc.get(root).unwrap();
    assert!(matches!(frame.kind, NodeKind::Frame(_)));
    // the viewBox includes the shadow overflow, so the frame is at least the layer's size
    assert!(frame.size.x >= orig_w - 0.5 && frame.size.y >= orig_h - 0.5, "{:?} vs {orig_w}x{orig_h}", frame.size);
    // the layer's own geometry shows up as a rounded Rectangle
    let rect = added
        .iter()
        .map(|id| s.app().doc.get(*id).unwrap())
        .find(|n| matches!(n.kind, NodeKind::Rectangle) && (n.size.x - orig_w).abs() < 0.5 && (n.size.y - orig_h).abs() < 0.5);
    assert!(rect.is_some(), "no {orig_w}x{orig_h} rectangle among {:?}", added.iter().map(|id| s.app().doc.get(*id).unwrap().name.clone()).collect::<Vec<_>>());

    // and it renders: the pasted frame is not empty
    let pm = designos_render::render_node(&s.app().doc, root, 1.0).unwrap();
    assert!(pm.data().chunks_exact(4).any(|p| p[3] > 0));
}
