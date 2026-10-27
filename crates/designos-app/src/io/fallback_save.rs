//! Where an export goes when no save dialog can open: the Downloads folder, or the home folder.
//! A file that is already there is never replaced; the new one gets ` 2`, ` 3` and so on before its extension.

use std::io::Write;
use std::path::{Path, PathBuf};

/// A failed fallback write together with the exact candidate path that was opened or tried.
#[derive(Debug)]
pub struct WriteNewInError {
    pub path: PathBuf,
    pub error: std::io::Error,
}

impl std::fmt::Display for WriteNewInError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.error)
    }
}

impl std::error::Error for WriteNewInError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// The user's home folder.
pub fn user_home() -> Option<PathBuf> {
    ["HOME", "USERPROFILE"].iter().find_map(|name| std::env::var_os(name).filter(|v| !v.is_empty())).map(PathBuf::from)
}

/// `Downloads` inside `home`, creating it when missing so exports never fall back to `~`.
pub fn fallback_dir_in(home: &Path) -> PathBuf {
    let downloads = home.join("Downloads");
    if !downloads.exists() {
        let _ = std::fs::create_dir(&downloads);
    }
    downloads
}

const MAX_STEM_CHARS: usize = 100;
/// With the extension and a ` 10000` suffix this stays under the 255-byte file name limit of common file systems.
const MAX_STEM_BYTES: usize = 200;
/// A longer tail after the last dot is part of the name, not an extension.
const MAX_EXTENSION_BYTES: usize = 16;

/// The first [`MAX_STEM_CHARS`] characters of `stem`, and at most [`MAX_STEM_BYTES`] bytes, cut on a character boundary.
fn cut_stem(stem: &str) -> String {
    let mut cut: String = stem.chars().take(MAX_STEM_CHARS).collect();
    while cut.len() > MAX_STEM_BYTES {
        cut.pop();
    }
    cut
}

/// Writes `bytes` to `dir/file_name`, or to the first free `name 2.ext`, `name 3.ext`, ... Never replaces a file.
/// The name is cleaned first, so a layer called `../x` stays inside `dir`.
pub fn write_new_in(dir: &Path, file_name: &str, bytes: &[u8]) -> Result<PathBuf, WriteNewInError> {
    let clean = designos_export::sanitize_file_name(file_name);
    let (stem, ext) = match clean.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && ext.len() <= MAX_EXTENSION_BYTES => (stem, Some(ext)),
        _ => (clean.as_str(), None),
    };
    let stem = cut_stem(stem);
    for n in 1..=10_000u32 {
        let name = match (n, ext) {
            (1, Some(ext)) => format!("{stem}.{ext}"),
            (1, None) => stem.clone(),
            (_, Some(ext)) => format!("{stem} {n}.{ext}"),
            (_, None) => format!("{stem} {n}"),
        };
        let path = dir.join(name);
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                if let Err(e) = file.write_all(bytes) {
                    std::fs::remove_file(&path).ok();
                    return Err(WriteNewInError { path, error: e });
                }
                return Ok(path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(WriteNewInError { path, error: e }),
        }
    }
    let path = dir.join(if ext.is_some() { format!("{stem}.{}", ext.unwrap_or_default()) } else { stem });
    Err(WriteNewInError { path, error: std::io::Error::new(std::io::ErrorKind::AlreadyExists, "too many candidate names") })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("designos_fallback_{}_{name}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_fallback_is_downloads_when_it_exists() {
        let home = scratch("home_with_downloads");
        std::fs::create_dir(home.join("Downloads")).unwrap();
        assert_eq!(fallback_dir_in(&home), home.join("Downloads"));
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn the_fallback_is_always_downloads_and_never_home() {
        let home = scratch("home_without_downloads");
        assert_eq!(fallback_dir_in(&home), home.join("Downloads"));
        assert!(home.join("Downloads").is_dir(), "missing Downloads folder is created");
        std::fs::remove_dir_all(home.join("Downloads")).unwrap();
        std::fs::write(home.join("Downloads"), b"not a folder").unwrap();
        assert_eq!(fallback_dir_in(&home), home.join("Downloads"));
        assert!(write_new_in(&fallback_dir_in(&home), "a.png", b"x").is_err(), "blocked Downloads errors instead of writing to ~");
        assert!(!home.join("a.png").exists());
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn a_free_name_is_used_as_is() {
        let dir = scratch("free");
        let path = write_new_in(&dir, "Frame 1.png", b"png").unwrap();
        assert_eq!(path, dir.join("Frame 1.png"));
        assert_eq!(std::fs::read(path).unwrap(), b"png");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn taken_names_get_a_number_before_the_extension() {
        let dir = scratch("taken");
        std::fs::write(dir.join("Frame 1.png"), b"first").unwrap();
        std::fs::write(dir.join("Frame 1 2.png"), b"second").unwrap();
        let path = write_new_in(&dir, "Frame 1.png", b"third").unwrap();
        assert_eq!(path, dir.join("Frame 1 3.png"));
        assert_eq!(std::fs::read(dir.join("Frame 1.png")).unwrap(), b"first");
        assert_eq!(std::fs::read(dir.join("Frame 1 2.png")).unwrap(), b"second");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_name_without_an_extension_gets_a_plain_number() {
        let dir = scratch("no_extension");
        std::fs::write(dir.join("notes"), b"first").unwrap();
        assert_eq!(write_new_in(&dir, "notes", b"x").unwrap(), dir.join("notes 2"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_name_cannot_leave_the_folder() {
        let dir = scratch("escape");
        let path = write_new_in(&dir, "../../evil.png", b"x").unwrap();
        assert_eq!(path.parent(), Some(dir.as_path()));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_long_name_is_cut_to_fit_the_file_system() {
        let dir = scratch("long_ascii");
        let path = write_new_in(&dir, &format!("{}.png", "a".repeat(300)), b"x").unwrap();
        let file = path.file_name().unwrap().to_str().unwrap();
        assert_eq!(file, format!("{}.png", "a".repeat(100)), "the stem keeps its first 100 characters");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_long_name_of_wide_characters_is_cut_on_a_character_boundary_within_the_byte_limit() {
        let dir = scratch("long_wide");
        let path = write_new_in(&dir, &format!("{}.png", "界".repeat(300)), b"x").unwrap();
        let file = path.file_name().unwrap().to_str().unwrap();
        assert!(file.ends_with(".png") && file.chars().filter(|c| *c == '界').count() >= 60, "{file}");
        assert!(file.len() <= 255 - " 10000".len(), "{} bytes leaves no room for a number: {file}", file.len());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_long_tail_after_a_dot_is_not_kept_as_an_extension() {
        let dir = scratch("long_tail");
        let path = write_new_in(&dir, &format!("v1.{}", "x".repeat(300)), b"x").unwrap();
        assert!(path.file_name().unwrap().len() <= 255);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_cut_name_still_never_overwrites() {
        let dir = scratch("long_twice");
        let name = format!("{}.png", "b".repeat(300));
        let first = write_new_in(&dir, &name, b"first").unwrap();
        let second = write_new_in(&dir, &name, b"second").unwrap();
        assert_ne!(first, second);
        assert_eq!(std::fs::read(&first).unwrap(), b"first");
        assert_eq!(second.file_name().unwrap().to_str().unwrap(), format!("{} 2.png", "b".repeat(100)));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_missing_folder_is_an_error_not_a_panic() {
        let dir = scratch("missing").join("nope");
        assert!(write_new_in(&dir, "a.png", b"x").is_err());
        std::fs::remove_dir_all(dir.parent().unwrap()).ok();
    }

    #[test]
    fn an_error_names_the_cleaned_path_that_was_tried() {
        let dir = scratch("error_path").join("missing");
        let err = write_new_in(&dir, "../Notes.png", b"x").unwrap_err().to_string();
        assert!(err.contains("missing/_Notes.png"), "{err}");
        assert!(!err.contains("../Notes.png"), "{err}");
        std::fs::remove_dir_all(dir.parent().unwrap()).ok();
    }

    #[test]
    fn a_long_name_error_uses_the_cut_stem() {
        let dir = scratch("long_error_path").join("missing");
        let name = format!("{}.png", "a".repeat(300));
        let err = write_new_in(&dir, &name, b"x").unwrap_err().to_string();
        assert!(err.contains(&format!("missing/{}.png", "a".repeat(100))), "{err}");
        assert!(!err.contains(&name), "{err}");
        std::fs::remove_dir_all(dir.parent().unwrap()).ok();
    }
}
