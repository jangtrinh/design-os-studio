//! Regression probes for the headless writers covered by issue #1009.
//! These tests were added against the base commit before the production fix.

use std::path::{Path, PathBuf};

const KEEP: &[u8] = b"keep this user data";

struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("designos_cli_writers_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn document(&self) -> PathBuf {
        let path = self.path("card.designos");
        let mut doc = designos_doc::Document::new("Card");
        doc.add_page("Page 1");
        crate::io::save_path(&doc, &path).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(doc: &Path, args: &[&str]) -> Result<(), String> {
    let mut argv = vec![doc.to_string_lossy().into_owned()];
    argv.extend(args.iter().map(|s| (*s).to_string()));
    let opts = crate::cli::parse(argv)?;
    crate::run_agent_cli(&opts)
}

#[test]
fn export_spec_rejects_wrong_extension_existing_file_and_symlink() {
    let s = Scratch::new("spec");
    let doc = s.document();

    let wrong = s.path("out.txt");
    std::fs::write(&wrong, KEEP).unwrap();
    let err = run(&doc, &["--export-spec", wrong.to_str().unwrap()]).unwrap_err();
    assert!(err.contains(".json"), "{err}");
    assert_eq!(std::fs::read(&wrong).unwrap(), KEEP);

    let existing = s.path("out.json");
    std::fs::write(&existing, KEEP).unwrap();
    let err = run(&doc, &["--export-spec", existing.to_str().unwrap()]).unwrap_err();
    assert!(err.contains("--overwrite"), "{err}");
    assert_eq!(std::fs::read(&existing).unwrap(), KEEP);

    #[cfg(unix)]
    {
        let victim = s.path("victim.json");
        let link = s.path("link.json");
        std::fs::write(&victim, KEEP).unwrap();
        std::os::unix::fs::symlink(&victim, &link).unwrap();
        let err = run(&doc, &["--export-spec", link.to_str().unwrap(), "--overwrite"]).unwrap_err();
        assert!(err.contains("symbolic link"), "{err}");
        assert_eq!(std::fs::read(&victim).unwrap(), KEEP);
    }
}

#[test]
fn export_spec_and_variables_allow_case_insensitive_extensions_with_explicit_overwrite() {
    let s = Scratch::new("overwrite");
    let doc = s.document();

    let spec = s.path("OUT.JSON");
    std::fs::write(&spec, KEEP).unwrap();
    run(&doc, &["--export-spec", spec.to_str().unwrap(), "--overwrite"]).unwrap();
    assert!(serde_json::from_slice::<serde_json::Value>(&std::fs::read(&spec).unwrap()).is_ok());

    let vars = s.path("TOKENS.ZIP");
    std::fs::write(&vars, KEEP).unwrap();
    run(&doc, &["--export-variables", vars.to_str().unwrap(), "--overwrite"]).unwrap();
    assert!(std::fs::read(&vars).unwrap().starts_with(b"PK"));
}

#[test]
fn export_variables_rejects_existing_json_and_symlink_bundle_entry() {
    let s = Scratch::new("variables");
    let doc = s.document();

    let existing = s.path("tokens.json");
    std::fs::write(&existing, KEEP).unwrap();
    let err = run(&doc, &["--export-variables", existing.to_str().unwrap()]).unwrap_err();
    assert!(err.contains("--overwrite"), "{err}");
    assert_eq!(std::fs::read(&existing).unwrap(), KEEP);

    #[cfg(unix)]
    {
        let victim = s.path("victim.json");
        let dir = s.path("bundle");
        let link = dir.join("tokens.manifest.json");
        std::fs::write(&victim, KEEP).unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        std::os::unix::fs::symlink(&victim, &link).unwrap();
        let err = run(&doc, &["--export-variables", dir.to_str().unwrap(), "--format", "dtcg"]).unwrap_err();
        assert!(err.contains("symbolic link"), "{err}");
        assert_eq!(std::fs::read(&victim).unwrap(), KEEP);
    }
}

#[test]
fn import_capture_out_rejects_a_final_symlink() {
    let s = Scratch::new("out");
    let capture = s.path("card.capture.json");
    std::fs::write(
        &capture,
        r#"{"schema_version":1,"url":"https://example.com/card","title":"Card","viewport":{"width":100,"height":100,"dpr":1},"root":{"tag":"div","bounds":{"x":0,"y":0,"w":10,"h":10},"style":{"display":"block"}}}"#,
    )
    .unwrap();
    let victim = s.path("victim.designos");
    let link = s.path("link.designos");
    std::fs::write(&victim, KEEP).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&victim, &link).unwrap();
    #[cfg(unix)]
    {
        let err = run(&s.path("new.designos"), &["--import-capture", capture.to_str().unwrap(), "--out", link.to_str().unwrap()]).unwrap_err();
        assert!(err.contains("symbolic link"), "{err}");
        assert_eq!(std::fs::read(&victim).unwrap(), KEEP);
    }
}

#[test]
fn screenshot_targets_validate_png_and_allow_repeatable_regular_files() {
    let s = Scratch::new("screenshot");
    let png = s.path("SHOT.PNG");
    std::fs::write(&png, KEEP).unwrap();
    crate::cli_export_write::check_screenshot_target(&png, None).unwrap();
    assert!(crate::cli_export_write::check_screenshot_target(&s.path("shot.jpg"), None).is_err());

    #[cfg(unix)]
    {
        let victim = s.path("victim.png");
        let link = s.path("link.png");
        std::fs::write(&victim, KEEP).unwrap();
        std::os::unix::fs::symlink(&victim, &link).unwrap();
        let err = crate::cli_export_write::check_screenshot_target(&link, None).unwrap_err();
        assert!(err.contains("symbolic link"), "{err}");
    }
}

#[test]
fn screenshot_sequence_rejects_symlink_directory_and_frame() {
    let s = Scratch::new("sequence");
    let frames = s.path("frames");
    std::fs::create_dir_all(&frames).unwrap();
    crate::cli_export_write::check_screenshot_target(&frames, Some(2)).unwrap();

    #[cfg(unix)]
    {
        let outside = s.path("outside");
        std::fs::create_dir_all(&outside).unwrap();
        let linked_dir = s.path("linked-frames");
        std::os::unix::fs::symlink(&outside, &linked_dir).unwrap();
        assert!(crate::cli_export_write::check_screenshot_target(&linked_dir, Some(2)).unwrap_err().contains("symbolic link"));

        let frame = frames.join("frame-0000.jpg");
        let victim = s.path("victim.jpg");
        std::fs::write(&victim, KEEP).unwrap();
        std::os::unix::fs::symlink(&victim, &frame).unwrap();
        assert!(crate::cli_export_write::check_screenshot_target(&frames, Some(2)).unwrap_err().contains("symbolic link"));
    }
}
