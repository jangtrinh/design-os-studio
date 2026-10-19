//! Catalog of agent CLIs, background version/sign-in probes (`RunCommand`), and deterministic
//! row ordering for the new-session picker and compact `+` menu (`#652`).
//!
//! Behaviour rules (`docs/specs/v0.6/652-new-session-picker-installed-clis-version-login.md` §3):
//! - Four built-in CLI descriptors (`Claude`, `Codex`, `Grok`, `Antigravity`) plus custom CLI
//!   entries (`custom_clis` in Studio's `settings.json`), `"Other CLI..."`, and `"Shell"` last.
//! - Probes run on background worker threads, never on the UI thread, with a 3 s timeout per call.
//! - Claude sign-in parses only the boolean `loggedIn` field of `claude auth status` JSON (`true`
//!   -> `Signed in`, `false` -> `Not signed in`, missing/non-boolean/non-JSON -> `Sign-in not checked`),
//!   never storing or logging account names or emails.
//! - Codex sign-in parses `codex login status` exit 0 with a line starting `"Logged in"` -> `Signed in`;
//!   every other output or exit code -> `Sign-in not checked`.
//! - Grok and Antigravity have no status command -> `Sign-in not checked`.

use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::agent_start_failure::{install_url, login_command, FixAction};
use crate::terminal::{extra_cli_path_dirs, runtime_binary_available, AgentCliPreset};

/// Per-command timeout for version and sign-in probes (`#652` §3 rule 2).
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// Warning caption shown on `"Other CLI..."` and saved custom CLI rows (`#652` §3 rule 7).
pub const CUSTOM_CLI_WARNING: &str = "Plain command: Studio tools not connected";

/// Default footer permission caption (`#652` §3 rule 9).
pub const DEFAULT_PERMISSION_CAPTION: &str = "Skip permissions";

/// Sign-in state reported for a CLI descriptor (`#652` §4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignIn {
    Yes,
    No,
    Unknown,
    Checking,
}

impl SignIn {
    /// Human-readable status pill label (`#652` §3 rule 3).
    pub fn label(self) -> &'static str {
        match self {
            Self::Yes => "Signed in",
            Self::No => "Not signed in",
            Self::Unknown => "Sign-in not checked",
            Self::Checking => "Checking...",
        }
    }
}

/// Cached probe result for one built-in CLI descriptor (`#652` §4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliProbe {
    pub installed: bool,
    pub version: Option<String>,
    pub signed_in: SignIn,
}

impl CliProbe {
    /// Initial state while a background probe is in flight for an installed binary.
    pub fn checking(installed: bool) -> Self {
        Self { installed, version: None, signed_in: if installed { SignIn::Checking } else { SignIn::Unknown } }
    }
}

/// Static descriptor for a built-in agent CLI (`#652` §3 rule 1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliDescriptor {
    pub preset: AgentCliPreset,
    pub name: &'static str,
    pub binary: &'static str,
    pub install_folders: Vec<PathBuf>,
    pub version_cmd: &'static [&'static str],
    pub sign_in_cmd: Option<&'static [&'static str]>,
    pub install_url: &'static str,
    pub login_command: &'static str,
}

/// Saved custom CLI command in Studio's `settings.json` (`#652` §4).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomCliEntry {
    pub label: String,
    pub command: String,
}

/// Fixed catalog order of the four built-in agent CLIs (`#652` §3 rules 1 & 4).
pub const CATALOG_PRESETS: [AgentCliPreset; 4] = [AgentCliPreset::Claude, AgentCliPreset::Codex, AgentCliPreset::Grok, AgentCliPreset::Antigravity];

/// Returns the catalog of built-in CLI descriptors in fixed order (`Claude`, `Codex`, `Grok`, `Antigravity`).
pub fn catalog() -> Vec<CliDescriptor> {
    vec![
        CliDescriptor {
            preset: AgentCliPreset::Claude,
            name: "Claude",
            binary: "claude",
            install_folders: AgentCliPreset::Claude.install_dirs(),
            version_cmd: &["claude", "--version"],
            sign_in_cmd: Some(&["claude", "auth", "status"]),
            install_url: install_url(AgentCliPreset::Claude),
            login_command: login_command(AgentCliPreset::Claude),
        },
        CliDescriptor {
            preset: AgentCliPreset::Codex,
            name: "Codex",
            binary: "codex",
            install_folders: AgentCliPreset::Codex.install_dirs(),
            version_cmd: &["codex", "--version"],
            sign_in_cmd: Some(&["codex", "login", "status"]),
            install_url: install_url(AgentCliPreset::Codex),
            login_command: login_command(AgentCliPreset::Codex),
        },
        CliDescriptor {
            preset: AgentCliPreset::Grok,
            name: "Grok",
            binary: "grok",
            install_folders: AgentCliPreset::Grok.install_dirs(),
            version_cmd: &["grok", "--version"],
            sign_in_cmd: None,
            install_url: install_url(AgentCliPreset::Grok),
            login_command: login_command(AgentCliPreset::Grok),
        },
        CliDescriptor {
            preset: AgentCliPreset::Antigravity,
            name: "Antigravity",
            binary: "agy",
            install_folders: AgentCliPreset::Antigravity.install_dirs(),
            version_cmd: &["agy", "--version"],
            sign_in_cmd: None,
            install_url: install_url(AgentCliPreset::Antigravity),
            login_command: login_command(AgentCliPreset::Antigravity),
        },
    ]
}

/// Output of a single subprocess invocation in [`RunCommand`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

impl CommandOutput {
    pub fn ok(stdout: impl Into<String>) -> Self {
        Self { exit_code: Some(0), stdout: stdout.into(), stderr: String::new(), timed_out: false }
    }

    pub fn exited(code: i32, stdout: impl Into<String>, stderr: impl Into<String>) -> Self {
        Self { exit_code: Some(code), stdout: stdout.into(), stderr: stderr.into(), timed_out: false }
    }

    pub fn timeout() -> Self {
        Self { exit_code: None, stdout: String::new(), stderr: String::new(), timed_out: true }
    }
}

/// Seam for running CLI install/version/sign-in probes (`#652` §10).
pub trait RunCommand: Send + Sync {
    /// Checks whether `binary` is installed in `install_folders` or on `PATH`.
    fn is_installed(&self, binary: &str, install_folders: &[PathBuf]) -> bool;

    /// Runs `argv` with `timeout`, returning stdout/stderr/exit status.
    fn run(&self, argv: &[&str], install_folders: &[PathBuf], timeout: Duration) -> CommandOutput;
}

/// Real OS subprocess runner used by Studio at runtime.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemCommandRunner;

impl SystemCommandRunner {
    fn resolve_binary(binary: &str, install_folders: &[PathBuf]) -> Option<PathBuf> {
        let trimmed = binary.trim();
        if trimmed.is_empty() {
            return None;
        }
        let mut dirs: Vec<PathBuf> = Vec::new();
        if let Some(path_os) = std::env::var_os("PATH") {
            for p in std::env::split_paths(&path_os) {
                if !p.as_os_str().is_empty() {
                    dirs.push(p);
                }
            }
        }
        dirs.extend(install_folders.iter().cloned());
        dirs.extend(extra_cli_path_dirs());

        for d in dirs {
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
}

impl RunCommand for SystemCommandRunner {
    fn is_installed(&self, binary: &str, install_folders: &[PathBuf]) -> bool {
        if runtime_binary_available(binary) {
            return true;
        }
        Self::resolve_binary(binary, install_folders).is_some()
    }

    fn run(&self, argv: &[&str], install_folders: &[PathBuf], timeout: Duration) -> CommandOutput {
        let Some((&prog, args)) = argv.split_first() else {
            return CommandOutput::exited(127, "", "empty argv");
        };
        let resolved = Self::resolve_binary(prog, install_folders).unwrap_or_else(|| PathBuf::from(prog));
        let mut cmd = Command::new(resolved);
        cmd.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut extra_paths: Vec<String> = install_folders.iter().map(|p| p.to_string_lossy().into_owned()).collect();
        extra_paths.extend(extra_cli_path_dirs().into_iter().map(|p| p.to_string_lossy().into_owned()));
        if !extra_paths.is_empty() {
            let current_path = std::env::var("PATH").unwrap_or_default();
            let sep = if cfg!(windows) { ";" } else { ":" };
            let combined = if current_path.is_empty() { extra_paths.join(sep) } else { format!("{}{sep}{current_path}", extra_paths.join(sep)) };
            cmd.env("PATH", combined);
        }

        let Ok(mut child) = cmd.spawn() else {
            return CommandOutput::exited(127, "", "failed to spawn");
        };

        let mut stdout_pipe = child.stdout.take();
        let mut stderr_pipe = child.stderr.take();
        let stdout_handle = std::thread::spawn(move || {
            let mut buf = String::new();
            if let Some(ref mut out) = stdout_pipe {
                let _ = out.read_to_string(&mut buf);
            }
            buf
        });
        let stderr_handle = std::thread::spawn(move || {
            let mut buf = String::new();
            if let Some(ref mut err) = stderr_pipe {
                let _ = err.read_to_string(&mut buf);
            }
            buf
        });

        let start = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let stdout = stdout_handle.join().unwrap_or_default();
                    let stderr = stderr_handle.join().unwrap_or_default();
                    return CommandOutput { exit_code: status.code(), stdout, stderr, timed_out: false };
                }
                Ok(None) => {
                    if start.elapsed() >= timeout {
                        let _ = child.kill();
                        let _ = child.wait();
                        return CommandOutput::timeout();
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(_) => {
                    let _ = child.kill();
                    return CommandOutput::exited(1, "", "wait error");
                }
            }
        }
    }
}

/// Extracts the first dotted numeric version token (`<digits>.<digits>[.<digits>...]`) from `raw` (`#652` §3 rule 2).
///
/// Examples:
/// - `"2.1.302 (Claude Code)"` -> `Some("2.1.302")`
/// - `"codex-cli 0.163.0"` -> `Some("0.163.0")`
/// - `"grok 1.0.62 (1474f9cbdd91) [stable]"` -> `Some("1.0.62")`
/// - `"1.4.2"` -> `Some("1.4.2")`
pub fn parse_version(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    let n = bytes.len();
    let mut i = 0;
    while i < n {
        if bytes[i].is_ascii_digit() {
            let start = i;
            let mut has_dot = false;
            while i < n && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                if bytes[i] == b'.' {
                    if i == start || i + 1 >= n || !bytes[i + 1].is_ascii_digit() {
                        break;
                    }
                    has_dot = true;
                }
                i += 1;
            }
            if has_dot {
                let candidate = &raw[start..i];
                if candidate.split('.').all(|seg| !seg.is_empty() && seg.chars().all(|c| c.is_ascii_digit())) {
                    return Some(candidate.to_string());
                }
            }
        } else {
            i += 1;
        }
    }
    None
}

/// Parses `claude auth status` JSON for the boolean `loggedIn` field ONLY (`#652` §3 rule 3 & §5).
/// Account details (emails, orgs, tokens) are never read, stored, or logged.
pub fn parse_claude_sign_in(stdout: &str) -> SignIn {
    let trimmed = stdout.trim();
    // Find JSON object if preceded/followed by any noise, or parse directly.
    let json_slice = if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if start <= end {
            &trimmed[start..=end]
        } else {
            trimmed
        }
    } else {
        return SignIn::Unknown;
    };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(json_slice) else {
        return SignIn::Unknown;
    };
    match val.get("loggedIn").and_then(|v| v.as_bool()) {
        Some(true) => SignIn::Yes,
        Some(false) => SignIn::No,
        None => SignIn::Unknown,
    }
}

/// Parses `codex login status` output (`#652` §3 rule 3):
/// `Signed in` ONLY when the command exits 0 and prints a line starting with `"Logged in"`.
/// Every other output or exit code returns `SignIn::Unknown` (`"Sign-in not checked"`).
pub fn parse_codex_sign_in(exit_ok: bool, stdout: &str, stderr: &str) -> SignIn {
    if !exit_ok {
        return SignIn::Unknown;
    }
    let has_logged_in_line = stdout.lines().chain(stderr.lines()).map(str::trim).any(|line| line.starts_with("Logged in"));
    if has_logged_in_line {
        SignIn::Yes
    } else {
        SignIn::Unknown
    }
}

/// Runs `runner.run(...)` with a hard wall-clock `timeout` enforced even if `runner.run` blocks.
fn run_enforcing_timeout(runner: &Arc<dyn RunCommand>, argv: &'static [&'static str], install_folders: &[PathBuf], timeout: Duration) -> CommandOutput {
    let runner_cl = Arc::clone(runner);
    let folders = install_folders.to_vec();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let out = runner_cl.run(argv, &folders, timeout);
        let _ = tx.send(out);
    });
    match rx.recv_timeout(timeout) {
        Ok(out) => out,
        Err(_) => CommandOutput::timeout(),
    }
}

/// Probes one [`CliDescriptor`] using `runner` and `timeout` (`#652` §3 rules 2 & 3).
pub fn probe_descriptor(desc: &CliDescriptor, runner: &Arc<dyn RunCommand>, timeout: Duration) -> CliProbe {
    if !runner.is_installed(desc.binary, &desc.install_folders) {
        return CliProbe { installed: false, version: None, signed_in: SignIn::Unknown };
    }

    let ver_out = run_enforcing_timeout(runner, desc.version_cmd, &desc.install_folders, timeout);
    let version = if ver_out.timed_out { None } else { parse_version(&ver_out.stdout).or_else(|| parse_version(&ver_out.stderr)) };

    let signed_in = match desc.sign_in_cmd {
        None => SignIn::Unknown,
        Some(cmd) => {
            let status_out = run_enforcing_timeout(runner, cmd, &desc.install_folders, timeout);
            if status_out.timed_out {
                SignIn::Unknown
            } else {
                match desc.preset {
                    AgentCliPreset::Claude => parse_claude_sign_in(&status_out.stdout),
                    AgentCliPreset::Codex => parse_codex_sign_in(status_out.exit_code == Some(0), &status_out.stdout, &status_out.stderr),
                    _ => SignIn::Unknown,
                }
            }
        }
    };

    CliProbe { installed: true, version, signed_in }
}

/// In-memory cache of [`CliProbe`] results per [`AgentCliPreset`], refreshed asynchronously off the UI thread (`#652` §3 rule 2).
#[derive(Clone)]
pub struct CatalogProbeCache {
    probes: Arc<Mutex<HashMap<AgentCliPreset, CliProbe>>>,
    in_flight: Arc<Mutex<bool>>,
    runner: Arc<dyn RunCommand>,
    timeout: Duration,
}

impl Default for CatalogProbeCache {
    fn default() -> Self {
        Self::new(Arc::new(SystemCommandRunner), PROBE_TIMEOUT)
    }
}

impl CatalogProbeCache {
    pub fn new(runner: Arc<dyn RunCommand>, timeout: Duration) -> Self {
        Self { probes: Arc::new(Mutex::new(HashMap::new())), in_flight: Arc::new(Mutex::new(false)), runner, timeout }
    }

    /// Triggers an asynchronous background refresh of all catalog descriptors off the UI thread.
    /// Existing cached probes remain visible immediately; if no probe has been cached yet for a
    /// preset, seeds `Checking...` so the first UI frame returns immediately without blocking.
    pub fn refresh_async(&self) {
        let descriptors = catalog();
        {
            let mut map = self.probes.lock().unwrap();
            for desc in &descriptors {
                map.entry(desc.preset).or_insert_with(|| CliProbe { installed: true, version: None, signed_in: SignIn::Checking });
            }
        }
        if let Ok(mut active) = self.in_flight.lock() {
            *active = true;
        }

        let probes_ref = Arc::clone(&self.probes);
        let in_flight_ref = Arc::clone(&self.in_flight);
        let runner = Arc::clone(&self.runner);
        let timeout = self.timeout;

        std::thread::spawn(move || {
            let mut handles = Vec::with_capacity(descriptors.len());
            for desc in descriptors {
                let r = Arc::clone(&runner);
                let p_map = Arc::clone(&probes_ref);
                handles.push(std::thread::spawn(move || {
                    let res = probe_descriptor(&desc, &r, timeout);
                    if let Ok(mut guard) = p_map.lock() {
                        guard.insert(desc.preset, res);
                    }
                }));
            }
            for h in handles {
                let _ = h.join();
            }
            if let Ok(mut active) = in_flight_ref.lock() {
                *active = false;
            }
        });
    }

    /// Runs a synchronous probe of all catalog descriptors (used by deterministic unit tests).
    pub fn refresh_blocking(&self) {
        for desc in catalog() {
            let res = probe_descriptor(&desc, &self.runner, self.timeout);
            if let Ok(mut guard) = self.probes.lock() {
                guard.insert(desc.preset, res);
            }
        }
        if let Ok(mut active) = self.in_flight.lock() {
            *active = false;
        }
    }

    /// True while a background probe batch is still running.
    pub fn is_probing(&self) -> bool {
        self.in_flight.lock().map(|g| *g).unwrap_or(false)
    }

    /// Returns a snapshot of current probes for all catalog presets.
    pub fn snapshot(&self) -> HashMap<AgentCliPreset, CliProbe> {
        self.probes.lock().map(|g| g.clone()).unwrap_or_default()
    }

    /// Overwrites a single preset's cached probe (useful for UI tests and captures).
    pub fn set_probe(&self, preset: AgentCliPreset, probe: CliProbe) {
        if let Ok(mut guard) = self.probes.lock() {
            guard.insert(preset, probe);
        }
    }
}

/// Kind of row in the new-session picker and compact `+` menu (`#652` §3 rules 3–8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickerRowKind {
    CatalogCli {
        preset: AgentCliPreset,
        binary: &'static str,
        installed: bool,
        version: Option<String>,
        signed_in: SignIn,
        is_last_choice: bool,
        install_action: Option<FixAction>,
    },
    CustomCli {
        entry: CustomCliEntry,
    },
    OtherCli,
    Shell,
}

/// One ordered row in the new-session picker or compact `+` menu (`#652` §3 rules 3–8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickerRow {
    pub id: String,
    pub label: String,
    pub version: Option<String>,
    pub state_label: Option<String>,
    pub caption: Option<String>,
    pub selectable: bool,
    pub is_last_choice: bool,
    pub install_action: Option<FixAction>,
    pub kind: PickerRowKind,
}

/// Builds the ordered list of picker rows according to `#652` §3 rules 3, 4, 6, 7, 8:
/// 1. Installed CLIs first in fixed catalog order (`Claude`, `Codex`, `Grok`, `Antigravity`),
///    with the file's `last_cli` ordered first when present and installed;
/// 2. Uninstalled CLIs after them, greyed (`Not installed`, `selectable: false`) with `FixAction::OpenInstallPage`;
/// 3. Saved `custom_clis` entries (with caption `"Plain command: Studio tools not connected"`);
/// 4. `"Other CLI..."` row (with caption `"Plain command: Studio tools not connected"`);
/// 5. `"Shell"` row last (never the default).
pub fn build_picker_rows(probes: &HashMap<AgentCliPreset, CliProbe>, last_cli: Option<&str>, custom_clis: &[CustomCliEntry]) -> Vec<PickerRow> {
    let descriptors = catalog();
    let last_clean = last_cli.map(str::trim).filter(|s| !s.is_empty());

    let mut installed_rows: Vec<PickerRow> = Vec::new();
    let mut uninstalled_rows: Vec<PickerRow> = Vec::new();

    for desc in descriptors {
        let probe = probes.get(&desc.preset).cloned().unwrap_or_else(|| {
            let inst = cfg!(test) || runtime_binary_available(desc.binary);
            CliProbe::checking(inst)
        });
        let id = match desc.preset {
            AgentCliPreset::Claude => "agents-dropdown-claude".to_string(),
            AgentCliPreset::Codex => "agents-dropdown-codex".to_string(),
            AgentCliPreset::Grok => "agents-dropdown-grok".to_string(),
            AgentCliPreset::Antigravity => "agents-dropdown-antigravity".to_string(),
            _ => format!("agents-dropdown-{}", desc.binary),
        };

        if probe.installed {
            let is_last = last_clean == Some(desc.binary);
            installed_rows.push(PickerRow {
                id,
                label: desc.name.to_string(),
                version: probe.version.clone(),
                state_label: Some(probe.signed_in.label().to_string()),
                caption: None,
                selectable: true,
                is_last_choice: is_last,
                install_action: None,
                kind: PickerRowKind::CatalogCli {
                    preset: desc.preset,
                    binary: desc.binary,
                    installed: true,
                    version: probe.version,
                    signed_in: probe.signed_in,
                    is_last_choice: is_last,
                    install_action: None,
                },
            });
        } else {
            let action = FixAction::OpenInstallPage(desc.install_url);
            uninstalled_rows.push(PickerRow {
                id,
                label: desc.name.to_string(),
                version: None,
                state_label: Some("Not installed".to_string()),
                caption: None,
                selectable: false,
                is_last_choice: false,
                install_action: Some(action.clone()),
                kind: PickerRowKind::CatalogCli {
                    preset: desc.preset,
                    binary: desc.binary,
                    installed: false,
                    version: None,
                    signed_in: SignIn::Unknown,
                    is_last_choice: false,
                    install_action: Some(action),
                },
            });
        }
    }

    // Rule 4: If the file's `last_cli` is among the installed CLIs, move it to the front while
    // preserving the relative fixed order of the remaining installed CLIs.
    if let Some(pos) = installed_rows.iter().position(|r| r.is_last_choice) {
        if pos > 0 {
            let chosen = installed_rows.remove(pos);
            installed_rows.insert(0, chosen);
        }
    }

    let mut rows = Vec::with_capacity(installed_rows.len() + uninstalled_rows.len() + custom_clis.len() + 2);
    rows.extend(installed_rows);
    rows.extend(uninstalled_rows);

    for (idx, entry) in custom_clis.iter().enumerate() {
        let display_label = if entry.label.trim().is_empty() { entry.command.clone() } else { entry.label.clone() };
        rows.push(PickerRow {
            id: format!("agents-dropdown-custom-{idx}"),
            label: display_label,
            version: None,
            state_label: None,
            caption: Some(CUSTOM_CLI_WARNING.to_string()),
            selectable: true,
            is_last_choice: false,
            install_action: None,
            kind: PickerRowKind::CustomCli { entry: entry.clone() },
        });
    }

    rows.push(PickerRow {
        id: "agents-dropdown-other-cli".to_string(),
        label: "Other CLI...".to_string(),
        version: None,
        state_label: None,
        caption: Some(CUSTOM_CLI_WARNING.to_string()),
        selectable: true,
        is_last_choice: false,
        install_action: None,
        kind: PickerRowKind::OtherCli,
    });

    rows.push(PickerRow {
        id: "agents-dropdown-shell".to_string(),
        label: "Shell".to_string(),
        version: None,
        state_label: None,
        caption: None,
        selectable: true,
        is_last_choice: false,
        install_action: None,
        kind: PickerRowKind::Shell,
    });

    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comments::settings::{add_custom_cli, load_custom_clis};
    use crate::terminal::TerminalState;

    #[derive(Default)]
    struct FakeRunner {
        installed: HashMap<String, bool>,
        responses: HashMap<Vec<String>, CommandOutput>,
        delay: Duration,
    }

    impl FakeRunner {
        fn with_all_installed() -> Self {
            let mut r = Self::default();
            for bin in ["claude", "codex", "grok", "agy"] {
                r.installed.insert(bin.to_string(), true);
            }
            r.responses.insert(vec!["claude".into(), "--version".into()], CommandOutput::ok("2.1.302 (Claude Code)\n"));
            r.responses.insert(vec!["codex".into(), "--version".into()], CommandOutput::ok("codex-cli 0.163.0\n"));
            r.responses.insert(vec!["grok".into(), "--version".into()], CommandOutput::ok("grok 1.0.62 (1474f9cbdd91) [stable]\n"));
            r.responses.insert(vec!["agy".into(), "--version".into()], CommandOutput::ok("1.4.2\n"));
            r.responses
                .insert(vec!["claude".into(), "auth".into(), "status".into()], CommandOutput::ok(r#"{"loggedIn": true, "email": "do-not-store@example.com"}"#));
            r.responses.insert(vec!["codex".into(), "login".into(), "status".into()], CommandOutput::ok("Logged in using ChatGPT\n"));
            r
        }
    }

    impl RunCommand for FakeRunner {
        fn is_installed(&self, binary: &str, _install_folders: &[PathBuf]) -> bool {
            self.installed.get(binary).copied().unwrap_or(false)
        }

        fn run(&self, argv: &[&str], _install_folders: &[PathBuf], _timeout: Duration) -> CommandOutput {
            if !self.delay.is_zero() {
                std::thread::sleep(self.delay);
            }
            let key: Vec<String> = argv.iter().map(|s| s.to_string()).collect();
            self.responses.get(&key).cloned().unwrap_or_else(|| CommandOutput::exited(1, "", "not configured"))
        }
    }

    fn temp_test_dir(tag: &str) -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("dos-652-cat-{tag}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    // 1. `version_parsing_for_the_four_real_outputs`
    #[test]
    fn version_parsing_for_the_four_real_outputs() {
        assert_eq!(parse_version("2.1.302 (Claude Code)").as_deref(), Some("2.1.302"));
        assert_eq!(parse_version("codex-cli 0.163.0").as_deref(), Some("0.163.0"));
        assert_eq!(parse_version("grok 1.0.62 (1474f9cbdd91) [stable]").as_deref(), Some("1.0.62"));
        assert_eq!(parse_version("1.4.2").as_deref(), Some("1.4.2"));
    }

    // 2. `installed_cli_row_shows_version_and_state`
    #[test]
    fn installed_cli_row_shows_version_and_state() {
        let runner: Arc<dyn RunCommand> = Arc::new(FakeRunner::with_all_installed());
        let cache = CatalogProbeCache::new(runner, PROBE_TIMEOUT);
        cache.refresh_blocking();
        let rows = build_picker_rows(&cache.snapshot(), None, &[]);
        let claude_row = rows.iter().find(|r| r.label == "Claude").expect("Claude row");
        assert_eq!(claude_row.version.as_deref(), Some("2.1.302"));
        assert_eq!(claude_row.state_label.as_deref(), Some("Signed in"));
        assert!(claude_row.selectable);
        assert!(!format!("{claude_row:?}").contains("do-not-store@example.com"), "account details must never be stored");
    }

    // 3. `claude_signed_out_json`
    #[test]
    fn claude_signed_out_json() {
        assert_eq!(parse_claude_sign_in(r#"{"loggedIn": false}"#), SignIn::No);
        assert_eq!(parse_claude_sign_in(r#"{"loggedIn": false}"#).label(), "Not signed in");
        // Missing field, non-boolean field, or non-JSON -> Sign-in not checked (never a guess)
        assert_eq!(parse_claude_sign_in(r#"{"account": "none"}"#), SignIn::Unknown);
        assert_eq!(parse_claude_sign_in(r#"{"loggedIn": "false"}"#), SignIn::Unknown);
        assert_eq!(parse_claude_sign_in("not json output"), SignIn::Unknown);
        assert_eq!(SignIn::Unknown.label(), "Sign-in not checked");
    }

    // 4. `codex_only_the_captured_signed_in_phrase_counts`
    #[test]
    fn codex_only_the_captured_signed_in_phrase_counts() {
        assert_eq!(parse_codex_sign_in(true, "Logged in using ChatGPT\n", ""), SignIn::Yes);
        assert_eq!(parse_codex_sign_in(true, "Logged in using ChatGPT\n", "").label(), "Signed in");
        // Non-zero exit or any other output -> Sign-in not checked
        assert_eq!(parse_codex_sign_in(false, "Logged in using ChatGPT\n", ""), SignIn::Unknown);
        assert_eq!(parse_codex_sign_in(true, "Not logged in\n", ""), SignIn::Unknown);
        assert_eq!(parse_codex_sign_in(false, "Not logged in\n", ""), SignIn::Unknown);
        assert_eq!(parse_codex_sign_in(true, "", ""), SignIn::Unknown);
    }

    // 5. `cli_without_status_command_is_not_checked`
    #[test]
    fn cli_without_status_command_is_not_checked() {
        let runner: Arc<dyn RunCommand> = Arc::new(FakeRunner::with_all_installed());
        let cache = CatalogProbeCache::new(runner, PROBE_TIMEOUT);
        cache.refresh_blocking();
        let snap = cache.snapshot();
        let grok = snap.get(&AgentCliPreset::Grok).expect("grok probe");
        let agy = snap.get(&AgentCliPreset::Antigravity).expect("agy probe");
        assert_eq!(grok.signed_in, SignIn::Unknown);
        assert_eq!(grok.signed_in.label(), "Sign-in not checked");
        assert_eq!(agy.signed_in, SignIn::Unknown);
        assert_eq!(agy.signed_in.label(), "Sign-in not checked");
    }

    // 6. `shell_row_is_last_and_other_cli_row_says_studio_tools_not_connected`
    #[test]
    fn shell_row_is_last_and_other_cli_row_says_studio_tools_not_connected() {
        let runner: Arc<dyn RunCommand> = Arc::new(FakeRunner::with_all_installed());
        let cache = CatalogProbeCache::new(runner, PROBE_TIMEOUT);
        cache.refresh_blocking();
        let rows = build_picker_rows(&cache.snapshot(), None, &[]);
        let last = rows.last().expect("non-empty rows");
        assert_eq!(last.label, "Shell");
        assert_eq!(last.kind, PickerRowKind::Shell);
        assert!(!last.is_last_choice, "Shell is never the default");

        let other = rows.iter().find(|r| r.kind == PickerRowKind::OtherCli).expect("Other CLI row");
        assert_eq!(other.label, "Other CLI...");
        assert_eq!(other.caption.as_deref(), Some("Plain command: Studio tools not connected"));
    }

    // 8. `custom_cli_is_saved_for_all_files`
    #[test]
    fn custom_cli_is_saved_for_all_files() {
        let root = temp_test_dir("custom-cli-all-files");
        let mut file_a = TerminalState::for_tab(1);
        file_a.bind_library_file(&root, "file-a", "win-a");
        let mut file_b = TerminalState::for_tab(2);
        file_b.bind_library_file(&root, "file-b", "win-b");

        // Add custom CLI while working in file A
        add_custom_cli(&root, "Aider", "aider --model sonnet").expect("save custom cli");

        // Listed in file B via the shared Studio settings.json
        let loaded_for_b = load_custom_clis(&root);
        assert_eq!(loaded_for_b, vec![CustomCliEntry { label: "Aider".to_string(), command: "aider --model sonnet".to_string() }]);

        let rows_b = build_picker_rows(&HashMap::new(), file_b.session_registry.last_cli.as_deref(), &loaded_for_b);
        let custom_row = rows_b.iter().find(|r| r.label == "Aider").expect("Aider listed in file B");
        assert_eq!(custom_row.caption.as_deref(), Some("Plain command: Studio tools not connected"));

        // Never stored in any file's sessions.json registry
        assert!(file_a.session_registry.sessions.is_empty());
        assert!(file_b.session_registry.sessions.is_empty());
        assert!(!file_a.session_dir.join("sessions.json").exists());
        assert!(!file_b.session_dir.join("sessions.json").exists());

        file_a.shutdown();
        file_b.shutdown();
        let _ = std::fs::remove_dir_all(&root);
    }

    // 9. `missing_binary_is_greyed_with_an_install_link`
    #[test]
    fn missing_binary_is_greyed_with_an_install_link() {
        let mut fake = FakeRunner::with_all_installed();
        fake.installed.insert("grok".to_string(), false);
        let runner: Arc<dyn RunCommand> = Arc::new(fake);
        let cache = CatalogProbeCache::new(runner, PROBE_TIMEOUT);
        cache.refresh_blocking();
        let rows = build_picker_rows(&cache.snapshot(), None, &[]);

        // Installed CLIs (Claude, Codex, Antigravity) come before uninstalled (Grok)
        let labels: Vec<&str> = rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(&labels[..4], &["Claude", "Codex", "Antigravity", "Grok"]);

        let grok_row = &rows[3];
        assert_eq!(grok_row.state_label.as_deref(), Some("Not installed"));
        assert!(!grok_row.selectable, "missing binary row must not be selectable");
        assert_eq!(grok_row.install_action, Some(FixAction::OpenInstallPage(install_url(AgentCliPreset::Grok))));
    }

    // 10. `probe_runs_off_the_ui_thread`
    #[test]
    fn probe_runs_off_the_ui_thread() {
        struct BlockingRunner {
            started_tx: Mutex<Option<mpsc::Sender<()>>>,
            release_rx: Mutex<Option<mpsc::Receiver<()>>>,
        }
        impl RunCommand for BlockingRunner {
            fn is_installed(&self, _binary: &str, _install_folders: &[PathBuf]) -> bool {
                true
            }
            fn run(&self, _argv: &[&str], _install_folders: &[PathBuf], _timeout: Duration) -> CommandOutput {
                if let Some(tx) = self.started_tx.lock().unwrap().take() {
                    let _ = tx.send(());
                }
                if let Some(rx) = self.release_rx.lock().unwrap().take() {
                    let _ = rx.recv_timeout(Duration::from_secs(2));
                }
                CommandOutput::ok("2.1.302 (Claude Code)\n")
            }
        }

        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let runner: Arc<dyn RunCommand> = Arc::new(BlockingRunner { started_tx: Mutex::new(Some(started_tx)), release_rx: Mutex::new(Some(release_rx)) });
        let cache = CatalogProbeCache::new(runner, PROBE_TIMEOUT);

        let t0 = Instant::now();
        cache.refresh_async();
        // First frame snapshot returns immediately before the blocking runner finishes
        let first_frame_rows = build_picker_rows(&cache.snapshot(), None, &[]);
        assert!(t0.elapsed() < Duration::from_millis(200), "refresh_async must not block the UI thread");
        assert!(first_frame_rows.iter().any(|r| r.label == "Claude" && r.state_label.as_deref() == Some("Checking...")));

        // Wait for background worker to enter `run`, then release it
        let _ = started_rx.recv_timeout(Duration::from_secs(2));
        let _ = release_tx.send(());

        let deadline = Instant::now() + Duration::from_secs(2);
        while cache.is_probing() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        let updated_rows = build_picker_rows(&cache.snapshot(), None, &[]);
        let claude = updated_rows.iter().find(|r| r.label == "Claude").unwrap();
        assert_eq!(claude.version.as_deref(), Some("2.1.302"));
    }

    // 11. `probe_timeout_degrades_to_unknown`
    #[test]
    fn probe_timeout_degrades_to_unknown() {
        let mut fake = FakeRunner::with_all_installed();
        // Simulate a runner that sleeps past the probe timeout
        fake.delay = Duration::from_millis(120);
        let runner: Arc<dyn RunCommand> = Arc::new(fake);
        let short_timeout = Duration::from_millis(30);
        let desc = &catalog()[0]; // Claude
        let probe = probe_descriptor(desc, &runner, short_timeout);
        assert!(probe.installed);
        assert_eq!(probe.signed_in, SignIn::Unknown);
        assert_eq!(probe.signed_in.label(), "Sign-in not checked");
    }

    // 12. `last_choice_orders_first_and_is_per_file`
    #[test]
    fn last_choice_orders_first_and_is_per_file() {
        let runner: Arc<dyn RunCommand> = Arc::new(FakeRunner::with_all_installed());
        let cache = CatalogProbeCache::new(runner, PROBE_TIMEOUT);
        cache.refresh_blocking();
        let snap = cache.snapshot();

        let root = temp_test_dir("last-choice-per-file");
        let mut file_a = TerminalState::for_tab(1);
        file_a.bind_library_file(&root, "file-a", "win-a");
        file_a.record_last_cli(AgentCliPreset::Codex);

        let mut file_b = TerminalState::for_tab(2);
        file_b.bind_library_file(&root, "file-b", "win-b");

        let rows_a = build_picker_rows(&snap, file_a.session_registry.last_cli.as_deref(), &[]);
        let rows_b = build_picker_rows(&snap, file_b.session_registry.last_cli.as_deref(), &[]);

        let labels_a: Vec<&str> = rows_a.iter().take(4).map(|r| r.label.as_str()).collect();
        let labels_b: Vec<&str> = rows_b.iter().take(4).map(|r| r.label.as_str()).collect();

        assert_eq!(labels_a, vec!["Codex", "Claude", "Grok", "Antigravity"]);
        assert!(rows_a[0].is_last_choice);
        assert_eq!(labels_b, vec!["Claude", "Codex", "Grok", "Antigravity"]);
        assert!(!rows_b[0].is_last_choice);

        file_a.shutdown();
        file_b.shutdown();
        let _ = std::fs::remove_dir_all(&root);
    }
}
