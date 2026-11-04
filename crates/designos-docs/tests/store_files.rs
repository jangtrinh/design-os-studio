//! Moving, removing and renaming page files happens under the same advisory lock and hash check as a write, and
//! the manifest can be edited against a fresh read with a retry.

use designos_docs::{ContentHash, DocsError, DocsStore, Expect, Group, Manifest, Page};

fn store() -> (tempfile::TempDir, DocsStore) {
    let tmp = tempfile::tempdir().unwrap();
    let store = DocsStore::at(tmp.path().join("docs"));
    (tmp, store)
}

fn write(store: &DocsStore, rel: &str, text: &str) -> ContentHash {
    store.write_page(rel, &Page::parse(text), Expect::Absent).unwrap()
}

#[test]
fn a_page_moves_out_only_if_it_still_has_the_content_that_was_read() {
    let (tmp, store) = store();
    let hash = write(&store, "spec/a.md", "---\ntitle: A\n---\n\ntext\n");
    let dest = tmp.path().join("trash/page.md");
    store.write_page("spec/a.md", &Page::parse("---\ntitle: A\n---\n\nchanged\n"), Expect::Hash(hash)).unwrap();
    let err = store.move_page_out("spec/a.md", &dest, hash).unwrap_err();
    assert!(matches!(err, DocsError::Conflict { .. }), "{err}");
    assert!(store.dir().join("spec/a.md").exists() && !dest.exists(), "nothing moved");
    let fresh = store.read_page("spec/a.md").unwrap().hash;
    store.move_page_out("spec/a.md", &dest, fresh).unwrap();
    assert!(!store.dir().join("spec/a.md").exists() && dest.exists());
}

#[test]
fn moving_a_page_out_waits_for_the_docs_lock_and_gives_up_with_busy() {
    let (tmp, store) = store();
    let store = store.with_lock_timeout(std::time::Duration::from_millis(60));
    let hash = write(&store, "a.md", "text\n");
    let lock = std::fs::File::options().write(true).open(store.dir().join(".docs.lock")).unwrap();
    lock.lock().unwrap();
    let dest = tmp.path().join("t/page.md");
    let err = store.move_page_out("a.md", &dest, hash).unwrap_err();
    assert!(matches!(err, DocsError::Busy { .. }), "{err}");
    assert!(store.dir().join("a.md").exists(), "a locked folder is left alone");
    let err = store.remove_page("a.md", hash).unwrap_err();
    assert!(matches!(err, DocsError::Busy { .. }), "{err}");
}

#[test]
fn remove_page_checks_the_hash_under_the_lock() {
    let (_tmp, store) = store();
    let hash = write(&store, "a.md", "text\n");
    std::fs::write(store.dir().join("a.md"), "someone else wrote this\n").unwrap();
    assert!(matches!(store.remove_page("a.md", hash), Err(DocsError::Conflict { .. })));
    assert!(store.dir().join("a.md").exists());
    let fresh = store.read_page("a.md").unwrap().hash;
    store.remove_page("a.md", fresh).unwrap();
    assert!(!store.dir().join("a.md").exists());
}

#[test]
fn rename_page_moves_a_file_and_refuses_to_overwrite_another() {
    let (_tmp, store) = store();
    let a = write(&store, "a.md", "one\n");
    write(&store, "b.md", "two\n");
    assert!(matches!(store.rename_page("a.md", "b.md", a), Err(DocsError::Conflict { .. })));
    assert_eq!(std::fs::read_to_string(store.dir().join("b.md")).unwrap(), "two\n");
    store.rename_page("a.md", "c.md", a).unwrap();
    assert!(!store.dir().join("a.md").exists() && store.dir().join("c.md").exists());
}

#[test]
fn a_rename_that_only_changes_the_case_works_on_any_file_system() {
    let (_tmp, store) = store();
    let hash = write(&store, "spec/Payment.md", "text\n");
    store.rename_page("spec/Payment.md", "spec/payment.md", hash).unwrap();
    let names: Vec<String> = store.list_page_files().unwrap();
    assert_eq!(names, vec!["spec/payment.md".to_string()], "the file has the new case, and there is one file");
    let hash = store.read_page("spec/payment.md").unwrap().hash;
    store.rename_page("spec/payment.md", "spec/PAYMENT.md", hash).unwrap();
    assert_eq!(store.list_page_files().unwrap(), vec!["spec/PAYMENT.md".to_string()]);
}

#[test]
fn adopt_file_puts_a_file_in_and_never_overwrites() {
    let (tmp, store) = store();
    let src = tmp.path().join("in.md");
    std::fs::write(&src, "back\n").unwrap();
    store.adopt_file(&src, "spec/x.md").unwrap();
    assert_eq!(std::fs::read_to_string(store.dir().join("spec/x.md")).unwrap(), "back\n");
    std::fs::write(&src, "again\n").unwrap();
    assert!(matches!(store.adopt_file(&src, "spec/x.md"), Err(DocsError::Conflict { .. })));
    assert_eq!(std::fs::read_to_string(store.dir().join("spec/x.md")).unwrap(), "back\n");
}

#[test]
fn update_manifest_edits_a_fresh_read_and_retries_after_a_conflict() {
    let (_tmp, store) = store();
    let mut m = Manifest::new();
    m.group_mut(Group::Spec).pages = vec!["a.md".into()];
    store.write_manifest(&m, Expect::Absent).unwrap();
    let mut calls = 0;
    store
        .update_manifest(|m| {
            calls += 1;
            if calls == 1 {
                // Another writer gets in after the read this edit is based on.
                let mut other = store.read_manifest().unwrap();
                other.value.group_mut(Group::Spec).pages.push("b.md".into());
                store.write_manifest(&other.value, Expect::Hash(other.hash)).unwrap();
            }
            m.group_mut(Group::Spec).pages.push("c.md".into());
        })
        .unwrap();
    assert_eq!(calls, 2, "the edit ran again on the fresh manifest");
    assert_eq!(store.read_manifest().unwrap().value.group(Group::Spec).pages, vec!["a.md", "b.md", "c.md"]);
}

#[test]
fn update_manifest_starts_from_an_empty_manifest_when_there_is_none() {
    let (_tmp, store) = store();
    store.update_manifest(|m| m.group_mut(Group::Overview).pages.push("overview.md".into())).unwrap();
    assert_eq!(store.read_manifest().unwrap().value.group(Group::Overview).pages, vec!["overview.md"]);
}
