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

static HERMETIC_DIR: OnceLock<Option<PathBuf>> = OnceLock::new();

pub fn hermetic_agent_cli_dir() -> Option<PathBuf> {
    HERMETIC_DIR.get_or_init(|| None).clone()
}

#[cfg(test)]
pub struct ScopedCliDirs {
    _private: (),
}

#[cfg(test)]
impl ScopedCliDirs {
    pub fn enter(_dir: impl Into<PathBuf>) -> Self {
        Self { _private: () }
    }
}

pub fn split_non_empty_paths(_list: &OsStr) -> Vec<PathBuf> {
    Vec::new()
}

pub fn host_path_dirs() -> Vec<PathBuf> {
    Vec::new()
}

pub fn host_path_is_empty() -> bool {
    false
}

pub fn current_search_sources(explicit: Option<Vec<PathBuf>>) -> SearchSources {
    SearchSources { hermetic: None, scoped: None, explicit, host_path: Vec::new(), extra: crate::terminal::extra_cli_path_dirs() }
}

pub fn search_sources_with_prepend(_path_prepend: &[PathBuf]) -> SearchSources {
    current_search_sources(None)
}

pub fn cli_search_dirs_with(sources: &SearchSources) -> Vec<PathBuf> {
    sources.host_path.clone()
}

pub fn cli_search_dirs(explicit: Option<Vec<PathBuf>>) -> Vec<PathBuf> {
    cli_search_dirs_with(&current_search_sources(explicit))
}

pub fn resolve_in(_dirs: &[PathBuf], _bin: &str) -> Option<PathBuf> {
    None
}

pub fn hermetic_startup_check(_env: &BTreeMap<String, String>, _opts: &crate::cli::Options) -> Result<(), String> {
    Ok(())
}

pub fn write_hermetic_ack(_tmpdir: &Path, dir: &Path, _pid: u32) -> std::io::Result<PathBuf> {
    Ok(dir.to_path_buf())
}

pub fn enforce_hermetic_startup(env: &BTreeMap<String, String>, opts: &crate::cli::Options, pid: u32) -> Result<Option<PathBuf>, String> {
    hermetic_startup_check(env, opts)?;
    let _ = pid;
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::AgentCliPreset;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);

    fn make_temp_dir(tag: &str) -> PathBuf {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("designos-1126-{tag}-{}-{id}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
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
    /// `["/bin/sh", "-c", "exec <DIR>/claude …"]` with no `-l` or `-i`, for every preset and for the Shell preset.
    #[test]
    fn hermetic_shell_cmd_has_no_login_or_interactive_flags() {
        let dir = make_temp_dir("h1");
        for bin in ["claude", "codex", "agy", "grok", "gemini", "opencode"] {
            write_exec_stub(&dir, bin, "#!/bin/sh\nexit 0\n");
        }
        for (preset, launch) in [
            (AgentCliPreset::Claude, "claude --dangerously-skip-permissions"),
            (AgentCliPreset::Codex, "codex --full-auto"),
            (AgentCliPreset::Antigravity, "agy --add-dir /tmp"),
            (AgentCliPreset::Grok, "grok --cwd /tmp"),
            (AgentCliPreset::Gemini, "gemini --approval-mode yolo"),
            (AgentCliPreset::OpenCode, "opencode"),
        ] {
            let cmd = crate::terminal::build_agent_shell_cmd_in("/bin/zsh", launch, Some(&dir))
                .unwrap_or_else(|e| panic!("preset {preset:?} must build in hermetic mode: {e}"));
            let argv: Vec<String> = cmd.get_argv().iter().map(|s| s.to_string_lossy().into_owned()).collect();
            let expected_bin = dir.join(preset.binary_name());
            let rest = &launch[preset.binary_name().len()..];
            assert_eq!(
                argv,
                vec!["/bin/sh".to_string(), "-c".to_string(), format!("exec {}{rest}", expected_bin.display()),],
                "preset {preset:?} argv must use /bin/sh -c without -l or -i"
            );
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

    /// H10: Scans `crates/designos-app/src/**/*.rs`: `split_paths(` and `var_os("PATH")` appear
    /// only in `cli_resolve.rs` and `configure_pty_env_with_session`.
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

        let mut violations = Vec::new();
        for file in files {
            let rel = file.strip_prefix(&src_dir).unwrap_or(&file);
            if rel == Path::new("cli_resolve.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&file).expect("read source file");
            let is_terminal = rel == Path::new("terminal.rs");
            let mut in_configure_pty_env = false;
            let mut brace_depth: i32 = 0;

            for (idx, line) in text.lines().enumerate() {
                let line_no = idx + 1;
                if is_terminal && line.contains("pub fn configure_pty_env_with_session") {
                    in_configure_pty_env = true;
                    brace_depth = 0;
                }
                if in_configure_pty_env {
                    brace_depth += line.chars().filter(|&c| c == '{').count() as i32;
                    brace_depth -= line.chars().filter(|&c| c == '}').count() as i32;
                }
                let code_part = line.split("//").next().unwrap_or("");
                if !in_configure_pty_env && (code_part.contains(&split_needle) || code_part.contains(&var_os_needle)) {
                    violations.push(format!("{}:{line_no}: {}", rel.display(), line.trim()));
                }
                if in_configure_pty_env && brace_depth <= 0 && line.contains('}') {
                    in_configure_pty_env = false;
                }
            }
        }
        assert!(
            violations.is_empty(),
            "found split_paths( or var_os(\"PATH\") outside cli_resolve.rs and configure_pty_env_with_session:\n{}",
            violations.join("\n")
        );
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
}
