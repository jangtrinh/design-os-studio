//! `designos FILE --scan-design-system OUT.json`: a headless command that reads a document and writes one JSON file.
//! The output path is checked before the document is read, an existing file is replaced only with `--overwrite`, and a
//! symbolic link is never written through. The scan itself is tested in `designos-fig`.

use std::path::{Path, PathBuf};

const KEEP: &[u8] = b"user data that must survive";

/// A directory of its own under the system temp dir, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("designos_scan_design_system_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    /// A saved document with one empty page: no variables, no components, no frames.
    fn empty_document(&self) -> PathBuf {
        let path = self.path("empty.designos");
        let mut doc = designos_doc::Document::new("Empty");
        doc.add_page("Page 1");
        crate::io::save_path(&doc, &path).unwrap();
        path
    }

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

/// Runs `designos DOC --scan-design-system TARGET [extra...]` in-process. A refusal comes back as the error text.
fn scan(doc: &Path, target: &Path, extra: &[&str]) -> Result<(), String> {
    let mut args = vec![doc.to_string_lossy().into_owned(), "--scan-design-system".into(), target.to_string_lossy().into_owned()];
    args.extend(extra.iter().map(|a| a.to_string()));
    let opts = crate::cli::parse(args)?;
    crate::run_agent_cli(&opts)
}

#[test]
fn a_document_with_no_variables_and_no_frames_scans_to_empty_lists_and_a_complete_sensing() {
    let s = Scratch::new("empty");
    let doc = s.empty_document();
    let target = s.path("ds.json");
    scan(&doc, &target, &[]).unwrap();
    let written: serde_json::Value = serde_json::from_slice(&std::fs::read(&target).unwrap()).unwrap();
    assert_eq!(
        written,
        serde_json::json!({
            "components": [], "tokens": [], "styles": [], "frames": [],
            "counts": {"components": 0, "tokens": 0, "styles": 0},
            "sensing": {"status": "complete"},
        })
    );
    assert_eq!(s.names(), ["ds.json", "empty.designos"], "the scan changes no file but its output");
}

#[test]
fn two_scans_of_one_file_are_byte_identical() {
    let s = Scratch::new("twice");
    let doc = s.empty_document();
    scan(&doc, &s.path("a.json"), &[]).unwrap();
    scan(&doc, &s.path("b.json"), &[]).unwrap();
    assert_eq!(std::fs::read(s.path("a.json")).unwrap(), std::fs::read(s.path("b.json")).unwrap());
}

#[test]
fn a_target_without_the_json_extension_is_refused_and_left_alone() {
    let s = Scratch::new("extension");
    let doc = s.empty_document();
    let victim = s.path("victim.txt");
    std::fs::write(&victim, KEEP).unwrap();
    let missing = s.path("new.txt");
    for extra in [&[][..], &["--overwrite"][..]] {
        for target in [&victim, &missing] {
            let err = scan(&doc, target, extra).unwrap_err();
            assert!(err.contains(".json"), "the error names the extension: {err}");
        }
        assert_eq!(std::fs::read(&victim).unwrap(), KEEP);
        assert!(!missing.exists(), "a refused name is not created");
    }
}

#[test]
fn an_existing_file_is_replaced_only_with_the_overwrite_flag() {
    let s = Scratch::new("overwrite");
    let doc = s.empty_document();
    let target = s.path("ds.json");
    std::fs::write(&target, KEEP).unwrap();
    let err = scan(&doc, &target, &[]).unwrap_err();
    assert!(err.contains("--overwrite"), "the error names the way forward: {err}");
    assert_eq!(std::fs::read(&target).unwrap(), KEEP);
    scan(&doc, &target, &["--overwrite"]).unwrap();
    assert!(std::fs::read(&target).unwrap().starts_with(b"{"), "the old bytes are gone");
    assert_eq!(s.names(), ["ds.json", "empty.designos"], "no temporary file is left behind");
}

#[cfg(unix)]
#[test]
fn a_symlink_is_never_written_through() {
    let s = Scratch::new("symlink");
    let doc = s.empty_document();
    let victim = s.path("victim.txt");
    std::fs::write(&victim, KEEP).unwrap();
    let link = s.path("link.json");
    std::os::unix::fs::symlink(&victim, &link).unwrap();
    for extra in [&[][..], &["--overwrite"][..]] {
        let err = scan(&doc, &link, extra).unwrap_err();
        assert!(err.contains("symbolic link"), "{err}");
        assert_eq!(std::fs::read(&victim).unwrap(), KEEP, "the file behind the link is untouched");
    }
}

#[test]
fn an_unusable_target_stops_the_command_before_the_file_is_read() {
    let s = Scratch::new("order");
    let missing_input = s.path("not-there.designos");
    let err = scan(&missing_input, &s.path("ds.txt"), &[]).unwrap_err();
    assert!(err.contains(".json"), "the target is refused first, not the missing input: {err}");
}

#[test]
fn a_document_whose_nodes_have_no_figma_id_is_refused_with_the_way_forward() {
    let s = Scratch::new("no-figma-id");
    let path = s.path("made-in-studio.designos");
    let mut doc = designos_doc::Document::new("Studio");
    let page = doc.add_page("Page 1");
    let id = doc.alloc_id();
    let mut comp = designos_doc::Node::new(id, designos_doc::NodeKind::Component(Default::default()));
    comp.name = "Button".into();
    doc.insert(page, comp).unwrap();
    crate::io::save_path(&doc, &path).unwrap();
    let target = s.path("ds.json");
    let err = scan(&path, &target, &[]).unwrap_err();
    assert!(err.contains("Button") && err.contains(".fig"), "{err}");
    assert!(!target.exists(), "nothing is written when the scan is refused");
}
