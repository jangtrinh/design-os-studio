//! A deleted docs page goes to the library trash as an item of its own: Home lists it, counts it, and can put it
//! back into its folder and its manifest group.

use designos_doc::Document;
use designos_docs::{DocsStore, Expect, Group, Manifest, Page};
use designos_library::{Library, TrashKind};
use std::path::PathBuf;

struct Tmp(PathBuf);
impl Tmp {
    fn new() -> (Self, Library) {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("designos_trash_pages_{}_{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        let lib = Library::open(&p).unwrap();
        (Tmp(p), lib)
    }
}
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A project with Spec pages `a` and `b`, listed in the manifest.
fn project_with_pages(lib: &Library) -> (String, DocsStore) {
    let project = lib.create_project("Checkout").unwrap();
    lib.create_file(&project.id, None, "Web", &Document::new("Web")).unwrap();
    let store = DocsStore::for_project(lib.root(), &project.id).unwrap();
    for (rel, title) in [("spec/a.md", "Alpha"), ("spec/b.md", "Beta")] {
        store.write_page(rel, &Page::parse(&format!("---\ntitle: {title}\n---\n\ntext of {title}\n")), Expect::Absent).unwrap();
    }
    let mut m = Manifest::new();
    m.group_mut(Group::Spec).pages = vec!["spec/a.md".into(), "spec/b.md".into()];
    store.write_manifest(&m, Expect::Absent).unwrap();
    (project.id, store)
}

fn spec_pages(store: &DocsStore) -> Vec<String> {
    store.read_manifest().unwrap().value.group(Group::Spec).pages.clone()
}

#[test]
fn a_deleted_page_is_listed_in_the_trash_with_its_title_project_path_and_group() {
    let (_t, lib) = Tmp::new();
    let (project, store) = project_with_pages(&lib);
    let item = lib.trash_page(&project, "spec/a.md", "Alpha").unwrap();
    assert!(!store.dir().join("spec/a.md").exists(), "the file left the docs folder");
    assert_eq!(spec_pages(&store), vec!["spec/b.md"], "and the manifest");
    let listed = lib.list_trash();
    assert_eq!(listed.len(), 1, "{listed:?}");
    let t = &listed[0];
    assert_eq!((t.id.as_str(), t.kind.clone(), t.name.as_str()), (item.id.as_str(), TrashKind::Page, "Alpha"));
    assert_eq!(t.project.as_deref(), Some(project.as_str()));
    assert_eq!(t.project_name.as_deref(), Some("Checkout"));
    let page = t.page.as_ref().expect("page record");
    assert_eq!((page.path.as_str(), page.group.as_deref()), ("spec/a.md", Some("spec")));
}

#[test]
fn restoring_a_page_puts_the_file_back_in_its_group() {
    let (_t, lib) = Tmp::new();
    let (project, store) = project_with_pages(&lib);
    let item = lib.trash_page(&project, "spec/a.md", "Alpha").unwrap();
    let rel = lib.restore_page(&item.id).unwrap();
    assert_eq!(rel, "spec/a.md");
    assert!(std::fs::read_to_string(store.dir().join("spec/a.md")).unwrap().contains("text of Alpha"));
    assert_eq!(spec_pages(&store), vec!["spec/b.md", "spec/a.md"], "back in Spec, at the end");
    assert!(lib.list_trash().is_empty(), "the trash entry is gone");
}

#[test]
fn restore_goes_through_the_generic_restore_of_the_trash_list() {
    let (_t, lib) = Tmp::new();
    let (project, store) = project_with_pages(&lib);
    let item = lib.trash_page(&project, "spec/b.md", "Beta").unwrap();
    lib.restore(&item.id).unwrap();
    assert!(store.dir().join("spec/b.md").exists());
}

#[test]
fn restoring_when_the_name_is_taken_uses_the_next_free_name() {
    let (_t, lib) = Tmp::new();
    let (project, store) = project_with_pages(&lib);
    let item = lib.trash_page(&project, "spec/a.md", "Alpha").unwrap();
    // The name is used again while the page is in the trash.
    store.write_page("spec/a.md", &Page::parse("---\ntitle: New Alpha\n---\n"), Expect::Absent).unwrap();
    let rel = lib.restore_page(&item.id).unwrap();
    assert_eq!(rel, "spec/a-2.md");
    assert!(std::fs::read_to_string(store.dir().join("spec/a.md")).unwrap().contains("New Alpha"), "the page that is there stays");
    assert!(std::fs::read_to_string(store.dir().join("spec/a-2.md")).unwrap().contains("text of Alpha"));
    assert!(spec_pages(&store).contains(&"spec/a-2.md".to_string()));
}

#[test]
fn a_page_without_a_listed_group_comes_back_in_discovery() {
    let (_t, lib) = Tmp::new();
    let project = lib.create_project("P").unwrap();
    let store = DocsStore::for_project(lib.root(), &project.id).unwrap();
    store.write_page("loose.md", &Page::parse("---\ntitle: Loose\n---\n"), Expect::Absent).unwrap();
    let item = lib.trash_page(&project.id, "loose.md", "Loose").unwrap();
    assert_eq!(lib.list_trash()[0].page.as_ref().unwrap().group, None, "it was in no group");
    lib.restore_page(&item.id).unwrap();
    assert_eq!(store.read_manifest().unwrap().value.group(Group::Discovery).pages, vec!["loose.md"]);
}

#[test]
fn restoring_into_a_trashed_project_says_to_restore_the_project_first() {
    let (_t, lib) = Tmp::new();
    let (project, _store) = project_with_pages(&lib);
    let item = lib.trash_page(&project, "spec/a.md", "Alpha").unwrap();
    lib.delete_project(&project).unwrap();
    let err = lib.restore_page(&item.id).unwrap_err().to_string();
    assert!(err.contains("restore the project first"), "{err}");
    assert_eq!(lib.list_trash().iter().filter(|t| t.kind == TrashKind::Page).count(), 1, "the page stays in the trash");
}

#[test]
fn empty_trash_counts_and_erases_trashed_pages() {
    let (_t, lib) = Tmp::new();
    let (project, store) = project_with_pages(&lib);
    lib.trash_page(&project, "spec/a.md", "Alpha").unwrap();
    lib.trash_page(&project, "spec/b.md", "Beta").unwrap();
    assert_eq!(lib.list_trash().len(), 2, "the count Home shows in its confirmation includes pages");
    lib.empty_trash().unwrap();
    assert!(lib.list_trash().is_empty());
    assert!(!store.dir().join("spec/a.md").exists());
}

#[test]
fn trashing_a_page_that_is_not_there_changes_nothing() {
    let (_t, lib) = Tmp::new();
    let (project, store) = project_with_pages(&lib);
    assert!(lib.trash_page(&project, "spec/none.md", "None").is_err());
    assert!(lib.list_trash().is_empty());
    assert_eq!(spec_pages(&store), vec!["spec/a.md", "spec/b.md"]);
}
