//! Hermetic and host CLI resolution (`#1126` rule 5).

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const HERMETIC_MARKER_FILENAME: &str = ".designos-hermetic";
pub const HERMETIC_ACTIVE_FILENAME: &str = ".designos-hermetic-active";

/// Inputs to [`cli_search_dirs_with`], ordered by precedence:
/// 1. `hermetic`: release with `DESIGNOS_AGENT_CLI_DIR` set -> `[DIR]`
/// 2. `scoped`: `cfg(test)` thread-local [`ScopedCliDirs`] guard -> `[stub_dir]`
/// 3. `explicit`: caller's own path list -> `explicit`
/// 4. `host_path` + `extra`: host `PATH` plus `extra_cli_path_dirs()`
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchSources {
    pub hermetic: Option<PathBuf>,
    pub scoped: Option<PathBuf>,
    pub explicit: Option<Vec<PathBuf>>,
    pub host_path: Vec<PathBuf>,
    pub extra: Vec<PathBuf>,
}

#[cfg(not(test))]
static HERMETIC_DIR: OnceLock<Option<PathBuf>> = OnceLock::new();

#[cfg(test)]
thread_local! {
    static TEST_HERMETIC_DIR: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
    static TEST_AGENT_CLI_DIR_ENV: std::cell::RefCell<Option<std::ffi::OsString>> = const { std::cell::RefCell::new(None) };
}

/// Thread-local guard simulating `DESIGNOS_AGENT_CLI_DIR` in `cfg(test)` builds without mutating
/// the process environment. Restores the previous thread-local value on drop.
#[cfg(test)]
pub struct TestAgentCliDirEnv {
    prev: Option<std::ffi::OsString>,
}

#[cfg(test)]
impl TestAgentCliDirEnv {
    pub fn set(dir: impl AsRef<OsStr>) -> Self {
        let next = Some(dir.as_ref().to_os_string());
        let prev = TEST_AGENT_CLI_DIR_ENV.with(|slot| slot.replace(next));
        Self { prev }
    }
}

#[cfg(test)]
impl Drop for TestAgentCliDirEnv {
    fn drop(&mut self) {
        let prev = self.prev.take();
        TEST_AGENT_CLI_DIR_ENV.with(|slot| {
            *slot.borrow_mut() = prev;
        });
    }
}

fn mode_active(pinned: bool, env_dir: Option<&OsStr>) -> bool {
    pinned || env_dir.is_some()
}

fn has_dot_or_dotdot_component(path: &Path) -> bool {
    path.components().any(|c| matches!(c, std::path::Component::ParentDir | std::path::Component::CurDir))
}

/// Validates that `raw_dir` is a non-empty, absolute path with no `.` or `..` components,
/// points to an existing directory, canonicalizes it, and verifies `.designos-hermetic` inside it.
pub fn validate_and_canonicalize_hermetic_dir(raw_dir: &str) -> Result<PathBuf, String> {
    if raw_dir.trim().is_empty() {
        return Err("DESIGNOS_AGENT_CLI_DIR is empty".to_string());
    }
    let dir = Path::new(raw_dir);
    if !dir.is_absolute() {
        return Err(format!("DESIGNOS_AGENT_CLI_DIR ({}) must be an absolute path", dir.display()));
    }
    if has_dot_or_dotdot_component(dir) {
        return Err(format!("DESIGNOS_AGENT_CLI_DIR ({}) must not contain '.' or '..'", dir.display()));
    }
    if !dir.is_dir() {
        return Err(format!("DESIGNOS_AGENT_CLI_DIR ({}) is not an existing directory", dir.display()));
    }
    let canon = std::fs::canonicalize(dir).map_err(|e| format!("DESIGNOS_AGENT_CLI_DIR ({}) cannot be canonicalized: {e}", dir.display()))?;
    if !canon.join(HERMETIC_MARKER_FILENAME).is_file() {
        return Err(format!("DESIGNOS_AGENT_CLI_DIR ({}) is missing {HERMETIC_MARKER_FILENAME}", dir.display()));
    }
    Ok(canon)
}

/// True when hermetic mode was activated at startup or requested via `DESIGNOS_AGENT_CLI_DIR`.
pub fn hermetic_mode_active() -> bool {
    #[cfg(not(test))]
    {
        let pinned = HERMETIC_DIR.get().is_some_and(Option::is_some);
        let env_dir = std::env::var_os("DESIGNOS_AGENT_CLI_DIR");
        mode_active(pinned, env_dir.as_deref())
    }
    #[cfg(test)]
    {
        let pinned = TEST_HERMETIC_DIR.with(|slot| slot.borrow().is_some());
        TEST_AGENT_CLI_DIR_ENV.with(|slot| mode_active(pinned, slot.borrow().as_deref()))
    }
}

/// Returns the canonical `PathBuf` validated and stored in `HERMETIC_DIR` at startup.
/// Never re-reads or re-canonicalizes `DESIGNOS_AGENT_CLI_DIR` after startup.
pub fn hermetic_agent_cli_dir() -> Option<PathBuf> {
    #[cfg(not(test))]
    {
        HERMETIC_DIR.get().and_then(Clone::clone)
    }
    #[cfg(test)]
    {
        TEST_HERMETIC_DIR.with(|slot| slot.borrow().clone())
    }
}

#[cfg(test)]
thread_local! {
    static SCOPED_CLI_DIR: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// Thread-local guard (`#1126` rule 5, precedence 2) entered by the layout gate's `Sim` for the
/// lifetime of a single case. Restores the previous thread-local value on drop.
#[cfg(test)]
pub struct ScopedCliDirs {
    prev: Option<PathBuf>,
}

#[cfg(test)]
impl ScopedCliDirs {
    pub fn enter(dir: impl Into<PathBuf>) -> Self {
        let raw: PathBuf = dir.into();
        let next = Some(std::fs::canonicalize(&raw).unwrap_or(raw));
        let prev = SCOPED_CLI_DIR.with(|slot| slot.replace(next));
        Self { prev }
    }
}

#[cfg(test)]
impl Drop for ScopedCliDirs {
    fn drop(&mut self) {
        let prev = self.prev.take();
        SCOPED_CLI_DIR.with(|slot| {
            *slot.borrow_mut() = prev;
        });
    }
}

fn current_scoped_cli_dir() -> Option<PathBuf> {
    #[cfg(test)]
    {
        SCOPED_CLI_DIR.with(|slot| slot.borrow().clone())
    }
    #[cfg(not(test))]
    {
        None
    }
}

/// Splits an OS path list into non-empty [`PathBuf`] entries.
pub fn split_non_empty_paths(list: &OsStr) -> Vec<PathBuf> {
    std::env::split_paths(list).filter(|p| !p.as_os_str().is_empty()).collect()
}

/// Directories listed in the process's `PATH` environment variable.
pub fn host_path_dirs() -> Vec<PathBuf> {
    std::env::var_os("PATH").as_deref().map(split_non_empty_paths).unwrap_or_default()
}

/// Raw `PATH` string from the process environment (or empty string if unset/non-UTF-8).
pub fn host_path_raw() -> String {
    std::env::var("PATH").unwrap_or_default()
}

/// Raw `PATH` OS string from the process environment.
pub fn host_path_os() -> Option<std::ffi::OsString> {
    std::env::var_os("PATH")
}

/// True when `PATH` is set to the empty string `""`.
pub fn host_path_is_empty() -> bool {
    std::env::var_os("PATH").is_some_and(|p| p.is_empty())
}

/// Builds the child `PATH` string for `agent_cli_catalog`:
/// - In hermetic mode (`hermetic_agent_cli_dir()` is `Some(dir)`): `dir` only.
/// - In scoped test mode (`ScopedCliDirs` is `Some(dir)`): `dir` only.
/// - In non-hermetic mode: `install_folders` + `extra_cli_path_dirs()` prepended to the process `PATH`
///   in the exact legacy order.
pub fn catalog_child_path(install_folders: &[PathBuf]) -> Option<String> {
    if let Some(dir) = hermetic_agent_cli_dir() {
        return Some(dir.to_string_lossy().into_owned());
    }
    if let Some(dir) = current_scoped_cli_dir() {
        return Some(dir.to_string_lossy().into_owned());
    }
    let mut extra_paths: Vec<String> = install_folders.iter().map(|p| p.to_string_lossy().into_owned()).collect();
    extra_paths.extend(crate::terminal::extra_cli_path_dirs().into_iter().map(|p| p.to_string_lossy().into_owned()));
    if extra_paths.is_empty() {
        return None;
    }
    let current_path = host_path_raw();
    let sep = if cfg!(windows) { ";" } else { ":" };
    let combined = if current_path.is_empty() { extra_paths.join(sep) } else { format!("{}{sep}{current_path}", extra_paths.join(sep)) };
    Some(combined)
}

/// Captures the current [`SearchSources`] from the environment, `OnceLock`, and thread-local guard.
pub fn current_search_sources(explicit: Option<Vec<PathBuf>>) -> SearchSources {
    SearchSources {
        hermetic: hermetic_agent_cli_dir(),
        scoped: current_scoped_cli_dir(),
        explicit,
        host_path: host_path_dirs(),
        extra: crate::terminal::extra_cli_path_dirs(),
    }
}

/// Like [`current_search_sources`], prepending `path_prepend` to `host_path` (active only at precedence 4).
pub fn search_sources_with_prepend(path_prepend: &[PathBuf]) -> SearchSources {
    let mut host_path = path_prepend.to_vec();
    host_path.extend(host_path_dirs());
    SearchSources {
        hermetic: hermetic_agent_cli_dir(),
        scoped: current_scoped_cli_dir(),
        #[cfg(test)]
        explicit: Some(path_prepend.to_vec()),
        #[cfg(not(test))]
        explicit: None,
        host_path,
        extra: crate::terminal::extra_cli_path_dirs(),
    }
}

/// Pure precedence resolver (`#1126` rule 5):
/// 1. `hermetic` -> `[dir]`
/// 2. `scoped` -> `[dir]`
/// 3. `explicit` -> `list`
/// 4. `host_path` + `extra`
pub fn cli_search_dirs_with(sources: &SearchSources) -> Vec<PathBuf> {
    if let Some(ref dir) = sources.hermetic {
        return vec![dir.clone()];
    }
    if let Some(ref dir) = sources.scoped {
        return vec![dir.clone()];
    }
    if let Some(ref list) = sources.explicit {
        return list.clone();
    }
    let mut dirs = sources.host_path.clone();
    dirs.extend(sources.extra.iter().cloned());
    dirs
}

/// Wrapper reading `DESIGNOS_AGENT_CLI_DIR`, [`ScopedCliDirs`], `PATH`, and `extra_cli_path_dirs()`.
pub fn cli_search_dirs(explicit: Option<Vec<PathBuf>>) -> Vec<PathBuf> {
    cli_search_dirs_with(&current_search_sources(explicit))
}

/// Hermetic file lookup of `bin` inside `dir`: canonicalizes `dir` and the candidate,
/// and refuses any candidate that is not a regular file directly inside the canonical `dir`.
pub fn resolve_in_hermetic(dir: &Path, bin: &str) -> Option<PathBuf> {
    let trimmed = bin.trim();
    if trimmed.is_empty() || trimmed.contains('/') || trimmed.contains('\\') || trimmed.contains("..") {
        return None;
    }
    let canon_dir = std::fs::canonicalize(dir).ok()?;
    let check_candidate = |cand: PathBuf| -> Option<PathBuf> {
        let canon_cand = std::fs::canonicalize(&cand).ok()?;
        let meta = std::fs::metadata(&canon_cand).ok()?;
        if meta.is_file() && canon_cand.parent() == Some(canon_dir.as_path()) {
            Some(canon_cand)
        } else {
            None
        }
    };
    if let Some(found) = check_candidate(canon_dir.join(trimmed)) {
        return Some(found);
    }
    if cfg!(windows) {
        for ext in ["exe", "cmd", "bat"] {
            if let Some(found) = check_candidate(canon_dir.join(format!("{trimmed}.{ext}"))) {
                return Some(found);
            }
        }
    }
    None
}

/// Pure file lookup of `bin` across `dirs`.
/// When hermetic mode is active (or resolving against a single isolated/stub directory),
/// canonicalizes the candidate and refuses any target outside the canonical directory.
pub fn resolve_in(dirs: &[PathBuf], bin: &str) -> Option<PathBuf> {
    let trimmed = bin.trim();
    if trimmed.is_empty() {
        return None;
    }
    let enforce_containment = hermetic_agent_cli_dir().is_some() || dirs.len() == 1;
    for d in dirs {
        if enforce_containment || d.join(HERMETIC_MARKER_FILENAME).is_file() {
            if let Some(found) = resolve_in_hermetic(d, trimmed) {
                return Some(found);
            }
            continue;
        }
        let cand = d.join(trimmed);
        if cand.is_file() {
            return Some(cand);
        }
        if cfg!(windows) {
            for ext in ["exe", "cmd", "bat"] {
                let win_cand = d.join(format!("{trimmed}.{ext}"));
                if win_cand.is_file() {
                    return Some(win_cand);
                }
            }
        }
    }
    None
}

fn hermetic_startup_check_inner(env: &BTreeMap<String, String>, opts: &crate::cli::Options) -> Result<Option<PathBuf>, String> {
    let Some(raw_dir) = env.get("DESIGNOS_AGENT_CLI_DIR") else {
        return Ok(None);
    };
    let mut missing = Vec::new();
    if opts.screenshot.is_none() {
        missing.push("--screenshot is required".to_string());
    }
    let mut canon_stub_dir = None;
    match validate_and_canonicalize_hermetic_dir(raw_dir) {
        Ok(canon) => canon_stub_dir = Some(canon),
        Err(e) => missing.push(e),
    }
    for var in ["DESIGNOS_LIBRARY", "TMPDIR", "HOME"] {
        match env.get(var).map(String::as_str).filter(|s| !s.trim().is_empty()) {
            None => missing.push(format!("{var} is unset")),
            Some(val) => {
                let dir = Path::new(val);
                if !dir.is_dir() {
                    missing.push(format!("{var} ({}) is not an existing directory", dir.display()));
                } else if !dir.join(HERMETIC_MARKER_FILENAME).is_file() {
                    missing.push(format!("{var} ({}) is missing {HERMETIC_MARKER_FILENAME}", dir.display()));
                }
            }
        }
    }
    if missing.is_empty() {
        Ok(canon_stub_dir)
    } else {
        Err(format!("hermetic agent mode denied: {}", missing.join("; ")))
    }
}

/// Deny-by-default startup check (`#1126` rule 5).
/// When `DESIGNOS_AGENT_CLI_DIR` is present in `env`, requires an absolute canonicalizable path with no `..`,
/// `--screenshot`, and `.designos-hermetic` markers in `DESIGNOS_AGENT_CLI_DIR`, `DESIGNOS_LIBRARY`, `TMPDIR`, and `HOME`.
pub fn hermetic_startup_check(env: &BTreeMap<String, String>, opts: &crate::cli::Options) -> Result<(), String> {
    hermetic_startup_check_inner(env, opts).map(|_| ())
}

/// Writes `$TMPDIR/.designos-hermetic-active` with `pid` and `dir` after [`hermetic_startup_check`] passes.
pub fn write_hermetic_ack(tmpdir: &Path, dir: &Path, pid: u32) -> std::io::Result<PathBuf> {
    let ack_path = tmpdir.join(HERMETIC_ACTIVE_FILENAME);
    let body = format!("pid={pid}\ndir={}\n", dir.display());
    std::fs::write(&ack_path, body.as_bytes())?;
    Ok(ack_path)
}

/// Runs [`hermetic_startup_check`] and, when `DESIGNOS_AGENT_CLI_DIR` is active, stores the validated canonical
/// stub directory in `HERMETIC_DIR` and writes the acknowledgement file with that canonical path.
pub fn enforce_hermetic_startup(env: &BTreeMap<String, String>, opts: &crate::cli::Options, pid: u32) -> Result<Option<PathBuf>, String> {
    let Some(canon_dir) = hermetic_startup_check_inner(env, opts)? else {
        #[cfg(not(test))]
        let _ = HERMETIC_DIR.set(None);
        #[cfg(test)]
        TEST_HERMETIC_DIR.with(|slot| *slot.borrow_mut() = None);
        return Ok(None);
    };
    let tmp = env.get("TMPDIR").expect("TMPDIR validated by hermetic_startup_check");
    let ack = write_hermetic_ack(Path::new(tmp), &canon_dir, pid).map_err(|e| format!("hermetic agent mode failed to write acknowledgement: {e}"))?;
    #[cfg(not(test))]
    let _ = HERMETIC_DIR.set(Some(canon_dir));
    #[cfg(test)]
    TEST_HERMETIC_DIR.with(|slot| *slot.borrow_mut() = Some(canon_dir));
    Ok(Some(ack))
}

/// OS-environment entrypoint for [`enforce_hermetic_startup`]: reads `vars_os` without panicking on non-UTF-8
/// values, rejecting a non-UTF-8 `DESIGNOS_AGENT_CLI_DIR` and ignoring non-UTF-8 values of other variables.
pub fn enforce_hermetic_startup_os<I, K, V>(env_os: I, opts: &crate::cli::Options, pid: u32) -> Result<Option<PathBuf>, String>
where
    I: IntoIterator<Item = (K, V)>,
    K: Into<std::ffi::OsString>,
    V: Into<std::ffi::OsString>,
{
    let mut env_map = BTreeMap::new();
    for (k, v) in env_os {
        let k_os: std::ffi::OsString = k.into();
        let v_os: std::ffi::OsString = v.into();
        if k_os.as_os_str() == OsStr::new("DESIGNOS_AGENT_CLI_DIR") {
            match v_os.into_string() {
                Ok(val) => {
                    env_map.insert("DESIGNOS_AGENT_CLI_DIR".to_string(), val);
                }
                Err(_) => {
                    return Err("hermetic agent mode denied: DESIGNOS_AGENT_CLI_DIR is not valid UTF-8".to_string());
                }
            }
        } else if let (Ok(k_str), Ok(v_str)) = (k_os.into_string(), v_os.into_string()) {
            env_map.insert(k_str, v_str);
        }
    }
    enforce_hermetic_startup(&env_map, opts, pid)
}

/// Returns the path to the checked-in POSIX agent CLI stubs in `tests/layout-baseline/stubs`.
#[cfg(test)]
pub fn stub_dir() -> &'static Path {
    static STUB_DIR: OnceLock<PathBuf> = OnceLock::new();
    STUB_DIR.get_or_init(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/layout-baseline/stubs"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::AgentCliPreset;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);

    struct ResetTestHermeticDir {
        prev: Option<PathBuf>,
    }

    impl ResetTestHermeticDir {
        fn enter() -> Self {
            let prev = TEST_HERMETIC_DIR.with(|slot| slot.replace(None));
            Self { prev }
        }
    }

    impl Drop for ResetTestHermeticDir {
        fn drop(&mut self) {
            let prev = self.prev.take();
            TEST_HERMETIC_DIR.with(|slot| {
                *slot.borrow_mut() = prev;
            });
        }
    }

    fn make_temp_dir(tag: &str) -> PathBuf {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("designos-1126-{tag}-{}-{id}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let _ = std::fs::write(dir.join(HERMETIC_MARKER_FILENAME), b"ok\n");
        std::fs::canonicalize(&dir).unwrap_or(dir)
    }

    fn write_exec_stub(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, body.as_bytes()).expect("write stub");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod stub");
        }
        path
    }

    /// H1: `build_agent_shell_cmd_in`, given the hermetic dir as an explicit argument, gives argv
    /// `[canonical_stub_path, ...parsed_args]` directly (never `/bin/sh -c`, no `-l` or `-i`),
    /// for every preset and for the Shell preset.
    #[test]
    fn hermetic_shell_cmd_has_no_login_or_interactive_flags() {
        let dir = make_temp_dir("h1");
        for bin in ["claude", "codex", "agy", "grok", "gemini", "opencode"] {
            write_exec_stub(&dir, bin, "#!/bin/sh\nexit 0\n");
        }
        for (preset, launch, expected_tail) in [
            (AgentCliPreset::Claude, "claude --dangerously-skip-permissions", vec!["--dangerously-skip-permissions"]),
            (AgentCliPreset::Codex, "codex --full-auto", vec!["--full-auto"]),
            (AgentCliPreset::Antigravity, "agy --add-dir /tmp", vec!["--add-dir", "/tmp"]),
            (AgentCliPreset::Grok, "grok --cwd /tmp", vec!["--cwd", "/tmp"]),
            (AgentCliPreset::Gemini, "gemini --approval-mode yolo", vec!["--approval-mode", "yolo"]),
            (AgentCliPreset::OpenCode, "opencode", vec![]),
        ] {
            let cmd = crate::terminal::build_agent_shell_cmd_in("/bin/zsh", launch, Some(&dir))
                .unwrap_or_else(|e| panic!("preset {preset:?} must build in hermetic mode: {e}"));
            let argv: Vec<String> = cmd.get_argv().iter().map(|s| s.to_string_lossy().into_owned()).collect();
            let expected_bin = dir.join(preset.binary_name()).display().to_string();
            let mut expected = vec![expected_bin];
            expected.extend(expected_tail.into_iter().map(str::to_string));
            assert_eq!(argv, expected, "preset {preset:?} argv must exec canonical binary directly without /bin/sh -c, -l, or -i");
        }

        let shell_cmd = crate::terminal::build_shell_preset_cmd_in("/bin/zsh", Some(&dir));
        let shell_argv: Vec<String> = shell_cmd.get_argv().iter().map(|s| s.to_string_lossy().into_owned()).collect();
        assert_eq!(shell_argv, vec!["/bin/sh".to_string()], "Shell preset in hermetic mode must use /bin/sh with no -l or -i");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// H2: Pure `cli_search_dirs_with` over built `SearchSources` (no env, no `OnceLock`).
    #[test]
    fn search_dirs_precedence_is_hermetic_scoped_explicit_host() {
        let hermetic_dir = PathBuf::from("/tmp/hermetic-stubs");
        let scoped_dir = PathBuf::from("/tmp/scoped-stubs");
        let explicit_dir = PathBuf::from("/tmp/explicit-stubs");
        let host_dir = PathBuf::from("/tmp/host-bin");
        let extra_dir = PathBuf::from("/tmp/extra-bin");

        let all = SearchSources {
            hermetic: Some(hermetic_dir.clone()),
            scoped: Some(scoped_dir.clone()),
            explicit: Some(vec![explicit_dir.clone()]),
            host_path: vec![host_dir.clone()],
            extra: vec![extra_dir.clone()],
        };
        assert_eq!(cli_search_dirs_with(&all), vec![hermetic_dir], "hermetic must win over scoped, explicit, and host");

        let scoped_only = SearchSources { hermetic: None, ..all.clone() };
        assert_eq!(cli_search_dirs_with(&scoped_only), vec![scoped_dir], "scoped must win over explicit and host");

        let explicit_only = SearchSources { hermetic: None, scoped: None, ..all.clone() };
        assert_eq!(cli_search_dirs_with(&explicit_only), vec![explicit_dir], "explicit must win over host and extra");

        let host_and_extra = SearchSources { hermetic: None, scoped: None, explicit: None, host_path: vec![host_dir.clone()], extra: vec![extra_dir.clone()] };
        assert_eq!(cli_search_dirs_with(&host_and_extra), vec![host_dir, extra_dir], "fallback must concatenate host_path and extra");
    }

    /// H3: `configure_pty_env_with_session_and_hermetic`, given the hermetic dir explicitly,
    /// sets `PATH=DIR:/usr/bin:/bin` and removes `BASH_ENV` and `ENV`.
    #[test]
    fn hermetic_pty_env_is_stub_path_only() {
        let dir = make_temp_dir("h3");
        let mut cmd = portable_pty::CommandBuilder::new("/bin/sh");
        cmd.env("BASH_ENV", "/tmp/evil-bash-env");
        cmd.env("ENV", "/tmp/evil-sh-env");

        crate::terminal::configure_pty_env_with_session_and_hermetic(&mut cmd, AgentCliPreset::Claude, None, None, Some(&dir));

        let sep = if cfg!(windows) { ";" } else { ":" };
        let expected_path = format!("{}{sep}/usr/bin{sep}/bin", dir.display());
        assert_eq!(cmd.get_env("PATH").and_then(|v| v.to_str()), Some(expected_path.as_str()), "hermetic PTY env must set PATH=DIR:/usr/bin:/bin");
        assert_eq!(cmd.get_env("BASH_ENV"), None, "hermetic PTY env must remove BASH_ENV");
        assert_eq!(cmd.get_env("ENV"), None, "hermetic PTY env must remove ENV");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// H4: `hermetic_startup_check` returns `Err` naming the missing item for each of:
    /// no `--screenshot`; `DESIGNOS_LIBRARY` unset; `TMPDIR` without a marker;
    /// `HOME` without a marker; `DIR` without a marker. `Ok` only with all of them.
    #[test]
    fn hermetic_mode_is_denied_without_every_requirement() {
        let root = make_temp_dir("h4");
        let stubs = root.join("stubs");
        let lib = root.join("lib");
        let tmp = root.join("tmp");
        let home = root.join("home");
        for d in [&stubs, &lib, &tmp, &home] {
            std::fs::create_dir_all(d).unwrap();
            std::fs::write(d.join(HERMETIC_MARKER_FILENAME), b"ok\n").unwrap();
        }

        let full_env = || -> BTreeMap<String, String> {
            let mut m = BTreeMap::new();
            m.insert("DESIGNOS_AGENT_CLI_DIR".into(), stubs.display().to_string());
            m.insert("DESIGNOS_LIBRARY".into(), lib.display().to_string());
            m.insert("TMPDIR".into(), tmp.display().to_string());
            m.insert("HOME".into(), home.display().to_string());
            m
        };
        let shot_opts = crate::cli::Options { screenshot: Some(root.join("out.png")), ..Default::default() };

        // 1. No --screenshot
        let err_no_shot = hermetic_startup_check(&full_env(), &crate::cli::Options::default()).expect_err("missing --screenshot must be denied");
        assert!(err_no_shot.contains("--screenshot"), "error must name --screenshot: {err_no_shot}");

        // 2. DESIGNOS_LIBRARY unset
        let mut env_no_lib = full_env();
        env_no_lib.remove("DESIGNOS_LIBRARY");
        let err_no_lib = hermetic_startup_check(&env_no_lib, &shot_opts).expect_err("unset DESIGNOS_LIBRARY must be denied");
        assert!(err_no_lib.contains("DESIGNOS_LIBRARY"), "error must name DESIGNOS_LIBRARY: {err_no_lib}");

        // 3. TMPDIR without marker
        std::fs::remove_file(tmp.join(HERMETIC_MARKER_FILENAME)).unwrap();
        let err_no_tmp_marker = hermetic_startup_check(&full_env(), &shot_opts).expect_err("TMPDIR without marker must be denied");
        assert!(err_no_tmp_marker.contains("TMPDIR"), "error must name TMPDIR: {err_no_tmp_marker}");
        std::fs::write(tmp.join(HERMETIC_MARKER_FILENAME), b"ok\n").unwrap();

        // 4. HOME without marker
        std::fs::remove_file(home.join(HERMETIC_MARKER_FILENAME)).unwrap();
        let err_no_home_marker = hermetic_startup_check(&full_env(), &shot_opts).expect_err("HOME without marker must be denied");
        assert!(err_no_home_marker.contains("HOME"), "error must name HOME: {err_no_home_marker}");
        std::fs::write(home.join(HERMETIC_MARKER_FILENAME), b"ok\n").unwrap();

        // 5. DIR without marker
        std::fs::remove_file(stubs.join(HERMETIC_MARKER_FILENAME)).unwrap();
        let err_no_dir_marker = hermetic_startup_check(&full_env(), &shot_opts).expect_err("DESIGNOS_AGENT_CLI_DIR without marker must be denied");
        assert!(err_no_dir_marker.contains("DESIGNOS_AGENT_CLI_DIR"), "error must name DESIGNOS_AGENT_CLI_DIR: {err_no_dir_marker}");
        std::fs::write(stubs.join(HERMETIC_MARKER_FILENAME), b"ok\n").unwrap();

        // 6. All present -> Ok(())
        hermetic_startup_check(&full_env(), &shot_opts).expect("all markers + --screenshot must pass");

        let _ = std::fs::remove_dir_all(&root);
    }

    /// H5: `"claude; rm -rf x"`, `"claude || codex"`, `"$(x)"` and `"foo"` (not in `DIR`) are refused,
    /// with no fallback lookup.
    #[test]
    fn hermetic_mode_refuses_compound_or_unknown_commands() {
        let dir = make_temp_dir("h5");
        write_exec_stub(&dir, "claude", "#!/bin/sh\nexit 0\n");

        for bad in ["claude; rm -rf x", "claude || codex", "claude && codex", "claude | cat", "`claude`", "$(x)", "foo", "/usr/bin/env claude"] {
            let res = crate::terminal::build_agent_shell_cmd_in("/bin/zsh", bad, Some(&dir));
            let err = match res {
                Ok(_) => panic!("hermetic mode must refuse {bad:?}"),
                Err(e) => e,
            };
            assert!(err.starts_with("hermetic mode: refused command "), "unexpected error for {bad:?}: {err}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// H6: Pure `resolve_in(&[DIR], bin)` returns `DIR/bin` or `None`.
    /// `resolve_cli_executable` and `resolve_grok_binary`, called with
    /// `SearchSources{hermetic: DIR, host_path: <dir with fake claude and grok>}` passed explicitly,
    /// return `DIR` paths or `None`, never the host path.
    #[test]
    fn every_resolver_uses_resolve_in() {
        let root = make_temp_dir("h6");
        let stub_dir = root.join("stubs");
        let host_dir = root.join("host");
        std::fs::create_dir_all(&stub_dir).unwrap();
        std::fs::create_dir_all(&host_dir).unwrap();

        let stub_claude = write_exec_stub(&stub_dir, "claude", "#!/bin/sh\nexit 0\n");
        let _host_claude = write_exec_stub(&host_dir, "claude", "#!/bin/sh\nexit 0\n");
        let _host_grok = write_exec_stub(&host_dir, "grok", "#!/bin/sh\nexit 0\n");

        assert_eq!(resolve_in(std::slice::from_ref(&stub_dir), "claude"), Some(stub_claude.clone()));
        assert_eq!(resolve_in(std::slice::from_ref(&stub_dir), "grok"), None);

        let sources =
            SearchSources { hermetic: Some(stub_dir.clone()), scoped: None, explicit: None, host_path: vec![host_dir.clone()], extra: vec![host_dir.clone()] };

        assert_eq!(crate::agent_session_failure::resolve_cli_executable("claude", &sources), Some(stub_claude));
        assert_eq!(
            crate::agent_session_failure::resolve_cli_executable("grok", &sources),
            None,
            "resolve_cli_executable must never fall back to host_path when hermetic is set"
        );
        assert_eq!(
            crate::agent_usage::resolve_grok_binary(&sources),
            None,
            "resolve_grok_binary must return None when grok is absent from hermetic dir, even if present in host_path/extra"
        );

        let stub_grok = write_exec_stub(&stub_dir, "grok", "#!/bin/sh\nexit 0\n");
        assert_eq!(crate::agent_usage::resolve_grok_binary(&sources), Some(stub_grok));

        let _ = std::fs::remove_dir_all(&root);
    }

    /// H7: Thread-local, deterministic: inside `ScopedCliDirs::enter(stub)`, `cli_search_dirs(None)`
    /// is `[stub]` and `AgentCliPreset::Claude.is_installed()` is true. After the guard drops it is
    /// the host list again. Another thread is unaffected.
    #[test]
    fn scoped_guard_applies_only_inside_a_gate_case() {
        let stub = make_temp_dir("h7");
        write_exec_stub(&stub, "claude", "#!/bin/sh\nexit 0\n");
        let outside_dirs = cli_search_dirs(None);
        assert_ne!(outside_dirs, vec![stub.clone()]);

        {
            let _guard = ScopedCliDirs::enter(&stub);
            assert_eq!(cli_search_dirs(None), vec![stub.clone()]);
            assert!(AgentCliPreset::Claude.is_installed(), "Claude must be reported installed from ScopedCliDirs stub");
            assert!(!AgentCliPreset::Codex.is_installed(), "Codex is absent from stub dir so is_installed() must be false");

            let stub_for_thread = stub.clone();
            let other_thread_dirs = std::thread::spawn(move || {
                let d = cli_search_dirs(None);
                assert_ne!(d, vec![stub_for_thread], "another thread must not see this thread's ScopedCliDirs");
                d
            })
            .join()
            .expect("join thread");
            assert_eq!(other_thread_dirs, outside_dirs);
        }

        assert_eq!(cli_search_dirs(None), outside_dirs, "after ScopedCliDirs drops, cli_search_dirs(None) must return the host list again");
        let _ = std::fs::remove_dir_all(&stub);
    }

    /// H9: After `hermetic_startup_check` passes, `write_hermetic_ack(tmp, DIR, pid)` writes
    /// `tmp/.designos-hermetic-active` with that pid and `DIR`. A denied check writes nothing.
    #[test]
    fn hermetic_mode_writes_the_acknowledgement() {
        let _reset_hermetic = ResetTestHermeticDir::enter();
        let root = make_temp_dir("h9");
        let stubs = root.join("stubs");
        let lib = root.join("lib");
        let tmp = root.join("tmp");
        let home = root.join("home");
        for d in [&stubs, &lib, &tmp, &home] {
            std::fs::create_dir_all(d).unwrap();
            std::fs::write(d.join(HERMETIC_MARKER_FILENAME), b"ok\n").unwrap();
        }

        let mut env = BTreeMap::new();
        env.insert("DESIGNOS_AGENT_CLI_DIR".into(), stubs.display().to_string());
        env.insert("DESIGNOS_LIBRARY".into(), lib.display().to_string());
        env.insert("TMPDIR".into(), tmp.display().to_string());
        env.insert("HOME".into(), home.display().to_string());

        // Denied check (no --screenshot) must write nothing
        let ack_path = tmp.join(HERMETIC_ACTIVE_FILENAME);
        let denied = enforce_hermetic_startup(&env, &crate::cli::Options::default(), 98765);
        assert!(denied.is_err(), "missing --screenshot must fail");
        assert!(!ack_path.exists(), "denied hermetic check must not write {HERMETIC_ACTIVE_FILENAME}");

        // Passing check writes <tmp>/.designos-hermetic-active with pid and DIR
        let shot_opts = crate::cli::Options { screenshot: Some(root.join("out.png")), ..Default::default() };
        let written =
            enforce_hermetic_startup(&env, &shot_opts, 98765).expect("valid hermetic startup must succeed").expect("must return acknowledgement path");
        assert_eq!(written, ack_path);
        let content = std::fs::read_to_string(&ack_path).expect("read .designos-hermetic-active");
        assert!(content.contains("98765"), "ack must contain pid: {content}");
        assert!(content.contains(&stubs.display().to_string()), "ack must contain DIR: {content}");

        let _ = std::fs::remove_dir_all(&root);
    }

    /// H10 + 8(g): Scans `crates/designos-app/src/**/*.rs` for `split_paths(`, `var_os("PATH")`,
    /// `var("PATH")`, `Command::new("<agent>")`, and `PathBuf::from("<agent>")`.
    /// Allow-lists only `cli_resolve.rs` and documented non-agent uses (`io/dialog_probe.rs`'s Linux
    /// `zenity` file-chooser availability lookup).
    #[test]
    fn hermetic_resolver_census() {
        fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    collect_rs_files(&p, out);
                } else if p.extension().and_then(|e| e.to_str()) == Some("rs") {
                    out.push(p);
                }
            }
        }

        let src_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        collect_rs_files(&src_dir, &mut files);
        files.sort();

        let split_needle = ["split_paths", "("].concat();
        let var_os_needle = ["var_os(", "\"PATH\"", ")"].concat();
        let var_needle = ["var(", "\"PATH\"", ")"].concat();
        let agents = ["claude", "codex", "grok", "gemini", "agy", "opencode"];
        let cmd_agent_needles: Vec<String> = agents.iter().map(|a| ["Command::new(\"", a, "\")"].concat()).collect();
        let pathbuf_agent_needles: Vec<String> = agents.iter().map(|a| ["PathBuf::from(\"", a, "\")"].concat()).collect();

        // Documented non-agent PATH allow-list:
        // - `io/dialog_probe.rs`: probes `PATH` for Linux `zenity` file-chooser availability; never resolves or spawns agent CLIs.
        let allowed_non_agent_var_path: &[(&Path, &str)] =
            &[(Path::new("io/dialog_probe.rs"), "Linux zenity file-chooser availability probe (never resolves agent CLIs)")];

        let mut violations = Vec::new();
        for file in files {
            let rel = file.strip_prefix(&src_dir).unwrap_or(&file);
            if rel == Path::new("cli_resolve.rs")
                || rel == Path::new("test_agent_path.rs")
                || rel == Path::new("terminal/real_cli_launch_tests.rs")
                || rel == Path::new("terminal/capture_lock_tests.rs")
                || rel == Path::new("agent_permissions/approval_channel_tests.rs")
            {
                continue;
            }
            let text = std::fs::read_to_string(&file).expect("read source file");
            let allow_var_path = allowed_non_agent_var_path.iter().any(|(allowed_rel, _reason)| rel == *allowed_rel);

            for (idx, line) in text.lines().enumerate() {
                let line_no = idx + 1;
                let code_part = line.split("//").next().unwrap_or("");
                let has_path_violation =
                    code_part.contains(&split_needle) || code_part.contains(&var_os_needle) || (!allow_var_path && code_part.contains(&var_needle));
                let has_agent_violation =
                    cmd_agent_needles.iter().any(|n| code_part.contains(n)) || pathbuf_agent_needles.iter().any(|n| code_part.contains(n));
                if has_path_violation || has_agent_violation {
                    violations.push(format!("{}:{line_no}: {}", rel.display(), line.trim()));
                }
            }
        }
        assert!(violations.is_empty(), "found unallowed PATH or bare-agent lookup outside cli_resolve.rs:\n{}", violations.join("\n"));
    }

    /// Round 10b census: no file under `crates/designos-app/src` contains
    /// `set_var("DESIGNOS_AGENT_CLI_DIR"` or `remove_var("DESIGNOS_AGENT_CLI_DIR"`.
    #[test]
    fn no_source_file_sets_or_removes_agent_cli_dir_env() {
        fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    collect_rs_files(&p, out);
                } else if p.extension().and_then(|e| e.to_str()) == Some("rs") {
                    out.push(p);
                }
            }
        }

        let src_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        collect_rs_files(&src_dir, &mut files);
        files.sort();

        let set_needle = ["set_var(\"", "DESIGNOS_AGENT_CLI_DIR", "\""].concat();
        let remove_needle = ["remove_var(\"", "DESIGNOS_AGENT_CLI_DIR", "\""].concat();

        let mut violations = Vec::new();
        for file in files {
            let rel = file.strip_prefix(&src_dir).unwrap_or(&file);
            let text = std::fs::read_to_string(&file).expect("read source file");
            for (idx, line) in text.lines().enumerate() {
                let line_no = idx + 1;
                let code_part = line.split("//").next().unwrap_or("");
                if code_part.contains(&set_needle) || code_part.contains(&remove_needle) {
                    violations.push(format!("{}:{line_no}: {}", rel.display(), line.trim()));
                }
            }
        }
        assert!(
            violations.is_empty(),
            "found forbidden process-environment mutation of DESIGNOS_AGENT_CLI_DIR under crates/designos-app/src:\n{}",
            violations.join("\n")
        );
    }

    /// 8(a): Startup order: `run_startup_before_window` (the function `main` calls first after `cli::parse`)
    /// enforces hermetic startup before `--help`, `--version`, `--seed-library`, and `run_agent_cli` (`--apply-spec`),
    /// returning exit code 2 and writing nothing when the hermetic environment is invalid.
    #[test]
    fn startup_check_runs_before_seed_library_and_agent_cli() {
        let root = make_temp_dir("r2-8a");
        let doc_path = root.join("copy.designos");
        let spec_path = root.join("spec.json");
        let seed_dir = root.join("seeded");
        std::fs::create_dir_all(&seed_dir).unwrap();

        let mut doc = designos_doc::Document::new("Original");
        crate::io::ensure_page(&mut doc);
        crate::io::save_path(&doc, &doc_path).expect("save initial doc");
        let initial_bytes = std::fs::read(&doc_path).expect("read initial doc bytes");

        std::fs::write(&spec_path, br#"{"kind":"frame","name":"Injected","width":120,"height":80}"#).expect("write spec");

        let bad_env = || -> Vec<(std::ffi::OsString, std::ffi::OsString)> { vec![("DESIGNOS_AGENT_CLI_DIR".into(), root.join("no-marker").into_os_string())] };

        // 1. --apply-spec with invalid hermetic env exits 2 and does not modify doc_path
        let apply_opts = crate::cli::Options { file: Some(doc_path.clone()), apply_spec: Some(spec_path), ..Default::default() };
        let res_apply = crate::run_startup_before_window(&apply_opts, bad_env(), 4242);
        let (code_apply, msg_apply) = res_apply.expect_err("--apply-spec with invalid hermetic env must fail");
        assert_eq!(code_apply, 2, "--apply-spec must exit 2: {msg_apply}");
        assert!(msg_apply.contains("hermetic agent mode denied"), "error must come from hermetic check: {msg_apply}");
        assert_eq!(std::fs::read(&doc_path).unwrap(), initial_bytes, "--apply-spec must not modify the target file when hermetic check fails");

        // 2. --seed-library with invalid hermetic env exits 2 and writes nothing into seed_dir
        let seed_opts = crate::cli::Options { seed_library: Some(seed_dir.clone()), ..Default::default() };
        let res_seed = crate::run_startup_before_window(&seed_opts, bad_env(), 4242);
        let (code_seed, msg_seed) = res_seed.expect_err("--seed-library with invalid hermetic env must fail");
        assert_eq!(code_seed, 2, "--seed-library must exit 2: {msg_seed}");
        assert!(msg_seed.contains("hermetic agent mode denied"), "error must come from hermetic check: {msg_seed}");
        assert!(std::fs::read_dir(&seed_dir).unwrap().next().is_none(), "--seed-library must leave directory empty when hermetic check fails");

        // 3. --help and --version with invalid hermetic env also exit 2
        let help_opts = crate::cli::Options { help: true, ..Default::default() };
        assert_eq!(crate::run_startup_before_window(&help_opts, bad_env(), 4242).unwrap_err().0, 2);
        let ver_opts = crate::cli::Options { version: true, ..Default::default() };
        assert_eq!(crate::run_startup_before_window(&ver_opts, bad_env(), 4242).unwrap_err().0, 2);

        // 4. Non-UTF-8 DESIGNOS_AGENT_CLI_DIR exits 2; non-UTF-8 other variable is ignored without panic
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            let non_utf8 = std::ffi::OsString::from_vec(vec![0xff, 0xfe, 0xfd]);
            let bad_utf8_env = vec![(std::ffi::OsString::from("DESIGNOS_AGENT_CLI_DIR"), non_utf8.clone())];
            let (code_utf8, _) = crate::run_startup_before_window(&crate::cli::Options::default(), bad_utf8_env, 4242)
                .expect_err("non-UTF-8 DESIGNOS_AGENT_CLI_DIR must exit 2");
            assert_eq!(code_utf8, 2);

            let other_non_utf8_env = vec![(std::ffi::OsString::from("UNRELATED_BIN_VAR"), non_utf8)];
            assert_eq!(
                crate::run_startup_before_window(&crate::cli::Options::default(), other_non_utf8_env, 4242),
                Ok(crate::StartupDisposition::ContinueToWindow),
                "non-UTF-8 unrelated env var must be ignored when DESIGNOS_AGENT_CLI_DIR is unset"
            );
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    /// 8(b): A symlinked stub pointing outside the stub directory is refused.
    #[test]
    #[cfg(unix)]
    fn hermetic_mode_refuses_symlink_outside_stub_dir() {
        let root = make_temp_dir("r2-8b");
        let stubs = root.join("stubs");
        let outside = root.join("outside");
        std::fs::create_dir_all(&stubs).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(stubs.join(HERMETIC_MARKER_FILENAME), b"ok\n").unwrap();

        let outside_claude = write_exec_stub(&outside, "claude", "#!/bin/sh\nexit 0\n");
        std::os::unix::fs::symlink(&outside_claude, stubs.join("claude")).expect("symlink claude -> outside");

        assert_eq!(
            resolve_in(std::slice::from_ref(&stubs), "claude"),
            None,
            "resolve_in in hermetic/stub dir must refuse a symlink pointing outside the stub dir"
        );
        assert!(
            crate::terminal::build_agent_shell_cmd_in("/bin/zsh", "claude --dangerously-skip-permissions", Some(&stubs)).is_err(),
            "build_agent_shell_cmd_in must refuse a symlinked binary pointing outside the stub dir"
        );
        let sources = SearchSources { hermetic: Some(stubs), ..Default::default() };
        assert_eq!(
            crate::agent_session_failure::resolve_cli_executable("claude", &sources),
            None,
            "resolve_cli_executable must refuse a symlink pointing outside the hermetic dir"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// 8(c): A relative `DESIGNOS_AGENT_CLI_DIR` or a path containing `..` is refused at startup.
    #[test]
    fn hermetic_startup_refuses_relative_or_dotdot_dir() {
        let root = make_temp_dir("r2-8c");
        let stubs = root.join("stubs");
        let lib = root.join("lib");
        let tmp = root.join("tmp");
        let home = root.join("home");
        for d in [&stubs, &lib, &tmp, &home] {
            std::fs::create_dir_all(d).unwrap();
            std::fs::write(d.join(HERMETIC_MARKER_FILENAME), b"ok\n").unwrap();
        }
        let shot_opts = crate::cli::Options { screenshot: Some(root.join("out.png")), ..Default::default() };

        let mut env_rel = BTreeMap::new();
        env_rel.insert("DESIGNOS_AGENT_CLI_DIR".into(), "stubs".into());
        env_rel.insert("DESIGNOS_LIBRARY".into(), lib.display().to_string());
        env_rel.insert("TMPDIR".into(), tmp.display().to_string());
        env_rel.insert("HOME".into(), home.display().to_string());
        let err_rel = hermetic_startup_check(&env_rel, &shot_opts).expect_err("relative DESIGNOS_AGENT_CLI_DIR must be denied");
        assert!(err_rel.contains("DESIGNOS_AGENT_CLI_DIR"), "error must name DESIGNOS_AGENT_CLI_DIR: {err_rel}");

        let mut env_dotdot = env_rel;
        env_dotdot.insert("DESIGNOS_AGENT_CLI_DIR".into(), format!("{}/../stubs", stubs.display()));
        let err_dotdot = hermetic_startup_check(&env_dotdot, &shot_opts).expect_err(".. in DESIGNOS_AGENT_CLI_DIR must be denied");
        assert!(err_dotdot.contains("DESIGNOS_AGENT_CLI_DIR"), "error must name DESIGNOS_AGENT_CLI_DIR: {err_dotdot}");

        let _ = std::fs::remove_dir_all(&root);
    }

    /// 8(d): `claude & /usr/bin/true`, `claude > x`, `claude < x`, and unexpanded non-`DESIGNOS_*`
    /// variables are refused in hermetic mode; valid `$DESIGNOS_*` variables are expanded without a shell.
    #[test]
    fn hermetic_mode_refuses_background_redirect_and_unexpanded_vars() {
        let dir = make_temp_dir("r2-8d");
        write_exec_stub(&dir, "claude", "#!/bin/sh\nexit 0\n");

        for bad in ["claude & /usr/bin/true", "claude > x", "claude < x", "claude $HOME", "claude ${USER}", "claude $1"] {
            let res = crate::terminal::build_agent_shell_cmd_in("/bin/zsh", bad, Some(&dir));
            let err = match res {
                Ok(cmd) => panic!("hermetic mode must refuse {bad:?}, got argv {:?}", cmd.get_argv()),
                Err(e) => e,
            };
            assert!(err.starts_with("hermetic mode: refused command "), "unexpected error for {bad:?}: {err}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 8(e): The grok usage caller (`TerminalState::refresh_tab_usage`) with no `grok` stub spawns nothing.
    #[test]
    fn grok_usage_caller_with_no_grok_stub_spawns_nothing() {
        let empty_stubs = make_temp_dir("r2-8e-empty");
        let _guard = ScopedCliDirs::enter(&empty_stubs);

        let mut st = crate::terminal::TerminalState::default();
        st.session_dir = empty_stubs.join("session");
        std::fs::create_dir_all(&st.session_dir).unwrap();
        let mut tab =
            crate::terminal::TerminalTab::spawn_with_session_cmd(1, AgentCliPreset::Grok, 24, 80, None, None, Some(portable_pty::CommandBuilder::new("true")))
                .expect("spawn dummy tab");
        tab.session_uuid = Some("grok-sid-8e".to_string());
        st.tabs.push(tab);

        st.refresh_tab_usage(0);
        assert!(
            st.tabs[0].usage.elapsed_since_last_poll().is_none(),
            "refresh_tab_usage must not trigger grok usage subprocess when grok binary is absent from stub dir"
        );

        let _ = std::fs::remove_dir_all(&empty_stubs);
    }

    /// 8(f): With hermetic mode off, the child `PATH` built in `agent_cli_catalog.rs` prepends
    /// `install_folders` plus `extra_cli_path_dirs()` before the process `PATH`, in the exact old order.
    #[test]
    #[cfg(unix)]
    fn non_hermetic_catalog_path_prepends_install_and_extra_before_host_path() {
        use crate::agent_cli_catalog::{RunCommand, SystemCommandRunner};
        use std::time::Duration;

        let install_dir = make_temp_dir("r2-8f-install");
        write_exec_stub(&install_dir, "print-path", "#!/bin/sh\nprintf '%s' \"$PATH\"\n");

        let out = SystemCommandRunner.run(&["print-path"], std::slice::from_ref(&install_dir), Duration::from_secs(30));
        assert_eq!(out.exit_code, Some(0), "print-path must succeed: stderr={}", out.stderr);

        let sep = if cfg!(windows) { ";" } else { ":" };
        let mut extra_paths: Vec<String> = vec![install_dir.to_string_lossy().into_owned()];
        extra_paths.extend(crate::terminal::extra_cli_path_dirs().into_iter().map(|p| p.to_string_lossy().into_owned()));
        let current_path = std::env::var("PATH").unwrap_or_default();
        let expected = if current_path.is_empty() { extra_paths.join(sep) } else { format!("{}{sep}{current_path}", extra_paths.join(sep)) };

        assert_eq!(out.stdout, expected, "non-hermetic catalog child PATH must prepend install_folders + extra_cli_path_dirs() before process PATH");
        let _ = std::fs::remove_dir_all(&install_dir);
    }

    /// H11: Inside a scoped guard, `warm_cli_version_cache(Claude)` resolves to the stub path
    /// on the calling thread, as recorded by the test hook before `thread::spawn`.
    #[test]
    fn version_warmer_resolves_on_the_calling_thread() {
        let stub = make_temp_dir("h11");
        let claude = write_exec_stub(&stub, "claude", "#!/bin/sh\necho 'claude 1.0.0-stub'\n");
        let _ = crate::agent_session_failure::take_last_warmer_resolved();

        {
            let _guard = ScopedCliDirs::enter(&stub);
            crate::agent_session_failure::warm_cli_version_cache(AgentCliPreset::Claude);
        }

        let resolved = crate::agent_session_failure::take_last_warmer_resolved();
        assert_eq!(resolved, Some(Some(claude)), "warm_cli_version_cache must resolve on the calling thread inside ScopedCliDirs before spawning");
        let _ = std::fs::remove_dir_all(&stub);
    }

    /// H8: Each checked-in stub: `--version` exit 0 with a fixed string; `auth status` and
    /// `login status` exit 0 with fixed strings; `agy --mcp-config x` exit 1 with one error line;
    /// anything else logs `$0 $*` to `DESIGNOS_STUB_LOG` and runs `cat`. Also verifies `shell/tests.rs`
    /// uses `stub_dir()` instead of writing dynamic stubs.
    #[test]
    #[cfg(unix)]
    fn stub_scripts_behave() {
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;
        use std::process::{Command, Stdio};

        let dir = super::stub_dir();
        let expected_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/layout-baseline/stubs");
        assert_eq!(dir, expected_dir.as_path(), "stub_dir() must point to checked-in tests/layout-baseline/stubs");

        let tmp = make_temp_dir("h8-log");
        let log_path = tmp.join("invocations.log");

        for bin in ["claude", "codex", "agy", "grok", "gemini", "opencode"] {
            let script = dir.join(bin);
            assert!(script.is_file(), "missing checked-in stub {}", script.display());
            let mode = std::fs::metadata(&script).expect("stub metadata").permissions().mode();
            assert_ne!(mode & 0o111, 0, "stub {} must be executable", script.display());

            let ver = Command::new(&script).arg("--version").env("DESIGNOS_STUB_LOG", &log_path).output().expect("run --version");
            assert!(ver.status.success(), "{bin} --version must exit 0");
            assert!(!String::from_utf8_lossy(&ver.stdout).trim().is_empty(), "{bin} --version must print fixed string");

            let auth = Command::new(&script).args(["auth", "status"]).env("DESIGNOS_STUB_LOG", &log_path).output().expect("run auth status");
            assert!(auth.status.success(), "{bin} auth status must exit 0");

            let login = Command::new(&script).args(["login", "status"]).env("DESIGNOS_STUB_LOG", &log_path).output().expect("run login status");
            assert!(login.status.success(), "{bin} login status must exit 0");

            let mut child = Command::new(&script)
                .args(["--flag", "arg"])
                .env("DESIGNOS_STUB_LOG", &log_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("spawn stub cat mode");
            child.stdin.take().expect("stdin").write_all(b"ping\n").expect("write stdin");
            let out = child.wait_with_output().expect("wait stub cat mode");
            assert!(out.status.success(), "{bin} cat mode must exit 0");
            assert_eq!(String::from_utf8_lossy(&out.stdout), "ping\n", "{bin} must exec /bin/cat");
            assert!(out.stderr.is_empty(), "{bin} cat mode must print no extra output");

            let log_text = std::fs::read_to_string(&log_path).expect("read stub log");
            let expected_line = format!("{} --flag arg", script.display());
            assert!(log_text.lines().any(|l| l == expected_line), "expected stub log to contain `{expected_line}`, got:\n{log_text}");
        }

        let agy_fail = Command::new(dir.join("agy")).args(["--mcp-config", "x"]).env("DESIGNOS_STUB_LOG", &log_path).output().expect("run agy --mcp-config x");
        assert_eq!(agy_fail.status.code(), Some(1), "agy --mcp-config x must exit 1");
        let combined = format!("{}{}", String::from_utf8_lossy(&agy_fail.stdout), String::from_utf8_lossy(&agy_fail.stderr));
        let non_empty_lines: Vec<&str> = combined.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(non_empty_lines.len(), 1, "agy --mcp-config x must print one error line, got: {combined:?}");

        let shell_tests = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/shell/tests.rs")).expect("read shell/tests.rs");
        assert!(!shell_tests.contains("designos-stub-agent-clis"), "shell/tests.rs must use checked-in stub_dir() instead of writing dynamic stubs");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Round 3 Part 1.1: With a parent `DESIGNOS_MCP_BIN=/usr/bin/true` (and a conflicting
    /// `DESIGNOS_WORKSPACE` and `DESIGNOS_FILE`), the hermetic Codex argv carries
    /// `designos_mcp_binary()` and the session workspace, matching the PTY child env.
    #[test]
    fn hermetic_codex_expansion_uses_tab_env_not_parent_env() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let root = make_temp_dir("r3-p1-codex-env");
        let stubs = root.join("stubs");
        let ws_dir = root.join("session-ws");
        std::fs::create_dir_all(&stubs).unwrap();
        std::fs::create_dir_all(&ws_dir).unwrap();
        std::fs::write(stubs.join(HERMETIC_MARKER_FILENAME), b"ok\n").unwrap();
        write_exec_stub(&stubs, "codex", "#!/bin/sh\nexit 0\n");
        let doc_file = ws_dir.join("board.designos");
        std::fs::write(&doc_file, b"{}").unwrap();

        let prev_mcp = std::env::var_os("DESIGNOS_MCP_BIN");
        let prev_ws = std::env::var_os("DESIGNOS_WORKSPACE");
        let prev_file = std::env::var_os("DESIGNOS_FILE");
        std::env::set_var("DESIGNOS_MCP_BIN", "/usr/bin/true");
        std::env::set_var("DESIGNOS_WORKSPACE", "/tmp/conflicting-parent-workspace");
        std::env::set_var("DESIGNOS_FILE", "/tmp/conflicting-parent-file.designos");

        let codex_launch = AgentCliPreset::Codex.cli_launch_command_for_mode(crate::agent_permissions::AgentPermissionMode::Ask);
        let cmd_res = crate::terminal::build_agent_shell_cmd_in("/bin/zsh", codex_launch, Some(&stubs));

        let mut pty_cmd = portable_pty::CommandBuilder::new("/bin/sh");
        crate::terminal::configure_pty_env_with_session_and_hermetic(&mut pty_cmd, AgentCliPreset::Codex, None, None, Some(&stubs));
        let expected_mcp_bin = pty_cmd.get_env("DESIGNOS_MCP_BIN").and_then(|v| v.to_str()).expect("PTY DESIGNOS_MCP_BIN").to_string();
        let expected_ws = pty_cmd.get_env("DESIGNOS_WORKSPACE").and_then(|v| v.to_str()).expect("PTY DESIGNOS_WORKSPACE").to_string();

        match prev_mcp {
            Some(v) => std::env::set_var("DESIGNOS_MCP_BIN", v),
            None => std::env::remove_var("DESIGNOS_MCP_BIN"),
        }
        match prev_ws {
            Some(v) => std::env::set_var("DESIGNOS_WORKSPACE", v),
            None => std::env::remove_var("DESIGNOS_WORKSPACE"),
        }
        match prev_file {
            Some(v) => std::env::set_var("DESIGNOS_FILE", v),
            None => std::env::remove_var("DESIGNOS_FILE"),
        }

        let cmd = cmd_res.expect("hermetic Codex preset command must build");
        let argv: Vec<String> = cmd.get_argv().iter().map(|s| s.to_string_lossy().into_owned()).collect();
        let joined = argv.join(" ");

        assert_ne!(expected_mcp_bin, "/usr/bin/true", "PTY DESIGNOS_MCP_BIN must be designos_mcp_binary(), not parent /usr/bin/true");
        assert!(!joined.contains("/usr/bin/true"), "hermetic Codex argv must not use parent DESIGNOS_MCP_BIN=/usr/bin/true, got argv: {argv:?}");
        assert!(!joined.contains("/tmp/conflicting-parent-workspace"), "hermetic Codex argv must not use parent DESIGNOS_WORKSPACE, got argv: {argv:?}");
        assert!(joined.contains(&expected_mcp_bin), "hermetic Codex argv must contain designos_mcp_binary() ({expected_mcp_bin}), got argv: {argv:?}");
        assert!(joined.contains(&expected_ws), "hermetic Codex argv must contain session workspace ({expected_ws}), got argv: {argv:?}");

        let _ = std::fs::remove_dir_all(&root);
    }

    /// Round 3 Part 1.2: A symlinked `DESIGNOS_AGENT_CLI_DIR` that passed startup, retargeted
    /// afterwards at another dir holding a `claude`, still resolves to the original stub dir
    /// (or refuses if passed the retargeted path without marker). It never runs the retargeted `claude`.
    #[test]
    #[cfg(unix)]
    fn hermetic_startup_dir_is_pinned_after_symlink_retarget() {
        let _reset_hermetic = ResetTestHermeticDir::enter();
        let root = make_temp_dir("r3-p1-retarget");
        let orig_stubs = root.join("orig-stubs");
        let retargeted = root.join("retargeted");
        let lib = root.join("lib");
        let tmp = root.join("tmp");
        let home = root.join("home");
        for d in [&orig_stubs, &lib, &tmp, &home] {
            std::fs::create_dir_all(d).unwrap();
            std::fs::write(d.join(HERMETIC_MARKER_FILENAME), b"ok\n").unwrap();
        }
        std::fs::create_dir_all(&retargeted).unwrap();

        let orig_claude = write_exec_stub(&orig_stubs, "claude", "#!/bin/sh\nexit 0\n");
        let retargeted_claude = write_exec_stub(&retargeted, "claude", "#!/bin/sh\nexit 0\n");
        let orig_canon = std::fs::canonicalize(&orig_stubs).unwrap();
        let orig_claude_canon = std::fs::canonicalize(&orig_claude).unwrap();
        let retargeted_claude_canon = std::fs::canonicalize(&retargeted_claude).unwrap();

        let link_dir = root.join("cli-link");
        std::os::unix::fs::symlink(&orig_stubs, &link_dir).expect("symlink cli-link -> orig_stubs");

        let mut env = BTreeMap::new();
        env.insert("DESIGNOS_AGENT_CLI_DIR".into(), link_dir.display().to_string());
        env.insert("DESIGNOS_LIBRARY".into(), lib.display().to_string());
        env.insert("TMPDIR".into(), tmp.display().to_string());
        env.insert("HOME".into(), home.display().to_string());
        let shot_opts = crate::cli::Options { screenshot: Some(root.join("out.png")), ..Default::default() };

        enforce_hermetic_startup(&env, &shot_opts, 98765).expect("startup with valid symlinked stub dir must succeed");

        // Retarget cli-link at `retargeted` (which has no .designos-hermetic marker).
        std::fs::remove_file(&link_dir).expect("remove cli-link");
        std::os::unix::fs::symlink(&retargeted, &link_dir).expect("retarget cli-link -> retargeted");

        // 1. Without the `or_else(canonicalize)` fallback, passing `&link_dir` directly must be refused.
        let res = crate::terminal::build_agent_shell_cmd_in("/bin/zsh", "claude --dangerously-skip-permissions", Some(&link_dir));
        assert!(res.is_err(), "passing a retargeted symlink without .designos-hermetic must be refused, got argv: {:?}", res.map(|c| c.get_argv().to_vec()));

        // 2. Even if `retargeted` also has `.designos-hermetic` and simulated `DESIGNOS_AGENT_CLI_DIR` still points at `link_dir`,
        // `hermetic_agent_cli_dir()` and spawn must use the canonical `orig_stubs` pinned at startup, never `retargeted`.
        std::fs::write(retargeted.join(HERMETIC_MARKER_FILENAME), b"ok\n").unwrap();
        let (pinned_dir, cmd_from_pinned) = {
            let _sim_env = TestAgentCliDirEnv::set(&link_dir);
            let pinned_dir = hermetic_agent_cli_dir();
            let cmd_from_pinned = crate::terminal::build_agent_shell_cmd_in("/bin/zsh", "claude --dangerously-skip-permissions", pinned_dir.as_deref());
            (pinned_dir, cmd_from_pinned)
        };

        assert_eq!(
            pinned_dir.as_deref(),
            Some(orig_canon.as_path()),
            "hermetic_agent_cli_dir() must return the canonical path validated at startup ({}), not the retargeted symlink",
            orig_canon.display()
        );
        let cmd = cmd_from_pinned.expect("spawn with startup-pinned hermetic dir must succeed");
        let argv0 = cmd.get_argv().first().map(PathBuf::from).unwrap_or_default();
        assert_ne!(argv0, retargeted_claude_canon, "hermetic spawn must never resolve to retargeted claude");
        assert_eq!(argv0, orig_claude_canon, "hermetic spawn must resolve to the original startup-validated claude");

        let _ = std::fs::remove_dir_all(&root);
    }

    /// Round 9b: Running the symlink-retarget startup-pin scenario and the H11 scoped version-warmer
    /// scenario concurrently on two threads must never leak the startup-pinned stub path into the
    /// `ScopedCliDirs` thread.
    #[test]
    #[cfg(unix)]
    fn retarget_and_scoped_version_warmer_do_not_race_across_threads() {
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let b1 = std::sync::Arc::clone(&barrier);
        let t_retarget = std::thread::spawn(move || {
            b1.wait();
            for _ in 0..40 {
                hermetic_startup_dir_is_pinned_after_symlink_retarget();
            }
        });
        let b2 = std::sync::Arc::clone(&barrier);
        let t_warmer = std::thread::spawn(move || {
            b2.wait();
            for _ in 0..200 {
                version_warmer_resolves_on_the_calling_thread();
            }
        });
        t_retarget.join().expect("retarget thread must not panic");
        t_warmer.join().expect("scoped version_warmer thread must not panic");
    }

    /// Round 9c + 10b: In live wrappers (`hermetic_mode_active`, `hermetic_agent_cli_dir`, `cli_search_dirs`),
    /// a startup-pinned hermetic directory (precedence 1) must beat an active `ScopedCliDirs` guard
    /// (precedence 2), and `hermetic_mode_active()` under `cfg(test)` must mirror release by returning
    /// `true` when either the thread-local pin or `TestAgentCliDirEnv` is set.
    #[test]
    fn hermetic_pin_beats_scoped_guard_and_mode_active_reads_env_var() {
        let _reset_hermetic = ResetTestHermeticDir::enter();
        let root = make_temp_dir("r9c-precedence");
        let hermetic_stubs = root.join("hermetic-stubs");
        let scoped_stubs = root.join("scoped-stubs");
        let lib = root.join("lib");
        let tmp = root.join("tmp");
        let home = root.join("home");
        for d in [&hermetic_stubs, &scoped_stubs, &lib, &tmp, &home] {
            std::fs::create_dir_all(d).unwrap();
            std::fs::write(d.join(HERMETIC_MARKER_FILENAME), b"ok\n").unwrap();
        }
        let hermetic_canon = std::fs::canonicalize(&hermetic_stubs).unwrap();

        // 1. With no pin set, setting `TestAgentCliDirEnv` makes
        // `hermetic_mode_active()` true while `hermetic_agent_cli_dir()` remains `None` (mirroring release).
        let (active_from_env, dir_from_env_without_pin) = {
            let _sim_env = TestAgentCliDirEnv::set(&hermetic_stubs);
            (hermetic_mode_active(), hermetic_agent_cli_dir())
        };
        assert!(!hermetic_mode_active(), "hermetic_mode_active() must restore to false after TestAgentCliDirEnv drops");
        assert!(active_from_env, "hermetic_mode_active() must be true when TestAgentCliDirEnv is set");
        assert_eq!(dir_from_env_without_pin, None, "hermetic_agent_cli_dir() must remain None before enforce_hermetic_startup pins it");

        // 2. After `enforce_hermetic_startup` pins `hermetic_stubs`, entering `ScopedCliDirs` on the
        // same thread must not hide the hermetic pin: precedence 1 (hermetic) beats precedence 2 (scoped).
        let mut env = BTreeMap::new();
        env.insert("DESIGNOS_AGENT_CLI_DIR".into(), hermetic_stubs.display().to_string());
        env.insert("DESIGNOS_LIBRARY".into(), lib.display().to_string());
        env.insert("TMPDIR".into(), tmp.display().to_string());
        env.insert("HOME".into(), home.display().to_string());
        let shot_opts = crate::cli::Options { screenshot: Some(root.join("out.png")), ..Default::default() };
        enforce_hermetic_startup(&env, &shot_opts, 98765).expect("startup with valid hermetic dir must succeed");

        {
            let _scoped = ScopedCliDirs::enter(&scoped_stubs);
            assert!(hermetic_mode_active(), "hermetic_mode_active() must stay true inside ScopedCliDirs when pinned");
            assert_eq!(
                hermetic_agent_cli_dir().as_deref(),
                Some(hermetic_canon.as_path()),
                "hermetic_agent_cli_dir() must return the pinned hermetic dir even when ScopedCliDirs is active"
            );
            assert_eq!(
                cli_search_dirs(None),
                vec![hermetic_canon.clone()],
                "cli_search_dirs(None) must rank hermetic (precedence 1) above scoped (precedence 2)"
            );
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    /// Round 10b: Thread A holds `TestAgentCliDirEnv::set(...)` and reads `hermetic_mode_active() == true`,
    /// while Thread B (with no pin) reads `hermetic_mode_active() == false`.
    #[test]
    fn simulated_agent_cli_dir_does_not_leak_across_threads() {
        let b_set = std::sync::Arc::new(std::sync::Barrier::new(2));
        let b_done = std::sync::Arc::new(std::sync::Barrier::new(2));
        let b_set_a = std::sync::Arc::clone(&b_set);
        let b_done_a = std::sync::Arc::clone(&b_done);
        let t_a = std::thread::spawn(move || {
            let _reset = ResetTestHermeticDir::enter();
            let active_in_guard = {
                let _sim_env = TestAgentCliDirEnv::set("/tmp/sim-agent-cli-dir");
                let a = hermetic_mode_active();
                b_set_a.wait();
                b_done_a.wait();
                a
            };
            let active_after_drop = hermetic_mode_active();
            (active_in_guard, active_after_drop)
        });
        let t_b = std::thread::spawn(move || {
            let _reset = ResetTestHermeticDir::enter();
            b_set.wait();
            let active = hermetic_mode_active();
            b_done.wait();
            active
        });
        let (a_in_guard, a_after_drop) = t_a.join().expect("thread A must not panic");
        let b_active = t_b.join().expect("thread B must not panic");
        assert!(a_in_guard, "thread A holding TestAgentCliDirEnv must read hermetic_mode_active() == true");
        assert!(!a_after_drop, "thread A must read hermetic_mode_active() == false after TestAgentCliDirEnv drops");
        assert!(!b_active, "thread B with no pin must read hermetic_mode_active() == false while thread A holds TestAgentCliDirEnv");
    }
}
