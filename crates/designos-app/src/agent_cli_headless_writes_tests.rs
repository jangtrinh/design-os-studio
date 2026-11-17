//! The files the headless agent CLI writes besides `--export-fig`: the document it saves (in place or with `--out`) and the
//! `--screenshot` capture. An agent session can reach these commands, so a path it names is not trusted: a link at the final
//! component is never written through, a name that is not a design file is refused, and a refusal leaves the file it points at
//! untouched. The exports are in `agent_cli_export_replace_tests`.
//!
use std::path::{Path, PathBuf};

pub(crate) const KEEP: &[u8] = b"keep this user data";

/// A directory of its own under the system temp dir, removed when dropped.
pub(crate) struct Scratch(PathBuf);

impl Scratch {
    pub(crate) fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("designos_cli_headless_writes_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    pub(crate) fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    /// A saved document named `card.designos`.
    pub(crate) fn document(&self) -> PathBuf {
        let path = self.path("card.designos");
        let mut doc = designos_doc::Document::new("Card");
        doc.add_page("Page 1");
        crate::io::save_path(&doc, &path).unwrap();
        path
    }

    pub(crate) fn names(&self) -> Vec<String> {
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

pub(crate) fn run(args: &[&str]) -> Result<(), String> {
    let opts = crate::cli::parse(args.iter().map(|a| a.to_string()))?;
    crate::run_agent_cli(&opts)
}

pub(crate) fn text(p: &Path) -> &str {
    p.to_str().unwrap()
}

#[cfg(unix)]
pub(crate) fn link(target: &Path, at: &Path) {
    std::os::unix::fs::symlink(target, at).unwrap();
}

#[cfg(unix)]
pub(crate) fn is_link(path: &Path) -> bool {
    std::fs::symlink_metadata(path).unwrap().file_type().is_symlink()
}

// --- the document a command saves --------------------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn an_in_place_save_never_writes_through_a_link() {
    let s = Scratch::new("in_place_link");
    let real = s.document();
    let before = std::fs::read(&real).unwrap();
    let linked = s.path("link.designos");
    link(&real, &linked);

    let err = run(&[text(&linked), "--add-comment", "note"]).unwrap_err();

    assert!(err.contains("symbolic link"), "{err}");
    assert_eq!(std::fs::read(&real).unwrap(), before, "the document the link points at was written through");
    assert!(is_link(&linked));
}

#[cfg(unix)]
#[test]
fn an_out_that_is_a_link_is_refused_even_when_it_dangles() {
    let s = Scratch::new("out_link");
    let doc = s.document();
    let victim = s.path("victim.designos");
    std::fs::write(&victim, KEEP).unwrap();
    let to_file = s.path("to-file.designos");
    let dangling = s.path("dangling.designos");
    link(&victim, &to_file);
    link(&s.path("nothing-here.designos"), &dangling);

    for out in [&to_file, &dangling] {
        let err = run(&[text(&doc), "--add-comment", "note", "--out", text(out)]).unwrap_err();
        assert!(err.contains("symbolic link"), "{err}");
        assert!(is_link(out));
    }
    assert_eq!(std::fs::read(&victim).unwrap(), KEEP);
    assert!(!s.path("nothing-here.designos").exists(), "a dangling link must not make the file behind it");
}

#[cfg(unix)]
#[test]
fn a_document_with_a_second_name_is_not_saved_over_one_of_its_names() {
    let s = Scratch::new("second_name");
    let doc = s.document();
    let other = s.path("other-name.designos");
    std::fs::hard_link(&doc, &other).unwrap();
    let before = std::fs::read(&doc).unwrap();

    let err = run(&[text(&doc), "--add-comment", "note"]).unwrap_err();

    assert!(err.contains("second name"), "{err}");
    assert_eq!(std::fs::read(&doc).unwrap(), before);
    assert_eq!(std::fs::read(&other).unwrap(), before);
}

#[test]
fn a_name_that_is_not_a_design_file_is_refused_and_nothing_is_written() {
    let s = Scratch::new("extension");
    let doc = s.document();
    let svg = s.path("icon.svg");
    let svg_text: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="5" height="5"/></svg>"#;
    std::fs::write(&svg, svg_text).unwrap();

    for out in ["out.txt", "out.png", "out.svg"] {
        let target = s.path(out);
        let err = run(&[text(&doc), "--add-comment", "note", "--out", text(&target)]).unwrap_err();
        assert!(err.contains(".designos") && err.contains("--out <name>.designos"), "the refusal names the endings and the fix: {err}");
        assert!(!target.exists(), "{out} was created");
    }
    let err = run(&[text(&svg), "--add-comment", "note"]).unwrap_err();
    assert!(err.contains(".designos"), "{err}");
    assert_eq!(std::fs::read(&svg).unwrap(), svg_text, "an SVG must not be overwritten with a design file");
}

#[test]
fn a_design_file_name_in_any_letter_case_and_a_name_without_one_are_saved() {
    let s = Scratch::new("extension_ok");
    let doc = s.document();

    for (out, written) in [("a.DESIGNOS", "a.DESIGNOS"), ("b.fig", "b.fig"), ("c.FIG", "c.FIG"), ("d.pen", "d.pen"), ("plain", "plain.designos")] {
        run(&[text(&doc), "--add-comment", "note", "--out", text(&s.path(out))]).unwrap_or_else(|e| panic!("{out}: {e}"));
        assert!(s.path(written).is_file(), "{out} did not write {written}: {:?}", s.names());
    }
}

#[test]
fn an_out_in_a_folder_that_does_not_exist_makes_the_folder() {
    let s = Scratch::new("out_folder");
    let doc = s.document();
    let out = s.path("new").join("deeper").join("copy.designos");

    run(&[text(&doc), "--add-comment", "note", "--out", text(&out)]).unwrap();

    assert!(out.is_file());
}

#[test]
fn a_save_replaces_the_file_whole_and_leaves_no_temporary_file() {
    let s = Scratch::new("replace");
    let doc = s.document();

    run(&[text(&doc), "--add-comment", "note"]).unwrap();

    assert_eq!(s.names(), vec!["card.designos".to_string()]);
    assert!(std::fs::read_to_string(&doc).unwrap().contains("note"), "the comment was saved");
}

// --- --screenshot: the target is checked before the window opens -----------------------------------------------------------

fn startup(args: &[&str]) -> Result<crate::StartupDisposition, (i32, String)> {
    let opts = crate::cli::parse(args.iter().map(|a| a.to_string())).unwrap();
    crate::run_startup_before_window(&opts, std::iter::empty::<(String, String)>(), 4242)
}

#[test]
fn a_capture_name_that_is_not_a_png_stops_the_run_before_the_window() {
    let s = Scratch::new("shot_extension");
    for name in ["shot.jpg", "shot.svg", "shot"] {
        let target = s.path(name);
        let (code, message) = startup(&["--screenshot", text(&target)]).expect_err(name);
        assert_eq!(code, 2, "{name}");
        assert!(message.contains(".png"), "{name}: {message}");
    }
}

#[test]
fn a_capture_target_that_is_a_png_or_missing_lets_the_run_open_the_window() {
    let s = Scratch::new("shot_ok");
    let existing = s.path("SHOT.PNG");
    std::fs::write(&existing, KEEP).unwrap();
    for target in [existing, s.path("fresh.png"), s.path("missing-folder").join("fresh.png")] {
        assert!(matches!(startup(&["--screenshot", text(&target)]), Ok(crate::StartupDisposition::ContinueToWindow)), "{}", target.display());
    }
    assert_eq!(std::fs::read(s.path("SHOT.PNG")).unwrap(), KEEP, "the check writes nothing");
}

#[cfg(unix)]
#[test]
fn a_capture_target_that_is_a_link_stops_the_run_before_the_window() {
    let s = Scratch::new("shot_link");
    let victim = s.path("victim.png");
    std::fs::write(&victim, KEEP).unwrap();
    let to_file = s.path("link.png");
    let dangling = s.path("dangling.png");
    link(&victim, &to_file);
    link(&s.path("nothing-here.png"), &dangling);

    for target in [&to_file, &dangling] {
        let (code, message) = startup(&["--screenshot", text(target)]).expect_err("a link must stop the run");
        assert_eq!(code, 2);
        assert!(message.contains("symbolic link"), "{message}");
    }
    assert_eq!(std::fs::read(&victim).unwrap(), KEEP);
}

#[cfg(unix)]
#[test]
fn a_recording_target_with_a_linked_folder_or_frame_stops_the_run_before_the_window() {
    let s = Scratch::new("shot_sequence_link");
    let panel = "micro-scale-tool:seq:3";
    let outside = s.path("outside");
    std::fs::create_dir_all(&outside).unwrap();
    let linked_folder = s.path("linked-frames");
    link(&outside, &linked_folder);
    let (code, message) = startup(&["--screenshot", text(&linked_folder), "--panel", panel]).expect_err("a linked folder must stop the run");
    assert_eq!(code, 2);
    assert!(message.contains("symbolic link"), "{message}");

    let folder = s.path("frames");
    std::fs::create_dir_all(&folder).unwrap();
    let victim = s.path("victim.jpg");
    std::fs::write(&victim, KEEP).unwrap();
    link(&victim, &folder.join("frame-0002.jpg"));
    let (code, message) = startup(&["--screenshot", text(&folder), "--panel", panel]).expect_err("a linked frame must stop the run");
    assert_eq!(code, 2);
    assert!(message.contains("symbolic link"), "{message}");

    let clean = s.path("clean-frames");
    assert!(matches!(startup(&["--screenshot", text(&clean), "--panel", panel]), Ok(crate::StartupDisposition::ContinueToWindow)));
    assert_eq!(std::fs::read(&victim).unwrap(), KEEP);
    assert!(std::fs::read_dir(&outside).unwrap().next().is_none());
}
