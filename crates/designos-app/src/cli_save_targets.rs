//! Where the headless commands save and capture. An agent session can reach them, so a path it names is not trusted: a link at
//! the final component is never written through, a name that is not a design file is not written over, and a command that also
//! changes the document stops before it changes anything when its target is unusable.
//!
//! The writes themselves go through `designos_fonts::write_atomic_with_no_follow`: it opens the file it replaces without
//! following a link, so a link planted after the check here is refused there too. The checks here exist to stop early and to word
//! the refusal; the writer is the one that cannot be raced.
//!
//! Not here: `--export-fig` and `--scan-design-system` (their own rule, see `cli_export_write` and `cli_scan_design_system`).

use designos_fonts::NoFollowRefusal;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The file name endings a headless save writes: the native one, and the `.fig` and `.pen` exports.
const SAVE_ENDINGS: [&str; 3] = [crate::io::NATIVE_EXT, "fig", "pen"];

fn has_ending(path: &Path, ending: &str) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case(ending))
}

/// The path a headless save writes: a path with no ending gets the native one.
pub fn save_path_with_ending(path: &Path) -> PathBuf {
    let mut with_ending = path.to_path_buf();
    if with_ending.extension().is_none() {
        with_ending.set_extension(crate::io::NATIVE_EXT);
    }
    with_ending
}

/// The sentence for a path the writer refused. `verb` is the thing being done: `save`, `write` or `capture`.
pub fn refusal_message(verb: &str, path: &Path, refusal: NoFollowRefusal) -> String {
    let path = path.display();
    match refusal {
        NoFollowRefusal::Link => format!("Refusing to {verb} {path}: it is a symbolic link. Use a path that is not a link."),
        NoFollowRefusal::SecondName => format!("Refusing to {verb} {path}: it has a second name (a hard link). Use a path that has one name."),
        NoFollowRefusal::Directory => format!("Refusing to {verb} {path}: it is a directory."),
        NoFollowRefusal::NotRegularFile => format!("Refusing to {verb} {path}: it is not a regular file."),
        NoFollowRefusal::NotWritable => format!("Refusing to {verb} {path}: it is read-only or not writable. Make it writable first, or choose a new path."),
    }
}

/// The sentence for a failed write of `path`: the refusal in the words above, or what the operating system said.
pub fn explain_write_error(verb: &str, path: &Path, e: &io::Error) -> String {
    match designos_fonts::no_follow_refusal(e) {
        Some(refusal) => refusal_message(verb, path, refusal),
        None => format!("Cannot {verb} {}: {e}", path.display()),
    }
}

/// Refuses what is at `path` unless it is nothing, or a plain file with one name. Looks at it without following a link.
fn inspect_existing(verb: &str, path: &Path) -> Result<(), String> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("Cannot inspect {}: {e}", path.display())),
    };
    if meta.file_type().is_symlink() {
        return Err(refusal_message(verb, path, NoFollowRefusal::Link));
    }
    if !meta.is_file() {
        return Err(refusal_message(verb, path, if meta.is_dir() { NoFollowRefusal::Directory } else { NoFollowRefusal::NotRegularFile }));
    }
    #[cfg(unix)]
    if std::os::unix::fs::MetadataExt::nlink(&meta) > 1 {
        return Err(refusal_message(verb, path, NoFollowRefusal::SecondName));
    }
    Ok(())
}

// --- the document a command saves ---------------------------------------------------------------------------------------------

/// Checks the path a command that changes the document will save to (`--out`, or the file itself), before anything is loaded or
/// changed: it must end in `.designos`, `.fig` or `.pen` (a path with no ending gets the first), and be new or a plain file.
pub fn check_save_target(path: &Path) -> Result<(), String> {
    let effective = save_path_with_ending(path);
    if !SAVE_ENDINGS.iter().any(|ending| has_ending(&effective, ending)) {
        return Err(format!(
            "Refusing to save {}: a design file ends in .designos, .fig or .pen. Name the output path with one of those endings.",
            effective.display()
        ));
    }
    inspect_existing("save", &effective)
}

// --- replacing an export file -------------------------------------------------------------------------------------------------

/// Checks the file `--export-spec` or `--export-variables` replaces or creates: with `ending`, the name must end in it (in any
/// letter case), and what is there must be nothing or a plain file.
pub fn check_replace_target(path: &Path, ending: Option<&str>) -> Result<(), String> {
    if let Some(ending) = ending {
        if !has_ending(path, ending) {
            return Err(format!("Refusing to write {}: the output path must end in .{ending}. Name it with a .{ending} ending.", path.display()));
        }
    }
    inspect_existing("write", path)
}

/// Checks the folder a bundle of files (variables, recorded frames) is written into and every file it will hold, before any of
/// them is written. `verb` words the refusal (`write`, `capture to`).
pub fn check_replace_folder(verb: &str, folder: &Path, entries: &[String]) -> Result<(), String> {
    match fs::symlink_metadata(folder) {
        Ok(meta) if meta.file_type().is_symlink() => return Err(refusal_message(verb, folder, NoFollowRefusal::Link)),
        Ok(meta) if !meta.is_dir() => return Err(format!("Refusing to {verb} {}: it is not a directory.", folder.display())),
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("Cannot inspect {}: {e}", folder.display())),
    }
    entries.iter().try_for_each(|name| inspect_existing(verb, &folder.join(name)))
}

/// Replaces the file at `path` (or creates it, and the folders above it) with `bytes`, and nothing is written through a link.
/// This is the rule these exports have always had for an existing file: it is replaced.
pub fn replace_file(path: &Path, ending: Option<&str>, bytes: &[u8]) -> Result<(), String> {
    check_replace_target(path, ending)?;
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|e| format!("Cannot create {}: {e}", parent.display()))?;
    }
    designos_fonts::write_atomic_no_follow(path, bytes).map_err(|e| explain_write_error("write", path, &e))
}

// --- --screenshot -------------------------------------------------------------------------------------------------------------

/// The number of frames of a `micro-<slug>:seq:<N>` recording, from what follows `:seq:`.
pub fn sequence_total(after_seq: &str) -> usize {
    after_seq.trim().parse::<usize>().unwrap_or(60).max(2)
}

/// The recorded frames of a `--panel …:seq:<N>` capture, when that is what `panel` asks for.
pub fn sequence_frames_of(panel: Option<&str>) -> Option<usize> {
    panel.and_then(|panel| panel.split_once(":seq:")).map(|(_, count)| sequence_total(count))
}

/// The file name of frame `n` of a recording.
pub fn sequence_frame_name(n: usize) -> String {
    format!("frame-{n:04}.jpg")
}

/// Checks a `--screenshot` target before the window opens. One image goes to a `.png` that is new or a plain file (a repeat capture
/// replaces it). A recording goes to a folder that is new or a real folder, whose frames are new or plain files.
pub fn check_screenshot_target(path: &Path, sequence_frames: Option<usize>) -> Result<(), String> {
    let Some(total) = sequence_frames else {
        if !has_ending(path, "png") {
            return Err(format!("Refusing to capture to {}: a screenshot is a .png. Name the output path with a .png ending.", path.display()));
        }
        return inspect_existing("capture to", path);
    };
    let frames: Vec<String> = (0..total).map(sequence_frame_name).collect();
    check_replace_folder("capture to", path, &frames)
}

/// Makes the folder a recording is written into. A folder that is a link is refused: its frames would land wherever it points.
pub fn ensure_capture_folder(folder: &Path) -> io::Result<()> {
    if fs::symlink_metadata(folder).is_ok_and(|meta| meta.file_type().is_symlink()) {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, format!("{} is a link, not a folder", folder.display())));
    }
    fs::create_dir_all(folder)
}

fn image_error(e: image::ImageError) -> io::Error {
    match e {
        image::ImageError::IoError(io) => io,
        other => io::Error::other(other),
    }
}

/// Writes the capture of one image as a PNG, streamed into the file; an existing plain file is replaced, a link is refused.
pub fn write_png(path: &Path, image: &image::RgbaImage) -> io::Result<()> {
    designos_fonts::write_atomic_with_no_follow(path, |out| image.write_with_encoder(image::codecs::png::PngEncoder::new(out)).map_err(image_error))
}

/// Writes one frame of a recording as a JPEG, streamed into the file; an existing plain file is replaced, a link is refused.
pub fn write_jpeg(path: &Path, image: &image::RgbImage) -> io::Result<()> {
    designos_fonts::write_atomic_with_no_follow(path, |out| image.write_with_encoder(image::codecs::jpeg::JpegEncoder::new(out)).map_err(image_error))
}
