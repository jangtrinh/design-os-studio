//! Storage: location in the library, atomic writes, and conflict detection by content hash.

use designos_docs::{BlockKind, ContentHash, DocsError, DocsStore, Expect, Group, Inline, Manifest, Page, MANIFEST_FILE};
use std::path::Path;
use std::sync::{Arc, Barrier};

fn store() -> (tempfile::TempDir, DocsStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = DocsStore::at(dir.path().join("docs"));
    (dir, store)
}

fn page(text: &str) -> Page {
    Page::parse(text).unwrap()
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

#[test]
fn docs_live_in_the_project_folder_of_the_library() {
    let lib_dir = tempfile::tempdir().unwrap();
    let lib = designos_library::Library::open(lib_dir.path()).unwrap();
    let project = lib.create_project("Checkout").unwrap();
    let store = DocsStore::for_project(lib.root(), &project.id).unwrap();
    assert_eq!(store.dir(), lib.path("projects").join(&project.id).join("docs"));
    assert!(lib.path("projects").join(&project.id).is_dir(), "the project folder the library made is the parent");
}

#[test]
fn a_project_id_cannot_escape_the_projects_folder() {
    for bad in ["", "..", "../x", "a/b", "a\\b", "."] {
        assert!(matches!(DocsStore::for_project(Path::new("/lib"), bad), Err(DocsError::InvalidPath { .. })), "{bad:?}");
    }
}

#[test]
fn write_then_read_gives_the_same_page_and_hash() {
    let (_t, s) = store();
    let p = page("# Hello\n\nWorld.\n");
    let h = s.write_page("about.md", &p, Expect::Absent).unwrap();
    let read = s.read_page("about.md").unwrap();
    assert_eq!(read.hash, h);
    assert_eq!(read.hash, ContentHash::of(b"# Hello\n\nWorld.\n"));
    assert_eq!(read.value.render(), "# Hello\n\nWorld.\n");
    assert_eq!(std::fs::read_to_string(s.dir().join("about.md")).unwrap(), "# Hello\n\nWorld.\n");
    assert_eq!(ContentHash::from_hex(&h.to_string()).unwrap(), h);
}

#[test]
fn subfolders_are_created_and_listed() {
    let (_t, s) = store();
    s.write_page("us/us-1.md", &page("# US 1\n"), Expect::Absent).unwrap();
    s.write_page("about.md", &page("# About\n"), Expect::Absent).unwrap();
    assert_eq!(s.list_page_files().unwrap(), ["about.md", "us/us-1.md"]);
}

#[test]
fn a_write_after_an_outside_edit_is_a_conflict_and_leaves_the_file_unchanged() {
    let (_t, s) = store();
    s.write_page("a.md", &page("# One\n"), Expect::Absent).unwrap();
    let read = s.read_page("a.md").unwrap();

    std::fs::write(s.dir().join("a.md"), "# Edited elsewhere\n").unwrap();

    let mut mine = read.value.clone();
    mine.blocks[0] = designos_docs::Block::new(BlockKind::Heading { level: 1, content: vec![Inline::Text("Mine".into())] });
    let err = s.write_page("a.md", &mine, Expect::Hash(read.hash)).unwrap_err();
    match err {
        DocsError::Conflict { ref path, expected, found } => {
            assert_eq!(path, "a.md");
            assert_eq!(expected, Some(read.hash));
            assert_eq!(found, Some(ContentHash::of(b"# Edited elsewhere\n")));
        }
        other => panic!("expected Conflict, got {other:?}"),
    }
    assert_eq!(std::fs::read_to_string(s.dir().join("a.md")).unwrap(), "# Edited elsewhere\n", "file unchanged");
    assert!(err.to_string().contains("a.md") && err.to_string().contains("changed"), "{err}");
}

#[test]
fn a_conflict_when_the_file_was_deleted_or_appeared_in_the_meantime() {
    let (_t, s) = store();
    s.write_page("a.md", &page("# One\n"), Expect::Absent).unwrap();
    let h = s.read_page("a.md").unwrap().hash;

    // Expecting content, but the page is gone.
    std::fs::remove_file(s.dir().join("a.md")).unwrap();
    assert!(matches!(s.write_page("a.md", &page("# Two\n"), Expect::Hash(h)), Err(DocsError::Conflict { found: None, .. })));
    assert!(!s.dir().join("a.md").exists(), "a refused write must not resurrect the file");

    // Expecting to create a page, but it exists.
    std::fs::write(s.dir().join("b.md"), "# Theirs\n").unwrap();
    assert!(matches!(s.write_page("b.md", &page("# Mine\n"), Expect::Absent), Err(DocsError::Conflict { expected: None, found: Some(_), .. })));
    assert_eq!(std::fs::read_to_string(s.dir().join("b.md")).unwrap(), "# Theirs\n");
}

#[test]
fn a_conflict_is_a_conflict_even_when_the_outside_edit_changed_nothing_visible() {
    // Same length, different bytes: the hash, not size or mtime, decides.
    let (_t, s) = store();
    s.write_page("a.md", &page("# AAA\n"), Expect::Absent).unwrap();
    let h = s.read_page("a.md").unwrap().hash;
    std::fs::write(s.dir().join("a.md"), "# BBB\n").unwrap();
    assert!(matches!(s.write_page("a.md", &page("# CCC\n"), Expect::Hash(h)), Err(DocsError::Conflict { .. })));
}

#[test]
fn writing_identical_content_is_a_no_op() {
    let (_t, s) = store();
    let h = s.write_page("a.md", &page("# One\n"), Expect::Absent).unwrap();
    let before = std::fs::metadata(s.dir().join("a.md")).unwrap().modified().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    assert_eq!(s.write_page("a.md", &page("# One\n"), Expect::Hash(h)).unwrap(), h);
    assert_eq!(std::fs::metadata(s.dir().join("a.md")).unwrap().modified().unwrap(), before, "no spurious change event");
}

#[test]
fn writes_go_through_a_temp_file_and_leave_nothing_behind() {
    let (_t, s) = store();
    let h = s.write_page("a.md", &page("# One\n"), Expect::Absent).unwrap();
    s.write_page("a.md", &page("# Two\n"), Expect::Hash(h)).unwrap();
    assert_eq!(files_in(s.dir()).into_iter().filter(|n| !n.starts_with('.')).collect::<Vec<_>>(), ["a.md"], "{:?}", files_in(s.dir()));
    assert!(files_in(s.dir()).iter().all(|n| !n.ends_with(".tmp")), "{:?}", files_in(s.dir()));
}

#[test]
fn a_failed_write_keeps_the_old_content() {
    let (_t, s) = store();
    let h = s.write_page("a.md", &page("# One\n"), Expect::Absent).unwrap();
    // A directory where the file should go makes the final rename impossible.
    std::fs::create_dir(s.dir().join("blocked.md")).unwrap();
    assert!(s.write_page("blocked.md", &page("# X\n"), Expect::Absent).is_err());
    assert_eq!(s.read_page("a.md").unwrap().hash, h);
}

#[test]
fn only_one_of_many_concurrent_writers_with_the_same_base_wins() {
    let (_t, s) = store();
    let s = Arc::new(s);
    let h = s.write_page("race.md", &page("# Base\n"), Expect::Absent).unwrap();
    let n = 8;
    let gate = Arc::new(Barrier::new(n));
    let handles: Vec<_> = (0..n)
        .map(|i| {
            let (s, gate) = (s.clone(), gate.clone());
            std::thread::spawn(move || {
                gate.wait();
                s.write_page("race.md", &page(&format!("# Writer {i}\n")), Expect::Hash(h))
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1, "{results:?}");
    assert_eq!(results.iter().filter(|r| matches!(r, Err(DocsError::Conflict { .. }))).count(), n - 1);
}

#[test]
fn unsafe_page_paths_are_refused_before_touching_disk() {
    let (t, s) = store();
    for bad in ["../escape.md", "/abs.md", "a/../../b.md", "x.txt", "", "a\\b.md", ".hidden.md", "dir//x.md", "docs-manifest.json"] {
        assert!(matches!(s.write_page(bad, &page("# x\n"), Expect::Absent), Err(DocsError::InvalidPath { .. })), "{bad:?}");
        assert!(matches!(s.read_page(bad), Err(DocsError::InvalidPath { .. })), "{bad:?}");
    }
    assert!(!t.path().join("escape.md").exists());
}

#[test]
fn reading_a_missing_page_or_manifest_says_what_to_do() {
    let (_t, s) = store();
    let e = s.read_page("nope.md").unwrap_err();
    assert!(matches!(e, DocsError::NotFound { .. }), "{e:?}");
    assert!(e.to_string().contains("nope.md"));
    assert!(matches!(s.read_manifest().unwrap_err(), DocsError::NotFound { .. }));
}

#[test]
fn the_manifest_is_validated_on_read_and_write_and_conflict_checked() {
    let (_t, s) = store();
    let mut m = Manifest::new();
    m.group_mut(Group::Overview).pages.push("about.md".into());
    let h = s.write_manifest(&m, Expect::Absent).unwrap();
    assert!(s.dir().join(MANIFEST_FILE).is_file());
    let read = s.read_manifest().unwrap();
    assert_eq!((read.hash, read.value.clone()), (h, m.clone()));

    // An invalid manifest never reaches the disk.
    let mut bad = m.clone();
    bad.group_mut(Group::Spec).pages.push("../x.md".into());
    assert!(matches!(s.write_manifest(&bad, Expect::Hash(h)), Err(DocsError::Manifest(_))));
    assert_eq!(s.read_manifest().unwrap().hash, h);

    // A hand-broken manifest is refused with the field named.
    std::fs::write(s.dir().join(MANIFEST_FILE), "{\"schema_version\":1}").unwrap();
    let DocsError::Manifest(e) = s.read_manifest().unwrap_err() else { panic!("manifest error expected") };
    assert_eq!(e.field, "groups");
    assert!(matches!(s.write_manifest(&m, Expect::Hash(h)), Err(DocsError::Conflict { .. })));
}
