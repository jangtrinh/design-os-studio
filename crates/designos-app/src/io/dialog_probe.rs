//! Tells a cancelled file dialog from one that could not open.
//!
//! `rfd` answers `None` for both. On Linux a dialog needs `zenity` or an xdg-desktop-portal with a
//! FileChooser backend; when neither is installed every `None` is a failure, not a cancel. The probe reads
//! only the file system, so it never blocks on D-Bus, and it leans toward "present": a wrong "present"
//! keeps the old silent cancel, while a wrong "absent" would write a file after a real cancel.

use super::file_dialog::DialogOutcome;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

const FILE_CHOOSER_INTERFACE: &str = "org.freedesktop.impl.portal.FileChooser";

/// Maps what `rfd` returned to an outcome. `backend_present` runs only when nothing was picked.
pub fn classify<T>(picked: Option<T>, backend_present: impl FnOnce() -> bool) -> DialogOutcome<T> {
    match picked {
        Some(value) => DialogOutcome::Picked(value),
        None if backend_present() => DialogOutcome::Cancelled,
        None => DialogOutcome::Unavailable,
    }
}

/// Whether this machine can show a native file dialog. Only Linux can lack one.
pub fn dialog_backend_present() -> bool {
    !cfg!(target_os = "linux") || ProbeEnv::from_process().backend_present()
}

/// Where a Linux dialog backend could live.
#[derive(Debug, Default)]
pub struct ProbeEnv {
    pub path_dirs: Vec<PathBuf>,
    pub data_dirs: Vec<PathBuf>,
    /// Flatpak and Snap apps reach the portal through their sandbox, not through the host's files.
    pub sandboxed: bool,
}

impl ProbeEnv {
    pub fn from_process() -> Self {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
        let sandboxed = var("FLATPAK_ID").is_some() || var("SNAP").is_some() || Path::new("/.flatpak-info").exists();
        Self::from_vars(var("PATH").as_deref(), var("XDG_DATA_HOME").as_deref(), var("XDG_DATA_DIRS").as_deref(), var("HOME").as_deref(), sandboxed)
    }

    pub fn from_vars(path: Option<&str>, data_home: Option<&str>, data_dirs: Option<&str>, home: Option<&str>, sandboxed: bool) -> Self {
        let split = |list: &str| std::env::split_paths(list).filter(|p| !p.as_os_str().is_empty()).collect::<Vec<_>>();
        let mut dirs = Vec::new();
        match (data_home, home) {
            (Some(dir), _) => dirs.push(PathBuf::from(dir)),
            (None, Some(home)) => dirs.push(Path::new(home).join(".local/share")),
            (None, None) => {}
        }
        dirs.extend(split(data_dirs.unwrap_or("/usr/local/share:/usr/share")));
        Self { path_dirs: path.map(split).unwrap_or_default(), data_dirs: dirs, sandboxed }
    }

    pub fn backend_present(&self) -> bool {
        self.sandboxed || self.path_dirs.iter().any(|dir| is_executable(&dir.join("zenity"))) || self.data_dirs.iter().any(|dir| has_file_chooser_portal(dir))
    }
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// A portal backend package ships `<data dir>/xdg-desktop-portal/portals/<name>.portal` listing what it implements.
fn has_file_chooser_portal(data_dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(data_dir.join("xdg-desktop-portal/portals")) else { return false };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension() == Some(OsStr::new("portal")))
        .any(|path| std::fs::read_to_string(path).is_ok_and(|text| text.contains(FILE_CHOOSER_INTERFACE)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("designos_probe_{}_{name}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn env_in(root: &Path) -> ProbeEnv {
        ProbeEnv { path_dirs: vec![root.join("bin")], data_dirs: vec![root.join("share")], sandboxed: false }
    }

    fn write_portal(root: &Path, name: &str, body: &str) {
        let dir = root.join("share/xdg-desktop-portal/portals");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(name), body).unwrap();
    }

    #[cfg(unix)]
    fn write_zenity(root: &Path, mode: u32) {
        use std::os::unix::fs::PermissionsExt;
        let bin = root.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("zenity"), "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(bin.join("zenity"), std::fs::Permissions::from_mode(mode)).unwrap();
    }

    #[test]
    fn nothing_picked_and_no_backend_is_unavailable() {
        assert_eq!(classify::<PathBuf>(None, || false), DialogOutcome::Unavailable);
    }

    #[test]
    fn nothing_picked_with_a_backend_is_a_cancel() {
        assert_eq!(classify::<PathBuf>(None, || true), DialogOutcome::Cancelled);
    }

    #[test]
    fn a_pick_never_runs_the_probe() {
        let picked = classify(Some(PathBuf::from("/a.png")), || panic!("the probe must not run after a pick"));
        assert_eq!(picked, DialogOutcome::Picked(PathBuf::from("/a.png")));
    }

    #[test]
    fn an_empty_machine_has_no_backend() {
        let root = scratch("empty");
        assert!(!env_in(&root).backend_present());
        std::fs::remove_dir_all(&root).ok();
    }

    #[cfg(unix)]
    #[test]
    fn an_executable_zenity_on_the_path_is_a_backend() {
        let root = scratch("zenity");
        write_zenity(&root, 0o755);
        assert!(env_in(&root).backend_present());
        std::fs::remove_dir_all(&root).ok();
    }

    #[cfg(unix)]
    #[test]
    fn a_zenity_file_that_cannot_run_is_not_a_backend() {
        let root = scratch("zenity_not_executable");
        write_zenity(&root, 0o644);
        assert!(!env_in(&root).backend_present());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_portal_that_implements_the_file_chooser_is_a_backend() {
        let root = scratch("portal");
        write_portal(&root, "gtk.portal", "[portal]\nDBusName=org.freedesktop.impl.portal.desktop.gtk\nInterfaces=org.freedesktop.impl.portal.FileChooser;org.freedesktop.impl.portal.Settings;\n");
        assert!(env_in(&root).backend_present());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_portal_without_a_file_chooser_is_not_a_backend() {
        let root = scratch("portal_without_chooser");
        write_portal(&root, "other.portal", "[portal]\nInterfaces=org.freedesktop.impl.portal.Settings;\n");
        write_portal(&root, "notes.txt", "org.freedesktop.impl.portal.FileChooser");
        assert!(!env_in(&root).backend_present());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_sandboxed_app_always_counts_as_having_a_portal() {
        let root = scratch("sandboxed");
        let env = ProbeEnv { sandboxed: true, ..env_in(&root) };
        assert!(env.backend_present());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_environment_falls_back_to_the_standard_data_dirs() {
        let env = ProbeEnv::from_vars(Some("/usr/bin:/bin"), None, None, Some("/home/u"), false);
        assert_eq!(env.path_dirs, [PathBuf::from("/usr/bin"), PathBuf::from("/bin")]);
        assert_eq!(env.data_dirs, [PathBuf::from("/home/u/.local/share"), PathBuf::from("/usr/local/share"), PathBuf::from("/usr/share")]);
        let set = ProbeEnv::from_vars(None, Some("/data"), Some("/a:/b"), None, false);
        assert_eq!(set.data_dirs, [PathBuf::from("/data"), PathBuf::from("/a"), PathBuf::from("/b")]);
        assert!(set.path_dirs.is_empty());
    }
}
