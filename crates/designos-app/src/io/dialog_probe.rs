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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

const FILE_CHOOSER_INTERFACE: &str = "org.freedesktop.impl.portal.FileChooser";
const PORTAL_BUS_NAME: &str = "org.freedesktop.portal.Desktop";
const PORTAL_OBJECT_PATH: &str = "/org/freedesktop/portal/desktop";
const PORTAL_FILE_CHOOSER_INTERFACE: &str = "org.freedesktop.portal.FileChooser";

/// Upper bound on probing whether a Linux file chooser backend can open (`< 2 s` per AC-2).
pub const MAX_DIALOG_OPEN_WAIT: Duration = Duration::from_millis(1500);

thread_local! {
    static LAST_BACKEND_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
}

#[cfg(not(test))]
static PROBE_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
thread_local! {
    static TEST_PROBE_IN_FLIGHT: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
}

struct ProbeInFlightGuard {
    #[cfg(test)]
    flag: Arc<AtomicBool>,
}

impl ProbeInFlightGuard {
    fn try_acquire() -> Option<Self> {
        #[cfg(not(test))]
        {
            PROBE_IN_FLIGHT.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).ok().map(|_| Self {})
        }
        #[cfg(test)]
        {
            let flag = TEST_PROBE_IN_FLIGHT.with(Arc::clone);
            flag.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).ok().map(|_| Self { flag })
        }
    }
}

impl Drop for ProbeInFlightGuard {
    fn drop(&mut self) {
        #[cfg(not(test))]
        PROBE_IN_FLIGHT.store(false, Ordering::Release);
        #[cfg(test)]
        self.flag.store(false, Ordering::Release);
    }
}

#[cfg(test)]
pub(crate) fn wait_for_test_probe_idle() {
    let flag = TEST_PROBE_IN_FLIGHT.with(Arc::clone);
    let deadline = Instant::now() + Duration::from_secs(2);
    while flag.load(Ordering::Acquire) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[derive(Default)]
struct ProbeCancel {
    state: Mutex<(bool, Option<std::task::Waker>)>,
}

impl ProbeCancel {
    fn cancel(&self) {
        if let Ok(mut guard) = self.state.lock() {
            guard.0 = true;
            if let Some(waker) = guard.1.take() {
                waker.wake();
            }
        }
    }

    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    async fn run<F, T>(&self, fut: F) -> Result<T, String>
    where
        F: std::future::Future<Output = Result<T, String>>,
    {
        let mut fut = std::pin::pin!(fut);
        std::future::poll_fn(|cx| {
            if let Ok(mut guard) = self.state.lock() {
                if guard.0 {
                    return std::task::Poll::Ready(Err("Input/output error: timed out".to_string()));
                }
                guard.1 = Some(cx.waker().clone());
            }
            fut.as_mut().poll(cx)
        })
        .await
    }
}

static ACTIVE_PROBE_CANCEL: Mutex<Option<Arc<ProbeCancel>>> = Mutex::new(None);

fn install_probe_cancel() -> Arc<ProbeCancel> {
    let cancel = Arc::new(ProbeCancel::default());
    if let Ok(mut slot) = ACTIVE_PROBE_CANCEL.lock() {
        *slot = Some(Arc::clone(&cancel));
    }
    cancel
}

fn cancel_active_probe() {
    if let Ok(mut slot) = ACTIVE_PROBE_CANCEL.lock() {
        if let Some(cancel) = slot.take() {
            cancel.cancel();
        }
    }
}

pub fn set_last_backend_error(reason: impl Into<String>) {
    LAST_BACKEND_ERROR.with(|slot| *slot.borrow_mut() = Some(reason.into()));
}

pub fn take_last_backend_error() -> Option<String> {
    LAST_BACKEND_ERROR.with(|slot| slot.borrow_mut().take())
}

fn forced_unavailable() -> bool {
    #[cfg(any(debug_assertions, test))]
    let raw = std::env::var_os("DESIGNOS_FORCE_NO_FILE_DIALOG");
    #[cfg(not(any(debug_assertions, test)))]
    let raw: Option<std::ffi::OsString> = None;
    let forced = forced_unavailable_in(cfg!(any(debug_assertions, test)), raw.as_deref());
    if forced {
        set_last_backend_error("forced unavailable via DESIGNOS_FORCE_NO_FILE_DIALOG");
    }
    forced
}

pub(crate) fn forced_unavailable_in(debug_or_test: bool, raw: Option<&OsStr>) -> bool {
    debug_or_test && raw.is_some_and(|v| !v.is_empty() && v != OsStr::new("0"))
}

fn is_probe_timeout(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("org.freedesktop.dbus.error.noreply")
        || lower.contains("org.freedesktop.dbus.error.timedout")
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
    let Some(slot_guard) = ProbeInFlightGuard::try_acquire() else {
        return Err("previous portal check still running".to_string());
    };
    let has_zenity = env.has_zenity();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _guard = slot_guard;
        let _ = tx.send(portal_check());
    });
    match rx.recv_timeout(timeout) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(err)) if is_probe_timeout(&err) => Err(err),
        Ok(Err(_)) if has_zenity => Ok(()),
        Ok(Err(err)) => Err(err),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            cancel_active_probe();
            Err(format!("xdg-desktop-portal check timed out after {} ms", timeout.as_millis()))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) if has_zenity => Ok(()),
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
    let cancel = install_probe_cancel();
    dialog_outcome_bounded(move || linux_portal_ready(&cancel), MAX_DIALOG_OPEN_WAIT, show, Instant::now, &env)
}

/// Maps the Linux session-bus portal readiness check into a single result:
/// 1. connecting to the session bus;
/// 2. checking `org.freedesktop.DBus.Properties.Get("org.freedesktop.portal.FileChooser", "version")`
///    with D-Bus auto-start allowed so an installed-but-not-yet-running portal activates on first use;
/// 3. using `org.freedesktop.DBus.NameHasOwner("org.freedesktop.portal.Desktop")` only to word the error
///    when the FileChooser call fails.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn map_portal_probe<C>(
    connect_session: impl FnOnce() -> Result<C, String>,
    name_has_owner: impl FnOnce(&C, &str) -> Result<bool, String>,
    file_chooser_version: impl FnOnce(&C, &str, &str, &str) -> Result<(), String>,
) -> Result<(), String> {
    let conn = connect_session().map_err(|e| format!("no D-Bus session bus: {e}"))?;
    match file_chooser_version(&conn, PORTAL_BUS_NAME, PORTAL_OBJECT_PATH, PORTAL_FILE_CHOOSER_INTERFACE) {
        Ok(()) => Ok(()),
        Err(e) if is_probe_timeout(&e) => Err(format!("{PORTAL_FILE_CHOOSER_INTERFACE} unavailable on {PORTAL_BUS_NAME}: {e}")),
        Err(e) => match name_has_owner(&conn, PORTAL_BUS_NAME) {
            Ok(false) => Err(format!("{PORTAL_BUS_NAME} has no owner on the session bus")),
            Ok(true) => Err(format!("{PORTAL_FILE_CHOOSER_INTERFACE} unavailable on {PORTAL_BUS_NAME}: {e}")),
            Err(owner_err) => {
                Err(format!("{PORTAL_FILE_CHOOSER_INTERFACE} unavailable on {PORTAL_BUS_NAME}: {e} (org.freedesktop.DBus.NameHasOwner failed: {owner_err})"))
            }
        },
    }
}

#[cfg(not(target_os = "linux"))]
fn linux_portal_ready(_cancel: &ProbeCancel) -> Result<(), String> {
    Ok(())
}

/// Verifies on the Linux session D-Bus (bounded by the remaining budget of [`MAX_DIALOG_OPEN_WAIT`] and
/// with D-Bus auto-start allowed) that `org.freedesktop.portal.Desktop` exposes
/// `org.freedesktop.portal.FileChooser`.
#[cfg(target_os = "linux")]
fn linux_portal_ready(cancel: &ProbeCancel) -> Result<(), String> {
    use zbus::zvariant::OwnedValue;

    let deadline = Instant::now() + MAX_DIALOG_OPEN_WAIT;
    let remaining_budget = || -> Result<Duration, String> {
        let rem = deadline.saturating_duration_since(Instant::now());
        if rem.is_zero() {
            Err("Input/output error: timed out".to_string())
        } else {
            Ok(rem)
        }
    };

    map_portal_probe::<zbus::Connection>(
        || {
            let budget = remaining_budget()?;
            zbus::block_on(cancel.run(async {
                zbus::connection::Builder::session().map_err(|e| e.to_string())?.method_timeout(budget).build().await.map_err(|e| e.to_string())
            }))
        },
        |conn: &zbus::Connection, bus_name| {
            let _budget = remaining_budget()?;
            zbus::block_on(cancel.run(async {
                let reply = conn
                    .call_method(Some("org.freedesktop.DBus"), "/org/freedesktop/DBus", Some("org.freedesktop.DBus"), "NameHasOwner", &(bus_name,))
                    .await
                    .map_err(|e| e.to_string())?;
                reply.body().deserialize::<bool>().map_err(|e| e.to_string())
            }))
        },
        |conn: &zbus::Connection, bus_name, object_path, iface| {
            let _budget = remaining_budget()?;
            zbus::block_on(cancel.run(async {
                let reply = conn
                    .call_method(Some(bus_name), object_path, Some("org.freedesktop.DBus.Properties"), "Get", &(iface, "version"))
                    .await
                    .map_err(|e| e.to_string())?;
                let _version: OwnedValue = reply.body().deserialize().map_err(|e| e.to_string())?;
                Ok(())
            }))
        },
    )
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
            || map_portal_probe(|| Ok(()), |(), _| Ok(false), |(), _, _, _| Err("org.freedesktop.DBus.Error.ServiceUnknown".into())),
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
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let started = Instant::now();
        let out = dialog_outcome_bounded::<PathBuf>(
            move || {
                let _ = release_rx.recv_timeout(Duration::from_secs(2));
                Ok(())
            },
            Duration::from_millis(80),
            || panic!("show must not run when portal readiness check times out"),
            clock_taking(HUMAN),
            &env,
        );
        let _ = release_tx.send(());
        wait_for_test_probe_idle();
        assert!(started.elapsed() < Duration::from_secs(2));
        assert_eq!(out, DialogOutcome::Unavailable);
        let err = take_last_backend_error().unwrap_or_default();
        assert!(err.contains("timed out"), "unexpected error: {err}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn portal_probe_result_mapping_covers_no_owner_error_timeout_and_ready() {
        let root = scratch("portal_mapping");
        write_portal(&root, "gtk.portal", "[portal]\nDBusName=org.freedesktop.impl.portal.desktop.gtk\nInterfaces=org.freedesktop.impl.portal.FileChooser;\n");
        let env = env_in(&root);

        // 1. No owner and not activatable: FileChooser fails and NameHasOwner returns Ok(false).
        let no_owner = dialog_outcome_bounded::<PathBuf>(
            || map_portal_probe(|| Ok(()), |(), _| Ok(false), |(), _, _, _| Err("org.freedesktop.DBus.Error.ServiceUnknown".into())),
            Duration::from_millis(100),
            || panic!("show must not run when portal has no owner"),
            clock_taking(HUMAN),
            &env,
        );
        assert_eq!(no_owner, DialogOutcome::Unavailable);
        assert_eq!(take_last_backend_error().as_deref(), Some("org.freedesktop.portal.Desktop has no owner on the session bus"));

        // 2a. Error on session bus connection (no zenity): Unavailable with session-bus error.
        let bus_err = dialog_outcome_bounded::<PathBuf>(
            || map_portal_probe(|| Err::<(), _>("address not set".into()), |(), _| unreachable!(), |(), _, _, _| unreachable!()),
            Duration::from_millis(100),
            || panic!("show must not run on session bus error without zenity"),
            clock_taking(HUMAN),
            &env,
        );
        assert_eq!(bus_err, DialogOutcome::Unavailable);
        assert_eq!(take_last_backend_error().as_deref(), Some("no D-Bus session bus: address not set"));

        // 2b. Error on FileChooser and NameHasOwner: Unavailable with NameHasOwner error included.
        let owner_err = dialog_outcome_bounded::<PathBuf>(
            || map_portal_probe(|| Ok(()), |(), _| Err("bus disconnected".into()), |(), _, _, _| Err("ServiceUnknown".into())),
            Duration::from_millis(100),
            || panic!("show must not run on NameHasOwner error"),
            clock_taking(HUMAN),
            &env,
        );
        assert_eq!(owner_err, DialogOutcome::Unavailable);
        assert!(take_last_backend_error().unwrap_or_default().contains("org.freedesktop.DBus.NameHasOwner failed: bus disconnected"));

        // 2c. Error on FileChooser Properties.Get: Unavailable with FileChooser error.
        let chooser_err = dialog_outcome_bounded::<PathBuf>(
            || map_portal_probe(|| Ok(()), |(), _| Ok(true), |(), _, _, _| Err("unknown interface".into())),
            Duration::from_millis(100),
            || panic!("show must not run on FileChooser property error"),
            clock_taking(HUMAN),
            &env,
        );
        assert_eq!(chooser_err, DialogOutcome::Unavailable);
        assert_eq!(
            take_last_backend_error().as_deref(),
            Some("org.freedesktop.portal.FileChooser unavailable on org.freedesktop.portal.Desktop: unknown interface")
        );

        // 3. Timeout: portal probe exceeds timeout bound.
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let timed_out = dialog_outcome_bounded::<PathBuf>(
            move || {
                let _ = release_rx.recv_timeout(Duration::from_secs(2));
                map_portal_probe(|| Ok(()), |(), _| Ok(true), |(), _, _, _| Ok(()))
            },
            Duration::from_millis(50),
            || panic!("show must not run on portal timeout"),
            clock_taking(HUMAN),
            &env,
        );
        let _ = release_tx.send(());
        wait_for_test_probe_idle();
        assert_eq!(timed_out, DialogOutcome::Unavailable);
        assert!(take_last_backend_error().unwrap_or_default().contains("timed out"));

        // 4. Ready: session bus connects, FileChooser responds -> show() runs.
        let ready_picked = dialog_outcome_bounded(
            || map_portal_probe(|| Ok(()), |(), _| unreachable!("NameHasOwner not needed when FileChooser responds"), |(), _, _, _| Ok(())),
            Duration::from_millis(100),
            || Some(PathBuf::from("/tmp/ready.fig")),
            clock_taking(FAST),
            &env,
        );
        assert_eq!(ready_picked, DialogOutcome::Picked(PathBuf::from("/tmp/ready.fig")));
        assert_eq!(take_last_backend_error(), None);

        std::fs::remove_dir_all(&root).ok();
    }

    #[cfg(unix)]
    #[test]
    fn round3_a_zenity_with_no_owner_or_service_unknown_runs_show() {
        let _ = take_last_backend_error();
        let root = scratch("round3_a_zenity_no_owner");
        write_zenity(&root, 0o755);
        let env = env_in(&root);

        for probe_err in [
            "org.freedesktop.portal.Desktop has no owner on the session bus",
            "org.freedesktop.portal.FileChooser unavailable on org.freedesktop.portal.Desktop: org.freedesktop.DBus.Error.ServiceUnknown: The name org.freedesktop.portal.Desktop was not provided by any .service files",
        ] {
            let _ = take_last_backend_error();
            let show_called = std::cell::Cell::new(false);
            let outcome = dialog_outcome_bounded(
                || Err(probe_err.to_string()),
                Duration::from_millis(100),
                || {
                    show_called.set(true);
                    Some(PathBuf::from("/tmp/zenity-fallback.fig"))
                },
                clock_taking(FAST),
                &env,
            );
            assert!(
                show_called.get(),
                "expected show (rfd -> zenity) to run for probe error: {probe_err}"
            );
            assert_eq!(outcome, DialogOutcome::Picked(PathBuf::from("/tmp/zenity-fallback.fig")));
            assert_eq!(take_last_backend_error(), None);
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn round3_b_no_zenity_with_portal_activating_within_budget_runs_show() {
        let _ = take_last_backend_error();
        let root = scratch("round3_b_portal_activates");
        write_portal(&root, "gtk.portal", "[portal]\nInterfaces=org.freedesktop.impl.portal.FileChooser\n");
        let env = env_in(&root);

        let activated = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let activated_in_probe = std::sync::Arc::clone(&activated);
        let show_called = std::cell::Cell::new(false);
        let outcome = dialog_outcome_bounded(
            move || {
                map_portal_probe(
                    || Ok(()),
                    |(), _| Ok(false),
                    |(), _, _, _| {
                        activated_in_probe.store(true, std::sync::atomic::Ordering::SeqCst);
                        Ok(())
                    },
                )
            },
            Duration::from_millis(200),
            || {
                show_called.set(true);
                Some(PathBuf::from("/tmp/activated-portal.fig"))
            },
            clock_taking(FAST),
            &env,
        );
        assert!(activated.load(std::sync::atomic::Ordering::SeqCst), "FileChooser version call must run with auto-start even when NameHasOwner was false");
        assert!(show_called.get(), "show must run when portal activates in time");
        assert_eq!(outcome, DialogOutcome::Picked(PathBuf::from("/tmp/activated-portal.fig")));
        assert_eq!(take_last_backend_error(), None);

        std::fs::remove_dir_all(&root).ok();
    }

    #[cfg(unix)]
    #[test]
    fn round3_c_zenity_with_probe_timeout_is_unavailable_and_never_runs_show() {
        let _ = take_last_backend_error();
        let root = scratch("round3_c_zenity_timeout");
        write_zenity(&root, 0o755);
        let env = env_in(&root);

        // 1. A session-bus or FileChooser method_timeout inside the probe returns a timed-out
        // error string; even with zenity on PATH, a timeout must stay Unavailable and never run show.
        let show_called = std::cell::Cell::new(false);
        let outcome = dialog_outcome_bounded(
            || Err("no D-Bus session bus: Input/output error: timed out".to_string()),
            Duration::from_millis(100),
            || {
                show_called.set(true);
                Some(PathBuf::from("/tmp/should-not-open.fig"))
            },
            clock_taking(HUMAN),
            &env,
        );
        assert_eq!(outcome, DialogOutcome::Unavailable);
        assert!(!show_called.get(), "show must never run when the portal probe times out, even with zenity installed");
        let err = take_last_backend_error().expect("timeout error recorded");
        assert!(err.contains("timed out"), "unexpected error: {err}");

        // 2. Caller-side recv_timeout when the probe thread exceeds the budget.
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let show_called_2 = std::cell::Cell::new(false);
        let outcome_2 = dialog_outcome_bounded(
            move || {
                let _ = release_rx.recv_timeout(Duration::from_secs(2));
                Ok(())
            },
            Duration::from_millis(25),
            || {
                show_called_2.set(true);
                Some(PathBuf::from("/tmp/should-not-open-2.fig"))
            },
            clock_taking(HUMAN),
            &env,
        );
        let _ = release_tx.send(());
        wait_for_test_probe_idle();
        assert_eq!(outcome_2, DialogOutcome::Unavailable);
        assert!(!show_called_2.get());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn round3_d_hanging_probe_thread_spawns_no_second_thread_and_reports_still_running() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let _ = take_last_backend_error();
        let root = scratch("round3_d_single_probe_thread");
        write_portal(&root, "gtk.portal", "[portal]\nInterfaces=org.freedesktop.impl.portal.FileChooser\n");
        let env = env_in(&root);

        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let spawned_threads = Arc::new(AtomicUsize::new(0));
        let spawned_1 = Arc::clone(&spawned_threads);

        let first = dialog_outcome_bounded(
            move || {
                spawned_1.fetch_add(1, Ordering::SeqCst);
                let _ = release_rx.recv_timeout(Duration::from_secs(2));
                Ok(())
            },
            Duration::from_millis(25),
            || Some(PathBuf::from("/tmp/never-1.fig")),
            clock_taking(HUMAN),
            &env,
        );
        assert_eq!(first, DialogOutcome::Unavailable);
        assert_eq!(spawned_threads.load(Ordering::SeqCst), 1);
        let _ = take_last_backend_error();

        let spawned_2 = Arc::clone(&spawned_threads);
        let show_called = std::cell::Cell::new(false);
        let second = dialog_outcome_bounded(
            move || {
                spawned_2.fetch_add(1, Ordering::SeqCst);
                Ok(())
            },
            Duration::from_millis(25),
            || {
                show_called.set(true);
                Some(PathBuf::from("/tmp/never-2.fig"))
            },
            clock_taking(HUMAN),
            &env,
        );
        assert_eq!(second, DialogOutcome::Unavailable);
        assert!(!show_called.get());
        assert_eq!(spawned_threads.load(Ordering::SeqCst), 1, "hanging first probe must prevent spawning a second probe thread");
        assert_eq!(take_last_backend_error().as_deref(), Some("previous portal check still running"));

        let _ = release_tx.send(());
        wait_for_test_probe_idle();
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn round3_f_release_build_hook_is_off() {
        assert!(!forced_unavailable_in(false, Some(OsStr::new("1"))), "release build (debug_or_test = false) must ignore DESIGNOS_FORCE_NO_FILE_DIALOG=1");
        assert!(
            !forced_unavailable_in(false, Some(OsStr::new("toast"))),
            "release build (debug_or_test = false) must ignore DESIGNOS_FORCE_NO_FILE_DIALOG=toast"
        );
        assert!(
            !forced_unavailable_in(false, Some(OsStr::new("dialog"))),
            "release build (debug_or_test = false) must ignore DESIGNOS_FORCE_NO_FILE_DIALOG=dialog"
        );
        assert!(forced_unavailable_in(true, Some(OsStr::new("1"))));
        assert!(forced_unavailable_in(true, Some(OsStr::new("toast"))));
        assert!(!forced_unavailable_in(true, Some(OsStr::new("0"))));
        assert!(!forced_unavailable_in(true, Some(OsStr::new(""))));
        assert!(!forced_unavailable_in(true, None));
    }

    #[cfg(unix)]
    #[test]
    fn round4_dbus_error_timedout_with_zenity_is_unavailable_and_never_runs_show() {
        let _ = take_last_backend_error();
        let root = scratch("round4_zenity_dbus_timedout");
        write_zenity(&root, 0o755);
        let env = env_in(&root);

        let show_called = std::cell::Cell::new(false);
        let outcome = dialog_outcome_bounded(
            || Err("org.freedesktop.DBus.Error.TimedOut: no details".to_string()),
            Duration::from_millis(100),
            || {
                show_called.set(true);
                Some(PathBuf::from("/tmp/should-not-open-r4.fig"))
            },
            clock_taking(HUMAN),
            &env,
        );
        assert_eq!(outcome, DialogOutcome::Unavailable);
        assert!(!show_called.get(), "show must never run when the portal probe returns org.freedesktop.DBus.Error.TimedOut, even with zenity installed");
        let err = take_last_backend_error().expect("timeout error recorded");
        assert!(err.to_ascii_lowercase().contains("org.freedesktop.dbus.error.timedout"), "unexpected error: {err}");

        let show_called_via_map = std::cell::Cell::new(false);
        let outcome_via_map = dialog_outcome_bounded(
            || map_portal_probe(|| Ok(()), |(), _| Ok(false), |(), _, _, _| Err("org.freedesktop.DBus.Error.TimedOut: no details".to_string())),
            Duration::from_millis(100),
            || {
                show_called_via_map.set(true);
                Some(PathBuf::from("/tmp/should-not-open-r4-map.fig"))
            },
            clock_taking(HUMAN),
            &env,
        );
        assert_eq!(outcome_via_map, DialogOutcome::Unavailable);
        assert!(!show_called_via_map.get());

        std::fs::remove_dir_all(&root).ok();
    }
}
