use super::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

pub(crate) struct Tmp(pub PathBuf);

impl Tmp {
    pub fn new() -> Self {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!("designos-project-tab-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir_all(&dir).unwrap();
        Tmp(dir)
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn store(tmp: &Tmp) -> DocsStore {
    DocsStore::at(tmp.0.join("docs"))
}

/// Writes `text` as a page and lists it under `group`.
fn add_page(store: &DocsStore, group: Group, rel: &str, text: &str) {
    store.write_page(rel, &Page::parse(text), Expect::Absent).unwrap();
    let rel = rel.to_string();
    update_manifest(store, move |m| m.group_mut(group).pages.push(rel.clone())).unwrap();
}

fn manifest(store: &DocsStore) -> Manifest {
    store.read_manifest().unwrap().value
}

#[test]
fn first_open_writes_the_overview_page_and_the_manifest_as_real_files() {
    let tmp = Tmp::new();
    let store = store(&tmp);
    let made = ensure_first_page(&store, "Checkout: web").unwrap();
    assert_eq!(made.as_deref(), Some("overview.md"));
    let text = std::fs::read_to_string(tmp.0.join("docs/overview.md")).unwrap();
    let page = Page::parse(&text);
    let front = page.front_matter().unwrap();
    assert_eq!(front.title(), Some("Checkout: web"));
    assert_eq!(front.description(), Some(OVERVIEW_DESCRIPTION));
    assert!(front.problems().is_empty());
    assert_eq!(manifest(&store).group(Group::Overview).pages, vec!["overview.md".to_string()]);
    // A second open never writes again, even after the user empties the project of pages.
    assert_eq!(ensure_first_page(&store, "Checkout: web").unwrap(), None);
    std::fs::remove_file(tmp.0.join("docs/overview.md")).unwrap();
    assert_eq!(ensure_first_page(&store, "Checkout: web").unwrap(), None, "a manifest means the user shaped this folder");
}

#[test]
fn a_folder_that_already_has_pages_is_left_alone() {
    let tmp = Tmp::new();
    let store = store(&tmp);
    std::fs::create_dir_all(tmp.0.join("docs")).unwrap();
    std::fs::write(tmp.0.join("docs/notes.md"), "# Notes\n").unwrap();
    assert_eq!(ensure_first_page(&store, "Checkout").unwrap(), None);
    assert!(store.read_manifest().is_err(), "no manifest was written");
}

#[test]
fn pages_no_group_lists_follow_their_front_matter_group_and_never_vanish() {
    let tmp = Tmp::new();
    let store = store(&tmp);
    add_page(&store, Group::Spec, "spec/b.md", "---\ntitle: B\n---\n# B\n");
    add_page(&store, Group::Spec, "spec/a.md", "# A\n");
    store.write_page("loose.md", &Page::parse("---\ntitle: Loose\ngroup: spec\n---\n"), Expect::Absent).unwrap();
    store.write_page("plain.md", &Page::parse("Hello\n"), Expect::Absent).unwrap();
    let snap = Scanner::new(store, None).snapshot().unwrap();
    assert_eq!(snap.group_pages(Group::Spec), vec!["spec/b.md", "spec/a.md", "loose.md"], "listed order first, then the loose page of that group");
    assert_eq!(snap.group_pages(Group::Discovery), vec!["plain.md"]);
    assert_eq!(snap.title_of("spec/a.md"), "A", "the first H1 names a page without a front matter title");
    assert_eq!(snap.title_of("plain.md"), "Plain", "the file name names a page with neither");
}

#[test]
fn a_listed_page_whose_file_is_missing_stays_in_the_tree() {
    let tmp = Tmp::new();
    let store = store(&tmp);
    add_page(&store, Group::Spec, "spec/gone.md", "# Gone\n");
    std::fs::remove_file(tmp.0.join("docs/spec/gone.md")).unwrap();
    let snap = Scanner::new(store, None).snapshot().unwrap();
    assert_eq!(snap.group_pages(Group::Spec), vec!["spec/gone.md"]);
    assert_eq!(snap.title_of("spec/gone.md"), "Gone");
}

#[test]
fn an_unreadable_manifest_is_reported_and_blocks_writes_that_would_replace_it() {
    let tmp = Tmp::new();
    let store = store(&tmp);
    add_page(&store, Group::Spec, "spec/a.md", "# A\n");
    std::fs::write(tmp.0.join("docs/docs-manifest.json"), "{ not json").unwrap();
    let snap = Scanner::new(store.clone(), None).snapshot().unwrap();
    assert!(snap.manifest_error.as_deref().unwrap().contains("docs-manifest.json"), "{:?}", snap.manifest_error);
    assert_eq!(snap.group_pages(Group::Discovery), vec!["spec/a.md"], "pages still show");
    assert!(reorder_pages(&store, Group::Spec, &["spec/a.md".into()]).is_err());
    assert_eq!(std::fs::read_to_string(tmp.0.join("docs/docs-manifest.json")).unwrap(), "{ not json", "the broken file is not overwritten");
}

#[test]
fn renaming_a_page_writes_the_new_file_the_manifest_and_the_links_to_it() {
    let tmp = Tmp::new();
    let store = store(&tmp);
    add_page(&store, Group::Spec, "spec/payment-flow.md", "---\ntitle: Payment flow\ndescription: How it goes.\n---\n## Steps\n");
    add_page(&store, Group::Overview, "overview.md", "See [the flow](spec/payment-flow.md#steps) first.\n");
    let done = rename_page(&store, "spec/payment-flow.md", "  Payment methods ").unwrap();
    assert_eq!(done.new_rel, "spec/payment-methods.md");
    assert_eq!(done.links_updated, 1);
    assert!(done.warnings.is_empty(), "{:?}", done.warnings);
    assert!(!tmp.0.join("docs/spec/payment-flow.md").exists(), "the old file is gone");
    let renamed = store.read_page("spec/payment-methods.md").unwrap().value;
    assert_eq!(renamed.front_matter().unwrap().title(), Some("Payment methods"));
    assert_eq!(renamed.front_matter().unwrap().description(), Some("How it goes."), "other front matter survives");
    assert_eq!(manifest(&store).group(Group::Spec).pages, vec!["spec/payment-methods.md".to_string()]);
    let overview = std::fs::read_to_string(tmp.0.join("docs/overview.md")).unwrap();
    assert_eq!(overview, "See [the flow](spec/payment-methods.md#steps) first.\n");
}

#[test]
fn a_rename_that_collides_with_another_page_takes_the_next_free_name() {
    let tmp = Tmp::new();
    let store = store(&tmp);
    add_page(&store, Group::Spec, "spec/a.md", "# A\n");
    add_page(&store, Group::Spec, "spec/b.md", "# B\n");
    let done = rename_page(&store, "spec/b.md", "A").unwrap();
    assert_eq!(done.new_rel, "spec/a-2.md");
    assert!(tmp.0.join("docs/spec/a.md").exists(), "the other page is untouched");
}

#[test]
fn renaming_to_an_empty_name_changes_nothing() {
    let tmp = Tmp::new();
    let store = store(&tmp);
    add_page(&store, Group::Spec, "spec/a.md", "# A\n");
    let err = rename_page(&store, "spec/a.md", "   ").unwrap_err();
    assert!(err.contains("needs a name"), "{err}");
    assert!(tmp.0.join("docs/spec/a.md").exists());
}

#[test]
fn a_failed_manifest_write_during_a_rename_leaves_the_old_page_alone() {
    let tmp = Tmp::new();
    let store = store(&tmp);
    add_page(&store, Group::Spec, "spec/a.md", "# A\n");
    std::fs::write(tmp.0.join("docs/docs-manifest.json"), "{ not json").unwrap();
    assert!(rename_page(&store, "spec/a.md", "Bee").is_err());
    assert!(tmp.0.join("docs/spec/a.md").exists());
    assert!(!tmp.0.join("docs/spec/bee.md").exists(), "no half-renamed copy is left behind");
}

#[test]
fn reordering_persists_and_adopts_pages_the_manifest_did_not_list() {
    let tmp = Tmp::new();
    let store = store(&tmp);
    add_page(&store, Group::Spec, "spec/a.md", "# A\n");
    add_page(&store, Group::Spec, "spec/b.md", "# B\n");
    store.write_page("spec/c.md", &Page::parse("---\ngroup: spec\n---\n# C\n"), Expect::Absent).unwrap();
    let snap = Scanner::new(store.clone(), None).snapshot().unwrap();
    let mut order = snap.group_pages(Group::Spec);
    assert_eq!(order, vec!["spec/a.md", "spec/b.md", "spec/c.md"]);
    let moved = order.remove(2);
    order.insert(0, moved);
    reorder_pages(&store, Group::Spec, &order).unwrap();
    assert_eq!(manifest(&store).group(Group::Spec).pages, vec!["spec/c.md", "spec/a.md", "spec/b.md"]);
    let again = Scanner::new(store, None).snapshot().unwrap();
    assert_eq!(again.group_pages(Group::Spec), vec!["spec/c.md", "spec/a.md", "spec/b.md"]);
}

#[test]
fn deleting_a_page_makes_a_restorable_trash_item_and_takes_it_out_of_the_manifest() {
    let tmp = Tmp::new();
    let library = designos_library::Library::open(tmp.0.join("library")).unwrap();
    let project = library.create_project("Checkout").unwrap();
    let store = DocsStore::for_project(library.root(), &project.id).unwrap();
    add_page(&store, Group::Spec, "spec/cart-rules.md", "---\ntitle: Cart rules\n---\n## Limits\n");
    add_page(&store, Group::Spec, "spec/b.md", "# B\n");
    trash_page(&store, library.root(), &project.id, "spec/cart-rules.md").unwrap();
    assert!(!store.dir().join("spec/cart-rules.md").exists());
    let items = library.list_trash();
    assert_eq!(items.len(), 1, "the page shows in the library's trash list");
    assert_eq!((items[0].kind.clone(), items[0].name.as_str()), (designos_library::TrashKind::Page, "Cart rules"));
    assert_eq!(manifest(&store).group(Group::Spec).pages, vec!["spec/b.md".to_string()]);
    library.restore(&items[0].id).unwrap();
    assert_eq!(std::fs::read_to_string(store.dir().join("spec/cart-rules.md")).unwrap(), "---\ntitle: Cart rules\n---\n## Limits\n", "byte for byte");
    assert_eq!(manifest(&store).group(Group::Spec).pages, vec!["spec/b.md".to_string(), "spec/cart-rules.md".to_string()]);
}

#[test]
fn a_delete_while_another_program_holds_the_docs_lock_deletes_nothing() {
    let tmp = Tmp::new();
    let library = designos_library::Library::open(tmp.0.join("library")).unwrap();
    let project = library.create_project("P").unwrap();
    let store = DocsStore::for_project(library.root(), &project.id).unwrap().with_lock_timeout(std::time::Duration::from_millis(30));
    add_page(&store, Group::Spec, "spec/a.md", "# A\n");
    let held = std::fs::File::options().create(true).truncate(false).write(true).open(store.dir().join(".docs.lock")).unwrap();
    held.lock().unwrap();
    let err = trash_page(&store, library.root(), &project.id, "spec/a.md").unwrap_err();
    assert!(err.contains("nothing was deleted"), "{err}");
    assert!(store.dir().join("spec/a.md").exists(), "the page is where it was");
    assert_eq!(manifest(&store).group(Group::Spec).pages, vec!["spec/a.md".to_string()]);
    assert!(library.list_trash().is_empty(), "no trash entry for a page that was not deleted");
    held.unlock().unwrap();
}

#[test]
fn a_new_page_is_created_in_its_group_folder_and_listed() {
    let tmp = Tmp::new();
    let store = store(&tmp);
    add_page(&store, Group::Spec, "spec/untitled.md", "# Taken\n");
    let rel = create_page(&store, Group::Spec, "Untitled").unwrap();
    assert_eq!(rel, "spec/untitled-2.md");
    assert_eq!(store.read_page(&rel).unwrap().value.front_matter().unwrap().title(), Some("Untitled"));
    assert_eq!(manifest(&store).group(Group::Spec).pages, vec!["spec/untitled.md".to_string(), rel]);
    assert!(create_page(&store, Group::Screens, "x").is_err(), "pinned frames are not pages");
    assert_eq!(create_page(&store, Group::Overview, "Intro").unwrap(), "intro.md");
    assert_eq!(create_page(&store, Group::Discovery, "Research").unwrap(), "discovery/research.md");
}

#[test]
fn a_comment_only_raw_block_renders_nothing_but_other_raw_text_does() {
    assert!(is_comment_only("<!-- -->"));
    assert!(is_comment_only("  <!-- a -->\n<!-- b -->  "));
    assert!(!is_comment_only("<!-- a --> text"));
    assert!(!is_comment_only("<!-- unterminated"));
    assert!(!is_comment_only(""));
    assert!(renders_nothing(&BlockKind::Raw("<!-- -->".into())));
    assert!(!renders_nothing(&BlockKind::Raw("<div>x</div>".into())));
}

#[test]
fn slugs_keep_letters_and_digits_and_never_come_out_empty() {
    assert_eq!(slugify("Payment methods"), "payment-methods");
    assert_eq!(slugify("  A / B: c?! "), "a-b-c");
    assert_eq!(slugify("Thanh toán"), "thanh-toán");
    assert_eq!(slugify("???"), "untitled");
    assert_eq!(title_from_path("spec/payment-flow.md"), "Payment flow");
}

#[test]
fn the_scanner_re_reads_a_page_that_changed_and_forgets_one_that_was_deleted() {
    let tmp = Tmp::new();
    let store = store(&tmp);
    add_page(&store, Group::Spec, "spec/a.md", "# A\n");
    let mut scanner = Scanner::new(store.clone(), None);
    assert_eq!(scanner.snapshot().unwrap().title_of("spec/a.md"), "A");
    store.write_page("spec/a.md", &Page::parse("# Alpha\n"), Expect::Hash(store.read_page("spec/a.md").unwrap().hash)).unwrap();
    scanner.invalidate("spec/a.md", None);
    assert_eq!(scanner.snapshot().unwrap().title_of("spec/a.md"), "Alpha");
    std::fs::remove_file(tmp.0.join("docs/spec/a.md")).unwrap();
    assert!(!scanner.snapshot().unwrap().pages.contains_key("spec/a.md"));
}

#[test]
fn a_reorder_keeps_a_page_that_was_added_after_the_tree_was_drawn() {
    let tmp = Tmp::new();
    let store = store(&tmp);
    add_page(&store, Group::Spec, "spec/a.md", "# A\n");
    add_page(&store, Group::Spec, "spec/b.md", "# B\n");
    // The tree shows [a, b]; an agent adds c; the user then drags b above a.
    add_page(&store, Group::Spec, "spec/c.md", "# C\n");
    reorder_pages(&store, Group::Spec, &["spec/b.md".into(), "spec/a.md".into()]).unwrap();
    assert_eq!(manifest(&store).group(Group::Spec).pages, vec!["spec/b.md", "spec/a.md", "spec/c.md"], "c stays in Spec, after the pages the user ordered");
}

#[test]
fn a_reorder_does_not_bring_back_a_page_that_was_deleted_after_the_tree_was_drawn() {
    let tmp = Tmp::new();
    let store = store(&tmp);
    add_page(&store, Group::Spec, "spec/a.md", "# A\n");
    add_page(&store, Group::Spec, "spec/b.md", "# B\n");
    let hash = store.read_page("spec/b.md").unwrap().hash;
    store.remove_page("spec/b.md", hash).unwrap();
    update_manifest(&store, |m| m.group_mut(Group::Spec).pages.retain(|p| p != "spec/b.md")).unwrap();
    reorder_pages(&store, Group::Spec, &["spec/b.md".into(), "spec/a.md".into()]).unwrap();
    assert_eq!(manifest(&store).group(Group::Spec).pages, vec!["spec/a.md"]);
}

#[test]
fn a_rename_that_only_changes_the_case_of_the_file_name_works() {
    let tmp = Tmp::new();
    let store = store(&tmp);
    add_page(&store, Group::Spec, "spec/Payment.md", "---\ntitle: Payment\n---\n## Steps\n");
    add_page(&store, Group::Overview, "overview.md", "See [pay](spec/Payment.md).\n");
    // "Payment" slugs to payment.md: the same file on a case-insensitive file system.
    let done = rename_page(&store, "spec/Payment.md", "Payment").unwrap();
    assert_eq!(done.new_rel, "spec/payment.md", "{done:?}");
    assert_eq!(store.list_page_files().unwrap(), vec!["overview.md", "spec/payment.md"], "one file, with the new case");
    assert_eq!(manifest(&store).group(Group::Spec).pages, vec!["spec/payment.md"]);
    assert_eq!(done.links_updated, 1);
    assert!(done.warnings.is_empty(), "{:?}", done.warnings);
}

#[test]
fn a_rename_while_another_program_holds_the_docs_lock_changes_nothing() {
    let tmp = Tmp::new();
    let store = store(&tmp).with_lock_timeout(std::time::Duration::from_millis(30));
    add_page(&store, Group::Spec, "spec/a.md", "---\ntitle: A\n---\n");
    let held = std::fs::File::options().create(true).truncate(false).write(true).open(tmp.0.join("docs/.docs.lock")).unwrap();
    held.lock().unwrap();
    let err = rename_page(&store, "spec/a.md", "Alpha").unwrap_err();
    assert!(err.contains("locked"), "{err}");
    assert_eq!(store.list_page_files().unwrap(), vec!["spec/a.md"], "no copy was left behind");
    assert_eq!(manifest(&store).group(Group::Spec).pages, vec!["spec/a.md"]);
    held.unlock().unwrap();
    assert_eq!(rename_page(&store, "spec/a.md", "Alpha").unwrap().new_rel, "spec/alpha.md", "and it works once the lock is free");
}
