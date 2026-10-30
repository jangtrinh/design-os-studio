//! UI tests for Dev mode: code languages, Ready for dev, Compare changes, annotations and links.

use super::*;
use crate::dev::{Language, LeftTab};
use crate::home::dialog::Intent;
use crate::panels::menus::Action;
use designos_doc::{diff_subtree, ChangeKind, DevLink, DevStatus};
use designos_library::Library;
use std::path::PathBuf;

fn dev_sim() -> (Sim, NodeId) {
    let s = Sim::with(Options { select: Some("Card".into()), dev: Some("css".into()), ..Default::default() });
    let card = s.by_name("Card");
    (s, card)
}

fn run_action(s: &mut Sim, a: Action) {
    let ctx = s.ctx.clone();
    crate::panels::menus::run(&mut s.shell.app, &ctx, a);
    s.frame(vec![]);
    s.frame(vec![]);
}

fn frames(s: &mut Sim, n: usize) {
    for _ in 0..n {
        s.frame(vec![]);
    }
}

fn status(s: &Sim, id: NodeId) -> Option<DevStatus> {
    s.app().doc.get(id).unwrap().dev.status
}

#[test]
fn dev_header_marks_ready_and_edits_flip_the_status_to_changed_and_back() {
    let (mut s, card) = dev_sim();
    assert!(s.app().ui.dev_mode);
    let b = s.rect_of("dev-ready");
    s.click(b.center());
    assert_eq!(status(&s, card), Some(DevStatus::ReadyForDev));

    s.shell.app.edit_nodes(&[card], |n| n.opacity = 0.5);
    frames(&mut s, 10);
    assert_eq!(status(&s, card), Some(DevStatus::Changed), "an edit after marking flips the status");

    s.cmd_key(Key::Z);
    frames(&mut s, 10);
    assert_eq!(status(&s, card), Some(DevStatus::ReadyForDev), "undoing the edit restores it");
}

#[test]
fn context_menu_marks_frames_ready_for_dev() {
    let (mut s, card) = dev_sim();
    let p = s.screen(660.0, 150.0);
    s.right_click(p);
    assert!(s.app().ui.ctx_menu.is_some());
    let labels = crate::panels::context::labels(s.app());
    assert!(labels.contains(&"Ready for dev".to_string()), "{labels:?}");
    assert!(!labels.contains(&"Mark as completed".to_string()), "no status yet");
    s.shell.app.ui.ctx_menu = None;
    s.shell.app.select_only(card);
    run_action(&mut s, Action::MarkReadyForDev);
    assert_eq!(status(&s, card), Some(DevStatus::ReadyForDev));
    // the menu now offers the follow-up actions
    let labels = crate::panels::context::labels(s.app());
    assert!(labels.contains(&"Mark as completed".to_string()) && labels.contains(&"Remove dev status".to_string()), "{labels:?}");
    run_action(&mut s, Action::MarkCompleted);
    assert_eq!(status(&s, card), Some(DevStatus::Completed));
    run_action(&mut s, Action::ClearDevStatus);
    assert_eq!(status(&s, card), None);
    // sections accept it, plain rectangles do not
    let hero = s.by_name("Hero");
    s.shell.app.select_only(hero);
    run_action(&mut s, Action::MarkReadyForDev);
    assert_eq!(status(&s, hero), None);
}

#[test]
fn language_dropdown_switches_the_code_and_copy_reports() {
    let (mut s, _) = dev_sim();
    for (i, lang) in Language::ALL.into_iter().enumerate() {
        let dd = s.rect_of("dev-lang");
        s.click(dd.center());
        // the popup sits under the dropdown, shifted up when the longer list would leave the screen
        let menu_h = Language::ALL.len() as f32 * theme::MENU_ITEM_HEIGHT + 2.0 * theme::MENU_PADDING;
        let top = (dd.bottom() + 2.0).min(s.ctx.content_rect().bottom() - menu_h - 2.0);
        let y = top + theme::MENU_PADDING + (i as f32 + 0.5) * theme::MENU_ITEM_HEIGHT;
        s.click(Pos2::new(dd.center().x, y));
        assert_eq!(s.app().dev.lang, lang, "option {i}");
        frames(&mut s, 2);
    }
    let copy = s.rect_of("dev-copy-code");
    s.click(copy.center());
    assert!(s.app().toast.as_ref().is_some_and(|t| t.text.starts_with("Copied")));
    // every language produces code for the selection
    let card = s.by_name("Card");
    for l in Language::ALL {
        let code = l.generate(&s.app().export_doc(), card, false);
        assert!(!code.is_empty());
    }
}

#[test]
fn annotation_editor_adds_pins_properties_and_undoes() {
    let (mut s, card) = dev_sim();
    let add = s.rect_of("annot-add");
    s.click(add.center());
    assert!(s.app().dev.annot.as_ref().is_some_and(|e| e.node == card && e.index.is_none()));
    let field = s.rect_of("annot-text");
    s.click(field.center());
    s.type_text("Match the 8px grid");
    s.frame(vec![]);
    let chip = s.rect_of("annot-chip-Width");
    s.click(chip.center());
    let chip = s.rect_of("annot-chip-Fill");
    s.click(chip.center());
    let save = s.rect_of("dev-annot-save");
    s.click(save.center());
    frames(&mut s, 2);
    let a = &s.app().doc.get(card).unwrap().dev.annotations;
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].text, "Match the 8px grid");
    assert_eq!(a[0].properties, vec!["Width".to_string(), "Fill".to_string()]);
    assert!(s.app().dev.annot.is_none(), "the editor closed");
    // it draws as a callout without trouble, and undo removes it
    frames(&mut s, 2);
    s.cmd_key(Key::Z);
    assert!(s.app().doc.get(card).unwrap().dev.annotations.is_empty());
    s.mods = Modifiers::COMMAND | Modifiers::SHIFT;
    s.key(Key::Z);
    s.mods = Modifiers::NONE;
    assert_eq!(s.app().doc.get(card).unwrap().dev.annotations.len(), 1, "redo brings it back");
}

#[test]
fn cancelling_the_annotation_editor_leaves_the_document_alone() {
    let (mut s, card) = dev_sim();
    let add = s.rect_of("annot-add");
    s.click(add.center());
    let cancel = s.rect_of("btn-\"annot-cancel\"");
    s.click(cancel.center());
    assert!(s.app().dev.annot.is_none());
    assert!(s.app().doc.get(card).unwrap().dev.is_empty());
}

#[test]
fn clicking_a_dev_link_asks_before_opening_it() {
    let (mut s, card) = dev_sim();
    s.shell.app.dev_edit(card, |d| d.links.push(DevLink { label: "Storybook".into(), url: "https://storybook.example.com/card".into() }));
    frames(&mut s, 2);
    let link = s.rect_of("dev-link-0");
    s.click(link.center());
    let d = s.app().dialog.as_ref().expect("a confirmation");
    assert_eq!(d.intent, Intent::OpenDevLink("https://storybook.example.com/card".into()));
    // cancelling opens nothing and closes the dialog
    s.key(Key::Escape);
    assert!(s.app().dialog.is_none());
}

#[test]
fn adding_a_link_validates_the_url() {
    let (mut s, card) = dev_sim();
    let add = s.rect_of("link-new");
    s.click(add.center());
    assert!(s.app().dev.link.is_some());
    let label = s.rect_of("link-label");
    s.click(label.center());
    s.type_text("Jira");
    s.frame(vec![]);
    let url = s.rect_of("link-url");
    s.click(url.center());
    s.type_text("javascript:alert(1)");
    s.frame(vec![]);
    let add = s.rect_of("dev-link-add");
    s.click(add.center());
    assert!(s.app().doc.get(card).unwrap().dev.links.is_empty(), "unsafe schemes are refused");
    assert!(s.app().toast.as_ref().is_some_and(|t| t.error));
    // a valid link goes in
    let url = s.rect_of("link-url");
    s.click(url.center());
    s.type_text("https://example.atlassian.net/browse/D-1");
    s.frame(vec![]);
    let add = s.rect_of("dev-link-add");
    s.click(add.center());
    let links = &s.app().doc.get(card).unwrap().dev.links;
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].url, "https://example.atlassian.net/browse/D-1");
}

#[test]
fn left_panel_lists_ready_items_per_page_and_selects_on_click() {
    let (mut s, card) = dev_sim();
    assert_eq!(s.app().dev.left_tab, LeftTab::ReadyForDev);
    run_action(&mut s, Action::MarkReadyForDev);
    s.shell.app.clear_selection();
    frames(&mut s, 2);
    let r = s.rect_of(&format!("dev-item-{}", card.0));
    s.click(r.center());
    assert_eq!(s.app().selection, vec![card]);
    // the Layers tab brings back the layer tree
    s.shell.app.dev.left_tab = LeftTab::Layers;
    frames(&mut s, 2);
    assert!(s.app().ui.dev_mode);
}

#[test]
fn compare_changes_opens_from_the_header_and_closes_with_escape() {
    let (mut s, card) = dev_sim();
    run_action(&mut s, Action::MarkReadyForDev);
    s.shell.app.edit_nodes(&[card], |n| n.name = "Quarterly card".into());
    s.shell.app.edit_nodes(&[card], |n| n.opacity = 0.8);
    frames(&mut s, 10);
    assert_eq!(status(&s, card), Some(DevStatus::Changed));
    let b = s.rect_of("dev-compare");
    s.click(b.center());
    let v = s.app().dev.compare.as_ref().expect("the compare view");
    assert_eq!(v.node, card);
    let props: Vec<String> =
        v.changes.iter().flat_map(|c| if let ChangeKind::Modified(p) = &c.kind { p.iter().map(|p| p.property.clone()).collect() } else { vec![] }).collect();
    assert!(props.contains(&"Name".to_string()) && props.contains(&"Opacity".to_string()), "{props:?}");
    frames(&mut s, 2);
    s.key(Key::Escape);
    assert!(s.app().dev.compare.is_none());
    // "Mark ready" takes a new baseline
    let b = s.rect_of("btn-(\"dev-remark\", 0)");
    s.click(b.center());
    frames(&mut s, 10);
    assert_eq!(status(&s, card), Some(DevStatus::ReadyForDev));
}

#[test]
fn remarking_after_changes_takes_a_new_baseline() {
    let (mut s, card) = dev_sim();
    run_action(&mut s, Action::MarkReadyForDev);
    s.shell.app.edit_nodes(&[card], |n| n.opacity = 0.7);
    frames(&mut s, 10);
    assert_eq!(status(&s, card), Some(DevStatus::Changed));
    run_action(&mut s, Action::MarkReadyForDev);
    frames(&mut s, 10);
    assert_eq!(status(&s, card), Some(DevStatus::ReadyForDev));
}

#[test]
fn inspect_sections_for_variables_components_and_assets_have_data() {
    let (s, card) = dev_sim();
    let used = crate::dev::data::variables_used(&s.app().doc, card);
    assert!(!used.is_empty(), "the demo card binds Theme variables");
    let theme = used.iter().find(|u| u.name == "Surface/Card").expect("the card surface variable");
    let modes: Vec<&str> = theme.modes.iter().map(|(m, _)| m.as_str()).collect();
    assert_eq!(modes, vec!["Light", "Dark"]);
    assert!(theme.modes.iter().all(|(_, v)| v.is_some()));
    // vector assets: the star and polygon on the dashboard
    let board = s.by_name("Dashboard");
    assert_eq!(designos_export::vector_assets(&s.app().doc, board).len(), 2);
    // an instance lists its component and properties
    let inst = s.app().doc.nodes.values().find(|n| matches!(n.kind, NodeKind::Instance { .. })).map(|n| n.id).expect("the demo has instances");
    let rows = crate::dev::data::component_rows(&s.app().doc, inst);
    assert_eq!(rows[0].0, "Component");
    assert!(rows.len() > 1, "{rows:?}");
}

#[test]
fn token_handoff_copies_codegen_reference_and_effective_value_without_editing() {
    let (mut s, card) = dev_sim();
    let used = crate::dev::data::variables_used(&s.app().doc, card);
    let index = used.iter().position(|u| u.name == "Surface/Card").unwrap();
    let token = &s.app().doc.variables[&used[index].id];
    let reference = designos_export::css_variable_reference(token);
    // the code prints the reference with the resolved value as its fallback: `var(--Surface-Card, #FFF)`
    let name_part = reference.trim_end_matches(')');
    assert!(designos_export::css_for(&s.app().doc, card).contains(&format!("{name_part}, ")));
    let designos_doc::VariableValue::Color(color) = used[index].effective[0].value.clone().unwrap() else { panic!("color token") };
    let value = crate::dev::color::ColorFormat::Rgb.value(color, color.a);
    let before = s.app().doc.to_json().unwrap();
    let ctx = s.ctx.clone();
    let mut time = 0.0;
    let mut draw = |events| {
        time += 0.05;
        let output = ctx.run_ui(
            RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 4000.0))), time: Some(time), events, ..Default::default() },
            |ui| {
                let mut c = crate::panels::inspect::Cur { x0: 0.0, oy: 0.0, y: 0.0, w: 240.0 };
                crate::dev::sections::variables_section(&mut s.shell.app, ui, &mut c, card);
            },
        );
        output
            .platform_output
            .commands
            .into_iter()
            .filter_map(|cmd| match cmd {
                egui::OutputCommand::CopyText(text) => Some(text),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    draw(vec![]);
    for (key, expected) in [(format!("dev-token-ref-{index}"), reference), (format!("dev-token-value-{index}-0"), value)] {
        let r = ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", key)))).unwrap();
        let mut copied = Vec::new();
        for pressed in [true, false] {
            copied.extend(draw(vec![
                Event::PointerMoved(r.center()),
                Event::PointerButton { pos: r.center(), button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE },
            ]));
        }
        assert_eq!(copied, vec![expected]);
    }
    assert_eq!(s.app().doc.to_json().unwrap(), before, "inspection is read-only");
}

#[test]
fn token_code_syntax_rows_copy_each_platforms_syntax() {
    use designos_doc::CodeSyntaxPlatform;
    let (mut s, card) = dev_sim();
    let used = crate::dev::data::variables_used(&s.app().doc, card);
    let index = used.iter().position(|u| u.name == "Surface/Card").unwrap();
    let id = used[index].id.clone();
    let syntax = s.shell.app.doc.variables.get_mut(&id).unwrap();
    syntax.code_syntax.insert(CodeSyntaxPlatform::Web, "var(--surface-card, #fff)".into());
    syntax.code_syntax.insert(CodeSyntaxPlatform::Android, "R.color.surfaceCard".into());
    syntax.code_syntax.insert(CodeSyntaxPlatform::Ios, "Color.surfaceCard".into());
    let before = s.app().doc.to_json().unwrap();
    let ctx = s.ctx.clone();
    let mut time = 0.0;
    let mut draw = |events| {
        time += 0.05;
        let output = ctx.run_ui(
            RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 4000.0))), time: Some(time), events, ..Default::default() },
            |ui| {
                let mut c = crate::panels::inspect::Cur { x0: 0.0, oy: 0.0, y: 0.0, w: 240.0 };
                crate::dev::sections::variables_section(&mut s.shell.app, ui, &mut c, card);
            },
        );
        output
            .platform_output
            .commands
            .into_iter()
            .filter_map(|cmd| match cmd {
                egui::OutputCommand::CopyText(text) => Some(text),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    draw(vec![]);
    for (key, expected) in [
        (format!("dev-token-ref-{index}"), "var(--surface-card, #fff)"),
        (format!("dev-token-syntax-{index}-android"), "R.color.surfaceCard"),
        (format!("dev-token-syntax-{index}-ios"), "Color.surfaceCard"),
    ] {
        let r = ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", key.clone())))).unwrap_or_else(|| panic!("no row {key}"));
        let mut copied = Vec::new();
        for pressed in [true, false] {
            copied.extend(draw(vec![
                Event::PointerMoved(r.center()),
                Event::PointerButton { pos: r.center(), button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE },
            ]));
        }
        assert_eq!(copied, vec![expected.to_string()], "{key}");
    }
    assert_eq!(s.app().doc.to_json().unwrap(), before, "inspection is read-only");
}

#[test]
fn color_handoff_copies_the_selected_format() {
    use crate::dev::color::ColorFormat;
    let (mut s, card) = dev_sim();
    let color = crate::panels::inspect::collect_colors(&s.app().doc, card)[0].clone();
    let ctx = s.ctx.clone();
    let mut time = 0.0;
    let mut draw = |format, events| {
        s.shell.app.dev.color_format = format;
        time += 0.05;
        let output = ctx.run_ui(
            RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 8000.0))), time: Some(time), events, ..Default::default() },
            |ui| crate::panels::inspect::show(&mut s.shell.app, ui, Rect::from_min_size(Pos2::ZERO, Vec2::new(240.0, 8000.0))),
        );
        output
            .platform_output
            .commands
            .into_iter()
            .filter_map(|cmd| match cmd {
                egui::OutputCommand::CopyText(text) => Some(text),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    for format in ColorFormat::ALL {
        draw(format, vec![]);
        let r = ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", "dev-copy-color-0".to_string())))).unwrap();
        let mut copied = Vec::new();
        for pressed in [true, false] {
            copied.extend(draw(
                format,
                vec![
                    Event::PointerMoved(r.center()),
                    Event::PointerButton { pos: r.center(), button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE },
                ],
            ));
        }
        assert_eq!(copied, vec![format.value(color.color, color.opacity)]);
    }
}

#[test]
fn selecting_instances_and_vectors_draws_the_dev_panel() {
    let (mut s, _) = dev_sim();
    let inst = s.app().doc.nodes.values().find(|n| matches!(n.kind, NodeKind::Instance { .. })).map(|n| n.id).unwrap();
    s.shell.app.select_only(inst);
    frames(&mut s, 3);
    let board = s.by_name("Dashboard");
    s.shell.app.select_only(board);
    frames(&mut s, 3);
    s.shell.app.selection.clear();
    frames(&mut s, 3);
}

// ---- library files keep the snapshot in the version history

struct TmpDir(PathBuf);

impl TmpDir {
    fn new() -> Self {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("designos_dev_ui_{}_{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        Self(p)
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn library_files_store_the_snapshot_as_a_named_version() {
    let dir = TmpDir::new();
    let mut s = Sim::with(Options { home: true, library: Some(dir.0.clone()), ..Default::default() });
    let lib: Library = s.app().lib.clone().expect("a library");
    let proj = lib.list_projects()[0].id.clone();
    let id = lib.create_file(&proj, None, "Demo fixture", &crate::demo::demo_document()).expect("create demo fixture file").id;
    crate::home::apply(&mut s.shell.app, crate::home::Act::Open(id.clone()));
    frames(&mut s, 3);
    let card = s.by_name("Card");
    s.shell.app.select_only(card);
    s.shell.app.dev_mark_ready(&[card]);
    let n = s.app().doc.get(card).unwrap().clone();
    let v = n.dev.snapshot_version.expect("a version number");
    assert!(n.dev.snapshot.is_none(), "no inline copy for library files");
    let versions = lib.list_versions(&id);
    let saved = versions.iter().find(|x| x.n == v).expect("the version exists");
    assert!(saved.label.as_deref().is_some_and(|l| l.starts_with("Ready for dev: Card")), "{:?}", saved.label);

    let w0 = n.size.x;
    s.shell.app.edit_nodes(&[card], |n| n.size.x = w0 + 30.0);
    s.shell.app.dev_refresh(true);
    assert_eq!(status(&s, card), Some(DevStatus::Changed));
    let (old, nodes) = s.app().dev_snapshot(card).expect("the snapshot loads from the version");
    assert_eq!(old.get(card).unwrap().size.x, w0);
    let diff = diff_subtree(&nodes, &s.app().doc, card);
    assert!(diff.iter().any(|c| c.id == card), "{diff:?}");
}

#[test]
fn dev_mode_inspect_shows_slot_for_component_slot_frame_and_instance() {
    use designos_doc::{
        create_instance, instances_of, ComponentPropDef, FrameProps, Node, OverrideField, PropKind, PropValue, Rect as DocRect, TextStyle, Vec2 as DocVec2,
    };
    let (mut s, _) = dev_sim();
    let page = s.app().page;
    let comp_id = s.shell.app.doc.alloc_id();
    let mut comp = Node::new(comp_id, NodeKind::Component(FrameProps::default()));
    comp.name = "Dialog".into();
    comp.size = DocVec2::new(240.0, 120.0);
    comp.component.props.push(ComponentPropDef { name: "Body#1:0".into(), kind: PropKind::Slot, default: PropValue::Slot(vec![]) });
    let comp_id = s.shell.app.doc.insert(page, comp).unwrap();

    let slot_id = s.shell.app.doc.alloc_id();
    let mut slot = Node::frame(slot_id, DocRect::new(0.0, 0.0, 216.0, 96.0));
    slot.name = "Body".into();
    slot.component.is_slot = true;
    slot.component.bindings.insert(OverrideField::SlotContent, "Body#1:0".into());
    let slot_id = s.shell.app.doc.insert(comp_id, slot).unwrap();

    create_instance(&mut s.shell.app.doc, comp_id, page, DocVec2::new(300.0, 0.0)).apply(&mut s.shell.app.doc).unwrap();
    let inst_id = instances_of(&s.shell.app.doc, comp_id)[0];
    let inst_slot = s.shell.app.doc.get(inst_id).unwrap().children[0];
    let txt_id = s.shell.app.doc.alloc_id();
    let mut txt = Node::text(txt_id, DocRect::new(0.0, 0.0, 100.0, 20.0), "Hello slot", TextStyle::default());
    txt.name = "Greeting".into();
    txt.component.is_slot_content = true;
    s.shell.app.doc.insert(inst_slot, txt).unwrap();

    let comp_rows = crate::dev::data::component_rows(&s.app().doc, comp_id);
    assert!(comp_rows.iter().any(|(k, v)| k == "Body" && v.contains("Slot")), "component rows must list the slot: {comp_rows:?}");

    let slot_rows = crate::dev::data::component_rows(&s.app().doc, slot_id);
    assert!(slot_rows.iter().any(|(k, v)| k == "Slot" && v == "Body"), "slot frame rows must identify the slot: {slot_rows:?}");

    let inst_rows = crate::dev::data::component_rows(&s.app().doc, inst_id);
    assert!(
        inst_rows.iter().any(|(k, v)| k == "Body" && v.contains("Slot") && v.contains("Greeting")),
        "instance rows must show the slot and its child content: {inst_rows:?}"
    );
}

#[test]
fn dev_mode_component_playground_toggles_props_without_mutating_doc_and_updates_codegen_live() {
    let mut s = Sim::with(Options { select: Some("Get started".into()), dev: Some("react".into()), ..Default::default() });
    let inst = s.by_name("Get started");
    assert!(s.app().doc.get(inst).is_some());
    frames(&mut s, 2);

    let before_hash = crate::home::doc_hash(&s.app().doc);
    let before_epoch = s.app().doc.mutation_epoch();

    let copy_code = |s: &mut Sim| -> String {
        let p = s.rect_of("dev-inspect-code").center();
        let mut copied = String::new();
        for pressed in [true, false] {
            s.t += 0.05;
            let raw = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 1088.0))),
                time: Some(s.t),
                events: vec![Event::PointerMoved(p), Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }],
                ..Default::default()
            };
            let ctx = s.ctx.clone();
            let out = ctx.run_ui(raw, |ui| s.shell.draw(ui));
            for cmd in out.platform_output.commands {
                if let egui::OutputCommand::CopyText(text) = cmd {
                    copied = text;
                }
            }
        }
        copied
    };

    // Baseline React snippet has `label="Get started"` and default `Show icon = true` (omitted).
    let initial_code = copy_code(&mut s);
    assert!(initial_code.contains("label=\"Get started\""), "initial code: {initial_code}");
    assert!(!initial_code.contains("showIcon={false}"), "initial code should not have showIcon={{false}}: {initial_code}");

    // Toggle `Show icon` in the Dev Mode Component Playground.
    let show_icon_toggle = s.rect_of("dev-playground-prop-Show icon");
    s.click(show_icon_toggle.center());
    frames(&mut s, 2);

    // Document hash and mutation epoch must remain 100% unchanged!
    assert_eq!(crate::home::doc_hash(&s.app().doc), before_hash, "playground prop toggle must never mutate the document");
    assert_eq!(s.app().doc.mutation_epoch(), before_epoch, "playground prop toggle must not advance mutation_epoch");

    // Live codegen must immediately reflect `showIcon={false}`.
    let toggled_code = copy_code(&mut s);
    assert!(toggled_code.contains("showIcon={false}"), "live codegen should include showIcon={{false}}: {toggled_code}");

    // Edit the `Label` text property in the playground.
    let label_field = s.rect_of("dev-playground-prop-Label");
    s.click(label_field.center());
    // The click lands inside the text, wherever the glyphs sit at this size: End puts the caret after "Get started".
    s.key(Key::End);
    s.type_text(" now");
    frames(&mut s, 2);

    assert_eq!(crate::home::doc_hash(&s.app().doc), before_hash, "playground text edit must never mutate the document");
    let edited_code = copy_code(&mut s);
    assert!(edited_code.contains("Get started now"), "live codegen should reflect edited label: {edited_code}");

    // Reset button restores baseline props and codegen without touching the document.
    let reset_btn = s.rect_of("dev-playground-reset");
    s.click(reset_btn.center());
    frames(&mut s, 2);
    assert_eq!(crate::home::doc_hash(&s.app().doc), before_hash);
    let reset_code = copy_code(&mut s);
    assert_eq!(reset_code, initial_code, "resetting playground restores initial codegen");
}

/// The showcase `List row` instance with its `Icon` swap prop's `preferred` list emptied.
fn list_row_without_preferred() -> (Sim, NodeId, NodeId) {
    let mut s = Sim::with(Options { fixture: Some("showcase".into()), select: Some("List row".into()), dev: Some("react".into()), ..Default::default() });
    let set = s.app().doc.nodes.values().find(|n| n.name == "List row" && matches!(n.kind, NodeKind::ComponentSet(_))).map(|n| n.id).unwrap();
    let doc = &mut s.shell.app.doc;
    let props = &mut doc.get_mut(set).unwrap().component.props;
    for def in props.iter_mut() {
        if let designos_doc::PropKind::InstanceSwap { preferred } = &mut def.kind {
            preferred.clear();
        }
    }
    doc.bump_mutation_epoch();
    let (inst, comp) = list_row_instance_of(&s);
    frames(&mut s, 2);
    (s, inst, comp)
}

fn list_row_instance_of(s: &Sim) -> (NodeId, NodeId) {
    let doc = &s.app().doc;
    let inst = doc.nodes.values().find(|n| n.name == "List row" && matches!(n.kind, NodeKind::Instance { .. })).map(|n| n.id).expect("a List row instance");
    (inst, designos_doc::instance_component(doc.get(inst).unwrap()).unwrap())
}

fn list_row_instance() -> (Sim, NodeId, NodeId) {
    let s = Sim::with(Options { fixture: Some("showcase".into()), select: Some("List row".into()), dev: Some("react".into()), ..Default::default() });
    let (inst, comp) = list_row_instance_of(&s);
    (s, inst, comp)
}

#[test]
fn playground_swap_menu_lists_the_files_components_when_there_are_no_preferred_values() {
    use crate::dev::playground::{swap_choices, swap_list_builds};
    let (mut s, _inst, comp) = list_row_without_preferred();
    let family = designos_doc::components_of(&s.app().doc, s.app().doc.get(comp).unwrap().parent.unwrap());
    let components: Vec<NodeId> = s.app().doc.nodes.values().filter(|n| matches!(n.kind, NodeKind::Component(_))).map(|n| n.id).collect();

    let closed = swap_choices(&mut s.shell.app, false, &[], comp);
    assert!(closed.is_empty(), "a closed menu builds nothing");

    let open = swap_choices(&mut s.shell.app, true, &[], comp);
    assert!(open.len() > 20, "the file's components are listed, got {}", open.len());
    assert!(open.iter().all(|c| components.contains(c)), "every choice is a main component");
    assert!(open.iter().all(|c| !family.contains(c)), "the component being edited is not a choice");
    let icon = components.iter().copied().find(|c| s.app().doc.get(*c).unwrap().name.starts_with("Icon/")).unwrap();
    assert!(open.contains(&icon), "icons are choices");
    let _ = swap_list_builds();
}

#[test]
fn playground_swap_list_is_cached_per_revision() {
    use crate::dev::playground::{swap_choices, swap_list_builds};
    let (mut s, _inst, comp) = list_row_without_preferred();
    let base = swap_list_builds();
    for _ in 0..6 {
        swap_choices(&mut s.shell.app, true, &[], comp);
    }
    assert_eq!(swap_list_builds(), base + 1, "an unchanged revision reuses the list");
    s.shell.app.doc.bump_mutation_epoch();
    swap_choices(&mut s.shell.app, true, &[], comp);
    assert_eq!(swap_list_builds(), base + 2, "a new revision rebuilds it");
    swap_choices(&mut s.shell.app, false, &[], comp);
    assert_eq!(swap_list_builds(), base + 2, "a closed menu never builds");
}

#[test]
fn playground_swap_menu_opens_from_the_control_and_builds_the_list_only_then() {
    use crate::dev::playground::swap_list_builds;
    let (mut s, _inst, _comp) = list_row_without_preferred();
    frames(&mut s, 3);
    let base = swap_list_builds();
    frames(&mut s, 3);
    assert_eq!(swap_list_builds(), base, "no list while the menu is closed");
    let ctl = s.rect_of("dev-playground-prop-Icon");
    s.click(ctl.center());
    frames(&mut s, 3);
    assert!(swap_list_builds() > base, "opening the menu builds the list");
    assert_eq!(swap_list_builds(), base + 1, "and only once while it stays open");
}

#[test]
fn playground_swap_with_preferred_values_stays_preferred_only() {
    use crate::dev::playground::{swap_choices, swap_list_builds};
    let (s, _inst, comp) = list_row_instance();
    let mut s = s;
    let preferred: Vec<NodeId> = s.app().doc.nodes.values().filter(|n| n.name.starts_with("Icon/")).map(|n| n.id).take(3).collect();
    assert_eq!(preferred.len(), 3);
    let base = swap_list_builds();
    let got = swap_choices(&mut s.shell.app, true, &preferred, comp);
    assert_eq!(got, preferred);
    assert_eq!(swap_list_builds(), base, "nothing is scanned when the prop names its choices");
}

/// What the canvas shows for a status: the label badge (canvas font) and the Dev annotation pill (caption font),
/// counted among the "Ready for dev" texts painted inside the canvas area, and whether the pill's hit region exists.
fn canvas_status_marks(s: &Sim, id: NodeId) -> (usize, usize, bool) {
    fn walk(shape: &egui::Shape, out: &mut Vec<(Pos2, f32)>) {
        match shape {
            egui::Shape::Vec(inner) => inner.iter().for_each(|shape| walk(shape, out)),
            egui::Shape::Text(text) if text.galley.job.text == "Ready for dev" => {
                if let Some(section) = text.galley.job.sections.first() {
                    out.push((text.pos, section.format.font_id.size));
                }
            }
            _ => {}
        }
    }
    let mut texts = Vec::new();
    s.shapes.iter().for_each(|clipped| walk(&clipped.shape, &mut texts));
    let canvas = s.app().canvas_rect;
    let on_canvas = |size: f32| texts.iter().filter(|(pos, text_size)| canvas.contains(*pos) && *text_size == size).count();
    let pill_registered = s.ctx.read_response(egui::Id::new(("dev-badge", id.0))).is_some();
    (on_canvas(theme::FONT_SIZE_CANVAS), on_canvas(theme::FONT_SIZE_CAPTION), pill_registered)
}

/// Every painted shape of the last frame, groups opened.
fn painted_shapes(s: &Sim) -> Vec<&egui::Shape> {
    fn walk<'a>(shape: &'a egui::Shape, out: &mut Vec<&'a egui::Shape>) {
        match shape {
            egui::Shape::Vec(inner) => inner.iter().for_each(|shape| walk(shape, out)),
            other => out.push(other),
        }
    }
    let mut out = Vec::new();
    s.shapes.iter().for_each(|clipped| walk(&clipped.shape, &mut out));
    out
}

/// What one Dev tab paints: its label (the leftmost text inside the tab box) and its count badge, when it has one.
struct TabPaint {
    tab: Rect,
    label: String,
    text_left: f32,
    text_width: f32,
    badge: Option<Rect>,
}

fn tab_paint(s: &Sim, key: &str) -> TabPaint {
    let tab = s.rect_of(key);
    let shapes = painted_shapes(s);
    let text = shapes
        .iter()
        .filter_map(|shape| match shape {
            egui::Shape::Text(t) if tab.contains(t.pos) => Some(t),
            _ => None,
        })
        .min_by(|a, b| a.pos.x.total_cmp(&b.pos.x))
        .unwrap_or_else(|| panic!("nothing is painted inside {key} {tab:?}"));
    let badge = shapes.iter().find_map(|shape| match shape {
        egui::Shape::Rect(r) if r.rect.height() == 16.0 && tab.contains_rect(r.rect) => Some(r.rect),
        _ => None,
    });
    TabPaint { tab, label: text.galley.job.text.clone(), text_left: text.pos.x, text_width: text.galley.size().x, badge }
}

/// The tab hugs its content in whole pixels, rounded up, with the same inset on both sides.
fn assert_tab_hugs_its_content(p: &TabPaint) {
    let content_right = p.badge.map_or(p.text_left + p.text_width.ceil(), |badge| badge.right());
    assert!((p.text_left - p.tab.left() - theme::INSET_CONTROL).abs() < 0.01, "{:?}: {} px in from the left", p.label, p.text_left - p.tab.left());
    assert!((p.tab.right() - content_right - theme::INSET_CONTROL).abs() < 0.01, "{:?}: {} px in from the right", p.label, p.tab.right() - content_right);
}

#[test]
fn the_dev_tabs_hug_their_content_with_the_same_inset_on_both_sides() {
    let (mut s, _) = dev_sim();
    // A status in the file: the strip has a count to show.
    run_action(&mut s, Action::MarkReadyForDev);
    // At the default panel width the strip does not fit beside the layers buttons: the count badge goes, then the label elides.
    let ready = tab_paint(&s, "dev-left-tab-ready");
    assert!(ready.label.starts_with("Ready") && ready.label.ends_with('\u{2026}'), "the label elides on the way out, got {:?}", ready.label);
    assert!(ready.badge.is_none(), "the count badge is dropped before the label elides");
    assert_tab_hugs_its_content(&ready);
    let layers = tab_paint(&s, "dev-left-tab-layers");
    assert_eq!(layers.label, "Layers");
    assert!(layers.badge.is_none());
    assert_tab_hugs_its_content(&layers);
}

#[test]
fn a_wider_left_panel_keeps_the_full_ready_label_and_its_count_badge() {
    let (mut s, _) = dev_sim();
    run_action(&mut s, Action::MarkReadyForDev);
    s.shell.app.ui.left_panel_width = 280.0;
    frames(&mut s, 2);
    let ready = tab_paint(&s, "dev-left-tab-ready");
    assert_eq!(ready.label, "Ready for dev");
    assert!(ready.badge.is_some(), "the count badge is painted when the strip fits");
    assert_tab_hugs_its_content(&ready);
    let layers = tab_paint(&s, "dev-left-tab-layers");
    assert_eq!(layers.label, "Layers");
    assert_tab_hugs_its_content(&layers);
}

#[test]
fn the_dev_tabs_stay_clear_of_the_layers_buttons_at_every_left_panel_width() {
    let (mut s, _) = dev_sim();
    let page = s.app().page;
    for _ in 0..12 {
        let id = s.shell.app.doc.alloc_id();
        let mut node = designos_doc::Node::frame(id, designos_doc::Rect::new(0.0, 0.0, 10.0, 10.0));
        node.dev.status = Some(DevStatus::ReadyForDev);
        s.shell.app.doc.insert(page, node).unwrap();
    }
    s.shell.app.doc.bump_mutation_epoch();
    // The layers buttons are drawn only while the Layers tab is the active one.
    s.shell.app.dev.left_tab = LeftTab::Layers;
    for width in [200.0, theme::LEFT_PANEL_WIDTH, 280.0, 360.0] {
        s.shell.app.ui.left_panel_width = width;
        frames(&mut s, 2);
        let ready = s.rect_of("dev-left-tab-ready");
        let layers = s.rect_of("dev-left-tab-layers");
        let search = s.rect_of("layers-search");
        let collapse = s.rect_of("layers-collapse-all");
        let buttons = collapse.union(search);
        for (name, tab) in [("ready", ready), ("layers", layers)] {
            assert!(!tab.intersects(buttons), "{name} tab {tab:?} paints under the layers buttons {buttons:?} at width {width}");
            assert!(
                tab.right() <= collapse.left() - theme::SPACE_FACT + 0.5,
                "{name} tab ends at {} against the buttons at {} (width {width})",
                tab.right(),
                collapse.left()
            );
        }
        assert!(ready.right() <= layers.left(), "the tabs keep their order at width {width}");
        let ready_paint = tab_paint(&s, "dev-left-tab-ready");
        if width <= theme::LEFT_PANEL_WIDTH {
            assert!(ready_paint.badge.is_none(), "no room for the two-digit count at width {width}");
        }
        assert!(!ready_paint.label.is_empty(), "the ready tab keeps a label at width {width}");
    }
}

#[test]
fn a_top_level_frame_shows_its_status_once_in_the_label_badge_and_not_as_a_dev_pill() {
    let (mut s, card) = dev_sim();
    run_action(&mut s, Action::MarkReadyForDev);
    assert_eq!(status(&s, card), Some(DevStatus::ReadyForDev));
    let (badges, pills, pill_registered) = canvas_status_marks(&s, card);
    assert_eq!((badges, pills, pill_registered), (1, 0, false), "one badge beside the frame name, no pill over it");
}

#[test]
fn a_nested_frame_keeps_the_dev_pill_because_it_has_no_label_badge() {
    let (mut s, _) = dev_sim();
    let welcome = s.by_name("Welcome");
    let id = s.shell.app.doc.alloc_id();
    let inner = s.shell.app.doc.insert(welcome, designos_doc::Node::frame(id, designos_doc::Rect::new(20.0, 20.0, 120.0, 80.0))).unwrap();
    s.shell.app.select_only(inner);
    run_action(&mut s, Action::MarkReadyForDev);
    assert_eq!(status(&s, inner), Some(DevStatus::ReadyForDev));
    let (_, pills, pill_registered) = canvas_status_marks(&s, inner);
    assert_eq!((pills, pill_registered), (1, true), "a nested layer has no frame label, so the pill stays");
}

#[test]
fn the_dev_pill_returns_when_the_frame_label_is_hidden_by_zoom() {
    let (mut s, card) = dev_sim();
    run_action(&mut s, Action::MarkReadyForDev);
    s.shell.app.view.zoom = theme::CANVAS_LABEL_MAX_ZOOM_HIDE * 0.5;
    frames(&mut s, 3);
    let (badges, _, pill_registered) = canvas_status_marks(&s, card);
    assert_eq!(badges, 0, "the labels, and their badges, are hidden at this zoom");
    assert!(pill_registered, "the pill is the only status mark left, so it is painted");
}

#[test]
fn playground_swap_control_is_disabled_when_the_file_has_no_swap_target() {
    use crate::dev::playground::swap_target_exists;
    let (mut s, _inst, comp) = list_row_without_preferred();
    assert!(swap_target_exists(s.app(), comp));
    let others: Vec<NodeId> = s.app().doc.nodes.values().filter(|n| matches!(n.kind, NodeKind::Component(_))).map(|n| n.id).collect();
    let family = designos_doc::components_of(&s.app().doc, s.app().doc.get(comp).unwrap().parent.unwrap());
    for id in others.into_iter().filter(|c| !family.contains(c)) {
        let n = s.shell.app.doc.get_mut(id).unwrap();
        n.kind = NodeKind::Frame(n.kind.frame_props().cloned().unwrap_or_default());
    }
    s.shell.app.doc.bump_mutation_epoch();
    assert!(!swap_target_exists(s.app(), comp), "only the edited component's own family is left");
}
