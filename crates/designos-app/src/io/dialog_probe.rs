//! Tells a cancelled file dialog from one that could not open.
//!
//! `rfd` answers `None` for both (and `Some(vec![])` from `pick_files` on Linux failure). On Linux a
//! dialog needs `zenity` or an xdg-desktop-portal with a FileChooser backend, and three signals mark a
//! dialog as unavailable:
//! - before calling `rfd`, neither `zenity` nor a live `org.freedesktop.portal.Desktop` owner on the
//!   session D-Bus can answer within [`MAX_DIALOG_OPEN_WAIT`];
//! - the dialog returned in under [`MIN_HUMAN_DIALOG`]: a real dialog stays open until a person acts;
//! - the file-system probe finds neither `zenity` nor a portal backend.

use super::file_dialog::DialogOutcome;
use std::cell::RefCell;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const FILE_CHOOSER_INTERFACE: &str = "org.freedesktop.impl.portal.FileChooser";

/// Upper bound on probing whether a Linux file chooser backend can open (`< 2 s` per AC-2).
pub const MAX_DIALOG_OPEN_WAIT: Duration = Duration::from_millis(1500);

thread_local! {
    static LAST_BACKEND_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
}

pub fn set_last_backend_error(reason: impl Into<String>) {
    LAST_BACKEND_ERROR.with(|slot| *slot.borrow_mut() = Some(reason.into()));
}

pub fn take_last_backend_error() -> Option<String> {
    LAST_BACKEND_ERROR.with(|slot| slot.borrow_mut().take())
}

fn forced_unavailable() -> bool {
    let forced = std::env::var_os("DESIGNOS_FORCE_NO_FILE_DIALOG").is_some_and(|v| !v.is_empty() && v != OsStr::new("0"));
    if forced {
        set_last_backend_error("forced unavailable via DESIGNOS_FORCE_NO_FILE_DIALOG");
    }
    forced
}

/// Runs `show` and maps what it returned to an outcome. `backend_present` runs only when nothing was picked.
pub fn classify<T>(show: impl FnOnce() -> Option<T>, clock: impl Fn() -> Instant, backend_present: impl FnOnce() -> bool) -> DialogOutcome<T> {
    let started = clock();
    let picked = show();
    let elapsed = clock().saturating_duration_since(started);
    match picked {
        Some(value) => DialogOutcome::Picked(value),
        // Two signals, either one is enough: no human acts that fast, or nothing on disk could show a dialog.
        // The first also catches a portal file left behind with no service running.
        None if elapsed < MIN_HUMAN_DIALOG || !backend_present() => DialogOutcome::Unavailable,
        None => DialogOutcome::Cancelled,
    }
}

/// Checks within `timeout` that a chooser backend exists on disk and can answer on the session bus.
pub fn check_chooser_ready_bounded(
    env: &ProbeEnv,
    portal_check: impl FnOnce() -> Result<(), String> + Send + 'static,
    timeout: Duration,
) -> Result<(), String> {
    if !env.backend_present() {
        return Err("neither xdg-desktop-portal FileChooser nor zenity found on disk".to_string());
    }
    let has_zenity = env.has_zenity();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(portal_check());
    });
    match rx.recv_timeout(timeout) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(err)) if has_zenity && err.starts_with("no D-Bus session bus") => Ok(()),
        Ok(Err(err)) => Err(err),
        Err(mpsc::RecvTimeoutError::Timeout) => Err(format!("xdg-desktop-portal check timed out after {} ms", timeout.as_millis())),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err("xdg-desktop-portal readiness check failed unexpectedly".to_string()),
    }
}

/// Runs `portal_check` with a bounded wait before calling `show`, then classifies the result.
pub fn dialog_outcome_bounded<T>(
    portal_check: impl FnOnce() -> Result<(), String> + Send + 'static,
    timeout: Duration,
    show: impl FnOnce() -> Option<T>,
    clock: impl Fn() -> Instant,
    env: &ProbeEnv,
) -> DialogOutcome<T> {
    if let Err(reason) = check_chooser_ready_bounded(env, portal_check, timeout) {
        set_last_backend_error(reason);
        return DialogOutcome::Unavailable;
    }
    let outcome = classify(show, clock, || env.backend_present());
    if matches!(outcome, DialogOutcome::Unavailable) {
        LAST_BACKEND_ERROR.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.is_none() {
                *slot = Some("file dialog closed in under 300 ms without opening a chooser".to_string());
            }
        });
    }
    outcome
}

/// A dialog that returns nothing sooner than this never showed: a person cannot see it and cancel it that fast.
pub const MIN_HUMAN_DIALOG: Duration = Duration::from_millis(300);

/// Runs a native file dialog and tells a cancel from a dialog that could not open. Only Linux can lack a
/// dialog, so everywhere else a dialog that returns nothing is a cancel (unless forced via test hook).
pub fn dialog_outcome<T>(show: impl FnOnce() -> Option<T>) -> DialogOutcome<T> {
    if forced_unavailable() {
        return DialogOutcome::Unavailable;
    }
    if !cfg!(target_os = "linux") {
        return show().map_or(DialogOutcome::Cancelled, DialogOutcome::Picked);
    }
    let env = ProbeEnv::from_process();
    dialog_outcome_bounded(linux_portal_ready, MAX_DIALOG_OPEN_WAIT, show, Instant::now, &env)
}

#[cfg(not(target_os = "linux"))]
fn linux_portal_ready() -> Result<(), String> {
    Ok(())
}

/// Verifies on the Linux session D-Bus (with a 500 ms per-call bound and no auto-start) that
/// `org.freedesktop.portal.Desktop` has an active owner and exposes `org.freedesktop.portal.FileChooser`.
#[cfg(target_os = "linux")]
fn linux_portal_ready() -> Result<(), String> {
    use std::ffi::{c_char, c_int, c_uint, c_void, CStr};

    #[repr(C)]
    struct DBusError {
        name: *const c_char,
        message: *const c_char,
        dummy: c_uint,
        padding: *const c_void,
    }

    const DBUS_BUS_SESSION: c_int = 0;
    const DBUS_TYPE_STRING: c_int = b's' as c_int;
    const DBUS_TYPE_BOOLEAN: c_int = b'b' as c_int;
    const DBUS_TYPE_INVALID: c_int = 0;
    const CALL_TIMEOUT_MS: c_int = 500;

    unsafe {
        let mut handle = std::ptr::null_mut();
        for candidate in [c"libdbus-1.so.3", c"libdbus-1.so"] {
            handle = libc::dlopen(candidate.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
            if !handle.is_null() {
                break;
            }
        }
        if handle.is_null() {
            return Err("libdbus-1.so.3 not available".to_string());
        }

        macro_rules! sym {
            ($name:literal, $ty:ty) => {{
                let c = CStr::from_bytes_with_nul_unchecked(concat!($name, "\0").as_bytes());
                let ptr = libc::dlsym(handle, c.as_ptr());
                if ptr.is_null() {
                    libc::dlclose(handle);
                    return Err(concat!("missing symbol in libdbus: ", $name).to_string());
                }
                std::mem::transmute::<*mut c_void, $ty>(ptr)
            }};
        }

        let dbus_error_init = sym!("dbus_error_init", unsafe extern "C" fn(*mut DBusError));
        let dbus_error_free = sym!("dbus_error_free", unsafe extern "C" fn(*mut DBusError));
        let dbus_bus_get_private = sym!("dbus_bus_get_private", unsafe extern "C" fn(c_int, *mut DBusError) -> *mut c_void);
        let dbus_connection_set_exit_on_disconnect = sym!("dbus_connection_set_exit_on_disconnect", unsafe extern "C" fn(*mut c_void, c_uint));
        let dbus_connection_close = sym!("dbus_connection_close", unsafe extern "C" fn(*mut c_void));
        let dbus_connection_unref = sym!("dbus_connection_unref", unsafe extern "C" fn(*mut c_void));
        let dbus_message_new_method_call =
            sym!("dbus_message_new_method_call", unsafe extern "C" fn(*const c_char, *const c_char, *const c_char, *const c_char) -> *mut c_void);
        let dbus_message_set_no_start = sym!("dbus_message_set_no_start", unsafe extern "C" fn(*mut c_void, c_uint));
        let dbus_message_append_args = sym!("dbus_message_append_args", unsafe extern "C" fn(*mut c_void, c_int, ...) -> c_uint);
        let dbus_message_get_args = sym!("dbus_message_get_args", unsafe extern "C" fn(*mut c_void, *mut DBusError, c_int, ...) -> c_uint);
        let dbus_message_unref = sym!("dbus_message_unref", unsafe extern "C" fn(*mut c_void));
        let dbus_connection_send_with_reply_and_block =
            sym!("dbus_connection_send_with_reply_and_block", unsafe extern "C" fn(*mut c_void, *mut c_void, c_int, *mut DBusError) -> *mut c_void);

        let err_string = |err: &DBusError, fallback: &str| -> String {
            if !err.message.is_null() {
                CStr::from_ptr(err.message).to_string_lossy().into_owned()
            } else {
                fallback.to_string()
            }
        };

        let mut err = std::mem::zeroed::<DBusError>();
        dbus_error_init(&mut err);
        let conn = dbus_bus_get_private(DBUS_BUS_SESSION, &mut err);
        if conn.is_null() {
            let msg = format!("no D-Bus session bus: {}", err_string(&err, "connection failed"));
            dbus_error_free(&mut err);
            libc::dlclose(handle);
            return Err(msg);
        }
        dbus_connection_set_exit_on_disconnect(conn, 0);

        let cleanup = |conn: *mut c_void, err: &mut DBusError| {
            dbus_error_free(err);
            dbus_connection_close(conn);
            dbus_connection_unref(conn);
            libc::dlclose(handle);
        };

        // 1. Check that org.freedesktop.portal.Desktop has an active owner on the session bus.
        let msg = dbus_message_new_method_call(
            c"org.freedesktop.DBus".as_ptr(),
            c"/org/freedesktop/DBus".as_ptr(),
            c"org.freedesktop.DBus".as_ptr(),
            c"NameHasOwner".as_ptr(),
        );
        if msg.is_null() {
            cleanup(conn, &mut err);
            return Err("cannot allocate D-Bus NameHasOwner message".to_string());
        }
        let portal_name = c"org.freedesktop.portal.Desktop".as_ptr();
        if dbus_message_append_args(msg, DBUS_TYPE_STRING, &portal_name, DBUS_TYPE_INVALID) == 0 {
            dbus_message_unref(msg);
            cleanup(conn, &mut err);
            return Err("cannot encode D-Bus NameHasOwner arguments".to_string());
        }
        let reply = dbus_connection_send_with_reply_and_block(conn, msg, CALL_TIMEOUT_MS, &mut err);
        dbus_message_unref(msg);
        if reply.is_null() {
            let m = format!("org.freedesktop.DBus.NameHasOwner failed: {}", err_string(&err, "no reply"));
            cleanup(conn, &mut err);
            return Err(m);
        }
        let mut owned: c_uint = 0;
        let ok = dbus_message_get_args(reply, &mut err, DBUS_TYPE_BOOLEAN, &mut owned, DBUS_TYPE_INVALID);
        dbus_message_unref(reply);
        if ok == 0 || owned == 0 {
            cleanup(conn, &mut err);
            return Err("org.freedesktop.portal.Desktop has no owner on the session bus".to_string());
        }

        // 2. Check that org.freedesktop.portal.FileChooser is exposed and responsive (no auto-start).
        let prop_msg = dbus_message_new_method_call(
            c"org.freedesktop.portal.Desktop".as_ptr(),
            c"/org/freedesktop/portal/desktop".as_ptr(),
            c"org.freedesktop.DBus.Properties".as_ptr(),
            c"Get".as_ptr(),
        );
        if prop_msg.is_null() {
            cleanup(conn, &mut err);
            return Err("cannot allocate D-Bus Properties.Get message".to_string());
        }
        dbus_message_set_no_start(prop_msg, 1);
        let iface = c"org.freedesktop.portal.FileChooser".as_ptr();
        let prop = c"version".as_ptr();
        if dbus_message_append_args(prop_msg, DBUS_TYPE_STRING, &iface, DBUS_TYPE_STRING, &prop, DBUS_TYPE_INVALID) == 0 {
            dbus_message_unref(prop_msg);
            cleanup(conn, &mut err);
            return Err("cannot encode D-Bus Properties.Get arguments".to_string());
        }
        let prop_reply = dbus_connection_send_with_reply_and_block(conn, prop_msg, CALL_TIMEOUT_MS, &mut err);
        dbus_message_unref(prop_msg);
        if prop_reply.is_null() {
            let m = format!("org.freedesktop.portal.FileChooser unavailable on org.freedesktop.portal.Desktop: {}", err_string(&err, "no reply"));
            cleanup(conn, &mut err);
            return Err(m);
        }
        dbus_message_unref(prop_reply);
        cleanup(conn, &mut err);
        Ok(())
    }
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

    pub fn has_zenity(&self) -> bool {
        self.path_dirs.iter().any(|dir| is_executable(&dir.join("zenity")))
    }

    pub fn has_portal(&self) -> bool {
        self.data_dirs.iter().any(|dir| has_file_chooser_portal(dir))
    }

    pub fn backend_present(&self) -> bool {
        self.sandboxed || self.has_zenity() || self.has_portal()
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

    /// A clock that reads `start` first and `start + elapsed` after, so a dialog appears to take `elapsed`.
    fn clock_taking(elapsed: Duration) -> impl Fn() -> Instant {
        let start = Instant::now();
        let reads = std::cell::Cell::new(0u32);
        move || {
            reads.set(reads.get() + 1);
            if reads.get() == 1 {
                start
            } else {
                start + elapsed
            }
        }
    }

    const FAST: Duration = Duration::from_millis(50);
    const HUMAN: Duration = Duration::from_secs(2);

    #[test]
    fn nothing_picked_and_no_backend_is_unavailable() {
        assert_eq!(classify::<PathBuf>(|| None, clock_taking(HUMAN), || false), DialogOutcome::Unavailable);
    }

    #[test]
    fn nothing_picked_after_a_person_could_have_acted_with_a_backend_is_a_cancel() {
        assert_eq!(classify::<PathBuf>(|| None, clock_taking(HUMAN), || true), DialogOutcome::Cancelled);
    }

    #[test]
    fn nothing_picked_at_once_is_unavailable_even_when_the_probe_sees_a_backend() {
        // A stale portal file with no running service answers instantly; nobody cancels a dialog in 50 ms.
        assert_eq!(classify::<PathBuf>(|| None, clock_taking(FAST), || true), DialogOutcome::Unavailable);
    }

    #[test]
    fn the_cutoff_is_exact() {
        let just_under = MIN_HUMAN_DIALOG - Duration::from_millis(1);
        assert_eq!(classify::<PathBuf>(|| None, clock_taking(just_under), || true), DialogOutcome::Unavailable);
        assert_eq!(classify::<PathBuf>(|| None, clock_taking(MIN_HUMAN_DIALOG), || true), DialogOutcome::Cancelled);
    }

    #[test]
    fn a_pick_is_a_pick_however_fast() {
        assert_eq!(classify(|| Some(PathBuf::from("/a.png")), clock_taking(FAST), || false), DialogOutcome::Picked(PathBuf::from("/a.png")));
    }

    #[test]
    fn a_pick_never_runs_the_probe() {
        let picked = classify(|| Some(PathBuf::from("/a.png")), clock_taking(HUMAN), || panic!("the probe must not run after a pick"));
        assert_eq!(picked, DialogOutcome::Picked(PathBuf::from("/a.png")));
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn off_linux_a_dialog_that_returns_nothing_is_always_a_cancel() {
        assert_eq!(dialog_outcome::<PathBuf>(|| None), DialogOutcome::Cancelled);
        assert_eq!(dialog_outcome(|| Some(PathBuf::from("/a.png"))), DialogOutcome::Picked(PathBuf::from("/a.png")));
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

    #[test]
    fn portal_without_session_bus_owner_is_unavailable_and_records_backend_error() {
        let root = scratch("portal_no_owner");
        write_portal(&root, "gtk.portal", "[portal]\nDBusName=org.freedesktop.impl.portal.desktop.gtk\nInterfaces=org.freedesktop.impl.portal.FileChooser;\n");
        let env = env_in(&root);
        let out = dialog_outcome_bounded::<PathBuf>(
            || Err("org.freedesktop.portal.Desktop has no owner on the session bus".to_string()),
            Duration::from_millis(100),
            || panic!("show must not run when portal has no owner and zenity is absent"),
            clock_taking(HUMAN),
            &env,
        );
        assert_eq!(out, DialogOutcome::Unavailable);
        assert_eq!(take_last_backend_error().as_deref(), Some("org.freedesktop.portal.Desktop has no owner on the session bus"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn hanging_portal_check_times_out_within_bound_and_never_calls_show() {
        let root = scratch("portal_hang");
        let env = ProbeEnv { sandboxed: true, ..env_in(&root) };
        let started = Instant::now();
        let out = dialog_outcome_bounded::<PathBuf>(
            || {
                std::thread::sleep(Duration::from_millis(350));
                Ok(())
            },
            Duration::from_millis(80),
            || panic!("show must not run when portal readiness check times out"),
            clock_taking(HUMAN),
            &env,
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        assert_eq!(out, DialogOutcome::Unavailable);
        let err = take_last_backend_error().unwrap_or_default();
        assert!(err.contains("timed out"), "unexpected error: {err}");
        std::fs::remove_dir_all(&root).ok();
    }
}
