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
    assert!(labels.contains(&"Mark as ready for dev".to_string()), "{labels:?}");
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
        let menu_h = Language::ALL.len() as f32 * 24.0 + 2.0 * theme::MENU_PADDING;
        let top = (dd.bottom() + 2.0).min(s.ctx.content_rect().bottom() - menu_h - 2.0);
        let y = top + theme::MENU_PADDING + i as f32 * 24.0 + 12.0;
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
    let id = lib.all_files().into_iter().find(|f| f.name == "Welcome").expect("seeded file").id;
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
