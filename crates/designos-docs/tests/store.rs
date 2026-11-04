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
    Page::parse(text)
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

#[test]
fn a_lock_held_by_another_program_fails_fast_with_an_error_that_names_the_fix() {
    let (_tmp, store) = store();
    let store = store.with_lock_timeout(std::time::Duration::from_millis(150));
    store.write_page("a.md", &page("a\n"), Expect::Absent).unwrap();
    let holder = std::fs::File::options().create(true).truncate(false).write(true).open(store.dir().join(".docs.lock")).unwrap();
    holder.lock().unwrap();

    let started = std::time::Instant::now();
    let err = store.write_page("b.md", &page("b\n"), Expect::Absent).unwrap_err();
    assert!(started.elapsed() < std::time::Duration::from_secs(2), "the wait must be bounded, took {:?}", started.elapsed());
    assert!(matches!(err, DocsError::Busy { .. }), "{err:?}");
    let shown = err.to_string();
    assert!(shown.contains("fix:") && shown.contains("retry"), "{shown}");
    // The lock frees itself when its holder exits; deleting the file while a holder lives would let two writers in.
    assert!(!shown.contains("delete"), "{shown}");
    assert!(!store.dir().join("b.md").exists(), "nothing may be written while the lock is held elsewhere");

    holder.unlock().unwrap();
    store.write_page("b.md", &page("b\n"), Expect::Absent).unwrap();
}

#[test]
fn a_page_over_the_size_limit_is_a_typed_error_that_names_the_limit_and_the_fix() {
    let (_tmp, store) = store();
    std::fs::create_dir_all(store.dir()).unwrap();
    std::fs::write(store.dir().join("big.md"), vec![b'a'; 8 * 1024 * 1024 + 1]).unwrap();
    let err = store.read_page("big.md").unwrap_err();
    assert!(format!("{err:?}").starts_with("TooLarge { ") && format!("{err:?}").contains("limit: 8388608"), "{err:?}");
    let text = err.to_string();
    assert!(text.contains("big.md") && text.contains("8388608") && text.contains("fix: split the page"), "{text}");
}

#[cfg(unix)]
mod symlinks {
    use super::*;
    use std::os::unix::fs::symlink;

    /// A docs folder holding `spec/real.md`, with `secret.md` and a `private/` folder outside the docs folder.
    fn with_outside() -> (tempfile::TempDir, DocsStore, std::path::PathBuf) {
        let (tmp, store) = store();
        std::fs::create_dir_all(store.dir().join("spec")).unwrap();
        std::fs::write(store.dir().join("spec/real.md"), "# real\n").unwrap();
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(outside.join("private")).unwrap();
        std::fs::write(outside.join("secret.md"), "# SECRET\n").unwrap();
        std::fs::write(outside.join("private/notes.md"), "# NOTES\n").unwrap();
        (tmp, store, outside)
    }

    fn assert_refused(err: DocsError, what: &str) {
        assert!(format!("{err:?}").starts_with("OutsideDocs { "), "{what}: {err:?}");
        let text = err.to_string();
        assert!(text.contains("outside the docs folder") && text.contains("fix:"), "{what}: {text}");
    }

    #[test]
    fn a_file_symlink_to_outside_the_docs_folder_is_refused_for_read_and_write() {
        let (_tmp, store, outside) = with_outside();
        symlink(outside.join("secret.md"), store.dir().join("spec/leak.md")).unwrap();
        assert_refused(store.read_page("spec/leak.md").unwrap_err(), "read");
        let hash = ContentHash::of(b"# SECRET\n");
        assert_refused(store.write_page("spec/leak.md", &page("# overwritten\n"), Expect::Hash(hash)).unwrap_err(), "write");
        assert_eq!(std::fs::read_to_string(outside.join("secret.md")).unwrap(), "# SECRET\n", "nothing was written through the link");
    }

    #[test]
    fn a_folder_symlink_to_outside_the_docs_folder_is_refused() {
        let (_tmp, store, outside) = with_outside();
        symlink(&outside, store.dir().join("linked")).unwrap();
        assert_refused(store.read_page("linked/secret.md").unwrap_err(), "read through a folder link");
        assert_refused(store.read_page("linked/private/notes.md").unwrap_err(), "read through a nested folder");
        assert_refused(store.write_page("linked/new.md", &page("# x\n"), Expect::Absent).unwrap_err(), "create through a folder link");
        assert!(!outside.join("new.md").exists());
    }

    #[test]
    fn a_manifest_symlinked_outside_is_refused() {
        let (_tmp, store, outside) = with_outside();
        std::fs::write(outside.join("m.json"), Manifest::new().to_json_pretty().unwrap()).unwrap();
        symlink(outside.join("m.json"), store.dir().join(MANIFEST_FILE)).unwrap();
        assert_refused(store.read_manifest().unwrap_err(), "manifest");
    }

    #[test]
    fn a_symlink_that_stays_inside_the_docs_folder_still_reads_and_a_missing_page_is_just_missing() {
        let (_tmp, store, _outside) = with_outside();
        symlink(store.dir().join("spec/real.md"), store.dir().join("alias.md")).unwrap();
        assert!(store.read_page("alias.md").is_ok());
        assert!(matches!(store.read_page("spec/missing.md").unwrap_err(), DocsError::NotFound { .. }));
        assert!(matches!(store.read_page("nope/missing.md").unwrap_err(), DocsError::NotFound { .. }));
    }
}

#[cfg(unix)]
mod links_r2 {
    use super::*;
    use std::os::unix::fs::symlink;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn docs_with_outside() -> (tempfile::TempDir, DocsStore, std::path::PathBuf) {
        let (tmp, store) = store();
        std::fs::create_dir_all(store.dir()).unwrap();
        std::fs::write(store.dir().join("inside.md"), "# inside\n").unwrap();
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret.md"), "# SECRET\n").unwrap();
        (tmp, store, outside)
    }

    #[test]
    fn a_hard_link_to_a_file_outside_the_docs_folder_is_refused_with_a_fix() {
        let (_tmp, store, outside) = docs_with_outside();
        std::fs::hard_link(outside.join("secret.md"), store.dir().join("hard.md")).unwrap();
        let err = store.read_page("hard.md").unwrap_err();
        assert!(format!("{err:?}").starts_with("HardLinked { "), "{err:?}");
        let text = err.to_string();
        assert!(text.contains("hard.md") && text.contains("more than one") && text.contains("fix:"), "{text}");
        let hash = ContentHash::of(b"# SECRET\n");
        assert!(store.write_page("hard.md", &page("# x\n"), Expect::Hash(hash)).is_err(), "no write through a hard link either");
        assert_eq!(std::fs::read_to_string(outside.join("secret.md")).unwrap(), "# SECRET\n");
        assert!(store.read_page("inside.md").is_ok(), "an ordinary file is unaffected");
    }

    /// A link name that is swapped between `inside.md` and a file outside, while another thread reads it. The page read
    /// must be one of the two things the name legitimately pointed at when the read started (inside), or an error; it
    /// must never be the outside file.
    #[test]
    fn a_symlink_swapped_while_the_page_is_read_never_leaks_the_outside_file() {
        let (tmp, store, outside) = docs_with_outside();
        let link = store.dir().join("x.md");
        symlink(store.dir().join("inside.md"), &link).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let swapper = {
            let (stop, dir, outside) = (stop.clone(), store.dir().to_path_buf(), outside.clone());
            std::thread::spawn(move || {
                let mut flip = false;
                while !stop.load(Ordering::Relaxed) {
                    let target = if flip { dir.join("inside.md") } else { outside.join("secret.md") };
                    let tmp_link = dir.join(".x.tmp");
                    let _ = std::fs::remove_file(&tmp_link);
                    symlink(&target, &tmp_link).unwrap();
                    std::fs::rename(&tmp_link, dir.join("x.md")).unwrap();
                    flip = !flip;
                }
            })
        };
        let mut leaks = 0;
        for _ in 0..20_000 {
            if let Ok(v) = store.read_page("x.md") {
                if v.value.render().contains("SECRET") {
                    leaks += 1;
                }
            }
        }
        stop.store(true, Ordering::Relaxed);
        swapper.join().unwrap();
        drop(tmp);
        assert_eq!(leaks, 0, "the outside file was read through the swapped link");
    }
}

/// Studio saves a page by writing a temporary file and renaming it over the page. A read that overlaps such a save must
/// return a page, or ask for another read; it must never report the page as outside the docs folder.
#[test]
fn a_page_saved_by_rename_while_it_is_read_is_never_reported_as_outside_the_docs_folder() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let (_tmp, store) = store();
    std::fs::create_dir_all(store.dir()).unwrap();
    std::fs::write(store.dir().join("x.md"), "# v0\n").unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let saver = {
        let (stop, dir) = (stop.clone(), store.dir().to_path_buf());
        std::thread::spawn(move || {
            let mut n = 0u64;
            while !stop.load(Ordering::Relaxed) {
                n += 1;
                std::fs::write(dir.join(".x.tmp"), format!("# v{n}\n")).unwrap();
                std::fs::rename(dir.join(".x.tmp"), dir.join("x.md")).unwrap();
            }
        })
    };
    let (mut ok, mut changed, mut other) = (0, 0, Vec::new());
    for _ in 0..3_000 {
        match store.read_page("x.md") {
            Ok(_) => ok += 1,
            Err(DocsError::ChangedDuringRead { .. }) => changed += 1,
            Err(e) => other.push(e.to_string()),
        }
    }
    stop.store(true, Ordering::Relaxed);
    saver.join().unwrap();
    assert!(other.is_empty(), "reads that failed for another reason ({ok} ok, {changed} changed): {:?}", &other[..other.len().min(3)]);
    assert!(ok > 2_000, "most reads succeed: {ok} ok, {changed} changed");
}
