//! UI tests for the projects library: the home screen, opening / renaming / saving
//! library files, trash, drag and drop, and the removed account / plan labels.

use super::*;
use crate::home::{Act, DropTo, Intent, Section};
use designos_doc::Document;
use designos_library::Library;
use std::path::PathBuf;

pub(super) struct TmpDir(PathBuf);

impl TmpDir {
    fn new() -> Self {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("designos_home_ui_{}_{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        Self(p)
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        // The thumbnail thread may still be writing; ignore failures.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(super) fn home_sim() -> (Sim, TmpDir) {
    let dir = TmpDir::new();
    let s = Sim::with(Options { home: true, library: Some(dir.0.clone()), ..Default::default() });
    (s, dir)
}

fn lib(s: &Sim) -> Library {
    s.app().lib.clone().expect("a library")
}

fn click_rect(s: &mut Sim, name: &str) {
    let r = s.rect_of(name);
    s.click(r.center());
}

fn double_click(s: &mut Sim, p: Pos2) {
    s.click(p);
    s.click(p);
}

fn run_action(s: &mut Sim, a: crate::panels::menus::Action) {
    let ctx = s.ctx.clone();
    crate::panels::menus::run(&mut s.shell.app, &ctx, a);
}

fn file_id(s: &Sim, name: &str) -> String {
    lib(s).all_files().into_iter().find(|f| f.name == name).unwrap_or_else(|| panic!("no library file {name}")).id
}

fn open(s: &mut Sim, name: &str) {
    let id = file_id(s, name);
    crate::home::apply(&mut s.shell.app, Act::Open(id));
    s.frame(vec![]);
    s.frame(vec![]);
}

/// Frames until `secs` of UI time have passed.
fn wait(s: &mut Sim, secs: f64) {
    let end = s.t + secs;
    while s.t < end {
        s.frame(vec![]);
    }
}

#[test]
fn first_launch_shows_the_home_screen_with_the_getting_started_project() {
    let (s, _d) = home_sim();
    let home = s.app().home.as_ref().expect("home screen");
    assert_eq!(home.section, Section::Recents);
    let names: Vec<_> = home.snap.projects.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Getting started"]);
    assert_eq!(home.snap.files.len(), 1);
    assert_eq!(home.snap.files[0].name, "DESIGN-OS Studio Starter");
    // the seeded file carries the starter document and already has a thumbnail
    let l = lib(&s);
    assert!(l.load_file(&home.snap.files[0].id).unwrap().nodes.len() > 10);
    assert!(l.thumbnail_path(&home.snap.files[0]).is_some());
    let card = s.rect_of("home-card-DESIGN-OS Studio Starter");
    assert!(card.width() > 150.0 && card.min.x > theme::HOME_SIDEBAR_W);
    s.rect_of("home-project-Getting started");
    s.rect_of("home-new-file");
    s.rect_of("home-import");
    assert_eq!(s.shell.title, theme::APP_TITLE);
}

fn painted_home(s: &Sim) -> (Vec<egui::epaint::RectShape>, Vec<(String, egui::Color32)>) {
    fn walk(shape: &egui::Shape, rects: &mut Vec<egui::epaint::RectShape>, texts: &mut Vec<(String, egui::Color32)>) {
        match shape {
            egui::Shape::Rect(r) => rects.push(r.clone()),
            egui::Shape::Text(t) => {
                let color = t.override_text_color.or_else(|| t.galley.job.sections.first().map(|sec| sec.format.color)).unwrap_or(egui::Color32::TRANSPARENT);
                texts.push((t.galley.text().to_string(), color));
            }
            egui::Shape::Vec(inner) => inner.iter().for_each(|s| walk(s, rects, texts)),
            _ => {}
        }
    }
    let mut rects = Vec::new();
    let mut texts = Vec::new();
    for clipped in &s.shapes {
        walk(&clipped.shape, &mut rects, &mut texts);
    }
    (rects, texts)
}

#[test]
fn home_chrome_uses_studio_theme_colours() {
    let (s, _d) = home_sim();
    let (rects, texts) = painted_home(&s);

    let side = rects
        .iter()
        .filter(|r| (r.rect.width() - theme::HOME_SIDEBAR_W).abs() < 0.5 && r.rect.height() > 400.0 && r.fill.a() == 255)
        .max_by(|a, b| a.rect.height().total_cmp(&b.rect.height()))
        .expect("sidebar background");
    assert_eq!(side.fill, theme::BG, "sidebar background");

    let main = rects
        .iter()
        .filter(|r| r.rect.left() >= theme::HOME_SIDEBAR_W - 0.5 && r.rect.width() > 800.0 && r.rect.height() > 400.0 && r.fill.a() == 255)
        .max_by(|a, b| (a.rect.width() * a.rect.height()).total_cmp(&(b.rect.width() * b.rect.height())))
        .expect("main area background");
    assert_eq!(main.fill, theme::BG, "main area background");

    let card = s.rect_of("home-card-DESIGN-OS Studio Starter");
    let card_fill = rects.iter().find(|r| r.rect == card && r.fill.a() == 255).expect("file card fill");
    assert_eq!(card_fill.fill, theme::BG_SECONDARY, "file card fill");
    let card_stroke = rects.iter().find(|r| r.rect == card && r.stroke.width > 0.0).expect("file card stroke");
    assert_eq!(card_stroke.stroke.color, theme::BORDER, "file card stroke");

    let button = s.rect_of("home-new-file");
    let button_fill = rects.iter().find(|r| r.rect == button && r.fill.a() == 255).expect("+ Design file fill");
    assert_eq!(button_fill.fill, theme::BG_BRAND, "+ Design file fill");
    let label = texts.iter().find(|(text, _)| text == "+ Design file").expect("+ Design file label");
    assert_eq!(label.1, theme::TEXT_ON_BRAND, "+ Design file label");
}

#[test]
fn new_design_file_creates_and_opens_an_untitled_library_file() {
    let (mut s, _d) = home_sim();
    click_rect(&mut s, "home-new-file");
    assert!(s.app().home.is_none(), "the editor opens");
    let lf = s.app().lib_file.clone().expect("a library file is open");
    assert_eq!((lf.name.as_str(), lf.project.as_str()), ("Untitled", "Getting started"));
    assert_eq!(s.app().doc.name, "Untitled");
    assert_eq!(lib(&s).all_files().len(), 2);
    assert!(lib(&s).load_file(&lf.id).is_ok());
}

#[test]
fn double_clicking_a_card_opens_it_and_single_click_selects() {
    let (mut s, _d) = home_sim();
    let card = s.rect_of("home-card-DESIGN-OS Studio Starter");
    s.click(card.center());
    assert_eq!(s.app().home.as_ref().unwrap().selected.as_deref(), Some(file_id(&s, "DESIGN-OS Studio Starter").as_str()));
    assert!(s.app().home.is_some());
    let before = lib(&s).file(&file_id(&s, "DESIGN-OS Studio Starter")).unwrap().opened;
    double_click(&mut s, card.center());
    assert!(s.app().home.is_none());
    assert_eq!(s.app().lib_file.as_ref().unwrap().name, "DESIGN-OS Studio Starter");
    assert!(s.app().doc.nodes.len() > 10, "the starter document loaded");
    assert!(lib(&s).file(&file_id(&s, "DESIGN-OS Studio Starter")).unwrap().opened > before, "opening updates Recents");
}

#[test]
fn the_editor_header_shows_project_and_file_and_none_of_the_account_labels() {
    let (mut s, _d) = home_sim();
    open(&mut s, "DESIGN-OS Studio Starter");
    let r = s.rect_of("file-header");
    assert_eq!(theme::RAIL_WIDTH, 48.0, "the icon-only rail uses the 48px token width");
    assert_eq!(r.left(), theme::RAIL_WIDTH + theme::BORDER_WIDTH + theme::INSET_FRAME, "the header sits in the left panel right after the rail");
    assert!(r.left() >= theme::RAIL_WIDTH, "the header sits in the left panel");
    // the second line is the save state, not a plan
    assert_eq!(s.app().lib_file.as_ref().unwrap().project, "Getting started");
    assert!(!s.app().autosave.pending());
}

#[test]
fn renaming_in_the_header_persists_to_the_library() {
    let (mut s, _d) = home_sim();
    open(&mut s, "DESIGN-OS Studio Starter");
    let id = s.app().lib_file.as_ref().unwrap().id.clone();
    let hit = s.rect_of("file-header");
    double_click(&mut s, Pos2::new(hit.left() + 90.0, hit.center().y));
    assert!(s.app().renaming_file, "double-click starts renaming");
    s.frame(vec![]);
    s.frame(vec![]);
    s.type_text("Launch page");
    s.key(Key::Enter);
    s.frame(vec![]);
    assert!(!s.app().renaming_file);
    assert_eq!(lib(&s).file(&id).unwrap().name, "Launch page");
    assert_eq!(s.app().lib_file.as_ref().unwrap().name, "Launch page");
    assert_eq!(s.app().doc.name, "Launch page");
}

/// The Agents panel's `Other CLI…` form was open with the caret in it when the user went back to the File panel and
/// renamed the file: the form is hidden behind that panel, so the name field gets every letter.
#[test]
fn renaming_in_the_header_gets_the_letters_after_the_other_cli_form_was_open() {
    let (mut s, _d) = home_sim();
    open(&mut s, "DESIGN-OS Studio Starter");
    s.use_stub_agent_clis();
    click_rect(&mut s, "rail-Agents");
    let other = crate::agent_picker::rows_for_state(&mut s.shell.app.terminal)
        .into_iter()
        .find(|r| r.kind == crate::agent_cli_catalog::PickerRowKind::OtherCli)
        .expect("the Other CLI row");
    for _ in 0..400 {
        s.frame(vec![]);
        if !s.shell.app.terminal.picker.cache.is_probing() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    click_rect(&mut s, "agents-composer-agent-pill");
    s.shell.app.terminal.picker.menu_more_open = true;
    s.frame(vec![]);
    click_rect(&mut s, &other.id);
    click_rect(&mut s, "agents-other-cli-label-input");
    s.type_text("ab");
    assert_eq!(s.app().terminal.picker.other_label_input, "ab", "precondition: the form had the caret and took the letters");

    click_rect(&mut s, "rail-File");
    // A click within 0.3 s of the one on the rail would count as part of a triple click.
    wait(&mut s, 0.5);
    let hit = s.rect_of("file-header");
    double_click(&mut s, Pos2::new(hit.left() + 90.0, hit.center().y));
    assert!(s.app().renaming_file, "double-click starts renaming");
    s.frame(vec![]);
    s.frame(vec![]);
    s.type_text("Launch page");
    s.key(Key::Enter);
    s.frame(vec![]);
    assert_eq!(s.app().lib_file.as_ref().unwrap().name, "Launch page", "the name field took the letters");
    assert!(s.app().terminal.picker.other_label_input.is_empty(), "and the closed form none");
    assert!(s.app().terminal.picker.other_command_input.is_empty());
    for mut tab in s.shell.app.terminal.tabs.drain(..) {
        let _ = tab.child.kill();
    }
}

#[test]
fn renaming_a_file_from_the_home_dialog_persists() {
    let (mut s, _d) = home_sim();
    let id = file_id(&s, "DESIGN-OS Studio Starter");
    crate::home::apply(&mut s.shell.app, Act::RenameFile(id.clone()));
    s.frame(vec![]);
    s.frame(vec![]);
    assert!(matches!(s.app().dialog.as_ref().map(|d| &d.intent), Some(Intent::RenameFile(_))));
    s.type_text("Intro");
    s.frame(vec![]);
    click_rect(&mut s, "dialog-ok");
    assert!(s.app().dialog.is_none());
    assert_eq!(lib(&s).file(&id).unwrap().name, "Intro");
    assert!(s.app().home.as_ref().unwrap().snap.files.iter().any(|f| f.name == "Intro"), "the grid refreshed");
}

#[test]
fn right_click_menu_moves_a_file_to_the_trash_and_trash_restores_it() {
    let (mut s, _d) = home_sim();
    let card = s.rect_of("home-card-DESIGN-OS Studio Starter");
    s.right_click(card.center());
    assert!(s.app().home.as_ref().unwrap().menu.is_some(), "context menu open");
    // "Move to trash" is the 8th row, after three separators
    let pos = card.center();
    let y = pos.y + theme::MENU_PADDING + 7.0 * theme::CONTROL_SM + 3.0 * crate::widgets::menu::SEP_H + theme::CONTROL_SM / 2.0;
    s.click(Pos2::new(pos.x + 60.0, y));
    s.frame(vec![]);
    let l = lib(&s);
    assert!(l.all_files().is_empty());
    assert_eq!(l.list_trash().len(), 1);
    assert!(s.app().home.as_ref().unwrap().snap.files.is_empty());
    // Trash view: restore
    click_rect(&mut s, "home-trash");
    assert_eq!(s.app().home.as_ref().unwrap().section, Section::Trash);
    s.frame(vec![]);
    click_rect(&mut s, "home-restore-DESIGN-OS Studio Starter");
    s.frame(vec![]);
    assert_eq!(l.all_files().len(), 1);
    assert!(l.list_trash().is_empty());
    assert!(s.app().home.as_ref().unwrap().snap.trash.is_empty());
}

#[test]
fn emptying_the_trash_needs_a_confirmation() {
    let (mut s, _d) = home_sim();
    let id = file_id(&s, "DESIGN-OS Studio Starter");
    crate::home::apply(&mut s.shell.app, Act::Trash(id));
    s.frame(vec![]);
    click_rect(&mut s, "home-trash");
    s.frame(vec![]);
    click_rect(&mut s, "home-empty-trash");
    s.frame(vec![]);
    s.frame(vec![]);
    assert_eq!(s.app().dialog.as_ref().map(|d| d.intent.clone()), Some(Intent::EmptyTrash));
    assert_eq!(lib(&s).list_trash().len(), 1, "nothing is deleted before confirming");
    // Cancel keeps everything
    click_rect(&mut s, "dialog-cancel");
    assert!(s.app().dialog.is_none());
    assert_eq!(lib(&s).list_trash().len(), 1);
    // Confirm empties
    click_rect(&mut s, "home-empty-trash");
    s.frame(vec![]);
    s.frame(vec![]);
    click_rect(&mut s, "dialog-ok");
    assert!(lib(&s).list_trash().is_empty());
    s.frame(vec![]);
    assert!(s.app().home.as_ref().unwrap().snap.trash.is_empty());
}

#[test]
fn dragging_a_card_onto_a_project_moves_the_file() {
    let (mut s, _d) = home_sim();
    let l = lib(&s);
    let other = l.create_project("Marketing").unwrap();
    s.shell.app.home.as_mut().unwrap().dirty = true;
    s.frame(vec![]);
    s.frame(vec![]);
    let from = s.rect_of("home-card-DESIGN-OS Studio Starter").center();
    let to = s.rect_of("home-project-Marketing").center();
    s.drag(from, to);
    s.frame(vec![]);
    let f = l.all_files().into_iter().find(|f| f.name == "DESIGN-OS Studio Starter").unwrap();
    assert_eq!(f.project, other.id);
    // and into a folder via the action used by the folder rows
    let folder = l.create_folder(&other.id, None, "Q4").unwrap();
    crate::home::apply(&mut s.shell.app, Act::Drop(f.id.clone(), DropTo::Folder(other.id.clone(), folder.id.clone())));
    assert_eq!(l.file(&f.id).unwrap().folder.as_deref(), Some(folder.id.as_str()));
}

#[test]
fn search_filters_files_and_the_sidebar_navigates_sections() {
    let (mut s, _d) = home_sim();
    let l = lib(&s);
    let p = l.list_projects()[0].id.clone();
    let doc = Document::new("x");
    l.create_file(&p, None, "Checkout flow", &doc).unwrap();
    s.shell.app.home.as_mut().unwrap().dirty = true;
    s.frame(vec![]);
    s.shell.app.home.as_mut().unwrap().search = "check".into();
    s.frame(vec![]);
    let files = s.app().home.as_ref().unwrap().files();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "Checkout flow");
    assert_eq!(s.app().home.as_ref().unwrap().title(), "Results for \u{201c}check\u{201d}");
    // choosing a section clears the search
    click_rect(&mut s, "home-starred");
    let h = s.app().home.as_ref().unwrap();
    assert_eq!((h.section.clone(), h.search.as_str()), (Section::Starred, ""));
    click_rect(&mut s, "home-project-Getting started");
    assert!(matches!(s.app().home.as_ref().unwrap().section, Section::Project(..)));
    assert_eq!(s.app().home.as_ref().unwrap().files().len(), 2);
}

#[test]
fn back_to_files_flushes_edits_and_returns_to_the_home_screen() {
    let (mut s, _d) = home_sim();
    open(&mut s, "DESIGN-OS Studio Starter");
    let id = s.app().lib_file.as_ref().unwrap().id.clone();
    let before = lib(&s).load_file(&id).unwrap().nodes.len();
    s.key(Key::R);
    s.mods = Modifiers::COMMAND;
    s.drag(s.screen(300.0, 800.0), s.screen(380.0, 860.0));
    s.mods = Modifiers::NONE;
    assert_eq!(s.count(), before + 1);
    assert_eq!(lib(&s).load_file(&id).unwrap().nodes.len(), before, "not saved yet");
    run_action(&mut s, crate::panels::menus::Action::BackToFiles);
    s.frame(vec![]);
    assert!(s.app().home.is_some());
    assert_eq!(lib(&s).load_file(&id).unwrap().nodes.len(), before + 1, "leaving the editor saved the edit");
}

#[test]
fn autosave_runs_after_the_idle_delay_and_ctrl_s_saves_at_once() {
    let (mut s, _d) = home_sim();
    open(&mut s, "DESIGN-OS Studio Starter");
    let id = s.app().lib_file.as_ref().unwrap().id.clone();
    let before = lib(&s).load_file(&id).unwrap().nodes.len();
    s.key(Key::R);
    s.mods = Modifiers::COMMAND;
    s.drag(s.screen(300.0, 800.0), s.screen(380.0, 860.0));
    s.mods = Modifiers::NONE;
    wait(&mut s, 0.8);
    assert!(s.app().autosave.pending(), "a save is pending");
    assert_eq!(lib(&s).load_file(&id).unwrap().nodes.len(), before, "not before the 1.5s idle delay");
    wait(&mut s, 1.2);
    assert_eq!(lib(&s).load_file(&id).unwrap().nodes.len(), before + 1, "autosaved");
    assert_eq!(s.app().autosave.saves, 1);
    assert!(!s.app().autosave.pending());
    // Ctrl+S is immediate
    s.mods = Modifiers::COMMAND;
    s.drag(s.screen(500.0, 800.0), s.screen(560.0, 860.0));
    s.mods = Modifiers::NONE;
    s.key(Key::R);
    s.mods = Modifiers::COMMAND;
    s.drag(s.screen(600.0, 800.0), s.screen(660.0, 860.0));
    s.mods = Modifiers::NONE;
    let now = s.count();
    s.cmd_key(Key::S);
    assert_eq!(lib(&s).load_file(&id).unwrap().nodes.len(), now);
    assert!(s.app().toast.as_ref().is_some_and(|t| t.text.starts_with("Saved to")));
}

#[test]
fn unchanged_documents_are_never_rewritten_by_autosave() {
    let (mut s, _d) = home_sim();
    open(&mut s, "DESIGN-OS Studio Starter");
    let id = s.app().lib_file.as_ref().unwrap().id.clone();
    let m0 = lib(&s).file(&id).unwrap().modified;
    // zooming bumps the canvas revision but changes nothing in the document
    s.shell.app.zoom_to(2.0);
    wait(&mut s, 3.0);
    assert_eq!(lib(&s).file(&id).unwrap().modified, m0);
    assert_eq!(s.app().autosave.saves, 0);
}

#[test]
fn external_files_keep_their_name_and_offer_save_to_library() {
    let dir = TmpDir::new();
    std::fs::create_dir_all(&dir.0).unwrap();
    let ext = dir.0.join("outside.designos");
    let mut d = Document::new("Outside");
    d.add_page("P");
    std::fs::write(&ext, d.to_json().unwrap()).unwrap();
    let mut s = Sim::with(Options { file: Some(ext.clone()), library: Some(dir.0.join("lib")), ..Default::default() });
    assert!(s.app().home.is_none(), "a file argument opens the editor");
    assert!(s.app().lib_file.is_none());
    assert_eq!(s.app().doc.name, "Outside");
    click_rect(&mut s, "save-to-library");
    s.frame(vec![]);
    assert_eq!(s.app().dialog.as_ref().map(|d| d.intent.clone()), Some(Intent::SaveToLibrary));
    click_rect(&mut s, "dialog-ok");
    assert!(s.app().dialog.is_none());
    let lf = s.app().lib_file.clone().expect("now a library file");
    assert_eq!(lf.name, "Outside");
    assert_eq!(lib(&s).file(&lf.id).unwrap().name, "Outside");
    assert!(ext.exists(), "the original is untouched");
}

#[test]
fn importing_a_file_adds_it_to_the_project() {
    let (mut s, d) = home_sim();
    let svg = d.0.join("logo.svg");
    std::fs::write(&svg, r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><rect width="8" height="8" fill="#f00"/></svg>"##).unwrap();
    let p = lib(&s).list_projects()[0].id.clone();
    assert_eq!(s.shell.app.import_into_library(&[svg], &p, None), 1);
    s.frame(vec![]);
    s.frame(vec![]);
    assert!(s.app().home.as_ref().unwrap().snap.files.iter().any(|f| f.name == "logo"));
}

#[test]
fn account_and_plan_labels_are_gone_from_the_ui_sources() {
    // The product has no accounts, plans or cloud; guard against the Figma labels coming back.
    let sources = [
        ("left.rs", include_str!("../../panels/left.rs")),
        ("right.rs", include_str!("../../panels/right.rs")),
        ("rail.rs", include_str!("../../panels/rail.rs")),
        ("menus.rs", include_str!("../../panels/menus.rs")),
        ("toolbar.rs", include_str!("../../panels/toolbar.rs")),
    ];
    let banned = ["\"Drafts\"", "\"Free\"", "Upgrade", "\"Team\"", "\"Pro\""];
    for (file, src) in sources {
        for b in banned {
            assert!(!src.contains(b), "{file} still mentions {b}");
        }
    }
    assert!(crate::panels::rail::ITEMS.iter().any(|i| i.2 == "Agents"));
}

#[test]
fn help_menu_offers_shortcuts_about_and_a_confirmed_github_link() {
    let (mut s, _d) = home_sim();
    open(&mut s, "DESIGN-OS Studio Starter");
    run_action(&mut s, crate::panels::menus::Action::Shortcuts);
    s.frame(vec![]);
    assert_eq!(s.app().dialog.as_ref().map(|d| d.intent.clone()), Some(Intent::Shortcuts));
    s.key(Key::Escape);
    assert!(s.app().dialog.is_none());
    run_action(&mut s, crate::panels::menus::Action::AboutDesignOS);
    s.frame(vec![]);
    assert_eq!(s.app().dialog.as_ref().map(|d| d.intent.clone()), Some(Intent::About));
    click_rect(&mut s, "dialog-ok");
    assert!(s.app().dialog.is_none());
    // the GitHub link asks first and is cancellable
    run_action(&mut s, crate::panels::menus::Action::OpenGithub);
    s.frame(vec![]);
    assert_eq!(s.app().dialog.as_ref().map(|d| d.intent.clone()), Some(Intent::OpenGithub));
    click_rect(&mut s, "dialog-cancel");
    assert!(s.app().dialog.is_none());
    assert!(s.app().toast.as_ref().is_none_or(|t| !t.error));
}

#[test]
fn library_option_sets_the_root_and_home_option_shows_the_home_screen() {
    let dir = TmpDir::new();
    let s = Sim::with(Options { home: true, library: Some(dir.0.clone()), screenshot: Some(dir.0.join("shot.png")), ..Default::default() });
    assert_eq!(lib(&s).root(), dir.0.as_path());
    assert!(s.app().home.is_some());
    assert!(dir.0.join("projects").exists(), "the library was created under --library");
    // without --home a screenshot run stays in the editor and never touches a library
    let e = Sim::with(Options { screenshot: Some(dir.0.join("e.png")), ..Default::default() });
    assert!(e.app().home.is_none() && e.app().lib.is_none());
}

#[test]
fn userflows_uf01_to_uf08_end_to_end_figma_parity() {
    use crate::app::{MenuKind, RailItem, RightTab};
    use crate::canvas::tools::Tool;

    let mut s = Sim::new();
    // The runtime picks below start agent CLIs. Stubs that stay alive keep the tab bar still between a click and
    // the next one, on a machine with the real CLIs and on a runner without any.
    s.use_stub_agent_clis();
    s.frame(vec![]);

    // UF-01: Left Icon Rail Navigation & Collapse
    assert_eq!(s.app().ui.rail, None);
    assert!(!s.app().ui.left_collapsed);

    // Click Agents -> opens Agents panel
    click_rect(&mut s, "rail-Agents");
    assert_eq!(s.app().ui.rail, Some(RailItem::Agents));
    assert!(!s.app().ui.left_collapsed);

    // Default Interactive Shell + the agent menu of the composer pill (Claude / Codex / Grok / Antigravity)
    assert_eq!(s.app().terminal.active_runtime, crate::terminal::AgentCliPreset::Shell);
    assert!(s.app().terminal.side_panel_pty_open);
    click_rect(&mut s, "agents-composer-agent-pill");
    assert!(s.app().terminal.runtime_dropdown_open);
    click_rect(&mut s, "agents-dropdown-codex");
    assert_eq!(s.app().terminal.active_runtime, crate::terminal::AgentCliPreset::Codex);
    click_rect(&mut s, "agents-composer-agent-pill");
    click_rect(&mut s, "agents-dropdown-antigravity");
    assert_eq!(s.app().terminal.active_runtime, crate::terminal::AgentCliPreset::Antigravity);
    click_rect(&mut s, "agents-composer-agent-pill");
    click_rect(&mut s, "agents-dropdown-claude");
    assert_eq!(s.app().terminal.active_runtime, crate::terminal::AgentCliPreset::Claude);
    // One tab per pick, all still running: a tab that exited would have moved the pill mid-flow.
    #[cfg(unix)]
    {
        let terminal = &mut s.shell.app.terminal;
        assert_eq!(terminal.tabs.len(), 3, "Codex, Antigravity and Claude each started a tab");
        for tab in terminal.tabs.iter_mut() {
            assert!(
                tab.start_failure.is_none() && tab.is_alive(),
                "{} must still be running, not {:?}",
                tab.title,
                tab.start_failure.as_ref().map(|f| f.cause)
            );
        }
    }

    s.shell.app.terminal.submit_chat_prompt("Create a SaaS analytics card in Claude Code", None);
    assert!(
        s.app().terminal.active_composer().pending_send.as_deref() == Some("Create a SaaS analytics card in Claude Code")
            || s.app().terminal.active_composer().history.back().map(String::as_str) == Some("Create a SaaS analytics card in Claude Code")
            || s.app().terminal.active_composer().last_outcome.is_some()
    );

    // Click File while in Agents -> switches to File panel in 1 click (not MenuKind::FileMenu)
    click_rect(&mut s, "rail-File");
    assert_eq!(s.app().ui.rail, None);
    assert!(!s.app().ui.left_collapsed);
    assert_eq!(s.app().ui.menu, None);

    // Click File again while already on File -> collapses Left Sidebar
    click_rect(&mut s, "rail-File");
    assert!(s.app().ui.left_collapsed);

    // Click Assets while collapsed -> expands Left Sidebar and shows Assets panel
    click_rect(&mut s, "rail-Assets");
    assert!(!s.app().ui.left_collapsed);
    assert_eq!(s.app().ui.rail, Some(RailItem::Assets));

    // UF-02: Assets Panel Library Card & View Toggle & Libraries Modal
    assert!(!s.app().assets.grid);
    click_rect(&mut s, "assets-view-grid");
    assert!(s.app().assets.grid);
    click_rect(&mut s, "assets-view-list");
    assert!(!s.app().assets.grid);
    click_rect(&mut s, "assets-libraries");
    assert!(s.app().libs.open);
    s.key(Key::Escape);
    s.frame(vec![]);
    assert!(!s.app().libs.open);

    // Return to File panel and test UF-03: Collapse Layers button (`layers-collapse-all`)
    click_rect(&mut s, "rail-File");
    assert_eq!(s.app().ui.rail, None);
    assert!(!s.app().ui.left_collapsed);
    let card_id = s.by_name("Card");
    s.shell.app.layers.expanded.insert(card_id);
    s.frame(vec![]);
    assert!(!s.app().layers.expanded.is_empty());
    click_rect(&mut s, "layers-collapse-all");
    assert!(s.app().layers.expanded.is_empty());

    // UF-04: Bottom Floating Toolbar 3-Way Mode Switcher (`Draw`, `Design`, `Dev mode`)
    click_rect(&mut s, "toolbar-mode-draw");
    assert!(!s.app().ui.dev_mode);
    assert_eq!(s.app().tool, Tool::Pencil);
    click_rect(&mut s, "toolbar-mode-design");
    assert!(!s.app().ui.dev_mode);
    assert_eq!(s.app().tool, Tool::Move);
    click_rect(&mut s, "toolbar-mode-dev");
    assert!(s.app().ui.dev_mode);
    assert_eq!(s.app().ui.right_tab, Some(RightTab::Design));
    // Return to Design mode via Dev Mode bottom dock toggle
    click_rect(&mut s, "dev-mode-toggle");
    assert!(!s.app().ui.dev_mode);

    // UF-05: Actions / Command Palette (`Ctrl+K` & Toolbar Sparkle Button)
    click_rect(&mut s, "toolbar-actions-btn");
    assert_eq!(s.app().ui.menu, Some(MenuKind::Palette));
    s.key(Key::Escape);
    assert_eq!(s.app().ui.menu, None);

    // UF-06 & UF-07: Right Inspector Contextual Sections & the Cmd+` toggle of the Agents panel
    s.shell.app.select_only(card_id);
    s.frame(vec![]);
    assert_eq!(s.app().selection, vec![card_id]);
    s.cmd_key(Key::Backtick);
    assert_eq!(s.app().ui.rail, Some(RailItem::Agents));
    // The terminal keeps the keyboard while it has focus; the toggle is for the moment nothing in the panel does.
    s.shell.app.terminal.viewport_focused = false;
    s.cmd_key(Key::Backtick);
    assert_eq!(s.app().ui.rail, None);

    // UF-08: Main Menu (`AI balance`, `Help and account`)
    click_rect(&mut s, "rail-menu");
    assert_eq!(s.app().ui.menu, Some(MenuKind::Main));
    s.key(Key::Escape);
    assert_eq!(s.app().ui.menu, None);

    for mut tab in s.shell.app.terminal.tabs.drain(..) {
        let _ = tab.child.kill();
    }
}

#[test]
fn import_without_a_dialog_opens_path_prompt_from_toast_and_imports_file() {
    let (mut s, d) = home_sim();
    let sub = d.0.join("Thư mục 2026");
    std::fs::create_dir_all(&sub).unwrap();
    let file = sub.join("Thiết kế mẫu.designos");
    let mut doc = Document::new("Thiết kế mẫu");
    doc.add_page("Trang 1");
    crate::io::save_path(&doc, &file).unwrap();

    crate::home::import_files_dialog(&mut s.shell.app, &crate::io::ScriptedDialogs::answering(crate::io::DialogOutcome::Unavailable));
    s.frame(vec![]);
    assert!(s.app().toast.as_ref().is_some_and(|t| t.enter_path));

    click_rect(&mut s, "toast-enter-path");
    assert!(s.app().toast.is_none());
    assert_eq!(s.app().dialog.as_ref().map(|dlg| dlg.intent.clone()), Some(Intent::ImportPath));
    assert_eq!(s.app().dialog.as_ref().and_then(|dlg| dlg.hint.as_deref()), Some("Path to a .fig, .designos, .json or .svg file"));
    assert!(!s.app().dialog.as_ref().unwrap().can_submit(), "Import stays off while the field is empty");

    s.type_text(&file.display().to_string());
    s.key(Key::Enter);
    assert!(s.app().dialog.is_none());
    assert!(s.app().home.is_none(), "entering a path opens the file in a tab");
    assert_eq!(s.app().doc.name, "Thiết kế mẫu");
    assert_eq!(s.app().file_path.as_deref(), Some(file.as_path()));
}

fn open_import_prompt(s: &mut Sim) {
    crate::home::import_files_dialog(&mut s.shell.app, &crate::io::ScriptedDialogs::answering(crate::io::DialogOutcome::Unavailable));
    s.frame(vec![]);
    click_rect(s, "toast-enter-path");
    assert_eq!(s.app().dialog.as_ref().map(|dlg| dlg.intent.clone()), Some(Intent::ImportPath));
}

fn toast_is_painted(s: &Sim, text: &str) -> bool {
    s.shapes.iter().any(|c| matches!(&c.shape, egui::Shape::Text(t) if t.galley.text() == text))
}

#[test]
fn typed_import_path_from_home_opens_the_fig() {
    let (mut s, d) = home_sim();
    let home = d.0.join("typed-home");
    std::fs::create_dir_all(&home).unwrap();
    let _guard = crate::home::dialog::set_import_home_for_test(home.clone());
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../designos-fig/tests/fixtures/legacy-0.4.0-rich.fig");
    let name = "legacy-0.4.0-rich.fig";
    let dest = home.join(name);
    std::fs::copy(&src, &dest).expect("copy the .fig into the test home");

    open_import_prompt(&mut s);
    s.type_text(&format!("~/{name}"));
    s.key(Key::Enter);

    assert!(s.app().dialog.is_none(), "the prompt closes");
    assert!(s.app().home.is_none(), "entering ~/file.fig opens the file");
    assert_eq!(s.app().file_path.as_deref(), Some(dest.as_path()));
}

#[test]
fn typed_import_path_relative_shows_the_message() {
    let (mut s, _d) = home_sim();
    open_import_prompt(&mut s);
    s.type_text("docs/x.fig");
    s.key(Key::Enter);
    s.frame(vec![]);

    assert!(s.app().dialog.is_none(), "the prompt closes");
    assert!(s.app().home.is_some(), "Home stays open");
    let toast = s.app().toast.as_ref().expect("an error toast");
    assert!(toast.error, "the toast is an error");
    assert_eq!(toast.text, "Type the full path to the file, starting with / or ~/.");
    assert!(toast_is_painted(&s, &toast.text), "the message is painted on the next frame");
}

#[test]
fn round3_e_save_as_and_image_fill_unavailable_show_toast_without_enter_path_while_import_has_it() {
    const NO_FILE_DIALOG: &str = "No file dialog. Install zenity or xdg-desktop-portal.";
    let (mut s, _d) = home_sim();
    let unavailable = crate::io::ScriptedDialogs::answering(crate::io::DialogOutcome::Unavailable);

    // 1. Save As with Unavailable file dialog shows plain NO_FILE_DIALOG toast with no Enter a path… action.
    s.shell.app.save_with(true, &unavailable);
    s.frame(vec![]);
    let toast = s.app().toast.as_ref().expect("Save As unavailable must show NO_FILE_DIALOG toast");
    assert_eq!(toast.text, NO_FILE_DIALOG);
    assert!(!toast.enter_path, "Save As toast must not attach Enter a path… action");
    assert!(
        s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", "toast-enter-path".to_string())))).is_none(),
        "toast-enter-path button must not render for Save As"
    );

    // 2. Image fill with Unavailable file dialog also shows plain NO_FILE_DIALOG toast with no Enter a path… action.
    s.shell.app.toast = None;
    let mut paint = designos_doc::Paint::solid(designos_doc::Color::WHITE);
    assert!(!crate::panels::properties::choose_image_fill(&mut s.shell.app, &mut paint, &unavailable));
    s.frame(vec![]);
    let toast = s.app().toast.as_ref().expect("Image fill unavailable must show NO_FILE_DIALOG toast");
    assert_eq!(toast.text, NO_FILE_DIALOG);
    assert!(!toast.enter_path, "Image fill toast must not attach Enter a path… action");
    assert!(
        s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", "toast-enter-path".to_string())))).is_none(),
        "toast-enter-path button must not render for image fill"
    );

    // 3. Home > Import with Unavailable file dialog still attaches Enter a path… action.
    s.shell.app.toast = None;
    crate::home::import_files_dialog(&mut s.shell.app, &unavailable);
    s.frame(vec![]);
    let toast = s.app().toast.as_ref().expect("Home > Import unavailable must show NO_FILE_DIALOG toast");
    assert_eq!(toast.text, NO_FILE_DIALOG);
    assert!(toast.enter_path, "Home > Import toast must keep Enter a path… action");
    assert!(
        s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", "toast-enter-path".to_string())))).is_some(),
        "toast-enter-path button must render for Home > Import"
    );
}

fn painted_texts(s: &Sim) -> Vec<String> {
    s.shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Text(t) => Some(t.galley.text().to_string()),
            _ => None,
        })
        .collect()
}

fn has_home_rect(s: &Sim, name: &str) -> bool {
    s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", name.to_string())))).is_some()
}

#[test]
fn starter_asset_exists_and_is_at_most_5_mb() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/starter/designos-studio-starter.designos");
    assert!(path.is_file(), "starter asset must exist at {}", path.display());
    let bytes = std::fs::read(&path).expect("read starter asset");
    assert!(bytes.len() <= 5 * 1024 * 1024, "starter asset is {} bytes (> 5 MB)", bytes.len());
    let text = std::str::from_utf8(&bytes).expect("starter asset is UTF-8");
    let doc = Document::from_json(text).expect("starter asset decodes as a valid Document");
    let page_names: Vec<&str> = doc.pages.iter().filter_map(|pid| doc.get(*pid).map(|n| n.name.as_str())).collect();
    assert_eq!(doc.pages.len(), 2, "starter asset must have 2 pages, found {page_names:?}");
    assert_eq!(doc.get(doc.pages[0]).unwrap().children.len(), 3, "first page of starter asset must have 3 frames");
    assert!(page_names.contains(&"About this file"), "starter asset must contain an 'About this file' page, found {page_names:?}");
}

#[test]
fn ac01_fresh_library_seeds_getting_started_and_starter_file() {
    let (s, _d) = home_sim();
    let l = lib(&s);
    let projects = l.list_projects();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].name, "Getting started");
    let files = l.all_files();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "DESIGN-OS Studio Starter");
    assert!(l.thumbnail_path(&files[0]).is_some(), "starter file must have its thumbnail generated on seed");
    assert!(l.root().join(".starter-seeded").is_file(), ".starter-seeded marker must exist");
}

#[test]
fn ac02_no_arg_launch_opens_home_and_never_builtin_demo() {
    let d = TmpDir::new();
    let mut opts = crate::cli::parse(std::iter::empty::<String>()).unwrap();
    assert!(opts.wants_home());
    opts.home = true;
    opts.library = Some(d.0.clone());
    let s = Sim::with(opts);
    assert!(s.app().home.is_some(), "no-arg launch must open Home");
    assert_eq!(s.app().doc.name, "Untitled", "underlying editor document must be blank Untitled, not demo_document");
    assert_eq!(s.app().doc.nodes.len(), 1, "no-arg launch must build an empty document with only Page 1, never demo_document()");
    assert!(s.app().doc.nodes.values().all(|n| n.name != "Welcome" && n.name != "Hero"), "no-arg launch must never build demo_document()");
}

#[test]
fn ac04_home_screen_contains_none_of_the_legacy_names() {
    let (mut s, _d) = home_sim();
    let forbidden = ["Marlow", "Welcome", "Launch Brainstorm", "Product Exploration", "Design Token Sheet", "Product UI Kit", "Starter Kits"];
    for nav in ["home-nav-recents", "home-nav-getting-started", "home-nav-templates", "home-nav-resources"] {
        if has_home_rect(&s, nav) {
            click_rect(&mut s, nav);
            s.frame(vec![]);
        }
        let painted = painted_texts(&s);
        for bad in forbidden {
            assert!(!painted.iter().any(|t| t.contains(bad)), "Home view {nav} painted legacy sample name {bad:?} in {painted:?}");
        }
    }
}

#[test]
fn ac05_upgrade_from_v062_library_seeds_starter_once_and_handles_name_clash() {
    let d = TmpDir::new();
    let l = Library::open(&d.0).unwrap();
    let p = l.create_project("Getting started").unwrap();
    let mut welcome = Document::new("Welcome");
    welcome.add_page("Page 1");
    l.create_file(&p.id, None, "Welcome", &welcome).unwrap();
    let mut marlow = Document::new("Marlow — Mobile Banking Kit");
    marlow.add_page("Page 1");
    l.create_file(&p.id, None, "Marlow — Mobile Banking Kit", &marlow).unwrap();
    std::fs::write(d.0.join(".seeded"), b"1\n").unwrap();

    // Upgrade launch: `.seeded` exists, `.starter-seeded` does not.
    let s = Sim::with(Options { home: true, library: Some(d.0.clone()), ..Default::default() });
    let l = lib(&s);
    let names: Vec<String> = l.all_files().into_iter().map(|f| f.name).collect();
    assert!(names.contains(&"Welcome".to_string()), "existing Welcome file kept: {names:?}");
    assert!(names.contains(&"Marlow — Mobile Banking Kit".to_string()), "existing Marlow copy kept: {names:?}");
    assert!(names.contains(&"DESIGN-OS Studio Starter".to_string()), "starter seeded once on upgrade: {names:?}");
    assert_eq!(names.len(), 3);
    assert!(d.0.join(".starter-seeded").is_file());

    // Delete "Getting started" and relaunch: no reseed because `.starter-seeded` is present.
    l.delete_project(&p.id).unwrap();
    let s2 = Sim::with(Options { home: true, library: Some(d.0.clone()), ..Default::default() });
    assert!(lib(&s2).all_files().is_empty(), "deleting Getting started after starter seed must not reseed on relaunch");

    // Name clash on upgrade: existing "Getting started" already has a user file named "DESIGN-OS Studio Starter".
    let d_clash = TmpDir::new();
    let l_clash = Library::open(&d_clash.0).unwrap();
    let p_clash = l_clash.create_project("Getting started").unwrap();
    let mut user_doc = Document::new("User file");
    user_doc.add_page("User page");
    let user_entry = l_clash.create_file(&p_clash.id, None, "DESIGN-OS Studio Starter", &user_doc).unwrap();
    std::fs::write(d_clash.0.join(".seeded"), b"1\n").unwrap();

    let s_clash = Sim::with(Options { home: true, library: Some(d_clash.0.clone()), ..Default::default() });
    let clash_files = lib(&s_clash).all_files();
    let clash_names: Vec<&str> = clash_files.iter().map(|f| f.name.as_str()).collect();
    assert!(clash_names.contains(&"DESIGN-OS Studio Starter"), "{clash_names:?}");
    assert!(clash_names.contains(&"DESIGN-OS Studio Starter (1)"), "clashing upgrade seed must suffix with (1): {clash_names:?}");
    let reloaded_user = lib(&s_clash).load_file(&user_entry.id).unwrap();
    assert!(reloaded_user.nodes.values().any(|n| n.name == "User page"), "user file was never overwritten");
}

#[test]
fn ac06_missing_recent_file_shows_file_not_found_and_remove() {
    let d = TmpDir::new();
    let l = Library::open(&d.0).unwrap();
    let p = l.create_project("Getting started").unwrap();
    let mut doc = Document::new("Missing draft");
    doc.add_page("Page 1");
    let entry = l.create_file(&p.id, None, "Missing draft", &doc).unwrap();
    std::fs::write(d.0.join(".seeded"), b"1\n").unwrap();
    std::fs::write(d.0.join(".starter-seeded"), b"1\n").unwrap();

    // Remove the underlying .designos file from disk so the manifest entry points to a missing file.
    let disk_path = crate::home::library_file_disk_path(&l, &p.id, &entry.id);
    std::fs::remove_file(&disk_path).unwrap();

    let mut s = Sim::with(Options { home: true, library: Some(d.0.clone()), ..Default::default() });
    let painted = painted_texts(&s);
    assert!(painted.iter().any(|t| t.contains("File not found")), "missing recent file must show 'File not found', got {painted:?}");
    assert!(painted.iter().any(|t| t == "Remove"), "missing recent file must offer 'Remove', got {painted:?}");

    click_rect(&mut s, "home-remove-Missing draft");
    s.frame(vec![]);
    assert!(lib(&s).all_files().is_empty(), "clicking Remove drops the missing file entry from the library");
}

#[test]
fn ac10_help_open_starter_sample_creates_numbered_copies_in_current_project() {
    let (mut s, _d) = home_sim();
    let id0 = lib(&s).all_files()[0].id.clone();
    s.shell.app.open_library_file(&id0);
    s.frame(vec![]);

    for expected in ["DESIGN-OS Studio Starter (1)", "DESIGN-OS Studio Starter (2)"] {
        assert!(
            crate::panels::menus::preset_menu(&s.ctx, &mut s.shell.app, "main/Help and account/Open starter sample", s.ctx.content_rect()),
            "Help and account -> Open starter sample must exist"
        );
        s.frame(vec![]);
        s.key(Key::Enter);
        s.frame(vec![]);
        let names: Vec<String> = lib(&s).all_files().into_iter().map(|f| f.name).collect();
        assert!(names.contains(&expected.to_string()), "expected {expected:?} in {names:?}");
    }
}

#[test]
fn corrupt_starter_asset_toasts_and_leaves_library_usable_without_marker() {
    let d = TmpDir::new();
    crate::home::TEST_STARTER_BYTES_OVERRIDE.with(|cell| cell.set(Some(b"{not valid json")));
    let s = Sim::with(Options { home: true, library: Some(d.0.clone()), ..Default::default() });
    crate::home::TEST_STARTER_BYTES_OVERRIDE.with(|cell| cell.set(None));

    let toast = s.app().toast.as_ref().expect("corrupt starter asset must show an error toast");
    assert!(toast.error);
    assert_eq!(toast.text, "Could not create the starter sample");
    assert!(s.app().lib.is_some(), "library remains usable after corrupt starter asset");
    assert!(!d.0.join(".starter-seeded").exists(), "no .starter-seeded marker written when starter asset fails");
    assert!(!d.0.join(".seeded").exists(), "no .seeded marker written when fresh starter asset fails");
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

#[test]
fn home_header_buttons_are_button_sm() {
    let (s, _d) = home_sim();
    let head = theme::HOME_HEADER_H;
    for name in ["home-new-file", "home-import"] {
        let r = s.rect_of(name);
        assert!(near(r.height(), theme::CONTROL_SM), "{name} height {} want {}", r.height(), theme::CONTROL_SM);
        let side_top = s.rect_of("home-workspace-header").top() - 10.0;
        let cy = r.center().y;
        assert!(near(cy, side_top + head / 2.0), "{name} centre {cy} want {}", side_top + head / 2.0);
    }
}

#[test]
fn home_sidebar_nav_rows_follow_home_density() {
    let (s, _d) = home_sim();
    for name in ["home-recents", "home-trash"] {
        let r = s.rect_of(name);
        assert!(near(r.height(), theme::CONTROL_SM), "{name} height {} want {}", r.height(), theme::CONTROL_SM);
    }
}

#[test]
fn home_filter_bar_controls_are_button_sm() {
    let (s, _d) = home_sim();
    for name in ["home-filter", "home-sort", "home-view-grid"] {
        let r = s.rect_of(name);
        assert!(near(r.height(), theme::CONTROL_SM), "{name} height {} want {}", r.height(), theme::CONTROL_SM);
    }
    let filter = s.rect_of("home-filter");
    let card = s.rect_of("home-card-DESIGN-OS Studio Starter");
    let side_top = s.rect_of("home-workspace-header").top() - 10.0;
    let content_top = side_top + theme::HOME_HEADER_H + theme::CONTROL_SM + 10.0 + 1.0;
    assert!(near(filter.top(), side_top + theme::HOME_HEADER_H + theme::SPACE_FACT), "filter {}", filter.top());
    assert!(near(card.top(), content_top + theme::HOME_PAD), "card {} content {}", card.top(), content_top);
}

fn home_sources(rel: &str) -> &'static str {
    match rel {
        "mod" => include_str!("../../home/mod.rs"),
        "thumb" => include_str!("../../home/thumb.rs"),
        "time" => include_str!("../../home/time.rs"),
        "session" => include_str!("../../home/session.rs"),
        _ => unreachable!(),
    }
}

fn sites_without_texture_tints(src: &str) -> Vec<usize> {
    let clean = super::chrome_census::clean_rust(src);
    let lines: Vec<&str> = clean.split('\n').collect();
    super::chrome_census::chrome_colour_sites(src).into_iter().filter(|line| !super::chrome_census::texture_tint_line(lines[line - 1])).collect()
}

#[test]
fn home_sources_paint_no_chrome_colour_literals() {
    assert_eq!(sites_without_texture_tints(home_sources("mod")), Vec::<usize>::new());
    assert_eq!(super::chrome_census::chrome_colour_sites(home_sources("thumb")), Vec::<usize>::new());
    assert_eq!(super::chrome_census::chrome_colour_sites(home_sources("time")), Vec::<usize>::new());
    assert_eq!(super::chrome_census::chrome_colour_sites(home_sources("session")), Vec::<usize>::new());
}

#[test]
fn home_nav_and_list_rows_use_list_row_look() {
    use crate::widgets::kit::{self, State, Surface};
    let (mut s, _d) = home_sim();
    let selected = kit::row_look(State::Selected, Surface::Primary);
    let (rects, texts) = painted_home(&s);
    let nav = s.rect_of("home-recents");
    let fill = rects.iter().find(|r| r.rect == nav && r.fill == selected.fill).expect("selected nav fill");
    assert_eq!(fill.fill, theme::BG_SELECTED);
    assert!(rects.iter().all(|r| r.rect != nav || r.stroke.color.a() == 0), "selected nav row has no stroke");
    let label = texts.iter().find(|(t, _)| t == "Recents").expect("Recents label");
    assert_eq!(label.1, selected.text);

    click_rect(&mut s, "home-view-list");
    s.frame(vec![]);
    click_rect(&mut s, "home-card-DESIGN-OS Studio Starter");
    s.frame(vec![]);
    let (rects, texts) = painted_home(&s);
    let row = s.rect_of("home-card-DESIGN-OS Studio Starter");
    assert!(near(row.height(), theme::HOME_LIST_ROW_H), "list row stays {}", row.height());
    let row_fill = rects.iter().find(|r| r.rect == row && r.fill == selected.fill).expect("selected list fill");
    assert_eq!(row_fill.fill, theme::BG_SELECTED);
    assert!(rects.iter().all(|r| r.rect != row || r.stroke.color.a() == 0), "selected list row has no stroke");
    let name = texts.iter().find(|(t, _)| t == "DESIGN-OS Studio Starter").expect("list name");
    assert_eq!(name.1, selected.text);
}

#[test]
fn home_thumb_stage_uses_home_thumb_bg() {
    let src = home_sources("mod");
    assert!(src.contains("theme::HOME_THUMB_BG"));
    assert!(!src.contains("let _ = theme::HOME_THUMB_BG"));
}

/// A library whose only file has lost its copy on disk, drawn on Home.
fn missing_file_sim() -> (Sim, TmpDir) {
    let d = TmpDir::new();
    let l = Library::open(&d.0).unwrap();
    let p = l.create_project("Getting started").unwrap();
    let mut doc = Document::new("Missing draft");
    doc.add_page("Page 1");
    let entry = l.create_file(&p.id, None, "Missing draft", &doc).unwrap();
    std::fs::write(d.0.join(".seeded"), b"1\n").unwrap();
    std::fs::write(d.0.join(".starter-seeded"), b"1\n").unwrap();
    let disk_path = crate::home::library_file_disk_path(&l, &p.id, &entry.id);
    std::fs::remove_file(&disk_path).unwrap();
    let s = Sim::with(Options { home: true, library: Some(d.0.clone()), ..Default::default() });
    (s, d)
}

/// The control drawn over `rect` is a secondary-destructive button at rest: its fill, its 1 px error border and its
/// label, read from the shapes the frame painted.
fn assert_paints_secondary_destructive(s: &Sim, rect: Rect, label: &str) {
    use crate::widgets::kit::{self, Hierarchy, State};
    let rest = kit::button_look(Hierarchy::SecondaryDestructive, State::Default);
    assert_eq!(rest.border, theme::BORDER_ERROR_SUBTLE, "the kit's error border");
    let (rects, texts) = painted_home(s);
    assert!(rects.iter().any(|r| r.rect == rect && r.fill == rest.fill), "{label}: fill {:?} over {rect:?}", rest.fill);
    assert!(
        rects.iter().any(|r| r.rect == rect && r.stroke.width == 1.0 && r.stroke.color == theme::BORDER_ERROR_SUBTLE),
        "{label}: no 1 px error border over {rect:?}"
    );
    assert!(texts.iter().any(|(t, c)| t == label && *c == theme::TEXT_ERROR), "{label}: label in the error text colour");
}

#[test]
fn home_empty_trash_is_destructive_secondary() {
    let (mut s, _d) = home_sim();
    let id = file_id(&s, "DESIGN-OS Studio Starter");
    crate::home::apply(&mut s.shell.app, Act::Trash(id));
    s.frame(vec![]);
    click_rect(&mut s, "home-trash");
    s.frame(vec![]);
    let btn = s.rect_of("home-empty-trash");
    assert_paints_secondary_destructive(&s, btn, "Empty trash");
}

#[test]
fn home_missing_file_uses_error_roles() {
    let (mut s, _d) = missing_file_sim();
    let (_rects, texts) = painted_home(&s);
    let missing = texts.iter().find(|(t, _)| t.contains("File not found")).expect("missing copy");
    assert_eq!(missing.1, theme::TEXT_ERROR);
    let remove = s.rect_of("home-remove-Missing draft");
    assert!(near(remove.height(), theme::CONTROL_SM), "remove height {}", remove.height());
    assert_paints_secondary_destructive(&s, remove, "Remove");

    click_rect(&mut s, "home-view-list");
    s.frame(vec![]);
    let remove = s.rect_of("home-remove-Missing draft");
    assert_paints_secondary_destructive(&s, remove, "Remove");
}

#[test]
fn home_has_no_figma_blue_or_ui3_palette() {
    let src = home_sources("mod");
    assert!(!src.contains("UI3_"));
    assert!(!src.contains("PrimaryBlue"));
    assert!(!src.contains("0x0D, 0x99, 0xFF"));
}

#[test]
fn home_primary_button_is_brand_with_dark_label() {
    let (s, _d) = home_sim();
    let (rects, texts) = painted_home(&s);
    let button = s.rect_of("home-new-file");
    assert!(rects.iter().any(|r| r.rect == button && r.fill == theme::BG_BRAND), "+ Design file is painted on the brand fill");
    assert!(texts.iter().any(|(t, c)| t == "+ Design file" && *c == theme::TEXT_ON_BRAND), "+ Design file label is the dark brand label");
    let brand: Vec<Rect> = rects.iter().filter(|r| r.fill == theme::BG_BRAND && r.rect.left() >= theme::HOME_SIDEBAR_W).map(|r| r.rect).collect();
    assert_eq!(brand, [button], "no other Home control takes the brand fill");
    // the secondary header buttons stay on the page colour
    let import = s.rect_of("home-import");
    assert!(rects.iter().any(|r| r.rect == import && r.fill == theme::BG), "Import is a secondary button");
}

#[test]
fn home_filter_menu_rows_follow_the_home_pitch() {
    let (mut s, _d) = home_sim();
    click_rect(&mut s, "home-filter");
    s.frame(vec![]);
    let first = s.rect_of("menu-row-0-0");
    let second = s.rect_of("menu-row-0-1");
    assert!(near(first.height(), theme::CONTROL_SM), "first row {} want {}", first.height(), theme::CONTROL_SM);
    assert!(near(second.top() - first.top(), theme::CONTROL_SM), "row pitch {} want {}", second.top() - first.top(), theme::CONTROL_SM);
}

#[test]
fn home_layout_regions_unchanged() {
    let (s, _d) = home_sim();
    let card = s.rect_of("home-card-DESIGN-OS Studio Starter");
    assert!(near(card.left(), theme::HOME_SIDEBAR_W + theme::HOME_PAD), "card x {}", card.left());
    assert!(near(card.width(), theme::HOME_CARD_W), "card w {}", card.width());
    assert_eq!(theme::HOME_HEADER_H, 64.0);
    assert_eq!(theme::HOME_SIDEBAR_W, 264.0);
    let side_top = s.rect_of("home-workspace-header").top() - 10.0;
    let filter = s.rect_of("home-filter");
    assert!(near(filter.top(), side_top + theme::HOME_HEADER_H + theme::SPACE_FACT), "header stays 64");
}

/// Ten pixels a character and sixteen for "…", so a test can add widths up by hand.
fn crumb_width(text: &str) -> f32 {
    text.chars().map(|c| if c == '…' { 16.0 } else { 10.0 }).sum()
}

fn crumb_texts<'a>(shown: &'a [crate::home::Crumb<'a>]) -> Vec<&'a str> {
    shown.iter().map(|c| &*c.text).collect()
}

#[test]
fn fit_breadcrumb_shortened_last_segment_fits_its_budget() {
    // 10 characters at 10 px each, 55 px available, ellipsis 16 px.
    let out = crate::home::fit_breadcrumb(&["abcdefghij"], 24.0, 55.0, crumb_width);
    let w = crumb_width(&out[0].text);
    assert!(w <= 55.0, "{:?} is {w} px, budget 55", crumb_texts(&out));
    assert_eq!(out[0].segment, Some(0));
}

#[test]
fn fit_breadcrumb_drops_leading_segments_behind_an_ellipsis_before_it_shortens() {
    let labels = ["Brand", "Marketing", "Logos"];
    // 50 + 24 + 90 + 24 + 50 = 238 in full.
    let all = crate::home::fit_breadcrumb(&labels, 24.0, 238.0, crumb_width);
    assert_eq!(crumb_texts(&all), ["Brand", "Marketing", "Logos"]);
    assert_eq!(all.iter().map(|c| c.segment).collect::<Vec<_>>(), [Some(0), Some(1), Some(2)]);
    // "…" 16 + 24 + 90 + 24 + 50 = 204: the project goes first.
    let lead = crate::home::fit_breadcrumb(&labels, 24.0, 204.0, crumb_width);
    assert_eq!(crumb_texts(&lead), ["…", "Marketing", "Logos"]);
    assert_eq!(lead.iter().map(|c| c.segment).collect::<Vec<_>>(), [None, Some(1), Some(2)]);
    // "…" 16 + 24 + 50 = 90 keeps the last segment whole.
    let last = crate::home::fit_breadcrumb(&labels, 24.0, 90.0, crumb_width);
    assert_eq!(crumb_texts(&last), ["…", "Logos"]);
    // 80 px: the last segment is cut to what the leading "…" leaves, "Lo…" = 36 px.
    let cut = crate::home::fit_breadcrumb(&labels, 24.0, 80.0, crumb_width);
    assert_eq!(crumb_texts(&cut), ["…", "Lo…"]);
    assert_eq!(cut[1].segment, Some(2));
}

/// One frame at a window size other than the Sim's fixed 1600 x 1088. The rects it records stay readable until the
/// next `Sim` frame, which draws at the fixed size again.
fn frame_at(s: &mut Sim, size: Vec2) {
    s.t += 0.05;
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), time: Some(s.t), ..Default::default() };
    let ctx = s.ctx.clone();
    let shell = &mut s.shell;
    let out = ctx.run_ui(raw, |ui| shell.draw(ui));
    s.shapes = out.shapes;
}

#[test]
fn home_breadcrumb_clears_header_buttons_at_min_window() {
    let (mut s, _d) = home_sim();
    click_rect(&mut s, "home-project-Getting started");
    let size = Vec2::new(theme::MIN_WINDOW.0, theme::MIN_WINDOW.1);
    frame_at(&mut s, size);
    frame_at(&mut s, size);
    let crumb = s.rect_of("home-crumb-0");
    let button = s.rect_of("home-new-folder");
    assert!(crumb.right() + theme::SPACE_GROUP <= button.left() + 0.5, "crumb right {} button left {}", crumb.right(), button.left());
}

#[test]
fn a_crumb_opens_its_own_segment_when_a_project_and_its_folders_share_a_name() {
    let (mut s, _d) = home_sim();
    let l = lib(&s);
    let project = l.create_project("Brand").unwrap();
    let outer = l.create_folder(&project.id, None, "Brand").unwrap();
    let inner = l.create_folder(&project.id, Some(&outer.id), "Logos").unwrap();
    s.shell.app.home.as_mut().unwrap().dirty = true;
    s.frame(vec![]);
    s.frame(vec![]);
    crate::home::apply(&mut s.shell.app, Act::Go(Section::Project(project.id.clone(), Some(inner.id.clone()))));
    s.frame(vec![]);
    s.frame(vec![]);
    // Brand / Brand / Logos: the middle crumb is the folder, not the project that has the same name.
    click_rect(&mut s, "home-crumb-1");
    s.frame(vec![]);
    assert_eq!(s.app().home.as_ref().unwrap().section, Section::Project(project.id.clone(), Some(outer.id.clone())));
    // The project crumb still opens the project root.
    click_rect(&mut s, "home-crumb-0");
    s.frame(vec![]);
    assert_eq!(s.app().home.as_ref().unwrap().section, Section::Project(project.id, None));
}

#[test]
fn template_card_icon_tile_and_category_pill_keep_an_edge_on_the_banner() {
    let (mut s, _d) = home_sim();
    s.shell.app.home.as_mut().unwrap().section = Section::Templates;
    s.frame(vec![]);
    s.frame(vec![]);
    let card = s.rect_of("home-template-design");
    let (rects, _) = painted_home(&s);
    let edge = |r: &egui::epaint::RectShape| r.stroke.width == 1.0 && r.stroke.color == theme::BORDER;
    let tile = Rect::from_min_size(card.min + Vec2::new(12.0, 14.0), Vec2::splat(28.0));
    assert!(rects.iter().any(|r| r.rect == tile && edge(r)), "the icon tile has no 1 px BORDER stroke over {tile:?}");
    let pill =
        rects.iter().any(|r| near(r.rect.top(), card.top() + 16.0) && near(r.rect.height(), 20.0) && near(r.rect.right(), card.right() - 12.0) && edge(r));
    assert!(pill, "the category pill has no 1 px BORDER stroke");
}

#[test]
fn home_trash_row_clears_the_divider_above_it() {
    let (s, _d) = home_sim();
    let trash = s.rect_of("home-trash");
    let (rects, _) = painted_home(&s);
    let divider = rects
        .iter()
        .filter(|r| r.fill == theme::BORDER && near(r.rect.height(), theme::BORDER_WIDTH) && r.rect.width() > 200.0 && r.rect.bottom() <= trash.top() + 1.0)
        .map(|r| r.rect)
        .max_by(|a, b| a.top().total_cmp(&b.top()))
        .expect("the hairline above the Trash row");
    let gap = trash.top() - divider.bottom();
    assert!(gap >= theme::SPACE_INLINE, "the Trash row starts {gap} px under its divider");
}

#[test]
fn home_workspace_bell_is_centred_on_the_workspace_row() {
    let (s, _d) = home_sim();
    let bell = s.rect_of("home-notifications");
    let header = s.rect_of("home-workspace-header");
    assert!(near(bell.center().y, header.center().y), "bell centre {} header centre {}", bell.center().y, header.center().y);
}

#[test]
fn board_badge_is_a_bordered_gray_badge() {
    use crate::widgets::kit::{count_badge_look, Tone};
    let d = TmpDir::new();
    let l = Library::open(&d.0).unwrap();
    let p = l.create_project("Getting started").unwrap();
    let mut doc = Document::new("Sticky wall");
    doc.kind = designos_doc::DocumentKind::Board;
    doc.add_page("Page 1");
    l.create_file(&p.id, None, "Sticky wall", &doc).unwrap();
    std::fs::write(d.0.join(".seeded"), b"1\n").unwrap();
    std::fs::write(d.0.join(".starter-seeded"), b"1\n").unwrap();
    let mut s = Sim::with(Options { home: true, library: Some(d.0.clone()), ..Default::default() });
    for _ in 0..4 {
        s.frame(vec![]);
    }
    let card = s.rect_of("home-card-Sticky wall");
    let look = count_badge_look(Tone::Neutral);
    let (rects, texts) = painted_home(&s);
    let top = card.top() + theme::HOME_THUMB_H - theme::SPACE_INLINE - theme::BADGE_SM;
    let badge = rects
        .iter()
        .find(|r| {
            near(r.rect.left(), card.left() + theme::SPACE_INLINE) && near(r.rect.top(), top) && near(r.rect.height(), theme::BADGE_SM) && r.fill == look.fill
        })
        .expect("the Board badge fill: gray, 22 px tall");
    assert!(rects.iter().any(|r| r.rect == badge.rect && r.stroke.width == 1.0 && r.stroke.color == look.border), "the Board badge has the gray badge border");
    assert!(texts.iter().any(|(t, c)| t == "Board" && *c == look.text), "the Board label is the gray badge text");
}

/// The kit's focus band: a 2 px `FOCUS_RING` stroke 4 px outside the control.
fn focus_ring_around(s: &Sim, control: Rect) -> bool {
    let outer = control.expand(theme::SPACE_FACT);
    let (rects, _) = painted_home(s);
    rects.iter().any(|r| {
        r.stroke.color == theme::FOCUS_RING
            && r.stroke.width == 2.0
            && near(r.rect.min.x, outer.min.x)
            && near(r.rect.min.y, outer.min.y)
            && near(r.rect.max.x, outer.max.x)
            && near(r.rect.max.y, outer.max.y)
    })
}

/// Gives `id` the keyboard focus and draws it: the band appears around `control`, and not before.
fn assert_focus_ring_follows(s: &mut Sim, id: egui::Id, control: Rect, what: &str) {
    assert!(!focus_ring_around(s, control), "{what}: a focus ring before the control has the focus");
    s.ctx.memory_mut(|m| m.request_focus(id));
    s.frame(vec![]);
    s.frame(vec![]);
    assert!(focus_ring_around(s, control), "{what}: no focus ring around {control:?}");
}

#[test]
fn home_secondary_controls_draw_the_focus_ring() {
    let (mut s, _d) = home_sim();
    for (key, name) in [("home-filter", "filter"), ("home-sort", "sort")] {
        let control = s.rect_of(key);
        assert_focus_ring_follows(&mut s, egui::Id::new(key), control, name);
    }
    let home = |s: &mut Sim, section| {
        s.shell.app.home.as_mut().unwrap().section = section;
        s.frame(vec![]);
        s.frame(vec![]);
    };
    home(&mut s, Section::Templates);
    let card = s.rect_of("home-template-design");
    let cta = Rect::from_min_size(
        Pos2::new(card.left() + theme::SPACE_GROUP, card.bottom() - theme::SPACE_GROUP - theme::CONTROL_SM),
        Vec2::new(card.width() - theme::SPACE_GROUP * 2.0, theme::CONTROL_SM),
    );
    assert_focus_ring_follows(&mut s, egui::Id::new(("home-tpl-card", "home-template-design")), cta, "template card button");
    home(&mut s, Section::Resources);
    let hub = s.rect_of("home-resource-libraries");
    assert_focus_ring_follows(&mut s, egui::Id::new(("home-res-b1", "home-resource-libraries")), hub, "hub card button");
    home(&mut s, Section::AllProjects);
    let project = s.app().home.as_ref().unwrap().snap.projects[0].clone();
    let folder = s.rect_of(&format!("home-project-new-folder-{}", project.name));
    assert_focus_ring_follows(&mut s, egui::Id::new(("home-proj-new-folder", &project.id)), folder, "+ Folder");
}
