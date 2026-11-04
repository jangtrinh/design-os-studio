//! The Project tab, driven through the whole window: opening a project, the doc tree, rename, delete, reorder, the
//! reading column and its contents, and a page changed from outside.

use super::unsaved_dot::TmpDir;
use super::*;
use crate::home::Act;
use crate::tabs::project_tab::fixture::{self, CART_RULES, ERROR_HANDLING, OVERVIEW, PAYMENT_FLOW, PAYMENT_METHODS};
use crate::tabs::project_tab::{layout, PageView, ProjectTabState};
use crate::tabs::ProjectTabKind;
use designos_doc::Document;
use designos_docs::{DocsStore, Group, Manifest};
use std::time::{Duration, Instant};

fn checkout_sim() -> (Sim, TmpDir, String, String) {
    let dir = TmpDir::new();
    let mut s = Sim::with(Options { home: true, library: Some(dir.0.join("lib")), ..Default::default() });
    s.shell.app.tabs.always_visible = true;
    let lib = s.app().lib.clone().expect("library");
    let project = lib.create_project("Checkout").unwrap();
    let file = lib.create_file(&project.id, None, "Checkout-web", &Document::new("Checkout-web")).unwrap();
    let store = DocsStore::for_project(lib.root(), &project.id).unwrap();
    fixture::write_checkout_docs(&store, "Checkout", &file.id);
    (s, dir, project.id, file.id)
}

fn docs_of(s: &Sim, project: &str) -> DocsStore {
    DocsStore::for_project(s.app().lib.as_ref().unwrap().root(), project).unwrap()
}

/// Frames until `done`, letting the docs worker (a real thread) and the file watcher run in between.
fn settle(s: &mut Sim, mut done: impl FnMut(&mut Sim) -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(8);
    while Instant::now() < end {
        s.frame(vec![]);
        if done(s) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    false
}

fn state(s: &Sim) -> &ProjectTabState {
    let tabs = &s.app().tabs;
    &tabs.project_states[&tabs.entries[tabs.active_idx].id]
}

fn page_rel(s: &Sim) -> Option<String> {
    match &state(s).page {
        PageView::Loaded(p) => Some(p.rel.clone()),
        _ => None,
    }
}

fn open_project(s: &mut Sim, id: &str) {
    crate::home::apply(&mut s.shell.app, Act::OpenProject(id.to_string()));
    assert!(settle(s, |s| page_rel(s).is_some()), "the first page of the project loads");
}

fn open_page(s: &mut Sim, rel: &str) {
    let at = s.rect_of(&format!("project-tree-page-{rel}")).center();
    s.click(at);
    assert!(settle(s, |s| page_rel(s).as_deref() == Some(rel)), "{rel} opens");
}

fn manifest(s: &Sim, project: &str) -> Manifest {
    docs_of(s, project).read_manifest().unwrap().value
}

#[test]
fn opening_a_project_gives_a_project_tab_and_opening_it_again_focuses_that_tab() {
    let (mut s, _dir, pid, _file) = checkout_sim();
    open_project(&mut s, &pid);
    let tabs = &s.app().tabs;
    assert_eq!(tabs.entries[tabs.active_idx].kind, ProjectTabKind::Project);
    assert_eq!(tabs.entries[tabs.active_idx].title, "Checkout \u{b7} Project");
    assert!(s.app().home.is_none());
    let count = tabs.entries.len();
    let mut tabs = std::mem::take(&mut s.shell.app.tabs);
    tabs.enter_home(&mut s.shell.app);
    s.shell.app.tabs = tabs;
    s.frame(vec![]);
    assert!(s.app().home.is_some());
    crate::home::apply(&mut s.shell.app, Act::OpenProject(pid.clone()));
    s.frame(vec![]);
    s.frame(vec![]);
    assert_eq!(s.app().tabs.entries.len(), count, "the open Project tab is focused, not duplicated");
    assert!(s.app().tabs.active_is_project());
    assert!(s.app().home.is_none());
}

#[test]
fn the_project_context_menu_and_the_project_header_offer_open_project() {
    let (mut s, _dir, pid, _file) = checkout_sim();
    let h = s.app().home.as_ref().unwrap();
    let entries = crate::home::context_entries(h, &crate::home::Target::Project(pid.clone()));
    assert_eq!(entries[0].0.label, "Open project");
    assert_eq!(entries[0].1, Act::OpenProject(pid.clone()));
    let labels: Vec<&str> = entries.iter().map(|e| e.0.label.as_str()).collect();
    assert_eq!(labels, vec!["Open project", "New folder", "Rename", "Move to trash"]);
    assert!(entries[1].0.separator_after, "a separator sits between New folder and Rename");
    // Inside the project the header's button group starts with Open project.
    crate::home::apply(&mut s.shell.app, Act::Go(crate::home::Section::Project(pid.clone(), None)));
    s.shell.app.home.as_mut().unwrap().dirty = true;
    s.frame(vec![]);
    s.frame(vec![]);
    let button = s.rect_of("home-open-project");
    let new_folder = s.rect_of("home-new-folder");
    assert!(button.right() < new_folder.left(), "Open project is the leftmost button");
    s.click(button.center());
    assert!(settle(&mut s, |s| s.app().tabs.active_is_project()), "the header button opens the Project tab");
}

#[test]
fn the_tree_lists_the_five_groups_in_fixed_order_with_their_pages() {
    let (mut s, _dir, pid, _file) = checkout_sim();
    open_project(&mut s, &pid);
    let ys: Vec<f32> = ["Overview", "Spec", "Screens", "States", "Discovery"].iter().map(|g| s.rect_of(&format!("project-tree-group-{g}")).top()).collect();
    assert!(ys.windows(2).all(|w| w[0] < w[1]), "groups run top to bottom in order: {ys:?}");
    let spec = s.rect_of(&format!("project-tree-page-{PAYMENT_METHODS}"));
    let cart = s.rect_of(&format!("project-tree-page-{CART_RULES}"));
    assert!(spec.top() < cart.top() && spec.top() > ys[1] && cart.top() < ys[2], "Spec pages sit under Spec, in manifest order");
    assert_eq!(spec.height(), 32.0, "tree rows are 32 tall");
    assert_eq!(state(&s).selected.as_deref(), Some(OVERVIEW), "the Overview page opens first");
}

#[test]
fn the_regions_are_where_the_layout_puts_them() {
    let (mut s, _dir, pid, _file) = checkout_sim();
    open_project(&mut s, &pid);
    let bar = s.app().ui.regions.tab_bar.expect("tab bar");
    let full = Rect::from_min_max(Pos2::new(0.0, bar.bottom()), Pos2::new(1600.0, 1088.0));
    let expected = layout::compute(full);
    assert_eq!(s.rect_of("project-tree"), expected.tree);
    assert_eq!(s.rect_of("project-column"), expected.column);
    assert_eq!(s.rect_of("project-contents"), expected.contents.expect("contents at 1600 px"));
    let tree = s.rect_of("project-tree");
    assert_eq!((tree.left(), tree.width()), (48.0, 240.0));
}

#[test]
fn renaming_a_page_from_the_tree_writes_the_new_file_and_the_manifest() {
    let (mut s, _dir, pid, _file) = checkout_sim();
    open_project(&mut s, &pid);
    open_page(&mut s, CART_RULES);
    s.key(Key::F2);
    assert!(state(&s).tree.rename.is_some(), "F2 starts renaming the open page");
    s.cmd_key(Key::A);
    s.type_text("Basket rules");
    s.key(Key::Enter);
    let docs = docs_of(&s, &pid);
    assert!(
        settle(&mut s, |_| docs.dir().join("spec/basket-rules.md").exists() && !docs.dir().join(CART_RULES).exists()),
        "the file is renamed and the old one is gone"
    );
    let renamed = docs.read_page("spec/basket-rules.md").unwrap().value;
    assert_eq!(renamed.front_matter().unwrap().title(), Some("Basket rules"));
    let spec = manifest(&s, &pid).group(Group::Spec).pages.clone();
    assert_eq!(spec, vec![PAYMENT_METHODS, PAYMENT_FLOW, "spec/basket-rules.md", ERROR_HANDLING]);
    assert!(
        settle(&mut s, |s| state(s).selected.as_deref() == Some("spec/basket-rules.md") && page_rel(s).as_deref() == Some("spec/basket-rules.md")),
        "the open page follows the rename"
    );
    // The link in the Payment methods page that pointed at Cart rules follows too.
    let methods = std::fs::read_to_string(docs.dir().join(PAYMENT_METHODS)).unwrap();
    assert!(methods.contains("[Cart rules](basket-rules.md)"), "{methods}");
}

#[test]
fn delete_page_asks_first_and_moves_the_page_to_the_library_trash() {
    let (mut s, _dir, pid, _file) = checkout_sim();
    open_project(&mut s, &pid);
    let docs = docs_of(&s, &pid);
    let at = s.rect_of(&format!("project-tree-page-{ERROR_HANDLING}")).center();
    s.right_click(at);
    s.click(s.rect_of("project-menu-Delete").center());
    let cancel = s.rect_of("project-delete-cancel");
    assert!(s.rect_of("project-delete-confirm").left() > cancel.right(), "Delete page is the right-hand button");
    // Enter answers the question the safe way: Cancel has the focus.
    s.key(Key::Enter);
    s.frame(vec![]);
    assert!(docs.dir().join(ERROR_HANDLING).exists(), "Enter cancelled the delete");
    // Open it again and confirm.
    assert!(state(&s).delete.is_none());
    s.right_click(at);
    s.click(s.rect_of("project-menu-Delete").center());
    assert!(state(&s).delete.is_some());
    s.click(s.rect_of("project-delete-confirm").center());
    assert!(
        settle(&mut s, |s| !docs.dir().join(ERROR_HANDLING).exists() && manifest(s, &pid).group(Group::Spec).pages.len() == 3),
        "the page file leaves the docs folder and the manifest"
    );
    let items = s.app().lib.as_ref().unwrap().list_trash();
    assert_eq!(items.len(), 1, "one trash item");
    assert_eq!((items[0].kind.clone(), items[0].name.as_str()), (designos_library::TrashKind::Page, "Error handling"));
    let spec = manifest(&s, &pid).group(Group::Spec).pages.clone();
    assert_eq!(spec, vec![PAYMENT_METHODS, PAYMENT_FLOW, CART_RULES]);
    assert!(settle(&mut s, |s| !state(s).snapshot.pages.contains_key(ERROR_HANDLING)), "the tree drops the page");
}

#[test]
fn dragging_a_page_inside_its_group_persists_the_new_order() {
    let (mut s, _dir, pid, _file) = checkout_sim();
    open_project(&mut s, &pid);
    let from = s.rect_of(&format!("project-tree-page-{CART_RULES}")).center();
    let first = s.rect_of(&format!("project-tree-page-{PAYMENT_METHODS}"));
    s.drag(from, Pos2::new(first.center().x, first.top() + 2.0));
    assert!(
        settle(&mut s, |s| manifest(s, &pid).group(Group::Spec).pages.first().map(String::as_str) == Some(CART_RULES)),
        "the manifest holds the dragged order"
    );
    assert_eq!(manifest(&s, &pid).group(Group::Spec).pages, vec![CART_RULES, PAYMENT_METHODS, PAYMENT_FLOW, ERROR_HANDLING]);
}

#[test]
fn dropping_a_page_in_another_group_changes_nothing() {
    let (mut s, _dir, pid, _file) = checkout_sim();
    open_project(&mut s, &pid);
    let from = s.rect_of(&format!("project-tree-page-{CART_RULES}")).center();
    let other = s.rect_of("project-tree-page-discovery/checkout-research.md").center();
    s.drag(from, other);
    for _ in 0..10 {
        s.frame(vec![]);
        std::thread::sleep(Duration::from_millis(15));
    }
    let m = manifest(&s, &pid);
    assert_eq!(m.group(Group::Spec).pages, vec![PAYMENT_METHODS, PAYMENT_FLOW, CART_RULES, ERROR_HANDLING]);
    assert_eq!(m.group(Group::Discovery).pages.len(), 2);
}

#[test]
fn a_project_without_docs_gets_its_overview_page_as_a_real_file() {
    let dir = TmpDir::new();
    let mut s = Sim::with(Options { home: true, library: Some(dir.0.join("lib")), ..Default::default() });
    s.shell.app.tabs.always_visible = true;
    let lib = s.app().lib.clone().unwrap();
    let fresh = lib.create_project("Fresh start").unwrap();
    let docs = DocsStore::for_project(lib.root(), &fresh.id).unwrap();
    assert!(docs.list_page_files().unwrap().is_empty());
    open_project(&mut s, &fresh.id);
    assert_eq!(state(&s).selected.as_deref(), Some("overview.md"));
    let page = docs.read_page("overview.md").unwrap().value;
    assert_eq!(page.front_matter().unwrap().title(), Some("Fresh start"));
    assert_eq!(page.front_matter().unwrap().description(), Some("What this project is and where to start."));
    assert_eq!(docs.read_manifest().unwrap().value.group(Group::Overview).pages, vec!["overview.md".to_string()]);
}

fn long_page(extra: &str) -> String {
    let mut text = String::from("---\ntitle: Long page\n---\n");
    for n in 1..=6 {
        text.push_str(&format!("\n## Section {n}\n\n"));
        for p in 0..8 {
            text.push_str(&format!("Paragraph {p} of section {n} with enough words to take a line or two when the column is 690 wide.\n\n"));
        }
    }
    text.push_str(extra);
    text
}

#[test]
fn the_contents_follow_the_page_and_a_click_scrolls_to_the_heading() {
    let (mut s, _dir, pid, _file) = checkout_sim();
    let docs = docs_of(&s, &pid);
    std::fs::write(docs.dir().join("spec/long.md"), long_page("")).unwrap();
    open_project(&mut s, &pid);
    assert!(settle(&mut s, |s| s.ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new(("test-rect", "project-tree-page-spec/long.md".to_string())))).is_some()));
    open_page(&mut s, "spec/long.md");
    assert!(settle(&mut s, |s| state(s).reader.headings().len() == 6), "six H2 headings in the contents");
    assert_eq!(state(&s).reader.active_heading(), Some(0), "the first heading is active at the top");
    let third = s.rect_of("project-contents-2");
    s.click(third.center());
    for _ in 0..12 {
        s.frame(vec![]);
    }
    assert!(state(&s).reader.scroll > 100.0, "the page scrolled: {}", state(&s).reader.scroll);
    assert_eq!(state(&s).reader.active_heading(), Some(2), "the clicked heading is the active one");
}

#[test]
fn an_outside_edit_refreshes_the_open_page_and_keeps_the_scroll_position() {
    let (mut s, _dir, pid, _file) = checkout_sim();
    let docs = docs_of(&s, &pid);
    std::fs::write(docs.dir().join("spec/long.md"), long_page("")).unwrap();
    open_project(&mut s, &pid);
    assert!(settle(&mut s, |s| s.ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new(("test-rect", "project-tree-page-spec/long.md".to_string())))).is_some()));
    open_page(&mut s, "spec/long.md");
    assert!(settle(&mut s, |s| state(s).reader.headings().len() == 6));
    let column = s.rect_of("project-column");
    s.mv(column.center());
    s.frame(vec![Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: Vec2::new(0.0, -300.0),
        phase: egui::TouchPhase::Move,
        modifiers: Modifiers::NONE,
    }]);
    for _ in 0..3 {
        s.frame(vec![]);
    }
    // Let the 200 ms scroll animation finish.
    for _ in 0..10 {
        s.frame(vec![]);
    }
    let before = state(&s).reader.scroll;
    assert!(before > 100.0, "scrolled down: {before}");
    let rev_before = match &state(&s).page {
        PageView::Loaded(p) => p.rev,
        _ => panic!("page not loaded"),
    };
    // Another program appends a section.
    std::fs::write(docs.dir().join("spec/long.md"), long_page("\n## Added from outside\n\nNew text.\n")).unwrap();
    assert!(
        settle(&mut s, |s| matches!(&state(s).page, PageView::Loaded(p) if p.rev > rev_before && p.page.headings().len() == 7)),
        "the changed file shows without reopening"
    );
    let after = state(&s).reader.scroll;
    assert!((after - before).abs() < 1.0, "scroll stayed at {before}, now {after}");
    assert!(settle(&mut s, |s| state(s).reader.headings().len() == 7), "contents list the new heading");
}

#[test]
fn a_page_over_8_mb_says_it_is_not_watched() {
    let (mut s, _dir, pid, _file) = checkout_sim();
    let docs = docs_of(&s, &pid);
    std::fs::write(docs.dir().join("spec/huge.md"), vec![b'a'; 9 * 1024 * 1024]).unwrap();
    open_project(&mut s, &pid);
    assert!(settle(&mut s, |s| s.ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new(("test-rect", "project-tree-page-spec/huge.md".to_string())))).is_some()));
    let at = s.rect_of("project-tree-page-spec/huge.md").center();
    s.click(at);
    assert!(settle(&mut s, |s| matches!(&state(s).page, PageView::Failed { .. })));
    match &state(&s).page {
        PageView::Failed { message, .. } => assert_eq!(message, "This page is larger than 8 MB and is not watched for outside changes."),
        _ => unreachable!(),
    }
}

/// Writes the Checkout library the capture sheets use: `DESIGNOS_PROJECT_FIXTURE_OUT=<dir>`; prints the ids.
#[test]
#[ignore = "writes a library for the screenshot sheets"]
fn write_the_checkout_fixture_library() {
    let out = std::env::var("DESIGNOS_PROJECT_FIXTURE_OUT").expect("DESIGNOS_PROJECT_FIXTURE_OUT");
    let lib = designos_library::Library::open(&out).unwrap();
    let project = lib.create_project("Checkout").unwrap();
    let web = lib.create_file(&project.id, None, "Checkout-web", &Document::new("Checkout-web")).unwrap();
    lib.create_file(&project.id, None, "Checkout-mobile", &Document::new("Checkout-mobile")).unwrap();
    let store = DocsStore::for_project(lib.root(), &project.id).unwrap();
    // `DESIGNOS_PROJECT_FIXTURE_EMPTY=1` leaves the docs folder out: the project a user opens for the first time.
    if std::env::var("DESIGNOS_PROJECT_FIXTURE_EMPTY").is_err() {
        fixture::write_checkout_docs(&store, "Checkout", &web.id);
    }
    // `DESIGNOS_PROJECT_FIXTURE_TRASH=<page>` deletes a page like the Project tab does; with
    // `DESIGNOS_PROJECT_FIXTURE_RESTORE=1` it is then restored through the library, like Home's Restore button.
    if let Ok(rel) = std::env::var("DESIGNOS_PROJECT_FIXTURE_TRASH") {
        let item = lib.trash_page(&project.id, &rel, "Error handling").unwrap();
        if std::env::var("DESIGNOS_PROJECT_FIXTURE_RESTORE").is_ok() {
            lib.restore(&item.id).unwrap();
        }
    }
    println!("PROJECT_ID={}\nWEB_FILE_ID={}", project.id, web.id);
}

/// Opens Home on its Trash section (the Project tab stays open behind it).
fn open_home_trash(s: &mut Sim) {
    let mut tabs = std::mem::take(&mut s.shell.app.tabs);
    tabs.enter_home(&mut s.shell.app);
    s.shell.app.tabs = tabs;
    crate::home::apply(&mut s.shell.app, Act::Go(crate::home::Section::Trash));
    s.shell.app.home.as_mut().unwrap().dirty = true;
    s.frame(vec![]);
    s.frame(vec![]);
}

#[test]
fn a_deleted_page_shows_in_homes_trash_is_counted_and_restore_puts_it_back() {
    let (mut s, _dir, pid, _file) = checkout_sim();
    open_project(&mut s, &pid);
    let docs = docs_of(&s, &pid);
    let at = s.rect_of(&format!("project-tree-page-{ERROR_HANDLING}")).center();
    s.right_click(at);
    s.click(s.rect_of("project-menu-Delete").center());
    s.click(s.rect_of("project-delete-confirm").center());
    assert!(settle(&mut s, |s| !docs.dir().join(ERROR_HANDLING).exists() && manifest(s, &pid).group(Group::Spec).pages.len() == 3));
    open_home_trash(&mut s);
    let items = s.app().home.as_ref().unwrap().snap.trash.clone();
    assert_eq!(items.len(), 1, "Home lists the page: {items:?}");
    assert_eq!((items[0].kind.clone(), items[0].name.as_str()), (designos_library::TrashKind::Page, "Error handling"));
    // "Empty trash" counts it: the confirmation is asked about one item.
    crate::home::apply(&mut s.shell.app, Act::EmptyTrash);
    assert!(
        s.app().dialog.as_ref().is_some_and(|d| d.lines.iter().any(|(l, _)| l.contains("1 item will be permanently deleted"))),
        "the confirmation counts the page"
    );
    s.shell.app.dialog = None;
    crate::home::apply(&mut s.shell.app, Act::Restore(items[0].id.clone()));
    assert!(docs.dir().join(ERROR_HANDLING).exists(), "the file is back");
    assert_eq!(manifest(&s, &pid).group(Group::Spec).pages.last().map(String::as_str), Some(ERROR_HANDLING), "listed in Spec again");
}

#[test]
fn document_save_and_export_shortcuts_do_nothing_on_a_project_tab() {
    let (mut s, _dir, pid, _file) = checkout_sim();
    open_project(&mut s, &pid);
    let before = (s.app().file_path.clone(), s.app().dialog.is_some());
    for (key, shift) in [(Key::S, false), (Key::S, true)] {
        s.mods = Modifiers { shift, ..Modifiers::NONE };
        s.cmd_key(key);
        s.mods = Modifiers::NONE;
        for _ in 0..3 {
            s.frame(vec![]);
        }
        assert!(s.app().toast.is_none(), "no save-as toast for the tab's placeholder document: {:?}", s.app().toast.as_ref().map(|t| t.text.clone()));
        assert_eq!((s.app().file_path.clone(), s.app().dialog.is_some()), before, "no file dialog and no new file path");
        assert!(s.app().tabs.active_is_project(), "still on the Project tab");
    }
}

#[test]
fn an_idle_project_tab_with_a_page_open_asks_for_no_repaint() {
    let (mut s, _dir, pid, _file) = checkout_sim();
    open_project(&mut s, &pid);
    open_page(&mut s, PAYMENT_METHODS);
    let _ = s.settle();
    s.assert_idle_window_asks_for_no_repaint("a Project tab with a page open");
}

#[test]
fn move_up_and_move_down_from_the_page_menu_write_the_manifest() {
    let (mut s, _dir, pid, _file) = checkout_sim();
    open_project(&mut s, &pid);
    let at = s.rect_of(&format!("project-tree-page-{CART_RULES}")).center();
    s.right_click(at);
    s.click(s.rect_of("project-menu-Move up").center());
    assert!(settle(&mut s, |s| manifest(s, &pid).group(Group::Spec).pages.get(1).map(String::as_str) == Some(CART_RULES)));
    assert_eq!(manifest(&s, &pid).group(Group::Spec).pages, vec![PAYMENT_METHODS, CART_RULES, PAYMENT_FLOW, ERROR_HANDLING]);
    // The rows moved: Cart rules now sits where Payment flow was. Move it down twice from there.
    assert!(settle(&mut s, |s| s.rect_of(&format!("project-tree-page-{CART_RULES}")).top() < s.rect_of(&format!("project-tree-page-{PAYMENT_FLOW}")).top()));
    for _ in 0..2 {
        let before = manifest(&s, &pid).group(Group::Spec).pages.clone();
        let at = s.rect_of(&format!("project-tree-page-{CART_RULES}")).center();
        s.right_click(at);
        s.click(s.rect_of("project-menu-Move down").center());
        assert!(settle(&mut s, |s| manifest(s, &pid).group(Group::Spec).pages != before));
        // The tree shows the new order before the next menu is opened on a row.
        assert!(settle(&mut s, |s| state(s).snapshot.manifest.group(Group::Spec).pages == manifest(s, &pid).group(Group::Spec).pages));
        s.frame(vec![]);
    }
    assert_eq!(manifest(&s, &pid).group(Group::Spec).pages, vec![PAYMENT_METHODS, PAYMENT_FLOW, ERROR_HANDLING, CART_RULES]);
}

#[test]
fn a_project_tab_behind_another_tab_does_not_wake_the_window_for_disk_changes_and_catches_up_when_shown() {
    let (mut s, _dir, pid, _file) = checkout_sim();
    open_project(&mut s, &pid);
    let project_idx = s.app().tabs.active_idx;
    // Another tab comes to the front; the Project tab is parked.
    let mut tabs = std::mem::take(&mut s.shell.app.tabs);
    tabs.open_new_design_tab(&mut s.shell.app);
    s.shell.app.tabs = tabs;
    for _ in 0..6 {
        s.frame(vec![]);
    }
    assert!(!s.app().tabs.active_is_project());
    // The window goes quiet first, so a repaint request after this is a new one.
    let _ = s.settle();
    let wakes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = wakes.clone();
    s.ctx.set_request_repaint_callback(move |_| {
        counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    });
    // An agent writes a page while the user works elsewhere.
    let docs = docs_of(&s, &pid);
    std::fs::write(docs.dir().join("spec/new-from-agent.md"), "---\ntitle: New from agent\n---\n\ntext\n").unwrap();
    std::thread::sleep(Duration::from_millis(2500));
    assert_eq!(wakes.load(std::sync::atomic::Ordering::SeqCst), 0, "a parked Project tab does not ask the window to repaint");
    // Back on the Project tab the new page is there.
    let mut tabs = std::mem::take(&mut s.shell.app.tabs);
    tabs.switch_to(project_idx, &mut s.shell.app);
    s.shell.app.tabs = tabs;
    assert!(
        settle(&mut s, |s| s.app().tabs.active_is_project() && state(s).snapshot.pages.contains_key("spec/new-from-agent.md")),
        "the tree catches up on activation"
    );
}
