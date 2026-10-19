//! UI tests for shared libraries: the Libraries modal, library components in
//! the Assets panel and the update review.

use super::*;
use crate::app::RailItem;
use designos_doc::{instances_of, library_instance_info, Command, NodeKind, OverrideField};
use std::path::{Path, PathBuf};

fn tmp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("designos-ui-libs-{name}-{}", designos_library::new_id()));
    let _ = std::fs::remove_dir_all(&p);
    p
}

fn sim(root: &Path) -> Sim {
    let mut s = Sim::new();
    s.shell.app.libs.set_root(root.to_path_buf());
    s
}

fn frames(s: &mut Sim, n: usize) {
    for _ in 0..n {
        s.frame(vec![]);
    }
}

/// File A publishes its components; returns after the first publish.
fn publish_from_a(root: &Path) -> Sim {
    let mut a = sim(root);
    a.shell.app.open_libraries();
    frames(&mut a, 3);
    a.shell.app.libs.description = "First version".into();
    let b = a.rect_of("lib-publish");
    a.click(b.center());
    frames(&mut a, 2);
    a
}

fn enable_in_b(root: &Path) -> Sim {
    let mut b = sim(root);
    b.shell.app.new_document();
    b.shell.app.open_libraries();
    frames(&mut b, 3);
    let t = b.rect_of("lib-toggle:Untitled");
    b.click(t.center());
    frames(&mut b, 2);
    b
}

#[test]
fn modal_publishes_this_file_and_lists_the_library() {
    let root = tmp("publish");
    let a = publish_from_a(&root);
    let info = a.app().doc.library.published.clone().expect("published");
    assert_eq!(info.version, 1);
    assert_eq!(info.description, "First version");
    assert!(root.join("libraries").join(&info.library_id).join("1.json").exists());
    let idx = designos_library::Library::open(&root).unwrap().library_index(&info.library_id).unwrap();
    assert_eq!(idx.versions[0].description, "First version");
    assert!(idx.components >= 1);
    assert!(!idx.versions[0].changelog.is_empty());
    // Nothing changed since: the modal shows no changes and the button is disabled.
    let mut a = a;
    a.shell.app.refresh_libraries();
    frames(&mut a, 2);
    assert!(a.app().libs.changes.is_empty());
    let before = std::fs::read_dir(root.join("libraries").join(&info.library_id)).unwrap().count();
    let b = a.rect_of("lib-publish");
    a.click(b.center());
    assert_eq!(std::fs::read_dir(root.join("libraries").join(&info.library_id)).unwrap().count(), before, "disabled button does not republish");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn changelog_lists_changed_assets_since_last_publish() {
    let root = tmp("changelog");
    let mut a = publish_from_a(&root);
    let main = a.by_name("Primary button");
    a.shell.app.edit_nodes(&[main], |n| n.size.x += 30.0);
    a.shell.app.refresh_libraries();
    frames(&mut a, 2);
    let ch = a.app().libs.changes.clone();
    assert_eq!(ch.len(), 1, "{ch:?}");
    assert_eq!(ch[0].name, "Primary button");
    assert_eq!(ch[0].change, designos_doc::UpdateKind::Changed);
    a.rect_of("lib-change:Primary button");
    let b = a.rect_of("lib-publish");
    a.click(b.center());
    assert_eq!(a.app().doc.library.published.as_ref().unwrap().version, 2);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn enable_library_drag_component_and_review_updates() {
    let root = tmp("flow");
    let mut a = publish_from_a(&root);
    let mut b = enable_in_b(&root);

    // Enable pins the latest version and the modal toggle reflects it.
    assert_eq!(b.app().doc.library.enabled.len(), 1);
    assert_eq!(b.app().doc.library.enabled[0].version, 1);
    assert_eq!(b.app().doc.library.enabled[0].name, "Untitled");

    // The library's components show up in the Assets panel and can be dragged out.
    b.shell.app.close_libraries();
    b.shell.app.ui.rail = Some(RailItem::Assets);
    frames(&mut b, 6);
    let row = b.rect_of("asset:Primary button");
    let canvas_mid = b.app().canvas_rect.center();
    b.drag(row.center(), canvas_mid);
    let inst = b.app().selection.first().copied().expect("instance selected");
    assert!(matches!(b.app().doc.get(inst).unwrap().kind, NodeKind::Instance { .. }));
    let info = library_instance_info(&b.app().doc, inst).expect("library instance");
    assert_eq!((info.library_name.as_str(), info.asset_name.as_str()), ("Untitled", "Primary button"));
    let lp = b.app().doc.library_page.expect("hidden library page");
    assert!(!b.app().doc.pages.contains(&lp));

    // Library copies cannot be selected: sanitize drops them and they are not on a page.
    let comp = designos_doc::instance_component(b.app().doc.get(inst).unwrap()).unwrap();
    b.shell.app.selection = vec![comp];
    b.shell.app.sanitize();
    assert!(b.app().selection.is_empty());

    // Inspector row names the library.
    b.shell.app.selection = vec![inst];
    frames(&mut b, 3);
    b.rect_of("instance-library");

    // Override a label in the instance.
    let label =
        b.app().doc.get(inst).unwrap().children.iter().copied().find(|c| matches!(b.app().doc.get(*c).unwrap().kind, NodeKind::Text(_))).expect("a text layer");
    let mut n = b.app().doc.get(label).unwrap().clone();
    if let NodeKind::Text(t) = &mut n.kind {
        t.content = "Mine".into();
    }
    n.component.overrides.insert(OverrideField::Characters);
    let mut hist_doc = std::mem::take(&mut b.shell.app.doc);
    Command::Update { node: Box::new(n) }.apply(&mut hist_doc).unwrap();
    b.shell.app.doc = hist_doc;
    let w0 = b.app().doc.get(inst).unwrap().size.x;

    // A changes the component and republishes.
    let main = a.by_name("Primary button");
    a.shell.app.edit_nodes(&[main], |n| n.size.x += 40.0);
    a.shell.app.refresh_libraries();
    frames(&mut a, 2);
    let pb = a.rect_of("lib-publish");
    a.click(pb.center());
    assert_eq!(a.app().doc.library.published.as_ref().unwrap().version, 2);

    // B notices the new version and offers a review.
    frames(&mut b, 60);
    assert!(b.app().update_banner_visible());
    b.rect_of("library-updates-banner");
    let rv = b.rect_of("library-updates-review");
    b.click(rv.center());
    frames(&mut b, 2);
    assert!(b.app().libs.review);
    b.rect_of("lib-upd:Primary button");

    // "Update selected" is disabled until something is checked.
    let sel_btn = b.rect_of("library-update-selected");
    b.click(sel_btn.center());
    assert!(b.app().libs.review, "nothing selected: stays open");
    let check = b.rect_of("lib-check:Primary button");
    b.click(check.center());
    assert_eq!(b.app().libs.selected.len(), 1);
    let sel_btn = b.rect_of("library-update-selected");
    b.click(sel_btn.center());
    frames(&mut b, 2);
    assert!(!b.app().libs.review);
    assert_eq!(b.app().doc.get(inst).unwrap().size.x, w0 + 40.0, "instance follows the updated library component");
    let NodeKind::Text(t) = &b.app().doc.get(label).unwrap().kind else { panic!() };
    assert_eq!(t.content, "Mine", "instance override survives the update");
    assert_eq!(b.app().doc.library.enabled[0].version, 2);

    // One undo step reverts the whole update.
    b.cmd_key(Key::Z);
    assert_eq!(b.app().doc.get(inst).unwrap().size.x, w0);
    assert_eq!(b.app().doc.library.enabled[0].version, 1);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn update_all_and_dismissing_the_banner() {
    let root = tmp("all");
    let mut a = publish_from_a(&root);
    let mut b = enable_in_b(&root);
    let key = {
        let snap = b.shell.app.latest_snapshot(&b.app().doc.library.enabled[0].id.clone()).unwrap();
        snap.assets.iter().find(|x| x.name == "Primary button").unwrap().key.clone()
    };
    let lib_id = b.app().doc.library.enabled[0].id.clone();
    b.shell.app.drop_library_asset(&lib_id, &key, designos_doc::Vec2::new(40.0, 40.0)).expect("dropped");
    let main = a.by_name("Primary button");
    a.shell.app.edit_nodes(&[main], |n| n.size.y += 8.0);
    a.shell.app.refresh_libraries();
    frames(&mut a, 2);
    let pb = a.rect_of("lib-publish");
    a.click(pb.center());

    // The banner can be dismissed for this version...
    b.shell.app.close_libraries();
    frames(&mut b, 60);
    assert!(b.app().update_banner_visible());
    let x = b.rect_of("library-updates-dismiss");
    b.click(x.center());
    frames(&mut b, 2);
    assert!(!b.app().update_banner_visible());
    // ...but the Libraries modal still offers the review.
    b.shell.app.open_libraries();
    frames(&mut b, 3);
    let upd = b.rect_of("lib-update:Untitled");
    b.click(upd.center());
    frames(&mut b, 2);
    assert!(b.app().libs.review);
    let all = b.rect_of("library-update-all");
    b.click(all.center());
    frames(&mut b, 2);
    assert!(!b.app().libs.review);
    assert_eq!(b.app().doc.library.enabled[0].version, 2);
    let comp = b.app().doc.library.external.values().find_map(|e| e.node).unwrap();
    assert!(!instances_of(&b.app().doc, designos_doc::components_of(&b.app().doc, comp)[0]).is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn removed_library_assets_are_flagged_in_the_review() {
    let root = tmp("removed");
    let mut a = publish_from_a(&root);
    let mut b = enable_in_b(&root);
    let lib_id = b.app().doc.library.enabled[0].id.clone();
    let key = {
        let snap = b.shell.app.latest_snapshot(&lib_id).unwrap();
        snap.assets.iter().find(|x| x.name == "Primary button").unwrap().key.clone()
    };
    b.shell.app.drop_library_asset(&lib_id, &key, designos_doc::Vec2::new(40.0, 40.0)).expect("dropped");

    // A deletes the component and republishes.
    let main = a.by_name("Primary button");
    a.shell.app.selection = vec![main];
    a.shell.app.doc = {
        let mut d = std::mem::take(&mut a.shell.app.doc);
        Command::Remove { id: main }.apply(&mut d).unwrap();
        d
    };
    a.shell.app.refresh_libraries();
    frames(&mut a, 2);
    let pb = a.rect_of("lib-publish");
    a.click(pb.center());

    b.shell.app.close_libraries();
    frames(&mut b, 3);
    b.shell.app.libs.invalidate();
    b.shell.app.open_updates_review();
    frames(&mut b, 3);
    let all = b.app().libs.pending_all();
    let u = all[0].updates.iter().find(|u| u.name == "Primary button").expect("listed");
    assert_eq!(u.change, designos_doc::UpdateKind::Removed);
    b.rect_of("lib-upd:Primary button");
    // The local copy stays and instances keep working.
    assert!(b.app().doc.library.external.contains_key(&designos_doc::local_key(&lib_id, &key)));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn go_to_main_of_unavailable_library_shows_a_read_only_preview() {
    let root = tmp("preview");
    let _a = publish_from_a(&root);
    let mut b = enable_in_b(&root);
    let lib_id = b.app().doc.library.enabled[0].id.clone();
    let key = {
        let snap = b.shell.app.latest_snapshot(&lib_id).unwrap();
        snap.assets.iter().find(|x| x.name == "Primary button").unwrap().key.clone()
    };
    let inst = b.shell.app.drop_library_asset(&lib_id, &key, designos_doc::Vec2::new(40.0, 40.0)).unwrap();
    // The source file was never saved to disk: no file to open.
    b.shell.app.go_to_main(inst);
    frames(&mut b, 3);
    assert!(b.app().libs.preview.is_some());
    b.rect_of("library-preview-done");
    let done = b.rect_of("library-preview-done");
    b.click(done.center());
    frames(&mut b, 2);
    assert!(b.app().libs.preview.is_none());
    let _ = std::fs::remove_dir_all(root);
}

/// File "A" with a red color variable and a red color style, published.
fn publish_red(root: &Path) -> (Sim, String) {
    use designos_doc::{Color, Paint, Style, StyleMeta, StyleProperty, VariableKind, VariableStore, VariableValue};
    let mut a = sim(root);
    a.shell.app.new_document();
    let mut store = VariableStore::from_doc(&a.app().doc);
    let coll = store.add_collection("Brand");
    let mode = store.collections[&coll].default_mode.clone();
    let id = store.add_variable(&coll, "Brand/Red", VariableKind::Color).unwrap();
    store.set_value(&id, &mode, VariableValue::Color(Color::rgb(255, 0, 0)));
    let n = store.add_variable(&coll, "Brand/Radius", VariableKind::Number).unwrap();
    store.set_value(&n, &mode, VariableValue::Number(8.0));
    assert!(a.shell.app.commit_store(store, &[], &[]));
    a.shell.app.doc.styles.insert("Brand/Red".into(), Style::Fill(vec![Paint::solid(Color::rgb(255, 0, 0))]));
    a.shell
        .app
        .doc
        .style_meta
        .insert("Brand/Red".into(), StyleMeta { name: "Brand/Red".into(), key: None, description: String::new(), property: StyleProperty::Fill, bindings: Default::default() });
    assert!(a.shell.app.publish_file());
    let lib_id = a.app().doc.library.published.clone().unwrap().library_id;
    (a, lib_id)
}

/// A second document with one white 40x40 rectangle (selected) that uses the library.
fn consumer_with_rect(root: &Path, lib_id: &str) -> (Sim, designos_doc::NodeId) {
    use designos_doc::{Color, Node, Paint, Rect};
    let mut b = sim(root);
    b.shell.app.new_document();
    let page = b.app().page;
    let id = b.shell.app.doc.alloc_id();
    let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 40.0, 40.0));
    n.name = "Box".into();
    n.fills = vec![Paint::solid(Color::rgb(255, 255, 255))];
    b.shell.app.doc.insert(page, n).unwrap();
    b.shell.app.set_library_enabled(lib_id, true);
    b.shell.app.selection = vec![id];
    b.shell.app.changed();
    frames(&mut b, 3);
    (b, id)
}

fn box_pixel(s: &Sim, id: designos_doc::NodeId) -> [u8; 3] {
    let pm = designos_render::render_node(&s.app().doc, id, 1.0).unwrap();
    let p = pm.pixel(20, 20).unwrap();
    [p.red(), p.green(), p.blue()]
}

#[test]
fn library_variables_bind_through_the_picker_in_one_undo_step() {
    let root = tmp("varpick");
    let (_a, lib_id) = publish_red(&root);
    let (mut b, id) = consumer_with_rect(&root, &lib_id);
    assert!(b.app().doc.variables.is_empty(), "nothing is copied before it is used");
    assert_eq!(box_pixel(&b, id), [255, 255, 255]);

    let sw = b.rect_of("sw:false:0");
    b.mv(Pos2::new(sw.right() + 40.0, sw.center().y));
    b.frame(vec![]);
    b.click(b.rect_of("bind:fills/0").center());
    b.frame(vec![]);
    b.rect_of("pick:Brand/Red");
    let radius = b.ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new(("test-rect", "pick:Brand/Radius".to_string()))));
    assert!(radius.is_none(), "number variables are filtered out of the color picker");
    b.click(b.rect_of("pick:Brand/Red").center());

    let local = b.app().doc.get(id).unwrap().variables.bindings.get("fills/0").cloned().expect("bound");
    assert_eq!(b.app().doc.variables[&local].name, "Brand/Red");
    assert!(b.app().doc.library.external.keys().any(|k| k.starts_with(&lib_id)), "copy registered");
    assert_eq!(box_pixel(&b, id), [255, 0, 0], "the bound color renders");

    // One undo removes the binding and the copy together.
    b.cmd_key(Key::Z);
    assert!(b.app().doc.get(id).unwrap().variables.bindings.is_empty());
    assert!(b.app().doc.variables.is_empty() && b.app().doc.variable_collections.is_empty(), "the copy is gone too");
    assert!(b.app().doc.library.external.is_empty());
    assert_eq!(box_pixel(&b, id), [255, 255, 255]);
    assert_eq!(b.app().doc.library.enabled.len(), 1, "the library stays enabled");
    // and redo brings both back
    b.mods = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
    b.key(Key::Z);
    b.mods = egui::Modifiers::NONE;
    assert_eq!(box_pixel(&b, id), [255, 0, 0]);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn color_picker_libraries_tab_lists_library_styles_and_applies_one_undoably() {
    let root = tmp("stylepick");
    let (_a, lib_id) = publish_red(&root);
    let (mut b, id) = consumer_with_rect(&root, &lib_id);
    b.click(b.rect_of("sw:false:0").center());
    b.frame(vec![]);
    b.click(b.rect_of("picker-tab:Libraries").center());
    b.frame(vec![]);
    let row = b.rect_of("picker-style:Brand/Red");
    b.click(row.center());
    let n = b.app().doc.get(id).unwrap();
    let key = n.style_refs.get("fill").cloned().expect("style applied");
    assert!(key.starts_with(&lib_id), "{key}");
    assert!(b.app().doc.styles.contains_key(&key));
    assert_eq!(box_pixel(&b, id), [255, 0, 0]);
    assert!(b.app().props.picker.is_none());

    b.cmd_key(Key::Z);
    assert!(b.app().doc.styles.is_empty() && b.app().doc.style_meta.is_empty(), "the imported style is undone");
    assert!(b.app().doc.get(id).unwrap().style_refs.is_empty());
    assert!(b.app().doc.library.external.is_empty());
    assert_eq!(box_pixel(&b, id), [255, 255, 255]);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn creating_a_style_from_the_picker_is_one_undo_step() {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let hero = s.by_name("Hero");
    let before = s.app().doc.styles.clone();
    s.click(s.rect_of("sw:false:0").center());
    s.frame(vec![]);
    s.click(s.rect_of("picker-add-style").center());
    s.frame(vec![]);
    let key = s.app().doc.get(hero).unwrap().style_refs.get("fill").cloned().expect("linked to the new style");
    assert!(s.app().doc.styles.contains_key(&key) && s.app().doc.style_meta.contains_key(&key));
    s.cmd_key(Key::Z);
    assert_eq!(format!("{:?}", s.app().doc.styles), format!("{before:?}"), "undo restores the previous styles");
    assert!(!s.app().doc.style_meta.contains_key(&key));
    assert!(s.app().doc.get(hero).unwrap().style_refs.is_empty(), "linking was part of the same step");
}

#[test]
fn updating_a_library_style_is_undoable() {
    use designos_doc::{Color, Paint, Style};
    let root = tmp("styleupd");
    let (mut a, lib_id) = publish_red(&root);
    let (mut b, id) = consumer_with_rect(&root, &lib_id);
    let key = b.shell.app.library_styles().into_iter().find(|s| s.name == "Brand/Red").unwrap().key;
    let local = b.shell.app.use_library_style(&lib_id, &key).expect("imported");
    let red = Style::Fill(vec![Paint::solid(Color::rgb(255, 0, 0))]);
    assert_eq!(format!("{:?}", b.app().doc.styles[&local]), format!("{red:?}"));
    b.shell.app.apply_style(&[id], &local);

    // A turns the style blue and republishes; B applies the update.
    a.shell.app.doc.styles.insert("Brand/Red".into(), Style::Fill(vec![Paint::solid(Color::rgb(0, 0, 255))]));
    assert!(a.shell.app.publish_file());
    assert_eq!(a.app().doc.library.published.as_ref().unwrap().version, 2);
    let n = b.shell.app.apply_library_updates(&lib_id, std::slice::from_ref(&key));
    assert_eq!(n, 1);
    assert_eq!(format!("{:?}", b.app().doc.styles[&local]), format!("{:?}", Style::Fill(vec![Paint::solid(Color::rgb(0, 0, 255))])));
    assert_eq!(box_pixel(&b, id), [0, 0, 255]);
    b.cmd_key(Key::Z);
    assert_eq!(format!("{:?}", b.app().doc.styles[&local]), format!("{red:?}"), "undo restores the previous style");
    assert_eq!(box_pixel(&b, id), [255, 0, 0]);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn shared_libraries_follow_the_projects_library_root_and_open_its_files() {
    let root = tmp("oneroot");
    let mut s = Sim::with(Options { library: Some(root.clone()), ..Default::default() });
    assert_eq!(s.app().libs.root.as_deref(), Some(root.as_path()), "attach_library points the shared libraries at the same root");
    assert_eq!(s.shell.app.library().unwrap().root(), root.as_path());

    // Publish a projects-library file: the library remembers the file id.
    let home_file = s.app().lib.as_ref().unwrap().all_files()[0].id.clone();
    s.shell.app.open_library_file(&home_file);
    assert!(s.shell.app.publish_file());
    let lib_id = s.app().doc.library.published.clone().unwrap().library_id;
    let idx = designos_library::Library::open(&root).unwrap().library_index(&lib_id).unwrap();
    assert_eq!(idx.source_file.as_deref(), Some(home_file.as_str()));
    s.shell.app.flush_autosave();

    // Another file uses the library; "Go to main component" opens the first one through the home store.
    s.shell.app.new_library_file(None);
    assert_ne!(s.app().lib_file.as_ref().unwrap().id, home_file);
    s.shell.app.set_library_enabled(&lib_id, true);
    let key = {
        let snap = s.shell.app.latest_snapshot(&lib_id).unwrap();
        snap.assets.iter().find(|x| x.name == "Primary button").unwrap().key.clone()
    };
    let inst = s.shell.app.drop_library_asset(&lib_id, &key, designos_doc::Vec2::new(40.0, 40.0)).unwrap();
    s.shell.app.go_to_main(inst);
    assert_eq!(s.app().lib_file.as_ref().unwrap().id, home_file);
    assert!(!s.app().selection.is_empty(), "the main component is selected");
    let _ = std::fs::remove_dir_all(root);
}

/// File B holds an instance whose slot (limits `min`/`max`) already has one child; returns (slot, centre of the slot).
fn slot_with_one_child_in_b(b: &mut Sim, max: u32) -> (NodeId, designos_doc::Vec2) {
    slot_in_b(b, None, max, 1)
}

/// File B holds an instance whose slot has `fill` children; returns (slot, centre of the slot).
fn slot_in_b(b: &mut Sim, min: Option<u32>, max: u32, fill: usize) -> (NodeId, designos_doc::Vec2) {
    use designos_doc::{Node, Rect, SlotPropConfig, Vec2};
    let app = &mut b.shell.app;
    let page = app.page;
    let run = |app: &mut App, cmd: Command| {
        let mut tx = crate::edit::Tx::new();
        tx.cmd(&mut app.doc, cmd).unwrap();
        app.run_tx(tx);
    };
    let host = app.doc.alloc_id();
    run(app, Command::Insert { parent: page, index: 0, nodes: vec![Node::frame(host, Rect::new(600.0, 100.0, 240.0, 120.0))] });
    let slot_frame = app.doc.alloc_id();
    let mut slot_node = Node::frame(slot_frame, Rect::new(10.0, 10.0, 160.0, 80.0));
    slot_node.name = "Body".into();
    run(app, Command::Insert { parent: host, index: 0, nodes: vec![slot_node] });
    let cmd = designos_doc::create_component(&mut app.doc, host);
    run(app, cmd);
    let cmd = designos_doc::convert_to_slot(&mut app.doc, slot_frame);
    run(app, cmd);
    let cmd = designos_doc::set_slot_config(&mut app.doc, host, "Body", SlotPropConfig { min_children: min, max_children: Some(max), ..Default::default() });
    run(app, cmd);
    let cmd = designos_doc::create_instance(&mut app.doc, host, page, Vec2::new(900.0, 300.0));
    run(app, cmd);
    let inst = designos_doc::instances_of(&app.doc, host)[0];
    let slot = *app.doc.get(inst).unwrap().children.iter().find(|c| app.doc.get(**c).unwrap().name == "Body").unwrap();
    for _ in 0..fill {
        let id = app.doc.alloc_id();
        let child = Node::frame(id, Rect::new(0.0, 0.0, 20.0, 20.0));
        let cmd = designos_doc::insert_into_slot(&mut app.doc, slot, 0, vec![child]).unwrap();
        run(app, cmd);
    }
    app.toast = None;
    let c = crate::util::rect_center(&crate::util::world_aabb(&app.doc, slot));
    (slot, c)
}

fn primary_button_key(b: &mut Sim) -> (String, String) {
    let lib_id = b.app().doc.library.enabled[0].id.clone();
    let key = b.shell.app.latest_snapshot(&lib_id).unwrap().assets.iter().find(|x| x.name == "Primary button").unwrap().key.clone();
    (lib_id, key)
}

#[test]
fn a_library_drop_on_a_full_slot_is_refused_and_changes_nothing() {
    let root = tmp("slot-drop-full");
    let _a = publish_from_a(&root);
    let mut b = enable_in_b(&root);
    let (slot, at) = slot_with_one_child_in_b(&mut b, 1);
    let (lib_id, key) = primary_button_key(&mut b);
    let (nodes, kids) = (b.count(), b.app().doc.get(slot).unwrap().children.clone());
    assert!(b.app().doc.library_page.is_none());
    assert!(b.shell.app.drop_library_asset(&lib_id, &key, at).is_none());
    // The import is rolled back: no imported component, no instance. (The empty hidden library page is
    // created before the command runs and stays, as it does when an import is a no-op.)
    let lib_page_kids = b.app().doc.library_page.and_then(|p| b.app().doc.get(p)).map_or(0, |p| p.children.len());
    assert_eq!(lib_page_kids, 0, "no imported component is left behind");
    assert_eq!(b.count(), nodes + usize::from(b.app().doc.library_page.is_some()), "no layer was added");
    assert_eq!(b.app().doc.get(slot).unwrap().children, kids);
    let toast = b.app().toast.as_ref().expect("toast");
    assert!(toast.error && toast.text.contains("maximum"), "{}", toast.text);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_library_drop_on_a_slot_with_room_joins_it_and_marks_the_slot() {
    let root = tmp("slot-drop-room");
    let _a = publish_from_a(&root);
    let mut b = enable_in_b(&root);
    let (slot, at) = slot_with_one_child_in_b(&mut b, 2);
    let (lib_id, key) = primary_button_key(&mut b);
    let dropped = b.shell.app.drop_library_asset(&lib_id, &key, at).expect("dropped");
    assert_eq!(b.app().doc.get(dropped).unwrap().parent, Some(slot));
    assert_eq!(b.app().doc.get(slot).unwrap().children.len(), 2);
    assert!(b.app().doc.get(slot).unwrap().component.overrides.contains(&OverrideField::SlotContent));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_library_drop_into_an_empty_slot_below_its_min_is_allowed() {
    let root = tmp("slot-drop-min");
    let _a = publish_from_a(&root);
    let mut b = enable_in_b(&root);
    let (slot, at) = slot_in_b(&mut b, Some(2), 4, 0);
    let (lib_id, key) = primary_button_key(&mut b);
    let dropped = b.shell.app.drop_library_asset(&lib_id, &key, at).expect("the first of the two required children can be added");
    assert_eq!(b.app().doc.get(slot).unwrap().children, vec![dropped]);
    assert!(b.app().toast.as_ref().is_none_or(|t| !t.error), "no refusal toast");
    let _ = std::fs::remove_dir_all(root);
}

fn tiny_png() -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    image::RgbaImage::new(4, 4).write_to(&mut out, image::ImageFormat::Png).expect("encode png");
    out.into_inner()
}

/// File A publishes a button whose background is an image fill.
fn publish_image_button_from_a(root: &Path) -> Sim {
    let mut a = sim(root);
    let main = a.by_name("Primary button");
    let bg = *a.app().doc.get(main).unwrap().children.iter().find(|c| a.app().doc.get(**c).unwrap().name == "Bg").unwrap();
    a.shell.app.doc.images.insert("pic".into(), tiny_png());
    a.shell.app.edit_nodes(&[bg], |n| {
        n.fills = vec![designos_doc::Paint {
            kind: designos_doc::PaintKind::Image { image_ref: "pic".into(), scale_mode: designos_doc::ImageScaleMode::Fill },
            opacity: 1.0,
            visible: true,
            blend: designos_doc::BlendMode::Normal,
        }]
    });
    a.shell.app.open_libraries();
    frames(&mut a, 3);
    let b = a.rect_of("lib-publish");
    a.click(b.center());
    frames(&mut a, 2);
    a
}

#[test]
fn a_refused_first_library_drop_stores_no_image_bytes() {
    let root = tmp("slot-drop-images");
    let _a = publish_image_button_from_a(&root);
    let mut b = enable_in_b(&root);
    let (slot, at) = slot_with_one_child_in_b(&mut b, 1);
    let (lib_id, key) = primary_button_key(&mut b);
    let snap = b.shell.app.latest_snapshot(&lib_id).unwrap();
    assert!(!snap.images.is_empty(), "the published component carries its image");
    let stored = b.app().doc.images.keys().cloned().collect::<Vec<_>>();
    let kids = b.app().doc.get(slot).unwrap().children.clone();
    assert!(b.shell.app.drop_library_asset(&lib_id, &key, at).is_none());
    assert_eq!(b.app().doc.get(slot).unwrap().children, kids);
    assert_eq!(b.app().doc.images.keys().cloned().collect::<Vec<_>>(), stored, "the refused import left no bytes in the file");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn an_allowed_first_library_drop_keeps_its_image_bytes() {
    let root = tmp("slot-drop-images-ok");
    let _a = publish_image_button_from_a(&root);
    let mut b = enable_in_b(&root);
    let (slot, at) = slot_in_b(&mut b, None, 2, 0);
    let (lib_id, key) = primary_button_key(&mut b);
    let dropped = b.shell.app.drop_library_asset(&lib_id, &key, at).expect("dropped");
    assert_eq!(b.app().doc.get(dropped).unwrap().parent, Some(slot));
    assert!(!b.app().doc.images.is_empty(), "the imported component keeps the image its fill refers to");
    let _ = std::fs::remove_dir_all(root);
}
