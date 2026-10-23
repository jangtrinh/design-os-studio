//! `designos FILE --export-fig OUT`: the output path is checked before anything is written. An agent session can
//! reach this command, so a wrong name, an existing file and a symbolic link must each be refused and leave the
//! file they point at untouched.

use std::path::{Path, PathBuf};

const KEEP: &[u8] = b"user data that must survive";

/// A directory of its own under the system temp dir, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("designos_export_fig_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    /// A saved document to export, named `card.designos`.
    fn document(&self) -> PathBuf {
        let path = self.path("card.designos");
        let mut doc = designos_doc::Document::new("Card");
        doc.add_page("Page 1");
        crate::io::save_path(&doc, &path).unwrap();
        path
    }

    /// The names in the directory, sorted.
    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.0).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Runs `designos DOC --export-fig TARGET [extra...]` in-process. A refusal comes back as the error text.
fn export(doc: &Path, target: &Path, extra: &[&str]) -> Result<(), String> {
    let mut args = vec![doc.to_string_lossy().into_owned(), "--export-fig".into(), target.to_string_lossy().into_owned()];
    args.extend(extra.iter().map(|a| a.to_string()));
    let opts = crate::cli::parse(args)?;
    crate::run_agent_cli(&opts)
}

/// Both spellings of the command: without the overwrite flag, and with it.
const WITHOUT_AND_WITH_OVERWRITE: [&[&str]; 2] = [&[], &["--overwrite"]];

fn is_fig_archive(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK")
}

#[test]
fn a_target_without_the_fig_extension_is_refused_and_left_alone() {
    let s = Scratch::new("extension");
    let doc = s.document();
    let victim = s.path("victim.txt");
    std::fs::write(&victim, KEEP).unwrap();
    let bare = s.path("victim");
    std::fs::write(&bare, KEEP).unwrap();
    let missing = s.path("new.txt");
    for extra in WITHOUT_AND_WITH_OVERWRITE {
        for target in [&victim, &bare, &missing] {
            let err = export(&doc, target, extra).unwrap_err();
            assert!(err.contains(".fig"), "the error names the extension: {err}");
        }
        assert_eq!(std::fs::read(&victim).unwrap(), KEEP);
        assert_eq!(std::fs::read(&bare).unwrap(), KEEP);
        assert!(!missing.exists(), "a refused name is not created");
    }
}

#[test]
fn the_fig_extension_is_accepted_in_any_letter_case() {
    let s = Scratch::new("case");
    let doc = s.document();
    let target = s.path("OUT.FIG");
    export(&doc, &target, &[]).unwrap();
    assert!(is_fig_archive(&std::fs::read(&target).unwrap()));
}

#[test]
fn an_existing_file_is_not_replaced_without_the_overwrite_flag() {
    let s = Scratch::new("exists");
    let doc = s.document();
    let target = s.path("out.fig");
    std::fs::write(&target, KEEP).unwrap();
    let err = export(&doc, &target, &[]).unwrap_err();
    assert!(err.contains("--overwrite"), "the error names the way forward: {err}");
    assert_eq!(std::fs::read(&target).unwrap(), KEEP);
}

#[test]
fn the_overwrite_flag_replaces_a_regular_file_and_leaves_no_temporary_file() {
    let s = Scratch::new("overwrite");
    let doc = s.document();
    let target = s.path("out.fig");
    std::fs::write(&target, KEEP).unwrap();
    export(&doc, &target, &["--overwrite"]).unwrap();
    assert!(is_fig_archive(&std::fs::read(&target).unwrap()), "the old bytes are gone");
    assert_eq!(s.names(), ["card.designos", "out.fig"], "no temporary file is left behind");
}

#[cfg(unix)]
#[test]
fn the_overwrite_flag_keeps_the_permissions_of_the_replaced_file() {
    use std::os::unix::fs::PermissionsExt;
    let s = Scratch::new("mode");
    let doc = s.document();
    let target = s.path("out.fig");
    std::fs::write(&target, KEEP).unwrap();
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o640)).unwrap();
    export(&doc, &target, &["--overwrite"]).unwrap();
    assert!(is_fig_archive(&std::fs::read(&target).unwrap()));
    assert_eq!(std::fs::metadata(&target).unwrap().permissions().mode() & 0o777, 0o640);
}

#[cfg(unix)]
#[test]
fn the_overwrite_flag_does_not_replace_a_read_only_file() {
    use std::os::unix::fs::PermissionsExt;
    let s = Scratch::new("read-only");
    let doc = s.document();
    let target = s.path("out.fig");
    std::fs::write(&target, KEEP).unwrap();
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o444)).unwrap();
    let err = export(&doc, &target, &["--overwrite"]).unwrap_err();
    assert!(err.contains("read-only"), "{err}");
    assert_eq!(std::fs::read(&target).unwrap(), KEEP);
    assert_eq!(s.names(), ["card.designos", "out.fig"], "nothing else was written");
}

#[test]
fn a_new_file_is_created_with_its_missing_folders() {
    let s = Scratch::new("new");
    let doc = s.document();
    let target = s.path("out/nested/new.fig");
    export(&doc, &target, &[]).unwrap();
    assert!(is_fig_archive(&std::fs::read(&target).unwrap()));
}

#[test]
fn a_directory_is_never_replaced() {
    let s = Scratch::new("directory");
    let doc = s.document();
    let target = s.path("out.fig");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("inside.txt"), KEEP).unwrap();
    for extra in WITHOUT_AND_WITH_OVERWRITE {
        let err = export(&doc, &target, extra).unwrap_err();
        assert!(err.contains("regular file"), "{err}");
        assert_eq!(std::fs::read(target.join("inside.txt")).unwrap(), KEEP);
    }
}

#[cfg(unix)]
#[test]
fn a_symlink_to_a_file_is_never_written_through() {
    let s = Scratch::new("symlink");
    let doc = s.document();
    let victim = s.path("victim.txt");
    std::fs::write(&victim, KEEP).unwrap();
    let link = s.path("link.fig");
    std::os::unix::fs::symlink(&victim, &link).unwrap();
    for extra in WITHOUT_AND_WITH_OVERWRITE {
        let err = export(&doc, &link, extra).unwrap_err();
        assert!(err.contains("symbolic link"), "{err}");
        assert_eq!(std::fs::read(&victim).unwrap(), KEEP, "the file behind the link is untouched");
        assert_eq!(std::fs::read_link(&link).unwrap(), victim, "the link is still the same link");
    }
}

#[cfg(unix)]
#[test]
fn a_symlink_to_a_missing_target_is_never_written_through() {
    let s = Scratch::new("dangling");
    let doc = s.document();
    let gone = s.path("gone.txt");
    let link = s.path("link.fig");
    std::os::unix::fs::symlink(&gone, &link).unwrap();
    for extra in WITHOUT_AND_WITH_OVERWRITE {
        let err = export(&doc, &link, extra).unwrap_err();
        assert!(err.contains("symbolic link"), "{err}");
        assert!(!gone.exists(), "the link's target is not created");
        assert_eq!(std::fs::read_link(&link).unwrap(), gone);
    }
}

/// The command checks the target before it starts, and the writer checks again at the moment it writes: a path can
/// change in between, so each check has to hold on its own.
#[test]
fn the_writer_refuses_on_its_own_without_the_up_front_check() {
    use crate::cli_export_write::write_export;
    let s = Scratch::new("writer");
    let wrong_name = s.path("note.txt");
    assert!(write_export(&wrong_name, "fig", b"new", true).unwrap_err().contains(".fig"));
    assert!(!wrong_name.exists());

    let existing = s.path("exists.fig");
    std::fs::write(&existing, KEEP).unwrap();
    assert!(write_export(&existing, "fig", b"new", false).unwrap_err().contains("--overwrite"));
    assert_eq!(std::fs::read(&existing).unwrap(), KEEP);

    #[cfg(unix)]
    {
        let link = s.path("link.fig");
        std::os::unix::fs::symlink(&existing, &link).unwrap();
        for overwrite in [false, true] {
            assert!(write_export(&link, "fig", b"new", overwrite).unwrap_err().contains("symbolic link"));
            assert_eq!(std::fs::read(&existing).unwrap(), KEEP);
        }
    }
}

#[test]
fn a_refused_target_stops_the_command_before_it_changes_the_document() {
    let s = Scratch::new("preflight");
    let doc = s.document();
    let spec = s.path("spec.json");
    std::fs::write(&spec, r#"{"preset":"easeui-card","name":"PricingCard"}"#).unwrap();
    let victim = s.path("victim.txt");
    std::fs::write(&victim, KEEP).unwrap();
    let nodes_before = crate::io::load_path(&doc).unwrap().nodes.len();
    let opts = crate::cli::parse([
        doc.to_string_lossy().into_owned(),
        "--apply-spec".into(),
        spec.to_string_lossy().into_owned(),
        "--export-fig".into(),
        victim.to_string_lossy().into_owned(),
    ])
    .unwrap();
    assert!(crate::run_agent_cli(&opts).is_err());
    assert_eq!(crate::io::load_path(&doc).unwrap().nodes.len(), nodes_before, "the spec was not applied");
    assert_eq!(std::fs::read(&victim).unwrap(), KEEP);
}
