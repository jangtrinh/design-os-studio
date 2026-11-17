//! Where `--screenshot` writes: the capture of a single image and the frames of a `micro-<slug>:seq:<N>` recording. The path comes
//! from the command line, so a link at the final component, or a recording folder that is a link, is never written through. An
//! existing regular image is still replaced (repeat captures are what the flag is for).

use super::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const SIZE: (u32, u32) = (1600, 1088);
const KEEP: &[u8] = b"keep this image";

fn scratch_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("designos_shot_targets_{}_{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn single_sim(path: &Path) -> Sim {
    Sim::with(Options {
        fixture: Some(crate::showcase::FIXTURE.to_string()),
        select: Some("none".to_string()),
        size: Some(SIZE),
        screenshot: Some(path.to_path_buf()),
        ..Default::default()
    })
}

fn sequence_sim(folder: &Path) -> Sim {
    Sim::with(Options {
        fixture: Some(crate::showcase::FIXTURE.to_string()),
        panel: Some("micro-scale-tool:seq:2".to_string()),
        size: Some(SIZE),
        screenshot: Some(folder.to_path_buf()),
        ..Default::default()
    })
}

/// Runs frames until the window asks for the screenshot, then delivers an image for it.
fn deliver_one_image(s: &mut Sim) {
    let asked = (1..=60).any(|_| {
        s.frame(vec![]);
        s.commands.iter().any(|c| matches!(c, egui::ViewportCommand::Screenshot(_)))
    });
    assert!(asked, "the window never asked for a screenshot");
    let image = Arc::new(egui::ColorImage::filled([SIZE.0 as usize, SIZE.1 as usize], egui::Color32::from_rgb(0x18, 0x18, 0x1B)));
    s.frame(vec![Event::Screenshot { viewport_id: egui::ViewportId::ROOT, user_data: egui::UserData::default(), image }]);
}

fn outcome(s: &Sim) -> ShotOutcome {
    s.shell.shot.as_ref().expect("shot state exists").outcome.clone()
}

fn assert_refused(s: &Sim) {
    match outcome(s) {
        ShotOutcome::Failed(reason) => assert!(reason.starts_with("cannot write "), "expected a cannot-write reason, got: {reason:?}"),
        other => panic!("expected the capture to fail, got: {other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn a_capture_is_never_written_through_a_link_at_its_path() {
    let dir = scratch_dir("single_link");
    let victim = dir.join("victim.png");
    let path = dir.join("shot.png");
    std::fs::write(&victim, KEEP).unwrap();
    std::os::unix::fs::symlink(&victim, &path).unwrap();
    let mut s = single_sim(&path);

    deliver_one_image(&mut s);

    assert_refused(&s);
    assert_eq!(std::fs::read(&victim).unwrap(), KEEP, "the file the link points at was written through");
    assert!(std::fs::symlink_metadata(&path).unwrap().file_type().is_symlink());
    SHOT_WRITE_FAILED.store(false, Ordering::SeqCst);
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn a_recorded_frame_is_never_written_through_a_link() {
    let dir = scratch_dir("frame_link");
    let victim = dir.join("victim.jpg");
    let folder = dir.join("frames");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(&victim, KEEP).unwrap();
    std::os::unix::fs::symlink(&victim, folder.join("frame-0000.jpg")).unwrap();
    let mut s = sequence_sim(&folder);

    deliver_one_image(&mut s);

    assert_refused(&s);
    assert_eq!(std::fs::read(&victim).unwrap(), KEEP);
    SHOT_WRITE_FAILED.store(false, Ordering::SeqCst);
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn a_recording_folder_that_is_a_link_is_not_written_into() {
    let dir = scratch_dir("folder_link");
    let outside = dir.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    let folder = dir.join("frames");
    std::os::unix::fs::symlink(&outside, &folder).unwrap();
    let mut s = sequence_sim(&folder);

    deliver_one_image(&mut s);

    assert_refused(&s);
    assert!(std::fs::read_dir(&outside).unwrap().next().is_none(), "a frame was written into the folder behind the link");
    SHOT_WRITE_FAILED.store(false, Ordering::SeqCst);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_capture_replaces_an_existing_image_and_leaves_no_temporary_file() {
    let dir = scratch_dir("replace");
    let path = dir.join("shot.png");
    std::fs::write(&path, KEEP).unwrap();
    let mut s = single_sim(&path);

    deliver_one_image(&mut s);

    assert_eq!(outcome(&s), ShotOutcome::Written);
    let bytes = std::fs::read(&path).unwrap();
    let decoded = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png).expect("the capture is a PNG");
    assert_eq!((decoded.width(), decoded.height()), SIZE);
    let names: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names, vec!["shot.png".to_string()], "no temporary file stays behind");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_recorded_frame_is_a_jpeg_written_into_a_folder_that_is_made_when_missing() {
    let dir = scratch_dir("frames_ok");
    let folder = dir.join("new").join("frames");
    let mut s = sequence_sim(&folder);

    deliver_one_image(&mut s);

    let bytes = std::fs::read(folder.join("frame-0000.jpg")).expect("the first frame was written");
    let decoded = image::load_from_memory_with_format(&bytes, image::ImageFormat::Jpeg).expect("the frame is a JPEG");
    assert_eq!((decoded.width(), decoded.height()), SIZE);
    let _ = std::fs::remove_dir_all(&dir);
}
